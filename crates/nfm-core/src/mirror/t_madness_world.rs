//! `TMadnessWorld.java` — Madness's WORLD-DEPENDENT surface with the same
//! scoped fixture the Java class uses: a zeroed `ContO` (empty `.rad`
//! stream), no trackers and no `xtGraphics` reads, so every pin is pure
//! field arithmetic. Group "MadnessWorld" (`Suite.java` position 6).
//! Pin source: `Madness.java:122-277,290-370,563-648,650-730,732-2436`;
//! narrative: notes/JAVA-BEHAVIOR/B9_madness.md (items 9-49, 71-76).
//!
//! Consumes `constants.toml [madness]` like the Java class
//! (`TMadnessWorld.java:40`).
//!
//! Port-shape mapping (each documented at its pin):
//! - the ctor's `Medium`/`Record`/`xt` refs arrive as per-call arguments
//!   (`Madness::new(im)`), so every fixture owns its `Record` and shares one
//!   `Rig` (`Medium` + `XtGraphics`, the Java class's shared `Medium m` and
//!   its null `xt`s);
//! - `regx`/`regy`/`regz` take `Option` collaborators because the original
//!   holds `m`/`rpd`/`xt` as nullable fields — `None` is Java's `null` and
//!   panics at the same deref (`err_null_*`, `regy_im0_reaches_xt_crash`);
//! - the instance `dammult`/`bounce` tables are shared read-only constants
//!   in the port (`DAMMULT`/`BOUNCE`), so table reads go through them.

use std::panic::AssertUnwindSafe;

use crate::checkpoints::CheckPoints;
use crate::conto::{Base, Conto};
use crate::control::Control;
use crate::madness::{BOUNCE, DAMMULT, Madness};
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::plane::Plane;
use crate::record::Record;
use crate::trackers::Trackers;
use crate::xtgraphics::XtGraphics;

/// The Java class's shared `new Medium()` plus the `XtGraphics` the port's
/// non-null `&mut xt` seam requires in place of the Java fixtures' null
/// `xt`s (no pin here reaches an xt read; `XtGraphics::new()` is a cheap
/// empty-sprite object).
struct Rig {
    m: Medium,
    xt: XtGraphics,
}

pub(crate) fn run() {
    check::group("MadnessWorld");
    // TMadnessWorld.java:40 — the class-level fixture load and its shape pin.
    let k = Tml::load("constants.toml").table("madness");
    k.require_keys(&[
        "power_reset",
        "damage_record_gate",
        "destruction_maxmag",
        "destruction_hitmag",
    ]);
    let mut rig = Rig {
        m: Medium::new(),
        xt: XtGraphics::new(),
    };

    one_default_tick(&mut rig);
    speed_ladder(&mut rig, &k);
    handicap_power75(&mut rig);
    wzy_and_steer(&mut rig, &k);
    attitude_snap(&mut rig);
    reseto_pins(&mut rig, &k);
    reg_pins(&mut rig, &k);
    error_pins(&mut rig);
    colide_pins(&mut rig);
}

/// A drive/reg*-ready `ContO` (`TMadnessWorld.java:723-744`): the empty
/// `.rad` stream still allocates `p = new Plane[120]` (ContO.java:210) and
/// leaves `npl == 0` plus every field drive/reg* read at 0; the writes
/// below pin that fixture explicitly.
fn car(m: &Medium) -> Conto {
    let mut c = Conto::place(&Base::default(), m, &mut Trackers::new(), 0, 0, 0, 0);
    // ContO.java's declared non-zero initializers (:21-25) that the port's
    // test-support `Base::default()` does not carry — the Java fixture sees
    // them from the ctor, so re-state them.
    c.disline = 7;
    c.grounded = 1.0;
    c.planes.clear();
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
    }
    c
}

/// One 3-point plane (`TMadnessWorld.java:746-750`); `wz == 0` is the
/// random-scatter path, `wz != 0` skips it.
fn plane(m: &Medium, wz: i32) -> Plane {
    Plane::new(
        m,
        &[0, 10, 0],
        &[0, 0, 10],
        &[0, 0, 0],
        3,
        [1, 2, 3],
        false,
        0,
        0,
        0,
        wz,
        7,
        0,
        false,
    )
}

/// One all-default drive tick (`TMadnessWorld.java:752-759`); `nofocus` is
/// re-armed because the drive tail clears it (Madness.java:2219).
fn tick(
    rig: &mut Rig,
    c: &mut Conto,
    ctl: &mut Control,
    cp: &mut CheckPoints,
    im: usize,
) -> Madness {
    let mut md = Madness::new(im);
    md.cn = 0;
    md.nofocus = true;
    let mut rpd = Record::new();
    md.drive(
        ctl,
        c,
        &Trackers::new(),
        cp,
        &mut rig.m,
        &mut rpd,
        &mut rig.xt,
    );
    md
}

/// One tick with a pre-set `wxz`, returning the resulting `wxz`
/// (`TMadnessWorld.java:761-771`).
fn center(rig: &mut Rig, wxz: i32, speed: f32) -> i32 {
    let mut c = car(&rig.m);
    c.wxz = wxz;
    let mut md = Madness::new(1);
    md.cn = 0;
    md.speed = speed;
    md.nofocus = true;
    let mut rpd = Record::new();
    md.drive(
        &mut Control::new(),
        &mut c,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd,
        &mut rig.xt,
    );
    c.wxz
}

