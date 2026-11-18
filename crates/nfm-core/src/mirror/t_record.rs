//! `TRecord.java` — Record, the ghost/replay recorder: the recorder window
//! and ring shape, the `recx`/`recz` cross-indexed write quirk, the frame
//! shift and insert, the dest-after-decrement 230 gate, `play`/`playh`'s
//! placement + embos + part loops, the `reg*`/`chip*` damage replay (both
//! the geometry pushes and the HSB damage tint), `cotchinow`'s h-copy,
//! `reset`'s once-per-run `starcar` snapshot and `py`'s wrapping int math.
//! Group "Record" (`Suite.java` position 11 — the campaign's largest single
//! class: 496 `Check` sites, 532 runtime pins). Pin source: `Record.java`
//! (whole class) via `tests/java/TRecord.java`; narrative:
//! notes/JAVA-BEHAVIOR/B4_record.md.
//!
//! Consumes `constants.toml [record]` (the recorder window + ring shape,
//! `TRecord.java:43-47`) and `determinism.toml [vectors].record_tenths`
//! (the replay's fixed tenths stream, `TRecord.java:1703-1704`).
//!
//! Port-shape mapping (documented at each pin):
//! - Java's `Record.m` reference is a per-call argument in the port, so the
//!   ctor's identity pin is the compile-time identity (`ctor_m_identity`);
//! - the `TenthsMedium` subclass override (`TRecord.java:1702-1712`) is the
//!   port's [`crate::random::RngSource::Vector`] seam on `Medium.rng`, and
//!   its `draws` counter reads that vector's cursor (equal to the Java count
//!   while a group stays inside one 12-entry cycle — every pinned group here
//!   consumes at most 6 entries), so the Java class's `TestRng.install`
//!   scoping has no observable counterpart and is not replayed;
//! - `ContO.npl`/`ContO.p` are the port's `planes` vector: `npl` is
//!   `planes.len()` and `p[i]` is `planes[i]`;
//! - `ContO.maxR` is `Conto.maxr`; Java `null` array/instance slots are the
//!   port's `None` (`car[..]`, `ocar[..]`, `starcar[..]`) or empty vectors.

use std::panic::AssertUnwindSafe;

use crate::conto::{Base, Conto};
use crate::madness::Madness;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::plane::Plane;
use crate::random::RngSource;
use crate::record::Record;
use crate::trackers::Trackers;

/// The Java class's `KR` bindings (`TRecord.java:43-47`).
struct K {
    cntf: i32,
    frames: i32,
    ring: i32,
    ring_last: i32,
    vec: Vec<f32>,
}

pub(crate) fn run() {
    check::group("Record");
    // TRecord.java:43-51 — the constants load, the shape keys, and the
    // replay-vector length pin (the replay period is pinned behavior).
    let kr = Tml::load("constants.toml").table("record");
    kr.require_keys(&["cntf", "ring_slots", "ring_last", "frames", "hist_slot"]);
    let k = K {
        cntf: kr.intv("cntf"),
        frames: kr.intv("frames"),
        ring: kr.intv("ring_slots"),
        ring_last: kr.intv("ring_last"),
        vec: Tml::load("determinism.toml")
            .table("vectors")
            .float_arr("record_tenths"),
    };
    check::eq_i32(k.vec.len() as i32, 12, "tenths_vector_len_12");

    ctor_defaults(&k);
    ring_quirk();
    ring_wraps(&k);
    rec_frames(&k);
    rec_history();
    rec_spark(&k);
    rec_dest_230(&k);
    play_frames();
    play_gates();
    play_parts(&k);
    playh_frames();
    playh_lastfr(&k);
    regx_group(&k);
    regy_group(&k);
    regz_group(&k);
    chip_group(&k);
    reg_tint(&k);
    cotchinow_group();
    reset_group();
    py_group();
}

fn madness(im: usize) -> Madness {
    // `new Madness(m, r, null, 1)`: the port's per-car args are per-call, so
    // only `cn`/`im` are observable (Record reads `clrad[cn]`, `cn`,
    // `flipy[cn]`, `msquash[cn]`, `im`).
    let mut md = Madness::new(im);
    md.cn = 0;
    md
}

/// `TRecord.car` (`TRecord.java:1609-1636`) — the empty-byte `ContO` seam:
/// `new ContO(new byte[0], m, new Trackers())` runs the line parser over an
/// empty stream, allocating `Plane[120]` and leaving `npl == 0` with every
/// placement field at its initializer. The fixture re-states the fields the
/// recorder reads; `disline`/`grounded` are the two non-zero `ContO`
/// initializers (`ContO.java:21-25`) that the port's test-support
/// `Base::default()` does not carry, so they are set here.
fn car(m: &Medium) -> Conto {
    let mut c = Conto::place(&Base::default(), m, &mut Trackers::new(), 0, 0, 0, 0);
    c.disline = 7;
    c.grounded = 1.0;
    c.planes.clear(); // npl = 0
    c.x = 0;
    c.y = 0;
    c.z = 0;
    c.xz = 0;
    c.xy = 0;
    c.zy = 0;
    c.wxz = 0;
    c.wzy = 0;
    c.dist = 0;
    c.maxr = 0;
    c.grat = 0;
    for i in 0..4 {
        c.keyx[i] = 0;
        c.keyz[i] = 0;
        c.scx[i] = 0;
        c.scz[i] = 0;
        c.stg[i] = 0;
        c.sx[i] = 0;
        c.sy[i] = 0;
        c.sz[i] = 0;
        c.smag[i] = 0.0;
        c.fulls[i] = false;
    }
    c
}

const ZERO_OY: [i32; 3] = [0, 0, 0];

/// `TRecord.planed` (`TRecord.java:1644-1650`) — one 3-point plane whose
/// colour channels are all equal, so `Plane` marks it `nocol`
/// (`Plane.java:132-134`): the `reg*` HSB tint branch is unreachable.
fn planed(m: &Medium, oy: [i32; 3], wz: i32) -> Conto {
    let mut c = car(m);
    c.planes.push(Plane::new(
        m,
        &[0, 10, 0],
        &[0, 0, 10],
        &oy,
        3,
        [1, 1, 1],
        false,
        0,
        0,
        0,
        wz,
        7,
        0,
        false,
    ));
    c
}

/// `TRecord.tinted` (`TRecord.java:1654-1660`) — a plane whose colour
/// channels differ, so the `reg*` tint branch is live.
fn tinted(m: &Medium, oc: [i32; 3], bfase: i32, glass: bool, gr: i32) -> Conto {
    let mut c = car(m);
    c.planes.push(Plane::new(
        m,
        &[0, 10, 0],
        &[0, 0, 10],
        &ZERO_OY,
        3,
        oc,
        glass,
        gr,
        0,
        0,
        0,
        7,
        bfase,
        false,
    ));
    c
}

/// `TRecord.five` (`TRecord.java:1664-1671`) — five empty-byte `ContO`s with
/// distinct `maxR`, the size `reset()` indexes.
fn five(m: &Medium) -> Vec<Conto> {
    let mut a = Vec::new();
    for i in 0..5 {
        let mut c = car(m);
        c.maxr = 30 + i;
        a.push(c);
    }
    a
}

/// The `TenthsMedium` stand-in (`TRecord.java:1702-1712`): `random()`
/// REPLACED by the measured tenths vector, the Java subclass override
/// expressed through the port's `Medium.rng` vector seam.
struct Tenths {
    m: Medium,
}

impl Tenths {
    fn new(vec: &[f32]) -> Tenths {
        let mut m = Medium::new();
        m.rng = Some(RngSource::Vector(vec.to_vec(), 0));
        Tenths { m }
    }

    /// `TenthsMedium.draws` — the vector cursor. The Java counter is exact;
    /// the cursor equals it while the group consumes fewer than `VEC.length`
    /// entries, which every pinned group here does (max 6 of 12).
    fn draws(&self) -> i32 {
        match &self.m.rng {
            Some(RngSource::Vector(_, i)) => *i as i32,
            _ => 0,
        }
    }
}

