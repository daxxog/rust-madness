//! JDK 8 `ProcessPath` shape engine — the exact pipeline behind
//! `Graphics.fillRoundRect` / `drawRoundRect` on an untransformed
//! `BufferedImage.TYPE_INT_RGB` surface with JDK 1.8.0_40.
//!
//! `SunGraphics2D` routes both calls through `LoopPipe`, which wraps the
//! shape into a `Path2D.Float` (narrowing every double coordinate to
//! `f32`) and feeds it to the `FillPath` / `DrawPath` natives. Those run
//! the shared `ProcessPath` driver (`ProcessPath.c`): a forward-differencing
//! flattener for the four bezier corners, `StoreFixedLine` point
//! accumulation + `FillPolygon` active-edge scanning for fills, and
//! `ProcessFixedLine` subpixel endpoint fixup + the `LineUtils.h`
//! Bresenham executor (with `LoopMacros.h`'s crucial
//! `BUMPMINOR += BUMPMAJOR` fused diagonal minor step) for draws.
//!
//! Everything here is transliterated from the jdk8 native sources
//! (`jdk/src/share/native/sun/java2d/loops/{ProcessPath.c, ProcessPath.h,
//! LineUtils.h, DrawLine.c, LoopMacros.h, FillPath.c, DrawPath.c}`),
//! preserving the C's exact arithmetic: `jfloat` math stays f32,
//! interpolations run in f64, `x as i32` reproduces C `(jint)` truncation,
//! and wrapping ops mirror the C's two's-complement overflow behaviour.
//! The stroke hint is assumed `VALUE_STROKE_DEFAULT` (pixels at corners);
//! the applet never sets `STROKE_PURE`, and segment type `SEG_QUADTO` is
//! unreachable from AWT geometry (rects, round rects, ellipses, arcs and
//! polygons never emit quads).

// ════════════════════════════ Output ════════════════════════════

/// `PixelSink` receives the renderer's output pixels.
pub trait PixelSink {
    /// One painted pixel (`pDrawPixel` / single-pixel line).
    fn pixel(&mut self, x: i32, y: i32);
    /// `drawScanline` — inclusive `[x0, x1]` span on row `y`.
    fn span(&mut self, x0: i32, x1: i32, y: i32);
}

// ════════════════════════════ Constants ════════════════════════════

/// `MDP_MULT` — subpixel resolution of the fixed-point geometry.
const MDP_MULT: i32 = 1024;
/// `MDP_PREC` (== `MDP_SHIFT`).
const MDP_PREC: i32 = 10;
/// `MDP_W_MASK` — strips the fraction bits.
const MDP_W_MASK: i32 = -MDP_MULT;
/// `MDP_HALF_MULT`.
const MDP_HALF_MULT: i32 = MDP_MULT / 2;

/// `FWD_PREC` — forward-differencing precision bits.
const FWD_PREC: i32 = 7;
/// `DF_CUB_STEPS`.
const DF_CUB_STEPS: i32 = 3;
/// `DF_CUB_SHIFT = FWD_PREC + DF_CUB_STEPS*3 - MDP_PREC`.
const DF_CUB_SHIFT: i32 = FWD_PREC + DF_CUB_STEPS * 3 - MDP_PREC;
/// `DF_CUB_COUNT`.
const DF_CUB_COUNT: i32 = 1 << DF_CUB_STEPS;
/// `DF_CUB_DEC_BND`.
const DF_CUB_DEC_BND: i32 = 1 << (DF_CUB_STEPS * 3 + FWD_PREC + 2);
/// `DF_CUB_INC_BND`.
const DF_CUB_INC_BND: i32 = 1 << (DF_CUB_STEPS * 3 + FWD_PREC - 1);
/// `CUB_A_MDP_MULT`.
const CUB_A_MDP_MULT: i32 = 1 << FWD_PREC;
/// `CUB_B_MDP_MULT`.
const CUB_B_MDP_MULT: i32 = 1 << (DF_CUB_STEPS + FWD_PREC + 1);
/// `CUB_C_MDP_MULT`.
const CUB_C_MDP_MULT: i32 = 1 << (DF_CUB_STEPS * 2 + FWD_PREC);

/// `MAX_CUB_SIZE` (compared as float in the C).
const MAX_CUB_SIZE: f32 = 256.0;

/// `EPSF`.
const EPSF: f32 = 1.0 / MDP_MULT as f32;
/// `CALC_BND`.
const CALC_BND: i32 = 1 << (30 - MDP_PREC);
/// `UPPER_BND` (ProcessPath.h: `FLT_MAX/4.0f`).
const UPPER_BND: f32 = f32::MAX / 4.0;
/// `LOWER_BND`.
const LOWER_BND: f32 = -UPPER_BND;
/// `UPPER_OUT_BND`.
const UPPER_OUT_BND: i32 = 1 << (30 - MDP_PREC);
/// `LOWER_OUT_BND`.
const LOWER_OUT_BND: i32 = -UPPER_OUT_BND;

/// RoundRectIterator's `acv`, bit-pinned from JDK 1.8.0_40
/// (`0x3fcca75ddd596bbe` = `0.22385762508460333`). The expression is
/// `(1 - cv) / 2` with `a = 1 - cos(PI/4)`, `b = tan(PI/4)`,
/// `c = sqrt(1 + b*b) - 1 + a`, `cv = 4/3·a·b / c`.
const ACV: f64 = f64::from_bits(0x3fcca75ddd596bbe);

// Bump direction masks (`LineUtils.h`).
const BUMP_POS_PIXEL: i32 = 0x1;
const BUMP_NEG_PIXEL: i32 = 0x2;
const BUMP_POS_SCAN: i32 = 0x4;
const BUMP_NEG_SCAN: i32 = 0x8;

#[inline]
fn wmul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

#[inline]
fn wdiv(a: i32, b: i32) -> i32 {
    a.wrapping_div(b)
}

#[inline]
fn wsub(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}

/// `ABS32(X)` — branchless wrapping absolute value.
#[inline]
fn i32_abs(v: i32) -> i32 {
    (v ^ (v >> 31)).wrapping_sub(v >> 31)
}

/// C `(jint)` truncating cast of a jfloat expression.
#[inline]
fn fi32(v: f32) -> i32 {
    v as i32
}

// ════════════════════════════ Path ════════════════════════════

/// Path segment types mirroring `java.awt.geom.PathIterator`.
pub const SEG_MOVETO: u8 = 0;
pub const SEG_LINETO: u8 = 1;
pub const SEG_CUBICTO: u8 = 3;
pub const SEG_CLOSE: u8 = 4;

/// A path in `Path2D.Float` storage: `types[k]` consumes
/// {1, 2, 3, 0} coordinate pairs from `coords`.
#[derive(Clone, Default)]
pub struct FlatPath {
    pub coords: Vec<f32>,
    pub types: Vec<u8>,
}

