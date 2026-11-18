/**
 * Pins the five SMALL support classes: ModInstrument + ModTrackInfo (pure
 * data containers), Trackers (the 500-slot wheel tracker ring),
 * SuperStream (the wrap-forever ByteArrayInputStream that RadicalMod's
 * music streaming depends on), and CheckPoints (the race-ranking pass).
 *
 * No assets, no AWT, no audio: every fixture below is a bare array or an
 * empty-byte-ContO, the same seam TMadnessWorld already proved safe.
 *
 * Audio note: SuperStream is a sun.audio-compatible InputStream, but none
 * of its own methods touch a mixer - only RadicalMod hands it to
 * AudioPlayer.player. Everything pinned here is pure buffer math, so no
 * device I/O happens. Two behaviors are deliberately NOT pinned because
 * they cannot terminate (documented inline): read(byte[],int,int) on a
 * ZERO-length buffer with len>0 loops forever (:18-31), and checkstat's
 * typ[] scan hangs if every typ[0..n-1] is <= 0 (:56-60).
 *
 * Pin source: notes/JAVA-BEHAVIOR/B1_audio_tiny.md; every value below was
 * re-derived from the source (and cross-checked by running the original).
 */
public class TSmall {
   /** constants.toml - the 500-slot tracker ring and the 140-entry typ[]. */
   static final Toml KC = Toml.load("tests/fixtures/toml/constants.toml");
   static final int TRACKERS = KC.table("wheels").intv("trackers_slots");
   static final int CP_TYP = KC.table("madness").intv("focus_scan_len");

   static void run() {
      Check.group("Small");
      KC.table("wheels").requireKeys("trackers_slots");
      KC.table("madness").requireKeys("focus_scan_len");
      modInstrument();
      modTrackInfo();
      trackers();
      superStream();
      checkPoints();
   }

   /** ModInstrument.java:1-12 - a data container, so the implicit default
    *  ctor (the only code) leaves every field at its declared state. */
   private static void modInstrument() {
      ModInstrument mi = new ModInstrument();
      Check.eq(mi.name, null, "mi_name_null");
      Check.eq(mi.samples, null, "mi_samples_null");
      Check.eq(mi.sample_length, 0, "mi_sample_length_0");
      Check.eq(mi.finetune_rate, 0, "mi_finetune_rate_0");
      Check.eq(mi.period_low_limit, 0, "mi_period_low_limit_0");
      Check.eq(mi.period_high_limit, 0, "mi_period_high_limit_0");
      Check.eq(mi.finetune_value, 0, "mi_finetune_value_0");
      Check.eq(mi.volume, 0, "mi_volume_0");
      Check.eq(mi.repeat_point, 0, "mi_repeat_point_0");
      Check.eq(mi.repeat_length, 0, "mi_repeat_length_0");
      // 10 fields total (ModInstrument.java:2-11); B1_audio_tiny.md line 2
      // said "11 fields" - corrected to 10 from the source
      Check.eq(declaredFieldCount(ModInstrument.class), 10, "mi_field_count_10");

      // the container is writable and per-instance, not shared
      mi.sample_length = 1234;
      mi.volume = 64;
      ModInstrument other = new ModInstrument();
      Check.eq(mi.sample_length, 1234, "mi_write_sample_length");
      Check.eq(other.sample_length, 0, "mi_second_instance_independent");
      Check.eq(other.volume, 0, "mi_second_instance_volume_0");
   }

