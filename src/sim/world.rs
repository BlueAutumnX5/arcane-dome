use super::fighter::*;
use super::gadgets::thruster_under;
use super::objectives::{FlagEvent, FlagState, ZoneState};
use super::physics::{self, hazard_dps, in_water, move_body, overlaps_solid, standing_on_hatch};
use super::{DT, Input, ObjectState, Phase, Projectile, Rng};
use crate::data::{
    Category, GameData, MapDef, ModeDef, ObjectKind, PickupDef, PickupKind, ProjectileKind, Rules,
    SpawnDef,
};
use crate::i18n::tn;
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
            Self::Easy => tn("Easy"),
            Self::Normal => tn("Normal"),
            Self::Hard => tn("Hard"),
        }
    }
}

/// How an Elimination match is played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElimStyle {
    /// One life per round; most rounds won wins.
    #[default]
    Rounds,
    /// Everyone has their own lives; out of lives, out of the match.
    LastManStanding,
    /// Each team shares a pool of lives (teams only).
    Attrition,
}

/// How long "GO" stays up after the countdown.
pub const GO_SHOWN_S: f32 = 1.0;

/// Most teams a team match can have.
pub const MAX_TEAMS: usize = 4;
/// Team names, by team number.
/// Team colour names, by palette index (the colour a team picked).
pub const TEAM_NAMES: [&str; MAX_TEAMS] = [tn("Blue"), tn("Red"), tn("Green"), tn("Yellow")];