impl FlatPath {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

/// `RoundRectIterator` flattened through `new Path2D.Float(rr)`, exactly
/// as `LoopPipe.fillRoundRect` / `drawRoundRect` build it. Returns `None`
/// for the empty path (`RoundRectIterator` skips everything when
/// `aw < 0 || ah < 0`, which happens when `w` or `h` itself is negative).
pub fn round_rect_path(x: i32, y: i32, w: i32, h: i32, aw: i32, ah: i32) -> Option<FlatPath> {
    let fw = w as f64;
    let fh = h as f64;
    let fx = x as f64;
    let fy = y as f64;
    // RoundRectIterator ctor: `aw = min(w, |arcw|)`, `ah = min(h, |arch|)`.
    let arcw = min_f(fw, f64_abs(aw as f64));
    let arch = min_f(fh, f64_abs(ah as f64));
    if arcw < 0.0 || arch < 0.0 {
        return None;
    }

    // ctrlpts rows: 4 doubles per point (v0, v1, v2, v3) = point
    // (x + v0*w + v1*aw, y + v2*h + v3*ah), f64 math left to right,
    // narrowed to f32 by the Path2D.Float storage (`types` add SEG_CLOSE).
    const ROW_PTS: [usize; 9] = [1, 1, 3, 1, 3, 1, 3, 1, 3];
    const ROW_TYPES: [u8; 9] = [
        SEG_MOVETO,
        SEG_LINETO,
        SEG_CUBICTO,
        SEG_LINETO,
        SEG_CUBICTO, //
        SEG_LINETO,
        SEG_CUBICTO,
        SEG_LINETO,
        SEG_CUBICTO,
    ];
    let rows: [&[f64]; 9] = [
        &[0.0, 0.0, 0.0, 0.5],
        &[0.0, 0.0, 1.0, -0.5],
        &[
            0.0, 0.0, 1.0, -ACV, //
            0.0, ACV, 1.0, 0.0, //
            0.0, 0.5, 1.0, 0.0,
        ],
        &[1.0, -0.5, 1.0, 0.0],
        &[
            1.0, -ACV, 1.0, 0.0, //
            1.0, 0.0, 1.0, -ACV, //
            1.0, 0.0, 1.0, -0.5,
        ],
        &[1.0, 0.0, 0.0, 0.5],
        &[
            1.0, 0.0, 0.0, ACV, //
            1.0, -ACV, 0.0, 0.0, //
            1.0, -0.5, 0.0, 0.0,
        ],
        &[0.0, 0.5, 0.0, 0.0],
        &[
            0.0, ACV, 0.0, 0.0, //
            0.0, 0.0, 0.0, ACV, //
            0.0, 0.0, 0.0, 0.5,
        ],
    ];

    let mut coords = Vec::with_capacity(32);
    let mut types = Vec::with_capacity(10);
    for r in 0..9 {
        let q = rows[r];
        for p in 0..ROW_PTS[r] {
            let v = &q[p * 4..p * 4 + 4];
            // Java: `x + ctrls[i+0]*w + ctrls[i+1]*aw` in f64, left→right.
            let px = ((fx + v[0] * fw) + v[1] * arcw) as f32;
            let py = ((fy + v[2] * fh) + v[3] * arch) as f32;
            coords.push(px);
            coords.push(py);
        }
        types.push(ROW_TYPES[r]);
    }
    types.push(SEG_CLOSE);

    Some(FlatPath { coords, types })
}

#[inline]
fn f64_abs(v: f64) -> f64 {
    v.abs()
}

/// Java `Math.min` for non-NaN values: equal values yield `a`.
#[inline]
fn min_f(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

// ════════════════════════════ Clipping primitives ════════════════════════════

/// The `CRES_*` enum order: MIN_CLIPPED, MAX_CLIPPED, NOT_CLIPPED, INVISIBLE.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cres {
    MinClipped,
    MaxClipped,
    NotClipped,
    Invisible,
}

impl Cres {
    /// `IS_CLIPPED`.
    #[inline]
    fn is_clipped(self) -> bool {
        self == Cres::MinClipped || self == Cres::MaxClipped
    }
}

/// `CLIP(a1, b1, a2, b2, t)` for jfloat coordinates: interpolate b at the
/// boundary t, in f64 with the `(b2 - b1)` / `(a2 - a1)` differences taken
/// in f32 first (exact C promotion semantics).
#[inline]
fn clipf(a1: f32, b1: f32, a2: f32, b2: f32, t: f64) -> f32 {
    let d = (t - a1 as f64) * ((b2 - b1) as f64) / ((a2 - a1) as f64);
    (b1 as f64 + d) as f32
}

/// `CLIP` for jint coordinates (ints promote to f64 exactly).
#[inline]
fn clipi(a1: i32, b1: i32, a2: i32, b2: i32, t: f64) -> i32 {
    let d = (t - a1 as f64) * (b2 - b1) as f64 / (a2 - a1) as f64;
    (b1 as f64 + d) as i32
}

/// `TESTANDCLIP` (jfloat instantiation): test `a1` against
/// `[line_min, line_max]`, clamp it to the boundary and linearly
/// interpolate `b1` toward `b2`. Mutates `a1`/`b1`.
fn test_and_clipf(
    line_min: f32,
    line_max: f32,
    a1: &mut f32,
    b1: &mut f32,
    a2: f32,
    b2: f32,
    res: &mut Cres,
) {
    *res = Cres::NotClipped;
    if *a1 < line_min || *a1 > line_max {
        let t: f64 = if *a1 < line_min {
            if a2 < line_min {
                *res = Cres::Invisible;
                return;
            }
            *res = Cres::MinClipped;
            line_min as f64
        } else {
            if a2 > line_max {
                *res = Cres::Invisible;
                return;
            }
            *res = Cres::MaxClipped;
            line_max as f64
        };
        *b1 = clipf(*a1, *b1, a2, b2, t);
        *a1 = t as f32;
    }
}

/// `TESTANDCLIP` (jint instantiation — `StoreFixedLine`'s
/// `xMinf*MDP_MULT` family). Mutates `a1`/`b1`.
fn test_and_clipi(
    line_min: i32,
    line_max: i32,
    a1: &mut i32,
    b1: &mut i32,
    a2: i32,
    b2: i32,
    res: &mut Cres,
) {
    *res = Cres::NotClipped;
    if *a1 < line_min || *a1 > line_max {
        let t: f64 = if *a1 < line_min {
            if a2 < line_min {
                *res = Cres::Invisible;
                return;
            }
            *res = Cres::MinClipped;
            line_min as f64
        } else {
            if a2 > line_max {
                *res = Cres::Invisible;
                return;
            }
            *res = Cres::MaxClipped;
            line_max as f64
        };
        *b1 = clipi(*a1, *b1, a2, b2, t);
        *a1 = t as i32;
    }
}

/// `TESTANDCLIP` as used by `PROCESS_LINE`'s `checkBounds`: jint pixel
/// coordinates compared against *f32* bounds (the C promotes the jint to
/// jfloat for the comparisons but keeps f64 for the interpolation).
fn test_and_clip_mixed(
    line_min: f32,
    line_max: f32,
    a1: &mut i32,
    b1: &mut i32,
    a2: i32,
    b2: i32,
    res: &mut Cres,
) {
    *res = Cres::NotClipped;
    let a1f = *a1 as f32;
    if a1f < line_min || a1f > line_max {
        let t: f64 = if a1f < line_min {
            if (a2 as f32) < line_min {
                *res = Cres::Invisible;
                return;
            }
            *res = Cres::MinClipped;
            line_min as f64
        } else {
            if (a2 as f32) > line_max {
                *res = Cres::Invisible;
                return;
            }
            *res = Cres::MaxClipped;
            line_max as f64
        };
        *b1 = clipi(*a1, *b1, a2, b2, t);
        *a1 = t as i32;
    }
}

/// `CLIPCLAMP` results: resolution + mutated `(a1, b1)` + the clump
/// vertex `(a3, b3)` (x-part only updated on clips — CLIPCLAMP sets
/// `a3 = a1` but leaves `b3` at its original value) + the possibly
/// re-pinned `(a2, b2)` (INVISIBLE-left clump pins `a2` to LINE_MIN).
struct ClampOut<T> {
    res: Cres,
    p1: (T, T),
    p3: (T, T),
    p2: (T, T),
}

/// `CLIPCLAMP` for jfloat (ProcessLine fill branch).
fn clip_clampf(line_min: f32, line_max: f32, p1: (f32, f32), p2: (f32, f32)) -> ClampOut<f32> {
    let (mut a1, mut b1) = p1;
    let (mut a2, b2) = p2;
    let mut res = Cres::NotClipped;
    // a3/b3 take (a1, b1) before the test; only a3 is refreshed on clips.
    let mut p3 = p1;
    test_and_clipf(line_min, line_max, &mut a1, &mut b1, a2, b2, &mut res);
    if res == Cres::MinClipped {
        p3.0 = a1;
    } else if res == Cres::MaxClipped {
        p3.0 = a1;
        res = Cres::MaxClipped;
    } else if res == Cres::Invisible {
        if a1 > line_max {
            res = Cres::Invisible;
        } else {
            // Left-side clump: pin both a's on LINE_MIN, keep the ys.
            a1 = line_min;
            a2 = line_min;
            res = Cres::NotClipped;
        }
    }
    ClampOut {
        res,
        p1: (a1, b1),
        p3,
        p2: (a2, b2),
    }
}

/// `CLIPCLAMP` for jint (`StoreFixedLine` clipping path).
fn clip_clampi(line_min: i32, line_max: i32, p1: (i32, i32), p2: (i32, i32)) -> ClampOut<i32> {
    let (mut a1, mut b1) = p1;
    let (mut a2, b2) = p2;
    let mut res = Cres::NotClipped;
    let mut p3 = p1;
    test_and_clipi(line_min, line_max, &mut a1, &mut b1, a2, b2, &mut res);
    if res == Cres::MinClipped {
        p3.0 = a1;
    } else if res == Cres::MaxClipped {
        p3.0 = a1;
        res = Cres::MaxClipped;
    } else if res == Cres::Invisible {
        if a1 > line_max {
            res = Cres::Invisible;
        } else {
            a1 = line_min;
            a2 = line_min;
            res = Cres::NotClipped;
        }
    }
    ClampOut {
        res,
        p1: (a1, b1),
        p3,
        p2: (a2, b2),
    }
}

// ════════════════════════════ LineUtils.h ════════════════════════════

/// `LineUtils_ProcessLine`'s `SETORDERED`.
#[inline]
fn set_ordered(a: i32, b: i32, shorten: i32) -> (i32, i32) {
    if a < b {
        (a, b - shorten)
    } else {
        (b + shorten, a)
    }
}

/// `outcode(x, y, ...)` — TOP=1, BOTTOM=2, LEFT=4, RIGHT=8.
#[inline]
fn outcode(x: i32, y: i32, xmin: i32, ymin: i32, xmax: i32, ymax: i32) -> i32 {
    const OUTCODE_TOP: i32 = 1;
    const OUTCODE_BOTTOM: i32 = 2;
    const OUTCODE_LEFT: i32 = 4;
    const OUTCODE_RIGHT: i32 = 8;
    let cy = if y < ymin {
        OUTCODE_TOP
    } else if y > ymax {
        OUTCODE_BOTTOM
    } else {
        0
    };
    let cx = if x < xmin {
        OUTCODE_LEFT
    } else if x > xmax {
        OUTCODE_RIGHT
    } else {
        0
    };
    cy | cx
}

/// `InitBumps` — mask to concrete pointer offset; in our two-axis pixel
/// space `PIXELSTRIDE` = (+1, 0) and `SCANSTRIDE` = (0, +1). The C's
/// `BUMPMINOR` chain falls through to NOOP (0, 0) and `BUMPMAJOR`'s tail
/// is -SCANSTRIDE; both tails are unreachable (a bump mask is always one
/// of the four), but the shapes differ per `major`, hence the lint
/// allowance on the duplicated block bodies.
#[allow(clippy::if_same_then_else)]
fn bump_pair(major: bool, mask: i32) -> (i32, i32) {
    if mask & BUMP_POS_PIXEL != 0 {
        (1, 0)
    } else if mask & BUMP_NEG_PIXEL != 0 {
        (-1, 0)
    } else if mask & BUMP_POS_SCAN != 0 {
        (0, 1)
    } else if mask & BUMP_NEG_SCAN != 0 {
        (0, -1)
    } else if major {
        (0, -1) // unreachable; InitBumps' major fallback, kept for shape
    } else {
        (0, 0)
    }
}

/// `LineUtils_SetupBresenham` result.
struct BresSetup {
    start_x: i32,
    start_y: i32,
    steps: i32,
    error: i32,
    errmajor: i32,
    errminor: i32,
    bumpmajor: (i32, i32),
    bumpminor: (i32, i32),
}

/// `LineUtils_SetupBresenham` (the "small" jint path — canvas pixel
/// coordinates are far below the `OverflowsSmall` ±16383 limit, so the
/// `LineUtils_SetupBresenhamBig` pre-clip wrapper never triggers).
/// Returns `None` when the executor must not run.
fn setup_bresenham(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    shorten: i32,
    bounds: &RasBounds,
) -> Option<BresSetup> {
    let mut x1 = x1;
    let mut y1 = y1;
    let mut x2 = x2;
    let mut y2 = y2;
    let orig_x1 = x1;
    let orig_y1 = y1;
    let orig_x2 = x2;
    let orig_y2 = y2;

    let dx = wsub(x2, x1);
    let dy = wsub(y2, y1);
    let mut ax = i32_abs(dx);
    let mut ay = i32_abs(dy);

    let cxmin = bounds.x_min;
    let cymin = bounds.y_min;
    let cxmax = bounds.x_max - 1;
    let cymax = bounds.y_max - 1;
    let xmajor = ax >= ay;

    let mut outcode1 = outcode(x1, y1, cxmin, cymin, cxmax, cymax);
    let mut outcode2 = outcode(x2, y2, cxmin, cymin, cxmax, cymax);
    while (outcode1 | outcode2) != 0 {
        let mut xsteps: i32;
        let mut ysteps: i32;
        if (outcode1 & outcode2) != 0 {
            return None;
        }
        if outcode1 != 0 {
            if (outcode1 & (outcode::TOP | outcode::BOTTOM)) != 0 {
                y1 = if outcode1 & outcode::TOP != 0 {
                    cymin
                } else {
                    cymax
                };
                ysteps = wsub(y1, orig_y1);
                if ysteps < 0 {
                    ysteps = -ysteps;
                }
                xsteps = wmul(wmul(2, ysteps), ax) + ay;
                if xmajor {
                    xsteps += ay - ax - 1;
                }
                xsteps = wdiv(xsteps, 2 * ay);
                if dx < 0 {
                    xsteps = -xsteps;
                }
                x1 = orig_x1 + xsteps;
            } else if (outcode1 & (outcode::LEFT | outcode::RIGHT)) != 0 {
                x1 = if outcode1 & outcode::LEFT != 0 {
                    cxmin
                } else {
                    cxmax
                };
                xsteps = wsub(x1, orig_x1);
                if xsteps < 0 {
                    xsteps = -xsteps;
                }
                ysteps = wmul(wmul(2, xsteps), ay) + ax;
                if !xmajor {
                    ysteps += ax - ay - 1;
                }
                ysteps = wdiv(ysteps, 2 * ax);
                if dy < 0 {
                    ysteps = -ysteps;
                }
                y1 = orig_y1 + ysteps;
            }
            outcode1 = outcode(x1, y1, cxmin, cymin, cxmax, cymax);
        } else {
            if (outcode2 & (outcode::TOP | outcode::BOTTOM)) != 0 {
                y2 = if outcode2 & outcode::TOP != 0 {
                    cymin
                } else {
                    cymax
                };
                ysteps = wsub(y2, orig_y2);
                if ysteps < 0 {
                    ysteps = -ysteps;
                }
                xsteps = wmul(wmul(2, ysteps), ax) + ay;
                if xmajor {
                    xsteps += ay - ax;
                } else {
                    xsteps -= 1;
                }
                xsteps = wdiv(xsteps, 2 * ay);
                if dx > 0 {
                    xsteps = -xsteps;
                }
                x2 = orig_x2 + xsteps;
            } else if (outcode2 & (outcode::LEFT | outcode::RIGHT)) != 0 {
                x2 = if outcode2 & outcode::LEFT != 0 {
                    cxmin
                } else {
                    cxmax
                };
                xsteps = wsub(x2, orig_x2);
                if xsteps < 0 {
                    xsteps = -xsteps;
                }
                ysteps = wmul(wmul(2, xsteps), ay) + ax;
                if xmajor {
                    ysteps -= 1;
                } else {
                    ysteps += ax - ay;
                }
                ysteps = wdiv(ysteps, 2 * ax);
                if dy > 0 {
                    ysteps = -ysteps;
                }
                y2 = orig_y2 + ysteps;
            }
            outcode2 = outcode(x2, y2, cxmin, cymin, cxmax, cymax);
        }
    }

    let start_x = x1;
    let start_y = y1;

    // Bump masks follow the ORIGINAL (pre-clip) delta signs.
    let errmajor;
    let errminor;
    let bumpmajor_mask;
    let bumpminor_mask;
    let mut steps;
    let mut shorten = shorten;
    if xmajor {
        errmajor = 2 * ay; // signed; > 0 iff the line is not horizontal
        errminor = 2 * ax;
        bumpmajor_mask = if dx < 0 {
            BUMP_NEG_PIXEL
        } else {
            BUMP_POS_PIXEL
        };
        bumpminor_mask = if dy < 0 { BUMP_NEG_SCAN } else { BUMP_POS_SCAN };
        ax = -ax; // for the clip adjustment below
        steps = wsub(x2, x1);
        if x2 != orig_x2 {
            shorten = 0;
        }
    } else {
        errmajor = 2 * ax;
        errminor = 2 * ay;
        bumpmajor_mask = if dy < 0 { BUMP_NEG_SCAN } else { BUMP_POS_SCAN };
        bumpminor_mask = if dx < 0 {
            BUMP_NEG_PIXEL
        } else {
            BUMP_POS_PIXEL
        };
        ay = -ay; // for the clip adjustment below
        steps = wsub(y2, y1);
        if y2 != orig_y2 {
            shorten = 0;
        }
    }

    steps = i32_abs(steps) + 1 - shorten;
    if steps == 0 {
        return None;
    }

    let mut error = -(errminor / 2); // C jint division truncates toward zero
    if y1 != orig_y1 {
        let ysteps = i32_abs(wsub(y1, orig_y1));
        error = error.wrapping_add(wmul(wmul(ysteps, ax), 2));
    }
    if x1 != orig_x1 {
        let xsteps = i32_abs(wsub(x1, orig_x1));
        error = error.wrapping_add(wmul(wmul(xsteps, ay), 2));
    }
    error += errmajor;
    let errminor = errminor - errmajor;

    let bumpmajor = bump_pair(true, bumpmajor_mask);
    // `BUMPMINOR += BUMPMAJOR` — the minor bump advances BOTH axes.
    let bm = bump_pair(false, bumpminor_mask);
    let bumpminor = (bm.0 + bumpmajor.0, bm.1 + bumpmajor.1);

    Some(BresSetup {
        start_x,
        start_y,
        steps,
        error,
        errmajor,
        errminor,
        bumpmajor,
        bumpminor,
    })
}

mod outcode {
    pub const TOP: i32 = 1;
    pub const BOTTOM: i32 = 2;
    pub const LEFT: i32 = 4;
    pub const RIGHT: i32 = 8;
}

// ════════════════════════════ Engine ════════════════════════════

/// Raster bounds (`SurfaceDataRasInfo.bounds` — `{0, 0, w, h}` for a
/// full-canvas unclipped surface).
pub struct RasBounds {
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
}

/// `DrawHandler` — raster bounds + the fractional clip box
/// (`ProcessPath.c:1496-1521`).
struct DrawHandler {
    b: RasBounds,
    x_minf: f32,
    x_maxf: f32,
    y_minf: f32,
    y_maxf: f32,
}

/// FillData point (`Point` in ProcessPath.c:1768+).
struct Pnt {
    x: i32, // fixed ×1024
    y: i32,
    last_point: bool,
}

/// Active-list edge (`Edge` in ProcessPath.c:1778+). Links are pool
/// indices; `-1` = NULL.
struct Edge {
    x: i32,
    dx: i32,
    dir: i32,
    prev: isize,
    next: isize,
}

/// `clipMode`: `PH_MODE_DRAW_CLIP` / `PH_MODE_FILL_CLIP`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lane {
    Draw,
    Fill,
}

