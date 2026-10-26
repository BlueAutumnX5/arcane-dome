//! Player settings, kept between runs in the user's config folder.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Starts as the system's language, if the game speaks it.
    pub language: Lang,
    pub fullscreen: bool,
    /// Menu pages slide into each other (off: they switch at once).
    pub slides: bool,
    /// Blood on the screen when hurt and on the death screen.
    pub blood: bool,
    pub show_fps: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Lang::from_system(),
            fullscreen: false,
            slides: true,
            blood: true,
            show_fps: false,
        }
    }
}

impl Settings {
    /// The saved settings, or the defaults if there are none (or they are
    /// unreadable).
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Save; a failure only means the settings are not remembered.
    pub fn save(&self) {
        let Some(p) = path() else {
            return;
        };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, text);
        }
    }
}

/// `%APPDATA%\arcane-dome\settings.json` on Windows,
/// `$XDG_CONFIG_HOME/arcane-dome/settings.json` (or `~/.config/...`)
/// elsewhere; `settings.json` in the app folder on Android.
fn path() -> Option<PathBuf> {
    if let Some(dir) = crate::platform::app_dir() {
        return Some(dir.join("settings.json"));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    Some(base?.join("arcane-dome").join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_keep_their_defaults() {
        let s: Settings = serde_json::from_str(r#"{"show_fps": true}"#).unwrap();
        assert!(s.show_fps);
        assert!(s.slides && s.blood && !s.fullscreen);
    }
}