/// `TRecord.ctorDefaults` (`TRecord.java:79-160`, `Record.java:4-76`,
/// `:281-285`): the declared state, the ctor's inert `cotchinow(0)` and the
/// `[300][5]` / `[5][4][30]` / `[5][4][7]` shapes.
fn ctor_defaults(k: &K) {
    let r = Record::new();
    // Java `r.m == m`: the port takes the Medium per call, so the stored
    // reference is the compile-time identity at every call site.
    check::ok(true, "ctor_m_identity");
    check::eq_i32(r.caught, 0, "ctor_caught_0");
    check::ok(!r.hcaught, "ctor_hcaught_false");
    check::ok(r.prepit, "ctor_prepit_true");
    check::eq_i32(r.cntf, k.cntf, "ctor_cntf_50");
    check::eq_i32(r.lastfr, 0, "ctor_lastfr_0");
    check::eq_i32(r.wasted, 0, "ctor_wasted_0");
    check::eq_i32(r.whenwasted, 0, "ctor_whenwasted_0");
    check::eq_i32(r.powered, 0, "ctor_powered_0");
    // :284 cotchinow(0) with caught==0: no starcar, no h transfer
    check::ok(r.starcar[0].is_none(), "ctor_cotchinow_no_starcar");
    for i in 0..5 {
        check::eq_i32(r.hfix[i], -1, &format!("ctor_hfix{i}_minus1"));
        check::eq_i32(r.hdest[i], -1, &format!("ctor_hdest{i}_minus1"));
    }

    // declared shapes (:8-75)
    check::eq_i32(r.ocar.len() as i32, 5, "shape_ocar_5");
    check::eq_i32(r.car.len() as i32, 6, "shape_car_6");
    check::eq_i32(r.car[5].len() as i32, 5, "shape_car_row_5");
    check::eq_i32(r.squash.len() as i32, 6, "shape_squash_6");
    check::eq_i32(r.squash[5].len() as i32, 5, "shape_squash_row_5");
    check::eq_i32(r.fix.len() as i32, 5, "shape_fix_5");
    check::eq_i32(r.dest.len() as i32, 5, "shape_dest_5");
    check::eq_i32(r.cntdest.len() as i32, 5, "shape_cntdest_5");
    check::eq_i32(r.starcar.len() as i32, 5, "shape_starcar_5");
    check::eq_i32(r.hsquash.len() as i32, 5, "shape_hsquash_5");
    check::eq_i32(r.x.len() as i32, k.frames, "shape_x_300");
    check::eq_i32(r.x[k.frames as usize - 1].len() as i32, 5, "shape_x_row_5");
    check::eq_i32(r.y.len() as i32, k.frames, "shape_y_300");
    check::eq_i32(r.hx.len() as i32, k.frames, "shape_hx_300");
    check::eq_i32(r.ns.len() as i32, 5, "shape_ns_5x4");
    check::eq_i32(r.ns[4].len() as i32, 4, "shape_ns_part_4");
    check::eq_i32(r.sspark.len() as i32, 5, "shape_sspark_5");
    check::eq_i32(r.sspark[4].len() as i32, 4, "shape_sspark_part_4");
    check::eq_i32(r.sspark[4][3].len() as i32, 30, "shape_sspark_slot_30");
    check::eq_i32(r.smag.len() as i32, 5, "shape_smag_5x4x30");
    check::eq_i32(r.fulls[4][3].len() as i32, 30, "shape_fulls_slot_30");
    check::eq_i32(r.nry.len() as i32, 5, "shape_nry_5x4");
    check::eq_i32(r.nry[4].len() as i32, 4, "shape_nry_part_4");
    check::eq_i32(r.rx.len() as i32, 5, "shape_rx_5");
    check::eq_i32(r.rx[4].len() as i32, 4, "shape_rx_part_4");
    check::eq_i32(r.rx[4][3].len() as i32, k.ring, "shape_rx_slot_7");
    check::eq_i32(r.magy.len() as i32, 5, "shape_magy_5x4x7");
    check::eq_i32(r.mtouch.len() as i32, 5, "shape_mtouch_5");
    check::eq_i32(r.mtouch[4].len() as i32, k.ring, "shape_mtouch_slot_7");
    check::eq_i32(r.hmtouch.len() as i32, 5, "shape_hmtouch_5x7");
    check::eq_i32(r.hry[4][3].len() as i32, k.ring, "shape_hry_slot_7");

    let last = k.frames as usize - 1;
    // zero / false defaults at both ends of every array family
    check::eq_i32(r.x[0][0], 0, "zero_x_first");
    check::eq_i32(r.x[last][4], 0, "zero_x_last");
    check::eq_i32(r.y[last][4], 0, "zero_y_last");
    check::eq_i32(r.z[last][4], 0, "zero_z_last");
    check::eq_i32(r.xy[last][4], 0, "zero_xy_last");
    check::eq_i32(r.zy[last][4], 0, "zero_zy_last");
    check::eq_i32(r.xz[last][4], 0, "zero_xz_last");
    check::eq_i32(r.wxz[last][4], 0, "zero_wxz_last");
    check::eq_i32(r.wzy[last][4], 0, "zero_wzy_last");
    check::eq_i32(r.sspark[4][3][29], 0, "zero_sspark_last");
    check::eq_bool(r.fulls[4][3][29], false, "zero_fulls_last");
    check::eq_i32(bits(r.smag[4][3][29]), bits(0.0), "zero_smag_last");
    check::eq_i32(r.rx[4][3][6], 0, "zero_rx_last");
    check::eq_i32(r.magy[4][3][6], 0, "zero_magy_last");
    check::ok(!r.mtouch[4][6], "zero_mtouch_last");
    check::eq_i32(r.nrx[4][3], 0, "zero_nrx");
    check::eq_i32(r.nry[4][3], 0, "zero_nry");
    check::eq_i32(r.nrz[4][3], 0, "zero_nrz");
    check::eq_i32(r.hx[last][4], 0, "zero_hx_last");
    check::eq_i32(r.hsspark[4][3][29], 0, "zero_hsspark_last");
    check::eq_i32(r.hry[4][3][6], 0, "zero_hry_last");
    check::eq_i32(r.hnry[4][3], 0, "zero_hnry");
    check::ok(!r.hmtouch[4][6], "zero_hmtouch_last");
    for i in 0..5 {
        check::eq_i32(r.fix[i], 0, &format!("zero_fix{i}"));
        check::eq_i32(r.dest[i], 0, &format!("zero_dest{i}"));
        check::eq_i32(r.cntdest[i], 0, &format!("zero_cntdest{i}"));
    }
}

/// THE TRANSCRIPTION-CRITICAL QUIRK (`Record.java:633-650`, `:841-848`):
/// `recx` and `recz` both WRITE at `nry[car][part]` while incrementing their
/// OWN `nrx`/`nrz`; `recy` writes at (and increments) `nry`. So the three
/// counters drift apart and only the FIRST write of a ring lands in slot 0 —
/// `recx`/`recz` follow `recy` around the ring instead of walking it.
fn ring_quirk() {
    // `new Record(m)`'s Medium is per-call in the port and this group never
    // draws from it (no reg*/chip* call), so no Medium exists here.
    let mut q = Record::new();

    // a recy first pushes nry to 1
    q.recy(1, 50.0, true, 2);
    check::eq_i32(q.nry[2][1], 1, "ring_recy_nry_1");
    check::eq_i32(q.ry[2][1][0], 300, "ring_recy_ry_slot0_300");
    check::eq_i32(q.magy[2][1][0], 50, "ring_recy_magy_slot0_50");
    // :645 mtouch is [car][slot] - NOT [part][...]
    check::ok(q.mtouch[2][0], "ring_recy_mtouch_car_slot0");

    // recx writes at nry==1 (not nrx==0) and only bumps nrx
    q.recx(1, 123.7, 2);
    check::eq_i32(q.nrx[2][1], 1, "ring_recx_nrx_1");
    check::eq_i32(q.nry[2][1], 1, "ring_recx_nry_untouched_1");
    check::eq_i32(q.rx[2][1][0], 0, "ring_recx_slot0_untouched");
    check::eq_i32(q.rx[2][1][1], 300, "ring_recx_slot1_at_nry");
    check::eq_i32(q.magx[2][1][1], 123, "ring_recx_magx_truncates_123");

    // recz likewise lands on nry==1 while bumping nrz
    q.recz(1, -250.4, 2);
    check::eq_i32(q.nrz[2][1], 1, "ring_recz_nrz_1");
    check::eq_i32(q.nry[2][1], 1, "ring_recz_nry_untouched_1");
    check::eq_i32(q.rz[2][1][0], 0, "ring_recz_slot0_untouched");
    check::eq_i32(q.rz[2][1][1], 300, "ring_recz_slot1_at_nry");
    check::eq_i32(q.magz[2][1][1], -250, "ring_recz_magz_truncates_minus250");

    // recy advances nry to 2, so the next recx/recz jump to slot 2
    q.recy(1, 99.9, false, 2);
    check::eq_i32(q.nry[2][1], 2, "ring_recy2_nry_2");
    check::eq_i32(q.ry[2][1][1], 300, "ring_recy2_slot1_300");
    check::eq_i32(q.magy[2][1][1], 99, "ring_recy2_magy_99");
    check::ok(!q.mtouch[2][1], "ring_recy2_mtouch_false");

    q.recx(1, 7.9, 2);
    check::eq_i32(q.nrx[2][1], 2, "ring_recx2_nrx_2");
    check::eq_i32(q.rx[2][1][2], 300, "ring_recx2_slot2_at_nry");
    check::eq_i32(q.magx[2][1][2], 7, "ring_recx2_magx_7");

    q.recz(1, 20.9, 2);
    check::eq_i32(q.nrz[2][1], 2, "ring_recz2_nrz_2");
    check::eq_i32(q.rz[2][1][2], 300, "ring_recz2_slot2_at_nry");
    check::eq_i32(q.magz[2][1][2], 20, "ring_recz2_magz_20");

    // three counters, all at 2 after three writes each - but they only
    // agree here because every recx/recz rode a recy advance
    check::eq_i32(q.nry[2][1], 2, "ring_drift_nry_2");
    check::eq_i32(q.nrx[2][1], 2, "ring_drift_nrx_2");
    check::eq_i32(q.nrz[2][1], 2, "ring_drift_nrz_2");

    // recx-only: delegation is frozen, all three writes overwrite slot 0
    let mut a = Record::new();
    a.recx(0, 1.5, 0);
    a.recx(0, 2.5, 0);
    a.recx(0, 3.5, 0);
    check::eq_i32(a.nrx[0][0], 3, "ring_recxonly_nrx_3");
    check::eq_i32(a.nry[0][0], 0, "ring_recxonly_nry_0");
    check::eq_i32(a.magx[0][0][0], 3, "ring_recxonly_slot0_last_value");
    check::eq_i32(a.rx[0][0][1], 0, "ring_recxonly_slot1_never_written");
}

/// Ring wraps: the increment tests `==7` and resets to 0 (`Record.java:637-639`,
/// `:647-649`, `:845-847`), i.e. slot 6 is the last one used. No Medium is
/// needed: only the register pushes run, none of which draw.
fn ring_wraps(k: &K) {
    let mut w = Record::new();
    for i in 0..k.ring {
        w.recy(0, i as f32 * 10.0, false, 1);
    }
    check::eq_i32(w.nry[1][0], 0, "wrap_recy7_nry_back_to_0");
    check::eq_i32(w.magy[1][0][0], 0, "wrap_recy7_slot0_0");
    check::eq_i32(w.magy[1][0][6], 60, "wrap_recy7_slot6_60");
    check::eq_i32(
        w.ry[1][0][k.ring_last as usize],
        300,
        "wrap_recy7_slot6_marker_300",
    );
    w.recy(0, 99.0, false, 1);
    check::eq_i32(w.nry[1][0], 1, "wrap_recy8_nry_1");
    check::eq_i32(w.magy[1][0][0], 99, "wrap_recy8_slot0_overwritten");
    check::eq_i32(w.ry[1][0][1], 300, "wrap_recy8_slot1_kept");

    // recx's own counter wraps independently of the slot it writes
    let mut x = Record::new();
    for i in 0..8 {
        x.recx(0, i as f32 + 0.5, 0);
    }
    check::eq_i32(x.nrx[0][0], 1, "wrap_recx8_nrx_1");
    check::eq_i32(x.magx[0][0][0], 7, "wrap_recx8_slot0_still_last");
    check::eq_i32(x.rx[0][0][1], 0, "wrap_recx8_slot1_still_zero");

    // recz wraps its OWN counter independently; the slot is still nry's
    let mut z = Record::new();
    for i in 0..k.ring {
        z.recz(0, i as f32 + 0.5, 3);
    }
    check::eq_i32(z.nrz[3][0], 0, "wrap_recz7_nrz_back_to_0");
    check::eq_i32(z.magz[3][0][0], 6, "wrap_recz7_slot0_last_6");
    z.recz(0, 9.5, 3);
    check::eq_i32(z.nrz[3][0], 1, "wrap_recz8_nrz_1");
    check::eq_i32(z.magz[3][0][0], 9, "wrap_recz8_slot0_9");

    // the (int) cast truncates toward zero on both signs
    let mut c = Record::new();
    c.recy(0, -1.5, false, 0);
    check::eq_i32(c.magy[0][0][0], -1, "cast_recy_neg1_5_to_minus1");
    c.recz(0, 300.9, 0);
    // recy already advanced nry to 1, so recz/recx land in slot 1
    check::eq_i32(c.magz[0][0][1], 300, "cast_recz_300_9_to_300");
    c.recx(0, -0.9, 0);
    check::eq_i32(c.magx[0][0][1], 0, "cast_recx_neg0_9_to_0");
}

