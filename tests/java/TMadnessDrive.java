/**
 * Drives the ORIGINAL Madness.drive(Control,ContO,Trackers,CheckPoints)
 * over MULTI-TICK runs and pins the observable state at chosen ticks.
 *
 * Position in Suite: APPENDED after TAudioGoldens. Safe because every
 * random() call goes through VecMedium, a Medium subclass whose
 * random() returns a repeating 12-value vector (the TRecord.TenthsMedium
 * pattern, sanctioned for drive loops), so no draw depends on the order
 * of earlier tests, on Suite slot layout, or on JIT timing. Math.random
 * is never consulted here.
 *
 * Physics notes pinned by measurements (all values below were read off
 * this exact fixture, then asserted):
 *   - the ground clamp is var21 > 245 where var21 = grat + y + scy
 *     (Madness.java:1188,1418), so a fixture at y==240 touches ground
 *     on its first tick;
 *   - mtouch is only set by tracker contact (:1742/:1751) or the
 *     four-wheel attitude snap (:1944), never by the ground clamp;
 *   - im!=0 keeps every xtGraphics site unreachable (:164-166,
 *     :302-304, :662-664 gate on im==0||colidim; :1357 and
 *     :2231/:2244 gate on im==0), which is why the fixtures use im==1
 *     except where an im==0 arm is the point;
 *   - nofocus is re-armed before each tick because :2219 clears it.
 */
public class TMadnessDrive {
   /** Repeating pseudo-random stream: 12 values, then it wraps.
    *  The vector is tests/fixtures/toml/determinism.toml [vectors] vecmedium
    *  (the single source of truth); the static read is a few KB, on first
    *  use mid-suite, so no earlier pin's ordering changes. */
   static class VecMedium extends Medium {
      private static final float[] VEC = Toml.load("tests/fixtures/toml/determinism.toml")
            .table("vectors").floatArr("vecmedium");
      int draws;

      public float random() {
         float v = VEC[draws % VEC.length];
         draws++;
         return v;
      }
   }

   /** The scenario scripts and the load-bearing game constants. One parse
    *  each, on first use mid-suite (Toml consumes no Math.random, so no
    *  earlier pin's ordering changes). */
   static final Toml SC = Toml.load("tests/fixtures/toml/scenarios.toml");
   static final Toml MAD = Toml.load("tests/fixtures/toml/constants.toml").table("madness");

   /** A phase flag: absent means false (the Control initializer). */
   static boolean flag(Toml ph, String key) {
      return ph.has(key) && ph.boolv(key);
   }

