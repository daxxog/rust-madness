//! `tracker` — ProTracker module parser: `Mod` plus its records.
//!
//! Faithful port of `reference/vineflower/Mod.java` (201 lines),
//! `ModInstrument.java` and `ModTrackInfo.java` (small records).
//!
//! Zip handling: the Java `Mod(String, Applet)` constructor (Mod.java:40-62)
//! fetches the `.zipo` URL and feeds the FIRST zip entry to `LoadMod` through
//! a plain `ZipInputStream` — notably NOT the custom zipo index, just the
//! archive's first member. The music zips are ordinary single-entry deflate
//! zips with central-directory sizes present, so `crate::zipo::Zipo::read`
//! parses them as-is and `entries[0].data` is exactly the module image (pinned
//! by `music_zips_are_plain_zips_with_mod_first_entry`). The port keeps
//! both seams: [`Mod::from_zipo`] is the constructor's container-first-entry
//! path (failures swallowed into the broken object, like Mod.java:59-61),
//! and [`Mod::load_mod`] / [`Mod::load_mod_into`] are `LoadMod` over the raw
//! module bytes.

use std::fmt;

/// Parse failure for [`Mod::load_mod`].
///
/// Java reports these as `IOException`/`EOFException`; the one special case
/// is truncated sample data, which `LoadMod` (Mod.java:138-144) catches and
/// turns into a printed warning with partially-filled sample arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModError {
    /// Fewer bytes remain than the header/pattern structure needs.
    Truncated { at: usize, need: usize },
    /// Sample data ran out mid-file (caught by the caller per Mod.java:142).
    TruncatedSamples,
}

impl fmt::Display for ModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModError::Truncated { at, need } => {
                write!(f, "mod truncated at byte {at} (need {need} more)")
            }
            ModError::TruncatedSamples => write!(f, "EOF on MOD file"),
        }
    }
}

impl std::error::Error for ModError {}

/// Big-endian cursor over the module image (stands in for Java's
/// `DataInputStream`; `take` is `readFully`, which throws on truncation).
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ModError> {
        let at = self.pos;
        let end = at
            .checked_add(n)
            .ok_or(ModError::Truncated { at, need: n })?;
        if end > self.buf.len() {
            return Err(ModError::Truncated { at, need: n });
        }
        self.pos = end;
        Ok(&self.buf[at..end])
    }

    /// `DataInputStream.readByte() & 0xFF` (Mod.java:198-200).
    fn read_u8(&mut self) -> Result<i32, ModError> {
        Ok(self.take(1)?[0] as i32)
    }

    /// `DataInputStream.readShort() & 0xFFFF`, big-endian (Mod.java:64-66).
    fn read_u16(&mut self) -> Result<i32, ModError> {
        let b = self.take(2)?;
        Ok((((b[0] as i32) << 8) | b[1] as i32) & 65535)
    }

    fn read_i32_be(&mut self) -> Result<i32, ModError> {
        let b = self.take(4)?;
        Ok(((b[0] as i32) << 24) | ((b[1] as i32) << 16) | ((b[2] as i32) << 8) | b[3] as i32)
    }

    fn skip(&mut self, n: usize) -> Result<(), ModError> {
        self.take(n).map(|_| ())
    }

    /// `readText`: fixed-width Latin-1 string, trailing NULs stripped by
    /// scanning from the end for the last nonzero byte (Mod.java:68-79).
    fn read_text(&mut self, n: usize) -> Result<String, ModError> {
        let b = self.take(n)?;
        let mut len = 0;
        for (i, &byte) in b.iter().enumerate() {
            if byte != 0 {
                len = i + 1;
            }
        }
        Ok(b[..len].iter().map(|&c| c as char).collect())
    }
}

/// One instrument/sample header + its (signed 8-bit) PCM.
///
/// `ModInstrument.java` (12 lines). `finetune_rate`, `period_low_limit` and
/// `period_high_limit` are filled in later by `ModSlayer::startplaying`;
/// `name` is never read back (kept for record shape).
#[allow(dead_code)] // the never-read `name` field (record shape); the cfg(test) mirror pins the rest
#[derive(Debug, Clone)]
pub struct ModInstrument {
    pub(crate) name: String,
    pub(crate) samples: Vec<i8>,
    pub(crate) sample_length: i32,
    pub(crate) finetune_rate: i32,
    pub(crate) period_low_limit: i32,
    pub(crate) period_high_limit: i32,
    pub(crate) finetune_value: i32,
    pub(crate) volume: i32,
    pub(crate) repeat_point: i32,
    pub(crate) repeat_length: i32,
}

