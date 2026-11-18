// FIDELITY: Madness.drive(): Java `if (x>hi) x=hi; if (x<lo) x=lo;`, `x>330 || x<30` and `!(a<b)` float checks mirror Madness.java exactly.
#![allow(
    clippy::manual_clamp,
    clippy::manual_range_contains,
    clippy::neg_cmp_op_on_partial_ord
)]
//! Port of `Madness.java` — the per-car physics/identity state that the
//! original pairs with a `ContO` pool slot by index (it is NOT a `ContO`
//! subclass; `GameSparker` keeps a `Madness[5]` beside the pool and the
//! `colide`/`drive` calls take both).
//!
//! This slice: everything — the damage pair now pushes its dents into
//! the shared `Record` (the `rpd` pointer the original Madness holds),
//! `drive()` stamps the dest/fix/cotchinow bookkeeping into it, and the
//! crash/skid/fix voices report through `&mut XtGraphics` (the `xt` ref
//! the original Madness holds), like `Conto` passes `Medium`.

use crate::checkpoints::{CheckPoints, PLAYERS};
use crate::conto::Conto;
use crate::control::Control;
use crate::medium::Medium;
use crate::record::Record;
use crate::trackers::Trackers;
use crate::xtgraphics::XtGraphics;

// == per-car stat tables (Madness.java:11-55), indexed by `cn` (the chosen
// car number 0..9). They are instance fields in the original, but a grep
// over the class shows only `moment` is ever written after the literal —
// `reseto` overrides it for the balance-tweak `cn == 7` cars — so every
// table is shared read-only data EXCEPT `moment`, which stays owned below.
pub static ACELF: [[f32; 3]; 10] = [
    [11.0, 5.0, 3.0],
    [8.0, 7.0, 5.0],
    [10.0, 5.0, 3.5],
    [11.0, 6.0, 3.5],
    [10.0, 5.0, 3.5],
    [12.0, 6.0, 3.0],
    [7.0, 9.0, 4.0],
    [9.0, 5.0, 3.0],
    [11.0, 7.0, 4.0],
    [12.0, 6.0, 3.5],
];
pub static SWITS: [[i32; 3]; 10] = [
    [50, 180, 280],
    [50, 200, 310],
    [60, 180, 275],
    [70, 190, 295],
    [70, 170, 275],
    [60, 200, 290],
    [60, 170, 280],
    [50, 160, 270],
    [80, 200, 300],
    [70, 210, 290],
];
pub static HANDB: [i32; 11] = [7, 10, 7, 15, 12, 8, 9, 10, 7, 7, 7]; // 11 slots in the original
pub static AIRS: [f32; 10] = [1.0, 1.2, 0.95, 1.0, 1.5, 1.0, 0.9, 0.8, 1.3, 1.0];
pub static AIRC: [i32; 10] = [70, 30, 40, 40, 30, 50, 40, 10, 100, 60];
pub static DRAG: [f32; 10] = [0.5; 10];
pub static TURN: [i32; 10] = [6, 9, 5, 7, 6, 7, 5, 4, 7, 6];
pub static GRIP: [f32; 10] = [20.0, 27.0, 18.0, 22.0, 15.0, 20.0, 25.0, 25.0, 25.0, 27.0];
pub static BOUNCE: [f32; 10] = [1.2, 1.05, 1.3, 1.15, 1.3, 1.2, 1.15, 0.8, 1.1, 1.15];
pub static SIMAG: [f32; 10] = [0.9, 0.85, 1.05, 0.9, 0.85, 0.9, 1.05, 1.1, 0.9, 1.15];
pub static COMPRAD: [f32; 10] = [0.5, 0.4, 0.8, 0.5, 0.5, 0.5, 0.5, 1.5, 0.5, 0.8];
pub static PUSH: [i32; 10] = [2, 2, 3, 3, 2, 2, 2, 4, 2, 2];
pub static REVPUSH: [i32; 10] = [2, 3, 2, 2, 2, 2, 2, 1, 2, 1];
pub static LIFT: [i32; 10] = [0, 30, 0, 20, 0, 30, 0, 0, 30, 0];
pub static REVLIFT: [i32; 10] = [0, 0, 15, 0, 0, 0, 0, 0, 0, 32];
pub static POWERLOSS: [i32; 10] = [
    2_500_000, 2_500_000, 3_500_000, 2_500_000, 2_500_000, 2_500_000, 3_200_000, 4_500_000,
    3_000_000, 5_500_000,
];
pub static FLIPY: [i32; 10] = [-50, -26, -90, -41, -55, -53, -54, -85, -60, -127];
pub static MSQUASH: [i32; 10] = [7, 3, 7, 2, 3, 3, 6, 10, 3, 8];
pub static CLRAD: [i32; 10] = [3300, 1500, 4700, 3000, 1700, 2100, 3500, 7000, 4000, 4000];
pub static MAXMAG: [i32; 10] = [3500, 1700, 7500, 5000, 3000, 4100, 6000, 9000, 4400, 9500];
pub static DAMMULT: [f32; 10] = [
    1.0, 2.028, 0.9375, 1.1791, 1.0, 0.9066, 1.0, 0.6969, 0.8266, 0.7667,
];
pub static OUTDAM: [i32; 10] = [77, 35, 80, 67, 55, 75, 81, 100, 75, 90];

pub struct Madness {
    pub cn: usize, // car number 0..9, indexes the tables above
    pub im: usize, // pool slot 0..4
    pub mxz: i32,
    pub cxz: i32,
    /// The only table `reseto` ever writes (Madness.java:572-578 keeps a
    /// sway-balance override for `cn == 7` cars). Owned, like the original
    /// instance field — and the override persists across resets if the car
    /// number changes afterwards, exactly as in the original.
    pub moment: [f32; 10],
    pub dominate: [bool; PLAYERS],
    pub caught: [bool; PLAYERS],
    pub pzy: i32,
    pub pxy: i32,
    pub speed: f32,
    pub forca: f32,
    pub scy: [f32; 4],
    pub scz: [f32; 4],
    pub scx: [f32; 4],
    pub mtouch: bool,
    pub wtouch: bool,
    pub cntouch: i32,
    pub capsized: bool,
    pub txz: i32,
    pub fxz: i32,
    pub pmlt: i32,
    pub nmlt: i32,
    pub dcnt: i32,
    pub skid: i32,
    pub pushed: bool,
    pub gtouch: bool,
    pub pl: bool,
    pub pr: bool,
    pub pd: bool,
    pub pu: bool,
    pub loop_: i32,
    pub ucomp: f32,
    pub dcomp: f32,
    pub lcomp: f32,
    pub rcomp: f32,
    pub lxz: i32,
    pub travxy: i32,
    pub travzy: i32,
    pub travxz: i32,
    pub trcnt: i32,
    pub capcnt: i32,
    pub srfcnt: i32,
    pub rtab: bool,
    pub ftab: bool,
    pub btab: bool,
    pub surfer: bool,
    pub powerup: f32,
    pub xtpower: i32,
    pub tilt: f32,
    pub squash: i32,
    pub nbsq: i32,
    pub hitmag: i32,
    pub cntdest: i32,
    pub dest: bool,
    pub newcar: bool,
    pub pan: i32,
    pub pcleared: i32,
    pub clear: i32,
    pub nlaps: i32,
    pub focus: i32,
    pub power: f32,
    pub missedcp: i32,
    pub lastcolido: i32,
    pub point: i32,
    pub nofocus: bool,
    pub colidim: bool,
}

impl Madness {
    /// `Madness` constructor (Madness.java:279-284): identity index plus
    /// reset-to-defaults. The original also stores `Medium`/`Record`/
    /// `xtGraphics` refs; those arrive as method arguments with the
    /// milestone that needs them.
    pub fn new(im: usize) -> Madness {
        Madness {
            cn: 0,
            im,
            mxz: 0,
            cxz: 0,
            moment: [1.2, 0.75, 1.4, 1.0, 0.85, 1.25, 1.4, 2.0, 1.5, 2.0],
            dominate: [false; PLAYERS],
            caught: [false; PLAYERS],
            pzy: 0,
            pxy: 0,
            speed: 0.0,
            forca: 0.0,
            scy: [0.0; 4],
            scz: [0.0; 4],
            scx: [0.0; 4],
            mtouch: false,
            wtouch: false,
            cntouch: 0,
            capsized: false,
            txz: 0,
            fxz: 0,
            pmlt: 1,
            nmlt: 1,
            dcnt: 0,
            skid: 0,
            pushed: false,
            gtouch: false,
            pl: false,
            pr: false,
            pd: false,
            pu: false,
            loop_: 0,
            ucomp: 0.0,
            dcomp: 0.0,
            lcomp: 0.0,
            rcomp: 0.0,
            lxz: 0,
            travxy: 0,
            travzy: 0,
            travxz: 0,
            trcnt: 0,
            capcnt: 0,
            srfcnt: 0,
            rtab: false,
            ftab: false,
            btab: false,
            surfer: false,
            powerup: 0.0,
            xtpower: 0,
            tilt: 0.0,
            squash: 0,
            nbsq: 0,
            hitmag: 0,
            cntdest: 0,
            dest: false,
            newcar: false,
            pan: 0,
            pcleared: 0,
            clear: 0,
            nlaps: 0,
            focus: -1,
            power: 75.0,
            missedcp: 0,
            lastcolido: 0,
            point: 0,
            nofocus: false,
            colidim: false,
        }
    }

    /// `Madness.py` (Madness.java:118-120): int squares wrap in Java — and
    /// so does the subtraction producing them.
    pub fn py(&self, a: i32, b: i32, c: i32, d: i32) -> i32 {
        let d1 = a.wrapping_sub(b);
        let d2 = c.wrapping_sub(d);
        d1.wrapping_mul(d1).wrapping_add(d2.wrapping_mul(d2))
    }

