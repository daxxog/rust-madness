//! `TContODraw.java` — ContO's drawing half: `d()` (:510-643), `pdust`
//! (:58-197), `fixit` (:843-1067), `electrify` (:1069-1211) and `lowshadow`
//! (:710-841). Group "ContODraw" (`Suite.java` position 13), 220 static
//! Check sites. Pin source: `tests/java/TContODraw.java`; narrative:
//! notes/JAVA-BEHAVIOR/B6_conto.md.
//!
//! Every pin here reads FIELDS, never pixels, so the port's documented
//! `fill_polygon` scan-converter stand-in (`canvas.rs`) is not exercised on
//! the assertion side at all — the `graphic()` counterpart is just a
//! `Canvas::new(600, 500)`, the offscreen `BufferedImage` size.
//!
//! Determinism: `Medium.random()` is reached by `pdust` (16 per drawn
//! puff), `fixit` (32) and `electrify` (30/26 per call). The Java class
//! replays the measured seed-101 tenths vector through a `Medium`
//! subclass whose `random()` is `VEC[draws % 48]`; the port replays the
//! same 48 values through `RngSource::Vector`, the established seam (see
//! `mirror/t_record.rs`). Because the port's vector cursor wraps, the
//! fixture is pre-expanded to eight periods (384 entries) so the cursor
//! equals the Java draw counter inside every pinned group (the longest,
//! the fixit cycle, consumes 256 draws). Values are byte-identical to the
//! wrapping form and the shape pin still asserts the TOML vector is 48.
//!
//! `RecordingPlane extends Plane` has no Rust subclass; the draw-order pins
//! read the `#[cfg(test)]` index log `conto::draw_log`, pushed where `d()`
//! dispatches each plane. The mirror assigns its A/B/C tags to the same
//! indices, so the pinned ORDER is the Java order.

use std::panic::AssertUnwindSafe;

use crate::canvas::Canvas;
use crate::conto::{Conto, draw_log};
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::plane::Plane;
use crate::random::RngSource;
use crate::trackers::Trackers;

struct K {
    maxr_draw_gate: i32,
    pdust_impulse: i32,
}

/// The 48-value fixture; a replay vector is the fixture repeated enough
/// periods that no pinned group wraps the port's cursor.
const VEC_LEN: usize = 48;
const PERIODS: usize = 8;

pub(crate) fn run() {
    check::group("ContODraw");
    let kc = Tml::load("constants.toml").table("conto");
    kc.require_keys(&["maxr_draw_gate", "pdust_impulse", "pdust_draws"]);
    let k = K {
        maxr_draw_gate: kc.intv("maxr_draw_gate"),
        pdust_impulse: kc.intv("pdust_impulse"),
    };
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    // shape pin: the replay period is part of the pinned behavior
    check::eq_i32(vec.len() as i32, VEC_LEN as i32, "tenths_vector_len_48");

    dist_and_gate(&k);
    spark_phase(&k);
    dust_counters();
    dust_loop();
    d_sub_calls();
    drawing_errors();
    pdust(&k);
    draw_order();
    fixit();
    electrify();
    lowshadow();
}

/// `TContODraw.TenthsMedium` — `random()` replaced by the measured
/// seed-101 tenths vector, counting draws through the cursor.
struct Tenths {
    m: Medium,
}

impl Tenths {
    fn new(vec: &[f32]) -> Tenths {
        let mut replay = Vec::with_capacity(vec.len() * PERIODS);
        for _ in 0..PERIODS {
            replay.extend_from_slice(vec);
        }
        let mut m = Medium::new();
        m.rng = Some(RngSource::Vector(replay, 0));
        Tenths { m }
    }

    fn draws(&self) -> i32 {
        match &self.m.rng {
            Some(RngSource::Vector(_, i)) => *i as i32,
            _ => 0,
        }
    }
}

/// `TContODraw.FixedMedium` — a constant-valued stream for the formula
/// endpoints (380 − 0·760 etc.), same expanded-period counting.
struct Fixed {
    m: Medium,
}

impl Fixed {
    fn new(v: f32) -> Fixed {
        let mut m = Medium::new();
        m.rng = Some(RngSource::Vector(vec![v; VEC_LEN * PERIODS], 0));
        Fixed { m }
    }

    fn draws(&self) -> i32 {
        match &self.m.rng {
            Some(RngSource::Vector(_, i)) => *i as i32,
            _ => 0,
        }
    }
}

/// `TContODraw.car` (`TContODraw.java:976-978`) — the empty-byte ctor seam.
fn car(m: &Medium) -> Conto {
    Conto::from_bytes(Some(&[]), m)
}

/// `TContODraw.plane` (`TContODraw.java:982-988`) — `npl` 1 over a 3-point
/// plane whose colours are distinct (so `Plane` marks it `!nocol`).
fn plane(m: &Medium) -> Conto {
    let mut c = car(m);
    c.planes.set_slot(
        0,
        plane3(m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [10, 20, 30]),
    );
    c.planes.set_npl(1);
    c
}

fn plane3(m: &Medium, ox: [i32; 3], oz: [i32; 3], oy: [i32; 3], oc: [i32; 3]) -> Plane {
    Plane::new(m, &ox, &oz, &oy, 3, oc, false, 0, 0, 0, 0, 7, 0, false)
}

/// `graphic()` (`TContODraw.java:990-992`) — the offscreen 600x500 image;
/// no pin reads its pixels, so the port's Canvas is the counterpart.
fn graphic() -> Canvas {
    Canvas::new(600, 500)
}

