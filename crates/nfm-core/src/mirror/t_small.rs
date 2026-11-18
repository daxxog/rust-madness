//! `TSmall.java` — the five SMALL support classes: `ModInstrument` +
//! `ModTrackInfo` (pure data containers), `Trackers` (the 500-slot wheel
//! tracker ring), `SuperStream` (the wrap-forever stream) and
//! `CheckPoints` (the race-ranking pass). Group "Small" (`Suite.java`
//! position 7). Pin source: `ModInstrument.java`, `ModTrackInfo.java`,
//! `Trackers.java`, `SuperStream.java:4-31`, `CheckPoints.java:2-86`;
//! narrative: notes/JAVA-BEHAVIOR/B1_audio_tiny.md.
//!
//! Consumes the WHOLE `constants.toml` (Java `KC` = the root load,
//! `TSmall.java:23`) and binds the two sub-tables it reads: `[wheels]`'s
//! 500-slot ring and `[madness]`'s 140-entry `typ[]`.
//!
//! Port-shape mapping (documented at each pin): Java's `null` array/name
//! fields are the port's empty `Vec`/`String`, and the two reflection pins
//! (`declaredFieldCount`/`declaredMethodCount`) have no Rust runtime
//! analogue — they are pinned through an exhaustive destructure plus a
//! counted binding list, and a documented literal respectively.

use std::panic::AssertUnwindSafe;

use crate::checkpoints::{CP_CAP, CheckPoints, PLAYERS};
use crate::conto::{Base, Conto};
use crate::madness::Madness;
use crate::medium::Medium;
use crate::mirror::fixture::Tml;
use crate::mirror::harness as check;
use crate::radicalmod::SuperStream;
use crate::tracker::{ModInstrument, ModTrackInfo};
use crate::trackers::{TRACKER_CAP, Trackers};

pub(crate) fn run() {
    check::group("Small");
    // TSmall.java:23-25 — the root load and the two derived lengths.
    let kc = Tml::load("constants.toml");
    let trackers = kc.table("wheels").intv("trackers_slots");
    let cp_typ = kc.table("madness").intv("focus_scan_len");
    kc.table("wheels").require_keys(&["trackers_slots"]);
    kc.table("madness").require_keys(&["focus_scan_len"]);

    mod_instrument();
    mod_track_info();
    trackers_pins(trackers);
    super_stream();
    check_points(cp_typ);
}

/// Counts the arguments (all zero-sized probes), so the count comes from
/// the same list the exhaustive destructure binds — Rust has no reflection.
macro_rules! field_count {
    ($($f:expr),* $(,)?) => {
        [$( { let _ = &$f; 0u8 } ),*].len()
    };
}

/// `ModInstrument.java:1-12` — a data container, so the implicit default
/// ctor (the only code) leaves every field at its declared state.
fn mod_instrument() {
    let mut mi = ModInstrument::new();
    // Java's `null` name/samples arrays are the port's empty containers.
    check::ok(mi.name.is_empty(), "mi_name_null");
    check::ok(mi.samples.is_empty(), "mi_samples_null");
    check::eq_i32(mi.sample_length, 0, "mi_sample_length_0");
    check::eq_i32(mi.finetune_rate, 0, "mi_finetune_rate_0");
    check::eq_i32(mi.period_low_limit, 0, "mi_period_low_limit_0");
    check::eq_i32(mi.period_high_limit, 0, "mi_period_high_limit_0");
    check::eq_i32(mi.finetune_value, 0, "mi_finetune_value_0");
    check::eq_i32(mi.volume, 0, "mi_volume_0");
    check::eq_i32(mi.repeat_point, 0, "mi_repeat_point_0");
    check::eq_i32(mi.repeat_length, 0, "mi_repeat_length_0");
    // 10 fields total (ModInstrument.java:2-11); the destructure fails to
    // compile if the port's field set drifts, and the count is the binding
    // list's length.
    let ModInstrument {
        name,
        samples,
        sample_length,
        finetune_rate,
        period_low_limit,
        period_high_limit,
        finetune_value,
        volume,
        repeat_point,
        repeat_length,
    } = &mi;
    check::eq_i32(
        field_count!(
            name,
            samples,
            sample_length,
            finetune_rate,
            period_low_limit,
            period_high_limit,
            finetune_value,
            volume,
            repeat_point,
            repeat_length
        ) as i32,
        10,
        "mi_field_count_10",
    );

    // the container is writable and per-instance, not shared
    mi.sample_length = 1234;
    mi.volume = 64;
    let other = ModInstrument::new();
    check::eq_i32(mi.sample_length, 1234, "mi_write_sample_length");
    check::eq_i32(other.sample_length, 0, "mi_second_instance_independent");
    check::eq_i32(other.volume, 0, "mi_second_instance_volume_0");
}

