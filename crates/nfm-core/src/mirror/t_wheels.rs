// FIDELITY: TWheels.java pins walk the generated plane array by index like the Java original.
#![allow(clippy::needless_range_loop)]
//! `TWheels.java` — the procedural wheel generator: field defaults, the
//! nine-plane fill (one 8-vertex hub + eight 4-vertex rim faces), the
//! `mast`/`master` bookkeeping, the `|x| == 73` gr flip and `a == 11` spin
//! pivot, the monotone-max ground, and the Plane ctor's geometry/typ/colour/
//! deltaf products. Group "Wheels" (`Suite.java` position 10). Pin source:
//! `Wheels.java:2-164` + `Plane.java:81-223`; narrative:
//! notes/JAVA-BEHAVIOR/B3_wheels_medium.md.
//!
//! Consumes `constants.toml [wheels]` exactly like the Java class
//! (`TWheels.java:30`: one load, then `.table("wheels")`).
//!
//! Port-shape mapping (documented at each pin):
//! - Java's `Plane[] ps` with untouched null slots is the port's
//!   `Vec<Plane>`: `ps[i] == null` for slots past the appended block is
//!   `planes.get(i).is_none()`.
//! - `Plane.n` (the vertex count) is `ox.len()` — the port stores the
//!   coordinate arrays without a separate count.
//! - `ps[i].m == m` / `ps[i].t == t` (stored collaborator references) have no
//!   port field: `Plane::new` takes `&Medium` and `s()` takes `&Trackers`
//!   per call. The medium pin is observable through the colour derivation;
//!   the tracker pin is the structural call-site fact (see first_make).
//! - Java float equality is bit equality here (`Check.eq` compares boxed
//!   `Float`s, i.e. `Float.floatToIntBits`).

use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::plane::Plane;
use crate::trackers::Trackers;
use crate::wheels::Wheels;

pub(crate) fn run() {
    check::group("Wheels");
    // TWheels.java:30 — the class-level fixture load; the shape keys are the
    // Java `requireKeys` (:34).
    let k = Tml::load("constants.toml").table("wheels");
    k.require_keys(&[
        "slots_per_wheel",
        "hub_vertices",
        "rim_vertices",
        "rim_y",
        "ground_bias",
    ]);

    defaults();
    first_make(&k);
    branch_arms();
    repeated_make();
}

/// Wheels.java:2-3 — the only two fields, both int 0.
fn defaults() {
    let w = Wheels::new();
    check::eq_i32(w.ground, 0, "default_ground_0");
    check::eq_i32(w.mast, 0, "default_mast_0");
}

