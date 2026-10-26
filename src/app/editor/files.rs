//! Map files beyond the editor's drafts: publishing into the game's map
//! list, and exporting / importing to share maps between installs.

use crate::data::GameData;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The folder maps are exported to and imported from: "Arcane Dome Maps"
/// in the user's Documents (or home) folder, or `ARCANE_DOME_MAPS` if set
/// (a portable install, or tests).
pub fn share_dir() -> PathBuf {
    match std::env::var_os("ARCANE_DOME_MAPS") {
        Some(dir) => PathBuf::from(dir),
        None => documents_dir().join("Arcane Dome Maps"),
    }
}

/// The user's Documents folder: on Linux the XDG setting if there is one,
/// else `~/Documents`; on Windows `%USERPROFILE%\Documents`. Falls back to
/// the home folder, then the current one.
fn documents_dir() -> PathBuf {
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let Some(home) = home else {
        return PathBuf::from(".");
    };
    if !cfg!(windows)
        && let Some(dir) = xdg_documents(&home)
    {
        return dir;
    }
    let docs = home.join("Documents");
    if docs.is_dir() { docs } else { home }
}

/// `XDG_DOCUMENTS_DIR` from `~/.config/user-dirs.dirs`, if set.
fn xdg_documents(home: &Path) -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let text = std::fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
    parse_xdg_documents(&text, home)
}

/// The Documents line of a `user-dirs.dirs` file, with `$HOME` filled in.
fn parse_xdg_documents(text: &str, home: &Path) -> Option<PathBuf> {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("XDG_DOCUMENTS_DIR="))?;
    let value = line.split_once('=')?.1.trim().trim_matches('"');
    let path = match value.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(value),
    };
    // A Documents folder equal to home means "not set up".
    (path != home).then_some(path)
}

/// Write a map to `<dir>/<id>.json`, creating the folder if needed.
pub fn export(dir: &Path, id: &str, json: &str) -> Result<PathBuf, String> {
    let path = dir.join(format!("{id}.json"));
    std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::write(&path, json))
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(path)
}

/// A map file found in the share folder.
pub struct SharedMap {
    pub path: PathBuf,
    /// The map's name, or the file name if it has none.
    pub name: String,
    /// Why it cannot be imported, if it can't.
    pub problem: Option<String>,
}

/// Every `.json` file in `dir`, by name.
pub fn list_shared(dir: &Path) -> Vec<SharedMap> {
    let mut list: Vec<SharedMap> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("json"))
        })
        .map(|path| {
            let file = path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()));
            match parsed {
                Ok(v) => SharedMap {
                    name: v
                        .get("name")
                        .and_then(Value::as_str)
                        .map_or(file, str::to_owned),
                    problem: v
                        .get("shapes")
                        .is_none()
                        .then(|| "not an Arcane Dome map".to_owned()),
                    path,
                },
                Err(e) => SharedMap {
                    name: file,
                    problem: Some(format!("not readable: {e}")),
                    path,
                },
            }
        })
        .collect();
    list.sort_by_key(|m| m.name.to_lowercase());
    list
}

/// Put a map into the game's map list (`assets/data/maps/<id>.json`), then
/// load the game data as the game would. If that fails, the old file (or
/// none) is put back, so a bad map can never stop the game from starting.
/// Returns the freshly loaded data.
pub fn publish(assets: &Path, id: &str, json: &str) -> Result<GameData, String> {
    let path = assets.join("data/maps").join(format!("{id}.json"));
    let before = std::fs::read(&path).ok();
    std::fs::write(&path, json).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    match GameData::load(assets) {
        Ok(data) => Ok(data),
        Err(e) => {
            let undo = match &before {
                Some(old) => std::fs::write(&path, old),
                None => std::fs::remove_file(&path),
            };
            let mut msg = format!("Not published: the game could not load it ({e})");
            if let Err(u) = undo {
                msg.push_str(&format!(". Also could not undo: {u}"));
            }
            Err(msg)
        }
    }
}

/// Open a folder in the system's file manager.
pub fn open_folder(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = std::process::Command::new(cmd)
        .arg(dir)
        .spawn()
        .map_err(|e| format!("Could not open {}: {e}", dir.display()))?;
    // Reap it when it exits, so it does not linger.
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty folder under the system temp folder.
    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("arcane-dome-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    #[test]
    fn xdg_documents_line_is_read() {
        let home = Path::new("/home/me");
        let text =
            "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOCUMENTS_DIR=\"$HOME/Dokumen\"\n";
        assert_eq!(
            parse_xdg_documents(text, home),
            Some(PathBuf::from("/home/me/Dokumen"))
        );
        assert_eq!(
            parse_xdg_documents("XDG_DOCUMENTS_DIR=\"$HOME/\"", home),
            None
        );
        assert_eq!(parse_xdg_documents("", home), None);
    }

    #[test]
    fn exported_maps_are_listed_by_name() {
        let dir = temp_dir("share");
        export(&dir, "b_map", r#"{"name": "Bravo", "shapes": []}"#).unwrap();
        export(&dir, "a_map", r#"{"name": "alpha", "shapes": []}"#).unwrap();
        std::fs::write(dir.join("junk.json"), "not json").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let list = list_shared(&dir);
        let names: Vec<_> = list.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Bravo", "junk"]);
        assert!(list[0].problem.is_none() && list[2].problem.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publishing_a_bad_map_puts_the_old_file_back() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let assets = temp_dir("publish");
        copy_dir(&root.join("data"), &assets.join("data"));
        let path = assets.join("data/maps/redstone_arches.json");
        let original = std::fs::read_to_string(&path).unwrap();

        // Broken: the loader refuses it, so the original comes back.
        let err = publish(&assets, "redstone_arches", "{}").unwrap_err();
        assert!(err.starts_with("Not published"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);

        // A broken new map leaves no file behind.
        publish(&assets, "brand_new", "{}").unwrap_err();
        assert!(!assets.join("data/maps/brand_new.json").exists());

        // A good one is published and loads.
        let data = publish(&assets, "redstone_arches", &original).unwrap();
        assert!(data.maps.iter().any(|m| m.id == "redstone_arches"));
        let _ = std::fs::remove_dir_all(&assets);
    }
}
