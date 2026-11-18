// FIDELITY: Mirrors the original `Madness.drive(Control, ContO, Trackers, CheckPoints, ...)` Java arity verbatim.
#![allow(clippy::too_many_arguments)]
//! `TMadnessDrive.java` — the ORIGINAL `Madness.drive(Control, ContO,
//! Trackers, CheckPoints)` over multi-tick runs, pinning observable state at
//! chosen ticks, plus the renderer/`Medium` paths the other classes leave
//! untouched. Group "MadnessDrive" (`Suite.java` position 19). Pin source:
//! `Madness.java:7-2436`, `Medium.java:125-564`, `ContO.java:7-56` via
//! `tests/java/TMadnessDrive.java`; narrative: notes/JAVA-BEHAVIOR/
//! B9_madness.md (items 9-49, 71-76) and B3_wheels_medium.md (the Medium
//! coverage-closure section).
//!
//! Consumes ALL THREE fixtures the Java class does: `scenarios.toml` (the
//! whole file: [[scenario]] rows, nested [[scenario.phase]] rows and the
//! [scenario.atN]/[scenario.end] expectation tables), `constants.toml
//! [madness]` and `determinism.toml [vectors].vecmedium` (the VecMedium
//! subclass's VEC, `TMadnessDrive.java:30-40`).
//!
//! Port-shape mapping (documented at each pin):
//! - the Java `VecMedium extends Medium` override is the port's
//!   `RngSource::Vector(vec, 0)` seam on `Medium.rng`: the vector REPLACES
//!   `random()` wholesale, exactly like the subclass. One shared instance for
//!   the whole class (draws accumulate, like Java's `m`); each fixture that
//!   says `new VecMedium()` gets a fresh cursor.
//! - the Java `Madness(m, rpd, xt, im)` refs arrive as per-call arguments in
//!   the port's `Madness::drive(ctl, o, t, cp, m, rpd, xt)`; every fixture
//!   owns its `Record` and shares one `Rig` (Medium + XtGraphics, the Java
//!   class's shared `m` and its null `xt`s — no drive pin reaches an xt read).
//! - `new ContO(new byte[0], m, new Trackers())` is [`Conto::blank`]: the
//!   declared empty-model state (`npl == 0`, disline 7, grounded 1.0) that
//!   the `car()` helper then zeroes explicitly.
//! - the `Medium` block's 64x64 `BufferedImage` Graphics is a port `Canvas`
//!   (any real target works; no pixel is pinned, B3).

use crate::checkpoints::CheckPoints;
use crate::conto::{Conto, around, aroundtrack, watch};
use crate::control::Control;
use crate::madness::Madness;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::plane::Plane;
use crate::random::RngSource;
use crate::record::Record;
use crate::trackers::Trackers;
use crate::xtgraphics::XtGraphics;

/// The Java class's shared `Medium` (a VecMedium) plus the `XtGraphics` the
/// port's non-null `&mut xt` seam requires in place of the fixtures' null
/// `xt`s (`XtGraphics::new()` is a cheap empty-sprite object).
struct Rig {
    m: Medium,
    xt: XtGraphics,
}

/// `new VecMedium()`: a fresh cursor over the shared vector
/// (`TMadnessDrive.java:30-40`).
fn vec_rig(vec: &[f32]) -> Rig {
    let mut m = Medium::new();
    m.rng = Some(RngSource::Vector(vec.to_vec(), 0));
    Rig {
        m,
        xt: XtGraphics::new(),
    }
}

/// `flag(Toml, key)` (`:49-51`): absent means false (the Control initializer).
fn flag(ph: &Tml, key: &str) -> bool {
    ph.has(key) && ph.boolv(key)
}

pub(crate) fn run() {
    check::group("MadnessDrive");
    let vec = Tml::load("determinism.toml")
        .table("vectors")
        .float_arr("vecmedium");
    // shape pin: the wrap period is part of the pinned behavior
    check::eq_i32(vec.len() as i32, 12, "vecmedium_len_12");
    // shape pins: the fixture scripts and the constant table are present
    let sc = Tml::load("scenarios.toml");
    check::eq_i32(sc.table_count("scenario") as i32, 4, "scenarios_rows_4");
    let mad = Tml::load("constants.toml").table("madness");
    mad.require_keys(&[
        "power_default",
        "wall_recoil_cap",
        "capsize_detect",
        "powerup_travxy_div",
        "trick_rotate",
        "ground_clamp",
        "focus_scan_len",
    ]);

    let mut rig = vec_rig(&vec);
    scenario_race(&mut rig, &sc);
    scenario_roll(&mut rig, &sc);
    scenario_air(&mut rig, &sc);
    loop_latch(&mut rig);
    scenario_loopminus(&mut rig, &sc);
    wall_collisions(&mut rig);
    hard_wall(&mut rig);
    slopes(&mut rig);
    capsize(&mut rig, &mad);
    attitude_snap_fixtures(&mut rig, &mad);
    destruction(&mut rig, &mad);
    fix_points(&mut rig);
    checkpoints(&mut rig);
    focus_machine(&mut rig);
    powerup_machine(&mut rig, &mad);
    trick_accumulation(&mut rig);
    dest_capcnt(&mut rig, &mad);
    attitude_onto_conto(&mut rig);
    impulse_clamps(&mut rig);
    mxz_autoturn(&mut rig, &mad);
    ground_cn7(&mut rig);
    skid_scatter(&mut rig, &vec);
    reg_scatter(&mut rig, &vec);
    colide_pins(&mut rig);
    im0_wall_reset(&mut rig);
    residual_air(&mut rig, &mad);
    residual_motion(&mut rig, &mad, &vec);
    residual_checkpoints(&mut rig);
    residual_colide_ties(&mut rig);
    let mut bandm = residual_colide_bands(&mut rig, &vec);
    residual_colide_press(&mut rig);
    residual_colide_hsb(&mut rig, &mut bandm);
    residual_misc(&mut rig);
    residual_hsb(&mut rig, &vec);
    residual_tail_a(&mut rig, &mad, &vec);
    residual_tail_b(&mut rig, &mad);
    residual_tail_c(&mut rig);
    residual_tail_d(&mut rig);
    medium_block(&mut rig, &vec);
}

// ─────────────────────────────────────────────────────────── scenario scripts

/// `a.` a 400-tick race over floor tiles, walls and slopes
/// (`TMadnessDrive.java:63-150`): scenarios.toml [[scenario]] id="race".
fn scenario_race(rig: &mut Rig, sc: &Tml) {
    let race = sc.table_at("scenario", 0);
    check::eq_str(race.str("id"), "race", "race_scenario_first");
    let mut tr = Trackers::new();
    let mut nt = 0usize;
    for i in 0..race.intv("course_floors") {
        floor(
            &mut tr,
            nt,
            0,
            race.intv("floor_y"),
            race.intv("floor_z_base") + i * race.intv("floor_z_step"),
            i % race.intv("floor_skd_mod"),
            race.intv("floor_dam"),
            race.intv("floor_radx"),
            race.intv("floor_radz"),
            race.intv("floor_rady"),
        );
        nt += 1;
    }
    for i in 0..race.intv("course_walls") {
        let wz = race.intv("wall_z_base") + i * race.intv("wall_z_step");
        wallx(
            &mut tr,
            nt,
            race.intv("wall_x"),
            race.intv("wall_y"),
            wz,
            race.intv("wall_xy"),
            race.intv("wall_radx"),
            race.intv("wall_radz"),
            race.intv("wall_rady"),
            race.intv("wall_dam"),
        );
        nt += 1;
        wallx(
            &mut tr,
            nt,
            -race.intv("wall_x"),
            race.intv("wall_y"),
            wz,
            -race.intv("wall_xy"),
            race.intv("wall_radx"),
            race.intv("wall_radz"),
            race.intv("wall_rady"),
            race.intv("wall_dam"),
        );
        nt += 1;
    }
    for i in 0..race.intv("course_slopes") {
        let idx = nt;
        nt += 1;
        tr.x[idx] = race.intv("slope_x");
        tr.y[idx] = race.intv("slope_y");
        tr.z[idx] =
            race.intv("slope_z_base") + i * race.intv("slope_z_step") + race.intv("slope_z_mid");
        tr.xy[idx] = race.intv("slope_xy");
        tr.zy[idx] = 0;
        tr.skd[idx] = 0;
        tr.dam[idx] = race.intv("slope_dam");
        tr.radx[idx] = race.intv("slope_radx");
        tr.radz[idx] = race.intv("slope_radz");
        tr.rady[idx] = race.intv("slope_rady");
    }
    tr.nt = nt;
    let mut cp = CheckPoints::new();
    cp.n = race.intv("checkpoints");
    cp.nsp = race.intv("checkpoints");
    for i in 0..race.intv("checkpoints") {
        cp.typ[i as usize] = 1;
        cp.x[i as usize] = race.intv("checkpoint_x");
        cp.z[i as usize] = race.intv("checkpoint_z_base") + i * race.intv("checkpoint_z_step");
        cp.y[i as usize] = race.intv("checkpoint_y");
    }
    cp.typ[race.intv("checkpoint_typ2_index") as usize] = 2; // a z-axis checkpoint in the same course
    let mut c = car();
    c.y = race.intv("start_y");
    let mut md = Madness::new(race.intv("im") as usize);
    md.cn = race.intv("cn") as usize;
    md.power = race.floatv("power");
    let mut rpd = Record::new();
    let mut ctrl = Control::new();
    let rp = race.table_at("phase", 0);
    for t in 0..race.intv("ticks") {
        ctrl.up = flag(&rp, "up");
        ctrl.left = (t % rp.intv("left_period")) < rp.intv("left_ticks");
        ctrl.right = (t % rp.intv("right_period")) < rp.intv("right_ticks");
        ctrl.handb = (t % rp.intv("handb_period")) < rp.intv("handb_ticks");
        ctrl.down = (t % rp.intv("down_period")) < rp.intv("down_ticks");
        ctrl.wall = rp.intv("wall");
        md.nofocus = (t % rp.intv("nofocus_period")) != rp.intv("nofocus_zero");
        md.drive(
            &mut ctrl,
            &mut c,
            &tr,
            &mut cp,
            &mut rig.m,
            &mut rpd,
            &mut rig.xt,
        );
        if t == 0 {
            let e = race.table("at0");
            check::eq_i32(c.y, e.intv("y"), "race_t0_y_170");
            check::eq_i32(c.x, e.intv("x"), "race_t0_x_70");
            check::ok(md.mtouch == e.boolv("mtouch"), "race_t0_mtouch");
            check::eq_i32(bits(md.speed), bits(e.floatv("speed")), "race_t0_speed_0");
        }
        if t == 40 {
            let e = race.table("at40");
            check::eq_i32(c.y, e.intv("y"), "race_t40_y_195");
            check::eq_i32(c.x, e.intv("x"), "race_t40_x_minus85");
            check::eq_i32(c.z, e.intv("z"), "race_t40_z_3550");
            check::eq_i32(
                bits(md.speed),
                bits(e.floatv("speed")),
                "race_t40_speed_185_842",
            );
            check::eq_i32(md.trcnt, e.intv("trcnt"), "race_t40_trcnt_1");
        }
        if t == 80 {
            let e = race.table("at80");
            check::eq_i32(c.y, e.intv("y"), "race_t80_y_248");
            check::eq_i32(c.x, e.intv("x"), "race_t80_x_284");
            check::eq_i32(c.z, e.intv("z"), "race_t80_z_10245");
        }
    }
    check::ok(
        md.mtouch == race.table("end").boolv("mtouch"),
        "race_end_mtouch",
    );
}

/// `a2.` free roll on flat ground: the ground clamp and the power decay
/// (`TMadnessDrive.java:152-189`, Madness.java:1417-1461,2404-2410).
fn scenario_roll(rig: &mut Rig, sc: &Tml) {
    let roll_sc = sc.table_at("scenario", 1);
    check::eq_str(roll_sc.str("id"), "roll", "roll_scenario_second");
    let mut roll = car();
    roll.y = roll_sc.intv("start_y");
    let mut rm = mk(roll_sc.intv("im") as usize, roll_sc.floatv("power"));
    let mut rpd = Record::new();
    rm.nofocus = true;
    rm.drive(
        &mut Control::new(),
        &mut roll,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd,
        &mut rig.xt,
    );
    let r0 = roll_sc.table("at0");
    check::eq_i32(roll.y, r0.intv("y"), "roll_t0_y_250");
    check::eq_i32(
        bits(rm.scy[0]),
        bits(r0.floatv("scy0")),
        "roll_t0_scy_minus1_4",
    );
    check::eq_i32(
        bits(rm.scy[3]),
        bits(r0.floatv("scy3")),
        "roll_t0_scy3_minus1_4",
    );
    check::eq_i32(
        bits(rm.power),
        bits(r0.floatv("power")),
        "roll_t0_power_97_62352",
    );
    check::ok(rm.wtouch == r0.boolv("wtouch"), "roll_t0_wtouch");
    check::ok(rm.gtouch == r0.boolv("gtouch"), "roll_t0_gtouch");
    check::eq_i32(rm.skid, r0.intv("skid"), "roll_t0_skid_2");
    check::eq_i32(rm.trcnt, r0.intv("trcnt"), "roll_t0_trcnt_0");

    // Tick 1: mtouch at entry pulls scy to 0 (:1318-1326), the clamp then
    // sees 5.5999994 but scy is already 0, and power keeps decaying.
    rm.nofocus = true;
    rm.drive(
        &mut Control::new(),
        &mut roll,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut rpd,
        &mut rig.xt,
    );
    let r1 = roll_sc.table("at1");
    check::eq_i32(bits(rm.scy[0]), bits(r1.floatv("scy0")), "roll_t1_scy_0");
    check::eq_i32(
        bits(rm.power),
        bits(r1.floatv("power")),
        "roll_t1_power_97_251366",
    );
    check::eq_i32(rm.skid, r1.intv("skid"), "roll_t1_skid_0");
    for _ in 2..roll_sc.intv("ticks") {
        rm.nofocus = true;
        rm.drive(
            &mut Control::new(),
            &mut roll,
            &Trackers::new(),
            &mut CheckPoints::new(),
            &mut rig.m,
            &mut rpd,
            &mut rig.xt,
        );
    }
    let r11 = roll_sc.table("at11");
    check::eq_i32(
        bits(rm.power),
        bits(r11.floatv("power")),
        "roll_t11_power_93_750465",
    );
    check::eq_i32(
        bits(rm.speed),
        bits(r11.floatv("speed")),
        "roll_t11_speed_0",
    );
    check::eq_i32(roll.y, r11.intv("y"), "roll_t11_y_250");
}

