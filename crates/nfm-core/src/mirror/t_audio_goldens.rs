//! `TAudioGoldens.java` — the eleven stage tracks at the game's exact
//! per-stage parameters: the decode golden (`Mod` → `ModSlayer` →
//! `turnbytes()` → md5 + length), the structural rows (tracks / patterns /
//! song length / instruments / `bpm_samples`), and the DEVICE-FREE mix
//! golden (the 8080-Hz `BufferedInputStream(in, 1024)` boundary: `ceil(len /
//! 1024)` calls × 1024 samples, md5 + byte counts). Group "AudioGolden"
//! (`Suite.java` position 18 — note the GROUP is singular, the class is
//! plural). Pin source: `RadicalMod.java:37-50`, `ModSlayer.java:553-621`,
//! `SuperStream.java:18-31`, `xtGraphics.java:1249-1324`; narrative:
//! notes/JAVA-BEHAVIOR/B13_audio_goldens.md.
//!
//! Consumes BOTH fixtures the Java class does: `constants.toml
//! [audio_device]` (the 1024-byte device frame at 8000 Hz) and `audio.toml`
//! directly (one `[[stage]]` row per stage, `TAudioGoldens.java:74-101`).
//!
//! Port-shape mapping (documented at each pin):
//! - the Java `new Mod(track + ".zipo", new TestApplet("assets/music"))`
//!   inflates `assets/music/<track>.zipo`'s first entry; the TOML `path`
//!   ("music/<track>.zipo") resolved under the Java class's second applet
//!   ("assets") is the SAME file — the port's `include_bytes!` table below
//!   is that mapping, so both suites read one asset per stage.
//! - the Java `new RadicalMod(path, gain, rate, bpm, assets)` takes the
//!   inflated image in the port (`RadicalMod::new(bytes, gain, rate, bpm)`);
//!   the ctor re-parses and re-mixes independently of the decode golden,
//!   exactly like Java (two `Mod`/`ModSlayer` runs per stage).
//! - the mix pump reads the port's `SuperStream` with `read_at` (the
//!   established device-free pump — `SuperStream.read` fills exactly `len`
//!   and resets at EOF); `AudioBus::pull` is the port-only device path and
//!   is deliberately not used.

use crate::hash::md5_hex;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::modslayer::ModSlayer;
use crate::radicalmod::{RadicalMod, SuperStream};
use crate::tracker::Mod;
use crate::zipo::Zipo;

/// The per-stage `.zipo` containers, `assets/music/<id>.zipo` in stage order
/// (the Java `new Mod(track + ".zipo", music)` path with `music` rooted at
/// `assets/music`; `path` in audio.toml resolves to the same file under the
/// `assets` root).
const STAGE_ZIPOS: [(&str, &[u8]); 11] = [
    (
        "stage1",
        include_bytes!("../../../../assets/music/stage1.zipo"),
    ),
    (
        "stage2",
        include_bytes!("../../../../assets/music/stage2.zipo"),
    ),
    (
        "stage3",
        include_bytes!("../../../../assets/music/stage3.zipo"),
    ),
    (
        "stage4",
        include_bytes!("../../../../assets/music/stage4.zipo"),
    ),
    (
        "stage5",
        include_bytes!("../../../../assets/music/stage5.zipo"),
    ),
    (
        "stage6",
        include_bytes!("../../../../assets/music/stage6.zipo"),
    ),
    (
        "stage7",
        include_bytes!("../../../../assets/music/stage7.zipo"),
    ),
    (
        "stage8",
        include_bytes!("../../../../assets/music/stage8.zipo"),
    ),
    (
        "stage9",
        include_bytes!("../../../../assets/music/stage9.zipo"),
    ),
    (
        "stage10",
        include_bytes!("../../../../assets/music/stage10.zipo"),
    ),
    (
        "stage11",
        include_bytes!("../../../../assets/music/stage11.zipo"),
    ),
];

/// Every key a stage row must carry (`TAudioGoldens.java:85-90`, the shape
/// pin at load).
const STAGE_KEYS: [&str; 16] = [
    "id",
    "path",
    "gain",
    "rate",
    "bpm",
    "decode_bytes",
    "decode_md5",
    "mix_calls",
    "mix_samples",
    "mix_md5",
    "num_tracks",
    "num_patterns",
    "song_length_patterns",
    "song_repeat_patterns",
    "instruments",
    "bpm_samples",
];

