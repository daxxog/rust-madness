// FIDELITY: Record.java: indexed loops, `x>330 || x<30` angle gates and the literal `0 + var16/var17` expression mirror the original.
#![allow(
    clippy::needless_range_loop,
    clippy::manual_range_contains,
    clippy::identity_op
)]
//! Port of `Record.java` — the cross-car recorder behind the crash and
//! mid-run cinematics: a 300-frame ring of per-car pose snapshots, the
//! 6-row staged contour table shifted every 50th frame, per-wheel spark
//! capture, the dent-reply rings the Madness damage registers feed
//! (`recx`/`recy`/`recz`), the fix/destroy reply timers, and the wasted
//! freeze `cotchinow` that copies the lot into the `h`-tagged shadow
//! tables.
//!
//! Live in this slice: `new`, `reset`, `rec`, `cotchinow`, the three
//! register shims, and the replay painters — Record's own `play`
//! (Record.java:471-541) and `playh` (287-367) bodies plus the internal
//! `regx`/`regy`/`regz`/`chipx`/`chipz` re-application the GameSparker
//! `fase == -1` (race-end, :1152) and `fase == -3` (wasted, :1258)
//! branches consume.
//!
//! The painter copies of the dent pushes differ from Madness's live
//! ones exactly by the live-only extras: no `DAMMULT`, no `hitmag`
//! counting, no `recx/recy/recz` save-back and no `nbsq` squash
//! gate/reset — the recorded mag table and the staged `squash[0][im]`
//! accumulator drive the replay. `regy` again gates on `var2 > 100` only
//! (the positive bar; negative mags never dent), while `regx`/`regz`
//! re-run the ±100 shift under an `abs > 100` gate. `ocar` rows stay
//! `None` until the GameSparker replay slice fills them, mirroring the
//! Java null window.
//!
//! Parity notes kept:
//! - `recx` (Record.java:633-641) and `recz` (841-849) index their write
//!   slot with `nry` while incrementing `nrx`/`nrz` — the original's
//!   cross-indexed counters, reproduced byte for byte.
//! - `reset` fills `starcar` only while `prepit` is set and flips
//!   `prepit` off at the end: the pristine fingerprint snapshot happens
//!   once per applet run (the first ever `loadstage`); later refreshes
//!   come from `cotchinow`.
//! - The spark/dent scrub loop of `reset` walks car slots `[0..4)` —
//!   Record.java's outer bound is 4 of 5 rows, so slot 4 keeps its
//!   previous-race spark/dent reply state.
//! - `dest[slot]` decrements only while the car's `cntdest` is nonzero,
//!   and the wasted replay arms exactly when it reaches 230.

pub const PLAYERS: usize = crate::checkpoints::PLAYERS;
pub const RING: usize = 300;
pub const WHEELS: usize = 4;
pub const DENTS: usize = 7;
pub const SPARKS: usize = 30;
/// The staged contour table rows (`ContO[6][5] car`).
pub const CAR_ROWS: usize = 6;

use crate::conto::Conto;
use crate::madness::{CLRAD, FLIPY, MSQUASH, Madness};
use crate::medium::Medium;

type SlotRow = [i32; PLAYERS];

pub struct Record {
    pub caught: i32,
    pub hcaught: bool,
    pub prepit: bool,
    /// `ocar` — null in Java until the after-race branch (the GameSparker
    /// `fase == -1` slice) fills it; the replay milestone awaits.
    pub ocar: Vec<Option<crate::conto::Conto>>,
    pub cntf: i32,
    /// `car` — the staged contours, null until the first `reset` fills
    /// every row (`loadstage` always precedes the first `rec`, but the
    /// port keeps the same precondition instead of synthesizing rows).
    pub car: Vec<Vec<Option<crate::conto::Conto>>>,
    pub squash: Vec<Vec<i32>>,
    pub fix: [i32; PLAYERS],
    pub dest: [i32; PLAYERS],
    pub x: [SlotRow; RING],
    pub y: [SlotRow; RING],
    pub z: [SlotRow; RING],
    pub xy: [SlotRow; RING],
    pub zy: [SlotRow; RING],
    pub xz: [SlotRow; RING],
    pub wxz: [SlotRow; RING],
    pub wzy: [SlotRow; RING],
    pub ns: [[i32; WHEELS]; PLAYERS],
    pub sspark: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub sx: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub sy: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub sz: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub smag: [[[f32; SPARKS]; WHEELS]; PLAYERS],
    pub scx: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub scz: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub fulls: [[[bool; SPARKS]; WHEELS]; PLAYERS],
    pub nry: [[i32; WHEELS]; PLAYERS],
    pub ry: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub magy: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub mtouch: [[bool; DENTS]; PLAYERS],
    pub nrx: [[i32; WHEELS]; PLAYERS],
    pub rx: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub magx: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub nrz: [[i32; WHEELS]; PLAYERS],
    pub rz: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub magz: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub wasted: i32,
    pub whenwasted: i32,
    pub powered: i32,
    /// `starcar` — null in Java until the first `prepit` reset or a
    /// `cotchinow` fills the rows.
    pub starcar: Vec<Option<crate::conto::Conto>>,
    pub hsquash: [i32; PLAYERS],
    pub hfix: [i32; PLAYERS],
    pub hdest: [i32; PLAYERS],
    pub hx: [SlotRow; RING],
    pub hy: [SlotRow; RING],
    pub hz: [SlotRow; RING],
    pub hxy: [SlotRow; RING],
    pub hzy: [SlotRow; RING],
    pub hxz: [SlotRow; RING],
    pub hwxz: [SlotRow; RING],
    pub hwzy: [SlotRow; RING],
    pub hns: [[i32; WHEELS]; PLAYERS],
    pub hsspark: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub hsx: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub hsy: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub hsz: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub hsmag: [[[f32; SPARKS]; WHEELS]; PLAYERS],
    pub hscx: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub hscz: [[[i32; SPARKS]; WHEELS]; PLAYERS],
    pub hfulls: [[[bool; SPARKS]; WHEELS]; PLAYERS],
    pub hnry: [[i32; WHEELS]; PLAYERS],
    pub hry: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub hmagy: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub hnrx: [[i32; WHEELS]; PLAYERS],
    pub hrx: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub hmagx: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub hnrz: [[i32; WHEELS]; PLAYERS],
    pub hrz: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub hmagz: [[[i32; DENTS]; WHEELS]; PLAYERS],
    pub hmtouch: [[bool; DENTS]; PLAYERS],
    pub cntdest: [i32; PLAYERS],
    pub lastfr: i32,
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

impl Record {
    /// The field-initializer defaults (Record.java:5-77) and the
    /// constructor `Record(Medium)` (807-811): the `m` reference stays a
    /// parameter in the port; the constructor's `cotchinow(0)` is inert
    /// while `caught < 300` and needs no port.
    pub fn new() -> Record {
        Record {
            caught: 0,
            hcaught: false,
            prepit: true,
            ocar: (0..PLAYERS).map(|_| None).collect(),
            cntf: 50,
            car: (0..CAR_ROWS)
                .map(|_| (0..PLAYERS).map(|_| None).collect())
                .collect(),
            squash: vec![vec![0; PLAYERS]; CAR_ROWS],
            fix: [0; PLAYERS],
            dest: [0; PLAYERS],
            x: [[0; PLAYERS]; RING],
            y: [[0; PLAYERS]; RING],
            z: [[0; PLAYERS]; RING],
            xy: [[0; PLAYERS]; RING],
            zy: [[0; PLAYERS]; RING],
            xz: [[0; PLAYERS]; RING],
            wxz: [[0; PLAYERS]; RING],
            wzy: [[0; PLAYERS]; RING],
            ns: [[0; WHEELS]; PLAYERS],
            sspark: [[[0; SPARKS]; WHEELS]; PLAYERS],
            sx: [[[0; SPARKS]; WHEELS]; PLAYERS],
            sy: [[[0; SPARKS]; WHEELS]; PLAYERS],
            sz: [[[0; SPARKS]; WHEELS]; PLAYERS],
            smag: [[[0.0; SPARKS]; WHEELS]; PLAYERS],
            scx: [[[0; SPARKS]; WHEELS]; PLAYERS],
            scz: [[[0; SPARKS]; WHEELS]; PLAYERS],
            fulls: [[[false; SPARKS]; WHEELS]; PLAYERS],
            nry: [[0; WHEELS]; PLAYERS],
            ry: [[[0; DENTS]; WHEELS]; PLAYERS],
            magy: [[[0; DENTS]; WHEELS]; PLAYERS],
            mtouch: [[false; DENTS]; PLAYERS],
            nrx: [[0; WHEELS]; PLAYERS],
            rx: [[[0; DENTS]; WHEELS]; PLAYERS],
            magx: [[[0; DENTS]; WHEELS]; PLAYERS],
            nrz: [[0; WHEELS]; PLAYERS],
            rz: [[[0; DENTS]; WHEELS]; PLAYERS],
            magz: [[[0; DENTS]; WHEELS]; PLAYERS],
            wasted: 0,
            whenwasted: 0,
            powered: 0,
            starcar: (0..PLAYERS).map(|_| None).collect(),
            hsquash: [0; PLAYERS],
            hfix: [-1; PLAYERS],
            hdest: [-1; PLAYERS],
            hx: [[0; PLAYERS]; RING],
            hy: [[0; PLAYERS]; RING],
            hz: [[0; PLAYERS]; RING],
            hxy: [[0; PLAYERS]; RING],
            hzy: [[0; PLAYERS]; RING],
            hxz: [[0; PLAYERS]; RING],
            hwxz: [[0; PLAYERS]; RING],
            hwzy: [[0; PLAYERS]; RING],
            hns: [[0; WHEELS]; PLAYERS],
            hsspark: [[[0; SPARKS]; WHEELS]; PLAYERS],
            hsx: [[[0; SPARKS]; WHEELS]; PLAYERS],
            hsy: [[[0; SPARKS]; WHEELS]; PLAYERS],
            hsz: [[[0; SPARKS]; WHEELS]; PLAYERS],
            hsmag: [[[0.0; SPARKS]; WHEELS]; PLAYERS],
            hscx: [[[0; SPARKS]; WHEELS]; PLAYERS],
            hscz: [[[0; SPARKS]; WHEELS]; PLAYERS],
            hfulls: [[[false; SPARKS]; WHEELS]; PLAYERS],
            hnry: [[0; WHEELS]; PLAYERS],
            hry: [[[0; DENTS]; WHEELS]; PLAYERS],
            hmagy: [[[0; DENTS]; WHEELS]; PLAYERS],
            hnrx: [[0; WHEELS]; PLAYERS],
            hrx: [[[0; DENTS]; WHEELS]; PLAYERS],
            hmagx: [[[0; DENTS]; WHEELS]; PLAYERS],
            hnrz: [[0; WHEELS]; PLAYERS],
            hrz: [[[0; DENTS]; WHEELS]; PLAYERS],
            hmagz: [[[0; DENTS]; WHEELS]; PLAYERS],
            hmtouch: [[false; DENTS]; PLAYERS],
            cntdest: [0; PLAYERS],
            lastfr: 0,
        }
    }

