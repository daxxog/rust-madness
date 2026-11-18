// FIDELITY: Plane.java painter: indexed `for (int ch/k...)` loops, the Java ctor/draw arity and Math.max/min clamps are faithful.
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::manual_clamp
)]
//! Port of `Plane.java` — one polygon of a `Conto` model and its painter:
//! model→camera transform (`rot`), projection (`xs`/`ys`), the `d()` fill
//! path (visibility cull, HSB lighting by `projf/deltaf`, distance fog, the
//! painter's `av` distance, outline styles incl. road edges and glass
//! frames), the embos wobble/broken-panel branch and the chip spark debris
//! (both inside `d()`), and the `s()` ground/tracker shadow projector.

use crate::canvas::Canvas;
use crate::color;
use crate::medium::Medium;
use crate::trackers::Trackers;

/// ContO and Plane both define the identical rotation helper (ContO.java:
/// 649-658, Plane.java:1016-1025); it lives here once.
pub fn rot(m: &Medium, a: &mut [i32], b: &mut [i32], px: i32, py: i32, angle: i32) {
    if angle != 0 {
        let n = a.len().min(b.len());
        for i in 0..n {
            let x = a[i];
            let y = b[i];
            // note the single (int) cast around the whole subtraction, as in
            // the original: px + (int)((x-px)*cos - (y-py)*sin)
            a[i] = px + ((x - px) as f32 * m.cos(angle) - (y - py) as f32 * m.sin(angle)) as i32;
            b[i] = py + ((x - px) as f32 * m.sin(angle) + (y - py) as f32 * m.cos(angle)) as i32;
        }
    }
}

pub(crate) fn clampc(c: [i32; 3]) -> [u8; 3] {
    [
        c[0].clamp(0, 255) as u8,
        c[1].clamp(0, 255) as u8,
        c[2].clamp(0, 255) as u8,
    ]
}

pub struct Plane {
    pub ox: Vec<i32>,
    pub oy: Vec<i32>,
    pub oz: Vec<i32>,
    pub c: [i32; 3],
    pub oc: [i32; 3],
    pub hsb: [f32; 3],
    pub glass: bool,
    pub gr: i32,
    pub disline: i32,
    pub road: bool,
    /// Only wheel cap planes carry master 1..wheels (Wheels.make).
    pub master: i32,
    /// Spin pivots (wheel center within the model).
    pub wx: i32,
    pub wy: i32,
    pub wz: i32,
    pub deltaf: f32,
    pub projf: f32,
    /// Painter's distance computed in the previous `d()` pass.
    pub av: i32,
    pub bfase: i32,
    pub nocol: bool,
    // debris/ordnance state — pending ports, asserted 0 in d()/s()
    pub embos: i32,
    pub chip: i32,
    pub ctmag: f32,
    pub cxz: i32,
    pub cxy: i32,
    pub czy: i32,
    pub cox: [i32; 3],
    pub coz: [i32; 3],
    pub coy: [i32; 3],
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    pub vx: i32,
    pub vy: i32,
    pub vz: i32,
    /// Flat-axis heuristic: 1 = x-major, 2 = y-major, 3 = z-major.
    pub typ: i32,
    pub pa: usize,
    pub pb: usize,
    pub flx: i32,
}

impl Plane {
    /// Port of the `Plane` constructor (Plane.java:81-224). `n` is the
    /// vertex-array length like the original; colors are re-derived against
    /// the current `Medium` snap.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
        // No n >= 3 precondition: Plane.java:101-109 allocates n slots and
        // only then reads ox[2] at :111-113, so a short plane fails with an
        // index error in the original rather than an assert here (TPlane's
        // `err_n2_ox2_aioobe`).
        let mut p = Plane {
            ox: ox[..n].to_vec(),
            oy: oy[..n].to_vec(),
            oz: oz[..n].to_vec(),
            c: [0; 3],
            oc,
            hsb: [0.0; 3],
            glass,
            gr,
            disline,
            road,
            master: 0,
            wx,
            wy,
            wz,
            deltaf: 1.0,
            projf: 1.0,
            av: 0,
            bfase,
            nocol: false,
            embos: 0,
            chip: 0,
            ctmag: 0.0,
            cxz: 0,
            cxy: 0,
            czy: 0,
            cox: [0; 3],
            coz: [0; 3],
            coy: [0; 3],
            dx: 0,
            dy: 0,
            dz: 0,
            vx: 0,
            vy: 0,
            vz: 0,
            typ: 0,
            pa: 0,
            pb: 0,
            flx: 0,
        };

        // Plane.java:111-124 — flat-axis heuristic from vertex 1 vs 2 spans
        let len2_x = (p.ox[2] - p.ox[1]).abs();
        let len2_y = (p.oy[2] - p.oy[1]).abs();
        let len2_z = (p.oz[2] - p.oz[1]).abs();
        if len2_y <= len2_x && len2_y <= len2_z {
            p.typ = 2;
        }
        if len2_x <= len2_y && len2_x <= len2_z {
            p.typ = 1;
        }
        if len2_z <= len2_x && len2_z <= len2_y {
            p.typ = 3;
        }

        if oc[0] == oc[1] && oc[1] == oc[2] {
            p.nocol = true;
        }

        if !glass {
            for k in 0..3 {
                let mut v = (oc[k] as f32 + oc[k] as f32 * (m.snap[k] as f32 / 100.0)) as i32;
                v = v.clamp(0, 255);
                p.c[k] = v;
            }
        } else {
            for k in 0..3 {
                p.c[k] = (m.csky[k] * m.fade[0] * 2 + m.cfade[k] * 3000) / (m.fade[0] * 2 + 3000);
            }
        }