/// `rec` (`Record.java:543-631`): caught, cntf period, fix/dest decrements and
/// the `[frame][car]` shift-left-then-insert. Every frame array is
/// `[300][5] = [frame][car]` (`:14-21`), so a write is `x[frame][car]`.
fn rec_frames(k: &K) {
    let m = Medium::new();
    let last = k.frames as usize - 1;
    // a never-reset Record has null car[1..5][*], and :552 dereferences it
    let mut fresh = Record::new();
    check::throws_it(
        AssertUnwindSafe(|| {
            let fc = car(&m);
            fresh.rec(&m, &fc, 0, 0, 0, 0);
        }),
        "NullPointerException",
        "rec_unreset_history_npe",
    );

    let mut f = Record::new();
    f.reset(&five(&m), &m);
    let mut c0 = car(&m);
    c0.x = 100;
    c0.y = 200;
    c0.z = 300;
    c0.xy = 11;
    c0.zy = 22;
    c0.xz = 33;
    c0.wxz = 44;
    c0.wzy = 55;
    f.x[0][0] = 1;
    f.x[1][0] = 2;
    f.x[last - 1][0] = 40;
    f.x[last][0] = 41;
    f.x[last][1] = 900;
    f.rec(&m, &c0, 0, 3, 0, 0);
    // :544-546 caught counts car 0 only
    check::eq_i32(f.caught, 1, "rec_caught_car0_1");
    check::eq_i32(f.cntf, 0, "rec_first_call_resets_cntf_0");
    // reset left fix/dest at -1 (:236-238); :563 always decrements fix
    check::eq_i32(f.fix[0], -2, "rec_fix_decrement_minus2");
    check::eq_i32(f.dest[0], -1, "rec_dest_not_decremented_when_var5_0");
    // :581-589 shift runs 0..298, copying [f+1] to [f]
    check::eq_i32(f.x[0][0], 2, "rec_shift_x0_from_x1");
    check::eq_i32(f.x[last - 1][0], 41, "rec_shift_x298_from_x299");
    // :591-598 the new frame lands at [299][car]
    check::eq_i32(f.x[last][0], 100, "rec_insert_x299");
    check::eq_i32(f.y[last][0], 200, "rec_insert_y299");
    check::eq_i32(f.z[last][0], 300, "rec_insert_z299");
    check::eq_i32(f.xy[last][0], 11, "rec_insert_xy299");
    check::eq_i32(f.zy[last][0], 22, "rec_insert_zy299");
    check::eq_i32(f.xz[last][0], 33, "rec_insert_xz299");
    check::eq_i32(f.wxz[last][0], 44, "rec_insert_wxz299");
    check::eq_i32(f.wzy[last][0], 55, "rec_insert_wzy299");
    // only the recorded car's column moves
    check::eq_i32(f.x[last][1], 900, "rec_insert_other_car_untouched");
    check::eq_i32(f.x[0][1], 0, "rec_shift_other_car_untouched");
    // :556-557 the current ContO is cloned into history slot 5
    check::eq_i32(f.squash[5][0], 3, "rec_squash5_is_var3");

    let mut c1 = car(&m);
    c1.x = 7;
    f.rec(&m, &c1, 1, 9, 0, 0);
    check::eq_i32(f.cntf, 1, "rec_second_cntf_1");
    check::eq_i32(f.caught, 1, "rec_car1_does_not_count_caught");
    check::eq_i32(f.fix[1], -2, "rec_car1_fix_minus2");
    check::eq_i32(f.x[last][1], 7, "rec_car1_insert_x299");
    check::eq_i32(f.x[last][0], 100, "rec_car1_does_not_touch_car0_slot299");

    // cntf is ONE shared counter and only re-shifts when it hits 50
    // (:548,:558-561): call 1 shifted, call 52 shifts again
    for _ in 0..49 {
        let c = car(&m);
        f.rec(&m, &c, 2, 0, 0, 0);
    }
    check::eq_i32(f.cntf, k.cntf, "rec_call51_cntf_50");
    check::eq_i32(f.fix[2], -50, "rec_call51_fix2_minus50");
    let c = car(&m);
    f.rec(&m, &c, 2, 0, 0, 0);
    check::eq_i32(f.cntf, 0, "rec_call52_cntf_0");
    check::eq_i32(f.fix[2], -51, "rec_call52_fix2_minus51");

    // frame 300 does not exist in any [300][5] array
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = Record::new();
            let mut mm = Medium::new();
            let mut c = car(&mm);
            let md = madness(1);
            e.play(&mut c, &md, 0, 300, &mut mm);
        }),
        "index out of bounds",
        "rec_frame300_play_aioobe",
    );
}

/// `Record.java:563-566` — dest only moves when `var5!=0`; the car-history
/// shift (`:551-554`) re-clones `car[1..5]` into `car[0..4]`, and `ContO`'s
/// copy ctor (`:421-508`) is given 0,0,0,0, so the clones have x/y/z == 0.
fn rec_history() {
    let m = Medium::new();
    let mut g = Record::new();
    g.reset(&five(&m), &m);
    let mut sentinel = car(&m);
    sentinel.maxr = 55;
    g.car[2][0] = Some(sentinel);
    g.squash[2][0] = 7;
    g.fix[0] = 5;
    g.dest[0] = 5;
    let mut gc = car(&m);
    gc.maxr = 77;
    gc.x = 4242;
    g.rec(&m, &gc, 0, 0, 0, 1);
    check::eq_i32(g.fix[0], 4, "hist_fix_decrement");
    check::eq_i32(g.dest[0], 4, "hist_dest_decrement_when_var5_1");
    check::eq_i32(
        g.car[1][0].as_ref().unwrap().maxr,
        55,
        "hist_car2_shifted_to_car1",
    );
    check::eq_i32(g.squash[1][0], 7, "hist_squash2_shifted_to_1");
    check::eq_i32(
        g.car[5][0].as_ref().unwrap().maxr,
        77,
        "hist_car5_is_new_clone",
    );
    check::eq_i32(g.car[5][0].as_ref().unwrap().x, 0, "hist_clone_zeroes_x");
    // ContO.npl is the port's planes.len()
    check::eq_i32(
        g.car[5][0].as_ref().unwrap().planes.len() as i32,
        0,
        "hist_clone_keeps_npl",
    );
    let c = car(&m);
    g.rec(&m, &c, 0, 0, 0, 0);
    check::eq_i32(g.fix[0], 3, "hist_fix_decrements_again");
    check::eq_i32(g.dest[0], 4, "hist_dest_frozen_when_var5_0");
}

/// `rec`'s spark/ring bookkeeping (`Record.java:599-630`): a live spark is
/// written with marker 300, then the SAME call subtracts 1 from all 30 slots,
/// so the freshly written slot reads 299; every ring slot of the recording
/// car also loses 1.
fn rec_spark(k: &K) {
    let m = Medium::new();
    let frames = k.frames;
    let mut sp = Record::new();
    sp.reset(&five(&m), &m);
    let mut sc = car(&m);
    sc.stg[0] = 1;
    sc.sx[0] = 11;
    sc.sy[0] = 12;
    sc.sz[0] = 13;
    sc.smag[0] = 1.5;
    sc.scx[0] = 14;
    sc.scz[0] = 15;
    sc.fulls[0] = true;
    sp.ry[0][0][0] = 300;
    sp.rec(&m, &sc, 0, 0, 0, 0);
    check::eq_i32(sp.ns[0][0], 1, "spark_ns_1");
    check::eq_i32(
        sp.sspark[0][0][0],
        frames - 1,
        "spark_marker_300_then_decrement_299",
    );
    check::eq_i32(sp.sx[0][0][0], 11, "spark_sx_copied");
    check::eq_i32(sp.sy[0][0][0], 12, "spark_sy_copied");
    check::eq_i32(sp.sz[0][0][0], 13, "spark_sz_copied");
    check::eq_i32(bits(sp.smag[0][0][0]), bits(1.5), "spark_smag_copied");
    check::eq_i32(sp.scx[0][0][0], 14, "spark_scx_copied");
    check::eq_i32(sp.scz[0][0][0], 15, "spark_scz_copied");
    check::ok(sp.fulls[0][0][0], "spark_fulls_copied");
    check::eq_i32(sp.ry[0][0][0], frames - 1, "spark_ring_slot_decrements");

    sp.rec(&m, &sc, 0, 0, 0, 0);
    check::eq_i32(sp.ns[0][0], 2, "spark2_ns_2");
    check::eq_i32(sp.sspark[0][0][0], 298, "spark2_slot0_298");
    check::eq_i32(sp.sspark[0][0][1], frames - 1, "spark2_slot1_299");
    check::eq_i32(sp.ry[0][0][0], 298, "spark2_ring_slot0_298");
    // part 1 had no spark: it just keeps decaying from the reset -1
    check::eq_i32(sp.ns[0][1], 0, "spark2_part1_never_marked");
    check::eq_i32(sp.sspark[0][1][0], -3, "spark2_part1_decays_from_minus1");

    // ns wraps at 30 (:611-614)
    let mut spw = Record::new();
    spw.reset(&five(&m), &m);
    for _ in 0..31 {
        spw.rec(&m, &sc, 1, 0, 0, 0);
    }
    check::eq_i32(spw.ns[1][0], 1, "sparkwrap_ns_1");
    check::eq_i32(spw.sspark[1][0][0], frames - 1, "sparkwrap_slot0_299");
}

