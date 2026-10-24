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

/// `teams` 0 plays free-for-all.
fn run_match(
    data: &Arc<GameData>,
    map: &str,
    mode: &str,
    teams: u8,
    seconds: u32,
    seed: u64,
) -> World {
    let cfg = MatchConfig {
        map_id: map.into(),
        mode_id: mode.into(),
        bots: 7,
        teams,
        team_bots: [3, 4, 0, 0],
        difficulty: Difficulty::Hard,
        team_difficulty: [Difficulty::Hard; 4],
        include_player: false,
        seed,
        countdown_s: 0.0,
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
            for teams in [0, 2] {
                let w = run_match(&data, &map.id, &mode.id, teams, 90, 7);
                let kills: u32 = w.fighters.iter().map(|f| f.kills).sum();
                assert!(
                    kills >= 2,
                    "{} / {} ({teams} teams): only {kills} kills in 90s",
                    map.id,
                    mode.id
                );
                for f in &w.fighters {
                    assert!(f.feet.is_finite(), "{} escaped to {:?}", f.name, f.feet);
                }
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
                bots: 2,
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
    let a = run_match(&data, "foundry_ascent", "deathmatch", 0, 20, 42);
    let b = run_match(&data, "foundry_ascent", "deathmatch", 0, 20, 42);
    assert_eq!(a.tick, b.tick);
    assert_eq!(a.fighters, b.fighters);
    assert_eq!(a.scores, b.scores);
}

#[test]
fn four_team_bot_matches_involve_every_team() {
    let data = shipped();
    for map in &data.maps {
        for mode in data.modes.iter().filter(|m| m.is_team() && m.teams >= 4) {
            let cfg = MatchConfig {
                map_id: map.id.clone(),
                mode_id: mode.id.clone(),
                teams: 4,
                team_bots: [2, 2, 2, 2],
                team_difficulty: [Difficulty::Hard; 4],
                include_player: false,
                seed: 11,
                countdown_s: 0.0,
                ..Default::default()
            };
            let mut world = World::new(Arc::clone(&data), cfg).unwrap();
            let mut bots = BotSet::new(&world);
            let mut inputs = vec![Input::default(); world.fighters.len()];
            let mut rounds_done = 0;
            for _ in 0..120 * TICK_RATE {
                bots.think(&world, &mut inputs);
                world.step(&inputs);
                rounds_done += world
                    .events
                    .iter()
                    .filter(|e| matches!(e, arcane_dome::sim::Event::RoundOver { .. }))
                    .count();
                if matches!(world.phase, Phase::MatchOver { .. }) {
                    break;
                }
            }
            let label = format!("{} / {}", map.id, mode.id);
            for t in 0..4u8 {
                let team = || world.fighters.iter().filter(move |f| f.team == Some(t));
                let kills: u32 = team().map(|f| f.kills).sum();
                let deaths: u32 = team().map(|f| f.deaths).sum();
                assert!(
                    kills + deaths > 0,
                    "{label}: team {t} never fought (no kills, no deaths)"
                );
            }
            if !mode.respawn {
                assert!(rounds_done >= 1, "{label}: no elimination round finished");
            }
            for f in &world.fighters {
                assert!(f.feet.is_finite(), "{label}: {} escaped", f.name);
            }
        }
    }
}

#[test]
fn lives_styles_play_out_on_every_map() {
    use arcane_dome::sim::ElimStyle;
    let data = shipped();
    for map in &data.maps {
        for (style, teams) in [(ElimStyle::LastManStanding, 0), (ElimStyle::Attrition, 2)] {
            let cfg = MatchConfig {
                map_id: map.id.clone(),
                mode_id: "elimination".into(),
                elim_style: style,
                lives: Some(2),
                teams,
                bots: 5,
                team_bots: [3, 3, 0, 0],
                difficulty: Difficulty::Hard,
                team_difficulty: [Difficulty::Hard; 4],
                include_player: false,
                countdown_s: 0.0,
                seed: 5,
                ..Default::default()
            };
            let mut world = World::new(Arc::clone(&data), cfg).unwrap();
            let mut bots = BotSet::new(&world);
            let mut inputs = vec![Input::default(); world.fighters.len()];
            for _ in 0..300 * TICK_RATE {
                bots.think(&world, &mut inputs);
                world.step(&inputs);
                if matches!(world.phase, Phase::MatchOver { .. }) {
                    break;
                }
            }
            let label = format!("{} / {style:?}", map.id);
            let out = world.fighters.iter().filter(|f| f.out).count();
            assert!(out > 0, "{label}: nobody ran out of lives in 5 minutes");
            for f in &world.fighters {
                assert!(f.feet.is_finite(), "{label}: {} escaped", f.name);
                assert!(!(f.out && f.alive), "{label}: {} is out but alive", f.name);
            }
        }
    }
}
