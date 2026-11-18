/**
 * Pins Madness's WORLD-DEPENDENT surface (drive / reseto / reg* / colide)
 * with a SCOPED fixture: no xtGraphics at all. Every instance is im!=0
 * wherever the code would otherwise dereference xt, because the only xt
 * reads in this class are gated:
 *   - regy :164-166  `if (this.im == 0 || this.colidim) this.xt.crash(...)`
 *   - regz :302-304  same gate
 *   - regx :662-664  same gate
 *   - drive :1357    `if (this.im == 0 && !this.capsized) this.xt.skid(...)`
 *                    (inside the mtouch/skid-scatter branch :1239-1374)
 *   - drive :2231,:2244  `if (this.im == 0 && ...) this.xt.carfixed...`
 *                    (inside the fix-point loop, fn==0 here)
 *   - drive :2417-2420   im==0 wall reset only
 * and colide sets `var2.colidim = true` only when `this.im == 0` (:476,
 * :488, :518, :530), which is what can drag the OTHER car onto the crash
 * path (verified: a push with this.im==0 NPEs at Madness.java:663).
 * The only im==0 ticks used below are the four power!=98 handicap pins
 * (:934-940): they carry no trackers, never reach mtouch, and fn==0, so
 * neither xt site in drive can run - confirmed by running them with a null
 * xt before pinning them.
 *
 * Fixture decision (a): a garbage/empty-byte ContO with hand-assigned
 * package-private fields. `new ContO(new byte[0], m, t)` runs the line-based
 * ASCII .rad parser (ContO.java:207-419) with an empty stream, so it
 * allocates p=new Plane[120] (:210) and leaves every field at its declared
 * initializer (npl=0, x/y/z/xz/xy/zy/wxz/wzy/dist/maxR/grat=0, keyx/keyz
 * zero, ContO.java:10-56); exceptions are swallowed anyway (:411-416) and no
 * asset bytes are needed. car() below re-states each field those paths read
 * as 0 so the fixture cannot drift. npl==0 means the plane loops (and their
 * `this.m.random()` calls at :173, :310, :670) never execute; the only pins
 * that carry a plane are the :670/:671 exception pins, where wz==0 is
 * exactly the point and the call throws before random is reached (part
 * index) or with m==null.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B9_madness.md; every value was re-verified
 * against reference/vineflower/Madness.java (probe-measured, then asserted).
 */
