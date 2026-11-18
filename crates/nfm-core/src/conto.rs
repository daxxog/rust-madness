// FIDELITY: ContO.java placement/electrify: indexed `for (int k...)` loops and the Java placement ctor arity are kept verbatim.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]
//! Port of `ContO.java` — the model container (`Conto`), its `Plane[] p` +
//! `int npl` storage ([`Planes`]), the line-based `.rad` byte constructor
//! ([`Conto::from_bytes`], ContO.java:207-419, including its swallowed
//! exceptions and partial object), the model-backed `Base` and the placement
//! constructor `ContO(ContO, x, y, z, yaw)`, the `d()` render orchestrator,
//! the electric arcs (`electrify`) and the fix flash (`fixit`).

use std::ops::{Index, IndexMut};

use crate::canvas::Canvas;
use crate::checkpoints::CheckPoints;
use crate::color;
use crate::medium::Medium;
use crate::model::Model;
use crate::plane::{Plane, clampc, rot};
use crate::trackers::Trackers;
use crate::wheels::Wheels;

/// `this.p = new Plane[120]` (ContO.java:210) — the byte constructor always
/// allocates exactly 120 slots, whatever the file contains.
pub const PLANE_SLOTS: usize = 120;

/// The Java `Plane[] p` + `int npl` pair, with Java's two shapes:
/// the byte constructor allocates a fixed 120-slot array whose tail stays
/// `null` (`ContO.java:210`), while the copy constructor allocates exactly
/// `npl` slots (`ContO.java:432`). `len()` is `npl` (the live prefix the
/// drawing loops walk), `p_len()` is `p.length`; indexing the live prefix
/// panics like the original dereference (`NullPointerException` for a null
/// slot, `ArrayIndexOutOfBoundsException` past `p.length`).
///
/// The engine never sees a partial parse: the `.rad` loader drops failed
/// models, and every placed contour has `npl` non-null slots. The nullable
/// shape exists for the byte constructor and the mirror's field-level
/// fixtures, exactly like the original.
pub struct Planes {
    slots: Vec<Option<Plane>>,
    npl: usize,
    /// Mirror seam for `ContO.p == null` (the copy-ctor no-catch pin sets
    /// the field to null in Java, `TContO.java:619`); the original then
    /// NPEs at its first read (`ContO.java:438`).
    null_array: bool,
}

impl Planes {
    pub fn empty() -> Planes {
        Planes {
            slots: Vec::new(),
            npl: 0,
            null_array: false,
        }
    }

    /// `new Plane[n]` — n null slots.
    pub fn capacity(n: usize) -> Planes {
        Planes {
            // `Plane` is not Clone; build the null tail by hand.
            slots: std::iter::repeat_with(|| None).take(n).collect(),
            npl: 0,
            null_array: false,
        }
    }

    /// A fully-populated array (the copy constructor's `new Plane[npl]`).
    pub fn from_planes(v: Vec<Plane>) -> Planes {
        let n = v.len();
        Planes {
            slots: v.into_iter().map(Some).collect(),
            npl: n,
            null_array: false,
        }
    }

    /// Java `npl` — the live prefix every `for (i < npl)` loop walks.
    pub fn len(&self) -> usize {
        self.npl
    }

    pub fn is_empty(&self) -> bool {
        self.npl == 0
    }

    /// Java `p.length`.
    pub fn p_len(&self) -> usize {
        self.slots.len()
    }

    /// `ContO.p = null` (test seam; see the field note).
    pub fn set_null(&mut self) {
        self.null_array = true;
        self.slots.clear();
    }

    /// `p[i]` without a dereference: `None` for a null slot or past the end.
    /// Used by the mirror's `Check.eq(c.p[i], null, ...)` pins.
    pub fn slot(&self, i: usize) -> Option<&Plane> {
        self.slots.get(i).and_then(|p| p.as_ref())
    }

    pub fn slot_mut(&mut self, i: usize) -> Option<&mut Plane> {
        self.slots.get_mut(i).and_then(|p| p.as_mut())
    }

    /// `p[i] = new Plane(...)` plus Java's implicit `npl = i + 1` for the
    /// byte-constructor slots.
    pub fn set_slot(&mut self, i: usize, p: Plane) {
        while self.slots.len() <= i {
            self.slots.push(None);
        }
        self.slots[i] = Some(p);
        if self.npl <= i {
            self.npl = i + 1;
        }
    }

    /// `this.npl = n`.
    pub fn set_npl(&mut self, n: usize) {
        self.npl = n;
    }

    /// Engine/test helper: append at `npl` (grows the array when it is not a
    /// fixed-capacity byte-ctor array).
    pub fn push(&mut self, p: Plane) {
        if self.npl < self.slots.len() {
            self.slots[self.npl] = Some(p);
        } else {
            self.slots.push(Some(p));
        }
        self.npl += 1;
    }

    /// `npl = 0` — drop the live prefix but keep the allocated slots (a
    /// cleared byte-ctor array is still `p.length == 120`).
    pub fn clear(&mut self) {
        self.npl = 0;
    }
}

fn deref_slot(o: &Option<Plane>) -> &Plane {
    o.as_ref()
        .expect("NullPointerException: ContO.p[] slot is null (ContO.java:588)")
}

fn deref_slot_mut(o: &mut Option<Plane>) -> &mut Plane {
    o.as_mut()
        .expect("NullPointerException: ContO.p[] slot is null (ContO.java:588)")
}

impl Index<usize> for Planes {
    type Output = Plane;

    fn index(&self, i: usize) -> &Plane {
        if self.null_array {
            panic!("NullPointerException: ContO.p is null (ContO.java:438)");
        }
        match self.slots.get(i) {
            Some(Some(p)) => p,
            Some(None) => {
                panic!("NullPointerException: ContO.p[{i}] is null (ContO.java:588)")
            }
            None => panic!(
                "ArrayIndexOutOfBoundsException: index out of bounds: p[{i}] past p.length {} (ContO.java:268)",
                self.slots.len()
            ),
        }
    }
}

impl IndexMut<usize> for Planes {
    fn index_mut(&mut self, i: usize) -> &mut Plane {
        if self.null_array {
            panic!("NullPointerException: ContO.p is null (ContO.java:438)");
        }
        let len = self.slots.len();
        match self.slots.get_mut(i) {
            Some(Some(p)) => p,
            Some(None) => {
                panic!("NullPointerException: ContO.p[{i}] is null (ContO.java:588)")
            }
            None => panic!(
                "ArrayIndexOutOfBoundsException: index out of bounds: p[{i}] past p.length {len} (ContO.java:268)"
            ),
        }
    }
}

