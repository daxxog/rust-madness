//! Port of the `.rad` model format — the `ContO(byte[], Medium, Trackers)`
//! constructor from the original game
//! (reference/vineflower/ContO.java, lines 207-419) plus its `getvalue`
//! helper (lines 683-700), reproduced with their original quirks so parsed
//! values are bit-compatible with the Java engine.
//!
//! `.rad` files are text command streams; the parser is line-based:
//!
//! ```text
//! div(19)            scale factor n/10.0 applied to coordinates from here on
//! shadow()           cast a shadow
//! stonecold()        draw without outlines ("noline")
//! road() / notroad() mark subsequent polygons as road / not road
//! grounded(n)        grounding factor n/100.0
//! disp(n disline(n)  display level / outline style (default 7)
//! <p> ... </p>       one polygon: gr(n), c(r,g,b), glass, p(x,y,z)*, `</p>`
//! w(x,y,z,a,mr,mz)   a wheel anchor; Base::new runs Wheels.make at this
//!                    slot, appending 9 planes (order faithful)
//! tracks(n)          declare n <track> surface trackers
//! <track> ... </track> sky-tracker: c(r,g,b), xy, zy, radx, rady, radz,
//!                    ty, tx, tz, skid, dam
//! ```

use std::fmt;

/// Original: `this.p = new Plane[120]` (ContO.java:210). Models must fit:
/// planes parsed from `</p>` plus 9 per wheel stay within this.
pub const PLANE_CAP: usize = 120;

/// One polygon in model space. Field names match the original `Plane`
/// constructor parameters (see the `ContO(ContO, ...)` copy constructor,
/// ContO.java:421-452): `ox`/`oz`/`oy` hold the vertex x/z/y coordinates,
/// `oc` the color, plus the flags the parser manages.
pub struct Plane {
    pub ox: Vec<i32>,
    pub oz: Vec<i32>,
    pub oy: Vec<i32>,
    pub oc: [i32; 3],
    pub glass: bool,
    pub gr: i32,
    pub road: bool,
    /// 0 when parsed; wheel planes set their own later (Wheels.make, pending).
    pub bfase: i32,
}

/// Arguments of a `w(...)` line in exactly the form `Wheels.make(...)` will
/// consume once that subsystem is ported (ContO.java:277-288).
pub struct Wheel {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub a: i32,
    pub mr: i32,
    pub mz: i32,
}

/// One `<track>...</track>` block. The original stores these in allocations
/// on `Trackers` (ContO.java:292-375) but the data belongs to the model; the
/// placement constructor copies each entry into the shared `Trackers` object
/// at load time.
pub struct Tracker {
    pub c: [i32; 3],
    pub txy: i32,
    pub tzy: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub radx: i32,
    pub rady: i32,
    pub radz: i32,
    pub skd: i32,
    pub dam: i32,
}

pub struct Model {
    /// `<p>` polygons. Original keeps a fixed `[Plane; 120]` (`this.p`)
    /// plus `npl`; `Vec` with the same contents.
    pub planes: Vec<Plane>,
    /// `w(...)` lines, in file order. In the original, wheels append 9
    /// planes each directly into `p` at parse time (npl += 9,
    /// ContO.java:289); the parse here is Medium-free, so `Base::new`
    /// interleaves them at the recorded slots with the same order.
    pub wheels: Vec<Wheel>,
    /// For each wheel, `Wheels.make`'s insertion index into `p` (= plane
    /// count at the `w()` line), mirroring the original's direct
    /// `p[npl] += 9-plane` append so plane order stays faithful.
    pub wheel_at: Vec<usize>,
    /// Wheel anchors (ContO.java:274-275), max 4 like the original arrays.
    pub keyx: [i32; 4],
    pub keyz: [i32; 4],
    pub trackers: Vec<Tracker>,
    pub max_r: i32,
    pub disp: i32,
    /// Outline style, default 7 (ContO.java:22).
    pub disline: i32,
    pub shadow: bool,
    /// Set by the `stonecold` command in the original code.
    pub noline: bool,
    /// Default 1.0; `grounded(n)` sets n/100.0 (ContO.java:401-403).
    pub grounded: f32,
}

#[derive(Debug)]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub text: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ContO loading error\nAt file: {}\nAt line {}: {}\n--------------------",
            self.file, self.line, self.text
        )
    }
}

impl std::error::Error for ParseError {}