public class TMadnessWorld {
   /** constants.toml [madness] - the reset power and the reg* damage gate. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("madness");

   static void run() {
      Check.group("MadnessWorld");
      K.requireKeys("power_reset", "damage_record_gate", "destruction_maxmag", "destruction_hitmag");
      Medium m = new Medium();

      // ---- B9(33-35): one all-default drive tick, im==1 ----
      // Zeroed ContO, no trackers (nt==0), CheckPoints all-zero (n==0,
      // typ[] all 0), Madness.nofocus=true: without it the focus block
      // (:2164-2168) can loop forever. im==1 keeps every im==0 xt site
      // listed in the class comment unreachable.
      ContO c = car(m);
      CheckPoints cp = new CheckPoints();
      Control ctrl = new Control(m);
      Madness md = tick(m, c, ctrl, cp, 1);
      // scy[i] += 7 at :1186-1191 -> var21[]==7 -> contO.y at :1957
      Check.eq(c.y, 7, "tick_y_scy_lift_7");
      Check.eq(c.x, 0, "tick_x_zero");
      Check.eq(c.z, 0, "tick_z_zero");
      // no ground contact (:1417) and no tracker -> the :1409 arm
      Check.eq(md.skid, 2, "tick_skid_2");
      // !mtouch at :2264-2268
      Check.eq(md.trcnt, 1, "tick_trcnt_1");
      Check.eq(md.lxz, 0, "tick_lxz_follows_xz");
      // clear == var119 + nlaps*nsp with n==0 -> :2150-2152
      Check.eq(md.nlaps, 1, "tick_nlaps_1");
      // focus==-1 -> im!=0 arm :2157-2159
      Check.eq(md.point, 1, "tick_point_1_im1");
      // focus==-1 and im!=0 leaves a zero missedcp alone (:2188-2190)
      Check.eq(md.missedcp, 0, "tick_missedcp_0_im1");
      // !mtouch takes :2264, so the :2404-2410 decay never runs
      Check.eq(md.power, 75.0F, "tick_power_untouched_75");
      Check.eq(c.wxz, 0, "tick_wxz_no_steer_0");
      Check.eq(c.wzy, 0, "tick_wzy_speed0_0");
      Check.eq(c.xy, 0, "tick_xy_zero");
      Check.eq(c.zy, 0, "tick_zy_zero");
      Check.ok(!md.mtouch, "tick_mtouch_false");
      Check.ok(!md.wtouch, "tick_wtouch_false");
      Check.eq(md.cntouch, 0, "tick_cntouch_zero");
      Check.eq(cp.dested[1], 0, "tick_dested_untouched");
      // srfcnt<10 and wall==-1 -> :2291-2297 still increments
      Check.eq(md.srfcnt, 1, "tick_srfcnt_1");

      // ---- B9(36,39,41,43): speed ladder, im==1, power==98 ----
      // cn=0: swits {50,180,280}, acelf {11,5,3}, handb[0]==7
      // (Madness.java:23-24,11-12,35). Each tick is a fresh fixture.
      ContO c0 = car(m);
      Control up0 = new Control(m);
      up0.up = true;
      Madness s0 = new Madness(m, new Record(m), null, 1);
      s0.cn = 0;
      s0.power = K.floatv("power_reset");
      s0.nofocus = true;
      s0.drive(up0, c0, new Trackers(), new CheckPoints());
      // from 0 no threshold is met -> :976-977: 5.5 + 98*11/196 == 11.0
      Check.eq(s0.speed, 11.0F, "speed_up_from_0_11");
      // wzy -= var50 with var50 = 20*11/(154*simag[0]) truncated by the
      // int cast at :1082 -> -1 (no wrap: |wzy| <= 45)
      Check.eq(c0.wzy, -1, "speed_up_from_0_wzy_minus1");

      // up at 200: the 280 plateau arm at :979 needs speed >= swits[0][2]
      // first, so :976 accelerates instead: 200 + 3.0/2 + 98*3.0/196
      // == 203.0 (acelf[0][2]==3.0, not 3.5)
      ContO c1 = car(m);
      Control up1 = new Control(m);
      up1.up = true;
      Madness s1 = new Madness(m, new Record(m), null, 1);
      s1.cn = 0;
      s1.power = K.floatv("power_reset");
      s1.speed = 200.0F;
      s1.nofocus = true;
      s1.drive(up1, c1, new Trackers(), new CheckPoints());
      Check.eq(s1.speed, 203.0F, "speed_up_at_200_accelerates_203");

      // up at 300: all three thresholds met (var52==3) -> :979 pins the
      // plateau 280/2 + 98*280/196 == 280.0 (and 300 is pulled DOWN to it)
      ContO c2 = car(m);
      Control up2 = new Control(m);
      up2.up = true;
      Madness s2 = new Madness(m, new Record(m), null, 1);
      s2.cn = 0;
      s2.power = K.floatv("power_reset");
      s2.speed = 300.0F;
      s2.nofocus = true;
      s2.drive(up2, c2, new Trackers(), new CheckPoints());
      Check.eq(s2.speed, 280.0F, "speed_up_at_300_plateau_280");

      // down at -1000: both down thresholds met -> :958 pins
      // -(180/2 + 98*180/196) == -180.0
      ContO c3 = car(m);
      Control down3 = new Control(m);
      down3.down = true;
      Madness s3 = new Madness(m, new Record(m), null, 1);
      s3.cn = 0;
      s3.power = K.floatv("power_reset");
      s3.speed = -1000.0F;
      s3.nofocus = true;
      s3.drive(down3, c3, new Trackers(), new CheckPoints());
      Check.eq(s3.speed, -180.0F, "speed_down_at_neg1000_minus180");

      // down at 100: :942-944 subtracts handb[0]/2 == 3 (INT division)
      ContO c4 = car(m);
      Control down4 = new Control(m);
      down4.down = true;
      Madness s4 = new Madness(m, new Record(m), null, 1);
      s4.cn = 0;
      s4.power = K.floatv("power_reset");
      s4.speed = 100.0F;
      s4.nofocus = true;
      s4.drive(down4, c4, new Trackers(), new CheckPoints());
      Check.eq(s4.speed, 97.0F, "speed_down_at_100_97");

      // ---- B9(40,42): the power!=98 handicap, only reachable with im==0 ----
      // :934-940 is `if (im == 0 && power != 98)`: stage 6 keeps var49==power,
      // stage 8 multiplies by 0.9, every other stage by 0.76. These ticks
      // still touch no xt: drive's only xt reads are the mtouch skid branch
      // (:1357-1359) and the fix-point loop (:2231-2232, :2244-2245), and
      // this fixture is mtouch==false with fn==0 (verified by running them
      // against a null xt).
      // power 75, speed 0, stage 1 -> var49 == 57 -> :977: 5.5 + 57*11/196
      ContO h0 = car(m);
      Control hu0 = new Control(m);
      hu0.up = true;
      Madness hp0 = new Madness(m, new Record(m), null, 0);
      hp0.cn = 0;
      hp0.power = 75.0F;
      hp0.nofocus = true;
      CheckPoints hcp0 = new CheckPoints();
      hcp0.stage = 1;
      hp0.drive(hu0, h0, new Trackers(), hcp0);
      Check.eq(hp0.speed, 8.698979F, "speed_up_from_0_power75_handicap");

      // power 75, speed 200, stage 6 -> NO handicap -> 200 + 1.5 + 75*3/196
      ContO h6 = car(m);
      Control hu6 = new Control(m);
      hu6.up = true;
      Madness hp6 = new Madness(m, new Record(m), null, 0);
      hp6.cn = 0;
      hp6.power = 75.0F;
      hp6.speed = 200.0F;
      hp6.nofocus = true;
      CheckPoints hcp6 = new CheckPoints();
      hcp6.stage = 6;
      hp6.drive(hu6, h6, new Trackers(), hcp6);
      Check.eq(hp6.speed, 202.64796F, "speed_up_at_200_power75_stage6_no_handicap");

      // stage 1 -> var49 == 57 -> 200 + 1.5 + 57*3/196
      ContO h1 = car(m);
      Control hu1 = new Control(m);
      hu1.up = true;
      Madness hp1 = new Madness(m, new Record(m), null, 0);
      hp1.cn = 0;
      hp1.power = 75.0F;
      hp1.speed = 200.0F;
      hp1.nofocus = true;
      CheckPoints hcp1 = new CheckPoints();
      hcp1.stage = 1;
      hp1.drive(hu1, h1, new Trackers(), hcp1);
      Check.eq(hp1.speed, 202.37245F, "speed_up_at_200_power75_stage1_076");

      // stage 8 is the OTHER branch: `else if (stage != 6)` -> *0.9
      ContO h8 = car(m);
      Control hu8 = new Control(m);
      hu8.up = true;
      Madness hp8 = new Madness(m, new Record(m), null, 0);
      hp8.cn = 0;
      hp8.power = 75.0F;
      hp8.speed = 200.0F;
      hp8.nofocus = true;
      CheckPoints hcp8 = new CheckPoints();
      hcp8.stage = 8;
      hp8.drive(hu8, h8, new Trackers(), hcp8);
      Check.eq(hp8.speed, 202.53316F, "speed_up_at_200_power75_stage8_09");

      // ---- B9(44): wzy wrap, speed 280 (var50 capped at 20) ----
      // wzy = -40 - 20 = -60 -> < -45 so += 45 -> -15 (:1077-1089)
      ContO c5 = car(m);
      c5.wzy = -40;
      Madness s5 = new Madness(m, new Record(m), null, 1);
      s5.cn = 0;
      s5.power = K.floatv("power_reset");
      s5.speed = 280.0F;
      s5.nofocus = true;
      s5.drive(new Control(m), c5, new Trackers(), new CheckPoints());
      Check.eq(c5.wzy, -15, "wzy_minus40_wraps_to_minus15");
      Check.eq(s5.speed, 280.0F, "wzy_tick_speed_unchanged");

      // ---- B9(45): steer clamps, turn[0]==6 (:1091-1103) ----
      ContO c6 = car(m);
      c6.wxz = -30;
      Control r6 = new Control(m);
      r6.right = true;
      Madness s6 = new Madness(m, new Record(m), null, 1);
      s6.cn = 0;
      s6.nofocus = true;
      s6.drive(r6, c6, new Trackers(), new CheckPoints());
      Check.eq(c6.wxz, -36, "wxz_right_clamps_minus36");

      ContO c7 = car(m);
      c7.wxz = 34;
      Control l7 = new Control(m);
      l7.left = true;
      Madness s7 = new Madness(m, new Record(m), null, 1);
      s7.cn = 0;
      s7.nofocus = true;
      s7.drive(l7, c7, new Trackers(), new CheckPoints());
      Check.eq(c7.wxz, 36, "wxz_left_clamps_36");

      // ---- B9(46): self-centering at |speed|<10 (:1105-1117) ----
      Check.eq(center(m, 5, 0.0F), 4, "wxz_recenter_5_to_4");
      Check.eq(center(m, 1, 0.0F), 0, "wxz_recenter_1_to_0");
      Check.eq(center(m, -5, 0.0F), -4, "wxz_recenter_neg5_to_minus4");

      // ---- B9(47): self-centering at |speed|>=10 -> snap by turn*2==12 ----
      Check.eq(center(m, 10, 20.0F), 0, "wxz_snap_10_to_0");
      Check.eq(center(m, 20, 20.0F), 8, "wxz_snap_20_to_8");

      // ---- B9(48-49): attitude snap, |speed|<=10 but !mtouch (:2004-2023) ----
      ContO c8 = car(m);
      Madness s8 = new Madness(m, new Record(m), null, 1);
      s8.cn = 0;
      s8.nofocus = true;
      s8.pxy = 10;
      s8.drive(new Control(m), c8, new Trackers(), new CheckPoints());
      // 0 + 2 + 10/2 == 7
      Check.eq(c8.xy, 7, "attitude_pxy_10_to_7");

      ContO c9 = car(m);
      Madness s9 = new Madness(m, new Record(m), null, 1);
      s9.cn = 0;
      s9.nofocus = true;
      s9.pzy = -10;
      s9.drive(new Control(m), c9, new Trackers(), new CheckPoints());
      Check.eq(c9.zy, -7, "attitude_pzy_minus10_to_minus7");

      ContO c10 = car(m);
      Madness s10 = new Madness(m, new Record(m), null, 1);
      s10.cn = 0;
      s10.nofocus = true;
      s10.pxy = 3;
      s10.drive(new Control(m), c10, new Trackers(), new CheckPoints());
      // |pxy-xy| < 4 -> deadband copies pxy exactly (:2011-2013)
      Check.eq(c10.xy, 3, "attitude_deadband_pxy_3_to_3");

      // ---- B9(22-26): reseto ----
      // Every asserted field is pre-dirtied so "reset" is observable. im==0
      // is safe here: reseto touches no xt at all (:563-648).
      ContO rc = car(m);
      // keyx all 0, keyz all 100 -> forca == (400/10000)*(bounce[0]-0.3)
      rc.keyz[0] = 100;
      rc.keyz[1] = 100;
      rc.keyz[2] = 100;
      rc.keyz[3] = 100;
      CheckPoints rcp = new CheckPoints();
      rcp.pcs = 4;
      rcp.stage = 3;
      rcp.dested[0] = 2;
      Madness rm = new Madness(m, new Record(m), null, 0);
      rm.power = 50.0F;
      rm.focus = 4;
      rm.clear = 2;
      rm.nlaps = 3;
      rm.missedcp = 3;
      rm.dest = true;
      rm.newcar = true;
      rm.hitmag = 99;
      rm.squash = 3;
      rm.nbsq = 1;
      rm.cntdest = 2;
      rm.lastcolido = 70;
      rm.speed = 12.0F;
      rm.scy[0] = 9.0F;
      rm.scx[0] = 9.0F;
      rm.scz[0] = 9.0F;
      rm.dominate[2] = true;
      rm.caught[2] = true;
      rm.cntouch = 7;
      rm.srfcnt = 5;
      rm.surfer = true;
      rm.nofocus = true;
      rm.reseto(0, rc, rcp);
      Check.eq(rm.cn, 0, "reseto_cn");
      Check.eq(rm.power, K.floatv("power_reset"), "reseto_power_98");
      Check.eq(rm.focus, -1, "reseto_focus_minus1");
      Check.eq(rm.pcleared, 4, "reseto_pcleared_is_cp_pcs");
      Check.eq(rm.clear, 0, "reseto_clear_0");
      Check.eq(rm.nlaps, 0, "reseto_nlaps_0");
      Check.eq(rm.missedcp, 0, "reseto_missedcp_0");
      Check.ok(!rm.dest, "reseto_dest_false");
      Check.ok(!rm.newcar, "reseto_newcar_false");
      Check.eq(rm.hitmag, 0, "reseto_hitmag_0");
      Check.eq(rm.squash, 0, "reseto_squash_0");
      Check.eq(rm.nbsq, 0, "reseto_nbsq_0");
      Check.eq(rm.cntdest, 0, "reseto_cntdest_0");
      Check.eq(rm.lastcolido, 0, "reseto_lastcolido_0");
      Check.eq(rm.speed, 0.0F, "reseto_speed_0");
      Check.eq(rm.scy[0], 0.0F, "reseto_scy_0");
      Check.eq(rm.scx[0], 0.0F, "reseto_scx_0");
      Check.eq(rm.scz[0], 0.0F, "reseto_scz_0");
      Check.ok(!rm.dominate[2], "reseto_dominate_cleared");
      Check.ok(!rm.caught[2], "reseto_caught_cleared");
      Check.eq(rcp.dested[0], 0, "reseto_dested_im_zeroed");
      // B9(23): cntouch/srfcnt/surfer never appear in :563-648
      Check.eq(rm.cntouch, 7, "reseto_preserves_cntouch");
      Check.eq(rm.srfcnt, 5, "reseto_preserves_srfcnt");
      Check.ok(rm.surfer, "reseto_preserves_surfer");
      Check.ok(!rm.nofocus, "reseto_clears_nofocus");
      // :593-600, mirrored exactly: sum of the four |(keyz,keyx)| wheels
      float wheel = (float)Math.sqrt(100.0 * 100.0);
      float forcaWant = (wheel + wheel + wheel + wheel) / 10000.0F * (float)((double)rm.bounce[0] - 0.3);
      Check.eq(rm.forca, forcaWant, "reseto_forca_keyz_100_formula");
      Check.ok(rm.forca > 0.0359F && rm.forca < 0.0361F, "reseto_forca_approx_0036");

      // B9(25): reseto(7,..) permanently writes moment[7], but only for im==0
      ContO m7a = car(m);
      CheckPoints cp7a = new CheckPoints();
      cp7a.stage = 10;
      Madness r7a = new Madness(m, new Record(m), null, 0);
      r7a.reseto(7, m7a, cp7a);
      Check.eq(r7a.moment[7], 1.7F, "reseto7_stage10_moment_1_7");

      ContO m7b = car(m);
      CheckPoints cp7b = new CheckPoints();
      cp7b.stage = 5;
      Madness r7b = new Madness(m, new Record(m), null, 0);
      r7b.reseto(7, m7b, cp7b);
      Check.eq(r7b.moment[7], 2.0F, "reseto7_other_stage_moment_2_0");

      ContO m7c = car(m);
      CheckPoints cp7c = new CheckPoints();
      cp7c.stage = 10;
      Madness r7c = new Madness(m, new Record(m), null, 1);
      r7c.reseto(7, m7c, cp7c);
      Check.eq(r7c.moment[7], 2.0F, "reseto7_im1_keeps_declared_2_0");

      // B9(26): cn is the row every later dammult/clrad/turn/grip lookup uses
      ContO rc9 = car(m);
      Madness r9 = new Madness(m, new Record(m), null, 0);
      r9.reseto(9, rc9, new CheckPoints());
      Check.eq(r9.cn, 9, "reseto9_cn_9");
      Check.eq(r9.dammult[r9.cn], 0.7667F, "reseto9_dammult_row_7667");

      // ---- B9(9-16): regx / regz / regy ----
      // The fixture has npl==0, so the damage-scatter loops (and their
      // m.random() calls at :173,:310,:670) are unreachable by construction
      // - the "every Plane.wz != 0" recipe is only needed once a plane
      // exists (see the exception pins below).
      Record rx = new Record(m);
      Madness md9 = new Madness(m, rx, null, 1);
      md9.cn = 0; // dammult[0]==1.0
      md9.regx(0, K.floatv("damage_record_gate"), car(m));
      // strict `> 100.0F` at :652 -> complete no-op
      Check.eq(rx.nrx[1][0], 0, "regx_100_noop_nrx");
      Check.eq(rx.magx[1][0][0], 0, "regx_100_noop_magx");
      Check.eq(md9.hitmag, 0, "regx_100_noop_hitmag");

      Record rx2 = new Record(m);
      Madness md10 = new Madness(m, rx2, null, 1);
      md10.cn = 0;
      md10.regx(0, K.floatv("damage_record_gate") + 0.5F, car(m));
      // recx(:653) records the PRE-subtraction 100.5 -> (int) -> 100
      Check.eq(rx2.nrx[1][0], 1, "regx_100_5_records");
      Check.eq(rx2.magx[1][0][0], 100, "regx_100_5_magx_100");

      // cn=9 -> dammult 0.7667 (:54): 130*0.7667 == 99.67 no-op,
      // 131*0.7667 == 100.44 fires (and records (int)100)
      Record rx3 = new Record(m);
      Madness md11 = new Madness(m, rx3, null, 1);
      md11.cn = 9;
      md11.regx(0, 130.0F, car(m));
      Check.eq(rx3.nrx[1][0], 0, "regx_cn9_130_noop");
      md11.regx(0, 131.0F, car(m));
      Check.eq(rx3.nrx[1][0], 1, "regx_cn9_131_fires");
      Check.eq(rx3.magx[1][0][0], 100, "regx_cn9_131_magx_100");

      // B9(12): regz records for im==1 and never reaches crash (im!=0,
      // colidim false), so xt may stay null
      Record rz = new Record(m);
      Madness md12 = new Madness(m, rz, null, 1);
      md12.cn = 0;
      md12.regz(0, -150.0F, car(m));
      Check.eq(rz.nrz[1][0], 1, "regz_150_im1_records");
      Check.eq(rz.magz[1][0][0], -150, "regz_150_im1_magz_neg150");
      Check.eq(md12.hitmag, 0, "regz_150_im1_hitmag_0");

      // B9(14): regy's guard is `var2 > 100` (no absolute value), so a
      // large NEGATIVE damage is fully inert
      Record ry = new Record(m);
      Madness md13 = new Madness(m, ry, null, 1);
      md13.cn = 0;
      md13.regy(0, -1000.0F, car(m));
      Check.eq(ry.nry[1][0], 0, "regy_neg1000_inert_nry");
      Check.eq(md13.hitmag, 0, "regy_neg1000_inert_hitmag");
      Check.eq(md13.nbsq, 0, "regy_neg1000_inert_nbsq");

      // B9(15): recy runs for every im (no im gate at :125); magy keeps the
      // post-dammult value and Record.mtouch[im][slot] stores this.mtouch
      for (int im = 1; im <= 4; im++) {
         Record ry2 = new Record(m);
         Madness md14 = new Madness(m, ry2, null, im);
         md14.cn = 0;
         md14.mtouch = true;
         md14.regy(0, 150.0F, car(m));
         Check.eq(ry2.nry[im][0], 1, "regy_150_im" + im + "_records");
         Check.eq(ry2.magy[im][0][0], 150, "regy_150_im" + im + "_magy_150");
         Check.ok(ry2.mtouch[im][0], "regy_150_im" + im + "_mtouch_flag");
         Check.eq(md14.nbsq, 0, "regy_150_im" + im + "_nbsq_0");
      }

      // im==0 additionally hits `this.xt.crash` (:164-166): with a null xt
      // the NPE proves recy (:125) already ran - the ordering the port needs
      final Record ry0 = new Record(m);
      final Madness md15 = new Madness(m, ry0, null, 0);
      final ContO c15 = car(m);
      md15.cn = 0;
      Check.throwsIt(new Check.Task() {
         public void run() {
            md15.regy(0, 150.0F, c15);
         }
      }, NullPointerException.class, "regy_im0_reaches_xt_crash");
      Check.eq(ry0.nry[0][0], 1, "regy_im0_recy_recorded_before_crash");

      // ---- B9(27-32): exception behavior ----
      // cn==10 is never bounds-checked by reseto (:564), so every reg*
      // indexes dammult[10] first (:123,:291,:651)
      Check.throwsIt(new Check.Task() {
         public void run() {
            Madness e = new Madness(m, new Record(m), null, 1);
            e.cn = 10;
            e.regx(0, 150.0F, car(m));
         }
      }, ArrayIndexOutOfBoundsException.class, "err_cn10_regx_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            Madness e = new Madness(m, new Record(m), null, 1);
            e.cn = 10;
            e.regy(0, 150.0F, car(m));
         }
      }, ArrayIndexOutOfBoundsException.class, "err_cn10_regy_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            Madness e = new Madness(m, new Record(m), null, 1);
            e.cn = 10;
            e.regz(0, 150.0F, car(m));
         }
      }, ArrayIndexOutOfBoundsException.class, "err_cn10_regz_aioobe");

      // part index 4: the damage is recorded BEFORE the plane loop, so the
      // AIOOBE lands in Record.recx/recy/recz - rx/ry/rz are int[5][4]..., so
      // the [im][4] write is out of bounds (Record.java:633-649, 841-848).
      // It never reaches Madness's keyx[4]/keyz[4].
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO e = car(m);
            e.p[0] = plane(m, 0);
            e.npl = 1;
            Madness x = new Madness(m, new Record(m), null, 1);
            x.cn = 0;
            x.regx(4, 150.0F, e);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_part4_regx_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO e = car(m);
            e.p[0] = plane(m, 0);
            e.npl = 1;
            Madness x = new Madness(m, new Record(m), null, 1);
            x.cn = 0;
            x.regy(4, 150.0F, e);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_part4_regy_aioobe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO e = car(m);
            e.p[0] = plane(m, 0);
            e.npl = 1;
            Madness x = new Madness(m, new Record(m), null, 1);
            x.cn = 0;
            x.regz(4, 150.0F, e);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_part4_regz_aioobe");

      // B9(29): dominate/caught are size 5 (:56-57) -> other.im==5 throws at
      // :419 before any push can run
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO a = car(m);
            ContO b = car(m);
            a.maxR = 10;
            b.maxR = 10;
            Madness other = new Madness(m, new Record(m), null, 5);
            other.cn = 0;
            other.speed = 5.0F;
            other.power = 75.0F;
            Madness e = new Madness(m, new Record(m), null, 0);
            e.cn = 0;
            e.speed = 10.0F;
            e.power = 75.0F;
            e.colide(a, other, b);
         }
      }, ArrayIndexOutOfBoundsException.class, "err_colide_other_im5_aioobe");

      // B9(30): drive's final bookkeeping indexes CheckPoints.dested[im]
      // (CheckPoints.java:20) at :2433 -> im==5 throws
      Check.throwsIt(new Check.Task() {
         public void run() {
            Madness e = new Madness(m, new Record(m), null, 5);
            e.cn = 0;
            e.nofocus = true;
            e.drive(new Control(m), car(m), new Trackers(), new CheckPoints());
         }
      }, ArrayIndexOutOfBoundsException.class, "err_drive_im5_dested_aioobe");

      // B9(31): m==null + |damage|>100 + a wz==0 plane -> NPE at m.random
      // (:671); a wz!=0 plane short-circuits :670 and nothing happens
      Check.throwsIt(new Check.Task() {
         public void run() {
            ContO e = car(m);
            e.p[0] = plane(m, 0);
            e.npl = 1;
            Madness x = new Madness(null, new Record(m), null, 1);
            x.cn = 0;
            x.regx(0, 150.0F, e);
         }
      }, NullPointerException.class, "err_null_m_wz0_npe");
      ContO cmOk = car(m);
      cmOk.p[0] = plane(m, 1);
      cmOk.npl = 1;
      Madness mdOk = new Madness(null, new Record(m), null, 1);
      mdOk.cn = 0;
      mdOk.regx(0, 150.0F, cmOk);
      Check.eq(mdOk.hitmag, 0, "err_null_m_wz1_hitmag_0");
      Check.eq(cmOk.p[0].ox[0], 0, "err_null_m_wz1_plane_untouched");

      // B9(32): rpd==null NPEs at the recx call (:653)
      Check.throwsIt(new Check.Task() {
         public void run() {
            Madness x = new Madness(m, null, null, 1);
            x.cn = 0;
            x.regx(0, 150.0F, car(m));
         }
      }, NullPointerException.class, "err_null_rpd_npe");

      // ---- B9(71-76): colide ----
      // colide reads the MADNESS impulse arrays (this.scx/scz, :441-444,
      // :456-475), never ContO.scx/scz - the ContO copy is visual only.
      ContO ca = car(m);
      ContO cb = car(m);
      ca.maxR = 10;
      cb.maxR = 10;
      Madness oth = new Madness(m, new Record(m), null, 1);
      oth.cn = 0;
      oth.speed = 5.0F;
      oth.power = 75.0F;
      Madness me = new Madness(m, new Record(m), null, 0);
      me.cn = 0;
      me.speed = 10.0F;
      me.power = 75.0F;
      me.colide(ca, oth, cb);
      // |75*10*1.2| > |75*5*1.2| -> :421-432
      Check.ok(me.dominate[1], "colide_dominate_bigger_impulse");
      Check.ok(me.caught[1], "colide_caught_set");
      // :540-542, written inside the contact pair loop; no push happened
      // (all impulse arrays 0) so no reg*/xt deref even with this.im==0
      Check.eq(oth.lastcolido, 70, "colide_other_lastcolido_70");
      // lift[0]==0 (:47) -> the per-pair lift leaves scy alone
      Check.eq(oth.scy[0], 0.0F, "colide_cn0_lift_zero");

