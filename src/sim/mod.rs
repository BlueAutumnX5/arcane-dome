//! The deterministic game simulation. Runs at a fixed 60 Hz and never touches
//! the window, input devices or rendering, so it can be stepped in tests.

mod combat;
mod fighter;
mod gadgets;
mod input;
pub mod physics;
mod rng;
mod rules;
mod variants;
mod world;

pub use combat::{MAX_PROJECTILES, Projectile};
pub use fighter::{Buff, Fighter, Slot};
pub use fighter::{
    CROUCH_SIZE, HEAD_HEIGHT, MAX_HEALTH, MAX_SHIELD, RESPAWN_DELAY, SPAWN_PROTECT, STAND_SIZE,
};
pub use gadgets::{ObjectState, absorb};
pub use input::Input;
pub use rng::Rng;
pub use rules::Phase;
pub use world::{
    Difficulty, ElimStyle, Event, GO_SHOWN_S, MAX_TEAMS, MatchConfig, PickupState, TEAM_NAMES,
    World,
};

/// Simulation ticks per second.
pub const TICK_RATE: u32 = 60;
/// Length of one simulation tick in seconds.
pub const DT: f32 = 1.0 / TICK_RATE as f32;
