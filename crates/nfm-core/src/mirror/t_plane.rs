//! `TPlane.java` — Plane, the polygon workhorse every visible game part
//! delegates to: the ctor field map / colour bake / `deltaf`, the pure
//! helpers (`ys`/`xs`/`spy`/`rot`), the deterministic `d()` arms (visibility
//! cull, `av`, brightness bands, outlines, road/glass frames), the chip and
//! embos state machines, and `s()`'s ground/tracker shadow projector. Group
//! "Plane" (`Suite.java` position 14); the Java probe executes **390** pins
//! (379 static `Check` sites — the preset walk, per-tracker loops and the
//! `want` arrays expand some sites). Pin source: `tests/java/TPlane.java`;
//! narrative: notes/JAVA-BEHAVIOR/B5_plane.md (the two accepted-open items
//! at B5:68-69 are pinned only as the Java class pins them: the NaN-fill
//! and n>3-intent rows are deliberately absent there and absent here).
//!
//! Fixture: `constants.toml [plane]` (Java `K`), the same four
//! `requireKeys` and the `av_probe` read, exactly like the Java class.
//! `presetRecipe` is NOT the determinism.toml `control_preset` row — the
//! Java class presets its own Medium state inline (cntrn 20, rand all
//! `start`, diup false, trn 2) and pins the first 12 values; those literals
//! are normative here too.
//!
//! Port-shape mapping (documented at each pin):
//! - `Plane.n` (Java's `int n`, Plane.java:10/:100) is `ox.len()`; the port
//!   stores the coordinate arrays without a separate count (the batch-5
//!   ContO/Wheels mapping), so `p.n` pins read the length.
//! - `Plane.m`/`Plane.t` (stored collaborator references) have no port
//!   field: `Plane::new` takes `&Medium` and `s()` takes `&Trackers` per
//!   call. Their identity pins are the compile-time call-site identities
//!   (the `t_conto`/`t_wheels` precedent), recorded with `ok(true)`.
//! - `PresetMedium.draws` is the walk counter delta `20 − cntrn`: every
//!   pinned stream stays inside the 20-call window before `Medium.random()`
//!   refills, so the delta equals the Java counter exactly. `FixedMedium`
//!   replays a constant through `RngSource::Vector` and counts its cursor
//!   (the `t_contodraw` seam).
//! - Java `null` Graphics is unrepresentable (`d`/`s` take `&mut Canvas`);
//!   the null-safe pins pass a real canvas and assert the state that the
//!   original promises for the null call (nothing drawn / no field write).
//! - Java's raw-array unchecked accesses are Type/Slice errors in the port:
//!   `n == -1` (NegativeArraySizeException), a short `int[] color` (fixed
//!   `[i32; 3]`), `rot(..., cnt=6)` on 3-slot arrays (the port's `rot` has
//!   no count), a null `Medium`/`Trackers` (`&` references), and the
//!   `p.t == null` field read are documented structural pins (`ok(true)`);
//!   the crashes the port CAN reach (`n == 2`, a short coordinate array)
//!   stay real `throws_it` pins.
//! - PIXELS: TPlane is the batch that forces the rasterizer question. The
//!   Java pins are point samples on a `BufferedImage`; the port's
//!   `Canvas::fill_polygon` now routes through the byte-faithful JDK 8
//!   `ProcessPath` fill lane in `roundrect.rs` (the `FillPath` native), so
//!   the spark polygon's exact 35-pixel count and every interior sample
//!   match the Oracle 8u40 measurement. `drawPolygon` outline pins sample
//!   only exact vertices / axis-aligned edges, which the integer Bresenham
//!   reproduces.
//! - `declaredFieldCount(Plane.class) == 41` has no Rust reflection: an
//!   exhaustive destructure counts the port's 38 stored fields and adds the
//!   three Java-only collaborators (`m`, `t`, `n`) as documented structural
//!   fields (the `t_small` precedent).

use std::panic::AssertUnwindSafe;

use crate::canvas::Canvas;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::bits;
use crate::plane::{Plane, rot};
use crate::random::RngSource;
use crate::trackers::Trackers;

pub(crate) fn run() {
    check::group("Plane");
    // TPlane.java:58,67 — the class-level load and the shape pin.
    let k = Tml::load("constants.toml").table("plane");
    k.require_keys(&[
        "deltaf_3_4_5",
        "projf_3_4_5",
        "deltaf_sample_bits",
        "av_far",
    ]);

    preset_recipe();
    field_defaults();
    ctor_mapping();
    typ_pins();
    color_pins();
    deltaf_projf(&k);
    helpers();
    ctor_errors();
    d_basics();
    d_flx();
    d_projf(&k);
    d_trk(&k);
    d_outline();
    d_errors();
    s_machines();
    s_trackers();
    s_modes();
    s_culling();
    embos_ranges();
    embos_chip();
    extra_arms();
}

/// The B5 seam recipe (`TPlane.PresetMedium`): the real `Medium.random()`
/// walk started at cntrn 20 / rand all `start` / trn 2 / diup false. Each
/// call walks the three digits one step and returns `rand[trn]/10`.
struct Preset {
    m: Medium,
}

impl Preset {
    fn new(start: i32) -> Preset {
        let mut m = Medium::new();
        m.cntrn = 20;
        m.rand = [start; 3];
        m.diup = [false; 3];
        m.trn = 2;
        Preset { m }
    }

    fn random(&mut self) -> f32 {
        self.m.random()
    }

    /// `PresetMedium.draws` — one walk step per call while cntrn > 0, and
    /// every pinned stream is shorter than the 20-call refill window.
    fn draws(&self) -> i32 {
        20 - self.m.cntrn
    }
}

/// `TPlane.FixedMedium` — a constant stream; the cursor is the draw count
/// (the `t_contodraw` vector seam).
struct Fixed {
    m: Medium,
}

impl Fixed {
    fn new(v: f32) -> Fixed {
        let mut m = Medium::new();
        m.rng = Some(RngSource::Vector(vec![v; 64], 0));
        Fixed { m }
    }

    fn draws(&self) -> i32 {
        match &self.m.rng {
            Some(RngSource::Vector(_, i)) => *i as i32,
            _ => 0,
        }
    }
}

/// `TPlane.mk` minus the `Trackers` argument (no port field; `s()` takes the
/// tracker at the call site).
#[allow(clippy::too_many_arguments)]
fn mk(
    m: &Medium,
    ox: &[i32],
    oz: &[i32],
    oy: &[i32],
    n: usize,
    oc: [i32; 3],
    glass: bool,
    gr: i32,
    wx: i32,
    wy: i32,
    wz: i32,
    disline: i32,
    bfase: i32,
    road: bool,
) -> Plane {
    Plane::new(
        m, ox, oz, oy, n, oc, glass, gr, wx, wy, wz, disline, bfase, road,
    )
}

/// `TPlane.plain` — the three-vertex plane used for field-default reads.
fn plain() -> Plane {
    mk(
        &Medium::new(),
        &[0, 10, 0],
        &[0, 0, 10],
        &[0, 0, 0],
        3,
        [1, 2, 3],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    )
}

/// `TPlane.sample` — the canonical `d()` fixture (identity projection at
/// z == focus_point, black outline, interior point (277,202)).
fn sample(m: &Medium) -> Plane {
    mk(
        m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    )
}

/// `TPlane.shadow` — the canonical `s()` fixture (z 400/420/440).
fn shadow(m: &Medium, _t: &Trackers) -> Plane {
    mk(
        m,
        &[275, 285, 275],
        &[400, 420, 440],
        &[200, 200, 210],
        3,
        [10, 20, 30],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    )
}

fn colored(m: &Medium, c: [i32; 3], glass: bool) -> Plane {
    colored_b(m, c, glass, 0)
}

fn colored_b(m: &Medium, c: [i32; 3], glass: bool, bfase: i32) -> Plane {
    mk(
        m,
        &[0, 10, 0],
        &[0, 0, 10],
        &[0, 0, 0],
        3,
        c,
        glass,
        0,
        0,
        0,
        0,
        7,
        bfase,
        false,
    )
}

fn faded(m: &mut Medium, v: i32) {
    for f in m.fade.iter_mut() {
        *f = v;
    }
}

/// `TPlane.matchedTracker` — every vertex matches the shadow fixture.
fn matched_tracker(y: i32, r: i32, g: i32, b: i32, radius: i32) -> Trackers {
    let mut t = Trackers::new();
    t.nt = 1;
    t.radx[0] = radius;
    t.radz[0] = radius;
    t.x[0] = 275;
    t.z[0] = 400;
    t.y[0] = y;
    t.c[0] = [r, g, b];
    t
}

/// `TPlane.image` — the 600x500 offscreen frame.
fn image() -> Canvas {
    Canvas::new(600, 500)
}

/// `TPlane.paint` — the frame pre-filled with the BG sentinel; "nothing
/// drawn" is then a flat buffer compare.
fn painted() -> Canvas {
    let mut cv = image();
    cv.fill([0x01, 0x02, 0x03]);
    cv
}

fn pixel(cv: &Canvas, x: usize, y: usize) -> i32 {
    let at = (y * cv.w + x) * 3;
    ((cv.px[at] as i32) << 16) | ((cv.px[at + 1] as i32) << 8) | cv.px[at + 2] as i32
}

fn blank_pixels() -> Vec<u8> {
    painted().px
}

fn count(cv: &Canvas, rgb: i32) -> i32 {
    let mut n = 0;
    for y in 0..cv.h {
        for x in 0..cv.w {
            if pixel(cv, x, y) == rgb {
                n += 1;
            }
        }
    }
    n
}

/// `TPlane.snapshot` — every Plane field `s()` could touch, as one string.
/// Floats are bit images so the comparison is exact and formatting-free.
fn snapshot(p: &Plane) -> String {
    let mut s = String::new();
    s.push_str(&format!("{:?}{:?}{:?}", p.ox, p.oy, p.oz));
    s.push_str(&format!(
        "|{}{}{}{}{}{}{}{}{}",
        p.ox.len(),
        p.glass,
        p.gr,
        p.disline,
        p.road,
        p.master,
        p.wx,
        p.wy,
        p.wz
    ));
    s.push_str(&format!("|{:?}{:?}", p.c, p.oc));
    s.push_str(&format!(
        "|{} {} {}",
        bits(p.hsb[0]),
        bits(p.hsb[1]),
        bits(p.hsb[2])
    ));
    s.push_str(&format!(
        "|{}{}{}{}{}",
        bits(p.deltaf),
        bits(p.projf),
        p.av,
        p.bfase,
        p.nocol
    ));
    s.push_str(&format!(
        "|{}{}{}{}{:?}{:?}{:?}{}",
        p.chip,
        bits(p.ctmag),
        p.cxz,
        p.cxy,
        p.cox,
        p.coz,
        p.coy,
        p.czy
    ));
    s.push_str(&format!(
        "|{}{}{}{}{}{}{}{}{}{}{}",
        p.dx, p.dy, p.dz, p.vx, p.vy, p.vz, p.embos, p.typ, p.pa, p.pb, p.flx
    ));
    s
}

/// `TPlane.trackerSnapshot` — every Trackers field as one string.
fn tracker_snapshot(t: &Trackers) -> String {
    format!(
        "{}|{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
        t.nt, t.x, t.y, t.z, t.xy, t.zy, t.skd, t.dam, t.c, t.radx, t.radz, t.rady
    )
}

