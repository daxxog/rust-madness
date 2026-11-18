//! MD5 (RFC 1321) for the port's digest pins.
//!
//! One implementation for every pin site, in both shapes the callers need:
//! [`md5`] returns the raw 16 bytes (the jpeg/roundrect/xtgraphics pixel
//! batteries compare them directly) and [`md5_hex`] the lowercase hex string
//! (the mirror's frame/audio digests, the JDK `MessageDigest` shape).
//!
//! Test-only by construction: `lib.rs` declares the module `#[cfg(test)]`,
//! because production never hashes and the crate deliberately carries no
//! hashing dependency. The pinned digests themselves are the correctness
//! proof — a wrong implementation fails them loudly.

/// The per-round left-rotation amounts (RFC 1321 §3.4).
const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, //
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, //
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, //
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// The sine-derived round constants `floor(2^32 * abs(sin(i + 1)))`, i.e.
/// the table form of the RFC's `T[i]` (RFC 1321 §3.4).
const K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee, //
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501, //
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be, //
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821, //
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa, //
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8, //
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed, //
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a, //
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c, //
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70, //
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05, //
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665, //
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039, //
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1, //
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1, //
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

/// The MD5 digest of `data` (RFC 1321), little-endian words like every
/// reference implementation.
pub(crate) fn md5(data: &[u8]) -> [u8; 16] {
    // Padding (§3.1-3.2): 0x80, zeros to 56 mod 64, then the bit length as a
    // little-endian u64.
    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());

    let (mut a0, mut b0, mut c0, mut d0) = (
        0x6745_2301u32,
        0xefcd_ab89u32,
        0x98ba_dcfeu32,
        0x1032_5476u32,
    );
    for chunk in msg.as_chunks::<64>().0 {
        let mut m = [0u32; 16];
        for (i, wi) in m.iter_mut().enumerate() {
            *wi = u32::from_le_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64usize {
            let (f, g) = if i < 16 {
                ((b & c) | (!b & d), i)
            } else if i < 32 {
                ((d & b) | (!d & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | !d), (7 * i) % 16)
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    for (i, word) in [a0, b0, c0, d0].iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// The digest as lowercase hex — the JDK `MessageDigest` string shape the
/// mirror's pins compare against.
pub(crate) fn md5_hex(data: &[u8]) -> String {
    md5(data).iter().map(|b| format!("{b:02x}")).collect()
}