/// B6(28-30,34,41): d() clears dist first (:511-513), the entry gate is the
/// xs/ys/maxR/fade/disp conjunction (:520-524), and dist is the DOUBLY
/// truncated projection distance times grounded (:613-626). The gate needs
/// xs(var2+maxR,var4) > 0; with x=y=z=0 that is 8*maxR-1925 > 0, i.e.
/// maxR >= 241 - at maxR 200/240 nothing draws and dist stays 0.
fn dist_and_gate(k: &K) {
    let mut m = Medium::new();
    let t = Trackers::new();
    let mut g = graphic();

    let mut a = car(&m);
    a.maxr = 200;
    a.dist = 77;
    a.d(&mut g, &mut m, &t);
    check::eq_i32(a.dist, 0, "gate_maxR200_fails_dist_zeroed");

    let mut b = car(&m);
    b.maxr = k.maxr_draw_gate - 1;
    b.d(&mut g, &mut m, &t);
    check::eq_i32(b.dist, 0, "gate_maxR240_fails");

    let mut c = car(&m);
    c.maxr = k.maxr_draw_gate;
    c.d(&mut g, &mut m, &t);
    check::eq_i32(c.dist, 18, "gate_maxR241_passes_dist18");

    let mut d = car(&m);
    d.maxr = 300;
    d.d(&mut g, &mut m, &t);
    check::eq_i32(d.dist, 18, "dist_maxR300_18");
    check::eq_i32(d.x + d.y + d.z, 0, "dist_does_not_move_car");

    let mut half = car(&m);
    half.maxr = 300;
    half.grounded = 0.5;
    half.d(&mut g, &mut m, &t);
    check::eq_i32(half.dist, 9, "dist_grounded_half_9");

    let mut dbl = car(&m);
    dbl.maxr = 300;
    dbl.grounded = 2.0;
    dbl.d(&mut g, &mut m, &t);
    check::eq_i32(dbl.dist, 36, "dist_grounded_double_36");

    // (int)sqrt((int)sqrt(175^2+100^2+150^2)) == 15
    let mut off = car(&m);
    off.maxr = 300;
    off.x = 100;
    off.y = 50;
    off.z = 100;
    off.d(&mut g, &mut m, &t);
    check::eq_i32(off.dist, 15, "dist_offorigin_15");
    off.grounded = 0.5;
    off.d(&mut g, &mut m, &t);
    check::eq_i32(off.dist, 7, "dist_offorigin_half_7");

    // the disp test is the on-screen width minimum
    let mut wide = car(&m);
    wide.maxr = 300;
    wide.disp = 100000;
    wide.dist = 55;
    wide.d(&mut g, &mut m, &t);
    check::eq_i32(wide.dist, 0, "gate_disp_too_wide_fails");

    // the right-edge test (xs(var2-maxR) < m.w) fails for a car parked far
    // to the right: xs(9700,50) is far beyond m.w==550
    let mut far_right = car(&m);
    far_right.maxr = 300;
    far_right.x = 10000;
    far_right.dist = 44;
    far_right.d(&mut g, &mut m, &t);
    check::eq_i32(far_right.dist, 0, "gate_off_right_edge_fails");

    // dist==0 does NOT mean "not drawn": the final stg advance only reads
    // dist, so a car exactly at the camera still advances its dust counters
    let mut drawn = car(&m);
    drawn.maxr = 20;
    drawn.x = 275;
    drawn.y = 200;
    drawn.z = 0;
    drawn.stg[0] = 1;
    drawn.d(&mut g, &mut m, &t);
    check::eq_i32(drawn.dist, 0, "drawn_dist_can_be_zero");
    check::eq_i32(drawn.stg[0], 2, "drawn_dist_zero_still_advances");

    // gate off: no dust work, no Plane.d, no addsp, zero random draws
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut tm = Tenths::new(&vec);
    let mut idle = car(&tm.m);
    tm.m.nsp = 2;
    idle.d(&mut g, &mut tm.m, &t);
    check::eq_i32(idle.dist, 0, "gateoff_dist_0");
    check::eq_i32(tm.draws(), 0, "gateoff_zero_draws");
    check::eq_i32(tm.m.nsp, 2, "gateoff_nsp_unchanged");
}

/// B6(31-34,64): the shadow phase. A plain shadow spawns through
/// Medium.addsp (:559) with (x-m.x, z-m.z, (int)(maxR*0.8)) and saturates at
/// five (Medium.java:445-452); crs (:526-529) and the low-shadow branch
/// (:530-531) never call addsp; a matching tracker routes through s(...,0)
/// but still spawns. Shadow never writes a ContO field.
fn spark_phase(k: &K) {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut g = graphic();

    let mut tm = Tenths::new(&vec);
    let mut c = car(&tm.m);
    c.maxr = 300;
    c.shadow = true;
    c.d(&mut g, &mut tm.m, &t());
    check::eq_i32(tm.m.nsp, 1, "spark_nsp_1");
    check::eq_i32(tm.m.spx[0], 0, "spark_spx_x_minus_mx");
    check::eq_i32(tm.m.spz[0], 0, "spark_spz_z_minus_mz");
    check::eq_i32(tm.m.sprad[0], 240, "spark_sprad_int_maxR_0_8");
    check::eq_i32(tm.draws(), 0, "spark_zero_draws");
    // B6(64): shadow only writes m.nsp/spx/spz/sprad
    check::eq_i32(c.stg[0], 0, "spark_leaves_conto_dust_alone");
    check::eq_i32(c.planes.len() as i32, 0, "spark_leaves_conto_npl");
    check::eq_i32(c.x + c.y + c.z, 0, "spark_leaves_conto_position");
    check::eq_i32(c.dist, 18, "spark_dist_still_projected");
    for _ in 0..5 {
        c.d(&mut g, &mut tm.m, &t());
    }
    check::eq_i32(tm.m.nsp, 5, "spark_saturates_at_5");
    check::eq_i32(tm.m.sprad[4], 240, "spark_slot4_sprad");

    // the spark offsets are car-minus-medium, not absolute: m.x=100/m.z=50
    let mut ox = Tenths::new(&vec);
    let mut off_car = car(&ox.m);
    off_car.maxr = 300;
    off_car.shadow = true;
    off_car.x = 300;
    off_car.z = 200;
    ox.m.x = 100;
    ox.m.z = 50;
    off_car.d(&mut g, &mut ox.m, &t());
    check::eq_i32(ox.m.nsp, 1, "spark_offset_nsp_1");
    check::eq_i32(ox.m.spx[0], 200, "spark_offset_spx_x_minus_mx");
    check::eq_i32(ox.m.spz[0], 150, "spark_offset_spz_z_minus_mz");

    let mut cm = Tenths::new(&vec);
    let mut crs = plane(&cm.m);
    crs.maxr = 300;
    crs.shadow = true;
    cm.m.crs = true;
    crs.d(&mut g, &mut cm.m, &t());
    check::eq_i32(cm.m.nsp, 0, "crs_no_addsp");
    check::eq_i32(cm.draws(), 0, "crs_zero_draws");

    let mut lm = Tenths::new(&vec);
    let mut low = car(&lm.m);
    low.maxr = 300;
    low.shadow = true;
    low.y = 2300;
    lm.m.zy = 90;
    low.d(&mut g, &mut lm.m, &t());
    check::eq_i32(lm.m.nsp, 0, "lowshadow_branch_no_addsp");
    check::eq_i32(low.dist, 46, "lowshadow_branch_dist_46");
    check::eq_i32(lm.draws(), 0, "lowshadow_branch_zero_draws");

    // a tracker under the car sends the planes through s(...,0)... and the
    // addsp call still happens afterwards (:559)
    let mut tt = Tenths::new(&vec);
    let mut tk = Trackers::new();
    tk.nt = 1;
    tk.radx[0] = 100;
    tk.radz[0] = 100;
    let mut trk = car(&tt.m);
    trk.maxr = 300;
    trk.shadow = true;
    trk.planes.set_slot(
        0,
        plane3(&tt.m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [10, 20, 30]),
    );
    trk.planes.set_npl(1);
    trk.d(&mut g, &mut tt.m, &tk);
    check::eq_i32(tt.m.nsp, 1, "tracker_shadow_still_spawns");

    // no tracker: the ground-projected s(...,1) arm runs (:550-556)
    let mut gt = Tenths::new(&vec);
    let mut ground = plane(&gt.m);
    ground.maxr = 300;
    ground.shadow = true;
    ground.d(&mut g, &mut gt.m, &t());
    check::eq_i32(gt.m.nsp, 1, "ground_shadow_arm_spawns");

    // shadow == false writes nothing at all
    let mut off = Tenths::new(&vec);
    let mut plain = car(&off.m);
    plain.maxr = 300;
    plain.d(&mut g, &mut off.m, &t());
    check::eq_i32(off.m.nsp, 0, "noshadow_no_spark");
    check::eq_i32(off.draws(), 0, "noshadow_zero_draws");
    let _ = k;
}

