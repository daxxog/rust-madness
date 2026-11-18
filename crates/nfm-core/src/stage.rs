//! Port of the stage layer of `GameSparker.java` + `xtGraphics` start data:
//! the `loadstage()` command parser (`getint`/`getstring` scans kept
//! verbatim, including the `getint("chk", ...)` on `set` lines and the
//! `)`-suffix flag gates) and scene assembly — palette commands into
//! `Medium`, piece placements into the object pool, `maxr/maxl/maxt/maxb`
//! wall rows + their wall trackers, and the 5 car start slots.

use crate::checkpoints::CheckPoints;
use crate::conto::{Base, Conto};
use crate::control::Control;
use crate::medium::Medium;
use crate::model::Model;
use crate::trackers::Trackers;
use crate::wheels::Wheels;
use crate::xtgraphics::XtGraphics;
use crate::zipo::Zipo;

/// The model table of `loadbase()` (GameSparker.java:168-212), in order.
/// Stage commands index this positionally (`set(14,...)` = slot 14 =
/// `turn`; slot 39 = `thewall`, which `maxr/maxl/maxt/maxb` rows use).
pub const MODEL_TABLE: [&str; 43] = [
    "2000tornados",
    "formula7",
    "canyenaro",
    "lescrab",
    "nimi",
    "maxrevenge",
    "leadoxide",
    "king",
    "radicalone",
    "drmonster",
    "road",
    "froad",
    "twister2",
    "twister1",
    "turn",
    "offroad",
    "bumproad",
    "offturn",
    "nroad",
    "nturn",
    "roblend",
    "noblend",
    "rnblend",
    "roadend",
    "offroadend",
    "hpground",
    "ramp30",
    "cramp35",
    "dramp15",
    "dhilo15",
    "slide10",
    "takeoff",
    "sramp22",
    "offbump",
    "offramp",
    "sofframp",
    "halfpipe",
    "spikes",
    "rail",
    "thewall",
    "checkpoint",
    "fixpoint",
    "offcheckpoint",
];

/// Pairing rule of `loadbase()` (GameSparker.java:220-241): a zipo entry
/// names itself like `road.rad`; it lands in the LAST table slot whose name
/// is a prefix of the entry name (`var10` keeps the last match). Index =
/// table position; stage `set(N,...)` refers to that position.
pub fn model_slot_for_entry(entry_name: &str) -> Option<usize> {
    let mut slot = 0usize;
    let mut found = false;
    for (i, name) in MODEL_TABLE.iter().enumerate() {
        if entry_name.starts_with(name) {
            slot = i;
            found = true;
        }
    }
    if found { Some(slot) } else { None }
}

/// Port of `GameSparker.getint(name, line, idx)` (GameSparker.java:131-148).
/// The scan starts at `name.len() + 1` (skipping `key(`), walks chars, and
/// on `,`/`)` bumps the column and consumes EXACTLY one extra char — but the
/// column check runs after that bump, so the first char after the separator
/// belongs to the new column. An empty column throws like
/// `Integer.valueOf("")` (NumberFormatException) — surfaced as an error.
///
/// Quirk kept: `loadstage()` reads `set()` coords with
/// `getint("chk", line, i)` — the arity layout matches, so the scan starts
/// one char later but lands on the same columns. Port calls keep `chk`.
pub fn getint(name: &str, line: &str, idx: i32) -> Result<i32, StageError> {
    getstring(name, line, idx).and_then(|s| {
        s.parse::<i32>()
            .map_err(|_| StageError::new(line.to_string(), format!("bad integer `{s}`")))
    })
}

/// Port of `GameSparker.getstring` (GameSparker.java:112-129).
pub fn getstring(name: &str, line: &str, idx: i32) -> Result<String, StageError> {
    let chars: Vec<char> = line.chars().collect();
    let mut col = 0i32;
    let mut acc = String::new();
    let start = name.chars().count() + 1; // name.len() + 1
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c == ',' || c == ')' {
            col += 1;
            i += 1; // the separator consumes the slot after it as its own
        }
        if col == idx {
            acc.push(chars[i]);
        }
        i += 1;
    }
    if acc.is_empty() && col != idx {
        // like Integer.valueOf(""), only raised for numeric use; getstring
        // tolerates empty (name string may be empty)
        Ok(acc)
    } else if col < idx {
        Err(StageError::new(line.to_string(), "missing columns".into()))
    } else {
        Ok(acc)
    }
}