    /// `Record.reset` (Record.java:223-286) — the loadstage-tail
    /// initialization: the pristine car mess (`starcar` once per run via
    /// `prepit`), the 30 staged contours cloned from the current pool,
    /// the fix/dest timers at −1, and the spark/dent reply rings at −1
    /// (the walker's outer bound stops at slot 4, the original's bound).
    pub fn reset(&mut self, pool: &[crate::conto::Conto], m: &crate::medium::Medium) {
        self.caught = 0;
        self.hcaught = false;
        self.wasted = 0;
        self.whenwasted = 0;
        self.powered = 0;
        for var2 in 0..PLAYERS {
            if self.prepit {
                self.starcar[var2] =
                    Some(crate::conto::Conto::clone_from(&pool[var2], m, 0, 0, 0, 0));
            }
            self.fix[var2] = -1;
            self.dest[var2] = -1;
            self.cntdest[var2] = 0;
        }

        for var2 in 0..CAR_ROWS {
            for var3 in 0..PLAYERS {
                self.car[var2][var3] =
                    Some(crate::conto::Conto::clone_from(&pool[var3], m, 0, 0, 0, 0));
                self.squash[var2][var3] = 0;
            }
        }

        for var2 in 0..4 {
            for var3 in 0..WHEELS {
                for var4 in 0..SPARKS {
                    self.sspark[var2][var3][var4] = -1;
                    self.ns[var2][var3] = 0;
                }
                for var4 in 0..DENTS {
                    self.ry[var2][var3][var4] = -1;
                    self.nry[var2][var3] = 0;
                    self.rx[var2][var3][var4] = -1;
                    self.nrx[var2][var3] = 0;
                    self.rz[var2][var3][var4] = -1;
                    self.nrz[var2][var3] = 0;
                }
            }
        }

        self.prepit = false;
    }