    /// `Madness.rpy` (Madness.java:286-288): float squares summed left to
    /// right, then truncated to int (NaN casts to 0 under saturating casts,
    /// matching `(int)`).
    pub fn rpy(&self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> i32 {
        let s = (a - b) * (a - b) + (c - d) * (c - d) + (e - f) * (e - f);
        s as i32
    }

    /// `reseto` (Madness.java:563-648): reset this car's racing state with
    /// identity `cn` (the chosen car number) against its pool slot, at race
    /// start. `cn` is a slot-independent stat-table index; `im` stays as
    /// constructed (and indexes `cp.dested`).
    pub fn reseto(&mut self, cn: usize, o: &mut Conto, cp: &mut CheckPoints) {
        self.cn = cn;
        for slot in 0..PLAYERS {
            self.dominate[slot] = false;
            self.caught[slot] = false;
        }

        // Madness.java:572-578 — sway override for `cn == 7` cars (the two
        // int-only stage-10 tweak is keep-1.7/else-2.0)
        if self.cn == 7 && self.im == 0 {
            if cp.stage == 10 {
                self.moment[self.cn] = 1.7;
            } else {
                self.moment[self.cn] = 2.0;
            }
        }

        self.mxz = 0;
        self.cxz = 0;
        self.pzy = 0;
        self.pxy = 0;
        self.speed = 0.0;
        for slot in 0..4 {
            self.scy[slot] = 0.0;
            self.scx[slot] = 0.0;
            self.scz[slot] = 0.0;
        }

        // Madness.java:593-600 — the spring force scales with the sum of
        // the four wheel key lengths; each term is `sqrt(keyz[i]² + keyx[i]²)`
        // with the int product/sum wrapping before the f64 widen, and the
        // `bounce` line subtracts 0.3 in f64 before the f32 round-trip.
        let mult = ((BOUNCE[self.cn] as f64) - 0.3) as f32;
        self.forca = (0.0f32
            + (o.keyz[0]
                .wrapping_mul(o.keyz[0])
                .wrapping_add(o.keyx[0].wrapping_mul(o.keyx[0])) as f64)
                .sqrt() as f32
            + (o.keyz[1]
                .wrapping_mul(o.keyz[1])
                .wrapping_add(o.keyx[1].wrapping_mul(o.keyx[1])) as f64)
                .sqrt() as f32
            + (o.keyz[2]
                .wrapping_mul(o.keyz[2])
                .wrapping_add(o.keyx[2].wrapping_mul(o.keyx[2])) as f64)
                .sqrt() as f32
            + (o.keyz[3]
                .wrapping_mul(o.keyz[3])
                .wrapping_add(o.keyx[3].wrapping_mul(o.keyx[3])) as f64)
                .sqrt() as f32)
            / 10000.0
            * mult;
        self.mtouch = false;
        self.wtouch = false;
        self.txz = 0;
        self.fxz = 0;
        self.pmlt = 1;
        self.nmlt = 1;
        self.dcnt = 0;
        self.skid = 0;
        self.pushed = false;
        self.gtouch = false;
        self.pl = false;
        self.pr = false;
        self.pd = false;
        self.pu = false;
        self.loop_ = 0;
        self.ucomp = 0.0;
        self.dcomp = 0.0;
        self.lcomp = 0.0;
        self.rcomp = 0.0;
        self.lxz = 0;
        self.travxy = 0;
        self.travzy = 0;
        self.travxz = 0;
        self.rtab = false;
        self.ftab = false;
        self.btab = false;
        self.powerup = 0.0;
        self.xtpower = 0;
        self.trcnt = 0;
        self.capcnt = 0;
        self.tilt = 0.0;
        self.pan = 0;
        self.pcleared = cp.pcs;
        self.clear = 0;
        self.nlaps = 0;
        self.focus = -1;
        self.missedcp = 0;
        self.nofocus = false;
        self.power = 98.0;
        self.lastcolido = 0;
        cp.dested[self.im] = 0;
        self.squash = 0;
        self.nbsq = 0;
        self.hitmag = 0;
        self.cntdest = 0;
        self.dest = false;
        self.newcar = false;
    }

    /// `Madness.colide` (Madness.java:383-553) — the pairwise crash model.
    /// Builds both cars' four wheel anchors (with the capsized-car
    /// `flipy + squash` quirk — SELF's capsized flag picks the branch for BOTH
    /// cars), runs the six `rot` walks, then arms `dominate` from the
    /// `|power·speed·moment|` duel (ties fall to the heavier moment). The
    /// dominant car walks the 4×4 wheel matrix: the x- and z-arms apply
    /// the cross-push (the OTHER's spring gets SELF's push multiplier and
    /// vice versa, ±300 clamped) through the `reg*` calls with the
    /// `colidim` gate armed around them, drop the `revlift` sink on SELF's
    /// wheel and add `lift` to the OTHER's. Only the dominant side does
    /// work, and only while the pair stays caught/on-speed.
    pub fn colide(
        &mut self,
        o: &mut Conto,
        other: &mut Madness,
        other_o: &mut Conto,
        m: &mut Medium,
        rpd: &mut Record,
        xt: &mut XtGraphics,
    ) {
        let mut var4 = [0.0f32; 4];
        let mut var5 = [0.0f32; 4];
        let mut var6 = [0.0f32; 4];
        let mut var7 = [0.0f32; 4];
        let mut var8 = [0.0f32; 4];
        let mut var9 = [0.0f32; 4];
        let mut var10 = 0usize;
        while var10 < 4 {
            var4[var10] = (o.x + o.keyx[var10]) as f32;
            var5[var10] = if self.capsized {
                (o.y + FLIPY[self.cn] + self.squash) as f32
            } else {
                (o.y + o.grat) as f32
            };
            var6[var10] = (o.z + o.keyz[var10]) as f32;
            var7[var10] = (other_o.x + other_o.keyx[var10]) as f32;
            var8[var10] = if self.capsized {
                (other_o.y + FLIPY[other.cn] + other.squash) as f32
            } else {
                (other_o.y + other_o.grat) as f32
            };
            var9[var10] = (other_o.z + other_o.keyz[var10]) as f32;
            var10 += 1;
        }

        self.rot(m, &mut var4, &mut var5, o.x, o.y, o.xy, 4);
        self.rot(m, &mut var5, &mut var6, o.y, o.z, o.zy, 4);
        self.rot(m, &mut var4, &mut var6, o.x, o.z, o.xz, 4);
        self.rot(m, &mut var7, &mut var8, other_o.x, other_o.y, other_o.xy, 4);
        self.rot(m, &mut var8, &mut var9, other_o.y, other_o.z, other_o.zy, 4);
        self.rot(m, &mut var7, &mut var9, other_o.x, other_o.z, other_o.xz, 4);

        // Madness.java:417-436 — the sphere-range test arming the duel.
        if (self.rpy(
            o.x as f32,
            other_o.x as f32,
            o.y as f32,
            other_o.y as f32,
            o.z as f32,
            other_o.z as f32,
        ) as f64)
            < ((o.maxr.wrapping_mul(o.maxr) + other_o.maxr.wrapping_mul(other_o.maxr)) as f64) * 1.5
        {
            if !self.caught[other.im] && (self.speed != 0.0 || other.speed != 0.0) {
                let p1 = (self.power * self.speed * self.moment[self.cn]).abs();
                let p2 = (other.power * other.speed * other.moment[other.cn]).abs();
                if p1 != p2 {
                    self.dominate[other.im] = p1 > p2;
                } else {
                    self.dominate[other.im] = self.moment[self.cn] > other.moment[other.cn];
                }
                self.caught[other.im] = true;
            }
        } else if self.caught[other.im] {
            self.caught[other.im] = false;
        }

        // Madness.java:438-552 — the dominant side's wheel matrix.
        if self.dominate[other.im] {
            let dsz = (self.scz[0] - other.scz[0])
                + (self.scz[1] - other.scz[1])
                + (self.scz[2] - other.scz[2])
                + (self.scz[3] - other.scz[3]);
            let dsx = (self.scx[0] - other.scx[0])
                + (self.scx[1] - other.scx[1])
                + (self.scx[2] - other.scx[2])
                + (self.scx[3] - other.scx[3]);
            let var10_enc = ((dsz * dsz + dsx * dsx) / 16.0) as i32;

            for var11 in 0..4 {
                for var12 in 0..4 {
                    if (self.rpy(
                        var4[var11],
                        var7[var12],
                        var5[var11],
                        var8[var12],
                        var6[var11],
                        var9[var12],
                    ) as f32)
                        < ((var10_enc + 7000) as f32) * (COMPRAD[other.cn] + COMPRAD[self.cn])
                    {
                        // Madness.java:456-496 — the x-arm: the cross-push.
                        if (self.scx[var11] * self.moment[self.cn]).abs()
                            > (other.scx[var12] * other.moment[other.cn]).abs()
                        {
                            let mut var13x = other.scx[var12] * REVPUSH[self.cn] as f32;
                            if var13x > 300.0 {
                                var13x = 300.0;
                            }
                            if var13x < -300.0 {
                                var13x = -300.0;
                            }
                            let mut var14 = self.scx[var11] * PUSH[self.cn] as f32;
                            if var14 > 300.0 {
                                var14 = 300.0;
                            }
                            if var14 < -300.0 {
                                var14 = -300.0;
                            }
                            other.scx[var12] += var14;
                            if self.im == 0 {
                                other.colidim = true;
                            }
                            other.regx(
                                var12,
                                var14 * self.moment[self.cn],
                                other_o,
                                Some(&mut *m),
                                Some(&mut *rpd),
                                Some(&mut *xt),
                            );
                            if other.colidim {
                                other.colidim = false;
                            }
                            self.scx[var11] -= var13x;
                            // Madness.java:486 — `-var13 * var2.moment[this.cn]`:
                            // the OTHER car's moment table indexed by SELF's car
                            // number (a source quirk, not `var2.cn`).
                            self.regx(
                                var11,
                                -var13x * other.moment[self.cn],
                                o,
                                Some(&mut *m),
                                Some(&mut *rpd),
                                Some(&mut *xt),
                            );
                            self.scy[var11] -= REVLIFT[self.cn] as f32;
                            if self.im == 0 {
                                other.colidim = true;
                            }
                            other.regy(
                                var12,
                                (REVLIFT[self.cn] * 7) as f32,
                                other_o,
                                Some(&mut *m),
                                Some(&mut *rpd),
                                Some(&mut *xt),
                            );
                            if other.colidim {
                                other.colidim = false;
                            }
                        }

                        // Madness.java:498-538 — the z-arm, mirrored.
                        if (self.scz[var11] * self.moment[self.cn]).abs()
                            > (other.scz[var12] * other.moment[other.cn]).abs()
                        {
                            let mut var16 = other.scz[var12] * REVPUSH[self.cn] as f32;
                            if var16 > 300.0 {
                                var16 = 300.0;
                            }
                            if var16 < -300.0 {
                                var16 = -300.0;
                            }
                            let mut var17 = self.scz[var11] * PUSH[self.cn] as f32;
                            if var17 > 300.0 {
                                var17 = 300.0;
                            }
                            if var17 < -300.0 {
                                var17 = -300.0;
                            }
                            other.scz[var12] += var17;
                            if self.im == 0 {
                                other.colidim = true;
                            }
                            other.regz(
                                var12,
                                var17 * self.moment[self.cn],
                                other_o,
                                Some(&mut *m),
                                Some(&mut *rpd),
                                Some(&mut *xt),
                            );
                            if other.colidim {
                                other.colidim = false;
                            }
                            self.scz[var11] -= var16;
                            // Madness.java:528 — `-var16 * var2.moment[this.cn]`
                            // (the same self.cn index quirk as the x-arm).
                            self.regz(
                                var11,
                                -var16 * other.moment[self.cn],
                                o,
                                Some(&mut *m),
                                Some(&mut *rpd),
                                Some(&mut *xt),
                            );
                            self.scy[var11] -= REVLIFT[self.cn] as f32;
                            if self.im == 0 {
                                other.colidim = true;
                            }
                            other.regy(
                                var12,
                                (REVLIFT[self.cn] * 7) as f32,
                                other_o,
                                Some(&mut *m),
                                Some(&mut *rpd),
                                Some(&mut *xt),
                            );
                            if other.colidim {
                                other.colidim = false;
                            }
                        }

                        // Madness.java:540-548 — the referee markers and
                        // the lift sink on the other car's wheel.
                        if self.im == 0 {
                            other.lastcolido = 70;
                        }
                        if other.im == 0 {
                            self.lastcolido = 70;
                        }
                        other.scy[var12] -= LIFT[self.cn] as f32;
                    }
                }
            }
        }
    }

    /// `Madness.distruct` (Madness.java:555-562) — the wreck trigger:
    /// every non-wheel plane snaps to embos 1, and the Plane.d embos
    /// ladder then walks each panel up to the 12 break, the chip spark,
    /// and the ≥ 16 broken-panel state by itself.
    pub fn distruct(&mut self, o: &mut Conto) {
        for var2 in 0..o.planes.len() {
            if o.planes[var2].wz == 0 {
                o.planes[var2].embos = 1;
            }
        }
    }

    /// `rot(float[], float[], int, int, int, int)` (Madness.java:372-382) —
    /// rotates `var6` coordinate pairs of the two arrays around the
    /// (var3, var4) centre by the integer angle var5. Angle 0 skips all
    /// work; every float pair keeps the original evaluation order.
    ///
    /// `pub(crate)` for the Phase B mirror: `TMadness` pins `rot` directly
    /// (TMadness.java:33-49). Not game-facing API.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rot(
        &self,
        m: &Medium,
        var1: &mut [f32],
        var2: &mut [f32],
        var3: i32,
        var4: i32,
        var5: i32,
        var6: usize,
    ) {
        if var5 != 0 {
            for var7 in 0..var6 {
                let var8 = var1[var7];
                let var9 = var2[var7];
                var1[var7] = var3 as f32
                    + ((var8 - var3 as f32) * m.cos(var5) - (var9 - var4 as f32) * m.sin(var5));
                var2[var7] = var4 as f32
                    + (var8 - var3 as f32) * m.sin(var5)
                    + (var9 - var4 as f32) * m.cos(var5);
            }
        }
    }

    /// `regx` (Madness.java:650-730) — the x-axis wall dent: vertex dents
    /// push `ox` outward by `dmg·cos(xz)·cos(xy)` and `oz` along
    /// `−dmg·sin(xz)·cos(zy)`, with the bfase/HSB scorch ladder for
    /// damaged panels. The `new Color()` statement (717) is dead in the
    /// original and not ported.
    ///
    /// `m`/`rpd`/`xt` are `Option` because the original holds all three as
    /// nullable fields (`Madness.java:4-6`): a `None` here is Java's `null`,
    /// and each deref site panics with the source's NPE at its own line
    /// (TMadnessWorld pins `err_null_m_wz0_npe` / `err_null_rpd_npe` /
    /// `regy_im0_reaches_xt_crash`). The game always passes `Some`.
    pub fn regx(
        &mut self,
        var1: usize,
        mut var2: f32,
        o: &mut Conto,
        mut m: Option<&mut Medium>,
        rpd: Option<&mut Record>,
        xt: Option<&mut XtGraphics>,
    ) {
        var2 *= DAMMULT[self.cn];
        if var2.abs() > 100.0 {
            // Madness.java:653 — `this.rpd.recx(...)`; the first deref of the
            // nullable Record (before any plane or Medium touch).
            rpd.expect("NullPointerException: Madness.rpd is null (Madness.java:653)")
                .recx(var1, var2, self.im);
            if var2 > 100.0 {
                var2 -= 100.0;
            }
            if var2 < -100.0 {
                var2 += 100.0;
            }
            // Madness.java:662-663 — the regx crash report; a null xt NPEs
            // here only for the player slot (or a colidim car).
            if self.im == 0 || self.colidim {
                xt.expect("NullPointerException: Madness.xt is null (Madness.java:663)")
                    .crash(var2, 0);
            }
            for var4 in 0..o.planes.len() {
                let mut var5 = 0.0f32;
                for var6 in 0..o.planes[var4].ox.len() {
                    if o.planes[var4].wz == 0
                        && self.py(
                            o.keyx[var1],
                            o.planes[var4].ox[var6],
                            o.keyz[var1],
                            o.planes[var4].oz[var6],
                        ) < CLRAD[self.cn]
                    {
                        // Madness.java:670-671 — the plane loop's only
                        // Medium deref; a `wz != 0` plane short-circuits the
                        // `&&` before this (TMadnessWorld `err_null_m_wz1_*`).
                        let mm = m
                            .as_mut()
                            .expect("NullPointerException: Madness.m is null (Madness.java:671)");
                        var5 = var2 / 20.0 * mm.random();
                        o.planes[var4].oz[var6] = (o.planes[var4].oz[var6] as f32
                            - var5 * mm.sin(o.xz) * mm.cos(o.zy))
                            as i32;
                        o.planes[var4].ox[var6] = (o.planes[var4].ox[var6] as f32
                            + var5 * mm.cos(o.xz) * mm.cos(o.xy))
                            as i32;
                        self.hitmag = (self.hitmag as f32 + var5.abs()) as i32;
                    }
                }

                if var5 != 0.0 {
                    self.scorch_panel(var4, var5, false, o);
                }
            }
        }
    }

    /// `regy` (Madness.java:122-277) — the ground/roof dent, driven by
    /// wheel var1. Direction tags (var4/var5) come from the body zy/xy
    /// wrapped to 0..360; the head gate here is `var2 > 100` — no abs,
    /// unlike regz/regx. The nbsq == 0 path squashes the model pivots
    /// (oy around flipy + squash), the `var5 * var4 == -1` path only
    /// increments nbsq; the squash vertices path uses the `var15 / var16`
    /// int division.
    ///
    /// `m`/`rpd`/`xt` are `Option` for the original's nullable fields, like
    /// [`Madness::regx`]; the game always passes `Some`.
    pub fn regy(
        &mut self,
        var1: usize,
        mut var2: f32,
        o: &mut Conto,
        mut m: Option<&mut Medium>,
        rpd: Option<&mut Record>,
        xt: Option<&mut XtGraphics>,
    ) {
        var2 *= DAMMULT[self.cn];
        if var2 > 100.0 {
            // Madness.java:125 — `this.rpd.recy(...)`, the first Record deref.
            rpd.expect("NullPointerException: Madness.rpd is null (Madness.java:125)")
                .recy(var1, var2, self.mtouch, self.im);
            var2 -= 100.0;
            let mut var4 = 0i32;
            let mut var5 = 0i32;
            let mut var6 = o.zy;
            let mut var7 = o.xy;
            while var6 < 360 {
                var6 = var6.wrapping_add(360);
            }
            while var6 > 360 {
                var6 = var6.wrapping_sub(360);
            }
            if var6 < 210 && var6 > 150 {
                var4 = -1;
            }
            if var6 > 330 || var6 < 30 {
                var4 = 1;
            }
            while var7 < 360 {
                var7 = var7.wrapping_add(360);
            }
            while var7 > 360 {
                var7 = var7.wrapping_sub(360);
            }
            if var7 < 210 && var7 > 150 {
                var5 = -1;
            }
            if var7 > 330 || var7 < 30 {
                var5 = 1;
            }
            // Madness.java:165 — the angled crash report.
            if self.im == 0 || self.colidim {
                xt.expect("NullPointerException: Madness.xt is null (Madness.java:165)")
                    .crash(var2, var5 * var4);
            }
            if var5.wrapping_mul(var4) == 0 || self.mtouch {
                for var8 in 0..o.planes.len() {
                    let mut var9 = 0.0f32;
                    for var10 in 0..o.planes[var8].ox.len() {
                        if o.planes[var8].wz == 0
                            && self.py(
                                o.keyx[var1],
                                o.planes[var8].ox[var10],
                                o.keyz[var1],
                                o.planes[var8].oz[var10],
                            ) < CLRAD[self.cn]
                        {
                            // Madness.java:173-174 — the plane loop's Medium
                            // deref (nullable field, see regx).
                            let mm = m.as_mut().expect(
                                "NullPointerException: Madness.m is null (Madness.java:174)",
                            );
                            var9 = var2 / 20.0 * mm.random();
                            o.planes[var8].oz[var10] =
                                (o.planes[var8].oz[var10] as f32 + var9 * mm.sin(var6)) as i32;
                            o.planes[var8].ox[var10] =
                                (o.planes[var8].ox[var10] as f32 - (var9 * mm.sin(var7))) as i32;
                            self.hitmag = (self.hitmag as f32 + var9.abs()) as i32;
                        }
                    }

                    if var9 != 0.0 {
                        self.scorch_panel(var8, var9, true, o);
                    }
                }
            }

            if var5.wrapping_mul(var4) == -1 {
                if self.nbsq > 0 {
                    let mut var15 = 0i32;
                    let mut var16 = 1i32;
                    for var18 in 0..o.planes.len() {
                        let mut var11 = 0.0f32;
                        for var12 in 0..o.planes[var18].ox.len() {
                            if o.planes[var18].wz == 0 {
                                // Madness.java:244 — the squash branch's
                                // Medium deref.
                                var11 = var2 / 15.0
                                    * m.as_mut()
                                        .expect(
                                            "NullPointerException: Madness.m is null (Madness.java:244)",
                                        )
                                        .random();
                                if ((o.planes[var18].oy[var12] - FLIPY[self.cn] - self.squash)
                                    .wrapping_abs()
                                    < MSQUASH[self.cn] * 3
                                    || o.planes[var18].oy[var12] < FLIPY[self.cn] + self.squash)
                                    && self.squash < MSQUASH[self.cn]
                                {
                                    o.planes[var18].oy[var12] =
                                        (o.planes[var18].oy[var12] as f32 + var11) as i32;
                                    var15 = (var15 as f32 + var11) as i32;
                                    var16 += 1;
                                    self.hitmag = (self.hitmag as f32 + var11.abs()) as i32;
                                }
                            }
                        }

                        if o.planes[var18].glass {
                            o.planes[var18].gr -= 5;
                        } else if var11 != 0.0 {
                            o.planes[var18].bfase = (o.planes[var18].bfase as f32 + var11) as i32;
                        }

                        if var11.abs() >= 1.0 {
                            o.planes[var18].chip = 1;
                            o.planes[var18].ctmag = var11;
                        }
                    }

                    self.squash += var15 / var16;
                    self.nbsq = 0;
                } else {
                    self.nbsq += 1;
                }
            }
        }
    }

