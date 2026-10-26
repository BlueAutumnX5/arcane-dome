//! King of the Hill, Domination, Territories and Capture the Flag on a
//! small hand-made arena with three zones and two flag bases.

use arcane_dome::data::{GameData, parse_map, parse_modes, parse_weapon};
use arcane_dome::sim::{DT, Event, FlagEvent, Input, MatchConfig, Phase, World};
use macroquad::math::vec2;
use std::sync::Arc;

const PISTOL: &str = r#"{ "id": "pistol", "name": "Pistol", "category": "starter",
  "damage": 10, "fire_rate_rps": 5, "magazine": 10, "reserve_ammo": 0, "reload_time_s": 1,
  "range_tiles": 40, "projectile": { "type": "hitscan" } }"#;

const MODES: &str = r#"{
  "king_of_the_hill": { "teams": 4, "respawn": true, "score_limit": 100, "time_limit_s": 600,
    "rules": "king_of_the_hill", "hill_move_s": 30 },
  "domination": { "teams": 4, "team_only": true, "respawn": true, "score_limit": 200,
    "time_limit_s": 600, "rules": "domination", "capture_s": 6, "zone_points_per_s": 0.5 },
  "territories": { "teams": 2, "team_only": true, "respawn": true, "score_limit": 3,
    "time_limit_s": 600, "rules": "territories", "capture_s": 6 },
  "capture_the_flag": { "teams": 4, "team_only": true, "respawn": true, "score_limit": 3,
    "time_limit_s": 600, "rules": "capture_the_flag", "flag_return_s": 15 }
}"#;

/// Floor along row 4 (feet at y = 4), 40 wide. Zones centred at x = 10,
/// 20 and 30; Blue's flag at 3, Red's at 37.
fn arena_json() -> String {
    let grid = [
        "########################################",
        "#......................................#",
        "#......................................#",
        "#......................................#",
        "########################################",
    ];
    format!(
        r##"{{ "id": "arena", "name": "Arena",
        "modes": ["king_of_the_hill", "domination", "territories", "capture_the_flag"],
        "legend": {{ "#": "solid", ".": "air" }},
        "grid": {},
        "team_counts": [2],
        "spawns": [
          {{"x":1,"y":3,"team":0}},{{"x":2,"y":3,"team":0}},
          {{"x":37,"y":3,"team":1}},{{"x":38,"y":3,"team":1}},
          {{"x":5,"y":3}},{{"x":15,"y":3}},{{"x":25,"y":3}},{{"x":35,"y":3}}
        ],
        "zones": [{{"x":19.5,"y":3,"w":4}},{{"x":9.5,"y":3,"w":4}},{{"x":29.5,"y":3,"w":4}}],
        "flags": [{{"x":2.5,"y":3,"team":0}},{{"x":36.5,"y":3,"team":1}}] }}"##,
        serde_json::to_string(&grid).unwrap()
    )
}

