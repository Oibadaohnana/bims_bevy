//! The game's fingerprint: one number for the source every rule is
//! compiled from, so two players about to share a world can tell whether
//! they run the same game (`net::FINGERPRINT`).
//!
//! `wire::PROTOCOL` comes from `BUILD`, which moves only when the game is
//! pushed to the server; a game built from a tree with changes since (a
//! `cargo run` beside a shipped build, or yesterday's binary beside
//! today's) passes that check and plays another simulation — the worlds
//! part at the first checksum and again after every resync. Every file of
//! the crates the world is stepped by, the app's own (what crosses the
//! wire and how it is applied) and the lock file (the libraries'
//! versions) is read in a fixed order and hashed with FNV-1a.

use std::fs;
use std::path::{Path, PathBuf};

/// The crates whose source the fingerprint is taken of, from the
/// workspace's `crates/`.
const CRATES: &[&str] = &[
    "app",
    "economy",
    "flight",
    "game",
    "physics",
    "ship",
    "shipdesign",
    "time",
    "wire",
    "world",
    "worldgen",
];

fn main() {
    let here = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let crates = here.parent().expect("crates/app is in crates/");
    let root = crates.parent().expect("crates/ is in the workspace");
    let mut files = Vec::new();
    for name in CRATES {
        let src = crates.join(name).join("src");
        println!("cargo:rerun-if-changed={}", src.display());
        walk(&src, &mut files);
    }
    let lock = root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    files.push(lock);
    files.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for file in &files {
        // The path from the workspace's root, so where the tree is checked
        // out does not move the number.
        let name = file.strip_prefix(root).unwrap_or(file);
        eat(name.to_string_lossy().replace('\\', "/").as_bytes());
        eat(&[0]);
        eat(&fs::read(file).unwrap_or_default());
        eat(&[0]);
    }
    println!("cargo:rustc-env=BIMS_FINGERPRINT={hash:016x}");
}

/// Every `.rs` and `.wgsl` file under `dir`, and the data files read at
/// compile time beside them.
fn walk(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, into);
        } else if path
            .extension()
            .is_some_and(|e| e == "rs" || e == "ron" || e == "wgsl")
        {
            into.push(path);
        }
    }
}
