//! `TContO.java` — ContO's line-based `.rad` grammar (the byte[]
//! constructor, ContO.java:207-419), its copy constructor (:421-508), the
//! pure helpers (:199,:645,:649,:683,:702,:1213) and the `dust()` spawn
//! (:660-681). Group "ContO" (`Suite.java` position 12), 338 static Check
//! sites. Pin source: `tests/java/TContO.java`; narrative:
//! notes/JAVA-BEHAVIOR/B6_conto.md.
//!
//! Structure (the campaign's one big storage change, sanctioned by these
//! pins): [`crate::conto::Planes`] is Java's `Plane[] p` + `int npl`, so the
//! byte constructor's fixed 120-slot array with a null tail and the copy
//! constructor's exactly-`npl` array both exist, and a partial parse can be
//! inspected slot by slot.
//!
//! Port-shape mapping:
//! - the byte constructor is [`crate::conto::Conto::from_bytes`]; Java
//!   `new ContO(null, m, t)` is `from_bytes(None, &m)` (the swallowed NPE
//!   leaves the same empty state). `c.m`/`c.t` have no stored counterpart
//!   (the port takes both per call), so the identity pins are the
//!   compile-time identities, like `t_record`;
//! - `c.p.length` is `planes.p_len()`, `c.npl` is `planes.len()`, `p[i]` is
//!   `planes[i]`, and Java's null tail slots are `planes.slot(i).is_none()`;
//! - `c.maxR` is `Conto.maxr`; the staged `<track>` arrays are
//!   `Option<Vec<_>>` (null until `tracks(N)`);
//! - `ContO.getvalue` is the port's crate-level
//!   [`crate::model::getvalue`] (same scan, returning `Err` where Java
//!   throws — the throws-pins map the error text);
//! - `ContO.rot` is the shared [`crate::plane::rot`] over slices of length
//!   `n` (the two copies are textually identical, Plane.java:1016-1025).

use std::panic::AssertUnwindSafe;

use crate::conto::Conto;
use crate::medium::Medium;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::model::getvalue;
use crate::plane::rot;
use crate::trackers::Trackers;

pub(crate) fn run() {
    check::group("ContO");
    byte_defaults();
    null_bytes();
    one_plane();
    partial_planes();
    color_glass_road();
    div_and_gr();
    abort_inside_plane();
    globals();
    disline_ordering();
    tracks();
    wheels();
    wheel_overflow();
    plane_overflow();
    trackers_untouched();
    real_car();
    copy_ctor();
    copy_snap_blend();
    copy_rotation();
    copy_trackers();
    copy_no_catch();
    helpers();
    dust_spawn();
}

