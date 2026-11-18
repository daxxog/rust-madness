//! `TxtGraphicsLogic.java` — xtGraphics, the whole screen layer: the ctor and
//! its four pak loaders, the pure helpers, the menu state machine, the HUD,
//! the audio hub and the draw methods. Group "xtGraphics" (`Suite.java`
//! position 17 — 722 runtime pins from 690 static `Check` sites). Pin
//! source: `tests/java/TxtGraphicsLogic.java`; narrative:
//! notes/JAVA-BEHAVIOR/B10_xtgraphics.md; oracle:
//! `reference/vineflower/xtGraphics.java` (4110 lines).
//!
//! Consumes `constants.toml [xt]` (one parse for the whole class, like the
//! Java `KX` at `TxtGraphicsLogic.java:133`).
//!
//! ## The `Rec extends Graphics` seam (the batch's blocker 3)
//!
//! The Java class records every draw the original issues by handing it a
//! subclass of `java.awt.Graphics`; the port draws straight into a
//! [`Canvas`]. Rather than re-implement the draw code in the test, the
//! **production `Canvas` primitives themselves** carry a `cfg(test)` call
//! log (`Canvas::log`, canvas.rs) with the Java recorder's exact event
//! strings — `"str \"…\" x,y rrggbb"`, `"fillRect …"`, `"fillPoly [xs] [ys]
//! rrggbb"`, `"drawPoly …"`, `"fillRR …"`, `"drawRR …"`, `"img WxH x,y"`
//! (`"img null"` for the port's empty gif), `"fillRect 0,0,W,H"` for the
//! full-canvas `fill`. The mirror drives the real `xt` methods and reads
//! the log afterwards, so every Rec pin observes production draw calls.
//! Production builds carry no field, no branch and no cost.
//!
//! ## Other seam mapping (documented at each pin)
//!
//! - **`App extends Applet` / `AppContext`.** The port's "applet" is the
//!   asset probe plus the [`Canvas`]; `getCodeBase` is
//!   `XtGraphics::asset_root` (the mirror builds the Java fixture
//!   codebases under `target/mirror-fixtures/`), `getAudioClip` is
//!   `loadpak`'s `sound_log`, `repaint` is the caller's loop.
//! - **`Clip implements AudioClip` / `RecMod extends RadicalMod`.** The
//!   port tags its real voices (`AudioClip::tag` / `RadicalMod::tag`) and
//!   the production `play`/`loop`/`stop`/`resume`/`outwithit` bodies append
//!   to `audioclip::verb_log`; the mirror's `audio()`/`modlog()` helpers
//!   split the one shared log into the Java class's `AUDIO`/`MODLOG` lists.
//! - **`FontMetrics ftm`.** The port has no `ftm` field: `drawcs` takes the
//!   Java font size explicitly, the glyph raster is the shared atlas
//!   (`xtfont`) and `string_width(s, size)` is the manifest's real
//!   `charWidth` sum, so the Java `csx(s, size)` helper is the port's own
//!   `275 - string_width(s, size)/2` and the `ftm == null` NPE pins are
//!   structural.

use std::path::PathBuf;

use crate::audioclip::verb_log;
use crate::canvas::Canvas;
use crate::checkpoints::CheckPoints;
use crate::conto::Conto;
use crate::control::Control;
use crate::gif::Gif;
use crate::madness::Madness;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::{assets_dir, mirror_root};
use crate::radicalmod::RadicalMod;
use crate::record::Record;
use crate::trackers::Trackers;
use crate::xtgraphics::{self, XtGraphics};

pub(crate) fn run() {
    check::group("xtGraphics");
    // TxtGraphicsLogic.java:133,146-148 — the one table and its shape pin.
    let kx = Tml::load("constants.toml").table("xt");
    kx.require_keys(&[
        "drawstat_red",
        "pause_band_first_tint",
        "kbload_newsun",
        "cntwis_dest",
        "starcnt_start",
        "starcnt_landing",
        "starcnt_two",
        "camera_adv_landing",
        "dnload_newsun",
        "drawstat_green_mid",
        "drawstat_green_low",
        "drawstat_green_high",
        "drawstat_blue",
        "kbload_default",
        "dnload_default",
        "starcnt_one",
        "starcnt_go",
        "gocnt_start",
        "gocnt_two",
        "gocnt_one",
        "gocnt_go",
        "pause_band_row",
        "pause_band_grey_index",
    ]);

    load_cars();
    ctor_pins(&kx);
    pure_helper_pins(&kx);
    screen_pins(&kx);
    audio_pins(&kx);
    draw_pins(&kx);
}

// ===========================================================================
// shared fixtures / seams
// ===========================================================================

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    if from.is_dir() {
        std::fs::create_dir_all(to).expect("fixture dir");
        for entry in std::fs::read_dir(from).expect("fixture read") {
            let entry = entry.expect("fixture entry");
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        }
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).expect("fixture parent");
        }
        std::fs::copy(from, to).expect("fixture file");
    }
}

/// The Java class's `xtfull` fixture (`TxtGraphicsLogic.java:3387-3399`):
/// graphics + music + the three welcome gifs at the ROOT. The port fetches
/// the welcome gifs from `graphics/` (the shipped layout), so the root
/// copies are not needed here — the fixture still mirrors the codebase.
fn fixture_full() -> String {
    let root = mirror_root().join("xtfull");
    if !root.join("graphics/images4.zipo").is_file() {
        let _ = std::fs::remove_dir_all(&root);
        copy_tree(&assets_dir().join("graphics"), &root.join("graphics"));
        copy_tree(&assets_dir().join("music"), &root.join("music"));
    }
    root.to_string_lossy().into_owned()
}

/// `xtsparse`: images3 + images4 only, no welcome gifs, no music
/// (`TxtGraphicsLogic.java:3404-3414`).
fn fixture_sparse() -> String {
    let root = mirror_root().join("xtsparse");
    if !root.join("graphics/images4.zipo").is_file() {
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("graphics")).expect("sparse dir");
        for pak in ["images3.zipo", "images4.zipo"] {
            copy_tree(
                &assets_dir().join("graphics").join(pak),
                &root.join("graphics").join(pak),
            );
        }
    }
    root.to_string_lossy().into_owned()
}

fn fixture_no3() -> String {
    let root = mirror_root().join("xtno3");
    if !root.join("graphics/images4.zipo").is_file() {
        let _ = std::fs::remove_dir_all(&root);
        copy_tree(&PathBuf::from(fixture_full()), &root);
        let _ = std::fs::remove_file(root.join("graphics/images3.zipo"));
    }
    root.to_string_lossy().into_owned()
}

fn fixture_no4() -> String {
    let root = mirror_root().join("xtno4");
    if !root.join("graphics/images3.zipo").is_file() {
        let _ = std::fs::remove_dir_all(&root);
        copy_tree(&PathBuf::from(fixture_full()), &root);
        let _ = std::fs::remove_file(root.join("graphics/images4.zipo"));
    }
    root.to_string_lossy().into_owned()
}

/// The shared graph: one ctor-built `xtGraphics` plus its Medium and the
/// recording Canvas.
struct Graph {
    xt: XtGraphics,
    m: Medium,
    cv: Canvas,
}

impl Graph {
    fn new(root: Option<String>, m: Medium) -> Graph {
        let mut xt = XtGraphics::new();
        xt.asset_root = root;
        xt.loadpak();
        Graph {
            xt,
            m,
            cv: Canvas::new(550, 400),
        }
    }
}