/// `ProcessHandler` + `FillData` + `DrawHandler` state for one render.
struct Engine<'a, S: PixelSink> {
    sink: &'a mut S,
    lane: Lane,
    dh: DrawHandler,
    /// FillData points (fixed-point ×1024).
    pts: Vec<Pnt>,
    fd_ymin: i32,
    fd_ymax: i32,
    /// Draw-lane `pixelInfo` (XOR-mode dedup state).
    pix: [i32; 5],
}

impl<'a, S: PixelSink> Engine<'a, S> {
    /// `doFillPath` / `doDrawPath` DrawHandler init plus `ProcessPath`'s
    /// ADJUST + fractional clip box. The applet's canvases are identity
    /// transforms with no user clip, so `transX/transY` and the
    /// STROKE_PURE shift never apply.
    fn new(lane: Lane, sink: &'a mut S, w: i32, h: i32) -> Engine<'a, S> {
        let mut dh = DrawHandler {
            b: RasBounds {
                x_min: 0,
                x_max: w,
                y_min: 0,
                y_max: h,
            },
            x_minf: 0.0,
            x_maxf: 0.0,
            y_minf: 0.0,
            y_maxf: 0.0,
        };
        // ADJUST(..., LOWER_OUT_BND, UPPER_OUT_BND) — canvas bounds are
        // deep inside those limits; mirror the code anyway.
        dh.b.x_min = dh.b.x_min.clamp(LOWER_OUT_BND, UPPER_OUT_BND);
        dh.b.y_min = dh.b.y_min.clamp(LOWER_OUT_BND, UPPER_OUT_BND);
        dh.b.x_max = dh.b.x_max.clamp(LOWER_OUT_BND, UPPER_OUT_BND);
        dh.b.y_max = dh.b.y_max.clamp(LOWER_OUT_BND, UPPER_OUT_BND);
        dh.x_minf = dh.b.x_min as f32 - 0.5;
        dh.y_minf = dh.b.y_min as f32 - 0.5;
        dh.x_maxf = dh.b.x_max as f32 - 0.5 - EPSF;
        dh.y_maxf = dh.b.y_max as f32 - 0.5 - EPSF;
        Engine {
            sink,
            lane,
            dh,
            pts: Vec::new(),
            fd_ymin: 0,
            fd_ymax: 0,
            pix: [0; 5],
        }
    }

    // ─────── FillData ───────

    /// `FD_IS_EMPTY`.
    #[inline]
    fn fd_is_empty(&self) -> bool {
        self.pts.is_empty()
    }

    /// `FD_IS_ENDED`.
    #[inline]
    fn fd_is_ended(&self) -> bool {
        self.pts[self.pts.len() - 1].last_point
    }

    /// `FD_ADD_POINT` — appends (x, y) and maintains `plgYMin`/`plgYMax`,
    /// initializing them from the first point.
    #[inline]
    fn fd_add_point(&mut self, x: i32, y: i32) {
        if self.pts.is_empty() {
            self.fd_ymin = y;
            self.fd_ymax = y;
        } else {
            if self.fd_ymin > y {
                self.fd_ymin = y;
            }
            if self.fd_ymax < y {
                self.fd_ymax = y;
            }
        }
        self.pts.push(Pnt {
            x,
            y,
            last_point: false,
        });
    }

    /// `FD_SET_ENDED`.
    #[inline]
    fn fd_set_ended(&mut self) {
        let n = self.pts.len();
        self.pts[n - 1].last_point = true;
    }

    /// `endSubPath` (fill lane; the draw lane's stub is a no-op).
    #[inline]
    fn fd_end_sub_path(&mut self) {
        if self.lane != Lane::Fill {
            return;
        }
        if !self.fd_is_empty() {
            self.fd_set_ended();
        }
    }

    /// `hnd->pProcessFixedLine(...)` — the lane-function dispatcher used
    /// by `DrawMonotonicCubic` (fixed-point emit, `endSubPath` = false).
    #[inline]
    fn fd_fixed_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, check_bounds: bool) {
        match self.lane {
            Lane::Fill => self.store_fixed_line(x1, y1, x2, y2, check_bounds, false),
            Lane::Draw => self.process_fixed_line(x1, y1, x2, y2, check_bounds),
        }
    }

    // ─────── StoreFixedLine (fill lane) ───────