    /// `regz` (Madness.java:290-370) — the z-axis wall dent; same shape as
    /// regx with `oz += dmg·cos(xz)·cos(zy)` / `ox += dmg·sin(xz)·cos(xy)`
    /// and the abs-100 head gate. bfase gains |var5| here (regy adds the
    /// signed value).
    ///
    /// `m`/`rpd`/`xt` are `Option` for the original's nullable fields, like
    /// [`Madness::regx`]; the game always passes `Some`.
    pub fn regz(
        &mut self,
        var1: usize,
        mut var2: f32,
        o: &mut Conto,
        mut m: Option<&mut Medium>,
        rpd: Option<&mut Record>,
        xt: Option<&mut XtGraphics>,
    ) {
        var2 *= DAMMULT[self.cn];
        if var2.abs() > 100.0 {
            // Madness.java:293 — `this.rpd.recz(...)`, the first Record deref.
            rpd.expect("NullPointerException: Madness.rpd is null (Madness.java:293)")
                .recz(var1, var2, self.im);
            if var2 > 100.0 {
                var2 -= 100.0;
            }
            if var2 < -100.0 {
                var2 += 100.0;
            }
            // Madness.java:302-303 — the regz crash report.
            if self.im == 0 || self.colidim {
                xt.expect("NullPointerException: Madness.xt is null (Madness.java:303)")
                    .crash(var2, 0);
            }
            for var4 in 0..o.planes.len() {
                let mut var5 = 0.0f32;
                for var6 in 0..o.planes[var4].ox.len() {
                    if o.planes[var4].wz == 0
                        && self.py(
                            o.keyx[var1],
                            o.planes[var4].ox[var6],
                            o.keyz[var1],
                            o.planes[var4].oz[var6],
                        ) < CLRAD[self.cn]
                    {
                        // Madness.java:309-310 — the plane loop's only Medium
                        // deref (the `wz != 0` short-circuit leaves it inert).
                        let mm = m
                            .as_mut()
                            .expect("NullPointerException: Madness.m is null (Madness.java:310)");
                        var5 = var2 / 20.0 * mm.random();
                        o.planes[var4].oz[var6] = (o.planes[var4].oz[var6] as f32
                            + var5 * mm.cos(o.xz) * mm.cos(o.zy))
                            as i32;
                        o.planes[var4].ox[var6] = (o.planes[var4].ox[var6] as f32
                            + var5 * mm.sin(o.xz) * mm.cos(o.xy))
                            as i32;
                        self.hitmag = (self.hitmag as f32 + var5.abs()) as i32;
                    }
                }

                if var5 != 0.0 {
                    self.scorch_panel(var4, var5, false, o);
                }
            }
        }
    }

    /// The damage ladder shared by all three reg* paths (the identical
    /// chip/HSB block, Madness.java:181-230/318-367/678-727): chip markers,
    /// the shade thresholds guarded by `!nocol && !glass`, `bfase += dmg`
    /// AFTER the thresholds (signed in regy, absolute in regz/regx), the
    /// getHSBColor regrade last, and the glass `gr −= |dmg·1.5|` penalty
    /// for every panel with `var9 != 0`.
    fn scorch_panel(&mut self, var4: usize, var5: f32, signed_bfase: bool, o: &mut Conto) {
        let p = &mut o.planes[var4];
        if var5.abs() >= 1.0 {
            p.chip = 1;
            p.ctmag = var5;
        }

        if !p.nocol && !p.glass {
            if p.bfase > 20 && (p.hsb[1] as f64) > 0.2 {
                p.hsb[1] = 0.2;
            }
            if p.bfase > 30 {
                if (p.hsb[2] as f64) < 0.5 {
                    p.hsb[2] = 0.5;
                }
                if (p.hsb[1] as f64) > 0.1 {
                    p.hsb[1] = 0.1;
                }
            }
            if p.bfase > 40 {
                p.hsb[1] = 0.05;
            }
            if p.bfase > 50 {
                if (p.hsb[2] as f64) > 0.8 {
                    p.hsb[2] = 0.8;
                }
                p.hsb[0] = 0.075;
                p.hsb[1] = 0.05;
            }
            if p.bfase > 60 {
                p.hsb[0] = 0.05;
            }

            p.bfase = (p.bfase as f32 + if signed_bfase { var5 } else { var5.abs() }) as i32;
            // Madness.java:220/357/717 — the `new Color(c[0], c[1], c[2])`
            // statement is dead in the original; the observable step is the
            // Color.getHSBColor writeback (JDK port in color.rs).
            p.c = crate::color::hsb_to_rgb(p.hsb[0], p.hsb[1], p.hsb[2]);
        }

        if p.glass {
            p.gr = (p.gr as f64 - ((var5 as f64) * 1.5).abs()) as i32;
        }
    }

