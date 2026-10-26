//! Juggernaut and Gun Game rules on a small hand-made arena.

use arcane_dome::data::{GameData, parse_map, parse_modes, parse_weapon};
use arcane_dome::sim::{DT, Event, Input, MAX_HEALTH, MatchConfig, Phase, World};
use std::sync::Arc;

const PISTOL: &str = r#"{ "id": "pistol", "name": "Pistol", "category": "starter",
  "damage": 10, "fire_rate_rps": 5, "magazine": 10, "reserve_ammo": 0, "reload_time_s": 1,
  "range_tiles": 40, "projectile": { "type": "bullet", "speed": 80, "lifetime_s": 1 } }"#;
const RAIL: &str = r#"{ "id": "rail", "name": "Rail", "category": "long_range",
  "damage": 40, "fire_rate_rps": 1, "magazine": 5, "reserve_ammo": 10,
  "reload_time_s": 1, "range_tiles": 60, "projectile": { "type": "hitscan" } }"#;
const SMG: &str = r#"{ "id": "smg", "name": "SMG", "category": "automatic",
  "damage": 5, "fire_rate_rps": 10, "magazine": 2, "reserve_ammo": 4, "reload_time_s": 0.2,
  "range_tiles": 30, "projectile": { "type": "hitscan" } }"#;
const BATON: &str = r#"{ "id": "baton", "name": "Baton", "category": "melee", "loot": false,
  "damage": 55, "fire_rate_rps": 2, "magazine": 0, "reserve_ammo": 0, "range_tiles": 2,
  "projectile": { "type": "melee_arc" } }"#;

const MODES: &str = r#"{
  "deathmatch": { "teams": 4, "respawn": true, "score_limit": 3, "time_limit_s": 600 },
  "gun_game": { "teams": 4, "respawn": true, "time_limit_s": 600, "rules": "gun_game",
    "ladder": ["smg", "rail", "baton"] },
  "juggernaut": { "teams": 1, "respawn": true, "score_limit": 5, "time_limit_s": 600,
    "rules": "juggernaut", "juggernaut_health": 500, "juggernaut_shield": 200 }
}"#;

fn arena_json() -> String {
    let grid = [
        "##############################",
        "#............................#",
        "#............................#",
        "#............................#",
        "##############################",
    ];
    format!(
        r##"{{ "id": "arena", "name": "Arena",
        "modes": ["deathmatch", "gun_game", "juggernaut"],
        "legend": {{ "#": "solid", ".": "air" }},
        "grid": {},
        "spawns": [
          {{"x":2,"y":3,"team":0}},{{"x":3,"y":3,"team":0}},
          {{"x":26,"y":3,"team":1}},{{"x":27,"y":3,"team":1}},
          {{"x":5,"y":3}},{{"x":10,"y":3}},{{"x":19,"y":3}},{{"x":24,"y":3}}
        ],
        "pickups": [
          {{"type":"health","x":8,"y":3,"respawn_s":10}},
          {{"type":"ammo","x":12,"y":3,"respawn_s":10}},
          {{"type":"weapon","weapon_id":"rail","x":16,"y":3,"respawn_s":10}}
        ],
        "objects": [{{"type":"weapon_station","x":21,"y":3}}] }}"##,
        serde_json::to_string(&grid).unwrap()
    )
}

fn data() -> Arc<GameData> {
    let weapons: Vec<_> = [
        ("pistol", PISTOL),
        ("rail", RAIL),
        ("smg", SMG),
        ("baton", BATON),
    ]
    .iter()
    .map(|(id, t)| parse_weapon(&format!("weapons/{id}.json"), id, t).unwrap())
    .collect();
    let ids: Vec<&str> = weapons.iter().map(|w| w.id.as_str()).collect();
    let modes = parse_modes("modes.json", MODES).unwrap();
    let mode_ids: Vec<&str> = modes.iter().map(|m| m.id.as_str()).collect();
    let map = parse_map("maps/arena.json", "arena", &arena_json(), &ids, &mode_ids).unwrap();
    Arc::new(GameData::from_parts(weapons, vec![map], modes).unwrap())
}

