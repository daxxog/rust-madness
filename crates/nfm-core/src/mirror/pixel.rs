// FIDELITY: PixelSuite rig mirrors Java `Race.toRaceStart()` (`:1023-1026`), which mutates state; renaming to appease to_* convention would break 1:1 naming.
#![allow(clippy::wrong_self_convention)]
//! `tests/pixel/PixelSuite.java` — the screens, atlas and font-probe sections
//! (PixelSuite.screens `:817-923`, atlasVerify `:681-770` and fontProbe
//! `:469-509`).
//!
//! This is the pixel platform's SECOND Java suite: `Suite.java`'s 20 logic
//! classes stay in [`crate::mirror::mirror_suite`], and this module reports
//! its own line. The ground truth is the committed glyph atlas
//! (`tests/fixtures/xtfont-atlas/`, B12_pixel.md §4) plus the two font-probe
//! frame md5s in `pixel.toml [probes]`; the probe re-renders
//! `"The Introductory Stage"` black-on-white at (10,40) into a fresh 550x400
//! frame by BLITTING THE ATLAS CELLS through `Canvas::draw_string`, exactly
//! the model the production text path uses. If the coverage extraction, the
//! cell placement or the SRC_OVER rounding is wrong, these md5s move.
//!
//! Java ids are used verbatim where the assertion is the same one; the
//! JDK-only font-identity family is not fabricated here:
//! - the font-identity probes (`font_sansserif_bold_{13,11}_raster_equals_
//!   shipped_ttf`, `font_logical13_name_composite`, `font_physical13_name/
//!   family`, `font_metrics_{13,11}_logical_equals_physical`) compare the
//!   logical vs physical JDK fonts - there is one atlas in the port.
//!
//! The four SHA-256 file pins (`atlas_manifest_sha256`,
//! `atlas_txt_sha256_matches_manifest`, `atlas_ttf_file_matches_manifest`,
//! `atlas_png_matches_manifest`) and `font_source_ttf_sha256` are landed with
//! the mirror-local, cfg(test) FIPS 180-4 helper in [`super::sha256`]: the
//! same bytes Java's `MessageDigest` hashes, no new dependency.
//!
//! Our atlas section re-parses the manifest and re-decodes the PNG
//! INDEPENDENTLY of `xtfont` for the advance/cell checks, so a loader
//! indexing bug cannot hide behind a tautology.
//!
//! The screens section (`:817-923`) is driven through the REAL `XtGraphics`
//! methods into a fresh 550x400 canvas per pin, with the Java rig's field
//! writes and tick counts; each md5 is over the FULL frame (the rows are
//! strictly stronger than the masked inline xt captures). All 43 `screen_*`
//! rows are landed (car 4 = Nimi, the user's named repro state, joined the
//! car0/car9 stats family in the play-truth stretch): the clicknow pair uses
//! ONE Graphics for `loading()` + `clicknow()`, the game-truth sequence
//! (GameSparker.java:1507/592/597/:655 - the applet's persistent `rd`),
//! after the fixture fix + re-bless.
//!
//! The race section ([`race`]) lands all 36 `race_*` rows — the stage-1
//! countdown ladder, the mid-race 10/50/100 physics frames, the stat/
//! drawstat/arrow/levelhigh composites, the pause/flex transforms, the
//! after-race branch and the stage-5/10 first frames — as documented in its
//! own header. All 84 expected.md5 rows are landed, including the blessed
//! `atlas_manifest_sha256` digest row (the C1 SHA-256 deferral, closed by the
//! mirror-local helper).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::canvas::Canvas;
use crate::checkpoints::{CheckPoints, PLAYERS};
use crate::conto::{Base, Conto};
use crate::control::Control;
use crate::hash::md5_hex;
use crate::madness::Madness;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::sha256;
use crate::png;
use crate::radicalmod::RadicalMod;
use crate::shell::{Game, Shell, build_cars};
use crate::trackers::Trackers;
use crate::xtfont;
use crate::xtgraphics::XtGraphics;

pub(crate) fn run() {
    check::group("Pixel");
    let pixel = Tml::load("pixel.toml");
    pixel.require_keys(&["atlas", "probes", "frames", "frame"]);
    let atlas = pixel.table("atlas");
    let probes = pixel.table("probes");
    let frames = pixel.table("frames");
    let frame = pixel.table("frame");
    probes.require_keys(&["font_probe_13_md5", "font_probe_11_md5"]);
    frames.require_keys(&["file", "row_count"]);
    frame.require_keys(&["w", "h"]);

    // Java's main() runs fontProbe -> atlasVerify -> screens (`:137-140`)
    // under the one seeded Math.random() injection; the mirror's runner
    // installs the same seed (mirror/mod.rs). The row consumer is the
    // expected.md5 baseline, loaded once like PixelSuite.loadExpected.
    let mut rows = Rows::load(&frames.str("file"));
    font_probe(&mut rows, &probes, &atlas);
    atlas_verify(&mut rows, &atlas);
    screens(&mut rows);
    race(&mut rows);

    // ---- the two-file shape checks (PixelSuite.main `:144-153`) ----------
    // FULL LANDING: Java's verify mode asserts `PINNED == EXPECTED.size()`
    // (`:146`); all 84 rows (the atlas trio, the two font-probe frames, the
    // 43 screens and the 36 race frames) are produced by this run, so the
    // mirror now uses that exact form. `pinned` counts emissions, so a
    // duplicate or a renamed/dropped pin still moves the sum and fails; the
    // 0-remainder pin below is the mirror-only diagnostic that flags a
    // row-count-neutral id drift.
    let unlanded: Vec<&String> = rows
        .expected
        .keys()
        .filter(|id| !rows.pinned.contains(id))
        .collect();
    check::eq_i32(
        rows.pinned.len() as i32,
        rows.expected.len() as i32,
        "expected_md5_row_count_matches_pins",
    );
    check::eq_i32(
        unlanded.len() as i32,
        0,
        "expected_md5_unlanded_row_count_0",
    );
    // The screen and race sections are complete (the clicknow pair landed
    // with the fixture's one-Graphics fix; the race rows in this batch).
    let unlanded_screens: Vec<&String> = unlanded
        .iter()
        .copied()
        .filter(|id| id.starts_with("screen_"))
        .collect();
    check::eq_i32(
        unlanded_screens.len() as i32,
        0,
        "screen_rows_unlanded_count_0",
    );
    let unlanded_race: Vec<&String> = unlanded
        .iter()
        .copied()
        .filter(|id| id.starts_with("race_"))
        .collect();
    check::eq_i32(unlanded_race.len() as i32, 0, "race_rows_unlanded_count_0");
    check::eq_i32(
        frames.intv("row_count"),
        rows.expected.len() as i32,
        "frames_row_count_matches_expected_file",
    );
    check::eq_str(
        format!("{}x{}", frame.intv("w"), frame.intv("h")),
        format!("{}x{}", CANVAS_W, CANVAS_H),
        "frame_geometry_matches_canvas",
    );
}

/// The 550x400 `TYPE_INT_RGB` platform canvas (`pixel.toml [frame]`).
const CANVAS_W: i32 = 550;
const CANVAS_H: i32 = 400;

// ── the expected.md5 row consumer (PixelSuite `:174-220`) ────────────────

/// `PixelSuite.loadExpected` + `pin`: the committed baseline is the pin.
/// A row the run emits but the file does not carry fails with Java's
/// `<id>_missing_from_expected_md5` id; a value mismatch fails with the
/// row id itself. The mirror never blesses (the file is the artifact).
struct Rows {
    expected: BTreeMap<String, String>,
    /// The row ids this run produced, in emission order — the landed
    /// subset of Java's `PINNED` (all 84 of 84 rows are landed; the
    /// count/0-remainder pins in [`run`] enforce it).
    pinned: Vec<String>,
}