/// B9(33-35): one all-default drive tick, im==1 (Madness.java:1186-1191,
/// 1409-1411, 1957-2003, 2150-2223, 2264-2269, 2291-2297).
fn one_default_tick(rig: &mut Rig) {
    let mut c = car(&rig.m);
    let mut cp = CheckPoints::new();
    let mut ctrl = Control::new();
    let md = tick(rig, &mut c, &mut ctrl, &mut cp, 1);
    // scy[i] += 7 at :1186-1191 -> var21[] == 7 -> contO.y at :1957
    check::eq_i32(c.y, 7, "tick_y_scy_lift_7");
    check::eq_i32(c.x, 0, "tick_x_zero");
    check::eq_i32(c.z, 0, "tick_z_zero");
    // no ground contact (:1417) and no tracker -> the :1409 arm
    check::eq_i32(md.skid, 2, "tick_skid_2");
    // !mtouch at :2264-2268
    check::eq_i32(md.trcnt, 1, "tick_trcnt_1");
    check::eq_i32(md.lxz, 0, "tick_lxz_follows_xz");
    // clear == var119 + nlaps*nsp with n==0 -> :2150-2152
    check::eq_i32(md.nlaps, 1, "tick_nlaps_1");
    // focus==-1 -> im!=0 arm :2157-2159
    check::eq_i32(md.point, 1, "tick_point_1_im1");
    // focus==-1 and im!=0 leaves a zero missedcp alone (:2188-2190)
    check::eq_i32(md.missedcp, 0, "tick_missedcp_0_im1");
    // !mtouch takes :2264, so the :2404-2410 decay never runs
    check::eq_i32(
        md.power.to_bits() as i32,
        75.0f32.to_bits() as i32,
        "tick_power_untouched_75",
    );
    check::eq_i32(c.wxz, 0, "tick_wxz_no_steer_0");
    check::eq_i32(c.wzy, 0, "tick_wzy_speed0_0");
    check::eq_i32(c.xy, 0, "tick_xy_zero");
    check::eq_i32(c.zy, 0, "tick_zy_zero");
    check::ok(!md.mtouch, "tick_mtouch_false");
    check::ok(!md.wtouch, "tick_wtouch_false");
    check::eq_i32(md.cntouch, 0, "tick_cntouch_zero");
    check::eq_i32(cp.dested[1], 0, "tick_dested_untouched");
    // srfcnt<10 and wall==-1 -> :2291-2297 still increments
    check::eq_i32(md.srfcnt, 1, "tick_srfcnt_1");
}

/// B9(36,39,41,43): the speed ladder at im==1 with power==98 — cn 0 gates
/// `swits {50,180,280}`, steps `acelf {11,5,3}` (Madness.java:23-24,11-12).
/// Each tick is a fresh fixture.
fn speed_ladder(rig: &mut Rig, k: &Tml) {
    let mut c0 = car(&rig.m);
    let mut up0 = Control::new();
    up0.up = true;
    let mut s0 = Madness::new(1);
    s0.cn = 0;
    s0.power = k.floatv("power_reset");
    s0.nofocus = true;
    let mut rpd = Record::new();
    s0.drive(
        &mut up0,
        &mut c0,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd,
        &mut rig.xt,
    );
    // from 0 no threshold is met -> :976-977: 5.5 + 98*11/196 == 11.0
    check::eq_i32(
        s0.speed.to_bits() as i32,
        11.0f32.to_bits() as i32,
        "speed_up_from_0_11",
    );
    // wzy -= var50 with var50 = 20*11/(154*simag[0]) truncated by the int
    // cast at :1082 -> -1 (Madness.java:1077-1089)
    check::eq_i32(c0.wzy, -1, "speed_up_from_0_wzy_minus1");

    // up at 200: the 280 plateau arm at :979 needs speed >= swits[0][2]
    // first, so :976 accelerates instead: 200 + 3.0/2 + 98*3.0/196 == 203.0
    let mut c1 = car(&rig.m);
    let mut up1 = Control::new();
    up1.up = true;
    let mut s1 = Madness::new(1);
    s1.cn = 0;
    s1.power = k.floatv("power_reset");
    s1.speed = 200.0;
    s1.nofocus = true;
    let mut rpd1 = Record::new();
    s1.drive(
        &mut up1,
        &mut c1,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd1,
        &mut rig.xt,
    );
    check::eq_i32(
        s1.speed.to_bits() as i32,
        203.0f32.to_bits() as i32,
        "speed_up_at_200_accelerates_203",
    );

    // up at 300: all three thresholds met (var52==3) -> :979 pins the
    // plateau 280/2 + 98*280/196 == 280.0 (and 300 is pulled DOWN to it)
    let mut c2 = car(&rig.m);
    let mut up2 = Control::new();
    up2.up = true;
    let mut s2 = Madness::new(1);
    s2.cn = 0;
    s2.power = k.floatv("power_reset");
    s2.speed = 300.0;
    s2.nofocus = true;
    let mut rpd2 = Record::new();
    s2.drive(
        &mut up2,
        &mut c2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd2,
        &mut rig.xt,
    );
    check::eq_i32(
        s2.speed.to_bits() as i32,
        280.0f32.to_bits() as i32,
        "speed_up_at_300_plateau_280",
    );

    // down at -1000: both down thresholds met -> :958 pins
    // -(180/2 + 98*180/196) == -180.0
    let mut c3 = car(&rig.m);
    let mut down3 = Control::new();
    down3.down = true;
    let mut s3 = Madness::new(1);
    s3.cn = 0;
    s3.power = k.floatv("power_reset");
    s3.speed = -1000.0;
    s3.nofocus = true;
    let mut rpd3 = Record::new();
    s3.drive(
        &mut down3,
        &mut c3,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd3,
        &mut rig.xt,
    );
    check::eq_i32(
        s3.speed.to_bits() as i32,
        (-180.0f32).to_bits() as i32,
        "speed_down_at_neg1000_minus180",
    );

    // down at 100: :942-944 subtracts handb[0]/2 == 3 (INT division)
    let mut c4 = car(&rig.m);
    let mut down4 = Control::new();
    down4.down = true;
    let mut s4 = Madness::new(1);
    s4.cn = 0;
    s4.power = k.floatv("power_reset");
    s4.speed = 100.0;
    s4.nofocus = true;
    let mut rpd4 = Record::new();
    s4.drive(
        &mut down4,
        &mut c4,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd4,
        &mut rig.xt,
    );
    check::eq_i32(
        s4.speed.to_bits() as i32,
        97.0f32.to_bits() as i32,
        "speed_down_at_100_97",
    );
}