pub(crate) fn run() {
    check::group("AudioGolden");
    // TAudioGoldens.java:74-77 — the device boundary, constants.toml
    // [audio_device] (B13 §1.1): BufferedInputStream(in, 1024), ULAW fallback
    // at 8000 Hz, so one call is 1024 samples = 128 ms.
    let kad = Tml::load("constants.toml").table("audio_device");
    kad.require_keys(&["rate", "frame_bytes", "ms_per_call"]);
    let call_bytes = kad.intv("frame_bytes");
    let device_rate = kad.intv("rate");
    check::eq_i32(
        call_bytes * 1000 / device_rate,
        kad.intv("ms_per_call"),
        "audio_device_ms_per_call_128",
    );

    // TAudioGoldens.java:97-112 — the fixture is the data source: one row
    // per stage, in stage order, each carrying the full key set.
    let audio = Tml::load("audio.toml");
    check::eq_i32(audio.table_count("stage") as i32, 11, "audio_stage_rows_11");
    let rows = audio.tables("stage");
    let mut ids = String::new();
    for (i, s) in rows.iter().enumerate() {
        s.require_keys(&STAGE_KEYS);
        if i > 0 {
            ids.push(',');
        }
        ids.push_str(&s.str("id"));
    }
    check::eq_str(
        ids,
        "stage1,stage2,stage3,stage4,stage5,stage6,stage7,stage8,stage9,stage10,stage11",
        "audio_stage_ids_in_order",
    );

    for (i, row) in rows.iter().enumerate() {
        stage(i, row, call_bytes);
    }
}

/// The raw `.zipo` container for a stage id (`assets/music/<id>.zipo`, the
/// Java `new Mod(track + ".zipo", music)` path).
fn stage_zipo(track: &str) -> &'static [u8] {
    STAGE_ZIPOS
        .iter()
        .find(|(id, _)| *id == track)
        .map(|(_, raw)| *raw)
        .unwrap_or_else(|| panic!("no stage zipo for {track}"))
}

/// The inflated module image (the container's first entry, Mod.java:40-62) —
/// what the port's `RadicalMod::new` takes in place of Java's path+Applet.
fn module_bytes(raw: &[u8]) -> Vec<u8> {
    Zipo::read(raw)
        .expect("stage zipo parses")
        .entries
        .into_iter()
        .next()
        .expect("music zips are single-entry")
        .data
}

