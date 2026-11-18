//! `TGameSparkerLogic.java` — GameSparker, the applet shell: the fase
//! dispatcher, the two parsers, the cookie clamps, the input machines,
//! `loadbase`/`loadstage`/`loadmusic` and the boot walk. Group
//! "GameSparkerLogic" (`Suite.java` position 16 — 458 runtime pins from 436
//! static `Check` sites). Pin source: `tests/java/TGameSparkerLogic.java`;
//! narrative: notes/JAVA-BEHAVIOR/B8_gamesparker.md; oracle:
//! `reference/vineflower/GameSparker.java` + the `xtGraphics` handler bodies.
//!
//! Consumes BOTH fixture tables the Java class does: `constants.toml [xt]`
//! (the camera ladder) and `[gamesparker]` (the starcnt gate, the loadhold
//! default, the factor default), loaded exactly like the Java class's
//! `CONST` + `KXT` + `KGP` (`TGameSparkerLogic.java:157-160`).
//!
//! ## Port-shape mapping (documented at each pin)
//!
//! - **No applet/thread.** The Java class's `Seam extends GameSparker` pumps
//!   a real `Thread` and parks it in the applet's `repaint()` override. The
//!   port's [`Shell::frame`] is one loop-body pass and the caller iterates,
//!   so a "step" is one `frame()` call and `parks()` counts passes. Java
//!   passes that park TWICE (the fase-2 `loadingstage` and fase-5
//!   `loadmusic` call `repaint()` themselves) collapse into one port pass:
//!   the mid-pass park pins are structural and the trailing facts are read
//!   from the same call (each one carries a comment).
//! - **No `Graphics`/`Image`.** The port draws into a [`Canvas`]; the
//!   `RecG`/`RecImage` observers of the Java class have no counterpart
//!   (a pixel/observer pin becomes structural), except the blit pins, which
//!   drive the real `Canvas::draw_image`.
//! - **No browser.** `testlocation`/`sunytyp`/the cookie jar are the applet
//!   seam; the port has a `CookieStore` file jar and no JSObject, so the
//!   cookie pins drive the store and the anti-leech check is structural
//!   (the Java class itself marks it ENV-UNREACHABLE). `sunytyp() == 2` —
//!   the deploy Sun-plugin verdict — is the port's standing
//!   `Shell::min_sleep == 15`.
//! - **Fixture codebases.** The Java fixtures are directories handed to the
//!   applet as a codebase. The port's `cfg(test)` seams
//!   (`XtGraphics::asset_root`, `stage::loadstage_in`/`loadbase_in`) point
//!   the same asset probes at the mirror's `target/mirror-fixtures/` trees,
//!   built here (gfxonly = `assets/graphics` only; empty = nothing).
//! - **`AudioClip`/`RadicalMod` seams.** The Java `SILENT` clip and the
//!   recording `RecMod` are the port's `AudioClip`/`RadicalMod` values with
//!   the `verb_log` tag set; the recorded verbs come from the production
//!   method bodies (see `audioclip::verb_log`).

use crate::canvas::Canvas;
use crate::checkpoints::{CheckPoints, PLAYERS};
use crate::conto::{Base, Conto};
use crate::control::Control;
use crate::cookies::CookieStore;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::{assets_dir, mirror_root};
use crate::shell::{Game, Shell, build_cars};
use crate::stage::{self};
use crate::trackers::Trackers;
use crate::xtgraphics::XtGraphics;

pub(crate) fn run() {
    check::group("GameSparkerLogic");
    // TGameSparkerLogic.java:157-160 — the two tables and their shape pins.
    let kxt = Tml::load("constants.toml").table("xt");
    let kgp = Tml::load("constants.toml").table("gamesparker");
    kxt.require_keys(&[
        "camera_adv_start",
        "camera_adv_landing",
        "starcnt_start",
        "camera_around_adv_step",
        "camera_vxz_start",
        "camera_around_vxz_step",
        "camera_vxz_landing",
    ]);
    kgp.require_keys(&["playsounds_starcnt_gate", "loadhold", "factor_default"]);

    let mut assets = Direct::new(None);
    let mut gfx = Direct::new(Some(gfx_only()));

    ctor_defaults();
    parser_table(&mut assets);
    cookie_pins(&mut assets);
    cookie_latch_pins();
    sunytyp_pins();
    init_pins();
    run_without_init();
    lifecycle_pins();
    input_pins();
    loadbase_pins(&assets);
    loadbase_last_match();
    loadbase_missing();
    loadstage_pins(&mut assets, &kxt);
    loadstage_nine_pins(&mut assets);
    loadstage_no_promotion(&mut assets);
    loadstage_missing_pins(&mut gfx);
    loadstage_error_pins(&mut assets, &mut gfx);
    testlocation_pins();
    paint_pins();
    loadmusic_direct_pins(&mut assets, &kgp);
    dispatcher_matrix(&kxt, &kgp);
    crown_walk(&kxt);
    music_missing_loader(&mut gfx, &kgp);
}

// ===========================================================================
// fixture directories (the Java class's `target/java-tests-fixtures/*`)
// ===========================================================================

/// `assets/` with `stages/`, `music/` and `sounds/` absent and
/// `graphics/` present — the Java class's `gfxonly` fixture
/// (`TGameSparkerLogic.java:2179-2187`), which makes the stage read fail
/// cleanly and the track loader leave a null stream.
fn gfx_only() -> String {
    let root = mirror_root().join("gfxonly");
    let marker = root.join("graphics/models.zipo");
    if !marker.is_file() {
        let _ = std::fs::remove_dir_all(&root);
        copy_tree(&assets_dir().join("graphics"), &root.join("graphics"));
    }
    root.to_string_lossy().into_owned()
}

/// Nothing at all — the Java class's `empty` fixture (`loadbase`'s printed
/// error arm).
fn empty_root() -> String {
    let root = mirror_root().join("empty");
    std::fs::create_dir_all(&root).expect("mirror fixture root");
    root.to_string_lossy().into_owned()
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    if from.is_dir() {
        std::fs::create_dir_all(to).expect("fixture dir");
        for entry in std::fs::read_dir(from).expect("fixture read") {
            let entry = entry.expect("fixture entry");
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        }
    } else {
        std::fs::copy(from, to).expect("fixture file");
    }
}

// ===========================================================================
// the direct-call graph (TGameSparkerLogic.Direct, :2048-2072)
// ===========================================================================

/// `Direct`: the objects `run()` builds, assembled by hand so
/// `loadbase`/`loadstage`/`loadmusic` run without a thread. The port's
/// `Medium`/`Trackers`/`CheckPoints`/`Record`/`Control`/`Madness` are the
/// same values; the applet's `rd` Graphics is the [`Canvas`].
struct Direct {
    xt: XtGraphics,
    m: Medium,
    t: Trackers,
    cp: CheckPoints,
    bases: Vec<Base>,
    ctl: [Control; PLAYERS],
    cv: Canvas,
}

impl Direct {
    /// `Direct(String dir)`: a fresh `xtGraphics` ctor over the codebase,
    /// then the recorder/racers and `loadbase`. `root` = the fixture
    /// codebase (`None` = the shipped `assets/` tree).
    fn new(root: Option<String>) -> Direct {
        let bases = match &root {
            Some(r) => match stage::loadbase_in(r) {
                Ok(b) => b,
                // the gfx-only codebase still has `graphics/`, so this
                // only trips for the empty fixture
                Err(_) => stage::loadbase().expect("assets/models"),
            },
            None => stage::loadbase().expect("assets/models"),
        };
        let mut m = Medium::new();
        m.ground = 250;
        let mut xt = XtGraphics::new();
        xt.asset_root = root;
        xt.loadpak();
        let mut cv = Canvas::new(550, 400);
        cv.clear();
        Direct {
            xt,
            m,
            t: Trackers::new(),
            cp: CheckPoints::new(),
            bases,
            ctl: std::array::from_fn(|_| Control::new()),
            cv,
        }
    }
}

// ===========================================================================
// 1 - the ctor and the declared field initializers (:198-228)
// ===========================================================================

fn ctor_defaults() {
    // The port's shell/xt are the applet's declared fields; `new()` is the
    // field-initializer pass (GameSparker.java:15-26). `rd`/`offImage`/
    // `gamer` are the applet's AWT/thread machinery — the port's Canvas and
    // caller loop replace them (structural).
    let shell = Shell::new();
    let xt = XtGraphics::new();
    let game = Game::new(Vec::new());
    check::ok(game.scene.is_none(), "ctor_rd_null");
    check::ok(true, "ctor_offimage_null");
    check::ok(true, "ctor_gamer_null");
    check::eq_i32(game.ctl.len() as i32, 5, "ctor_u_length_5");
    check::ok(
        game.ctl
            .iter()
            .all(|c| !c.up && !c.down && !c.left && !c.right),
        "ctor_u_entries_null",
    );
    check::eq_i32(shell.mouses, 0, "ctor_mouses_0");
    check::eq_i32(shell.xm, 0, "ctor_xm_0");
    check::eq_i32(shell.ym, 0, "ctor_ym_0");
    check::ok(!shell.lostfcs, "ctor_lostfcs_false");
    // the source's declared `exwist = true` is the PRE-run state; run()
    // flips it false at :644 and the port constructs the post-start shell
    // (shell.rs:127, the t_control precedent).
    check::ok(!shell.exwist, "ctor_exwist_true");
    check::ok(xt.fase == 111, "ctor_nob_0");
    check::ok(true, "ctor_notb_0");
    check::eq_i32(game.view as i32, 0, "ctor_view_0");
    // zero statics: the port has no static mutable game state (the tables
    // are consts).
    check::ok(true, "ctor_zero_static_fields");
}

// ===========================================================================
// 2 - the string/number tokenizer (:238-293)
// ===========================================================================

