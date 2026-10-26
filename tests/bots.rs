//! Headless bot-only matches on the shipped maps.

use arcane_dome::ai::BotSet;
use arcane_dome::data::GameData;
use arcane_dome::sim::{Difficulty, Event, Input, MatchConfig, Phase, TICK_RATE, World};
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
fn bots_hunt_the_juggernaut_and_climb_the_gun_game_ladder() {
    let data = shipped();
    for map in &data.maps {
        for (mode, teams) in [("juggernaut", 0), ("gun_game", 0), ("gun_game", 2)] {
            let cfg = MatchConfig {
                map_id: map.id.clone(),
                mode_id: mode.into(),
                bots: 7,
                teams,
                team_bots: [3, 4, 0, 0],
                difficulty: Difficulty::Hard,
                team_difficulty: [Difficulty::Hard; 4],
                include_player: false,
                seed: 11,
                countdown_s: 0.0,
                ..Default::default()
            };
            let mut world = World::new(Arc::clone(&data), cfg).unwrap();
            let mut bots = BotSet::new(&world);
            let mut inputs = vec![Input::default(); world.fighters.len()];
            let (mut new_juggs, mut climbs) = (0, 0);
            for _ in 0..120 * TICK_RATE {
                bots.think(&world, &mut inputs);
                world.step(&inputs);
                for e in &world.events {
                    match e {
                        Event::Juggernaut { .. } => new_juggs += 1,
                        Event::Ladder { up: true, .. } => climbs += 1,
                        _ => {}
                    }
                }
            }
            let label = format!("{} / {mode} ({teams} teams)", map.id);
            if mode == "juggernaut" {
                assert!(new_juggs >= 2, "{label}: the Juggernaut never fell");
                assert!(
                    world.scores.iter().sum::<i32>() > 0,
                    "{label}: nobody scored"
                );
            } else {
                assert!(climbs >= 3, "{label}: only {climbs} climbs in 120s");
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

/// A flat, open field 60 x 30 (floor at y 26, no ceiling in reach), with the
/// game's own weapons and modes.
fn open_field() -> Arc<GameData> {
    use arcane_dome::data::parse_map;
    let shipped = shipped();
    let spawns: Vec<String> = [10, 20, 40, 50]
        .iter()
        .map(|x| format!(r#"{{"x": {x}, "y": 26}}"#))
        .chain(
            [5, 7]
                .iter()
                .map(|x| format!(r#"{{"x": {x}, "y": 26, "team": 0}}"#)),
        )
        .chain(
            [53, 55]
                .iter()
                .map(|x| format!(r#"{{"x": {x}, "y": 26, "team": 1}}"#)),
        )
        .collect();
    let text = format!(
        r#"{{"id": "field", "name": "Field", "size": [60, 30], "modes": ["deathmatch"],
            "shapes": [{{"kind": "solid", "material": "rock",
                         "points": [[0, 26], [60, 26], [60, 30], [0, 30]]}}],
            "spawns": [{}]}}"#,
        spawns.join(", ")
    );
    let weapons: Vec<&str> = shipped.weapons.iter().map(|w| w.id.as_str()).collect();
    let modes: Vec<&str> = shipped.modes.iter().map(|m| m.id.as_str()).collect();
    let map = parse_map("field.json", "field", &text, &weapons, &modes).unwrap();
    let mut data =
        GameData::from_parts(shipped.weapons.clone(), vec![map], shipped.modes.clone()).unwrap();
    data.powerups = shipped.powerups.clone();
    Arc::new(data)
}

/// How far from a target 12 tiles away each of a grenade bot's first shots
/// first comes down (0 for a direct hit before landing). Target and bot
/// stand still and the target cannot die: this measures the bot's judgement
/// of the range, not its footwork or the grenades' rolling.
fn grenade_landings(difficulty: Difficulty, seed: u64, shots: usize) -> Vec<f32> {
    use arcane_dome::sim::Event;
    use macroquad::math::vec2;
    let data = open_field();
    let cfg = MatchConfig {
        map_id: "field".into(),
        bots: 1,
        difficulty,
        seed,
        countdown_s: 0.0,
        powerups: false,
        ..Default::default()
    };
    let mut w = World::new(Arc::clone(&data), cfg).unwrap();
    let gl = data.weapon_index("thumper").unwrap();
    // The bot owns only the grenade launcher, with plenty of ammo.
    for (k, slot) in w.fighters[1].inventory.iter_mut().enumerate() {
        slot.owned = k == gl;
        slot.mag = if k == gl { 99 } else { 0 };
        slot.reserve = if k == gl { 99 } else { 0 };
    }
    w.fighters[1].current = gl;
    let mut bots = BotSet::new(&w);
    let mut inputs = vec![Input::default(); w.fighters.len()];
    // Shots in flight: id -> last vertical speed; landed ones are removed.
    let mut flying: std::collections::HashMap<u32, f32> = Default::default();
    let mut landed: Vec<(u32, f32)> = Vec::new();
    for _ in 0..40 * TICK_RATE {
        w.fighters[0].feet = vec2(20.0, 26.0);
        w.fighters[0].vel = vec2(0.0, 0.0);
        w.fighters[0].health = 100.0;
        w.fighters[0].spawn_protect = 0.0;
        w.fighters[1].feet = vec2(32.0, 26.0);
        w.fighters[1].vel = vec2(0.0, 0.0);
        bots.think(&w, &mut inputs);
        inputs[0] = Input::default();
        w.step(&inputs);
        let target = w.fighters[0].aabb().center();
        for p in w.projectiles.iter().filter(|p| p.owner == 1) {
            if landed.iter().any(|(id, _)| *id == p.id) {
                continue;
            }
            let vy = flying.insert(p.id, p.vel.y).unwrap_or(p.vel.y);
            if vy > 0.0 && p.vel.y <= 0.0 {
                landed.push((p.id, (p.pos.x - target.x).abs()));
            }
        }
        for e in &w.events {
            if let Event::Impact {
                owner: 1, id, pos, ..
            } = e
                && !landed.iter().any(|(l, _)| l == id)
            {
                // Went off before landing: a direct hit, or wherever it ended.
                let d = if pos.distance(target) < 1.5 {
                    0.0
                } else {
                    (pos.x - target.x).abs()
                };
                landed.push((*id, d));
            }
        }
        if landed.len() >= shots {
            break;
        }
    }
    landed.sort_by_key(|(id, _)| *id);
    landed.iter().map(|(_, d)| *d).collect()
}

#[test]
fn grenade_bots_walk_their_shots_in() {
    for difficulty in [Difficulty::Normal, Difficulty::Hard] {
        // Seeds far apart (nearby ones can play out the same).
        let runs: Vec<Vec<f32>> = (1..=24)
            .map(|s| grenade_landings(difficulty, s * 7919, 5))
            .collect();
        // Average distance the shots came down from the target.
        let off = |shots: std::ops::Range<usize>| {
            let v: Vec<f32> = runs
                .iter()
                .flat_map(|m| m.get(shots.clone()).unwrap_or(&[]).to_vec())
                .collect();
            assert!(v.len() >= 20, "too few shots fired: {runs:?}");
            v.iter().sum::<f32>() / v.len() as f32
        };
        let first = off(0..1);
        let later = off(2..5);
        let good_first = runs
            .iter()
            .filter(|m| m.first().is_some_and(|d| *d < 1.5))
            .count();
        eprintln!(
            "{difficulty:?}: first shot lands {first:.2} off, shots 3-5 {later:.2} off, {good_first}/24 first shots on target"
        );
        // Usually off at first, closer once ranged in...
        assert!(first > later * 1.3, "{difficulty:?}: {runs:?}");
        // ...but not always a dumb first miss.
        assert!(good_first >= 3, "{difficulty:?}: {runs:?}");
    }
}
