use super::nav::{EdgeKind, NavGraph};
use crate::data::{Category, ObjectKind, PickupKind, TileKind, WeaponDef};
use crate::sim::physics::line_of_sight;
use crate::sim::{DT, Difficulty, Fighter, Input, Rng, World};
use macroquad::math::{Vec2, vec2};
use std::f32::consts::PI;

/// How far a bot can see, in tiles.
const SIGHT_RANGE: f32 = 36.0;
/// Below this health a bot goes for health or backs off.
const LOW_HEALTH: f32 = 40.0;
/// How long a bot keeps chasing a target it lost sight of.
const MEMORY_S: f32 = 2.5;

/// Everything that makes one difficulty feel different from another.
struct Params {
    /// Random delay before noticing a newly seen enemy (seconds, min..max).
    reaction: (f32, f32),
    /// Random aim wobble, in degrees.
    aim_error_deg: f32,
    /// How much target movement is led (0..1).
    lead: f32,
    /// Random hop chance per second while fighting.
    jumpy: f32,
    /// 0 aims at the chest, 1 at the head.
    head_bias: f32,
    /// Half-width of the view cone, in degrees. Enemies outside it are unseen.
    fov_half_deg: f32,
    /// How fast the aim can turn, in radians per second.
    turn_rate: f32,
    /// Enemies this close are noticed even from behind (footsteps).
    hear_range: f32,
    /// Automatic weapons fire in bursts: (hold, pause) seconds.
    burst: (f32, f32),
    /// How often a bot stops to stand or crouch (per second while roaming).
    idle_rate: f32,
}