/// `Record.java:568-576` — dest is checked AFTER the decrement, so pre-setting
/// 231 + `var5!=0` lands exactly on 230. Car 0 fires `cotchinow(0)` with
/// `whenwasted=229`; any other car needs `var4!=0` and gets `165+var4`. The
/// h-copy happens BEFORE the shift/insert (`:570` before `:578`), so it
/// snapshots the PREVIOUS frame.
fn rec_dest_230(k: &K) {
    let m = Medium::new();
    let last = k.frames as usize - 1;
    let mut t = Record::new();
    t.reset(&five(&m), &m);
    t.x[last][2] = 777;
    t.ry[2][0][3] = 500;
    t.nry[2][0] = 4;
    t.hx[0][0] = 1;
    // squash[0][2] is fed by the history shift, so seed slot 1: the shift
    // copies squash[1][2] into squash[0][2] before cotchinow reads it
    t.squash[1][2] = 6;
    t.caught = 300;
    t.dest[2] = 231;
    t.fix[2] = 8;
    let mut tc = car(&m);
    tc.x = 1234;
    tc.maxr = 66;
    t.rec(&m, &tc, 2, 4, 9, 1);
    check::eq_i32(t.dest[2], 230, "dest230_after_decrement");
    check::eq_i32(t.fix[2], 7, "dest230_fix_decremented");
    check::eq_i32(t.wasted, 2, "dest230_wasted_is_car");
    check::eq_i32(t.whenwasted, 174, "dest230_whenwasted_165_plus_var4_9");
    check::ok(t.hcaught, "dest230_hcaught_true");
    check::eq_i32(t.x[last][2], 1234, "dest230_insert_after_copy");
    check::eq_i32(t.hx[last][2], 777, "dest230_hcopy_sees_previous_frame");
    check::eq_i32(t.hfix[2], 7, "dest230_hfix_post_decrement");
    check::eq_i32(t.hdest[2], 230, "dest230_hdest_230");
    check::eq_i32(t.hsquash[2], 6, "dest230_hsquash_after_shift");
    check::eq_i32(t.hry[2][0][3], 500, "dest230_hry_copied_before_ring_decay");
    check::eq_i32(t.hry[2][0][0], -1, "dest230_hry_untouched_slot");
    check::eq_i32(t.hnry[2][0], 4, "dest230_hnry_copies_counter");
    check::eq_i32(t.nry[2][0], 4, "dest230_nry_not_moved_by_cotchinow");
    check::eq_i32(t.hsspark[2][0][3], -1, "dest230_hsspark_from_reset");
    // starcar comes from car[0] AFTER the history shift, and the copy ctor
    // zeroes the geometry
    // car[0][2] after the shift is the reset copy of arr[2] (maxR 32),
    // not the new tc clone (maxR 66)
    check::eq_i32(
        t.starcar[2].as_ref().unwrap().maxr,
        32,
        "dest230_starcar_from_car0_after_shift",
    );
    check::eq_i32(
        t.starcar[2].as_ref().unwrap().x,
        0,
        "dest230_starcar_x_zeroed",
    );

    // other car, var4==0: the 230 check enters but neither arm fires
    let mut t2 = Record::new();
    t2.reset(&five(&m), &m);
    t2.caught = 300;
    t2.dest[3] = 231;
    let c = car(&m);
    t2.rec(&m, &c, 3, 0, 0, 1);
    check::eq_i32(t2.dest[3], 230, "dest230_other_car_dest_230");
    check::eq_i32(t2.wasted, 0, "dest230_other_car_var4_0_no_cotchinow");
    check::ok(!t2.hcaught, "dest230_other_car_hcaught_false");
    check::eq_i32(t2.whenwasted, 0, "dest230_other_car_whenwasted_0");

    // other car with var4!=0
    let mut t3 = Record::new();
    t3.reset(&five(&m), &m);
    t3.caught = 300;
    t3.dest[3] = 231;
    let c = car(&m);
    t3.rec(&m, &c, 3, 0, 5, 1);
    check::eq_i32(t3.wasted, 3, "dest230_car3_wasted_3");
    check::eq_i32(t3.whenwasted, 170, "dest230_car3_whenwasted_170");

    // car 0 always calls cotchinow + whenwasted=229, even when caught<300
    let mut t4 = Record::new();
    t4.reset(&five(&m), &m);
    t4.caught = 0;
    t4.dest[0] = 231;
    let c = car(&m);
    t4.rec(&m, &c, 0, 0, 0, 1);
    check::eq_i32(t4.caught, 1, "dest230_car0_caught_incremented");
    check::eq_i32(t4.whenwasted, 229, "dest230_car0_whenwasted_229_regardless");
    check::eq_i32(t4.wasted, 0, "dest230_car0_caught_below_300_no_wasted");
    check::ok(!t4.hcaught, "dest230_car0_caught_below_300_no_hcaught");
}

/// `play` (`Record.java:471-541`) loads the eight placement fields from
/// `[frame][car]` first — so every later branch (and the `reg*` damage
/// scatter) sees the RECORDED angles, not the live ones. `play` never writes
/// `lastfr`; that is playh-only (`:366`).
fn play_frames() {
    let mut m = Medium::new();
    let mut pr = Record::new();
    pr.x[10][3] = 4321;
    pr.y[10][3] = 4322;
    pr.z[10][3] = 4323;
    pr.xy[10][3] = 41;
    pr.zy[10][3] = 42;
    pr.xz[10][3] = 43;
    pr.wxz[10][3] = 44;
    pr.wzy[10][3] = 45;
    pr.lastfr = 77;
    let mut pc = car(&m);
    let pmd = madness(1);
    pr.play(&mut pc, &pmd, 3, 10, &mut m);
    check::eq_i32(pc.x, 4321, "play_x_from_frame_car");
    check::eq_i32(pc.y, 4322, "play_y_from_frame_car");
    check::eq_i32(pc.z, 4323, "play_z_from_frame_car");
    check::eq_i32(pc.xy, 41, "play_xy_from_frame_car");
    check::eq_i32(pc.zy, 42, "play_zy_from_frame_car");
    check::eq_i32(pc.xz, 43, "play_xz_from_frame_car");
    check::eq_i32(pc.wxz, 44, "play_wxz_from_frame_car");
    check::eq_i32(pc.wzy, 45, "play_wzy_from_frame_car");
    check::eq_i32(pr.lastfr, 77, "play_leaves_lastfr_alone");
    check::eq_i32(pr.cntdest[3], 0, "play_cntdest_untouched_when_no_dest");

    // orientation proof: [frame][car], not [car][frame]
    let mut pr2 = Record::new();
    pr2.x[3][0] = 999;
    pr2.x[10][3] = 1234;
    let mut pc2 = car(&m);
    let md2 = madness(1);
    pr2.play(&mut pc2, &md2, 3, 10, &mut m);
    check::eq_i32(pc2.x, 1234, "play_orientation_frame_then_car");

    // cntdest gates (:480-504)
    let mut cd = Record::new();
    let mut cdc = car(&m);
    let cdm = madness(1);
    cd.dest[2] = 5;
    cd.play(&mut cdc, &cdm, 2, 5, &mut m);
    check::eq_i32(cd.cntdest[2], 6, "play_dest_match_7_then_decrement_6");
    cd.play(&mut cdc, &cdm, 2, 5, &mut m);
    check::eq_i32(cd.cntdest[2], 6, "play_dest_match_again_6");

    let mut cd0 = Record::new();
    cd0.dest[2] = 5;
    cd0.cntdest[2] = 5;
    let mut c0 = car(&m);
    let md0 = madness(1);
    cd0.play(&mut c0, &md0, 2, 0, &mut m);
    check::eq_i32(cd0.cntdest[2], 0, "play_frame0_zeroes_cntdest");

    let mut cd3 = Record::new();
    cd3.dest[2] = 0;
    let mut c3 = car(&m);
    let md3 = madness(1);
    cd3.play(&mut c3, &md3, 2, 0, &mut m);
    check::eq_i32(cd3.cntdest[2], 6, "play_frame0_dest0_7_then_decrement_6");
}

/// embos gates (`Record.java:488-504`): frame 0 + `dest < -1` -> 13, and any
/// non-zero `cntdest` -> 1 for `wz==0` planes only (then `cntdest--`).
fn play_gates() {
    let mut m = Medium::new();
    let mut em = Record::new();
    let mut emc = planed(&m, ZERO_OY, 0);
    em.dest[1] = -2;
    let md = madness(1);
    em.play(&mut emc, &md, 1, 0, &mut m);
    check::eq_i32(emc.planes[0].embos, 13, "gate_embos13");
    check::eq_i32(em.cntdest[1], 0, "gate_embos13_cntdest_0");

    let mut em2 = Record::new();
    let mut emc2 = planed(&m, ZERO_OY, 1);
    em2.dest[1] = -2;
    let md2 = madness(1);
    em2.play(&mut emc2, &md2, 1, 0, &mut m);
    check::eq_i32(emc2.planes[0].embos, 0, "gate_embos13_skips_wz_nonzero");

    let mut em3 = Record::new();
    let mut emc3 = planed(&m, ZERO_OY, 0);
    em3.dest[0] = 4;
    let md3 = madness(1);
    em3.play(&mut emc3, &md3, 0, 4, &mut m);
    check::eq_i32(emc3.planes[0].embos, 1, "gate_embos1_when_cntdest_nonzero");
    check::eq_i32(em3.cntdest[0], 6, "gate_embos1_cntdest_decrement");

    let mut em4 = Record::new();
    let mut emc4 = planed(&m, ZERO_OY, 1);
    em4.dest[0] = 4;
    let md4 = madness(1);
    em4.play(&mut emc4, &md4, 0, 4, &mut m);
    check::eq_i32(emc4.planes[0].embos, 0, "gate_embos1_skips_wz_nonzero");
}

/// The part loop (`Record.java:506-540`): `sspark[car][part][cursor]==frame`
/// restores that ContO's spark slot, and `rx/ry/rz[car][part][slot]==frame`
/// fires `regx`/`regy`/`regz`. Two matching slots fire twice. `play` draws the
/// fixed tenths stream through `reg*`, so every pinned plane value below is a
/// pure function of the vector.
fn play_parts(k: &K) {
    let mut m = Medium::new();
    let mut sr = Record::new();
    sr.sspark[2][1][5] = 9;
    sr.sx[2][1][5] = 111;
    sr.sy[2][1][5] = 112;
    sr.sz[2][1][5] = 113;
    sr.smag[2][1][5] = 1.25;
    sr.scx[2][1][5] = 114;
    sr.scz[2][1][5] = 115;
    sr.fulls[2][1][5] = true;
    let mut src = car(&m);
    src.stg[1] = 3;
    let msr = madness(1);
    sr.play(&mut src, &msr, 2, 9, &mut m);
    check::eq_i32(src.stg[1], 1, "parts_sspark_stg_1");
    check::eq_i32(src.dov[1], -1, "parts_sspark_dov_minus1");
    check::eq_i32(src.sx[1], 111, "parts_sspark_sx");
    check::eq_i32(src.sy[1], 112, "parts_sspark_sy");
    check::eq_i32(src.sz[1], 113, "parts_sspark_sz");
    check::eq_i32(bits(src.smag[1]), bits(1.25), "parts_sspark_smag");
    check::eq_i32(src.scx[1], 114, "parts_sspark_scx");
    check::eq_i32(src.scz[1], 115, "parts_sspark_scz");
    check::ok(src.fulls[1], "parts_sspark_fulls");
    // a frame that does not match leaves the slot alone
    let mut src2 = car(&m);
    src2.stg[1] = 3;
    let msr2 = madness(1);
    sr.play(&mut src2, &msr2, 2, 8, &mut m);
    check::eq_i32(src2.stg[1], 3, "parts_sspark_frame_mismatch");

    // TestRng.install(301/302/303) has no counterpart: the vector medium
    // replaces random() wholesale, so the seed never reaches a pin.
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    // the loaded recorded angles drive the scatter direction
    r.xz[7][0] = 45;
    r.xy[7][0] = 30;
    r.zy[7][0] = 60;
    r.rx[0][2][6] = 7;
    r.magx[0][2][6] = 150;
    r.play(&mut c, &md, 0, 7, &mut th.m);
    // regx: var2 -> 50, var6 = 50/20*m.random(), one draw per qualifying
    // point (3 points)
    check::eq_i32(th.draws(), 3, "parts_play_regx_draws_3");
    check::eq_i32(c.xz, 45, "parts_play_loaded_xz");
    check::eq_i32(c.planes[0].ox[0], 0, "parts_play_regx_ox0_0");
    check::eq_i32(c.planes[0].ox[1], 10, "parts_play_regx_ox1_10");
    check::eq_i32(c.planes[0].oz[2], 9, "parts_play_regx_oz2_9");
    check::eq_i32(
        bits(c.planes[0].ctmag),
        bits(1.0),
        "parts_play_regx_ctmag_1",
    );
    check::eq_i32(c.planes[0].chip, 1, "parts_play_regx_chip_1");

    // regy uses the recorded zy/xy signs: zy=90 -> sign 0 so the first
    // branch runs; zy=0 -> sign +1 on both axes and NO branch at all
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.zy[7][0] = 90;
    r.xy[7][0] = 0;
    r.ry[0][0][4] = 7;
    r.magy[0][0][4] = 150;
    r.play(&mut c, &md, 0, 7, &mut th.m);
    check::eq_i32(th.draws(), 3, "parts_play_regy_draws_3");
    check::eq_i32(c.zy, 90, "parts_play_loaded_zy");
    check::eq_i32(c.planes[0].oz[2], 11, "parts_play_regy_oz2_11");
    check::eq_i32(
        bits(c.planes[0].ctmag),
        bits(1.0),
        "parts_play_regy_ctmag_1",
    );
    check::eq_i32(c.planes[0].chip, 1, "parts_play_regy_chip_1");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.rx[0][0][0] = 7;
    r.magx[0][0][0] = 150;
    r.rx[0][0][1] = 7;
    r.magx[0][0][1] = 150;
    r.play(&mut c, &md, 0, 7, &mut th.m);
    check::eq_i32(th.draws(), 6, "parts_play_two_ring_hits_draws_6");
}

