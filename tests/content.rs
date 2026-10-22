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
    assert_eq!(data.weapons.len(), 10);
    assert!(!data.maps.is_empty(), "at least one map");
    assert_eq!(data.modes.len(), 3);
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
