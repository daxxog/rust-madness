import java.awt.Event;
import java.util.Arrays;

/**
 * Pins Control - the AI brain behind every non-player car. The port must make
 * exactly the same decisions, so this class drives the real preform() through
 * every branch that does not need an asset or a device.
 *
 * DETERMINISM: the only entropy Control touches is Medium.random()
 * (Control.java:134,153,162,172,190,208,232,249,259,427,434,441,485,574,575,
 * 686,799,812,813,820,824,825,837,838,844,848,849,856,962). Like TPlane, this
 * class PRESETS the real Medium walk instead of overriding random() (this is
 * far past TestRng's JIT window - see TestRng's header):
 *
 *    cntrn=100, rand={5,5,5}, diup={false,false,false}, trn=0
 *      -> 0.4, 0.3, 0.2, 0.1, 0.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, ...
 *      (after 12 calls: cntrn=88, trn=0, rand={3,3,3})
 *      (both sides are fixture data: determinism.toml [recipes] control_preset)
 *
 * verified against Medium.java:49-89: cntrn is non-zero on entry so the
 * 9-Math.random refill never runs; every call decrements all three rand
 * entries, advances trn with a 3->0 wrap and returns rand[trn]/10. Presetting
 * cntrn to 100 (rather than 20) keeps even the longest pass (12 draws, pin 47)
 * inside the same linear walk. One pin needs a 0.1 first value and uses
 * rand={2,2,2} (the trickphase -1 arm of pin 75).
 *
 * FIXTURES: a bare Medium (preset as above), a bare Madness (im/cn/mtouch/...
 * written directly, xt==null - preform never dereferences xt), a single
 * EMPTY-BYTE ContO seam (new ContO(new byte[0], m, Trackers) leaves every field
 * at its initializer; only x/y/z/xz are read, Control.java:322,336,340,510,
 * 514,680,684,798,930, and preform writes no ContO field, so one shared
 * instance is repositioned per fixture), a bare CheckPoints and a bare
 * Trackers. The touch branch's path select needs cp.n>0 and cp.typ[0]>0 to
 * terminate (Control.java:597-615 loops on typ values; with n==0 and a low
 * skiplev it walks off the 140-entry array), so cp() below is a 1-checkpoint
 * track and cpBul() a 5-checkpoint one for the bulistc branch (which does
 * point-2 at Control.java:540 and needs point>=2).
 *
 * Pin source: notes/JAVA-BEHAVIOR/B7_control.md, re-derived line by line from
 * reference/vineflower/Control.java. Values were measured by running the
 * unmodified original on the gate's Oracle 8u40 and then re-derived from the
 * source. B7 lines the source contradicts are corrected in the class comment
 * of the relevant method and reported to the B7 doc:
 *   - fpnt lives on Control (Control.java:16,1137), not CheckPoints;
 *   - pin 37's default fixture escapes to the TRICK branch (usebounce picks
 *     wtouch), where Control.java:882-885 turns trfix 3 -> 4 and adds 30 to
 *     statusque - the "stays 3" row needs the touch branch;
 *   - pin 40's randtcnt==-1 (Control.java:666) is overwritten to 0 by
 *     Control.java:686 in the same pass whenever turncnt >= 0;
 *   - pin 45's special override survives only when BOTH fpnt targets are
 *     >=2000 away (Control.java:660-671 would otherwise hit);
 *   - pin 68 ends at swat==1, not 0: the saftey block zeroes it (Control.java:
 *     940) and the same call's trickfase==2 block advances 0 -> 1 (:945-954);
 *   - pin 75 cannot derive trickprf from power: the plan block only runs when
 *     Madness.mtouch is true (:72) while the trick block needs var20 false
 *     (:498), so trickprf must be preset (and y is ContO.y=1500).
 *
 * Coverage note: this class drives 99.1% of Control's instructions and 89.9%
 * of its branches (JaCoCo). The seven lines it cannot reach are structurally
 * dead for the ORIGINAL inputs: :345 (var12 < 0 is impossible - var11 is 0 or
 * 180 and Math.atan returns (-90,90), so var12 is in (0,360)); :600 (wrapping
 * the lap-count scan to index 0 re-creates the same condition that entered it,
 * i.e. only reachable by non-terminating tracks); and :816/:821/:828/:833/:840
 * (the "other side" of the second Math.random() > Math.random() pair in the
 * var26 >= 16 setup - Medium.random() is a strictly descending 10-value cycle
 * with one 0->9 wrap per 10 draws, so the wrap can only satisfy one of the two
 * pair tests at those fixed offsets; the matching arms are pinned at the
 * phase that can reach them, see T1/T2/T3b).
 *
 * GameSparker (pins 82-87): new GameSparker() is an asset-free Applet with no
 * explicit constructor - only Object/Applet/Component field initialization and
 * the declared u/exwist/view/mouses initializers run (GameSparker.java:14-26).
 * No thread is started (gamer stays null, GameSparker.java:292-298 is start()),
 * no asset is read, no xtGraphics exists and no window/peer is created. It is
 * therefore cheap and pinned here. Stop/start of focus-stealing windows is not
 * involved: an Applet is not addNotify'd.
 */
public class TControl {
   /** constants.toml [control] - the plan-block values and their pinned rows. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("control");

   /** The one empty-byte ContO seam (its .rad parser prints one error block). */
   private static ContO car;

   static void run() {
      Check.group("Control");
      K.requireKeys("saftey_default", "acuracy_lead", "statusque_trick_add",
            "trfix_trick_to", "fpnt_near_units", "trickfase_air", "trickfase_ground");
      car = new ContO(new byte[0], new Medium(), new Trackers());
      presetRecipe();
      defaults();
      pyPins();
      flagPins();
      resetPins();
      gatePins();
      planPins();
      turnPins();
      trfixPins();
      fixPins();
      bulistcPins();
      attackPins();
      pathPins();
      actPins();
      steerPins();
      wallPins();
      trickPins();
      trickExtraPins();
      cornerPins();
      errorPins();
      inputPins();
   }

   // ------------------------------------------------------------- determinism

   /** The B7 seam recipe, pinned so every expectation below has a documented
    *  value stream. Medium.java:49-89 with cntrn already non-zero. Both sides
    *  (the recipe that sets the Medium up and the walk it must produce) come
    *  from tests/fixtures/toml/determinism.toml [recipes] [control_preset]. */
   private static void presetRecipe() {
      float[] want = RECIPE.floatArr("walk");
      Preset p = new Preset();
      for (int i = 0; i < want.length; i++) {
         Check.eq(bits(p.random()), bits(want[i]), "preset_value" + i);
      }
      Check.eq(p.draws, want.length, "preset_twelve_draws");
      Check.eq(p.cntrn, RECIPE.intv("walk_cntrn"), "preset_cntrn_100_minus_12");
      Check.eq(p.trn, RECIPE.intv("walk_trn"), "preset_trn_wraps_back_to_0");
      Check.eq(Arrays.toString(p.rand), Arrays.toString(RECIPE.intArr("walk_rand")),
            "preset_rand_after_12");
      // the trickphase -1 arm starts 0.1 <= trickprf/3
      Preset lo = new Preset(RECIPE.intv("alt_rand"));
      Check.eq(bits(lo.random()), bits(RECIPE.floatv("alt_first")), "preset_rand2_first_0_1");
      // shape pins: the rand recipe is a triple and the walk is one period
      Check.eq(RECIPE.intArr("rand").length, 3, "preset_rand_triple_len_3");
      Check.eq(want.length, 12, "preset_walk_len_12");
   }

   // ---------------------------------------------------- defaults and ctor

   /** B7(1,2) / Control.java:2-63,1159-1161 - the 62 declared fields and what
    *  a fresh Control leaves at its initializer. The ctor assigns ONLY m. */
   private static void defaults() {
      Check.eq(declaredFieldCount(Control.class), 62, "field_count_62");

      Control c = new Control(null);
      Check.ok(c.m == null, "default_m_null");
      Check.ok(!c.left, "default_left_false");
      Check.ok(!c.right, "default_right_false");
      Check.ok(!c.up, "default_up_false");
      Check.ok(!c.down, "default_down_false");
      Check.ok(!c.handb, "default_handb_false");
      Check.ok(!c.enter, "default_enter_false");
      Check.ok(!c.arrace, "default_arrace_false");
      Check.ok(!c.mutem, "default_mutem_false");
      Check.ok(!c.mutes, "default_mutes_false");
      Check.eq(c.pan, 0, "default_pan_0");
      Check.eq(c.attack, 0, "default_attack_0");
      Check.eq(c.acr, 0, "default_acr_0");
      Check.ok(!c.afta, "default_afta_false");
      Check.eq(Arrays.toString(c.fpnt), "[0, 0, 0, 0, 0]", "default_fpnt_zero");
      Check.eq(c.fpnt.length, 5, "default_fpnt_len5");
      Check.eq(c.trfix, 0, "default_trfix_0");
      Check.ok(!c.forget, "default_forget_false");
      Check.ok(!c.bulistc, "default_bulistc_false");
      Check.eq(c.acuracy, 0, "default_acuracy_0");
      Check.eq(c.upwait, 0, "default_upwait_0");
      Check.ok(!c.agressed, "default_agressed_false");
      Check.eq(bits(c.skiplev), bits(K.floatv("skiplev_default")), "default_skiplev_1");
      Check.eq(c.clrnce, K.intv("clrnce_default"), "default_clrnce_5");
      Check.eq(c.rampp, 0, "default_rampp_0");
      Check.eq(c.turntyp, 0, "default_turntyp_0");
      Check.eq(c.saftey, K.intv("saftey_default"), "default_saftey_30");
      Check.ok(!c.perfection, "default_perfection_false");
      Check.eq(bits(c.mustland), bits(K.floatv("mustland_default")), "default_mustland_0_5");
      Check.ok(!c.usebounce, "default_usebounce_false");
      Check.eq(bits(c.trickprf), bits(K.floatv("trickprf_default")), "default_trickprf_0_5");
      Check.ok(!c.zyinv, "default_zyinv_false");
      Check.ok(!c.lastl, "default_lastl_false");
      Check.ok(!c.wlastl, "default_wlastl_false");
      Check.eq(c.hold, 0, "default_hold_0");
      Check.eq(c.wall, -1, "default_wall_minus1");
      Check.eq(c.lwall, -1, "default_lwall_minus1");
      Check.eq(c.stcnt, 0, "default_stcnt_0");
      Check.eq(c.statusque, 0, "default_statusque_0");
      Check.eq(c.turncnt, 0, "default_turncnt_0");
      Check.eq(c.randtcnt, 0, "default_randtcnt_0");
      Check.eq(c.upcnt, 0, "default_upcnt_0");
      Check.eq(c.trickfase, 0, "default_trickfase_0");
      Check.eq(c.swat, 0, "default_swat_0");
      Check.ok(!c.udcomp, "default_udcomp_false");
      Check.ok(!c.lrcomp, "default_lrcomp_false");
      Check.ok(!c.udbare, "default_udbare_false");
      Check.ok(!c.lrbare, "default_lrbare_false");
      Check.ok(!c.onceu, "default_onceu_false");
      Check.ok(!c.onced, "default_onced_false");
      Check.ok(!c.oncel, "default_oncel_false");
      Check.ok(!c.oncer, "default_oncer_false");
      Check.eq(c.lrdirect, 0, "default_lrdirect_0");
      Check.eq(c.uddirect, 0, "default_uddirect_0");
      Check.eq(c.lrstart, 0, "default_lrstart_0");
      Check.eq(c.udstart, 0, "default_udstart_0");
      Check.eq(c.oxy, 0, "default_oxy_0");
      Check.eq(c.ozy, 0, "default_ozy_0");
      Check.eq(c.flycnt, 0, "default_flycnt_0");
      Check.ok(!c.lrswt, "default_lrswt_false");
      Check.ok(!c.udswt, "default_udswt_false");
      Check.eq(c.actwait, 0, "default_actwait_0");
      Check.eq(c.cntrn, 0, "default_cntrn_0");

      // B7(2): the reference is stored verbatim and nothing else changes
      Medium mm = new Medium();
      Control c2 = new Control(mm);
      Check.ok(c2.m == mm, "ctor_m_reference_identity");
      Check.eq(snapshot(c2), snapshot(c), "ctor_other_fields_identical_to_null_ctor");
      Check.ok(c.fpnt != c2.fpnt, "ctor_fpnt_is_per_instance");
   }

   /** B7(3) / Control.java:1175-1177 - py reads no field and wraps as int. */
   private static void pyPins() {
      Control c = new Control(null);
      Check.eq(c.py(3, 1, 5, 2), 13, "py_3_1_5_2_13");
      Check.eq(c.py(-1, 0, -1, 0), 2, "py_neg1_0_neg1_0_2");
      Check.eq(c.py(0, 100000, 0, 0), 1410065408, "py_1e10_int_wrap_1410065408");
      Check.eq(c.py(1, 1, 1, 1), 0, "py_same_point_0");
   }

   // ------------------------------------------------------------ flag clearing

   /** B7(4) / Control.java:1163-1173 - falseo clears exactly the 9 UI flags and
    *  writes nothing else. */
   private static void flagPins() {
      Control c = new Control(null);
      c.left = c.right = c.up = c.down = c.handb = true;
      c.enter = c.arrace = c.mutem = c.mutes = true;
      c.pan = 1;
      c.attack = 2;
      c.acr = 3;
      c.trfix = 2;
      c.wall = 7;
      c.zyinv = true;
      c.hold = 3;
      c.upcnt = 4;
      c.acuracy = 5;
      c.trickfase = K.intv("trickfase_ground");
      String before = snapshot(c);
      c.falseo();
      Check.ok(!c.left, "falseo_left_false");
      Check.ok(!c.right, "falseo_right_false");
      Check.ok(!c.up, "falseo_up_false");
      Check.ok(!c.down, "falseo_down_false");
      Check.ok(!c.handb, "falseo_handb_false");
      Check.ok(!c.enter, "falseo_enter_false");
      Check.ok(!c.arrace, "falseo_arrace_false");
      Check.ok(!c.mutem, "falseo_mutem_false");
      Check.ok(!c.mutes, "falseo_mutes_false");
      Check.ok(c.zyinv, "falseo_keeps_zyinv");
      Check.eq(c.wall, 7, "falseo_keeps_wall_7");
      Check.eq(c.attack, 2, "falseo_keeps_attack_2");
      Check.eq(c.trfix, 2, "falseo_keeps_trfix_2");
      Check.eq(c.hold, 3, "falseo_keeps_hold_3");
      Check.eq(c.upcnt, 4, "falseo_keeps_upcnt_4");
      Check.eq(c.acuracy, 5, "falseo_keeps_acuracy_5");
      Check.eq(c.trickfase, K.intv("trickfase_ground"), "falseo_keeps_trickfase_2");
      Check.eq(c.pan + c.acr, 4, "falseo_keeps_pan_acr");
      // only the nine flag characters differ from the pre-call snapshot
      Check.ok(!before.equals(snapshot(c)), "falseo_changed_something");

      // B7(5) / :66-70 - the five drive flags are cleared before any gate, even
      // on a pass that immediately falls through to the trick block.
      Control c5 = new Control(null);
      c5.left = c5.right = c5.up = c5.down = c5.handb = true;
      c5.enter = true;
      c5.arrace = true;
      Madness md5 = mad(null, 0);
      md5.mtouch = false;
      md5.wtouch = false;
      carAt(0, 0, 0, 0);
      c5.preform(md5, car, cp(), new Trackers());
      Check.ok(!c5.left && !c5.right && !c5.up && !c5.down && !c5.handb,
               "preform_entry_clears_five_flags");
      Check.ok(c5.enter, "preform_entry_keeps_enter");
      Check.ok(c5.arrace, "preform_entry_keeps_arrace");

      // B7(6) / :71-72 - dest==true returns immediately; m is never touched
      Control c6 = new Control(null);
      c6.left = c6.right = c6.up = c6.down = c6.handb = true;
      c6.enter = true;
      c6.arrace = true;
      c6.attack = 40;
      c6.wall = 7;
      Madness md6 = mad(null, 0);
      md6.dest = true;
      c6.preform(md6, car, cp(), new Trackers());
      Check.ok(!c6.left && !c6.right && !c6.up && !c6.down && !c6.handb,
               "dest_true_five_flags_false");
      Check.ok(c6.enter, "dest_true_keeps_enter");
      Check.ok(c6.arrace, "dest_true_keeps_arrace");
      Check.eq(c6.attack, 40, "dest_true_keeps_attack_40");
      Check.eq(c6.wall, 7, "dest_true_keeps_wall_7");
   }

   // -------------------------------------------------------------- reset/fpnt

