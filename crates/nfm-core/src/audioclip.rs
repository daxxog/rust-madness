//! `audioclip` — the Sun .au sound-effect voice.
//!
//! Faithful port of the `java.applet.AudioClip` side of `xtGraphics`:
//! the `.au` reader (Sun AU header + μ-law payload), the play/loop/stop
//! state machine, and `getSound` with its JDK-1.3 warm-up quirk
//! (xtGraphics.java:4002-4010).
//!
//! The clips are 8-bit μ-law (verified `encoding == 1` on the fetched
//! assets, 8000 Hz mono); the payload decodes through the same `s2u_lut`
//! table `modslayer` uses — via the table's own pseudo-inverse (the
//! first linear value that re-encodes to the byte, so
//! `encode(decode(b)) == b`; pinned by `decode_round_trips_s2u_lut`).
//! Encoding 2 (8-bit linear) decodes by scaling; anything else yields an
//! empty decode (the assets contain only 1; the branch is defensive).
//!
//! Like `radicalmod`, this models the cursor only: `play()`/`loop_()`
//! arm the flags from the head and `stop()` clears them — the wall-time
//! drain is the real-time shell's. A missing/broken file parses to an
//! empty clip that plays nothing, exactly like the source's silent
//! `getAudioClip` failure (no println anywhere on this path — the
//! source prints none either).

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::audiobus::{AudioBus, VoiceHandle, VoiceMode};
use crate::modslayer::S2U_LUT;

/// The mirror batch's recording seam (`TxtGraphicsLogic`'s `RecMod` /
/// `Clip` test subclasses): a thread-local list of `"tag.verb"` events the
/// production voices append when they are tagged. Untagged voices (every
/// production instance) record nothing, so the hot paths only pay one
/// empty-string check. Test-only: the whole module is `cfg(test)`.
///
/// This is the *observable equivalent* of the Java fixtures' shared
/// `MODLOG`/`AUDIO` lists: the events are emitted by `RadicalMod`'s and
/// `AudioClip`'s own `play`/`resume`/`stop`/`outwithit`/`loop` bodies at
/// the same call sites, so a mirror pin that reads the log is an
/// observation of the real production calls.
#[cfg(test)]
pub(crate) mod verb_log {
    use std::cell::RefCell;

    thread_local! {
        static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    /// Append `tag.verb` (no-op for an untagged voice).
    pub(crate) fn push(tag: &str, verb: &str) {
        if !tag.is_empty() {
            LOG.with(|l| l.borrow_mut().push(format!("{tag}.{verb}")));
        }
    }

    pub(crate) fn clear() {
        LOG.with(|l| l.borrow_mut().clear());
    }

    pub(crate) fn snapshot() -> Vec<String> {
        LOG.with(|l| l.borrow().clone())
    }
}

/// Sun AU header, all big-endian `u32`s (`.snd`, 24 bytes fixed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuHeader {
    /// Data offset in bytes (24 + annotation length; 28 on game files).
    pub data_offset: u32,
    /// Payload length in bytes (`0xFFFFFFFF` = unknown/streamed).
    pub data_size: u32,
    /// 1 = 8-bit μ-law, 2 = 8-bit linear PCM, ...
    pub encoding: u32,
    /// Sample rate in Hz (8000 on game files).
    pub sample_rate: u32,
    pub channels: u32,
}

/// `.au` parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuError {
    TooShort,
    BadMagic { got: u32 },
    Truncated { need: usize, have: usize },
}

impl fmt::Display for AuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuError::TooShort => write!(f, "file too small for an AU header"),
            AuError::BadMagic { got } => write!(f, "bad AU magic {got:#010x}"),
            AuError::Truncated { need, have } => {
                write!(f, "AU payload truncated: need {need} bytes, have {have}")
            }
        }
    }
}

impl std::error::Error for AuError {}

