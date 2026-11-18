// FIDELITY: TJavaRandom.java vectors use Java-style indexed loops and the literal `live >= 0.0 && live < 1.0` [0,1) check.
#![allow(clippy::needless_range_loop, clippy::manual_range_contains)]
//! `TJavaRandom.java` — the Javadoc-specified LCG vectors, the Math slot
//! seam and the Medium bridge, as literal value pins (group "Rng"). These
//! vectors are the cross-language pin material: the port's `JavaRandom`
//! and the Medium injection seam must reproduce every literal here.

use crate::medium::Medium;
use crate::mirror::harness as check;
use crate::random::{self, JavaRandom, RngSource};

pub(crate) fn run() {
    check::group("Rng");
    lcg_vectors();
    reseed_repeat();
    math_injection();
    medium_bridge();
    uninstall_contract();
}

/// Random(42): doubles (bit-exact), nextInt(100), the power-of-two and
/// rejection nextInt paths, nextBoolean and nextLong.
fn lcg_vectors() {
    let mut d = JavaRandom::new(42);
    let doubles: [i64; 8] = [
        0x3fe7_4833_a06f_f457, // 0.7275636800328681
        0x3fe5_dcf7_7862_2e01, // 0.6832234717598454
        0x3fd3_c20f_3f12_bbb4, // 0.30871945533265976
        0x3fd1_bba7_6b52_c856, // 0.27707849007413665
        0x3fe5_4c2d_50bb_0864, // 0.6655489517945736
        0x3fec_e86c_f39c_2cbe, // 0.9033722646721782
        0x3fd7_9a23_a61b_35c8, // 0.36878291341130565
        0x3fd1_a5db_3b0b_fbe2, // 0.2757480694417024
    ];
    for i in 0..doubles.len() {
        check::eq_i64(
            d.next_double().to_bits() as i64,
            doubles[i],
            &format!("lcg_next_double_bits_{i}"),
        );
    }

    let mut hi = JavaRandom::new(42);
    let hundreds = [30, 63, 48, 84, 70, 25, 5, 18];
    for i in 0..hundreds.len() {
        check::eq_i32(
            hi.next_int(100),
            hundreds[i],
            &format!("lcg_next_int_100_{i}"),
        );
    }

    // 16 is a power of two: the (bound * next(31)) >> 31 arm
    check::eq_i32(JavaRandom::new(42).next_int(16), 11, "lcg_next_int_16_pow2");
    // 1000000000 is not a power of two: the rejection arm
    check::eq_i32(
        JavaRandom::new(42).next_int(1_000_000_000),
        562431130,
        "lcg_next_int_1e9",
    );

    let mut b = JavaRandom::new(42);
    let booleans = [true, false, true, false];
    for i in 0..booleans.len() {
        check::eq_bool(
            b.next_boolean(),
            booleans[i],
            &format!("lcg_next_boolean_{i}"),
        );
    }

    let mut l = JavaRandom::new(42);
    check::eq_i64(l.next_long(), -5025562857975149833, "lcg_next_long_0");
    check::eq_i64(l.next_long(), -5843495416241995736, "lcg_next_long_1");
}

/// setSeed(42) on an already-advanced Random(42) restarts the stream: the
/// next doubles are again lcg_next_double_bits_0/1.
fn reseed_repeat() {
    let mut r = JavaRandom::new(42);
    for _ in 0..20 {
        r.next_double();
    }
    r.set_seed(42);
    check::eq_i64(
        r.next_double().to_bits() as i64,
        0x3fe7_4833_a06f_f457,
        "reseed_next_double_bits_0",
    );
    check::eq_i64(
        r.next_double().to_bits() as i64,
        0x3fe5_dcf7_7862_2e01,
        "reseed_next_double_bits_1",
    );
}

