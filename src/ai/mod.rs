//! Bot brains. They read the world and produce the same `Input` a player would,
//! so the sim never needs to know who is a bot.

mod bot;
pub mod nav;

use crate::sim::{Input, World};
use bot::Brain;
pub use nav::{EdgeKind, NavGraph};

pub struct BotSet {
    nav: NavGraph,
    brains: Vec<Option<Brain>>,
}

impl BotSet {
    pub fn new(world: &World) -> Self {
        let nav = NavGraph::build(&world.map);
        let brains = world
            .fighters
            .iter()
            .map(|f| {
                let skill = match f.team {
                    Some(t) if world.mode.is_team() => world.config.team_difficulty[t as usize],
                    _ => world.config.difficulty,
                };
                f.is_bot.then(|| Brain::new(f.id, world.config.seed, skill))
            })
            .collect();
        Self { nav, brains }
    }

    /// Fill in inputs for every bot. Non-bot entries are left untouched.
    pub fn think(&mut self, world: &World, inputs: &mut [Input]) {
        for (i, brain) in self.brains.iter_mut().enumerate() {
            if let (Some(b), Some(slot)) = (brain, inputs.get_mut(i)) {
                *slot = b.think(world, &self.nav);
            }
        }
    }

    /// Where bot `i` is trying to land, while in the air (debugging tools).
    pub fn landing_target(&self, i: usize) -> Option<macroquad::math::Vec2> {
        self.brains.get(i)?.as_ref()?.landing_target()
    }

    /// One line about what bot `i` is up to, for debugging tools.
    pub fn describe(&self, world: &World, i: usize) -> String {
        match self.brains.get(i) {
            Some(Some(b)) => b.describe(world, &self.nav),
            _ => String::from("not a bot"),
        }
    }

    pub fn nav(&self) -> &NavGraph {
        &self.nav
    }
}