fn world_with(cfg: MatchConfig) -> World {
    let mut w = World::new(
        data(),
        MatchConfig {
            map_id: "arena".into(),
            countdown_s: 0.0,
            ..cfg
        },
    )
    .unwrap();
    for f in &mut w.fighters {
        f.spawn_protect = 0.0;
    }
    w
}

fn juggernaut(bots: u32) -> World {
    world_with(MatchConfig {
        mode_id: "juggernaut".into(),
        bots,
        ..Default::default()
    })
}

fn gun_game(teams: u8) -> World {
    world_with(MatchConfig {
        mode_id: "gun_game".into(),
        bots: 3,
        teams,
        team_bots: [1, 2, 0, 0],
        ..Default::default()
    })
}

/// Kill `victim` outright, credited to `by` (or to nobody) with `weapon`.
fn kill(w: &mut World, victim: usize, by: Option<usize>, weapon: Option<&str>) {
    let weapon = weapon.and_then(|id| w.data.weapon_index(id));
    w.fighters[victim].spawn_protect = 0.0;
    w.damage(victim, by, weapon, 10_000.0, false, false);
    assert!(!w.fighters[victim].alive, "the kill did not land");
}

fn run(w: &mut World, secs: f32) {
    let idle = vec![Input::default(); w.fighters.len()];
    for _ in 0..(secs / DT) as usize {
        w.step(&idle);
    }
}

fn hunter(w: &World, not: &[usize]) -> usize {
    (0..w.fighters.len())
        .find(|&i| Some(i) != w.juggernaut && !not.contains(&i))
        .unwrap()
}

#[test]
fn a_juggernaut_starts_red_with_500_health_and_200_shield() {
    let w = juggernaut(3);
    let j = w.juggernaut.expect("someone is the Juggernaut");
    let f = &w.fighters[j];
    assert_eq!((f.health, f.shield, f.max_health), (500.0, 200.0, 500.0));
    assert_eq!(f.tint, Some(1), "the Juggernaut is red");
    for o in w.fighters.iter().filter(|o| o.id != j) {
        assert_eq!((o.health, o.shield, o.tint), (MAX_HEALTH, 0.0, Some(0)));
    }
}

#[test]
fn hunters_are_one_side_and_cannot_hurt_each_other() {
    let mut w = juggernaut(3);
    let j = w.juggernaut.unwrap();
    let a = hunter(&w, &[]);
    let b = hunter(&w, &[a]);
    assert!(w.allies(a, b));
    assert!(!w.allies(a, j) && !w.allies(j, b));
    w.damage(b, Some(a), None, 50.0, false, false);
    assert_eq!(w.fighters[b].health, MAX_HEALTH);
}

#[test]
fn killing_the_juggernaut_scores_and_takes_over_the_role() {
    let mut w = juggernaut(3);
    let j = w.juggernaut.unwrap();
    let k = hunter(&w, &[]);
    kill(&mut w, j, Some(k), Some("rail"));
    assert_eq!(w.scores[k], 1);
    assert_eq!(w.juggernaut, Some(k));
    let f = &w.fighters[k];
    assert_eq!((f.health, f.shield, f.tint), (500.0, 200.0, Some(1)));
    // The old one comes back as a plain hunter.
    run(&mut w, 6.0);
    let old = &w.fighters[j];
    assert!(old.alive);
    assert_eq!(
        (old.health, old.shield, old.tint),
        (MAX_HEALTH, 0.0, Some(0))
    );
}

#[test]
fn kills_as_the_juggernaut_score_and_the_bonus_doubles_them() {
    for (bonus, points) in [(false, 1), (true, 2)] {
        let mut w = world_with(MatchConfig {
            mode_id: "juggernaut".into(),
            bots: 3,
            jugg_bonus: bonus,
            ..Default::default()
        });
        let j = w.juggernaut.unwrap();
        let v = hunter(&w, &[]);
        kill(&mut w, v, Some(j), Some("rail"));
        assert_eq!(w.scores[j], points, "bonus {bonus}");
        assert_eq!(w.juggernaut, Some(j));
    }
}