/// `b.` air control: handb in the air starts loop 1 -> 2
/// (`TMadnessDrive.java:191-238`).
fn scenario_air(rig: &mut Rig, sc: &Tml) {
    let air_sc = sc.table_at("scenario", 2);
    check::eq_str(air_sc.str("id"), "air_handb", "air_scenario_third");
    let mut air = car();
    air.y = air_sc.intv("start_y");
    let mut am = mk(air_sc.intv("im") as usize, air_sc.floatv("power"));
    let mut rpd = Record::new();
    let mut ac = Control::new();
    for t in 0..air_sc.intv("ticks") {
        let ph = air_sc.table_at("phase", t as usize);
        ac.handb = flag(&ph, "handb");
        ac.up = flag(&ph, "up");
        ac.left = flag(&ph, "left");
        ac.down = flag(&ph, "down");
        ac.right = flag(&ph, "right");
        am.nofocus = true;
        am.drive(
            &mut ac,
            &mut air,
            &Trackers::new(),
            &mut CheckPoints::new(),
            &mut rig.m,
            &mut rpd,
            &mut rig.xt,
        );
        if t == 0 {
            let e = air_sc.table("at0");
            check::eq_i32(am.loop_, e.intv("loop"), "air_handb_loop_2");
            check::eq_i32(
                bits(am.scy[0]),
                bits(e.floatv("scy0")),
                "air_handb_scy_average_7",
            );
            check::eq_i32(
                bits(am.scy[3]),
                bits(e.floatv("scy3")),
                "air_handb_scy3_average_7",
            );
        }
        if t == 1 {
            check::eq_i32(
                am.loop_,
                air_sc.table("at1").intv("loop"),
                "air_handb_second_tick_loop_2",
            );
        }
        if t == 2 {
            let e = air_sc.table("at2");
            check::eq_i32(bits(am.ucomp), bits(e.floatv("ucomp")), "air_up_ucomp_10_5");
            check::eq_i32(bits(am.lcomp), bits(e.floatv("lcomp")), "air_left_lcomp_7");
            check::eq_i32(am.pzy, e.intv("pzy"), "air_up_pzy_minus10");
            check::eq_i32(am.pxy, e.intv("pxy"), "air_left_pxy_minus7");
            check::eq_i32(air.y, e.intv("y"), "air_up_y_92");
        }
        if t == 3 {
            let e = air_sc.table("at3");
            check::eq_i32(
                bits(am.ucomp),
                bits(e.floatv("ucomp")),
                "air_release_ucomp_decays_10",
            );
            check::eq_i32(
                bits(am.lcomp),
                bits(e.floatv("lcomp")),
                "air_release_lcomp_decays_5",
            );
        }
        if t == 4 {
            let e = air_sc.table("at4");
            check::ok(
                e.boolv("dcomp_positive") == (am.dcomp > 0.0),
                "air_down_dcomp_positive",
            );
            check::eq_i32(bits(am.rcomp), bits(e.floatv("rcomp")), "air_right_rcomp_7");
            check::eq_i32(am.pxy, e.intv("pxy"), "air_right_pxy_minus8");
        }
    }
}