/// B9(40,42): the `power != 98` handicap, only reachable with im==0
/// (Madness.java:934-940) — stage 6 keeps `var49 == power`, stage 8
/// multiplies by 0.9, every other stage by 0.76.
fn handicap_power75(rig: &mut Rig) {
    // power 75, speed 0, stage 1 -> var49 == 57 -> :977: 5.5 + 57*11/196
    let mut h0 = car(&rig.m);
    let mut hu0 = Control::new();
    hu0.up = true;
    let mut hp0 = Madness::new(0);
    hp0.cn = 0;
    hp0.power = 75.0;
    hp0.nofocus = true;
    let mut hcp0 = CheckPoints::new();
    hcp0.stage = 1;
    let mut rpd0 = Record::new();
    hp0.drive(
        &mut hu0,
        &mut h0,
        &Trackers::new(),
        &mut hcp0,
        &mut rig.m,
        &mut rpd0,
        &mut rig.xt,
    );
    check::eq_i32(
        hp0.speed.to_bits() as i32,
        8.698979f32.to_bits() as i32,
        "speed_up_from_0_power75_handicap",
    );

    // power 75, speed 200, stage 6 -> NO handicap -> 200 + 1.5 + 75*3/196
    let mut h6 = car(&rig.m);
    let mut hu6 = Control::new();
    hu6.up = true;
    let mut hp6 = Madness::new(0);
    hp6.cn = 0;
    hp6.power = 75.0;
    hp6.speed = 200.0;
    hp6.nofocus = true;
    let mut hcp6 = CheckPoints::new();
    hcp6.stage = 6;
    let mut rpd6 = Record::new();
    hp6.drive(
        &mut hu6,
        &mut h6,
        &Trackers::new(),
        &mut hcp6,
        &mut rig.m,
        &mut rpd6,
        &mut rig.xt,
    );
    check::eq_i32(
        hp6.speed.to_bits() as i32,
        202.64796f32.to_bits() as i32,
        "speed_up_at_200_power75_stage6_no_handicap",
    );

    // stage 1 -> var49 == 57 -> 200 + 1.5 + 57*3/196
    let mut h1 = car(&rig.m);
    let mut hu1 = Control::new();
    hu1.up = true;
    let mut hp1 = Madness::new(0);
    hp1.cn = 0;
    hp1.power = 75.0;
    hp1.speed = 200.0;
    hp1.nofocus = true;
    let mut hcp1 = CheckPoints::new();
    hcp1.stage = 1;
    let mut rpd1 = Record::new();
    hp1.drive(
        &mut hu1,
        &mut h1,
        &Trackers::new(),
        &mut hcp1,
        &mut rig.m,
        &mut rpd1,
        &mut rig.xt,
    );
    check::eq_i32(
        hp1.speed.to_bits() as i32,
        202.37245f32.to_bits() as i32,
        "speed_up_at_200_power75_stage1_076",
    );

    // stage 8 is the OTHER branch: `else if (stage != 6)` -> *0.9
    let mut h8 = car(&rig.m);
    let mut hu8 = Control::new();
    hu8.up = true;
    let mut hp8 = Madness::new(0);
    hp8.cn = 0;
    hp8.power = 75.0;
    hp8.speed = 200.0;
    hp8.nofocus = true;
    let mut hcp8 = CheckPoints::new();
    hcp8.stage = 8;
    let mut rpd8 = Record::new();
    hp8.drive(
        &mut hu8,
        &mut h8,
        &Trackers::new(),
        &mut hcp8,
        &mut rig.m,
        &mut rpd8,
        &mut rig.xt,
    );
    check::eq_i32(
        hp8.speed.to_bits() as i32,
        202.53316f32.to_bits() as i32,
        "speed_up_at_200_power75_stage8_09",
    );
}

/// B9(44-47): the wzy wrap and the steer clamps / self-centering
/// (Madness.java:1077-1130).
fn wzy_and_steer(rig: &mut Rig, k: &Tml) {
    // wzy = -40 - 20 = -60 -> < -45 so += 45 -> -15
    let mut c5 = car(&rig.m);
    c5.wzy = -40;
    let mut s5 = Madness::new(1);
    s5.cn = 0;
    s5.power = k.floatv("power_reset");
    s5.speed = 280.0;
    s5.nofocus = true;
    let mut rpd5 = Record::new();
    s5.drive(
        &mut Control::new(),
        &mut c5,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd5,
        &mut rig.xt,
    );
    check::eq_i32(c5.wzy, -15, "wzy_minus40_wraps_to_minus15");
    check::eq_i32(
        s5.speed.to_bits() as i32,
        280.0f32.to_bits() as i32,
        "wzy_tick_speed_unchanged",
    );

    // steer clamps, turn[0]==6 (:1091-1103)
    let mut c6 = car(&rig.m);
    c6.wxz = -30;
    let mut r6 = Control::new();
    r6.right = true;
    let mut s6 = Madness::new(1);
    s6.cn = 0;
    s6.nofocus = true;
    let mut rpd6 = Record::new();
    s6.drive(
        &mut r6,
        &mut c6,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd6,
        &mut rig.xt,
    );
    check::eq_i32(c6.wxz, -36, "wxz_right_clamps_minus36");

    let mut c7 = car(&rig.m);
    c7.wxz = 34;
    let mut l7 = Control::new();
    l7.left = true;
    let mut s7 = Madness::new(1);
    s7.cn = 0;
    s7.nofocus = true;
    let mut rpd7 = Record::new();
    s7.drive(
        &mut l7,
        &mut c7,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd7,
        &mut rig.xt,
    );
    check::eq_i32(c7.wxz, 36, "wxz_left_clamps_36");

    // self-centering at |speed|<10 (:1105-1117)
    check::eq_i32(center(rig, 5, 0.0), 4, "wxz_recenter_5_to_4");
    check::eq_i32(center(rig, 1, 0.0), 0, "wxz_recenter_1_to_0");
    check::eq_i32(center(rig, -5, 0.0), -4, "wxz_recenter_neg5_to_minus4");

    // self-centering at |speed|>=10 -> snap by turn*2==12 (:1118-1130)
    check::eq_i32(center(rig, 10, 20.0), 0, "wxz_snap_10_to_0");
    check::eq_i32(center(rig, 20, 20.0), 8, "wxz_snap_20_to_8");
}