   /** ModTrackInfo.java:1-38 - the ctor (:35-37) only allocates arp[3];
    *  every other field keeps its initializer, oldperiod starting at 1. */
   private static void modTrackInfo() {
      ModTrackInfo ti = new ModTrackInfo();
      Check.eq(ti.samples, null, "ti_samples_null");
      Check.eq(ti.position, 0, "ti_position_0");
      Check.eq(ti.length, 0, "ti_length_0");
      Check.eq(ti.repeat, 0, "ti_repeat_0");
      Check.eq(ti.replen, 0, "ti_replen_0");
      Check.eq(ti.volume, 0, "ti_volume_0");
      Check.eq(ti.error, 0, "ti_error_0");
      Check.eq(ti.pitch, 0, "ti_pitch_0");
      Check.eq(ti.old_position, 0, "ti_old_position_0");
      Check.eq(ti.start_period, 0, "ti_start_period_0");
      Check.eq(ti.period, 0, "ti_period_0");
      Check.eq(ti.step, 0, "ti_step_0");
      Check.eq(ti.effect, 0, "ti_effect_0");
      Check.eq(ti.portto, 0, "ti_portto_0");
      Check.eq(ti.vibpos, 0, "ti_vibpos_0");
      Check.eq(ti.trempos, 0, "ti_trempos_0");
      Check.eq(ti.oldsampofs, 0, "ti_oldsampofs_0");
      // the ctor's ONLY write (:36)
      Check.eq(ti.arp.length, 3, "ti_arp_length_3");
      Check.eq(ti.arp[0], 0, "ti_arp0_0");
      Check.eq(ti.arp[1], 0, "ti_arp1_0");
      Check.eq(ti.arp[2], 0, "ti_arp2_0");
      Check.eq(ti.arpindex, 0, "ti_arpindex_0");
      Check.eq(ti.oldperiod, 1, "ti_oldperiod_1");
      Check.eq(ti.vol_slide, 0, "ti_vol_slide_0");
      Check.eq(ti.port_inc, 0, "ti_port_inc_0");
      Check.eq(ti.port_up, 0, "ti_port_up_0");
      Check.eq(ti.port_down, 0, "ti_port_down_0");
      Check.eq(ti.vib_rate, 0, "ti_vib_rate_0");
      Check.eq(ti.vib_depth, 0, "ti_vib_depth_0");
      Check.eq(ti.trem_rate, 0, "ti_trem_rate_0");
      Check.eq(ti.trem_depth, 0, "ti_trem_depth_0");
      Check.eq(ti.retrig, 0, "ti_retrig_0");
      Check.eq(ti.finetune_rate, 0, "ti_finetune_rate_0");
      Check.eq(ti.period_low_limit, 0, "ti_period_low_limit_0");
      Check.eq(ti.period_high_limit, 0, "ti_period_high_limit_0");
      Check.eq(declaredFieldCount(ModTrackInfo.class), 32, "ti_field_count_32");

      // arp is allocated fresh per instance - a write must not leak
      ti.arp[1] = 7;
      ModTrackInfo other = new ModTrackInfo();
      Check.eq(ti.arp[1], 7, "ti_arp_write_visible");
      Check.eq(other.arp[1], 0, "ti_arp_per_instance");
      Check.ok(ti.arp != other.arp, "ti_arp_distinct_array");
   }

   /** Trackers.java:1-14 - no explicit ctor and no methods at all; the
    *  implicit ctor allocates the eleven 500-slot arrays (c is
    *  int[500][3]) and nt starts at 0. */
   private static void trackers() {
      Trackers t = new Trackers();
      Check.eq(t.x.length, TRACKERS, "tr_x_len_500");
      Check.eq(t.y.length, TRACKERS, "tr_y_len_500");
      Check.eq(t.z.length, TRACKERS, "tr_z_len_500");
      Check.eq(t.xy.length, TRACKERS, "tr_xy_len_500");
      Check.eq(t.zy.length, TRACKERS, "tr_zy_len_500");
      Check.eq(t.skd.length, TRACKERS, "tr_skd_len_500");
      Check.eq(t.dam.length, TRACKERS, "tr_dam_len_500");
      Check.eq(t.c.length, TRACKERS, "tr_c_len_500");
      Check.eq(t.radx.length, TRACKERS, "tr_radx_len_500");
      Check.eq(t.radz.length, TRACKERS, "tr_radz_len_500");
      Check.eq(t.rady.length, TRACKERS, "tr_rady_len_500");
      Check.eq(t.nt, 0, "tr_nt_0");
      // every row of the collision table is a fresh 3-slot array
      Check.eq(t.c[0].length, 3, "tr_c_row0_len_3");
      Check.eq(t.c[TRACKERS - 1].length, 3, "tr_c_row499_len_3");
      Check.eq(t.c[0][2], 0, "tr_c_row0_slot2_0");
      Check.eq(t.x[TRACKERS - 1], 0, "tr_x_last_0");
      Check.ok(t.c[0] != t.c[1], "tr_c_rows_distinct");
      // no methods are declared on Trackers; it is pure storage
      Check.eq(declaredMethodCount(Trackers.class), 0, "tr_no_declared_methods");
      // the slot writes drive reads at Madness.java:1336/1342/1353
      t.nt = 1;
      t.skd[0] = 3;
      t.dam[0] = 12;
      t.c[0][1] = 9;
      Check.eq(t.nt, 1, "tr_nt_write");
      Check.eq(t.skd[0], 3, "tr_skd_write");
      Check.eq(t.dam[0], 12, "tr_dam_write");
      Check.eq(t.c[0][1], 9, "tr_c_write");
      Trackers other = new Trackers();
      Check.eq(other.nt, 0, "tr_second_instance_nt_0");
      Check.eq(other.skd[0], 0, "tr_second_instance_skd_0");
   }

