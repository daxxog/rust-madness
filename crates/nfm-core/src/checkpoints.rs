// FIDELITY: CheckPoints.update() keeps Java CheckPoints.java `for (int var7 = 1; ...)` indexed loops verbatim.
#![allow(clippy::needless_range_loop)]
//! Port of `CheckPoints.java` — the gameplay/track registry built by
//! `loadstage()`: checkpoint (`)p`/`chk`) and fix (`fix`) positions, lap
//! bookkeeping state, and the per-frame `checkstat` placement pass.
//! Fixed capacities like the original (140 entries, 5 players).

use crate::conto::Conto;
use crate::madness::Madness;

pub struct CheckPoints {
    pub x: Vec<i32>,
    pub z: Vec<i32>,
    pub y: Vec<i32>,
    pub typ: Vec<i32>,
    pub pcs: i32,
    pub nsp: i32,
    pub n: i32,
    pub fx: Vec<i32>,
    pub fz: Vec<i32>,
    pub fy: Vec<i32>,
    pub roted: Vec<bool>,
    pub special: Vec<bool>,
    pub fn_: i32,
    pub stage: i32,
    pub nlaps: i32,
    pub name: String,
    pub pos: Vec<i32>,
    pub clear: Vec<i32>,
    pub dested: Vec<i32>,
    pub wasted: i32,
    pub haltall: bool,
    pub opx: Vec<i32>,
    pub opz: Vec<i32>,
    pub onscreen: Vec<i32>,
}

pub const CP_CAP: usize = 140;
pub const PLAYERS: usize = 5;

impl Default for CheckPoints {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckPoints {
    pub fn new() -> CheckPoints {
        CheckPoints {
            x: vec![0; CP_CAP],
            z: vec![0; CP_CAP],
            y: vec![0; CP_CAP],
            typ: vec![0; CP_CAP],
            pcs: 0,
            nsp: 0,
            n: 0,
            fx: vec![0; PLAYERS],
            fz: vec![0; PLAYERS],
            fy: vec![0; PLAYERS],
            roted: vec![false; PLAYERS],
            special: vec![false; PLAYERS],
            fn_: 0,
            stage: 1,
            nlaps: 0,
            name: "hogan rewish".to_string(),
            pos: vec![4; PLAYERS],
            clear: vec![0; PLAYERS],
            dested: vec![0; PLAYERS],
            wasted: 0,
            haltall: false,
            opx: vec![0; PLAYERS],
            opz: vec![0; PLAYERS],
            onscreen: vec![0; PLAYERS],
        }
    }

    /// `CheckPoints.checkstat` (CheckPoints.java:27-81) — per-frame race
    /// placement. Snapshot pass: every slot's `onscreen` (the ContO dist
    /// marker), `opx`/`opz` and `clear` (dested slots clear to −1). The
    /// snapshot and the ranking both run only while !`haltall`. The
    /// pairwise O(n²) ranking adds +1 to the beaten slot of each pair:
    /// on different clears the higher clear wins; on a tie, the slot
    /// nearest its next gate (py² walked from var3's pcleared + 1 over
    /// the gate rows) wins. The wasted count runs ALWAYS and only over
    /// slots 1..4 — the player slot's own destruction is excluded.
    pub fn checkstat(&mut self, mad: &[Madness], pool: &[Conto]) {
        if !self.haltall {
            for var3 in 0..PLAYERS {
                self.pos[var3] = 0;
                self.onscreen[var3] = pool[var3].dist;
                self.opx[var3] = pool[var3].x;
                self.opz[var3] = pool[var3].z;
                if self.dested[var3] == 0 {
                    self.clear[var3] = mad[var3].clear;
                } else {
                    self.clear[var3] = -1;
                }
            }

            for var3 in 0..PLAYERS {
                for var4 in (var3 + 1)..PLAYERS {
                    if self.clear[var3] != self.clear[var4] {
                        if self.clear[var3] < self.clear[var4] {
                            self.pos[var3] += 1;
                        } else {
                            self.pos[var4] += 1;
                        }
                    } else {
                        let mut var5 = mad[var3].pcleared + 1;
                        while self.typ[var5 as usize] <= 0 {
                            var5 = var5.wrapping_add(1);
                            if var5 as usize == self.n as usize {
                                var5 = 0;
                            }
                        }

                        if self.py(
                            pool[var3].x / 100,
                            self.x[var5 as usize] / 100,
                            pool[var3].z / 100,
                            self.z[var5 as usize] / 100,
                        ) > self.py(
                            pool[var4].x / 100,
                            self.x[var5 as usize] / 100,
                            pool[var4].z / 100,
                            self.z[var5 as usize] / 100,
                        ) {
                            self.pos[var3] += 1;
                        } else {
                            self.pos[var4] += 1;
                        }
                    }
                }
            }
        }

        self.wasted = 0;
        for var7 in 1..PLAYERS {
            if mad[var7].dest {
                self.wasted += 1;
            }
        }
    }