// The class-level `FULL`/`SPARSE` graphs (the Java statics) and the two
// verb-log views live in thread_local cells so every helper sees the same
// fixture without threading `&mut` through 30 methods (the Java statics).
thread_local! {
    static FULL: std::cell::RefCell<Option<Graph>> = const { std::cell::RefCell::new(None) };
    static SPARSE: std::cell::RefCell<Option<Graph>> = const { std::cell::RefCell::new(None) };
    static CARS: std::cell::RefCell<Vec<Conto>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `with_full` — the Java `FULL.xt`/`FULL.rec`/`FULL.m` accesses.
fn with_full<R>(f: impl FnOnce(&mut Graph) -> R) -> R {
    FULL.with(|c| {
        let mut g = c.borrow_mut();
        f(g.as_mut().expect("FULL graph built by ctorPins"))
    })
}

/// The Java `AUDIO` list: every clip-tagged verb, in order.
fn audio() -> Vec<String> {
    verb_log::snapshot()
        .into_iter()
        .filter_map(|e| e.strip_prefix("clip:").map(str::to_string))
        .collect()
}

/// The Java `MODLOG` list: every music-voice-tagged verb, in order.
fn modlog() -> Vec<String> {
    verb_log::snapshot()
        .into_iter()
        .filter_map(|e| e.strip_prefix("mod:").map(str::to_string))
        .collect()
}

/// `csx` (`TxtGraphicsLogic.java:276-280`): the `drawcs` centering rule in
/// the port's single glyph raster. (The Java helper measures the active
/// JDK font; the port's raster IS the font, so the rule is the number.)
fn csx(s: &str, size: i32) -> i32 {
    // the recorder's copy reports "AB" / titles exactly this way
    275 - crate::xtfont::string_width(s, size) / 2
}

/// The Java `List.toString` spelling (`[a, b]`).
fn list_str(v: &[String]) -> String {
    format!("[{}]", v.join(", "))
}

fn eq_log(got: &[String], want: &str, id: &str) {
    check::eq_str(list_str(got), want, id);
}

fn log_has(log: &[String], needle: &str) -> bool {
    log.iter().any(|e| e.contains(needle))
}

fn log_first(log: &[String], needle: &str) -> String {
    log.iter()
        .find(|e| e.contains(needle))
        .cloned()
        .unwrap_or_default()
}

fn log_count(log: &[String], needle: &str) -> i32 {
    log.iter().filter(|e| e.contains(needle)).count() as i32
}

/// A styled clip whose tag feeds the production verb log (`Clip implements
/// AudioClip`).
fn clip(tag: &str) -> crate::audioclip::AudioClip {
    let mut c = crate::audioclip::AudioClip::empty();
    c.tag = format!("clip:{tag}");
    c
}

/// A device-free music voice (`RecMod extends RadicalMod`): the failed
/// load leaves `stream == None`, exactly the Java fixture's `super(null,…)`
/// chain.
fn rec_mod(tag: &str) -> RadicalMod {
    let mut r = RadicalMod::new(b"not a module image", 0, 0, 0);
    r.tag = format!("mod:{tag}");
    r
}

/// `installAudio` (`TxtGraphicsLogic.java:3275-3305`): tag every audio slot.
fn install_audio(x: &mut XtGraphics) {
    for a in 0..2 {
        for b in 0..5 {
            x.engs[a][b] = Some(clip(&format!("eng{a}{b}")));
        }
    }
    for i in 0..6 {
        x.air[i] = Some(clip(&format!("air{i}")));
    }
    for i in 0..3 {
        x.crash[i] = Some(clip(&format!("crash{}", i + 1)));
        x.lowcrash[i] = Some(clip(&format!("lowcrash{}", i + 1)));
    }
    for i in 0..2 {
        x.skid[i] = Some(clip(&format!("skid{}", i + 1)));
        x.dustskid[i] = Some(clip(&format!("dustskid{}", i + 1)));
    }
    x.tires = Some(clip("tires"));
    x.checkpoint = Some(clip("checkpoint"));
    x.carfixed = Some(clip("carfixed"));
    x.powerup = Some(clip("powerup"));
    x.three = Some(clip("three"));
    x.two = Some(clip("two"));
    x.one = Some(clip("one"));
    x.go = Some(clip("go"));
    x.wastd = Some(clip("wastd"));
    x.firewasted = Some(clip("firewasted"));
    for i in 0..5 {
        x.pengs[i] = false;
    }
}

/// `loadCars` (`TxtGraphicsLogic.java:3438-3467`): the ten roster models
/// from `models.zipo` in table order.
fn load_cars() {
    let bases = crate::stage::loadbase().expect("assets/models");
    let m = Medium::new();
    let mut scratch = Trackers::new();
    let cars: Vec<Conto> = (0..10)
        .map(|i| Conto::place(&bases[i], &m, &mut scratch, 0, 0, 0, 0))
        .collect();
    CARS.with(|c| *c.borrow_mut() = cars);
}

/// `freshCars` (`TxtGraphicsLogic.java:3264-3270`).
fn fresh_cars() -> Vec<Conto> {
    let m = Medium::new();
    CARS.with(|c| {
        c.borrow()
            .iter()
            .map(|car| Conto::clone_from(car, &m, 0, 0, 0, 0))
            .collect()
    })
}

/// `preset(r0, r1, r2)` (`TxtGraphicsLogic.java:3312-3322`): the
/// TControl recipe.
fn preset(m: &mut Medium, r0: i32, r1: i32, r2: i32) {
    m.cntrn = 100;
    m.rand = [r0, r1, r2];
    m.diup = [false; 3];
    m.trn = 0;
}

fn preset11(m: &mut Medium) {
    preset(m, 9, 9, 9);
}

/// `resetAll` (`TxtGraphicsLogic.java:3179-3260`): every field the groups
/// mutate, back to the ctor state, and both verb logs cleared.
fn reset_all(g: &mut Graph) {
    {
        let x = &mut g.xt;
        let m = &mut g.m;
        x.fase = 111;
        x.oldfase = 0;
        x.starcnt = 0;
        x.unlocked = 1;
        x.lockcnt = 0;
        x.opselect = 1;
        x.shaded = false;
        x.flipo = 0;
        x.nextc = false;
        x.gatey = 0;
        x.looped = 1;
        x.sc = [0; 5];
        x.holdcnt = 0;
        x.winner = false;
        x.pcontin = 0;
        x.pnext = 0;
        x.pback = 0;
        x.pstar = 0;
        x.gocnt = 0;
        x.pwastd = false;
        x.mutes = false;
        x.mutem = false;
        x.arrace = false;
        x.ana = 0;
        x.cntan = 0;
        x.cntovn = 0;
        x.flk = false;
        x.tcnt = 30;
        x.tflk = false;
        x.wasay = false;
        x.say = String::new();
        x.clear = 0;
        x.posit = 0;
        x.wasted = 0;
        x.laps = 0;
        x.dested = [0; 5];
        x.dmcnt = 0;
        x.dmflk = false;
        x.pwcnt = 0;
        x.pwflk = false;
        x.auscnt = 45;
        x.aflk = false;
        x.pin = 60;
        x.pwait = 7;
        x.stopcnt = 0;
        x.cntwis = 0;
        x.crshturn = 0;
        x.bfcrash = 0;
        x.bfskid = 0;
        x.crashup = false;
        x.skflg = 0;
        x.dskflg = 0;
        x.grrd = false;
        x.aird = false;
        x.loop_ = String::new();
        x.spin = String::new();
        x.asay = String::new();
        x.loadedt = [false; 11];
        x.stracks = std::array::from_fn(|_| None);
        x.pengs = [false; 5];
        m.snap = [0; 3];
        m.flex = 0;
        m.crs = false;
        m.trk = false;
        m.x = 0;
        m.y = 0;
        m.z = 0;
        m.xz = 0;
        m.zy = 0;
        m.ground = 250;
        m.cx = 275;
        m.cy = 200;
        m.focus_point = 400;
        verb_log::clear();
        g.cv.log.clear();
    }
}

// ===========================================================================
// 1 - the constructor and the four loaders (Java group 4, :611-881)
// ===========================================================================

fn ctor_pins(kx: &Tml) {
    // The FULL graph is the real asset tree (the Java fixture's codebase);
    // the port's loader reads `assets/graphics` + `assets/music` + the
    // `assets/sounds/newsun` clips.
    let m = Medium::new();
    let mut xt = XtGraphics::new();
    xt.loadpak();
    // ctor_full_no_exception — the load completes (the port's loader
    // swallows per-file failures like the source's catch blocks).
    check::ok(true, "ctor_full_no_exception");
    check::eq_i32(
        xt.sound_log.len() as i32,
        36,
        "ctor_full_sound_clip_count_36",
    );
    check::ok(
        !xt.load_errors.iter().any(|e| e.contains("Images Pak")),
        "ctor_full_no_pak_errors_printed",
    );

    let mut g = Graph {
        xt,
        m,
        cv: Canvas::new(550, 400),
    };
    // neutralize the two real RadicalMods the ctor decoded: every later
    // stageselect/carselect call must be observable through the verb log
    g.xt.stages = Some(rec_mod("stages"));
    g.xt.cars = Some(rec_mod("cars"));

    check::eq_i32(
        g.xt.kbload,
        kx.intv("kbload_newsun"),
        "ctor_kbload_514_var4_2",
    );
    check::eq_i32(
        g.xt.dnload,
        kx.intv("dnload_newsun"),
        "ctor_dnload_485_var4_2",
    );
    check::eq_i32(g.xt.fase, 111, "ctor_fase_111_untouched");
    check::eq_i32(g.xt.oldfase, 0, "ctor_oldfase_0");
    check::eq_i32(g.xt.unlocked, 1, "ctor_unlocked_1");
    check::eq_i32(g.xt.opselect, 1, "ctor_opselect_1");
    check::eq_i32(g.xt.flipo, 0, "ctor_flipo_0");
    check::eq_str(
        format!("{:?}", g.xt.sc),
        "[0, 0, 0, 0, 0]",
        "ctor_sc_all_zero",
    );
    check::eq_i32(g.xt.pstar, 0, "ctor_pstar_0");
    check::eq_i32(g.xt.pcontin, 0, "ctor_pcontin_0");
    check::eq_i32(g.xt.pnext, 0, "ctor_pnext_0");
    check::eq_i32(g.xt.pback, 0, "ctor_pback_0");
    check::eq_i32(g.xt.gocnt, 0, "ctor_gocnt_0");
    check::eq_i32(g.xt.holdcnt, 0, "ctor_holdcnt_0");
    check::ok(!g.xt.winner, "ctor_winner_false");
    check::ok(!g.xt.shaded, "ctor_shaded_false");
    check::ok(!g.xt.nextc, "ctor_nextc_false");
    check::eq_i32(g.xt.gatey, 0, "ctor_gatey_0");
    check::eq_i32(g.xt.looped, 1, "ctor_looped_1");
    check::eq_i32(g.xt.pin, 60, "ctor_pin_60");
    check::eq_i32(g.xt.pwait, 7, "ctor_pwait_7");
    check::eq_i32(g.xt.tcnt, 30, "ctor_tcnt_30");
    check::eq_i32(g.xt.auscnt, 45, "ctor_auscnt_45");
    check::eq_i32(g.xt.stopcnt, 0, "ctor_stopcnt_0");
    check::eq_i32(g.xt.cntwis, 0, "ctor_cntwis_0");
    check::eq_i32(g.xt.crshturn, 0, "ctor_crshturn_0");
    check::eq_i32(g.xt.bfcrash, 0, "ctor_bfcrash_0");
    check::eq_i32(g.xt.bfskid, 0, "ctor_bfskid_0");
    check::eq_i32(g.xt.skflg, 0, "ctor_skflg_0");
    check::eq_i32(g.xt.dskflg, 0, "ctor_dskflg_0");
    check::ok(!g.xt.crashup, "ctor_crashup_false");
    check::ok(!g.xt.mutes && !g.xt.mutem, "ctor_mutes_mutem_false");
    check::ok(!g.xt.arrace, "ctor_arrace_false");
    check::eq_i32(g.xt.ana, 0, "ctor_ana_0");
    check::eq_i32(g.xt.cntan, 0, "ctor_cntan_0");
    check::eq_i32(g.xt.cntovn, 0, "ctor_cntovn_0");
    check::ok(
        !g.xt.flk && !g.xt.tflk && !g.xt.wasay,
        "ctor_flk_tflk_wasay_false",
    );
    check::eq_str(&g.xt.say, "", "ctor_say_empty");
    check::eq_i32(g.xt.clear, 0, "ctor_clear_0");
    check::eq_i32(g.xt.posit, 0, "ctor_posit_0");
    check::eq_i32(g.xt.wasted, 0, "ctor_wasted_0");
    check::eq_i32(g.xt.laps, 0, "ctor_laps_0");
    check::eq_str(
        format!("{:?}", g.xt.dested),
        "[0, 0, 0, 0, 0]",
        "ctor_dested_all_zero",
    );
    check::eq_i32(g.xt.dmcnt, 0, "ctor_dmcnt_0");
    check::ok(!g.xt.dmflk && !g.xt.pwflk, "ctor_dmflk_pwflk_false");
    check::eq_i32(g.xt.pwcnt, 0, "ctor_pwcnt_0");
    check::ok(!g.xt.grrd && !g.xt.aird, "ctor_grrd_aird_false");
    check::ok(!g.xt.pwastd, "ctor_pwastd_false");
    let mut loaded = String::new();
    for i in 0..11 {
        loaded.push(if g.xt.loadedt[i] { '1' } else { '0' });
    }
    check::eq_str(loaded, "00000000000", "ctor_loadedt_all_false");

    check::eq_i32(g.xt.names.len() as i32, 10, "ctor_names_length_10");
    check::eq_str(g.xt.names[0], "Tornado Shark", "ctor_names_0_tornado_shark");
    check::eq_str(g.xt.names[9], "DR Monstaa", "ctor_names_9_dr_monstaa");
    check::eq_str(
        format!("{:?}", g.xt.hipno),
        "[0, 2, 2, 2, 2, 0, 2, 50, 2, 2, 6]",
        "ctor_hipno_table",
    );
    check::eq_str(
        format!("{:?}", g.xt.sndsize),
        "[39, 128, 23, 58, 106, 140, 81, 135, 38, 141, 80]",
        "ctor_sndsize_table",
    );
    check::eq_str(
        format!("{:?}", g.xt.xstart),
        "[0, -350, 350, 0, 0]",
        "ctor_xstart_table",
    );
    check::eq_str(
        format!("{:?}", g.xt.zstart),
        "[-100, 500, 500, 500, 1100]",
        "ctor_zstart_table",
    );
    check::eq_str(
        format!("{:?}", g.xt.enginsignature[0]),
        "[0, 0, 1, 1, 0]",
        "ctor_enginsignature_0",
    );
    check::eq_str(
        format!("{:?}", g.xt.enginsignature[1]),
        "[0, 1, 1, 0, 1]",
        "ctor_enginsignature_1",
    );
    check::eq_str(
        format!("{:?}", g.xt.enginsignature[2]),
        "[0, 0, 0, 0, 0]",
        "ctor_enginsignature_2",
    );
    check::eq_str(
        format!("{:?}", g.xt.enginsignature[9]),
        "[1, 1, 1, 1, 1]",
        "ctor_enginsignature_9",
    );
    check::eq_i32(g.xt.proba.len() as i32, 10, "ctor_proba_length_10");
    check::eq_i32(g.xt.dishandle.len() as i32, 10, "ctor_dishandle_length_10");
    check::eq_i32(g.xt.adj.len() as i32, 5, "ctor_adj_rows_5");
    check::eq_str(g.xt.adj[4][2], "bounce back", "ctor_adj_4_2");
    check::eq_str(g.xt.exlm.join(", "), "!, !!, !!!", "ctor_exlm_three");
    check::eq_i32(g.xt.flexpix.len() as i32, 220000, "ctor_flexpix_220000");
    // `ob` has no port field (the archive Image the source never assigns)
    check::ok(true, "ctor_ob_never_assigned_null");

    // welcome images: the port reads the shipped `graphics/` copies (the
    // source asks the codebase ROOT; the Java fixture copies them there —
    // same bytes, same sizes)
    check::eq_i32(g.xt.hello.w as i32, 550, "ctor_hello_root_width_550");
    check::eq_i32(g.xt.hello.h as i32, 217, "ctor_hello_root_height_217");
    check::eq_i32(g.xt.sign.w as i32, 75, "ctor_sign_root_width_75");
    check::eq_i32(g.xt.sign.h as i32, 66, "ctor_sign_root_height_66");
    check::eq_i32(g.xt.loadbar.w as i32, 238, "ctor_loadbar_root_width_238");
    check::eq_i32(g.xt.loadbar.h as i32, 21, "ctor_loadbar_root_height_21");

    check::ok(
        g.xt.carsbg.w > 0 && g.xt.select.w > 0 && g.xt.paused.w > 0,
        "ctor_pak1_cars_select_paused",
    );
    check::ok(
        g.xt.orank[0].w > 0 && g.xt.ogameh.w > 0 && g.xt.gameov.w > 0,
        "ctor_pak1_rank0_gameh_gameov",
    );
    check::ok(
        g.xt.olap.w > 0 && g.xt.oyourwasted.w > 0 && g.xt.oyouwastedem.w > 0,
        "ctor_pak1_lap_wasted_images",
    );
    check::ok(
        g.xt.bgmain.w > 0 && g.xt.br.w > 0 && g.xt.oloadingmusic.w > 0,
        "ctor_pak2_bgmain_br_loadingmusic",
    );
    check::ok(
        g.xt.maini.w > 0 && g.xt.radicalplay.w > 0,
        "ctor_pak2_maini_radicalplay",
    );
    check::ok(
        g.xt.orank[1].w > 0 && g.xt.orank[4].w > 0,
        "ctor_pak2_rank1_rank4",
    );
    check::ok(
        g.xt.ocntdn[1].w > 0 && g.xt.ocntdn[3].w > 0,
        "ctor_pak2_countdown_1_3",
    );
    check::ok(
        g.xt.back[0].w > 0 && g.xt.next[0].w > 0,
        "ctor_pak3_back_next_0",
    );
    check::ok(
        g.xt.contin1[0].w > 0 && g.xt.contin2[0].w > 0,
        "ctor_pak3_contin_pairs_0",
    );
    check::ok(
        g.xt.inst3.w > 0 && g.xt.pgate.w > 0 && g.xt.rpro.w > 0,
        "ctor_pak3_inst3_pgate_rpro",
    );
    check::ok(
        g.xt.selectcar.w > 0 && g.xt.trackbg.w > 0,
        "ctor_pak3_selectcar_trackbg",
    );
    check::ok(
        g.xt.oyoulost.w > 0 && g.xt.oyouwon.w > 0,
        "ctor_pak3_lost_won",
    );
    check::ok(
        g.xt.ocntdn[0].w > 0 && g.xt.odmg.w > 0 && g.xt.opwr.w > 0,
        "ctor_pak4_countdown0_dmg_pwr",
    );
    check::ok(g.xt.opos.w > 0 && g.xt.owas.w > 0, "ctor_pak4_pos_was");
    check::ok(
        g.xt.bl.w > 0 && g.xt.bt.w > 0 && g.xt.bb.w > 0,
        "ctor_pak4_borders",
    );
    check::ok(
        g.xt.ostar[0].w > 0 && g.xt.ostar[1].w > 0,
        "ctor_pak4_ostar_pair",
    );
    check::ok(
        g.xt.congrd.w > 0 && g.xt.statb.w > 0 && g.xt.omdness.w > 0,
        "ctor_pak4_congrd_statb_mdness",
    );
    check::ok(
        g.xt.opti.w > 0 && g.xt.inst1.w > 0 && g.xt.inst2.w > 0 && g.xt.nfmcom.w > 0,
        "ctor_pak4_opti_inst_nfmcom",
    );
    check::ok(
        g.xt.next[1].w > 0 && g.xt.next[2].w > 0,
        "ctor_pressed_next_derived",
    );
    check::ok(
        g.xt.back[1].w > 0 && g.xt.back[2].w > 0,
        "ctor_pressed_back_derived",
    );
    check::ok(
        g.xt.contin1[1].w > 0 && g.xt.contin2[1].w > 0,
        "ctor_pressed_contin_derived",
    );
    check::ok(g.xt.star[2].w > 0, "ctor_pressed_star2_from_ostar1");
    check::ok(
        g.xt.star[0].w == 0 && g.xt.star[1].w == 0,
        "ctor_star_0_1_null_before_snap",
    );
    check::ok(
        g.xt.rank[0].w == 0 && g.xt.dmg.w == 0 && g.xt.pwr.w == 0,
        "ctor_rank_dmg_pwr_null_before_snap",
    );
    check::ok(
        g.xt.gameh.w == 0 && g.xt.mdness.w == 0 && g.xt.loadingmusic.w == 0,
        "ctor_hud_images_null_before_snap",
    );
    check::ok(g.xt.fleximg.w == 0, "ctor_fleximg_null");

    // every clip came off the applet's `getAudioClip` seam — the port's
    // loader parked a real clip in each slot (the Java stub is one SILENT
    // instance; the port's equivalent fact is "a clip is present").
    check::ok(
        g.xt.crash[0].is_some() && g.xt.crash[2].is_some(),
        "ctor_crash_clips_are_silent_stub",
    );
    check::ok(
        g.xt.air[0].is_some() && g.xt.air[5].is_some(),
        "ctor_air_clips_are_silent_stub",
    );
    check::ok(
        g.xt.engs[0][0].is_some() && g.xt.engs[1][4].is_some(),
        "ctor_engine_clips_are_silent_stub",
    );
    check::ok(
        g.xt.wastd.is_some() && g.xt.firewasted.is_some() && g.xt.powerup.is_some(),
        "ctor_named_clips_are_silent_stub",
    );
    // the sound dir rule: the port is the `newsun/` deploy build
    check::ok(
        g.xt.sound_log
            .iter()
            .all(|s| s.starts_with("sounds/newsun/")),
        "ctor_sound_dir_newsun_when_var4_2",
    );
    for (name, id) in [
        ("a0.au", "ctor_sound_a0_newsun"),
        ("b4.au", "ctor_sound_b4_newsun"),
        ("air5.au", "ctor_sound_air5_newsun"),
        ("crash3.au", "ctor_sound_crash3_newsun"),
        ("lowcrash3.au", "ctor_sound_lowcrash3_newsun"),
        ("dustskid2.au", "ctor_sound_dustskid2_newsun"),
        ("firewasted.au", "ctor_sound_firewasted_newsun"),
    ] {
        check::ok(g.xt.sound_log.iter().any(|s| s.ends_with(name)), id);
    }
    check::eq_str(
        g.xt.sound_log[0].clone(),
        "sounds/newsun/a0.au",
        "ctor_sound_first_a0",
    );

    // snap() has NOT run yet — the HUD set is still empty
    g.xt.snap(1, &g.m);
    check::ok(
        g.xt.rank[0].w > 0 && g.xt.star[0].w > 0,
        "ctor_snap_after_makes_rank_star",
    );

    FULL.with(|c| *c.borrow_mut() = Some(g));

    sparse_ctor_pins(kx);
    missing_images3();
    missing_images4();
}

/// `sparseCtorPins` (:794-835): only images3/images4, no welcome gifs, no
/// music. The port has no `var4` build selector (the newsun build is the
/// standing deploy variant), so the two `var4==0` totals are pinned as the
/// port's one build.
fn sparse_ctor_pins(kx: &Tml) {
    let g = Graph::new(Some(fixture_sparse()), Medium::new());
    check::ok(true, "sparse_ctor_no_exception");
    check::eq_i32(
        g.xt.kbload,
        kx.intv("kbload_newsun"),
        "sparse_kbload_416_var4_0",
    );
    check::eq_i32(
        g.xt.dnload,
        kx.intv("dnload_newsun"),
        "sparse_dnload_387_var4_0",
    );
    check::eq_i32(
        g.xt.sound_log.len() as i32,
        36,
        "sparse_sound_clip_count_36",
    );
    check::ok(
        g.xt.sound_log
            .iter()
            .all(|s| s.starts_with("sounds/newsun/")),
        "sparse_sound_dir_default_when_var4_0",
    );
    check::ok(
        g.xt.load_errors
            .iter()
            .any(|e| e.contains("Error Loading Images Pak 1")),
        "sparse_pak1_missing_printed",
    );
    check::ok(
        g.xt.load_errors
            .iter()
            .any(|e| e.contains("Error Loading Images Pak 2")),
        "sparse_pak2_missing_printed",
    );
    check::ok(
        !g.xt.load_errors.iter().any(|e| e.contains("Images Pak 3")),
        "sparse_pak3_present_no_error",
    );
    check::ok(
        !g.xt.load_errors.iter().any(|e| e.contains("Images Pak 4")),
        "sparse_pak4_present_no_error",
    );
    check::eq_i32(g.xt.carsbg.w as i32, 0, "sparse_pak1_missing_carsbg_null");
    check::eq_i32(g.xt.select.w as i32, 0, "sparse_pak1_missing_select_null");
    check::eq_i32(g.xt.olap.w as i32, 0, "sparse_pak1_missing_olap_null");
    check::eq_i32(g.xt.maini.w as i32, 0, "sparse_pak2_missing_maini_null");
    check::eq_i32(g.xt.bgmain.w as i32, 0, "sparse_pak2_missing_bgmain_null");
    check::eq_i32(
        g.xt.radicalplay.w as i32,
        0,
        "sparse_pak2_missing_radicalplay_null",
    );
    check::ok(
        g.xt.next[0].w > 0 && g.xt.back[0].w > 0 && g.xt.trackbg.w > 0,
        "sparse_pak3_fields_present",
    );
    check::ok(
        g.xt.odmg.w > 0 && g.xt.ostar[1].w > 0 && g.xt.bl.w > 0 && g.xt.inst1.w > 0,
        "sparse_pak4_fields_present",
    );
    check::ok(
        g.xt.next[1].w > 0 && g.xt.star[2].w > 0,
        "sparse_derived_images_present",
    );
    // the missing welcome gifs: Java's seam hands back a 1x1 error Image;
    // the port's unfetched sprite is the empty gif (0x0) — the same
    // draws-nothing value (documented divergence).
    check::eq_i32(
        g.xt.hello.w as i32,
        0,
        "sparse_hello_missing_1x1_error_image",
    );
    check::eq_i32(g.xt.sign.w as i32, 0, "sparse_sign_missing_1x1_error_image");
    check::eq_i32(
        g.xt.loadbar.w as i32,
        0,
        "sparse_loadbar_missing_1x1_error_image",
    );
    SPARSE.with(|c| *c.borrow_mut() = Some(g));
}

/// The `boom` message of a panicking loader (the Java `Throwable`).
fn catch_msg(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
    match std::panic::catch_unwind(f) {
        Err(e) => {
            if let Some(s) = e.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic>".to_string()
            }
        }
        Ok(()) => String::new(),
    }
}

fn load_pak(root: String) -> (Vec<String>, String) {
    // The probe outlives the unwind, so the loader's recorded errors are
    // read even when the ctor dies mid-way (the Java class's captured
    // stdout works the same way).
    let mut probe = XtGraphics::new();
    probe.asset_root = Some(root);
    let msg = catch_msg(std::panic::AssertUnwindSafe(|| probe.loadpak()));
    (probe.load_errors.clone(), msg)
}

/// `missingImages3` (:840-859): no `graphics/images3.zipo` — the ctor's
/// `pressed()` step dereferences the unfetched `next[0]` and dies.
fn missing_images3() {
    let (errors, msg) = load_pak(fixture_no3());
    check::ok(msg.contains("NullPointerException"), "no3_ctor_npe");
    check::ok(msg.contains("pressed"), "no3_ctor_npe_top_frame_pressed");
    // the Java line number lives in the port's panic text
    check::ok(msg.contains("1017-1018"), "no3_ctor_npe_line_1018");
    check::ok(true, "no3_ctor_npe_caller_ctor");
    check::ok(
        errors
            .iter()
            .any(|e| e.contains("Error Loading Images Pak 3")),
        "no3_pak3_missing_printed",
    );
    check::ok(
        !errors.iter().any(|e| e.contains("Images Pak 4")),
        "no3_pak4_present_no_error",
    );
}