fn data() -> Arc<GameData> {
    let weapons = vec![parse_weapon("weapons/pistol.json", "pistol", PISTOL).unwrap()];
    let modes = parse_modes("modes.json", MODES).unwrap();
    let mode_ids: Vec<&str> = modes.iter().map(|m| m.id.as_str()).collect();
    let map = parse_map(
        "maps/arena.json",
        "arena",
        &arena_json(),
        &["pistol"],
        &mode_ids,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    for m in &modes {
        assert_eq!(m.map_problem(&map), None, "{}", m.id);
    }
    Arc::new(GameData::from_parts(weapons, vec![map], modes).unwrap())
}

/// Blue: the player and one bot (fighters 0 and 1); Red: two bots (2, 3).
/// Everyone starts parked at the far left or right edge, out of every zone.
fn world(mode: &str, teams: u8) -> World {
    let mut w = World::new(
        data(),
        MatchConfig {
            map_id: "arena".into(),
            mode_id: mode.into(),
            teams,
            bots: 3,
            team_bots: [1, 2, 0, 0],
            countdown_s: 0.0,
            ..Default::default()
        },
    )
    .unwrap();
    for i in 0..w.fighters.len() {
        let x = if w.side_of(i) == 0 && teams >= 2 {
            1.0
        } else {
            39.0
        };
        put(&mut w, i, x);
    }
    w
}

fn put(w: &mut World, i: usize, x: f32) {
    let f = &mut w.fighters[i];
    f.feet = vec2(x, 4.0);
    f.vel = vec2(0.0, 0.0);
    f.spawn_protect = 0.0;
}

fn run(w: &mut World, secs: f32) -> Vec<Event> {
    let idle = vec![Input::default(); w.fighters.len()];
    let mut seen = Vec::new();
    for _ in 0..(secs / DT).round() as usize {
        w.step(&idle);
        seen.extend(w.events.iter().cloned());
    }
    seen
}

fn zone_x(w: &World, k: usize) -> f32 {
    w.zones[k].def.x
}

#[test]
fn zones_are_numbered_left_to_right() {
    let w = world("domination", 2);
    let xs: Vec<f32> = w.zones.iter().map(|z| z.def.x).collect();
    assert_eq!(xs, vec![10.0, 20.0, 30.0]);
}

#[test]
fn whoever_alone_holds_the_hill_scores_a_point_a_second() {
    let mut w = world("king_of_the_hill", 2);
    let h = w.hill.unwrap();
    let x = zone_x(&w, h);
    put(&mut w, 0, x);
    run(&mut w, 5.0);
    assert_eq!(w.scores, vec![5, 0]);
    assert_eq!(w.zones[h].owner, Some(0));
    // Contested: nobody scores.
    let x = zone_x(&w, h) + 0.5;
    put(&mut w, 2, x);
    run(&mut w, 3.0);
    assert_eq!(w.scores, vec![5, 0]);
    assert_eq!(w.zones[h].owner, None);
}

#[test]
fn free_for_all_king_of_the_hill_scores_each_fighter() {
    let mut w = world("king_of_the_hill", 0);
    let h = w.hill.unwrap();
    let x = zone_x(&w, h);
    put(&mut w, 2, x);
    run(&mut w, 4.0);
    assert_eq!(w.scores[2], 4);
    assert_eq!(w.scores.iter().sum::<i32>(), 4);
}

#[test]
fn the_hill_moves_on_its_timer_unless_switched_off() {
    let mut w = world("king_of_the_hill", 2);
    let first = w.hill.unwrap();
    let events = run(&mut w, 30.5);
    let moved = events
        .iter()
        .filter(|e| matches!(e, Event::HillMoved { .. }))
        .count();
    assert_eq!(moved, 1);
    assert_ne!(w.hill.unwrap(), first, "never the same zone twice in a row");

    let mut w = World::new(
        data(),
        MatchConfig {
            map_id: "arena".into(),
            mode_id: "king_of_the_hill".into(),
            hill_move_s: Some(0.0),
            countdown_s: 0.0,
            ..Default::default()
        },
    )
    .unwrap();
    let first = w.hill;
    run(&mut w, 70.0);
    assert_eq!(w.hill, first);
}

#[test]
fn a_lone_fighter_captures_a_zone_in_six_seconds_and_a_pair_faster() {
    let mut w = world("domination", 2);
    let x = zone_x(&w, 0);
    put(&mut w, 0, x);
    run(&mut w, 5.8);
    assert_eq!(w.zones[0].owner, None);
    assert_eq!(w.zones[0].capturer, Some(0));
    run(&mut w, 0.4);
    assert_eq!(w.zones[0].owner, Some(0));

    // Two together: 1.5 times as fast, so 4 seconds.
    let x = zone_x(&w, 2);
    put(&mut w, 2, x);
    let x = zone_x(&w, 2) + 0.5;
    put(&mut w, 3, x);
    run(&mut w, 4.1);
    assert_eq!(w.zones[2].owner, Some(1));
}

#[test]
fn a_contested_zone_does_not_change_hands() {
    let mut w = world("domination", 2);
    let x = zone_x(&w, 1);
    put(&mut w, 0, x);
    let x = zone_x(&w, 1) + 0.5;
    put(&mut w, 2, x);
    run(&mut w, 10.0);
    assert_eq!(w.zones[1].owner, None);
    assert_eq!(w.zones[1].progress, 0.0);
}

#[test]
fn taking_an_enemy_zone_undoes_their_progress_first() {
    let mut w = world("domination", 2);
    let x = zone_x(&w, 1);
    put(&mut w, 0, x);
    run(&mut w, 3.0);
    put(&mut w, 0, 1.0);
    let x = zone_x(&w, 1);
    put(&mut w, 2, x);
    // Blue had half; Red spends 3 s undoing it, then 6 s capturing.
    run(&mut w, 8.5);
    assert_eq!(w.zones[1].owner, None);
    run(&mut w, 0.7);
    assert_eq!(w.zones[1].owner, Some(1));
}

#[test]
fn held_zones_earn_domination_points() {
    let mut w = world("domination", 2);
    let x = zone_x(&w, 0);
    put(&mut w, 0, x);
    let x = zone_x(&w, 1);
    put(&mut w, 1, x);
    run(&mut w, 6.1);
    assert_eq!((w.zones[0].owner, w.zones[1].owner), (Some(0), Some(0)));
    // Two zones at half a point a second each: a point a second.
    let before = w.scores[0];
    run(&mut w, 10.0);
    assert_eq!(w.scores[0] - before, 10);
    assert_eq!(w.scores[1], 0);
}

#[test]
fn domination_needs_teams_even_when_free_for_all_was_picked() {
    let w = world("domination", 0);
    assert!(w.mode.is_team());
    assert_eq!(w.num_sides(), 2);
}

#[test]
fn territories_start_with_each_team_holding_its_own_end() {
    let w = world("territories", 2);
    let owners: Vec<_> = w.zones.iter().map(|z| z.owner).collect();
    assert_eq!(owners, vec![Some(0), None, Some(1)]);
}

#[test]
fn territories_push_one_zone_at_a_time_and_score_for_holding_all() {
    let mut w = world("territories", 2);
    // Blue can't jump the line to Red's end zone.
    let x = zone_x(&w, 2);
    put(&mut w, 0, x);
    run(&mut w, 7.0);
    assert_eq!(w.zones[2].owner, Some(1));
    assert!(!w.can_push(2, 0));
    // The middle first, then Red's end.
    let x = zone_x(&w, 1);
    put(&mut w, 0, x);
    run(&mut w, 6.2);
    assert_eq!(w.zones[1].owner, Some(0));
    let x = zone_x(&w, 2);
    put(&mut w, 0, x);
    let events = run(&mut w, 6.2);
    assert_eq!(w.scores, vec![1, 0]);
    assert!(events.contains(&Event::Pushed { side: 0 }));
    // The line resets.
    let owners: Vec<_> = w.zones.iter().map(|z| z.owner).collect();
    assert_eq!(owners, vec![Some(0), None, Some(1)]);
}

#[test]
fn bring_the_enemy_flag_home_to_score() {
    let mut w = world("capture_the_flag", 2);
    assert_eq!(w.flags.len(), 2);
    let red = w.flags.iter().position(|f| f.side == 1).unwrap();
    put(&mut w, 0, 36.5);
    let events = run(&mut w, 0.1);
    assert_eq!(w.flags[red].carrier, Some(0));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Flag {
            what: FlagEvent::Taken,
            ..
        }
    )));
    put(&mut w, 0, 2.5);
    run(&mut w, 0.1);
    assert_eq!(w.scores, vec![1, 0]);
    assert!(w.flags[red].at_home());
    assert_eq!(w.flags[red].pos, w.flags[red].home);
}