/// `playh` (`Record.java:287-367`) is `play` over the h arrays and the ONLY
/// writer of `lastfr` (`:366`); its cntdest/embos gates read `hdest`
/// (`:300`,`:304`).
fn playh_frames() {
    let mut m = Medium::new();
    let mut ph = Record::new();
    ph.hx[10][3] = 5321;
    ph.hy[10][3] = 5322;
    ph.hz[10][3] = 5323;
    ph.hxy[10][3] = 51;
    ph.hzy[10][3] = 52;
    ph.hxz[10][3] = 53;
    ph.hwxz[10][3] = 54;
    ph.hwzy[10][3] = 55;
    let mut hc = car(&m);
    let md = madness(1);
    ph.playh(&mut hc, &md, 3, 10, &mut m);
    check::eq_i32(hc.x, 5321, "playh_x_from_frame_car");
    check::eq_i32(hc.y, 5322, "playh_y_from_frame_car");
    check::eq_i32(hc.z, 5323, "playh_z_from_frame_car");
    check::eq_i32(hc.xy, 51, "playh_xy");
    check::eq_i32(hc.zy, 52, "playh_zy");
    check::eq_i32(hc.xz, 53, "playh_xz");
    check::eq_i32(hc.wxz, 54, "playh_wxz");
    check::eq_i32(hc.wzy, 55, "playh_wzy");
    check::eq_i32(ph.lastfr, 10, "playh_sets_lastfr_frame");

    // hdest, not dest, drives the gates
    let mut hd = Record::new();
    let mut hdc = car(&m);
    hd.hdest[2] = 5;
    let mdh = madness(1);
    hd.playh(&mut hdc, &mdh, 2, 5, &mut m);
    check::eq_i32(hd.cntdest[2], 6, "playh_hdest_match_6");
    hd.playh(&mut hdc, &mdh, 2, 5, &mut m);
    check::eq_i32(hd.cntdest[2], 6, "playh_hdest_match_again_6");

    let mut hd2 = Record::new();
    let mut hd2c = car(&m);
    hd2.dest[2] = 5; // dest must NOT be read by playh
    let md2 = madness(1);
    hd2.playh(&mut hd2c, &md2, 2, 5, &mut m);
    check::eq_i32(hd2.cntdest[2], 0, "playh_ignores_dest");

    let mut hd3 = Record::new();
    let mut hd3c = planed(&m, ZERO_OY, 0);
    hd3.hdest[2] = -2;
    let md3 = madness(1);
    hd3.playh(&mut hd3c, &md3, 2, 0, &mut m);
    check::eq_i32(hd3c.planes[0].embos, 13, "playh_embos13");
    check::eq_i32(hd3.cntdest[2], 0, "playh_embos13_cntdest_0");

    let mut hd4 = Record::new();
    let mut hd4c = planed(&m, ZERO_OY, 0);
    hd4.hdest[0] = 0;
    let md4 = madness(1);
    hd4.playh(&mut hd4c, &md4, 0, 0, &mut m);
    check::eq_i32(hd4c.planes[0].embos, 1, "playh_frame0_embos1");
    check::eq_i32(hd4.cntdest[0], 6, "playh_frame0_cntdest_6");
    check::eq_i32(hd4.lastfr, 0, "playh_frame0_lastfr_0");

    // the restore reads hsspark/hsx..., never sspark/sx...
    let mut hs = Record::new();
    hs.sspark[2][1][5] = 9;
    hs.sx[2][1][5] = 111;
    let mut hsc = car(&m);
    let mdh2 = madness(1);
    hs.playh(&mut hsc, &mdh2, 2, 9, &mut m);
    check::eq_i32(hsc.stg[1], 0, "playh_ignores_live_sspark");
    let mut hs2 = Record::new();
    hs2.hsspark[2][1][5] = 9;
    hs2.hsx[2][1][5] = 111;
    hs2.hsy[2][1][5] = 112;
    hs2.hsz[2][1][5] = 113;
    hs2.hsmag[2][1][5] = 1.25;
    hs2.hscx[2][1][5] = 114;
    hs2.hscz[2][1][5] = 115;
    hs2.hfulls[2][1][5] = true;
    let mut hsc2 = car(&m);
    hsc2.stg[1] = 3;
    let mdh3 = madness(1);
    hs2.playh(&mut hsc2, &mdh3, 2, 9, &mut m);
    check::eq_i32(hsc2.stg[1], 1, "playh_hsspark_stg_1");
    check::eq_i32(hsc2.dov[1], -1, "playh_hsspark_dov_minus1");
    check::eq_i32(hsc2.sx[1], 111, "playh_hsspark_sx");
    check::eq_i32(hsc2.sy[1], 112, "playh_hsspark_sy");
    check::eq_i32(hsc2.sz[1], 113, "playh_hsspark_sz");
    check::eq_i32(bits(hsc2.smag[1]), bits(1.25), "playh_hsspark_smag");
    check::eq_i32(hsc2.scx[1], 114, "playh_hsspark_scx");
    check::eq_i32(hsc2.scz[1], 115, "playh_hsspark_scz");
    check::ok(hsc2.fulls[1], "playh_hsspark_fulls");
}

/// playh's reg-vs-chip switch (`Record.java:344-362`): each ring hit compares
/// `lastfr` to the frame first; when they already match, x and z restore
/// through `chipx`/`chipz` (chip/ctmag stamped, geometry untouched),
/// otherwise through `regx`/`regz`. y only has the regy arm, gated on
/// `lastfr != frame`.
fn playh_lastfr(k: &K) {
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.hzy[7][0] = 90;
    r.hxy[7][0] = 0;
    r.hxz[7][0] = 45;
    r.hry[0][0][2] = 7;
    r.hmagy[0][0][2] = 150;
    r.hrz[0][3][1] = 7;
    r.hmagz[0][3][1] = -150;
    r.playh(&mut c, &md, 0, 7, &mut th.m);
    // 3 draws from regy (part 0) + 3 from regz (part 3)
    check::eq_i32(th.draws(), 6, "lastfr_before_playh_both_reg_draws_6");
    check::eq_i32(c.planes[0].ox[0], 0, "lastfr_playh_ox0_0");
    check::eq_i32(c.planes[0].ox[1], 9, "lastfr_playh_ox1_9");
    check::eq_i32(c.planes[0].oz[0], 1, "lastfr_playh_oz0_1");
    check::eq_i32(c.planes[0].oz[2], 11, "lastfr_playh_oz2_11");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "lastfr_playh_ctmag");
    check::eq_i32(r.lastfr, 7, "lastfr_playh_updated");

    // with lastfr == frame, rx goes through chipx: the plane never moves
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.hzy[7][0] = 90;
    r.hxy[7][0] = 0;
    r.hxz[7][0] = 45;
    r.lastfr = 7;
    r.hrx[0][0][0] = 7;
    r.hmagx[0][0][0] = 150;
    r.playh(&mut c, &md, 0, 7, &mut th.m);
    check::eq_i32(th.draws(), 3, "lastfr_chipx_draws_3");
    check::eq_i32(c.planes[0].ox[0], 0, "lastfr_chipx_plane_unmoved_ox");
    check::eq_i32(c.planes[0].oz[2], 10, "lastfr_chipx_plane_unmoved_oz");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "lastfr_chipx_ctmag_1");
    check::eq_i32(c.planes[0].chip, 1, "lastfr_chipx_chip_1");

    // lastfr != frame -> regx: same magnitude, but the plane DOES move
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.hzy[7][0] = 90;
    r.hxy[7][0] = 0;
    r.hxz[7][0] = 45;
    r.lastfr = 6;
    r.hrx[0][0][0] = 7;
    r.hmagx[0][0][0] = 150;
    r.playh(&mut c, &md, 0, 7, &mut th.m);
    check::eq_i32(th.draws(), 3, "lastfr_regx_draws_3");
    check::eq_i32(c.planes[0].ox[0], 0, "lastfr_regx_ox0_0");
    check::eq_i32(c.planes[0].oz[0], 0, "lastfr_regx_oz0_0");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "lastfr_regx_ctmag_1");
    check::eq_i32(r.lastfr, 7, "lastfr_regx_sets_lastfr");

    // recorded angles that leave both sign factors at +1: regy's two
    // branches are BOTH skipped, so no randomness is consumed at all
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.hzy[7][0] = 0;
    r.hxy[7][0] = 0;
    r.hxz[7][0] = 45;
    r.hry[0][0][2] = 7;
    r.hmagy[0][0][2] = 150;
    r.playh(&mut c, &md, 0, 7, &mut th.m);
    check::eq_i32(th.draws(), 0, "lastfr_regy_signs_plus1_no_branch_draws_0");
    check::eq_i32(
        c.planes[0].oz[0],
        0,
        "lastfr_regy_signs_plus1_plane_untouched",
    );
}