#[derive(Debug)]
pub struct StageError {
    pub line: String,
    pub message: String,
}

impl StageError {
    fn new(line: String, message: String) -> StageError {
        StageError { line, message }
    }
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at line: `{}`)", self.message, self.line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getint_getstring_quirks() {
        // the set/chk arity quirk: getint starts at name.len() + 1, so
        // reading a `set` line with the key "chk" still lands on the
        // same columns (loadstage passes "chk" for set lines)
        assert_eq!(getint("chk", "set(10,-4550,2800,0)p", 1).unwrap(), -4550);
        assert_eq!(getint("chk", "set(10,-4550,2800,0)p", 2).unwrap(), 2800);
        // fix reads its columns as x,y,z swapped: column 3 is y
        assert_eq!(
            getint("fix", "fix(41,-9100,10600,-1400,0)", 3).unwrap(),
            -1400
        );
        // getstring keeps spaces (stage names)
        let s = getstring("name", "name(The Introductory Stage)", 0).unwrap();
        assert_eq!(s, "The Introductory Stage");
        // empty column -> Integer.valueOf("") throws -> error
        assert!(getint("chk", "set(10,,0,0)", 1).is_err());
    }

    #[test]
    fn parse_stage_1() {
        let text = include_str!("../../../assets/stages/1.txt");
        let stage = parse_stage(text).unwrap();
        assert_eq!(stage.name, "The Introductory Stage");
        assert_eq!(stage.nlaps, 4);

        let mut snaps = 0;
        let mut sky = 0;
        let mut ground = 0;
        let mut fog = 0;
        let mut fadefrom = 0;
        let mut sets = 0;
        let mut set_marker = 0;
        let mut set_variants = 0;
        let mut chks = 0;
        let mut fixes = 0;
        let mut maxr = 0;
        let mut maxl = 0;
        let mut maxt = 0;
        let mut maxb = 0;
        for cmd in &stage.cmds {
            match cmd {
                Cmd::Snap { .. } => snaps += 1,
                Cmd::Sky { .. } => sky += 1,
                Cmd::Ground { .. } => ground += 1,
                Cmd::Fog { .. } => fog += 1,
                Cmd::Fadefrom { .. } => fadefrom += 1,
                Cmd::Set { typ, marker, .. } => {
                    sets += 1;
                    if *marker {
                        set_marker += 1;
                    }
                    if *typ != 0 {
                        set_variants += 1;
                    }
                }
                Cmd::Chk { .. } => chks += 1,
                Cmd::Fix { mid, special, .. } => {
                    fixes += 1;
                    assert_eq!(*mid, 41); // fixpoint
                    assert!(!*special);
                }
                Cmd::MaxRow { side, count, .. } => match side {
                    Side::Right => {
                        maxr += 1;
                        assert_eq!(*count, 10);
                    }
                    Side::Left => {
                        maxl += 1;
                        assert_eq!(*count, 10);
                    }
                    Side::Top => {
                        maxt += 1;
                        assert_eq!(*count, 4);
                    }
                    Side::Bottom => {
                        maxb += 1;
                        assert_eq!(*count, 4);
                    }
                },
            }
        }
        assert_eq!((snaps, sky, ground, fog, fadefrom), (1, 1, 1, 1, 1));
        assert_eq!(sets, 30);
        // 18 )p-gated sets (13 bare )p + 4 )pt + 2 )pr); 6 of them carry a
        // nonzero typ
        assert_eq!(set_marker, 18);
        assert_eq!(set_variants, 6);
        assert_eq!((chks, fixes), (2, 1));
        assert_eq!((maxr, maxl, maxt, maxb), (1, 1, 1, 1));
        assert_eq!(stage.cmds.len(), 42);
    }

    #[test]
    fn model_table_slot_pairing() {
        // loadbase(): entry self-names land in the LAST matching slot
        assert_eq!(model_slot_for_entry("road.rad"), Some(10));
        assert_eq!(model_slot_for_entry("froad.rad"), Some(11));
        assert_eq!(model_slot_for_entry("checkpoint.rad"), Some(40));
        assert_eq!(model_slot_for_entry("offcheckpoint.rad"), Some(42));
        assert_eq!(model_slot_for_entry("formula7.rad"), Some(1));
        assert_eq!(model_slot_for_entry("mystery.rad"), None);
    }
}