    /// `StoreFixedLine` (ProcessPath.c:2094): fill-lane point accumulation
    /// with clipping/clumping. `end_sub` carries the `lastClipped`
    /// end-of-subpath notification.
    fn store_fixed_line(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        check_bounds: bool,
        end_sub: bool,
    ) {
        if !check_bounds {
            // Plain accumulation: first point only when the list is empty
            // or just finished; always the second point.
            if self.fd_is_empty() || self.fd_is_ended() {
                self.fd_add_point(x1, y1);
            }
            self.fd_add_point(x2, y2);
            if end_sub {
                self.fd_set_ended();
            }
            return;
        }

        // Clipping path (enterable only from the flattener when a segment
        // crosses the visible-area boundary).
        let out_x_min = fi32(self.dh.x_minf * MDP_MULT as f32);
        let out_x_max = fi32(self.dh.x_maxf * MDP_MULT as f32);
        let out_y_min = fi32(self.dh.y_minf * MDP_MULT as f32);
        let out_y_max = fi32(self.dh.y_maxf * MDP_MULT as f32);
        let mut x1 = x1;
        let mut y1 = y1;
        let mut x2 = x2;
        let mut y2 = y2;
        let mut res = Cres::NotClipped;
        let mut last_clipped;

        // TESTANDCLIP(outYMin, outYMax, y1, x1, y2, x2, jint, res)
        test_and_clipi(out_y_min, out_y_max, &mut y1, &mut x1, y2, x2, &mut res);
        if res == Cres::Invisible {
            return;
        }
        // TESTANDCLIP(outYMin, outYMax, y2, x2, y1, x1, jint, res)
        test_and_clipi(out_y_min, out_y_max, &mut y2, &mut x2, y1, x1, &mut res);
        if res == Cres::Invisible {
            return;
        }
        last_clipped = res.is_clipped();

        // CLIPCLAMP(outXMin, outXMax, x1, y1, x2, y2, x3, y3, jint, res)
        let mut c = clip_clampi(out_x_min, out_x_max, (x1, y1), (x2, y2));
        x1 = c.p1.0;
        y1 = c.p1.1;
        x2 = c.p2.0;
        if c.res == Cres::MinClipped {
            // Clamping only by left boundary: bite off the clump lead-in.
            self.store_fixed_line(c.p3.0, c.p3.1, x1, y1, false, last_clipped);
        } else if c.res == Cres::Invisible {
            return;
        }

        // CLIPCLAMP(outXMin, outXMax, x2, y2, x1, y1, x3, y3, jint, res)
        c = clip_clampi(out_x_min, out_x_max, (x2, y2), (x1, y1));
        x2 = c.p1.0;
        y2 = c.p1.1;

        // Checking if there was a clip by the right boundary.
        last_clipped = last_clipped || c.res == Cres::MaxClipped;

        // The clipped recursions pass checkBounds=false.
        self.store_fixed_line(x1, y1, x2, y2, false, last_clipped);

        // Clamping only by left boundary.
        if c.res == Cres::MinClipped {
            self.store_fixed_line(x2, y2, c.p3.0, c.p3.1, false, last_clipped);
        }
    }

    // ─────── Draw lane ───────

    /// `PROCESS_POINT` (ProcessPath.c:176-217).
    fn process_point(&mut self, fx: i32, fy: i32, check_bounds: bool) {
        let px = fx >> MDP_PREC;
        let py = fy >> MDP_PREC;
        if check_bounds
            && (self.dh.b.y_min > py
                || self.dh.b.y_max <= py
                || self.dh.b.x_min > px
                || self.dh.b.x_max <= px)
        {
            return; // `break;` in the macro
        }
        if self.pix[0] == 0 {
            self.pix[0] = 1;
            self.pix[1] = px;
            self.pix[2] = py;
            self.pix[3] = px;
            self.pix[4] = py;
            self.dpixel(px, py);
        } else if (px != self.pix[3] || py != self.pix[4])
            && (px != self.pix[1] || py != self.pix[2])
        {
            self.dpixel(px, py);
            self.pix[3] = px;
            self.pix[4] = py;
        }
    }

