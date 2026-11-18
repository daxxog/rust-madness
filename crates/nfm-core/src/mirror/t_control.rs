//! `TControl.java` — Control, the AI brain behind every non-player car: the
//! declared field set and its 62-field shape, `falseo`, `reset`'s
//! fix-marker → facepoint walk, `py`, and the whole `preform` state machine
//! (the `statusque` strategy refresh, the plan ladders, the attack scan, the
//! touch-branch path select, the steering/wall machine and the airborne
//! trick phases). Group "Control" (`Suite.java` position 15 — 668 runtime
//! pins from 637 static `Check` sites; the campaign's second-largest class).
//! Pin source: `reference/vineflower/Control.java:2-1177` via
//! `tests/java/TControl.java`; narrative: notes/JAVA-BEHAVIOR/B7_control.md.
//!
//! Consumes BOTH fixture tables the Java class does:
//! `constants.toml [control]` (the class-level `K`, `TControl.java:80-88`)
//! and `determinism.toml [recipes].control_preset` (the `Preset` walk
//! recipe, `TControl.java:3000-3028`).
//!
//! Port-shape mapping (documented at each pin):
//! - Java's `Control.m` slot is the port's per-call `&mut Medium` argument
//!   (`Control::preform`), so the ctor's identity and null pins are
//!   structural (`ok(true)`, the documented `t_plane`/`t_record` precedent);
//!   the field-count pin adds the missing slot back to the destructured
//!   stored count;
//! - Java's `CheckPoints`/`ContO`/`Trackers` null-argument pins
//!   (`TControl.java:2834-2865`) are unrepresentable through `&` arguments —
//!   the type system rejects the fixture, so those pins are structural too;
//! - the Java `Preset extends Medium` fixture is [`Preset`], a thin wrapper
//!   that presets the port's `Medium` walk and counts draws;
//! - `GameSparker`'s key/mouse handlers (pins 82-87) are the port's
//!   `Shell::key_down`/`key_up`/`mouse_down` over `Game.ctl[0]`; the event
//!   object collapses to its Java `Event` code (and the click pixel). The
//!   two `boolean` returns are AWT event-consumed contracts with no port
//!   counterpart (structural pins), and `Shell`'s `exwist` starts `false`
//!   because the port constructs the shell in the post-`run()`
//!   `GameSparker.java:644` state — the pre-run `true` fixture is re-armed
//!   explicitly at pin 84.

use std::panic::AssertUnwindSafe;

use crate::checkpoints::CheckPoints;
use crate::conto::{Base, Conto};
use crate::control::Control;
use crate::madness::Madness;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::shell::{Game, Shell};
use crate::trackers::Trackers;

pub(crate) fn run() {
    check::group("Control");
    // TControl.java:80,87 — the class-level constants load and its shape pin.
    let k = Tml::load("constants.toml").table("control");
    k.require_keys(&[
        "saftey_default",
        "acuracy_lead",
        "statusque_trick_add",
        "trfix_trick_to",
        "fpnt_near_units",
        "trickfase_air",
        "trickfase_ground",
    ]);
    // TControl.java:3000-3001 — the determinism recipe (single source of
    // truth for the preset walk AND the expected values).
    let recipe = Tml::load("determinism.toml")
        .table("recipes")
        .table("control_preset");

    preset_recipe(&recipe);
    defaults(&k);
    py_pins();
    flag_pins(&k);
    reset_pins(&k);
    gate_pins(&k, &recipe);
    plan_pins(&k, &recipe);
    turn_pins(&recipe);
    trfix_pins(&recipe);
    fix_pins(&k, &recipe);
    bulistc_pins(&recipe);
    attack_pins(&recipe);
    // TControl.java's run() calls pathPins BEFORE actPins — the class's
    // definition order differs from its call order.
    path_pins(&recipe);
    act_pins(&recipe);
    steer_pins(&recipe);
    wall_pins(&recipe);
    trick_pins(&k, &recipe);
    trick_extra_pins(&k, &recipe);
    corner_pins(&k, &recipe);
    error_pins(&recipe);
    input_pins();
}

/// The Java fixture's `Preset extends Medium` (`TControl.java:3004-3028`):
/// the port's `Medium` with the recipe's walk state preset. The Java
/// subclass's `draws` counter is derived from the walk state (below).
/// `Preset(int start)` sets all three `rand` entries.
struct Preset {
    m: Medium,
}

impl Preset {
    fn new(r: &Tml) -> Preset {
        Preset::with_start(r, r.int_arr("rand")[0])
    }

    fn with_start(r: &Tml, start: i32) -> Preset {
        let mut m = Medium::new();
        m.cntrn = r.intv("cntrn");
        m.rand = [start; 3];
        m.diup = [r.boolv("diup"); 3];
        m.trn = r.intv("trn") as usize;
        Preset { m }
    }

    /// One walk draw (the Java `Preset.random` override's `super` call; the
    /// draw count is read back through [`Preset::draws`]).
    fn random(&mut self) -> f32 {
        self.m.random()
    }

    /// `PresetMedium.draws` — one walk step per `random()`, and every pinned
    /// stream stays INSIDE the recipe's non-zero `cntrn` window (no refill),
    /// so the counter is the walk state's delta from the recipe value (the
    /// `t_plane` precedent, here with the recipe's 100-call window; the
    /// longest fixture is pin 18's two passes == 20 draws).
    fn draws(&self, r: &Tml) -> i32 {
        r.intv("cntrn") - self.m.cntrn
    }
}

/// `TControl.mad` (`:3032-3036`): `new Madness(m, new Record(m), null, im)`
/// with `cn == 0`. The port's `Madness::new(im)` takes no collaborators
/// (`m`/`rpd`/`xt` are per-call arguments), so only `cn` is re-stated.
/// `preform` never dereferences `xt`.
fn mad(im: usize) -> Madness {
    let mut md = Madness::new(im);
    md.cn = 0;
    md
}

/// `TControl.plan` (`:3040-3045`): opens the plan gate the way the real loop
/// does (`stcnt` past `statusque`), with `mtouch` armed.
fn plan(
    c: &mut Control,
    md: &mut Madness,
    cp: &CheckPoints,
    t: &Trackers,
    car: &Conto,
    m: &mut Medium,
) {
    md.mtouch = true;
    c.stcnt = 1;
    c.statusque = 0;
    c.preform(md, car, cp, t, m);
}

/// `TControl.touch` (`:3048-3051`): one touch-branch pass with the plan gate
/// closed (`stcnt == statusque == 0`).
fn touch(c: &mut Control, md: &mut Madness, cp: &CheckPoints, car: &Conto, m: &mut Medium) {
    md.mtouch = true;
    c.preform(md, car, cp, &Trackers::new(), m);
}

/// `TControl.car` (`:89`, `:3055-3060`): the empty-byte `ContO` seam. The
/// Java parser leaves every field at its initializer and allocates
/// `Plane[120]`; the port's test-support `Base::default()` placement carries
/// `disline`/`grounded` zero, so the two declared non-zero `ContO`
/// initializers (`ContO.java:21-25`) are re-stated. Control reads only
/// `x/y/z/xz` and writes no `ContO` field, so one instance is repositioned
/// per fixture.
fn car_seam() -> Conto {
    let m = Medium::new();
    let mut c = Conto::place(&Base::default(), &m, &mut Trackers::new(), 0, 0, 0, 0);
    c.disline = 7;
    c.grounded = 1.0;
    c.planes.clear(); // npl = 0
    c.x = 0;
    c.y = 0;
    c.z = 0;
    c.xz = 0;
    c
}

fn car_at(car: &mut Conto, x: i32, y: i32, z: i32, xz: i32) {
    car.x = x;
    car.y = y;
    car.z = z;
    car.xz = xz;
}

/// `TControl.cp` (`:3064-3071`): one positive track point so the touch
/// branch's path select terminates (`Control.java:597-615` needs `typ>0`
/// and `n>0`).
fn cp() -> CheckPoints {
    let mut p = CheckPoints::new();
    p.n = 1;
    p.typ[0] = 1;
    p.x[0] = 0;
    p.z[0] = 0;
    p
}

/// `TControl.cpBul` (`:3075-3082`): five positive track points for the
/// bulistc branch, which does `var23 = point - 2` (`:540`).
fn cp_bul() -> CheckPoints {
    let mut p = CheckPoints::new();
    p.n = 5;
    for i in 0..5 {
        p.typ[i] = 1;
    }
    p
}

/// `TControl.snapshot` (`:3100-3117`): every Control field except `m`, so
/// "nothing else changed" is one compare. The exact spelling is private to
/// this module — every use compares snapshots with snapshots.
fn snapshot(c: &Control) -> String {
    let flags = format!(
        "{}{}{}{}{}{}{}{}{}",
        c.left, c.right, c.up, c.down, c.handb, c.enter, c.arrace, c.mutem, c.mutes
    );
    let tail = [
        c.pan.to_string(),
        c.attack.to_string(),
        c.acr.to_string(),
        c.afta.to_string(),
        format!("{:?}", c.fpnt),
        c.trfix.to_string(),
        c.forget.to_string(),
        c.bulistc.to_string(),
        c.acuracy.to_string(),
        c.upwait.to_string(),
        c.agressed.to_string(),
        bits(c.skiplev).to_string(),
        c.clrnce.to_string(),
        c.rampp.to_string(),
        c.turntyp.to_string(),
        c.saftey.to_string(),
        c.perfection.to_string(),
        bits(c.mustland).to_string(),
        c.usebounce.to_string(),
        bits(c.trickprf).to_string(),
        c.zyinv.to_string(),
        c.lastl.to_string(),
        c.wlastl.to_string(),
        c.hold.to_string(),
        c.wall.to_string(),
        c.lwall.to_string(),
        c.stcnt.to_string(),
        c.statusque.to_string(),
        c.turncnt.to_string(),
        c.randtcnt.to_string(),
        c.upcnt.to_string(),
        c.trickfase.to_string(),
        c.swat.to_string(),
        c.udcomp.to_string(),
        c.lrcomp.to_string(),
        c.udbare.to_string(),
        c.lrbare.to_string(),
        c.onceu.to_string(),
        c.onced.to_string(),
        c.oncel.to_string(),
        c.oncer.to_string(),
        c.lrdirect.to_string(),
        c.uddirect.to_string(),
        c.lrstart.to_string(),
        c.udstart.to_string(),
        c.oxy.to_string(),
        c.ozy.to_string(),
        c.flycnt.to_string(),
        c.lrswt.to_string(),
        c.udswt.to_string(),
        c.actwait.to_string(),
        c.cntrn.to_string(),
    ];
    format!("{flags}|{}", tail.join(","))
}

/// Counts the destructured field bindings (the `t_small`/`t_plane`
/// reflection substitute): the destructure stops compiling if the port's
/// field set drifts from the list.
macro_rules! control_field_count {
    ($($f:ident),* $(,)?) => {
        [$( { let _ = &$f; 0u8 } ),*].len()
    };
}

// ------------------------------------------------------------- determinism

/// `TControl.presetRecipe` (`:119-136`): the B7 seam recipe pinned so every
/// expectation below has a documented value stream — 12 draws, the state
/// after them, the `rand={2,2,2}` alternative and the two shape pins.
fn preset_recipe(r: &Tml) {
    let want = r.float_arr("walk");
    let mut p = Preset::new(r);
    for (i, want_i) in want.iter().enumerate() {
        check::eq_i32(bits(p.random()), bits(*want_i), &format!("preset_value{i}"));
    }
    check::eq_i32(p.draws(r), want.len() as i32, "preset_twelve_draws");
    check::eq_i32(p.m.cntrn, r.intv("walk_cntrn"), "preset_cntrn_100_minus_12");
    check::eq_i32(
        p.m.trn as i32,
        r.intv("walk_trn"),
        "preset_trn_wraps_back_to_0",
    );
    check::eq_str(
        format!("{:?}", p.m.rand),
        format!("{:?}", r.int_arr("walk_rand")),
        "preset_rand_after_12",
    );
    // the trickphase -1 arm starts at 0.1 <= trickprf/3
    let mut lo = Preset::with_start(r, r.intv("alt_rand"));
    check::eq_i32(
        bits(lo.random()),
        bits(r.floatv("alt_first")),
        "preset_rand2_first_0_1",
    );
    // shape pins: the rand recipe is a triple and the walk is one period
    check::eq_i32(
        r.int_arr("rand").len() as i32,
        3,
        "preset_rand_triple_len_3",
    );
    check::eq_i32(want.len() as i32, 12, "preset_walk_len_12");
}

// ------------------------------------------------------ defaults and ctor

/// `TControl.defaults` (`:142-216`, B7(1,2)): the 62 declared fields and
/// what a fresh Control leaves at its initializer. The ctor assigns ONLY `m`.
fn defaults(k: &Tml) {
    let c = Control::new();

    #[allow(unused_variables)]
    {
        let Control {
            left,
            right,
            up,
            down,
            handb,
            enter,
            arrace,
            mutem,
            mutes,
            pan,
            attack,
            acr,
            afta,
            fpnt,
            trfix,
            forget,
            bulistc,
            acuracy,
            upwait,
            agressed,
            skiplev,
            clrnce,
            rampp,
            turntyp,
            saftey,
            perfection,
            mustland,
            usebounce,
            trickprf,
            zyinv,
            lastl,
            wlastl,
            hold,
            wall,
            lwall,
            stcnt,
            statusque,
            turncnt,
            randtcnt,
            upcnt,
            trickfase,
            swat,
            udcomp,
            lrcomp,
            udbare,
            lrbare,
            onceu,
            onced,
            oncel,
            oncer,
            lrdirect,
            uddirect,
            lrstart,
            udstart,
            oxy,
            ozy,
            flycnt,
            lrswt,
            udswt,
            actwait,
            cntrn,
        } = &c;
        let stored = control_field_count!(
            left, right, up, down, handb, enter, arrace, mutem, mutes, pan, attack, acr, afta,
            fpnt, trfix, forget, bulistc, acuracy, upwait, agressed, skiplev, clrnce, rampp,
            turntyp, saftey, perfection, mustland, usebounce, trickprf, zyinv, lastl, wlastl, hold,
            wall, lwall, stcnt, statusque, turncnt, randtcnt, upcnt, trickfase, swat, udcomp,
            lrcomp, udbare, lrbare, onceu, onced, oncel, oncer, lrdirect, uddirect, lrstart,
            udstart, oxy, ozy, flycnt, lrswt, udswt, actwait, cntrn,
        );
        // Java's 62 = these 61 stored fields + `m`, which the port takes per
        // call; the exhaustive destructure makes the field set a compile pin.
        check::eq_i32((stored + 1) as i32, 62, "field_count_62");
    }

    // `c.m == null`: the port's Control holds no Medium at all — the
    // reference is `preform`'s last argument (structural).
    check::ok(true, "default_m_null");
    check::ok(!c.left, "default_left_false");
    check::ok(!c.right, "default_right_false");
    check::ok(!c.up, "default_up_false");
    check::ok(!c.down, "default_down_false");
    check::ok(!c.handb, "default_handb_false");
    check::ok(!c.enter, "default_enter_false");
    check::ok(!c.arrace, "default_arrace_false");
    check::ok(!c.mutem, "default_mutem_false");
    check::ok(!c.mutes, "default_mutes_false");
    check::eq_i32(c.pan, 0, "default_pan_0");
    check::eq_i32(c.attack, 0, "default_attack_0");
    check::eq_i32(c.acr, 0, "default_acr_0");
    check::ok(!c.afta, "default_afta_false");
    check::eq_str(
        format!("{:?}", c.fpnt),
        "[0, 0, 0, 0, 0]",
        "default_fpnt_zero",
    );
    check::eq_i32(c.fpnt.len() as i32, 5, "default_fpnt_len5");
    check::eq_i32(c.trfix, 0, "default_trfix_0");
    check::ok(!c.forget, "default_forget_false");
    check::ok(!c.bulistc, "default_bulistc_false");
    check::eq_i32(c.acuracy, 0, "default_acuracy_0");
    check::eq_i32(c.upwait, 0, "default_upwait_0");
    check::ok(!c.agressed, "default_agressed_false");
    check::eq_i32(
        bits(c.skiplev),
        bits(k.floatv("skiplev_default")),
        "default_skiplev_1",
    );
    check::eq_i32(c.clrnce, k.intv("clrnce_default"), "default_clrnce_5");
    check::eq_i32(c.rampp, 0, "default_rampp_0");
    check::eq_i32(c.turntyp, 0, "default_turntyp_0");
    check::eq_i32(c.saftey, k.intv("saftey_default"), "default_saftey_30");
    check::ok(!c.perfection, "default_perfection_false");
    check::eq_i32(
        bits(c.mustland),
        bits(k.floatv("mustland_default")),
        "default_mustland_0_5",
    );
    check::ok(!c.usebounce, "default_usebounce_false");
    check::eq_i32(
        bits(c.trickprf),
        bits(k.floatv("trickprf_default")),
        "default_trickprf_0_5",
    );
    check::ok(!c.zyinv, "default_zyinv_false");
    check::ok(!c.lastl, "default_lastl_false");
    check::ok(!c.wlastl, "default_wlastl_false");
    check::eq_i32(c.hold, 0, "default_hold_0");
    check::eq_i32(c.wall, -1, "default_wall_minus1");
    check::eq_i32(c.lwall, -1, "default_lwall_minus1");
    check::eq_i32(c.stcnt, 0, "default_stcnt_0");
    check::eq_i32(c.statusque, 0, "default_statusque_0");
    check::eq_i32(c.turncnt, 0, "default_turncnt_0");
    check::eq_i32(c.randtcnt, 0, "default_randtcnt_0");
    check::eq_i32(c.upcnt, 0, "default_upcnt_0");
    check::eq_i32(c.trickfase, 0, "default_trickfase_0");
    check::eq_i32(c.swat, 0, "default_swat_0");
    check::ok(!c.udcomp, "default_udcomp_false");
    check::ok(!c.lrcomp, "default_lrcomp_false");
    check::ok(!c.udbare, "default_udbare_false");
    check::ok(!c.lrbare, "default_lrbare_false");
    check::ok(!c.onceu, "default_onceu_false");
    check::ok(!c.onced, "default_onced_false");
    check::ok(!c.oncel, "default_oncel_false");
    check::ok(!c.oncer, "default_oncer_false");
    check::eq_i32(c.lrdirect, 0, "default_lrdirect_0");
    check::eq_i32(c.uddirect, 0, "default_uddirect_0");
    check::eq_i32(c.lrstart, 0, "default_lrstart_0");
    check::eq_i32(c.udstart, 0, "default_udstart_0");
    check::eq_i32(c.oxy, 0, "default_oxy_0");
    check::eq_i32(c.ozy, 0, "default_ozy_0");
    check::eq_i32(c.flycnt, 0, "default_flycnt_0");
    check::ok(!c.lrswt, "default_lrswt_false");
    check::ok(!c.udswt, "default_udswt_false");
    check::eq_i32(c.actwait, 0, "default_actwait_0");
    check::eq_i32(c.cntrn, 0, "default_cntrn_0");

    // B7(2): the reference is stored verbatim and nothing else changes.
    // `c2.m == mm` is the compile-time call-site identity in the port; the
    // fixture below keeps the observable half — a second Control is
    // field-identical except for its own (per-instance) fpnt table.
    let mut c2 = Control::new();
    check::ok(true, "ctor_m_reference_identity");
    check::eq_str(
        snapshot(&c2),
        snapshot(&c),
        "ctor_other_fields_identical_to_null_ctor",
    );
    c2.fpnt[0] = 1;
    check::ok(
        c.fpnt[0] == 0 && c2.fpnt[0] == 1,
        "ctor_fpnt_is_per_instance",
    );
}

