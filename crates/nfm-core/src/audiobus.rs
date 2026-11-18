//! `audiobus` — the dependency-free software mix bus the device backend
//! pulls from.
//!
//! The JDK model is one background player thread per started stream,
//! each draining its own bytes in wall time while the OS mixes the
//! concurrent voices. The port inverts that: a single bus owns every
//! voice cursor, and the device thread pulls mixed output through
//! [`AudioBus::pull`]. Everything here is std-only (`Arc`/`Mutex`);
//! the device backend (which owns the output stream) lands next round.
//!
//! Single source of truth: `VoiceHandle` (`{ bus, index }`, cloned
//! into the voice on bind). Un-wired voices keep today's local cursors
//! exactly (all headless tests pass unchanged); once bound,
//! `play`/`resume`/`stop` forward to the bus voice and `posit()` reads
//! the bus cursor, so the audio thread's drain is the one true cursor.
//! There is no unbind path — `stopallnow` silences bound voices in place
//! and re-`play` re-arms them.
//!
//! Bind timing note: voices bind per voice the first time the shell sees
//! them unbound while a host drives audio (the handle's presence IS the
//! latch — deliberately not a shell-wide once-flag, so stage tracks
//! built later by `loadmusic` still join the mix instead of staying
//! silent on the device).

use std::sync::{Arc, Mutex};

use crate::audioclip::decode_ulaw;
use crate::host::AudioSink;

/// How a voice ends: one-shot clips stop at the data end, music and
/// looped clips wrap (mirroring `SuperStream`'s wrap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceMode {
    OneShot,
    Loop,
}

/// One mixed voice. `data` holds the μ-law bytes (shared `Arc`, never
/// copied per pull); `cursor` is the fractional position in `data`
/// advanced by the pull; `playing` gates mixing.
#[derive(Debug, Clone)]
pub struct Voice {
    pub data: Arc<Vec<i8>>,
    pub rate: u32,
    pub cursor: f64,
    pub mode: VoiceMode,
    pub playing: bool,
}

/// The mix bus: every voice plus its cursor behind one lock. Shared as
/// `Arc<Mutex<AudioBus>>` between the shell (loop thread) and the device
/// thread; the outer lock serialises pulls against play/stop/posit, the
/// inner one guards the vec itself. No reentrancy (`pull` never calls
/// back into the bus), so the nesting cannot deadlock.
#[derive(Debug, Default)]
pub struct AudioBus {
    voices: Mutex<Vec<Voice>>,
}

/// A voice's address on its bus. Cloned into the voice on bind; `Clone`
/// shares the address (bound voices are effectively shared — documented
/// on the bind sites).
#[derive(Debug, Clone)]
pub(crate) struct VoiceHandle {
    pub(crate) bus: Arc<Mutex<AudioBus>>,
    pub(crate) index: usize,
    /// The canonical buffer the voice was registered with. Clones share
    /// the allocation, so `trigger_or_restart` can group overlap copies
    /// by buffer identity (`Arc::ptr_eq`).
    pub(crate) data: Arc<Vec<i8>>,
}

impl VoiceHandle {
    pub(crate) fn with_voice<R>(&self, f: impl FnOnce(&mut Voice) -> R) -> Option<R> {
        let bus = self.bus.lock().ok()?;
        let mut voices = bus.voices.lock().ok()?;
        voices.get_mut(self.index).map(f)
    }
}

impl AudioBus {
    /// An empty bus: no voices, silence out.
    pub fn new() -> Self {
        Self {
            voices: Mutex::new(Vec::new()),
        }
    }

    /// Register a voice; returns its bus index. The caller (which holds
    /// the `Arc`) wraps it in a `VoiceHandle`. Registration clones the
    /// buffer reference (`Arc` clone) once — never per pull.
    pub fn register(
        &mut self,
        data: Arc<Vec<i8>>,
        rate: u32,
        cursor: f64,
        mode: VoiceMode,
        playing: bool,
    ) -> usize {
        // Outer lock is held by the caller (`Arc<Mutex<AudioBus>>`);
        // this inner lock is uncontended by construction.
        let mut voices = self.voices.lock().unwrap();
        let index = voices.len();
        voices.push(Voice {
            data,
            rate,
            cursor,
            mode,
            playing,
        });
        index
    }