impl ModInstrument {
    /// The implicit default constructor (`ModInstrument.java:1-12`): every
    /// field keeps its declared initializer. Java's `null` array/name
    /// become the port's empty `Vec`/`String` (the campaign's established
    /// null stand-in — see `t_modslayer`'s `vol_table` note).
    #[allow(dead_code)] // pinned only by the cfg(test) mirror (TSmall builds the Java default)
    pub(crate) fn new() -> Self {
        Self {
            name: String::new(),
            samples: Vec::new(),
            sample_length: 0,
            finetune_rate: 0,
            period_low_limit: 0,
            period_high_limit: 0,
            finetune_value: 0,
            volume: 0,
            repeat_point: 0,
            repeat_length: 0,
        }
    }
}

/// Per-channel live playback state.
///
/// `ModTrackInfo.java` (38 lines). Several fields are write-only or never
/// touched by the engine (`old_position`, `step`, `trempos`, `oldperiod`,
/// `trem_rate`, `trem_depth`, `retrig` — tremolo/retrig effects are
/// unimplemented stubs in `get_track` — and `start_period`); they are kept
/// so the record matches the Java shape.
#[allow(dead_code)] // the never-read fields above (unimplemented Java effects); the cfg(test) mirror pins the record
#[derive(Debug, Clone)]
pub struct ModTrackInfo {
    pub(crate) samples: Vec<i8>,
    pub(crate) position: i32,
    pub(crate) length: i32,
    pub(crate) repeat: i32,
    pub(crate) replen: i32,
    pub(crate) volume: i32,
    pub(crate) error: i32,
    pub(crate) pitch: i32,
    pub(crate) old_position: i32,
    pub(crate) start_period: i32,
    pub(crate) period: i32,
    pub(crate) step: i32,
    pub(crate) effect: i32,
    pub(crate) portto: i32,
    pub(crate) vibpos: i32,
    pub(crate) trempos: i32,
    pub(crate) oldsampofs: i32,
    pub(crate) arp: [i32; 3],
    pub(crate) arpindex: i32,
    pub(crate) oldperiod: i32,
    pub(crate) vol_slide: i32,
    pub(crate) port_inc: i32,
    pub(crate) port_up: i32,
    pub(crate) port_down: i32,
    pub(crate) vib_rate: i32,
    pub(crate) vib_depth: i32,
    pub(crate) trem_rate: i32,
    pub(crate) trem_depth: i32,
    pub(crate) retrig: i8,
    pub(crate) finetune_rate: i32,
    pub(crate) period_low_limit: i32,
    pub(crate) period_high_limit: i32,
}

impl ModTrackInfo {
    pub(crate) fn new() -> Self {
        Self {
            samples: Vec::new(),
            position: 0,
            length: 0,
            repeat: 0,
            replen: 0,
            volume: 0,
            error: 0,
            pitch: 0,
            old_position: 0,
            start_period: 0,
            period: 0,
            step: 0,
            effect: 0,
            portto: 0,
            vibpos: 0,
            trempos: 0,
            oldsampofs: 0,
            arp: [0; 3],
            arpindex: 0,
            oldperiod: 1,
            vol_slide: 0,
            port_inc: 0,
            port_up: 0,
            port_down: 0,
            vib_rate: 0,
            vib_depth: 0,
            trem_rate: 0,
            trem_depth: 0,
            retrig: 0,
            finetune_rate: 0,
            period_low_limit: 0,
            period_high_limit: 0,
        }
    }
}

/// Parsed ProTracker module (`Mod.java`).
///
/// `track_shift` is never read or written by the Java engine (stays 0);
/// kept for record shape. `s3m` is never set by `LoadMod` (always false for
/// these files) but selects the finetune branch in the mixer.
#[allow(dead_code)] // only `track_shift` is never read (Java record shape)
#[derive(Debug, Clone)]
pub struct Mod {
    pub(crate) name: String,
    pub(crate) numtracks: i32,
    pub(crate) track_shift: i32,
    pub(crate) numpatterns: i32,
    pub(crate) patterns: Vec<Vec<i8>>,
    pub(crate) insts: Vec<ModInstrument>,
    pub(crate) positions: Vec<i8>,
    pub(crate) song_length_patterns: i32,
    pub(crate) song_repeat_patterns: i32,
    pub(crate) s3m: bool,
    /// Java's `insts == null`: true on the object a failed
    /// `Mod(String, Applet)` leaves behind (Mod.java:40-62 catches every
    /// exception), until a successful `LoadMod` fills the fields. The
    /// readers keep returning the default field values; only `toString`
    /// dereferences `insts.length` (:37) and therefore NPEs.
    broken: bool,
}

