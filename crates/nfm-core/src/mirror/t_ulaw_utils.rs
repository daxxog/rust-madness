// FIDELITY: TUlawUtils.java table checks walk `s2u_lut` by index like the Java original.
#![allow(clippy::needless_range_loop)]
//! `TUlawUtils.java` — the `s2u_lut` table properties, the clamping encoder
//! and the two DISTINCT `linear2ulaw` overloads (group "UlawUtils"). Pin
//! source: `UlawUtils.java:8197-8215` plus the shape helpers at
//! `TUlawUtils.java:85-117`; narrative: notes/JAVA-BEHAVIOR/B11_ulawutils.md.
//!
//! No TOML fixture: the 8192-entry table IS the ground truth and is
//! transcribed literally in `crates/nfm-core/src/modslayer.rs`.

use crate::mirror::harness as check;
use crate::mirror::util::crc32;
use crate::modslayer::{S2U_LUT, linear2ulaw_byte, linear2ulaw_int, linear2ulawclip};

pub(crate) fn run() {
    check::group("UlawUtils");
    clip_encoder();
    lut_entries();
    byte_overload();
    int_overload();
    table_shape();
}

/// `linear2ulawclip` (TUlawUtils.java:13-31): clamp to [-32767, 32767] then
/// `s2u_lut[v >> 3 & 8191]`.
fn clip_encoder() {
    check::eq_i32(linear2ulawclip(0) as i32, -1, "clip_0");
    check::eq_i32(linear2ulawclip(7) as i32, -1, "clip_7_floor");
    check::eq_i32(linear2ulawclip(8) as i32, -2, "clip_8");
    check::eq_i32(linear2ulawclip(1000) as i32, -50, "clip_1000");
    check::eq_i32(linear2ulawclip(1024) as i32, -51, "clip_1024");
    check::eq_i32(linear2ulawclip(31744) as i32, -128, "clip_31744");
    check::eq_i32(linear2ulawclip(32767) as i32, -128, "clip_32767");
    // unclamped, 32776>>3&8191 would index 4097 (0x00): the clamp pins it
    check::eq_i32(linear2ulawclip(32776) as i32, -128, "clip_upper_clamp");
    // the clamp bound is -32767, NOT -32768 (UlawUtils.java:8198-8199)
    check::eq_i32(linear2ulawclip(-32776) as i32, 0, "clip_lower_clamp");
    check::eq_i32(linear2ulawclip(-1) as i32, 126, "clip_neg1");
    check::eq_i32(linear2ulawclip(-8) as i32, 126, "clip_neg8_floor");
    check::eq_i32(linear2ulawclip(-9) as i32, 125, "clip_neg9");
    check::eq_i32(linear2ulawclip(-31744) as i32, 1, "clip_neg31744");
    check::eq_i32(linear2ulawclip(-32767) as i32, 0, "clip_neg32767");
    // the 128-entry zero plateau at indices 4096..4223
    check::eq_i32(linear2ulawclip(-32760) as i32, 0, "clip_zero_plateau");
}

/// Table properties (TUlawUtils.java:33-47): the segment seams and the
/// missing 0x7F code.
fn lut_entries() {
    check::eq_i32(S2U_LUT[0] as i32, -1, "lut_0");
    check::eq_i32(S2U_LUT[4095] as i32, -128, "lut_4095");
    check::eq_i32(S2U_LUT[4096] as i32, 0, "lut_4096");
    check::eq_i32(S2U_LUT[4223] as i32, 0, "lut_4223");
    check::eq_i32(S2U_LUT[4224] as i32, 1, "lut_4224");
    check::eq_i32(S2U_LUT[8191] as i32, 126, "lut_8191");
    check::eq_i32(S2U_LUT.len() as i32, 8192, "lut_len");

    let mut bad = 0;
    for &b in S2U_LUT.iter() {
        if b == 127 {
            bad += 1;
        }
    }
    check::eq_i32(bad, 0, "lut_no_0x7F");
}