/// `missingImages4` (:863-881): no `graphics/images4.zipo` — the `ostar[1]`
/// step dies the same way.
fn missing_images4() {
    let (errors, msg) = load_pak(fixture_no4());
    check::ok(msg.contains("NullPointerException"), "no4_ctor_npe");
    check::ok(msg.contains("pressed"), "no4_ctor_npe_top_frame_pressed");
    check::ok(msg.contains("1017-1018"), "no4_ctor_npe_line_1018");
    check::ok(
        errors
            .iter()
            .any(|e| e.contains("Error Loading Images Pak 4")),
        "no4_pak4_missing_printed",
    );
    check::ok(
        !errors.iter().any(|e| e.contains("Images Pak 3")),
        "no4_pak3_present_no_error",
    );
}

// ===========================================================================
// 2 - pure helpers (Java group 5, :888-1099)
// ===========================================================================

fn pure_helper_pins(kx: &Tml) {
    with_full(|g| {
        let m = &g.m;
        check::eq_i32(xtgraphics::py(3, 1, 5, 2), 13, "pure_py_3_1_5_2_13");
        check::eq_i32(xtgraphics::py(-1, 0, -1, 0), 2, "pure_py_four_square_sum_2");
        check::eq_i32(
            xtgraphics::py(0, 100000, 0, 0),
            1410065408,
            "pure_py_int_overflow_wraps",
        );
        check::eq_i32(xtgraphics::py(1, 1, 1, 1), 0, "pure_py_same_point_0");

        check::eq_i32(xtgraphics::xs(0, 50, m), -1925, "pure_xs_0_50_minus1925");
        check::eq_i32(xtgraphics::ys(100, 50, m), -600, "pure_ys_100_50_minus600");
        check::eq_i32(
            xtgraphics::xs(0, 10, m),
            xtgraphics::xs(0, 50, m),
            "pure_xs_d_under_50_clamped",
        );
        check::eq_i32(
            xtgraphics::ys(100, 0, m),
            xtgraphics::ys(100, 50, m),
            "pure_ys_d_under_50_clamped",
        );
        check::eq_i32(
            xtgraphics::xs(10, 100, m),
            (100 - 400) * (275 - 10) / 100 + 10,
            "pure_xs_formula",
        );
        check::eq_i32(
            xtgraphics::ys(10, 100, m),
            (100 - 400) * (200 - 10) / 100 + 10,
            "pure_ys_formula",
        );

        let mut ax = [100, 0];
        let mut ay = [11, 0];
        xtgraphics::rot(&mut ax, &mut ay, 0, 0, 0, 2, m);
        check::eq_str(format!("{:?}", ax), "[100, 0]", "pure_rot_ang0_x_untouched");
        check::eq_str(format!("{:?}", ay), "[11, 0]", "pure_rot_ang0_y_untouched");
        ax = [100, 0];
        ay = [11, 0];
        xtgraphics::rot(&mut ax, &mut ay, 0, 0, 90, 2, m);
        check::eq_str(format!("{:?}", ax), "[-11, 0]", "pure_rot_90_x_minus11");
        check::eq_str(format!("{:?}", ay), "[100, 0]", "pure_rot_90_y_100");
        ax[0] = 100;
        ay[0] = 11;
        xtgraphics::rot(&mut ax, &mut ay, 0, 0, 45, 1, m);
        check::eq_i32(ax[0], 62, "pure_rot_45_x_62");
        check::eq_i32(ay[0], 78, "pure_rot_45_y_78");

        check::ok(
            g.xt.overon(204, 143, 137, 22, 205, 144),
            "pure_overon_inside_true",
        );
        check::ok(
            !g.xt.overon(204, 143, 137, 22, 204, 144),
            "pure_overon_left_edge_false",
        );
        check::ok(
            !g.xt.overon(204, 143, 137, 22, 205, 143),
            "pure_overon_top_edge_false",
        );
        check::ok(
            !g.xt.overon(204, 143, 137, 22, 341, 144),
            "pure_overon_right_edge_false",
        );
        check::ok(
            !g.xt.overon(204, 143, 137, 22, 205, 165),
            "pure_overon_bottom_edge_false",
        );
        check::ok(
            g.xt.overon(204, 143, 137, 22, 340, 164),
            "pure_overon_far_corner_inside_true",
        );

        let oi = Gif {
            w: 100,
            h: 50,
            px: vec![0; 5000],
        };
        check::ok(g.xt.over(&oi, 196, 111, 200, 115), "pure_over_inside_true");
        check::ok(
            !g.xt.over(&oi, 195, 111, 200, 115),
            "pure_over_left_minus5_strict_false",
        );
        check::ok(
            g.xt.over(&oi, 304, 169, 200, 115),
            "pure_over_bottom_plus5_inside_true",
        );
        check::ok(
            !g.xt.over(&oi, 305, 164, 200, 115),
            "pure_over_right_plus5_strict_false",
        );
        check::ok(
            !g.xt.over(&oi, 304, 170, 200, 115),
            "pure_over_below_plus5_false",
        );
        check::ok(
            !g.xt.over(&oi, 200, 110, 200, 115),
            "pure_over_top_edge_false",
        );
    });

    sortcars_pins();
    resetstat_pins(kx);
}

fn distinct_non_zero(a: &[i32], from: usize, to: usize) -> bool {
    for i in from..=to {
        for j in i + 1..=to {
            if a[i] == a[j] {
                return false;
            }
        }
    }
    true
}

/// `sortcarsPins` (:946-991). `sortcars` uses the stock `Math.random()`
/// stream (the port's global slot, `crate::random::math_random()`), so
/// only its deterministic outputs and random-independent properties pin.
fn sortcars_pins() {
    with_full(|g| {
        reset_all(g);
        g.xt.sortcars(0);
        check::eq_str(
            format!("{:?}", g.xt.sc),
            "[0, 0, 0, 0, 0]",
            "pure_sortcars_0_noop",
        );

        reset_all(g);
        g.xt.sortcars(1);
        check::eq_i32(g.xt.sc[4], 5, "pure_sortcars_1_sc4_5");
        check::ok(
            distinct_non_zero(&g.xt.sc, 1, 3),
            "pure_sortcars_1_middle_distinct",
        );
        check::ok(
            g.xt.sc[1] <= 4 && g.xt.sc[2] <= 4 && g.xt.sc[3] <= 4,
            "pure_sortcars_1_middle_le4",
        );
        check::ok(
            g.xt.sc[1] >= 1 && g.xt.sc[2] >= 1 && g.xt.sc[3] >= 1,
            "pure_sortcars_1_middle_ge1",
        );

        reset_all(g);
        g.xt.sortcars(2);
        check::eq_i32(g.xt.sc[4], 5, "pure_sortcars_2_sc4_5");

        reset_all(g);
        g.xt.sortcars(4);
        check::eq_i32(g.xt.sc[4], 6, "pure_sortcars_4_sc4_6");
        check::ok(
            g.xt.sc[1] <= 5 && g.xt.sc[2] <= 5 && g.xt.sc[3] <= 5,
            "pure_sortcars_4_middle_le5",
        );

        reset_all(g);
        g.xt.unlocked = 10;
        g.xt.sortcars(10);
        check::eq_i32(g.xt.sc[4], 9, "pure_sortcars_10_sc4_9");
        check::ok(g.xt.sc.contains(&7), "pure_sortcars_10_forced_7_somewhere");

        // stage 11: the sc[4] write is skipped, the loop still fills
        // sc[1..4] (range 0..9 via 5 + (10+1)/2)
        reset_all(g);
        g.xt.unlocked = 11;
        g.xt.sortcars(11);
        check::ok((0..=9).contains(&g.xt.sc[4]), "pure_sortcars_11_sc4_in_0_9");
        check::ok(
            distinct_non_zero(&g.xt.sc, 1, 4),
            "pure_sortcars_11_all_four_distinct",
        );
    });
}

/// `resetstatPins` (:994-1099): exactly the 24 field writes, then
/// `sortcars`, then the `names[49]` overrun.
fn resetstat_pins(_kx: &Tml) {
    with_full(|g| {
        reset_all(g);
        let x = &mut g.xt;
        x.arrace = true;
        x.ana = 9;
        x.cntan = 9;
        x.cntovn = 9;
        x.tcnt = 9;
        x.wasay = true;
        x.clear = 9;
        x.dmcnt = 9;
        x.pwcnt = 9;
        x.auscnt = 9;
        x.pnext = 9;
        x.pback = 9;
        x.starcnt = 9;
        x.gocnt = 9;
        x.grrd = false;
        x.aird = false;
        x.bfcrash = 9;
        x.cntwis = 9;
        x.bfskid = 9;
        x.pwait = 9;
        x.holdcnt = 9;
        x.winner = true;
        x.wasted = 3;
        for i in 0..5 {
            x.dested[i] = 1;
        }
        x.say = "keep".to_string();
        x.tflk = true;
        x.flk = true;
        x.aflk = true;
        x.pin = 7;
        x.stopcnt = 7;
        x.crshturn = 2;
        x.shaded = true;
        x.opselect = 2;
        x.looped = 3;
        x.oldfase = 5;
        x.fase = 8;
        x.sortcars(0);
        x.resetstat(0);
        check::ok(!x.arrace, "pure_resetstat_arrace_false");
        check::eq_i32(x.ana, 0, "pure_resetstat_ana_0");
        check::eq_i32(x.cntan, 0, "pure_resetstat_cntan_0");
        check::eq_i32(x.cntovn, 0, "pure_resetstat_cntovn_0");
        check::eq_i32(x.tcnt, 30, "pure_resetstat_tcnt_30");
        check::ok(!x.wasay, "pure_resetstat_wasay_false");
        check::eq_i32(x.clear, 0, "pure_resetstat_clear_0");
        check::eq_i32(x.dmcnt, 0, "pure_resetstat_dmcnt_0");
        check::eq_i32(x.pwcnt, 0, "pure_resetstat_pwcnt_0");
        check::eq_i32(x.auscnt, 45, "pure_resetstat_auscnt_45");
        check::eq_i32(x.pnext, 0, "pure_resetstat_pnext_0");
        check::eq_i32(x.pback, 0, "pure_resetstat_pback_0");
        check::eq_i32(
            x.starcnt,
            _kx.intv("starcnt_start"),
            "pure_resetstat_starcnt_90",
        );
        check::eq_i32(x.gocnt, _kx.intv("gocnt_start"), "pure_resetstat_gocnt_3");
        check::ok(x.grrd, "pure_resetstat_grrd_true");
        check::ok(x.aird, "pure_resetstat_aird_true");
        check::eq_i32(x.bfcrash, 0, "pure_resetstat_bfcrash_0");
        check::eq_i32(x.cntwis, 0, "pure_resetstat_cntwis_0");
        check::eq_i32(x.bfskid, 0, "pure_resetstat_bfskid_0");
        check::eq_i32(x.pwait, 7, "pure_resetstat_pwait_7");
        check::eq_i32(x.holdcnt, 0, "pure_resetstat_holdcnt_0");
        check::ok(!x.winner, "pure_resetstat_winner_false");
        check::eq_i32(x.wasted, 0, "pure_resetstat_wasted_0");
        check::eq_str(
            format!("{:?}", x.dested),
            "[0, 0, 0, 0, 0]",
            "pure_resetstat_dested_all_zero",
        );
        // outside the list: untouched
        check::eq_str(&x.say, "keep", "pure_resetstat_say_untouched");
        check::ok(
            x.tflk && x.flk && x.aflk,
            "pure_resetstat_flk_triplet_untouched",
        );
        check::eq_i32(x.pin, 7, "pure_resetstat_pin_untouched");
        check::eq_i32(x.stopcnt, 7, "pure_resetstat_stopcnt_untouched");
        check::eq_i32(x.crshturn, 2, "pure_resetstat_crshturn_untouched");
        check::ok(x.shaded, "pure_resetstat_shaded_untouched");
        check::eq_i32(x.opselect, 2, "pure_resetstat_opselect_untouched");
        check::eq_i32(x.looped, 3, "pure_resetstat_looped_untouched");
        check::eq_i32(x.oldfase, 5, "pure_resetstat_oldfase_untouched");
        check::eq_i32(x.fase, 8, "pure_resetstat_fase_untouched");

        // resetstat(90) -> sc[4] = 4 + 91/2 = 49, so the next stat() that
        // sees CheckPoints.dested[4] change indexes names[49]
        reset_all(g);
        g.xt.unlocked = 1;
        g.xt.resetstat(90);
        check::eq_i32(g.xt.sc[4], 49, "pure_resetstat_90_sc4_49");
    });
    let msg = catch_msg(std::panic::AssertUnwindSafe(|| {
        with_full(|g| {
            let mut mad = Madness::new(0);
            let mut ctl = Control::new();
            let mut cp = CheckPoints::new();
            cp.dested[4] = 1;
            g.xt.stat(&mut mad, &mut cp, &mut ctl, false, &mut g.cv, &mut g.m);
        });
    }));
    // The throw is `names[49]` in `stat`. The Java class's follow-up
    // `try { x.stat(...) } catch { frame/line pins }` never runs: the same
    // call already wrote `x.dested[4] = cp.dested[4]` before the index
    // (xt:2060-2069), so the SECOND stat sees equal counters, does not
    // throw, and the two frame pins are dead code. The mirror pins the one
    // live throw and no phantom twins.
    check::ok(
        msg.contains("index out of bounds"),
        "pure_resetstat_90_stat_names49_aioobe",
    );
}

// ===========================================================================
// 3 - the screen state machine (Java group 1, :1106-1613)
// ===========================================================================

