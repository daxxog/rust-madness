// FIDELITY: TMedium.java pins iterate the Java fixture arrays by index to preserve literal ordering.
#![allow(clippy::needless_range_loop)]
//! `TMedium.java` — the Medium math core: field defaults, the ctor's
//! degree tables, the while-loop sin/cos normalization, `ys()`'s int
//! projection, `random()`'s 3-digit random walk, the fade machine
//! (`adjstfade`/`fadfrom`/`setfade`) and `addsp`'s 5-slot guard. Group
//! "Medium" (`Suite.java` position 3). Pin source:
//! `Medium.java:5-152,427-452,535-541`; narrative:
//! notes/JAVA-BEHAVIOR/B3_wheels_medium.md.
//!
//! Consumes `constants.toml [medium]` exactly like the Java class
//! (`TMedium.java:11`: one load, then `.table("medium")`).
//!
//! `random()`'s entropy is Java's single global `Math.random()`; the pinned
//! facts are structural (refill cadence, the ±1 walk, trn cycling, the
//! 10-value domain), never digits. The mirror drives the port's per-medium
//! seam with a seeded `java.util.Random` (the TJavaRandom mediumBridge
//! recipe) so the run is deterministic.

use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::random::{JavaRandom, RngSource};

pub(crate) fn run() {
    check::group("Medium");
    // TMedium.java:11 — the class-level fixture load. Values are normative;
    // the shape keys are the Java `requireKeys`.
    let k = Tml::load("constants.toml").table("medium");
    k.require_keys(&["focus_point", "ground", "fade0", "fade7", "addsp_slots"]);

    let mut m = Medium::new();
    defaults(&m, &k);
    trig_tables(&m);
    wrapping(&m);
    ys_pins(&m);
    random_walk(&mut m);
    fade_machine(&mut m, &k);
    addsp_guard(&mut m, &k);
}

/// Field defaults (Medium.java:5-47) — the Java instance is fresh, so every
/// TOML default is a direct ctor pin.
fn defaults(m: &Medium, k: &Tml) {
    check::eq_i32(m.focus_point, k.intv("focus_point"), "default_focus_point");
    check::eq_i32(m.ground, k.intv("ground"), "default_ground");
    check::eq_i32(m.skyline, k.intv("skyline"), "default_skyline");
    check::eq_i32(m.origfade, k.intv("fade0"), "default_origfade");
    check::eq_i32(m.cx, k.intv("cx"), "default_cx");
    check::eq_i32(m.cy, k.intv("cy"), "default_cy");
    check::eq_i32(m.cz, k.intv("cz"), "default_cz");
    check::eq_i32(m.vxz, k.intv("vxz"), "default_vxz");
    check::eq_i32(m.adv, k.intv("adv"), "default_adv");
    check::eq_i32(m.hit, k.intv("hit"), "default_hit");
    check::eq_i32(m.ptcnt, k.intv("ptcnt"), "default_ptcnt");
    check::eq_i32(m.w, k.intv("w"), "default_w");
    check::eq_i32(m.h, k.intv("h"), "default_h");
    check::eq_i32(m.nsp, 0, "default_nsp");
    check::eq_i32(m.cntrn, 0, "default_cntrn");
    check::eq_i32(m.trn as i32, 0, "default_trn");
    check::eq_i32(m.fade.len() as i32, 8, "default_fade_len");
    check::eq_i32(m.fade[0], k.intv("fade0"), "default_fade0");
    check::eq_i32(m.fade[7], k.intv("fade7"), "default_fade7");
    check::eq_i32(m.csky[0], k.intv("csky0"), "default_csky0");
    check::eq_i32(m.csky[2], k.intv("csky2"), "default_csky2");
    check::eq_i32(m.cgrnd[0], k.intv("cgrnd0"), "default_cgrnd0");
    check::eq_i32(m.cgrnd[2], k.intv("cgrnd2"), "default_cgrnd2");
    check::eq_i32(m.cfade[0], k.intv("cfade0"), "default_cfade0");
    check::eq_i32(m.cfade[1], k.intv("cfade1"), "default_cfade1");
}

