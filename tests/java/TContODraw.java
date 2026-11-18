import java.awt.Graphics;
import java.awt.image.BufferedImage;
import java.util.ArrayList;
import java.util.List;

/**
 * Pins ContO's DRAWING half: d() (:510-643), pdust (:58-197), fixit (:843-1067),
 * electrify (:1069-1211) and lowshadow (:710-841). Everything is driven off
 * BufferedImage Graphics - offscreen and headless-safe - and only FIELDS are
 * pinned, never pixels (pixel md5s are a later batch). The grammar, copy ctor
 * and pure helpers live in TContO.
 *
 * Determinism: Medium.random() is the only entropy here. This class is far too
 * late in the suite for TestRng's reflection injection to be observable (C2
 * constant-folds Math's holder - see TestRng's header), so value pins use
 * TenthsMedium: a Medium whose random() replays the measured seed-101 tenths
 * vector and counts draws. VEC below is that vector, measured in a fresh JVM
 * (TestRng.install(101) then 48 fresh Medium.random() calls; the first 12 match
 * TRecord's shorter copy of the same measurement). The replay makes both the
 * draw COUNTS (structural) and the values (vectors) stable without JVM timing.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B6_conto.md, re-derived from ContO.java and
 * probe-measured on the gate's Oracle 8u40.
 */
public class TContODraw {
   /** constants.toml [conto] - the d() draw gate and the dust impulse boundary. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("conto");

   static void run() {
      Check.group("ContODraw");
      K.requireKeys("maxr_draw_gate", "pdust_impulse", "pdust_draws");
      // shape pin: the replay period is part of the pinned behavior
      Check.eq(TenthsMedium.VEC.length, 48, "tenths_vector_len_48");
      distAndGate();
      sparkPhase();
      dustCounters();
      dustLoop();
      dSubCalls();
      drawingErrors();
      pdust();
      drawOrder();
      fixit();
      electrify();
      lowshadow();
   }

   /** B6(28-30,34,41): d() clears dist first (:511-513), the entry gate is the
    *  xs/ys/maxR/fade/disp conjunction (:520-524), and dist is the DOUBLY
    *  truncated projection distance times grounded (:613-626). The gate needs
    *  xs(var2+maxR,var4) > 0; with x=y=z=0 that is 8*maxR-1925 > 0, i.e.
    *  maxR >= 241 - at maxR 200/240 nothing draws and dist stays 0. */
   private static void distAndGate() {
      Medium m = new Medium();
      Graphics g = graphic();

      ContO a = car(m);
      a.maxR = 200;
      a.dist = 77;
      a.d(g);
      Check.eq(a.dist, 0, "gate_maxR200_fails_dist_zeroed");

      ContO b = car(m);
      b.maxR = K.intv("maxr_draw_gate") - 1;
      b.d(g);
      Check.eq(b.dist, 0, "gate_maxR240_fails");

      ContO c = car(m);
      c.maxR = K.intv("maxr_draw_gate");
      c.d(g);
      Check.eq(c.dist, 18, "gate_maxR241_passes_dist18");

      ContO d = car(m);
      d.maxR = 300;
      d.d(g);
      Check.eq(d.dist, 18, "dist_maxR300_18");
      Check.eq(d.x + d.y + d.z, 0, "dist_does_not_move_car");

      ContO half = car(m);
      half.maxR = 300;
      half.grounded = 0.5F;
      half.d(g);
      Check.eq(half.dist, 9, "dist_grounded_half_9");

      ContO dbl = car(m);
      dbl.maxR = 300;
      dbl.grounded = 2.0F;
      dbl.d(g);
      Check.eq(dbl.dist, 36, "dist_grounded_double_36");

      // (int)sqrt((int)sqrt(175^2+100^2+150^2)) == 15
      ContO off = car(m);
      off.maxR = 300;
      off.x = 100;
      off.y = 50;
      off.z = 100;
      off.d(g);
      Check.eq(off.dist, 15, "dist_offorigin_15");
      off.grounded = 0.5F;
      off.d(g);
      Check.eq(off.dist, 7, "dist_offorigin_half_7");

      // the disp test is the on-screen width minimum
      ContO wide = car(m);
      wide.maxR = 300;
      wide.disp = 100000;
      wide.dist = 55;
      wide.d(g);
      Check.eq(wide.dist, 0, "gate_disp_too_wide_fails");

      // the right-edge test (xs(var2-maxR) < m.w) fails for a car parked far
      // to the right: xs(9700,50) is far beyond m.w==550
      ContO farRight = car(m);
      farRight.maxR = 300;
      farRight.x = 10000;
      farRight.dist = 44;
      farRight.d(g);
      Check.eq(farRight.dist, 0, "gate_off_right_edge_fails");

      // dist==0 does NOT mean "not drawn": the final stg advance only reads
      // dist, so a car exactly at the camera still advances its dust counters
      ContO drawn = car(m);
      drawn.maxR = 20;
      drawn.x = 275;
      drawn.y = 200;
      drawn.z = 0;
      drawn.stg[0] = 1;
      drawn.d(g);
      Check.eq(drawn.dist, 0, "drawn_dist_can_be_zero");
      Check.eq(drawn.stg[0], 2, "drawn_dist_zero_still_advances");

      // gate off: no dust work, no Plane.d, no addsp, zero random draws
      TenthsMedium tm = new TenthsMedium();
      ContO idle = car(tm);
      tm.nsp = 2;
      idle.d(g);
      Check.eq(idle.dist, 0, "gateoff_dist_0");
      Check.eq(tm.draws, 0, "gateoff_zero_draws");
      Check.eq(tm.nsp, 2, "gateoff_nsp_unchanged");
   }

