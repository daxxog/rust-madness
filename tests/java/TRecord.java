/**
 * Pins Record - the game's ghost/replay recorder - so the Rust mirror can
 * replay a recording 1:1. Everything is driven through the empty-byte ContO
 * seam: `new ContO(new byte[0], m, new Trackers())` runs the line-based .rad
 * parser (ContO.java:207-419) over an empty stream, allocating Plane[120]
 * (:210) and leaving npl==0 plus every placement field at its initializer
 * (ContO.java:10-56). The tests then write the handful of package-private
 * fields each path reads, exactly as TMadnessWorld/TSmall do.
 *
 * Record touches NO xtGraphics and NO Graphics: its only collaborators are
 * Medium (sin/cos tables + random()), ContO, Plane and java.awt.Color (the
 * reg* damage tint, which is pure HSB math - see regTint()). The
 * Madness argument of play/playh/regX/chipX is read for clrad[cn], cn,
 * flipy[cn], msquash[cn] and im only (Record.java:124,195-198,383,410,753,
 * 780), so `new Madness(m, r, null, 1)` is safe with a null xt.
 *
 * Determinism: Medium.random() (Medium.java:49-89) is the ONLY entropy
 * Record draws - regy :125,:193; regx :781; regz :411; chipx :754;
 * chipz :384. Its real output is a function of Math.random(), and TestRng
 * still scopes a seed around every random-consuming group (install/
 * uninstall in try/finally), but the pinned literals are produced by
 * TenthsMedium: a Medium whose random() replays the measured seed-101
 * tenths vector. Reason (probed on the gate's Oracle 8u40): TestRng's
 * reflection write to Math's `static final` holder stops being observed
 * once HotSpot compiles java.lang.Math.random() and constant-folds that
 * field, so late in this long suite a fresh install() keeps serving the
 * PREVIOUS generator's stream. A seeded vector would therefore silently
 * depend on JIT timing; TenthsMedium keeps the literals exactly the seeded
 * ones while removing that dependency. It also pins HOW MANY m.random()
 * draws each call makes (fixed by the plane/point loops, independent of the
 * values) and keeps the RNG out of the assertion path.
 *
 * Plane fixtures: nocol planes (oc channels all equal) make the reg* HSB/
 * Color tint branch unreachable - see planed(); the tint itself is pinned
 * separately by regTint() with oc={200,100,50}.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B4_record.md; every literal below was
 * re-derived from reference/vineflower/Record.java and probe-measured on the
 * gate's Oracle 8u40 (scratch harness under target/java-tests-probe).
 */
public class TRecord {
   /** constants.toml [record] - the recorder window and ring shape. */
   static final Toml KR = Toml.load("tests/fixtures/toml/constants.toml").table("record");
   static final int FRAMES = KR.intv("frames");
   static final int RING = KR.intv("ring_slots");
   static final int RING_LAST = KR.intv("ring_last");
   static final int CNTF = KR.intv("cntf");

   static void run() {
      Check.group("Record");
      KR.requireKeys("cntf", "ring_slots", "ring_last", "frames", "hist_slot");
      // shape pin: the replay period is part of the pinned behavior
      Check.eq(TenthsMedium.VEC.length, 12, "tenths_vector_len_12");
      ctorDefaults();
      ringQuirk();
      ringWraps();
      recFrames();
      recHistory();
      recSpark();
      recDest230();
      playFrames();
      playGates();
      playParts();
      playhFrames();
      playhLastfr();
      regxGroup();
      regyGroup();
      regzGroup();
      chipGroup();
      regTint();
      cotchinowGroup();
      resetGroup();
      pyGroup();
   }

   /** Record.java:4-76 declares the state; the ctor (:281-285) only stores
    *  m, zeroes caught and calls cotchinow(0) - and cotchinow's whole body is
    *  gated on caught>=300 (:653), so on a fresh instance it does nothing. */
   private static void ctorDefaults() {
      Medium m = new Medium();
      Record r = new Record(m);
      Check.eq(r.m, m, "ctor_m_identity");
      Check.eq(r.caught, 0, "ctor_caught_0");
      Check.ok(!r.hcaught, "ctor_hcaught_false");
      Check.ok(r.prepit, "ctor_prepit_true");
      Check.eq(r.cntf, CNTF, "ctor_cntf_50");
      Check.eq(r.lastfr, 0, "ctor_lastfr_0");
      Check.eq(r.wasted, 0, "ctor_wasted_0");
      Check.eq(r.whenwasted, 0, "ctor_whenwasted_0");
      Check.eq(r.powered, 0, "ctor_powered_0");
      // :284 cotchinow(0) with caught==0: no starcar, no h transfer
      Check.eq(r.starcar[0], null, "ctor_cotchinow_no_starcar");
      for (int i = 0; i < 5; i++) {
         Check.eq(r.hfix[i], -1, "ctor_hfix" + i + "_minus1");
         Check.eq(r.hdest[i], -1, "ctor_hdest" + i + "_minus1");
      }

      // declared shapes (:8-75)
      Check.eq(r.ocar.length, 5, "shape_ocar_5");
      Check.eq(r.car.length, 6, "shape_car_6");
      Check.eq(r.car[5].length, 5, "shape_car_row_5");
      Check.eq(r.squash.length, 6, "shape_squash_6");
      Check.eq(r.squash[5].length, 5, "shape_squash_row_5");
      Check.eq(r.fix.length, 5, "shape_fix_5");
      Check.eq(r.dest.length, 5, "shape_dest_5");
      Check.eq(r.cntdest.length, 5, "shape_cntdest_5");
      Check.eq(r.starcar.length, 5, "shape_starcar_5");
      Check.eq(r.hsquash.length, 5, "shape_hsquash_5");
      Check.eq(r.x.length, FRAMES, "shape_x_300");
      Check.eq(r.x[FRAMES - 1].length, 5, "shape_x_row_5");
      Check.eq(r.y.length, FRAMES, "shape_y_300");
      Check.eq(r.hx.length, FRAMES, "shape_hx_300");
      Check.eq(r.ns.length, 5, "shape_ns_5x4");
      Check.eq(r.ns[4].length, 4, "shape_ns_part_4");
      Check.eq(r.sspark.length, 5, "shape_sspark_5");
      Check.eq(r.sspark[4].length, 4, "shape_sspark_part_4");
      Check.eq(r.sspark[4][3].length, 30, "shape_sspark_slot_30");
      Check.eq(r.smag.length, 5, "shape_smag_5x4x30");
      Check.eq(r.fulls[4][3].length, 30, "shape_fulls_slot_30");
      Check.eq(r.nry.length, 5, "shape_nry_5x4");
      Check.eq(r.nry[4].length, 4, "shape_nry_part_4");
      Check.eq(r.rx.length, 5, "shape_rx_5");
      Check.eq(r.rx[4].length, 4, "shape_rx_part_4");
      Check.eq(r.rx[4][3].length, RING, "shape_rx_slot_7");
      Check.eq(r.magy.length, 5, "shape_magy_5x4x7");
      Check.eq(r.mtouch.length, 5, "shape_mtouch_5");
      Check.eq(r.mtouch[4].length, RING, "shape_mtouch_slot_7");
      Check.eq(r.hmtouch.length, 5, "shape_hmtouch_5x7");
      Check.eq(r.hry[4][3].length, RING, "shape_hry_slot_7");

      // zero / false defaults at both ends of every array family
      Check.eq(r.x[0][0], 0, "zero_x_first");
      Check.eq(r.x[FRAMES - 1][4], 0, "zero_x_last");
      Check.eq(r.y[FRAMES - 1][4], 0, "zero_y_last");
      Check.eq(r.z[FRAMES - 1][4], 0, "zero_z_last");
      Check.eq(r.xy[FRAMES - 1][4], 0, "zero_xy_last");
      Check.eq(r.zy[FRAMES - 1][4], 0, "zero_zy_last");
      Check.eq(r.xz[FRAMES - 1][4], 0, "zero_xz_last");
      Check.eq(r.wxz[FRAMES - 1][4], 0, "zero_wxz_last");
      Check.eq(r.wzy[FRAMES - 1][4], 0, "zero_wzy_last");
      Check.eq(r.sspark[4][3][29], 0, "zero_sspark_last");
      Check.eq(r.fulls[4][3][29], Boolean.FALSE, "zero_fulls_last");
      Check.eq(r.smag[4][3][29], 0.0F, "zero_smag_last");
      Check.eq(r.rx[4][3][6], 0, "zero_rx_last");
      Check.eq(r.magy[4][3][6], 0, "zero_magy_last");
      Check.ok(!r.mtouch[4][6], "zero_mtouch_last");
      Check.eq(r.nrx[4][3], 0, "zero_nrx");
      Check.eq(r.nry[4][3], 0, "zero_nry");
      Check.eq(r.nrz[4][3], 0, "zero_nrz");
      Check.eq(r.hx[FRAMES - 1][4], 0, "zero_hx_last");
      Check.eq(r.hsspark[4][3][29], 0, "zero_hsspark_last");
      Check.eq(r.hry[4][3][6], 0, "zero_hry_last");
      Check.eq(r.hnry[4][3], 0, "zero_hnry");
      Check.ok(!r.hmtouch[4][6], "zero_hmtouch_last");
      for (int i = 0; i < 5; i++) {
         Check.eq(r.fix[i], 0, "zero_fix" + i);
         Check.eq(r.dest[i], 0, "zero_dest" + i);
         Check.eq(r.cntdest[i], 0, "zero_cntdest" + i);
      }
   }