/// `ModTrackInfo.java:1-38` — the ctor (:35-37) only allocates `arp[3]`;
/// every other field keeps its initializer, `oldperiod` starting at 1.
fn mod_track_info() {
    let mut ti = ModTrackInfo::new();
    check::ok(ti.samples.is_empty(), "ti_samples_null");
    check::eq_i32(ti.position, 0, "ti_position_0");
    check::eq_i32(ti.length, 0, "ti_length_0");
    check::eq_i32(ti.repeat, 0, "ti_repeat_0");
    check::eq_i32(ti.replen, 0, "ti_replen_0");
    check::eq_i32(ti.volume, 0, "ti_volume_0");
    check::eq_i32(ti.error, 0, "ti_error_0");
    check::eq_i32(ti.pitch, 0, "ti_pitch_0");
    check::eq_i32(ti.old_position, 0, "ti_old_position_0");
    check::eq_i32(ti.start_period, 0, "ti_start_period_0");
    check::eq_i32(ti.period, 0, "ti_period_0");
    check::eq_i32(ti.step, 0, "ti_step_0");
    check::eq_i32(ti.effect, 0, "ti_effect_0");
    check::eq_i32(ti.portto, 0, "ti_portto_0");
    check::eq_i32(ti.vibpos, 0, "ti_vibpos_0");
    check::eq_i32(ti.trempos, 0, "ti_trempos_0");
    check::eq_i32(ti.oldsampofs, 0, "ti_oldsampofs_0");
    // the ctor's ONLY write (:36)
    check::eq_i32(ti.arp.len() as i32, 3, "ti_arp_length_3");
    check::eq_i32(ti.arp[0], 0, "ti_arp0_0");
    check::eq_i32(ti.arp[1], 0, "ti_arp1_0");
    check::eq_i32(ti.arp[2], 0, "ti_arp2_0");
    check::eq_i32(ti.arpindex, 0, "ti_arpindex_0");
    check::eq_i32(ti.oldperiod, 1, "ti_oldperiod_1");
    check::eq_i32(ti.vol_slide, 0, "ti_vol_slide_0");
    check::eq_i32(ti.port_inc, 0, "ti_port_inc_0");
    check::eq_i32(ti.port_up, 0, "ti_port_up_0");
    check::eq_i32(ti.port_down, 0, "ti_port_down_0");
    check::eq_i32(ti.vib_rate, 0, "ti_vib_rate_0");
    check::eq_i32(ti.vib_depth, 0, "ti_vib_depth_0");
    check::eq_i32(ti.trem_rate, 0, "ti_trem_rate_0");
    check::eq_i32(ti.trem_depth, 0, "ti_trem_depth_0");
    check::eq_i32(ti.retrig as i32, 0, "ti_retrig_0");
    check::eq_i32(ti.finetune_rate, 0, "ti_finetune_rate_0");
    check::eq_i32(ti.period_low_limit, 0, "ti_period_low_limit_0");
    check::eq_i32(ti.period_high_limit, 0, "ti_period_high_limit_0");
    // 32 fields (ModTrackInfo.java:2-21 + the ctor's arp) — same
    // compile-time shape guard as `mi_field_count_10`.
    let ModTrackInfo {
        samples,
        position,
        length,
        repeat,
        replen,
        volume,
        error,
        pitch,
        old_position,
        start_period,
        period,
        step,
        effect,
        portto,
        vibpos,
        trempos,
        oldsampofs,
        arp,
        arpindex,
        oldperiod,
        vol_slide,
        port_inc,
        port_up,
        port_down,
        vib_rate,
        vib_depth,
        trem_rate,
        trem_depth,
        retrig,
        finetune_rate,
        period_low_limit,
        period_high_limit,
    } = &ti;
    check::eq_i32(
        field_count!(
            samples,
            position,
            length,
            repeat,
            replen,
            volume,
            error,
            pitch,
            old_position,
            start_period,
            period,
            step,
            effect,
            portto,
            vibpos,
            trempos,
            oldsampofs,
            arp,
            arpindex,
            oldperiod,
            vol_slide,
            port_inc,
            port_up,
            port_down,
            vib_rate,
            vib_depth,
            trem_rate,
            trem_depth,
            retrig,
            finetune_rate,
            period_low_limit,
            period_high_limit
        ) as i32,
        32,
        "ti_field_count_32",
    );

    // arp is allocated fresh per instance - a write must not leak
    ti.arp[1] = 7;
    let other = ModTrackInfo::new();
    check::eq_i32(ti.arp[1], 7, "ti_arp_write_visible");
    check::eq_i32(other.arp[1], 0, "ti_arp_per_instance");
    // Java compares array identity (`ti.arp != other.arp`); owned `[i32; 3]`
    // fields cannot alias, and a shared array would have carried ti's 7 into
    // other — so the contents test is the observable side of the identity
    // statement.
    check::ok(ti.arp != other.arp, "ti_arp_distinct_array");
}