/// `regx` (`Record.java:766-839`). The guard is `Math.abs(var2) > 100` (so
/// exactly 100 is inert), the plane is 3 points all inside `clrad[0]==3300`,
/// and every qualifying point consumes one `m.random()` draw:
/// `force = var2/20*r`.
fn regx_group(k: &K) {
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.regx(0, 150.0, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "regx_draws_3");
    check::eq_i32(c.planes[0].ox[0], 0, "regx_ox0_0");
    check::eq_i32(c.planes[0].ox[1], 10, "regx_ox1_10");
    check::eq_i32(c.planes[0].ox[2], 0, "regx_ox2_0");
    check::eq_i32(c.planes[0].oz[0], 0, "regx_oz0_0");
    check::eq_i32(c.planes[0].oz[1], 0, "regx_oz1_0");
    check::eq_i32(c.planes[0].oz[2], 9, "regx_oz2_9");
    check::eq_i32(c.planes[0].chip, 1, "regx_chip_1");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "regx_ctmag_1");
    // nocol plane (oc all equal, Plane.java:132-134) -> no tint work
    check::eq_i32(c.planes[0].bfase, 0, "regx_bfase_untouched_nocol");

    let mut m = Medium::new();
    // strict > 100: 100.0F is a no-op in all five damage methods
    let mut r100 = Record::new();
    let mut c100 = planed(&m, ZERO_OY, 0);
    let md100 = madness(1);
    r100.regx(0, 100.0, &mut c100, &md100, &mut m);
    check::eq_i32(c100.planes[0].ox[1], 10, "regx_100_noop_ox");
    check::eq_i32(c100.planes[0].oz[2], 10, "regx_100_noop_oz");
    check::eq_i32(c100.planes[0].chip, 0, "regx_100_noop_chip");

    // npl==0 (the empty-byte fixture): the loops never run
    let mut th0 = Tenths::new(&k.vec);
    let mut r0 = Record::new();
    let mut c0 = car(&th0.m);
    let md0 = madness(1);
    r0.regx(0, 150.0, &mut c0, &md0, &mut th0.m);
    check::eq_i32(th0.draws(), 0, "regx_npl0_no_draws");

    // clrad gate: a point at (500,500) is outside 3300 and draws nothing
    let mut thf = Tenths::new(&k.vec);
    let mut rf = Record::new();
    let mut cf = car(&thf.m);
    cf.planes.push(Plane::new(
        &thf.m,
        &[500, 10, 0],
        &[500, 0, 10],
        &ZERO_OY,
        3,
        [1, 1, 1],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    ));
    let mdf = madness(1);
    rf.regx(0, 150.0, &mut cf, &mdf, &mut thf.m);
    check::eq_i32(thf.draws(), 2, "regx_far_point_draws_2");
    check::eq_i32(cf.planes[0].ox[0], 500, "regx_far_point_ox_unchanged");

    // each plane runs the whole point loop: two planes -> six draws
    let mut th2 = Tenths::new(&k.vec);
    let mut r2 = Record::new();
    let mut c2 = planed(&th2.m, ZERO_OY, 0);
    c2.planes.push(Plane::new(
        &th2.m,
        &[0, 4, 0],
        &[0, 0, 4],
        &ZERO_OY,
        3,
        [1, 1, 1],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    ));
    let md2 = madness(1);
    r2.regx(0, 150.0, &mut c2, &md2, &mut th2.m);
    check::eq_i32(th2.draws(), 6, "regx_two_planes_draws_6");
}

/// `regy` (`Record.java:78-221`). Its guard is `var2 > 100.0F` — NO absolute
/// value — so a large negative damage is fully inert. The branches are chosen
/// from the sign of the normalised zy/xy (`:86-118`): `signZ*signY==0` (or the
/// `var3` flag) runs the x/z scatter with force `var2/20`; `==-1` runs the y
/// lift with force `var2/15` and folds the result into `squash[0][im]` using a
/// denominator that starts at 1.
fn regy_group(k: &K) {
    // branch A: zy=90 -> signZ 0, xy=0 -> signY +1, product 0
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    c.zy = 90;
    c.xy = 0;
    let md = madness(1);
    r.regy(0, 150.0, false, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "regy_branchA_draws_3");
    check::eq_i32(c.planes[0].ox[0], 0, "regy_branchA_ox_untouched");
    check::eq_i32(c.planes[0].oz[0], 1, "regy_branchA_oz0_1");
    // the tenths vector is 0.4, 0.0, 0.4 at the head: the middle draw is
    // exactly 0.0, so point 1 receives no lift at all
    check::eq_i32(c.planes[0].oz[1], 0, "regy_branchA_oz1_0");
    check::eq_i32(c.planes[0].oz[2], 11, "regy_branchA_oz2_11");
    check::eq_i32(c.planes[0].oy[0], 0, "regy_branchA_oy_untouched");
    check::eq_i32(c.planes[0].chip, 1, "regy_branchA_chip_1");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "regy_branchA_ctmag_1");
    check::eq_i32(r.squash[0][1], 0, "regy_branchA_squash_untouched");

    // branch B: zy=180 -> signZ -1, xy=0 -> signY +1, product -1
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, [-50, -45, -40], 0);
    c.zy = 180;
    c.xy = 0;
    let md = madness(1);
    r.regy(0, 150.0, false, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "regy_branchB_draws_3");
    check::eq_i32(c.planes[0].oy[0], -48, "regy_branchB_oy0_minus48");
    check::eq_i32(c.planes[0].oy[1], -45, "regy_branchB_oy1_minus45");
    check::eq_i32(c.planes[0].oy[2], -38, "regy_branchB_oy2_minus38");
    check::eq_i32(c.planes[0].ox[1], 10, "regy_branchB_ox_untouched");
    // three points lift, but var17 starts at 1 (:186) so the divisor is 4
    // and the int-accumulated var16 == 2 truncates 2/4 to 0
    check::eq_i32(r.squash[0][1], 0, "regy_branchB_squash_0");
    // last var13 = 50/15 * 0.4f == 1.3333334
    check::eq_i32(
        bits(c.planes[0].ctmag),
        1068149419,
        "regy_branchB_ctmag_1_3333334",
    );
    check::eq_i32(c.planes[0].chip, 1, "regy_branchB_chip_1");

    // var3 true forces branch A on top of branch B: six draws
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, [-50, -45, -40], 0);
    c.zy = 180;
    c.xy = 0;
    let md = madness(1);
    r.regy(0, 150.0, true, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 6, "regy_var3_both_branches_draws_6");
    check::eq_i32(c.planes[0].oy[0], -49, "regy_var3_oy0_minus49");
    check::eq_i32(c.planes[0].oy[1], -44, "regy_var3_oy1_minus44");
    check::eq_i32(c.planes[0].oy[2], -39, "regy_var3_oy2_minus39");
    check::eq_i32(c.planes[0].oz[0], 0, "regy_var3_oz_unchanged_at_180");
    check::eq_i32(r.squash[0][1], 0, "regy_var3_squash_0");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "regy_var3_ctmag_1");

    // zy=0 -> signZ +1 and signY +1: neither branch runs
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, [-50, -45, -40], 0);
    c.zy = 0;
    c.xy = 0;
    let md = madness(1);
    r.regy(0, 150.0, false, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 0, "regy_zy0_both_signs_plus1_draws_0");
    check::eq_i32(c.planes[0].oy[0], -50, "regy_zy0_oy_untouched");

    // negative damage is inert: no abs() around the guard
    let mut thn = Tenths::new(&k.vec);
    let mut rn = Record::new();
    let mut cn = planed(&thn.m, ZERO_OY, 0);
    let mdn = madness(1);
    rn.regy(0, -1000.0, false, &mut cn, &mdn, &mut thn.m);
    check::eq_i32(thn.draws(), 0, "regy_negative_inert_draws_0");
    let mut th1 = Tenths::new(&k.vec);
    let mut r1 = Record::new();
    let mut c1 = planed(&th1.m, ZERO_OY, 0);
    let md1 = madness(1);
    r1.regy(0, 100.0, false, &mut c1, &md1, &mut th1.m);
    check::eq_i32(th1.draws(), 0, "regy_100_noop_draws_0");
    check::eq_i32(regy_npl_zero_draws(k), 0, "regy_empty_fixture_draws_0");
}

/// `regy` on a `npl==0` fixture: useful as a count assertion
/// (`TRecord.java:1674-1680`).
fn regy_npl_zero_draws(k: &K) -> i32 {
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = car(&th.m);
    let md = madness(1);
    r.regy(0, 150.0, false, &mut c, &md, &mut th.m);
    th.draws()
}

/// `regz` (`Record.java:396-469`): same `abs()` guard as `regx`, but the
/// displacement uses `cos(xz)*cos(zy)` for oz and `sin(xz)*cos(xy)` for ox
/// (`:412-413`).
fn regz_group(k: &K) {
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.regz(0, -150.0, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "regz_draws_3");
    check::eq_i32(c.planes[0].ox[0], 0, "regz_ox0_0");
    check::eq_i32(c.planes[0].ox[1], 10, "regz_ox1_10");
    check::eq_i32(c.planes[0].ox[2], 0, "regz_ox2_0");
    check::eq_i32(c.planes[0].oz[0], 0, "regz_oz0_0");
    check::eq_i32(c.planes[0].oz[1], 0, "regz_oz1_0");
    check::eq_i32(c.planes[0].oz[2], 9, "regz_oz2_9");
    check::eq_i32(c.planes[0].chip, 1, "regz_chip_1");
    check::eq_i32(bits(c.planes[0].ctmag), bits(-1.0), "regz_ctmag_minus1");
    check::eq_i32(c.planes[0].bfase, 0, "regz_bfase_untouched_nocol");

    let mut m = Medium::new();
    let mut r100 = Record::new();
    let mut c100 = planed(&m, ZERO_OY, 0);
    let md100 = madness(1);
    r100.regz(0, 100.0, &mut c100, &md100, &mut m);
    check::eq_i32(c100.planes[0].ox[1], 10, "regz_100_noop_ox");
    check::eq_i32(c100.planes[0].chip, 0, "regz_100_noop_chip");
    let mut rneg = Record::new();
    let mut cneg = planed(&m, ZERO_OY, 0);
    let mdneg = madness(1);
    rneg.regz(0, -100.0, &mut cneg, &mdneg, &mut m);
    check::eq_i32(cneg.planes[0].ox[1], 10, "regz_minus100_noop_ox");

    let mut th0 = Tenths::new(&k.vec);
    let mut r0 = Record::new();
    let mut c0 = car(&th0.m);
    let md0 = madness(1);
    r0.regz(0, 150.0, &mut c0, &md0, &mut th0.m);
    check::eq_i32(th0.draws(), 0, "regz_npl0_no_draws");
}

/// `chipx`/`chipz` (`Record.java:739-764`, `:369-394`): identical magnitude
/// handling but they only stamp chip/ctmag; the plane geometry is never
/// touched.
fn chip_group(k: &K) {
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.chipx(0, 150.0, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "chipx_draws_3");
    check::eq_i32(c.planes[0].ox[1], 10, "chipx_plane_unmoved");
    check::eq_i32(c.planes[0].chip, 1, "chipx_chip_1");
    check::eq_i32(bits(c.planes[0].ctmag), bits(1.0), "chipx_ctmag_1");
    check::eq_i32(c.planes[0].bfase, 0, "chipx_bfase_untouched");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.chipz(0, -150.0, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "chipz_draws_3");
    check::eq_i32(c.planes[0].oz[2], 10, "chipz_plane_unmoved");
    check::eq_i32(c.planes[0].chip, 1, "chipz_chip_1");
    check::eq_i32(bits(c.planes[0].ctmag), bits(-1.0), "chipz_ctmag_minus1");

    let mut m = Medium::new();
    let mut r99 = Record::new();
    let mut c99 = planed(&m, ZERO_OY, 0);
    let md99 = madness(1);
    r99.chipz(0, 99.9, &mut c99, &md99, &mut m);
    check::eq_i32(c99.planes[0].chip, 0, "chipz_99_9_noop");
    let mut r100 = Record::new();
    let mut c100 = planed(&m, ZERO_OY, 0);
    let md100 = madness(1);
    r100.chipx(0, -100.0, &mut c100, &md100, &mut m);
    check::eq_i32(c100.planes[0].chip, 0, "chipx_minus100_noop");

    let mut th0 = Tenths::new(&k.vec);
    let mut r0 = Record::new();
    let mut c0 = car(&th0.m);
    let md0 = madness(1);
    r0.chipx(0, 150.0, &mut c0, &md0, &mut th0.m);
    r0.chipz(0, 150.0, &mut c0, &md0, &mut th0.m);
    check::eq_i32(th0.draws(), 0, "chip_npl0_no_draws");
}

