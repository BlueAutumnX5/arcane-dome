use super::{DataError, is_snake_case, parse_json};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Match rules. Behavior comes from the fields, not the id, so new modes
/// (e.g. a free-for-all elimination) need only a new entry in `modes.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeDef {
    #[serde(skip)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// 1 = everyone for themselves, 2 = two teams.
    pub teams: u8,
    /// false = elimination rounds.
    pub respawn: bool,
    #[serde(default)]
    pub score_limit: Option<u32>,
    #[serde(default)]
    pub rounds: Option<u32>,
    /// Whole match when respawning, per round otherwise.
    pub time_limit_s: f32,
}

impl ModeDef {
    pub fn display_name(&self) -> String {
        if let Some(n) = &self.name {
            return n.clone();
        }
        self.id
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn is_team(&self) -> bool {
        self.teams == 2
    }
}

pub fn parse_modes(label: &str, text: &str) -> Result<Vec<ModeDef>, Vec<DataError>> {
    let raw: BTreeMap<String, ModeDef> = parse_json(label, text).map_err(|e| vec![e])?;
    let mut errors = Vec::new();
    let mut out = Vec::new();
    for (id, mut mode) in raw {
        let mut bad = |msg: String| errors.push(DataError::new(label, format!("{id}.{msg}")));
        if !is_snake_case(&id) {
            bad("id must be snake_case".into());
        }
        if !(1..=2).contains(&mode.teams) {
            bad(format!("teams: must be 1 or 2 (got {})", mode.teams));
        }
        if mode.respawn && mode.score_limit.is_none_or(|s| s == 0) {
            bad("score_limit: required and greater than 0 when respawn is true".into());
        }
        if !mode.respawn && mode.rounds.is_none_or(|r| r == 0) {
            bad("rounds: required and greater than 0 when respawn is false".into());
        }
        if mode.time_limit_s <= 0.0 {
            bad(format!(
                "time_limit_s: must be greater than 0 (got {})",
                mode.time_limit_s
            ));
        }
        mode.id = id;
        out.push(mode);
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spec_modes() {
        let text = r#"{
          "deathmatch":       { "teams": 1, "respawn": true,  "score_limit": 25, "time_limit_s": 600 },
          "team_elimination": { "teams": 2, "respawn": false, "rounds": 5,       "time_limit_s": 180 }
        }"#;
        let modes = parse_modes("modes.json", text).unwrap();
        assert_eq!(modes.len(), 2);
        assert_eq!(modes[1].display_name(), "Team Elimination");
        assert!(!modes[1].respawn);
    }

    #[test]
    fn elimination_needs_rounds() {
        let text = r#"{ "x": { "teams": 2, "respawn": false, "time_limit_s": 60 } }"#;
        let err = parse_modes("modes.json", text).unwrap_err();
        assert!(
            err[0].to_string().starts_with("modes.json: x.rounds"),
            "{}",
            err[0]
        );
    }
}
