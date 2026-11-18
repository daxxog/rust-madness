// FIDELITY: TModSlayer.java pins the Java finetune formulas verbatim: `(1536 - 0)` and `(428 * 1)`.
#![allow(clippy::identity_op)]
//! `TModSlayer.java` — the six live beattrack effect handlers, the
//! startplaying formulas, updatetracks' pattern-advance state machine and
//! the E2E stage1 `turnbytes()` digest at the game's exact parameters.
//! Group "ModSlayer" (`Suite.java` position 4). Pin source:
//! `ModSlayer.java:364-438,505-551,776-819`; narrative:
//! notes/JAVA-BEHAVIOR/B2_mod_modslayer.md.
//!
//! One TOML fact: `constants.toml [audio_device].rate` is the fixtures'
//! `ModSlayer` sampling rate (the pinned device rate, [`device_rate`]);
//! everything else is a formula or an observed state-machine outcome. The
//! `fixture()` helper mirrors the Java one exactly — a BROKEN
//! `Mod` (the Java ctor catches the missing `fixture.zipo`, leaving default
//! fields) with the mixer fields then written by hand — and the E2E row
//! loads the real `assets/music/stage1.zipo`, whose md5 is also the port's
//! stored digest (`modslayer` module tests).

use std::sync::OnceLock;

use crate::hash::md5_hex;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::mirror::util::crc32;
use crate::modslayer::{ModSlayer, SINTABLE};
use crate::tracker::{Mod, ModInstrument, ModTrackInfo};

const STAGE1_ZIPO: &[u8] = include_bytes!("../../../../assets/music/stage1.zipo");

/// `constants.toml [audio_device].rate` (constants.toml:204-207; B13 §1.1):
/// the ULAW device rate. The fixtures run `ModSlayer` AT the device rate, so
/// their sampling-rate arguments and every expected-value pitch/`finetune`
/// divisor read this key instead of a second copy of 8000. Provenance
/// wiring, not a value change: the key IS 8000 (Phase A accepted-open
/// micro-item C4; notes/JAVA-BEHAVIOR/B2_mod_modslayer.md's coverage-closure
/// section — the TModEdges group).
pub(crate) fn device_rate() -> i32 {
    static RATE: OnceLock<i32> = OnceLock::new();
    *RATE.get_or_init(|| {
        Tml::load("constants.toml")
            .table("audio_device")
            .intv("rate")
    })
}

pub(crate) fn run() {
    check::group("ModSlayer");
    effects();
    startplaying_formulas();
    updatetracks_machine();
    e2e_stage1();
}

/// A `ModTrackInfo` with one effect bit, a volume and a period
/// (`TModSlayer.java:217-226`). `pub(crate)`: TModEdges's beattrack arms call
/// it exactly like the Java `TModSlayer.mti` helper.
pub(crate) fn mti(effect: i32, volume: i32, period: i32) -> ModTrackInfo {
    let mut t = ModTrackInfo::new();
    t.effect = effect;
    t.volume = volume;
    t.period = period;
    t.finetune_rate = device_rate();
    t.period_low_limit = 1;
    t.period_high_limit = 99999;
    t
}

/// The hand-built broken-ctor Mod with the fields a mixer needs
/// (`TModSlayer.java:229-243`). `Mod::from_zipo(None)` is exactly the
/// object Java's `new Mod("fixture.zipo", app)` leaves behind after its
/// swallowed `FileNotFoundException`; every field below is then assigned
/// as in the Java helper. `pub(crate)`: TModEdges reuses it verbatim, like
/// the Java class's `TModSlayer.fixture` call.
pub(crate) fn fixture(numtracks: i32, s3m: bool) -> Mod {
    let mut m = Mod::from_zipo(None);
    m.s3m = s3m;
    m.numtracks = numtracks;
    m.insts = vec![ModInstrument {
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
    }];
    m.insts[0].finetune_value = 0;
    m.positions = vec![0i8; 128];
    m.positions[0] = 0;
    m.positions[1] = 0;
    m.song_length_patterns = 2;
    m.song_repeat_patterns = 2;
    m.patterns = (0..256)
        .map(|_| vec![0i8; (numtracks * 4 * 64) as usize])
        .collect();
    m.name = "fixture".to_string();
    m
}

