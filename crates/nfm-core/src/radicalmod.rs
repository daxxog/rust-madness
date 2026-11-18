//! `radicalmod` — the looping music voice: `RadicalMod` + `SuperStream`.
//!
//! Faithful port of `reference/vineflower/RadicalMod.java` (81 lines) and
//! `reference/vineflower/SuperStream.java` (32 lines).
//!
//! `SuperStream` is `ByteArrayInputStream` with a cursor that wraps to 0 at
//! EOF instead of returning -1, so the song loops forever. The port models
//! only the cursor: NOTHING drains it here. In the JDK the drain is
//! `AudioPlayer.player.start(stream)` — a background thread consuming bytes
//! in wall time — which belongs to the real-time shell slice, not this
//! headless port. Consequence (documented wherever it matters): `posit()`
//! stays at the full length after `play()` and only shrinks when the test
//! (or a future drain) calls `read`.
//!
//! `RadicalMod::new` mirrors the constructor (RadicalMod.java:37-50):
//! `Mod::load_mod` over the module bytes, `ModSlayer::new`, `turnbytes`
//! into the stream; any failure leaves `stream = None` and prints the
//! source's `"Error Loading Mod: ..."` line. (`System.gc()` has no port.)
//! Java's try wraps ONLY `turnbytes()` (:41-47), so a mixer that reads out
//! of range — the reversed gain/rate order is the observable case
//! (TRadicalMod `argorder_swapped_stream_null`) — throws and is caught
//! there. Rust has no checked exceptions: the same failure is a bounds
//! panic, so the catch maps to `catch_unwind` around that one call.
//!
//! `posit()` dereferences the stream with NO null guard
//! (RadicalMod.java:78-80), so a `None` stream panics with the source's
//! `NullPointerException` (TRadicalMod `fail_posit_npe`); it does not
//! degrade to 0.

use std::sync::{Arc, Mutex};

use crate::audiobus::{AudioBus, VoiceHandle, VoiceMode};
use crate::modslayer::ModSlayer;
use crate::tracker::Mod;

/// `SuperStream` (SuperStream.java): the mixed μ-law bytes with a
/// wrapping cursor. `ByteArrayInputStream.read()` returns the unsigned
/// byte or -1 at EOF; both overrides reset to 0 and keep going instead.
///
/// `new` takes `Option<Vec<i8>>` because the source parameter is a
/// `byte[]` and the JDK's `ByteArrayInputStream` ctor dereferences it: a
/// `None` is Java's `null` and panics with the source's NPE
/// (SuperStream.java:4-6; TSmall pins `ss_ctor_null_npe`).
#[derive(Debug, Clone)]
pub struct SuperStream {
    /// `ByteArrayInputStream.buf`: Java stores the caller's ARRAY by
    /// reference (no copy), so the port's buffer is `pub(crate)` for the
    /// same reachability (TSmall pins `ss_ctor_no_copy_sees_mutation`).
    pub(crate) data: Vec<i8>,
    pos: usize,
}

impl SuperStream {
    pub fn new(data: Option<Vec<i8>>) -> Self {
        Self {
            data: data.expect(
                "NullPointerException: SuperStream(byte[] null) (ByteArrayInputStream ctor, \
                 SuperStream.java:5)",
            ),
            pos: 0,
        }
    }

    /// `read()` (SuperStream.java:8-16): one byte as the JDK's unsigned
    /// `int` (`buf[pos++] & 0xFF`), or -1 when the buffer is empty (the
    /// reset-then-reread wrap cannot make progress). Wraps at EOF.
    pub fn read_byte(&mut self) -> i32 {
        if self.pos >= self.data.len() {
            self.pos = 0;
            if self.data.is_empty() {
                return -1;
            }
        }
        let b = self.data[self.pos];
        self.pos += 1;
        (b as u8) as i32
    }

