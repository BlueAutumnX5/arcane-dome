//! Movement, damage and match-rule tests on a small hand-made arena.

use arcane_dome::data::{GameData, parse_map, parse_modes, parse_powerups, parse_weapon};
use arcane_dome::sim::{
    DT, Event, Input, MAX_HEALTH, MAX_SHIELD, MatchConfig, Phase, STAND_SIZE, World,
};
use macroquad::math::{Vec2, vec2};
use std::sync::Arc;

const PISTOL: &str = r#"{ "id": "pistol", "name": "Pistol", "category": "starter",
  "damage": 10, "fire_rate_rps": 5, "magazine": 10, "reserve_ammo": 0, "reload_time_s": 1,
  "range_tiles": 40, "projectile": { "type": "bullet", "speed": 80, "lifetime_s": 1 } }"#;
const RAIL: &str = r#"{ "id": "rail", "name": "Rail", "category": "long_range",
  "damage": 40, "headshot_mult": 2.5, "fire_rate_rps": 1, "magazine": 5, "reserve_ammo": 10,
  "reload_time_s": 1, "range_tiles": 60, "projectile": { "type": "hitscan" } }"#;
const BOOM: &str = r#"{ "id": "boom", "name": "Boom", "category": "explosive",
  "damage": 0, "fire_rate_rps": 1, "magazine": 1, "reserve_ammo": 5, "range_tiles": 30,
  "projectile": { "type": "rocket", "speed": 30, "lifetime_s": 2,
    "explosion": { "radius_tiles": 4, "damage": 80, "falloff": "linear", "knockback": 10 } } }"#;
const LEECH: &str = r#"{ "id": "leech", "name": "Leech", "category": "automatic",
  "damage": 20, "fire_rate_rps": 10, "magazine": 30, "reserve_ammo": 0, "range_tiles": 30,
  "projectile": { "type": "hitscan" }, "specials": [{ "type": "lifesteal", "ratio": 0.5 }] }"#;

const POWERUPS: &str = r##"{ "first_spawn_s": 1, "respawn_s": 120, "powerups": [
  { "id": "divine_shield", "name": "Divine Shield", "color": "#a45cff", "duration_s": 20, "effect": { "type": "invulnerable" } },
  { "id": "rapid_fire", "name": "Rapid Fire", "color": "#ffd23c", "duration_s": 20, "effect": { "type": "fire_rate", "mult": 2 } },
  { "id": "critical_strike", "name": "Critical Strike", "color": "#ff3c3c", "duration_s": 20, "effect": { "type": "damage", "mult": 2 } },
  { "id": "shadowstep", "name": "Shadowstep", "color": "#3c8cff", "effect": { "type": "teleport", "charges": 1 } }
] }"##;

const MODES: &str = r#"{
  "deathmatch":  { "teams": 4, "respawn": true,  "score_limit": 3, "team_score_limit": 5, "time_limit_s": 600 },
  "elimination": { "teams": 4, "respawn": false, "rounds": 3, "time_limit_s": 60 }
}"#;

fn arena_json() -> String {
    let grid = [
        "##############################",
        "#............................#",
        "#............................#",
        "#............................#",
        "#............................#",
        "#............................#",
        "#............................#",
        "##############################",
    ];
    format!(
        r##"{{ "id": "arena", "name": "Arena",
        "modes": ["deathmatch", "elimination"],
        "legend": {{ "#": "solid", ".": "air" }},
        "grid": {},
        "spawns": [
          {{"x":2,"y":6,"team":0}},{{"x":3,"y":6,"team":0}},
          {{"x":26,"y":6,"team":1}},{{"x":27,"y":6,"team":1}},
          {{"x":5,"y":6}},{{"x":10,"y":6}},{{"x":19,"y":6}},{{"x":24,"y":6}}
        ],
        "pickups": [{{"type":"health","x":8,"y":6,"respawn_s":10}}],
        "objects": [
          {{"type":"thruster","x":12,"y":6,"height_tiles":4}},
          {{"type":"shield_station","x":21,"y":6,"shield":50}},
          {{"type":"powerup_spot","x":22,"y":6}}
        ] }}"##,
        serde_json::to_string(&grid).unwrap()
    )
}