fn params(d: Difficulty) -> Params {
    match d {
        Difficulty::Easy => Params {
            reaction: (0.9, 1.4),
            aim_error_deg: 14.0,
            lead: 0.0,
            jumpy: 0.05,
            head_bias: 0.0,
            fov_half_deg: 50.0,
            turn_rate: 2.5,
            hear_range: 3.0,
            burst: (0.5, 0.8),
            idle_rate: 0.18,
        },
        Difficulty::Normal => Params {
            reaction: (0.55, 0.85),
            aim_error_deg: 8.0,
            lead: 0.45,
            jumpy: 0.15,
            head_bias: 0.15,
            fov_half_deg: 60.0,
            turn_rate: 4.5,
            hear_range: 4.0,
            burst: (0.8, 0.5),
            idle_rate: 0.12,
        },
        Difficulty::Hard => Params {
            reaction: (0.3, 0.5),
            aim_error_deg: 4.0,
            lead: 0.85,
            jumpy: 0.35,
            head_bias: 0.4,
            fov_half_deg: 70.0,
            turn_rate: 8.0,
            hear_range: 5.0,
            burst: (1.2, 0.3),
            idle_rate: 0.07,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Goal {
    Wander(usize),
    Pickup(usize),
    Hunt(usize),
    Retreat(usize),
    /// A station or the powerup spot (index into `World::objects`).
    Object(usize),
}

/// What a bot does with its legs while in a fight.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Stance {
    Strafe,
    /// Stand still (optionally crouched) and shoot.
    Hold {
        crouch: bool,
    },
}

pub(crate) struct Brain {
    id: usize,
    rng: Rng,
    p: Params,
    goal: Goal,
    goal_timer: f32,
    path: Vec<(usize, EdgeKind)>,
    replan: f32,
    /// Enemy the bot is aware of and fighting.
    target: Option<usize>,
    /// Is the target in view right now?
    target_visible: bool,
    seen_for: f32,
    memory: f32,
    last_seen: Vec2,
    /// Enemy being noticed, and the time left before the bot reacts.
    noticing: Option<(usize, f32)>,
    /// Current aim direction, in radians; turns at a limited speed.
    aim_angle: f32,
    jitter: f32,
    jitter_timer: f32,
    stance: Stance,
    stance_timer: f32,
    strafe: f32,
    burst_on: bool,
    burst_timer: f32,
    /// Seconds left standing still while roaming.
    idle_left: f32,
    idle_crouch: bool,
    /// Seconds left looking the other way during an idle glance.
    glance_left: f32,
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
            target_visible: false,
            seen_for: 0.0,
            memory: 0.0,
            last_seen: Vec2::ZERO,
            noticing: None,
            aim_angle: 0.0,
            jitter: 0.0,
            jitter_timer: 0.0,
            stance: Stance::Strafe,
            stance_timer: 0.0,
            strafe: 1.0,
            burst_on: true,
            burst_timer: 0.0,
            idle_left: 0.0,
            idle_crouch: false,
            glance_left: 0.0,
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
            self.noticing = None;
            self.memory = 0.0;
            self.idle_left = 0.0;
            self.aim_angle = if me.facing < 0.0 { PI } else { 0.0 };
        }

        self.perceive(world, me);
        let target = self.target;
        let nearest = world
            .fighters
            .iter()
            .filter(|o| o.alive && !world.allies(self.id, o.id))
            .min_by(|a, b| {
                me.feet
                    .distance(a.feet)
                    .total_cmp(&me.feet.distance(b.feet))
            })
            .map(|o| o.id);

        // Commit to a goal for a few seconds instead of re-deciding constantly
        // (which made bots walk back and forth). Re-decide early only when the
        // goal is done or gone, or health just got low.
        self.goal_timer -= DT;
        let hurt =
            me.health < LOW_HEALTH && !matches!(self.goal, Goal::Pickup(_) | Goal::Retreat(_));
        if self.goal_timer <= 0.0 || hurt || !self.goal_valid(world, nav) {
            let goal = self.choose_goal(world, nav, target, nearest);
            if goal != self.goal {
                self.path.clear();
                self.replan = 0.0;
            }
            self.goal = goal;
            self.goal_timer = self.rng.range(2.5, 4.0);
        }

        let eye = me.eye();
        let mut input = Input::default();
        let def = &world.data.weapons[me.current];

        let fighting = matches!(self.goal, Goal::Hunt(_) | Goal::Wander(_));
        match (fighting, target) {
            (true, Some(t))
                if self.target_visible
                    && eye.distance(world.fighters[t].center()) < preferred_range(def) + 6.0 =>
            {
                self.combat_move(world, me, t, def, &mut input)
            }
            (true, Some(_)) if !self.target_visible => {
                // Lost sight: head for where they were last seen.
                let dest = self.last_seen;
                self.follow(world, nav, me, dest, &mut input);
            }
            _ => {
                let dest = self.goal_dest(world, nav);
                self.follow(world, nav, me, dest, &mut input);
                self.maybe_idle(me, &mut input);
            }
        }

        // Where the bot wants to look; the real aim turns toward it gradually.
        let want = match (target, self.target_visible) {
            (Some(t), true) => Some(self.aim_point(world, me, t, def)),
            (Some(_), false) => Some(self.last_seen - vec2(0.0, 1.0)),
            _ => None,
        };
        let desired = match want {
            Some(p) => {
                let d = p - eye;
                d.y.atan2(d.x)
            }
            None => self.idle_look(me, &input),
        };
        self.turn_toward(desired);
        let dir = vec2(self.aim_angle.cos(), self.aim_angle.sin());
        input.aim = eye + dir * 6.0;

        if let (Some(t), true) = (target, self.target_visible) {
            self.fire_control(world, me, t, def, desired, &mut input);
        } else if def.uses_ammo() && me.inventory[me.current].mag < def.magazine / 2 {
            input.reload = true;
        }

        self.pick_weapon(
            world,
            me,
            target.filter(|_| self.target_visible),
            &mut input,
        );

        // Shadowstep away when a fight is going badly.
        if me.teleport_charges > 0 && me.health < 35.0 && self.target_visible {
            input.ability = true;
        }

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

    /// Can this bot currently see enemy `o`? Only inside its view cone, unless
    /// the enemy is close enough to hear or has just shot it.
    fn can_see(&self, world: &World, me: &Fighter, o: &Fighter) -> bool {
        let eye = me.eye();
        let to = o.center() - eye;
        let dist = to.length();
        if dist > SIGHT_RANGE || !line_of_sight(&world.map, eye, o.center()) {
            return false;
        }
        let facing = vec2(self.aim_angle.cos(), self.aim_angle.sin());
        let in_cone = facing.angle_between(to).abs().to_degrees() <= self.p.fov_half_deg;
        let shot_by = me
            .last_attacker
            .is_some_and(|(a, t)| a == o.id && world.time - t < 1.0);
        in_cone || dist <= self.p.hear_range || shot_by
    }

    /// Update awareness: notice new enemies after a human-like delay, and
    /// remember a lost target for a little while.
    fn perceive(&mut self, world: &World, me: &Fighter) {
        let eye = me.eye();
        let mut seen: Vec<&Fighter> = world
            .fighters
            .iter()
            .filter(|o| o.alive && !world.allies(self.id, o.id) && self.can_see(world, me, o))
            .collect();
        seen.sort_by(|a, b| {
            eye.distance(a.center())
                .total_cmp(&eye.distance(b.center()))
        });

        // Keep fighting the current target while it stays in view.
        if let Some(t) = self.target
            && let Some(o) = seen.iter().find(|o| o.id == t)
        {
            self.target_visible = true;
            self.seen_for += DT;
            self.memory = MEMORY_S;
            self.last_seen = o.feet;
            self.noticing = None;
            return;
        }
        self.target_visible = false;
        if let Some(t) = self.target {
            self.memory -= DT;
            if self.memory <= 0.0 || !world.fighters[t].alive {
                self.target = None;
            }
        }

        // Notice the nearest enemy in view, after the reaction delay.
        let Some(c) = seen.first().map(|o| o.id) else {
            self.noticing = None;
            return;
        };
        match self.noticing {
            Some((n, left)) if n == c => {
                let left = left - DT;
                if left <= 0.0 {
                    self.target = Some(c);
                    self.target_visible = true;
                    self.seen_for = 0.0;
                    self.memory = MEMORY_S;
                    self.last_seen = world.fighters[c].feet;
                    self.noticing = None;
                    self.idle_left = 0.0;
                } else {
                    self.noticing = Some((n, left));
                }
            }
            _ => {
                let (lo, hi) = self.p.reaction;
                self.noticing = Some((c, self.rng.range(lo, hi)));
            }
        }
    }

    /// Turn the aim toward `desired` (radians) no faster than the turn rate.
    fn turn_toward(&mut self, desired: f32) {
        let mut diff = (desired - self.aim_angle) % (2.0 * PI);
        if diff > PI {
            diff -= 2.0 * PI;
        } else if diff < -PI {
            diff += 2.0 * PI;
        }
        let max = self.p.turn_rate * DT;
        self.aim_angle += diff.clamp(-max, max);
    }

    /// Where to look with no enemy in mind: straight ahead along the way we
    /// are going, or mostly ahead while idle with an occasional glance back.
    fn idle_look(&mut self, me: &Fighter, input: &Input) -> f32 {
        let ahead = if input.move_x != 0.0 {
            input.move_x
        } else {
            me.facing
        };
        let base = if ahead < 0.0 { PI } else { 0.0 };
        if self.glance_left > 0.0 {
            self.glance_left -= DT;
            return PI - base;
        }
        base
    }

    /// While roaming, sometimes stop for a moment, standing or crouched.
    fn maybe_idle(&mut self, me: &Fighter, input: &mut Input) {
        let urgent = matches!(self.goal, Goal::Retreat(_)) || me.health < LOW_HEALTH;
        if self.idle_left > 0.0 && !urgent && me.on_ground {
            self.idle_left -= DT;
            input.move_x = 0.0;
            input.jump = false;
            input.crouch = self.idle_crouch;
            // Mostly stare ahead; now and then a quick look around.
            if self.glance_left <= 0.0 && self.rng.chance(0.08 * DT) {
                self.glance_left = self.rng.range(0.6, 1.2);
            }
            return;
        }
        if !urgent && me.on_ground && self.rng.chance(self.p.idle_rate * DT) {
            self.idle_left = self.rng.range(0.6, 2.4);
            self.idle_crouch = self.rng.chance(0.3);
        }
    }

    /// The best station or powerup worth walking to right now.
    fn wanted_object(&self, world: &World, armed: bool) -> Option<usize> {
        let me = &world.fighters[self.id];
        (0..world.objects.len())
            .filter(|&k| world.objects[k].ready)
            .filter_map(|k| {
                let o = &world.objects[k];
                let dist = me.feet.distance(o.pos);
                let worth = match o.def.kind {
                    ObjectKind::PowerupSpot if o.powerup.is_some() => 60.0,
                    ObjectKind::ShieldStation if me.shield < 50.0 => 30.0,
                    ObjectKind::WeaponStation if o.weapon.is_some_and(|w| !me.owns(w)) => {
                        if armed {
                            15.0
                        } else {
                            35.0
                        }
                    }
                    _ => return None,
                };
                (dist < worth).then_some((k, dist / worth))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    fn goal_valid(&self, world: &World, nav: &NavGraph) -> bool {
        match self.goal {
            Goal::Pickup(k) => world.pickups[k].active,
            Goal::Object(k) => world.objects[k].ready,
            Goal::Hunt(j) => world.fighters[j].alive,
            Goal::Wander(n) | Goal::Retreat(n) => {
                world.fighters[self.id].feet.distance(nav.pos(n)) > 1.0
            }
        }
    }

    fn goal_dest(&self, world: &World, nav: &NavGraph) -> Vec2 {
        match self.goal {
            Goal::Pickup(k) => world.pickups[k].pos + vec2(0.0, 0.4),
            Goal::Object(k) => world.objects[k].pos,
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

        if let Some(k) = self.wanted_object(world, armed) {
            return Goal::Object(k);
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
                let double_jump =
                    !me.on_ground && me.air_jumps > 0 && me.vel.y > -3.0 && p.y < me.feet.y - 0.8;
                input.jump = first_jump || double_jump;
            }
            EdgeKind::Thrust => {
                // The pad does the launching; steer and add a double jump if short.
                input.jump_held = true;
                input.jump =
                    !me.on_ground && me.air_jumps > 0 && me.vel.y > -3.0 && p.y < me.feet.y - 0.8;
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
        // Alternate between strafing and holding still (sometimes crouched).
        self.stance_timer -= DT;
        if self.stance_timer <= 0.0 {
            if self.rng.chance(0.4) {
                self.stance = Stance::Hold {
                    crouch: self.rng.chance(0.4),
                };
                self.stance_timer = self.rng.range(0.5, 1.6);
            } else {
                self.stance = Stance::Strafe;
                self.strafe = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                self.stance_timer = self.rng.range(0.6, 1.5);
            }
        }
        let toward = (them.feet.x - me.feet.x).signum();
        input.move_x = if dist > pref + 3.0 {
            toward
        } else if dist < pref - 3.0 {
            -toward
        } else {
            match self.stance {
                Stance::Strafe => self.strafe,
                Stance::Hold { crouch } => {
                    input.crouch = crouch && me.on_ground;
                    0.0
                }
            }
        };
        if self.hazard_ahead(world, me, input.move_x) {
            input.move_x = -input.move_x;
            self.strafe = input.move_x;
        }
        if me.on_ground && !input.crouch && self.rng.chance(self.p.jumpy * DT) {
            input.jump = true;
        }
        input.jump_held = true;
    }

    /// The point a bot tries to hit: chest or head, led by the target's motion,
    /// plus a slowly changing wobble.
    fn aim_point(&mut self, world: &World, me: &Fighter, t: usize, def: &WeaponDef) -> Vec2 {
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
            self.jitter_timer = self.rng.range(0.25, 0.5);
        }
        let d = point - eye;
        let a = d.y.atan2(d.x) + self.jitter;
        eye + vec2(a.cos(), a.sin()) * d.length().max(1.0)
    }

    /// Shoot only once the aim has actually swung onto the target, in bursts
    /// for automatic weapons.
    fn fire_control(
        &mut self,
        world: &World,
        me: &Fighter,
        t: usize,
        def: &WeaponDef,
        desired: f32,
        input: &mut Input,
    ) {
        let eye = me.eye();
        let dist = eye.distance(world.fighters[t].center());
        let mut off = (desired - self.aim_angle).abs() % (2.0 * PI);
        if off > PI {
            off = 2.0 * PI - off;
        }
        let on_target = off.to_degrees() < 8.0;
        let safe = def
            .projectile
            .explosion
            .as_ref()
            .is_none_or(|e| dist > e.radius_tiles * 0.9);

        // Burst rhythm for fast-firing weapons.
        if def.fire_rate_rps >= 5.0 {
            self.burst_timer -= DT;
            if self.burst_timer <= 0.0 {
                self.burst_on = !self.burst_on;
                let (hold, pause) = self.p.burst;
                self.burst_timer = if self.burst_on {
                    self.rng.range(hold * 0.6, hold * 1.4)
                } else {
                    self.rng.range(pause * 0.6, pause * 1.4)
                };
            }
        } else {
            self.burst_on = true;
        }

        if on_target && self.burst_on && dist <= def.range_tiles * 1.05 && safe {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::sim::MatchConfig;
    use std::sync::Arc;

    fn duel(difficulty: Difficulty) -> (World, NavGraph) {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let data = Arc::new(GameData::load(&assets).unwrap());
        let cfg = MatchConfig {
            map_id: "skyforge_yard".into(),
            enemy_bots: 1,
            difficulty,
            ..Default::default()
        };
        let mut w = World::new(data, cfg).unwrap();
        // Bot and player on the same open floor, ten tiles apart.
        w.fighters[1].feet = vec2(30.5, 23.0);
        w.fighters[0].feet = vec2(20.5, 23.0);
        let nav = NavGraph::build(&w.map);
        (w, nav)
    }

    fn watch(brain: &mut Brain, w: &World, nav: &NavGraph, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            brain.perceive(w, &w.fighters[brain.id]);
            let _ = nav;
        }
    }

    #[test]
    fn bots_do_not_see_behind_them() {
        let (w, nav) = duel(Difficulty::Hard);
        let mut b = Brain::new(1, 1, Difficulty::Hard);
        b.aim_angle = 0.0; // facing right, the player is to the left
        watch(&mut b, &w, &nav, 2.0);
        assert_eq!(b.target, None);
    }

    #[test]
    fn bots_notice_after_a_reaction_delay() {
        let (w, nav) = duel(Difficulty::Easy);
        let mut b = Brain::new(1, 1, Difficulty::Easy);
        b.aim_angle = PI; // facing the player
        watch(&mut b, &w, &nav, 0.8);
        assert_eq!(b.target, None, "easy bots need at least 0.9 s to react");
        watch(&mut b, &w, &nav, 0.7);
        assert_eq!(b.target, Some(0));
    }
}