    /// `ProcessFixedLine` (ProcessPath.c:473): draw-lane subpixel endpoint
    /// computation, then `PROCESS_LINE`.
    fn process_fixed_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, check_bounds: bool) {
        // Inside a (X,Y),(X+MDP_MULT,Y+MDP_MULT) box?
        let c = (x1 ^ x2) | (y1 ^ y2);
        if (c & MDP_W_MASK) == 0 {
            if c == 0 {
                // Same start/end points with integer coordinates.
                self.process_point(x1 + MDP_HALF_MULT, y1 + MDP_HALF_MULT, check_bounds);
            }
            return;
        }

        let rx1: i32;
        let ry1: i32;
        let rx2: i32;
        let ry2: i32;
        if x1 == x2 || y1 == y2 {
            rx1 = x1 + MDP_HALF_MULT;
            rx2 = x2 + MDP_HALF_MULT;
            ry1 = y1 + MDP_HALF_MULT;
            ry2 = y2 + MDP_HALF_MULT;
        } else {
            // Neither dx nor dy can be zero here.
            let dx = wsub(x2, x1);
            let dy = wsub(y2, y1);
            let fx1 = x1 & MDP_W_MASK;
            let fy1 = y1 & MDP_W_MASK;
            let fx2 = x2 & MDP_W_MASK;
            let fy2 = y2 & MDP_W_MASK;

            // First endpoint: snap to the pixel edge it leaves toward the
            // segment (center-half rounding at the far edge).
            if fx1 == x1 || fy1 == y1 {
                rx1 = x1 + MDP_HALF_MULT;
                ry1 = y1 + MDP_HALF_MULT;
            } else {
                let bx1 = if x1 < x2 { fx1 + MDP_MULT } else { fx1 };
                let by1 = if y1 < y2 { fy1 + MDP_MULT } else { fy1 };
                // Intersection with column bx1.
                let cross = y1.wrapping_add(wdiv(wmul(wsub(bx1, x1), dy), dx));
                if cross >= fy1 && cross <= fy1 + MDP_MULT {
                    rx1 = bx1;
                    ry1 = cross + MDP_HALF_MULT;
                } else {
                    // Intersection with row by1.
                    let cross = x1.wrapping_add(wdiv(wmul(wsub(by1, y1), dx), dy));
                    rx1 = cross + MDP_HALF_MULT;
                    ry1 = by1;
                }
            }

            // Second endpoint.
            if fx2 == x2 || fy2 == y2 {
                rx2 = x2 + MDP_HALF_MULT;
                ry2 = y2 + MDP_HALF_MULT;
            } else {
                let bx2 = if x1 > x2 { fx2 + MDP_MULT } else { fx2 };
                let by2 = if y1 > y2 { fy2 + MDP_MULT } else { fy2 };
                let cross = y2.wrapping_add(wdiv(wmul(wsub(bx2, x2), dy), dx));
                if cross >= fy2 && cross <= fy2 + MDP_MULT {
                    rx2 = bx2;
                    ry2 = cross + MDP_HALF_MULT;
                } else {
                    let cross = x2.wrapping_add(wdiv(wmul(wsub(by2, y2), dx), dy));
                    rx2 = cross + MDP_HALF_MULT;
                    ry2 = by2;
                }
            }
        }

        self.process_line_fixed(rx1, ry1, rx2, ry2, check_bounds);
    }

    /// `PROCESS_LINE` (ProcessPath.c:113): pixel casts, optional bounds
    /// clip, XOR-dedup bookkeeping and the Bresenham dispatch.
    fn process_line_fixed(&mut self, fx0: i32, fy0: i32, fx1: i32, fy1: i32, check_bounds: bool) {
        let mut x0 = fx0 >> MDP_PREC;
        let mut y0 = fy0 >> MDP_PREC;
        let mut x1 = fx1 >> MDP_PREC;
        let mut y1 = fy1 >> MDP_PREC;
        let mut res = Cres::NotClipped;

        if check_bounds {
            // jint pixels checked against f32 bounds (`xMinf + 0.5f`...).
            let x_minf = self.dh.x_minf + 0.5;
            let y_minf = self.dh.y_minf + 0.5;
            let x_maxf = self.dh.x_maxf + 0.5;
            let y_maxf = self.dh.y_maxf + 0.5;
            // TESTANDCLIP(yMinf, yMaxf, Y0, X0, Y1, X1, jint, res)
            test_and_clip_mixed(y_minf, y_maxf, &mut y0, &mut x0, y1, x1, &mut res);
            if res == Cres::Invisible {
                return;
            }
            // TESTANDCLIP(yMinf, yMaxf, Y1, X1, Y0, X0, jint, res)
            test_and_clip_mixed(y_minf, y_maxf, &mut y1, &mut x1, y0, x0, &mut res);
            if res == Cres::Invisible {
                return;
            }
            // TESTANDCLIP(xMinf, xMaxf, X0, Y0, X1, Y1, jint, res)
            test_and_clip_mixed(x_minf, x_maxf, &mut x0, &mut y0, x1, y1, &mut res);
            if res == Cres::Invisible {
                return;
            }
            // TESTANDCLIP(xMinf, xMaxf, X1, Y1, X0, Y0, jint, res)
            test_and_clip_mixed(x_minf, x_maxf, &mut x1, &mut y1, x0, y0, &mut res);
            if res == Cres::Invisible {
                return;
            }
        }

        // Single-pixel segment.
        if ((x0 ^ x1) | (y0 ^ y1)) == 0 {
            if self.pix[0] == 0 {
                self.pix[0] = 1;
                self.pix[1] = x0;
                self.pix[2] = y0;
                self.pix[3] = x0;
                self.pix[4] = y0;
                self.dpixel(x0, y0);
            } else if (x0 != self.pix[3] || y0 != self.pix[4])
                && (x0 != self.pix[1] || y0 != self.pix[2])
            {
                self.dpixel(x0, y0);
                self.pix[3] = x0;
                self.pix[4] = y0;
            }
            return;
        }

        // Paint the joint pixel again if it already belongs to the path.
        if self.pix[0] != 0
            && ((self.pix[1] == x0 && self.pix[2] == y0)
                || (self.pix[3] == x0 && self.pix[4] == y0))
        {
            self.dpixel(x0, y0);
        }

        self.draw_line_int(x0, y0, x1, y1);

        if self.pix[0] == 0 {
            self.pix[0] = 1;
            self.pix[1] = x0;
            self.pix[2] = y0;
            self.pix[3] = x0;
            self.pix[4] = y0;
        }
        // Switch on the last pixel if it was drawn by a previous segment.
        if (self.pix[1] == x1 && self.pix[2] == y1) || (self.pix[3] == x1 && self.pix[4] == y1) {
            self.dpixel(x1, y1);
        }
        self.pix[3] = x1;
        self.pix[4] = y1;
    }

    #[inline]
    fn dpixel(&mut self, x: i32, y: i32) {
        self.sink.pixel(x, y);
    }

    // ─────── LineUtils_ProcessLine / the executor ───────

    /// `LineUtils_ProcessLine` (LineUtils.h:44): raster gate for
    /// horizontal/vertical runs and `SetupBresenham` + executor for the
    /// diagonal case. `shorten` is always 0 on these call sites.
    fn draw_line_int(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        if y1 == y2 {
            if y1 >= self.dh.b.y_min && y1 < self.dh.b.y_max {
                // SETORDERED(X1, X2, tx1, tx2, 0) then ++tx2 (inclusive).
                let (mut tx1, mut tx2) = set_ordered(x1, x2, 0);
                tx2 += 1;
                if tx2 < tx1 {
                    tx2 -= 1; // integer overflow
                }
                if tx1 < self.dh.b.x_min {
                    tx1 = self.dh.b.x_min;
                }
                if tx2 > self.dh.b.x_max {
                    tx2 = self.dh.b.x_max;
                }
                if tx1 < tx2 {
                    // steps = tx2 - tx1, bumpmajor = POS_PIXEL, errmajor = 0.
                    self.solid_draw_line(tx1, y1, tx2 - tx1, 0, (1, 0), 0, (0, 0), 0);
                }
            }
        } else if x1 == x2 {
            if x1 >= self.dh.b.x_min && x1 < self.dh.b.x_max {
                let (mut ty1, mut ty2) = set_ordered(y1, y2, 0);
                ty2 += 1;
                if ty2 < ty1 {
                    ty2 -= 1;
                }
                if ty1 < self.dh.b.y_min {
                    ty1 = self.dh.b.y_min;
                }
                if ty2 > self.dh.b.y_max {
                    ty2 = self.dh.b.y_max;
                }
                if ty1 < ty2 {
                    self.solid_draw_line(x1, ty1, ty2 - ty1, 0, (0, 1), 0, (0, 0), 0);
                }
            }
        } else if let Some(setup) = setup_bresenham(x1, y1, x2, y2, 0, &self.dh.b) {
            self.solid_draw_line(
                setup.start_x,
                setup.start_y,
                setup.steps,
                setup.error,
                setup.bumpmajor,
                setup.errmajor,
                setup.bumpminor,
                setup.errminor,
            );
        }
    }

    /// `AnyIntDrawLine` (LoopMacros.h `DEFINE_SOLID_DRAWLINE`): the exact
    /// executor, including the fused minor bump (`BUMPMINOR += BUMPMAJOR`).
    #[allow(clippy::too_many_arguments)]
    fn solid_draw_line(
        &mut self,
        mut x: i32,
        mut y: i32,
        steps: i32,
        mut error: i32,
        bumpmajor: (i32, i32),
        errmajor: i32,
        bumpminor: (i32, i32),
        errminor: i32,
    ) {
        let mut steps = steps;
        if errmajor == 0 {
            loop {
                self.dpixel(x, y);
                x = x.wrapping_add(bumpmajor.0);
                y = y.wrapping_add(bumpmajor.1);
                steps -= 1;
                if steps <= 0 {
                    break;
                }
            }
        } else {
            loop {
                self.dpixel(x, y);
                if error < 0 {
                    x = x.wrapping_add(bumpmajor.0);
                    y = y.wrapping_add(bumpmajor.1);
                    error = error.wrapping_add(errmajor);
                } else {
                    x = x.wrapping_add(bumpminor.0);
                    y = y.wrapping_add(bumpminor.1);
                    error = error.wrapping_sub(errminor);
                }
                steps -= 1;
                if steps <= 0 {
                    break;
                }
            }
        }
    }

    // ─────── ProcessLine (the driver's float-layer segment entry) ───────

    /// `ProcessLine(hnd, coord1, coord2, pixelInfo)` (ProcessPath.c:1366):
    /// clips a float segment against the fractional box and dispatches to
    /// the lane's fixed-point line routine.
    fn process_line(&mut self, c1: (f32, f32), c2: (f32, f32)) {
        let (mut x1, mut y1) = c1;
        let (mut x2, mut y2) = c2;
        let x_min = self.dh.x_minf;
        let y_min = self.dh.y_minf;
        let x_max = self.dh.x_maxf;
        let y_max = self.dh.y_maxf;
        let mut res = Cres::NotClipped;
        let mut last_clipped;

        // TESTANDCLIP(yMin, yMax, y1, x1, y2, x2, jfloat, res)
        test_and_clipf(y_min, y_max, &mut y1, &mut x1, y2, x2, &mut res);
        if res == Cres::Invisible {
            return;
        }
        let mut clipped = res.is_clipped();
        // TESTANDCLIP(yMin, yMax, y2, x2, y1, x1, jfloat, res)
        test_and_clipf(y_min, y_max, &mut y2, &mut x2, y1, x1, &mut res);
        if res == Cres::Invisible {
            return;
        }
        last_clipped = res.is_clipped();
        clipped = clipped || last_clipped;

        if self.lane == Lane::Draw {
            // TESTANDCLIP(xMin, xMax, x1, y1, x2, y2, jfloat, res)
            test_and_clipf(x_min, x_max, &mut x1, &mut y1, x2, y2, &mut res);
            if res == Cres::Invisible {
                return;
            }
            clipped = clipped || res.is_clipped();
            // TESTANDCLIP(xMin, xMax, x2, y2, x1, y1, jfloat, res)
            test_and_clipf(x_min, x_max, &mut x2, &mut y2, x1, y1, &mut res);
            if res == Cres::Invisible {
                return;
            }
            last_clipped = last_clipped || res.is_clipped();
            clipped = clipped || last_clipped;

            let x1i = fi32(x1 * MDP_MULT as f32);
            let y1i = fi32(y1 * MDP_MULT as f32);
            let x2i = fi32(x2 * MDP_MULT as f32);
            let y2i = fi32(y2 * MDP_MULT as f32);
            // checkBounds = clipped (avoid OOB from the float rounding);
            // endSubPath = lastClipped (unused by the draw routine).
            self.process_fixed_line(x1i, y1i, x2i, y2i, clipped);
        } else {
            // Fill: clumping starting from the first vertex.
            let mut c = clip_clampf(x_min, x_max, (x1, y1), (x2, y2));
            x1 = c.p1.0;
            y1 = c.p1.1;
            let x1i = fi32(x1 * MDP_MULT as f32);
            let y1i = fi32(y1 * MDP_MULT as f32);

            // Clamping only by left boundary.
            if c.res == Cres::MinClipped {
                let x3i = fi32(c.p3.0 * MDP_MULT as f32);
                let y3i = fi32(c.p3.1 * MDP_MULT as f32);
                self.store_fixed_line(x3i, y3i, x1i, y1i, false, last_clipped);
            } else if c.res == Cres::Invisible {
                return;
            }

            // Clamping starting from the last vertex.
            c = clip_clampf(x_min, x_max, (x2, y2), (x1, y1));
            x2 = c.p1.0;
            y2 = c.p1.1;

            // Checking if there was a clip by the right boundary.
            last_clipped = last_clipped || c.res == Cres::MaxClipped;

            let x2i = fi32(x2 * MDP_MULT as f32);
            let y2i = fi32(y2 * MDP_MULT as f32);
            self.store_fixed_line(x1i, y1i, x2i, y2i, false, last_clipped);

            // Clamping only by left boundary.
            if c.res == Cres::MinClipped {
                let x3i = fi32(c.p3.0 * MDP_MULT as f32);
                let y3i = fi32(c.p3.1 * MDP_MULT as f32);
                self.store_fixed_line(x2i, y2i, x3i, y3i, false, last_clipped);
            }
        }
    }

    // ─────── Cubic flattening ───────

    /// `DrawMonotonicCubic` (ProcessPath.c:993): forward differencing on a
    /// monotonic cubic already bounded by the endpoint, emitting fixed
    /// point segments into `pProcessFixedLine`.
    fn draw_monotonic_cubic(&mut self, coords: &[f32; 8], check_bounds: bool) {
        let x0 = fi32(coords[0] * MDP_MULT as f32);
        let y0 = fi32(coords[1] * MDP_MULT as f32);
        let xe = fi32(coords[6] * MDP_MULT as f32);
        let ye = fi32(coords[7] * MDP_MULT as f32);

        // Fractional part of the first control point, scaled by the
        // forward-differencing precision.
        let mut px = (x0 & !MDP_W_MASK) << DF_CUB_SHIFT;
        let mut py = (y0 & !MDP_W_MASK) << DF_CUB_SHIFT;

        let mut inc_step_bnd1 = DF_CUB_INC_BND;
        let mut inc_step_bnd2 = DF_CUB_INC_BND << 1;
        let mut dec_step_bnd1 = DF_CUB_DEC_BND;
        let mut dec_step_bnd2 = DF_CUB_DEC_BND << 1;

        let mut count = DF_CUB_COUNT;
        let mut shift = DF_CUB_SHIFT;

        // Forward difference coefficients (f32 math, then (jint)).
        let ax = fi32(
            (((-coords[0] + 3.0 * coords[2]) - 3.0 * coords[4]) + coords[6])
                * CUB_A_MDP_MULT as f32,
        );
        let ay = fi32(
            (((-coords[1] + 3.0 * coords[3]) - 3.0 * coords[5]) + coords[7])
                * CUB_A_MDP_MULT as f32,
        );
        let bx =
            fi32(((3.0 * coords[0] - 6.0 * coords[2]) + 3.0 * coords[4]) * CUB_B_MDP_MULT as f32);
        let by =
            fi32(((3.0 * coords[1] - 6.0 * coords[3]) + 3.0 * coords[5]) * CUB_B_MDP_MULT as f32);
        let cx = fi32((-3.0 * coords[0] + 3.0 * coords[2]) * CUB_C_MDP_MULT as f32);
        let cy = fi32((-3.0 * coords[1] + 3.0 * coords[3]) * CUB_C_MDP_MULT as f32);

        let dddpx = 6 * ax;
        let dddpy = 6 * ay;

        let mut ddpx = dddpx + bx;
        let mut ddpy = dddpy + by;

        let mut dpx = ax + (bx >> 1) + cx;
        let mut dpy = ay + (by >> 1) + cy;

        let mut x1;
        let mut y1;

        let mut x2 = x0;
        let mut y2 = y0;

        // Whole part of the first point.
        let x0w = x0 & MDP_W_MASK;
        let y0w = y0 & MDP_W_MASK;

        let dx = wsub(xe, x0);
        let dy = wsub(ye, y0);

        while count > 0 {
            // Decreasing step ×2 while the second difference is big.
            while (ddpx.wrapping_add(dec_step_bnd1) as u32) > dec_step_bnd2 as u32
                || (ddpy.wrapping_add(dec_step_bnd1) as u32) > dec_step_bnd2 as u32
            {
                ddpx = (ddpx << 1).wrapping_sub(dddpx);
                ddpy = (ddpy << 1).wrapping_sub(dddpy);
                dpx = (dpx << 2).wrapping_sub(ddpx >> 1);
                dpy = (dpy << 2).wrapping_sub(ddpy >> 1);
                count <<= 1;
                dec_step_bnd1 <<= 3;
                dec_step_bnd2 <<= 3;
                inc_step_bnd1 <<= 3;
                inc_step_bnd2 <<= 3;
                px <<= 3;
                py <<= 3;
                shift += 3;
            }

            // Increasing step ×2 while the first difference is small
            // (only on even steps).
            while ((count & 1) ^ 1) == 1
                && shift > DF_CUB_SHIFT
                && (dpx.wrapping_add(inc_step_bnd1) as u32) <= inc_step_bnd2 as u32
                && (dpy.wrapping_add(inc_step_bnd1) as u32) <= inc_step_bnd2 as u32
            {
                dpx = (dpx >> 2) + (ddpx >> 3);
                dpy = (dpy >> 2) + (ddpy >> 3);
                ddpx = (ddpx.wrapping_add(dddpx)) >> 1;
                ddpy = (ddpy.wrapping_add(dddpy)) >> 1;
                count >>= 1;
                dec_step_bnd1 >>= 3;
                dec_step_bnd2 >>= 3;
                inc_step_bnd1 >>= 3;
                inc_step_bnd2 >>= 3;
                px >>= 3;
                py >>= 3;
                shift -= 3;
            }

            count -= 1;

            if count != 0 {
                // One step less than necessary: the last segment lands on
                // the exact endpoint, avoiding accumulated error.
                px = px.wrapping_add(dpx);
                py = py.wrapping_add(dpy);

                dpx = dpx.wrapping_add(ddpx);
                dpy = dpy.wrapping_add(ddpy);
                ddpx = ddpx.wrapping_add(dddpx);
                ddpy = ddpy.wrapping_add(dddpy);

                x1 = x2;
                y1 = y2;

                x2 = x0w + (px >> shift);
                y2 = y0w + (py >> shift);

                // Bound the monotonic coordinate by the endpoint.
                if ((xe.wrapping_sub(x2)) ^ dx) < 0 {
                    x2 = xe;
                }
                if ((ye.wrapping_sub(y2)) ^ dy) < 0 {
                    y2 = ye;
                }

                self.fd_fixed_line(x1, y1, x2, y2, check_bounds);
            } else {
                self.fd_fixed_line(x2, y2, xe, ye, check_bounds);
            }
        }
    }

    /// `ProcessMonotonicCubic` (ProcessPath.c:1159): cull/clamp against
    /// the clip box, split oversized curves in half, otherwise flatten.
    fn process_monotonic_cubic(&mut self, coords: &mut [f32; 8]) {
        let mut x_min = coords[0];
        let mut x_max = coords[0];
        let mut y_min = coords[1];
        let mut y_max = coords[1];
        // CALC_MIN/CALC_MAX over all control points.
        for k in [2usize, 4, 6] {
            if x_min > coords[k] {
                x_min = coords[k];
            }
            if x_max < coords[k] {
                x_max = coords[k];
            }
            if y_min > coords[k + 1] {
                y_min = coords[k + 1];
            }
            if y_max < coords[k + 1] {
                y_max = coords[k + 1];
            }
        }

        if self.lane == Lane::Draw {
            // Skip curves entirely out of the visible area.
            if self.dh.x_maxf < x_min
                || self.dh.x_minf > x_max
                || self.dh.y_maxf < y_min
                || self.dh.y_minf > y_max
            {
                return;
            }
        } else {
            // Fill: skip curves above, below or behind the right boundary.
            if self.dh.y_maxf < y_min || self.dh.y_minf > y_max || self.dh.x_maxf < x_min {
                return;
            }
            // Clamp x to the left boundary when fully behind it.
            if self.dh.x_minf > x_max {
                coords[0] = self.dh.x_minf;
                coords[2] = self.dh.x_minf;
                coords[4] = self.dh.x_minf;
                coords[6] = self.dh.x_minf;
            }
        }

        if x_max - x_min > MAX_CUB_SIZE || y_max - y_min > MAX_CUB_SIZE {
            // Subdivision (de Casteljau midpoint) — order-sensitive.
            let mut coords1 = [0.0f32; 8];
            coords1[6] = coords[6];
            coords1[7] = coords[7];
            coords1[4] = (coords[4] + coords[6]) / 2.0f32;
            coords1[5] = (coords[5] + coords[7]) / 2.0f32;
            let tx = (coords[2] + coords[4]) / 2.0f32;
            let ty = (coords[3] + coords[5]) / 2.0f32;
            coords1[2] = (tx + coords1[4]) / 2.0f32;
            coords1[3] = (ty + coords1[5]) / 2.0f32;
            coords[2] = (coords[0] + coords[2]) / 2.0f32;
            coords[3] = (coords[1] + coords[3]) / 2.0f32;
            coords[4] = (coords[2] + tx) / 2.0f32;
            coords[5] = (coords[3] + ty) / 2.0f32;
            coords[6] = (coords[4] + coords1[2]) / 2.0f32;
            coords1[0] = coords[6];
            coords[7] = (coords[5] + coords1[3]) / 2.0f32;
            coords1[1] = coords[7];

            self.process_monotonic_cubic(coords);
            self.process_monotonic_cubic(&mut coords1);
        } else {
            let check_bounds = self.dh.x_minf > x_min
                || self.dh.x_maxf < x_max
                || self.dh.y_minf > y_min
                || self.dh.y_maxf < y_max;
            self.draw_monotonic_cubic(coords, check_bounds);
        }
    }

    /// `ProcessFirstMonotonicPartOfCubic` (ProcessPath.c:1253): bite the
    /// head of the curve up to parameter t, then recurse on the bitten
    /// piece. Order-sensitive de Casteljau steps.
    fn bite_cubic(&mut self, coords: &mut [f32; 8], t: f32) {
        let mut coords1 = [0.0f32; 8];
        coords1[0] = coords[0];
        coords1[1] = coords[1];
        let tx = coords[2] + t * (coords[4] - coords[2]);
        let ty = coords[3] + t * (coords[5] - coords[3]);
        coords1[2] = coords[0] + t * (coords[2] - coords[0]);
        coords1[3] = coords[1] + t * (coords[3] - coords[1]);
        coords1[4] = coords1[2] + t * (tx - coords1[2]);
        coords1[5] = coords1[3] + t * (ty - coords1[3]);
        coords[4] = coords[4] + t * (coords[6] - coords[4]);
        coords[5] = coords[5] + t * (coords[7] - coords[5]);
        coords[2] = tx + t * (coords[4] - tx);
        coords[3] = ty + t * (coords[5] - ty);
        coords1[6] = coords1[4] + t * (coords[2] - coords1[4]);
        coords[0] = coords1[6];
        coords1[7] = coords1[5] + t * (coords[3] - coords1[5]);
        coords[1] = coords1[7];

        self.process_monotonic_cubic(&mut coords1);
    }

    /// `SOLVEQUADINRANGE` (ProcessPath.c:411): roots of A·t² + B·t + C inside
    /// (0, 1), appended to `params` in ascending `cnt` count.
    fn solve_quad_in_range(a: f64, b: f64, c: f64, params: &mut [f64; 4], cnt: &mut usize) {
        // Note: the macro is used only inside its `do {} while(0)` with the
        // caller's `break`s replaced by plain returns of the macro instance.
        if a != 0.0 {
            let mut d = b * b - 4.0 * a * c;
            if d < 0.0 {
                return;
            }
            d = d.sqrt();
            // One root via (-B ± d)/2A, the other via 2C/(-B ± d); pick the
            // sign making B+d maximal in magnitude.
            let d = if b < 0.0 { -d } else { d };
            let q = (b + d) / -2.0;
            let mut param = q / a;
            if param < 1.0 && param > 0.0 {
                params[*cnt] = param;
                *cnt += 1;
            }
            if d == 0.0 || q == 0.0 {
                return;
            }
            param = c / q;
            if param < 1.0 && param > 0.0 {
                params[*cnt] = param;
                *cnt += 1;
            }
        } else {
            // B·t + C = 0
            if b == 0.0 {
                return;
            }
            let param = -c / b;
            if param < 1.0 && param > 0.0 {
                params[*cnt] = param;
                *cnt += 1;
            }
        }
    }

    /// `ProcessCubic` (ProcessPath.c:1284): split into monotonic pieces at
    /// the interior extremup parameters, then flatten each.
    fn process_cubic(&mut self, coords: &mut [f32; 8]) {
        let mut params = [0.0f64; 4];
        let mut cnt = 0usize;

        // Simple monotonicity pre-check in X.
        if (coords[0] > coords[2] || coords[2] > coords[4] || coords[4] > coords[6])
            && (coords[0] < coords[2] || coords[2] < coords[4] || coords[4] < coords[6])
        {
            // Extrema of X(t) via dX/dt = 0 (C: f32 expression, widened).
            let ax = (-coords[0] + 3.0 * coords[2] - 3.0 * coords[4] + coords[6]) as f64;
            let bx = (2.0 * (coords[0] - 2.0 * coords[2] + coords[4])) as f64;
            let cx = (-coords[0] + coords[2]) as f64;
            Self::solve_quad_in_range(ax, bx, cx, &mut params, &mut cnt);
        }

        // Monotonicity pre-check in Y.
        if (coords[1] > coords[3] || coords[3] > coords[5] || coords[5] > coords[7])
            && (coords[1] < coords[3] || coords[3] < coords[5] || coords[5] < coords[7])
        {
            let ay = (-coords[1] + 3.0 * coords[3] - 3.0 * coords[5] + coords[7]) as f64;
            let by = (2.0 * (coords[1] - 2.0 * coords[3] + coords[5])) as f64;
            let cy = (-coords[1] + coords[3]) as f64;
            Self::solve_quad_in_range(ay, by, cy, &mut params, &mut cnt);
        }

        if cnt > 0 {
            // Insertion sort of the (up to 4) parameters.
            for i in 1..cnt {
                let value = params[i];
                let mut j = i;
                while j > 0 && params[j - 1] > value {
                    params[j] = params[j - 1];
                    j -= 1;
                }
                params[j] = value;
            }

            // Bite each monotonic piece in turn.
            self.bite_cubic(coords, params[0] as f32);
            for i in 1..cnt {
                let param = params[i] - params[i - 1];
                if param > 0.0 {
                    // Scale the parameter to the rest of the curve.
                    self.bite_cubic(coords, (param / (1.0 - params[i - 1])) as f32);
                }
            }
        }

        self.process_monotonic_cubic(coords);
    }

    // ─────── ProcessPath driver ───────

    /// `ProcessPath` (ProcessPath.c:1460): the segment machine. Consumes
    /// the `Path2D.Float` arrays (translation 0 — the applet's canvases
    /// are identity transforms). Returns false only on malformed type
    /// coordinate indexing, which well-formed paths never hit.
    fn process_path(&mut self, path: &FlatPath) -> bool {
        let coords = &path.coords;
        let mut t_coords = [0.0f32; 8];
        let mut close_coord = [0.0f32; 2];
        self.pix[0] = 0;
        let mut skip = false;
        let mut subpath_started = false;
        let mut last_x;
        let mut last_y;
        let mut index = 0usize;

        // STROKE_DEFAULT (`pixels at corners`): closeCoord = (0, 0).
        close_coord[0] = 0.0;
        close_coord[1] = 0.0;

        for &seg_type in &path.types {
            match seg_type {
                SEG_MOVETO => {
                    if index + 2 <= coords.len() {
                        // Close the previous subpath first. (The C nests
                        // lane/coord ifs; collapsed here, same order.)
                        if subpath_started && !skip {
                            if self.lane == Lane::Fill
                                && (t_coords[0] != close_coord[0] || t_coords[1] != close_coord[1])
                            {
                                self.process_line(
                                    (t_coords[0], t_coords[1]),
                                    (close_coord[0], close_coord[1]),
                                );
                            }
                            self.fd_end_sub_path();
                        }

                        t_coords[0] = coords[index] + 0.0;
                        index += 1;
                        t_coords[1] = coords[index] + 0.0;
                        index += 1;

                        // Skip MOVETOs outside [LOWER_BND, UPPER_BND].
                        if t_coords[0] < UPPER_BND
                            && t_coords[0] > LOWER_BND
                            && t_coords[1] < UPPER_BND
                            && t_coords[1] > LOWER_BND
                        {
                            subpath_started = true;
                            skip = false;
                            close_coord[0] = t_coords[0];
                            close_coord[1] = t_coords[1];
                        } else {
                            skip = true;
                        }
                    } else {
                        return false;
                    }
                }
                SEG_LINETO => {
                    if index + 2 <= coords.len() {
                        // lastX = tCoords[2] = coords[index++] + transXf;
                        t_coords[2] = coords[index] + 0.0;
                        last_x = t_coords[2];
                        index += 1;
                        // lastY = tCoords[3] = coords[index++] + transYf;
                        t_coords[3] = coords[index] + 0.0;
                        last_y = t_coords[3];
                        index += 1;
                        // Validity check (also rejects NaN/Infinity).
                        if last_x < UPPER_BND
                            && last_x > LOWER_BND
                            && last_y < UPPER_BND
                            && last_y > LOWER_BND
                        {
                            if skip {
                                // Restart the subpath at this valid point.
                                t_coords[0] = last_x;
                                close_coord[0] = last_x;
                                t_coords[1] = last_y;
                                close_coord[1] = last_y;
                                subpath_started = true;
                                skip = false;
                            } else {
                                self.process_line(
                                    (t_coords[0], t_coords[1]),
                                    (t_coords[2], t_coords[3]),
                                );
                                t_coords[0] = last_x;
                                t_coords[1] = last_y;
                            }
                        }
                    } else {
                        return false;
                    }
                }
                SEG_CUBICTO => {
                    if index + 6 <= coords.len() {
                        t_coords[2] = coords[index] + 0.0;
                        index += 1;
                        t_coords[3] = coords[index] + 0.0;
                        index += 1;
                        t_coords[4] = coords[index] + 0.0;
                        index += 1;
                        t_coords[5] = coords[index] + 0.0;
                        index += 1;
                        last_x = coords[index] + 0.0;
                        t_coords[6] = last_x;
                        index += 1;
                        last_y = coords[index] + 0.0;
                        t_coords[7] = last_y;
                        index += 1;

                        if last_x < UPPER_BND
                            && last_x > LOWER_BND
                            && last_y < UPPER_BND
                            && last_y > LOWER_BND
                        {
                            if skip {
                                // Restart with the valid endpoint.
                                t_coords[0] = t_coords[6];
                                close_coord[0] = t_coords[6];
                                t_coords[1] = t_coords[7];
                                close_coord[1] = t_coords[7];
                                subpath_started = true;
                                skip = false;
                            } else {
                                if t_coords[2] < UPPER_BND
                                    && t_coords[2] > LOWER_BND
                                    && t_coords[3] < UPPER_BND
                                    && t_coords[3] > LOWER_BND
                                    && t_coords[4] < UPPER_BND
                                    && t_coords[4] > LOWER_BND
                                    && t_coords[5] < UPPER_BND
                                    && t_coords[5] > LOWER_BND
                                {
                                    self.process_cubic(&mut t_coords);
                                } else {
                                    // Degenerate controls: straight line.
                                    self.process_line(
                                        (t_coords[0], t_coords[1]),
                                        (t_coords[6], t_coords[7]),
                                    );
                                }
                                t_coords[0] = last_x;
                                t_coords[1] = last_y;
                            }
                        }
                    } else {
                        return false;
                    }
                }
                SEG_CLOSE => {
                    if subpath_started && !skip {
                        skip = false;
                        if t_coords[0] != close_coord[0] || t_coords[1] != close_coord[1] {
                            self.process_line(
                                (t_coords[0], t_coords[1]),
                                (close_coord[0], close_coord[1]),
                            );
                            // For following segments without a new moveTo.
                            t_coords[0] = close_coord[0];
                            t_coords[1] = close_coord[1];
                        }
                        self.fd_end_sub_path();
                    }
                }
                // SEG_QUADTO(2) cannot occur in AWT geometry built from
                // rects/round rects/lines/polygons/ellipses/arcs.
                _ => unreachable!("SEG_QUADTO is not reachable from AWT geometry"),
            }
        }

        // Close an unclosed trailing subpath.
        if subpath_started && !skip {
            if self.lane == Lane::Fill
                && (t_coords[0] != close_coord[0] || t_coords[1] != close_coord[1])
            {
                self.process_line((t_coords[0], t_coords[1]), (close_coord[0], close_coord[1]));
            }
            self.fd_end_sub_path();
        }

        true
    }

    // ─────── FillPolygon ───────

    /// `FillPolygon` (ProcessPath.c:1946) with `WIND_NON_ZERO`
    /// (`counterMask = -1`).
    fn fill_polygon(&mut self) {
        let n = self.pts.len();
        if n <= 1 {
            return;
        }
        let pts: Vec<(i32, i32)> = self.pts.iter().map(|p| (p.x, p.y)).collect();
        let last_flags: Vec<bool> = self.pts.iter().map(|p| p.last_point).collect();
        let plg_ymin = self.fd_ymin;
        let plg_ymax = self.fd_ymax;
        let dh_x_min = self.dh.b.x_min;
        let right_bnd = self.dh.b.x_max - 1;

        let hash_offset = (plg_ymin - 1) & MDP_W_MASK;
        let hash_size = ((plg_ymax - plg_ymin) >> MDP_PREC) + 4;

        // Points between the same pair of scanlines share a bucket; the
        // C follows the LIFO `nextByY` chain, i.e. reverse insertion.
        let mut y_hash: Vec<Vec<usize>> = vec![Vec::new(); hash_size.max(1) as usize];
        for (i, &(_, py)) in pts.iter().enumerate() {
            let bucket = ((py - hash_offset - 1) >> MDP_PREC) as usize;
            y_hash[bucket].push(i);
        }

        let mut edges: Vec<Edge> = Vec::with_capacity(n);
        let mut pt_edge: Vec<isize> = vec![-1; n];
        let mut head: isize = -1;
        let mut nact = 0usize;

        let mut y = hash_offset + MDP_MULT;
        let mut k = 0i32;
        while y <= plg_ymax && k < hash_size {
            for &i in y_hash[k as usize].iter().rev() {
                // Degenerate-edge bookkeeping for the segment (pt->prev, pt).
                if i > 0 && !last_flags[i - 1] {
                    let pv_y = pts[i - 1].1;
                    let pe = pt_edge[i - 1];
                    if pe >= 0 && pv_y <= y {
                        delete_active(&mut edges, &mut head, pe);
                        pt_edge[i - 1] = -1;
                    } else if pv_y > y {
                        insert_active(
                            &mut edges,
                            &mut head,
                            &mut pt_edge,
                            &mut nact,
                            i - 1,
                            i,
                            y,
                            &pts,
                        );
                    }
                }
                // ... and for the segment (pt, pt->next).
                if !last_flags[i] && i + 1 < n {
                    let nx_y = pts[i + 1].1;
                    let pe = pt_edge[i];
                    if pe >= 0 && nx_y <= y {
                        delete_active(&mut edges, &mut head, pe);
                        pt_edge[i] = -1;
                    } else if nx_y > y {
                        insert_active(
                            &mut edges,
                            &mut head,
                            &mut pt_edge,
                            &mut nact,
                            i,
                            i + 1,
                            y,
                            &pts,
                        );
                    }
                }
            }

            if head >= 0 {
                // Bubble sort by edge x (combats the almost-sorted list).
                active_bubble_sort(&mut edges, &mut head);
                // Correct the back links of the double linked list.
                let mut prev: isize = -1;
                let mut cur = head;
                while cur >= 0 {
                    edges[cur as usize].prev = prev;
                    prev = cur;
                    cur = edges[cur as usize].next;
                }

                // Winding walk over the spans of this scanline.
                let mut xl = dh_x_min;
                let mut xr;
                let mut counter = 0i32;
                let mut drawing = false;
                let mut cur_edge = head;
                while cur_edge >= 0 {
                    counter += edges[cur_edge as usize].dir;
                    // counterMask = -1 → any non-zero winding ...
                    if counter != 0 && !drawing {
                        xl = (edges[cur_edge as usize].x + MDP_MULT - 1) >> MDP_PREC;
                        drawing = true;
                    }
                    if counter == 0 && drawing {
                        xr = (edges[cur_edge as usize].x - 1) >> MDP_PREC;
                        if xl <= xr {
                            self.scan_span(xl, xr, y >> MDP_PREC);
                        }
                        drawing = false;
                    }
                    // Every edge advances regardless of its state.
                    let dx = edges[cur_edge as usize].dx;
                    edges[cur_edge as usize].x = edges[cur_edge as usize].x.wrapping_add(dx);
                    cur_edge = edges[cur_edge as usize].next;
                }

                // Run-off to the right boundary (correct rendering of
                // shapes clipped at the right side).
                if drawing && xl <= right_bnd {
                    self.scan_span(xl, right_bnd, y >> MDP_PREC);
                }
            }

            y += MDP_MULT;
            k += 1;
        }
    }

    #[inline]
    fn scan_span(&mut self, x0: i32, x1: i32, y: i32) {
        self.sink.span(x0, x1, y);
    }
}

