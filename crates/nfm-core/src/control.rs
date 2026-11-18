// FIDELITY: Control.preform(): Java `!(a < b)` float comparisons are load-bearing (NaN/partial-order shape must match Control.java).
#![allow(clippy::neg_cmp_op_on_partial_ord)]
//! Port of `Control.java` — the per-player control/AI state. This slice
//! holds the input flags (`left`/`right`/`up`/`down`/`handb`/`enter`),
//! the full field set with its original defaults (Control.java:1-63),
//! and the race pieces everything consumes: `falseo`, `reset` (the
//! fix-marker → facepoint mapping, Control.java:1120-1157), `py`, and
//! the AI driver `preform` (Control.java:65-1119) that fase0 runs on
//! the four competitor slots.
//!
//! The `Medium` reference (Control.java:11) is not held — like the other
//! ports, methods take it as an argument when a piece needs it.

use crate::checkpoints::CheckPoints;
use crate::conto::Conto;
use crate::madness::{MAXMAG, Madness, SWITS};
use crate::medium::Medium;
use crate::trackers::Trackers;

pub struct Control {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub handb: bool,
    pub enter: bool,
    pub arrace: bool,
    pub mutem: bool,
    pub mutes: bool,
    pub pan: i32,
    pub attack: i32,
    pub acr: i32,
    pub afta: bool,
    /// Fix-marker → nearest-facepoint table (reset, Control.java:1131-1147).
    pub fpnt: [i32; 5],
    pub trfix: i32,
    pub forget: bool,
    pub bulistc: bool,
    pub acuracy: i32,
    pub upwait: i32,
    pub agressed: bool,
    pub skiplev: f32,
    pub clrnce: i32,
    pub rampp: i32,
    pub turntyp: i32,
    pub saftey: i32,
    pub perfection: bool,
    pub mustland: f32,
    pub usebounce: bool,
    pub trickprf: f32,
    /// Written by `Madness.drive` (Madness.java:782) — the z-flip flag the
    /// camera/HUD consults.
    pub zyinv: bool,
    pub lastl: bool,
    pub wlastl: bool,
    pub hold: i32,
    pub wall: i32,
    pub lwall: i32,
    pub stcnt: i32,
    pub statusque: i32,
    pub turncnt: i32,
    pub randtcnt: i32,
    pub upcnt: i32,
    pub trickfase: i32,
    pub swat: i32,
    pub udcomp: bool,
    pub lrcomp: bool,
    pub udbare: bool,
    pub lrbare: bool,
    pub onceu: bool,
    pub onced: bool,
    pub oncel: bool,
    pub oncer: bool,
    pub lrdirect: i32,
    pub uddirect: i32,
    pub lrstart: i32,
    pub udstart: i32,
    pub oxy: i32,
    pub ozy: i32,
    pub flycnt: i32,
    pub lrswt: bool,
    pub udswt: bool,
    pub actwait: i32,
    pub cntrn: i32,
}

impl Control {
    /// Field defaults per Control.java:2-63.
    pub fn new() -> Control {
        Control {
            left: false,
            right: false,
            up: false,
            down: false,
            handb: false,
            enter: false,
            arrace: false,
            mutem: false,
            mutes: false,
            pan: 0,
            attack: 0,
            acr: 0,
            afta: false,
            fpnt: [0; 5],
            trfix: 0,
            forget: false,
            bulistc: false,
            acuracy: 0,
            upwait: 0,
            agressed: false,
            skiplev: 1.0,
            clrnce: 5,
            rampp: 0,
            turntyp: 0,
            saftey: 30,
            perfection: false,
            mustland: 0.5,
            usebounce: false,
            trickprf: 0.5,
            zyinv: false,
            lastl: false,
            wlastl: false,
            hold: 0,
            wall: -1,
            lwall: -1,
            stcnt: 0,
            statusque: 0,
            turncnt: 0,
            randtcnt: 0,
            upcnt: 0,
            trickfase: 0,
            swat: 0,
            udcomp: false,
            lrcomp: false,
            udbare: false,
            lrbare: false,
            onceu: false,
            onced: false,
            oncel: false,
            oncer: false,
            lrdirect: 0,
            uddirect: 0,
            lrstart: 0,
            udstart: 0,
            oxy: 0,
            ozy: 0,
            flycnt: 0,
            lrswt: false,
            udswt: false,
            actwait: 0,
            cntrn: 0,
        }
    }

    /// `falseo` (Control.java:1163-1173): release every input flag.
    pub fn falseo(&mut self) {
        self.left = false;
        self.right = false;
        self.up = false;
        self.down = false;
        self.handb = false;
        self.enter = false;
        self.arrace = false;
        self.mutem = false;
        self.mutes = false;
    }

    /// `reset` (Control.java:1120-1157): wipe the AI bookkeeping and map
    /// each fix marker (`cp.fx`/`fz`, `fn_` of them) to the nearest
    /// facepoint in the checkpoint ring (`cp.x`/`z`, `n` of them, in
    /// hundredths), backing the index off by 4 so the AI looks ahead of
    /// the marker. The `py` comparisons use the class's own copy.
    pub fn reset(&mut self, cp: &CheckPoints) {
        self.pan = 0;
        self.attack = 0;
        self.acr = 0;
        self.afta = false;
        self.trfix = 0;
        self.acuracy = 0;
        self.upwait = 0;
        self.forget = false;
        self.bulistc = false;

        for f in 0..cp.fn_ as usize {
            let mut best = -10i32;
            for w in 0..cp.n as usize {
                let d = self.py(cp.fx[f] / 100, cp.x[w] / 100, cp.fz[f] / 100, cp.z[w] / 100);
                // Control.java:1135-1138 — condition order as written: the
                // `best == -10` sentinel loses to a strict comparison.
                if d < best || best == -10 {
                    best = d;
                    self.fpnt[f] = w as i32;
                }
            }
        }

        for f in 0..cp.fn_ as usize {
            self.fpnt[f] -= 4;
            if self.fpnt[f] < 0 {
                self.fpnt[f] += cp.nsp;
            }
        }

        self.left = false;
        self.right = false;
        self.up = false;
        self.down = false;
        self.handb = false;
        self.arrace = false;
        self.mutem = false;
        self.mutes = false;
    }