// ------------------------------------------------------------- ctor state

/// The B5 seam recipe, pinned so every state-machine expectation below has
/// a documented, reproducible value stream (Medium.java:49-89).
fn preset_recipe() {
    let mut m = Preset::new(6);
    let want = [
        0.5f32, 0.4, 0.3, 0.2, 0.1, 0.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4,
    ];
    for (i, w) in want.iter().enumerate() {
        check::eq_i32(bits(m.random()), bits(*w), &format!("preset_value{i}"));
    }
    check::eq_i32(m.draws(), 12, "preset_twelve_calls");
    check::eq_i32(m.m.cntrn, 8, "preset_cntrn_20_minus_12");
    check::eq_i32(m.m.trn as i32, 2, "preset_trn_wraps_back_to_2");
    check::eq_str(
        format!("{:?}", m.m.rand),
        "[4, 4, 4]",
        "preset_rand_after_12",
    );

    // a start of 7 gives the "spark polygon drawn" branch (>0.5) first
    let mut hi = Preset::new(7);
    check::eq_i32(bits(hi.random()), bits(0.6), "preset_hi_first_0_6");
    // and a start of 3 gives 0.2, used for the flx draw arm
    let mut lo = Preset::new(3);
    check::eq_i32(bits(lo.random()), bits(0.2), "preset_lo_first_0_2");
}

/// Counts the destructured field bindings (the `t_small` reflection
/// substitute): the destructure stops compiling if the port's field set
/// drifts from the list.
macro_rules! plane_field_count {
    ($($f:ident),* $(,)?) => {
        [$( { let _ = &$f; 0u8 } ),*].len()
    };
}

/// Plane.java:5-45 — the 41 declared fields and the values a fresh
/// construction leaves at its initializer.
fn field_defaults() {
    let p = plain();

    #[allow(unused_variables)]
    {
        let Plane {
            ox,
            oy,
            oz,
            c,
            oc,
            hsb,
            glass,
            gr,
            disline,
            road,
            master,
            wx,
            wy,
            wz,
            deltaf,
            projf,
            av,
            bfase,
            nocol,
            embos,
            chip,
            ctmag,
            cxz,
            cxy,
            czy,
            cox,
            coz,
            coy,
            dx,
            dy,
            dz,
            vx,
            vy,
            vz,
            typ,
            pa,
            pb,
            flx,
        } = &p;
        let stored = plane_field_count!(
            ox, oy, oz, c, oc, hsb, glass, gr, disline, road, master, wx, wy, wz, deltaf, projf,
            av, bfase, nocol, embos, chip, ctmag, cxz, cxy, czy, cox, coz, coy, dx, dy, dz, vx, vy,
            vz, typ, pa, pb, flx,
        );
        // Java's 41 = these 38 stored fields + the three the port derives or
        // takes per call: m, t, n (n is ox.len()).
        check::eq_i32((stored + 3) as i32, 41, "field_count_41");
    }

    check::eq_i32(p.c.len() as i32, 3, "default_c_len3");
    check::eq_i32(p.oc.len() as i32, 3, "default_oc_len3");
    check::eq_i32(p.hsb.len() as i32, 3, "default_hsb_len3");
    check::eq_i32(p.av, 0, "default_av_0");
    check::eq_i32(p.chip, 0, "default_chip_0");
    check::eq_i32(bits(p.ctmag), bits(0.0), "default_ctmag_0");
    check::eq_i32(p.cxz, 0, "default_cxz_0");
    check::eq_i32(p.cxy, 0, "default_cxy_0");
    check::eq_i32(p.czy, 0, "default_czy_0");
    check::eq_str(format!("{:?}", p.cox), "[0, 0, 0]", "default_cox_zero");
    check::eq_str(format!("{:?}", p.coz), "[0, 0, 0]", "default_coz_zero");
    check::eq_str(format!("{:?}", p.coy), "[0, 0, 0]", "default_coy_zero");
    check::eq_i32(p.dx, 0, "default_dx_0");
    check::eq_i32(p.dy, 0, "default_dy_0");
    check::eq_i32(p.dz, 0, "default_dz_0");
    check::eq_i32(p.vx, 0, "default_vx_0");
    check::eq_i32(p.vy, 0, "default_vy_0");
    check::eq_i32(p.vz, 0, "default_vz_0");
    check::eq_i32(p.embos, 0, "default_embos_0");
    check::eq_i32(p.pa as i32, 0, "default_pa_0");
    check::eq_i32(p.pb as i32, 0, "default_pb_0");
    check::eq_i32(p.flx, 0, "default_flx_0");
    check::eq_i32(p.master, 0, "default_master_0");
    check::eq_i32(bits(p.projf), bits(1.0), "default_projf_1");
    check::ok(!p.glass, "default_glass_false");
    check::ok(!p.road, "default_road_false");
    check::ok(!p.nocol, "default_nocol_false");
    check::eq_i32(p.wx, 0, "default_wx_0");
    check::eq_i32(p.wy, 0, "default_wy_0");
    check::eq_i32(p.wz, 0, "default_wz_0");
    check::eq_i32(p.disline, 7, "default_disline_arg_passthrough_7");
}

/// Plane.java:81-109,159-200,201-223 — the exact 15-arg param -> field map;
/// param 4 is oz and param 5 is oy (the B5 trap).
fn ctor_mapping() {
    let m = Medium::new();
    let mut x = [1, 2, 3];
    let z = [4, 5, 6];
    let y = [7, 8, 9];
    // The Java fixture passes {11,22,33,44}; the ctor reads only the first
    // three (Plane.java:128-130) and the port's oc is exactly [i32; 3].
    let p = mk(
        &m,
        &x,
        &z,
        &y,
        3,
        [11, 22, 33],
        false,
        5,
        6,
        7,
        8,
        6,
        9,
        true,
    );

    check::eq_str(format!("{:?}", p.ox), "[1, 2, 3]", "ctor_p3_to_ox");
    check::eq_str(format!("{:?}", p.oz), "[4, 5, 6]", "ctor_p4_to_oz_not_oy");
    check::eq_str(format!("{:?}", p.oy), "[7, 8, 9]", "ctor_p5_to_oy_not_oz");
    check::eq_i32(p.ox.len() as i32, 3, "ctor_p6_to_n");
    check::eq_str(
        format!("{:?}", p.oc),
        "[11, 22, 33]",
        "ctor_p7_oc_first_three",
    );
    // `p.m == m` / `p.t == t`: per-call arguments in the port; the identity
    // is the compile-time call-site identity.
    check::ok(true, "ctor_p1_to_m");
    check::ok(true, "ctor_p2_to_t");
    check::eq_i32(p.gr, 5, "ctor_p9_to_gr");
    check::eq_i32(p.wx, 6, "ctor_p10_to_wx");
    check::eq_i32(p.wy, 7, "ctor_p11_to_wy");
    check::eq_i32(p.wz, 8, "ctor_p12_to_wz");
    check::eq_i32(p.disline, 6, "ctor_p13_to_disline");
    check::eq_i32(p.bfase, 9, "ctor_p14_to_bfase");
    check::ok(p.road, "ctor_p15_to_road");
    check::ok(!p.glass, "ctor_p8_false_to_glass");
    check::eq_i32(p.typ, 3, "ctor_tie_spans_fall_to_3");

    // the coordinate arrays are copied (Plane.java:101-109), so caller
    // mutation cannot reach the plane
    x[0] = 99;
    let _ = x[0];
    check::eq_i32(p.ox[0], 1, "ctor_copies_ox_defensively");

    // the ctor does not call loadprojf
    check::eq_i32(bits(p.projf), bits(1.0), "ctor_never_calls_loadprojf");
    check::eq_i32(
        p.av + p.chip + p.embos + p.flx,
        0,
        "ctor_spark_state_untouched",
    );
}

/// Plane.java:111-124 — typ is the smallest axis span; every `if` still
/// runs, so ties are overwritten 2 -> 1 -> 3.
fn typ_pins() {
    check::eq_i32(
        mk(
            &Medium::new(),
            &[0, 0, 0],
            &[0, 0, 100],
            &[0, 0, 400],
            3,
            [1, 2, 3],
            false,
            0,
            0,
            0,
            0,
            7,
            0,
            false,
        )
        .typ,
        1,
        "typ_1_x_smallest",
    );
    check::eq_i32(
        mk(
            &Medium::new(),
            &[0, 400, 0],
            &[0, 0, 100],
            &[0, 0, 0],
            3,
            [1, 2, 3],
            false,
            0,
            0,
            0,
            0,
            7,
            0,
            false,
        )
        .typ,
        2,
        "typ_2_y_smallest",
    );
    check::eq_i32(
        mk(
            &Medium::new(),
            &[0, 300, 0],
            &[0, 0, 0],
            &[0, 0, 400],
            3,
            [1, 2, 3],
            false,
            0,
            0,
            0,
            0,
            7,
            0,
            false,
        )
        .typ,
        3,
        "typ_3_z_smallest",
    );
    // all three spans equal -> the last if wins
    check::eq_i32(
        mk(
            &Medium::new(),
            &[0, 100, 0],
            &[0, 100, 0],
            &[0, 100, 0],
            3,
            [1, 2, 3],
            false,
            0,
            0,
            0,
            0,
            7,
            0,
            false,
        )
        .typ,
        3,
        "typ_all_equal_falls_to_3",
    );
    // x and y tie below z: 2 is set first, then 1 overwrites it
    check::eq_i32(
        mk(
            &Medium::new(),
            &[0, 100, 0],
            &[0, 200, 0],
            &[0, 100, 0],
            3,
            [1, 2, 3],
            false,
            0,
            0,
            0,
            0,
            7,
            0,
            false,
        )
        .typ,
        1,
        "typ_xy_tie_falls_2_to_1",
    );
}