   /** THE TRANSCRIPTION-CRITICAL QUIRK (:633-650, :841-848): recx and recz
    *  both WRITE at nry[car][part] while incrementing their OWN nrx/nrz;
    *  recy writes at (and increments) nry. So nrx/nry/nrz drift apart and
    *  only the FIRST write of a ring lands in slot 0 - recx/recz follow
    *  recy around the ring instead of walking it themselves. */
   private static void ringQuirk() {
      Medium m = new Medium();
      Record q = new Record(m);

      // a recy first pushes nry to 1
      q.recy(1, 50.0F, true, 2);
      Check.eq(q.nry[2][1], 1, "ring_recy_nry_1");
      Check.eq(q.ry[2][1][0], 300, "ring_recy_ry_slot0_300");
      Check.eq(q.magy[2][1][0], 50, "ring_recy_magy_slot0_50");
      // :645 mtouch is [car][slot] - NOT [part][...]
      Check.ok(q.mtouch[2][0], "ring_recy_mtouch_car_slot0");

      // recx writes at nry==1 (not nrx==0) and only bumps nrx
      q.recx(1, 123.7F, 2);
      Check.eq(q.nrx[2][1], 1, "ring_recx_nrx_1");
      Check.eq(q.nry[2][1], 1, "ring_recx_nry_untouched_1");
      Check.eq(q.rx[2][1][0], 0, "ring_recx_slot0_untouched");
      Check.eq(q.rx[2][1][1], 300, "ring_recx_slot1_at_nry");
      Check.eq(q.magx[2][1][1], 123, "ring_recx_magx_truncates_123");

      // recz likewise lands on nry==1 while bumping nrz
      q.recz(1, -250.4F, 2);
      Check.eq(q.nrz[2][1], 1, "ring_recz_nrz_1");
      Check.eq(q.nry[2][1], 1, "ring_recz_nry_untouched_1");
      Check.eq(q.rz[2][1][0], 0, "ring_recz_slot0_untouched");
      Check.eq(q.rz[2][1][1], 300, "ring_recz_slot1_at_nry");
      Check.eq(q.magz[2][1][1], -250, "ring_recz_magz_truncates_minus250");

      // recy advances nry to 2, so the next recx/recz jump to slot 2
      q.recy(1, 99.9F, false, 2);
      Check.eq(q.nry[2][1], 2, "ring_recy2_nry_2");
      Check.eq(q.ry[2][1][1], 300, "ring_recy2_slot1_300");
      Check.eq(q.magy[2][1][1], 99, "ring_recy2_magy_99");
      Check.ok(!q.mtouch[2][1], "ring_recy2_mtouch_false");

      q.recx(1, 7.9F, 2);
      Check.eq(q.nrx[2][1], 2, "ring_recx2_nrx_2");
      Check.eq(q.rx[2][1][2], 300, "ring_recx2_slot2_at_nry");
      Check.eq(q.magx[2][1][2], 7, "ring_recx2_magx_7");

      q.recz(1, 20.9F, 2);
      Check.eq(q.nrz[2][1], 2, "ring_recz2_nrz_2");
      Check.eq(q.rz[2][1][2], 300, "ring_recz2_slot2_at_nry");
      Check.eq(q.magz[2][1][2], 20, "ring_recz2_magz_20");

      // three counters, all at 2 after three writes each - but they only
      // agree here because every recx/recz rode a recy advance
      Check.eq(q.nry[2][1], 2, "ring_drift_nry_2");
      Check.eq(q.nrx[2][1], 2, "ring_drift_nrx_2");
      Check.eq(q.nrz[2][1], 2, "ring_drift_nrz_2");

      // recx-only: delegation is frozen, all three writes overwrite slot 0
      Record a = new Record(m);
      a.recx(0, 1.5F, 0);
      a.recx(0, 2.5F, 0);
      a.recx(0, 3.5F, 0);
      Check.eq(a.nrx[0][0], 3, "ring_recxonly_nrx_3");
      Check.eq(a.nry[0][0], 0, "ring_recxonly_nry_0");
      Check.eq(a.magx[0][0][0], 3, "ring_recxonly_slot0_last_value");
      Check.eq(a.rx[0][0][1], 0, "ring_recxonly_slot1_never_written");
   }

   /** Ring wraps: the increment tests ==7 and resets to 0 (:637-639,
    *  :647-649, :845-847), i.e. slot 6 is the last one used. */
   private static void ringWraps() {
      Medium m = new Medium();
      Record w = new Record(m);
      for (int i = 0; i < RING; i++) {
         w.recy(0, i * 10.0F, false, 1);
      }
      Check.eq(w.nry[1][0], 0, "wrap_recy7_nry_back_to_0");
      Check.eq(w.magy[1][0][0], 0, "wrap_recy7_slot0_0");
      Check.eq(w.magy[1][0][6], 60, "wrap_recy7_slot6_60");
      Check.eq(w.ry[1][0][RING_LAST], 300, "wrap_recy7_slot6_marker_300");
      w.recy(0, 99.0F, false, 1);
      Check.eq(w.nry[1][0], 1, "wrap_recy8_nry_1");
      Check.eq(w.magy[1][0][0], 99, "wrap_recy8_slot0_overwritten");
      Check.eq(w.ry[1][0][1], 300, "wrap_recy8_slot1_kept");

      // recx's own counter wraps independently of the slot it writes
      Record x = new Record(m);
      for (int i = 0; i < 8; i++) {
         x.recx(0, i + 0.5F, 0);
      }
      Check.eq(x.nrx[0][0], 1, "wrap_recx8_nrx_1");
      Check.eq(x.magx[0][0][0], 7, "wrap_recx8_slot0_still_last");
      Check.eq(x.rx[0][0][1], 0, "wrap_recx8_slot1_still_zero");

      // recz wraps its OWN counter independently; the slot is still nry's
      Record z = new Record(m);
      for (int i = 0; i < RING; i++) {
         z.recz(0, i + 0.5F, 3);
      }
      Check.eq(z.nrz[3][0], 0, "wrap_recz7_nrz_back_to_0");
      Check.eq(z.magz[3][0][0], 6, "wrap_recz7_slot0_last_6");
      z.recz(0, 9.5F, 3);
      Check.eq(z.nrz[3][0], 1, "wrap_recz8_nrz_1");
      Check.eq(z.magz[3][0][0], 9, "wrap_recz8_slot0_9");

      // the (int) cast truncates toward zero on both signs
      Record c = new Record(m);
      c.recy(0, -1.5F, false, 0);
      Check.eq(c.magy[0][0][0], -1, "cast_recy_neg1_5_to_minus1");
      c.recz(0, 300.9F, 0);
      // recy already advanced nry to 1, so recz/recx land in slot 1
      Check.eq(c.magz[0][0][1], 300, "cast_recz_300_9_to_300");
      c.recx(0, -0.9F, 0);
      Check.eq(c.magx[0][0][1], 0, "cast_recx_neg0_9_to_0");
   }

   /** rec (:543-631): caught, cntf period, fix/dest decrements and the
    *  [frame][car] shift-left-then-insert. Every frame array is
    *  [300][5] = [frame][car] (:14-21), so a write is x[frame][car]. */
   private static void recFrames() {
      Medium m = new Medium();
      final Record fresh = new Record(m);
      final ContO fc = car(m);
      // a never-reset Record has null car[1..5][*], and :552 dereferences it
      Check.throwsIt(new Check.Task() {
         public void run() {
            fresh.rec(fc, 0, 0, 0, 0);
         }
      }, NullPointerException.class, "rec_unreset_history_npe");

      Record f = new Record(m);
      f.reset(five(m));
      ContO c0 = car(m);
      c0.x = 100;
      c0.y = 200;
      c0.z = 300;
      c0.xy = 11;
      c0.zy = 22;
      c0.xz = 33;
      c0.wxz = 44;
      c0.wzy = 55;
      f.x[0][0] = 1;
      f.x[1][0] = 2;
      f.x[FRAMES - 2][0] = 40;
      f.x[FRAMES - 1][0] = 41;
      f.x[FRAMES - 1][1] = 900;
      f.rec(c0, 0, 3, 0, 0);
      // :544-546 caught counts car 0 only
      Check.eq(f.caught, 1, "rec_caught_car0_1");
      Check.eq(f.cntf, 0, "rec_first_call_resets_cntf_0");
      // reset left fix/dest at -1 (:236-238); :563 always decrements fix
      Check.eq(f.fix[0], -2, "rec_fix_decrement_minus2");
      Check.eq(f.dest[0], -1, "rec_dest_not_decremented_when_var5_0");
      // :581-589 shift runs 0..298, copying [f+1] to [f]
      Check.eq(f.x[0][0], 2, "rec_shift_x0_from_x1");
      Check.eq(f.x[FRAMES - 2][0], 41, "rec_shift_x298_from_x299");
      // :591-598 the new frame lands at [299][car]
      Check.eq(f.x[FRAMES - 1][0], 100, "rec_insert_x299");
      Check.eq(f.y[FRAMES - 1][0], 200, "rec_insert_y299");
      Check.eq(f.z[FRAMES - 1][0], 300, "rec_insert_z299");
      Check.eq(f.xy[FRAMES - 1][0], 11, "rec_insert_xy299");
      Check.eq(f.zy[FRAMES - 1][0], 22, "rec_insert_zy299");
      Check.eq(f.xz[FRAMES - 1][0], 33, "rec_insert_xz299");
      Check.eq(f.wxz[FRAMES - 1][0], 44, "rec_insert_wxz299");
      Check.eq(f.wzy[FRAMES - 1][0], 55, "rec_insert_wzy299");
      // only the recorded car's column moves
      Check.eq(f.x[FRAMES - 1][1], 900, "rec_insert_other_car_untouched");
      Check.eq(f.x[0][1], 0, "rec_shift_other_car_untouched");
      // :556-557 the current ContO is cloned into history slot 5
      Check.eq(f.squash[5][0], 3, "rec_squash5_is_var3");

      ContO c1 = car(m);
      c1.x = 7;
      f.rec(c1, 1, 9, 0, 0);
      Check.eq(f.cntf, 1, "rec_second_cntf_1");
      Check.eq(f.caught, 1, "rec_car1_does_not_count_caught");
      Check.eq(f.fix[1], -2, "rec_car1_fix_minus2");
      Check.eq(f.x[FRAMES - 1][1], 7, "rec_car1_insert_x299");
      Check.eq(f.x[FRAMES - 1][0], 100, "rec_car1_does_not_touch_car0_slot299");

      // cntf is ONE shared counter and only re-shifts when it hits 50
      // (:548,:558-561): call 1 shifted, call 52 shifts again
      for (int i = 0; i < 49; i++) {
         f.rec(car(m), 2, 0, 0, 0);
      }
      Check.eq(f.cntf, CNTF, "rec_call51_cntf_50");
      Check.eq(f.fix[2], -50, "rec_call51_fix2_minus50");
      f.rec(car(m), 2, 0, 0, 0);
      Check.eq(f.cntf, 0, "rec_call52_cntf_0");
      Check.eq(f.fix[2], -51, "rec_call52_fix2_minus51");

      // frame 300 does not exist in any [300][5] array
      Check.throwsIt(new Check.Task() {
         public void run() {
            Record e = new Record(m);
            e.play(car(m), new Madness(m, e, null, 1), 0, 300);
         }
      }, ArrayIndexOutOfBoundsException.class, "rec_frame300_play_aioobe");
   }

