use super::nav::{EdgeKind, NavGraph};
use crate::data::{Category, ObjectKind, PickupKind, WeaponDef};
use crate::sim::physics::{hazard_dps, line_of_sight, standing_on_hatch};
use crate::sim::{DT, Difficulty, Event, Fighter, Input, Rng, World};
use macroquad::math::{Vec2, vec2};
use std::f32::consts::PI;

/// How far a bot can see, in tiles.
const SIGHT_RANGE: f32 = 36.0;
/// Below this health a bot goes for health or backs off.
const LOW_HEALTH: f32 = 40.0;
/// How hard an airborne bot corrects toward its landing spot (1/s).
const AIR_STEER: f32 = 6.0;
const RUN_SPEED: f32 = 9.0;
const STAND_HEIGHT: f32 = 1.8;
/// How far above the higher end of a jump the bot rises before letting go.
const JUMP_APEX: f32 = 1.1;
/// About how high a full jump rises.
const FULL_JUMP_RISE: f32 = 3.2;
/// Highest a hazard escape aims, above the feet.
const ESCAPE_RISE: f32 = 2.6;
/// A hunting bot turns toward its quarry once this close.
const HUNT_LOOK: f32 = 6.0;
/// How long a bot keeps chasing a target it lost sight of.
const MEMORY_S: f32 = 2.5;
/// A shot needs at least this much drop (tiles) or flight time (seconds)
/// before a bot judges its range; quicker shots just go where aimed.
const RANGED_DROP: f32 = 0.4;
const RANGED_FLIGHT_S: f32 = 0.3;
/// Largest range correction, as a share of the distance.
const RANGE_FIX_MAX: f32 = 0.45;
/// Lobbed and long shots wobble this much of the usual amount.
const LOB_STEADY: f32 = 0.35;
/// A shot ending this close to its target counts as a hit: nothing to fix.
const HIT_NEAR: f32 = 1.5;
/// A shot that came back past its shooter (it hit something overhead)
/// counts as this much too high, as a share of the distance.
const BOUNCED_BACK: f32 = 0.2;
/// How fast the first-swing overshoot settles (seconds to fade to ~37%).
const SETTLE_S: f32 = 0.35;
/// How long being hit shakes the aim.
const FLINCH_S: f32 = 0.6;

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
    /// In a fight, the chance each footwork change is holding still rather
    /// than strafing.
    hold_chance: f32,
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
    /// While roaming, a bot walks for this long (seconds, min..max), then
    /// stops a moment to stand or crouch.
    travel: (f32, f32),
    /// Chance to break away from a fight (instead of committing to it)
    /// each time the bot sizes one up.
    break_away: f32,
    /// Lobbed and long shots: the chance the first shot at a new target is
    /// misjudged, and how badly (share of the distance, min..max).
    misjudge_chance: f32,
    misjudge: (f32, f32),
    /// Share of a seen miss the next shot corrects.
    correct: f32,
    /// First swing onto a new target overshoots by up to this, in degrees.
    settle_deg: f32,
    /// Aim wobble is multiplied by this just after being hit.
    flinch: f32,
}