    /// `Control.py` (Control.java:1175-1177) — same wrapped square pair as
    /// `CheckPoints.py`; the subtraction wraps in Java too.
    pub fn py(&self, a: i32, b: i32, c: i32, d: i32) -> i32 {
        let d1 = a.wrapping_sub(b);
        let d2 = c.wrapping_sub(d);
        d1.wrapping_mul(d1).wrapping_add(d2.wrapping_mul(d2))
    }

    /// `Control.preform` (Control.java:65-1119) — the competitor brain.
    /// Every frame it clears the five press flags and then, unless the
    /// car is destroyed, works through two ladders:
    ///
    /// 1. the `mtouch` strategy refresh (every `statusque` frames,
    ///    counted by `stcnt`): reaction `acuracy` (7−my pos)·leader·
    ///    (6−stage·2), the `upwait` planner with its stage ladders, the
    ///    `skiplev` shortcut appetite (decayed −0.1 when the car is not
    ///    behind the player, raised +0.2 when it is, capped per stage),
    ///    the `rampp` ramp appetite (0..3 from the stream; pinned while
    ///    the power meter is full/low), the `cntrn` 5-frame turn-style
    ///    election (random turntyp, stage overrides, the `agressed`
    ///    flags for the hostile stages), `saftey` (dampened to ≤ 20 and
    ///    multiplied when leading), `mustland`, `trickprf`, the
    ///    `usebounce`/`perfection` rolls, and the near-enemy scan that
    ///    arms `attack` (a 40·(clear gap + 1) frame chase on the victim
    ///    slot, with the stage/im special-friend overrides) or zeroes
    ///    `clrnce` when the player hovers close. Then the `trfix`
    ///    damage ladder (1 at half damage, 2 at 80% when not stage-6/8/
    ///    11, 3 the forced seek, 4 the settled seek that pads
    ///    `statusque` by 30) and the `bulistc` bullet arming.
    /// 2. the press machine: while rolling (`wtouch`, or `mtouch` under
    ///    `usebounce`) it drives the point marker — throttle pressing
    ///    (`upcnt`/`actwait`/`upwait`), the attack-chase `pan`, the
    ///    `rampp`/`skiplev`/`bulistc` off-route shortcuts (each skipping
    ///    gate types −4/−1/0/−3 with its own walk), the trfix 2/3
    ///    nearest-fix pick through `fpnt`/`special`/`forget`, the
    ///    heading pan (recomputed on a `turncnt`/`randtcnt` beat), and
    ///    the turn motor (`clrnce` deadband with the wall override, the
    ///    > 50° deep-turn handb/brake presses by `turntyp`, the mirrored
    ///    > +180 arcs) plus the wall-reaction steer against the tracker
    ///    > the drive flagged (`wlastl`/`lwall`/`hold` with the `dam`
    ///    > probe). In the air it runs the trickfase ladder: 0 (arm using
    ///    > `scysum × (y−300)/4000` gated by `trickprf` through the random
    ///    > ladder — ud/lr press starts, swt flips, the `trfix`
    ///    > override), −1 (number crunch failed), 1 (the presses with the
    ///    > `pxy/pzy` 180° swt correction and the dump into fase 2 once
    ///    > the lift relative to the drop beats `-saftey`), and 2 (the
    ///    > swat 0→3 ladder — the comp probes, the capsized-stall
    ///    > dwell, then the `oncu/onde/oncl/oncr` once-flaps pressing to
    ///    > `mustland`, and the "comp" mode where the capsized
    ///    > `perfection` demands `|pzy/pxy±90 − 90| > 30`).
    ///
    /// Everything rolled goes through `Medium.random()` in source order
    /// (the port's deterministic stream); all `(int)` casts are `as`
    /// casts (saturating, NaN-free inputs here).
    pub fn preform(
        &mut self,
        var1: &mut Madness,
        var2: &Conto,
        var3: &CheckPoints,
        var4: &Trackers,
        m: &mut Medium,
    ) {
        self.left = false;
        self.right = false;
        self.up = false;
        self.down = false;
        self.handb = false;
        if !var1.dest {
            if var1.mtouch {
                if self.stcnt > self.statusque {
                    // Stat ladder: reaction accuracy from the placement
                    // and stage, then the (pos0 − pos[im])³ cube as an
                    // int multiply chain (the sources are placement
                    // counts, no overflow) with the float cast and the
                    // ×var5 stage ladders.
                    self.acuracy = (7 - var3.pos[var1.im]) * var3.pos[0] * (6 - var3.stage * 2);
                    if self.acuracy < 0 {
                        self.acuracy = 0;
                    }

                    let mut var5 = 0.0f32;
                    if var3.stage == 1 {
                        var5 = 2.0;
                    }

                    if var3.stage == 2 {
                        var5 = 1.5;
                    }

                    if var3.stage == 3 && var1.im != 4 {
                        var5 = 0.5;
                    }

                    if var3.stage == 4 {
                        var5 = 0.5;
                    }

                    let cubed = (var3.pos[0] - var3.pos[var1.im])
                        * (var3.pos[0] - var3.pos[var1.im])
                        * (var3.pos[0] - var3.pos[var1.im]);
                    self.upwait = (cubed as f32 * var5) as i32;
                    if self.upwait > 80 {
                        self.upwait = 80;
                    }

                    if var3.stage == 1 && self.upwait < 20 {
                        self.upwait = 20;
                    }

                    var5 = 0.0;
                    if var3.stage == 1 || var3.stage == 2 {
                        var5 = 1.0;
                    }

                    if var3.stage == 4 {
                        var5 = 0.5;
                    }

                    if var3.stage == 7 {
                        var5 = 0.5;
                    }

                    if var3.stage == 10 {
                        var5 = 0.5;
                    }

                    if var3.pos[var1.im] - var3.pos[0] >= -1 {
                        self.skiplev = (self.skiplev as f64 - 0.1) as f32;
                        if self.skiplev < 0.0 {
                            self.skiplev = 0.0;
                        }
                    } else {
                        self.skiplev = (self.skiplev as f64 + 0.2) as f32;
                        if self.skiplev > var5 {
                            self.skiplev = var5;
                        }
                    }

                    self.rampp = (m.random() * 4.0 - 2.0) as i32;
                    if var1.power == 98.0 {
                        self.rampp = -1;
                    }

                    if var1.power < 75.0 && self.rampp == -1 {
                        self.rampp = 0;
                    }

                    if var1.power < 60.0 {
                        self.rampp = 1;
                    }

                    if var3.stage == 6 {
                        self.rampp = 2;
                    }

                    if self.cntrn == 0 {
                        self.agressed = false;
                        self.turntyp = (m.random() * 4.0) as i32;
                        if (var3.stage == 3 || var3.stage == 9) && var1.im == 4 {
                            self.turntyp = 1;
                            if self.attack == 0 {
                                self.agressed = true;
                            }
                        }

                        if var3.pos[0] - var3.pos[var1.im] < 0 {
                            self.turntyp = (m.random() * 2.0) as i32;
                        }

                        if var3.stage == 10 {
                            self.turntyp = 2;
                        }

                        if self.attack != 0 {
                            self.turntyp = 2;
                            if var3.stage == 9 || var3.stage == 10 || var3.stage == 11 {
                                self.turntyp = (m.random() * 3.0) as i32;
                            }
                        }

                        if var3.stage == 6 {
                            self.turntyp = 1;
                            self.agressed = true;
                        }

                        if var3.stage == 7
                            || var3.stage == 9
                            || var3.stage == 10
                            || var3.stage == 11
                        {
                            self.agressed = true;
                        }

                        self.cntrn = 5;
                    } else {
                        self.cntrn -= 1;
                    }

                    // Safety margin: half the missing power scaled by a
                    // half-centred roll, capped at 20.
                    self.saftey = (((98.0 - var1.power) / 2.0) as f64
                        * ((m.random() / 2.0) as f64 + 0.5))
                        as i32;
                    if self.saftey > 20 {
                        self.saftey = 20;
                    }

                    var5 = 0.0;
                    if var3.stage == 1 {
                        var5 = 0.9;
                    }

                    if var3.stage == 2 {
                        var5 = 0.7;
                    }

                    if var3.stage == 4 {
                        var5 = 0.4;
                    }

                    self.mustland = var5 + ((m.random() / 2.0) as f64 - 0.25) as f32;
                    var5 = 1.0;
                    if var3.stage == 1 {
                        var5 = 5.0;
                    }

                    if var3.stage == 2 {
                        var5 = 2.0;
                    }

                    if var3.stage == 4 {
                        var5 = 1.5;
                    }

                    if var1.power > 50.0 {
                        if var3.pos[0] - var3.pos[var1.im] > 0 {
                            self.saftey = (self.saftey as f32 * var5) as i32;
                        } else {
                            self.mustland = 0.0;
                        }
                    } else {
                        self.mustland -= 0.5;
                    }

                    self.trickprf = (var1.power - 38.0) / 50.0 - m.random() / 2.0;
                    if var1.power < 60.0 {
                        self.trickprf = -1.0;
                    }

                    if var3.stage == 6 && self.trickprf as f64 > 0.5 {
                        self.trickprf = 0.5;
                    }

                    if var3.stage == 3 && var1.im == 4 && self.trickprf as f64 > 0.7 {
                        self.trickprf = 0.7;
                    }

                    if var3.stage == 11 {
                        self.trickprf = -1.0;
                    }

                    self.usebounce = m.random() > var1.power / 100.0;

                    if var3.stage == 9 {
                        self.usebounce = false;
                    }

                    self.perfection = m.random() <= var1.hitmag as f32 / MAXMAG[var1.cn] as f32;

                    if 100.0 * var1.hitmag as f32 / MAXMAG[var1.cn] as f32 > 60.0 {
                        self.perfection = true;
                    }

                    if var3.stage == 6
                        || var3.stage == 8
                        || var3.stage == 9
                        || var3.stage == 10
                        || var3.stage == 11
                    {
                        self.perfection = true;
                    }

                    if var3.stage == 3 && var1.im == 4 {
                        self.perfection = true;
                    }

                    self.clrnce = 5;
                    if self.attack == 0 {
                        let mut var6 = true;
                        if var3.stage == 3 || var3.stage == 1 || var3.stage == 4 || var3.stage == 9
                        {
                            var6 = self.afta;
                        }

                        if var3.stage == 8 || var3.stage == 6 {
                            var6 = false;
                        }

                        if var3.stage == 3 && var1.im == 4 {
                            var6 = false;
                        }

                        let mut var7 = false;
                        if var3.stage == 4 || var3.stage == 10 {
                            var7 = true;
                        }

                        if var3.stage == 3 && var1.im == 4 {
                            var7 = true;
                        }

                        let mut var8: i32 = 60;
                        if var3.stage == 3 || var3.stage == 11 {
                            var8 = 30;
                        }

                        if var3.stage == 5 {
                            var8 = 40;
                        }

                        if var3.stage == 6 && self.bulistc {
                            var8 = 40;
                        }

                        if var3.stage == 9 && self.bulistc {
                            var8 = 30;
                        }

                        for var9 in 0..5 {
                            if var9 != var1.im && var3.clear[var9] != -1 {
                                let mut var10 = var2.xz;
                                if self.zyinv {
                                    var10 += 180;
                                }

                                while var10 < 0 {
                                    var10 += 360;
                                }

                                while var10 > 180 {
                                    var10 -= 360;
                                }

                                let mut var11: i32 = 0;
                                if var3.opx[var9] - var2.x >= 0 {
                                    var11 = 180;
                                }

                                let mut var12 = ((90.0 + var11 as f64)
                                    + f64::atan(
                                        (var3.opz[var9] - var2.z) as f64
                                            / (var3.opx[var9] - var2.x) as f64,
                                    ) / (std::f64::consts::PI / 180.0))
                                    as i32;
                                while var12 < 0 {
                                    var12 += 360;
                                }

                                while var12 > 180 {
                                    var12 -= 360;
                                }

                                let mut var13 = (var10 - var12).abs();
                                if var13 > 180 {
                                    var13 = (var13 - 360).abs();
                                }

                                let mut var14 =
                                    2000 * (((var3.clear[var9] - var1.clear).abs()) + 1);
                                if (var3.stage == 6 || var3.stage == 9) && self.bulistc {
                                    var14 = 6000;
                                }

                                if var3.stage == 3
                                    && var1.im == 4
                                    && var3.wasted < 2
                                    && var14 > 4000
                                {
                                    var14 = 4000;
                                }

                                if var13 < 85 + 15 * (((var3.clear[var9] - var1.clear).abs()) + 1)
                                    && self.py(
                                        var2.x / 100,
                                        var3.opx[var9] / 100,
                                        var2.z / 100,
                                        var3.opz[var9] / 100,
                                    ) < var14
                                    && self.afta
                                    && var1.power > var8 as f32
                                {
                                    let mut var15 =
                                        (35 - (var3.clear[var9] - var1.clear).abs() * 10) as f32;
                                    if var15 < 1.0 {
                                        var15 = 1.0;
                                    }

                                    let mut var16 = ((var3.pos[var1.im] + 1) * (5 - var3.pos[var9]))
                                        as f32
                                        / var15;
                                    if var3.stage != 11 && var16 as f64 > 0.7 {
                                        var16 = 0.7;
                                    }

                                    if var9 != 0 && var3.pos[0] < var3.pos[var1.im] {
                                        var16 = 0.0;
                                    }

                                    if var9 != 0 && var7 {
                                        var16 = 0.0;
                                    }

                                    if var7 && var3.stage == 3 && var9 == 0 {
                                        if var3.wasted >= 2 {
                                            var16 *= 0.5;
                                        } else {
                                            var16 = 0.0;
                                        }
                                    }

                                    if (var3.stage == 3 || var3.stage == 9) && var9 == 4 {
                                        var16 = 0.0;
                                    }

                                    if var3.stage == 6 {
                                        var16 = 0.0;
                                        if self.bulistc && var9 == 0 {
                                            var16 = 1.0;
                                        }
                                    }

                                    if var3.stage == 8 {
                                        var16 = 0.0;
                                    }

                                    if var3.stage == 9 && var1.im == 4 {
                                        var16 = 0.0;
                                    }

                                    if var3.stage == 9 && self.bulistc {
                                        if var9 == 0 {
                                            var16 = 1.0;
                                        } else {
                                            var16 = 0.0;
                                        }
                                    }

                                    if var3.stage == 9
                                        && (var3.pos[var9] == 4 || var3.pos[var9] == 3)
                                    {
                                        var16 = 0.0;
                                    }

                                    if m.random() < var16 {
                                        self.attack =
                                            40 * ((var3.clear[var9] - var1.clear).abs() + 1);
                                        if self.attack > 500 {
                                            self.attack = 500;
                                        }

                                        self.acr = var9 as i32;
                                        self.turntyp = (1.0 + m.random() * 2.0) as i32;
                                    }
                                }

                                if var6
                                    && var13 > 100
                                    && self.py(
                                        var2.x / 100,
                                        var3.opx[var9] / 100,
                                        var2.z / 100,
                                        var3.opz[var9] / 100,
                                    ) < 300
                                    && m.random() as f64
                                        > 0.6 - (var3.pos[var1.im] as f32 / 10.0) as f64
                                {
                                    self.clrnce = 0;
                                    self.acuracy = 0;
                                }
                            }
                        }
                    }

                    let mut var22 = false;
                    if var3.stage == 6 || var3.stage == 8 || var3.stage == 11 {
                        var22 = true;
                    }

                    if var3.stage == 9 && var1.im == 4 {
                        var22 = true;
                    }

                    if self.trfix != 3 {
                        self.trfix = 0;
                        if 100.0 * var1.hitmag as f32 / MAXMAG[var1.cn] as f32 > 50.0 {
                            self.trfix = 1;
                        }

                        if !var22 && 100.0 * var1.hitmag as f32 / MAXMAG[var1.cn] as f32 > 80.0 {
                            self.trfix = 2;
                        }
                    } else {
                        self.upwait = 0;
                        self.acuracy = 0;
                        self.skiplev = 1.0;
                        self.clrnce = 2;
                    }

                    if !self.bulistc {
                        if var3.stage == 6 && var1.cn == 7 {
                            self.bulistc = true;
                        }

                        if var3.stage == 9
                            && self.afta
                            && (var3.pos[var1.im] == 4 || var3.pos[var1.im] == 3)
                            && var1.cn != 9
                            && self.trfix != 0
                        {
                            self.bulistc = true;
                        }
                    }

                    self.stcnt = 0;
                    self.statusque = (20.0 * m.random()) as i32;
                } else {
                    self.stcnt += 1;
                }
            }

            let var20 = if self.usebounce {
                var1.wtouch
            } else {
                var1.mtouch
            };

            if var20 {
                if self.trickfase != 0 {
                    self.trickfase = 0;
                }

                if self.trfix == 2 || self.trfix == 3 {
                    self.attack = 0;
                }

                if self.attack != 0 {
                    self.up = true;
                    let mut var24: i32 = 0;
                    if var3.opx[self.acr as usize] - var2.x >= 0 {
                        var24 = 180;
                    }

                    self.pan = ((90.0 + var24 as f64)
                        + f64::atan(
                            (var3.opz[self.acr as usize] - var2.z) as f64
                                / (var3.opx[self.acr as usize] - var2.x) as f64,
                        ) / (std::f64::consts::PI / 180.0)) as i32;
                    self.attack -= 1;
                    if self.attack <= 0 {
                        self.attack = 0;
                    }
                } else {
                    if self.upcnt < 30 {
                        self.up = true;
                    }

                    if self.upcnt < 25 + self.actwait {
                        self.upcnt += 1;
                    } else {
                        self.upcnt = 0;
                        self.actwait = self.upwait;
                    }

                    let mut var23: i32 = var1.point;
                    let mut var31: i32 = 50;
                    if var3.stage == 9 {
                        var31 = 20;
                    }

                    if self.bulistc
                        && self.trfix != 2
                        && self.trfix != 3
                        && self.trfix != 4
                        && !(var1.power < var31 as f32)
                    {
                        var23 -= 2;
                        if var23 < 0 {
                            var23 += var3.n;
                        }

                        if var3.stage == 9 && var23 > 76 {
                            var23 = 76;
                        }

                        while var3.typ[var23 as usize] == -4 {
                            var23 -= 1;
                            if var23 < 0 {
                                var23 += var3.n;
                            }
                        }

                        var1.nofocus = true;
                    } else {
                        if self.rampp == 1 && var3.typ[var23 as usize] <= 0 {
                            let mut var32 = var23 + 1;
                            if var32 == var3.n {
                                var32 = 0;
                            }

                            if var3.typ[var32 as usize] == -2 {
                                var23 = var32;
                            }
                        }

                        if self.rampp == -1 && var3.typ[var23 as usize] == -2 {
                            var23 += 1;
                            if var23 == var3.n {
                                var23 = 0;
                            }
                        }

                        if !(m.random() > self.skiplev) {
                            if m.random() > self.skiplev {
                                while var3.typ[var23 as usize] == -1 {
                                    var23 += 1;
                                    if var23 == var3.n {
                                        var23 = 0;
                                    }
                                }
                            }
                        } else {
                            let mut var33 = var23;
                            let mut var38 = false;
                            if var3.typ[var23 as usize] > 0 {
                                let mut var40 = 0i32;
                                for var43 in 0..var3.n as usize {
                                    if var3.typ[var43] > 0 && var43 < var33 as usize {
                                        var40 += 1;
                                    }
                                }

                                var38 = var1.clear != var40 + var1.nlaps * var3.nsp;
                            }

                            while var3.typ[var33 as usize] == 0
                                || var3.typ[var33 as usize] == -1
                                || var3.typ[var33 as usize] == -3
                                || var38
                            {
                                var23 = var33;
                                var33 += 1;
                                if var33 == var3.n {
                                    var33 = 0;
                                }

                                var38 = false;
                                if var3.typ[var33 as usize] > 0 {
                                    let mut var41 = 0i32;
                                    for var44 in 0..var3.n as usize {
                                        if var3.typ[var44] > 0 && var44 < var33 as usize {
                                            var41 += 1;
                                        }
                                    }

                                    var38 = var1.clear != var41 + var1.nlaps * var3.nsp;
                                }
                            }
                        }
                    }

                    if self.rampp == 2 {
                        let mut var34 = var23 + 1;
                        if var34 == var3.n {
                            var34 = 0;
                        }

                        if var3.typ[var34 as usize] == -2 && var23 != var1.point {
                            var23 -= 1;
                            if var23 < 0 {
                                var23 += var3.n;
                            }
                        }
                    }

                    if self.bulistc {
                        var1.nofocus = true;
                    }
                    if var3.stage != 11
                        && (var1.missedcp == 0
                            || self.forget
                            || self.trfix == 4
                            || var3.stage == 10)
                        && self.trfix != 0
                    {
                        if self.trfix == 2 {
                            let mut var35 = -10i32;
                            let mut var39 = 0usize;
                            for var42 in 0..var3.fn_ as usize {
                                let d = self.py(
                                    var2.x / 100,
                                    var3.x[self.fpnt[var42] as usize] / 100,
                                    var2.z / 100,
                                    var3.z[self.fpnt[var42] as usize] / 100,
                                );
                                if d < var35 || var35 == -10 {
                                    var35 = d;
                                    var39 = var42;
                                }
                            }

                            if var3.stage == 9 && self.bulistc {
                                var39 = 1;
                            }

                            var23 = self.fpnt[var39];
                            self.forget = var3.special[var39];
                        }

                        for var36 in 0..var3.fn_ as usize {
                            if self.py(
                                var2.x / 100,
                                var3.x[self.fpnt[var36] as usize] / 100,
                                var2.z / 100,
                                var3.z[self.fpnt[var36] as usize] / 100,
                            ) < 2000
                            {
                                self.forget = false;
                                self.actwait = 0;
                                self.upwait = 0;
                                self.turntyp = 2;
                                self.randtcnt = -1;
                                self.acuracy = 0;
                                self.rampp = 0;
                                self.trfix = 3;
                            }
                        }

                        if self.trfix == 3 {
                            var1.nofocus = true;
                        }
                    }

                    if self.turncnt > self.randtcnt {
                        let mut var37: i32 = 0;
                        if var3.x[var23 as usize] - var2.x >= 0 {
                            var37 = 180;
                        }

                        self.pan = ((90.0 + var37 as f64)
                            + f64::atan(
                                ((var3.z[var23 as usize] - var2.z) as f64)
                                    / ((var3.x[var23 as usize] - var2.x) as f64),
                            ) / (std::f64::consts::PI / 180.0))
                            as i32;
                        self.turncnt = 0;
                        self.randtcnt = (self.acuracy as f32 * m.random()) as i32;
                    } else {
                        self.turncnt += 1;
                    }
                }
                let mut var25 = var2.xz;
                if self.zyinv {
                    var25 += 180;
                }

                while var25 < 0 {
                    var25 += 360;
                }

                while var25 > 180 {
                    var25 -= 360;
                }

                while self.pan < 0 {
                    self.pan += 360;
                }

                while self.pan > 180 {
                    self.pan -= 360;
                }

                if self.wall != -1 && self.hold == 0 {
                    self.clrnce = 0;
                }

                if self.hold == 0 {
                    if (var25 - self.pan).abs() < 180 {
                        if (var25 - self.pan).abs() > self.clrnce {
                            if var25 < self.pan {
                                self.left = true;
                                self.lastl = true;
                            } else {
                                self.right = true;
                                self.lastl = false;
                            }

                            if (var25 - self.pan).abs() > 50
                                && var1.speed > SWITS[var1.cn][0] as f32
                                && self.turntyp != 0
                            {
                                if self.turntyp == 1 {
                                    self.down = true;
                                }

                                if self.turntyp == 2 {
                                    self.handb = true;
                                }

                                if !self.agressed {
                                    self.up = false;
                                }
                            }
                        }
                    } else if (var25 - self.pan).abs() < 360 - self.clrnce {
                        if var25 < self.pan {
                            self.right = true;
                            self.lastl = false;
                        } else {
                            self.left = true;
                            self.lastl = true;
                        }

                        if (var25 - self.pan).abs() < 310
                            && var1.speed > SWITS[var1.cn][0] as f32
                            && self.turntyp != 0
                        {
                            if self.turntyp == 1 {
                                self.down = true;
                            }

                            if self.turntyp == 2 {
                                self.handb = true;
                            }

                            if !self.agressed {
                                self.up = false;
                            }
                        }
                    }
                }

                if self.wall != -1 {
                    if self.lwall != self.wall {
                        if self.lastl {
                            self.left = true;
                        } else {
                            self.right = true;
                        }

                        self.wlastl = self.lastl;
                        self.lwall = self.wall;
                    } else if self.wlastl {
                        self.left = true;
                    } else {
                        self.right = true;
                    }

                    if var4.dam[self.wall as usize] != 0 {
                        self.hold += 1;
                        if self.hold > 10 {
                            self.hold = 10;
                        }
                    } else {
                        self.hold = 1;
                    }

                    self.wall = -1;
                } else if self.hold != 0 {
                    self.hold -= 1;
                }
            } else {
                if self.trickfase == 0 {
                    let var26 = ((var1.scy[0] + var1.scy[1] + var1.scy[2] + var1.scy[3])
                        * (var2.y - 300) as f32
                        / 4000.0) as i32;
                    if var26 > 9 && m.random() > self.trickprf / 3.0 {
                        self.oxy = var1.pxy;
                        self.ozy = var1.pzy;
                        self.flycnt = 0;
                        self.uddirect = 0;
                        self.lrdirect = 0;
                        self.udswt = false;
                        self.lrswt = false;
                        self.trickfase = 1;
                        if var26 < 16 {
                            self.uddirect = -1;
                            self.udstart = 0;
                            self.udswt = false;
                        } else if !(m.random() > m.random()) && var3.stage != 8 {
                            if m.random() > m.random() {
                                self.lrdirect = -1;
                            } else {
                                self.lrdirect = 1;
                            }

                            self.lrstart = (10.0 * m.random() * self.trickprf) as i32;
                            if m.random() as f64 > 0.75 {
                                self.lrswt = true;
                            }

                            if m.random() > self.trickprf + 0.3 {
                                if m.random() > m.random() {
                                    self.uddirect = -1;
                                } else {
                                    self.uddirect = 1;
                                }

                                self.udstart = (30.0 * m.random()) as i32;
                                if m.random() as f64 > 0.85 {
                                    self.udswt = true;
                                }
                            }
                        } else {
                            if m.random() > m.random() {
                                self.uddirect = -1;
                            } else {
                                self.uddirect = 1;
                            }

                            self.udstart = (10.0 * m.random() * self.trickprf) as i32;
                            if m.random() as f64 > 0.85 {
                                self.udswt = true;
                            }

                            if m.random() > self.trickprf + 0.3 && var3.stage != 8 {
                                if m.random() > m.random() {
                                    self.lrdirect = -1;
                                } else {
                                    self.lrdirect = 1;
                                }

                                self.lrstart = (30.0 * m.random()) as i32;
                                if m.random() as f64 > 0.75 {
                                    self.lrswt = true;
                                }
                            }
                        }

                        if self.trfix == 3 || self.trfix == 4 {
                            if var3.stage != 8 {
                                if self.lrdirect == -1 || var1.power < 60.0 {
                                    self.uddirect = -1;
                                }

                                self.lrdirect = 0;
                            } else if self.uddirect != 0 {
                                self.uddirect = 1;
                                self.udswt = false;
                            }
                        }
                    } else {
                        self.trickfase = -1;
                    }

                    if !self.afta {
                        self.afta = true;
                    }

                    if self.trfix == 3 {
                        self.trfix = 4;
                        self.statusque += 30;
                    }
                }

                if self.trickfase == 1 {
                    self.flycnt += 1;
                    if self.lrdirect != 0 && self.flycnt > self.lrstart {
                        if self.lrswt && (var1.pxy - self.oxy).abs() > 180 {
                            if self.lrdirect == -1 {
                                self.lrdirect = 1;
                            } else {
                                self.lrdirect = -1;
                            }

                            self.lrswt = false;
                        }

                        if self.lrdirect == -1 {
                            self.handb = true;
                            self.left = true;
                        } else {
                            self.handb = true;
                            self.right = true;
                        }
                    }

                    if self.uddirect != 0 && self.flycnt > self.udstart {
                        if self.udswt && (var1.pzy - self.ozy).abs() > 180 {
                            if self.uddirect == -1 {
                                self.uddirect = 1;
                            } else {
                                self.uddirect = -1;
                            }

                            self.udswt = false;
                        }

                        if self.uddirect == -1 {
                            self.handb = true;
                            self.down = true;
                        } else {
                            self.handb = true;
                            self.up = true;
                        }
                    }

                    if (var1.scy[0] + var1.scy[1] + var1.scy[2] + var1.scy[3]) * 100.0
                        / ((var2.y - 300) as f32)
                        < -(self.saftey as f32)
                    {
                        self.onceu = false;
                        self.onced = false;
                        self.oncel = false;
                        self.oncer = false;
                        self.lrcomp = false;
                        self.udcomp = false;
                        self.udbare = false;
                        self.lrbare = false;
                        self.trickfase = 2;
                        self.swat = 0;
                    }
                }

                if self.trickfase == 2 {
                    if self.swat == 0 {
                        if var1.dcomp != 0.0 || var1.ucomp != 0.0 {
                            self.udbare = true;
                        }

                        if var1.lcomp != 0.0 || var1.rcomp != 0.0 {
                            self.lrbare = true;
                        }

                        self.swat = 1;
                    }

                    if var1.wtouch {
                        if self.swat == 1 {
                            self.swat = 2;
                        }
                    } else if self.swat == 2 {
                        if var1.capsized && m.random() > self.mustland {
                            if self.udbare {
                                self.lrbare = true;
                                self.udbare = false;
                            } else if self.lrbare {
                                self.udbare = true;
                                self.lrbare = false;
                            }
                        }

                        self.swat = 3;
                    }

                    if self.udbare {
                        let mut var27 = var1.pzy + 90;
                        while var27 < 0 {
                            var27 += 360;
                        }

                        while var27 > 180 {
                            var27 -= 360;
                        }

                        var27 = var27.abs();
                        if var1.lcomp - var1.rcomp < 5.0 && (self.onced || self.onceu) {
                            self.udcomp = true;
                        }

                        if var1.dcomp > var1.ucomp {
                            if var1.capsized {
                                if self.udcomp {
                                    if var27 > 90 {
                                        self.up = true;
                                    } else {
                                        self.down = true;
                                    }
                                } else if !self.onced {
                                    self.down = true;
                                }
                            } else {
                                if self.udcomp {
                                    if self.perfection && (var27 - 90).abs() > 30 {
                                        if var27 > 90 {
                                            self.up = true;
                                        } else {
                                            self.down = true;
                                        }
                                    }
                                } else if m.random() > self.mustland {
                                    self.up = true;
                                }

                                self.onced = true;
                            }
                        } else if var1.capsized {
                            if self.udcomp {
                                if var27 > 90 {
                                    self.up = true;
                                } else {
                                    self.down = true;
                                }
                            } else if !self.onceu {
                                self.up = true;
                            }
                        } else {
                            if self.udcomp {
                                if self.perfection && (var27 - 90).abs() > 30 {
                                    if var27 > 90 {
                                        self.up = true;
                                    } else {
                                        self.down = true;
                                    }
                                }
                            } else if m.random() > self.mustland {
                                self.down = true;
                            }

                            self.onceu = true;
                        }
                    }

                    if self.lrbare {
                        let mut var29 = var1.pxy + 90;
                        if self.zyinv {
                            var29 += 180;
                        }

                        while var29 < 0 {
                            var29 += 360;
                        }

                        while var29 > 180 {
                            var29 -= 360;
                        }

                        var29 = var29.abs();
                        if var1.lcomp - var1.rcomp < 10.0 && (self.oncel || self.oncer) {
                            self.lrcomp = true;
                        }

                        if var1.lcomp > var1.rcomp {
                            if var1.capsized {
                                if self.lrcomp {
                                    if var29 > 90 {
                                        self.left = true;
                                    } else {
                                        self.right = true;
                                    }
                                } else if !self.oncel {
                                    self.left = true;
                                }
                            } else {
                                if self.lrcomp {
                                    if self.perfection && (var29 - 90).abs() > 30 {
                                        if var29 > 90 {
                                            self.left = true;
                                        } else {
                                            self.right = true;
                                        }
                                    }
                                } else if m.random() > self.mustland {
                                    self.right = true;
                                }

                                self.oncel = true;
                            }
                        } else if var1.capsized {
                            if self.lrcomp {
                                if var29 > 90 {
                                    self.left = true;
                                } else {
                                    self.right = true;
                                }
                            } else if !self.oncer {
                                self.right = true;
                            }
                        } else {
                            if self.lrcomp {
                                if self.perfection && (var29 - 90).abs() > 30 {
                                    if var29 > 90 {
                                        self.left = true;
                                    } else {
                                        self.right = true;
                                    }
                                }
                            } else if m.random() > self.mustland {
                                self.left = true;
                            }

                            self.oncer = true;
                        }
                    }
                }
            }
        }
    }
}