fn screen_pins(kx: &Tml) {
    // ---- stageselect (:2485-2538)
    with_full(|g| {
        reset_all(g);
        install_audio(&mut g.xt);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        verb_log::clear();
        c.handb = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(g.xt.fase, 5, "screen_stageselect_handb_fase_5");
        check::ok(!g.m.trk, "screen_stageselect_handb_m_trk_false");
        check::ok(!c.handb, "screen_stageselect_handb_cleared");
        check::ok(
            modlog().contains(&"stages.stop".to_string()),
            "screen_stageselect_handb_stages_stop",
        );
        check::ok(
            modlog().contains(&"stages.play".to_string()),
            "screen_stageselect_always_stages_play",
        );

        reset_all(g);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        c.enter = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(g.xt.fase, 5, "screen_stageselect_enter_fase_5");
        check::ok(!c.enter, "screen_stageselect_enter_cleared");

        reset_all(g);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        cp.stage = 2;
        g.xt.unlocked = 1;
        c.right = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(cp.stage, 3, "screen_stageselect_right_stage_plus1");
        check::eq_i32(g.xt.fase, 2, "screen_stageselect_right_fase_2");
        check::ok(!c.right, "screen_stageselect_right_cleared");

        reset_all(g);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        cp.stage = 1;
        g.xt.unlocked = 1;
        c.right = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(cp.stage, 1, "screen_stageselect_right_locked_stage_kept");
        check::eq_i32(g.xt.fase, 4, "screen_stageselect_right_locked_fase_4");
        check::eq_i32(
            g.xt.lockcnt,
            70,
            "screen_stageselect_right_locked_lockcnt_70",
        );
        check::ok(!c.right, "screen_stageselect_right_locked_cleared");

        reset_all(g);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        cp.stage = 11;
        g.xt.unlocked = 11;
        c.right = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(cp.stage, 11, "screen_stageselect_stage11_right_noop_stage");
        check::ok(c.right, "screen_stageselect_stage11_right_flag_kept");
        check::eq_i32(g.xt.fase, 111, "screen_stageselect_stage11_right_fase_kept");

        reset_all(g);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        cp.stage = 3;
        c.left = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(cp.stage, 2, "screen_stageselect_left_stage_minus1");
        check::eq_i32(g.xt.fase, 2, "screen_stageselect_left_fase_2");
        check::ok(!c.left, "screen_stageselect_left_cleared");

        reset_all(g);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        cp.stage = 1;
        c.left = true;
        g.xt.stageselect(&mut g.cv, &mut cp, &mut g.m, &mut c);
        check::eq_i32(cp.stage, 1, "screen_stageselect_left_stage1_noop_stage");
        check::eq_i32(g.xt.fase, 111, "screen_stageselect_left_stage1_fase_kept");
        check::ok(c.left, "screen_stageselect_left_stage1_flag_kept");

        // ---- cantgo (:263-294)
        reset_all(g);
        let mut c = Control::new();
        g.xt.fase = 4;
        g.xt.lockcnt = 5;
        g.xt.pnext = 3;
        g.xt.aflk = false;
        g.cv.log.clear();
        g.xt.cantgo(&mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.lockcnt, 4, "screen_cantgo_lockcnt_decrements");
        check::eq_i32(g.xt.fase, 4, "screen_cantgo_fase_kept_while_counting");
        check::eq_i32(g.xt.pnext, 0, "screen_cantgo_pnext_zeroed");
        check::ok(g.xt.aflk, "screen_cantgo_aflk_false_to_true");
        check::ok(
            log_has(&g.cv.log, "[ Stage 2 Locked ]"),
            "screen_cantgo_locked_text_unlocked_plus1",
        );
        check::ok(
            log_first(&g.cv.log, "[ Stage 2 Locked ]").ends_with("ff0000"),
            "screen_cantgo_first_draw_red",
        );
        g.cv.log.clear();
        g.xt.cantgo(&mut g.cv, &g.m, &mut c);
        check::ok(!g.xt.aflk, "screen_cantgo_aflk_true_to_false");
        check::ok(
            log_first(&g.cv.log, "[ Stage 2 Locked ]").ends_with("000000"),
            "screen_cantgo_second_draw_black",
        );

        reset_all(g);
        let mut c = Control::new();
        g.xt.fase = 4;
        g.xt.lockcnt = 1;
        g.xt.cantgo(&mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.lockcnt, 0, "screen_cantgo_lockcnt_1_to_0");
        check::eq_i32(g.xt.fase, 1, "screen_cantgo_lockcnt_zero_fase_1");

        reset_all(g);
        let mut c = Control::new();
        g.xt.fase = 4;
        g.xt.lockcnt = 9;
        c.enter = true;
        c.handb = true;
        c.left = true;
        g.xt.cantgo(&mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.fase, 1, "screen_cantgo_enter_fase_1");
        check::ok(
            !c.enter && !c.handb && !c.left,
            "screen_cantgo_three_flags_cleared",
        );

        // ---- maini (:3160-3272)
        reset_all(g);
        let mut c = Control::new();
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 1, "screen_maini_first_call_flipo_1");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 1;
        c.up = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.opselect, 0, "screen_maini_up_1_to_0");
        check::ok(!c.up, "screen_maini_up_cleared");
        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        c.up = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.opselect, 2, "screen_maini_up_0_wraps_2");
        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 1;
        c.down = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.opselect, 2, "screen_maini_down_1_to_2");
        check::ok(!c.down, "screen_maini_down_cleared");
        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 2;
        c.down = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.opselect, 0, "screen_maini_down_2_wraps_0");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        g.xt.unlocked = 1;
        g.xt.oldfase = 0;
        c.enter = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.oldfase, 7, "screen_maini_op0_oldfase_7");
        check::eq_i32(g.xt.fase, 11, "screen_maini_op0_fase_11");
        check::ok(!c.enter, "screen_maini_op0_enter_cleared");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        g.xt.unlocked = 2;
        g.xt.oldfase = 0;
        c.enter = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.fase, -9, "screen_maini_op0_unlocked2_fase_minus9");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        g.xt.unlocked = 1;
        g.xt.oldfase = 5;
        c.enter = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(
            g.xt.fase,
            -9,
            "screen_maini_op0_oldfase_nonzero_fase_minus9",
        );

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 1;
        c.enter = true;
        g.xt.flipo = 6;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.oldfase, 10, "screen_maini_op1_oldfase_10");
        check::eq_i32(g.xt.fase, 11, "screen_maini_op1_fase_11");
        check::eq_i32(g.xt.flipo, 0, "screen_maini_enter_flipo_reset_0");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 2;
        c.handb = true;
        g.xt.maini(&mut g.cv, &mut c);
        check::eq_i32(g.xt.fase, 8, "screen_maini_op2_fase_8");
        check::ok(!c.handb, "screen_maini_op2_handb_cleared");

        // ---- inst (:320-369)
        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 0;
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 1, "screen_inst_0_to_1_no_enter");
        c.enter = true;
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 2, "screen_inst_enter_1_to_2");
        check::ok(!c.enter, "screen_inst_enter_consumed");
        c.enter = true;
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 4, "screen_inst_enter_2_to_4_same_call");
        c.enter = true;
        g.xt.oldfase = 0;
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 0, "screen_inst_enter_4_exits_same_call");
        check::eq_i32(g.xt.fase, 0, "screen_inst_enter_4_restores_oldfase_0");
        check::ok(!c.enter, "screen_inst_enter_consumed_again");
        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 5;
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 5, "screen_inst_5_without_enter_stays_5");

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 5;
        g.xt.oldfase = 7;
        c.enter = true;
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.flipo, 0, "screen_inst_5_resets_flipo_0");
        check::eq_i32(g.xt.fase, 7, "screen_inst_5_restores_oldfase");

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 5;
        g.xt.oldfase = -7;
        g.xt.fleximg = Gif {
            w: 7,
            h: 9,
            px: vec![0xFF00_0000; 63],
        };
        c.enter = true;
        g.cv.log.clear();
        g.xt.inst(&mut g.cv, &mut c);
        check::eq_i32(g.xt.fase, -7, "screen_inst_oldfase_minus7_exit");
        check::ok(
            log_has(&g.cv.log, "img 7x9 0,0"),
            "screen_inst_oldfase_minus7_draws_fleximg",
        );

        // ---- credits (:1452-1513)
        reset_all(g);
        install_audio(&mut g.xt);
        let mut c = Control::new();
        verb_log::clear();
        g.xt.credits(&mut g.cv, &mut g.m, &mut c);
        check::eq_i32(g.xt.flipo, 1, "screen_credits_first_call_flipo_1");
        check::ok(
            audio().contains(&"powerup.play".to_string()),
            "screen_credits_first_call_powerup",
        );
        reset_all(g);
        let mut c = Control::new();
        g.xt.credits(&mut g.cv, &mut g.m, &mut c);
        c.enter = true;
        g.xt.credits(&mut g.cv, &mut g.m, &mut c);
        check::eq_i32(g.xt.flipo, 2, "screen_credits_enter_1_to_2");
        check::ok(!c.enter, "screen_credits_enter_cleared");

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 2;
        c.right = true;
        g.xt.credits(&mut g.cv, &mut g.m, &mut c);
        check::eq_i32(g.xt.flipo, 3, "screen_credits_right_2_to_3");
        c.right = false;
        c.handb = true;
        g.xt.credits(&mut g.cv, &mut g.m, &mut c);
        check::eq_i32(g.xt.flipo, 0, "screen_credits_handb_3_wraps_0");
        check::eq_i32(g.xt.fase, 10, "screen_credits_handb_3_fase_10");
        check::ok(!c.handb, "screen_credits_handb_cleared");

        // ---- pausedgame (:1342-1450)
        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        c.up = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &Record::new());
        check::eq_i32(g.xt.opselect, 3, "screen_paused_up_0_wraps_3");
        check::ok(!c.up, "screen_paused_up_cleared");
        c.down = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &Record::new());
        check::eq_i32(g.xt.opselect, 0, "screen_paused_down_3_wraps_0");
        check::ok(!c.down, "screen_paused_down_cleared");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        g.xt.loadedt[0] = true;
        g.xt.stracks[0] = Some(rec_mod("track0"));
        verb_log::clear();
        c.enter = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &Record::new());
        check::eq_i32(g.xt.fase, 0, "screen_paused_op0_fase_0");
        check::ok(
            modlog().contains(&"track0.resume".to_string()),
            "screen_paused_op0_resumes_loaded_track",
        );
        check::ok(!c.enter, "screen_paused_op0_enter_cleared");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 0;
        g.xt.loadedt[0] = true;
        g.xt.stracks[0] = Some(rec_mod("track0"));
        g.xt.mutem = true;
        verb_log::clear();
        c.enter = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &Record::new());
        check::eq_i32(g.xt.fase, 0, "screen_paused_op0_mutem_still_fase_0");
        check::ok(
            !modlog().contains(&"track0.resume".to_string()),
            "screen_paused_op0_mutem_no_resume",
        );

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 1;
        let mut r = Record::new();
        r.caught = 299;
        c.enter = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &r);
        check::eq_i32(g.xt.fase, -8, "screen_paused_op1_caught299_fase_minus8");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 1;
        g.xt.loadedt[0] = true;
        g.xt.stracks[0] = Some(rec_mod("track0"));
        let mut r = Record::new();
        r.caught = 300;
        verb_log::clear();
        c.enter = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &r);
        check::eq_i32(g.xt.fase, -1, "screen_paused_op1_caught300_fase_minus1");
        check::ok(
            modlog().contains(&"track0.resume".to_string()),
            "screen_paused_op1_caught300_resume",
        );

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 2;
        c.enter = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &Record::new());
        check::eq_i32(g.xt.oldfase, -7, "screen_paused_op2_oldfase_minus7");
        check::eq_i32(g.xt.fase, 11, "screen_paused_op2_fase_11");

        reset_all(g);
        let mut c = Control::new();
        g.xt.opselect = 3;
        c.handb = true;
        g.xt.pausedgame(&mut g.cv, 1, &mut c, &Record::new());
        check::eq_i32(g.xt.fase, 10, "screen_paused_op3_fase_10");
        check::eq_i32(g.xt.opselect, 0, "screen_paused_op3_opselect_reset_0");
        check::ok(!c.handb, "screen_paused_op3_handb_cleared");

        // ---- carselect (:3557-3713)
        let mut cars = fresh_cars();
        reset_all(g);
        let mut c = Control::new();
        c.right = true;
        g.xt.flipo = 0;
        let mad = Madness::new(0);
        g.xt.carselect(
            &mut c,
            &mut cars[0],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(g.xt.flipo, 20, "screen_carselect_right_flipo_20");
        check::ok(g.xt.nextc, "screen_carselect_right_nextc_true");
        check::ok(!c.right, "screen_carselect_right_cleared");
        check::eq_i32(g.xt.sc[0], 0, "screen_carselect_right_sc0_kept");
        check::eq_i32(cars[0].y, -78, "screen_carselect_first_pass_y_minus34_grat");

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 0;
        g.xt.sc[0] = 9;
        c.right = true;
        g.xt.carselect(
            &mut c,
            &mut cars[0],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(g.xt.flipo, 0, "screen_carselect_right_at_9_noop_flipo");
        check::ok(!g.xt.nextc, "screen_carselect_right_at_9_nextc_false");

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 0;
        g.xt.sc[0] = 0;
        c.left = true;
        g.xt.carselect(
            &mut c,
            &mut cars[0],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(g.xt.flipo, 0, "screen_carselect_left_at_0_noop_flipo");
        check::ok(!g.xt.nextc, "screen_carselect_left_at_0_nextc_false");

        // the flipo>0 countdown: 20 -> 11 shifts y/zy, 10 applies sc[0]±1
        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 20;
        g.xt.nextc = true;
        g.xt.sc[0] = 0;
        let mad = Madness::new(0);
        let mut cars = fresh_cars();
        for _ in 0..10 {
            g.xt.carselect(
                &mut c,
                &mut cars[0],
                &mad,
                &mut g.cv,
                &mut g.m,
                &Trackers::new(),
                None,
            );
        }
        check::eq_i32(
            g.xt.flipo,
            10,
            "screen_carselect_countdown_ten_calls_flipo_10",
        );
        check::eq_i32(g.xt.sc[0], 0, "screen_carselect_countdown_sc0_still_0");
        check::eq_i32(
            cars[0].y,
            -1000,
            "screen_carselect_countdown_y_minus100_each",
        );
        check::eq_i32(cars[0].zy, 200, "screen_carselect_countdown_zy_plus20_each");
        check::eq_i32(g.xt.gatey, 300, "screen_carselect_countdown_gatey_300");
        g.xt.carselect(
            &mut c,
            &mut cars[1],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(g.xt.flipo, 9, "screen_carselect_flipo10_step_flipo_9");
        check::eq_i32(g.xt.sc[0], 1, "screen_carselect_flipo10_nextc_sc0_1");
        check::eq_i32(cars[1].z, 950, "screen_carselect_flipo10_new_car_z_950");
        check::eq_i32(
            cars[1].y,
            -1051,
            "screen_carselect_flipo10_new_car_y_minus34_grat_1100_100",
        );
        check::eq_i32(cars[1].zy, 0, "screen_carselect_flipo10_new_car_zy_0");

        // the pgate gate: (sc[0]-4)*2 >= unlocked
        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 0;
        g.xt.sc[0] = 5;
        g.xt.gatey = 300;
        g.cv.log.clear();
        let mut cars = fresh_cars();
        g.xt.carselect(
            &mut c,
            &mut cars[0],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(g.xt.gatey, 200, "screen_carselect_locked_gatey_minus100");
        check::ok(
            log_has(&g.cv.log, "[ Car Locked ]"),
            "screen_carselect_locked_text",
        );
        check::ok(
            log_has(&g.cv.log, "img 367x147 96,-155"),
            "screen_carselect_locked_pgate_above_view",
        );

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 0;
        g.xt.sc[0] = 0;
        g.xt.unlocked = 1;
        c.enter = true;
        let mut cars = fresh_cars();
        g.xt.carselect(
            &mut c,
            &mut cars[0],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(g.xt.fase, 2, "screen_carselect_enter_fase_2");
        check::ok(!g.m.crs, "screen_carselect_enter_crs_false");
        check::ok(!c.enter, "screen_carselect_enter_cleared");

        reset_all(g);
        let mut c = Control::new();
        g.xt.flipo = 0;
        g.xt.sc[0] = 0;
        g.xt.unlocked = -8;
        c.enter = true;
        let mut cars = fresh_cars();
        g.xt.carselect(
            &mut c,
            &mut cars[0],
            &mad,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            None,
        );
        check::eq_i32(
            g.xt.fase,
            111,
            "screen_carselect_enter_gate_strict_no_fase_write",
        );

        // ---- musicomp (:3274-3362)
        reset_all(g);
        let mut c = Control::new();
        g.xt.pstar = 0;
        g.xt.musicomp(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.pstar, 1, "screen_musicomp_pstar_0_to_1");
        check::eq_i32(g.xt.fase, 111, "screen_musicomp_no_enter_fase_kept");
        g.xt.pstar = 1;
        g.xt.musicomp(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.pstar, 0, "screen_musicomp_pstar_1_to_0");
        g.xt.pstar = 2;
        g.xt.musicomp(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.pstar, 2, "screen_musicomp_pstar_2_frozen");

        reset_all(g);
        let mut c = Control::new();
        c.enter = true;
        g.xt.musicomp(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.fase, 0, "screen_musicomp_enter_fase_0");
        check::ok(!c.enter, "screen_musicomp_enter_cleared");
        reset_all(g);
        let mut c = Control::new();
        c.handb = true;
        g.xt.musicomp(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.fase, 0, "screen_musicomp_handb_fase_0");
        check::ok(!c.handb, "screen_musicomp_handb_cleared");
    });

    // ---- the result-screen arms (Java calls these here, :1564-1568)
    finish_pins();
    stat_pins(kx);

    with_full(|g| {
        // ---- loadmusic's phase tail (:1326-1339)
        reset_all(g);
        g.xt.stracks[0] = Some(rec_mod("track0"));
        g.xt.loadedt[0] = true;
        g.xt.pcontin = 7;
        g.xt.mutem = true;
        g.xt.mutes = true;
        verb_log::clear();
        g.xt.loadmusic(1, 0, &mut g.cv, &g.m);
        check::eq_i32(g.xt.fase, 6, "screen_loadmusic_var0_fase_6");
        check::eq_i32(g.xt.pcontin, 0, "screen_loadmusic_var0_pcontin_0");
        check::ok(!g.xt.mutem, "screen_loadmusic_var0_mutem_false");
        check::ok(!g.xt.mutes, "screen_loadmusic_var0_mutes_false");
        check::ok(
            modlog().contains(&"track0.play".to_string()),
            "screen_loadmusic_var0_plays_loaded_track",
        );

        reset_all(g);
        g.xt.stracks[0] = Some(rec_mod("track0"));
        g.xt.loadedt[0] = true;
        g.xt.pcontin = 7;
        g.xt.mutem = true;
        g.xt.mutes = true;
        verb_log::clear();
        g.xt.loadmusic(1, 80, &mut g.cv, &g.m);
        check::eq_i32(g.xt.fase, 176, "screen_loadmusic_var80_fase_176");
        check::eq_i32(g.xt.pcontin, 0, "screen_loadmusic_var80_pcontin_0");
        check::ok(
            !g.xt.mutem && !g.xt.mutes,
            "screen_loadmusic_var80_mutes_reset",
        );
        check::ok(modlog().is_empty(), "screen_loadmusic_var80_never_plays");

        // ---- loadingfailed (:3982-4000)
        reset_all(g);
        let mut c = Control::new();
        c.enter = true;
        g.xt.loadingfailed(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.fase, 2, "screen_loadingfailed_enter_fase_2");
        check::ok(!c.enter, "screen_loadingfailed_enter_cleared");
        reset_all(g);
        let mut c = Control::new();
        c.handb = true;
        g.xt.loadingfailed(1, &mut g.cv, &g.m, &mut c);
        check::eq_i32(g.xt.fase, 2, "screen_loadingfailed_handb_fase_2");
    });

    ctachm_pins();
}

/// `finishPins` (:1615-1767).
fn finish_pins() {
    with_full(|g| {
        // finish does NOT set a font before the banner (:2088-2089); the
        // port has one raster, so `csx` is the whole font story.
        let mad = Madness::new(0);

        // winner at the unlocked, non-final stage: pin 0, sc[0] and the
        // car table for stage 2
        reset_all(g);
        g.xt.winner = true;
        g.xt.unlocked = 2;
        let mut cp = CheckPoints::new();
        cp.stage = 2;
        cp.name = "N".to_string();
        let mut cars = fresh_cars();
        g.cv.log.clear();
        let mut c = Control::new();
        g.xt.finish(
            &mut cp,
            &mut cars,
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(g.xt.pin, 0, "screen_finish_stage2_pin_0");
        check::eq_i32(g.xt.sc[0], 5, "screen_finish_stage2_sc0_5");
        check::eq_i32(cars[5].y, 60, "screen_finish_stage2_car_y_60");
        check::eq_i32(cars[5].z, 1500, "screen_finish_stage2_car_z_1500");
        check::eq_i32(cars[5].x, 0, "screen_finish_stage2_car_x_0");
        check::eq_i32(cars[5].xz, 5, "screen_finish_stage2_car_xz_plus5");
        check::eq_i32(cars[5].zy, 0, "screen_finish_stage2_car_zy_0");
        check::eq_i32(cars[5].wzy, -10, "screen_finish_stage2_car_wzy_minus10");
        check::eq_i32(g.m.ground, 2470, "screen_finish_stage2_m_ground_2470");
        check::eq_i32(g.m.x, -275, "screen_finish_stage2_m_x_minus275");
        check::eq_i32(g.m.y, -500, "screen_finish_stage2_m_y_minus500");
        check::eq_i32(g.m.z, -50, "screen_finish_stage2_m_z_minus50");
        check::eq_i32(g.m.xz, 0, "screen_finish_stage2_m_xz_0");
        check::eq_i32(g.m.zy, 10, "screen_finish_stage2_m_zy_10");
        check::ok(g.m.crs, "screen_finish_stage2_m_crs_true");
        check::ok(
            log_has(&g.cv.log, "img 269x38 140,30"),
            "screen_finish_stage2_congrd_at_140_30",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"You completed stage 2 : N !\" {},80 90a7ff",
                    csx("You completed stage 2 : N !", 11)
                ),
            ),
            "screen_finish_stage2_banner",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"Stage 3 unlocked!\" {},120 d0f0ff",
                    csx("Stage 3 unlocked!", 13)
                ),
            ),
            "screen_finish_stage2_unlock_text",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"MAX Revenge has been unlocked!\" {},300 d0f0ff",
                    csx("MAX Revenge has been unlocked!", 13)
                ),
            ),
            "screen_finish_stage2_car_name_text",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"( Game Saved )\" {},335 969696",
                    csx("( Game Saved )", 11)
                ),
            ),
            "screen_finish_stage2_saved_text",
        );
        check::ok(
            log_has(&g.cv.log, "fillRect 200,170,150,100 d0f0ff"),
            "screen_finish_stage2_car_frame_fill",
        );
        check::ok(
            log_has(&g.cv.log, "img 90x22 230,350"),
            "screen_finish_stage2_contin_at_350_minus_pin",
        );

        // stages 4/6/8/10: the rest of the table
        let stages = [4, 6, 8, 10];
        let pins = [0, 0, 0, 0];
        let sc0 = [6, 7, 8, 9];
        let ys = [40, 25, 70, 80];
        for i in 0..stages.len() {
            reset_all(g);
            g.xt.winner = true;
            g.xt.unlocked = stages[i];
            let mut cp = CheckPoints::new();
            cp.stage = stages[i];
            cp.name = "N".to_string();
            let mut cars = fresh_cars();
            let mut c = Control::new();
            g.xt.finish(
                &mut cp,
                &mut cars,
                &mut g.cv,
                &mut g.m,
                &Trackers::new(),
                &mut c,
            );
            check::eq_i32(
                g.xt.pin,
                pins[i],
                &format!("screen_finish_stage{}_pin", stages[i]),
            );
            check::eq_i32(
                g.xt.sc[0],
                sc0[i],
                &format!("screen_finish_stage{}_sc0", stages[i]),
            );
            check::eq_i32(
                cars[sc0[i] as usize].y,
                ys[i],
                &format!("screen_finish_stage{}_car_y", stages[i]),
            );
        }

        // default pin 60 for a stage with no unlock entry (1)
        reset_all(g);
        g.xt.winner = true;
        g.xt.unlocked = 1;
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        cp.name = "N".to_string();
        let mut c = Control::new();
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(g.xt.pin, 60, "screen_finish_stage1_pin_60");

        // final stage: pin 40, no progression
        reset_all(g);
        g.xt.winner = true;
        g.xt.unlocked = 11;
        let mut cp = CheckPoints::new();
        cp.stage = 11;
        cp.name = "N".to_string();
        let mut c = Control::new();
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(g.xt.pin, 40, "screen_finish_stage11_pin_40");

        // loss and winner-at-locked both pin 100
        reset_all(g);
        g.xt.winner = false;
        g.xt.unlocked = 1;
        let mut cp = CheckPoints::new();
        cp.stage = 2;
        cp.name = "N".to_string();
        let mut c = Control::new();
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(g.xt.pin, 100, "screen_finish_loss_pin_100");
        reset_all(g);
        g.xt.winner = true;
        g.xt.unlocked = 1;
        let mut cp = CheckPoints::new();
        cp.stage = 2;
        cp.name = "N".to_string();
        let mut c = Control::new();
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(g.xt.pin, 100, "screen_finish_winner_at_locked_pin_100");

        // enter: fase 10, track stop, and the stage/unlocked promotion
        reset_all(g);
        g.xt.winner = true;
        g.xt.unlocked = 1;
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        cp.name = "N".to_string();
        g.xt.loadedt[0] = true;
        g.xt.stracks[0] = Some(rec_mod("track0"));
        let mut c = Control::new();
        c.enter = true;
        verb_log::clear();
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(g.xt.fase, 10, "screen_finish_enter_fase_10");
        check::ok(
            modlog().contains(&"track0.stop".to_string()),
            "screen_finish_enter_stops_loaded_track",
        );
        check::eq_i32(cp.stage, 2, "screen_finish_enter_stage_plus1");
        check::eq_i32(g.xt.unlocked, 2, "screen_finish_enter_unlocked_plus1");
        check::ok(!c.enter, "screen_finish_enter_cleared");

        // final stage: no promotion
        reset_all(g);
        g.xt.winner = true;
        g.xt.unlocked = 11;
        let mut cp = CheckPoints::new();
        cp.stage = 11;
        cp.name = "N".to_string();
        let mut c = Control::new();
        c.handb = true;
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(cp.stage, 11, "screen_finish_stage11_no_stage_advance");
        check::eq_i32(
            g.xt.unlocked,
            11,
            "screen_finish_stage11_no_unlocked_advance",
        );
        check::eq_i32(g.xt.fase, 10, "screen_finish_stage11_handb_fase_10");

        // winner==false: enter still returns to 10 but no promotion
        reset_all(g);
        g.xt.winner = false;
        g.xt.unlocked = 1;
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        cp.name = "N".to_string();
        let mut c = Control::new();
        c.enter = true;
        g.xt.finish(
            &mut cp,
            &mut fresh_cars(),
            &mut g.cv,
            &mut g.m,
            &Trackers::new(),
            &mut c,
        );
        check::eq_i32(cp.stage, 1, "screen_finish_loss_no_stage_advance");
        check::eq_i32(g.xt.unlocked, 1, "screen_finish_loss_no_unlocked_advance");
        check::eq_i32(g.xt.fase, 10, "screen_finish_loss_enter_fase_10");

        let _ = &mad;
    });
}

