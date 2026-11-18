//! Pure helpers shared by several mirror modules: the float bit pattern
//! (`bits`), the `java.util.zip.CRC32` transcription, and the two
//! fixture-directory probes.
//!
//! Test-only by construction: the parent [`crate::mirror`] module is
//! `#[cfg(test)]`, so none of this reaches the production library. Class
//! fixtures (`car`/`madness`/`mk`/`snapshot`/`preset` builders and the
//! `*_pins` sections) stay in their own T-module: each transcribes a
//! different Java class, and keeping them 1:1 is the mirror's point.

use std::path::PathBuf;

/// `Float.floatToIntBits(Float)` — the raw IEEE-754 bits of a `float`. Java
/// `float` equality is bit equality here (`Check.eq` boxes `Float`, i.e.
/// `Float.floatToIntBits`), so every fixture pins a computed float exactly
/// through this convention. `Float.floatToIntBits` collapses all NaNs to
/// `0x7fc00000`, which is also what Rust's `f32::to_bits` yields for the
/// canonical NaN; the pinned streams contain none.
pub(crate) fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}

/// `java.util.zip.CRC32.getValue()` over the byte array (CRC-32/ISO-HDLC:
/// reflected polynomial 0xEDB88320, init/xorout 0xFFFFFFFF); `getValue` is
/// a long, so the 32-bit value widens unsigned. Each signed byte is fed as
/// its unsigned value (`update(byte[])` takes the low 8 bits).
pub(crate) fn crc32(data: &[i8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u8 as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// The Java classes' `target/java-tests-fixtures/*` scratch home: under the
/// workspace build target (the probe scratch area), shared by the classes
/// that build fixture trees from the shipped assets.
pub(crate) fn mirror_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/mirror-fixtures")
}

/// The shipped asset tree, probed like `fixture::root` (cargo runs tests
/// from the crate dir; the suite may also be driven from the repo root).
pub(crate) fn assets_dir() -> PathBuf {
    for cand in ["../../assets", "assets"] {
        let p = PathBuf::from(cand);
        if p.is_dir() {
            return p;
        }
    }
    panic!("assets not found (run cargo from the repo root or crates/nfm-core)");
}