/// `INSERT_ACTIVE` (ProcessPath.c:1899). Argument order mirrors the C
/// macro's parameters.
#[allow(clippy::too_many_arguments)]
fn insert_active(
    edges: &mut Vec<Edge>,
    head: &mut isize,
    pt_edge: &mut [isize],
    nact: &mut usize,
    pnt_idx: usize,
    np_idx: usize,
    cy: i32,
    pts: &[(i32, i32)],
) {
    // Skipping horizontal segments.
    if pts[pnt_idx].1 == pts[np_idx].1 {
        return;
    }
    let d_x = wsub(pts[np_idx].0, pts[pnt_idx].0);
    let d_y = wsub(pts[np_idx].1, pts[pnt_idx].1);
    let dir;
    let mut x;
    let dy;
    if pts[pnt_idx].1 < pts[np_idx].1 {
        // Edge descends screen-wise from `pnt`: insert reaching down.
        dir = -1;
        x = pts[pnt_idx].0;
        dy = cy - pts[pnt_idx].1;
    } else {
        // Edge ascends: its top is `np`.
        dir = 1;
        x = pts[np_idx].0;
        dy = cy - pts[np_idx].1;
    }
    let dx;
    if i32_abs(d_x) > CALC_BND {
        dx = ((d_x as f64) * (MDP_MULT as f64) / (d_y as f64)) as i32;
        x += ((d_x as f64) * (dy as f64) / (d_y as f64)) as i32;
    } else {
        dx = wdiv(d_x << MDP_PREC, d_y);
        x = x.wrapping_add(wdiv(wmul(d_x, dy), d_y));
    }
    edges.push(Edge {
        x,
        dx,
        dir,
        prev: -1,
        next: *head,
    });
    let new_idx = edges.len() as isize - 1;
    if *head >= 0 {
        edges[*head as usize].prev = new_idx;
    }
    *head = new_idx;
    pt_edge[pnt_idx] = new_idx;
    *nact += 1;
}