/// B9(48-49): the attitude snap, |speed|<=10 but !mtouch (:2004-2023).
fn attitude_snap(rig: &mut Rig) {
    let mut c8 = car(&rig.m);
    let mut s8 = Madness::new(1);
    s8.cn = 0;
    s8.nofocus = true;
    s8.pxy = 10;
    let mut rpd8 = Record::new();
    s8.drive(
        &mut Control::new(),
        &mut c8,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd8,
        &mut rig.xt,
    );
    // 0 + 2 + 10/2 == 7
    check::eq_i32(c8.xy, 7, "attitude_pxy_10_to_7");

    let mut c9 = car(&rig.m);
    let mut s9 = Madness::new(1);
    s9.cn = 0;
    s9.nofocus = true;
    s9.pzy = -10;
    let mut rpd9 = Record::new();
    s9.drive(
        &mut Control::new(),
        &mut c9,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd9,
        &mut rig.xt,
    );
    check::eq_i32(c9.zy, -7, "attitude_pzy_minus10_to_minus7");

    let mut c10 = car(&rig.m);
    let mut s10 = Madness::new(1);
    s10.cn = 0;
    s10.nofocus = true;
    s10.pxy = 3;
    let mut rpd10 = Record::new();
    s10.drive(
        &mut Control::new(),
        &mut c10,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd10,
        &mut rig.xt,
    );
    // |pxy-xy| < 4 -> deadband copies pxy exactly (:2011-2013)
    check::eq_i32(c10.xy, 3, "attitude_deadband_pxy_3_to_3");
}

/// B9(22-26): `reseto` (Madness.java:563-648). Every asserted field is
/// pre-dirtied so "reset" is observable; im==0 is safe (reseto touches no
/// xt at all).
fn reseto_pins(rig: &mut Rig, k: &Tml) {
    let mut rc = car(&rig.m);
    // keyx all 0, keyz all 100 -> forca == (400/10000)*(bounce[0]-0.3)
    rc.keyz[0] = 100;
    rc.keyz[1] = 100;
    rc.keyz[2] = 100;
    rc.keyz[3] = 100;
    let mut rcp = CheckPoints::new();
    rcp.pcs = 4;
    rcp.stage = 3;
    rcp.dested[0] = 2;
    let mut rm = Madness::new(0);
    rm.power = 50.0;
    rm.focus = 4;
    rm.clear = 2;
    rm.nlaps = 3;
    rm.missedcp = 3;
    rm.dest = true;
    rm.newcar = true;
    rm.hitmag = 99;
    rm.squash = 3;
    rm.nbsq = 1;
    rm.cntdest = 2;
    rm.lastcolido = 70;
    rm.speed = 12.0;
    rm.scy[0] = 9.0;
    rm.scx[0] = 9.0;
    rm.scz[0] = 9.0;
    rm.dominate[2] = true;
    rm.caught[2] = true;
    rm.cntouch = 7;
    rm.srfcnt = 5;
    rm.surfer = true;
    rm.nofocus = true;
    rm.reseto(0, &mut rc, &mut rcp);
    check::eq_i32(rm.cn as i32, 0, "reseto_cn");
    check::eq_i32(
        rm.power.to_bits() as i32,
        k.floatv("power_reset").to_bits() as i32,
        "reseto_power_98",
    );
    check::eq_i32(rm.focus, -1, "reseto_focus_minus1");
    check::eq_i32(rm.pcleared, 4, "reseto_pcleared_is_cp_pcs");
    check::eq_i32(rm.clear, 0, "reseto_clear_0");
    check::eq_i32(rm.nlaps, 0, "reseto_nlaps_0");
    check::eq_i32(rm.missedcp, 0, "reseto_missedcp_0");
    check::ok(!rm.dest, "reseto_dest_false");
    check::ok(!rm.newcar, "reseto_newcar_false");
    check::eq_i32(rm.hitmag, 0, "reseto_hitmag_0");
    check::eq_i32(rm.squash, 0, "reseto_squash_0");
    check::eq_i32(rm.nbsq, 0, "reseto_nbsq_0");
    check::eq_i32(rm.cntdest, 0, "reseto_cntdest_0");
    check::eq_i32(rm.lastcolido, 0, "reseto_lastcolido_0");
    check::eq_i32(
        rm.speed.to_bits() as i32,
        0.0f32.to_bits() as i32,
        "reseto_speed_0",
    );
    check::eq_i32(
        rm.scy[0].to_bits() as i32,
        0.0f32.to_bits() as i32,
        "reseto_scy_0",
    );
    check::eq_i32(
        rm.scx[0].to_bits() as i32,
        0.0f32.to_bits() as i32,
        "reseto_scx_0",
    );
    check::eq_i32(
        rm.scz[0].to_bits() as i32,
        0.0f32.to_bits() as i32,
        "reseto_scz_0",
    );
    check::ok(!rm.dominate[2], "reseto_dominate_cleared");
    check::ok(!rm.caught[2], "reseto_caught_cleared");
    check::eq_i32(rcp.dested[0], 0, "reseto_dested_im_zeroed");
    // B9(23): cntouch/srfcnt/surfer never appear in :563-648
    check::eq_i32(rm.cntouch, 7, "reseto_preserves_cntouch");
    check::eq_i32(rm.srfcnt, 5, "reseto_preserves_srfcnt");
    check::ok(rm.surfer, "reseto_preserves_surfer");
    check::ok(!rm.nofocus, "reseto_clears_nofocus");
    // :593-600, mirrored exactly: sum of the four |(keyz,keyx)| wheels
    let wheel = (100.0f64 * 100.0).sqrt() as f32;
    let forca_want = (wheel + wheel + wheel + wheel) / 10000.0 * ((BOUNCE[0] as f64 - 0.3) as f32);
    check::eq_i32(
        rm.forca.to_bits() as i32,
        forca_want.to_bits() as i32,
        "reseto_forca_keyz_100_formula",
    );
    check::ok(
        rm.forca > 0.0359 && rm.forca < 0.0361,
        "reseto_forca_approx_0036",
    );

    // B9(25): reseto(7,..) permanently writes moment[7], but only for im==0
    let mut m7a = car(&rig.m);
    let mut cp7a = CheckPoints::new();
    cp7a.stage = 10;
    let mut r7a = Madness::new(0);
    r7a.reseto(7, &mut m7a, &mut cp7a);
    check::eq_i32(
        r7a.moment[7].to_bits() as i32,
        1.7f32.to_bits() as i32,
        "reseto7_stage10_moment_1_7",
    );

    let mut m7b = car(&rig.m);
    let mut cp7b = CheckPoints::new();
    cp7b.stage = 5;
    let mut r7b = Madness::new(0);
    r7b.reseto(7, &mut m7b, &mut cp7b);
    check::eq_i32(
        r7b.moment[7].to_bits() as i32,
        2.0f32.to_bits() as i32,
        "reseto7_other_stage_moment_2_0",
    );

    let mut m7c = car(&rig.m);
    let mut cp7c = CheckPoints::new();
    cp7c.stage = 10;
    let mut r7c = Madness::new(1);
    r7c.reseto(7, &mut m7c, &mut cp7c);
    check::eq_i32(
        r7c.moment[7].to_bits() as i32,
        2.0f32.to_bits() as i32,
        "reseto7_im1_keeps_declared_2_0",
    );

    // B9(26): cn is the row every later dammult/clrad/turn/grip lookup uses
    let mut rc9 = car(&rig.m);
    let mut r9 = Madness::new(0);
    r9.reseto(9, &mut rc9, &mut CheckPoints::new());
    check::eq_i32(r9.cn as i32, 9, "reseto9_cn_9");
    check::eq_i32(
        DAMMULT[r9.cn].to_bits() as i32,
        0.7667f32.to_bits() as i32,
        "reseto9_dammult_row_7667",
    );
}

