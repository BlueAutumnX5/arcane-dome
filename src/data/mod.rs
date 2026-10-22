//! Loading and validating the JSON game data in `assets/data`.
//!
//! Nothing here knows about rendering or simulation. Every loader reports
//! errors as `<file>: <field>: <problem>` so content authors can fix them fast.

mod error;
mod map;
mod modes;
mod weapon;

pub use error::{DataError, DataErrors};
pub use map::{BotHint, MapDef, PickupDef, PickupKind, SpawnDef, TileKind, parse_map};
pub use modes::{ModeDef, parse_modes};
pub use weapon::{
    Category, ExplosionDef, Falloff, ProjectileDef, ProjectileKind, Special, WeaponDef,
    parse_weapon,
};

use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// All content the game needs, loaded and cross-checked.
#[derive(Debug, Clone)]
pub struct GameData {
    /// Sorted into weapon-bar order (by category, then id). Index = slot.
    pub weapons: Vec<WeaponDef>,
    pub maps: Vec<MapDef>,
    pub modes: Vec<ModeDef>,
}

impl GameData {
    /// Load everything under `<assets>/data`. Returns every error found, not just the first.
    pub fn load(assets: &Path) -> Result<Self, DataErrors> {
        let data_dir = assets.join("data");
        let mut errors = Vec::new();

        let modes = match read_file(&data_dir, "modes.json") {
            Ok(text) => parse_modes("modes.json", &text).unwrap_or_else(|e| {
                errors.extend(e);
                Vec::new()
            }),
            Err(e) => {
                errors.push(e);
                Vec::new()
            }
        };

        let weapons = load_dir(&data_dir, "weapons", &mut errors, |label, stem, text| {
            parse_weapon(label, stem, text)
        });
        let weapon_ids: Vec<&str> = weapons.iter().map(|w| w.id.as_str()).collect();
        let mode_ids: Vec<&str> = modes.iter().map(|m| m.id.as_str()).collect();

        let maps = load_dir(&data_dir, "maps", &mut errors, |label, stem, text| {
            parse_map(label, stem, text, &weapon_ids, &mode_ids)
        });

        if errors.is_empty() {
            Self::from_parts(weapons, maps, modes)
        } else {
            Err(DataErrors(errors))
        }
    }

    /// Build from already-parsed parts (used by `load` and by tests).
    pub fn from_parts(
        mut weapons: Vec<WeaponDef>,
        mut maps: Vec<MapDef>,
        modes: Vec<ModeDef>,
    ) -> Result<Self, DataErrors> {
        let mut errors = Vec::new();
        if weapons.is_empty() {
            errors.push(DataError::new(
                "weapons/",
                "folder contains no weapon files",
            ));
        } else if !weapons.iter().any(|w| w.category == Category::Starter) {
            errors.push(DataError::new(
                "weapons/",
                "at least one weapon must have category \"starter\" (spawn loadout)",
            ));
        }
        if maps.is_empty() {
            errors.push(DataError::new("maps/", "folder contains no map files"));
        }
        if modes.is_empty() {
            errors.push(DataError::new("modes.json", "defines no modes"));
        }
        if !errors.is_empty() {
            return Err(DataErrors(errors));
        }
        weapons.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.id.cmp(&b.id)));
        maps.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self {
            weapons,
            maps,
            modes,
        })
    }

    pub fn weapon_index(&self, id: &str) -> Option<usize> {
        self.weapons.iter().position(|w| w.id == id)
    }

    pub fn map(&self, id: &str) -> Option<&MapDef> {
        self.maps.iter().find(|m| m.id == id)
    }

    pub fn mode(&self, id: &str) -> Option<&ModeDef> {
        self.modes.iter().find(|m| m.id == id)
    }
}

/// Find the `assets` folder: `$ARCANE_DOME_ASSETS`, the working directory,
/// next to the executable, or the crate root (for `cargo run`).
pub fn find_assets_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(p) = std::env::var("ARCANE_DOME_ASSETS") {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(PathBuf::from("assets"));
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("assets"));
        candidates.push(dir.join("../../assets"));
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
    candidates
        .into_iter()
        .find(|p| p.join("data").join("modes.json").is_file())
}

fn read_file(data_dir: &Path, label: &str) -> Result<String, DataError> {
    std::fs::read_to_string(data_dir.join(label))
        .map_err(|e| DataError::new(label, format!("cannot read file: {e}")))
}

fn load_dir<T>(
    data_dir: &Path,
    sub: &str,
    errors: &mut Vec<DataError>,
    parse: impl Fn(&str, &str, &str) -> Result<T, Vec<DataError>>,
) -> Vec<T> {
    let dir = data_dir.join(sub);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(DataError::new(
                format!("{sub}/"),
                format!("cannot read folder: {e}"),
            ));
            return Vec::new();
        }
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();

    let mut out = Vec::new();
    for path in files {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let label = format!("{sub}/{file_name}");
        match std::fs::read_to_string(&path) {
            Ok(text) => match parse(&label, &stem, &text) {
                Ok(item) => out.push(item),
                Err(e) => errors.extend(e),
            },
            Err(e) => errors.push(DataError::new(&label, format!("cannot read file: {e}"))),
        }
    }
    out
}

/// Deserialize JSON, reporting the exact field path on failure
/// (e.g. `projectile.type: unknown variant "laser"`).
pub(crate) fn parse_json<T: DeserializeOwned>(label: &str, text: &str) -> Result<T, DataError> {
    let de = &mut serde_json::Deserializer::from_str(text);
    serde_path_to_error::deserialize(de).map_err(|e| {
        let path = e.path().to_string();
        let inner = e.into_inner();
        if path.is_empty() || path == "." {
            DataError::new(label, inner.to_string())
        } else {
            DataError::new(label, format!("{path}: {inner}"))
        }
    })
}

/// `snake_case` identifiers: lowercase letters, digits and underscores.
pub(crate) fn is_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !s.starts_with('_')
}