   /** B7(7-13) / Control.java:1120-1157 - the clear set, the untouched set and
    *  the nearest-fixpoint walk. */
   private static void resetPins() {
      // B7(7): reset(new CheckPoints()) clears exactly its 14 fields...
      Control c = new Control(null);
      c.pan = 9;
      c.attack = 9;
      c.acr = 9;
      c.afta = true;
      c.trfix = 2;
      c.acuracy = 9;
      c.upwait = 9;
      c.forget = true;
      c.bulistc = true;
      c.left = c.right = c.up = c.down = c.handb = true;
      c.arrace = c.mutem = c.mutes = true;
      c.enter = true;
      c.zyinv = true;
      c.upcnt = 9;
      c.trickfase = K.intv("trickfase_ground");
      c.wall = 3;
      c.hold = 4;
      c.skiplev = 0.3F;
      c.clrnce = 9;
      c.rampp = 2;
      c.turntyp = 3;
      c.saftey = 4;
      c.perfection = true;
      c.mustland = 0.2F;
      c.usebounce = true;
      c.trickprf = 0.1F;
      c.agressed = true;
      c.stcnt = 4;
      c.statusque = 5;
      c.turncnt = 6;
      c.randtcnt = 7;
      c.swat = 8;
      c.lrdirect = 1;
      c.uddirect = -1;
      c.lrstart = 2;
      c.udstart = 3;
      c.oxy = 4;
      c.ozy = 5;
      c.flycnt = 6;
      c.lrswt = true;
      c.udswt = true;
      c.actwait = 8;
      c.cntrn = 9;
      c.lastl = true;
      c.wlastl = true;
      c.lwall = 2;
      c.reset(new CheckPoints());
      Check.eq(c.pan, 0, "reset_pan_0");
      Check.eq(c.attack, 0, "reset_attack_0");
      Check.eq(c.acr, 0, "reset_acr_0");
      Check.ok(!c.afta, "reset_afta_false");
      Check.eq(c.trfix, 0, "reset_trfix_0");
      Check.eq(c.acuracy, 0, "reset_acuracy_0");
      Check.eq(c.upwait, 0, "reset_upwait_0");
      Check.ok(!c.forget, "reset_forget_false");
      Check.ok(!c.bulistc, "reset_bulistc_false");
      Check.ok(!c.left && !c.right && !c.up && !c.down && !c.handb,
               "reset_five_drive_flags_false");
      Check.ok(!c.arrace && !c.mutem && !c.mutes, "reset_three_ui_flags_false");
      Check.ok(c.enter, "reset_keeps_enter");
      Check.ok(c.zyinv, "reset_keeps_zyinv");
      Check.eq(c.upcnt, 9, "reset_keeps_upcnt");
      Check.eq(c.trickfase, K.intv("trickfase_ground"), "reset_keeps_trickfase");
      Check.eq(c.wall, 3, "reset_keeps_wall");
      Check.eq(c.hold, 4, "reset_keeps_hold");
      Check.eq(bits(c.skiplev), bits(0.3F), "reset_keeps_skiplev");
      Check.eq(c.clrnce, 9, "reset_keeps_clrnce");
      Check.eq(c.rampp, 2, "reset_keeps_rampp");
      Check.eq(c.turntyp, 3, "reset_keeps_turntyp");
      Check.eq(c.saftey, 4, "reset_keeps_saftey");
      Check.ok(c.perfection, "reset_keeps_perfection");
      Check.eq(bits(c.mustland), bits(0.2F), "reset_keeps_mustland");
      Check.ok(c.usebounce, "reset_keeps_usebounce");
      Check.eq(bits(c.trickprf), bits(0.1F), "reset_keeps_trickprf");
      Check.ok(c.agressed, "reset_keeps_agressed");
      Check.eq(c.stcnt, 4, "reset_keeps_stcnt");
      Check.eq(c.statusque, 5, "reset_keeps_statusque");
      Check.eq(c.turncnt, 6, "reset_keeps_turncnt");
      Check.eq(c.randtcnt, 7, "reset_keeps_randtcnt");
      Check.eq(c.swat, 8, "reset_keeps_swat");
      Check.eq(c.lrdirect, 1, "reset_keeps_lrdirect");
      Check.eq(c.uddirect, -1, "reset_keeps_uddirect");
      Check.eq(c.lrstart, 2, "reset_keeps_lrstart");
      Check.eq(c.udstart, 3, "reset_keeps_udstart");
      Check.eq(c.oxy, 4, "reset_keeps_oxy");
      Check.eq(c.ozy, 5, "reset_keeps_ozy");
      Check.eq(c.flycnt, 6, "reset_keeps_flycnt");
      Check.ok(c.lrswt && c.udswt, "reset_keeps_swt_flags");
      Check.eq(c.actwait, 8, "reset_keeps_actwait");
      Check.eq(c.cntrn, 9, "reset_keeps_cntrn");
      Check.ok(c.lastl && c.wlastl, "reset_keeps_lastl_flags");
      Check.eq(c.lwall, 2, "reset_keeps_lwall");

      // B7(8): fn==0 means the fpnt loops never run, so fpnt survives
      Control c8 = new Control(null);
      c8.fpnt[2] = 77;
      c8.reset(new CheckPoints());
      Check.eq(Arrays.toString(c8.fpnt), "[0, 0, 77, 0, 0]", "reset_fn0_fpnt_survives");

      // B7(9): nearest 0 -> 0 - 4 + nsp(10) == 6
      CheckPoints p9 = new CheckPoints();
      p9.fn = 1;
      p9.n = 1;
      p9.nsp = 10;
      Control c9 = new Control(null);
      c9.reset(p9);
      Check.eq(c9.fpnt[0], 6, "reset_nearest_0_minus4_plus10_6");

      // B7(10): nearest index 1 -> 1 - 4 + 20 == 17
      CheckPoints p10 = new CheckPoints();
      p10.fn = 1;
      p10.n = 3;
      p10.nsp = 20;
      p10.fx[0] = 1000;
      p10.x[1] = 500;
      p10.x[2] = 2000;
      Control c10 = new Control(null);
      c10.reset(p10);
      Check.eq(c10.fpnt[0], 17, "reset_nearest_index1_minus4_plus20_17");

      // B7(11): nsp==0 does not wrap the negative
      CheckPoints p11 = new CheckPoints();
      p11.fn = 1;
      p11.n = 1;
      p11.nsp = 0;
      Control c11 = new Control(null);
      c11.reset(p11);
      Check.eq(c11.fpnt[0], -4, "reset_nsp0_no_wrap_minus4");

      // B7(12): equal distances keep the first index (strict < on later passes)
      CheckPoints p12a = new CheckPoints();
      p12a.fn = 1;
      p12a.n = 2;
      p12a.nsp = 10;
      p12a.x[0] = -100;
      p12a.x[1] = 100;
      Control c12a = new Control(null);
      c12a.reset(p12a);
      Check.eq(c12a.fpnt[0], 6, "reset_equal_distance_keeps_first_index");
      CheckPoints p12b = new CheckPoints();
      p12b.fn = 1;
      p12b.n = 2;
      p12b.nsp = 10;
      p12b.x[0] = 300;
      p12b.x[1] = 100;
      Control c12b = new Control(null);
      c12b.reset(p12b);
      Check.eq(c12b.fpnt[0], 7, "reset_later_nearer_point_wins_7");

      // B7(13): n==0 runs no inner iteration, so the stale fpnt[0]==0 is mapped
      CheckPoints p13 = new CheckPoints();
      p13.fn = 1;
      p13.n = 0;
      p13.nsp = 10;
      Control c13 = new Control(null);
      c13.reset(p13);
      Check.eq(c13.fpnt[0], 6, "reset_n0_stale_fpnt_maps_to_6");
   }

   // ---------------------------------------------------------------- AI gates

   /** B7(14-17) / Control.java:71-73,484-498,796-880 - the m==null-safe trick
    *  pass, usebounce's wtouch-over-mtouch selection and the plan gate. */
   private static void gatePins() {
      // B7(14): no plan, no touch, all scy zero -> trickfase -1, afta true, and
      // no Medium is ever touched (m==null survives the whole pass).
      Madness md = mad(null, 0);
      md.dest = false;
      md.mtouch = false;
      md.wtouch = false;
      md.power = 75.0F;
      Control c = new Control(null);
      c.usebounce = false;
      carAt(0, 0, 0, 0);
      c.preform(md, car, cp(), new Trackers());
      Check.eq(c.trickfase, K.intv("trickfase_none"), "mnull_pass_trickfase_minus1");
      Check.ok(c.afta, "mnull_pass_afta_true");
      Check.ok(!c.left && !c.right && !c.up && !c.down && !c.handb,
               "mnull_pass_five_flags_false");

      // B7(15a): mtouch=true but usebounce selects wtouch, which is false -
      // identical outcome, still no Medium needed
      Madness md2 = mad(null, 0);
      md2.mtouch = true;
      md2.wtouch = false;
      Control c2 = new Control(null);
      c2.usebounce = true;
      c2.preform(md2, car, cp(), new Trackers());
      Check.eq(c2.trickfase, K.intv("trickfase_none"), "usebounce_selects_wtouch_trickfase_minus1");
      Check.ok(c2.afta, "usebounce_selects_wtouch_afta_true");
      Check.eq(c2.stcnt, 1, "usebounce_selects_wtouch_plan_skipped_stcnt1");

      // B7(15b): usebounce=false + mtouch=true runs the touch branch: trickfase
      // stays 0 and afta stays false (two random draws, :574-575)
      Preset pm = new Preset();
      Madness md3 = mad(pm, 0);
      md3.mtouch = true;
      Control c3 = new Control(pm);
      c3.trickfase = 5;
      c3.usebounce = false;
      c3.preform(md3, car, cp(), new Trackers());
      Check.eq(c3.trickfase, 0, "touch_branch_resets_trickfase_to_0");
      Check.ok(!c3.afta, "touch_branch_afta_false");
      Check.eq(pm.draws, 2, "touch_branch_two_random_draws");

      // B7(16): the plan gate is strict - stcnt==statusque skips and increments
      Preset pm4 = new Preset();
      Madness md4 = mad(pm4, 0);
      md4.mtouch = true;
      Control c4 = new Control(pm4);
      c4.stcnt = 3;
      c4.statusque = 3;
      c4.acuracy = 7;
      c4.upwait = 8;
      c4.skiplev = 0.35F;
      c4.preform(md4, car, cp(), new Trackers());
      Check.eq(c4.stcnt, 4, "plan_gate_equal_skips_stcnt4");
      Check.eq(c4.acuracy, 7, "plan_gate_equal_keeps_acuracy");
      Check.eq(c4.upwait, 8, "plan_gate_equal_keeps_upwait");
      Check.eq(bits(c4.skiplev), bits(0.35F), "plan_gate_equal_keeps_skiplev");

      // B7(17): stcnt>statusque runs the plan and leaves stcnt==0 (:484)
      Preset pm5 = new Preset();
      Madness md5 = mad(pm5, 0);
      md5.mtouch = true;
      Control c5 = new Control(pm5);
      c5.stcnt = 4;
      c5.statusque = 3;
      c5.preform(md5, car, cp(), new Trackers());
      Check.eq(c5.stcnt, 0, "plan_gate_open_stcnt0");
   }

   // ------------------------------------------------------------ plan latents

   /** B7(18-30) / Control.java:74-275 - the deterministic plan formulas. */
   private static void planPins() {
      // B7(18): cntrn==0 is consumed at :151-188 - 5, then 4 on the next pass
      Preset m = new Preset();
      Madness md = mad(m, 0);
      md.mtouch = true;
      Control c = new Control(m);
      c.stcnt = 1;
      c.statusque = 0;
      c.preform(md, car, cp(), new Trackers());
      Check.eq(c.cntrn, 5, "plan_cntrn_first_pass_5");
      // re-open the gate by hand: the pass itself reset stcnt to 0
      c.stcnt = 1;
      c.statusque = 0;
      c.preform(md, car, cp(), new Trackers());
      Check.eq(c.cntrn, 4, "plan_cntrn_second_pass_4");
      Check.eq(m.draws, 20, "plan_two_passes_ten_draws_each");

      // B7(19): default cp, im=0, stage 1 -> (7-4)*4*(6-2)==48,
      // upwait floored to 20 by :101-103, skiplev 1.0-0.1
      Preset m19 = new Preset();
      Madness md19 = mad(m19, 0);
      Control c19 = new Control(m19);
      plan(md19, c19, cp(), new Trackers());
      Check.eq(c19.acuracy, K.intv("acuracy_lead"), "plan_default_acuracy_48");
      Check.eq(c19.upwait, K.intv("upwait_lead"), "plan_default_upwait_floor_20");
      Check.eq(bits(c19.skiplev), bits(0.9F), "plan_default_skiplev_0_9");
      Check.eq(c19.cntrn, 5, "plan_default_cntrn_5");
      Check.eq(m19.draws, 8, "plan_default_eight_draws");
      // usebounce picked the trick branch on this pass (0.9 > 75/100)
      Check.eq(c19.trickfase, K.intv("trickfase_none"), "plan_default_escapes_to_trick_branch");

      // B7(20): stage 2, pos 4/1, im 1 -> (7-1)*4*(6-4)==48; 3^3*1.5 == 40.5
      // truncated to 40
      Preset m20 = new Preset();
      Madness md20 = mad(m20, 1);
      CheckPoints p20 = cp();
      p20.stage = 2;
      p20.pos[0] = 4;
      p20.pos[1] = 1;
      Control c20 = new Control(m20);
      plan(md20, c20, p20, new Trackers());
      Check.eq(c20.acuracy, K.intv("acuracy_lead"), "plan_stage2_pos4_1_acuracy_48");
      Check.eq(c20.upwait, K.intv("upwait_stage2"), "plan_stage2_pos4_1_upwait_40");

      // B7(21): stage 2, pos equal -> 24 and no stage-1 floor
      Preset m21 = new Preset();
      Madness md21 = mad(m21, 2);
      CheckPoints p21 = cp();
      p21.stage = 2;
      p21.pos[0] = 3;
      p21.pos[2] = 3;
      Control c21 = new Control(m21);
      plan(md21, c21, p21, new Trackers());
      Check.eq(c21.acuracy, K.intv("acuracy_stage2"), "plan_stage2_pos3_3_acuracy_24");
      Check.eq(c21.upwait, 0, "plan_stage2_pos3_3_upwait_0");

      // B7(22): behind by 4 -> skiplev jumps +0.2 but is capped by stage:
      // stage 3 cap default 0.0, stage 4 cap 0.5, stage 1 cap 1.0
      for (int[] st : new int[][]{{3, 0}, {4, 1}, {1, 2}}) {
         int stage = st[0];
         float want = stage == 3 ? 0.0F : (stage == 4 ? 0.5F : 1.0F);
         Preset mm = new Preset();
         Madness mdd = mad(mm, 1);
         CheckPoints pp = cp();
         pp.stage = stage;
         pp.pos[0] = 4;
         pp.pos[1] = 0;
         Control cc = new Control(mm);
         plan(mdd, cc, pp, new Trackers());
         Check.eq(bits(cc.skiplev), bits(want), "plan_behind_stage" + stage + "_skiplev_cap");
      }

      // B7(23): power==98 -> rampp -1 and saftey 0 ((98-98)/2 * anything)
      Preset m23 = new Preset();
      Madness md23 = mad(m23, 0);
      md23.power = 98.0F;
      Control c23 = new Control(m23);
      plan(md23, c23, cp(), new Trackers());
      Check.eq(c23.rampp, -1, "plan_power98_rampp_minus1");
      Check.eq(c23.saftey, 0, "plan_power98_saftey_0");
      Check.ok(!c23.usebounce, "plan_power98_usebounce_false");

      // B7(24): power<60 forces rampp 1 (:143) and trickprf -1 (:233)
      Preset m24 = new Preset();
      Madness md24 = mad(m24, 0);
      md24.power = 59.0F;
      CheckPoints p24 = cp();
      p24.stage = 2;
      Control c24 = new Control(m24);
      plan(md24, c24, p24, new Trackers());
      Check.eq(c24.rampp, 1, "plan_power59_rampp1_not_stage6");
      Check.eq(bits(c24.trickprf), bits(-1.0F), "plan_power59_trickprf_minus1");

      // B7(25): stage 11 forces trickprf -1 regardless of power
      Preset m25 = new Preset();
      Madness md25 = mad(m25, 0);
      CheckPoints p25 = cp();
      p25.stage = 11;
      Control c25 = new Control(m25);
      plan(md25, c25, p25, new Trackers());
      Check.eq(bits(c25.trickprf), bits(-1.0F), "plan_stage11_trickprf_minus1");

      // B7(26): stage 6 -> rampp 2; power 98 (trickprf 1.2-r/2 >= 0.75) is
      // capped to 0.5 by :237-239
      Preset m26 = new Preset();
      Madness md26 = mad(m26, 0);
      md26.power = 98.0F;
      CheckPoints p26 = cp();
      p26.stage = 6;
      Control c26 = new Control(m26);
      plan(md26, c26, p26, new Trackers());
      Check.eq(c26.rampp, 2, "plan_stage6_rampp2");
      Check.eq(bits(c26.trickprf), bits(0.5F), "plan_stage6_power98_trickprf_cap_0_5");
      Check.ok(!c26.usebounce, "plan_stage6_power98_usebounce_false");

      // B7(27): perfection is forced on stages 6/8/9/10/11 and 3@im4 even with
      // hitmag==0; stage 1/2/3/5/7 stay at the random arm (false here).
      for (int stage : new int[]{6, 8, 9, 10, 11}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         CheckPoints pp = cp();
         pp.stage = stage;
         Control cc = new Control(mm);
         plan(mdd, cc, pp, new Trackers());
         Check.ok(cc.perfection, "plan_stage" + stage + "_perfection_true");
      }
      Preset m27 = new Preset();
      Madness md27 = mad(m27, 4);
      CheckPoints p27 = cp();
      p27.stage = 3;
      Control c27 = new Control(m27);
      plan(md27, c27, p27, new Trackers());
      Check.ok(c27.perfection, "plan_stage3_im4_perfection_true");
      for (int stage : new int[]{1, 2, 3, 5, 7}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         CheckPoints pp = cp();
         pp.stage = stage;
         Control cc = new Control(mm);
         plan(mdd, cc, pp, new Trackers());
         Check.ok(!cc.perfection, "plan_stage" + stage + "_perfection_random_false");
      }

      // B7(28): 70% of maxmag forces perfection (>60), 50% does not; exactly
      // 60 is not >60 (and the 7th draw 0.8 > 0.5 leaves the random arm false)
      for (int[] hm : new int[][]{{2450, 1}, {1750, 0}, {2100, 0}}) {
         int hit = hm[0];
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         mdd.hitmag = hit;
         Control cc = new Control(mm);
         plan(mdd, cc, cp(), new Trackers());
         if (hm[1] == 1) {
            Check.ok(cc.perfection, "plan_hitmag" + hit + "_perfection_true");
         } else {
            Check.ok(!cc.perfection, "plan_hitmag" + hit + "_perfection_false");
         }
      }

      // B7(29): stage 9 disables usebounce whatever the random draw was
      Preset m29 = new Preset();
      Madness md29 = mad(m29, 0);
      CheckPoints p29 = cp();
      p29.stage = 9;
      Control c29 = new Control(m29);
      plan(md29, c29, p29, new Trackers());
      Check.ok(!c29.usebounce, "plan_stage9_usebounce_false");

      // B7(30): power>50 with pos[0]-pos[im] <= 0 zeroes mustland exactly;
      // behind instead multiplies saftey (case 2 -> 6*5 == 30) and keeps 0.7
      Preset m30 = new Preset();
      Madness md30 = mad(m30, 0);
      Control c30 = new Control(m30);
      plan(md30, c30, cp(), new Trackers());
      Check.eq(bits(c30.mustland), 0, "plan_mustland_zero_bits");
      Preset m30b = new Preset();
      Madness md30b = mad(m30b, 1);
      CheckPoints p30b = cp();
      p30b.pos[0] = 4;
      p30b.pos[1] = 0;
      Control c30b = new Control(m30b);
      plan(md30b, c30b, p30b, new Trackers());
      Check.eq(c30b.saftey, 30, "plan_behind_saftey_multiplied_30");
      Check.eq(bits(c30b.mustland), bits(0.7F), "plan_behind_mustland_0_7");
   }