/// B6(35): when the entry gate fails, dist stays 0 and the trailing loop
/// (:630-642) advances every live dust slot: 1->2, 2->3, 3->4, 4->0.
fn dust_counters() {
    let mut m = Medium::new();
    let t = Trackers::new();
    let mut g = graphic();
    let mut c = car(&m);
    c.maxr = 0;
    c.stg[0] = 1;
    c.stg[1] = 4;
    c.stg[2] = 0;
    c.stg[3] = 2;
    c.d(&mut g, &mut m, &t);
    check::eq_i32(c.dist, 0, "counters_gateoff_dist_0");
    check::eq_i32(c.stg[0], 2, "counters_stg0_1_to_2");
    check::eq_i32(c.stg[1], 0, "counters_stg1_4_to_0");
    check::eq_i32(c.stg[2], 0, "counters_stg2_0_stays");
    check::eq_i32(c.stg[3], 3, "counters_stg3_2_to_3");
}

/// B6(36,37,43,44): the gate path runs pdust(slot,g,-1) for live slots
/// (:576-580) then, per drawn plane, pdust(master-1,g,+1) (:606-607). The
/// -1 pass needs dov<0 (or dov==0, which merely promotes dov to 1), so a
/// fulls particle started with dov==0 neither draws nor zeroes this frame.
fn dust_loop() {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut g = graphic();

    let mut tm = Tenths::new(&vec);
    let mut c = car(&tm.m);
    c.maxr = 300;
    c.stg[0] = 1;
    c.dov[0] = -1;
    c.smag[0] = 1.0;
    c.sy[0] = 300;
    c.d(&mut g, &mut tm.m, &t());
    check::eq_i32(c.stg[0], 2, "loop_stg_2");
    check::eq_i32(c.dov[0], -1, "loop_dov_stays_minus1");
    check::eq_i32(c.sy[0], 220, "loop_sy_clamped_250_minus_30");
    check::eq_i32(c.sx[0], 0, "loop_sx_0");
    check::eq_i32(tm.draws(), 16, "loop_pdust_draws_16");
    check::eq_i32(c.dist, 18, "loop_dist_18");

    let mut fm = Tenths::new(&vec);
    let mut fulls = car(&fm.m);
    fulls.maxr = 300;
    fulls.stg[0] = 1;
    fulls.dov[0] = -1;
    fulls.fulls[0] = true;
    fulls.smag[0] = 1.0;
    fulls.d(&mut g, &mut fm.m, &t());
    check::eq_i32(fulls.stg[0], 2, "fulls_stg_2");
    check::eq_i32(fulls.dov[0], 0, "fulls_reaching_stg2_zeroes_dov");
    check::eq_i32(fm.draws(), 16, "fulls_draws_16");

    // master==1 points back at slot 0; the -1 pass with dov==0 only promotes
    // dov, so the only 16 draws come from the +1 master call
    let mut ma = Tenths::new(&vec);
    let mut m1 = plane(&ma.m);
    m1.maxr = 300;
    m1.planes[0].master = 1;
    m1.stg[0] = 1;
    m1.dov[0] = 0;
    m1.d(&mut g, &mut ma.m, &t());
    check::eq_i32(m1.stg[0], 2, "master1_stg_2");
    check::eq_i32(m1.dov[0], -1, "master1_dov_flipped_to_minus1");
    check::eq_i32(ma.draws(), 16, "master1_draws_16");

    let mut mb = Tenths::new(&vec);
    let mut m0 = plane(&mb.m);
    m0.maxr = 300;
    m0.stg[0] = 1;
    m0.dov[0] = 0;
    m0.d(&mut g, &mut mb.m, &t());
    check::eq_i32(m0.stg[0], 1, "master0_stg_stays_1_no_draw");
    check::eq_i32(m0.dov[0], 1, "master0_dov_promoted_1");
    check::eq_i32(mb.draws(), 0, "master0_no_draws");

    let mut mc = Tenths::new(&vec);
    let mut m3 = plane(&mc.m);
    m3.maxr = 300;
    m3.planes[0].master = 3;
    m3.stg[2] = 1;
    m3.dov[2] = 0;
    m3.d(&mut g, &mut mc.m, &t());
    check::eq_i32(m3.stg[2], 2, "master3_slot2_stg_2");
    check::eq_i32(m3.dov[2], -1, "master3_slot2_dov_minus1");
    check::eq_i32(m3.stg[0], 0, "master3_slot0_untouched");
    check::eq_i32(mc.draws(), 16, "master3_draws_16");

    // a master pointing at a dead slot is skipped entirely (:606)
    let mut md = Tenths::new(&vec);
    let mut dead = plane(&md.m);
    dead.maxr = 300;
    dead.planes[0].master = 2;
    dead.stg[1] = 0;
    dead.dov[1] = 0;
    dead.d(&mut g, &mut md.m, &t());
    check::eq_i32(dead.dov[1], 0, "master_dead_slot_dov_untouched");
    check::eq_i32(md.draws(), 0, "master_dead_slot_no_draws");
}

/// B6(53,64): inside the ys gate at :564 d() calls electrify (:565-567)
/// first, then fixit (:569-571), before the dust loop and the av sort. Both
/// are random consumers: 30 for a fresh electrify and 32 for a fixit at
/// fcnt==1.
fn d_sub_calls() {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut g = graphic();
    let mut tm = Tenths::new(&vec);
    let mut c = car(&tm.m);
    c.maxr = 300;
    c.elec = true;
    c.fix = true;
    c.fcnt = 1;
    c.d(&mut g, &mut tm.m, &t());
    check::eq_i32(c.elc[0], 2, "dsub_elec_seeded_elc2");
    check::eq_i32(c.xy, 11, "dsub_elec_xy_11");
    check::eq_i32(c.zy, 0, "dsub_elec_zy_0");
    check::eq_i32(c.fcnt, 2, "dsub_fix_fcnt_advanced_2");
    check::eq_i32(tm.draws(), 62, "dsub_30_plus_32_draws_62");
    check::eq_i32(c.dist, 18, "dsub_dist_18");

    // without the flags neither sub-call runs
    let mut off = Tenths::new(&vec);
    let mut c2 = car(&off.m);
    c2.maxr = 300;
    c2.fcnt = 1;
    c2.d(&mut g, &mut off.m, &t());
    check::eq_i32(off.draws(), 0, "dsub_flags_off_zero_draws");
    check::eq_i32(c2.fcnt, 1, "dsub_flags_off_despite_fcnt");
}

