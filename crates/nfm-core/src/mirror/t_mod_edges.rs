// FIDELITY: TModEdges.java mirrors the Java Mod.java byte-writing code, including its explicit `(byte)` casts and helper arity.
#![allow(clippy::too_many_arguments, clippy::unnecessary_cast)]
//! `TModEdges.java` — the LOADER EDGES of `Mod.java` (synthetic MOD byte
//! images exercising the FOURCC voice table, the sequence/instrument clamps,
//! the repeat-sample copy and the truncated-sample catch) plus the remaining
//! effect/pattern edges of `ModSlayer.java` (arpeggio/portamento offsets, the
//! dead E1/E2 switch, both PORT_TO fall-short arms, vibrato clamping, the
//! dead 0xFF pattern-jump, the nloops wrap, oversampling, the loud
//! `startplaying` arm and the 3.5M `turnbytes` cap). Group "ModEdges"
//! (`Suite.java` position 20). Pin source: `Mod.java:101-196`,
//! `ModSlayer.java:364-438,523-621,776-819` via `tests/java/TModEdges.java`;
//! narrative: notes/JAVA-BEHAVIOR/B2_mod_modslayer.md (the coverage-closure
//! section — the tracker's provisional B4 mapping is wrong: TModEdges is B2).
//!
//! Consumes `constants.toml [modslayer]` (`TModEdges.java:28`: the
//! `turnbytes_cap` row) and, like `TModSlayer`, `[audio_device].rate` as the
//! fixtures' `ModSlayer` sampling rate (the device rate,
//! `t_modslayer::device_rate`; C4 provenance wiring — the pin VALUES are
//! unchanged because the key's value is 8000).
//!
//! Port-shape mapping (documented at each pin):
//! - `new Mod("edges.zipo", app)` + `LoadMod(ByteArrayInputStream)` is the
//!   port's `Mod::from_zipo(None)` (the ctor's swallowed missing file) plus
//!   `load_mod_into(bytes)`; both are the public seam.
//! - the Java `Inst` row class is [`Inst`] here, and `build()` writes the
//!   identical byte image (magic always at 1080).
//! - `TModSlayer.fixture`/`TModSlayer.mti` are the shared helpers (widened to
//!   `pub(crate)` in `t_modslayer.rs`), exactly as the Java class calls them.
//! - the Java `Check.throwsIt(...ArrayIndexOutOfBoundsException)` maps to the
//!   port's bounds panic, asserted through the message ("index out of
//!   bounds"), the established `t_control`/`t_mod` mapping.

use std::panic::AssertUnwindSafe;

use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::t_modslayer::{device_rate, fixture, mti};
use crate::modslayer::{LOUD_VOL_ADJ, ModSlayer, SINTABLE};
use crate::tracker::{Mod, ModTrackInfo};

pub(crate) fn run() {
    check::group("ModEdges");
    // TModEdges.java:28 — the class-level fixture load.
    let km = Tml::load("constants.toml").table("modslayer");
    check::eq_i32(
        km.intv("turnbytes_cap"),
        3_500_000,
        "turnbytes_cap_key_3500000",
    );

    voice_detection();
    sequence_clamps();
    instrument_clamps();
    sample_repeat_copy();
    truncated_samples();
    slayer_edges();
    beattrack_edges();
    updatetracks_edges();
    turnbytes_edges(&km);
}

/// One instrument descriptor: u16 word counts as stored on disk
/// (`TModEdges.java:314-324`).
struct Inst {
    words: i32,
    point_words: i32,
    length_words: i32,
}

impl Inst {
    fn new(words: i32, point_words: i32, length_words: i32) -> Inst {
        Inst {
            words,
            point_words,
            length_words,
        }
    }
}

/// The FOURCC pin-key formula (`TModEdges.java:291-293`): the four chars as
/// `charAt(0)*1000 + charAt(1)*100 + charAt(2)*10 + charAt(3)`.
fn key(fourcc: &str) -> i32 {
    let b = fourcc.as_bytes();
    b[0] as i32 * 1000 + b[1] as i32 * 100 + b[2] as i32 * 10 + b[3] as i32
}