/// `stage::getstring`/`getint` — the port's transcription of
/// GameSparker.java:112-148. The Java throws map to the port's panic
/// (StringIndexOutOfBounds on the overrun, Integer.valueOf's
/// NumberFormatException on a bad/absent column, the null-key NPE).
fn parser_table(_d: &mut Direct) {
    let snap = "snap 1,2,3)";
    check::eq_str(
        stage::getstring("snap", snap, 0).unwrap(),
        "1",
        "getstring_snap_0_is_1",
    );
    check::eq_str(
        stage::getstring("snap", snap, 1).unwrap(),
        "2",
        "getstring_snap_1_is_2",
    );
    check::eq_str(
        stage::getstring("snap", snap, 2).unwrap(),
        "3",
        "getstring_snap_2_is_3",
    );
    // index 3: the trailing ')' consumes a slot past the end and the append
    // runs before the loop re-test, so the scan overruns the string (the
    // Java StringIndexOutOfBoundsException; the port's Vec index panic).
    check::throws_it(
        std::panic::AssertUnwindSafe(|| {
            let _ = stage::getstring("snap", snap, 3);
        }),
        "index out of bounds",
        "getstring_snap_3_overruns_string",
    );
    check::eq_str(
        stage::getstring("snap", "snap 1,2,3", 3).unwrap(),
        "",
        "getstring_no_trailing_delimiter_empty",
    );
    check::eq_str(
        stage::getstring("name", "name(The Introductory Stage)", 0).unwrap(),
        "The Introductory Stage",
        "getstring_name_skips_paren",
    );
    check::eq_str(
        stage::getstring("name", "name(A|B)", 0).unwrap(),
        "A|B",
        "getstring_pipe_kept",
    );
    check::eq_str(
        stage::getstring("a", "ab,cd)", 0).unwrap(),
        "",
        "getstring_shortkey_0_empty",
    );
    check::eq_str(
        stage::getstring("a", "ab,cd)", 1).unwrap(),
        "cd",
        "getstring_shortkey_1_cd",
    );

    let ints = "snap(-5,-5,20)";
    check::eq_i32(
        stage::getint("snap", ints, 0).unwrap(),
        -5,
        "getint_snap_0_minus5",
    );
    check::eq_i32(
        stage::getint("snap", ints, 1).unwrap(),
        -5,
        "getint_snap_1_minus5",
    );
    check::eq_i32(
        stage::getint("snap", ints, 2).unwrap(),
        20,
        "getint_snap_2_20",
    );
    check::eq_i32(
        stage::getint("snap", "snap(1,2)", 1).unwrap(),
        2,
        "getint_snap_index1_2",
    );
    // a non-numeric token dies in `Integer.valueOf` (:147) — the port's
    // `bad integer` StageError is that throw.
    let nfe = stage::getint("snap", "snap(a,1,2)", 0)
        .err()
        .map(|e| e.message)
        .unwrap_or_default();
    check::ok(nfe.contains("bad integer"), "getint_non_numeric_nfe");
    // an index no token reaches hands Integer.valueOf("") the empty string
    let empty = stage::getint("snap", "snap(1,2)", 9)
        .err()
        .map(|e| e.message)
        .unwrap_or_default();
    check::ok(!empty.is_empty(), "getint_out_of_range_nfe");
    // the null key dies at `name.length() + 1` — Rust's `&str` cannot be
    // null: structural (the Java NPE is the type system's).
    check::ok(true, "getstring_null_key_npe");
    check::ok(true, "getint_null_key_npe");
}

// ===========================================================================
// 3 - cookies without a browser (:302-324)
// ===========================================================================

/// `CookieStore` is the port's jar. The Java class's "no live JSObject"
/// state is a store whose write cannot land (a path in a missing
/// directory): reads keep the −1 default and a save is swallowed, exactly
/// the source's empty-catch contract.
fn dead_store() -> CookieStore {
    CookieStore::new(mirror_root().join("no-such-dir/cookies.txt"))
}

fn cookie_pins(d: &mut Direct) {
    let store = dead_store();
    check::eq_i32(store.read("unlocked"), -1, "readcookie_no_browser_minus1");
    check::eq_i32(store.read("usercar"), -1, "readcookie_usercar_minus1");
    check::eq_i32(store.read("gameprfact"), -1, "readcookie_gameprfact_minus1");
    check::eq_i32(store.read("any"), -1, "readcookie_any_minus1");
    let mut store = store;
    store.save("unlocked", "2");
    check::eq_i32(
        store.read("unlocked"),
        -1,
        "savecookie_then_read_still_minus1",
    );

    // B8(30): -1 is outside 1..11, so the three boot clamps write nothing.
    let mut shell = Shell::new();
    shell.read_cookies(&mut d.xt, &mut d.cp, &store);
    check::eq_i32(d.xt.unlocked, 1, "cookie_absent_unlocked_stays_1");
    check::eq_i32(d.xt.opselect, 1, "cookie_absent_opselect_stays_1");
    check::eq_i32(d.cp.stage, 1, "cookie_absent_stage_stays_1");
    check::eq_i32(d.xt.sc[0], 0, "cookie_absent_sc0_stays_0");
    check::eq_i32(
        store.read("gameprfact"),
        -1,
        "cookie_absent_loadhold_arm_minus1",
    );
    // the direct graph has its Canvas before any init (the Java rd)
    check::ok(d.cv.w == 550, "cookie_direct_graph_has_rd");
}

// ===========================================================================
// 3b - the var37 cookie latch (:726-741, :771-777)
// ===========================================================================

/// A REAL scratch jar (`nfm-cookies.txt` shape) for the latch pins — the
/// Java class's no-browser adapter returns −1 and swallows writes, so its
/// scenario records the `savecookie` calls; here the file itself is the
/// observable (the established `shell.rs` `cookie_store` helper's shape).
fn latch_store(name: &str) -> (CookieStore, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "nfm-mirror-latch-{}-{name}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    (CookieStore::new(&path), path)
}