      // B9(72): equal |power*speed*moment| -> the moment tiebreak decides
      ContO ca2 = car(m);
      ContO cb2 = car(m);
      ca2.maxR = 10;
      cb2.maxR = 10;
      Madness oth2 = new Madness(m, new Record(m), null, 2);
      oth2.cn = 3; // moment[3]==1.0
      oth2.speed = 2.0F;
      oth2.power = 60.0F;
      Madness me2 = new Madness(m, new Record(m), null, 1);
      me2.cn = 0; // moment[0]==1.2
      me2.speed = 2.0F;
      me2.power = 50.0F;
      me2.colide(ca2, oth2, cb2);
      // 50*2*1.2 == 60*2*1.0 -> :426-427
      Check.ok(me2.dominate[2], "colide_tie_moment_breaks_tie");
      Check.ok(me2.caught[2], "colide_tie_caught_set");

      // B9(73): both speeds 0 -> the :419 guard leaves BOTH flags untouched
      ContO ca3 = car(m);
      ContO cb3 = car(m);
      ca3.maxR = 10;
      cb3.maxR = 10;
      Madness oth3 = new Madness(m, new Record(m), null, 1);
      oth3.cn = 0;
      oth3.speed = 0.0F;
      oth3.power = 75.0F;
      Madness me3 = new Madness(m, new Record(m), null, 0);
      me3.cn = 0;
      me3.speed = 0.0F;
      me3.power = 75.0F;
      me3.caught[1] = true;
      me3.colide(ca3, oth3, cb3);
      Check.ok(!me3.dominate[1], "colide_zero_speed_keeps_dominate");
      Check.ok(me3.caught[1], "colide_zero_speed_keeps_caught");

