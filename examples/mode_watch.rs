//! Bot-only Juggernaut and Gun Game matches, for tuning those modes:
//! `cargo run --release --example mode_watch -- <map_id> [matches]`.
//! Juggernaut: how long a reign lasts and how many kills it brings.
//! Gun Game: how long a match takes and how often fighters get knocked back.

use arcane_dome::ai::BotSet;
use arcane_dome::data::{GameData, find_assets_dir};
use arcane_dome::sim::{DT, Difficulty, Event, Input, MatchConfig, Phase, World};
use std::sync::Arc;

fn main() {
    let assets = find_assets_dir().expect("assets folder");
    let data = Arc::new(GameData::load(&assets).expect("game data"));
    let mut args = std::env::args().skip(1);
    let map = args.next().unwrap_or_else(|| "foundry_ascent".into());
    let matches: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);

    for (mode, teams) in [("juggernaut", 0), ("gun_game", 0), ("gun_game", 2)] {
        for difficulty in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
            let (mut reigns, mut reign_s, mut reign_kills) = (0u32, 0.0f32, 0u32);
            let (mut length_s, mut climbs, mut knocks, mut finished) = (0.0f32, 0u32, 0u32, 0u32);
            for seed in 0..matches {
                let cfg = MatchConfig {
                    map_id: map.clone(),
                    mode_id: mode.into(),
                    teams,
                    bots: 7,
                    team_bots: [4, 4, 0, 0],
                    difficulty,
                    team_difficulty: [difficulty; 4],
                    include_player: false,
                    seed: seed * 17 + 3,
                    ..Default::default()
                };
                let mut w = World::new(Arc::clone(&data), cfg).unwrap();
                let mut bots = BotSet::new(&w);
                let mut inputs = vec![Input::default(); w.fighters.len()];
                let mut since = 0.0;
                while !matches!(w.phase, Phase::MatchOver { .. }) {
                    bots.think(&w, &mut inputs);
                    w.step(&inputs);
                    for e in &w.events {
                        match *e {
                            Event::Juggernaut { .. } => {
                                if w.time > DT * 2.0 {
                                    reigns += 1;
                                    reign_s += w.time - since;
                                }
                                since = w.time;
                            }
                            Event::Kill {
                                killer: Some(k), ..
                            } if w.juggernaut == Some(k) => reign_kills += 1,
                            Event::Ladder { up: true, .. } => climbs += 1,
                            Event::Ladder { up: false, .. } => knocks += 1,
                            _ => {}
                        }
                    }
                }
                length_s += w.time;
                if w.time < w.mode.time_limit_s - 1.0 {
                    finished += 1;
                }
            }
            let n = matches as f32;
            let label = format!("{mode} {teams} teams {difficulty:?}");
            if mode == "juggernaut" {
                println!(
                    "{label}: {:.0}s matches, {:.1} reigns each, {:.0}s and {:.1} kills a reign",
                    length_s / n,
                    reigns as f32 / n,
                    reign_s / reigns.max(1) as f32,
                    reign_kills as f32 / reigns.max(1) as f32,
                );
            } else {
                println!(
                    "{label}: {:.0}s matches, {finished}/{matches} finished the ladder, {:.0} climbs and {:.1} knock-backs each",
                    length_s / n,
                    climbs as f32 / n,
                    knocks as f32 / n,
                );
            }
        }
    }
}