fn params(d: Difficulty) -> Params {
    match d {
        Difficulty::Easy => Params {
            reaction: (0.7, 1.1),
            aim_error_deg: 14.0,
            lead: 0.0,
            jumpy: 0.05,
            hold_chance: 0.6,
            head_bias: 0.0,
            fov_half_deg: 56.0,
            turn_rate: 3.2,
            hear_range: 3.0,
            burst: (0.5, 0.8),
            travel: (2.0, 4.5),
            break_away: 0.3,
            misjudge_chance: 0.9,
            misjudge: (0.25, 0.5),
            correct: 0.45,
            settle_deg: 18.0,
            flinch: 2.0,
        },
        Difficulty::Normal => Params {
            reaction: (0.55, 0.85),
            aim_error_deg: 8.0,
            lead: 0.45,
            jumpy: 0.15,
            hold_chance: 0.4,
            head_bias: 0.15,
            fov_half_deg: 60.0,
            turn_rate: 4.5,
            hear_range: 4.0,
            burst: (0.8, 0.5),
            travel: (3.0, 6.0),
            break_away: 0.35,
            misjudge_chance: 0.7,
            misjudge: (0.15, 0.35),
            correct: 0.65,
            settle_deg: 11.0,
            flinch: 1.6,
        },
        Difficulty::Hard => Params {
            reaction: (0.3, 0.5),
            aim_error_deg: 4.0,
            lead: 0.85,
            jumpy: 0.35,
            hold_chance: 0.4,
            head_bias: 0.4,
            fov_half_deg: 70.0,
            turn_rate: 8.0,
            hear_range: 5.0,
            burst: (1.2, 0.3),
            travel: (4.5, 8.0),
            break_away: 0.25,
            misjudge_chance: 0.45,
            misjudge: (0.08, 0.2),
            correct: 0.85,
            settle_deg: 6.0,
            flinch: 1.3,
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
    /// Where a jump in progress should land; held until the bot touches down
    /// so it never changes its mind mid-air and drops into a pit.
    air_to: Option<Vec2>,
    /// Feet height where the current jump took off.
    jump_from_y: f32,
    /// The current jump must stay low (terrain overhead).
    low_jump: bool,
    /// Where a hunted enemy was when the hunt was (re)decided.
    hunt_spot: Vec2,
    /// The enemy currently being fought, and the time until the bot next
    /// decides whether to stay in the fight or break away.
    engaged: Option<usize>,
    engage_timer: f32,
    /// What the bot did this tick (debugging tools): fight, chase or roam.
    doing: &'static str,
    /// Seconds of walking left before the next stop while roaming.
    travel_left: f32,
    /// Last place the bot stood on solid, safe ground.
    ground_x: f32,
    weapon_timer: f32,
    was_alive: bool,
    /// Lobbed or long shots: the aim correction learned from this bot's own
    /// misses (per tile of distance), and the (target, weapon) it is for.
    range_fix: Vec2,
    range_for: Option<(usize, usize)>,
    /// Extra aim angle after spotting a target, fading as the aim settles.
    settle: f32,
    /// This bot's shots in flight, being judged.
    shots: Vec<Shot>,
    /// The current shot is lobbed or long (see `aim_point`).
    lobbing: bool,
}

/// One of a bot's own shots it is watching.
struct Shot {
    id: u32,
    /// Which side of the target it started on (crossing means it passed).
    side: f32,
    /// Vertical speed last tick (falling turning to rising is a landing).
    vy: f32,
    /// How it was judged (where it came down, or how it passed), applied
    /// when it goes off unless it went off on target.
    miss: Option<f32>,
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
            air_to: None,
            jump_from_y: 0.0,
            low_jump: false,
            hunt_spot: Vec2::ZERO,
            engaged: None,
            engage_timer: 0.0,
            doing: "roam",
            travel_left: 3.0,
            ground_x: 0.0,
            weapon_timer: 0.0,
            was_alive: false,
            range_fix: Vec2::ZERO,
            range_for: None,
            settle: 0.0,
            shots: Vec::new(),
            lobbing: false,
        }
    }

    pub fn landing_target(&self) -> Option<Vec2> {
        self.air_to
    }

    pub fn describe(&self, world: &World, nav: &NavGraph) -> String {
        let dest = self.goal_dest(world, nav);
        let path: Vec<String> = self
            .path
            .iter()
            .take(3)
            .map(|&(n, k)| {
                let p = nav.pos(n);
                format!("{k:?}({:.1},{:.1})", p.x, p.y)
            })
            .collect();
        format!(
            "idle {:.2} goal {:?} doing {} dest ({:.1},{:.1}) target {:?} visible {} air_to {:?} stuck {:.2} path [{}]",
            self.idle_left,
            self.goal,
            self.doing,
            dest.x,
            dest.y,
            self.target,
            self.target_visible,
            self.air_to,
            self.stuck,
            path.join(" ")
        )
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
            self.air_to = None;
            self.ground_x = me.feet.x;
            self.aim_angle = if me.facing < 0.0 { PI } else { 0.0 };
        }

        if me.on_ground {
            // A new jump from here sets its own landing spot.
            self.air_to = None;
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
            if let Goal::Hunt(j) = goal {
                // Head for where the quarry is now, not wherever it moves
                // next: following live positions through floors made two
                // bots chase each other's reflections forever.
                self.hunt_spot = world.fighters[j].feet;
            }
        }

        self.size_up_fight(world, nav, me);

        let eye = me.eye();
        let mut input = Input::default();
        let def = &world.data.weapons[me.current];

        let fighting = matches!(self.goal, Goal::Hunt(_) | Goal::Wander(_));
        match (fighting, target) {
            (true, Some(t))
                if self.target_visible
                    && eye.distance(world.fighters[t].center()) < preferred_range(def) + 6.0 =>
            {
                self.doing = "fight";
                self.idle_left = 0.0;
                self.combat_move(world, me, t, def, &mut input)
            }
            (true, Some(_)) if !self.target_visible => {
                // Lost sight: head for where they were last seen.
                self.doing = "chase";
                self.idle_left = 0.0;
                let dest = self.last_seen;
                self.follow(world, nav, me, dest, &mut input);
            }
            _ => {
                self.doing = "roam";
                let dest = self.goal_dest(world, nav);
                self.follow(world, nav, me, dest, &mut input);
                self.maybe_idle(me, &mut input);
            }
        }

        self.learn_from_shots(world, me);
        // Where the bot wants to look; the real aim turns toward it gradually.
        let want = match (target, self.target_visible) {
            (Some(t), true) => Some(self.aim_point(world, me, t, def)),
            (Some(_), false) => Some(self.last_seen - vec2(0.0, 1.0)),
            // Closing in on a hunted enemy: look where its footsteps are.
            _ => match self.goal {
                Goal::Hunt(j) if world.fighters[j].center().distance(eye) < HUNT_LOOK => {
                    Some(world.fighters[j].center())
                }
                _ => None,
            },
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

        self.escape_hazard(world, nav, me, &mut input);
        self.air_control(world, me, &mut input);

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

        // Sides can change (a new Juggernaut): never keep a friend as target.
        if self.target.is_some_and(|t| world.allies(self.id, t)) {
            self.target = None;
        }
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

        // Notice the nearest enemy in view, after the reaction delay. Keep
        // counting down on the one already being noticed while it stays in
        // view, even if another walks closer (or the countdown never ends).
        let Some(nearest) = seen.first().map(|o| o.id) else {
            self.noticing = None;
            return;
        };
        let c = match self.noticing {
            Some((n, _)) if seen.iter().any(|o| o.id == n) => n,
            _ => nearest,
        };
        match self.noticing {
            Some((n, left)) if n == c => {
                let left = left - DT;
                if left <= 0.0 {
                    // The first swing onto a new enemy overshoots a little.
                    let side = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                    self.settle = side * self.rng.range(0.5, 1.0) * self.p.settle_deg.to_radians();
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
    /// Roaming has a rhythm: walk a few seconds, stop a moment (sometimes
    /// crouched, mostly looking ahead), walk on.
    fn maybe_idle(&mut self, me: &Fighter, input: &mut Input) {
        let urgent = matches!(self.goal, Goal::Retreat(_)) || me.health < LOW_HEALTH;
        if urgent {
            self.idle_left = 0.0;
            return;
        }
        if self.idle_left > 0.0 {
            if !me.on_ground {
                // Mid-jump: finish it, then stop.
                return;
            }
            self.idle_left -= DT;
            input.move_x = 0.0;
            input.jump = false;
            input.crouch = self.idle_crouch;
            // Mostly stare ahead; now and then a quick look around.
            if self.glance_left <= 0.0 && self.rng.chance(0.08 * DT) {
                self.glance_left = self.rng.range(0.6, 1.2);
            }
            if self.idle_left <= 0.0 {
                let (lo, hi) = self.p.travel;
                self.travel_left = self.rng.range(lo, hi);
            }
            return;
        }
        self.travel_left -= DT;
        // Stop only on plain ground, not halfway into a jump or a drop.
        let walking = matches!(self.path.first(), None | Some((_, EdgeKind::Walk)));
        if self.travel_left <= 0.0 && me.on_ground && walking {
            self.idle_left = self.rng.range(1.0, 3.0);
            self.idle_crouch = self.rng.chance(0.4);
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
            Goal::Hunt(_) => self.hunt_spot,
            Goal::Wander(n) | Goal::Retreat(n) => nav.pos(n),
        }
    }

    /// A spot well away from `enemy` without running too far: the best of a
    /// few random nav nodes.
    fn retreat_node(&mut self, nav: &NavGraph, me: Vec2, enemy: Vec2) -> Option<usize> {
        if nav.nodes.is_empty() {
            return None;
        }
        (0..12)
            .map(|_| self.rng.below(nav.nodes.len()))
            .max_by(|&a, &b| {
                let score = |n: usize| nav.pos(n).distance(enemy) - 0.4 * nav.pos(n).distance(me);
                score(a).total_cmp(&score(b))
            })
    }

    /// Spotting an enemy: shoot first, then size the fight up and either
    /// commit to it or break away (still firing while backing off). The
    /// choice comes up again now and then during a long fight.
    fn size_up_fight(&mut self, world: &World, nav: &NavGraph, me: &Fighter) {
        let t = match (self.target, self.target_visible) {
            (Some(t), true) => t,
            _ => {
                self.engaged = None;
                return;
            }
        };
        if self.engaged != Some(t) {
            self.engaged = Some(t);
            self.engage_timer = self.rng.range(0.8, 1.8);
            return;
        }
        self.engage_timer -= DT;
        if self.engage_timer > 0.0 {
            return;
        }
        let hurt = if me.health < 60.0 { 0.25 } else { 0.0 };
        if self.rng.chance(self.p.break_away + hurt)
            && let Some(n) = self.retreat_node(nav, me.feet, world.fighters[t].feet)
        {
            self.goal = Goal::Retreat(n);
            self.goal_timer = self.rng.range(3.0, 5.0);
            self.path.clear();
            self.replan = 0.0;
            self.engage_timer = self.rng.range(5.0, 8.0);
        } else {
            self.engage_timer = self.rng.range(3.0, 6.0);
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
                && let Some(n) = self.retreat_node(nav, me.feet, world.fighters[t].feet)
            {
                return Goal::Retreat(n);
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
            // Plan to the floor under the destination: an enemy in mid-air
            // (say, riding a thruster) is reached by standing below them.
            let floor = world
                .map
                .rest_height(dest, 0.35)
                .or_else(|| world.map.ground_below(dest, 0.3, world.map.height));
            let dest = floor.map_or(dest, |y| vec2(dest.x, y));
            self.path = match (nav.nearest(me.feet), nav.nearest(dest)) {
                (Some(a), Some(b)) => nav.find_path(a, b).unwrap_or_default(),
                _ => Vec::new(),
            };
        }
        while let Some(&(n, _)) = self.path.first() {
            let p = nav.pos(n);
            if me.on_ground && (me.feet.x - p.x).abs() < 0.35 && (me.feet.y - p.y).abs() < 0.45 {
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
                // Rise only as high as the arc needs (the nav graph plans
                // hops about a tile above the higher end): a full jump
                // across a gap under a ledge would land on the ledge.
                let apex = self.jump_from_y.min(p.y) - JUMP_APEX;
                input.jump_held = me.on_ground || !self.low_jump || me.feet.y > apex;
                // Over a pit, take off with a run-up or from the very edge.
                let dir = dx.signum();
                let over_pit = deadly_below(world, me, me.feet.x + dx * 0.5);
                let ready = !over_pit
                    || me.vel.x * dir > RUN_SPEED * 0.65
                    || deadly_below(world, me, me.feet.x + dir * 0.45);
                let first_jump = me.on_ground && (higher || dx.abs() > 1.2) && ready;
                if first_jump && higher && !column_clear(world, me.feet.x, me.feet.y, p.y) {
                    // Something overhead (the far end of the floor we are
                    // climbing to): back out from under it before jumping.
                    input.move_x = -dir;
                } else if first_jump {
                    input.jump = true;
                    input.jump_held = true;
                    self.air_to = Some(p);
                    self.jump_from_y = me.feet.y;
                    // A full jump gives the most air time (long gaps need
                    // it); keep it low only if something overhead along the
                    // way would catch a full one.
                    let top = me.feet.y.min(p.y) - FULL_JUMP_RISE - STAND_HEIGHT;
                    let low = me.feet.y.min(p.y) - JUMP_APEX - STAND_HEIGHT;
                    let (x0, x1) = (me.feet.x.min(p.x) - 0.4, me.feet.x.max(p.x) + 0.4);
                    self.low_jump = world.map.terrain.box_blocked(vec2(x0, top), vec2(x1, low));
                }
                // Double jump near the apex when the target is still above.
                let double_jump =
                    !me.on_ground && me.air_jumps > 0 && me.vel.y > -3.0 && p.y < me.feet.y - 0.05;
                input.jump |= double_jump;
            }
            EdgeKind::Thrust => {
                input.jump_held = true;
                if me.on_ground {
                    // Line up on the pad, then press jump to launch.
                    let pad = world
                        .objects
                        .iter()
                        .filter(|o| o.def.kind == ObjectKind::Thruster)
                        .map(|o| o.pos)
                        .min_by(|a, b| a.distance(me.feet).total_cmp(&b.distance(me.feet)));
                    if let Some(pad) = pad {
                        // Stop on the pad first: the launch is so fast that any
                        // sideways drift carries the body into the floors above.
                        let dx = pad.x - me.feet.x;
                        input.move_x = if dx.abs() > 0.15 { dx.signum() } else { 0.0 };
                        input.jump = dx.abs() < 0.3 && me.vel.x.abs() < 1.0;
                        if input.jump {
                            self.air_to = Some(p);
                        }
                    }
                } else {
                    // Steer toward the landing, double jumping if short.
                    input.jump = me.air_jumps > 0 && me.vel.y > -3.0 && p.y < me.feet.y - 0.05;
                }
            }
            EdgeKind::Drop => {
                if me.on_ground {
                    // Drop only once fully over the hatch: standing half on
                    // the floor beside it, a drop does nothing. Otherwise
                    // step toward the side that is all hatch.
                    let hatch_at =
                        |x: f32| standing_on_hatch(&world.map, vec2(x, me.feet.y), me.size());
                    if hatch_at(me.feet.x) {
                        input.drop = true;
                        input.move_x = 0.0;
                    } else if hatch_at(me.feet.x + 0.4) {
                        input.move_x = 1.0;
                    } else if hatch_at(me.feet.x - 0.4) {
                        input.move_x = -1.0;
                    } else {
                        input.drop = true;
                    }
                }
            }
            EdgeKind::Fall if me.on_ground && dx.abs() < 0.6 => {
                // The landing is nearly straight below: step off whichever
                // side of the ledge is open.
                let open = |d: f32| {
                    world
                        .map
                        .ground_below(vec2(me.feet.x + d * 0.7, me.feet.y), 0.3, 0.6)
                        .is_none()
                };
                let toward = if dx.abs() > 0.01 { dx.signum() } else { 1.0 };
                input.move_x = if open(toward) {
                    toward
                } else if open(-toward) {
                    -toward
                } else {
                    toward
                };
            }
            EdgeKind::Walk | EdgeKind::Fall => {
                // Only what lies before the next node counts: a node at the
                // edge of a pit is somewhere to stop, not to hop from.
                let reach = dx.abs().min(0.9);
                if self.hazard_ahead(world, me, input.move_x, reach) {
                    input.jump = me.on_ground;
                    input.jump_held = true;
                }
            }
        }
    }

    /// Standing in lava (say, with a shield powerup on): climb out toward
    /// the nearest safe spot.
    fn escape_hazard(&mut self, world: &World, nav: &NavGraph, me: &Fighter, input: &mut Input) {
        if hazard_dps(&world.map, me.feet, me.size()) <= 0.0 {
            return;
        }
        // The nearest safe node one jump can reach (a platform overhead is
        // only bumped from below; the rim of the pit is the way out).
        let Some(p) = nav
            .nodes
            .iter()
            .copied()
            .filter(|n| n.y >= me.feet.y - ESCAPE_RISE)
            .min_by(|a, b| a.distance(me.feet).total_cmp(&b.distance(me.feet)))
        else {
            return;
        };
        input.move_x = (p.x - me.feet.x).signum();
        input.jump_held = true;
        input.jump = me.on_ground || (me.air_jumps > 0 && me.vel.y > -2.0);
        self.air_to = Some(p);
    }

    /// Steer while airborne: toward the committed landing spot if there is
    /// one, otherwise away from a landing in lava or a bottomless drop.
    fn air_control(&mut self, world: &World, me: &Fighter, input: &mut Input) {
        if me.on_ground {
            if !deadly_below(world, me, me.feet.x) {
                self.ground_x = me.feet.x;
            }
            return;
        }
        let steer = |x: f32| ((x - me.feet.x) * AIR_STEER / RUN_SPEED).clamp(-1.0, 1.0);
        let doomed = deadly_below(world, me, me.feet.x + me.vel.x * 0.3);
        if let Some(to) = self.air_to.filter(|to| !deadly_below(world, me, to.x)) {
            input.move_x = steer(to.x);
            // Climbing: rise through clear air first, then drift over, so the
            // head does not catch the underside of the floor being climbed to.
            // Probe where the bot will be once it could stop: its sideways
            // speed carries it a while (air braking takes ~0.2 s).
            let dir = input.move_x.signum();
            let probe = me.feet.x + me.vel.x * 0.2 + dir * 0.2;
            if me.feet.y > to.y - 0.05
                && me.vel.y < 0.0
                && !column_clear(world, probe, me.feet.y, to.y)
            {
                input.move_x = 0.0;
            }
        } else if doomed || self.air_to.is_some() {
            // No plan, or the planned landing is deadly: back to safe ground.
            input.move_x = steer(self.ground_x);
        }
        // Falling toward lava or a bottomless drop: spend the air jump.
        if doomed && me.air_jumps > 0 && me.vel.y > 0.0 && !me.in_water {
            input.jump = true;
            input.jump_held = true;
        }
    }

    /// Is there a hazard or a deadly drop within `reach` tiles in direction
    /// `dir`?
    fn hazard_ahead(&self, world: &World, me: &Fighter, dir: f32, reach: f32) -> bool {
        if dir == 0.0 || reach < 0.15 {
            return false;
        }
        // Would the body hurt that far ahead?
        let ahead = vec2(me.feet.x + dir * reach, me.feet.y);
        if hazard_dps(&world.map, ahead, me.size()) > 0.0 {
            return true;
        }
        // Or a deadly drop there: no floor at all, or a floor in a hazard
        // (lava, a bottomless void)?
        me.on_ground && deadly_below(world, me, ahead.x)
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
            if self.rng.chance(self.p.hold_chance) {
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
        if self.hazard_ahead(world, me, input.move_x, 0.9) {
            input.move_x = -input.move_x;
            self.strafe = input.move_x;
        }
        let at_brink =
            deadly_below(world, me, me.feet.x - 0.6) || deadly_below(world, me, me.feet.x + 0.6);
        if me.on_ground && !input.crouch && !at_brink && self.rng.chance(self.p.jumpy * DT) {
            // A combat hop lands about where it started.
            input.jump = true;
            self.air_to = Some(vec2(me.feet.x + input.move_x * 0.5, me.feet.y));
        }
        input.jump_held = true;
    }

    /// The point a bot tries to hit: chest or head, led by the target's
    /// motion, plus a slowly changing wobble. Lobbed and long shots also
    /// carry the bot's judgement of the range (see `learn_from_shots`).
    fn aim_point(&mut self, world: &World, me: &Fighter, t: usize, def: &WeaponDef) -> Vec2 {
        let them = &world.fighters[t];
        let eye = me.eye();
        let body = them.aabb();
        let mut point = body.center();
        point.y += (body.min.y + 0.2 - point.y) * self.p.head_bias;
        let pd = &def.projectile;
        self.lobbing = false;
        if pd.kind.is_physical() && pd.speed > 0.0 {
            let dist = eye.distance(point);
            let tt = dist / pd.speed;
            let drop = 0.5 * def.gravity() * tt * tt;
            point += them.vel * tt * self.p.lead;
            point.y -= drop;
            if drop >= RANGED_DROP || tt >= RANGED_FLIGHT_S {
                self.lobbing = true;
                // A new target or weapon: judge the range afresh, often a
                // bit off (too high or too low), sometimes spot on.
                let key = (t, me.current);
                if self.range_for != Some(key) {
                    self.range_for = Some(key);
                    self.range_fix = Vec2::ZERO;
                    self.shots.clear();
                    if self.rng.chance(self.p.misjudge_chance) {
                        let (lo, hi) = self.p.misjudge;
                        let side = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                        self.range_fix.y = side * self.rng.range(lo, hi);
                    }
                }
                point += self.range_fix * dist;
            }
        }
        self.jitter_timer -= DT;
        if self.jitter_timer <= 0.0 {
            // Freshly hit: the hands shake more.
            let hurt = me
                .last_attacker
                .is_some_and(|(_, at)| world.time - at < FLINCH_S);
            let mut shake = if hurt { self.p.flinch } else { 1.0 };
            // Lobbing is deliberate: steady hands, the error is in judging
            // the range (see `range_fix`).
            if self.lobbing {
                shake *= LOB_STEADY;
            }
            self.jitter = self.rng.range(-1.0, 1.0) * self.p.aim_error_deg.to_radians() * shake;
            self.jitter_timer = self.rng.range(0.25, 0.5);
        }
        self.settle *= (-DT / SETTLE_S).exp();
        let d = point - eye;
        let a = d.y.atan2(d.x) + self.jitter + self.settle;
        eye + vec2(a.cos(), a.sin()) * d.length().max(1.0)
    }

    /// Follow this bot's own lobbed or long shots and judge each one once,
    /// the way a person would: where it first comes down (long or short),
    /// or how high it passes the target, or where it ends if it ends first.
    /// Once it goes off (and did not go off on target), the next shot
    /// corrects by part of the miss, so shots walk onto the target.
    fn learn_from_shots(&mut self, world: &World, me: &Fighter) {
        let Some((t, weapon)) = self.range_for else {
            self.shots.clear();
            return;
        };
        let them = &world.fighters[t];
        if !them.alive {
            self.shots.clear();
            return;
        }
        let target = them.aabb().center();
        let dist = me.eye().distance(target).max(1.0);
        // +1 when the target is to the right: positive range errors are long.
        let toward = (target.x - me.feet.x).signum();
        let side = |x: f32| (x - target.x).signum();
        // Each miss as a share of the distance: positive means aim higher.
        let mut misses: Vec<f32> = Vec::new();
        for p in world
            .projectiles
            .iter()
            .filter(|p| p.owner == self.id && p.weapon == weapon)
        {
            let Some(shot) = self.shots.iter_mut().find(|s| s.id == p.id) else {
                self.shots.push(Shot {
                    id: p.id,
                    side: side(p.pos.x),
                    vy: p.vel.y,
                    miss: None,
                });
                continue;
            };
            if shot.miss.is_some() {
                continue;
            }
            if shot.vy > 0.0 && p.vel.y <= 0.0 {
                // First came down here: long or short of the target.
                let long = (p.pos.x - target.x) * toward;
                shot.miss = Some(-long * 0.5 / dist);
            } else if side(p.pos.x) != shot.side {
                // Passed the target in the air: above (aim lower) or below.
                shot.miss = Some((p.pos.y - target.y) / dist);
            }
            shot.vy = p.vel.y;
        }
        for e in &world.events {
            let Event::Impact { id, owner, pos, .. } = *e else {
                continue;
            };
            if owner != self.id {
                continue;
            }
            let Some(i) = self.shots.iter().position(|s| s.id == id) else {
                continue;
            };
            let shot = self.shots.swap_remove(i);
            // It went off on target (say, rolled in): nothing to fix.
            if pos.distance(target) < HIT_NEAR {
                continue;
            }
            if let Some(miss) = shot.miss {
                misses.push(miss);
            } else if (pos.x - target.x) * (me.feet.x - pos.x) >= 0.0 {
                // Ended short, between us: raise the aim.
                misses.push((pos.x - target.x).abs() * 0.5 / dist);
            } else if (pos.x - me.feet.x) * toward < 0.0 {
                // Came back past the shooter: it hit something overhead.
                misses.push(-BOUNCED_BACK);
            }
        }
        for miss in misses {
            // A little noise in judging it: nobody measures exactly.
            let gain = self.p.correct * self.rng.range(0.8, 1.2);
            self.range_fix.y -= miss * gain;
            self.range_fix = self.range_fix.clamp_length_max(RANGE_FIX_MAX);
        }
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
        let on_target = off.to_degrees() < if self.lobbing { 3.0 } else { 8.0 };
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

/// Is the column a body would rise through, from feet height `from_y` up to
/// standing at `to_y`, free of solid terrain at `x`?
fn column_clear(world: &World, x: f32, from_y: f32, to_y: f32) -> bool {
    let top = to_y - STAND_HEIGHT - 0.05;
    let bottom = from_y - STAND_HEIGHT;
    bottom <= top
        || !world
            .map
            .terrain
            .box_blocked(vec2(x - 0.37, top), vec2(x + 0.37, bottom))
}

/// Would falling from (`x`, the bot's height) end in a hazard or with no
/// floor at all? There is no fall damage, so a long drop alone is fine.
fn deadly_below(world: &World, me: &Fighter, x: f32) -> bool {
    match world
        .map
        .ground_below(vec2(x, me.feet.y), 0.3, world.map.height)
    {
        None => true,
        Some(y) => hazard_dps(&world.map, vec2(x, y), me.size()) > 0.0,
    }
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
            map_id: "foundry_ascent".into(),
            bots: 1,
            difficulty,
            countdown_s: 0.0,
            ..Default::default()
        };
        let mut w = World::new(data, cfg).unwrap();
        // Bot and player on the same open floor, eight tiles apart.
        w.fighters[1].feet = vec2(22.0, 32.0);
        w.fighters[0].feet = vec2(14.0, 32.0);
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
        watch(&mut b, &w, &nav, 0.6);
        assert_eq!(b.target, None, "easy bots need at least 0.7 s to react");
        watch(&mut b, &w, &nav, 0.6);
        assert_eq!(b.target, Some(0));
    }

    #[test]
    fn harder_bots_see_wider_react_faster_and_hold_less() {
        let [e, n, h] = Difficulty::ALL.map(params);
        for (lo, hi) in [(&e, &n), (&n, &h)] {
            assert!(lo.fov_half_deg < hi.fov_half_deg);
            assert!(lo.reaction.0 > hi.reaction.0 && lo.reaction.1 > hi.reaction.1);
            assert!(lo.turn_rate < hi.turn_rate);
            assert!(lo.hold_chance >= hi.hold_chance);
        }
    }
}