/// 128-byte position table with the listed slots set (`:296-302`).
fn positions(slots: &[i32]) -> Vec<i8> {
    let mut p = vec![0i8; 128];
    for (i, &v) in slots.iter().enumerate() {
        p[i] = v as i8;
    }
    p
}

/// n identical instrument descriptors (`:305-311`).
fn insts(n: usize, words: i32, point_words: i32, length_words: i32) -> Vec<Inst> {
    (0..n)
        .map(|_| Inst::new(words, point_words, length_words))
        .collect()
}

/// Builds one synthetic MOD (`TModEdges.java:331-378`). `insts.len()`
/// decides the instrument count the parser uses (31 or 15); the magic is
/// always written at the fixed 1080 read offset.
fn build(
    magic: &str,
    songlen: i32,
    repeat: i32,
    pos: &[i8],
    numpat: i32,
    numtracks: i32,
    ins: &[Inst],
    truncate: bool,
) -> Vec<u8> {
    let n_instr = ins.len() as i32;
    let seq_off = 20 + n_instr * 30;
    let pattern_off = seq_off + 2 + 128 + 4;
    let pattern_bytes = numpat * numtracks * 4 * 64;
    let mut sample_bytes = 0;
    for i in ins.iter() {
        sample_bytes += i.words * 2;
    }
    let mut size = pattern_off + pattern_bytes + sample_bytes;
    if size < 1084 {
        size = 1084;
    }
    let mut out = vec![0u8; size as usize];
    out[..4].copy_from_slice(b"edge");
    for (i, one) in ins.iter().enumerate() {
        let o = 20 + i as i32 * 30;
        out[o as usize] = (b'a' + (i as i32 % 26) as u8) as u8;
        put_u16(&mut out, o + 22, one.words);
        out[(o + 24) as usize] = 0; // finetune
        out[(o + 25) as usize] = 64; // volume
        put_u16(&mut out, o + 26, one.point_words);
        put_u16(&mut out, o + 28, one.length_words);
    }
    out[seq_off as usize] = songlen as u8;
    out[(seq_off + 1) as usize] = repeat as u8;
    for (i, &b) in pos.iter().enumerate() {
        out[(seq_off + 2 + i as i32) as usize] = b as u8;
    }
    for (i, ch) in magic.bytes().enumerate() {
        out[1080 + i] = ch;
    }
    for p in 0..numpat {
        out[(pattern_off + p * numtracks * 256) as usize] = (p + 1) as u8;
    }
    let so = pattern_off + pattern_bytes;
    for i in 0..sample_bytes {
        out[(so + i) as usize] = ((i % 8) + 1) as u8;
    }
    if truncate && sample_bytes > 0 {
        let cut = (size - sample_bytes / 2) as usize;
        return out[..cut].to_vec();
    }
    out
}

fn put_u16(b: &mut [u8], off: i32, v: i32) {
    b[off as usize] = ((v >> 8) & 0xFF) as u8;
    b[(off + 1) as usize] = (v & 0xFF) as u8;
}

/// `LoadMod` is the only public entrance (`:385-390`): the failed-ctor
/// object revived by the raw parse.
fn load(bytes: &[u8]) -> Mod {
    let mut m = Mod::from_zipo(None);
    m.load_mod_into(bytes).expect("synthetic MOD parses");
    m
}