#[test]
fn scoring_switches_and_never_both_off() {
    // (kills as Juggernaut, killing the Juggernaut) -> points for each.
    let cases = [
        ((false, true), (0, 1)),
        ((true, false), (1, 0)),
        ((false, false), (1, 1)),
    ];
    for ((as_jugg, on_jugg), (got_as, got_on)) in cases {
        let mut w = world_with(MatchConfig {
            mode_id: "juggernaut".into(),
            bots: 3,
            jugg_kills_score: as_jugg,
            killing_jugg_scores: on_jugg,
            ..Default::default()
        });
        let j = w.juggernaut.unwrap();
        let v = hunter(&w, &[]);
        kill(&mut w, v, Some(j), None);
        assert_eq!(
            w.scores[j], got_as,
            "{as_jugg} {on_jugg}: kill as Juggernaut"
        );
        let k = hunter(&w, &[v]);
        kill(&mut w, j, Some(k), None);
        assert_eq!(
            w.scores[k], got_on,
            "{as_jugg} {on_jugg}: killing the Juggernaut"
        );
    }
}

#[test]
fn a_juggernaut_lost_to_the_map_or_their_own_blast_goes_to_someone_at_random() {
    for own in [false, true] {
        let mut w = juggernaut(4);
        let j = w.juggernaut.unwrap();
        // Someone hit them first: that does not make it their kill.
        let k = hunter(&w, &[]);
        w.damage(j, Some(k), None, 30.0, false, false);
        kill(&mut w, j, own.then_some(j), None);
        let new = w.juggernaut.unwrap();
        assert_ne!(new, j, "own blast: {own}");
        assert!(w.fighters[new].alive);
        assert_eq!(w.fighters[new].health, 500.0);
        assert!(w.scores.iter().all(|&s| s == 0), "nobody scores: {own}");
        assert!(
            w.events
                .iter()
                .any(|e| matches!(e, Event::Kill { killer: None, .. }))
        );
    }
}

#[test]
fn health_packs_heal_a_juggernaut_up_to_their_own_cap() {
    let mut w = juggernaut(1);
    let j = w.juggernaut.unwrap();
    w.fighters[j].health = 300.0;
    let pack = w.pickups[0].pos;
    w.fighters[j].feet = pack + macroquad::math::vec2(0.0, 0.4);
    run(&mut w, 0.1);
    assert_eq!(w.fighters[j].health, 350.0);
}

#[test]
fn juggernaut_reaching_the_score_limit_wins() {
    let mut w = juggernaut(5);
    let j = w.juggernaut.unwrap();
    for v in (0..w.fighters.len()).filter(|&i| i != j) {
        kill(&mut w, v, Some(j), None);
    }
    run(&mut w, DT);
    assert_eq!(w.phase, Phase::MatchOver { winner: Some(j) });
}

#[test]
fn gun_game_starts_everyone_on_the_first_weapon_with_the_baton_beside_it() {
    let w = gun_game(0);
    let smg = w.data.weapon_index("smg").unwrap();
    let baton = w.data.weapon_index("baton").unwrap();
    for f in &w.fighters {
        assert_eq!(f.current, smg);
        let owned: Vec<usize> = (0..f.inventory.len()).filter(|&k| f.owns(k)).collect();
        assert_eq!(owned, vec![smg, baton], "{}", f.name);
    }
    // Weapons come from the ladder only; health packs stay.
    assert_eq!(w.pickups.len(), 1);
    assert!(w.objects.is_empty());
    assert_eq!(w.mode.score_limit, Some(3));
}

