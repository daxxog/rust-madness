// FIDELITY: Medium.java math: Java `!(a<b)` compares, Math.max/min clamps and indexed degree-table loops are preserved verbatim.
#![allow(
    clippy::neg_cmp_op_on_partial_ord,
    clippy::manual_clamp,
    clippy::needless_range_loop
)]
//! Port of `Medium.java` — the camera (position `x/y/z`, yaw `xz`, pitch
//! `zy`, baked screen-center pivots `cx/cy/cz`), the fog/sky palettes and
//! the backdrop renderer `d()`.
//!
//! Divergence notes:
//! - `random()` — the original's single entropy source is the global
//!   `Math.random()`; the port keeps the EXACT walk structure
//!   (`rand[]`/`diup[]`/`cntrn`, Medium.java:49-89) and now reads the
//!   same global slot ([`crate::random::math_random`]) by default, so
//!   its refills interleave with `xtGraphics.random()`/`sortcars` in
//!   Java's call order. The `rng` seam stays for tests that need a
//!   private stream (a seeded `java.util.Random` for the TestRng bridge,
//!   or a fixed vector REPLACING the whole method — the
//!   VecMedium/TenthsMedium subclass stand-in). C3b: the race pixel rows
//!   forced the shared-stream default (the old per-Medium xorshift made
//!   `playsounds`/`preform` draws invisible to `sortcars`).
//! - `watch`/`around` camera modes live in `conto.rs` (they need `Conto`);
//!   so does `aroundtrack` (it needs `CheckPoints`).

use crate::canvas::Canvas;
use crate::random::RngSource;

pub struct Medium {
    pub focus_point: i32,
    /// World ground level; recomputed from `y` by `d()` (250 - y).
    pub ground: i32,
    pub skyline: i32,
    pub fade: [i32; 8],
    pub csky: [i32; 3],
    pub cgrnd: [i32; 3],
    pub cfade: [i32; 3],
    pub snap: [i32; 3],
    pub origfade: i32,
    pub flex: i32,
    pub trk: bool,
    pub crs: bool,
    /// Screen center x (275 on the 550x400 applet).
    pub cx: i32,
    pub cy: i32,
    /// Camera depth offset (50).
    pub cz: i32,
    /// Camera yaw (degrees).
    pub xz: i32,
    /// Camera pitch (degrees).
    pub zy: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    /// Spotlights used to punch holes in shadows (max 5).
    pub nsp: i32,
    pub spx: [i32; 5],
    pub spz: [i32; 5],
    pub sprad: [i32; 5],
    pub td: bool,
    pub vxz: i32,
    pub adv: i32,
    pub vert: bool,
    pub trns: i32,
    pub tcos: [f32; 360],
    pub tsin: [f32; 360],
    // random() state (documented divergence above)
    pub cntrn: i32,
    pub diup: [bool; 3],
    pub rand: [i32; 3],
    pub trn: usize,
    /// Injected entropy source for `random()` (test seam; None = the
    /// global `Math.random()` slot, Java's single stream).
    pub(crate) rng: Option<RngSource>,
    // aroundtrack state (written by conto.rs's aroundtrack)
    pub hit: i32,
    pub ptr: usize,
    pub ptcnt: i32,
    pub nrnd: i32,
    pub trx: i64,
    pub trz: i64,
}

impl Default for Medium {
    fn default() -> Self {
        Self::new()
    }
}

impl Medium {
    pub fn new() -> Medium {
        let mut m = Medium {
            focus_point: 400,
            ground: 250,
            skyline: -300,
            fade: [3000, 6000, 9000, 12000, 15000, 18000, 21000, 24000],
            csky: [217, 224, 255],
            cgrnd: [205, 200, 200],
            cfade: [255, 220, 220],
            snap: [0; 3],
            origfade: 3000,
            flex: 0,
            trk: false,
            crs: false,
            cx: 275,
            cy: 200,
            cz: 50,
            xz: 0,
            zy: 0,
            x: 0,
            y: 0,
            z: 0,
            w: 550,
            h: 400,
            nsp: 0,
            spx: [0; 5],
            spz: [0; 5],
            sprad: [0; 5],
            td: false,
            vxz: 180,
            adv: 500,
            vert: false,
            trns: 1,
            tcos: [0.0; 360],
            tsin: [0.0; 360],
            cntrn: 0,
            diup: [false; 3],
            rand: [0; 3],
            trn: 0,
            rng: None,
            hit: 60000,
            ptr: 0,
            ptcnt: -10,
            nrnd: 0,
            trx: 0,
            trz: 0,
        };
        // Medium.java:111-123 — degree tables
        for i in 0..360 {
            m.tcos[i] = (i as f64 * (core::f64::consts::PI / 180.0)).cos() as f32;
            m.tsin[i] = (i as f64 * (core::f64::consts::PI / 180.0)).sin() as f32;
        }
        m
    }

    /// Wrapping degree lookup like `Medium.sin/cos`.
    pub fn sin(&self, deg: i32) -> f32 {
        let mut d = deg;
        while d >= 360 {
            d -= 360;
        }
        while d < 0 {
            d += 360;
        }
        self.tsin[d as usize]
    }

