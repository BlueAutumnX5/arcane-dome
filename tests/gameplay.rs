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
  "deathmatch":       { "teams": 1, "respawn": true,  "score_limit": 3, "time_limit_s": 600 },
  "team_deathmatch":  { "teams": 2, "respawn": true,  "score_limit": 5, "time_limit_s": 600 },
  "team_elimination": { "teams": 2, "respawn": false, "rounds": 3,      "time_limit_s": 60 }
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
        "modes": ["deathmatch", "team_deathmatch", "team_elimination"],
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

fn world(mode: &str, enemies: u32, friends: u32) -> World {
    let cfg = MatchConfig {
        map_id: "arena".into(),
        mode_id: mode.into(),
        enemy_bots: enemies,
        friendly_bots: friends,
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