/// Voice detection (`Mod.java:111-126`, `TModEdges.java:35-57`): every
/// listed FOURCC sets 31 instruments; only 8CHN/6CHN/28CH change numtracks.
/// FLT8 is the documented quirk: 31 instruments, 4 tracks.
fn voice_detection() {
    let magics = [
        "M.K.", "M!K!", "M&K!", "FLT4", "FLT8", "8CHN", "6CHN", "28CH",
    ];
    let tracks = [4, 4, 4, 4, 4, 8, 6, 28];
    for (magic, &want_tracks) in magics.iter().zip(tracks.iter()) {
        let m = load(&build(
            magic,
            3,
            2,
            &positions(&[0, 1, 2]),
            3,
            want_tracks,
            &insts(31, 64, 0, 0),
            false,
        ));
        let k = key(magic);
        check::eq_i32(
            m.get_num_tracks(),
            want_tracks,
            &format!("voice_{k}_tracks"),
        );
        check::eq_i32(
            m.insts.len() as i32,
            31,
            &format!("voice_{k}_31_instruments"),
        );
        check::eq_i32(m.num_patterns(), 3, &format!("voice_{k}_patterns"));
    }

    // an unlisted FOURCC leaves var3==15 (:103,118) — the 15-instrument
    // ProTracker fallback, numtracks still 4 (:104)
    let fifteen = load(&build(
        "ZZZZ",
        2,
        1,
        &positions(&[0, 1]),
        2,
        4,
        &insts(15, 64, 0, 0),
        false,
    ));
    check::eq_i32(
        fifteen.insts.len() as i32,
        15,
        "voice_unknown_15_instruments",
    );
    check::eq_i32(fifteen.get_num_tracks(), 4, "voice_unknown_tracks_4");
    check::eq_i32(fifteen.num_patterns(), 2, "voice_unknown_patterns_2");
    check::eq_str(fifteen.get_name(), "edge", "voice_unknown_name_trimmed");
}

/// readSequence repeat clamp (`Mod.java:86-98`, `TModEdges.java:59-73`).
fn sequence_clamps() {
    let clamped = load(&build(
        "M.K.",
        5,
        200,
        &positions(&[0, 3]),
        4,
        4,
        &insts(31, 64, 0, 0),
        false,
    ));
    check::eq_i32(clamped.song_length_patterns, 5, "seq_song_length_5");
    check::eq_i32(
        clamped.song_repeat_patterns,
        5,
        "seq_repeat_clamped_to_length",
    );
    check::eq_i32(clamped.num_patterns(), 4, "seq_numpatterns_max_pos_plus_1");
    let unclamped = load(&build(
        "M.K.",
        5,
        2,
        &positions(&[0]),
        1,
        4,
        &insts(31, 64, 0, 0),
        false,
    ));
    check::eq_i32(
        unclamped.song_repeat_patterns,
        2,
        "seq_repeat_kept_when_smaller",
    );
    // the all-zero position table is the "empty section" edge:
    // numpatterns == 1 (max 0 + 1, Mod.java:90-98)
    let empty = load(&build(
        "M.K.",
        0,
        0,
        &positions(&[]),
        1,
        4,
        &insts(31, 64, 0, 0),
        false,
    ));
    check::eq_i32(empty.num_patterns(), 1, "seq_empty_positions_one_pattern");
}

/// readInstrument clamps (`Mod.java:156-174`, `TModEdges.java:75-92`):
/// words are doubled; repeat_point clamps to sample_length and
/// repeat_length to sample_length - repeat_point.
fn instrument_clamps() {
    let mut cl = insts(31, 64, 0, 0);
    cl[0] = Inst::new(64, 200, 100);
    cl[1] = Inst::new(64, 0, 200);
    cl[2] = Inst::new(10, 2, 3);
    let clamps = load(&build("M.K.", 1, 0, &positions(&[0]), 1, 4, &cl, false));
    check::eq_i32(
        clamps.insts[0].sample_length,
        128,
        "inst_sample_length_words_x2",
    );
    check::eq_i32(
        clamps.insts[0].repeat_point,
        128,
        "inst_repeat_point_clamped",
    );
    check::eq_i32(
        clamps.insts[0].repeat_length,
        0,
        "inst_repeat_length_clamped_zero",
    );
    check::eq_i32(
        clamps.insts[1].repeat_length,
        128,
        "inst_repeat_length_clamped_to_sample",
    );
    // instrument 2: point 2 words (->4), length 3 words (->6) into a
    // 20-byte sample: 4+6 <= 20 so neither clamp fires
    check::eq_i32(clamps.insts[2].repeat_point, 4, "inst_repeat_point_kept");
    check::eq_i32(clamps.insts[2].repeat_length, 6, "inst_repeat_length_kept");
}