   static void run() throws Exception {
      Check.group("MadnessDrive");
      // shape pin: the wrap period is part of the pinned behavior
      Check.eq(VecMedium.VEC.length, 12, "vecmedium_len_12");
      // shape pins: the fixture scripts and the constant table are present
      Check.eq(SC.tableCount("scenario"), 4, "scenarios_rows_4");
      MAD.requireKeys("power_default", "wall_recoil_cap", "capsize_detect",
            "powerup_travxy_div", "trick_rotate", "ground_clamp", "focus_scan_len");
      Medium m = new VecMedium();

      // ---- a. a 400-tick race over floor tiles, walls and slopes ----
      // Script + expectations: scenarios.toml [[scenario]] id="race" (course
      // geometry, phase flags, pinned ticks). This is the integration pump:
      // floors of every skid class, side walls (xy=+-90), sloped tiles
      // (xy=45) and a 6-checkpoint course.
      Toml race = SC.tableAt("scenario", 0);
      Check.eq(race.str("id"), "race", "race_scenario_first");
      Trackers tr = new Trackers();
      int nt = 0;
      for (int i = 0; i < race.intv("course_floors"); i++) {
         floor(tr, nt++, 0, race.intv("floor_y"),
               race.intv("floor_z_base") + i * race.intv("floor_z_step"),
               i % race.intv("floor_skd_mod"), race.intv("floor_dam"),
               race.intv("floor_radx"), race.intv("floor_radz"), race.intv("floor_rady"));
      }
      for (int i = 0; i < race.intv("course_walls"); i++) {
         int wz = race.intv("wall_z_base") + i * race.intv("wall_z_step");
         wallx(tr, nt++, race.intv("wall_x"), race.intv("wall_y"), wz, race.intv("wall_xy"),
               race.intv("wall_radx"), race.intv("wall_radz"), race.intv("wall_rady"),
               race.intv("wall_dam"));
         wallx(tr, nt++, -race.intv("wall_x"), race.intv("wall_y"), wz, -race.intv("wall_xy"),
               race.intv("wall_radx"), race.intv("wall_radz"), race.intv("wall_rady"),
               race.intv("wall_dam"));
      }
      for (int i = 0; i < race.intv("course_slopes"); i++) {
         int idx = nt++;
         tr.x[idx] = race.intv("slope_x");
         tr.y[idx] = race.intv("slope_y");
         tr.z[idx] = race.intv("slope_z_base") + i * race.intv("slope_z_step")
               + race.intv("slope_z_mid");
         tr.xy[idx] = race.intv("slope_xy");
         tr.zy[idx] = 0;
         tr.skd[idx] = 0;
         tr.dam[idx] = race.intv("slope_dam");
         tr.radx[idx] = race.intv("slope_radx");
         tr.radz[idx] = race.intv("slope_radz");
         tr.rady[idx] = race.intv("slope_rady");
      }
      tr.nt = nt;
      CheckPoints cp = new CheckPoints();
      cp.n = race.intv("checkpoints");
      cp.nsp = race.intv("checkpoints");
      for (int i = 0; i < race.intv("checkpoints"); i++) {
         cp.typ[i] = 1;
         cp.x[i] = race.intv("checkpoint_x");
         cp.z[i] = race.intv("checkpoint_z_base") + i * race.intv("checkpoint_z_step");
         cp.y[i] = race.intv("checkpoint_y");
      }
      cp.typ[race.intv("checkpoint_typ2_index")] = 2; // a z-axis checkpoint in the same course
      ContO c = car(m);
      c.y = race.intv("start_y");
      Madness md = new Madness(m, new Record(m), null, race.intv("im"));
      md.cn = race.intv("cn");
      md.power = race.floatv("power");
      Control ctrl = new Control(m);
      Toml rp = race.tableAt("phase", 0);
      for (int t = 0; t < race.intv("ticks"); t++) {
         ctrl.up = flag(rp, "up");
         ctrl.left = (t % rp.intv("left_period")) < rp.intv("left_ticks");
         ctrl.right = (t % rp.intv("right_period")) < rp.intv("right_ticks");
         ctrl.handb = (t % rp.intv("handb_period")) < rp.intv("handb_ticks");
         ctrl.down = (t % rp.intv("down_period")) < rp.intv("down_ticks");
         ctrl.wall = rp.intv("wall");
         md.nofocus = (t % rp.intv("nofocus_period")) != rp.intv("nofocus_zero");
         md.drive(ctrl, c, tr, cp);
         if (t == 0) {
            Toml e = race.table("at0");
            Check.eq(c.y, e.intv("y"), "race_t0_y_170");
            Check.eq(c.x, e.intv("x"), "race_t0_x_70");
            Check.ok(md.mtouch == e.boolv("mtouch"), "race_t0_mtouch");
            Check.eq(md.speed, e.floatv("speed"), "race_t0_speed_0");
         }
         if (t == 40) {
            Toml e = race.table("at40");
            Check.eq(c.y, e.intv("y"), "race_t40_y_195");
            Check.eq(c.x, e.intv("x"), "race_t40_x_minus85");
            Check.eq(c.z, e.intv("z"), "race_t40_z_3550");
            Check.eq(md.speed, e.floatv("speed"), "race_t40_speed_185_842");
            Check.eq(md.trcnt, e.intv("trcnt"), "race_t40_trcnt_1");
         }
         if (t == 80) {
            Toml e = race.table("at80");
            Check.eq(c.y, e.intv("y"), "race_t80_y_248");
            Check.eq(c.x, e.intv("x"), "race_t80_x_284");
            Check.eq(c.z, e.intv("z"), "race_t80_z_10245");
         }
      }
      Check.ok(md.mtouch == race.table("end").boolv("mtouch"), "race_end_mtouch");

      // ---- a2. free roll on flat ground: the ground clamp and the
      // power decay (Madness.java:1417-1461,2404-2410) ----
      // Tick 0: scy 0 -> +7 (:1190) -> 247 > 245 -> clamp to 250,
      // wtouch/gtouch true, regy(7*1.2) then scy = 7-8.4 = -1.4000006,
      // and power = 98 - 98^3/2500000 = 97.62352 (:2407).
      Toml rollSc = SC.tableAt("scenario", 1);
      Check.eq(rollSc.str("id"), "roll", "roll_scenario_second");
      ContO roll = car(m);
      roll.y = rollSc.intv("start_y");
      Madness rm = mk(m, new Record(m), rollSc.intv("im"), rollSc.floatv("power"));
      rm.nofocus = true;
      rm.drive(new Control(m), roll, new Trackers(), new CheckPoints());
      Toml r0 = rollSc.table("at0");
      Check.eq(roll.y, r0.intv("y"), "roll_t0_y_250");
      Check.eq(rm.scy[0], r0.floatv("scy0"), "roll_t0_scy_minus1_4");
      Check.eq(rm.scy[3], r0.floatv("scy3"), "roll_t0_scy3_minus1_4");
      Check.eq(rm.power, r0.floatv("power"), "roll_t0_power_97_62352");
      Check.ok(rm.wtouch == r0.boolv("wtouch"), "roll_t0_wtouch");
      Check.ok(rm.gtouch == r0.boolv("gtouch"), "roll_t0_gtouch");
      Check.eq(rm.skid, r0.intv("skid"), "roll_t0_skid_2");
      Check.eq(rm.trcnt, r0.intv("trcnt"), "roll_t0_trcnt_0");
      // Tick 1: mtouch at entry pulls scy to 0 (:1318-1326), the clamp
      // then sees 5.5999994 but scy is already 0, and power keeps
      // decaying (:2407) -> 97.251366.
      rm.nofocus = true;
      rm.drive(new Control(m), roll, new Trackers(), new CheckPoints());
      Toml r1 = rollSc.table("at1");
      Check.eq(rm.scy[0], r1.floatv("scy0"), "roll_t1_scy_0");
      Check.eq(rm.power, r1.floatv("power"), "roll_t1_power_97_251366");
      Check.eq(rm.skid, r1.intv("skid"), "roll_t1_skid_0");
      for (int i = 2; i < rollSc.intv("ticks"); i++) {
         rm.nofocus = true;
         rm.drive(new Control(m), roll, new Trackers(), new CheckPoints());
      }
      Toml r11 = rollSc.table("at11");
      Check.eq(rm.power, r11.floatv("power"), "roll_t11_power_93_750465");
      Check.eq(rm.speed, r11.floatv("speed"), "roll_t11_speed_0");
      Check.eq(roll.y, r11.intv("y"), "roll_t11_y_250");

      // ---- b. air control: handb in the air starts loop 1 -> 2 ----
      // With wtouch false :819 sets loop=1, then :830-838 averages scy
      // to 7 and sets loop=2 in the SAME tick. up+left then runs the
      // :842-1073 air block: ucomp 10.5, lcomp 7, pzy -10, pxy -7.
      Toml airSc = SC.tableAt("scenario", 2);
      Check.eq(airSc.str("id"), "air_handb", "air_scenario_third");
      ContO air = car(m);
      air.y = airSc.intv("start_y");
      Madness am = mk(m, new Record(m), airSc.intv("im"), airSc.floatv("power"));
      Control ac = new Control(m);
      for (int t = 0; t < airSc.intv("ticks"); t++) {
         Toml ph = airSc.tableAt("phase", t);
         ac.handb = flag(ph, "handb");
         ac.up = flag(ph, "up");
         ac.left = flag(ph, "left");
         ac.down = flag(ph, "down");
         ac.right = flag(ph, "right");
         am.nofocus = true;
         am.drive(ac, air, new Trackers(), new CheckPoints());
         if (t == 0) {
            Toml e = airSc.table("at0");
            Check.eq(am.loop, e.intv("loop"), "air_handb_loop_2");
            Check.eq(am.scy[0], e.floatv("scy0"), "air_handb_scy_average_7");
            Check.eq(am.scy[3], e.floatv("scy3"), "air_handb_scy3_average_7");
         }
         if (t == 1) {
            Check.eq(am.loop, airSc.table("at1").intv("loop"), "air_handb_second_tick_loop_2");
         }
         if (t == 2) {
            Toml e = airSc.table("at2");
            Check.eq(am.ucomp, e.floatv("ucomp"), "air_up_ucomp_10_5");
            Check.eq(am.lcomp, e.floatv("lcomp"), "air_left_lcomp_7");
            Check.eq(am.pzy, e.intv("pzy"), "air_up_pzy_minus10");
            Check.eq(am.pxy, e.intv("pxy"), "air_left_pxy_minus7");
            Check.eq(air.y, e.intv("y"), "air_up_y_92");
         }
         if (t == 3) {
            Toml e = airSc.table("at3");
            Check.eq(am.ucomp, e.floatv("ucomp"), "air_release_ucomp_decays_10");
            Check.eq(am.lcomp, e.floatv("lcomp"), "air_release_lcomp_decays_5");
         }
         if (t == 4) {
            Toml e = airSc.table("at4");
            Check.ok(e.boolv("dcomp_positive") == (am.dcomp > 0.0F), "air_down_dcomp_positive");
            Check.eq(am.rcomp, e.floatv("rcomp"), "air_right_rcomp_7");
            Check.eq(am.pxy, e.intv("pxy"), "air_right_pxy_minus8");
         }
      }

      // ---- b2. loop==-1 fixes the press flags (the :790-813 block) ----
      // wtouch with loop==2 -> loop=-1, and left/right/up/down latch the
      // pl/pr/pu/pd flags ONCE. The four rcomp/lcomp resets happen too.
      // b2 stays a one-tick branch fixture (not a script): the wtouch latch.
      ContO lc = car(m);
      lc.y = 50;
      Madness lm = mk(m, new Record(m), 1, 98.0F);
      lm.loop = 2;
      lm.wtouch = true;
      Control lcc = new Control(m);
      lcc.left = true;
      lcc.up = true;
      lcc.down = true;
      lcc.right = true;
      lm.nofocus = true;
      lm.drive(lcc, lc, new Trackers(), new CheckPoints());
      Check.eq(lm.loop, -1, "loopm_wtouch_loop_minus1");
      Check.ok(lm.pl, "loopm_pl_latched");
      Check.ok(lm.pr, "loopm_pr_latched");
      Check.ok(lm.pu, "loopm_pu_latched");
      Check.ok(lm.pd, "loopm_pd_latched");

      // b3. the loop==-1 airborne block (:992-1073): y<100 required
      Toml lmSc = SC.tableAt("scenario", 3);
      Check.eq(lmSc.str("id"), "loopminus_air", "loopminus_scenario_fourth");
      ContO la = car(m);
      la.y = lmSc.intv("start_y");
      Madness l2 = mk(m, new Record(m), lmSc.intv("im"), lmSc.floatv("power"));
      l2.loop = lmSc.intv("loop");
      if (lmSc.has("wtouch")) {
         l2.wtouch = lmSc.boolv("wtouch");
      }
      Control l2c = new Control(m);
      for (int t = 0; t < lmSc.intv("ticks"); t++) {
         Toml ph = lmSc.tableAt("phase", t);
         l2c.left = flag(ph, "left");
         l2c.up = flag(ph, "up");
         l2c.down = flag(ph, "down");
         l2c.right = flag(ph, "right");
         l2.nofocus = true;
         l2.drive(l2c, la, new Trackers(), new CheckPoints());
         if (t == 0) {
            Toml e = lmSc.table("at0");
            Check.eq(l2.lcomp, e.floatv("lcomp"), "loopminus_left_lcomp_7");
            Check.eq(l2.ucomp, e.floatv("ucomp"), "loopminus_up_ucomp_7");
            Check.eq(l2.pxy, e.intv("pxy"), "loopminus_pxy_minus7");
            Check.eq(l2.pzy, e.intv("pzy"), "loopminus_pzy_minus7");
         }
         if (t == 1) {
            Toml e = lmSc.table("at1");
            Check.eq(l2.lcomp, e.floatv("lcomp"), "loopminus_left_lcomp_9");
            Check.eq(la.y, e.intv("y"), "loopminus_y_71");
         }
         if (t == 2) {
            Toml e = lmSc.table("at2");
            Check.eq(l2.pxy, e.intv("pxy"), "loopminus_pxy_minus27");
            Check.eq(la.y, e.intv("y"), "loopminus_y_92");
         }
      }

      // ---- c. wall collisions through colide()-style trackers ----
      // Each flat wall branch (:1528 zy=-90, :1561 zy=90, :1594 xy=-90,
      // :1627 xy=90) pins wall index, skid=2, the signed impulse pull and the
      // regx/regz magnitude.
      for (int axis = 0; axis < 2; axis++) {
         for (int sgn = 0; sgn < 2; sgn++) {
            ContO w = car(m);
            w.y = 240;
            Trackers wt = new Trackers();
            wt.nt = 1;
            Madness wm = mk(m, new Record(m), 1, 98.0F);
            Control wc = new Control(m);
            if (axis == 0) {
               wallx(wt, 0, 0, 240, 0, sgn == 0 ? 90 : -90, 100, 100, 60, 20);
               float v = sgn == 0 ? 5.0F : -5.0F;
               for (int i = 0; i < 4; i++) {
                  wm.scx[i] = v;
               }
            } else {
               wallz(wt, 0, 0, 240, 0, sgn == 0 ? 90 : -90, 100, 100, 60, 20);
               float v = sgn == 0 ? 5.0F : -5.0F;
               for (int i = 0; i < 4; i++) {
                  wm.scz[i] = v;
               }
            }
            wm.nofocus = true;
            wm.drive(wc, w, wt, new CheckPoints());
            Check.eq(wm.skid, 2, "wall_" + (axis == 0 ? "xy" : "zy") + (sgn == 0 ? "p" : "n") + "_skid2");
            Check.eq(wc.wall, 0, "wall_" + (axis == 0 ? "xy" : "zy") + (sgn == 0 ? "p" : "n") + "_wall_index");
            if (axis == 0) {
               Check.eq(wm.scx[0], sgn == 0 ? -1.5000005F : 1.5000005F,
                     "wall_xy" + (sgn == 0 ? "p" : "n") + "_scx_recoil");
            } else {
               Check.eq(wm.scz[0], sgn == 0 ? -1.5000005F : 1.5000005F,
                     "wall_zy" + (sgn == 0 ? "p" : "n") + "_scz_recoil");
            }
         }
      }

      // c2. a hard wall hit: damage 50*1.3*dam(20) = 1300 > 100 records
      // through regz and flips pmlt (xz==0 -> var109 -45 -> nmlt -1).
      ContO hw = car(m);
      hw.y = 240;
      Trackers hwt = new Trackers();
      hwt.nt = 1;
      wallz(hwt, 0, 0, 240, 0, 90, 100, 100, 60, 20);
      Record hwr = new Record(m);
      Madness hwm = mk(m, hwr, 1, 98.0F);
      for (int i = 0; i < 4; i++) {
         hwm.scz[i] = 50.0F;
      }
      hwm.speed = 50.0F;
      Control hwc = new Control(m);
      hwm.nofocus = true;
      hwm.drive(hwc, hw, hwt, new CheckPoints());
      Check.eq(hwc.wall, 0, "hardwall_wall_index");
      Check.eq(hwr.nrz[1][0], 1, "hardwall_regz_recorded");
      Check.eq(hwr.magz[1][0][0], -1300, "hardwall_magz_minus1300");
      Check.eq(hwm.scz[0], -15.0F, "hardwall_scz_minus15");
      Check.eq(hwm.pmlt, -1, "hardwall_pmlt_minus1");
      Check.eq(hwm.nmlt, -1, "hardwall_nmlt_minus1");
      Check.eq(hw.xz, 0, "hardwall_xz_unchanged");

      // c3. sloped tracker tiles (xy=45, zy=45): the :1660-1738 blocks
      for (int axis = 0; axis < 2; axis++) {
         ContO sl = car(m);
         sl.y = 240;
         Trackers st = new Trackers();
         st.nt = 1;
         if (axis == 0) {
            wallz(st, 0, 0, 240, 0, 45, 100, 100, 60, 20);
         } else {
            wallx(st, 0, 0, 240, 0, 45, 100, 100, 60, 20);
         }
         Madness sm = mk(m, new Record(m), 1, 98.0F);
         sm.nofocus = true;
         sm.drive(new Control(m), sl, st, new CheckPoints());
         Check.eq(sm.scy[0], -7.460916F, "slope" + axis + "_scy_minus7_46");
         Check.ok(sm.mtouch, "slope" + axis + "_mtouch");
         Check.ok(sm.wtouch, "slope" + axis + "_wtouch");
         Check.eq(sl.y, 245, "slope" + axis + "_y_245");
      }

      // ---- d. capsize / attitude normalisation (:738-780,1899-1947) ----
      // pzy=180,pxy=0: var7 only -> capsized, flipy installs -50
      ContO cap = car(m);
      cap.y = 250;
      Madness cm = mk(m, new Record(m), 1, 98.0F);
      cm.pzy = MAD.intv("capsnap_result");
      Control cc = new Control(m);
      cm.nofocus = true;
      cm.drive(cc, cap, new Trackers(), new CheckPoints());
      Check.ok(cm.capsized, "caps_pzy180_capsized");
      Check.ok(cc.zyinv, "caps_pzy180_zyinv");
      Check.eq(cap.y, 200, "caps_pzy180_y_200");
      Check.eq(cap.zy, 180, "caps_pzy180_zy_180");
      // pzy=180 AND pxy=180: var7&&var11 clears var11 -> not capsized
      ContO cap2 = car(m);
      cap2.y = 250;
      Madness cm2 = mk(m, new Record(m), 1, 98.0F);
      cm2.pzy = MAD.intv("capsnap_result");
      cm2.pxy = MAD.intv("capsnap_result");
      cm2.nofocus = true;
      cm2.drive(new Control(m), cap2, new Trackers(), new CheckPoints());
      Check.ok(!cm2.capsized, "caps_both180_not_capsized");
      Check.eq(cap2.y, 250, "caps_both180_y_250");
      Check.eq(cap2.xy, 180, "caps_both180_xy_180");
      // pzy=100 (the >90 detection) and pzy=-100 (the >270 wrap)
      ContO cap3 = car(m);
      cap3.y = 250;
      Madness cm3 = mk(m, new Record(m), 1, 98.0F);
      cm3.pzy = MAD.intv("capsize_detect") + 10;
      cm3.nofocus = true;
      cm3.drive(new Control(m), cap3, new Trackers(), new CheckPoints());
      Check.ok(cm3.capsized, "caps_pzy100_capsized");
      Check.eq(cm3.pzy, MAD.intv("capsize_detect") + 10, "caps_pzy100_pzy_kept");
      Check.eq(cap3.zy, 52, "caps_pzy100_zy_52");
      ContO cap4 = car(m);
      cap4.y = 250;
      Madness cm4 = mk(m, new Record(m), 1, 98.0F);
      cm4.pzy = -(MAD.intv("capsize_detect") + 10);
      cm4.nofocus = true;
      cm4.drive(new Control(m), cap4, new Trackers(), new CheckPoints());
      Check.eq(cm4.pzy, MAD.intv("capsize_full") - (MAD.intv("capsize_detect") + 10),
            "caps_pzy_neg100_wraps_260");
      Check.eq(cap4.zy, 308, "caps_pzy_neg100_zy_308");

      // d2. the four-wheel attitude snap to 180/0 (:1899-1947)
      // all four wheels exceed 245 with scy equal -> var66==4, pzy/pzy
      // are folded into [0,360] and snapped to 180 when in (170,190).
      ContO sn = car(m);
      sn.y = 250;
      Madness snm = mk(m, new Record(m), 1, 98.0F);
      snm.pzy = MAD.intv("capsnap_probe");
      snm.pxy = MAD.intv("capsnap_deadband") / 2;
      snm.nofocus = true;
      snm.drive(new Control(m), sn, new Trackers(), new CheckPoints());
      Check.eq(snm.pzy, MAD.intv("capsnap_result"), "snap_pzy175_to_180");
      Check.eq(sn.zy, MAD.intv("capsnap_result"), "snap_zy_180");
      Check.eq(snm.pxy, 0, "snap_pxy_lt10_snaps_0");

      // ---- e. destruction ladder (:2076-2099,2255-2262,2425-2435) ----
      // hitmag 4000 > maxmag[0] 3500, cntdest 0 -> distruct + dest[im]
      ContO dc = car(m);
      dc.y = 240;
      dc.p[0] = plane(m, 3, 0, 0, false, false, 0);
      dc.p[0].chip = 4;
      dc.p[0].embos = 7;
      dc.p[1] = plane(m, 3, 5, 0, false, false, 0);
      dc.p[1].chip = 2;
      dc.p[1].embos = 9;
      dc.npl = 2;
      Record dr = new Record(m);
      Madness dm = mk(m, dr, 1, 98.0F);
      dm.hitmag = MAD.intv("destruction_hitmag");
      dm.nofocus = true;
      dm.drive(new Control(m), dc, new Trackers(), new CheckPoints());
      Check.eq(dm.cntdest, 1, "dest_cntdest_1");
      Check.ok(!dm.dest, "dest_not_yet");
      Check.eq(dr.dest[1], 300, "dest_rpd_dest_300");
      // distruct marks wz==0 planes, then the chip/embos sweep (:2089)
      Check.eq(dc.p[0].embos, 13, "dest_plane0_embos_13");
      Check.eq(dc.p[1].embos, 13, "dest_plane1_embos_13");
      Check.eq(dc.p[0].chip, 0, "dest_plane0_chip_0");
      Check.eq(dc.p[1].chip, 0, "dest_plane1_chip_0");

      // cntdest==7 -> dest=true, dested[1]=1 with lastcolido 0 (:2427)
      ContO dc2 = car(m);
      dc2.y = 240;
      CheckPoints dcp = new CheckPoints();
      Madness dm2 = mk(m, dr, 1, 98.0F);
      dm2.hitmag = MAD.intv("destruction_hitmag");
      dm2.cntdest = MAD.intv("destruction_cntdest");
      dm2.nofocus = true;
      dm2.drive(new Control(m), dc2, new Trackers(), dcp);
      Check.ok(dm2.dest, "dest_cntdest7_dest_true");
      Check.eq(dcp.dested[1], 1, "dest_dested_1");
      // dest && lastcolido!=0 -> dested 2 (:2429-2431)
      ContO dc3 = car(m);
      dc3.y = 240;
      CheckPoints dcp3 = new CheckPoints();
      Madness dm3 = mk(m, new Record(m), 1, 98.0F);
      dm3.dest = true;
      dm3.lastcolido = 40;
      dm3.nofocus = true;
      dm3.drive(new Control(m), dc3, new Trackers(), dcp3);
      Check.eq(dcp3.dested[1], 2, "dest_dested_2_with_lastcolido");
      Check.eq(dm3.lastcolido, 40, "dest_lastcolido_untouched");
      // undestroyed with a stale dested flag -> cleared (:2433-2434)
      dm3.dest = false;
      dm3.lastcolido = 0;
      dm3.nofocus = true;
      dm3.drive(new Control(m), dc3, new Trackers(), dcp3);
      Check.eq(dcp3.dested[1], 0, "dest_dested_cleared");

      // e2. fcnt==8 (and 7) reset the whole damage state (:2255-2262)
      ContO f8 = car(m);
      f8.y = 240;
      f8.fcnt = 8;
      Madness f8m = mk(m, new Record(m), 1, 98.0F);
      f8m.hitmag = MAD.intv("destruction_hitmag");
      f8m.dest = true;
      f8m.squash = 3;
      f8m.cntdest = 2;
      f8m.nofocus = true;
      f8m.drive(new Control(m), f8, new Trackers(), new CheckPoints());
      Check.ok(f8m.newcar, "fcnt8_newcar");
      Check.ok(!f8m.dest, "fcnt8_dest_false");
      Check.eq(f8m.hitmag, 0, "fcnt8_hitmag_0");
      Check.eq(f8m.cntdest, 0, "fcnt8_cntdest_0");
      Check.eq(f8m.squash, 0, "fcnt8_squash_0");

      // ---- f. fix points (:2225-2253) ----
      // dist==1 -> var2.fix = true and rpd.fix[im]=300
      ContO fx = car(m);
      fx.y = 240;
      fx.dist = 1;
      CheckPoints fxcp = new CheckPoints();
      fxcp.fn = 1;
      fxcp.fx[0] = 0;
      fxcp.fy[0] = 0;
      fxcp.fz[0] = 0;
      Record fxr = new Record(m);
      Madness fxm = mk(m, fxr, 1, 98.0F);
      fxm.nofocus = true;
      fxm.drive(new Control(m), fx, new Trackers(), fxcp);
      Check.ok(fx.fix, "fix_dist1_sets_fix");
      Check.eq(fxr.fix[1], 300, "fix_rpd_fix_300");
      // dist==0 -> fcnt=8, no fix flag (:2228-2229)
      ContO fx2 = car(m);
      fx2.y = 240;
      CheckPoints fxcp2 = new CheckPoints();
      fxcp2.fn = 1;
      Madness fx2m = mk(m, new Record(m), 1, 98.0F);
      fx2m.nofocus = true;
      fx2m.drive(new Control(m), fx2, new Trackers(), fxcp2);
      Check.eq(fx2.fcnt, 8, "fix_dist0_fcnt_8");
      // roted==true uses the z/x projection (:2240-2251)
      ContO fx3 = car(m);
      fx3.y = 240;
      fx3.dist = 1;
      CheckPoints fxcp3 = new CheckPoints();
      fxcp3.fn = 1;
      fxcp3.roted[0] = true;
      Record fx3r = new Record(m);
      Madness fx3m = mk(m, fx3r, 1, 98.0F);
      fx3m.nofocus = true;
      fx3m.drive(new Control(m), fx3, new Trackers(), fxcp3);
      Check.ok(fx3.fix, "fix_roted_sets_fix");
      Check.eq(fx3r.fix[1], 300, "fix_roted_rpd_300");

      // ---- g. checkpoints: typ 1 and typ 2 advance clear ----
      // clear counts typ>0 checkpoints; a box hit at clear == n-1
      // advances it and resets focus (:2110-2152).
      ContO k = car(m);
      k.y = 240;
      k.z = 0;
      CheckPoints kcp = new CheckPoints();
      kcp.n = 3;
      kcp.nsp = 3;
      kcp.typ[0] = 1;
      kcp.x[0] = 0;
      kcp.y[0] = 240;
      kcp.z[0] = 0;
      kcp.typ[1] = 2;
      kcp.x[1] = 0;
      kcp.y[1] = 240;
      kcp.z[1] = 100;
      kcp.typ[2] = 1;
      kcp.x[2] = 0;
      kcp.y[2] = 240;
      kcp.z[2] = 200;
      Madness km = mk(m, new Record(m), 1, 98.0F);
      km.clear = 0;
      km.nlaps = 0;
      km.nofocus = true;
      km.drive(new Control(m), k, new Trackers(), kcp);
      // typ1 cp0 and typ2 cp1 both clear in one tick (their boxes are
      // 100 apart and cp1's z-window is 700 wide)
      Check.eq(km.clear, 2, "cp_tick0_clear_2");
      Check.eq(km.pcleared, 1, "cp_tick0_pcleared_1");
      Check.eq(km.nlaps, 0, "cp_tick0_nlaps_0");
      km.nofocus = true;
      km.drive(new Control(m), k, new Trackers(), kcp);
      Check.eq(km.clear, 2, "cp_tick1_clear_kept_2");
      k.z = 200;
      km.nofocus = true;
      km.drive(new Control(m), k, new Trackers(), kcp);
      Check.eq(km.clear, 3, "cp_tick2_clear_3");
      Check.eq(km.pcleared, 2, "cp_tick2_pcleared_2");
      Check.eq(km.nlaps, 1, "cp_tick2_nlaps_1");
      Check.eq(km.point, 0, "cp_tick2_point_0");

      // g2. typ=-3 checkpoints zero the chosen point (:2180-2182)
      ContO t3 = car(m);
      t3.y = 240;
      CheckPoints t3cp = new CheckPoints();
      t3cp.n = 1;
      t3cp.nsp = 1;
      t3cp.typ[0] = -3;
      t3cp.x[0] = 0;
      t3cp.z[0] = 0;
      t3cp.y[0] = 240;
      Madness t3m = mk(m, new Record(m), 1, 98.0F);
      t3m.nofocus = true;
      t3m.drive(new Control(m), t3, new Trackers(), t3cp);
      Check.eq(t3m.point, 0, "cp_minus3_point_0");
      Check.eq(t3m.nlaps, 1, "cp_minus3_nlaps_1");

      // ---- h. focus / missedcp machine (:2154-2221) ----
      // car 30000 units away from a focus=1 checkpoint, im==0, mtouch:
      // missedcp 0 -> 1 at dist > 800 (:2194-2198)
      CheckPoints fcp = new CheckPoints();
      fcp.n = 3;
      fcp.nsp = 3;
      for (int i = 0; i < 3; i++) {
         fcp.typ[i] = 1;
         fcp.x[i] = i == 0 ? 50000 : 0;
         fcp.z[i] = i * 1000;
         fcp.y[i] = 240;
      }
      ContO fc = car(m);
      fc.y = 240;
      fc.x = 30000;
      Madness fm = mk(m, new Record(m), 0, 98.0F);
      fm.focus = 1;
      fm.missedcp = 0;
      fm.mtouch = true;
      fm.drive(new Control(m), fc, new Trackers(), fcp);
      Check.eq(fm.missedcp, 1, "focus_far_missedcp_1");
      Check.eq(fm.focus, 1, "focus_far_kept_1");
      // missedcp -2 and dist < 400 -> back to 0 (:2200-2202)
      ContO fc2 = car(m);
      fc2.y = 240;
      Madness fm2 = mk(m, new Record(m), 0, 98.0F);
      fm2.focus = 1;
      fm2.missedcp = -2;
      fm2.mtouch = true;
      fm2.drive(new Control(m), fc2, new Trackers(), fcp);
      Check.eq(fm2.missedcp, 0, "focus_near_clears_minus2");
      // missedcp!=0 and dist < 250 -> 68 (:2204-2208)
      ContO fc3 = car(m);
      fc3.y = 240;
      Madness fm3 = mk(m, new Record(m), 0, 98.0F);
      fm3.focus = 1;
      fm3.pcleared = 0;
      fm3.missedcp = 1;
      fm3.mtouch = true;
      fm3.drive(new Control(m), fc3, new Trackers(), fcp);
      Check.eq(fm3.missedcp, 68, "focus_very_near_missedcp_68");
      // nofocus with a live focus clears it (:2213-2216)
      ContO fc4 = car(m);
      fc4.y = 240;
      Madness fm4 = mk(m, new Record(m), 0, 98.0F);
      fm4.focus = 1;
      fm4.missedcp = 3;
      fm4.nofocus = true;
      fm4.drive(new Control(m), fc4, new Trackers(), fcp);
      Check.eq(fm4.focus, -1, "focus_nofocus_cleared");
      Check.eq(fm4.missedcp, 0, "focus_nofocus_missedcp_0");

      // ---- i. powerup machine (:2304-2349) ----
      // trcnt==9 + gtouch: travxy 100/24 = 4.1666665, clamped power 98,
      // xtpower 100, then the tracker counter advances to 10.
      ContO pu = car(m);
      pu.y = 240;
      Madness pm = mk(m, new Record(m), 1, 98.0F);
      pm.gtouch = true;
      pm.trcnt = MAD.intv("trcnt_powerup_arm");
      pm.travxy = 100;
      pm.nofocus = true;
      pm.drive(new Control(m), pu, new Trackers(), new CheckPoints());
      Check.eq(pm.powerup, (float) pm.travxy / MAD.floatv("powerup_travxy_div"), "powerup_travxy_4_1667");
      Check.eq(pm.power, MAD.floatv("powerup_clamp"), "powerup_power_clamped_98");
      Check.eq(pm.xtpower, MAD.intv("powerup_xt_low"), "powerup_xtpower_100");
      Check.eq(pm.trcnt, 10, "powerup_trcnt_10");
      // the flat-trick table: rtab+ftab+btab+surfer == 30+40+40+30 = 140
      ContO pu2 = car(m);
      pu2.y = 240;
      Madness pm2 = mk(m, new Record(m), 1, 0.0F);
      pm2.gtouch = true;
      pm2.trcnt = MAD.intv("trcnt_powerup_arm");
      pm2.rtab = true;
      pm2.ftab = true;
      pm2.btab = true;
      pm2.surfer = true;
      pm2.nofocus = true;
      pm2.drive(new Control(m), pu2, new Trackers(), new CheckPoints());
      Check.eq(pm2.powerup, (float) (MAD.intv("powerup_rtab") + MAD.intv("powerup_ftab")
            + MAD.intv("powerup_btab") + MAD.intv("powerup_surfer")), "powerup_table_140");
      Check.eq(pm2.power, MAD.floatv("powerup_clamp"), "powerup_table_power_98");
      Check.eq(pm2.xtpower, MAD.intv("powerup_xt_low"), "powerup_table_xtpower_100");
      // im==0 cotchinow branch: caught=300 makes cotchinow record the
      // powerup and set hcaught (:2334-2340)
      ContO pu3 = car(m);
      pu3.y = 240;
      Record pur = new Record(m);
      pur.caught = MAD.intv("cotchinow_caught");
      for (int i = 0; i < 5; i++) {
         pur.car[0][i] = car(m);
      }
      Madness pm3 = mk(m, pur, 0, 0.0F);
      pm3.gtouch = true;
      pm3.trcnt = MAD.intv("trcnt_powerup_arm");
      pm3.rtab = true;
      pm3.ftab = true;
      pm3.btab = true;
      pm3.surfer = true;
      pm3.nofocus = true;
      pm3.drive(new Control(m), pu3, new Trackers(), new CheckPoints());
      Check.eq(pur.powered, (int) (MAD.intv("powerup_rtab") + MAD.intv("powerup_ftab")
            + MAD.intv("powerup_btab") + MAD.intv("powerup_surfer")), "powerup_im0_powered_140");
      Check.ok(pur.hcaught, "powerup_im0_hcaught");
      Check.eq(pur.whenwasted, MAD.intv("cotchinow_whenwasted"), "powerup_im0_whenwasted_225");

      // ---- j. trick accumulation (:2270-2289) ----
      // loop==2 airborne, right held: travxy +20/tick; rtab flips at
      // |travxy| > 135; travzy grows from ucomp-dcomp; travxz from lxz.
      ContO trk = car(m);
      trk.y = 50;
      Madness tm = mk(m, new Record(m), 1, 98.0F);
      tm.loop = 2;
      tm.rcomp = 20.0F;
      tm.lcomp = 0.0F;
      tm.ucomp = 20.0F;
      tm.dcomp = 0.0F;
      Control trc = new Control(m);
      trc.right = true;
      for (int i = 0; i < 7; i++) {
         tm.nofocus = true;
         tm.drive(trc, trk, new Trackers(), new CheckPoints());
      }
      Check.eq(tm.travxy, 140, "trick_travxy_140");
      Check.eq(tm.travzy, 124, "trick_travzy_124");
      Check.ok(tm.rtab, "trick_rtab_true");

      // ---- k. dest locks the capcnt machine (:2367-2401) ----
      // The capcnt block is the `else` of `if (!capsized)` inside
      // `else if (!dest)`, so a DESTROYED car must NOT cycle it: capcnt
      // stays exactly where the fixture put it while the grip block
      // bleeds the speed (20 -> 19.5 -> ... -> 1.5). The live cycle is
      // pinned by the capsized fixtures below.
      ContO dd = car(m);
      dd.y = 240;
      Madness dmc = mk(m, new Record(m), 1, 98.0F);
      dmc.dest = true;
      dmc.mtouch = true;
      dmc.capcnt = 1;
      for (int i = 0; i < MAD.intv("capsize_respawn_ticks") - 1; i++) {
         tick(m, dd, new Control(m), new CheckPoints(), 1, dmc);
      }
      Check.eq(dmc.capcnt, 1, "dest_capcnt_not_cycled_1");
      Check.eq(dmc.speed, 1.5F, "dest_speed_bleeds_to_1_5");
      // the trcnt!=0 reset arm
      ContO dt = car(m);
      dt.y = 240;
      Madness dmt = mk(m, new Record(m), 1, 98.0F);
      dmt.dest = true;
      dmt.mtouch = true;
      dmt.trcnt = 5;
      dmt.travxy = 10;
      dmt.surfer = true;
      dmt.nofocus = true;
      dmt.drive(new Control(m), dt, new Trackers(), new CheckPoints());
      Check.eq(dmt.trcnt, 5, "destcap_trcnt5_untouched");
      Check.eq(dmt.travxy, 10, "destcap_travxy_untouched");

      // ---- l. attitude snap onto the ContO (:2004-2023) ----
      // pxy 10 / xy 0 (deadband) and xy 100 -> 48 (the 2+half step)
      ContO at = car(m);
      at.y = 0;
      Madness am2 = mk(m, new Record(m), 1, 98.0F);
      am2.pxy = 10;
      am2.pzy = -10;
      am2.nofocus = true;
      am2.drive(new Control(m), at, new Trackers(), new CheckPoints());
      Check.eq(at.xy, 7, "att_pxy10_xy7");
      Check.eq(at.zy, -7, "att_pzy_minus10_zy_minus7");
      ContO at2 = car(m);
      at2.y = 0;
      at2.xy = 100;
      at2.zy = -100;
      Madness am3 = mk(m, new Record(m), 1, 98.0F);
      am3.nofocus = true;
      am3.drive(new Control(m), at2, new Trackers(), new CheckPoints());
      Check.eq(at2.xy, 48, "att_xy100_xy48");
      Check.eq(at2.zy, -48, "att_zy_minus100_zy_minus48");

      // ---- m. impulse clamps (:1198-1218) ----
      // scx[0]=500 with the other three at 0: avg 125, 500-125 > 200, so
      // scx[0] becomes 325; scz[0]=-500 -> avg -125 -> -325.
      ContO ic = car(m);
      ic.y = 240;
      Madness im = mk(m, new Record(m), 1, 98.0F);
      im.scx[0] = 500.0F;
      im.scz[0] = -500.0F;
      im.nofocus = true;
      im.drive(new Control(m), ic, new Trackers(), new CheckPoints());
      Check.eq(im.scx[0], 325.0F, "imp_scx_clamp_325");
      Check.eq(im.scx[1], 0.0F, "imp_scx1_unchanged_0");
      Check.eq(im.scz[0], -325.0F, "imp_scz_clamp_minus325");

      // ---- n. mxz auto-turn (:1159-1178) ----
      // |speed|>30: within 30 degrees cxz steps a quarter of the gap,
      // otherwise +-10; the >180 wrap runs :1160-1166.
      Check.eq(cxzTick(m, 100.0F, 0, 170), 10, "mxz_drift_step_10");
      Check.eq(cxzTick(m, 50.0F, 0, 100), 10, "mxz_small_gap_quarter_10");
      Check.eq(cxzTick(m, 50.0F, 0, 200), MAD.intv("mxz_wrap_result"), "mxz_wrap_360_to_350");

      // ---- o. ground clamp with the cn==7 bounce (0.8) ----
      // scy 150 -> var70 0.3+1.1, regy(165) then scy = 150-165 = -15.7
      // after the 1.1 floor (:1452-1460) and y clamps to 250.
      ContO g7 = car(m);
      g7.y = 240;
      Madness gm = mk(m, new Record(m), 1, 98.0F);
      gm.cn = 7;
      for (int i = 0; i < 4; i++) {
         gm.scy[i] = 150.0F;
      }
      gm.nofocus = true;
      gm.drive(new Control(m), g7, new Trackers(), new CheckPoints());
      Check.eq(g7.y, 250, "ground7_y_250");
      Check.eq(gm.scy[0], -15.699997F, "ground7_scy_minus15_7");
      Check.ok(gm.mtouch, "ground7_mtouch");

      // ---- p. skid scatter with a skd==3 tracker (:1328-1374) ----
      // dcnt 100 > threshold and random 0.2/0.7 draw: the bounce at
      // :1369-1373 rewrites scy[rand*4]; skid stays 1.
      Medium skm = new VecMedium();
      ContO sc = car(skm);
      sc.y = 240;
      Trackers sct = new Trackers();
      sct.nt = 1;
      floor(sct, 0, 0, 240, 0, 3, 20, 100, 100, 40);
      Madness scm = mk(skm, new Record(skm), 1, 98.0F);
      scm.dcnt = 100;
      scm.mtouch = true;
      scm.txz = 1;
      scm.speed = 50.0F;
      scm.nofocus = true;
      scm.drive(new Control(m), sc, sct, new CheckPoints());
      Check.eq(scm.dcnt, 104, "skid3_dcnt_104");
      Check.eq(scm.skid, 1, "skid3_skid_1");
      Check.eq(scm.scy[0], -7.8750005F, "skid3_scy0_minus7_875");

      // ---- q. regy/regz/regx damage scatter with wz==0 planes ----
      // The empty-ContO fixture has npl==0, so these fixtures add planes
      // explicitly: bfase 25 crosses the colour bands, glass decrements
      // gr, and the im==1 path never dereferences xt.
      Medium rg = new VecMedium();
      ContO ry = car(rg);
      ry.p[0] = plane(rg, 3, 0, 25, false, false, 0);
      ry.p[1] = plane(rg, 3, 0, 65, true, false, 0);
      ry.npl = 2;
      Record ryr = new Record(rg);
      Madness rym = new Madness(rg, ryr, null, 1);
      rym.cn = 0;
      rym.mtouch = true;
      rym.regy(0, 150.0F, ry);
      Check.eq(ryr.nry[1][0], 1, "regy_scatter_recorded");
      Check.eq(ry.p[0].chip, 1, "regy_scatter_chip");
      Check.eq(ry.p[0].bfase, 26, "regy_scatter_bfase_26");
      Check.eq(ry.p[1].bfase, 65, "regy_scatter_bfase_65");
      Check.eq(ry.p[1].gr, -1, "regy_scatter_glass_gr_minus1");
      Check.eq(ry.p[0].hsb[1], 0.2F, "regy_scatter_hsb1_0_2");
      // the squash ladder: zy=180 (var4=-1) x xy=0 (var5=1) -> product -1
      ContO sq = car(rg);
      sq.zy = 180;
      sq.p[0] = plane(rg, 3, 0, 0, false, false, -50);
      sq.npl = 1;
      Madness sqm = new Madness(rg, new Record(rg), null, 1);
      sqm.cn = 0;
      sqm.mtouch = true;
      sqm.regy(0, 150.0F, sq);
      Check.eq(sqm.nbsq, 1, "regy_squash_first_strike_nbsq_1");
      sqm.regy(0, 150.0F, sq);
      Check.eq(sqm.nbsq, 0, "regy_squash_consumed_nbsq_0");
      Check.eq(sq.p[0].oy[0], -49, "regy_squash_oy_minus49");

      ContO rz = car(rg);
      rz.p[0] = plane(rg, 3, 0, 25, false, false, 0);
      rz.p[1] = plane(rg, 3, 0, 65, true, false, 0);
      rz.npl = 2;
      Record rzr = new Record(rg);
      Madness rzm = new Madness(rg, rzr, null, 1);
      rzm.cn = 0;
      rzm.regz(0, 150.0F, rz);
      Check.eq(rzr.nrz[1][0], 1, "regz_scatter_recorded");
      Check.eq(rz.p[0].chip, 1, "regz_scatter_chip");
      Check.eq(rz.p[0].bfase, 26, "regz_scatter_bfase_26");
      Check.eq(rz.p[1].gr, -2, "regz_scatter_glass_gr_minus2");
      Check.eq(rz.p[0].oz[0], 2, "regz_scatter_oz_2");

      ContO rx = car(rg);
      rx.p[0] = plane(rg, 3, 0, 25, false, false, 0);
      rx.p[1] = plane(rg, 3, 0, 65, true, false, 0);
      rx.npl = 2;
      Record rxr = new Record(rg);
      Madness rxm = new Madness(rg, rxr, null, 1);
      rxm.cn = 0;
      rxm.regx(0, 150.0F, rx);
      Check.eq(rxr.nrx[1][0], 1, "regx_scatter_recorded");
      Check.eq(rx.p[0].chip, 1, "regx_scatter_chip");
      Check.eq(rx.p[1].gr, -1, "regx_scatter_glass_gr_minus1");
      // the nocol/glass sweep at bfase 55/61 (:188-217)
      ContO rx2 = car(rg);
      rx2.p[0] = plane(rg, 3, 0, 55, false, true, 0);
      rx2.p[1] = plane(rg, 3, 0, 61, false, false, 0);
      rx2.npl = 2;
      Madness rx2m = new Madness(rg, new Record(rg), null, 1);
      rx2m.cn = 0;
      rx2m.regx(0, 150.0F, rx2);
      Check.eq(rx2.p[1].hsb[0], 0.05F, "regx_bfase61_hsb0_0_05");

      // ---- r. colide push clamps and the caught gate ----
      // revpush/push are clamped to +-300; the dominant car's impulse is
      // reduced by the reversed one (:456-538).
      ContO ca = car(m);
      ContO cb = car(m);
      ca.maxR = 10;
      cb.maxR = 10;
      Madness oth = mk(m, new Record(m), 1, 75.0F);
      oth.speed = 5.0F;
      Madness me = mk(m, new Record(m), 1, 75.0F);
      me.speed = 10.0F;
      for (int i = 0; i < 4; i++) {
         me.scx[i] = 400.0F;
         oth.scx[i] = 300.0F;
      }
      me.colide(ca, oth, cb);
      Check.ok(me.dominate[1], "colide_clamp_dominates");
      Check.eq(me.scx[0], 100.0F, "colide_clamp_this_scx_100");
      Check.eq(oth.scx[0], 600.0F, "colide_clamp_other_scx_600");
      // caught already true suppresses the domination rewrite (:419)
      ContO cc2 = car(m);
      ContO cd = car(m);
      cc2.maxR = 10;
      cd.maxR = 10;
      Madness oth2 = mk(m, new Record(m), 1, 75.0F);
      oth2.speed = 5.0F;
      Madness me2 = mk(m, new Record(m), 1, 75.0F);
      me2.speed = 10.0F;
      me2.caught[1] = true;
      me2.colide(cc2, oth2, cd);
      Check.ok(!me2.dominate[1], "colide_caught_gate_keeps_dominate");
      Check.ok(me2.caught[1], "colide_caught_gate_keeps_caught");
      // this.capsized uses flipy+squash for the wheel height (:394-406)
      ContO ce = car(m);
      ContO cf = car(m);
      ce.maxR = 10;
      cf.maxR = 10;
      Madness oth3 = mk(m, new Record(m), 1, 75.0F);
      oth3.speed = 5.0F;
      Madness me3 = mk(m, new Record(m), 1, 75.0F);
      me3.capsized = true;
      me3.squash = 3;
      me3.speed = 10.0F;
      me3.colide(ce, oth3, cf);
      Check.ok(me3.dominate[1], "colide_capsized_dominates");

      // ---- s. im==0 wall reset (:2417-2420) ----
      // The wall must stay below the 100 damage gate (scx 1, dam 20 ->
      // 26) so im==0 never reaches the xt crash at :663.
      ContO iw = car(m);
      iw.y = 240;
      Trackers iwt = new Trackers();
      iwt.nt = 1;
      wallx(iwt, 0, 0, 240, 0, 90, 100, 100, 60, 20);
      Madness iwm = mk(m, new Record(m), 0, 98.0F);
      for (int i = 0; i < 4; i++) {
         iwm.scx[i] = 1.0F;
      }
      Control iwc = new Control(m);
      iwm.nofocus = true;
      iwm.drive(iwc, iw, iwt, new CheckPoints());
      Check.eq(iwc.wall, -1, "im0_wall_reset_minus1");
      Check.eq(iwm.hitmag, 0, "im0_wall_below_damage_gate");

      // ---- u. residual arms: air releases, deeper colour bands, the
      // capsized respawn flip and the remaining colide tie arms ----

      // u1. loop==2 release arms (:863-918): ucomp/dcomp/lcomp/rcomp all
      // decay by 0.5*airs per tick when their key is up.
      ContO ua = car(m);
      ua.y = 50;
      Madness um = mk(m, new Record(m), 1, 98.0F);
      um.loop = 2;
      um.ucomp = 6.0F;
      um.dcomp = 6.0F;
      um.lcomp = 6.0F;
      um.rcomp = 6.0F;
      um.nofocus = true;
      um.drive(new Control(m), ua, new Trackers(), new CheckPoints());
      Check.eq(um.ucomp, 5.5F, "air_release_ucomp_5_5");
      Check.eq(um.dcomp, 5.5F, "air_release_dcomp_5_5");
      Check.eq(um.lcomp, 4.0F, "air_release_lcomp_4");
      Check.eq(um.rcomp, 4.0F, "air_release_rcomp_4");

      // u2. loop==-1 release arms (:1003-1063): the latched flags block a
      // second press, and releasing decays the comps.
      ContO ub = car(m);
      ub.y = 50;
      Madness um2 = mk(m, new Record(m), 1, 98.0F);
      um2.loop = -1;
      um2.lcomp = 6.0F;
      um2.rcomp = 6.0F;
      um2.ucomp = 6.0F;
      um2.dcomp = 6.0F;
      um2.pl = true;
      um2.pr = true;
      um2.pu = true;
      um2.pd = true;
      um2.nofocus = true;
      um2.drive(new Control(m), ub, new Trackers(), new CheckPoints());
      Check.eq(um2.lcomp, 4.0F, "loopminus_release_lcomp_4");
      Check.eq(um2.rcomp, 4.0F, "loopminus_release_rcomp_4");
      Check.eq(um2.ucomp, 4.0F, "loopminus_release_ucomp_4");
      Check.eq(um2.dcomp, 4.0F, "loopminus_release_dcomp_4");
      Check.ok(!um2.pl, "loopminus_pl_cleared");

      // u3. the var7==true (upside-down) air rotation arms: pzy>90 flips
      // the sign of the xz term (:921-925 and :1066-1070).
      ContO uc = car(m);
      uc.y = 50;
      Madness um3 = mk(m, new Record(m), 1, 98.0F);
      um3.loop = 2;
      um3.pzy = 180;
      um3.pxy = 0;
      um3.ucomp = 10.0F;
      um3.dcomp = 0.0F;
      um3.nofocus = true;
      um3.drive(new Control(m), uc, new Trackers(), new CheckPoints());
      Check.ok(um3.pzy != 0, "air_upside_down_pzy_moves");
      ContO ud = car(m);
      ud.y = 50;
      Madness um4 = mk(m, new Record(m), 1, 98.0F);
      um4.loop = -1;
      um4.pzy = 180;
      um4.ucomp = 8.0F;
      um4.nofocus = true;
      um4.drive(new Control(m), ud, new Trackers(), new CheckPoints());
      Check.ok(um4.pzy != 0, "loopminus_upside_down_pzy_moves");

      // u4. the capsized respawn flip (:2367-2401): dest==false and
      // capsized==true is the ONLY way into the capcnt block. Start at
      // capcnt 1 and walk the 30-tick counter.
      ContO cflc = car(m);
      cflc.y = 240;
      Madness cfl = mk(m, new Record(m), 1, 98.0F);
      cfl.pzy = 180;
      cfl.pxy = 0;
      cfl.capcnt = 1;
      for (int i = 0; i < MAD.intv("capsize_respawn_ticks") - 1; i++) {
         cfl.nofocus = true;
         cfl.drive(new Control(m), cflc, new Trackers(), new CheckPoints());
      }
      Check.eq(cfl.capcnt, 0, "capsflip_capcnt_cycle_0");
      Check.eq(cfl.speed, 0.0F, "capsflip_speed_0");
      // a fresh capsized car with capcnt 0 and scx/scz inside 70 arms
      // the counter (:2379-2390)
      ContO cf2 = car(m);
      cf2.y = 240;
      Madness cfl2 = mk(m, new Record(m), 1, 98.0F);
      cfl2.pzy = 180;
      cfl2.pxy = 0;
      cfl2.nofocus = true;
      cfl2.drive(new Control(m), cf2, new Trackers(), new CheckPoints());
      Check.eq(cfl2.capcnt, 1, "capsflip_capcnt_arm_1");
      // the !dest && !capsized arm clears a stale capcnt (:2300-2301)
      ContO cf3 = car(m);
      cf3.y = 240;
      Madness cfl3 = mk(m, new Record(m), 1, 98.0F);
      cfl3.capcnt = 5;
      cfl3.nofocus = true;
      cfl3.drive(new Control(m), cf3, new Trackers(), new CheckPoints());
      Check.eq(cfl3.capcnt, 0, "caps_stale_capcnt_cleared");

      // u5. skid==2 with capsized+var8 recomputes speed negative (:1389-1399)
      ContO sg = car(m);
      sg.y = 240;
      Madness sgm = mk(m, new Record(m), 1, 98.0F);
      sgm.mtouch = true;
      sgm.speed = 20.0F;
      sgm.skid = 2;
      sgm.pzy = 180;
      sgm.pxy = 100;
      sgm.txz = 0;
      sgm.nofocus = true;
      sgm.drive(new Control(m), sg, new Trackers(), new CheckPoints());
      Check.eq(sgm.speed, 15.0F, "caps_skid2_speed_15");
      // capsized with zero total impulse zeroes the tracker skid (:1403)
      ContO sz = car(m);
      sz.y = 240;
      Madness szm = mk(m, new Record(m), 1, 98.0F);
      szm.mtouch = true;
      szm.capsized = true;
      szm.skid = 2;
      szm.nofocus = true;
      szm.drive(new Control(m), sz, new Trackers(), new CheckPoints());
      Check.ok(szm.speed <= 0.0F, "caps_zero_impulse_speed");

      // u6. sloped tracker with a NON-zero skid: the dust arm at :1686-1689
      // and the mtouch==var56 variant.
      Medium slm = new VecMedium();
      ContO ss = car(slm);
      ss.y = 240;
      Trackers sst = new Trackers();
      sst.nt = 1;
      wallz(sst, 0, 0, 240, 0, 45, 100, 100, 60, 20);
      sst.skd[0] = 1;
      Madness ssm = mk(slm, new Record(slm), 1, 98.0F);
      ssm.nofocus = true;
      ssm.drive(new Control(m), ss, sst, new CheckPoints());
      Check.ok(ssm.scy[0] < 0.0F, "slope_skid1_scy_negative");
      // same tile but mtouch already true -> var56 true skips the dust
      ContO ss2 = car(slm);
      ss2.y = 240;
      Madness ssm2 = mk(slm, new Record(slm), 1, 98.0F);
      ssm2.mtouch = true;
      ssm2.skid = 2;
      ssm2.speed = 10.0F;
      ssm2.nofocus = true;
      ssm2.drive(new Control(m), ss2, sst, new CheckPoints());
      Check.ok(ssm2.scy[0] < 0.0F, "slope_mtouch_scy_negative");

      // u7. wzy positive wrap (:1087-1088) and handb on negative speed
      // (:984-989)
      ContO ww = car(m);
      ww.y = 240;
      ww.wzy = 40;
      Madness wwm = mk(m, new Record(m), 1, 98.0F);
      wwm.speed = -280.0F;
      wwm.nofocus = true;
      wwm.drive(new Control(m), ww, new Trackers(), new CheckPoints());
      Check.eq(ww.wzy, 35, "wzy_plus40_wraps_35");
      ContO hb = car(m);
      hb.y = 240;
      Madness hbm = mk(m, new Record(m), 1, 98.0F);
      hbm.speed = -100.0F;
      hbm.mtouch = true;
      Control hbc = new Control(m);
      hbc.handb = true;
      hbm.nofocus = true;
      hbm.drive(hbc, hb, new Trackers(), new CheckPoints());
      Check.ok(hbm.speed > -100.0F, "handb_negative_speed_rises");

      // u8. xtpower countdown (:2411-2413) and lastcolido decay (:2421-2423)
      ContO xp = car(m);
      xp.y = 240;
      Madness xpm = mk(m, new Record(m), 1, 98.0F);
      xpm.xtpower = 5;
      xpm.nofocus = true;
      xpm.drive(new Control(m), xp, new Trackers(), new CheckPoints());
      Check.eq(xpm.xtpower, 4, "xtpower_decays_4");
      ContO lc2 = car(m);
      lc2.y = 240;
      Madness lcm = mk(m, new Record(m), 1, 98.0F);
      lcm.lastcolido = 5 * MAD.intv("lastcolido_decay");
      lcm.nofocus = true;
      lcm.drive(new Control(m), lc2, new Trackers(), new CheckPoints());
      Check.eq(lcm.lastcolido, 4 * MAD.intv("lastcolido_decay"), "lastcolido_decays_4");

      // u9. the negative impulse clamps (:1207-1216)
      ContO nc = car(m);
      nc.y = 240;
      Madness ncm = mk(m, new Record(m), 1, 98.0F);
      ncm.scx[0] = -500.0F;
      ncm.scz[0] = 500.0F;
      ncm.nofocus = true;
      ncm.drive(new Control(m), nc, new Trackers(), new CheckPoints());
      Check.eq(ncm.scx[0], -325.0F, "imp_scx_neg_clamp_minus325");
      Check.eq(ncm.scz[0], 325.0F, "imp_scz_pos_clamp_325");

      // u10. ground dust with var27==0 and a non-7 scy (:1422-1434)
      ContO gd = car(m);
      gd.y = 240;
      Madness gdm = mk(m, new Record(m), 1, 98.0F);
      for (int i = 0; i < 4; i++) {
         gdm.scy[i] = 100.0F;
      }
      gdm.nofocus = true;
      gdm.drive(new Control(m), gd, new Trackers(), new CheckPoints());
      Check.ok(gd.y == 250, "ground_dust_y_250");

      // u11. powerup: travzy/travxz terms and the >150 xtpower arm
      ContO pz = car(m);
      pz.y = 240;
      Madness pzm = mk(m, new Record(m), 1, 0.0F);
      pzm.gtouch = true;
      pzm.trcnt = 9;
      pzm.travxy = 2000;
      pzm.travzy = 2000;
      pzm.travxz = 2000;
      pzm.rtab = true;
      pzm.ftab = true;
      pzm.btab = true;
      pzm.surfer = true;
      pzm.nofocus = true;
      pzm.drive(new Control(m), pz, new Trackers(), new CheckPoints());
      Check.eq(pzm.xtpower, MAD.intv("powerup_xt_high"), "powerup_over150_xtpower_200");
      Check.ok(pzm.powerup > MAD.floatv("powerup_xt_threshold"), "powerup_over150_value");

      // u12. trick table boundaries: ftab above +135 and btab below -135
      ContO tb = car(m);
      tb.y = 50;
      Madness tbm = mk(m, new Record(m), 1, 98.0F);
      tbm.loop = 2;
      tbm.travzy = MAD.intv("trick_rotate") - MAD.intv("trick_probe_delta");
      tbm.ucomp = 20.0F;
      tbm.dcomp = 0.0F;
      tbm.nofocus = true;
      tbm.drive(new Control(m), tb, new Trackers(), new CheckPoints());
      Check.ok(tbm.ftab, "trick_ftab_over_135");
      ContO tb2 = car(m);
      tb2.y = 50;
      Madness tbm2 = mk(m, new Record(m), 1, 98.0F);
      tbm2.loop = 2;
      tbm2.travzy = -(MAD.intv("trick_rotate") - MAD.intv("trick_probe_delta"));
      tbm2.ucomp = 0.0F;
      tbm2.dcomp = 20.0F;
      tbm2.nofocus = true;
      tbm2.drive(new Control(m), tb2, new Trackers(), new CheckPoints());
      Check.ok(tbm2.btab, "trick_btab_below_minus135");

      // u13. checkpoint var6 arms (:2114-2115,:2129-2130): clear equal to
      // the running typ>0 count arms the focus repick.
      ContO cv = car(m);
      cv.y = 240;
      cv.z = 5000;
      CheckPoints cvc = new CheckPoints();
      cvc.n = 2;
      cvc.nsp = 2;
      cvc.typ[0] = 1;
      cvc.x[0] = 0;
      cvc.y[0] = 240;
      cvc.z[0] = 0;
      cvc.typ[1] = 2;
      cvc.x[1] = 0;
      cvc.y[1] = 240;
      cvc.z[1] = 5000;
      Madness cvm = mk(m, new Record(m), 1, 98.0F);
      cvm.clear = 1;
      cvm.nofocus = true;
      cvm.drive(new Control(m), cv, new Trackers(), cvc);
      Check.eq(cvm.clear, 2, "cp_var6_type2_clear_advances_2");
      // and the type-1 arm with clear==1 at the first checkpoint
      ContO cv2 = car(m);
      cv2.y = 240;
      CheckPoints cvc2 = new CheckPoints();
      cvc2.n = 2;
      cvc2.nsp = 2;
      cvc2.typ[0] = 1;
      cvc2.x[0] = 0;
      cvc2.y[0] = 240;
      cvc2.z[0] = 0;
      cvc2.typ[1] = 1;
      cvc2.x[1] = 50000;
      cvc2.y[1] = 240;
      cvc2.z[1] = 0;
      Madness cv2m = mk(m, new Record(m), 1, 98.0F);
      cv2m.clear = 1;
      cv2m.nofocus = true;
      cv2m.drive(new Control(m), cv2, new Trackers(), cvc2);
      Check.eq(cv2m.clear, 1, "cp_var6_type1_clear_kept_1");

      // u14. focus repick with var114 < pcleared (:2170-2172)
      ContO fr = car(m);
      fr.y = 240;
      fr.x = 5000;
      CheckPoints frc = new CheckPoints();
      frc.n = 4;
      frc.nsp = 4;
      for (int i = 0; i < 4; i++) {
         frc.typ[i] = 1;
         frc.x[i] = i * 20000;
         frc.z[i] = 0;
         frc.y[i] = 240;
      }
      fr.x = 25000;
      Madness frm = mk(m, new Record(m), 0, 98.0F);
      frm.focus = -1;
      frm.pcleared = 1;
      frm.clear = 1;
      frm.nlaps = 0;
      frm.drive(new Control(m), fr, new Trackers(), frc);
      Check.eq(frm.point, 2, "focus_repick_point_2");
      Check.eq(frm.focus, 2, "focus_repick_focus_2");

      // u15. focus!= -1 with im!=0 always sets missedcp=1 (:2209-2210)
      ContO fi = car(m);
      fi.y = 240;
      fi.x = 30000;
      Madness fim = mk(m, new Record(m), 1, 98.0F);
      fim.focus = 1;
      fim.missedcp = 0;
      fim.mtouch = true;
      fim.drive(new Control(m), fi, new Trackers(), frc);
      Check.eq(fim.missedcp, 1, "focus_im1_missedcp_1");

      // u16. fix point misses: far z (:2227) and far x on the roted arm
      // (:2240)
      ContO fz = car(m);
      fz.y = 240;
      CheckPoints fzc = new CheckPoints();
      fzc.fn = 1;
      fzc.fz[0] = 5000;
      Madness fzm = mk(m, new Record(m), 1, 98.0F);
      fzm.nofocus = true;
      fzm.drive(new Control(m), fz, new Trackers(), fzc);
      Check.eq(fz.fcnt, 0, "fix_far_z_no_fix");
      ContO fz2 = car(m);
      fz2.y = 240;
      CheckPoints fzc2 = new CheckPoints();
      fzc2.fn = 1;
      fzc2.roted[0] = true;
      fzc2.fx[0] = 5000;
      Madness fz2m = mk(m, new Record(m), 1, 98.0F);
      fz2m.nofocus = true;
      fz2m.drive(new Control(m), fz2, new Trackers(), fzc2);
      Check.eq(fz2.fcnt, 0, "fix_roted_far_x_no_fix");

      // u17. fcnt==7 also triggers the reset block
      ContO f7 = car(m);
      f7.y = 240;
      f7.fcnt = 7;
      Madness f7m = mk(m, new Record(m), 1, 98.0F);
      f7m.dest = true;
      f7m.nofocus = true;
      f7m.drive(new Control(m), f7, new Trackers(), new CheckPoints());
      Check.ok(f7m.newcar, "fcnt7_newcar");
      Check.ok(!f7m.dest, "fcnt7_dest_false");

      // u18. colide: the weaker car loses (:424), the moment tie with a
      // smaller moment loses (:429), and the z-push block clamps
      ContO wa = car(m);
      ContO wb = car(m);
      wa.maxR = 10;
      wb.maxR = 10;
      Madness othw = mk(m, new Record(m), 1, 60.0F);
      othw.speed = 20.0F;
      Madness mew = mk(m, new Record(m), 1, 60.0F);
      mew.speed = 5.0F;
      mew.colide(wa, othw, wb);
      Check.ok(!mew.dominate[1], "colide_weaker_dominate_false");
      Check.ok(mew.caught[1], "colide_weaker_caught_true");
      // equal power*speed*moment, smaller moment -> false (:429)
      ContO wc2 = car(m);
      ContO wd2 = car(m);
      wc2.maxR = 10;
      wd2.maxR = 10;
      Madness oth2b = mk(m, new Record(m), 1, 50.0F);
      oth2b.cn = 0; // moment 1.2
      oth2b.speed = 2.0F;
      Madness me2b = mk(m, new Record(m), 1, 60.0F);
      me2b.cn = 3; // moment 1.0
      me2b.speed = 2.0F;
      me2b.colide(wc2, oth2b, wd2);
      Check.ok(!me2b.dominate[1], "colide_moment_tie_loses");
      // z push with clamps (:498-536)
      ContO ze = car(m);
      ContO zf = car(m);
      ze.maxR = 10;
      zf.maxR = 10;
      Madness oz = mk(m, new Record(m), 1, 70.0F);
      oz.speed = 4.0F;
      Madness mz = mk(m, new Record(m), 1, 70.0F);
      mz.speed = 8.0F;
      for (int i = 0; i < 4; i++) {
         mz.scz[i] = 500.0F;
         oz.scz[i] = -400.0F;
      }
      mz.colide(ze, oz, zf);
      Check.ok(mz.dominate[1], "colide_zpush_dominates");
      Check.ok(mz.scz[0] != 500.0F, "colide_zpush_this_moved");
      // other.im==0 sets this.lastcolido (:544-545) - keep the push small
      ContO zi = car(m);
      ContO zj = car(m);
      zi.maxR = 10;
      zj.maxR = 10;
      Madness oi = mk(m, new Record(m), 0, 75.0F);
      oi.speed = 5.0F;
      Madness mi = mk(m, new Record(m), 1, 75.0F);
      mi.speed = 10.0F;
      mi.scx[0] = 10.0F;
      mi.colide(zi, oi, zj);
      Check.eq(mi.lastcolido, 70, "colide_other_im0_sets_this_lastcolido");

      // u19. the deeper colour bands on all three reg* scatter blocks
      // (:193-216 regy, :330-353 regz, :691-705 regx)
      Medium bandm = new VecMedium();
      for (int band = 0; band < 3; band++) {
         int bf = 35 + band * 15; // 35, 50, 65
         ContO bc = car(bandm);
         bc.p[0] = plane(bandm, 3, 0, bf, false, false, 0);
         bc.p[1] = plane(bandm, 3, 0, bf, true, false, 0);
         bc.npl = 2;
         Madness bm = new Madness(bandm, new Record(bandm), null, 1);
         bm.cn = 0;
         bm.mtouch = true;
         bm.regy(0, 150.0F, bc);
         Check.ok(bc.p[0].bfase > bf, "regy_band" + bf + "_damaged");
      }
      ContO bz = car(bandm);
      bz.p[0] = plane(bandm, 3, 0, 65, false, false, 0);
      bz.p[1] = plane(bandm, 3, 0, 65, true, false, 0);
      bz.npl = 2;
      Madness bzm = new Madness(bandm, new Record(bandm), null, 1);
      bzm.cn = 0;
      bzm.regz(0, 150.0F, bz);
      Check.ok(bz.p[0].bfase > 65, "regz_band65_damaged");
      ContO bx = car(bandm);
      bx.p[0] = plane(bandm, 3, 0, 65, false, false, 0);
      bx.p[1] = plane(bandm, 3, 0, 65, true, false, 0);
      bx.npl = 2;
      Madness bxm = new Madness(bandm, new Record(bandm), null, 1);
      bxm.cn = 0;
      bxm.regx(0, 150.0F, bx);
      Check.ok(bx.p[0].bfase > 65, "regx_band65_damaged");

      // u20. loop==-1 press arms with the flags clear (:993-1063)
      ContO l3 = car(m);
      l3.y = 50;
      Madness l3m = mk(m, new Record(m), 1, 98.0F);
      l3m.loop = -1;
      l3m.lcomp = 0.0F;
      l3m.ucomp = 0.0F;
      l3m.rcomp = 0.0F;
      l3m.dcomp = 0.0F;
      Control l3c = new Control(m);
      l3c.left = true;
      l3c.up = true;
      l3c.right = true;
      l3c.down = true;
      l3m.nofocus = true;
      l3m.drive(l3c, l3, new Trackers(), new CheckPoints());
      Check.eq(l3m.lcomp, 7.0F, "loopminus_press_lcomp_7");
      Check.eq(l3m.ucomp, 7.0F, "loopminus_press_ucomp_7");
      Check.eq(l3m.rcomp, 7.0F, "loopminus_press_rcomp_7");
      Check.eq(l3m.dcomp, 7.0F, "loopminus_press_dcomp_7");

      // u21. handb with `pushed` true keeps loop 0, so the :928 speed
      // block (not the air block) applies the :984 bite to a negative
      // speed.
      ContO hp = car(m);
      hp.y = 240;
      Madness hpm = mk(m, new Record(m), 1, 98.0F);
      hpm.speed = -100.0F;
      hpm.pushed = true;
      hpm.wtouch = true;
      hpm.gtouch = true;
      Control hpc = new Control(m);
      hpc.handb = true;
      hpm.nofocus = true;
      hpm.drive(hpc, hp, new Trackers(), new CheckPoints());
      Check.eq(hpm.speed, -93.0F, "handb_pushed_negative_speed_minus93");

      // u22. colide moment tie-breaks (equal impulse products):
      // this moment 2.0 (cn 9) beats other 1.0 (cn 3) (:426-427)
      ContO ma = car(m);
      ContO mb = car(m);
      ma.maxR = 10;
      mb.maxR = 10;
      Madness om = mk(m, new Record(m), 1, 100.0F);
      om.cn = 3;
      om.speed = 1.0F;
      Madness mmx = mk(m, new Record(m), 1, 50.0F);
      mmx.cn = 9;
      mmx.speed = 1.0F;
      mmx.colide(ma, om, mb);
      Check.ok(mmx.dominate[1], "colide_moment_2_beats_1");
      // the losing side of the same tie (:429)
      ContO mc = car(m);
      ContO md2 = car(m);
      mc.maxR = 10;
      md2.maxR = 10;
      Madness om2 = mk(m, new Record(m), 1, 50.0F);
      om2.cn = 9;
      om2.speed = 1.0F;
      Madness mm2 = mk(m, new Record(m), 1, 100.0F);
      mm2.cn = 3;
      mm2.speed = 1.0F;
      mm2.colide(mc, om2, md2);
      Check.ok(!mm2.dominate[1], "colide_moment_1_loses_to_2");

      // u23. dark and bright planes drive the hsb brightness clamps
      // (:193-198 and :206-212)
      ContO dk = car(bandm);
      dk.p[0] = planeCol(bandm, 3, 0, 35, false, new int[]{30, 40, 50});
      dk.npl = 1;
      Madness dkm = new Madness(bandm, new Record(bandm), null, 1);
      dkm.cn = 0;
      dkm.mtouch = true;
      dkm.regy(0, 150.0F, dk);
      Check.eq(dk.p[0].hsb[2], 0.5F, "regy_dark_plane_hsb2_0_5");
      ContO br = car(bandm);
      br.p[0] = planeCol(bandm, 3, 0, 55, false, new int[]{240, 250, 255});
      br.npl = 1;
      Madness brm = new Madness(bandm, new Record(bandm), null, 1);
      brm.cn = 0;
      brm.mtouch = true;
      brm.regy(0, 150.0F, br);
      Check.eq(br.p[0].hsb[2], 0.8F, "regy_bright_plane_hsb2_0_8");
      // the same clamps through regz/regx
      ContO dz = car(bandm);
      dz.p[0] = planeCol(bandm, 3, 0, 35, false, new int[]{30, 40, 50});
      dz.npl = 1;
      Madness dzm = new Madness(bandm, new Record(bandm), null, 1);
      dzm.cn = 0;
      dzm.regz(0, 150.0F, dz);
      Check.eq(dz.p[0].hsb[2], 0.5F, "regz_dark_plane_hsb2_0_5");
      ContO dx = car(bandm);
      dx.p[0] = planeCol(bandm, 3, 0, 55, false, new int[]{240, 250, 255});
      dx.npl = 1;
      Madness dxm = new Madness(bandm, new Record(bandm), null, 1);
      dxm.cn = 0;
      dxm.regx(0, 150.0F, dx);
      Check.eq(dx.p[0].hsb[2], 0.8F, "regx_bright_plane_hsb2_0_8");

      // u24. ground dust with var27==0: a skd==0 floor tile under all
      // wheels selects the :1429 increment arm
      ContO gz = car(m);
      gz.y = 240;
      Trackers gzt = new Trackers();
      gzt.nt = 1;
      floor(gzt, 0, 0, 240, 0, 0, 20, 100, 100, 40);
      Madness gzm = mk(m, new Record(m), 1, 98.0F);
      for (int i = 0; i < 4; i++) {
         gzm.scy[i] = 100.0F;
      }
      gzm.nofocus = true;
      gzm.drive(new Control(m), gz, gzt, new CheckPoints());
      Check.eq(gz.y, 240, "ground_skd0_dust_y_240");

      // u25. power==0 keeps the :2409 floor instead of decaying
      ContO p0 = car(m);
      p0.y = 240;
      Madness p0m = mk(m, new Record(m), 1, 0.0F);
      p0m.nofocus = true;
      p0m.drive(new Control(m), p0, new Trackers(), new CheckPoints());
      Check.eq(p0m.power, 0.0F, "power0_stays_0");

      // u26. capsized + !dest + trcnt!=0 resets the tracker counters
      // (:2367-2377)
      ContO cr = car(m);
      cr.y = 240;
      Madness crm = mk(m, new Record(m), 1, 98.0F);
      crm.pzy = 180;
      crm.pxy = 0;
      crm.trcnt = 5;
      crm.travxy = 10;
      crm.travzy = 10;
      crm.travxz = 10;
      crm.ftab = true;
      crm.rtab = true;
      crm.btab = true;
      crm.surfer = true;
      crm.nofocus = true;
      crm.drive(new Control(m), cr, new Trackers(), new CheckPoints());
      Check.eq(crm.trcnt, 0, "caps_reset_trcnt_0");
      Check.eq(crm.travxy, 0, "caps_reset_travxy_0");
      Check.ok(!crm.surfer, "caps_reset_surfer_false");

      // u27. cntouch reaches 10 -> mtouch (:1951-1952): one wheel only,
      // so the tracker contact sets wtouch without mtouch.
      Trackers c1t = new Trackers();
      c1t.nt = 1;
      floor(c1t, 0, 10, 10, 0, 0, 20, 5, 5, 20);
      ContO c1 = car(m);
      c1.y = 0;
      c1.x = 10;
      c1.keyx[1] = 50;
      c1.keyx[2] = 50;
      c1.keyx[3] = 50;
      Madness c1m = mk(m, new Record(m), 1, 98.0F);
      c1m.cntouch = 9;
      c1m.nofocus = true;
      c1m.drive(new Control(m), c1, c1t, new CheckPoints());
      Check.eq(c1m.cntouch, 10, "cntouch_9_to_10");
      ContO c2 = car(m);
      c2.y = 0;
      c2.x = 10;
      c2.keyx[1] = 50;
      c2.keyx[2] = 50;
      c2.keyx[3] = 50;
      c1m.cntouch = 10;
      c1m.mtouch = false;
      c1m.nofocus = true;
      c1m.drive(new Control(m), c2, c1t, new CheckPoints());
      Check.ok(c1m.mtouch, "cntouch_10_arms_mtouch");

      // u28. xz>45 makes pmlt +1 (:1839-1850), and the xz==0 case pins
      // the other arm (already covered by hardwall).
      ContO pmc = car(m);
      pmc.y = 240;
      pmc.xz = 90;
      Trackers pmt = new Trackers();
      pmt.nt = 1;
      wallz(pmt, 0, 0, 240, 0, 90, 100, 100, 60, 20);
      Madness pmm = mk(m, new Record(m), 1, 98.0F);
      for (int i = 0; i < 4; i++) {
         pmm.scz[i] = 10.0F;
      }
      pmm.nofocus = true;
      pmm.drive(new Control(m), pmc, pmt, new CheckPoints());
      Check.eq(pmm.pmlt, 1, "pmlt_xz90_plus1");

      // u29. a push with this.im==0 sets the other's colidim around each
      // reg* call (:476-495 and :518-537). Impulses stay below the 100
      // damage gate so the other car never reaches an xt site.
      ContO ia = car(m);
      ContO ib = car(m);
      ia.maxR = 10;
      ib.maxR = 10;
      Madness iother = mk(m, new Record(m), 1, 75.0F);
      iother.speed = 5.0F;
      Madness ime = mk(m, new Record(m), 0, 75.0F);
      ime.speed = 10.0F;
      ime.scx[0] = 10.0F;
      ime.scz[0] = 10.0F;
      ime.colide(ia, iother, ib);
      Check.eq(ime.lastcolido, 0, "colide_im0_this_lastcolido_0");
      Check.ok(iother.scx[0] > 0.0F, "colide_im0_push_other_scx");
      Check.ok(iother.scz[0] > 0.0F, "colide_im0_push_other_scz");

      // u30. the second-operand squash gate (oy below flipy+squash) and
      // the glass decrement in the squash block (:246-248,:258-259)
      ContO sq2 = car(m);
      sq2.zy = 180;
      sq2.p[0] = plane(m, 3, 0, 0, false, false, -100);
      sq2.p[1] = plane(m, 3, 0, 0, true, false, -200);
      sq2.npl = 2;
      Madness sq2m = new Madness(m, new Record(m), null, 1);
      sq2m.cn = 0;
      sq2m.mtouch = true;
      sq2m.regy(0, 150.0F, sq2);
      sq2m.regy(0, 150.0F, sq2);
      Check.ok(sq2.p[0].oy[0] <= -50, "regy_squash_second_operand_oy");
      Check.eq(sq2.p[1].gr, -8, "regy_squash_glass_gr_minus8");

      // u31. premangled hsb values exercise the regy/regz/regx band
      // clamps that the Plane constructor normally pre-applies
      Medium hm = new VecMedium();
      Plane hp2 = plane(hm, 3, 0, 35, false, false, 0);
      hp2.hsb[1] = 0.9F;
      hp2.hsb[2] = 0.1F;
      ContO hc = car(hm);
      hc.p[0] = hp2;
      hc.npl = 1;
      Madness hcm = new Madness(hm, new Record(hm), null, 1);
      hcm.cn = 0;
      hcm.mtouch = true;
      hcm.regy(0, 150.0F, hc);
      Check.eq(hp2.hsb[1], 0.1F, "regy_band30_hsb1_0_1");
      Check.eq(hp2.hsb[2], 0.5F, "regy_band30_hsb2_0_5");
      Plane hp3 = plane(hm, 3, 0, 55, false, false, 0);
      hp3.hsb[2] = 0.95F;
      ContO hc3 = car(hm);
      hc3.p[0] = hp3;
      hc3.npl = 1;
      Madness hc3m = new Madness(hm, new Record(hm), null, 1);
      hc3m.cn = 0;
      hc3m.mtouch = true;
      hc3m.regy(0, 150.0F, hc3);
      Check.eq(hp3.hsb[2], 0.8F, "regy_band50_hsb2_0_8");
      Plane hz = plane(hm, 3, 0, 35, false, false, 0);
      hz.hsb[1] = 0.9F;
      hz.hsb[2] = 0.1F;
      ContO hzc = car(hm);
      hzc.p[0] = hz;
      hzc.npl = 1;
      Madness hzm = new Madness(hm, new Record(hm), null, 1);
      hzm.cn = 0;
      hzm.regz(0, 150.0F, hzc);
      Check.eq(hz.hsb[2], 0.5F, "regz_band30_hsb2_0_5");
      Plane hz2 = plane(hm, 3, 0, 55, false, false, 0);
      hz2.hsb[2] = 0.95F;
      ContO hzc2 = car(hm);
      hzc2.p[0] = hz2;
      hzc2.npl = 1;
      Madness hz2m = new Madness(hm, new Record(hm), null, 1);
      hz2m.cn = 0;
      hz2m.regz(0, 150.0F, hzc2);
      Check.eq(hz2.hsb[2], 0.8F, "regz_band50_hsb2_0_8");
      Plane hx = plane(hm, 3, 0, 35, false, false, 0);
      hx.hsb[1] = 0.9F;
      hx.hsb[2] = 0.1F;
      ContO hxc = car(hm);
      hxc.p[0] = hx;
      hxc.npl = 1;
      Madness hxm = new Madness(hm, new Record(hm), null, 1);
      hxm.cn = 0;
      hxm.regx(0, 150.0F, hxc);
      Check.eq(hx.hsb[2], 0.5F, "regx_band30_hsb2_0_5");
      Plane hx2 = plane(hm, 3, 0, 55, false, false, 0);
      hx2.hsb[2] = 0.95F;
      ContO hxc2 = car(hm);
      hxc2.p[0] = hx2;
      hxc2.npl = 1;
      Madness hx2m = new Madness(hm, new Record(hm), null, 1);
      hx2m.cn = 0;
      hx2m.regx(0, 150.0F, hxc2);
      Check.eq(hx2.hsb[2], 0.8F, "regx_band50_hsb2_0_8");

      // u32. missedcp reset for im!=0 with an active focus (:2188-2190)
      ContO fmis = car(m);
      fmis.y = 240;
      fmis.x = 30000;
      Madness fmism = mk(m, new Record(m), 1, 98.0F);
      fmism.focus = -1;
      fmism.missedcp = 5;
      fmism.nofocus = true;
      fmism.drive(new Control(m), fmis, new Trackers(), frc);
      Check.eq(fmism.missedcp, 0, "missedcp_im1_minus1_cleared");

      // u33. nmlt=1 needs |xz-45| > 90: xz=-90 gives 135 (:1852-1862)
      ContO nm = car(m);
      nm.y = 240;
      nm.xz = -90;
      Trackers nmt = new Trackers();
      nmt.nt = 1;
      wallz(nmt, 0, 0, 240, 0, -90, 100, 100, 60, 20);
      Madness nmm = mk(m, new Record(m), 1, 98.0F);
      for (int i = 0; i < 4; i++) {
         nmm.scz[i] = -10.0F;
      }
      nmm.nofocus = true;
      nmm.drive(new Control(m), nm, nmt, new CheckPoints());
      Check.eq(nmm.nmlt, 1, "nmlt_xz_minus90_plus1");

      // u34. roted fix point with dist==0 sets fcnt=8 (:2241-2242)
      ContO rf = car(m);
      rf.y = 240;
      CheckPoints rfc = new CheckPoints();
      rfc.fn = 1;
      rfc.roted[0] = true;
      Madness rfm = mk(m, new Record(m), 1, 98.0F);
      rfm.nofocus = true;
      rfm.drive(new Control(m), rf, new Trackers(), rfc);
      Check.eq(rf.fcnt, 8, "fix_roted_dist0_fcnt_8");

      // u35. surfer latch: mtouch false and a wall id supplied by Control
      // (:2291-2294)
      ContO sv = car(m);
      sv.y = 50;
      Madness svm = mk(m, new Record(m), 1, 98.0F);
      Control svc = new Control(m);
      svc.wall = 3;
      svm.nofocus = true;
      svm.drive(svc, sv, new Trackers(), new CheckPoints());
      Check.ok(svm.surfer, "surfer_wall_id_latches");

      // u36. a skd==2 sloped tile under all four wheels arms mtouch via
      // var78==4 (:1717-1743)
      ContO sk2 = car(m);
      sk2.y = 240;
      Trackers sk2t = new Trackers();
      sk2t.nt = 1;
      wallz(sk2t, 0, 0, 240, 0, 45, 100, 100, 60, 20);
      sk2t.skd[0] = 2;
      Madness sk2m = mk(m, new Record(m), 1, 98.0F);
      sk2m.nofocus = true;
      sk2m.drive(new Control(m), sk2, sk2t, new CheckPoints());
      Check.ok(sk2m.mtouch, "slope_skd2_mtouch");

      // u37. dcnt<0 clamp in the grip loop (:1362-1366)
      ContO dcc = car(m);
      dcc.y = 240;
      Madness dccm = mk(m, new Record(m), 1, 98.0F);
      dccm.mtouch = true;
      dccm.dcnt = 1;
      dccm.speed = 0.0F;
      dccm.nofocus = true;
      dccm.drive(new Control(m), dcc, new Trackers(), new CheckPoints());
      Check.eq(dccm.dcnt, 0, "dcnt_neg_clamped_0");

      // u38. the small-ucomp/dcomp floors in the air block (:844-852,
      // :868-876)
      ContO fl = car(m);
      fl.y = 50;
      Madness flm = mk(m, new Record(m), 1, 98.0F);
      flm.loop = 2;
      for (int i = 0; i < 4; i++) {
         flm.scy[i] = -200.0F;
      }
      Control flc = new Control(m);
      flc.up = true;
      flc.down = true;
      flm.nofocus = true;
      flm.drive(flc, fl, new Trackers(), new CheckPoints());
      Check.eq(flm.ucomp, 5.5F, "air_floor_ucomp_5_5");
      Check.eq(flm.dcomp, 5.5F, "air_floor_dcomp_5_5");

      // u39. regy angle normalisation for non-zero zy/xy (:132-162):
      // zy=180 lands in the "upside-down" band, zy=10 in the "upright"
      // band; the same for xy.
      Medium am2m = new VecMedium();
      for (int ab = 0; ab < 2; ab++) {
         ContO ac2 = car(am2m);
         ac2.zy = ab == 0 ? 180 : 10;
         ac2.xy = ab == 0 ? 10 : 180;
         ac2.p[0] = plane(am2m, 3, 0, 0, false, false, 0);
         ac2.npl = 1;
         Madness a2m = new Madness(am2m, new Record(am2m), null, 1);
         a2m.cn = 0;
         a2m.mtouch = true;
         a2m.regy(0, 150.0F, ac2);
         Check.eq(a2m.nbsq, 1, "regy_angle_band" + ab + "_nbsq");
      }

      // u40. colide push with negative impulses reaches both -300 clamps
      // (:462-463,:471-472,:513-514)
      ContO na = car(m);
      ContO nb = car(m);
      na.maxR = 10;
      nb.maxR = 10;
      Madness no = mk(m, new Record(m), 1, 70.0F);
      no.speed = 4.0F;
      Madness nm2 = mk(m, new Record(m), 1, 70.0F);
      nm2.speed = 8.0F;
      for (int i = 0; i < 4; i++) {
         nm2.scx[i] = 800.0F;
         nm2.scz[i] = 800.0F;
         no.scx[i] = -500.0F;
         no.scz[i] = -500.0F;
      }
      nm2.colide(na, no, nb);
      Check.ok(nm2.dominate[1], "colide_neg_clamp_dominates");
      // the push repeats once per wheel pair, so slot 0 accumulates:
      // 800 -> 1100 -> 1400 -> 1700 -> 2000 (var14 pinned at +300 each)
      Check.eq(nm2.scx[0], MAD.floatv("colide_push_walk"), "colide_neg_clamp_this_scx_2000");
      // this side negative, other near zero -> var14/var17 clamp at -300
      ContO nc2 = car(m);
      ContO nd2 = car(m);
      nc2.maxR = 10;
      nd2.maxR = 10;
      Madness no2 = mk(m, new Record(m), 1, 70.0F);
      no2.speed = 4.0F;
      Madness nm3 = mk(m, new Record(m), 1, 70.0F);
      nm3.speed = 8.0F;
      for (int i = 0; i < 4; i++) {
         nm3.scx[i] = -400.0F;
         nm3.scz[i] = -400.0F;
         no2.scx[i] = -100.0F;
         no2.scz[i] = -100.0F;
      }
      // force domination by a bigger moment
      nm3.cn = 9;
      nm3.colide(nc2, no2, nd2);
      Check.ok(nm3.dominate[1], "colide_neg_this_dominates");
      // var14 clamps at -300, so the other slot lands on -400 while this
      // slot walks -400 -> -200 -> 0 and then stops (weaker than the other)
      Check.eq(no2.scx[0], -700.0F, "colide_neg_other_scx_minus700");

      // u41. pzy/pxy beyond 360 exercise the >270 normalisation loops
      // (:739-755)
      ContO w4 = car(m);
      w4.y = 250;
      Madness w4m = mk(m, new Record(m), 1, 98.0F);
      w4m.pzy = MAD.intv("capsize_over");
      w4m.pxy = MAD.intv("capsize_over");
      w4m.nofocus = true;
      w4m.drive(new Control(m), w4, new Trackers(), new CheckPoints());
      Check.eq(w4m.pzy, MAD.intv("capsize_over") - MAD.intv("capsize_full"), "caps_pzy400_normalises_40");
      Check.eq(w4m.pxy, MAD.intv("capsize_over") - MAD.intv("capsize_full"), "caps_pxy400_normalises_40");

      // u42. dcnt clamp needs var27==0: a flat skd-0 tile keeps
      // var64 == grip so the else arm runs (:1362-1366)
      ContO dcd = car(m);
      dcd.y = 240;
      Trackers dct = new Trackers();
      dct.nt = 1;
      floor(dct, 0, 0, 240, 0, 0, 20, 100, 100, 40);
      Madness dc2m = mk(m, new Record(m), 1, 98.0F);
      dc2m.mtouch = true;
      dc2m.dcnt = 1;
      dc2m.speed = 0.0F;
      dc2m.nofocus = true;
      dc2m.drive(new Control(m), dcd, dct, new CheckPoints());
      Check.eq(dc2m.dcnt, 0, "dcnt_skd0_clamped_0");

      // u43. high scy on a slope crosses the 0.3/0.4 dust caps and the
      // cn==7 bounce floor (:1487-1517,:1573-1583)
      ContO hs = car(m);
      hs.y = 240;
      Trackers hst = new Trackers();
      hst.nt = 1;
      wallz(hst, 0, 0, 240, 0, 45, 100, 100, 60, 20);
      hst.skd[0] = 1;
      Madness hsm = mk(m, new Record(m), 1, 98.0F);
      hsm.cn = 7;
      for (int i = 0; i < 4; i++) {
         hsm.scy[i] = 150.0F;
      }
      hsm.nofocus = true;
      hsm.drive(new Control(m), hs, hst, new CheckPoints());
      Check.ok(hs.y != 0, "highscy_slope_y_moved");

      // u44. asymmetric wheel heights drive the wheel-pair angle arms
      // (:1759-1837,:1879-1885)
      ContO ww2 = car(m);
      ww2.y = 0;
      Madness ww2m = mk(m, new Record(m), 1, 98.0F);
      ww2m.scy[0] = 0.0F;
      ww2m.scy[1] = 10.0F;
      ww2m.scy[2] = 20.0F;
      ww2m.scy[3] = 30.0F;
      ww2m.nofocus = true;
      ww2m.drive(new Control(m), ww2, new Trackers(), new CheckPoints());
      Check.ok(ww2m.pzy != 0 || ww2m.pxy != 0, "wheel_pair_asymmetry_attitude");

      // u45. the hcaught==false arm of the cotchinow gate (:2336)
      ContO pc = car(m);
      pc.y = 240;
      Record pcr = new Record(m);
      Madness pcm = mk(m, pcr, 0, 0.0F);
      pcm.gtouch = true;
      pcm.trcnt = 9;
      pcm.travxy = 400;
      pcm.travzy = 400;
      pcm.travxz = 400;
      pcm.surfer = true;
      pcm.nofocus = true;
      pcm.drive(new Control(m), pc, new Trackers(), new CheckPoints());
      Check.ok(pcm.powerup > 60.0F, "powerup_im0_above_60");
      Check.ok(!pcr.hcaught, "powerup_im0_hcaught_false_below_300");

      // u46. dested!=0 with dest true skips the rewrite (:2426)
      ContO de = car(m);
      de.y = 240;
      CheckPoints dec = new CheckPoints();
      dec.dested[1] = 2;
      Madness dem = mk(m, new Record(m), 1, 98.0F);
      dem.dest = true;
      dem.nofocus = true;
      dem.drive(new Control(m), de, new Trackers(), dec);
      Check.eq(dec.dested[1], 2, "dest_dested_already_set_kept");

      // u47. flat tracker contact with a pre-lifted scy crosses the
      // 0.3/0.4 caps and, with cn==7, the 1.1 bounce floor
      // (:1481-1523)
      ContO fh = car(m);
      fh.y = 0;
      Trackers fht = new Trackers();
      fht.nt = 1;
      floor(fht, 0, 0, 150, 0, 0, 20, 100, 100, 100);
      Madness fhm = mk(m, new Record(m), 1, 98.0F);
      fhm.cn = 7;
      fhm.pxy = 90;
      fhm.pzy = 90;
      for (int i = 0; i < 4; i++) {
         fhm.scy[i] = 150.0F;
      }
      fhm.nofocus = true;
      fhm.drive(new Control(m), fh, fht, new CheckPoints());
      Check.ok(fh.y != 0, "flat_high_scy_contact");
      // a cn==7 bounce (0.8) with a zero angle term floors at 1.1
      ContO fb = car(m);
      fb.y = 0;
      Trackers fbt = new Trackers();
      fbt.nt = 1;
      floor(fbt, 0, 0, 150, 0, 0, 20, 100, 100, 100);
      Madness fbm = mk(m, new Record(m), 1, 98.0F);
      fbm.cn = 7;
      fbm.pxy = 0;
      fbm.pzy = 0;
      for (int i = 0; i < 4; i++) {
         fbm.scy[i] = 150.0F;
      }
      fbm.nofocus = true;
      fbm.drive(new Control(m), fb, fbt, new CheckPoints());
      Check.ok(fb.y != 0, "flat_bounce_floor_contact");

      // u48. wall contact with mtouch already true (var56) zeroes the
      // angle term (:1538-1547,:1571-1579)
      ContO wm2 = car(m);
      wm2.y = 240;
      Trackers wm2t = new Trackers();
      wm2t.nt = 1;
      wallz(wm2t, 0, 0, 240, 0, 90, 100, 100, 60, 20);
      Madness wm2m = mk(m, new Record(m), 1, 98.0F);
      wm2m.cn = 7;
      wm2m.mtouch = true;
      wm2m.speed = 50.0F;
      for (int i = 0; i < 4; i++) {
         wm2m.scz[i] = 10.0F;
      }
      wm2m.nofocus = true;
      wm2m.drive(new Control(m), wm2, wm2t, new CheckPoints());
      Check.eq(wm2m.skid, 2, "wall_mtouch_skid2");

      // u49. steep slopes (|zy|,|xy| > 50) hit the 1.0 ratio floors
      // (:1663-1665,:1703-1705)
      ContO st = car(m);
      st.y = 240;
      Trackers stt = new Trackers();
      stt.nt = 1;
      wallz(stt, 0, 0, 240, 0, 60, 100, 100, 60, 20);
      Madness stm = mk(m, new Record(m), 1, 98.0F);
      stm.nofocus = true;
      stm.drive(new Control(m), st, stt, new CheckPoints());
      Check.ok(st.y != 0, "steep_zy60_contact");
      ContO st2 = car(m);
      st2.y = 240;
      Trackers st2t = new Trackers();
      st2t.nt = 1;
      wallx(st2t, 0, 0, 240, 0, 60, 100, 100, 60, 20);
      st2t.skd[0] = 2;
      Madness st2m = mk(m, new Record(m), 1, 98.0F);
      st2m.nofocus = true;
      st2m.drive(new Control(m), st2, st2t, new CheckPoints());
      Check.ok(st2m.mtouch, "steep_xy60_skd2_mtouch");

      // u50. xz=180 makes |xz+-45| exceed 180, so both pmlt/nmlt
      // normalisation subtractions run (:1839-1862)
      ContO n2 = car(m);
      n2.y = 240;
      n2.xz = 300;
      Trackers n2t = new Trackers();
      n2t.nt = 1;
      wallz(n2t, 0, 0, 240, 0, 90, 100, 100, 60, 20);
      Madness n2m = mk(m, new Record(m), 1, 98.0F);
      for (int i = 0; i < 4; i++) {
         n2m.scz[i] = 10.0F;
      }
      n2m.nofocus = true;
      n2m.drive(new Control(m), n2, n2t, new CheckPoints());
      Check.eq(n2m.pmlt, -1, "xz300_pmlt_minus1");
      Check.eq(n2m.nmlt, 1, "xz300_nmlt_plus1");

      // u51. one-sided wheel asymmetry reaches the var83>var81 fold
      // (:1879-1885)
      ContO aw = car(m);
      aw.y = 0;
      Madness awm = mk(m, new Record(m), 1, 98.0F);
      awm.scy[0] = 0.0F;
      awm.scy[1] = 0.0F;
      awm.scy[2] = 0.0F;
      awm.scy[3] = 60.0F;
      awm.nofocus = true;
      awm.drive(new Control(m), aw, new Trackers(), new CheckPoints());
      Check.ok(awm.pxy != 0 || awm.pzy != 0, "asym_scy_wheel_fold");

      // u52. the zy==-90 wall with mtouch at entry: pzy=180 flips the
      // grip target scz negative so the :1528 branch fires with var56
      // true, zeroing var93 and flooring var94 at 1.1 (:1538-1551)
      ContO wz = car(m);
      wz.y = 240;
      Trackers wzt = new Trackers();
      wzt.nt = 1;
      wallz(wzt, 0, 0, 240, 0, -90, 100, 100, 60, 20);
      Madness wzm = mk(m, new Record(m), 1, 98.0F);
      wzm.cn = 7;
      wzm.mtouch = true;
      wzm.speed = 50.0F;
      wzm.pzy = 180;
      wzm.nofocus = true;
      wzm.drive(new Control(m), wz, wzt, new CheckPoints());
      Check.eq(wzm.skid, 0, "wall_mtouch_minuszy_skid_reset_0");

      // ---- t. Medium: the paths the renderer suite leaves untouched ----
      Medium mm = new Medium();
      // palette low clamps (:131-132,:140-141,:149-150, setgrnd/setsky)
      mm.setfade(-10, -10, -10);
      Check.eq(mm.cfade[0], 0, "medium_setfade_low_clamp");
      Check.eq(mm.cfade[2], 0, "medium_setfade2_low_clamp");
      mm.setgrnd(-5, -5, -5);
      Check.eq(mm.cgrnd[0], 0, "medium_setgrnd_low_clamp");
      mm.setsky(-7, -7, -7);
      Check.eq(mm.csky[0], 0, "medium_setsky_low_clamp");
      mm.setfade(300, 300, 300);
      Check.eq(mm.cfade[0], 255, "medium_setfade_high_clamp");
      // adjstfade growth: +500 toward origfade then clamp (:435-441)
      Medium mf = new Medium();
      mf.origfade = 9000;
      mf.fade[0] = 5000;
      mf.adjstfade(15.0F);
      Check.eq(mf.fade[0], 5500, "medium_adjstfade_grow_500");
      Check.eq(mf.fade[7], 44000, "medium_adjstfade_fadfrom_44000");
      mf.fade[0] = 8900;
      mf.adjstfade(15.0F);
      Check.eq(mf.fade[0], 9000, "medium_adjstfade_clamps_origfade");
      Medium mf2 = new Medium();
      mf2.adjstfade(10.0F);
      Check.eq(mf2.fade[0], 3000, "medium_adjstfade_floor_3000");
      // d(Graphics) with the clamps and both zy signs (:154-283)
      // a SMALL raster: d() only needs a real Graphics; the polygon
      // branches are identical at any size and the pixels are not pinned
      java.awt.image.BufferedImage img = new java.awt.image.BufferedImage(64, 64,
            java.awt.image.BufferedImage.TYPE_INT_RGB);
      java.awt.Graphics g = img.getGraphics();
      Medium mdw = new Medium();
      mdw.zy = 95;
      mdw.xz = 400;
      mdw.y = 10;
      mdw.d(g);
      Check.eq(mdw.zy, 90, "medium_d_zy_clamp_90");
      Check.eq(mdw.xz, 40, "medium_d_xz_wrap_40");
      Check.eq(mdw.y, 0, "medium_d_y_clamp_0");
      Check.eq(mdw.ground, 250, "medium_d_ground_250");
      Medium mdw2 = new Medium();
      mdw2.zy = -95;
      mdw2.xz = -10;
      mdw2.y = -1;
      mdw2.d(g);
      Check.eq(mdw2.zy, -90, "medium_d_zy_clamp_minus90");
      Check.eq(mdw2.xz, 350, "medium_d_xz_wrap_350");
      Check.eq(mdw2.ground, 251, "medium_d_ground_251");
      Medium mdw3 = new Medium();
      mdw3.flex = 2;
      mdw3.zy = 30;
      mdw3.d(g);
      Check.eq(mdw3.zy, 30, "medium_d_flex2_zy_kept");
      // watch(): td respawn, both x signs and the trns decay (:285-326)
      Medium mwt = new VecMedium();
      ContO wc = car(mwt);
      wc.x = 100;
      mwt.flex = 1;
      mwt.td = true;
      mwt.trns = 1;
      mwt.watch(wc, 0);
      Check.eq(mwt.flex, 0, "medium_watch_clears_flex");
      Check.ok(!mwt.td, "medium_watch_clears_td");
      Check.eq(mwt.x, 500, "medium_watch_x_500");
      Check.eq(mwt.y, -520, "medium_watch_y_minus520");
      Check.eq(mwt.z, 5000, "medium_watch_z_5000");
      Check.eq(mwt.trns, 1, "medium_watch_trns_stays_1");
      mwt.x = 0;
      mwt.y = 0;
      mwt.z = 0;
      mwt.trns = 5;
      mwt.watch(wc, 90);
      Check.eq(mwt.trns, 4, "medium_watch_trns_decays_4");
      Check.eq(mwt.xz, -156, "medium_watch_xz_minus156");
      // around(): both clamp arms and the wide-shot advance (:334-396)
      Medium mar = new Medium();
      mar.flex = 3;
      mar.adv = -510;
      mar.around(wc, false);
      Check.eq(mar.flex, 0, "medium_around_clears_flex");
      Check.eq(mar.adv, -508, "medium_around_adv_minus508");
      Check.ok(!mar.vert, "medium_around_vert_false");
      Check.eq(mar.trns, 5, "medium_around_trns_5");
      Medium mar2 = new Medium();
      mar2.adv = 905;
      mar2.around(wc, false);
      Check.eq(mar2.adv, 907, "medium_around_adv_907");
      Check.ok(mar2.vert, "medium_around_vert_true");
      Medium mar3 = new Medium();
      mar3.around(wc, true);
      Check.eq(mar3.adv, 480, "medium_around_closeup_adv_480");
      Check.eq(mar3.vxz, 184, "medium_around_closeup_vxz_184");

      // aroundtrack(): the tracker walk, smoothing and ptr wrap
      // (:454-504). hit falls 3000 per call from 60000, clamps at 5000,
      // then trx/trz smooth toward cp.x/ptr and ptr wraps after ptcnt 7.
      CheckPoints atcp = new CheckPoints();
      atcp.n = 1;
      atcp.x[0] = 30000;
      atcp.z[0] = 0;
      Medium mat = new Medium();
      for (int i = 0; i < 38; i++) {
         mat.aroundtrack(atcp);
      }
      Check.eq(mat.hit, 5000, "medium_aroundtrack_hit_5000");
      Check.eq(mat.nrnd, 1, "medium_aroundtrack_nrnd_1");
      Check.eq(mat.ptr, 0, "medium_aroundtrack_ptr_wrapped_0");
      Check.eq(mat.trx, 26349L, "medium_aroundtrack_trx_26349");
      Check.eq(mat.zy, 9, "medium_aroundtrack_zy_9");
      // zy in (9,30) decays by 1 (:499-500)
      mat.zy = 20;
      mat.aroundtrack(atcp);
      Check.eq(mat.zy, 19, "medium_aroundtrack_zy_decays_19");
      // vxz > 360 wraps (:481-483); 120 calls reach 256 -> 420 -> 60
      Medium mat2 = new Medium();
      for (int i = 0; i < 120; i++) {
         mat2.aroundtrack(atcp);
      }
      Check.eq(mat2.vxz, 60, "medium_aroundtrack_vxz_wrap_60");
      // sin/cos wrap for negative angles (:99-109,:554-564)
      Medium msc = new Medium();
      Check.eq(msc.cos(-30), 0.8660254F, "medium_cos_neg30");
      Check.eq(msc.sin(360), 0.0F, "medium_sin_360");
      // the per-channel clamps on every palette setter
      Medium mpc = new Medium();
      mpc.setfade(300, -10, 300);
      Check.eq(mpc.cfade[1], 0, "medium_setfade_ch1_low");
      Check.eq(mpc.cfade[2], 255, "medium_setfade_ch2_high");
      mpc.setgrnd(-5, 300, -5);
      Check.eq(mpc.cgrnd[1], 255, "medium_setgrnd_ch1_high");
      Check.eq(mpc.cgrnd[2], 0, "medium_setgrnd_ch2_low");
      mpc.setsky(300, -7, -7);
      Check.eq(mpc.csky[0], 255, "medium_setsky_ch0_high");
      Check.eq(mpc.csky[1], 0, "medium_setsky_ch1_low");
      mpc.setgrnd(300, 300, 300);
      Check.eq(mpc.cgrnd[0], 255, "medium_setgrnd_ch0_high");
      Check.eq(mpc.cgrnd[2], 255, "medium_setgrnd_ch2_high");
      mpc.setsky(-7, 300, 300);
      Check.eq(mpc.csky[1], 255, "medium_setsky_ch1_high");
      Check.eq(mpc.csky[2], 255, "medium_setsky_ch2_high");
      // d() with zy==0 skips the rotated projection (:188,:241)
      Medium md0 = new Medium();
      md0.d(g);
      Check.eq(md0.ground, 250, "medium_d_flat_ground_250");
      // watch(): the two sign arms and the >6000 td respawn
      Medium mws = new VecMedium();
      ContO wfar = car(mws);
      wfar.x = 1000;
      wfar.y = 100;
      mws.x = 0;
      mws.y = 0;
      mws.watch(wfar, 0);
      Check.eq(mws.xz, -270, "medium_watch_positive_x_xz");
      Medium mws2 = new VecMedium();
      ContO wfar2 = car(mws2);
      wfar2.x = 7000;
      mws2.x = 0;
      mws2.y = 0;
      mws2.watch(wfar2, 0);
      Check.ok(mws2.td, "medium_watch_far_sets_td");
   }