fn data() -> Arc<GameData> {
    let weapons = [
        ("pistol", PISTOL),
        ("rail", RAIL),
        ("boom", BOOM),
        ("leech", LEECH),
    ]
    .iter()
    .map(|(id, t)| parse_weapon(&format!("weapons/{id}.json"), id, t).unwrap())
    .collect::<Vec<_>>();
    let ids: Vec<&str> = weapons.iter().map(|w| w.id.as_str()).collect();
    let modes = parse_modes("modes.json", MODES).unwrap();
    let mode_ids: Vec<&str> = modes.iter().map(|m| m.id.as_str()).collect();
    let map = parse_map("maps/arena.json", "arena", &arena_json(), &ids, &mode_ids).unwrap();
    let mut data = GameData::from_parts(weapons, vec![map], modes).unwrap();
    data.powerups = parse_powerups("powerups.json", POWERUPS).unwrap();
    Arc::new(data)
}

/// "team_deathmatch" means deathmatch with teams on, "deathmatch" without.
fn mode_and_teams(mode: &str, teams: u8) -> (String, u8) {
    match mode.strip_prefix("team_") {
        Some(base) => (base.to_owned(), teams),
        None => (mode.to_owned(), 0),
    }
}

fn world(mode: &str, enemies: u32, friends: u32) -> World {
    let (mode, teams) = mode_and_teams(mode, 2);
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: mode,
        bots: enemies,
        teams,
        team_bots: [friends, enemies, 0, 0],
        countdown_s: 0.0,
        ..Default::default()
    };
    let mut w = World::new(data(), cfg).unwrap();
    for f in &mut w.fighters {
        f.spawn_protect = 0.0;
    }
    w
}

/// Put fighter `i` standing at tile column `x` with `weapon` equipped and ready.
fn place(w: &mut World, i: usize, x: f32, weapon: &str) {
    let wi = w.data.weapon_index(weapon).unwrap();
    let def = w.data.weapons[wi].clone();
    let f = &mut w.fighters[i];
    f.feet = vec2(x, 7.0);
    f.vel = Vec2::ZERO;
    f.on_ground = true;
    f.current = wi;
    f.inventory[wi].owned = true;
    f.inventory[wi].mag = def.magazine;
    f.inventory[wi].reserve = def.reserve_ammo;
    f.fire_cooldown = 0.0;
    f.reload_timer = 0.0;
    f.spawn_protect = 0.0;
}

fn idle(n: usize) -> Vec<Input> {
    vec![Input::default(); n]
}

fn run(w: &mut World, ticks: usize) {
    let n = w.fighters.len();
    for _ in 0..ticks {
        w.step(&idle(n));
    }
}

fn aim_at(_w: &World, _i: usize, target: Vec2) -> Input {
    Input {
        aim: target,
        fire: true,
        ..Default::default()
    }
}

#[test]
fn fighters_settle_on_the_floor() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 60);
    for f in &w.fighters {
        assert_eq!(f.feet.y, 7.0);
        assert!(f.on_ground);
    }
}

#[test]
fn double_jump_works_exactly_once() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 30);
    let jump = Input {
        jump: true,
        jump_held: true,
        ..Default::default()
    };
    let hold = Input {
        jump_held: true,
        ..Default::default()
    };
    w.step(&[jump]);
    assert!(w.fighters[0].vel.y < -15.0, "first jump");
    for _ in 0..10 {
        w.step(&[hold]);
    }
    w.step(&[jump]);
    let after_double = w.fighters[0].vel.y;
    assert!(after_double < -13.0, "double jump: {after_double}");
    for _ in 0..5 {
        w.step(&[hold]);
    }
    let before = w.fighters[0].vel.y;
    w.step(&[jump]);
    assert!(w.fighters[0].vel.y > before, "third jump must do nothing");
}

#[test]
fn releasing_jump_early_gives_a_short_hop() {
    let mut a = world("deathmatch", 1, 0);
    let mut b = a.clone();
    run(&mut a, 30);
    run(&mut b, 30);
    let press = Input {
        jump: true,
        jump_held: true,
        ..Default::default()
    };
    let held = Input {
        jump_held: true,
        ..Default::default()
    };
    a.step(&[press]);
    b.step(&[press]);
    let (mut top_a, mut top_b) = (7.0f32, 7.0f32);
    for _ in 0..60 {
        a.step(&[held]);
        b.step(&[Input::default()]);
        top_a = top_a.min(a.fighters[0].feet.y);
        top_b = top_b.min(b.fighters[0].feet.y);
    }
    assert!(top_b > top_a + 1.0, "short hop {top_b} vs full {top_a}");
}

