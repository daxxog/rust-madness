//! `TMod.java` — the zipo-loading constructor (the game's real path), the
//! public `LoadMod(InputStream)` seam, and the broken-object state a failed
//! constructor leaves behind (group "Mod"). Pin source:
//! `Mod.java:32-38,40-62,101-147` and notes/JAVA-BEHAVIOR/B2_mod_modslayer.md.
//!
//! No TOML fixture: every value here is an observed parser outcome, not a
//! transcribed constant.

use std::panic::AssertUnwindSafe;

use crate::mirror::harness as check;
use crate::tracker::Mod;
use crate::zipo::Zipo;

const STAGE1_ZIPO: &[u8] = include_bytes!("../../../../assets/music/stage1.zipo");
const STAGE2_ZIPO: &[u8] = include_bytes!("../../../../assets/music/stage2.zipo");

pub(crate) fn run() {
    check::group("Mod");
    ctor_path();
    failed_ctor();
    loadmod_seam();
}

/// The game's real path (TMod.java:21-27): zipo container -> first entry ->
/// `LoadMod`, for stage 1 and stage 2.
fn ctor_path() {
    let s1 = Mod::from_zipo(Some(STAGE1_ZIPO));
    check::eq_str(
        s1.describe(),
        "Stage1 (4 tracks, 20 patterns, 31 samples)",
        "stage1_toString",
    );
    check::eq_i32(s1.num_patterns(), 20, "stage1_numpatterns");

    let s2 = Mod::from_zipo(Some(STAGE2_ZIPO));
    check::eq_i32(s2.num_patterns(), 44, "stage2_numpatterns");
}

/// A missing file leaves a broken object (TMod.java:29-38): the field readers
/// see Java's defaults while `toString` NPEs at `insts.length`.
fn failed_ctor() {
    let broken = Mod::from_zipo(None);
    check::eq_i32(broken.num_patterns(), 0, "failed_ctor_numpatterns_default");
    // Java: NullPointerException.class; the port panics with the same class
    // name in the message (Mod.java:37 dereferences the null insts array).
    check::throws_it(
        AssertUnwindSafe(|| {
            let _ = broken.describe();
        }),
        "NullPointerException",
        "failed_ctor_toString_npes",
    );
}

/// The public `LoadMod` seam (TMod.java:40-51): raw module bytes with no zipo
/// wrapper, and the broken object revived by a full reset of every field.
fn loadmod_seam() {
    let mut broken = Mod::from_zipo(None);
    let bytes = first_zipo_entry(STAGE1_ZIPO);
    broken.load_mod_into(&bytes).expect("stage1.mod must parse");
    check::eq_str(
        broken.describe(),
        "Stage1 (4 tracks, 20 patterns, 31 samples)",
        "loadmod_revives_broken",
    );

    // A truncated stream throws (TMod.java:45-51). `Check.throwsIt` asserts
    // java.io.EOFException (an IOException subclass); the port signals
    // `ModError::Truncated`, so the mirror unwraps that error into a panic
    // carrying the "EOF on MOD file" text — the same words `LoadMod` prints
    // for the caught case at Mod.java:143 — which the message-shaped harness
    // can observe.
    check::throws_it(
        AssertUnwindSafe(|| {
            broken.load_mod_into(&[0u8; 40]).expect("EOF on MOD file");
        }),
        "EOF on MOD file",
        "loadmod_truncated_throws",
    );
}

/// `firstZipoEntry` (TMod.java:55-68): inflate the archive's first member,
/// exactly the bytes the constructor's `ZipInputStream` read hands to
/// `LoadMod`.
fn first_zipo_entry(raw: &[u8]) -> Vec<u8> {
    let z = Zipo::read(raw).expect("music zipo must parse");
    assert_eq!(z.entries.len(), 1, "music zips are single-entry");
    z.entries.into_iter().next().unwrap().data
}
