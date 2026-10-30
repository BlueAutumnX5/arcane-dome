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

/// Memory the game holds, in bytes: RAM (its resident set), and graphics
/// memory where the kernel reports it (Linux GPU drivers list it per open
/// GPU file in /proc/self/fdinfo; an integrated GPU's memory is system RAM
/// that the RAM figure does not include). `None` where the system won't say.
pub fn memory_use() -> (Option<u64>, Option<u64>) {
    let ram = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map(|pages| pages * 4096);
    let mut clients: Vec<String> = Vec::new();
    let mut gpu = None;
    for entry in std::fs::read_dir("/proc/self/fdinfo")
        .into_iter()
        .flatten()
        .flatten()
    {
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        // One GPU client can be open under several file numbers.
        if let Some((client, bytes)) = drm_memory(&text)
            && !clients.contains(&client)
        {
            clients.push(client);
            gpu = Some(gpu.unwrap_or(0) + bytes);
        }
    }
    (ram, gpu)
}

/// A GPU file's client id and the memory it holds, from its fdinfo text:
/// resident memory if listed, else the totals (older kernels), else the
/// older `drm-memory-` lines.
fn drm_memory(fdinfo: &str) -> Option<(String, u64)> {
    let client = fdinfo
        .lines()
        .find_map(|l| l.strip_prefix("drm-client-id:"))?
        .trim()
        .to_owned();
    let key = ["drm-resident-", "drm-total-", "drm-memory-"]
        .into_iter()
        .find(|k| fdinfo.contains(k))?;
    let bytes = fdinfo
        .lines()
        .filter(|l| l.starts_with(key))
        .filter_map(|l| {
            let mut v = l.split_once(':')?.1.split_whitespace();
            let n: u64 = v.next()?.parse().ok()?;
            Some(match v.next() {
                Some("KiB") => n << 10,
                Some("MiB") => n << 20,
                Some("GiB") => n << 30,
                _ => n,
            })
        })
        .sum();
    Some((client, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_memory_is_read_from_a_drm_fdinfo() {
        let i915 = "pos:\t0\nflags:\t02100002\ndrm-driver:\ti915\ndrm-client-id:\t42\n\
                    drm-total-system0:\t12 MiB\ndrm-resident-system0:\t10 MiB\n\
                    drm-resident-stolen-system0:\t512 KiB\ndrm-engine-render:\t9000 ns\n";
        assert_eq!(
            drm_memory(i915),
            Some(("42".into(), (10 << 20) + (512 << 10)))
        );
        let old_amd = "drm-client-id:\t7\ndrm-memory-vram:\t2048 KiB\ndrm-memory-gtt:\t1024 KiB\n";
        assert_eq!(drm_memory(old_amd), Some(("7".into(), 3 << 20)));
        assert_eq!(drm_memory("pos:\t0\nflags:\t02\n"), None);
    }

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