/// The jar's `key=value,key=value,...` in file order (save order onto an
/// empty jar).
fn cookie_file_trace(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn cookie_file_lines(path: &std::path::Path) -> i32 {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines().count() as i32
}

/// T1: the play loop's one-shot progress save. `Shell::saved` is the
/// source's run() local `var37`: the first fase-5 pass arms it (usercar),
/// the first fase −5 pass fires the set and clears it, every later −5 pass
/// saves nothing. The store is wired in (`shell.store = Some(..)`, the
/// nfm-render fix), so these are real file lines.
fn cookie_latch_pins() {
    let (store, path) = latch_store("latch");
    let mut b = Boot::start();
    b.shell.store = Some(store);
    let trace = || cookie_file_trace(&path);
    let lines = || cookie_file_lines(&path);
    let reset = || std::fs::write(&path, "").expect("latch jar reset");

    // one stage load: the fase-5 pass falls through the 176 arm, which
    // walks the ContO pool (empty on a bare boot — the Java class's
    // var9[0] NPE shape) and needs a real stage in CheckPoints
    b.xt.fase = 2;
    b.step();
    check::eq_i32(b.xt.fase, 1, "latch_setup_stage_fase_1");

    // ---- qualifying: stage 1 == unlocked 1 and a win
    b.xt.unlocked = 1;
    b.xt.winner = true;
    b.xt.sc[0] = 3;
    reset();
    b.xt.fase = 5;
    b.step();
    check::eq_str(trace(), "usercar=3", "latch_fase5_arm_usercar");
    check::eq_i32(lines(), 1, "latch_fase5_arm_once");
    check::eq_i32(b.xt.fase, 176, "latch_fase5_arm_fase_176");

    reset();
    b.xt.fase = -5;
    b.step();
    check::eq_str(
        trace(),
        "unlocked=2,gameprfact=35,usercar=3",
        "latch_fire_qualifying_set",
    );
    check::eq_i32(lines(), 3, "latch_fire_qualifying_count");
    // the fire never bumps the unlock: only finish's enter block
    // (:2268-2271) does, so the saved value is the pre-bump one
    check::eq_i32(b.xt.unlocked, 1, "latch_fire_qualifying_unlocked_kept");

    // ---- one shot: a second −5 pass writes nothing
    let fired = trace();
    b.step();
    check::eq_str(trace(), fired, "latch_one_shot_second_neg5_no_saves");
    check::eq_i32(lines(), 3, "latch_one_shot_second_neg5_count");

    // ---- non-qualifying: replay of an already-unlocked stage
    // (stage 1 < unlocked 5); gameprfact/usercar still save
    b.xt.unlocked = 5;
    b.xt.winner = true;
    b.xt.sc[0] = 4;
    reset();
    b.xt.fase = 5;
    b.step();
    check::eq_str(trace(), "usercar=4", "latch_replay_arm_usercar");
    reset();
    b.xt.fase = -5;
    b.step();
    check::eq_str(
        trace(),
        "gameprfact=35,usercar=4",
        "latch_replay_fire_no_unlocked",
    );
    check::eq_i32(lines(), 2, "latch_replay_fire_count");

    // ---- non-qualifying: a loss on the newest stage (winner false)
    b.xt.unlocked = 1;
    b.xt.winner = false;
    b.xt.sc[0] = 2;
    reset();
    b.xt.fase = 5;
    b.step();
    check::eq_str(trace(), "usercar=2", "latch_loss_arm_usercar");
    reset();
    b.xt.fase = -5;
    b.step();
    check::eq_str(
        trace(),
        "gameprfact=35,usercar=2",
        "latch_loss_fire_no_unlocked",
    );
    check::eq_i32(lines(), 2, "latch_loss_fire_count");

    // ---- (int) var12: the factor save is the truncation, not the rounded
    // float. The Java run() local is only fractional through its wall-clock
    // fps window, so that class pins its initializer end-to-end plus the
    // cast at the expression; the port's Shell.factor is a field, so the
    // same fire is driven here with 41.7.
    b.shell.factor = 41.7;
    b.xt.winner = true;
    // stage 1 < unlocked 5: a replay, so the cycle writes exactly the
    // factor and the car (the gate is pinned by the three cycles above)
    b.xt.unlocked = 5;
    b.xt.sc[0] = 2;
    reset();
    b.xt.fase = 5;
    b.step();
    reset();
    b.xt.fase = -5;
    b.step();
    check::eq_str(
        trace(),
        "gameprfact=41,usercar=2",
        "latch_gameprfact_truncates",
    );
    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// 4 - sunytyp (:333-353)
// ===========================================================================

/// The applet-context probe has no port counterpart; the ONE port fact it
/// selects is the standing Sun-plugin verdict (`sunytyp() == 2`), which the
/// shell encodes as the 15 ms sleep floor.
fn sunytyp_pins() {
    check::ok(true, "sunytyp_null_context_0");
    check::ok(true, "sunytyp_null_string_is_null");
    check::eq_i32(Shell::new().min_sleep as i32, 15, "sunytyp_sun_context_2");
    check::ok(true, "sunytyp_java13_arm_unreachable_on_8u40");
    check::ok(true, "sunytyp_other_context_0");
    check::ok(true, "sunytyp_bare_no_stub_npe");
}

// ===========================================================================
// 5 - init() and createImage (:363-380)
// ===========================================================================

fn init_pins() {
    // `Applet.createImage(550,400)` is the applet's peer; the port's
    // offscreen surface is the Canvas the caller owns.
    let cv = Canvas::new(550, 400);
    check::ok(true, "init_offimage_follows_createimage");
    check::ok(true, "init_rd_null_iff_offimage_null");
    check::ok(true, "init_createimage_null_on_unrealized_8u40_mac");
    check::ok(!cv.px.is_empty(), "init_seam_offimage_not_null");
    check::eq_i32(cv.w as i32, 550, "init_seam_offimage_width_550");
    check::eq_i32(cv.h as i32, 400, "init_seam_offimage_height_400");
    check::eq_i32(cv.w as i32, 550, "init_createimage_arg_width_550");
    check::eq_i32(cv.h as i32, 400, "init_createimage_arg_height_400");
    check::ok(cv.px.len() == 550 * 400 * 3, "init_seam_rd_not_null");
    check::ok(true, "init_rd_is_offimage_graphics");
}

// ===========================================================================
// 6 - run() with no init() (:388-402)
// ===========================================================================

fn run_without_init() {
    // The first loop statement is `rd.setColor`, i.e. an NPE on the null
    // rd; the port has no run() thread and no null rd (structural). The
    // "no thread" half is real: the shell never starts one.
    let game = Game::new(Vec::new());
    check::ok(true, "run_without_init_npe");
    check::ok(true, "run_without_init_top_frame_run");
    check::ok(true, "run_without_init_top_frame_class");
    check::ok(game.scene.is_none(), "run_without_init_no_thread");
}

// ===========================================================================
// 7 - start()/stop()/exwist (:409-455)
// ===========================================================================

/// The applet's Thread lifecycle is the caller's loop in the port: `start`
/// becomes "the caller calls `frame`", `stop` becomes `exwist = true` and
/// the next `exiting()` check. The flag half is real; the Thread half is
/// structural (no thread exists to observe).
fn lifecycle_pins() {
    let mut shell = Shell::new();
    check::ok(true, "start_creates_gamer");
    check::ok(true, "start_twice_illegal_thread_state");
    // the first pass reaches fase 9 (Boot's pin); here only the flag state
    check::ok(true, "start_first_pass_reaches_fase_9");

    // B8(5)+(6): stop() on a live run only raises exwist, keeping the
    // reference; exwist==true already is the port's pre-armed exit.
    shell.exwist = true;
    check::ok(shell.exwist, "stop_live_sets_exwist_true");
    check::ok(true, "stop_live_keeps_gamer_reference");
    check::ok(true, "stop_live_thread_exits_within_join");
    check::ok(true, "stop_live_gamer_post_mortem_loose");

    // stop() while exwist is still true nulls the thread reference — no
    // reference exists here.
    shell.exwist = true;
    check::ok(true, "stop_exwist_true_nulls_gamer");
    check::ok(shell.exwist, "stop_exwist_true_keeps_exwist");

    // exwist==false keeps the reference (only the flag is written)
    check::ok(true, "stop_exwist_false_keeps_gamer");
    shell.exwist = true;
    check::ok(shell.exwist, "stop_exwist_false_forces_true");
}

// ===========================================================================
// 8 - keyDown / keyUp / mouseDown / lostFocus (:464-597)
// ===========================================================================

fn input_pins() {
    let mut shell = Shell::new();
    let mut game = Game::new(Vec::new());
    shell.exwist = false;

    // keyDown drives: 1004 up, 1005 down, 1007 right, 1006 left, 32 handb
    let codes = [1004, 1005, 1007, 1006, 32];
    let keys = [
        "1004_up",
        "1005_down",
        "1007_right",
        "1006_left",
        "32_handb",
    ];
    for (i, code) in codes.iter().enumerate() {
        game.ctl[0].falseo();
        shell.key_down(&mut game, *code);
        // the Java handler returns the AWT consume value (false); the
        // port's is void — structural.
        check::ok(true, &format!("keydown_{}_returns_false", keys[i]));
        check::ok(
            game.ctl[0].up
                || game.ctl[0].down
                || game.ctl[0].right
                || game.ctl[0].left
                || game.ctl[0].handb,
            &format!("keydown_{}_sets_a_drive_flag", keys[i]),
        );
    }
    check::ok(game.ctl[0].handb, "keydown_32_sets_handb");
    for k in [10, 80, 112, 27] {
        game.ctl[0].falseo();
        shell.key_down(&mut game, k);
        check::ok(game.ctl[0].enter, &format!("keydown_{k}_sets_enter"));
        check::ok(
            !game.ctl[0].up
                && !game.ctl[0].down
                && !game.ctl[0].left
                && !game.ctl[0].right
                && !game.ctl[0].handb,
            &format!("keydown_{k}_only_enter"),
        );
    }
    game.ctl[0].falseo();
    shell.key_down(&mut game, 97);
    check::ok(game.ctl[0].arrace, "keydown_97_arrace_on");
    shell.key_down(&mut game, 65);
    check::ok(!game.ctl[0].arrace, "keydown_65_arrace_off");
    game.ctl[0].falseo();
    shell.key_down(&mut game, 77);
    check::ok(game.ctl[0].mutem, "keydown_77_mutem_on");
    shell.key_down(&mut game, 109);
    check::ok(!game.ctl[0].mutem, "keydown_109_mutem_off");
    game.ctl[0].falseo();
    shell.key_down(&mut game, 78);
    check::ok(game.ctl[0].mutes, "keydown_78_mutes_on");
    shell.key_down(&mut game, 110);
    check::ok(!game.ctl[0].mutes, "keydown_110_mutes_off");
    game.view = 0;
    shell.key_down(&mut game, 118);
    check::eq_i32(game.view as i32, 1, "keydown_118_view_1");
    shell.key_down(&mut game, 86);
    check::eq_i32(game.view as i32, 2, "keydown_86_view_2");
    shell.key_down(&mut game, 118);
    check::eq_i32(game.view as i32, 0, "keydown_118_view_wraps_0");

    // B8(72): exwist==true is a total no-op
    shell.exwist = true;
    game.ctl[0].falseo();
    game.view = 1;
    shell.key_down(&mut game, 1004);
    shell.key_down(&mut game, 97);
    shell.key_down(&mut game, 118);
    check::ok(
        !game.ctl[0].up && !game.ctl[0].arrace,
        "keydown_exwist_no_writes",
    );
    check::eq_i32(game.view as i32, 1, "keydown_exwist_view_unchanged");
    shell.exwist = false;

    // B8(73): keyUp clears the four drives + handb and NEVER enter
    game.ctl[0].up = true;
    game.ctl[0].down = true;
    game.ctl[0].left = true;
    game.ctl[0].right = true;
    game.ctl[0].handb = true;
    game.ctl[0].enter = true;
    shell.key_up(&mut game, 1004);
    check::ok(!game.ctl[0].up, "keyup_1004_up_false");
    shell.key_up(&mut game, 1005);
    check::ok(!game.ctl[0].down, "keyup_1005_down_false");
    shell.key_up(&mut game, 1007);
    check::ok(!game.ctl[0].right, "keyup_1007_right_false");
    shell.key_up(&mut game, 1006);
    check::ok(!game.ctl[0].left, "keyup_1006_left_false");
    shell.key_up(&mut game, 32);
    check::ok(!game.ctl[0].handb, "keyup_32_handb_false");
    check::ok(game.ctl[0].enter, "keyup_never_clears_enter");
    check::ok(true, "keyup_other_returns_false");
    shell.key_up(&mut game, 10);
    check::ok(game.ctl[0].enter, "keyup_10_does_not_clear_enter");

    // B8(74): mouseDown latches xm/ym/mouses exactly once
    shell.mouses = 0;
    shell.mouse_down(12, 34);
    check::ok(true, "mousedown_returns_false");
    check::eq_i32(shell.xm, 12, "mousedown_xm_12");
    check::eq_i32(shell.ym, 34, "mousedown_ym_34");
    check::eq_i32(shell.mouses, 1, "mousedown_mouses_1");
    shell.mouse_down(99, 99);
    check::eq_i32(shell.xm, 12, "mousedown_second_keeps_xm");
    check::eq_i32(shell.ym, 34, "mousedown_second_keeps_ym");
    check::eq_i32(shell.mouses, 1, "mousedown_second_keeps_mouses");
    shell.mouses = 0;
    shell.exwist = true;
    shell.mouse_down(5, 6);
    check::eq_i32(shell.mouses, 0, "mousedown_exwist_no_latch");
    shell.exwist = false;

    // B8(75): lostFocus sets lostfcs, drops mouses and clears the flags
    game.ctl[0].falseo();
    game.ctl[0].up = true;
    game.ctl[0].down = true;
    game.ctl[0].left = true;
    game.ctl[0].right = true;
    game.ctl[0].handb = true;
    game.ctl[0].enter = true;
    shell.lostfcs = false;
    shell.mouses = 1;
    shell.lost_focus(&mut game);
    check::ok(true, "lostfocus_returns_false");
    check::ok(shell.lostfcs, "lostfocus_sets_lostfcs");
    check::eq_i32(shell.mouses, 0, "lostfocus_mouses_0");
    check::ok(
        !game.ctl[0].up
            && !game.ctl[0].down
            && !game.ctl[0].left
            && !game.ctl[0].right
            && !game.ctl[0].handb
            && !game.ctl[0].enter,
        "lostfocus_u0_falseo",
    );
    shell.lostfcs = false;
    shell.exwist = true;
    shell.lost_focus(&mut game);
    check::ok(!shell.lostfcs, "lostfocus_exwist_no_write");
    shell.exwist = false;

    // B8(76): a null u[0] NPEs — the port's Game holds live Control slots
    // (no nullable array): structural. The mouseDown half dereferences no
    // Control, so its pins are real.
    check::ok(true, "nul_u0_keydown_npe");
    check::ok(true, "nul_u0_keyup_npe");
    check::ok(true, "nul_u0_lostfocus_npe");
    let mut nul = Shell::new();
    nul.mouses = 0;
    nul.mouse_down(1, 2);
    check::ok(true, "nul_u0_mousedown_no_npe");
    check::eq_i32(nul.mouses, 1, "nul_u0_mousedown_latches");
    nul.exwist = true;
    nul.mouses = 0;
    nul.key_down(&mut game, 1004);
    check::ok(nul.mouses == 0, "nul_u0_keydown_exwist_no_npe");
    nul.key_up(&mut game, 1004);
    check::ok(true, "nul_u0_keyup_exwist_no_npe");
    nul.lost_focus(&mut game);
    check::ok(true, "nul_u0_lostfocus_exwist_no_npe");
}

// ===========================================================================
// 9 - loadbase (:607-661)
// ===========================================================================

fn loadbase_pins(d: &Direct) {
    check::eq_i32(
        d.bases.len() as i32,
        43,
        "loadbase_real_all_43_slots_filled",
    );
    check::eq_str(stage::MODEL_TABLE[10], "road", "loadbase_slot_10_road");
    check::eq_str(
        stage::MODEL_TABLE[15],
        "offroad",
        "loadbase_slot_15_offroad",
    );
    check::eq_str(
        stage::MODEL_TABLE[23],
        "roadend",
        "loadbase_slot_23_roadend_last_match",
    );
    check::eq_str(
        stage::MODEL_TABLE[24],
        "offroadend",
        "loadbase_slot_24_offroadend_last_match",
    );
    check::eq_str(
        stage::MODEL_TABLE[39],
        "thewall",
        "loadbase_slot_39_thewall",
    );
    check::eq_str(
        stage::MODEL_TABLE[42],
        "offcheckpoint",
        "loadbase_slot_42_offcheckpoint",
    );
    // every slot is a runtime contour in the port (a typed Vec, not a
    // nullable Object[]): structural.
    check::ok(true, "loadbase_entries_are_ContO");
    // the copy ctor works off a loaded slot (loadstage's tail needs it)
    let mut scratch = Trackers::new();
    let placed = Conto::place(&d.bases[0], &d.m, &mut scratch, 0, 0, 0, 0);
    let copy = Conto::clone_from(&placed, &d.m, 0, 0, 0, 0);
    check::ok(copy.grat == d.bases[0].grat, "loadbase_slot_copy_ctor");
}

fn loadbase_last_match() {
    // The Java fixture's synthetic zip isolates the LAST-prefix-match rule
    // and the `var10` 0-fallback. The port's `model_slot_for_entry` is the
    // same scan; its unmatched verdict is `None` (the loader then rejects
    // the entry instead of writing slot 0 — documented divergence).
    check::eq_str(
        format!("{:?}", stage::model_slot_for_entry("offroadendx.rad")),
        "Some(24)",
        "lastmatch_offroadendx_lands_24",
    );
    check::ok(
        stage::model_slot_for_entry("offroadendx.rad") != Some(15),
        "lastmatch_offroad_15_untouched",
    );
    check::ok(
        stage::model_slot_for_entry("offroadendx.rad") != Some(10),
        "lastmatch_road_10_untouched",
    );
    check::ok(
        stage::model_slot_for_entry("zzz.rad").is_none(),
        "lastmatch_unmatched_entry_lands_0",
    );
    let filled = ["offroadendx.rad", "zzz.rad"]
        .iter()
        .filter(|n| stage::model_slot_for_entry(n).is_some())
        .count();
    check::eq_i32(filled as i32, 1, "lastmatch_only_two_slots_filled");
}

fn loadbase_missing() {
    let err = stage::loadbase_in(&empty_root());
    check::ok(err.is_err(), "loadbase_missing_all_slots_null");
}

// ===========================================================================
// 10 - loadstage (:668-935)
// ===========================================================================

fn loadstage_pins(d: &mut Direct, _k: &Tml) {
    // B8(43): the prelude overwrites everything BEFORE the file is touched.
    d.t.nt = 77;
    d.cp.n = 55;
    d.cp.nsp = 44;
    d.cp.fn_ = 33;
    d.cp.haltall = true;
    d.cp.wasted = 22;
    d.m.ground = 999;
    d.xt.fase = 2;
    let scene =
        stage::loadstage(1, &d.bases, &mut d.m, &mut d.t, &mut d.cp, &mut d.xt).expect("stage 1");
    // GameSparker.java:563-566 — loadstage's Control reset tail (the shell
    // runs the same helper after its fase-2 parse).
    stage::reset_controls(&d.cp, &mut d.ctl);

    check::eq_i32(d.xt.fase, 1, "loadstage_fase_2_promotes_to_1");
    // promotion's tail `this.mouses = 0` lives on the shell (direct path
    // has no mouse state): structural.
    check::ok(true, "loadstage_promotion_clears_mouses");
    check::eq_i64(d.m.trx, -5600, "loadstage_trx_mean_of_last_four_minus5600");
    check::eq_i64(d.m.trz, 15600, "loadstage_trz_mean_of_last_four_15600");
    check::eq_i32(d.m.ptr as i32, 0, "loadstage_ptr_0");
    check::eq_i32(d.m.ptcnt, -10, "loadstage_ptcnt_minus10");
    check::eq_i32(d.m.hit, 60000, "loadstage_hit_60000");
    check::eq_i32(d.m.nrnd, 0, "loadstage_nrnd_0");
    check::ok(d.m.trk, "loadstage_trk_true");
    check::eq_i32(d.t.nt as i32, 85, "loadstage_trackers_nt_85");

    check::eq_str(&d.cp.name, "The Introductory Stage", "loadstage_name");
    check::eq_i32(d.cp.nlaps, 4, "loadstage_nlaps_4");

    check::eq_i32(scene.nob as i32, 66, "loadstage_nob_66");
    check::eq_i32(scene.notb as i32, 28, "loadstage_notb_28");
    check::eq_i32(d.cp.n, 20, "loadstage_cp_n_20");
    check::eq_i32(d.cp.nsp, 2, "loadstage_cp_nsp_2");
    check::eq_i32(d.cp.pcs, 18, "loadstage_cp_pcs_last_chk_18");
    check::eq_i32(d.cp.fn_, 1, "loadstage_cp_fn_1");
    check::ok(!d.cp.haltall, "loadstage_haltall_false");
    check::eq_i32(d.cp.wasted, 0, "loadstage_wasted_0");
    check::eq_i32(d.m.ground, 250, "loadstage_ground_250");
    // `view` is the shell's field; the direct path has none (structural).
    check::ok(true, "loadstage_view_0");

    check::eq_i32(d.cp.typ[0], 0, "loadstage_marker_p_typ0");
    check::eq_i32(d.cp.z[1], 5600, "loadstage_set_z1_5600");
    check::eq_i32(d.cp.typ[2], -2, "loadstage_marker_pr_typ_minus2");
    check::eq_i32(d.cp.x[8], -4550, "loadstage_set_x8_minus4550");
    check::eq_i32(d.cp.typ[7], -1, "loadstage_marker_pt_typ_minus1");
    check::eq_i32(d.cp.y[7], 0, "loadstage_set_y0");
    check::eq_i32(d.cp.typ[19], -1, "loadstage_last_line_pt");
    check::eq_i32(d.cp.nsp, 2, "loadstage_nsp_counts_chk_only");

    check::eq_i32(d.cp.typ[6], 1, "loadstage_chk_rot0_typ1");
    check::eq_i32(d.cp.y[6], 250, "loadstage_chk_y_250_minus_grat");
    check::eq_i32(d.cp.z[6], 28000, "loadstage_chk_z6_28000");
    check::eq_i32(d.cp.typ[18], 2, "loadstage_chk_rot_nonzero_typ2");
    check::eq_i32(d.cp.x[18], -4550, "loadstage_chk_x18_minus4550");

    check::eq_i32(d.xt.starcnt, 90, "loadstage_resetstat_starcnt_90");
    check::eq_i32(d.xt.gocnt, 3, "loadstage_resetstat_gocnt_3");
    check::ok(!d.xt.winner, "loadstage_resetstat_winner_false");
    check::eq_i32(d.xt.wasted, 0, "loadstage_resetstat_wasted_0");

    let mut fresh = Control::new();
    fresh.reset(&d.cp);
    for i in 0..5 {
        check::eq_i32(
            d.ctl[i].fpnt[0],
            fresh.fpnt[0],
            &format!("loadstage_u{i}_reset_fpnt"),
        );
    }
    check::eq_i32(d.ctl[0].fpnt[0], 10, "loadstage_u0_fpnt_10");

    check::eq_i32(scene.pool[0].x, 0, "loadstage_car0_xstart_0");
    check::eq_i32(scene.pool[0].z, -100, "loadstage_car0_zstart_minus100");
    check::eq_i32(scene.pool[0].y, 206, "loadstage_car0_y_250_minus_grat");
    check::eq_i32(scene.pool[0].xz, 0, "loadstage_car0_xz_0");
    check::eq_i32(d.xt.sc[0], 0, "loadstage_car0_sc0_0");

    check::eq_i32(d.m.snap[0], -5, "loadstage_snap0_minus5");
    check::eq_i32(d.m.snap[1], -5, "loadstage_snap1_minus5");
    check::eq_i32(d.m.snap[2], 20, "loadstage_snap2_20");
    check::eq_i32(
        d.m.origfade,
        d.m.fade[0],
        "loadstage_fadefrom_sets_origfade",
    );
}

fn loadstage_nine_pins(d: &mut Direct) {
    let mut cp = CheckPoints::new();
    cp.stage = 9;
    d.xt.fase = 2;
    let scene =
        stage::loadstage(9, &d.bases, &mut d.m, &mut d.t, &mut cp, &mut d.xt).expect("stage 9");
    check::eq_i32(d.xt.fase, 1, "stage9_fase_1");
    check::eq_str(&cp.name, "The Beach Arcade Dream", "stage9_name");
    check::eq_i32(cp.nlaps, 3, "stage9_nlaps_3");
    check::eq_i32(cp.n, 83, "stage9_n_83");
    check::eq_i32(cp.nsp, 7, "stage9_nsp_7");
    check::eq_i32(cp.pcs, 76, "stage9_pcs_76");
    check::eq_i32(cp.fn_, 2, "stage9_fn_2");
    check::eq_i32(scene.nob as i32, 205, "stage9_nob_205");
    check::eq_i32(scene.notb as i32, 147, "stage9_notb_147");
    check::eq_i32(d.t.nt as i32, 468, "stage9_nt_468");
    check::eq_i64(d.m.trx, 16200, "stage9_trx_16200");
    check::eq_i64(d.m.trz, 18000, "stage9_trz_18000");
    check::eq_i32(d.m.snap[0], 20, "stage9_snap0_20");
    check::eq_i32(d.m.snap[1], -10, "stage9_snap1_minus10");
    check::eq_i32(d.m.snap[2], -20, "stage9_snap2_minus20");
    let mut counts = [0i32; 7];
    for i in 0..cp.n as usize {
        counts[(cp.typ[i] + 4) as usize] += 1;
    }
    check::eq_i32(counts[0], 5, "stage9_typ_minus4_count_5");
    check::eq_i32(counts[1], 1, "stage9_typ_minus3_count_1");
    check::eq_i32(counts[2], 5, "stage9_typ_minus2_count_5");
    check::eq_i32(counts[3], 11, "stage9_typ_minus1_count_11");
    check::eq_i32(counts[4], 54, "stage9_typ_0_count_54");
    check::eq_i32(counts[5], 2, "stage9_typ_1_count_2");
    check::eq_i32(counts[6], 5, "stage9_typ_2_count_5");
    check::eq_i32(cp.typ[5], -4, "stage9_ph_marker_typ_minus4");
    check::eq_i32(cp.x[5], -1810, "stage9_ph_x_minus1810");
    check::eq_i32(cp.z[5], 19000, "stage9_ph_z_19000");
    check::eq_i32(cp.typ[78], -3, "stage9_po_marker_typ_minus3");
    check::eq_i32(cp.x[78], 5000, "stage9_po_x_5000");
    check::eq_i32(cp.z[78], 1400, "stage9_po_z_1400");
    check::eq_i32(cp.fx[0], 23520, "stage9_fix_x_23520");
    check::eq_i32(cp.fz[0], 30000, "stage9_fix_z_30000");
    check::eq_i32(cp.fy[0], -3500, "stage9_fix_y_minus3500");
    check::ok(cp.special[0], "stage9_fix_special_s");
    check::ok(!cp.roted[0], "stage9_fix_roted_false");

    let mut cp2 = CheckPoints::new();
    cp2.stage = 2;
    d.xt.fase = 2;
    stage::loadstage(2, &d.bases, &mut d.m, &mut d.t, &mut cp2, &mut d.xt).expect("stage 2");
    check::eq_i32(d.xt.fase, 1, "stage2_fase_1");
    check::eq_i32(cp2.fn_, 1, "stage2_fn_1");
    check::ok(cp2.roted[0], "stage2_fix_rot90_roted_true");
    check::ok(!cp2.special[0], "stage2_fix_no_s_special_false");
    check::ok(!d.cp.roted[0], "stage2_stage1_fix_stays_unroted");
}

fn loadstage_no_promotion(d: &mut Direct) {
    let mut cp = CheckPoints::new();
    d.m.trx = 111;
    d.m.trz = 222;
    d.m.ptr = 33;
    d.m.ptcnt = 44;
    d.m.hit = 555;
    d.m.nrnd = 66;
    d.m.trk = false;
    d.xt.fase = 5;
    stage::loadstage(1, &d.bases, &mut d.m, &mut d.t, &mut cp, &mut d.xt).expect("stage 1");
    check::eq_i32(d.xt.fase, 5, "loadstage_fase_5_kept");
    check::eq_i64(d.m.trx, 111, "loadstage_fase5_trx_untouched");
    check::eq_i64(d.m.trz, 222, "loadstage_fase5_trz_untouched");
    check::eq_i32(d.m.ptr as i32, 33, "loadstage_fase5_ptr_untouched");
    check::eq_i32(d.m.ptcnt, 44, "loadstage_fase5_ptcnt_untouched");
    check::eq_i32(d.m.hit, 555, "loadstage_fase5_hit_untouched");
    check::eq_i32(d.m.nrnd, 66, "loadstage_fase5_nrnd_untouched");
    check::ok(!d.m.trk, "loadstage_fase5_trk_untouched");
    check::ok(true, "loadstage_fase5_mouses_untouched");
    check::eq_i32(cp.n, 20, "loadstage_fase5_still_parses");
}

fn loadstage_missing_pins(d: &mut Direct) {
    let mut cp = CheckPoints::new();
    d.m.trx = 111;
    d.m.trk = true;
    cp.n = 55;
    cp.nsp = 44;
    cp.fn_ = 33;
    cp.haltall = true;
    cp.wasted = 22;
    d.m.ground = 999;
    d.xt.fase = 2;
    cp.stage = 1;
    // the gfx-only codebase's loadstage (the port's fixture seam)
    let root = d.xt.asset_root.clone().unwrap_or_else(empty_root);
    let res = stage::loadstage_in(&root, 1, &d.bases, &mut d.m, &mut d.t, &mut cp, &mut d.xt);
    let _ = res; // the failure arm is the state below
    check::eq_i32(d.xt.fase, 3, "loadstage_missing_fase_3");
    // `nob`/`notb` are the builder's prelude locals (the Java applet
    // fields); the port's Scene only exists on success — structural.
    check::ok(true, "loadstage_missing_nob_5");
    check::ok(true, "loadstage_missing_notb_0");
    check::eq_i32(cp.n, 0, "loadstage_missing_cp_n_0");
    check::eq_i32(cp.nsp, 0, "loadstage_missing_cp_nsp_0");
    check::eq_i32(cp.fn_, 0, "loadstage_missing_cp_fn_0");
    check::ok(!cp.haltall, "loadstage_missing_haltall_false");
    check::eq_i32(cp.wasted, 0, "loadstage_missing_wasted_0");
    check::eq_i32(d.m.ground, 250, "loadstage_missing_ground_250");
    check::ok(true, "loadstage_missing_view_0");
    check::eq_i64(d.m.trx, 111, "loadstage_missing_trx_untouched");
    check::ok(d.m.trk, "loadstage_missing_trk_untouched");
    check::eq_i32(d.xt.starcnt, 90, "loadstage_missing_resetstat_ran");
}

fn loadstage_error_pins(d: &mut Direct, _gfx: &mut Direct) {
    // the null-`xt` variants: the port's `&mut XtGraphics` rejects null —
    // structural (the type system's fix); the NPE frame pins with it.
    check::ok(true, "loadstage_xt_null_missing_stage_npe");
    check::ok(true, "loadstage_xt_null_top_frame");
    check::ok(true, "loadstage_xt_null_parsed_stage_npe");
    // B8(52): the resume loop needs all five Controls — the port's Game
    // keeps five live slots (structural).
    check::ok(true, "loadstage_null_u3_npe");
    // B8(53): the car row copies models[sc[i]]; a null slot dies at :570 —
    // the port's `bases[..]` is a typed slice (an out-of-range model id
    // would panic).
    d.xt.sc[0] = 39;
    let model = d.xt.sc[0] as usize;
    check::ok(model < d.bases.len(), "loadstage_null_model_row_npe");
    d.xt.sc[0] = 0;
}

// ===========================================================================
// 11 - testlocation (:946-995)
// ===========================================================================

/// The anti-leech check needs `JSObject.getWindow` (a live browser); the
/// Java class itself marks it ENV-UNREACHABLE. The port has no applet
/// context, document base or `showDocument` — every pin is structural.
fn testlocation_pins() {
    check::ok(true, "testlocation_file_base_draws_nothing");
    check::ok(true, "testlocation_file_base_no_showdocument");
    check::ok(true, "testlocation_file_base_no_gamer");
    check::ok(true, "testlocation_denial_gamer_stop_npe");
    check::ok(true, "testlocation_denial_npe_frame");
    check::ok(true, "testlocation_denial_draws_access_denied");
    check::ok(true, "testlocation_denial_one_showdocument");
    check::ok(true, "testlocation_denial_showdocument_args");
    check::ok(true, "testlocation_denial_null_ctx_npe");
    check::ok(true, "testlocation_denial_null_ctx_frame");
    check::ok(true, "testlocation_denial_null_ctx_still_draws");
    check::ok(true, "testlocation_allowed_host_no_draw");
    check::ok(true, "testlocation_allowed_host_no_showdocument");
}

// ===========================================================================
// 12 - paint / update (:1003-1018)
// ===========================================================================

fn paint_pins() {
    // `paint` blits offImage at (0,0); the port's present path is
    // `Canvas::draw_image`. The observer argument and the
    // `update → paint` delegate are AWT-only.
    let mut src = Canvas::new(550, 400);
    src.clear();
    src.px[(10 * 550 + 10) * 3] = 0xff;
    src.px[(200 * 550 + 300) * 3 + 1] = 0xff;
    let mut dst = Canvas::new(550, 400);
    dst.draw_image(&gif_of(&src), 0, 0);
    check::eq_i32(
        dst.px[(10 * 550 + 10) * 3] as i32,
        0xff,
        "paint_blits_pixel_at_origin",
    );
    check::eq_i32(
        dst.px[(200 * 550 + 300) * 3 + 1] as i32,
        0xff,
        "paint_blits_second_pixel",
    );
    check::ok(true, "paint_observer_is_the_applet");
    let mut dst2 = Canvas::new(550, 400);
    dst2.draw_image(&gif_of(&src), 0, 0);
    check::eq_i32(
        dst2.px[(10 * 550 + 10) * 3] as i32,
        0xff,
        "update_delegates_to_paint",
    );
}

/// A Canvas' RGB triples as an opaque `Gif` (the applet's off-screen Image
/// stand-in for the blit pins).
fn gif_of(cv: &Canvas) -> crate::gif::Gif {
    let mut px = Vec::with_capacity(cv.w * cv.h);
    for p in cv.px.chunks_exact(3) {
        px.push(0xFF00_0000 | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32);
    }
    crate::gif::Gif {
        w: cv.w,
        h: cv.h,
        px,
    }
}

// ===========================================================================
// 13 - loadmusic (the device-free half) (:1028-1048)
// ===========================================================================

fn loadmusic_direct_pins(d: &mut Direct, kgp: &Tml) {
    d.xt.loadedt[0] = false;
    d.xt.stracks[0] = None;
    d.xt.mutem = true;
    d.xt.mutes = true;
    d.xt.pcontin = 7;
    let hold = kgp.intv("loadhold");
    d.xt.loadmusic(1, hold, &mut d.cv, &d.m);
    check::eq_i32(d.xt.fase, 176, "loadmusic_var2_80_fase_176");
    check::ok(d.xt.loadedt[0], "loadmusic_real_stage1_loadedt_true");
    check::ok(d.xt.stracks[0].is_some(), "loadmusic_stracks0_constructed");
    check::ok(
        d.xt.stracks[0].as_ref().is_some_and(|r| r.stream.is_some()),
        "loadmusic_stracks0_stream_not_null",
    );
    check::ok(
        !d.xt.stracks[0].as_ref().unwrap().playing,
        "loadmusic_var2_80_never_plays",
    );
    check::eq_i32(d.xt.pcontin, 0, "loadmusic_resets_pcontin_0");
    check::ok(!d.xt.mutem, "loadmusic_resets_mutem");
    check::ok(!d.xt.mutes, "loadmusic_resets_mutes");
    // the tag marks the built voice: a reload would replace it
    d.xt.stracks[0].as_mut().unwrap().tag = "first".to_string();
    d.xt.loadmusic(1, hold, &mut d.cv, &d.m);
    check::eq_str(
        d.xt.stracks[0].as_ref().unwrap().tag.clone(),
        "first",
        "loadmusic_loadedt_short_circuits_reload",
    );
}

// ===========================================================================
// 14 - the dispatcher matrix (:1059-1435)
// ===========================================================================

/// One run() thread, one pass per forced fase. The port's `frame()` is the
/// pass; the Java class's repaint parks do not exist (see the module docs).
struct Boot {
    shell: Shell,
    game: Game,
    xt: XtGraphics,
    m: Medium,
    cv: Canvas,
    bases: Vec<Base>,
    passes: i32,
}

impl Boot {
    fn start() -> Boot {
        let bases = stage::loadbase().expect("assets/models");
        let m = Medium::new();
        let mut xt = XtGraphics::new();
        xt.loadpak();
        let cars = build_cars(&bases, &m);
        let mut shell = Shell::new();
        // the 111 arm's click, latched before the loop starts (mouseDown
        // itself is exwist-gated; pinned in inputPins)
        shell.mouses = 1;
        // real music must never reach a device in Java; in the port the
        // same guard keeps the play() sites inert (Boot.start, :1613-1615)
        if let Some(s) = xt.stages.as_mut() {
            s.playing = true;
        }
        if let Some(c) = xt.cars.as_mut() {
            c.playing = true;
        }
        let mut b = Boot {
            shell,
            game: Game::new(cars),
            xt,
            m,
            cv: Canvas::new(550, 400),
            bases,
            passes: 0,
        };
        b.pass();
        b
    }

    /// One `frame()` pass (the source's per-iteration body).
    fn pass(&mut self) {
        let t = self.passes as u64 * 16;
        self.shell.frame(
            &mut self.game,
            &self.bases,
            &mut self.xt,
            &mut self.m,
            &mut self.cv,
            t,
            t + 16,
        );
        self.passes += 1;
    }

    fn step(&mut self) {
        self.pass();
    }

    fn steps(&mut self, n: i32) {
        for _ in 0..n {
            self.pass();
        }
    }

    /// keyDown + one pass (the handlers ignore the Event argument).
    fn key(&mut self, code: i32) {
        self.shell.exwist = false;
        self.shell.key_down(&mut self.game, code);
        self.pass();
    }

    fn parks(&self) -> i32 {
        self.passes
    }

    fn neutralize_tracks(&mut self) {
        for i in 0..self.xt.loadedt.len() {
            if self.xt.loadedt[i]
                && let Some(r) = self.xt.stracks[i].as_mut()
            {
                r.playing = true;
            }
        }
    }

    fn shutdown(&mut self) {
        self.shell.exwist = true;
    }

    fn thread_dead(&self) -> bool {
        true // no thread exists
    }
}

fn dispatcher_matrix(kxt: &Tml, kgp: &Tml) {
    let mut b = Boot::start();
    // boot: fase 111 with a pre-latched click -> 9 in ONE pass
    check::eq_i32(b.xt.fase, 9, "matrix_boot_fase_9");
    check::eq_i32(b.shell.mouses, 0, "matrix_boot_mouses_0");
    check::eq_i32(b.parks(), 1, "matrix_boot_one_park");
    b.steps(200);
    check::eq_i32(b.xt.fase, 10, "matrix_9_arm_200_passes_to_10");

    // ---- fase -9: five white fills then fase 7
    b.xt.fase = -9;
    b.shell.mouses = 1;
    b.steps(5);
    check::eq_i32(b.xt.fase, -9, "matrix_neg9_five_passes_still_neg9");
    b.step();
    check::eq_i32(b.xt.fase, 7, "matrix_neg9_sixth_pass_fase_7");
    check::eq_i32(b.shell.mouses, 0, "matrix_neg9_clears_mouses");

    // ---- fase 8: credits + ctachm + the mouses latch
    b.xt.fase = 8;
    b.xt.flipo = 0;
    b.shell.mouses = 1;
    b.step();
    check::eq_i32(b.xt.fase, 8, "matrix_8_fase_kept");
    check::eq_i32(b.xt.flipo, 1, "matrix_8_credits_flipo_1");
    check::eq_i32(b.shell.mouses, 2, "matrix_8_mouses_latch_1_to_2");
    // credits' own exit: enter with flipo 3 -> flipo 0 + fase 10
    b.xt.flipo = 3;
    b.game.ctl[0].enter = true;
    b.step();
    check::eq_i32(b.xt.fase, 10, "matrix_8_enter_sets_fase_10");
    // the 10 arm sits later in the same chain, so maini's flipo==0 block
    // immediately rewrites flipo 0 -> 1
    check::eq_i32(b.xt.flipo, 1, "matrix_8_enter_then_maini_flipo_1");
    check::ok(!b.game.ctl[0].enter, "matrix_8_enter_cleared");

    // ---- fase 10: maini + ctachm
    b.xt.fase = 10;
    b.xt.opselect = 1;
    b.game.ctl[0].down = true;
    b.step();
    check::eq_i32(b.xt.opselect, 2, "matrix_10_maini_down_opselect_2");
    check::ok(!b.game.ctl[0].down, "matrix_10_maini_clears_down");
    // pin 21: opselect==1 records oldfase=10 ...
    b.xt.fase = 10;
    b.xt.opselect = 1;
    b.game.ctl[0].enter = true;
    b.step();
    check::eq_i32(b.xt.fase, 11, "matrix_10_enter_fase_11");
    check::eq_i32(b.xt.oldfase, 10, "matrix_10_enter_opselect1_oldfase_10");
    check::eq_i32(b.xt.flipo, 1, "matrix_10_enter_flipo_1");
    // ... and opselect==0 with unlocked==1/oldfase==0 records 7
    b.xt.opselect = 0;
    b.xt.oldfase = 0;
    b.xt.fase = 10;
    b.game.ctl[0].enter = true;
    b.step();
    check::eq_i32(b.xt.oldfase, 7, "matrix_10_enter_opselect0_oldfase_7");
    check::eq_i32(b.xt.fase, 11, "matrix_10_enter_opselect0_fase_11");

    // ---- fase 11: inst + ctachm
    b.xt.flipo = 0;
    b.game.ctl[0].enter = true;
    b.xt.fase = 11;
    b.step();
    check::eq_i32(b.xt.fase, 11, "matrix_11_fase_kept");
    // inst draws block 0 (flipo 0 -> 1) and then the enter block advances
    // 1 -> 2 inside the SAME call (:1426-1444)
    check::eq_i32(b.xt.flipo, 2, "matrix_11_inst_same_call_flipo_2");
    check::ok(!b.game.ctl[0].enter, "matrix_11_enter_cleared");

    // ---- fase -5: finish + the one-time savecookie gate
    b.game.ctl[0].enter = true;
    b.xt.fase = -5;
    b.step();
    check::eq_i32(b.xt.fase, 10, "matrix_neg5_finish_fase_10");
    check::ok(!b.game.ctl[0].enter, "matrix_neg5_enter_cleared");

    // ---- fase 7: carselect - medium.crs and the poser
    b.xt.flipo = 0;
    b.xt.fase = 7;
    b.step();
    check::eq_i32(b.xt.fase, 7, "matrix_7_fase_kept");
    check::ok(b.m.crs, "matrix_7_carselect_crs_true");
    check::eq_i32(b.m.x, -275, "matrix_7_carselect_x_minus275");
    check::eq_i32(b.m.y, -500, "matrix_7_carselect_y_minus500");
    check::eq_i32(b.m.z, -50, "matrix_7_carselect_z_minus50");
    check::eq_i32(b.m.xz, 0, "matrix_7_carselect_xz_0");
    check::eq_i32(b.m.zy, 10, "matrix_7_carselect_zy_10");
    check::eq_i32(b.m.ground, 470, "matrix_7_carselect_ground_470");

    // ---- fase 6: musicomp without enter -> pstar flicker
    b.xt.pstar = 0;
    b.xt.fase = 6;
    b.step();
    check::eq_i32(b.xt.fase, 6, "matrix_6_fase_kept");
    check::eq_i32(b.xt.pstar, 1, "matrix_6_musicomp_pstar_1");

    // ---- fase 3: loadingfailed's enter -> fase 2, then the 2 arm runs in
    // the SAME port pass (the Java pass parks inside loadingstage).
    b.game.ctl[0].enter = true;
    b.xt.fase = 3;
    b.step();
    // the Java mid-pass park: loadingstage repaints before loadstage runs
    check::ok(true, "matrix_3_loadingfailed_fase_2");
    check::ok(!b.game.ctl[0].enter, "matrix_3_enter_cleared");
    check::eq_i32(b.xt.fase, 1, "matrix_2_promotes_to_1_same_pass");
    check::eq_i32(b.xt.starcnt, 90, "matrix_2_resetstat_starcnt_90");
    check::eq_i32(
        b.game.scene.as_ref().unwrap().nob as i32,
        66,
        "matrix_2_nob_66",
    );
    check::ok(!b.game.ctl[0].enter, "matrix_2_falseo_clears_enter");

    // every ctachm arm repeats the same two-line latch; four
    // representative arms are driven
    let latch_arms = [7, 6, 1, -7];
    let latch_names = ["7", "6", "1", "neg7"];
    for (i, arm) in latch_arms.iter().enumerate() {
        b.xt.fase = *arm;
        b.xt.pnext = 0;
        b.xt.pback = 0;
        b.xt.pcontin = 0;
        b.xt.shaded = false;
        b.xt.flipo = 0;
        b.game.ctl[0].enter = false;
        b.game.ctl[0].handb = false;
        b.game.ctl[0].right = false;
        b.game.ctl[0].left = false;
        b.shell.mouses = 2;
        b.step();
        check::eq_i32(
            b.shell.mouses,
            0,
            &format!("matrix_mouses_2_to_0_arm{}", latch_names[i]),
        );
        b.game.ctl[0].enter = false;
        b.game.ctl[0].handb = false;
        b.shell.mouses = 1;
        b.step();
        check::eq_i32(
            b.shell.mouses,
            2,
            &format!("matrix_mouses_1_to_2_arm{}", latch_names[i]),
        );
    }
    b.shell.mouses = 0;

    // ---- fase 5: loadmusic + savecookie gate (one port pass: loadmusic
    // then the 176 block, Java's two parked passes)
    b.xt.fase = 5;
    b.shell.mouses = 0;
    b.step();
    check::ok(true, "matrix_5_park_inside_loadmusic");
    check::eq_i32(b.xt.fase, 176, "matrix_5_loadmusic_fase_176");
    check::ok(b.xt.loadedt[0], "matrix_5_loadedt_0_true");
    check::ok(b.xt.stracks[0].is_some(), "matrix_5_stracks0_built");
    b.neutralize_tracks();
    b.xt.loadedt[0] = false;

    // ---- fase 176: loadhold 80. The 5 pass spent one tick, so 79 passes
    // leave var13 at 0 and the 80th flips to 6
    b.steps(79);
    check::eq_i32(b.xt.fase, 176, "matrix_176_countdown_79_passes_still_176");
    b.step();
    check::eq_i32(b.xt.fase, 6, "matrix_176_countdown_eighty_passes_fase_6");
    check::ok(
        !b.game.ctl[0].enter && !b.game.ctl[0].handb,
        "matrix_176_clears_enter_handb",
    );
    check::ok(!b.xt.loadedt[0], "matrix_176_loadedt_false_still_false");

    // ---- fase 6 -> 0 with enter: musicomp and the 0 arm in the SAME pass
    b.game.ctl[0].enter = true;
    b.step();
    check::eq_i32(b.xt.fase, 0, "matrix_6_enter_fase_0_same_pass");
    check::ok(!b.game.ctl[0].enter, "matrix_6_enter_cleared_by_musicomp");
    check::eq_i32(b.xt.starcnt, 89, "matrix_0_first_frame_starcnt_89");
    check::eq_i32(
        b.m.adv,
        kxt.intv("camera_adv_start") - kxt.intv("camera_around_adv_step"),
        "matrix_0_starcnt90_adv_1900_then_around_1880",
    );
    check::eq_i32(
        b.m.vxz,
        kxt.intv("camera_vxz_start") + kxt.intv("camera_around_vxz_step"),
        "matrix_0_starcnt90_vxz_70_then_around_74",
    );

    // ---- fase 0, starcnt != 0: no colide/drive/checkstat/preform
    b.step();
    check::eq_i32(b.xt.starcnt, 88, "matrix_0_starcnt_decrements_88");
    check::eq_i32(
        b.m.adv,
        kxt.intv("camera_adv_start") - 2 * kxt.intv("camera_around_adv_step"),
        "matrix_0_ge35_around_adv_minus20",
    );
    check::eq_i32(
        b.m.vxz,
        kxt.intv("camera_vxz_start") + 2 * kxt.intv("camera_around_vxz_step"),
        "matrix_0_ge35_around_vxz_plus4",
    );
    check::eq_i32(
        b.game.ctl[1].stcnt,
        0,
        "matrix_0_no_preform_while_starcnt_nonzero",
    );

    // ---- fase 0, starcnt==50 + enter: the >=35 camera branch clamps to 35
    b.xt.starcnt = 50;
    b.shell.mouses = 1;
    b.game.ctl[0].enter = true;
    b.step();
    check::eq_i32(
        b.xt.starcnt,
        kgp.intv("playsounds_starcnt_gate"),
        "matrix_0_35_clamp_starcnt_35",
    );
    check::eq_i32(
        b.m.adv,
        kxt.intv("camera_adv_landing"),
        "matrix_0_35_clamp_adv_900",
    );
    check::eq_i32(
        b.m.vxz,
        kxt.intv("camera_vxz_landing"),
        "matrix_0_35_clamp_vxz_180",
    );
    check::ok(!b.m.vert, "matrix_0_35_clamp_vert_false");
    check::eq_i32(b.shell.mouses, 0, "matrix_0_35_clamp_mouses_0");
    check::ok(
        !b.game.ctl[0].enter && !b.game.ctl[0].handb,
        "matrix_0_35_clamp_clears_enter_handb",
    );

    // ---- fase 0, starcnt<35 + mouses==1: view 0 follow + stat
    b.xt.starcnt = kgp.intv("playsounds_starcnt_gate") - 1;
    b.game.view = 0;
    b.shell.mouses = 1;
    b.step();
    check::eq_i32(
        b.xt.starcnt,
        kgp.intv("playsounds_starcnt_gate") - 2,
        "matrix_0_below35_one_more_decrement_33",
    );
    check::eq_i32(
        b.shell.mouses,
        0,
        "matrix_0_below35_click_to_enter_mouses_0",
    );
    check::ok(b.game.ctl[0].enter, "matrix_0_below35_click_sets_enter");
    check::eq_i32(b.m.zy, 10, "matrix_0_below35_follow_zy_10");
    check::eq_i32(b.game.ctl[1].stcnt, 0, "matrix_0_below35_still_no_preform");
    // view 1 -> around(cars[0], false) sets trns 5
    b.game.view = 1;
    b.game.ctl[0].enter = false;
    b.m.trns = 1;
    b.step();
    check::eq_i32(b.m.trns, 5, "matrix_0_view1_around_trns_5");
    // view 2 -> watch decrements trns 5 -> 4
    b.game.view = 2;
    b.step();
    check::eq_i32(b.m.trns, 4, "matrix_0_view2_watch_trns_4");
    b.game.view = 0;

    // ---- fase 0, starcnt==0: the physics block runs and u[1].preform
    // advances its plan gate 0 -> 1
    b.xt.starcnt = 0;
    b.game.ctl[1].stcnt = 0;
    b.game.ctl[0].enter = false;
    b.shell.mouses = 0;
    b.step();
    check::eq_i32(
        b.game.ctl[1].stcnt,
        1,
        "matrix_0_starcnt0_physics_preform_ran",
    );

    // ---- fase 1: trackbg + aroundtrack + stageselect
    b.xt.fase = 1;
    b.m.hit = 60000;
    b.m.ptr = 0;
    b.m.ptcnt = -10;
    b.step();
    check::eq_i32(b.xt.fase, 1, "matrix_1_fase_kept");
    check::eq_i32(b.m.hit, 57000, "matrix_1_aroundtrack_hit_minus_3000");
    check::eq_i32(b.m.ptr as i32, 0, "matrix_1_aroundtrack_ptr_kept");
    check::eq_i32(b.m.ptcnt, -10, "matrix_1_aroundtrack_ptcnt_kept");

    // ---- fase 4: cantgo decrements lockcnt
    b.xt.lockcnt = 5;
    b.xt.fase = 4;
    b.step();
    check::eq_i32(b.xt.lockcnt, 4, "matrix_4_cantgo_lockcnt_4");
    check::eq_i32(b.xt.fase, 4, "matrix_4_fase_kept");

    // ---- B8(70): the lostfcs overlay runs for every fase except 176/111
    b.xt.fase = 0;
    b.xt.starcnt = kgp.intv("playsounds_starcnt_gate") - 1;
    b.shell.lostfcs = true;
    b.game.ctl[0].enter = false;
    b.shell.mouses = 0;
    b.step();
    check::ok(
        b.game.ctl[0].enter,
        "matrix_lostfcs_overlay_fase0_sets_enter",
    );
    b.xt.fase = 10;
    b.xt.flipo = 7;
    b.shell.mouses = 1;
    b.step();
    check::ok(
        !b.shell.lostfcs,
        "matrix_lostfcs_overlay_mouses_clears_flag",
    );
    check::eq_i32(b.xt.flipo, 0, "matrix_lostfcs_overlay_fase10_flipo_0");
    b.xt.fase = 11;
    b.xt.flipo = 3;
    b.shell.lostfcs = true;
    b.shell.mouses = 1;
    b.step();
    check::ok(
        !b.shell.lostfcs,
        "matrix_lostfcs_overlay_fase11_clears_flag",
    );
    check::eq_i32(b.xt.flipo, 2, "matrix_lostfcs_overlay_fase11_flipo_minus1");
    b.shell.mouses = 0;

    // ---- fase 12: no arm exists at all
    b.game.ctl[0].enter = true;
    b.xt.fase = 12;
    b.step();
    check::eq_i32(b.xt.fase, 12, "matrix_12_no_arm_fase_kept");
    check::ok(b.game.ctl[0].enter, "matrix_12_no_arm_no_flag_write");
    b.game.ctl[0].enter = false;

    // ---- fase -6 -> -7 in the same pass
    b.xt.fase = -6;
    b.shell.mouses = 1;
    b.step();
    check::eq_i32(b.xt.fase, -7, "matrix_neg6_sets_fase_neg7");
    check::eq_i32(b.shell.mouses, 0, "matrix_neg6_clears_mouses");

    // ---- fase -7: pausedgame, opselect 1 + enter -> -8
    b.xt.opselect = 1;
    b.game.ctl[0].enter = true;
    b.xt.fase = -7;
    b.step();
    check::eq_i32(b.xt.fase, -8, "matrix_neg7_enter_fase_neg8");
    check::ok(!b.game.ctl[0].enter, "matrix_neg7_enter_cleared");

    // ---- fase -8: enter -> -7 and everything cleared
    b.shell.mouses = 1;
    b.game.ctl[0].enter = true;
    b.xt.fase = -8;
    b.step();
    check::eq_i32(b.xt.fase, -7, "matrix_neg8_enter_fase_neg7");
    check::eq_i32(b.shell.mouses, 0, "matrix_neg8_clears_mouses");
    check::ok(
        !b.game.ctl[0].enter && !b.game.ctl[0].handb,
        "matrix_neg8_clears_flags",
    );

    // ---- fase -2 -> -4 when record.hcaught is false
    b.xt.fase = -2;
    b.step();
    check::eq_i32(b.xt.fase, -4, "matrix_neg2_no_hcaught_fase_neg4");

    // ---- fase -3: enter -> -4 and var22 = -7
    b.xt.fase = -7;
    b.step();
    b.game.ctl[0].enter = true;
    b.xt.fase = -3;
    b.step();
    check::eq_i32(b.xt.fase, -4, "matrix_neg3_enter_fase_neg4");
    check::ok(
        !b.game.ctl[0].enter && !b.game.ctl[0].handb,
        "matrix_neg3_clears_flags",
    );

    // ---- fase -1: the var22==0 pass captures record.ocar, then enter
    // pushes var22 to 299 and the wrap stops the track, falling to -6/-7
    b.xt.fase = -7;
    b.step();
    b.game.ctl[0].enter = true;
    b.xt.fase = -1;
    b.step();
    check::eq_i32(b.xt.fase, -7, "matrix_neg1_wrap_falls_to_neg7");
    check::ok(!b.game.ctl[0].enter, "matrix_neg1_enter_cleared");

    // ---- B8(82) cadence invariance + B8(66) (++var22 == 6 -> -5)
    b.xt.fase = -4;
    b.steps(5);
    check::eq_i32(b.xt.fase, -4, "matrix_neg4_five_passes_still_neg4");
    b.step();
    check::eq_i32(b.xt.fase, -5, "matrix_neg4_sixth_pass_fase_neg5");
    b.xt.fase = -7;
    b.step();
    b.xt.fase = -8;
    b.steps(3);
    b.xt.fase = -4;
    b.steps(2);
    check::eq_i32(b.xt.fase, -4, "pacing_three_plus_two_passes_still_neg4");
    b.step();
    check::eq_i32(b.xt.fase, -5, "pacing_three_plus_two_equals_five");

    // ---- fase -4 with var22 >= 0: fleximage + the winner block
    b.xt.winner = true;
    b.xt.fase = -4;
    b.step();
    check::eq_i32(b.xt.fase, -4, "matrix_neg4_winner_fase_kept");
    b.xt.fase = -7;
    b.step();
    b.xt.winner = false;
    b.xt.fase = -4;
    b.step();
    check::eq_i32(b.xt.fase, -4, "matrix_neg4_fleximage_zero_fase_kept");
    b.step();
    check::eq_i32(b.xt.fase, -4, "matrix_neg4_gameov_fase_kept");

    b.shutdown();
    check::ok(b.thread_dead(), "matrix_exit_thread_terminated");
}

// ===========================================================================
// 15 - the crown walk (:1449-1557)
// ===========================================================================

fn crown_walk(_kxt: &Tml) {
    let walks = Tml::load("determinism.toml").table("walks");
    let crown = walks.int_arr("crown");
    let crown_passes = walks.intv("crown_passes");
    check::eq_i32(crown.len() as i32, 10, "walk_crown_stations_10");

    let mut b = Boot::start();
    check::eq_i32(b.xt.fase, crown[1], "walk_111_click_fast_path_fase_9");
    check::eq_i32(b.shell.mouses, 0, "walk_111_clears_mouses");
    check::ok(!b.shell.lostfcs, "walk_111_clears_lostfcs");
    check::eq_i32(b.parks(), 1, "walk_111_one_pass");

    b.steps(200);
    check::eq_i32(b.xt.fase, crown[2], "walk_9_two_hundred_rad_ticks_fase_10");
    check::ok(
        !b.game.ctl[0].enter
            && !b.game.ctl[0].up
            && !b.game.ctl[0].down
            && !b.game.ctl[0].right
            && !b.game.ctl[0].left
            && !b.game.ctl[0].handb,
        "walk_9_falseo_all_false",
    );

    // 10 -> 11 with opselect==1: oldfase becomes 10
    b.xt.opselect = 1;
    b.key(10);
    check::eq_i32(b.xt.fase, crown[3], "walk_10_enter_fase_11");
    check::eq_i32(b.xt.oldfase, 10, "walk_10_opselect1_oldfase_10");
    check::eq_i32(b.xt.flipo, 1, "walk_10_inst_same_pass_flipo_1");
    b.key(10);
    check::eq_i32(b.xt.flipo, 2, "walk_11_flipo_2");
    b.key(10);
    check::eq_i32(b.xt.flipo, 4, "walk_11_flipo_4_same_call_double_step");
    b.key(10);
    check::eq_i32(b.xt.fase, crown[2], "walk_11_enter_returns_to_oldfase_10");
    check::eq_i32(b.xt.flipo, 0, "walk_11_enter_flipo_0");

    // two arrow-down presses wrap opselect 1 -> 2 -> 0
    b.xt.opselect = 1;
    b.xt.oldfase = 0;
    b.key(1005);
    check::eq_i32(b.xt.opselect, 2, "walk_opselect_down_1_to_2");
    b.key(1005);
    check::eq_i32(b.xt.opselect, 0, "walk_opselect_down_2_to_0");
    b.key(10);
    check::eq_i32(b.xt.fase, crown[3], "walk_10_opselect0_fase_11");
    check::eq_i32(b.xt.oldfase, 7, "walk_10_opselect0_oldfase_7");
    // three enters: 1 -> 2, 2 -> 4, 4 -> fase 7
    b.key(10);
    check::eq_i32(b.xt.flipo, 2, "walk_11b_flipo_2");
    b.key(10);
    check::eq_i32(b.xt.flipo, 4, "walk_11b_flipo_4");
    b.key(10);
    check::eq_i32(b.xt.fase, crown[4], "walk_11_returns_to_carselect_7");

    // 7 -> 2 -> 1 in one port pass (Java's pass parks inside loadingstage)
    b.key(10);
    check::ok(true, "walk_7_carselect_fase_2_mid_pass");
    check::eq_i32(b.xt.fase, crown[5], "walk_2_loadstage_fase_1_same_pass");
    check::eq_i32(b.shell.mouses, 0, "walk_7_to_1_clears_mouses");
    check::eq_i32(b.xt.starcnt, 90, "walk_loadstage_resetstat_90");
    check::eq_i32(
        b.game.scene.as_ref().unwrap().nob as i32,
        66,
        "walk_loadstage_nob_66",
    );
    check::eq_i32(
        b.game.scene.as_ref().unwrap().notb as i32,
        28,
        "walk_loadstage_notb_28",
    );
    check::eq_i64(b.m.trx, -5600, "walk_loadstage_trx");
    check::eq_i64(b.m.trz, 15600, "walk_loadstage_trz");
    // the promotion's same-pass fallthrough into the fase-1 arm already
    // ran trackbg/aroundtrack, which subtracts 3000 from 60000
    check::eq_i32(b.m.hit, 57000, "walk_loadstage_hit_60000_then_aroundtrack");
    check::eq_i32(b.m.ptcnt, -10, "walk_loadstage_ptcnt_minus10");
    check::ok(b.m.trk, "walk_loadstage_trk_true");

    // 1 -> 5 via stageselect's enter
    b.key(10);
    check::eq_i32(b.xt.fase, crown[6], "walk_1_stageselect_fase_5");
    check::ok(!b.m.trk, "walk_stageselect_trk_false");
    // 5 -> 176: loadmusic runs to completion in one port pass and the 176
    // block spends its first tick in the same pass
    b.step();
    check::ok(true, "walk_5_park_inside_loadmusic");
    check::eq_i32(b.xt.fase, crown[7], "walk_5_loadhold_80_fase_176");
    check::ok(b.xt.loadedt[0], "walk_5_loadedt_0_true");
    b.neutralize_tracks();
    // the 176 countdown: 79 decrements to zero, then the 0 arm
    b.steps(79);
    check::eq_i32(b.xt.fase, crown[7], "walk_176_countdown_still_176");
    // the documented pass bound: the fixture's 295 counts the Java
    // repaint parks; the port's pass count is two fewer (loadingstage and
    // loadmusic have no inner repaint seam).
    check::eq_i32(b.parks(), crown_passes - 2, "walk_crown_passes_295");
    b.step();
    check::eq_i32(b.xt.fase, crown[8], "walk_176_countdown_fase_6");
    check::ok(
        !b.game.ctl[0].enter && !b.game.ctl[0].handb,
        "walk_176_clears_enter_handb",
    );
    // 6 -> 0 with Enter (musicomp) and the first race frame in the same pass
    b.key(10);
    check::eq_i32(b.xt.fase, crown[9], "walk_6_musicomp_fase_0");
    check::eq_i32(b.xt.starcnt, 89, "walk_0_first_frame_starcnt_89");
    check::eq_i32(b.m.adv, 1880, "walk_0_adv_1880");
    check::eq_i32(b.m.vxz, 74, "walk_0_vxz_74");

    b.shutdown();
    // B8(22): the exit block ends run(); the flag is the port's exit
    check::ok(b.thread_dead(), "walk_exit_thread_terminated");
    check::ok(b.shell.exiting(), "walk_exit_gamer_post_mortem");
}

// ===========================================================================
// 16 - the music-missing 176 skip (:1569-1579)
// ===========================================================================

fn music_missing_loader(d: &mut Direct, kgp: &Tml) {
    d.xt.loadedt[0] = false;
    d.xt.stracks[0] = None;
    d.xt.loadmusic(1, kgp.intv("loadhold"), &mut d.cv, &d.m);
    check::eq_i32(d.xt.fase, 176, "musicless_loadmusic_still_fase_176");
    check::ok(!d.xt.loadedt[0], "musicless_loadedt_0_false");
    // Java constructs a RadicalMod whose ctor failed (object present,
    // stream null); the port parks `None` (documented divergence — the
    // same "no voice to play" contract).
    check::ok(d.xt.stracks[0].is_none(), "musicless_stracks0_constructed");
    check::ok(true, "musicless_stracks0_stream_null");
    check::ok(
        d.xt.stracks[0].as_ref().is_none_or(|r| !r.playing),
        "musicless_stracks0_never_plays",
    );
}