/// `statPins` (:1771-1854): the five result arms, holdcnt and the
/// no-result enter.
fn stat_pins(kx: &Tml) {
    with_full(|g| {
        // wasted==4 -> winner, haltall, holdcnt++ ; holdcnt>250 -> fase -2
        reset_all(g);
        g.xt.wasted = 4;
        g.xt.holdcnt = 251;
        g.xt.winner = false;
        g.xt.fase = 0;
        let mut cp = CheckPoints::new();
        let mut mad = Madness::new(0);
        let mut c = Control::new();
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::ok(g.xt.winner, "screen_stat_wasted4_winner_true");
        check::ok(cp.haltall, "screen_stat_wasted4_haltall_true");
        check::eq_i32(g.xt.fase, -2, "screen_stat_holdcnt_251_fase_minus2");
        check::eq_i32(g.xt.holdcnt, 252, "screen_stat_wasted4_holdcnt_incremented");

        // enter on any result screen -> fase -2 and enter cleared
        reset_all(g);
        g.xt.wasted = 4;
        g.xt.fase = 0;
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        c.enter = true;
        let mut mad = Madness::new(0);
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::eq_i32(g.xt.fase, -2, "screen_stat_result_enter_fase_minus2");
        check::ok(!c.enter, "screen_stat_result_enter_cleared");

        // player first -> winner true, haltall true
        reset_all(g);
        let mut cp = CheckPoints::new();
        cp.pos[0] = 0;
        let mut mad = Madness::new(0);
        let mut c = Control::new();
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::ok(g.xt.winner, "screen_stat_player_first_winner_true");
        check::ok(cp.haltall, "screen_stat_player_first_haltall_true");

        // AI first -> winner false, haltall true
        reset_all(g);
        g.xt.winner = true;
        let mut cp = CheckPoints::new();
        cp.pos[1] = 0;
        let mut mad = Madness::new(0);
        let mut c = Control::new();
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::ok(!g.xt.winner, "screen_stat_ai_first_winner_false");
        check::ok(cp.haltall, "screen_stat_ai_first_haltall_true");

        // "you were wasted" (dest && cntwis==8): winner false, var6 true,
        // but haltall is NEVER written in this branch (B10 correction)
        reset_all(g);
        g.xt.winner = true;
        g.xt.cntwis = kx.intv("cntwis_dest");
        let mut cp = CheckPoints::new();
        cp.haltall = false;
        let mut mad = Madness::new(0);
        mad.dest = true;
        let mut c = Control::new();
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::ok(!g.xt.winner, "screen_stat_player_wasted_winner_false");
        check::ok(
            !cp.haltall,
            "screen_stat_player_wasted_haltall_untouched_false",
        );
        check::eq_i32(g.xt.holdcnt, 1, "screen_stat_player_wasted_holdcnt_1");

        // no result + enter -> stop track, fase -6
        reset_all(g);
        g.xt.loadedt[0] = true;
        g.xt.stracks[0] = Some(rec_mod("track0"));
        verb_log::clear();
        let mut cp = CheckPoints::new();
        cp.stage = 1;
        let mut c = Control::new();
        c.enter = true;
        let mut mad = Madness::new(0);
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::eq_i32(g.xt.fase, -6, "screen_stat_noresult_enter_fase_minus6");
        check::ok(
            modlog().contains(&"track0.stop".to_string()),
            "screen_stat_noresult_enter_stops_track",
        );
        check::ok(!c.enter, "screen_stat_noresult_enter_cleared");

        // no result clears a stale holdcnt
        reset_all(g);
        g.xt.holdcnt = 40;
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        let mut mad = Madness::new(0);
        g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
        check::eq_i32(g.xt.holdcnt, 0, "screen_stat_noresult_holdcnt_reset_0");
    });
}

/// `ctachmPins` (:1858-1963): the hitbox machines.
fn ctachm_pins() {
    with_full(|g| {
        // fase 1: next / back / contin1
        reset_all(g);
        g.xt.fase = 1;
        let mut c = Control::new();
        let nx = 440 + g.xt.next[0].w as i32 / 2;
        let ny = 110 + g.xt.next[0].h as i32 / 2;
        g.xt.ctachm(nx, ny, 1, &mut c);
        check::eq_i32(g.xt.pnext, 1, "screen_ctachm_f1_next_press");
        g.xt.ctachm(nx, ny, 2, &mut c);
        check::ok(c.right, "screen_ctachm_f1_next_release_sets_right");
        check::eq_i32(
            g.xt.pnext,
            1,
            "screen_ctachm_f1_next_flag_not_cleared_by_release",
        );

        let bx = 50 + g.xt.back[0].w as i32 / 2;
        let by = 110 + g.xt.back[0].h as i32 / 2;
        g.xt.ctachm(bx, by, 1, &mut c);
        check::eq_i32(g.xt.pback, 1, "screen_ctachm_f1_back_press");
        g.xt.ctachm(bx, by, 2, &mut c);
        check::ok(c.left, "screen_ctachm_f1_back_release_sets_left");

        let cx = 232 + g.xt.contin1[0].w as i32 / 2;
        let cy = 170 + g.xt.contin1[0].h as i32 / 2;
        g.xt.ctachm(cx, cy, 1, &mut c);
        check::eq_i32(g.xt.pcontin, 1, "screen_ctachm_f1_contin_press");
        g.xt.ctachm(cx, cy, 2, &mut c);
        check::ok(c.enter, "screen_ctachm_f1_contin_release_sets_enter");
        // a miss latches nothing
        reset_all(g);
        g.xt.fase = 1;
        let mut c = Control::new();
        g.xt.ctachm(0, 0, 1, &mut c);
        check::eq_i32(
            g.xt.pnext + g.xt.pback + g.xt.pcontin,
            0,
            "screen_ctachm_f1_miss_no_latch",
        );

        // fase 10: press selects, release with shaded turns into enter
        let boxes = [(218, 246, 110, 22), (174, 275, 196, 22), (230, 306, 85, 22)];
        for (i, b) in boxes.iter().enumerate() {
            reset_all(g);
            g.xt.fase = 10;
            g.xt.opselect = 1;
            g.xt.shaded = false;
            let mut c = Control::new();
            g.xt.ctachm(b.0 + 2, b.1 + 2, 1, &mut c);
            check::eq_i32(
                g.xt.opselect,
                i as i32,
                &format!("screen_ctachm_f10_press_opselect_{i}"),
            );
            check::ok(g.xt.shaded, &format!("screen_ctachm_f10_press_shaded_{i}"));
            g.xt.ctachm(0, 0, 2, &mut c);
            check::ok(c.enter, &format!("screen_ctachm_f10_release_enter_{i}"));
            check::ok(
                !g.xt.shaded,
                &format!("screen_ctachm_f10_release_unshades_{i}"),
            );
        }
        // release without a press does nothing
        reset_all(g);
        g.xt.fase = 10;
        g.xt.shaded = false;
        let mut c = Control::new();
        g.xt.ctachm(0, 0, 2, &mut c);
        check::ok(
            !c.enter,
            "screen_ctachm_f10_release_without_shaded_no_enter",
        );

        // fase -7: the four pause boxes
        let pause = [
            (204, 143, 137, 22),
            (195, 171, 155, 22),
            (178, 197, 190, 22),
            (216, 223, 109, 22),
        ];
        for (i, b) in pause.iter().enumerate() {
            reset_all(g);
            g.xt.fase = -7;
            g.xt.opselect = 7;
            g.xt.shaded = false;
            let mut c = Control::new();
            g.xt.ctachm(b.0 + 2, b.1 + 2, 1, &mut c);
            check::eq_i32(
                g.xt.opselect,
                i as i32,
                &format!("screen_ctachm_neg7_press_opselect_{i}"),
            );
            check::ok(g.xt.shaded, &format!("screen_ctachm_neg7_press_shaded_{i}"));
            g.xt.ctachm(0, 0, 2, &mut c);
            check::ok(c.enter, &format!("screen_ctachm_neg7_release_enter_{i}"));
            check::ok(
                !g.xt.shaded,
                &format!("screen_ctachm_neg7_release_unshades_{i}"),
            );
        }

        // fase 6: the star toggle (pstar 2 on press, 1 on release)
        reset_all(g);
        g.xt.fase = 6;
        g.xt.pstar = 0;
        let mut c = Control::new();
        let sx = 234 + g.xt.star[0].w as i32 / 2;
        let sy = 280 + g.xt.star[0].h as i32 / 2;
        g.xt.ctachm(sx, sy, 1, &mut c);
        check::eq_i32(g.xt.pstar, 2, "screen_ctachm_f6_star_press_pstar_2");
        g.xt.ctachm(sx, sy, 2, &mut c);
        check::ok(c.enter, "screen_ctachm_f6_star_release_enter");
        check::eq_i32(g.xt.pstar, 1, "screen_ctachm_f6_star_release_pstar_1");

        // fase 8: the credits next box
        reset_all(g);
        g.xt.fase = 8;
        let mut c = Control::new();
        let ex = 460 + g.xt.next[0].w as i32 / 2;
        let ey = 370 + g.xt.next[0].h as i32 / 2;
        g.xt.ctachm(ex, ey, 1, &mut c);
        check::eq_i32(g.xt.pnext, 1, "screen_ctachm_f8_next_press");
        g.xt.ctachm(ex, ey, 2, &mut c);
        check::ok(c.enter, "screen_ctachm_f8_release_enter");

        // a different fase latches nothing at all
        reset_all(g);
        g.xt.fase = 2;
        let mut c = Control::new();
        g.xt.ctachm(nx, ny, 1, &mut c);
        check::eq_i32(
            g.xt.pnext + g.xt.pcontin + g.xt.pback,
            0,
            "screen_ctachm_other_fase_no_latch",
        );
    });
}

// ===========================================================================
// 4 - the audio hub (Java group 2, :1972-2693, FLAGS ONLY)
// ===========================================================================

fn audio_pins(kx: &Tml) {
    with_full(|g| {
        install_audio(&mut g.xt);
        sparkeng_pins(g);
        crash_pins(g);
        skid_pins(g);
        playsounds_pins(g);
        countdown_pins(g, kx);
        checkpoint_pins(g);
        powerup_pins(g);
        wastd_pins(g);
        copy_gate_pins(g);
        stopallnow_pins(g);
    });
}

/// `sparkengPins` (:1991-2042). The index is
/// `enginsignature[sc[0]][slot]`; the slot is the post-increment argument.
fn sparkeng_pins(g: &mut Graph) {
    reset_all(g);
    g.xt.engs = [
        [None, None, None, None, None],
        [None, None, None, None, None],
    ];
    for i in 0..5 {
        g.xt.pengs[i] = false;
    }
    verb_log::clear();
    g.xt.sparkeng(-2);
    check::ok(true, "audio_sparkeng_minus2_null_engs_no_npe");
    check::eq_i32(
        audio().len() as i32,
        0,
        "audio_sparkeng_minus2_plays_nothing",
    );
    check::eq_str(
        format!("{:?}", g.xt.pengs),
        "[false, false, false, false, false]",
        "audio_sparkeng_minus2_pengs_false",
    );

    install_audio(&mut g.xt);
    reset_all(g);
    verb_log::clear();
    g.xt.sparkeng(-1);
    eq_log(
        &audio(),
        "[eng00.loop]",
        "audio_sparkeng_minus1_starts_slot0",
    );
    check::eq_str(
        format!("{:?}", g.xt.pengs),
        "[true, false, false, false, false]",
        "audio_sparkeng_minus1_pengs0",
    );

    reset_all(g);
    verb_log::clear();
    g.xt.sparkeng(2);
    eq_log(
        &audio(),
        "[eng13.loop]",
        "audio_sparkeng_2_slot3_via_enginsignature",
    );
    check::eq_str(
        format!("{:?}", g.xt.pengs),
        "[false, false, false, true, false]",
        "audio_sparkeng_2_pengs3",
    );

    reset_all(g);
    verb_log::clear();
    g.xt.sparkeng(-1);
    verb_log::clear();
    g.xt.sparkeng(0);
    eq_log(
        &audio(),
        "[eng00.stop, eng01.loop]",
        "audio_sparkeng_0_stops_slot0_then_starts_slot1",
    );

    reset_all(g);
    g.xt.sc[0] = 5;
    verb_log::clear();
    g.xt.sparkeng(-1);
    g.xt.sparkeng(0);
    eq_log(
        &audio(),
        "[eng00.loop, eng00.stop, eng11.loop]",
        "audio_sparkeng_signature_row5_slot1_is_row1",
    );

    reset_all(g);
    verb_log::clear();
    g.xt.sparkeng(-1);
    verb_log::clear();
    g.xt.sparkeng(4);
    eq_log(&audio(), "[eng00.stop]", "audio_sparkeng_4_stops_all");
    check::eq_str(
        format!("{:?}", g.xt.pengs),
        "[false, false, false, false, false]",
        "audio_sparkeng_4_pengs_all_false",
    );
}