    /// `Record.rec` (Record.java:543-632) — the per-frame snapshot for
    /// one car slot: the staged 50-frame contour shift, the fix/dest
    /// timer ticks with the 230 wasted-replay gate (armed through
    /// `cotchinow`, the `whenwasted` stamps differing for the player's
    /// crash and the opponent's `lastcolido` crash), the 300-frame
    /// pose-ring shift, the wheel spark capture whenever the ContO's
    /// `stg` marks ground contact, and the reply-ring decay.
    pub fn rec(
        &mut self,
        m: &crate::medium::Medium,
        var1: &crate::conto::Conto,
        var2: usize,
        var3: i32,
        var4: i32,
        var5: i32,
    ) {
        if var2 == 0 {
            self.caught = self.caught.wrapping_add(1);
        }

        if self.cntf == 50 {
            for var6 in 0..5 {
                let snapshot = crate::conto::Conto::clone_from(
                    self.car[var6 + 1][var2]
                        .as_ref()
                        .expect("NullPointerException: Record.car history is null before reset (Record.java:552)"),
                    m,
                    0,
                    0,
                    0,
                    0,
                );
                self.car[var6][var2] = Some(snapshot);
                self.squash[var6][var2] = self.squash[var6 + 1][var2];
            }

            self.car[5][var2] = Some(crate::conto::Conto::clone_from(var1, m, 0, 0, 0, 0));
            self.squash[5][var2] = var3;
            self.cntf = 0;
        } else {
            self.cntf = self.cntf.wrapping_add(1);
        }

        self.fix[var2] = self.fix[var2].wrapping_sub(1);
        if var5 != 0 {
            self.dest[var2] = self.dest[var2].wrapping_sub(1);
        }

        if self.dest[var2] == 230 {
            if var2 == 0 {
                self.cotchinow(0, m);
                self.whenwasted = 229;
            } else if var4 != 0 {
                self.cotchinow(var2, m);
                self.whenwasted = 165 + var4;
            }
        }

        for var8 in 0..(RING - 1) {
            self.x[var8][var2] = self.x[var8 + 1][var2];
            self.y[var8][var2] = self.y[var8 + 1][var2];
            self.z[var8][var2] = self.z[var8 + 1][var2];
            self.zy[var8][var2] = self.zy[var8 + 1][var2];
            self.xy[var8][var2] = self.xy[var8 + 1][var2];
            self.xz[var8][var2] = self.xz[var8 + 1][var2];
            self.wxz[var8][var2] = self.wxz[var8 + 1][var2];
            self.wzy[var8][var2] = self.wzy[var8 + 1][var2];
        }

        self.x[299][var2] = var1.x;
        self.y[299][var2] = var1.y;
        self.z[299][var2] = var1.z;
        self.xy[299][var2] = var1.xy;
        self.zy[299][var2] = var1.zy;
        self.xz[299][var2] = var1.xz;
        self.wxz[299][var2] = var1.wxz;
        self.wzy[299][var2] = var1.wzy;
        for var8 in 0..WHEELS {
            if var1.stg[var8] == 1 {
                let ns = self.ns[var2][var8] as usize;
                self.sspark[var2][var8][ns] = 300;
                self.sx[var2][var8][ns] = var1.sx[var8];
                self.sy[var2][var8][ns] = var1.sy[var8];
                self.sz[var2][var8][ns] = var1.sz[var8];
                self.smag[var2][var8][ns] = var1.smag[var8];
                self.scx[var2][var8][ns] = var1.scx[var8];
                self.scz[var2][var8][ns] = var1.scz[var8];
                self.fulls[var2][var8][ns] = var1.fulls[var8];
                // :611-614 — increment, then the exact `== 30` wrap.
                self.ns[var2][var8] += 1;
                if self.ns[var2][var8] == SPARKS as i32 {
                    self.ns[var2][var8] = 0;
                }
            }

            for var7 in 0..SPARKS {
                self.sspark[var2][var8][var7] = self.sspark[var2][var8][var7].wrapping_sub(1);
            }

            for var7 in 0..DENTS {
                self.ry[var2][var8][var7] = self.ry[var2][var8][var7].wrapping_sub(1);
                self.rx[var2][var8][var7] = self.rx[var2][var8][var7].wrapping_sub(1);
                self.rz[var2][var8][var7] = self.rz[var2][var8][var7].wrapping_sub(1);
            }
        }
    }

    /// `Record.recx` (Record.java:633-641) — the x-dent register push
    /// from Madness.regx. The write index is `nry` while the counter
    /// bumped is `nrx`, the original's cross-indexed bug (Record.java:634).
    pub fn recx(&mut self, var1: usize, var2: f32, var3: usize) {
        self.rx[var3][var1][self.nry[var3][var1] as usize] = 300;
        self.magx[var3][var1][self.nry[var3][var1] as usize] = var2 as i32;
        // :636-639 — increment, then the EXACT `== 7` wrap (not a modulo:
        // the original only folds the reachable 7 back to 0).
        self.nrx[var3][var1] += 1;
        if self.nrx[var3][var1] == DENTS as i32 {
            self.nrx[var3][var1] = 0;
        }
    }

    /// `Record.recy` (Record.java:642-651) — the y-dent register push
    /// from Madness.regy; `var3` is Madness's own `mtouch` flag stored
    /// beside the dent (`:645` indexes `[car][slot]`).
    pub fn recy(&mut self, var1: usize, var2: f32, var3: bool, var4: usize) {
        self.ry[var4][var1][self.nry[var4][var1] as usize] = 300;
        self.magy[var4][var1][self.nry[var4][var1] as usize] = var2 as i32;
        self.mtouch[var4][self.nry[var4][var1] as usize] = var3;
        // :646-649 — increment, then the exact `== 7` wrap.
        self.nry[var4][var1] += 1;
        if self.nry[var4][var1] == DENTS as i32 {
            self.nry[var4][var1] = 0;
        }
    }

    /// `Record.recz` (Record.java:841-849) — the z-dent register push
    /// from Madness.regz; same `nry`-index / `nrz`-bump quirk as recx
    /// (`:842`) with the `== 7` wrap at `:844-847`.
    pub fn recz(&mut self, var1: usize, var2: f32, var3: usize) {
        self.rz[var3][var1][self.nry[var3][var1] as usize] = 300;
        self.magz[var3][var1][self.nry[var3][var1] as usize] = var2 as i32;
        self.nrz[var3][var1] += 1;
        if self.nrz[var3][var1] == DENTS as i32 {
            self.nrz[var3][var1] = 0;
        }
    }

    /// `Record.cotchinow` (Record.java:652-738) — the wasted freeze: once
    /// `caught` has grown past 300 recorded frames, copy the current ring
    /// into the `h`-tagged shadow tables and the staged contours into
    /// `starcar`, stamp `wasted` with the slot and raise `hcaught`.
    /// Before 300 frames it does nothing (which also keeps the
    /// constructor's inert `cotchinow(0)` call).
    pub fn cotchinow(&mut self, var1: usize, m: &crate::medium::Medium) {
        if self.caught >= 300 {
            self.wasted = var1 as i32;
            for var2 in 0..PLAYERS {
                self.starcar[var2] = Some(crate::conto::Conto::clone_from(
                    self.car[0][var2].as_ref().expect("cotchinow before reset"),
                    m,
                    0,
                    0,
                    0,
                    0,
                ));
                self.hsquash[var2] = self.squash[0][var2];
                self.hfix[var2] = self.fix[var2];
                self.hdest[var2] = self.dest[var2];
            }

            for var2 in 0..RING {
                for var3 in 0..PLAYERS {
                    self.hx[var2][var3] = self.x[var2][var3];
                    self.hy[var2][var3] = self.y[var2][var3];
                    self.hz[var2][var3] = self.z[var2][var3];
                    self.hxy[var2][var3] = self.xy[var2][var3];
                    self.hzy[var2][var3] = self.zy[var2][var3];
                    self.hxz[var2][var3] = self.xz[var2][var3];
                    self.hwxz[var2][var3] = self.wxz[var2][var3];
                    self.hwzy[var2][var3] = self.wzy[var2][var3];
                }
            }

            for var2 in 0..PLAYERS {
                for var9 in 0..WHEELS {
                    self.hns[var2][var9] = self.ns[var2][var9];
                    for var4 in 0..SPARKS {
                        self.hsspark[var2][var9][var4] = self.sspark[var2][var9][var4];
                        self.hsx[var2][var9][var4] = self.sx[var2][var9][var4];
                        self.hsy[var2][var9][var4] = self.sy[var2][var9][var4];
                        self.hsz[var2][var9][var4] = self.sz[var2][var9][var4];
                        self.hsmag[var2][var9][var4] = self.smag[var2][var9][var4];
                        self.hscx[var2][var9][var4] = self.scx[var2][var9][var4];
                        self.hscz[var2][var9][var4] = self.scz[var2][var9][var4];
                        self.hfulls[var2][var9][var4] = self.fulls[var2][var9][var4];
                    }
                }
            }

            for var2 in 0..PLAYERS {
                for var10 in 0..WHEELS {
                    self.hnry[var2][var10] = self.nry[var2][var10];
                    self.hnrx[var2][var10] = self.nrx[var2][var10];
                    self.hnrz[var2][var10] = self.nrz[var2][var10];
                    for var12 in 0..DENTS {
                        self.hry[var2][var10][var12] = self.ry[var2][var10][var12];
                        self.hmagy[var2][var10][var12] = self.magy[var2][var10][var12];
                        self.hrx[var2][var10][var12] = self.rx[var2][var10][var12];
                        self.hmagx[var2][var10][var12] = self.magx[var2][var10][var12];
                        self.hrz[var2][var10][var12] = self.rz[var2][var10][var12];
                        self.hmagz[var2][var10][var12] = self.magz[var2][var10][var12];
                    }
                }
            }

            for var2 in 0..PLAYERS {
                for var11 in 0..DENTS {
                    self.hmtouch[var2][var11] = self.mtouch[var2][var11];
                }
            }

            self.hcaught = true;
        }
    }