   // ---- fixture helpers ----

   /** A drive-ready ContO: the empty stream leaves npl==0, all zeros. */
   static ContO car(Medium m) {
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
      }
      return c;
   }

   static Madness mk(Medium m, Record r, int im, float power) {
      Madness md = new Madness(m, r, null, im);
      md.cn = 0;
      md.power = power;
      return md;
   }

   /** One tick with nofocus armed and im applied. */
   static Madness tick(Medium m, ContO c, Control ctrl, CheckPoints cp, int im) {
      Madness md = new Madness(m, new Record(m), null, im);
      md.cn = 0;
      md.power = 98.0F;
      md.nofocus = true;
      md.drive(ctrl, c, new Trackers(), cp);
      return md;
   }

   /** Second tick on an existing Madness (keeps its accumulated state). */
   static void tick(Medium m, ContO c, Control ctrl, CheckPoints cp, int im, Madness md) {
      md.nofocus = true;
      md.drive(ctrl, c, new Trackers(), cp);
   }

   /** One tick returning the resulting cxz for the auto-turn pins. */
   static int cxzTick(Medium m, float speed, int cxz, int mxz) {
      ContO c = car(m);
      c.y = 240;
      Madness md = mk(m, new Record(m), 1, 98.0F);
      md.speed = speed;
      md.cxz = cxz;
      md.mxz = mxz;
      md.nofocus = true;
      md.drive(new Control(m), c, new Trackers(), new CheckPoints());
      return md.cxz;
   }

   /** One flat (xy=zy=0) floor tile. */
   static void floor(Trackers tr, int i, int x, int y, int z, int skd, int dam,
         int radx, int radz, int rady) {
      tr.x[i] = x;
      tr.y[i] = y;
      tr.z[i] = z;
      tr.xy[i] = 0;
      tr.zy[i] = 0;
      tr.skd[i] = skd;
      tr.dam[i] = dam;
      tr.radx[i] = radx;
      tr.radz[i] = radz;
      tr.rady[i] = rady;
   }

   /** A wall perpendicular to x (xy=+-90). */
   static void wallx(Trackers tr, int i, int x, int y, int z, int xy,
         int radx, int radz, int rady, int dam) {
      floor(tr, i, x, y, z, 0, dam, radx, radz, rady);
      tr.xy[i] = xy;
   }

   /** A wall perpendicular to z (zy=+-90). */
   static void wallz(Trackers tr, int i, int x, int y, int z, int zy,
         int radx, int radz, int rady, int dam) {
      floor(tr, i, x, y, z, 0, dam, radx, radz, rady);
      tr.zy[i] = zy;
   }

   /** A 3-vertex plane; wz==0 is the random-scatter path. */
   static Plane plane(Medium m, int n, int wz, int bfase, boolean glass,
         boolean nocol, int oy0) {
      int[] ox = new int[n];
      int[] oz = new int[n];
      int[] oy = new int[n];
      for (int i = 0; i < n; i++) {
         ox[i] = i * 10;
         oz[i] = i * 5;
         oy[i] = oy0 + i;
      }
      Plane p = new Plane(m, new Trackers(), ox, oz, oy, n,
            new int[]{100, 120, 140}, glass, 0, 0, wz, 0, 7, bfase, false);
      p.nocol = nocol;
      return p;
   }

   /** A 3-vertex plane with an explicit colour triple. */
   static Plane planeCol(Medium m, int n, int wz, int bfase, boolean glass, int[] col) {
      int[] ox = new int[n];
      int[] oz = new int[n];
      int[] oy = new int[n];
      for (int i = 0; i < n; i++) {
         ox[i] = i * 10;
         oz[i] = i * 5;
         oy[i] = i;
      }
      Plane p = new Plane(m, new Trackers(), ox, oz, oy, n, col, glass, 0, 0, wz, 0, 7, bfase, false);
      return p;
   }
}