   /** SuperStream.java:4-31 - ByteArrayInputStream that WRAPS at EOF
    *  instead of reporting it: read() re-reads buf[0] forever (:8-16)
    *  and read(b,off,len) never returns early for len>0, resetting and
    *  continuing until exactly len bytes were copied (:18-31).
    *  (Empty-buffer read(b,off,len) with len>0 never terminates - the
    *  loop has no progress path once super.read keeps returning -1 - so
    *  it is unpinnable without a thread; see the class comment.) */
   private static void superStream() {
      // :4-6 delegates to ByteArrayInputStream(byte[]), which keeps the
      // caller's array by reference (no defensive copy)
      byte[] shared = new byte[]{1, 2, 3};
      SuperStream ss = new SuperStream(shared);
      Check.eq(ss.available(), 3, "ss_ctor_available_3");
      shared[0] = 9;
      Check.eq(ss.read(), 9, "ss_ctor_no_copy_sees_mutation");
      Check.eq(ss.available(), 2, "ss_read_advances_available_2");

      // read(): bytes in order, then the EOF re-read wraps to buf[0]
      SuperStream w = new SuperStream(new byte[]{10, 20, 30});
      Check.eq(w.read(), 10, "ss_read_10");
      Check.eq(w.read(), 20, "ss_read_20");
      Check.eq(w.available(), 1, "ss_available_1");
      Check.eq(w.read(), 30, "ss_read_30");
      Check.eq(w.available(), 0, "ss_available_0_at_eof");
      // :9-13 - super.read()==-1 -> reset() -> super.read() == buf[0]
      Check.eq(w.read(), 10, "ss_read_eof_wraps_to_first");
      Check.eq(w.read(), 20, "ss_read_after_wrap_20");
      Check.eq(w.available(), 1, "ss_available_1_after_wrap");

      // zero-length buffer: the wrap attempt also fails, so -1 forever
      SuperStream empty = new SuperStream(new byte[0]);
      Check.eq(empty.available(), 0, "ss_empty_available_0");
      Check.eq(empty.read(), -1, "ss_empty_read_minus1");
      Check.eq(empty.read(), -1, "ss_empty_read_minus1_again");

      // one-byte buffer: the same byte forever
      SuperStream one = new SuperStream(new byte[]{42});
      Check.eq(one.read(), 42, "ss_single_read_42");
      Check.eq(one.available(), 0, "ss_single_available_0");
      Check.eq(one.read(), 42, "ss_single_wraps");
      Check.eq(one.read(), 42, "ss_single_wraps_again");

      // read(b,off,len) exact fit: one super.read, no reset, returns len
      byte[] dst = new byte[]{-1, -1, -1, -1, -1, -1, -1, -1};
      SuperStream r2 = new SuperStream(new byte[]{1, 2});
      Check.eq(r2.read(dst, 0, 2), 2, "ss_read_exact_returns_2");
      Check.eq(dst[0], 1, "ss_read_exact_dst0");
      Check.eq(dst[1], 2, "ss_read_exact_dst1");
      Check.eq(dst[2], -1, "ss_read_exact_tail_untouched");
      Check.eq(r2.available(), 0, "ss_read_exact_available_0");

      // len > buffer: copies the tail, resets, copies the head (:25-27)
      byte[] dst5 = new byte[5];
      SuperStream r5 = new SuperStream(new byte[]{1, 2, 3});
      Check.eq(r5.read(dst5, 0, 5), 5, "ss_read_overlong_returns_5");
      Check.eq(dst5[0], 1, "ss_read_overlong_dst0");
      Check.eq(dst5[1], 2, "ss_read_overlong_dst1");
      Check.eq(dst5[2], 3, "ss_read_overlong_dst2");
      Check.eq(dst5[3], 1, "ss_read_overlong_dst3_wrapped");
      Check.eq(dst5[4], 2, "ss_read_overlong_dst4_wrapped");
      Check.eq(r5.available(), 1, "ss_read_overlong_available_1");

      // the offset variant must write only from off and respect the wrap
      byte[] dstO = new byte[]{-1, -1, -1, -1, -1, -1};
      SuperStream ro = new SuperStream(new byte[]{7, 8, 9});
      Check.eq(ro.read(dstO, 2, 4), 4, "ss_read_offset_returns_4");
      Check.eq(dstO[0], -1, "ss_read_offset_before0_untouched");
      Check.eq(dstO[1], -1, "ss_read_offset_before1_untouched");
      Check.eq(dstO[2], 7, "ss_read_offset_dst2");
      Check.eq(dstO[3], 8, "ss_read_offset_dst3");
      Check.eq(dstO[4], 9, "ss_read_offset_dst4");
      Check.eq(dstO[5], 7, "ss_read_offset_dst5_wrapped");
      // consumed 3 + 1 bytes: pos is 1, so 2 of the 3 remain
      Check.eq(ro.available(), 2, "ss_read_offset_available_2");

      // len==0: the while body never runs, returns 0 and consumes nothing
      byte[] dstZ = new byte[]{-1, -1, -1, -1, -1, -1, -1, -1};
      SuperStream rz = new SuperStream(new byte[]{5, 6});
      Check.eq(rz.read(dstZ, 0, 0), 0, "ss_read_len0_returns_0");
      Check.eq(rz.available(), 2, "ss_read_len0_available_2");
      Check.eq(dstZ[0], -1, "ss_read_len0_dst_untouched");

      // already exhausted: first super.read is -1, so the reset happens
      // before ANY byte of this call is copied (:22-27)
      SuperStream re = new SuperStream(new byte[]{4, 5, 6});
      re.read();
      re.read();
      re.read();
      Check.eq(re.available(), 0, "ss_exhausted_available_0");
      byte[] dstE = new byte[3];
      Check.eq(re.read(dstE, 0, 3), 3, "ss_exhausted_read_resets_returns_3");
      Check.eq(dstE[0], 4, "ss_exhausted_dst0");
      Check.eq(dstE[1], 5, "ss_exhausted_dst1");
      Check.eq(dstE[2], 6, "ss_exhausted_dst2");

      // many wraps inside one call still fill exactly len
      byte[] dst7 = new byte[7];
      SuperStream r7 = new SuperStream(new byte[]{1, 2});
      Check.eq(r7.read(dst7, 0, 7), 7, "ss_read_7_wraps_returns_7");
      Check.eq(dst7[0], 1, "ss_read_7_dst0");
      Check.eq(dst7[1], 2, "ss_read_7_dst1");
      Check.eq(dst7[2], 1, "ss_read_7_dst2");
      Check.eq(dst7[3], 2, "ss_read_7_dst3");
      Check.eq(dst7[4], 1, "ss_read_7_dst4");
      Check.eq(dst7[5], 2, "ss_read_7_dst5");
      Check.eq(dst7[6], 1, "ss_read_7_dst6");
      Check.eq(r7.available(), 1, "ss_read_7_available_1");

      // null buffer: ByteArrayInputStream's own ctor dereferences it
      Check.throwsIt(new Check.Task() {
         public void run() {
            new SuperStream(null);
         }
      }, NullPointerException.class, "ss_ctor_null_npe");
   }

