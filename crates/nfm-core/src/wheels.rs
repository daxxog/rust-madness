// FIDELITY: Wheels.java: the indexed `for (int k...)` ground/geometry loop mirrors the original evaluation order.
#![allow(clippy::needless_range_loop)]
//! Port of `Wheels.java` — geometry for one wheel: an octagonal tire face
//! (cap plane, n=8) plus 8 side rectangles between consecutive octagon
//! corners (n=4 each): 9 planes per wheel, appended at the current plane
//! position like the original does (`p[var4]`).
//!
//! Wheel-side colors are gray `[75, 75, 75]`; the cap plane carries
//! `master = 1..wheels` (that's why `ContO.d`'s dust lookup
//! `stg[master-1]` never indexes out of its 4-slot array — only wheel-cap
//! planes have master, ContO.java:606).

use crate::medium::Medium;
use crate::plane::Plane;

pub struct Wheels {
    /// Highest wheel-bottom y seen so far; feeds `ContO.grat`.
    pub ground: i32,
    /// Wheel master counter (per wheel, incremented in `make`).
    pub mast: i32,
}

impl Default for Wheels {
    fn default() -> Self {
        Self::new()
    }
}

impl Wheels {
    pub fn new() -> Wheels {
        Wheels { ground: 0, mast: 0 }
    }