/// `DELETE_ACTIVE` (ProcessPath.c:1885).
fn delete_active(edges: &mut [Edge], head: &mut isize, e: isize) {
    let prevp = edges[e as usize].prev;
    let nextp = edges[e as usize].next;
    if prevp >= 0 {
        edges[prevp as usize].next = nextp;
    } else {
        *head = nextp;
    }
    if nextp >= 0 {
        edges[nextp as usize].prev = prevp;
    }
}

/// `LBUBBLE_SORT` (ProcessPath.c:1843) — stable-shape-agnostic bubble
/// sort over the active list, swapping on `>=`.
fn active_bubble_sort(edges: &mut [Edge], head: &mut isize) {
    let mut s: isize = -1;
    let mut was_swap = true;
    // r precedes p; s bounds how far comparisons run.
    loop {
        let next_head = if *head >= 0 {
            edges[*head as usize].next
        } else {
            -1
        };
        if s == next_head || !was_swap {
            break;
        }
        let mut r = *head;
        let mut p = *head;
        let mut q = edges[p as usize].next;
        was_swap = false;
        while p != s {
            if edges[p as usize].x >= edges[q as usize].x {
                was_swap = true;
                // Swap p and q in the linked list.
                let temp = edges[q as usize].next;
                edges[q as usize].next = p;
                edges[p as usize].next = temp;
                if p == *head {
                    *head = q;
                } else {
                    edges[r as usize].next = q;
                }
                r = q;
            } else {
                r = p;
                p = edges[p as usize].next;
            }
            q = edges[p as usize].next;
            if q == s {
                s = p;
            }
        }
    }
}