   /** CheckPoints.java:2-25 (fields), :27-81 (checkstat), :83-86 (py). */
   private static void checkPoints() {
      // one Medium for every fixture: it is only the sin/cos table source
      Medium m = new Medium();

      // ---- field defaults (:2-25) ----
      CheckPoints d = new CheckPoints();
      Check.eq(d.x.length, CP_TYP, "cp_x_len_140");
      Check.eq(d.z.length, CP_TYP, "cp_z_len_140");
      Check.eq(d.y.length, CP_TYP, "cp_y_len_140");
      Check.eq(d.typ.length, CP_TYP, "cp_typ_len_140");
      Check.eq(d.x[139], 0, "cp_x_last_0");
      Check.eq(d.pcs, 0, "cp_pcs_0");
      Check.eq(d.nsp, 0, "cp_nsp_0");
      Check.eq(d.n, 0, "cp_n_0");
      Check.eq(d.fx.length, 5, "cp_fx_len_5");
      Check.eq(d.fz.length, 5, "cp_fz_len_5");
      Check.eq(d.fy.length, 5, "cp_fy_len_5");
      Check.eq(d.fx[4], 0, "cp_fx4_0");
      Check.ok(!d.roted[0], "cp_roted_default_false");
      Check.ok(!d.special[4], "cp_special_default_false");
      Check.eq(d.fn, 0, "cp_fn_0");
      Check.eq(d.stage, 1, "cp_stage_1");
      Check.eq(d.nlaps, 0, "cp_nlaps_0");
      Check.eq(d.name, "hogan rewish", "cp_default_name");
      // pos is the ONE field with a non-zero initializer (:18)
      Check.eq(d.pos.length, 5, "cp_pos_len_5");
      for (int i = 0; i < 5; i++) {
         Check.eq(d.pos[i], 4, "cp_pos" + i + "_4");
      }
      Check.eq(d.clear.length, 5, "cp_clear_len_5");
      Check.eq(d.dested.length, 5, "cp_dested_len_5");
      Check.eq(d.clear[4], 0, "cp_clear4_0");
      Check.eq(d.dested[4], 0, "cp_dested4_0");
      Check.eq(d.wasted, 0, "cp_wasted_0");
      Check.ok(!d.haltall, "cp_haltall_default_false");
      Check.eq(d.opx.length, 5, "cp_opx_len_5");
      Check.eq(d.opz.length, 5, "cp_opz_len_5");
      Check.eq(d.onscreen.length, 5, "cp_onscreen_len_5");
      Check.eq(d.opx[0], 0, "cp_opx0_0");
      Check.eq(d.opz[0], 0, "cp_opz0_0");
      Check.eq(d.onscreen[0], 0, "cp_onscreen0_0");

      // ---- py (:83-86): plain int squared distance, no abs, no clamp ----
      Check.eq(d.py(1, 0, 1, 0), 2, "cp_py_1_0_1_0_2");
      Check.eq(d.py(0, 0, 0, 0), 0, "cp_py_same_0");
      Check.eq(d.py(10, 20, 30, 40), 200, "cp_py_10_20_30_40_200");
      Check.eq(d.py(-1, 0, 0, 0), 1, "cp_py_neg1_0_1");
      // the pairs are (1,2) and (3,4), not (1,3) and (2,4)
      Check.eq(d.py(-3, -4, -3, -4), 2, "cp_py_neg_pairs_2");
      Check.eq(d.py(-3, -3, -4, -4), 0, "cp_py_same_negative_0");
      Check.eq(d.py(30000, 0, 0, 0), 900000000, "cp_py_30000_900000000");
      // 65536^2 == 4294967296 wraps to exactly 0 in int arithmetic
      Check.eq(d.py(65536, 0, 0, 0), 0, "cp_py_int_wraps_to_0");

      // ---- A: ranking pass, dested and pcleared/typ wrap ----
      // :31-41 copies pos/onscreen/opx/opz and derives clear (dested!=0
      // means -1, ANY non-zero value - here 2). :45-70 ranks.
      // :56-60: pcleared+1 == 4 with typ[4]==0 walks to 5 == n and wraps
      // to 0, whose typ[0]==1 stops the scan.
      CheckPoints cpA = new CheckPoints();
      cpA.n = 5;
      cpA.typ[0] = 1;
      cpA.typ[1] = 0;
      cpA.typ[2] = 1;
      cpA.typ[3] = 1;
      cpA.typ[4] = 0;
      cpA.dested[4] = 2;
      // clear = {2, 1, 2, 0, -1}; pcleared[0] is the only one a tie reads
      Madness[] mdA = new Madness[]{
            racer(m, 2, 3, false), racer(m, 1, 0, false), racer(m, 2, 0, false), racer(m, 0, 0, false), racer(m, 3, 0, false)};
      ContO[] coA = new ContO[]{car(m, 0, 0, 11), car(m, 0, 0, 22), car(m, 100, 0, 33), car(m, 0, 0, 44), car(m, 0, 0, 55)};
      cpA.checkstat(mdA, coA);
      // (0,1) 2>1 -> pos1; (0,2) tie, dist 0 vs 1 -> pos2;
      // (0,3) 2>0 -> pos3; (0,4) 2>-1 -> pos4; (1,2) 1<2 -> pos1;
      // (1,3) 1>0 -> pos3; (1,4) 1>-1 -> pos4; (2,3) 2>0 -> pos3;
      // (2,4) 2>-1 -> pos4; (3,4) 0>-1 -> pos4
      Check.eq(cpA.pos[0], 0, "cpA_pos0_0");
      Check.eq(cpA.pos[1], 2, "cpA_pos1_2");
      Check.eq(cpA.pos[2], 1, "cpA_pos2_1");
      Check.eq(cpA.pos[3], 3, "cpA_pos3_3");
      Check.eq(cpA.pos[4], 4, "cpA_pos4_4");
      Check.eq(cpA.clear[0], 2, "cpA_clear0_from_madness");
      Check.eq(cpA.clear[1], 1, "cpA_clear1");
      Check.eq(cpA.clear[3], 0, "cpA_clear3");
      // dested[4]==2 turns the Madness clear of 3 into -1
      Check.eq(cpA.clear[4], -1, "cpA_clear4_dested_minus1");
      Check.eq(cpA.wasted, 0, "cpA_wasted_0");
      // the mirror fields are copied unconditionally, dested or not (:33-35)
      Check.eq(cpA.onscreen[0], 11, "cpA_onscreen0_dist");
      Check.eq(cpA.onscreen[4], 55, "cpA_onscreen4_dist");
      Check.eq(cpA.opx[2], 100, "cpA_opx2_x");
      Check.eq(cpA.opx[3], 0, "cpA_opx3_0");
      Check.eq(cpA.opz[4], 0, "cpA_opz4_0");

      // ---- B: the other tie arm, the typ scan's non-wrap exit, and
      //         Java's truncating /100 on negative coordinates ----
      // typ[1]==0 -> var5 1 -> 2 (2 != n) exits on typ[2]==1 (:56-60).
      // contO[0].x == 500 -> 500/100 == 5 (py 25) against x[1]==0 (py 0);
      // contO[3].x == -150 -> -1 and contO[4].x == -50 -> 0: both /100
      // divisions truncate toward zero, giving py 1 vs 0.
      CheckPoints cpB = new CheckPoints();
      cpB.n = 5;
      cpB.typ[0] = 1;
      cpB.typ[1] = 0;
      cpB.typ[2] = 1;
      cpB.typ[3] = 1;
      cpB.typ[4] = 1;
      Madness[] mdB = new Madness[]{
            racer(m, 0, 0, true), racer(m, 0, 0, false), racer(m, 0, 0, true), racer(m, 5, 0, false), racer(m, 5, 0, true)};
      ContO[] coB = new ContO[]{car(m, 500, 0, 1), car(m, 0, 0, 2), car(m, 0, 0, 3), car(m, -150, 0, 4), car(m, -50, 0, 5)};
      cpB.checkstat(mdB, coB);
      // (0,1) tie, 25>0 -> pos0; (0,2) tie, 25>0 -> pos0;
      // (0,3)/(0,4) 0<5 -> pos0 twice; (1,2) tie, 0>0 false -> pos2;
      // (1,3)/(1,4) 0<5 -> pos1 twice; (2,3)/(2,4) 0<5 -> pos2 twice;
      // (3,4) tie, 1>0 (from -150/100==-1) -> pos3
      Check.eq(cpB.pos[0], 4, "cpB_pos0_4");
      Check.eq(cpB.pos[1], 2, "cpB_pos1_2");
      Check.eq(cpB.pos[2], 3, "cpB_pos2_3");
      Check.eq(cpB.pos[3], 1, "cpB_pos3_1");
      Check.eq(cpB.pos[4], 0, "cpB_pos4_0");
      // :73-80 counts dest for players 1..4 only - index 0 is excluded
      Check.eq(cpB.wasted, 2, "cpB_wasted_skips_player0");

      // ---- C: haltall (:28) skips the whole ranking block but STILL
      //         recomputes wasted (:73-80) - passing a null ContO[] proves
      //         var2 is never read ----
      CheckPoints cpC = new CheckPoints();
      cpC.haltall = true;
      for (int i = 0; i < 5; i++) {
         cpC.pos[i] = 9;
         cpC.onscreen[i] = 7;
         cpC.opx[i] = 8;
         cpC.opz[i] = 6;
         cpC.clear[i] = 1;
      }
      cpC.dested[0] = 3;
      Madness[] mdC = new Madness[]{
            racer(m, 0, 0, true), racer(m, 0, 0, false), racer(m, 0, 0, false), racer(m, 0, 0, true), racer(m, 0, 0, false)};
      cpC.checkstat(mdC, null);
      Check.eq(cpC.pos[0], 9, "cpC_pos0_untouched");
      Check.eq(cpC.pos[4], 9, "cpC_pos4_untouched");
      Check.eq(cpC.onscreen[0], 7, "cpC_onscreen_untouched");
      Check.eq(cpC.opx[0], 8, "cpC_opx_untouched");
      Check.eq(cpC.opz[0], 6, "cpC_opz_untouched");
      Check.eq(cpC.clear[0], 1, "cpC_clear_untouched");
      Check.eq(cpC.wasted, 1, "cpC_wasted_counts_player3_only");

      // ---- D: checkstat READS dested but never writes it ----
      // Only Madness.drive maintains dested (Madness.java:2425-2435).
      Check.eq(cpA.dested[4], 2, "cpA_dested_preserved");

      // ---- E: no bounds check on the checkpoint cursor ----
      // pcleared==139 on the tied player makes var5==140 and typ[140] is
      // already out of range (:56) -> AIOOBE, before any pos write matters
      final CheckPoints cpE = new CheckPoints();
      cpE.n = 5;
      cpE.typ[0] = 1;
      final Madness[] mdE = new Madness[]{
            racer(m, 0, 139, false), racer(m, 0, 0, false), racer(m, 0, 0, false), racer(m, 0, 0, false), racer(m, 0, 0, false)};
      final ContO[] coE = new ContO[]{car(m, 0, 0, 0), car(m, 0, 0, 0), car(m, 0, 0, 0), car(m, 0, 0, 0), car(m, 0, 0, 0)};
      Check.throwsIt(new Check.Task() {
         public void run() {
            cpE.checkstat(mdE, coE);
         }
      }, ArrayIndexOutOfBoundsException.class, "cpE_pcleared_139_aioobe");
   }