/// B6(38-40): d()'s failure modes are NOT caught. m.fade is int[8]
/// (Medium.java:8) so disline 8 or -1 throws AIOOBE at :523; a null plane
/// throws NPE in the av sort (:588); npl beyond p.length throws AIOOBE at
/// p[120] (:586-588) - but only when the earlier slots are non-null.
fn drawing_errors() {
    let mut m = Medium::new();
    let t = Trackers::new();
    let mut g = graphic();
    for dl in [8, -1] {
        check::throws_it(
            AssertUnwindSafe(|| {
                let mut c = car(&m);
                c.maxr = 300;
                c.disline = dl;
                c.d(&mut g, &mut m, &t);
            }),
            "index out of bounds",
            &format!("err_disline{}_aioobe", if dl < 0 { "neg" } else { "8" }),
        );
    }
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut c = car(&m);
            c.maxr = 300;
            c.planes.set_npl(1);
            c.d(&mut g, &mut m, &t);
        }),
        "NullPointerException",
        "err_null_plane_npe",
    );
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut c = car(&m);
            c.maxr = 300;
            // Java shares one Plane object across the 120 slots; separate
            // equal instances are equivalent here because the sort only
            // reads `av` and the pin trips on p[120] first.
            for i in 0..120 {
                c.planes
                    .set_slot(i, plane3(&m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [1, 2, 3]));
            }
            c.planes.set_npl(121);
            c.d(&mut g, &mut m, &t);
        }),
        "index out of bounds",
        "err_npl121_aioobe",
    );
}