   /** B6(31-34,64): the shadow phase. A plain shadow spawns through
    *  Medium.addsp (:559) with (x-m.x, z-m.z, (int)(maxR*0.8)) and saturates at
    *  five (Medium.java:445-452); crs (:526-529) and the low-shadow branch
    *  (:530-531) never call addsp; a matching tracker routes through s(...,0)
    *  but still spawns. Shadow never writes a ContO field. */
   private static void sparkPhase() {
      Graphics g = graphic();

      TenthsMedium tm = new TenthsMedium();
      ContO c = car(tm);
      c.maxR = 300;
      c.shadow = true;
      c.d(g);
      Check.eq(tm.nsp, 1, "spark_nsp_1");
      Check.eq(tm.spx[0], 0, "spark_spx_x_minus_mx");
      Check.eq(tm.spz[0], 0, "spark_spz_z_minus_mz");
      Check.eq(tm.sprad[0], 240, "spark_sprad_int_maxR_0_8");
      Check.eq(tm.draws, 0, "spark_zero_draws");
      // B6(64): shadow only writes m.nsp/spx/spz/sprad
      Check.eq(c.stg[0], 0, "spark_leaves_conto_dust_alone");
      Check.eq(c.npl, 0, "spark_leaves_conto_npl");
      Check.eq(c.x + c.y + c.z, 0, "spark_leaves_conto_position");
      Check.eq(c.dist, 18, "spark_dist_still_projected");
      for (int i = 0; i < 5; i++) {
         c.d(g);
      }
      Check.eq(tm.nsp, 5, "spark_saturates_at_5");
      Check.eq(tm.sprad[4], 240, "spark_slot4_sprad");

      // the spark offsets are car-minus-medium, not absolute: m.x=100/m.z=50
      TenthsMedium ox = new TenthsMedium();
      ContO offCar = car(ox);
      offCar.maxR = 300;
      offCar.shadow = true;
      offCar.x = 300;
      offCar.z = 200;
      ox.x = 100;
      ox.z = 50;
      offCar.d(g);
      Check.eq(ox.nsp, 1, "spark_offset_nsp_1");
      Check.eq(ox.spx[0], 200, "spark_offset_spx_x_minus_mx");
      Check.eq(ox.spz[0], 150, "spark_offset_spz_z_minus_mz");

      TenthsMedium cm = new TenthsMedium();
      ContO crs = plane(cm);
      crs.maxR = 300;
      crs.shadow = true;
      cm.crs = true;
      crs.d(g);
      Check.eq(cm.nsp, 0, "crs_no_addsp");
      Check.eq(cm.draws, 0, "crs_zero_draws");

      TenthsMedium lm = new TenthsMedium();
      ContO low = car(lm);
      low.maxR = 300;
      low.shadow = true;
      low.y = 2300;
      lm.zy = 90;
      low.d(g);
      Check.eq(lm.nsp, 0, "lowshadow_branch_no_addsp");
      Check.eq(low.dist, 46, "lowshadow_branch_dist_46");
      Check.eq(lm.draws, 0, "lowshadow_branch_zero_draws");

      // a tracker under the car sends the planes through s(...,0)... and the
      // addsp call still happens afterwards (:559)
      TenthsMedium tt = new TenthsMedium();
      Trackers tk = new Trackers();
      tk.nt = 1;
      tk.radx[0] = 100;
      tk.radz[0] = 100;
      ContO trk = new ContO(new byte[0], tt, tk);
      trk.maxR = 300;
      trk.shadow = true;
      trk.npl = 1;
      trk.p[0] = new Plane(tt, tk, new int[]{0, 10, 0}, new int[]{0, 0, 10}, new int[]{0, 0, 0}, 3,
            new int[]{10, 20, 30}, false, 0, 0, 0, 0, 7, 0, false);
      trk.d(g);
      Check.eq(tt.nsp, 1, "tracker_shadow_still_spawns");

      // no tracker: the ground-projected s(...,1) arm runs (:550-556)
      TenthsMedium gt = new TenthsMedium();
      ContO ground = plane(gt);
      ground.maxR = 300;
      ground.shadow = true;
      ground.d(g);
      Check.eq(gt.nsp, 1, "ground_shadow_arm_spawns");

      // shadow == false writes nothing at all
      TenthsMedium off = new TenthsMedium();
      ContO plain = car(off);
      plain.maxR = 300;
      plain.d(g);
      Check.eq(off.nsp, 0, "noshadow_no_spark");
      Check.eq(off.draws, 0, "noshadow_zero_draws");
   }

   /** B6(35): when the entry gate fails, dist stays 0 and the trailing loop
    *  (:630-642) advances every live dust slot: 1->2, 2->3, 3->4, 4->0. */
   private static void dustCounters() {
      Graphics g = graphic();
      ContO c = car(new Medium());
      c.maxR = 0;
      c.stg[0] = 1;
      c.stg[1] = 4;
      c.stg[2] = 0;
      c.stg[3] = 2;
      c.d(g);
      Check.eq(c.dist, 0, "counters_gateoff_dist_0");
      Check.eq(c.stg[0], 2, "counters_stg0_1_to_2");
      Check.eq(c.stg[1], 0, "counters_stg1_4_to_0");
      Check.eq(c.stg[2], 0, "counters_stg2_0_stays");
      Check.eq(c.stg[3], 3, "counters_stg3_2_to_3");
   }