    pub fn cos(&self, deg: i32) -> f32 {
        let mut d = deg;
        while d >= 360 {
            d -= 360;
        }
        while d < 0 {
            d += 360;
        }
        self.tcos[d as usize]
    }

    /// `Medium.ys(v, d)` — vertical projection, depth clamped at 10
    /// (Medium.java:91-97); product/add overflow-wrap in Java like the
    /// Plane/ContO variants.
    pub fn ys(&self, v: i32, dep: i32) -> i32 {
        let mut d = dep;
        if d < 10 {
            d = 10;
        }
        (d - self.focus_point)
            .wrapping_mul(self.cy - v)
            .wrapping_div(d)
            .wrapping_add(v)
    }

    /// `Medium.setfade(r, g, b)` (Medium.java:125-152).
    pub fn setfade(&mut self, r: i32, g: i32, b: i32) {
        self.cfade = self.snap_blend([r, g, b]);
    }

    /// `Medium.setgrnd(r, g, b)` (Medium.java:398-425).
    pub fn setgrnd(&mut self, r: i32, g: i32, b: i32) {
        self.cgrnd = self.snap_blend([r, g, b]);
    }

    /// `Medium.setsky(r, g, b)` (Medium.java:506-533).
    pub fn setsky(&mut self, r: i32, g: i32, b: i32) {
        self.csky = self.snap_blend([r, g, b]);
    }

    fn snap_blend(&self, c: [i32; 3]) -> [i32; 3] {
        let mut out = c;
        for (k, v) in out.iter_mut().enumerate() {
            *v = (c[k] as f32 + c[k] as f32 * (self.snap[k] as f32 / 100.0)) as i32;
            *v = (*v).clamp(0, 255);
        }
        out
    }

    /// `Medium.setsnap(r, g, b)` (Medium.java:328-332).
    pub fn setsnap(&mut self, r: i32, g: i32, b: i32) {
        self.snap = [r, g, b];
    }

    /// `Medium.fadfrom(v)` (Medium.java:535-541).
    pub fn fadfrom(&mut self, v: i32) {
        for k in 0..8 {
            self.fade[k] = v * (k as i32 + 1);
        }
    }

    /// `Medium.adjstfade` (Medium.java:427-443).
    pub fn adjstfade(&mut self, speed: f32) {
        if speed < 15.0 {
            self.fade[0] = (self.origfade as f32 - 1000.0 * (15.0 - speed)) as i32;
            if self.fade[0] < 3000 {
                self.fade[0] = 3000;
            }
            self.fadfrom(self.fade[0]);
        } else if self.fade[0] != self.origfade {
            self.fade[0] += 500;
            if self.fade[0] > self.origfade {
                self.fade[0] = self.origfade;
            }
            self.fadfrom(self.fade[0]);
        }
    }

    /// `Medium.addsp` (Medium.java:445-452) — shadow spotlight registry.
    pub fn addsp(&mut self, x: i32, z: i32, rad: i32) {
        if self.nsp != 5 {
            let i = self.nsp as usize;
            self.spx[i] = x;
            self.spz[i] = z;
            self.sprad[i] = rad;
            self.nsp += 1;
        }
    }

    /// `Medium.random()` (Medium.java:49-89) — the exact walk structure:
    /// a refill on cntrn == 0 (one draw per digit + the diup comparison
    /// pair), then every call steps all three digits ±1 with the 10/0 and
    /// -1/9 wraps, advances trn with the 3->0 wrap and returns
    /// rand\[trn\]/10. Entropy comes from `draw()`. The diup polarity is
    /// the source's: `diup = !(a > b)` (Medium.java:55-59).
    pub fn random(&mut self) -> f32 {
        // A subclass-style override REPLACES the whole method (the vector
        // player mirrors VecMedium/TenthsMedium): no walk, no cntrn.
        if let Some(RngSource::Vector(v, i)) = self.rng.as_mut() {
            let x = v[*i];
            *i += 1;
            if *i == v.len() {
                *i = 0;
            }
            return x;
        }

        if self.cntrn == 0 {
            for k in 0..3 {
                self.rand[k] = (10.0 * self.draw()) as i32;
                let a = self.draw();
                let b = self.draw();
                self.diup[k] = !(a > b);
            }
            self.cntrn = 20;
        } else {
            self.cntrn -= 1;
        }

        for k in 0..3 {
            if self.diup[k] {
                self.rand[k] += 1;
                if self.rand[k] == 10 {
                    self.rand[k] = 0;
                }
            } else {
                self.rand[k] -= 1;
                if self.rand[k] == -1 {
                    self.rand[k] = 9;
                }
            }
        }

        self.trn += 1;
        if self.trn == 3 {
            self.trn = 0;
        }

        self.rand[self.trn] as f32 / 10.0
    }

