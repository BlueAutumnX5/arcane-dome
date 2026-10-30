//! The shipped content in `assets/data` must always load cleanly.

use arcane_dome::data::{GameData, ProjectileKind};
use std::collections::HashSet;
use std::path::Path;

fn shipped() -> GameData {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    GameData::load(&assets).unwrap_or_else(|e| panic!("shipped data is invalid:\n{e}"))
}

#[test]
fn shipped_content_loads() {
    let data = shipped();
    assert_eq!(data.weapons.len(), 11);
    assert!(!data.maps.is_empty(), "at least one map");
    // In the order Quick Match offers them.
    let ids: Vec<&str> = data.modes.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "deathmatch",
            "elimination",
            "gun_game",
            "juggernaut",
            "king_of_the_hill",
            "domination",
            "territories",
            "capture_the_flag"
        ]
    );
    for m in &data.modes {
        let teams = match m.id.as_str() {
            "juggernaut" => 1,
            "territories" => 2,
            _ => 4,
        };
        assert_eq!(m.teams, teams, "{}", m.id);
    }
}

#[test]
fn every_map_offers_every_objective_mode() {
    let data = shipped();
    for map in &data.maps {
        for mode in &data.modes {
            assert!(map.modes.contains(&mode.id), "{} lacks {}", map.id, mode.id);
            assert_eq!(mode.map_problem(map), None, "{} / {}", map.id, mode.id);
        }
        assert!(map.zones.len() >= 3, "{}: zones for Territories", map.id);
    }
}

#[test]
fn gun_game_climbs_every_weapon_and_ends_on_melee() {
    let data = shipped();
    let gg = data.mode("gun_game").unwrap();
    let mut ladder: Vec<&str> = gg.ladder.iter().map(String::as_str).collect();
    let last = data.weapon_index(ladder[ladder.len() - 1]).unwrap();
    assert_eq!(data.weapons[last].projectile.kind, ProjectileKind::MeleeArc);
    ladder.sort();
    let mut all: Vec<&str> = data.weapons.iter().map(|w| w.id.as_str()).collect();
    all.sort();
    assert_eq!(ladder, all, "each weapon once");
    // The baton is only Gun Game's: never a random handout.
    assert!(!data.weapons[last].is_loot());
}

#[test]
fn weapons_cover_at_least_five_projectile_types() {
    let data = shipped();
    let kinds: HashSet<ProjectileKind> = data.weapons.iter().map(|w| w.projectile.kind).collect();
    for k in [
        ProjectileKind::Hitscan,
        ProjectileKind::Bullet,
        ProjectileKind::Grenade,
        ProjectileKind::Rocket,
        ProjectileKind::Beam,
    ] {
        assert!(kinds.contains(&k), "missing {k:?}");
    }
}

#[test]
fn every_map_supports_every_mode() {
    let data = shipped();
    for m in &data.maps {
        for mode in &data.modes {
            assert!(m.modes.contains(&mode.id), "{} lacks {}", m.id, mode.id);
        }
    }
}