#[test]
fn crouching_is_shorter_and_slower() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 30);
    let start = w.fighters[0].feet.x;
    let crouch_walk = Input {
        crouch: true,
        move_x: 1.0,
        ..Default::default()
    };
    for _ in 0..30 {
        w.step(&[crouch_walk]);
    }
    let f = &w.fighters[0];
    assert!(f.crouching);
    assert!(f.size().y < STAND_SIZE.y);
    let crouch_dist = f.feet.x - start;
    assert!(crouch_dist > 0.5 && crouch_dist < 2.5, "{crouch_dist}");
}

#[test]
fn headshots_use_the_multiplier() {
    let mut w = world("deathmatch", 2, 0);
    place(&mut w, 0, 5.0, "rail");
    place(&mut w, 1, 15.0, "pistol");
    place(&mut w, 2, 25.0, "pistol");
    // Body shot at the waist, then a head shot near the top of the body.
    let body = vec2(15.0, 6.2);
    let head = vec2(15.0, 5.35);
    let i = aim_at(&w, 0, body);
    w.step(&[i]);
    assert_eq!(w.fighters[1].health, MAX_HEALTH - 40.0);

    place(&mut w, 0, 5.0, "rail");
    place(&mut w, 2, 15.0, "pistol");
    w.fighters[1].feet = vec2(25.0, 7.0);
    let i = aim_at(&w, 0, head);
    w.step(&[i]);
    assert_eq!(w.fighters[2].health, MAX_HEALTH - 100.0);
    assert!(!w.fighters[2].alive);
}

#[test]
fn kills_score_and_dead_fighters_respawn() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 10);
    w.damage(1, Some(0), None, 150.0, false, false);
    assert!(!w.fighters[1].alive);
    assert_eq!(w.fighters[0].kills, 1);
    assert_eq!(w.fighters[1].deaths, 1);
    assert_eq!(w.scores[0], 1);
    run(&mut w, (3.0 / DT) as usize + 2);
    assert!(w.fighters[1].alive, "respawned after the delay");
    assert_eq!(w.fighters[1].health, MAX_HEALTH);
}

#[test]
fn reaching_the_score_limit_ends_the_match() {
    let mut w = world("deathmatch", 1, 0);
    for _ in 0..3 {
        w.fighters[1].spawn_protect = 0.0;
        w.damage(1, Some(0), None, 500.0, false, false);
        run(&mut w, 200);
    }
    assert_eq!(w.phase, Phase::MatchOver { winner: Some(0) });
}

#[test]
fn no_friendly_fire_in_team_modes() {
    let mut w = world("team_deathmatch", 1, 1);
    // Fighter 1 is a friendly bot (team 0).
    assert_eq!(w.fighters[1].team, Some(0));
    w.damage(1, Some(0), None, 50.0, false, false);
    assert_eq!(w.fighters[1].health, MAX_HEALTH);
    w.damage(2, Some(0), None, 50.0, false, false);
    assert_eq!(w.fighters[2].health, MAX_HEALTH - 50.0);
}

#[test]
fn explosion_damage_falls_off_with_distance() {
    let mut w = world("deathmatch", 2, 0);
    let boom = w.data.weapon_index("boom").unwrap();
    place(&mut w, 0, 3.0, "pistol");
    place(&mut w, 1, 10.0, "pistol");
    place(&mut w, 2, 20.0, "pistol");
    // Blast 1 tile from fighter 1's body edge, far from fighter 2.
    let edge = w.fighters[1].aabb().min.x;
    w.explode(vec2(edge - 1.0, 6.0), 0, boom);
    let expected = 80.0 * (1.0 - 1.0 / 4.0);
    assert!((MAX_HEALTH - w.fighters[1].health - expected).abs() < 0.01);
    assert!(w.fighters[1].vel.x > 0.0, "knocked away from the blast");
    assert_eq!(w.fighters[2].health, MAX_HEALTH);
    assert_eq!(w.fighters[0].health, MAX_HEALTH);
}

