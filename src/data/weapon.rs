use super::{DataError, is_snake_case, parse_json};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Weapon-bar grouping. Declaration order is the slot order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Starter,
    Sidearm,
    CloseRange,
    Automatic,
    LongRange,
    Heavy,
    Explosive,
    Melee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileKind {
    Hitscan,
    Bullet,
    Grenade,
    Rocket,
    Beam,
    MeleeArc,
}

impl ProjectileKind {
    /// True for kinds that become moving projectiles in the world.
    pub fn is_physical(self) -> bool {
        matches!(self, Self::Bullet | Self::Grenade | Self::Rocket)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Falloff {
    #[default]
    Linear,
    None,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplosionDef {
    pub radius_tiles: f32,
    pub damage: f32,
    #[serde(default)]
    pub falloff: Falloff,
    #[serde(default)]
    pub knockback: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileDef {
    #[serde(rename = "type")]
    pub kind: ProjectileKind,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub gravity: f32,
    #[serde(default = "default_lifetime")]
    pub lifetime_s: f32,
    #[serde(default)]
    pub bounces: u32,
    #[serde(default)]
    pub homing_strength: f32,
    #[serde(default)]
    pub pierce: u32,
    #[serde(default)]
    pub explosion: Option<ExplosionDef>,
}

fn default_lifetime() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SpecialKind {
    Lifesteal,
    Burn,
    Multishot,
    CreditsBonus,
    Crit,
    Stun,
    ShieldBreak,
    Freeze,
}

/// On-hit or on-fire modifiers. Written in JSON either as a bare name
/// (`"burn"`) or as an object with parameters (`{ "type": "lifesteal", "ratio": 0.25 }`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(from = "RawSpecial")]
pub enum Special {
    Lifesteal {
        ratio: f32,
    },
    Burn {
        dps: f32,
        duration_s: f32,
    },
    Multishot {
        extra: u32,
    },
    /// Reserved for the credits economy (not in Milestone 1).
    CreditsBonus {
        mult: f32,
    },
    Crit {
        chance: f32,
        mult: f32,
    },
    Stun {
        duration_s: f32,
    },
    /// Reserved for armor abilities (not in Milestone 1).
    ShieldBreak,
    Freeze {
        duration_s: f32,
        slow: f32,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawSpecial {
    Name(SpecialKind),
    Params(SpecialParams),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecialParams {
    #[serde(rename = "type")]
    kind: SpecialKind,
    ratio: Option<f32>,
    dps: Option<f32>,
    duration_s: Option<f32>,
    extra: Option<u32>,
    chance: Option<f32>,
    mult: Option<f32>,
    slow: Option<f32>,
}

impl From<RawSpecial> for Special {
    fn from(raw: RawSpecial) -> Self {
        let p = match raw {
            RawSpecial::Name(kind) => SpecialParams {
                kind,
                ratio: None,
                dps: None,
                duration_s: None,
                extra: None,
                chance: None,
                mult: None,
                slow: None,
            },
            RawSpecial::Params(p) => p,
        };
        match p.kind {
            SpecialKind::Lifesteal => Special::Lifesteal {
                ratio: p.ratio.unwrap_or(0.25),
            },
            SpecialKind::Burn => Special::Burn {
                dps: p.dps.unwrap_or(8.0),
                duration_s: p.duration_s.unwrap_or(3.0),
            },
            SpecialKind::Multishot => Special::Multishot {
                extra: p.extra.unwrap_or(1),
            },
            SpecialKind::CreditsBonus => Special::CreditsBonus {
                mult: p.mult.unwrap_or(1.5),
            },
            SpecialKind::Crit => Special::Crit {
                chance: p.chance.unwrap_or(0.1),
                mult: p.mult.unwrap_or(2.0),
            },
            SpecialKind::Stun => Special::Stun {
                duration_s: p.duration_s.unwrap_or(0.4),
            },
            SpecialKind::ShieldBreak => Special::ShieldBreak,
            SpecialKind::Freeze => Special::Freeze {
                duration_s: p.duration_s.unwrap_or(1.5),
                slow: p.slow.unwrap_or(0.5),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponDef {
    pub id: String,
    pub name: String,
    pub category: Category,
    #[serde(default)]
    pub unlock_cost: u32,
    pub damage: f32,
    #[serde(default = "one")]
    pub headshot_mult: f32,
    pub fire_rate_rps: f32,
    #[serde(default = "one_u32")]
    pub pellets: u32,
    #[serde(default)]
    pub spread_deg: f32,
    #[serde(default)]
    pub recoil: f32,
    /// Rounds per magazine. 0 means the weapon never uses ammo (melee).
    pub magazine: u32,
    pub reserve_ammo: u32,
    #[serde(default)]
    pub reload_time_s: f32,
    pub range_tiles: f32,
    pub projectile: ProjectileDef,
    #[serde(default)]
    pub specials: Vec<Special>,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub sounds: BTreeMap<String, String>,
}

fn one() -> f32 {
    1.0
}

fn one_u32() -> u32 {
    1
}

impl WeaponDef {
    pub fn uses_ammo(&self) -> bool {
        self.magazine > 0
    }

    /// Starter weapons never run dry, so nobody is ever left unarmed.
    pub fn infinite_reserve(&self) -> bool {
        self.category == Category::Starter
    }

    pub fn total_pellets(&self) -> u32 {
        let extra: u32 = self
            .specials
            .iter()
            .map(|s| match s {
                Special::Multishot { extra } => *extra,
                _ => 0,
            })
            .sum();
        self.pellets + extra
    }

    /// Short label for the weapon bar.
    pub fn short_name(&self) -> String {
        self.name
            .split_whitespace()
            .next()
            .unwrap_or(&self.name)
            .to_owned()
    }
}

/// Parse and validate one weapon file. `stem` is the file name without `.json`.
pub fn parse_weapon(label: &str, stem: &str, text: &str) -> Result<WeaponDef, Vec<DataError>> {
    let def: WeaponDef = parse_json(label, text).map_err(|e| vec![e])?;
    let problems = validate(&def, stem);
    if problems.is_empty() {
        Ok(def)
    } else {
        Err(problems
            .into_iter()
            .map(|m| DataError::new(label, m))
            .collect())
    }
}

fn validate(w: &WeaponDef, stem: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |ok: bool, msg: String| {
        if !ok {
            out.push(msg);
        }
    };

    check(
        is_snake_case(&w.id),
        format!("id: \"{}\" must be snake_case", w.id),
    );
    check(
        w.id == stem,
        format!("id: \"{}\" must match the file name \"{stem}\"", w.id),
    );
    check(!w.name.trim().is_empty(), "name: must not be empty".into());
    check(
        w.damage >= 0.0,
        format!("damage: must be 0 or more (got {})", w.damage),
    );
    check(
        w.headshot_mult >= 1.0,
        format!(
            "headshot_mult: must be 1.0 or more (got {})",
            w.headshot_mult
        ),
    );
    check(
        w.fire_rate_rps > 0.0,
        format!(
            "fire_rate_rps: must be greater than 0 (got {})",
            w.fire_rate_rps
        ),
    );
    check(w.pellets >= 1, "pellets: must be at least 1".into());
    check(
        (0.0..=180.0).contains(&w.spread_deg),
        format!(
            "spread_deg: must be between 0 and 180 (got {})",
            w.spread_deg
        ),
    );
    check(
        w.recoil >= 0.0,
        format!("recoil: must be 0 or more (got {})", w.recoil),
    );
    let melee = w.category == Category::Melee || w.projectile.kind == ProjectileKind::MeleeArc;
    check(
        w.magazine > 0 || melee,
        "magazine: must be greater than 0 for non-melee weapons".into(),
    );
    check(
        w.reload_time_s >= 0.0,
        format!("reload_time_s: must be 0 or more (got {})", w.reload_time_s),
    );
    check(
        w.range_tiles > 0.0,
        format!(
            "range_tiles: must be greater than 0 (got {})",
            w.range_tiles
        ),
    );

    let p = &w.projectile;
    if p.kind.is_physical() {
        check(
            p.speed > 0.0,
            format!(
                "projectile.speed: must be greater than 0 for {:?} (got {})",
                p.kind, p.speed
            ),
        );
        check(
            p.lifetime_s > 0.0,
            format!(
                "projectile.lifetime_s: must be greater than 0 (got {})",
                p.lifetime_s
            ),
        );
    }
    if matches!(p.kind, ProjectileKind::Grenade | ProjectileKind::Rocket) {
        check(
            p.explosion.is_some(),
            format!(
                "projectile.explosion: required for {:?} projectiles",
                p.kind
            ),
        );
    }
    check(
        p.homing_strength >= 0.0,
        "projectile.homing_strength: must be 0 or more".into(),
    );
    if let Some(e) = &p.explosion {
        check(
            e.radius_tiles > 0.0,
            format!(
                "projectile.explosion.radius_tiles: must be greater than 0 (got {})",
                e.radius_tiles
            ),
        );
        check(
            e.damage >= 0.0,
            "projectile.explosion.damage: must be 0 or more".into(),
        );
    }

    for (i, s) in w.specials.iter().enumerate() {
        match *s {
            Special::Lifesteal { ratio } => check(
                (0.0..=1.0).contains(&ratio),
                format!("specials[{i}].ratio: must be between 0 and 1 (got {ratio})"),
            ),
            Special::Crit { chance, mult } => {
                check(
                    (0.0..=1.0).contains(&chance),
                    format!("specials[{i}].chance: must be between 0 and 1 (got {chance})"),
                );
                check(
                    mult >= 1.0,
                    format!("specials[{i}].mult: must be 1.0 or more (got {mult})"),
                );
            }
            Special::Burn { dps, duration_s } => check(
                dps >= 0.0 && duration_s >= 0.0,
                format!("specials[{i}]: dps and duration_s must be 0 or more"),
            ),
            Special::Freeze { slow, .. } => check(
                (0.0..=1.0).contains(&slow),
                format!("specials[{i}].slow: must be between 0 and 1 (got {slow})"),
            ),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "id": "arc_rifle", "name": "Arc Rifle", "category": "automatic",
        "damage": 9.0, "headshot_mult": 2.0, "fire_rate_rps": 8.0,
        "spread_deg": 2.5, "magazine": 30, "reserve_ammo": 150,
        "reload_time_s": 1.6, "range_tiles": 28,
        "projectile": { "type": "bullet", "speed": 90.0, "lifetime_s": 1.2 },
        "specials": ["burn", { "type": "lifesteal", "ratio": 0.5 }]
    }"#;

    #[test]
    fn parses_valid_weapon() {
        let w = parse_weapon("weapons/arc_rifle.json", "arc_rifle", VALID).unwrap();
        assert_eq!(w.category, Category::Automatic);
        assert_eq!(w.projectile.kind, ProjectileKind::Bullet);
        assert_eq!(w.pellets, 1);
        assert_eq!(
            w.specials,
            vec![
                Special::Burn {
                    dps: 8.0,
                    duration_s: 3.0
                },
                Special::Lifesteal { ratio: 0.5 }
            ]
        );
    }

    #[test]
    fn bad_enum_names_file_and_field() {
        let text = VALID.replace("\"bullet\"", "\"laser\"");
        let err = parse_weapon("weapons/arc_rifle.json", "arc_rifle", &text).unwrap_err();
        let msg = err[0].to_string();
        assert!(
            msg.starts_with("weapons/arc_rifle.json: projectile.type"),
            "{msg}"
        );
        assert!(msg.contains("laser"), "{msg}");
    }

    #[test]
    fn missing_field_is_reported() {
        let text = VALID.replace("\"fire_rate_rps\": 8.0,", "");
        let err = parse_weapon("weapons/arc_rifle.json", "arc_rifle", &text).unwrap_err();
        assert!(err[0].to_string().contains("fire_rate_rps"), "{}", err[0]);
    }

    #[test]
    fn unknown_field_is_rejected() {
        let text = VALID.replace("\"damage\"", "\"dammage\": 1, \"damage\"");
        let err = parse_weapon("weapons/arc_rifle.json", "arc_rifle", &text).unwrap_err();
        assert!(err[0].to_string().contains("dammage"), "{}", err[0]);
    }

    #[test]
    fn id_must_match_file_name() {
        let err = parse_weapon("weapons/other.json", "other", VALID).unwrap_err();
        assert_eq!(
            err[0].to_string(),
            "weapons/other.json: id: \"arc_rifle\" must match the file name \"other\""
        );
    }

    #[test]
    fn semantic_errors_are_all_collected() {
        let text = VALID
            .replace("\"fire_rate_rps\": 8.0", "\"fire_rate_rps\": 0")
            .replace("\"magazine\": 30", "\"magazine\": 0");
        let err = parse_weapon("weapons/arc_rifle.json", "arc_rifle", &text).unwrap_err();
        assert_eq!(err.len(), 2, "{err:?}");
        assert!(err[0].message.starts_with("fire_rate_rps"));
        assert!(err[1].message.starts_with("magazine"));
    }

    #[test]
    fn rockets_need_an_explosion() {
        let text = VALID.replace("\"bullet\"", "\"rocket\"");
        let err = parse_weapon("weapons/arc_rifle.json", "arc_rifle", &text).unwrap_err();
        assert!(
            err[0].message.starts_with("projectile.explosion"),
            "{err:?}"
        );
    }
}
