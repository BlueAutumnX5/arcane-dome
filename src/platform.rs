//! What differs between the desktop and Android builds: where the game's
//! files live, and whether the player has a touch screen.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod bundled {
    include!(concat!(env!("OUT_DIR"), "/bundled_assets.rs"));
}

/// Play with on-screen touch controls? Always on Android; `ARCANE_TOUCH=1`
/// turns them on for a desktop test run.
pub fn touch() -> bool {
    static TOUCH: OnceLock<bool> = OnceLock::new();
    *TOUCH.get_or_init(|| cfg!(target_os = "android") || std::env::var_os("ARCANE_TOUCH").is_some())
}

/// The screen height the menus and HUD are laid out for: everything is
/// drawn bigger on a touch screen, to fit fingers on a small screen.
pub fn ui_height() -> f32 {
    if touch() { 540.0 } else { 720.0 }
}

/// The app's private folder on Android (settings, unpacked game data,
/// published maps). `None` on the desktop, where each kind of file has its
/// usual home.
pub fn app_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(android_files_dir).clone()
}

#[cfg(target_os = "android")]
fn android_files_dir() -> Option<PathBuf> {
    use macroquad::miniquad::{self, native::android};
    // SAFETY: JNI calls on the game thread, which miniquad attaches; the
    // activity reference stays valid while the game runs.
    unsafe {
        let env = android::attach_jni_env();
        let activity = android::ACTIVITY;
        let file = miniquad::call_object_method!(env, activity, "getFilesDir", "()Ljava/io/File;");
        if file.is_null() {
            return None;
        }
        let path =
            miniquad::call_object_method!(env, file, "getAbsolutePath", "()Ljava/lang/String;");
        if path.is_null() {
            return None;
        }
        Some(PathBuf::from(miniquad::get_utf_str!(env, path)))
    }
}

#[cfg(not(target_os = "android"))]
fn android_files_dir() -> Option<PathBuf> {
    None
}

/// The game data carried inside the app, unpacked into the app folder so
/// it loads like any `assets` folder. `None` if this build carries none.
pub fn bundled_assets() -> Option<PathBuf> {
    if bundled::FILES.is_empty() {
        return None;
    }
    let root = app_dir()?.join("assets");
    unpack(bundled::FILES, bundled::FINGERPRINT, &root).ok()?;
    Some(root)
}

/// The marker that records which bundle was unpacked last.
const MARK: &str = ".bundled";

/// Write `files` under `root` unless this exact bundle is already there.
/// Files that an older bundle had and this one dropped are removed; any
/// other file (a map the player published) is left alone.
fn unpack(files: &[(&str, &[u8])], fingerprint: &str, root: &Path) -> std::io::Result<()> {
    let mark = root.join(MARK);
    let old = std::fs::read_to_string(&mark).unwrap_or_default();
    let mut lines = old.lines();
    if lines.next() == Some(fingerprint) {
        return Ok(());
    }
    for gone in lines.filter(|l| !files.iter().any(|(rel, _)| rel == l)) {
        let _ = std::fs::remove_file(root.join(gone));
    }
    for (rel, bytes) in files {
        let path = root.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, bytes)?;
    }
    let mut text = format!("{fingerprint}\n");
    for (rel, _) in files {
        text.push_str(rel);
        text.push('\n');
    }
    std::fs::write(mark, text)
}

/// On Android a crash would vanish without a trace: send the message to
/// the system log and keep a copy in `crash.txt` in the app folder.
pub fn install_crash_log() {
    #[cfg(target_os = "android")]
    {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let text = format!("Arcane Dome crashed: {info}");
            if let Ok(c) = std::ffi::CString::new(text.clone()) {
                // SAFETY: a valid C string for the logger to copy.
                unsafe { macroquad::miniquad::native::android::console_error(c.as_ptr()) };
            }
            if let Some(dir) = app_dir() {
                let _ = std::fs::write(dir.join("crash.txt"), &text);
            }
            default(info);
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_unpacks_once_and_cleans_up_after_itself() {
        let root = std::env::temp_dir().join(format!("arcane-dome-unpack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let v1: &[(&str, &[u8])] = &[("data/a.json", b"1"), ("data/maps/old.json", b"o")];
        unpack(v1, "v1", &root).unwrap();
        assert_eq!(std::fs::read(root.join("data/a.json")).unwrap(), b"1");

        // A map the player published survives updates.
        std::fs::write(root.join("data/maps/mine.json"), "m").unwrap();
        // The same bundle again leaves edits alone.
        std::fs::write(root.join("data/a.json"), "edited").unwrap();
        unpack(v1, "v1", &root).unwrap();
        assert_eq!(std::fs::read(root.join("data/a.json")).unwrap(), b"edited");

        // A new bundle rewrites its files and drops the ones it no longer has.
        let v2: &[(&str, &[u8])] = &[("data/a.json", b"2")];
        unpack(v2, "v2", &root).unwrap();
        assert_eq!(std::fs::read(root.join("data/a.json")).unwrap(), b"2");
        assert!(!root.join("data/maps/old.json").exists());
        assert!(root.join("data/maps/mine.json").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
