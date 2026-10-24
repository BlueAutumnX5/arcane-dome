use super::physics::Aabb;
use macroquad::math::{Vec2, vec2};

pub const STAND_SIZE: Vec2 = Vec2::new(0.7, 1.8);
pub const CROUCH_SIZE: Vec2 = Vec2::new(0.7, 1.2);
/// Top part of the body that counts as a headshot.
pub const HEAD_HEIGHT: f32 = 0.45;
pub const MAX_HEALTH: f32 = 100.0;
pub const MAX_SHIELD: f32 = 100.0;

// Movement tuning, in tiles and seconds.
pub(crate) const RUN_SPEED: f32 = 9.0;
pub(crate) const CROUCH_SPEED: f32 = 4.0;
pub(crate) const GROUND_ACCEL: f32 = 90.0;
pub(crate) const AIR_ACCEL: f32 = 45.0;
pub(crate) const GRAVITY: f32 = 48.0;
/// About 3.2 tiles high.
pub(crate) const JUMP_VEL: f32 = 17.5;
/// About 2.3 more tiles from the apex.
pub(crate) const DOUBLE_JUMP_VEL: f32 = 15.0;
/// Upward speed kept when jump is released early (short hops).
pub(crate) const JUMP_CUT_VEL: f32 = 7.0;
pub(crate) const MAX_FALL: f32 = 32.0;
pub(crate) const COYOTE_TIME: f32 = 0.08;
pub(crate) const DROP_THROUGH_TIME: f32 = 0.25;
pub(crate) const SWIM_VEL: f32 = 9.0;
pub(crate) const WATER_GRAVITY: f32 = 0.35;
pub(crate) const WATER_SPEED: f32 = 0.6;
pub(crate) const WATER_MAX_FALL: f32 = 5.0;
/// Seconds from death to respawning (respawn modes).
pub const RESPAWN_DELAY: f32 = 5.0;
pub(crate) const SPAWN_PROTECT: f32 = 1.5;
pub(crate) const SWITCH_DELAY: f32 = 0.2;
pub(crate) const MAX_BLOOM_DEG: f32 = 12.0;
pub(crate) const BLOOM_DECAY_DEG: f32 = 20.0;

/// Ammo state of one weapon in a fighter's inventory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Slot {
    pub owned: bool,
    pub mag: u32,
    pub reserve: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fighter {
    pub id: usize,
    pub name: String,
    /// `None` in free-for-all modes.
    pub team: Option<u8>,
    /// Team colour, as an index into the team palette (set from the match
    /// setup; `None` in free-for-all).
    pub tint: Option<u8>,
    pub is_bot: bool,

    /// Bottom-center of the body, in tiles.
    pub feet: Vec2,
    pub vel: Vec2,
    /// Normalized aim direction.
    pub aim: Vec2,
    pub facing: f32,
    pub on_ground: bool,
    pub in_water: bool,
    pub crouching: bool,
    pub air_jumps: u8,
    pub coyote: f32,
    pub drop_timer: f32,
    /// Rising from a jump (releasing jump early cuts it short).
    pub jump_rising: bool,

    pub health: f32,
    pub alive: bool,
    pub respawn_timer: f32,
    /// World time of the last death (for the death screen).
    pub died_at: f32,
    pub spawn_protect: f32,

    /// One entry per weapon in `GameData::weapons` (same index = same weapon).
    pub inventory: Vec<Slot>,
    pub current: usize,
    pub fire_cooldown: f32,
    pub reload_timer: f32,
    /// Extra spread from sustained fire, in degrees.
    pub bloom: f32,
    /// Seconds since this fighter's beam last fired (for drawing).
    pub beam_age: f32,

    pub burn: Option<Burn>,
    pub stun: f32,
    pub freeze: f32,
    pub freeze_slow: f32,
    pub last_attacker: Option<(usize, f32)>,

    /// Damage-reducing shield, 0..=MAX_SHIELD.
    pub shield: f32,
    /// Timed powerups currently active.
    pub buffs: Vec<Buff>,
    /// Shadowstep-style teleports waiting to be used, and which powerup gave them.
    pub teleport_charges: u32,
    pub teleport_powerup: Option<usize>,

    pub kills: u32,
    pub deaths: u32,
}

/// An active timed powerup (index into `GameData::powerups.powerups`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buff {
    pub powerup: usize,
    pub time_left: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burn {
    pub dps: f32,
    pub time_left: f32,
    pub source: usize,
    pub weapon: usize,
}

impl Fighter {
    pub fn new(id: usize, name: String, team: Option<u8>, is_bot: bool, weapons: usize) -> Self {
        Self {
            id,
            name,
            team,
            tint: team,
            is_bot,
            feet: Vec2::ZERO,
            vel: Vec2::ZERO,
            aim: vec2(1.0, 0.0),
            facing: 1.0,
            on_ground: false,
            in_water: false,
            crouching: false,
            air_jumps: 1,
            coyote: 0.0,
            drop_timer: 0.0,
            jump_rising: false,
            health: MAX_HEALTH,
            alive: false,
            respawn_timer: 0.0,
            died_at: 0.0,
            spawn_protect: 0.0,
            inventory: vec![Slot::default(); weapons],
            current: 0,
            fire_cooldown: 0.0,
            reload_timer: 0.0,
            bloom: 0.0,
            beam_age: f32::INFINITY,
            burn: None,
            stun: 0.0,
            freeze: 0.0,
            freeze_slow: 1.0,
            last_attacker: None,
            shield: 0.0,
            buffs: Vec::new(),
            teleport_charges: 0,
            teleport_powerup: None,
            kills: 0,
            deaths: 0,
        }
    }

    pub fn size(&self) -> Vec2 {
        if self.crouching {
            CROUCH_SIZE
        } else {
            STAND_SIZE
        }
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::from_feet(self.feet, self.size())
    }

    pub fn center(&self) -> Vec2 {
        self.aabb().center()
    }

    /// Where shots come from (shoulder height).
    pub fn eye(&self) -> Vec2 {
        vec2(self.feet.x, self.feet.y - self.size().y * 0.78)
    }

    pub fn is_headshot_point(&self, p: Vec2) -> bool {
        p.y <= self.aabb().min.y + HEAD_HEIGHT
    }

    pub fn owns(&self, weapon: usize) -> bool {
        self.inventory.get(weapon).is_some_and(|s| s.owned)
    }

    pub fn is_reloading(&self) -> bool {
        self.reload_timer > 0.0
    }
}