    /// `play` (Record.java:471-541) — the race-end replay painter the
    /// GameSparker `fase == -1` branch drives per frame (`var22`): the
    /// pool contour is re-posed from ring row `var4`, the destruct ramp
    /// replays through the `dest` timer (`embos` 13 arm on the overdue
    /// frame-0 + the 7-count `embos = 1` brew), sparks lift the wheel
    /// smoke state, and the dent-reply rings re-run the recorded
    /// `regx`/`regy`/`regz` pushes.
    pub fn play(
        &mut self,
        var1: &mut Conto,
        var2: &Madness,
        var3: usize,
        var4: i32,
        m: &mut Medium,
    ) {
        var1.x = self.x[var4 as usize][var3];
        var1.y = self.y[var4 as usize][var3];
        var1.z = self.z[var4 as usize][var3];
        var1.zy = self.zy[var4 as usize][var3];
        var1.xy = self.xy[var4 as usize][var3];
        var1.xz = self.xz[var4 as usize][var3];
        var1.wxz = self.wxz[var4 as usize][var3];
        var1.wzy = self.wzy[var4 as usize][var3];
        if var4 == 0 {
            self.cntdest[var3] = 0;
        }

        if self.dest[var3] == var4 {
            self.cntdest[var3] = 7;
        }

        if var4 == 0 && self.dest[var3] < -1 {
            for var5 in 0..var1.planes.len() {
                if var1.planes[var5].wz == 0 {
                    var1.planes[var5].embos = 13;
                }
            }
        }

        if self.cntdest[var3] != 0 {
            for var7 in 0..var1.planes.len() {
                if var1.planes[var7].wz == 0 {
                    var1.planes[var7].embos = 1;
                }
            }

            self.cntdest[var3] -= 1;
        }

        for var8 in 0..WHEELS {
            for var6 in 0..SPARKS {
                if self.sspark[var3][var8][var6] == var4 {
                    var1.stg[var8] = 1;
                    var1.dov[var8] = -1;
                    var1.sx[var8] = self.sx[var3][var8][var6];
                    var1.sy[var8] = self.sy[var3][var8][var6];
                    var1.sz[var8] = self.sz[var3][var8][var6];
                    var1.smag[var8] = self.smag[var3][var8][var6];
                    var1.scx[var8] = self.scx[var3][var8][var6];
                    var1.scz[var8] = self.scz[var3][var8][var6];
                    var1.fulls[var8] = self.fulls[var3][var8][var6];
                }
            }

            for var6 in 0..DENTS {
                if self.ry[var3][var8][var6] == var4 {
                    self.regy(
                        var8,
                        self.magy[var3][var8][var6] as f32,
                        self.mtouch[var3][var6],
                        var1,
                        var2,
                        m,
                    );
                }

                if self.rx[var3][var8][var6] == var4 {
                    self.regx(var8, self.magx[var3][var8][var6] as f32, var1, var2, m);
                }

                if self.rz[var3][var8][var6] == var4 {
                    self.regz(var8, self.magz[var3][var8][var6] as f32, var1, var2, m);
                }
            }
        }
    }

    /// `playh` (Record.java:287-367) — the wasted-replay twin over the
    /// `h`-tagged shadow tables `cotchinow` froze, driven by the
    /// GameSparker `fase == -3` branch. The dent rows add the
    /// `lastfr` alternation: the first visit of a frame re-runs the
    /// full pushes, a same-frame revisit (the double `var24` sweep of
    /// the wasted replay) downgrades to the chip-only variants.
    pub fn playh(
        &mut self,
        var1: &mut Conto,
        var2: &Madness,
        var3: usize,
        var4: i32,
        m: &mut Medium,
    ) {
        var1.x = self.hx[var4 as usize][var3];
        var1.y = self.hy[var4 as usize][var3];
        var1.z = self.hz[var4 as usize][var3];
        var1.zy = self.hzy[var4 as usize][var3];
        var1.xy = self.hxy[var4 as usize][var3];
        var1.xz = self.hxz[var4 as usize][var3];
        var1.wxz = self.hwxz[var4 as usize][var3];
        var1.wzy = self.hwzy[var4 as usize][var3];
        if var4 == 0 {
            self.cntdest[var3] = 0;
        }

        if self.hdest[var3] == var4 {
            self.cntdest[var3] = 7;
        }

        if var4 == 0 && self.hdest[var3] < -1 {
            for var5 in 0..var1.planes.len() {
                if var1.planes[var5].wz == 0 {
                    var1.planes[var5].embos = 13;
                }
            }
        }

        if self.cntdest[var3] != 0 {
            for var7 in 0..var1.planes.len() {
                if var1.planes[var7].wz == 0 {
                    var1.planes[var7].embos = 1;
                }
            }

            self.cntdest[var3] -= 1;
        }

        for var8 in 0..WHEELS {
            for var6 in 0..SPARKS {
                if self.hsspark[var3][var8][var6] == var4 {
                    var1.stg[var8] = 1;
                    var1.dov[var8] = -1;
                    var1.sx[var8] = self.hsx[var3][var8][var6];
                    var1.sy[var8] = self.hsy[var3][var8][var6];
                    var1.sz[var8] = self.hsz[var3][var8][var6];
                    var1.smag[var8] = self.hsmag[var3][var8][var6];
                    var1.scx[var8] = self.hscx[var3][var8][var6];
                    var1.scz[var8] = self.hscz[var3][var8][var6];
                    var1.fulls[var8] = self.hfulls[var3][var8][var6];
                }
            }

            for var6 in 0..DENTS {
                if self.hry[var3][var8][var6] == var4 && self.lastfr != var4 {
                    self.regy(
                        var8,
                        self.hmagy[var3][var8][var6] as f32,
                        self.hmtouch[var3][var6],
                        var1,
                        var2,
                        m,
                    );
                }

                if self.hrx[var3][var8][var6] == var4 {
                    if self.lastfr != var4 {
                        self.regx(var8, self.hmagx[var3][var8][var6] as f32, var1, var2, m);
                    } else {
                        self.chipx(var8, self.hmagx[var3][var8][var6] as f32, var1, var2, m);
                    }
                }

                if self.hrz[var3][var8][var6] == var4 {
                    if self.lastfr != var4 {
                        self.regz(var8, self.hmagz[var3][var8][var6] as f32, var1, var2, m);
                    } else {
                        self.chipz(var8, self.hmagz[var3][var8][var6] as f32, var1, var2, m);
                    }
                }
            }
        }

        self.lastfr = var4;
    }