/// B9(9-16): `regx` / `regz` / `regy` (Madness.java:650-730, 290-370,
/// 122-277). The fixture has `npl == 0`, so the damage-scatter loops (and
/// their `m.random()` calls) are unreachable by construction.
fn reg_pins(rig: &mut Rig, k: &Tml) {
    let mut rx = Record::new();
    let mut md9 = Madness::new(1);
    md9.cn = 0; // dammult[0]==1.0
    let mut c = car(&rig.m);
    md9.regx(
        0,
        k.floatv("damage_record_gate"),
        &mut c,
        Some(&mut rig.m),
        Some(&mut rx),
        Some(&mut rig.xt),
    );
    // strict `> 100.0F` at :652 -> complete no-op
    check::eq_i32(rx.nrx[1][0], 0, "regx_100_noop_nrx");
    check::eq_i32(rx.magx[1][0][0], 0, "regx_100_noop_magx");
    check::eq_i32(md9.hitmag, 0, "regx_100_noop_hitmag");

    let mut rx2 = Record::new();
    let mut md10 = Madness::new(1);
    md10.cn = 0;
    let mut c2 = car(&rig.m);
    md10.regx(
        0,
        k.floatv("damage_record_gate") + 0.5,
        &mut c2,
        Some(&mut rig.m),
        Some(&mut rx2),
        Some(&mut rig.xt),
    );
    // recx(:653) records the PRE-subtraction 100.5 -> (int) -> 100
    check::eq_i32(rx2.nrx[1][0], 1, "regx_100_5_records");
    check::eq_i32(rx2.magx[1][0][0], 100, "regx_100_5_magx_100");

    // cn=9 -> dammult 0.7667 (:54): 130*0.7667 == 99.67 no-op,
    // 131*0.7667 == 100.44 fires (and records (int)100)
    let mut rx3 = Record::new();
    let mut md11 = Madness::new(1);
    md11.cn = 9;
    let mut c3 = car(&rig.m);
    md11.regx(
        0,
        130.0,
        &mut c3,
        Some(&mut rig.m),
        Some(&mut rx3),
        Some(&mut rig.xt),
    );
    check::eq_i32(rx3.nrx[1][0], 0, "regx_cn9_130_noop");
    let mut c4 = car(&rig.m);
    md11.regx(
        0,
        131.0,
        &mut c4,
        Some(&mut rig.m),
        Some(&mut rx3),
        Some(&mut rig.xt),
    );
    check::eq_i32(rx3.nrx[1][0], 1, "regx_cn9_131_fires");
    check::eq_i32(rx3.magx[1][0][0], 100, "regx_cn9_131_magx_100");

    // B9(12): regz records for im==1 and never reaches crash
    let mut rz = Record::new();
    let mut md12 = Madness::new(1);
    md12.cn = 0;
    let mut c5 = car(&rig.m);
    md12.regz(
        0,
        -150.0,
        &mut c5,
        Some(&mut rig.m),
        Some(&mut rz),
        Some(&mut rig.xt),
    );
    check::eq_i32(rz.nrz[1][0], 1, "regz_150_im1_records");
    check::eq_i32(rz.magz[1][0][0], -150, "regz_150_im1_magz_neg150");
    check::eq_i32(md12.hitmag, 0, "regz_150_im1_hitmag_0");

    // B9(14): regy's guard is `var2 > 100` (no absolute value), so a
    // large NEGATIVE damage is fully inert
    let mut ry = Record::new();
    let mut md13 = Madness::new(1);
    md13.cn = 0;
    let mut c6 = car(&rig.m);
    md13.regy(
        0,
        -1000.0,
        &mut c6,
        Some(&mut rig.m),
        Some(&mut ry),
        Some(&mut rig.xt),
    );
    check::eq_i32(ry.nry[1][0], 0, "regy_neg1000_inert_nry");
    check::eq_i32(md13.hitmag, 0, "regy_neg1000_inert_hitmag");
    check::eq_i32(md13.nbsq, 0, "regy_neg1000_inert_nbsq");

    // B9(15): recy runs for every im (no im gate at :125); magy keeps the
    // post-dammult value and Record.mtouch[im][slot] stores this.mtouch
    for im in 1..=4 {
        let mut ry2 = Record::new();
        let mut md14 = Madness::new(im);
        md14.cn = 0;
        md14.mtouch = true;
        let mut c7 = car(&rig.m);
        md14.regy(
            0,
            150.0,
            &mut c7,
            Some(&mut rig.m),
            Some(&mut ry2),
            Some(&mut rig.xt),
        );
        check::eq_i32(ry2.nry[im][0], 1, &format!("regy_150_im{im}_records"));
        check::eq_i32(
            ry2.magy[im][0][0],
            150,
            &format!("regy_150_im{im}_magy_150"),
        );
        check::ok(ry2.mtouch[im][0], &format!("regy_150_im{im}_mtouch_flag"));
        check::eq_i32(md14.nbsq, 0, &format!("regy_150_im{im}_nbsq_0"));
    }

    // im==0 additionally hits `this.xt.crash` (:164-166) — the Java fixture
    // passes a null xt, so the NPE proves recy (:125) already ran. The port
    // models the nullable xt as `None` at the same deref.
    let mut ry0 = Record::new();
    let mut md15 = Madness::new(0);
    md15.cn = 0;
    let mut c15 = car(&rig.m);
    check::throws_it(
        AssertUnwindSafe(|| {
            md15.regy(0, 150.0, &mut c15, Some(&mut rig.m), Some(&mut ry0), None);
        }),
        "NullPointerException",
        "regy_im0_reaches_xt_crash",
    );
    check::eq_i32(ry0.nry[0][0], 1, "regy_im0_recy_recorded_before_crash");
}