/// The beattrack effect handlers (ModSlayer.java:364-438).
fn effects() {
    let rate = device_rate();
    // guard: period_low_limit == 0 becomes 1 (division protection)
    let mut t = ModTrackInfo::new();
    t.period_low_limit = 0;
    t.period_high_limit = 99999;
    let m4 = fixture(4, false);
    let mut sl = ModSlayer::new(m4, rate, 350, 125);
    t.finetune_rate = rate;
    sl.beattrack(&mut t);
    check::eq_i32(t.period_low_limit, 1, "guard_low_limit");

    // VOL_SLIDE (effect&1): volume clamped to [0, 64]
    let mut t = mti(1, 10, 0);
    t.vol_slide = 5;
    sl.beattrack(&mut t);
    check::eq_i32(t.volume, 15, "vol_slide_add");
    let mut t = mti(1, 70, 0);
    sl.beattrack(&mut t);
    check::eq_i32(t.volume, 64, "vol_slide_high_clamp");
    let mut t = mti(1, -5, 0);
    sl.beattrack(&mut t);
    check::eq_i32(t.volume, 0, "vol_slide_low_clamp");

    // PORT_DOWN (effect&2): period += port_down, clamp to high_limit,
    // then pitch = finetune_rate / period
    let mut t = mti(2, 0, 100);
    t.port_down = 50;
    t.period_high_limit = 1712;
    sl.beattrack(&mut t);
    check::eq_i32(t.period, 150, "port_down_add");
    check::eq_i32(t.pitch, rate / 150, "port_down_pitch");
    let mut t = mti(2, 0, 1700);
    t.port_down = 50;
    t.period_high_limit = 1712;
    sl.beattrack(&mut t);
    check::eq_i32(t.period, 1712, "port_down_clamp");

    // PORT_UP (effect&4): the S3M underflow quirk — clamps to the
    // period_HIGH_limit on s3m, the low limit otherwise
    let ms3m = fixture(4, true);
    let mut sls3m = ModSlayer::new(ms3m, rate, 350, 125);
    let mut t = mti(4, 0, 50);
    t.port_up = 100;
    t.period_low_limit = 10;
    t.period_high_limit = 200;
    sls3m.beattrack(&mut t);
    check::eq_i32(t.period, 200, "port_up_s3m_underflow_goes_HIGH");
    let mut t = mti(4, 0, 50);
    t.port_up = 100;
    t.period_low_limit = 10;
    t.period_high_limit = 200;
    sl.beattrack(&mut t);
    check::eq_i32(t.period, 10, "port_up_mod_underflow_goes_low");

    // PORT_TO (effect&32): approaches portto from both sides
    let mut t = mti(32, 0, 100);
    t.portto = 500;
    t.port_inc = 700;
    sl.beattrack(&mut t);
    check::eq_i32(t.period, 500, "port_to_up_lands");
    let mut t = mti(32, 0, 500);
    t.portto = 100;
    t.port_inc = 700;
    sl.beattrack(&mut t);
    check::eq_i32(t.period, 100, "port_to_down_lands");

    // VIBRATO (effect&8): vibpos += rate<<2; second sine half (bit 128)
    // NEGATES the offset; pitch = finetune_rate / (period+off)
    let mut t = mti(8, 0, 0);
    t.vib_rate = 1;
    t.vib_depth = 128;
    t.period_low_limit = 1;
    t.period_high_limit = 99999;
    sl.beattrack(&mut t);
    check::eq_i32(t.vibpos, 4, "vibrato_phase_step");
    check::eq_i32(t.pitch, rate / SINTABLE[1], "vibrato_first_half");

    let mut t = mti(8, 0, 1000);
    t.vibpos = 124;
    t.vib_rate = 2;
    t.vib_depth = 128;
    t.period_low_limit = 1;
    t.period_high_limit = 99999;
    sl.beattrack(&mut t);
    // vibpos 124+8=132: index 132>>2&31 = 1 (same entry), but bit 128 set
    // -> the offset NEGATES: period 1000 - sintable[1]
    check::eq_i32(t.vibpos, 132, "vibrato_phase_b");
    check::eq_i32(
        t.pitch,
        rate / (1000 - SINTABLE[1]),
        "vibrato_second_half_negates",
    );

    // ARPEGGIO (effect&16): pitch cycles through arp[0..2], wrap at 3
    let mut t = mti(16, 0, 0);
    t.finetune_rate = rate;
    t.arp[0] = 100;
    t.arp[1] = 200;
    t.arp[2] = 400;
    sl.beattrack(&mut t);
    check::eq_i32(t.pitch, 80, "arpeggio_0");
    check::eq_i32(t.arpindex, 1, "arpeggio_index_1");
    sl.beattrack(&mut t);
    check::eq_i32(t.pitch, 40, "arpeggio_1");
    sl.beattrack(&mut t);
    check::eq_i32(t.pitch, 20, "arpeggio_2");
    check::eq_i32(t.arpindex, 0, "arpeggio_wrap");
}