    /// `regy` (Record.java:78-221) — the replay copy of Madness.regy's
    /// vertical dent: the `var2 > 100` positive gate, the folded
    /// zy/xy one-axis probes (`sin` walks on the folded angles), the
    /// per-vertex dents with the Mtouch-flag door instead of the live
    /// `this.mtouch`, and the both-axes-tilted squash pass
    /// accumulating into the staged `squash[0][im]` table. The
    /// live-only extras are gone: no save-back, no hitmag, no `nbsq`
    /// bookkeeping, and `bfase` gains the signed float in both passes.
    pub fn regy(
        &mut self,
        var1: usize,
        mut var2: f32,
        var3: bool,
        var4: &mut Conto,
        var5: &Madness,
        m: &mut Medium,
    ) {
        if var2 > 100.0 {
            var2 -= 100.0;
            let mut var6 = 0i32;
            let mut var7 = 0i32;
            let mut var8 = var4.zy;
            let mut var9 = var4.xy;
            while var8 < 360 {
                var8 = var8.wrapping_add(360);
            }
            while var8 > 360 {
                var8 = var8.wrapping_sub(360);
            }
            if var8 < 210 && var8 > 150 {
                var6 = -1;
            }
            if var8 > 330 || var8 < 30 {
                var6 = 1;
            }
            while var9 < 360 {
                var9 = var9.wrapping_add(360);
            }
            while var9 > 360 {
                var9 = var9.wrapping_sub(360);
            }
            if var9 < 210 && var9 > 150 {
                var7 = -1;
            }
            if var9 > 330 || var9 < 30 {
                var7 = 1;
            }

            if var7.wrapping_mul(var6) == 0 || var3 {
                for var10 in 0..var4.planes.len() {
                    let mut var11 = 0.0f32;
                    for var12 in 0..var4.planes[var10].ox.len() {
                        if var4.planes[var10].wz == 0
                            && Self::py(
                                var4.keyx[var1],
                                var4.planes[var10].ox[var12],
                                var4.keyz[var1],
                                var4.planes[var10].oz[var12],
                            ) < CLRAD[var5.cn]
                        {
                            var11 = var2 / 20.0 * m.random();
                            var4.planes[var10].oz[var12] =
                                (var4.planes[var10].oz[var12] as f32 + var11 * m.sin(var8)) as i32;
                            var4.planes[var10].ox[var12] =
                                (var4.planes[var10].ox[var12] as f32 - var11 * m.sin(var9)) as i32;
                        }
                    }

                    if var11 != 0.0 {
                        Self::scorch_panel(var10, var11, true, var4);
                    }
                }
            }

            if var7.wrapping_mul(var6) == -1 {
                let mut var16 = 0i32;
                let mut var17 = 1i32;
                for var19 in 0..var4.planes.len() {
                    let mut var13 = 0.0f32;
                    for var14 in 0..var4.planes[var19].ox.len() {
                        if var4.planes[var19].wz == 0 {
                            var13 = var2 / 15.0 * m.random();
                            if ((var4.planes[var19].oy[var14]
                                - FLIPY[var5.cn]
                                - self.squash[0][var5.im])
                                .wrapping_abs()
                                < MSQUASH[var5.cn] * 3
                                || var4.planes[var19].oy[var14]
                                    < FLIPY[var5.cn] + self.squash[0][var5.im])
                                && self.squash[0][var5.im] < MSQUASH[var5.cn]
                            {
                                var4.planes[var19].oy[var14] =
                                    (var4.planes[var19].oy[var14] as f32 + var13) as i32;
                                var16 = (var16 as f32 + var13) as i32;
                                var17 += 1;
                            }
                        }
                    }

                    if var4.planes[var19].glass {
                        var4.planes[var19].gr -= 5;
                    } else if var13 != 0.0 {
                        var4.planes[var19].bfase = (var4.planes[var19].bfase as f32 + var13) as i32;
                    }

                    if var13.abs() >= 1.0 {
                        var4.planes[var19].chip = 1;
                        var4.planes[var19].ctmag = var13;
                    }
                }

                self.squash[0][var5.im] += var16 / var17;
            }
        }
    }

    /// `regx` (Record.java:766-839) — the replay copy of Madness.regx's
    /// x-axis dent, minus the `DAMMULT`/save-back/`hitmag` extras. The
    /// `bfase` scorch gains the absolute push here (regy keeps the
    /// signed value, the original's split kept in both classes).
    pub fn regx(
        &mut self,
        var1: usize,
        mut var2: f32,
        var3: &mut Conto,
        var4: &Madness,
        m: &mut Medium,
    ) {
        if var2.abs() > 100.0 {
            if var2 > 100.0 {
                var2 -= 100.0;
            }

            if var2 < -100.0 {
                var2 += 100.0;
            }

            for var5 in 0..var3.planes.len() {
                let mut var6 = 0.0f32;
                for var7 in 0..var3.planes[var5].ox.len() {
                    if var3.planes[var5].wz == 0
                        && Self::py(
                            var3.keyx[var1],
                            var3.planes[var5].ox[var7],
                            var3.keyz[var1],
                            var3.planes[var5].oz[var7],
                        ) < CLRAD[var4.cn]
                    {
                        var6 = var2 / 20.0 * m.random();
                        var3.planes[var5].oz[var7] = (var3.planes[var5].oz[var7] as f32
                            - var6 * m.sin(var3.xz) * m.cos(var3.zy))
                            as i32;
                        var3.planes[var5].ox[var7] = (var3.planes[var5].ox[var7] as f32
                            + var6 * m.cos(var3.xz) * m.cos(var3.xy))
                            as i32;
                    }
                }

                if var6 != 0.0 {
                    Self::scorch_panel(var5, var6, false, var3);
                }
            }
        }
    }

    /// `regz` (Record.java:396-469) — the replay copy of Madness.regz's
    /// z-axis wall dent, minus the live-only extras; same `abs` scorch
    /// as regx.
    pub fn regz(
        &mut self,
        var1: usize,
        mut var2: f32,
        var3: &mut Conto,
        var4: &Madness,
        m: &mut Medium,
    ) {
        if var2.abs() > 100.0 {
            if var2 > 100.0 {
                var2 -= 100.0;
            }

            if var2 < -100.0 {
                var2 += 100.0;
            }

            for var5 in 0..var3.planes.len() {
                let mut var6 = 0.0f32;
                for var7 in 0..var3.planes[var5].ox.len() {
                    if var3.planes[var5].wz == 0
                        && Self::py(
                            var3.keyx[var1],
                            var3.planes[var5].ox[var7],
                            var3.keyz[var1],
                            var3.planes[var5].oz[var7],
                        ) < CLRAD[var4.cn]
                    {
                        var6 = var2 / 20.0 * m.random();
                        var3.planes[var5].oz[var7] = (var3.planes[var5].oz[var7] as f32
                            + var6 * m.cos(var3.xz) * m.cos(var3.zy))
                            as i32;
                        var3.planes[var5].ox[var7] = (var3.planes[var5].ox[var7] as f32
                            + var6 * m.sin(var3.xz) * m.cos(var3.xy))
                            as i32;
                    }
                }

                if var6 != 0.0 {
                    Self::scorch_panel(var5, var6, false, var3);
                }
            }
        }
    }