/// The sound-effect voice (`java.applet.AudioClip`).
///
/// `data` holds the raw payload bytes (μ-law for the game assets);
/// `decode()` expands them to linear 16-bit. `pos` is the drain cursor
/// (advanced by the real-time slice headlessly only in tests — the
/// production drain runs through the bus once bound). `playing`/`looping`
/// are `pub` like `RadicalMod.playing`. `handle` is the bus address once
/// `AudioClip::bind_audio` runs; `position()` reads through it when
/// bound so the audio thread's drain is the one true cursor.
#[derive(Debug, Clone)]
pub struct AudioClip {
    pub header: AuHeader,
    pub data: Vec<u8>,
    pub playing: bool,
    pub looping: bool,
    pos: usize,
    pub(crate) handle: Option<VoiceHandle>,
    /// Test-only recording tag (see [`verb_log`]); empty in production.
    #[cfg(test)]
    pub tag: String,
}

impl AudioClip {
    /// Parse one `.au` file image (header + payload).
    pub fn parse(bytes: &[u8]) -> Result<Self, AuError> {
        if bytes.len() < 24 {
            return Err(AuError::TooShort);
        }
        let be = |at: usize| {
            ((bytes[at] as u32) << 24)
                | ((bytes[at + 1] as u32) << 16)
                | ((bytes[at + 2] as u32) << 8)
                | bytes[at + 3] as u32
        };
        let magic = be(0);
        if magic != 0x2e736e64 {
            return Err(AuError::BadMagic { got: magic });
        }
        let header = AuHeader {
            data_offset: be(4),
            data_size: be(8),
            encoding: be(12),
            sample_rate: be(16),
            channels: be(20),
        };
        let start = header.data_offset as usize;
        // `data_size == 0xFFFFFFFF` means "unknown" (streamed .au); take
        // everything past the offset then.
        let end = if header.data_size == 0xFFFFFFFF {
            bytes.len()
        } else {
            start.saturating_add(header.data_size as usize)
        };
        if start > bytes.len() || end > bytes.len() {
            return Err(AuError::Truncated {
                need: end.saturating_sub(bytes.len()),
                have: bytes.len(),
            });
        }
        Ok(Self {
            header,
            data: bytes[start..end].to_vec(),
            playing: false,
            looping: false,
            pos: 0,
            handle: None,
            #[cfg(test)]
            tag: String::new(),
        })
    }

    /// The degenerate clip a missing/broken file degrades to: empty,
    /// silent, flaggable (the source's broken `AudioClip`).
    pub fn empty() -> Self {
        Self {
            header: AuHeader {
                data_offset: 24,
                data_size: 0,
                encoding: 1,
                sample_rate: 8000,
                channels: 1,
            },
            data: Vec::new(),
            playing: false,
            looping: false,
            pos: 0,
            handle: None,
            #[cfg(test)]
            tag: String::new(),
        }
    }

    /// `play()` — one-shot from the head. Headless, this restarts the
    /// local cursor. Bound, this goes through the bus overlap path
    /// (`trigger_or_restart`): a stopped primary arms in place, while a
    /// still-sounding one stacks another copy instead of truncating it.
    /// The JDK's `AudioPlayer` spawns an overlapping player per `play()`,
    /// which the powerup wash (re-played every frame while `auscnt < 45`)
    /// and crash-grind retriggers (GameSparker.java:860-874, every 2–3
    /// frames vs ~1.5 s clips) rely on for density.
    pub fn play(&mut self) {
        #[cfg(test)]
        verb_log::push(&self.tag, "play");
        self.pos = 0;
        self.playing = true;
        self.looping = false;
        if let Some(h) = &self.handle
            && let Ok(mut bus) = h.bus.lock()
        {
            bus.trigger_or_restart(h.index, Arc::clone(&h.data), self.header.sample_rate);
        }
    }

    /// `loop()` (`loop` is a keyword — transcribed as `loop_`) — from the
    /// head, wrapping (the `SuperStream`-style wrap lives in the drain).
    /// Forwards to the bus voice when bound (cursor reset, Loop arm).
    pub fn loop_(&mut self) {
        #[cfg(test)]
        verb_log::push(&self.tag, "loop");
        self.pos = 0;
        self.playing = true;
        self.looping = true;
        if let Some(h) = &self.handle {
            h.with_voice(|v| {
                v.cursor = 0.0;
                v.playing = true;
                v.mode = VoiceMode::Loop;
            });
        }
    }