impl Rows {
    fn load(file: &str) -> Rows {
        let path = repo_path(file);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => {
                // exactly the Java arm: record the failure and keep going,
                // so every pin below also reports itself missing.
                check::ok(false, "expected_md5_file_missing");
                String::new()
            }
        };
        let mut expected = BTreeMap::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((id, value)) = line.split_once(' ') {
                expected.insert(id.to_string(), value.trim().to_string());
            }
        }
        Rows {
            expected,
            pinned: Vec::new(),
        }
    }

    /// `PixelSuite.pin(id, got)` (`:182-194`).
    fn pin(&mut self, id: &str, got: &str) {
        self.pinned.push(id.to_string());
        match self.expected.get(id) {
            Some(want) => check::eq_str(got, want, id),
            None => check::ok(false, &format!("{id}_missing_from_expected_md5")),
        }
    }
}

/// The repo-relative path probe (`tests/pixel/expected.md5` from either the
/// workspace root or the crate directory) — the same two-candidate rule as
/// the TOML loader.
fn repo_path(rel: &str) -> PathBuf {
    for cand in [format!("../../{rel}"), rel.to_string()] {
        let p = PathBuf::from(&cand);
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(rel)
}

/// The shipped TTF's bytes (`pixel.toml [atlas] ttf_path`), resolved through
/// the same two-candidate repo rule as the TOML/expected.md5 files.
fn read_ttf(atlas: &Tml) -> Vec<u8> {
    let path = repo_path(&atlas.str("ttf_path"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// `PixelSuite.md5(BufferedImage)`: the 550x400 ARGB ints serialized
/// big-endian (channel order fixed, `:1146-1170`). `TYPE_INT_RGB` forces
/// alpha 0xff, so each pixel is `ff R G B`.
fn frame_md5(cv: &Canvas) -> String {
    let mut bytes = Vec::with_capacity(cv.w * cv.h * 4);
    for px in cv.px.chunks_exact(3) {
        bytes.extend_from_slice(&[0xff, px[0], px[1], px[2]]);
    }
    md5_hex(&bytes)
}

/// The `probeFrame` recipe (`:452-463`): a fresh white frame, black text at
/// (10,40). The Java call uses `new Font("SansSerif", BOLD, size)`; the
/// atlas row IS that font, so the port draws the same string at the same
/// size.
fn probe_md5(size: i32) -> String {
    let mut cv = Canvas::new(550, 400);
    cv.fill([255, 255, 255]);
    cv.draw_string("The Introductory Stage", 10, 40, [0, 0, 0], size);
    frame_md5(&cv)
}

/// `fontProbe()` (`:469-509`): the two frame pins are the batch's oracle.
/// The line-metric pins carry over (they come from the same FontMetrics the
/// atlas manifest records) and the shipped TTF's own digest is landed through
/// the mirror-local SHA-256; the JDK font-identity half is not portable.
fn font_probe(rows: &mut Rows, probes: &Tml, atlas: &Tml) {
    let probe13 = probe_md5(13);
    check::eq_str(
        &probe13,
        probes.str("font_probe_13_md5"),
        "font_probe_13_md5_matches_manifest",
    );
    rows.pin("font_probe_13_md5", &probe13);
    let probe11 = probe_md5(11);
    check::eq_str(
        &probe11,
        probes.str("font_probe_11_md5"),
        "font_probe_11_md5_matches_manifest",
    );
    rows.pin("font_probe_11_md5", &probe11);

    let a = xtfont::atlas();
    // the Java Metrics.toString shape (`:522-525`)
    for size in [13, 11] {
        let m = a.metrics(size);
        check::eq_str(
            format!(
                "ascent={} descent={} leading={} maxAdvance={} height={}",
                m.ascent, m.descent, m.leading, m.max_advance, m.height
            ),
            if size == 13 {
                "ascent=12 descent=3 leading=1 maxAdvance=17 height=16"
            } else {
                "ascent=10 descent=3 leading=0 maxAdvance=15 height=13"
            },
            &format!("font_metrics_{size}_values"),
        );
    }

    // `font_source_ttf_sha256` (`:508`): the shipped face's digest, through
    // the mirror-local SHA-256 (same bytes, no JDK MessageDigest).
    let ttf_sha = sha256::hex(&read_ttf(atlas));
    check::eq_str(&ttf_sha, atlas.str("sha256_ttf"), "font_source_ttf_sha256");
}

/// One decoded atlas cell cut out of the PNG by the MIRROR (independent of
/// `xtfont`): same rule, coverage = 255 - gray with a neutrality check.
fn raw_cell(img: &png::Image, a: &xtfont::Atlas, size: i32, cp: u32) -> Vec<u8> {
    let row = if size == a.sizes()[0] { 0 } else { 1 };
    let col = (cp - a.codepoint_min) as usize;
    let mut out = vec![0u8; a.cell_w * a.cell_h];
    for cy in 0..a.cell_h {
        for cx in 0..a.cell_w {
            let p = ((row * a.cell_h + cy) * img.w + col * a.cell_w + cx) * 3;
            let (r, g, b) = (img.rgb[p], img.rgb[p + 1], img.rgb[p + 2]);
            assert!(
                r == g && r == b,
                "atlas ink is not neutral at cell ({col},{cy},{cx})"
            );
            out[cy * a.cell_w + cx] = 255 - r;
        }
    }
    out
}

/// `atlasVerify()` (`:681-770`): the manifest's shape/facts, the PNG cut,
/// the 190 advances and the 126-cell sample. Checks whose Java body needs
/// the JDK font or a SHA-256 are listed in the module header.
fn atlas_verify(rows: &mut Rows, atlas: &Tml) {
    let dir = xtfont::fixture_dir();
    let txt = std::fs::read_to_string(dir.join("atlas.txt")).expect("atlas.txt readable");
    let man = parse_manifest(&txt);
    let a = xtfont::atlas();

    // ---- shape: the manifest vs pixel.toml (the suite constants) --------
    check::eq_str(&a.format, atlas.str("format"), "atlas_format_tag");
    check::eq_i32(a.columns as i32, atlas.intv("columns"), "atlas_columns_95");
    check::eq_i32(a.rows as i32, atlas.intv("rows"), "atlas_rows_2");
    check::eq_i32(
        a.cell_w as i32,
        atlas.intv("cell_w"),
        "atlas_cell_width_matches_suite",
    );
    check::eq_i32(
        a.cell_h as i32,
        atlas.intv("cell_h"),
        "atlas_cell_height_matches_suite",
    );
    check::eq_i32(
        a.baseline,
        atlas.intv("baseline"),
        "atlas_baseline_matches_suite",
    );
    check::eq_i32(
        a.glyph_dx,
        atlas.intv("glyph_dx"),
        "atlas_glyph_dx_matches_suite",
    );
    check::eq_i32(
        a.codepoint_min as i32,
        atlas.intv("codepoint_min"),
        "atlas_codepoint_min_32",
    );
    check::eq_i32(
        a.codepoint_max as i32,
        atlas.intv("codepoint_max"),
        "atlas_codepoint_max_126",
    );
    let sizes = a.sizes();
    check::eq_str(
        format!("{sizes:?}"),
        format!("{:?}", atlas.int_arr("sizes")),
        "atlas_sizes_13_11",
    );

    // ---- the artifact digests (`PixelSuite.sha256` at `:690-704`) --------
    // `atlas_manifest_sha256` is the BLESSED `expected.md5` row: the
    // manifest file's own digest (the PNG/TTF are pinned transitively through
    // its embedded keys). `atlas_txt_sha256_matches_manifest` then checks the
    // file against pixel.toml's copy of that digest; the TTF/PNG file pins
    // hash the actual bytes, exactly Java's `sha256(new File(...))` calls.
    let txt_sha = sha256::hex(txt.as_bytes());
    rows.pin("atlas_manifest_sha256", &txt_sha);
    check::eq_str(
        &txt_sha,
        atlas.str("sha256_manifest"),
        "atlas_txt_sha256_matches_manifest",
    );
    check::eq_str(
        man.get("sha256_ttf").cloned().unwrap_or_default(),
        atlas.str("sha256_ttf"),
        "atlas_manifest_ttf_sha256",
    );
    check::eq_str(
        sha256::hex(&read_ttf(atlas)),
        man.get("sha256_ttf").cloned().unwrap_or_default(),
        "atlas_ttf_file_matches_manifest",
    );
    check::eq_str(
        a.sha256_ttf.clone(),
        atlas.str("sha256_ttf"),
        "atlas_loader_ttf_sha256",
    );
    let png_bytes = std::fs::read(dir.join("atlas.png")).expect("atlas.png readable");
    check::eq_str(
        sha256::hex(&png_bytes),
        atlas.str("sha256_png"),
        "atlas_png_matches_manifest",
    );
    check::eq_str(
        man.get("ttf").cloned().unwrap_or_default(),
        atlas.str("ttf_path"),
        "atlas_ttf_path_matches_manifest",
    );

    // ---- the glyph records ----------------------------------------------
    let glyph_lines: Vec<&str> = txt
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("glyph "))
        .collect();
    check::eq_i32(
        glyph_lines.len() as i32,
        (a.columns * a.rows) as i32,
        "atlas_glyph_record_count_190",
    );
    rows.pin("atlas_glyph_count", &glyph_lines.len().to_string());

    // ---- the PNG, decoded by the mirror itself --------------------------
    let img = png::decode(&png_bytes);
    check::ok(true, "atlas_png_readable");
    check::eq_i32(
        img.w as i32,
        (a.columns * a.cell_w) as i32,
        "atlas_png_width",
    );
    check::eq_i32(img.h as i32, (a.rows * a.cell_h) as i32, "atlas_png_height");

    // ---- all 190 advances, re-read from the manifest text ----------------
    let mut adv_mismatch = 0;
    for line in &glyph_lines {
        let f: Vec<&str> = line["glyph ".len()..].split_ascii_whitespace().collect();
        let size: i32 = f[0].parse().expect("glyph size");
        let cp: u32 = f[1].parse().expect("glyph codepoint");
        let adv: i32 = f[2].parse().expect("glyph advance");
        if a.advance(size, cp) != adv {
            adv_mismatch += 1;
        }
    }
    check::eq_i32(adv_mismatch, 0, "atlas_advance_mismatch_count_0");

    // ---- the 126-cell raster sample (space + digits + upper + lower) ----
    let sample = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut sampled = 0;
    let mut cell_mismatch = 0;
    for size in [13, 11] {
        for ch in sample.chars() {
            let cp = ch as u32;
            if raw_cell(&img, a, size, cp) != a.cell(size, cp) {
                cell_mismatch += 1;
            }
            sampled += 1;
        }
    }
    check::eq_i32(cell_mismatch, 0, "atlas_sampled_cell_mismatch_count_0");
    check::eq_i32(sampled, 126, "atlas_sampled_cell_count_126");
    rows.pin("atlas_sampled_cell_count", &sampled.to_string());
}

// ── the screens (PixelSuite.screens `:817-923`) ─────────────────────────

/// The one-constructor fixture (`PixelSuite.setup` `:412-433`): the port's
/// ctor spine (`XtGraphics::new()` + `loadpak`, which ends at dnload 485 /
/// kbload 514 on the deploy variant), the two audio seams replaced by
/// device-free `RecMod` stand-ins, and `snap(1)` — the mandatory
/// predecessor for loadingmusic/star (`xtGraphics.java:2564-2566`).
struct Rig {
    xt: XtGraphics,
    m: Medium,
    /// The contour roster's own medium (`PixelSuite.loadCars`' `mm`,
    /// `:1100-1129`): a fresh default `Medium` shared by every fresh car,
    /// exactly the object `ContO.d` reads in the Java fixture.
    car_m: Medium,
    ctl: Control,
    t: Trackers,
    /// The ten roster models, one live contour per model — the port's
    /// stand-in for the Java `loadbase` pool entry carselect mutates in
    /// place (B12 §6.1).
    cars: Vec<Conto>,
}

impl Rig {
    fn new() -> Rig {
        let mut xt = XtGraphics::new();
        xt.loadpak();
        xt.stages = Some(rec_mod("stages"));
        xt.cars = Some(rec_mod("cars"));
        let m = Medium::new();
        xt.snap(1, &m);
        let ctl = Control::new();
        let bases = crate::stage::loadbase().expect("the roster models");
        let mut t = Trackers::new();
        let scratch = Medium::new();
        let cars = (0..10)
            .map(|i| Conto::place(&bases[i], &scratch, &mut t, 0, 0, 0, 0))
            .collect();
        Rig {
            xt,
            m,
            car_m: Medium::new(),
            ctl,
            t,
            cars,
        }
    }

    /// `preset()` (`:1081-1090`): the TControl walk with a large cntrn
    /// budget (Medium.random only refills from the seeded stream at 0; the
    /// walk is identical for every positive budget).
    fn preset(&mut self) {
        self.m.cntrn = 1_000_000;
        self.m.rand = [5, 5, 5];
        self.m.diup = [false; 3];
        self.m.trn = 0;
    }

    /// `freshCars()` (`:1092-1098`): one detached contour per roster model,
    /// cloned against the roster's own medium (the Java copy ctor's `m`).
    fn fresh_cars(&self) -> Vec<Conto> {
        self.cars
            .iter()
            .map(|c| Conto::clone_from(c, &self.car_m, 0, 0, 0, 0))
            .collect()
    }
}

/// `RecMod` (`:263-291`): a RadicalMod whose play/stop/resume only record.
fn rec_mod(tag: &str) -> RadicalMod {
    let mut r = RadicalMod::new(b"not a module image", 0, 0, 0);
    r.tag = format!("mod:{tag}");
    r
}

fn fresh() -> Canvas {
    Canvas::new(CANVAS_W as usize, CANVAS_H as usize)
}

/// `PixelSuite.screens()` (`:817-923`): the boot/loading/menu frames in
/// emission order. Every pin is an md5 over the FULL frame — the rows are
/// strictly stronger than the masked inline xt captures, which is what the
/// folding in xtgraphics.rs relies on.
fn screens(rows: &mut Rows) {
    let mut rig = Rig::new();

    // ---- nofocus (xt:1101-1113). drawcs var7==3 never reads Medium, so
    // the Java pin runs with m==null.
    {
        let mut cv = fresh();
        rig.xt.aflk = false;
        rig.xt.nofocus(&mut cv, &rig.m);
        rows.pin("screen_nofocus", &frame_md5(&cv));
    }

    // ---- loading (xt:2897-2928): the dnload 485 / kbload 514 end state
    // and the 0 KB head.
    rows.pin(
        "screen_loading_final_95pct",
        &frame_md5(&render_loading(&mut rig, 485)),
    );
    rows.pin(
        "screen_loading_start_0kb",
        &frame_md5(&render_loading(&mut rig, 0)),
    );

    // ---- clicknow (xt:3364-3376), the fase-111 overlay on the ctor's
    // loading canvas (GameSparker.java:649-655). The fixture paints BOTH
    // calls on ONE persistent Graphics (the applet's `rd`, :1507/:592/
    // :597/:655), so the caption rasterizes at the loading call's 11pt -
    // exactly this rig's fresh-canvas-then-loading-then-clicknow sequence
    // (`render_clicknow`).
    rows.pin(
        "screen_clicknow_over_loading_aflk_false",
        &frame_md5(&render_clicknow(&mut rig, false)),
    );
    rows.pin(
        "screen_clicknow_over_loading_aflk_true",
        &frame_md5(&render_clicknow(&mut rig, true)),
    );

    // ---- rad = fase 9 (xt:3390-3412); var2 is GameSparker's var23 tick.
    for tick in 0..=1 {
        rows.pin(
            &format!("screen_rad_tick{tick}_aflk_false"),
            &frame_md5(&render_rad(&mut rig, tick, false)),
        );
        rows.pin(
            &format!("screen_rad_tick{tick}_aflk_true"),
            &frame_md5(&render_rad(&mut rig, tick, true)),
        );
    }
    check::eq_str(
        frame_md5(&render_rad(&mut rig, 0, false)),
        frame_md5(&render_rad(&mut rig, 199, false)),
        "screen_rad_tick_paint_neutral",
    );

    // ---- maini = fase 10 (xt:3160-3272).
    for op in 0..=2 {
        rows.pin(
            &format!("screen_maini_opselect{op}"),
            &frame_md5(&render_maini(&mut rig, op, false)),
        );
    }
    rows.pin(
        "screen_maini_opselect1_shaded",
        &frame_md5(&render_maini(&mut rig, 1, true)),
    );

    // ---- inst = fase 11 (xt:320-369). 0/2/4 are transition draws whose
    // canvases pre-roll through 1/3/5, exactly the Java fixture.
    for f in 0..=5 {
        rows.pin(
            &format!("screen_inst_flipo{f}"),
            &frame_md5(&render_inst(&mut rig, f)),
        );
    }

    // ---- credits = fase 8 (xt:1452-1513), pages 0..3.
    for f in 0..=3 {
        rows.pin(
            &format!("screen_credits_flipo{f}"),
            &frame_md5(&render_credits(&mut rig, f)),
        );
    }

    // ---- carselect = fase 7 (xt:3557-3704): the SECOND tick's canvas.
    rows.pin(
        "screen_carselect_car0_stats",
        &frame_md5(&render_carselect(&mut rig, 0, 0, 1, 0, false, 2)),
    );
    // car 4 = Nimi, the user's named repro state (unlocked=1 selects it: the
    // gate arm (4-4)*2 >= 1 is false - the T2 capture proved Java == Rust).
    rows.pin(
        "screen_carselect_car4_stats",
        &frame_md5(&render_carselect(&mut rig, 4, 0, 1, 0, false, 2)),
    );
    rows.pin(
        "screen_carselect_locked_gatey0",
        &frame_md5(&render_carselect(&mut rig, 5, 0, 1, 0, false, 2)),
    );
    rows.pin(
        "screen_carselect_locked_gatey300",
        &frame_md5(&render_carselect(&mut rig, 5, 0, 1, 300, false, 2)),
    );
    rows.pin(
        "screen_carselect_countdown_flipo20",
        &frame_md5(&render_carselect(&mut rig, 0, 20, 1, 300, true, 2)),
    );
    rows.pin(
        "screen_carselect_car9_stats",
        &frame_md5(&render_carselect(&mut rig, 9, 0, 11, 0, false, 2)),
    );

    // ---- stageselect = fase 1 (xt:2485-2538): the handler layer on black
    // (the fase-1 flyover backdrop is GameSparker-side, B12 §6.5).
    rows.pin(
        "screen_stageselect_stage1",
        &frame_md5(&render_stageselect(&mut rig, 1, 0, 0)),
    );
    rows.pin(
        "screen_stageselect_stage5",
        &frame_md5(&render_stageselect(&mut rig, 5, 0, 0)),
    );
    rows.pin(
        "screen_stageselect_stage5_buttons_pressed",
        &frame_md5(&render_stageselect(&mut rig, 5, 1, 1)),
    );
    rows.pin(
        "screen_stageselect_stage11",
        &frame_md5(&render_stageselect(&mut rig, 11, 0, 0)),
    );

    // ---- loadingfailed = fase 3 (xt:3982-4000).
    rows.pin(
        "screen_loadingfailed_stage1",
        &frame_md5(&render_loadingfailed(&mut rig, 1)),
    );
    rows.pin(
        "screen_loadingfailed_stage10",
        &frame_md5(&render_loadingfailed(&mut rig, 10)),
    );

    // ---- cantgo = fase 4 (xt:263-294): lockcnt is paint-neutral.
    rows.pin(
        "screen_cantgo_aflk_false_red",
        &frame_md5(&render_cantgo(&mut rig, 1, 70, false)),
    );
    rows.pin(
        "screen_cantgo_aflk_true_black",
        &frame_md5(&render_cantgo(&mut rig, 1, 70, true)),
    );
    rows.pin(
        "screen_cantgo_unlocked10",
        &frame_md5(&render_cantgo(&mut rig, 10, 70, false)),
    );
    check::eq_str(
        frame_md5(&render_cantgo(&mut rig, 1, 70, false)),
        frame_md5(&render_cantgo(&mut rig, 1, 7, false)),
        "screen_cantgo_lockcnt_paint_neutral",
    );

    // ---- musicomp = fase 6 (xt:3274-3362).
    for p in 0..=2 {
        rows.pin(
            &format!("screen_musicomp_stage1_pstar{p}"),
            &frame_md5(&render_musicomp(&mut rig, 1, 1, p)),
        );
    }
    rows.pin(
        "screen_musicomp_stage10_pstar0",
        &frame_md5(&render_musicomp(&mut rig, 10, 1, 0)),
    );
    rows.pin(
        "screen_musicomp_stage2_pstar0",
        &frame_md5(&render_musicomp(&mut rig, 2, 1, 0)),
    );
}

fn render_loading(rig: &mut Rig, dnload: i32) -> Canvas {
    let mut cv = fresh();
    rig.xt.dnload = dnload;
    rig.xt.loading(&mut cv, &rig.m);
    cv
}

fn render_clicknow(rig: &mut Rig, aflk: bool) -> Canvas {
    let mut cv = fresh();
    rig.xt.dnload = 485;
    rig.xt.loading(&mut cv, &rig.m); // the 11pt metrics clicknow centers with
    rig.xt.aflk = aflk;
    rig.xt.clicknow(&mut cv, &rig.m);
    cv
}

fn render_rad(rig: &mut Rig, tick: i32, aflk: bool) -> Canvas {
    let mut cv = fresh();
    rig.preset();
    rig.xt.aflk = aflk;
    rig.xt.rad(&mut cv, &mut rig.m, tick == 0);
    cv
}

fn render_maini(rig: &mut Rig, opselect: i32, shaded: bool) -> Canvas {
    let mut cv = fresh();
    rig.xt.flipo = 0;
    rig.xt.opselect = opselect;
    rig.xt.shaded = shaded;
    rig.xt.aflk = false;
    rig.xt.maini(&mut cv, &mut rig.ctl);
    cv
}

fn render_inst(rig: &mut Rig, target: i32) -> Canvas {
    let mut cv = fresh();
    let mut c = Control::new();
    rig.xt.flipo = 0;
    c.enter = false;
    rig.xt.inst(&mut cv, &mut c); // 0 -> 1, paints inst1 + next
    if target == 0 || target == 1 {
        if target == 1 {
            rig.xt.inst(&mut cv, &mut c);
        }
        return cv;
    }
    c.enter = true;
    rig.xt.inst(&mut cv, &mut c); // 1 -> 2
    c.enter = false;
    rig.xt.inst(&mut cv, &mut c); // 2 -> 3, paints inst2 + next
    if target == 2 || target == 3 {
        if target == 3 {
            rig.xt.inst(&mut cv, &mut c);
        }
        return cv;
    }
    c.enter = true;
    rig.xt.inst(&mut cv, &mut c); // 3 -> 4
    c.enter = false;
    rig.xt.inst(&mut cv, &mut c); // 4 -> 5, paints inst3 + contin2
    if target == 4 {
        return cv;
    }
    rig.xt.inst(&mut cv, &mut c); // 5 stays 5, repaints contin2
    cv
}

fn render_credits(rig: &mut Rig, target: i32) -> Canvas {
    let mut cv = fresh();
    rig.preset();
    rig.xt.flipo = target;
    rig.xt.aflk = false;
    rig.xt.credits(&mut cv, &mut rig.m, &mut Control::new());
    cv
}

fn render_carselect(
    rig: &mut Rig,
    car: i32,
    flipo: i32,
    unlocked: i32,
    gatey: i32,
    nextc: bool,
    ticks: i32,
) -> Canvas {
    let mut cv = fresh();
    rig.xt.sc[0] = car;
    rig.xt.unlocked = unlocked;
    rig.xt.gatey = gatey;
    rig.xt.nextc = nextc;
    rig.xt.pback = 0;
    rig.xt.pnext = 0;
    rig.xt.pcontin = 0;
    let mut models = rig.fresh_cars();
    let mad = Madness::new(0);
    let mut c = Control::new();
    // The real flipo>0 countdown begins from the seated idle frame.
    if flipo > 0 {
        rig.xt.flipo = 0;
        rig.preset();
        rig.xt.aflk = false;
        let idx = rig.xt.sc[0] as usize;
        rig.xt.carselect(
            &mut c,
            &mut models[idx],
            &mad,
            &mut cv,
            &mut rig.m,
            &rig.t,
            Some(&mut rig.car_m),
        );
    }
    rig.xt.flipo = flipo;
    for _ in 0..ticks {
        rig.preset();
        rig.xt.aflk = false;
        let idx = rig.xt.sc[0] as usize;
        rig.xt.carselect(
            &mut c,
            &mut models[idx],
            &mad,
            &mut cv,
            &mut rig.m,
            &rig.t,
            Some(&mut rig.car_m),
        );
    }
    cv
}

fn render_stageselect(rig: &mut Rig, stage: i32, pback: i32, pnext: i32) -> Canvas {
    let mut cv = fresh();
    let mut cp = CheckPoints::new();
    cp.stage = stage;
    cp.name = "The Introductory Stage".to_string();
    rig.xt.pback = pback;
    rig.xt.pnext = pnext;
    rig.xt.pcontin = 0;
    rig.xt
        .stageselect(&mut cv, &mut cp, &mut rig.m, &mut Control::new());
    cv
}

fn render_loadingfailed(rig: &mut Rig, stage: i32) -> Canvas {
    let mut cv = fresh();
    rig.xt.pcontin = 0;
    rig.xt
        .loadingfailed(stage, &mut cv, &rig.m, &mut Control::new());
    cv
}

fn render_cantgo(rig: &mut Rig, unlocked: i32, lockcnt: i32, aflk: bool) -> Canvas {
    let mut cv = fresh();
    rig.xt.unlocked = unlocked;
    rig.xt.lockcnt = lockcnt;
    rig.xt.aflk = aflk;
    rig.xt.pback = 0;
    rig.xt.cantgo(&mut cv, &rig.m, &mut Control::new());
    cv
}

fn render_musicomp(rig: &mut Rig, stage: i32, unlocked: i32, pstar: i32) -> Canvas {
    let mut cv = fresh();
    rig.xt.pstar = pstar;
    rig.xt.unlocked = unlocked;
    rig.xt.musicomp(stage, &mut cv, &rig.m, &mut Control::new());
    cv
}

// ── the race frames (PixelRace.java) ────────────────────────────────────

// All 36 race rows are landed (C3b part 1 + part 2). The pump and its
// state cross-checks are the fixture's; nothing is blessed or faked: a row
// is pinned only when it is byte-exact.
//
// The Java-truth fixes this section forced, in order:
// - `Plane::d`'s underside (`behind`) heuristic had both the vertex swap
//   and the spy comparison inverted;
// - `Medium.random()`'s default entropy now reads Java's ONE global
//   `Math.random()` slot instead of a private per-Medium stream;
// - `Madness.colide`'s x/z self-push used the OTHER car's moment indexed
//   by the other's car number; the source reads `var2.moment[this.cn]`
//   (Madness.java:486/528) — with two different car models the dent force
//   is off by the moment ratio (the pass-58 regz −375 vs −225);
// - `Plane.d`'s three random-roll comparisons (`embos <= 11 && > 0.5`,
//   the chip spark's `> 0.6`, the flx cull's `> 0.3`) were compared in f32
//   where the source promotes the float roll to double first
//   (Plane.java:237/513/842). A roll of exactly 0.6f is 0.6000000238… as a
//   double, so Java kills the spark and the port did not; that reordered
//   the shared Medium draw stream from physics pass 59 on, cascading into
//   the tint and band edges of every later composite.

/// `PixelRace.Race` (`:968-1039`): one pumped applet plus its stage graph.
/// The Java class builds a throw-away `GameSparker` subclass whose
/// `repaint()` is a per-pass gate; the port runs the same pass body
/// through [`Shell::frame`] (which IS one pass) and keeps the canvas
/// between passes, so each `step()` here is one `App.step()` there.
struct RaceRig {
    bases: Vec<Base>,
    xt: XtGraphics,
    m: Medium,
    game: Game,
    shell: Shell,
    cv: Canvas,
    stage_n: i32,
    /// The fixture's own physics-pass counter (`Race.physics`), test-only.
    physics: i32,
    /// The synthetic frame clock: 16 ms per pass keeps the fps machine's
    /// 530 ms windows in the same (unextended) arm the container pump
    /// takes, and `adjstfade` a no-op because `fade[0] == origfade`.
    clock: u64,
}

impl RaceRig {
    /// `Race.open(stage)` (`:987-997`): the run() prologue's graph, the
    /// cookie seam (`readcookie("unlocked") == stage`, everything else -1),
    /// then pass 1 - the fase-111 clicknow arm.
    fn open(stage_n: i32) -> RaceRig {
        let bases = crate::stage::loadbase().expect("the roster models");
        let m = Medium::new();
        let cars = build_cars(&bases, &m);
        let mut game = Game::new(cars);
        let shell = Shell::new();
        let mut xt = XtGraphics::new();
        xt.loadpak();
        // GameSparker.java:610-621 - the unlocked/stage cookie.
        xt.unlocked = stage_n;
        if stage_n != 11 {
            game.cp.stage = stage_n;
        }
        xt.opselect = 0;
        let mut rig = RaceRig {
            bases,
            xt,
            m,
            game,
            shell,
            cv: Canvas::new(CANVAS_W as usize, CANVAS_H as usize),
            stage_n,
            physics: 0,
            clock: 0,
        };
        // pass 1: fase 111 (clicknow); loadStage()'s pass 2 is next.
        rig.step();
        rig
    }

    /// `Race.loadStage()` (`:1002-1019`). The Java pump parks INSIDE
    /// `loadingstage`'s own `this.app.repaint()` (xtGraphics.java:317),
    /// which is where the fixture injects `fase = 0`: `loadstage` then
    /// skips its `fase == 2` promotion and the run loop's same-pass
    /// continuation is the starcnt==90 reset arm. The port has no
    /// repaint, so the rig splits the pass at exactly that point: draw
    /// `loadingstage`, inject fase 0, run the fase-2 arm's `loadstage`
    /// plus its post-parse tail (the shell's `:362-386` lines, which the
    /// Java method keeps inside `loadstage` itself), then one shell pass
    /// - the same Java pass's fase-0 arm. The re-run `xt.snap(stage)` is
    ///   the Java PixelGrabber normalisation (B12 §8.4); the port's snap is
    ///   a pure derivation of the decoded paks, so re-running it is a no-op
    ///   kept for pump fidelity.
    fn load_stage(&mut self) {
        self.xt.fase = 2;
        self.xt.loadingstage(self.stage_n, &mut self.cv, &self.m);
        self.xt.fase = 0;
        self.complete_load();
        self.step();
        self.xt.snap(self.stage_n, &self.m);
    }

    /// The fase-2 arm's `loadstage` plus the post-parse tail, exactly the
    /// shell's `:362-386` lines (GameSparker.java:526-586 keeps the tail
    /// inside `loadstage`; the port returns the scene and the shell does
    /// the control/car/recorder resets). The rig must run them here
    /// because it is emulating the fixture's mid-pass gate.
    fn complete_load(&mut self) {
        let loaded = crate::stage::loadstage(
            self.stage_n,
            &self.bases,
            &mut self.m,
            &mut self.game.t,
            &mut self.game.cp,
            &mut self.xt,
        );
        if let Ok(mut scene) = loaded {
            crate::stage::reset_controls(&self.game.cp, &mut self.game.ctl);
            for (i, (md, o)) in self
                .game
                .mad
                .iter_mut()
                .zip(scene.pool.iter_mut())
                .enumerate()
            {
                md.reseto(self.xt.sc[i] as usize, o, &mut self.game.cp);
            }
            self.game.rpd.reset(&scene.pool, &self.m);
            for i in 0..PLAYERS {
                let model = self.xt.sc[i] as usize;
                self.game.newcar[model] = Some(Conto::clone_from(
                    &self.game.cars[model],
                    &self.m,
                    0,
                    0,
                    0,
                    0,
                ));
            }
            self.game.starcnt = self.xt.starcnt;
            self.game.view = 0;
            self.game.scene = Some(scene);
        }
        self.game.ctl[0].falseo();
    }

    /// `Race.toRaceStart()` (`:1023-1026`): jump the ladder; the next pass
    /// enters the physics block. Stages 5/10 only.
    fn to_race_start(&mut self) {
        // KXT.intv("starcnt_race") == 0
        self.xt.starcnt = 0;
        self.step();
    }

    fn step(&mut self) {
        let start = self.clock;
        self.clock += 16;
        let _ = self.shell.frame(
            &mut self.game,
            &self.bases,
            &mut self.xt,
            &mut self.m,
            &mut self.cv,
            start,
            self.clock,
        );
    }

    fn frame(&self) -> &Canvas {
        &self.cv
    }
}

fn uniform(cv: &Canvas, rgb: [u8; 3]) -> bool {
    cv.px.chunks_exact(3).all(|p| p == rgb)
}

fn pin_cv(rows: &mut Rows, id: &str, cv: &Canvas) {
    rows.pin(id, &frame_md5(cv));
}

/// `PixelRace.pins()` (`:231-268`): the pin ORDER is the fixture's —
/// stage 1's ladder, mid-race ticks, composites, transforms and after-race
/// branch, then the stage-5 and stage-10 first frames.
fn race(rows: &mut Rows) {
    let kxt = Tml::load("constants.toml").table("xt");
    let mut one = RaceRig::open(1);
    one.load_stage();
    countdown_ladder(rows, &mut one, &kxt);
    let backdrop = mid_race(rows, &mut one);
    variants(rows, &mut one, &backdrop, &kxt);
    transforms(rows, &mut one, &backdrop);
    after_race(rows, &mut one);

    for stage in [5, 10] {
        let mut r = RaceRig::open(stage);
        r.load_stage();
        r.to_race_start();
        pin_cv(
            rows,
            &format!("race_stage{stage}_starcnt0_first_race_frame"),
            r.frame(),
        );
    }
}

/// `PixelRace.countdownLadder` (`:274-345`).
fn countdown_ladder(rows: &mut Rows, r: &mut RaceRig, kxt: &Tml) {
    check::eq_i32(
        r.xt.starcnt,
        kxt.intv("starcnt_start") - 1,
        "race_stage1_starcnt90_reset_exits_89",
    );
    check::eq_i32(
        r.m.adv,
        kxt.intv("camera_adv_start") - kxt.intv("camera_around_adv_step"),
        "race_stage1_starcnt90_adv_1900_then_around_minus20",
    );
    check::eq_i32(
        r.m.vxz,
        kxt.intv("camera_vxz_start") + kxt.intv("camera_around_vxz_step"),
        "race_stage1_starcnt90_vxz_70_then_around_plus4",
    );
    check::ok(
        uniform(r.frame(), [255, 255, 255]),
        "race_stage1_starcnt90_reset_frame_is_white",
    );
    let reset_white = frame_md5(r.frame());
    pin_cv(rows, "race_stage1_starcnt90_reset_white", r.frame());

    r.step(); // enters 89 -> 88: the first reset-camera draw
    check::eq_i32(
        r.xt.starcnt,
        kxt.intv("starcnt_start") - 2,
        "race_stage1_starcnt89_first_decrement_exits_88",
    );
    pin_cv(rows, "race_stage1_starcnt89_first_decrement", r.frame());

    step_until(r, kxt.intv("starcnt_landing"));
    check::eq_i32(
        r.xt.starcnt,
        kxt.intv("starcnt_landing"),
        "race_stage1_starcnt35_landing_observed_35",
    );
    check::eq_i32(
        r.m.adv,
        kxt.intv("camera_adv_landing"),
        "race_stage1_starcnt35_landing_adv_900",
    );
    check::eq_i32(
        r.m.vxz,
        kxt.intv("camera_vxz_landing"),
        "race_stage1_starcnt35_landing_vxz_180",
    );
    check::ok(!r.m.vert, "race_stage1_starcnt35_landing_vert_false");
    check::eq_i32(r.shell.mouses, 0, "race_stage1_starcnt35_landing_mouses_0");
    check::eq_i32(r.m.flex, 1, "race_stage1_starcnt35_landing_flex_0_to_1");
    check::ok(
        uniform(r.frame(), [255, 255, 255]),
        "race_stage1_starcnt35_landing_frame_is_white",
    );
    check::eq_str(
        frame_md5(r.frame()),
        reset_white,
        "race_stage1_reset_and_landing_white_frames_identical",
    );
    pin_cv(rows, "race_stage1_starcnt35_landing_white", r.frame());

    r.step(); // 35 -> 34: the first VISIBLE follow()+stat frame
    check::eq_i32(
        r.xt.starcnt,
        kxt.intv("starcnt_visible"),
        "race_stage1_starcnt34_first_visible_hud_observed_34",
    );
    check::eq_i32(r.m.flex, 2, "race_stage1_starcnt34_full_panel_flex_1_to_2");
    check::eq_i32(
        r.xt.gocnt,
        kxt.intv("gocnt_start"),
        "race_stage1_starcnt34_gocnt_3",
    );
    pin_cv(
        rows,
        "race_stage1_starcnt34_first_visible_hud_full_panel",
        r.frame(),
    );

    r.step(); // 34 -> 33: m.flex==2, the incremental arm
    check::eq_i32(r.m.flex, 2, "race_stage1_incremental_first_flex_stays_2");
    let incremental = frame_md5(r.frame());
    pin_cv(rows, "race_stage1_incremental_hud_first", r.frame());

    r.step(); // 33 -> 32
    check::eq_i32(r.m.flex, 2, "race_stage1_incremental_second_flex_stays_2");
    check::eq_str(
        frame_md5(r.frame()),
        incremental,
        "race_stage1_incremental_pass_is_idempotent",
    );
    pin_cv(rows, "race_stage1_incremental_hud_second", r.frame());

    step_until(r, kxt.intv("starcnt_two"));
    check::eq_i32(
        r.xt.gocnt,
        kxt.intv("gocnt_two"),
        "race_stage1_gocnt2_at_starcnt24",
    );
    pin_cv(rows, "race_stage1_gocnt2_starcnt24", r.frame());

    step_until(r, kxt.intv("starcnt_one"));
    check::eq_i32(
        r.xt.gocnt,
        kxt.intv("gocnt_one"),
        "race_stage1_gocnt1_at_starcnt13",
    );
    pin_cv(rows, "race_stage1_gocnt1_starcnt13", r.frame());

    step_until(r, kxt.intv("starcnt_go"));
    check::eq_i32(
        r.xt.gocnt,
        kxt.intv("gocnt_go"),
        "race_stage1_gocnt0_at_starcnt2",
    );
    pin_cv(rows, "race_stage1_gocnt0_go_sprite", r.frame());

    step_until(r, kxt.intv("starcnt_race"));
    check::eq_i32(
        r.xt.starcnt,
        kxt.intv("starcnt_race"),
        "race_stage1_countdown_reaches_0",
    );

    r.step(); // first physics pass: colide/drive/rec/checkstat/preform
    check::eq_i32(
        r.xt.starcnt,
        kxt.intv("starcnt_race"),
        "race_stage1_first_physics_frame_starcnt_0",
    );
    check::eq_i32(r.xt.holdcnt, 0, "race_stage1_no_result_panel_holdcnt_0");
    check::ok(!r.xt.winner, "race_stage1_no_result_panel_winner_false");
    r.physics = 1;
    pin_cv(rows, "race_stage1_starcnt0_full_race_frame", r.frame());
}

/// `PixelRace.midRace` (`:351-382`) — the 50/100-tick physics rows, the
/// live-race state check and the canonical composite backdrop.
fn mid_race(rows: &mut Rows, r: &mut RaceRig) -> Canvas {
    check::eq_i32(r.xt.sc[0], 0, "race_stage1_player_car_0");
    let mut distinct = 1;
    for i in 1..5 {
        let mut seen = false;
        for j in 0..i {
            if r.xt.sc[i] == r.xt.sc[j] {
                seen = true;
            }
        }
        if !seen {
            distinct += 1;
        }
    }
    check::eq_i32(distinct, 5, "race_stage1_five_distinct_cars");
    check::eq_i32(r.xt.sc[4], 5, "race_stage1_stage1_rival_car_4_plus_half");

    step_to(r, 10);
    pin_cv(rows, "race_stage1_tick10_midrace", r.frame());
    step_to(r, 50);
    pin_cv(rows, "race_stage1_tick50_midrace", r.frame());
    step_to(r, 100);
    pin_cv(rows, "race_stage1_tick100_midrace", r.frame());
    check::eq_i32(r.xt.fase, 0, "race_stage1_tick100_still_fase_0");
    check::eq_i32(r.xt.cntwis, 0, "race_stage1_tick100_cntwis_0");
    copy_cv(r.frame())
}

fn copy_cv(cv: &Canvas) -> Canvas {
    let mut out = Canvas::new(cv.w, cv.h);
    out.px.copy_from_slice(&cv.px);
    out
}

/// `PixelRace.resetStat` (`:871-928`): the handler-level reset of every
/// field stat/drawstat/arrow/levelhigh read. The Java fixture's `r.mad` is
/// the race's own standalone `Madness` (constructed at `Race` ctor, never a
/// physics slot); a fresh one here is the same declared state.
fn reset_stat(r: &mut RaceRig, mad: &mut Madness, ctl: &mut Control, starcnt: i32) {
    let x = &mut r.xt;
    x.fase = 0;
    x.starcnt = starcnt;
    x.gocnt = 0;
    x.holdcnt = 0;
    x.winner = false;
    x.wasted = 0;
    x.cntwis = 0;
    x.posit = 0;
    x.laps = 0;
    x.clear = 0;
    x.arrace = false;
    x.ana = 0;
    x.cntan = 0;
    x.cntovn = 0;
    x.flk = false;
    x.tcnt = 30;
    x.tflk = false;
    x.wasay = false;
    x.say = String::new();
    x.dmcnt = 0;
    x.dmflk = false;
    x.pwcnt = 0;
    x.pwflk = false;
    x.auscnt = 45;
    x.aflk = false;
    x.crashup = false;
    x.skflg = 0;
    x.dskflg = 0;
    x.loop_ = String::new();
    x.spin = String::new();
    x.asay = String::new();
    x.dested = [0; 5];

    mad.dest = false;
    mad.missedcp = 0;
    mad.point = 0;
    mad.mtouch = false;
    mad.hitmag = 0;
    mad.newcar = false;
    mad.nlaps = 0;
    mad.clear = 0;
    mad.power = 75.0;
    mad.im = 0;
    mad.cn = 0;

    ctl.enter = false;
    ctl.handb = false;
    ctl.arrace = false;
    ctl.up = false;
    ctl.down = false;
    ctl.left = false;
    ctl.right = false;
    ctl.mutes = false;
    ctl.mutem = false;
}

fn fresh_cp() -> CheckPoints {
    let mut cp = CheckPoints::new();
    cp.nlaps = 0;
    cp.nsp = 0;
    cp.stage = 1;
    cp.wasted = 0;
    cp.haltall = false;
    cp
}

/// `PixelRace.arrowCp` (`:942-962`): the player at the origin and one rival
/// ahead-right and on screen.
fn arrow_cp() -> CheckPoints {
    let mut cp = fresh_cp();
    cp.n = 8;
    cp.opx[0] = 0;
    cp.opz[0] = 0;
    cp.opx[1] = 900;
    cp.opz[1] = 1800;
    cp.onscreen[0] = 40;
    cp.onscreen[1] = 40;
    cp.x[0] = 1200;
    cp.z[0] = 2400;
    cp.x[1] = 900;
    cp.z[1] = 1800;
    for i in 0..5 {
        cp.dested[i] = 0;
    }
    cp
}

/// `PixelRace.variants` (`:413-649`): the stat/drawstat/arrow/levelhigh
/// composite rows, each painted over a fresh copy of the real 100-tick
/// frame.
fn variants(rows: &mut Rows, r: &mut RaceRig, base: &Canvas, kxt: &Tml) {
    // The Java fixture's own handler-level pair (`Race.mad`/`Race.ctl`),
    // never the race's live slots.
    let mut mad = Madness::new(0);
    let mut ctl = Control::new();
    let starcnt = kxt.intv("starcnt_race");

    // ---- stat: wasted==4, flex==2 (the winner panel) -------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.xt.wasted = 4;
    r.m.flex = 2;
    r.xt.holdcnt = 0;
    let mut cv = copy_cv(base);
    ctl.enter = false;
    let mut cp = fresh_cp();
    r.xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut r.m);
    check::ok(r.xt.winner, "race_stat_wasted4_winner_true");
    check::ok(cp.haltall, "race_stat_wasted4_haltall_true");
    check::eq_i32(r.xt.holdcnt, 1, "race_stat_wasted4_holdcnt_1");
    pin_cv(rows, "race_stat_wasted4_flex2_winner_panel", &cv);

    // ---- the same panel with flex==0 (the csky erase rectangle) --------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.xt.wasted = 4;
    r.m.flex = 0;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut fresh_cp(), &mut ctl, true, &mut cv, &mut r.m);
    check::eq_i32(r.m.flex, 1, "race_stat_wasted4_flex0_bumped_to_1");
    pin_cv(rows, "race_stat_wasted4_flex0_csky_erase", &cv);

    // ---- holdcnt>250 auto-continue (:1616-1622) ------------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.xt.wasted = 4;
    r.m.flex = 2;
    r.xt.holdcnt = 251;
    r.xt.fase = 0;
    let mut cv = copy_cv(base);
    r.xt.stat(
        &mut mad,
        &mut fresh_cp(),
        &mut ctl,
        false,
        &mut cv,
        &mut r.m,
    );
    check::eq_i32(
        r.xt.fase,
        -2,
        "race_stat_holdcnt_251_auto_continue_fase_minus2",
    );
    check::eq_i32(r.xt.holdcnt, 252, "race_stat_holdcnt_251_incremented_252");

    // ---- stat: player wasted (var1.dest && cntwis==8) -------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.xt.cntwis = 8;
    mad.dest = true;
    r.m.flex = 2;
    let mut cv = copy_cv(base);
    let mut cp = fresh_cp();
    r.xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut r.m);
    check::ok(!r.xt.winner, "race_stat_player_wasted_winner_false");
    check::ok(!cp.haltall, "race_stat_player_wasted_haltall_untouched");
    pin_cv(rows, "race_stat_player_wasted_panel", &cv);

    // ---- stat: AI first ------------------------------------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 2;
    let mut cp = fresh_cp();
    cp.pos[0] = 4;
    cp.pos[1] = 0;
    cp.clear[0] = 0;
    cp.clear[1] = 0;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut r.m);
    check::ok(!r.xt.winner, "race_stat_ai_first_winner_false");
    check::ok(cp.haltall, "race_stat_ai_first_haltall_true");
    pin_cv(rows, "race_stat_ai_first_panel", &cv);

    // ---- stat: player finished first -----------------------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 2;
    let mut cp = fresh_cp();
    cp.pos[0] = 0;
    cp.clear[0] = 0;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut r.m);
    check::ok(r.xt.winner, "race_stat_player_first_winner_true");
    pin_cv(rows, "race_stat_player_first_panel", &cv);

    // ---- the arrow machine: missed checkpoint --------------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 2;
    r.xt.starcnt = starcnt;
    r.xt.arrace = false;
    ctl.arrace = false;
    r.xt.ana = 0;
    r.xt.flk = false;
    mad.missedcp = 20;
    mad.point = 1;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut arrow_cp(), &mut ctl, true, &mut cv, &mut r.m);
    check::ok(r.xt.flk, "race_arrow_missedcp_flk_toggles_to_true");
    check::eq_i32(mad.missedcp, 21, "race_arrow_missedcp_incremented");
    pin_cv(rows, "race_arrow_missedcp_flash_orange", &cv);

    // second call: the flash machine has flipped flk
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut arrow_cp(), &mut ctl, true, &mut cv, &mut r.m);
    check::ok(!r.xt.flk, "race_arrow_missedcp_flk_toggles_back");
    pin_cv(rows, "race_arrow_missedcp_flash_red", &cv);

    // ---- cars-onscreen arrow -------------------------------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 2;
    r.xt.starcnt = starcnt;
    r.xt.arrace = true;
    ctl.arrace = true;
    mad.missedcp = 0;
    let mut cp = arrow_cp();
    cp.onscreen[1] = 40;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut cp, &mut ctl, true, &mut cv, &mut r.m);
    pin_cv(rows, "race_arrow_cars_onscreen", &cv);

    // ---- wrong way, red ------------------------------------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 2;
    r.xt.starcnt = starcnt;
    r.xt.arrace = false;
    r.xt.ana = 170;
    r.xt.cntan = 50;
    r.xt.cntovn = 0;
    r.xt.flk = false;
    ctl.arrace = false;
    mad.missedcp = 0;
    mad.mtouch = true;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut arrow_cp(), &mut ctl, true, &mut cv, &mut r.m);
    check::eq_i32(r.xt.cntovn, 1, "race_arrow_wrongway_cntovn_1");
    check::eq_i32(r.xt.cntan, 40, "race_arrow_wrongway_cntan_clamped_40");
    check::ok(r.xt.flk, "race_arrow_wrongway_flk_true");
    pin_cv(rows, "race_arrow_wrongway_flash_red", &cv);

    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 2;
    r.xt.starcnt = starcnt;
    r.xt.ana = 170;
    r.xt.cntan = 50;
    r.xt.cntovn = 0;
    r.xt.flk = true;
    mad.missedcp = 0;
    mad.mtouch = true;
    let mut cv = copy_cv(base);
    r.xt.stat(&mut mad, &mut arrow_cp(), &mut ctl, true, &mut cv, &mut r.m);
    check::ok(!r.xt.flk, "race_arrow_wrongway_flk_false");
    pin_cv(rows, "race_arrow_wrongway_flash_orange", &cv);

    // ---- drawstat ------------------------------------------------------
    for (id, max, hit, newcar, power, auscnt, aflk) in [
        (
            "race_drawstat_dmg_full_pwr_max",
            100,
            100,
            false,
            98.0,
            45,
            false,
        ),
        (
            "race_drawstat_dmg_mid_pwr_half",
            100,
            60,
            false,
            50.0,
            45,
            false,
        ),
        (
            "race_drawstat_dmg_low_newcar_erase",
            100,
            10,
            true,
            20.0,
            45,
            false,
        ),
        (
            "race_drawstat_pwr_low_flash",
            100,
            60,
            false,
            30.0,
            44,
            true,
        ),
    ] {
        reset_stat(r, &mut mad, &mut ctl, starcnt);
        r.m.flex = 2;
        r.xt.auscnt = auscnt;
        r.xt.aflk = aflk;
        let mut cv = copy_cv(base);
        r.xt.drawstat(&mut cv, max, hit, newcar, power, &r.m);
        pin_cv(rows, id, &cv);
    }

    // ---- levelhigh's three titles --------------------------------------
    for (id, wasted, whenwasted) in [
        ("race_levelhigh_you_wasted_em", 4, 0),
        ("race_levelhigh_wasted", 0, 229),
        ("race_levelhigh_stunts", 0, 0),
    ] {
        let mut cv = copy_cv(base);
        r.xt.levelhigh(&mut cv, wasted, whenwasted, &r.m);
        pin_cv(rows, id, &cv);
    }

    // ---- flex 0 and 1 take the same full-panel arm ---------------------
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 0;
    r.xt.starcnt = starcnt;
    let mut a0 = copy_cv(base);
    r.xt.stat(&mut mad, &mut fresh_cp(), &mut ctl, true, &mut a0, &mut r.m);
    let flex0 = frame_md5(&a0);
    reset_stat(r, &mut mad, &mut ctl, starcnt);
    r.m.flex = 1;
    r.xt.starcnt = starcnt;
    let mut a1 = copy_cv(base);
    r.xt.stat(&mut mad, &mut fresh_cp(), &mut ctl, true, &mut a1, &mut r.m);
    check::eq_str(
        frame_md5(&a1),
        flex0,
        "race_stat_flex0_and_flex1_arms_paint_identically",
    );
}