impl<'a> IntoIterator for &'a Planes {
    type Item = &'a Plane;
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, Option<Plane>>, fn(&'a Option<Plane>) -> &'a Plane>;

    fn into_iter(self) -> Self::IntoIter {
        assert!(
            self.npl <= self.slots.len(),
            "ArrayIndexOutOfBoundsException: npl {} past p.length {}",
            self.npl,
            self.slots.len()
        );
        self.slots[..self.npl]
            .iter()
            .map(deref_slot as fn(&'a Option<Plane>) -> &'a Plane)
    }
}

impl<'a> IntoIterator for &'a mut Planes {
    type Item = &'a mut Plane;
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'a, Option<Plane>>,
        fn(&'a mut Option<Plane>) -> &'a mut Plane,
    >;

    fn into_iter(self) -> Self::IntoIter {
        assert!(
            self.npl <= self.slots.len(),
            "ArrayIndexOutOfBoundsException: npl {} past p.length {}",
            self.npl,
            self.slots.len()
        );
        let n = self.npl;
        self.slots[..n]
            .iter_mut()
            .map(deref_slot_mut as fn(&'a mut Option<Plane>) -> &'a mut Plane)
    }
}

impl From<Vec<Plane>> for Planes {
    fn from(v: Vec<Plane>) -> Planes {
        Planes::from_planes(v)
    }
}

impl Default for Planes {
    fn default() -> Planes {
        Planes::empty()
    }
}

// ── `.rad` byte-constructor support ─────────────────────────────────────

/// Java `String.trim()`: strips chars <= U+0020 from both ends (content is
/// ASCII; `str::trim` follows Unicode White_Space, so stay exact).
fn java_trim(s: &str) -> &str {
    let b = s.as_bytes();
    let mut start = 0;
    let mut end = b.len();
    while start < end && b[start] <= 0x20 {
        start += 1;
    }
    while end > start && b[end - 1] <= 0x20 {
        end -= 1;
    }
    &s[start..end]
}

/// Splits like `DataInputStream.readLine()` for the ASCII `.rad` streams:
/// on `\n`, dropping a trailing `\r` (the loader only reads text models).
fn java_lines(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    text.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect()
}

/// `(int)((float)value * var9)` (ContO.java:255-257 etc.).
fn java_scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale) as i32
}

/// The whole `try` body of the byte constructor (ContO.java:226-416).
/// `Err` is the swallowed exception: it stops the stream and returns the
/// wheel state so `from_bytes` can still set `grat` (`:418`). Returns the
/// `Wheels` accumulator on both paths because `ground` survives an abort.
fn parse_stream(c: &mut Conto, data: &[u8], m: &Medium) -> Result<Wheels, Wheels> {
    let mut wheels = Wheels::new();
    // parser locals — ContO.java:211-224 (`var9` scale, `var6` in-plane,
    // `var7` in-track, `var8` vertex count, `var13` colour, `var14`
    // glass, `var15` road, `var17` wheel index, `var18` per-plane gr)
    let mut scale = 1.0f32;
    let mut in_plane = false;
    let mut in_track = false;
    let mut vx = [0i32; 100];
    let mut vy = [0i32; 100];
    let mut vz = [0i32; 100];
    let mut nv = 0usize;
    let mut color = [0i32; 3];
    let mut glass = false;
    let mut road = false;
    let mut wheel_n = 0usize;
    let mut gr = 1i32;

    // `getvalue` throws on a malformed column; the catch aborts the stream.
    macro_rules! gv {
        ($line:expr, $key:expr, $col:expr) => {
            match crate::model::getvalue($line, $key, $col) {
                Ok(v) => v,
                Err(_) => return Err(wheels),
            }
        };
    }
    // An index/range failure in the original's arrays.
    macro_rules! abort {
        () => {
            return Err(wheels)
        };
    }

    for raw in java_lines(data) {
        let line = java_trim(&raw);

        if line.starts_with("<p>") {
            in_plane = true;
            nv = 0;
            gr = 0; // ContO.java:235 — gr is per-plane state
        }

        if in_plane {
            if line.starts_with("gr") {
                gr = gv!(line, "gr", 0);
            }

            if line.starts_with("c") {
                glass = false; // :244 — a colour line clears glass
                color[0] = gv!(line, "c", 0);
                color[1] = gv!(line, "c", 1);
                color[2] = gv!(line, "c", 2);
            }

            if line.starts_with("glass") {
                glass = true;
            }

            if line.starts_with("p") {
                if nv >= vx.len() {
                    abort!(); // var10[var8] past 99
                }
                let gx = gv!(line, "p", 0);
                let gy = gv!(line, "p", 1);
                let gz = gv!(line, "p", 2);
                vx[nv] = java_scaled(gx, scale);
                vy[nv] = java_scaled(gy, scale);
                vz[nv] = java_scaled(gz, scale);
                let (x, y, z) = (vx[nv], vy[nv], vz[nv]);
                // :258-261 — int squares/sum wrap in Java before the sqrt
                let r = (x
                    .wrapping_mul(x)
                    .wrapping_add(y.wrapping_mul(y))
                    .wrapping_add(z.wrapping_mul(z)) as f64)
                    .sqrt() as i32;
                if r > c.maxr {
                    c.maxr = r;
                }
                nv += 1;
            }
        }

        if line.starts_with("</p>") {
            // :268 — `new Plane(...)` is evaluated before `npl++`. A plane
            // with fewer than 3 points indexes ox[2] inside Plane and
            // throws (Plane.java:111-113), aborting the whole parse.
            if nv < 3 {
                abort!();
            }
            let npl = c.planes.len();
            if npl >= c.planes.p_len() {
                abort!(); // this.p[this.npl] past 120
            }
            let p = Plane::new(
                m,
                &vx[..nv],
                &vz[..nv],
                &vy[..nv],
                nv,
                color,
                glass,
                gr,
                0,
                0,
                0,
                c.disline,
                0,
                road,
            );
            c.planes.set_slot(npl, p);
            c.planes.set_npl(npl + 1);
            in_plane = false;
        }

        if line.starts_with("w") {
            if wheel_n >= 4 {
                abort!(); // keyx[4] / keyz[4]
            }
            let gx = gv!(line, "w", 0);
            let gy = gv!(line, "w", 1);
            let gz = gv!(line, "w", 2);
            let ga = gv!(line, "w", 3);
            let gmr = gv!(line, "w", 4);
            let gmz = gv!(line, "w", 5);
            c.keyx[wheel_n] = java_scaled(gx, scale);
            c.keyz[wheel_n] = java_scaled(gz, scale);
            wheel_n += 1;
            let mut made = Vec::new();
            wheels.make(
                m,
                &mut made,
                java_scaled(gx, scale),
                java_scaled(gy, scale),
                java_scaled(gz, scale),
                ga, // `a` is NOT scaled (:285)
                java_scaled(gmr, scale),
                java_scaled(gmz, scale),
            );
            let at = c.planes.len();
            for (i, p) in made.into_iter().enumerate() {
                if at + i >= c.planes.p_len() {
                    abort!(); // Wheels.make's p[var4] write
                }
                c.planes.set_slot(at + i, p);
            }
            c.planes.set_npl(at + 9); // :289 — after make returns
        }

        if line.starts_with("tracks") {
            let n = gv!(line, "tracks", 0);
            if n < 0 {
                abort!(); // NegativeArraySizeException
            }
            let n = n as usize;
            c.txy = Some(vec![0; n]);
            c.tzy = Some(vec![0; n]);
            c.tc = Some(vec![[0; 3]; n]);
            c.tradx = Some(vec![0; n]);
            c.tradz = Some(vec![0; n]);
            c.trady = Some(vec![0; n]);
            c.tx = Some(vec![0; n]);
            c.ty = Some(vec![0; n]);
            c.tz = Some(vec![0; n]);
            c.skd = Some(vec![0; n]);
            c.dam = Some(vec![0; n]);
        }

        if line.starts_with("<track>") {
            in_track = true;
            let i = c.tnt;
            // this.dam[i] — null array is an NPE, a short array an AIOOBE;
            // both abort (:309-310).
            if c.dam.as_ref().is_none_or(|a| i >= a.len()) {
                abort!();
            }
            c.dam.as_mut().unwrap()[i] = 1;
            c.skd.as_mut().unwrap()[i] = 0;
            c.ty.as_mut().unwrap()[i] = 0;
            c.tx.as_mut().unwrap()[i] = 0;
            c.tz.as_mut().unwrap()[i] = 0;
            c.txy.as_mut().unwrap()[i] = 0;
            c.tzy.as_mut().unwrap()[i] = 0;
            c.trady.as_mut().unwrap()[i] = 0;
            c.tradx.as_mut().unwrap()[i] = 0;
            c.tradz.as_mut().unwrap()[i] = 0;
            c.tc.as_mut().unwrap()[i] = [0; 3];
        }

        if in_track {
            let i = c.tnt;
            if c.dam.as_ref().is_none_or(|a| i >= a.len()) {
                abort!();
            }
            if line.starts_with("c") {
                let col = [gv!(line, "c", 0), gv!(line, "c", 1), gv!(line, "c", 2)];
                c.tc.as_mut().unwrap()[i] = col;
            }
            if line.starts_with("xy") {
                c.txy.as_mut().unwrap()[i] = gv!(line, "xy", 0);
            }
            if line.starts_with("zy") {
                c.tzy.as_mut().unwrap()[i] = gv!(line, "zy", 0);
            }
            if line.starts_with("radx") {
                let v = gv!(line, "radx", 0);
                c.tradx.as_mut().unwrap()[i] = java_scaled(v, scale);
            }
            if line.starts_with("rady") {
                let v = gv!(line, "rady", 0);
                c.trady.as_mut().unwrap()[i] = java_scaled(v, scale);
            }
            if line.starts_with("radz") {
                let v = gv!(line, "radz", 0);
                c.tradz.as_mut().unwrap()[i] = java_scaled(v, scale);
            }
            if line.starts_with("ty") {
                let v = gv!(line, "ty", 0);
                c.ty.as_mut().unwrap()[i] = java_scaled(v, scale);
            }
            if line.starts_with("tx") {
                let v = gv!(line, "tx", 0);
                c.tx.as_mut().unwrap()[i] = java_scaled(v, scale);
            }
            if line.starts_with("tz") {
                let v = gv!(line, "tz", 0);
                c.tz.as_mut().unwrap()[i] = java_scaled(v, scale);
            }
            if line.starts_with("skid") {
                c.skd.as_mut().unwrap()[i] = gv!(line, "skid", 0);
            }
            if line.starts_with("dam") {
                c.dam.as_mut().unwrap()[i] = 3;
            }
        }

        if line.starts_with("</track>") {
            in_track = false;
            c.tnt += 1;
        }

        if line.starts_with("disp") {
            c.disp = gv!(line, "disp", 0);
        }

        if line.starts_with("disline") {
            c.disline = gv!(line, "disline", 0);
        }

        if line.starts_with("shadow") {
            c.shadow = true;
        }

        if line.starts_with("stonecold") {
            c.noline = true;
        }

        if line.starts_with("road") {
            road = true;
        }

        if line.starts_with("notroad") {
            road = false;
        }

        if line.starts_with("grounded") {
            c.grounded = gv!(line, "grounded", 0) as f32 / 100.0;
        }

        if line.starts_with("div") {
            scale = gv!(line, "div", 0) as f32 / 10.0;
        }
    }

    Ok(wheels)
}

/// The parsed model with runtime planes built — the model-backed form of the
/// state after the original `ContO(byte[], Medium, Trackers)` constructor:
/// planes parsed (9 wheel planes per `w()` inserted at their recorded slots,
/// into an `npl`-sized array — the good-parse shape, not the byte
/// constructor's fixed 120 slots), tracker blocks staged (NOT yet in
/// `Trackers` — the placement does that), and `grat` from `Wheels.ground`.
/// The exact Java byte-constructor object is [`Conto::from_bytes`].
/// Default exists for unit-test support (an empty base with no planes).
#[derive(Default)]
pub struct Base {
    pub planes: Vec<Plane>,
    pub keyx: [i32; 4],
    pub keyz: [i32; 4],
    // `<track>` block data, staged until placement copies it into `Trackers`
    pub txy: Vec<i32>,
    pub tzy: Vec<i32>,
    pub tc: Vec<[i32; 3]>,
    pub tx: Vec<i32>,
    pub ty: Vec<i32>,
    pub tz: Vec<i32>,
    pub skd: Vec<i32>,
    pub dam: Vec<i32>,
    pub tradx: Vec<i32>,
    pub rady: Vec<i32>,
    pub radz: Vec<i32>,
    pub maxr: i32,
    pub disp: i32,
    pub disline: i32,
    pub shadow: bool,
    pub noline: bool,
    pub grounded: f32,
    /// `this.grat = var16.ground` (ContO.java:418).
    pub grat: i32,
}

impl Base {
    /// Build from a parsed `Model`. Wheel planes are created by
    /// `Wheels.make` at their original `p[]` positions (order faithful to
    /// the single-pass original constructor).
    pub fn new(model: &Model, m: &Medium, wheels: &mut Wheels) -> Base {
        let mut planes: Vec<Plane> = Vec::new();
        let mut wi = 0usize; // next wheel to insert

        // Interleaved: wheel planes land exactly where the original's
        // `p[npl] += 9` appends put them (model.wheel_at = planes.len() then).
        for (idx, mp) in model.planes.iter().enumerate() {
            while wi < model.wheel_at.len() && model.wheel_at[wi] == idx {
                let w = &model.wheels[wi];
                wheels.make(m, &mut planes, w.x, w.y, w.z, w.a, w.mr, w.mz);
                wi += 1;
            }
            planes.push(Plane::new(
                m,
                &mp.ox,
                &mp.oz,
                &mp.oy,
                mp.oy.len(),
                mp.oc,
                mp.glass,
                mp.gr,
                0,
                0,
                0,
                model.disline,
                mp.bfase,
                mp.road,
            ));
        }
        // trailing wheels (w() lines after the last <p>)
        while wi < model.wheel_at.len() && model.wheel_at[wi] == model.planes.len() {
            let w = &model.wheels[wi];
            wheels.make(m, &mut planes, w.x, w.y, w.z, w.a, w.mr, w.mz);
            wi += 1;
        }

        Base {
            planes,
            keyx: model.keyx,
            keyz: model.keyz,
            txy: model.trackers.iter().map(|t| t.txy).collect(),
            tzy: model.trackers.iter().map(|t| t.tzy).collect(),
            tc: model.trackers.iter().map(|t| t.c).collect(),
            tx: model.trackers.iter().map(|t| t.x).collect(),
            ty: model.trackers.iter().map(|t| t.y).collect(),
            tz: model.trackers.iter().map(|t| t.z).collect(),
            skd: model.trackers.iter().map(|t| t.skd).collect(),
            dam: model.trackers.iter().map(|t| t.dam).collect(),
            tradx: model.trackers.iter().map(|t| t.radx).collect(),
            rady: model.trackers.iter().map(|t| t.rady).collect(),
            radz: model.trackers.iter().map(|t| t.radz).collect(),
            maxr: model.max_r,
            disp: model.disp,
            disline: model.disline,
            shadow: model.shadow,
            noline: model.noline,
            grounded: model.grounded,
            grat: wheels.ground,
        }
    }
}

/// A placed model — the result of the placement constructor plus the
/// mutable pose/animation state the engine drives per frame.
pub struct Conto {
    /// Java `p` + `npl` (see [`Planes`]).
    pub planes: Planes,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub xz: i32,
    pub xy: i32,
    pub zy: i32,
    /// Wheel spin angles (ContO fields wxz/wzy).
    pub wxz: i32,
    pub wzy: i32,
    pub dist: i32,
    pub maxr: i32,
    pub disp: i32,
    pub disline: i32,
    pub shadow: bool,
    pub noline: bool,
    pub grounded: f32,
    pub grat: i32,
    pub keyx: [i32; 4],
    pub keyz: [i32; 4],
    /// Dust phase counters (ContO.java:44) — 0 while a wheel's slot is
    /// idle; the `dust()` emitter seeds 1 and `pdust()` steps it 1..4.
    pub stg: [i32; 4],
    /// Set by `loadstage()` on `fix` pieces (the pylon arcs come from
    /// `electrify()` in `d()`; the flash comes from `fixit()` once gameplay
    /// sets `fix`).
    pub elec: bool,
    pub fix: bool,
    /// Set for rotated fix points (`fix(...,ang!=0)`).
    pub roted: bool,
    /// Electric-arc state (ContO.java:52-54): two arcs, each with an
    /// `edl`/`edr` vertical offset and an `elc` burst-phase counter.
    pub elc: [i32; 2],
    pub edl: [i32; 2],
    pub edr: [i32; 2],
    /// Fix-flash countdown, 0 = idle (ContO.java:56); advanced on every
    /// `d()` while `fix` is true and cleared again past 7.
    pub fcnt: i32,
    /// Wheel-dust particle state (ContO.java:41-49), one slot per wheel
    /// (size 4): world position `sx/sy/sz`, per-frame drift velocities
    /// `scx/scz`, size scale `smag`, the draw-pass alternator `dov`, and
    /// the `fulls` opacity flag.
    pub sx: [i32; 4],
    pub sy: [i32; 4],
    pub sz: [i32; 4],
    pub dov: [i32; 4],
    pub smag: [f32; 4],
    pub scx: [i32; 4],
    pub scz: [i32; 4],
    pub fulls: [bool; 4],
    /// `<track>` rows staged by the byte constructor (`ContO.java:29-40`),
    /// `null` (Rust `None`) until a `tracks(N)` line allocates them
    /// (`ContO.java:292-305`). The copy constructor appends these to the
    /// shared `Trackers` (`ContO.java:467-499`) and leaves them behind
    /// (`this.tnt` is never copied, `ContO.java:40`).
    pub tnt: usize,
    pub txy: Option<Vec<i32>>,
    pub tzy: Option<Vec<i32>>,
    pub tc: Option<Vec<[i32; 3]>>,
    pub tradx: Option<Vec<i32>>,
    pub tradz: Option<Vec<i32>>,
    pub trady: Option<Vec<i32>>,
    pub tx: Option<Vec<i32>>,
    pub ty: Option<Vec<i32>>,
    pub tz: Option<Vec<i32>>,
    pub skd: Option<Vec<i32>>,
    pub dam: Option<Vec<i32>>,
}

impl Conto {
    /// The declared field state (`ContO.java:7-56`): `p` empty, `npl == 0`,
    /// `disline == 7`, `grounded == 1.0`, everything else zero/false/null.
    pub fn blank() -> Conto {
        Conto {
            planes: Planes::empty(),
            x: 0,
            y: 0,
            z: 0,
            xz: 0,
            xy: 0,
            zy: 0,
            wxz: 0,
            wzy: 0,
            dist: 0,
            maxr: 0,
            disp: 0,
            disline: 7,
            shadow: false,
            noline: false,
            grounded: 1.0,
            grat: 0,
            keyx: [0; 4],
            keyz: [0; 4],
            stg: [0; 4],
            elec: false,
            fix: false,
            roted: false,
            elc: [0; 2],
            edl: [0; 2],
            edr: [0; 2],
            fcnt: 0,
            sx: [0; 4],
            sy: [0; 4],
            sz: [0; 4],
            dov: [0; 4],
            smag: [0.0; 4],
            scx: [0; 4],
            scz: [0; 4],
            fulls: [false; 4],
            tnt: 0,
            txy: None,
            tzy: None,
            tc: None,
            tradx: None,
            tradz: None,
            trady: None,
            tx: None,
            ty: None,
            tz: None,
            skd: None,
            dam: None,
        }
    }

    /// Place a base model at (x, y, z) with a yaw — the loader's use of the
    /// copy constructor `ContO(ContO, int, int, int, int)` (ContO.java:
    /// 421-508) with the parsed `Base` as the source. Copies each plane
    /// through `Plane::new` (re-deriving colors/deltaf against the same
    /// `Medium`), applies the yaw, refreshes `projf`, and copies staged
    /// trackers into the shared `Trackers`. Like the original, `p` is
    /// allocated at exactly `npl` slots (`:432`) and the destination keeps
    /// no staged `<track>` rows (`tnt` is never copied, `:40`).
    pub fn place(
        base: &Base,
        m: &Medium,
        t: &mut Trackers,
        x: i32,
        y: i32,
        z: i32,
        yaw: i32,
    ) -> Conto {
        let mut c = Conto::blank();
        c.planes = Planes::from_planes(
            base.planes
                .iter()
                .map(|p| {
                    Plane::new(
                        m,
                        &p.ox,
                        &p.oz,
                        &p.oy,
                        p.oy.len(),
                        p.oc,
                        p.glass,
                        p.gr,
                        p.wx,
                        p.wy,
                        p.wz,
                        base.disline,
                        p.bfase,
                        p.road,
                    )
                })
                .collect(),
        );
        c.x = x;
        c.y = y;
        c.z = z;
        c.maxr = base.maxr;
        c.disp = base.disp;
        c.disline = base.disline;
        c.shadow = base.shadow;
        c.noline = base.noline;
        c.grounded = base.grounded;
        c.grat = base.grat;
        c.keyx = base.keyx;
        c.keyz = base.keyz;

        // master copy + yaw rotation + projf refresh (ContO.java:461-465)
        for (dst, src) in (&mut c.planes).into_iter().zip(base.planes.iter()) {
            dst.master = src.master;
            let mut ox = dst.ox.clone();
            let mut oz = dst.oz.clone();
            rot(m, &mut ox, &mut oz, 0, 0, yaw);
            dst.ox = ox;
            dst.oz = oz;
            dst.loadprojf();
        }

        // staged trackers -> shared Trackers (ContO.java:467-499)
        if !base.txy.is_empty() {
            if t.nt + base.txy.len() > crate::trackers::TRACKER_CAP {
                // the original would throw ArrayIndexOutOfBounds; abort the
                // same way, but with a legible message
                panic!(
                    "Trackers overflow: {} + {} > {}",
                    t.nt,
                    base.txy.len(),
                    crate::trackers::TRACKER_CAP
                );
            }
            for k in 0..base.txy.len() {
                let j = t.nt;
                t.xy[j] =
                    (base.txy[k] as f32 * m.cos(yaw) - base.tzy[k] as f32 * m.sin(yaw)) as i32;
                t.zy[j] =
                    (base.tzy[k] as f32 * m.cos(yaw) + base.txy[k] as f32 * m.sin(yaw)) as i32;
                for ch in 0..3 {
                    let v = (base.tc[k][ch] as f32
                        + base.tc[k][ch] as f32 * (m.snap[ch] as f32 / 100.0))
                        as i32;
                    t.c[j][ch] = v.clamp(0, 255);
                }
                t.x[j] = ((c.x as f32) + base.tx[k] as f32 * m.cos(yaw)
                    - base.tz[k] as f32 * m.sin(yaw)) as i32;
                t.z[j] = ((c.z as f32)
                    + base.tz[k] as f32 * m.cos(yaw)
                    + base.tx[k] as f32 * m.sin(yaw)) as i32;
                t.y[j] = c.y + base.ty[k];
                t.skd[j] = base.skd[k];
                t.dam[j] = base.dam[k];
                let mut ang = yaw.abs();
                if ang == 180 {
                    ang = 0;
                }
                t.radx[j] =
                    (base.tradx[k] as f32 * m.cos(ang) + base.radz[k] as f32 * m.sin(ang)) as i32;
                t.radz[j] =
                    (base.tradx[k] as f32 * m.sin(ang) + base.radz[k] as f32 * m.cos(ang)) as i32;
                t.rady[j] = base.rady[k];
                t.nt += 1;
            }
        }

        c
    }

    /// The copy constructor `ContO(ContO, int, int, int, int)`
    /// (ContO.java:421-508), engine shape: the port's runtime contours are
    /// built from `Base` models whose `<track>` rows were already copied by
    /// [`Conto::place`], so their `tnt` is 0 and the staged-tracker block
    /// (`:467-499`) writes nothing. The full Java shape — sharing the
    /// source's `Trackers` and appending its rows — is [`Conto::clone_into`].
    pub fn clone_from(other: &Conto, m: &Medium, x: i32, y: i32, z: i32, adj: i32) -> Conto {
        Conto::clone_into(other, m, x, y, z, adj, &mut Trackers::new())
    }

    /// The copy constructor `ContO(ContO, int, int, int, int)`
    /// (ContO.java:421-508): a snapshot of a live contour — every plane
    /// rebuilt through `Plane::new` from the source's CURRENT rotted
    /// vertex arrays, the scalars carried over, x/y/z from the args,
    /// rotations zero, then the master copy + `adj` rotation pass with
    /// the `projf` refresh, then the source's staged `<track>` rows
    /// appended to the SHARED `Trackers` at `t.nt` (`:467-499`).
    ///
    /// `dist`/`wxz`/`wzy`/`stg`/`tnt` and the dust/fix actors stay at
    /// their fresh-contour defaults exactly like the Java field defaults
    /// the constructor leaves. Unlike the byte constructor there is no
    /// try/catch: a null source `p` (or null slot) panics like the
    /// original `NullPointerException` (`:435-438`).
    pub fn clone_into(
        other: &Conto,
        m: &Medium,
        x: i32,
        y: i32,
        z: i32,
        adj: i32,
        t: &mut Trackers,
    ) -> Conto {
        let mut c = Conto::blank();
        let n = other.planes.len();
        let mut built: Vec<Plane> = Vec::with_capacity(n);
        for i in 0..n {
            let p = &other.planes[i];
            built.push(Plane::new(
                m,
                &p.ox,
                &p.oz,
                &p.oy,
                p.oy.len(),
                p.oc,
                p.glass,
                p.gr,
                p.wx,
                p.wy,
                p.wz,
                other.disline,
                p.bfase,
                p.road,
            ));
        }
        c.planes = Planes::from_planes(built);
        c.x = x;
        c.y = y;
        c.z = z;
        c.maxr = other.maxr;
        c.disp = other.disp;
        c.disline = other.disline;
        c.noline = other.noline;
        c.shadow = other.shadow;
        c.grounded = other.grounded;
        c.grat = other.grat;
        c.keyx = other.keyx;
        c.keyz = other.keyz;

        // master copy + adj rotation + projf refresh (ContO.java:461-465)
        for i in 0..n {
            let src = &other.planes[i];
            let dst = &mut c.planes[i];
            dst.master = src.master;
            let mut ox = dst.ox.clone();
            let mut oz = dst.oz.clone();
            rot(m, &mut ox, &mut oz, 0, 0, adj);
            dst.ox = ox;
            dst.oz = oz;
            dst.loadprojf();
        }

        // staged trackers -> shared Trackers (ContO.java:467-499)
        if other.tnt != 0 {
            if t.nt + other.tnt > crate::trackers::TRACKER_CAP {
                panic!(
                    "Trackers overflow: {} + {} > {}",
                    t.nt,
                    other.tnt,
                    crate::trackers::TRACKER_CAP
                );
            }
            let txy = other
                .txy
                .as_ref()
                .expect("NullPointerException: ContO.txy is null (ContO.java:469)");
            let tzy = other.tzy.as_ref().expect("ContO.tzy is null");
            let tc = other.tc.as_ref().expect("ContO.tc is null");
            let tx = other.tx.as_ref().expect("ContO.tx is null");
            let ty = other.ty.as_ref().expect("ContO.ty is null");
            let tz = other.tz.as_ref().expect("ContO.tz is null");
            let skd = other.skd.as_ref().expect("ContO.skd is null");
            let dam = other.dam.as_ref().expect("ContO.dam is null");
            let tradx = other.tradx.as_ref().expect("ContO.tradx is null");
            let tradz = other.tradz.as_ref().expect("ContO.tradz is null");
            let rady = other.trady.as_ref().expect("ContO.trady is null");
            for k in 0..other.tnt {
                let j = t.nt;
                t.xy[j] = (txy[k] as f32 * m.cos(adj) - tzy[k] as f32 * m.sin(adj)) as i32;
                t.zy[j] = (tzy[k] as f32 * m.cos(adj) + txy[k] as f32 * m.sin(adj)) as i32;
                for ch in 0..3 {
                    let v =
                        (tc[k][ch] as f32 + tc[k][ch] as f32 * (m.snap[ch] as f32 / 100.0)) as i32;
                    t.c[j][ch] = v.clamp(0, 255);
                }
                t.x[j] =
                    (c.x as f32 + tx[k] as f32 * m.cos(adj) - tz[k] as f32 * m.sin(adj)) as i32;
                t.z[j] =
                    (c.z as f32 + tz[k] as f32 * m.cos(adj) + tx[k] as f32 * m.sin(adj)) as i32;
                t.y[j] = c.y + ty[k];
                t.skd[j] = skd[k];
                t.dam[j] = dam[k];
                let mut ang = adj.abs();
                if ang == 180 {
                    ang = 0;
                }
                t.radx[j] = (tradx[k] as f32 * m.cos(ang) + tradz[k] as f32 * m.sin(ang)) as i32;
                t.radz[j] = (tradx[k] as f32 * m.sin(ang) + tradz[k] as f32 * m.cos(ang)) as i32;
                t.rady[j] = rady[k];
                t.nt += 1;
            }
        }

        c
    }

    /// Port of `ContO(byte[], Medium, Trackers)` (ContO.java:207-419).
    ///
    /// `data == None` is the Java `new ContO(null, m, t)` case: the NPE is
    /// thrown at `new ByteArrayInputStream(null)` (`:227`) and swallowed by
    /// the catch (`:411-416`), leaving the empty state. Every parse failure
    /// (`ArrayIndexOutOfBoundsException`, `NullPointerException`,
    /// `NumberFormatException`, `StringIndexOutOfBoundsException`) aborts
    /// the rest of the stream and keeps the PARTIAL object, exactly like
    /// the original's single try/catch around the whole line loop. `p` is
    /// always a fixed 120-slot array (`:210`) whose tail stays null, and
    /// `grat` is set from the wheel ground even on the error path (`:418`).
    /// The `Trackers` argument is never written (`:207-419` only stores the
    /// reference at `:209`; the port takes it per call).
    pub fn from_bytes(data: Option<&[u8]>, m: &Medium) -> Conto {
        let mut c = Conto::blank();
        c.planes = Planes::capacity(PLANE_SLOTS);
        let wheels = match data {
            Some(bytes) => match parse_stream(&mut c, bytes, m) {
                Ok(w) | Err(w) => w,
            },
            None => Wheels::new(),
        };
        c.grat = wheels.ground;
        c
    }

    /// `ContO.xs` (ContO.java:702-708) — depth clamped at 50; the product
    /// and add overflow-wrap in Java (Conto.java:707), same parity note as
    /// `Plane.xs`.
    pub fn xs(&self, m: &Medium, v: i32, d: i32) -> i32 {
        let d = if d < 50 { 50 } else { d };
        (d - m.focus_point)
            .wrapping_mul(m.cx - v)
            .wrapping_div(d)
            .wrapping_add(v)
    }

    /// `ContO.ys` (ContO.java:199-205) — depth clamped at 50; same
    /// wrapping-parity note.
    pub fn ys(&self, m: &Medium, v: i32, d: i32) -> i32 {
        let d = if d < 50 { 50 } else { d };
        (d - m.focus_point)
            .wrapping_mul(m.cy - v)
            .wrapping_div(d)
            .wrapping_add(v)
    }

    /// `ContO.getpy` (ContO.java:645-647) — (point − position)/10 per axis,
    /// squared and summed; int division truncates toward zero and the
    /// subtraction/product wrap in Java.
    pub fn getpy(&self, a: i32, b: i32, c: i32) -> i32 {
        let dx = a.wrapping_sub(self.x).wrapping_div(10);
        let dy = b.wrapping_sub(self.y).wrapping_div(10);
        let dz = c.wrapping_sub(self.z).wrapping_div(10);
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .wrapping_add(dz.wrapping_mul(dz))
    }

    /// `ContO.py` (ContO.java:1213-1215) — squared 2D distance for the fix
    /// flash extents; the squares/sum overflow-wrap in Java.
    pub(crate) fn py(a: i32, b: i32, c: i32, d: i32) -> i32 {
        (a.wrapping_sub(b))
            .wrapping_mul(a.wrapping_sub(b))
            .wrapping_add(c.wrapping_sub(d).wrapping_mul(c.wrapping_sub(d)))
    }

    /// Port of `ContO.electrify(Graphics)` (ContO.java:1069-1211) — the
    /// two lightning arcs of an elec marker: per-arc jitter state in
    /// `elc/edl/edr`, camera transform, whole-octagon screen cull, the
    /// cyan fill (orange in `trk` mode) and the brighter outline while the
    /// pillar is near (`az[0] < 4000`). Ends by spinning the marker's
    /// invisible rotation (`xy`/`zy` += 11, ContO.java:1206-1210).
    ///
    /// Arc widths/jitters consume `m.random()` per call; the applet's PRNG
    /// is emulated by a deterministic xorshift here (documented
    /// divergence), so arc shapes are our stream's, not the applet's.
    pub fn electrify(&mut self, cv: &mut Canvas, m: &mut Medium) {
        for a in 0..2 {
            // ContO.java:1073-1077 — a spent arc reseeds its mid offsets
            if self.elc[a] == 0 {
                self.edl[a] = (380.0f32 - m.random() * 760.0) as i32;
                self.edr[a] = (380.0f32 - m.random() * 760.0) as i32;
                self.elc[a] = 1;
            }
            // ContO.java:1079-1082 — jittered mids and ±252 cross jitter
            let midl = (self.edl[a] as f32 + (190.0f32 - m.random() * 380.0)) as i32;
            let midd = (self.edr[a] as f32 + (190.0f32 - m.random() * 380.0)) as i32;
            let jit2 = (m.random() * 126.0) as i32;
            let jit1 = (m.random() * 126.0) as i32;
            // ContO.java:1083-1087 — burst phase; past the cap it reseeds
            if self.elc[a] as f32 > m.random() * 7.0 {
                self.elc[a] = 0;
            } else {
                self.elc[a] = self.elc[a].wrapping_add(1);
            }

            // ContO.java:1089-1113 — the 8 corners: pillar halfwidth 504,
            // midpoints at ±252 with the two jitters, tips ±10 ± rnd*10
            let mut ax = [0i32; 8];
            let mut ay = [0i32; 8];
            let mut az = [self.z - m.z; 8];
            ax[0] = self.x - m.x - 504;
            ay[0] = self.y - m.y - self.edl[a] - 10 - (m.random() * 10.0) as i32;
            ax[1] = self.x - m.x - 252 + jit1;
            ay[1] = self.y - m.y - midl - 10 - (m.random() * 10.0) as i32;
            ax[2] = self.x - m.x + 252 - jit2;
            ay[2] = self.y - m.y - midd - 10 - (m.random() * 10.0) as i32;
            ax[3] = self.x - m.x + 504;
            ay[3] = self.y - m.y - self.edr[a] - 10 - (m.random() * 10.0) as i32;
            ax[4] = self.x - m.x + 504;
            ay[4] = self.y - m.y - self.edr[a] + 10 + (m.random() * 10.0) as i32;
            ax[5] = self.x - m.x + 252 - jit2;
            ay[5] = self.y - m.y - midd + 10 + (m.random() * 10.0) as i32;
            ax[6] = self.x - m.x - 252 + jit1;
            ay[6] = self.y - m.y - midl + 10 + (m.random() * 10.0) as i32;
            ax[7] = self.x - m.x - 504;
            ay[7] = self.y - m.y - self.edl[a] + 10 + (m.random() * 10.0) as i32;

            // ContO.java:1114-1119 — rotated markers roll 90°, then the
            // camera transform (no self `xy` on electric arcs)
            if self.roted {
                rot(m, &mut ax, &mut az, self.x - m.x, self.z - m.z, 90);
            }
            rot(m, &mut ax, &mut az, m.cx, m.cz, m.xz);
            rot(m, &mut ay, &mut az, m.cy, m.cz, m.zy);

            // ContO.java:1120-1151 — project and count corners off each
            // screen edge (or behind depth 10); fully culled arcs skip
            let mut sx = [0i32; 8];
            let mut sy = [0i32; 8];
            let (mut out_t, mut out_b, mut out_l, mut out_r) = (0, 0, 0, 0);
            for i in 0..8 {
                sx[i] = self.xs(m, ax[i], az[i]);
                sy[i] = self.ys(m, ay[i], az[i]);
                if sy[i] < 0 || az[i] < 10 {
                    out_t += 1;
                }
                if sy[i] > m.h || az[i] < 10 {
                    out_b += 1;
                }
                if sx[i] < 0 || az[i] < 10 {
                    out_l += 1;
                }
                if sx[i] > m.w || az[i] < 10 {
                    out_r += 1;
                }
            }
            if out_l == 8 || out_t == 8 || out_b == 8 || out_r == 8 {
                continue;
            }
            // ContO.java:1153-1201 — cyan fill (the tracker-recolor mode
            // paints it orange), then the brighter outline when near
            let mut r = 0i32;
            let mut g = (255.0f32 + 255.0f32 * (m.snap[1] as f32 / 250.0)) as i32;
            let mut b = (223.0f32 + 223.0f32 * (m.snap[2] as f32 / 250.0)) as i32;
            g = g.clamp(0, 255);
            b = b.clamp(0, 255);
            if m.trk {
                r = 255;
                g = 128;
                b = 0;
            }
            cv.fill_polygon(&sx, &sy, [r as u8, g as u8, b as u8]);
            if az[0] < 4000 {
                g = (223.0f32 + 223.0f32 * (m.snap[1] as f32 / 250.0)) as i32;
                g = g.clamp(0, 255);
                b = (255.0f32 + 255.0f32 * (m.snap[2] as f32 / 250.0)) as i32;
                b = b.clamp(0, 255);
                cv.draw_polygon(&sx, &sy, [r as u8, g as u8, b as u8]);
            }
        }

        // ContO.java:1206-1210 — the invisible pillar spin; `int` += wraps
        if !self.roted {
            self.xy = self.xy.wrapping_add(11);
        } else {
            self.zy = self.zy.wrapping_add(11);
        }
    }

    /// Port of `ContO.fixit(Graphics)` (ContO.java:843-1067) — the blue
    /// flash of a consumed fix point, animated by `fcnt` 1..8: a one-shot
    /// plane recolor, per-frame `flx` marks consumed by `Plane.d`'s glow
    /// paths, two expanding jittered octagons over the marker, and the
    /// reset that clears `fix`.
    pub fn fixit(&mut self, cv: &mut Canvas, m: &mut Medium) {
        // ContO.java:844-880 — recolor every plane to fix-light blue with
        // the snap boost, then the HSB round-trip back into the plane
        if self.fcnt == 1 {
            for p in &mut self.planes {
                p.hsb[0] = 0.57;
                p.hsb[2] = 0.8;
                p.hsb[1] = 0.8;
                let mut rgb = color::hsb_to_rgb(p.hsb[0], p.hsb[1], p.hsb[2]);
                for k in 0..3 {
                    let v = (rgb[k] as f32 + rgb[k] as f32 * (m.snap[k] as f32 / 100.0)) as i32;
                    if v > 255 {
                        rgb[k] = 255;
                    } else if v < 0 {
                        rgb[k] = 0;
                    } else {
                        rgb[k] = v;
                    }
                }
                p.hsb = color::rgb_to_hsb(rgb[0], rgb[1], rgb[2]);
                p.flx = 1;
            }
        }

        // ContO.java:882-892 — the recolor marks the glow paths use
        if self.fcnt == 2 {
            for p in &mut self.planes {
                p.flx = 1;
            }
        }
        if self.fcnt == 4 {
            for p in &mut self.planes {
                p.flx = 3;
            }
        }

        // ContO.java:894-1059 — the expanding white flash octagons
        if self.fcnt == 1 || self.fcnt > 2 {
            let mut ax = [0i32; 8];
            let mut ay = [0i32; 8];
            let mut az = [0i32; 4];
            for i in 0..4 {
                ax[i] = self.keyx[i] + self.x - m.x;
                ay[i] = self.grat + self.y - m.y;
                az[i] = self.keyz[i] + self.z - m.z;
            }
            // ContO.java:906-910 — self xy/zy/xz then camera; the second
            // pivot z-component is `this.z - this.m.y` in the shipped
            // applet (javap fixit@531 loads Medium.y) — quirk kept
            rot(m, &mut ax, &mut ay, self.x - m.x, self.y - m.y, self.xy);
            rot(m, &mut ay, &mut az, self.y - m.y, self.z - m.y, self.zy);
            rot(m, &mut ax, &mut az, self.x - m.x, self.z - m.z, self.xz);
            rot(m, &mut ax, &mut az, m.cx, m.cz, m.xz);
            rot(m, &mut ay, &mut az, m.cy, m.cz, m.zy);

            // ContO.java:917-941 — extents: max |dx| (ex), max |dy| (ey)
            // and max py(...) (fd2), whose sqrt/1.5 floors the first two
            let mut ex = 0i32;
            let mut ey = 0i32;
            let mut d2max = 0i32;
            for i in 0..4 {
                for j in 0..4 {
                    let dx = ax[i].wrapping_sub(ax[j]).wrapping_abs();
                    if dx > ex {
                        ex = dx;
                    }
                    let dy = ay[i].wrapping_sub(ay[j]).wrapping_abs();
                    if dy > ey {
                        ey = dy;
                    }
                    let d2 = Self::py(ax[i], ax[j], ay[i], ay[j]);
                    if d2 > d2max {
                        d2max = d2;
                    }
                }
            }
            d2max = ((d2max as f64).sqrt() / 1.5) as i32;
            if ex < d2max {
                ex = d2max;
            }
            if ey < d2max {
                ey = d2max;
            }

            // ContO.java:943-948 — projected center in camera space
            let fx = m.cx
                + ((self.x - m.x - m.cx) as f32 * m.cos(m.xz)
                    - (self.z - m.z - m.cz) as f32 * m.sin(m.xz)) as i32;
            let mut fd = m.cz
                + ((self.x - m.x - m.cx) as f32 * m.sin(m.xz)
                    + (self.z - m.z - m.cz) as f32 * m.cos(m.xz)) as i32;
            let fy = m.cy
                + ((self.y - m.y - m.cy) as f32 * m.cos(m.zy) - (fd - m.cz) as f32 * m.sin(m.zy))
                    as i32;
            fd = m.cz
                + ((self.y - m.y - m.cy) as f32 * m.sin(m.zy) + (fd - m.cz) as f32 * m.cos(m.zy))
                    as i32;

            // ContO.java:949-964 — the outer octagon: every corner in
            // double math jittered off m.random(), projected via xs/ys
            // (coords hoisted to locals so each random draw runs in the
            // applet's order, before the projection borrows m)
            let mut px = [0i32; 8];
            let mut py_ = [0i32; 8];
            let cx0 = ((fx as f64) - (ex as f64) / 0.8 - (m.random() as f64) * ((ex as f64) / 2.4))
                as i32;
            let cy0 = ((fy as f64)
                - (ey as f64) / 1.92
                - (m.random() as f64) * ((ey as f64) / 5.67)) as i32;
            px[0] = self.xs(m, cx0, fd);
            py_[0] = self.ys(m, cy0, fd);
            let cx1 = ((fx as f64) - (ex as f64) / 0.8 - (m.random() as f64) * ((ex as f64) / 2.4))
                as i32;
            let cy1 = ((fy as f64)
                + (ey as f64) / 1.92
                + (m.random() as f64) * ((ey as f64) / 5.67)) as i32;
            px[1] = self.xs(m, cx1, fd);
            py_[1] = self.ys(m, cy1, fd);
            let cx2 = ((fx as f64)
                - (ex as f64) / 1.92
                - (m.random() as f64) * ((ex as f64) / 5.67)) as i32;
            let cy2 = ((fy as f64) + (ey as f64) / 0.8 + (m.random() as f64) * ((ey as f64) / 2.4))
                as i32;
            px[2] = self.xs(m, cx2, fd);
            py_[2] = self.ys(m, cy2, fd);
            let cx3 = ((fx as f64)
                + (ex as f64) / 1.92
                + (m.random() as f64) * ((ex as f64) / 5.67)) as i32;
            let cy3 = ((fy as f64) + (ey as f64) / 0.8 + (m.random() as f64) * ((ey as f64) / 2.4))
                as i32;
            px[3] = self.xs(m, cx3, fd);
            py_[3] = self.ys(m, cy3, fd);
            let cx4 = ((fx as f64) + (ex as f64) / 0.8 + (m.random() as f64) * ((ex as f64) / 2.4))
                as i32;
            let cy4 = ((fy as f64)
                + (ey as f64) / 1.92
                + (m.random() as f64) * ((ey as f64) / 5.67)) as i32;
            px[4] = self.xs(m, cx4, fd);
            py_[4] = self.ys(m, cy4, fd);
            let cx5 = ((fx as f64) + (ex as f64) / 0.8 + (m.random() as f64) * ((ex as f64) / 2.4))
                as i32;
            let cy5 = ((fy as f64)
                - (ey as f64) / 1.92
                - (m.random() as f64) * ((ey as f64) / 5.67)) as i32;
            px[5] = self.xs(m, cx5, fd);
            py_[5] = self.ys(m, cy5, fd);
            let cx6 = ((fx as f64)
                + (ex as f64) / 1.92
                + (m.random() as f64) * ((ex as f64) / 5.67)) as i32;
            let cy6 = ((fy as f64) - (ey as f64) / 0.8 - (m.random() as f64) * ((ey as f64) / 2.4))
                as i32;
            px[6] = self.xs(m, cx6, fd);
            py_[6] = self.ys(m, cy6, fd);
            let cx7 = ((fx as f64)
                - (ex as f64) / 1.92
                - (m.random() as f64) * ((ex as f64) / 5.67)) as i32;
            let cy7 = ((fy as f64) - (ey as f64) / 0.8 - (m.random() as f64) * ((ey as f64) / 2.4))
                as i32;
            px[7] = self.xs(m, cx7, fd);
            py_[7] = self.ys(m, cy7, fd);

            // ContO.java:965-982 — the wobble sweep: fcnt 3..7 rotates the
            // octagon around its projected center by +22/+22/0/−22/−22
            let prj_x = self.xs(m, fx, fd);
            let prj_y = self.ys(m, fy, fd);
            for (c, ang) in [(3, 22), (4, 22), (5, 0), (6, -22), (7, -22)] {
                if self.fcnt == c {
                    rot(m, &mut px, &mut py_, prj_x, prj_y, ang);
                }
            }

            // ContO.java:985-1013 — the wide white belly
            let mut r = (191.0f32 + 191.0f32 * (m.snap[0] as f32 / 200.0)) as i32;
            let mut g = (232.0f32 + 232.0f32 * (m.snap[1] as f32 / 200.0)) as i32;
            let mut b = (255.0f32 + 255.0f32 * (m.snap[2] as f32 / 200.0)) as i32;
            r = r.clamp(0, 255);
            g = g.clamp(0, 255);
            b = b.clamp(0, 255);
            cv.fill_polygon(&px, &py_, [r as u8, g as u8, b as u8]);

            // ContO.java:1014-1029 — the inner octagon: x corners in float
            // math with the int-div-4 jitter, y corners in double with the
            // ÷9.6 jitter (the applet indeed mixes float and double here)
            let cx0 = ((fx - ex) as f32 - m.random() * ((ex / 4) as f32)) as i32;
            let cy0 = ((fy as f64) - (ey as f64) / 2.4 - (m.random() as f64) * ((ey as f64) / 9.6))
                as i32;
            px[0] = self.xs(m, cx0, fd);
            py_[0] = self.ys(m, cy0, fd);
            let cx1 = ((fx - ex) as f32 - m.random() * ((ex / 4) as f32)) as i32;
            let cy1 = ((fy as f64) + (ey as f64) / 2.4 + (m.random() as f64) * ((ey as f64) / 9.6))
                as i32;
            px[1] = self.xs(m, cx1, fd);
            py_[1] = self.ys(m, cy1, fd);
            let cx2 = ((fx as f64) - (ex as f64) / 2.4 - (m.random() as f64) * ((ex as f64) / 9.6))
                as i32;
            let cy2 = ((fy + ey) as f32 + m.random() * ((ey / 4) as f32)) as i32;
            px[2] = self.xs(m, cx2, fd);
            py_[2] = self.ys(m, cy2, fd);
            let cx3 = ((fx as f64) + (ex as f64) / 2.4 + (m.random() as f64) * ((ex as f64) / 9.6))
                as i32;
            let cy3 = ((fy + ey) as f32 + m.random() * ((ey / 4) as f32)) as i32;
            px[3] = self.xs(m, cx3, fd);
            py_[3] = self.ys(m, cy3, fd);
            let cx4 = ((fx + ex) as f32 + m.random() * ((ex / 4) as f32)) as i32;
            let cy4 = ((fy as f64) + (ey as f64) / 2.4 + (m.random() as f64) * ((ey as f64) / 9.6))
                as i32;
            px[4] = self.xs(m, cx4, fd);
            py_[4] = self.ys(m, cy4, fd);
            let cx5 = ((fx + ex) as f32 + m.random() * ((ex / 4) as f32)) as i32;
            let cy5 = ((fy as f64) - (ey as f64) / 2.4 - (m.random() as f64) * ((ey as f64) / 9.6))
                as i32;
            px[5] = self.xs(m, cx5, fd);
            py_[5] = self.ys(m, cy5, fd);
            let cx6 = ((fx as f64) + (ex as f64) / 2.4 + (m.random() as f64) * ((ex as f64) / 9.6))
                as i32;
            let cy6 = ((fy - ey) as f32 - m.random() * ((ey / 4) as f32)) as i32;
            px[6] = self.xs(m, cx6, fd);
            py_[6] = self.ys(m, cy6, fd);
            let cx7 = ((fx as f64) - (ex as f64) / 2.4 - (m.random() as f64) * ((ex as f64) / 9.6))
                as i32;
            let cy7 = ((fy - ey) as f32 - m.random() * ((ey / 4) as f32)) as i32;
            px[7] = self.xs(m, cx7, fd);
            py_[7] = self.ys(m, cy7, fd);

            // ContO.java:1030-1058 — the brighter core
            r = (213.0f32 + 213.0f32 * (m.snap[0] as f32 / 200.0)) as i32;
            g = (239.0f32 + 239.0f32 * (m.snap[1] as f32 / 200.0)) as i32;
            b = (255.0f32 + 255.0f32 * (m.snap[2] as f32 / 200.0)) as i32;
            r = r.clamp(0, 255);
            g = g.clamp(0, 255);
            b = b.clamp(0, 255);
            cv.fill_polygon(&px, &py_, [r as u8, g as u8, b as u8]);
        }

        // ContO.java:1061-1066 — an 8-frame flash, then clear the flag
        if self.fcnt > 7 {
            self.fcnt = 0;
            self.fix = false;
        } else {
            self.fcnt = self.fcnt.wrapping_add(1);
        }
    }

    /// Port of `ContO.dust(...)` (ContO.java:660-681) — the wheel-dust
    /// emitter car physics calls per wheel: `wheel` 0..3, world position,
    /// per-frame drift `scx`/`scz`, puff size `mag`, the `full` opacity
    /// flag, and `gate` (the wheel's longitudinal speed): leading wheels
    /// (0/2) stay dry above +5 speed, the rear pair (1/3) above −5. A
    /// slot seeds only while idle (stg == 0) and the movement is real.
    pub fn dust(
        &mut self,
        wheel: i32,
        x: f32,
        y: f32,
        z: f32,
        scx: f32,
        scz: f32,
        mag: f32,
        full: bool,
        gate: i32,
    ) {
        let mut leading = false;
        if gate > 5 && (wheel == 0 || wheel == 2) {
            leading = true;
        }
        if gate < -5 && (wheel == 1 || wheel == 3) {
            leading = true;
        }

        if self.stg[wheel as usize] == 0 && scx.abs() + scz.abs() > 100.0 && !leading {
            self.sx[wheel as usize] = x as i32;
            self.sy[wheel as usize] = y as i32;
            self.sz[wheel as usize] = z as i32;
            self.stg[wheel as usize] = 1;
            self.dov[wheel as usize] = -1;
            self.smag[wheel as usize] = mag;
            self.scx[wheel as usize] = scx as i32;
            self.scz[wheel as usize] = scz as i32;
            self.fulls[wheel as usize] = full;
        }
    }

    /// Port of `ContO.pdust(slot, Graphics, dir)` (ContO.java:58-197) —
    /// the wheel-dust particle: ground-toned puff that tracker surfaces
    /// recolor, drifts/falls each step, and culls with the shipped `== 4`
    /// count quirk. `dir −1` is the pre-sort pass (d() ContO.java:576-580)
    /// and `+1` the per-plane master pass (ContO.java:606-608); the `dov`
    /// alternator switches the drawing between the two, and `dov`/`stg`
    /// advance on every charged call (drawn or culled).
    pub(crate) fn pdust(
        &mut self,
        s: usize,
        cv: &mut Canvas,
        m: &mut Medium,
        t: &Trackers,
        dir: i32,
    ) {
        if dir * self.dov[s] > 0 {
            // ContO.java:60-65 — density exponent; no int overflow is
            // possible (stg stays 0..4), so plain ops match Java here
            let mut dens = if self.fulls[s] {
                self.stg[s] * self.stg[s]
            } else {
                self.stg[s] * self.stg[s] * self.stg[s] + 1
            };

            // ContO.java:67-69 — base ground blend; byte-verified quirk:
            // all three channels blend with cfade[0] (the red fade
            // channels), not their own (javap pdust@81/122/163)
            let c0 = m.cfade[0] * 2;
            let mut r = (m.cgrnd[0] * dens + c0 + m.csky[0]) / (3 + dens);
            let mut g = (m.cgrnd[1] * dens + c0 + m.csky[1]) / (3 + dens);
            let mut b = (m.cgrnd[2] * dens + c0 + m.csky[2]) / (3 + dens);

            // ContO.java:71-84 — tracker surfaces recolor the puff inside
            // their non-vertical footprint; skid marks keep the density
            for tr in 0..t.nt {
                if (t.zy[tr]).wrapping_abs() != 90
                    && (t.xy[tr]).wrapping_abs() != 90
                    && (self.sx[s] - t.x[tr]).wrapping_abs() < t.radx[tr]
                    && (self.sz[s] - t.z[tr]).wrapping_abs() < t.radz[tr]
                {
                    if t.skd[tr] == 0 {
                        dens = self.stg[s] * self.stg[s] * self.stg[s] + 2;
                    }
                    r = ((t.c[tr][0] as f64 * 0.87 * dens as f64
                        + (m.cfade[0] * 2) as f64
                        + m.csky[0] as f64)
                        / (3 + dens) as f64) as i32;
                    g = ((t.c[tr][1] as f64 * 0.87 * dens as f64
                        + (m.cfade[0] * 2) as f64
                        + m.csky[1] as f64)
                        / (3 + dens) as f64) as i32;
                    b = ((t.c[tr][2] as f64 * 0.87 * dens as f64
                        + (m.cfade[0] * 2) as f64
                        + m.csky[2] as f64)
                        / (3 + dens) as f64) as i32;
                }
            }

            // ContO.java:86-101 — camera transform (sy capped at the
            // literal 250 ground)
            if self.sy[s] > 250 {
                self.sy[s] = 250;
            }
            let dx = (self.sx[s] - m.x - m.cx) as f32;
            let dz = (self.sz[s] - m.z - m.cz) as f32;
            let mut dp = m.cz + (dx * m.sin(m.xz) + dz * m.cos(m.xz)) as i32;
            let cxp = m.cx + (dx * m.cos(m.xz) - dz * m.sin(m.xz)) as i32;
            let cyp = m.cy
                + ((self.sy[s] - m.y - m.cy) as f32 * m.cos(m.zy)
                    - (dp - m.cz) as f32 * m.sin(m.zy)) as i32;
            dp = m.cz
                + ((self.sy[s] - m.y - m.cy) as f32 * m.sin(m.zy)
                    + (dp - m.cz) as f32 * m.cos(m.zy)) as i32;

            // ContO.java:102 — radial screen distance; the int squares and
            // their sum overflow-wrap in Java before the double widening
            let rad = (m.cy - cyp)
                .wrapping_mul(m.cy - cyp)
                .wrapping_add((m.cx - cxp).wrapping_mul(m.cx - cxp))
                .wrapping_add(dp.wrapping_mul(dp));
            let rfog = (rad as f64).sqrt() as i32;

            // ContO.java:103-111 — up to 8 crossed fade bands, each
            // blending 1/4 toward cfade (per channel here)
            for band in 0..8 {
                if rfog > m.fade[band] {
                    r = (r * 3 + m.cfade[0]) / 4;
                    g = (g * 3 + m.cfade[1]) / 4;
                    b = (b * 3 + m.cfade[2]) / 4;
                }
            }

            // ContO.java:113-120 — step drift: fast sideways puffs rise
            // slower, drifting puffs fall fast; scx/scz feed the next frame.
            // Every (int) here wraps the WHOLE float expression, not a
            // subtrahend: (int)(f32(sy) - X) truncates the RESULT
            // (ContO.java:113-118).
            if self.scx[s].wrapping_abs() + self.scz[s].wrapping_abs() > 150 {
                self.sy[s] = (self.sy[s] as f32 - (3.0f32 + 27.0f32 * self.smag[s])) as i32;
            } else {
                self.sy[s] = (self.sy[s] as f32 - (23.0f32 + 7.0f32 * self.smag[s])) as i32;
            }
            // ContO.java:119-120 — the (int) cast wraps the whole SUM, not
            // just the drift: with smag 0 the quotient is NaN/±Infinity and
            // NaN poisons the sum to 0 while ±Inf saturates at MIN/MAX.
            self.sx[s] = (self.sx[s] as f32
                + self.scx[s] as f32 / ((self.stg[s] + 1) as f32 * self.smag[s]))
                as i32;
            self.sz[s] = (self.sz[s] as f32
                + self.scz[s] as f32 / ((self.stg[s] + 1) as f32 * self.smag[s]))
                as i32;

            // ContO.java:121-143 — the puff octagon: x corners in float
            // math ((18 + rnd*18 + (stg-3)*6)*mag), y corners in double
            // ((7.5 + rnd*7.5 + (stg-3)*2.5)*mag), around (cxp, cyp) at
            // depth dp (coords hoisted so each random draw runs in the
            // applet's order, before the projection borrows m)
            let mut px = [0i32; 8];
            let mut py_ = [0i32; 8];
            let v14 = self.stg[s] - 3;
            let dx0 = (cxp as f32
                - (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            let dy0 = (cyp as f64
                - (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            px[0] = self.xs(m, dx0, dp);
            py_[0] = self.ys(m, dy0, dp);
            if py_[0] < 45 && m.flex != 0 {
                m.flex = 0;
            }
            let dx1 = (cxp as f32
                - (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            let dy1 = (cyp as f64
                + (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            px[1] = self.xs(m, dx1, dp);
            py_[1] = self.ys(m, dy1, dp);
            let dx2 = (cxp as f64
                - (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            let dy2 = (cyp as f32
                + (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            px[2] = self.xs(m, dx2, dp);
            py_[2] = self.ys(m, dy2, dp);
            let dx3 = (cxp as f64
                + (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            let dy3 = (cyp as f32
                + (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            px[3] = self.xs(m, dx3, dp);
            py_[3] = self.ys(m, dy3, dp);
            let dx4 = (cxp as f32
                + (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            let dy4 = (cyp as f64
                + (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            px[4] = self.xs(m, dx4, dp);
            py_[4] = self.ys(m, dy4, dp);
            let dx5 = (cxp as f32
                + (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            let dy5 = (cyp as f64
                - (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            px[5] = self.xs(m, dx5, dp);
            py_[5] = self.ys(m, dy5, dp);
            let dx6 = (cxp as f64
                + (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            let dy6 = (cyp as f32
                - (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            px[6] = self.xs(m, dx6, dp);
            py_[6] = self.ys(m, dy6, dp);
            let dx7 = (cxp as f64
                - (7.5f64 + m.random() as f64 * 7.5f64 + v14 as f64 * 2.5) * self.smag[s] as f64)
                as i32;
            let dy7 = (cyp as f32
                - (18.0f32 + m.random() * 18.0f32 + (v14 * 6) as f32) * self.smag[s])
                as i32;
            px[7] = self.xs(m, dx7, dp);
            py_[7] = self.ys(m, dy7, dp);

            // ContO.java:144-175 — the 4-edge count cull; the shipped code
            // compares against exactly 4 (javap pdust@2052-2070: four
            // iconst_4 if_icmpeq chains), so 5-of-8 out on one edge still
            // paints; heights < 45 wipe the Medium flex flag meanwhile
            let (mut out_t, mut out_b, mut out_l, mut out_r) = (0, 0, 0, 0);
            for i in 0..8 {
                if py_[i] < 0 || dp < 10 {
                    out_t += 1;
                }
                if py_[i] > m.h || dp < 10 {
                    out_b += 1;
                }
                if px[i] < 0 || dp < 10 {
                    out_l += 1;
                }
                if px[i] > m.w || dp < 10 {
                    out_r += 1;
                }
                if py_[i] < 45 && m.flex != 0 {
                    m.flex = 0;
                }
            }

            if !(out_l == 4 || out_t == 4 || out_b == 4 || out_r == 4) {
                cv.fill_polygon(&px, &py_, [r as u8, g as u8, b as u8]);
            }

            // ContO.java:182-193 — dov flips after a drawn pass (drawn
            // with dir +1 this time, −1 the next); stg advances whether
            // drawn or culled, and `fulls` puffs release their slot at
            // stg 2 (dov reset so the emitter must re-seed)
            if self.dov[s] == 1 {
                self.dov[s] = -1;
            }

            if self.stg[s] == 4 {
                self.stg[s] = 0;
            } else {
                self.stg[s] += 1;
                if self.stg[s] == 2 && self.fulls[s] {
                    self.dov[s] = 0;
                }
            }
        } else if self.dov[s] == 0 {
            self.dov[s] = 1;
        }
    }

    /// Port of `ContO.d(Graphics)` (ContO.java:510-643) — the frame draw
    /// for one model: camera-relative cull, shadow pass, painter's sort on
    /// per-plane `av` from the previous pass, then per-plane `d()`.
    pub fn d(&mut self, cv: &mut Canvas, m: &mut Medium, t: &Trackers) {
        self.dist = 0;

        // ContO.java:515-519 — contour center in camera space
        let dx = (self.x - m.x - m.cx) as f32;
        let dz = (self.z - m.z - m.cz) as f32;
        let c2 = m.cx + (dx * m.cos(m.xz) - dz * m.sin(m.xz)) as i32;
        let c3 = m.cz + (dx * m.sin(m.xz) + dz * m.cos(m.xz)) as i32;
        let c4 = m.cz
            + ((self.y - m.y - m.cy) as f32 * m.sin(m.zy) + (c3 - m.cz) as f32 * m.cos(m.zy))
                as i32;

        // ContO.java:520-524 — bounding cull
        let near = self.xs(m, c2 + self.maxr, c4);
        let far = self.xs(m, c2 - self.maxr, c4);
        let culled = !(near > 0
            && far < m.w
            && c4 > -self.maxr
            && (c4 < m.fade[self.disline as usize] + self.maxr || m.trk)
            && (near - far > self.disp || m.trk));

        if !culled {
            // shadow pass (ContO.java:525-560)
            if self.shadow {
                if m.crs {
                    for p in &self.planes {
                        p.s(
                            cv,
                            m,
                            t,
                            self.x - m.x,
                            self.y - m.y,
                            self.z - m.z,
                            self.xz,
                            self.xy,
                            self.zy,
                            2,
                        );
                    }
                } else if c4 >= 2000 {
                    self.lowshadow(cv, m, t, c4);
                } else {
                    // tracker-surface vs ground shadow (ContO.java:533-557)
                    let mut on_tracker = false;
                    for k in (0..t.nt).rev() {
                        if t.zy[k].abs() != 90
                            && t.xy[k].abs() != 90
                            && (self.x - t.x[k]).abs() < t.radx[k] + self.maxr
                            && (self.z - t.z[k]).abs() < t.radz[k] + self.maxr
                        {
                            on_tracker = true;
                            break;
                        }
                    }
                    if on_tracker {
                        for p in &self.planes {
                            p.s(
                                cv,
                                m,
                                t,
                                self.x - m.x,
                                self.y - m.y,
                                self.z - m.z,
                                self.xz,
                                self.xy,
                                self.zy,
                                0,
                            );
                        }
                    } else {
                        // ground-window check (ContO.java:550-556)
                        let gy = m.cy
                            + ((m.ground - m.cy) as f32 * m.cos(m.zy)
                                - (c3 - m.cz) as f32 * m.sin(m.zy))
                                as i32;
                        let gz = m.cz
                            + ((m.ground - m.cy) as f32 * m.sin(m.zy)
                                + (c3 - m.cz) as f32 * m.cos(m.zy))
                                as i32;
                        if self.ys(m, gy + self.maxr, gz) > 0
                            && self.ys(m, gy - self.maxr, gz) < m.h
                        {
                            for p in &self.planes {
                                p.s(
                                    cv,
                                    m,
                                    t,
                                    self.x - m.x,
                                    self.y - m.y,
                                    self.z - m.z,
                                    self.xz,
                                    self.xy,
                                    self.zy,
                                    1,
                                );
                            }
                        }
                    }
                    // ContO.java:559 — register as a shadow hole
                    m.addsp(
                        self.x - m.x,
                        self.z - m.z,
                        ((self.maxr as f64) * 0.8) as i32,
                    );
                }
            }

            // vertical window check (ContO.java:563-564)
            let vy_ = m.cy
                + ((self.y - m.y - m.cy) as f32 * m.cos(m.zy) - (c3 - m.cz) as f32 * m.sin(m.zy))
                    as i32;
            if self.ys(m, vy_ + self.maxr, c4) > 0 && self.ys(m, vy_ - self.maxr, c4) < m.h {
                // ContO.java:565-571 — electric arcs / fix flash run under
                // the same gate as the plane painting
                if self.elec {
                    self.electrify(cv, m);
                }
                if self.fix {
                    self.fixit(cv, m);
                }

                // ContO.java:576-580 — wheel dust drawn ahead of the sort
                // for the slots whose dov alternator says it's their turn
                for s in 0..4 {
                    if self.stg[s] != 0 {
                        self.pdust(s, cv, m, t, -1);
                    }
                }

                // painter's sort on the PREVIOUS pass's plane distances
                // (ContO.java:586-600); ties send the later index farther
                let npl = self.planes.len();
                let mut cnt = vec![0usize; npl];
                for i in 0..npl {
                    for j in (i + 1)..npl {
                        if self.planes[i].av != self.planes[j].av {
                            if self.planes[i].av < self.planes[j].av {
                                cnt[i] += 1;
                            } else {
                                cnt[j] += 1;
                            }
                        } else {
                            // the original's `else if (i > j)` branch is
                            // dead code in this loop shape; the `else`
                            // always runs
                            cnt[j] += 1;
                        }
                    }
                }

                // pose locals first — the draw mutates `self.planes`
                let px = self.x - m.x;
                let py = self.y - m.y;
                let pz = self.z - m.z;
                let (axz, axy, azy, wxz, wzy, noline) =
                    (self.xz, self.xy, self.zy, self.wxz, self.wzy, self.noline);

                // ContO.java:602-611 — draw by sort key; after each plane
                // paints, its master wheel (planes reference wheel 1..4)
                // can trail a dust puff
                for slot in 0..npl {
                    for i in 0..npl {
                        if cnt[i] == slot {
                            // Mirror seam for `RecordingPlane extends Plane`
                            // (TContODraw.drawOrder): Rust cannot subclass,
                            // so the dispatch records the plane INDEX here
                            // and the mirror maps its fixture tags to the
                            // indices it assigned. Test-only; no pixels read.
                            #[cfg(test)]
                            draw_log::push(i);
                            self.planes[i].d(cv, m, px, py, pz, axz, axy, azy, wxz, wzy, noline);
                            let master = self.planes[i].master;
                            if master != 0 && self.stg[(master - 1) as usize] != 0 {
                                self.pdust((master - 1) as usize, cv, m, t, 1);
                            }
                        }
                    }
                }

                // ContO.java:613-626 — camera distance * grounded; the int
                // squares and their sum overflow-wrap in Java before the
                // double widening
                let dx = m.x + m.cx - self.x;
                let dz = m.z - self.z;
                let dy = m.y + m.cy - self.y;
                let inner = (dx
                    .wrapping_mul(dx)
                    .wrapping_add(dz.wrapping_mul(dz))
                    .wrapping_add(dy.wrapping_mul(dy)) as f64)
                    .sqrt() as i32;
                self.dist = ((inner as f64).sqrt() * self.grounded as f64) as i32;
            }
        }

        // dust timer advance (ContO.java:630-642) — idle while stg is zero
        if self.dist == 0 {
            for k in 0..4 {
                if self.stg[k] != 0 {
                    if self.stg[k] == 4 {
                        self.stg[k] = 0;
                    } else {
                        self.stg[k] += 1;
                    }
                }
            }
        }
    }

    /// `ContO.lowshadow(Graphics, int)` (ContO.java:710-841) — the coarse
    /// shadow used when the contour center projects >= 2000 camera units
    /// deep (`d()` ContO.java:530). The quad hangs off the model's key
    /// points (x scaled 1.2, z scaled 1.4 with a sign flipped when the
    /// model's pitch folds past 90); it lies on `m.ground` unless > 2 of
    /// its corners fall inside a (untilted) tracker plan, in which case the
    /// tracker's surface height and ground tint take over.
    pub(crate) fn lowshadow(&self, cv: &mut Canvas, m: &Medium, t: &Trackers, dist: i32) {
        let mut x3 = [0i32; 4];
        let mut y4 = [0i32; 4];
        let mut z5 = [0i32; 4];
        let var6 = {
            // byte var6 = ±1 from the folded |zy| (ContO.java:714-724)
            let mut var7 = self.zy.abs();
            while var7 > 270 {
                var7 -= 360;
            }
            if var7.abs() > 90 { -1 } else { 1 }
        };

        // 4 corners in key-point order 0, 1, 3, 2 (ContO.java:726-733)
        for (k, &ki) in [0usize, 1, 3, 2].iter().enumerate() {
            x3[k] = (self.keyx[ki] as f64 * 1.2 + self.x as f64 - m.x as f64) as i32;
            z5[k] = ((self.keyz[ki] * var6) as f64 * 1.4 + self.z as f64 - m.z as f64) as i32;
            y4[k] = m.ground;
        }
        rot(m, &mut x3, &mut z5, self.x - m.x, self.z - m.z, self.xz);

        let mut var8 = (m.cgrnd[0] as f32 as f64 / 1.5) as i32;
        let mut var9 = (m.cgrnd[1] as f32 as f64 / 1.5) as i32;
        let mut var10 = (m.cgrnd[2] as f32 as f64 / 1.5) as i32;

        // tracker surface search, nearest-slot first (ContO.java:744-792)
        for k in (0..t.nt).rev() {
            let mut var12 = 0;
            for c in 0..4 {
                if t.zy[k].abs() != 90
                    && t.xy[k].abs() != 90
                    && (x3[c] - (t.x[k] - m.x)).abs() < t.radx[k]
                    && (z5[c] - (t.z[k] - m.z)).abs() < t.radz[k]
                {
                    var12 += 1;
                }
            }
            if var12 > 2 {
                for c in 0..4 {
                    y4[c] = t.y[k] - m.y;
                    if t.zy[k] != 0 {
                        let a = (z5[c] - (t.z[k] - m.z - t.radz[k])) as f32;
                        y4[c] = (y4[c] as f32
                            + (a * m.sin(t.zy[k]) / m.sin(90 - t.zy[k])
                                - t.radz[k] as f32 * m.sin(t.zy[k]) / m.sin(90 - t.zy[k])))
                            as i32;
                    }
                    if t.xy[k] != 0 {
                        let a = (x3[c] - (t.x[k] - m.x - t.radx[k])) as f32;
                        y4[c] = (y4[c] as f32
                            + (a * m.sin(t.xy[k]) / m.sin(90 - t.xy[k])
                                - t.radx[k] as f32 * m.sin(t.xy[k]) / m.sin(90 - t.xy[k])))
                            as i32;
                    }
                }
                var8 = (t.c[k][0] as f32 as f64 / 1.5) as i32;
                var9 = (t.c[k][1] as f32 as f64 / 1.5) as i32;
                var10 = (t.c[k][2] as f32 as f64 / 1.5) as i32;
                break;
            }
        }

        // camera-space rotation (ContO.java:794-795)
        rot(m, &mut x3, &mut z5, m.cx, m.cz, m.xz);
        rot(m, &mut y4, &mut z5, m.cy, m.cz, m.zy);

        // screen projection + cull, matching the four independent tests
        // (ContO.java:803-825); z (z5) stays raw for the clip tests
        let (mut c20, mut c22, mut c14, mut c15) = (0, 0, 0, 0);
        for k in 0..4 {
            x3[k] = self.xs(m, x3[k], z5[k]);
            y4[k] = self.ys(m, y4[k], z5[k]);
            if y4[k] < 0 || z5[k] < 10 {
                c20 += 1;
            }
            if y4[k] > m.h || z5[k] < 10 {
                c22 += 1;
            }
            if x3[k] < 0 || z5[k] < 10 {
                c14 += 1;
            }
            if x3[k] > m.w || z5[k] < 10 {
                c15 += 1;
            }
        }
        if c14 == 4 || c20 == 4 || c22 == 4 || c15 == 4 {
            return;
        }

        // fog: each crossed fade band blends 1/4 toward cfade (ContO.java:830-836)
        for k in 0..8 {
            if dist > m.fade[k] {
                var8 = (var8 * 3 + m.cfade[0]) / 4;
                var9 = (var9 * 3 + m.cfade[1]) / 4;
                var10 = (var10 * 3 + m.cfade[2]) / 4;
            }
        }
        cv.fill_polygon(&x3, &y4, clampc([var8, var9, var10]));
    }
}

/// Camera modes from `Medium.java` whose signatures need a `Conto`; they
/// live here to keep `medium.rs` free of the type. `aroundtrack` needs
/// `CheckPoints` (not yet ported).
/// `Medium.follow(ContO, int)` (Medium.java:543-552).
pub fn follow(m: &mut Medium, c: &Conto, ang: i32) {
    // the original simplifies (var1.z - 800 - var1.z) to the constant 800
    m.zy = 10;
    m.xz = -ang;
    m.x = c.x - m.cx + (800.0 * m.sin(ang)) as i32;
    m.z = c.z - m.cz + (-800.0 * m.cos(ang)) as i32;
    m.y = c.y - 250 - m.cy;
    if m.trns != 1 {
        m.trns = 1;
    }
}

/// `Medium.watch(ContO, int)` (Medium.java:285-326) — chase camera.
pub fn watch(m: &mut Medium, c: &Conto, ang: i32) {
    if m.flex != 0 {
        m.flex = 0;
    }

    if m.td {
        m.y = ((c.y - 300) as f32 - 1100.0 * m.random()) as i32;
        m.x = c.x
            + ((c.x + 400 - c.x) as f32 * m.cos(ang) - (c.z + 5000 - c.z) as f32 * m.sin(ang))
                as i32;
        m.z = c.z
            + ((c.x + 400 - c.x) as f32 * m.sin(ang) + (c.z + 5000 - c.z) as f32 * m.cos(ang))
                as i32;
        m.td = false;
    }

    let mut var3 = 0i32;
    if c.x - m.x - m.cx > 0 {
        var3 = 180;
    }

    let var4 = -((90 + var3) as f64
        + ((c.z - m.z) as f64 / (c.x - m.x - m.cx) as f64).atan() / (core::f64::consts::PI / 180.0))
        as i32;
    let mut var3 = 0i32;
    if c.y - m.y - m.cy < 0 {
        var3 = -180;
    }

    // the int squares and their sums overflow-wrap in Java before the
    // double widening (Medium.java:308/316)
    let dx = c.x - m.x - m.cx;
    let dy = c.y - m.y - m.cy;
    let dz = c.z - m.z;
    let var5 = (dz.wrapping_mul(dz).wrapping_add(dx.wrapping_mul(dx)) as f64).sqrt() as i32;
    let var6 = ((90 + var3) as f64
        - (var5 as f64 / dy as f64).atan() / (core::f64::consts::PI / 180.0)) as i32;
    m.xz += (var4 - m.xz) / m.trns;
    if m.trns != 1 {
        m.trns -= 1;
    }
    m.zy += (var6 - m.zy) / 5;
    if (dz
        .wrapping_mul(dz)
        .wrapping_add(dx.wrapping_mul(dx))
        .wrapping_add(dy.wrapping_mul(dy)) as f64)
        .sqrt()
        > 6000.0
    {
        m.td = true;
    }
}

/// `Medium.aroundtrack(CheckPoints)` (Medium.java:454-504) — the stage-load
/// flyover: camera spirals down from a height towards the track center,
/// then swoops from checkpoint to checkpoint. State runs on `Medium`
/// (`hit`, `ptr`, `ptcnt`, `nrnd`) seeded by `Scene::build`.
pub fn aroundtrack(m: &mut Medium, cp: &CheckPoints) {
    if m.flex != 0 {
        m.flex = 0;
    }

    m.y = -m.hit;
    m.x = m.cx + m.trx as i32 + (12000.0f32 * m.cos(m.vxz)) as i32;
    m.z = m.trz as i32 + (12000.0f32 * m.sin(m.vxz)) as i32;
    m.hit -= 3000;
    if m.hit < 5000 {
        m.hit = 5000;
        m.trx -= (m.trx - cp.x[m.ptr] as i64) / 10;
        m.trz -= (m.trz - cp.z[m.ptr] as i64) / 10;
        if m.ptcnt == 7 {
            m.ptr += 1;
            if m.ptr as i32 == cp.n {
                m.ptr = 0;
                m.nrnd += 1;
            }
            m.ptcnt = 0;
        } else {
            m.ptcnt += 1;
        }
    }

    m.vxz += 2;
    if m.vxz > 360 {
        m.vxz -= 360;
    }

    m.xz = -m.vxz - 90;
    let mut var2 = 0i32;
    if -m.y - m.cy < 0 {
        var2 = -180;
    }

    let var3 = (((m.trz - m.z as i64 + m.cz as i64) * (m.trz - m.z as i64 + m.cz as i64)
        + (m.trx - m.x as i64 - m.cx as i64) * (m.trx - m.x as i64 - m.cx as i64))
        as f64)
        .sqrt() as i32;
    if m.zy >= 30 || m.hit == 57000 {
        m.zy = ((90 + var2) as f64
            - (var3 as f64 / (-m.y - m.cy) as f64).atan() / (core::f64::consts::PI / 180.0))
            as i32;
    } else if m.zy > 9 {
        m.zy -= 1;
    } else {
        m.zy = 9;
    }
}

/// `Medium.around(ContO, boolean)` (Medium.java:334-396) — orbit camera.
pub fn around(m: &mut Medium, c: &Conto, run: bool) {
    if m.flex != 0 {
        m.flex = 0;
    }

    if !run {
        if !m.vert {
            m.adv += 2;
        } else {
            m.adv -= 2;
        }
        if m.adv > 900 {
            m.vert = true;
        }
        if m.adv < -500 {
            m.vert = false;
        }
    } else {
        m.adv -= 20;
    }

    let mut var3 = 500 + m.adv;
    if var3 < 1000 {
        var3 = 1000;
    }

    m.y = c.y - m.adv;
    if m.y > 10 {
        m.vert = false;
    }

    // (var1.x - var3 - var1.x) simplifies to -var3
    m.x = c.x + ((c.x - var3 - c.x) as f32 * m.cos(m.vxz)) as i32;
    m.z = c.z + ((c.x - var3 - c.x) as f32 * m.sin(m.vxz)) as i32;
    if !run {
        m.vxz += 2;
    } else {
        m.vxz += 4;
    }

    let mut var4 = 0i32;
    let mut var5 = m.y;
    if var5 > 0 {
        var5 = 0;
    }
    if c.y - var5 - m.cy < 0 {
        var4 = -180;
    }

    // int squares + sum overflow-wrap in Java before the widening
    // (Medium.java:385)
    let dx = c.x - m.x - m.cx;
    let dz = c.z - m.z + m.cz;
    let var6 = (dz.wrapping_mul(dz).wrapping_add(dx.wrapping_mul(dx)) as f64).sqrt() as i32;
    let mut var7 = ((90 + var4) as f64
        - (var6 as f64 / (c.y - var5 - m.cy) as f64).atan() / (core::f64::consts::PI / 180.0))
        as i32;
    m.xz = -m.vxz + 90;
    if run {
        var7 -= 15;
    }
    m.zy += (var7 - m.zy) / 10;
    if m.trns != 5 {
        m.trns = 5;
    }
}

/// The `RecordingPlane extends Plane` stand-in (`TContODraw.java:998-1015`):
/// Java logs the draw ORDER from an overridden `Plane.d`, which Rust cannot
/// express by subclassing. `Conto::d` pushes the plane INDEX at the dispatch
/// point instead; the mirror assigns its fixture tags (A/B/C) to the same
/// indices, so the pinned sequence is identical. A thread-local keeps the
/// suite single-threaded and cheap; it never touches pixels or port state.
#[cfg(test)]
pub(crate) mod draw_log {
    use std::cell::RefCell;

    thread_local! {
        static DRAWN: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn clear() {
        DRAWN.with_borrow_mut(|v| v.clear());
    }

    pub(crate) fn push(i: usize) {
        DRAWN.with_borrow_mut(|v| v.push(i));
    }

    pub(crate) fn entries() -> Vec<usize> {
        DRAWN.with_borrow(|v| v.clone())
    }
}