    /// Port of `Wheels.make(Medium, Trackers, Plane[], npl, x, y, z, a, mr, mz)`
    /// (Wheels.java:5-164). Geometry scales `mr`/10 and `mz`/10; `a` is a
    /// flag (11 shifts the spin pivot out in `x`); every plane carries the
    /// literal `disline` 7 like the original constructor calls, regardless
    /// of the model's own `disline(n)` setting. Appends 9 planes.
    #[allow(clippy::too_many_arguments)]
    pub fn make(
        &mut self,
        m: &Medium,
        planes: &mut Vec<Plane>,
        x: i32,
        y: i32,
        z: i32,
        a: i32,
        mr: i32,
        mz: i32,
    ) {
        // var16/var17 — radius scales in x and in (y,z)
        let rx = mr as f32 / 10.0;
        let rz = mz as f32 / 10.0;
        // var15 — non-zero spin pivot offset for the special `a == 11` wheel
        let var15 = if a == 11 {
            (x as f32 + 4.0 * rx) as i32
        } else {
            0
        };
        // var18 — side-plane gr flips for wheels at |x| == 73
        let var18 = if x.abs() == 73 { -1 } else { 1 };

        // var11 — the cap face depth: all 8 corners sit at x - 4*rx (the
        // do-while at Wheels.java:24-26); the tread bands' outboard face is
        // x + 4*rx (var11[2]/[3] at each quad build).
        let oxl = (x as f32 - 4.0 * rx) as i32;
        let ox8: [i32; 8] = [oxl; 8];
        let oxr = (x as f32 + 4.0 * rx) as i32;

        // cap octagon corners: oy = var12 (y ± 5/12 rz), oz = var13 (z ± 5/12)
        let oy8: [i32; 8] = [
            (y as f32 - 12.0 * rz) as i32,
            (y as f32 - 12.0 * rz) as i32,
            (y as f32 - 5.0 * rz) as i32,
            (y as f32 + 5.0 * rz) as i32,
            (y as f32 + 12.0 * rz) as i32,
            (y as f32 + 12.0 * rz) as i32,
            (y as f32 + 5.0 * rz) as i32,
            (y as f32 - 5.0 * rz) as i32,
        ];
        let oz8: [i32; 8] = [
            (z as f32 + 5.0 * rz) as i32,
            (z as f32 - 5.0 * rz) as i32,
            (z as f32 - 12.0 * rz) as i32,
            (z as f32 - 12.0 * rz) as i32,
            (z as f32 - 5.0 * rz) as i32,
            (z as f32 + 5.0 * rz) as i32,
            (z as f32 + 12.0 * rz) as i32,
            (z as f32 + 12.0 * rz) as i32,
        ];

        let gray = [75, 75, 75];

        // cap plane (Plane.java-side args: ox, oz, oy, n, oc, glass, gr,
        // wx, wy, wz, disline, bfase, road) — gr 0, carries the master id
        planes.push(Plane::new(
            m, &ox8, &oz8, &oy8, 8, gray, false, 0, var15, y, z, 7, 0, false,
        ));
        self.mast += 1;
        if let Some(last) = planes.last_mut() {
            last.master = self.mast;
        }

        // 8 tread bands, one per octagon edge. Each band spans the two face
        // depths (oxl at the cap side, oxr outboard), and the winding starts
        // at the even corner: Wheels.java's quad sweep alternates direction
        // — edges 0/2/4/6 run the cap's corner order, 1/3/5/7 run it
        // reversed (Wheels.java:48-161).
        let grs = [var18, var18, var18, -1, -1, -1, var18, var18];
        for k in 0..8 {
            let (e0, e1) = if k % 2 == 0 {
                (k, (k + 1) % 8)
            } else {
                ((k + 1) % 8, k)
            };
            let ox = [oxl, oxl, oxr, oxr];
            let oy = [oy8[e0], oy8[e1], oy8[e1], oy8[e0]];
            let oz = [oz8[e0], oz8[e1], oz8[e1], oz8[e0]];
            planes.push(Plane::new(
                m, &ox, &oz, &oy, 4, gray, false, grs[k], var15, y, z, 7, 0, false,
            ));
            if k == 4 {
                // Wheels.java:118-120 — ground bookkeeping sits between the
                // 5th and 6th tread bands
                let bottom = (y as f32 + 12.0 * rz + 1.0) as i32;
                if self.ground < bottom {
                    self.ground = bottom;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medium::Medium;

    #[test]
    fn make_appends_the_nine_wheel_planes() {
        let m = Medium::new();
        let mut w = Wheels::new();
        let mut planes = Vec::new();
        // rx = rz = 2.0 (mr = mz = 20): all geometry exact
        w.make(&m, &mut planes, 40, -20, 0, 0, 20, 20);
        assert_eq!(planes.len(), 9, "1 cap + 8 tread bands");

        // cap octagon: every corner at x - 4*rx = 32, gr 0, carries master 1
        let cap = &planes[0];
        assert_eq!(cap.ox, vec![32; 8]);
        assert_eq!(cap.oy, vec![-44, -44, -30, -10, 4, 4, -10, -30]);
        assert_eq!(cap.oz, vec![10, -10, -24, -24, -10, 10, 24, 24]);
        assert_eq!(cap.oc, [75, 75, 75]);
        assert_eq!(cap.gr, 0);
        assert_eq!(cap.master, 1);
        assert_eq!((cap.wx, cap.wy, cap.wz), (0, -20, 0));
        assert_eq!(cap.disline, 7);
        assert_eq!(cap.bfase, 0);
        assert!(!cap.road);
        assert!(!cap.glass);

        // tread bands span the two face depths (32 .. 48); winding starts
        // at the even corner — edges 0/2/4/6 forward, 1/3/5/7 reversed
        let span = [32, 32, 48, 48];
        let expected = [
            ([-44, -44, -44, -44], [10, -10, -10, 10]),
            ([-30, -44, -44, -30], [-24, -10, -10, -24]),
            ([-30, -10, -10, -30], [-24, -24, -24, -24]),
            ([4, -10, -10, 4], [-10, -24, -24, -10]),
            ([4, 4, 4, 4], [-10, 10, 10, -10]),
            ([-10, 4, 4, -10], [24, 10, 10, 24]),
            ([-10, -30, -30, -10], [24, 24, 24, 24]),
            ([-44, -30, -30, -44], [10, 24, 24, 10]),
        ];
        let grs = [1, 1, 1, -1, -1, -1, 1, 1];
        for (i, (oy, oz)) in expected.iter().enumerate() {
            let band = &planes[1 + i];
            assert_eq!(band.ox, span, "band {i} ox");
            assert_eq!(band.oy, oy, "band {i} oy");
            assert_eq!(band.oz, oz, "band {i} oz");
            assert_eq!(band.gr, grs[i], "band {i} gr");
            assert_eq!(band.master, 0, "band {i} has no master");
            assert_eq!(band.disline, 7, "band {i} disline");
        }

        // wheel-bottom bookkeeping: y + 12*rz + 1 = 5
        assert_eq!(w.ground, 5);
        assert_eq!(w.mast, 1);
    }

    #[test]
    fn make_flips_gr_and_shifts_the_pivot_on_the_73_wheel() {
        let m = Medium::new();
        let mut w = Wheels::new();
        let mut planes = Vec::new();
        w.make(&m, &mut planes, -73, -20, 0, 11, 20, 20);

        // |x| == 73 flips every side plane's gr; a == 11 moves the spin
        // pivot out to x + 4*rx = -65
        let cap = &planes[0];
        assert_eq!(cap.wx, -65);
        assert_eq!(cap.gr, 0);
        assert_eq!(cap.master, 1);
        for (i, p) in planes[1..9].iter().enumerate() {
            assert_eq!(p.gr, -1, "band {i} gr");
            assert_eq!(p.wx, -65, "band {i} pivot");
        }
        assert_eq!(w.ground, 5);
    }

    #[test]
    fn make_counts_masters_across_wheels() {
        let m = Medium::new();
        let mut w = Wheels::new();
        let mut planes = Vec::new();
        w.make(&m, &mut planes, 40, -20, 0, 0, 20, 20);
        w.make(&m, &mut planes, -73, -28, 500, 0, 20, 20);

        assert_eq!(planes.len(), 18);
        assert_eq!(planes[0].master, 1);
        assert_eq!(planes[9].master, 2);
        assert_eq!(w.mast, 2);

        // ground keeps the highest wheel bottom: wheel 1 = 5,
        // wheel 2 = -28 + 24 + 1 = -3 (stays 5)
        assert_eq!(w.ground, 5);
    }
}