// ════════════════════════════ Entries ════════════════════════════

/// `FillPath` (FillPath.c): `ProcessPath` then `FillPolygon`. Winding
/// rule is WIND_NON_ZERO for every `Path2D.Float` an AWT shape builds.
pub fn fill_path<S: PixelSink>(sink: &mut S, path: &FlatPath, w: i32, h: i32) {
    let mut eng = Engine::new(Lane::Fill, sink, w, h);
    if eng.process_path(path) {
        eng.fill_polygon();
    }
}

/// `DrawPath` (DrawPath.c): `ProcessPath` with the draw-lane line/pixel
/// primitives.
pub fn draw_path<S: PixelSink>(sink: &mut S, path: &FlatPath, w: i32, h: i32) {
    let mut eng = Engine::new(Lane::Draw, sink, w, h);
    let _ = eng.process_path(path);
}

/// `Graphics.drawLine(x1, y1, x2, y2)` on an untransformed INT_RGB
/// surface — the `sun.java2d.loops.DrawLine` native: outcode-clipped
/// `SetupBresenham` + the fused-minor Bresenham executor (`DrawLine.c`,
/// shorten = 0). Note `drawLine` does NOT route through `ProcessPath`.
pub fn draw_line<S: PixelSink>(sink: &mut S, x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) {
    let mut eng = Engine::new(Lane::Draw, sink, w, h);
    eng.draw_line_int(x1, y1, x2, y2);
}

// ────────────────────────── test helpers ──────────────────────────

#[cfg(test)]
mod test_support {
    use crate::hash::md5;

    /// The pixel-int stream a JDK `PixelGrabber`/`getRGB` dump hashes:
    /// BE (0xFF, R, G, B) per pixel over the whole surface.
    pub fn argb_stream(cv: &crate::canvas::Canvas) -> Vec<u8> {
        let mut b = Vec::with_capacity(cv.w * cv.h * 4);
        for px3 in cv.px.as_chunks::<3>().0 {
            b.push(0xFF);
            b.push(px3[0]);
            b.push(px3[1]);
            b.push(px3[2]);
        }
        b
    }

    pub fn hex(b: &[u8; 16]) -> String {
        b.iter().map(|v| format!("{:02x}", v)).collect()
    }

    pub fn argb_md5(cv: &crate::canvas::Canvas) -> [u8; 16] {
        md5(&argb_stream(cv))
    }
}

// ════════════════════════════ Tests ════════════════════════════

#[cfg(test)]
mod tests {
    use super::round_rect_path;
    use super::test_support::{argb_md5, hex};
    use crate::canvas::Canvas;

    /// MD5 over the full 550×400 opaque-black canvas, rendered through
    /// `Canvas::fill_round_rect` / `draw_round_rect` / `draw_line`.
    #[test]
    fn fill_round_rect_matches_ob40() {
        const F: [[i32; 6]; 20] = [
            [125, 315, 300, 80, 30, 70], // loading/clicknow panel geometry
            [10, 10, 110, 22, 7, 20],
            [10, 10, 196, 22, 7, 20],
            [10, 10, 85, 22, 7, 20],
            [10, 10, 22, 22, 7, 7],
            [10, 10, 20, 20, 40, 40],
            [10, 10, 13, 9, 20, 4],
            [10, 10, 11, 11, 1, 1],
            [10, 10, 9, 7, 50, 1],
            [10, 10, 1, 1, 0, 0],
            [10, 10, 2, 2, 5, 2],
            [10, 10, 17, 13, 3, 13],
            [10, 10, 300, 80, 0, 0],
            [10, 10, 300, 80, 30, 70],
            [10, 10, 50, 30, 25, 15],
            [10, 10, 31, 20, 15, 10],
            [10, 10, 8, 8, 20, 20],
            [10, 10, 40, 40, 30, 30],
            [10, 10, 100, 40, 10, 10],
            [10, 10, 46, 26, 20, 15],
        ];
        const WANT: [&str; 20] = [
            "6bdf79294ce5045e9a34cfea672e81f6",
            "a047b9820f14f4a2281f784d7c3a566a",
            "19c8dc371ad1929a5f94b27b8d5a55f4",
            "1864629ea4a5010c124017a29f72ae2e",
            "ebff41887672127edb87a27e47acfbc4",
            "4c608749a5f3d4ac2a1fe28270d7592e",
            "57b1f675029d093e573cc373b91ef60e",
            "9ae8071d741e73b9ac9f18be5efa37e0",
            "41710441bdc0d816a85704b96c22f5ea",
            "e95cb920ed8dd4f9730b7d06834dd2ba",
            "90cff28d515004a8579b932a9842c37f",
            "3f7b2852f60b15709b511eaefe31efab",
            "decb7e4a61720b391a3eb9e4d97d37fd",
            "c5131935f9f8d471e07d5e61066480e1",
            "c73b3e21515feb9de8da3e95a54acfd3",
            "f586f3e727e8231fa4687de00a9f29e1",
            "2abc57af3e1604d48fc66f716d9fcc8f",
            "a29e446aef77525b953d9f66864aba4b",
            "1d618901beca6f4f8140f685c1df21b3",
            "58c9482f688006295e10e840909e499f",
        ];
        assert_eq!(F.len(), WANT.len());
        for (i, s) in F.iter().enumerate() {
            let mut cv = Canvas::new(550, 400);
            cv.fill_round_rect(s[0], s[1], s[2], s[3], s[4], s[5], [0xCC, 0x00, 0x00]);
            assert_eq!(hex(&argb_md5(&cv)), WANT[i], "F{} mismatch", i);
        }
    }

    /// MD5 over the full 550×400 opaque-black canvas, rendered through
    /// `Canvas::draw_round_rect` / `draw_line`.
    #[test]
    fn draw_round_rect_matches_ob40() {
        const D: [[i32; 6]; 14] = [
            [10, 10, 110, 22, 7, 20],
            [10, 10, 196, 22, 7, 20],
            [10, 10, 85, 22, 7, 20],
            [10, 10, 300, 80, 30, 70],
            [10, 10, 22, 22, 7, 7],
            [10, 10, 5, 5, 10, 5],
            [10, 10, 1, 1, 0, 0],
            [10, 10, 2, 2, 3, 3],
            [10, 10, 9, 7, 50, 1],
            [10, 10, 17, 13, 3, 13],
            [10, 10, 50, 30, 25, 15],
            [10, 10, 8, 8, 20, 20],
            [10, 10, 40, 40, 30, 30],
            [10, 10, 31, 20, 15, 10],
        ];
        const WANT: [&str; 14] = [
            "5da25def9007e5a724652f954f0eac12",
            "23783b11d4adb9e20eb71495e037aad0",
            "6d000d07a0a6a9ffe0f6ff6e94c2c333",
            "4550c6374c97543f774c1afa3311c8c3",
            "5803aab443bed7b0d71fb800064273f8",
            "b262c811cd3c3be8ce308049dd5f34a8",
            "eaad1bbb3e80a15d11480a4f337c857b",
            "88371c0a2b79c8453dc7303775be9386",
            "4f12f179b4802f9153201146553549d8",
            "377ca8537ae7a27e712278b33585c947",
            "c6bf8264fa4c08951d193ddf94bcde12",
            "95223b0b7b3315ed471f500857a49e56",
            "3d53f5427e7a39fa04dd63dfae0d5eef",
            "4645e522b7b9d6b2e8e455b9bec40ecc",
        ];
        assert_eq!(D.len(), WANT.len());
        for (i, s) in D.iter().enumerate() {
            let mut cv = Canvas::new(550, 400);
            cv.draw_round_rect(s[0], s[1], s[2], s[3], s[4], s[5], [0x00, 0x00, 0xCC]);
            assert_eq!(hex(&argb_md5(&cv)), WANT[i], "D{} mismatch", i);
        }
    }

    /// `Graphics.drawLine` pins — the `DrawLine` native path.
    #[test]
    fn draw_line_matches_ob40() {
        const L: [[i32; 4]; 15] = [
            [20, 20, 30, 30],
            [20, 20, 30, 25],
            [20, 20, 25, 30],
            [20, 20, 40, 27],
            [20, 20, 27, 40],
            [20, 20, 43, 31],
            [30, 30, 20, 20],
            [20, 20, 12, 28],
            [20, 20, 28, 12],
            [20, 20, 31, 27],
            [20, 20, 21, 24],
            [20, 20, 24, 21],
            [20, 20, 23, 30],
            [20, 20, 30, 23],
            [20, 20, 10, 40],
        ];
        const WANT: [&str; 15] = [
            "0da3a4cb3dc7053ca384c4a5c5507cd2",
            "9046b52911c87b47dce16b663a9c79b6",
            "811d172bce0d65912ceabd9fadc057fa",
            "edcde49dbc81df95b38aed194f51cf10",
            "ef79583f070c5391dff84f1ef622465f",
            "727f746d1569ab08b73cede439af5415",
            "0da3a4cb3dc7053ca384c4a5c5507cd2",
            "491d39005cf80fbed2a5d33e5969085f",
            "f35ff89f7a3bce7ec4e65510667c8fa1",
            "664159e7ef0f06ba18688970926f188d",
            "d4e596b94f7580d30e041bdd45e7862a",
            "b63c04b09a361047149fb8edbc2aae58",
            "c2e93d41549e95b14debe0f81541a841",
            "a7e6a76049cd73c8cb95cd4ebee99c2d",
            "87a94809ddea075c8dfe4694c78e78a6",
        ];
        assert_eq!(L.len(), WANT.len());
        for (i, s) in L.iter().enumerate() {
            let mut cv = Canvas::new(550, 400);
            cv.draw_line(s[0], s[1], s[2], s[3], [0x00, 0x00, 0xCC]);
            assert_eq!(hex(&argb_md5(&cv)), WANT[i], "L{} mismatch", i);
        }
    }

    /// Path construction rejects the inverted extent the iterator
    /// produces an empty path for (Java's `Graphics` clips such calls to
    /// a no-op).
    #[test]
    fn inverted_round_rect_is_empty() {
        assert!(round_rect_path(10, 10, -5, 20, 3, 7).is_none());
        assert!(round_rect_path(10, 10, 40, -5, 3, 7).is_none());
        assert!(round_rect_path(10, 10, 40, 40, 30, 70).is_some());
    }
}
