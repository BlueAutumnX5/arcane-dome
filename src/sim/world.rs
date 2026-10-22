use super::fighter::*;
use super::physics::{self, move_body, overlaps_kind, standing_on_one_way};
use super::{DT, Input, ObjectState, Phase, Projectile, Rng};
use crate::data::{
    Category, GameData, MapDef, ModeDef, ObjectKind, PickupDef, PickupKind, ProjectileKind,
    SpawnDef, TileKind,
};
use macroquad::math::{Vec2, vec2};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Self::Easy, Self::Normal, Self::Hard];

    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }
}

/// Everything chosen on the custom match screen.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchConfig {
    pub map_id: String,
    pub mode_id: String,
    /// Overrides the mode's score limit (respawn modes).
    pub score_limit: Option<u32>,
    /// Overrides the mode's round count (elimination modes).
    pub rounds: Option<u32>,
    /// Overrides the mode's time limit.
    pub time_limit_s: Option<f32>,
    /// Bots on the player's team (team modes only).
    pub friendly_bots: u32,
    /// Opposing bots (in free-for-all, simply the number of bots).
    pub enemy_bots: u32,
    /// Skill of opposing bots (and of every bot in free-for-all).
    pub difficulty: Difficulty,
    /// Skill of bots on the player's team.
    pub friendly_difficulty: Difficulty,
    /// Use the map's powerup spot.
    pub powerups: bool,
    /// Use the map's weapon and shield stations.
    pub stations: bool,
    /// Magazines never empty.
    pub infinite_ammo: bool,
    pub seed: u64,
    /// False for bot-only matches (tests, attract mode).
    pub include_player: bool,
    pub player_name: String,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            map_id: String::new(),
            mode_id: "deathmatch".into(),
            score_limit: None,
            rounds: None,
            time_limit_s: None,
            friendly_bots: 0,
            enemy_bots: 5,
            difficulty: Difficulty::Normal,
            friendly_difficulty: Difficulty::Normal,
            powerups: true,
            stations: true,
            infinite_ammo: false,
            seed: 1,
            include_player: true,
            player_name: "You".into(),
        }
    }
}