/// B6(43-47): pdust's own contract. The guard is `var3*dov > 0` (:59), so a
/// matching sign draws 16 random values (:124-143) and advances stg
/// (:186-193). dov==0 is NOT a no-op for the -1 call - the else-if at
/// :194-196 promotes it to 1. sy clamps to 250 then falls by
/// 23+7*smag (<=150) or 3+27*smag (>150); sx/sz divide by (stg+1)*smag.
fn pdust(k: &K) {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut g = graphic();

    let mut t1 = Tenths::new(&vec);
    let mut c1 = car(&t1.m);
    c1.stg[0] = 1;
    c1.dov[0] = -1;
    c1.smag[0] = 1.0;
    c1.sy[0] = 100;
    c1.pdust(0, &mut g, &mut t1.m, &t(), -1);
    check::eq_i32(c1.stg[0], 2, "pdust_stg_2");
    check::eq_i32(c1.sy[0], 70, "pdust_sy_100_minus_30");
    check::eq_i32(t1.draws(), 16, "pdust_draws_16");

    // mismatched signs are inert
    let mut t2 = Tenths::new(&vec);
    let mut c2 = car(&t2.m);
    c2.stg[0] = 1;
    c2.dov[0] = 1;
    c2.sy[0] = 200;
    c2.pdust(0, &mut g, &mut t2.m, &t(), -1);
    check::eq_i32(c2.stg[0], 1, "pdust_minus1_with_dov_pos_inert");
    check::eq_i32(c2.dov[0], 1, "pdust_minus1_with_dov_pos_keeps_dov");
    check::eq_i32(c2.sy[0], 200, "pdust_minus1_with_dov_pos_keeps_sy");
    check::eq_i32(t2.draws(), 0, "pdust_minus1_with_dov_pos_zero_draws");

    let mut t3 = Tenths::new(&vec);
    let mut c3 = car(&t3.m);
    c3.stg[0] = 1;
    c3.dov[0] = -1;
    c3.sy[0] = 200;
    c3.pdust(0, &mut g, &mut t3.m, &t(), 1);
    check::eq_i32(c3.stg[0], 1, "pdust_plus1_with_dov_neg_inert");
    check::eq_i32(t3.draws(), 0, "pdust_plus1_with_dov_neg_zero_draws");

    // dov==0 promotes to 1 for EITHER sign argument (the else-if at :194)
    let mut t4 = Tenths::new(&vec);
    let mut c4 = car(&t4.m);
    c4.stg[0] = 1;
    c4.sy[0] = 200;
    c4.pdust(0, &mut g, &mut t4.m, &t(), -1);
    check::eq_i32(c4.dov[0], 1, "pdust_minus1_dov0_promotes_1");
    check::eq_i32(c4.stg[0], 1, "pdust_minus1_dov0_no_draw");
    check::eq_i32(t4.draws(), 0, "pdust_minus1_dov0_zero_draws");

    let mut t5 = Tenths::new(&vec);
    let mut c5 = car(&t5.m);
    c5.stg[0] = 1;
    c5.pdust(0, &mut g, &mut t5.m, &t(), 1);
    check::eq_i32(c5.dov[0], 1, "pdust_plus1_dov0_promotes_1");
    check::eq_i32(t5.draws(), 0, "pdust_plus1_dov0_zero_draws");

    // full phase cycle 1->2->3->4->0, one 30-unit fall per pass
    let mut t6 = Tenths::new(&vec);
    let mut c6 = car(&t6.m);
    c6.dov[0] = -1;
    c6.smag[0] = 1.0;
    c6.sy[0] = 100;
    let sy_want = [70, 40, 10, -20, -50];
    for i in 0..5 {
        c6.pdust(0, &mut g, &mut t6.m, &t(), -1);
        check::eq_i32(
            c6.stg[0],
            if i == 4 { 0 } else { i + 1 },
            &format!("pdust_cycle_stg{i}"),
        );
        check::eq_i32(c6.sy[0], sy_want[i as usize], &format!("pdust_cycle_sy{i}"));
    }
    check::eq_i32(t6.draws(), 80, "pdust_cycle_draws_80");

    // the >150 arm: 3+27*smag instead of 23+7*smag, and the drift divides
    let mut t7 = Tenths::new(&vec);
    let mut c7 = car(&t7.m);
    c7.stg[0] = 1;
    c7.dov[0] = -1;
    c7.smag[0] = 2.0;
    c7.sy[0] = 300;
    c7.scx[0] = k.pdust_impulse + 51;
    c7.pdust(0, &mut g, &mut t7.m, &t(), -1);
    check::eq_i32(c7.sy[0], 193, "pdust_sy151_250_minus_57");
    check::eq_i32(c7.sx[0], 37, "pdust_sx_drift_37");

    // float->int cast pins: x/0.0f is +/-Infinity (MIN/MAX_VALUE) and 0/0 is NaN (0)
    let mut t8 = Tenths::new(&vec);
    let mut c8 = car(&t8.m);
    c8.stg[0] = 1;
    c8.dov[0] = -1;
    c8.smag[0] = 0.0;
    c8.sx[0] = 999;
    c8.sz[0] = 888;
    c8.pdust(0, &mut g, &mut t8.m, &t(), -1);
    check::eq_i32(c8.sx[0], 0, "pdust_smag0_scx0_nan_to_0");
    check::eq_i32(c8.sz[0], 0, "pdust_smag0_scz0_nan_to_0");

    let mut t9 = Tenths::new(&vec);
    let mut c9 = car(&t9.m);
    c9.stg[0] = 1;
    c9.dov[0] = -1;
    c9.smag[0] = 0.0;
    c9.scx[0] = k.pdust_impulse;
    c9.pdust(0, &mut g, &mut t9.m, &t(), -1);
    check::eq_i32(c9.sx[0], i32::MAX, "pdust_smag0_scx100_inf_to_max");

    let mut t10 = Tenths::new(&vec);
    let mut c10 = car(&t10.m);
    c10.stg[0] = 1;
    c10.dov[0] = -1;
    c10.smag[0] = 0.0;
    c10.scx[0] = -k.pdust_impulse;
    c10.pdust(0, &mut g, &mut t10.m, &t(), -1);
    check::eq_i32(c10.sx[0], i32::MIN, "pdust_smag0_scxneg100_inf_to_min");

    // the clamp at :86-88 happens before the fall
    let mut t11 = Tenths::new(&vec);
    let mut c11 = car(&t11.m);
    c11.stg[0] = 1;
    c11.dov[0] = -1;
    c11.smag[0] = 1.0;
    c11.sy[0] = 300;
    c11.pdust(0, &mut g, &mut t11.m, &t(), -1);
    check::eq_i32(c11.sy[0], 220, "pdust_sy300_clamps_250_then_falls");

    // a tracker over the spark recolours the polygon (:71-83) and, when it
    // is not skidded, feeds var4 = stg^3+2 (:76-78); no ContO field changes
    let mut t12 = Tenths::new(&vec);
    let mut tk = Trackers::new();
    tk.nt = 1;
    tk.radx[0] = 100;
    tk.radz[0] = 100;
    tk.c[0][0] = 10;
    tk.c[0][1] = 20;
    tk.c[0][2] = 30;
    let mut c12 = car(&t12.m);
    c12.stg[0] = 1;
    c12.dov[0] = -1;
    c12.smag[0] = 1.0;
    c12.sy[0] = 100;
    c12.pdust(0, &mut g, &mut t12.m, &tk, -1);
    check::eq_i32(c12.stg[0], 2, "pdust_tracker_stg_2");
    check::eq_i32(c12.sy[0], 70, "pdust_tracker_sy_70");
    check::eq_i32(t12.draws(), 16, "pdust_tracker_draws_16");

    // skidded tracker: the var4 arm at :76-78 is skipped
    let mut t13 = Tenths::new(&vec);
    let mut tk2 = Trackers::new();
    tk2.nt = 1;
    tk2.radx[0] = 100;
    tk2.radz[0] = 100;
    tk2.skd[0] = 1;
    let mut c13 = car(&t13.m);
    c13.stg[0] = 1;
    c13.dov[0] = -1;
    c13.smag[0] = 1.0;
    c13.sy[0] = 100;
    c13.pdust(0, &mut g, &mut t13.m, &tk2, -1);
    check::eq_i32(c13.stg[0], 2, "pdust_skid_stg_2");
    check::eq_i32(t13.draws(), 16, "pdust_skid_draws_16");

    // a far spark makes the projected depth exceed fade[0..2] (:105-111);
    // still only colour maths, so the counters are unchanged
    let mut t14 = Tenths::new(&vec);
    let mut c14 = car(&t14.m);
    c14.stg[0] = 1;
    c14.dov[0] = -1;
    c14.smag[0] = 1.0;
    c14.sx[0] = 5000;
    c14.sy[0] = 100;
    c14.sz[0] = 5000;
    c14.pdust(0, &mut g, &mut t14.m, &t(), -1);
    check::eq_i32(c14.stg[0], 2, "pdust_far_stg_2");
    check::eq_i32(c14.sx[0], 5000, "pdust_far_sx_5000");
    check::eq_i32(c14.sy[0], 70, "pdust_far_sy_70");

    // a spark far BELOW the viewport makes the projected vertex height
    // negative (<45), which resets m.flex to 0 (:126-128 and :168-170)
    let mut t15 = Tenths::new(&vec);
    let mut c15 = car(&t15.m);
    c15.stg[0] = 1;
    c15.dov[0] = -1;
    c15.smag[0] = 1.0;
    c15.sy[0] = -5000;
    t15.m.flex = 5;
    c15.pdust(0, &mut g, &mut t15.m, &t(), -1);
    check::eq_i32(c15.stg[0], 2, "pdust_flex_stg_2");
    check::eq_i32(t15.m.flex, 0, "pdust_flex_reset_0");
    check::eq_i32(t15.draws(), 16, "pdust_flex_draws_16");
}

/// B6(42): the av sort. Ranks count planes with a LARGER av, so the draw
/// order is DESCENDING av (:586-600); on an exact tie the higher index gets
/// the higher rank and is drawn later. The `RecordingPlane` order log is the
/// port's `conto::draw_log` (see the module header).
fn draw_order() {
    let mut m = Medium::new();
    let t = Trackers::new();
    let mut g = graphic();

    let mut c1 = car(&m);
    c1.maxr = 300;
    for (i, av) in [5, 1, 1].iter().enumerate() {
        let mut p = plane3(&m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [1, 2, 3]);
        p.av = *av;
        c1.planes.set_slot(i, p);
    }
    c1.planes.set_npl(3);
    draw_log::clear();
    c1.d(&mut g, &mut m, &t);
    check::eq_str(
        tags(&[("A", 0), ("B", 1), ("C", 2)], &draw_log::entries()),
        "[A, B, C]",
        "order_av_5_1_1_descending",
    );
    check::eq_i32(c1.dist, 18, "order_gate_passed");

    let mut c2 = car(&m);
    c2.maxr = 300;
    for (i, av) in [1, 5, 3].iter().enumerate() {
        let mut p = plane3(&m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [1, 2, 3]);
        p.av = *av;
        c2.planes.set_slot(i, p);
    }
    c2.planes.set_npl(3);
    draw_log::clear();
    c2.d(&mut g, &mut m, &t);
    check::eq_str(
        tags(&[("A", 0), ("B", 1), ("C", 2)], &draw_log::entries()),
        "[B, C, A]",
        "order_av_1_5_3_descending",
    );

    let mut c3 = car(&m);
    c3.maxr = 300;
    for i in 0..3 {
        let mut p = plane3(&m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [1, 2, 3]);
        p.av = 2;
        c3.planes.set_slot(i, p);
    }
    c3.planes.set_npl(3);
    draw_log::clear();
    c3.d(&mut g, &mut m, &t);
    check::eq_str(
        tags(&[("A", 0), ("B", 1), ("C", 2)], &draw_log::entries()),
        "[A, B, C]",
        "order_tie_ascending_index",
    );

    // a single plane still draws exactly once
    let mut c4 = car(&m);
    c4.maxr = 300;
    let mut p = plane3(&m, [0, 10, 0], [0, 0, 10], [0, 0, 0], [1, 2, 3]);
    p.av = 0;
    c4.planes.set_slot(0, p);
    c4.planes.set_npl(1);
    draw_log::clear();
    c4.d(&mut g, &mut m, &t);
    check::eq_str(
        tags(&[("Z", 0)], &draw_log::entries()),
        "[Z]",
        "order_single_plane",
    );
}