#[test]
fn own_explosions_hurt_half() {
    let mut w = world("deathmatch", 1, 0);
    let boom = w.data.weapon_index("boom").unwrap();
    place(&mut w, 0, 5.0, "pistol");
    place(&mut w, 1, 25.0, "pistol");
    let c = w.fighters[0].center();
    w.explode(c, 0, boom);
    assert!((w.fighters[0].health - (MAX_HEALTH - 40.0)).abs() < 0.01);
}

#[test]
fn lifesteal_heals_the_attacker() {
    let mut w = world("deathmatch", 1, 0);
    place(&mut w, 0, 5.0, "leech");
    place(&mut w, 1, 15.0, "pistol");
    w.fighters[0].health = 50.0;
    let i = aim_at(&w, 0, vec2(15.0, 6.2));
    w.step(&[i]);
    assert_eq!(w.fighters[1].health, MAX_HEALTH - 20.0);
    assert_eq!(w.fighters[0].health, 60.0);
}

#[test]
fn bullets_travel_and_hit() {
    let mut w = world("deathmatch", 1, 0);
    place(&mut w, 0, 5.0, "pistol");
    place(&mut w, 1, 15.0, "pistol");
    let i = aim_at(&w, 0, vec2(15.0, 6.2));
    w.step(&[i]);
    assert_eq!(w.projectiles.len(), 1);
    assert_eq!(w.fighters[1].health, MAX_HEALTH);
    run(&mut w, 15);
    assert!(w.projectiles.is_empty());
    assert_eq!(w.fighters[1].health, MAX_HEALTH - 10.0);
}

#[test]
fn health_pickup_heals_and_respawns() {
    let mut w = world("deathmatch", 1, 0);
    place(&mut w, 0, 8.5, "pistol");
    place(&mut w, 1, 25.0, "pistol");
    w.fighters[0].health = 30.0;
    run(&mut w, 1);
    assert_eq!(w.fighters[0].health, 80.0);
    assert!(!w.pickups[0].active);
    w.fighters[0].feet.x = 3.0;
    run(&mut w, (10.0 / DT) as usize + 2);
    assert!(w.pickups[0].active);
}

#[test]
fn elimination_rounds_end_when_a_team_is_wiped() {
    let mut w = world("team_elimination", 1, 0);
    run(&mut w, 5);
    w.damage(1, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    assert!(matches!(
        w.phase,
        Phase::RoundOver {
            winner: Some(0),
            ..
        }
    ));
    assert_eq!(w.round_wins, vec![1, 0]);
    run(&mut w, (3.0 / DT) as usize + 2);
    assert_eq!(w.phase, Phase::Playing);
    assert_eq!(w.round, 2);
    assert!(w.fighters.iter().all(|f| f.alive));
    // Second win takes the best-of-3.
    w.fighters[1].spawn_protect = 0.0;
    w.damage(1, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    assert_eq!(w.phase, Phase::MatchOver { winner: Some(0) });
}

#[test]
fn reload_refills_the_magazine() {
    let mut w = world("deathmatch", 1, 0);
    place(&mut w, 0, 5.0, "rail");
    let rail = w.data.weapon_index("rail").unwrap();
    w.fighters[0].inventory[rail].mag = 1;
    w.step(&[Input {
        reload: true,
        ..Default::default()
    }]);
    run(&mut w, 70);
    let s = w.fighters[0].inventory[rail];
    assert_eq!((s.mag, s.reserve), (5, 6));
}

#[test]
fn kill_events_are_emitted() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 2);
    w.damage(1, Some(0), None, 500.0, true, false);
    assert!(w.events.iter().any(|e| matches!(
        e,
        Event::Kill {
            victim: 1,
            killer: Some(0),
            headshot: true,
            ..
        }
    )));
}

#[test]
fn shield_absorbs_part_of_a_hit() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 2);
    w.fighters[1].shield = 100.0;
    w.damage(1, Some(0), None, 40.0, false, false);
    assert!((w.fighters[1].shield - 72.0).abs() < 1e-3);
    assert!((w.fighters[1].health - 88.0).abs() < 1e-3);
}