impl Mod {
    /// The object a failed `Mod(String, Applet)` leaves behind
    /// (Mod.java:40-62): every field at its Java default — `numtracks` and
    /// `numpatterns` 0, `patterns`/`insts`/`positions` null (`broken` holds
    /// that null-ness for `insts`; [`Mod::describe`] is the only deref).
    pub(crate) fn broken() -> Mod {
        Mod {
            name: String::new(),
            numtracks: 0,
            track_shift: 0,
            numpatterns: 0,
            patterns: Vec::new(),
            insts: Vec::new(),
            positions: Vec::new(),
            song_length_patterns: 0,
            song_repeat_patterns: 0,
            s3m: false,
            broken: true,
        }
    }

    /// `Mod(String, Applet)` (Mod.java:40-62): fetch the `.zipo` container,
    /// take its FIRST zip entry and `LoadMod` those bytes. `None` stands in
    /// for the constructor's failed `new URL(...).openStream()` (a missing
    /// file); both that throw and a parse throw are swallowed at :59-61,
    /// leaving the broken/partially-parsed object behind.
    pub fn from_zipo(container: Option<&[u8]>) -> Mod {
        let mut m = Mod::broken();
        if let Some(entry) = container.and_then(first_zipo_entry) {
            // The catch at Mod.java:59-61: a throw is a print, never a
            // propagation. `load_mod_into` leaves the partial state.
            let _ = m.load_mod_into(&entry);
        }
        m
    }

    /// `LoadMod` (Mod.java:101-147): parse a module image (the first entry
    /// of the `.zipo`, already inflated by the caller).
    pub fn load_mod(bytes: &[u8]) -> Result<Mod, ModError> {
        let mut m = Mod::broken();
        m.load_mod_into(bytes)?;
        Ok(m)
    }