/// `TControl.pyPins` (`:219-225`, B7(3)): `py` reads no field and wraps as
/// int (`Control.java:1175-1177`).
fn py_pins() {
    let c = Control::new();
    check::eq_i32(c.py(3, 1, 5, 2), 13, "py_3_1_5_2_13");
    check::eq_i32(c.py(-1, 0, -1, 0), 2, "py_neg1_0_neg1_0_2");
    check::eq_i32(
        c.py(0, 100000, 0, 0),
        1410065408,
        "py_1e10_int_wrap_1410065408",
    );
    check::eq_i32(c.py(1, 1, 1, 1), 0, "py_same_point_0");
}

// ---------------------------------------------------------- flag clearing

/// `TControl.flagPins` (`:231-300`, B7(4-6)): `falseo` clears exactly the
/// nine UI flags (`:1163-1172`), `preform` clears the five drive flags
/// before any gate (`:66-70`), and `dest == true` returns immediately
/// (`:71-72`).
fn flag_pins(k: &Tml) {
    let mut car = car_seam();

    let mut c = Control::new();
    c.left = true;
    c.right = true;
    c.up = true;
    c.down = true;
    c.handb = true;
    c.enter = true;
    c.arrace = true;
    c.mutem = true;
    c.mutes = true;
    c.pan = 1;
    c.attack = 2;
    c.acr = 3;
    c.trfix = 2;
    c.wall = 7;
    c.zyinv = true;
    c.hold = 3;
    c.upcnt = 4;
    c.acuracy = 5;
    c.trickfase = k.intv("trickfase_ground");
    let before = snapshot(&c);
    c.falseo();
    check::ok(!c.left, "falseo_left_false");
    check::ok(!c.right, "falseo_right_false");
    check::ok(!c.up, "falseo_up_false");
    check::ok(!c.down, "falseo_down_false");
    check::ok(!c.handb, "falseo_handb_false");
    check::ok(!c.enter, "falseo_enter_false");
    check::ok(!c.arrace, "falseo_arrace_false");
    check::ok(!c.mutem, "falseo_mutem_false");
    check::ok(!c.mutes, "falseo_mutes_false");
    check::ok(c.zyinv, "falseo_keeps_zyinv");
    check::eq_i32(c.wall, 7, "falseo_keeps_wall_7");
    check::eq_i32(c.attack, 2, "falseo_keeps_attack_2");
    check::eq_i32(c.trfix, 2, "falseo_keeps_trfix_2");
    check::eq_i32(c.hold, 3, "falseo_keeps_hold_3");
    check::eq_i32(c.upcnt, 4, "falseo_keeps_upcnt_4");
    check::eq_i32(c.acuracy, 5, "falseo_keeps_acuracy_5");
    check::eq_i32(
        c.trickfase,
        k.intv("trickfase_ground"),
        "falseo_keeps_trickfase_2",
    );
    check::eq_i32(c.pan + c.acr, 4, "falseo_keeps_pan_acr");
    // only the nine flag characters differ from the pre-call snapshot
    check::ok(before != snapshot(&c), "falseo_changed_something");

    // B7(5) / :66-70 — the five drive flags are cleared before any gate,
    // even on a pass that immediately falls through to the trick block.
    let mut c5 = Control::new();
    c5.left = true;
    c5.right = true;
    c5.up = true;
    c5.down = true;
    c5.handb = true;
    c5.enter = true;
    c5.arrace = true;
    let mut md5 = mad(0);
    md5.mtouch = false;
    md5.wtouch = false;
    car_at(&mut car, 0, 0, 0, 0);
    c5.preform(&mut md5, &car, &cp(), &Trackers::new(), &mut Medium::new());
    check::ok(
        !c5.left && !c5.right && !c5.up && !c5.down && !c5.handb,
        "preform_entry_clears_five_flags",
    );
    check::ok(c5.enter, "preform_entry_keeps_enter");
    check::ok(c5.arrace, "preform_entry_keeps_arrace");

    // B7(6) / :71-72 — dest == true returns immediately; m is never touched
    let mut c6 = Control::new();
    c6.left = true;
    c6.right = true;
    c6.up = true;
    c6.down = true;
    c6.handb = true;
    c6.enter = true;
    c6.arrace = true;
    c6.attack = 40;
    c6.wall = 7;
    let mut md6 = mad(0);
    md6.dest = true;
    c6.preform(&mut md6, &car, &cp(), &Trackers::new(), &mut Medium::new());
    check::ok(
        !c6.left && !c6.right && !c6.up && !c6.down && !c6.handb,
        "dest_true_five_flags_false",
    );
    check::ok(c6.enter, "dest_true_keeps_enter");
    check::ok(c6.arrace, "dest_true_keeps_arrace");
    check::eq_i32(c6.attack, 40, "dest_true_keeps_attack_40");
    check::eq_i32(c6.wall, 7, "dest_true_keeps_wall_7");
}

// ------------------------------------------------------------- reset/fpnt

/// `TControl.resetPins` (`:306-466`, B7(7-13)): the clear set, the untouched
/// set and the nearest-fixpoint walk (`Control.java:1120-1157`).
fn reset_pins(k: &Tml) {
    // B7(7): reset(new CheckPoints()) clears exactly its 14 fields...
    let mut c = Control::new();
    c.pan = 9;
    c.attack = 9;
    c.acr = 9;
    c.afta = true;
    c.trfix = 2;
    c.acuracy = 9;
    c.upwait = 9;
    c.forget = true;
    c.bulistc = true;
    c.left = true;
    c.right = true;
    c.up = true;
    c.down = true;
    c.handb = true;
    c.arrace = true;
    c.mutem = true;
    c.mutes = true;
    c.enter = true;
    c.zyinv = true;
    c.upcnt = 9;
    c.trickfase = k.intv("trickfase_ground");
    c.wall = 3;
    c.hold = 4;
    c.skiplev = 0.3;
    c.clrnce = 9;
    c.rampp = 2;
    c.turntyp = 3;
    c.saftey = 4;
    c.perfection = true;
    c.mustland = 0.2;
    c.usebounce = true;
    c.trickprf = 0.1;
    c.agressed = true;
    c.stcnt = 4;
    c.statusque = 5;
    c.turncnt = 6;
    c.randtcnt = 7;
    c.swat = 8;
    c.lrdirect = 1;
    c.uddirect = -1;
    c.lrstart = 2;
    c.udstart = 3;
    c.oxy = 4;
    c.ozy = 5;
    c.flycnt = 6;
    c.lrswt = true;
    c.udswt = true;
    c.actwait = 8;
    c.cntrn = 9;
    c.lastl = true;
    c.wlastl = true;
    c.lwall = 2;
    c.reset(&CheckPoints::new());
    check::eq_i32(c.pan, 0, "reset_pan_0");
    check::eq_i32(c.attack, 0, "reset_attack_0");
    check::eq_i32(c.acr, 0, "reset_acr_0");
    check::ok(!c.afta, "reset_afta_false");
    check::eq_i32(c.trfix, 0, "reset_trfix_0");
    check::eq_i32(c.acuracy, 0, "reset_acuracy_0");
    check::eq_i32(c.upwait, 0, "reset_upwait_0");
    check::ok(!c.forget, "reset_forget_false");
    check::ok(!c.bulistc, "reset_bulistc_false");
    check::ok(
        !c.left && !c.right && !c.up && !c.down && !c.handb,
        "reset_five_drive_flags_false",
    );
    check::ok(
        !c.arrace && !c.mutem && !c.mutes,
        "reset_three_ui_flags_false",
    );
    check::ok(c.enter, "reset_keeps_enter");
    check::ok(c.zyinv, "reset_keeps_zyinv");
    check::eq_i32(c.upcnt, 9, "reset_keeps_upcnt");
    check::eq_i32(
        c.trickfase,
        k.intv("trickfase_ground"),
        "reset_keeps_trickfase",
    );
    check::eq_i32(c.wall, 3, "reset_keeps_wall");
    check::eq_i32(c.hold, 4, "reset_keeps_hold");
    check::eq_i32(bits(c.skiplev), bits(0.3), "reset_keeps_skiplev");
    check::eq_i32(c.clrnce, 9, "reset_keeps_clrnce");
    check::eq_i32(c.rampp, 2, "reset_keeps_rampp");
    check::eq_i32(c.turntyp, 3, "reset_keeps_turntyp");
    check::eq_i32(c.saftey, 4, "reset_keeps_saftey");
    check::ok(c.perfection, "reset_keeps_perfection");
    check::eq_i32(bits(c.mustland), bits(0.2), "reset_keeps_mustland");
    check::ok(c.usebounce, "reset_keeps_usebounce");
    check::eq_i32(bits(c.trickprf), bits(0.1), "reset_keeps_trickprf");
    check::ok(c.agressed, "reset_keeps_agressed");
    check::eq_i32(c.stcnt, 4, "reset_keeps_stcnt");
    check::eq_i32(c.statusque, 5, "reset_keeps_statusque");
    check::eq_i32(c.turncnt, 6, "reset_keeps_turncnt");
    check::eq_i32(c.randtcnt, 7, "reset_keeps_randtcnt");
    check::eq_i32(c.swat, 8, "reset_keeps_swat");
    check::eq_i32(c.lrdirect, 1, "reset_keeps_lrdirect");
    check::eq_i32(c.uddirect, -1, "reset_keeps_uddirect");
    check::eq_i32(c.lrstart, 2, "reset_keeps_lrstart");
    check::eq_i32(c.udstart, 3, "reset_keeps_udstart");
    check::eq_i32(c.oxy, 4, "reset_keeps_oxy");
    check::eq_i32(c.ozy, 5, "reset_keeps_ozy");
    check::eq_i32(c.flycnt, 6, "reset_keeps_flycnt");
    check::ok(c.lrswt && c.udswt, "reset_keeps_swt_flags");
    check::eq_i32(c.actwait, 8, "reset_keeps_actwait");
    check::eq_i32(c.cntrn, 9, "reset_keeps_cntrn");
    check::ok(c.lastl && c.wlastl, "reset_keeps_lastl_flags");
    check::eq_i32(c.lwall, 2, "reset_keeps_lwall");

    // B7(8): fn==0 means the fpnt loops never run, so fpnt survives
    let mut c8 = Control::new();
    c8.fpnt[2] = 77;
    c8.reset(&CheckPoints::new());
    check::eq_str(
        format!("{:?}", c8.fpnt),
        "[0, 0, 77, 0, 0]",
        "reset_fn0_fpnt_survives",
    );

    // B7(9): nearest 0 -> 0 - 4 + nsp(10) == 6
    let mut p9 = CheckPoints::new();
    p9.fn_ = 1;
    p9.n = 1;
    p9.nsp = 10;
    let mut c9 = Control::new();
    c9.reset(&p9);
    check::eq_i32(c9.fpnt[0], 6, "reset_nearest_0_minus4_plus10_6");

    // B7(10): nearest index 1 -> 1 - 4 + 20 == 17
    let mut p10 = CheckPoints::new();
    p10.fn_ = 1;
    p10.n = 3;
    p10.nsp = 20;
    p10.fx[0] = 1000;
    p10.x[1] = 500;
    p10.x[2] = 2000;
    let mut c10 = Control::new();
    c10.reset(&p10);
    check::eq_i32(c10.fpnt[0], 17, "reset_nearest_index1_minus4_plus20_17");

    // B7(11): nsp==0 does not wrap the negative
    let mut p11 = CheckPoints::new();
    p11.fn_ = 1;
    p11.n = 1;
    p11.nsp = 0;
    let mut c11 = Control::new();
    c11.reset(&p11);
    check::eq_i32(c11.fpnt[0], -4, "reset_nsp0_no_wrap_minus4");

    // B7(12): equal distances keep the first index (strict < on later passes)
    let mut p12a = CheckPoints::new();
    p12a.fn_ = 1;
    p12a.n = 2;
    p12a.nsp = 10;
    p12a.x[0] = -100;
    p12a.x[1] = 100;
    let mut c12a = Control::new();
    c12a.reset(&p12a);
    check::eq_i32(c12a.fpnt[0], 6, "reset_equal_distance_keeps_first_index");
    let mut p12b = CheckPoints::new();
    p12b.fn_ = 1;
    p12b.n = 2;
    p12b.nsp = 10;
    p12b.x[0] = 300;
    p12b.x[1] = 100;
    let mut c12b = Control::new();
    c12b.reset(&p12b);
    check::eq_i32(c12b.fpnt[0], 7, "reset_later_nearer_point_wins_7");

    // B7(13): n==0 runs no inner iteration, so the stale fpnt[0]==0 is mapped
    let mut p13 = CheckPoints::new();
    p13.fn_ = 1;
    p13.n = 0;
    p13.nsp = 10;
    let mut c13 = Control::new();
    c13.reset(&p13);
    check::eq_i32(c13.fpnt[0], 6, "reset_n0_stale_fpnt_maps_to_6");
}

// ------------------------------------------------------------------ AI gates

/// `TControl.gatePins` (`:472-539`, B7(14-17)): the m==null-safe trick pass,
/// `usebounce`'s wtouch-over-mtouch selection and the strict plan gate
/// (`Control.java:71-73,484-498,796-880`).
fn gate_pins(k: &Tml, r: &Tml) {
    let mut car = car_seam();
    car_at(&mut car, 0, 0, 0, 0);

    // B7(14): no plan, no touch, all scy zero -> trickfase -1, afta true, and
    // no Medium is ever touched (Java's m==null survives the whole pass; the
    // port's `&mut Medium` is unused on this path).
    let mut md = mad(0);
    md.dest = false;
    md.mtouch = false;
    md.wtouch = false;
    md.power = 75.0;
    let mut c = Control::new();
    c.usebounce = false;
    c.preform(&mut md, &car, &cp(), &Trackers::new(), &mut Medium::new());
    check::eq_i32(
        c.trickfase,
        k.intv("trickfase_none"),
        "mnull_pass_trickfase_minus1",
    );
    check::ok(c.afta, "mnull_pass_afta_true");
    check::ok(
        !c.left && !c.right && !c.up && !c.down && !c.handb,
        "mnull_pass_five_flags_false",
    );

    // B7(15a): mtouch=true but usebounce selects wtouch, which is false -
    // identical outcome, still no Medium needed
    let mut md2 = mad(0);
    md2.mtouch = true;
    md2.wtouch = false;
    let mut c2 = Control::new();
    c2.usebounce = true;
    c2.preform(&mut md2, &car, &cp(), &Trackers::new(), &mut Medium::new());
    check::eq_i32(
        c2.trickfase,
        k.intv("trickfase_none"),
        "usebounce_selects_wtouch_trickfase_minus1",
    );
    check::ok(c2.afta, "usebounce_selects_wtouch_afta_true");
    check::eq_i32(c2.stcnt, 1, "usebounce_selects_wtouch_plan_skipped_stcnt1");

    // B7(15b): usebounce=false + mtouch=true runs the touch branch: trickfase
    // stays 0 and afta stays false (two random draws, :574-575)
    let mut pm = Preset::new(r);
    let mut md3 = mad(0);
    md3.mtouch = true;
    let mut c3 = Control::new();
    c3.trickfase = 5;
    c3.usebounce = false;
    c3.preform(&mut md3, &car, &cp(), &Trackers::new(), &mut pm.m);
    check::eq_i32(c3.trickfase, 0, "touch_branch_resets_trickfase_to_0");
    check::ok(!c3.afta, "touch_branch_afta_false");
    check::eq_i32(pm.draws(r), 2, "touch_branch_two_random_draws");

    // B7(16): the plan gate is strict - stcnt==statusque skips and increments
    let mut pm4 = Preset::new(r);
    let mut md4 = mad(0);
    md4.mtouch = true;
    let mut c4 = Control::new();
    c4.stcnt = 3;
    c4.statusque = 3;
    c4.acuracy = 7;
    c4.upwait = 8;
    c4.skiplev = 0.35;
    c4.preform(&mut md4, &car, &cp(), &Trackers::new(), &mut pm4.m);
    check::eq_i32(c4.stcnt, 4, "plan_gate_equal_skips_stcnt4");
    check::eq_i32(c4.acuracy, 7, "plan_gate_equal_keeps_acuracy");
    check::eq_i32(c4.upwait, 8, "plan_gate_equal_keeps_upwait");
    check::eq_i32(
        bits(c4.skiplev),
        bits(0.35),
        "plan_gate_equal_keeps_skiplev",
    );

    // B7(17): stcnt>statusque runs the plan and leaves stcnt==0 (:484)
    let mut pm5 = Preset::new(r);
    let mut md5 = mad(0);
    md5.mtouch = true;
    let mut c5 = Control::new();
    c5.stcnt = 4;
    c5.statusque = 3;
    c5.preform(&mut md5, &car, &cp(), &Trackers::new(), &mut pm5.m);
    check::eq_i32(c5.stcnt, 0, "plan_gate_open_stcnt0");
}

// -------------------------------------------------------------- plan latents