#[test]
fn no_capture_while_your_own_flag_is_away() {
    let mut w = world("capture_the_flag", 2);
    let (blue, red) = (0, 1);
    put(&mut w, 0, 36.5);
    run(&mut w, 0.1);
    put(&mut w, 2, 2.5);
    run(&mut w, 0.1);
    assert_eq!(w.flags[blue].carrier, Some(2));
    put(&mut w, 0, 2.5);
    run(&mut w, 0.5);
    assert_eq!(w.scores, vec![0, 0]);
    assert_eq!(w.flags[red].carrier, Some(0));
}

#[test]
fn a_fallen_carrier_drops_the_flag_which_its_team_can_return() {
    let mut w = world("capture_the_flag", 2);
    let red = 1;
    put(&mut w, 0, 36.5);
    run(&mut w, 0.1);
    put(&mut w, 0, 20.0);
    w.damage(0, Some(2), None, 1_000.0, false, false);
    assert!(!w.fighters[0].alive);
    let f = &w.flags[red];
    assert_eq!((f.carrier, f.pos), (None, vec2(20.0, 4.0)));
    assert_eq!(f.return_in, Some(15.0));
    // A Red fighter touches it: straight home.
    put(&mut w, 2, 20.0);
    run(&mut w, 0.1);
    assert!(w.flags[red].at_home());
}

#[test]
fn a_dropped_flag_goes_home_by_itself_and_blue_can_pick_it_up_again() {
    let mut w = world("capture_the_flag", 2);
    let red = 1;
    put(&mut w, 0, 36.5);
    run(&mut w, 0.1);
    put(&mut w, 0, 20.0);
    w.damage(0, Some(2), None, 1_000.0, false, false);
    // A Blue teammate picks the dropped flag up where it fell.
    put(&mut w, 1, 20.0);
    run(&mut w, 0.1);
    assert_eq!(w.flags[red].carrier, Some(1));
    put(&mut w, 1, 30.0);
    w.damage(1, Some(2), None, 1_000.0, false, false);
    run(&mut w, 14.8);
    assert!(!w.flags[red].at_home());
    run(&mut w, 0.3);
    assert!(w.flags[red].at_home());
}

#[test]
fn captures_reach_the_score_limit() {
    let mut w = world("capture_the_flag", 2);
    for _ in 0..3 {
        put(&mut w, 0, 36.5);
        run(&mut w, 0.1);
        put(&mut w, 0, 2.5);
        run(&mut w, 0.1);
    }
    assert_eq!(w.phase, Phase::MatchOver { winner: Some(0) });
}

#[test]
fn kills_do_not_score_in_objective_modes() {
    for mode in [
        "king_of_the_hill",
        "domination",
        "territories",
        "capture_the_flag",
    ] {
        let mut w = world(mode, 2);
        w.damage(2, Some(0), None, 1_000.0, false, false);
        assert_eq!(w.fighters[0].kills, 1, "{mode}");
        assert!(w.scores.iter().all(|&s| s == 0), "{mode}");
    }
}

#[test]
fn maps_must_carry_what_their_modes_need() {
    let data = data();
    let mut map = data.maps[0].clone();
    let problem = |id: &str, map: &arcane_dome::data::MapDef| {
        data.mode(id).unwrap().map_problem(map).unwrap_or_default()
    };
    map.zones.truncate(2);
    assert_eq!(problem("domination", &map), "");
    assert!(problem("territories", &map).contains("at least 3 zones"));
    map.zones.clear();
    assert!(problem("king_of_the_hill", &map).contains("at least 1 zone"));
    map.flags.pop();
    assert!(problem("capture_the_flag", &map).contains("one flag for team 1 in 2 team matches"));
}