/// readSampleData repeat copy (`Mod.java:149-153`, `TModEdges.java:94-110`):
/// repeat_length > 3 copies 8 bytes from repeat_point to samples[sample_length].
fn sample_repeat_copy() {
    let mut rep_inst = insts(31, 64, 0, 0);
    rep_inst[0] = Inst::new(4, 0, 4);
    let rep = load(&build(
        "M.K.",
        1,
        0,
        &positions(&[0]),
        1,
        4,
        &rep_inst,
        false,
    ));
    check::eq_i32(rep.insts[0].samples[0] as i32, 1, "sample_first_byte");
    check::eq_i32(rep.insts[0].samples[7] as i32, 8, "sample_last_byte");
    check::eq_i32(
        rep.insts[0].samples[8] as i32,
        1,
        "sample_repeat_copy_first",
    );
    check::eq_i32(
        rep.insts[0].samples[15] as i32,
        8,
        "sample_repeat_copy_last",
    );
    // repeat_length <= 3 skips the copy: the tail stays zero
    let mut norep_inst = insts(31, 64, 0, 0);
    norep_inst[0] = Inst::new(4, 0, 1);
    let norep = load(&build(
        "M.K.",
        1,
        0,
        &positions(&[0]),
        1,
        4,
        &norep_inst,
        false,
    ));
    check::eq_i32(
        norep.insts[0].samples[8] as i32,
        0,
        "sample_short_repeat_no_copy",
    );
}

/// Truncated sample data (`Mod.java:138-144`, `TModEdges.java:112-119`): the
/// EOF is CAUGHT and only prints a warning, so the object stays parsed.
fn truncated_samples() {
    let mut eof_inst = insts(31, 64, 0, 0);
    eof_inst[0] = Inst::new(64, 0, 0);
    let eof = load(&build(
        "M.K.",
        1,
        0,
        &positions(&[0]),
        1,
        4,
        &eof_inst,
        true,
    ));
    check::eq_i32(eof.num_patterns(), 1, "eof_samples_patterns_kept");
    check::eq_i32(eof.get_num_tracks(), 4, "eof_samples_tracks_kept");
}

/// A ready ModSlayer with MOD limits installed on instrument 0
/// (`TModEdges.java:284-289`).
fn slayer() -> ModSlayer {
    let rate = device_rate();
    let m = fixture(4, false);
    let mut s = ModSlayer::new(m, rate, 350, 125);
    s.startplaying(false);
    s
}

/// `row()` / `row2()` (`TModEdges.java:254-281`): one get_track row with the
/// given effect/param on track 0, optionally starting from a caller-supplied
/// track.
fn row2(
    s: &mut ModSlayer,
    prev: Option<ModTrackInfo>,
    sample: i32,
    period: i32,
    effect: i32,
    param: i32,
) -> ModTrackInfo {
    let mut patt = vec![0i8; 16];
    patt[0] = ((sample & 0xF0) | ((period >> 8) & 0x0F)) as i8;
    patt[1] = (period & 0xFF) as i8;
    patt[2] = (((sample & 0x0F) << 4) | (effect & 0x0F)) as i8;
    patt[3] = param as i8;
    s.patt = patt;
    s.pattofs = 0;
    s.row = 0;
    s.tracks[0] = prev.unwrap_or_else(ModTrackInfo::new);
    s.updatetracks();
    s.tracks[0].clone()
}

fn row(s: &mut ModSlayer, sample: i32, period: i32, effect: i32, param: i32) -> ModTrackInfo {
    row2(s, None, sample, period, effect, param)
}