impl Default for Control {
    fn default() -> Control {
        Control::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoints::CP_CAP;

    #[test]
    fn falseo_releases_inputs() {
        let mut c = Control::new();
        c.left = true;
        c.up = true;
        c.enter = true;
        c.falseo();
        assert!(!c.left && !c.up && !c.enter && !c.handb);
    }

    #[test]
    fn reset_maps_fix_markers_ahead_of_nearest_facepoint() {
        let mut cp = CheckPoints::new();
        // Ring at x=0..nsp·100, z=0 hundredths; one fix marker 5 facepoints
        // behind the start (x≈−40000+...) → fpnt 1, backed off −4 → nsp−3.
        cp.nsp = 4;
        cp.n = 4;
        cp.fn_ = 1;
        cp.fx[0] = -400; // 4 hundredths "behind" facepoint 0 in x/100 space
        cp.fz[0] = 0;
        for w in 0..4usize {
            cp.x[w] = (w * 100) as i32;
            cp.z[w] = 0;
        }
        let mut c = Control::new();
        c.reset(&cp);
        // nearest facepoint for (x=-4, z=0) is index 0; −4 wrap over nsp=4 → 0
        assert_eq!(c.fpnt[0], 0);
    }

    #[test]
    fn py_matches_java_wrap() {
        let c = Control::new();
        assert_eq!(c.py(3, 1, 4, 2), 8);
        assert_eq!(c.py(CP_CAP as i32, CP_CAP as i32, 0, 0), 0);
    }

    // `preform` fixtures — the AI slots run the same chain pieces as the
    // player; rigs reuse the drive/record blank pieces.

    fn blank_conto() -> Conto {
        use crate::conto::Base;
        let m = Medium::new();
        let mut t = Trackers::new();
        let base = Base {
            keyx: [10, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        Conto::place(&base, &m, &mut t, 0, 0, 0, 0)
    }

    fn ai_rig(im: usize) -> (Madness, Conto, Control, CheckPoints, Medium, Trackers) {
        let mut mad = Madness::new(im);
        let mut o = blank_conto();
        let mut cp = CheckPoints::new();
        mad.reseto(im, &mut o, &mut cp);
        o.grat = 30;
        o.y = 250 - 30;
        mad.mtouch = true;
        // A three-facepoint ring: gate, key checkpoint, gate — both walks
        // (rampp back-step and the skiplev clear-count probe) settle on a
        // `typ` row after one step like a real route list.
        cp.n = 3;
        cp.nsp = 1;
        cp.typ[0] = -2;
        cp.typ[1] = 1;
        cp.typ[2] = -2;
        cp.x[0] = 999900;
        cp.x[1] = 999900;
        cp.x[2] = 999900;
        let ctl = Control::new();
        let m = Medium::new();
        let t = Trackers::new();
        (mad, o, ctl, cp, m, t)
    }

    #[test]
    fn preform_strategy_arms_after_the_statusque_frame() {
        let (mut mad, o, mut ctl, mut cp, mut m, t) = ai_rig(1);
        // pos [3,1] → acuracy (7−1)·3·(6−2) = 72, the upwait cube 2³·2.0
        // floors at the stage-1 minimum 20, and skiplev decays 0.2 on the
        // trailing branch. Call 1 only counts (stcnt 0 is not > 0 yet).
        cp.pos[0] = 3;
        cp.pos[1] = 1;
        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert!(ctl.up, "call 1 already presses through the grounded branch");
        assert_eq!(ctl.upcnt, 1);

        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert_eq!(ctl.acuracy, 72);
        assert_eq!(ctl.upwait, 20);
        // Behind the player (pos 1 vs 3) the appetite raises 1.0 → 1.2 and
        // the stage-1||2 cap (`var5` = 1.0) clamps it right back.
        assert_eq!(ctl.skiplev, 1.0);
        assert_eq!(ctl.rampp, -1, "power == 98 pins the ramp appetite back");
        assert_eq!(ctl.saftey, 0);
        assert_eq!(ctl.cntrn, 5);
        assert_eq!(ctl.stcnt, 0);
        assert!(
            (0..=20).contains(&ctl.statusque),
            "statusque = (int)(20 * random)"
        );
        assert!((0..=3).contains(&ctl.turntyp));
        assert!(!ctl.usebounce, "damage-free rolls stay under the 0.98 gate");
        assert!(!ctl.perfection);
        assert_eq!(ctl.upcnt, 2);
        assert!(ctl.up);
        // The point walk took the gate (typ -2) → aim ahead: pan folds to
        // −90, car at angle 0 steers right.
        assert!(ctl.right && !ctl.left);
        assert!(!ctl.lastl);
        assert!(!ctl.down && !ctl.handb, "speed 0 never deep-turns");
        assert!(!mad.nofocus);
        assert_eq!(ctl.trickfase, 0);
    }

    #[test]
    fn preform_air_arms_trick_fase_one_at_low_lift() {
        let (mut mad, mut o, mut ctl, cp, mut m, t) = ai_rig(1);
        // Airborne over the 300 line with +200 spring lift: var26 = 12
        // gates past 9, the sub-16 notch takes the plain down-press side
        // (no roll), and fase 1 fires with the press in the same frame.
        mad.mtouch = false;
        mad.wtouch = false;
        o.y = 550;
        mad.scy = [50.0; 4];
        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert_eq!(ctl.trickfase, 1);
        assert!(ctl.afta);
        assert_eq!(ctl.flycnt, 1);
        assert!(ctl.handb && ctl.down, "uddirect −1 presses handb + down");
        assert!(!ctl.left && !ctl.right && !ctl.up);
        assert_eq!(ctl.uddirect, -1);
        assert_eq!(ctl.udstart, 0);
        // The dump needs −saftey above the lift ratio (80 ≥ 0): fase 1 holds.
        assert_eq!(ctl.trickfase, 1);
    }

    #[test]
    fn preform_air_marks_number_crunch_failure_minus_one() {
        let (mut mad, mut o, mut ctl, cp, mut m, t) = ai_rig(1);
        // −200 lift over the 300 line: var26 truncates to −12, never
        // reaches 9 → fase −1 (still marks afta for the attack scan).
        mad.mtouch = false;
        mad.wtouch = false;
        o.y = 550;
        mad.scy = [-50.0; 4];
        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert_eq!(ctl.trickfase, -1);
        assert!(ctl.afta);
        assert!(!ctl.up && !ctl.down && !ctl.handb);
    }

    #[test]
    fn preform_dump_falls_into_the_swat_ladder() {
        let (mut mad, mut o, mut ctl, cp, mut m, t) = ai_rig(1);
        mad.mtouch = false;
        mad.wtouch = false;
        mad.dcomp = 2.0;
        mad.ucomp = 0.0;
        mad.lcomp = 3.0;
        mad.rcomp = 0.0;
        o.y = 550;
        mad.scy = [50.0; 4];
        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert_eq!(ctl.trickfase, 1);
        // The lift flips negative and the drop closes in: the same-frame
        // dump gate lands fase 2, whose swat-0 arm picks up the pressed
        // comps (both barrels armed) and ticks swat to 1.
        mad.scy = [-100.0; 4];
        o.y = 500;
        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert_eq!(ctl.trickfase, 2);
        assert_eq!(ctl.swat, 1);
        assert!(ctl.udbare && ctl.lrbare);
        assert!(ctl.wlastl == ctl.lastl, "wall reaction untouched in air");
        // Next tick: wtouch false with swat 1 waits; the ud barrel records
        // its once-flap and arms the comp mode.
        ctl.preform(&mut mad, &o, &cp, &t, &mut m);
        assert_eq!(ctl.swat, 1);
        assert!(ctl.onced);
        assert!(ctl.udcomp);
    }
}