    /// `stop()` — clears both flags, on the bus voice too when bound.
    pub fn stop(&mut self) {
        #[cfg(test)]
        verb_log::push(&self.tag, "stop");
        self.playing = false;
        self.looping = false;
        if let Some(h) = &self.handle {
            h.with_voice(|v| {
                v.playing = false;
            });
        }
    }

    /// Bind this voice's payload to the bus: registers the bytes (an
    /// `Arc` clone, once) at the clip's native rate and current play
    /// state, and stores the handle future `play`/`loop_`/`stop` calls
    /// forward through. Idempotent (a bound voice keeps its handle);
    /// empty payloads are skipped (nothing to mix). The bus cursor
    /// starts at the local `pos` so binding never jumps `position()`.
    pub(crate) fn bind_audio(&mut self, bus: &Arc<Mutex<AudioBus>>) {
        if self.handle.is_some() || self.data.is_empty() {
            return;
        }
        let Ok(mut locked) = bus.lock() else {
            return;
        };
        let shared: Arc<Vec<i8>> = Arc::new(self.data.iter().map(|&b| b as i8).collect());
        let index = locked.register(
            Arc::clone(&shared),
            self.header.sample_rate,
            self.pos as f64,
            if self.looping {
                VoiceMode::Loop
            } else {
                VoiceMode::OneShot
            },
            self.playing,
        );
        self.handle = Some(VoiceHandle {
            bus: Arc::clone(bus),
            index,
            data: shared,
        });
    }

    /// The drain position in payload bytes: the bus cursor when bound,
    /// the local `pos` headlessly. Public like `posit()`: the device
    /// backend reads it from outside the crate.
    pub fn position(&self) -> usize {
        if let Some(h) = &self.handle
            && let Some(pos) = h.with_voice(|v| (v.cursor.floor() as usize).min(v.data.len()))
        {
            return pos;
        }
        self.pos
    }

    /// Expand the payload to linear 16-bit samples.
    pub fn decode(&self) -> Vec<i16> {
        match self.header.encoding {
            1 => self.data.iter().map(|&b| decode_ulaw(b)).collect(),
            2 => self.data.iter().map(|&b| ((b as i8) as i16) << 8).collect(),
            _ => Vec::new(),
        }
    }
}