/// The ctor's tables are the double-precision degree formula, float-cast
/// (Medium.java:111-123): exact quarter-turns plus the formula pin for the
/// inexact ones.
fn trig_tables(m: &Medium) {
    check::eq_i32(bits(m.tcos[0]), bits(1.0), "tcos_0");
    check::eq_i32(bits(m.tsin[0]), bits(0.0), "tsin_0");
    check::eq_i32(bits(m.tsin[90]), bits(1.0), "tsin_90");
    check::eq_i32(bits(m.tsin[270]), bits(-1.0), "tsin_270");
    check::eq_i32(bits(m.tcos[180]), bits(-1.0), "tcos_180");
    // Java `(float)Math.cos(90 * (Math.PI / 180.0))`: the product is a
    // double, cos is double, then the float cast (Rust f64 -> f32 is IEEE
    // round-to-nearest, the same cast).
    check::eq_i32(
        bits(m.tcos[90]),
        bits((90.0f64 * (core::f64::consts::PI / 180.0)).cos() as f32),
        "tcos_90_formula",
    );
    check::eq_i32(
        bits(m.tsin[180]),
        bits((180.0f64 * (core::f64::consts::PI / 180.0)).sin() as f32),
        "tsin_180_formula",
    );
}

/// sin()/cos() normalize with while loops, not modulo (Medium.java:99-109,
/// 554-564).
fn wrapping(m: &Medium) {
    check::eq_i32(bits(m.sin(390)), bits(m.sin(30)), "sin_390_wraps");
    check::eq_i32(bits(m.sin(-30)), bits(m.sin(330)), "sin_neg30_wraps");
    check::eq_i32(bits(m.cos(-90)), bits(m.cos(270)), "cos_neg90_wraps");
    check::eq_i32(bits(m.sin(720 + 45)), bits(m.sin(45)), "sin_765_wraps");
}

/// ys(): int arithmetic, truncating division, d<10 clamp
/// (Medium.java:91-97).
fn ys_pins(m: &Medium) {
    check::eq_i32(m.ys(100, 50), -600, "ys_100_50");
    check::eq_i32(m.ys(0, 50), -1400, "ys_0_50");
    check::eq_i32(m.ys(5, 10), -7600, "ys_5_10");
    check::eq_i32(m.ys(0, 9), -7800, "ys_d_clamped");
    // truncation toward zero: -34900/51 = -684.31 -> -684
    check::eq_i32(m.ys(100, 51), -584, "ys_truncates_toward_zero");
}

/// random(): the 3-digit random-walk structure (Medium.java:49-89). The
/// Java class reads the global unseeded `Math.random()`; the port's seam
/// takes a seeded `java.util.Random`, which makes the run reproducible
/// without changing any pinned fact.
fn random_walk(m: &mut Medium) {
    m.rng = Some(RngSource::Java(JavaRandom::new(11)));

    m.random();
    // call 1 refills: cntrn 0 -> 20, trn 0 -> 1
    check::eq_i32(m.cntrn, 20, "random_first_call_refills");
    check::eq_i32(m.trn as i32, 1, "random_trn_1");
    m.random();
    // call 2 only decrements
    check::eq_i32(m.cntrn, 19, "random_second_call_decrements");
    check::eq_i32(m.trn as i32, 2, "random_trn_2");
    m.random();
    check::eq_i32(m.trn as i32, 0, "random_trn_cycles");

    let before = m.rand;
    m.random();
    for i in 0..3 {
        let delta = (m.rand[i] - before[i] + 10) % 10;
        check::ok(delta == 1 || delta == 9, &format!("random_walks_slot{i}"));
    }

    let mut domain = true;
    for _ in 0..25 {
        let v = m.random();
        let mut in_domain = false;
        for k in 0..10 {
            if v == k as f32 / 10.0 {
                in_domain = true;
            }
        }
        if !in_domain {
            domain = false;
        }
    }
    check::ok(domain, "random_domain_10_values");

    // cadence (probe-verified): refill at call 1 and every 21 calls after —
    // 29 calls in, the last refill was call 22 -> cntrn 13; the next refill
    // lands at call 43
    check::eq_i32(m.cntrn, 13, "random_cntrn_after_29_calls");
    for _ in 0..14 {
        m.random();
    }
    check::eq_i32(m.cntrn, 20, "random_refills_every_21_calls");
}