/// `TControl.planPins` (`:544-728`, B7(18-30)): the deterministic plan
/// formulas (`Control.java:74-275`).
fn plan_pins(k: &Tml, r: &Tml) {
    let car = car_seam();
    let t = Trackers::new();

    // B7(18): cntrn==0 is consumed at :151-188 - 5, then 4 on the next pass
    let mut m = Preset::new(r);
    let mut md = mad(0);
    md.mtouch = true;
    let mut c = Control::new();
    c.stcnt = 1;
    c.statusque = 0;
    c.preform(&mut md, &car, &cp(), &t, &mut m.m);
    check::eq_i32(c.cntrn, 5, "plan_cntrn_first_pass_5");
    // re-open the gate by hand: the pass itself reset stcnt to 0
    c.stcnt = 1;
    c.statusque = 0;
    c.preform(&mut md, &car, &cp(), &t, &mut m.m);
    check::eq_i32(c.cntrn, 4, "plan_cntrn_second_pass_4");
    check::eq_i32(m.draws(r), 20, "plan_two_passes_ten_draws_each");

    // B7(19): default cp, im=0, stage 1 -> (7-4)*4*(6-2)==48,
    // upwait floored to 20 by :101-103, skiplev 1.0-0.1
    let mut m19 = Preset::new(r);
    let mut md19 = mad(0);
    let mut c19 = Control::new();
    plan(&mut c19, &mut md19, &cp(), &t, &car, &mut m19.m);
    check::eq_i32(
        c19.acuracy,
        k.intv("acuracy_lead"),
        "plan_default_acuracy_48",
    );
    check::eq_i32(
        c19.upwait,
        k.intv("upwait_lead"),
        "plan_default_upwait_floor_20",
    );
    check::eq_i32(bits(c19.skiplev), bits(0.9), "plan_default_skiplev_0_9");
    check::eq_i32(c19.cntrn, 5, "plan_default_cntrn_5");
    check::eq_i32(m19.draws(r), 8, "plan_default_eight_draws");
    // usebounce picked the trick branch on this pass (0.9 > 75/100)
    check::eq_i32(
        c19.trickfase,
        k.intv("trickfase_none"),
        "plan_default_escapes_to_trick_branch",
    );

    // B7(20): stage 2, pos 4/1, im 1 -> (7-1)*4*(6-4)==48; 3^3*1.5 == 40.5
    // truncated to 40
    let mut m20 = Preset::new(r);
    let mut md20 = mad(1);
    let mut p20 = cp();
    p20.stage = 2;
    p20.pos[0] = 4;
    p20.pos[1] = 1;
    let mut c20 = Control::new();
    plan(&mut c20, &mut md20, &p20, &t, &car, &mut m20.m);
    check::eq_i32(
        c20.acuracy,
        k.intv("acuracy_lead"),
        "plan_stage2_pos4_1_acuracy_48",
    );
    check::eq_i32(
        c20.upwait,
        k.intv("upwait_stage2"),
        "plan_stage2_pos4_1_upwait_40",
    );

    // B7(21): stage 2, pos equal -> 24 and no stage-1 floor
    let mut m21 = Preset::new(r);
    let mut md21 = mad(2);
    let mut p21 = cp();
    p21.stage = 2;
    p21.pos[0] = 3;
    p21.pos[2] = 3;
    let mut c21 = Control::new();
    plan(&mut c21, &mut md21, &p21, &t, &car, &mut m21.m);
    check::eq_i32(
        c21.acuracy,
        k.intv("acuracy_stage2"),
        "plan_stage2_pos3_3_acuracy_24",
    );
    check::eq_i32(c21.upwait, 0, "plan_stage2_pos3_3_upwait_0");

    // B7(22): behind by 4 -> skiplev jumps +0.2 but is capped by stage:
    // stage 3 cap default 0.0, stage 4 cap 0.5, stage 1 cap 1.0
    for (stage, want) in [(3, 0.0f32), (4, 0.5), (1, 1.0)] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(1);
        let mut pp = cp();
        pp.stage = stage;
        pp.pos[0] = 4;
        pp.pos[1] = 0;
        let mut cc = Control::new();
        plan(&mut cc, &mut mdd, &pp, &t, &car, &mut mm.m);
        check::eq_i32(
            bits(cc.skiplev),
            bits(want),
            &format!("plan_behind_stage{stage}_skiplev_cap"),
        );
    }

    // B7(23): power==98 -> rampp -1 and saftey 0 ((98-98)/2 * anything)
    let mut m23 = Preset::new(r);
    let mut md23 = mad(0);
    md23.power = 98.0;
    let mut c23 = Control::new();
    plan(&mut c23, &mut md23, &cp(), &t, &car, &mut m23.m);
    check::eq_i32(c23.rampp, -1, "plan_power98_rampp_minus1");
    check::eq_i32(c23.saftey, 0, "plan_power98_saftey_0");
    check::ok(!c23.usebounce, "plan_power98_usebounce_false");

    // B7(24): power<60 forces rampp 1 (:143) and trickprf -1 (:233)
    let mut m24 = Preset::new(r);
    let mut md24 = mad(0);
    md24.power = 59.0;
    let mut p24 = cp();
    p24.stage = 2;
    let mut c24 = Control::new();
    plan(&mut c24, &mut md24, &p24, &t, &car, &mut m24.m);
    check::eq_i32(c24.rampp, 1, "plan_power59_rampp1_not_stage6");
    check::eq_i32(
        bits(c24.trickprf),
        bits(-1.0),
        "plan_power59_trickprf_minus1",
    );

    // B7(25): stage 11 forces trickprf -1 regardless of power
    let mut m25 = Preset::new(r);
    let mut md25 = mad(0);
    let mut p25 = cp();
    p25.stage = 11;
    let mut c25 = Control::new();
    plan(&mut c25, &mut md25, &p25, &t, &car, &mut m25.m);
    check::eq_i32(
        bits(c25.trickprf),
        bits(-1.0),
        "plan_stage11_trickprf_minus1",
    );

    // B7(26): stage 6 -> rampp 2; power 98 (trickprf 1.2-r/2 >= 0.75) is
    // capped to 0.5 by :237-239
    let mut m26 = Preset::new(r);
    let mut md26 = mad(0);
    md26.power = 98.0;
    let mut p26 = cp();
    p26.stage = 6;
    let mut c26 = Control::new();
    plan(&mut c26, &mut md26, &p26, &t, &car, &mut m26.m);
    check::eq_i32(c26.rampp, 2, "plan_stage6_rampp2");
    check::eq_i32(
        bits(c26.trickprf),
        bits(0.5),
        "plan_stage6_power98_trickprf_cap_0_5",
    );
    check::ok(!c26.usebounce, "plan_stage6_power98_usebounce_false");

    // B7(27): perfection is forced on stages 6/8/9/10/11 and 3@im4 even with
    // hitmag==0; stage 1/2/3/5/7 stay at the random arm (false here).
    for stage in [6, 8, 9, 10, 11] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        let mut pp = cp();
        pp.stage = stage;
        let mut cc = Control::new();
        plan(&mut cc, &mut mdd, &pp, &t, &car, &mut mm.m);
        check::ok(cc.perfection, &format!("plan_stage{stage}_perfection_true"));
    }
    let mut m27 = Preset::new(r);
    let mut md27 = mad(4);
    let mut p27 = cp();
    p27.stage = 3;
    let mut c27 = Control::new();
    plan(&mut c27, &mut md27, &p27, &t, &car, &mut m27.m);
    check::ok(c27.perfection, "plan_stage3_im4_perfection_true");
    for stage in [1, 2, 3, 5, 7] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        let mut pp = cp();
        pp.stage = stage;
        let mut cc = Control::new();
        plan(&mut cc, &mut mdd, &pp, &t, &car, &mut mm.m);
        check::ok(
            !cc.perfection,
            &format!("plan_stage{stage}_perfection_random_false"),
        );
    }

    // B7(28): 70% of maxmag forces perfection (>60), 50% does not; exactly
    // 60 is not >60 (and the 7th draw 0.8 > 0.5 leaves the random arm false)
    for (hit, want) in [(2450, true), (1750, false), (2100, false)] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        mdd.hitmag = hit;
        let mut cc = Control::new();
        plan(&mut cc, &mut mdd, &cp(), &t, &car, &mut mm.m);
        if want {
            check::ok(cc.perfection, &format!("plan_hitmag{hit}_perfection_true"));
        } else {
            check::ok(
                !cc.perfection,
                &format!("plan_hitmag{hit}_perfection_false"),
            );
        }
    }

    // B7(29): stage 9 disables usebounce whatever the random draw was
    let mut m29 = Preset::new(r);
    let mut md29 = mad(0);
    let mut p29 = cp();
    p29.stage = 9;
    let mut c29 = Control::new();
    plan(&mut c29, &mut md29, &p29, &t, &car, &mut m29.m);
    check::ok(!c29.usebounce, "plan_stage9_usebounce_false");

    // B7(30): power>50 with pos[0]-pos[im] <= 0 zeroes mustland exactly;
    // behind instead multiplies saftey (stage 1 -> 6*5 == 30) and keeps 0.7
    let mut m30 = Preset::new(r);
    let mut md30 = mad(0);
    let mut c30 = Control::new();
    plan(&mut c30, &mut md30, &cp(), &t, &car, &mut m30.m);
    check::eq_i32(bits(c30.mustland), 0, "plan_mustland_zero_bits");
    let mut m30b = Preset::new(r);
    let mut md30b = mad(1);
    let mut p30b = cp();
    p30b.pos[0] = 4;
    p30b.pos[1] = 0;
    let mut c30b = Control::new();
    plan(&mut c30b, &mut md30b, &p30b, &t, &car, &mut m30b.m);
    check::eq_i32(c30b.saftey, 30, "plan_behind_saftey_multiplied_30");
    check::eq_i32(bits(c30b.mustland), bits(0.7), "plan_behind_mustland_0_7");
}

/// `TControl.turnPins` (`:732-772`, B7(31-34)): the cntrn-gated
/// turntyp/agressed ladder (`Control.java:151-188`, all fixtures start with
/// cntrn==0).
fn turn_pins(r: &Tml) {
    let car = car_seam();
    let t = Trackers::new();

    // B7(31): stage 10 -> turntyp 2 (and agressed from :181)
    let mut m31 = Preset::new(r);
    let mut md31 = mad(0);
    let mut p31 = cp();
    p31.stage = 10;
    let mut c31 = Control::new();
    plan(&mut c31, &mut md31, &p31, &t, &car, &mut m31.m);
    check::eq_i32(c31.turntyp, 2, "turn_stage10_turntyp2");
    check::ok(c31.agressed, "turn_stage10_agressed_true");
    check::eq_i32(c31.cntrn, 5, "turn_stage10_cntrn_5");

    // B7(32): stage 6 overrides the random draw with 1 and agressed
    let mut m32 = Preset::new(r);
    let mut md32 = mad(0);
    let mut p32 = cp();
    p32.stage = 6;
    let mut c32 = Control::new();
    plan(&mut c32, &mut md32, &p32, &t, &car, &mut m32.m);
    check::eq_i32(c32.turntyp, 1, "turn_stage6_turntyp1");
    check::ok(c32.agressed, "turn_stage6_agressed_true");

    // B7(33): stage 3 @ im 4 -> turntyp 1, agressed true while attack==0
    let mut m33 = Preset::new(r);
    let mut md33 = mad(4);
    let mut p33 = cp();
    p33.stage = 3;
    let mut c33 = Control::new();
    plan(&mut c33, &mut md33, &p33, &t, &car, &mut m33.m);
    check::eq_i32(c33.turntyp, 1, "turn_stage3_im4_turntyp1");
    check::ok(c33.agressed, "turn_stage3_im4_agressed_true");

    // B7(34): stage 1, not behind -> :152 clears agressed and nothing sets it
    let mut m34 = Preset::new(r);
    let mut md34 = mad(0);
    let mut c34 = Control::new();
    c34.agressed = true;
    plan(&mut c34, &mut md34, &cp(), &t, &car, &mut m34.m);
    check::ok(!c34.agressed, "turn_stage1_agressed_false");
    check::eq_i32(c34.turntyp, 1, "turn_stage1_random_turntyp_1");
}

/// `TControl.trfixPins` (`:776-858`, B7(35-38)): the trfix ladder, the var22
/// overrides, the trfix==3 special path and the touch-branch attack reset
/// (`Control.java:449-472,503-505,882-885`).
fn trfix_pins(r: &Tml) {
    let car = car_seam();
    let t = Trackers::new();

    // B7(35): strict >50 / >80 boundaries on maxmag[0]==3500
    for (hit, want) in [(1750, 0), (1751, 1), (2800, 1), (2835, 2)] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        mdd.hitmag = hit;
        let mut cc = Control::new();
        plan(&mut cc, &mut mdd, &cp(), &t, &car, &mut mm.m);
        check::eq_i32(cc.trfix, want, &format!("trfix_hitmag{hit}_is{want}"));
    }

    // B7(36): stage 6/11 (and stage 9 @ im 4) set var22, so the >80 arm is
    // skipped and trfix tops out at 1
    for stage in [6, 11] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        mdd.hitmag = 2835;
        let mut pp = cp();
        pp.stage = stage;
        let mut cc = Control::new();
        plan(&mut cc, &mut mdd, &pp, &t, &car, &mut mm.m);
        check::eq_i32(cc.trfix, 1, &format!("trfix_var22_stage{stage}_1"));
    }
    let mut m36 = Preset::new(r);
    let mut md36 = mad(4);
    md36.hitmag = 2835;
    let mut p36 = cp();
    p36.stage = 9;
    let mut c36 = Control::new();
    plan(&mut c36, &mut md36, &p36, &t, &car, &mut m36.m);
    check::eq_i32(c36.trfix, 1, "trfix_var22_stage9_im4_1");

    // B7(37): preset trfix==3 takes the else path (:467-472). This fixture
    // keeps the TICK in the touch branch (power 98 -> usebounce false); the
    // default power-75 form escapes to the trick branch and is pinned below.
    let mut m37 = Preset::new(r);
    let mut md37 = mad(0);
    md37.mtouch = true;
    md37.power = 98.0;
    let mut c37 = Control::new();
    c37.trfix = 3;
    c37.upwait = 9;
    c37.acuracy = 9;
    c37.skiplev = 0.3;
    c37.clrnce = 9;
    plan(&mut c37, &mut md37, &cp(), &t, &car, &mut m37.m);
    check::eq_i32(c37.trfix, 3, "trfix3_preset_stays_3");
    check::eq_i32(c37.upwait, 0, "trfix3_upwait_0");
    check::eq_i32(c37.acuracy, 0, "trfix3_acuracy_0");
    check::eq_i32(bits(c37.skiplev), bits(1.0), "trfix3_skiplev_1");
    check::eq_i32(c37.clrnce, 2, "trfix3_clrnce_2");
    check::eq_i32(c37.statusque, 14, "trfix3_statusque_untouched_14");
    check::ok(md37.nofocus, "trfix3_nofocus_true");
    check::eq_i32(c37.trickfase, 0, "trfix3_touch_branch_trickfase_0");
    check::ok(!c37.afta, "trfix3_touch_branch_afta_false");

    // The B7 row's original power-75 fixture: usebounce becomes true and
    // wtouch is false, so the TRICK branch runs and :882-885 maps 3 -> 4 and
    // adds 30 to statusque.
    let mut m37b = Preset::new(r);
    let mut md37b = mad(0);
    md37b.mtouch = true;
    let mut c37b = Control::new();
    c37b.trfix = 3;
    c37b.statusque = 0;
    plan(&mut c37b, &mut md37b, &cp(), &t, &car, &mut m37b.m);
    check::eq_i32(c37b.trfix, 4, "trfix3_power75_escapes_to_trick_trfix4");
    check::eq_i32(c37b.statusque, 44, "trfix3_power75_trick_statusque_plus30");
    check::eq_i32(c37b.clrnce, 2, "trfix3_power75_clrnce_2");

    // B7(38): the touch branch zeroes attack when trfix is 2 or 3
    for tf in [2, 3] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        mdd.mtouch = true;
        let mut cc = Control::new();
        cc.trfix = tf;
        cc.attack = 40;
        cc.preform(&mut mdd, &car, &cp(), &t, &mut mm.m);
        check::eq_i32(cc.attack, 0, &format!("trfix{tf}_touch_attack_reset_0"));
        check::ok(!cc.handb, &format!("trfix{tf}_touch_no_handb"));
    }
}

/// `TControl.fixPins` (`:863-1025`, B7(39-45)): the fix-point gate, the two
/// loops and the special/bulistc override (`Control.java:636-676`). fpnt
/// lives on Control (`:16`), not on CheckPoints.
fn fix_pins(k: &Tml, r: &Tml) {
    let mut car = car_seam();
    let t = Trackers::new();

    // B7(39): missedcp!=0 with forget==false leaves the whole block skipped
    let mut m39 = Preset::new(r);
    let mut md39 = mad(0);
    md39.mtouch = true;
    md39.missedcp = 1;
    let mut p39 = cp();
    p39.fn_ = 1;
    let mut c39 = Control::new();
    c39.trfix = 1;
    c39.fpnt[0] = 0;
    c39.preform(&mut md39, &car, &p39, &t, &mut m39.m);
    check::eq_i32(c39.trfix, 1, "fix_missedcp1_trfix1_unchanged");
    check::ok(!c39.forget, "fix_missedcp1_forget_false");
    check::ok(!md39.nofocus, "fix_missedcp1_nofocus_false");

    // B7(40): missedcp==0 hits the fn loop; every field in the hit body is
    // written. randtcnt is the corrected final value: :666 writes -1 but :686
    // immediately recomputes it from acuracy==0 (turncnt 0 > -1).
    let mut m40 = Preset::new(r);
    let mut md40 = mad(0);
    md40.mtouch = true;
    let mut p40 = cp();
    p40.fn_ = 1;
    let mut c40 = Control::new();
    c40.trfix = 1;
    c40.upwait = 11;
    c40.acuracy = 12;
    c40.rampp = 3;
    c40.turntyp = 4;
    c40.randtcnt = 13;
    c40.actwait = 14;
    c40.fpnt[0] = 0;
    c40.preform(&mut md40, &car, &p40, &t, &mut m40.m);
    check::eq_i32(c40.trfix, 3, "fix_hit_trfix3");
    check::ok(!c40.forget, "fix_hit_forget_false");
    check::eq_i32(c40.upwait, 0, "fix_hit_upwait_0");
    check::eq_i32(c40.acuracy, 0, "fix_hit_acuracy_0");
    check::eq_i32(c40.rampp, 0, "fix_hit_rampp_0");
    check::eq_i32(c40.turntyp, 2, "fix_hit_turntyp2");
    check::eq_i32(c40.randtcnt, 0, "fix_hit_randtcnt_overwritten_to_0");
    check::eq_i32(c40.actwait, 0, "fix_hit_actwait_0");
    check::eq_i32(c40.turncnt, 0, "fix_hit_turncnt_recomputed_0");
    check::ok(md40.nofocus, "fix_hit_nofocus_true");

    // With a negative turncnt the :678 test fails to re-enter and the -1
    // written at :666 survives (this is the only observable form of pin 40's
    // -1; the real loop's turncnt is never negative).
    let mut m40b = Preset::new(r);
    let mut md40b = mad(0);
    md40b.mtouch = true;
    let mut p40b = cp();
    p40b.fn_ = 1;
    let mut c40b = Control::new();
    c40b.trfix = 1;
    c40b.turncnt = -5;
    c40b.fpnt[0] = 0;
    c40b.preform(&mut md40b, &car, &p40b, &t, &mut m40b.m);
    check::eq_i32(
        c40b.randtcnt,
        -1,
        "fix_hit_randtcnt_minus1_when_turncnt_negative",
    );
    check::eq_i32(c40b.turncnt, -4, "fix_hit_turncnt_increment_neg");

    // B7(41): the py<2000 boundary (py is in /100 units)
    for (x, want) in [(4400, 3), (4500, 1)] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        mdd.mtouch = true;
        let mut pp = cp();
        pp.fn_ = 1;
        let mut cc = Control::new();
        cc.trfix = 1;
        cc.fpnt[0] = 0;
        car_at(&mut car, x, 0, 0, 0);
        cc.preform(&mut mdd, &car, &pp, &t, &mut mm.m);
        check::eq_i32(cc.trfix, want, &format!("fix_py_boundary_x{x}_trfix{want}"));
    }
    car_at(&mut car, 0, 0, 0, 0);

    // B7(42): stage 11 disables the fix block outright
    let mut m42 = Preset::new(r);
    let mut md42 = mad(0);
    md42.mtouch = true;
    let mut p42 = cp();
    p42.stage = 11;
    p42.fn_ = 1;
    let mut c42 = Control::new();
    c42.trfix = 1;
    c42.fpnt[0] = 0;
    c42.preform(&mut md42, &car, &p42, &t, &mut m42.m);
    check::eq_i32(c42.trfix, 1, "fix_stage11_disabled_trfix1");
    check::ok(!md42.nofocus, "fix_stage11_nofocus_false");

    // B7(43): trfix==4 passes the gate (:636) and the loop re-marks 3
    let mut m43 = Preset::new(r);
    let mut md43 = mad(0);
    md43.mtouch = true;
    let mut p43 = cp();
    p43.fn_ = 1;
    let mut c43 = Control::new();
    c43.trfix = 4;
    c43.fpnt[0] = 0;
    c43.preform(&mut md43, &car, &p43, &t, &mut m43.m);
    check::eq_i32(c43.trfix, 3, "fix_trfix4_remark_3");
    check::ok(md43.nofocus, "fix_trfix4_nofocus_true");

    // B7(44): forget==true overrides a non-zero missedcp
    let mut m44 = Preset::new(r);
    let mut md44 = mad(0);
    md44.mtouch = true;
    md44.missedcp = 3;
    let mut p44 = cp();
    p44.fn_ = 1;
    let mut c44 = Control::new();
    c44.trfix = 1;
    c44.forget = true;
    c44.fpnt[0] = 0;
    c44.preform(&mut md44, &car, &p44, &t, &mut m44.m);
    check::ok(!c44.forget, "fix_forget_cleared");
    check::eq_i32(c44.trfix, 3, "fix_forget_trfix3");
    check::ok(md44.nofocus, "fix_forget_nofocus_true");

    // B7(45): trfix==2 nearest index is overridden to 1 on stage 9+bulistc
    // (:648-650) and special[1] sets forget. Both fpnt targets must be
    // >=2000 (/100) away or the generic loop at :660-671 clears forget again.
    let far = k.intv("fpnt_near_units") * 4 + 1000;
    let mut m45 = Preset::new(r);
    let mut md45 = mad(0);
    md45.mtouch = true;
    let mut p45 = cp();
    p45.stage = 9;
    p45.fn_ = 2;
    p45.special[1] = true;
    p45.x[0] = far;
    p45.x[6] = far;
    let mut c45 = Control::new();
    c45.trfix = 2;
    c45.bulistc = true;
    c45.fpnt[0] = 0;
    c45.fpnt[1] = 6;
    c45.preform(&mut md45, &car, &p45, &t, &mut m45.m);
    check::ok(c45.forget, "fix_special_override_forget_true");
    check::eq_i32(c45.trfix, 2, "fix_special_override_trfix_stays_2");
    check::ok(c45.bulistc, "fix_special_override_bulistc_true");
    check::ok(md45.nofocus, "fix_special_override_nofocus_true");

    // the contrast: fpnt[0] near the car makes :660-671 hit, clear forget and
    // promote trfix to 3
    let mut m45b = Preset::new(r);
    let mut md45b = mad(0);
    md45b.mtouch = true;
    let mut p45b = cp();
    p45b.stage = 9;
    p45b.fn_ = 2;
    p45b.special[1] = true;
    p45b.x[0] = 0;
    p45b.x[6] = 9000;
    let mut c45b = Control::new();
    c45b.trfix = 2;
    c45b.bulistc = true;
    c45b.fpnt[0] = 0;
    c45b.fpnt[1] = 6;
    c45b.preform(&mut md45b, &car, &p45b, &t, &mut m45b.m);
    check::ok(
        !c45b.forget,
        "fix_special_override_cleared_by_near_first_point",
    );
    check::eq_i32(
        c45b.trfix,
        3,
        "fix_special_override_near_first_point_trfix3",
    );
}