        p.hsb = color::rgb_to_hsb(p.c[0], p.c[1], p.c[2]);

        // Plane.java:163-194 — brighten-phase color rules
        if !p.nocol && !p.glass {
            if bfase > 20 && p.hsb[1] > 0.2 {
                p.hsb[1] = 0.2;
            }
            if bfase > 30 {
                if p.hsb[2] < 0.5 {
                    p.hsb[2] = 0.5;
                }
                if p.hsb[1] > 0.1 {
                    p.hsb[1] = 0.1;
                }
            }
            if bfase > 40 {
                p.hsb[1] = 0.05;
            }
            if bfase > 50 {
                if p.hsb[2] > 0.8 {
                    p.hsb[2] = 0.8;
                }
                p.hsb[0] = 0.075;
                p.hsb[1] = 0.05;
            }
            if bfase > 60 {
                p.hsb[0] = 0.05;
            }
        }

        // Plane.java:201-223 — deltaf scaling: product of distances between
        // the first three vertices, / 3
        for i in 0..3 {
            for j in 0..3 {
                if j != i {
                    let dx = p.ox[i] - p.ox[j];
                    let dy = p.oy[i] - p.oy[j];
                    let dz = p.oz[i] - p.oz[j];
                    // the int squares and their sum overflow-wrap in Java
                    // (Plane.java:212-214); the wrapped sum is widened, not
                    // the terms
                    let d2 = dx
                        .wrapping_mul(dx)
                        .wrapping_add(dy.wrapping_mul(dy))
                        .wrapping_add(dz.wrapping_mul(dz)) as f64;
                    p.deltaf *= (d2.sqrt() / 100.0) as f32;
                }
            }
        }
        p.deltaf /= 3.0;