/// The get_track edge parses (`ModSlayer.java:623-773`, `TModEdges.java:121-164`).
fn slayer_edges() {
    let mut sl = slayer();

    // effect 0 == arpeggio: the period walk runs 12..47 and breaks at 48
    // when period(0) is below every threshold (:657-661)
    let t = row(&mut sl, 0, 0, 0, 5);
    check::eq_i32(t.effect, 16, "track_arp_effect_bit");
    check::eq_i32(t.arp[0], 107, "track_arp_period_at_48");
    check::eq_i32(t.arp[1], 80, "track_arp_plus_low_nibble_5");
    check::eq_i32(t.arpindex, 0, "track_arp_index_reset");

    // case 1 with a non-zero param sets port_up (:671-673)
    let t = row(&mut sl, 0, 0, 1, 7);
    check::eq_i32(t.effect, 4, "track_port_up_effect_bit");
    check::eq_i32(t.port_up, 7, "track_port_up_value");
    // param 0 leaves port_up at its default
    let t = row(&mut sl, 0, 0, 1, 0);
    check::eq_i32(t.port_up, 0, "track_port_up_zero_kept");

    // case 9 param 0 replays oldsampofs (:717-723)
    let t = row(&mut sl, 0, 0, 9, 5);
    check::eq_i32(t.position, 1280, "track_sample_offset_param_5");
    let mut old = ModTrackInfo::new();
    old.oldsampofs = 6;
    let t = row2(&mut sl, Some(old), 0, 0, 9, 0);
    check::eq_i32(t.position, 1536, "track_sample_offset_old_6_shl8");
    check::eq_i32(t.oldsampofs, 6, "track_sample_offset_old_kept");

    // case 12 volume: in-range keeps the param, out-of-range clamps 64
    let t = row(&mut sl, 0, 0, 12, 30);
    check::eq_i32(t.volume, 30, "track_set_volume_30");
    let t = row(&mut sl, 0, 0, 12, 70);
    check::eq_i32(t.volume, 64, "track_set_volume_clamps_64");

    // case 14: the E1/E2 sub-switch is DEAD ORIGINAL CODE. var9 is
    // `var7 & 240` (:737), i.e. always a multiple of 16, while the cases
    // test 1 and 2, so only the default arm can ever run.
    let t = row(&mut sl, 1, 855, 14, 0x15);
    check::eq_i32(t.period, 855, "track_e14_param_1x_noop_dead_switch");
    let t = row(&mut sl, 1, 115, 14, 0x25);
    check::eq_i32(t.period, 115, "track_e14_param_2x_noop_dead_switch");
    let t = row(&mut sl, 1, 300, 14, 0x05);
    check::eq_i32(t.period, 300, "track_e14_param_0x_noop");
}

/// beattrack remaining arms (`TModEdges.java:166-192`).
fn beattrack_edges() {
    let rate = device_rate();
    // PORT_TO both fall-short directions (:400-407)
    let mut slb = slayer();
    // approaching from BELOW: the else-if branch subtracts and then
    // pins portto (period 100-100=0 -> 500)
    let mut t = mti(32, 0, 100);
    t.portto = 500;
    t.port_inc = 100;
    slb.beattrack(&mut t);
    check::eq_i32(t.period, 500, "port_to_under_lands_at_portto");
    // portto == period: the else-if guard `portto > period` is false,
    // so the period is left alone (the second false arm, :405)
    let mut t = mti(32, 0, 100);
    t.portto = 100;
    t.port_inc = 100;
    slb.beattrack(&mut t);
    check::eq_i32(t.period, 100, "port_to_equal_leaves_period");
    // VIBRATO high-limit clamp (:420-428)
    let mut t = mti(8, 0, 855);
    t.vib_rate = 1;
    t.vib_depth = 128;
    t.period_low_limit = 113;
    t.period_high_limit = 856;
    t.finetune_rate = rate;
    slb.beattrack(&mut t);
    check::eq_i32(t.period, 855, "vibrato_period_untouched");
    check::eq_i32(
        t.pitch,
        rate / (855 + SINTABLE[1]),
        "vibrato_under_high_limit",
    );
}