/// Formats the logged plane indices as the Java `List<String>.toString()`
/// shape, mapping each index to the fixture's tag (a missing tag means the
/// port drew a plane the fixture did not).
fn tags(map: &[(&str, usize)], log: &[usize]) -> String {
    let names: Vec<&str> = log
        .iter()
        .map(|i| {
            map.iter()
                .find(|(_, idx)| idx == i)
                .map(|(name, _)| *name)
                .unwrap_or("?")
        })
        .collect();
    format!("[{}]", names.join(", "))
}

/// B6(50-52): fixit's counter machine. fcnt advances every call (:1061-1066;
/// 8 resets to 0 and clears fix); the drawing block runs for fcnt==1 or
/// fcnt>2 (:894) and costs exactly 32 random draws (:949-964,:1014-1029).
/// fcnt==1 rewrites every plane's hsb to Color.RGBtoHSB of
/// getHSBColor(0.57,0.8,0.8) after the snap blend, and sets flx=1 (:844-880).
fn fixit() {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut g = graphic();

    let mut tm = Tenths::new(&vec);
    let mut c = plane(&tm.m);
    c.fix = true;
    let mut prev = 0;
    for i in 1..=8 {
        c.fixit(&mut g, &mut tm.m);
        check::eq_i32(c.fcnt, i, &format!("fixit_cycle_fcnt{i}"));
        prev = tm.draws();
    }
    check::eq_i32(prev, 192, "fixit_cycle_total_draws_192");
    check::ok(c.fix, "fixit_cycle_fix_still_true");

    let before = tm.draws();
    c.fixit(&mut g, &mut tm.m);
    check::eq_i32(c.fcnt, 0, "fixit_ninth_fcnt_0");
    check::ok(!c.fix, "fixit_ninth_clears_fix");
    check::eq_i32(tm.draws() - before, 32, "fixit_ninth_draws_32");

    // entry fcnt==1: flx + hsb, 32 draws
    let mut t1 = Tenths::new(&vec);
    let mut f1 = plane(&t1.m);
    f1.fcnt = 1;
    f1.fixit(&mut g, &mut t1.m);
    check::eq_i32(f1.planes[0].flx, 1, "fixit_entry1_flx_1");
    check::eq_i32(
        bits(f1.planes[0].hsb[0]),
        1058148276,
        "fixit_entry1_hsb0_bits",
    );
    check::eq_i32(
        bits(f1.planes[0].hsb[1]),
        1061981325,
        "fixit_entry1_hsb1_bits",
    );
    check::eq_i32(
        bits(f1.planes[0].hsb[2]),
        1061997773,
        "fixit_entry1_hsb2_bits",
    );
    check::eq_i32(t1.draws(), 32, "fixit_entry1_draws_32");
    check::eq_i32(f1.fcnt, 2, "fixit_entry1_advances_to_2");

    // entry fcnt==2: flx only, and the drawing block is skipped (2 is not > 2)
    let mut t2 = Tenths::new(&vec);
    let mut f2 = plane(&t2.m);
    let hsb0 = bits(f2.planes[0].hsb[0]);
    let hsb1 = bits(f2.planes[0].hsb[1]);
    let hsb2 = bits(f2.planes[0].hsb[2]);
    f2.fcnt = 2;
    f2.fixit(&mut g, &mut t2.m);
    check::eq_i32(f2.planes[0].flx, 1, "fixit_entry2_flx_1");
    // fcnt==2 leaves the Plane ctor's hsb (from c={10,20,30}) alone
    check::eq_i32(hsb0, 1058362709, "fixit_entry2_hsb0_plane_ctor");
    check::eq_i32(
        bits(f2.planes[0].hsb[0]),
        hsb0,
        "fixit_entry2_hsb0_unchanged",
    );
    check::eq_i32(
        bits(f2.planes[0].hsb[1]),
        hsb1,
        "fixit_entry2_hsb1_unchanged",
    );
    check::eq_i32(
        bits(f2.planes[0].hsb[2]),
        hsb2,
        "fixit_entry2_hsb2_unchanged",
    );
    check::eq_i32(t2.draws(), 0, "fixit_entry2_draws_0");
    check::eq_i32(f2.fcnt, 3, "fixit_entry2_advances_to_3");

    // entry fcnt==4: flx=3, drawing still runs
    let mut t4 = Tenths::new(&vec);
    let mut f4 = plane(&t4.m);
    f4.fcnt = 4;
    f4.fixit(&mut g, &mut t4.m);
    check::eq_i32(f4.planes[0].flx, 3, "fixit_entry4_flx_3");
    check::eq_i32(t4.draws(), 32, "fixit_entry4_draws_32");

    // entry fcnt==7 draws but writes no plane flag
    let mut t7 = Tenths::new(&vec);
    let mut f7 = plane(&t7.m);
    f7.fcnt = 7;
    f7.fixit(&mut g, &mut t7.m);
    check::eq_i32(f7.planes[0].flx, 0, "fixit_entry7_flx_untouched");
    check::eq_i32(t7.draws(), 32, "fixit_entry7_draws_32");
    check::eq_i32(f7.fcnt, 8, "fixit_entry7_advances_to_8");

    // entry fcnt==8: draws once more, then resets
    let mut t8 = Tenths::new(&vec);
    let mut f8 = plane(&t8.m);
    f8.fcnt = 8;
    f8.fixit(&mut g, &mut t8.m);
    check::eq_i32(t8.draws(), 32, "fixit_entry8_draws_32");
    check::eq_i32(f8.fcnt, 0, "fixit_entry8_resets_0");
    check::ok(!f8.fix, "fixit_entry8_clears_fix");

    // entry fcnt==0: counter only
    let mut t0 = Tenths::new(&vec);
    let mut f0 = plane(&t0.m);
    f0.fixit(&mut g, &mut t0.m);
    check::eq_i32(t0.draws(), 0, "fixit_entry0_draws_0");
    check::eq_i32(f0.fcnt, 1, "fixit_entry0_advances_to_1");

    // npl==0: no throw, counter and draws still happen
    let mut tn = Tenths::new(&vec);
    let mut fnn = car(&tn.m);
    fnn.fcnt = 1;
    fnn.fixit(&mut g, &mut tn.m);
    check::eq_i32(tn.draws(), 32, "fixit_npl0_draws_32");
    check::eq_i32(fnn.fcnt, 2, "fixit_npl0_advances_to_2");

    // snap is applied to the getHSBColor result before RGBtoHSB
    let mut ts = Tenths::new(&vec);
    let mut fs = plane(&ts.m);
    ts.m.snap = [50, 50, 50];
    fs.fcnt = 1;
    fs.fixit(&mut g, &mut ts.m);
    check::eq_i32(
        bits(fs.planes[0].hsb[0]),
        1057728519,
        "fixit_snap50_hsb0_bits",
    );
    check::eq_i32(
        bits(fs.planes[0].hsb[1]),
        1061339843,
        "fixit_snap50_hsb1_bits",
    );
    check::eq_i32(
        bits(fs.planes[0].hsb[2]),
        1065353216,
        "fixit_snap50_hsb2_bits",
    );

    // distinct wheel points exercise the py/max comparisons at :920-941
    let mut tk = Tenths::new(&vec);
    let mut fk = plane(&tk.m);
    fk.keyx = [100, 0, -100, 0];
    fk.keyz = [0, 100, 0, -100];
    fk.grat = 50;
    fk.fcnt = 1;
    fk.fixit(&mut g, &mut tk.m);
    check::eq_i32(fk.planes[0].flx, 1, "fixit_keypoints_flx_1");
    check::eq_i32(tk.draws(), 32, "fixit_keypoints_draws_32");
    check::eq_i32(fk.fcnt, 2, "fixit_keypoints_fcnt_2");

    // snap extremes drive the >255 and <0 colour clamps at :850-875
    let mut thigh = Tenths::new(&vec);
    let mut fhigh = plane(&thigh.m);
    thigh.m.snap = [200, 200, 200];
    fhigh.fcnt = 1;
    fhigh.fixit(&mut g, &mut thigh.m);
    check::eq_i32(fhigh.planes[0].flx, 1, "fixit_snap200_flx_1");
    check::eq_i32(thigh.draws(), 32, "fixit_snap200_draws_32");
    let mut tlow = Tenths::new(&vec);
    let mut flow = plane(&tlow.m);
    tlow.m.snap = [-200, -200, -200];
    flow.fcnt = 1;
    flow.fixit(&mut g, &mut tlow.m);
    check::eq_i32(flow.planes[0].flx, 1, "fixit_snapneg200_flx_1");
    check::eq_i32(tlow.draws(), 32, "fixit_snapneg200_draws_32");
}