/// `PixelRace.transforms` (`:678-702`): pauseimage and fleximage over the
/// real 100-tick backdrop.
fn transforms(rows: &mut Rows, r: &mut RaceRig, base: &Canvas) {
    let mut cv = copy_cv(base);
    r.xt.pauseimage(&mut cv, &mut r.m);
    check::eq_i32(r.m.flex, 0, "race_pauseimage_sets_m_flex_0");
    pin_cv(rows, "race_pauseimage_over_race_frame", &cv);

    let mut cv = copy_cv(base);
    r.xt.fleximage(&mut cv, &mut r.m, 0);
    pin_cv(rows, "race_fleximage_phase0_hsb_hue_rotate", &cv);

    let mut cv = copy_cv(base);
    r.xt.fleximage(&mut cv, &mut r.m, 3);
    pin_cv(rows, "race_fleximage_phase3_dither_only", &cv);
}

/// `PixelRace.afterRace` (`:708-738`): the fase -2 branch. A caught crash
/// enters the -3 replay (black hold, then the levelhigh frame); otherwise
/// the -4 gameover screen.
fn after_race(rows: &mut Rows, r: &mut RaceRig) {
    r.xt.fase = -2;
    // The fixture's `r.ctl.enter/handb = false` writes land on the
    // handler-level Control, not the applet's live slot (this.u[0]), so
    // they are inert; the app's button state is released only by the
    // mouse field.
    r.shell.mouses = 0;
    r.step();
    let fase = r.xt.fase;
    check::ok(
        fase == -3 || fase == -4,
        "race_neg2_branch_is_minus3_or_minus4",
    );
    if fase == -3 {
        pin_cv(rows, "race_neg3_starcar_restore_black_hold", r.frame());
        let mut guard = 0;
        while guard < 80 {
            guard += 1;
            r.step();
            if !uniform(r.frame(), [0, 0, 0]) {
                break;
            }
        }
        check::ok(
            !uniform(r.frame(), [0, 0, 0]),
            "race_neg3_first_visible_frame_reached",
        );
        pin_cv(rows, "race_neg3_levelhigh_stunts_replay", r.frame());
    } else {
        pin_cv(rows, "race_neg4_gameover_mdness_over_trackbg", r.frame());
    }
}

fn step_to(r: &mut RaceRig, tick: i32) {
    while r.physics < tick {
        r.step();
        r.physics += 1;
    }
}

/// `PixelRace.stepUntil` (`:393-402`) with the fixture's stuck-countdown
/// guard: a broken ladder fails loudly instead of hanging the run.
fn step_until(r: &mut RaceRig, starcnt: i32) {
    let mut guard = 0;
    while r.xt.starcnt > starcnt {
        guard += 1;
        assert!(
            guard <= 400,
            "countdown stuck at {} waiting for {} (guard={guard})",
            r.xt.starcnt,
            starcnt
        );
        r.step();
    }
}

/// The manifest's `key=value` half (PixelSuite.parseManifest `:772-786`).
fn parse_manifest(txt: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for line in txt.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("glyph ") {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}
