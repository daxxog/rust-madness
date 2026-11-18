//! `TRadicalMod.java` — the music-file loader: the ctor's Mod -> ModSlayer ->
//! turnbytes -> SuperStream chain, `posit()`, the `stop()` playing guard, the
//! null-guarded `outwithit()` close, the failed-ctor state and the null-stream
//! transitions of stop/resume/play/outwithit. Group "RadicalMod"
//! (`Suite.java` position 9). Pin source: `RadicalMod.java:8-80`; narrative:
//! notes/JAVA-BEHAVIOR/B1_audio_tiny.md.
//!
//! No TOML: the Java class consumes none (the stage1 length is TModSlayer's
//! independent pin).
//!
//! Port-shape mapping (documented at each pin):
//! - Java's `(String path, Applet)` ctor head is the port's pre-inflated
//!   `module_bytes` (the first `.zipo` entry, `RadicalMod.java:42`'s
//!   `turnbytes()` image); the `TestApplet("assets")` code base is the
//!   asset root the bytes came from.
//! - `stream == null` is `stream.is_none()` — the port's field is
//!   `Option<SuperStream>` (`RadicalMod.java:5` is a nullable reference).
//! - `stream.getClass().getName()` is reflection identity in Java. The port
//!   field's TYPE is `SuperStream`; `std::any::type_name_of_val` is the
//!   Rust runtime type identity, compared on its last path segment (Java's
//!   `getName()` is the simple class name).
//! - `AudioPlayer.player.start/stop` are the real-time shell's (see the
//!   `radicalmod` module docs); the null-stream audio pins observe the
//!   unconditional `playing` writes at `:15/:59/:74`, exactly what the Java
//!   comments say the calls prove.

use std::panic::AssertUnwindSafe;

use crate::mirror::harness as check;
use crate::radicalmod::{RadicalMod, SuperStream};
use crate::zipo::Zipo;

pub(crate) fn run() {
    check::group("RadicalMod");
    let stage1 = stage1_bytes();
    let mut rm = ctor_success(&stage1);
    stop_guard(&mut rm);
    outwithit_paths(&mut rm);
    failure_paths();
    arg_order(&stage1);
}

/// The fixture module image: `music/stage1.zipo`'s first entry, the same
/// bytes Java's `Mod` inflates behind `new Mod("music/stage1.zipo", app)`
/// (Mod.java:40-62). `include_bytes` keeps the mirror independent of cargo's
/// working directory, like the Java class's fixed asset root.
fn stage1_bytes() -> Vec<u8> {
    let raw: &[u8] = include_bytes!("../../../../assets/music/stage1.zipo");
    Zipo::read(raw)
        .expect("stage1.zipo inflates")
        .entries
        .into_iter()
        .next()
        .expect("stage1 has one entry")
        .data
}

/// The Rust runtime class name — Java `getClass().getName()` returns the
/// simple name, `type_name` the fully-qualified path, so take the last
/// segment.
fn class_name(of: &SuperStream) -> &'static str {
    std::any::type_name_of_val(of)
        .rsplit("::")
        .next()
        .expect("type_name has a segment")
}

/// `TRadicalMod.ctorSuccess` (TRadicalMod.java:46-59): RadicalMod.java:37-50
/// with the game's stage1 parameters (xtGraphics.java:1250: gain=350,
/// rate=8400). The ctor builds Mod + ModSlayer, calls turnbytes(), wraps the
/// bytes in a SuperStream and leaves `playing` at its field default.
fn ctor_success(stage1: &[u8]) -> RadicalMod {
    let rm = RadicalMod::new(stage1, 350, 8400, 135);
    check::ok(rm.stream.is_some(), "ctor_stream_not_null");
    check::eq_str(
        class_name(rm.stream.as_ref().expect("stream built")),
        "SuperStream",
        "ctor_stream_superstream",
    );
    check::eq_bool(rm.playing, false, "ctor_playing_false");
    // SuperStream delegates to ByteArrayInputStream over the turnbytes()
    // array (SuperStream.java:4-6), so available() is the FULL byte count;
    // 1191330 is TModSlayer's independent stage1_len pin.
    check::eq_i32(
        rm.stream.as_ref().expect("stream built").available() as i32,
        1_191_330,
        "ctor_stream_available_1191330",
    );
    // RadicalMod.java:78-80 is a bare `return this.stream.available();`
    check::eq_i32(rm.posit(), 1_191_330, "ctor_posit_1191330");
    rm
}

/// RadicalMod.java:8-17 — `stop()` is guarded by `if (this.playing)`; with
/// `playing == false` it touches neither the player nor the stream.
fn stop_guard(rm: &mut RadicalMod) {
    let before = rm.posit();
    rm.stop();
    check::eq_bool(rm.playing, false, "stop_not_playing_stays_false");
    check::eq_i32(rm.posit(), before, "stop_not_playing_posit_unchanged");
    // calling it twice is equally inert
    rm.stop();
    check::eq_bool(rm.playing, false, "stop_twice_stays_false");
}