/// Expand one μ-law byte through `S2U_LUT`: the first linear value whose
/// table entry re-encodes to `b` (so `linear2ulawclip(decode(b)) == b`).
/// `pub(crate)` for the mix bus ([`crate::audiobus`]), which decodes
/// every pulled voice through it.
pub(crate) fn decode_ulaw(b: u8) -> i16 {
    use std::sync::OnceLock;
    static INVERSE: OnceLock<[i16; 256]> = OnceLock::new();
    let inv = INVERSE.get_or_init(|| {
        let mut table = [0i16; 256];
        let mut done = [false; 256];
        let mut remaining = 256;
        for (k, &entry) in S2U_LUT.iter().enumerate() {
            let idx = entry as u8 as usize;
            if !done[idx] {
                // `k` indexes `(v >> 3) & 8191`: `k < 4096` is the
                // non-negative half (`v = k << 3`), the rest the
                // negative half (`v = (k - 8192) << 3`).
                table[idx] = if k < 4096 {
                    (k << 3) as i16
                } else {
                    ((k as i32 - 8192) << 3) as i16
                };
                done[idx] = true;
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
        }
        table
    });
    inv[b as usize]
}

/// `getSound` (xtGraphics.java:4002-4010): `Applet.getAudioClip` over the
/// asset roots, with the JDK-1.3 warm-up quirk — `default/`-prefixed
/// clips are `play()`ed, yielded, then `stop()`ed so the first real play
/// doesn't hitch (the yield is a no-op headlessly). Missing/broken files
/// degrade to [`AudioClip::empty()`] (the source's silent failure).
pub fn get_sound(radical_path: &str) -> AudioClip {
    let bytes = sound_bytes(radical_path);
    let mut clip = bytes
        .as_deref()
        .map(AudioClip::parse)
        .and_then(Result::ok)
        .unwrap_or_else(AudioClip::empty);
    if radical_path.starts_with("sounds/default") {
        clip.play();
        clip.stop();
    }
    clip
}

/// The codebase probe for one clip path (`assets/{radical_path}` — the
/// pack's canonical name): debug builds keep the historic `assets` →
/// `../../assets` filesystem probe and fall back to the embedded pack;
/// release builds read the pack only. See `pack.rs` for the mapping.
fn sound_bytes(radical_path: &str) -> Option<Vec<u8>> {
    #[cfg(debug_assertions)]
    {
        for base in ["assets", "../../assets"] {
            if let Ok(bytes) = std::fs::read(format!("{base}/{radical_path}")) {
                return Some(bytes);
            }
        }
        crate::pack::get(&format!("assets/{radical_path}")).map(<[u8]>::to_vec)
    }
    #[cfg(not(debug_assertions))]
    {
        crate::pack::get(&format!("assets/{radical_path}")).map(<[u8]>::to_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::md5_hex;
    use crate::modslayer::linear2ulawclip;

    fn newsun(name: &str) -> Vec<u8> {
        let tail = format!("assets/sounds/newsun/{name}");
        ["", "../../"]
            .iter()
            .find_map(|base| std::fs::read(format!("{base}{tail}")).ok())
            .unwrap_or_else(|| panic!("missing asset {tail}"))
    }

    #[test]
    fn au_header_parse_pins_against_the_jdk_dump() {
        // Scratch `AuDump.java` reads the same file raw and md5s the
        // payload past the header; the port pins the header fields and
        // the payload bytes identically.
        let raw = newsun("a0.au");
        let clip = AudioClip::parse(&raw).expect("a0.au must parse");
        assert_eq!(
            clip.header,
            AuHeader {
                data_offset: 28,
                data_size: 1495,
                encoding: 1,
                sample_rate: 8000,
                channels: 1,
            }
        );
        assert_eq!(clip.data.len(), 1495);
        assert_eq!(md5_hex(&clip.data), "e3097523952dc256477690370bc8fe37");
        assert!(!clip.playing && !clip.looping);
    }

    #[test]
    fn decode_round_trips_s2u_lut() {
        // Every byte the table emits re-encodes to itself through the
        // table's own pseudo-inverse ...
        let mut missing = Vec::new();
        for b in 0..=255u8 {
            if linear2ulawclip(decode_ulaw(b) as i32) as u8 != b {
                missing.push(b);
            }
        }
        // ... except 0x7F, which has no preimage in s2u_lut (verified
        // against the JDK dump: the table image is the other 255 bytes)
        // and decodes to silence, like the textbook expansion.
        assert_eq!(missing, [0x7F]);
        assert_eq!(decode_ulaw(0x7F), 0);
    }

    #[test]
    fn state_machine_play_loop_stop() {
        let mut c = AudioClip::empty();
        c.play();
        assert!(c.playing && !c.looping);
        c.loop_();
        assert!(c.playing && c.looping);
        c.stop();
        assert!(!c.playing && !c.looping);
    }

    #[test]
    fn getsound_default_variant_warms_up_then_stops() {
        // The xt:4002-4010 quirk on a missing default/ file: the warm-up
        // plays then stops, leaving a silent, unflagged clip.
        let c = get_sound("sounds/default/a0.au");
        assert!(c.data.is_empty());
        assert!(!c.playing && !c.looping);
    }

    #[test]
    fn getsound_loads_the_real_newsun_clip() {
        let c = get_sound("sounds/newsun/a0.au");
        assert_eq!(c.data.len(), 1495);
        assert!(!c.playing, "no warm-up outside sounds/default");

        // a second voice pins too (countdown clip, payload md5 from AuDump)
        let raw = newsun("three.au");
        let three = AudioClip::parse(&raw).expect("three.au must parse");
        assert_eq!(three.data.len(), 12103);
        assert_eq!(md5_hex(&three.data), "a42d10932d31e119ffc2aa1f17c46ea8");
    }
}