        p
    }

    /// Port of `Plane.loadprojf` (Plane.java:47-71) — keeps the reference
    /// projection factor synced to the current (rotated) coords.
    pub fn loadprojf(&mut self) {
        self.projf = 1.0;
        for i in 0..3 {
            for j in 0..3 {
                if j != i {
                    let dx = self.ox[i] - self.ox[j];
                    let dz = self.oz[i] - self.oz[j];
                    let d2 = dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz)) as f64;
                    self.projf *= (d2.sqrt() / 100.0) as f32;
                }
            }
        }
        self.projf /= 3.0;
    }

    /// `Plane.xs` (Plane.java:1027-1033): depth clamped at `m.cz`; the
    /// product/add overflow-wrap in Java (far flyover depths exceed
    /// i32::MAX), so use wrapping ops for parity.
    pub fn xs(&self, m: &Medium, v: i32, d: i32) -> i32 {
        let d = if d < m.cz { m.cz } else { d };
        (d - m.focus_point)
            .wrapping_mul(m.cx - v)
            .wrapping_div(d)
            .wrapping_add(v)
    }

    /// `Plane.ys` (Plane.java:73-79): depth clamped at `m.cz`; same
    /// wrapping-parity note as `xs`.
    pub fn ys(&self, m: &Medium, v: i32, d: i32) -> i32 {
        let d = if d < m.cz { m.cz } else { d };
        (d - m.focus_point)
            .wrapping_mul(m.cy - v)
            .wrapping_div(d)
            .wrapping_add(v)
    }

    /// `Plane.spy` (Plane.java:1174-1176).
    pub fn spy(&self, m: &Medium, x: i32, z: i32) -> i32 {
        let dx = (x - m.cx) as f64;
        (dx * dx + z as f64 * z as f64).sqrt() as i32
    }

    /// Port of `Plane.d(...)` (Plane.java:226-1014) — the per-plane painter.
    /// Args mirror the ContO.d call (ContO.java:605):
    /// `(g, x-rel, y-rel, z-rel, axz, axy, azy, wxz, wzy, noline)` where
    /// `noline` doubles as the "darkened/no-outline" flag for far/near
    /// planes.
    #[allow(clippy::too_many_arguments)]
    pub fn d(
        &mut self,
        cv: &mut Canvas,
        m: &mut Medium,
        px: i32,
        py: i32,
        pz: i32,
        axz: i32,
        axy: i32,
        azy: i32,
        wxz: i32,
        wzy: i32,
        mut shade: bool,
    ) {
        let n = self.ox.len();

        // Plane.java:230-549 — pristine vertex load vs the embos wobble
        // branch; the branch also paints its flicker/broken-panel debris
        // and returns the typ-scaled reload the main body runs on.
        let (mut vx, mut vy, mut vz) = if self.embos == 0 {
            (
                (0..n).map(|i| self.ox[i] + px).collect(),
                (0..n).map(|i| self.oy[i] + py).collect(),
                (0..n).map(|i| self.oz[i] + pz).collect(),
            )
        } else {
            self.d_embos(cv, m, px, py, pz, axz, axy, azy, n)
        };

        // wheel spin pivots (Plane.java:505-511)
        if self.wz != 0 {
            rot(m, &mut vy, &mut vz, self.wy + py, self.wz + pz, wzy);
        }
        if self.wx != 0 {
            rot(m, &mut vx, &mut vz, self.wx + px, self.wz + pz, wxz);
        }

        // Plane.java:513-556 — chip spark decay + the live debris overlay,
        // drawn on the chip's own stored pose angles (before the model
        // pose rotations, as in the original).
        self.d_chip(cv, m, px, py, pz, axz, axy, azy);

        // model pose rotation (Plane.java:627-629)
        rot(m, &mut vx, &mut vy, px, py, axy);
        rot(m, &mut vy, &mut vz, py, pz, azy);
        rot(m, &mut vx, &mut vz, px, pz, axz);
        if axz != 0 || azy != 0 || axy != 0 {
            // Plane.java:632-652 — refresh projf on rotated coords
            self.projf = 1.0;
            for i in 0..3 {
                for j in 0..3 {
                    if j != i {
                        let dx = vx[i] - vx[j];
                        let dz = vz[i] - vz[j];
                        // int squares + sum overflow-wrap in Java
                        // (Plane.java:641-644)
                        let d2 = dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz)) as f64;
                        self.projf *= (d2.sqrt() / 100.0) as f32;
                    }
                }
            }
            self.projf /= 3.0;
        }

        // camera yaw (Plane.java:655)
        rot(m, &mut vx, &mut vz, m.cx, m.cz, m.xz);

        // Underside heuristics (Plane.java:656-700): nearest-horizontal/ -
        // vertical vertex pair, then a spy() distance test — steals the
        // plane for shading if it faces away from the camera
        let sx_pre: Vec<i32> = (0..n).map(|i| self.xs(m, vx[i], vz[i])).collect();
        let sy_pre: Vec<i32> = (0..n).map(|i| self.ys(m, vy[i], vz[i])).collect();

        let mut behind = false; // var48
        let mut best = 500i32; // var61
        let mut ia = 0usize; // var68 (index used as `a`)
        let mut ib = 0usize; // var65
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    let score = (sx_pre[i] - sx_pre[j]).abs() - (sy_pre[i] - sy_pre[j]).abs();
                    if score < best {
                        ia = i;
                        ib = j;
                        best = score;
                    }
                }
            }
        }
        if sy_pre[ib] < sy_pre[ia] {
            std::mem::swap(&mut ia, &mut ib);
        }
        if self.spy(m, vx[ib], vz[ib]) > self.spy(m, vx[ia], vz[ia]) {
            behind = true;
            let mut ties = 0;
            for i in 0..n {
                if vz[i] < 50 && vy[i] > m.cy {
                    behind = false;
                } else if vy[i] == vy[0] {
                    ties += 1;
                }
            }
            if ties == n && vy[0] > m.cy {
                behind = false;
            }
        }

        // camera pitch (Plane.java:702)
        rot(m, &mut vy, &mut vz, m.cy, m.cz, m.zy);

        // project + edge-of-screen counts (Plane.java:704-750)
        let mut visible = true;
        let mut sx: Vec<i32> = Vec::with_capacity(n);
        let mut sy: Vec<i32> = Vec::with_capacity(n);
        let (mut cnt_top, mut cnt_bot, mut cnt_left, mut cnt_right, mut cnt_near) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        for i in 0..n {
            let sx_i = self.xs(m, vx[i], vz[i]);
            let sy_i = self.ys(m, vy[i], vz[i]);
            sx.push(sx_i);
            sy.push(sy_i);
            if sy_i < 0 || vz[i] < 10 {
                cnt_top += 1;
            }
            if sy_i > m.h || vz[i] < 10 {
                cnt_bot += 1;
            }
            if sx_i < 0 || vz[i] < 10 {
                cnt_left += 1;
            }
            if sx_i > m.w || vz[i] < 10 {
                cnt_right += 1;
            }
            if sy_i < 45 && m.flex != 0 {
                m.flex = 0;
            }
            if vz[i] < 10 {
                cnt_near += 1;
            }
        }
        if cnt_left == n || cnt_top == n || cnt_bot == n || cnt_right == n {
            visible = false;
        }
        if m.trk && (cnt_left != 0 || cnt_top != 0 || cnt_bot != 0 || cnt_right != 0) {
            visible = false;
        }
        if cnt_near != 0 {
            shade = true;
        }

        if visible {
            // Plane.java:753-817 — bounding midpoints via count scans
            let (mut ymax, mut ymin, mut xmax, mut xmin, mut zmax, mut zmin) =
                (0i32, 0i32, 0i32, 0i32, 0i32, 0i32);
            for i in 0..n {
                let (mut c1, mut c2, mut c3, mut c4, mut c5, mut c6) =
                    (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
                for j in 0..n {
                    if vy[i] >= vy[j] {
                        c1 += 1;
                    }
                    if vy[i] <= vy[j] {
                        c2 += 1;
                    }
                    if vx[i] >= vx[j] {
                        c3 += 1;
                    }
                    if vx[i] <= vx[j] {
                        c4 += 1;
                    }
                    if vz[i] >= vz[j] {
                        c5 += 1;
                    }
                    if vz[i] <= vz[j] {
                        c6 += 1;
                    }
                }
                if c1 == n {
                    ymax = vy[i];
                }
                if c2 == n {
                    ymin = vy[i];
                }
                if c3 == n {
                    xmax = vx[i];
                }
                if c4 == n {
                    xmin = vx[i];
                }
                if c5 == n {
                    zmax = vz[i];
                }
                if c6 == n {
                    zmin = vz[i];
                }
            }
            let (ymid, xmid, zmid) = ((ymax + ymin) / 2, (xmax + xmin) / 2, (zmax + zmin) / 2);
            // int squares/products/sum overflow-wrap in Java before the
            // widening (Plane.java:824-828); Math.abs keeps MIN_VALUE
            // negative, hence wrapping_abs
            let ay = m.cy - ymid;
            let ax = m.cx - xmid;
            let gr3 = self.gr.wrapping_mul(self.gr).wrapping_mul(self.gr);
            self.av = (ay
                .wrapping_mul(ay)
                .wrapping_add(ax.wrapping_mul(ax))
                .wrapping_add(zmid.wrapping_mul(zmid))
                .wrapping_add(gr3.wrapping_abs()) as f64)
                .sqrt() as i32;
            if !m.trk && (self.av > m.fade[self.disline as usize] || self.av == 0) {
                visible = false;
            }
            if self.gr > 0 && self.av > 3500 {
                visible = false;
            }
            if self.av > 3000 && m.adv <= 900 {
                shade = true;
            }
            // Plane.java:842 — `(double)this.m.random() > 0.3`: the float roll
            // is promoted to double before the compare, so exactly 0.3f
            // (0.30000001192…) takes the > branch. An f32 compare does not.
            if self.flx != 0 && m.random() as f64 > 0.3 {
                visible = false;
            }
        }

        if visible {
            // Plane.java:848-888 — brightness factor
            let mut fac = self.projf / self.deltaf + 0.3;
            if shade {
                let mut boosted = false;
                if fac > 1.0 {
                    if fac >= 1.27 {
                        boosted = true;
                    }
                    fac = 1.0;
                }
                fac *= if boosted { 0.89 } else { 0.86 };
                if fac < 0.37 {
                    fac = 0.37;
                }
                if self.gr == -9 {
                    fac = 0.7;
                }
                if self.gr != -7 && behind {
                    fac = 0.32;
                }
                if self.gr == -8 {
                    fac = 1.0;
                }
            } else {
                if fac > 1.0 {
                    fac = 1.0;
                }
                if fac < 0.6 || behind {
                    fac = 0.6;
                }
            }

            // Plane.java:891-901 — final color
            let mut c = if !m.trk {
                color::hsb_to_rgb(self.hsb[0], self.hsb[1], self.hsb[2] * fac)
            } else {
                let ob = color::rgb_to_hsb(self.oc[0], self.oc[1], self.oc[2]);
                color::hsb_to_rgb(0.105, 0.95, ob[2] * fac + 0.1)
            };

            // Plane.java:903-912 — per-band fog blend
            for k in 0..8 {
                if self.av > m.fade[k] {
                    for ch in 0..3 {
                        c[ch] = (c[ch] * 3 + m.cfade[ch]) / 4;
                    }
                }
            }

            cv.fill_polygon(&sx, &sy, clampc(c));

            // outlines (Plane.java:916-1012)
            if !shade {
                if self.flx == 0 {
                    cv.draw_polygon(&sx, &sy, [0, 0, 0]);
                } else {
                    if self.flx == 2 {
                        cv.draw_polygon(&sx, &sy, [0, 0, 0]);
                    }
                    if self.flx == 1 {
                        let r = 0i32;
                        let g = ((223.0 + 223.0 * self.m_snap01(m, 1)) as i32).clamp(0, 255);
                        let b = ((255.0 + 255.0 * (m.snap[2] as f32 / 100.0)) as i32).clamp(0, 255);
                        cv.draw_polygon(&sx, &sy, clampc([r, g, b]));
                        self.flx = 2;
                    }
                    if self.flx == 3 {
                        let r = 0i32;
                        let g = ((255.0 + 255.0 * (m.snap[1] as f32 / 100.0)) as i32).clamp(0, 255);
                        let b = ((223.0 + 223.0 * (m.snap[2] as f32 / 100.0)) as i32).clamp(0, 255);
                        cv.draw_polygon(&sx, &sy, clampc([r, g, b]));
                        self.flx = 2;
                    }
                }
            } else if self.av <= 3000 && !m.trk && self.road && m.fade[0] > 4000 {
                // road edge darkening outline (Plane.java:976-993)
                let mut c2 = c;
                for ch in 0..3 {
                    c2[ch] = (c2[ch] - 10).max(0);
                }
                cv.draw_polygon(&sx, &sy, clampc(c2));
            }

            if !self.glass && self.gr == -10 && !m.trk {
                // glass frame outline in the unfogged base color blend
                let mut c = self.c;
                for k in 0..8 {
                    if self.av > m.fade[k] {
                        for ch in 0..3 {
                            c[ch] = (c[ch] * 3 + m.cfade[ch]) / 4;
                        }
                    }
                }
                cv.draw_polygon(&sx, &sy, clampc(c));
            }
        }
    }

    /// `Plane.d` embos branch (Plane.java:237-504): the wobble flicker
    /// polygon (embos ≤ 11, non-glass, random > 0.5), the brighten/heat
    /// ladders over `var44` and `hsb[2]` with the embos 12/13 state flips,
    /// the embos 16 broken-poly picks and the two snap-colored debris
    /// triangles, then the typ-scaled vertex reload and the embos ticker
    /// (70 wraps back to 16). Returns the reloaded world coords.
    #[allow(clippy::too_many_arguments)]
    fn d_embos(
        &mut self,
        cv: &mut Canvas,
        m: &mut Medium,
        px: i32,
        py: i32,
        pz: i32,
        axz: i32,
        axy: i32,
        azy: i32,
        n: usize,
    ) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
        // jitter coords (Java var11/var13/var12) and their projections
        // (var43/var15)
        let mut jx = vec![0i32; n];
        let mut jy = vec![0i32; n];
        let mut jz = vec![0i32; n];

        // Plane.java:237-259 — the flicker wobble draws unconditioned by
        // the visibility culls.
        // Plane.java:237 — the same float->double promotion (0.5 is exact,
        // kept on f64 for the source's comparison shape).
        if self.embos <= 11 && m.random() as f64 > 0.5 && !self.glass {
            for i in 0..n {
                jx[i] = ((self.ox[i] + px) as f32 + (15.0 - m.random() * 30.0)) as i32;
                jy[i] = ((self.oy[i] + py) as f32 + (15.0 - m.random() * 30.0)) as i32;
                jz[i] = ((self.oz[i] + pz) as f32 + (15.0 - m.random() * 30.0)) as i32;
            }
            rot(m, &mut jx, &mut jy, px, py, axy);
            rot(m, &mut jy, &mut jz, py, pz, azy);
            rot(m, &mut jx, &mut jz, px, pz, axz);
            rot(m, &mut jx, &mut jz, m.cx, m.cz, m.xz);
            rot(m, &mut jy, &mut jz, m.cy, m.cz, m.zy);
            let fx: Vec<i32> = (0..n).map(|i| self.xs(m, jx[i], jz[i])).collect();
            let fy: Vec<i32> = (0..n).map(|i| self.ys(m, jy[i], jz[i])).collect();
            cv.fill_polygon(&fx, &fy, [230u8, 230, 230]);
        }

        // Plane.java:262-300 — the brighten ladder consuming the hsb.
        let mut var44 = 1.0f32;
        if self.embos <= 4 {
            var44 = 1.0 + m.random() / 5.0;
        }
        if self.embos > 4 && self.embos <= 7 {
            var44 = 1.0 + m.random() / 4.0;
        }
        if self.embos > 7 && self.embos <= 9 {
            var44 = 1.0 + m.random() / 3.0;
            if self.hsb[2] as f64 > 0.7 {
                self.hsb[2] = 0.7;
            }
        }
        if self.embos > 9 && self.embos <= 10 {
            var44 = 1.0 + m.random() / 2.0;
            if self.hsb[2] as f64 > 0.6 {
                self.hsb[2] = 0.6;
            }
        }
        if self.embos > 10 && self.embos <= 12 {
            var44 = 1.0 + m.random() / 1.0;
            if self.hsb[2] as f64 > 0.5 {
                self.hsb[2] = 0.5;
            }
        }
        if self.embos == 12 {
            self.chip = 1;
            self.ctmag = 2.0;
            self.bfase = -7;
        }
        if self.embos == 13 {
            self.hsb[1] = 0.2;
            self.hsb[2] = 0.4;
        }
        if self.embos == 16 {
            self.pa = (m.random() * n as f32) as i32 as usize;
            self.pb = (m.random() * n as f32) as i32 as usize;
            while self.pa == self.pb {
                self.pb = (m.random() * n as f32) as i32 as usize;
            }
        }

        // Plane.java:302-521 — the broken-panel debris at embos ≥ 16:
        // pa/pb held to ±100 in x/z, a random mid-lift vertex, two snap-
        // colored fills (second pass re-scales var23 by 0.8).
        if self.embos >= 16 {
            let mut sign_y = 1i32; // var49
            let mut sign_x = 1i32; // var54
            let mut fold = azy.wrapping_abs();
            while fold > 270 {
                fold -= 360;
            }
            if fold.abs() > 90 {
                sign_y = -1;
            }
            let mut fold2 = axy.wrapping_abs();
            while fold2 > 270 {
                fold2 -= 360;
            }
            if fold2.abs() > 90 {
                sign_x = -1;
            }

            let mut bx = [0i32; 3];
            let mut by = [0i32; 3];
            let mut bz = [0i32; 3];
            bx[0] = self.ox[self.pa] + px;
            by[0] = self.oy[self.pa] + py;
            bz[0] = self.oz[self.pa] + pz;
            bx[1] = self.ox[self.pb] + px;
            by[1] = self.oy[self.pb] + py;
            bz[1] = self.oz[self.pb] + pz;
            while (bx[0] - bx[1]).abs() > 100 {
                if bx[1] > bx[0] {
                    bx[1] -= 30;
                } else {
                    bx[1] += 30;
                }
            }
            while (bz[0] - bz[1]).abs() > 100 {
                if bz[1] > bz[0] {
                    bz[1] -= 30;
                } else {
                    bz[1] += 30;
                }
            }
            let var21 = (((bx[0] - bx[1]).abs() / 3) as f64 * (0.5 - m.random() as f64)) as i32;
            let var22 = (((bz[0] - bz[1]).abs() / 3) as f64 * (0.5 - m.random() as f64)) as i32;
            bx[2] = (bx[0] + bx[1]) / 2 + var21;
            bz[2] = (bz[0] + bz[1]) / 2 + var22;
            let mut var23 = (((bx[0] - bx[1]).abs() + (bz[0] - bz[1]).abs()) as f64 / 1.5
                * ((m.random() / 2.0) as f64 + 0.5)) as i32;
            by[2] = (by[0] + by[1]) / 2 - sign_y * sign_x * var23;
            rot(m, &mut bx, &mut by, px, py, axy);
            rot(m, &mut by, &mut bz, py, pz, azy);
            rot(m, &mut bx, &mut bz, px, pz, axz);
            rot(m, &mut bx, &mut bz, m.cx, m.cz, m.xz);
            rot(m, &mut by, &mut bz, m.cy, m.cz, m.zy);
            let sx3v: Vec<i32> = (0..3).map(|k| self.xs(m, bx[k], bz[k])).collect();
            let sy3v: Vec<i32> = (0..3).map(|k| self.ys(m, by[k], bz[k])).collect();
            let c1 = (255.0f32 + 255.0 * (m.snap[0] as f32 / 400.0)) as i32;
            let c2 = (169.0f32 + 169.0 * (m.snap[1] as f32 / 300.0)) as i32;
            let c3 = (89.0f32 + 89.0 * (m.snap[2] as f32 / 200.0)) as i32;
            cv.fill_polygon(
                &sx3v,
                &sy3v,
                clampc([c1.clamp(0, 255), c2.clamp(0, 255), c3.clamp(0, 255)]),
            );

            // second fill — same vertices, redone pulls, var23 × 0.8,
            // the 207/136 variant colors.
            bx[0] = self.ox[self.pa] + px;
            by[0] = self.oy[self.pa] + py;
            bz[0] = self.oz[self.pa] + pz;
            bx[1] = self.ox[self.pb] + px;
            by[1] = self.oy[self.pb] + py;
            bz[1] = self.oz[self.pb] + pz;
            while (bx[0] - bx[1]).abs() > 100 {
                if bx[1] > bx[0] {
                    bx[1] -= 30;
                } else {
                    bx[1] += 30;
                }
            }
            while (bz[0] - bz[1]).abs() > 100 {
                if bz[1] > bz[0] {
                    bz[1] -= 30;
                } else {
                    bz[1] += 30;
                }
            }
            bx[2] = (bx[0] + bx[1]) / 2 + var21;
            bz[2] = (bz[0] + bz[1]) / 2 + var22;
            var23 = ((var23 as f64) * 0.8) as i32;
            by[2] = (by[0] + by[1]) / 2 - sign_y * sign_x * var23;
            rot(m, &mut bx, &mut by, px, py, axy);
            rot(m, &mut by, &mut bz, py, pz, azy);
            rot(m, &mut bx, &mut bz, px, pz, axz);
            rot(m, &mut bx, &mut bz, m.cx, m.cz, m.xz);
            rot(m, &mut by, &mut bz, m.cy, m.cz, m.zy);
            let sx3v: Vec<i32> = (0..3).map(|k| self.xs(m, bx[k], bz[k])).collect();
            let sy3v: Vec<i32> = (0..3).map(|k| self.ys(m, by[k], bz[k])).collect();
            let c1 = (255.0f32 + 255.0 * (m.snap[0] as f32 / 400.0)) as i32;
            let c2 = (207.0f32 + 207.0 * (m.snap[1] as f32 / 300.0)) as i32;
            let c3 = (136.0f32 + 136.0 * (m.snap[2] as f32 / 200.0)) as i32;
            cv.fill_polygon(
                &sx3v,
                &sy3v,
                clampc([c1.clamp(0, 255), c2.clamp(0, 255), c3.clamp(0, 255)]),
            );
        }

        // Plane.java:524-538 — the typ-scaled reload the main body runs on.
        let mut vx = vec![0i32; n];
        let mut vy = vec![0i32; n];
        let mut vz = vec![0i32; n];
        for i in 0..n {
            vx[i] = if self.typ == 1 {
                (self.ox[i] as f32 * var44 + px as f32) as i32
            } else {
                self.ox[i] + px
            };
            vy[i] = if self.typ == 2 {
                (self.oy[i] as f32 * var44 + py as f32) as i32
            } else {
                self.oy[i] + py
            };
            vz[i] = if self.typ == 3 {
                (self.oz[i] as f32 * var44 + pz as f32) as i32
            } else {
                self.oz[i] + pz
            };
        }

        // Plane.java:498-503 — the embos ticker (70 wraps back to 16).
        if self.embos != 70 {
            self.embos += 1;
        } else {
            self.embos = 16;
        }
        (vx, vy, vz)
    }

    /// `Plane.d` chip machine (Plane.java:513-637): spark decay, the
    /// cox/coy/coz anchor snapshot with the ctmag-scaled offshoots,
    /// velocity/earth-gravity drift (vy + 7), the ground re-check against
    /// m.ground, and the three-vertex debris fill tinted by the plane's
    /// color (HSB at bfase −7). Runs on the chip's own stored angles.
    fn d_chip(
        &mut self,
        cv: &mut Canvas,
        m: &mut Medium,
        px: i32,
        py: i32,
        pz: i32,
        axz: i32,
        axy: i32,
        azy: i32,
    ) {
        // Plane.java:513-519 — the spark dies on a > 0.6 roll or when the
        // panel is already plain; nocol panels re-arm a faint bfase. The
        // roll is a float promoted to double before the compare, so
        // exactly 0.6f (0.6000000238…) takes the > branch.
        if self.chip == 1 && (m.random() as f64 > 0.6 || self.bfase == 0) {
            self.chip = 0;
            if self.bfase == 0 && self.nocol {
                self.bfase = 1;
            }
        }
        if self.chip == 0 {
            return;
        }
        if self.chip == 1 {
            // Plane.java:521-548 — the snapshot and the debris vertices.
            self.cxz = axz;
            self.cxy = axy;
            self.czy = azy;
            let pick = (m.random() * self.ox.len() as f32) as i32;
            let pick = pick as usize;
            self.cox[0] = self.ox[pick];
            self.coz[0] = self.oz[pick];
            self.coy[0] = self.oy[pick];
            if self.ctmag > 3.0 {
                self.ctmag = 3.0;
            }
            if self.ctmag < -3.0 {
                self.ctmag = -3.0;
            }
            self.cox[1] = (self.cox[0] as f32 + self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            self.cox[2] = (self.cox[0] as f32 + self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            self.coy[1] = (self.coy[0] as f32 + self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            self.coy[2] = (self.coy[0] as f32 + self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            self.coz[1] = (self.coz[0] as f32 + self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            self.coz[2] = (self.coz[0] as f32 + self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            self.dx = 0;
            self.dy = 0;
            self.dz = 0;
            if self.bfase != -7 {
                self.vx = (self.ctmag * (30.0 - m.random() * 60.0)) as i32;
                self.vz = (self.ctmag * (30.0 - m.random() * 60.0)) as i32;
                self.vy = (self.ctmag * (30.0 - m.random() * 60.0)) as i32;
            } else {
                self.vx = (self.ctmag * (10.0 - m.random() * 20.0)) as i32;
                self.vz = (self.ctmag * (10.0 - m.random() * 20.0)) as i32;
                self.vy = (self.ctmag * (10.0 - m.random() * 20.0)) as i32;
            }
            self.chip = 2;
        }

        // Plane.java:552-631 — simulate and paint the debris triangle.
        let mut bx = [self.cox[0] + px, self.cox[1] + px, self.cox[2] + px];
        let mut by = [self.coy[0] + py, self.coy[1] + py, self.coy[2] + py];
        let mut bz = [self.coz[0] + pz, self.coz[1] + pz, self.coz[2] + pz];
        rot(m, &mut bx, &mut by, px, py, self.cxy);
        rot(m, &mut by, &mut bz, py, pz, self.czy);
        rot(m, &mut bx, &mut bz, px, pz, self.cxz);
        for k in 0..3 {
            bx[k] += self.dx;
            by[k] += self.dy;
            bz[k] += self.dz;
        }
        self.dx = self.dx.wrapping_add(self.vx);
        self.dz = self.dz.wrapping_add(self.vz);
        self.dy = self.dy.wrapping_add(self.vy);
        self.vy += 7;
        if by[0] > m.ground {
            self.chip = 19;
        }
        rot(m, &mut bx, &mut bz, m.cx, m.cz, m.xz);
        rot(m, &mut by, &mut bz, m.cy, m.cz, m.zy);
        let sx3v: Vec<i32> = (0..3).map(|k| self.xs(m, bx[k], bz[k])).collect();
        let sy3v: Vec<i32> = (0..3).map(|k| self.ys(m, by[k], bz[k])).collect();
        for k in 0..3 {
            if sy3v[k] < 45 && m.flex != 0 {
                m.flex = 0;
            }
        }
        let pick = (m.random() * 3.0) as i32;
        let col: [i32; 3] = if self.bfase != -7 {
            match pick {
                0 => {
                    // Color.darker() — (int)(ch*0.7), floored at 0
                    [
                        (self.c[0] as f32 * 0.7) as i32,
                        (self.c[1] as f32 * 0.7) as i32,
                        (self.c[2] as f32 * 0.7) as i32,
                    ]
                }
                1 => self.c,
                _ => {
                    // Color.brighter() — (int)min(ch*1.2, 255)
                    [
                        (self.c[0] as f32 * 1.2).min(255.0) as i32,
                        (self.c[1] as f32 * 1.2).min(255.0) as i32,
                        (self.c[2] as f32 * 1.2).min(255.0) as i32,
                    ]
                }
            }
        } else {
            color::hsb_to_rgb(self.hsb[0], self.hsb[1], self.hsb[2])
        };
        cv.fill_polygon(&sx3v, &sy3v, clampc(col));
        self.chip += 1;
        if self.chip == 20 {
            self.chip = 0;
        }
    }

    fn m_snap01(&self, m: &Medium, k: usize) -> f32 {
        m.snap[k] as f32 / 100.0
    }

    /// Port of `Plane.s(...)` (Plane.java:1035-1172) — shadow projector.
    /// `mode`: 0 = onto a matching tracker surface, 1 = onto the ground
    /// plane, 2 = under-car (crs) fixed color.
    #[allow(clippy::too_many_arguments)]
    pub fn s(
        &self,
        cv: &mut Canvas,
        m: &Medium,
        t: &Trackers,
        px: i32,
        py: i32,
        pz: i32,
        axz: i32,
        axy: i32,
        azy: i32,
        mode: i32,
    ) {
        if self.gr > 0 {
            return;
        }
        let n = self.ox.len();
        let mut vx: Vec<i32> = (0..n).map(|i| self.ox[i] + px).collect();
        let mut vy: Vec<i32> = (0..n).map(|i| self.oy[i] + py).collect();
        let mut vz: Vec<i32> = (0..n).map(|i| self.oz[i] + pz).collect();

        rot(m, &mut vx, &mut vy, px, py, axy);
        rot(m, &mut vy, &mut vz, py, pz, azy);
        rot(m, &mut vx, &mut vz, px, pz, axz);

        // shadow color (cgrnd / 1.5)
        let mut c = [
            ((m.cgrnd[0] as f64) / 1.5) as i32,
            ((m.cgrnd[1] as f64) / 1.5) as i32,
            ((m.cgrnd[2] as f64) / 1.5) as i32,
        ];

        // flat shadow at ground by default
        for i in 0..n {
            vy[i] = m.ground;
        }

        // mode 0: find the deepest tracker this plane hovers over
        if mode == 0 {
            for trk in (0..t.nt).rev() {
                let mut hits = 0;
                for i in 0..n {
                    if t.zy[trk].abs() != 90
                        && t.xy[trk].abs() != 90
                        && (vx[i] - (t.x[trk] - m.x)).abs() < t.radx[trk]
                        && (vz[i] - (t.z[trk] - m.z)).abs() < t.radz[trk]
                    {
                        hits += 1;
                    }
                }
                if hits > n / 2 {
                    for i in 0..n {
                        vy[i] = t.y[trk] - m.y;
                        if t.zy[trk] != 0 {
                            let ramp = (vz[i] - (t.z[trk] - m.z - t.radz[trk])) as f32
                                * m.sin(t.zy[trk])
                                / m.sin(90 - t.zy[trk]);
                            let skip =
                                t.radz[trk] as f32 * m.sin(t.zy[trk]) / m.sin(90 - t.zy[trk]);
                            vy[i] = (vy[i] as f32 + ramp - skip) as i32;
                        }
                        if t.xy[trk] != 0 {
                            let ramp = (vx[i] - (t.x[trk] - m.x - t.radx[trk])) as f32
                                * m.sin(t.xy[trk])
                                / m.sin(90 - t.xy[trk]);
                            let skip =
                                t.radx[trk] as f32 * m.sin(t.xy[trk]) / m.sin(90 - t.xy[trk]);
                            vy[i] = (vy[i] as f32 + ramp - skip) as i32;
                        }
                    }
                    c = [
                        ((t.c[trk][0] as f64) / 1.5) as i32,
                        ((t.c[trk][1] as f64) / 1.5) as i32,
                        ((t.c[trk][2] as f64) / 1.5) as i32,
                    ];
                    break;
                }
            }
        }

        let mut show = true;
        if mode == 2 {
            c = [76, 98, 142];
        } else if m.nsp > 0 {
            // spotlights punch holes in the shadow (Plane.java:1115-1122)
            for sp in 0..m.nsp as usize {
                for i in 0..n {
                    if (vx[i] - m.spx[sp]).abs() < m.sprad[sp]
                        && (vz[i] - m.spz[sp]).abs() < m.sprad[sp]
                    {
                        show = false;
                    }
                }
            }
        }

        if show {
            rot(m, &mut vx, &mut vz, m.cx, m.cz, m.xz);
            rot(m, &mut vy, &mut vz, m.cy, m.cz, m.zy);
            let mut sx: Vec<i32> = Vec::with_capacity(n);
            let mut sy: Vec<i32> = Vec::with_capacity(n);
            let (mut c_top, mut c_bot, mut c_left, mut c_right) = (0usize, 0usize, 0usize, 0usize);
            for i in 0..n {
                let x = self.xs(m, vx[i], vz[i]);
                let y = self.ys(m, vy[i], vz[i]);
                sx.push(x);
                sy.push(y);
                if y < 0 || vz[i] < 10 {
                    c_top += 1;
                }
                if y > m.h || vz[i] < 10 {
                    c_bot += 1;
                }
                if x < 0 || vz[i] < 10 {
                    c_left += 1;
                }
                if x > m.w || vz[i] < 10 {
                    c_right += 1;
                }
            }
            if c_left == n || c_top == n || c_bot == n || c_right == n {
                show = false;
            }

            if show {
                // fog blend per the plane's own (stale) av, as the original
                for k in 0..8 {
                    if self.av > m.fade[k] {
                        for ch in 0..3 {
                            c[ch] = (c[ch] * 3 + m.cfade[ch]) / 4;
                        }
                    }
                }
                cv.fill_polygon(&sx, &sy, clampc(c));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> Plane {
        let m = Medium::new();
        Plane::new(
            &m,
            &[0, 100, 50],
            &[0, 0, 80],
            &[0, 0, 20],
            3,
            [120, 130, 140],
            false,
            10,
            0,
            0,
            0,
            7,
            5,
            false,
        )
    }

    #[test]
    fn embos_ticker_walks_the_ladder_without_jittering_glass_free() {
        let mut m = Medium::new();
        let mut cv = Canvas::new(550, 400);
        // Madness.java:2096 — the drive tail re-arms a struck panel at 13;
        // > 11 skips the flicker branch and the brighten ladders.
        let mut p = tri();
        p.embos = 13;
        p.d(&mut cv, &mut m, 0, 0, 3000, 0, 0, 0, 0, 0, false);
        // Plane.java:297-299 — the 13 state pins the HSB, then 498-501
        // ticks the counter.
        assert_eq!(p.embos, 14);
        assert_eq!(p.hsb[1], 0.2);
        assert_eq!(p.hsb[2], 0.4);
        p.d(&mut cv, &mut m, 0, 0, 3000, 0, 0, 0, 0, 0, false);
        assert_eq!(p.embos, 15);
        // The tick reaches 16 first (no work that frame); the NEXT call
        // sees 16, arms pa/pb and paints the debris, then ticks to 18.
        p.d(&mut cv, &mut m, 0, 0, 3000, 0, 0, 0, 0, 0, false);
        assert_eq!(p.embos, 16, "reaching 16 is a plain tick");
        p.d(&mut cv, &mut m, 0, 0, 3000, 0, 0, 0, 0, 0, false);
        assert_eq!(p.embos, 17, "16 arms pa/pb and draws, then ticks");
        // The debris vertex picks landed inside the vertex range.
        assert!(p.pa < p.ox.len());
        assert!(p.pb < p.ox.len());
        assert_ne!(p.pa, p.pb);
    }

    #[test]
    fn chip_machine_runs_snapshot_decay_and_tick() {
        let mut m = Medium::new();
        let mut cv = Canvas::new(550, 400);
        let mut p = tri();
        p.chip = 1;
        p.ctmag = 2.0;
        p.bfase = 5;
        // One call either decays the spark (Plane.java:513-519 roll) or
        // runs the snapshot/debris machine — both leave chip ≠ 1.
        p.d(&mut cv, &mut m, 0, 0, 3000, 0, 0, 0, 0, 0, false);
        assert_ne!(p.chip, 1);
        // A live chip machine moved the anchor offsets and stepped the
        // counter (521-548 setup → chip 2, then the +1 tick).
        p.chip = 1;
        p.ctmag = 2.0;
        p.dx = 0;
        p.dy = 0;
        p.dz = 0;
        while p.chip == 1 {
            p.d(&mut cv, &mut m, 0, 0, 3000, 0, 0, 0, 0, 0, false);
        }
        if p.chip != 0 {
            // the debris triangle took its snapshot this frame
            assert_eq!(p.chip, 3, "setup arms 2 then the tick");
            assert_ne!(p.cox[1], 0, "offshoot vertices were built");
        }
    }
}