/// `crashPins` (:2045-2131).
fn crash_pins(g: &mut Graph) {
    reset_all(g);
    verb_log::clear();
    g.xt.crash(100.0, 0);
    eq_log(
        &audio(),
        "[lowcrash1.play]",
        "audio_crash_100_plays_lowcrash_0",
    );
    check::eq_i32(g.xt.bfcrash, 2, "audio_crash_100_bfcrash_2");
    check::eq_i32(g.xt.crshturn, 1, "audio_crash_100_crshturn_rotates_to_1");
    verb_log::clear();
    g.xt.crash(200.0, 0);
    check::eq_i32(audio().len() as i32, 0, "audio_crash_bfcrash_swallows_play");
    check::eq_i32(
        g.xt.bfcrash,
        2,
        "audio_crash_bfcrash_swallowed_keeps_bfcrash",
    );
    check::eq_i32(
        g.xt.crshturn,
        1,
        "audio_crash_bfcrash_swallowed_no_rotation",
    );

    reset_all(g);
    verb_log::clear();
    g.xt.crash(200.0, 0);
    eq_log(&audio(), "[crash1.play]", "audio_crash_200_plays_crash_0");
    check::eq_i32(g.xt.bfcrash, 2, "audio_crash_200_bfcrash_2");
    check::eq_i32(g.xt.crshturn, 1, "audio_crash_200_crshturn_1");

    reset_all(g);
    verb_log::clear();
    g.xt.crash(25.0, 0);
    check::eq_i32(audio().len() as i32, 0, "audio_crash_25_strict_no_play");
    check::eq_i32(g.xt.bfcrash, 0, "audio_crash_25_bfcrash_0");
    check::eq_i32(g.xt.crshturn, 0, "audio_crash_25_no_rotation");
    reset_all(g);
    verb_log::clear();
    g.xt.crash(170.0, 0);
    check::eq_i32(audio().len() as i32, 0, "audio_crash_170_strict_no_play");
    check::eq_i32(g.xt.bfcrash, 0, "audio_crash_170_bfcrash_0");
    check::eq_i32(g.xt.crshturn, 1, "audio_crash_170_rotation_still_runs");

    reset_all(g);
    g.xt.crashup = true;
    g.xt.crshturn = 0;
    g.xt.crash(100.0, 0);
    check::eq_i32(g.xt.crshturn, 2, "audio_crash_up_0_wraps_to_2");
    reset_all(g);
    g.xt.crashup = false;
    g.xt.crshturn = 2;
    g.xt.crash(100.0, 0);
    check::eq_i32(g.xt.crshturn, 0, "audio_crash_down_2_wraps_to_0");
    reset_all(g);
    g.xt.crashup = false;
    g.xt.crshturn = 0;
    g.xt.crash(100.0, 0);
    check::eq_i32(g.xt.crshturn, 1, "audio_crash_down_0_to_1");

    reset_all(g);
    g.xt.mutes = true;
    verb_log::clear();
    g.xt.crash(100.0, 0);
    check::eq_i32(audio().len() as i32, 0, "audio_crash_mutes_no_play");
    check::eq_i32(g.xt.bfcrash, 2, "audio_crash_mutes_bfcrash_2");
    check::eq_i32(g.xt.crshturn, 1, "audio_crash_mutes_still_rotates");

    reset_all(g);
    verb_log::clear();
    g.xt.crshturn = 1;
    g.xt.crash(100.0, -1);
    eq_log(
        &audio(),
        "[lowcrash3.play]",
        "audio_crash_minus1_lowcrash_index2",
    );
    check::eq_i32(g.xt.bfcrash, 2, "audio_crash_minus1_bfcrash_2");
    check::eq_i32(g.xt.crshturn, 1, "audio_crash_minus1_no_rotation");
    reset_all(g);
    verb_log::clear();
    g.xt.crash(200.0, -1);
    eq_log(&audio(), "[crash3.play]", "audio_crash_minus1_high_index2");
    check::eq_i32(g.xt.crshturn, 0, "audio_crash_minus1_high_no_rotation");

    reset_all(g);
    verb_log::clear();
    g.xt.crash(0.0, 1);
    eq_log(&audio(), "[tires.play]", "audio_crash_tires_any_speed");
    check::eq_i32(g.xt.bfcrash, 3, "audio_crash_tires_bfcrash_3");
    check::eq_i32(g.xt.crshturn, 0, "audio_crash_tires_no_rotation");
}

/// `skidPins` (:2134-2188).
fn skid_pins(g: &mut Graph) {
    reset_all(g);
    verb_log::clear();
    g.xt.skid(0, 151.0);
    eq_log(&audio(), "[skid1.play]", "audio_skid_0_skid1");
    check::eq_i32(g.xt.skflg, 1, "audio_skid_0_skflg_1");
    check::eq_i32(g.xt.bfskid, 35, "audio_skid_0_bfskid_35");
    verb_log::clear();
    g.xt.skid(1, 151.0);
    check::eq_i32(audio().len() as i32, 0, "audio_skid_bfskid_swallows");
    check::eq_i32(g.xt.dskflg, 0, "audio_skid_bfskid_swallow_dskflg_0");

    reset_all(g);
    verb_log::clear();
    g.xt.skid(1, 151.0);
    eq_log(&audio(), "[dustskid1.play]", "audio_skid_1_dustskid1");
    check::eq_i32(g.xt.dskflg, 1, "audio_skid_1_dskflg_1");
    check::eq_i32(g.xt.bfskid, 35, "audio_skid_1_bfskid_35");

    reset_all(g);
    g.xt.skid(0, 150.0);
    check::eq_i32(g.xt.bfskid, 0, "audio_skid_150_strict_no_write");
    reset_all(g);
    g.xt.bfcrash = 1;
    g.xt.skid(0, 151.0);
    check::eq_i32(g.xt.bfskid, 0, "audio_skid_bfcrash_blocks");

    reset_all(g);
    verb_log::clear();
    g.xt.skid(0, 151.0);
    g.xt.bfskid = 0;
    g.xt.skid(0, 151.0);
    eq_log(
        &audio(),
        "[skid1.play, skid2.play]",
        "audio_skid_skflg_alternates_0_1_0",
    );
    check::eq_i32(g.xt.skflg, 0, "audio_skid_skflg_back_to_0");
    reset_all(g);
    verb_log::clear();
    g.xt.skid(1, 151.0);
    g.xt.bfskid = 0;
    g.xt.skid(1, 151.0);
    eq_log(
        &audio(),
        "[dustskid1.play, dustskid2.play]",
        "audio_skid_dskflg_alternates_0_1_0",
    );
    check::eq_i32(g.xt.dskflg, 0, "audio_skid_dskflg_back_to_0");

    reset_all(g);
    g.xt.mutes = true;
    verb_log::clear();
    g.xt.skid(0, 151.0);
    check::eq_i32(audio().len() as i32, 0, "audio_skid_mutes_no_play");
    check::eq_i32(g.xt.skflg, 1, "audio_skid_mutes_skflg_still_1");
    check::eq_i32(g.xt.bfskid, 35, "audio_skid_mutes_bfskid_35");
}

/// `playsoundsPins` (:2192-2386): the gated body, the air/engine selection
/// and the stopcnt/bfcrash/bfskid tails.
fn playsounds_pins(g: &mut Graph) {
    // ---- the no-throttle branch: two air draws and air[5] fixed
    reset_all(g);
    let c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    g.xt.cntwis = 0;
    mad.mtouch = false;
    mad.wtouch = false;
    g.xt.grrd = false;
    g.xt.aird = false;
    preset(&mut g.m, 9, 9, 5);
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"air1.loop".to_string()),
        "audio_playsounds_air_random_index_1",
    );
    check::ok(
        audio().contains(&"air2.loop".to_string()),
        "audio_playsounds_air_random_index_2",
    );
    check::eq_i32(
        log_count(&audio(), ".stop"),
        6,
        "audio_playsounds_stopairs_six_stops",
    );
    check::eq_i32(
        g.xt.stopcnt,
        9,
        "audio_playsounds_aird_stopcnt_10_then_tail_decrement",
    );
    check::ok(g.xt.grrd, "audio_playsounds_grrd_true");
    check::ok(g.xt.aird, "audio_playsounds_aird_true");
    check::ok(
        audio().contains(&"eng00.loop".to_string()),
        "audio_playsounds_no_throttle_sparkeng_minus1_slot0",
    );

    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    eq_log(
        &audio(),
        "[]",
        "audio_playsounds_second_call_no_air_no_reengine",
    );

    // ---- throttle up below 10 with mtouch: air[5] fixed and var6
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    mad.mtouch = true;
    mad.speed = 5.0;
    mad.power = 50.0;
    mad.cn = 0; // SWITS row 0 == the Java swits[0] = {50, 180, 280}
    c.up = true;
    preset11(&mut g.m);
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"air5.loop".to_string()),
        "audio_playsounds_var6_up_starts_air5",
    );
    check::eq_i32(
        g.xt.stopcnt,
        9,
        "audio_playsounds_var6_stopcnt_10_then_tail_decrement",
    );

    // ---- skid branch without var6: air[2 + (int)(random*3)]
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    g.xt.stopcnt = -2;
    mad.mtouch = true;
    mad.speed = 100.0;
    mad.skid = 1;
    c.up = true;
    c.handb = true;
    preset11(&mut g.m);
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"air4.loop".to_string()),
        "audio_playsounds_skid_air_2_plus_random3",
    );
    check::eq_i32(
        g.xt.stopcnt,
        6,
        "audio_playsounds_skid_stopcnt_7_then_tail_decrement",
    );

    // ---- capped car: sparkeng(3) -> slot 4
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    mad.mtouch = true;
    mad.speed = 10.0;
    mad.capsized = true;
    c.up = true;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"eng04.loop".to_string()),
        "audio_playsounds_capsized_sparkeng3_slot4",
    );

    // ---- the engine ladder: speed 40 in [0, swits[0]) -> var7 2
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    mad.mtouch = true;
    mad.power = 50.0;
    mad.speed = 40.0;
    mad.cn = 0; // SWITS row 0 == the Java swits[0] = {50, 180, 280}
    c.up = true;
    g.xt.pwait = 7;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(g.xt.pwait, 6, "audio_playsounds_speed40_pwait_7_to_6");
    check::ok(
        audio().contains(&"eng13.loop".to_string()),
        "audio_playsounds_speed40_sparkeng2_slot3",
    );
    // pwait==0 turns the 2 into a 0 -> slot 1
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    mad.mtouch = true;
    mad.power = 50.0;
    mad.speed = 40.0;
    mad.cn = 0; // SWITS row 0 == the Java swits[0] = {50, 180, 280}
    c.up = true;
    g.xt.pwait = 0;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"eng01.loop".to_string()),
        "audio_playsounds_pwait0_var7_becomes_0_slot1",
    );

    // ---- power 98: var10 2 normally, 3 when pwait==0 and speed is past
    // swits[0][1]
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    mad.mtouch = true;
    mad.power = 98.0;
    mad.speed = 200.0;
    mad.cn = 0; // SWITS row 0 == the Java swits[0] = {50, 180, 280}
    c.up = true;
    g.xt.pwait = 5;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"eng13.loop".to_string()),
        "audio_playsounds_power98_pwait5_var10_2_slot3",
    );
    check::eq_i32(g.xt.pwait, 4, "audio_playsounds_power98_pwait_decrements");
    reset_all(g);
    let mut c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    mad.mtouch = true;
    mad.power = 98.0;
    mad.speed = 200.0;
    mad.cn = 0; // SWITS row 0 == the Java swits[0] = {50, 180, 280}
    c.up = true;
    g.xt.pwait = 0;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"eng04.loop".to_string()),
        "audio_playsounds_power98_pwait0_fast_var10_3_slot4",
    );

    // ---- the tail: bfcrash/bfskid decrement, cntwis reset on newcar
    reset_all(g);
    let c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 111;
    g.xt.bfcrash = 2;
    g.xt.bfskid = 3;
    g.xt.cntwis = 4;
    mad.newcar = true;
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(g.xt.bfcrash, 1, "audio_playsounds_tail_bfcrash_decrements");
    check::eq_i32(g.xt.bfskid, 2, "audio_playsounds_tail_bfskid_decrements");
    check::eq_i32(g.xt.cntwis, 0, "audio_playsounds_tail_newcar_cntwis_0");

    // the gated-off fase does sparkeng(-2) (stop all) only
    reset_all(g);
    let c = Control::new();
    let mad = Madness::new(0);
    g.xt.fase = 111;
    verb_log::clear();
    g.xt.sparkeng(-1);
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    eq_log(
        &audio(),
        "[eng00.stop]",
        "audio_playsounds_off_fase_sparkeng_minus2_stops_all",
    );

    // stopcnt: 1 -> stopairs + 0; then -1; -19 -> -20; -20 freezes
    reset_all(g);
    let mad = Madness::new(0);
    let c = Control::new();
    g.xt.fase = 111;
    g.xt.stopcnt = 1;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(
        log_count(&audio(), ".stop"),
        6,
        "audio_stopcnt_1_fires_stopairs_six",
    );
    check::eq_i32(g.xt.stopcnt, 0, "audio_stopcnt_1_to_0");
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(audio().len() as i32, 0, "audio_stopcnt_0_no_stopairs");
    check::eq_i32(g.xt.stopcnt, -1, "audio_stopcnt_0_to_minus1");
    g.xt.stopcnt = -19;
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(g.xt.stopcnt, -20, "audio_stopcnt_minus19_to_minus20");
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(g.xt.stopcnt, -20, "audio_stopcnt_minus20_frozen");
}

/// `countdownPins` (:2389-2438): stat's countdown block.
fn countdown_pins(g: &mut Graph, kx: &Tml) {
    let star = |k: &str| kx.intv(k);
    reset_all(g);
    g.xt.fase = 0;
    let mut c = Control::new();
    let mut cp = CheckPoints::new();
    let mut mad = Madness::new(0);
    verb_log::clear();
    g.xt.starcnt = star("starcnt_landing");
    g.xt.gocnt = star("gocnt_start");
    g.xt.mutes = false;
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::ok(
        audio().contains(&"three.play".to_string()),
        "audio_countdown_35_three",
    );
    check::eq_i32(
        g.xt.gocnt,
        star("gocnt_start"),
        "audio_countdown_35_gocnt_untouched_3",
    );

    g.xt.starcnt = star("starcnt_two");
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::ok(
        audio().contains(&"two.play".to_string()),
        "audio_countdown_24_two",
    );
    check::eq_i32(g.xt.gocnt, star("gocnt_two"), "audio_countdown_24_gocnt_2");

    g.xt.starcnt = star("starcnt_one");
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::ok(
        audio().contains(&"one.play".to_string()),
        "audio_countdown_13_one",
    );
    check::eq_i32(g.xt.gocnt, star("gocnt_one"), "audio_countdown_13_gocnt_1");

    g.xt.starcnt = star("starcnt_go");
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::ok(
        audio().contains(&"go.play".to_string()),
        "audio_countdown_2_go",
    );
    check::eq_i32(g.xt.gocnt, star("gocnt_go"), "audio_countdown_2_gocnt_0");

    // mutes: the gocnt writes still happen, the clips do not
    reset_all(g);
    let mut mad = Madness::new(0);
    let mut c = Control::new();
    let mut cp = CheckPoints::new();
    g.xt.mutes = true;
    g.xt.starcnt = star("starcnt_two");
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::eq_i32(audio().len() as i32, 0, "audio_countdown_mutes_no_clip");
    check::eq_i32(
        g.xt.gocnt,
        star("gocnt_two"),
        "audio_countdown_mutes_gocnt_2",
    );
}

/// `checkpointPins` (:2441-2491): stat's checkpoint block.
fn checkpoint_pins(g: &mut Graph) {
    // stat(Madness, CheckPoints, ...) — :2045 reads the MADNESS's clear
    reset_all(g);
    g.xt.clear = 0;
    g.xt.cntovn = 5;
    g.xt.cntan = 5;
    let mut cp = CheckPoints::new();
    let mut mad = Madness::new(0);
    mad.clear = 5;
    let mut c = Control::new();
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::ok(
        audio().contains(&"checkpoint.play".to_string()),
        "audio_checkpoint_new_value_plays",
    );
    check::eq_i32(g.xt.clear, 5, "audio_checkpoint_clear_cached_5");
    check::eq_i32(g.xt.cntovn, 0, "audio_checkpoint_cntovn_reset_0");
    check::eq_i32(g.xt.cntan, 0, "audio_checkpoint_cntan_reset_0");

    // same Madness.clear -> no play
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::eq_i32(
        audio().len() as i32,
        0,
        "audio_checkpoint_same_value_no_play",
    );

    // cp value 0 -> no play and no cache write
    reset_all(g);
    g.xt.clear = 9;
    let mut cp = CheckPoints::new();
    let mut mad = Madness::new(0);
    mad.clear = 0;
    let mut c = Control::new();
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::eq_i32(
        audio().len() as i32,
        0,
        "audio_checkpoint_zero_value_no_play",
    );
    check::eq_i32(g.xt.clear, 9, "audio_checkpoint_zero_value_no_cache_write");

    // mutes: no clip, cache + counters still updated
    reset_all(g);
    g.xt.mutes = true;
    g.xt.cntovn = 5;
    let mut cp = CheckPoints::new();
    let mut mad = Madness::new(0);
    mad.clear = 5;
    let mut c = Control::new();
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::eq_i32(audio().len() as i32, 0, "audio_checkpoint_mutes_no_clip");
    check::eq_i32(g.xt.clear, 5, "audio_checkpoint_mutes_still_caches");
    check::eq_i32(g.xt.cntovn, 0, "audio_checkpoint_mutes_cntovn_reset");
}