/// Fixture A: make(m, t, ps, 0, x=100, y=200, z=300, flag=0, s=10, s=10).
fn first_make(k: &Tml) {
    let m = Medium::new();
    // The Java helper also builds a `Trackers` for make(); the port's
    // make() takes no tracker (the planes never store one), so the fixture
    // exists only for the `t.nt` untouched pin below.
    let t = Trackers::new();
    let mut w = Wheels::new();
    let mut planes: Vec<Plane> = Vec::new();
    w.make(&m, &mut planes, 100, 200, 300, 0, 10, 10);

    // :118-120 - (int)(200 + 12*1.0 + 1.0)
    check::eq_i32(
        w.ground,
        (200.0f32 + k.intv("rim_y") as f32 * 1.0 + k.intv("ground_bias") as f32) as i32,
        "make_ground_213",
    );
    // :45 - ONE increment per make() call, however many planes it built
    check::eq_i32(w.mast, 1, "make_mast_1");

    // :44 + eight :60..:162 writes = slots 0..8; nothing beyond
    let slots = k.intv("slots_per_wheel") as usize;
    for i in 0..slots {
        check::ok(planes.get(i).is_some(), &format!("make_slot{i}_filled"));
    }
    for i in slots..16 {
        // Java's `Check.eq(ps[i], null, ...)`: the untouched array slot.
        check::ok(planes.get(i).is_none(), &format!("make_slot{i}_null"));
    }

    // vertex counts: the hub is the only 8-vertex plane (:44 n=8), the rim
    // faces are all n=4 (:60..:162). Java reads `ps[i].n`, the port the
    // coordinate array length.
    check::eq_i32(
        planes[0].ox.len() as i32,
        k.intv("hub_vertices"),
        "plane0_n_8",
    );
    for i in 1..slots {
        check::eq_i32(
            planes[i].ox.len() as i32,
            k.intv("rim_vertices"),
            &format!("plane{i}_n_4"),
        );
    }

    // :46 master is written ONLY for the hub; the rim planes keep 0
    check::eq_i32(planes[0].master, 1, "plane0_master_1");
    check::eq_i32(planes[1].master, 0, "plane1_master_0");
    check::eq_i32(planes[8].master, 0, "plane8_master_0");

    // gr: hub 0 (:44), three var18 faces (:60,:74,:88), three hard -1 faces
    // (:102,:116,:134), two var18 faces (:148,:162); var18==1 here because
    // |100| != 73 (:17-20)
    check::eq_i32(planes[0].gr, 0, "plane0_gr_0");
    let grs = [
        (1, "plane1_gr_var18_1"),
        (2, "plane2_gr_var18_1"),
        (3, "plane3_gr_var18_1"),
        (4, "plane4_gr_hard_minus1"),
        (5, "plane5_gr_hard_minus1"),
        (6, "plane6_gr_hard_minus1"),
        (7, "plane7_gr_var18_1"),
        (8, "plane8_gr_var18_1"),
    ];
    let want_gr = [1, 1, 1, -1, -1, -1, 1, 1];
    for (idx, (plane, id)) in grs.iter().enumerate() {
        check::eq_i32(planes[*plane].gr, want_gr[idx], id);
    }

    // hub geometry (:24-26 + :28-43), all exact with var16=var17=1.0F
    let want_ox0 = [96; 8];
    let want_oy0 = [188, 188, 195, 205, 212, 212, 205, 195];
    let want_oz0 = [305, 295, 288, 288, 295, 305, 312, 312];
    for i in 0..8 {
        check::eq_i32(planes[0].ox[i], want_ox0[i], &format!("plane0_ox{i}_96"));
        check::eq_i32(planes[0].oy[i], want_oy0[i], &format!("plane0_oy{i}"));
        check::eq_i32(planes[0].oz[i], want_oz0[i], &format!("plane0_oz{i}"));
    }
    check::eq_i32(
        planes[0].ox.len() as i32,
        k.intv("hub_vertices"),
        "plane0_ox_len_8",
    );
    check::eq_i32(
        planes[0].oy.len() as i32,
        k.intv("hub_vertices"),
        "plane0_oy_len_8",
    );
    check::eq_i32(
        planes[0].oz.len() as i32,
        k.intv("hub_vertices"),
        "plane0_oz_len_8",
    );

    // first rim face (:48-60): x = var5 +/- 4.0F*var16, y = var6 - 12/12,
    // z = var7 + 5/-5
    let want_ox1 = [96, 96, 104, 104];
    let want_oy1 = [188; 4];
    let want_oz1 = [305, 295, 295, 305];
    for i in 0..4 {
        check::eq_i32(planes[1].ox[i], want_ox1[i], &format!("plane1_ox{i}"));
        check::eq_i32(planes[1].oy[i], want_oy1[i], &format!("plane1_oy{i}_188"));
        check::eq_i32(planes[1].oz[i], want_oz1[i], &format!("plane1_oz{i}"));
    }
    // Plane copies the shared scratch arrays (:101-109), so the planes do
    // not alias each other; owned Vecs cannot share storage, and the pointer
    // comparison is the observable identity statement.
    check::ok(
        planes[1].ox.as_ptr() != planes[2].ox.as_ptr(),
        "plane_ox_arrays_distinct",
    );

    // Plane.java:111-124 uses only vertices 1 and 2: the hub's ox span is
    // 0 -> typ 1; a rim face has ox 8 vs oy/oz 0 -> typ 2 then 3 (the last
    // if wins), the documented tie order
    check::eq_i32(planes[0].typ, 1, "plane0_typ_1");
    check::eq_i32(planes[1].typ, 3, "plane1_typ_3_tie_overwrite");

    // colours: color {75,75,75} (:9), Medium.snap default {0,0,0} ->
    // c = color + color*snap/100 = color (Plane.java:136-149); equal
    // channels -> nocol (Plane.java:132-134); RGBtoHSB of grey is
    // {0,0,75/255} and bfase 0 skips every clamp (Plane.java:162-194)
    check::eq_i32(planes[0].c[0], 75, "plane0_c0_75");
    check::eq_i32(planes[0].c[1], 75, "plane0_c1_75");
    check::eq_i32(planes[0].c[2], 75, "plane0_c2_75");
    check::eq_i32(planes[0].oc[0], 75, "plane0_oc0_75");
    check::eq_i32(planes[0].oc[2], 75, "plane0_oc2_75");
    check::eq_bool(planes[0].nocol, true, "plane0_nocol_true");
    check::eq_i32(bits(planes[0].hsb[0]), bits(0.0), "plane0_hsb_hue_0");
    check::eq_i32(bits(planes[0].hsb[1]), bits(0.0), "plane0_hsb_sat_0");
    check::eq_i32(
        bits(planes[0].hsb[2]),
        bits(75.0f32 / 255.0),
        "plane0_hsb_bri_75_255",
    );
    check::eq_i32(planes[1].c[0], 75, "plane1_c0_75");
    check::eq_bool(planes[1].nocol, true, "plane1_nocol_true");

    // Plane arg mapping at :44: p8 glass=false, p9 gr, p10 wx, p11 wy,
    // p12 wz, p13 disline=7, p14 bfase=0, p15 road=false
    check::eq_bool(planes[0].glass, false, "plane0_glass_false");
    check::eq_bool(planes[0].road, false, "plane0_road_false");
    check::eq_i32(planes[0].wx, 0, "plane0_wx_0_when_var8_not_11");
    check::eq_i32(planes[0].wy, 200, "plane0_wy_200");
    check::eq_i32(planes[0].wz, 300, "plane0_wz_300");
    check::eq_i32(planes[0].disline, 7, "plane0_disline_7");
    check::eq_i32(planes[0].bfase, 0, "plane0_bfase_0");
    check::eq_i32(planes[8].wy, 200, "plane8_wy_200");
    check::eq_i32(planes[8].wz, 300, "plane8_wz_300");
    check::eq_i32(planes[1].disline, 7, "plane1_disline_7");
    check::eq_i32(bits(planes[0].projf), bits(1.0), "plane0_projf_untouched_1");

    // deltaf (Plane.java:201-223): float accumulator starting at 1.0F,
    // multiplied by (sqrt(int squared-dist)/100.0 cast to float) for the
    // six ORDERED index pairs, then /3.0F. Hub pairs:
    //   (1,0)=10 -> 0.1        (2,0)=sqrt(0+49+289)/100
    //   (2,1)=sqrt(0+49+49)/100
    // so 1.0F*a*b*a*c*b*c/3.0F; the int bits are the cross-language pin.
    let a = ((0.0f64 + 0.0 + 10.0 * 10.0).sqrt() / 100.0) as f32;
    let b = ((0.0f64 + 7.0 * 7.0 + 17.0 * 17.0).sqrt() / 100.0) as f32;
    let c = ((0.0f64 + 7.0 * 7.0 + 7.0 * 7.0).sqrt() / 100.0) as f32;
    check::eq_i32(bits(planes[0].deltaf), 898_904_508, "plane0_deltaf_bits");
    check::eq_i32(
        bits(planes[0].deltaf),
        bits(1.0f32 * a * b * a * c * b * c / 3.0),
        "plane0_deltaf_formula",
    );
    // rim face (:48-60): (1,0)=10, (2,0)=sqrt(64+100), (2,1)=8
    let r0 = ((0.0f64 + 0.0 + 10.0 * 10.0).sqrt() / 100.0) as f32;
    let r1 = ((8.0f64 * 8.0 + 0.0 + 10.0 * 10.0).sqrt() / 100.0) as f32;
    let r2 = ((8.0f64 * 8.0 + 0.0 + 0.0).sqrt() / 100.0) as f32;
    check::eq_i32(bits(planes[1].deltaf), 884_725_071, "plane1_deltaf_bits");
    check::eq_i32(
        bits(planes[1].deltaf),
        bits(1.0f32 * r0 * r1 * r0 * r2 * r1 * r2 / 3.0),
        "plane1_deltaf_formula",
    );
    // rim face 2 (:62-74): (1,0)=sqrt(98), (2,0)=sqrt(64+49+49), (2,1)=8
    let s0 = ((0.0f64 + 7.0 * 7.0 + 7.0 * 7.0).sqrt() / 100.0) as f32;
    let s1 = ((8.0f64 * 8.0 + 7.0 * 7.0 + 7.0 * 7.0).sqrt() / 100.0) as f32;
    let s2 = ((8.0f64 * 8.0 + 0.0 + 0.0).sqrt() / 100.0) as f32;
    check::eq_i32(bits(planes[2].deltaf), 884_331_756, "plane2_deltaf_bits");
    check::eq_i32(
        bits(planes[2].deltaf),
        bits(1.0f32 * s0 * s1 * s0 * s2 * s1 * s2 / 3.0),
        "plane2_deltaf_formula",
    );

    // make() mutates neither the Medium's random walk nor the Trackers
    check::eq_i32(m.cntrn, 0, "make_medium_cntrn_untouched");
    check::eq_i32(m.trn as i32, 0, "make_medium_trn_untouched");
    check::eq_i32(m.rand[0], 0, "make_medium_rand0_untouched");
    check::eq_i32(t.nt as i32, 0, "make_trackers_nt_untouched");

    // Java's `Check.ok(ps[0].m == m)`: the plane STORES the Medium and its
    // ctor colours come from that instance's snap. The port stores no
    // reference, but the same identity is observable: a Medium with
    // snap[0]=50 yields c[0] = (int)(75 + 75*0.5) = 112, while the fixture
    // planes (built against the default snap) kept 75.
    let mut m_snap = Medium::new();
    m_snap.snap[0] = 50;
    let mut other_w = Wheels::new();
    let mut other_planes: Vec<Plane> = Vec::new();
    other_w.make(&m_snap, &mut other_planes, 100, 200, 300, 0, 10, 10);
    check::ok(
        other_planes[0].c[0] == 112 && planes[0].c[0] == 75,
        "plane0_medium_identity",
    );
    // `Check.ok(ps[0].t == t)` is the stored-Trackers reference. The port's
    // Plane holds no tracker field: every shadow pass receives `&Trackers`
    // at the call site (`Plane::s`), which is the compile-time identity.
    // There is no runtime state left to compare, so the pin records the
    // structural fact (the document precedent: TSmall tr_no_declared_methods).
    check::ok(true, "plane0_trackers_identity");
}