/// B6(53-55): electrify. Placement is unconditional: !roted -> xy+=11,
/// roted -> zy+=11 (:1206-1210). Each arc seeds edl/edr=(int)(380-random*760)
/// only when its elc==0 (:1073-1077), then flips elc to 0 when
/// elc > random*7, else increments (:1083-1087). An arc costs 15 draws when
/// it seeds (2 seed + 4 state + 1 flip + 8 vertex) and 13 otherwise.
fn electrify() {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let mut g = graphic();

    let mut tm = Tenths::new(&vec);
    let mut c = car(&tm.m);
    c.electrify(&mut g, &mut tm.m);
    check::eq_i32(c.edl[0], 76, "elc_edl0_76");
    check::eq_i32(c.edl[1], -304, "elc_edl1_minus304");
    check::eq_i32(c.edr[0], 380, "elc_edr0_380");
    check::eq_i32(c.edr[1], 0, "elc_edr1_0");
    check::eq_i32(c.elc[0], 2, "elc_elc0_2");
    check::eq_i32(c.elc[1], 2, "elc_elc1_2");
    check::eq_i32(c.xy, 11, "elc_xy_plus11");
    check::eq_i32(c.zy, 0, "elc_zy_untouched");
    check::eq_i32(tm.draws(), 30, "elc_first_call_draws_30");

    let before = tm.draws();
    c.electrify(&mut g, &mut tm.m);
    check::eq_i32(c.edl[0], 76, "elc_second_call_keeps_edl0");
    check::eq_i32(c.edr[0], 380, "elc_second_call_keeps_edr0");
    check::eq_i32(c.elc[0], 3, "elc_second_call_elc0_3");
    check::eq_i32(c.elc[1], 3, "elc_second_call_elc1_3");
    check::eq_i32(c.xy, 22, "elc_second_call_xy_plus11_again");
    check::eq_i32(tm.draws() - before, 26, "elc_second_call_draws_26");

    let mut tr = Tenths::new(&vec);
    let mut rot = car(&tr.m);
    rot.roted = true;
    rot.electrify(&mut g, &mut tr.m);
    check::eq_i32(rot.xy, 0, "roted_xy_untouched");
    check::eq_i32(rot.zy, 11, "roted_zy_plus11");
    check::eq_i32(tr.draws(), 30, "roted_draws_30");

    // flip rule with a fixed draw stream: the check reads the value sampled
    // AFTER 4 state draws, which is VEC[4]==0.3 -> threshold 2.1
    let mut fp = Tenths::new(&vec);
    let mut flip_up = car(&fp.m);
    flip_up.elc[0] = 2;
    flip_up.elc[1] = 5;
    flip_up.electrify(&mut g, &mut fp.m);
    check::eq_i32(flip_up.elc[0], 3, "elc_flip_2_gt_2_1_false_increments_3");
    check::eq_i32(flip_up.elc[1], 6, "elc_flip_5_gt_6_3_false_increments_6");
    check::eq_i32(fp.draws(), 26, "elc_flip_preseeded_draws_26");

    let mut fp2 = Tenths::new(&vec);
    let mut flip_down = car(&fp2.m);
    flip_down.elc[0] = 3;
    flip_down.elc[1] = 4;
    flip_down.electrify(&mut g, &mut fp2.m);
    check::eq_i32(flip_down.elc[0], 0, "elc_flip_3_gt_2_1_true_resets_0");
    check::eq_i32(flip_down.elc[1], 5, "elc_flip_4_gt_6_3_false_increments_5");
    check::eq_i32(fp2.draws(), 26, "elc_flip2_draws_26");

    // a live arc is never re-seeded, so its edl/edr survive
    let mut half = Fixed::new(0.5);
    let mut live = car(&half.m);
    live.elc[0] = 1;
    live.edl[0] = 111;
    live.edr[0] = 222;
    live.elc[1] = 1;
    live.electrify(&mut g, &mut half.m);
    check::eq_i32(live.edl[0], 111, "elc_live_edl_preserved");
    check::eq_i32(live.edr[0], 222, "elc_live_edr_preserved");
    check::eq_i32(live.elc[0], 2, "elc_live_elc0_2");
    check::eq_i32(live.elc[1], 2, "elc_live_elc1_2");

    // formula endpoint: random()==0 -> 380-0 == 380; the flip then sees
    // elc(1) > 0*7 and resets to 0
    let mut zero = Fixed::new(0.0);
    let mut endp = car(&zero.m);
    endp.electrify(&mut g, &mut zero.m);
    check::eq_i32(endp.edl[0], 380, "elc_zero_draw_edl_380");
    check::eq_i32(endp.edr[0], 380, "elc_zero_draw_edr_380");
    check::eq_i32(endp.elc[0], 0, "elc_zero_draw_flip_to_0");
    check::eq_i32(endp.elc[1], 0, "elc_zero_draw_flip_to_0_arc2");
    check::eq_i32(zero.draws(), 30, "elc_zero_draws_30");

    // 0.5 -> 380-380 == 0, flip threshold 3.5 -> increments
    let mut mid = Fixed::new(0.5);
    let mut midc = car(&mid.m);
    midc.electrify(&mut g, &mut mid.m);
    check::eq_i32(midc.edl[0], 0, "elc_half_draw_edl_0");
    check::eq_i32(midc.edr[0], 0, "elc_half_draw_edr_0");
    check::eq_i32(midc.elc[0], 2, "elc_half_draw_increments_2");
    check::eq_i32(midc.elc[1], 2, "elc_half_draw_increments_2_arc2");

    // m.trk overrides the arc colour (:1173-1177) and the clamp branches at
    // :1155-1171 / :1182-1198 run under snap extremes
    let mut ttrk = Tenths::new(&vec);
    let mut trk = car(&ttrk.m);
    ttrk.m.trk = true;
    ttrk.m.snap = [200, 200, 200];
    trk.electrify(&mut g, &mut ttrk.m);
    check::eq_i32(trk.edl[0], 76, "elc_trk_edl_76");
    check::eq_i32(ttrk.draws(), 30, "elc_trk_draws_30");
    let mut tlow = Tenths::new(&vec);
    let mut low = car(&tlow.m);
    tlow.m.snap = [-200, -200, -200];
    low.electrify(&mut g, &mut tlow.m);
    check::eq_i32(low.edl[1], -304, "elc_snapneg_edl1_minus304");
    check::eq_i32(tlow.draws(), 30, "elc_snapneg_draws_30");
}