/// updatetracks pattern-jump (DEAD) / nloops arms (`TModEdges.java:194-218`).
fn updatetracks_edges() {
    let rate = device_rate();
    // positions is byte[] (Mod.java:18), so a 0xFF byte reads as -1;
    // `positions[order_pos] == 255` (:536) can never be true and the
    // jump/phrase-reset body :537-539 is dead original code. The -1
    // falls through and indexes patterns[-1] at :541 (original bug).
    let mut m_jump = fixture(4, false);
    m_jump.positions[1] = -1i8;
    let mut s_jump = ModSlayer::new(m_jump, rate, 350, 125);
    s_jump.startplaying(false);
    s_jump.order_pos = 1;
    check::throws_it(
        AssertUnwindSafe(|| {
            s_jump.updatetracks();
        }),
        "index out of bounds",
        "jump_0xff_indexes_minus1_aioobe",
    );

    // nloops 2 -> one end-of-song wrap leaves mod_done false (:526-531)
    let mut m_two = fixture(4, false);
    m_two.song_length_patterns = 0;
    let mut s_two = ModSlayer::new(m_two, rate, 350, 125);
    s_two.startplaying(false);
    s_two.nloops = 2;
    s_two.updatetracks();
    check::eq_i32(s_two.nloops, 1, "nloops_decrements_to_1");
    check::ok(!s_two.mod_done, "nloops_two_not_done");
}

/// turnbytes oversampling / loud arm / the 3.5M cap (`TModEdges.java:220-251`).
fn turnbytes_edges(km: &Tml) {
    let rate = device_rate();
    // oversample==2 -> pairwise average; oversample==3 -> integer mean
    let m_os = fixture(4, false);
    let mut s_os = ModSlayer::new(m_os, rate, 350, 125);
    s_os.oversample = 2;
    check::eq_i32(
        s_os.turnbytes().len() as i32,
        123_840,
        "oversample2_len_123840",
    );
    let m_os3 = fixture(4, false);
    let mut s_os3 = ModSlayer::new(m_os3, rate, 350, 125);
    s_os3.oversample = 3;
    check::eq_i32(
        s_os3.turnbytes().len() as i32,
        123_840,
        "oversample3_len_123840",
    );

    // loud startplaying arm (:777) - loud_vol_adj instead of normal
    let m_loud = fixture(4, false);
    let mut s_loud = ModSlayer::new(m_loud, rate, 350, 125);
    s_loud.startplaying(true);
    // Java `Check.eq(sLoud.vol_adj, ModSlayer.loud_vol_adj)` compares the
    // array REFERENCES (identity); the port compares the slice pointers.
    check::ok(
        std::ptr::eq(s_loud.vol_adj, &LOUD_VOL_ADJ[..]),
        "startplaying_loud_vol_adj",
    );

    // turnbytes 3.5M cap (:598-603): a long silent song with bpm 60 at
    // 44.1k yields 1837 samples per tick; 8 patterns x 64 rows x 6 ticks =
    // 3072 ticks > 3.5M samples, so var6 stops at the cap and the output
    // length is exactly 3500000.
    let mut m_long = fixture(4, false);
    for i in 0..128 {
        m_long.positions[i] = 0;
    }
    m_long.song_length_patterns = 8;
    m_long.song_repeat_patterns = 8;
    m_long.patterns = vec![vec![0i8; 4 * 4 * 64]; 64];
    let mut s_long = ModSlayer::new(m_long, 44100, 350, 60);
    let capped = s_long.turnbytes();
    check::eq_i32(
        capped.len() as i32,
        km.intv("turnbytes_cap"),
        "turnbytes_caps_at_3500000",
    );
    check::ok(s_long.mod_done, "turnbytes_long_song_done");
}