/// startplaying's formulas (ModSlayer.java:776-819). The Java class reads
/// the `Mod` it handed to the constructor (mutated in place); the port owns
/// the module, so the same fields are read through `song`.
fn startplaying_formulas() {
    let rate = device_rate();
    // MOD finetune: 22748294283264 / (mixspeed * (1536 - value))
    let mut m1 = fixture(4, false);
    m1.insts[0].finetune_value = 0;
    let mut s1 = ModSlayer::new(m1, rate, 350, 125);
    s1.startplaying(false);
    check::eq_i32(
        s1.song.insts[0].finetune_rate,
        (22748294283264i64 / (i64::from(rate) * (1536 - 0))) as i32,
        "mod_finetune_formula",
    );
    check::eq_i32(s1.song.insts[0].period_low_limit, 113, "mod_period_low");
    check::eq_i32(s1.song.insts[0].period_high_limit, 856, "mod_period_high");
    check::eq_i32(
        s1.bpm_samples,
        rate / (24 * 125 / 60),
        "bpm_samples_int_math",
    );

    // S3M finetune: (428 * value << 8) / mixspeed
    let mut m1s = fixture(4, true);
    m1s.insts[0].finetune_value = 1;
    let mut s1s = ModSlayer::new(m1s, rate, 350, 125);
    s1s.startplaying(false);
    check::eq_i32(
        s1s.song.insts[0].finetune_rate,
        ((428i64 * 1) << 8) as i32 / rate,
        "s3m_finetune_formula",
    );
    check::eq_i32(s1s.song.insts[0].period_low_limit, 14, "s3m_period_low");
    check::eq_i32(s1s.song.insts[0].period_high_limit, 1712, "s3m_period_high");

    // vol_shift by numtracks: >8 -> 2, >4 -> 1, else 0
    let m6 = fixture(6, false);
    let mut s6 = ModSlayer::new(m6, rate, 350, 125);
    s6.startplaying(false);
    check::eq_i32(s6.vol_shift, 1, "vol_shift_6tracks");
    let m9 = fixture(9, false);
    let mut s9 = ModSlayer::new(m9, rate, 350, 125);
    s9.startplaying(false);
    check::eq_i32(s9.vol_shift, 2, "vol_shift_9tracks");
    check::eq_i32(s1.vol_shift, 0, "vol_shift_4tracks");

    // make_vol_table8 runs only when !bit16 (vol_table is byte[16640]);
    // the port's Vec stands in for Java's array — empty == null
    s1.startplaying(false);
    check::eq_i32(s1.vol_table.len() as i32, 16640, "vol_table8_built");
    let m1b = fixture(4, false);
    let mut s1b = ModSlayer::new(m1b, rate, 350, 125);
    s1b.bit16 = true;
    s1b.startplaying(false);
    check::ok(s1b.vol_table.is_empty(), "bit16_skips_vol_table8");
}

/// updatetracks: the pattern-advance state machine (ModSlayer.java:523-551).
fn updatetracks_machine() {
    let rate = device_rate();
    let madv = fixture(4, false);
    let mut sadv = ModSlayer::new(madv, rate, 350, 125);
    sadv.startplaying(false);
    // startplaying leaves row=64: the first call wraps to row 1,
    // order_pos 0 -> 1, tempo_wait reset to the tempo
    sadv.updatetracks();
    check::eq_i32(sadv.row, 1, "updatetracks_row_wraps");
    check::eq_i32(sadv.order_pos, 1, "updatetracks_order_advances");
    check::eq_i32(sadv.tempo_wait, 6, "updatetracks_tempo_wait");
    // pattofs after one row = 4 bytes per note x numtracks — the get_track
    // stride
    check::eq_i32(sadv.pattofs, 4 * 4, "updatetracks_pattofs_row_stride");
    // 64 more calls: row walks 2..64, then the 65th call advances the
    // order again (positions[1] = 0 in the fixture)
    for _ in 0..63 {
        sadv.updatetracks();
    }
    check::eq_i32(sadv.row, 64, "updatetracks_row_walks");
    check::eq_i32(sadv.order_pos, 1, "updatetracks_stays_until_wrap");
    sadv.updatetracks();
    check::eq_i32(sadv.order_pos, 2, "updatetracks_order_wraps_every_64");

    // mod_done: song_length 0 forces the wrap; nloops 1 -> 0 ends it
    let mut mdone = fixture(4, false);
    mdone.song_length_patterns = 0;
    let mut sdone = ModSlayer::new(mdone, rate, 350, 125);
    sdone.startplaying(false);
    check::ok(!sdone.mod_done, "mod_done_not_yet");
    sdone.updatetracks();
    check::ok(sdone.mod_done, "mod_done_after_wrap");
    check::eq_i32(sdone.nloops, 0, "mod_done_nloops");
}

/// E2E: the game's real stage1 render (xtGraphics.java:1250 ->
/// `RadicalMod("music/stage1.zipo", 350, 8400, 135)` ->
/// `ModSlayer(mod, 8400, 350, 135)`); the md5 EQUALS the port's stored
/// digest — the mixer is byte-faithful.
fn e2e_stage1() {
    // Java `new Mod("stage1.zipo", app)` — the ctor's zip step takes the
    // container's first entry and LoadMods it (TMod's ctor_path recipe).
    let stage1 = Mod::from_zipo(Some(STAGE1_ZIPO));
    let mut se2e = ModSlayer::new(stage1, 8400, 350, 135);
    let out = se2e.turnbytes();
    check::eq_i32(out.len() as i32, 1191330, "stage1_len");
    check::eq_i64(crc32(&out) as i64, 4025273850, "stage1_crc32");
    let raw: Vec<u8> = out.iter().map(|&b| b as u8).collect();
    check::eq_str(
        md5_hex(&raw),
        "f1f2a25eb468517faa2218cf1712395f",
        "stage1_md5",
    );
}