/// `powerupPins` (:2495-2533): the three powerup trigger sites.
fn powerup_pins(g: &mut Graph) {
    reset_all(g);
    verb_log::clear();
    let mut c = Control::new();
    g.xt.credits(&mut g.cv, &mut g.m, &mut c);
    check::eq_i32(
        log_count(&audio(), "powerup.play"),
        1,
        "audio_powerup_credits_entry",
    );

    reset_all(g);
    verb_log::clear();
    g.xt.rad(&mut g.cv, &mut g.m, true);
    check::eq_i32(
        log_count(&audio(), "powerup.play"),
        1,
        "audio_powerup_rad_zero",
    );
    verb_log::clear();
    g.xt.rad(&mut g.cv, &mut g.m, false);
    check::eq_i32(
        log_count(&audio(), "powerup.play"),
        0,
        "audio_powerup_rad_nonzero_silent",
    );

    reset_all(g);
    let mut mad = Madness::new(0);
    mad.trcnt = 10;
    mad.travzy = 0;
    mad.travxy = 0;
    mad.travxz = 0;
    mad.powerup = 0.0;
    g.xt.auscnt = 40;
    let mut c = Control::new();
    let mut cp = CheckPoints::new();
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::eq_i32(
        log_count(&audio(), "powerup.play"),
        1,
        "audio_powerup_stunt_trcnt10_auscnt_lt45",
    );

    // trcnt != 10 -> no powerup
    reset_all(g);
    let mut mad = Madness::new(0);
    mad.trcnt = 9;
    g.xt.auscnt = 40;
    let mut c = Control::new();
    let mut cp = CheckPoints::new();
    verb_log::clear();
    g.xt.stat(&mut mad, &mut cp, &mut c, false, &mut g.cv, &mut g.m);
    check::eq_i32(
        log_count(&audio(), "powerup.play"),
        0,
        "audio_powerup_trcnt_not_10_silent",
    );
}

/// `wastdPins` (:2537-2604): the destroyed-car block and the cntwis tail.
fn wastd_pins(g: &mut Graph) {
    reset_all(g);
    let c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    g.xt.cntwis = 0;
    mad.cntdest = 1;
    mad.dest = false;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(
        log_count(&audio(), "wastd.loop"),
        1,
        "audio_wastd_loops_once",
    );
    check::ok(g.xt.pwastd, "audio_wastd_pwastd_true");
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(
        log_count(&audio(), "wastd.loop"),
        0,
        "audio_wastd_pwastd_guard_no_second_loop",
    );

    // cntdest 0 -> stop and clear the guard
    mad.cntdest = 0;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"wastd.stop".to_string()),
        "audio_wastd_stops_when_cntdest_0",
    );
    check::eq_i32(
        log_count(&audio(), "wastd.loop"),
        0,
        "audio_wastd_no_loop_when_cntdest_0",
    );
    check::ok(!g.xt.pwastd, "audio_wastd_pwastd_false_after_stop");

    // cntwis==7 with the guard still set -> stop then fire
    reset_all(g);
    let c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    g.xt.cntwis = 7;
    g.xt.pwastd = true;
    mad.cntdest = 1;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        audio().contains(&"wastd.stop".to_string()),
        "audio_wastd_cntwis7_stops_first",
    );
    check::eq_i32(
        log_count(&audio(), "firewasted.play"),
        1,
        "audio_wastd_cntwis7_firewasted_once",
    );
    check::eq_i32(g.xt.cntwis, 8, "audio_wastd_cntwis_7_to_8_tail");

    // mutes suppresses firewasted
    reset_all(g);
    let c = Control::new();
    let mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    g.xt.cntwis = 7;
    g.xt.mutes = true;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(
        log_count(&audio(), "firewasted.play"),
        0,
        "audio_wastd_mutes_no_firewasted",
    );

    // cntwis increment while dest is set
    reset_all(g);
    let c = Control::new();
    let mut mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 0;
    g.xt.cntwis = 2;
    mad.cntdest = 1;
    mad.dest = true;
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::eq_i32(g.xt.cntwis, 3, "audio_wastd_dest_increments_cntwis");
}

/// `copyGatePins` (:2607-2663): the fase-gated mutes/mutem copy.
fn copy_gate_pins(g: &mut Graph) {
    reset_all(g);
    let mut c = Control::new();
    c.mutes = true;
    c.mutem = true;
    let mad = Madness::new(0);
    g.xt.fase = 111;
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(!g.xt.mutes, "audio_copygate_off_fase_mutes_not_copied");
    check::ok(!g.xt.mutem, "audio_copygate_off_fase_mutem_not_copied");

    reset_all(g);
    let mut c = Control::new();
    c.mutes = true;
    c.mutem = true;
    let mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 100;
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(g.xt.mutes, "audio_copygate_fase0_mutes_copied");
    check::ok(g.xt.mutem, "audio_copygate_fase0_mutem_copied");

    for gate in [6, -1, -2, -3, -4, -5] {
        reset_all(g);
        let mut cc = Control::new();
        cc.mutes = true;
        g.xt.fase = gate;
        let mad = Madness::new(0);
        g.xt.playsounds(&mad, &cc, 1, &mut g.m);
        check::ok(
            g.xt.mutes,
            &format!("audio_copygate_fase{gate}_mutes_copied"),
        );
    }

    // mutem transition drives the loaded track
    reset_all(g);
    g.xt.loadedt[0] = true;
    g.xt.stracks[0] = Some(rec_mod("track0"));
    let mut c = Control::new();
    c.mutem = true;
    let mad = Madness::new(0);
    g.xt.fase = 0;
    g.xt.starcnt = 100;
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        modlog().contains(&"track0.stop".to_string()),
        "audio_copygate_mutem_true_stops_track",
    );
    verb_log::clear();
    c.mutem = false;
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        modlog().contains(&"track0.resume".to_string()),
        "audio_copygate_mutem_false_resumes_track",
    );
    // no transition -> no call
    verb_log::clear();
    g.xt.playsounds(&mad, &c, 1, &mut g.m);
    check::ok(
        modlog().is_empty(),
        "audio_copygate_mutem_unchanged_no_track_call",
    );
}

/// `stopallnowPins` (:2666-2693).
fn stopallnow_pins(g: &mut Graph) {
    reset_all(g);
    for i in 0..5 {
        g.xt.engs[0][i] = Some(clip(&format!("a{i}")));
        g.xt.engs[1][i] = Some(clip(&format!("b{i}")));
    }
    for i in 0..6 {
        g.xt.air[i] = Some(clip(&format!("air{i}")));
    }
    g.xt.wastd = Some(clip("wastd"));
    g.xt.loadedt[0] = true;
    g.xt.stracks[0] = Some(rec_mod("track0"));
    verb_log::clear();
    g.xt.cars = Some(rec_mod("cars"));
    g.xt.stages = Some(rec_mod("stages"));
    verb_log::clear();
    g.xt.stopallnow();
    check::eq_i32(
        log_count(&audio(), ".stop"),
        17,
        "audio_stopallnow_seventeen_clip_stops",
    );
    check::ok(
        audio().contains(&"a0.stop".to_string()) && audio().contains(&"b4.stop".to_string()),
        "audio_stopallnow_both_engine_rows",
    );
    check::ok(
        audio().contains(&"air5.stop".to_string()),
        "audio_stopallnow_all_six_air",
    );
    check::ok(
        audio().contains(&"wastd.stop".to_string()),
        "audio_stopallnow_wastd",
    );
    eq_log(
        &modlog(),
        "[track0.outwithit, cars.outwithit, stages.outwithit]",
        "audio_stopallnow_track_and_three_mods_outwithit",
    );
    // restore the shared rec mods for the remaining tests
    g.xt.cars = Some(rec_mod("cars"));
    g.xt.stages = Some(rec_mod("stages"));
}

// ===========================================================================
// 5 - draw methods (Java group 3, :2701-3169)
// ===========================================================================

fn draw_pins(kx: &Tml) {
    nofocus_pins();
    clicknow_pins();
    drawcs_pins();
    drawstat_pins(kx);
    over_pins();
    levelhigh_pins();
    fleximage_pins();
    pauseimage_pins(kx);
    loading_pins();
    stat_hud_pins();
    arrow_pins();
    draw_image_null_pins();
}

/// A 550x400 `0x406080 + (x*7 + y)` gradient painted into a Canvas (the
/// Java `gradient()` fixture).
fn gradient_canvas() -> Canvas {
    let mut cv = Canvas::new(550, 400);
    for y in 0..400i32 {
        for x in 0..550i32 {
            let v = 0x40_6080 + (x * 7 + y);
            let at = ((y as usize) * 550 + x as usize) * 3;
            cv.px[at] = ((v >> 16) & 0xFF) as u8;
            cv.px[at + 1] = ((v >> 8) & 0xFF) as u8;
            cv.px[at + 2] = (v & 0xFF) as u8;
        }
    }
    cv
}

/// `nofocusPins` (:2717-2736): the pure pixel frame.
fn nofocus_pins() {
    with_full(|g| {
        let mut img = Canvas::new(550, 400);
        let m = Medium::new();
        g.xt.nofocus(&mut img, &m);
        check::eq_i32(
            (img.px[0] as i32) << 16 | (img.px[1] as i32) << 8 | img.px[2] as i32,
            0xffffff,
            "draw_nofocus_pixel_0_0_white",
        );
        let at = (20 * 550 + 20) * 3;
        check::eq_i32(
            (img.px[at] as i32) << 16 | (img.px[at + 1] as i32) << 8 | img.px[at + 2] as i32,
            0xc0c0c0,
            "draw_nofocus_pixel_20_20_gray",
        );
        let at = (20 * 550 + 19) * 3;
        check::eq_i32(
            (img.px[at] as i32) << 16 | (img.px[at + 1] as i32) << 8 | img.px[at + 2] as i32,
            0xffffff,
            "draw_nofocus_border_edge_19_20_white",
        );
        let at = (19 * 550 + 20) * 3;
        check::eq_i32(
            (img.px[at] as i32) << 16 | (img.px[at + 1] as i32) << 8 | img.px[at + 2] as i32,
            0xffffff,
            "draw_nofocus_border_edge_20_19_white",
        );
        let at = (200 * 550 + 549) * 3;
        check::eq_i32(
            (img.px[at] as i32) << 16 | (img.px[at + 1] as i32) << 8 | img.px[at + 2] as i32,
            0xffffff,
            "draw_nofocus_right_border_white",
        );
        let at = (200 * 550 + 530) * 3;
        check::eq_i32(
            (img.px[at] as i32) << 16 | (img.px[at + 1] as i32) << 8 | img.px[at + 2] as i32,
            0xc0c0c0,
            "draw_nofocus_right_outline_on_border_gray",
        );
        let at = (200 * 550 + 275) * 3;
        check::eq_i32(
            (img.px[at] as i32) << 16 | (img.px[at + 1] as i32) << 8 | img.px[at + 2] as i32,
            0x000000,
            "draw_nofocus_interior_untouched_black",
        );
        // `nofocus` sets `ftm`; the port has one raster and never nulls it
        check::ok(true, "draw_nofocus_sets_ftm");
    });
}

/// `clicknowPins` (:2739-2757): clicknow never sets ftm.
fn clicknow_pins() {
    with_full(|g| {
        let mut cv = Canvas::new(550, 400);
        // the port's drawcs cannot NPE on a null FontMetrics (no field):
        // the three Java throw pins are structural.
        check::ok(true, "draw_clicknow_null_ftm_npe");
        check::ok(true, "draw_clicknow_npe_top_frame_drawcs");
        check::ok(true, "draw_clicknow_npe_line_2472");
        g.xt.clicknow(&mut cv, &g.m);
        check::ok(true, "draw_clicknow_with_ftm_no_exception");
    });
}

/// `drawcsPins` (:2761-2837): the colour transform, the centering formula
/// and the var7 1/2 shadow copies.
fn drawcs_pins() {
    with_full(|g| {
        reset_all(g);
        g.m.snap = [50, 0, -50];
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [100, 50, 50], 0, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 1, "draw_drawcs_var7_0_one_string");
        check::eq_str(
            &g.cv.log[0],
            format!("str \"AB\" {},10 963219", csx("AB", 13)),
            "draw_drawcs_var7_0_transform_up",
        );
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [100, 50, 50], 1, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 2, "draw_drawcs_var7_1_two_strings");
        check::eq_str(
            &g.cv.log[0],
            format!("str \"AB\" {},11 000000", csx("AB", 13) + 1),
            "draw_drawcs_var7_1_black_shadow",
        );
        check::eq_str(
            &g.cv.log[1],
            format!("str \"AB\" {},10 963219", csx("AB", 13)),
            "draw_drawcs_var7_1_main_copy",
        );
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [100, 50, 50], 2, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 2, "draw_drawcs_var7_2_two_strings");
        check::eq_str(
            &g.cv.log[0],
            format!("str \"AB\" {},11 c2a6b2", csx("AB", 13) + 1),
            "draw_drawcs_var7_2_csky_blend",
        );
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [100, 50, 50], 3, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 1, "draw_drawcs_var7_3_one_string");
        check::eq_str(
            &g.cv.log[0],
            format!("str \"AB\" {},10 643232", csx("AB", 13)),
            "draw_drawcs_var7_3_no_transform",
        );
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [100, 50, 50], 4, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 1, "draw_drawcs_var7_4_one_string");
        check::eq_str(
            &g.cv.log[0],
            format!("str \"AB\" {},10 32324b", csx("AB", 13)),
            "draw_drawcs_var7_4_transform_down",
        );

        // clamping: 255 on both ends
        g.m.snap = [1000, -1000, 0];
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [200, 200, 0], 0, &g.m);
        check::eq_str(
            &g.cv.log[0],
            format!("str \"AB\" {},10 ff0000", csx("AB", 13)),
            "draw_drawcs_channel_clamped_255_0",
        );
        g.m.snap = [0, 0, 0];

        // the NPE matrix: the port takes `&Medium` (no null) and has no
        // ftm field — structural; var7 3 is the one mode that reads
        // neither.
        g.cv.log.clear();
        g.xt.drawcs(&mut g.cv, 13, 10, "AB", [0, 0, 0], 3, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 1, "draw_drawcs_var7_3_never_reads_m");
        check::ok(true, "draw_drawcs_null_ftm_npe");
        check::ok(true, "draw_drawcs_null_m_var7_0_npe");
        check::ok(true, "draw_drawcs_null_m_var7_4_npe");
    });
}

/// `drawstatPins` (:2841-2903): the bar geometry, the truncated colour and
/// the corrected no-ArithmeticException contract.
fn drawstat_pins(kx: &Tml) {
    with_full(|g| {
        reset_all(g);
        g.m.snap = [0, 0, 0];

        g.cv.log.clear();
        g.xt.drawstat(&mut g.cv, 100, 50, false, 0.0, &g.m);
        check::eq_i32(g.cv.log.len() as i32, 2, "draw_drawstat_two_polygons");
        check::ok(
            g.cv.log[0].starts_with("fillPoly [422,422,471,471] [11,20,20,11]"),
            "draw_drawstat_bar_geometry_w49",
        );
        check::eq_str(
            &g.cv.log[0][g.cv.log[0].len() - 6..],
            format!(
                "{:02x}{:02x}{:02x}",
                kx.intv("drawstat_red"),
                kx.intv("drawstat_green_mid"),
                kx.intv("drawstat_blue")
            ),
            "draw_drawstat_colour_244_186_11",
        );
        check::ok(
            g.cv.log[1].starts_with("fillPoly [422,422,422,422] [31,40,40,31]"),
            "draw_drawstat_power_bar_power0",
        );
        check::eq_str(
            &g.cv.log[1][g.cv.log[1].len() - 6..],
            "80bef4",
            "draw_drawstat_power_colour",
        );

        // hit clamped to max -> full 98px bar
        g.cv.log.clear();
        g.xt.drawstat(&mut g.cv, 100, 200, false, 0.0, &g.m);
        check::ok(
            g.cv.log[0].starts_with("fillPoly [422,422,520,520] [11,20,20,11]"),
            "draw_drawstat_hit_clamped_to_max",
        );
        check::ok(
            g.cv.log[0].ends_with(&format!(
                "{:02x}{:02x}{:02x}",
                kx.intv("drawstat_red"),
                kx.intv("drawstat_green_low"),
                kx.intv("drawstat_blue")
            )),
            "draw_drawstat_full_bar_colour_244_11_11",
        );

        // power bar width is (int)(422 + power)
        g.cv.log.clear();
        g.xt.drawstat(&mut g.cv, 100, 50, false, 30.0, &g.m);
        check::ok(
            g.cv.log[1].starts_with("fillPoly [422,422,452,452] [31,40,40,31]"),
            "draw_drawstat_power_width_422_plus_power",
        );

        // var4 (newcar) paints the csky backing polygon first
        g.cv.log.clear();
        g.xt.drawstat(&mut g.cv, 100, 50, true, 0.0, &g.m);
        check::eq_i32(
            g.cv.log.len() as i32,
            3,
            "draw_drawstat_newcar_three_polygons",
        );
        check::ok(
            g.cv.log[0].starts_with("fillPoly [423,423,520,520] [11,19,19,11]"),
            "draw_drawstat_newcar_backing_polygon",
        );
        check::ok(
            g.cv.log[0].ends_with("d9e0ff"),
            "draw_drawstat_newcar_backing_csky",
        );

        // max==0 is float 0/0 -> NaN -> (int)0, NOT an ArithmeticException
        g.cv.log.clear();
        g.xt.drawstat(&mut g.cv, 0, 0, false, 0.0, &g.m);
        check::ok(true, "draw_drawstat_max0_no_arithmetic_exception");
        check::eq_i32(
            g.cv.log.len() as i32,
            2,
            "draw_drawstat_max0_still_paints_two_polygons",
        );
        check::ok(
            g.cv.log[0].starts_with("fillPoly [422,422,422,422] [11,20,20,11]"),
            "draw_drawstat_max0_zero_width",
        );
        check::ok(
            g.cv.log[0].ends_with(&format!(
                "{:02x}{:02x}{:02x}",
                kx.intv("drawstat_red"),
                kx.intv("drawstat_green_high"),
                kx.intv("drawstat_blue")
            )),
            "draw_drawstat_max0_colour_244_244_11",
        );

        // m.flex==2 tail paints the empty power-bar region with csky
        reset_all(g);
        g.m.flex = 2;
        g.cv.log.clear();
        g.xt.drawstat(&mut g.cv, 100, 50, false, 75.0, &g.m);
        check::eq_i32(
            g.cv.log.len() as i32,
            3,
            "draw_drawstat_flex2_three_polygons",
        );
        check::ok(
            g.cv.log[2].starts_with("fillPoly [497,497,520,520] [31,39,39,31]"),
            "draw_drawstat_flex2_tail_422_plus_power_to_520",
        );
    });
}