   /** B6(36,37,43,44): the gate path runs pdust(slot,g,-1) for live slots
    *  (:576-580) then, per drawn plane, pdust(master-1,g,+1) (:606-607). The
    *  -1 pass needs dov<0 (or dov==0, which merely promotes dov to 1), so a
    *  fulls particle started with dov==0 neither draws nor zeroes this frame. */
   private static void dustLoop() {
      Graphics g = graphic();

      TenthsMedium tm = new TenthsMedium();
      ContO c = car(tm);
      c.maxR = 300;
      c.stg[0] = 1;
      c.dov[0] = -1;
      c.smag[0] = 1.0F;
      c.sy[0] = 300;
      c.d(g);
      Check.eq(c.stg[0], 2, "loop_stg_2");
      Check.eq(c.dov[0], -1, "loop_dov_stays_minus1");
      Check.eq(c.sy[0], 220, "loop_sy_clamped_250_minus_30");
      Check.eq(c.sx[0], 0, "loop_sx_0");
      Check.eq(tm.draws, 16, "loop_pdust_draws_16");
      Check.eq(c.dist, 18, "loop_dist_18");

      TenthsMedium fm = new TenthsMedium();
      ContO fulls = car(fm);
      fulls.maxR = 300;
      fulls.stg[0] = 1;
      fulls.dov[0] = -1;
      fulls.fulls[0] = true;
      fulls.smag[0] = 1.0F;
      fulls.d(g);
      Check.eq(fulls.stg[0], 2, "fulls_stg_2");
      Check.eq(fulls.dov[0], 0, "fulls_reaching_stg2_zeroes_dov");
      Check.eq(fm.draws, 16, "fulls_draws_16");

      // master==1 points back at slot 0; the -1 pass with dov==0 only promotes
      // dov, so the only 16 draws come from the +1 master call
      TenthsMedium ma = new TenthsMedium();
      ContO m1 = plane(ma);
      m1.maxR = 300;
      m1.p[0].master = 1;
      m1.stg[0] = 1;
      m1.dov[0] = 0;
      m1.d(g);
      Check.eq(m1.stg[0], 2, "master1_stg_2");
      Check.eq(m1.dov[0], -1, "master1_dov_flipped_to_minus1");
      Check.eq(ma.draws, 16, "master1_draws_16");

      TenthsMedium mb = new TenthsMedium();
      ContO m0 = plane(mb);
      m0.maxR = 300;
      m0.stg[0] = 1;
      m0.dov[0] = 0;
      m0.d(g);
      Check.eq(m0.stg[0], 1, "master0_stg_stays_1_no_draw");
      Check.eq(m0.dov[0], 1, "master0_dov_promoted_1");
      Check.eq(mb.draws, 0, "master0_no_draws");

      TenthsMedium mc = new TenthsMedium();
      ContO m3 = plane(mc);
      m3.maxR = 300;
      m3.p[0].master = 3;
      m3.stg[2] = 1;
      m3.dov[2] = 0;
      m3.d(g);
      Check.eq(m3.stg[2], 2, "master3_slot2_stg_2");
      Check.eq(m3.dov[2], -1, "master3_slot2_dov_minus1");
      Check.eq(m3.stg[0], 0, "master3_slot0_untouched");
      Check.eq(mc.draws, 16, "master3_draws_16");

      // a master pointing at a dead slot is skipped entirely (:606)
      TenthsMedium md = new TenthsMedium();
      ContO dead = plane(md);
      dead.maxR = 300;
      dead.p[0].master = 2;
      dead.stg[1] = 0;
      dead.dov[1] = 0;
      dead.d(g);
      Check.eq(dead.dov[1], 0, "master_dead_slot_dov_untouched");
      Check.eq(md.draws, 0, "master_dead_slot_no_draws");
   }

   /** B6(53,64): inside the ys gate at :564 d() calls electrify (:565-567)
    *  first, then fixit (:569-571), before the dust loop and the av sort. Both
    *  are random consumers: 30 for a fresh electrify and 32 for a fixit at
    *  fcnt==1. */
   private static void dSubCalls() {
      Graphics g = graphic();
      TenthsMedium tm = new TenthsMedium();
      ContO c = car(tm);
      c.maxR = 300;
      c.elec = true;
      c.fix = true;
      c.fcnt = 1;
      c.d(g);
      Check.eq(c.elc[0], 2, "dsub_elec_seeded_elc2");
      Check.eq(c.xy, 11, "dsub_elec_xy_11");
      Check.eq(c.zy, 0, "dsub_elec_zy_0");
      Check.eq(c.fcnt, 2, "dsub_fix_fcnt_advanced_2");
      Check.eq(tm.draws, 62, "dsub_30_plus_32_draws_62");
      Check.eq(c.dist, 18, "dsub_dist_18");

      // without the flags neither sub-call runs
      TenthsMedium off = new TenthsMedium();
      ContO c2 = car(off);
      c2.maxR = 300;
      c2.fcnt = 1;
      c2.d(g);
      Check.eq(off.draws, 0, "dsub_flags_off_zero_draws");
      Check.eq(c2.fcnt, 1, "dsub_flags_off_despite_fcnt");
   }