/// Things that happened during one tick, for effects, sounds and the kill feed.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Shot {
        fighter: usize,
        weapon: usize,
    },
    Tracer {
        from: Vec2,
        to: Vec2,
        kind: ProjectileKind,
    },
    Hit {
        victim: usize,
        attacker: Option<usize>,
        amount: f32,
        headshot: bool,
        pos: Vec2,
    },
    Kill {
        victim: usize,
        killer: Option<usize>,
        weapon: Option<usize>,
        headshot: bool,
    },
    Explosion {
        pos: Vec2,
        radius: f32,
    },
    Pickup {
        fighter: usize,
        kind: PickupKind,
        pos: Vec2,
    },
    Spawn {
        fighter: usize,
    },
    RoundOver {
        winner: Option<usize>,
    },
    MatchOver {
        winner: Option<usize>,
    },
    Thrust {
        fighter: usize,
        pos: Vec2,
    },
    /// A station or powerup spot just became available.
    Deployed {
        object: usize,
    },
    /// A fighter took what a station or powerup spot offered.
    Taken {
        object: usize,
        fighter: usize,
    },
    Teleport {
        fighter: usize,
        from: Vec2,
        to: Vec2,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickupState {
    pub def: PickupDef,
    pub pos: Vec2,
    /// Resolved weapon index for weapon crates.
    pub weapon: Option<usize>,
    pub active: bool,
    pub timer: f32,
}

#[derive(Debug, Clone)]
pub struct World {
    pub data: Arc<GameData>,
    pub map: MapDef,
    pub mode: ModeDef,
    pub config: MatchConfig,
    pub fighters: Vec<Fighter>,
    pub projectiles: Vec<Projectile>,
    pub pickups: Vec<PickupState>,
    /// Thrusters, stations and powerup spots.
    pub objects: Vec<ObjectState>,
    /// Standable cells (feet positions) used as teleport destinations.
    pub stand_cells: Vec<Vec2>,
    /// Events from the most recent tick only.
    pub events: Vec<Event>,
    pub rng: Rng,
    pub tick: u64,
    pub time: f32,
    pub phase: Phase,
    /// Seconds left in the match (respawn modes) or round (elimination).
    pub time_left: f32,
    /// Score per side (team or fighter, see `side_of`).
    pub scores: Vec<i32>,
    pub round: u32,
    pub round_wins: Vec<u32>,
}

const BOT_NAMES: [&str; 16] = [
    "Vex", "Orrin", "Talla", "Brisk", "Moth", "Kestrel", "Juno", "Pike", "Sable", "Wren", "Dax",
    "Ember", "Quill", "Rook", "Ivo", "Nyx",
];

impl World {
    pub fn new(data: Arc<GameData>, config: MatchConfig) -> Result<Self, String> {
        let map = data
            .map(&config.map_id)
            .ok_or_else(|| format!("unknown map \"{}\"", config.map_id))?
            .clone();
        let mut mode = data
            .mode(&config.mode_id)
            .ok_or_else(|| format!("unknown mode \"{}\"", config.mode_id))?
            .clone();
        if !map.modes.contains(&mode.id) {
            return Err(format!(
                "map \"{}\" does not support mode \"{}\"",
                map.id, mode.id
            ));
        }
        if let Some(s) = config.score_limit {
            mode.score_limit = Some(s.max(1));
        }
        if let Some(r) = config.rounds {
            mode.rounds = Some(r.max(1));
        }
        if let Some(t) = config.time_limit_s {
            mode.time_limit_s = t.max(1.0);
        }

        let team = |t: u8| if mode.is_team() { Some(t) } else { None };
        let n_weapons = data.weapons.len();
        let mut fighters = Vec::new();
        if config.include_player {
            fighters.push(Fighter::new(
                0,
                config.player_name.clone(),
                team(0),
                false,
                n_weapons,
            ));
        }
        let friendly = if mode.is_team() {
            config.friendly_bots
        } else {
            0
        };
        let teams = std::iter::repeat_n(0u8, friendly as usize)
            .chain(std::iter::repeat_n(1u8, config.enemy_bots as usize));
        for (k, t) in teams.enumerate() {
            let id = fighters.len();
            let name = BOT_NAMES[k % BOT_NAMES.len()].to_owned();
            fighters.push(Fighter::new(id, name, team(t), true, n_weapons));
        }
        if fighters.len() < 2 {
            return Err("a match needs at least two fighters".into());
        }

        let pickups = map
            .pickups
            .iter()
            .map(|p| PickupState {
                def: p.clone(),
                pos: vec2(p.x as f32 + 0.5, p.y as f32 + 0.6),
                weapon: p.weapon_id.as_deref().and_then(|id| data.weapon_index(id)),
                active: true,
                timer: 0.0,
            })
            .collect();

        let first_powerup = data.powerups.first_spawn_s;
        let objects = map
            .objects
            .iter()
            .filter(|o| match o.kind {
                ObjectKind::PowerupSpot => config.powerups,
                ObjectKind::WeaponStation | ObjectKind::ShieldStation => config.stations,
                ObjectKind::Thruster => true,
            })
            .map(|o| ObjectState::new(o, first_powerup))
            .collect();
        let stand_cells = super::gadgets::stand_cells(&map);

        let sides = if mode.is_team() { 2 } else { fighters.len() };
        let mut world = Self {
            rng: Rng::new(config.seed),
            data,
            map,
            time_left: mode.time_limit_s,
            mode,
            config,
            fighters,
            projectiles: Vec::new(),
            pickups,
            objects,
            stand_cells,
            events: Vec::new(),
            tick: 0,
            time: 0.0,
            phase: Phase::Playing,
            scores: vec![0; sides],
            round: 1,
            round_wins: vec![0; sides],
        };
        world.start_round();
        world.events.clear();
        Ok(world)
    }

    /// Advance the simulation by one fixed tick. `inputs[i]` drives fighter `i`;
    /// missing entries count as idle.
    pub fn step(&mut self, inputs: &[Input]) {
        self.events.clear();
        self.tick += 1;
        self.time += DT;
        match &mut self.phase {
            Phase::MatchOver { .. } => return,
            Phase::RoundOver { timer, .. } => {
                *timer -= DT;
                if *timer <= 0.0 {
                    self.next_round();
                    return;
                }
            }
            Phase::Playing => self.time_left = (self.time_left - DT).max(0.0),
        }
        for i in 0..self.fighters.len() {
            let input = inputs.get(i).copied().unwrap_or_default();
            self.update_fighter(i, &input);
        }
        self.update_projectiles();
        self.update_pickups();
        self.update_objects();
        self.update_respawns();
        self.check_rules();
    }

    fn update_fighter(&mut self, i: usize, input: &Input) {
        let map = &self.map;
        let f = &mut self.fighters[i];
        if !f.alive {
            return;
        }
        f.spawn_protect = (f.spawn_protect - DT).max(0.0);
        f.stun = (f.stun - DT).max(0.0);
        f.freeze = (f.freeze - DT).max(0.0);
        f.fire_cooldown = (f.fire_cooldown - DT).max(-DT);
        f.bloom = (f.bloom - BLOOM_DECAY_DEG * DT).max(0.0);
        f.drop_timer = (f.drop_timer - DT).max(0.0);
        f.beam_age += DT;

        let input = if f.stun > 0.0 {
            Input {
                aim: input.aim,
                ..Default::default()
            }
        } else {
            *input
        };

        let to_aim = input.aim - f.eye();
        if to_aim.length_squared() > 1e-6 {
            f.aim = to_aim.normalize();
            if f.aim.x.abs() > 0.05 {
                f.facing = f.aim.x.signum();
            }
        }

        // Crouch only on the ground; stay crouched while something is overhead.
        if input.crouch && f.on_ground {
            f.crouching = true;
        } else if f.crouching && !overlaps_kind(map, f.feet, STAND_SIZE, TileKind::Solid) {
            f.crouching = false;
        }
        f.in_water = overlaps_kind(map, f.feet, f.size(), TileKind::Water);

        let slow = if f.freeze > 0.0 { f.freeze_slow } else { 1.0 };
        let water = if f.in_water { WATER_SPEED } else { 1.0 };
        let base = if f.crouching { CROUCH_SPEED } else { RUN_SPEED };
        let target = input.move_x.clamp(-1.0, 1.0) * base * slow * water;
        let accel = if f.on_ground { GROUND_ACCEL } else { AIR_ACCEL };
        f.vel.x = approach(f.vel.x, target, accel * DT);

        if input.jump {
            if f.crouching && f.on_ground && standing_on_one_way(map, f.feet, f.size()) {
                f.drop_timer = DROP_THROUGH_TIME;
                f.on_ground = false;
            } else if f.in_water {
                f.vel.y = -SWIM_VEL;
            } else if f.on_ground || f.coyote > 0.0 {
                f.vel.y = -JUMP_VEL;
                f.coyote = 0.0;
                f.on_ground = false;
                f.jump_rising = true;
            } else if f.air_jumps > 0 {
                f.vel.y = -DOUBLE_JUMP_VEL;
                f.air_jumps -= 1;
                f.jump_rising = true;
            }
        }
        if f.jump_rising && !input.jump_held && f.vel.y < -JUMP_CUT_VEL {
            f.vel.y = -JUMP_CUT_VEL;
        }

        let (gravity, max_fall) = if f.in_water {
            (GRAVITY * WATER_GRAVITY, WATER_MAX_FALL)
        } else {
            (GRAVITY, MAX_FALL)
        };
        f.vel.y = (f.vel.y + gravity * DT).min(max_fall.max(f.vel.y));
        let size = f.size();
        let res = move_body(map, &mut f.feet, size, &mut f.vel, DT, f.drop_timer > 0.0);
        if res.landed {
            f.on_ground = true;
            f.air_jumps = 1;
            f.coyote = COYOTE_TIME;
        } else {
            f.on_ground = false;
            f.coyote = (f.coyote - DT).max(0.0);
        }
        if f.vel.y >= 0.0 {
            f.jump_rising = false;
        }
        f.thrust_cooldown = (f.thrust_cooldown - DT).max(0.0);
        let spiked = overlaps_kind(map, f.feet, size, TileKind::Spikes);
        let burn = f.burn;

        self.handle_weapons(i, &input);
        self.update_buffs(i);
        self.check_thrusters(i);
        if input.ability {
            self.use_ability(i);
        }

        if let Some(mut b) = burn {
            b.time_left -= DT;
            self.fighters[i].burn = (b.time_left > 0.0).then_some(b);
            self.damage(i, Some(b.source), Some(b.weapon), b.dps * DT, false, false);
        }
        if spiked {
            self.damage(i, None, None, SPIKE_DPS * DT, false, false);
        }
    }

    fn update_pickups(&mut self) {
        for k in 0..self.pickups.len() {
            let p = &mut self.pickups[k];
            if !p.active {
                p.timer -= DT;
                if p.timer <= 0.0 {
                    p.active = true;
                }
                continue;
            }
            let pos = p.pos;
            for i in 0..self.fighters.len() {
                let f = &self.fighters[i];
                if f.alive && f.aabb().expand(0.3).contains(pos) && self.try_collect(i, k) {
                    let p = &mut self.pickups[k];
                    p.active = false;
                    p.timer = p.def.respawn_s;
                    self.events.push(Event::Pickup {
                        fighter: i,
                        kind: p.def.kind,
                        pos,
                    });
                    break;
                }
            }
        }
    }

    /// Apply a pickup if it is useful to the fighter. Returns true if taken.
    fn try_collect(&mut self, i: usize, k: usize) -> bool {
        let data = Arc::clone(&self.data);
        let p = &self.pickups[k];
        let f = &mut self.fighters[i];
        match p.def.kind {
            PickupKind::Health => {
                if f.health >= MAX_HEALTH {
                    return false;
                }
                f.health = (f.health + p.def.amount.unwrap_or(50.0)).min(MAX_HEALTH);
                f.burn = None;
                true
            }
            PickupKind::Ammo => {
                let mut took = false;
                for (w, slot) in f.inventory.iter_mut().enumerate() {
                    let def = &data.weapons[w];
                    if slot.owned && def.uses_ammo() && slot.reserve < def.reserve_ammo {
                        slot.reserve =
                            (slot.reserve + def.reserve_ammo.div_ceil(2)).min(def.reserve_ammo);
                        took = true;
                    }
                }
                took
            }
            PickupKind::Weapon => match p.weapon {
                Some(w) => give_weapon(f, &data.weapons[w], w),
                None => false,
            },
        }
    }

    fn update_respawns(&mut self) {
        if !self.mode.respawn || self.phase != Phase::Playing {
            return;
        }
        for i in 0..self.fighters.len() {
            let f = &mut self.fighters[i];
            if !f.alive {
                f.respawn_timer -= DT;
                if f.respawn_timer <= 0.0 {
                    self.spawn_fighter(i);
                }
            }
        }
    }

    pub(crate) fn spawn_fighter(&mut self, i: usize) {
        let spawn = self.choose_spawn(i);
        let f = &mut self.fighters[i];
        f.feet = vec2(spawn.x as f32 + 0.5, spawn.y as f32 + 1.0);
        f.vel = Vec2::ZERO;
        f.health = MAX_HEALTH;
        f.alive = true;
        f.crouching = false;
        f.on_ground = false;
        f.in_water = false;
        f.air_jumps = 1;
        f.coyote = 0.0;
        f.drop_timer = 0.0;
        f.jump_rising = false;
        f.spawn_protect = SPAWN_PROTECT;
        f.reload_timer = 0.0;
        f.fire_cooldown = 0.3;
        f.bloom = 0.0;
        f.burn = None;
        f.stun = 0.0;
        f.freeze = 0.0;
        f.last_attacker = None;
        f.shield = 0.0;
        f.buffs.clear();
        f.teleport_charges = 0;
        f.teleport_powerup = None;
        f.thrust_cooldown = 0.0;
        self.give_loadout(i);
        self.events.push(Event::Spawn { fighter: i });
    }

    /// Pick the allowed spawn point farthest from living enemies (with a bit of randomness).
    fn choose_spawn(&mut self, i: usize) -> SpawnDef {
        let team_mode = self.mode.is_team();
        let team = self.fighters[i].team;
        let mut candidates: Vec<SpawnDef> = self
            .map
            .spawns
            .iter()
            .filter(|s| {
                if team_mode {
                    s.team == team
                } else {
                    s.team.is_none()
                }
            })
            .copied()
            .collect();
        if candidates.is_empty() {
            candidates = self.map.spawns.clone();
        }
        let enemies: Vec<Vec2> = self
            .fighters
            .iter()
            .filter(|o| o.alive && o.id != i && !self.allies(i, o.id))
            .map(|o| o.feet)
            .collect();
        let mut best = candidates[0];
        let mut best_score = f32::MIN;
        for s in candidates {
            let p = vec2(s.x as f32 + 0.5, s.y as f32 + 1.0);
            let nearest = enemies
                .iter()
                .map(|e| e.distance(p))
                .fold(50.0f32, f32::min);
            let score = nearest + self.rng.range(0.0, 4.0);
            if score > best_score {
                best_score = score;
                best = s;
            }
        }
        best
    }

    /// Starter weapons plus one random other weapon, which is equipped.
    fn give_loadout(&mut self, i: usize) {
        let data = Arc::clone(&self.data);
        let extras: Vec<usize> = (0..data.weapons.len())
            .filter(|&w| data.weapons[w].category != Category::Starter)
            .collect();
        let extra = (!extras.is_empty()).then(|| extras[self.rng.below(extras.len())]);
        let f = &mut self.fighters[i];
        let full = |w: usize| Slot {
            owned: true,
            mag: data.weapons[w].magazine,
            reserve: data.weapons[w].reserve_ammo,
        };
        f.current = 0;
        for (w, def) in data.weapons.iter().enumerate() {
            f.inventory[w] = if def.category == Category::Starter {
                full(w)
            } else {
                Slot::default()
            };
        }
        if let Some(w) = extra {
            f.inventory[w] = full(w);
            f.current = w;
        } else if let Some(w) = data
            .weapons
            .iter()
            .position(|d| d.category == Category::Starter)
        {
            f.current = w;
        }
    }

    /// Player-facing helper: the fighter's line of sight to a point.
    pub fn can_see(&self, i: usize, p: Vec2) -> bool {
        physics::line_of_sight(&self.map, self.fighters[i].eye(), p)
    }
}

/// Give a weapon with full ammo (and equip it), or top up its reserve.
/// Returns false if there was nothing to gain.
pub(crate) fn give_weapon(f: &mut Fighter, def: &crate::data::WeaponDef, w: usize) -> bool {
    let slot = &mut f.inventory[w];
    if !slot.owned {
        *slot = Slot {
            owned: true,
            mag: def.magazine,
            reserve: def.reserve_ammo,
        };
        f.current = w;
        f.reload_timer = 0.0;
        f.fire_cooldown = f.fire_cooldown.max(SWITCH_DELAY);
        true
    } else if slot.reserve < def.reserve_ammo {
        slot.reserve = def.reserve_ammo;
        true
    } else {
        false
    }
}

fn approach(cur: f32, target: f32, max_delta: f32) -> f32 {
    if cur < target {
        (cur + max_delta).min(target)
    } else {
        (cur - max_delta).max(target)
    }
}