/// RadicalMod.java:19-35 — `outwithit()` with `playing == false` skips the
/// player branch, then closes the stream only if non-null (:29-31).
/// `ByteArrayInputStream.close()` is a no-op, so `posit()` still works and
/// still reports the whole buffer.
fn outwithit_paths(rm: &mut RadicalMod) {
    rm.outwithit();
    check::eq_bool(rm.playing, false, "outwithit_not_playing_stays_false");
    check::ok(rm.stream.is_some(), "outwithit_keeps_stream_reference");
    check::eq_i32(rm.posit(), 1_191_330, "outwithit_posit_after_close_1191330");
    // idempotent: the second call closes an already-closed stream
    rm.outwithit();
    check::eq_bool(rm.playing, false, "outwithit_twice_stays_false");
    check::eq_i32(rm.posit(), 1_191_330, "outwithit_twice_posit_unchanged");
}

/// `TRadicalMod.failurePaths` (TRadicalMod.java:93-150). A missing file
/// makes Mod's ctor leave `insts == null` and `turnbytes()` throw at
/// ModSlayer.java:800, which RadicalMod.java:44-46 catches: `stream = null`,
/// `playing` stays false. In Rust the missing file is absent module bytes
/// (the port's ctor takes the inflated image, so a garbage image is the
/// parse failure that maps to `insts == null`).
fn failure_paths() {
    let mut bad = RadicalMod::new(b"music/nope.zipo", 350, 8400, 135);
    check::ok(bad.stream.is_none(), "fail_stream_null");
    check::eq_bool(bad.playing, false, "fail_playing_false");

    // RadicalMod.java:78-80 has NO null guard: posit() dereferences the null
    // stream and throws NullPointerException (B1 said "returns 0" — wrong).
    // `throws_it` maps the Java exception TYPE to the panic message, so the
    // port's NPE panic names the exception (see `RadicalMod::posit`).
    let fbad = &bad;
    check::throws_it(
        AssertUnwindSafe(|| {
            fbad.posit();
        }),
        "NullPointerException",
        "fail_posit_npe",
    );

    // :29-31 null-guards the close, so outwithit() on the failed instance
    // calls NOTHING (no exception at all) and leaves playing false
    bad.outwithit();
    check::ok(bad.stream.is_none(), "fail_outwithit_stream_stays_null");
    check::eq_bool(bad.playing, false, "fail_outwithit_playing_stays_false");

    // stop() with playing == false again: the guard wins over the null stream
    bad.stop();
    check::eq_bool(bad.playing, false, "fail_stop_not_playing");

    // ---- null-stream AudioPlayer paths (:8-17, :52-76) ----
    // The playing field is package-private (:6), so the test can set it; the
    // port's field is `pub` for the same reachability. The Java probe showed
    // the 8u40 player calls neither throw nor open a line, so what these pins
    // prove is that the playing writes at :15/:59/:74 happen unconditionally
    // outside the try blocks — the port has no player to call at all.
    bad.playing = true;
    bad.stop();
    check::eq_bool(bad.playing, false, "audio_stop_null_stream_resets_playing");
    bad.stop();
    check::eq_bool(bad.playing, false, "audio_stop_guard_when_already_false");

    bad.playing = false;
    bad.resume();
    check::eq_bool(bad.playing, true, "audio_resume_null_stream_sets_playing");
    bad.resume();
    check::eq_bool(bad.playing, true, "audio_resume_guard_when_playing");

    bad.stop();
    check::eq_bool(bad.playing, false, "audio_stop_after_resume_resets");
    bad.play();
    check::eq_bool(bad.playing, true, "audio_play_null_stream_sets_playing");
    bad.play();
    check::eq_bool(bad.playing, true, "audio_play_guard_when_playing");

    // outwithit while playing == true on the null-stream instance: the
    // playing branch runs (:20-27), the close is skipped (:30), no throw
    bad.outwithit();
    check::eq_bool(bad.playing, false, "audio_outwithit_playing_null_stream");
    check::ok(bad.stream.is_none(), "audio_outwithit_stream_stays_null");
}

/// `TRadicalMod.argOrder` (TRadicalMod.java:154-159): proves the ctor's int
/// order. The game's (gain=350, rate=8400) order yields a stream; the
/// reversed (8400, 350) order makes the mixer read out of bounds and the ctor
/// catches it (RadicalMod.java:41-47) — Java swallows the
/// ArrayIndexOutOfBoundsException, Rust the equivalent bounds panic.
fn arg_order(stage1: &[u8]) {
    let swapped = RadicalMod::new(stage1, 8400, 350, 135);
    check::ok(swapped.stream.is_none(), "argorder_swapped_stream_null");
    check::eq_bool(swapped.playing, false, "argorder_swapped_playing_false");
}