/// `TControl.bulistcPins` (`:1030-1065`, B7(46-48)): bulistc enablement and
/// its nofocus effect (`Control.java:474-481,631-633`). The bulistc branch
/// subtracts 2 from point (`:540`), so the fixture keeps point>=2 and
/// cp.n==5.
fn bulistc_pins(r: &Tml) {
    let car = car_seam();
    let t = Trackers::new();

    // B7(46): stage 6 && cn 7 && bulistc false
    let mut m46 = Preset::new(r);
    let mut md46 = mad(0);
    md46.cn = 7;
    md46.point = 2;
    let mut p46 = cp_bul();
    p46.stage = 6;
    let mut c46 = Control::new();
    plan(&mut c46, &mut md46, &p46, &t, &car, &mut m46.m);
    check::ok(c46.bulistc, "bulistc_stage6_cn7_true");

    // B7(47): stage 9, afta, pos 4, cn!=9, trfix!=0 (1751/3500 > 50%)
    let mut m47 = Preset::new(r);
    let mut md47 = mad(0);
    md47.hitmag = 1751;
    md47.point = 2;
    let mut p47 = cp_bul();
    p47.stage = 9;
    let mut c47 = Control::new();
    c47.afta = true;
    plan(&mut c47, &mut md47, &p47, &t, &car, &mut m47.m);
    check::ok(c47.bulistc, "bulistc_stage9_afta_pos4_true");
    check::eq_i32(c47.trfix, 1, "bulistc_stage9_fixture_trfix1");

    // B7(48): bulistc + trfix 2 + skipped plan routes through :631-633
    let mut m48 = Preset::new(r);
    let mut md48 = mad(0);
    md48.mtouch = true;
    let mut c48 = Control::new();
    c48.bulistc = true;
    c48.trfix = 2;
    c48.preform(&mut md48, &car, &cp(), &t, &mut m48.m);
    check::ok(md48.nofocus, "bulistc_trfix2_nofocus_true");
    check::eq_i32(c48.trfix, 2, "bulistc_trfix2_stays_2");
}

// ---------------------------------------------------------- attack acquisition

/// `TControl.attackPins` (`:1072-1286`): the plan's attack-target loop
/// (`Control.java:278-447`) — the per-checkpoint geometry gates, the
/// var15/var16 stage modifiers and the 500 clamp. Every fixture has
/// `afta == true` so the `:368` gate can pass.
fn attack_pins(r: &Tml) {
    let mut car = car_seam();
    let t = Trackers::new();
    car_at(&mut car, 0, 0, 0, 0);

    // A: stage 11 never caps var16 at 0.7 (:376). With pos 4/0 at im 0 the
    // candidate index 1 has 5*5/35 == 0.714, so the 8th draw (0.7) acquires
    // it: attack 40, acr 1, then turntyp = (int)(1 + 0.6*2) == 2. usebounce
    // stays true -> the trick branch runs, so the attack is NOT decremented.
    let mut ma = Preset::new(r);
    let mut mda = mad(0);
    let mut pa = cp();
    pa.stage = 11;
    pa.pos[0] = 4;
    pa.pos[1] = 0;
    pa.opx[1] = 100;
    pa.opz[1] = 0;
    let mut ca = Control::new();
    ca.afta = true;
    plan(&mut ca, &mut mda, &pa, &t, &car, &mut ma.m);
    check::eq_i32(ca.attack, 40, "attack_stage11_acquires_40");
    check::eq_i32(ca.acr, 1, "attack_stage11_acr_1");
    check::eq_i32(ca.turntyp, 2, "attack_stage11_turntyp_2");
    check::eq_i32(ma.draws(r), 13, "attack_stage11_thirteen_draws");
    check::eq_i32(ca.statusque, 4, "attack_stage11_statusque_4");

    // A2: a 15-clear difference clamps var15 to 1 (:370-373) and the reward
    // to 500 (:429-431)
    let mut ma2 = Preset::new(r);
    let mut mda2 = mad(0);
    let mut pa2 = cp();
    pa2.stage = 11;
    pa2.pos[0] = 4;
    pa2.pos[1] = 0;
    pa2.opx[1] = 100;
    pa2.opz[1] = 0;
    pa2.clear[1] = 14;
    let mut ca2 = Control::new();
    ca2.afta = true;
    plan(&mut ca2, &mut mda2, &pa2, &t, &car, &mut ma2.m);
    check::eq_i32(ca2.attack, 500, "attack_clear14_reward_clamped_500");
    check::eq_i32(ca2.acr, 1, "attack_clear14_acr_1");
    check::eq_i32(ca2.turntyp, 2, "attack_clear14_turntyp_2");

    // A3: var9 != 0 with pos[0] < pos[im] zeroes var16 (:380-382)
    let mut ma3 = Preset::new(r);
    let mut mda3 = mad(1);
    let mut pa3 = cp();
    pa3.stage = 2;
    pa3.pos[0] = 2;
    pa3.pos[1] = 4;
    pa3.opx[0] = 100;
    pa3.opz[0] = 0;
    let mut ca3 = Control::new();
    ca3.afta = true;
    plan(&mut ca3, &mut mda3, &pa3, &t, &car, &mut ma3.m);
    check::eq_i32(ca3.attack, 0, "attack_pos0_below_posim_no_attack");

    // B: stage 6 + bulistc turns var16 into 1.0 for index 0 (:400-405)
    let mut mb = Preset::new(r);
    let mut mdb = mad(1);
    let mut pb = cp();
    pb.stage = 6;
    pb.opx[0] = 100;
    pb.opz[0] = 0;
    let mut cb = Control::new();
    cb.afta = true;
    cb.bulistc = true;
    plan(&mut cb, &mut mdb, &pb, &t, &car, &mut mb.m);
    check::eq_i32(cb.attack, 40, "attack_stage6_bulistc_index0_40");
    check::eq_i32(cb.acr, 0, "attack_stage6_bulistc_acr_0");
    check::eq_i32(cb.turntyp, 2, "attack_stage6_bulistc_turntyp_2");

    // C: stage 3 @ im 4 - wasted<2 zeroes index 0 (:388-393), wasted>=2
    // halves the untouched var16; var14 stays 2000 until clear differs
    for wasted in [1, 2] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(4);
        let mut pp = cp();
        pp.stage = 3;
        pp.wasted = wasted;
        pp.opx[0] = 100;
        pp.opz[0] = 0;
        let mut cc = Control::new();
        cc.afta = true;
        plan(&mut cc, &mut mdd, &pp, &t, &car, &mut mm.m);
        check::eq_i32(
            cc.attack,
            0,
            &format!("attack_stage3_im4_wasted{wasted}_no_attack"),
        );
    }

    // C2: a 5-clear gap pushes var14 over 4000, so stage 3 @ im4 caps it at
    // 4000 (:362-364); the gate still passes and wasted<2 zeroes index 0
    let mut mc2 = Preset::new(r);
    let mut mdc2 = mad(4);
    let mut pc2 = cp();
    pc2.stage = 3;
    pc2.clear[0] = 5;
    pc2.opx[0] = 100;
    pc2.opz[0] = 0;
    let mut cc2 = Control::new();
    cc2.afta = true;
    plan(&mut cc2, &mut mdc2, &pc2, &t, &car, &mut mc2.m);
    check::eq_i32(
        cc2.attack,
        0,
        "attack_stage3_im4_clear5_var14_capped_no_attack",
    );

    // D: stage 8 zeroes var16 for every candidate (:407-409)
    let mut md_ = Preset::new(r);
    let mut mdd_ = mad(1);
    let mut pd = cp();
    pd.stage = 8;
    pd.opx[0] = 100;
    pd.opz[0] = 0;
    let mut cd = Control::new();
    cd.afta = true;
    plan(&mut cd, &mut mdd_, &pd, &t, &car, &mut md_.m);
    check::eq_i32(cd.attack, 0, "attack_stage8_no_attack");

    // E: stage 9 @ im 4 zeroes var16 (:411-413); usebounce is forced false so
    // the tick ends in the touch branch (two extra path-select draws)
    let mut me = Preset::new(r);
    let mut mde = mad(4);
    let mut pe = cp();
    pe.stage = 9;
    pe.opx[0] = 100;
    pe.opz[0] = 0;
    let mut ce = Control::new();
    ce.afta = true;
    plan(&mut ce, &mut mde, &pe, &t, &car, &mut me.m);
    check::eq_i32(ce.attack, 0, "attack_stage9_im4_no_attack");
    check::eq_i32(ce.trickfase, 0, "attack_stage9_im4_touch_branch");
    check::eq_i32(me.draws(r), 14, "attack_stage9_im4_fourteen_draws");

    // E2: stage 9 with pos 4 zeroes every candidate via :423-425
    let mut me2 = Preset::new(r);
    let mut mde2 = mad(0);
    let mut pe2 = cp();
    pe2.stage = 9;
    pe2.pos[1] = 3;
    pe2.opx[1] = 100;
    pe2.opz[1] = 0;
    let mut ce2 = Control::new();
    ce2.afta = true;
    plan(&mut ce2, &mut mde2, &pe2, &t, &car, &mut me2.m);
    check::eq_i32(ce2.attack, 0, "attack_stage9_pos3_zeroes_candidate");

    // E3: stage 9 + bulistc gives index 0 var16 1.0 (:415-421); pos[0]==2
    // avoids the :423 zeroing. usebounce is false, so the same tick's touch
    // branch decrements the freshly acquired 40 to 39 and skips nofocus.
    let mut me3 = Preset::new(r);
    let mut mde3 = mad(1);
    let mut pe3 = cp();
    pe3.stage = 9;
    pe3.pos[0] = 2;
    pe3.opx[0] = 100;
    pe3.opz[0] = 0;
    let mut ce3 = Control::new();
    ce3.afta = true;
    ce3.bulistc = true;
    plan(&mut ce3, &mut mde3, &pe3, &t, &car, &mut me3.m);
    check::eq_i32(
        ce3.attack,
        39,
        "attack_stage9_bulistc_40_then_touch_decrement_39",
    );
    check::eq_i32(ce3.acr, 0, "attack_stage9_bulistc_acr_0");
    check::eq_i32(ce3.turntyp, 2, "attack_stage9_bulistc_turntyp_2");
    check::eq_i32(me3.draws(r), 14, "attack_stage9_bulistc_fourteen_draws");
    check::ok(
        !mde3.nofocus,
        "attack_stage9_bulistc_attack_branch_skips_nofocus",
    );

    // G: stage 10 sets var7, so every var9 != 0 candidate gets var16 0
    let mut mg = Preset::new(r);
    let mut mdg = mad(1);
    let mut pg = cp();
    pg.stage = 10;
    pg.opx[0] = 100;
    pg.opz[0] = 0;
    let mut cg = Control::new();
    cg.afta = true;
    plan(&mut cg, &mut mdg, &pg, &t, &car, &mut mg.m);
    check::eq_i32(cg.attack, 0, "attack_stage10_var7_no_attack");
    check::eq_i32(cg.turntyp, 2, "attack_stage10_turntyp_2");

    // H: var13 > 100 + near target + random > 0.6-pos/10 zeroes clrnce and
    // acuracy (:438-444). xz 90 against a -90 target gives var13 180.
    let mut mh = Preset::new(r);
    let mut mdh = mad(0);
    let mut ph = cp();
    ph.stage = 2;
    ph.pos[0] = 4;
    ph.opx[1] = 100;
    ph.opz[1] = 0;
    let mut ch = Control::new();
    ch.afta = true;
    car_at(&mut car, 0, 0, 0, 90);
    plan(&mut ch, &mut mdh, &ph, &t, &car, &mut mh.m);
    check::eq_i32(ch.clrnce, 0, "attack_var13_gt100_clrnce_0");
    check::eq_i32(ch.acuracy, 0, "attack_var13_gt100_acuracy_0");
    check::eq_i32(mh.draws(r), 12, "attack_var13_gt100_twelve_draws");

    // I: zyinv adds 180 to var10 (:323-325); xz -90 gives the same angle
    let mut mi = Preset::new(r);
    let mut mdi = mad(0);
    let mut pi = cp();
    pi.stage = 2;
    pi.pos[0] = 4;
    pi.opx[1] = 100;
    pi.opz[1] = 0;
    let mut ci = Control::new();
    ci.afta = true;
    ci.zyinv = true;
    car_at(&mut car, 0, 0, 0, -90);
    plan(&mut ci, &mut mdi, &pi, &t, &car, &mut mi.m);
    check::eq_i32(ci.clrnce, 0, "attack_zyinv_clrnce_0");
    check::eq_i32(ci.acuracy, 0, "attack_zyinv_acuracy_0");
}

// ---------------------------------------------------------------- actuation

/// `TControl.actPins` (`:1292-1361`, B7(49-53)): the upcnt/actwait machine
/// and the attack steering clamp (`Control.java:507-531`).
fn act_pins(r: &Tml) {
    let mut car = car_seam();
    let t = Trackers::new();
    car_at(&mut car, 0, 0, 0, 0);

    // B7(49): upcnt 0 < 30 -> up, then increments
    let mut m49 = Preset::new(r);
    let mut md49 = mad(0);
    md49.mtouch = true;
    let mut c49 = Control::new();
    c49.preform(&mut md49, &car, &cp(), &t, &mut m49.m);
    check::ok(c49.up, "act_upcnt0_up_true");
    check::eq_i32(c49.upcnt, 1, "act_upcnt0_becomes_1");
    check::eq_i32(c49.actwait, 0, "act_upcnt0_actwait_0");

    // B7(50): upcnt 25 >= 25+0 -> reset and load upwait
    let mut m50 = Preset::new(r);
    let mut md50 = mad(0);
    md50.mtouch = true;
    let mut c50 = Control::new();
    c50.upcnt = 25;
    c50.upwait = 40;
    c50.preform(&mut md50, &car, &cp(), &t, &mut m50.m);
    check::eq_i32(c50.upcnt, 0, "act_upcnt25_reset_0");
    check::eq_i32(c50.actwait, 40, "act_upcnt25_actwait_upwait_40");
    check::ok(c50.up, "act_upcnt25_up_true");

    // B7(51): opx==x and opz==z -> 0/0 NaN -> (int)NaN == 0
    let mut m51 = Preset::new(r);
    let mut md51 = mad(0);
    md51.mtouch = true;
    let mut p51 = cp();
    p51.opx[0] = 0;
    p51.opz[0] = 0;
    let mut c51 = Control::new();
    c51.attack = 1;
    c51.acr = 0;
    c51.preform(&mut md51, &car, &p51, &t, &mut m51.m);
    check::eq_i32(c51.pan, 0, "act_nan_pan_0");
    check::eq_i32(c51.attack, 0, "act_nan_attack_0");
    check::ok(c51.up, "act_nan_up_true");

    // B7(52): target 100 ahead -> pan 270 normalised to -90, right; speed 0
    // prevents the down/handb arm
    let mut m52 = Preset::new(r);
    let mut md52 = mad(0);
    md52.mtouch = true;
    let mut p52 = cp();
    p52.opx[0] = 100;
    p52.opz[0] = 0;
    let mut c52 = Control::new();
    c52.attack = 1;
    c52.acr = 0;
    c52.turntyp = 0;
    c52.preform(&mut md52, &car, &p52, &t, &mut m52.m);
    check::eq_i32(c52.pan, -90, "act_target_right_pan_minus90");
    check::ok(c52.right, "act_target_right_right_true");
    check::ok(!c52.lastl, "act_target_right_lastl_false");
    check::eq_i32(c52.attack, 0, "act_target_right_attack_0");
    check::ok(c52.up, "act_target_right_up_true");

    // B7(53): the decrement path clamps at 0 - 501 -> 500
    let mut m53 = Preset::new(r);
    let mut md53 = mad(0);
    md53.mtouch = true;
    let mut p53 = cp();
    p53.opx[0] = 100;
    let mut c53 = Control::new();
    c53.attack = 501;
    c53.acr = 0;
    c53.preform(&mut md53, &car, &p53, &t, &mut m53.m);
    check::eq_i32(c53.attack, 500, "act_attack501_becomes_500");
}

// ---------------------------------------------------------------- steering