   /** Counts the source-declared fields only: the JaCoCo agent adds a
    *  synthetic $jacocoData field (and $jacocoInit method) at load time,
    *  so an unfiltered count differs between the plain and --coverage
    *  runs of the very same suite. */
   private static int declaredFieldCount(Class<?> c) {
      int n = 0;
      for (java.lang.reflect.Field f : c.getDeclaredFields()) {
         if (!f.isSynthetic()) {
            ++n;
         }
      }
      return n;
   }

   /** Source-declared methods only, for the same reason. */
   private static int declaredMethodCount(Class<?> c) {
      int n = 0;
      for (java.lang.reflect.Method mt : c.getDeclaredMethods()) {
         if (!mt.isSynthetic()) {
            ++n;
         }
      }
      return n;
   }

   /** A CheckPoints-visible ContO: the empty .rad stream allocates
    *  Plane[120] and leaves every field at 0 (ContO.java:207-419), so only
    *  the three fields checkstat reads (:33-35, :62-63) need writing. */
   private static ContO car(Medium m, int x, int z, int dist) {
      ContO c = new ContO(new byte[0], m, new Trackers());
      c.x = x;
      c.z = z;
      c.dist = dist;
      return c;
   }

   /** A CheckPoints-visible Madness: im is irrelevant (checkstat reads
    *  only clear/pcleared/dest, :37, :54, :77). */
   private static Madness racer(Medium m, int clear, int pcleared, boolean dest) {
      Madness md = new Madness(m, new Record(m), null, 0);
      md.clear = clear;
      md.pcleared = pcleared;
      md.dest = dest;
      return md;
   }
}