      // B9(74): distance gate - 200^2 == 40000 >= (100*100 + 100*100)*1.5
      ContO ca4 = car(m);
      ContO cb4 = car(m);
      ca4.maxR = 100;
      cb4.maxR = 100;
      cb4.x = 200;
      Madness oth4 = new Madness(m, new Record(m), null, 1);
      oth4.cn = 0;
      oth4.speed = 5.0F;
      oth4.power = 75.0F;
      Madness me4 = new Madness(m, new Record(m), null, 0);
      me4.cn = 0;
      me4.speed = 10.0F;
      me4.power = 75.0F;
      me4.dominate[1] = true;
      me4.caught[1] = true;
      me4.colide(ca4, oth4, cb4);
      Check.ok(me4.dominate[1], "colide_distance_gate_keeps_dominate");
      Check.ok(!me4.caught[1], "colide_distance_gate_clears_caught");

      // B9(75): cn==1 -> lift 30; 16 pair hits x -30 per wheel slot == -120.
      // this.im==0 is xt-free here because every impulse is 0, so the push
      // blocks (:456/:498) never run.
      ContO ca5 = car(m);
      ContO cb5 = car(m);
      ca5.maxR = 10;
      cb5.maxR = 10;
      Madness oth5 = new Madness(m, new Record(m), null, 1);
      oth5.cn = 0;
      oth5.speed = 5.0F;
      oth5.power = 75.0F;
      Madness me5 = new Madness(m, new Record(m), null, 0);
      me5.cn = 1; // lift 30, moment 0.75
      me5.speed = 10.0F;
      me5.power = 75.0F;
      me5.colide(ca5, oth5, cb5);
      Check.ok(me5.dominate[1], "colide_cn1_dominates");
      Check.eq(oth5.scy[0], -120.0F, "colide_cn1_lift_scy0_minus120");
      Check.eq(oth5.scy[3], -120.0F, "colide_cn1_lift_scy3_minus120");
      Check.eq(oth5.lastcolido, 70, "colide_cn1_other_lastcolido_70");
      Check.eq(me5.lastcolido, 0, "colide_cn1_this_lastcolido_0");