/// One parsed stage line, in file order. `loadstage()` effects captured:
/// - `set(mid,x,z,ang)` `+ )p / )pt / )pr / )po / )ph` suffixes (the `)p`
///   substring gate matches the `)pX` variants too — typ overrides run in
///   original order, last write wins).
/// - `chk(mid,x,z,ang)` — checkpoint, typ 1 when ang==0 else 2.
/// - `fix(mid,x,z,y,ang)[+ )s]` — note the swapped y/z: y = column 3,
///   z = column 2 (GameSparker.java:407-412).
/// - `maxr/maxl/maxt/maxb(n, at, off)` — n wall pieces of model 39
///   (`thewall`) + one tall invisible wall tracker each.
#[derive(Debug, Clone)]
pub enum Cmd {
    /// `snap(x,y,z)` — camera offset percentages into `Medium.setsnap`.
    Snap { r: i32, g: i32, b: i32 },
    /// `sky(r,g,b)` → `setsky`.
    Sky { r: i32, g: i32, b: i32 },
    /// `ground(r,g,b)` → `setgrnd`.
    Ground { r: i32, g: i32, b: i32 },
    /// `fog(r,g,b)` → `setfade`.
    Fog { r: i32, g: i32, b: i32 },
    /// `fadefrom(depth)` → `fadfrom` + `origfade = fade[0]`.
    Fadefrom { depth: i32 },
    Set {
        mid: usize,
        x: i32,
        z: i32,
        ang: i32,
        /// The `)p` substring gate — true also for a bare `)p`. The
        /// original registers the checkpoint marker whenever this gate
        /// matches, regardless of the final typ value.
        marker: bool,
        /// 0 for bare `)p` (and plain sets), `)pt` = -1, `)pr` = -2,
        /// `)po` = -3, `)ph` = -4 (chain order as in `loadstage()`).
        typ: i32,
    },
    Chk {
        mid: usize,
        x: i32,
        z: i32,
        ang: i32,
    },
    Fix {
        mid: usize,
        x: i32,
        z: i32,
        y: i32,
        ang: i32,
        special: bool,
    },
    MaxRow {
        side: Side,
        count: i32,
        at: i32,
        off: i32,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Side {
    Right,
    Bottom,
    Top,
    Left,
}

#[derive(Debug, Default)]
pub struct Stage {
    pub name: String,
    pub nlaps: i32,
    pub cmds: Vec<Cmd>,
}

/// Faithful port of the command loop of `loadstage()`
/// (GameSparker.java:310-527) — every line is tested against every prefix
/// like the original (startsWidth chains, no `else`).
pub fn parse_stage(text: &str) -> Result<Stage, StageError> {
    let mut stage = Stage::default();
    let mut nlaps_seen = false;

    for mut line in text.lines() {
        line = line.trim();
        if line.is_empty() {
            continue; // DataInputStream.readLine + trim: empty lines no-op
        }

        if line.starts_with("name") {
            let s = getstring("name", line, 0)?;
            stage.name = s.replace('|', ",");
        }

        if line.starts_with("nlaps") {
            stage.nlaps = getint("nlaps", line, 0)?;
            nlaps_seen = true;
        }

        if line.starts_with("snap") {
            stage.cmds.push(Cmd::Snap {
                r: getint("snap", line, 0)?,
                g: getint("snap", line, 1)?,
                b: getint("snap", line, 2)?,
            });
        }

        if line.starts_with("sky") {
            stage.cmds.push(Cmd::Sky {
                r: getint("sky", line, 0)?,
                g: getint("sky", line, 1)?,
                b: getint("sky", line, 2)?,
            });
        }

        if line.starts_with("ground") {
            stage.cmds.push(Cmd::Ground {
                r: getint("ground", line, 0)?,
                g: getint("ground", line, 1)?,
                b: getint("ground", line, 2)?,
            });
        }

        if line.starts_with("fog") {
            stage.cmds.push(Cmd::Fog {
                r: getint("fog", line, 0)?,
                g: getint("fog", line, 1)?,
                b: getint("fog", line, 2)?,
            });
        }

        if line.starts_with("fadefrom") {
            stage.cmds.push(Cmd::Fadefrom {
                depth: getint("fadefrom", line, 0)?,
            });
        }

        if line.starts_with("set") {
            let mid_i = getint("set", line, 0)?;
            let mid = model_slot_for_index(mid_i)?;
            // )p gate: indexOf(")p") matches the )pX variants as well; a
            // match registers the marker even when a variant never
            // overwrites the typ (plain )p keeps typ 0)
            let mut typ = 0i32;
            if line.contains(")pt") {
                typ = -1;
            }
            if line.contains(")pr") {
                typ = -2;
            }
            if line.contains(")po") {
                typ = -3;
            }
            if line.contains(")ph") {
                typ = -4;
            }
            stage.cmds.push(Cmd::Set {
                mid,
                x: getint("chk", line, 1)?,
                z: getint("chk", line, 2)?,
                ang: getint("chk", line, 3)?,
                marker: line.contains(")p"),
                typ,
            });
        }

        if line.starts_with("chk") {
            stage.cmds.push(Cmd::Chk {
                mid: model_slot_for_index(getint("chk", line, 0)?)?,
                x: getint("chk", line, 1)?,
                z: getint("chk", line, 2)?,
                ang: getint("chk", line, 3)?,
            });
        }

        if line.starts_with("fix") {
            stage.cmds.push(Cmd::Fix {
                mid: model_slot_for_index(getint("fix", line, 0)?)?,
                x: getint("fix", line, 1)?,
                z: getint("fix", line, 2)?,
                y: getint("fix", line, 3)?,
                ang: getint("fix", line, 4)?,
                special: line.contains(")s"),
            });
        }

        let row = if line.starts_with("maxr") {
            Some(Side::Right)
        } else if line.starts_with("maxl") {
            Some(Side::Left)
        } else if line.starts_with("maxt") {
            Some(Side::Top)
        } else if line.starts_with("maxb") {
            Some(Side::Bottom)
        } else {
            None
        };
        if let Some(side) = row {
            stage.cmds.push(Cmd::MaxRow {
                side,
                count: getint(side_key(side), line, 0)?,
                at: getint(side_key(side), line, 1)?,
                off: getint(side_key(side), line, 2)?,
            });
        }

        let _ = nlaps_seen; // (kept for future validation symmetry)
    }

    Ok(stage)
}

fn side_key(side: Side) -> &'static str {
    match side {
        Side::Right => "maxr",
        Side::Left => "maxl",
        Side::Top => "maxt",
        Side::Bottom => "maxb",
    }
}

fn model_slot_for_index(i: i32) -> Result<usize, StageError> {
    if (0..43).contains(&i) {
        Ok(i as usize)
    } else {
        // the original would throw ArrayIndexOutOfBounds into loadstage's
        // catch → "Error in stage N / At line: ..."
        Err(StageError::new(
            String::new(),
            format!("model index {i} out of range"),
        ))
    }
}

/// Car start data (`xtGraphics.java:37-38`): 5 slots, player 1 first.
/// `sc` = selected car model per slot (menus set it; sortcars deals the
/// opponents). These consts initialize `XtGraphics::new`; the placements
/// read the xt fields.
pub const XSTART: [i32; 5] = [0, -350, 350, 0, 0];
pub const ZSTART: [i32; 5] = [-100, 500, 500, 500, 1100];

/// A built stage scene: pool contours in original slot order (slots 0..4 =
/// cars, 5.. = pieces), plus the `notb` bound used by the stage-view draw
/// and the tracker-center the aroundtrack camera orbits.
pub struct Scene {
    pub pool: Vec<Conto>,
    /// `GameSparker.notb`: one past the last `)p`-marked or `chk`/`fix`
    /// piece slot; the stage-view draw only rasterizes pool[5..notb].
    pub notb: usize,
    /// `GameSparker.nob` (GameSparker.java:24): one past the last placed
    /// contour — starts at 5 (the car slots) and advances on every
    /// placement. The race-view draw scans pool[0..nob]; stage-view
    /// registrations move `notb` up inside that same range.
    pub nob: usize,
    /// `Medium.trx/trz` — average of the last 4 wall trackers (the track
    /// center the flyover orbits).
    pub trx: i64,
    pub trz: i64,
}

impl Scene {
    /// Port of the placement half of `loadstage()` (GameSparker.java:311-576,
    /// render-relevant parts only): palette commands were already applied to
    /// `Medium` by the caller (same command order), pieces go into the pool,
    /// wall trackers into `Trackers`, markers into `CheckPoints`. Every
    /// `sky` line also runs `xt.snap(stage)` (GameSparker.java:336-337),
    /// re-tinting the HUD sprites with the palette in effect.
    ///
    /// Pending (documented): `Madness`/`Record` resets, fix-piece
    /// `electrify/fixit` visuals (the `elec` flag is set here and skipped at
    /// draw time in `Conto.d`).
    pub fn build(
        stage: &Stage,
        bases: &[Base],
        m: &mut Medium,
        t: &mut Trackers,
        cp: &mut CheckPoints,
        xt: &mut XtGraphics,
        stage_n: i32,
    ) -> Result<Scene, StageError> {
        t.nt = 0;
        let mut pool: Vec<Conto> = Vec::new();
        let mut nob = 5usize; // slots 0..4 reserved for the cars, filled last
        let mut notb = 0usize;
        cp.n = 0;
        cp.nsp = 0;
        cp.fn_ = 0;
        cp.haltall = false;
        cp.wasted = 0;
        // the `nlaps` line sets the checkpoints' lap budget in place
        // (GameSparker.java:432-434 — `var5.nlaps = getint(...)`; the
        // result screens gate on `nlaps * nsp`). The stage `name` line
        // lands on `checkPoints.name` the same way (GameSparker.java:435).
        cp.nlaps = stage.nlaps;
        cp.name = stage.name.clone();
        m.ground = 250;

        for cmd in &stage.cmds {
            match cmd {
                Cmd::Snap { r, g, b } => m.setsnap(*r, *g, *b),
                Cmd::Sky { r, g, b } => {
                    m.setsky(*r, *g, *b);
                    // GameSparker.java:336-337 — the HUD re-tint after
                    // every sky line.
                    xt.snap(stage_n, m);
                }
                Cmd::Ground { r, g, b } => m.setgrnd(*r, *g, *b),
                Cmd::Fog { r, g, b } => m.setfade(*r, *g, *b),
                Cmd::Fadefrom { depth } => {
                    m.fadfrom(*depth);
                    m.origfade = m.fade[0];
                }
                Cmd::Set {
                    mid,
                    x,
                    z,
                    ang,
                    marker,
                    typ,
                } => {
                    let base = &bases[*mid];
                    let y = m.ground - base.grat;
                    pool.push(Conto::place(base, m, t, *x, y, *z, *ang));
                    // GameSparker.java:357-380 — the )p gate registers the
                    // marker (typ 0 for bare )p), notb = nob + 1 before the
                    // increment = the new nob
                    let n = cp.n as usize;
                    if *marker {
                        cp.x[n] = *x;
                        cp.z[n] = *z;
                        cp.y[n] = 0;
                        cp.typ[n] = *typ;
                        cp.n += 1;
                        notb = nob + 1;
                    }
                    nob += 1;
                }
                Cmd::Chk { mid, x, z, ang } => {
                    let base = &bases[*mid];
                    let y = m.ground - base.grat;
                    pool.push(Conto::place(base, m, t, *x, y, *z, *ang));
                    let n = cp.n as usize;
                    cp.x[n] = *x;
                    cp.z[n] = *z;
                    cp.y[n] = m.ground - base.grat;
                    cp.typ[n] = if *ang == 0 { 1 } else { 2 };
                    cp.pcs = cp.n;
                    cp.n += 1;
                    cp.nsp += 1;
                    nob += 1;
                    notb = nob;
                }
                Cmd::Fix {
                    mid,
                    x,
                    z,
                    y,
                    ang,
                    special,
                } => {
                    let base = &bases[*mid];
                    pool.push(Conto::place(base, m, t, *x, *y, *z, *ang));
                    let c = pool.last_mut().unwrap();
                    c.elec = true; // arcs draw via electrify() in Conto::d
                    if *ang != 0 {
                        cp.roted[cp.fn_ as usize] = true;
                        c.roted = true;
                    }
                    cp.special[cp.fn_ as usize] = *special;
                    cp.fx[cp.fn_ as usize] = *x;
                    cp.fz[cp.fn_ as usize] = *z;
                    cp.fy[cp.fn_ as usize] = *y;
                    cp.fn_ += 1;
                    nob += 1;
                    notb = nob;
                }
                Cmd::MaxRow {
                    side,
                    count,
                    at,
                    off,
                } => {
                    // thewall (slot 39) repeated; wall tracker per branch
                    let base = &bases[39];
                    let y = m.ground - base.grat;
                    for k in 0..*count {
                        let piece = match side {
                            Side::Right | Side::Left => {
                                Conto::place(base, m, t, *at, y, k * 4800 + *off, 0)
                            }
                            Side::Top => Conto::place(base, m, t, k * 4800 + *off, y, *at, 90),
                            Side::Bottom => Conto::place(base, m, t, k * 4800 + *off, y, *at, 90),
                        };
                        pool.push(piece);
                        nob += 1;
                    }
                    let n = t.nt;
                    t.y[n] = -5000;
                    t.rady[n] = 7100;
                    match side {
                        Side::Right => {
                            t.x[n] = *at + 500;
                            t.radx[n] = 600;
                            t.z[n] = *count * 4800 / 2 + *off - 2400;
                            t.radz[n] = *count * 4800 / 2;
                            t.xy[n] = 90;
                            t.zy[n] = 0;
                        }
                        Side::Left => {
                            t.x[n] = *at - 500;
                            t.radx[n] = 600;
                            t.z[n] = *count * 4800 / 2 + *off - 2400;
                            t.radz[n] = *count * 4800 / 2;
                            t.xy[n] = -90;
                            t.zy[n] = 0;
                        }
                        Side::Top => {
                            t.z[n] = *at + 500;
                            t.radz[n] = 600;
                            t.x[n] = *count * 4800 / 2 + *off - 2400;
                            t.radx[n] = *count * 4800 / 2;
                            t.zy[n] = 90;
                            t.xy[n] = 0;
                        }
                        Side::Bottom => {
                            t.z[n] = *at - 500;
                            t.radz[n] = 600;
                            t.x[n] = *count * 4800 / 2 + *off - 2400;
                            t.radx[n] = *count * 4800 / 2;
                            t.zy[n] = -90;
                            t.xy[n] = 0;
                        }
                    }
                    t.dam[n] = 1;
                    t.nt += 1;
                }
            }
        }

        // GameSparker.java:537-558 — track center = average of last 4 wall
        // trackers. It is computed here and STORED on the scene; the
        // medium writes belong to the `fase == 2` promotion (the source's
        // `if (var6.fase == 2)` block), so a pre-promoted `fase` leaves
        // the medium untouched (the loadstage pins).
        let mut trx = 0i64;
        let mut trz = 0i64;
        if t.nt >= 4 {
            for k in 0..4 {
                trx += t.x[t.nt - 4 + k] as i64;
                trz += t.z[t.nt - 4 + k] as i64;
            }
            trx /= 4;
            trz /= 4;
        }

        // GameSparker.java:569-572 — the 5 cars at their start slots,
        // models from xt's sorted roster (sc[0] = the player's pick,
        // sortcars deals the opponents; the menu slice feeds sc[0]).
        let mut cars = Vec::with_capacity(5);
        for i in 0..5 {
            let base = &bases[xt.sc[i] as usize];
            cars.push(Conto::place(
                base,
                m,
                t,
                xt.xstart[i],
                250 - base.grat,
                xt.zstart[i],
                0,
            ));
        }
        let mut all = cars;
        all.extend(pool);
        let nob = all.len();

        Ok(Scene {
            pool: all,
            notb,
            nob,
            trx,
            trz,
        })
    }
}

/// Read one asset file.
///
/// The probe order is build-mode-dependent (the D2a pack contract):
///
/// - **Debug**: the historic filesystem probe — `assets/{tail}` (the CLI's
///   cwd) then `../../assets/{tail}` (the crate dir, the test cwd) — and
///   ONLY if both miss, the embedded resource pack (`assets/{tail}`, see
///   [`crate::pack`]). Without `tools/pack.sh` that archive is empty, so the
///   fallback is a no-op and the pre-pack behavior is unchanged.
/// - **Release**: the pack ONLY. The archive is inside the executable, so a
///   shipped binary reads no resource file from the filesystem; a missing
///   name is a clean `NotFound` error.
///
/// `Some(root)` is the test-only fixture codebase seam (`loadbase_in` /
/// `loadstage_in`); those callers exist only under `cfg(test)`, so a
/// non-test build never resolves assets through it.
fn read_asset_at(root: Option<&str>, tail: &str) -> Result<Vec<u8>, std::io::Error> {
    #[cfg(test)]
    {
        if let Some(root) = root {
            return std::fs::read(format!("{root}/{tail}"));
        }
    }
    #[cfg(not(test))]
    let _ = root;

    #[cfg(debug_assertions)]
    {
        for base in ["assets", "../../assets"] {
            if let Ok(bytes) = std::fs::read(format!("{base}/{tail}")) {
                return Ok(bytes);
            }
        }
        if let Some(bytes) = crate::pack::get(&format!("assets/{tail}")) {
            return Ok(bytes.to_vec());
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("assets/{tail}: not on disk and not in the resource pack"),
        ))
    }
    #[cfg(not(debug_assertions))]
    {
        crate::pack::get(&format!("assets/{tail}"))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("resources.pack: no entry assets/{tail}"),
                )
            })
    }
}