/// Pointing Math's generator at Random(7) makes math_random() emit
/// exactly Random(7).nextDouble(); two values are also pinned as literals
/// so the seam is strong without the local reference.
fn math_injection() {
    let _guard = random::install(7);
    let mut r = JavaRandom::new(7);
    let mut first = 0f64;
    let mut second = 0f64;
    for i in 0..5 {
        let got = random::math_random();
        let want = r.next_double();
        check::eq_i64(
            got.to_bits() as i64,
            want.to_bits() as i64,
            &format!("math_injection_call_{i}"),
        );
        if i == 0 {
            first = got;
        }
        if i == 1 {
            second = got;
        }
    }
    check::eq_i64(
        first.to_bits() as i64,
        0x3fe7_61e2_f51b_b9a0,
        "math_injection_first_bits_pin",
    );
    check::eq_i64(
        second.to_bits() as i64,
        0x3fe7_f932_8813_b859,
        "math_injection_second_bits_pin",
    );
} // guard drops -> uninstall

/// Medium.random() (Medium.java:49-89) draws ONLY from the injected
/// stream, so its 5-call output vector is a pure function of the seed
/// (the port: the per-medium `rng` seam, the walk unchanged). Seeding
/// twice must replay it; a different seed must not.
fn medium_bridge() {
    let seed11 = draw_medium_vector(11);
    let seed11_again = draw_medium_vector(11);
    for i in 0..5 {
        check::eq_i32(
            seed11_again[i].to_bits() as i32,
            seed11[i].to_bits() as i32,
            &format!("medium_bridge_reseed_same_{i}"),
        );
    }

    // literal pin of the seed-11 vector (the Rust mirror's target)
    let want11: [i32; 5] = [1063675494, 1045220557, 0, 1058642330, 1063675494];
    for i in 0..5 {
        check::eq_i32(
            seed11[i].to_bits() as i32,
            want11[i],
            &format!("medium_bridge_seed11_bits_{i}"),
        );
    }

    // a different seed must give a different vector (seed sensitivity)
    let seed12 = draw_medium_vector(12);
    let want12: [i32; 5] = [0, 1056964608, 0, 1050253722, 1061997773];
    for i in 0..5 {
        check::eq_i32(
            seed12[i].to_bits() as i32,
            want12[i],
            &format!("medium_bridge_seed12_bits_{i}"),
        );
    }
    check::ok(seed11 != seed12, "medium_bridge_seed12_vector_differs");
}

/// Five Medium.random() draws from a fresh Medium pointed at Random(seed)
/// — the per-medium stand-in for TestRng.install(seed) + new Medium().
fn draw_medium_vector(seed: i64) -> [f32; 5] {
    let mut m = Medium::new();
    m.rng = Some(RngSource::Java(JavaRandom::new(seed)));
    let mut out = [0f32; 5];
    for v in &mut out {
        *v = m.random();
    }
    out
}

/// After uninstall() the slot must hold the FIRST saved original again,
/// not the injected instance. TestRng asserts this by reflection identity;
/// the port's seam is a scoped slot source, so the pins assert the
/// observables (PHASE-B ledger: mechanism differs, intent preserved).
fn uninstall_contract() {
    // During install the slot is the injected generator (proven by the
    // math_injection pins above); after the guard drops it is the default
    // again — field != injected (id 1), and a second install/drop cycle
    // restores it again (id 2: uninstall_restores_saved_original).
    {
        let _g = random::install(12345);
    }
    check::ok(
        random::current_is_default(),
        "uninstall_field_no_longer_injected",
    );
    {
        let _g = random::install(7);
    }
    check::ok(
        random::current_is_default(),
        "uninstall_restores_saved_original",
    );

    // The restored generator is the default deterministic one (the
    // "saved original is a Random" identity): its draws sit on the
    // xorshift32 k/65536 grid, which an injected LCG stream never does.
    let v = random::math_random();
    check::ok(grid16(v), "uninstall_saved_original_is_random");

    // And it is still live: a draw in [0, 1).
    let live = random::math_random();
    check::ok(
        live >= 0.0 && live < 1.0,
        "uninstall_math_random_still_live",
    );
}

/// True when v is on the k/65536 grid (the xorshift32 draw shape).
fn grid16(v: f64) -> bool {
    (v * 65536.0).fract() == 0.0
}