/// Everything chosen on the custom match screen.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchConfig {
    pub map_id: String,
    pub mode_id: String,
    /// Overrides the mode's score limit (respawn modes).
    pub score_limit: Option<u32>,
    /// Overrides the mode's round count (elimination modes).
    pub rounds: Option<u32>,
    /// Overrides the mode's time limit; `Some(0.0)` turns it off.
    pub time_limit_s: Option<f32>,
    /// Elimination: how it is played.
    pub elim_style: ElimStyle,
    /// Elimination with lives: lives per fighter (Last Man Standing) or per
    /// team (Attrition). `None` = the mode's default; `Some(0)` = unlimited.
    pub lives: Option<u32>,
    /// Free-for-all: how many bots.
    pub bots: u32,
    /// Free-for-all: how good the bots are.
    pub difficulty: Difficulty,
    /// How many teams: 0 (or 1) plays free-for-all, 2 to [`MAX_TEAMS`]
    /// plays in teams, if the mode allows it.
    pub teams: u8,
    /// Team modes: bots on each team. Team 0 is the player's; the player is
    /// not counted here.
    pub team_bots: [u32; MAX_TEAMS],
    /// Team modes: how good each team's bots are.
    pub team_difficulty: [Difficulty; MAX_TEAMS],
    /// Team modes: each team's colour, as an index into the palette
    /// ([`TEAM_NAMES`]). Must be all different; otherwise the defaults are used.
    pub team_colors: [u8; MAX_TEAMS],
    /// Use the map's powerup spot.
    pub powerups: bool,
    /// Use the map's weapon and shield stations.
    pub stations: bool,
    /// Magazines never empty.
    pub infinite_ammo: bool,
    /// Juggernaut: what scores. At least one stays on (both off counts as
    /// both on). The bonus makes a kill as the Juggernaut worth 2.
    pub jugg_kills_score: bool,
    pub killing_jugg_scores: bool,
    pub jugg_bonus: bool,
    /// Gun Game: a melee kill sends the victim back one weapon, and so does
    /// dying with nobody to credit (your own fire, the map).
    pub melee_demotes: bool,
    pub suicide_demotes: bool,
    /// King of the Hill: seconds before the hill moves (`Some(0.0)` = never;
    /// `None` = the mode's).
    pub hill_move_s: Option<f32>,
    pub seed: u64,
    /// "3, 2, 1, GO" before the match and each elimination round, in
    /// seconds. Everyone can aim but not move or shoot until it ends.
    pub countdown_s: f32,
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
            elim_style: ElimStyle::Rounds,
            lives: None,
            bots: 5,
            difficulty: Difficulty::Normal,
            teams: 0,
            team_bots: [2, 3, 2, 2],
            team_difficulty: [Difficulty::Normal; MAX_TEAMS],
            team_colors: [0, 1, 2, 3],
            powerups: true,
            stations: true,
            infinite_ammo: false,
            jugg_kills_score: true,
            killing_jugg_scores: true,
            jugg_bonus: false,
            melee_demotes: true,
            suicide_demotes: true,
            hill_move_s: None,
            seed: 1,
            countdown_s: 3.0,
            include_player: true,
            player_name: tn("You").into(),
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
    /// A bullet hit a wall or floor here and left a mark.
    Mark {
        pos: Vec2,
    },
    /// A projectile's flight ended here (hit, wall, fuse or off the map),
    /// so its shooter can see where the shot went.
    Impact {
        id: u32,
        owner: usize,
        weapon: usize,
        pos: Vec2,
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
    /// Juggernaut: this fighter just became it.
    Juggernaut {
        fighter: usize,
    },
    /// Gun Game: a side moved up (or was knocked down) to this rung.
    Ladder {
        side: usize,
        rung: usize,
        up: bool,
    },
    /// Domination and Territories: a side captured a zone.
    ZoneTaken {
        zone: usize,
        side: usize,
    },
    /// King of the Hill: the hill moved to this zone.
    HillMoved {
        zone: usize,
    },
    /// Territories: a side held every zone and scored; the line resets.
    Pushed {
        side: usize,
    },
    /// Capture the Flag: something happened to a team's flag (index into
    /// `World::flags`), with the fighter who did it.
    Flag {
        flag: usize,
        fighter: Option<usize>,
        what: FlagEvent,
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
    /// Id for the next projectile fired.
    pub next_shot: u32,
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
    /// Elimination style in play (Attrition falls back to Last Man Standing
    /// without teams; deathmatch ignores it).
    pub style: ElimStyle,
    /// Lives each fighter or team starts with (`None` = unlimited).
    pub lives_limit: Option<u32>,
    /// Last Man Standing: lives left per fighter, this one included.
    pub lives_left: Vec<u32>,
    /// Attrition: lives left in each team's shared pool.
    pub team_lives: Vec<u32>,
    /// Seconds left before the fight starts. It keeps counting below zero
    /// for a moment (down to `-GO_SHOWN_S`) so the HUD can show "GO".
    pub countdown: f32,
    /// Juggernaut: who it is.
    pub juggernaut: Option<usize>,
    /// Gun Game: the weapons in order. A side's rung is its score.
    pub ladder: Vec<usize>,
    /// Zone modes: the map's zones, left to right.
    pub zones: Vec<ZoneState>,
    /// King of the Hill: the zone that is the hill, and seconds before it
    /// moves.
    pub hill: Option<usize>,
    pub hill_timer: f32,
    /// Capture the Flag: one flag per team.
    pub flags: Vec<FlagState>,
    /// Fractions of a point earned over time, per side.
    pub score_acc: Vec<f32>,
}

/// Enough for the biggest match (4 teams of 7); beyond that names get a number.
const BOT_NAMES: [&str; 32] = [
    "Vex", "Orrin", "Talla", "Brisk", "Moth", "Kestrel", "Juno", "Pike", "Sable", "Wren", "Dax",
    "Ember", "Quill", "Rook", "Ivo", "Nyx", "Lark", "Fenn", "Bram", "Hollis", "Mira", "Zeke",
    "Tamsin", "Odo", "Rhea", "Sorrel", "Vale", "Flint", "Gale", "Indra", "Bex", "Corin",
];

impl World {
    pub fn new(data: Arc<GameData>, mut config: MatchConfig) -> Result<Self, String> {
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
        // Teams are a setting, not a mode: stamp the actual count into this
        // match's copy of the mode (1 = free-for-all), so `mode.is_team()`
        // answers for this match.
        let wanted = if mode.team_only {
            config.teams.max(2)
        } else {
            config.teams
        };
        let num_teams = if mode.teams >= 2 && wanted >= 2 {
            (wanted as usize).clamp(2, (mode.teams as usize).min(MAX_TEAMS))
        } else {
            0
        };
        mode.teams = num_teams.max(1) as u8;
        if num_teams > 0 && mode.team_score_limit.is_some() {
            mode.score_limit = mode.team_score_limit;
        }
        // Limits: 0 turns one off. All of them off makes an endless match
        // (left from the pause menu).
        if let Some(s) = config.score_limit {
            mode.score_limit = (s > 0).then_some(s);
        }
        if let Some(r) = config.rounds {
            mode.rounds = Some(r.max(1));
        }
        if let Some(t) = config.time_limit_s {
            mode.time_limit_s = if t > 0.0 { t.max(1.0) } else { f32::INFINITY };
        }
        let gun_game = mode.rules == Rules::GunGame;
        let ladder: Vec<usize> = mode
            .ladder
            .iter()
            .filter_map(|id| data.weapon_index(id))
            .collect();
        if gun_game {
            if ladder.is_empty() {
                return Err(format!("mode \"{}\" has no weapons in its ladder", mode.id));
            }
            // Win by finishing the ladder.
            mode.score_limit = Some(ladder.len() as u32);
        }
        let style = match config.elim_style {
            _ if mode.respawn => ElimStyle::Rounds,
            ElimStyle::Attrition if num_teams == 0 => ElimStyle::LastManStanding,
            s => s,
        };
        let default_lives = match style {
            ElimStyle::Attrition => mode.team_lives.unwrap_or(15),
            _ => mode.lives.unwrap_or(3),
        };
        let lives_limit = match config.lives {
            Some(0) => None,
            Some(n) => Some(n),
            None => Some(default_lives),
        };
        if style != ElimStyle::Rounds {
            mode.rounds = Some(1);
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
        let bot_teams: Vec<u8> = if mode.is_team() {
            (0..num_teams)
                .flat_map(|t| std::iter::repeat_n(t as u8, config.team_bots[t] as usize))
                .collect()
        } else {
            vec![0; config.bots as usize]
        };
        for (k, t) in bot_teams.into_iter().enumerate() {
            let id = fighters.len();
            let lap = k / BOT_NAMES.len();
            let base = BOT_NAMES[k % BOT_NAMES.len()];
            let name = if lap == 0 {
                base.to_owned()
            } else {
                format!("{base} {}", lap + 1)
            };
            fighters.push(Fighter::new(id, name, team(t), true, n_weapons));
        }
        if fighters.len() < 2 {
            return Err("a match needs at least two fighters".into());
        }
        // Team colours: a palette index each, all different.
        let mut colors = config.team_colors;
        let mut seen = [false; MAX_TEAMS];
        let distinct = colors
            .iter()
            .all(|&c| (c as usize) < MAX_TEAMS && !std::mem::replace(&mut seen[c as usize], true));
        if !distinct {
            colors = [0, 1, 2, 3];
        }
        config.team_colors = colors;
        for f in &mut fighters {
            f.tint = f.team.map(|t| colors[t as usize]);
        }
        if mode.is_team() {
            let manned = (0..num_teams as u8)
                .filter(|&t| fighters.iter().any(|f| f.team == Some(t)))
                .count();
            if manned < 2 {
                return Err("a team match needs at least two teams with fighters".into());
            }
        }

        // Gun Game hands out its own weapons, with ammo that never runs out.
        let pickups = map
            .pickups
            .iter()
            .filter(|p| !gun_game || p.kind == PickupKind::Health)
            .map(|p| PickupState {
                def: p.clone(),
                pos: p.feet() - vec2(0.0, 0.4),
                weapon: p.weapon_id.as_deref().and_then(|id| data.weapon_index(id)),
                active: true,
                timer: 0.0,
            })
            .collect();

        let objects = map
            .objects
            .iter()
            .filter(|o| match o.kind {
                ObjectKind::PowerupSpot => config.powerups,
                ObjectKind::WeaponStation => config.stations && !gun_game,
                ObjectKind::ShieldStation => config.stations,
                ObjectKind::Thruster => true,
            })
            .map(|o| ObjectState::new(o, &data))
            .collect();
        let stand_cells = super::gadgets::stand_cells(&map);

        let n_fighters = fighters.len();
        let sides = if mode.is_team() {
            num_teams
        } else {
            fighters.len()
        };
        let mut world = Self {
            rng: Rng::new(config.seed),
            data,
            map,
            time_left: mode.time_limit_s,
            mode,
            config,
            fighters,
            projectiles: Vec::new(),
            next_shot: 0,
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
            style,
            lives_limit,
            lives_left: vec![lives_limit.unwrap_or(0); n_fighters],
            team_lives: vec![lives_limit.unwrap_or(0); sides],
            countdown: 0.0,
            juggernaut: None,
            ladder,
            zones: Vec::new(),
            hill: None,
            hill_timer: 0.0,
            flags: Vec::new(),
            score_acc: Vec::new(),
        };
        world.setup_objectives();
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
            Phase::Playing if self.countdown > 0.0 => {
                // Before the start: fighters settle and aim, nothing else runs.
                self.countdown -= DT;
                for i in 0..self.fighters.len() {
                    let input = inputs.get(i).copied().unwrap_or_default();
                    self.update_fighter(i, &input);
                }
                return;
            }
            Phase::Playing => {
                self.countdown = (self.countdown - DT).max(-GO_SHOWN_S);
                self.time_left = (self.time_left - DT).max(0.0);
            }
        }
        for i in 0..self.fighters.len() {
            let input = inputs.get(i).copied().unwrap_or_default();
            self.update_fighter(i, &input);
        }
        self.update_projectiles();
        self.update_pickups();
        self.update_objects();
        self.update_respawns();
        self.update_objectives();
        self.check_rules();
    }

    fn update_fighter(&mut self, i: usize, input: &Input) {
        let waiting = self.countdown > 0.0;
        let map = &self.map;
        let f = &mut self.fighters[i];
        if !f.alive {
            return;
        }
        if !waiting {
            f.spawn_protect = (f.spawn_protect - DT).max(0.0);
        }
        f.stun = (f.stun - DT).max(0.0);
        f.freeze = (f.freeze - DT).max(0.0);
        f.fire_cooldown = (f.fire_cooldown - DT).max(-DT);
        f.bloom = (f.bloom - BLOOM_DECAY_DEG * DT).max(0.0);
        f.drop_timer = (f.drop_timer - DT).max(0.0);
        f.beam_age += DT;

        let input = if f.stun > 0.0 || waiting {
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
        } else if f.crouching && !overlaps_solid(map, f.feet, STAND_SIZE) {
            f.crouching = false;
        }
        f.in_water = in_water(map, f.feet, f.size());

        let slow = if f.freeze > 0.0 { f.freeze_slow } else { 1.0 };
        let water = if f.in_water { WATER_SPEED } else { 1.0 };
        let base = if f.crouching { CROUCH_SPEED } else { RUN_SPEED };
        let target = input.move_x.clamp(-1.0, 1.0) * base * slow * water;
        let accel = if f.on_ground { GROUND_ACCEL } else { AIR_ACCEL };
        f.vel.x = approach(f.vel.x, target, accel * DT);

        let on_hatch = f.on_ground && standing_on_hatch(map, f.feet, f.size());
        if input.drop && on_hatch {
            // Pressing down on a hatch drops through it.
            f.drop_timer = DROP_THROUGH_TIME;
            f.on_ground = false;
            f.crouching = false;
        } else if input.jump {
            if f.crouching && on_hatch {
                f.drop_timer = DROP_THROUGH_TIME;
                f.on_ground = false;
            } else if f.in_water {
                f.vel.y = -SWIM_VEL;
            } else if let Some((pad, speed)) = (f.on_ground || f.coyote > 0.0)
                .then(|| thruster_under(&self.objects, f.aabb()))
                .flatten()
            {
                f.vel.y = -speed;
                f.on_ground = false;
                f.crouching = false;
                f.coyote = 0.0;
                f.air_jumps = 1;
                f.jump_rising = false;
                self.events.push(Event::Thrust {
                    fighter: i,
                    pos: pad,
                });
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
        let res = move_body(
            map,
            &mut f.feet,
            size,
            &mut f.vel,
            DT,
            f.drop_timer > 0.0,
            f.on_ground,
        );
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
        let hazard = hazard_dps(map, f.feet, size);
        let burn = f.burn;

        self.handle_weapons(i, &input);
        self.update_buffs(i);
        if input.ability {
            self.use_ability(i);
        }

        if let Some(mut b) = burn {
            b.time_left -= DT;
            self.fighters[i].burn = (b.time_left > 0.0).then_some(b);
            self.damage(i, Some(b.source), Some(b.weapon), b.dps * DT, false, false);
        }
        if hazard > 0.0 {
            self.damage(i, None, None, hazard * DT, false, false);
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
                if f.health >= f.max_health {
                    return false;
                }
                f.health = (f.health + p.def.amount.unwrap_or(50.0)).min(f.max_health);
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
        if !self.respawns() || self.phase != Phase::Playing {
            return;
        }
        for i in 0..self.fighters.len() {
            let f = &mut self.fighters[i];
            if !f.alive && !f.out {
                f.respawn_timer -= DT;
                if f.respawn_timer <= 0.0 {
                    self.spawn_fighter(i);
                }
            }
        }
    }

    pub(crate) fn spawn_fighter(&mut self, i: usize) {
        let spawn = self.choose_spawn(i);
        // Stand on the floor, not half sunk into it on a slope.
        let feet = spawn.feet();
        let rest = self.map.rest_height(feet, STAND_SIZE.x * 0.5);
        let jugg = self.juggernaut == Some(i);
        let f = &mut self.fighters[i];
        f.feet = vec2(feet.x, rest.unwrap_or(feet.y));
        f.vel = Vec2::ZERO;
        f.health = f.max_health;
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
        // A Juggernaut comes with a full shield; anyone else starts without.
        f.shield = if jugg { f.max_shield } else { 0.0 };
        f.buffs.clear();
        f.teleport_charges = 0;
        f.teleport_powerup = None;
        self.give_loadout(i);
        self.events.push(Event::Spawn { fighter: i });
    }

    /// Pick the allowed spawn point farthest from living enemies (with a bit of
    /// randomness). A team the map has no spawns for uses the free-for-all
    /// spawns, and then also tries to stay close to its living teammates.
    fn choose_spawn(&mut self, i: usize) -> SpawnDef {
        let team_mode = self.mode.is_team();
        let team = self.fighters[i].team;
        // Spawns can change hands with the number of teams (see `by_teams`).
        let n = if team_mode { self.num_sides() } else { 0 };
        let own = |s: &&SpawnDef| {
            if team_mode {
                s.team_for(n) == team
            } else {
                s.team.is_none()
            }
        };
        let mut candidates: Vec<SpawnDef> = self.map.spawns.iter().filter(own).copied().collect();
        let borrowed = candidates.is_empty();
        if borrowed {
            candidates = self
                .map
                .spawns
                .iter()
                .filter(|s| s.team.is_none() && s.team_for(n).is_none())
                .copied()
                .collect();
        }
        if candidates.is_empty() {
            candidates = self.map.spawns.clone();
        }
        let enemies: Vec<Vec2> = self
            .fighters
            .iter()
            .filter(|o| o.alive && o.id != i && !self.allies(i, o.id))
            .map(|o| o.feet)
            .collect();
        let friends: Vec<Vec2> = self
            .fighters
            .iter()
            .filter(|o| team_mode && borrowed && o.alive && o.id != i && self.allies(i, o.id))
            .map(|o| o.feet)
            .collect();
        let mut best = candidates[0];
        let mut best_score = f32::MIN;
        for s in candidates {
            let p = s.feet();
            let nearest = enemies
                .iter()
                .map(|e| e.distance(p))
                .fold(50.0f32, f32::min);
            let buddy = friends
                .iter()
                .map(|f| f.distance(p))
                .fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d))));
            let score = nearest - buddy.map_or(0.0, |d| d * 0.6) + self.rng.range(0.0, 4.0);
            if score > best_score {
                best_score = score;
                best = s;
            }
        }
        best
    }

    /// Starter weapons plus one random other weapon, which is equipped. In
    /// Gun Game, just the weapon of the fighter's rung.
    fn give_loadout(&mut self, i: usize) {
        if self.mode.rules == Rules::GunGame {
            let f = &mut self.fighters[i];
            f.inventory.fill(Slot::default());
            self.arm_for_rung(i);
            return;
        }
        let data = Arc::clone(&self.data);
        let extras: Vec<usize> = (0..data.weapons.len())
            .filter(|&w| data.weapons[w].is_loot())
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