/// One stage (`TAudioGoldens.java:124-180`): decode golden + structural pins
/// + device-free mix golden. Every expected value is the TOML row's.
fn stage(i: usize, s: &Tml, call_bytes: i32) {
    let track = s.str("id");
    // `path` is the Java RadicalMod argument ("music/<id>.zipo" under the
    // `assets` root); read it for shape, and resolve the same file through
    // the id-keyed include_bytes table above.
    let _path = s.str("path");
    let gain = s.intv("gain");
    let rate = s.intv("rate");
    let bpm = s.intv("bpm");
    let decode_bytes = s.intv("decode_bytes");
    let zipo = stage_zipo(&track);
    let bytes = module_bytes(zipo);

    // ---- decode golden (ModSlayer.java:553) ----
    let song = Mod::from_zipo(Some(zipo));
    check::eq_i32(
        song.numtracks,
        s.intv("num_tracks"),
        &format!("{track}_tracks"),
    );
    check::eq_i32(
        song.num_patterns(),
        s.intv("num_patterns"),
        &format!("{track}_numpatterns"),
    );
    check::eq_i32(
        song.patterns.len() as i32,
        s.intv("num_patterns"),
        &format!("{track}_pattern_rows"),
    );
    check::eq_i32(
        song.song_length_patterns,
        s.intv("song_length_patterns"),
        &format!("{track}_song_length"),
    );
    check::eq_i32(
        song.song_repeat_patterns,
        s.intv("song_repeat_patterns"),
        &format!("{track}_song_repeat"),
    );
    check::eq_i32(
        song.insts.len() as i32,
        s.intv("instruments"),
        &format!("{track}_instruments"),
    );

    let mut slayer = ModSlayer::new(song, rate, gain, bpm);
    // capture the tempo before the mixer runs: Fxx effects may retune
    // bpm_samples mid-song (get_track case 15, ModSlayer.java:759-769)
    slayer.startplaying(false);
    check::eq_i32(
        slayer.bpm_samples,
        s.intv("bpm_samples"),
        &format!("{track}_bpm_samples"),
    );
    check::eq_i32(
        slayer.numtracks,
        s.intv("num_tracks"),
        &format!("{track}_slayer_tracks"),
    );

    let decoded = slayer.turnbytes();
    check::eq_i32(
        decoded.len() as i32,
        decode_bytes,
        &format!("{track}_decode_len"),
    );
    let decode_md5 = md5_hex(&u8s(&decoded));
    check::eq_str(
        decode_md5.clone(),
        s.str("decode_md5"),
        &format!("{track}_decode_md5"),
    );

    // ---- the game's ctor path (RadicalMod.java:37-50) ----
    // proves the (gain, rate) order again: the reversed order would make
    // turnbytes throw and leave stream == null (TRadicalMod pin)
    let mut rm = RadicalMod::new(&bytes, gain, rate, bpm);
    check::ok(rm.stream.is_some(), &format!("{track}_stream_not_null"));
    check::eq_bool(rm.playing, false, &format!("{track}_ctor_playing_false"));
    // available() BEFORE any read: the full turnbytes() array
    // (SuperStream over ByteArrayInputStream, SuperStream.java:4-6)
    check::eq_i32(
        rm.stream.as_ref().expect("stream built").available() as i32,
        decode_bytes,
        &format!("{track}_stream_available"),
    );

    // ---- mix golden: the device-free per-call pump ----
    let calls = s.intv("mix_calls");
    let mixed = pump(rm.stream.as_mut().expect("stream built"), calls, call_bytes);
    let mix_len = s.intv("mix_samples");
    check::eq_i32(mixed.len() as i32, mix_len, &format!("{track}_mix_samples"));
    check::eq_i32(
        mix_len,
        (decode_bytes + call_bytes - 1) / call_bytes * call_bytes,
        &format!("{track}_mix_len_is_ceiling"),
    );
    check::eq_str(
        md5_hex(&u8s(&mixed)),
        s.str("mix_md5"),
        &format!("{track}_mix_md5"),
    );
    // the pump's first DECODE_LEN bytes are the whole decoded track:
    // a second independent turnbytes() run (inside RadicalMod) is
    // byte-identical, and chunking only appends the wrap-in head
    check::ok(
        same_prefix(&mixed, &decoded, decode_bytes as usize),
        &format!("{track}_mix_prefix_matches_decode"),
    );

    if i == 0 {
        // THE standing Java==Rust fidelity proof (B13 contract)
        check::eq_str(
            decode_md5,
            "f1f2a25eb468517faa2218cf1712395f",
            "stage1_matches_rust_port_digest",
        );
    }
}

/// Reads `calls` device-sized frames (`TAudioGoldens.java:185-198`). Every
/// call must return exactly `call_bytes` (SuperStream.java:18-31 fills to
/// len, resetting on EOF), so the concatenation is one buffer-rounded pass.
fn pump(stream: &mut SuperStream, calls: i32, call_bytes: i32) -> Vec<i8> {
    let cb = call_bytes as usize;
    assert!(calls >= 0, "call count is non-negative");
    let mut out = vec![0i8; calls as usize * cb];
    let mut one = vec![0i8; cb];
    for c in 0..calls as usize {
        let n = stream.read_at(&mut one, 0, cb);
        if n != cb {
            // cannot happen with SuperStream; keep the golden honest
            panic!("short device read {n} at call {c}");
        }
        out[c * cb..(c + 1) * cb].copy_from_slice(&one);
    }
    out
}

/// `samePrefix` (`TAudioGoldens.java:200-210`).
fn same_prefix(a: &[i8], b: &[i8], n: usize) -> bool {
    if a.len() < n || b.len() < n {
        return false;
    }
    a[..n] == b[..n]
}

/// `byte[]` → the unsigned bytes `md5` hashes in Java.
fn u8s(data: &[i8]) -> Vec<u8> {
    data.iter().map(|&b| b as u8).collect()
}