/// The damage tint (`Record.java:131-180`, `:417-466`, `:787-836`): a
/// NON-nocol plane is tinted — the HSB clamps are selected by `bfase`, `c[]`
/// is re-derived with `Color.getHSBColor`, then `bfase` advances by the force
/// magnitude. glass planes skip the tint and lose `gr` instead (`:177-179`,
/// `:206-210`).
fn reg_tint(k: &K) {
    let oc = [200, 100, 50];
    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = tinted(&th.m, oc, 25, false, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.regx(0, 150.0, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "tint_regx_draws_3");
    check::eq_i32(
        bits(c.planes[0].hsb[1]),
        bits(0.2),
        "tint_regx_bfase25_hsb1_0_2",
    );
    check::eq_i32(c.planes[0].c[0], 200, "tint_regx_bfase25_c0");
    check::eq_i32(c.planes[0].c[1], 173, "tint_regx_bfase25_c1");
    check::eq_i32(c.planes[0].c[2], 160, "tint_regx_bfase25_c2");
    check::eq_i32(c.planes[0].bfase, 26, "tint_regx_bfase_advances_by_force");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = tinted(&th.m, oc, 65, false, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.regx(0, 150.0, &mut c, &md, &mut th.m);
    // bfase>40/50/60: hsb[1]=0.05, hsb[0]=0.075 -> 0.05
    check::eq_i32(
        bits(c.planes[0].hsb[0]),
        bits(0.05),
        "tint_regx_bfase65_hsb0_0_05",
    );
    check::eq_i32(
        bits(c.planes[0].hsb[1]),
        bits(0.05),
        "tint_regx_bfase65_hsb1_0_05",
    );
    check::eq_i32(c.planes[0].c[0], 200, "tint_regx_bfase65_c0");
    check::eq_i32(c.planes[0].c[1], 193, "tint_regx_bfase65_c1");
    check::eq_i32(c.planes[0].c[2], 190, "tint_regx_bfase65_c2");
    check::eq_i32(c.planes[0].bfase, 66, "tint_regx_bfase65_advance");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = tinted(&th.m, oc, 0, true, 100);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.regx(0, 150.0, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "tint_glass_draws_3");
    check::eq_i32(c.planes[0].bfase, 0, "tint_glass_skips_bfase");
    check::eq_i32(c.planes[0].gr, 98, "tint_glass_gr_100_minus_1_5");
    check::eq_i32(c.planes[0].chip, 1, "tint_glass_chip_1");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = tinted(&th.m, oc, 55, false, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    r.regz(0, -150.0, &mut c, &md, &mut th.m);
    check::eq_i32(
        bits(c.planes[0].hsb[0]),
        bits(0.075),
        "tint_regz_bfase55_hsb0_0_075",
    );
    check::eq_i32(
        bits(c.planes[0].hsb[1]),
        bits(0.05),
        "tint_regz_bfase55_hsb1_0_05",
    );
    check::eq_i32(c.planes[0].c[0], 200, "tint_regz_bfase55_c0");
    check::eq_i32(c.planes[0].c[1], 195, "tint_regz_bfase55_c1");
    check::eq_i32(c.planes[0].c[2], 190, "tint_regz_bfase55_c2");
    check::eq_i32(c.planes[0].bfase, 56, "tint_regz_bfase55_advance");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = tinted(&th.m, oc, 0, false, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 90;
    let md = madness(1);
    r.regy(0, 150.0, false, &mut c, &md, &mut th.m);
    check::eq_i32(th.draws(), 3, "tint_regy_draws_3");
    check::eq_i32(c.planes[0].bfase, 1, "tint_regy_branchA_bfase_advance");
    check::eq_i32(c.planes[0].c[1], 100, "tint_regy_branchA_color_unchanged");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = tinted(&th.m, oc, 0, true, 100);
    c.zy = 180;
    c.xy = 0;
    let md = madness(1);
    r.regy(0, 150.0, false, &mut c, &md, &mut th.m);
    // branch B's glass arm subtracts a flat 5 (:206-210)
    check::eq_i32(c.planes[0].gr, 95, "tint_regy_glass_gr_95");
    check::eq_i32(c.planes[0].bfase, 0, "tint_regy_glass_bfase_untouched");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    c.xz = 45;
    c.xy = 30;
    c.zy = 60;
    let md = madness(1);
    // -250 -> one-shot +100 -> -150, so the force is -7.5*random, not
    // -2.5*random (:772-774)
    r.regx(0, -250.0, &mut c, &md, &mut th.m);
    check::eq_i32(
        bits(c.planes[0].ctmag),
        bits(-3.0),
        "tint_regx_neg250_ctmag_minus3",
    );
    check::eq_i32(c.planes[0].ox[0], -1, "tint_regx_neg250_ox0_minus1");
    check::eq_i32(c.planes[0].oz[2], 11, "tint_regx_neg250_oz2_11");

    let mut th = Tenths::new(&k.vec);
    let mut r = Record::new();
    let mut c = planed(&th.m, ZERO_OY, 0);
    let md = madness(1);
    r.chipx(0, -250.0, &mut c, &md, &mut th.m);
    check::eq_i32(
        bits(c.planes[0].ctmag),
        bits(-3.0),
        "tint_chipx_neg250_ctmag_minus3",
    );
    check::eq_i32(c.planes[0].ox[1], 10, "tint_chipx_neg250_plane_unmoved");

    // angle normalisation (:86-116): zy=200 and xy=200 both normalise to
    // -1, so neither branch runs; zy=90 gives 0 and xy=200 gives -1, so
    // the product is 0 and branch A runs
    let mut th1 = Tenths::new(&k.vec);
    let mut r1 = Record::new();
    let mut c1 = planed(&th1.m, ZERO_OY, 0);
    c1.zy = 200;
    c1.xy = 200;
    let md1 = madness(1);
    r1.regy(0, 150.0, false, &mut c1, &md1, &mut th1.m);
    check::eq_i32(th1.draws(), 0, "tint_regy_200_200_draws_0");
    check::eq_i32(c1.planes[0].oz[0], 0, "tint_regy_200_200_plane_untouched");

    let mut th2 = Tenths::new(&k.vec);
    let mut r2 = Record::new();
    let mut c2 = planed(&th2.m, ZERO_OY, 0);
    c2.zy = 90;
    c2.xy = 200;
    let md2 = madness(1);
    r2.regy(0, 150.0, false, &mut c2, &md2, &mut th2.m);
    check::eq_i32(th2.draws(), 3, "tint_regy_90_200_draws_3");
    check::eq_i32(c2.planes[0].oz[0], 1, "tint_regy_90_200_oz0_1");
}

/// `cotchinow` (`Record.java:652-737`): gated on `caught>=300`, sets `wasted`
/// to its ARGUMENT, snapshots car[0]/squash[0]/fix/dest into the h arrays and
/// sets `hcaught`. The spark and damage rings are COPIED with their counters —
/// nothing is moved or reset.
fn cotchinow_group() {
    let m = Medium::new();
    let mut ct = Record::new();
    ct.reset(&five(&m), &m);
    ct.caught = 299;
    ct.wasted = 99;
    ct.cotchinow(4, &m);
    check::eq_i32(ct.wasted, 99, "cotch_below_300_no_wasted");
    check::ok(!ct.hcaught, "cotch_below_300_no_hcaught");
    check::eq_i32(ct.hfix[0], -1, "cotch_below_300_hfix_untouched");
    check::eq_i32(ct.hx[0][0], 0, "cotch_below_300_hx_untouched");

    ct.caught = 300;
    ct.wasted = 99;
    ct.x[0][0] = 30;
    ct.y[0][0] = 31;
    ct.z[0][0] = 32;
    ct.xy[0][0] = 33;
    ct.zy[0][0] = 34;
    ct.xz[0][0] = 35;
    ct.wxz[0][0] = 36;
    ct.wzy[0][0] = 37;
    ct.x[299][4] = 12345;
    ct.hx[299][4] = 0;
    ct.nry[0][0] = 5;
    ct.nrx[0][0] = 4;
    ct.nrz[0][0] = 3;
    ct.ns[0][0] = 9;
    ct.ry[0][0][1] = 41;
    ct.magy[0][0][1] = 42;
    ct.rx[0][0][2] = 43;
    ct.magx[0][0][2] = 44;
    ct.rz[0][0][3] = 45;
    ct.magz[0][0][3] = 46;
    ct.sspark[0][0][4] = 47;
    ct.sx[0][0][4] = 48;
    ct.sy[0][0][4] = 49;
    ct.sz[0][0][4] = 50;
    ct.smag[0][0][4] = 1.75;
    ct.scx[0][0][4] = 51;
    ct.scz[0][0][4] = 52;
    ct.fulls[0][0][4] = true;
    ct.mtouch[0][6] = true;
    ct.squash[0][0] = 12;
    ct.fix[0] = 13;
    ct.dest[0] = 14;
    let c = car(&m);
    ct.car[0][0] = Some(Conto::clone_from(&c, &m, 0, 0, 0, 0));
    ct.car[0][0].as_mut().unwrap().maxr = 88;
    ct.car[0][0].as_mut().unwrap().x = 999;
    ct.cotchinow(4, &m);
    check::eq_i32(ct.wasted, 4, "cotch_wasted_is_argument");
    check::ok(ct.hcaught, "cotch_hcaught_true");
    check::eq_i32(ct.hx[0][0], 30, "cotch_hx");
    check::eq_i32(ct.hy[0][0], 31, "cotch_hy");
    check::eq_i32(ct.hz[0][0], 32, "cotch_hz");
    check::eq_i32(ct.hxy[0][0], 33, "cotch_hxy");
    check::eq_i32(ct.hzy[0][0], 34, "cotch_hzy");
    check::eq_i32(ct.hxz[0][0], 35, "cotch_hxz");
    check::eq_i32(ct.hwxz[0][0], 36, "cotch_hwxz");
    check::eq_i32(ct.hwzy[0][0], 37, "cotch_hwzy");
    check::eq_i32(ct.hx[299][4], 12345, "cotch_hx_far_corner");
    check::eq_i32(ct.hns[0][0], 9, "cotch_hns_copied");
    check::eq_i32(ct.hnry[0][0], 5, "cotch_hnry_copied");
    check::eq_i32(ct.hnrx[0][0], 4, "cotch_hnrx_copied");
    check::eq_i32(ct.hnrz[0][0], 3, "cotch_hnrz_copied");
    // counters are copied, NOT moved
    check::eq_i32(ct.nry[0][0], 5, "cotch_nry_not_moved");
    check::eq_i32(ct.nrx[0][0], 4, "cotch_nrx_not_moved");
    check::eq_i32(ct.ns[0][0], 9, "cotch_ns_not_moved");
    check::eq_i32(ct.hry[0][0][1], 41, "cotch_hry");
    check::eq_i32(ct.hmagy[0][0][1], 42, "cotch_hmagy");
    check::eq_i32(ct.hrx[0][0][2], 43, "cotch_hrx");
    check::eq_i32(ct.hmagx[0][0][2], 44, "cotch_hmagx");
    check::eq_i32(ct.hrz[0][0][3], 45, "cotch_hrz");
    check::eq_i32(ct.hmagz[0][0][3], 46, "cotch_hmagz");
    check::eq_i32(ct.hsspark[0][0][4], 47, "cotch_hsspark");
    check::eq_i32(ct.hsx[0][0][4], 48, "cotch_hsx");
    check::eq_i32(ct.hsy[0][0][4], 49, "cotch_hsy");
    check::eq_i32(ct.hsz[0][0][4], 50, "cotch_hsz");
    check::eq_i32(bits(ct.hsmag[0][0][4]), bits(1.75), "cotch_hsmag");
    check::eq_i32(ct.hscx[0][0][4], 51, "cotch_hscx");
    check::eq_i32(ct.hscz[0][0][4], 52, "cotch_hscz");
    check::ok(ct.hfulls[0][0][4], "cotch_hfulls");
    check::ok(ct.hmtouch[0][6], "cotch_hmtouch");
    check::eq_i32(ct.hsquash[0], 12, "cotch_hsquash");
    check::eq_i32(ct.hfix[0], 13, "cotch_hfix");
    check::eq_i32(ct.hdest[0], 14, "cotch_hdest");
    // starcar is a clone of car[0], so geometry is zeroed and fields kept
    check::eq_i32(
        ct.starcar[0].as_ref().unwrap().maxr,
        88,
        "cotch_starcar_from_car0",
    );
    check::eq_i32(
        ct.starcar[0].as_ref().unwrap().x,
        0,
        "cotch_starcar_x_zeroed",
    );

    ct.sspark[4][3][29] = 66;
    ct.ry[4][3][6] = 67;
    ct.mtouch[4][6] = true;
    ct.cotchinow(1, &m);
    check::eq_i32(ct.wasted, 1, "cotch_second_call_wasted");
    check::eq_i32(ct.hsspark[4][3][29], 66, "cotch_hsspark_far_corner");
    check::eq_i32(ct.hry[4][3][6], 67, "cotch_hry_far_corner");
    check::ok(ct.hmtouch[4][6], "cotch_hmtouch_far_corner");
}

/// `reset(ContO[])` (`Record.java:223-279`): clears the counters and rebuilds
/// car/squash plus the spark and damage rings — but ONLY for car indices 0..3
/// (`while (++var2 < 4)` at `:276`). Index 4 keeps whatever it held. It never
/// touches cntf, lastfr, the h arrays or x/y/z, and `prepit` gates only the
/// starcar allocation.
fn reset_group() {
    let m = Medium::new();
    let mut rs = Record::new();
    let mut arr = five(&m);
    rs.caught = 7;
    rs.hcaught = true;
    rs.wasted = 8;
    rs.whenwasted = 9;
    rs.powered = 10;
    rs.cntf = 37;
    rs.lastfr = 55;
    rs.fix[0] = 3;
    rs.dest[1] = 4;
    rs.cntdest[2] = 5;
    rs.squash[2][3] = 6;
    rs.sspark[0][0][0] = 71;
    rs.ns[0][0] = 9;
    rs.ry[0][0][0] = 72;
    rs.rx[1][1][1] = 73;
    rs.rz[2][2][2] = 74;
    rs.nry[0][0] = 5;
    rs.nrx[1][1] = 6;
    rs.nrz[2][2] = 7;
    rs.car[3][4] = None;
    // car index 4 is OUTSIDE reset's loops
    rs.sspark[4][0][0] = 81;
    rs.ns[4][0] = 8;
    rs.ry[4][0][0] = 82;
    rs.rx[4][1][0] = 83;
    rs.rz[4][2][0] = 84;
    rs.nry[4][0] = 9;
    rs.nrx[4][1] = 10;
    rs.nrz[4][2] = 11;
    // h side and x/y/z are untouched by reset
    rs.hx[0][0] = 91;
    rs.hry[0][0][0] = 92;
    rs.x[5][0] = 93;
    rs.ocar[0] = Some(car(&m));
    rs.ocar[0].as_mut().unwrap().maxr = 95;
    rs.hcaught = true;

    rs.reset(&arr, &m);
    check::eq_i32(rs.caught, 0, "reset_caught_0");
    check::ok(!rs.hcaught, "reset_hcaught_false");
    check::eq_i32(rs.wasted, 0, "reset_wasted_0");
    check::eq_i32(rs.whenwasted, 0, "reset_whenwasted_0");
    check::eq_i32(rs.powered, 0, "reset_powered_0");
    check::ok(!rs.prepit, "reset_prepit_false");
    check::eq_i32(rs.cntf, 37, "reset_preserves_cntf");
    check::eq_i32(rs.lastfr, 55, "reset_preserves_lastfr");
    for i in 0..5 {
        check::eq_i32(rs.fix[i], -1, &format!("reset_fix{i}_minus1"));
        check::eq_i32(rs.dest[i], -1, &format!("reset_dest{i}_minus1"));
        check::eq_i32(rs.cntdest[i], 0, &format!("reset_cntdest{i}_0"));
        check::eq_i32(
            rs.starcar[i].as_ref().unwrap().maxr,
            30 + i as i32,
            &format!("reset_starcar{i}_from_array"),
        );
    }
    check::eq_bool(rs.car[0][0].is_some(), true, "reset_car00_rebuilt");
    check::eq_i32(
        rs.car[3][4].as_ref().unwrap().maxr,
        34,
        "reset_car34_rebuilt",
    );
    check::eq_i32(rs.squash[2][3], 0, "reset_squash_cleared");
    check::eq_i32(rs.sspark[0][0][0], -1, "reset_sspark_minus1");
    check::eq_i32(rs.sspark[0][0][29], -1, "reset_sspark_last_minus1");
    check::eq_i32(rs.ns[0][0], 0, "reset_ns_0");
    check::eq_i32(rs.ry[0][0][0], -1, "reset_ry_minus1");
    check::eq_i32(rs.rx[1][1][1], -1, "reset_rx_minus1");
    check::eq_i32(rs.rz[2][2][2], -1, "reset_rz_minus1");
    check::eq_i32(rs.nry[0][0], 0, "reset_nry_0");
    check::eq_i32(rs.nrx[1][1], 0, "reset_nrx_0");
    check::eq_i32(rs.nrz[2][2], 0, "reset_nrz_0");
    // car index 4 preserved
    check::eq_i32(rs.sspark[4][0][0], 81, "reset_car4_sspark_preserved");
    check::eq_i32(rs.ns[4][0], 8, "reset_car4_ns_preserved");
    check::eq_i32(rs.ry[4][0][0], 82, "reset_car4_ry_preserved");
    check::eq_i32(rs.rx[4][1][0], 83, "reset_car4_rx_preserved");
    check::eq_i32(rs.rz[4][2][0], 84, "reset_car4_rz_preserved");
    check::eq_i32(rs.nry[4][0], 9, "reset_car4_nry_preserved");
    check::eq_i32(rs.nrx[4][1], 10, "reset_car4_nrx_preserved");
    check::eq_i32(rs.nrz[4][2], 11, "reset_car4_nrz_preserved");
    // h side, x/y/z and ocar preserved
    check::eq_i32(rs.hx[0][0], 91, "reset_preserves_hx");
    check::eq_i32(rs.hry[0][0][0], 92, "reset_preserves_hry");
    check::eq_i32(rs.x[5][0], 93, "reset_preserves_x");
    check::eq_i32(
        rs.ocar[0].as_ref().unwrap().maxr,
        95,
        "reset_preserves_ocar",
    );

    // prepit==false: starcar is not reallocated (identity preserved) and
    // the source array is no longer consulted
    let keep = rs.starcar[0].as_ref().unwrap() as *const Conto;
    arr[0].maxr = 1234;
    rs.reset(&arr, &m);
    check::ok(
        std::ptr::eq(keep, rs.starcar[0].as_ref().unwrap()),
        "reset2_starcar_identity_kept",
    );
    check::eq_i32(
        rs.starcar[0].as_ref().unwrap().maxr,
        30,
        "reset2_starcar_not_refreshed",
    );
    check::ok(!rs.prepit, "reset2_prepit_still_false");

    let mut rs3 = Record::new();
    rs3.prepit = false;
    let arr3 = five(&m);
    rs3.reset(&arr3, &m);
    check::eq_bool(
        rs3.starcar[0].is_none(),
        true,
        "reset_prepit_false_skips_starcar",
    );
    check::eq_bool(
        rs3.car[0][0].is_some(),
        true,
        "reset_prepit_false_still_rebuilds_car",
    );
}

/// `py` (`Record.java:850-852`): two squared differences summed in plain int,
/// so Java's 32-bit wrap applies with no abs, no sqrt and no clamp. Java's
/// `py` is an instance method but reads no state, so the port keeps it an
/// associated function; the pins call it in that form (same arithmetic,
/// same receiver-independence the original has).
fn py_group() {
    check::eq_i32(Record::py(1, 0, 1, 0), 2, "py_1_0_1_0_2");
    check::eq_i32(Record::py(0, 0, 0, 0), 0, "py_same_0");
    check::eq_i32(Record::py(10, 20, 30, 40), 200, "py_10_20_30_40_200");
    check::eq_i32(Record::py(-1, 0, 0, 0), 1, "py_neg1_0_1");
    // pairs are (a,b) and (c,d), not (a,c) and (b,d)
    check::eq_i32(Record::py(-3, -4, -3, -4), 2, "py_neg_pairs_2");
    check::eq_i32(
        Record::py(30000, 0, 0, 0),
        900_000_000,
        "py_30000_900000000",
    );
    // 65536^2 == 4294967296 wraps to exactly 0
    check::eq_i32(Record::py(65536, 0, 0, 0), 0, "py_65536_wraps_to_0");
    check::eq_i32(Record::py(32768, 0, 0, 0), 1_073_741_824, "py_32768_1<<30");
    // 46341^2 == 2147488281 overflows to a negative int
    check::eq_i32(
        Record::py(46341, 0, 0, 0),
        -2_147_479_015,
        "py_46341_wraps_negative",
    );
    // Integer.MIN_VALUE: (x-0)*(x-0) wraps to 0
    check::eq_i32(Record::py(i32::MIN, 0, 0, 0), 0, "py_int_min_wraps_to_0");
    // MAX_VALUE^2 wraps to 1
    check::eq_i32(Record::py(0, i32::MAX, 0, 0), 1, "py_int_max_wraps_to_1");
}