#[test]
fn shield_station_tops_up_and_goes_on_cooldown() {
    let mut w = world("deathmatch", 1, 0);
    let k = w
        .objects
        .iter()
        .position(|o| o.def.kind == arcane_dome::data::ObjectKind::ShieldStation)
        .unwrap();
    w.objects[k].ready = true;
    w.objects[k].timer = 0.0;
    place(&mut w, 0, 21.5, "pistol");
    place(&mut w, 1, 3.0, "pistol");
    run(&mut w, 1);
    assert_eq!(w.fighters[0].shield, 50.0);
    assert!(!w.objects[k].ready);
    assert!(w.objects[k].timer >= 59.0);
    assert!(w.fighters[0].shield <= MAX_SHIELD);
}

#[test]
fn thrusters_launch_high() {
    let mut w = world("deathmatch", 1, 0);
    place(&mut w, 0, 12.5, "pistol");
    place(&mut w, 1, 3.0, "pistol");
    // Standing on the pad does nothing by itself.
    run(&mut w, 30);
    assert!(w.fighters[0].on_ground, "pad launched without a jump");
    assert!((w.fighters[0].feet.y - 7.0).abs() < 0.01);

    let mut jump = idle(2);
    jump[0].jump = true;
    jump[0].jump_held = true;
    w.step(&jump);
    let mut top = 7.0f32;
    for _ in 0..90 {
        w.step(&idle(2));
        top = top.min(w.fighters[0].feet.y);
    }
    // 4 tiles requested; the room is only 6 tall so the ceiling may cap it.
    assert!(top < 3.6, "launched to {top}");
}

#[test]
fn powerups_change_damage_fire_rate_and_invulnerability() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 2);
    let id = |w: &World, name: &str| {
        w.data
            .powerups
            .powerups
            .iter()
            .position(|p| p.id == name)
            .unwrap()
    };
    let crit = id(&w, "critical_strike");
    w.apply_powerup(0, crit);
    w.damage(1, Some(0), None, 10.0, false, false);
    assert_eq!(w.fighters[1].health, MAX_HEALTH - 20.0);
    assert_eq!(w.fire_rate_mult(0), 1.0);
    let rapid = id(&w, "rapid_fire");
    w.apply_powerup(0, rapid);
    assert_eq!(w.fire_rate_mult(0), 2.0);
    let shield = id(&w, "divine_shield");
    w.apply_powerup(1, shield);
    w.damage(1, Some(0), None, 50.0, false, false);
    assert_eq!(w.fighters[1].health, MAX_HEALTH - 20.0);
    run(&mut w, (20.0 / DT) as usize + 2);
    assert!(w.fighters[1].buffs.is_empty(), "buffs expire after 20 s");
}

#[test]
fn shadowstep_teleports_once() {
    let mut w = world("deathmatch", 1, 0);
    run(&mut w, 2);
    let p = w
        .data
        .powerups
        .powerups
        .iter()
        .position(|p| p.id == "shadowstep")
        .unwrap();
    w.apply_powerup(0, p);
    assert_eq!(w.fighters[0].teleport_charges, 1);
    let before = w.fighters[0].feet;
    w.step(&[Input {
        ability: true,
        ..Default::default()
    }]);
    assert_eq!(w.fighters[0].teleport_charges, 0);
    assert!(w.fighters[0].feet.distance(before) > 3.0);
    assert!(w.events.iter().any(|e| matches!(e, Event::Teleport { .. })));
}

#[test]
fn powerup_spot_appears_after_the_first_delay() {
    let mut w = world("deathmatch", 1, 0);
    let k = w
        .objects
        .iter()
        .position(|o| o.def.kind == arcane_dome::data::ObjectKind::PowerupSpot)
        .unwrap();
    assert!(!w.objects[k].ready);
    run(&mut w, (1.0 / DT) as usize + 2);
    assert!(w.objects[k].ready);
}

/// A team match with `bots[t]` bots on team `t` (the player is on team 0).
fn teams_world(mode: &str, bots: &[u32]) -> World {
    let mut team_bots = [0; 4];
    team_bots[..bots.len()].copy_from_slice(bots);
    let (mode, teams) = mode_and_teams(mode, bots.len() as u8);
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: mode,
        teams,
        team_bots,
        countdown_s: 0.0,
        ..Default::default()
    };
    let mut w = World::new(data(), cfg).unwrap();
    for f in &mut w.fighters {
        f.spawn_protect = 0.0;
    }
    w
}