/// `TControl.steerPins` (`:1367-1493`, B7(54-60)): pan normalisation, the
/// clrnce dead band and the three turntyp arms (`Control.java:678-763`).
fn steer_pins(r: &Tml) {
    let mut car = car_seam();
    let t = Trackers::new();

    // B7(54): turncnt==randtcnt==0 freezes the preset pan for this tick
    let mut m54 = Preset::new(r);
    let mut md54 = mad(0);
    md54.mtouch = true;
    let mut c54 = Control::new();
    c54.turncnt = 0;
    c54.randtcnt = 0;
    c54.pan = 0;
    c54.clrnce = 5;
    c54.turntyp = 0;
    car_at(&mut car, 0, 0, 0, 90);
    c54.preform(&mut md54, &car, &cp(), &t, &mut m54.m);
    check::ok(c54.right, "steer_xz90_right_true");
    check::ok(!c54.left, "steer_xz90_left_false");
    check::ok(!c54.lastl, "steer_xz90_lastl_false");
    check::eq_i32(c54.pan, 0, "steer_turncnt0_pan_frozen_0");
    check::eq_i32(c54.turncnt, 1, "steer_turncnt0_becomes_1");

    // B7(55): a skipped plan leaves clrnce at 100 -> |90| > 100 is false, so
    // neither flag fires; an open plan rewrites clrnce to 5 first (:277)
    let mut m55 = Preset::new(r);
    let mut md55 = mad(0);
    md55.mtouch = true;
    let mut c55 = Control::new();
    c55.clrnce = 100;
    c55.pan = 0;
    car_at(&mut car, 0, 0, 0, 90);
    c55.preform(&mut md55, &car, &cp(), &t, &mut m55.m);
    check::eq_i32(c55.clrnce, 100, "steer_skipped_plan_keeps_clrnce_100");
    check::ok(!c55.left && !c55.right, "steer_clrnce100_dead_band");
    let mut m55b = Preset::new(r);
    let mut md55b = mad(0);
    md55b.power = 98.0; // keeps usebounce false -> touch branch
    let mut c55b = Control::new();
    c55b.clrnce = 100;
    c55b.pan = 0;
    car_at(&mut car, 0, 0, 0, 90);
    plan(&mut c55b, &mut md55b, &cp(), &t, &car, &mut m55b.m);
    check::eq_i32(c55b.clrnce, 5, "steer_open_plan_rewrites_clrnce_5");
    check::ok(c55b.right, "steer_open_plan_right_true");

    // B7(56): pan 270 normalises to -90; |diff|==180 takes the second branch
    let mut m56 = Preset::new(r);
    let mut md56 = mad(0);
    md56.mtouch = true;
    let mut c56 = Control::new();
    c56.pan = 270;
    car_at(&mut car, 0, 0, 0, 90);
    c56.preform(&mut md56, &car, &cp(), &t, &mut m56.m);
    check::eq_i32(c56.pan, -90, "steer_pan270_normalises_minus90");
    check::ok(c56.left, "steer_diff180_left_true");
    check::ok(c56.lastl, "steer_diff180_lastl_true");

    // B7(57): xz -90 turns left
    let mut m57 = Preset::new(r);
    let mut md57 = mad(0);
    md57.mtouch = true;
    let mut c57 = Control::new();
    car_at(&mut car, 0, 0, 0, -90);
    c57.preform(&mut md57, &car, &cp(), &t, &mut m57.m);
    check::ok(c57.left, "steer_xzminus90_left_true");
    check::ok(c57.lastl, "steer_xzminus90_lastl_true");

    // a preset negative pan normalises +360 before the comparison (:705-707)
    let mut m_neg = Preset::new(r);
    let mut md_neg = mad(0);
    md_neg.mtouch = true;
    let mut c_neg = Control::new();
    c_neg.pan = -200;
    c_neg.upcnt = 30;
    car_at(&mut car, 0, 0, 0, 0);
    c_neg.preform(&mut md_neg, &car, &cp(), &t, &mut m_neg.m);
    check::eq_i32(c_neg.pan, 160, "steer_panminus200_normalises_160");
    check::ok(c_neg.left, "steer_panminus200_left_true");
    check::ok(!c_neg.right, "steer_panminus200_right_false");

    // B7(58): fast enough + diff>50 + turntyp 1 -> down, and up is forced
    // off when !agressed
    let mut m58 = Preset::new(r);
    let mut md58 = mad(0);
    md58.mtouch = true;
    md58.speed = 100.0;
    let mut c58 = Control::new();
    c58.turntyp = 1;
    car_at(&mut car, 0, 0, 0, 90);
    c58.preform(&mut md58, &car, &cp(), &t, &mut m58.m);
    check::ok(c58.down, "steer_turntyp1_down_true");
    check::ok(!c58.up, "steer_turntyp1_up_forced_false");
    check::ok(c58.right, "steer_turntyp1_right_true");

    // B7(59): turntyp 2 uses handb; turntyp 0 does neither
    let mut m59 = Preset::new(r);
    let mut md59 = mad(0);
    md59.mtouch = true;
    md59.speed = 100.0;
    let mut c59 = Control::new();
    c59.turntyp = 2;
    car_at(&mut car, 0, 0, 0, 90);
    c59.preform(&mut md59, &car, &cp(), &t, &mut m59.m);
    check::ok(c59.handb, "steer_turntyp2_handb_true");
    check::ok(!c59.down, "steer_turntyp2_down_false");
    check::ok(!c59.up, "steer_turntyp2_up_forced_false");
    let mut m59b = Preset::new(r);
    let mut md59b = mad(0);
    md59b.mtouch = true;
    md59b.speed = 100.0;
    let mut c59b = Control::new();
    c59b.turntyp = 0;
    car_at(&mut car, 0, 0, 0, 90);
    c59b.preform(&mut md59b, &car, &cp(), &t, &mut m59b.m);
    check::ok(!c59b.down && !c59b.handb, "steer_turntyp0_neither");
    check::ok(c59b.up, "steer_turntyp0_up_kept");

    // B7(60): agressed keeps the up bit
    let mut m60 = Preset::new(r);
    let mut md60 = mad(0);
    md60.mtouch = true;
    md60.speed = 100.0;
    let mut c60 = Control::new();
    c60.turntyp = 2;
    c60.agressed = true;
    car_at(&mut car, 0, 0, 0, 90);
    c60.preform(&mut md60, &car, &cp(), &t, &mut m60.m);
    check::ok(c60.handb, "steer_agressed_handb_true");
    check::ok(c60.up, "steer_agressed_up_kept");
}

// --------------------------------------------------------------------- wall

/// `TControl.wallPins` (`:1498-1602`, B7(61-65)): the wall/hold machine
/// (`Control.java:713-715,767-795`).
fn wall_pins(r: &Tml) {
    let mut car = car_seam();
    car_at(&mut car, 0, 0, 0, 0);

    // B7(61): wall 0 + hold 0 zeroes clrnce, sticky lastl sets left and the
    // wall is consumed; dam[0]==0 -> hold becomes 1
    let mut m61 = Preset::new(r);
    let mut md61 = mad(0);
    md61.mtouch = true;
    let mut c61 = Control::new();
    c61.wall = 0;
    c61.hold = 0;
    c61.lwall = -1;
    c61.lastl = true;
    c61.upcnt = 30;
    c61.preform(&mut md61, &car, &cp(), &Trackers::new(), &mut m61.m);
    check::eq_i32(c61.clrnce, 0, "wall_touch_clrnce_0");
    check::ok(c61.left, "wall_touch_lastl_left_true");
    check::ok(!c61.right, "wall_touch_right_false");
    check::ok(c61.wlastl, "wall_touch_wlastl_true");
    check::eq_i32(c61.lwall, 0, "wall_touch_lwall_0");
    check::eq_i32(c61.hold, 1, "wall_touch_dam0_hold_1");
    check::eq_i32(c61.wall, -1, "wall_touch_consumed_minus1");

    // B7(62): lastl false picks right
    let mut m62 = Preset::new(r);
    let mut md62 = mad(0);
    md62.mtouch = true;
    let mut c62 = Control::new();
    c62.wall = 0;
    c62.lwall = -1;
    c62.lastl = false;
    c62.upcnt = 30;
    c62.preform(&mut md62, &car, &cp(), &Trackers::new(), &mut m62.m);
    check::ok(c62.right, "wall_touch_lastl_false_right_true");
    check::ok(!c62.wlastl, "wall_touch_lastl_false_wlastl_false");
    check::eq_i32(c62.hold, 1, "wall_touch_lastl_false_hold_1");
    check::eq_i32(c62.wall, -1, "wall_touch_lastl_false_consumed");

    // B7(63): lwall==wall keeps the sticky side even when lastl flipped
    let mut m63 = Preset::new(r);
    let mut md63 = mad(0);
    md63.mtouch = true;
    let mut c63 = Control::new();
    c63.wall = 0;
    c63.lwall = 0;
    c63.wlastl = true;
    c63.lastl = false;
    c63.upcnt = 30;
    c63.preform(&mut md63, &car, &cp(), &Trackers::new(), &mut m63.m);
    check::ok(c63.left, "wall_sticky_left_true");
    check::ok(!c63.right, "wall_sticky_right_false");
    check::eq_i32(c63.hold, 1, "wall_sticky_dam0_hold_1");
    check::eq_i32(c63.wall, -1, "wall_sticky_consumed");

    // lwall==wall with wlastl false takes the right branch (:777-781)
    let mut mw = Preset::new(r);
    let mut mdw = mad(0);
    mdw.mtouch = true;
    let mut cw = Control::new();
    cw.wall = 0;
    cw.lwall = 0;
    cw.wlastl = false;
    cw.upcnt = 30;
    cw.preform(&mut mdw, &car, &cp(), &Trackers::new(), &mut mw.m);
    check::ok(!cw.left, "wall_sticky_right_left_false");
    check::ok(cw.right, "wall_sticky_right_right_true");
    check::eq_i32(cw.hold, 1, "wall_sticky_right_hold_1");
    check::eq_i32(cw.wall, -1, "wall_sticky_right_consumed");

    // B7(64): no wall but a live hold skips the steering block and decrements
    let mut m64 = Preset::new(r);
    let mut md64 = mad(0);
    md64.mtouch = true;
    let mut c64 = Control::new();
    c64.wall = -1;
    c64.hold = 5;
    c64.upcnt = 30;
    c64.preform(&mut md64, &car, &cp(), &Trackers::new(), &mut m64.m);
    check::eq_i32(c64.hold, 4, "hold_5_decrements_4");
    check::ok(
        !c64.left && !c64.right && !c64.down && !c64.handb,
        "hold_gate_suppresses_steering",
    );

    // B7(65): dam[0]!=0 increments and clamps at 10; dam[0]==0 drops to 1
    let mut m65 = Preset::new(r);
    let mut md65 = mad(0);
    md65.mtouch = true;
    let mut t65 = Trackers::new();
    t65.dam[0] = 5;
    let mut c65 = Control::new();
    c65.wall = 0;
    c65.hold = 10;
    c65.upcnt = 30;
    c65.preform(&mut md65, &car, &cp(), &t65, &mut m65.m);
    check::eq_i32(c65.hold, 10, "wall_dam5_hold_clamped_10");
    check::eq_i32(c65.wall, -1, "wall_dam5_consumed");
    let mut m65b = Preset::new(r);
    let mut md65b = mad(0);
    md65b.mtouch = true;
    let mut c65b = Control::new();
    c65b.wall = 0;
    c65b.hold = 10;
    c65b.upcnt = 30;
    c65b.preform(&mut md65b, &car, &cp(), &Trackers::new(), &mut m65b.m);
    check::eq_i32(c65b.hold, 1, "wall_dam0_hold_drops_1");
}

// ------------------------------------------------------------- path select

/// `TControl.pathPins` (`:1610-1721`): the touch branch's checkpoint
/// selection (`Control.java:533-633`) — the bulistc skip-2 path, the rampp
/// biases, the skiplev random walk and the lap-count filter. pan is observed
/// with turncnt 1 > randtcnt 0, which recomputes it from the selected
/// index (`:678-686`).
fn path_pins(r: &Tml) {
    let mut car = car_seam();
    car_at(&mut car, 0, 0, 0, 0);

    // P1: bulistc's own branch (:539-555) takes point-2 and skips the random
    // walk entirely, so this pass draws nothing and pan stays frozen
    let mut m1 = Preset::new(r);
    let mut md1 = mad(0);
    md1.point = 2;
    let mut c1 = Control::new();
    c1.bulistc = true;
    c1.trfix = 1;
    touch(&mut c1, &mut md1, &cp(), &car, &mut m1.m);
    check::ok(md1.nofocus, "path_bulistc_nofocus_true");
    check::eq_i32(m1.draws(r), 0, "path_bulistc_zero_draws");
    check::eq_i32(c1.pan, 0, "path_bulistc_pan_frozen_0");
    check::eq_i32(c1.turncnt, 1, "path_bulistc_turncnt_1");
    check::eq_i32(c1.randtcnt, 0, "path_bulistc_randtcnt_0");

    // P2: stage 9 clamps var23 to 76 and the -4 skip walks backwards
    // (:545-553); the whole pass stays in the bulistc arm
    let mut m2 = Preset::new(r);
    let mut md2 = mad(0);
    md2.point = 100;
    let mut p2 = cp();
    p2.stage = 9;
    let mut c2 = Control::new();
    c2.bulistc = true;
    c2.trfix = 1;
    touch(&mut c2, &mut md2, &p2, &car, &mut m2.m);
    check::ok(md2.nofocus, "path_stage9_point100_nofocus_true");

    // P3: rampp 1 looks one past a non-positive checkpoint for a -2
    // (:557-566); the selected index 1 makes pan -90
    let mut m3 = Preset::new(r);
    let mut md3 = mad(0);
    let mut p3 = cp_bul();
    p3.typ[0] = 0;
    p3.typ[1] = -2;
    p3.x[1] = 100;
    let mut c3 = Control::new();
    c3.rampp = 1;
    c3.turncnt = 1;
    touch(&mut c3, &mut md3, &p3, &car, &mut m3.m);
    check::eq_i32(c3.pan, -90, "path_rampp1_moves_to_minus2_index_1");
    check::eq_i32(c3.turncnt, 0, "path_rampp1_turncnt_reset_0");
    check::eq_i32(c3.randtcnt, 0, "path_rampp1_randtcnt_0");

    // P4: rampp -1 skips a -2 checkpoint at point (:568-572)
    let mut m4 = Preset::new(r);
    let mut md4 = mad(0);
    let mut p4 = cp_bul();
    p4.typ[0] = -2;
    p4.x[1] = 100;
    let mut c4 = Control::new();
    c4.rampp = -1;
    c4.turncnt = 1;
    touch(&mut c4, &mut md4, &p4, &car, &mut m4.m);
    check::eq_i32(c4.pan, -90, "path_ramppminus1_skips_minus2_to_index_1");

    // P5: first draw <= skiplev and second > skiplev runs the while that
    // skips -1 entries (:574-581); x[2] selects a left-side target
    let mut m5 = Preset::with_start(r, 1); // 0.0, 0.9, 0.8, ...
    let mut md5 = mad(0);
    let mut p5 = cp_bul();
    p5.typ[0] = -1;
    p5.typ[1] = -1;
    p5.typ[2] = 1;
    p5.x[2] = -100;
    let mut c5 = Control::new();
    c5.skiplev = 0.4;
    c5.turncnt = 1;
    touch(&mut c5, &mut md5, &p5, &car, &mut m5.m);
    check::eq_i32(
        c5.pan,
        90,
        "path_skiplev_inner_walk_skips_minus1_to_index_2",
    );
    check::eq_i32(m5.draws(r), 3, "path_skiplev_inner_walk_three_draws");

    // P6: draw > skiplev enters the lap-count scan (:582-615). clear 4 with
    // five positive checkpoints walks forward to index 3, whose left-side
    // target makes pan 90.
    let mut m6 = Preset::new(r);
    let mut md6 = mad(0);
    md6.clear = 4;
    let mut p6 = cp_bul();
    p6.x[3] = -100;
    let mut c6 = Control::new();
    c6.skiplev = -1.0;
    c6.turncnt = 1;
    touch(&mut c6, &mut md6, &p6, &car, &mut m6.m);
    check::eq_i32(c6.pan, 90, "path_clear_scan_walks_to_index_3");
    check::eq_i32(m6.draws(r), 2, "path_clear_scan_two_draws");

    // P7: rampp 2 backs up over a -2 one past the selected index (:618-629)
    let mut m7 = Preset::with_start(r, 1);
    let mut md7 = mad(0);
    let mut p7 = cp_bul();
    p7.typ[0] = -1;
    p7.typ[1] = -1;
    p7.typ[2] = 1;
    p7.typ[3] = -2;
    p7.x[1] = -100;
    let mut c7 = Control::new();
    c7.rampp = 2;
    c7.skiplev = 0.4;
    c7.turncnt = 1;
    touch(&mut c7, &mut md7, &p7, &car, &mut m7.m);
    check::eq_i32(c7.pan, 90, "path_rampp2_backs_up_to_index_1");
    check::eq_i32(m7.draws(r), 3, "path_rampp2_three_draws");
}

// ------------------------------------------------------------------- tricks