/// The fade machine: adjstfade's guard/growth/low branches
/// (Medium.java:427-443), fadfrom's argument-times-(i+1) ladder (:535-541)
/// and setfade's snap blend (:125-152).
fn fade_machine(m: &mut Medium, k: &Tml) {
    // the fade[0] != origfade guard makes the growth arm a no-op when
    // already at rest
    m.adjstfade(20.0);
    check::eq_i32(m.fade[0], k.intv("fade_floor"), "adjstfade_guard_noop");
    check::eq_i32(m.fade[7], k.intv("fade7"), "adjstfade_guard_noop_fade7");
    m.fade[0] = 2000;
    m.adjstfade(20.0);
    check::eq_i32(
        m.fade[0],
        2000 + k.intv("fade_step"),
        "adjstfade_growth_500",
    );
    check::eq_i32(m.fade[1], 5000, "adjstfade_growth_fadfrom");
    check::eq_i32(m.fade[7], 20000, "adjstfade_growth_fadfrom7");
    m.origfade = 6000;
    m.adjstfade(10.0);
    // 6000 - 1000*(15-10) = 1000 -> clamped to 3000, then fadfrom
    check::eq_i32(m.fade[0], k.intv("fade_floor"), "adjstfade_low_clamped");
    m.adjstfade(14.9);
    // float artifact: 14.9F = 14.8999996..., so 15-14.9F > 0.1 and
    // 6000 - 1000.0F*0.10000004 = 5899.9999 -> (int) truncates to 5899,
    // NOT 5900 (the gate caught the naive expectation)
    check::eq_i32(m.fade[0], 5899, "adjstfade_low_float_truncation");
    check::eq_i32(m.fade[1], 11798, "adjstfade_low_fadfrom");

    m.fadfrom(100);
    check::eq_i32(m.fade[0], 100, "fadfrom_0");
    check::eq_i32(m.fade[3], 400, "fadfrom_3");
    check::eq_i32(m.fade[7], 800, "fadfrom_7");

    // setfade blends each channel with snap[i]/100 then clamps; snap
    // defaults 0 -> exact copy
    m.setfade(10, 20, 30);
    check::eq_i32(m.cfade[0], 10, "setfade_snap0");
    check::eq_i32(m.cfade[1], 20, "setfade_snap1");
    check::eq_i32(m.cfade[2], 30, "setfade_snap2");
    m.snap[0] = 50;
    m.snap[1] = 200;
    m.snap[2] = 0;
    m.setfade(100, 100, 100);
    check::eq_i32(m.cfade[0], 150, "setfade_snap50");
    check::eq_i32(m.cfade[1], 255, "setfade_snap200_clamped");
    check::eq_i32(m.cfade[2], 100, "setfade_snap0b");
}

/// addsp: 5 slots, the 6th add is a no-op (Medium.java:445-452).
fn addsp_guard(m: &mut Medium, k: &Tml) {
    m.addsp(1, 2, 3);
    m.addsp(4, 5, 6);
    m.addsp(7, 8, 9);
    m.addsp(10, 11, 12);
    m.addsp(13, 14, 15);
    check::eq_i32(m.nsp, k.intv("addsp_slots"), "addsp_full");
    m.addsp(99, 99, 99);
    check::eq_i32(m.nsp, k.intv("addsp_slots"), "addsp_guard");
    check::eq_i32(m.spx[4], 13, "addsp_slot4_untouched");
    check::eq_i32(m.spx[0], 1, "addsp_slot0");
}