   /** :563-566 dest only moves when var5!=0; the car-history shift
    *  (:551-554) re-clones car[1..5] into car[0..4], and ContO's copy ctor
    *  (:421-508) is given 0,0,0,0, so the clones have x/y/z==0. */
   private static void recHistory() {
      Medium m = new Medium();
      Record g = new Record(m);
      g.reset(five(m));
      ContO sentinel = car(m);
      sentinel.maxR = 55;
      g.car[2][0] = sentinel;
      g.squash[2][0] = 7;
      g.fix[0] = 5;
      g.dest[0] = 5;
      ContO gc = car(m);
      gc.maxR = 77;
      gc.x = 4242;
      g.rec(gc, 0, 0, 0, 1);
      Check.eq(g.fix[0], 4, "hist_fix_decrement");
      Check.eq(g.dest[0], 4, "hist_dest_decrement_when_var5_1");
      Check.eq(g.car[1][0].maxR, 55, "hist_car2_shifted_to_car1");
      Check.eq(g.squash[1][0], 7, "hist_squash2_shifted_to_1");
      Check.eq(g.car[5][0].maxR, 77, "hist_car5_is_new_clone");
      Check.eq(g.car[5][0].x, 0, "hist_clone_zeroes_x");
      Check.eq(g.car[5][0].npl, 0, "hist_clone_keeps_npl");
      g.rec(car(m), 0, 0, 0, 0);
      Check.eq(g.fix[0], 3, "hist_fix_decrements_again");
      Check.eq(g.dest[0], 4, "hist_dest_frozen_when_var5_0");
   }

   /** rec's spark/ring bookkeeping (:599-630): a live spark is written with
    *  marker 300, then the SAME call subtracts 1 from all 30 slots, so the
    *  freshly written slot reads 299; every ring slot of the recording car
    *  also loses 1. */
   private static void recSpark() {
      Medium m = new Medium();
      Record sp = new Record(m);
      sp.reset(five(m));
      ContO sc = car(m);
      sc.stg[0] = 1;
      sc.sx[0] = 11;
      sc.sy[0] = 12;
      sc.sz[0] = 13;
      sc.smag[0] = 1.5F;
      sc.scx[0] = 14;
      sc.scz[0] = 15;
      sc.fulls[0] = true;
      sp.ry[0][0][0] = 300;
      sp.rec(sc, 0, 0, 0, 0);
      Check.eq(sp.ns[0][0], 1, "spark_ns_1");
      Check.eq(sp.sspark[0][0][0], FRAMES - 1, "spark_marker_300_then_decrement_299");
      Check.eq(sp.sx[0][0][0], 11, "spark_sx_copied");
      Check.eq(sp.sy[0][0][0], 12, "spark_sy_copied");
      Check.eq(sp.sz[0][0][0], 13, "spark_sz_copied");
      Check.eq(bits(sp.smag[0][0][0]), bits(1.5F), "spark_smag_copied");
      Check.eq(sp.scx[0][0][0], 14, "spark_scx_copied");
      Check.eq(sp.scz[0][0][0], 15, "spark_scz_copied");
      Check.ok(sp.fulls[0][0][0], "spark_fulls_copied");
      Check.eq(sp.ry[0][0][0], FRAMES - 1, "spark_ring_slot_decrements");

      sp.rec(sc, 0, 0, 0, 0);
      Check.eq(sp.ns[0][0], 2, "spark2_ns_2");
      Check.eq(sp.sspark[0][0][0], 298, "spark2_slot0_298");
      Check.eq(sp.sspark[0][0][1], FRAMES - 1, "spark2_slot1_299");
      Check.eq(sp.ry[0][0][0], 298, "spark2_ring_slot0_298");
      // part 1 had no spark: it just keeps decaying from the reset -1
      Check.eq(sp.ns[0][1], 0, "spark2_part1_never_marked");
      Check.eq(sp.sspark[0][1][0], -3, "spark2_part1_decays_from_minus1");

      // ns wraps at 30 (:611-614)
      Record spw = new Record(m);
      spw.reset(five(m));
      for (int i = 0; i < 31; i++) {
         spw.rec(sc, 1, 0, 0, 0);
      }
      Check.eq(spw.ns[1][0], 1, "sparkwrap_ns_1");
      Check.eq(spw.sspark[1][0][0], FRAMES - 1, "sparkwrap_slot0_299");
   }

   /** :568-576 - dest is checked AFTER the decrement, so pre-setting 231 +
    *  var5!=0 lands exactly on 230. car 0 fires cotchinow(0) with
    *  whenwasted=229; any other car needs var4!=0 and gets 165+var4. The
    *  h-copy happens BEFORE the shift/insert (:570 before :578), so it
    *  snapshots the PREVIOUS frame. */
   private static void recDest230() {
      Medium m = new Medium();
      Record t = new Record(m);
      ContO[] arr = five(m);
      t.reset(arr);
      t.x[FRAMES - 1][2] = 777;
      t.ry[2][0][3] = 500;
      t.nry[2][0] = 4;
      t.hx[0][0] = 1;
      // squash[0][2] is fed by the history shift, so seed slot 1: the shift
      // copies squash[1][2] into squash[0][2] before cotchinow reads it
      t.squash[1][2] = 6;
      t.caught = 300;
      t.dest[2] = 231;
      t.fix[2] = 8;
      ContO tc = car(m);
      tc.x = 1234;
      tc.maxR = 66;
      t.rec(tc, 2, 4, 9, 1);
      Check.eq(t.dest[2], 230, "dest230_after_decrement");
      Check.eq(t.fix[2], 7, "dest230_fix_decremented");
      Check.eq(t.wasted, 2, "dest230_wasted_is_car");
      Check.eq(t.whenwasted, 174, "dest230_whenwasted_165_plus_var4_9");
      Check.ok(t.hcaught, "dest230_hcaught_true");
      Check.eq(t.x[FRAMES - 1][2], 1234, "dest230_insert_after_copy");
      Check.eq(t.hx[FRAMES - 1][2], 777, "dest230_hcopy_sees_previous_frame");
      Check.eq(t.hfix[2], 7, "dest230_hfix_post_decrement");
      Check.eq(t.hdest[2], 230, "dest230_hdest_230");
      Check.eq(t.hsquash[2], 6, "dest230_hsquash_after_shift");
      Check.eq(t.hry[2][0][3], 500, "dest230_hry_copied_before_ring_decay");
      Check.eq(t.hry[2][0][0], -1, "dest230_hry_untouched_slot");
      Check.eq(t.hnry[2][0], 4, "dest230_hnry_copies_counter");
      Check.eq(t.nry[2][0], 4, "dest230_nry_not_moved_by_cotchinow");
      Check.eq(t.hsspark[2][0][3], -1, "dest230_hsspark_from_reset");
      // starcar comes from car[0] AFTER the history shift, and the copy ctor
      // zeroes the geometry
      // car[0][2] after the shift is the reset copy of arr[2] (maxR 32),
      // not the new tc clone (maxR 66)
      Check.eq(t.starcar[2].maxR, 32, "dest230_starcar_from_car0_after_shift");
      Check.eq(t.starcar[2].x, 0, "dest230_starcar_x_zeroed");

      // other car, var4==0: the 230 check enters but neither arm fires
      Record t2 = new Record(m);
      t2.reset(five(m));
      t2.caught = 300;
      t2.dest[3] = 231;
      t2.rec(car(m), 3, 0, 0, 1);
      Check.eq(t2.dest[3], 230, "dest230_other_car_dest_230");
      Check.eq(t2.wasted, 0, "dest230_other_car_var4_0_no_cotchinow");
      Check.ok(!t2.hcaught, "dest230_other_car_hcaught_false");
      Check.eq(t2.whenwasted, 0, "dest230_other_car_whenwasted_0");

      // other car with var4!=0
      Record t3 = new Record(m);
      t3.reset(five(m));
      t3.caught = 300;
      t3.dest[3] = 231;
      t3.rec(car(m), 3, 0, 5, 1);
      Check.eq(t3.wasted, 3, "dest230_car3_wasted_3");
      Check.eq(t3.whenwasted, 170, "dest230_car3_whenwasted_170");

      // car 0 always calls cotchinow + whenwasted=229, even when caught<300
      Record t4 = new Record(m);
      t4.reset(five(m));
      t4.caught = 0;
      t4.dest[0] = 231;
      t4.rec(car(m), 0, 0, 0, 1);
      Check.eq(t4.caught, 1, "dest230_car0_caught_incremented");
      Check.eq(t4.whenwasted, 229, "dest230_car0_whenwasted_229_regardless");
      Check.eq(t4.wasted, 0, "dest230_car0_caught_below_300_no_wasted");
      Check.ok(!t4.hcaught, "dest230_car0_caught_below_300_no_hcaught");
   }

   /** play (:471-541) loads the eight placement fields from [frame][car]
    *  first - so every later branch (and the reg* damage scatter) sees the
    *  RECORDED angles, not the live ones. play never writes lastfr; that is
    *  playh-only (:366). */
   private static void playFrames() {
      Medium m = new Medium();
      Record pr = new Record(m);
      pr.x[10][3] = 4321;
      pr.y[10][3] = 4322;
      pr.z[10][3] = 4323;
      pr.xy[10][3] = 41;
      pr.zy[10][3] = 42;
      pr.xz[10][3] = 43;
      pr.wxz[10][3] = 44;
      pr.wzy[10][3] = 45;
      pr.lastfr = 77;
      ContO pc = car(m);
      Madness pmd = new Madness(m, pr, null, 1);
      pmd.cn = 0;
      pr.play(pc, pmd, 3, 10);
      Check.eq(pc.x, 4321, "play_x_from_frame_car");
      Check.eq(pc.y, 4322, "play_y_from_frame_car");
      Check.eq(pc.z, 4323, "play_z_from_frame_car");
      Check.eq(pc.xy, 41, "play_xy_from_frame_car");
      Check.eq(pc.zy, 42, "play_zy_from_frame_car");
      Check.eq(pc.xz, 43, "play_xz_from_frame_car");
      Check.eq(pc.wxz, 44, "play_wxz_from_frame_car");
      Check.eq(pc.wzy, 45, "play_wzy_from_frame_car");
      Check.eq(pr.lastfr, 77, "play_leaves_lastfr_alone");
      Check.eq(pr.cntdest[3], 0, "play_cntdest_untouched_when_no_dest");

      // orientation proof: [frame][car], not [car][frame]
      Record pr2 = new Record(m);
      pr2.x[3][0] = 999;
      pr2.x[10][3] = 1234;
      ContO pc2 = car(m);
      pr2.play(pc2, new Madness(m, pr2, null, 1), 3, 10);
      Check.eq(pc2.x, 1234, "play_orientation_frame_then_car");

      // cntdest gates (:480-504)
      Record cd = new Record(m);
      ContO cdc = car(m);
      Madness cdm = new Madness(m, cd, null, 1);
      cd.dest[2] = 5;
      cd.play(cdc, cdm, 2, 5);
      Check.eq(cd.cntdest[2], 6, "play_dest_match_7_then_decrement_6");
      cd.play(cdc, cdm, 2, 5);
      Check.eq(cd.cntdest[2], 6, "play_dest_match_again_6");

      Record cd0 = new Record(m);
      cd0.dest[2] = 5;
      cd0.cntdest[2] = 5;
      cd0.play(car(m), new Madness(m, cd0, null, 1), 2, 0);
      Check.eq(cd0.cntdest[2], 0, "play_frame0_zeroes_cntdest");

      Record cd3 = new Record(m);
      cd3.dest[2] = 0;
      cd3.play(car(m), new Madness(m, cd3, null, 1), 2, 0);
      Check.eq(cd3.cntdest[2], 6, "play_frame0_dest0_7_then_decrement_6");
   }

