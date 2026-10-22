//! The deterministic game simulation. Runs at a fixed 60 Hz and never touches
//! the window, input devices or rendering, so it can be stepped in tests.

mod combat;
mod fighter;
mod input;
pub mod physics;
mod rng;
mod rules;
mod world;

pub use combat::Projectile;
pub use fighter::{CROUCH_SIZE, HEAD_HEIGHT, MAX_HEALTH, STAND_SIZE};
pub use fighter::{Fighter, Slot};
pub use input::Input;
pub use rng::Rng;
pub use rules::Phase;
pub use world::{Difficulty, Event, MatchConfig, PickupState, World};

/// Simulation ticks per second.
pub const TICK_RATE: u32 = 60;
/// Length of one simulation tick in seconds.
pub const DT: f32 = 1.0 / TICK_RATE as f32;