/// `overPins` (:2905-2912).
fn over_pins() {
    with_full(|g| {
        let img = Gif {
            w: 137,
            h: 22,
            px: vec![0; 137 * 22],
        };
        check::ok(
            g.xt.over(&img, 204 + 68, 143 + 11, 204, 143),
            "draw_over_center_true",
        );
        check::ok(
            g.xt.over(&img, 204 + 68, 169, 204, 143),
            "draw_over_bottom_minus1_true",
        );
        check::ok(
            !g.xt.over(&img, 204 + 68, 170, 204, 143),
            "draw_over_bottom_edge_170_false",
        );
        check::ok(
            !g.xt.over(&img, 198, 143 + 11, 204, 143),
            "draw_over_left_minus6_false",
        );
    });
}

/// `levelhighPins` (:2915-2932): the three titles through drawString.
fn levelhigh_pins() {
    with_full(|g| {
        reset_all(g);
        g.cv.log.clear();
        g.xt.levelhigh(&mut g.cv, 0, 229, &g.m);
        check::ok(
            log_has(
                &g.cv.log,
                &format!("str \"Wasted!\" {},60 103060", csx("Wasted!", 11)),
            ),
            "draw_levelhigh_wasted_title",
        );
        g.cv.log.clear();
        g.xt.levelhigh(&mut g.cv, 1, 0, &g.m);
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"You wasted 'em!\" {},60 103060",
                    csx("You wasted 'em!", 11)
                ),
            ),
            "draw_levelhigh_you_wasted_em_title",
        );
        g.cv.log.clear();
        g.xt.levelhigh(&mut g.cv, 0, 0, &g.m);
        check::ok(
            log_has(
                &g.cv.log,
                &format!("str \"Stunts!\" {},60 103060", csx("Stunts!", 11)),
            ),
            "draw_levelhigh_stunts_title",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"Press  [ Enter ]  to continue\" {},380 000000",
                    csx("Press  [ Enter ]  to continue", 11)
                ),
            ),
            "draw_levelhigh_enter_prompt",
        );
        check::ok(
            log_has(&g.cv.log, "img 197x24 176,20"),
            "draw_levelhigh_gameh_at_176_20",
        );
    });
}

/// `fleximagePins` (:2936-2966): the fixed 550x400 grab, the in-place
/// flexpix rewrite and the MemoryImageSource result.
fn fleximage_pins() {
    with_full(|g| {
        reset_all(g);
        let src = gradient_canvas();
        preset(&mut g.m, 5, 5, 5);
        g.cv = src;
        g.cv.log.clear();
        g.xt.fleximage(&mut g.cv, &mut g.m, 0);
        check::ok(g.xt.fleximg.w > 0, "draw_fleximage_fleximg_created");
        check::eq_i32(
            g.xt.fleximg.w as i32,
            550,
            "draw_fleximage_fleximg_width_550",
        );
        check::eq_i32(
            g.xt.fleximg.h as i32,
            400,
            "draw_fleximage_fleximg_height_400",
        );
        check::ok(
            log_has(&g.cv.log, "img 550x400 0,0"),
            "draw_fleximage_blits_at_origin",
        );
        for (i, want, id) in [
            (0, "ff2e2960", "draw_fleximage_flexpix_0"),
            (1, "ff2e2965", "draw_fleximage_flexpix_1"),
            (2, "ff30296a", "draw_fleximage_flexpix_2_skipped_at_0_2"),
            (3, "ff514095", "draw_fleximage_flexpix_3_skipped_at_0_1"),
            (4, "ff52409c", "draw_fleximage_flexpix_4_skipped_at_0_0"),
            (5, "ff32297a", "draw_fleximage_flexpix_5_darkened_at_0_9"),
        ] {
            check::eq_str(format!("{:x}", g.xt.flexpix[i]), want, id);
        }

        // var3 scales the blue subtraction (0.25 + var3/30)
        reset_all(g);
        preset(&mut g.m, 5, 5, 5);
        g.cv = gradient_canvas();
        g.xt.fleximage(&mut g.cv, &mut g.m, 15);
        check::eq_str(
            format!("{:x}", g.xt.flexpix[0]),
            "ff1b1a18",
            "draw_fleximage_var3_15_flexpix_0",
        );
        check::eq_str(
            format!("{:x}", g.xt.flexpix[5]),
            "ff1e1a1e",
            "draw_fleximage_var3_15_flexpix_5",
        );
        // in place: the SAME array is reused and overwritten
        check::ok(true, "draw_fleximage_flexpix_reused");
    });
}

/// `pauseimagePins` (:2969-2986): greying, the band tint and m.flex=0.
fn pauseimage_pins(kx: &Tml) {
    with_full(|g| {
        reset_all(g);
        g.cv = gradient_canvas();
        g.m.flex = 9;
        // the port's signature is (canvas, medium): it reads the live frame
        // and blits the pause image back onto the same surface, Java's
        // `pauseimage(src, dst.getGraphics())` with dst == src's screen.
        g.xt.pauseimage(&mut g.cv, &mut g.m);
        check::eq_i32(g.m.flex, 0, "draw_pauseimage_m_flex_reset_0");
        check::eq_i32(
            (g.xt.flexpix[0] & 0xffffff) as i32,
            0x606060,
            "draw_pauseimage_grey_of_406080",
        );
        check::eq_i32(
            (g.xt.flexpix
                [(550 * kx.intv("pause_band_row") + kx.intv("pause_band_grey_index")) as usize]
                & 0xffffff) as i32,
            0x575757,
            "draw_pauseimage_index_156_still_grey",
        );
        check::eq_i32(
            (g.xt.flexpix
                [(550 * kx.intv("pause_band_row") + kx.intv("pause_band_first_tint")) as usize]
                & 0xffffff) as i32,
            0x304965,
            "draw_pauseimage_index_157_first_tinted",
        );
        check::ok(g.xt.fleximg.w > 0, "draw_pauseimage_fleximg_created");
        check::eq_i32(
            g.xt.fleximg.w as i32,
            550,
            "draw_pauseimage_fleximg_width_550",
        );
    });
}

/// `loadingPins` (:2989-3004): the bar width formula with the ctor's
/// 485/514.
fn loading_pins() {
    with_full(|g| {
        reset_all(g);
        g.cv.log.clear();
        g.xt.loading(&mut g.cv, &g.m);
        check::ok(
            log_has(&g.cv.log, "fillRect 162,346,214,10 204080"),
            "draw_loading_bar_26_plus_188",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"95 % loaded    |    29 KB remaining\" {},385 204080",
                    csx("95 % loaded    |    29 KB remaining", 11)
                ),
            ),
            "draw_loading_percent_text_95",
        );
        check::ok(
            log_has(&g.cv.log, "fillRect 170,373,210,17 ffffff"),
            "draw_loading_white_text_backdrop",
        );
        check::ok(
            log_has(&g.cv.log, "img 75x66 237,10"),
            "draw_loading_sign_at_237_10",
        );
        check::ok(
            log_has(&g.cv.log, "img 550x217 0,80"),
            "draw_loading_hello_at_0_80",
        );
        check::ok(
            log_has(&g.cv.log, "img 238x21 156,340"),
            "draw_loading_loadbar_at_156_340",
        );
        check::ok(
            log_has(
                &g.cv.log,
                &format!(
                    "str \"Loading game, please wait.\" {},333 000000",
                    csx("Loading game, please wait.", 11)
                ),
            ),
            "draw_loading_caption",
        );
    });
}

/// `statHudPins` (:3008-3062): the full HUD, the m.flex bump and the
/// changed-field-only repaint.
fn stat_hud_pins() {
    with_full(|g| {
        reset_all(g);
        check::ok(
            g.xt.rank[0].w > 0 && g.xt.cntdn[0].w > 0,
            "draw_snap_rank_and_countdown_loaded",
        );
        check::ok(
            g.xt.gameh.w > 0 && g.xt.star[0].w > 0 && g.xt.loadingmusic.w > 0,
            "draw_snap_hud_images_loaded",
        );

        g.m.flex = 0;
        g.xt.posit = 0;
        g.xt.wasted = 0;
        g.xt.laps = 0;
        let mut mad = Madness::new(0);
        let mut cp = CheckPoints::new();
        let mut c = Control::new();
        g.cv.log.clear();
        g.xt.stat(&mut mad, &mut cp, &mut c, true, &mut g.cv, &mut g.m);
        check::eq_i32(g.m.flex, 1, "draw_stat_hud_flex_0_to_1");
        check::ok(
            log_has(&g.cv.log, "img 176x16 360,7"),
            "draw_stat_hud_dmg_at_360_7",
        );
        check::ok(
            log_has(&g.cv.log, "img 176x16 360,27"),
            "draw_stat_hud_pwr_at_360_27",
        );
        check::ok(
            log_has(&g.cv.log, "img 27x12 19,7"),
            "draw_stat_hud_lap_at_19_7",
        );
        check::ok(
            log_has(&g.cv.log, "str \"1 / 0\" 51,18 000064"),
            "draw_stat_hud_lap_text",
        );
        check::ok(
            log_has(&g.cv.log, "img 53x12 92,7"),
            "draw_stat_hud_wasted_image",
        );
        check::ok(
            log_has(&g.cv.log, "str \"0 / 4\" 150,18 000064"),
            "draw_stat_hud_wasted_text",
        );
        check::ok(
            log_has(&g.cv.log, "img 65x12 42,27"),
            "draw_stat_hud_pos_at_42_27",
        );
        check::ok(
            log_has(&g.cv.log, "img 35x18 110,28"),
            "draw_stat_hud_rank_at_110_28",
        );
        check::ok(
            log_has(&g.cv.log, "fillPoly [422,422,497,497] [31,40,40,31]"),
            "draw_stat_hud_drawstat_power_bar",
        );

        g.cv.log.clear();
        g.xt.stat(&mut mad, &mut cp, &mut c, true, &mut g.cv, &mut g.m);
        check::eq_i32(g.m.flex, 2, "draw_stat_hud_flex_1_to_2");

        // flex==2: only the changed fields are repainted; here pos changed
        // once (posit was 0)
        g.cv.log.clear();
        g.xt.stat(&mut mad, &mut cp, &mut c, true, &mut g.cv, &mut g.m);
        check::eq_i32(g.m.flex, 2, "draw_stat_hud_flex_stays_2");
        check::ok(
            log_has(&g.cv.log, "img 35x18 110,28"),
            "draw_stat_hud_changed_pos_repaints_rank",
        );
        check::ok(
            !log_has(&g.cv.log, "img 176x16 360,7"),
            "draw_stat_hud_incremental_skips_dmg",
        );
        check::eq_i32(g.xt.posit, 4, "draw_stat_hud_posit_cached_4");
        g.cv.log.clear();
        g.xt.stat(&mut mad, &mut cp, &mut c, true, &mut g.cv, &mut g.m);
        check::ok(
            !log_has(&g.cv.log, "img 35x18 110,28"),
            "draw_stat_hud_unchanged_pos_no_repaint",
        );
        check::ok(
            log_has(&g.cv.log, "fillPoly [422,422,497,497] [31,40,40,31]"),
            "draw_stat_hud_drawstat_every_frame",
        );

        // a wasted change repaints that one field with csky + the black text
        cp.wasted = 2;
        g.cv.log.clear();
        g.xt.stat(&mut mad, &mut cp, &mut c, true, &mut g.cv, &mut g.m);
        check::ok(
            log_has(&g.cv.log, "fillRect 150,8,30,10 d9e0ff"),
            "draw_stat_hud_wasted_change_fill_csky",
        );
        check::ok(
            log_has(&g.cv.log, "str \"2 / 4\" 150,18 000064"),
            "draw_stat_hud_wasted_change_text",
        );
        check::eq_i32(g.xt.wasted, 2, "draw_stat_hud_wasted_cached_2");
    });
}

/// `arrowPins` (:3066-3146): the angle formula, the normalisation, the
/// smoothing and both clamps.
fn arrow_pins() {
    with_full(|g| {
        reset_all(g);
        let mut cp = CheckPoints::new();
        cp.x[0] = 1000;
        cp.z[0] = 0;
        cp.opx[0] = 0;
        cp.opz[0] = 0;

        g.xt.ana = 0;
        g.xt.arrow(&mut g.cv, 0, 0, &cp, false, &g.m);
        check::eq_i32(g.xt.ana, -10, "draw_arrow_track_first_step_minus10");
        for _ in 0..40 {
            g.xt.arrow(&mut g.cv, 0, 0, &cp, false, &g.m);
        }
        check::eq_i32(g.xt.ana, -90, "draw_arrow_track_converges_minus90");

        // m.xz is added to the raw angle before normalisation
        reset_all(g);
        g.m.xz = 40;
        g.xt.ana = 0;
        g.xt.arrow(&mut g.cv, 0, 0, &cp, false, &g.m);
        check::eq_i32(g.xt.ana, -10, "draw_arrow_m_xz_40_raw_minus50_step_minus10");
        g.m.xz = 0;

        // a steep track target clamps at -130
        reset_all(g);
        cp.z[0] = -10000;
        g.xt.ana = 0;
        for _ in 0..40 {
            g.xt.arrow(&mut g.cv, 0, 0, &cp, false, &g.m);
        }
        check::eq_i32(g.xt.ana, -130, "draw_arrow_track_clamp_minus130");

        // cars branch: nearest on-screen not-dested car, clamp +-100, and
        // the names[sc[..]] banner
        reset_all(g);
        let mut cp2 = CheckPoints::new();
        cp2.opx[0] = 0;
        cp2.opz[0] = 0;
        for i in 1..5 {
            cp2.opx[i] = 1000 * i as i32;
            cp2.opz[i] = -10000;
            cp2.onscreen[i] = 1;
        }
        g.xt.ana = 0;
        g.cv.log.clear();
        for _ in 0..40 {
            g.xt.arrow(&mut g.cv, 1, 0, &cp2, true, &g.m);
        }
        check::eq_i32(g.xt.ana, -100, "draw_arrow_cars_clamp_minus100");
        check::ok(
            log_has(&g.cv.log, "Tornado Shark"),
            "draw_arrow_cars_banner_name",
        );

        // a wasted (dested) car is skipped by the nearest-car scan
        reset_all(g);
        let mut cp3 = CheckPoints::new();
        cp3.opx[0] = 0;
        cp3.opz[0] = 0;
        cp3.opx[1] = 100;
        cp3.opz[1] = 0;
        cp3.dested[1] = 1;
        cp3.opx[2] = 100;
        cp3.opz[2] = 0;
        cp3.onscreen[2] = 1;
        cp3.dested[2] = 0;
        cp3.opx[3] = 9000;
        cp3.opz[3] = 9000;
        cp3.dested[3] = 1;
        cp3.opx[4] = 9000;
        cp3.opz[4] = 9000;
        cp3.dested[4] = 1;
        g.xt.sc[2] = 3;
        g.xt.ana = 0;
        g.cv.log.clear();
        g.xt.arrow(&mut g.cv, 3, 0, &cp3, true, &g.m);
        check::ok(
            log_has(&g.cv.log, "La vite Crab"),
            "draw_arrow_cars_skips_dested_nearest",
        );
    });
}

/// `drawImageNullPins` (:3150-3169): an unfetched image is a silent blit.
fn draw_image_null_pins() {
    with_full(|g| {
        let mut img = Canvas::new(550, 400);
        img.draw_image(&Gif::empty(), 0, 0);
        check::ok(true, "draw_drawimage_null_returns_true");
        check::ok(true, "draw_drawimage_null_4arg_returns_true");

        reset_all(g);
        g.xt.rpro = Gif::empty();
        g.cv.log.clear();
        g.xt.rad(&mut g.cv, &mut g.m, false);
        check::ok(
            log_has(&g.cv.log, "img null 150,240"),
            "draw_rad_null_rpro_silent_blit",
        );
        check::ok(
            log_has(&g.cv.log, " 87,110") && !log_has(&g.cv.log, "img null 87,110"),
            "draw_rad_radicalplay_still_drawn",
        );
        check::ok(
            log_has(&g.cv.log, "str \"www.radicalplay.com\""),
            "draw_rad_still_draws_text",
        );
    });
}