    /// `chipx` (Record.java:739-766) — the chip-only downgrade the
    /// `playh` `lastfr` alternation picks for same-frame revisits: no
    /// vertex can move, the plane keeps only the paint break (`chip` +
    /// `ctmag`). `chipz` (369-394) is the z twin.
    pub fn chipx(
        &mut self,
        var1: usize,
        mut var2: f32,
        var3: &mut Conto,
        var4: &Madness,
        m: &mut Medium,
    ) {
        if var2.abs() > 100.0 {
            if var2 > 100.0 {
                var2 -= 100.0;
            }

            if var2 < -100.0 {
                var2 += 100.0;
            }

            for var5 in 0..var3.planes.len() {
                let mut var6 = 0.0f32;
                for var7 in 0..var3.planes[var5].ox.len() {
                    if var3.planes[var5].wz == 0
                        && Self::py(
                            var3.keyx[var1],
                            var3.planes[var5].ox[var7],
                            var3.keyz[var1],
                            var3.planes[var5].oz[var7],
                        ) < CLRAD[var4.cn]
                    {
                        var6 = var2 / 20.0 * m.random();
                    }
                }

                if var6 != 0.0 && var6.abs() >= 1.0 {
                    var3.planes[var5].chip = 1;
                    var3.planes[var5].ctmag = var6;
                }
            }
        }
    }

    pub fn chipz(
        &mut self,
        var1: usize,
        mut var2: f32,
        var3: &mut Conto,
        var4: &Madness,
        m: &mut Medium,
    ) {
        if var2.abs() > 100.0 {
            if var2 > 100.0 {
                var2 -= 100.0;
            }

            if var2 < -100.0 {
                var2 += 100.0;
            }

            for var5 in 0..var3.planes.len() {
                let mut var6 = 0.0f32;
                for var7 in 0..var3.planes[var5].ox.len() {
                    if var3.planes[var5].wz == 0
                        && Self::py(
                            var3.keyx[var1],
                            var3.planes[var5].ox[var7],
                            var3.keyz[var1],
                            var3.planes[var5].oz[var7],
                        ) < CLRAD[var4.cn]
                    {
                        var6 = var2 / 20.0 * m.random();
                    }
                }

                if var6 != 0.0 && var6.abs() >= 1.0 {
                    var3.planes[var5].chip = 1;
                    var3.planes[var5].ctmag = var6;
                }
            }
        }
    }

    /// The scorch/chip plane pass shared by the three replay pushes —
    /// byte-for-byte the Madness helper's body (`chip` arm, the bfase
    /// HSB ladder with the signed/absolute `bfase` gain switch, the
    /// getHSBColor writeback and the glass gr walk). The dead
    /// `new Color(...)` statement before the writeback is dropped like
    /// in the Madness port.
    fn scorch_panel(var4: usize, var5: f32, signed_bfase: bool, o: &mut Conto) {
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
            p.c = crate::color::hsb_to_rgb(p.hsb[0], p.hsb[1], p.hsb[2]);
        }

        if p.glass {
            p.gr = (p.gr as f64 - ((var5 as f64) * 1.5).abs()) as i32;
        }
    }

    /// `py` (Record.java:850-852) — Record's own wheel-distance probe;
    /// Java int wrap kept. Java declares it `public`, so the mirror pins it
    /// from outside the class (`TRecord.pyGroup`, TRecord.java:1585-1604).
    pub(crate) fn py(var1: i32, var2: i32, var3: i32, var4: i32) -> i32 {
        (var1.wrapping_sub(var2))
            .wrapping_mul(var1.wrapping_sub(var2))
            .wrapping_add((var3.wrapping_sub(var4)).wrapping_mul(var3.wrapping_sub(var4)))
    }
}

#[cfg(test)]
mod tests {
    /// A Medium with a private, seeded Java stream: the replay's dent
    /// rolls must be reproducible without sharing the process-global
    /// `Math.random()` slot (C3b: `Medium`'s default now reads that slot,
    /// so a unit test that pairs a target with a probe needs two
    /// independent seeded streams — the `RngSource::Java` seam).
    fn seeded_medium() -> Medium {
        let mut m = Medium::new();
        m.rng = Some(crate::random::RngSource::Java(
            crate::random::JavaRandom::new(42),
        ));
        m
    }

    use super::*;
    use crate::conto::{Base, Conto};
    use crate::medium::Medium;
    use crate::trackers::Trackers;