/// `TControl.trickPins` (`:1727-1918`, B7(66-75)): the airborne trick phases
/// (`Control.java:796-1115`). Every fixture here runs mtouch==false and
/// wtouch==false.
fn trick_pins(k: &Tml, r: &Tml) {
    let mut car = car_seam();
    car_at(&mut car, 0, 0, 0, 0);

    // B7(66): trickfase 1, lrdirect -1 -> flycnt++, handb + left
    let mut m66 = Preset::new(r);
    let mut md66 = mad(0);
    let mut c66 = Control::new();
    c66.trickfase = k.intv("trickfase_air");
    c66.lrdirect = -1;
    c66.lrstart = 0;
    c66.saftey = 30;
    c66.preform(&mut md66, &car, &cp(), &Trackers::new(), &mut m66.m);
    check::eq_i32(c66.flycnt, 1, "trick_fase1_flycnt_1");
    check::ok(c66.handb, "trick_fase1_lrdirect_minus1_handb");
    check::ok(c66.left, "trick_fase1_lrdirect_minus1_left");
    check::ok(!c66.right, "trick_fase1_lrdirect_minus1_right_false");
    check::eq_i32(
        c66.trickfase,
        k.intv("trickfase_air"),
        "trick_fase1_stays_1",
    );
    check::eq_i32(m66.draws(r), 0, "trick_fase1_no_random");

    // B7(67): lrdirect +1 -> right; uddirect -1 -> down; +1 -> up
    let mut m67 = Preset::new(r);
    let mut md67 = mad(0);
    let mut c67 = Control::new();
    c67.trickfase = k.intv("trickfase_air");
    c67.lrdirect = 1;
    c67.preform(&mut md67, &car, &cp(), &Trackers::new(), &mut m67.m);
    check::ok(c67.handb && c67.right, "trick_fase1_lrdirect1_handb_right");
    let mut m67b = Preset::new(r);
    let mut md67b = mad(0);
    let mut c67b = Control::new();
    c67b.trickfase = k.intv("trickfase_air");
    c67b.uddirect = -1;
    c67b.preform(&mut md67b, &car, &cp(), &Trackers::new(), &mut m67b.m);
    check::ok(
        c67b.handb && c67b.down,
        "trick_fase1_uddirect_minus1_handb_down",
    );
    let mut m67c = Preset::new(r);
    let mut md67c = mad(0);
    let mut c67c = Control::new();
    c67c.trickfase = k.intv("trickfase_air");
    c67c.uddirect = 1;
    c67c.preform(&mut md67c, &car, &cp(), &Trackers::new(), &mut m67c.m);
    check::ok(c67c.handb && c67c.up, "trick_fase1_uddirect1_handb_up");

    // B7(68): the saftey test flips to phase 2 and clears all trick flags.
    // The same pass then advances swat 0 -> 1 (:945-954), not 0.
    let mut m68 = Preset::new(r);
    let mut md68 = mad(0);
    let mut c68 = Control::new();
    c68.trickfase = k.intv("trickfase_air");
    c68.saftey = -1;
    c68.udcomp = true;
    c68.lrcomp = true;
    c68.udbare = true;
    c68.lrbare = true;
    c68.onceu = true;
    c68.onced = true;
    c68.oncel = true;
    c68.oncer = true;
    c68.swat = 7;
    c68.preform(&mut md68, &car, &cp(), &Trackers::new(), &mut m68.m);
    check::eq_i32(
        c68.trickfase,
        k.intv("trickfase_ground"),
        "trick_saftey_negative_phase2",
    );
    check::eq_i32(c68.swat, 1, "trick_saftey_negative_swat_same_call_1");
    check::ok(
        !c68.udcomp && !c68.lrcomp && !c68.udbare && !c68.lrbare,
        "trick_saftey_negative_clears_comp_bare",
    );
    check::ok(
        !c68.onceu && !c68.onced && !c68.oncel && !c68.oncer,
        "trick_saftey_negative_clears_once",
    );

    // B7(69): swat 0 reads the compression flags; wtouch advances 1 -> 2
    let mut m69 = Preset::new(r);
    let mut md69 = mad(0);
    md69.wtouch = true;
    md69.dcomp = 1.0;
    let mut c69 = Control::new();
    c69.trickfase = k.intv("trickfase_ground");
    c69.preform(&mut md69, &car, &cp(), &Trackers::new(), &mut m69.m);
    check::ok(c69.udbare, "trick_swat0_dcomp_sets_udbare");
    check::ok(!c69.lrbare, "trick_swat0_no_lrcomp_lrbare_false");
    check::eq_i32(c69.swat, 2, "trick_wtouch_swat2");
    check::ok(c69.onced, "trick_swat0_non_capsized_marks_onced");
    check::eq_i32(m69.draws(r), 1, "trick_swat0_one_random_draw");

    // B7(70): swat 2 without wtouch -> 3 unconditionally
    let mut m70 = Preset::new(r);
    let mut md70 = mad(0);
    let mut c70 = Control::new();
    c70.trickfase = k.intv("trickfase_ground");
    c70.swat = 2;
    c70.preform(&mut md70, &car, &cp(), &Trackers::new(), &mut m70.m);
    check::eq_i32(c70.swat, 3, "trick_swat2_notouch_swat3");
    check::eq_i32(m70.draws(r), 0, "trick_swat2_notouch_zero_draws");

    // B7(71): capsized + dcomp>ucomp + !udcomp + !onced -> down, onced is
    // written only by the non-capsized arm so it stays false
    let mut m71 = Preset::new(r);
    let mut md71 = mad(0);
    md71.capsized = true;
    md71.dcomp = 2.0;
    md71.ucomp = 1.0;
    md71.lcomp = 5.0;
    let mut c71 = Control::new();
    c71.trickfase = k.intv("trickfase_ground");
    c71.swat = 3;
    c71.udbare = true;
    c71.preform(&mut md71, &car, &cp(), &Trackers::new(), &mut m71.m);
    check::ok(c71.down, "trick_capsized_dcomp_down_true");
    check::ok(!c71.onced, "trick_capsized_onced_untouched_false");
    check::ok(
        !c71.udcomp,
        "trick_capsized_lcomp_minus_rcomp_5_udcomp_false",
    );

    // B7(72): capsized + udcomp + var27 135 -> up, onced stays true
    let mut m72 = Preset::new(r);
    let mut md72 = mad(0);
    md72.capsized = true;
    md72.dcomp = 2.0;
    md72.ucomp = 1.0;
    md72.lcomp = 1.0;
    md72.pzy = 45;
    let mut c72 = Control::new();
    c72.trickfase = k.intv("trickfase_ground");
    c72.swat = 3;
    c72.udbare = true;
    c72.udcomp = true;
    c72.onced = true;
    c72.preform(&mut md72, &car, &cp(), &Trackers::new(), &mut m72.m);
    check::ok(c72.up, "trick_capsized_udcomp_var27_135_up");
    check::ok(!c72.down, "trick_capsized_udcomp_down_false");
    check::ok(c72.onced, "trick_capsized_udcomp_onced_stays_true");

    // B7(73): lrbare + capsized + lcomp>rcomp + !lrcomp + !oncel -> left
    let mut m73 = Preset::new(r);
    let mut md73 = mad(0);
    md73.capsized = true;
    md73.lcomp = 10.0;
    let mut c73 = Control::new();
    c73.trickfase = k.intv("trickfase_ground");
    c73.swat = 3;
    c73.lrbare = true;
    c73.preform(&mut md73, &car, &cp(), &Trackers::new(), &mut m73.m);
    check::ok(c73.left, "trick_capsized_lcomp_left_true");
    check::ok(!c73.right, "trick_capsized_lcomp_right_false");
    check::ok(!c73.oncel, "trick_capsized_oncel_untouched_false");
    check::ok(
        !c73.lrcomp,
        "trick_capsized_lcomp_minus_rcomp_10_lrcomp_false",
    );

    // B7(74): not capsized + udcomp + perfection false -> no up/down write
    // (the random arm at :1011 is the else of `if (udcomp)`), onced becomes true
    let mut m74 = Preset::new(r);
    let mut md74 = mad(0);
    md74.dcomp = 1.0;
    md74.pzy = 45;
    let mut c74 = Control::new();
    c74.trickfase = k.intv("trickfase_ground");
    c74.swat = 3;
    c74.udbare = true;
    c74.udcomp = true;
    c74.perfection = false;
    c74.preform(&mut md74, &car, &cp(), &Trackers::new(), &mut m74.m);
    check::ok(!c74.up && !c74.down, "trick_perfection_false_no_write");
    check::ok(c74.onced, "trick_perfection_false_onced_true");
    check::ok(!c74.onceu, "trick_perfection_false_onceu_false");
    check::eq_i32(m74.draws(r), 0, "trick_perfection_false_zero_draws");

    // B7(75): phase 0 -> 1 -> the same call's phase-1 block. trickprf must be
    // preset (the plan block never runs on a non-touch pass); -1 makes the
    // :799 draw always pass. var26 == 40*1200/4000 == 12 > 9.
    let mut m75 = Preset::new(r);
    let mut md75 = mad(0);
    md75.power = 50.0;
    for i in 0..4 {
        md75.scy[i] = 10.0;
    }
    let mut c75 = Control::new();
    c75.trickprf = -1.0;
    car_at(&mut car, 0, 1500, 0, 0);
    c75.preform(&mut md75, &car, &cp(), &Trackers::new(), &mut m75.m);
    check::eq_i32(m75.draws(r), 1, "trick_phase0_one_draw");
    check::ok(c75.afta, "trick_phase0_afta_true");
    check::eq_i32(
        c75.trickfase,
        k.intv("trickfase_air"),
        "trick_phase0_sets_phase1",
    );
    check::eq_i32(c75.uddirect, -1, "trick_phase0_var26_12_uddirect_minus1");
    check::eq_i32(c75.udstart, 0, "trick_phase0_udstart_0");
    check::eq_i32(c75.flycnt, 1, "trick_phase0_same_call_flycnt_1");
    check::ok(c75.handb, "trick_phase0_same_call_handb");
    check::ok(c75.down, "trick_phase0_same_call_down");
    check::ok(!c75.up, "trick_phase0_same_call_up_false");

    // the other arm of :799: a 0.1 draw is NOT > trickprf/3 for trickprf 0.5
    let mut lo = Preset::with_start(r, 2);
    let mut md75b = mad(0);
    for i in 0..4 {
        md75b.scy[i] = 10.0;
    }
    let mut c75b = Control::new();
    c75b.trickprf = 0.5;
    car_at(&mut car, 0, 1500, 0, 0);
    c75b.preform(&mut md75b, &car, &cp(), &Trackers::new(), &mut lo.m);
    check::eq_i32(lo.draws(r), 1, "trick_phase0_neg_one_draw");
    check::eq_i32(
        c75b.trickfase,
        k.intv("trickfase_none"),
        "trick_phase0_low_draw_sets_minus1",
    );
    check::ok(c75b.afta, "trick_phase0_low_draw_afta_true");
}

/// `TControl.trickExtraPins` (`:1924-2364`): the remaining trick-arm corners —
/// the var26 >= 16 setup (`Control.java:812-873`), the lrswt/udswt reversals
/// (`:891-919`), the capsized swat-2 swap (`:962-970`) and the
/// capsized/non-capsized up/down/left/right ladders (`:975-1113`).
fn trick_extra_pins(k: &Tml, r: &Tml) {
    let mut car = car_seam();
    car_at(&mut car, 0, 0, 0, 0);

    // T1: var26 == 40*1600/4000 == 16 enters the else arm (:836-859) with a
    // decreasing preset; trfix 3 then clears lrdirect (:862-868)
    let mut m1 = Preset::new(r);
    let mut md1 = mad(0);
    for i in 0..4 {
        md1.scy[i] = 10.0;
    }
    let mut c1 = Control::new();
    c1.trickprf = -1.0;
    c1.trfix = 3;
    car_at(&mut car, 0, 1900, 0, 0);
    c1.preform(&mut md1, &car, &cp(), &Trackers::new(), &mut m1.m);
    check::eq_i32(
        c1.trickfase,
        k.intv("trickfase_air"),
        "trick_var26_16_else_arm_phase1",
    );
    check::eq_i32(c1.uddirect, -1, "trick_var26_16_else_arm_uddirect_minus1");
    check::eq_i32(c1.lrdirect, 0, "trick_var26_16_trfix3_clears_lrdirect");
    check::eq_i32(c1.lrstart, 12, "trick_var26_16_lrstart_12");
    check::eq_i32(c1.udstart, -9, "trick_var26_16_udstart_negative_9");
    check::ok(!c1.lrswt && !c1.udswt, "trick_var26_16_swt_false");
    check::eq_i32(c1.trfix, 4, "trick_var26_16_trfix3_becomes_4");
    check::eq_i32(m1.draws(r), 12, "trick_var26_16_twelve_draws");
    check::ok(c1.down && c1.handb, "trick_var26_16_phase1_down_handb");
    check::ok(!c1.left && !c1.right, "trick_var26_16_no_lr_steer");
    check::eq_i32(c1.flycnt, 1, "trick_var26_16_flycnt_1");

    // T2: an increasing preset makes :812 `!(r > r)` true, entering the
    // lr-first arm (:813-835)
    let mut m2 = Preset::with_start(r, 2); // 0.1, 0.0, 0.9, 0.8, ...
    let mut md2 = mad(0);
    for i in 0..4 {
        md2.scy[i] = 10.0;
    }
    let mut c2 = Control::new();
    c2.trickprf = -1.0;
    car_at(&mut car, 0, 1900, 0, 0);
    c2.preform(&mut md2, &car, &cp(), &Trackers::new(), &mut m2.m);
    check::eq_i32(
        c2.trickfase,
        k.intv("trickfase_air"),
        "trick_var26_16_first_arm_phase1",
    );
    check::eq_i32(c2.uddirect, -1, "trick_var26_16_first_arm_uddirect_minus1");
    check::eq_i32(c2.lrdirect, -1, "trick_var26_16_first_arm_lrdirect_minus1");
    check::eq_i32(
        c2.lrstart,
        -6,
        "trick_var26_16_first_arm_lrstart_negative_6",
    );
    check::eq_i32(c2.udstart, 3, "trick_var26_16_first_arm_udstart_3");
    check::eq_i32(m2.draws(r), 12, "trick_var26_16_first_arm_twelve_draws");
    check::ok(c2.left && c2.handb, "trick_var26_16_first_arm_left_handb");
    check::eq_i32(c2.flycnt, 1, "trick_var26_16_first_arm_flycnt_1");

    // T3: stage 8 skips the lr arm and the trfix 3 branch forces
    // uddirect 1 with udswt false (:848,869-872)
    let mut m3 = Preset::new(r);
    let mut md3 = mad(0);
    for i in 0..4 {
        md3.scy[i] = 10.0;
    }
    let mut p3 = cp();
    p3.stage = 8;
    let mut c3 = Control::new();
    c3.trickprf = -1.0;
    c3.trfix = 3;
    car_at(&mut car, 0, 1900, 0, 0);
    c3.preform(&mut md3, &car, &p3, &Trackers::new(), &mut m3.m);
    check::eq_i32(
        c3.trickfase,
        k.intv("trickfase_air"),
        "trick_stage8_trfix3_phase1",
    );
    check::eq_i32(c3.uddirect, 1, "trick_stage8_trfix3_uddirect_1");
    check::ok(!c3.udswt, "trick_stage8_trfix3_udswt_false");
    check::eq_i32(c3.lrdirect, 0, "trick_stage8_trfix3_lrdirect_0");
    check::eq_i32(m3.draws(r), 8, "trick_stage8_trfix3_eight_draws");
    check::eq_i32(c3.trfix, 4, "trick_stage8_trfix3_becomes_4");

    // T3b: three phases of the same var26>=16 setup reach the other pair
    // else arms - S6 sets udswt (:844-845), S0 sets lrswt (:856-857) and S9
    // takes lrdirect=1 (:850-852)
    let mut s6 = Preset::with_start(r, 6);
    let mut mds6 = mad(0);
    for i in 0..4 {
        mds6.scy[i] = 10.0;
    }
    let mut cs6 = Control::new();
    cs6.trickprf = -1.0;
    car_at(&mut car, 0, 1900, 0, 0);
    cs6.preform(&mut mds6, &car, &cp(), &Trackers::new(), &mut s6.m);
    check::eq_i32(
        cs6.trickfase,
        k.intv("trickfase_air"),
        "trick_phase6_phase1",
    );
    check::eq_i32(cs6.lrstart, 15, "trick_phase6_lrstart_15");
    check::eq_i32(cs6.udstart, 0, "trick_phase6_udstart_0");
    check::ok(cs6.udswt, "trick_phase6_udswt_true");
    check::ok(!cs6.lrswt, "trick_phase6_lrswt_false");
    check::eq_i32(s6.draws(r), 12, "trick_phase6_twelve_draws");
    let mut s0 = Preset::with_start(r, 0);
    let mut mds0 = mad(0);
    for i in 0..4 {
        mds0.scy[i] = 10.0;
    }
    let mut cs0 = Control::new();
    cs0.trickprf = -1.0;
    car_at(&mut car, 0, 1900, 0, 0);
    cs0.preform(&mut mds0, &car, &cp(), &Trackers::new(), &mut s0.m);
    check::eq_i32(cs0.lrstart, 27, "trick_phase0_lrstart_27");
    check::eq_i32(cs0.udstart, -4, "trick_phase0_udstart_negative_4");
    check::ok(cs0.lrswt, "trick_phase0_lrswt_true");
    check::ok(!cs0.udswt, "trick_phase0_udswt_false");
    let mut s9 = Preset::with_start(r, 9);
    let mut mds9 = mad(0);
    for i in 0..4 {
        mds9.scy[i] = 10.0;
    }
    let mut cs9 = Control::new();
    cs9.trickprf = -1.0;
    car_at(&mut car, 0, 1900, 0, 0);
    cs9.preform(&mut mds9, &car, &cp(), &Trackers::new(), &mut s9.m);
    check::eq_i32(cs9.lrdirect, 1, "trick_phase9_lrdirect_1");
    check::eq_i32(cs9.lrstart, 24, "trick_phase9_lrstart_24");
    check::eq_i32(cs9.udstart, -3, "trick_phase9_udstart_negative_3");

    // T4: lrswt flips lrdirect when |pxy-oxy| > 180 (:891-899)
    let mut m4 = Preset::new(r);
    let mut md4 = mad(0);
    md4.pxy = 200;
    let mut c4 = Control::new();
    c4.trickfase = k.intv("trickfase_air");
    c4.lrdirect = -1;
    c4.lrswt = true;
    c4.oxy = 0;
    c4.lrstart = 0;
    c4.preform(&mut md4, &car, &cp(), &Trackers::new(), &mut m4.m);
    check::eq_i32(c4.lrdirect, 1, "trick_lrswt_flips_lrdirect_to_1");
    check::ok(!c4.lrswt, "trick_lrswt_cleared");
    check::ok(c4.right && c4.handb, "trick_lrswt_flip_right_handb");

    // T5: udswt flips uddirect (:910-918)
    let mut m5 = Preset::new(r);
    let mut md5 = mad(0);
    md5.pzy = 200;
    let mut c5 = Control::new();
    c5.trickfase = k.intv("trickfase_air");
    c5.uddirect = -1;
    c5.udswt = true;
    c5.ozy = 0;
    c5.udstart = 0;
    c5.preform(&mut md5, &car, &cp(), &Trackers::new(), &mut m5.m);
    check::eq_i32(c5.uddirect, 1, "trick_udswt_flips_uddirect_to_1");
    check::ok(!c5.udswt, "trick_udswt_cleared");
    check::ok(c5.up && c5.handb, "trick_udswt_flip_up_handb");

    // T6: capsized swat 2 with a 0.6 draw > mustland 0.5 swaps the bare
    // flags (:962-970) and then advances swat to 3
    let mut m6 = Preset::with_start(r, 7); // first draw 0.6
    let mut md6 = mad(0);
    md6.capsized = true;
    md6.dcomp = 1.0;
    let mut c6 = Control::new();
    c6.trickfase = k.intv("trickfase_ground");
    c6.swat = 2;
    c6.udbare = true;
    c6.preform(&mut md6, &car, &cp(), &Trackers::new(), &mut m6.m);
    check::eq_i32(c6.swat, 3, "trick_swat2_capsized_swap_swat3");
    check::ok(!c6.udbare, "trick_swat2_capsized_swap_udbare_false");
    check::ok(c6.lrbare, "trick_swat2_capsized_swap_lrbare_true");
    check::eq_i32(m6.draws(r), 1, "trick_swat2_capsized_swap_one_draw");

    // T7: pzy -180 wraps var27 through +360 to -90 (:978-984); capsized +
    // udcomp at exactly 90 takes the down branch
    let mut m7 = Preset::new(r);
    let mut md7 = mad(0);
    md7.capsized = true;
    md7.dcomp = 1.0;
    md7.pzy = -180;
    let mut c7 = Control::new();
    c7.trickfase = k.intv("trickfase_ground");
    c7.swat = 3;
    c7.udbare = true;
    c7.udcomp = true;
    c7.onced = true;
    c7.preform(&mut md7, &car, &cp(), &Trackers::new(), &mut m7.m);
    check::ok(c7.down, "trick_capsized_var27_wrap_down_true");
    check::ok(!c7.up, "trick_capsized_var27_wrap_up_false");
    check::ok(c7.onced, "trick_capsized_var27_wrap_onced_kept");

    // T8: not capsized + udcomp + perfection + |var27-90|>30 -> up (:1028-1035)
    let mut m8 = Preset::new(r);
    let mut md8 = mad(0);
    md8.dcomp = 1.0;
    md8.pzy = 45;
    let mut c8 = Control::new();
    c8.trickfase = k.intv("trickfase_ground");
    c8.swat = 3;
    c8.udbare = true;
    c8.udcomp = true;
    c8.perfection = true;
    c8.preform(&mut md8, &car, &cp(), &Trackers::new(), &mut m8.m);
    check::ok(c8.up, "trick_perfection_true_var27_135_up");
    check::ok(!c8.down, "trick_perfection_true_down_false");
    check::ok(c8.onced, "trick_perfection_true_onced_true");

    // T9: not capsized + !udcomp + draw > mustland -> up (:1036-1040)
    let mut m9 = Preset::with_start(r, 7);
    let mut md9 = mad(0);
    md9.dcomp = 1.0;
    let mut c9 = Control::new();
    c9.trickfase = k.intv("trickfase_ground");
    c9.swat = 3;
    c9.udbare = true;
    c9.preform(&mut md9, &car, &cp(), &Trackers::new(), &mut m9.m);
    check::ok(c9.up, "trick_random_udcomp_false_up_true");
    check::ok(c9.onced, "trick_random_udcomp_false_onced_true");
    check::eq_i32(m9.draws(r), 1, "trick_random_udcomp_false_one_draw");

    // T10: lrbare capsized + lrcomp + var29 135 -> left (:1063-1069)
    let mut m10 = Preset::new(r);
    let mut md10 = mad(0);
    md10.capsized = true;
    md10.lcomp = 10.0;
    md10.pxy = 45;
    let mut c10 = Control::new();
    c10.trickfase = k.intv("trickfase_ground");
    c10.swat = 3;
    c10.lrbare = true;
    c10.lrcomp = true;
    c10.preform(&mut md10, &car, &cp(), &Trackers::new(), &mut m10.m);
    check::ok(c10.left, "trick_capsized_lrcomp_var29_135_left");
    check::ok(!c10.right, "trick_capsized_lrcomp_right_false");
    check::ok(!c10.oncel, "trick_capsized_lrcomp_oncel_false");

    // T11: lrbare capsized + !lrcomp + !oncel with lcomp<=rcomp -> right (:1089-1098)
    let mut m11 = Preset::new(r);
    let mut md11 = mad(0);
    md11.capsized = true;
    md11.rcomp = 10.0;
    let mut c11 = Control::new();
    c11.trickfase = k.intv("trickfase_ground");
    c11.swat = 3;
    c11.lrbare = true;
    c11.preform(&mut md11, &car, &cp(), &Trackers::new(), &mut m11.m);
    check::ok(!c11.left, "trick_capsized_else_right_left_false");
    check::ok(c11.right, "trick_capsized_else_right_right_true");
    check::ok(!c11.oncer, "trick_capsized_else_right_oncer_false");

    // T12: not capsized + lrcomp + perfection + var29 135 -> left (:1075-1082)
    let mut m12 = Preset::new(r);
    let mut md12 = mad(0);
    md12.lcomp = 10.0;
    md12.pxy = 45;
    let mut c12 = Control::new();
    c12.trickfase = k.intv("trickfase_ground");
    c12.swat = 3;
    c12.lrbare = true;
    c12.lrcomp = true;
    c12.perfection = true;
    c12.oncel = true;
    c12.preform(&mut md12, &car, &cp(), &Trackers::new(), &mut m12.m);
    check::ok(c12.left, "trick_perfection_lr_left_true");
    check::ok(!c12.right, "trick_perfection_lr_right_false");
    check::ok(c12.oncel, "trick_perfection_lr_oncel_kept");

    // T13: not capsized + !lrcomp + lcomp>rcomp + draw > mustland -> right
    let mut m13 = Preset::with_start(r, 7);
    let mut md13 = mad(0);
    md13.lcomp = 10.0;
    let mut c13 = Control::new();
    c13.trickfase = k.intv("trickfase_ground");
    c13.swat = 3;
    c13.lrbare = true;
    c13.preform(&mut md13, &car, &cp(), &Trackers::new(), &mut m13.m);
    check::ok(!c13.left, "trick_random_lr_left_false");
    check::ok(c13.right, "trick_random_lr_right_true");
    check::ok(c13.oncel, "trick_random_lr_oncel_true");
    check::eq_i32(m13.draws(r), 1, "trick_random_lr_one_draw");

    // T14: not capsized + !lrcomp + lcomp<=rcomp + draw > mustland -> left
    let mut m14 = Preset::with_start(r, 7);
    let mut md14 = mad(0);
    md14.rcomp = 10.0;
    let mut c14 = Control::new();
    c14.trickfase = k.intv("trickfase_ground");
    c14.swat = 3;
    c14.lrbare = true;
    c14.preform(&mut md14, &car, &cp(), &Trackers::new(), &mut m14.m);
    check::ok(c14.left, "trick_random_lr_else_left_true");
    check::ok(!c14.right, "trick_random_lr_else_right_false");
    check::ok(c14.oncer, "trick_random_lr_else_oncer_true");

    // T15: zyinv adds 180 to var29 and pxy -180 wraps through +360 (:1044-1057)
    let mut m15 = Preset::new(r);
    let mut md15 = mad(0);
    md15.capsized = true;
    md15.lcomp = 10.0;
    md15.pxy = -180;
    let mut c15 = Control::new();
    c15.trickfase = k.intv("trickfase_ground");
    c15.swat = 3;
    c15.lrbare = true;
    c15.zyinv = true;
    c15.preform(&mut md15, &car, &cp(), &Trackers::new(), &mut m15.m);
    check::ok(c15.left, "trick_lr_zyinv_wrap_left");

    // T16: lcomp-rcomp < 10 with oncel sets lrcomp (:1059-1061)
    let mut m16 = Preset::new(r);
    let mut md16 = mad(0);
    md16.capsized = true;
    md16.lcomp = 5.0;
    md16.pxy = 45;
    let mut c16 = Control::new();
    c16.trickfase = k.intv("trickfase_ground");
    c16.swat = 3;
    c16.lrbare = true;
    c16.oncel = true;
    c16.preform(&mut md16, &car, &cp(), &Trackers::new(), &mut m16.m);
    check::ok(c16.lrcomp, "trick_lrcomp_set_below_10_with_oncel");
    check::ok(c16.left, "trick_lrcomp_set_left");

    // T17: capsized + lrcomp at var29 0 takes the right branch (:1090-1095)
    let mut m17 = Preset::new(r);
    let mut md17 = mad(0);
    md17.capsized = true;
    md17.lcomp = 10.0;
    md17.pxy = -90;
    let mut c17 = Control::new();
    c17.trickfase = k.intv("trickfase_ground");
    c17.swat = 3;
    c17.lrbare = true;
    c17.lrcomp = true;
    c17.preform(&mut md17, &car, &cp(), &Trackers::new(), &mut m17.m);
    check::ok(!c17.left, "trick_capsized_lrcomp_var29_0_left_false");
    check::ok(c17.right, "trick_capsized_lrcomp_var29_0_right_true");

    // T18: capsized with onceu + small compression gap sets udcomp (:987-989)
    let mut m18 = Preset::new(r);
    let mut md18 = mad(0);
    md18.capsized = true;
    md18.ucomp = 2.0;
    md18.dcomp = 1.0;
    md18.lcomp = 1.0;
    md18.pzy = 45;
    let mut c18 = Control::new();
    c18.trickfase = k.intv("trickfase_ground");
    c18.swat = 3;
    c18.udbare = true;
    c18.onceu = true;
    c18.preform(&mut md18, &car, &cp(), &Trackers::new(), &mut m18.m);
    check::ok(c18.udcomp, "trick_udcomp_set_below_5_with_onceu");
    check::ok(c18.up, "trick_udcomp_set_up");
    check::ok(!c18.down, "trick_udcomp_set_down_false");

    // T19: not capsized, lcomp<=rcomp, !lrcomp, perfection - the perfection
    // arm needs lrcomp, so only oncer is written (:1099-1112)
    let mut m19 = Preset::new(r);
    let mut md19 = mad(0);
    md19.rcomp = 10.0;
    md19.pxy = 45;
    let mut c19 = Control::new();
    c19.trickfase = k.intv("trickfase_ground");
    c19.swat = 3;
    c19.lrbare = true;
    c19.perfection = true;
    c19.preform(&mut md19, &car, &cp(), &Trackers::new(), &mut m19.m);
    check::ok(!c19.left, "trick_lr_else_perfection_left_false");
    check::ok(!c19.right, "trick_lr_else_perfection_right_false");
    check::ok(c19.oncer, "trick_lr_else_perfection_oncer_true");

    // T20: capsized + udcomp with var27 0 -> down (:1018-1023)
    let mut m20 = Preset::new(r);
    let mut md20 = mad(0);
    md20.capsized = true;
    md20.dcomp = 1.0;
    md20.pzy = -90;
    let mut c20 = Control::new();
    c20.trickfase = k.intv("trickfase_ground");
    c20.swat = 3;
    c20.udbare = true;
    c20.udcomp = true;
    c20.preform(&mut md20, &car, &cp(), &Trackers::new(), &mut m20.m);
    check::ok(c20.down, "trick_capsized_udcomp_var27_0_down");
    check::ok(!c20.up, "trick_capsized_udcomp_var27_0_up_false");

    // T21: non-capsized + dcomp<=ucomp + udcomp + perfection covers both
    // sides of :1028-1035
    for pzy in [45, -90] {
        let mut mm = Preset::new(r);
        let mut mdd = mad(0);
        mdd.ucomp = 1.0;
        mdd.pzy = pzy;
        let mut cc = Control::new();
        cc.trickfase = k.intv("trickfase_ground");
        cc.swat = 3;
        cc.udbare = true;
        cc.udcomp = true;
        cc.perfection = true;
        cc.preform(&mut mdd, &car, &cp(), &Trackers::new(), &mut mm.m);
        if pzy == 45 {
            check::ok(cc.up, "trick_perfection_upcomp_arm_var27_135_up");
            check::ok(!cc.down, "trick_perfection_upcomp_arm_down_false");
        } else {
            check::ok(cc.down, "trick_perfection_upcomp_arm_var27_0_down");
            check::ok(!cc.up, "trick_perfection_upcomp_arm_up_false");
        }
    }

    // T22: lcomp>rcomp + lrcomp + perfection var29 0 -> right (:1075-1081)
    let mut m22 = Preset::new(r);
    let mut md22 = mad(0);
    md22.lcomp = 10.0;
    md22.pxy = -90;
    let mut c22 = Control::new();
    c22.trickfase = k.intv("trickfase_ground");
    c22.swat = 3;
    c22.lrbare = true;
    c22.lrcomp = true;
    c22.perfection = true;
    c22.preform(&mut md22, &car, &cp(), &Trackers::new(), &mut m22.m);
    check::ok(c22.right, "trick_perfection_lr_var29_0_right");
    check::ok(!c22.left, "trick_perfection_lr_var29_0_left_false");

    // T23: lcomp<=rcomp + lrcomp + perfection var29 135 -> left (:1100-1106)
    let mut m23 = Preset::new(r);
    let mut md23 = mad(0);
    md23.rcomp = 1.0;
    md23.pxy = 45;
    let mut c23 = Control::new();
    c23.trickfase = k.intv("trickfase_ground");
    c23.swat = 3;
    c23.lrbare = true;
    c23.lrcomp = true;
    c23.perfection = true;
    c23.preform(&mut md23, &car, &cp(), &Trackers::new(), &mut m23.m);
    check::ok(c23.left, "trick_perfection_lr_else_var29_135_left");
    check::ok(!c23.right, "trick_perfection_lr_else_var29_135_right_false");

    // T24: capsized + udcomp with dcomp<=ucomp and var27 0 -> down
    // (:1017-1023, the other half of the T20 branch)
    let mut m24 = Preset::new(r);
    let mut md24 = mad(0);
    md24.capsized = true;
    md24.ucomp = 1.0;
    md24.pzy = -90;
    let mut c24 = Control::new();
    c24.trickfase = k.intv("trickfase_ground");
    c24.swat = 3;
    c24.udbare = true;
    c24.udcomp = true;
    c24.preform(&mut md24, &car, &cp(), &Trackers::new(), &mut m24.m);
    check::ok(c24.down, "trick_capsized_else_udcomp_var27_0_down");
    check::ok(!c24.up, "trick_capsized_else_udcomp_var27_0_up_false");
}