    /// Concurrent one-shot copies per source buffer before the oldest
    /// gets stolen. Bounds the JDK's unbounded AudioPlayer stacking
    /// (which spawns a player thread per `play()` call) while keeping
    /// the grind/wash density the game mixes for.
    const MAX_OVERLAPS: usize = 8;

    /// Fire-and-forget one-shot trigger for `AudioClip::play`: when the
    /// primary voice (`index`) is stopped, arm it in place (cursor 0);
    /// when it is still sounding, stack another copy instead of
    /// truncating it — reusing a finished same-buffer slot first,
    /// appending while the group (primary + copies) is below
    /// `MAX_OVERLAPS + 1`, else stealing the oldest non-primary slot.
    /// Same-buffer grouping is by buffer identity (`Arc::ptr_eq`), so
    /// each clip's wash is bounded independently and the primary itself
    /// is never stolen out from under `posit()` readers.
    pub fn trigger_or_restart(&mut self, index: usize, data: Arc<Vec<i8>>, rate: u32) {
        let mut voices = self.voices.lock().unwrap();
        let playing = voices.get(index).is_some_and(|v| v.playing);
        if !playing {
            if let Some(v) = voices.get_mut(index) {
                v.cursor = 0.0;
                v.playing = true;
                v.mode = VoiceMode::OneShot;
            }
            return;
        }
        if let Some(slot) = voices
            .iter()
            .position(|v| !v.playing && Arc::ptr_eq(&v.data, &data))
        {
            let v = &mut voices[slot];
            v.cursor = 0.0;
            v.playing = true;
            v.mode = VoiceMode::OneShot;
            return;
        }
        let group = voices
            .iter()
            .filter(|v| Arc::ptr_eq(&v.data, &data))
            .count();
        if group < Self::MAX_OVERLAPS + 1 {
            voices.push(Voice {
                data,
                rate,
                cursor: 0.0,
                mode: VoiceMode::OneShot,
                playing: true,
            });
            return;
        }
        let slot = voices
            .iter()
            .enumerate()
            .find(|(i, v)| *i != index && Arc::ptr_eq(&v.data, &data))
            .or_else(|| {
                voices
                    .iter()
                    .enumerate()
                    .find(|(_, v)| Arc::ptr_eq(&v.data, &data))
            })
            .map(|(i, _)| i);
        if let Some(slot) = slot {
            let v = &mut voices[slot];
            v.cursor = 0.0;
            v.playing = true;
            v.mode = VoiceMode::OneShot;
        }
    }

    /// Number of registered voices (the latch test asserts late joins
    /// don't duplicate).
    pub fn voice_count(&self) -> usize {
        self.voices.lock().map(|v| v.len()).unwrap_or(0)
    }

    /// Whether the voice at `index` is currently armed (device-side
    /// status without touching cursors).
    pub fn is_playing(&self, index: usize) -> bool {
        let Ok(voices) = self.voices.lock() else {
            return false;
        };
        voices.get(index).is_some_and(|v| v.playing)
    }