/// The byte overload (TUlawUtils.java:49-57):
/// `s2u_lut[((b & 255) << 5) + 15]` — an unrelated index formula that never
/// reads [0..14] or [8176..8191].
fn byte_overload() {
    check::eq_i32(linear2ulaw_byte(0) as i32, -16, "byte_0");
    check::eq_i32(linear2ulaw_byte(1) as i32, -32, "byte_1");
    check::eq_i32(linear2ulaw_byte(15) as i32, -80, "byte_15");
    check::eq_i32(linear2ulaw_byte(127) as i32, -128, "byte_127");
    check::eq_i32(linear2ulaw_byte(-128) as i32, 0, "byte_neg128");
    check::eq_i32(linear2ulaw_byte(-1) as i32, 111, "byte_neg1");
    check::ok(
        linear2ulaw_byte(0) != linear2ulaw_int(0),
        "overloads_differ",
    );
}

/// The int overload (TUlawUtils.java:59-74): `s2u_lut[v >> 3]` with NO clamp.
fn int_overload() {
    check::eq_i32(linear2ulaw_int(0) as i32, -1, "int_0");
    check::eq_i32(linear2ulaw_int(7) as i32, -1, "int_7_floor");
    check::eq_i32(linear2ulaw_int(8) as i32, -2, "int_8");
    check::eq_i32(linear2ulaw_int(32767) as i32, -128, "int_32767");
    check::eq_i32(linear2ulaw_int(65535) as i32, 126, "int_65535");
    // Java: ArrayIndexOutOfBoundsException (index 8192 / index -1); the port
    // indexes the 8192-slice unguarded, so the bounds check panics with the
    // slice message. `as usize` on the arithmetic-shifted negative wraps
    // huge, so the same check fires for both out-of-range directions.
    check::throws_it(
        || {
            linear2ulaw_int(65536);
        },
        "index out of bounds",
        "int_65536_throws",
    );
    check::throws_it(
        || {
            linear2ulaw_int(-1);
        },
        "index out of bounds",
        "int_neg1_throws",
    );
}

/// The whole-table shape pins (TUlawUtils.java:76-117): descending -1..-128
/// half, a 128-entry zero plateau, ascending 1..126 half; 255 distinct byte
/// values; and a CRC-32 over every entry.
fn table_shape() {
    check::ok(is_shape(), "lut_shape");
    check::eq_i32(distinct_count(), 255, "lut_255_distinct");
    // java.util.zip.CRC32.getValue() is a long; same 32-bit value widened.
    check::eq_i64(table_crc() as i64, 2352583155, "lut_crc32");
}

/// `isShape` (TUlawUtils.java:85-102) verbatim, including the `4225` start
/// that skips index 4224 in the final ascending scan.
fn is_shape() -> bool {
    for i in 1..4096 {
        if S2U_LUT[i] > S2U_LUT[i - 1] {
            return false;
        }
    }
    for i in 4096..4224 {
        if S2U_LUT[i] != 0 {
            return false;
        }
    }
    for i in 4225..8192 {
        if S2U_LUT[i] < S2U_LUT[i - 1] {
            return false;
        }
    }
    true
}

/// `distinctCount` (TUlawUtils.java:104-110): `Set<Byte>` cardinality.
fn distinct_count() -> i32 {
    let mut seen = [false; 256];
    for &b in S2U_LUT.iter() {
        seen[b as u8 as usize] = true;
    }
    seen.iter().filter(|&&s| s).count() as i32
}

/// `tableCrc` (TUlawUtils.java:112-117): `CRC32.update(byte[])` over the
/// table. The CRC-32/ISO-HDLC transcription lives in
/// [`crc32`](crate::mirror::util::crc32); `update(byte[])` feeds each
/// entry's low 8 bits, so a signed byte is fed as its unsigned value.
fn table_crc() -> u32 {
    crc32(&S2U_LUT)
}
