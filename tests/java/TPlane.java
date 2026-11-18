import java.awt.Color;
import java.awt.Graphics;
import java.awt.image.BufferedImage;
import java.util.Arrays;

/**
 * Pins Plane, the polygon workhorse every visible game part delegates to. The
 * port's rasterizer and geometry path must mirror this class exactly, so this
 * class is deliberately split between cheap structural pins (ctor mapping /
 * defaults / colour bake / deltaf &amp; projf / pure helpers xs-ys-spy-rot), the
 * deterministic d() branches, and the two state machines d() runs for chip
 * sparks and embossed damage.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B5_plane.md, re-derived line by line from
 * reference/vineflower/Plane.java. Every expected value below was measured by
 * running the unmodified original on the gate's Oracle 8u40 and then
 * re-derived from the source before being pinned.
 *
 * RANDOMNESS: Medium.random() is reachable only when embos!=0, chip!=0 or
 * flx!=0 (Plane.java:237,263-289,303-307,361-365,513-518,525-553,603,842).
 * A plane with all three at 0 is fully deterministic - that is asserted
 * structurally (zero draws, untouched walk counters), not assumed. For the
 * state machines this class uses PRESET MEDIUM STATE (the B5 seam recipe) so
 * the real Medium.random() walk supplies the values:
 *
 *    cntrn=20, rand={6,6,6}, diup={false,false,false}, trn=2
 *      -> 0.5, 0.4, 0.3, 0.2, 0.1, 0.0, 0.9, 0.8, 0.7, 0.6, 0.5, ...
 *
 * verified against Medium.java:49-89 (each call decrements all three rand
 * entries, advances trn with a 3->0 wrap, returns rand[trn]/10; cntrn stays
 * non-zero for 20 calls so the 9-Math.random refill never runs). PresetMedium
 * replays that state and counts calls; its first 12 values are pinned as the
 * recipe. A constant FixedMedium is used only where B5 names "random 0.5".
 *
 * This class is far past TestRng's JIT window (see TestRng's header), so no
 * value pin here relies on injection.
 *
 * PIXELS: single-pixel probes are point observations of Java2D's
 * non-antialiased polygon rasterizer on the pinned Oracle 8u40 build, driven
 * offscreen the same way TContODraw drives its Graphics rigs (that class pins
 * only fields; this one has to read the fill to prove the brightness arms).
 * Only interior points and exact integer vertices of small triangles are
 * sampled; boundary pixels that fillPolygon may or may not paint are never
 * pinned. The orchestrator cross-checks these rows in the Temurin container.
 *
 * Two behaviours are deliberately NOT pinned, both documented by B5:
 *   - the OPEN at B5:68 - projf==0 AND deltaf==0 makes var88 NaN and the
 *     resulting getHSBColor(NaN) pixel colour is a JDK internal;
 *   - the OPEN at B5:69 - whether n>3 parts are intended; d()'s projf loop
 *     hardcodes 3 vertices (Plane.java:634-650) while culling/av use n
 *     (:660-817). We pin the 3-only quirk but not an intent.
 * Also NOT pinned: embos=16 with n=1 hangs forever in the while (pa == pb)
 * loop at Plane.java:306-308 (random()*1 always truncates to 0) - the same
 * kind of non-terminating case TSmall documents instead of executing.
 */
public class TPlane {
   /** constants.toml [plane] - the deltaf/projf geometry pins. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("plane");

   private static final int W = 600;
   private static final int H = 500;
   /** Sentinel background so "nothing drawn" is one array compare. */
   private static final int BG = 0x010203;

   static void run() {
      Check.group("Plane");
      K.requireKeys("deltaf_3_4_5", "projf_3_4_5", "deltaf_sample_bits", "av_far");
      presetRecipe();
      fieldDefaults();
      ctorMapping();
      typPins();
      colorPins();
      deltafProjf();
      helpers();
      ctorErrors();
      dBasics();
      dFlx();
      dProjf();
      dTrk();
      dOutline();
      dErrors();
      sMachines();
      sTrackers();
      sModes();
      sCulling();
      embosRanges();
      embosChip();
      extraArms();
   }

   // ------------------------------------------------------------- ctor state

   /** The B5 seam recipe, pinned so every state-machine expectation below has
    *  a documented, reproducible value stream. Medium.java:49-89: with cntrn
    *  already 20 each call walks all three rand entries by one in the diup
    *  direction, advances trn (3 wraps to 0) and returns rand[trn]/10. */
   private static void presetRecipe() {
      PresetMedium m = new PresetMedium(6);
      float[] want = {0.5F, 0.4F, 0.3F, 0.2F, 0.1F, 0.0F,
                      0.9F, 0.8F, 0.7F, 0.6F, 0.5F, 0.4F};
      for (int i = 0; i < want.length; i++) {
         Check.eq(bits(m.random()), bits(want[i]), "preset_value" + i);
      }
      Check.eq(m.draws, 12, "preset_twelve_calls");
      Check.eq(m.cntrn, 8, "preset_cntrn_20_minus_12");
      Check.eq(m.trn, 2, "preset_trn_wraps_back_to_2");
      Check.eq(Arrays.toString(m.rand), "[4, 4, 4]", "preset_rand_after_12");

      // a start of 7 gives the "spark polygon drawn" branch (>0.5) first
      PresetMedium hi = new PresetMedium(7);
      Check.eq(bits(hi.random()), bits(0.6F), "preset_hi_first_0_6");
      // and a start of 3 gives 0.2, used for the flx draw arm
      PresetMedium lo = new PresetMedium(3);
      Check.eq(bits(lo.random()), bits(0.2F), "preset_lo_first_0_2");
   }

   /** Plane.java:5-45 - the 41 declared fields and the values a fresh
    *  construction leaves at their initializer (there is no no-arg ctor, so
    *  the observable default is "field not written by the 15-arg ctor"). */
   private static void fieldDefaults() {
      Check.eq(declaredFieldCount(Plane.class), 41, "field_count_41");

      Plane p = plain();
      Check.eq(p.c.length, 3, "default_c_len3");
      Check.eq(p.oc.length, 3, "default_oc_len3");
      Check.eq(p.hsb.length, 3, "default_hsb_len3");
      Check.eq(p.av, 0, "default_av_0");
      Check.eq(p.chip, 0, "default_chip_0");
      Check.eq(bits(p.ctmag), bits(0.0F), "default_ctmag_0");
      Check.eq(p.cxz, 0, "default_cxz_0");
      Check.eq(p.cxy, 0, "default_cxy_0");
      Check.eq(p.czy, 0, "default_czy_0");
      Check.eq(Arrays.toString(p.cox), "[0, 0, 0]", "default_cox_zero");
      Check.eq(Arrays.toString(p.coz), "[0, 0, 0]", "default_coz_zero");
      Check.eq(Arrays.toString(p.coy), "[0, 0, 0]", "default_coy_zero");
      Check.eq(p.dx, 0, "default_dx_0");
      Check.eq(p.dy, 0, "default_dy_0");
      Check.eq(p.dz, 0, "default_dz_0");
      Check.eq(p.vx, 0, "default_vx_0");
      Check.eq(p.vy, 0, "default_vy_0");
      Check.eq(p.vz, 0, "default_vz_0");
      Check.eq(p.embos, 0, "default_embos_0");
      Check.eq(p.pa, 0, "default_pa_0");
      Check.eq(p.pb, 0, "default_pb_0");
      Check.eq(p.flx, 0, "default_flx_0");
      Check.eq(p.master, 0, "default_master_0");
      Check.eq(bits(p.projf), bits(1.0F), "default_projf_1");
      Check.ok(!p.glass, "default_glass_false");
      Check.ok(!p.road, "default_road_false");
      Check.ok(!p.nocol, "default_nocol_false");
      Check.eq(p.wx, 0, "default_wx_0");
      Check.eq(p.wy, 0, "default_wy_0");
      Check.eq(p.wz, 0, "default_wz_0");
      Check.eq(p.disline, 7, "default_disline_arg_passthrough_7");
   }

   /** Plane.java:81-109,159-200,201-223 - the exact 15-arg param -> field map.
    *  The trap B5 calls out: param 4 is oz and param 5 is oy (Plane.java:107-108),
    *  the reverse of the "x, y, z" reading order. */
   private static void ctorMapping() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      int[] x = {1, 2, 3};
      int[] z = {4, 5, 6};
      int[] y = {7, 8, 9};
      Plane p = new Plane(m, t, x, z, y, 3, new int[]{11, 22, 33, 44}, false,
                          5, 6, 7, 8, 6, 9, true);

      Check.eq(Arrays.toString(p.ox), "[1, 2, 3]", "ctor_p3_to_ox");
      Check.eq(Arrays.toString(p.oz), "[4, 5, 6]", "ctor_p4_to_oz_not_oy");
      Check.eq(Arrays.toString(p.oy), "[7, 8, 9]", "ctor_p5_to_oy_not_oz");
      Check.eq(p.n, 3, "ctor_p6_to_n");
      Check.eq(Arrays.toString(p.oc), "[11, 22, 33]", "ctor_p7_oc_first_three");
      Check.ok(p.m == m, "ctor_p1_to_m");
      Check.ok(p.t == t, "ctor_p2_to_t");
      Check.eq(p.gr, 5, "ctor_p9_to_gr");
      Check.eq(p.wx, 6, "ctor_p10_to_wx");
      Check.eq(p.wy, 7, "ctor_p11_to_wy");
      Check.eq(p.wz, 8, "ctor_p12_to_wz");
      Check.eq(p.disline, 6, "ctor_p13_to_disline");
      Check.eq(p.bfase, 9, "ctor_p14_to_bfase");
      Check.ok(p.road, "ctor_p15_to_road");
      Check.ok(!p.glass, "ctor_p8_false_to_glass");
      Check.eq(p.typ, 3, "ctor_tie_spans_fall_to_3");

      // the coordinate arrays are copied (Plane.java:101-109), so caller
      // mutation cannot reach the plane
      x[0] = 99;
      Check.eq(p.ox[0], 1, "ctor_copies_ox_defensively");