    /// The mixer the device thread pulls: for each playing voice, advance
    /// the fractional cursor by `voice.rate / rate` per output sample,
    /// LINEARLY INTERPOLATE the two neighboring μ-law bytes at the
    /// fractional cursor, and SUM into `out` with i16 clamping. The
    /// interpolation is what makes 7500–9000 Hz voices clean at
    /// 44100/48000 Hz: nearest-sample would stairstep (harsh aliasing on
    /// upsample). Additive (the device supplies a fresh buffer each
    /// callback; layered pulls compose). OneShot voices stop
    /// (`playing = false`) when the cursor passes the end — the pull
    /// after the end contributes silence; the last valid sample
    /// interpolates toward the final byte. Loop voices wrap (the seam
    /// wraps too). Empty or paused voices contribute nothing (a paused
    /// voice keeps its cursor). `rate == 0` would divide by zero: yields
    /// silence.
    ///
    /// Per-sample cost is two table lookups plus a few flops — trivial
    /// next to the millisecond-scale device budget (see the latency note
    /// in nfm-render's `audio` module).
    pub fn pull(&mut self, out: &mut [i16], rate: u32) {
        if rate == 0 {
            return;
        }
        let mut voices = self.voices.lock().unwrap();
        for voice in voices.iter_mut() {
            if !voice.playing {
                continue;
            }
            let len = voice.data.len();
            if len == 0 {
                continue;
            }
            let step = voice.rate as f64 / rate as f64;
            let mut cursor = voice.cursor;
            for slot in out.iter_mut() {
                let mut idx = cursor.floor() as usize;
                if idx >= len {
                    match voice.mode {
                        VoiceMode::OneShot => {
                            voice.playing = false;
                            break;
                        }
                        VoiceMode::Loop => {
                            cursor %= len as f64;
                            idx = cursor.floor() as usize;
                        }
                    }
                }
                let i0 = idx;
                // The neighbor across the seam wraps for loops; one-shots
                // clamp onto the final byte (weight shifts fully to it).
                let i1 = match voice.mode {
                    VoiceMode::OneShot => (i0 + 1).min(len - 1),
                    VoiceMode::Loop => (i0 + 1) % len,
                };
                let frac = (cursor - i0 as f64) as f32;
                let s0 = decode_ulaw(voice.data[i0] as u8) as f32;
                let s1 = decode_ulaw(voice.data[i1] as u8) as f32;
                let sample = (s0 + (s1 - s0) * frac).round() as i32;
                *slot = ((*slot as i32) + sample).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                cursor += step;
            }
            if voice.playing {
                voice.cursor = cursor;
            }
        }
    }
}