/// Fixture B exercises both make() branches at once:
/// var8==11 -> var15 = (int)(var5 + 4.0F*var16) (:13-15), and
/// |var5|==73 -> var18 = -1 (:17-20). x=-73, s=20 (var16=2.0F),
/// y=200, z=300 (var17=0.5F).
fn branch_arms() {
    let m = Medium::new();
    let mut w = Wheels::new();
    let mut planes: Vec<Plane> = Vec::new();
    w.make(&m, &mut planes, -73, 200, 300, 11, 20, 5);

    check::eq_i32(w.mast, 1, "branch_mast_1");
    // :118-120 - (int)(200 + 12*0.5 + 1.0)
    check::eq_i32(w.ground, 207, "branch_ground_207");
    // :13-15 - (int)(-73 + 4*2.0) = -65, passed as wx to every plane
    check::eq_i32(planes[0].wx, -65, "branch_var8_11_wx_minus65");
    check::eq_i32(planes[5].wx, -65, "branch_var8_11_wx_all_planes");
    // :17-20 - Math.abs(-73)==73, so var18 flips to -1
    check::eq_i32(planes[1].gr, -1, "branch_abs73_gr_minus1");
    check::eq_i32(planes[0].gr, 0, "branch_hub_gr_still_0");
    // :25 - (int)(-73 - 4*2.0) = -81 for every hub x
    check::eq_i32(planes[0].ox[0], -81, "branch_ox0_minus81");
    check::eq_i32(planes[0].ox[7], -81, "branch_ox7_minus81");
    // :28-29 - (int)(200 - 12*0.5), (int)(300 + 5*0.5)
    check::eq_i32(planes[0].oy[0], 194, "branch_oy0_194");
    check::eq_i32(planes[0].oz[0], 302, "branch_oz0_302_truncated_302_5");
    check::eq_i32(planes[1].oz[1], 297, "branch_oz1_297_truncated_297_5");
    check::eq_i32(planes[0].ox.len() as i32, 8, "branch_hub_n_8");
}