/// B6(63): lowshadow is Graphics-only - it writes no ContO/Medium field and
/// consumes no random values (planes/shadows are pure locals).
fn lowshadow() {
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("contodraw_tenths");
    let tm = Tenths::new(&vec);
    let mut c = plane(&tm.m);
    c.x = 10;
    c.y = 20;
    c.z = 30;
    c.keyx[0] = 1;
    c.keyz[0] = 2;
    c.planes[0].hsb[0] = 0.25;
    let c0 = c.planes[0].c[0];
    let m_nsp = tm.m.nsp;
    c.lowshadow(&mut graphic(), &tm.m, &t(), 500);
    check::eq_i32(tm.draws(), 0, "lowshadow_zero_draws");
    check::eq_i32(c.x, 10, "lowshadow_x_untouched");
    check::eq_i32(c.y, 20, "lowshadow_y_untouched");
    check::eq_i32(c.z, 30, "lowshadow_z_untouched");
    check::eq_i32(c.planes.len() as i32, 1, "lowshadow_npl_untouched");
    check::eq_i32(c.stg[0], 0, "lowshadow_stg_untouched");
    check::eq_i32(bits(c.smag[0]), bits(0.0), "lowshadow_smag_untouched");
    check::eq_i32(
        bits(c.planes[0].hsb[0]),
        bits(0.25),
        "lowshadow_hsb_untouched",
    );
    check::eq_i32(c.planes[0].c[0], c0, "lowshadow_c_untouched");
    check::eq_i32(c.keyx[0], 1, "lowshadow_keyx_untouched");
    check::eq_i32(tm.m.nsp, m_nsp, "lowshadow_nsp_untouched");

    // with a live tracker the polygon color comes from t.c (:787-789) but
    // still nothing on ContO moves
    let tt = Tenths::new(&vec);
    let mut tk = Trackers::new();
    tk.nt = 1;
    let tc = car(&tt.m);
    tc.lowshadow(&mut graphic(), &tt.m, &tk, 100);
    check::eq_i32(tt.draws(), 0, "lowshadow_tracker_zero_draws");
    check::eq_i32(tk.nt as i32, 1, "lowshadow_tracker_nt_untouched");

    // a tracker whose x/z lands under all four key points takes the
    // recolour arm (:744-792); zy>270 folds and |zy|>90 flips the side
    let tc2 = Tenths::new(&vec);
    let mut tk2 = Trackers::new();
    tk2.nt = 1;
    tk2.radx[0] = 100;
    tk2.radz[0] = 100;
    tk2.y[0] = 120;
    tk2.c[0][0] = 40;
    tk2.c[0][1] = 50;
    tk2.c[0][2] = 60;
    let mut lc = car(&tc2.m);
    lc.zy = 180;
    lc.lowshadow(&mut graphic(), &tc2.m, &tk2, 3000);
    check::eq_i32(tc2.draws(), 0, "lowshadow_tracker_match_zero_draws");
    check::eq_i32(lc.zy, 180, "lowshadow_tracker_match_zy_untouched");
    check::eq_i32(tk2.nt as i32, 1, "lowshadow_tracker_match_nt_untouched");
    lc.zy = 300;
    lc.lowshadow(&mut graphic(), &tc2.m, &tk2, 3000);
    check::eq_i32(tc2.draws(), 0, "lowshadow_zy300_zero_draws");
    check::eq_i32(lc.zy, 300, "lowshadow_zy300_untouched");

    // a tilted tracker enters the sin(zy)/sin(90-zy) and sin(xy) height
    // corrections at :762-784 (still field-free)
    let tc3 = Tenths::new(&vec);
    let mut tk3 = Trackers::new();
    tk3.nt = 1;
    tk3.radx[0] = 100;
    tk3.radz[0] = 100;
    tk3.xy[0] = 30;
    tk3.zy[0] = 20;
    let mut lt = car(&tc3.m);
    lt.keyx[0] = 10;
    lt.keyz[0] = 10;
    lt.lowshadow(&mut graphic(), &tc3.m, &tk3, 500);
    check::eq_i32(tc3.draws(), 0, "lowshadow_tilted_zero_draws");
    check::eq_i32(lt.planes.len() as i32, 0, "lowshadow_tilted_npl_untouched");
    check::eq_i32(tk3.nt as i32, 1, "lowshadow_tilted_nt_untouched");
}

fn t() -> Trackers {
    Trackers::new()
}