/// `Trackers.java:1-14` — no explicit ctor and no methods at all; the
/// implicit ctor allocates the eleven 500-slot arrays (`c` is
/// `int[500][3]`) and `nt` starts at 0.
fn trackers_pins(slots: i32) {
    let mut t = Trackers::new();
    check::eq_i32(t.x.len() as i32, slots, "tr_x_len_500");
    check::eq_i32(t.y.len() as i32, slots, "tr_y_len_500");
    check::eq_i32(t.z.len() as i32, slots, "tr_z_len_500");
    check::eq_i32(t.xy.len() as i32, slots, "tr_xy_len_500");
    check::eq_i32(t.zy.len() as i32, slots, "tr_zy_len_500");
    check::eq_i32(t.skd.len() as i32, slots, "tr_skd_len_500");
    check::eq_i32(t.dam.len() as i32, slots, "tr_dam_len_500");
    check::eq_i32(t.c.len() as i32, slots, "tr_c_len_500");
    check::eq_i32(t.radx.len() as i32, slots, "tr_radx_len_500");
    check::eq_i32(t.radz.len() as i32, slots, "tr_radz_len_500");
    check::eq_i32(t.rady.len() as i32, slots, "tr_rady_len_500");
    check::eq_i32(t.nt as i32, 0, "tr_nt_0");
    // every row of the collision table is a fresh 3-slot array
    check::eq_i32(t.c[0].len() as i32, 3, "tr_c_row0_len_3");
    check::eq_i32(t.c[TRACKER_CAP - 1].len() as i32, 3, "tr_c_row499_len_3");
    check::eq_i32(t.c[0][2], 0, "tr_c_row0_slot2_0");
    check::eq_i32(t.x[TRACKER_CAP - 1], 0, "tr_x_last_0");
    // `t.c[0] != t.c[1]` in Java compares array references; Rust arrays are
    // values, so the identity test is a pointer comparison of the two rows.
    check::ok(!std::ptr::eq(&t.c[0], &t.c[1]), "tr_c_rows_distinct");
    // no methods are declared on Trackers; it is pure storage. Rust has no
    // reflection: the port declares only `new()` (the implicit-ctor
    // analogue Java does not report either), so the count is the literal 0
    // — a behaviour method added later must revisit this id.
    check::eq_i32(0, 0, "tr_no_declared_methods");
    // the slot writes drive reads at Madness.java:1336/1342/1353
    t.nt = 1;
    t.skd[0] = 3;
    t.dam[0] = 12;
    t.c[0][1] = 9;
    check::eq_i32(t.nt as i32, 1, "tr_nt_write");
    check::eq_i32(t.skd[0], 3, "tr_skd_write");
    check::eq_i32(t.dam[0], 12, "tr_dam_write");
    check::eq_i32(t.c[0][1], 9, "tr_c_write");
    let other = Trackers::new();
    check::eq_i32(other.nt as i32, 0, "tr_second_instance_nt_0");
    check::eq_i32(other.skd[0], 0, "tr_second_instance_skd_0");
}

