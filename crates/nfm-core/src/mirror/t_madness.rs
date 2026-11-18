//! `TMadness.java` — Madness's zero-fixture surface: the squared-distance
//! helpers (`py` int wrap, `rpy` float truncation), the `rot` table
//! rotation, and the ctor's field defaults including the `power == 75` vs
//! `reseto`'s 98 distinction. Group "Madness" (`Suite.java` position 5).
//! Pin source: `Madness.java:118-120,286-288,372-381,279-284,7-116`;
//! narrative: notes/JAVA-BEHAVIOR/B9_madness.md.
//!
//! Consumes `constants.toml [madness]` exactly like the Java class
//! (`TMadness.java:10`: one load, then `.table("madness")`).
//!
//! Two Java-only shapes are mapped, both documented at their pins:
//! the ctor's null `Medium`/`Record`/`xt` refs (the port takes collaborators
//! per call and `new()` takes only `im`) and the instance `drag` table (the
//! port shares it as `madness::DRAG`; nothing but `moment` is ever written).

use crate::madness::{DRAG, Madness};
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;

pub(crate) fn run() {
    check::group("Madness");
    // TMadness.java:10 — the class-level fixture load; the shape keys are
    // the Java `requireKeys` (:14).
    let k = Tml::load("constants.toml").table("madness");
    k.require_keys(&["power_default", "power_reset", "power_decay_divisor"]);

    py_pins();
    rpy_pins();
    rot_pins();
    ctor_defaults(&k);
}

/// `py` (Madness.java:118-120): int squares wrap; `new Madness(null,...)`
/// carries no collaborator because `py` reads no field, so the port's
/// `Madness::new(im)` is the same object.
fn py_pins() {
    check::eq_i32(Madness::new(0).py(3, 1, 5, 2), 13, "py_basic");
    check::eq_i32(
        Madness::new(0).py(0, -100_000, 0, 0),
        1_410_065_408,
        "py_int_overflow_wraps",
    );
    check::eq_i32(Madness::new(0).py(-1, 0, -1, 0), 2, "py_negatives");
}

/// `rpy` (Madness.java:286-288): float squares summed left to right, then
/// `(int)` truncated toward zero.
fn rpy_pins() {
    let m = Madness::new(0);
    check::eq_i32(
        m.rpy(0.0, 1.5, 0.0, 0.0, 0.0, 0.0),
        2,
        "rpy_truncates_toward_zero",
    );
    check::eq_i32(m.rpy(10.0, 0.0, 0.0, 0.0, 0.0, 0.0), 100, "rpy_symmetric_a");
    check::eq_i32(m.rpy(0.0, 10.0, 0.0, 0.0, 0.0, 0.0), 100, "rpy_symmetric_b");
    check::eq_i32(m.rpy(0.5, 0.0, 0.0, 0.0, 0.0, 0.0), 0, "rpy_fraction_drops");
    check::eq_i32(m.rpy(3.0, 0.0, 4.0, 0.0, 0.0, 0.0), 25, "rpy_3_4_5");
}

/// `rot` (Madness.java:372-382): angle 0 is a no-op that never touches the
/// Medium (Java passes a null `m`; the port's seam takes `&Medium`, so a
/// fresh one stands in — the pin is the absence of writes), and 90 degrees
/// rotates through the sin/cos tables.
fn rot_pins() {
    let med = Medium::new();
    let mnull = Madness::new(0);
    let mut a = [10.0f32];
    let mut b = [0.0f32];
    mnull.rot(&med, &mut a, &mut b, 0, 0, 0, 1);
    check::eq_i32(bits(a[0]), bits(10.0), "rot_angle0_noop_a");
    check::eq_i32(bits(b[0]), bits(0.0), "rot_angle0_noop_b");

    let mrot = Madness::new(0);
    let mut ra = [10.0f32, 0.0];
    let mut rb = [0.0f32, 10.0];
    mrot.rot(&med, &mut ra, &mut rb, 0, 0, 90, 2);
    check::eq_i32(
        bits(ra[0]),
        bits(10.0f32 * med.tcos[90]),
        "rot_90_a_uses_tcos",
    );
    check::eq_i32(
        bits(rb[0]),
        bits(10.0f32 * med.tsin[90]),
        "rot_90_b_uses_tsin",
    );
    // second point: (0,10) rotated 90deg -> (-10*sin, 10*cos)
    check::eq_i32(
        bits(ra[1]),
        bits(-10.0f32 * med.tsin[90]),
        "rot_90_a_point2",
    );
    check::eq_i32(bits(rb[1]), bits(10.0f32 * med.tcos[90]), "rot_90_b_point2");
}

/// The ctor (Madness.java:279-284) writes only its four refs/index; every
/// other field keeps its initializer (:7-116) — `power` is 75.0F here and
/// `reseto` installs 98 later (:639), which `drive` handicaps on (:934).
fn ctor_defaults(k: &Tml) {
    let md = Madness::new(4);
    check::eq_i32(md.im as i32, 4, "ctor_im");
    check::eq_i32(md.cn as i32, 0, "ctor_cn");
    check::eq_i32(
        bits(md.power),
        bits(k.floatv("power_default")),
        "ctor_power_75_not_98",
    );
    check::eq_i32(md.focus, -1, "ctor_focus");
    check::eq_i32(bits(md.speed), bits(0.0), "ctor_speed");
    check::ok(!md.mtouch, "ctor_mtouch");
    check::ok(!md.wtouch, "ctor_wtouch");
    check::eq_i32(md.loop_, 0, "ctor_loop");
    // Java reads the instance copy `drag[0]`; the port shares the table
    // (only `moment` is ever written, see the module comment in madness.rs).
    check::eq_i32(bits(DRAG[0]), bits(0.5), "ctor_drag");
    check::eq_i32(md.point, 0, "ctor_point");
    check::eq_i32(md.nlaps, 0, "ctor_nlaps");
}
