//! Aim assist (the touch fire button): who it locks on to and where it aims.

use arcane_dome::ai::assist::AutoAim;
use arcane_dome::data::{GameData, parse_map};
use arcane_dome::sim::{MatchConfig, World};
use macroquad::math::vec2;
use std::path::Path;
use std::sync::Arc;

/// A flat 60 x 30 field with a wall from x 14 to 15 (y 14 to 26).
fn field() -> Arc<GameData> {
    let shipped = GameData::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap();
    let text = r#"{"id": "field", "name": "Field", "size": [60, 30], "modes": ["deathmatch"],
        "team_counts": [2],
        "shapes": [
            {"kind": "solid", "material": "rock", "points": [[0, 26], [60, 26], [60, 30], [0, 30]]},
            {"kind": "solid", "material": "rock", "points": [[14, 14], [15, 14], [15, 26], [14, 26]]}
        ],
        "spawns": [{"x": 20, "y": 26}, {"x": 30, "y": 26}, {"x": 40, "y": 26}, {"x": 50, "y": 26},
                   {"x": 20, "y": 26, "team": 0}, {"x": 24, "y": 26, "team": 0},
                   {"x": 40, "y": 26, "team": 1}, {"x": 44, "y": 26, "team": 1}]}"#;
    let weapons: Vec<&str> = shipped.weapons.iter().map(|w| w.id.as_str()).collect();
    let modes: Vec<&str> = shipped.modes.iter().map(|m| m.id.as_str()).collect();
    let map = parse_map("field.json", "field", text, &weapons, &modes).unwrap();
    Arc::new(
        GameData::from_parts(shipped.weapons.clone(), vec![map], shipped.modes.clone()).unwrap(),
    )
}

fn world(cfg: MatchConfig) -> World {
    World::new(
        field(),
        MatchConfig {
            map_id: "field".into(),
            countdown_s: 0.0,
            powerups: false,
            ..cfg
        },
    )
    .unwrap()
}

fn place(w: &mut World, at: &[f32]) {
    for (i, &x) in at.iter().enumerate() {
        w.fighters[i].feet = vec2(x, 26.0);
        w.fighters[i].vel = vec2(0.0, 0.0);
    }
}

#[test]
fn locks_on_to_the_nearest_enemy_in_sight_and_holds_the_lock() {
    let mut w = world(MatchConfig {
        bots: 3,
        ..Default::default()
    });
    w.fighters[0].current = w.data.weapon_index("peacekeeper").unwrap();
    // Behind the wall (closer, unseen), in the open, and far off.
    place(&mut w, &[20.0, 12.0, 30.0, 50.0]);
    let mut aim = AutoAim::default();
    let at = aim.update(&w, 0).expect("a target");
    assert_eq!(aim.target, Some(2));
    let body = w.fighters[2].aabb().center();
    // A fast bullet: straight at the body, give or take a little drop.
    assert!((at - body).length() < 1.0, "aims at {at}, body at {body}");

    // A slightly closer one does not steal the lock...
    place(&mut w, &[20.0, 12.0, 30.0, 27.5]);
    aim.update(&w, 0);
    assert_eq!(aim.target, Some(2));
    // ...a much closer one does.
    place(&mut w, &[20.0, 12.0, 30.0, 24.0]);
    aim.update(&w, 0);
    assert_eq!(aim.target, Some(3));

    // Nobody in sight: no target.
    place(&mut w, &[20.0, 12.0, 11.0, 10.0]);
    assert_eq!(aim.update(&w, 0), None);
    assert_eq!(aim.target, None);
}

#[test]
fn teammates_are_never_targets() {
    let mut w = world(MatchConfig {
        teams: 2,
        team_bots: [1, 1, 0, 0],
        ..Default::default()
    });
    let ally = (1..w.fighters.len()).find(|&i| w.allies(0, i)).unwrap();
    let enemy = (1..w.fighters.len()).find(|&i| !w.allies(0, i)).unwrap();
    w.fighters[0].feet = vec2(20.0, 26.0);
    w.fighters[ally].feet = vec2(23.0, 26.0);
    w.fighters[enemy].feet = vec2(32.0, 26.0);
    let mut aim = AutoAim::default();
    aim.update(&w, 0);
    assert_eq!(aim.target, Some(enemy));
}

#[test]
fn lobbed_shots_aim_above_the_target() {
    let mut w = world(MatchConfig {
        bots: 1,
        ..Default::default()
    });
    place(&mut w, &[20.0, 32.0]);
    let gl = w.data.weapon_index("thumper").unwrap();
    w.fighters[0].current = gl;
    let at = AutoAim::default().update(&w, 0).unwrap();
    let body = w.fighters[1].aabb().center();
    assert!(at.y < body.y - 1.0, "aims at {at}, body at {body}");
}