   /** embos gates (:488-504): frame 0 + dest < -1 -> 13, and any non-zero
    *  cntdest -> 1 for wz==0 planes only (then cntdest--). */
   private static void playGates() {
      Medium m = new Medium();
      Record em = new Record(m);
      ContO emc = planed(m, ZERO_OY, 0);
      em.dest[1] = -2;
      em.play(emc, new Madness(m, em, null, 1), 1, 0);
      Check.eq(emc.p[0].embos, 13, "gate_embos13");
      Check.eq(em.cntdest[1], 0, "gate_embos13_cntdest_0");

      Record em2 = new Record(m);
      ContO emc2 = planed(m, ZERO_OY, 1);
      em2.dest[1] = -2;
      em2.play(emc2, new Madness(m, em2, null, 1), 1, 0);
      Check.eq(emc2.p[0].embos, 0, "gate_embos13_skips_wz_nonzero");

      Record em3 = new Record(m);
      ContO emc3 = planed(m, ZERO_OY, 0);
      em3.dest[0] = 4;
      em3.play(emc3, new Madness(m, em3, null, 1), 0, 4);
      Check.eq(emc3.p[0].embos, 1, "gate_embos1_when_cntdest_nonzero");
      Check.eq(em3.cntdest[0], 6, "gate_embos1_cntdest_decrement");

      Record em4 = new Record(m);
      ContO emc4 = planed(m, ZERO_OY, 1);
      em4.dest[0] = 4;
      em4.play(emc4, new Madness(m, em4, null, 1), 0, 4);
      Check.eq(emc4.p[0].embos, 0, "gate_embos1_skips_wz_nonzero");
   }

