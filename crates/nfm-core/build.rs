//! Build script for `nfm-core`: locate the play-time resource pack and hand
//! its path to `pack.rs`, which embeds it with `include_bytes!`.
//!
//! Search order (batch D2a):
//!   1. `$NFM_PACK` (an explicit archive; must exist),
//!   2. `<workspace-root>/resources.pack`, where the workspace root is the
//!      first ancestor of `CARGO_MANIFEST_DIR` whose `Cargo.toml` carries a
//!      `[workspace]` table.
//!
//! When neither exists (a dev tree that has not run `tools/pack.sh`) we
//! generate the 22-byte empty-zip record into `OUT_DIR` and embed that:
//! `Zipo::read` accepts it, so every pack lookup misses and the debug
//! filesystem probe in the loaders stays in charge — behavior identical to
//! before the pack existed.
//!
//! std-only on purpose: `nfm-core` takes no new dependency for this.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The minimal valid zip: an End-of-Central-Directory record with zero
/// entries, zero central-directory size and offset, no comment.
const EMPTY_ZIP: [u8; 22] = [
    0x50, 0x4b, 0x05, 0x06, // EOCD signature
    0, 0, // this disk
    0, 0, // central-directory start disk
    0, 0, // entries on this disk
    0, 0, // entries total
    0, 0, 0, 0, // central-directory size
    0, 0, 0, 0, // central-directory offset
    0, 0, // comment length
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=NFM_PACK");

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    let root = workspace_root(&manifest_dir).unwrap_or_else(|| manifest_dir.clone());
    let pack_at_root = root.join("resources.pack");

    let pack = match env::var_os("NFM_PACK") {
        Some(v) => {
            let p = PathBuf::from(v);
            if !p.is_file() {
                panic!("NFM_PACK={:?} is not a file", p);
            }
            fs::canonicalize(&p).unwrap_or(p)
        }
        None if pack_at_root.is_file() => pack_at_root.clone(),
        None => {
            let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"))
                .join("resources.pack");
            // Idempotent: rewriting the same bytes would bump the file's
            // mtime, and cargo would then consider `nfm-core` dirty on every
            // build (a self-perpetuating rebuild).
            if fs::read(&out).ok().as_deref() != Some(EMPTY_ZIP.as_slice()) {
                fs::write(&out, EMPTY_ZIP).expect("write the empty resource pack");
            }
            out
        }
    };

    println!("cargo:rustc-env=NFM_PACK_PATH={}", pack.display());
    println!("cargo:rerun-if-changed={}", pack.display());
    // No watch on the workspace root / on `<root>/resources.pack` while it is
    // MISSING: cargo treats a missing watched path as dirty on every build
    // and recompiles the crate forever (2 s per build in a fresh dev tree).
    // `tools/pack.sh` therefore touches this build script after packing, so
    // the FIRST pack of a tree that was built without one is picked up; once
    // the archive exists the watch above tracks every later rewrite.
}

/// Walk up from `start` (inclusive) to the workspace root — the first
/// ancestor whose `Cargo.toml` contains a `[workspace]` table.
fn workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if let Ok(text) = fs::read_to_string(d.join("Cargo.toml"))
            && text.contains("[workspace]")
        {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}