   /** B7(31-34) / Control.java:151-188 - the cntrn-gated turntyp/agressed
    *  ladder (all fixtures start with cntrn==0). */
   private static void turnPins() {
      // B7(31): stage 10 -> turntyp 2 (and agressed from :181)
      Preset m31 = new Preset();
      Madness md31 = mad(m31, 0);
      CheckPoints p31 = cp();
      p31.stage = 10;
      Control c31 = new Control(m31);
      plan(md31, c31, p31, new Trackers());
      Check.eq(c31.turntyp, 2, "turn_stage10_turntyp2");
      Check.ok(c31.agressed, "turn_stage10_agressed_true");
      Check.eq(c31.cntrn, 5, "turn_stage10_cntrn_5");

      // B7(32): stage 6 overrides the random draw with 1 and agressed
      Preset m32 = new Preset();
      Madness md32 = mad(m32, 0);
      CheckPoints p32 = cp();
      p32.stage = 6;
      Control c32 = new Control(m32);
      plan(md32, c32, p32, new Trackers());
      Check.eq(c32.turntyp, 1, "turn_stage6_turntyp1");
      Check.ok(c32.agressed, "turn_stage6_agressed_true");

      // B7(33): stage 3 @ im 4 -> turntyp 1, agressed true while attack==0
      Preset m33 = new Preset();
      Madness md33 = mad(m33, 4);
      CheckPoints p33 = cp();
      p33.stage = 3;
      Control c33 = new Control(m33);
      plan(md33, c33, p33, new Trackers());
      Check.eq(c33.turntyp, 1, "turn_stage3_im4_turntyp1");
      Check.ok(c33.agressed, "turn_stage3_im4_agressed_true");

      // B7(34): stage 1, not behind -> :152 clears agressed and nothing sets it
      Preset m34 = new Preset();
      Madness md34 = mad(m34, 0);
      Control c34 = new Control(m34);
      c34.agressed = true;
      plan(md34, c34, cp(), new Trackers());
      Check.ok(!c34.agressed, "turn_stage1_agressed_false");
      Check.eq(c34.turntyp, 1, "turn_stage1_random_turntyp_1");
   }

   /** B7(35-38) / Control.java:449-472,503-505 - the trfix ladder, the var22
    *  overrides, the trfix==3 special path and the touch-branch attack reset. */
   private static void trfixPins() {
      // B7(35): strict >50 / >80 boundaries on maxmag[0]==3500
      for (int[] hm : new int[][]{{1750, 0}, {1751, 1}, {2800, 1}, {2835, 2}}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         mdd.hitmag = hm[0];
         Control cc = new Control(mm);
         plan(mdd, cc, cp(), new Trackers());
         Check.eq(cc.trfix, hm[1], "trfix_hitmag" + hm[0] + "_is" + hm[1]);
      }

      // B7(36): stage 6/11 (and stage 9 @ im 4) set var22, so the >80 arm is
      // skipped and trfix tops out at 1
      for (int stage : new int[]{6, 11}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         mdd.hitmag = 2835;
         CheckPoints pp = cp();
         pp.stage = stage;
         Control cc = new Control(mm);
         plan(mdd, cc, pp, new Trackers());
         Check.eq(cc.trfix, 1, "trfix_var22_stage" + stage + "_1");
      }
      Preset m36 = new Preset();
      Madness md36 = mad(m36, 4);
      md36.hitmag = 2835;
      CheckPoints p36 = cp();
      p36.stage = 9;
      Control c36 = new Control(m36);
      plan(md36, c36, p36, new Trackers());
      Check.eq(c36.trfix, 1, "trfix_var22_stage9_im4_1");

      // B7(37): preset trfix==3 takes the else path (:467-472). This fixture
      // keeps the TICK in the touch branch (power 98 -> usebounce false); the
      // default power-75 form escapes to the trick branch and is pinned below.
      Preset m37 = new Preset();
      Madness md37 = mad(m37, 0);
      md37.mtouch = true;
      md37.power = 98.0F;
      Control c37 = new Control(m37);
      c37.trfix = 3;
      c37.upwait = 9;
      c37.acuracy = 9;
      c37.skiplev = 0.3F;
      c37.clrnce = 9;
      plan(md37, c37, cp(), new Trackers());
      Check.eq(c37.trfix, 3, "trfix3_preset_stays_3");
      Check.eq(c37.upwait, 0, "trfix3_upwait_0");
      Check.eq(c37.acuracy, 0, "trfix3_acuracy_0");
      Check.eq(bits(c37.skiplev), bits(1.0F), "trfix3_skiplev_1");
      Check.eq(c37.clrnce, 2, "trfix3_clrnce_2");
      Check.eq(c37.statusque, 14, "trfix3_statusque_untouched_14");
      Check.ok(md37.nofocus, "trfix3_nofocus_true");
      Check.eq(c37.trickfase, 0, "trfix3_touch_branch_trickfase_0");
      Check.ok(!c37.afta, "trfix3_touch_branch_afta_false");

      // The B7 row's original power-75 fixture: usebounce becomes true and
      // wtouch is false, so the TRICK branch runs and :882-885 maps 3 -> 4 and
      // adds 30 to statusque.
      Preset m37b = new Preset();
      Madness md37b = mad(m37b, 0);
      md37b.mtouch = true;
      Control c37b = new Control(m37b);
      c37b.trfix = 3;
      c37b.statusque = 0;
      plan(md37b, c37b, cp(), new Trackers());
      Check.eq(c37b.trfix, 4, "trfix3_power75_escapes_to_trick_trfix4");
      Check.eq(c37b.statusque, 44, "trfix3_power75_trick_statusque_plus30");
      Check.eq(c37b.clrnce, 2, "trfix3_power75_clrnce_2");

      // B7(38): the touch branch zeroes attack when trfix is 2 or 3
      for (int tf : new int[]{2, 3}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         mdd.mtouch = true;
         Control cc = new Control(mm);
         cc.trfix = tf;
         cc.attack = 40;
         cc.preform(mdd, car, cp(), new Trackers());
         Check.eq(cc.attack, 0, "trfix" + tf + "_touch_attack_reset_0");
         Check.ok(!cc.handb, "trfix" + tf + "_touch_no_handb");
      }
   }

   /** B7(39-45) / Control.java:636-676 - the fix-point gate, the two loops and
    *  the special/bulistc override. fpnt lives on Control (Control.java:16),
    *  not on CheckPoints. */
   private static void fixPins() {
      // B7(39): missedcp!=0 with forget==false leaves the whole block skipped
      Preset m39 = new Preset();
      Madness md39 = mad(m39, 0);
      md39.mtouch = true;
      md39.missedcp = 1;
      CheckPoints p39 = cp();
      p39.fn = 1;
      Control c39 = new Control(m39);
      c39.trfix = 1;
      c39.fpnt[0] = 0;
      c39.preform(md39, car, p39, new Trackers());
      Check.eq(c39.trfix, 1, "fix_missedcp1_trfix1_unchanged");
      Check.ok(!c39.forget, "fix_missedcp1_forget_false");
      Check.ok(!md39.nofocus, "fix_missedcp1_nofocus_false");

      // B7(40): missedcp==0 hits the fn loop; every field in the hit body is
      // written. randtcnt is the corrected final value: :666 writes -1 but :686
      // immediately recomputes it from acuracy==0 (turncnt 0 > -1).
      Preset m40 = new Preset();
      Madness md40 = mad(m40, 0);
      md40.mtouch = true;
      CheckPoints p40 = cp();
      p40.fn = 1;
      Control c40 = new Control(m40);
      c40.trfix = 1;
      c40.upwait = 11;
      c40.acuracy = 12;
      c40.rampp = 3;
      c40.turntyp = 4;
      c40.randtcnt = 13;
      c40.actwait = 14;
      c40.fpnt[0] = 0;
      c40.preform(md40, car, p40, new Trackers());
      Check.eq(c40.trfix, 3, "fix_hit_trfix3");
      Check.ok(!c40.forget, "fix_hit_forget_false");
      Check.eq(c40.upwait, 0, "fix_hit_upwait_0");
      Check.eq(c40.acuracy, 0, "fix_hit_acuracy_0");
      Check.eq(c40.rampp, 0, "fix_hit_rampp_0");
      Check.eq(c40.turntyp, 2, "fix_hit_turntyp2");
      Check.eq(c40.randtcnt, 0, "fix_hit_randtcnt_overwritten_to_0");
      Check.eq(c40.actwait, 0, "fix_hit_actwait_0");
      Check.eq(c40.turncnt, 0, "fix_hit_turncnt_recomputed_0");
      Check.ok(md40.nofocus, "fix_hit_nofocus_true");

      // With a negative turncnt the :678 test fails to re-enter and the -1
      // written at :666 survives (this is the only observable form of pin 40's
      // -1; the real loop's turncnt is never negative).
      Preset m40b = new Preset();
      Madness md40b = mad(m40b, 0);
      md40b.mtouch = true;
      CheckPoints p40b = cp();
      p40b.fn = 1;
      Control c40b = new Control(m40b);
      c40b.trfix = 1;
      c40b.turncnt = -5;
      c40b.fpnt[0] = 0;
      c40b.preform(md40b, car, p40b, new Trackers());
      Check.eq(c40b.randtcnt, -1, "fix_hit_randtcnt_minus1_when_turncnt_negative");
      Check.eq(c40b.turncnt, -4, "fix_hit_turncnt_increment_neg");

      // B7(41): the py<2000 boundary (py is in /100 units)
      for (int[] xw : new int[][]{{4400, 3}, {4500, 1}}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         mdd.mtouch = true;
         CheckPoints pp = cp();
         pp.fn = 1;
         Control cc = new Control(mm);
         cc.trfix = 1;
         cc.fpnt[0] = 0;
         carAt(xw[0], 0, 0, 0);
         cc.preform(mdd, car, pp, new Trackers());
         Check.eq(cc.trfix, xw[1], "fix_py_boundary_x" + xw[0] + "_trfix" + xw[1]);
      }
      carAt(0, 0, 0, 0);

      // B7(42): stage 11 disables the fix block outright
      Preset m42 = new Preset();
      Madness md42 = mad(m42, 0);
      md42.mtouch = true;
      CheckPoints p42 = cp();
      p42.stage = 11;
      p42.fn = 1;
      Control c42 = new Control(m42);
      c42.trfix = 1;
      c42.fpnt[0] = 0;
      c42.preform(md42, car, p42, new Trackers());
      Check.eq(c42.trfix, 1, "fix_stage11_disabled_trfix1");
      Check.ok(!md42.nofocus, "fix_stage11_nofocus_false");

      // B7(43): trfix==4 passes the gate (:636) and the loop re-marks 3
      Preset m43 = new Preset();
      Madness md43 = mad(m43, 0);
      md43.mtouch = true;
      CheckPoints p43 = cp();
      p43.fn = 1;
      Control c43 = new Control(m43);
      c43.trfix = 4;
      c43.fpnt[0] = 0;
      c43.preform(md43, car, p43, new Trackers());
      Check.eq(c43.trfix, 3, "fix_trfix4_remark_3");
      Check.ok(md43.nofocus, "fix_trfix4_nofocus_true");

      // B7(44): forget==true overrides a non-zero missedcp
      Preset m44 = new Preset();
      Madness md44 = mad(m44, 0);
      md44.mtouch = true;
      md44.missedcp = 3;
      CheckPoints p44 = cp();
      p44.fn = 1;
      Control c44 = new Control(m44);
      c44.trfix = 1;
      c44.forget = true;
      c44.fpnt[0] = 0;
      c44.preform(md44, car, p44, new Trackers());
      Check.ok(!c44.forget, "fix_forget_cleared");
      Check.eq(c44.trfix, 3, "fix_forget_trfix3");
      Check.ok(md44.nofocus, "fix_forget_nofocus_true");

      // B7(45): trfix==2 nearest index is overridden to 1 on stage 9+bulistc
      // (:648-650) and special[1] sets forget. Both fpnt targets must be
      // >=2000 (/100) away or the generic loop at :660-671 clears forget again.
      Preset m45 = new Preset();
      Madness md45 = mad(m45, 0);
      md45.mtouch = true;
      CheckPoints p45 = cp();
      p45.stage = 9;
      p45.fn = 2;
      p45.special[1] = true;
      p45.x[0] = K.intv("fpnt_near_units") * 4 + 1000;
      p45.x[6] = K.intv("fpnt_near_units") * 4 + 1000;
      Control c45 = new Control(m45);
      c45.trfix = 2;
      c45.bulistc = true;
      c45.fpnt[0] = 0;
      c45.fpnt[1] = 6;
      c45.preform(md45, car, p45, new Trackers());
      Check.ok(c45.forget, "fix_special_override_forget_true");
      Check.eq(c45.trfix, 2, "fix_special_override_trfix_stays_2");
      Check.ok(c45.bulistc, "fix_special_override_bulistc_true");
      Check.ok(md45.nofocus, "fix_special_override_nofocus_true");

      // the contrast: fpnt[0] near the car makes :660-671 hit, clear forget and
      // promote trfix to 3
      Preset m45b = new Preset();
      Madness md45b = mad(m45b, 0);
      md45b.mtouch = true;
      CheckPoints p45b = cp();
      p45b.stage = 9;
      p45b.fn = 2;
      p45b.special[1] = true;
      p45b.x[0] = 0;
      p45b.x[6] = 9000;
      Control c45b = new Control(m45b);
      c45b.trfix = 2;
      c45b.bulistc = true;
      c45b.fpnt[0] = 0;
      c45b.fpnt[1] = 6;
      c45b.preform(md45b, car, p45b, new Trackers());
      Check.ok(!c45b.forget, "fix_special_override_cleared_by_near_first_point");
      Check.eq(c45b.trfix, 3, "fix_special_override_near_first_point_trfix3");
   }

   /** B7(46-48) / Control.java:474-481,631-633 - bulistc enablement and its
    *  nofocus effect. The bulistc branch subtracts 2 from point (:540), so the
    *  fixture keeps point>=2 and cp.n==5. */
   private static void bulistcPins() {
      // B7(46): stage 6 && cn 7 && bulistc false
      Preset m46 = new Preset();
      Madness md46 = mad(m46, 0);
      md46.cn = 7;
      md46.point = 2;
      CheckPoints p46 = cpBul();
      p46.stage = 6;
      Control c46 = new Control(m46);
      plan(md46, c46, p46, new Trackers());
      Check.ok(c46.bulistc, "bulistc_stage6_cn7_true");

      // B7(47): stage 9, afta, pos 4, cn!=9, trfix!=0 (1751/3500 > 50%)
      Preset m47 = new Preset();
      Madness md47 = mad(m47, 0);
      md47.hitmag = 1751;
      md47.point = 2;
      CheckPoints p47 = cpBul();
      p47.stage = 9;
      Control c47 = new Control(m47);
      c47.afta = true;
      plan(md47, c47, p47, new Trackers());
      Check.ok(c47.bulistc, "bulistc_stage9_afta_pos4_true");
      Check.eq(c47.trfix, 1, "bulistc_stage9_fixture_trfix1");

      // B7(48): bulistc + trfix 2 + skipped plan routes through :631-633
      Preset m48 = new Preset();
      Madness md48 = mad(m48, 0);
      md48.mtouch = true;
      Control c48 = new Control(m48);
      c48.bulistc = true;
      c48.trfix = 2;
      c48.preform(md48, car, cp(), new Trackers());
      Check.ok(md48.nofocus, "bulistc_trfix2_nofocus_true");
      Check.eq(c48.trfix, 2, "bulistc_trfix2_stays_2");
   }

   // ---------------------------------------------------- attack acquisition

   /** The plan's attack-target loop (Control.java:278-447): the per-checkpoint
    *  geometry gates, the var15/var16 stage modifiers and the 430 clamp. Every
    *  fixture here has afta==true so the :368 gate can pass. */
   private static void attackPins() {
      // A: stage 11 never caps var16 at 0.7 (:376). With pos 4/0 at im 0 the
      // candidate index 1 has 5*5/35 == 0.714, so the 8th draw (0.7) acquires
      // it: attack 40, acr 1, then turntyp = (int)(1 + 0.6*2) == 2. usebounce
      // stays true -> the trick branch runs, so the attack is NOT decremented.
      Preset mA = new Preset();
      Madness mdA = mad(mA, 0);
      CheckPoints pA = cp();
      pA.stage = 11;
      pA.pos[0] = 4;
      pA.pos[1] = 0;
      pA.opx[1] = 100;
      pA.opz[1] = 0;
      Control cA = new Control(mA);
      cA.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdA, cA, pA, new Trackers());
      Check.eq(cA.attack, 40, "attack_stage11_acquires_40");
      Check.eq(cA.acr, 1, "attack_stage11_acr_1");
      Check.eq(cA.turntyp, 2, "attack_stage11_turntyp_2");
      Check.eq(mA.draws, 13, "attack_stage11_thirteen_draws");
      Check.eq(cA.statusque, 4, "attack_stage11_statusque_4");