    /// `drive()` — first seam (Madness.java:732-1179): the orientation
    /// head, the air-trick inputs (`loop == 2` spins and the `loop == -1`
    /// landing recovery), the gear-table acceleration/brake machine over
    /// `ACELF`/`SWITS`, the wheel-spin wobble (`wzy`), steering (`wxz`)
    /// and the steer application (`var53`/`fxz`). The ground contact,
    /// suspension springs, wall hits and the wheel-driven body
    /// reconstruction (Madness.java:1181-2060) continue in later slices;
    /// the seam sits right before the `mxz`-style camera-poling ladder
    /// (Madness.java:1159-1179 is included here).
    ///
    /// Faithful details carried over verbatim:
    /// - gear gates are `swits[cn][i] / 2 + var49 * swits[cn][i] / 196`
    ///   (== swits·(0.5 + power/196): full-power frames gate at the raw
    ///   table values 50/180/280 for car 0), reverse caps at `swits[cn][1]`,
    ///   and `handb[cn] / 2` int-divides before its float cast;
    /// - the `dcomp` decay condition checks `ucomp > -2.0F` — the
    ///   original's copy-paste quirk (Madness.java:886), kept;
    /// - `var53 = (int)(3600.0F / (speed * speed))` saturates to
    ///   `i32::MAX` at standstill (f32 inf → (int) max on both platforms)
    ///   before its minimum-5 clamp, keeping the steer division inert;
    /// - `Math.abs(i32::MIN)` stays negative (`wrapping_abs`) and the
    ///   >270 spirals wrap (`wrapping_sub`).
    ///
    /// `xt` is the `XtGraphics` ref the original holds.
    /// It voices crash/skid/carfixed; eight params like `fase0`.
    #[allow(clippy::too_many_arguments)]
    pub fn drive(
        &mut self,
        ctl: &mut Control,
        o: &mut Conto,
        t: &Trackers,
        cp: &mut CheckPoints,
        m: &mut Medium,
        rpd: &mut Record,
        xt: &mut XtGraphics,
    ) {
        let mut var5 = 1i32;
        let mut var6 = 1i32;
        let mut var7 = false;
        let mut var8 = false;
        let mut var9 = false;
        self.capsized = false;

        // Madness.java:739-761 — flip detection off the persisted pitch/roll
        let mut var10 = self.pzy.wrapping_abs();
        while var10 > 270 {
            var10 = var10.wrapping_sub(360);
        }
        var10 = var10.wrapping_abs();
        if var10 > 90 {
            var7 = true;
        }

        let mut var11 = false;
        let mut var12 = self.pxy.wrapping_abs();
        while var12 > 270 {
            var12 = var12.wrapping_sub(360);
        }
        var12 = var12.wrapping_abs();
        if var12 > 90 {
            var11 = true;
            var6 = -1;
        }

        // Madness.java:763-780 — grat vs the capsized flip pivot
        let mut var13 = o.grat;
        if var7 {
            if var11 {
                var11 = false;
                var8 = true;
            } else {
                var11 = true;
                self.capsized = true;
            }
            var5 = -1;
        } else if var11 {
            self.capsized = true;
        }

        if self.capsized {
            var13 = FLIPY[self.cn] + self.squash;
        }

        ctl.zyinv = var7;

        // air forces: drift (var14/var15) and the atmosphere bounce (var16);
        // consumed by the body reconstruction in a later slice
        let mut var14 = 0.0f32;
        let mut var15 = 0.0f32;
        let mut var16 = 0.0f32;

        if self.mtouch {
            self.loop_ = 0;
        }

        if self.wtouch {
            if self.loop_ == 2 || self.loop_ == -1 {
                self.loop_ = -1;
                if ctl.left {
                    self.pl = true;
                }
                if ctl.right {
                    self.pr = true;
                }
                if ctl.up {
                    self.pu = true;
                }
                if ctl.down {
                    self.pd = true;
                }
            }

            self.ucomp = 0.0;
            self.dcomp = 0.0;
            self.lcomp = 0.0;
            self.rcomp = 0.0;
        }

        if ctl.handb {
            if !self.pushed {
                if !self.wtouch {
                    if self.loop_ == 0 {
                        self.loop_ = 1;
                    }
                } else if self.gtouch {
                    self.pushed = true;
                }
            }
        } else {
            self.pushed = false;
        }

        if self.loop_ == 1 {
            let var17 = (self.scy[0] + self.scy[1] + self.scy[2] + self.scy[3]) / 4.0;
            for slot in 0..4 {
                self.scy[slot] = var17;
            }
            self.loop_ = 2;
        }

        if !self.dest {
            if self.loop_ == 2 {
                // Madness.java:843-927 — airborne trick spins
                if ctl.up {
                    if self.ucomp == 0.0 {
                        self.ucomp = 10.0 + (self.scy[0] + 50.0) / 20.0;
                        if self.ucomp < 5.0 {
                            self.ucomp = 5.0;
                        }
                        if self.ucomp > 10.0 {
                            self.ucomp = 10.0;
                        }
                        self.ucomp *= AIRS[self.cn];
                    }

                    if self.ucomp < 20.0 {
                        self.ucomp = (self.ucomp as f64 + 0.5 * AIRS[self.cn] as f64) as f32;
                    }

                    var14 = (-AIRC[self.cn] as f32) * m.sin(o.xz) * var6 as f32;
                    var15 = (AIRC[self.cn] as f32) * m.cos(o.xz) * var6 as f32;
                } else if self.ucomp != 0.0 && self.ucomp > -2.0 {
                    self.ucomp = (self.ucomp as f64 - 0.5 * AIRS[self.cn] as f64) as f32;
                }

                if ctl.down {
                    if self.dcomp == 0.0 {
                        self.dcomp = 10.0 + (self.scy[0] + 50.0) / 20.0;
                        if self.dcomp < 5.0 {
                            self.dcomp = 5.0;
                        }
                        if self.dcomp > 10.0 {
                            self.dcomp = 10.0;
                        }
                        self.dcomp *= AIRS[self.cn];
                    }

                    if self.dcomp < 20.0 {
                        self.dcomp = (self.dcomp as f64 + 0.5 * AIRS[self.cn] as f64) as f32;
                    }

                    var16 = -AIRC[self.cn] as f32;
                } else if self.dcomp != 0.0 && self.ucomp > -2.0 {
                    self.dcomp = (self.dcomp as f64 - 0.5 * AIRS[self.cn] as f64) as f32;
                }

                if ctl.left {
                    if self.lcomp == 0.0 {
                        self.lcomp = 5.0;
                    }

                    if self.lcomp < 20.0 {
                        self.lcomp += 2.0 * AIRS[self.cn];
                    }

                    var14 = (-AIRC[self.cn] as f32) * m.cos(o.xz) * var5 as f32;
                    var15 = (-AIRC[self.cn] as f32) * m.sin(o.xz) * var5 as f32;
                } else if self.lcomp > 0.0 {
                    self.lcomp -= 2.0 * AIRS[self.cn];
                }

                if ctl.right {
                    if self.rcomp == 0.0 {
                        self.rcomp = 5.0;
                    }

                    if self.rcomp < 20.0 {
                        self.rcomp += 2.0 * AIRS[self.cn];
                    }

                    var14 = (AIRC[self.cn] as f32) * m.cos(o.xz) * var5 as f32;
                    var15 = (AIRC[self.cn] as f32) * m.sin(o.xz) * var5 as f32;
                } else if self.rcomp > 0.0 {
                    self.rcomp -= 2.0 * AIRS[self.cn];
                }

                self.pzy = (self.pzy as f32 + (self.dcomp - self.ucomp) * m.cos(self.pxy)) as i32;
                if var7 {
                    o.xz = (o.xz as f32 + (self.dcomp - self.ucomp) * m.sin(self.pxy)) as i32;
                } else {
                    o.xz = (o.xz as f32 - (self.dcomp - self.ucomp) * m.sin(self.pxy)) as i32;
                }

                self.pxy = (self.pxy as f32 + (self.rcomp - self.lcomp)) as i32;
            } else {
                // Madness.java:928-990 — grounded throttle/brake/reverse
                let mut var49 = self.power;
                if var49 < 40.0 {
                    var49 = 40.0;
                }

                if self.im == 0 && self.power != 98.0 {
                    if cp.stage != 6 && cp.stage != 8 {
                        var49 = (var49 as f64 * 0.76) as f32;
                    } else if cp.stage != 6 {
                        var49 = (var49 as f64 * 0.9) as f32;
                    }
                }

                if ctl.down {
                    if self.speed > 0.0 {
                        self.speed -= (HANDB[self.cn] / 2) as f32;
                    } else {
                        // reverse: two gear gates, then the swits[cn][1] cap
                        let mut var51 = 0i32;
                        let mut var19 = 0;
                        while var19 < 2 {
                            if self.speed
                                <= -((SWITS[self.cn][var19] / 2) as f32
                                    + var49 * SWITS[self.cn][var19] as f32 / 196.0)
                            {
                                var51 += 1;
                            }
                            var19 += 1;
                        }

                        if var51 != 2 {
                            self.speed -= ACELF[self.cn][var51 as usize] / 2.0
                                + var49 * ACELF[self.cn][var51 as usize] / 196.0;
                        } else {
                            self.speed = -((SWITS[self.cn][1] / 2) as f32
                                + var49 * SWITS[self.cn][1] as f32 / 196.0);
                        }
                    }
                }

                if ctl.up {
                    if self.speed < 0.0 {
                        self.speed += HANDB[self.cn] as f32;
                    } else {
                        // three gear gates, then the swits[cn][2] cap
                        let mut var52 = 0i32;
                        let mut var54 = 0;
                        while var54 < 3 {
                            if self.speed
                                >= (SWITS[self.cn][var54] / 2) as f32
                                    + var49 * SWITS[self.cn][var54] as f32 / 196.0
                            {
                                var52 += 1;
                            }
                            var54 += 1;
                        }

                        if var52 != 3 {
                            // Madness.java:977 is `speed + acelf/2 + var49*acelf/196`
                            // with NO parentheses: left-to-right evaluation,
                            // i.e. `(speed + a/2) + var49*a/196`. Writing it
                            // as `speed += a/2 + var49*a/196` changes the
                            // rounding (1 ulp at TMadnessDrive's race T4).
                            self.speed = self.speed
                                + ACELF[self.cn][var52 as usize] / 2.0
                                + var49 * ACELF[self.cn][var52 as usize] / 196.0;
                        } else {
                            self.speed = (SWITS[self.cn][2] / 2) as f32
                                + var49 * SWITS[self.cn][2] as f32 / 196.0;
                        }
                    }
                }

                if ctl.handb && self.speed.abs() > HANDB[self.cn] as f32 {
                    if self.speed < 0.0 {
                        self.speed += HANDB[self.cn] as f32;
                    } else {
                        self.speed -= HANDB[self.cn] as f32;
                    }
                }

                // Madness.java:992-1073 — the loop==-1 landing recovery
                if self.loop_ == -1 && o.y < 100 {
                    if ctl.left {
                        if !self.pl {
                            if self.lcomp == 0.0 {
                                self.lcomp = 5.0 * AIRS[self.cn];
                            }

                            if self.lcomp < 20.0 {
                                self.lcomp += 2.0 * AIRS[self.cn];
                            }
                        }
                    } else {
                        if self.lcomp > 0.0 {
                            self.lcomp -= 2.0 * AIRS[self.cn];
                        }
                        self.pl = false;
                    }

                    if ctl.right {
                        if !self.pr {
                            if self.rcomp == 0.0 {
                                self.rcomp = 5.0 * AIRS[self.cn];
                            }

                            if self.rcomp < 20.0 {
                                self.rcomp += 2.0 * AIRS[self.cn];
                            }
                        }
                    } else {
                        if self.rcomp > 0.0 {
                            self.rcomp -= 2.0 * AIRS[self.cn];
                        }
                        self.pr = false;
                    }

                    if ctl.up {
                        if !self.pu {
                            if self.ucomp == 0.0 {
                                self.ucomp = 5.0 * AIRS[self.cn];
                            }

                            if self.ucomp < 20.0 {
                                self.ucomp += 2.0 * AIRS[self.cn];
                            }
                        }
                    } else {
                        if self.ucomp > 0.0 {
                            self.ucomp -= 2.0 * AIRS[self.cn];
                        }
                        self.pu = false;
                    }

                    if ctl.down {
                        if !self.pd {
                            if self.dcomp == 0.0 {
                                self.dcomp = 5.0 * AIRS[self.cn];
                            }

                            if self.dcomp < 20.0 {
                                self.dcomp += 2.0 * AIRS[self.cn];
                            }
                        }
                    } else {
                        if self.dcomp > 0.0 {
                            self.dcomp -= 2.0 * AIRS[self.cn];
                        }
                        self.pd = false;
                    }

                    self.pzy =
                        (self.pzy as f32 + (self.dcomp - self.ucomp) * m.cos(self.pxy)) as i32;
                    if var7 {
                        o.xz = (o.xz as f32 + (self.dcomp - self.ucomp) * m.sin(self.pxy)) as i32;
                    } else {
                        o.xz = (o.xz as f32 - (self.dcomp - self.ucomp) * m.sin(self.pxy)) as i32;
                    }

                    self.pxy = (self.pxy as f32 + (self.rcomp - self.lcomp)) as i32;
                }
            }
        }

        // Madness.java:1077-1089 — wheel-spin wobble from speed
        let mut var50 = 20.0 * self.speed / (154.0 * SIMAG[self.cn]);
        if var50 > 20.0 {
            var50 = 20.0;
        }

        o.wzy = (o.wzy as f32 - var50) as i32;
        if o.wzy < -45 {
            o.wzy += 45;
        }
        if o.wzy > 45 {
            o.wzy -= 45;
        }

        // Madness.java:1091-1131 — steering ramp (turn per frame, ±36 clip)
        // and its two decay rates
        if ctl.right {
            o.wxz -= TURN[self.cn];
            if o.wxz < -36 {
                o.wxz = -36;
            }
        }

        if ctl.left {
            o.wxz += TURN[self.cn];
            if o.wxz > 36 {
                o.wxz = 36;
            }
        }

        if o.wxz != 0 && !ctl.left && !ctl.right {
            if (self.speed as f64).abs() < 10.0 {
                if o.wxz.wrapping_abs() == 1 {
                    o.wxz = 0;
                }
                if o.wxz > 0 {
                    o.wxz -= 1;
                }
                if o.wxz < 0 {
                    o.wxz += 1;
                }
            } else {
                if o.wxz.wrapping_abs() < TURN[self.cn] * 2 {
                    o.wxz = 0;
                }
                if o.wxz > 0 {
                    o.wxz -= TURN[self.cn] * 2;
                }
                if o.wxz < 0 {
                    o.wxz += TURN[self.cn] * 2;
                }
            }
        }

        // Madness.java:1133-1141 — the steer-division interval; f32 inf at
        // standstill casts to i32::MAX (saturating casts match (int)), then
        // the minimum clamp keeps it from dividing by zero
        let mut var53 = (3600.0f32 / (self.speed * self.speed)) as i32;
        if var53 < 5 {
            var53 = 5;
        }
        if self.speed < 0.0 {
            var53 = var53.wrapping_neg();
        }

        // Madness.java:1142-1157 — steer on the ground; fxz persists when
        // the wheels lose touch
        if self.wtouch {
            if !self.capsized {
                if !ctl.handb {
                    self.fxz = o.wxz / (var53.wrapping_mul(3));
                } else {
                    self.fxz = o.wxz / var53;
                }

                o.xz += o.wxz / var53;
            }

            self.wtouch = false;
            self.gtouch = false;
        } else {
            o.xz += self.fxz;
        }

        // Madness.java:1159-1179 — the camera-pan ladder toward mxz (mxz is
        // written from the suspension vector in a later slice)
        if self.speed > 30.0 || self.speed < -100.0 {
            while (self.mxz - self.cxz).wrapping_abs() > 180 {
                if self.cxz > self.mxz {
                    self.cxz -= 360;
                } else if self.cxz < self.mxz {
                    self.cxz += 360;
                }
            }

            if (self.mxz - self.cxz).wrapping_abs() < 30 {
                self.cxz = (self.cxz as f32 + (self.mxz - self.cxz) as f32 / 4.0) as i32;
            } else {
                if self.cxz > self.mxz {
                    self.cxz -= 10;
                }
                if self.cxz < self.mxz {
                    self.cxz += 10;
                }
            }
        }

        // == Madness.java:1181-1226 — wheel world positions =================
        // (x+keyx, grat+y, z+keyz) rotated by pxy/pzy/xz, the 7.0 scy
        // preload, the ±200 clamps around the four-wheel average, and the
        // displacement pass (scx/scz move by the panel average, scy by the
        // wheel's own bounce).
        let mut var55 = [0.0f32; 4];
        let mut var20_ = [0.0f32; 4];
        let mut var21 = [0.0f32; 4];
        for var22 in 0..4 {
            var55[var22] = (o.keyx[var22] + o.x) as f32;
            var21[var22] = (var13 + o.y) as f32;
            var20_[var22] = (o.z + o.keyz[var22]) as f32;
            self.scy[var22] += 7.0;
        }

        self.rot(m, &mut var55, &mut var21, o.x, o.y, self.pxy, 4);
        self.rot(m, &mut var21, &mut var20_, o.y, o.z, self.pzy, 4);
        self.rot(m, &mut var55, &mut var20_, o.x, o.z, o.xz, 4);

        let mut var56 = false;
        #[allow(unused_assignments)]
        let mut var23 = 0.0f64;
        let var25 = ((self.scx[0] + self.scx[1] + self.scx[2] + self.scx[3]) / 4.0) as i32;
        let var26 = ((self.scz[0] + self.scz[1] + self.scz[2] + self.scz[3]) / 4.0) as i32;
        // Madness.java:1226 — `var27 = 1` BEFORE the surface scan: with no
        // tracker under the car the original keeps the default road surface.
        let mut var27 = 1usize;

        for wheel in 0..4 {
            if self.scx[wheel] - (var25 as f32) > 200.0 {
                self.scx[wheel] = 200.0 + var25 as f32;
            }
            if self.scx[wheel] - (var25 as f32) < -200.0 {
                self.scx[wheel] = var25 as f32 - 200.0;
            }
            if self.scz[wheel] - (var26 as f32) > 200.0 {
                self.scz[wheel] = 200.0 + var26 as f32;
            }
            if self.scz[wheel] - (var26 as f32) < -200.0 {
                self.scz[wheel] = var26 as f32 - 200.0;
            }
        }

        for wheel in 0..4 {
            var21[wheel] += self.scy[wheel];
            var55[wheel] += (self.scx[0] + self.scx[1] + self.scx[2] + self.scx[3]) / 4.0;
            var20_[wheel] += (self.scz[0] + self.scz[1] + self.scz[2] + self.scz[3]) / 4.0;
        }

        // Madness.java:1230-1237 — the surface pick under the car body
        for var28 in 0..t.nt {
            if t.zy[var28].wrapping_abs() != 90
                && t.xy[var28].wrapping_abs() != 90
                && (o.x - t.x[var28]).wrapping_abs() < t.radx[var28]
                && (o.z - t.z[var28]).wrapping_abs() < t.radz[var28]
            {
                var27 = t.skd[var28] as usize;
            }
        }

        // == Madness.java:1239-1411 — the mtouch grip/skim/suspension block =
        if self.mtouch {
            let mut var64 = GRIP[self.cn];
            var64 -= (self.txz - o.xz).wrapping_abs() as f32 * self.speed / 250.0;
            if ctl.handb {
                var64 -= ((self.txz - o.xz).wrapping_abs().wrapping_mul(4)) as f32;
            }

            if var64 < GRIP[self.cn] {
                if self.skid != 2 {
                    self.skid = 1;
                }
                self.speed -= self.speed / 100.0;
            } else if self.skid == 1 {
                self.skid = 2;
            }

            if var27 == 1 {
                var64 = (var64 as f64 * 0.75) as f32;
            }
            if var27 == 2 {
                var64 = (var64 as f64 * 0.55) as f32;
            }

            let mut var29 = ((self.speed * m.sin(o.xz) * m.cos(self.pzy)) as i32).wrapping_neg();
            let mut var30 = (self.speed * m.cos(o.xz) * m.cos(self.pzy)) as i32;
            let mut var31 = ((self.speed * m.sin(self.pzy)) as i32).wrapping_neg();
            if self.capsized || self.dest || cp.haltall {
                var29 = 0;
                var30 = 0;
                var31 = 0;
                var64 = GRIP[self.cn] / 5.0;
                if self.speed > 0.0 {
                    self.speed -= 2.0;
                } else {
                    self.speed += 2.0;
                }
            }

            if self.speed.abs() > DRAG[self.cn] {
                if self.speed > 0.0 {
                    self.speed -= DRAG[self.cn];
                } else {
                    self.speed += DRAG[self.cn];
                }
            } else {
                self.speed = 0.0;
            }

            if var64 < 1.0 {
                var64 = 1.0;
            }

            // the suspension walk toward the speed vector, then the dust
            // sites with the dcnt wear counter
            let mut var32 = 0.0f32;
            let mut var33 = 0.0f32;
            let mut var34 = 0usize;
            loop {
                if (self.scx[var34] - var29 as f32).abs() > var64 {
                    if self.scx[var34] < var29 as f32 {
                        self.scx[var34] += var64;
                    } else {
                        self.scx[var34] -= var64;
                    }
                } else {
                    self.scx[var34] = var29 as f32;
                }

                if (self.scz[var34] - var30 as f32).abs() > var64 {
                    if self.scz[var34] < var30 as f32 {
                        self.scz[var34] += var64;
                    } else {
                        self.scz[var34] -= var64;
                    }
                } else {
                    self.scz[var34] = var30 as f32;
                }

                if (self.scy[var34] - var31 as f32).abs() > var64 {
                    if self.scy[var34] < var31 as f32 {
                        self.scy[var34] += var64;
                    } else {
                        self.scy[var34] -= var64;
                    }
                } else {
                    self.scy[var34] = var31 as f32;
                }

                if var64 < GRIP[self.cn] {
                    if self.txz != o.xz {
                        self.dcnt += 1;
                    } else if self.dcnt != 0 {
                        self.dcnt = 0;
                    }

                    if !((self.dcnt as f32) > 40.0 * var64 / GRIP[self.cn]) && !self.capsized {
                        if var27 == 1 && (m.random() as f64) > 0.85 {
                            o.dust(
                                var34 as i32,
                                var55[var34],
                                var21[var34],
                                var20_[var34],
                                self.scx[var34],
                                self.scz[var34],
                                1.1 * SIMAG[self.cn],
                                false,
                                self.tilt as i32,
                            );
                        }

                        if (var27 == 2 || var27 == 3) && (m.random() as f64) > 0.7 {
                            o.dust(
                                var34 as i32,
                                var55[var34],
                                var21[var34],
                                var20_[var34],
                                self.scx[var34],
                                self.scz[var34],
                                1.15 * SIMAG[self.cn],
                                false,
                                self.tilt as i32,
                            );
                        }
                    } else {
                        let var35 = if var27 != 0 { 1.2f32 } else { 1.0f32 };
                        if (m.random() as f64) > 0.75 {
                            o.dust(
                                var34 as i32,
                                var55[var34],
                                var21[var34],
                                var20_[var34],
                                self.scx[var34],
                                self.scz[var34],
                                var35 * SIMAG[self.cn],
                                true,
                                self.tilt as i32,
                            );
                            // Madness.java:1357-1359 — the player skid
                            // report (uncapsized only).
                            if self.im == 0 && !self.capsized {
                                let v = (self.scx[var34] * self.scx[var34]
                                    + self.scz[var34] * self.scz[var34])
                                    as f64;
                                xt.skid(var27 as i32, v.sqrt() as f32);
                            }
                        }
                    }
                } else if self.dcnt != 0 {
                    self.dcnt -= 2;
                    if self.dcnt < 0 {
                        self.dcnt = 0;
                    }
                }

                if var27 == 3 {
                    let var84 = (m.random() * 4.0) as i32;
                    self.scy[var84 as usize] = ((-100.0f64
                        * m.random() as f64
                        * (self.speed / SWITS[self.cn][2] as f32) as f64)
                        * (BOUNCE[self.cn] as f64 - 0.3))
                        as f32;
                }

                var32 += self.scx[var34];
                var33 += self.scz[var34];
                var34 += 1;
                if var34 >= 4 {
                    break;
                }
            }

            // Madness.java:1380-1408 — heading reconstruction from the
            // suspension vector and the skid-2 exit speed
            self.txz = o.xz;
            var5 = if var32 > 0.0 { -1 } else { 1 };
            var23 = var33 as f64 / ((var32 * var32 + var33 * var33) as f64).sqrt();
            self.mxz = (var23.acos() / (std::f64::consts::PI / 180.0) * var5 as f64) as i32;
            if self.skid == 2 {
                if !self.capsized {
                    var32 /= 4.0;
                    var33 /= 4.0;
                    let var76 = ((var32 * var32 + var33 * var33) as f64).sqrt() as f32;
                    if var8 {
                        self.speed = -(var76 * m.cos(self.mxz.wrapping_sub(o.xz)));
                    } else {
                        self.speed = var76 * m.cos(self.mxz.wrapping_sub(o.xz));
                    }
                }
                self.skid = 0;
            }

            if self.capsized && var32 == 0.0 && var33 == 0.0 {
                var27 = 0;
            }

            self.mtouch = false;
            var56 = true;
        } else if self.skid != 2 {
            self.skid = 2;
        }

        // == Madness.java:1413-1464 — the global-plane contact ==============
        // a wheel deeper than y=245 lands at 250 (wtouch + gtouch), with
        // the landing dust, the own-wheel bounce and the pull-others
        // loop — that loop's subtraction happens after `var21[w] = 250`,
        // so it computes 0 and is a faithful no-op.
        let mut var66 = 0i32;
        let mut var67 = [false; 4];
        let mut var68_u = 0usize;
        loop {
            let var68 = var68_u;
            if var21[var68] > 245.0 {
                var66 += 1;
                self.wtouch = true;
                self.gtouch = true;
                if !var56 && self.scy[var68] != 7.0 {
                    let mut var70 = self.scy[var68] / 333.33;
                    if (var70 as f64) > 0.3 {
                        var70 = 0.3;
                    }
                    var70 = if var27 == 0 {
                        ((var70 as f64) + 1.1) as f32
                    } else {
                        ((var70 as f64) + 1.2) as f32
                    };
                    o.dust(
                        var68 as i32,
                        var55[var68],
                        var21[var68],
                        var20_[var68],
                        self.scx[var68],
                        self.scz[var68],
                        var70 * SIMAG[self.cn],
                        true,
                        0,
                    );
                }

                var21[var68] = 250.0;
                for var72 in 0..4 {
                    if var68 != var72 && var21[var72] <= 245.0 {
                        var21[var72] -= var21[var68] - 250.0;
                    }
                }

                let var73 = (m.sin(self.pxy)).abs() + (m.sin(self.pzy)).abs();
                let mut var74 = var73 / 3.0;
                if (var74 as f64) > 0.4 {
                    var74 = 0.4;
                }

                let mut var75 = var74 + BOUNCE[self.cn];
                if (var75 as f64) < 1.1 {
                    var75 = 1.1;
                }

                self.regy(
                    var68,
                    (self.scy[var68] * var75).abs(),
                    o,
                    Some(&mut *m),
                    Some(&mut *rpd),
                    Some(&mut *xt),
                );
                if self.scy[var68] > 0.0 {
                    self.scy[var68] -= (self.scy[var68] * var75).abs();
                }
            }

            var67[var68] = false;
            var68_u += 1;
            if var68_u >= 4 {
                break;
            }
        }

        // == Madness.java:1466-1753 — the tracker contact loop ==============
        // each tracker box-tests the four surviving wheels; a hit lands
        // one of flat-land / ±90 wall (regz·regx damage) / twisted-plane
        // cases. var78 counts ramp contacts per tracker, var80 flat-tracker
        // contacts per tracker, and var68 (renamed var68_cnt here) counts
        // twisted contacts across the whole loop — ==4 forces mtouch.
        let mut var68_cnt = 0i32;
        for var76 in 0..t.nt {
            let mut var78 = 0i32;
            let mut var80 = 0i32;
            let mut var82 = 0usize;
            loop {
                if !var67[var82]
                    && var55[var82] > (t.x[var76] - t.radx[var76]) as f32
                    && var55[var82] < (t.x[var76] + t.radx[var76]) as f32
                    && var20_[var82] > (t.z[var76] - t.radz[var76]) as f32
                    && var20_[var82] < (t.z[var76] + t.radz[var76]) as f32
                    && var21[var82] > (t.y[var76] - t.rady[var76]) as f32
                    && var21[var82] < (t.y[var76] + t.rady[var76]) as f32
                {
                    // Madness.java:1481-1526 — flat tracker landing
                    if t.xy[var76] == 0
                        && t.zy[var76] == 0
                        && t.y[var76] != 250
                        && var21[var82] > (t.y[var76] - 5) as f32
                    {
                        var80 += 1;
                        self.wtouch = true;
                        self.gtouch = true;
                        if !var56 && self.scy[var82] != 7.0 {
                            let mut var85 = self.scy[var82] / 333.33;
                            if (var85 as f64) > 0.3 {
                                var85 = 0.3;
                            }
                            var85 = if var27 == 0 {
                                ((var85 as f64) + 1.1) as f32
                            } else {
                                ((var85 as f64) + 1.2) as f32
                            };
                            o.dust(
                                var82 as i32,
                                var55[var82],
                                var21[var82],
                                var20_[var82],
                                self.scx[var82],
                                self.scz[var82],
                                var85 * SIMAG[self.cn],
                                true,
                                0,
                            );
                        }

                        // the seed-5 pull loop recomputes 0 (same no-op as
                        // the global-plane case), kept in order
                        var21[var82] = t.y[var76] as f32;
                        for var87 in 0..4 {
                            if var82 != var87 && var21[var87] <= (t.y[var76] - 5) as f32 {
                                var21[var87] -= var21[var82] - t.y[var76] as f32;
                            }
                        }

                        let var88 = (m.sin(self.pxy)).abs() + (m.sin(self.pzy)).abs();
                        let mut var89 = var88 / 3.0;
                        if (var89 as f64) > 0.4 {
                            var89 = 0.4;
                        }
                        let mut var90 = var89 + BOUNCE[self.cn];
                        if (var90 as f64) < 1.1 {
                            var90 = 1.1;
                        }
                        self.regy(
                            var82,
                            (self.scy[var82] * var90).abs(),
                            o,
                            Some(&mut *m),
                            Some(&mut *rpd),
                            Some(&mut *xt),
                        );
                        if self.scy[var82] > 0.0 {
                            self.scy[var82] -= (self.scy[var82] * var90).abs();
                        }
                        var67[var82] = true;
                    }

                    // Madness.java:1528-1559 — north wall (zy == −90)
                    if t.zy[var76] == -90
                        && var20_[var82] < (t.z[var76] + t.radz[var76]) as f32
                        && self.scz[var82] < 0.0
                    {
                        var20_[var82] = (t.z[var76] + t.radz[var76]) as f32;
                        for var91 in 0..4 {
                            if var82 != var91
                                && var20_[var91] >= (t.z[var76] + t.radz[var76]) as f32
                            {
                                var20_[var91] -=
                                    var20_[var82] - (t.z[var76] + t.radz[var76]) as f32;
                            }
                        }
                        let var92 = (m.cos(self.pxy)).abs() + (m.cos(self.pzy)).abs();
                        let mut var93 = var92 / 4.0;
                        if (var93 as f64) > 0.3 {
                            var93 = 0.3;
                        }
                        if var56 {
                            var93 = 0.0;
                        }
                        let mut var94 = (var93 as f64 + (BOUNCE[self.cn] as f64 - 0.2)) as f32;
                        if (var94 as f64) < 1.1 {
                            var94 = 1.1;
                        }
                        self.regz(
                            var82,
                            (self.scz[var82] * var94 * t.dam[var76] as f32).abs(),
                            o,
                            Some(&mut *m),
                            Some(&mut *rpd),
                            Some(&mut *xt),
                        );
                        self.scz[var82] += (self.scz[var82] * var94).abs();
                        self.skid = 2;
                        var9 = true;
                        var67[var82] = true;
                        ctl.wall = var76 as i32;
                    }

                    // Madness.java:1561-1592 — south wall (zy == +90)
                    if t.zy[var76] == 90
                        && var20_[var82] > (t.z[var76] - t.radz[var76]) as f32
                        && self.scz[var82] > 0.0
                    {
                        var20_[var82] = (t.z[var76] - t.radz[var76]) as f32;
                        for var95 in 0..4 {
                            if var82 != var95
                                && var20_[var95] <= (t.z[var76] - t.radz[var76]) as f32
                            {
                                var20_[var95] -=
                                    var20_[var82] - (t.z[var76] - t.radz[var76]) as f32;
                            }
                        }
                        let var96 = (m.cos(self.pxy)).abs() + (m.cos(self.pzy)).abs();
                        let mut var97 = var96 / 4.0;
                        if (var97 as f64) > 0.3 {
                            var97 = 0.3;
                        }
                        if var56 {
                            var97 = 0.0;
                        }
                        let mut var98 = (var97 as f64 + (BOUNCE[self.cn] as f64 - 0.2)) as f32;
                        if (var98 as f64) < 1.1 {
                            var98 = 1.1;
                        }
                        self.regz(
                            var82,
                            -((self.scz[var82] * var98 * t.dam[var76] as f32).abs()),
                            o,
                            Some(&mut *m),
                            Some(&mut *rpd),
                            Some(&mut *xt),
                        );
                        self.scz[var82] -= (self.scz[var82] * var98).abs();
                        self.skid = 2;
                        var9 = true;
                        var67[var82] = true;
                        ctl.wall = var76 as i32;
                    }

                    // Madness.java:1594-1625 — west wall (xy == −90)
                    if t.xy[var76] == -90
                        && var55[var82] < (t.x[var76] + t.radx[var76]) as f32
                        && self.scx[var82] < 0.0
                    {
                        var55[var82] = (t.x[var76] + t.radx[var76]) as f32;
                        for var99 in 0..4 {
                            if var82 != var99 && var55[var99] >= (t.x[var76] + t.radx[var76]) as f32
                            {
                                var55[var99] -= var55[var82] - (t.x[var76] + t.radx[var76]) as f32;
                            }
                        }
                        let var100 = (m.cos(self.pxy)).abs() + (m.cos(self.pzy)).abs();
                        let mut var101 = var100 / 4.0;
                        if (var101 as f64) > 0.3 {
                            var101 = 0.3;
                        }
                        if var56 {
                            var101 = 0.0;
                        }
                        let mut var102 = (var101 as f64 + (BOUNCE[self.cn] as f64 - 0.2)) as f32;
                        if (var102 as f64) < 1.1 {
                            var102 = 1.1;
                        }
                        self.regx(
                            var82,
                            (self.scx[var82] * var102 * t.dam[var76] as f32).abs(),
                            o,
                            Some(&mut *m),
                            Some(&mut *rpd),
                            Some(&mut *xt),
                        );
                        self.scx[var82] += (self.scx[var82] * var102).abs();
                        self.skid = 2;
                        var9 = true;
                        var67[var82] = true;
                        ctl.wall = var76 as i32;
                    }

                    // Madness.java:1627-1658 — east wall (xy == +90)
                    if t.xy[var76] == 90
                        && var55[var82] > (t.x[var76] - t.radx[var76]) as f32
                        && self.scx[var82] > 0.0
                    {
                        var55[var82] = (t.x[var76] - t.radx[var76]) as f32;
                        for var103 in 0..4 {
                            if var82 != var103
                                && var55[var103] <= (t.x[var76] - t.radx[var76]) as f32
                            {
                                var55[var103] -= var55[var82] - (t.x[var76] - t.radx[var76]) as f32;
                            }
                        }
                        let var104 = (m.cos(self.pxy)).abs() + (m.cos(self.pzy)).abs();
                        let mut var105 = var104 / 4.0;
                        if (var105 as f64) > 0.3 {
                            var105 = 0.3;
                        }
                        if var56 {
                            var105 = 0.0;
                        }
                        let mut var106 = (var105 as f64 + (BOUNCE[self.cn] as f64 - 0.2)) as f32;
                        if (var106 as f64) < 1.1 {
                            var106 = 1.1;
                        }
                        self.regx(
                            var82,
                            -((self.scx[var82] * var106 * t.dam[var76] as f32).abs()),
                            o,
                            Some(&mut *m),
                            Some(&mut *rpd),
                            Some(&mut *xt),
                        );
                        self.scx[var82] -= (self.scx[var82] * var106).abs();
                        self.skid = 2;
                        var9 = true;
                        var67[var82] = true;
                        ctl.wall = var76 as i32;
                    }

                    // Madness.java:1660-1698 — twisted zy plane
                    if t.zy[var76] != 0 && t.zy[var76] != 90 && t.zy[var76] != -90 {
                        let var107 = 90i32.wrapping_add(t.zy[var76]);
                        let mut var36 = 1.0 + (50 - t.zy[var76].wrapping_abs()) as f32 / 30.0;
                        if var36 < 1.0 {
                            var36 = 1.0;
                        }

                        let var37 = t.y[var76] as f32
                            + ((var21[var82] - t.y[var76] as f32) * m.cos(var107)
                                - (var20_[var82] - t.z[var76] as f32) * m.sin(var107));
                        let mut var38 = t.z[var76] as f32
                            + (var21[var82] - t.y[var76] as f32) * m.sin(var107)
                            + (var20_[var82] - t.z[var76] as f32) * m.cos(var107);
                        if var38 > t.z[var76] as f32 && var38 < (t.z[var76] + 200) as f32 {
                            self.scy[var82] -= (var38 - t.z[var76] as f32) / var36;
                            var38 = t.z[var76] as f32;
                        }

                        if var38 > (t.z[var76] - 30) as f32 {
                            if t.skd[var76] == 2 {
                                var78 += 1;
                            } else {
                                var68_cnt += 1;
                            }
                            self.wtouch = true;
                            self.gtouch = false;
                            if !var56 && var27 != 0 {
                                o.dust(
                                    var82 as i32,
                                    var55[var82],
                                    var21[var82],
                                    var20_[var82],
                                    self.scx[var82],
                                    self.scz[var82],
                                    1.4 * SIMAG[self.cn],
                                    true,
                                    0,
                                );
                            }
                        }

                        var21[var82] = t.y[var76] as f32
                            + ((var37 - t.y[var76] as f32) * m.cos(var107.wrapping_neg())
                                - (var38 - t.z[var76] as f32) * m.sin(var107.wrapping_neg()));
                        var20_[var82] = t.z[var76] as f32
                            + (var37 - t.y[var76] as f32) * m.sin(var107.wrapping_neg())
                            + (var38 - t.z[var76] as f32) * m.cos(var107.wrapping_neg());
                        var67[var82] = true;
                    }

                    // Madness.java:1700-1738 — twisted xy plane
                    if t.xy[var76] != 0 && t.xy[var76] != 90 && t.xy[var76] != -90 {
                        let var108 = 90i32.wrapping_add(t.xy[var76]);
                        let mut var116 = 1.0 + (50 - t.xy[var76].wrapping_abs()) as f32 / 30.0;
                        if var116 < 1.0 {
                            var116 = 1.0;
                        }

                        let var118 = t.y[var76] as f32
                            + ((var21[var82] - t.y[var76] as f32) * m.cos(var108)
                                - (var55[var82] - t.x[var76] as f32) * m.sin(var108));
                        let mut var121 = t.x[var76] as f32
                            + (var21[var82] - t.y[var76] as f32) * m.sin(var108)
                            + (var55[var82] - t.x[var76] as f32) * m.cos(var108);
                        if var121 > t.x[var76] as f32 && var121 < (t.x[var76] + 200) as f32 {
                            self.scy[var82] -= (var121 - t.x[var76] as f32) / var116;
                            var121 = t.x[var76] as f32;
                        }

                        if var121 > (t.x[var76] - 30) as f32 {
                            if t.skd[var76] == 2 {
                                var78 += 1;
                            } else {
                                var68_cnt += 1;
                            }
                            self.wtouch = true;
                            self.gtouch = false;
                            if !var56 && var27 != 0 {
                                o.dust(
                                    var82 as i32,
                                    var55[var82],
                                    var21[var82],
                                    var20_[var82],
                                    self.scx[var82],
                                    self.scz[var82],
                                    1.4 * SIMAG[self.cn],
                                    true,
                                    0,
                                );
                            }
                        }

                        var21[var82] = t.y[var76] as f32
                            + ((var118 - t.y[var76] as f32) * m.cos(var108.wrapping_neg())
                                - (var121 - t.x[var76] as f32) * m.sin(var108.wrapping_neg()));
                        var55[var82] = t.x[var76] as f32
                            + (var118 - t.y[var76] as f32) * m.sin(var108.wrapping_neg())
                            + (var121 - t.x[var76] as f32) * m.cos(var108.wrapping_neg());
                        var67[var82] = true;
                    }
                }

                var82 += 1;
                if var82 >= 4 {
                    break;
                }
            }

            if var78 == 4 {
                self.mtouch = true;
            }

            if var80 == 4 {
                var66 = 4;
            }
        }

        if var68_cnt == 4 {
            self.mtouch = true;
        }

        // == Madness.java:1755-1837 — pitch/roll recovery from the wheel
        // diagonal spreads (the acos of the wheel separation ratio) ========
        let mut var77 = 0i32;
        let mut var79 = 0i32;
        let mut var81 = 0i32;
        let mut var83 = 0i32;
        if self.scy[2] != self.scy[0] {
            var5 = if self.scy[2] < self.scy[0] { -1 } else { 1 };
            var23 = (((var20_[0] - var20_[2]) * (var20_[0] - var20_[2])
                + (var21[0] - var21[2]) * (var21[0] - var21[2])
                + (var55[0] - var55[2]) * (var55[0] - var55[2])) as f64)
                .sqrt()
                / (o.keyz[0]
                    .wrapping_abs()
                    .wrapping_add(o.keyz[2].wrapping_abs()) as f64);
            if var23 >= 0.9998 {
                var77 = var5;
            } else {
                var77 = (var23.acos() / (std::f64::consts::PI / 180.0) * var5 as f64) as i32;
            }
        }

        if self.scy[3] != self.scy[1] {
            var5 = if self.scy[3] < self.scy[1] { -1 } else { 1 };
            var23 = (((var20_[1] - var20_[3]) * (var20_[1] - var20_[3])
                + (var21[1] - var21[3]) * (var21[1] - var21[3])
                + (var55[1] - var55[3]) * (var55[1] - var55[3])) as f64)
                .sqrt()
                / (o.keyz[1]
                    .wrapping_abs()
                    .wrapping_add(o.keyz[3].wrapping_abs()) as f64);
            if var23 >= 0.9998 {
                var79 = var5;
            } else {
                var79 = (var23.acos() / (std::f64::consts::PI / 180.0) * var5 as f64) as i32;
            }
        }

        if self.scy[1] != self.scy[0] {
            var5 = if self.scy[1] < self.scy[0] { -1 } else { 1 };
            var23 = (((var20_[0] - var20_[1]) * (var20_[0] - var20_[1])
                + (var21[0] - var21[1]) * (var21[0] - var21[1])
                + (var55[0] - var55[1]) * (var55[0] - var55[1])) as f64)
                .sqrt()
                / (o.keyx[0]
                    .wrapping_abs()
                    .wrapping_add(o.keyx[1].wrapping_abs()) as f64);
            if var23 >= 0.9998 {
                var81 = var5;
            } else {
                var81 = (var23.acos() / (std::f64::consts::PI / 180.0) * var5 as f64) as i32;
            }
        }

        if self.scy[3] != self.scy[2] {
            var5 = if self.scy[3] < self.scy[2] { -1 } else { 1 };
            var23 = (((var20_[2] - var20_[3]) * (var20_[2] - var20_[3])
                + (var21[2] - var21[3]) * (var21[2] - var21[3])
                + (var55[2] - var55[3]) * (var55[2] - var55[3])) as f64)
                .sqrt()
                / (o.keyx[2]
                    .wrapping_abs()
                    .wrapping_add(o.keyx[3].wrapping_abs()) as f64);
            if var23 >= 0.9998 {
                var83 = var5;
            } else {
                var83 = (var23.acos() / (std::f64::consts::PI / 180.0) * var5 as f64) as i32;
            }
        }

        // Madness.java:1839-1862 — the wall-hit quadrant multipliers
        if var9 {
            let mut var109 = o.xz.wrapping_add(45).wrapping_abs();
            while var109 > 180 {
                var109 = var109.wrapping_sub(360);
            }
            if var109.wrapping_abs() > 90 {
                self.pmlt = 1;
            } else {
                self.pmlt = -1;
            }
            var109 = o.xz.wrapping_sub(45).wrapping_abs();
            while var109 > 180 {
                var109 = var109.wrapping_sub(360);
            }
            if var109.wrapping_abs() > 90 {
                self.nmlt = 1;
            } else {
                self.nmlt = -1;
            }
        }

        // Madness.java:1865-1878 — the body yaw spin from the suspension
        // differential (the forca torque)
        o.xz = (o.xz as f32
            + self.forca
                * (self.scz[0] * self.nmlt as f32 - self.scz[1] * self.pmlt as f32
                    + self.scz[2] * self.pmlt as f32
                    - self.scz[3] * self.nmlt as f32
                    + self.scx[0] * self.pmlt as f32
                    + self.scx[1] * self.nmlt as f32
                    - self.scx[2] * self.nmlt as f32
                    - self.scx[3] * self.pmlt as f32)) as i32;

        // Madness.java:1879-1897 — the larger pitch/roll wins the apply
        if var79.wrapping_abs() > var77.wrapping_abs() {
            var77 = var79;
        }
        if var83.wrapping_abs() > var81.wrapping_abs() {
            var81 = var83;
        }
        if !var7 {
            self.pzy = self.pzy.wrapping_add(var77);
        } else {
            self.pzy = self.pzy.wrapping_sub(var77);
        }
        if !var11 {
            self.pxy = self.pxy.wrapping_add(var81);
        } else {
            self.pxy = self.pxy.wrapping_sub(var81);
        }

        // Madness.java:1899-1947 — the all-four-on-plane normalization
        if var66 == 4 {
            let mut var111 = 0i32;
            while self.pzy < 360 {
                self.pzy = self.pzy.wrapping_add(360);
                o.zy = o.zy.wrapping_add(360);
            }
            while self.pzy > 360 {
                self.pzy = self.pzy.wrapping_sub(360);
                o.zy = o.zy.wrapping_sub(360);
            }
            if self.pzy < 190 && self.pzy > 170 {
                self.pzy = 180;
                o.zy = 180;
                var111 += 1;
            }
            if self.pzy > 350 || self.pzy < 10 {
                self.pzy = 0;
                o.zy = 0;
                var111 += 1;
            }
            while self.pxy < 360 {
                self.pxy = self.pxy.wrapping_add(360);
                o.xy = o.xy.wrapping_add(360);
            }
            while self.pxy > 360 {
                self.pxy = self.pxy.wrapping_sub(360);
                o.xy = o.xy.wrapping_sub(360);
            }
            if self.pxy < 190 && self.pxy > 170 {
                self.pxy = 180;
                o.xy = 180;
                var111 += 1;
            }
            if self.pxy > 350 || self.pxy < 10 {
                self.pxy = 0;
                o.xy = 0;
                var111 += 1;
            }
            if var111 == 2 {
                self.mtouch = true;
            }
        }

        // Madness.java:1949-1955 — cntouch arbitration
        if self.mtouch || !self.wtouch {
            self.cntouch = 0;
        } else if self.cntouch == 10 {
            self.mtouch = true;
        } else {
            self.cntouch += 1;
        }

        // Madness.java:1957 — body y from the averaged wheel heights
        o.y = ((var21[0] + var21[1] + var21[2] + var21[3]) / 4.0
            - var13 as f32 * m.cos(self.pzy) * m.cos(self.pxy)
            + var16) as i32;
        var5 = if var7 { -1 } else { 1 };

        // Madness.java:1964-1983 — body x from the wheel span
        o.x = ((var55[0] - o.keyx[0] as f32 * m.cos(o.xz)
            + (var5.wrapping_mul(o.keyz[0])) as f32 * m.sin(o.xz)
            + var55[1]
            - o.keyx[1] as f32 * m.cos(o.xz)
            + (var5.wrapping_mul(o.keyz[1])) as f32 * m.sin(o.xz)
            + var55[2]
            - o.keyx[2] as f32 * m.cos(o.xz)
            + (var5.wrapping_mul(o.keyz[2])) as f32 * m.sin(o.xz)
            + var55[3]
            - o.keyx[3] as f32 * m.cos(o.xz)
            + (var5.wrapping_mul(o.keyz[3])) as f32 * m.sin(o.xz))
            / 4.0
            + var13 as f32 * m.sin(self.pxy) * m.cos(o.xz)
            - var13 as f32 * m.sin(self.pzy) * m.sin(o.xz)
            + var14) as i32;

        // Madness.java:1984-2003 — body z from the wheel span
        o.z = ((var20_[0]
            - (var5.wrapping_mul(o.keyz[0])) as f32 * m.cos(o.xz)
            - o.keyx[0] as f32 * m.sin(o.xz)
            + var20_[1]
            - (var5.wrapping_mul(o.keyz[1])) as f32 * m.cos(o.xz)
            - o.keyx[1] as f32 * m.sin(o.xz)
            + var20_[2]
            - (var5.wrapping_mul(o.keyz[2])) as f32 * m.cos(o.xz)
            - o.keyx[2] as f32 * m.sin(o.xz)
            + var20_[3]
            - (var5.wrapping_mul(o.keyz[3])) as f32 * m.cos(o.xz)
            - o.keyx[3] as f32 * m.sin(o.xz))
            / 4.0
            + var13 as f32 * m.sin(self.pxy) * m.sin(o.xz)
            - var13 as f32 * m.sin(self.pzy) * m.cos(o.xz)
            + var15) as i32;

        // Madness.java:2004-2024 — the visual xy/zy chase toward a settled
        // pitch/roll (the int divide is the 2 fine-step interpolation)
        if self.speed.abs() > 10.0 || !self.mtouch {
            if (self.pxy - o.xy).wrapping_abs() >= 4 {
                if self.pxy > o.xy {
                    o.xy = o.xy.wrapping_add(2 + (self.pxy - o.xy) / 2);
                } else {
                    o.xy = o.xy.wrapping_sub(2 + (o.xy - self.pxy) / 2);
                }
            } else {
                o.xy = self.pxy;
            }

            if (self.pzy - o.zy).wrapping_abs() >= 4 {
                if self.pzy > o.zy {
                    o.zy = o.zy.wrapping_add(2 + (self.pzy - o.zy) / 2);
                } else {
                    o.zy = o.zy.wrapping_sub(2 + (o.zy - self.pzy) / 2);
                }
            } else {
                o.zy = self.pzy;
            }
        }

        // Madness.java:2026-2048 — the speed lean (tilt)
        if self.wtouch && !self.capsized {
            let var112 = (self.speed / SWITS[self.cn][2] as f32 * 14.0) as f64
                * (BOUNCE[self.cn] as f64 - 0.4);
            if ctl.left && self.tilt < var112 as f32 && self.tilt >= 0.0 {
                self.tilt = (self.tilt as f64 + 0.4) as f32;
            } else if ctl.right && (self.tilt as f64) > -var112 && self.tilt <= 0.0 {
                self.tilt = (self.tilt as f64 - 0.4) as f32;
            } else if (self.tilt as f64).abs() > 3.0 * (BOUNCE[self.cn] as f64 - 0.4) {
                if self.tilt > 0.0 {
                    self.tilt = (self.tilt as f64 - 3.0 * (BOUNCE[self.cn] as f64 - 0.3)) as f32;
                } else {
                    self.tilt = (self.tilt as f64 + 3.0 * (BOUNCE[self.cn] as f64 - 0.3)) as f32;
                }
            } else {
                self.tilt = 0.0;
            }

            o.xy = (o.xy as f32 + self.tilt) as i32;
            if self.gtouch {
                o.y = (o.y as f64 - self.tilt as f64 / 1.5) as i32;
            }
        } else if self.tilt != 0.0 {
            self.tilt = 0.0;
        }

        // Madness.java:2050-2074 — the offroad surface shake (skd 2 ramps
        // 6.0/3.0, skd 1 roads 4.0/2.0, each × bounce − 0.3)
        if self.wtouch && var27 == 2 {
            // Madness.java:2050-2063 — TWO independent random draws (one
            // for zy, one for xy); the inner parenthesised term is f32, then
            // widened to double for the bounce multiply.
            o.zy = o.zy.wrapping_add(
                ((m.random() * 6.0 * self.speed / SWITS[self.cn][2] as f32
                    - 3.0 * self.speed / SWITS[self.cn][2] as f32) as f64
                    * (BOUNCE[self.cn] as f64 - 0.3)) as i32,
            );
            o.xy = o.xy.wrapping_add(
                ((m.random() * 6.0 * self.speed / SWITS[self.cn][2] as f32
                    - 3.0 * self.speed / SWITS[self.cn][2] as f32) as f64
                    * (BOUNCE[self.cn] as f64 - 0.3)) as i32,
            );
        }

        if self.wtouch && var27 == 1 {
            // Madness.java:2065-2074 — same f32 inner term, 4.0/2.0 ramp,
            // again two independent draws.
            o.zy = o.zy.wrapping_add(
                ((m.random() * 4.0 * self.speed / SWITS[self.cn][2] as f32
                    - 2.0 * self.speed / SWITS[self.cn][2] as f32) as f64
                    * (BOUNCE[self.cn] as f64 - 0.3)) as i32,
            );
            o.xy = o.xy.wrapping_add(
                ((m.random() * 4.0 * self.speed / SWITS[self.cn][2] as f32
                    - 2.0 * self.speed / SWITS[self.cn][2] as f32) as f64
                    * (BOUNCE[self.cn] as f64 - 0.3)) as i32,
            );
        }
        // Madness.java:2076-2087 — the distruct gate. `hitmag` over the
        // maxmag table arms destruction; corrosive `distruct` sets every
        // non-wheel plane's embos to 1 and the Plane.d ladder walks the
        // wreck visuals. The cntdest ladder and the `dest` flag are
        // drive-side state and live here already. The rpd.dest[im]
        // write is a Record-side no-op for now.
        if self.hitmag > MAXMAG[self.cn] && !self.dest {
            self.distruct(o);
            if self.cntdest == 7 {
                self.dest = true;
            } else {
                self.cntdest = self.cntdest.wrapping_add(1);
            }
            // Madness.java:2084-2086 — cntdest == 1 → rpd.dest[im] = 300.
            if self.cntdest == 1 {
                rpd.dest[self.im] = 300;
            }
        }

        // Madness.java:2089-2099 — an intact car clears the chip markers
        // and re-arms the emboss pulse on every plane.
        if o.dist == 0 {
            for p in &mut o.planes {
                if p.chip != 0 {
                    p.chip = 0;
                }
                if p.embos != 0 {
                    p.embos = 13;
                }
            }
        }

        // Madness.java:2101-2153 — checkpoint bookkeeping. var114 is the
        // next-checkpoint candidate, var117 the best `py` score, var6 the
        // focus tree multiplier (nofocus → every gate weighs the same),
        // and var119 counts positive gates ahead of the lap distance.
        let mut var114: i32 = 0;
        let mut var117: i32 = 0;
        let mut var119: i32 = 0;
        let mut var6 = if self.nofocus { 1 } else { 7 };
        for var122 in 0..cp.n as usize {
            if cp.typ[var122] > 0 {
                var119 = var119.wrapping_add(1);
                let rel = var119.wrapping_add(self.nlaps.wrapping_mul(cp.nsp));
                if cp.typ[var122] == 1 {
                    if self.clear == rel {
                        var6 = 1;
                    }
                    let scz =
                        (self.scz[0] + self.scz[1] + self.scz[2] + self.scz[3]).abs() / 4.0 + 60.0;
                    if ((o.z - cp.z[var122]).wrapping_abs() as f32) < scz
                        && (o.x - cp.x[var122]).wrapping_abs() < 700
                        && (o.y - cp.y[var122]).wrapping_abs() < 800
                        && self.clear == rel.wrapping_sub(1)
                    {
                        self.clear = rel;
                        self.pcleared = var122 as i32;
                        self.focus = -1;
                    }
                }

                if cp.typ[var122] == 2 {
                    if self.clear == rel {
                        var6 = 1;
                    }
                    let scx =
                        (self.scx[0] + self.scx[1] + self.scx[2] + self.scx[3]).abs() / 4.0 + 60.0;
                    if ((o.x - cp.x[var122]).wrapping_abs() as f32) < scx
                        && (o.z - cp.z[var122]).wrapping_abs() < 700
                        && (o.y - cp.y[var122]).wrapping_abs() < 800
                        && self.clear == rel.wrapping_sub(1)
                    {
                        self.clear = rel;
                        self.pcleared = var122 as i32;
                        self.focus = -1;
                    }
                }
            }

            let score = self
                .py(o.x / 100, cp.x[var122] / 100, o.z / 100, cp.z[var122] / 100)
                .wrapping_mul(var6);
            if score < var117 || var117 == 0 {
                var114 = var122 as i32;
                var117 = score;
            }
        }

        if self.clear == var119.wrapping_add(self.nlaps.wrapping_mul(cp.nsp)) {
            self.nlaps = self.nlaps.wrapping_add(1);
        }

        // Madness.java:2154-2223 — focus/missed-checkpoint machine.
        if self.focus == -1 {
            var114 = var114.wrapping_add(if self.im == 0 { 2 } else { 1 });
            if !self.nofocus {
                var119 = self.pcleared.wrapping_add(1);
                while cp.typ[var119 as usize] <= 0 {
                    var119 = var119.wrapping_add(1);
                    if var119 as usize == cp.n as usize {
                        var119 = 0;
                    }
                }
                if var114 > var119
                    && (self.clear != self.nlaps.wrapping_mul(cp.nsp) || var114 < self.pcleared)
                {
                    var114 = var119;
                    self.focus = var119;
                }
            }
            if var114 >= cp.n {
                var114 = var114.wrapping_sub(cp.n);
            }
            if cp.typ[var114 as usize] == -3 {
                var114 = 0;
            }
            if self.im == 0 {
                self.missedcp = -1;
            } else if self.missedcp != 0 {
                self.missedcp = 0;
            }
        } else {
            var114 = self.focus;
            let near = (self.py(
                o.x / 10,
                cp.x[self.focus as usize] / 10,
                o.z / 10,
                cp.z[self.focus as usize] / 10,
            ) as f64)
                .sqrt();
            if self.im == 0 {
                if self.missedcp == 0 && self.mtouch && near > 800.0 {
                    self.missedcp = 1;
                }
                if self.missedcp == -2 && near < 400.0 {
                    self.missedcp = 0;
                }
                if self.missedcp != 0 && self.mtouch && near < 250.0 {
                    self.missedcp = 68;
                }
            } else {
                self.missedcp = 1;
            }
            if self.nofocus {
                self.focus = -1;
                self.missedcp = 0;
            }
        }
        if self.nofocus {
            self.nofocus = false;
        }
        self.point = var114;

        // Madness.java:2225-2253 — the fix stations. `!roted` gates the
        // z-line test, `roted` the x-line one; the `xt.carfixed` sting
        // fires for the player on a fresh fix (muted by `xt.mutes`).
        // The rpd.fix[im] = 300 snapshot is live.
        for var123 in 0..cp.fn_ as usize {
            if !cp.roted[var123] {
                if (o.z - cp.fz[var123]).wrapping_abs() < 200
                    && self.py(
                        o.x / 100,
                        cp.fx[var123] / 100,
                        o.y / 100,
                        cp.fy[var123] / 100,
                    ) < 30
                {
                    if o.dist == 0 {
                        o.fcnt = 8;
                    } else {
                        // Madness.java:2232 — the fix-station sting.
                        if self.im == 0
                            && !o.fix
                            && !xt.mutes
                            && let Some(c) = xt.carfixed.as_mut()
                        {
                            c.play();
                        }
                        o.fix = true;
                    }
                    // Madness.java:2238 — rpd.fix[im] = 300 (Record).
                    rpd.fix[self.im] = 300;
                }
            } else if (o.x - cp.fx[var123]).wrapping_abs() < 200
                && self.py(
                    o.z / 100,
                    cp.fz[var123] / 100,
                    o.y / 100,
                    cp.fy[var123] / 100,
                ) < 30
            {
                if o.dist == 0 {
                    o.fcnt = 8;
                } else {
                    // Madness.java:2245 — the rotated fix-station sting.
                    if self.im == 0
                        && !o.fix
                        && !xt.mutes
                        && let Some(c) = xt.carfixed.as_mut()
                    {
                        c.play();
                    }
                    o.fix = true;
                }
                // Madness.java:2251 — rpd.fix[im] = 300 (Record).
                rpd.fix[self.im] = 300;
            }
        }

        // Madness.java:2255-2262 — a car at the fix countdown fully
        // restores and swaps to the showroom model.
        if o.fcnt == 7 || o.fcnt == 8 {
            self.squash = 0;
            self.nbsq = 0;
            self.hitmag = 0;
            self.cntdest = 0;
            self.dest = false;
            self.newcar = true;
        }

        // Madness.java:2264-2415 — the air-trick recorder and powerup
        // machine. While airborne it counts the stick travels (135°
        // arm flags, wall-surfing), while grounded it banks the trick
        // power at trcnt == 9 and clears it a tick later, and capsized
        // cars unflip after 30 still frames.
        if !self.mtouch {
            if self.trcnt != 1 {
                self.trcnt = 1;
                self.lxz = o.xz;
            }
            if self.loop_ == 2 || self.loop_ == -1 {
                self.travxy = (self.travxy as f32 + (self.rcomp - self.lcomp)) as i32;
                if self.travxy.wrapping_abs() > 135 {
                    self.rtab = true;
                }
                self.travzy = (self.travzy as f32 + (self.ucomp - self.dcomp)) as i32;
                if self.travzy > 135 {
                    self.ftab = true;
                }
                if self.travzy < -135 {
                    self.btab = true;
                }
            }
            if self.lxz != o.xz {
                self.travxz = self.travxz.wrapping_add(self.lxz.wrapping_sub(o.xz));
                self.lxz = o.xz;
            }
            if self.srfcnt < 10 {
                if ctl.wall != -1 {
                    self.surfer = true;
                }
                self.srfcnt = self.srfcnt.wrapping_add(1);
            }
        } else if !self.dest {
            if !self.capsized {
                if self.capcnt != 0 {
                    self.capcnt = 0;
                }
                if self.gtouch && self.trcnt != 0 {
                    if self.trcnt == 9 {
                        self.powerup = 0.0;
                        if self.travxy.wrapping_abs() > 90 {
                            self.powerup += self.travxy.wrapping_abs() as f32 / 24.0;
                        } else if self.rtab {
                            self.powerup += 30.0;
                        }
                        if self.travzy.wrapping_abs() > 90 {
                            self.powerup += self.travzy.wrapping_abs() as f32 / 18.0;
                        } else {
                            if self.ftab {
                                self.powerup += 40.0;
                            }
                            if self.btab {
                                self.powerup += 40.0;
                            }
                        }
                        if self.travxz.wrapping_abs() > 90 {
                            self.powerup += self.travxz.wrapping_abs() as f32 / 18.0;
                        }
                        if self.surfer {
                            self.powerup += 30.0;
                        }
                        self.power += self.powerup;
                        // Madness.java:2334-2340 — im == 0 && (int)powerup >
                        // rpd.powered && wasted == 0 && powerup > 60 → the
                        // cotchinow branch; it only stamps when `caught`
                        // already holds 300 recorded frames.
                        if self.im == 0
                            && self.powerup as i32 > rpd.powered
                            && rpd.wasted == 0
                            && self.powerup > 60.0
                        {
                            rpd.cotchinow(0, m);
                            if rpd.hcaught {
                                rpd.whenwasted = 225;
                                rpd.powered = self.powerup as i32;
                            }
                        }
                        if self.power > 98.0 {
                            self.power = 98.0;
                            if self.powerup > 150.0 {
                                self.xtpower = 200;
                            } else {
                                self.xtpower = 100;
                            }
                        }
                    }

                    if self.trcnt == 10 {
                        self.travxy = 0;
                        self.travzy = 0;
                        self.travxz = 0;
                        self.ftab = false;
                        self.rtab = false;
                        self.btab = false;
                        self.trcnt = 0;
                        self.srfcnt = 0;
                        self.surfer = false;
                    } else {
                        self.trcnt = self.trcnt.wrapping_add(1);
                    }
                }
            } else {
                if self.trcnt != 0 {
                    self.travxy = 0;
                    self.travzy = 0;
                    self.travxz = 0;
                    self.ftab = false;
                    self.rtab = false;
                    self.btab = false;
                    self.trcnt = 0;
                    self.srfcnt = 0;
                    self.surfer = false;
                }
                if self.capcnt == 0 {
                    let mut var124 = 0;
                    let mut var126 = 0usize;
                    loop {
                        if self.scz[var126].abs() < 70.0 && self.scx[var126].abs() < 70.0 {
                            var124 += 1;
                        }
                        var126 += 1;
                        if var126 == 4 {
                            break;
                        }
                    }
                    if var124 == 4 {
                        self.capcnt = 1;
                    }
                } else {
                    self.capcnt = self.capcnt.wrapping_add(1);
                    if self.capcnt == 30 {
                        self.speed = 0.0;
                        o.y += FLIPY[self.cn];
                        self.pxy = self.pxy.wrapping_add(180);
                        o.xy += 180;
                        self.capcnt = 0;
                    }
                }
            }

            if self.trcnt == 0 {
                if self.xtpower == 0 {
                    if self.power > 0.0 {
                        self.power = self.power
                            - self.power * self.power * self.power / POWERLOSS[self.cn] as f32;
                    } else {
                        self.power = 0.0;
                    }
                } else {
                    self.xtpower = self.xtpower.wrapping_add(-1);
                }
            }
        }

        // Madness.java:2417-2423 — the wall marker refresh for player 0
        // and lastcolido countdown for the opponents.
        if self.im == 0 {
            if ctl.wall != -1 {
                ctl.wall = -1;
            }
        } else if self.lastcolido != 0 && !self.dest {
            self.lastcolido = self.lastcolido.wrapping_add(-1);
        }

        // Madness.java:2425-2435 — the dest slot bookkeeping feeding
        // CheckPoints.dested (the race-liveness of the slot).
        if self.dest {
            if cp.dested[self.im] == 0 {
                if self.lastcolido == 0 {
                    cp.dested[self.im] = 1;
                } else {
                    cp.dested[self.im] = 2;
                }
            }
        } else if cp.dested[self.im] != 0 {
            cp.dested[self.im] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conto::{Base, Conto};
    use crate::medium::Medium;
    use crate::trackers::Trackers;

    /// An empty `Conto` with wheel keys at ±10 for the `forca` path.
    fn blank_conto() -> Conto {
        let m = Medium::new();
        let mut t = Trackers::new();
        let base = Base {
            keyx: [10, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        Conto::place(&base, &m, &mut t, 0, 0, 0, 0)
    }

    #[test]
    fn reseto_sets_identity_and_clears_state() {
        let mut m = Madness::new(0);
        let mut o = blank_conto();
        let mut cp = CheckPoints::new();
        cp.pcs = 12;
        m.reseto(3, &mut o, &mut cp);
        assert_eq!(m.cn, 3);
        assert_eq!(m.pcleared, 12);
        assert!((m.power - 98.0).abs() < 1e-6);
        assert_eq!(m.focus, -1);
        assert!(!m.dest);
        // forca = (4·sqrt(10²+10²)) / 10000 · (bounce[3] − 0.3), all f32.
        let expect = 4.0_f32 * (200.0_f64.sqrt() as f32) / 10000.0 * ((1.15_f64 - 0.3) as f32);
        assert!((m.forca - expect).abs() < 1e-6);
    }

    #[test]
    fn reseto_keeps_moment_override_on_reslotting() {
        // The `cn == 7` override is only written when the car number lands
        // on 7 for player slot 0 — a later reset with a different car keeps
        // the value the instance holds (Madness.java:572-578).
        let mut m = Madness::new(0);
        let mut o = blank_conto();
        let mut cp = CheckPoints::new();
        m.reseto(7, &mut o, &mut cp);
        assert_eq!(m.moment[7], 2.0);
        m.reseto(2, &mut o, &mut cp);
        assert_eq!(m.moment[7], 2.0);
    }

    #[test]
    fn py_rpy_match_java_arith() {
        let m = Madness::new(0);
        assert_eq!(m.py(3, 1, 4, 2), 8);
        // MIN − MAX = −1 under wrap, so the square pair sums to 1.
        assert_eq!(m.py(i32::MIN, i32::MAX, 0, 0), 1);
        assert_eq!(m.rpy(2.0, 0.0, 1.0, 0.0, 0.0, 0.0), 5);
    }

    /// The five drive() behaviour tests below ride a fresh car-0 slot:
    /// power 98 (full), speed 0, parked Conto, empty trackers. Gears for
    /// cn 0 gate at speed 50 / 180 / 280 when power == 98 (each gate is
    /// `swits/2 + 98*swits/196 == swits`), stepping ACELF[0] = 11/5/3.
    fn drive_rig() -> (
        Madness,
        Conto,
        Control,
        CheckPoints,
        Medium,
        Trackers,
        XtGraphics,
    ) {
        let mut mad = Madness::new(0);
        let mut o = blank_conto();
        let mut cp = CheckPoints::new();
        mad.reseto(0, &mut o, &mut cp);
        // Spawn resting on the global plane like the stage placement
        // (body y = 250 - grat): wheels land at 250 from frame 1, the
        // mtouch chain is live, and no skid-exit artifact can fire.
        o.grat = 30;
        o.y = 250 - 30;
        mad.mtouch = true;
        // Game-like checkpoint rows: the applet's loadstage always leaves
        // positive `typ` rows within `n` (the focus walk needs one after
        // `pcleared`), so give the rig two far-away gates.
        cp.n = 2;
        cp.nsp = 1;
        cp.typ[0] = 1;
        cp.typ[1] = 1;
        cp.x[0] = 999900;
        cp.x[1] = 999900;
        let ctl = Control::new();
        let m = Medium::new();
        let t = Trackers::new();
        let xt = XtGraphics::new();
        (mad, o, ctl, cp, m, t, xt)
    }

    #[test]
    fn drive_accel_climbs_gears_and_caps_at_280() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        ctl.up = true;
        for _ in 0..5 {
            // Pin the power meter: the tail's power decay reads the same
            // field the throttle gain multiplies, and this test pins the
            // gear mechanics (see the powerup/decay test below).
            mad.power = 98.0;
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        // Five first-gear frames of +11, each followed by the mtouch
        // block's 0.5 drag for car 0: net +10.5/frame. Gate0 (50) is
        // passed on the fifth, so frame six steps into the 5/frame gear.
        assert!((mad.speed - 52.5).abs() < 1e-4, "speed {}", mad.speed);
        mad.power = 98.0;
        mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        assert!((mad.speed - 57.0).abs() < 1e-4);
        // Full throttle settles one drag-step under the swits[0][2] cap:
        // the throttle clamps to 280 and the mtouch drag trims 0.5.
        for _ in 0..200 {
            mad.power = 98.0;
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        assert_eq!(mad.speed, 279.5);
        // No steering input leaves wxz untouched at the ramp.
        assert_eq!(o.wxz, 0);
        // Up-righting: pzy 0 is face-up, zyinv must stay false.
        assert!(!ctl.zyinv);
    }

    #[test]
    fn drive_reverse_gains_then_caps_at_minus_180() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        ctl.down = true;
        for _ in 0..6 {
            // Pin the power meter (see the accel test note).
            mad.power = 98.0;
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        // −11×5 minus the per-frame drag: net −10.5 for five frames,
        // then the sixth frame's −5 gain after the (−50) gate, also
        // trimmed by the drag: −52.5 → −57.5 → −57.0.
        assert!((mad.speed + 57.0).abs() < 1e-4, "speed {}", mad.speed);
        for _ in 0..300 {
            mad.power = 98.0;
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        // Reverse settles into a 2-frame limit cycle at the swits[cn][1]
        // gate: entry −179.5 misses the `<= −180` gate (drag trimmed it),
        // so the gear gain is −5 instead of the var51==2 catch; the next
        // frame the catch re-clamps to −180 and the drag trims 0.5.
        // Even frame counts from the mid-check land on −184.0.
        assert_eq!(mad.speed, -184.0);
    }

    #[test]
    fn drive_handbrake_bleeds_by_handb_int_div() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        mad.speed = -100.0;
        ctl.handb = true;
        for _ in 0..30 {
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        // The 7-per-frame handbrake bleed lands at −2, then the mtouch
        // drag creeps |speed| down to the 0 floor (drag ≥ |speed| → 0).
        assert_eq!(mad.speed, 0.0);
    }

    #[test]
    fn drive_power_decays_by_cubed_ratio() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        // With the car grounded (mtouch live, dest false) the power meter
        // drains by power³/powerloss[0] each frame (Madness.java:2404-
        // 2410). Throttle-free frame: 98 - 98³/2500000.
        mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        assert!(
            (mad.power - (98.0 - 941192.0f32 / 2500000.0)).abs() < 1e-4,
            "power {}",
            mad.power
        );
    }

    #[test]
    fn drive_trick_powerup_banks_at_trcnt_9_and_resets_at_10() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        // A grounded post-trick state: the recorder banked travxy = 100,
        // and the meter is spent (a full meter would clamp the bank).
        mad.gtouch = true;
        mad.trcnt = 9;
        mad.travxy = 100;
        mad.power = 0.0;
        mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        // Madness.java:2306-2333 — |travxy| > 90 → powerup += |travxy|/24
        // (no other gates), power banks it, trcnt steps 9 → 10.
        assert!(
            (mad.power - 100.0f32 / 24.0).abs() < 1e-4,
            "power {}",
            mad.power
        );
        assert_eq!(mad.trcnt, 10);
        mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        // Madness.java:2352-2361 — the tick after 9 clears the recorders.
        assert_eq!(mad.trcnt, 0);
        assert_eq!(mad.travxy, 0);
        assert!(!mad.rtab && !mad.ftab && !mad.btab);
        assert!(!mad.surfer);
        assert_eq!(mad.srfcnt, 0);
    }

    #[test]
    fn drive_dest_ladder_flips_flag_after_seven_counts() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        // Over the maxmag[0] = 3500 gate (Madness.java:2076-2087): the
        // distruct call itself is deferred, but the cntdest ladder drives
        // the `dest` slot flip and the CheckPoints.dested bookkeeping.
        mad.hitmag = 4000;
        for _ in 0..8 {
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        assert!(mad.dest);
        assert_eq!(mad.cntdest, 7);
        // Madness.java:2425-2432 — lastcolido 0 dests into slot 1.
        assert_eq!(cp.dested[0], 1);
    }

    #[test]
    fn distruct_arms_only_body_planes() {
        // Madness.java:555-562 — the wreck trigger embosses the non-wheel
        // planes and leaves the wheel spin planes (wz != 0) pristine.
        let m = Medium::new();
        let mut base = Base {
            keyx: [10, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        use crate::plane::Plane;
        base.planes.push(Plane::new(
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
        ));
        base.planes.push(Plane::new(
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
            9,
            7,
            5,
            false,
        ));

        let mut t = Trackers::new();
        let mut o = Conto::place(&base, &m, &mut t, 0, 0, 0, 0);
        let mut cp = CheckPoints::new();
        let mut mad = Madness::new(0);
        mad.reseto(0, &mut o, &mut cp);

        mad.distruct(&mut o);
        assert_eq!(o.planes[0].embos, 1);
        assert_eq!(o.planes[1].embos, 0, "the wz plane keeps its spin");
    }

    #[test]
    fn drive_capsized_unflips_after_thirty_still_frames() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        // Madness.java:2379-2401 — a capsized car with every wheel spring
        // settled (|scx|,|scz| < 70) arms capcnt on frame 1 and unflips
        // on the 30th: speed 0, y += flipy[0] (−50), pxy/xy += 180. The
        // head re-arms `capsized` from the folded angles each frame
        // (Madness.java:737-777), so the test keeps the car flipped with
        // pzy = 180, and after the unflip writes pxy = 180 the doubly-
        // flipped head state drops the flag again.
        mad.pzy = 180;
        for _ in 0..30 {
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        assert_eq!(mad.capcnt, 0);
        assert_eq!(mad.speed, 0.0);
        assert_eq!(mad.pxy, 180);
        assert_eq!(o.xy, 180);
    }

    #[test]
    fn colide_arms_dominate_and_sinks_the_other_wheel() {
        // Self = pool slot 0 in car 1 (LIFT 30 / PUSH 2 / moment 0.75),
        // other = slot 1 in car 0 (moment 1.2) — Madness.java:383-553.
        let mut rpd = crate::record::Record::new();
        let mut m0 = Madness::new(0);
        let mut o0 = blank_conto();
        o0.maxr = 900;
        let mut cp = CheckPoints::new();
        m0.reseto(1, &mut o0, &mut cp);

        let mut m1 = Madness::new(1);
        let mut o1 = blank_conto();
        o1.maxr = 900;
        m1.reseto(0, &mut o1, &mut cp);

        let mut med = Medium::new();

        // Overlapping wheel anchors put the whole 4×4 matrix in range;
        // the ploughing car dominates the still one.
        for v in 0..4 {
            m0.scx[v] = 5.0;
            m0.scz[v] = 6.0;
        }
        m0.speed = 100.0;
        m0.colide(
            &mut o0,
            &mut m1,
            &mut o1,
            &mut med,
            &mut rpd,
            &mut XtGraphics::new(),
        );

        assert!(m0.caught[1]);
        assert!(m0.dominate[1]);
        // The x-arm fires on row v11 = 0 only: after it moves the other
        // springs, the strict `>` (|5·0.75| vs |10·1.2|) blocks the rest,
        // so every other wheel gains exactly one 5·PUSH[1] = 10 push.
        assert_eq!(m1.scx, [10.0; 4]);
        // The z-arm mirrors with the 6·PUSH[1] = 12 transfers.
        assert_eq!(m1.scz, [12.0; 4]);
        // LIFT[1] = 30 sinks once per matched (v11, v12) pair → four
        // sinks of 30 per other wheel, Madness.java:546-548.
        assert_eq!(m1.scy, [-120.0; 4]);
        assert_eq!(m1.lastcolido, 70);
        // Self keeps its springs (REVLIFT[1] = 0; var13x = 0 re-pushes).
        assert_eq!(m0.scx, [5.0; 4]);
        assert_eq!(m0.scz, [6.0; 4]);
        assert_eq!(m0.scy, [0.0; 4]);
        assert_eq!(m0.lastcolido, 0);
    }

    #[test]
    fn colide_idles_out_of_range_and_when_both_still() {
        let mut rpd = crate::record::Record::new();
        let mut m0 = Madness::new(0);
        let mut o0 = blank_conto();
        o0.maxr = 900;
        let mut cp = CheckPoints::new();
        m0.reseto(0, &mut o0, &mut cp);

        let mut m1 = Madness::new(1);
        let mut o1 = blank_conto();
        o1.maxr = 900;
        m1.reseto(0, &mut o1, &mut cp);
        // Parked outside the (maxR² + maxR²)·1.5 sphere: the float rpy
        // saturates past i32 range like Java's int cast, well above the
        // threshold, so the duel never arms.
        o1.x = 999999;

        let mut med = Medium::new();
        m0.colide(
            &mut o0,
            &mut m1,
            &mut o1,
            &mut med,
            &mut rpd,
            &mut XtGraphics::new(),
        );
        assert!(!m0.caught[1]);
        assert!(!m0.dominate[1]);
        assert_eq!(m1.scx, [0.0; 4]);
        assert_eq!(m1.lastcolido, 0);
    }

    #[test]
    fn drive_steering_ramps_clamps_and_decays() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        ctl.left = true;
        for _ in 0..10 {
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        assert_eq!(o.wxz, 36);
        ctl.left = false;
        for _ in 0..3 {
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        // Standstill decay steps −1 per frame.
        assert_eq!(o.wxz, 33);
        ctl.right = true;
        for _ in 0..12 {
            mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        }
        assert_eq!(o.wxz, -36);
        // Resting on the plane: contact re-arms wtouch every frame.
        assert!(mad.wtouch);
    }

    #[test]
    fn drive_steer_applies_when_wheels_touch() {
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        mad.wtouch = true;
        mad.speed = 100.0;
        ctl.left = true;
        // speed² = 10000 → var53 = (int)(3600/10000) = 0 → floor 5.
        mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        assert_eq!(o.wxz, 6);
        // xz += wxz / var53 = 6/5 = 1; fxz = 6/(5·3) = 0 (both int-div).
        assert_eq!(o.xz, 1);
        assert_eq!(mad.fxz, 0);
        // Contact from the resting wheels re-arms wtouch for next frame.
        assert!(mad.wtouch);

        // The conf variant of the same steer step divides wxz by var53
        // alone (fxz = 6/5), used for the wide slide (Madness.java:1147).
        let (mut mad, mut o, mut ctl, mut cp, mut m, t, mut xt) = drive_rig();
        let mut rpd = crate::record::Record::new();
        mad.wtouch = true;
        mad.speed = 100.0;
        ctl.left = true;
        ctl.handb = true;
        mad.drive(&mut ctl, &mut o, &t, &mut cp, &mut m, &mut rpd, &mut xt);
        assert_eq!(o.xz, 1);
        assert_eq!(mad.fxz, 1);
    }
}