/// `SuperStream.java:4-31` — `ByteArrayInputStream` that WRAPS at EOF
/// instead of reporting it. The empty-buffer `read(b,off,len)` with len>0
/// and the `checkstat` typ scan are deliberately unpinned (they cannot
/// terminate; TSmall.java:13-16).
fn super_stream() {
    // :4-6 delegates to ByteArrayInputStream(byte[]), which keeps the
    // caller's array by reference (no defensive copy)
    let shared = vec![1i8, 2, 3];
    let mut ss = SuperStream::new(Some(shared));
    check::eq_i32(ss.available() as i32, 3, "ss_ctor_available_3");
    // Rust moves the Vec, so "the caller's array" is the stream's buffer
    // itself: a write through it is what the Java caller's `shared[0] = 9`
    // is. There is no copy in between.
    ss.data[0] = 9;
    check::eq_i32(ss.read_byte(), 9, "ss_ctor_no_copy_sees_mutation");
    check::eq_i32(ss.available() as i32, 2, "ss_read_advances_available_2");

    // read(): bytes in order, then the EOF re-read wraps to buf[0]
    let mut w = SuperStream::new(Some(vec![10, 20, 30]));
    check::eq_i32(w.read_byte(), 10, "ss_read_10");
    check::eq_i32(w.read_byte(), 20, "ss_read_20");
    check::eq_i32(w.available() as i32, 1, "ss_available_1");
    check::eq_i32(w.read_byte(), 30, "ss_read_30");
    check::eq_i32(w.available() as i32, 0, "ss_available_0_at_eof");
    // :9-13 - super.read()==-1 -> reset() -> super.read() == buf[0]
    check::eq_i32(w.read_byte(), 10, "ss_read_eof_wraps_to_first");
    check::eq_i32(w.read_byte(), 20, "ss_read_after_wrap_20");
    check::eq_i32(w.available() as i32, 1, "ss_available_1_after_wrap");

    // zero-length buffer: the wrap attempt also fails, so -1 forever
    let mut empty = SuperStream::new(Some(Vec::new()));
    check::eq_i32(empty.available() as i32, 0, "ss_empty_available_0");
    check::eq_i32(empty.read_byte(), -1, "ss_empty_read_minus1");
    check::eq_i32(empty.read_byte(), -1, "ss_empty_read_minus1_again");

    // one-byte buffer: the same byte forever
    let mut one = SuperStream::new(Some(vec![42]));
    check::eq_i32(one.read_byte(), 42, "ss_single_read_42");
    check::eq_i32(one.available() as i32, 0, "ss_single_available_0");
    check::eq_i32(one.read_byte(), 42, "ss_single_wraps");
    check::eq_i32(one.read_byte(), 42, "ss_single_wraps_again");

    // read(b,off,len) exact fit: one super.read, no reset, returns len
    let mut dst = [-1i8; 8];
    let mut r2 = SuperStream::new(Some(vec![1, 2]));
    check::eq_i32(
        r2.read_at(&mut dst, 0, 2) as i32,
        2,
        "ss_read_exact_returns_2",
    );
    check::eq_i32(dst[0] as i32, 1, "ss_read_exact_dst0");
    check::eq_i32(dst[1] as i32, 2, "ss_read_exact_dst1");
    check::eq_i32(dst[2] as i32, -1, "ss_read_exact_tail_untouched");
    check::eq_i32(r2.available() as i32, 0, "ss_read_exact_available_0");

    // len > buffer: copies the tail, resets, copies the head (:25-27)
    let mut dst5 = [0i8; 5];
    let mut r5 = SuperStream::new(Some(vec![1, 2, 3]));
    check::eq_i32(
        r5.read_at(&mut dst5, 0, 5) as i32,
        5,
        "ss_read_overlong_returns_5",
    );
    check::eq_i32(dst5[0] as i32, 1, "ss_read_overlong_dst0");
    check::eq_i32(dst5[1] as i32, 2, "ss_read_overlong_dst1");
    check::eq_i32(dst5[2] as i32, 3, "ss_read_overlong_dst2");
    check::eq_i32(dst5[3] as i32, 1, "ss_read_overlong_dst3_wrapped");
    check::eq_i32(dst5[4] as i32, 2, "ss_read_overlong_dst4_wrapped");
    check::eq_i32(r5.available() as i32, 1, "ss_read_overlong_available_1");

    // the offset variant must write only from off and respect the wrap
    let mut dst_o = [-1i8; 6];
    let mut ro = SuperStream::new(Some(vec![7, 8, 9]));
    check::eq_i32(
        ro.read_at(&mut dst_o, 2, 4) as i32,
        4,
        "ss_read_offset_returns_4",
    );
    check::eq_i32(dst_o[0] as i32, -1, "ss_read_offset_before0_untouched");
    check::eq_i32(dst_o[1] as i32, -1, "ss_read_offset_before1_untouched");
    check::eq_i32(dst_o[2] as i32, 7, "ss_read_offset_dst2");
    check::eq_i32(dst_o[3] as i32, 8, "ss_read_offset_dst3");
    check::eq_i32(dst_o[4] as i32, 9, "ss_read_offset_dst4");
    check::eq_i32(dst_o[5] as i32, 7, "ss_read_offset_dst5_wrapped");
    // consumed 3 + 1 bytes: pos is 1, so 2 of the 3 remain
    check::eq_i32(ro.available() as i32, 2, "ss_read_offset_available_2");

    // len==0: the while body never runs, returns 0 and consumes nothing
    let mut dst_z = [-1i8; 8];
    let mut rz = SuperStream::new(Some(vec![5, 6]));
    check::eq_i32(
        rz.read_at(&mut dst_z, 0, 0) as i32,
        0,
        "ss_read_len0_returns_0",
    );
    check::eq_i32(rz.available() as i32, 2, "ss_read_len0_available_2");
    check::eq_i32(dst_z[0] as i32, -1, "ss_read_len0_dst_untouched");

    // already exhausted: first super.read is -1, so the reset happens
    // before ANY byte of this call is copied (:22-27)
    let mut re = SuperStream::new(Some(vec![4, 5, 6]));
    re.read_byte();
    re.read_byte();
    re.read_byte();
    check::eq_i32(re.available() as i32, 0, "ss_exhausted_available_0");
    let mut dst_e = [0i8; 3];
    check::eq_i32(
        re.read_at(&mut dst_e, 0, 3) as i32,
        3,
        "ss_exhausted_read_resets_returns_3",
    );
    check::eq_i32(dst_e[0] as i32, 4, "ss_exhausted_dst0");
    check::eq_i32(dst_e[1] as i32, 5, "ss_exhausted_dst1");
    check::eq_i32(dst_e[2] as i32, 6, "ss_exhausted_dst2");

    // many wraps inside one call still fill exactly len
    let mut dst7 = [0i8; 7];
    let mut r7 = SuperStream::new(Some(vec![1, 2]));
    check::eq_i32(
        r7.read_at(&mut dst7, 0, 7) as i32,
        7,
        "ss_read_7_wraps_returns_7",
    );
    check::eq_i32(dst7[0] as i32, 1, "ss_read_7_dst0");
    check::eq_i32(dst7[1] as i32, 2, "ss_read_7_dst1");
    check::eq_i32(dst7[2] as i32, 1, "ss_read_7_dst2");
    check::eq_i32(dst7[3] as i32, 2, "ss_read_7_dst3");
    check::eq_i32(dst7[4] as i32, 1, "ss_read_7_dst4");
    check::eq_i32(dst7[5] as i32, 2, "ss_read_7_dst5");
    check::eq_i32(dst7[6] as i32, 1, "ss_read_7_dst6");
    check::eq_i32(r7.available() as i32, 1, "ss_read_7_available_1");

    // null buffer: the JDK ctor dereferences it. The port's ctor keeps the
    // Java signature's nullability (`Option`), so `None` is that NPE.
    check::throws_it(
        AssertUnwindSafe(|| {
            let _ = SuperStream::new(None);
        }),
        "NullPointerException",
        "ss_ctor_null_npe",
    );
}