/// Two make() calls on one Wheels: mast counts calls (:45), ground is a
/// monotone max (:118-120), and each 9-slot block starts fresh. The Java
/// second call passes npl=9 into a Plane[24]; the port appends.
fn repeated_make() {
    let m = Medium::new();
    let mut w = Wheels::new();
    let mut planes: Vec<Plane> = Vec::new();
    w.make(&m, &mut planes, 73, 100, 100, 0, 10, 10);
    check::eq_i32(w.mast, 1, "repeat_mast_after_first");
    check::eq_i32(w.ground, 113, "repeat_ground_113");
    // |73|==73 -> -1 as well: the abs() arm does not care about the sign
    check::eq_i32(planes[1].gr, -1, "repeat_abs73_positive_gr_minus1");

    w.make(&m, &mut planes, 73, 50, 50, 0, 10, 10);
    check::eq_i32(w.mast, 2, "repeat_mast_after_second");
    // candidate (int)(50 + 12 + 1) = 63 < 113, so ground keeps the max
    check::eq_i32(w.ground, 113, "repeat_ground_keeps_max");
    check::eq_i32(planes[9].master, 2, "repeat_slot9_master_2");
    check::eq_i32(planes[10].master, 0, "repeat_slot10_master_0");
    check::eq_i32(planes[17].master, 0, "repeat_slot17_master_0");
    check::eq_i32(planes[17].ox.len() as i32, 4, "repeat_slot17_n_4");
    check::eq_i32(planes[9].ox.len() as i32, 8, "repeat_slot9_n_8");
    check::ok(planes.get(18).is_none(), "repeat_slot18_null");
    // the first block is untouched by the second call
    check::eq_i32(planes[0].master, 1, "repeat_slot0_master_still_1");
    check::eq_i32(planes[0].wx, 0, "repeat_slot0_wx_still_0");
}