/// `b2.` loop==-1 fixes the press flags: the wtouch latch
/// (`TMadnessDrive.java:240-260`).
fn loop_latch(rig: &mut Rig) {
    let mut lc = car();
    lc.y = 50;
    let mut lm = mk(1, 98.0);
    lm.loop_ = 2;
    lm.wtouch = true;
    let mut lcc = Control::new();
    lcc.left = true;
    lcc.up = true;
    lcc.down = true;
    lcc.right = true;
    lm.nofocus = true;
    lm.drive(
        &mut lcc,
        &mut lc,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(lm.loop_, -1, "loopm_wtouch_loop_minus1");
    check::ok(lm.pl, "loopm_pl_latched");
    check::ok(lm.pr, "loopm_pr_latched");
    check::ok(lm.pu, "loopm_pu_latched");
    check::ok(lm.pd, "loopm_pd_latched");
}

/// `b3.` the loop==-1 airborne block
/// (`TMadnessDrive.java:262-298`): scenarios.toml id="loopminus_air".
fn scenario_loopminus(rig: &mut Rig, sc: &Tml) {
    let lm_sc = sc.table_at("scenario", 3);
    check::eq_str(
        lm_sc.str("id"),
        "loopminus_air",
        "loopminus_scenario_fourth",
    );
    let mut la = car();
    la.y = lm_sc.intv("start_y");
    let mut l2 = mk(lm_sc.intv("im") as usize, lm_sc.floatv("power"));
    l2.loop_ = lm_sc.intv("loop");
    if lm_sc.has("wtouch") {
        l2.wtouch = lm_sc.boolv("wtouch");
    }
    let mut rpd = Record::new();
    let mut l2c = Control::new();
    for t in 0..lm_sc.intv("ticks") {
        let ph = lm_sc.table_at("phase", t as usize);
        l2c.left = flag(&ph, "left");
        l2c.up = flag(&ph, "up");
        l2c.down = flag(&ph, "down");
        l2c.right = flag(&ph, "right");
        l2.nofocus = true;
        l2.drive(
            &mut l2c,
            &mut la,
            &Trackers::new(),
            &mut CheckPoints::new(),
            &mut rig.m,
            &mut rpd,
            &mut rig.xt,
        );
        if t == 0 {
            let e = lm_sc.table("at0");
            check::eq_i32(
                bits(l2.lcomp),
                bits(e.floatv("lcomp")),
                "loopminus_left_lcomp_7",
            );
            check::eq_i32(
                bits(l2.ucomp),
                bits(e.floatv("ucomp")),
                "loopminus_up_ucomp_7",
            );
            check::eq_i32(l2.pxy, e.intv("pxy"), "loopminus_pxy_minus7");
            check::eq_i32(l2.pzy, e.intv("pzy"), "loopminus_pzy_minus7");
        }
        if t == 1 {
            let e = lm_sc.table("at1");
            check::eq_i32(
                bits(l2.lcomp),
                bits(e.floatv("lcomp")),
                "loopminus_left_lcomp_9",
            );
            check::eq_i32(la.y, e.intv("y"), "loopminus_y_71");
        }
        if t == 2 {
            let e = lm_sc.table("at2");
            check::eq_i32(l2.pxy, e.intv("pxy"), "loopminus_pxy_minus27");
            check::eq_i32(la.y, e.intv("y"), "loopminus_y_92");
        }
    }
}

// ────────────────────────────────────────────────────────── collision blocks

/// `c.` wall collisions through colide()-style trackers
/// (`TMadnessDrive.java:300-337`): each flat wall branch pins wall index,
/// skid=2 and the signed impulse recoil.
fn wall_collisions(rig: &mut Rig) {
    for axis in 0..2 {
        for sgn in 0..2 {
            let mut w = car();
            w.y = 240;
            let mut wt = Trackers::new();
            wt.nt = 1;
            let mut wm = mk(1, 98.0);
            let mut wc = Control::new();
            if axis == 0 {
                wallx(
                    &mut wt,
                    0,
                    0,
                    240,
                    0,
                    if sgn == 0 { 90 } else { -90 },
                    100,
                    100,
                    60,
                    20,
                );
                let v = if sgn == 0 { 5.0 } else { -5.0 };
                for i in 0..4 {
                    wm.scx[i] = v;
                }
            } else {
                wallz(
                    &mut wt,
                    0,
                    0,
                    240,
                    0,
                    if sgn == 0 { 90 } else { -90 },
                    100,
                    100,
                    60,
                    20,
                );
                let v = if sgn == 0 { 5.0 } else { -5.0 };
                for i in 0..4 {
                    wm.scz[i] = v;
                }
            }
            wm.nofocus = true;
            wm.drive(
                &mut wc,
                &mut w,
                &wt,
                &mut CheckPoints::new(),
                &mut rig.m,
                &mut Record::new(),
                &mut rig.xt,
            );
            let tag = format!(
                "wall_{}{}",
                if axis == 0 { "xy" } else { "zy" },
                if sgn == 0 { "p" } else { "n" }
            );
            check::eq_i32(wm.skid, 2, &format!("{tag}_skid2"));
            check::eq_i32(wc.wall, 0, &format!("{tag}_wall_index"));
            if axis == 0 {
                check::eq_i32(
                    bits(wm.scx[0]),
                    bits(if sgn == 0 { -1.5000005 } else { 1.5000005 }),
                    &format!("{tag}_scx_recoil"),
                );
            } else {
                check::eq_i32(
                    bits(wm.scz[0]),
                    bits(if sgn == 0 { -1.5000005 } else { 1.5000005 }),
                    &format!("{tag}_scz_recoil"),
                );
            }
        }
    }
}

/// `c2.` a hard wall hit (`TMadnessDrive.java:339-361`): damage
/// 50*1.3*dam(20) = 1300 > 100 records through regz and flips pmlt.
fn hard_wall(rig: &mut Rig) {
    let mut hw = car();
    hw.y = 240;
    let mut hwt = Trackers::new();
    hwt.nt = 1;
    wallz(&mut hwt, 0, 0, 240, 0, 90, 100, 100, 60, 20);
    let mut hwr = Record::new();
    let mut hwm = mk(1, 98.0);
    for i in 0..4 {
        hwm.scz[i] = 50.0;
    }
    hwm.speed = 50.0;
    let mut hwc = Control::new();
    hwm.nofocus = true;
    hwm.drive(
        &mut hwc,
        &mut hw,
        &hwt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut hwr,
        &mut rig.xt,
    );
    check::eq_i32(hwc.wall, 0, "hardwall_wall_index");
    check::eq_i32(hwr.nrz[1][0], 1, "hardwall_regz_recorded");
    check::eq_i32(hwr.magz[1][0][0], -1300, "hardwall_magz_minus1300");
    check::eq_i32(bits(hwm.scz[0]), bits(-15.0), "hardwall_scz_minus15");
    check::eq_i32(hwm.pmlt, -1, "hardwall_pmlt_minus1");
    check::eq_i32(hwm.nmlt, -1, "hardwall_nmlt_minus1");
    check::eq_i32(hw.xz, 0, "hardwall_xz_unchanged");
}

/// `c3.` sloped tracker tiles (xy=45, zy=45)
/// (`TMadnessDrive.java:363-381`).
fn slopes(rig: &mut Rig) {
    for axis in 0..2 {
        let mut sl = car();
        sl.y = 240;
        let mut st = Trackers::new();
        st.nt = 1;
        if axis == 0 {
            wallz(&mut st, 0, 0, 240, 0, 45, 100, 100, 60, 20);
        } else {
            wallx(&mut st, 0, 0, 240, 0, 45, 100, 100, 60, 20);
        }
        let mut sm = mk(1, 98.0);
        sm.nofocus = true;
        sm.drive(
            &mut Control::new(),
            &mut sl,
            &st,
            &mut CheckPoints::new(),
            &mut rig.m,
            &mut Record::new(),
            &mut rig.xt,
        );
        check::eq_i32(
            bits(sm.scy[0]),
            bits(-7.460916),
            &format!("slope{axis}_scy_minus7_46"),
        );
        check::ok(sm.mtouch, &format!("slope{axis}_mtouch"));
        check::ok(sm.wtouch, &format!("slope{axis}_wtouch"));
        check::eq_i32(sl.y, 245, &format!("slope{axis}_y_245"));
    }
}

/// `d.` capsize / attitude normalisation (`TMadnessDrive.java:383-439`).
fn capsize(rig: &mut Rig, mad: &Tml) {
    // pzy=180,pxy=0: var7 only -> capsized, flipy installs -50
    let mut cap = car();
    cap.y = 250;
    let mut cm = mk(1, 98.0);
    cm.pzy = mad.intv("capsnap_result");
    let mut cc = Control::new();
    cm.nofocus = true;
    cm.drive(
        &mut cc,
        &mut cap,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(cm.capsized, "caps_pzy180_capsized");
    check::ok(cc.zyinv, "caps_pzy180_zyinv");
    check::eq_i32(cap.y, 200, "caps_pzy180_y_200");
    check::eq_i32(cap.zy, 180, "caps_pzy180_zy_180");
    // pzy=180 AND pxy=180: var7&&var11 clears var11 -> not capsized
    let mut cap2 = car();
    cap2.y = 250;
    let mut cm2 = mk(1, 98.0);
    cm2.pzy = mad.intv("capsnap_result");
    cm2.pxy = mad.intv("capsnap_result");
    cm2.nofocus = true;
    cm2.drive(
        &mut Control::new(),
        &mut cap2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(!cm2.capsized, "caps_both180_not_capsized");
    check::eq_i32(cap2.y, 250, "caps_both180_y_250");
    check::eq_i32(cap2.xy, 180, "caps_both180_xy_180");
    // pzy=100 (the >90 detection) and pzy=-100 (the >270 wrap)
    let mut cap3 = car();
    cap3.y = 250;
    let mut cm3 = mk(1, 98.0);
    cm3.pzy = mad.intv("capsize_detect") + 10;
    cm3.nofocus = true;
    cm3.drive(
        &mut Control::new(),
        &mut cap3,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(cm3.capsized, "caps_pzy100_capsized");
    check::eq_i32(
        cm3.pzy,
        mad.intv("capsize_detect") + 10,
        "caps_pzy100_pzy_kept",
    );
    check::eq_i32(cap3.zy, 52, "caps_pzy100_zy_52");
    let mut cap4 = car();
    cap4.y = 250;
    let mut cm4 = mk(1, 98.0);
    cm4.pzy = -(mad.intv("capsize_detect") + 10);
    cm4.nofocus = true;
    cm4.drive(
        &mut Control::new(),
        &mut cap4,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        cm4.pzy,
        mad.intv("capsize_full") - (mad.intv("capsize_detect") + 10),
        "caps_pzy_neg100_wraps_260",
    );
    check::eq_i32(cap4.zy, 308, "caps_pzy_neg100_zy_308");
}

/// `d2.` the four-wheel attitude snap to 180/0
/// (`TMadnessDrive.java:427-439`).
fn attitude_snap_fixtures(rig: &mut Rig, mad: &Tml) {
    let mut sn = car();
    sn.y = 250;
    let mut snm = mk(1, 98.0);
    snm.pzy = mad.intv("capsnap_probe");
    snm.pxy = mad.intv("capsnap_deadband") / 2;
    snm.nofocus = true;
    snm.drive(
        &mut Control::new(),
        &mut sn,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(snm.pzy, mad.intv("capsnap_result"), "snap_pzy175_to_180");
    check::eq_i32(sn.zy, mad.intv("capsnap_result"), "snap_zy_180");
    check::eq_i32(snm.pxy, 0, "snap_pxy_lt10_snaps_0");
}

/// `e.` destruction ladder (`TMadnessDrive.java:441-510`).
fn destruction(rig: &mut Rig, mad: &Tml) {
    let mut dc = car();
    dc.y = 240;
    dc.planes
        .set_slot(0, plane(&rig.m, 3, 0, 0, false, false, 0));
    dc.planes.slot_mut(0).expect("slot 0").chip = 4;
    dc.planes.slot_mut(0).expect("slot 0").embos = 7;
    dc.planes
        .set_slot(1, plane(&rig.m, 3, 5, 0, false, false, 0));
    dc.planes.slot_mut(1).expect("slot 1").chip = 2;
    dc.planes.slot_mut(1).expect("slot 1").embos = 9;
    dc.planes.set_npl(2);
    let mut dr = Record::new();
    let mut dm = mk(1, 98.0);
    dm.hitmag = mad.intv("destruction_hitmag");
    dm.nofocus = true;
    dm.drive(
        &mut Control::new(),
        &mut dc,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut dr,
        &mut rig.xt,
    );
    check::eq_i32(dm.cntdest, 1, "dest_cntdest_1");
    check::ok(!dm.dest, "dest_not_yet");
    check::eq_i32(dr.dest[1], 300, "dest_rpd_dest_300");
    // distruct marks wz==0 planes, then the chip/embos sweep (:2089)
    check::eq_i32(
        dc.planes.slot(0).expect("slot 0").embos,
        13,
        "dest_plane0_embos_13",
    );
    check::eq_i32(
        dc.planes.slot(1).expect("slot 1").embos,
        13,
        "dest_plane1_embos_13",
    );
    check::eq_i32(
        dc.planes.slot(0).expect("slot 0").chip,
        0,
        "dest_plane0_chip_0",
    );
    check::eq_i32(
        dc.planes.slot(1).expect("slot 1").chip,
        0,
        "dest_plane1_chip_0",
    );

    // cntdest==7 -> dest=true, dested[1]=1 with lastcolido 0 (:2427)
    let mut dc2 = car();
    dc2.y = 240;
    let mut dcp = CheckPoints::new();
    let mut dm2 = mk(1, 98.0);
    dm2.hitmag = mad.intv("destruction_hitmag");
    dm2.cntdest = mad.intv("destruction_cntdest");
    dm2.nofocus = true;
    dm2.drive(
        &mut Control::new(),
        &mut dc2,
        &Trackers::new(),
        &mut dcp,
        &mut rig.m,
        &mut dr,
        &mut rig.xt,
    );
    check::ok(dm2.dest, "dest_cntdest7_dest_true");
    check::eq_i32(dcp.dested[1], 1, "dest_dested_1");
    // dest && lastcolido!=0 -> dested 2 (:2429-2431)
    let mut dc3 = car();
    dc3.y = 240;
    let mut dcp3 = CheckPoints::new();
    let mut dm3 = mk(1, 98.0);
    dm3.dest = true;
    dm3.lastcolido = 40;
    dm3.nofocus = true;
    dm3.drive(
        &mut Control::new(),
        &mut dc3,
        &Trackers::new(),
        &mut dcp3,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(dcp3.dested[1], 2, "dest_dested_2_with_lastcolido");
    check::eq_i32(dm3.lastcolido, 40, "dest_lastcolido_untouched");
    // undestroyed with a stale dested flag -> cleared (:2433-2434)
    dm3.dest = false;
    dm3.lastcolido = 0;
    dm3.nofocus = true;
    dm3.drive(
        &mut Control::new(),
        &mut dc3,
        &Trackers::new(),
        &mut dcp3,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(dcp3.dested[1], 0, "dest_dested_cleared");

    // e2. fcnt==8 (and 7) reset the whole damage state (:2255-2262)
    let mut f8 = car();
    f8.y = 240;
    f8.fcnt = 8;
    let mut f8m = mk(1, 98.0);
    f8m.hitmag = mad.intv("destruction_hitmag");
    f8m.dest = true;
    f8m.squash = 3;
    f8m.cntdest = 2;
    f8m.nofocus = true;
    f8m.drive(
        &mut Control::new(),
        &mut f8,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(f8m.newcar, "fcnt8_newcar");
    check::ok(!f8m.dest, "fcnt8_dest_false");
    check::eq_i32(f8m.hitmag, 0, "fcnt8_hitmag_0");
    check::eq_i32(f8m.cntdest, 0, "fcnt8_cntdest_0");
    check::eq_i32(f8m.squash, 0, "fcnt8_squash_0");
}

/// `f.` fix points (`TMadnessDrive.java:512-549`).
fn fix_points(rig: &mut Rig) {
    let mut fx = car();
    fx.y = 240;
    fx.dist = 1;
    let mut fxcp = CheckPoints::new();
    fxcp.fn_ = 1;
    fxcp.fx[0] = 0;
    fxcp.fy[0] = 0;
    fxcp.fz[0] = 0;
    let mut fxr = Record::new();
    let mut fxm = mk(1, 98.0);
    fxm.nofocus = true;
    fxm.drive(
        &mut Control::new(),
        &mut fx,
        &Trackers::new(),
        &mut fxcp,
        &mut rig.m,
        &mut fxr,
        &mut rig.xt,
    );
    check::ok(fx.fix, "fix_dist1_sets_fix");
    check::eq_i32(fxr.fix[1], 300, "fix_rpd_fix_300");
    // dist==0 -> fcnt=8, no fix flag (:2228-2229)
    let mut fx2 = car();
    fx2.y = 240;
    let mut fxcp2 = CheckPoints::new();
    fxcp2.fn_ = 1;
    let mut fx2m = mk(1, 98.0);
    fx2m.nofocus = true;
    fx2m.drive(
        &mut Control::new(),
        &mut fx2,
        &Trackers::new(),
        &mut fxcp2,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fx2.fcnt, 8, "fix_dist0_fcnt_8");
    // roted==true uses the z/x projection (:2240-2251)
    let mut fx3 = car();
    fx3.y = 240;
    fx3.dist = 1;
    let mut fxcp3 = CheckPoints::new();
    fxcp3.fn_ = 1;
    fxcp3.roted[0] = true;
    let mut fx3r = Record::new();
    let mut fx3m = mk(1, 98.0);
    fx3m.nofocus = true;
    fx3m.drive(
        &mut Control::new(),
        &mut fx3,
        &Trackers::new(),
        &mut fxcp3,
        &mut rig.m,
        &mut fx3r,
        &mut rig.xt,
    );
    check::ok(fx3.fix, "fix_roted_sets_fix");
    check::eq_i32(fx3r.fix[1], 300, "fix_roted_rpd_300");
}

/// `g.` checkpoints: typ 1 and typ 2 advance clear; g2 typ -3
/// (`TMadnessDrive.java:551-607`).
fn checkpoints(rig: &mut Rig) {
    let mut k = car();
    k.y = 240;
    k.z = 0;
    let mut kcp = CheckPoints::new();
    kcp.n = 3;
    kcp.nsp = 3;
    kcp.typ[0] = 1;
    kcp.x[0] = 0;
    kcp.y[0] = 240;
    kcp.z[0] = 0;
    kcp.typ[1] = 2;
    kcp.x[1] = 0;
    kcp.y[1] = 240;
    kcp.z[1] = 100;
    kcp.typ[2] = 1;
    kcp.x[2] = 0;
    kcp.y[2] = 240;
    kcp.z[2] = 200;
    let mut km = mk(1, 98.0);
    km.clear = 0;
    km.nlaps = 0;
    km.nofocus = true;
    km.drive(
        &mut Control::new(),
        &mut k,
        &Trackers::new(),
        &mut kcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    // typ1 cp0 and typ2 cp1 both clear in one tick (their boxes are
    // 100 apart and cp1's z-window is 700 wide)
    check::eq_i32(km.clear, 2, "cp_tick0_clear_2");
    check::eq_i32(km.pcleared, 1, "cp_tick0_pcleared_1");
    check::eq_i32(km.nlaps, 0, "cp_tick0_nlaps_0");
    km.nofocus = true;
    km.drive(
        &mut Control::new(),
        &mut k,
        &Trackers::new(),
        &mut kcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(km.clear, 2, "cp_tick1_clear_kept_2");
    k.z = 200;
    km.nofocus = true;
    km.drive(
        &mut Control::new(),
        &mut k,
        &Trackers::new(),
        &mut kcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(km.clear, 3, "cp_tick2_clear_3");
    check::eq_i32(km.pcleared, 2, "cp_tick2_pcleared_2");
    check::eq_i32(km.nlaps, 1, "cp_tick2_nlaps_1");
    check::eq_i32(km.point, 0, "cp_tick2_point_0");

    // g2. typ=-3 checkpoints zero the chosen point (:2180-2182)
    let mut t3 = car();
    t3.y = 240;
    let mut t3cp = CheckPoints::new();
    t3cp.n = 1;
    t3cp.nsp = 1;
    t3cp.typ[0] = -3;
    t3cp.x[0] = 0;
    t3cp.z[0] = 0;
    t3cp.y[0] = 240;
    let mut t3m = mk(1, 98.0);
    t3m.nofocus = true;
    t3m.drive(
        &mut Control::new(),
        &mut t3,
        &Trackers::new(),
        &mut t3cp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(t3m.point, 0, "cp_minus3_point_0");
    check::eq_i32(t3m.nlaps, 1, "cp_minus3_nlaps_1");
}

/// `h.` focus / missedcp machine (`TMadnessDrive.java:609-659`).
fn focus_machine(rig: &mut Rig) {
    let mut fcp = CheckPoints::new();
    fcp.n = 3;
    fcp.nsp = 3;
    for i in 0..3 {
        fcp.typ[i] = 1;
        fcp.x[i] = if i == 0 { 50000 } else { 0 };
        fcp.z[i] = i as i32 * 1000;
        fcp.y[i] = 240;
    }
    // car 30000 units away from a focus=1 checkpoint, im==0, mtouch:
    // missedcp 0 -> 1 at dist > 800 (:2194-2198)
    let mut fc = car();
    fc.y = 240;
    fc.x = 30000;
    let mut fm = mk(0, 98.0);
    fm.focus = 1;
    fm.missedcp = 0;
    fm.mtouch = true;
    fm.drive(
        &mut Control::new(),
        &mut fc,
        &Trackers::new(),
        &mut fcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fm.missedcp, 1, "focus_far_missedcp_1");
    check::eq_i32(fm.focus, 1, "focus_far_kept_1");
    // missedcp -2 and dist < 400 -> back to 0 (:2200-2202)
    let mut fc2 = car();
    fc2.y = 240;
    let mut fm2 = mk(0, 98.0);
    fm2.focus = 1;
    fm2.missedcp = -2;
    fm2.mtouch = true;
    fm2.drive(
        &mut Control::new(),
        &mut fc2,
        &Trackers::new(),
        &mut fcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fm2.missedcp, 0, "focus_near_clears_minus2");
    // missedcp!=0 and dist < 250 -> 68 (:2204-2208)
    let mut fc3 = car();
    fc3.y = 240;
    let mut fm3 = mk(0, 98.0);
    fm3.focus = 1;
    fm3.pcleared = 0;
    fm3.missedcp = 1;
    fm3.mtouch = true;
    fm3.drive(
        &mut Control::new(),
        &mut fc3,
        &Trackers::new(),
        &mut fcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fm3.missedcp, 68, "focus_very_near_missedcp_68");
    // nofocus with a live focus clears it (:2213-2216)
    let mut fc4 = car();
    fc4.y = 240;
    let mut fm4 = mk(0, 98.0);
    fm4.focus = 1;
    fm4.missedcp = 3;
    fm4.nofocus = true;
    fm4.drive(
        &mut Control::new(),
        &mut fc4,
        &Trackers::new(),
        &mut fcp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fm4.focus, -1, "focus_nofocus_cleared");
    check::eq_i32(fm4.missedcp, 0, "focus_nofocus_missedcp_0");
}

/// `i.` powerup machine (`TMadnessDrive.java:661-713`).
fn powerup_machine(rig: &mut Rig, mad: &Tml) {
    let mut pu = car();
    pu.y = 240;
    let mut pm = mk(1, 98.0);
    pm.gtouch = true;
    pm.trcnt = mad.intv("trcnt_powerup_arm");
    pm.travxy = 100;
    pm.nofocus = true;
    pm.drive(
        &mut Control::new(),
        &mut pu,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        bits(pm.powerup),
        bits(pm.travxy as f32 / mad.floatv("powerup_travxy_div")),
        "powerup_travxy_4_1667",
    );
    check::eq_i32(
        bits(pm.power),
        bits(mad.floatv("powerup_clamp")),
        "powerup_power_clamped_98",
    );
    check::eq_i32(
        pm.xtpower,
        mad.intv("powerup_xt_low"),
        "powerup_xtpower_100",
    );
    check::eq_i32(pm.trcnt, 10, "powerup_trcnt_10");
    // the flat-trick table: rtab+ftab+btab+surfer == 30+40+40+30 = 140
    let mut pu2 = car();
    pu2.y = 240;
    let mut pm2 = mk(1, 0.0);
    pm2.gtouch = true;
    pm2.trcnt = mad.intv("trcnt_powerup_arm");
    pm2.rtab = true;
    pm2.ftab = true;
    pm2.btab = true;
    pm2.surfer = true;
    pm2.nofocus = true;
    pm2.drive(
        &mut Control::new(),
        &mut pu2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        bits(pm2.powerup),
        bits(
            (mad.intv("powerup_rtab")
                + mad.intv("powerup_ftab")
                + mad.intv("powerup_btab")
                + mad.intv("powerup_surfer")) as f32,
        ),
        "powerup_table_140",
    );
    check::eq_i32(
        bits(pm2.power),
        bits(mad.floatv("powerup_clamp")),
        "powerup_table_power_98",
    );
    check::eq_i32(
        pm2.xtpower,
        mad.intv("powerup_xt_low"),
        "powerup_table_xtpower_100",
    );
    // im==0 cotchinow branch: caught=300 makes cotchinow record the
    // powerup and set hcaught (:2334-2340)
    let mut pu3 = car();
    pu3.y = 240;
    let mut pur = Record::new();
    pur.caught = mad.intv("cotchinow_caught");
    for i in 0..5 {
        pur.car[0][i] = Some(car());
    }
    let mut pm3 = mk(0, 0.0);
    pm3.gtouch = true;
    pm3.trcnt = mad.intv("trcnt_powerup_arm");
    pm3.rtab = true;
    pm3.ftab = true;
    pm3.btab = true;
    pm3.surfer = true;
    pm3.nofocus = true;
    pm3.drive(
        &mut Control::new(),
        &mut pu3,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut pur,
        &mut rig.xt,
    );
    check::eq_i32(
        pur.powered,
        mad.intv("powerup_rtab")
            + mad.intv("powerup_ftab")
            + mad.intv("powerup_btab")
            + mad.intv("powerup_surfer"),
        "powerup_im0_powered_140",
    );
    check::ok(pur.hcaught, "powerup_im0_hcaught");
    check::eq_i32(
        pur.whenwasted,
        mad.intv("cotchinow_whenwasted"),
        "powerup_im0_whenwasted_225",
    );
}

/// `j.` trick accumulation (`TMadnessDrive.java:715-734`).
fn trick_accumulation(rig: &mut Rig) {
    let mut trk = car();
    trk.y = 50;
    let mut tm = mk(1, 98.0);
    tm.loop_ = 2;
    tm.rcomp = 20.0;
    tm.lcomp = 0.0;
    tm.ucomp = 20.0;
    tm.dcomp = 0.0;
    let mut trc = Control::new();
    trc.right = true;
    let mut rpd = Record::new();
    for _ in 0..7 {
        tm.nofocus = true;
        tm.drive(
            &mut trc,
            &mut trk,
            &Trackers::new(),
            &mut CheckPoints::new(),
            &mut rig.m,
            &mut rpd,
            &mut rig.xt,
        );
    }
    check::eq_i32(tm.travxy, 140, "trick_travxy_140");
    check::eq_i32(tm.travzy, 124, "trick_travzy_124");
    check::ok(tm.rtab, "trick_rtab_true");
}

/// `k.` dest locks the capcnt machine (`TMadnessDrive.java:736-765`).
fn dest_capcnt(rig: &mut Rig, mad: &Tml) {
    let mut dd = car();
    dd.y = 240;
    let mut dmc = mk(1, 98.0);
    dmc.dest = true;
    dmc.mtouch = true;
    dmc.capcnt = 1;
    for _ in 0..mad.intv("capsize_respawn_ticks") - 1 {
        tick_on(
            rig,
            &mut dd,
            &mut Control::new(),
            &mut CheckPoints::new(),
            &mut dmc,
        );
    }
    check::eq_i32(dmc.capcnt, 1, "dest_capcnt_not_cycled_1");
    check::eq_i32(bits(dmc.speed), bits(1.5), "dest_speed_bleeds_to_1_5");
    // the trcnt!=0 reset arm
    let mut dt = car();
    dt.y = 240;
    let mut dmt = mk(1, 98.0);
    dmt.dest = true;
    dmt.mtouch = true;
    dmt.trcnt = 5;
    dmt.travxy = 10;
    dmt.surfer = true;
    dmt.nofocus = true;
    dmt.drive(
        &mut Control::new(),
        &mut dt,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(dmt.trcnt, 5, "destcap_trcnt5_untouched");
    check::eq_i32(dmt.travxy, 10, "destcap_travxy_untouched");
}

/// `l.` attitude snap onto the ContO (`TMadnessDrive.java:767-786`).
fn attitude_onto_conto(rig: &mut Rig) {
    // pxy 10 / xy 0 (deadband) and xy 100 -> 48 (the 2+half step)
    let mut at = car();
    at.y = 0;
    let mut am2 = mk(1, 98.0);
    am2.pxy = 10;
    am2.pzy = -10;
    am2.nofocus = true;
    am2.drive(
        &mut Control::new(),
        &mut at,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(at.xy, 7, "att_pxy10_xy7");
    check::eq_i32(at.zy, -7, "att_pzy_minus10_zy_minus7");
    let mut at2 = car();
    at2.y = 0;
    at2.xy = 100;
    at2.zy = -100;
    let mut am3 = mk(1, 98.0);
    am3.nofocus = true;
    am3.drive(
        &mut Control::new(),
        &mut at2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(at2.xy, 48, "att_xy100_xy48");
    check::eq_i32(at2.zy, -48, "att_zy_minus100_zy_minus48");
}

/// `m.` impulse clamps (`TMadnessDrive.java:788-800`).
fn impulse_clamps(rig: &mut Rig) {
    // scx[0]=500 with the other three at 0: avg 125, 500-125 > 200, so
    // scx[0] becomes 325; scz[0]=-500 -> avg -125 -> -325.
    let mut ic = car();
    ic.y = 240;
    let mut im = mk(1, 98.0);
    im.scx[0] = 500.0;
    im.scz[0] = -500.0;
    im.nofocus = true;
    im.drive(
        &mut Control::new(),
        &mut ic,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(im.scx[0]), bits(325.0), "imp_scx_clamp_325");
    check::eq_i32(bits(im.scx[1]), bits(0.0), "imp_scx1_unchanged_0");
    check::eq_i32(bits(im.scz[0]), bits(-325.0), "imp_scz_clamp_minus325");
}

/// `n.` mxz auto-turn (`TMadnessDrive.java:802-807`).
fn mxz_autoturn(rig: &mut Rig, mad: &Tml) {
    check::eq_i32(cxz_tick(rig, 100.0, 0, 170), 10, "mxz_drift_step_10");
    check::eq_i32(cxz_tick(rig, 50.0, 0, 100), 10, "mxz_small_gap_quarter_10");
    check::eq_i32(
        cxz_tick(rig, 50.0, 0, 200),
        mad.intv("mxz_wrap_result"),
        "mxz_wrap_360_to_350",
    );
}

/// One tick returning the resulting cxz (`TMadnessDrive.java:2270-2281`).
fn cxz_tick(rig: &mut Rig, speed: f32, cxz: i32, mxz: i32) -> i32 {
    let mut c = car();
    c.y = 240;
    let mut md = mk(1, 98.0);
    md.speed = speed;
    md.cxz = cxz;
    md.mxz = mxz;
    md.nofocus = true;
    md.drive(
        &mut Control::new(),
        &mut c,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    md.cxz
}

/// `o.` ground clamp with the cn==7 bounce
/// (`TMadnessDrive.java:809-823`).
fn ground_cn7(rig: &mut Rig) {
    let mut g7 = car();
    g7.y = 240;
    let mut gm = mk(1, 98.0);
    gm.cn = 7;
    for i in 0..4 {
        gm.scy[i] = 150.0;
    }
    gm.nofocus = true;
    gm.drive(
        &mut Control::new(),
        &mut g7,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(g7.y, 250, "ground7_y_250");
    check::eq_i32(bits(gm.scy[0]), bits(-15.699997), "ground7_scy_minus15_7");
    check::ok(gm.mtouch, "ground7_mtouch");
}

/// `p.` skid scatter with a skd==3 tracker
/// (`TMadnessDrive.java:825-843`): a FRESH VecMedium.
fn skid_scatter(_rig: &mut Rig, vec: &[f32]) {
    let mut skm = vec_rig(vec);
    let mut sc = car();
    sc.y = 240;
    let mut sct = Trackers::new();
    sct.nt = 1;
    floor(&mut sct, 0, 0, 240, 0, 3, 20, 100, 100, 40);
    let mut scm = mk(1, 98.0);
    scm.dcnt = 100;
    scm.mtouch = true;
    scm.txz = 1;
    scm.speed = 50.0;
    scm.nofocus = true;
    scm.drive(
        &mut Control::new(),
        &mut sc,
        &sct,
        &mut CheckPoints::new(),
        &mut skm.m,
        &mut Record::new(),
        &mut skm.xt,
    );
    check::eq_i32(scm.dcnt, 104, "skid3_dcnt_104");
    check::eq_i32(scm.skid, 1, "skid3_skid_1");
    check::eq_i32(bits(scm.scy[0]), bits(-7.8750005), "skid3_scy0_minus7_875");
}

/// `q.` regy/regz/regx damage scatter with wz==0 planes
/// (`TMadnessDrive.java:845-912`): one FRESH VecMedium shared by the block.
fn reg_scatter(_rig: &mut Rig, vec: &[f32]) {
    let mut rg = vec_rig(vec);
    let mut ry = car();
    ry.planes
        .set_slot(0, plane(&rg.m, 3, 0, 25, false, false, 0));
    ry.planes
        .set_slot(1, plane(&rg.m, 3, 0, 65, true, false, 0));
    ry.planes.set_npl(2);
    let mut ryr = Record::new();
    let mut rym = Madness::new(1);
    rym.cn = 0;
    rym.mtouch = true;
    rym.regy(
        0,
        150.0,
        &mut ry,
        Some(&mut rg.m),
        Some(&mut ryr),
        Some(&mut rg.xt),
    );
    check::eq_i32(ryr.nry[1][0], 1, "regy_scatter_recorded");
    check::eq_i32(
        ry.planes.slot(0).expect("slot 0").chip,
        1,
        "regy_scatter_chip",
    );
    check::eq_i32(
        ry.planes.slot(0).expect("slot 0").bfase,
        26,
        "regy_scatter_bfase_26",
    );
    check::eq_i32(
        ry.planes.slot(1).expect("slot 1").bfase,
        65,
        "regy_scatter_bfase_65",
    );
    check::eq_i32(
        ry.planes.slot(1).expect("slot 1").gr,
        -1,
        "regy_scatter_glass_gr_minus1",
    );
    check::eq_i32(
        bits(ry.planes.slot(0).expect("slot 0").hsb[1]),
        bits(0.2),
        "regy_scatter_hsb1_0_2",
    );
    // the squash ladder: zy=180 (var4=-1) x xy=0 (var5=1) -> product -1
    let mut sq = car();
    sq.zy = 180;
    sq.planes
        .set_slot(0, plane(&rg.m, 3, 0, 0, false, false, -50));
    sq.planes.set_npl(1);
    let mut sqm = Madness::new(1);
    sqm.cn = 0;
    sqm.mtouch = true;
    sqm.regy(
        0,
        150.0,
        &mut sq,
        Some(&mut rg.m),
        Some(&mut Record::new()),
        Some(&mut rg.xt),
    );
    check::eq_i32(sqm.nbsq, 1, "regy_squash_first_strike_nbsq_1");
    sqm.regy(
        0,
        150.0,
        &mut sq,
        Some(&mut rg.m),
        Some(&mut Record::new()),
        Some(&mut rg.xt),
    );
    check::eq_i32(sqm.nbsq, 0, "regy_squash_consumed_nbsq_0");
    check::eq_i32(
        sq.planes.slot(0).expect("slot 0").oy[0],
        -49,
        "regy_squash_oy_minus49",
    );

    let mut rz = car();
    rz.planes
        .set_slot(0, plane(&rg.m, 3, 0, 25, false, false, 0));
    rz.planes
        .set_slot(1, plane(&rg.m, 3, 0, 65, true, false, 0));
    rz.planes.set_npl(2);
    let mut rzr = Record::new();
    let mut rzm = Madness::new(1);
    rzm.cn = 0;
    rzm.regz(
        0,
        150.0,
        &mut rz,
        Some(&mut rg.m),
        Some(&mut rzr),
        Some(&mut rg.xt),
    );
    check::eq_i32(rzr.nrz[1][0], 1, "regz_scatter_recorded");
    check::eq_i32(
        rz.planes.slot(0).expect("slot 0").chip,
        1,
        "regz_scatter_chip",
    );
    check::eq_i32(
        rz.planes.slot(0).expect("slot 0").bfase,
        26,
        "regz_scatter_bfase_26",
    );
    check::eq_i32(
        rz.planes.slot(1).expect("slot 1").gr,
        -2,
        "regz_scatter_glass_gr_minus2",
    );
    check::eq_i32(
        rz.planes.slot(0).expect("slot 0").oz[0],
        2,
        "regz_scatter_oz_2",
    );

    let mut rx = car();
    rx.planes
        .set_slot(0, plane(&rg.m, 3, 0, 25, false, false, 0));
    rx.planes
        .set_slot(1, plane(&rg.m, 3, 0, 65, true, false, 0));
    rx.planes.set_npl(2);
    let mut rxr = Record::new();
    let mut rxm = Madness::new(1);
    rxm.cn = 0;
    rxm.regx(
        0,
        150.0,
        &mut rx,
        Some(&mut rg.m),
        Some(&mut rxr),
        Some(&mut rg.xt),
    );
    check::eq_i32(rxr.nrx[1][0], 1, "regx_scatter_recorded");
    check::eq_i32(
        rx.planes.slot(0).expect("slot 0").chip,
        1,
        "regx_scatter_chip",
    );
    check::eq_i32(
        rx.planes.slot(1).expect("slot 1").gr,
        -1,
        "regx_scatter_glass_gr_minus1",
    );
    // the nocol/glass sweep at bfase 55/61 (:188-217)
    let mut rx2 = car();
    rx2.planes
        .set_slot(0, plane(&rg.m, 3, 0, 55, false, true, 0));
    rx2.planes
        .set_slot(1, plane(&rg.m, 3, 0, 61, false, false, 0));
    rx2.planes.set_npl(2);
    let mut rx2m = Madness::new(1);
    rx2m.cn = 0;
    rx2m.regx(
        0,
        150.0,
        &mut rx2,
        Some(&mut rg.m),
        Some(&mut Record::new()),
        Some(&mut rg.xt),
    );
    check::eq_i32(
        bits(rx2.planes.slot(1).expect("slot 1").hsb[0]),
        bits(0.05),
        "regx_bfase61_hsb0_0_05",
    );
}

/// `r.` colide push clamps and the caught gate
/// (`TMadnessDrive.java:914-958`).
fn colide_pins(rig: &mut Rig) {
    // revpush/push are clamped to +-300; the dominant car's impulse is
    // reduced by the reversed one (:456-538).
    let mut ca = car();
    let mut cb = car();
    ca.maxr = 10;
    cb.maxr = 10;
    let mut oth = mk(1, 75.0);
    oth.speed = 5.0;
    let mut me = mk(1, 75.0);
    me.speed = 10.0;
    for i in 0..4 {
        me.scx[i] = 400.0;
        oth.scx[i] = 300.0;
    }
    me.colide(
        &mut ca,
        &mut oth,
        &mut cb,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(me.dominate[1], "colide_clamp_dominates");
    check::eq_i32(bits(me.scx[0]), bits(100.0), "colide_clamp_this_scx_100");
    check::eq_i32(bits(oth.scx[0]), bits(600.0), "colide_clamp_other_scx_600");
    // caught already true suppresses the domination rewrite (:419)
    let mut cc2 = car();
    let mut cd = car();
    cc2.maxr = 10;
    cd.maxr = 10;
    let mut oth2 = mk(1, 75.0);
    oth2.speed = 5.0;
    let mut me2 = mk(1, 75.0);
    me2.speed = 10.0;
    me2.caught[1] = true;
    me2.colide(
        &mut cc2,
        &mut oth2,
        &mut cd,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(!me2.dominate[1], "colide_caught_gate_keeps_dominate");
    check::ok(me2.caught[1], "colide_caught_gate_keeps_caught");
    // this.capsized uses flipy+squash for the wheel height (:394-406)
    let mut ce = car();
    let mut cf = car();
    ce.maxr = 10;
    cf.maxr = 10;
    let mut oth3 = mk(1, 75.0);
    oth3.speed = 5.0;
    let mut me3 = mk(1, 75.0);
    me3.capsized = true;
    me3.squash = 3;
    me3.speed = 10.0;
    me3.colide(
        &mut ce,
        &mut oth3,
        &mut cf,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(me3.dominate[1], "colide_capsized_dominates");
}

/// `s.` im==0 wall reset (`TMadnessDrive.java:960-976`).
fn im0_wall_reset(rig: &mut Rig) {
    // The wall must stay below the 100 damage gate (scx 1, dam 20 -> 26)
    // so im==0 never reaches the xt crash at :663.
    let mut iw = car();
    iw.y = 240;
    let mut iwt = Trackers::new();
    iwt.nt = 1;
    wallx(&mut iwt, 0, 0, 240, 0, 90, 100, 100, 60, 20);
    let mut iwm = mk(0, 98.0);
    for i in 0..4 {
        iwm.scx[i] = 1.0;
    }
    let mut iwc = Control::new();
    iwm.nofocus = true;
    iwm.drive(
        &mut iwc,
        &mut iw,
        &iwt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(iwc.wall, -1, "im0_wall_reset_minus1");
    check::eq_i32(iwm.hitmag, 0, "im0_wall_below_damage_gate");
}

/// `u1-u5`: loop==2 / loop==-1 releases, the upside-down air arms, the
/// capsized respawn flip and the capsized skid-2 speed recompute.
fn residual_air(rig: &mut Rig, mad: &Tml) {
    // u1. loop==2 release arms (:863-918)
    let mut ua = car();
    ua.y = 50;
    let mut um = mk(1, 98.0);
    um.loop_ = 2;
    um.ucomp = 6.0;
    um.dcomp = 6.0;
    um.lcomp = 6.0;
    um.rcomp = 6.0;
    um.nofocus = true;
    um.drive(
        &mut Control::new(),
        &mut ua,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(um.ucomp), bits(5.5), "air_release_ucomp_5_5");
    check::eq_i32(bits(um.dcomp), bits(5.5), "air_release_dcomp_5_5");
    check::eq_i32(bits(um.lcomp), bits(4.0), "air_release_lcomp_4");
    check::eq_i32(bits(um.rcomp), bits(4.0), "air_release_rcomp_4");
    // u2. loop==-1 release arms (:1003-1063)
    let mut ub = car();
    ub.y = 50;
    let mut um2 = mk(1, 98.0);
    um2.loop_ = -1;
    um2.lcomp = 6.0;
    um2.rcomp = 6.0;
    um2.ucomp = 6.0;
    um2.dcomp = 6.0;
    um2.pl = true;
    um2.pr = true;
    um2.pu = true;
    um2.pd = true;
    um2.nofocus = true;
    um2.drive(
        &mut Control::new(),
        &mut ub,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(um2.lcomp), bits(4.0), "loopminus_release_lcomp_4");
    check::eq_i32(bits(um2.rcomp), bits(4.0), "loopminus_release_rcomp_4");
    check::eq_i32(bits(um2.ucomp), bits(4.0), "loopminus_release_ucomp_4");
    check::eq_i32(bits(um2.dcomp), bits(4.0), "loopminus_release_dcomp_4");
    check::ok(!um2.pl, "loopminus_pl_cleared");
    // u3. the var7==true (upside-down) air rotation arms (:921-925,:1066-1070)
    let mut uc = car();
    uc.y = 50;
    let mut um3 = mk(1, 98.0);
    um3.loop_ = 2;
    um3.pzy = 180;
    um3.pxy = 0;
    um3.ucomp = 10.0;
    um3.dcomp = 0.0;
    um3.nofocus = true;
    um3.drive(
        &mut Control::new(),
        &mut uc,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(um3.pzy != 0, "air_upside_down_pzy_moves");
    let mut ud = car();
    ud.y = 50;
    let mut um4 = mk(1, 98.0);
    um4.loop_ = -1;
    um4.pzy = 180;
    um4.ucomp = 8.0;
    um4.nofocus = true;
    um4.drive(
        &mut Control::new(),
        &mut ud,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(um4.pzy != 0, "loopminus_upside_down_pzy_moves");
    // u4. the capsized respawn flip (:2367-2401)
    let mut cflc = car();
    cflc.y = 240;
    let mut cfl = mk(1, 98.0);
    cfl.pzy = 180;
    cfl.pxy = 0;
    cfl.capcnt = 1;
    for _ in 0..mad.intv("capsize_respawn_ticks") - 1 {
        cfl.nofocus = true;
        cfl.drive(
            &mut Control::new(),
            &mut cflc,
            &Trackers::new(),
            &mut CheckPoints::new(),
            &mut rig.m,
            &mut Record::new(),
            &mut rig.xt,
        );
    }
    check::eq_i32(cfl.capcnt, 0, "capsflip_capcnt_cycle_0");
    check::eq_i32(bits(cfl.speed), bits(0.0), "capsflip_speed_0");
    // a fresh capsized car with capcnt 0 and scx/scz inside 70 arms it
    let mut cf2 = car();
    cf2.y = 240;
    let mut cfl2 = mk(1, 98.0);
    cfl2.pzy = 180;
    cfl2.pxy = 0;
    cfl2.nofocus = true;
    cfl2.drive(
        &mut Control::new(),
        &mut cf2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(cfl2.capcnt, 1, "capsflip_capcnt_arm_1");
    // the !dest && !capsized arm clears a stale capcnt (:2300-2301)
    let mut cf3 = car();
    cf3.y = 240;
    let mut cfl3 = mk(1, 98.0);
    cfl3.capcnt = 5;
    cfl3.nofocus = true;
    cfl3.drive(
        &mut Control::new(),
        &mut cf3,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(cfl3.capcnt, 0, "caps_stale_capcnt_cleared");
    // u5. skid==2 with capsized+var8 recomputes speed negative (:1389-1399)
    let mut sg = car();
    sg.y = 240;
    let mut sgm = mk(1, 98.0);
    sgm.mtouch = true;
    sgm.speed = 20.0;
    sgm.skid = 2;
    sgm.pzy = 180;
    sgm.pxy = 100;
    sgm.txz = 0;
    sgm.nofocus = true;
    sgm.drive(
        &mut Control::new(),
        &mut sg,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(sgm.speed), bits(15.0), "caps_skid2_speed_15");
    // capsized with zero total impulse zeroes the tracker skid (:1403)
    let mut sz = car();
    sz.y = 240;
    let mut szm = mk(1, 98.0);
    szm.mtouch = true;
    szm.capsized = true;
    szm.skid = 2;
    szm.nofocus = true;
    szm.drive(
        &mut Control::new(),
        &mut sz,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(szm.speed <= 0.0, "caps_zero_impulse_speed");
}

/// `u6-u12`: slope dust, the wzy wrap, xtpower/lastcolido decay, the negative
/// impulse clamps, ground dust, the powerup >150 arm and the trick boundaries.
fn residual_motion(rig: &mut Rig, mad: &Tml, vecmedium: &[f32]) {
    // u6. sloped tracker with a NON-zero skid: the dust arm (:1686-1689)
    let mut slm = vec_rig(vecmedium);
    let mut ss = car();
    ss.y = 240;
    let mut sst = Trackers::new();
    sst.nt = 1;
    wallz(&mut sst, 0, 0, 240, 0, 45, 100, 100, 60, 20);
    sst.skd[0] = 1;
    let mut ssm = mk(1, 98.0);
    ssm.nofocus = true;
    ssm.drive(
        &mut Control::new(),
        &mut ss,
        &sst,
        &mut CheckPoints::new(),
        &mut slm.m,
        &mut Record::new(),
        &mut slm.xt,
    );
    check::ok(ssm.scy[0] < 0.0, "slope_skid1_scy_negative");
    // same tile but mtouch already true -> var56 true skips the dust
    let mut ss2 = car();
    ss2.y = 240;
    let mut ssm2 = mk(1, 98.0);
    ssm2.mtouch = true;
    ssm2.skid = 2;
    ssm2.speed = 10.0;
    ssm2.nofocus = true;
    ssm2.drive(
        &mut Control::new(),
        &mut ss2,
        &sst,
        &mut CheckPoints::new(),
        &mut slm.m,
        &mut Record::new(),
        &mut slm.xt,
    );
    check::ok(ssm2.scy[0] < 0.0, "slope_mtouch_scy_negative");
    // u7. wzy positive wrap (:1087-1088) and handb on negative speed
    let mut ww = car();
    ww.y = 240;
    ww.wzy = 40;
    let mut wwm = mk(1, 98.0);
    wwm.speed = -280.0;
    wwm.nofocus = true;
    wwm.drive(
        &mut Control::new(),
        &mut ww,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(ww.wzy, 35, "wzy_plus40_wraps_35");
    let mut hb = car();
    hb.y = 240;
    let mut hbm = mk(1, 98.0);
    hbm.speed = -100.0;
    hbm.mtouch = true;
    let mut hbc = Control::new();
    hbc.handb = true;
    hbm.nofocus = true;
    hbm.drive(
        &mut hbc,
        &mut hb,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(hbm.speed > -100.0, "handb_negative_speed_rises");
    // u8. xtpower countdown (:2411-2413) and lastcolido decay (:2421-2423)
    let mut xp = car();
    xp.y = 240;
    let mut xpm = mk(1, 98.0);
    xpm.xtpower = 5;
    xpm.nofocus = true;
    xpm.drive(
        &mut Control::new(),
        &mut xp,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(xpm.xtpower, 4, "xtpower_decays_4");
    let mut lc2 = car();
    lc2.y = 240;
    let mut lcm = mk(1, 98.0);
    lcm.lastcolido = 5 * mad.intv("lastcolido_decay");
    lcm.nofocus = true;
    lcm.drive(
        &mut Control::new(),
        &mut lc2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        lcm.lastcolido,
        4 * mad.intv("lastcolido_decay"),
        "lastcolido_decays_4",
    );
    // u9. the negative impulse clamps (:1207-1216)
    let mut nc = car();
    nc.y = 240;
    let mut ncm = mk(1, 98.0);
    ncm.scx[0] = -500.0;
    ncm.scz[0] = 500.0;
    ncm.nofocus = true;
    ncm.drive(
        &mut Control::new(),
        &mut nc,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(ncm.scx[0]), bits(-325.0), "imp_scx_neg_clamp_minus325");
    check::eq_i32(bits(ncm.scz[0]), bits(325.0), "imp_scz_pos_clamp_325");
    // u10. ground dust with var27==0 and a non-7 scy (:1422-1434)
    let mut gd = car();
    gd.y = 240;
    let mut gdm = mk(1, 98.0);
    for i in 0..4 {
        gdm.scy[i] = 100.0;
    }
    gdm.nofocus = true;
    gdm.drive(
        &mut Control::new(),
        &mut gd,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(gd.y == 250, "ground_dust_y_250");
    // u11. powerup: travzy/travxz terms and the >150 xtpower arm
    let mut pz = car();
    pz.y = 240;
    let mut pzm = mk(1, 0.0);
    pzm.gtouch = true;
    pzm.trcnt = 9;
    pzm.travxy = 2000;
    pzm.travzy = 2000;
    pzm.travxz = 2000;
    pzm.rtab = true;
    pzm.ftab = true;
    pzm.btab = true;
    pzm.surfer = true;
    pzm.nofocus = true;
    pzm.drive(
        &mut Control::new(),
        &mut pz,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        pzm.xtpower,
        mad.intv("powerup_xt_high"),
        "powerup_over150_xtpower_200",
    );
    check::ok(
        pzm.powerup > mad.floatv("powerup_xt_threshold"),
        "powerup_over150_value",
    );
    // u12. trick table boundaries: ftab above +135 and btab below -135
    let mut tb = car();
    tb.y = 50;
    let mut tbm = mk(1, 98.0);
    tbm.loop_ = 2;
    tbm.travzy = mad.intv("trick_rotate") - mad.intv("trick_probe_delta");
    tbm.ucomp = 20.0;
    tbm.dcomp = 0.0;
    tbm.nofocus = true;
    tbm.drive(
        &mut Control::new(),
        &mut tb,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(tbm.ftab, "trick_ftab_over_135");
    let mut tb2 = car();
    tb2.y = 50;
    let mut tbm2 = mk(1, 98.0);
    tbm2.loop_ = 2;
    tbm2.travzy = -(mad.intv("trick_rotate") - mad.intv("trick_probe_delta"));
    tbm2.ucomp = 0.0;
    tbm2.dcomp = 20.0;
    tbm2.nofocus = true;
    tbm2.drive(
        &mut Control::new(),
        &mut tb2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(tbm2.btab, "trick_btab_below_minus135");
}

/// `u13-u17`: the checkpoint var6 arms, the focus repick, the im!=0 missedcp
/// set and the fix-point misses plus the fcnt==7 reset.
fn residual_checkpoints(rig: &mut Rig) {
    // u13. checkpoint var6 arms (:2114-2115,:2129-2130)
    let mut cv = car();
    cv.y = 240;
    cv.z = 5000;
    let mut cvc = CheckPoints::new();
    cvc.n = 2;
    cvc.nsp = 2;
    cvc.typ[0] = 1;
    cvc.x[0] = 0;
    cvc.y[0] = 240;
    cvc.z[0] = 0;
    cvc.typ[1] = 2;
    cvc.x[1] = 0;
    cvc.y[1] = 240;
    cvc.z[1] = 5000;
    let mut cvm = mk(1, 98.0);
    cvm.clear = 1;
    cvm.nofocus = true;
    cvm.drive(
        &mut Control::new(),
        &mut cv,
        &Trackers::new(),
        &mut cvc,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(cvm.clear, 2, "cp_var6_type2_clear_advances_2");
    // and the type-1 arm with clear==1 at the first checkpoint
    let mut cv2 = car();
    cv2.y = 240;
    let mut cvc2 = CheckPoints::new();
    cvc2.n = 2;
    cvc2.nsp = 2;
    cvc2.typ[0] = 1;
    cvc2.x[0] = 0;
    cvc2.y[0] = 240;
    cvc2.z[0] = 0;
    cvc2.typ[1] = 1;
    cvc2.x[1] = 50000;
    cvc2.y[1] = 240;
    cvc2.z[1] = 0;
    let mut cv2m = mk(1, 98.0);
    cv2m.clear = 1;
    cv2m.nofocus = true;
    cv2m.drive(
        &mut Control::new(),
        &mut cv2,
        &Trackers::new(),
        &mut cvc2,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(cv2m.clear, 1, "cp_var6_type1_clear_kept_1");
    // u14. focus repick with var114 < pcleared (:2170-2172)
    let mut fr = car();
    fr.y = 240;
    fr.x = 5000;
    let mut frc = CheckPoints::new();
    frc.n = 4;
    frc.nsp = 4;
    for i in 0..4 {
        frc.typ[i] = 1;
        frc.x[i] = i as i32 * 20000;
        frc.z[i] = 0;
        frc.y[i] = 240;
    }
    fr.x = 25000;
    let mut frm = mk(0, 98.0);
    frm.focus = -1;
    frm.pcleared = 1;
    frm.clear = 1;
    frm.nlaps = 0;
    frm.drive(
        &mut Control::new(),
        &mut fr,
        &Trackers::new(),
        &mut frc,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(frm.point, 2, "focus_repick_point_2");
    check::eq_i32(frm.focus, 2, "focus_repick_focus_2");
    // u15. focus!= -1 with im!=0 always sets missedcp=1 (:2209-2210)
    let mut fi = car();
    fi.y = 240;
    fi.x = 30000;
    let mut fim = mk(1, 98.0);
    fim.focus = 1;
    fim.missedcp = 0;
    fim.mtouch = true;
    fim.drive(
        &mut Control::new(),
        &mut fi,
        &Trackers::new(),
        &mut frc,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fim.missedcp, 1, "focus_im1_missedcp_1");
    // u16. fix point misses: far z (:2227) and far x on the roted arm (:2240)
    let mut fz = car();
    fz.y = 240;
    let mut fzc = CheckPoints::new();
    fzc.fn_ = 1;
    fzc.fz[0] = 5000;
    let mut fzm = mk(1, 98.0);
    fzm.nofocus = true;
    fzm.drive(
        &mut Control::new(),
        &mut fz,
        &Trackers::new(),
        &mut fzc,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fz.fcnt, 0, "fix_far_z_no_fix");
    let mut fz2 = car();
    fz2.y = 240;
    let mut fzc2 = CheckPoints::new();
    fzc2.fn_ = 1;
    fzc2.roted[0] = true;
    fzc2.fx[0] = 5000;
    let mut fz2m = mk(1, 98.0);
    fz2m.nofocus = true;
    fz2m.drive(
        &mut Control::new(),
        &mut fz2,
        &Trackers::new(),
        &mut fzc2,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fz2.fcnt, 0, "fix_roted_far_x_no_fix");
    // u17. fcnt==7 also triggers the reset block
    let mut f7 = car();
    f7.y = 240;
    f7.fcnt = 7;
    let mut f7m = mk(1, 98.0);
    f7m.dest = true;
    f7m.nofocus = true;
    f7m.drive(
        &mut Control::new(),
        &mut f7,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(f7m.newcar, "fcnt7_newcar");
    check::ok(!f7m.dest, "fcnt7_dest_false");
}

/// `u18`: the colide weaker/tie/z-push arms.
fn residual_colide_ties(rig: &mut Rig) {
    // u18. colide: the weaker car loses (:424), the moment tie with a
    // smaller moment loses (:429), and the z-push block clamps
    let mut wa = car();
    let mut wb = car();
    wa.maxr = 10;
    wb.maxr = 10;
    let mut othw = mk(1, 60.0);
    othw.speed = 20.0;
    let mut mew = mk(1, 60.0);
    mew.speed = 5.0;
    mew.colide(
        &mut wa,
        &mut othw,
        &mut wb,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(!mew.dominate[1], "colide_weaker_dominate_false");
    check::ok(mew.caught[1], "colide_weaker_caught_true");
    // equal power*speed*moment, smaller moment -> false (:429)
    let mut wc2 = car();
    let mut wd2 = car();
    wc2.maxr = 10;
    wd2.maxr = 10;
    let mut oth2b = mk(1, 50.0);
    oth2b.cn = 0; // moment 1.2
    oth2b.speed = 2.0;
    let mut me2b = mk(1, 60.0);
    me2b.cn = 3; // moment 1.0
    me2b.speed = 2.0;
    me2b.colide(
        &mut wc2,
        &mut oth2b,
        &mut wd2,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(!me2b.dominate[1], "colide_moment_tie_loses");
    // z push with clamps (:498-536)
    let mut ze = car();
    let mut zf = car();
    ze.maxr = 10;
    zf.maxr = 10;
    let mut oz = mk(1, 70.0);
    oz.speed = 4.0;
    let mut mz = mk(1, 70.0);
    mz.speed = 8.0;
    for i in 0..4 {
        mz.scz[i] = 500.0;
        oz.scz[i] = -400.0;
    }
    mz.colide(
        &mut ze,
        &mut oz,
        &mut zf,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(mz.dominate[1], "colide_zpush_dominates");
    check::ok(mz.scz[0] != 500.0, "colide_zpush_this_moved");
    // other.im==0 sets this.lastcolido (:544-545) - keep the push small
    let mut zi = car();
    let mut zj = car();
    zi.maxr = 10;
    zj.maxr = 10;
    let mut oi = mk(0, 75.0);
    oi.speed = 5.0;
    let mut mi = mk(1, 75.0);
    mi.speed = 10.0;
    mi.scx[0] = 10.0;
    mi.colide(
        &mut zi,
        &mut oi,
        &mut zj,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(mi.lastcolido, 70, "colide_other_im0_sets_this_lastcolido");
}

/// `u19`: the deeper colour bands on all three reg* scatter blocks.
fn residual_colide_bands(_rig: &mut Rig, vecmedium: &[f32]) -> Rig {
    // u19. the deeper colour bands on all three reg* scatter blocks
    let mut bandm = vec_rig(vecmedium);
    for band in 0..3 {
        let bf = 35 + band * 15; // 35, 50, 65
        let mut bc = car();
        bc.planes
            .set_slot(0, plane(&bandm.m, 3, 0, bf, false, false, 0));
        bc.planes
            .set_slot(1, plane(&bandm.m, 3, 0, bf, true, false, 0));
        bc.planes.set_npl(2);
        let mut bm = Madness::new(1);
        bm.cn = 0;
        bm.mtouch = true;
        bm.regy(
            0,
            150.0,
            &mut bc,
            Some(&mut bandm.m),
            Some(&mut Record::new()),
            Some(&mut bandm.xt),
        );
        check::ok(
            bc.planes.slot(0).expect("slot 0").bfase > bf,
            &format!("regy_band{bf}_damaged"),
        );
    }
    let mut bz = car();
    bz.planes
        .set_slot(0, plane(&bandm.m, 3, 0, 65, false, false, 0));
    bz.planes
        .set_slot(1, plane(&bandm.m, 3, 0, 65, true, false, 0));
    bz.planes.set_npl(2);
    let mut bzm = Madness::new(1);
    bzm.cn = 0;
    bzm.regz(
        0,
        150.0,
        &mut bz,
        Some(&mut bandm.m),
        Some(&mut Record::new()),
        Some(&mut bandm.xt),
    );
    check::ok(
        bz.planes.slot(0).expect("slot 0").bfase > 65,
        "regz_band65_damaged",
    );
    let mut bx = car();
    bx.planes
        .set_slot(0, plane(&bandm.m, 3, 0, 65, false, false, 0));
    bx.planes
        .set_slot(1, plane(&bandm.m, 3, 0, 65, true, false, 0));
    bx.planes.set_npl(2);
    let mut bxm = Madness::new(1);
    bxm.cn = 0;
    bxm.regx(
        0,
        150.0,
        &mut bx,
        Some(&mut bandm.m),
        Some(&mut Record::new()),
        Some(&mut bandm.xt),
    );
    check::ok(
        bx.planes.slot(0).expect("slot 0").bfase > 65,
        "regx_band65_damaged",
    );
    bandm
}

/// `u20-u22`: the loop==-1 press arms, the pushed handb bite and the colide moment ties.
fn residual_colide_press(rig: &mut Rig) {
    // u20. loop==-1 press arms with the flags clear (:993-1063)
    let mut l3 = car();
    l3.y = 50;
    let mut l3m = mk(1, 98.0);
    l3m.loop_ = -1;
    l3m.lcomp = 0.0;
    l3m.ucomp = 0.0;
    l3m.rcomp = 0.0;
    l3m.dcomp = 0.0;
    let mut l3c = Control::new();
    l3c.left = true;
    l3c.up = true;
    l3c.right = true;
    l3c.down = true;
    l3m.nofocus = true;
    l3m.drive(
        &mut l3c,
        &mut l3,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(l3m.lcomp), bits(7.0), "loopminus_press_lcomp_7");
    check::eq_i32(bits(l3m.ucomp), bits(7.0), "loopminus_press_ucomp_7");
    check::eq_i32(bits(l3m.rcomp), bits(7.0), "loopminus_press_rcomp_7");
    check::eq_i32(bits(l3m.dcomp), bits(7.0), "loopminus_press_dcomp_7");
    // u21. handb with `pushed` true keeps loop 0 (:928/:984)
    let mut hp = car();
    hp.y = 240;
    let mut hpm = mk(1, 98.0);
    hpm.speed = -100.0;
    hpm.pushed = true;
    hpm.wtouch = true;
    hpm.gtouch = true;
    let mut hpc = Control::new();
    hpc.handb = true;
    hpm.nofocus = true;
    hpm.drive(
        &mut hpc,
        &mut hp,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        bits(hpm.speed),
        bits(-93.0),
        "handb_pushed_negative_speed_minus93",
    );
    // u22. colide moment tie-breaks: this moment 2.0 (cn 9) beats 1.0 (cn 3)
    let mut ma = car();
    let mut mb = car();
    ma.maxr = 10;
    mb.maxr = 10;
    let mut om = mk(1, 100.0);
    om.cn = 3;
    om.speed = 1.0;
    let mut mmx = mk(1, 50.0);
    mmx.cn = 9;
    mmx.speed = 1.0;
    mmx.colide(
        &mut ma,
        &mut om,
        &mut mb,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(mmx.dominate[1], "colide_moment_2_beats_1");
    // the losing side of the same tie (:429)
    let mut mc = car();
    let mut md2 = car();
    mc.maxr = 10;
    md2.maxr = 10;
    let mut om2 = mk(1, 50.0);
    om2.cn = 9;
    om2.speed = 1.0;
    let mut mm2 = mk(1, 100.0);
    mm2.cn = 3;
    mm2.speed = 1.0;
    mm2.colide(
        &mut mc,
        &mut om2,
        &mut md2,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(!mm2.dominate[1], "colide_moment_1_loses_to_2");
}

/// `u23`: dark/bright planes driving the hsb clamps through regy/regz/regx.
fn residual_colide_hsb(_rig: &mut Rig, bandm: &mut Rig) {
    // u23. dark and bright planes drive the hsb brightness clamps
    let mut dk = car();
    dk.planes
        .set_slot(0, plane_col(&bandm.m, 3, 0, 35, false, [30, 40, 50]));
    dk.planes.set_npl(1);
    let mut dkm = Madness::new(1);
    dkm.cn = 0;
    dkm.mtouch = true;
    dkm.regy(
        0,
        150.0,
        &mut dk,
        Some(&mut bandm.m),
        Some(&mut Record::new()),
        Some(&mut bandm.xt),
    );
    check::eq_i32(
        bits(dk.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.5),
        "regy_dark_plane_hsb2_0_5",
    );
    let mut br = car();
    br.planes
        .set_slot(0, plane_col(&bandm.m, 3, 0, 55, false, [240, 250, 255]));
    br.planes.set_npl(1);
    let mut brm = Madness::new(1);
    brm.cn = 0;
    brm.mtouch = true;
    brm.regy(
        0,
        150.0,
        &mut br,
        Some(&mut bandm.m),
        Some(&mut Record::new()),
        Some(&mut bandm.xt),
    );
    check::eq_i32(
        bits(br.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.8),
        "regy_bright_plane_hsb2_0_8",
    );
    // the same clamps through regz/regx
    let mut dz = car();
    dz.planes
        .set_slot(0, plane_col(&bandm.m, 3, 0, 35, false, [30, 40, 50]));
    dz.planes.set_npl(1);
    let mut dzm = Madness::new(1);
    dzm.cn = 0;
    dzm.regz(
        0,
        150.0,
        &mut dz,
        Some(&mut bandm.m),
        Some(&mut Record::new()),
        Some(&mut bandm.xt),
    );
    check::eq_i32(
        bits(dz.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.5),
        "regz_dark_plane_hsb2_0_5",
    );
    let mut dx = car();
    dx.planes
        .set_slot(0, plane_col(&bandm.m, 3, 0, 55, false, [240, 250, 255]));
    dx.planes.set_npl(1);
    let mut dxm = Madness::new(1);
    dxm.cn = 0;
    dxm.regx(
        0,
        150.0,
        &mut dx,
        Some(&mut bandm.m),
        Some(&mut Record::new()),
        Some(&mut bandm.xt),
    );
    check::eq_i32(
        bits(dx.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.8),
        "regx_bright_plane_hsb2_0_8",
    );
}

/// `u24-u30`: ground dust, the power floor, the capsized trcnt reset, the
/// cntouch arm, pmlt/nmlt the im==0 colide push and the squash gate.
fn residual_misc(rig: &mut Rig) {
    // u24. ground dust with var27==0: a skd==0 floor tile
    let mut gz = car();
    gz.y = 240;
    let mut gzt = Trackers::new();
    gzt.nt = 1;
    floor(&mut gzt, 0, 0, 240, 0, 0, 20, 100, 100, 40);
    let mut gzm = mk(1, 98.0);
    for i in 0..4 {
        gzm.scy[i] = 100.0;
    }
    gzm.nofocus = true;
    gzm.drive(
        &mut Control::new(),
        &mut gz,
        &gzt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(gz.y, 240, "ground_skd0_dust_y_240");
    // u25. power==0 keeps the :2409 floor instead of decaying
    let mut p0 = car();
    p0.y = 240;
    let mut p0m = mk(1, 0.0);
    p0m.nofocus = true;
    p0m.drive(
        &mut Control::new(),
        &mut p0,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(p0m.power), bits(0.0), "power0_stays_0");
    // u26. capsized + !dest + trcnt!=0 resets the tracker counters
    let mut cr = car();
    cr.y = 240;
    let mut crm = mk(1, 98.0);
    crm.pzy = 180;
    crm.pxy = 0;
    crm.trcnt = 5;
    crm.travxy = 10;
    crm.travzy = 10;
    crm.travxz = 10;
    crm.ftab = true;
    crm.rtab = true;
    crm.btab = true;
    crm.surfer = true;
    crm.nofocus = true;
    crm.drive(
        &mut Control::new(),
        &mut cr,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(crm.trcnt, 0, "caps_reset_trcnt_0");
    check::eq_i32(crm.travxy, 0, "caps_reset_travxy_0");
    check::ok(!crm.surfer, "caps_reset_surfer_false");
    // u27. cntouch reaches 10 -> mtouch (:1951-1952)
    let mut c1t = Trackers::new();
    c1t.nt = 1;
    floor(&mut c1t, 0, 10, 10, 0, 0, 20, 5, 5, 20);
    let mut c1 = car();
    c1.y = 0;
    c1.x = 10;
    c1.keyx[1] = 50;
    c1.keyx[2] = 50;
    c1.keyx[3] = 50;
    let mut c1m = mk(1, 98.0);
    c1m.cntouch = 9;
    c1m.nofocus = true;
    c1m.drive(
        &mut Control::new(),
        &mut c1,
        &c1t,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(c1m.cntouch, 10, "cntouch_9_to_10");
    let mut c2 = car();
    c2.y = 0;
    c2.x = 10;
    c2.keyx[1] = 50;
    c2.keyx[2] = 50;
    c2.keyx[3] = 50;
    c1m.cntouch = 10;
    c1m.mtouch = false;
    c1m.nofocus = true;
    c1m.drive(
        &mut Control::new(),
        &mut c2,
        &c1t,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(c1m.mtouch, "cntouch_10_arms_mtouch");
    // u28. xz>45 makes pmlt +1 (:1839-1850)
    let mut pmc = car();
    pmc.y = 240;
    pmc.xz = 90;
    let mut pmt = Trackers::new();
    pmt.nt = 1;
    wallz(&mut pmt, 0, 0, 240, 0, 90, 100, 100, 60, 20);
    let mut pmm = mk(1, 98.0);
    for i in 0..4 {
        pmm.scz[i] = 10.0;
    }
    pmm.nofocus = true;
    pmm.drive(
        &mut Control::new(),
        &mut pmc,
        &pmt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(pmm.pmlt, 1, "pmlt_xz90_plus1");
    // u29. a push with this.im==0 sets the other's colidim (:476-495,:518-537)
    let mut ia = car();
    let mut ib = car();
    ia.maxr = 10;
    ib.maxr = 10;
    let mut iother = mk(1, 75.0);
    iother.speed = 5.0;
    let mut ime = mk(0, 75.0);
    ime.speed = 10.0;
    ime.scx[0] = 10.0;
    ime.scz[0] = 10.0;
    ime.colide(
        &mut ia,
        &mut iother,
        &mut ib,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(ime.lastcolido, 0, "colide_im0_this_lastcolido_0");
    check::ok(iother.scx[0] > 0.0, "colide_im0_push_other_scx");
    check::ok(iother.scz[0] > 0.0, "colide_im0_push_other_scz");
    // u30. the second-operand squash gate and the glass decrement (:246-259)
    let mut sq2 = car();
    sq2.zy = 180;
    sq2.planes
        .set_slot(0, plane(&rig.m, 3, 0, 0, false, false, -100));
    sq2.planes
        .set_slot(1, plane(&rig.m, 3, 0, 0, true, false, -200));
    sq2.planes.set_npl(2);
    let mut sq2m = Madness::new(1);
    sq2m.cn = 0;
    sq2m.mtouch = true;
    sq2m.regy(
        0,
        150.0,
        &mut sq2,
        Some(&mut rig.m),
        Some(&mut Record::new()),
        Some(&mut rig.xt),
    );
    sq2m.regy(
        0,
        150.0,
        &mut sq2,
        Some(&mut rig.m),
        Some(&mut Record::new()),
        Some(&mut rig.xt),
    );
    check::ok(
        sq2.planes.slot(0).expect("slot 0").oy[0] <= -50,
        "regy_squash_second_operand_oy",
    );
    check::eq_i32(
        sq2.planes.slot(1).expect("slot 1").gr,
        -8,
        "regy_squash_glass_gr_minus8",
    );
}

/// `u31-u36`: premangled hsb clamps, the missedcp reset, nmlt, the roted fix
/// miss, the surfer latch and the skd-2 slope mtouch arm.
fn residual_hsb(rig: &mut Rig, vecmedium: &[f32]) {
    // u31. premangled hsb values exercise the regy/regz/regx band clamps
    let mut hm = vec_rig(vecmedium);
    let mut hp2 = plane(&hm.m, 3, 0, 35, false, false, 0);
    hp2.hsb[1] = 0.9;
    hp2.hsb[2] = 0.1;
    let mut hc = car();
    hc.planes.set_slot(0, hp2);
    hc.planes.set_npl(1);
    let mut hcm = Madness::new(1);
    hcm.cn = 0;
    hcm.mtouch = true;
    hcm.regy(
        0,
        150.0,
        &mut hc,
        Some(&mut hm.m),
        Some(&mut Record::new()),
        Some(&mut hm.xt),
    );
    check::eq_i32(
        bits(hc.planes.slot(0).expect("slot 0").hsb[1]),
        bits(0.1),
        "regy_band30_hsb1_0_1",
    );
    check::eq_i32(
        bits(hc.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.5),
        "regy_band30_hsb2_0_5",
    );
    let mut hp3 = plane(&hm.m, 3, 0, 55, false, false, 0);
    hp3.hsb[2] = 0.95;
    let mut hc3 = car();
    hc3.planes.set_slot(0, hp3);
    hc3.planes.set_npl(1);
    let mut hc3m = Madness::new(1);
    hc3m.cn = 0;
    hc3m.mtouch = true;
    hc3m.regy(
        0,
        150.0,
        &mut hc3,
        Some(&mut hm.m),
        Some(&mut Record::new()),
        Some(&mut hm.xt),
    );
    check::eq_i32(
        bits(hc3.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.8),
        "regy_band50_hsb2_0_8",
    );
    let mut hz = plane(&hm.m, 3, 0, 35, false, false, 0);
    hz.hsb[1] = 0.9;
    hz.hsb[2] = 0.1;
    let mut hzc = car();
    hzc.planes.set_slot(0, hz);
    hzc.planes.set_npl(1);
    let mut hzm = Madness::new(1);
    hzm.cn = 0;
    hzm.regz(
        0,
        150.0,
        &mut hzc,
        Some(&mut hm.m),
        Some(&mut Record::new()),
        Some(&mut hm.xt),
    );
    check::eq_i32(
        bits(hzc.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.5),
        "regz_band30_hsb2_0_5",
    );
    let mut hz2 = plane(&hm.m, 3, 0, 55, false, false, 0);
    hz2.hsb[2] = 0.95;
    let mut hzc2 = car();
    hzc2.planes.set_slot(0, hz2);
    hzc2.planes.set_npl(1);
    let mut hz2m = Madness::new(1);
    hz2m.cn = 0;
    hz2m.regz(
        0,
        150.0,
        &mut hzc2,
        Some(&mut hm.m),
        Some(&mut Record::new()),
        Some(&mut hm.xt),
    );
    check::eq_i32(
        bits(hzc2.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.8),
        "regz_band50_hsb2_0_8",
    );
    let mut hx = plane(&hm.m, 3, 0, 35, false, false, 0);
    hx.hsb[1] = 0.9;
    hx.hsb[2] = 0.1;
    let mut hxc = car();
    hxc.planes.set_slot(0, hx);
    hxc.planes.set_npl(1);
    let mut hxm = Madness::new(1);
    hxm.cn = 0;
    hxm.regx(
        0,
        150.0,
        &mut hxc,
        Some(&mut hm.m),
        Some(&mut Record::new()),
        Some(&mut hm.xt),
    );
    check::eq_i32(
        bits(hxc.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.5),
        "regx_band30_hsb2_0_5",
    );
    let mut hx2 = plane(&hm.m, 3, 0, 55, false, false, 0);
    hx2.hsb[2] = 0.95;
    let mut hxc2 = car();
    hxc2.planes.set_slot(0, hx2);
    hxc2.planes.set_npl(1);
    let mut hx2m = Madness::new(1);
    hx2m.cn = 0;
    hx2m.regx(
        0,
        150.0,
        &mut hxc2,
        Some(&mut hm.m),
        Some(&mut Record::new()),
        Some(&mut hm.xt),
    );
    check::eq_i32(
        bits(hxc2.planes.slot(0).expect("slot 0").hsb[2]),
        bits(0.8),
        "regx_band50_hsb2_0_8",
    );
    // u32. missedcp reset for im!=0 with an active focus (:2188-2190)
    let mut fmis = car();
    fmis.y = 240;
    fmis.x = 30000;
    let mut frc = CheckPoints::new();
    frc.n = 4;
    frc.nsp = 4;
    for i in 0..4 {
        frc.typ[i] = 1;
        frc.x[i] = i as i32 * 20000;
        frc.z[i] = 0;
        frc.y[i] = 240;
    }
    let mut fmism = mk(1, 98.0);
    fmism.focus = -1;
    fmism.missedcp = 5;
    fmism.nofocus = true;
    fmism.drive(
        &mut Control::new(),
        &mut fmis,
        &Trackers::new(),
        &mut frc,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(fmism.missedcp, 0, "missedcp_im1_minus1_cleared");
    // u33. nmlt=1 needs |xz-45| > 90: xz=-90 gives 135 (:1852-1862)
    let mut nm = car();
    nm.y = 240;
    nm.xz = -90;
    let mut nmt = Trackers::new();
    nmt.nt = 1;
    wallz(&mut nmt, 0, 0, 240, 0, -90, 100, 100, 60, 20);
    let mut nmm = mk(1, 98.0);
    for i in 0..4 {
        nmm.scz[i] = -10.0;
    }
    nmm.nofocus = true;
    nmm.drive(
        &mut Control::new(),
        &mut nm,
        &nmt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(nmm.nmlt, 1, "nmlt_xz_minus90_plus1");
    // u34. roted fix point with dist==0 sets fcnt=8 (:2241-2242)
    let mut rf = car();
    rf.y = 240;
    let mut rfc = CheckPoints::new();
    rfc.fn_ = 1;
    rfc.roted[0] = true;
    let mut rfm = mk(1, 98.0);
    rfm.nofocus = true;
    rfm.drive(
        &mut Control::new(),
        &mut rf,
        &Trackers::new(),
        &mut rfc,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(rf.fcnt, 8, "fix_roted_dist0_fcnt_8");
    // u35. surfer latch: mtouch false and a wall id supplied by Control
    let mut sv = car();
    sv.y = 50;
    let mut svm = mk(1, 98.0);
    let mut svc = Control::new();
    svc.wall = 3;
    svm.nofocus = true;
    svm.drive(
        &mut svc,
        &mut sv,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(svm.surfer, "surfer_wall_id_latches");
    // u36. a skd==2 sloped tile under all four wheels arms mtouch via var78==4
    let mut sk2 = car();
    sk2.y = 240;
    let mut sk2t = Trackers::new();
    sk2t.nt = 1;
    wallz(&mut sk2t, 0, 0, 240, 0, 45, 100, 100, 60, 20);
    sk2t.skd[0] = 2;
    let mut sk2m = mk(1, 98.0);
    sk2m.nofocus = true;
    sk2m.drive(
        &mut Control::new(),
        &mut sk2,
        &sk2t,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(sk2m.mtouch, "slope_skd2_mtouch");
}

/// `u37-u40`: the dcnt clamps, the air floors, the regy angle bands and the negative
/// colide push walks.
fn residual_tail_a(rig: &mut Rig, mad: &Tml, vecmedium: &[f32]) {
    // u37. dcnt<0 clamp in the grip loop (:1362-1366)
    let mut dcc = car();
    dcc.y = 240;
    let mut dccm = mk(1, 98.0);
    dccm.mtouch = true;
    dccm.dcnt = 1;
    dccm.speed = 0.0;
    dccm.nofocus = true;
    dccm.drive(
        &mut Control::new(),
        &mut dcc,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(dccm.dcnt, 0, "dcnt_neg_clamped_0");
    // u38. the small-ucomp/dcomp floors in the air block (:844-852,:868-876)
    let mut fl = car();
    fl.y = 50;
    let mut flm = mk(1, 98.0);
    flm.loop_ = 2;
    for i in 0..4 {
        flm.scy[i] = -200.0;
    }
    let mut flc = Control::new();
    flc.up = true;
    flc.down = true;
    flm.nofocus = true;
    flm.drive(
        &mut flc,
        &mut fl,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(bits(flm.ucomp), bits(5.5), "air_floor_ucomp_5_5");
    check::eq_i32(bits(flm.dcomp), bits(5.5), "air_floor_dcomp_5_5");
    // u39. regy angle normalisation for non-zero zy/xy (:132-162)
    let mut am2m = vec_rig(vecmedium);
    for ab in 0..2 {
        let mut ac2 = car();
        ac2.zy = if ab == 0 { 180 } else { 10 };
        ac2.xy = if ab == 0 { 10 } else { 180 };
        ac2.planes
            .set_slot(0, plane(&am2m.m, 3, 0, 0, false, false, 0));
        ac2.planes.set_npl(1);
        let mut a2m = Madness::new(1);
        a2m.cn = 0;
        a2m.mtouch = true;
        a2m.regy(
            0,
            150.0,
            &mut ac2,
            Some(&mut am2m.m),
            Some(&mut Record::new()),
            Some(&mut am2m.xt),
        );
        check::eq_i32(a2m.nbsq, 1, &format!("regy_angle_band{ab}_nbsq"));
    }
    // u40. colide push with negative impulses reaches both -300 clamps
    let mut na = car();
    let mut nb = car();
    na.maxr = 10;
    nb.maxr = 10;
    let mut no = mk(1, 70.0);
    no.speed = 4.0;
    let mut nm2 = mk(1, 70.0);
    nm2.speed = 8.0;
    for i in 0..4 {
        nm2.scx[i] = 800.0;
        nm2.scz[i] = 800.0;
        no.scx[i] = -500.0;
        no.scz[i] = -500.0;
    }
    nm2.colide(
        &mut na,
        &mut no,
        &mut nb,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(nm2.dominate[1], "colide_neg_clamp_dominates");
    // the push repeats once per wheel pair, so slot 0 accumulates:
    // 800 -> 1100 -> 1400 -> 1700 -> 2000 (var14 pinned at +300 each)
    check::eq_i32(
        bits(nm2.scx[0]),
        bits(mad.floatv("colide_push_walk")),
        "colide_neg_clamp_this_scx_2000",
    );
    // this side negative, other near zero -> var14/var17 clamp at -300
    let mut nc2 = car();
    let mut nd2 = car();
    nc2.maxr = 10;
    nd2.maxr = 10;
    let mut no2 = mk(1, 70.0);
    no2.speed = 4.0;
    let mut nm3 = mk(1, 70.0);
    nm3.speed = 8.0;
    for i in 0..4 {
        nm3.scx[i] = -400.0;
        nm3.scz[i] = -400.0;
        no2.scx[i] = -100.0;
        no2.scz[i] = -100.0;
    }
    // force domination by a bigger moment
    nm3.cn = 9;
    nm3.colide(
        &mut nc2,
        &mut no2,
        &mut nd2,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(nm3.dominate[1], "colide_neg_this_dominates");
    check::eq_i32(
        bits(no2.scx[0]),
        bits(-700.0),
        "colide_neg_other_scx_minus700",
    );
}

/// `u41-u44`: the >360 normalisation, the skd-0 dcnt clamp, the high-scy slope and
/// the wheel-pair asymmetry.
fn residual_tail_b(rig: &mut Rig, mad: &Tml) {
    // u41. pzy/pxy beyond 360 exercise the >270 normalisation loops (:739-755)
    let mut w4 = car();
    w4.y = 250;
    let mut w4m = mk(1, 98.0);
    w4m.pzy = mad.intv("capsize_over");
    w4m.pxy = mad.intv("capsize_over");
    w4m.nofocus = true;
    w4m.drive(
        &mut Control::new(),
        &mut w4,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(
        w4m.pzy,
        mad.intv("capsize_over") - mad.intv("capsize_full"),
        "caps_pzy400_normalises_40",
    );
    check::eq_i32(
        w4m.pxy,
        mad.intv("capsize_over") - mad.intv("capsize_full"),
        "caps_pxy400_normalises_40",
    );
    // u42. dcnt clamp needs var27==0: a flat skd-0 tile
    let mut dcd = car();
    dcd.y = 240;
    let mut dct = Trackers::new();
    dct.nt = 1;
    floor(&mut dct, 0, 0, 240, 0, 0, 20, 100, 100, 40);
    let mut dc2m = mk(1, 98.0);
    dc2m.mtouch = true;
    dc2m.dcnt = 1;
    dc2m.speed = 0.0;
    dc2m.nofocus = true;
    dc2m.drive(
        &mut Control::new(),
        &mut dcd,
        &dct,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(dc2m.dcnt, 0, "dcnt_skd0_clamped_0");
    // u43. high scy on a slope crosses the dust caps and the cn==7 floor
    let mut hs = car();
    hs.y = 240;
    let mut hst = Trackers::new();
    hst.nt = 1;
    wallz(&mut hst, 0, 0, 240, 0, 45, 100, 100, 60, 20);
    hst.skd[0] = 1;
    let mut hsm = mk(1, 98.0);
    hsm.cn = 7;
    for i in 0..4 {
        hsm.scy[i] = 150.0;
    }
    hsm.nofocus = true;
    hsm.drive(
        &mut Control::new(),
        &mut hs,
        &hst,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(hs.y != 0, "highscy_slope_y_moved");
    // u44. asymmetric wheel heights drive the wheel-pair angle arms
    let mut ww2 = car();
    ww2.y = 0;
    let mut ww2m = mk(1, 98.0);
    ww2m.scy[0] = 0.0;
    ww2m.scy[1] = 10.0;
    ww2m.scy[2] = 20.0;
    ww2m.scy[3] = 30.0;
    ww2m.nofocus = true;
    ww2m.drive(
        &mut Control::new(),
        &mut ww2,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(
        ww2m.pzy != 0 || ww2m.pxy != 0,
        "wheel_pair_asymmetry_attitude",
    );
}

/// `u45-u48`: the cotchinow hcaught arm, the dested skip, the flat scy contacts and
/// the mtouch wall arm.
fn residual_tail_c(rig: &mut Rig) {
    // u45. the hcaught==false arm of the cotchinow gate (:2336)
    let mut pc = car();
    pc.y = 240;
    let mut pcr = Record::new();
    let mut pcm = mk(0, 0.0);
    pcm.gtouch = true;
    pcm.trcnt = 9;
    pcm.travxy = 400;
    pcm.travzy = 400;
    pcm.travxz = 400;
    pcm.surfer = true;
    pcm.nofocus = true;
    pcm.drive(
        &mut Control::new(),
        &mut pc,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut pcr,
        &mut rig.xt,
    );
    check::ok(pcm.powerup > 60.0, "powerup_im0_above_60");
    check::ok(!pcr.hcaught, "powerup_im0_hcaught_false_below_300");
    // u46. dested!=0 with dest true skips the rewrite (:2426)
    let mut de = car();
    de.y = 240;
    let mut dec = CheckPoints::new();
    dec.dested[1] = 2;
    let mut dem = mk(1, 98.0);
    dem.dest = true;
    dem.nofocus = true;
    dem.drive(
        &mut Control::new(),
        &mut de,
        &Trackers::new(),
        &mut dec,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(dec.dested[1], 2, "dest_dested_already_set_kept");
    // u47. flat tracker contact with a pre-lifted scy crosses the caps
    let mut fh = car();
    fh.y = 0;
    let mut fht = Trackers::new();
    fht.nt = 1;
    floor(&mut fht, 0, 0, 150, 0, 0, 20, 100, 100, 100);
    let mut fhm = mk(1, 98.0);
    fhm.cn = 7;
    fhm.pxy = 90;
    fhm.pzy = 90;
    for i in 0..4 {
        fhm.scy[i] = 150.0;
    }
    fhm.nofocus = true;
    fhm.drive(
        &mut Control::new(),
        &mut fh,
        &fht,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(fh.y != 0, "flat_high_scy_contact");
    // a cn==7 bounce (0.8) with a zero angle term floors at 1.1
    let mut fb = car();
    fb.y = 0;
    let mut fbt = Trackers::new();
    fbt.nt = 1;
    floor(&mut fbt, 0, 0, 150, 0, 0, 20, 100, 100, 100);
    let mut fbm = mk(1, 98.0);
    fbm.cn = 7;
    fbm.pxy = 0;
    fbm.pzy = 0;
    for i in 0..4 {
        fbm.scy[i] = 150.0;
    }
    fbm.nofocus = true;
    fbm.drive(
        &mut Control::new(),
        &mut fb,
        &fbt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(fb.y != 0, "flat_bounce_floor_contact");
    // u48. wall contact with mtouch already true (var56) zeroes the angle term
    let mut wm2 = car();
    wm2.y = 240;
    let mut wm2t = Trackers::new();
    wm2t.nt = 1;
    wallz(&mut wm2t, 0, 0, 240, 0, 90, 100, 100, 60, 20);
    let mut wm2m = mk(1, 98.0);
    wm2m.cn = 7;
    wm2m.mtouch = true;
    wm2m.speed = 50.0;
    for i in 0..4 {
        wm2m.scz[i] = 10.0;
    }
    wm2m.nofocus = true;
    wm2m.drive(
        &mut Control::new(),
        &mut wm2,
        &wm2t,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(wm2m.skid, 2, "wall_mtouch_skid2");
}

/// `u49-u52`: the steep slopes, the xz=300 pmlt/nmlt folds, the one-sided wheel
/// asymmetry and the zy=-90 wall reset.
fn residual_tail_d(rig: &mut Rig) {
    // u49. steep slopes (|zy|,|xy| > 50) hit the 1.0 ratio floors
    let mut st = car();
    st.y = 240;
    let mut stt = Trackers::new();
    stt.nt = 1;
    wallz(&mut stt, 0, 0, 240, 0, 60, 100, 100, 60, 20);
    let mut stm = mk(1, 98.0);
    stm.nofocus = true;
    stm.drive(
        &mut Control::new(),
        &mut st,
        &stt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(st.y != 0, "steep_zy60_contact");
    let mut st2 = car();
    st2.y = 240;
    let mut st2t = Trackers::new();
    st2t.nt = 1;
    wallx(&mut st2t, 0, 0, 240, 0, 60, 100, 100, 60, 20);
    st2t.skd[0] = 2;
    let mut st2m = mk(1, 98.0);
    st2m.nofocus = true;
    st2m.drive(
        &mut Control::new(),
        &mut st2,
        &st2t,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(st2m.mtouch, "steep_xy60_skd2_mtouch");
    // u50. xz=180 makes |xz+-45| exceed 180, so both pmlt/nmlt run
    let mut n2 = car();
    n2.y = 240;
    n2.xz = 300;
    let mut n2t = Trackers::new();
    n2t.nt = 1;
    wallz(&mut n2t, 0, 0, 240, 0, 90, 100, 100, 60, 20);
    let mut n2m = mk(1, 98.0);
    for i in 0..4 {
        n2m.scz[i] = 10.0;
    }
    n2m.nofocus = true;
    n2m.drive(
        &mut Control::new(),
        &mut n2,
        &n2t,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(n2m.pmlt, -1, "xz300_pmlt_minus1");
    check::eq_i32(n2m.nmlt, 1, "xz300_nmlt_plus1");
    // u51. one-sided wheel asymmetry reaches the var83>var81 fold
    let mut aw = car();
    aw.y = 0;
    let mut awm = mk(1, 98.0);
    awm.scy[0] = 0.0;
    awm.scy[1] = 0.0;
    awm.scy[2] = 0.0;
    awm.scy[3] = 60.0;
    awm.nofocus = true;
    awm.drive(
        &mut Control::new(),
        &mut aw,
        &Trackers::new(),
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::ok(awm.pxy != 0 || awm.pzy != 0, "asym_scy_wheel_fold");
    // u52. the zy==-90 wall with mtouch at entry: pzy=180 flips the grip
    // target scz negative so the :1528 branch fires with var56 true
    let mut wz = car();
    wz.y = 240;
    let mut wzt = Trackers::new();
    wzt.nt = 1;
    wallz(&mut wzt, 0, 0, 240, 0, -90, 100, 100, 60, 20);
    let mut wzm = mk(1, 98.0);
    wzm.cn = 7;
    wzm.mtouch = true;
    wzm.speed = 50.0;
    wzm.pzy = 180;
    wzm.nofocus = true;
    wzm.drive(
        &mut Control::new(),
        &mut wz,
        &wzt,
        &mut CheckPoints::new(),
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
    check::eq_i32(wzm.skid, 0, "wall_mtouch_minuszy_skid_reset_0");
}

// ────────────────────────────────────────────────────────────── Medium block

/// `t.` Medium: the paths the renderer suite leaves untouched
/// (`TMadnessDrive.java:2059-2218`; B3's coverage-closure section).
fn medium_block(_rig: &mut Rig, vec: &[f32]) {
    // palette low clamps (:131-132,:140-141,:149-150, setgrnd/setsky)
    let mut mm = Medium::new();
    mm.setfade(-10, -10, -10);
    check::eq_i32(mm.cfade[0], 0, "medium_setfade_low_clamp");
    check::eq_i32(mm.cfade[2], 0, "medium_setfade2_low_clamp");
    mm.setgrnd(-5, -5, -5);
    check::eq_i32(mm.cgrnd[0], 0, "medium_setgrnd_low_clamp");
    mm.setsky(-7, -7, -7);
    check::eq_i32(mm.csky[0], 0, "medium_setsky_low_clamp");
    mm.setfade(300, 300, 300);
    check::eq_i32(mm.cfade[0], 255, "medium_setfade_high_clamp");
    // adjstfade growth: +500 toward origfade then clamp (:435-441)
    let mut mf = Medium::new();
    mf.origfade = 9000;
    mf.fade[0] = 5000;
    mf.adjstfade(15.0);
    check::eq_i32(mf.fade[0], 5500, "medium_adjstfade_grow_500");
    check::eq_i32(mf.fade[7], 44000, "medium_adjstfade_fadfrom_44000");
    mf.fade[0] = 8900;
    mf.adjstfade(15.0);
    check::eq_i32(mf.fade[0], 9000, "medium_adjstfade_clamps_origfade");
    let mut mf2 = Medium::new();
    mf2.adjstfade(10.0);
    check::eq_i32(mf2.fade[0], 3000, "medium_adjstfade_floor_3000");
    // d(Graphics) with the clamps and both zy signs (:154-283). The Java
    // fixture uses a 64x64 BufferedImage; the port Canvas is the established
    // raster stand-in (no pixels pinned, B3).
    let mut g = crate::canvas::Canvas::new(550, 400);
    let mut mdw = Medium::new();
    mdw.zy = 95;
    mdw.xz = 400;
    mdw.y = 10;
    mdw.d(&mut g);
    check::eq_i32(mdw.zy, 90, "medium_d_zy_clamp_90");
    check::eq_i32(mdw.xz, 40, "medium_d_xz_wrap_40");
    check::eq_i32(mdw.y, 0, "medium_d_y_clamp_0");
    check::eq_i32(mdw.ground, 250, "medium_d_ground_250");
    let mut mdw2 = Medium::new();
    mdw2.zy = -95;
    mdw2.xz = -10;
    mdw2.y = -1;
    mdw2.d(&mut g);
    check::eq_i32(mdw2.zy, -90, "medium_d_zy_clamp_minus90");
    check::eq_i32(mdw2.xz, 350, "medium_d_xz_wrap_350");
    check::eq_i32(mdw2.ground, 251, "medium_d_ground_251");
    let mut mdw3 = Medium::new();
    mdw3.flex = 2;
    mdw3.zy = 30;
    mdw3.d(&mut g);
    check::eq_i32(mdw3.zy, 30, "medium_d_flex2_zy_kept");
    // watch(): td respawn, both x signs and the trns decay (:285-326)
    let mut mwt = vec_rig(vec);
    let mut wc = car();
    wc.x = 100;
    mwt.m.flex = 1;
    mwt.m.td = true;
    mwt.m.trns = 1;
    watch(&mut mwt.m, &wc, 0);
    check::eq_i32(mwt.m.flex, 0, "medium_watch_clears_flex");
    check::ok(!mwt.m.td, "medium_watch_clears_td");
    check::eq_i32(mwt.m.x, 500, "medium_watch_x_500");
    check::eq_i32(mwt.m.y, -520, "medium_watch_y_minus520");
    check::eq_i32(mwt.m.z, 5000, "medium_watch_z_5000");
    check::eq_i32(mwt.m.trns, 1, "medium_watch_trns_stays_1");
    mwt.m.x = 0;
    mwt.m.y = 0;
    mwt.m.z = 0;
    mwt.m.trns = 5;
    watch(&mut mwt.m, &wc, 90);
    check::eq_i32(mwt.m.trns, 4, "medium_watch_trns_decays_4");
    check::eq_i32(mwt.m.xz, -156, "medium_watch_xz_minus156");
    // around(): both clamp arms and the wide-shot advance (:334-396)
    let mut mar = Medium::new();
    mar.flex = 3;
    mar.adv = -510;
    around(&mut mar, &wc, false);
    check::eq_i32(mar.flex, 0, "medium_around_clears_flex");
    check::eq_i32(mar.adv, -508, "medium_around_adv_minus508");
    check::ok(!mar.vert, "medium_around_vert_false");
    check::eq_i32(mar.trns, 5, "medium_around_trns_5");
    let mut mar2 = Medium::new();
    mar2.adv = 905;
    around(&mut mar2, &wc, false);
    check::eq_i32(mar2.adv, 907, "medium_around_adv_907");
    check::ok(mar2.vert, "medium_around_vert_true");
    let mut mar3 = Medium::new();
    around(&mut mar3, &wc, true);
    check::eq_i32(mar3.adv, 480, "medium_around_closeup_adv_480");
    check::eq_i32(mar3.vxz, 184, "medium_around_closeup_vxz_184");

    // aroundtrack(): the tracker walk, smoothing and ptr wrap (:454-504)
    let mut atcp = CheckPoints::new();
    atcp.n = 1;
    atcp.x[0] = 30000;
    atcp.z[0] = 0;
    let mut mat = Medium::new();
    for _ in 0..38 {
        aroundtrack(&mut mat, &atcp);
    }
    check::eq_i32(mat.hit, 5000, "medium_aroundtrack_hit_5000");
    check::eq_i32(mat.nrnd, 1, "medium_aroundtrack_nrnd_1");
    check::eq_i32(mat.ptr as i32, 0, "medium_aroundtrack_ptr_wrapped_0");
    check::eq_i64(mat.trx, 26349, "medium_aroundtrack_trx_26349");
    check::eq_i32(mat.zy, 9, "medium_aroundtrack_zy_9");
    // zy in (9,30) decays by 1 (:499-500)
    mat.zy = 20;
    aroundtrack(&mut mat, &atcp);
    check::eq_i32(mat.zy, 19, "medium_aroundtrack_zy_decays_19");
    // vxz > 360 wraps (:481-483); 120 calls reach 256 -> 420 -> 60
    let mut mat2 = Medium::new();
    for _ in 0..120 {
        aroundtrack(&mut mat2, &atcp);
    }
    check::eq_i32(mat2.vxz, 60, "medium_aroundtrack_vxz_wrap_60");
    // sin/cos wrap for negative angles (:99-109,:554-564)
    let msc = Medium::new();
    check::eq_i32(bits(msc.cos(-30)), bits(0.8660254), "medium_cos_neg30");
    check::eq_i32(bits(msc.sin(360)), bits(0.0), "medium_sin_360");
    // the per-channel clamps on every palette setter
    let mut mpc = Medium::new();
    mpc.setfade(300, -10, 300);
    check::eq_i32(mpc.cfade[1], 0, "medium_setfade_ch1_low");
    check::eq_i32(mpc.cfade[2], 255, "medium_setfade_ch2_high");
    mpc.setgrnd(-5, 300, -5);
    check::eq_i32(mpc.cgrnd[1], 255, "medium_setgrnd_ch1_high");
    check::eq_i32(mpc.cgrnd[2], 0, "medium_setgrnd_ch2_low");
    mpc.setsky(300, -7, -7);
    check::eq_i32(mpc.csky[0], 255, "medium_setsky_ch0_high");
    check::eq_i32(mpc.csky[1], 0, "medium_setsky_ch1_low");
    mpc.setgrnd(300, 300, 300);
    check::eq_i32(mpc.cgrnd[0], 255, "medium_setgrnd_ch0_high");
    check::eq_i32(mpc.cgrnd[2], 255, "medium_setgrnd_ch2_high");
    mpc.setsky(-7, 300, 300);
    check::eq_i32(mpc.csky[1], 255, "medium_setsky_ch1_high");
    check::eq_i32(mpc.csky[2], 255, "medium_setsky_ch2_high");
    // d() with zy==0 skips the rotated projection (:188,:241)
    let mut md0 = Medium::new();
    md0.d(&mut g);
    check::eq_i32(md0.ground, 250, "medium_d_flat_ground_250");
    // watch(): the two sign arms and the >6000 td respawn
    let mut mws = vec_rig(vec);
    let mut wfar = car();
    wfar.x = 1000;
    wfar.y = 100;
    mws.m.x = 0;
    mws.m.y = 0;
    watch(&mut mws.m, &wfar, 0);
    check::eq_i32(mws.m.xz, -270, "medium_watch_positive_x_xz");
    let mut mws2 = vec_rig(vec);
    let mut wfar2 = car();
    wfar2.x = 7000;
    mws2.m.x = 0;
    mws2.m.y = 0;
    watch(&mut mws2.m, &wfar2, 0);
    check::ok(mws2.m.td, "medium_watch_far_sets_td");
}

// ──────────────────────────────────────────────────────────── fixture helpers

/// A drive-ready `ContO` (`TMadnessDrive.java:2223-2245`): the empty stream
/// leaves npl==0, all zeros.
fn car() -> Conto {
    Conto::blank()
}

/// `mk(Medium, Record, im, power)` (`TMadnessDrive.java:2247-2252`).
fn mk(im: usize, power: f32) -> Madness {
    let mut md = Madness::new(im);
    md.cn = 0;
    md.power = power;
    md
}

/// Second tick on an existing Madness, keeping its accumulated state
/// (`TMadnessDrive.java:2264-2268`): nofocus armed, fresh Trackers and
/// CheckPoints, im already on `md`. The Java class's OTHER `tick` overload
/// (`:2254-2262`, a fresh `Madness` per call) is never invoked by a pin, so
/// it has no Rust twin (implementing it would only add dead code).
fn tick_on(
    rig: &mut Rig,
    c: &mut Conto,
    ctrl: &mut Control,
    cp: &mut CheckPoints,
    md: &mut Madness,
) {
    md.nofocus = true;
    md.drive(
        ctrl,
        c,
        &Trackers::new(),
        cp,
        &mut rig.m,
        &mut Record::new(),
        &mut rig.xt,
    );
}

/// One flat (xy=zy=0) floor tile (`TMadnessDrive.java:2283-2296`).
fn floor(
    tr: &mut Trackers,
    i: usize,
    x: i32,
    y: i32,
    z: i32,
    skd: i32,
    dam: i32,
    radx: i32,
    radz: i32,
    rady: i32,
) {
    tr.x[i] = x;
    tr.y[i] = y;
    tr.z[i] = z;
    tr.xy[i] = 0;
    tr.zy[i] = 0;
    tr.skd[i] = skd;
    tr.dam[i] = dam;
    tr.radx[i] = radx;
    tr.radz[i] = radz;
    tr.rady[i] = rady;
}

/// A wall perpendicular to x (xy=+-90) (`TMadnessDrive.java:2298-2303`).
#[allow(clippy::too_many_arguments)]
fn wallx(
    tr: &mut Trackers,
    i: usize,
    x: i32,
    y: i32,
    z: i32,
    xy: i32,
    radx: i32,
    radz: i32,
    rady: i32,
    dam: i32,
) {
    floor(tr, i, x, y, z, 0, dam, radx, radz, rady);
    tr.xy[i] = xy;
}

/// A wall perpendicular to z (zy=+-90) (`TMadnessDrive.java:2305-2310`).
#[allow(clippy::too_many_arguments)]
fn wallz(
    tr: &mut Trackers,
    i: usize,
    x: i32,
    y: i32,
    z: i32,
    zy: i32,
    radx: i32,
    radz: i32,
    rady: i32,
    dam: i32,
) {
    floor(tr, i, x, y, z, 0, dam, radx, radz, rady);
    tr.zy[i] = zy;
}

/// A 3-vertex plane; the Java helper passes its `wz` argument as the Plane
/// constructor's `wy` slot and leaves the plane's `wz` at 0, so the
/// random-scatter path always runs (`TMadnessDrive.java:2312-2327`).
fn plane(m: &Medium, n: usize, wz: i32, bfase: i32, glass: bool, nocol: bool, oy0: i32) -> Plane {
    let mut ox = vec![0i32; n];
    let mut oz = vec![0i32; n];
    let mut oy = vec![0i32; n];
    for i in 0..n {
        ox[i] = i as i32 * 10;
        oz[i] = i as i32 * 5;
        oy[i] = oy0 + i as i32;
    }
    let mut p = Plane::new(
        m,
        &ox,
        &oz,
        &oy,
        n,
        [100, 120, 140],
        glass,
        0,
        0,
        wz,
        0,
        7,
        bfase,
        false,
    );
    p.nocol = nocol;
    p
}

/// A 3-vertex plane with an explicit colour triple
/// (`TMadnessDrive.java:2329-2341`).
fn plane_col(m: &Medium, n: usize, wz: i32, bfase: i32, glass: bool, col: [i32; 3]) -> Plane {
    let mut ox = vec![0i32; n];
    let mut oz = vec![0i32; n];
    let mut oy = vec![0i32; n];
    for i in 0..n {
        ox[i] = i as i32 * 10;
        oz[i] = i as i32 * 5;
        oy[i] = i as i32;
    }
    Plane::new(
        m, &ox, &oz, &oy, n, col, glass, 0, 0, wz, 0, 7, bfase, false,
    )
}