impl AudioSink for AudioBus {
    fn pull(&mut self, out: &mut [i16], rate: u32) {
        AudioBus::pull(self, out, rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus_with(voices: Vec<Voice>) -> Arc<Mutex<AudioBus>> {
        Arc::new(Mutex::new(AudioBus {
            voices: Mutex::new(voices),
        }))
    }

    fn voice(data: &[u8], rate: u32, mode: VoiceMode, playing: bool) -> Voice {
        Voice {
            data: Arc::new(data.iter().map(|&b| b as i8).collect()),
            rate,
            cursor: 0.0,
            mode,
            playing,
        }
    }

    /// Two voices at different native rates sum with i16 clamping: a
    /// silent voice (0xFF decodes to 0) plus a loud one passes the loud
    /// one through, and doubling the loudest byte pins the rail.
    #[test]
    fn mixer_sums_two_rates_with_clamp() {
        let bus = bus_with(vec![
            voice(&[0xFF, 0xFF, 0xFF, 0xFF], 8000, VoiceMode::Loop, true),
            voice(&[0x80, 0x80, 0x80, 0x80], 9000, VoiceMode::Loop, true),
        ]);
        let mut out = [0i16; 4];
        bus.lock().unwrap().pull(&mut out, 8000);
        let loud = decode_ulaw(0x80);
        assert_eq!(out, [loud; 4]);

        // doubling the loudest byte in the table pins the positive rail
        fn peak_byte() -> u8 {
            (0..=255u8).max_by_key(|&b| decode_ulaw(b)).unwrap()
        }
        let bus = bus_with(vec![
            voice(&[peak_byte()], 8000, VoiceMode::Loop, true),
            voice(&[peak_byte()], 8000, VoiceMode::Loop, true),
        ]);
        let mut out = [0i16; 2];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert!(
            (decode_ulaw(peak_byte()) as i32) * 2 > i16::MAX as i32,
            "the test's premise: doubled peak exceeds the rail"
        );
        assert_eq!(out, [i16::MAX, i16::MAX]);
    }

    /// `trigger_or_restart` arms a stopped primary in place (no extra
    /// voice); while it sounds, further triggers stack bounded copies.
    #[test]
    fn trigger_stacks_bounded_copies() {
        let bus = Arc::new(Mutex::new(AudioBus::new()));
        let data = Arc::new(vec![10i8; 64]);
        let idx =
            bus.lock()
                .unwrap()
                .register(Arc::clone(&data), 8000, 0.0, VoiceMode::OneShot, false);
        // stopped primary → arm in place
        bus.lock()
            .unwrap()
            .trigger_or_restart(idx, Arc::clone(&data), 8000);
        {
            let b = bus.lock().unwrap();
            assert_eq!(b.voice_count(), 1);
            assert!(b.is_playing(idx));
        }
        // ten more triggers while sounding → cap at 1 + MAX_OVERLAPS
        for _ in 0..10 {
            bus.lock()
                .unwrap()
                .trigger_or_restart(idx, Arc::clone(&data), 8000);
        }
        {
            let b = bus.lock().unwrap();
            assert_eq!(b.voice_count(), 1 + AudioBus::MAX_OVERLAPS);
            assert!(b.is_playing(idx), "primary keeps its arm");
        }
    }

    /// Finished copies are reused before the pool grows; an exhausted
    /// pool steals the oldest copy, never the primary.
    #[test]
    fn trigger_reuses_finished_slots_then_steals_oldest() {
        let bus = Arc::new(Mutex::new(AudioBus::new()));
        let data = Arc::new(vec![10i8; 4]);
        let idx =
            bus.lock()
                .unwrap()
                .register(Arc::clone(&data), 8000, 0.0, VoiceMode::OneShot, false);
        bus.lock()
            .unwrap()
            .trigger_or_restart(idx, Arc::clone(&data), 8000);
        bus.lock()
            .unwrap()
            .trigger_or_restart(idx, Arc::clone(&data), 8000);
        assert_eq!(bus.lock().unwrap().voice_count(), 2);
        // drain both to the end (4 samples) plus one more pull so both
        // stop; the next trigger must reuse, not grow
        bus.lock().unwrap().pull(&mut [0i16; 4], 8000);
        bus.lock().unwrap().pull(&mut [0i16; 1], 8000);
        assert!(!bus.lock().unwrap().is_playing(idx));
        bus.lock()
            .unwrap()
            .trigger_or_restart(idx, Arc::clone(&data), 8000);
        assert_eq!(bus.lock().unwrap().voice_count(), 2, "finished slot reused");
        assert!(bus.lock().unwrap().is_playing(idx), "primary re-armed");
    }

    /// A 9000 Hz voice pulled at 8000 advances 9/8 per sample. Cursor
    /// travel is unchanged by interpolation (8 outputs → cursor 9.0),
    /// but each sample blends its neighbors: at cursor c = i0 + f,
    /// out = decode(i0) + (decode(i0+1) − decode(i0)) * f. Hand-check,
    /// with dN = decode_ulaw(N) (first-preimage values: d0 = -32768,
    /// d1 = -31744, d2 = -30712, d3 = -29688, d4 = -28656, d5 = -27632,
    /// d6 = -26600, d7 = -25576, d8 = -24544):
    /// k=0: c=0.0 → d0 = -32768;
    /// k=1: c=1.125 → d1 + (d2−d1)*0.125 = -31744 + 1032*0.125 = -31615;
    /// k=2: c=2.25 → d2 + (d3−d2)*0.25 = -30712 + 1024*0.25 = -30456;
    /// k=3: c=3.375 → d3 + (d4−d3)*0.375 = -29688 + 1032*0.375 = -29301;
    /// k=4: c=4.5 → d4 + (d5−d4)*0.5 = -28656 + 1024*0.5 = -28144;
    /// k=5: c=5.625 → d5 + (d6−d5)*0.625 = -27632 + 1032*0.625 = -26987;
    /// k=6: c=6.75 → d6 + (d7−d6)*0.75 = -26600 + 1024*0.75 = -25832;
    /// k=7: c=7.875 → d7 + (d8−d7)*0.875 = -25576 + 1032*0.875 = -24673.
    /// (All exact in f32: values < 2^24, eighths exact in binary.)
    #[test]
    fn resample_cursor_travel_and_first_samples() {
        let data: Vec<u8> = (0..64u8).collect();
        let bus = bus_with(vec![voice(&data, 9000, VoiceMode::OneShot, true)]);
        let mut out = [0i16; 8];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(
            out,
            [
                -32768, -31615, -30456, -29301, -28144, -26987, -25832, -24673
            ]
        );
        // strictly between the neighbors: interpolated, not floored
        assert!(out[1] > decode_ulaw(1) && out[1] < decode_ulaw(2));
        // 8 outputs × 1.125 steps
        let guard = bus.lock().unwrap();
        let voices = guard.voices.lock().unwrap();
        assert!((voices[0].cursor - 9.0).abs() < 1e-9);
    }

    /// OneShot stops exactly at the end: the consuming pull delivers
    /// every sample and leaves `playing` set; the next pull contributes
    /// silence (sentinel untouched) and clears it.
    #[test]
    fn oneshot_stops_at_end() {
        let bus = bus_with(vec![voice(
            &[0x80, 0x81, 0x82, 0x83],
            8000,
            VoiceMode::OneShot,
            true,
        )]);
        let mut out = [0i16; 4];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(
            out,
            [
                decode_ulaw(0x80),
                decode_ulaw(0x81),
                decode_ulaw(0x82),
                decode_ulaw(0x83)
            ]
        );
        assert!(bus.lock().unwrap().voices.lock().unwrap()[0].playing);

        let mut out = [0x7FFFi16; 2];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(out, [0x7FFF, 0x7FFF], "silence: buffer untouched");
        assert!(!bus.lock().unwrap().voices.lock().unwrap()[0].playing);
    }

    /// Loop wraps: 6 outputs over 4 samples read [0,1,2,3,0,1] and the
    /// cursor lands on 2.0 (wrapped, not linear 6.0).
    #[test]
    fn loop_wraps() {
        let bus = bus_with(vec![voice(
            &[0x80, 0x81, 0x82, 0x83],
            8000,
            VoiceMode::Loop,
            true,
        )]);
        let mut out = [0i16; 6];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(
            out,
            [
                decode_ulaw(0x80),
                decode_ulaw(0x81),
                decode_ulaw(0x82),
                decode_ulaw(0x83),
                decode_ulaw(0x80),
                decode_ulaw(0x81),
            ]
        );
        let guard = bus.lock().unwrap();
        let voices = guard.voices.lock().unwrap();
        assert!(voices[0].playing);
        assert!((voices[0].cursor - 2.0).abs() < 1e-9);
    }

    /// Empty and paused voices contribute silence; a paused voice keeps
    /// its cursor for the resume.
    #[test]
    fn empty_and_paused_voices_are_silent() {
        let mut v = voice(&[], 8000, VoiceMode::Loop, true);
        v.cursor = 3.0;
        let mut p = voice(&[0x80, 0x80], 8000, VoiceMode::Loop, false);
        p.cursor = 1.0;
        let bus = bus_with(vec![v, p]);
        let mut out = [0x7FFFi16; 3];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(out, [0x7FFF, 0x7FFF, 0x7FFF]);
        let guard = bus.lock().unwrap();
        let voices = guard.voices.lock().unwrap();
        assert!(voices[0].playing, "empty voice stays armed");
        assert_eq!(voices[1].cursor, 1.0, "paused cursor held");
    }

    /// A wired RadicalMod's posit() reflects the bus drain: pulls shrink
    /// it, stop() freezes it, resume() continues it, and a redundant
    /// play() while playing touches neither cursor.
    #[test]
    fn wired_posit_reflects_bus_pulls() {
        use crate::radicalmod::{RadicalMod, SuperStream};

        let bus = Arc::new(Mutex::new(AudioBus::new()));
        let mut r = RadicalMod {
            stream: Some(SuperStream::new(Some(vec![10i8; 100]))),
            playing: false,
            rate: 8000,
            handle: None,
            #[cfg(test)]
            tag: String::new(),
        };
        r.play();
        r.bind_audio(&bus);
        assert_eq!(r.posit(), 100);

        bus.lock().unwrap().pull(&mut [0i16; 10], 8000);
        assert_eq!(r.posit(), 90, "10 samples drained");

        // redundant play() while playing: neither cursor moves
        r.play();
        bus.lock().unwrap().pull(&mut [0i16; 10], 8000);
        assert_eq!(r.posit(), 80);

        // stop() forwards: the bus voice goes silent, posit() holds
        r.stop();
        let mut out = [0x7FFFi16; 10];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(out, [0x7FFF; 10], "stopped voice is silent");
        assert_eq!(r.posit(), 80, "cursor held while stopped");

        // resume() continues from the held cursor
        r.resume();
        bus.lock().unwrap().pull(&mut [0i16; 80], 8000);
        assert_eq!(r.posit(), 0);
    }

    /// A wired clip decodes through the bus at its header rate with the
    /// real payload: pulled samples equal the table decode of the file
    /// bytes, and position() tracks the drain.
    #[test]
    fn wired_clip_decodes_and_tracks_position() {
        use crate::audioclip::AudioClip;

        let raw: &[u8] = include_bytes!("../../../assets/sounds/newsun/a0.au");
        let mut c = AudioClip::parse(raw).expect("a0.au must parse");
        c.play();
        let bus = Arc::new(Mutex::new(AudioBus::new()));
        c.bind_audio(&bus);
        // position() is the drain cursor (0 = head), unlike music
        // posit() which reports the remainder
        assert_eq!(c.position(), 0);

        let mut out = [0i16; 100];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(c.position(), 100);
        for (i, s) in out.iter().enumerate() {
            assert_eq!(*s, decode_ulaw(raw[28 + i]), "sample {i}");
        }
    }

    /// Loop seam: with a fractional step the last sample before the wrap
    /// blends toward the first byte. Voice rate 12000, out 8000 (step
    /// 1.5) over [d0..d3]: cursors 0, 1.5, 3.0, 4.5→wrap 0.5, giving
    /// [d0, (d1+d2)/2, d3, (d0+d1)/2] with d0=-32768, d1=-31744,
    /// d2=-30712, d3=-29688 — all exact in f32.
    #[test]
    fn loop_seam_interpolates_across_wrap() {
        let bus = bus_with(vec![voice(&[0, 1, 2, 3], 12000, VoiceMode::Loop, true)]);
        let mut out = [0i16; 4];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(out, [-32768, -31228, -29688, -32256]);
    }

    /// OneShot tail: the last valid sample interpolates toward the final
    /// byte (no out-of-bounds read), and once the cursor passes the end
    /// the voice stops mid-buffer, leaving the rest untouched. Same
    /// 1.5-step setup over [d0..d3]: the consuming pull writes
    /// [d0, (d1+d2)/2, d3], and the next pull writes nothing and clears
    /// `playing`.
    #[test]
    fn oneshot_tail_blends_into_final_byte() {
        let bus = bus_with(vec![voice(&[0, 1, 2, 3], 12000, VoiceMode::OneShot, true)]);
        let mut out = [0i16; 3];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(out, [-32768, -31228, -29688]);
        assert!(bus.lock().unwrap().is_playing(0), "end reached, not passed");
        let mut out = [0x7FFFi16; 1];
        bus.lock().unwrap().pull(&mut out, 8000);
        assert_eq!(out, [0x7FFF], "stopped before writing");
        assert!(!bus.lock().unwrap().is_playing(0));
    }
}