/// Port of `ContO.getvalue(String,String,int)` (ContO.java:683-700),
/// including its separator-skip quirk: after each `,` or `)` the scan skips
/// one extra character (the original assumes well-formed comma-separated
/// lists with no spaces). Missing/empty/non-numeric values are errors here,
/// where the original's `Integer.valueOf` would throw and abort the whole
/// parse via the constructor's catch block.
pub fn getvalue(line: &str, key: &str, col: usize) -> Result<i32, String> {
    let b = line.as_bytes();
    let mut acc: Vec<u8> = Vec::new();
    let mut seps: usize = 0;
    let mut i = key.len() + 1; // index just past the '('
    while i < b.len() {
        // Java walks UTF-16 code units; .rad content is ASCII numerals.
        let c = b[i];
        if c == b',' || c == b')' {
            seps += 1;
            i += 1; // the original's skip-one-char quirk
        }
        if seps == col {
            if i >= b.len() {
                // Java's charAt would throw here and abort the parse.
                return Err(format!(
                    "value column {col} of {key}(...) runs past end of line"
                ));
            }
            acc.push(b[i]);
        }
        i += 1;
    }
    // Integer.valueOf(empty/string) throws -> parse error.
    let s = String::from_utf8_lossy(&acc).into_owned();
    s.parse::<i32>()
        .map_err(|_| format!("invalid integer {s:?} for {key}(...) column {col}"))
}

/// `(int)((float)value * var9)` — scale with Java's truncating cast.
/// Magnitudes here are tiny, so the f32->i32 saturation corner case of
/// Rust's `as` never differs from Java's wrap in practice.
fn java_scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale) as i32
}

/// Java `String.trim()`: strips chars <= U+0020 from both ends (Rust's
/// `str::trim` follows Unicode White_Space instead; content is ASCII so
/// this is equivalent, but stay exact).
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

/// Splits like `DataInputStream.readLine()` did: on `\n`, dropping a
/// trailing `\r`.
fn java_lines(data: &[u8]) -> impl Iterator<Item = String> {
    // Content is ASCII (numerals/letters); lossy conversion is for safety.
    let text = String::from_utf8_lossy(data);
    let mut owned: Vec<String> = Vec::new();
    for l in text.split('\n') {
        let l = l.strip_suffix('\r').unwrap_or(l);
        owned.push(l.to_string());
    }
    owned.into_iter()
}