/// B9(27-32): the exception surface (Madness.java:123, 653, 419, 2426).
fn error_pins(rig: &mut Rig) {
    // cn==10 is never bounds-checked by reseto (:564), so every reg*
    // indexes dammult[10] first (:123,:291,:651)
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = Madness::new(1);
            e.cn = 10;
            let mut c = car(&rig.m);
            let mut rpd = Record::new();
            e.regx(
                0,
                150.0,
                &mut c,
                Some(&mut rig.m),
                Some(&mut rpd),
                Some(&mut rig.xt),
            );
        }),
        "index out of bounds",
        "err_cn10_regx_aioobe",
    );
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = Madness::new(1);
            e.cn = 10;
            let mut c = car(&rig.m);
            let mut rpd = Record::new();
            e.regy(
                0,
                150.0,
                &mut c,
                Some(&mut rig.m),
                Some(&mut rpd),
                Some(&mut rig.xt),
            );
        }),
        "index out of bounds",
        "err_cn10_regy_aioobe",
    );
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = Madness::new(1);
            e.cn = 10;
            let mut c = car(&rig.m);
            let mut rpd = Record::new();
            e.regz(
                0,
                150.0,
                &mut c,
                Some(&mut rig.m),
                Some(&mut rpd),
                Some(&mut rig.xt),
            );
        }),
        "index out of bounds",
        "err_cn10_regz_aioobe",
    );

    // part index 4: the damage is recorded BEFORE the plane loop, so the
    // AIOOBE lands in Record.recx/recy/recz — the [im][4] write is out of
    // bounds (Record.java:633-649, 841-848). It never reaches keyx[4].
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = car(&rig.m);
            e.planes = crate::conto::Planes::from(vec![plane(&rig.m, 0)]);
            let mut x = Madness::new(1);
            x.cn = 0;
            let mut rpd = Record::new();
            x.regx(
                4,
                150.0,
                &mut e,
                Some(&mut rig.m),
                Some(&mut rpd),
                Some(&mut rig.xt),
            );
        }),
        "index out of bounds",
        "err_part4_regx_aioobe",
    );
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = car(&rig.m);
            e.planes = crate::conto::Planes::from(vec![plane(&rig.m, 0)]);
            let mut x = Madness::new(1);
            x.cn = 0;
            let mut rpd = Record::new();
            x.regy(
                4,
                150.0,
                &mut e,
                Some(&mut rig.m),
                Some(&mut rpd),
                Some(&mut rig.xt),
            );
        }),
        "index out of bounds",
        "err_part4_regy_aioobe",
    );
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = car(&rig.m);
            e.planes = crate::conto::Planes::from(vec![plane(&rig.m, 0)]);
            let mut x = Madness::new(1);
            x.cn = 0;
            let mut rpd = Record::new();
            x.regz(
                4,
                150.0,
                &mut e,
                Some(&mut rig.m),
                Some(&mut rpd),
                Some(&mut rig.xt),
            );
        }),
        "index out of bounds",
        "err_part4_regz_aioobe",
    );

    // B9(29): dominate/caught are size 5 (:56-57) -> other.im==5 throws at
    // :419 before any push can run
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut a = car(&rig.m);
            let mut b = car(&rig.m);
            a.maxr = 10;
            b.maxr = 10;
            let mut other = Madness::new(5);
            other.cn = 0;
            other.speed = 5.0;
            other.power = 75.0;
            let mut e = Madness::new(0);
            e.cn = 0;
            e.speed = 10.0;
            e.power = 75.0;
            let mut rpd = Record::new();
            e.colide(
                &mut a,
                &mut other,
                &mut b,
                &mut rig.m,
                &mut rpd,
                &mut rig.xt,
            );
        }),
        "index out of bounds",
        "err_colide_other_im5_aioobe",
    );

    // B9(30): drive's final bookkeeping indexes CheckPoints.dested[im]
    // (CheckPoints.java:20) at :2433 -> im==5 throws
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = Madness::new(5);
            e.cn = 0;
            e.nofocus = true;
            let mut c = car(&rig.m);
            let mut rpd = Record::new();
            e.drive(
                &mut Control::new(),
                &mut c,
                &Trackers::new(),
                &mut CheckPoints::new(),
                &mut rig.m,
                &mut rpd,
                &mut rig.xt,
            );
        }),
        "index out of bounds",
        "err_drive_im5_dested_aioobe",
    );

    // B9(31): m==null + |damage|>100 + a wz==0 plane -> NPE at m.random
    // (:671); a wz!=0 plane short-circuits :670 and nothing happens
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut e = car(&rig.m);
            e.planes = crate::conto::Planes::from(vec![plane(&rig.m, 0)]);
            let mut x = Madness::new(1);
            x.cn = 0;
            let mut rpd = Record::new();
            x.regx(0, 150.0, &mut e, None, Some(&mut rpd), Some(&mut rig.xt));
        }),
        "NullPointerException",
        "err_null_m_wz0_npe",
    );
    let mut cm_ok = car(&rig.m);
    cm_ok.planes = crate::conto::Planes::from(vec![plane(&rig.m, 1)]);
    let mut md_ok = Madness::new(1);
    md_ok.cn = 0;
    let mut rpd_ok = Record::new();
    md_ok.regx(
        0,
        150.0,
        &mut cm_ok,
        None,
        Some(&mut rpd_ok),
        Some(&mut rig.xt),
    );
    check::eq_i32(md_ok.hitmag, 0, "err_null_m_wz1_hitmag_0");
    check::eq_i32(cm_ok.planes[0].ox[0], 0, "err_null_m_wz1_plane_untouched");

    // B9(32): rpd==null NPEs at the recx call (:653)
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut x = Madness::new(1);
            x.cn = 0;
            let mut e = car(&rig.m);
            x.regx(0, 150.0, &mut e, Some(&mut rig.m), None, Some(&mut rig.xt));
        }),
        "NullPointerException",
        "err_null_rpd_npe",
    );
}