      // B9(76): single-axis push. this.im MUST be !=0: at :476 `this.im==0`
      // sets the other's colidim, which would send the other's regx into
      // its (null) xt. push[0]==2, revpush term 0 because other.scx==0.
      ContO ca6 = car(m);
      ContO cb6 = car(m);
      ca6.maxR = 10;
      cb6.maxR = 10;
      Record r6o = new Record(m);
      Madness oth6 = new Madness(m, r6o, null, 1);
      oth6.cn = 0;
      oth6.speed = 5.0F;
      oth6.power = 75.0F;
      Madness me6 = new Madness(m, new Record(m), null, 1);
      me6.cn = 0;
      me6.speed = 10.0F;
      me6.power = 75.0F;
      me6.scx[0] = 100.0F;
      me6.colide(ca6, oth6, cb6);
      Check.ok(me6.dominate[1], "colide_push_dominates");
      Check.eq(oth6.scx[0], 200.0F, "colide_push_other_scx_200");
      Check.eq(me6.scx[0], 100.0F, "colide_push_this_scx_unchanged_100");
      // the same pair loop pushes the other three zero slots too
      Check.eq(oth6.scx[3], 200.0F, "colide_push_other_scx3_200");
      Check.eq(me6.scx[3], 0.0F, "colide_push_this_scx3_0");
      // 200 * moment[0]==1.2 == 240 reaches the other's Record via regx (:480)
      Check.eq(r6o.nrx[1][0], 1, "colide_push_other_regx_records");
      Check.eq(r6o.magx[1][0][0], 240, "colide_push_other_regx_magx_240");
      // ContO.scx is untouched: only the Madness impulse array carries it
      Check.eq(cb6.scx[0], 0, "colide_push_conto_scx_untouched");
   }

   /**
    * A drive/reg*-ready ContO: the empty .rad stream still allocates
    * p=new Plane[120] (ContO.java:210) and leaves npl==0 plus every field
    * drive/reg* read at 0; the writes below pin that fixture explicitly.
    */
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
      }
      return c;
   }

   /** One 3-point plane; wz==0 is the random-scatter path, wz!=0 skips it. */
   private static Plane plane(Medium m, int wz) {
      return new Plane(m, new Trackers(), new int[]{0, 10, 0}, new int[]{0, 0, 10}, new int[]{0, 0, 0}, 3,
            new int[]{1, 2, 3}, false, 0, 0, 0, wz, 7, 0, false);
   }

   /** One all-default drive tick; nofocus is re-armed because :2219 clears it. */
   private static Madness tick(Medium m, ContO c, Control ctrl, CheckPoints cp, int im) {
      Madness md = new Madness(m, new Record(m), null, im);
      md.cn = 0;
      md.nofocus = true;
      md.drive(ctrl, c, new Trackers(), cp);
      return md;
   }

   /** Runs one tick with a pre-set wxz and returns the resulting wxz. */
   private static int center(Medium m, int wxz, float speed) {
      ContO c = car(m);
      c.wxz = wxz;
      Madness md = new Madness(m, new Record(m), null, 1);
      md.cn = 0;
      md.speed = speed;
      md.nofocus = true;
      md.drive(new Control(m), c, new Trackers(), new CheckPoints());
      return c.wxz;
   }
}