   /** B6(38-40): d()'s failure modes are NOT caught. m.fade is int[8]
    *  (Medium.java:8) so disline 8 or -1 throws AIOOBE at :523; a null plane
    *  throws NPE in the av sort (:588); npl beyond p.length throws AIOOBE at
    *  p[120] (:586-588) - but only when the earlier slots are non-null. */
   private static void drawingErrors() {
      final Graphics g = graphic();
      for (final int dl : new int[]{8, -1}) {
         Check.throwsIt(new Check.Task() {
            public void run() {
               ContO c = car(new Medium());
               c.maxR = 300;
               c.disline = dl;
               c.d(g);
            }
         }, ArrayIndexOutOfBoundsException.class, "err_disline" + (dl < 0 ? "neg" : dl) + "_aioobe");
      }
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO c = car(new Medium());
            c.maxR = 300;
            c.npl = 1;
            c.d(g);
         }
      }, NullPointerException.class, "err_null_plane_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO c = car(new Medium());
            c.maxR = 300;
            Plane shared = new Plane(c.m, c.t, new int[]{0, 10, 0}, new int[]{0, 0, 10}, new int[]{0, 0, 0},
                  3, new int[]{1, 2, 3}, false, 0, 0, 0, 0, 7, 0, false);
            for (int i = 0; i < 120; i++) {
               c.p[i] = shared;
            }
            c.npl = 121;
            c.d(g);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_npl121_aioobe");
   }

   /** B6(43-47): pdust's own contract. The guard is `var3*dov > 0` (:59), so a
    *  matching sign draws 16 random values (:124-143) and advances stg
    *  (:186-193). dov==0 is NOT a no-op for the -1 call - the else-if at
    *  :194-196 promotes it to 1. sy clamps to 250 then falls by
    *  23+7*smag (<=150) or 3+27*smag (>150); sx/sz divide by (stg+1)*smag. */
   private static void pdust() {
      Graphics g = graphic();

      TenthsMedium t1 = new TenthsMedium();
      ContO c1 = car(t1);
      c1.stg[0] = 1;
      c1.dov[0] = -1;
      c1.smag[0] = 1.0F;
      c1.sy[0] = 100;
      c1.pdust(0, g, -1);
      Check.eq(c1.stg[0], 2, "pdust_stg_2");
      Check.eq(c1.sy[0], 70, "pdust_sy_100_minus_30");
      Check.eq(t1.draws, 16, "pdust_draws_16");

      // mismatched signs are inert
      TenthsMedium t2 = new TenthsMedium();
      ContO c2 = car(t2);
      c2.stg[0] = 1;
      c2.dov[0] = 1;
      c2.sy[0] = 200;
      c2.pdust(0, g, -1);
      Check.eq(c2.stg[0], 1, "pdust_minus1_with_dov_pos_inert");
      Check.eq(c2.dov[0], 1, "pdust_minus1_with_dov_pos_keeps_dov");
      Check.eq(c2.sy[0], 200, "pdust_minus1_with_dov_pos_keeps_sy");
      Check.eq(t2.draws, 0, "pdust_minus1_with_dov_pos_zero_draws");

      TenthsMedium t3 = new TenthsMedium();
      ContO c3 = car(t3);
      c3.stg[0] = 1;
      c3.dov[0] = -1;
      c3.sy[0] = 200;
      c3.pdust(0, g, 1);
      Check.eq(c3.stg[0], 1, "pdust_plus1_with_dov_neg_inert");
      Check.eq(t3.draws, 0, "pdust_plus1_with_dov_neg_zero_draws");

      // dov==0 promotes to 1 for EITHER sign argument (the else-if at :194)
      TenthsMedium t4 = new TenthsMedium();
      ContO c4 = car(t4);
      c4.stg[0] = 1;
      c4.sy[0] = 200;
      c4.pdust(0, g, -1);
      Check.eq(c4.dov[0], 1, "pdust_minus1_dov0_promotes_1");
      Check.eq(c4.stg[0], 1, "pdust_minus1_dov0_no_draw");
      Check.eq(t4.draws, 0, "pdust_minus1_dov0_zero_draws");

      TenthsMedium t5 = new TenthsMedium();
      ContO c5 = car(t5);
      c5.stg[0] = 1;
      c5.pdust(0, g, 1);
      Check.eq(c5.dov[0], 1, "pdust_plus1_dov0_promotes_1");
      Check.eq(t5.draws, 0, "pdust_plus1_dov0_zero_draws");

      // full phase cycle 1->2->3->4->0, one 30-unit fall per pass
      TenthsMedium t6 = new TenthsMedium();
      ContO c6 = car(t6);
      c6.dov[0] = -1;
      c6.smag[0] = 1.0F;
      c6.sy[0] = 100;
      int[] syWant = {70, 40, 10, -20, -50};
      for (int i = 0; i < 5; i++) {
         c6.pdust(0, g, -1);
         Check.eq(c6.stg[0], i == 4 ? 0 : i + 1, "pdust_cycle_stg" + i);
         Check.eq(c6.sy[0], syWant[i], "pdust_cycle_sy" + i);
      }
      Check.eq(t6.draws, 80, "pdust_cycle_draws_80");

      // the >150 arm: 3+27*smag instead of 23+7*smag, and the drift divides
      TenthsMedium t7 = new TenthsMedium();
      ContO c7 = car(t7);
      c7.stg[0] = 1;
      c7.dov[0] = -1;
      c7.smag[0] = 2.0F;
      c7.sy[0] = 300;
      c7.scx[0] = K.intv("pdust_impulse") + 51;
      c7.pdust(0, g, -1);
      Check.eq(c7.sy[0], 193, "pdust_sy151_250_minus_57");
      Check.eq(c7.sx[0], 37, "pdust_sx_drift_37");

      // float->int cast pins: x/0.0f is +/-Infinity (MIN/MAX_VALUE) and 0/0 is NaN (0)
      TenthsMedium t8 = new TenthsMedium();
      ContO c8 = car(t8);
      c8.stg[0] = 1;
      c8.dov[0] = -1;
      c8.smag[0] = 0.0F;
      c8.sx[0] = 999;
      c8.sz[0] = 888;
      c8.pdust(0, g, -1);
      Check.eq(c8.sx[0], 0, "pdust_smag0_scx0_nan_to_0");
      Check.eq(c8.sz[0], 0, "pdust_smag0_scz0_nan_to_0");

      TenthsMedium t9 = new TenthsMedium();
      ContO c9 = car(t9);
      c9.stg[0] = 1;
      c9.dov[0] = -1;
      c9.smag[0] = 0.0F;
      c9.scx[0] = K.intv("pdust_impulse");
      c9.pdust(0, g, -1);
      Check.eq(c9.sx[0], Integer.MAX_VALUE, "pdust_smag0_scx100_inf_to_max");

      TenthsMedium t10 = new TenthsMedium();
      ContO c10 = car(t10);
      c10.stg[0] = 1;
      c10.dov[0] = -1;
      c10.smag[0] = 0.0F;
      c10.scx[0] = -K.intv("pdust_impulse");
      c10.pdust(0, g, -1);
      Check.eq(c10.sx[0], Integer.MIN_VALUE, "pdust_smag0_scxneg100_inf_to_min");

      // the clamp at :86-88 happens before the fall
      TenthsMedium t11 = new TenthsMedium();
      ContO c11 = car(t11);
      c11.stg[0] = 1;
      c11.dov[0] = -1;
      c11.smag[0] = 1.0F;
      c11.sy[0] = 300;
      c11.pdust(0, g, -1);
      Check.eq(c11.sy[0], 220, "pdust_sy300_clamps_250_then_falls");

      // a tracker over the spark recolours the polygon (:71-83) and, when it
      // is not skidded, feeds var4 = stg^3+2 (:76-78); no ContO field changes
      TenthsMedium t12 = new TenthsMedium();
      Trackers tk = new Trackers();
      tk.nt = 1;
      tk.radx[0] = 100;
      tk.radz[0] = 100;
      tk.c[0][0] = 10;
      tk.c[0][1] = 20;
      tk.c[0][2] = 30;
      ContO c12 = new ContO(new byte[0], t12, tk);
      c12.stg[0] = 1;
      c12.dov[0] = -1;
      c12.smag[0] = 1.0F;
      c12.sy[0] = 100;
      c12.pdust(0, g, -1);
      Check.eq(c12.stg[0], 2, "pdust_tracker_stg_2");
      Check.eq(c12.sy[0], 70, "pdust_tracker_sy_70");
      Check.eq(t12.draws, 16, "pdust_tracker_draws_16");

      // skidded tracker: the var4 arm at :76-78 is skipped
      TenthsMedium t13 = new TenthsMedium();
      Trackers tk2 = new Trackers();
      tk2.nt = 1;
      tk2.radx[0] = 100;
      tk2.radz[0] = 100;
      tk2.skd[0] = 1;
      ContO c13 = new ContO(new byte[0], t13, tk2);
      c13.stg[0] = 1;
      c13.dov[0] = -1;
      c13.smag[0] = 1.0F;
      c13.sy[0] = 100;
      c13.pdust(0, g, -1);
      Check.eq(c13.stg[0], 2, "pdust_skid_stg_2");
      Check.eq(t13.draws, 16, "pdust_skid_draws_16");

      // a far spark makes the projected depth exceed fade[0..2] (:105-111);
      // still only colour maths, so the counters are unchanged
      TenthsMedium t14 = new TenthsMedium();
      ContO c14 = car(t14);
      c14.stg[0] = 1;
      c14.dov[0] = -1;
      c14.smag[0] = 1.0F;
      c14.sx[0] = 5000;
      c14.sy[0] = 100;
      c14.sz[0] = 5000;
      c14.pdust(0, g, -1);
      Check.eq(c14.stg[0], 2, "pdust_far_stg_2");
      Check.eq(c14.sx[0], 5000, "pdust_far_sx_5000");
      Check.eq(c14.sy[0], 70, "pdust_far_sy_70");

      // a spark far BELOW the viewport makes the projected vertex height
      // negative (<45), which resets m.flex to 0 (:126-128 and :168-170)
      TenthsMedium t15 = new TenthsMedium();
      ContO c15 = car(t15);
      c15.stg[0] = 1;
      c15.dov[0] = -1;
      c15.smag[0] = 1.0F;
      c15.sy[0] = -5000;
      t15.flex = 5;
      c15.pdust(0, g, -1);
      Check.eq(c15.stg[0], 2, "pdust_flex_stg_2");
      Check.eq(t15.flex, 0, "pdust_flex_reset_0");
      Check.eq(t15.draws, 16, "pdust_flex_draws_16");
   }

   /** B6(42): the av sort. Ranks count planes with a LARGER av, so the draw
    *  order is DESCENDING av (:586-600); on an exact tie the higher index gets
    *  the higher rank and is drawn later. RecordingPlane is a Plane subclass
    *  that just records the call, so no pixels are involved. */
   private static void drawOrder() {
      Graphics g = graphic();

      List<String> one = new ArrayList<String>();
      ContO c1 = car(new Medium());
      c1.maxR = 300;
      c1.npl = 3;
      c1.p[0] = new RecordingPlane(c1.m, "A", 5, one);
      c1.p[1] = new RecordingPlane(c1.m, "B", 1, one);
      c1.p[2] = new RecordingPlane(c1.m, "C", 1, one);
      c1.d(g);
      Check.eq(one.toString(), "[A, B, C]", "order_av_5_1_1_descending");
      Check.eq(c1.dist, 18, "order_gate_passed");

      List<String> two = new ArrayList<String>();
      ContO c2 = car(new Medium());
      c2.maxR = 300;
      c2.npl = 3;
      c2.p[0] = new RecordingPlane(c2.m, "A", 1, two);
      c2.p[1] = new RecordingPlane(c2.m, "B", 5, two);
      c2.p[2] = new RecordingPlane(c2.m, "C", 3, two);
      c2.d(g);
      Check.eq(two.toString(), "[B, C, A]", "order_av_1_5_3_descending");

      List<String> tie = new ArrayList<String>();
      ContO c3 = car(new Medium());
      c3.maxR = 300;
      c3.npl = 3;
      c3.p[0] = new RecordingPlane(c3.m, "A", 2, tie);
      c3.p[1] = new RecordingPlane(c3.m, "B", 2, tie);
      c3.p[2] = new RecordingPlane(c3.m, "C", 2, tie);
      c3.d(g);
      Check.eq(tie.toString(), "[A, B, C]", "order_tie_ascending_index");

      // a single plane still draws exactly once
      List<String> solo = new ArrayList<String>();
      ContO c4 = car(new Medium());
      c4.maxR = 300;
      c4.npl = 1;
      c4.p[0] = new RecordingPlane(c4.m, "Z", 0, solo);
      c4.d(g);
      Check.eq(solo.toString(), "[Z]", "order_single_plane");
   }

   /** B6(50-52): fixit's counter machine. fcnt advances every call (:1061-1066;
    *  8 resets to 0 and clears fix); the drawing block runs for fcnt==1 or
    *  fcnt>2 (:894) and costs exactly 32 random draws (:949-964,:1014-1029).
    *  fcnt==1 rewrites every plane's hsb to Color.RGBtoHSB of
    *  getHSBColor(0.57,0.8,0.8) after the snap blend, and sets flx=1 (:844-880). */
   private static void fixit() {
      Graphics g = graphic();

      TenthsMedium tm = new TenthsMedium();
      ContO c = plane(tm);
      c.fix = true;
      int prev = 0;
      for (int i = 1; i <= 8; i++) {
         c.fixit(g);
         Check.eq(c.fcnt, i, "fixit_cycle_fcnt" + i);
         prev = tm.draws;
      }
      Check.eq(prev, 192, "fixit_cycle_total_draws_192");
      Check.ok(c.fix, "fixit_cycle_fix_still_true");

      int before = tm.draws;
      c.fixit(g);
      Check.eq(c.fcnt, 0, "fixit_ninth_fcnt_0");
      Check.ok(!c.fix, "fixit_ninth_clears_fix");
      Check.eq(tm.draws - before, 32, "fixit_ninth_draws_32");

      // entry fcnt==1: flx + hsb, 32 draws
      TenthsMedium t1 = new TenthsMedium();
      ContO f1 = plane(t1);
      f1.fcnt = 1;
      f1.fixit(g);
      Check.eq(f1.p[0].flx, 1, "fixit_entry1_flx_1");
      Check.eq(bits(f1.p[0].hsb[0]), 1058148276, "fixit_entry1_hsb0_bits");
      Check.eq(bits(f1.p[0].hsb[1]), 1061981325, "fixit_entry1_hsb1_bits");
      Check.eq(bits(f1.p[0].hsb[2]), 1061997773, "fixit_entry1_hsb2_bits");
      Check.eq(t1.draws, 32, "fixit_entry1_draws_32");
      Check.eq(f1.fcnt, 2, "fixit_entry1_advances_to_2");

      // entry fcnt==2: flx only, and the drawing block is skipped (2 is not > 2)
      TenthsMedium t2 = new TenthsMedium();
      ContO f2 = plane(t2);
      int hsb0 = bits(f2.p[0].hsb[0]);
      int hsb1 = bits(f2.p[0].hsb[1]);
      int hsb2 = bits(f2.p[0].hsb[2]);
      f2.fcnt = 2;
      f2.fixit(g);
      Check.eq(f2.p[0].flx, 1, "fixit_entry2_flx_1");
      // fcnt==2 leaves the Plane ctor's hsb (from c={10,20,30}) alone
      Check.eq(hsb0, 1058362709, "fixit_entry2_hsb0_plane_ctor");
      Check.eq(bits(f2.p[0].hsb[0]), hsb0, "fixit_entry2_hsb0_unchanged");
      Check.eq(bits(f2.p[0].hsb[1]), hsb1, "fixit_entry2_hsb1_unchanged");
      Check.eq(bits(f2.p[0].hsb[2]), hsb2, "fixit_entry2_hsb2_unchanged");
      Check.eq(t2.draws, 0, "fixit_entry2_draws_0");
      Check.eq(f2.fcnt, 3, "fixit_entry2_advances_to_3");

      // entry fcnt==4: flx=3, drawing still runs
      TenthsMedium t4 = new TenthsMedium();
      ContO f4 = plane(t4);
      f4.fcnt = 4;
      f4.fixit(g);
      Check.eq(f4.p[0].flx, 3, "fixit_entry4_flx_3");
      Check.eq(t4.draws, 32, "fixit_entry4_draws_32");

      // entry fcnt==7 draws but writes no plane flag
      TenthsMedium t7 = new TenthsMedium();
      ContO f7 = plane(t7);
      f7.fcnt = 7;
      f7.fixit(g);
      Check.eq(f7.p[0].flx, 0, "fixit_entry7_flx_untouched");
      Check.eq(t7.draws, 32, "fixit_entry7_draws_32");
      Check.eq(f7.fcnt, 8, "fixit_entry7_advances_to_8");

      // entry fcnt==8: draws once more, then resets
      TenthsMedium t8 = new TenthsMedium();
      ContO f8 = plane(t8);
      f8.fcnt = 8;
      f8.fixit(g);
      Check.eq(t8.draws, 32, "fixit_entry8_draws_32");
      Check.eq(f8.fcnt, 0, "fixit_entry8_resets_0");
      Check.ok(!f8.fix, "fixit_entry8_clears_fix");

      // entry fcnt==0: counter only
      TenthsMedium t0 = new TenthsMedium();
      ContO f0 = plane(t0);
      f0.fixit(g);
      Check.eq(t0.draws, 0, "fixit_entry0_draws_0");
      Check.eq(f0.fcnt, 1, "fixit_entry0_advances_to_1");

      // npl==0: no throw, counter and draws still happen
      TenthsMedium tn = new TenthsMedium();
      ContO fn = car(tn);
      fn.fcnt = 1;
      fn.fixit(g);
      Check.eq(tn.draws, 32, "fixit_npl0_draws_32");
      Check.eq(fn.fcnt, 2, "fixit_npl0_advances_to_2");

      // snap is applied to the getHSBColor result before RGBtoHSB
      TenthsMedium ts = new TenthsMedium();
      ContO fs = plane(ts);
      fs.m.snap[0] = 50;
      fs.m.snap[1] = 50;
      fs.m.snap[2] = 50;
      fs.fcnt = 1;
      fs.fixit(g);
      Check.eq(bits(fs.p[0].hsb[0]), 1057728519, "fixit_snap50_hsb0_bits");
      Check.eq(bits(fs.p[0].hsb[1]), 1061339843, "fixit_snap50_hsb1_bits");
      Check.eq(bits(fs.p[0].hsb[2]), 1065353216, "fixit_snap50_hsb2_bits");

      // distinct wheel points exercise the py/max comparisons at :920-941
      TenthsMedium tk = new TenthsMedium();
      ContO fk = plane(tk);
      fk.keyx = new int[]{100, 0, -100, 0};
      fk.keyz = new int[]{0, 100, 0, -100};
      fk.grat = 50;
      fk.fcnt = 1;
      fk.fixit(g);
      Check.eq(fk.p[0].flx, 1, "fixit_keypoints_flx_1");
      Check.eq(tk.draws, 32, "fixit_keypoints_draws_32");
      Check.eq(fk.fcnt, 2, "fixit_keypoints_fcnt_2");

      // snap extremes drive the >255 and <0 colour clamps at :850-875
      TenthsMedium thigh = new TenthsMedium();
      ContO fhigh = plane(thigh);
      fhigh.m.snap[0] = 200;
      fhigh.m.snap[1] = 200;
      fhigh.m.snap[2] = 200;
      fhigh.fcnt = 1;
      fhigh.fixit(g);
      Check.eq(fhigh.p[0].flx, 1, "fixit_snap200_flx_1");
      Check.eq(thigh.draws, 32, "fixit_snap200_draws_32");
      TenthsMedium tlow = new TenthsMedium();
      ContO flow = plane(tlow);
      flow.m.snap[0] = -200;
      flow.m.snap[1] = -200;
      flow.m.snap[2] = -200;
      flow.fcnt = 1;
      flow.fixit(g);
      Check.eq(flow.p[0].flx, 1, "fixit_snapneg200_flx_1");
      Check.eq(tlow.draws, 32, "fixit_snapneg200_draws_32");
   }

   /** B6(53-55): electrify. Placement is unconditional: !roted -> xy+=11,
    *  roted -> zy+=11 (:1206-1210). Each arc seeds edl/edr=(int)(380-random*760)
    *  only when its elc==0 (:1073-1077), then flips elc to 0 when
    *  elc > random*7, else increments (:1083-1087). An arc costs 15 draws when
    *  it seeds (2 seed + 4 state + 1 flip + 8 vertex) and 13 otherwise. */
   private static void electrify() {
      Graphics g = graphic();

      TenthsMedium tm = new TenthsMedium();
      ContO c = car(tm);
      c.electrify(g);
      Check.eq(c.edl[0], 76, "elc_edl0_76");
      Check.eq(c.edl[1], -304, "elc_edl1_minus304");
      Check.eq(c.edr[0], 380, "elc_edr0_380");
      Check.eq(c.edr[1], 0, "elc_edr1_0");
      Check.eq(c.elc[0], 2, "elc_elc0_2");
      Check.eq(c.elc[1], 2, "elc_elc1_2");
      Check.eq(c.xy, 11, "elc_xy_plus11");
      Check.eq(c.zy, 0, "elc_zy_untouched");
      Check.eq(tm.draws, 30, "elc_first_call_draws_30");

      int before = tm.draws;
      c.electrify(g);
      Check.eq(c.edl[0], 76, "elc_second_call_keeps_edl0");
      Check.eq(c.edr[0], 380, "elc_second_call_keeps_edr0");
      Check.eq(c.elc[0], 3, "elc_second_call_elc0_3");
      Check.eq(c.elc[1], 3, "elc_second_call_elc1_3");
      Check.eq(c.xy, 22, "elc_second_call_xy_plus11_again");
      Check.eq(tm.draws - before, 26, "elc_second_call_draws_26");

      TenthsMedium tr = new TenthsMedium();
      ContO rot = car(tr);
      rot.roted = true;
      rot.electrify(g);
      Check.eq(rot.xy, 0, "roted_xy_untouched");
      Check.eq(rot.zy, 11, "roted_zy_plus11");
      Check.eq(tr.draws, 30, "roted_draws_30");

      // flip rule with a fixed draw stream: the check reads the value sampled
      // AFTER 4 state draws, which is VEC[4]==0.3 -> threshold 2.1
      TenthsMedium fp = new TenthsMedium();
      ContO flipUp = car(fp);
      flipUp.elc[0] = 2;
      flipUp.elc[1] = 5;
      flipUp.electrify(g);
      Check.eq(flipUp.elc[0], 3, "elc_flip_2_gt_2_1_false_increments_3");
      Check.eq(flipUp.elc[1], 6, "elc_flip_5_gt_6_3_false_increments_6");
      Check.eq(fp.draws, 26, "elc_flip_preseeded_draws_26");

      TenthsMedium fp2 = new TenthsMedium();
      ContO flipDown = car(fp2);
      flipDown.elc[0] = 3;
      flipDown.elc[1] = 4;
      flipDown.electrify(g);
      Check.eq(flipDown.elc[0], 0, "elc_flip_3_gt_2_1_true_resets_0");
      Check.eq(flipDown.elc[1], 5, "elc_flip_4_gt_6_3_false_increments_5");
      Check.eq(fp2.draws, 26, "elc_flip2_draws_26");

      // a live arc is never re-seeded, so its edl/edr survive
      FixedMedium half = new FixedMedium(0.5F);
      ContO live = car(half);
      live.elc[0] = 1;
      live.edl[0] = 111;
      live.edr[0] = 222;
      live.elc[1] = 1;
      live.electrify(g);
      Check.eq(live.edl[0], 111, "elc_live_edl_preserved");
      Check.eq(live.edr[0], 222, "elc_live_edr_preserved");
      Check.eq(live.elc[0], 2, "elc_live_elc0_2");
      Check.eq(live.elc[1], 2, "elc_live_elc1_2");

      // formula endpoint: random()==0 -> 380-0 == 380; the flip then sees
      // elc(1) > 0*7 and resets to 0
      FixedMedium zero = new FixedMedium(0.0F);
      ContO endp = car(zero);
      endp.electrify(g);
      Check.eq(endp.edl[0], 380, "elc_zero_draw_edl_380");
      Check.eq(endp.edr[0], 380, "elc_zero_draw_edr_380");
      Check.eq(endp.elc[0], 0, "elc_zero_draw_flip_to_0");
      Check.eq(endp.elc[1], 0, "elc_zero_draw_flip_to_0_arc2");
      Check.eq(zero.draws, 30, "elc_zero_draws_30");

      // 0.5 -> 380-380 == 0, flip threshold 3.5 -> increments
      FixedMedium mid = new FixedMedium(0.5F);
      ContO midc = car(mid);
      midc.electrify(g);
      Check.eq(midc.edl[0], 0, "elc_half_draw_edl_0");
      Check.eq(midc.edr[0], 0, "elc_half_draw_edr_0");
      Check.eq(midc.elc[0], 2, "elc_half_draw_increments_2");
      Check.eq(midc.elc[1], 2, "elc_half_draw_increments_2_arc2");

      // m.trk overrides the arc colour (:1173-1177) and the clamp branches at
      // :1155-1171 / :1182-1198 run under snap extremes
      TenthsMedium ttrk = new TenthsMedium();
      ContO trk = car(ttrk);
      trk.m.trk = true;
      trk.m.snap[0] = 200;
      trk.m.snap[1] = 200;
      trk.m.snap[2] = 200;
      trk.electrify(g);
      Check.eq(trk.edl[0], 76, "elc_trk_edl_76");
      Check.eq(ttrk.draws, 30, "elc_trk_draws_30");
      TenthsMedium tlow = new TenthsMedium();
      ContO low = car(tlow);
      low.m.snap[0] = -200;
      low.m.snap[1] = -200;
      low.m.snap[2] = -200;
      low.electrify(g);
      Check.eq(low.edl[1], -304, "elc_snapneg_edl1_minus304");
      Check.eq(tlow.draws, 30, "elc_snapneg_draws_30");
   }

   /** B6(63): lowshadow is Graphics-only - it writes no ContO/Medium field and
    *  consumes no random values (planes/shadows are pure locals). */
   private static void lowshadow() {
      TenthsMedium tm = new TenthsMedium();
      ContO c = plane(tm);
      c.x = 10;
      c.y = 20;
      c.z = 30;
      c.keyx[0] = 1;
      c.keyz[0] = 2;
      c.p[0].hsb[0] = 0.25F;
      int c0 = c.p[0].c[0];
      int mNsp = tm.nsp;
      c.lowshadow(graphic(), 500);
      Check.eq(tm.draws, 0, "lowshadow_zero_draws");
      Check.eq(c.x, 10, "lowshadow_x_untouched");
      Check.eq(c.y, 20, "lowshadow_y_untouched");
      Check.eq(c.z, 30, "lowshadow_z_untouched");
      Check.eq(c.npl, 1, "lowshadow_npl_untouched");
      Check.eq(c.stg[0], 0, "lowshadow_stg_untouched");
      Check.eq(bits(c.smag[0]), bits(0.0F), "lowshadow_smag_untouched");
      Check.eq(bits(c.p[0].hsb[0]), bits(0.25F), "lowshadow_hsb_untouched");
      Check.eq(c.p[0].c[0], c0, "lowshadow_c_untouched");
      Check.eq(c.keyx[0], 1, "lowshadow_keyx_untouched");
      Check.eq(tm.nsp, mNsp, "lowshadow_nsp_untouched");

      // with a live tracker the polygon color comes from t.c (:787-789) but
      // still nothing on ContO moves
      TenthsMedium tt = new TenthsMedium();
      Trackers tk = new Trackers();
      tk.nt = 1;
      ContO tc = new ContO(new byte[0], tt, tk);
      tc.npl = 0;
      tc.lowshadow(graphic(), 100);
      Check.eq(tt.draws, 0, "lowshadow_tracker_zero_draws");
      Check.eq(tk.nt, 1, "lowshadow_tracker_nt_untouched");

      // a tracker whose x/z lands under all four key points takes the
      // recolour arm (:744-792); zy>270 folds and |zy|>90 flips the side
      TenthsMedium tc2 = new TenthsMedium();
      Trackers tk2 = new Trackers();
      tk2.nt = 1;
      tk2.radx[0] = 100;
      tk2.radz[0] = 100;
      tk2.y[0] = 120;
      tk2.c[0][0] = 40;
      tk2.c[0][1] = 50;
      tk2.c[0][2] = 60;
      ContO lc = new ContO(new byte[0], tc2, tk2);
      lc.npl = 0;
      lc.zy = 180;
      lc.lowshadow(graphic(), 3000);
      Check.eq(tc2.draws, 0, "lowshadow_tracker_match_zero_draws");
      Check.eq(lc.zy, 180, "lowshadow_tracker_match_zy_untouched");
      Check.eq(tk2.nt, 1, "lowshadow_tracker_match_nt_untouched");
      lc.zy = 300;
      lc.lowshadow(graphic(), 3000);
      Check.eq(tc2.draws, 0, "lowshadow_zy300_zero_draws");
      Check.eq(lc.zy, 300, "lowshadow_zy300_untouched");

      // a tilted tracker enters the sin(zy)/sin(90-zy) and sin(xy) height
      // corrections at :762-784 (still field-free)
      TenthsMedium tc3 = new TenthsMedium();
      Trackers tk3 = new Trackers();
      tk3.nt = 1;
      tk3.radx[0] = 100;
      tk3.radz[0] = 100;
      tk3.xy[0] = 30;
      tk3.zy[0] = 20;
      ContO lt = new ContO(new byte[0], tc3, tk3);
      lt.npl = 0;
      lt.keyx[0] = 10;
      lt.keyz[0] = 10;
      lt.lowshadow(graphic(), 500);
      Check.eq(tc3.draws, 0, "lowshadow_tilted_zero_draws");
      Check.eq(lt.npl, 0, "lowshadow_tilted_npl_untouched");
      Check.eq(tk3.nt, 1, "lowshadow_tilted_nt_untouched");
   }

   // ---------------------------------------------------------------- helpers

   private static ContO car(Medium m) {
      return new ContO(new byte[0], m, new Trackers());
   }

   /** npl==1 over a 3-point plane; the plane's colours are distinct so Plane
    *  marks it !nocol (Plane.java:132-134). */
   private static ContO plane(Medium m) {
      ContO c = car(m);
      c.npl = 1;
      c.p[0] = new Plane(m, new Trackers(), new int[]{0, 10, 0}, new int[]{0, 0, 10}, new int[]{0, 0, 0}, 3,
            new int[]{10, 20, 30}, false, 0, 0, 0, 0, 7, 0, false);
      return c;
   }

   private static Graphics graphic() {
      return new BufferedImage(600, 500, BufferedImage.TYPE_INT_RGB).createGraphics();
   }

   private static int bits(float f) {
      return Float.floatToIntBits(f);
   }

   /** A Plane that only records that d() reached it, so the av draw ORDER is
    *  observable without reading a single pixel. */
   private static class RecordingPlane extends Plane {
      private final String tag;
      private final List<String> log;

      RecordingPlane(Medium m, String tag, int av, List<String> log) {
         super(m, new Trackers(), new int[]{0, 10, 0}, new int[]{0, 0, 10}, new int[]{0, 0, 0}, 3,
               new int[]{1, 2, 3}, false, 0, 0, 0, 0, 7, 0, false);
         this.tag = tag;
         this.av = av;
         this.log = log;
      }

      public void d(Graphics g, int a, int b, int c, int e, int f, int h, int i, int j, boolean k) {
         log.add(tag);
      }
   }

   /** Replays the measured seed-101 Medium.random() tenths vector (a fresh
    *  Medium under TestRng.install(101), 48 calls; the first 12 match
    *  TRecord.TenthsMedium.VEC) and counts draws. Medium.random() is not final,
    *  so the override sees exactly the calls ContO makes. The 48 values live
    *  in tests/fixtures/toml/determinism.toml [vectors] contodraw_tenths. */
   static class TenthsMedium extends Medium {
      private static final float[] VEC = Toml.load("tests/fixtures/toml/determinism.toml")
            .table("vectors").floatArr("contodraw_tenths");
      int draws;

      public float random() {
         float v = VEC[draws % VEC.length];
         draws++;
         return v;
      }
   }

   /** A constant-valued Medium for formula endpoints (380-0*760 etc). */
   static class FixedMedium extends Medium {
      private final float v;
      int draws;

      FixedMedium(float v) {
         this.v = v;
      }

      public float random() {
         draws++;
         return v;
      }
   }
}