#[test]
fn four_teams_are_built_with_their_own_sides_and_names() {
    let w = teams_world("team_deathmatch", &[1, 2, 2, 2]);
    assert_eq!(w.fighters.len(), 8);
    assert_eq!(w.num_sides(), 4);
    let per_team: Vec<usize> = (0..4u8)
        .map(|t| w.fighters.iter().filter(|f| f.team == Some(t)).count())
        .collect();
    assert_eq!(per_team, vec![2, 2, 2, 2]);
    let names: Vec<String> = (0..4).map(|t| w.side_name(t)).collect();
    assert_eq!(names, vec!["Blue", "Red", "Green", "Gold"]);
    // Allies only within a team.
    let on = |t: u8| w.fighters.iter().position(|f| f.team == Some(t)).unwrap();
    let (blue, green, gold) = (on(0), on(2), on(3));
    let green2 = w.fighters.iter().rposition(|f| f.team == Some(2)).unwrap();
    assert!(w.allies(green, green2));
    assert!(!w.allies(blue, green));
    assert!(!w.allies(green, gold));
}

#[test]
fn team_count_is_capped_by_the_mode_and_needs_two_manned_teams() {
    // Asking for more teams than exist is capped at four.
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: "deathmatch".into(),
        teams: 9,
        team_bots: [1, 1, 1, 1],
        countdown_s: 0.0,
        ..Default::default()
    };
    assert_eq!(World::new(data(), cfg).unwrap().num_sides(), 4);
    // Teams off: free-for-all, whatever the team bot counts say.
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: "deathmatch".into(),
        teams: 0,
        bots: 3,
        countdown_s: 0.0,
        ..Default::default()
    };
    let w = World::new(data(), cfg).unwrap();
    assert!(w.fighters.iter().all(|f| f.team.is_none()));
    // Bots only on one team and no player: not a match.
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: "deathmatch".into(),
        teams: 3,
        team_bots: [0, 3, 0, 0],
        include_player: false,
        countdown_s: 0.0,
        ..Default::default()
    };
    assert!(World::new(data(), cfg).is_err());
}

