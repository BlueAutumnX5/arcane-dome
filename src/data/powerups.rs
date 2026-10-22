use super::{DataError, is_snake_case, parse_hex_color, parse_json};
use serde::Deserialize;

/// What a powerup does while active.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PowerupEffect {
    /// Takes no damage at all.
    Invulnerable,
    /// Multiplies weapon fire rate.
    FireRate { mult: f32 },
    /// Multiplies all damage dealt.
    Damage { mult: f32 },
    /// Random teleport to a platform. Held as charges and used with the
    /// ability key, or fired immediately when `on_pickup` is true.
    Teleport {
        #[serde(default = "one")]
        charges: u32,
        #[serde(default)]
        on_pickup: bool,
    },
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerupDef {
    pub id: String,
    pub name: String,
    /// `#rrggbb`, used for the orb and the fighter's overlay.
    #[serde(rename = "color")]
    pub color_hex: String,
    #[serde(skip)]
    pub color: [u8; 3],
    /// Seconds the effect lasts (0 for instant / charge-based effects).
    #[serde(default)]
    pub duration_s: f32,
    pub effect: PowerupEffect,
}

/// Contents of `powerups.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerupConfig {
    /// Delay before the first powerup appears in a match.
    pub first_spawn_s: f32,
    /// Delay before a new powerup appears after one is taken.
    pub respawn_s: f32,
    pub powerups: Vec<PowerupDef>,
}

impl Default for PowerupConfig {
    fn default() -> Self {
        Self {
            first_spawn_s: 45.0,
            respawn_s: 120.0,
            powerups: Vec::new(),
        }
    }
}

pub fn parse_powerups(label: &str, text: &str) -> Result<PowerupConfig, Vec<DataError>> {
    let mut cfg: PowerupConfig = parse_json(label, text).map_err(|e| vec![e])?;
    let mut errs = Vec::new();
    let mut bad = |m: String| errs.push(DataError::new(label, m));
    if cfg.first_spawn_s < 0.0 {
        bad("first_spawn_s: must be 0 or more".into());
    }
    if cfg.respawn_s <= 0.0 {
        bad("respawn_s: must be greater than 0".into());
    }
    let mut seen = Vec::new();
    for (i, p) in cfg.powerups.iter_mut().enumerate() {
        if !is_snake_case(&p.id) {
            bad(format!("powerups[{i}].id: \"{}\" must be snake_case", p.id));
        }
        if seen.contains(&p.id) {
            bad(format!("powerups[{i}].id: \"{}\" is used twice", p.id));
        }
        seen.push(p.id.clone());
        match parse_hex_color(&p.color_hex) {
            Some(c) => p.color = c,
            None => bad(format!(
                "powerups[{i}].color: \"{}\" is not a #rrggbb color",
                p.color_hex
            )),
        }
        if p.duration_s < 0.0 {
            bad(format!("powerups[{i}].duration_s: must be 0 or more"));
        }
        match p.effect {
            PowerupEffect::FireRate { mult } | PowerupEffect::Damage { mult } if mult <= 0.0 => {
                bad(format!("powerups[{i}].effect.mult: must be greater than 0"))
            }
            PowerupEffect::Teleport {
                charges: 0,
                on_pickup: false,
            } => bad(format!("powerups[{i}].effect.charges: must be at least 1")),
            _ => {}
        }
    }
    if errs.is_empty() { Ok(cfg) } else { Err(errs) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_effects() {
        let text = r##"{ "first_spawn_s": 45, "respawn_s": 120, "powerups": [
            { "id": "a", "name": "A", "color": "#ff0000", "duration_s": 20, "effect": { "type": "damage", "mult": 2 } },
            { "id": "b", "name": "B", "color": "#0000ff", "effect": { "type": "teleport" } }
        ] }"##;
        let cfg = parse_powerups("powerups.json", text).unwrap();
        assert_eq!(cfg.powerups[0].color, [255, 0, 0]);
        assert_eq!(cfg.powerups[0].effect, PowerupEffect::Damage { mult: 2.0 });
        assert_eq!(
            cfg.powerups[1].effect,
            PowerupEffect::Teleport {
                charges: 1,
                on_pickup: false
            }
        );
    }

    #[test]
    fn unknown_effect_names_the_field() {
        let text = r##"{ "first_spawn_s": 1, "respawn_s": 1, "powerups": [
            { "id": "a", "name": "A", "color": "#ff0000", "effect": { "type": "flight" } } ] }"##;
        let err = parse_powerups("powerups.json", text).unwrap_err();
        assert!(
            err[0].to_string().contains("powerups[0].effect"),
            "{}",
            err[0]
        );
    }
}