/// Port of `loadbase()` (GameSparker.java:220-241): every `models.zipo`
/// entry parses into the LAST table slot whose name prefixes the entry
/// name, and each filled model becomes a runtime `Base` against a
/// boot-time `Medium`.
///
/// Error strings are the render CLI's originals: a missing archive, an
/// unreadable archive, an entry no table name prefixes, a `rad` parse
/// failure (which aborts the whole load like the source), and leftover
/// unmatched slots.
pub fn loadbase() -> Result<Vec<Base>, String> {
    loadbase_at(None)
}

/// Test-only `loadbase` over an explicit asset root — the mirror's stand-in
/// for the Java class's fixture codebases (an `empty` root proves the
/// missing-archive error arm).
#[cfg(test)]
pub fn loadbase_in(root: &str) -> Result<Vec<Base>, String> {
    loadbase_at(Some(root))
}

fn loadbase_at(root: Option<&str>) -> Result<Vec<Base>, String> {
    let raw = read_asset_at(root, "graphics/models.zipo")
        .map_err(|e| format!("cannot read assets/graphics/models.zipo: {e}"))?;
    let Ok(zipo) = Zipo::read(&raw) else {
        return Err("bad zipo".to_string());
    };

    // loadbase() pairing (GameSparker.java:220-241): entry self-names land
    // in the LAST table slot whose name prefixes the entry name
    let mut slots: Vec<Option<Model>> = (0..MODEL_TABLE.len()).map(|_| None).collect();
    for entry in &zipo.entries {
        let Some(slot) = model_slot_for_entry(&entry.name) else {
            return Err(format!("unmatched model entry {}", entry.name));
        };
        match Model::parse_rad(&entry.name, &entry.data) {
            Ok(m) => slots[slot] = Some(m),
            Err(e) => {
                // a parse failure aborts the original's whole load
                return Err(format!("Error Reading Models: {e}"));
            }
        }
    }
    let unmatched = slots.iter().filter(|s| s.is_none()).count();
    if unmatched != 0 {
        return Err(format!("{unmatched} table slots unmatched"));
    }

    let bases: Vec<Base> = {
        let m = Medium::new(); // boot-time medium, like loadbase()
        slots
            .iter()
            .map(|mm| {
                let mut w = Wheels::new();
                Base::new(mm.as_ref().unwrap(), &m, &mut w)
            })
            .collect()
    };
    Ok(bases)
}