#[test]
fn a_gun_game_kill_moves_the_killer_to_the_next_weapon() {
    let mut w = gun_game(0);
    kill(&mut w, 1, Some(0), Some("smg"));
    assert_eq!(w.scores[0], 1);
    let rail = w.data.weapon_index("rail").unwrap();
    assert_eq!(w.fighters[0].current, rail);
    assert!(!w.fighters[0].owns(w.data.weapon_index("smg").unwrap()));
    assert!(w.fighters[0].owns(w.data.weapon_index("baton").unwrap()));
    assert_eq!(w.scores[1], 0);
    // On the last rung the baton is all there is.
    kill(&mut w, 2, Some(0), Some("rail"));
    let owned: Vec<usize> = (0..w.fighters[0].inventory.len())
        .filter(|&k| w.fighters[0].owns(k))
        .collect();
    assert_eq!(owned, vec![w.data.weapon_index("baton").unwrap()]);
}

#[test]
fn gun_game_ammo_never_runs_out() {
    let mut w = gun_game(0);
    // Straight up, into the ceiling.
    let fire = Input {
        fire: true,
        aim: w.fighters[0].feet - macroquad::math::vec2(0.0, 10.0),
        ..Default::default()
    };
    let mut inputs = vec![Input::default(); w.fighters.len()];
    inputs[0] = fire;
    // The SMG holds 2 rounds with 4 in reserve: plenty of reloads later it
    // still shoots.
    let mut shots = 0;
    for _ in 0..(6.0 / DT) as usize {
        w.step(&inputs);
        shots += w
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { fighter: 0, .. }))
            .count();
    }
    assert!(shots > 12, "only {shots} shots");
}

#[test]
fn melee_kills_and_suicides_knock_you_back_when_switched_on() {
    for on in [true, false] {
        let mut w = world_with(MatchConfig {
            mode_id: "gun_game".into(),
            bots: 3,
            melee_demotes: on,
            suicide_demotes: on,
            ..Default::default()
        });
        w.scores = vec![2, 2, 2, 2];
        kill(&mut w, 1, Some(0), Some("baton"));
        assert_eq!(w.scores[1], if on { 1 } else { 2 }, "melee, switch {on}");
        kill(&mut w, 2, None, None);
        assert_eq!(w.scores[2], if on { 1 } else { 2 }, "suicide, switch {on}");
        // Never below the first weapon.
        w.scores[3] = 0;
        kill(&mut w, 3, Some(3), None);
        assert_eq!(w.scores[3], 0);
    }
}

#[test]
fn a_kill_with_the_last_weapon_wins_gun_game() {
    let mut w = gun_game(0);
    w.scores[0] = 2;
    kill(&mut w, 1, Some(0), Some("baton"));
    run(&mut w, DT);
    assert_eq!(w.phase, Phase::MatchOver { winner: Some(0) });
}

#[test]
fn team_gun_game_moves_the_whole_team_up_together() {
    let mut w = gun_game(2);
    let mate = (1..w.fighters.len())
        .find(|&i| w.fighters[i].team == Some(0))
        .unwrap();
    let foe = (1..w.fighters.len())
        .find(|&i| w.fighters[i].team == Some(1))
        .unwrap();
    kill(&mut w, foe, Some(mate), Some("smg"));
    let rail = w.data.weapon_index("rail").unwrap();
    assert_eq!(w.scores[0], 1);
    assert_eq!(w.fighters[0].current, rail, "the player moves up too");
    assert_eq!(w.fighters[mate].current, rail);
    // The fallen foe comes back on their team's weapon.
    run(&mut w, 6.0);
    assert_eq!(w.fighters[foe].current, w.data.weapon_index("smg").unwrap());
}

#[test]
fn melee_only_weapons_are_never_handed_out_at_random() {
    let baton = data().weapon_index("baton").unwrap();
    for seed in 0..40 {
        let w = world_with(MatchConfig {
            mode_id: "deathmatch".into(),
            bots: 5,
            seed,
            ..Default::default()
        });
        assert!(w.fighters.iter().all(|f| !f.owns(baton)), "seed {seed}");
        assert!(w.objects.iter().all(|o| o.weapon != Some(baton)));
    }
}