#[test]
fn three_team_elimination_waits_for_the_last_team_standing() {
    let mut w = teams_world("team_elimination", &[0, 1, 1]);
    run(&mut w, 5);
    let red = w.fighters.iter().position(|f| f.team == Some(1)).unwrap();
    let green = w.fighters.iter().position(|f| f.team == Some(2)).unwrap();
    w.damage(red, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    // Two teams still stand: the round goes on.
    assert_eq!(w.phase, Phase::Playing);
    w.damage(green, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    assert!(matches!(
        w.phase,
        Phase::RoundOver {
            winner: Some(0),
            ..
        }
    ));
    assert_eq!(w.round_wins, vec![1, 0, 0]);
}

#[test]
fn any_of_four_teams_can_win_on_score() {
    let mut w = teams_world("team_deathmatch", &[0, 1, 1, 1]);
    run(&mut w, 5);
    let gold = w.fighters.iter().position(|f| f.team == Some(3)).unwrap();
    let red = w.fighters.iter().position(|f| f.team == Some(1)).unwrap();
    // Gold kills Red five times (the score limit in the test modes).
    for _ in 0..5 {
        w.fighters[red].spawn_protect = 0.0;
        w.damage(red, Some(gold), None, 500.0, false, false);
        run(&mut w, (5.0 / DT) as usize);
        if matches!(w.phase, Phase::MatchOver { .. }) {
            break;
        }
    }
    assert_eq!(w.scores[3], 5);
    assert_eq!(w.phase, Phase::MatchOver { winner: Some(3) });
}

#[test]
fn teams_without_spawns_use_free_spawns_near_each_other() {
    // The arena only has spawns for teams 0 and 1 (ends of the room) and
    // free spawns at x 5, 10, 19 and 24.
    for seed in 0..20 {
        let cfg = MatchConfig {
            map_id: "arena".into(),
            mode_id: "elimination".into(),
            teams: 4,
            team_bots: [1, 2, 2, 2],
            seed,
            countdown_s: 0.0,
            ..Default::default()
        };
        let w = World::new(data(), cfg).unwrap();
        for t in 2..4u8 {
            let xs: Vec<f32> = w
                .fighters
                .iter()
                .filter(|f| f.team == Some(t))
                .map(|f| f.feet.x)
                .collect();
            for &x in &xs {
                // Never in the Blue or Red corners.
                assert!(
                    x > 4.0 && x < 25.0,
                    "seed {seed}: team {t} spawned at x {x}"
                );
            }
            assert!(
                (xs[0] - xs[1]).abs() < 10.0,
                "seed {seed}: team {t} split up: {xs:?}"
            );
        }
    }
}

/// A world with the real pre-match countdown.
fn counted_world(mode: &str) -> World {
    let (mode, teams) = mode_and_teams(mode, 2);
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: mode,
        bots: 1,
        teams,
        team_bots: [0, 1, 0, 0],
        ..Default::default()
    };
    World::new(data(), cfg).unwrap()
}

#[test]
fn countdown_holds_everyone_until_go() {
    let mut w = counted_world("deathmatch");
    assert_eq!(w.countdown, 5.0);
    let clock = w.time_left;
    let protect = w.fighters[0].spawn_protect;
    run(&mut w, 30);
    let start = w.fighters[0].feet;
    // Trying to run and shoot during the countdown does nothing.
    let mut go = idle(2);
    go[0].move_x = 1.0;
    go[0].fire = true;
    for _ in 0..(4.0 / DT) as usize {
        w.step(&go);
    }
    assert!(w.countdown > 0.0);
    assert!(
        (w.fighters[0].feet.x - start.x).abs() < 1e-3,
        "moved during countdown"
    );
    assert!(w.projectiles.is_empty());
    assert_eq!(w.time_left, clock, "the clock ran during the countdown");
    assert_eq!(
        w.fighters[0].spawn_protect, protect,
        "spawn protection wore off"
    );
    // After GO the same input moves the fighter and the clock runs.
    for _ in 0..(1.5 / DT) as usize {
        w.step(&go);
    }
    assert!(w.countdown <= 0.0);
    assert!(
        w.fighters[0].feet.x > start.x + 1.0,
        "still frozen after GO"
    );
    assert!(w.time_left < clock);
}

#[test]
fn every_elimination_round_starts_with_a_countdown() {
    let mut w = counted_world("team_elimination");
    run(&mut w, (5.5 / DT) as usize);
    assert!(w.countdown <= 0.0);
    let red = w.fighters.iter().position(|f| f.team == Some(1)).unwrap();
    w.fighters[red].spawn_protect = 0.0;
    w.damage(red, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    assert!(matches!(w.phase, Phase::RoundOver { .. }));
    run(&mut w, (3.0 / DT) as usize + 2);
    assert_eq!(w.round, 2);
    assert!(
        w.countdown > 4.5,
        "round 2 did not count down: {}",
        w.countdown
    );
}

#[test]
fn free_for_all_elimination_goes_to_the_last_one_standing() {
    let mut w = world("elimination", 2, 0);
    assert!(!w.mode.is_team());
    assert_eq!(w.mode.match_name(), "Elimination");
    assert_eq!(w.num_sides(), 3);
    run(&mut w, 5);
    w.damage(1, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    assert_eq!(w.phase, Phase::Playing, "two still standing");
    w.damage(2, Some(0), None, 500.0, false, false);
    run(&mut w, 1);
    assert!(matches!(
        w.phase,
        Phase::RoundOver {
            winner: Some(0),
            ..
        }
    ));
    assert_eq!(w.round_wins, vec![1, 0, 0]);
}

#[test]
fn teams_switch_the_mode_name_and_score_limit() {
    let solo = world("deathmatch", 1, 0);
    assert_eq!(solo.mode.match_name(), "Deathmatch");
    assert_eq!(solo.mode.score_limit, Some(3));
    let team = world("team_deathmatch", 1, 1);
    assert_eq!(team.mode.match_name(), "Team Deathmatch");
    assert_eq!(team.mode.score_limit, Some(5), "team_score_limit not used");
}