/// Plane.java:129-194 — oc is the raw first three colour bytes; c is the
/// snap-scaled/clamped copy for solids, or the sky/fade blend for glass;
/// hsb is RGBtoHSB(c) with the bfase clamps only for a non-nocol, non-glass
/// plane. c/hsb are baked at construction (B5:63).
fn color_pins() {
    let a = colored(&Medium::new(), [100, 0, 200], false);
    check::eq_str(format!("{:?}", a.c), "[100, 0, 200]", "color_snap0_c_raw");
    check::eq_str(format!("{:?}", a.oc), "[100, 0, 200]", "color_snap0_oc_raw");
    check::ok(!a.nocol, "color_snap0_nocol_false");

    let mut m100 = Medium::new();
    m100.snap = [100, 100, 100];
    let b = colored(&m100, [100, 0, 200], false);
    check::eq_str(
        format!("{:?}", b.c),
        "[200, 0, 255]",
        "color_snap100_clamped_255",
    );

    let mut mneg = Medium::new();
    mneg.snap = [-50, -50, -50];
    let neg = colored(&mneg, [100, 0, 200], false);
    check::eq_str(format!("{:?}", neg.c), "[50, 0, 100]", "color_snapneg_half");

    // oc is never clamped, c always is
    let raw = colored(&Medium::new(), [300, -5, 0], false);
    check::eq_str(
        format!("{:?}", raw.oc),
        "[300, -5, 0]",
        "color_oc_never_clamped",
    );
    check::eq_str(
        format!("{:?}", raw.c),
        "[255, 0, 0]",
        "color_c_clamped_once",
    );

    let nc = colored(&Medium::new(), [10, 10, 10], false);
    check::ok(nc.nocol, "color_equal_rgb_sets_nocol");
    check::eq_str(
        format!("{:?}", nc.c),
        "[10, 10, 10]",
        "color_nocol_c_unsnapped",
    );
    check::eq_i32(bits(nc.hsb[0]), 0, "color_nocol_hsb0_0");
    check::eq_i32(bits(nc.hsb[1]), 0, "color_nocol_hsb1_0");
    check::eq_i32(bits(nc.hsb[2]), 1025548449, "color_nocol_hsb2_10_over_255");

    // glass ignores the colour argument and uses the sky/fade blend at the
    // default fade[0]=3000: (csky*6000 + cfade*3000) / 9000
    let g1 = colored(&Medium::new(), [1, 2, 3], true);
    let g2 = colored(&Medium::new(), [200, 100, 50], true);
    check::eq_str(
        format!("{:?}", g1.c),
        "[229, 222, 243]",
        "glass_c_sky_blend",
    );
    check::eq_str(
        format!("{:?}", g2.c),
        "[229, 222, 243]",
        "glass_c_ignores_color_arg",
    );
    check::eq_str(format!("{:?}", g1.oc), "[1, 2, 3]", "glass_oc_still_raw");
    check::eq_i32(bits(g1.hsb[0]), 1060692879, "glass_hsb0_blend");
    check::eq_i32(bits(g1.hsb[1]), 1035009239, "glass_hsb1_blend");
    check::eq_i32(bits(g1.hsb[2]), 1064563700, "glass_hsb2_blend");

    // bfase clamps: >20 sat, >30 bri floor+sat 0.1, >40 sat 0.05,
    // >50 hue 0.075 + bri cap 0.8, >60 hue 0.05 (Plane.java:163-194)
    let k70 = colored_b(&Medium::new(), [255, 0, 0], false, 70);
    check::eq_i32(bits(k70.hsb[0]), 1028443341, "bfase70_hue_0_05");
    check::eq_i32(bits(k70.hsb[1]), 1028443341, "bfase70_sat_0_05");
    check::eq_i32(bits(k70.hsb[2]), 1061997773, "bfase70_bri_0_8");
    let k25 = colored_b(&Medium::new(), [255, 0, 0], false, 25);
    check::eq_i32(bits(k25.hsb[1]), 1045220557, "bfase25_sat_only_0_2");
    check::eq_i32(bits(k25.hsb[2]), 1065353216, "bfase25_bri_untouched_1");
    let k21 = colored_b(&Medium::new(), [255, 0, 0], false, 21);
    check::eq_i32(
        bits(k21.hsb[1]),
        1045220557,
        "bfase21_just_over_20_clamps_sat",
    );
    let k31 = colored_b(&Medium::new(), [255, 0, 0], false, 31);
    check::eq_i32(bits(k31.hsb[1]), 1036831949, "bfase31_sat_0_1");
    check::eq_i32(bits(k31.hsb[2]), 1065353216, "bfase31_bri_above_half_kept");
    let k41 = colored_b(&Medium::new(), [255, 0, 0], false, 41);
    check::eq_i32(bits(k41.hsb[1]), 1028443341, "bfase41_sat_0_05");
    let k51 = colored_b(&Medium::new(), [255, 0, 0], false, 51);
    check::eq_i32(bits(k51.hsb[0]), 1033476506, "bfase51_hue_0_075");
    check::eq_i32(bits(k51.hsb[2]), 1061997773, "bfase51_bri_capped_0_8");
    let k61 = colored_b(&Medium::new(), [255, 0, 0], false, 61);
    check::eq_i32(bits(k61.hsb[0]), 1028443341, "bfase61_hue_0_05");

    // the >30 brightness floor raises a dark colour UP to 0.5
    let kdark = colored_b(&Medium::new(), [100, 0, 0], false, 31);
    check::eq_i32(bits(kdark.hsb[2]), 1056964608, "bfase31_bri_floored_0_5");

    // nocol short-circuits the whole clamp block (:163)
    let kn = colored_b(&Medium::new(), [128, 128, 128], false, 70);
    check::ok(kn.nocol, "bfase70_nocol_still_true");
    check::eq_i32(bits(kn.hsb[0]), 0, "bfase70_nocol_hue_untouched");
    check::eq_i32(bits(kn.hsb[1]), 0, "bfase70_nocol_sat_untouched");
    check::eq_i32(
        bits(kn.hsb[2]),
        1056997505,
        "bfase70_nocol_bri_128_over_255",
    );

    // glass short-circuits it too (:163 `!this.nocol && !this.glass`)
    let kg = colored_b(&Medium::new(), [255, 0, 0], true, 70);
    check::eq_i32(bits(kg.hsb[0]), 1060692879, "glass_bfase70_hue_unclamped");
    check::eq_i32(bits(kg.hsb[1]), 1035009239, "glass_bfase70_sat_unclamped");
    check::eq_i32(bits(kg.hsb[2]), 1064563700, "glass_bfase70_bri_unclamped");
}