fn ascii(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

fn conto(s: &str, m: &Medium) -> Conto {
    Conto::from_bytes(Some(&ascii(s)), m)
}

/// `TContO.slurp` (`TContO.java:773-783`): resolve the read-only asset from
/// the repo root or the package directory (cargo's test cwd).
fn slurp(rel: &str) -> Vec<u8> {
    for cand in [format!("../../{rel}"), rel.to_string()] {
        if let Ok(b) = std::fs::read(&cand) {
            return b;
        }
    }
    panic!("fixture not found: {rel}");
}

/// `ContO.java:10-56`: every field initializer, plus the four fixed-shape
/// arrays. `from_bytes(Some(&[]), ...)` never enters the loop body (:230).
fn byte_defaults() {
    let m = Medium::new();
    let c = Conto::from_bytes(Some(&[]), &m);
    // Java `c.m == m` / `c.t == t`: per-call arguments in the port.
    check::ok(true, "default_m_identity");
    check::ok(true, "default_t_identity");
    check::eq_i32(c.planes.p_len() as i32, 120, "default_p_len_120");
    check::eq_i32(c.planes.len() as i32, 0, "default_npl_0");
    check::eq_i32(c.x, 0, "default_x_0");
    check::eq_i32(c.y, 0, "default_y_0");
    check::eq_i32(c.z, 0, "default_z_0");
    check::eq_i32(c.xz, 0, "default_xz_0");
    check::eq_i32(c.xy, 0, "default_xy_0");
    check::eq_i32(c.zy, 0, "default_zy_0");
    check::eq_i32(c.wxz, 0, "default_wxz_0");
    check::eq_i32(c.wzy, 0, "default_wzy_0");
    check::eq_i32(c.dist, 0, "default_dist_0");
    check::eq_i32(c.maxr, 0, "default_maxR_0");
    check::eq_i32(c.disp, 0, "default_disp_0");
    check::eq_i32(c.disline, 7, "default_disline_7");
    check::ok(!c.shadow, "default_shadow_false");
    check::ok(!c.noline, "default_noline_false");
    check::eq_i32(bits(c.grounded), bits(1.0), "default_grounded_1");
    check::eq_i32(c.grat, 0, "default_grat_0");
    check::eq_i32(c.keyx.len() as i32, 4, "default_keyx_len_4");
    check::eq_i32(c.keyz.len() as i32, 4, "default_keyz_len_4");
    for i in 0..4 {
        check::eq_i32(c.keyx[i], 0, &format!("default_keyx{i}_0"));
        check::eq_i32(c.keyz[i], 0, &format!("default_keyz{i}_0"));
        check::eq_i32(c.sx[i], 0, &format!("default_sx{i}_0"));
        check::eq_i32(c.sy[i], 0, &format!("default_sy{i}_0"));
        check::eq_i32(c.sz[i], 0, &format!("default_sz{i}_0"));
        check::eq_i32(c.stg[i], 0, &format!("default_stg{i}_0"));
        check::eq_i32(c.dov[i], 0, &format!("default_dov{i}_0"));
        check::eq_i32(bits(c.smag[i]), bits(0.0), &format!("default_smag{i}_0"));
        check::eq_i32(c.scx[i], 0, &format!("default_scx{i}_0"));
        check::eq_i32(c.scz[i], 0, &format!("default_scz{i}_0"));
        check::ok(!c.fulls[i], &format!("default_fulls{i}_false"));
    }
    // tracker arrays stay null until a tracks(N) line (:29-39,:292-305)
    check::ok(c.txy.is_none(), "default_txy_null");
    check::ok(c.tzy.is_none(), "default_tzy_null");
    check::ok(c.tc.is_none(), "default_tc_null");
    check::ok(c.tradx.is_none(), "default_tradx_null");
    check::ok(c.tradz.is_none(), "default_tradz_null");
    check::ok(c.trady.is_none(), "default_trady_null");
    check::ok(c.tx.is_none(), "default_tx_null");
    check::ok(c.ty.is_none(), "default_ty_null");
    check::ok(c.tz.is_none(), "default_tz_null");
    check::ok(c.skd.is_none(), "default_skd_null");
    check::ok(c.dam.is_none(), "default_dam_null");
    check::eq_i32(c.tnt as i32, 0, "default_tnt_0");
    check::ok(!c.elec, "default_elec_false");
    check::ok(!c.roted, "default_roted_false");
    check::eq_i32(c.edl.len() as i32, 2, "default_edl_len_2");
    check::eq_i32(c.edr.len() as i32, 2, "default_edr_len_2");
    check::eq_i32(c.elc.len() as i32, 2, "default_elc_len_2");
    check::eq_i32(c.edl[1] + c.edr[1] + c.elc[1], 0, "default_arc_arrays_zero");
    check::ok(!c.fix, "default_fix_false");
    check::eq_i32(c.fcnt, 0, "default_fcnt_0");
}

/// B6(2): `new ContO(null, m, t)` NPEs inside the try (:227), the catch at
/// :411 swallows it, :418 still sets grat. Never assert the print.
fn null_bytes() {
    let m = Medium::new();
    let c = Conto::from_bytes(None, &m);
    check::ok(true, "null_bytes_no_throw");
    check::eq_i32(c.planes.p_len() as i32, 120, "null_bytes_p_len_120");
    check::eq_i32(c.planes.len() as i32, 0, "null_bytes_npl_0");
    check::eq_i32(c.grat, 0, "null_bytes_grat_0");
}

/// B6(3): one plane maps file x/y/z to ox/oy/oz (:255-257,:268); maxR is the
/// max (int)sqrt over the SCALED points (:258-261) — here (10,20,60).
fn one_plane() {
    let m = Medium::new();
    let c = conto("<p>\np(10,20,30)\np(40,20,30)\np(10,20,60)\n</p>\n", &m);
    check::eq_i32(c.planes.len() as i32, 1, "plane_npl_1");
    let p = &c.planes[0];
    check::eq_i32(p.ox.len() as i32, 3, "plane_n_3");
    check::eq_i32(p.ox[0], 10, "plane_ox0_10");
    check::eq_i32(p.ox[1], 40, "plane_ox1_40");
    check::eq_i32(p.ox[2], 10, "plane_ox2_10");
    check::eq_i32(p.oy[0], 20, "plane_oy0_20");
    check::eq_i32(p.oy[1], 20, "plane_oy1_20");
    check::eq_i32(p.oy[2], 20, "plane_oy2_20");
    check::eq_i32(p.oz[0], 30, "plane_oz0_30");
    check::eq_i32(p.oz[1], 30, "plane_oz1_30");
    check::eq_i32(p.oz[2], 60, "plane_oz2_60");
    check::eq_i32(c.maxr, 64, "plane_maxR_64");
    check::eq_i32(p.bfase, 0, "plane_bfase_0");
    check::eq_i32(p.wx + p.wy + p.wz, 0, "plane_w_0");
    check::eq_i32(p.disline, 7, "plane_disline_7");
    check::ok(!p.road, "plane_road_default_false");
    check::ok(!p.glass, "plane_glass_default_false");
    check::eq_i32(p.gr, 0, "plane_gr_default_0");
    // |dx|=30 |dy|=0 |dz|=30 -> typ 2 (Plane.java:111-124)
    check::eq_i32(p.typ, 2, "plane_typ_2");
    // oc all zero -> nocol (Plane.java:132-134) and c left at 0
    check::ok(p.nocol, "plane_nocol_true");
    check::eq_i32(p.c[2], 0, "plane_c_default_0");
    check::ok(c.planes.slot(1).is_none(), "plane_slot1_null");
    check::eq_i32(c.grat, 0, "plane_grat_0");
}

/// B6(4,5): fewer than 3 p() lines -> the Plane ctor reads ox[2]
/// (Plane.java:111) and throws AIOOBE, swallowed at :411. `new Plane` is
/// evaluated before npl++ (:268-269), so npl stays 0 while maxR already ate
/// the partial points (:258-261). Everything after the bad block is lost.
fn partial_planes() {
    let m = Medium::new();
    let two = conto("<p>\np(10,20,30)\np(40,20,30)\n</p>\n", &m);
    check::eq_i32(two.planes.len() as i32, 0, "two_point_npl_0");
    check::eq_i32(two.maxr, 53, "two_point_maxR_partial_53");
    check::ok(two.planes.slot(0).is_none(), "two_point_p0_null");

    let none = conto("<p>\n</p>\n", &m);
    check::eq_i32(none.planes.len() as i32, 0, "empty_plane_npl_0");
    check::eq_i32(none.maxr, 0, "empty_plane_maxR_0");

    // the abort kills the rest of the stream: the good third plane never lands
    let after = conto(
        "<p>\np(0,0,0)\np(1,0,0)\n</p>\n<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n",
        &m,
    );
    check::eq_i32(after.planes.len() as i32, 0, "abort_kills_following_planes");
    check::eq_i32(after.maxr, 1, "abort_maxR_from_partial_points");
}

/// B6(6,7,8): the colour triple (:243-248) and the glass flag (:250-252) are
/// parser-scope variables, so both persist across planes; a c() line is what
/// clears glass. road/notroad (:393-399) also persist.
fn color_glass_road() {
    let m = Medium::new();
    let col = conto(
        "<p>\nc(1,2,3)\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n",
        &m,
    );
    check::eq_i32(col.planes.len() as i32, 2, "color_npl_2");
    check::eq_i32(col.planes[1].oc[0], 1, "color_persists_plane2_oc0");
    check::eq_i32(col.planes[1].oc[1], 2, "color_persists_plane2_oc1");
    check::eq_i32(col.planes[1].oc[2], 3, "color_persists_plane2_oc2");

    let glass = conto(
        "<p>\nglass()\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n",
        &m,
    );
    check::ok(glass.planes[0].glass, "glass_plane1_true");
    check::ok(glass.planes[1].glass, "glass_sticky_plane2_true");

    let glass2 = conto(
        "<p>\nglass()\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n<p>\nc(9,9,9)\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n",
        &m,
    );
    check::ok(glass2.planes[0].glass, "glass_plane1_before_c_true");
    check::ok(!glass2.planes[1].glass, "glass_cleared_by_c_line");
    check::eq_i32(glass2.planes[1].oc[0], 9, "glass_plane2_oc_9");

    let road = conto(
        "road\n<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\nnotroad\n<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n<p>\np(0,9,0)\np(1,9,0)\np(2,9,0)\n</p>\n",
        &m,
    );
    check::ok(road.planes[0].road, "road_first_plane_true");
    check::ok(!road.planes[1].road, "road_notroad_second_false");
    check::ok(!road.planes[2].road, "road_notroad_sticky_third_false");
}

/// B6(9,10): div(n) rescales every later coordinate AND key/maxR with
/// n/10f (:405-407,:255-257), while gr is per-plane state that <p> resets
/// to 0 at :235 (so gr only survives inside the block that set it).
fn div_and_gr() {
    let m = Medium::new();
    let c = conto(
        "div(5)\n<p>\np(10,20,30)\np(40,20,30)\np(10,20,60)\n</p>\n",
        &m,
    );
    check::eq_i32(c.planes[0].ox[0], 5, "div_ox0_5");
    check::eq_i32(c.planes[0].ox[1], 20, "div_ox1_20");
    check::eq_i32(c.planes[0].oy[0], 10, "div_oy0_10");
    check::eq_i32(c.planes[0].oz[0], 15, "div_oz0_15");
    check::eq_i32(c.planes[0].oz[2], 30, "div_oz2_30");
    // (int)sqrt(5^2+10^2+30^2)=32.0 -> 32 over the SCALED ints
    check::eq_i32(c.maxr, 32, "div_maxR_32");

    let g = conto(
        "<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n<p>\ngr(3)\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n<p>\np(0,9,0)\np(1,9,0)\np(2,9,0)\n</p>\n",
        &m,
    );
    check::eq_i32(g.planes[0].gr, 0, "gr_default_0");
    check::eq_i32(g.planes[1].gr, 3, "gr_set_3");
    check::eq_i32(g.planes[2].gr, 0, "gr_reset_per_plane_0");
}

/// B6(11): "grounded(50)" inside an open <p> matches the gr prefix test at
/// :239 -> getvalue("gr", "grounded(50)", 0) -> NumberFormatException
/// (measured on :699) -> the whole parse aborts at :411. grounded is never
/// assigned, so it keeps 1.0F, while the already-built state survives.
fn abort_inside_plane() {
    let c = conto(
        "<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n<p>\ngrounded(50)\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n",
        &Medium::new(),
    );
    check::eq_i32(
        c.planes.len() as i32,
        1,
        "grprefix_npl_1_built_state_survives",
    );
    check::eq_i32(
        bits(c.grounded),
        bits(1.0),
        "grprefix_grounded_never_assigned",
    );
    check::eq_i32(c.maxr, 2, "grprefix_maxR_from_plane1");
    // the two p() lines after it never parse
    check::ok(c.planes.slot(1).is_none(), "grprefix_second_plane_absent");
    check::eq_i32(c.disp, 0, "grprefix_disp_0");
}

/// B6(12): the global single-shot lines (:377-403).
fn globals() {
    let c = conto(
        "disp(4)\ndisline(3)\nshadow()\nstonecold()\ngrounded(50)\n",
        &Medium::new(),
    );
    check::eq_i32(c.disp, 4, "global_disp_4");
    check::eq_i32(c.disline, 3, "global_disline_3");
    check::ok(c.shadow, "global_shadow_true");
    check::ok(c.noline, "global_stonecold_noline_true");
    check::eq_i32(bits(c.grounded), bits(0.5), "global_grounded_50_half");
}

/// B6(13): the Plane gets this.disline at construction (:268), so earlier
/// planes keep 7 after a later disline(9).
fn disline_ordering() {
    let c = conto(
        "<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\ndisline(9)\n<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n",
        &Medium::new(),
    );
    check::eq_i32(c.planes[0].disline, 7, "disline_first_plane_keeps_7");
    check::eq_i32(c.planes[1].disline, 9, "disline_second_plane_9");
    check::eq_i32(c.disline, 9, "disline_conto_9");
}

/// B6(14,15,16): tracks(N) allocates twelve arrays (:292-305); <track>
/// zero-fills slot tnt with dam=1/skd=0 (:307-322) and </track> bumps tnt
/// (:372-375); a dam line overrides to 3 (:367-369). No tracks(N): the null
/// this.dam NPEs (:309) - swallowed. Too many blocks: AIOOBE at dam[tnt]
/// (:309) - swallowed with tnt keeping the last good value.
fn tracks() {
    let m = Medium::new();
    let c = conto("tracks(2)\n<track>\n</track>\n<track>\ndam\n</track>\n", &m);
    check::eq_i32(c.tnt as i32, 2, "tracks_tnt_2");
    check::eq_i32(c.dam.as_ref().unwrap().len() as i32, 2, "tracks_dam_len_2");
    check::eq_i32(c.tc.as_ref().unwrap().len() as i32, 2, "tracks_tc_len_2");
    check::eq_i32(c.dam.as_ref().unwrap()[0], 1, "tracks_dam_default_1");
    check::eq_i32(c.skd.as_ref().unwrap()[0], 0, "tracks_skd_default_0");
    check::eq_i32(
        c.tx.as_ref().unwrap()[0] + c.ty.as_ref().unwrap()[0] + c.tz.as_ref().unwrap()[0],
        0,
        "tracks_xyz_default_0",
    );
    check::eq_i32(
        c.txy.as_ref().unwrap()[0]
            + c.tzy.as_ref().unwrap()[0]
            + c.tradx.as_ref().unwrap()[0]
            + c.trady.as_ref().unwrap()[0]
            + c.tradz.as_ref().unwrap()[0],
        0,
        "tracks_angles_default_0",
    );
    check::eq_i32(
        c.tc.as_ref().unwrap()[0][0] + c.tc.as_ref().unwrap()[0][1] + c.tc.as_ref().unwrap()[0][2],
        0,
        "tracks_color_default_0",
    );
    check::eq_i32(c.dam.as_ref().unwrap()[1], 3, "tracks_dam_line_sets_3");

    let f = conto(
        "tracks(1)\n<track>\nc(1,2,3)\nxy(10)\nzy(20)\nradx(30)\nrady(40)\nradz(50)\nty(60)\ntx(70)\ntz(80)\nskid(9)\n</track>\n",
        &m,
    );
    check::eq_i32(f.tnt as i32, 1, "trackrow_tnt_1");
    check::eq_i32(f.txy.as_ref().unwrap()[0], 10, "trackrow_xy_10");
    check::eq_i32(f.tzy.as_ref().unwrap()[0], 20, "trackrow_zy_20");
    check::eq_i32(f.tradx.as_ref().unwrap()[0], 30, "trackrow_radx_30");
    check::eq_i32(f.trady.as_ref().unwrap()[0], 40, "trackrow_rady_40");
    check::eq_i32(f.tradz.as_ref().unwrap()[0], 50, "trackrow_radz_50");
    check::eq_i32(f.ty.as_ref().unwrap()[0], 60, "trackrow_ty_60");
    check::eq_i32(f.tx.as_ref().unwrap()[0], 70, "trackrow_tx_70");
    check::eq_i32(f.tz.as_ref().unwrap()[0], 80, "trackrow_tz_80");
    check::eq_i32(f.skd.as_ref().unwrap()[0], 9, "trackrow_skid_9");
    check::eq_i32(f.dam.as_ref().unwrap()[0], 1, "trackrow_dam_default_1");
    check::eq_i32(f.tc.as_ref().unwrap()[0][0], 1, "trackrow_c0_1");
    check::eq_i32(f.tc.as_ref().unwrap()[0][2], 3, "trackrow_c2_3");

    let orphan = conto("<track>\n</track>\n", &m);
    check::eq_i32(orphan.tnt as i32, 0, "track_without_tracks_tnt_0");
    check::ok(orphan.dam.is_none(), "track_without_tracks_dam_null");

    let over = conto("tracks(1)\n<track>\n</track>\n<track>\n</track>\n", &m);
    check::eq_i32(over.tnt as i32, 1, "tracks_overflow_keeps_last_good_1");
    check::eq_i32(
        over.dam.as_ref().unwrap()[0],
        1,
        "tracks_overflow_slot0_dam_1",
    );
}

/// B6(17,18): w(x,y,z,a,mr,mz) stores keyx/keyz (:274-275, y ignored),
/// appends Wheels.make's NINE planes (:277-289) and leaves grat =
/// Wheels.ground (:418, Wheels.java:118-120). The 5th w line writes
/// keyx[4] and throws AIOOBE at :274 - swallowed. Wheels points never feed
/// maxR (:258 is only reached from the p() branch).
fn wheels() {
    let m = Medium::new();
    let c = conto("w(10,20,30,1,4,4)\n", &m);
    check::eq_i32(c.keyx[0], 10, "wheel_keyx0_10");
    // keyz reads argument 2, so the y=20 argument is not stored on ContO
    check::eq_i32(c.keyz[0], 30, "wheel_keyz0_30");
    check::eq_i32(c.planes.len() as i32, 9, "wheel_npl_9");
    check::eq_i32(c.maxr, 0, "wheel_maxR_0");
    check::eq_i32(c.grat, 25, "wheel_grat_25");
    check::eq_i32(c.tnt as i32, 0, "wheel_tnt_0");
    check::eq_i32(c.planes[0].master, 1, "wheel_p0_master_1");
    check::eq_i32(c.planes[1].master, 0, "wheel_p1_master_0");
    check::eq_i32(c.planes[8].master, 0, "wheel_p8_master_0");
    check::eq_i32(c.planes[0].ox.len() as i32, 8, "wheel_p0_n_8");
    check::eq_i32(c.planes[0].ox[0], 8, "wheel_p0_ox0_8");
    check::eq_i32(c.planes[0].oy[0], 15, "wheel_p0_oy0_15");
    check::eq_i32(c.planes[0].oz[0], 32, "wheel_p0_oz0_32");
    check::eq_i32(c.planes[0].gr, 0, "wheel_p0_gr_0");
    check::eq_i32(c.keyx[1], 0, "wheel_keyx1_untouched");

    // div() scales x/y/z/mr/mz but NOT a (getvalue raw at :285)
    let d = conto("div(5)\nw(0,10,0,11,10,10)\n", &m);
    check::eq_i32(d.keyx[0], 0, "wheeldiv_keyx0_0");
    check::eq_i32(d.keyz[0], 0, "wheeldiv_keyz0_0");
    check::eq_i32(d.grat, 12, "wheeldiv_grat_12");
    check::eq_i32(d.planes[0].wx, 2, "wheeldiv_wx_2_needs_unscaled_a");
    check::eq_i32(d.planes[0].ox[0], -2, "wheeldiv_ox0_minus2");
    check::eq_i32(d.planes[0].oy[0], -1, "wheeldiv_oy0_minus1");
    check::eq_i32(d.planes[0].oz[0], 2, "wheeldiv_oz0_2");
    check::eq_i32(d.planes[1].gr, 1, "wheeldiv_p1_gr_1");
}

/// B6(18): the 5th w() line throws at keyx[4] (:274) with four wheels
/// already appended, so npl==36 and grat survives from Wheels.ground.
fn wheel_overflow() {
    let c = conto(
        "w(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\n",
        &Medium::new(),
    );
    check::eq_i32(c.keyx[0], 1, "wheel5_keyx0_1");
    check::eq_i32(c.keyx[3], 1, "wheel5_keyx3_1");
    check::eq_i32(c.planes.len() as i32, 36, "wheel5_npl_36");
    // (int)(2 + 12*(4/10f) + 1) == 7 (Wheels.java:118-119), set at :418
    check::eq_i32(c.grat, 7, "wheel5_grat_7_from_partial_wheels");
}

/// B6(19): plane 121 indexes p[120] at :268 -> AIOOBE, swallowed; the first
/// 120 planes are intact.
fn plane_overflow() {
    let mut sb = String::new();
    for i in 0..121 {
        sb.push_str(&format!("<p>\np(0,{i},0)\np(1,{i},0)\np(2,{i},0)\n</p>\n"));
    }
    let c = conto(&sb, &Medium::new());
    check::eq_i32(c.planes.len() as i32, 120, "planes121_npl_120");
    check::eq_i32(c.planes.p_len() as i32, 120, "planes121_p_len_120");
    check::eq_i32(c.planes[119].oy[0], 119, "planes121_last_plane_kept");
}

/// B6(20): :207-419 never writes through the Trackers argument (only stores
/// it at :209), so a car full of <track> blocks leaves t.nt at 0.
fn trackers_untouched() {
    let t = Trackers::new();
    let c = conto("tracks(1)\n<track>\nc(1,2,3)\n</track>\n", &Medium::new());
    check::eq_i32(c.tnt as i32, 1, "trackers_untouched_conto_tnt_1");
    check::eq_i32(t.nt as i32, 0, "trackers_untouched_nt_0");
    check::eq_i32(t.x[0], 0, "trackers_untouched_x_0");
    check::eq_i32(t.c[0][0], 0, "trackers_untouched_c_0");
}

/// B6(21): the real read-only car fixture. 26 </p> + 4 wheels x 9 planes =
/// 62 (:268,:289); maxR 90 comes from (40,11,80) (:258-261); grat 57 from
/// Wheels.ground with y=20, mz=30 -> (int)(20+12*3+1) (:418,
/// Wheels.java:118-120).
fn real_car() {
    let rad = slurp("reference/external/catb0t/OBJ/mycars/Simple Car.rad");
    let t = Trackers::new();
    let c = Conto::from_bytes(Some(&rad), &Medium::new());
    check::eq_i32(c.planes.len() as i32, 62, "real_npl_62");
    check::eq_i32(c.maxr, 90, "real_maxR_90");
    check::eq_i32(c.grat, 57, "real_grat_57");
    check::eq_i32(c.keyx[0], -50, "real_keyx0_minus50");
    check::eq_i32(c.keyx[1], 50, "real_keyx1_50");
    check::eq_i32(c.keyx[2], -50, "real_keyx2_minus50");
    check::eq_i32(c.keyx[3], 50, "real_keyx3_50");
    check::eq_i32(c.keyz[0], 60, "real_keyz0_60");
    check::eq_i32(c.keyz[1], 60, "real_keyz1_60");
    check::eq_i32(c.keyz[2], -55, "real_keyz2_minus55");
    check::eq_i32(c.keyz[3], -55, "real_keyz3_minus55");
    check::eq_i32(c.tnt as i32, 0, "real_tnt_0");
    check::eq_i32(t.nt as i32, 0, "real_trackers_nt_0");
    check::eq_i32(c.planes[0].ox.len() as i32, 4, "real_p0_n_4");
    check::eq_i32(c.planes[0].ox[0], 40, "real_p0_ox_40");
    check::eq_i32(c.planes[0].oy[0], -10, "real_p0_oy_minus10");
    check::eq_i32(c.planes[0].oz[0], 70, "real_p0_oz_70");
    check::eq_i32(c.planes[0].oc[0], 78, "real_p0_oc0_78");
    check::eq_i32(c.planes[0].oc[2], 238, "real_p0_oc2_238");
    check::ok(c.planes[5].glass, "real_p5_is_glass_plane");
}

/// B6(22): placement args win, xz/xy/zy are forced to 0, every scalar is
/// copied (:424-431), p is reallocated to npl (:432), and the fields the
/// ctor never touches (tnt, dist, wxz/wzy, elec/roted/fix/fcnt) stay at their
/// initializers.
fn copy_ctor() {
    let m = Medium::new();
    let mut src = conto(
        "<p>\nc(100,50,25)\np(10,20,30)\np(40,20,30)\np(10,20,60)\n</p>\n",
        &m,
    );
    src.xz = 45;
    src.xy = 30;
    src.zy = 60;
    src.wxz = 9;
    src.wzy = 8;
    src.dist = 7;
    src.disp = 4;
    src.disline = 5;
    src.noline = true;
    src.shadow = true;
    src.grounded = 0.5;
    src.grat = 42;
    src.elec = true;
    src.roted = true;
    src.fix = true;
    src.fcnt = 6;
    src.keyx[0] = 11;
    src.keyx[3] = 13;
    src.keyz[1] = 21;
    src.stg[2] = 3;
    src.planes[0].master = 7;
    src.planes[0].bfase = 33;
    src.planes[0].c[0] = 111;
    let c = Conto::clone_into(&src, &m, 100, 200, 300, 0, &mut Trackers::new());
    // Java `c.m == m` / `c.t == src.t`: per-call in the port.
    check::ok(true, "copy_m_identity");
    check::ok(true, "copy_t_shared");
    check::eq_i32(c.x, 100, "copy_x_100");
    check::eq_i32(c.y, 200, "copy_y_200");
    check::eq_i32(c.z, 300, "copy_z_300");
    check::eq_i32(c.xz + c.xy + c.zy, 0, "copy_attitude_zeroed");
    check::eq_i32(c.planes.len() as i32, 1, "copy_npl_1");
    check::eq_i32(c.maxr, 64, "copy_maxR_64");
    check::eq_i32(c.disp, 4, "copy_disp_4");
    check::eq_i32(c.disline, 5, "copy_disline_5");
    check::ok(c.noline, "copy_noline_true");
    check::ok(c.shadow, "copy_shadow_true");
    check::eq_i32(bits(c.grounded), bits(0.5), "copy_grounded_half");
    check::eq_i32(c.grat, 42, "copy_grat_42");
    check::eq_i32(c.planes.p_len() as i32, 1, "copy_p_len_npl");
    check::eq_i32(c.planes[0].ox.len() as i32, 3, "copy_p0_n_3");
    check::eq_i32(c.planes[0].ox[0], 10, "copy_p0_ox0_10");
    check::eq_i32(c.planes[0].oz[2], 60, "copy_p0_oz2_60");
    // rebuilt from oc, NOT from the damaged src.p[0].c
    check::eq_i32(c.planes[0].oc[0], 100, "copy_p0_oc0_100");
    check::eq_i32(c.planes[0].c[0], 100, "copy_p0_c_from_oc_not_src_c");
    check::eq_i32(c.planes[0].bfase, 33, "copy_p0_bfase_33");
    check::eq_i32(c.planes[0].master, 7, "copy_p0_master_7");
    check::eq_i32(c.keyx[0], 11, "copy_keyx0_11");
    check::eq_i32(c.keyx[3], 13, "copy_keyx3_13");
    check::eq_i32(c.keyz[1], 21, "copy_keyz1_21");
    check::eq_i32(c.stg[2], 0, "copy_stg_zeroed");
    check::eq_i32(c.tnt as i32, 0, "copy_tnt_0");
    check::eq_i32(c.dist, 0, "copy_dist_0");
    check::eq_i32(c.wxz + c.wzy, 0, "copy_wxz_wzy_0");
    check::ok(!c.elec, "copy_elec_false");
    check::ok(!c.roted, "copy_roted_false");
    check::ok(!c.fix, "copy_fix_false");
    check::eq_i32(c.fcnt, 0, "copy_fcnt_0");
}

/// B6(23): Plane recomputes c from oc with the CURRENT m.snap
/// (Plane.java:136-149), so setting snap AFTER the src was built changes the
/// copy (and only the copy).
fn copy_snap_blend() {
    let mut m = Medium::new();
    let t = Trackers::new();
    let src = conto(
        "<p>\nc(100,50,25)\np(0,0,0)\np(10,0,0)\np(0,0,10)\n</p>\n",
        &m,
    );
    let _ = t;
    check::eq_i32(src.planes[0].c[0], 100, "snap_src_c_before_0");
    m.snap[0] = 10;
    m.snap[1] = 10;
    m.snap[2] = 10;
    let c = Conto::clone_into(&src, &m, 0, 0, 0, 0, &mut Trackers::new());
    check::eq_i32(src.planes[0].c[1], 50, "snap_src_plane_not_retinted");
    check::eq_i32(c.planes[0].c[0], 110, "snap_copy_c0_110");
    check::eq_i32(c.planes[0].c[1], 55, "snap_copy_c1_55");
    check::eq_i32(c.planes[0].c[2], 27, "snap_copy_c2_27");
    check::eq_i32(c.planes[0].oc[1], 50, "snap_copy_oc1_50");
}

/// B6(24): the copy rotates every plane about (0,0) by arg5 before
/// loadprojf (:461-465, Plane.java:1016-1025). 0 is a no-op; 90 truncates
/// cos(90)=6.123234E-17 to 0, so (100,0)->(0,100) and (100,50)->(-50,100).
fn copy_rotation() {
    let m = Medium::new();
    let src = conto("<p>\np(100,0,0)\np(100,10,0)\np(100,0,50)\n</p>\n", &m);
    let r0 = Conto::clone_into(&src, &m, 0, 0, 0, 0, &mut Trackers::new());
    check::eq_i32(r0.planes[0].ox[0], 100, "rot0_ox0_100");
    check::eq_i32(r0.planes[0].oz[0], 0, "rot0_oz0_0");
    check::eq_i32(r0.planes[0].ox[2], 100, "rot0_ox2_100");
    check::eq_i32(r0.planes[0].oz[2], 50, "rot0_oz2_50");

    let r90 = Conto::clone_into(&src, &m, 0, 0, 0, 90, &mut Trackers::new());
    check::eq_i32(r90.planes[0].ox[0], 0, "rot90_p0_ox0_0");
    check::eq_i32(r90.planes[0].oz[0], 100, "rot90_p0_oz0_100");
    check::eq_i32(r90.planes[0].ox[2], -50, "rot90_p2_ox_minus50");
    check::eq_i32(r90.planes[0].oz[2], 100, "rot90_p2_oz_100");
    // the source plane is untouched by the rotation
    check::eq_i32(src.planes[0].ox[0], 100, "rot90_src_untouched_100");
}

/// B6(25,26): the tracker rows are appended to the SHARED Trackers at
/// t.nt (:467-499); nothing guards duplication, so a second copy appends
/// two more rows. Rows rotate xy/zy, place x/z/y with the copy's x/y/z, copy
/// skd/dam, snap-blend and clamp tc (0..255) and rotate radx/radz with
/// |a|==180 collapsing to 0 (:489-495).
fn copy_trackers() {
    let m = Medium::new();
    let mut t = Trackers::new();
    let src = conto(
        "tracks(2)\n<track>\nc(10,20,30)\nxy(10)\nzy(20)\nradx(30)\nrady(40)\nradz(50)\nty(60)\ntx(70)\ntz(80)\nskid(9)\ndam\n</track>\n<track>\nc(200,200,200)\nxy(0)\nzy(0)\nradx(1)\nrady(2)\nradz(3)\nty(4)\ntx(5)\ntz(6)\n</track>\n",
        &m,
    );
    check::eq_i32(src.tnt as i32, 2, "copytrack_src_tnt_2");
    check::eq_i32(t.nt as i32, 0, "copytrack_byte_ctor_wrote_nothing");

    let c = Conto::clone_into(&src, &m, 100, 200, 300, 0, &mut t);
    check::eq_i32(c.tnt as i32, 0, "copytrack_dst_tnt_stays_0");
    check::eq_i32(t.nt as i32, 2, "copytrack_append_2");
    check::eq_i32(t.xy[0], 10, "copytrack_row0_xy_10");
    check::eq_i32(t.zy[0], 20, "copytrack_row0_zy_20");
    check::eq_i32(t.c[0][0], 10, "copytrack_row0_c0_10");
    check::eq_i32(t.x[0], 170, "copytrack_row0_x_170");
    check::eq_i32(t.y[0], 260, "copytrack_row0_y_260");
    check::eq_i32(t.z[0], 380, "copytrack_row0_z_380");
    check::eq_i32(t.skd[0], 9, "copytrack_row0_skd_9");
    check::eq_i32(t.dam[0], 3, "copytrack_row0_dam_3");
    check::eq_i32(t.radx[0], 30, "copytrack_row0_radx_30");
    check::eq_i32(t.radz[0], 50, "copytrack_row0_radz_50");
    check::eq_i32(t.rady[0], 40, "copytrack_row0_rady_40");
    check::eq_i32(t.x[1], 105, "copytrack_row1_x_105");
    check::eq_i32(t.z[1], 306, "copytrack_row1_z_306");
    check::eq_i32(t.y[1], 204, "copytrack_row1_y_204");
    check::eq_i32(t.radx[1], 1, "copytrack_row1_radx_1");

    // second copy of the same source: two more rows, no guard
    Conto::clone_into(&src, &m, 100, 200, 300, 0, &mut t);
    check::eq_i32(t.nt as i32, 4, "copytrack_second_copy_nt_4");
    check::eq_i32(t.xy[2], 10, "copytrack_row2_xy_10");
    check::eq_i32(t.xy[3], 0, "copytrack_row3_xy_0");

    // 90 degrees: xy=(int)(10*cos90-20*sin90)=-20, zy=10;
    // x=(int)(100+70*cos90-80*sin90)=20, z=370; radx=50, radz=30
    let m90 = Medium::new();
    let mut t90 = Trackers::new();
    let s90 = conto(
        "tracks(1)\n<track>\nc(10,10,10)\nxy(10)\nzy(20)\nradx(30)\nradz(50)\nty(60)\ntx(70)\ntz(80)\n</track>\n",
        &m90,
    );
    Conto::clone_into(&s90, &m90, 100, 200, 300, 90, &mut t90);
    check::eq_i32(t90.xy[0], -20, "copytrack90_xy_minus20");
    check::eq_i32(t90.zy[0], 10, "copytrack90_zy_10");
    check::eq_i32(t90.x[0], 20, "copytrack90_x_20");
    check::eq_i32(t90.z[0], 370, "copytrack90_z_370");
    check::eq_i32(t90.radx[0], 50, "copytrack90_radx_50");
    check::eq_i32(t90.radz[0], 30, "copytrack90_radz_30");
    check::eq_i32(t90.rady[0], 0, "copytrack90_rady_0");

    // 180 degrees: radii use |a|==180 -> 0 (:489-492), so they do NOT flip
    let m180 = Medium::new();
    let mut t180 = Trackers::new();
    let s180 = conto(
        "tracks(1)\n<track>\nc(10,10,10)\nxy(10)\nzy(20)\nradx(30)\nradz(50)\ntx(70)\ntz(80)\n</track>\n",
        &m180,
    );
    Conto::clone_into(&s180, &m180, 0, 0, 0, 180, &mut t180);
    check::eq_i32(t180.radx[0], 30, "copytrack180_radx_unrotated_30");
    check::eq_i32(t180.radz[0], 50, "copytrack180_radz_unrotated_50");
    check::eq_i32(t180.xy[0], -10, "copytrack180_xy_minus10");
    check::eq_i32(t180.zy[0], -20, "copytrack180_zy_minus20");

    // tc snap blend clamps both ends: (-5,300,10) at snap 100 -> (0,255,20)
    let mut mc = Medium::new();
    mc.snap[0] = 100;
    mc.snap[1] = 100;
    mc.snap[2] = 100;
    let mut tc = Trackers::new();
    let sc = conto("tracks(1)\n<track>\nc(-5,300,10)\n</track>\n", &mc);
    Conto::clone_into(&sc, &mc, 0, 0, 0, 0, &mut tc);
    check::eq_i32(tc.c[0][0], 0, "copytrack_tc_clamp_low_0");
    check::eq_i32(tc.c[0][1], 255, "copytrack_tc_clamp_high_255");
    check::eq_i32(tc.c[0][2], 20, "copytrack_tc_snap20");
}

/// B6(27): unlike the byte ctor, the copy ctor has no try/catch, so a
/// src.p==null NPEs out of :438.
fn copy_no_catch() {
    let m = Medium::new();
    let mut src = Conto::from_bytes(Some(&[]), &m);
    src.planes.set_npl(1);
    src.planes.set_null();
    check::throws_it(
        AssertUnwindSafe(|| {
            Conto::clone_from(&src, &m, 0, 0, 0, 0);
        }),
        "NullPointerException",
        "copy_null_p_npe",
    );
}

/// B6(56-62): the pure helpers, all integer-truncating.
fn helpers() {
    let m = Medium::new();
    let c = Conto::from_bytes(Some(&[]), &m);
    // ys/xs clamp depth to 50 first (:200-204,:703-706)
    check::eq_i32(c.ys(&m, 100, 50), -600, "ys_100_50_minus600");
    check::eq_i32(c.ys(&m, 50, 100), -400, "ys_50_100_minus400");
    check::eq_i32(c.ys(&m, 100, 0), -600, "ys_depth0_clamps_50");
    check::eq_i32(c.ys(&m, 0, 49), -1400, "ys_49_clamps_50");
    check::eq_i32(c.ys(&m, 0, 51), -1368, "ys_51_no_clamp");
    check::eq_i32(c.ys(&m, -100, -200), -2200, "ys_negative_depth_clamps_50");
    check::eq_i32(c.xs(&m, 275, 0), 275, "xs_275_0_275");
    check::eq_i32(c.xs(&m, 100, 0), -1125, "xs_100_0_minus1125");
    check::eq_i32(c.xs(&m, 0, 0), -1925, "xs_0_0_minus1925");
    check::eq_i32(c.xs(&m, 0, 49), -1925, "xs_49_clamps_50");
    check::eq_i32(c.xs(&m, 0, 51), -1881, "xs_51_no_clamp");
    check::eq_i32(c.xs(&m, 0, 100), -825, "xs_0_100_minus825");
    check::eq_i32(c.xs(&m, -100, -200), -2725, "xs_negative_depth_clamps_50");

    // getpy (:645-647): /10 then square, per axis, truncating toward zero
    check::eq_i32(c.getpy(25, 25, 25), 12, "getpy_25_12");
    check::eq_i32(c.getpy(-5, 0, 0), 0, "getpy_minus5_truncates_0");
    check::eq_i32(c.getpy(15, 7, 3), 1, "getpy_15_7_3_1");
    let mut c = c;
    c.x = 10;
    c.y = 20;
    c.z = 30;
    check::eq_i32(c.getpy(25, 25, 25), 1, "getpy_offsets_1");

    // py (:1213-1215) - squared distance, no abs and no sqrt
    check::eq_i32(Conto::py(3, 1, 4, 0), 20, "py_3_1_4_0_20");
    check::eq_i32(Conto::py(-3, 3, 0, 4), 52, "py_minus3_3_0_4_52");
    check::eq_i32(Conto::py(0, 0, 0, 0), 0, "py_same_0");

    // rot (:649-658): var5==0 is a no-op; 90 truncates the 6.123234E-17
    // cosine (Medium.java:115) to 0
    let mut a = [100];
    let mut b = [0];
    rot(&m, &mut a[..1], &mut b[..1], 0, 0, 90);
    check::eq_i32(a[0], 0, "rot90_x_0");
    check::eq_i32(b[0], 100, "rot90_y_100");
    let mut a2 = [100];
    let mut b2 = [7];
    rot(&m, &mut a2[..1], &mut b2[..1], 0, 0, 0);
    check::eq_i32(a2[0], 100, "rot0_x_untouched");
    check::eq_i32(b2[0], 7, "rot0_y_untouched");
    let mut a3 = [5, 6, 7];
    let mut b3 = [8, 9, 10];
    rot(&m, &mut a3[..0], &mut b3[..0], 0, 0, 90);
    check::eq_i32(a3[0], 5, "rot_n0_untouched");

    // getvalue (:683-700) — the port's crate-level function (line, key, col)
    check::eq_i32(
        getvalue("p(10,20,30)", "p", 0).unwrap(),
        10,
        "getvalue_p0_10",
    );
    check::eq_i32(
        getvalue("p(10,20,30)", "p", 1).unwrap(),
        20,
        "getvalue_p1_20",
    );
    check::eq_i32(
        getvalue("p(10,20,30)", "p", 2).unwrap(),
        30,
        "getvalue_p2_30",
    );
    check::eq_i32(
        getvalue("p(-5,-6,-7)", "p", 1).unwrap(),
        -6,
        "getvalue_negative_minus6",
    );
    // an early key end is a plain text->int failure
    expect_number_format(getvalue("txy(5)", "xy", 0), "getvalue_keylen_mismatch_nfe");
    expect_number_format(
        getvalue("grounded(50)", "gr", 0),
        "getvalue_grounded_prefix_nfe",
    );
    // index past the LAST value: ')' advances one past the line end
    expect_past_end(getvalue("p(10,20)", "p", 2), "getvalue_past_end_sioobe");
    // a trailing comma leaves an empty accumulator -> Integer.valueOf("")
    expect_number_format(
        getvalue("p(1,2,)", "p", 2),
        "getvalue_empty_accumulator_nfe",
    );
    // the ctor trims each line (:231), so only the RAW untrimmed line fails
    expect_number_format(getvalue(" p(1,2,3)", "p", 0), "getvalue_untrimmed_nfe");
    let trimmed = conto("<p>\n  p(1,2,3)  \np(4,2,3)\np(5,2,3)\n</p>\n", &m);
    check::eq_i32(trimmed.planes.len() as i32, 1, "trimmed_line_parses");
    check::eq_i32(trimmed.planes[0].ox[0], 1, "trimmed_line_ox0_1");
}

/// `ContO.getvalue` throws `NumberFormatException` for a non-numeric
/// accumulator; the port returns `Err` (the output line in
/// [`crate::model::getvalue`] names the failure). A "runs past end" error is
/// the `StringIndexOutOfBoundsException` shape instead (:691-695).
fn expect_number_format(r: Result<i32, String>, id: &str) {
    let ok = match &r {
        Err(msg) => !msg.contains("runs past end"),
        Ok(_) => false,
    };
    check::ok(ok, id);
}

fn expect_past_end(r: Result<i32, String>, id: &str) {
    let ok = match &r {
        Err(msg) => msg.contains("runs past end"),
        Ok(_) => false,
    };
    check::ok(ok, id);
}

/// B6(48,49): dust spawns only when slot stg==0, |var5|+|var6| is strictly
/// greater than 100, and the car's side does not thin this particular
/// particle: var9>5 suppresses slots 0/2 (:662-664), var9<-5 suppresses
/// 1/3 (:666-668). All ten fields are written with (int) truncation.
fn dust_spawn() {
    let m = Medium::new();
    let mut c = Conto::from_bytes(Some(&[]), &m);
    c.dust(0, 10.5, 20.9, -30.5, 60.0, 41.0, 2.5, true, 0);
    check::eq_i32(c.sx[0], 10, "dust_sx_10");
    check::eq_i32(c.sy[0], 20, "dust_sy_20");
    check::eq_i32(c.sz[0], -30, "dust_sz_minus30");
    check::eq_i32(c.stg[0], 1, "dust_stg_1");
    check::eq_i32(c.dov[0], -1, "dust_dov_minus1");
    check::eq_i32(bits(c.smag[0]), bits(2.5), "dust_smag_2_5");
    check::eq_i32(c.scx[0], 60, "dust_scx_60");
    check::eq_i32(c.scz[0], 41, "dust_scz_41");
    check::ok(c.fulls[0], "dust_fulls_true");
    check::eq_i32(c.stg[1], 0, "dust_other_slot_untouched");

    // strict > 100.0F
    let mut exact = Conto::from_bytes(Some(&[]), &m);
    exact.dust(0, 1.5, 2.5, 3.5, 60.0, 40.0, 2.5, true, 0);
    check::eq_i32(exact.stg[0], 0, "dust_exact100_no_spawn");
    let mut over = Conto::from_bytes(Some(&[]), &m);
    over.dust(0, 1.5, 2.5, 3.5, 60.0, 40.1, 2.5, true, 0);
    check::eq_i32(over.stg[0], 1, "dust_100_1_spawns");

    // a live particle blocks re-spawn
    let mut live = Conto::from_bytes(Some(&[]), &m);
    live.stg[0] = 1;
    live.dust(0, 1.5, 2.5, 3.5, 60.0, 41.0, 2.5, true, 0);
    check::eq_i32(live.stg[0], 1, "dust_live_slot_no_respawn");
    check::eq_i32(live.sx[0], 0, "dust_live_slot_sx_untouched");

    // side thinning: var9>5 kills 0/2, var9<-5 kills 1/3
    for slot in 0..4 {
        let mut up = Conto::from_bytes(Some(&[]), &m);
        up.dust(slot, 1.5, 2.5, 3.5, 60.0, 41.0, 2.5, true, 6);
        check::eq_i32(
            up.stg[slot as usize],
            if slot == 0 || slot == 2 { 0 } else { 1 },
            &format!("dust_v9_6_slot{slot}"),
        );
        let mut down = Conto::from_bytes(Some(&[]), &m);
        down.dust(slot, 1.5, 2.5, 3.5, 60.0, 41.0, 2.5, true, -6);
        check::eq_i32(
            down.stg[slot as usize],
            if slot == 1 || slot == 3 { 0 } else { 1 },
            &format!("dust_v9_minus6_slot{slot}"),
        );
    }
}