impl Model {
    /// Faithful port of the `ContO(byte[])` constructor. Sequential,
    /// independent `if` checks in the original order — a single line can
    /// match several `startsWith` branches, exactly like the Java code.
    pub fn parse_rad(fname: &str, data: &[u8]) -> Result<Model, ParseError> {
        let mut model = Model {
            planes: Vec::new(),
            wheels: Vec::new(),
            wheel_at: Vec::new(),
            keyx: [0; 4],
            keyz: [0; 4],
            trackers: Vec::new(),
            max_r: 0,
            disp: 0,
            disline: 7,
            shadow: false,
            noline: false,
            grounded: 1.0,
        };

        // Parser locals named after the original's var9/var6/var7/var8/
        // var13/var14/var15/var17/var18 (ContO.java:211-224).
        let mut scale: f32 = 1.0; // var9
        let mut in_plane = false; // var6
        let mut in_track = false; // var7
        let mut vx: Vec<i32> = Vec::new(); // var10
        let mut vy: Vec<i32> = Vec::new(); // var11
        let mut vz: Vec<i32> = Vec::new(); // var12
        let mut nv: usize = 0; // var8
        let mut color = [0i32; 3]; // var13
        let mut glass = false; // var14
        let mut road = false; // var15
        let mut wheel_n = 0usize; // var17
        let mut gr = 1i32; // var18

        for (idx, raw) in java_lines(data).enumerate() {
            let lineno = idx + 1;
            let line = java_trim(&raw);
            let err = |msg: String| ParseError {
                file: fname.to_string(),
                line: lineno,
                text: msg,
            };

            if line.starts_with("<p>") {
                in_plane = true;
                nv = 0;
                gr = 0;
                vx.clear();
                vy.clear();
                vz.clear();
            }

            if in_plane {
                if line.starts_with("gr") {
                    gr = getvalue(line, "gr", 0).map_err(&err)?;
                }

                if line.starts_with("c") {
                    glass = false;
                    color[0] = getvalue(line, "c", 0).map_err(&err)?;
                    color[1] = getvalue(line, "c", 1).map_err(&err)?;
                    color[2] = getvalue(line, "c", 2).map_err(&err)?;
                }

                if line.starts_with("glass") {
                    glass = true;
                }

                if line.starts_with("p") {
                    let gx = getvalue(line, "p", 0).map_err(&err)?;
                    let gy = getvalue(line, "p", 1).map_err(&err)?;
                    let gz = getvalue(line, "p", 2).map_err(&err)?;
                    let x = java_scaled(gx, scale);
                    let y = java_scaled(gy, scale);
                    let z = java_scaled(gz, scale);
                    vx.push(x);
                    vy.push(y);
                    vz.push(z);
                    // maxR from the norm of the stored (scaled) vertex,
                    // (int)Math.sqrt(...) (ContO.java:258-261).
                    let r = ((x as i64 * x as i64 + y as i64 * y as i64 + z as i64 * z as i64)
                        as f64)
                        .sqrt() as i32;
                    if r > model.max_r {
                        model.max_r = r;
                    }
                    nv += 1;
                }
            }

            if line.starts_with("</p>") {
                model.planes.push(Plane {
                    ox: vx[..nv].to_vec(),
                    oz: vz[..nv].to_vec(),
                    oy: vy[..nv].to_vec(),
                    oc: color,
                    glass,
                    gr,
                    road,
                    bfase: 0,
                });
                in_plane = false;
            }

            if line.starts_with("w") {
                if wheel_n >= 4 {
                    // The original would throw ArrayIndexOutOfBounds here.
                    return Err(err("more than 4 wheel anchors".to_string()));
                }
                let gx = getvalue(line, "w", 0).map_err(&err)?;
                let gy = getvalue(line, "w", 1).map_err(&err)?;
                let gz = getvalue(line, "w", 2).map_err(&err)?;
                let ga = getvalue(line, "w", 3).map_err(&err)?;
                let gmr = getvalue(line, "w", 4).map_err(&err)?;
                let gmz = getvalue(line, "w", 5).map_err(&err)?;
                model.keyx[wheel_n] = java_scaled(gx, scale);
                model.keyz[wheel_n] = java_scaled(gz, scale);
                model.wheel_at.push(model.planes.len());
                model.wheels.push(Wheel {
                    x: java_scaled(gx, scale),
                    y: java_scaled(gy, scale),
                    z: java_scaled(gz, scale),
                    a: ga,
                    mr: java_scaled(gmr, scale),
                    mz: java_scaled(gmz, scale),
                });
                wheel_n += 1;
                // The 9 wheel planes land at this recorded slot via
                // Wheels.make in `Base::new` (the parse is Medium-free;
                // ContO.java:277-289 appends them right here).
            }

            if line.starts_with("tracks") {
                let n = getvalue(line, "tracks", 0).map_err(&err)? as usize;
                // Original re-allocates the arrays here (ContO.java:292-305).
                model.trackers.clear();
                model.trackers.reserve_exact(n);
            }

            if line.starts_with("<track>") {
                in_track = true;
                model.trackers.push(Tracker {
                    c: [0; 3],
                    txy: 0,
                    tzy: 0,
                    x: 0,
                    y: 0,
                    z: 0,
                    radx: 0,
                    rady: 0,
                    radz: 0,
                    skd: 0,
                    dam: 1, // defaults set on <track> (ContO.java:307-322)
                });
            }

            if in_track {
                // Field lines can only follow a `<track>` (which pushed the
                // slot), so this is Some for well-formed files; a missing
                // slot is malformed and the original would hit AIOOBE here,
                // aborting the parse (ContO.java:324-370).
                let Some(t) = model.trackers.last_mut() else {
                    return Err(err(
                        "tracker field line without a live <track> slot".to_string()
                    ));
                };
                if line.starts_with("c") {
                    t.c[0] = getvalue(line, "c", 0).map_err(&err)?;
                    t.c[1] = getvalue(line, "c", 1).map_err(&err)?;
                    t.c[2] = getvalue(line, "c", 2).map_err(&err)?;
                }
                if line.starts_with("xy") {
                    t.txy = getvalue(line, "xy", 0).map_err(&err)?;
                }
                if line.starts_with("zy") {
                    t.tzy = getvalue(line, "zy", 0).map_err(&err)?;
                }
                if line.starts_with("radx") {
                    t.radx = java_scaled(getvalue(line, "radx", 0).map_err(&err)?, scale);
                }
                if line.starts_with("rady") {
                    t.rady = java_scaled(getvalue(line, "rady", 0).map_err(&err)?, scale);
                }
                if line.starts_with("radz") {
                    t.radz = java_scaled(getvalue(line, "radz", 0).map_err(&err)?, scale);
                }
                if line.starts_with("ty") {
                    t.y = java_scaled(getvalue(line, "ty", 0).map_err(&err)?, scale);
                }
                if line.starts_with("tx") {
                    t.x = java_scaled(getvalue(line, "tx", 0).map_err(&err)?, scale);
                }
                if line.starts_with("tz") {
                    t.z = java_scaled(getvalue(line, "tz", 0).map_err(&err)?, scale);
                }
                if line.starts_with("skid") {
                    t.skd = getvalue(line, "skid", 0).map_err(&err)?;
                }
                if line.starts_with("dam") {
                    t.dam = 3; // original literal (ContO.java:367-369)
                }
            }

            if line.starts_with("</track>") {
                in_track = false;
            }

            if line.starts_with("disp") {
                model.disp = getvalue(line, "disp", 0).map_err(&err)?;
            }

            if line.starts_with("disline") {
                model.disline = getvalue(line, "disline", 0).map_err(&err)?;
            }

            if line.starts_with("shadow") {
                model.shadow = true;
            }

            if line.starts_with("stonecold") {
                model.noline = true;
            }

            if line.starts_with("road") {
                road = true;
            }

            if line.starts_with("notroad") {
                road = false;
            }

            if line.starts_with("grounded") {
                model.grounded = getvalue(line, "grounded", 0).map_err(&err)? as f32 / 100.0;
            }

            if line.starts_with("div") {
                scale = getvalue(line, "div", 0).map_err(err)? as f32 / 10.0;
            }
        }

        Ok(model)
    }