/// Port of the `loadstage()` shell (GameSparker.java:310-330, 531-556 in
/// the catch/tail numbering): `resetstat` runs ahead of the parse so the
/// `sortcars` roster lands before the car placements below read it, then
/// the stage text parses and `Scene::build` runs the placement half.
///
/// On success — and only when `xt.fase == 2` still holds (the catch path
/// below diverts to 3 first) — the tail re-asserts the medium resets
/// `Scene::build` already ran (the track-center average it computed, then
/// `ptr`/`ptcnt`/`hit`/`nrnd`/`trk`) and lands `xt.fase = 1`, mirroring
/// the source's zero-then-average order by restoring the built center.
///
/// The catch path prints the source's three lines (`Error in stage N`,
/// the error, `At line: ...`) and sets `xt.fase = 3` (loadingfailed).
/// The entry resets (`nt`/`nob`/`notb`, the checkpoints, `ground`) ride
/// inside `Scene::build`, so on a failed parse they never apply — the
/// fase-3 menu short-circuits before any race code reads them.
/// The `Control`/`Madness`/`Record` resets after the tail belong to the
/// caller (they need the race-owned holders; see `shell::Shell::frame`).
pub fn loadstage(
    stage_n: i32,
    bases: &[Base],
    m: &mut Medium,
    t: &mut Trackers,
    cp: &mut CheckPoints,
    xt: &mut XtGraphics,
) -> Result<Scene, String> {
    loadstage_at(None, stage_n, bases, m, t, cp, xt)
}