/// `TControl.cornerPins` (`:2372-2794`): the remaining single-line clamps and
/// ladder corners — the plan-side skiplev/rampp/saftey/mustland clamps
/// (`Control.java:123-126,139-141,190-193,228-230`), the turntyp attack
/// overrides (`:169-173`), the var10 index normalisation (`:327-333`), the
/// checkpoint-index wraps (`:542,551,560,570,578`), the second steering branch
/// (`:742-763`) and the trick ladder's remaining else arms
/// (`:891-919,950-952,966-969,1000-1112`).
fn corner_pins(k: &Tml, r: &Tml) {
    let mut car = car_seam();
    let t = Trackers::new();
    car_at(&mut car, 0, 0, 0, 0);

    // K1: skiplev 0.05 - 0.1 clamps at 0 (:124-126)
    let mut m1 = Preset::new(r);
    let mut md1 = mad(0);
    let mut c1 = Control::new();
    c1.skiplev = 0.05;
    plan(&mut c1, &mut md1, &cp(), &t, &car, &mut m1.m);
    check::eq_i32(bits(c1.skiplev), bits(0.0), "corner_skiplev_floor_0");

    // K2: power 70 with a 0.2 first draw gives rampp -1, which :139 lifts to
    // 0 because power < 75
    let mut m2 = Preset::with_start(r, 3); // 0.2, 0.1, 0.0, ...
    let mut md2 = mad(0);
    md2.power = 70.0;
    let mut p2 = cp();
    p2.stage = 2;
    let mut c2 = Control::new();
    plan(&mut c2, &mut md2, &p2, &t, &car, &mut m2.m);
    check::eq_i32(c2.rampp, 0, "corner_power70_ramppminus1_lifted_to_0");
    check::eq_i32(c2.turntyp, 0, "corner_power70_turntyp_0");
    check::eq_i32(c2.saftey, 7, "corner_power70_saftey_7");
    check::eq_i32(m2.draws(r), 10, "corner_power70_ten_draws");

    // K3: attack != 0 forces turntyp 2 for non-9/10/11 stages (:169-170);
    // usebounce keeps this pass in the trick branch so attack is not spent
    let mut m3 = Preset::new(r);
    let mut md3 = mad(0);
    let mut c3 = Control::new();
    c3.attack = 5;
    let mut p3 = cp();
    p3.stage = 2;
    plan(&mut c3, &mut md3, &p3, &t, &car, &mut m3.m);
    check::eq_i32(c3.turntyp, 2, "corner_attack_nonzero_turntyp_2");
    check::eq_i32(c3.attack, 5, "corner_attack_trick_branch_not_decremented");

    // K4: attack != 0 on stage 9 uses (int)(random*3) instead (:171-173); the
    // touch branch then decrements 5 -> 4
    let mut m4 = Preset::new(r);
    let mut md4 = mad(0);
    let mut c4 = Control::new();
    c4.attack = 5;
    let mut p4 = cp();
    p4.stage = 9;
    plan(&mut c4, &mut md4, &p4, &t, &car, &mut m4.m);
    check::eq_i32(c4.turntyp, 0, "corner_attack_stage9_random_turntyp_0");
    check::eq_i32(c4.attack, 4, "corner_attack_stage9_touch_decrement_4");
    check::eq_i32(m4.draws(r), 9, "corner_attack_stage9_nine_draws");

    // K5: power 20 makes the raw saftey 23, clamped to 20 (:191-193)
    let mut m5 = Preset::new(r);
    let mut md5 = mad(0);
    md5.power = 20.0;
    let mut p5 = cp();
    p5.stage = 2;
    let mut c5 = Control::new();
    plan(&mut c5, &mut md5, &p5, &t, &car, &mut m5.m);
    check::eq_i32(c5.saftey, 20, "corner_low_power_saftey_clamped_20");
    check::eq_i32(c5.rampp, 1, "corner_low_power_rampp_1");

    // K6: power 40 takes the <=50 arm and subtracts 0.5 from mustland
    // (:228-230); the stage-2 base 0.7 - 0.2 - 0.5 == 0.0
    let mut m6 = Preset::new(r);
    let mut md6 = mad(0);
    md6.power = 40.0;
    let mut p6 = cp();
    p6.stage = 2;
    let mut c6 = Control::new();
    plan(&mut c6, &mut md6, &p6, &t, &car, &mut m6.m);
    check::eq_i32(
        bits(c6.mustland),
        0,
        "corner_power40_mustland_minus_half_zero",
    );
    check::eq_i32(c6.saftey, 17, "corner_power40_saftey_17");

    // K7: xz -90 walks both var10 normalisation loops inside the plan loop
    // (:327-333); the pass still computes stage-2 acuracy 24
    let mut m7 = Preset::new(r);
    let mut md7 = mad(0);
    let mut p7 = cp();
    p7.stage = 2;
    p7.opx[1] = 100;
    let mut c7 = Control::new();
    c7.afta = true;
    car_at(&mut car, 0, 0, 0, -90);
    plan(&mut c7, &mut md7, &p7, &t, &car, &mut m7.m);
    check::eq_i32(c7.acuracy, 24, "corner_xz_negative_plan_acuracy_24");
    check::eq_i32(c7.clrnce, 5, "corner_xz_negative_clrnce_5");
    check::eq_i32(m7.draws(r), 12, "corner_xz_negative_twelve_draws");
    car_at(&mut car, 0, 0, 0, 0);

    // K8: the bulistc -4 walk wraps backwards from index 3 to 4 (:540-553)
    let mut m8 = Preset::new(r);
    let mut md8 = mad(0);
    let mut p8 = cp_bul();
    p8.typ[0] = -4;
    p8.typ[1] = -4;
    p8.typ[2] = -4;
    p8.typ[3] = -4;
    let mut c8 = Control::new();
    c8.bulistc = true;
    c8.trfix = 1;
    touch(&mut c8, &mut md8, &p8, &car, &mut m8.m);
    check::ok(md8.nofocus, "corner_bulistc_minus4_wrap_nofocus");

    // K9: rampp 1 wraps point 4 -> 0 before reading the -2 flag (:558-565)
    let mut m9 = Preset::new(r);
    let mut md9 = mad(0);
    md9.point = 4;
    let mut p9 = cp_bul();
    p9.typ[4] = 0;
    p9.typ[0] = -2;
    p9.x[0] = -100;
    let mut c9 = Control::new();
    c9.rampp = 1;
    c9.turncnt = 1;
    touch(&mut c9, &mut md9, &p9, &car, &mut m9.m);
    check::eq_i32(c9.pan, 90, "corner_rampp1_wraps_to_index0");

    // K10: rampp -1 wraps point 4 -> 0 (:569-571)
    let mut m10 = Preset::new(r);
    let mut md10 = mad(0);
    md10.point = 4;
    let mut p10 = cp_bul();
    p10.typ[4] = -2;
    p10.x[0] = -100;
    let mut c10 = Control::new();
    c10.rampp = -1;
    c10.turncnt = 1;
    touch(&mut c10, &mut md10, &p10, &car, &mut m10.m);
    check::eq_i32(c10.pan, 90, "corner_ramppminus1_wraps_to_index0");

    // K11: the -1 skip walk wraps point 4 -> 0 (:576-580)
    let mut m11 = Preset::with_start(r, 1); // 0.0, 0.9, ...
    let mut md11 = mad(0);
    md11.point = 4;
    let mut p11 = cp_bul();
    p11.typ[4] = -1;
    p11.x[0] = -100;
    let mut c11 = Control::new();
    c11.skiplev = 0.4;
    c11.turncnt = 1;
    touch(&mut c11, &mut md11, &p11, &car, &mut m11.m);
    check::eq_i32(c11.pan, 90, "corner_skip_walk_wraps_to_index0");
    check::eq_i32(m11.draws(r), 3, "corner_skip_walk_three_draws");

    // K12: zyinv adds 180 to the steering var25 (:692-695), making the
    // difference exactly 180 -> the second branch turns left
    let mut m12 = Preset::new(r);
    let mut md12 = mad(0);
    let mut c12 = Control::new();
    c12.zyinv = true;
    c12.upcnt = 30;
    touch(&mut c12, &mut md12, &cp(), &car, &mut m12.m);
    check::ok(c12.left, "corner_steer_zyinv_left");
    check::ok(c12.lastl, "corner_steer_zyinv_lastl");
    check::ok(!c12.right, "corner_steer_zyinv_right_false");

    // K13: diff 340 uses the second branch and picks right (:742-749); the
    // 340 < 310 actuation guard stays closed
    let mut m13 = Preset::new(r);
    let mut md13 = mad(0);
    let mut c13 = Control::new();
    c13.pan = 170;
    c13.upcnt = 30;
    car_at(&mut car, 0, 0, 0, -170);
    touch(&mut c13, &mut md13, &cp(), &car, &mut m13.m);
    check::eq_i32(c13.pan, 170, "corner_second_branch_pan_kept");
    check::ok(c13.right, "corner_second_branch_right");
    check::ok(!c13.lastl, "corner_second_branch_lastl_false");
    check::ok(
        !c13.up && !c13.down && !c13.handb,
        "corner_second_branch_no_actuation",
    );

    // K14: diff 270 is inside the second branch's actuation window: turntyp 1
    // -> down and up forced off
    let mut m14 = Preset::new(r);
    let mut md14 = mad(0);
    md14.speed = 100.0;
    let mut c14 = Control::new();
    c14.pan = 170;
    c14.turntyp = 1;
    c14.upcnt = 30;
    car_at(&mut car, 0, 0, 0, -100);
    touch(&mut c14, &mut md14, &cp(), &car, &mut m14.m);
    check::ok(c14.right, "corner_second_branch_tt1_right");
    check::ok(c14.down, "corner_second_branch_tt1_down");
    check::ok(!c14.up, "corner_second_branch_tt1_up_false");
    // and turntyp 2 -> handb
    let mut m14b = Preset::new(r);
    let mut md14b = mad(0);
    md14b.speed = 100.0;
    let mut c14b = Control::new();
    c14b.pan = 170;
    c14b.turntyp = 2;
    c14b.upcnt = 30;
    car_at(&mut car, 0, 0, 0, -100);
    touch(&mut c14b, &mut md14b, &cp(), &car, &mut m14b.m);
    check::ok(c14b.right, "corner_second_branch_tt2_right");
    check::ok(c14b.handb, "corner_second_branch_tt2_handb");
    check::ok(!c14b.up, "corner_second_branch_tt2_up_false");
    car_at(&mut car, 0, 0, 0, 0);

    // K15/K16: the lrswt/udswt flips in the other direction (:891-899,910-918)
    let mut m15 = Preset::new(r);
    let mut md15 = mad(0);
    md15.pxy = 200;
    let mut c15 = Control::new();
    c15.trickfase = k.intv("trickfase_air");
    c15.lrdirect = 1;
    c15.lrswt = true;
    c15.oxy = 0;
    c15.lrstart = 0;
    c15.preform(&mut md15, &car, &cp(), &Trackers::new(), &mut m15.m);
    check::eq_i32(c15.lrdirect, -1, "corner_lrswt_flips_1_to_minus1");
    check::ok(c15.left, "corner_lrswt_flip_left");
    let mut m16 = Preset::new(r);
    let mut md16 = mad(0);
    md16.pzy = 200;
    let mut c16_ = Control::new();
    c16_.trickfase = k.intv("trickfase_air");
    c16_.uddirect = 1;
    c16_.udswt = true;
    c16_.ozy = 0;
    c16_.udstart = 0;
    c16_.preform(&mut md16, &car, &cp(), &Trackers::new(), &mut m16.m);
    check::eq_i32(c16_.uddirect, -1, "corner_udswt_flips_1_to_minus1");
    check::ok(c16_.down, "corner_udswt_flip_down");

    // K17: swat 0 turns a non-zero lcomp into lrbare (:950-952)
    let mut m17 = Preset::new(r);
    let mut md17 = mad(0);
    md17.wtouch = true;
    md17.lcomp = 1.0;
    let mut c17 = Control::new();
    c17.trickfase = k.intv("trickfase_ground");
    c17.preform(&mut md17, &car, &cp(), &Trackers::new(), &mut m17.m);
    check::ok(!c17.udbare, "corner_swat0_lcomp_udbare_false");
    check::ok(c17.lrbare, "corner_swat0_lcomp_lrbare_true");
    check::eq_i32(c17.swat, 2, "corner_swat0_lcomp_swat2");

    // K18: the capsized swap's other direction (:966-969)
    let mut m18 = Preset::with_start(r, 7); // 0.6 > mustland
    let mut md18 = mad(0);
    md18.capsized = true;
    md18.dcomp = 1.0;
    let mut c18 = Control::new();
    c18.trickfase = k.intv("trickfase_ground");
    c18.swat = 2;
    c18.lrbare = true;
    c18.preform(&mut md18, &car, &cp(), &Trackers::new(), &mut m18.m);
    check::ok(c18.udbare, "corner_swat2_swap_lrbare_to_udbare");
    check::ok(!c18.lrbare, "corner_swat2_swap_lrbare_false");
    check::eq_i32(c18.swat, 3, "corner_swat2_swap_swat3");

    // K19: non-capsized + udcomp + perfection, var27 0 -> down (:1003-1009)
    let mut m19 = Preset::new(r);
    let mut md19 = mad(0);
    md19.dcomp = 1.0;
    md19.pzy = -90;
    let mut c19 = Control::new();
    c19.trickfase = k.intv("trickfase_ground");
    c19.swat = 3;
    c19.udbare = true;
    c19.udcomp = true;
    c19.perfection = true;
    c19.preform(&mut md19, &car, &cp(), &Trackers::new(), &mut m19.m);
    check::ok(c19.down, "corner_perfection_var27_0_down");
    check::ok(!c19.up, "corner_perfection_var27_0_up_false");

    // K20: capsized + !udcomp + !onceu with dcomp<=ucomp -> up (:1017-1026)
    let mut m20 = Preset::new(r);
    let mut md20 = mad(0);
    md20.capsized = true;
    let mut c20 = Control::new();
    c20.trickfase = k.intv("trickfase_ground");
    c20.swat = 3;
    c20.udbare = true;
    c20.preform(&mut md20, &car, &cp(), &Trackers::new(), &mut m20.m);
    check::ok(c20.up, "corner_capsized_else_up_true");
    check::ok(!c20.down, "corner_capsized_else_down_false");

    // K21: non-capsized + udcomp + perfection var27 0 in the dcomp>ucomp
    // branch -> down (:1028-1035)
    let mut m21 = Preset::new(r);
    let mut md21 = mad(0);
    md21.dcomp = 1.0;
    md21.pzy = -90;
    let mut c21 = Control::new();
    c21.trickfase = k.intv("trickfase_ground");
    c21.swat = 3;
    c21.udbare = true;
    c21.udcomp = true;
    c21.perfection = true;
    c21.preform(&mut md21, &car, &cp(), &Trackers::new(), &mut m21.m);
    check::ok(c21.down, "corner_perfection_dcomp_upcomp_down");
    check::ok(!c21.up, "corner_perfection_dcomp_upcomp_up_false");

    // K22: non-capsized + !udcomp with dcomp<=ucomp, draw > mustland -> down
    // and onceu is set (:1036-1040)
    let mut m22 = Preset::with_start(r, 7);
    let mut md22 = mad(0);
    md22.ucomp = 1.0;
    let mut c22 = Control::new();
    c22.trickfase = k.intv("trickfase_ground");
    c22.swat = 3;
    c22.udbare = true;
    c22.preform(&mut md22, &car, &cp(), &Trackers::new(), &mut m22.m);
    check::ok(c22.down, "corner_random_udcomp_else_down_true");
    check::ok(!c22.up, "corner_random_udcomp_else_up_false");
    check::ok(c22.onceu, "corner_random_udcomp_else_onceu_true");

    // K23: var29 -90 walks BOTH normalisation loops (:1050-1056)
    let mut m23 = Preset::new(r);
    let mut md23 = mad(0);
    md23.capsized = true;
    md23.lcomp = 10.0;
    md23.pxy = -180;
    let mut c23 = Control::new();
    c23.trickfase = k.intv("trickfase_ground");
    c23.swat = 3;
    c23.lrbare = true;
    c23.preform(&mut md23, &car, &cp(), &Trackers::new(), &mut m23.m);
    check::ok(c23.left, "corner_lr_var29_wrap_left");

    // K24: non-capsized + lcomp<=rcomp + lrcomp + perfection var29 0 ->
    // right (:1099-1107)
    let mut m24 = Preset::new(r);
    let mut md24 = mad(0);
    md24.rcomp = 1.0;
    md24.pxy = -90;
    let mut c24 = Control::new();
    c24.trickfase = k.intv("trickfase_ground");
    c24.swat = 3;
    c24.lrbare = true;
    c24.lrcomp = true;
    c24.perfection = true;
    c24.preform(&mut md24, &car, &cp(), &Trackers::new(), &mut m24.m);
    check::ok(c24.right, "corner_perfection_lr_else_right");
    check::ok(!c24.left, "corner_perfection_lr_else_left_false");

    // K25/K26: capsized lcomp<=rcomp with lrcomp, var29 135 -> left and
    // var29 0 -> right (:1089-1095)
    let mut m25 = Preset::new(r);
    let mut md25 = mad(0);
    md25.capsized = true;
    md25.rcomp = 1.0;
    md25.pxy = 45;
    let mut c25 = Control::new();
    c25.trickfase = k.intv("trickfase_ground");
    c25.swat = 3;
    c25.lrbare = true;
    c25.lrcomp = true;
    c25.preform(&mut md25, &car, &cp(), &Trackers::new(), &mut m25.m);
    check::ok(c25.left, "corner_capsized_else_lrcomp_var29_135_left");
    check::ok(!c25.right, "corner_capsized_else_lrcomp_right_false");
    let mut m26 = Preset::new(r);
    let mut md26 = mad(0);
    md26.capsized = true;
    md26.rcomp = 1.0;
    md26.pxy = -90;
    let mut c26 = Control::new();
    c26.trickfase = k.intv("trickfase_ground");
    c26.swat = 3;
    c26.lrbare = true;
    c26.lrcomp = true;
    c26.preform(&mut md26, &car, &cp(), &Trackers::new(), &mut m26.m);
    check::ok(c26.right, "corner_capsized_else_lrcomp_var29_0_right");
    check::ok(!c26.left, "corner_capsized_else_lrcomp_left_false");

    // K27: |var10-var12| > 180 folds back through :353-355; the resulting
    // var13 20 still lets the :438 clearing branch fire
    let mut m27 = Preset::new(r);
    let mut md27 = mad(0);
    let mut p27 = cp();
    p27.stage = 2;
    p27.pos[0] = 4;
    p27.opx[1] = 100;
    p27.opz[1] = -567;
    let mut c27 = Control::new();
    c27.afta = true;
    car_at(&mut car, 0, 0, 0, 170);
    plan(&mut c27, &mut md27, &p27, &t, &car, &mut m27.m);
    check::eq_i32(c27.clrnce, 0, "corner_var13_wrap_clrnce_0");
    check::eq_i32(c27.acuracy, 0, "corner_var13_wrap_acuracy_0");
    check::eq_i32(m27.draws(r), 12, "corner_var13_wrap_twelve_draws");
    car_at(&mut car, 0, 0, 0, 0);

    // K28: a point>0 start runs the initial positive-count scan (:585-595)
    let mut m28 = Preset::new(r);
    let mut md28 = mad(0);
    md28.point = 2;
    md28.clear = 2;
    let mut p28 = cp_bul();
    p28.x[2] = -100;
    let mut c28 = Control::new();
    c28.skiplev = -1.0;
    c28.turncnt = 1;
    touch(&mut c28, &mut md28, &p28, &car, &mut m28.m);
    check::eq_i32(c28.pan, 90, "corner_initial_count_scan_keeps_point_2");
    check::eq_i32(m28.draws(r), 2, "corner_initial_count_scan_two_draws");

    // K29: rampp 2 underflow - the skip walk wraps point 4 to 0, so the
    // var23 != point guard passes and --var23 wraps back to 4 (:618-629)
    let mut m29 = Preset::with_start(r, 1); // 0.0, 0.9, ...
    let mut md29 = mad(0);
    md29.point = 4;
    let mut p29 = cp_bul();
    p29.typ[4] = -1;
    p29.typ[1] = -2;
    p29.x[4] = -100;
    let mut c29 = Control::new();
    c29.rampp = 2;
    c29.skiplev = 0.4;
    c29.turncnt = 1;
    touch(&mut c29, &mut md29, &p29, &car, &mut m29.m);
    check::eq_i32(c29.pan, 90, "corner_rampp2_underflow_wraps_to_index4");
    check::eq_i32(m29.draws(r), 3, "corner_rampp2_underflow_three_draws");
}