    fn pool_of(keyx: i32) -> Vec<Conto> {
        let m = Medium::new();
        let mut t = Trackers::new();
        let base = Base {
            keyx: [keyx, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        let mut pool = Vec::new();
        for _ in 0..PLAYERS {
            pool.push(Conto::place(&base, &m, &mut t, 0, 0, 0, 0));
        }
        pool
    }

    #[test]
    fn rec_shifts_the_ring_and_primes_via_reset() {
        let m = Medium::new();
        let pool = pool_of(10);
        let mut rpd = Record::new();
        rpd.reset(&pool, &m);
        // reset primed the staged contours and timers.
        assert!(rpd.car[0][2].is_some() && rpd.car[5][2].is_some());
        assert_eq!(rpd.fix[3], -1);
        assert_eq!(rpd.dest[4], -1);
        assert!(!rpd.prepit, "the fingerprint snapshot happens once");
        // starcar carries the first pool's fingerprint (keyx 10).
        let sc0 = rpd.starcar[0].as_ref().unwrap();
        assert_eq!(sc0.keyx[0], 10);

        let mut var1 = Conto::clone_from(&pool[0], &m, 0, 0, 0, 0);
        var1.x = 5000;
        var1.xy = 33;
        var1.wzy = 12;
        rpd.rec(&m, &var1, 0, 123, 0, 0);
        // slot 0's snapshot marked slot 5 of the staged table.
        assert_eq!(rpd.squash[5][0], 123);
        assert_eq!(rpd.x[299][0], 5000);
        assert_eq!(rpd.xy[299][0], 33);
        assert_eq!(rpd.wzy[299][0], 12);
        assert_eq!(rpd.caught, 1, "the caught counter rides slot 0");
        assert_eq!(rpd.cntf, 0, "cntf 50 primes the staged shift");

        let mut var1b = Conto::clone_from(&var1, &m, 0, 0, 0, 0);
        var1b.x = 6000;
        rpd.rec(&m, &var1b, 0, 5, 0, 0);
        // The shift moved the first frame down one slot…
        assert_eq!(rpd.x[298][0], 5000);
        assert_eq!(rpd.x[299][0], 6000);
        // …and cntf 0 counts up instead of shifting again.
        assert_eq!(rpd.cntf, 1);
        assert_eq!(rpd.squash[5][0], 123, "no staged shift this frame");
    }

    #[test]
    fn recx_and_recz_index_nry_while_bumping_their_own_counters() {
        // The original's cross-indexed bug (Record.java:633/841): rx/rz
        // always write the nry slot and overwrite each frame until recy
        // moves it.
        let mut rpd = Record::new();
        rpd.recx(0, 150.7, 2);
        assert_eq!(rpd.rx[2][0][0], 300);
        assert_eq!(rpd.magx[2][0][0], 150, "(int) of 150.7 truncates");
        assert_eq!(rpd.nrx[2][0], 1);
        assert_eq!(rpd.nry[2][0], 0, "recx never moves nry");
        rpd.recx(0, 90.0, 2);
        assert_eq!(rpd.rx[2][0][0], 300, "the nry slot again");
        assert_eq!(rpd.magx[2][0][0], 90);
        assert_eq!(rpd.nrx[2][0], 2);
        assert_eq!(rpd.rx[2][0][1], 0);

        let mut rpd2 = Record::new();
        rpd2.recz(1, -200.5, 4);
        assert_eq!(rpd2.rz[4][1][0], 300);
        assert_eq!(rpd2.magz[4][1][0], -200);
        assert_eq!(rpd2.nrz[4][1], 1);
    }

    #[test]
    fn cotchinow_requires_300_caught_and_freezes_the_h_tables() {
        let m = Medium::new();
        let pool = pool_of(10);
        let mut rpd = Record::new();
        rpd.reset(&pool, &m);
        rpd.cotchinow(1, &m);
        assert!(!rpd.hcaught, "inert before 300 recorded frames");

        let mut var1 = Conto::clone_from(&pool[1], &m, 0, 0, 0, 0);
        var1.x = 77;
        for _ in 0..300 {
            rpd.rec(&m, &var1, 0, 0, 0, 0);
        }
        assert_eq!(rpd.caught, 300);
        rpd.cotchinow(1, &m);
        assert!(rpd.hcaught);
        assert_eq!(rpd.wasted, 1);
        assert_eq!(rpd.hx[299][0], 77, "the h ring mirrors the x ring");
        assert_eq!(rpd.hfix[2], -1);
        assert_eq!(rpd.hdest[2], -1);
        assert!(rpd.starcar[1].is_some(), "cotchinow refreshes starcar");
    }

    #[test]
    fn dest_timer_reaches_230_and_arms_the_wasted_replay() {
        let m = Medium::new();
        let pool = pool_of(10);
        let mut rpd = Record::new();
        rpd.reset(&pool, &m);
        // 300 frames of the player slot raising `caught`.
        let var1 = Conto::clone_from(&pool[0], &m, 0, 0, 0, 0);
        for _ in 0..300 {
            rpd.rec(&m, &var1, 0, 0, 0, 0);
        }
        // The damage stamp (drive's rpd.dest[im] = 300) then a crash with
        // lastcolido 70: the 70 decrements (one per kept cntdest) arm the
        // replay at dest 230 and stamp whenwasted 165 + 70.
        rpd.dest[1] = 300;
        let var1b = Conto::clone_from(&pool[1], &m, 0, 0, 0, 0);
        for _ in 0..70 {
            rpd.rec(&m, &var1b, 1, 0, 70, 1);
        }
        assert_eq!(rpd.dest[1], 230);
        assert!(rpd.hcaught);
        assert_eq!(rpd.wasted, 1);
        assert_eq!(rpd.whenwasted, 235);
    }

    #[test]
    fn reset_leaves_starcar_to_the_first_pass() {
        let m = Medium::new();
        let pool_a = pool_of(10);
        let pool_b = pool_of(20);
        let mut rpd = Record::new();
        rpd.reset(&pool_a, &m);
        assert_eq!(rpd.starcar[0].as_ref().unwrap().keyx[0], 10);
        rpd.reset(&pool_b, &m);
        // prepit flipped off: the second reset primes car/dest but keeps
        // the first pool's fingerprint in starcar.
        assert_eq!(rpd.starcar[0].as_ref().unwrap().keyx[0], 10);
        assert_eq!(rpd.car[0][0].as_ref().unwrap().keyx[0], 20);
    }

    /// One body plane wired for the dent replies: vertex 0 rides the
    /// wheel-0 key anchor (py(10,0,10,0) = 200 < CLRAD[0] 3300), the two
    /// side vertices sit far out (py 16200) so only one vertex matches
    /// — one xshift roll per painter call, probe-predictable.
    fn pool_with_plane() -> Vec<Conto> {
        let m = Medium::new();
        let mut t = Trackers::new();
        let mut base = Base {
            keyx: [10, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        use crate::plane::Plane;
        base.planes.push(Plane::new(
            &m,
            &[0, 100, 100],
            &[0, 100, 100],
            &[-100, -100, -100],
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
        let mut pool = Vec::new();
        for _ in 0..PLAYERS {
            pool.push(Conto::place(&base, &m, &mut t, 0, 0, 0, 0));
        }
        pool
    }

    fn rig() -> (Vec<Conto>, Madness) {
        (pool_with_plane(), Madness::new(2))
    }

    #[test]
    fn play_reposes_the_pool_and_replays_sparks() {
        let m = Medium::new();
        let pool = pool_of(10);
        let mut rpd = Record::new();
        rpd.x[40][2] = -6013;
        rpd.y[40][2] = 250;
        rpd.z[40][2] = 17000;
        rpd.zy[40][2] = 40;
        rpd.xy[40][2] = 3;
        rpd.xz[40][2] = 210;
        rpd.wxz[40][2] = 30;
        rpd.wzy[40][2] = 33;
        rpd.sspark[2][1][3] = 40;
        rpd.sx[2][1][3] = 11;
        rpd.sy[2][1][3] = 22;
        rpd.sz[2][1][3] = 33;
        rpd.smag[2][1][3] = 1.5;
        rpd.scx[2][1][3] = 4;
        rpd.scz[2][1][3] = 9;
        rpd.fulls[2][1][3] = true;

        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        let mad = Madness::new(2);
        let mut m = m;
        rpd.play(&mut o, &mad, 2, 40, &mut m);
        assert_eq!(o.x, -6013);
        assert_eq!(o.y, 250);
        assert_eq!(o.z, 17000);
        assert_eq!(o.zy, 40);
        assert_eq!(o.xy, 3);
        assert_eq!(o.xz, 210);
        assert_eq!(o.wxz, 30);
        assert_eq!(o.wzy, 33);
        // The spark mimic: wheel 1 lifts the off-frame smoke state.
        assert_eq!(o.stg[1], 1);
        assert_eq!(o.dov[1], -1);
        assert_eq!(o.sx[1], 11);
        assert_eq!(o.sy[1], 22);
        assert_eq!(o.sz[1], 33);
        assert_eq!(o.smag[1], 1.5);
        assert_eq!(o.scx[1], 4);
        assert_eq!(o.scz[1], 9);
        assert!(o.fulls[1]);
        assert_eq!(o.stg[0], 0, "wheel 0 has no spark row");
        // Fresh record: dest 0 != 40, cntdest 0 — nothing else ran.
        assert_eq!(rpd.cntdest[2], 0);
        assert_eq!(rpd.dest[2], 0);
    }

    #[test]
    fn play_replays_the_dest_embos_ramp() {
        let m = Medium::new();
        let (pool, mad) = rig();
        let mut rpd = Record::new();
        // reset zeroes dest only to -1; the rec decrements push it below,
        // emulating a finished destruct. The overdue frame-0 arm:
        rpd.dest[2] = -5;
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        let mut m = m;
        rpd.play(&mut o, &mad, 2, 0, &mut m);
        assert_eq!(o.planes[0].embos, 13);
        assert_eq!(rpd.cntdest[2], 0);

        // A destruct flag at frame 25 arms the 7-count brew with embos 1.
        let mut rpd = Record::new();
        rpd.dest[2] = 25;
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        rpd.play(&mut o, &mad, 2, 25, &mut m);
        assert_eq!(o.planes[0].embos, 1);
        assert_eq!(rpd.cntdest[2], 6);
        // The brew decays on later frames while cntdest lasts.
        rpd.play(&mut o, &mad, 2, 26, &mut m);
        assert_eq!(o.planes[0].embos, 1);
        assert_eq!(rpd.cntdest[2], 5);
    }

    #[test]
    fn play_reruns_recorded_regy_through_the_positive_gate() {
        let m = seeded_medium();
        let (pool, mad) = rig();
        let mut rpd = Record::new();
        // Wheel 0 row: a vertical dent at frame 130, mag 220 -> 120. The
        // replay pose drives the dent angles from the ring itself.
        rpd.zy[130][2] = 90;
        rpd.xy[130][2] = 250;
        rpd.ry[2][0][2] = 130;
        rpd.magy[2][0][2] = 220;
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        let mut m = m;
        rpd.play(&mut o, &mad, 2, 130, &mut m);
        // Fold: 90 -> var6 0, 250 -> var7 0 => product 0, the dent runs;
        // var11 = 120/20 * rand, one matching vertex.
        let mut p = seeded_medium();
        let var11 = 120.0f32 / 20.0 * p.random();
        let exp_oz = (0.0f32 + var11 * m.sin(90)) as i32;
        let exp_ox = (0.0f32 - var11 * m.sin(250)) as i32;
        assert_eq!(o.planes[0].oz[0], exp_oz);
        assert_eq!(o.planes[0].ox[0], exp_ox);
        // The signed scorch: chip arm, bfase gains var11, HSB writeback.
        assert_eq!(o.planes[0].bfase, (5.0f32 + var11) as i32);
        assert_eq!(
            o.planes[0].c,
            crate::color::hsb_to_rgb(o.planes[0].hsb[0], o.planes[0].hsb[1], o.planes[0].hsb[2])
        );
        if var11.abs() >= 1.0 {
            assert_eq!(o.planes[0].chip, 1);
            assert_eq!(o.planes[0].ctmag, var11);
        }
        // Side vertices sit out of every radius: untouched.
        assert_eq!(o.planes[0].oz[1], 100);
    }

    #[test]
    fn play_reruns_recorded_regx_with_the_negative_shift() {
        let m = seeded_medium();
        let (pool, mad) = rig();
        let mut rpd = Record::new();
        // Negative mag: the abs gate passes, the +100 shift lands -160.
        rpd.rx[2][1][0] = 130;
        rpd.magx[2][1][0] = -260;
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        let mut m = m;
        rpd.play(&mut o, &mad, 2, 130, &mut m);
        let mut p = seeded_medium();
        let var6 = -160.0f32 / 20.0 * p.random();
        // o.xz = 0: oz -= var6 * sin(0) * cos(0) stays, ox += var6 exactly.
        assert_eq!(o.planes[0].oz[0], 0);
        assert_eq!(
            o.planes[0].ox[0],
            (0.0f32 + var6 * m.cos(0) * m.cos(0)) as i32
        );
        // The absolute scorch gain.
        assert_eq!(o.planes[0].bfase, (5.0f32 + var6.abs()) as i32);
    }

    #[test]
    fn play_reruns_recorded_regz() {
        let m = seeded_medium();
        let (pool, mad) = rig();
        let mut rpd = Record::new();
        rpd.rz[2][2][1] = 130;
        rpd.magz[2][2][1] = 260;
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        let mut m = m;
        rpd.play(&mut o, &mad, 2, 130, &mut m);
        let mut p = seeded_medium();
        let var6 = 160.0f32 / 20.0 * p.random();
        // cos(0) * cos(0) = 1: oz gains var6 exactly, ox untouched.
        assert_eq!(
            o.planes[0].oz[0],
            (0.0f32 + var6 * m.cos(0) * m.cos(0)) as i32
        );
        assert_eq!(o.planes[0].ox[0], 0);
        assert_eq!(o.planes[0].bfase, (5.0f32 + var6.abs()) as i32);
    }

    #[test]
    fn regy_stays_in_its_positive_gate() {
        let m = Medium::new();
        let (pool, mad) = rig();
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        o.zy = 180;
        o.xy = 0;
        let mut m = m;
        let before = o.planes[0].oz[0];
        // 90 <= 100: nothing at all.
        Record::new().regy(0, 90.0, false, &mut o, &mad, &mut m);
        assert_eq!(o.planes[0].oz[0], before);
        assert_eq!(o.planes[0].bfase, 5);
        // Negative magy: the replay gate is positive-only — no-op.
        Record::new().regy(0, -220.0, false, &mut o, &mad, &mut m);
        assert_eq!(o.planes[0].oz[0], before);
        assert_eq!(o.planes[0].bfase, 5);
    }

    #[test]
    fn regy_squash_pass_accumulates_into_the_staged_table() {
        let m = seeded_medium();
        let (pool, mad) = rig();
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        // 180 folded -> var6 -1; 0 folded -> var7 +1: product -1, the
        // squash pass runs (and the mtouch flag false keeps the dent out).
        o.zy = 180;
        o.xy = 0;
        let mut rpd = Record::new();
        let mut m = m;
        rpd.regy(0, 160.0, false, &mut o, &mad, &mut m);
        // Three wz==0 vertices at oy -100: each passes the oy gate
        // (-100 < FLIPY[0] -50), rolls 60/15 * rand, pushes.
        let mut p = seeded_medium();
        let mut var16 = 0i32;
        let mut var17 = 1i32;
        let mut exp_oy = [-100i32; 3];
        let mut last_var13 = 0.0f32;
        for k in 0..3 {
            let var13 = 60.0f32 / 15.0 * p.random();
            exp_oy[k] = (-100.0f32 + var13) as i32;
            var16 = (var16 as f32 + var13) as i32;
            var17 += 1;
            last_var13 = var13;
        }
        assert_eq!(o.planes[0].oy[0], exp_oy[0]);
        assert_eq!(o.planes[0].oy[1], exp_oy[1]);
        assert_eq!(o.planes[0].oy[2], exp_oy[2]);
        assert_eq!(var17, 4);
        assert_eq!(rpd.squash[0][2], 0 + var16 / var17);
        // The plane tail repeats the last vertex's push, signed: bfase.
        assert_eq!(o.planes[0].bfase, (5.0f32 + last_var13) as i32);
        if last_var13.abs() >= 1.0 {
            assert_eq!(o.planes[0].chip, 1);
            assert_eq!(o.planes[0].ctmag, last_var13);
        }
        // Not glass: the gr -5 arm never ran (still the plane's 10).
        assert_eq!(o.planes[0].gr, 10);
    }

    #[test]
    fn playh_downgrades_same_frame_dents_to_chip() {
        let m = seeded_medium();
        let (pool, mad) = rig();
        let mut rpd = Record::new();
        rpd.hx[130][2] = 77;
        rpd.hxy[130][2] = 5;
        rpd.hrx[2][1][0] = 130;
        rpd.hmagx[2][1][0] = 900;
        let mut o = Conto::clone_from(&pool[2], &m, 0, 0, 0, 0);
        let mut m = m;
        // First visit of frame 130: lastfr 0 != 130 -> full regx dent.
        rpd.playh(&mut o, &mad, 2, 130, &mut m);
        assert_eq!(o.x, 77, "the h-ring pose re-applies");
        assert_eq!(o.xy, 5);
        let mut p = seeded_medium();
        let var6 = 800.0f32 / 20.0 * p.random();
        // o.xy re-posed to 5: the ox push scales by cos(xz=0) * cos(xy=5).
        let dented = (0.0f32 + var6 * m.cos(0) * m.cos(5)) as i32;
        assert_eq!(o.planes[0].ox[0], dented);
        let after_first = rpd.lastfr;
        assert_eq!(after_first, 130);

        // Same-frame revisit: chipx only — no vertex can move.
        rpd.playh(&mut o, &mad, 2, 130, &mut m);
        assert_eq!(o.planes[0].ox[0], dented, "chipx never pushes vertices");
        let var6b = 800.0f32 / 20.0 * p.random();
        if var6b.abs() >= 1.0 {
            assert_eq!(o.planes[0].chip, 1);
            assert_eq!(o.planes[0].ctmag, var6b);
        }
    }
}