      // A2: a 15-clear difference clamps var15 to 1 (:370-373) and the reward
      // to 500 (:429-431)
      Preset mA2 = new Preset();
      Madness mdA2 = mad(mA2, 0);
      CheckPoints pA2 = cp();
      pA2.stage = 11;
      pA2.pos[0] = 4;
      pA2.pos[1] = 0;
      pA2.opx[1] = 100;
      pA2.opz[1] = 0;
      pA2.clear[1] = 14;
      Control cA2 = new Control(mA2);
      cA2.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdA2, cA2, pA2, new Trackers());
      Check.eq(cA2.attack, 500, "attack_clear14_reward_clamped_500");
      Check.eq(cA2.acr, 1, "attack_clear14_acr_1");
      Check.eq(cA2.turntyp, 2, "attack_clear14_turntyp_2");

      // A3: var9 != 0 with pos[0] < pos[im] zeroes var16 (:380-382)
      Preset mA3 = new Preset();
      Madness mdA3 = mad(mA3, 1);
      CheckPoints pA3 = cp();
      pA3.stage = 2;
      pA3.pos[0] = 2;
      pA3.pos[1] = 4;
      pA3.opx[0] = 100;
      pA3.opz[0] = 0;
      Control cA3 = new Control(mA3);
      cA3.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdA3, cA3, pA3, new Trackers());
      Check.eq(cA3.attack, 0, "attack_pos0_below_posim_no_attack");

      // B: stage 6 + bulistc turns var16 into 1.0 for index 0 (:400-405)
      Preset mB = new Preset();
      Madness mdB = mad(mB, 1);
      CheckPoints pB = cp();
      pB.stage = 6;
      pB.opx[0] = 100;
      pB.opz[0] = 0;
      Control cB = new Control(mB);
      cB.afta = true;
      cB.bulistc = true;
      carAt(0, 0, 0, 0);
      plan(mdB, cB, pB, new Trackers());
      Check.eq(cB.attack, 40, "attack_stage6_bulistc_index0_40");
      Check.eq(cB.acr, 0, "attack_stage6_bulistc_acr_0");
      Check.eq(cB.turntyp, 2, "attack_stage6_bulistc_turntyp_2");

      // C: stage 3 @ im 4 - wasted<2 zeroes index 0 (:388-393), wasted>=2
      // halves the untouched var16; var14 stays 2000 until clear differs
      for (int wasted : new int[]{1, 2}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 4);
         CheckPoints pp = cp();
         pp.stage = 3;
         pp.wasted = wasted;
         pp.opx[0] = 100;
         pp.opz[0] = 0;
         Control cc = new Control(mm);
         cc.afta = true;
         carAt(0, 0, 0, 0);
         plan(mdd, cc, pp, new Trackers());
         Check.eq(cc.attack, 0, "attack_stage3_im4_wasted" + wasted + "_no_attack");
      }
      // C2: a 5-clear gap pushes var14 over 4000, so stage 3 @ im4 caps it at
      // 4000 (:362-364); the gate still passes and wasted<2 zeroes index 0
      Preset mC2 = new Preset();
      Madness mdC2 = mad(mC2, 4);
      CheckPoints pC2 = cp();
      pC2.stage = 3;
      pC2.clear[0] = 5;
      pC2.opx[0] = 100;
      pC2.opz[0] = 0;
      Control cC2 = new Control(mC2);
      cC2.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdC2, cC2, pC2, new Trackers());
      Check.eq(cC2.attack, 0, "attack_stage3_im4_clear5_var14_capped_no_attack");

      // D: stage 8 zeroes var16 for every candidate (:407-409)
      Preset mD = new Preset();
      Madness mdD = mad(mD, 1);
      CheckPoints pD = cp();
      pD.stage = 8;
      pD.opx[0] = 100;
      pD.opz[0] = 0;
      Control cD = new Control(mD);
      cD.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdD, cD, pD, new Trackers());
      Check.eq(cD.attack, 0, "attack_stage8_no_attack");

      // E: stage 9 @ im 4 zeroes var16 (:411-413); usebounce is forced false so
      // the tick ends in the touch branch (two extra path-select draws)
      Preset mE = new Preset();
      Madness mdE = mad(mE, 4);
      CheckPoints pE = cp();
      pE.stage = 9;
      pE.opx[0] = 100;
      pE.opz[0] = 0;
      Control cE = new Control(mE);
      cE.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdE, cE, pE, new Trackers());
      Check.eq(cE.attack, 0, "attack_stage9_im4_no_attack");
      Check.eq(cE.trickfase, 0, "attack_stage9_im4_touch_branch");
      Check.eq(mE.draws, 14, "attack_stage9_im4_fourteen_draws");

      // E2: stage 9 with pos 4 zeroes every candidate via :423-425
      Preset mE2 = new Preset();
      Madness mdE2 = mad(mE2, 0);
      CheckPoints pE2 = cp();
      pE2.stage = 9;
      pE2.pos[1] = 3;
      pE2.opx[1] = 100;
      pE2.opz[1] = 0;
      Control cE2 = new Control(mE2);
      cE2.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdE2, cE2, pE2, new Trackers());
      Check.eq(cE2.attack, 0, "attack_stage9_pos3_zeroes_candidate");

      // E3: stage 9 + bulistc gives index 0 var16 1.0 (:415-421); pos[0]==2
      // avoids the :423 zeroing. usebounce is false, so the same tick's touch
      // branch decrements the freshly acquired 40 to 39 and skips nofocus.
      Preset mE3 = new Preset();
      Madness mdE3 = mad(mE3, 1);
      CheckPoints pE3 = cp();
      pE3.stage = 9;
      pE3.pos[0] = 2;
      pE3.opx[0] = 100;
      pE3.opz[0] = 0;
      Control cE3 = new Control(mE3);
      cE3.afta = true;
      cE3.bulistc = true;
      carAt(0, 0, 0, 0);
      plan(mdE3, cE3, pE3, new Trackers());
      Check.eq(cE3.attack, 39, "attack_stage9_bulistc_40_then_touch_decrement_39");
      Check.eq(cE3.acr, 0, "attack_stage9_bulistc_acr_0");
      Check.eq(cE3.turntyp, 2, "attack_stage9_bulistc_turntyp_2");
      Check.eq(mE3.draws, 14, "attack_stage9_bulistc_fourteen_draws");
      Check.ok(!mdE3.nofocus, "attack_stage9_bulistc_attack_branch_skips_nofocus");

      // G: stage 10 sets var7, so every var9 != 0 candidate gets var16 0
      Preset mG = new Preset();
      Madness mdG = mad(mG, 1);
      CheckPoints pG = cp();
      pG.stage = 10;
      pG.opx[0] = 100;
      pG.opz[0] = 0;
      Control cG = new Control(mG);
      cG.afta = true;
      carAt(0, 0, 0, 0);
      plan(mdG, cG, pG, new Trackers());
      Check.eq(cG.attack, 0, "attack_stage10_var7_no_attack");
      Check.eq(cG.turntyp, 2, "attack_stage10_turntyp_2");

      // H: var13 > 100 + near target + random > 0.6-pos/10 zeroes clrnce and
      // acuracy (:438-444). xz 90 against a -90 target gives var13 180.
      Preset mH = new Preset();
      Madness mdH = mad(mH, 0);
      CheckPoints pH = cp();
      pH.stage = 2;
      pH.pos[0] = 4;
      pH.opx[1] = 100;
      pH.opz[1] = 0;
      Control cH = new Control(mH);
      cH.afta = true;
      carAt(0, 0, 0, 90);
      plan(mdH, cH, pH, new Trackers());
      Check.eq(cH.clrnce, 0, "attack_var13_gt100_clrnce_0");
      Check.eq(cH.acuracy, 0, "attack_var13_gt100_acuracy_0");
      Check.eq(mH.draws, 12, "attack_var13_gt100_twelve_draws");

      // I: zyinv adds 180 to var10 (:323-325); xz -90 gives the same angle
      Preset mI = new Preset();
      Madness mdI = mad(mI, 0);
      CheckPoints pI = cp();
      pI.stage = 2;
      pI.pos[0] = 4;
      pI.opx[1] = 100;
      pI.opz[1] = 0;
      Control cI = new Control(mI);
      cI.afta = true;
      cI.zyinv = true;
      carAt(0, 0, 0, -90);
      plan(mdI, cI, pI, new Trackers());
      Check.eq(cI.clrnce, 0, "attack_zyinv_clrnce_0");
      Check.eq(cI.acuracy, 0, "attack_zyinv_acuracy_0");
   }

   // -------------------------------------------------------------- actuation

   /** B7(49-53) / Control.java:507-531 - the upcnt/actwait machine and the
    *  attack steering clamp. */
   private static void actPins() {
      carAt(0, 0, 0, 0);
      // B7(49): upcnt 0 < 30 -> up, then increments
      Preset m49 = new Preset();
      Madness md49 = mad(m49, 0);
      md49.mtouch = true;
      Control c49 = new Control(m49);
      c49.preform(md49, car, cp(), new Trackers());
      Check.ok(c49.up, "act_upcnt0_up_true");
      Check.eq(c49.upcnt, 1, "act_upcnt0_becomes_1");
      Check.eq(c49.actwait, 0, "act_upcnt0_actwait_0");

      // B7(50): upcnt 25 >= 25+0 -> reset and load upwait
      Preset m50 = new Preset();
      Madness md50 = mad(m50, 0);
      md50.mtouch = true;
      Control c50 = new Control(m50);
      c50.upcnt = 25;
      c50.upwait = 40;
      c50.preform(md50, car, cp(), new Trackers());
      Check.eq(c50.upcnt, 0, "act_upcnt25_reset_0");
      Check.eq(c50.actwait, 40, "act_upcnt25_actwait_upwait_40");
      Check.ok(c50.up, "act_upcnt25_up_true");

      // B7(51): opx==x and opz==z -> 0/0 NaN -> (int)NaN == 0
      Preset m51 = new Preset();
      Madness md51 = mad(m51, 0);
      md51.mtouch = true;
      CheckPoints p51 = cp();
      p51.opx[0] = 0;
      p51.opz[0] = 0;
      Control c51 = new Control(m51);
      c51.attack = 1;
      c51.acr = 0;
      c51.preform(md51, car, p51, new Trackers());
      Check.eq(c51.pan, 0, "act_nan_pan_0");
      Check.eq(c51.attack, 0, "act_nan_attack_0");
      Check.ok(c51.up, "act_nan_up_true");

      // B7(52): target 100 ahead -> pan 270 normalised to -90, right; speed 0
      // prevents the down/handb arm
      Preset m52 = new Preset();
      Madness md52 = mad(m52, 0);
      md52.mtouch = true;
      CheckPoints p52 = cp();
      p52.opx[0] = 100;
      p52.opz[0] = 0;
      Control c52 = new Control(m52);
      c52.attack = 1;
      c52.acr = 0;
      c52.turntyp = 0;
      c52.preform(md52, car, p52, new Trackers());
      Check.eq(c52.pan, -90, "act_target_right_pan_minus90");
      Check.ok(c52.right, "act_target_right_right_true");
      Check.ok(!c52.lastl, "act_target_right_lastl_false");
      Check.eq(c52.attack, 0, "act_target_right_attack_0");
      Check.ok(c52.up, "act_target_right_up_true");

      // B7(53): the decrement path clamps at 0 - 501 -> 500
      Preset m53 = new Preset();
      Madness md53 = mad(m53, 0);
      md53.mtouch = true;
      CheckPoints p53 = cp();
      p53.opx[0] = 100;
      Control c53 = new Control(m53);
      c53.attack = 501;
      c53.acr = 0;
      c53.preform(md53, car, p53, new Trackers());
      Check.eq(c53.attack, 500, "act_attack501_becomes_500");
   }

   // ---------------------------------------------------------------- steering

   /** B7(54-60) / Control.java:678-763 - pan normalisation, the clrnce dead
    *  band and the three turntyp arms. */
   private static void steerPins() {
      // B7(54): turncnt==randtcnt==0 freezes the preset pan for this tick
      Preset m54 = new Preset();
      Madness md54 = mad(m54, 0);
      md54.mtouch = true;
      Control c54 = new Control(m54);
      c54.turncnt = 0;
      c54.randtcnt = 0;
      c54.pan = 0;
      c54.clrnce = 5;
      c54.turntyp = 0;
      carAt(0, 0, 0, 90);
      c54.preform(md54, car, cp(), new Trackers());
      Check.ok(c54.right, "steer_xz90_right_true");
      Check.ok(!c54.left, "steer_xz90_left_false");
      Check.ok(!c54.lastl, "steer_xz90_lastl_false");
      Check.eq(c54.pan, 0, "steer_turncnt0_pan_frozen_0");
      Check.eq(c54.turncnt, 1, "steer_turncnt0_becomes_1");

      carAt(0, 0, 0, 90);
      // B7(55): a skipped plan leaves clrnce at 100 -> |90| > 100 is false, so
      // neither flag fires; an open plan rewrites clrnce to 5 first (:277)
      Preset m55 = new Preset();
      Madness md55 = mad(m55, 0);
      md55.mtouch = true;
      Control c55 = new Control(m55);
      c55.clrnce = 100;
      c55.pan = 0;
      c55.preform(md55, car, cp(), new Trackers());
      Check.eq(c55.clrnce, 100, "steer_skipped_plan_keeps_clrnce_100");
      Check.ok(!c55.left && !c55.right, "steer_clrnce100_dead_band");
      carAt(0, 0, 0, 90);
      Preset m55b = new Preset();
      Madness md55b = mad(m55b, 0);
      md55b.power = 98.0F; // keeps usebounce false -> touch branch
      Control c55b = new Control(m55b);
      c55b.clrnce = 100;
      c55b.pan = 0;
      plan(md55b, c55b, cp(), new Trackers());
      Check.eq(c55b.clrnce, 5, "steer_open_plan_rewrites_clrnce_5");
      Check.ok(c55b.right, "steer_open_plan_right_true");

      // B7(56): pan 270 normalises to -90; |diff|==180 takes the second branch
      carAt(0, 0, 0, 90);
      Preset m56 = new Preset();
      Madness md56 = mad(m56, 0);
      md56.mtouch = true;
      Control c56 = new Control(m56);
      c56.pan = 270;
      c56.preform(md56, car, cp(), new Trackers());
      Check.eq(c56.pan, -90, "steer_pan270_normalises_minus90");
      Check.ok(c56.left, "steer_diff180_left_true");
      Check.ok(c56.lastl, "steer_diff180_lastl_true");

      // B7(57): xz -90 turns left
      Preset m57 = new Preset();
      Madness md57 = mad(m57, 0);
      md57.mtouch = true;
      Control c57 = new Control(m57);
      carAt(0, 0, 0, -90);
      c57.preform(md57, car, cp(), new Trackers());
      Check.ok(c57.left, "steer_xzminus90_left_true");
      Check.ok(c57.lastl, "steer_xzminus90_lastl_true");

      // a preset negative pan normalises +360 before the comparison (:705-707)
      Preset mNeg = new Preset();
      Madness mdNeg = mad(mNeg, 0);
      mdNeg.mtouch = true;
      Control cNeg = new Control(mNeg);
      cNeg.pan = -200;
      cNeg.upcnt = 30;
      carAt(0, 0, 0, 0);
      cNeg.preform(mdNeg, car, cp(), new Trackers());
      Check.eq(cNeg.pan, 160, "steer_panminus200_normalises_160");
      Check.ok(cNeg.left, "steer_panminus200_left_true");
      Check.ok(!cNeg.right, "steer_panminus200_right_false");

      // B7(58): fast enough + diff>50 + turntyp 1 -> down, and up is forced
      // off when !agressed
      carAt(0, 0, 0, 90);
      Preset m58 = new Preset();
      Madness md58 = mad(m58, 0);
      md58.mtouch = true;
      md58.speed = 100.0F;
      Control c58 = new Control(m58);
      c58.turntyp = 1;
      c58.preform(md58, car, cp(), new Trackers());
      Check.ok(c58.down, "steer_turntyp1_down_true");
      Check.ok(!c58.up, "steer_turntyp1_up_forced_false");
      Check.ok(c58.right, "steer_turntyp1_right_true");

      // B7(59): turntyp 2 uses handb; turntyp 0 does neither
      carAt(0, 0, 0, 90);
      Preset m59 = new Preset();
      Madness md59 = mad(m59, 0);
      md59.mtouch = true;
      md59.speed = 100.0F;
      Control c59 = new Control(m59);
      c59.turntyp = 2;
      c59.preform(md59, car, cp(), new Trackers());
      Check.ok(c59.handb, "steer_turntyp2_handb_true");
      Check.ok(!c59.down, "steer_turntyp2_down_false");
      Check.ok(!c59.up, "steer_turntyp2_up_forced_false");
      carAt(0, 0, 0, 90);
      Preset m59b = new Preset();
      Madness md59b = mad(m59b, 0);
      md59b.mtouch = true;
      md59b.speed = 100.0F;
      Control c59b = new Control(m59b);
      c59b.turntyp = 0;
      c59b.preform(md59b, car, cp(), new Trackers());
      Check.ok(!c59b.down && !c59b.handb, "steer_turntyp0_neither");
      Check.ok(c59b.up, "steer_turntyp0_up_kept");

      // B7(60): agressed keeps the up bit
      carAt(0, 0, 0, 90);
      Preset m60 = new Preset();
      Madness md60 = mad(m60, 0);
      md60.mtouch = true;
      md60.speed = 100.0F;
      Control c60 = new Control(m60);
      c60.turntyp = 2;
      c60.agressed = true;
      c60.preform(md60, car, cp(), new Trackers());
      Check.ok(c60.handb, "steer_agressed_handb_true");
      Check.ok(c60.up, "steer_agressed_up_kept");
   }

   // --------------------------------------------------------------------- wall

   /** B7(61-65) / Control.java:713-715,767-795 - the wall/hold machine. */
   private static void wallPins() {
      carAt(0, 0, 0, 0);
      // B7(61): wall 0 + hold 0 zeroes clrnce, sticky lastl sets left and the
      // wall is consumed; dam[0]==0 -> hold becomes 1
      Preset m61 = new Preset();
      Madness md61 = mad(m61, 0);
      md61.mtouch = true;
      Trackers t61 = new Trackers();
      Control c61 = new Control(m61);
      c61.wall = 0;
      c61.hold = 0;
      c61.lwall = -1;
      c61.lastl = true;
      c61.upcnt = 30;
      c61.preform(md61, car, cp(), t61);
      Check.eq(c61.clrnce, 0, "wall_touch_clrnce_0");
      Check.ok(c61.left, "wall_touch_lastl_left_true");
      Check.ok(!c61.right, "wall_touch_right_false");
      Check.ok(c61.wlastl, "wall_touch_wlastl_true");
      Check.eq(c61.lwall, 0, "wall_touch_lwall_0");
      Check.eq(c61.hold, 1, "wall_touch_dam0_hold_1");
      Check.eq(c61.wall, -1, "wall_touch_consumed_minus1");

      // B7(62): lastl false picks right
      Preset m62 = new Preset();
      Madness md62 = mad(m62, 0);
      md62.mtouch = true;
      Control c62 = new Control(m62);
      c62.wall = 0;
      c62.lwall = -1;
      c62.lastl = false;
      c62.upcnt = 30;
      c62.preform(md62, car, cp(), new Trackers());
      Check.ok(c62.right, "wall_touch_lastl_false_right_true");
      Check.ok(!c62.wlastl, "wall_touch_lastl_false_wlastl_false");
      Check.eq(c62.hold, 1, "wall_touch_lastl_false_hold_1");
      Check.eq(c62.wall, -1, "wall_touch_lastl_false_consumed");

      // B7(63): lwall==wall keeps the sticky side even when lastl flipped
      Preset m63 = new Preset();
      Madness md63 = mad(m63, 0);
      md63.mtouch = true;
      Control c63 = new Control(m63);
      c63.wall = 0;
      c63.lwall = 0;
      c63.wlastl = true;
      c63.lastl = false;
      c63.upcnt = 30;
      c63.preform(md63, car, cp(), new Trackers());
      Check.ok(c63.left, "wall_sticky_left_true");
      Check.ok(!c63.right, "wall_sticky_right_false");
      Check.eq(c63.hold, 1, "wall_sticky_dam0_hold_1");
      Check.eq(c63.wall, -1, "wall_sticky_consumed");

      // lwall==wall with wlastl false takes the right branch (:777-781)
      Preset mW = new Preset();
      Madness mdW = mad(mW, 0);
      mdW.mtouch = true;
      Control cW = new Control(mW);
      cW.wall = 0;
      cW.lwall = 0;
      cW.wlastl = false;
      cW.upcnt = 30;
      cW.preform(mdW, car, cp(), new Trackers());
      Check.ok(!cW.left, "wall_sticky_right_left_false");
      Check.ok(cW.right, "wall_sticky_right_right_true");
      Check.eq(cW.hold, 1, "wall_sticky_right_hold_1");
      Check.eq(cW.wall, -1, "wall_sticky_right_consumed");

      // B7(64): no wall but a live hold skips the steering block and decrements
      Preset m64 = new Preset();
      Madness md64 = mad(m64, 0);
      md64.mtouch = true;
      Control c64 = new Control(m64);
      c64.wall = -1;
      c64.hold = 5;
      c64.upcnt = 30;
      c64.preform(md64, car, cp(), new Trackers());
      Check.eq(c64.hold, 4, "hold_5_decrements_4");
      Check.ok(!c64.left && !c64.right && !c64.down && !c64.handb,
               "hold_gate_suppresses_steering");

      // B7(65): dam[0]!=0 increments and clamps at 10; dam[0]==0 drops to 1
      Preset m65 = new Preset();
      Madness md65 = mad(m65, 0);
      md65.mtouch = true;
      Trackers t65 = new Trackers();
      t65.dam[0] = 5;
      Control c65 = new Control(m65);
      c65.wall = 0;
      c65.hold = 10;
      c65.upcnt = 30;
      c65.preform(md65, car, cp(), t65);
      Check.eq(c65.hold, 10, "wall_dam5_hold_clamped_10");
      Check.eq(c65.wall, -1, "wall_dam5_consumed");
      Preset m65b = new Preset();
      Madness md65b = mad(m65b, 0);
      md65b.mtouch = true;
      Control c65b = new Control(m65b);
      c65b.wall = 0;
      c65b.hold = 10;
      c65b.upcnt = 30;
      c65b.preform(md65b, car, cp(), new Trackers());
      Check.eq(c65b.hold, 1, "wall_dam0_hold_drops_1");
   }

   // ------------------------------------------------------------ path select

   /** The touch branch's checkpoint selection (Control.java:533-633): the
    *  bulistc skip-2 path, the rampp biases, the skiplev random walk and the
    *  lap-count filter. pan is observed with turncnt 1 > randtcnt 0, which
    *  recomputes it from cp.x[the selected index] (:678-686). */
   private static void pathPins() {
      // P1: bulistc's own branch (:539-555) takes point-2 and skips the random
      // walk entirely, so this pass draws nothing and pan stays frozen
      Preset m1 = new Preset();
      Madness md1 = mad(m1, 0);
      md1.point = 2;
      Control c1 = new Control(m1);
      c1.bulistc = true;
      c1.trfix = 1;
      carAt(0, 0, 0, 0);
      touch(c1, md1, cp());
      Check.ok(md1.nofocus, "path_bulistc_nofocus_true");
      Check.eq(m1.draws, 0, "path_bulistc_zero_draws");
      Check.eq(c1.pan, 0, "path_bulistc_pan_frozen_0");
      Check.eq(c1.turncnt, 1, "path_bulistc_turncnt_1");
      Check.eq(c1.randtcnt, 0, "path_bulistc_randtcnt_0");

      // P2: stage 9 clamps var23 to 76 and the -4 skip walks backwards
      // (:545-553); the whole pass stays in the bulistc arm
      Preset m2 = new Preset();
      Madness md2 = mad(m2, 0);
      md2.point = 100;
      CheckPoints p2 = cp();
      p2.stage = 9;
      Control c2 = new Control(m2);
      c2.bulistc = true;
      c2.trfix = 1;
      carAt(0, 0, 0, 0);
      touch(c2, md2, p2);
      Check.ok(md2.nofocus, "path_stage9_point100_nofocus_true");

      // P3: rampp 1 looks one past a non-positive checkpoint for a -2
      // (:557-566); the selected index 1 makes pan -90
      Preset m3 = new Preset();
      Madness md3 = mad(m3, 0);
      CheckPoints p3 = cpBul();
      p3.typ[0] = 0;
      p3.typ[1] = -2;
      p3.x[1] = 100;
      Control c3 = new Control(m3);
      c3.rampp = 1;
      c3.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c3, md3, p3);
      Check.eq(c3.pan, -90, "path_rampp1_moves_to_minus2_index_1");
      Check.eq(c3.turncnt, 0, "path_rampp1_turncnt_reset_0");
      Check.eq(c3.randtcnt, 0, "path_rampp1_randtcnt_0");

      // P4: rampp -1 skips a -2 checkpoint at point (:568-572)
      Preset m4 = new Preset();
      Madness md4 = mad(m4, 0);
      CheckPoints p4 = cpBul();
      p4.typ[0] = -2;
      p4.x[1] = 100;
      Control c4 = new Control(m4);
      c4.rampp = -1;
      c4.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c4, md4, p4);
      Check.eq(c4.pan, -90, "path_ramppminus1_skips_minus2_to_index_1");

      // P5: first draw <= skiplev and second > skiplev runs the while that
      // skips -1 entries (:574-581); x[2] selects a left-side target
      Preset m5 = new Preset(1); // 0.0, 0.9, 0.8, ...
      Madness md5 = mad(m5, 0);
      CheckPoints p5 = cpBul();
      p5.typ[0] = -1;
      p5.typ[1] = -1;
      p5.typ[2] = 1;
      p5.x[2] = -100;
      Control c5 = new Control(m5);
      c5.skiplev = 0.4F;
      c5.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c5, md5, p5);
      Check.eq(c5.pan, 90, "path_skiplev_inner_walk_skips_minus1_to_index_2");
      Check.eq(m5.draws, 3, "path_skiplev_inner_walk_three_draws");

      // P6: draw > skiplev enters the lap-count scan (:582-615). clear 4 with
      // five positive checkpoints walks forward to index 3, whose left-side
      // target makes pan 90.
      Preset m6 = new Preset();
      Madness md6 = mad(m6, 0);
      md6.clear = 4;
      CheckPoints p6 = cpBul();
      p6.x[3] = -100;
      Control c6 = new Control(m6);
      c6.skiplev = -1.0F;
      c6.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c6, md6, p6);
      Check.eq(c6.pan, 90, "path_clear_scan_walks_to_index_3");
      Check.eq(m6.draws, 2, "path_clear_scan_two_draws");

      // P7: rampp 2 backs up over a -2 one past the selected index (:618-629)
      Preset m7 = new Preset(1);
      Madness md7 = mad(m7, 0);
      CheckPoints p7 = cpBul();
      p7.typ[0] = -1;
      p7.typ[1] = -1;
      p7.typ[2] = 1;
      p7.typ[3] = -2;
      p7.x[1] = -100;
      Control c7 = new Control(m7);
      c7.rampp = 2;
      c7.skiplev = 0.4F;
      c7.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c7, md7, p7);
      Check.eq(c7.pan, 90, "path_rampp2_backs_up_to_index_1");
      Check.eq(m7.draws, 3, "path_rampp2_three_draws");
   }

   // ------------------------------------------------------------------- tricks

   /** B7(66-75) / Control.java:796-1115 - the airborne trick phases. Every
    *  fixture here runs mtouch==false and wtouch==false. */
   private static void trickPins() {
      carAt(0, 0, 0, 0);
      // B7(66): trickfase 1, lrdirect -1 -> flycnt++, handb + left
      Preset m66 = new Preset();
      Madness md66 = mad(m66, 0);
      Control c66 = new Control(m66);
      c66.trickfase = K.intv("trickfase_air");
      c66.lrdirect = -1;
      c66.lrstart = 0;
      c66.saftey = 30;
      c66.preform(md66, car, cp(), new Trackers());
      Check.eq(c66.flycnt, 1, "trick_fase1_flycnt_1");
      Check.ok(c66.handb, "trick_fase1_lrdirect_minus1_handb");
      Check.ok(c66.left, "trick_fase1_lrdirect_minus1_left");
      Check.ok(!c66.right, "trick_fase1_lrdirect_minus1_right_false");
      Check.eq(c66.trickfase, K.intv("trickfase_air"), "trick_fase1_stays_1");
      Check.eq(m66.draws, 0, "trick_fase1_no_random");

      // B7(67): lrdirect +1 -> right; uddirect -1 -> down; +1 -> up
      Preset m67 = new Preset();
      Madness md67 = mad(m67, 0);
      Control c67 = new Control(m67);
      c67.trickfase = K.intv("trickfase_air");
      c67.lrdirect = 1;
      c67.preform(md67, car, cp(), new Trackers());
      Check.ok(c67.handb && c67.right, "trick_fase1_lrdirect1_handb_right");
      Preset m67b = new Preset();
      Madness md67b = mad(m67b, 0);
      Control c67b = new Control(m67b);
      c67b.trickfase = K.intv("trickfase_air");
      c67b.uddirect = -1;
      c67b.preform(md67b, car, cp(), new Trackers());
      Check.ok(c67b.handb && c67b.down, "trick_fase1_uddirect_minus1_handb_down");
      Preset m67c = new Preset();
      Madness md67c = mad(m67c, 0);
      Control c67c = new Control(m67c);
      c67c.trickfase = K.intv("trickfase_air");
      c67c.uddirect = 1;
      c67c.preform(md67c, car, cp(), new Trackers());
      Check.ok(c67c.handb && c67c.up, "trick_fase1_uddirect1_handb_up");

      // B7(68): the saftey test flips to phase 2 and clears all trick flags.
      // The same pass then advances swat 0 -> 1 (:945-954), not 0.
      Preset m68 = new Preset();
      Madness md68 = mad(m68, 0);
      Control c68 = new Control(m68);
      c68.trickfase = K.intv("trickfase_air");
      c68.saftey = -1;
      c68.udcomp = true;
      c68.lrcomp = true;
      c68.udbare = true;
      c68.lrbare = true;
      c68.onceu = c68.onced = c68.oncel = c68.oncer = true;
      c68.swat = 7;
      c68.preform(md68, car, cp(), new Trackers());
      Check.eq(c68.trickfase, K.intv("trickfase_ground"), "trick_saftey_negative_phase2");
      Check.eq(c68.swat, 1, "trick_saftey_negative_swat_same_call_1");
      Check.ok(!c68.udcomp && !c68.lrcomp && !c68.udbare && !c68.lrbare,
               "trick_saftey_negative_clears_comp_bare");
      Check.ok(!c68.onceu && !c68.onced && !c68.oncel && !c68.oncer,
               "trick_saftey_negative_clears_once");

      // B7(69): swat 0 reads the compression flags; wtouch advances 1 -> 2
      Preset m69 = new Preset();
      Madness md69 = mad(m69, 0);
      md69.wtouch = true;
      md69.dcomp = 1.0F;
      Control c69 = new Control(m69);
      c69.trickfase = K.intv("trickfase_ground");
      c69.preform(md69, car, cp(), new Trackers());
      Check.ok(c69.udbare, "trick_swat0_dcomp_sets_udbare");
      Check.ok(!c69.lrbare, "trick_swat0_no_lrcomp_lrbare_false");
      Check.eq(c69.swat, 2, "trick_wtouch_swat2");
      Check.ok(c69.onced, "trick_swat0_non_capsized_marks_onced");
      Check.eq(m69.draws, 1, "trick_swat0_one_random_draw");

      // B7(70): swat 2 without wtouch -> 3 unconditionally
      Preset m70 = new Preset();
      Madness md70 = mad(m70, 0);
      Control c70 = new Control(m70);
      c70.trickfase = K.intv("trickfase_ground");
      c70.swat = 2;
      c70.preform(md70, car, cp(), new Trackers());
      Check.eq(c70.swat, 3, "trick_swat2_notouch_swat3");
      Check.eq(m70.draws, 0, "trick_swat2_notouch_zero_draws");

      // B7(71): capsized + dcomp>ucomp + !udcomp + !onced -> down, onced is
      // written only by the non-capsized arm so it stays false
      Preset m71 = new Preset();
      Madness md71 = mad(m71, 0);
      md71.capsized = true;
      md71.dcomp = 2.0F;
      md71.ucomp = 1.0F;
      md71.lcomp = 5.0F;
      Control c71 = new Control(m71);
      c71.trickfase = K.intv("trickfase_ground");
      c71.swat = 3;
      c71.udbare = true;
      c71.preform(md71, car, cp(), new Trackers());
      Check.ok(c71.down, "trick_capsized_dcomp_down_true");
      Check.ok(!c71.onced, "trick_capsized_onced_untouched_false");
      Check.ok(!c71.udcomp, "trick_capsized_lcomp_minus_rcomp_5_udcomp_false");

      // B7(72): capsized + udcomp + var27 135 -> up, onced stays true
      Preset m72 = new Preset();
      Madness md72 = mad(m72, 0);
      md72.capsized = true;
      md72.dcomp = 2.0F;
      md72.ucomp = 1.0F;
      md72.lcomp = 1.0F;
      md72.pzy = 45;
      Control c72 = new Control(m72);
      c72.trickfase = K.intv("trickfase_ground");
      c72.swat = 3;
      c72.udbare = true;
      c72.udcomp = true;
      c72.onced = true;
      c72.preform(md72, car, cp(), new Trackers());
      Check.ok(c72.up, "trick_capsized_udcomp_var27_135_up");
      Check.ok(!c72.down, "trick_capsized_udcomp_down_false");
      Check.ok(c72.onced, "trick_capsized_udcomp_onced_stays_true");

      // B7(73): lrbare + capsized + lcomp>rcomp + !lrcomp + !oncel -> left
      Preset m73 = new Preset();
      Madness md73 = mad(m73, 0);
      md73.capsized = true;
      md73.lcomp = 10.0F;
      Control c73 = new Control(m73);
      c73.trickfase = K.intv("trickfase_ground");
      c73.swat = 3;
      c73.lrbare = true;
      c73.preform(md73, car, cp(), new Trackers());
      Check.ok(c73.left, "trick_capsized_lcomp_left_true");
      Check.ok(!c73.right, "trick_capsized_lcomp_right_false");
      Check.ok(!c73.oncel, "trick_capsized_oncel_untouched_false");
      Check.ok(!c73.lrcomp, "trick_capsized_lcomp_minus_rcomp_10_lrcomp_false");

      // B7(74): not capsized + udcomp + perfection false -> no up/down write
      // (the random arm at :1011 is the else of `if (udcomp)`), onced becomes true
      Preset m74 = new Preset();
      Madness md74 = mad(m74, 0);
      md74.dcomp = 1.0F;
      md74.pzy = 45;
      Control c74 = new Control(m74);
      c74.trickfase = K.intv("trickfase_ground");
      c74.swat = 3;
      c74.udbare = true;
      c74.udcomp = true;
      c74.perfection = false;
      c74.preform(md74, car, cp(), new Trackers());
      Check.ok(!c74.up && !c74.down, "trick_perfection_false_no_write");
      Check.ok(c74.onced, "trick_perfection_false_onced_true");
      Check.ok(!c74.onceu, "trick_perfection_false_onceu_false");
      Check.eq(m74.draws, 0, "trick_perfection_false_zero_draws");

      // B7(75): phase 0 -> 1 -> the same call's phase-1 block. trickprf must be
      // preset (the plan block never runs on a non-touch pass); -1 makes the
      // :799 draw always pass. var26 == 40*1200/4000 == 12 > 9.
      Preset m75 = new Preset();
      Madness md75 = mad(m75, 0);
      md75.power = 50.0F;
      for (int i = 0; i < 4; i++) {
         md75.scy[i] = 10.0F;
      }
      Control c75 = new Control(m75);
      c75.trickprf = -1.0F;
      carAt(0, 1500, 0, 0);
      c75.preform(md75, car, cp(), new Trackers());
      Check.eq(m75.draws, 1, "trick_phase0_one_draw");
      Check.ok(c75.afta, "trick_phase0_afta_true");
      Check.eq(c75.trickfase, K.intv("trickfase_air"), "trick_phase0_sets_phase1");
      Check.eq(c75.uddirect, -1, "trick_phase0_var26_12_uddirect_minus1");
      Check.eq(c75.udstart, 0, "trick_phase0_udstart_0");
      Check.eq(c75.flycnt, 1, "trick_phase0_same_call_flycnt_1");
      Check.ok(c75.handb, "trick_phase0_same_call_handb");
      Check.ok(c75.down, "trick_phase0_same_call_down");
      Check.ok(!c75.up, "trick_phase0_same_call_up_false");

      // the other arm of :799: a 0.1 draw is NOT > trickprf/3 for trickprf 0.5
      Preset lo = new Preset(2);
      Madness md75b = mad(lo, 0);
      for (int i = 0; i < 4; i++) {
         md75b.scy[i] = 10.0F;
      }
      Control c75b = new Control(lo);
      c75b.trickprf = 0.5F;
      carAt(0, 1500, 0, 0);
      c75b.preform(md75b, car, cp(), new Trackers());
      Check.eq(lo.draws, 1, "trick_phase0_neg_one_draw");
      Check.eq(c75b.trickfase, K.intv("trickfase_none"), "trick_phase0_low_draw_sets_minus1");
      Check.ok(c75b.afta, "trick_phase0_low_draw_afta_true");
   }

   /** The remaining trick-arm corners: the var26 >= 16 setup (Control.java:
    *  812-873), the lrswt/udswt reversals (:891-919), the capsized swat-2 swap
    *  (:962-970) and the capsized/non-capsized up/down/left/right ladders
    *  (:975-1113). */
   private static void trickExtraPins() {
      carAt(0, 0, 0, 0);

      // T1: var26 == 40*1600/4000 == 16 enters the else arm (:836-859) with a
      // decreasing preset; trfix 3 then clears lrdirect (:862-868)
      Preset m1 = new Preset();
      Madness md1 = mad(m1, 0);
      for (int i = 0; i < 4; i++) {
         md1.scy[i] = 10.0F;
      }
      Control c1 = new Control(m1);
      c1.trickprf = -1.0F;
      c1.trfix = 3;
      carAt(0, 1900, 0, 0);
      c1.preform(md1, car, cp(), new Trackers());
      Check.eq(c1.trickfase, K.intv("trickfase_air"), "trick_var26_16_else_arm_phase1");
      Check.eq(c1.uddirect, -1, "trick_var26_16_else_arm_uddirect_minus1");
      Check.eq(c1.lrdirect, 0, "trick_var26_16_trfix3_clears_lrdirect");
      Check.eq(c1.lrstart, 12, "trick_var26_16_lrstart_12");
      Check.eq(c1.udstart, -9, "trick_var26_16_udstart_negative_9");
      Check.ok(!c1.lrswt && !c1.udswt, "trick_var26_16_swt_false");
      Check.eq(c1.trfix, 4, "trick_var26_16_trfix3_becomes_4");
      Check.eq(m1.draws, 12, "trick_var26_16_twelve_draws");
      Check.ok(c1.down && c1.handb, "trick_var26_16_phase1_down_handb");
      Check.ok(!c1.left && !c1.right, "trick_var26_16_no_lr_steer");
      Check.eq(c1.flycnt, 1, "trick_var26_16_flycnt_1");

      // T2: an increasing preset makes :812 `!(r > r)` true, entering the
      // lr-first arm (:813-835)
      Preset m2 = new Preset(2); // 0.1, 0.0, 0.9, 0.8, ...
      Madness md2 = mad(m2, 0);
      for (int i = 0; i < 4; i++) {
         md2.scy[i] = 10.0F;
      }
      Control c2 = new Control(m2);
      c2.trickprf = -1.0F;
      carAt(0, 1900, 0, 0);
      c2.preform(md2, car, cp(), new Trackers());
      Check.eq(c2.trickfase, K.intv("trickfase_air"), "trick_var26_16_first_arm_phase1");
      Check.eq(c2.uddirect, -1, "trick_var26_16_first_arm_uddirect_minus1");
      Check.eq(c2.lrdirect, -1, "trick_var26_16_first_arm_lrdirect_minus1");
      Check.eq(c2.lrstart, -6, "trick_var26_16_first_arm_lrstart_negative_6");
      Check.eq(c2.udstart, 3, "trick_var26_16_first_arm_udstart_3");
      Check.eq(m2.draws, 12, "trick_var26_16_first_arm_twelve_draws");
      Check.ok(c2.left && c2.handb, "trick_var26_16_first_arm_left_handb");
      Check.eq(c2.flycnt, 1, "trick_var26_16_first_arm_flycnt_1");

      // T3: stage 8 skips the lr arm and the trfix 3 branch forces
      // uddirect 1 with udswt false (:848,869-872)
      Preset m3 = new Preset();
      Madness md3 = mad(m3, 0);
      for (int i = 0; i < 4; i++) {
         md3.scy[i] = 10.0F;
      }
      CheckPoints p3 = cp();
      p3.stage = 8;
      Control c3 = new Control(m3);
      c3.trickprf = -1.0F;
      c3.trfix = 3;
      carAt(0, 1900, 0, 0);
      c3.preform(md3, car, p3, new Trackers());
      Check.eq(c3.trickfase, K.intv("trickfase_air"), "trick_stage8_trfix3_phase1");
      Check.eq(c3.uddirect, 1, "trick_stage8_trfix3_uddirect_1");
      Check.ok(!c3.udswt, "trick_stage8_trfix3_udswt_false");
      Check.eq(c3.lrdirect, 0, "trick_stage8_trfix3_lrdirect_0");
      Check.eq(m3.draws, 8, "trick_stage8_trfix3_eight_draws");
      Check.eq(c3.trfix, 4, "trick_stage8_trfix3_becomes_4");

      // T3b: three phases of the same var26>=16 setup reach the other pair
      // else arms - S6 sets udswt (:844-845), S0 sets lrswt (:856-857) and S9
      // takes lrdirect=1 (:850-852)
      Preset s6 = new Preset(6);
      Madness mds6 = mad(s6, 0);
      for (int i = 0; i < 4; i++) {
         mds6.scy[i] = 10.0F;
      }
      Control cs6 = new Control(s6);
      cs6.trickprf = -1.0F;
      carAt(0, 1900, 0, 0);
      cs6.preform(mds6, car, cp(), new Trackers());
      Check.eq(cs6.trickfase, K.intv("trickfase_air"), "trick_phase6_phase1");
      Check.eq(cs6.lrstart, 15, "trick_phase6_lrstart_15");
      Check.eq(cs6.udstart, 0, "trick_phase6_udstart_0");
      Check.ok(cs6.udswt, "trick_phase6_udswt_true");
      Check.ok(!cs6.lrswt, "trick_phase6_lrswt_false");
      Check.eq(s6.draws, 12, "trick_phase6_twelve_draws");
      Preset s0 = new Preset(0);
      Madness mds0 = mad(s0, 0);
      for (int i = 0; i < 4; i++) {
         mds0.scy[i] = 10.0F;
      }
      Control cs0 = new Control(s0);
      cs0.trickprf = -1.0F;
      carAt(0, 1900, 0, 0);
      cs0.preform(mds0, car, cp(), new Trackers());
      Check.eq(cs0.lrstart, 27, "trick_phase0_lrstart_27");
      Check.eq(cs0.udstart, -4, "trick_phase0_udstart_negative_4");
      Check.ok(cs0.lrswt, "trick_phase0_lrswt_true");
      Check.ok(!cs0.udswt, "trick_phase0_udswt_false");
      Preset s9 = new Preset(9);
      Madness mds9 = mad(s9, 0);
      for (int i = 0; i < 4; i++) {
         mds9.scy[i] = 10.0F;
      }
      Control cs9 = new Control(s9);
      cs9.trickprf = -1.0F;
      carAt(0, 1900, 0, 0);
      cs9.preform(mds9, car, cp(), new Trackers());
      Check.eq(cs9.lrdirect, 1, "trick_phase9_lrdirect_1");
      Check.eq(cs9.lrstart, 24, "trick_phase9_lrstart_24");
      Check.eq(cs9.udstart, -3, "trick_phase9_udstart_negative_3");

      // T4: lrswt flips lrdirect when |pxy-oxy| > 180 (:891-899)
      Preset m4 = new Preset();
      Madness md4 = mad(m4, 0);
      md4.pxy = 200;
      Control c4 = new Control(m4);
      c4.trickfase = K.intv("trickfase_air");
      c4.lrdirect = -1;
      c4.lrswt = true;
      c4.oxy = 0;
      c4.lrstart = 0;
      c4.preform(md4, car, cp(), new Trackers());
      Check.eq(c4.lrdirect, 1, "trick_lrswt_flips_lrdirect_to_1");
      Check.ok(!c4.lrswt, "trick_lrswt_cleared");
      Check.ok(c4.right && c4.handb, "trick_lrswt_flip_right_handb");

      // T5: udswt flips uddirect (:910-918)
      Preset m5 = new Preset();
      Madness md5 = mad(m5, 0);
      md5.pzy = 200;
      Control c5 = new Control(m5);
      c5.trickfase = K.intv("trickfase_air");
      c5.uddirect = -1;
      c5.udswt = true;
      c5.ozy = 0;
      c5.udstart = 0;
      c5.preform(md5, car, cp(), new Trackers());
      Check.eq(c5.uddirect, 1, "trick_udswt_flips_uddirect_to_1");
      Check.ok(!c5.udswt, "trick_udswt_cleared");
      Check.ok(c5.up && c5.handb, "trick_udswt_flip_up_handb");

      // T6: capsized swat 2 with a 0.6 draw > mustland 0.5 swaps the bare
      // flags (:962-970) and then advances swat to 3
      Preset m6 = new Preset(7); // first draw 0.6
      Madness md6 = mad(m6, 0);
      md6.capsized = true;
      md6.dcomp = 1.0F;
      Control c6 = new Control(m6);
      c6.trickfase = K.intv("trickfase_ground");
      c6.swat = 2;
      c6.udbare = true;
      c6.preform(md6, car, cp(), new Trackers());
      Check.eq(c6.swat, 3, "trick_swat2_capsized_swap_swat3");
      Check.ok(!c6.udbare, "trick_swat2_capsized_swap_udbare_false");
      Check.ok(c6.lrbare, "trick_swat2_capsized_swap_lrbare_true");
      Check.eq(m6.draws, 1, "trick_swat2_capsized_swap_one_draw");

      // T7: pzy -180 wraps var27 through +360 to -90 (:978-984); capsized +
      // udcomp at exactly 90 takes the down branch
      Preset m7 = new Preset();
      Madness md7 = mad(m7, 0);
      md7.capsized = true;
      md7.dcomp = 1.0F;
      md7.pzy = -180;
      Control c7 = new Control(m7);
      c7.trickfase = K.intv("trickfase_ground");
      c7.swat = 3;
      c7.udbare = true;
      c7.udcomp = true;
      c7.onced = true;
      c7.preform(md7, car, cp(), new Trackers());
      Check.ok(c7.down, "trick_capsized_var27_wrap_down_true");
      Check.ok(!c7.up, "trick_capsized_var27_wrap_up_false");
      Check.ok(c7.onced, "trick_capsized_var27_wrap_onced_kept");

      // T8: not capsized + udcomp + perfection + |var27-90|>30 -> up (:1028-1035)
      Preset m8 = new Preset();
      Madness md8 = mad(m8, 0);
      md8.dcomp = 1.0F;
      md8.pzy = 45;
      Control c8 = new Control(m8);
      c8.trickfase = K.intv("trickfase_ground");
      c8.swat = 3;
      c8.udbare = true;
      c8.udcomp = true;
      c8.perfection = true;
      c8.preform(md8, car, cp(), new Trackers());
      Check.ok(c8.up, "trick_perfection_true_var27_135_up");
      Check.ok(!c8.down, "trick_perfection_true_down_false");
      Check.ok(c8.onced, "trick_perfection_true_onced_true");

      // T9: not capsized + !udcomp + draw > mustland -> up (:1036-1040)
      Preset m9 = new Preset(7);
      Madness md9 = mad(m9, 0);
      md9.dcomp = 1.0F;
      Control c9 = new Control(m9);
      c9.trickfase = K.intv("trickfase_ground");
      c9.swat = 3;
      c9.udbare = true;
      c9.preform(md9, car, cp(), new Trackers());
      Check.ok(c9.up, "trick_random_udcomp_false_up_true");
      Check.ok(c9.onced, "trick_random_udcomp_false_onced_true");
      Check.eq(m9.draws, 1, "trick_random_udcomp_false_one_draw");

      // T10: lrbare capsized + lrcomp + var29 135 -> left (:1063-1069)
      Preset m10 = new Preset();
      Madness md10 = mad(m10, 0);
      md10.capsized = true;
      md10.lcomp = 10.0F;
      md10.pxy = 45;
      Control c10 = new Control(m10);
      c10.trickfase = K.intv("trickfase_ground");
      c10.swat = 3;
      c10.lrbare = true;
      c10.lrcomp = true;
      c10.preform(md10, car, cp(), new Trackers());
      Check.ok(c10.left, "trick_capsized_lrcomp_var29_135_left");
      Check.ok(!c10.right, "trick_capsized_lrcomp_right_false");
      Check.ok(!c10.oncel, "trick_capsized_lrcomp_oncel_false");

      // T11: lrbare capsized + !lrcomp + !oncel with lcomp<=rcomp -> right (:1089-1098)
      Preset m11 = new Preset();
      Madness md11 = mad(m11, 0);
      md11.capsized = true;
      md11.rcomp = 10.0F;
      Control c11 = new Control(m11);
      c11.trickfase = K.intv("trickfase_ground");
      c11.swat = 3;
      c11.lrbare = true;
      c11.preform(md11, car, cp(), new Trackers());
      Check.ok(!c11.left, "trick_capsized_else_right_left_false");
      Check.ok(c11.right, "trick_capsized_else_right_right_true");
      Check.ok(!c11.oncer, "trick_capsized_else_right_oncer_false");

      // T12: not capsized + lrcomp + perfection + var29 135 -> left (:1075-1082)
      Preset m12 = new Preset();
      Madness md12 = mad(m12, 0);
      md12.lcomp = 10.0F;
      md12.pxy = 45;
      Control c12 = new Control(m12);
      c12.trickfase = K.intv("trickfase_ground");
      c12.swat = 3;
      c12.lrbare = true;
      c12.lrcomp = true;
      c12.perfection = true;
      c12.oncel = true;
      c12.preform(md12, car, cp(), new Trackers());
      Check.ok(c12.left, "trick_perfection_lr_left_true");
      Check.ok(!c12.right, "trick_perfection_lr_right_false");
      Check.ok(c12.oncel, "trick_perfection_lr_oncel_kept");

      // T13: not capsized + !lrcomp + lcomp>rcomp + draw > mustland -> right
      Preset m13 = new Preset(7);
      Madness md13 = mad(m13, 0);
      md13.lcomp = 10.0F;
      Control c13 = new Control(m13);
      c13.trickfase = K.intv("trickfase_ground");
      c13.swat = 3;
      c13.lrbare = true;
      c13.preform(md13, car, cp(), new Trackers());
      Check.ok(!c13.left, "trick_random_lr_left_false");
      Check.ok(c13.right, "trick_random_lr_right_true");
      Check.ok(c13.oncel, "trick_random_lr_oncel_true");
      Check.eq(m13.draws, 1, "trick_random_lr_one_draw");

      // T14: not capsized + !lrcomp + lcomp<=rcomp + draw > mustland -> left
      Preset m14 = new Preset(7);
      Madness md14 = mad(m14, 0);
      md14.rcomp = 10.0F;
      Control c14 = new Control(m14);
      c14.trickfase = K.intv("trickfase_ground");
      c14.swat = 3;
      c14.lrbare = true;
      c14.preform(md14, car, cp(), new Trackers());
      Check.ok(c14.left, "trick_random_lr_else_left_true");
      Check.ok(!c14.right, "trick_random_lr_else_right_false");
      Check.ok(c14.oncer, "trick_random_lr_else_oncer_true");

      // T15: zyinv adds 180 to var29 and pxy -180 wraps through +360 (:1044-1057)
      Preset m15 = new Preset();
      Madness md15 = mad(m15, 0);
      md15.capsized = true;
      md15.lcomp = 10.0F;
      md15.pxy = -180;
      Control c15 = new Control(m15);
      c15.trickfase = K.intv("trickfase_ground");
      c15.swat = 3;
      c15.lrbare = true;
      c15.zyinv = true;
      c15.preform(md15, car, cp(), new Trackers());
      Check.ok(c15.left, "trick_lr_zyinv_wrap_left");

      // T16: lcomp-rcomp < 10 with oncel sets lrcomp (:1059-1061)
      Preset m16 = new Preset();
      Madness md16 = mad(m16, 0);
      md16.capsized = true;
      md16.lcomp = 5.0F;
      md16.pxy = 45;
      Control c16 = new Control(m16);
      c16.trickfase = K.intv("trickfase_ground");
      c16.swat = 3;
      c16.lrbare = true;
      c16.oncel = true;
      c16.preform(md16, car, cp(), new Trackers());
      Check.ok(c16.lrcomp, "trick_lrcomp_set_below_10_with_oncel");
      Check.ok(c16.left, "trick_lrcomp_set_left");

      // T17: capsized + lrcomp at var29 0 takes the right branch (:1090-1095)
      Preset m17 = new Preset();
      Madness md17 = mad(m17, 0);
      md17.capsized = true;
      md17.lcomp = 10.0F;
      md17.pxy = -90;
      Control c17 = new Control(m17);
      c17.trickfase = K.intv("trickfase_ground");
      c17.swat = 3;
      c17.lrbare = true;
      c17.lrcomp = true;
      c17.preform(md17, car, cp(), new Trackers());
      Check.ok(!c17.left, "trick_capsized_lrcomp_var29_0_left_false");
      Check.ok(c17.right, "trick_capsized_lrcomp_var29_0_right_true");

      // T18: capsized with onceu + small compression gap sets udcomp (:987-989)
      Preset m18 = new Preset();
      Madness md18 = mad(m18, 0);
      md18.capsized = true;
      md18.ucomp = 2.0F;
      md18.dcomp = 1.0F;
      md18.lcomp = 1.0F;
      md18.pzy = 45;
      Control c18 = new Control(m18);
      c18.trickfase = K.intv("trickfase_ground");
      c18.swat = 3;
      c18.udbare = true;
      c18.onceu = true;
      c18.preform(md18, car, cp(), new Trackers());
      Check.ok(c18.udcomp, "trick_udcomp_set_below_5_with_onceu");
      Check.ok(c18.up, "trick_udcomp_set_up");
      Check.ok(!c18.down, "trick_udcomp_set_down_false");

      // T19: not capsized, lcomp<=rcomp, !lrcomp, perfection - the perfection
      // arm needs lrcomp, so only oncer is written (:1099-1112)
      Preset m19 = new Preset();
      Madness md19 = mad(m19, 0);
      md19.rcomp = 10.0F;
      md19.pxy = 45;
      Control c19 = new Control(m19);
      c19.trickfase = K.intv("trickfase_ground");
      c19.swat = 3;
      c19.lrbare = true;
      c19.perfection = true;
      c19.preform(md19, car, cp(), new Trackers());
      Check.ok(!c19.left, "trick_lr_else_perfection_left_false");
      Check.ok(!c19.right, "trick_lr_else_perfection_right_false");
      Check.ok(c19.oncer, "trick_lr_else_perfection_oncer_true");
      // T20: capsized + udcomp with var27 0 -> down (:1018-1023)
      Preset m20 = new Preset();
      Madness md20 = mad(m20, 0);
      md20.capsized = true;
      md20.dcomp = 1.0F;
      md20.pzy = -90;
      Control c20 = new Control(m20);
      c20.trickfase = K.intv("trickfase_ground");
      c20.swat = 3;
      c20.udbare = true;
      c20.udcomp = true;
      c20.preform(md20, car, cp(), new Trackers());
      Check.ok(c20.down, "trick_capsized_udcomp_var27_0_down");
      Check.ok(!c20.up, "trick_capsized_udcomp_var27_0_up_false");

      // T21: non-capsized + dcomp<=ucomp + udcomp + perfection covers both
      // sides of :1028-1035
      for (int pzy : new int[]{45, -90}) {
         Preset mm = new Preset();
         Madness mdd = mad(mm, 0);
         mdd.ucomp = 1.0F;
         mdd.pzy = pzy;
         Control cc = new Control(mm);
         cc.trickfase = K.intv("trickfase_ground");
         cc.swat = 3;
         cc.udbare = true;
         cc.udcomp = true;
         cc.perfection = true;
         cc.preform(mdd, car, cp(), new Trackers());
         if (pzy == 45) {
            Check.ok(cc.up, "trick_perfection_upcomp_arm_var27_135_up");
            Check.ok(!cc.down, "trick_perfection_upcomp_arm_down_false");
         } else {
            Check.ok(cc.down, "trick_perfection_upcomp_arm_var27_0_down");
            Check.ok(!cc.up, "trick_perfection_upcomp_arm_up_false");
         }
      }

      // T22: lcomp>rcomp + lrcomp + perfection var29 0 -> right (:1075-1081)
      Preset m22 = new Preset();
      Madness md22 = mad(m22, 0);
      md22.lcomp = 10.0F;
      md22.pxy = -90;
      Control c22 = new Control(m22);
      c22.trickfase = K.intv("trickfase_ground");
      c22.swat = 3;
      c22.lrbare = true;
      c22.lrcomp = true;
      c22.perfection = true;
      c22.preform(md22, car, cp(), new Trackers());
      Check.ok(c22.right, "trick_perfection_lr_var29_0_right");
      Check.ok(!c22.left, "trick_perfection_lr_var29_0_left_false");

      // T23: lcomp<=rcomp + lrcomp + perfection var29 135 -> left (:1100-1106)
      Preset m23 = new Preset();
      Madness md23 = mad(m23, 0);
      md23.rcomp = 1.0F;
      md23.pxy = 45;
      Control c23 = new Control(m23);
      c23.trickfase = K.intv("trickfase_ground");
      c23.swat = 3;
      c23.lrbare = true;
      c23.lrcomp = true;
      c23.perfection = true;
      c23.preform(md23, car, cp(), new Trackers());
      Check.ok(c23.left, "trick_perfection_lr_else_var29_135_left");
      Check.ok(!c23.right, "trick_perfection_lr_else_var29_135_right_false");

      // T24: capsized + udcomp with dcomp<=ucomp and var27 0 -> down
      // (:1017-1023, the other half of the T20 branch)
      Preset m24 = new Preset();
      Madness md24 = mad(m24, 0);
      md24.capsized = true;
      md24.ucomp = 1.0F;
      md24.pzy = -90;
      Control c24 = new Control(m24);
      c24.trickfase = K.intv("trickfase_ground");
      c24.swat = 3;
      c24.udbare = true;
      c24.udcomp = true;
      c24.preform(md24, car, cp(), new Trackers());
      Check.ok(c24.down, "trick_capsized_else_udcomp_var27_0_down");
      Check.ok(!c24.up, "trick_capsized_else_udcomp_var27_0_up_false");

   }

   /** The remaining single-line clamps and ladder corners: plan-side skiplev/
    *  rampp/saftey/mustland clamps (Control.java:123-126,139-141,190-193,
    *  228-230), the turntyp attack overrides (:169-173), the var10 index
    *  normalisation (:327-333), the checkpoint-index wraps (:542,551,560,570,
    *  578), the second steering branch (:742-763) and the trick ladder's
    *  remaining else arms (:891-919,950-952,966-969,1000-1112). */
   private static void cornerPins() {
      // K1: skiplev 0.05 - 0.1 clamps at 0 (:124-126)
      Preset m1 = new Preset();
      Madness md1 = mad(m1, 0);
      Control c1 = new Control(m1);
      c1.skiplev = 0.05F;
      carAt(0, 0, 0, 0);
      plan(md1, c1, cp(), new Trackers());
      Check.eq(bits(c1.skiplev), bits(0.0F), "corner_skiplev_floor_0");

      // K2: power 70 with a 0.2 first draw gives rampp -1, which :139 lifts to
      // 0 because power < 75
      Preset m2 = new Preset(3); // 0.2, 0.1, 0.0, ...
      Madness md2 = mad(m2, 0);
      md2.power = 70.0F;
      CheckPoints p2 = cp();
      p2.stage = 2;
      Control c2 = new Control(m2);
      carAt(0, 0, 0, 0);
      plan(md2, c2, p2, new Trackers());
      Check.eq(c2.rampp, 0, "corner_power70_ramppminus1_lifted_to_0");
      Check.eq(c2.turntyp, 0, "corner_power70_turntyp_0");
      Check.eq(c2.saftey, 7, "corner_power70_saftey_7");
      Check.eq(m2.draws, 10, "corner_power70_ten_draws");

      // K3: attack != 0 forces turntyp 2 for non-9/10/11 stages (:169-170);
      // usebounce keeps this pass in the trick branch so attack is not spent
      Preset m3 = new Preset();
      Madness md3 = mad(m3, 0);
      Control c3 = new Control(m3);
      c3.attack = 5;
      CheckPoints p3 = cp();
      p3.stage = 2;
      carAt(0, 0, 0, 0);
      plan(md3, c3, p3, new Trackers());
      Check.eq(c3.turntyp, 2, "corner_attack_nonzero_turntyp_2");
      Check.eq(c3.attack, 5, "corner_attack_trick_branch_not_decremented");

      // K4: attack != 0 on stage 9 uses (int)(random*3) instead (:171-173); the
      // touch branch then decrements 5 -> 4
      Preset m4 = new Preset();
      Madness md4 = mad(m4, 0);
      Control c4 = new Control(m4);
      c4.attack = 5;
      CheckPoints p4 = cp();
      p4.stage = 9;
      carAt(0, 0, 0, 0);
      plan(md4, c4, p4, new Trackers());
      Check.eq(c4.turntyp, 0, "corner_attack_stage9_random_turntyp_0");
      Check.eq(c4.attack, 4, "corner_attack_stage9_touch_decrement_4");
      Check.eq(m4.draws, 9, "corner_attack_stage9_nine_draws");

      // K5: power 20 makes the raw saftey 23, clamped to 20 (:191-193)
      Preset m5 = new Preset();
      Madness md5 = mad(m5, 0);
      md5.power = 20.0F;
      CheckPoints p5 = cp();
      p5.stage = 2;
      Control c5 = new Control(m5);
      carAt(0, 0, 0, 0);
      plan(md5, c5, p5, new Trackers());
      Check.eq(c5.saftey, 20, "corner_low_power_saftey_clamped_20");
      Check.eq(c5.rampp, 1, "corner_low_power_rampp_1");

      // K6: power 40 takes the <=50 arm and subtracts 0.5 from mustland
      // (:228-230); the stage-2 base 0.7 - 0.2 - 0.5 == 0.0
      Preset m6 = new Preset();
      Madness md6 = mad(m6, 0);
      md6.power = 40.0F;
      CheckPoints p6 = cp();
      p6.stage = 2;
      Control c6 = new Control(m6);
      carAt(0, 0, 0, 0);
      plan(md6, c6, p6, new Trackers());
      Check.eq(bits(c6.mustland), 0, "corner_power40_mustland_minus_half_zero");
      Check.eq(c6.saftey, 17, "corner_power40_saftey_17");

      // K7: xz -90 walks both var10 normalisation loops inside the plan loop
      // (:327-333); the pass still computes stage-2 acuracy 24
      Preset m7 = new Preset();
      Madness md7 = mad(m7, 0);
      CheckPoints p7 = cp();
      p7.stage = 2;
      p7.opx[1] = 100;
      Control c7 = new Control(m7);
      c7.afta = true;
      carAt(0, 0, 0, -90);
      plan(md7, c7, p7, new Trackers());
      Check.eq(c7.acuracy, 24, "corner_xz_negative_plan_acuracy_24");
      Check.eq(c7.clrnce, 5, "corner_xz_negative_clrnce_5");
      Check.eq(m7.draws, 12, "corner_xz_negative_twelve_draws");

      // K8: the bulistc -4 walk wraps backwards from index 3 to 4 (:540-553)
      Preset m8 = new Preset();
      Madness md8 = mad(m8, 0);
      CheckPoints p8 = cpBul();
      p8.typ[0] = -4;
      p8.typ[1] = -4;
      p8.typ[2] = -4;
      p8.typ[3] = -4;
      Control c8 = new Control(m8);
      c8.bulistc = true;
      c8.trfix = 1;
      carAt(0, 0, 0, 0);
      touch(c8, md8, p8);
      Check.ok(md8.nofocus, "corner_bulistc_minus4_wrap_nofocus");

      // K9: rampp 1 wraps point 4 -> 0 before reading the -2 flag (:558-565)
      Preset m9 = new Preset();
      Madness md9 = mad(m9, 0);
      md9.point = 4;
      CheckPoints p9 = cpBul();
      p9.typ[4] = 0;
      p9.typ[0] = -2;
      p9.x[0] = -100;
      Control c9 = new Control(m9);
      c9.rampp = 1;
      c9.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c9, md9, p9);
      Check.eq(c9.pan, 90, "corner_rampp1_wraps_to_index0");

      // K10: rampp -1 wraps point 4 -> 0 (:569-571)
      Preset m10 = new Preset();
      Madness md10 = mad(m10, 0);
      md10.point = 4;
      CheckPoints p10 = cpBul();
      p10.typ[4] = -2;
      p10.x[0] = -100;
      Control c10 = new Control(m10);
      c10.rampp = -1;
      c10.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c10, md10, p10);
      Check.eq(c10.pan, 90, "corner_ramppminus1_wraps_to_index0");

      // K11: the -1 skip walk wraps point 4 -> 0 (:576-580)
      Preset m11 = new Preset(1); // 0.0, 0.9, ...
      Madness md11 = mad(m11, 0);
      md11.point = 4;
      CheckPoints p11 = cpBul();
      p11.typ[4] = -1;
      p11.x[0] = -100;
      Control c11 = new Control(m11);
      c11.skiplev = 0.4F;
      c11.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c11, md11, p11);
      Check.eq(c11.pan, 90, "corner_skip_walk_wraps_to_index0");
      Check.eq(m11.draws, 3, "corner_skip_walk_three_draws");

      // K12: zyinv adds 180 to the steering var25 (:692-695), making the
      // difference exactly 180 -> the second branch turns left
      Preset m12 = new Preset();
      Madness md12 = mad(m12, 0);
      Control c12 = new Control(m12);
      c12.zyinv = true;
      c12.upcnt = 30;
      carAt(0, 0, 0, 0);
      touch(c12, md12, cp());
      Check.ok(c12.left, "corner_steer_zyinv_left");
      Check.ok(c12.lastl, "corner_steer_zyinv_lastl");
      Check.ok(!c12.right, "corner_steer_zyinv_right_false");

      // K13: diff 340 uses the second branch and picks right (:742-749); the
      // 340 < 310 actuation guard stays closed
      Preset m13 = new Preset();
      Madness md13 = mad(m13, 0);
      Control c13 = new Control(m13);
      c13.pan = 170;
      c13.upcnt = 30;
      carAt(0, 0, 0, -170);
      touch(c13, md13, cp());
      Check.eq(c13.pan, 170, "corner_second_branch_pan_kept");
      Check.ok(c13.right, "corner_second_branch_right");
      Check.ok(!c13.lastl, "corner_second_branch_lastl_false");
      Check.ok(!c13.up && !c13.down && !c13.handb, "corner_second_branch_no_actuation");

      // K14: diff 270 is inside the second branch's actuation window: turntyp 1
      // -> down and up forced off
      Preset m14 = new Preset();
      Madness md14 = mad(m14, 0);
      md14.speed = 100.0F;
      Control c14 = new Control(m14);
      c14.pan = 170;
      c14.turntyp = 1;
      c14.upcnt = 30;
      carAt(0, 0, 0, -100);
      touch(c14, md14, cp());
      Check.ok(c14.right, "corner_second_branch_tt1_right");
      Check.ok(c14.down, "corner_second_branch_tt1_down");
      Check.ok(!c14.up, "corner_second_branch_tt1_up_false");
      // and turntyp 2 -> handb
      Preset m14b = new Preset();
      Madness md14b = mad(m14b, 0);
      md14b.speed = 100.0F;
      Control c14b = new Control(m14b);
      c14b.pan = 170;
      c14b.turntyp = 2;
      c14b.upcnt = 30;
      carAt(0, 0, 0, -100);
      touch(c14b, md14b, cp());
      Check.ok(c14b.right, "corner_second_branch_tt2_right");
      Check.ok(c14b.handb, "corner_second_branch_tt2_handb");
      Check.ok(!c14b.up, "corner_second_branch_tt2_up_false");

      // K15/K16: the lrswt/udswt flips in the other direction (:891-899,910-918)
      Preset m15 = new Preset();
      Madness md15 = mad(m15, 0);
      md15.pxy = 200;
      Control c15 = new Control(m15);
      c15.trickfase = K.intv("trickfase_air");
      c15.lrdirect = 1;
      c15.lrswt = true;
      c15.oxy = 0;
      c15.lrstart = 0;
      c15.preform(md15, car, cp(), new Trackers());
      Check.eq(c15.lrdirect, -1, "corner_lrswt_flips_1_to_minus1");
      Check.ok(c15.left, "corner_lrswt_flip_left");
      Preset m16 = new Preset();
      Madness md16 = mad(m16, 0);
      md16.pzy = 200;
      Control c16 = new Control(m16);
      c16.trickfase = K.intv("trickfase_air");
      c16.uddirect = 1;
      c16.udswt = true;
      c16.ozy = 0;
      c16.udstart = 0;
      c16.preform(md16, car, cp(), new Trackers());
      Check.eq(c16.uddirect, -1, "corner_udswt_flips_1_to_minus1");
      Check.ok(c16.down, "corner_udswt_flip_down");

      // K17: swat 0 turns a non-zero lcomp into lrbare (:950-952)
      Preset m17 = new Preset();
      Madness md17 = mad(m17, 0);
      md17.wtouch = true;
      md17.lcomp = 1.0F;
      Control c17 = new Control(m17);
      c17.trickfase = K.intv("trickfase_ground");
      c17.preform(md17, car, cp(), new Trackers());
      Check.ok(!c17.udbare, "corner_swat0_lcomp_udbare_false");
      Check.ok(c17.lrbare, "corner_swat0_lcomp_lrbare_true");
      Check.eq(c17.swat, 2, "corner_swat0_lcomp_swat2");

      // K18: the capsized swap's other direction (:966-969)
      Preset m18 = new Preset(7); // 0.6 > mustland
      Madness md18 = mad(m18, 0);
      md18.capsized = true;
      md18.dcomp = 1.0F;
      Control c18 = new Control(m18);
      c18.trickfase = K.intv("trickfase_ground");
      c18.swat = 2;
      c18.lrbare = true;
      c18.preform(md18, car, cp(), new Trackers());
      Check.ok(c18.udbare, "corner_swat2_swap_lrbare_to_udbare");
      Check.ok(!c18.lrbare, "corner_swat2_swap_lrbare_false");
      Check.eq(c18.swat, 3, "corner_swat2_swap_swat3");

      // K19: non-capsized + udcomp + perfection, var27 0 -> down (:1003-1009)
      Preset m19 = new Preset();
      Madness md19 = mad(m19, 0);
      md19.dcomp = 1.0F;
      md19.pzy = -90;
      Control c19 = new Control(m19);
      c19.trickfase = K.intv("trickfase_ground");
      c19.swat = 3;
      c19.udbare = true;
      c19.udcomp = true;
      c19.perfection = true;
      c19.preform(md19, car, cp(), new Trackers());
      Check.ok(c19.down, "corner_perfection_var27_0_down");
      Check.ok(!c19.up, "corner_perfection_var27_0_up_false");

      // K20: capsized + !udcomp + !onceu with dcomp<=ucomp -> up (:1017-1026)
      Preset m20 = new Preset();
      Madness md20 = mad(m20, 0);
      md20.capsized = true;
      Control c20 = new Control(m20);
      c20.trickfase = K.intv("trickfase_ground");
      c20.swat = 3;
      c20.udbare = true;
      c20.preform(md20, car, cp(), new Trackers());
      Check.ok(c20.up, "corner_capsized_else_up_true");
      Check.ok(!c20.down, "corner_capsized_else_down_false");

      // K21: non-capsized + udcomp + perfection var27 0 in the dcomp>ucomp
      // branch -> down (:1028-1035)
      Preset m21 = new Preset();
      Madness md21 = mad(m21, 0);
      md21.dcomp = 1.0F;
      md21.pzy = -90;
      Control c21 = new Control(m21);
      c21.trickfase = K.intv("trickfase_ground");
      c21.swat = 3;
      c21.udbare = true;
      c21.udcomp = true;
      c21.perfection = true;
      c21.preform(md21, car, cp(), new Trackers());
      Check.ok(c21.down, "corner_perfection_dcomp_upcomp_down");
      Check.ok(!c21.up, "corner_perfection_dcomp_upcomp_up_false");

      // K22: non-capsized + !udcomp with dcomp<=ucomp, draw > mustland -> down
      // and onceu is set (:1036-1040)
      Preset m22 = new Preset(7);
      Madness md22 = mad(m22, 0);
      md22.ucomp = 1.0F;
      Control c22 = new Control(m22);
      c22.trickfase = K.intv("trickfase_ground");
      c22.swat = 3;
      c22.udbare = true;
      c22.preform(md22, car, cp(), new Trackers());
      Check.ok(c22.down, "corner_random_udcomp_else_down_true");
      Check.ok(!c22.up, "corner_random_udcomp_else_up_false");
      Check.ok(c22.onceu, "corner_random_udcomp_else_onceu_true");

      // K23: var29 -90 walks BOTH normalisation loops (:1050-1056)
      Preset m23 = new Preset();
      Madness md23 = mad(m23, 0);
      md23.capsized = true;
      md23.lcomp = 10.0F;
      md23.pxy = -180;
      Control c23 = new Control(m23);
      c23.trickfase = K.intv("trickfase_ground");
      c23.swat = 3;
      c23.lrbare = true;
      c23.preform(md23, car, cp(), new Trackers());
      Check.ok(c23.left, "corner_lr_var29_wrap_left");

      // K24: non-capsized + lcomp<=rcomp + lrcomp + perfection var29 0 ->
      // right (:1099-1107)
      Preset m24 = new Preset();
      Madness md24 = mad(m24, 0);
      md24.rcomp = 1.0F;
      md24.pxy = -90;
      Control c24 = new Control(m24);
      c24.trickfase = K.intv("trickfase_ground");
      c24.swat = 3;
      c24.lrbare = true;
      c24.lrcomp = true;
      c24.perfection = true;
      c24.preform(md24, car, cp(), new Trackers());
      Check.ok(c24.right, "corner_perfection_lr_else_right");
      Check.ok(!c24.left, "corner_perfection_lr_else_left_false");

      // K25/K26: capsized lcomp<=rcomp with lrcomp, var29 135 -> left and
      // var29 0 -> right (:1089-1095)
      Preset m25 = new Preset();
      Madness md25 = mad(m25, 0);
      md25.capsized = true;
      md25.rcomp = 1.0F;
      md25.pxy = 45;
      Control c25 = new Control(m25);
      c25.trickfase = K.intv("trickfase_ground");
      c25.swat = 3;
      c25.lrbare = true;
      c25.lrcomp = true;
      c25.preform(md25, car, cp(), new Trackers());
      Check.ok(c25.left, "corner_capsized_else_lrcomp_var29_135_left");
      Check.ok(!c25.right, "corner_capsized_else_lrcomp_right_false");
      Preset m26 = new Preset();
      Madness md26 = mad(m26, 0);
      md26.capsized = true;
      md26.rcomp = 1.0F;
      md26.pxy = -90;
      Control c26 = new Control(m26);
      c26.trickfase = K.intv("trickfase_ground");
      c26.swat = 3;
      c26.lrbare = true;
      c26.lrcomp = true;
      c26.preform(md26, car, cp(), new Trackers());
      Check.ok(c26.right, "corner_capsized_else_lrcomp_var29_0_right");
      Check.ok(!c26.left, "corner_capsized_else_lrcomp_left_false");
      // K27: |var10-var12| > 180 folds back through :353-355; the resulting
      // var13 20 still lets the :438 clearing branch fire
      Preset m27 = new Preset();
      Madness md27 = mad(m27, 0);
      CheckPoints p27 = cp();
      p27.stage = 2;
      p27.pos[0] = 4;
      p27.opx[1] = 100;
      p27.opz[1] = -567;
      Control c27 = new Control(m27);
      c27.afta = true;
      carAt(0, 0, 0, 170);
      plan(md27, c27, p27, new Trackers());
      Check.eq(c27.clrnce, 0, "corner_var13_wrap_clrnce_0");
      Check.eq(c27.acuracy, 0, "corner_var13_wrap_acuracy_0");
      Check.eq(m27.draws, 12, "corner_var13_wrap_twelve_draws");

      // K28: a point>0 start runs the initial positive-count scan (:585-595)
      Preset m28 = new Preset();
      Madness md28 = mad(m28, 0);
      md28.point = 2;
      md28.clear = 2;
      CheckPoints p28 = cpBul();
      p28.x[2] = -100;
      Control c28 = new Control(m28);
      c28.skiplev = -1.0F;
      c28.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c28, md28, p28);
      Check.eq(c28.pan, 90, "corner_initial_count_scan_keeps_point_2");
      Check.eq(m28.draws, 2, "corner_initial_count_scan_two_draws");

      // K29: rampp 2 underflow - the skip walk wraps point 4 to 0, so the
      // var23 != point guard passes and --var23 wraps back to 4 (:618-629)
      Preset m29 = new Preset(1); // 0.0, 0.9, ...
      Madness md29 = mad(m29, 0);
      md29.point = 4;
      CheckPoints p29 = cpBul();
      p29.typ[4] = -1;
      p29.typ[1] = -2;
      p29.x[4] = -100;
      Control c29 = new Control(m29);
      c29.rampp = 2;
      c29.skiplev = 0.4F;
      c29.turncnt = 1;
      carAt(0, 0, 0, 0);
      touch(c29, md29, p29);
      Check.eq(c29.pan, 90, "corner_rampp2_underflow_wraps_to_index4");
      Check.eq(m29.draws, 3, "corner_rampp2_underflow_three_draws");

   }

   // ------------------------------------------------------------------- errors

   /** B7(76-81) - the missing bounds/null guards, documented not fixed. */
   private static void errorPins() {
      // B7(76): pos[5] on the plan gate
      Check.throwsIt(new Check.Task() {
         public void run() {
            Preset m = new Preset();
            Madness md = mad(m, 5);
            md.mtouch = true;
            Control c = new Control(m);
            plan(md, c, cp(), new Trackers());
         }
      }, ArrayIndexOutOfBoundsException.class, "err_im5_pos5_aioobe");

      // B7(77): maxmag[10] in the perfection ratio
      Check.throwsIt(new Check.Task() {
         public void run() {
            Preset m = new Preset();
            Madness md = mad(m, 0);
            md.mtouch = true;
            md.cn = 10;
            Control c = new Control(m);
            plan(md, c, cp(), new Trackers());
         }
      }, ArrayIndexOutOfBoundsException.class, "err_cn10_maxmag10_aioobe");

      // B7(78): the first m.random() of the plan (:134)
      Check.throwsIt(new Check.Task() {
         public void run() {
            Madness md = mad(null, 0);
            md.mtouch = true;
            Control c = new Control(null);
            plan(md, c, cp(), new Trackers());
         }
      }, NullPointerException.class, "err_null_medium_first_random_npe");

      // B7(79a): CheckPoints null at :74
      Check.throwsIt(new Check.Task() {
         public void run() {
            Preset m = new Preset();
            Madness md = mad(m, 0);
            md.mtouch = true;
            Control c = new Control(m);
            plan(md, c, null, new Trackers());
         }
      }, NullPointerException.class, "err_null_checkpoints_npe");

      // B7(79b): ContO null at var2.xz (:321-322) when attack==0
      Check.throwsIt(new Check.Task() {
         public void run() {
            Preset m = new Preset();
            Madness md = mad(m, 0);
            md.mtouch = true;
            Control c = new Control(m);
            c.preform(md, null, cp(), new Trackers());
         }
      }, NullPointerException.class, "err_null_conto_xz_npe");

      // B7(79c): Trackers null at the wall read (:783)
      Check.throwsIt(new Check.Task() {
         public void run() {
            Preset m = new Preset();
            Madness md = mad(m, 0);
            md.mtouch = true;
            Control c = new Control(m);
            c.wall = 0;
            c.preform(md, car, cp(), null);
         }
      }, NullPointerException.class, "err_null_trackers_dam_npe");

      // B7(80): dam[wall] bounds - 500 and -2 both escape the 500-entry array
      for (final int wall : new int[]{500, -2}) {
         Check.throwsIt(new Check.Task() {
            public void run() {
               Preset m = new Preset();
               Madness md = mad(m, 0);
               md.mtouch = true;
               Control c = new Control(m);
               c.wall = wall;
               c.preform(md, car, cp(), new Trackers());
            }
         }, ArrayIndexOutOfBoundsException.class, "err_wall" + wall + "_dam_aioobe");
      }

      // B7(81): reset's second loop writes fpnt[5]
      Check.throwsIt(new Check.Task() {
         public void run() {
            CheckPoints p = new CheckPoints();
            p.fn = 6;
            p.n = 1;
            new Control(null).reset(p);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_reset_fn6_fpnt5_aioobe");
   }

   // -------------------------------------------------------------------- input

   /** B7(82-87) / GameSparker.java:266-308,1519-1578 - the thin key/mouse
    *  handlers. new GameSparker() is asset-free (class header). */
   private static void inputPins() {
      GameSparker g = new GameSparker();
      Check.ok(g.exwist, "input_default_exwist_true");
      Check.eq(g.u[0], null, "input_default_u0_null");
      Check.eq(g.view, 0, "input_default_view_0");
      Check.eq(g.mouses, 0, "input_default_mouses_0");
      Check.ok(g.gamer == null, "input_no_thread_started");

      Control u = new Control(null);
      g.u[0] = u;
      Event e = new Event(new Object(), 0L, 0, 0, 0, 0, 0);

      // B7(84): the default exwist==true makes both handlers no-ops
      Check.ok(!g.keyDown(e, 1004), "input_keydown_returns_false");
      Check.ok(!u.up, "input_exwist_true_keydown_noop");
      u.up = true;
      Check.ok(!g.keyUp(e, 1004), "input_keyup_returns_false");
      Check.ok(u.up, "input_exwist_true_keyup_noop");
      u.up = false;

      // B7(82): exwist false -> the five drive flags
      g.exwist = false;
      Check.ok(!g.keyDown(e, 1004), "input_keydown_1004_false");
      Check.ok(u.up, "input_keydown_1004_up");
      g.keyDown(e, 1005);
      Check.ok(u.down, "input_keydown_1005_down");
      g.keyDown(e, 1007);
      Check.ok(u.right, "input_keydown_1007_right");
      g.keyDown(e, 1006);
      Check.ok(u.left, "input_keydown_1006_left");
      g.keyDown(e, 32);
      Check.ok(u.handb, "input_keydown_32_handb");

      // B7(83): keyUp clears the same five
      Check.ok(!g.keyUp(e, 1004), "input_keyup_1004_false");
      Check.ok(!u.up, "input_keyup_1004_up_cleared");
      g.keyUp(e, 1005);
      Check.ok(!u.down, "input_keyup_1005_down_cleared");
      g.keyUp(e, 1007);
      Check.ok(!u.right, "input_keyup_1007_right_cleared");
      g.keyUp(e, 1006);
      Check.ok(!u.left, "input_keyup_1006_left_cleared");
      g.keyUp(e, 32);
      Check.ok(!u.handb, "input_keyup_32_handb_cleared");

      // B7(85): enter keys set enter; keyUp never clears it
      g.keyDown(e, 10);
      Check.ok(u.enter, "input_keydown_10_enter");
      g.keyUp(e, 10);
      Check.ok(u.enter, "input_keyup_10_does_not_clear_enter");
      u.enter = false;
      g.keyDown(e, 80);
      Check.ok(u.enter, "input_keydown_80_enter");
      u.enter = false;
      g.keyDown(e, 112);
      Check.ok(u.enter, "input_keydown_112_enter");
      u.enter = false;
      g.keyDown(e, 27);
      Check.ok(u.enter, "input_keydown_27_enter");

      // B7(86): toggles and the view wrap
      Check.ok(!u.arrace, "input_arrace_starts_false");
      g.keyDown(e, 65);
      Check.ok(u.arrace, "input_keydown_65_arrace_toggle_on");
      g.keyDown(e, 97);
      Check.ok(!u.arrace, "input_keydown_97_arrace_toggle_off");
      g.keyDown(e, 77);
      Check.ok(u.mutem, "input_keydown_77_mutem_toggle_on");
      g.keyDown(e, 109);
      Check.ok(!u.mutem, "input_keydown_109_mutem_toggle_off");
      g.keyDown(e, 78);
      Check.ok(u.mutes, "input_keydown_78_mutes_toggle_on");
      g.keyDown(e, 110);
      Check.ok(!u.mutes, "input_keydown_110_mutes_toggle_off");
      g.view = 2;
      g.keyDown(e, 118);
      Check.eq(g.view, 0, "input_keydown_118_view_3_wraps_0");
      g.keyDown(e, 86);
      Check.eq(g.view, 1, "input_keydown_86_view_increments_1");

      // B7(87): mouseDown latches the first press and touches no Control field
      String before = snapshot(u);
      g.mouses = 0;
      g.mouseDown(e, 12, 34);
      Check.eq(g.mouses, 1, "input_mousedown_mouses_1");
      Check.eq(g.xm, 12, "input_mousedown_xm_12");
      Check.eq(g.ym, 34, "input_mousedown_ym_34");
      g.mouseDown(e, 99, 99);
      Check.eq(g.mouses, 1, "input_mousedown_second_ignored_mouses");
      Check.eq(g.xm, 12, "input_mousedown_second_ignored_xm");
      Check.eq(g.ym, 34, "input_mousedown_second_ignored_ym");
      Check.eq(snapshot(u), before, "input_mousedown_touches_no_control_field");
      g.exwist = true;
      g.mouses = 0;
      g.mouseDown(e, 55, 66);
      Check.eq(g.mouses, 0, "input_mousedown_exwist_noop");
   }

   // ------------------------------------------------------------------ helpers

   /** The B7 seam recipe: cntrn, the rand triple, diup and trn live in
    *  tests/fixtures/toml/determinism.toml [recipes] [control_preset]
    *  (single source of truth; the expected walk is pinned from the same
    *  table). */
   static final Toml RECIPE = Toml.load("tests/fixtures/toml/determinism.toml")
         .table("recipes").table("control_preset");

   /** The B7 seam: preset the real Medium.random() walk (Medium.java:49-89). */
   static class Preset extends Medium {
      int draws;

      Preset() {
         // the recipe's rand triple is uniform; Preset(int) sets all three
         this(RECIPE.intArr("rand")[0]);
      }

      Preset(int start) {
         cntrn = RECIPE.intv("cntrn");
         rand[0] = start;
         rand[1] = start;
         rand[2] = start;
         boolean diupValue = RECIPE.boolv("diup");
         diup[0] = diupValue;
         diup[1] = diupValue;
         diup[2] = diupValue;
         trn = RECIPE.intv("trn");
      }

      public float random() {
         draws++;
         return super.random();
      }
   }

   /** A fresh Madness with xt==null (preform never dereferences xt) and the
    *  documented cn==0 / im fixture. */
   private static Madness mad(Medium m, int im) {
      Madness md = new Madness(m, new Record(m), null, im);
      md.cn = 0;
      return md;
   }

   /** Open the plan gate (:73) the way the real loop does: stcnt is driven
    *  past statusque, then preform consumes it. */
   private static void plan(Madness md, Control c, CheckPoints cp, Trackers t) {
      md.mtouch = true;
      c.stcnt = 1;
      c.statusque = 0;
      c.preform(md, car, cp, t);
   }

   /** One touch-branch pass with the plan gate closed (stcnt==statusque==0). */
   private static void touch(Control c, Madness md, CheckPoints cp) {
      md.mtouch = true;
      c.preform(md, car, cp, new Trackers());
   }

   /** The shared ContO seam is repositioned per fixture; Control reads only
    *  x/y/z/xz and writes no ContO field. */
   private static void carAt(int x, int y, int z, int xz) {
      car.x = x;
      car.y = y;
      car.z = z;
      car.xz = xz;
   }

   /** One positive track point so the touch-branch path select terminates
    *  (Control.java:597-615 needs typ[*]>0 and n>0). */
   private static CheckPoints cp() {
      CheckPoints p = new CheckPoints();
      p.n = 1;
      p.typ[0] = 1;
      p.x[0] = 0;
      p.z[0] = 0;
      return p;
   }

   /** Five positive track points for the bulistc branch, which does
    *  var23 = point-2 (:540) and must stay in range. */
   private static CheckPoints cpBul() {
      CheckPoints p = new CheckPoints();
      p.n = 5;
      for (int i = 0; i < 5; i++) {
         p.typ[i] = 1;
      }
      return p;
   }

   private static int bits(float f) {
      return Float.floatToIntBits(f);
   }

   /** Source-declared fields only (JaCoCo adds a synthetic $jacocoData). */
   private static int declaredFieldCount(Class<?> c) {
      int n = 0;
      for (java.lang.reflect.Field f : c.getDeclaredFields()) {
         if (!f.isSynthetic()) {
            ++n;
         }
      }
      return n;
   }

   /** Every Control field except m, so "nothing else changed" is one compare. */
   private static String snapshot(Control c) {
      return c.left + "" + c.right + c.up + c.down + c.handb + c.enter
            + c.arrace + c.mutem + c.mutes
            + "|" + c.pan + "," + c.attack + "," + c.acr + "," + c.afta
            + "," + Arrays.toString(c.fpnt) + "," + c.trfix + "," + c.forget
            + "," + c.bulistc + "," + c.acuracy + "," + c.upwait + "," + c.agressed
            + "," + bits(c.skiplev) + "," + c.clrnce + "," + c.rampp + "," + c.turntyp
            + "," + c.saftey + "," + c.perfection + "," + bits(c.mustland)
            + "," + c.usebounce + "," + bits(c.trickprf) + "," + c.zyinv + "," + c.lastl
            + "," + c.wlastl + "," + c.hold + "," + c.wall + "," + c.lwall + "," + c.stcnt
            + "," + c.statusque + "," + c.turncnt + "," + c.randtcnt + "," + c.upcnt
            + "," + c.trickfase + "," + c.swat + "," + c.udcomp + "," + c.lrcomp
            + "," + c.udbare + "," + c.lrbare + "," + c.onceu + "," + c.onced
            + "," + c.oncel + "," + c.oncer + "," + c.lrdirect + "," + c.uddirect
            + "," + c.lrstart + "," + c.udstart + "," + c.oxy + "," + c.ozy
            + "," + c.flycnt + "," + c.lrswt + "," + c.udswt + "," + c.actwait
            + "," + c.cntrn;
   }
}