/// Plane.java:201-223 (deltaf: all six ordered 3D pairs / 100, / 3) and
/// Plane.java:47-71 (loadprojf: x/z only; oy is never read).
fn deltaf_projf(k: &Tml) {
    let mut p1 = mk(
        &Medium::new(),
        &[0, 300, 0],
        &[0, 0, 0],
        &[0, 0, 400],
        3,
        [1, 2, 3],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    check::eq_i32(
        bits(p1.deltaf),
        bits(k.floatv("deltaf_3_4_5")),
        "deltaf_3_4_5_1200",
    );
    check::eq_i32(bits(p1.projf), bits(1.0), "deltaf_ctor_leaves_projf_1");
    // x/z 0-300-0 vs 0-0-0 has a 0-length 0<->2 pair -> the product is 0
    p1.loadprojf();
    check::eq_i32(bits(p1.projf), bits(0.0), "loadprojf_zero_xz_pair_is_0");

    // moving the 400 onto z makes every x/z pair non-zero: 3-4-5 again
    let mut p2 = mk(
        &Medium::new(),
        &[0, 300, 0],
        &[0, 0, 400],
        &[0, 0, 0],
        3,
        [1, 2, 3],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    check::eq_i32(
        bits(p2.deltaf),
        bits(k.floatv("deltaf_3_4_5")),
        "deltaf_xyz_3_4_5_1200",
    );
    p2.loadprojf();
    check::eq_i32(
        bits(p2.projf),
        bits(k.floatv("projf_3_4_5")),
        "loadprojf_3_4_5_1200",
    );
    // loadprojf resets projf to 1.0 first, so it is idempotent
    p2.loadprojf();
    check::eq_i32(
        bits(p2.projf),
        bits(k.floatv("projf_3_4_5")),
        "loadprojf_idempotent",
    );

    let mut p3 = mk(
        &Medium::new(),
        &[5, 5, 5],
        &[5, 5, 5],
        &[5, 5, 5],
        3,
        [1, 2, 3],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    check::eq_i32(
        bits(p3.deltaf),
        bits(k.floatv("projf_zero")),
        "deltaf_identical_vertices_0",
    );
    p3.loadprojf();
    check::eq_i32(
        bits(p3.projf),
        bits(k.floatv("projf_zero")),
        "loadprojf_identical_vertices_0",
    );

    // IEEE-exact product for the canonical sample
    check::eq_i32(
        bits(sample(&Medium::new()).deltaf),
        k.longv("deltaf_sample_bits") as i32,
        "deltaf_sample_bits_6_6666684e7",
    );
}

/// Plane.java:73-79,1027-1033,1174-1176,1016-1025 — the pure helpers.
fn helpers() {
    let m = Medium::new();
    let p = sample(&m);
    check::eq_i32(p.ys(&m, 0, 0), -1400, "ys_z0_clamps_to_cz50_minus1400");
    check::eq_i32(p.ys(&m, 0, 10), -1400, "ys_z10_clamps_to_cz50_too");
    // Medium.ys clamps z to 10 instead of m.cz (:92-96): the contrast pin
    check::eq_i32(m.ys(0, 10), -7800, "medium_ys_z10_clamps_to_10_minus7800");
    check::eq_i32(p.ys(&m, 100, 50), -600, "ys_100_50_minus600");
    check::eq_i32(p.ys(&m, 200, 400), 200, "ys_at_focus_plane_identity");

    check::eq_i32(p.xs(&m, 0, 50), -1925, "xs_0_50_minus1925");
    check::eq_i32(p.xs(&m, 275, 400), 275, "xs_center_at_focus_275");
    check::eq_i32(
        p.xs(&m, 1000, 400),
        1000,
        "xs_zero_perspective_at_focus_point",
    );
    check::eq_i32(p.xs(&m, 0, 0), -1925, "xs_z0_clamps_to_cz50");

    check::eq_i32(p.spy(&m, 275, 0), 0, "spy_center_0");
    check::eq_i32(p.spy(&m, 272, 4), 5, "spy_int_sqrt_25");
    check::eq_i32(p.spy(&m, 275, 10), 10, "spy_vertical_10");

    let mut rx = [10];
    let mut ry = [20];
    rot(&m, &mut rx, &mut ry, 10, 10, 0);
    check::eq_i32(rx[0], 10, "rot_angle0_x_untouched");
    check::eq_i32(ry[0], 20, "rot_angle0_y_untouched");
    rx[0] = 10;
    ry[0] = 20;
    rot(&m, &mut rx, &mut ry, 10, 10, 90);
    check::eq_i32(rx[0], 0, "rot_90_x_truncates_to_0");
    check::eq_i32(ry[0], 10, "rot_90_y_truncates_to_10");
}

/// Plane.java:100-130 — construction failures. A null Medium dies on the
/// first palette read (:140 solid / :155 glass) but cannot be expressed in
/// the port; a null Trackers is fine for the ctor and for d().
fn ctor_errors() {
    // Null Medium: `&Medium` cannot be null. Java's NPE sites are the
    // palette reads at Plane.java:140 (solid) and :155 (glass); the port's
    // type system makes the read unreachable, so the pin is structural.
    check::ok(true, "err_null_medium_solid_npe");
    check::ok(true, "err_null_medium_glass_npe");
    // n == -1: the port's n is usize (Java's NegativeArraySizeException on
    // `new int[n]` is unrepresentable); structural.
    check::ok(true, "err_n_minus1_nase");

    // n == 2 reaches ox[2] at Plane.java:111 (the port's ctor no longer
    // asserts n >= 3, matching the original's unchecked read).
    check::throws_it(
        AssertUnwindSafe(|| {
            let m = Medium::new();
            let _ = mk(
                &m,
                &[0, 10, 0],
                &[0, 0, 10],
                &[0, 0, 0],
                2,
                [1, 2, 3],
                false,
                0,
                0,
                0,
                0,
                7,
                0,
                false,
            );
        }),
        "index out of bounds",
        "err_n2_ox2_aioobe",
    );
    // a coordinate array shorter than n fails in the ctor's copy loop
    // (Plane.java:105-109) — the port slices `ox[..n]`.
    check::throws_it(
        AssertUnwindSafe(|| {
            let m = Medium::new();
            let _ = mk(
                &m,
                &[0, 10, 0],
                &[0, 0, 10],
                &[0, 0, 0],
                4,
                [1, 2, 3],
                false,
                0,
                0,
                0,
                0,
                7,
                0,
                false,
            );
        }),
        "out of range",
        "err_short_x_array_aioobe",
    );
    // a colour array shorter than three: fixed `[i32; 3]` in the port.
    check::ok(true, "err_short_color_aioobe");
    // rot(..., cnt=6) on 3-slot arrays: the port's rot has no count and
    // walks the slice length (Plane.java:1018's loop bound).
    check::ok(true, "err_rot_cnt6_on_len3_aioobe");

    // a null Trackers is not touched by the ctor or d()
    let mut m = Medium::new();
    let mut nt = sample(&m);
    nt.d(&mut painted(), &mut m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(nt.av, 400, "null_trackers_d_still_projects");
    // `nt.t == null`: the port stores no tracker field (s() takes one).
    check::ok(true, "null_trackers_stays_null");
}

// ------------------------------------------------------------------ d()

/// Plane.java:226-235,627-653,660-842,847-919 — the canonical on-screen
/// plane, all zero angles/offsets, embos/chip/flx zero.
fn d_basics() {
    let mut img = painted();
    let mut m = Medium::new();
    let mut p = sample(&m);
    p.d(&mut img, &mut m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p.av, 400, "d_av_400");
    check::eq_i32(bits(p.projf), bits(1.0), "d_all_zero_angles_leave_projf");
    check::eq_i32(p.embos, 0, "d_embos_stays_0");
    check::eq_i32(p.chip, 0, "d_chip_stays_0");
    check::eq_i32(pixel(&img, 277, 202), 0xC8C8C8, "d_fill_200_200_200");
    check::eq_i32(
        pixel(&img, 276, 201),
        0xC8C8C8,
        "d_fill_second_interior_point",
    );
    check::eq_i32(
        pixel(&img, 274, 200),
        0x010203,
        "d_fill_does_not_reach_outside",
    );
    check::eq_i32(pixel(&img, 275, 200), 0x000000, "d_outline_black_at_vertex");
    check::eq_i32(pixel(&img, 275, 205), 0x000000, "d_outline_black_on_edge");

    // all local copies: the argument/field arrays are untouched
    let ax = [275, 285, 275];
    let az = [400, 400, 400];
    let ay = [200, 200, 210];
    let mut moved = mk(
        &Medium::new(),
        &ax,
        &az,
        &ay,
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut moved_m = Medium::new();
    moved.d(
        &mut painted(),
        &mut moved_m,
        10,
        20,
        30,
        10,
        20,
        30,
        10,
        20,
        false,
    );
    check::eq_str(
        format!("{:?}", ax),
        "[275, 285, 275]",
        "d_input_x_array_untouched",
    );
    check::eq_str(
        format!("{:?}", ay),
        "[200, 200, 210]",
        "d_input_y_array_untouched",
    );
    check::eq_str(
        format!("{:?}", az),
        "[400, 400, 400]",
        "d_input_z_array_untouched",
    );
    check::eq_str(
        format!("{:?}", moved.ox),
        "[275, 285, 275]",
        "d_field_ox_untouched",
    );

    // noline=true: var88 is clamped to 1.0 then x0.89 -> 200*0.89 == 178
    let mut ni = painted();
    let mut nm = Medium::new();
    let mut np = sample(&nm);
    np.d(&mut ni, &mut nm, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(np.av, 400, "d_noline_av_400");
    check::eq_i32(
        pixel(&ni, 277, 202),
        0xB2B2B2,
        "d_noline_brightness_0_89_178",
    );
    check::eq_i32(pixel(&ni, 275, 200), 0xB2B2B2, "d_noline_no_black_outline");

    // gr=-9 forces var88=0.7F only inside the noline arm (:869-871)
    let mut gi = painted();
    let mut gm = Medium::new();
    let mut gp = sample(&gm);
    gp.gr = -9;
    gp.d(&mut gi, &mut gm, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(pixel(&gi, 277, 202), 0x8C8C8C, "d_gr_minus9_noline_0_7_140");
    // with noline=false the gr branch is not reached at all
    let mut g2i = painted();
    let mut g2m = Medium::new();
    let mut g2p = sample(&g2m);
    g2p.gr = -9;
    g2p.d(&mut g2i, &mut g2m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&g2i, 277, 202),
        0xC8C8C8,
        "d_gr_minus9_line_has_no_effect",
    );

    // av>3000 + adv<=900 flips noline on internally (:838-840)
    let mut am = Medium::new();
    faded(&mut am, 30000);
    let mut ai = painted();
    let mut ap = mk(
        &am,
        &[250, 350, 250],
        &[4000, 4000, 4000],
        &[180, 180, 220],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    ap.d(&mut ai, &mut am, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(ap.av, 4000, "d_av_4000_depth");
    check::eq_i32(
        pixel(&ai, 274, 200),
        0xB2B2B2,
        "d_far_adv900_forces_noline_178",
    );
    am.adv = 901;
    let mut ai2 = painted();
    let mut ap2 = mk(
        &am,
        &[250, 350, 250],
        &[4000, 4000, 4000],
        &[180, 180, 220],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    ap2.d(&mut ai2, &mut am, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&ai2, 274, 200),
        0xC8C8C8,
        "d_far_adv901_keeps_full_brightness",
    );

    // off-screen: every vertex has z<10 -> cull at :712-746 before the draw
    let mut off = mk(
        &Medium::new(),
        &[5000, 5010, 5000],
        &[0, 0, 0],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut oi = painted();
    let mut om = Medium::new();
    off.d(&mut oi, &mut om, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(off.av, 0, "d_offscreen_fresh_av_stays_0");
    check::eq_i32(bits(off.projf), bits(1.0), "d_offscreen_projf_untouched");
    check::ok(oi.px == blank_pixels(), "d_offscreen_draws_nothing");
    off.av = 77;
    off.d(&mut painted(), &mut om, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(off.av, 77, "d_offscreen_av_not_written");
    check::eq_i32(bits(off.projf), bits(1.0), "d_offscreen_null_graphics_safe");

    // a projected y<45 zeroes m.flex in the cull loop (:731-733)
    let mut fm = Medium::new();
    fm.flex = 5;
    let mut fp = sample(&fm);
    fp.oy[0] = -200;
    fp.d(&mut painted(), &mut fm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(fm.flex, 0, "d_low_vertex_resets_flex");
    // control: the same plane on-screen leaves flex alone
    let mut fm2 = Medium::new();
    fm2.flex = 5;
    let mut fp2 = sample(&fm2);
    fp2.d(&mut painted(), &mut fm2, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(fm2.flex, 5, "d_onscreen_leaves_flex_5");

    // embos==chip==flx==0 consumes no randomness at all
    let mut zm = Preset::new(6);
    let mut zp = sample(&zm.m);
    zp.d(&mut painted(), &mut zm.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(zm.draws(), 0, "d_zero_random_draws");
    check::eq_i32(zm.m.cntrn, 20, "d_zero_draws_cntrn_untouched");
    check::eq_i32(zm.m.trn as i32, 2, "d_zero_draws_trn_untouched");
    check::eq_str(
        format!("{:?}", zm.m.rand),
        "[6, 6, 6]",
        "d_zero_draws_rand_untouched",
    );

    // gr>0 with av>3500 culls (:834-836) — same 4000-deep triangle
    let mut gm2 = Medium::new();
    faded(&mut gm2, 30000);
    let mut gm_img = painted();
    let mut gmp = mk(
        &gm2,
        &[250, 350, 250],
        &[4000, 4000, 4000],
        &[180, 180, 220],
        3,
        [200, 200, 200],
        false,
        1,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    gmp.d(&mut gm_img, &mut gm2, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(gmp.av, 4000, "d_gr1_far_av_4000");
    check::ok(
        gm_img.px == blank_pixels(),
        "d_gr1_far_3500_cull_nothing_drawn",
    );
}

/// Plane.java:842,916-975 — the flx outline animation. flx!=0 adds one
/// random draw (the >0.3 wake check), so every case presets the stream.
fn d_flx() {
    let mut hi = Preset::new(6); // first draw 0.5 > 0.3
    let mut wake = sample(&hi.m);
    wake.flx = 1;
    let mut wi = painted();
    wake.d(&mut wi, &mut hi.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(wake.av, 400, "d_flx1_av_computed_before_wake_check");
    check::eq_i32(wake.flx, 1, "d_flx1_high_random_culls_without_advancing");
    check::eq_i32(hi.draws(), 1, "d_flx1_high_random_one_draw");
    check::eq_i32(
        pixel(&wi, 277, 202),
        0x010203,
        "d_flx1_high_random_draws_nothing",
    );

    let mut lo = Preset::new(3); // first draw 0.2 <= 0.3
    let mut lit = sample(&lo.m);
    lit.flx = 1;
    let mut li = painted();
    lit.d(&mut li, &mut lo.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(lo.draws(), 1, "d_flx1_low_random_one_draw");
    check::eq_i32(lit.flx, 2, "d_flx1_low_random_advances_to_2");
    check::eq_i32(pixel(&li, 277, 202), 0xC8C8C8, "d_flx1_fill_still_drawn");
    check::eq_i32(
        pixel(&li, 275, 200),
        0x00DFFF,
        "d_flx1_outline_snap_colour_0_223_255",
    );

    let mut l3 = Preset::new(3);
    let mut lit3 = sample(&l3.m);
    lit3.flx = 3;
    let mut l3i = painted();
    lit3.d(&mut l3i, &mut l3.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(lit3.flx, 2, "d_flx3_advances_to_2");
    check::eq_i32(
        pixel(&l3i, 275, 200),
        0x00FFDF,
        "d_flx3_outline_snap_colour_0_255_223",
    );

    let mut l2 = Preset::new(3);
    let mut lit2 = sample(&l2.m);
    lit2.flx = 2;
    let mut l2i = painted();
    lit2.d(&mut l2i, &mut l2.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(lit2.flx, 2, "d_flx2_stays_2");
    check::eq_i32(pixel(&l2i, 275, 200), 0x000000, "d_flx2_outline_black");
    check::eq_i32(l2.m.cntrn, 19, "d_flx2_one_draw");
}

/// Plane.java:627-653 — the projf recompute runs only when one of the xz/
/// xy/zy ANGLE arguments is non-zero, and always over exactly three
/// vertices (:634-650), even when n==4.
fn d_projf(k: &Tml) {
    let mut p0 = sample(&Medium::new());
    p0.projf = 7.5;
    let mut m0 = Medium::new();
    p0.d(&mut painted(), &mut m0, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(bits(p0.projf), bits(7.5), "dprojf_zero_angles_untouched");

    let mut px = sample(&Medium::new());
    px.projf = 7.5;
    let mut mx = Medium::new();
    px.d(&mut painted(), &mut mx, 0, 0, 0, 90, 0, 0, 0, 0, false);
    check::eq_i32(bits(px.projf), bits(0.0), "dprojf_xz90_overwrites_0");

    let mut py = sample(&Medium::new());
    py.projf = 7.5;
    let mut my = Medium::new();
    py.d(&mut painted(), &mut my, 0, 0, 0, 0, 90, 0, 0, 0, false);
    check::eq_i32(bits(py.projf), bits(0.0), "dprojf_xy90_overwrites_0");

    // zy only rotates y/z: the sample's distances collapse onto the same
    // 3-4-5 multiset, so the value equals deltaf bit-for-bit
    let mut pz = sample(&Medium::new());
    pz.projf = 7.5;
    let mut mz = Medium::new();
    pz.d(&mut painted(), &mut mz, 0, 0, 0, 0, 0, 90, 0, 0, false);
    check::eq_i32(
        bits(pz.projf),
        k.longv("deltaf_sample_bits") as i32,
        "dprojf_zy90_overwrites_to_deltaf_value",
    );

    // n=4: only the first three vertices feed the recompute
    let mut a4 = mk(
        &Medium::new(),
        &[275, 285, 280, 275],
        &[400, 400, 400, 999],
        &[200, 200, 210, 250],
        4,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut a4m = Medium::new();
    a4.d(&mut painted(), &mut a4m, 0, 0, 0, 90, 0, 0, 0, 0, false);
    let mut b4 = mk(
        &Medium::new(),
        &[275, 285, 280, 275],
        &[400, 400, 400, 0],
        &[200, 200, 210, 250],
        4,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut b4m = Medium::new();
    b4.d(&mut painted(), &mut b4m, 0, 0, 0, 90, 0, 0, 0, 0, false);
    check::eq_i32(
        bits(a4.projf),
        bits(b4.projf),
        "dprojf_n4_ignores_fourth_vertex",
    );
    check::eq_i32(bits(a4.projf), 850588925, "dprojf_n4_first_three_value");
    check::eq_i32(a4.av + b4.av, 0, "dprojf_n4_both_culled_av_0");

    // coincident first three: the (ignored) 4th would make the product
    // non-zero, but the hardcoded 3 vertices yield 0.0 exactly
    let mut c4 = mk(
        &Medium::new(),
        &[275, 275, 275, 300],
        &[400, 400, 400, 400],
        &[200, 200, 200, 200],
        4,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut c4m = Medium::new();
    c4.d(&mut painted(), &mut c4m, 0, 0, 0, 90, 0, 0, 0, 0, false);
    check::eq_i32(
        bits(c4.projf),
        bits(0.0),
        "dprojf_n4_coincident_first3_zero",
    );

    // wx==0 skips the wxz rotation (:509-511) but wz!=0 still rotates y/z
    let mut base = painted();
    let mut bm = Medium::new();
    sample(&bm).d(&mut base, &mut bm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    let mut wide = painted();
    let mut wm = Medium::new();
    sample(&wm).d(&mut wide, &mut wm, 0, 0, 0, 0, 0, 0, 90, 0, false);
    check::ok(base.px == wide.px, "d_wx0_ignores_wxzarg");

    let mut wxp = sample(&Medium::new());
    wxp.wx = 100;
    let mut wx_img = painted();
    let mut wx_m = Medium::new();
    wxp.d(&mut wx_img, &mut wx_m, 0, 0, 0, 0, 0, 0, 90, 0, false);
    check::ok(base.px != wx_img.px, "d_wx100_applies_wxzarg");
    check::eq_i32(wxp.av, 0, "d_wx100_off_screen_av_0");

    let mut wzp = sample(&Medium::new());
    wzp.wz = 100;
    let mut wz_img = painted();
    let mut wz_m = Medium::new();
    wzp.d(&mut wz_img, &mut wz_m, 0, 0, 0, 0, 0, 0, 0, 90, false);
    check::ok(base.px != wz_img.px, "d_wz100_applies_wzyarg");
}

/// Plane.java:891-897 — with m.trk the fill comes from oc alone; and
/// :744-746 additionally culls the whole plane when ANY vertex leaves the
/// viewport.
fn d_trk(k: &Tml) {
    let mut m1 = Medium::new();
    m1.trk = true;
    let mut m2 = Medium::new();
    m2.trk = true;
    m2.snap = [100, 100, 100];
    let p1 = sample(&m1);
    let p2 = sample(&m2);
    check::eq_str(
        format!("{:?}", p1.oc),
        format!("{:?}", p2.oc),
        "dtrk_same_oc",
    );
    check::ok(p1.c != p2.c, "dtrk_different_c");
    let mut i1 = painted();
    let mut i2 = painted();
    let mut mm1 = m1;
    let mut mm2 = m2;
    let mut p1 = p1;
    let mut p2 = p2;
    p1.d(&mut i1, &mut mm1, 0, 0, 0, 0, 0, 0, 0, 0, false);
    p2.d(&mut i2, &mut mm2, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::ok(i1.px == i2.px, "dtrk_identical_pixels_from_oc_only");
    check::eq_i32(
        pixel(&i1, 277, 202),
        0xE2920B,
        "dtrk_fill_hsb_0_105_0_95_from_oc",
    );
    check::eq_i32(p1.av, 400, "dtrk_av_400");

    // one off-screen vertex: the plain plane still draws, the trk plane is
    // rejected outright
    let mut plain_m = Medium::new();
    let mut plain = mk(
        &plain_m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 1000],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut pi = painted();
    plain.d(&mut pi, &mut plain_m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::ok(pi.px != blank_pixels(), "dtrk_control_plain_draws");
    check::eq_i32(plain.av, k.intv("av_probe"), "dtrk_control_plain_av_565");

    let mut trk_m = Medium::new();
    trk_m.trk = true;
    let mut trk = mk(
        &trk_m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 1000],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut ti = painted();
    trk.d(&mut ti, &mut trk_m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::ok(ti.px == blank_pixels(), "dtrk_one_offscreen_vertex_culls");
    check::eq_i32(trk.av, 0, "dtrk_one_offscreen_av_0");
}

/// Plane.java:976-1012 — the two extra outline passes: road (fill-10) and
/// the gr==-10 c-fade outline. Both live in mutually exclusive branches.
fn d_outline() {
    let mut rm = Medium::new();
    rm.fade[0] = 5000;
    let mut road = mk(
        &rm,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        true,
    );
    let mut ri = painted();
    road.d(&mut ri, &mut rm, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(road.av, 400, "droad_av_400");
    check::eq_i32(pixel(&ri, 277, 202), 0xB2B2B2, "droad_interior_178");
    check::eq_i32(
        pixel(&ri, 275, 200),
        0xA8A8A8,
        "droad_outline_fill_minus_10_168",
    );

    // fade[0] must exceed 4000 strictly
    let mut rm2 = Medium::new();
    rm2.fade[0] = 4000;
    let mut road2 = mk(
        &rm2,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        true,
    );
    let mut ri2 = painted();
    road2.d(&mut ri2, &mut rm2, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(
        pixel(&ri2, 275, 200),
        0xB2B2B2,
        "droad_fade4000_no_extra_outline",
    );

    // noline=false never reaches the road arm (it is the else-if of !var10)
    let mut road3 = mk(
        &rm,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        true,
    );
    let mut ri3 = painted();
    road3.d(&mut ri3, &mut rm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&ri3, 277, 202),
        0xC8C8C8,
        "droad_noline_false_interior_200",
    );
    check::eq_i32(
        pixel(&ri3, 275, 200),
        0x000000,
        "droad_noline_false_black_outline",
    );

    // gr==-10, !glass, !trk: a second outline in c (after the fade blend)
    let mut g10m = Medium::new();
    let mut g10 = mk(
        &g10m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        -10,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut g10i = painted();
    g10.d(&mut g10i, &mut g10m, 0, 0, 0, 0, 0, 0, 0, 0, true);
    // the |gr^3| term inside av (Plane.java:822-829) is what makes this 401
    check::eq_i32(g10.av, 401, "dgr10_av_includes_abs_gr_cubed");
    check::eq_i32(
        pixel(&g10i, 277, 202),
        0xB2B2B2,
        "dgr10_interior_noline_178",
    );
    check::eq_i32(
        pixel(&g10i, 275, 200),
        0xC8C8C8,
        "dgr10_extra_outline_c_colour_200",
    );

    let mut ggm = Medium::new();
    let mut g10glass = mk(
        &ggm,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [1, 2, 3],
        true,
        -10,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut gg = painted();
    g10glass.d(&mut gg, &mut ggm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&gg, 275, 200),
        0x000000,
        "dgr10_glass_skips_extra_outline",
    );
    check::eq_i32(pixel(&gg, 277, 202), 0xE5DEF3, "dgr10_glass_fill_sky_blend");

    let mut tm = Medium::new();
    tm.trk = true;
    let mut g10trk = mk(
        &tm,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        -10,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut gt = painted();
    g10trk.d(&mut gt, &mut tm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&gt, 275, 200),
        0x000000,
        "dgr10_trk_skips_extra_outline",
    );
    check::eq_i32(pixel(&gt, 277, 202), 0xE2920B, "dgr10_trk_fill_oc_derived");
}

/// Plane.java:830 — fade is int[8], and the av-vs-fade test reads
/// fade[disline]. m.trk short-circuits the whole condition.
fn d_errors() {
    for dl in [8, -1] {
        let id = if dl < 0 {
            "derr_disline_minus1_aioobe".to_string()
        } else {
            format!("derr_disline{dl}_aioobe")
        };
        check::throws_it(
            AssertUnwindSafe(|| {
                let mut m = Medium::new();
                let mut p = sample(&m);
                p.disline = dl;
                p.d(&mut painted(), &mut m, 0, 0, 0, 0, 0, 0, 0, 0, false);
            }),
            "index out of bounds",
            &id,
        );
    }

    let mut tm = Medium::new();
    tm.trk = true;
    let mut p = sample(&tm);
    p.disline = 8;
    let mut img = painted();
    p.d(&mut img, &mut tm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p.av, 400, "derr_trk_short_circuits_fade_index");
    check::eq_i32(pixel(&img, 277, 202), 0xE2920B, "derr_trk_still_draws");
}

// ------------------------------------------------------------------ s()

/// Plane.java:1035-1172 — s() is pure with respect to game state; gr>0
/// returns immediately, and the no-tracker ground path assigns m.ground
/// verbatim (m.y is not subtracted). Fill is cgrnd/1.5.
fn s_machines() {
    let m = Medium::new();
    let t = matched_tracker(150, 150, 150, 150, 50);
    let p = shadow(&m, &t);
    let hsb0_before = bits(p.hsb[0]);
    let plane_before = snapshot(&p);
    let tracks_before = tracker_snapshot(&t);

    let mut img = painted();
    p.s(&mut img, &m, &t, 0, 0, 0, 0, 0, 0, 0);
    check::eq_str(snapshot(&p), plane_before, "s_leaves_plane_state_identical");
    check::eq_str(
        tracker_snapshot(&t),
        tracks_before,
        "s_leaves_trackers_state_identical",
    );
    check::eq_i32(p.av, 0, "s_never_writes_av");
    check::eq_i32(bits(p.hsb[0]), hsb0_before, "s_never_writes_hsb");
    check::eq_i32(
        pixel(&img, 277, 151),
        0x646464,
        "s_tracker_fill_t_c_over_1_5_100",
    );

    // no random consumer anywhere in s()
    let pm = Preset::new(6);
    let pms = shadow(&pm.m, &Trackers::new());
    pms.s(&mut painted(), &pm.m, &Trackers::new(), 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(pm.draws(), 0, "s_zero_random_draws");

    // gr>0 -> immediate return (:1036), even with a null Graphics
    let mut no_ground = shadow(&Medium::new(), &Trackers::new());
    let mut ngi = painted();
    no_ground.gr = 1;
    no_ground.s(
        &mut ngi,
        &Medium::new(),
        &Trackers::new(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    );
    check::ok(ngi.px == blank_pixels(), "s_gr1_paints_nothing");
    no_ground.s(
        &mut painted(),
        &Medium::new(),
        &Trackers::new(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    );
    check::eq_i32(no_ground.gr, 1, "s_gr1_returns_before_reading_graphics");

    // default ground shadow: cgrnd {205,200,200} / 1.5 -> {136,133,133}
    let ground = shadow(&Medium::new(), &Trackers::new());
    let mut gi = painted();
    ground.s(
        &mut gi,
        &Medium::new(),
        &Trackers::new(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    );
    check::eq_i32(pixel(&gi, 277, 248), 0x888585, "s_ground_cgrnd_over_1_5");

    // the no-tracker arm writes m.ground verbatim: m.y cannot change a pixel
    let mut my = Medium::new();
    my.y = 100;
    let with_y = shadow(&my, &Trackers::new());
    let mut yi = painted();
    with_y.s(&mut yi, &my, &Trackers::new(), 0, 0, 0, 0, 0, 0, 0);
    check::ok(gi.px == yi.px, "s_m_y_100_byte_identical");
    check::eq_i32(my.ground, 250, "s_m_y_does_not_touch_ground");

    // tracker offset arithmetic uses t.x-m.x / t.z-m.z
    let mut off = Medium::new();
    off.x = 100;
    off.z = 50;
    let mut toff = Trackers::new();
    toff.nt = 1;
    toff.radx[0] = 50;
    toff.radz[0] = 50;
    toff.x[0] = 375;
    toff.z[0] = 450;
    toff.y[0] = 150;
    toff.c[0] = [150, 150, 150];
    let rel = shadow(&off, &toff);
    let mut reli = painted();
    rel.s(&mut reli, &off, &toff, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(
        pixel(&reli, 277, 151),
        0x646464,
        "s_tracker_uses_relative_offsets",
    );
}

/// Plane.java:1054-1101 — the tracker lock: both zy/xy must avoid ±90,
/// every vertex must sit inside radx/radz, more than n/2 must match, and
/// the scan runs nt-1..0 so the highest index wins.
fn s_trackers() {
    // radx defaults to 0 -> |...| < 0 is never true
    let mut zero = Trackers::new();
    zero.nt = 1;
    zero.x[0] = 275;
    zero.z[0] = 400;
    zero.y[0] = 150;
    zero.c[0] = [150, 150, 150];
    let mut zi = painted();
    let m = Medium::new();
    shadow(&m, &zero).s(&mut zi, &m, &zero, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(
        pixel(&zi, 277, 248),
        0x888585,
        "s_radx0_rejects_uses_ground",
    );

    // |zy|==90 or |xy|==90 skips the tracker entirely
    let mut zy = matched_tracker(150, 150, 150, 150, 50);
    zy.zy[0] = 90;
    let mut zyi = painted();
    let m2 = Medium::new();
    shadow(&m2, &zy).s(&mut zyi, &m2, &zy, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(pixel(&zyi, 277, 248), 0x888585, "s_zy90_skips_tracker");
    check::eq_i32(
        pixel(&zyi, 277, 151),
        0x010203,
        "s_zy90_leaves_ground_height",
    );

    let mut xy = matched_tracker(150, 150, 150, 150, 50);
    xy.xy[0] = 90;
    let mut xyi = painted();
    let m3 = Medium::new();
    shadow(&m3, &xy).s(&mut xyi, &m3, &xy, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(pixel(&xyi, 277, 248), 0x888585, "s_xy90_skips_tracker");

    // two matching trackers: index 1 (scanned first) wins
    let mut two = Trackers::new();
    two.nt = 2;
    for i in 0..2 {
        two.radx[i] = 50;
        two.radz[i] = 50;
        two.x[i] = 275;
        two.z[i] = 400;
    }
    two.y[0] = 100;
    two.c[0] = [150, 150, 150];
    two.y[1] = 150;
    two.c[1] = [30, 60, 90];
    let mut twi = painted();
    let m4 = Medium::new();
    shadow(&m4, &two).s(&mut twi, &m4, &two, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(
        pixel(&twi, 277, 151),
        0x14283C,
        "s_two_trackers_highest_index_wins",
    );
    check::eq_i32(
        pixel(&twi, 277, 101),
        0x010203,
        "s_two_trackers_low_index_not_used",
    );

    // t.y-m.y is the tracker-side ground offset (:1073)
    let mut my = Medium::new();
    my.y = 40;
    let ty = matched_tracker(190, 150, 150, 150, 50);
    let mut tyi = painted();
    shadow(&my, &ty).s(&mut tyi, &my, &ty, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(pixel(&tyi, 277, 151), 0x646464, "s_tracker_y_minus_m_y_150");
}

/// Plane.java:1058,1110-1122 — mode gates. mode!=0 skips the tracker lock;
/// mode==2 forces (76,98,142) and escapes the spark-occlusion scan; every
/// other mode keeps cgrnd but still hides the shadow under a spark.
fn s_modes() {
    let t = matched_tracker(150, 150, 150, 150, 50);

    let mut m1i = painted();
    let m1 = Medium::new();
    shadow(&m1, &t).s(&mut m1i, &m1, &t, 0, 0, 0, 0, 0, 0, 1);
    check::eq_i32(
        pixel(&m1i, 277, 248),
        0x888585,
        "s_mode1_skips_tracker_lock",
    );
    check::eq_i32(
        pixel(&m1i, 277, 151),
        0x010203,
        "s_mode1_keeps_ground_height",
    );

    let mut m2i = painted();
    let m2 = Medium::new();
    shadow(&m2, &t).s(&mut m2i, &m2, &t, 0, 0, 0, 0, 0, 0, 2);
    check::eq_i32(pixel(&m2i, 277, 248), 0x4C628E, "s_mode2_forces_76_98_142");

    let mut spm = Medium::new();
    spm.nsp = 1;
    spm.spx[0] = 275;
    spm.spz[0] = 400;
    spm.sprad[0] = 50;
    let mut sp1 = painted();
    shadow(&spm, &Trackers::new()).s(&mut sp1, &spm, &Trackers::new(), 0, 0, 0, 0, 0, 0, 1);
    check::ok(sp1.px == blank_pixels(), "s_mode1_spark_occludes");
    let mut sp0 = painted();
    shadow(&spm, &Trackers::new()).s(&mut sp0, &spm, &Trackers::new(), 0, 0, 0, 0, 0, 0, 0);
    check::ok(sp0.px == blank_pixels(), "s_mode0_spark_occludes_too");
    let mut sp2 = painted();
    shadow(&spm, &Trackers::new()).s(&mut sp2, &spm, &Trackers::new(), 0, 0, 0, 0, 0, 0, 2);
    check::eq_i32(pixel(&sp2, 277, 248), 0x4C628E, "s_mode2_spark_still_drawn");

    // av beyond every fade entry: eight successive 3/4 blends toward cfade
    let mut far = shadow(&Medium::new(), &Trackers::new());
    far.av = 30000;
    let mut fi = painted();
    far.s(
        &mut fi,
        &Medium::new(),
        &Trackers::new(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    );
    check::eq_i32(
        pixel(&fi, 277, 248),
        0xF1D2D2,
        "s_av30000_blends_to_241_210_210",
    );

    // av==0 (fresh) does no blending at all
    let near = shadow(&Medium::new(), &Trackers::new());
    let mut ni = painted();
    near.s(
        &mut ni,
        &Medium::new(),
        &Trackers::new(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    );
    check::eq_i32(pixel(&ni, 277, 248), 0x888585, "s_av0_no_blend");
}

/// Plane.java:1059,1063,1132-1157 — the culling half of s().
fn s_culling() {
    let m = Medium::new();
    let t = Trackers::new();
    let off = mk(
        &m,
        &[5000, 5010, 5000],
        &[0, 0, 0],
        &[200, 200, 210],
        3,
        [10, 20, 30],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut oi = painted();
    off.s(&mut oi, &m, &t, 0, 0, 0, 0, 0, 0, 0);
    check::ok(oi.px == blank_pixels(), "s_offscreen_draws_nothing");
    off.s(&mut painted(), &m, &t, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(off.av, 0, "s_offscreen_null_graphics_safe");

    // Java's `t == null` NPE at :1059 is unrepresentable (`&Trackers`).
    check::ok(true, "s_null_trackers_npe");

    check::throws_it(
        AssertUnwindSafe(|| {
            let mut big = Trackers::new();
            big.nt = 501;
            let mm = Medium::new();
            shadow(&mm, &big).s(&mut painted(), &mm, &big, 0, 0, 0, 0, 0, 0, 0);
        }),
        "index out of bounds",
        "s_nt501_aioobe",
    );
}

// --------------------------------------------------------- embos / chip

/// Plane.java:261-289,478-496 — the embos brightness multipliers and the
/// per-typ axis scaling. The scaled axis is typ: x for 1, y for 2, z for 3.
fn embos_ranges() {
    let mut e5 = Preset::new(6);
    let mut p5 = sample(&e5.m);
    p5.embos = 5;
    p5.d(&mut painted(), &mut e5.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p5.embos, 6, "embos5_advances_to_6");
    check::eq_i32(e5.draws(), 2, "embos5_two_draws");
    check::eq_i32(p5.av, 440, "embos5_typ3_z_scaled_av_440");
    check::eq_i32(bits(p5.hsb[2]), 1061734601, "embos5_bri_uncapped");

    let mut e8 = Preset::new(6);
    let mut p8 = sample(&e8.m);
    p8.embos = 8;
    p8.d(&mut painted(), &mut e8.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p8.embos, 9, "embos8_advances_to_9");
    check::eq_i32(e8.draws(), 2, "embos8_two_draws");
    check::eq_i32(p8.av, 453, "embos8_av_453");
    check::eq_i32(bits(p8.hsb[2]), 1060320051, "embos8_bri_capped_0_7");

    let mut e10 = Preset::new(6);
    let mut p10 = sample(&e10.m);
    p10.embos = 10;
    p10.d(&mut painted(), &mut e10.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p10.embos, 11, "embos10_advances_to_11");
    check::eq_i32(e10.draws(), 2, "embos10_two_draws");
    check::eq_i32(p10.av, 480, "embos10_av_480");
    check::eq_i32(bits(p10.hsb[2]), 1058642330, "embos10_bri_capped_0_6");

    // typ 1 scales ox, typ 2 scales oy (the same 1.08 factor -> same av)
    let mut e1a = Preset::new(6);
    let mut t1 = mk(
        &e1a.m,
        &[275, 275, 275],
        &[400, 420, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    check::eq_i32(t1.typ, 1, "embos_typ1_fixture");
    t1.embos = 1;
    t1.d(&mut painted(), &mut e1a.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(t1.embos, 2, "embos1_typ1_advances_to_2");
    check::eq_i32(t1.av, 410, "embos1_typ1_av_410");
    check::eq_i32(e1a.draws(), 2, "embos1_typ1_two_draws");

    let mut e2a = Preset::new(6);
    let mut t2 = mk(
        &e2a.m,
        &[275, 285, 275],
        &[400, 420, 400],
        &[200, 200, 200],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    check::eq_i32(t2.typ, 2, "embos_typ2_fixture");
    t2.embos = 1;
    t2.d(&mut painted(), &mut e2a.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(t2.embos, 2, "embos1_typ2_advances_to_2");
    check::eq_i32(t2.av, 410, "embos1_typ2_av_410");
    check::eq_i32(e2a.draws(), 2, "embos1_typ2_two_draws");
}

/// B5 pins 41-48. Every d() starts from the preset recipe Medium so the
/// real Medium.random() stream is fixed.
fn embos_chip() {
    // pin 41: chip==1 with bfase==0 always resets; the nocol plane then
    // seeds bfase=1. The random is evaluated first (:513) => 1 draw.
    let mut c41 = Preset::new(6);
    let mut p41 = mk(
        &c41.m,
        &[0, 10, 0],
        &[0, 0, 10],
        &[0, 0, 0],
        3,
        [10, 10, 10],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    p41.chip = 1;
    p41.d(&mut painted(), &mut c41.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p41.chip, 0, "chip1_bfase0_resets_chip");
    check::eq_i32(p41.bfase, 1, "chip1_bfase0_nocol_seeds_bfase_1");
    check::eq_i32(c41.draws(), 1, "chip1_bfase0_one_random_draw");
    check::ok(p41.nocol, "chip1_nocol_plane");

    // pin 42: 19 -> draw -> 20 -> wraps to 0 (:621-624)
    let mut c42 = Preset::new(6);
    let mut p42 = sample(&c42.m);
    p42.chip = 19;
    p42.d(&mut painted(), &mut c42.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p42.chip, 0, "chip19_wraps_to_0");
    check::eq_i32(c42.draws(), 1, "chip19_one_colour_draw");

    // pin 43: chip>=2 is ballistic - dx/dy/dz are copied out, then advanced
    // by vx/vy/vz; vy gains 7 every call (:581-584)
    let mut c43 = Preset::new(6);
    let mut p43 = sample(&c43.m);
    p43.chip = 5;
    p43.vx = 1;
    p43.vy = 2;
    p43.vz = 3;
    p43.d(&mut painted(), &mut c43.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p43.dx, 1, "chip5_dx_copies_vx");
    check::eq_i32(p43.dy, 2, "chip5_dy_copies_vy");
    check::eq_i32(p43.dz, 3, "chip5_dz_copies_vz");
    check::eq_i32(p43.vy, 9, "chip5_vy_plus_7");
    check::eq_i32(p43.chip, 6, "chip5_advances_to_6");

    // ground hit: var55[0] > m.ground sets 19, then the ++ wraps it to 0
    let mut c43b = Preset::new(6);
    let mut p43b = sample(&c43b.m);
    p43b.chip = 5;
    p43b.vx = 1;
    p43b.vy = 2;
    p43b.vz = 3;
    c43b.m.ground = -10;
    p43b.d(&mut painted(), &mut c43b.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p43b.chip, 0, "chip5_ground_hit_19_then_wraps_0");
    check::eq_i32(p43b.vy, 9, "chip5_ground_hit_still_advances_vy");

    // pin 44: chip==1 seeds cxz/cxy/czy from the angles; ctmag clamps to
    // +-3 before the offsets (:529-535). Constant 0.5 stream.
    let mut fm = Fixed::new(0.5);
    let mut p44 = mk(
        &fm.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        5,
        false,
    );
    p44.chip = 1;
    p44.ctmag = 7.0;
    p44.d(&mut painted(), &mut fm.m, 0, 0, 0, 11, 22, 33, 0, 0, false);
    check::eq_i32(bits(p44.ctmag), bits(3.0), "chip1_ctmag_clamped_3");
    check::eq_i32(p44.cxz, 11, "chip1_cxz_from_xz_arg");
    check::eq_i32(p44.cxy, 22, "chip1_cxy_from_xy_arg");
    check::eq_i32(p44.czy, 33, "chip1_czy_from_zy_arg");
    check::eq_str(
        format!("{:?}", p44.cox),
        "[285, 285, 285]",
        "chip1_cox_seed_index",
    );
    check::eq_str(
        format!("{:?}", p44.coy),
        "[200, 200, 200]",
        "chip1_coy_seed_index",
    );
    check::eq_str(
        format!("{:?}", p44.coz),
        "[400, 400, 400]",
        "chip1_coz_seed_index",
    );
    check::eq_i32(p44.vx, 0, "chip1_vx_ctmag_times_0");
    check::eq_i32(p44.vy, 7, "chip1_vy_0_then_plus_7");
    check::eq_i32(p44.chip, 3, "chip1_advances_to_3");
    check::eq_i32(fm.draws(), 12, "chip1_twelve_draws");

    // pin 45: embos=12 -> chip/ctmag/bfase seed, then one d() runs the whole
    // primitive; with the preset recipe :513 samples 0.4 (no reset).
    let mut e12 = Preset::new(6);
    let mut p45 = sample(&e12.m);
    p45.embos = 12;
    p45.d(&mut painted(), &mut e12.m, 0, 0, 0, 11, 22, 33, 0, 0, false);
    check::eq_i32(bits(p45.ctmag), bits(2.0), "embos12_ctmag_2");
    check::eq_i32(p45.bfase, -7, "embos12_bfase_minus7");
    check::eq_i32(p45.embos, 13, "embos12_advances_to_13");
    check::eq_i32(p45.chip, 3, "embos12_preset_chip_ends_3");
    check::eq_i32(bits(p45.hsb[2]), bits(0.5), "embos12_bri_capped_0_5");
    check::eq_i32(p45.cxz, 11, "embos12_cxz_copied");
    check::eq_i32(p45.cxy, 22, "embos12_cxy_copied");
    check::eq_i32(p45.czy, 33, "embos12_czy_copied");
    check::eq_str(
        format!("{:?}", p45.cox),
        "[275, 287, 291]",
        "embos12_cox_preset",
    );
    check::eq_str(
        format!("{:?}", p45.coy),
        "[200, 220, 184]",
        "embos12_coy_preset",
    );
    check::eq_str(
        format!("{:?}", p45.coz),
        "[400, 388, 392]",
        "embos12_coz_preset",
    );
    check::eq_i32(p45.vx, -4, "embos12_vx_preset");
    check::eq_i32(p45.vy, 11, "embos12_vy_preset");
    check::eq_i32(p45.vz, 0, "embos12_vz_preset");
    check::eq_i32(e12.draws(), 13, "embos12_thirteen_draws");

    // pin 46: embos=13 rewrites the HSB, then falls through to a plain draw
    let mut e13 = Preset::new(6);
    let mut p46 = sample(&e13.m);
    p46.embos = 13;
    p46.d(&mut painted(), &mut e13.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(bits(p46.hsb[1]), bits(0.2), "embos13_sat_0_2");
    check::eq_i32(bits(p46.hsb[2]), bits(0.4), "embos13_bri_0_4");
    check::eq_i32(p46.embos, 14, "embos13_advances_to_14");
    check::eq_i32(e13.draws(), 0, "embos13_no_random_draws");

    // pin 47: 70 snaps to 16, 1 -> 2, and 0 never increments (:230,:498-503)
    let mut e70 = Preset::new(6);
    let mut p70 = sample(&e70.m);
    p70.embos = 70;
    p70.d(&mut painted(), &mut e70.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p70.embos, 16, "embos70_snaps_to_16");
    check::eq_i32(e70.draws(), 3, "embos70_spark_three_draws");
    check::eq_i32(
        p70.pa as i32 + p70.pb as i32,
        0,
        "embos70_does_not_reroll_pa_pb",
    );

    let mut e1 = Preset::new(6);
    let mut p1 = sample(&e1.m);
    p1.embos = 1;
    p1.d(&mut painted(), &mut e1.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(p1.embos, 2, "embos1_advances_to_2");
    check::eq_i32(e1.draws(), 2, "embos1_two_draws");

    let mut e0 = Preset::new(6);
    let mut p0 = sample(&e0.m);
    for _ in 0..3 {
        p0.d(&mut painted(), &mut e0.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    }
    check::eq_i32(p0.embos, 0, "embos0_stays_0_after_three_calls");
    check::eq_i32(e0.draws(), 0, "embos0_zero_draws");

    // pin 48: the grey spark polygon is guarded by !glass (:237). A fade
    // override culls the body so only the spark can paint.
    let mut hi_m = Preset::new(7);
    for i in 0..8 {
        hi_m.m.fade[i] = 100;
    }
    let mut spark = sample(&hi_m.m);
    spark.embos = 1;
    let mut si = painted();
    spark.d(&mut si, &mut hi_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(count(&si, 0xE6E6E6), 35, "embos1_grey_spark_pixels_35");
    check::eq_i32(hi_m.draws(), 11, "embos1_grey_spark_eleven_draws");

    let mut hi_g = Preset::new(7);
    for i in 0..8 {
        hi_g.m.fade[i] = 100;
    }
    let mut gspark = mk(
        &hi_g.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [1, 2, 3],
        true,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    gspark.embos = 1;
    let mut gsi = painted();
    gspark.d(&mut gsi, &mut hi_g.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(count(&gsi, 0xE6E6E6), 0, "embos1_glass_skips_grey_spark");
    check::eq_i32(hi_g.draws(), 2, "embos1_glass_skips_jitter_draws");

    // the chip block's own flex probe (:598-600) fires on cox[0]/coy[0]
    // projected below y=45 even when the body is comfortably on-screen
    let mut fx = Medium::new();
    fx.flex = 5;
    let mut cfx = sample(&fx);
    cfx.chip = 5;
    cfx.d(&mut painted(), &mut fx, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(fx.flex, 0, "chip_block_resets_flex");
    check::eq_i32(cfx.chip, 6, "chip_block_flex_case_chip_6");
    let mut fx2 = Medium::new();
    fx2.flex = 5;
    let mut cfx2 = sample(&fx2);
    cfx2.d(&mut painted(), &mut fx2, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(fx2.flex, 5, "body_only_leaves_flex_5");
}

/// The remaining deterministic corners: the embos>=16 spark ball, the c()
/// brighter/darker arm, the ctmag -3 clamp, the sub-1.0 brightness clamp
/// arms, the non-trk fade blend, the road clamp and the tilted-tracker
/// shadow.
fn extra_arms() {
    // embos==16 rolls pa/pb with a retry while they collide (:302-309);
    // the preset's 0.5,0.4 collide at index 1, 0.3 retries to 1,0
    let mut b16 = Preset::new(6);
    let mut ball = sample(&b16.m);
    ball.embos = 16;
    ball.d(&mut painted(), &mut b16.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(ball.embos, 17, "embos16_advances_to_17");
    check::eq_i32(ball.pa as i32, 1, "embos16_pa_1");
    check::eq_i32(ball.pb as i32, 0, "embos16_pb_retried_to_0");
    check::eq_i32(b16.draws(), 6, "embos16_six_draws");
    check::eq_i32(ball.av, 400, "embos16_body_av_400");

    // far-apart pa/pb vertices enter the while (|dx|>100)/(|dz|>100) walk
    let mut wide_m = Preset::new(6);
    let mut wide = mk(
        &wide_m.m,
        &[100, 400, 300],
        &[300, 600, 400],
        &[200, 200, 200],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    wide.embos = 16;
    wide.d(&mut painted(), &mut wide_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(wide.pa as i32, 1, "embos16_wide_pa_1");
    check::eq_i32(wide.pb as i32, 0, "embos16_wide_pb_0");
    check::eq_i32(wide.embos, 17, "embos16_wide_advances_to_17");
    check::eq_i32(wide_m.draws(), 6, "embos16_wide_six_draws");
    check::eq_i32(wide.av, 450, "embos16_wide_av_450");

    // spark colours are snap-scaled and clamped to 0..255
    let mut hi_m = Preset::new(6);
    hi_m.m.snap = [10000, 10000, 10000];
    let mut hi = sample(&hi_m.m);
    hi.embos = 16;
    hi.d(&mut painted(), &mut hi_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(hi.embos, 17, "embos16_snap_hi_completes");
    check::eq_i32(hi_m.draws(), 6, "embos16_snap_hi_six_draws");

    let mut lo_m = Preset::new(6);
    lo_m.m.snap = [-10000, -10000, -10000];
    let mut lo = sample(&lo_m.m);
    lo.embos = 16;
    lo.d(&mut painted(), &mut lo_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(lo.embos, 17, "embos16_snap_lo_completes");
    check::eq_i32(lo_m.draws(), 6, "embos16_snap_lo_six_draws");

    // the ball's tilt sign uses |angle| after a >270 unwrap and a >90 flip
    let mut ang_m = Preset::new(6);
    let mut ang = sample(&ang_m.m);
    ang.embos = 16;
    ang.d(
        &mut painted(),
        &mut ang_m.m,
        0,
        0,
        0,
        0,
        280,
        100,
        0,
        0,
        false,
    );
    check::eq_i32(ang.embos, 17, "embos16_angle_signs_completes");
    check::eq_i32(ang_m.draws(), 6, "embos16_angle_signs_six_draws");
    check::eq_i32(ang.av, 0, "embos16_angle_signs_culled_av_0");

    // the other sign combination covers |zy|>270 and the >90 xy flip
    let mut ang2_m = Preset::new(6);
    let mut ang2 = sample(&ang2_m.m);
    ang2.embos = 16;
    ang2.d(
        &mut painted(),
        &mut ang2_m.m,
        0,
        0,
        0,
        0,
        100,
        280,
        0,
        0,
        false,
    );
    check::eq_i32(ang2.embos, 17, "embos16_zy_unwrap_completes");
    check::eq_i32(ang2_m.draws(), 6, "embos16_zy_unwrap_six_draws");

    // reversed pa/pb x and z order takes the -=30 side of both walks
    let mut rev_m = Preset::new(6);
    let mut rev = mk(
        &rev_m.m,
        &[500, 100, 300],
        &[600, 300, 400],
        &[200, 200, 200],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    rev.embos = 16;
    rev.d(&mut painted(), &mut rev_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(rev.embos, 17, "embos16_reversed_walk_completes");
    check::eq_i32(rev_m.draws(), 6, "embos16_reversed_walk_six_draws");
    check::eq_i32(rev.av, 450, "embos16_reversed_walk_av_450");

    // the var48 back-face probe swaps its two reference vertices when the
    // second one projects higher (:679-683); the drawn result is unchanged
    let mut swap = mk(
        &Medium::new(),
        &[275, 285, 275],
        &[400, 400, 400],
        &[210, 200, 200],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut swi = painted();
    let mut swap_m = Medium::new();
    swap.d(&mut swi, &mut swap_m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(swap.av, 400, "d_backface_swap_av_400");
    check::eq_i32(pixel(&swi, 277, 202), 0xC8C8C8, "d_backface_swap_fill_200");

    // chip colour roll == 0 takes new Color(c).darker() (:603-607)
    let mut dark_m = Preset::new(3);
    let mut dark = sample(&dark_m.m);
    dark.chip = 5;
    dark.d(&mut painted(), &mut dark_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(dark.chip, 6, "chip_darker_arm_completes");
    check::eq_i32(dark_m.draws(), 1, "chip_darker_arm_one_draw");

    // ctmag also clamps at the low end (:533-535)
    let mut fc = Fixed::new(0.5);
    let mut neg_mag = mk(
        &fc.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        5,
        false,
    );
    neg_mag.chip = 1;
    neg_mag.ctmag = -7.0;
    neg_mag.d(&mut painted(), &mut fc.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        bits(neg_mag.ctmag),
        bits(-3.0),
        "chip1_ctmag_clamped_minus3",
    );
    check::eq_i32(neg_mag.chip, 3, "chip1_neg_ctmag_chip_3");

    // var88 = projf/deltaf + 0.3 in the noline arm
    let mut p86 = sample(&Medium::new());
    p86.projf = 6.0e-7;
    let mut i86 = painted();
    let mut m86 = Medium::new();
    p86.d(&mut i86, &mut m86, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(pixel(&i86, 277, 202), 0xACACAC, "d_var88_086_band_172");

    let mut p37 = sample(&Medium::new());
    p37.projf = 6.0e-9;
    let mut i37 = painted();
    let mut m37 = Medium::new();
    p37.d(&mut i37, &mut m37, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(pixel(&i37, 277, 202), 0x4A4A4A, "d_var88_floor_037_74");

    // gr==-8 forces full brightness (:877-879)
    let mut g8 = sample(&Medium::new());
    g8.gr = -8;
    let mut i8 = painted();
    let mut m8 = Medium::new();
    g8.d(&mut i8, &mut m8, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(
        pixel(&i8, 277, 202),
        0xC8C8C8,
        "d_gr_minus8_full_brightness_200",
    );
    check::eq_i32(g8.av, 400, "d_gr_minus8_av_400");

    // the non-trk fill also blends toward cfade once per exceeded fade entry
    let mut fm = Medium::new();
    fm.fade[0] = 100;
    let mut fade_p = sample(&fm);
    let mut fi = painted();
    fade_p.d(&mut fi, &mut fm, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&fi, 277, 202),
        0xD5CDCD,
        "d_fill_one_fade_blend_213_205_205",
    );
    check::eq_i32(fade_p.av, 400, "d_fill_fade_blend_av_400");

    // the road arm clamps each fill-10 channel at zero (:976-993)
    let mut rm = Medium::new();
    rm.fade[0] = 5000;
    let mut dark_road = mk(
        &rm,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [5, 5, 5],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        true,
    );
    let mut dri = painted();
    dark_road.d(&mut dri, &mut rm, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(pixel(&dri, 277, 202), 0x040404, "d_road_dark_interior_4");
    check::eq_i32(
        pixel(&dri, 275, 200),
        0x000000,
        "d_road_dark_outline_clamped_0",
    );

    // the gr==-10 arm blends its c toward cfade with the same 8-step loop
    let mut gfm = Medium::new();
    gfm.fade[0] = 100;
    let mut g10 = mk(
        &gfm,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        -10,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut g10i = painted();
    g10.d(&mut g10i, &mut gfm, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(
        pixel(&g10i, 277, 202),
        0xC5BCBC,
        "d_gr10_fill_blend_197_188_188",
    );
    check::eq_i32(
        pixel(&g10i, 275, 200),
        0xD5CDCD,
        "d_gr10_outline_blend_213_205_205",
    );
    check::eq_i32(g10.av, 401, "d_gr10_blend_av_401");

    // flx snap clamps: >255 on both channels and the mixed 0/255 case
    let mut fh_m = Preset::new(3);
    fh_m.m.snap = [200, 200, 200];
    let mut fh = mk(
        &fh_m.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    fh.flx = 1;
    let mut fhi = painted();
    fh.d(&mut fhi, &mut fh_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&fhi, 275, 200),
        0x00FFFF,
        "d_flx1_snap200_clamped_255_255",
    );

    let mut fmx_m = Preset::new(3);
    fmx_m.m.snap[1] = -200;
    fmx_m.m.snap[2] = 200;
    let mut fmx = mk(
        &fmx_m.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    fmx.flx = 1;
    let mut fmxi = painted();
    fmx.d(&mut fmxi, &mut fmx_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&fmxi, 275, 200),
        0x0000FF,
        "d_flx1_mixed_snap_clamped_0_255",
    );

    // the tilted-tracker height correction (:1074-1096)
    let tilt_m = Medium::new();
    let mut tilt_t = matched_tracker(150, 150, 150, 150, 100);
    tilt_t.xy[0] = 30;
    tilt_t.zy[0] = 20;
    let tilt = shadow(&tilt_m, &tilt_t);
    let before = snapshot(&tilt);
    let mut ti = painted();
    tilt.s(&mut ti, &tilt_m, &tilt_t, 0, 0, 0, 0, 0, 0, 0);
    check::eq_i32(pixel(&ti, 277, 155), 0x646464, "s_tilted_tracker_fill_100");
    check::eq_str(snapshot(&tilt), before, "s_tilted_tracker_no_state_change");
    check::eq_i32(tilt.av, 0, "s_tilted_tracker_av_untouched");

    // all vertices project to the same height as vertex 0, below cy: the
    // var48 probe's second test resets the flag (:697-699)
    let mut flat = mk(
        &Medium::new(),
        &[300, 275, 275],
        &[400, 400, 500],
        &[210, 210, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut fli = painted();
    let mut flat_m = Medium::new();
    flat.d(&mut fli, &mut flat_m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(flat.av, 450, "d_backface_flat_av_450");
    check::eq_i32(pixel(&fli, 277, 209), 0xC8C8C8, "d_backface_flat_fill_200");

    // a true var48 with gr!=-7 forces var88=0.32 (:873-875)
    let mut bf = mk(
        &Medium::new(),
        &[290, 300, 275],
        &[500, 400, 400],
        &[210, 200, 200],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    let mut bfi = painted();
    let mut bf_m = Medium::new();
    bf.d(&mut bfi, &mut bf_m, 0, 0, 0, 0, 0, 0, 0, 0, true);
    check::eq_i32(bf.av, 450, "d_backface_true_av_450");
    check::eq_i32(
        pixel(&bfi, 290, 202),
        0x404040,
        "d_backface_true_var88_032_64",
    );

    // flx==3/1 snap clamp combinations (both channels, both directions)
    let mut f3a_m = Preset::new(3);
    f3a_m.m.snap[1] = -200;
    f3a_m.m.snap[2] = 200;
    let mut f3a = mk(
        &f3a_m.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    f3a.flx = 3;
    let mut f3ai = painted();
    f3a.d(&mut f3ai, &mut f3a_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&f3ai, 275, 200),
        0x0000FF,
        "d_flx3_snap_clamped_0_255",
    );

    let mut f3b_m = Preset::new(3);
    f3b_m.m.snap[1] = 200;
    f3b_m.m.snap[2] = -200;
    let mut f3b = mk(
        &f3b_m.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    f3b.flx = 3;
    let mut f3bi = painted();
    f3b.d(&mut f3bi, &mut f3b_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&f3bi, 275, 200),
        0x00FF00,
        "d_flx3_snap_clamped_255_0",
    );

    let mut f1n_m = Preset::new(3);
    f1n_m.m.snap[2] = -200;
    let mut f1n = mk(
        &f1n_m.m,
        &[275, 285, 275],
        &[400, 400, 400],
        &[200, 200, 210],
        3,
        [200, 200, 200],
        false,
        0,
        0,
        0,
        0,
        7,
        0,
        false,
    );
    f1n.flx = 1;
    let mut f1ni = painted();
    f1n.d(&mut f1ni, &mut f1n_m.m, 0, 0, 0, 0, 0, 0, 0, 0, false);
    check::eq_i32(
        pixel(&f1ni, 275, 200),
        0x00DF00,
        "d_flx1_blue_clamp_0_223_0",
    );
}