   /** The part loop (:506-540): sspark[car][part][cursor]==frame restores
    *  that ContO's spark slot, and rx/ry/rz[car][part][slot]==frame fires
    *  regx/regy/regz. Two matching slots fire twice. play draws the seeded
    *  randomness through reg*, so every pinned plane value below is a pure
    *  function of the TestRng seed. */
   private static void playParts() {
      Medium m = new Medium();
      Record sr = new Record(m);
      sr.sspark[2][1][5] = 9;
      sr.sx[2][1][5] = 111;
      sr.sy[2][1][5] = 112;
      sr.sz[2][1][5] = 113;
      sr.smag[2][1][5] = 1.25F;
      sr.scx[2][1][5] = 114;
      sr.scz[2][1][5] = 115;
      sr.fulls[2][1][5] = true;
      ContO src = car(m);
      src.stg[1] = 3;
      sr.play(src, new Madness(m, sr, null, 1), 2, 9);
      Check.eq(src.stg[1], 1, "parts_sspark_stg_1");
      Check.eq(src.dov[1], -1, "parts_sspark_dov_minus1");
      Check.eq(src.sx[1], 111, "parts_sspark_sx");
      Check.eq(src.sy[1], 112, "parts_sspark_sy");
      Check.eq(src.sz[1], 113, "parts_sspark_sz");
      Check.eq(bits(src.smag[1]), bits(1.25F), "parts_sspark_smag");
      Check.eq(src.scx[1], 114, "parts_sspark_scx");
      Check.eq(src.scz[1], 115, "parts_sspark_scz");
      Check.ok(src.fulls[1], "parts_sspark_fulls");
      // a frame that does not match leaves the slot alone
      ContO src2 = car(m);
      src2.stg[1] = 3;
      sr.play(src2, new Madness(m, sr, null, 1), 2, 8);
      Check.eq(src2.stg[1], 3, "parts_sspark_frame_mismatch");

      TestRng.install(301);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         // the loaded recorded angles drive the scatter direction
         r.xz[7][0] = 45;
         r.xy[7][0] = 30;
         r.zy[7][0] = 60;
         r.rx[0][2][6] = 7;
         r.magx[0][2][6] = 150;
         r.play(c, md, 0, 7);
         // regx: var2 -> 50, var6 = 50/20*m.random(), one draw per qualifying
         // point (3 points)
         Check.eq(cm.draws, 3, "parts_play_regx_draws_3");
         Check.eq(c.xz, 45, "parts_play_loaded_xz");
         Check.eq(c.p[0].ox[0], 0, "parts_play_regx_ox0_0");
         Check.eq(c.p[0].ox[1], 10, "parts_play_regx_ox1_10");
         Check.eq(c.p[0].oz[2], 9, "parts_play_regx_oz2_9");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "parts_play_regx_ctmag_1");
         Check.eq(c.p[0].chip, 1, "parts_play_regx_chip_1");
      } finally {
         TestRng.uninstall();
      }

      // regy uses the recorded zy/xy signs: zy=90 -> sign 0 so the first
      // branch runs; zy=0 -> sign +1 on both axes and NO branch at all
      TestRng.install(302);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.zy[7][0] = 90;
         r.xy[7][0] = 0;
         r.ry[0][0][4] = 7;
         r.magy[0][0][4] = 150;
         r.play(c, md, 0, 7);
         Check.eq(cm.draws, 3, "parts_play_regy_draws_3");
         Check.eq(c.zy, 90, "parts_play_loaded_zy");
         Check.eq(c.p[0].oz[2], 11, "parts_play_regy_oz2_11");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "parts_play_regy_ctmag_1");
         Check.eq(c.p[0].chip, 1, "parts_play_regy_chip_1");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(303);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.rx[0][0][0] = 7;
         r.magx[0][0][0] = 150;
         r.rx[0][0][1] = 7;
         r.magx[0][0][1] = 150;
         r.play(c, md, 0, 7);
         Check.eq(cm.draws, 6, "parts_play_two_ring_hits_draws_6");
      } finally {
         TestRng.uninstall();
      }
   }

   /** playh (:287-367) is play over the h arrays and the ONLY writer of
    *  lastfr (:366); its cntdest/embos gates read hdest (:300,:304). */
   private static void playhFrames() {
      Medium m = new Medium();
      Record ph = new Record(m);
      ph.hx[10][3] = 5321;
      ph.hy[10][3] = 5322;
      ph.hz[10][3] = 5323;
      ph.hxy[10][3] = 51;
      ph.hzy[10][3] = 52;
      ph.hxz[10][3] = 53;
      ph.hwxz[10][3] = 54;
      ph.hwzy[10][3] = 55;
      ContO hc = car(m);
      ph.playh(hc, new Madness(m, ph, null, 1), 3, 10);
      Check.eq(hc.x, 5321, "playh_x_from_frame_car");
      Check.eq(hc.y, 5322, "playh_y_from_frame_car");
      Check.eq(hc.z, 5323, "playh_z_from_frame_car");
      Check.eq(hc.xy, 51, "playh_xy");
      Check.eq(hc.zy, 52, "playh_zy");
      Check.eq(hc.xz, 53, "playh_xz");
      Check.eq(hc.wxz, 54, "playh_wxz");
      Check.eq(hc.wzy, 55, "playh_wzy");
      Check.eq(ph.lastfr, 10, "playh_sets_lastfr_frame");

      // hdest, not dest, drives the gates
      Record hd = new Record(m);
      ContO hdc = car(m);
      hd.hdest[2] = 5;
      hd.playh(hdc, new Madness(m, hd, null, 1), 2, 5);
      Check.eq(hd.cntdest[2], 6, "playh_hdest_match_6");
      hd.playh(hdc, new Madness(m, hd, null, 1), 2, 5);
      Check.eq(hd.cntdest[2], 6, "playh_hdest_match_again_6");

      Record hd2 = new Record(m);
      ContO hd2c = car(m);
      hd2.dest[2] = 5; // dest must NOT be read by playh
      hd2.playh(hd2c, new Madness(m, hd2, null, 1), 2, 5);
      Check.eq(hd2.cntdest[2], 0, "playh_ignores_dest");

      Record hd3 = new Record(m);
      ContO hd3c = planed(m, ZERO_OY, 0);
      hd3.hdest[2] = -2;
      hd3.playh(hd3c, new Madness(m, hd3, null, 1), 2, 0);
      Check.eq(hd3c.p[0].embos, 13, "playh_embos13");
      Check.eq(hd3.cntdest[2], 0, "playh_embos13_cntdest_0");

      Record hd4 = new Record(m);
      ContO hd4c = planed(m, ZERO_OY, 0);
      hd4.hdest[0] = 0;
      hd4.playh(hd4c, new Madness(m, hd4, null, 1), 0, 0);
      Check.eq(hd4c.p[0].embos, 1, "playh_frame0_embos1");
      Check.eq(hd4.cntdest[0], 6, "playh_frame0_cntdest_6");
      Check.eq(hd4.lastfr, 0, "playh_frame0_lastfr_0");

      // the restore reads hsspark/hsx..., never sspark/sx...
      Record hs = new Record(m);
      hs.sspark[2][1][5] = 9;
      hs.sx[2][1][5] = 111;
      ContO hsc = car(m);
      hs.playh(hsc, new Madness(m, hs, null, 1), 2, 9);
      Check.eq(hsc.stg[1], 0, "playh_ignores_live_sspark");
      Record hs2 = new Record(m);
      hs2.hsspark[2][1][5] = 9;
      hs2.hsx[2][1][5] = 111;
      hs2.hsy[2][1][5] = 112;
      hs2.hsz[2][1][5] = 113;
      hs2.hsmag[2][1][5] = 1.25F;
      hs2.hscx[2][1][5] = 114;
      hs2.hscz[2][1][5] = 115;
      hs2.hfulls[2][1][5] = true;
      ContO hsc2 = car(m);
      hsc2.stg[1] = 3;
      hs2.playh(hsc2, new Madness(m, hs2, null, 1), 2, 9);
      Check.eq(hsc2.stg[1], 1, "playh_hsspark_stg_1");
      Check.eq(hsc2.dov[1], -1, "playh_hsspark_dov_minus1");
      Check.eq(hsc2.sx[1], 111, "playh_hsspark_sx");
      Check.eq(hsc2.sy[1], 112, "playh_hsspark_sy");
      Check.eq(hsc2.sz[1], 113, "playh_hsspark_sz");
      Check.eq(bits(hsc2.smag[1]), bits(1.25F), "playh_hsspark_smag");
      Check.eq(hsc2.scx[1], 114, "playh_hsspark_scx");
      Check.eq(hsc2.scz[1], 115, "playh_hsspark_scz");
      Check.ok(hsc2.fulls[1], "playh_hsspark_fulls");
   }

   /** playh's reg-vs-chip switch (:344-362): each ring hit compares lastfr to
    *  the frame first; when they already match, x and z restore through
    *  chipx/chipz (chip/ctmag stamped, geometry untouched), otherwise
    *  through regx/regz. y only has the regy arm, gated on lastfr!=frame. */
   private static void playhLastfr() {
      TestRng.install(201);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.hzy[7][0] = 90;
         r.hxy[7][0] = 0;
         r.hxz[7][0] = 45;
         r.hry[0][0][2] = 7;
         r.hmagy[0][0][2] = 150;
         r.hrz[0][3][1] = 7;
         r.hmagz[0][3][1] = -150;
         r.playh(c, md, 0, 7);
         // 3 draws from regy (part 0) + 3 from regz (part 3)
         Check.eq(cm.draws, 6, "lastfr_before_playh_both_reg_draws_6");
         Check.eq(c.p[0].ox[0], 0, "lastfr_playh_ox0_0");
         Check.eq(c.p[0].ox[1], 9, "lastfr_playh_ox1_9");
         Check.eq(c.p[0].oz[0], 1, "lastfr_playh_oz0_1");
         Check.eq(c.p[0].oz[2], 11, "lastfr_playh_oz2_11");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "lastfr_playh_ctmag");
         Check.eq(r.lastfr, 7, "lastfr_playh_updated");
      } finally {
         TestRng.uninstall();
      }

      // with lastfr == frame, rx goes through chipx: the plane never moves
      TestRng.install(201);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.hzy[7][0] = 90;
         r.hxy[7][0] = 0;
         r.hxz[7][0] = 45;
         r.lastfr = 7;
         r.hrx[0][0][0] = 7;
         r.hmagx[0][0][0] = 150;
         r.playh(c, md, 0, 7);
         Check.eq(cm.draws, 3, "lastfr_chipx_draws_3");
         Check.eq(c.p[0].ox[0], 0, "lastfr_chipx_plane_unmoved_ox");
         Check.eq(c.p[0].oz[2], 10, "lastfr_chipx_plane_unmoved_oz");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "lastfr_chipx_ctmag_1");
         Check.eq(c.p[0].chip, 1, "lastfr_chipx_chip_1");
      } finally {
         TestRng.uninstall();
      }

      // lastfr != frame -> regx: same magnitude, but the plane DOES move
      TestRng.install(201);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.hzy[7][0] = 90;
         r.hxy[7][0] = 0;
         r.hxz[7][0] = 45;
         r.lastfr = 6;
         r.hrx[0][0][0] = 7;
         r.hmagx[0][0][0] = 150;
         r.playh(c, md, 0, 7);
         Check.eq(cm.draws, 3, "lastfr_regx_draws_3");
         Check.eq(c.p[0].ox[0], 0, "lastfr_regx_ox0_0");
         Check.eq(c.p[0].oz[0], 0, "lastfr_regx_oz0_0");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "lastfr_regx_ctmag_1");
         Check.eq(r.lastfr, 7, "lastfr_regx_sets_lastfr");
      } finally {
         TestRng.uninstall();
      }

      // recorded angles that leave both sign factors at +1: regy's two
      // branches are BOTH skipped, so no randomness is consumed at all
      TestRng.install(202);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.hzy[7][0] = 0;
         r.hxy[7][0] = 0;
         r.hxz[7][0] = 45;
         r.hry[0][0][2] = 7;
         r.hmagy[0][0][2] = 150;
         r.playh(c, md, 0, 7);
         Check.eq(cm.draws, 0, "lastfr_regy_signs_plus1_no_branch_draws_0");
         Check.eq(c.p[0].oz[0], 0, "lastfr_regy_signs_plus1_plane_untouched");
      } finally {
         TestRng.uninstall();
      }
   }

   /** regx (:766-839). The guard is Math.abs(var2) > 100 (so exactly 100 is
    *  inert), the plane is 3 points all inside clrad[0]==3300, and every
    *  qualifying point consumes one m.random() draw: force = var2/20*r.
    *  m.random() returns tenths (Medium.java:88), so ctmag is quantised to
    *  var2/200. */
   private static void regxGroup() {
      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regx(0, 150.0F, c, md);
         Check.eq(cm.draws, 3, "regx_draws_3");
         Check.eq(c.p[0].ox[0], 0, "regx_ox0_0");
         Check.eq(c.p[0].ox[1], 10, "regx_ox1_10");
         Check.eq(c.p[0].ox[2], 0, "regx_ox2_0");
         Check.eq(c.p[0].oz[0], 0, "regx_oz0_0");
         Check.eq(c.p[0].oz[1], 0, "regx_oz1_0");
         Check.eq(c.p[0].oz[2], 9, "regx_oz2_9");
         Check.eq(c.p[0].chip, 1, "regx_chip_1");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "regx_ctmag_1");
         // nocol plane (oc all equal, Plane.java:132-134) -> no tint work
         Check.eq(c.p[0].bfase, 0, "regx_bfase_untouched_nocol");
      } finally {
         TestRng.uninstall();
      }

      Medium m = new Medium();
      // strict > 100: 100.0F is a no-op in all five damage methods
      Record r100 = new Record(m);
      ContO c100 = planed(m, ZERO_OY, 0);
      r100.regx(0, 100.0F, c100, new Madness(m, r100, null, 1));
      Check.eq(c100.p[0].ox[1], 10, "regx_100_noop_ox");
      Check.eq(c100.p[0].oz[2], 10, "regx_100_noop_oz");
      Check.eq(c100.p[0].chip, 0, "regx_100_noop_chip");

      // npl==0 (the empty-byte fixture): the loops never run
      TenthsMedium cm0 = new TenthsMedium();
      Record r0 = new Record(cm0);
      ContO c0 = car(cm0);
      r0.regx(0, 150.0F, c0, new Madness(cm0, r0, null, 1));
      Check.eq(cm0.draws, 0, "regx_npl0_no_draws");

      // clrad gate: a point at (500,500) is outside 3300 and draws nothing
      TenthsMedium cmf = new TenthsMedium();
      Record rf = new Record(cmf);
      ContO cf = new ContO(new byte[0], cmf, new Trackers());
      cf.npl = 1;
      cf.p[0] = new Plane(cmf, new Trackers(), new int[]{500, 10, 0}, new int[]{500, 0, 10},
            ZERO_OY, 3, new int[]{1, 1, 1}, false, 0, 0, 0, 0, 7, 0, false);
      rf.regx(0, 150.0F, cf, new Madness(cmf, rf, null, 1));
      Check.eq(cmf.draws, 2, "regx_far_point_draws_2");
      Check.eq(cf.p[0].ox[0], 500, "regx_far_point_ox_unchanged");

      // each plane runs the whole point loop: two planes -> six draws
      TenthsMedium cm2 = new TenthsMedium();
      Record r2 = new Record(cm2);
      ContO c2 = planed(cm2, ZERO_OY, 0);
      c2.npl = 2;
      c2.p[1] = new Plane(cm2, new Trackers(), new int[]{0, 4, 0}, new int[]{0, 0, 4},
            ZERO_OY, 3, new int[]{1, 1, 1}, false, 0, 0, 0, 0, 7, 0, false);
      r2.regx(0, 150.0F, c2, new Madness(cm2, r2, null, 1));
      Check.eq(cm2.draws, 6, "regx_two_planes_draws_6");
   }

   /** regy (:78-221). Its guard is `var2 > 100.0F` - NO absolute value - so
    *  a large negative damage is fully inert. The branches are chosen from
    *  the sign of the normalised zy/xy (:86-118): signZ*signY==0 (or the
    *  hmtouch flag) runs the x/z scatter with force var2/20; ==-1 runs the
    *  y lift with force var2/15 and folds the result into squash[0][im]
    *  using a denominator that starts at 1. */
   private static void regyGroup() {
      TestRng.install(101);
      try {
         // branch A: zy=90 -> signZ 0, xy=0 -> signY +1, product 0
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         c.zy = 90;
         c.xy = 0;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regy(0, 150.0F, false, c, md);
         Check.eq(cm.draws, 3, "regy_branchA_draws_3");
         Check.eq(c.p[0].ox[0], 0, "regy_branchA_ox_untouched");
         Check.eq(c.p[0].oz[0], 1, "regy_branchA_oz0_1");
         // the seed-101 vector is 0.4, 0.0, 0.4 tenths: the middle draw
         // is exactly 0.0, so point 1 receives no lift at all
         Check.eq(c.p[0].oz[1], 0, "regy_branchA_oz1_0");
         Check.eq(c.p[0].oz[2], 11, "regy_branchA_oz2_11");
         Check.eq(c.p[0].oy[0], 0, "regy_branchA_oy_untouched");
         Check.eq(c.p[0].chip, 1, "regy_branchA_chip_1");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "regy_branchA_ctmag_1");
         Check.eq(r.squash[0][1], 0, "regy_branchA_squash_untouched");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         // branch B: zy=180 -> signZ -1, xy=0 -> signY +1, product -1
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, new int[]{-50, -45, -40}, 0);
         c.zy = 180;
         c.xy = 0;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regy(0, 150.0F, false, c, md);
         Check.eq(cm.draws, 3, "regy_branchB_draws_3");
         Check.eq(c.p[0].oy[0], -48, "regy_branchB_oy0_minus48");
         Check.eq(c.p[0].oy[1], -45, "regy_branchB_oy1_minus45");
         Check.eq(c.p[0].oy[2], -38, "regy_branchB_oy2_minus38");
         Check.eq(c.p[0].ox[1], 10, "regy_branchB_ox_untouched");
         // three points lift, but var17 starts at 1 (:186) so the divisor is
         // 4 and the int-accumulated var16 == 4 -> squash += 1
         Check.eq(r.squash[0][1], 0, "regy_branchB_squash_0");
         // last var13 = 50/15 * 0.4f == 1.3333334
         Check.eq(bits(c.p[0].ctmag), 1068149419, "regy_branchB_ctmag_1_3333334");
         Check.eq(c.p[0].chip, 1, "regy_branchB_chip_1");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         // var3 true forces branch A on top of branch B: six draws
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, new int[]{-50, -45, -40}, 0);
         c.zy = 180;
         c.xy = 0;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regy(0, 150.0F, true, c, md);
         Check.eq(cm.draws, 6, "regy_var3_both_branches_draws_6");
         Check.eq(c.p[0].oy[0], -49, "regy_var3_oy0_minus49");
         Check.eq(c.p[0].oy[1], -44, "regy_var3_oy1_minus44");
         Check.eq(c.p[0].oy[2], -39, "regy_var3_oy2_minus39");
         Check.eq(c.p[0].oz[0], 0, "regy_var3_oz_unchanged_at_180");
         Check.eq(r.squash[0][1], 0, "regy_var3_squash_0");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "regy_var3_ctmag_1");
      } finally {
         TestRng.uninstall();
      }

      // zy=0 -> signZ +1 and signY +1: neither branch runs
      Medium m = new Medium();
      TenthsMedium cmz = new TenthsMedium();
      Record rz = new Record(cmz);
      ContO cz = planed(cmz, new int[]{-50, -45, -40}, 0);
      cz.zy = 0;
      cz.xy = 0;
      rz.regy(0, 150.0F, false, cz, new Madness(cmz, rz, null, 1));
      Check.eq(cmz.draws, 0, "regy_zy0_both_signs_plus1_draws_0");
      Check.eq(cz.p[0].oy[0], -50, "regy_zy0_oy_untouched");

      // negative damage is inert: no abs() around the guard
      TenthsMedium cmn = new TenthsMedium();
      Record rn = new Record(cmn);
      ContO cn = planed(cmn, ZERO_OY, 0);
      rn.regy(0, -1000.0F, false, cn, new Madness(cmn, rn, null, 1));
      Check.eq(cmn.draws, 0, "regy_negative_inert_draws_0");
      TenthsMedium cm1 = new TenthsMedium();
      Record r1 = new Record(cm1);
      ContO c1 = planed(cm1, ZERO_OY, 0);
      r1.regy(0, 100.0F, false, c1, new Madness(cm1, r1, null, 1));
      Check.eq(cm1.draws, 0, "regy_100_noop_draws_0");
      Check.eq(regyNplZeroDraws(), 0, "regy_empty_fixture_draws_0");
   }

   /** regz (:396-469): same abs() guard as regx, but the displacement uses
    *  cos(xz)*cos(zy) for oz and sin(xz)*cos(xy) for ox (:412-413). */
   private static void regzGroup() {
      TestRng.install(102);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regz(0, -150.0F, c, md);
         Check.eq(cm.draws, 3, "regz_draws_3");
         Check.eq(c.p[0].ox[0], 0, "regz_ox0_0");
         Check.eq(c.p[0].ox[1], 10, "regz_ox1_10");
         Check.eq(c.p[0].ox[2], 0, "regz_ox2_0");
         Check.eq(c.p[0].oz[0], 0, "regz_oz0_0");
         Check.eq(c.p[0].oz[1], 0, "regz_oz1_0");
         Check.eq(c.p[0].oz[2], 9, "regz_oz2_9");
         Check.eq(c.p[0].chip, 1, "regz_chip_1");
         Check.eq(bits(c.p[0].ctmag), bits(-1.0F), "regz_ctmag_minus1");
         Check.eq(c.p[0].bfase, 0, "regz_bfase_untouched_nocol");
      } finally {
         TestRng.uninstall();
      }

      Medium m = new Medium();
      Record r100 = new Record(m);
      ContO c100 = planed(m, ZERO_OY, 0);
      r100.regz(0, 100.0F, c100, new Madness(m, r100, null, 1));
      Check.eq(c100.p[0].ox[1], 10, "regz_100_noop_ox");
      Check.eq(c100.p[0].chip, 0, "regz_100_noop_chip");
      Record rneg = new Record(m);
      ContO cneg = planed(m, ZERO_OY, 0);
      rneg.regz(0, -100.0F, cneg, new Madness(m, rneg, null, 1));
      Check.eq(cneg.p[0].ox[1], 10, "regz_minus100_noop_ox");

      TenthsMedium cm0 = new TenthsMedium();
      Record r0 = new Record(cm0);
      ContO c0 = car(cm0);
      r0.regz(0, 150.0F, c0, new Madness(cm0, r0, null, 1));
      Check.eq(cm0.draws, 0, "regz_npl0_no_draws");
   }

   /** chipx/chipz (:739-764, :369-394): identical magnitude handling but
    *  they only stamp chip/ctmag; the plane geometry is never touched. */
   private static void chipGroup() {
      TestRng.install(102);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.chipx(0, 150.0F, c, md);
         Check.eq(cm.draws, 3, "chipx_draws_3");
         Check.eq(c.p[0].ox[1], 10, "chipx_plane_unmoved");
         Check.eq(c.p[0].chip, 1, "chipx_chip_1");
         Check.eq(bits(c.p[0].ctmag), bits(1.0F), "chipx_ctmag_1");
         Check.eq(c.p[0].bfase, 0, "chipx_bfase_untouched");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.chipz(0, -150.0F, c, md);
         Check.eq(cm.draws, 3, "chipz_draws_3");
         Check.eq(c.p[0].oz[2], 10, "chipz_plane_unmoved");
         Check.eq(c.p[0].chip, 1, "chipz_chip_1");
         Check.eq(bits(c.p[0].ctmag), bits(-1.0F), "chipz_ctmag_minus1");
      } finally {
         TestRng.uninstall();
      }

      Medium m = new Medium();
      Record r99 = new Record(m);
      ContO c99 = planed(m, ZERO_OY, 0);
      r99.chipz(0, 99.9F, c99, new Madness(m, r99, null, 1));
      Check.eq(c99.p[0].chip, 0, "chipz_99_9_noop");
      Record r100 = new Record(m);
      ContO c100 = planed(m, ZERO_OY, 0);
      r100.chipx(0, -100.0F, c100, new Madness(m, r100, null, 1));
      Check.eq(c100.p[0].chip, 0, "chipx_minus100_noop");

      TenthsMedium cm0 = new TenthsMedium();
      Record r0 = new Record(cm0);
      ContO c0 = car(cm0);
      r0.chipx(0, 150.0F, c0, new Madness(cm0, r0, null, 1));
      r0.chipz(0, 150.0F, c0, new Madness(cm0, r0, null, 1));
      Check.eq(cm0.draws, 0, "chip_npl0_no_draws");
   }

   /** The damage tint (:131-180, :417-466, :787-836): a NON-nocol plane is
    *  tinted - the HSB clamps are selected by bfase, c[] is re-derived with
    *  Color.getHSBColor, then bfase advances by the force magnitude. glass
    *  planes skip the tint and lose gr instead (:177-179, :206-210).
    *  The force magnitudes are the same seed-101 tenths vector, so these
    *  values are a pure function of the install. */
   private static void regTint() {
      int[] oc = new int[]{200, 100, 50};
      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = tinted(cm, oc, 25, false, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regx(0, 150.0F, c, md);
         Check.eq(cm.draws, 3, "tint_regx_draws_3");
         Check.eq(bits(c.p[0].hsb[1]), bits(0.2F), "tint_regx_bfase25_hsb1_0_2");
         Check.eq(c.p[0].c[0], 200, "tint_regx_bfase25_c0");
         Check.eq(c.p[0].c[1], 173, "tint_regx_bfase25_c1");
         Check.eq(c.p[0].c[2], 160, "tint_regx_bfase25_c2");
         Check.eq(c.p[0].bfase, 26, "tint_regx_bfase_advances_by_force");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = tinted(cm, oc, 65, false, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regx(0, 150.0F, c, md);
         // bfase>40/50/60: hsb[1]=0.05, hsb[0]=0.075 -> 0.05
         Check.eq(bits(c.p[0].hsb[0]), bits(0.05F), "tint_regx_bfase65_hsb0_0_05");
         Check.eq(bits(c.p[0].hsb[1]), bits(0.05F), "tint_regx_bfase65_hsb1_0_05");
         Check.eq(c.p[0].c[0], 200, "tint_regx_bfase65_c0");
         Check.eq(c.p[0].c[1], 193, "tint_regx_bfase65_c1");
         Check.eq(c.p[0].c[2], 190, "tint_regx_bfase65_c2");
         Check.eq(c.p[0].bfase, 66, "tint_regx_bfase65_advance");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = tinted(cm, oc, 0, true, 100);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regx(0, 150.0F, c, md);
         Check.eq(cm.draws, 3, "tint_glass_draws_3");
         Check.eq(c.p[0].bfase, 0, "tint_glass_skips_bfase");
         Check.eq(c.p[0].gr, 98, "tint_glass_gr_100_minus_1_5");
         Check.eq(c.p[0].chip, 1, "tint_glass_chip_1");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = tinted(cm, oc, 55, false, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regz(0, -150.0F, c, md);
         Check.eq(bits(c.p[0].hsb[0]), bits(0.075F), "tint_regz_bfase55_hsb0_0_075");
         Check.eq(bits(c.p[0].hsb[1]), bits(0.05F), "tint_regz_bfase55_hsb1_0_05");
         Check.eq(c.p[0].c[0], 200, "tint_regz_bfase55_c0");
         Check.eq(c.p[0].c[1], 195, "tint_regz_bfase55_c1");
         Check.eq(c.p[0].c[2], 190, "tint_regz_bfase55_c2");
         Check.eq(c.p[0].bfase, 56, "tint_regz_bfase55_advance");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = tinted(cm, oc, 0, false, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 90;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regy(0, 150.0F, false, c, md);
         Check.eq(cm.draws, 3, "tint_regy_draws_3");
         Check.eq(c.p[0].bfase, 1, "tint_regy_branchA_bfase_advance");
         Check.eq(c.p[0].c[1], 100, "tint_regy_branchA_color_unchanged");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = tinted(cm, oc, 0, true, 100);
         c.zy = 180;
         c.xy = 0;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.regy(0, 150.0F, false, c, md);
         // branch B's glass arm subtracts a flat 5 (:206-210)
         Check.eq(c.p[0].gr, 95, "tint_regy_glass_gr_95");
         Check.eq(c.p[0].bfase, 0, "tint_regy_glass_bfase_untouched");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         c.xz = 45;
         c.xy = 30;
         c.zy = 60;
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         // -250 -> one-shot +100 -> -150, so the force is -7.5*random, not
         // -2.5*random (:772-774)
         r.regx(0, -250.0F, c, md);
         Check.eq(bits(c.p[0].ctmag), bits(-3.0F), "tint_regx_neg250_ctmag_minus3");
         Check.eq(c.p[0].ox[0], -1, "tint_regx_neg250_ox0_minus1");
         Check.eq(c.p[0].oz[2], 11, "tint_regx_neg250_oz2_11");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm = new TenthsMedium();
         Record r = new Record(cm);
         ContO c = planed(cm, ZERO_OY, 0);
         Madness md = new Madness(cm, r, null, 1);
         md.cn = 0;
         r.chipx(0, -250.0F, c, md);
         Check.eq(bits(c.p[0].ctmag), bits(-3.0F), "tint_chipx_neg250_ctmag_minus3");
         Check.eq(c.p[0].ox[1], 10, "tint_chipx_neg250_plane_unmoved");
      } finally {
         TestRng.uninstall();
      }

      // angle normalisation (:86-116): zy=200 and xy=200 both normalise to
      // -1, so neither branch runs; zy=90 gives 0 and xy=200 gives -1, so
      // the product is 0 and branch A runs
      TestRng.install(101);
      try {
         TenthsMedium cm1 = new TenthsMedium();
         Record r1 = new Record(cm1);
         ContO c1 = planed(cm1, ZERO_OY, 0);
         c1.zy = 200;
         c1.xy = 200;
         r1.regy(0, 150.0F, false, c1, new Madness(cm1, r1, null, 1));
         Check.eq(cm1.draws, 0, "tint_regy_200_200_draws_0");
         Check.eq(c1.p[0].oz[0], 0, "tint_regy_200_200_plane_untouched");
      } finally {
         TestRng.uninstall();
      }

      TestRng.install(101);
      try {
         TenthsMedium cm2 = new TenthsMedium();
         Record r2 = new Record(cm2);
         ContO c2 = planed(cm2, ZERO_OY, 0);
         c2.zy = 90;
         c2.xy = 200;
         r2.regy(0, 150.0F, false, c2, new Madness(cm2, r2, null, 1));
         Check.eq(cm2.draws, 3, "tint_regy_90_200_draws_3");
         Check.eq(c2.p[0].oz[0], 1, "tint_regy_90_200_oz0_1");
      } finally {
         TestRng.uninstall();
      }
   }

   /** cotchinow (:652-737): gated on caught>=300, sets wasted to its
    *  ARGUMENT, snapshots car[0]/squash[0]/fix/dest into the h arrays and
    *  sets hcaught. The spark and damage rings are COPIED with their
    *  counters - nothing is moved or reset. */
   private static void cotchinowGroup() {
      Medium m = new Medium();
      Record ct = new Record(m);
      ContO[] arr = five(m);
      ct.reset(arr);
      ct.caught = 299;
      ct.wasted = 99;
      ct.cotchinow(4);
      Check.eq(ct.wasted, 99, "cotch_below_300_no_wasted");
      Check.ok(!ct.hcaught, "cotch_below_300_no_hcaught");
      Check.eq(ct.hfix[0], -1, "cotch_below_300_hfix_untouched");
      Check.eq(ct.hx[0][0], 0, "cotch_below_300_hx_untouched");

      ct.caught = 300;
      ct.wasted = 99;
      ct.x[0][0] = 30;
      ct.y[0][0] = 31;
      ct.z[0][0] = 32;
      ct.xy[0][0] = 33;
      ct.zy[0][0] = 34;
      ct.xz[0][0] = 35;
      ct.wxz[0][0] = 36;
      ct.wzy[0][0] = 37;
      ct.x[299][4] = 12345;
      ct.hx[299][4] = 0;
      ct.nry[0][0] = 5;
      ct.nrx[0][0] = 4;
      ct.nrz[0][0] = 3;
      ct.ns[0][0] = 9;
      ct.ry[0][0][1] = 41;
      ct.magy[0][0][1] = 42;
      ct.rx[0][0][2] = 43;
      ct.magx[0][0][2] = 44;
      ct.rz[0][0][3] = 45;
      ct.magz[0][0][3] = 46;
      ct.sspark[0][0][4] = 47;
      ct.sx[0][0][4] = 48;
      ct.sy[0][0][4] = 49;
      ct.sz[0][0][4] = 50;
      ct.smag[0][0][4] = 1.75F;
      ct.scx[0][0][4] = 51;
      ct.scz[0][0][4] = 52;
      ct.fulls[0][0][4] = true;
      ct.mtouch[0][6] = true;
      ct.squash[0][0] = 12;
      ct.fix[0] = 13;
      ct.dest[0] = 14;
      ct.car[0][0] = new ContO(car(m), 0, 0, 0, 0);
      ct.car[0][0].maxR = 88;
      ct.car[0][0].x = 999;
      ct.cotchinow(4);
      Check.eq(ct.wasted, 4, "cotch_wasted_is_argument");
      Check.ok(ct.hcaught, "cotch_hcaught_true");
      Check.eq(ct.hx[0][0], 30, "cotch_hx");
      Check.eq(ct.hy[0][0], 31, "cotch_hy");
      Check.eq(ct.hz[0][0], 32, "cotch_hz");
      Check.eq(ct.hxy[0][0], 33, "cotch_hxy");
      Check.eq(ct.hzy[0][0], 34, "cotch_hzy");
      Check.eq(ct.hxz[0][0], 35, "cotch_hxz");
      Check.eq(ct.hwxz[0][0], 36, "cotch_hwxz");
      Check.eq(ct.hwzy[0][0], 37, "cotch_hwzy");
      Check.eq(ct.hx[299][4], 12345, "cotch_hx_far_corner");
      Check.eq(ct.hns[0][0], 9, "cotch_hns_copied");
      Check.eq(ct.hnry[0][0], 5, "cotch_hnry_copied");
      Check.eq(ct.hnrx[0][0], 4, "cotch_hnrx_copied");
      Check.eq(ct.hnrz[0][0], 3, "cotch_hnrz_copied");
      // counters are copied, NOT moved
      Check.eq(ct.nry[0][0], 5, "cotch_nry_not_moved");
      Check.eq(ct.nrx[0][0], 4, "cotch_nrx_not_moved");
      Check.eq(ct.ns[0][0], 9, "cotch_ns_not_moved");
      Check.eq(ct.hry[0][0][1], 41, "cotch_hry");
      Check.eq(ct.hmagy[0][0][1], 42, "cotch_hmagy");
      Check.eq(ct.hrx[0][0][2], 43, "cotch_hrx");
      Check.eq(ct.hmagx[0][0][2], 44, "cotch_hmagx");
      Check.eq(ct.hrz[0][0][3], 45, "cotch_hrz");
      Check.eq(ct.hmagz[0][0][3], 46, "cotch_hmagz");
      Check.eq(ct.hsspark[0][0][4], 47, "cotch_hsspark");
      Check.eq(ct.hsx[0][0][4], 48, "cotch_hsx");
      Check.eq(ct.hsy[0][0][4], 49, "cotch_hsy");
      Check.eq(ct.hsz[0][0][4], 50, "cotch_hsz");
      Check.eq(bits(ct.hsmag[0][0][4]), bits(1.75F), "cotch_hsmag");
      Check.eq(ct.hscx[0][0][4], 51, "cotch_hscx");
      Check.eq(ct.hscz[0][0][4], 52, "cotch_hscz");
      Check.ok(ct.hfulls[0][0][4], "cotch_hfulls");
      Check.ok(ct.hmtouch[0][6], "cotch_hmtouch");
      Check.eq(ct.hsquash[0], 12, "cotch_hsquash");
      Check.eq(ct.hfix[0], 13, "cotch_hfix");
      Check.eq(ct.hdest[0], 14, "cotch_hdest");
      // starcar is a clone of car[0], so geometry is zeroed and fields kept
      Check.eq(ct.starcar[0].maxR, 88, "cotch_starcar_from_car0");
      Check.eq(ct.starcar[0].x, 0, "cotch_starcar_x_zeroed");

      ct.sspark[4][3][29] = 66;
      ct.ry[4][3][6] = 67;
      ct.mtouch[4][6] = true;
      ct.cotchinow(1);
      Check.eq(ct.wasted, 1, "cotch_second_call_wasted");
      Check.eq(ct.hsspark[4][3][29], 66, "cotch_hsspark_far_corner");
      Check.eq(ct.hry[4][3][6], 67, "cotch_hry_far_corner");
      Check.ok(ct.hmtouch[4][6], "cotch_hmtouch_far_corner");
   }

   /** reset(ContO[]) (:223-279): clears the counters and rebuilds car/squash
    *  plus the spark and damage rings - but ONLY for car indices 0..3
    *  (`while (++var2 < 4)` at :276). Index 4 keeps whatever it held. It
    *  never touches cntf, lastfr, the h arrays or x/y/z, and prepit gates
    *  only the starcar allocation. */
   private static void resetGroup() {
      Medium m = new Medium();
      Record rs = new Record(m);
      ContO[] arr = five(m);
      rs.caught = 7;
      rs.hcaught = true;
      rs.wasted = 8;
      rs.whenwasted = 9;
      rs.powered = 10;
      rs.cntf = 37;
      rs.lastfr = 55;
      rs.fix[0] = 3;
      rs.dest[1] = 4;
      rs.cntdest[2] = 5;
      rs.squash[2][3] = 6;
      rs.sspark[0][0][0] = 71;
      rs.ns[0][0] = 9;
      rs.ry[0][0][0] = 72;
      rs.rx[1][1][1] = 73;
      rs.rz[2][2][2] = 74;
      rs.nry[0][0] = 5;
      rs.nrx[1][1] = 6;
      rs.nrz[2][2] = 7;
      rs.car[3][4] = null;
      // car index 4 is OUTSIDE reset's loops
      rs.sspark[4][0][0] = 81;
      rs.ns[4][0] = 8;
      rs.ry[4][0][0] = 82;
      rs.rx[4][1][0] = 83;
      rs.rz[4][2][0] = 84;
      rs.nry[4][0] = 9;
      rs.nrx[4][1] = 10;
      rs.nrz[4][2] = 11;
      // h side and x/y/z are untouched by reset
      rs.hx[0][0] = 91;
      rs.hry[0][0][0] = 92;
      rs.x[5][0] = 93;
      rs.ocar[0] = car(m);
      rs.ocar[0].maxR = 95;
      rs.hcaught = true;

      rs.reset(arr);
      Check.eq(rs.caught, 0, "reset_caught_0");
      Check.ok(!rs.hcaught, "reset_hcaught_false");
      Check.eq(rs.wasted, 0, "reset_wasted_0");
      Check.eq(rs.whenwasted, 0, "reset_whenwasted_0");
      Check.eq(rs.powered, 0, "reset_powered_0");
      Check.ok(!rs.prepit, "reset_prepit_false");
      Check.eq(rs.cntf, 37, "reset_preserves_cntf");
      Check.eq(rs.lastfr, 55, "reset_preserves_lastfr");
      for (int i = 0; i < 5; i++) {
         Check.eq(rs.fix[i], -1, "reset_fix" + i + "_minus1");
         Check.eq(rs.dest[i], -1, "reset_dest" + i + "_minus1");
         Check.eq(rs.cntdest[i], 0, "reset_cntdest" + i + "_0");
         Check.eq(rs.starcar[i].maxR, 30 + i, "reset_starcar" + i + "_from_array");
      }
      Check.ok(rs.car[0][0] != null, "reset_car00_rebuilt");
      Check.eq(rs.car[3][4].maxR, 34, "reset_car34_rebuilt");
      Check.eq(rs.squash[2][3], 0, "reset_squash_cleared");
      Check.eq(rs.sspark[0][0][0], -1, "reset_sspark_minus1");
      Check.eq(rs.sspark[0][0][29], -1, "reset_sspark_last_minus1");
      Check.eq(rs.ns[0][0], 0, "reset_ns_0");
      Check.eq(rs.ry[0][0][0], -1, "reset_ry_minus1");
      Check.eq(rs.rx[1][1][1], -1, "reset_rx_minus1");
      Check.eq(rs.rz[2][2][2], -1, "reset_rz_minus1");
      Check.eq(rs.nry[0][0], 0, "reset_nry_0");
      Check.eq(rs.nrx[1][1], 0, "reset_nrx_0");
      Check.eq(rs.nrz[2][2], 0, "reset_nrz_0");
      // car index 4 preserved
      Check.eq(rs.sspark[4][0][0], 81, "reset_car4_sspark_preserved");
      Check.eq(rs.ns[4][0], 8, "reset_car4_ns_preserved");
      Check.eq(rs.ry[4][0][0], 82, "reset_car4_ry_preserved");
      Check.eq(rs.rx[4][1][0], 83, "reset_car4_rx_preserved");
      Check.eq(rs.rz[4][2][0], 84, "reset_car4_rz_preserved");
      Check.eq(rs.nry[4][0], 9, "reset_car4_nry_preserved");
      Check.eq(rs.nrx[4][1], 10, "reset_car4_nrx_preserved");
      Check.eq(rs.nrz[4][2], 11, "reset_car4_nrz_preserved");
      // h side, x/y/z and ocar preserved
      Check.eq(rs.hx[0][0], 91, "reset_preserves_hx");
      Check.eq(rs.hry[0][0][0], 92, "reset_preserves_hry");
      Check.eq(rs.x[5][0], 93, "reset_preserves_x");
      Check.eq(rs.ocar[0].maxR, 95, "reset_preserves_ocar");

      // prepit==false: starcar is not reallocated (identity preserved) and
      // the source array is no longer consulted
      ContO keep = rs.starcar[0];
      arr[0].maxR = 1234;
      rs.reset(arr);
      Check.ok(keep == rs.starcar[0], "reset2_starcar_identity_kept");
      Check.eq(rs.starcar[0].maxR, 30, "reset2_starcar_not_refreshed");
      Check.ok(!rs.prepit, "reset2_prepit_still_false");

      Record rs3 = new Record(m);
      rs3.prepit = false;
      ContO[] arr3 = five(m);
      rs3.reset(arr3);
      Check.eq(rs3.starcar[0], null, "reset_prepit_false_skips_starcar");
      Check.ok(rs3.car[0][0] != null, "reset_prepit_false_still_rebuilds_car");
   }

   /** py (:850-852): two squared differences summed in plain int, so Java's
    *  32-bit wrap applies with no abs, no sqrt and no clamp. */
   private static void pyGroup() {
      Medium m = new Medium();
      Record p = new Record(m);
      Check.eq(p.py(1, 0, 1, 0), 2, "py_1_0_1_0_2");
      Check.eq(p.py(0, 0, 0, 0), 0, "py_same_0");
      Check.eq(p.py(10, 20, 30, 40), 200, "py_10_20_30_40_200");
      Check.eq(p.py(-1, 0, 0, 0), 1, "py_neg1_0_1");
      // pairs are (a,b) and (c,d), not (a,c) and (b,d)
      Check.eq(p.py(-3, -4, -3, -4), 2, "py_neg_pairs_2");
      Check.eq(p.py(30000, 0, 0, 0), 900000000, "py_30000_900000000");
      // 65536^2 == 4294967296 wraps to exactly 0
      Check.eq(p.py(65536, 0, 0, 0), 0, "py_65536_wraps_to_0");
      Check.eq(p.py(32768, 0, 0, 0), 1073741824, "py_32768_1<<30");
      // 46341^2 == 2147488281 overflows to a negative int
      Check.eq(p.py(46341, 0, 0, 0), -2147479015, "py_46341_wraps_negative");
      // Integer.MIN_VALUE: (x-0)*(x-0) wraps to 0
      Check.eq(p.py(Integer.MIN_VALUE, 0, 0, 0), 0, "py_int_min_wraps_to_0");
      // MAX_VALUE^2 wraps to 1
      Check.eq(p.py(0, Integer.MAX_VALUE, 0, 0), 1, "py_int_max_wraps_to_1");
   }

   /** The empty-byte ContO seam: npl==0 and every placement field the
    *  recorder reads is 0 (ContO.java:207-419; proven by TMadnessWorld and
    *  TSmall). Re-stating them keeps the fixture from drifting. */
   private static ContO car(Medium m) {
      ContO c = new ContO(new byte[0], m, new Trackers());
      c.npl = 0;
      c.x = 0;
      c.y = 0;
      c.z = 0;
      c.xz = 0;
      c.xy = 0;
      c.zy = 0;
      c.wxz = 0;
      c.wzy = 0;
      c.dist = 0;
      c.maxR = 0;
      c.grat = 0;
      for (int i = 0; i < 4; i++) {
         c.keyx[i] = 0;
         c.keyz[i] = 0;
         c.scx[i] = 0;
         c.scz[i] = 0;
         c.stg[i] = 0;
         c.sx[i] = 0;
         c.sy[i] = 0;
         c.sz[i] = 0;
         c.smag[i] = 0.0F;
         c.fulls[i] = false;
      }
      return c;
   }

   private static final int[] ZERO_OY = new int[]{0, 0, 0};

   /** One 3-point plane whose colour channels are all equal, so Plane's ctor
    *  marks it nocol (Plane.java:132-134): the reg* HSB/Color tint branch is
    *  then unreachable and no AWT colour math is pinned. wz==0 is the
    *  scatter-eligible case (:410,:780); wz!=0 is skipped. */
   private static ContO planed(Medium m, int[] oy, int wz) {
      ContO c = car(m);
      c.npl = 1;
      c.p[0] = new Plane(m, new Trackers(), new int[]{0, 10, 0}, new int[]{0, 0, 10}, oy, 3,
            new int[]{1, 1, 1}, false, 0, 0, 0, wz, 7, 0, false);
      return c;
   }

   /** A plane whose colour channels differ, so Plane marks it !nocol
    *  (Plane.java:132-134) and the reg* tint branch is live. */
   private static ContO tinted(Medium m, int[] oc, int bfase, boolean glass, int gr) {
      ContO c = car(m);
      c.npl = 1;
      c.p[0] = new Plane(m, new Trackers(), new int[]{0, 10, 0}, new int[]{0, 0, 10}, ZERO_OY, 3,
            oc, glass, gr, 0, 0, 0, 7, bfase, false);
      return c;
   }

   /** Five empty-byte ContOs with distinct maxR, the size reset() indexes
    *  (:233,:247). */
   private static ContO[] five(Medium m) {
      ContO[] a = new ContO[5];
      for (int i = 0; i < 5; i++) {
         a[i] = car(m);
         a[i].maxR = 30 + i;
      }
      return a;
   }

   /** regy on a npl==0 fixture: useful as a count assertion. */
   private static int regyNplZeroDraws() {
      TenthsMedium cm = new TenthsMedium();
      Record r = new Record(cm);
      ContO c = car(cm);
      r.regy(0, 150.0F, false, c, new Madness(cm, r, null, 1));
      return cm.draws;
   }

   private static int bits(float f) {
      return Float.floatToIntBits(f);
   }

   /** Deterministic replay of Medium.random()'s TENTHS output, plus a draw
    *  counter. Medium.random() is not final, so the override sees exactly
    *  the calls Record makes (:125,:193,:384,:411,:754,:781).
    *
    *  WHY NOT THE SEEDED Math.random(): TestRng replaces Math's `static
    *  final` generator through reflection, which works early in a JVM but
    *  STOPS being observed once HotSpot compiles java.lang.Math.random()
    *  and constant-folds that holder field (probed on the gate's 8u40: after
    *  a warm-up, install(101) still served the PREVIOUS generator's stream).
    *  This suite is long enough to hit that point, so a seeded vector would
    *  silently depend on JIT timing. VEC is the measured seed-101
    *  Medium.random() vector (a fresh Medium, 12 calls), which keeps every
    *  literal below exactly the seeded one while removing the JVM timing
    *  dependency. The TestRng.install() scoping around each random group is
    *  kept so the intent (one seed per group) stays explicit. The literals
    *  live in tests/fixtures/toml/determinism.toml [vectors] record_tenths. */
   static class TenthsMedium extends Medium {
      private static final float[] VEC = Toml.load("tests/fixtures/toml/determinism.toml")
            .table("vectors").floatArr("record_tenths");
      int draws;

      public float random() {
         float v = VEC[draws % VEC.length];
         draws++;
         return v;
      }
   }
}