      // the ctor does not call loadprojf, so projf keeps its 1.0F initializer
      Check.eq(bits(p.projf), bits(1.0F), "ctor_never_calls_loadprojf");
      Check.eq(p.av + p.chip + p.embos + p.flx, 0, "ctor_spark_state_untouched");
   }

   /** Plane.java:111-124 - typ is the smallest axis span, with every `if`
    *  still running so ties are overwritten 2 -> 1 -> 3. */
   private static void typPins() {
      Check.eq(mk(new Medium(), null, new int[]{0, 0, 0}, new int[]{0, 0, 100},
                  new int[]{0, 0, 400}, 3, new int[]{1, 2, 3}, false,
                  0, 0, 0, 0, 7, 0, false).typ, 1, "typ_1_x_smallest");
      Check.eq(mk(new Medium(), null, new int[]{0, 400, 0}, new int[]{0, 0, 100},
                  new int[]{0, 0, 0}, 3, new int[]{1, 2, 3}, false,
                  0, 0, 0, 0, 7, 0, false).typ, 2, "typ_2_y_smallest");
      Check.eq(mk(new Medium(), null, new int[]{0, 300, 0}, new int[]{0, 0, 0},
                  new int[]{0, 0, 400}, 3, new int[]{1, 2, 3}, false,
                  0, 0, 0, 0, 7, 0, false).typ, 3, "typ_3_z_smallest");
      // all three spans equal -> the last if wins
      Check.eq(mk(new Medium(), null, new int[]{0, 100, 0}, new int[]{0, 100, 0},
                  new int[]{0, 100, 0}, 3, new int[]{1, 2, 3}, false,
                  0, 0, 0, 0, 7, 0, false).typ, 3, "typ_all_equal_falls_to_3");
      // x and y tie below z: 2 is set first, then 1 overwrites it
      Check.eq(mk(new Medium(), null, new int[]{0, 100, 0}, new int[]{0, 200, 0},
                  new int[]{0, 100, 0}, 3, new int[]{1, 2, 3}, false,
                  0, 0, 0, 0, 7, 0, false).typ, 1, "typ_xy_tie_falls_2_to_1");
   }

   /** Plane.java:129-194 - oc is the raw first three colour bytes; c is the
    *  snap-scaled/clamped copy for solids, or the sky/fade blend for glass;
    *  hsb is RGBtoHSB(c) with the bfase clamps applied only to a non-nocol,
    *  non-glass plane. c/hsb are baked here, so the Medium palette must be set
    *  before construction (B5:63). */
   private static void colorPins() {
      Plane a = colored(new Medium(), new int[]{100, 0, 200}, false);
      Check.eq(Arrays.toString(a.c), "[100, 0, 200]", "color_snap0_c_raw");
      Check.eq(Arrays.toString(a.oc), "[100, 0, 200]", "color_snap0_oc_raw");
      Check.ok(!a.nocol, "color_snap0_nocol_false");

      Medium m100 = new Medium();
      m100.snap[0] = 100;
      m100.snap[1] = 100;
      m100.snap[2] = 100;
      Plane b = colored(m100, new int[]{100, 0, 200}, false);
      Check.eq(Arrays.toString(b.c), "[200, 0, 255]", "color_snap100_clamped_255");

      Medium mneg = new Medium();
      mneg.snap[0] = -50;
      mneg.snap[1] = -50;
      mneg.snap[2] = -50;
      Check.eq(Arrays.toString(colored(mneg, new int[]{100, 0, 200}, false).c),
               "[50, 0, 100]", "color_snapneg_half");

      // oc is never clamped, c always is
      Plane raw = colored(new Medium(), new int[]{300, -5, 0}, false);
      Check.eq(Arrays.toString(raw.oc), "[300, -5, 0]", "color_oc_never_clamped");
      Check.eq(Arrays.toString(raw.c), "[255, 0, 0]", "color_c_clamped_once");

      Plane nc = colored(new Medium(), new int[]{10, 10, 10}, false);
      Check.ok(nc.nocol, "color_equal_rgb_sets_nocol");
      Check.eq(Arrays.toString(nc.c), "[10, 10, 10]", "color_nocol_c_unsnapped");
      Check.eq(bits(nc.hsb[0]), 0, "color_nocol_hsb0_0");
      Check.eq(bits(nc.hsb[1]), 0, "color_nocol_hsb1_0");
      Check.eq(bits(nc.hsb[2]), 1025548449, "color_nocol_hsb2_10_over_255");

      // glass ignores the colour argument entirely and uses the sky/fade blend
      // at the default fade[0]=3000: (csky*6000 + cfade*3000) / 9000
      Plane g1 = colored(new Medium(), new int[]{1, 2, 3}, true);
      Plane g2 = colored(new Medium(), new int[]{200, 100, 50}, true);
      Check.eq(Arrays.toString(g1.c), "[229, 222, 243]", "glass_c_sky_blend");
      Check.eq(Arrays.toString(g2.c), "[229, 222, 243]", "glass_c_ignores_color_arg");
      Check.eq(Arrays.toString(g1.oc), "[1, 2, 3]", "glass_oc_still_raw");
      Check.eq(bits(g1.hsb[0]), 1060692879, "glass_hsb0_blend");
      Check.eq(bits(g1.hsb[1]), 1035009239, "glass_hsb1_blend");
      Check.eq(bits(g1.hsb[2]), 1064563700, "glass_hsb2_blend");

      // bfase clamps: >20 sat, >30 bri floor+sat 0.1, >40 sat 0.05,
      // >50 hue 0.075 + bri cap 0.8, >60 hue 0.05 (Plane.java:163-194)
      Plane k70 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 70);
      Check.eq(bits(k70.hsb[0]), 1028443341, "bfase70_hue_0_05");
      Check.eq(bits(k70.hsb[1]), 1028443341, "bfase70_sat_0_05");
      Check.eq(bits(k70.hsb[2]), 1061997773, "bfase70_bri_0_8");
      Plane k25 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 25);
      Check.eq(bits(k25.hsb[1]), 1045220557, "bfase25_sat_only_0_2");
      Check.eq(bits(k25.hsb[2]), 1065353216, "bfase25_bri_untouched_1");
      Plane k21 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 21);
      Check.eq(bits(k21.hsb[1]), 1045220557, "bfase21_just_over_20_clamps_sat");
      Plane k31 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 31);
      Check.eq(bits(k31.hsb[1]), 1036831949, "bfase31_sat_0_1");
      Check.eq(bits(k31.hsb[2]), 1065353216, "bfase31_bri_above_half_kept");
      Plane k41 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 41);
      Check.eq(bits(k41.hsb[1]), 1028443341, "bfase41_sat_0_05");
      Plane k51 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 51);
      Check.eq(bits(k51.hsb[0]), 1033476506, "bfase51_hue_0_075");
      Check.eq(bits(k51.hsb[2]), 1061997773, "bfase51_bri_capped_0_8");
      Plane k61 = coloredB(new Medium(), new int[]{255, 0, 0}, false, 61);
      Check.eq(bits(k61.hsb[0]), 1028443341, "bfase61_hue_0_05");

      // the >30 brightness floor raises a dark colour UP to 0.5
      Plane kdark = coloredB(new Medium(), new int[]{100, 0, 0}, false, 31);
      Check.eq(bits(kdark.hsb[2]), 1056964608, "bfase31_bri_floored_0_5");

      // nocol short-circuits the whole clamp block (:163)
      Plane kn = coloredB(new Medium(), new int[]{128, 128, 128}, false, 70);
      Check.ok(kn.nocol, "bfase70_nocol_still_true");
      Check.eq(bits(kn.hsb[0]), 0, "bfase70_nocol_hue_untouched");
      Check.eq(bits(kn.hsb[1]), 0, "bfase70_nocol_sat_untouched");
      Check.eq(bits(kn.hsb[2]), 1056997505, "bfase70_nocol_bri_128_over_255");

      // glass short-circuits it too (:163 `!this.nocol && !this.glass`)
      Plane kg = coloredB(new Medium(), new int[]{255, 0, 0}, true, 70);
      Check.eq(bits(kg.hsb[0]), 1060692879, "glass_bfase70_hue_unclamped");
      Check.eq(bits(kg.hsb[1]), 1035009239, "glass_bfase70_sat_unclamped");
      Check.eq(bits(kg.hsb[2]), 1064563700, "glass_bfase70_bri_unclamped");
   }

   /** Plane.java:201-223 (deltaf, all six ordered 3D pairs / 100, / 3) and
    *  Plane.java:47-71 (loadprojf, x/z only - oy is never read). */
   private static void deltafProjf() {
      Plane p1 = mk(new Medium(), null, new int[]{0, 300, 0}, new int[]{0, 0, 0},
                    new int[]{0, 0, 400}, 3, new int[]{1, 2, 3}, false,
                    0, 0, 0, 0, 7, 0, false);
      Check.eq(bits(p1.deltaf), bits(K.floatv("deltaf_3_4_5")), "deltaf_3_4_5_1200");
      Check.eq(bits(p1.projf), bits(1.0F), "deltaf_ctor_leaves_projf_1");
      // x/z 0-300-0 vs 0-0-0 has a 0-length 0<->2 pair -> the product is 0
      p1.loadprojf();
      Check.eq(bits(p1.projf), bits(0.0F), "loadprojf_zero_xz_pair_is_0");

      // moving the 400 onto z makes every x/z pair non-zero: 3-4-5 again
      Plane p2 = mk(new Medium(), null, new int[]{0, 300, 0}, new int[]{0, 0, 400},
                    new int[]{0, 0, 0}, 3, new int[]{1, 2, 3}, false,
                    0, 0, 0, 0, 7, 0, false);
      Check.eq(bits(p2.deltaf), bits(K.floatv("deltaf_3_4_5")), "deltaf_xyz_3_4_5_1200");
      p2.loadprojf();
      Check.eq(bits(p2.projf), bits(K.floatv("projf_3_4_5")), "loadprojf_3_4_5_1200");
      // loadprojf resets projf to 1.0 first, so it is idempotent
      p2.loadprojf();
      Check.eq(bits(p2.projf), bits(K.floatv("projf_3_4_5")), "loadprojf_idempotent");

      Plane p3 = mk(new Medium(), null, new int[]{5, 5, 5}, new int[]{5, 5, 5},
                    new int[]{5, 5, 5}, 3, new int[]{1, 2, 3}, false,
                    0, 0, 0, 0, 7, 0, false);
      Check.eq(bits(p3.deltaf), bits(K.floatv("projf_zero")), "deltaf_identical_vertices_0");
      p3.loadprojf();
      Check.eq(bits(p3.projf), bits(K.floatv("projf_zero")), "loadprojf_identical_vertices_0");

      // IEEE-exact product for the canonical sample: six factors
      // 0.1,0.1,0.1,0.14142136,0.1,0.14142136 then /3
      Check.eq(bits(sample(new Medium()).deltaf), (int) K.longv("deltaf_sample_bits"),
               "deltaf_sample_bits_6_6666684e7");
   }

   /** Plane.java:73-79,1027-1033,1174-1176,1016-1025 - the pure helpers. */
   private static void helpers() {
      Medium m = new Medium();
      Plane p = sample(m);
      Check.eq(p.ys(0, 0), -1400, "ys_z0_clamps_to_cz50_minus1400");
      Check.eq(p.ys(0, 10), -1400, "ys_z10_clamps_to_cz50_too");
      // Medium.ys clamps z to 10 instead of m.cz (:92-96): the contrast pin
      Check.eq(m.ys(0, 10), -7800, "medium_ys_z10_clamps_to_10_minus7800");
      Check.eq(p.ys(100, 50), -600, "ys_100_50_minus600");
      Check.eq(p.ys(200, 400), 200, "ys_at_focus_plane_identity");

      Check.eq(p.xs(0, 50), -1925, "xs_0_50_minus1925");
      Check.eq(p.xs(275, 400), 275, "xs_center_at_focus_275");
      Check.eq(p.xs(1000, 400), 1000, "xs_zero_perspective_at_focus_point");
      Check.eq(p.xs(0, 0), -1925, "xs_z0_clamps_to_cz50");

      Check.eq(p.spy(275, 0), 0, "spy_center_0");
      Check.eq(p.spy(272, 4), 5, "spy_int_sqrt_25");
      Check.eq(p.spy(275, 10), 10, "spy_vertical_10");

      int[] rx = {10};
      int[] ry = {20};
      p.rot(rx, ry, 10, 10, 0, 1);
      Check.eq(rx[0], 10, "rot_angle0_x_untouched");
      Check.eq(ry[0], 20, "rot_angle0_y_untouched");
      rx[0] = 10;
      ry[0] = 20;
      p.rot(rx, ry, 10, 10, 90, 1);
      Check.eq(rx[0], 0, "rot_90_x_truncates_to_0");
      Check.eq(ry[0], 10, "rot_90_y_truncates_to_10");
   }

   /** Plane.java:100-130 - construction failures. A null Medium dies on the
    *  first palette read (:140 solid / :155 glass); a null Trackers is fine. */
   private static void ctorErrors() {
      Check.throwsIt(new Check.Task() {
         public void run() {
            new Plane(null, null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                      new int[]{0, 0, 0}, 3, new int[]{1, 2, 3}, false,
                      0, 0, 0, 0, 7, 0, false);
         }
      }, NullPointerException.class, "err_null_medium_solid_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            new Plane(null, null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                      new int[]{0, 0, 0}, 3, new int[]{1, 2, 3}, true,
                      0, 0, 0, 0, 7, 0, false);
         }
      }, NullPointerException.class, "err_null_medium_glass_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            new Plane(new Medium(), null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                      new int[]{0, 0, 0}, -1, new int[]{1, 2, 3}, false,
                      0, 0, 0, 0, 7, 0, false);
         }
      }, NegativeArraySizeException.class, "err_n_minus1_nase");
      Check.throwsIt(new Check.Task() {
         public void run() {
            new Plane(new Medium(), null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                      new int[]{0, 0, 0}, 2, new int[]{1, 2, 3}, false,
                      0, 0, 0, 0, 7, 0, false);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_n2_ox2_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            new Plane(new Medium(), null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                      new int[]{0, 0, 0}, 4, new int[]{1, 2, 3}, false,
                      0, 0, 0, 0, 7, 0, false);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_short_x_array_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            new Plane(new Medium(), null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                      new int[]{0, 0, 0}, 3, new int[]{1, 2}, false,
                      0, 0, 0, 0, 7, 0, false);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_short_color_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            sample(new Medium()).rot(new int[]{1, 2, 3}, new int[]{4, 5, 6}, 0, 0, 90, 6);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_rot_cnt6_on_len3_aioobe");

      // a null Trackers is not touched by the ctor or d()
      Plane nt = sample(new Medium());
      nt.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(nt.av, 400, "null_trackers_d_still_projects");
      Check.eq(nt.t, null, "null_trackers_stays_null");
   }

   // ------------------------------------------------------------------ d()

   /** Plane.java:226-235,627-653,660-842,847-919 - the canonical on-screen
    *  plane: (275,200),(285,200),(275,210) at z=400, colour 200/200/200,
    *  all angles and offsets zero, embos/chip/flx zero. Projection is the
    *  identity at z==focus_point, so av is the exact bounding-box/gr formula. */
   private static void dBasics() {
      BufferedImage img = image();
      Plane p = sample(new Medium());
      p.d(paint(img), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p.av, 400, "d_av_400");
      Check.eq(bits(p.projf), bits(1.0F), "d_all_zero_angles_leave_projf");
      Check.eq(p.embos, 0, "d_embos_stays_0");
      Check.eq(p.chip, 0, "d_chip_stays_0");
      Check.eq(pixel(img, 277, 202), 0xC8C8C8, "d_fill_200_200_200");
      Check.eq(pixel(img, 276, 201), 0xC8C8C8, "d_fill_second_interior_point");
      Check.eq(pixel(img, 274, 200), BG, "d_fill_does_not_reach_outside");
      Check.eq(pixel(img, 275, 200), 0x000000, "d_outline_black_at_vertex");
      Check.eq(pixel(img, 275, 205), 0x000000, "d_outline_black_on_edge");

      // all local copies: the argument/field arrays are untouched
      int[] ax = {275, 285, 275};
      int[] az = {400, 400, 400};
      int[] ay = {200, 200, 210};
      Plane moved = mk(new Medium(), null, ax, az, ay, 3, new int[]{200, 200, 200},
                       false, 0, 0, 0, 0, 7, 0, false);
      moved.d(paint(image()), 10, 20, 30, 10, 20, 30, 10, 20, false);
      Check.eq(Arrays.toString(ax), "[275, 285, 275]", "d_input_x_array_untouched");
      Check.eq(Arrays.toString(ay), "[200, 200, 210]", "d_input_y_array_untouched");
      Check.eq(Arrays.toString(az), "[400, 400, 400]", "d_input_z_array_untouched");
      Check.eq(Arrays.toString(moved.ox), "[275, 285, 275]", "d_field_ox_untouched");

      // noline=true: var88 is clamped to 1.0 then x0.89 -> 200*0.89 == 178
      BufferedImage ni = image();
      Plane np = sample(new Medium());
      np.d(paint(ni), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(np.av, 400, "d_noline_av_400");
      Check.eq(pixel(ni, 277, 202), 0xB2B2B2, "d_noline_brightness_0_89_178");
      Check.eq(pixel(ni, 275, 200), 0xB2B2B2, "d_noline_no_black_outline");

      // gr=-9 forces var88=0.7F only inside the noline arm (:869-871)
      BufferedImage gi = image();
      Plane gp = sample(new Medium());
      gp.gr = -9;
      gp.d(paint(gi), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(gi, 277, 202), 0x8C8C8C, "d_gr_minus9_noline_0_7_140");
      // with noline=false the gr branch is not reached at all
      BufferedImage g2i = image();
      Plane g2p = sample(new Medium());
      g2p.gr = -9;
      g2p.d(paint(g2i), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(g2i, 277, 202), 0xC8C8C8, "d_gr_minus9_line_has_no_effect");

      // av>3000 + adv<=900 flips noline on internally (:838-840). A 4000-deep
      // plane still projects to a 10x4 triangle: (272,198),(283,198),(272,202).
      Medium am = faded(new Medium(), 30000);
      BufferedImage ai = image();
      Plane ap = mk(am, null, new int[]{250, 350, 250}, new int[]{4000, 4000, 4000},
                    new int[]{180, 180, 220}, 3, new int[]{200, 200, 200}, false,
                    0, 0, 0, 0, 7, 0, false);
      ap.d(paint(ai), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(ap.av, 4000, "d_av_4000_depth");
      Check.eq(pixel(ai, 274, 200), 0xB2B2B2, "d_far_adv900_forces_noline_178");
      am.adv = 901;
      BufferedImage ai2 = image();
      Plane ap2 = mk(am, null, new int[]{250, 350, 250}, new int[]{4000, 4000, 4000},
                     new int[]{180, 180, 220}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      ap2.d(paint(ai2), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(ai2, 274, 200), 0xC8C8C8, "d_far_adv901_keeps_full_brightness");

      // off-screen: every vertex has z<10 -> var86/m.w/h counters cull at
      // :712-746 before the draw block; a fresh av stays 0 and a preset one
      // proves av is not written on the failed path
      Plane off = mk(new Medium(), null, new int[]{5000, 5010, 5000},
                     new int[]{0, 0, 0}, new int[]{200, 200, 210}, 3,
                     new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      BufferedImage oi = image();
      off.d(paint(oi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(off.av, 0, "d_offscreen_fresh_av_stays_0");
      Check.eq(bits(off.projf), bits(1.0F), "d_offscreen_projf_untouched");
      Check.ok(Arrays.equals(pixels(oi), blankPixels()), "d_offscreen_draws_nothing");
      off.av = 77;
      off.d(null, 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(off.av, 77, "d_offscreen_av_not_written");
      Check.eq(bits(off.projf), bits(1.0F), "d_offscreen_null_graphics_safe");

      // a projected y<45 zeroes m.flex in the cull loop (:731-733)
      Medium fm = new Medium();
      fm.flex = 5;
      Plane fp = sample(fm);
      fp.oy[0] = -200;
      fp.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(fm.flex, 0, "d_low_vertex_resets_flex");
      // control: the same plane on-screen leaves flex alone
      Medium fm2 = new Medium();
      fm2.flex = 5;
      Plane fp2 = sample(fm2);
      fp2.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(fm2.flex, 5, "d_onscreen_leaves_flex_5");

      // embos==chip==flx==0 consumes no randomness at all (:237,513,603,842)
      PresetMedium zm = new PresetMedium(6);
      Plane zp = sample(zm);
      zp.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(zm.draws, 0, "d_zero_random_draws");
      Check.eq(zm.cntrn, 20, "d_zero_draws_cntrn_untouched");
      Check.eq(zm.trn, 2, "d_zero_draws_trn_untouched");
      Check.eq(Arrays.toString(zm.rand), "[6, 6, 6]", "d_zero_draws_rand_untouched");

      // gr>0 with av>3500 culls (:834-836) - same 4000-deep triangle
      Medium gm = faded(new Medium(), 30000);
      BufferedImage gmImg = image();
      Plane gmp = mk(gm, null, new int[]{250, 350, 250}, new int[]{4000, 4000, 4000},
                     new int[]{180, 180, 220}, 3, new int[]{200, 200, 200}, false,
                     1, 0, 0, 0, 7, 0, false);
      gmp.d(paint(gmImg), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(gmp.av, 4000, "d_gr1_far_av_4000");
      Check.ok(Arrays.equals(pixels(gmImg), blankPixels()), "d_gr1_far_3500_cull_nothing_drawn");
   }

   /** Plane.java:842,916-975 - the flx outline animation. flx!=0 adds one
    *  random draw (a >0.3 wake check), so every case here presets the stream.
    *  flx==1 draws the green-ish snap outline and becomes 2; flx==3 draws the
    *  other one and also becomes 2; flx==2 only draws black. */
   private static void dFlx() {
      PresetMedium hi = new PresetMedium(6); // first draw 0.5 > 0.3
      Plane wake = sample(hi);
      wake.flx = 1;
      BufferedImage wi = image();
      wake.d(paint(wi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(wake.av, 400, "d_flx1_av_computed_before_wake_check");
      Check.eq(wake.flx, 1, "d_flx1_high_random_culls_without_advancing");
      Check.eq(hi.draws, 1, "d_flx1_high_random_one_draw");
      Check.eq(pixel(wi, 277, 202), BG, "d_flx1_high_random_draws_nothing");

      PresetMedium lo = new PresetMedium(3); // first draw 0.2 <= 0.3
      Plane lit = sample(lo);
      lit.flx = 1;
      BufferedImage li = image();
      lit.d(paint(li), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(lo.draws, 1, "d_flx1_low_random_one_draw");
      Check.eq(lit.flx, 2, "d_flx1_low_random_advances_to_2");
      Check.eq(pixel(li, 277, 202), 0xC8C8C8, "d_flx1_fill_still_drawn");
      Check.eq(pixel(li, 275, 200), 0x00DFFF, "d_flx1_outline_snap_colour_0_223_255");

      PresetMedium l3 = new PresetMedium(3);
      Plane lit3 = sample(l3);
      lit3.flx = 3;
      BufferedImage l3i = image();
      lit3.d(paint(l3i), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(lit3.flx, 2, "d_flx3_advances_to_2");
      Check.eq(pixel(l3i, 275, 200), 0x00FFDF, "d_flx3_outline_snap_colour_0_255_223");

      PresetMedium l2 = new PresetMedium(3);
      Plane lit2 = sample(l2);
      lit2.flx = 2;
      BufferedImage l2i = image();
      lit2.d(paint(l2i), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(lit2.flx, 2, "d_flx2_stays_2");
      Check.eq(pixel(l2i, 275, 200), 0x000000, "d_flx2_outline_black");
      Check.eq(l2.cntrn, 19, "d_flx2_one_draw");
   }

   /** Plane.java:627-653 - the projf recompute runs only when one of the xz/
    *  xy/zy ANGLE arguments is non-zero, and always over exactly three
    *  vertices (:634-650), even when n==4. */
   private static void dProjf() {
      Plane p0 = sample(new Medium());
      p0.projf = 7.5F;
      p0.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(bits(p0.projf), bits(7.5F), "dprojf_zero_angles_untouched");

      Plane px = sample(new Medium());
      px.projf = 7.5F;
      px.d(paint(image()), 0, 0, 0, 90, 0, 0, 0, 0, false);
      Check.eq(bits(px.projf), bits(0.0F), "dprojf_xz90_overwrites_0");

      Plane py = sample(new Medium());
      py.projf = 7.5F;
      py.d(paint(image()), 0, 0, 0, 0, 90, 0, 0, 0, false);
      Check.eq(bits(py.projf), bits(0.0F), "dprojf_xy90_overwrites_0");

      // zy only rotates y/z: the sample's distances collapse onto the same
      // 3-4-5 multiset, so the value equals deltaf bit-for-bit
      Plane pz = sample(new Medium());
      pz.projf = 7.5F;
      pz.d(paint(image()), 0, 0, 0, 0, 0, 90, 0, 0, false);
      Check.eq(bits(pz.projf), (int) K.longv("deltaf_sample_bits"),
            "dprojf_zy90_overwrites_to_deltaf_value");

      // n=4: only the first three vertices feed the recompute. Same first 3
      // with a wildly different 4th gives the identical non-zero projf.
      Plane a4 = mk(new Medium(), null, new int[]{275, 285, 280, 275},
                    new int[]{400, 400, 400, 999}, new int[]{200, 200, 210, 250}, 4,
                    new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      a4.d(null, 0, 0, 0, 90, 0, 0, 0, 0, false);
      Plane b4 = mk(new Medium(), null, new int[]{275, 285, 280, 275},
                    new int[]{400, 400, 400, 0}, new int[]{200, 200, 210, 250}, 4,
                    new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      b4.d(null, 0, 0, 0, 90, 0, 0, 0, 0, false);
      Check.eq(bits(a4.projf), bits(b4.projf), "dprojf_n4_ignores_fourth_vertex");
      Check.eq(bits(a4.projf), 850588925, "dprojf_n4_first_three_value");
      Check.eq(a4.av + b4.av, 0, "dprojf_n4_both_culled_av_0");

      // coincident first three: the (ignored) 4th would make the product
      // non-zero, but the hardcoded 3 vertices yield 0.0 exactly
      Plane c4 = mk(new Medium(), null, new int[]{275, 275, 275, 300},
                    new int[]{400, 400, 400, 400}, new int[]{200, 200, 200, 200}, 4,
                    new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      c4.d(null, 0, 0, 0, 90, 0, 0, 0, 0, false);
      Check.eq(bits(c4.projf), bits(0.0F), "dprojf_n4_coincident_first3_zero");

      // wx==0 skips the wxz rotation (:509-511) but wz!=0 still rotates y/z
      // (:505-507), so only wz changes the picture
      BufferedImage base = image();
      sample(new Medium()).d(paint(base), 0, 0, 0, 0, 0, 0, 0, 0, false);
      BufferedImage wide = image();
      sample(new Medium()).d(paint(wide), 0, 0, 0, 0, 0, 0, 90, 0, false);
      Check.ok(Arrays.equals(pixels(base), pixels(wide)), "d_wx0_ignores_wxzarg");

      Plane wxp = sample(new Medium());
      wxp.wx = 100;
      BufferedImage wxImg = image();
      wxp.d(paint(wxImg), 0, 0, 0, 0, 0, 0, 90, 0, false);
      Check.ok(!Arrays.equals(pixels(base), pixels(wxImg)), "d_wx100_applies_wxzarg");
      Check.eq(wxp.av, 0, "d_wx100_off_screen_av_0");

      Plane wzp = sample(new Medium());
      wzp.wz = 100;
      BufferedImage wzImg = image();
      wzp.d(paint(wzImg), 0, 0, 0, 0, 0, 0, 0, 90, false);
      Check.ok(!Arrays.equals(pixels(base), pixels(wzImg)), "d_wz100_applies_wzyarg");
   }

   /** Plane.java:891-897 - with m.trk the fill comes from oc alone (fixed
    *  hue/sat, brightness from oc's V), never from c; and :744-746 additionally
    *  culls the whole plane when ANY vertex leaves the viewport. */
   private static void dTrk() {
      Medium m1 = new Medium();
      m1.trk = true;
      Medium m2 = new Medium();
      m2.trk = true;
      m2.snap[0] = 100;
      m2.snap[1] = 100;
      m2.snap[2] = 100;
      Plane p1 = sample(m1);
      Plane p2 = sample(m2);
      Check.eq(Arrays.toString(p1.oc), Arrays.toString(p2.oc), "dtrk_same_oc");
      Check.ok(!Arrays.equals(p1.c, p2.c), "dtrk_different_c");
      BufferedImage i1 = image();
      BufferedImage i2 = image();
      p1.d(paint(i1), 0, 0, 0, 0, 0, 0, 0, 0, false);
      p2.d(paint(i2), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.ok(Arrays.equals(pixels(i1), pixels(i2)), "dtrk_identical_pixels_from_oc_only");
      Check.eq(pixel(i1, 277, 202), 0xE2920B, "dtrk_fill_hsb_0_105_0_95_from_oc");
      Check.eq(p1.av, 400, "dtrk_av_400");

      // one off-screen vertex: the plain plane still draws, the trk plane is
      // rejected outright
      Medium plainM = new Medium();
      Plane plain = mk(plainM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                       new int[]{200, 200, 1000}, 3, new int[]{200, 200, 200}, false,
                       0, 0, 0, 0, 7, 0, false);
      BufferedImage pi = image();
      plain.d(paint(pi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.ok(!Arrays.equals(pixels(pi), blankPixels()), "dtrk_control_plain_draws");
      Check.eq(plain.av, K.intv("av_probe"), "dtrk_control_plain_av_565");

      Medium trkM = new Medium();
      trkM.trk = true;
      Plane trk = mk(trkM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 1000}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      BufferedImage ti = image();
      trk.d(paint(ti), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.ok(Arrays.equals(pixels(ti), blankPixels()), "dtrk_one_offscreen_vertex_culls");
      Check.eq(trk.av, 0, "dtrk_one_offscreen_av_0");
   }

   /** Plane.java:976-1012 - the two extra outline passes: road (fill-10) and
    *  the gr==-10 c-fade outline. Both live in mutually exclusive branches. */
   private static void dOutline() {
      Medium rm = new Medium();
      rm.fade[0] = 5000;
      Plane road = mk(rm, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                      new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                      0, 0, 0, 0, 7, 0, true);
      BufferedImage ri = image();
      road.d(paint(ri), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(road.av, 400, "droad_av_400");
      Check.eq(pixel(ri, 277, 202), 0xB2B2B2, "droad_interior_178");
      Check.eq(pixel(ri, 275, 200), 0xA8A8A8, "droad_outline_fill_minus_10_168");

      // fade[0] must exceed 4000 strictly
      Medium rm2 = new Medium();
      rm2.fade[0] = 4000;
      Plane road2 = mk(rm2, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                       new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                       0, 0, 0, 0, 7, 0, true);
      BufferedImage ri2 = image();
      road2.d(paint(ri2), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(ri2, 275, 200), 0xB2B2B2, "droad_fade4000_no_extra_outline");

      // noline=false never reaches the road arm (it is the else-if of !var10)
      Plane road3 = mk(rm, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                       new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                       0, 0, 0, 0, 7, 0, true);
      BufferedImage ri3 = image();
      road3.d(paint(ri3), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(ri3, 277, 202), 0xC8C8C8, "droad_noline_false_interior_200");
      Check.eq(pixel(ri3, 275, 200), 0x000000, "droad_noline_false_black_outline");

      // gr==-10, !glass, !trk: a second outline in c (after the fade blend)
      Plane g10 = mk(new Medium(), null, new int[]{275, 285, 275},
                     new int[]{400, 400, 400}, new int[]{200, 200, 210}, 3,
                     new int[]{200, 200, 200}, false, -10, 0, 0, 0, 7, 0, false);
      BufferedImage g10i = image();
      g10.d(paint(g10i), 0, 0, 0, 0, 0, 0, 0, 0, true);
      // the |gr^3| term inside av (Plane.java:822-829) is what makes this 401
      Check.eq(g10.av, 401, "dgr10_av_includes_abs_gr_cubed");
      Check.eq(pixel(g10i, 277, 202), 0xB2B2B2, "dgr10_interior_noline_178");
      Check.eq(pixel(g10i, 275, 200), 0xC8C8C8, "dgr10_extra_outline_c_colour_200");

      Plane g10glass = mk(new Medium(), null, new int[]{275, 285, 275},
                          new int[]{400, 400, 400}, new int[]{200, 200, 210}, 3,
                          new int[]{1, 2, 3}, true, -10, 0, 0, 0, 7, 0, false);
      BufferedImage gg = image();
      g10glass.d(paint(gg), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(gg, 275, 200), 0x000000, "dgr10_glass_skips_extra_outline");
      Check.eq(pixel(gg, 277, 202), 0xE5DEF3, "dgr10_glass_fill_sky_blend");

      Medium tm = new Medium();
      tm.trk = true;
      Plane g10trk = mk(tm, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                        new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                        -10, 0, 0, 0, 7, 0, false);
      BufferedImage gt = image();
      g10trk.d(paint(gt), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(gt, 275, 200), 0x000000, "dgr10_trk_skips_extra_outline");
      Check.eq(pixel(gt, 277, 202), 0xE2920B, "dgr10_trk_fill_oc_derived");
   }

   /** Plane.java:830 - fade is int[8], and the av-vs-fade test reads
    *  fade[disline]. m.trk short-circuits the whole condition. */
   private static void dErrors() {
      Check.throwsIt(new Check.Task() {
         public void run() {
            Plane p = sample(new Medium());
            p.disline = 8;
            p.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
         }
      }, ArrayIndexOutOfBoundsException.class, "derr_disline8_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            Plane p = sample(new Medium());
            p.disline = -1;
            p.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
         }
      }, ArrayIndexOutOfBoundsException.class, "derr_disline_minus1_aioobe");

      Medium tm = new Medium();
      tm.trk = true;
      Plane p = sample(tm);
      p.disline = 8;
      BufferedImage img = image();
      p.d(paint(img), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p.av, 400, "derr_trk_short_circuits_fade_index");
      Check.eq(pixel(img, 277, 202), 0xE2920B, "derr_trk_still_draws");
   }

   // ------------------------------------------------------------------ s()

   /** Plane.java:1035-1172 - s() is pure with respect to game state: every
    *  write targets a local array, so Plane and Trackers snapshots are
    *  identical (B5:50, the corrected claim). gr>0 returns immediately, and
    *  the no-tracker ground path assigns m.ground verbatim (so m.y is NOT
    *  subtracted, unlike the tracker path at :1073). Fill is cgrnd/1.5. */
   private static void sMachines() {
      Medium m = new Medium();
      Trackers t = matchedTracker(150, 150, 150, 150, 50);
      Plane p = shadow(m, t);
      int hsb0Before = bits(p.hsb[0]);
      String planeBefore = snapshot(p);
      String tracksBefore = trackerSnapshot(t);

      BufferedImage img = image();
      p.s(paint(img), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(snapshot(p), planeBefore, "s_leaves_plane_state_identical");
      Check.eq(trackerSnapshot(t), tracksBefore, "s_leaves_trackers_state_identical");
      Check.eq(p.av, 0, "s_never_writes_av");
      Check.eq(bits(p.hsb[0]), hsb0Before, "s_never_writes_hsb");
      Check.eq(pixel(img, 277, 151), 0x646464, "s_tracker_fill_t_c_over_1_5_100");

      // no random consumer anywhere in s()
      PresetMedium pm = new PresetMedium(6);
      shadow(pm, new Trackers()).s(paint(image()), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pm.draws, 0, "s_zero_random_draws");

      // gr>0 -> immediate return (:1036), even with a null Graphics
      Plane noGround = shadow(new Medium(), new Trackers());
      BufferedImage ngi = image();
      noGround.gr = 1;
      noGround.s(paint(ngi), 0, 0, 0, 0, 0, 0, 0);
      Check.ok(Arrays.equals(pixels(ngi), blankPixels()), "s_gr1_paints_nothing");
      noGround.s(null, 0, 0, 0, 0, 0, 0, 0);
      Check.eq(noGround.gr, 1, "s_gr1_returns_before_reading_graphics");

      // default ground shadow: cgrnd {205,200,200} / 1.5 -> {136,133,133}
      Plane ground = shadow(new Medium(), new Trackers());
      BufferedImage gi = image();
      ground.s(paint(gi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(gi, 277, 248), 0x888585, "s_ground_cgrnd_over_1_5");

      // the no-tracker arm writes m.ground verbatim, so a m.y offset cannot
      // change a single pixel (contrast the tracker arm :1073)
      Medium my = new Medium();
      my.y = 100;
      Plane withY = shadow(my, new Trackers());
      BufferedImage yi = image();
      withY.s(paint(yi), 0, 0, 0, 0, 0, 0, 0);
      Check.ok(Arrays.equals(pixels(gi), pixels(yi)), "s_m_y_100_byte_identical");
      Check.eq(my.ground, 250, "s_m_y_does_not_touch_ground");

      // tracker offset arithmetic uses t.x-m.x / t.z-m.z
      Medium off = new Medium();
      off.x = 100;
      off.z = 50;
      Trackers toff = new Trackers();
      toff.nt = 1;
      toff.radx[0] = 50;
      toff.radz[0] = 50;
      toff.x[0] = 375;
      toff.z[0] = 450;
      toff.y[0] = 150;
      toff.c[0][0] = 150;
      toff.c[0][1] = 150;
      toff.c[0][2] = 150;
      Plane rel = shadow(off, toff);
      BufferedImage reli = image();
      rel.s(paint(reli), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(reli, 277, 151), 0x646464, "s_tracker_uses_relative_offsets");
   }

   /** Plane.java:1054-1101 - the tracker lock: both zy/xy must avoid +-90,
    *  every vertex must sit inside radx/radz, more than n/2 must match, and
    *  the scan runs nt-1..0 so the highest index wins. */
   private static void sTrackers() {
      // radx defaults to 0 -> |...| < 0 is never true
      Trackers zero = new Trackers();
      zero.nt = 1;
      zero.x[0] = 275;
      zero.z[0] = 400;
      zero.y[0] = 150;
      zero.c[0][0] = 150;
      zero.c[0][1] = 150;
      zero.c[0][2] = 150;
      BufferedImage zi = image();
      shadow(new Medium(), zero).s(paint(zi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(zi, 277, 248), 0x888585, "s_radx0_rejects_uses_ground");

      // |zy|==90 or |xy|==90 skips the tracker entirely
      Trackers zy = matchedTracker(150, 150, 150, 150, 50);
      zy.zy[0] = 90;
      BufferedImage zyi = image();
      shadow(new Medium(), zy).s(paint(zyi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(zyi, 277, 248), 0x888585, "s_zy90_skips_tracker");
      Check.eq(pixel(zyi, 277, 151), BG, "s_zy90_leaves_ground_height");

      Trackers xy = matchedTracker(150, 150, 150, 150, 50);
      xy.xy[0] = 90;
      BufferedImage xyi = image();
      shadow(new Medium(), xy).s(paint(xyi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(xyi, 277, 248), 0x888585, "s_xy90_skips_tracker");

      // two matching trackers: index 1 (scanned first) wins
      Trackers two = new Trackers();
      two.nt = 2;
      for (int i = 0; i < 2; i++) {
         two.radx[i] = 50;
         two.radz[i] = 50;
         two.x[i] = 275;
         two.z[i] = 400;
      }
      two.y[0] = 100;
      two.c[0][0] = 150;
      two.c[0][1] = 150;
      two.c[0][2] = 150;
      two.y[1] = 150;
      two.c[1][0] = 30;
      two.c[1][1] = 60;
      two.c[1][2] = 90;
      BufferedImage twi = image();
      shadow(new Medium(), two).s(paint(twi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(twi, 277, 151), 0x14283C, "s_two_trackers_highest_index_wins");
      Check.eq(pixel(twi, 277, 101), BG, "s_two_trackers_low_index_not_used");

      // t.y-m.y is the tracker-side ground offset (:1073)
      Medium my = new Medium();
      my.y = 40;
      Trackers ty = matchedTracker(190, 150, 150, 150, 50);
      BufferedImage tyi = image();
      shadow(my, ty).s(paint(tyi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(tyi, 277, 151), 0x646464, "s_tracker_y_minus_m_y_150");
   }

   /** Plane.java:1058,1110-1122 - mode gates. mode!=0 skips the tracker lock;
    *  mode==2 forces (76,98,142) and escapes the spark-occlusion scan; every
    *  other mode keeps cgrnd but still hides the shadow under a spark. */
   private static void sModes() {
      Trackers t = matchedTracker(150, 150, 150, 150, 50);

      BufferedImage m1i = image();
      shadow(new Medium(), t).s(paint(m1i), 0, 0, 0, 0, 0, 0, 1);
      Check.eq(pixel(m1i, 277, 248), 0x888585, "s_mode1_skips_tracker_lock");
      Check.eq(pixel(m1i, 277, 151), BG, "s_mode1_keeps_ground_height");

      BufferedImage m2i = image();
      shadow(new Medium(), t).s(paint(m2i), 0, 0, 0, 0, 0, 0, 2);
      Check.eq(pixel(m2i, 277, 248), 0x4C628E, "s_mode2_forces_76_98_142");

      Medium spm = new Medium();
      spm.nsp = 1;
      spm.spx[0] = 275;
      spm.spz[0] = 400;
      spm.sprad[0] = 50;
      BufferedImage sp1 = image();
      shadow(spm, new Trackers()).s(paint(sp1), 0, 0, 0, 0, 0, 0, 1);
      Check.ok(Arrays.equals(pixels(sp1), blankPixels()), "s_mode1_spark_occludes");
      BufferedImage sp0 = image();
      shadow(spm, new Trackers()).s(paint(sp0), 0, 0, 0, 0, 0, 0, 0);
      Check.ok(Arrays.equals(pixels(sp0), blankPixels()), "s_mode0_spark_occludes_too");
      BufferedImage sp2 = image();
      shadow(spm, new Trackers()).s(paint(sp2), 0, 0, 0, 0, 0, 0, 2);
      Check.eq(pixel(sp2, 277, 248), 0x4C628E, "s_mode2_spark_still_drawn");

      // av beyond every fade entry: eight successive 3/4 blends toward cfade
      Plane far = shadow(new Medium(), new Trackers());
      far.av = 30000;
      BufferedImage fi = image();
      far.s(paint(fi), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(fi, 277, 248), 0xF1D2D2, "s_av30000_blends_to_241_210_210");

      // av==0 (fresh) does no blending at all
      Plane near = shadow(new Medium(), new Trackers());
      BufferedImage ni = image();
      near.s(paint(ni), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(ni, 277, 248), 0x888585, "s_av0_no_blend");
   }

   /** Plane.java:1059,1063,1132-1157 - the culling half of s(). */
   private static void sCulling() {
      Plane off = mk(new Medium(), new Trackers(), new int[]{5000, 5010, 5000},
                     new int[]{0, 0, 0}, new int[]{200, 200, 210}, 3,
                     new int[]{10, 20, 30}, false, 0, 0, 0, 0, 7, 0, false);
      BufferedImage oi = image();
      off.s(paint(oi), 0, 0, 0, 0, 0, 0, 0);
      Check.ok(Arrays.equals(pixels(oi), blankPixels()), "s_offscreen_draws_nothing");
      off.s(null, 0, 0, 0, 0, 0, 0, 0);
      Check.eq(off.av, 0, "s_offscreen_null_graphics_safe");

      Check.throwsIt(new Check.Task() {
         public void run() {
            shadow(new Medium(), null).s(null, 0, 0, 0, 0, 0, 0, 0);
         }
      }, NullPointerException.class, "s_null_trackers_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            Trackers big = new Trackers();
            big.nt = 501;
            shadow(new Medium(), big).s(null, 0, 0, 0, 0, 0, 0, 0);
         }
      }, ArrayIndexOutOfBoundsException.class, "s_nt501_aioobe");
   }

   // --------------------------------------------------------- embos / chip

   /** Plane.java:261-289,478-496 - the embos brightness multipliers and the
    *  per-typ axis scaling. var44 = 1 + random()/4 (5..7), /3 (8..9), /2
    *  (9..10), /1 (10..12); the 8..9 arm caps hsb[2] at 0.7 and 9..10 at 0.6.
    *  The scaled axis is typ: x for 1, y for 2, z for 3. */
   private static void embosRanges() {
      PresetMedium e5 = new PresetMedium(6);
      Plane p5 = sample(e5);
      p5.embos = 5;
      p5.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p5.embos, 6, "embos5_advances_to_6");
      Check.eq(e5.draws, 2, "embos5_two_draws");
      Check.eq(p5.av, 440, "embos5_typ3_z_scaled_av_440");
      Check.eq(bits(p5.hsb[2]), 1061734601, "embos5_bri_uncapped");

      PresetMedium e8 = new PresetMedium(6);
      Plane p8 = sample(e8);
      p8.embos = 8;
      p8.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p8.embos, 9, "embos8_advances_to_9");
      Check.eq(e8.draws, 2, "embos8_two_draws");
      Check.eq(p8.av, 453, "embos8_av_453");
      Check.eq(bits(p8.hsb[2]), 1060320051, "embos8_bri_capped_0_7");

      PresetMedium e10 = new PresetMedium(6);
      Plane p10 = sample(e10);
      p10.embos = 10;
      p10.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p10.embos, 11, "embos10_advances_to_11");
      Check.eq(e10.draws, 2, "embos10_two_draws");
      Check.eq(p10.av, 480, "embos10_av_480");
      Check.eq(bits(p10.hsb[2]), 1058642330, "embos10_bri_capped_0_6");

      // typ 1 scales ox, typ 2 scales oy (the same 1.08 factor -> same av)
      PresetMedium e1a = new PresetMedium(6);
      Plane t1 = mk(e1a, null, new int[]{275, 275, 275}, new int[]{400, 420, 400},
                    new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                    0, 0, 0, 0, 7, 0, false);
      Check.eq(t1.typ, 1, "embos_typ1_fixture");
      t1.embos = 1;
      t1.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(t1.embos, 2, "embos1_typ1_advances_to_2");
      Check.eq(t1.av, 410, "embos1_typ1_av_410");
      Check.eq(e1a.draws, 2, "embos1_typ1_two_draws");

      PresetMedium e2a = new PresetMedium(6);
      Plane t2 = mk(e2a, null, new int[]{275, 285, 275}, new int[]{400, 420, 400},
                    new int[]{200, 200, 200}, 3, new int[]{200, 200, 200}, false,
                    0, 0, 0, 0, 7, 0, false);
      Check.eq(t2.typ, 2, "embos_typ2_fixture");
      t2.embos = 1;
      t2.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(t2.embos, 2, "embos1_typ2_advances_to_2");
      Check.eq(t2.av, 410, "embos1_typ2_av_410");
      Check.eq(e2a.draws, 2, "embos1_typ2_two_draws");
   }

   /** B5 pins 41-48. Every d() here starts from the preset recipe Medium so
    *  the real Medium.random() stream is fixed; the expected values are the
    *  measured consequence of that stream, re-derived from Plane.java. */
   private static void embosChip() {
      // pin 41: chip==1 with bfase==0 always resets; the nocol plane then
      // seeds bfase=1. The random is evaluated first (:513) => exactly 1 draw.
      PresetMedium c41 = new PresetMedium(6);
      Plane p41 = mk(c41, null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                     new int[]{0, 0, 0}, 3, new int[]{10, 10, 10}, false,
                     0, 0, 0, 0, 7, 0, false);
      p41.chip = 1;
      p41.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p41.chip, 0, "chip1_bfase0_resets_chip");
      Check.eq(p41.bfase, 1, "chip1_bfase0_nocol_seeds_bfase_1");
      Check.eq(c41.draws, 1, "chip1_bfase0_one_random_draw");
      Check.ok(p41.nocol, "chip1_nocol_plane");

      // pin 42: 19 -> draw -> 20 -> wraps to 0 (:621-624)
      PresetMedium c42 = new PresetMedium(6);
      Plane p42 = sample(c42);
      p42.chip = 19;
      p42.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p42.chip, 0, "chip19_wraps_to_0");
      Check.eq(c42.draws, 1, "chip19_one_colour_draw");

      // pin 43: chip>=2 is ballistic - dx/dy/dz are copied out, then advanced
      // by vx/vy/vz; vy gains 7 every call (:581-584)
      PresetMedium c43 = new PresetMedium(6);
      Plane p43 = sample(c43);
      p43.chip = 5;
      p43.vx = 1;
      p43.vy = 2;
      p43.vz = 3;
      p43.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p43.dx, 1, "chip5_dx_copies_vx");
      Check.eq(p43.dy, 2, "chip5_dy_copies_vy");
      Check.eq(p43.dz, 3, "chip5_dz_copies_vz");
      Check.eq(p43.vy, 9, "chip5_vy_plus_7");
      Check.eq(p43.chip, 6, "chip5_advances_to_6");

      // ground hit: var55[0] > m.ground sets 19, then the ++ wraps it to 0
      PresetMedium c43b = new PresetMedium(6);
      Plane p43b = sample(c43b);
      p43b.chip = 5;
      p43b.vx = 1;
      p43b.vy = 2;
      p43b.vz = 3;
      p43b.m.ground = -10;
      p43b.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p43b.chip, 0, "chip5_ground_hit_19_then_wraps_0");
      Check.eq(p43b.vy, 9, "chip5_ground_hit_still_advances_vy");

      // pin 44: chip==1 seeds cxz/cxy/czy from the angles; ctmag clamps to
      // +-3 before the offsets (:529-535). Constant 0.5 stream is the B5 case.
      FixedMedium fm = new FixedMedium(0.5F);
      Plane p44 = mk(fm, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 5, false);
      p44.chip = 1;
      p44.ctmag = 7.0F;
      p44.d(paint(image()), 0, 0, 0, 11, 22, 33, 0, 0, false);
      Check.eq(bits(p44.ctmag), bits(3.0F), "chip1_ctmag_clamped_3");
      Check.eq(p44.cxz, 11, "chip1_cxz_from_xz_arg");
      Check.eq(p44.cxy, 22, "chip1_cxy_from_xy_arg");
      Check.eq(p44.czy, 33, "chip1_czy_from_zy_arg");
      Check.eq(Arrays.toString(p44.cox), "[285, 285, 285]", "chip1_cox_seed_index");
      Check.eq(Arrays.toString(p44.coy), "[200, 200, 200]", "chip1_coy_seed_index");
      Check.eq(Arrays.toString(p44.coz), "[400, 400, 400]", "chip1_coz_seed_index");
      Check.eq(p44.vx, 0, "chip1_vx_ctmag_times_0");
      Check.eq(p44.vy, 7, "chip1_vy_0_then_plus_7");
      Check.eq(p44.chip, 3, "chip1_advances_to_3");
      Check.eq(fm.draws, 12, "chip1_twelve_draws");

      // pin 45: embos=12 -> chip/ctmag/bfase seed, then one d() runs the whole
      // primitive for the first time. With the preset recipe the :513 reset
      // check samples 0.4 (no reset) so chip ends 3, and bfase==-7 selects the
      // 10..-20 velocity arm (:550-553).
      PresetMedium e12 = new PresetMedium(6);
      Plane p45 = sample(e12);
      p45.embos = 12;
      p45.d(paint(image()), 0, 0, 0, 11, 22, 33, 0, 0, false);
      Check.eq(bits(p45.ctmag), bits(2.0F), "embos12_ctmag_2");
      Check.eq(p45.bfase, -7, "embos12_bfase_minus7");
      Check.eq(p45.embos, 13, "embos12_advances_to_13");
      Check.eq(p45.chip, 3, "embos12_preset_chip_ends_3");
      Check.eq(bits(p45.hsb[2]), bits(0.5F), "embos12_bri_capped_0_5");
      Check.eq(p45.cxz, 11, "embos12_cxz_copied");
      Check.eq(p45.cxy, 22, "embos12_cxy_copied");
      Check.eq(p45.czy, 33, "embos12_czy_copied");
      Check.eq(Arrays.toString(p45.cox), "[275, 287, 291]", "embos12_cox_preset");
      Check.eq(Arrays.toString(p45.coy), "[200, 220, 184]", "embos12_coy_preset");
      Check.eq(Arrays.toString(p45.coz), "[400, 388, 392]", "embos12_coz_preset");
      Check.eq(p45.vx, -4, "embos12_vx_preset");
      Check.eq(p45.vy, 11, "embos12_vy_preset");
      Check.eq(p45.vz, 0, "embos12_vz_preset");
      Check.eq(e12.draws, 13, "embos12_thirteen_draws");

      // pin 46: embos=13 rewrites the HSB, then falls through to a plain draw
      PresetMedium e13 = new PresetMedium(6);
      Plane p46 = sample(e13);
      p46.embos = 13;
      p46.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(bits(p46.hsb[1]), bits(0.2F), "embos13_sat_0_2");
      Check.eq(bits(p46.hsb[2]), bits(0.4F), "embos13_bri_0_4");
      Check.eq(p46.embos, 14, "embos13_advances_to_14");
      Check.eq(e13.draws, 0, "embos13_no_random_draws");

      // pin 47: 70 snaps to 16 (skipping the embos==16 pa/pb loop), 1 -> 2,
      // and 0 never increments at all (:230,:498-503)
      PresetMedium e70 = new PresetMedium(6);
      Plane p70 = sample(e70);
      p70.embos = 70;
      p70.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p70.embos, 16, "embos70_snaps_to_16");
      Check.eq(e70.draws, 3, "embos70_spark_three_draws");
      Check.eq(p70.pa + p70.pb, 0, "embos70_does_not_reroll_pa_pb");

      PresetMedium e1 = new PresetMedium(6);
      Plane p1 = sample(e1);
      p1.embos = 1;
      p1.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(p1.embos, 2, "embos1_advances_to_2");
      Check.eq(e1.draws, 2, "embos1_two_draws");

      PresetMedium e0 = new PresetMedium(6);
      Plane p0 = sample(e0);
      for (int i = 0; i < 3; i++) {
         p0.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      }
      Check.eq(p0.embos, 0, "embos0_stays_0_after_three_calls");
      Check.eq(e0.draws, 0, "embos0_zero_draws");

      // pin 48: the grey spark polygon is guarded by !glass (:237). A fade
      // override culls the body so only the spark can paint, which isolates
      // the (230,230,230) fill. non-glass consumes the 9 jitter draws too.
      PresetMedium hiM = new PresetMedium(7);
      for (int i = 0; i < 8; i++) {
         hiM.fade[i] = 100;
      }
      Plane spark = sample(hiM);
      spark.embos = 1;
      BufferedImage si = image();
      spark.d(paint(si), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(count(si, 0xE6E6E6), 35, "embos1_grey_spark_pixels_35");
      Check.eq(hiM.draws, 11, "embos1_grey_spark_eleven_draws");

      PresetMedium hiG = new PresetMedium(7);
      for (int i = 0; i < 8; i++) {
         hiG.fade[i] = 100;
      }
      Plane gspark = mk(hiG, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                        new int[]{200, 200, 210}, 3, new int[]{1, 2, 3}, true,
                        0, 0, 0, 0, 7, 0, false);
      gspark.embos = 1;
      BufferedImage gsi = image();
      gspark.d(paint(gsi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(count(gsi, 0xE6E6E6), 0, "embos1_glass_skips_grey_spark");
      Check.eq(hiG.draws, 2, "embos1_glass_skips_jitter_draws");

      // the chip block's own flex probe (:598-600) fires on cox[0]/coy[0]
      // projected below y=45 even when the body is comfortably on-screen
      Medium fx = new Medium();
      fx.flex = 5;
      Plane cfx = sample(fx);
      cfx.chip = 5;
      cfx.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(fx.flex, 0, "chip_block_resets_flex");
      Check.eq(cfx.chip, 6, "chip_block_flex_case_chip_6");
      Medium fx2 = new Medium();
      fx2.flex = 5;
      Plane cfx2 = sample(fx2);
      cfx2.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(fx2.flex, 5, "body_only_leaves_flex_5");

      // NOT pinned, by design: embos=16 with n==1 spins forever in
      // `while (pa == pb)` (:302-308) because (int)(random()*1) is always 0.
   }

   /** The remaining deterministic corners: the embos>=16 spark ball (pa/pb
    *  reroll, the >100 distance walk, the snap clamps), the c() brighter/
    *  darker arm, the ctmag -3 clamp, the sub-1.0 brightness clamp arms, the
    *  non-trk fade blend, the road clamp and the tilted-tracker shadow. */
   private static void extraArms() {
      // embos==16 rolls pa/pb with a retry while they collide (:302-309);
      // the preset's 0.5,0.4 collide at index 1, 0.3 retries to 1,0
      PresetMedium b16 = new PresetMedium(6);
      Plane ball = sample(b16);
      ball.embos = 16;
      ball.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(ball.embos, 17, "embos16_advances_to_17");
      Check.eq(ball.pa, 1, "embos16_pa_1");
      Check.eq(ball.pb, 0, "embos16_pb_retried_to_0");
      Check.eq(b16.draws, 6, "embos16_six_draws");
      Check.eq(ball.av, 400, "embos16_body_av_400");

      // far-apart pa/pb vertices enter the while (|dx|>100) / (|dz|>100)
      // 30-unit walk before the ball is projected (:345-359,:415-429)
      PresetMedium wideM = new PresetMedium(6);
      Plane wide = mk(wideM, null, new int[]{100, 400, 300}, new int[]{300, 600, 400},
                      new int[]{200, 200, 200}, 3, new int[]{200, 200, 200}, false,
                      0, 0, 0, 0, 7, 0, false);
      wide.embos = 16;
      wide.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(wide.pa, 1, "embos16_wide_pa_1");
      Check.eq(wide.pb, 0, "embos16_wide_pb_0");
      Check.eq(wide.embos, 17, "embos16_wide_advances_to_17");
      Check.eq(wideM.draws, 6, "embos16_wide_six_draws");
      Check.eq(wide.av, 450, "embos16_wide_av_450");

      // the spark colours are snap-scaled and clamped to 0..255 (:379-405,
      // :447-475); extreme snaps exercise both clamp sides
      PresetMedium hiM = new PresetMedium(6);
      hiM.snap[0] = 10000;
      hiM.snap[1] = 10000;
      hiM.snap[2] = 10000;
      Plane hi = sample(hiM);
      hi.embos = 16;
      hi.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(hi.embos, 17, "embos16_snap_hi_completes");
      Check.eq(hiM.draws, 6, "embos16_snap_hi_six_draws");

      PresetMedium loM = new PresetMedium(6);
      loM.snap[0] = -10000;
      loM.snap[1] = -10000;
      loM.snap[2] = -10000;
      Plane lo = sample(loM);
      lo.embos = 16;
      lo.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(lo.embos, 17, "embos16_snap_lo_completes");
      Check.eq(loM.draws, 6, "embos16_snap_lo_six_draws");

      // the ball's tilt sign uses |angle| after a >270 unwrap and a >90 flip
      // (:311-334); xy=280 unwraps to -80 and zy=100 flips the sign
      PresetMedium angM = new PresetMedium(6);
      Plane ang = sample(angM);
      ang.embos = 16;
      ang.d(paint(image()), 0, 0, 0, 0, 280, 100, 0, 0, false);
      Check.eq(ang.embos, 17, "embos16_angle_signs_completes");
      Check.eq(angM.draws, 6, "embos16_angle_signs_six_draws");
      Check.eq(ang.av, 0, "embos16_angle_signs_culled_av_0");

      // the other sign combination covers the |zy|>270 unwrap and the >90
      // xy flip (:316-334)
      PresetMedium ang2M = new PresetMedium(6);
      Plane ang2 = sample(ang2M);
      ang2.embos = 16;
      ang2.d(paint(image()), 0, 0, 0, 0, 100, 280, 0, 0, false);
      Check.eq(ang2.embos, 17, "embos16_zy_unwrap_completes");
      Check.eq(ang2M.draws, 6, "embos16_zy_unwrap_six_draws");

      // reversed pa/pb x and z order takes the -=30 side of both walks
      PresetMedium revM = new PresetMedium(6);
      Plane rev = mk(revM, null, new int[]{500, 100, 300}, new int[]{600, 300, 400},
                     new int[]{200, 200, 200}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      rev.embos = 16;
      rev.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(rev.embos, 17, "embos16_reversed_walk_completes");
      Check.eq(revM.draws, 6, "embos16_reversed_walk_six_draws");
      Check.eq(rev.av, 450, "embos16_reversed_walk_av_450");

      // the var48 back-face probe swaps its two reference vertices when the
      // second one projects higher (:679-683); the drawn result is unchanged
      // (same bounds, same fill), so this is a path pin not a pixel change
      Plane swap = mk(new Medium(), null, new int[]{275, 285, 275},
                      new int[]{400, 400, 400}, new int[]{210, 200, 200}, 3,
                      new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      BufferedImage swi = image();
      swap.d(paint(swi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(swap.av, 400, "d_backface_swap_av_400");
      Check.eq(pixel(swi, 277, 202), 0xC8C8C8, "d_backface_swap_fill_200");

      // chip colour roll == 0 takes new Color(c).darker() (:603-607)
      PresetMedium darkM = new PresetMedium(3); // first draw 0.2 -> var66 0
      Plane dark = sample(darkM);
      dark.chip = 5;
      dark.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(dark.chip, 6, "chip_darker_arm_completes");
      Check.eq(darkM.draws, 1, "chip_darker_arm_one_draw");

      // ctmag also clamps at the low end (:533-535)
      FixedMedium fc = new FixedMedium(0.5F);
      Plane negMag = mk(fc, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                        new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                        0, 0, 0, 0, 7, 5, false);
      negMag.chip = 1;
      negMag.ctmag = -7.0F;
      negMag.d(paint(image()), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(bits(negMag.ctmag), bits(-3.0F), "chip1_ctmag_clamped_minus3");
      Check.eq(negMag.chip, 3, "chip1_neg_ctmag_chip_3");

      // var88 = projf/deltaf + 0.3 in the noline arm: a 0.9 ratio lands in
      // the (1.0,1.27) band -> x0.86; a 0.01 ratio hits the 0.37 floor
      Plane p86 = sample(new Medium());
      p86.projf = 6.0e-7F;
      BufferedImage i86 = image();
      p86.d(paint(i86), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(i86, 277, 202), 0xACACAC, "d_var88_086_band_172");

      Plane p37 = sample(new Medium());
      p37.projf = 6.0e-9F;
      BufferedImage i37 = image();
      p37.d(paint(i37), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(i37, 277, 202), 0x4A4A4A, "d_var88_floor_037_74");

      // gr==-8 forces full brightness (:877-879)
      Plane g8 = sample(new Medium());
      g8.gr = -8;
      BufferedImage i8 = image();
      g8.d(paint(i8), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(i8, 277, 202), 0xC8C8C8, "d_gr_minus8_full_brightness_200");
      Check.eq(g8.av, 400, "d_gr_minus8_av_400");

      // the non-trk fill also blends toward cfade once per exceeded fade entry
      Medium fm = new Medium();
      fm.fade[0] = 100;
      Plane fadeP = sample(fm);
      BufferedImage fi = image();
      fadeP.d(paint(fi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(fi, 277, 202), 0xD5CDCD, "d_fill_one_fade_blend_213_205_205");
      Check.eq(fadeP.av, 400, "d_fill_fade_blend_av_400");

      // the road arm clamps each fill-10 channel at zero (:976-993)
      Medium rm = new Medium();
      rm.fade[0] = 5000;
      Plane darkRoad = mk(rm, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                          new int[]{200, 200, 210}, 3, new int[]{5, 5, 5}, false,
                          0, 0, 0, 0, 7, 0, true);
      BufferedImage dri = image();
      darkRoad.d(paint(dri), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(dri, 277, 202), 0x040404, "d_road_dark_interior_4");
      Check.eq(pixel(dri, 275, 200), 0x000000, "d_road_dark_outline_clamped_0");

      // the gr==-10 arm blends its c toward cfade with the same 8-step loop
      Medium gfm = new Medium();
      gfm.fade[0] = 100;
      Plane g10 = mk(gfm, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                     -10, 0, 0, 0, 7, 0, false);
      BufferedImage g10i = image();
      g10.d(paint(g10i), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(pixel(g10i, 277, 202), 0xC5BCBC, "d_gr10_fill_blend_197_188_188");
      Check.eq(pixel(g10i, 275, 200), 0xD5CDCD, "d_gr10_outline_blend_213_205_205");
      Check.eq(g10.av, 401, "d_gr10_blend_av_401");

      // flx snap clamps: >255 on both channels and the mixed 0/255 case
      PresetMedium fhM = new PresetMedium(3);
      fhM.snap[0] = 200;
      fhM.snap[1] = 200;
      fhM.snap[2] = 200;
      Plane fh = mk(fhM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                    new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                    0, 0, 0, 0, 7, 0, false);
      fh.flx = 1;
      BufferedImage fhi = image();
      fh.d(paint(fhi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(fhi, 275, 200), 0x00FFFF, "d_flx1_snap200_clamped_255_255");

      PresetMedium fmxM = new PresetMedium(3);
      fmxM.snap[1] = -200;
      fmxM.snap[2] = 200;
      Plane fmx = mk(fmxM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      fmx.flx = 1;
      BufferedImage fmxi = image();
      fmx.d(paint(fmxi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(fmxi, 275, 200), 0x0000FF, "d_flx1_mixed_snap_clamped_0_255");

      // the tilted-tracker height correction (:1074-1096): zy=20/xy=30 moves
      // the shadow body down and still writes no Plane/Trackers field
      Medium tiltM = new Medium();
      Trackers tiltT = matchedTracker(150, 150, 150, 150, 100);
      tiltT.xy[0] = 30;
      tiltT.zy[0] = 20;
      Plane tilt = shadow(tiltM, tiltT);
      String before = snapshot(tilt);
      BufferedImage ti = image();
      tilt.s(paint(ti), 0, 0, 0, 0, 0, 0, 0);
      Check.eq(pixel(ti, 277, 155), 0x646464, "s_tilted_tracker_fill_100");
      Check.eq(snapshot(tilt), before, "s_tilted_tracker_no_state_change");
      Check.eq(tilt.av, 0, "s_tilted_tracker_av_untouched");

      // all vertices project to the same height as vertex 0 and that height
      // is below cy: the var48 probe's second test resets the flag (:697-699)
      Plane flat = mk(new Medium(), null, new int[]{300, 275, 275},
                      new int[]{400, 400, 500}, new int[]{210, 210, 210}, 3,
                      new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      BufferedImage fli = image();
      flat.d(paint(fli), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(flat.av, 450, "d_backface_flat_av_450");
      Check.eq(pixel(fli, 277, 209), 0xC8C8C8, "d_backface_flat_fill_200");

      // a true var48 with gr!=-7 forces var88=0.32 (:873-875)
      Plane bf = mk(new Medium(), null, new int[]{290, 300, 275},
                    new int[]{500, 400, 400}, new int[]{210, 200, 200}, 3,
                    new int[]{200, 200, 200}, false, 0, 0, 0, 0, 7, 0, false);
      BufferedImage bfi = image();
      bf.d(paint(bfi), 0, 0, 0, 0, 0, 0, 0, 0, true);
      Check.eq(bf.av, 450, "d_backface_true_av_450");
      Check.eq(pixel(bfi, 290, 202), 0x404040, "d_backface_true_var88_032_64");

      // flx==3/1 snap clamp combinations (both channels, both directions)
      PresetMedium f3aM = new PresetMedium(3);
      f3aM.snap[1] = -200;
      f3aM.snap[2] = 200;
      Plane f3a = mk(f3aM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      f3a.flx = 3;
      BufferedImage f3ai = image();
      f3a.d(paint(f3ai), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(f3ai, 275, 200), 0x0000FF, "d_flx3_snap_clamped_0_255");

      PresetMedium f3bM = new PresetMedium(3);
      f3bM.snap[1] = 200;
      f3bM.snap[2] = -200;
      Plane f3b = mk(f3bM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      f3b.flx = 3;
      BufferedImage f3bi = image();
      f3b.d(paint(f3bi), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(f3bi, 275, 200), 0x00FF00, "d_flx3_snap_clamped_255_0");

      PresetMedium f1nM = new PresetMedium(3);
      f1nM.snap[2] = -200;
      Plane f1n = mk(f1nM, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                     new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                     0, 0, 0, 0, 7, 0, false);
      f1n.flx = 1;
      BufferedImage f1ni = image();
      f1n.d(paint(f1ni), 0, 0, 0, 0, 0, 0, 0, 0, false);
      Check.eq(pixel(f1ni, 275, 200), 0x00DF00, "d_flx1_blue_clamp_0_223_0");

      // NOT pinned (OPEN at B5:68): projf==0 && deltaf==0 => var88 NaN and a
      // JDK-internal getHSBColor fill. NOT pinned (OPEN at B5:69): n>3 intent.
   }

   // ---------------------------------------------------------------- helpers

   private static Plane mk(Medium m, Trackers t, int[] x, int[] z, int[] y, int n,
                           int[] c, boolean glass, int gr, int wx, int wy, int wz,
                           int disline, int bfase, boolean road) {
      return new Plane(m, t, x, z, y, n, c, glass, gr, wx, wy, wz, disline, bfase, road);
   }

   /** A plain 3-vertex plane used only for field-default reads. */
   private static Plane plain() {
      return mk(new Medium(), null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                new int[]{0, 0, 0}, 3, new int[]{1, 2, 3}, false, 0, 0, 0, 0, 7, 0, false);
   }

   /** The canonical d() fixture: identity projection at z==focus_point, a
    *  black outline, and an interior pixel at (277,202). */
   private static Plane sample(Medium m) {
      return mk(m, null, new int[]{275, 285, 275}, new int[]{400, 400, 400},
                new int[]{200, 200, 210}, 3, new int[]{200, 200, 200}, false,
                0, 0, 0, 0, 7, 0, false);
   }

   /** The canonical s() fixture: z varies 400/420/440 so the ground polygon
    *  slants (275,250),(285,248),(275,246). */
   private static Plane shadow(Medium m, Trackers t) {
      return mk(m, t, new int[]{275, 285, 275}, new int[]{400, 420, 440},
                new int[]{200, 200, 210}, 3, new int[]{10, 20, 30}, false,
                0, 0, 0, 0, 7, 0, false);
   }

   private static Plane colored(Medium m, int[] c, boolean glass) {
      return coloredB(m, c, glass, 0);
   }

   private static Plane coloredB(Medium m, int[] c, boolean glass, int bfase) {
      return mk(m, null, new int[]{0, 10, 0}, new int[]{0, 0, 10},
                new int[]{0, 0, 0}, 3, c, glass, 0, 0, 0, 0, 7, bfase, false);
   }

   private static Medium faded(Medium m, int v) {
      for (int i = 0; i < m.fade.length; i++) {
         m.fade[i] = v;
      }
      return m;
   }

   /** A tracker whose every vertex matches the shadow fixture, with the given
    *  ground height and colour. */
   private static Trackers matchedTracker(int y, int r, int g, int b, int radius) {
      Trackers t = new Trackers();
      t.nt = 1;
      t.radx[0] = radius;
      t.radz[0] = radius;
      t.x[0] = 275;
      t.z[0] = 400;
      t.y[0] = y;
      t.c[0][0] = r;
      t.c[0][1] = g;
      t.c[0][2] = b;
      return t;
   }

   private static BufferedImage image() {
      return new BufferedImage(W, H, BufferedImage.TYPE_INT_RGB);
   }

   /** Fills the frame with the BG sentinel; "nothing drawn" is then a flat
    *  array compare against a freshly painted frame. */
   private static Graphics paint(BufferedImage img) {
      Graphics g = img.createGraphics();
      g.setColor(new Color(BG));
      g.fillRect(0, 0, W, H);
      return g;
   }

   private static int pixel(BufferedImage img, int x, int y) {
      return img.getRGB(x, y) & 0xFFFFFF;
   }

   private static int[] pixels(BufferedImage img) {
      return img.getRGB(0, 0, W, H, null, 0, W);
   }

   /** A frame that had nothing drawn on it: the flat BG fill. */
   private static int[] blankPixels() {
      BufferedImage img = image();
      paint(img);
      return pixels(img);
   }

   private static int count(BufferedImage img, int rgb) {
      int[] all = pixels(img);
      int n = 0;
      for (int v : all) {
         if ((v & 0xFFFFFF) == rgb) {
            n++;
         }
      }
      return n;
   }

   private static int bits(float f) {
      return Float.floatToIntBits(f);
   }

   /** Source-declared fields only: the JaCoCo agent adds a synthetic
    *  $jacocoData field at load time, so the plain and --coverage runs would
    *  otherwise disagree on the very same source. */
   private static int declaredFieldCount(Class<?> c) {
      int n = 0;
      for (java.lang.reflect.Field f : c.getDeclaredFields()) {
         if (!f.isSynthetic()) {
            ++n;
         }
      }
      return n;
   }

   /** Every Plane field that s() could conceivably touch, as one string. */
   private static String snapshot(Plane p) {
      return Arrays.toString(p.ox) + Arrays.toString(p.oy) + Arrays.toString(p.oz)
            + "|" + p.n + p.glass + p.gr + p.disline + p.road + p.master
            + p.wx + p.wy + p.wz
            + "|" + Arrays.toString(p.c) + Arrays.toString(p.oc) + Arrays.toString(p.hsb)
            + "|" + bits(p.deltaf) + bits(p.projf) + p.av + p.bfase + p.nocol
            + "|" + p.chip + bits(p.ctmag) + p.cxz + p.cxy + p.czy
            + Arrays.toString(p.cox) + Arrays.toString(p.coz) + Arrays.toString(p.coy)
            + p.dx + p.dy + p.dz + p.vx + p.vy + p.vz
            + p.embos + p.typ + p.pa + p.pb + p.flx;
   }

   private static String trackerSnapshot(Trackers t) {
      return t.nt + "|" + Arrays.toString(t.x) + Arrays.toString(t.y) + Arrays.toString(t.z)
            + Arrays.toString(t.xy) + Arrays.toString(t.zy) + Arrays.toString(t.skd)
            + Arrays.toString(t.dam) + Arrays.deepToString(t.c)
            + Arrays.toString(t.radx) + Arrays.toString(t.radz) + Arrays.toString(t.rady);
   }

   /** Preset Medium.random() state from B5:63 / Medium.java:49-89. The first
    *  value is (start-1)/10 because cntrn is already non-zero (no refill) and
    *  each call decrements all three rand entries before trn advances to 0. */
   static class PresetMedium extends Medium {
      int draws;

      PresetMedium(int start) {
         cntrn = 20;
         rand[0] = start;
         rand[1] = start;
         rand[2] = start;
         diup[0] = false;
         diup[1] = false;
         diup[2] = false;
         trn = 2;
      }

      public float random() {
         draws++;
         return super.random();
      }
   }

   /** Constant-valued stream for the "random forced to 0.5" cases. */
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