    /// `read(byte[], int, int)` (SuperStream.java:18-31): fill
    /// `buf[off..off+len]` exactly, wrapping at EOF as often as needed,
    /// then return `len` (the original's accumulated count; 0 for len 0).
    /// Like the original, a len > 0 against a zero-length buffer never
    /// terminates — unpinned, per TSmall's class comment.
    pub fn read_at(&mut self, buf: &mut [i8], off: usize, len: usize) -> usize {
        let mut var4 = 0usize;
        while var4 < len {
            let var5 = self.data.len() - self.pos;
            if var5 > 0 {
                let n = var5.min(len - var4);
                buf[off + var4..off + var4 + n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                var4 += n;
            } else {
                // `super.read` returned -1 -> `this.reset()` (:25-26)
                self.pos = 0;
            }
        }
        var4
    }

    /// `read(byte[], int, int)` for the whole-slice case (`off == 0`).
    pub fn read_into(&mut self, buf: &mut [i8]) {
        self.read_at(buf, 0, buf.len());
    }

    /// `reset()` (`ByteArrayInputStream.reset()`): cursor back to 0.
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    /// `available()`: bytes remaining from the cursor.
    pub fn available(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
}

/// The music voice (RadicalMod.java).
///
/// `stream` is `pub` because `loadmusic` tests it for null exactly like
/// the source (`if (this.stracks[i].stream != null)`); `playing` is `pub`
/// for the same reason the source reads the field across classes.
/// `rate` is the native mix rate (the `rate` ctor arg, i.e.
/// `ModSlayer.samplingrate`); `handle` is the bus address once
/// `RadicalMod::bind_audio` runs, and `posit()` reads through it when
/// bound so the audio thread's drain is the one true cursor.
#[derive(Debug, Clone)]
pub struct RadicalMod {
    pub stream: Option<SuperStream>,
    pub playing: bool,
    pub rate: u32,
    pub(crate) handle: Option<VoiceHandle>,
    /// Test-only recording tag (see `audioclip::verb_log`): the mirror's
    /// `RecMod` seam tags a voice (`"track0"`, `"cars"`, `"stages"`) and
    /// then reads the verb log. Empty in production.
    #[cfg(test)]
    pub tag: String,
}

/// The `catch (Exception var9)` print payload (`RadicalMod.java:46`): Java
/// prints the exception's `toString`, Rust the panic payload. Exception TYPE
/// names are Java-only (the ctor's pin is the resulting `stream == null`, not
/// the text), so a non-string payload gets a placeholder.
fn panic_text(e: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic>".to_string()
    }
}

impl RadicalMod {
    /// `RadicalMod(String, gain, rate, bpm, Applet)` (RadicalMod.java:37-50)
    /// minus the applet fetch: `module_bytes` is the inflated module image
    /// (the first entry of the `.zipo`); `gain`/`rate`/`bpm` are the
    /// per-stage tuple in source order, i.e. `ModSlayer(mod, rate, gain,
    /// bpm)` (RadicalMod.java:39).
    pub fn new(module_bytes: &[u8], gain: i32, rate: i32, bpm: i32) -> Self {
        let stream = match Mod::load_mod(module_bytes) {
            Ok(song) => {
                let mut slayer = ModSlayer::new(song, rate, gain, bpm);
                // RadicalMod.java:41-47 — the try covers `turnbytes()` only,
                // and its catch turns the throw into `stream = null` plus the
                // printed line. Turnbytes is infallible for a real rate/gain
                // pair; a swapped one indexes the mix buffers out of range
                // (Java's ArrayIndexOutOfBoundsException, the pin sheet's
                // :44), which Rust reports as a bounds panic.
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| slayer.turnbytes()))
                {
                    Ok(bytes) => Some(SuperStream::new(Some(bytes))),
                    Err(e) => {
                        println!("Error Loading Mod: {}", panic_text(&e));
                        None
                    }
                }
            }
            Err(e) => {
                // RadicalMod.java:44-46; a Mod parse failure is the other
                // route to the source's `insts == null` (Mod.java:40-62).
                println!("Error Loading Mod: {e}");
                None
            }
        };
        Self {
            stream,
            playing: false,
            rate: rate.max(0) as u32,
            handle: None,
            #[cfg(test)]
            tag: String::new(),
        }
    }

    /// Bind this voice's stream to the bus: registers the mixed bytes (an
    /// `Arc` clone, once) at the native mix rate with the current play
    /// state, and stores the handle future `play`/`resume`/`stop` calls
    /// forward through. Music always loops (the `SuperStream` wrap).
    /// Idempotent; stream-less (`None`) voices are skipped. The bus
    /// cursor starts at the local cursor so binding never jumps `posit()`.
    pub(crate) fn bind_audio(&mut self, bus: &Arc<Mutex<AudioBus>>) {
        if self.handle.is_some() {
            return;
        }
        let Some(stream) = self.stream.as_ref() else {
            return;
        };
        let Ok(mut locked) = bus.lock() else {
            return;
        };
        let shared: Arc<Vec<i8>> = Arc::new(stream.data.clone());
        let index = locked.register(
            Arc::clone(&shared),
            self.rate,
            stream.pos as f64,
            VoiceMode::Loop,
            self.playing,
        );
        self.handle = Some(VoiceHandle {
            bus: Arc::clone(bus),
            index,
            data: shared,
        });
    }

    /// `play()` (RadicalMod.java:63-76): reset the stream if present, then
    /// flag playing. The `AudioPlayer.player.start` drain is the
    /// real-time shell's (see module docs). Note the source sets
    /// `playing = true` even when the stream is null — the try/catch
    /// swallows the failure — transcribed here: a failed load still
    /// "plays". Forwards the reset to the bus voice when bound.
    pub fn play(&mut self) {
        #[cfg(test)]
        crate::audioclip::verb_log::push(&self.tag, "play");
        if !self.playing {
            if let Some(stream) = self.stream.as_mut() {
                stream.reset();
            }
            self.playing = true;
            if let Some(h) = &self.handle {
                h.with_voice(|v| {
                    v.cursor = 0.0;
                    v.playing = true;
                    v.mode = VoiceMode::Loop;
                });
            }
        }
    }

    /// `stop()` (RadicalMod.java:8-17): flag cleared only when playing;
    /// the player-stop call rides with the real-time shell. Forwards to
    /// the bus voice when bound.
    pub fn stop(&mut self) {
        #[cfg(test)]
        crate::audioclip::verb_log::push(&self.tag, "stop");
        if self.playing {
            self.playing = false;
            if let Some(h) = &self.handle {
                h.with_voice(|v| {
                    v.playing = false;
                });
            }
        }
    }

    /// `resume()` (RadicalMod.java:52-61): like `play` but without the
    /// reset — the song continues where the cursor sits. Forwards the
    /// flag (not the cursor) to the bus voice when bound.
    pub fn resume(&mut self) {
        #[cfg(test)]
        crate::audioclip::verb_log::push(&self.tag, "resume");
        if !self.playing {
            self.playing = true;
            if let Some(h) = &self.handle {
                h.with_voice(|v| {
                    v.playing = true;
                    v.mode = VoiceMode::Loop;
                });
            }
        }
    }

    /// `posit()` (RadicalMod.java:78-80): `stream.available()`. The source has
    /// NO null guard — a `None` stream dereferences null and throws
    /// `NullPointerException`, so the port panics with that name (TRadicalMod
    /// `fail_posit_npe`); B1's "returns 0" was wrong. When bound, reads the
    /// bus cursor (the audio thread's drain) instead, so the wired position
    /// shrinks as the device pulls.
    pub fn posit(&self) -> i32 {
        if let Some(h) = &self.handle
            && let Some(remaining) =
                h.with_voice(|v| v.data.len().saturating_sub(v.cursor.floor() as usize))
        {
            return remaining as i32;
        }
        match self.stream.as_ref() {
            Some(s) => s.available() as i32,
            None => panic!(
                "NullPointerException: RadicalMod.posit() on a null stream \
                 (RadicalMod.java:78-80, no null guard)"
            ),
        }
    }

    /// `outwithit()` (RadicalMod.java:19-35): stop, then `close()` the
    /// stream. `ByteArrayInputStream.close()` is specified as a no-op, so
    /// the close collapses away and this is exactly `stop()`.
    pub fn outwithit(&mut self) {
        // The source is `stop()` + `stream.close()` (a no-op), but the
        // mirror's `RecMod` overrides `outwithit` separately from `stop`,
        // so the recorded verb is `outwithit` alone.
        #[cfg(test)]
        crate::audioclip::verb_log::push(&self.tag, "outwithit");
        self.playing = false;
        if let Some(h) = &self.handle {
            h.with_voice(|v| {
                v.playing = false;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zipo::Zipo;

    fn stage2_mod() -> Vec<u8> {
        let raw: &[u8] = include_bytes!("../../../assets/music/stage2.zipo");
        Zipo::read(raw)
            .unwrap()
            .entries
            .into_iter()
            .next()
            .unwrap()
            .data
    }

    #[test]
    fn new_loads_the_pinned_stage2_stream() {
        // xt:1257 — RadicalMod("music/stage2.zipo", 370, 9000, 145).
        let r = RadicalMod::new(&stage2_mod(), 370, 9000, 145);
        assert!(r.stream.is_some());
        assert!(!r.playing);
        // the pinned mixer length (modslayer::pin_stage2_turnbytes).
        assert_eq!(r.posit(), 2232930);
    }

    #[test]
    fn play_resets_resume_preserves_stop_preserves() {
        let mut r = RadicalMod::new(&stage2_mod(), 370, 9000, 145);
        let full = r.posit();
        r.play();
        assert!(r.playing);
        assert_eq!(r.posit(), full, "play() resets the cursor");
        // drain a little through the cursor
        let mut buf = [0i8; 1000];
        r.stream.as_mut().unwrap().read_into(&mut buf);
        assert_eq!(r.posit(), full - 1000);
        r.stop();
        assert!(!r.playing);
        assert_eq!(r.posit(), full - 1000, "stop() keeps the cursor");
        r.resume();
        assert!(r.playing);
        assert_eq!(r.posit(), full - 1000, "resume() keeps the cursor");
        r.stop();
        r.play();
        assert_eq!(r.posit(), full, "play() resets again");
    }

    #[test]
    fn read_wraps_at_eof_like_the_jdk() {
        let mut s = SuperStream::new(Some(vec![1i8, 2, 3]));
        assert_eq!([s.read_byte(), s.read_byte(), s.read_byte()], [1, 2, 3]);
        assert_eq!(s.read_byte(), 1, "read() wraps instead of -1");
        assert_eq!(s.available(), 2);
        // the bulk override fills exactly, wrapping as needed
        let mut buf = [0i8; 5];
        s.read_into(&mut buf);
        assert_eq!(buf, [2, 3, 1, 2, 3]);
        // reset() rewinds
        s.reset();
        assert_eq!(s.available(), 3);
        assert_eq!(s.read_byte(), 1);
    }

    #[test]
    fn failed_load_degrades_to_silent_no_music() {
        let mut r = RadicalMod::new(b"not a mod", 370, 9000, 145);
        assert!(r.stream.is_none());
        // RadicalMod.java:78-80 has no null guard: posit() throws NPE. The
        // port's panic is that throw (mirror pin fail_posit_npe).
        let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.posit()));
        let err = e.expect_err("posit on a None stream panics");
        let msg = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_default();
        assert!(msg.contains("NullPointerException"), "{msg}");
        // the play-on-null-stream quirk (RadicalMod.java:63-76): the flag
        // sets even with nothing to drain.
        r.play();
        assert!(r.playing);
        r.stop();
        assert!(!r.playing);
    }
}