    /// One entropy draw for the walk. Java has ONE stream: every
    /// `Medium.random()` refill reads the same global `Math.random()`
    /// holder that `xtGraphics.random()` and `sortcars` do, interleaved
    /// in call order. The default therefore reads the global slot
    /// ([`crate::random::math_random`]); the `RngSource::Java` seam stays
    /// for the single-medium mirror pins that replay a private stream
    /// (TMedium/TJavaRandom), and `Vector` still replaces the method.
    ///
    /// C3b: the race pixel rows forced this. The old per-Medium xorshift
    /// default gave the player a private stream, so `playsounds`/`preform`
    /// draws never interleaved with `sortcars`/`xtGraphics.random()`;
    /// Java's shared stream is observable in every race frame md5.
    fn draw(&mut self) -> f64 {
        match self.rng.as_mut() {
            Some(RngSource::Java(r)) => r.next_double(),
            _ => crate::random::math_random(),
        }
    }

    /// The backdrop: 8 ground fade bands, 8 sky fade bands rotated by the
    /// camera pitch, then the horizon blend strip (Medium.java:154-283).
    /// Draws with `fillPolygon` quads like the original AWT code.
    pub fn d(&mut self, cv: &mut Canvas) {
        self.nsp = 0;
        if self.zy > 90 {
            self.zy = 90;
        }
        if self.zy < -90 {
            self.zy = -90;
        }
        if self.xz > 360 {
            self.xz -= 360;
        }
        if self.xz < 0 {
            self.xz += 360;
        }
        if self.y > 0 {
            self.y = 0;
        }

        self.ground = 250 - self.y;

        fn clampc(c: [i32; 3]) -> [u8; 3] {
            [
                c[0].clamp(0, 255) as u8,
                c[1].clamp(0, 255) as u8,
                c[2].clamp(0, 255) as u8,
            ]
        }

        let w = self.w;
        let hgt = self.h;
        let mut xs = [0i32; 4];
        let mut ys = [0i32; 4];
        let mut color = self.cgrnd;
        let mut bottom = hgt;
        let mut band = 0;
        while band < 8 {
            // rotated ground line at each fade distance
            let depth = self.fade[band];
            let mut lvl = self.ground;
            let mut dep = depth;
            if self.zy != 0 {
                lvl = self.cy
                    + ((self.ground - self.cy) as f32 * self.cos(self.zy)
                        - (depth - self.cz) as f32 * self.sin(self.zy))
                        as i32;
                dep = self.cz
                    + ((self.ground - self.cy) as f32 * self.sin(self.zy)
                        + (depth - self.cz) as f32 * self.cos(self.zy))
                        as i32;
            }

            xs[0] = 0;
            ys[0] = self.ys(lvl, dep);
            xs[1] = 0;
            ys[1] = bottom;
            xs[2] = w;
            ys[2] = bottom;
            xs[3] = w;
            ys[3] = ys[0];
            bottom = ys[0];
            if band > 0 {
                for k in 0..3 {
                    color[k] = (color[k] * 3 + self.cfade[k]) / 4;
                }
            }
            if ys[0] < self.h && ys[1] > 0 {
                cv.fill_polygon(&xs, &ys, clampc(color));
            }
            band += 1;
        }

        // sky bands (text/browser versions identical structure)
        let mut color = self.csky;
        let mut top = 0;
        if self.flex == 2 {
            xs[0] = 200;
            ys[0] = 45;
            xs[1] = 200;
            ys[1] = 0;
            xs[2] = 350;
            ys[2] = 0;
            xs[3] = 350;
            ys[3] = 45;
            cv.fill_polygon(&xs, &ys, clampc(color));
            top = 45;
        }

        let mut band = 0;
        while band < 8 {
            let depth = self.fade[band];
            let mut lvl = self.skyline;
            let mut dep = depth;
            if self.zy != 0 {
                lvl = self.cy
                    + ((self.skyline - self.cy) as f32 * self.cos(self.zy)
                        - (depth - self.cz) as f32 * self.sin(self.zy))
                        as i32;
                dep = self.cz
                    + ((self.skyline - self.cy) as f32 * self.sin(self.zy)
                        + (depth - self.cz) as f32 * self.cos(self.zy))
                        as i32;
            }

            xs[0] = 0;
            ys[0] = self.ys(lvl, dep);
            if ys[0] > self.h {
                ys[0] = self.h;
            }
            xs[1] = 0;
            ys[1] = top;
            xs[2] = w;
            ys[2] = top;
            xs[3] = w;
            ys[3] = ys[0];
            top = ys[0];
            if band > 0 {
                for k in 0..3 {
                    color[k] = (color[k] * 3 + self.cfade[k]) / 4;
                }
            }
            if ys[0] > 0 && ys[1] < self.h {
                cv.fill_polygon(&xs, &ys, clampc(color));
            }
            band += 1;
        }

        // horizon blend strip between last sky and bottom-most ground band
        xs[0] = 0;
        ys[0] = top;
        xs[1] = 0;
        ys[1] = bottom;
        xs[2] = w;
        ys[2] = bottom;
        xs[3] = w;
        ys[3] = top;
        if ys[0] < self.h && ys[1] > 0 {
            cv.fill_polygon(&xs, &ys, clampc(self.cfade));
        }
    }
}