// -------------------------------------------------------------------- errors

/// `TControl.errorPins` (`:2799-2890`, B7(76-81)): the missing bounds/null
/// guards, documented not fixed.
fn error_pins(r: &Tml) {
    let car = car_seam();
    let t = Trackers::new();

    // B7(76): pos[5] on the plan gate
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut m = Preset::new(r);
            let mut md = mad(5);
            md.mtouch = true;
            let mut c = Control::new();
            plan(&mut c, &mut md, &cp(), &t, &car, &mut m.m);
        }),
        "index out of bounds",
        "err_im5_pos5_aioobe",
    );

    // B7(77): maxmag[10] in the perfection ratio
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut m = Preset::new(r);
            let mut md = mad(0);
            md.mtouch = true;
            md.cn = 10;
            let mut c = Control::new();
            plan(&mut c, &mut md, &cp(), &t, &car, &mut m.m);
        }),
        "index out of bounds",
        "err_cn10_maxmag10_aioobe",
    );

    // B7(78): the first m.random() of the plan (:134). The port's
    // `&mut Medium` cannot be null — structural.
    check::ok(true, "err_null_medium_first_random_npe");

    // B7(79a): CheckPoints null at :74 — `&CheckPoints` cannot be null.
    check::ok(true, "err_null_checkpoints_npe");

    // B7(79b): ContO null at var2.xz (:321-322) when attack==0 — `&Conto`
    // cannot be null.
    check::ok(true, "err_null_conto_xz_npe");

    // B7(79c): Trackers null at the wall read (:783) — `&Trackers` cannot be
    // null.
    check::ok(true, "err_null_trackers_dam_npe");

    // B7(80): dam[wall] bounds - 500 and -2 both escape the 500-entry array
    for wall in [500, -2] {
        check::throws_it(
            AssertUnwindSafe(|| {
                let mut m = Preset::new(r);
                let mut md = mad(0);
                md.mtouch = true;
                let mut c = Control::new();
                c.wall = wall;
                c.preform(&mut md, &car, &cp(), &Trackers::new(), &mut m.m);
            }),
            "index out of bounds",
            &format!("err_wall{wall}_dam_aioobe"),
        );
    }

    // B7(81): reset's second loop writes fpnt[5]
    check::throws_it(
        AssertUnwindSafe(|| {
            let mut p = CheckPoints::new();
            p.fn_ = 6;
            p.n = 1;
            Control::new().reset(&p);
        }),
        "index out of bounds",
        "err_reset_fn6_fpnt5_aioobe",
    );
}

// -------------------------------------------------------------------- input

/// `TControl.inputPins` (`:2896-2992`, B7(82-87)): the thin key/mouse
/// handlers (`GameSparker.java:266-308,1519-1578`). The port's seams are
/// `Shell::key_down`/`key_up` over `Game.ctl[0]` and `Shell::mouse_down`; the
/// `java.awt.Event` collapses to its code argument.
fn input_pins() {
    let mut g = Shell::new();
    let mut game = Game::new(Vec::new());

    // Java's declared `exwist = true` is the PRE-run state; GameSparker.java:
    // :644 flips it false at the top of run(), and the port constructs the
    // shell in that post-start state (shell.rs:127). The pin records the
    // port's initializer; pin 84 re-arms `true` for the pre-run fixtures.
    check::ok(!g.exwist, "input_default_exwist_true");
    // `u[0] == null`: the port's Game holds live Control slots (no nullable
    // array) — structural.
    check::ok(true, "input_default_u0_null");
    check::eq_i32(game.view as i32, 0, "input_default_view_0");
    check::eq_i32(g.mouses, 0, "input_default_mouses_0");
    // `gamer == null` (no thread): the port's Shell has no thread field —
    // structural.
    check::ok(true, "input_no_thread_started");

    // B7(84): the default exwist==true makes both handlers no-ops. Java's
    // `keyDown` returns false (an AWT consume contract); the port's handler
    // is void, so the return pins are structural.
    g.exwist = true;
    g.key_down(&mut game, 1004);
    check::ok(true, "input_keydown_returns_false");
    check::ok(!game.ctl[0].up, "input_exwist_true_keydown_noop");
    game.ctl[0].up = true;
    g.key_up(&mut game, 1004);
    check::ok(true, "input_keyup_returns_false");
    check::ok(game.ctl[0].up, "input_exwist_true_keyup_noop");
    game.ctl[0].up = false;

    // B7(82): exwist false -> the five drive flags
    g.exwist = false;
    g.key_down(&mut game, 1004);
    check::ok(true, "input_keydown_1004_false");
    check::ok(game.ctl[0].up, "input_keydown_1004_up");
    g.key_down(&mut game, 1005);
    check::ok(game.ctl[0].down, "input_keydown_1005_down");
    g.key_down(&mut game, 1007);
    check::ok(game.ctl[0].right, "input_keydown_1007_right");
    g.key_down(&mut game, 1006);
    check::ok(game.ctl[0].left, "input_keydown_1006_left");
    g.key_down(&mut game, 32);
    check::ok(game.ctl[0].handb, "input_keydown_32_handb");

    // B7(83): keyUp clears the same five
    g.key_up(&mut game, 1004);
    check::ok(true, "input_keyup_1004_false");
    check::ok(!game.ctl[0].up, "input_keyup_1004_up_cleared");
    g.key_up(&mut game, 1005);
    check::ok(!game.ctl[0].down, "input_keyup_1005_down_cleared");
    g.key_up(&mut game, 1007);
    check::ok(!game.ctl[0].right, "input_keyup_1007_right_cleared");
    g.key_up(&mut game, 1006);
    check::ok(!game.ctl[0].left, "input_keyup_1006_left_cleared");
    g.key_up(&mut game, 32);
    check::ok(!game.ctl[0].handb, "input_keyup_32_handb_cleared");

    // B7(85): enter keys set enter; keyUp never clears it
    g.key_down(&mut game, 10);
    check::ok(game.ctl[0].enter, "input_keydown_10_enter");
    g.key_up(&mut game, 10);
    check::ok(game.ctl[0].enter, "input_keyup_10_does_not_clear_enter");
    game.ctl[0].enter = false;
    g.key_down(&mut game, 80);
    check::ok(game.ctl[0].enter, "input_keydown_80_enter");
    game.ctl[0].enter = false;
    g.key_down(&mut game, 112);
    check::ok(game.ctl[0].enter, "input_keydown_112_enter");
    game.ctl[0].enter = false;
    g.key_down(&mut game, 27);
    check::ok(game.ctl[0].enter, "input_keydown_27_enter");

    // B7(86): toggles and the view wrap
    check::ok(!game.ctl[0].arrace, "input_arrace_starts_false");
    g.key_down(&mut game, 65);
    check::ok(game.ctl[0].arrace, "input_keydown_65_arrace_toggle_on");
    g.key_down(&mut game, 97);
    check::ok(!game.ctl[0].arrace, "input_keydown_97_arrace_toggle_off");
    g.key_down(&mut game, 77);
    check::ok(game.ctl[0].mutem, "input_keydown_77_mutem_toggle_on");
    g.key_down(&mut game, 109);
    check::ok(!game.ctl[0].mutem, "input_keydown_109_mutem_toggle_off");
    g.key_down(&mut game, 78);
    check::ok(game.ctl[0].mutes, "input_keydown_78_mutes_toggle_on");
    g.key_down(&mut game, 110);
    check::ok(!game.ctl[0].mutes, "input_keydown_110_mutes_toggle_off");
    game.view = 2;
    g.key_down(&mut game, 118);
    check::eq_i32(game.view as i32, 0, "input_keydown_118_view_3_wraps_0");
    g.key_down(&mut game, 86);
    check::eq_i32(game.view as i32, 1, "input_keydown_86_view_increments_1");

    // B7(87): mouseDown latches the first press and touches no Control field
    let before = snapshot(&game.ctl[0]);
    g.mouses = 0;
    g.mouse_down(12, 34);
    check::eq_i32(g.mouses, 1, "input_mousedown_mouses_1");
    check::eq_i32(g.xm, 12, "input_mousedown_xm_12");
    check::eq_i32(g.ym, 34, "input_mousedown_ym_34");
    g.mouse_down(99, 99);
    check::eq_i32(g.mouses, 1, "input_mousedown_second_ignored_mouses");
    check::eq_i32(g.xm, 12, "input_mousedown_second_ignored_xm");
    check::eq_i32(g.ym, 34, "input_mousedown_second_ignored_ym");
    check::eq_str(
        snapshot(&game.ctl[0]),
        before,
        "input_mousedown_touches_no_control_field",
    );
    g.exwist = true;
    g.mouses = 0;
    g.mouse_down(55, 66);
    check::eq_i32(g.mouses, 0, "input_mousedown_exwist_noop");
}