    /// What `npl` would be in the original after parse + Wheels.make.
    pub fn npl_estimate(&self) -> usize {
        self.planes.len() + self.wheels.len() * 9
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getvalue_matches_java_scan() {
        // plain single value
        assert_eq!(getvalue("c(226,120,56)", "c", 0), Ok(226));
        assert_eq!(getvalue("c(226,120,56)", "c", 1), Ok(120));
        assert_eq!(getvalue("c(226,120,56)", "c", 2), Ok(56));
        // skip-quirk: suffix after ')' must not leak into a value
        assert_eq!(getvalue("set(10,0,5600,180)p", "set", 3), Ok(180));
        // negative values
        assert_eq!(getvalue("p(-4550,2800,0)", "p", 0), Ok(-4550));
        // missing columns / junk values error like Integer.valueOf would throw
        assert!(getvalue("glass()", "glass", 0).is_err());
        assert!(getvalue("c(1,2,3)", "c", 3).is_err());
        assert!(getvalue("c(ab,2,3)", "c", 0).is_err());
    }

    #[test]
    fn parses_snippet_like_formula7() {
        let src = b"div(19)\nshadow()\n\n<p>\nc(226,120,56)\n\np(-7,0,67)\np(-10,-4,50)\np(10,-4,50)\np(7,0,67)\n</p>\n\n<p>\nc(150,150,150)\nglass\np(0,-8,-20)\n</p>\n\ngrounded(100)\n";
        let m = Model::parse_rad("test.rad", src).expect("parses");
        // div(19): scale 1.9 -> (-7,0,67)*1.9 = (-13,0,127) truncating f32 math
        assert_eq!(m.planes.len(), 2);
        let p0 = &m.planes[0];
        assert_eq!(p0.ox, [-13, -19, 19, 13]);
        assert_eq!(p0.oy, [0, -7, -7, 0]); // (int)(-4*1.9) = -7 (trunc)
        assert_eq!(p0.oz, [127, 95, 95, 127]);
        assert_eq!(p0.oc, [226, 120, 56]);
        assert!(!p0.glass && !p0.road);
        let p1 = &m.planes[1];
        assert_eq!(p1.oc, [150, 150, 150]);
        assert!(p1.glass);
        assert!(m.shadow);
        assert_eq!(m.grounded, 1.0);
        // maxR tracks the max per-vertex norm of the stored (scaled) ints,
        // (int)Math.sqrt(...) (ContO.java:258-261); (13,0,127) wins here:
        // sqrt(169 + 16129) = 127.66 -> 127
        assert_eq!(m.max_r, 127);
    }

    #[test]
    fn tracker_defaults_and_fields() {
        let src = b"tracks(2)\n<track>\nc(184,166,98)\nzy(90)\nradx(60)\nrady(60)\nradz(60)\n</track>\n<track>\nc(184,166,98)\nskid(1)\ndam()\n</track>\n";
        let m = Model::parse_rad("t.rad", src).expect("parses");
        assert_eq!(m.trackers.len(), 2);
        let t0 = &m.trackers[0];
        assert_eq!(t0.c, [184, 166, 98]);
        assert_eq!(t0.tzy, 90);
        assert_eq!(t0.radx, 60); // no div -> scale 1.0
        assert_eq!(t0.dam, 1); // default until a dam() line appears
        let t1 = &m.trackers[1];
        assert_eq!(t1.skd, 1);
        assert_eq!(t1.dam, 3);
    }
}