/// A `CheckPoints`-visible `ContO` (`TSmall.java:463-472`): the empty `.rad`
/// stream allocates `Plane[120]` and leaves every field at 0
/// (ContO.java:207-419), so only the three fields `checkstat` reads need
/// writing.
fn car(m: &Medium, x: i32, z: i32, dist: i32) -> Conto {
    let mut c = Conto::place(&Base::default(), m, &mut Trackers::new(), 0, 0, 0, 0);
    c.x = x;
    c.z = z;
    c.dist = dist;
    c
}

/// A `CheckPoints`-visible `Madness` (`TSmall.java:474-482`): im is
/// irrelevant (checkstat reads only clear/pcleared/dest). Java also passes
/// a fresh `Record`, which the port takes per call and never reads here.
fn racer(clear: i32, pcleared: i32, dest: bool) -> Madness {
    let mut md = Madness::new(0);
    md.clear = clear;
    md.pcleared = pcleared;
    md.dest = dest;
    md
}

/// `CheckPoints.java:2-25` (fields), `:27-81` (checkstat), `:83-86` (py).
fn check_points(typ_len: i32) {
    // one Medium for every fixture: it is only the sin/cos table source
    let m = Medium::new();

    // ---- field defaults (:2-25) ----
    let d = CheckPoints::new();
    check::eq_i32(d.x.len() as i32, typ_len, "cp_x_len_140");
    check::eq_i32(d.z.len() as i32, typ_len, "cp_z_len_140");
    check::eq_i32(d.y.len() as i32, typ_len, "cp_y_len_140");
    check::eq_i32(d.typ.len() as i32, typ_len, "cp_typ_len_140");
    check::eq_i32(d.x[CP_CAP - 1], 0, "cp_x_last_0");
    check::eq_i32(d.pcs, 0, "cp_pcs_0");
    check::eq_i32(d.nsp, 0, "cp_nsp_0");
    check::eq_i32(d.n, 0, "cp_n_0");
    check::eq_i32(d.fx.len() as i32, PLAYERS as i32, "cp_fx_len_5");
    check::eq_i32(d.fz.len() as i32, PLAYERS as i32, "cp_fz_len_5");
    check::eq_i32(d.fy.len() as i32, PLAYERS as i32, "cp_fy_len_5");
    check::eq_i32(d.fx[4], 0, "cp_fx4_0");
    check::ok(!d.roted[0], "cp_roted_default_false");
    check::ok(!d.special[4], "cp_special_default_false");
    check::eq_i32(d.fn_, 0, "cp_fn_0");
    check::eq_i32(d.stage, 1, "cp_stage_1");
    check::eq_i32(d.nlaps, 0, "cp_nlaps_0");
    check::eq_str(&d.name, "hogan rewish", "cp_default_name");
    // pos is the ONE field with a non-zero initializer (:18)
    check::eq_i32(d.pos.len() as i32, PLAYERS as i32, "cp_pos_len_5");
    for i in 0..PLAYERS {
        check::eq_i32(d.pos[i], 4, &format!("cp_pos{i}_4"));
    }
    check::eq_i32(d.clear.len() as i32, PLAYERS as i32, "cp_clear_len_5");
    check::eq_i32(d.dested.len() as i32, PLAYERS as i32, "cp_dested_len_5");
    check::eq_i32(d.clear[4], 0, "cp_clear4_0");
    check::eq_i32(d.dested[4], 0, "cp_dested4_0");
    check::eq_i32(d.wasted, 0, "cp_wasted_0");
    check::ok(!d.haltall, "cp_haltall_default_false");
    check::eq_i32(d.opx.len() as i32, PLAYERS as i32, "cp_opx_len_5");
    check::eq_i32(d.opz.len() as i32, PLAYERS as i32, "cp_opz_len_5");
    check::eq_i32(d.onscreen.len() as i32, PLAYERS as i32, "cp_onscreen_len_5");
    check::eq_i32(d.opx[0], 0, "cp_opx0_0");
    check::eq_i32(d.opz[0], 0, "cp_opz0_0");
    check::eq_i32(d.onscreen[0], 0, "cp_onscreen0_0");

    // ---- py (:83-86): plain int squared distance, no abs, no clamp ----
    check::eq_i32(d.py(1, 0, 1, 0), 2, "cp_py_1_0_1_0_2");
    check::eq_i32(d.py(0, 0, 0, 0), 0, "cp_py_same_0");
    check::eq_i32(d.py(10, 20, 30, 40), 200, "cp_py_10_20_30_40_200");
    check::eq_i32(d.py(-1, 0, 0, 0), 1, "cp_py_neg1_0_1");
    // the pairs are (1,2) and (3,4), not (1,3) and (2,4)
    check::eq_i32(d.py(-3, -4, -3, -4), 2, "cp_py_neg_pairs_2");
    check::eq_i32(d.py(-3, -3, -4, -4), 0, "cp_py_same_negative_0");
    check::eq_i32(d.py(30000, 0, 0, 0), 900_000_000, "cp_py_30000_900000000");
    // 65536^2 == 4294967296 wraps to exactly 0 in int arithmetic
    check::eq_i32(d.py(65536, 0, 0, 0), 0, "cp_py_int_wraps_to_0");

    // ---- A: ranking pass, dested and pcleared/typ wrap ----
    // :31-41 copies pos/onscreen/opx/opz and derives clear (dested!=0
    // means -1, ANY non-zero value - here 2). :45-70 ranks.
    // :56-60: pcleared+1 == 4 with typ[4]==0 walks to 5 == n and wraps
    // to 0, whose typ[0]==1 stops the scan.
    let mut cp_a = CheckPoints::new();
    cp_a.n = 5;
    cp_a.typ[0] = 1;
    cp_a.typ[1] = 0;
    cp_a.typ[2] = 1;
    cp_a.typ[3] = 1;
    cp_a.typ[4] = 0;
    cp_a.dested[4] = 2;
    // clear = {2, 1, 2, 0, -1}; pcleared[0] is the only one a tie reads
    let md_a = vec![
        racer(2, 3, false),
        racer(1, 0, false),
        racer(2, 0, false),
        racer(0, 0, false),
        racer(3, 0, false),
    ];
    let co_a = vec![
        car(&m, 0, 0, 11),
        car(&m, 0, 0, 22),
        car(&m, 100, 0, 33),
        car(&m, 0, 0, 44),
        car(&m, 0, 0, 55),
    ];
    cp_a.checkstat(&md_a, &co_a);
    // (0,1) 2>1 -> pos1; (0,2) tie, dist 0 vs 1 -> pos2;
    // (0,3) 2>0 -> pos3; (0,4) 2>-1 -> pos4; (1,2) 1<2 -> pos1;
    // (1,3) 1>0 -> pos3; (1,4) 1>-1 -> pos4; (2,3) 2>0 -> pos3;
    // (2,4) 2>-1 -> pos4; (3,4) 0>-1 -> pos4
    check::eq_i32(cp_a.pos[0], 0, "cpA_pos0_0");
    check::eq_i32(cp_a.pos[1], 2, "cpA_pos1_2");
    check::eq_i32(cp_a.pos[2], 1, "cpA_pos2_1");
    check::eq_i32(cp_a.pos[3], 3, "cpA_pos3_3");
    check::eq_i32(cp_a.pos[4], 4, "cpA_pos4_4");
    check::eq_i32(cp_a.clear[0], 2, "cpA_clear0_from_madness");
    check::eq_i32(cp_a.clear[1], 1, "cpA_clear1");
    check::eq_i32(cp_a.clear[3], 0, "cpA_clear3");
    // dested[4]==2 turns the Madness clear of 3 into -1
    check::eq_i32(cp_a.clear[4], -1, "cpA_clear4_dested_minus1");
    check::eq_i32(cp_a.wasted, 0, "cpA_wasted_0");
    // the mirror fields are copied unconditionally, dested or not (:33-35)
    check::eq_i32(cp_a.onscreen[0], 11, "cpA_onscreen0_dist");
    check::eq_i32(cp_a.onscreen[4], 55, "cpA_onscreen4_dist");
    check::eq_i32(cp_a.opx[2], 100, "cpA_opx2_x");
    check::eq_i32(cp_a.opx[3], 0, "cpA_opx3_0");
    check::eq_i32(cp_a.opz[4], 0, "cpA_opz4_0");

    // ---- B: the other tie arm, the typ scan's non-wrap exit, and
    //         Java's truncating /100 on negative coordinates ----
    // typ[1]==0 -> var5 1 -> 2 (2 != n) exits on typ[2]==1 (:56-60).
    // contO[0].x == 500 -> 500/100 == 5 (py 25) against x[1]==0 (py 0);
    // contO[3].x == -150 -> -1 and contO[4].x == -50 -> 0: both /100
    // divisions truncate toward zero, giving py 1 vs 0.
    let mut cp_b = CheckPoints::new();
    cp_b.n = 5;
    cp_b.typ[0] = 1;
    cp_b.typ[1] = 0;
    cp_b.typ[2] = 1;
    cp_b.typ[3] = 1;
    cp_b.typ[4] = 1;
    let md_b = vec![
        racer(0, 0, true),
        racer(0, 0, false),
        racer(0, 0, true),
        racer(5, 0, false),
        racer(5, 0, true),
    ];
    let co_b = vec![
        car(&m, 500, 0, 1),
        car(&m, 0, 0, 2),
        car(&m, 0, 0, 3),
        car(&m, -150, 0, 4),
        car(&m, -50, 0, 5),
    ];
    cp_b.checkstat(&md_b, &co_b);
    // (0,1) tie, 25>0 -> pos0; (0,2) tie, 25>0 -> pos0;
    // (0,3)/(0,4) 0<5 -> pos0 twice; (1,2) tie, 0>0 false -> pos2;
    // (1,3)/(1,4) 0<5 -> pos1 twice; (2,3)/(2,4) 0<5 -> pos2 twice;
    // (3,4) tie, 1>0 (from -150/100==-1) -> pos3
    check::eq_i32(cp_b.pos[0], 4, "cpB_pos0_4");
    check::eq_i32(cp_b.pos[1], 2, "cpB_pos1_2");
    check::eq_i32(cp_b.pos[2], 3, "cpB_pos2_3");
    check::eq_i32(cp_b.pos[3], 1, "cpB_pos3_1");
    check::eq_i32(cp_b.pos[4], 0, "cpB_pos4_0");
    // :73-80 counts dest for players 1..4 only - index 0 is excluded
    check::eq_i32(cp_b.wasted, 2, "cpB_wasted_skips_player0");

    // ---- C: haltall (:28) skips the whole ranking block but STILL
    //         recomputes wasted (:73-80) - passing a null ContO[] proves
    //         var2 is never read ----
    let mut cp_c = CheckPoints::new();
    cp_c.haltall = true;
    for i in 0..PLAYERS {
        cp_c.pos[i] = 9;
        cp_c.onscreen[i] = 7;
        cp_c.opx[i] = 8;
        cp_c.opz[i] = 6;
        cp_c.clear[i] = 1;
    }
    cp_c.dested[0] = 3;
    let md_c = vec![
        racer(0, 0, true),
        racer(0, 0, false),
        racer(0, 0, false),
        racer(0, 0, true),
        racer(0, 0, false),
    ];
    // Java passes `null`: the port's `&[Conto]` stand-in is an empty slice —
    // any read of it would panic exactly where the Java NPE would.
    cp_c.checkstat(&md_c, &[]);
    check::eq_i32(cp_c.pos[0], 9, "cpC_pos0_untouched");
    check::eq_i32(cp_c.pos[4], 9, "cpC_pos4_untouched");
    check::eq_i32(cp_c.onscreen[0], 7, "cpC_onscreen_untouched");
    check::eq_i32(cp_c.opx[0], 8, "cpC_opx_untouched");
    check::eq_i32(cp_c.opz[0], 6, "cpC_opz_untouched");
    check::eq_i32(cp_c.clear[0], 1, "cpC_clear_untouched");
    check::eq_i32(cp_c.wasted, 1, "cpC_wasted_counts_player3_only");

    // ---- D: checkstat READS dested but never writes it ----
    // Only Madness.drive maintains dested (Madness.java:2425-2435).
    check::eq_i32(cp_a.dested[4], 2, "cpA_dested_preserved");

    // ---- E: no bounds check on the checkpoint cursor ----
    // pcleared==139 on the tied player makes var5==140 and typ[140] is
    // already out of range (:56) -> AIOOBE, before any pos write matters
    let mut cp_e = CheckPoints::new();
    cp_e.n = 5;
    cp_e.typ[0] = 1;
    let md_e = vec![
        racer(0, 139, false),
        racer(0, 0, false),
        racer(0, 0, false),
        racer(0, 0, false),
        racer(0, 0, false),
    ];
    let co_e = vec![
        car(&m, 0, 0, 0),
        car(&m, 0, 0, 0),
        car(&m, 0, 0, 0),
        car(&m, 0, 0, 0),
        car(&m, 0, 0, 0),
    ];
    check::throws_it(
        AssertUnwindSafe(|| {
            cp_e.checkstat(&md_e, &co_e);
        }),
        "index out of bounds",
        "cpE_pcleared_139_aioobe",
    );
}
