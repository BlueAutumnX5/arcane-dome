//! Android builds carry the game data inside the app (an APK has no plain
//! folder to read from): this lists every file under `assets/data` for
//! `include_bytes!`, plus a fingerprint so an updated app knows to unpack
//! it again. Other targets get an empty list and read `assets/` as usual.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let data = root.join("assets").join("data");
    let android = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android");
    println!("cargo:rerun-if-changed=build.rs");

    let mut files = Vec::new();
    if android {
        println!("cargo:rerun-if-changed={}", data.display());
        collect(&data, &mut files);
        files.sort();
    }

    // FNV-1a over names and contents.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut out = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let rel = path
            .strip_prefix(root.join("assets"))
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        for b in rel.bytes().chain(std::fs::read(path).unwrap()) {
            hash = (hash ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        writeln!(
            out,
            "    ({rel:?}, include_bytes!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    writeln!(out, "pub const FINGERPRINT: &str = \"{hash:016x}\";").unwrap();

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bundled_assets.rs");
    std::fs::write(dest, out).unwrap();
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}
