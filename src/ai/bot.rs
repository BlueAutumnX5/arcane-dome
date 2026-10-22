use super::nav::{EdgeKind, NavGraph};
use crate::data::{Category, PickupKind, TileKind, WeaponDef};
use crate::sim::physics::line_of_sight;
use crate::sim::{DT, Difficulty, Fighter, Input, Rng, World};
use macroquad::math::{Vec2, vec2};

/// How far a bot notices enemies, in tiles.
const SIGHT_RANGE: f32 = 40.0;
/// Below this health a bot goes for health or backs off.
const LOW_HEALTH: f32 = 40.0;

struct Params {
    /// Seconds a target must be visible before firing.
    reaction: f32,
    /// Random aim wobble, in degrees.
    aim_error_deg: f32,
    /// How much target movement is led (0..1).
    lead: f32,
    /// Random jumps per second while fighting.
    jumpy: f32,
    /// 0 aims at the chest, 1 at the head.
    head_bias: f32,
}

fn params(d: Difficulty) -> Params {
    match d {
        Difficulty::Easy => Params {
            reaction: 0.7,
            aim_error_deg: 9.0,
            lead: 0.3,
            jumpy: 0.2,
            head_bias: 0.0,
        },
        Difficulty::Normal => Params {
            reaction: 0.4,
            aim_error_deg: 5.0,
            lead: 0.7,
            jumpy: 0.5,
            head_bias: 0.3,
        },
        Difficulty::Hard => Params {
            reaction: 0.2,
            aim_error_deg: 2.0,
            lead: 1.0,
            jumpy: 0.9,
            head_bias: 0.6,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Goal {
    Wander(usize),
    Pickup(usize),
    Hunt(usize),
    Retreat(usize),
}

pub(crate) struct Brain {
    id: usize,
    rng: Rng,
    p: Params,
    goal: Goal,
    goal_timer: f32,
    path: Vec<(usize, EdgeKind)>,
    replan: f32,
    target: Option<usize>,
    seen_for: f32,
    jitter: f32,
    jitter_timer: f32,
    strafe: f32,
    strafe_timer: f32,
    stuck: f32,
    last_x: f32,
    weapon_timer: f32,
    was_alive: bool,
}

impl Brain {
    pub fn new(id: usize, seed: u64, difficulty: Difficulty) -> Self {
        Self {
            id,
            rng: Rng::new(seed ^ (id as u64 + 1).wrapping_mul(0x9E37_79B9)),
            p: params(difficulty),
            goal: Goal::Wander(0),
            goal_timer: 0.0,
            path: Vec::new(),
            replan: 0.0,
            target: None,
            seen_for: 0.0,
            jitter: 0.0,
            jitter_timer: 0.0,
            strafe: 1.0,
            strafe_timer: 0.0,
            stuck: 0.0,
            last_x: 0.0,
            weapon_timer: 0.0,
            was_alive: false,
        }
    }

    pub fn think(&mut self, world: &World, nav: &NavGraph) -> Input {
        let me = &world.fighters[self.id];
        if !me.alive {
            self.was_alive = false;
            self.path.clear();
            return Input {
                aim: me.eye() + vec2(me.facing, 0.0),
                ..Default::default()
            };
        }
        if !self.was_alive {
            self.was_alive = true;
            self.goal_timer = 0.0;
            self.replan = 0.0;
            self.target = None;
        }

        let eye = me.eye();
        let enemies: Vec<&Fighter> = world
            .fighters
            .iter()
            .filter(|o| o.alive && !world.allies(self.id, o.id))
            .collect();
        let target = enemies
            .iter()
            .filter(|o| {
                eye.distance(o.center()) < SIGHT_RANGE && line_of_sight(&world.map, eye, o.center())
            })
            .min_by(|a, b| {
                eye.distance(a.center())
                    .total_cmp(&eye.distance(b.center()))
            })
            .map(|o| o.id);
        let nearest = enemies
            .iter()
            .min_by(|a, b| {
                me.feet
                    .distance(a.feet)
                    .total_cmp(&me.feet.distance(b.feet))
            })
            .map(|o| o.id);
        if target != self.target {
            self.target = target;
            self.seen_for = 0.0;
        }
        if target.is_some() {
            self.seen_for += DT;
        }

        self.goal_timer -= DT;
        if self.goal_timer <= 0.0 || !self.goal_valid(world, nav) {
            let goal = self.choose_goal(world, nav, target, nearest);
            if goal != self.goal {
                self.path.clear();
                self.replan = 0.0;
            }
            self.goal = goal;
            self.goal_timer = 0.7;
        }

        let mut input = Input {
            aim: eye + vec2(me.facing * 6.0, 0.0),
            ..Default::default()
        };
        let def = &world.data.weapons[me.current];

        let fighting = matches!(self.goal, Goal::Hunt(_) | Goal::Wander(_));
        match (fighting, target) {
            (true, Some(t))
                if eye.distance(world.fighters[t].center()) < preferred_range(def) + 6.0 =>
            {
                self.combat_move(world, me, t, def, &mut input)
            }
            _ => {
                let dest = self.goal_dest(world, nav);
                self.follow(world, nav, me, dest, &mut input);
            }
        }

        if let Some(t) = target {
            self.aim_and_fire(world, me, t, def, &mut input);
        } else if def.uses_ammo() && me.inventory[me.current].mag < def.magazine / 2 {
            input.reload = true;
        }

        self.pick_weapon(world, me, target, &mut input);

        // Unstick: jump when pushing against something, then replan.
        if input.move_x != 0.0 && (me.feet.x - self.last_x).abs() < 0.01 {
            self.stuck += DT;
        } else {
            self.stuck = 0.0;
        }
        self.last_x = me.feet.x;
        if self.stuck > 0.4 {
            input.jump = me.on_ground || me.vel.y > -2.0;
            input.jump_held = true;
            if self.stuck > 1.5 {
                self.stuck = 0.0;
                self.path.clear();
                self.replan = 0.0;
                self.goal_timer = 0.0;
            }
        }
        input
    }

    fn goal_valid(&self, world: &World, nav: &NavGraph) -> bool {
        match self.goal {
            Goal::Pickup(k) => world.pickups[k].active,
            Goal::Hunt(j) => world.fighters[j].alive,
            Goal::Wander(n) | Goal::Retreat(n) => {
                world.fighters[self.id].feet.distance(nav.pos(n)) > 1.0
            }
        }
    }

    fn goal_dest(&self, world: &World, nav: &NavGraph) -> Vec2 {
        match self.goal {
            Goal::Pickup(k) => world.pickups[k].pos + vec2(0.0, 0.4),
            Goal::Hunt(j) => world.fighters[j].feet,
            Goal::Wander(n) | Goal::Retreat(n) => nav.pos(n),
        }
    }

    fn choose_goal(
        &mut self,
        world: &World,
        nav: &NavGraph,
        target: Option<usize>,
        nearest: Option<usize>,
    ) -> Goal {
        let me = &world.fighters[self.id];
        let closest_pickup = |want: &dyn Fn(usize) -> bool| {
            (0..world.pickups.len())
                .filter(|&k| world.pickups[k].active && want(k))
                .min_by(|&a, &b| {
                    me.feet
                        .distance(world.pickups[a].pos)
                        .total_cmp(&me.feet.distance(world.pickups[b].pos))
                })
        };

        if me.health < LOW_HEALTH {
            if let Some(k) = closest_pickup(&|k| world.pickups[k].def.kind == PickupKind::Health) {
                return Goal::Pickup(k);
            }
            if let Some(t) = target
                && !nav.nodes.is_empty()
            {
                let from = world.fighters[t].feet;
                let best = (0..12)
                    .map(|_| self.rng.below(nav.nodes.len()))
                    .max_by(|&a, &b| {
                        let score = |n: usize| {
                            nav.pos(n).distance(from) - 0.4 * nav.pos(n).distance(me.feet)
                        };
                        score(a).total_cmp(&score(b))
                    });
                if let Some(n) = best {
                    return Goal::Retreat(n);
                }
            }
        }

        let armed = me.inventory.iter().enumerate().any(|(w, s)| {
            let d = &world.data.weapons[w];
            s.owned && d.category != Category::Starter && (s.mag > 0 || s.reserve > 0)
        });
        if !armed
            && let Some(k) = closest_pickup(&|k| {
                let p = &world.pickups[k];
                p.def.kind == PickupKind::Weapon
                    && p.weapon.is_some_and(|w| !me.owns(w))
                    && p.pos.distance(me.feet) < 35.0
            })
        {
            return Goal::Pickup(k);
        }
        if !armed
            && let Some(k) = closest_pickup(&|k| {
                let p = &world.pickups[k];
                p.def.kind == PickupKind::Ammo && p.pos.distance(me.feet) < 20.0
            })
        {
            return Goal::Pickup(k);
        }

        if let Some(t) = nearest {
            return Goal::Hunt(t);
        }
        Goal::Wander(self.rng.below(nav.nodes.len().max(1)))
    }

    /// Follow the nav graph toward `dest`.
    fn follow(
        &mut self,
        world: &World,
        nav: &NavGraph,
        me: &Fighter,
        dest: Vec2,
        input: &mut Input,
    ) {
        self.replan -= DT;
        if (self.replan <= 0.0 || self.path.is_empty()) && me.on_ground {
            self.replan = 0.5;
            self.path = match (nav.nearest(me.feet), nav.nearest(dest)) {
                (Some(a), Some(b)) => nav.find_path(a, b).unwrap_or_default(),
                _ => Vec::new(),
            };
        }
        while let Some(&(n, _)) = self.path.first() {
            let p = nav.pos(n);
            if me.on_ground && (me.feet.x - p.x).abs() < 0.3 && (me.feet.y - p.y).abs() < 0.3 {
                self.path.remove(0);
            } else {
                break;
            }
        }

        let Some(&(n, kind)) = self.path.first() else {
            let dx = dest.x - me.feet.x;
            if dx.abs() > 0.3 {
                input.move_x = dx.signum();
            }
            return;
        };
        let p = nav.pos(n);
        let dx = p.x - me.feet.x;
        input.move_x = if dx.abs() > 0.1 { dx.signum() } else { 0.0 };
        let higher = p.y < me.feet.y - 0.2;
        match kind {
            EdgeKind::Jump => {
                input.jump_held = true;
                let first_jump = me.on_ground && (higher || dx.abs() > 1.2);
                // Double jump near the apex when the target is still above.
                let double_jump = !me.on_ground
                    && me.air_jumps > 0
                    && me.vel.y > -3.0
                    && p.y < me.feet.y - 0.8;
                input.jump = first_jump || double_jump;
            }
            EdgeKind::Drop => {
                if me.on_ground {
                    input.crouch = true;
                    input.jump = true;
                    input.move_x = 0.0;
                }
            }
            EdgeKind::Walk | EdgeKind::Fall => {
                if self.hazard_ahead(world, me, input.move_x) {
                    input.jump = me.on_ground;
                    input.jump_held = true;
                }
            }
        }
    }

    fn hazard_ahead(&self, world: &World, me: &Fighter, dir: f32) -> bool {
        if dir == 0.0 {
            return false;
        }
        let x = (me.feet.x + dir * 0.9).floor() as i32;
        let y = (me.feet.y - 0.5).floor() as i32;
        world.map.tile(x, y) == TileKind::Spikes
    }

    fn combat_move(
        &mut self,
        world: &World,
        me: &Fighter,
        t: usize,
        def: &WeaponDef,
        input: &mut Input,
    ) {
        let them = &world.fighters[t];
        let dist = me.feet.distance(them.feet);
        let pref = preferred_range(def);
        self.strafe_timer -= DT;
        if self.strafe_timer <= 0.0 {
            self.strafe = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            self.strafe_timer = self.rng.range(0.4, 1.1);
        }
        let toward = (them.feet.x - me.feet.x).signum();
        input.move_x = if dist > pref + 3.0 {
            toward
        } else if dist < pref - 3.0 {
            -toward
        } else {
            self.strafe
        };
        if self.hazard_ahead(world, me, input.move_x) {
            input.move_x = -input.move_x;
            self.strafe = input.move_x;
        }
        if me.on_ground && self.rng.chance(self.p.jumpy * DT) {
            input.jump = true;
        }
        input.jump_held = true;
    }

    fn aim_and_fire(
        &mut self,
        world: &World,
        me: &Fighter,
        t: usize,
        def: &WeaponDef,
        input: &mut Input,
    ) {
        let them = &world.fighters[t];
        let eye = me.eye();
        let body = them.aabb();
        let mut point = body.center();
        point.y += (body.min.y + 0.2 - point.y) * self.p.head_bias;
        let pd = &def.projectile;
        if pd.kind.is_physical() && pd.speed > 0.0 {
            let tt = eye.distance(point) / pd.speed;
            point += them.vel * tt * self.p.lead;
            point.y -= 0.5 * pd.gravity * tt * tt;
        }
        self.jitter_timer -= DT;
        if self.jitter_timer <= 0.0 {
            self.jitter = self.rng.range(-1.0, 1.0) * self.p.aim_error_deg.to_radians();
            self.jitter_timer = 0.25;
        }
        let d = point - eye;
        let a = d.y.atan2(d.x) + self.jitter;
        input.aim = eye + vec2(a.cos(), a.sin()) * d.length().max(1.0);

        let dist = eye.distance(body.center());
        let safe = pd
            .explosion
            .as_ref()
            .is_none_or(|e| dist > e.radius_tiles * 0.9);
        if self.seen_for >= self.p.reaction && dist <= def.range_tiles * 1.05 && safe {
            input.fire = true;
        }
        if def.uses_ammo() && me.inventory[me.current].mag == 0 {
            input.reload = true;
        }
    }

    fn pick_weapon(
        &mut self,
        world: &World,
        me: &Fighter,
        target: Option<usize>,
        input: &mut Input,
    ) {
        self.weapon_timer -= DT;
        if self.weapon_timer > 0.0 {
            return;
        }
        self.weapon_timer = 1.0;
        let dist = target.map_or(12.0, |t| me.feet.distance(world.fighters[t].feet));
        let best = (0..world.data.weapons.len())
            .filter(|&w| {
                let s = me.inventory[w];
                let d = &world.data.weapons[w];
                s.owned && (!d.uses_ammo() || s.mag > 0 || s.reserve > 0 || d.infinite_reserve())
            })
            .max_by(|&a, &b| {
                weapon_score(&world.data.weapons[a], dist)
                    .total_cmp(&weapon_score(&world.data.weapons[b], dist))
            });
        if let Some(w) = best
            && w != me.current
        {
            input.select = Some(w);
        }
    }
}

/// Distance a bot likes to keep from its target with this weapon.
fn preferred_range(def: &WeaponDef) -> f32 {
    match def.category {
        Category::CloseRange | Category::Melee => 4.0,
        Category::LongRange => 16.0,
        _ => (def.range_tiles * 0.4).clamp(5.0, 12.0),
    }
}

/// Rough usefulness of a weapon at a distance: damage per second, scaled
/// down when out of range or when its own blast would hit the shooter.
fn weapon_score(def: &WeaponDef, dist: f32) -> f32 {
    let splash = def.projectile.explosion.as_ref().map_or(0.0, |e| e.damage);
    let mut s = (def.damage * def.total_pellets() as f32 + splash) * def.fire_rate_rps;
    if dist > def.range_tiles {
        s *= 0.2;
    }
    if let Some(e) = &def.projectile.explosion
        && dist < e.radius_tiles + 1.0
    {
        s *= 0.1;
    }
    if def.category == Category::Starter {
        s *= 0.8;
    }
    s
}