/// Test-only entry point that resolves the stage file against one asset
/// root — the mirror's stand-in for the Java class's fixture codebases
/// (`gfxonly`, which has graphics but no `stages/`, so the load fails into
/// the fase-3 arm exactly like the Java fixture).
#[cfg(test)]
pub fn loadstage_in(
    root: &str,
    stage_n: i32,
    bases: &[Base],
    m: &mut Medium,
    t: &mut Trackers,
    cp: &mut CheckPoints,
    xt: &mut XtGraphics,
) -> Result<Scene, String> {
    loadstage_at(Some(root), stage_n, bases, m, t, cp, xt)
}

fn loadstage_at(
    root: Option<&str>,
    stage_n: i32,
    bases: &[Base],
    m: &mut Medium,
    t: &mut Trackers,
    cp: &mut CheckPoints,
    xt: &mut XtGraphics,
) -> Result<Scene, String> {
    // GameSparker.java:311-320 — the prelude runs BEFORE the file is
    // touched, so a missing stage still leaves these resets behind (the
    // loadstage_missing pins). `view`/`nob`/`notb` are the shell's fields
    // in the port; the builder's own `nob = 5`/`notb = 0` start states are
    // established in `Scene::build`.
    t.nt = 0;
    cp.n = 0;
    cp.nsp = 0;
    cp.fn_ = 0;
    cp.haltall = false;
    cp.wasted = 0;
    m.ground = 250;

    // resetstat hoists ahead of the stage parse so sortcars' roster lands
    // before the car placements below read it — the source parses first,
    // but resetstat shares no state with the parse or the sky-line snaps,
    // so the outcomes line up.
    xt.resetstat(stage_n);

    let stage_path = format!("assets/stages/{stage_n}.txt");
    let raw = match read_asset_at(root, &format!("stages/{stage_n}.txt")) {
        Ok(raw) => raw,
        Err(_) => return Err(fail_read(stage_n, xt, &stage_path)),
    };
    let text = match String::from_utf8(raw) {
        Ok(text) => text,
        Err(_) => return Err(fail_read(stage_n, xt, &stage_path)),
    };
    let stage = match parse_stage(&text) {
        Ok(s) => s,
        // mirrors loadstage()'s catch: "Error in stage N / At line: ..."
        Err(e) => return Err(fail_stage(stage_n, xt, e)),
    };

    let scene = match Scene::build(&stage, bases, m, t, cp, xt, stage_n) {
        Ok(s) => s,
        // same catch arm (the whole placement half sits inside the
        // source's try); the CLI-facing string keeps the render CLI's
        // original build-error text.
        Err(e) => {
            xt.fase = 3;
            println!("Error in stage {stage_n}");
            println!("{e}");
            println!("At line: {}", e.line);
            return Err(format!("scene build failed: {e}"));
        }
    };

    if xt.fase == 2 {
        // GameSparker.java:537-558 — the promotion: the center resets to
        // the builder's average of the last wall trackers, the track state
        // arms, and the screen hands back to fase 1.
        m.trx = 0;
        m.trz = 0;
        m.trx = scene.trx;
        m.trz = scene.trz;
        m.ptr = 0;
        m.ptcnt = -10;
        m.hit = 60000;
        m.nrnd = 0;
        m.trk = true;
        xt.fase = 1;
    }
    Ok(scene)
}

