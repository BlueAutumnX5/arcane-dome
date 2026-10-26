//! The game's text in other languages.
//!
//! English is the key: code writes `t("Quick Match")`, and while Bahasa
//! Indonesia is on, the table in `assets/lang/id.json` turns that into "Main
//! Cepat". Text with numbers or names in it goes through `tf`, where `{}`
//! (or `{0}`, `{1}`, ... when a language needs another order) marks each
//! value. Anything missing from a table shows in English; a test checks that
//! nothing is missing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "id")]
    Indonesian,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::English, Lang::Indonesian];

    /// The language's own name for itself.
    pub fn label(self) -> &'static str {
        match self {
            Lang::English => "English",
            Lang::Indonesian => "Bahasa Indonesia",
        }
    }

    /// The system's language, if it is one the game speaks; else English.
    pub fn from_system() -> Lang {
        system_language().map_or(Lang::English, |code| Lang::from_code(&code))
    }

    /// The language a locale code (like "id_ID.UTF-8") names, or English.
    fn from_code(code: &str) -> Lang {
        // Older Java calls Indonesian "in".
        if code.starts_with("id") || code.starts_with("in_") || code == "in" {
            Lang::Indonesian
        } else {
            Lang::English
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Switch the language everything is shown in.
pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Indonesian,
        _ => Lang::English,
    }
}

/// The Indonesian table: English text to Indonesian text.
fn indonesian() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/lang/id.json"))
            .expect("assets/lang/id.json is not a valid table")
    })
}

/// `en` in the current language.
pub fn t(en: &str) -> &str {
    match current() {
        Lang::English => en,
        Lang::Indonesian => indonesian().get(en).map_or(en, String::as_str),
    }
}

/// Marks text kept in a fixed list (a `const`, say) that is translated
/// with `t` where it is shown, so the check below still finds it.
pub const fn tn(en: &str) -> &str {
    en
}

/// `template` in the current language, with `args` put in its `{}` (in
/// turn) or `{0}`, `{1}`, ... (by number).
pub fn tf(template: &str, args: &[&dyn Display]) -> String {
    fill(t(template), args)
}

fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if after[..close].chars().all(|c| c.is_ascii_digit()) => {
                let index = if close == 0 {
                    next += 1;
                    next - 1
                } else {
                    after[..close].parse().unwrap_or(usize::MAX)
                };
                if let Some(a) = args.get(index) {
                    out.push_str(&a.to_string());
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The language code the system is set to (like "id_ID.UTF-8" or "en").
#[cfg(not(target_os = "android"))]
fn system_language() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
}

#[cfg(target_os = "android")]
fn system_language() -> Option<String> {
    use macroquad::miniquad::{self, native::android};
    // SAFETY: plain JNI calls on the game thread (see platform::app_dir).
    unsafe {
        let env = android::attach_jni_env();
        let find_class = (**env).FindClass.unwrap();
        let static_method = (**env).GetStaticMethodID.unwrap();
        let call_static = (**env).CallStaticObjectMethod.unwrap();
        let class = find_class(env, c"java/util/Locale".as_ptr());
        if class.is_null() {
            return None;
        }
        let get_default = static_method(
            env,
            class,
            c"getDefault".as_ptr(),
            c"()Ljava/util/Locale;".as_ptr(),
        );
        let locale = call_static(env, class, get_default);
        if locale.is_null() {
            return None;
        }
        let code =
            miniquad::call_object_method!(env, locale, "getLanguage", "()Ljava/lang/String;");
        if code.is_null() {
            return None;
        }
        Some(miniquad::get_utf_str!(env, code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn values_fill_in_order_or_by_number() {
        assert_eq!(fill("Respawn in {}", &[&3]), "Respawn in 3");
        assert_eq!(fill("{} of {}", &[&"a", &"b"]), "a of b");
        assert_eq!(fill("{1} of {0}", &[&"a", &"b"]), "b of a");
        assert_eq!(fill("{x} {", &[&1]), "{x} {");
    }

    #[test]
    fn indonesian_is_picked_from_the_system_language() {
        let pick = Lang::from_code;
        assert_eq!(pick("id_ID.UTF-8"), Lang::Indonesian);
        assert_eq!(pick("in"), Lang::Indonesian);
        assert_eq!(pick("en_US.UTF-8"), Lang::English);
        assert_eq!(pick("it_IT"), Lang::English);
    }

    /// Every `t("...")`, `tf("...")` and `tn("...")` in the code, unescaped.
    fn keys_in(dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                keys_in(&path, out);
                continue;
            }
            if path.extension().is_none_or(|x| x != "rs") || path.ends_with("i18n.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let bytes = text.as_bytes();
            for call in ["t(", "tf(", "tn("] {
                let mut from = 0;
                while let Some(at) = text[from..].find(call) {
                    let start = from + at;
                    from = start + call.len();
                    // A whole word: not `text(` or `fmt(`.
                    if start > 0
                        && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')
                    {
                        continue;
                    }
                    // The text may start on the next line.
                    let skipped = text[from..].len() - text[from..].trim_start().len();
                    if !text[from + skipped..].starts_with('"') {
                        continue;
                    }
                    from += skipped + 1;
                    let mut s = String::new();
                    let mut chars = text[from..].chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => match chars.next() {
                                Some('n') => s.push('\n'),
                                Some(other) => s.push(other),
                                None => {}
                            },
                            c => s.push(c),
                        }
                    }
                    out.push((s, path.display().to_string()));
                }
            }
        }
    }

    #[test]
    fn every_text_in_the_code_has_indonesian() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut keys = Vec::new();
        keys_in(&root, &mut keys);
        assert!(keys.len() > 100, "found only {} texts", keys.len());
        let table = indonesian();
        let missing: Vec<_> = keys
            .iter()
            .filter(|(k, _)| !table.contains_key(k))
            .map(|(k, file)| format!("{file}: {k:?}"))
            .collect();
        assert!(
            missing.is_empty(),
            "no Indonesian for:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn translations_keep_every_value() {
        // Each `{}` / `{n}` in the English must be in the Indonesian too.
        let marks = |s: &str| {
            let mut m: Vec<String> = Vec::new();
            let mut n = 0;
            let mut rest = s;
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                match after.find('}') {
                    Some(close) if after[..close].chars().all(|c| c.is_ascii_digit()) => {
                        m.push(if close == 0 {
                            n += 1;
                            (n - 1).to_string()
                        } else {
                            after[..close].to_owned()
                        });
                        rest = &after[close + 1..];
                    }
                    _ => rest = after,
                }
            }
            m.sort();
            m
        };
        for (en, id) in indonesian() {
            assert_eq!(marks(en), marks(id), "values differ: {en:?} vs {id:?}");
        }
    }

    #[test]
    fn every_game_data_text_has_indonesian() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let data = crate::data::GameData::load(&assets).unwrap();
        let table = indonesian();
        let mut texts: Vec<String> = Vec::new();
        for w in &data.weapons {
            texts.push(w.name.clone());
            texts.push(w.short_name());
        }
        for m in &data.modes {
            texts.push(m.display_name());
            texts.push(m.description.clone());
        }
        for p in &data.powerups.powerups {
            texts.push(p.name.clone());
        }
        for m in &data.maps {
            texts.push(m.name.clone());
            texts.push(m.description.clone());
        }
        let missing: Vec<_> = texts
            .iter()
            .filter(|s| !s.is_empty() && !table.contains_key(*s))
            .collect();
        assert!(missing.is_empty(), "no Indonesian for: {missing:#?}");
    }
}