/// B9(71-76): `colide` (Madness.java:383-553) — colide reads the MADNESS
/// impulse arrays, never `ContO.scx/scz` (that copy is visual only).
fn colide_pins(rig: &mut Rig) {
    let mut ca = car(&rig.m);
    let mut cb = car(&rig.m);
    ca.maxr = 10;
    cb.maxr = 10;
    let mut oth = Madness::new(1);
    oth.cn = 0;
    oth.speed = 5.0;
    oth.power = 75.0;
    let mut me = Madness::new(0);
    me.cn = 0;
    me.speed = 10.0;
    me.power = 75.0;
    let mut rpd = Record::new();
    me.colide(
        &mut ca,
        &mut oth,
        &mut cb,
        &mut rig.m,
        &mut rpd,
        &mut rig.xt,
    );
    // |75*10*1.2| > |75*5*1.2| -> :421-432
    check::ok(me.dominate[1], "colide_dominate_bigger_impulse");
    check::ok(me.caught[1], "colide_caught_set");
    // :540-542, written inside the contact pair loop; no push happened
    // (all impulse arrays 0) so no reg*/xt deref even with this.im==0
    check::eq_i32(oth.lastcolido, 70, "colide_other_lastcolido_70");
    // lift[0]==0 (:47) -> the per-pair lift leaves scy alone
    check::eq_i32(
        oth.scy[0].to_bits() as i32,
        0.0f32.to_bits() as i32,
        "colide_cn0_lift_zero",
    );

    // B9(72): equal |power*speed*moment| -> the moment tiebreak decides
    let mut ca2 = car(&rig.m);
    let mut cb2 = car(&rig.m);
    ca2.maxr = 10;
    cb2.maxr = 10;
    let mut oth2 = Madness::new(2);
    oth2.cn = 3; // moment[3]==1.0
    oth2.speed = 2.0;
    oth2.power = 60.0;
    let mut me2 = Madness::new(1);
    me2.cn = 0; // moment[0]==1.2
    me2.speed = 2.0;
    me2.power = 50.0;
    let mut rpd2 = Record::new();
    me2.colide(
        &mut ca2,
        &mut oth2,
        &mut cb2,
        &mut rig.m,
        &mut rpd2,
        &mut rig.xt,
    );
    // 50*2*1.2 == 60*2*1.0 -> :426-427
    check::ok(me2.dominate[2], "colide_tie_moment_breaks_tie");
    check::ok(me2.caught[2], "colide_tie_caught_set");

    // B9(73): both speeds 0 -> the :419 guard leaves BOTH flags untouched
    let mut ca3 = car(&rig.m);
    let mut cb3 = car(&rig.m);
    ca3.maxr = 10;
    cb3.maxr = 10;
    let mut oth3 = Madness::new(1);
    oth3.cn = 0;
    oth3.speed = 0.0;
    oth3.power = 75.0;
    let mut me3 = Madness::new(0);
    me3.cn = 0;
    me3.speed = 0.0;
    me3.power = 75.0;
    me3.caught[1] = true;
    let mut rpd3 = Record::new();
    me3.colide(
        &mut ca3,
        &mut oth3,
        &mut cb3,
        &mut rig.m,
        &mut rpd3,
        &mut rig.xt,
    );
    check::ok(!me3.dominate[1], "colide_zero_speed_keeps_dominate");
    check::ok(me3.caught[1], "colide_zero_speed_keeps_caught");

    // B9(74): distance gate - 200^2 == 40000 >= (100*100 + 100*100)*1.5
    let mut ca4 = car(&rig.m);
    let mut cb4 = car(&rig.m);
    ca4.maxr = 100;
    cb4.maxr = 100;
    cb4.x = 200;
    let mut oth4 = Madness::new(1);
    oth4.cn = 0;
    oth4.speed = 5.0;
    oth4.power = 75.0;
    let mut me4 = Madness::new(0);
    me4.cn = 0;
    me4.speed = 10.0;
    me4.power = 75.0;
    me4.dominate[1] = true;
    me4.caught[1] = true;
    let mut rpd4 = Record::new();
    me4.colide(
        &mut ca4,
        &mut oth4,
        &mut cb4,
        &mut rig.m,
        &mut rpd4,
        &mut rig.xt,
    );
    check::ok(me4.dominate[1], "colide_distance_gate_keeps_dominate");
    check::ok(!me4.caught[1], "colide_distance_gate_clears_caught");

    // B9(75): cn==1 -> lift 30; 16 pair hits x -30 per wheel slot == -120.
    // this.im==0 is xt-free here because every impulse is 0, so the push
    // blocks (:456/:498) never run.
    let mut ca5 = car(&rig.m);
    let mut cb5 = car(&rig.m);
    ca5.maxr = 10;
    cb5.maxr = 10;
    let mut oth5 = Madness::new(1);
    oth5.cn = 0;
    oth5.speed = 5.0;
    oth5.power = 75.0;
    let mut me5 = Madness::new(0);
    me5.cn = 1; // lift 30, moment 0.75
    me5.speed = 10.0;
    me5.power = 75.0;
    let mut rpd5 = Record::new();
    me5.colide(
        &mut ca5,
        &mut oth5,
        &mut cb5,
        &mut rig.m,
        &mut rpd5,
        &mut rig.xt,
    );
    check::ok(me5.dominate[1], "colide_cn1_dominates");
    check::eq_i32(
        oth5.scy[0].to_bits() as i32,
        (-120.0f32).to_bits() as i32,
        "colide_cn1_lift_scy0_minus120",
    );
    check::eq_i32(
        oth5.scy[3].to_bits() as i32,
        (-120.0f32).to_bits() as i32,
        "colide_cn1_lift_scy3_minus120",
    );
    check::eq_i32(oth5.lastcolido, 70, "colide_cn1_other_lastcolido_70");
    check::eq_i32(me5.lastcolido, 0, "colide_cn1_this_lastcolido_0");

    // B9(76): single-axis push. this.im MUST be !=0: at :476 `this.im==0`
    // sets the other's colidim, which would send the other's regx into its
    // (null) xt. push[0]==2, revpush term 0 because other.scx==0.
    let mut ca6 = car(&rig.m);
    let mut cb6 = car(&rig.m);
    ca6.maxr = 10;
    cb6.maxr = 10;
    let mut r6o = Record::new();
    let mut oth6 = Madness::new(1);
    oth6.cn = 0;
    oth6.speed = 5.0;
    oth6.power = 75.0;
    let mut me6 = Madness::new(1);
    me6.cn = 0;
    me6.speed = 10.0;
    me6.power = 75.0;
    me6.scx[0] = 100.0;
    // The game shares ONE Record across all five cars (GameSparker.java:593,
    // 603), and the port's `colide` takes it once; the Java fixture's
    // per-car Records collapse onto this one, which is where the push's
    // regx lands in both readings.
    me6.colide(
        &mut ca6,
        &mut oth6,
        &mut cb6,
        &mut rig.m,
        &mut r6o,
        &mut rig.xt,
    );
    check::ok(me6.dominate[1], "colide_push_dominates");
    check::eq_i32(
        oth6.scx[0].to_bits() as i32,
        200.0f32.to_bits() as i32,
        "colide_push_other_scx_200",
    );
    check::eq_i32(
        me6.scx[0].to_bits() as i32,
        100.0f32.to_bits() as i32,
        "colide_push_this_scx_unchanged_100",
    );
    // the same pair loop pushes the other three zero slots too
    check::eq_i32(
        oth6.scx[3].to_bits() as i32,
        200.0f32.to_bits() as i32,
        "colide_push_other_scx3_200",
    );
    check::eq_i32(
        me6.scx[3].to_bits() as i32,
        0.0f32.to_bits() as i32,
        "colide_push_this_scx3_0",
    );
    // 200 * moment[0]==1.2 == 240 reaches the other's Record via regx (:480)
    check::eq_i32(r6o.nrx[1][0], 1, "colide_push_other_regx_records");
    check::eq_i32(r6o.magx[1][0][0], 240, "colide_push_other_regx_magx_240");
    // ContO.scx is untouched: only the Madness impulse array carries it
    check::eq_i32(cb6.scx[0], 0, "colide_push_conto_scx_untouched");
}