/// GameSparker.java:563-566 — the loadstage tail's Control walk: every
/// rider resets against the freshly-parsed checkpoint table (the source's
/// `do { this.u[var19].reset(var5); } while (++var19 < 5);`). The stage
/// loader cannot reach the shell's Controls, so the caller runs this after
/// a successful parse (the shell's fase-2 arm and the mirror's direct
/// graph both do).
pub fn reset_controls(cp: &CheckPoints, ctl: &mut [Control]) {
    for c in ctl.iter_mut() {
        c.reset(cp);
    }
}

/// The `loadstage()` catch arm: the source's three println lines plus the
/// `fase = 3` divert, returning the CLI-facing error string.
fn fail_stage(stage_n: i32, xt: &mut XtGraphics, e: StageError) -> String {
    xt.fase = 3;
    println!("Error in stage {stage_n}");
    println!("{e}");
    println!("At line: {}", e.line);
    format!("Error in stage {stage_n}\n{e}")
}

/// The same divert for an unreadable stage file (the source opens the
/// stream inside the try, so this lands in the catch too); the CLI-facing
/// string keeps the render CLI's original text.
fn fail_read(stage_n: i32, xt: &mut XtGraphics, stage_path: &str) -> String {
    xt.fase = 3;
    println!("Error in stage {stage_n}");
    println!("cannot read {stage_path}");
    println!("At line: ");
    format!("cannot read {stage_path}")
}