    /// `LoadMod(InputStream)` onto an existing object (Mod.java:101-147) —
    /// the public seam: raw module bytes, no zip wrapper, every field reset.
    /// Java mutates in place, so a throw (`EOFException`) leaves the
    /// partially parsed fields behind; this does the same (the pin only
    /// asserts the throw).
    pub fn load_mod_into(&mut self, bytes: &[u8]) -> Result<(), ModError> {
        *self = Mod::broken();
        let mut cur = Cursor { buf: bytes, pos: 0 };
        self.numtracks = 4; // Mod.java:104
        self.name = cur.read_text(20)?; // Mod.java:105

        // mark(1068); skip(1060); readInt; reset (Mod.java:106-109): peek the
        // 4-byte format tag at offset 20 + 1060, then rewind to offset 20.
        let after_name = cur.pos;
        cur.skip(1060)?;
        let magic = cur.read_i32_be()?;
        cur.pos = after_name;

        let mut var3: i32 = 15; // Mod.java:103 (`byte var3 = 15`)
        for v in VOICE_31_LIST {
            if magic == v {
                var3 = 31;
                break;
            }
        }

        if var3 == 31 {
            if magic == VOICE_8CHN {
                self.numtracks = 8;
            } else if magic == VOICE_6CHN {
                self.numtracks = 6;
            } else if magic == VOICE_28CH {
                self.numtracks = 28;
            }
        }

        self.insts.reserve(var3 as usize);
        for _ in 0..var3 {
            self.insts.push(read_instrument(&mut cur)?); // Mod.java:130-132
        }

        read_sequence(&mut cur, self)?; // Mod.java:134
        cur.skip(4)?; // skipBytes(4): the format tag itself (Mod.java:135)
        read_patterns(&mut cur, self)?; // Mod.java:136

        // Sample bodies; a short file ends the loop with a warning, keeping
        // whatever bytes (if any) were already read (Mod.java:138-144).
        for inst in self.insts.iter_mut() {
            match read_sample_data(&mut cur, inst) {
                Ok(()) => {}
                Err(ModError::TruncatedSamples) => {
                    println!("Warning: EOF on MOD file");
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        self.broken = false;
        Ok(())
    }

    /// `getName` (Mod.java:184-186).
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// `getNumTracks` (Mod.java:180-182).
    pub fn get_num_tracks(&self) -> i32 {
        self.numtracks
    }

    /// `getNumPatterns` (Mod.java:32-34).
    pub fn num_patterns(&self) -> i32 {
        self.numpatterns
    }

    /// `toString` (Mod.java:36-38).
    ///
    /// Java concatenates `this.insts.length` on the (possibly null) array a
    /// failed constructor leaves behind — the original never guards it, so
    /// this panics with the NPE the Java throws.
    pub fn describe(&self) -> String {
        assert!(
            !self.broken,
            "NullPointerException: Mod.insts is null (Mod.java:37)"
        );
        format!(
            "{} ({} tracks, {} patterns, {} samples)",
            self.name,
            self.numtracks,
            self.numpatterns,
            self.insts.len()
        )
    }
}

impl fmt::Display for Mod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

/// The constructor's zip step (Mod.java:43-46): open the container and take
/// the FIRST `ZipInputStream` entry's inflated bytes. `None` when the file
/// cannot be opened as a zip (`openStream`/`getNextEntry` throwing).
fn first_zipo_entry(raw: &[u8]) -> Option<Vec<u8>> {
    let z = crate::zipo::Zipo::read(raw).ok()?;
    z.entries.into_iter().next().map(|e| e.data)
}

/// `FOURCC` (Mod.java:176-178): four ASCII chars, big-endian.
pub fn fourcc(code: [u8; 4]) -> i32 {
    ((code[0] as i32) << 24) | ((code[1] as i32) << 16) | ((code[2] as i32) << 8) | code[3] as i32
}

const VOICE_MK: i32 = 0x4D2E4B2E; // FOURCC("M.K.")
const VOICE_MK2: i32 = 0x4D214B21; // FOURCC("M!K!")
const VOICE_MK3: i32 = 0x4D264B21; // FOURCC("M&K!")
const VOICE_FLT4: i32 = 0x464C5434; // FOURCC("FLT4")
const VOICE_FLT8: i32 = 0x464C5438; // FOURCC("FLT8")
const VOICE_28CH: i32 = 0x32384348; // FOURCC("28CH")
const VOICE_8CHN: i32 = 0x3843484E; // FOURCC("8CHN")
const VOICE_6CHN: i32 = 0x3643484E; // FOURCC("6CHN")

/// `voice_31_list` (Mod.java:30).
const VOICE_31_LIST: [i32; 8] = [
    VOICE_MK, VOICE_MK2, VOICE_MK3, VOICE_FLT4, VOICE_FLT8, VOICE_8CHN, VOICE_6CHN, VOICE_28CH,
];

/// `readSequence` (Mod.java:81-99). Position bytes compare SIGNED against the
/// running max (Java `byte` widens to `int`).
fn read_sequence(cur: &mut Cursor, m: &mut Mod) -> Result<(), ModError> {
    m.positions = vec![0i8; 128];
    m.song_length_patterns = cur.read_u8()?;
    m.song_repeat_patterns = cur.read_u8()?;
    let raw = cur.take(128)?;
    for (i, &b) in raw.iter().enumerate() {
        m.positions[i] = b as i8;
    }
    if m.song_repeat_patterns > m.song_length_patterns {
        m.song_repeat_patterns = m.song_length_patterns;
    }

    m.numpatterns = 0;
    for &p in m.positions.iter() {
        if (p as i32) > m.numpatterns {
            m.numpatterns = p as i32;
        }
    }
    m.numpatterns += 1;
    Ok(())
}

/// `readPatterns` (Mod.java:188-196).
fn read_patterns(cur: &mut Cursor, m: &mut Mod) -> Result<(), ModError> {
    let row_bytes = (m.numtracks * 4 * 64) as usize;
    m.patterns.clear();
    for _ in 0..m.numpatterns {
        let raw = cur.take(row_bytes)?;
        m.patterns.push(raw.iter().map(|&b| b as i8).collect());
    }
    Ok(())
}

/// `readInstrument` (Mod.java:156-174).
fn read_instrument(cur: &mut Cursor) -> Result<ModInstrument, ModError> {
    let name = cur.read_text(22)?;
    let sample_length = cur.read_u16()? << 1;
    // `(byte)(readu8 << 4)`: keep the low 8 bits, then sign-extend
    // (Mod.java:161).
    let finetune_value = (((cur.read_u8()? << 4) & 0xFF) as i8) as i32;
    let volume = cur.read_u8()?;
    let mut repeat_point = cur.read_u16()? << 1;
    let mut repeat_length = cur.read_u16()? << 1;
    if repeat_point > sample_length {
        repeat_point = sample_length;
    }
    if repeat_point + repeat_length > sample_length {
        repeat_length = sample_length - repeat_point;
    }
    Ok(ModInstrument {
        name,
        samples: vec![0i8; (sample_length as usize) + 8],
        sample_length,
        finetune_rate: 0,
        period_low_limit: 0,
        period_high_limit: 0,
        finetune_value,
        volume,
        repeat_point,
        repeat_length,
    })
}

/// `readSampleData` (Mod.java:149-153).
///
/// `readFully` copies whatever bytes remain and THEN throws on a short file,
/// so a truncated tail leaves a partial body behind; that throw is what the
/// caller catches (`TruncatedSamples`).
fn read_sample_data(cur: &mut Cursor, inst: &mut ModInstrument) -> Result<(), ModError> {
    let len = inst.sample_length as usize;
    let avail = cur.remaining().min(len);
    for (i, &b) in cur.buf[cur.pos..cur.pos + avail].iter().enumerate() {
        inst.samples[i] = b as i8;
    }
    cur.pos += avail;
    if avail < len {
        return Err(ModError::TruncatedSamples);
    }
    if inst.repeat_length > 3 {
        // System.arraycopy(samples, repeat_point, samples, sample_length, 8)
        let rp = inst.repeat_point as usize;
        let sl = len;
        inst.samples.copy_within(rp..rp + 8, sl);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zipo::Zipo;

    fn stage1_mod_bytes() -> Vec<u8> {
        let raw: &[u8] = include_bytes!("../../../assets/music/stage1.zipo");
        let z = Zipo::read(raw).expect("stage1.zipo must parse");
        assert_eq!(z.entries.len(), 1);
        z.entries.into_iter().next().unwrap().data
    }

    #[test]
    fn music_zips_are_plain_zips_with_mod_first_entry() {
        // The Java ctor reads the first ZipInputStream entry; Zipo::read
        // must yield that same member as-is (no custom-index divergence).
        let raw: &[u8] = include_bytes!("../../../assets/music/stage1.zipo");
        let z = Zipo::read(raw).expect("music zipo must parse with Zipo::read");
        assert_eq!(z.entries.len(), 1);
        assert_eq!(z.entries[0].name, "stage1.mod");
        assert_eq!(z.entries[0].data.len(), 69306);

        let raw2: &[u8] = include_bytes!("../../../assets/music/stage2.zipo");
        let z2 = Zipo::read(raw2).expect("music zipo must parse with Zipo::read");
        assert_eq!(z2.entries.len(), 1);
        assert_eq!(z2.entries[0].name, "stage2.mod");
        assert_eq!(z2.entries[0].data.len(), 230250);
    }

    #[test]
    fn fourcc_matches_java_static() {
        assert_eq!(fourcc(*b"M.K."), VOICE_MK);
        assert_eq!(fourcc(*b"M!K!"), VOICE_MK2);
        assert_eq!(fourcc(*b"FLT4"), VOICE_FLT4);
        assert_eq!(fourcc(*b"28CH"), VOICE_28CH);
    }

    #[test]
    fn load_stage1_metadata() {
        let m = Mod::load_mod(&stage1_mod_bytes()).expect("stage1.mod must parse");
        assert_eq!(m.get_name(), "Stage1");
        assert_eq!(m.get_num_tracks(), 4);
        assert_eq!(m.num_patterns(), 20);
        assert_eq!(m.insts.len(), 31);
        assert_eq!(m.describe(), "Stage1 (4 tracks, 20 patterns, 31 samples)");
    }

    #[test]
    fn display_impl_matches_describe() {
        // Verify Display impl writes the same as describe()
        let m = Mod::load_mod(&stage1_mod_bytes()).expect("stage1.mod must parse");
        assert_eq!(format!("{}", m), m.describe());
    }

    #[test]
    fn load_stage2_metadata() {
        let raw: &[u8] = include_bytes!("../../../assets/music/stage2.zipo");
        let z = Zipo::read(raw).expect("stage2.zipo must parse");
        let m = Mod::load_mod(&z.entries[0].data).expect("stage2.mod must parse");
        assert_eq!(m.get_name(), "Madness stage2");
        assert_eq!(m.get_num_tracks(), 4);
        assert_eq!(m.num_patterns(), 44);
        assert_eq!(m.insts.len(), 31);
        assert_eq!(
            m.describe(),
            "Madness stage2 (4 tracks, 44 patterns, 31 samples)"
        );
    }
}