    /// `CheckPoints.py` (CheckPoints.java:83-85); int squares wrap in Java
    /// — and so does the subtraction producing them.
    pub fn py(&self, a: i32, b: i32, c: i32, d: i32) -> i32 {
        let d1 = a.wrapping_sub(b);
        let d2 = c.wrapping_sub(d);
        d1.wrapping_mul(d1).wrapping_add(d2.wrapping_mul(d2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conto::Base;
    use crate::medium::Medium;
    use crate::trackers::Trackers;

    fn blank_pool() -> Vec<Conto> {
        let m = Medium::new();
        let mut t = Trackers::new();
        let base = Base {
            keyx: [10, -10, 10, -10],
            keyz: [10, 10, -10, -10],
            ..Default::default()
        };
        let mut pool = Vec::new();
        for _ in 0..PLAYERS {
            pool.push(Conto::place(&base, &m, &mut t, 0, 0, 0, 0));
        }
        pool
    }

    /// Two far-away gates, like the drive rig's list.
    fn cp2() -> CheckPoints {
        let mut cp = CheckPoints::new();
        cp.n = 2;
        cp.nsp = 1;
        cp.typ[0] = 1;
        cp.typ[1] = 1;
        cp.x[0] = 999900;
        cp.x[1] = 999900;
        cp
    }

    #[test]
    fn checkstat_ranks_clears_and_tie_breaks_by_next_gate() {
        let mut mad = Vec::new();
        for i in 0..PLAYERS {
            mad.push(Madness::new(i));
        }
        let pool = blank_pool();
        let mut cp = cp2();
        mad[0].clear = 2;
        // Clear 2 beats everyone's 0; the all-tie pairs break by the next
        // gate py (every slot sits at the same x/z so the later slot of
        // each pair takes the +1).
        cp.checkstat(&mad, &pool);
        assert_eq!(cp.pos, [0, 1, 2, 3, 4]);
        assert_eq!(cp.wasted, 0);
    }

    #[test]
    fn checkstat_dested_clears_minus_one_and_wasted_skips_player() {
        let mut mad = Vec::new();
        for i in 0..PLAYERS {
            mad.push(Madness::new(i));
        }
        let pool = blank_pool();
        let mut cp = cp2();
        cp.dested[2] = 1;
        // Slot 0's own destruction is excluded from the wasted count
        // (CheckPoints.java:74-80 walks var7 = 1..5).
        mad[0].dest = true;
        mad[1].dest = true;
        cp.checkstat(&mad, &pool);
        assert_eq!(cp.clear[2], -1);
        assert_eq!(cp.wasted, 1);
        assert_eq!(cp.pos, [0, 1, 4, 2, 3]);
    }

    #[test]
    fn checkstat_haltall_freezes_the_snapshot() {
        let mut mad = Vec::new();
        for i in 0..PLAYERS {
            mad.push(Madness::new(i));
        }
        let pool = blank_pool();
        let mut cp = cp2();
        cp.haltall = true;
        for k in 0..PLAYERS {
            cp.pos[k] = 9;
            cp.onscreen[k] = 7;
        }
        mad[1].dest = true;
        cp.checkstat(&mad, &pool);
        assert_eq!(cp.pos, [9, 9, 9, 9, 9]);
        assert_eq!(cp.onscreen[0], 7);
        assert_eq!(cp.wasted, 1, "the wasted count runs regardless");
    }
}
