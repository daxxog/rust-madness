//! The embedded play-time resource pack (batch D2a).
//!
//! `tools/pack.sh` builds ONE zip at the workspace root from `assets/**` and
//! the glyph-atlas fixtures; `build.rs` points `NFM_PACK_PATH` at it (or at a
//! generated empty zip when a dev tree has no pack) and this module embeds
//! those bytes with `include_bytes!`. The archive is opened ONCE per process
//! (lazy `OnceLock<Pack>`) and entries are handed out zero-copy as
//! `&'static [u8]`; a caller that wants an owned `Vec<u8>` copies that one
//! entry, never the archive.
//!
//! # Name mapping (the loaders' probes → the pack's canonical names)
//!
//! Entry names are REPO-ROOT-RELATIVE with forward slashes. The loaders keep
//! their historic tails (relative to `assets/`, plus the crate-dir probe) and
//! the pack lookup prepends `assets/`:
//!
//! | loader probe                                         | canonical pack name  |
//! |------------------------------------------------------|----------------------|
//! | `stage::read_asset_at(None, tail)`: `assets/{tail}`  | `assets/{tail}`      |
//! | same loader's crate-dir fallback `../../assets/{tail}` | `assets/{tail}`    |
//! | `XtGraphics::asset(tail)`: `assets` / `../../assets` | `assets/{tail}`      |
//! | `audioclip::get_sound(radical_path)`                 | `assets/{radical_path}` |
//! | `xtfont` atlas (`atlas.txt` / `atlas.png`)           | `tests/fixtures/xtfont-atlas/atlas.{txt,png}` |
//!
//! So `tail = "graphics/models.zipo"` is `assets/graphics/models.zipo`, and
//! `tail = "stages/1.txt"` is `assets/stages/1.txt`. The two filesystem roots
//! a debug build probes collapse onto the same canonical name.
//!
//! # Build modes
//!
//! - Release (`not(debug_assertions)`): the pack is the ONLY resource source.
//!   A missing name is a clean `NotFound`-style error / a loud panic for the
//!   atlas — no filesystem probe exists, so a shipped binary runs from any
//!   cwd with no asset tree beside it.
//! - Debug: env override first where one exists (`NFM_XTFONT_ATLAS`), then
//!   the filesystem exactly as before, then this pack. With no
//!   `resources.pack` the embedded archive is the empty zip, so the last
//!   step is a no-op and dev/mirror behavior is byte-identical to the
//!   pre-pack tree.

use std::sync::OnceLock;

use crate::zipo::Zipo;

/// The archive bytes `build.rs` pointed `NFM_PACK_PATH` at: the real
/// `resources.pack` when present, else a generated 22-byte empty zip.
static PACK_BYTES: &[u8] = include_bytes!(env!("NFM_PACK_PATH"));

struct Pack {
    zipo: Zipo,
}

static PACK: OnceLock<Pack> = OnceLock::new();

/// The process-wide archive, parsed on first use (never more than once).
fn pack() -> &'static Pack {
    PACK.get_or_init(|| match Zipo::read(PACK_BYTES) {
        Ok(zipo) => Pack { zipo },
        Err(e) => panic!(
            "resource pack: the embedded archive ({} bytes, from NFM_PACK_PATH) \
             is not a readable zip: {e}",
            PACK_BYTES.len()
        ),
    })
}

/// One entry's bytes, zero-copy, or `None` when the name is absent (an
/// empty pack misses everything, which is the dev-without-pack case).
pub fn get(name: &str) -> Option<&'static [u8]> {
    pack().zipo.get(name).map(|e| e.data.as_slice())
}

/// The number of entries in the embedded pack (0 for the empty-zip stub).
pub fn len() -> usize {
    pack().zipo.entries.len()
}

/// Entry names in archive order (diagnostics and tests).
pub fn names() -> Vec<&'static str> {
    pack()
        .zipo
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decisive round-trip in-process: with a real pack (built by
    /// `tools/pack.sh`) every entry's bytes equal the worktree file's. A dev
    /// tree without a pack embeds the empty zip, where there is nothing to
    /// compare — the fs probe is in charge there.
    #[test]
    fn embedded_pack_entries_match_the_worktree_files() {
        if len() == 0 {
            return;
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in names() {
            assert!(name.starts_with("assets/") || name.starts_with("tests/fixtures/"));
            let path = root.join(name);
            let disk = std::fs::read(&path).unwrap_or_else(|e| {
                panic!(
                    "pack entry {name:?} has no worktree file {}: {e}",
                    path.display()
                )
            });
            assert_eq!(
                get(name).unwrap(),
                disk.as_slice(),
                "pack entry {name:?} differs from its worktree file"
            );
        }
    }

    #[test]
    fn missing_names_miss_cleanly() {
        assert!(get("").is_none());
        assert!(get("assets/graphics/no-such-archive.zipo").is_none());
        assert!(get("tests/fixtures/xtfont-atlas/no-such-atlas.png").is_none());
    }

    /// The dev-without-pack stub must be a real zip the reader accepts (the
    /// bytes are `build.rs`'s `EMPTY_ZIP`; keep the two in sync).
    #[test]
    fn the_empty_pack_stub_is_a_valid_zip() {
        let eocd: [u8; 22] = [
            0x50, 0x4b, 0x05, 0x06, // EOCD signature
            0, 0, // this disk
            0, 0, // central-directory start disk
            0, 0, // entries on this disk
            0, 0, // entries total
            0, 0, 0, 0, // central-directory size
            0, 0, 0, 0, // central-directory offset
            0, 0, // comment length
        ];
        let zipo = Zipo::read(&eocd).expect("the 22-byte empty zip parses");
        assert!(zipo.entries.is_empty());
    }
}
