//! Headless bot-only matches on the shipped maps.

use arcane_dome::ai::BotSet;
use arcane_dome::data::GameData;
use arcane_dome::sim::{Difficulty, Input, MatchConfig, Phase, TICK_RATE, World};
use std::path::Path;
use std::sync::Arc;

fn shipped() -> Arc<GameData> {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    Arc::new(GameData::load(&assets).unwrap())
}

fn run_match(data: &Arc<GameData>, map: &str, mode: &str, seconds: u32, seed: u64) -> World {
    let cfg = MatchConfig {
        map_id: map.into(),
        mode_id: mode.into(),
        friendly_bots: 3,
        enemy_bots: 4,
        difficulty: Difficulty::Hard,
        include_player: false,
        seed,
        ..Default::default()
    };
    let mut world = World::new(Arc::clone(data), cfg).unwrap();
    let mut bots = BotSet::new(&world);
    let mut inputs = vec![Input::default(); world.fighters.len()];
    for _ in 0..seconds * TICK_RATE {
        bots.think(&world, &mut inputs);
        world.step(&inputs);
        if matches!(world.phase, Phase::MatchOver { .. }) {
            break;
        }
    }
    world
}

#[test]
fn bots_fight_on_every_map_and_mode() {
    let data = shipped();
    for map in &data.maps {
        for mode in &data.modes {
            let w = run_match(&data, &map.id, &mode.id, 90, 7);
            let kills: u32 = w.fighters.iter().map(|f| f.kills).sum();
            assert!(
                kills >= 2,
                "{} / {}: only {kills} kills in 90s",
                map.id,
                mode.id
            );
            for f in &w.fighters {
                assert!(f.feet.is_finite(), "{} escaped to {:?}", f.name, f.feet);
            }
        }
    }
}

#[test]
fn every_spawn_can_reach_every_spawn_pickup_and_object() {
    let data = shipped();
    for map in &data.maps {
        let world = World::new(
            Arc::clone(&data),
            MatchConfig {
                map_id: map.id.clone(),
                include_player: false,
                enemy_bots: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let bots = BotSet::new(&world);
        let nav = bots.nav();
        let spots = map
            .spawns
            .iter()
            .map(|s| s.feet())
            .chain(map.pickups.iter().map(|p| p.feet()))
            .chain(map.objects.iter().map(|o| o.feet()));
        let nodes: Vec<usize> = spots.map(|p| nav.nearest(p).unwrap()).collect();
        for &a in &nodes[..map.spawns.len()] {
            for &b in &nodes {
                assert!(
                    nav.find_path(a, b).is_some(),
                    "{}: no path {:?} -> {:?}",
                    map.id,
                    nav.nodes[a],
                    nav.nodes[b]
                );
            }
        }
    }
}

#[test]
fn same_seed_same_match() {
    let data = shipped();
    let a = run_match(&data, "foundry_ascent", "deathmatch", 20, 42);
    let b = run_match(&data, "foundry_ascent", "deathmatch", 20, 42);
    assert_eq!(a.tick, b.tick);
    assert_eq!(a.fighters, b.fighters);
    assert_eq!(a.scores, b.scores);
}
