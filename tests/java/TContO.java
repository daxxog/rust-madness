import java.io.ByteArrayOutputStream;
import java.io.FileInputStream;

/**
 * Pins ContO's LINE-BASED .rad grammar (the byte[] constructor, ContO.java:207-419),
 * its copy constructor (:421-508), the pure helpers (:199,:645,:649,:683,:702,:1213)
 * and the dust() spawn (:660-681). No Graphics is needed here; the drawing paths
 * (d/pdust/fixit/electrify/lowshadow) live in TContODraw.
 *
 * Fixtures are the byte stream itself - the original's parser is the thing under
 * test - plus one real read-only asset (reference/external/catb0t/OBJ/mycars/
 * "Simple Car.rad"). `new ContO(new byte[0], m, t)` is the empty seam also used by
 * TRecord/TMadnessWorld: it allocates p=new Plane[120] (:210) and leaves every
 * field at its initializer (:10-56).
 *
 * The ctor's catch prints `"At File: " + var1` (:413) - a byte[] identity hash -
 * so the abort pins assert "returns normally / keeps the partial state", never
 * the printed text.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B6_conto.md, re-derived line by line from
 * reference/vineflower/ContO.java and probe-measured on the gate's Oracle 8u40.
 * Pins 30/31/36/40/42/43/60/61 needed corrections; see the B6 doc.
 */
public class TContO {
   static void run() throws Exception {
      Check.group("ContO");
      byteDefaults();
      nullBytes();
      onePlane();
      partialPlanes();
      colorGlassRoad();
      divAndGr();
      abortInsidePlane();
      globals();
      dislineOrdering();
      tracks();
      wheels();
      wheelOverflow();
      planeOverflow();
      trackersUntouched();
      realCar();
      copyCtor();
      copySnapBlend();
      copyRotation();
      copyTrackers();
      copyNoCatch();
      helpers();
      dustSpawn();
   }

   /** ContO.java:10-56: every field initializer, plus the four fixed-shape
    *  arrays. `new ContO(new byte[0], ...)` never enters the loop body (:230). */
   private static void byteDefaults() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      ContO c = new ContO(new byte[0], m, t);
      Check.eq(c.m, m, "default_m_identity");
      Check.eq(c.t, t, "default_t_identity");
      Check.eq(c.p.length, 120, "default_p_len_120");
      Check.eq(c.npl, 0, "default_npl_0");
      Check.eq(c.x, 0, "default_x_0");
      Check.eq(c.y, 0, "default_y_0");
      Check.eq(c.z, 0, "default_z_0");
      Check.eq(c.xz, 0, "default_xz_0");
      Check.eq(c.xy, 0, "default_xy_0");
      Check.eq(c.zy, 0, "default_zy_0");
      Check.eq(c.wxz, 0, "default_wxz_0");
      Check.eq(c.wzy, 0, "default_wzy_0");
      Check.eq(c.dist, 0, "default_dist_0");
      Check.eq(c.maxR, 0, "default_maxR_0");
      Check.eq(c.disp, 0, "default_disp_0");
      Check.eq(c.disline, 7, "default_disline_7");
      Check.ok(!c.shadow, "default_shadow_false");
      Check.ok(!c.noline, "default_noline_false");
      Check.eq(bits(c.grounded), bits(1.0F), "default_grounded_1");
      Check.eq(c.grat, 0, "default_grat_0");
      Check.eq(c.keyx.length, 4, "default_keyx_len_4");
      Check.eq(c.keyz.length, 4, "default_keyz_len_4");
      for (int i = 0; i < 4; i++) {
         Check.eq(c.keyx[i], 0, "default_keyx" + i + "_0");
         Check.eq(c.keyz[i], 0, "default_keyz" + i + "_0");
         Check.eq(c.sx[i], 0, "default_sx" + i + "_0");
         Check.eq(c.sy[i], 0, "default_sy" + i + "_0");
         Check.eq(c.sz[i], 0, "default_sz" + i + "_0");
         Check.eq(c.stg[i], 0, "default_stg" + i + "_0");
         Check.eq(c.dov[i], 0, "default_dov" + i + "_0");
         Check.eq(bits(c.smag[i]), bits(0.0F), "default_smag" + i + "_0");
         Check.eq(c.scx[i], 0, "default_scx" + i + "_0");
         Check.eq(c.scz[i], 0, "default_scz" + i + "_0");
         Check.ok(!c.fulls[i], "default_fulls" + i + "_false");
      }
      // tracker arrays stay null until a tracks(N) line (:29-39,:292-305)
      Check.eq(c.txy, null, "default_txy_null");
      Check.eq(c.tzy, null, "default_tzy_null");
      Check.eq(c.tc, null, "default_tc_null");
      Check.eq(c.tradx, null, "default_tradx_null");
      Check.eq(c.tradz, null, "default_tradz_null");
      Check.eq(c.trady, null, "default_trady_null");
      Check.eq(c.tx, null, "default_tx_null");
      Check.eq(c.ty, null, "default_ty_null");
      Check.eq(c.tz, null, "default_tz_null");
      Check.eq(c.skd, null, "default_skd_null");
      Check.eq(c.dam, null, "default_dam_null");
      Check.eq(c.tnt, 0, "default_tnt_0");
      Check.ok(!c.elec, "default_elec_false");
      Check.ok(!c.roted, "default_roted_false");
      Check.eq(c.edl.length, 2, "default_edl_len_2");
      Check.eq(c.edr.length, 2, "default_edr_len_2");
      Check.eq(c.elc.length, 2, "default_elc_len_2");
      Check.eq(c.edl[1] + c.edr[1] + c.elc[1], 0, "default_arc_arrays_zero");
      Check.ok(!c.fix, "default_fix_false");
      Check.eq(c.fcnt, 0, "default_fcnt_0");
   }

   /** B6(2): `new ByteArrayInputStream(null)` throws inside the try (:227), the
    *  catch at :411 swallows it, :418 still sets grat. Never assert the print. */
   private static void nullBytes() {
      Medium m = new Medium();
      ContO c = null;
      try {
         c = new ContO(null, m, new Trackers());
      } catch (Throwable x) {
         c = null;
      }
      Check.ok(c != null, "null_bytes_no_throw");
      if (c != null) {
         Check.eq(c.p.length, 120, "null_bytes_p_len_120");
         Check.eq(c.npl, 0, "null_bytes_npl_0");
         Check.eq(c.grat, 0, "null_bytes_grat_0");
      }
   }

   /** B6(3): one plane maps file x/y/z to ox/oy/oz (:255-257,:268); maxR is the
    *  max (int)sqrt over the SCALED points (:258-261) - here (10,20,60). */
   private static void onePlane() {
      Medium m = new Medium();
      ContO c = new ContO(ascii("<p>\np(10,20,30)\np(40,20,30)\np(10,20,60)\n</p>\n"), m, new Trackers());
      Check.eq(c.npl, 1, "plane_npl_1");
      Check.eq(c.p[0].n, 3, "plane_n_3");
      Check.eq(c.p[0].ox[0], 10, "plane_ox0_10");
      Check.eq(c.p[0].ox[1], 40, "plane_ox1_40");
      Check.eq(c.p[0].ox[2], 10, "plane_ox2_10");
      Check.eq(c.p[0].oy[0], 20, "plane_oy0_20");
      Check.eq(c.p[0].oy[1], 20, "plane_oy1_20");
      Check.eq(c.p[0].oy[2], 20, "plane_oy2_20");
      Check.eq(c.p[0].oz[0], 30, "plane_oz0_30");
      Check.eq(c.p[0].oz[1], 30, "plane_oz1_30");
      Check.eq(c.p[0].oz[2], 60, "plane_oz2_60");
      Check.eq(c.maxR, 64, "plane_maxR_64");
      Check.eq(c.p[0].bfase, 0, "plane_bfase_0");
      Check.eq(c.p[0].wx + c.p[0].wy + c.p[0].wz, 0, "plane_w_0");
      Check.eq(c.p[0].disline, 7, "plane_disline_7");
      Check.ok(!c.p[0].road, "plane_road_default_false");
      Check.ok(!c.p[0].glass, "plane_glass_default_false");
      Check.eq(c.p[0].gr, 0, "plane_gr_default_0");
      // |dx|=30 |dy|=0 |dz|=30 -> typ 2 (Plane.java:111-124)
      Check.eq(c.p[0].typ, 2, "plane_typ_2");
      // oc all zero -> nocol (Plane.java:132-134) and c left at 0
      Check.ok(c.p[0].nocol, "plane_nocol_true");
      Check.eq(c.p[0].c[2], 0, "plane_c_default_0");
      Check.eq(c.p[1], null, "plane_slot1_null");
      Check.eq(c.grat, 0, "plane_grat_0");
   }

   /** B6(4,5): fewer than 3 p() lines -> the Plane ctor reads ox[2]
    *  (Plane.java:111) and throws AIOOBE, swallowed at :411. `new Plane` is
    *  evaluated before npl++ (:268-269), so npl stays 0 while maxR already ate
    *  the partial points (:258-261). Everything after the bad block is lost. */
   private static void partialPlanes() {
      Medium m = new Medium();
      ContO two = new ContO(ascii("<p>\np(10,20,30)\np(40,20,30)\n</p>\n"), m, new Trackers());
      Check.eq(two.npl, 0, "two_point_npl_0");
      Check.eq(two.maxR, 53, "two_point_maxR_partial_53");
      Check.eq(two.p[0], null, "two_point_p0_null");

      ContO none = new ContO(ascii("<p>\n</p>\n"), m, new Trackers());
      Check.eq(none.npl, 0, "empty_plane_npl_0");
      Check.eq(none.maxR, 0, "empty_plane_maxR_0");

      // the abort kills the rest of the stream: the good third plane never lands
      ContO after = new ContO(ascii("<p>\np(0,0,0)\np(1,0,0)\n</p>\n"
            + "<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"), m, new Trackers());
      Check.eq(after.npl, 0, "abort_kills_following_planes");
      Check.eq(after.maxR, 1, "abort_maxR_from_partial_points");
   }

   /** B6(6,7,8): the colour triple (:243-248) and the glass flag (:250-252) are
    *  parser-scope variables, so both persist across planes; a c() line is what
    *  clears glass. road/notroad (:393-399) also persist. */
   private static void colorGlassRoad() {
      Medium m = new Medium();
      ContO col = new ContO(ascii("<p>\nc(1,2,3)\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n"
            + "<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"), m, new Trackers());
      Check.eq(col.npl, 2, "color_npl_2");
      Check.eq(col.p[1].oc[0], 1, "color_persists_plane2_oc0");
      Check.eq(col.p[1].oc[1], 2, "color_persists_plane2_oc1");
      Check.eq(col.p[1].oc[2], 3, "color_persists_plane2_oc2");

      ContO glass = new ContO(ascii("<p>\nglass()\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n"
            + "<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"), m, new Trackers());
      Check.ok(glass.p[0].glass, "glass_plane1_true");
      Check.ok(glass.p[1].glass, "glass_sticky_plane2_true");

      ContO glass2 = new ContO(ascii("<p>\nglass()\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n"
            + "<p>\nc(9,9,9)\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"), m, new Trackers());
      Check.ok(glass2.p[0].glass, "glass_plane1_before_c_true");
      Check.ok(!glass2.p[1].glass, "glass_cleared_by_c_line");
      Check.eq(glass2.p[1].oc[0], 9, "glass_plane2_oc_9");

      ContO road = new ContO(ascii("road\n<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n"
            + "notroad\n<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"
            + "<p>\np(0,9,0)\np(1,9,0)\np(2,9,0)\n</p>\n"), m, new Trackers());
      Check.ok(road.p[0].road, "road_first_plane_true");
      Check.ok(!road.p[1].road, "road_notroad_second_false");
      Check.ok(!road.p[2].road, "road_notroad_sticky_third_false");
   }

   /** B6(9,10): div(n) rescales every later coordinate AND key/maxR with
    *  n/10f (:405-407,:255-257), while gr is per-plane state that <p> resets
    *  to 0 at :235 (so gr only survives inside the block that set it). */
   private static void divAndGr() {
      Medium m = new Medium();
      ContO c = new ContO(ascii("div(5)\n<p>\np(10,20,30)\np(40,20,30)\np(10,20,60)\n</p>\n"), m,
            new Trackers());
      Check.eq(c.p[0].ox[0], 5, "div_ox0_5");
      Check.eq(c.p[0].ox[1], 20, "div_ox1_20");
      Check.eq(c.p[0].oy[0], 10, "div_oy0_10");
      Check.eq(c.p[0].oz[0], 15, "div_oz0_15");
      Check.eq(c.p[0].oz[2], 30, "div_oz2_30");
      // (int)sqrt(5^2+10^2+30^2)=32.0 -> 32 over the SCALED ints
      Check.eq(c.maxR, 32, "div_maxR_32");

      ContO g = new ContO(ascii("<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n"
            + "<p>\ngr(3)\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"
            + "<p>\np(0,9,0)\np(1,9,0)\np(2,9,0)\n</p>\n"), m, new Trackers());
      Check.eq(g.p[0].gr, 0, "gr_default_0");
      Check.eq(g.p[1].gr, 3, "gr_set_3");
      Check.eq(g.p[2].gr, 0, "gr_reset_per_plane_0");
   }

   /** B6(11): "grounded(50)" inside an open <p> matches the gr prefix test at
    *  :239 -> getvalue("gr", "grounded(50)", 0) -> NumberFormatException
    *  (measured on :699) -> the whole parse aborts at :411. grounded is never
    *  assigned, so it keeps 1.0F, while the already-built state survives. */
   private static void abortInsidePlane() {
      ContO c = new ContO(ascii("<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\n"
            + "<p>\ngrounded(50)\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"), new Medium(), new Trackers());
      Check.eq(c.npl, 1, "grprefix_npl_1_built_state_survives");
      Check.eq(bits(c.grounded), bits(1.0F), "grprefix_grounded_never_assigned");
      Check.eq(c.maxR, 2, "grprefix_maxR_from_plane1");
      // the two p() lines after it never parse
      Check.eq(c.p[1], null, "grprefix_second_plane_absent");
      Check.eq(c.disp, 0, "grprefix_disp_0");
   }

   /** B6(12): the global single-shot lines (:377-403). */
   private static void globals() {
      ContO c = new ContO(ascii("disp(4)\ndisline(3)\nshadow()\nstonecold()\ngrounded(50)\n"),
            new Medium(), new Trackers());
      Check.eq(c.disp, 4, "global_disp_4");
      Check.eq(c.disline, 3, "global_disline_3");
      Check.ok(c.shadow, "global_shadow_true");
      Check.ok(c.noline, "global_stonecold_noline_true");
      Check.eq(bits(c.grounded), bits(0.5F), "global_grounded_50_half");
   }

   /** B6(13): the Plane gets this.disline at construction (:268), so earlier
    *  planes keep 7 after a later disline(9). */
   private static void dislineOrdering() {
      ContO c = new ContO(ascii("<p>\np(0,0,0)\np(1,0,0)\np(2,0,0)\n</p>\ndisline(9)\n"
            + "<p>\np(0,5,0)\np(1,5,0)\np(2,5,0)\n</p>\n"), new Medium(), new Trackers());
      Check.eq(c.p[0].disline, 7, "disline_first_plane_keeps_7");
      Check.eq(c.p[1].disline, 9, "disline_second_plane_9");
      Check.eq(c.disline, 9, "disline_conto_9");
   }

   /** B6(14,15,16): tracks(N) allocates twelve arrays (:292-305); <track>
    *  zero-fills slot tnt with dam=1/skd=0 (:307-322) and </track> bumps tnt
    *  (:372-375); a dam line overrides to 3 (:367-369). No tracks(N): the null
    *  this.dam NPEs (:309) - swallowed. Too many blocks: AIOOBE at dam[tnt]
    *  (:309) - swallowed with tnt keeping the last good value. */
   private static void tracks() {
      Medium m = new Medium();
      ContO c = new ContO(ascii("tracks(2)\n<track>\n</track>\n<track>\ndam\n</track>\n"), m,
            new Trackers());
      Check.eq(c.tnt, 2, "tracks_tnt_2");
      Check.eq(c.dam.length, 2, "tracks_dam_len_2");
      Check.eq(c.tc.length, 2, "tracks_tc_len_2");
      Check.eq(c.dam[0], 1, "tracks_dam_default_1");
      Check.eq(c.skd[0], 0, "tracks_skd_default_0");
      Check.eq(c.tx[0] + c.ty[0] + c.tz[0], 0, "tracks_xyz_default_0");
      Check.eq(c.txy[0] + c.tzy[0] + c.tradx[0] + c.trady[0] + c.tradz[0], 0, "tracks_angles_default_0");
      Check.eq(c.tc[0][0] + c.tc[0][1] + c.tc[0][2], 0, "tracks_color_default_0");
      Check.eq(c.dam[1], 3, "tracks_dam_line_sets_3");

      ContO f = new ContO(ascii("tracks(1)\n<track>\nc(1,2,3)\nxy(10)\nzy(20)\nradx(30)\nrady(40)\n"
            + "radz(50)\nty(60)\ntx(70)\ntz(80)\nskid(9)\n</track>\n"), m, new Trackers());
      Check.eq(f.tnt, 1, "trackrow_tnt_1");
      Check.eq(f.txy[0], 10, "trackrow_xy_10");
      Check.eq(f.tzy[0], 20, "trackrow_zy_20");
      Check.eq(f.tradx[0], 30, "trackrow_radx_30");
      Check.eq(f.trady[0], 40, "trackrow_rady_40");
      Check.eq(f.tradz[0], 50, "trackrow_radz_50");
      Check.eq(f.ty[0], 60, "trackrow_ty_60");
      Check.eq(f.tx[0], 70, "trackrow_tx_70");
      Check.eq(f.tz[0], 80, "trackrow_tz_80");
      Check.eq(f.skd[0], 9, "trackrow_skid_9");
      Check.eq(f.dam[0], 1, "trackrow_dam_default_1");
      Check.eq(f.tc[0][0], 1, "trackrow_c0_1");
      Check.eq(f.tc[0][2], 3, "trackrow_c2_3");

      ContO orphan = new ContO(ascii("<track>\n</track>\n"), m, new Trackers());
      Check.eq(orphan.tnt, 0, "track_without_tracks_tnt_0");
      Check.eq(orphan.dam, null, "track_without_tracks_dam_null");

      ContO over = new ContO(ascii("tracks(1)\n<track>\n</track>\n<track>\n</track>\n"), m,
            new Trackers());
      Check.eq(over.tnt, 1, "tracks_overflow_keeps_last_good_1");
      Check.eq(over.dam[0], 1, "tracks_overflow_slot0_dam_1");
   }

   /** B6(17,18): w(x,y,z,a,mr,mz) stores keyx/keyz (:274-275, y ignored),
    *  appends Wheels.make's NINE planes (:277-289) and leaves grat =
    *  Wheels.ground (:418, Wheels.java:118-120). The 5th w line writes
    *  keyx[4] and throws AIOOBE at :274 - swallowed. Wheels points never feed
    *  maxR (:258 is only reached from the p() branch). */
   private static void wheels() {
      Medium m = new Medium();
      ContO c = new ContO(ascii("w(10,20,30,1,4,4)\n"), m, new Trackers());
      Check.eq(c.keyx[0], 10, "wheel_keyx0_10");
      // keyz reads argument 2, so the y=20 argument is not stored on ContO
      Check.eq(c.keyz[0], 30, "wheel_keyz0_30");
      Check.eq(c.npl, 9, "wheel_npl_9");
      Check.eq(c.maxR, 0, "wheel_maxR_0");
      Check.eq(c.grat, 25, "wheel_grat_25");
      Check.eq(c.tnt, 0, "wheel_tnt_0");
      Check.eq(c.p[0].master, 1, "wheel_p0_master_1");
      Check.eq(c.p[1].master, 0, "wheel_p1_master_0");
      Check.eq(c.p[8].master, 0, "wheel_p8_master_0");
      Check.eq(c.p[0].n, 8, "wheel_p0_n_8");
      Check.eq(c.p[0].ox[0], 8, "wheel_p0_ox0_8");
      Check.eq(c.p[0].oy[0], 15, "wheel_p0_oy0_15");
      Check.eq(c.p[0].oz[0], 32, "wheel_p0_oz0_32");
      Check.eq(c.p[0].gr, 0, "wheel_p0_gr_0");
      Check.eq(c.keyx[1], 0, "wheel_keyx1_untouched");

      // div() scales x/y/z/mr/mz but NOT a (getvalue raw at :285)
      ContO d = new ContO(ascii("div(5)\nw(0,10,0,11,10,10)\n"), m, new Trackers());
      Check.eq(d.keyx[0], 0, "wheeldiv_keyx0_0");
      Check.eq(d.keyz[0], 0, "wheeldiv_keyz0_0");
      Check.eq(d.grat, 12, "wheeldiv_grat_12");
      Check.eq(d.p[0].wx, 2, "wheeldiv_wx_2_needs_unscaled_a");
      Check.eq(d.p[0].ox[0], -2, "wheeldiv_ox0_minus2");
      Check.eq(d.p[0].oy[0], -1, "wheeldiv_oy0_minus1");
      Check.eq(d.p[0].oz[0], 2, "wheeldiv_oz0_2");
      Check.eq(d.p[1].gr, 1, "wheeldiv_p1_gr_1");
   }

   /** B6(18): the 5th w() line throws at keyx[4] (:274) with four wheels
    *  already appended, so npl==36 and grat survives from Wheels.ground. */
   private static void wheelOverflow() {
      ContO c = new ContO(ascii("w(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\n"
            + "w(1,2,3,1,4,4)\nw(1,2,3,1,4,4)\n"), new Medium(), new Trackers());
      Check.eq(c.keyx[0], 1, "wheel5_keyx0_1");
      Check.eq(c.keyx[3], 1, "wheel5_keyx3_1");
      Check.eq(c.npl, 36, "wheel5_npl_36");
      // (int)(2 + 12*(4/10f) + 1) == 7 (Wheels.java:118-119), set at :418
      Check.eq(c.grat, 7, "wheel5_grat_7_from_partial_wheels");
   }

   /** B6(19): plane 121 indexes p[120] at :268 -> AIOOBE, swallowed; the first
    *  120 planes are intact. */
   private static void planeOverflow() {
      StringBuilder sb = new StringBuilder();
      for (int i = 0; i < 121; i++) {
         sb.append("<p>\np(0,").append(i).append(",0)\np(1,").append(i).append(",0)\np(2,").append(i)
               .append(",0)\n</p>\n");
      }
      ContO c = new ContO(ascii(sb.toString()), new Medium(), new Trackers());
      Check.eq(c.npl, 120, "planes121_npl_120");
      Check.eq(c.p.length, 120, "planes121_p_len_120");
      Check.eq(c.p[119].oy[0], 119, "planes121_last_plane_kept");
   }

   /** B6(20): :207-419 never writes through the Trackers argument (only stores
    *  it at :209), so a car full of <track> blocks leaves t.nt at 0. */
   private static void trackersUntouched() {
      Trackers t = new Trackers();
      ContO c = new ContO(ascii("tracks(1)\n<track>\nc(1,2,3)\n</track>\n"), new Medium(), t);
      Check.eq(c.tnt, 1, "trackers_untouched_conto_tnt_1");
      Check.eq(t.nt, 0, "trackers_untouched_nt_0");
      Check.eq(t.x[0], 0, "trackers_untouched_x_0");
      Check.eq(t.c[0][0], 0, "trackers_untouched_c_0");
   }

   /** B6(21): the real read-only car fixture. 26 </p> + 4 wheels x 9 planes =
    *  62 (:268,:289); maxR 90 comes from (40,11,80) (:258-261); grat 57 from
    *  Wheels.ground with y=20, mz=30 -> (int)(20+12*3+1) (:418,
    *  Wheels.java:118-120). */
   private static void realCar() throws Exception {
      byte[] rad = slurp("reference/external/catb0t/OBJ/mycars/Simple Car.rad");
      Trackers t = new Trackers();
      ContO c = new ContO(rad, new Medium(), t);
      Check.eq(c.npl, 62, "real_npl_62");
      Check.eq(c.maxR, 90, "real_maxR_90");
      Check.eq(c.grat, 57, "real_grat_57");
      Check.eq(c.keyx[0], -50, "real_keyx0_minus50");
      Check.eq(c.keyx[1], 50, "real_keyx1_50");
      Check.eq(c.keyx[2], -50, "real_keyx2_minus50");
      Check.eq(c.keyx[3], 50, "real_keyx3_50");
      Check.eq(c.keyz[0], 60, "real_keyz0_60");
      Check.eq(c.keyz[1], 60, "real_keyz1_60");
      Check.eq(c.keyz[2], -55, "real_keyz2_minus55");
      Check.eq(c.keyz[3], -55, "real_keyz3_minus55");
      Check.eq(c.tnt, 0, "real_tnt_0");
      Check.eq(t.nt, 0, "real_trackers_nt_0");
      Check.eq(c.p[0].n, 4, "real_p0_n_4");
      Check.eq(c.p[0].ox[0], 40, "real_p0_ox_40");
      Check.eq(c.p[0].oy[0], -10, "real_p0_oy_minus10");
      Check.eq(c.p[0].oz[0], 70, "real_p0_oz_70");
      Check.eq(c.p[0].oc[0], 78, "real_p0_oc0_78");
      Check.eq(c.p[0].oc[2], 238, "real_p0_oc2_238");
      Check.ok(c.p[5].glass, "real_p5_is_glass_plane");
   }

   /** B6(22): placement args win, xz/xy/zy are forced to 0, every scalar is
    *  copied (:424-431), p is reallocated to npl (:432), and the fields the
    *  ctor never touches (tnt, dist, wxz/wzy, elec/roted/fix/fcnt) stay at their
    *  initializers. */
   private static void copyCtor() {
      Medium m = new Medium();
      ContO src = new ContO(ascii("<p>\nc(100,50,25)\np(10,20,30)\np(40,20,30)\np(10,20,60)\n</p>\n"), m,
            new Trackers());
      src.xz = 45;
      src.xy = 30;
      src.zy = 60;
      src.wxz = 9;
      src.wzy = 8;
      src.dist = 7;
      src.disp = 4;
      src.disline = 5;
      src.noline = true;
      src.shadow = true;
      src.grounded = 0.5F;
      src.grat = 42;
      src.elec = true;
      src.roted = true;
      src.fix = true;
      src.fcnt = 6;
      src.keyx[0] = 11;
      src.keyx[3] = 13;
      src.keyz[1] = 21;
      src.stg[2] = 3;
      src.p[0].master = 7;
      src.p[0].bfase = 33;
      src.p[0].c[0] = 111;
      ContO c = new ContO(src, 100, 200, 300, 0);
      Check.eq(c.m, m, "copy_m_identity");
      Check.ok(c.t == src.t, "copy_t_shared");
      Check.eq(c.x, 100, "copy_x_100");
      Check.eq(c.y, 200, "copy_y_200");
      Check.eq(c.z, 300, "copy_z_300");
      Check.eq(c.xz + c.xy + c.zy, 0, "copy_attitude_zeroed");
      Check.eq(c.npl, 1, "copy_npl_1");
      Check.eq(c.maxR, 64, "copy_maxR_64");
      Check.eq(c.disp, 4, "copy_disp_4");
      Check.eq(c.disline, 5, "copy_disline_5");
      Check.ok(c.noline, "copy_noline_true");
      Check.ok(c.shadow, "copy_shadow_true");
      Check.eq(bits(c.grounded), bits(0.5F), "copy_grounded_half");
      Check.eq(c.grat, 42, "copy_grat_42");
      Check.eq(c.p.length, 1, "copy_p_len_npl");
      Check.eq(c.p[0].n, 3, "copy_p0_n_3");
      Check.eq(c.p[0].ox[0], 10, "copy_p0_ox0_10");
      Check.eq(c.p[0].oz[2], 60, "copy_p0_oz2_60");
      // rebuilt from oc, NOT from the damaged src.p[0].c
      Check.eq(c.p[0].oc[0], 100, "copy_p0_oc0_100");
      Check.eq(c.p[0].c[0], 100, "copy_p0_c_from_oc_not_src_c");
      Check.eq(c.p[0].bfase, 33, "copy_p0_bfase_33");
      Check.eq(c.p[0].master, 7, "copy_p0_master_7");
      Check.eq(c.keyx[0], 11, "copy_keyx0_11");
      Check.eq(c.keyx[3], 13, "copy_keyx3_13");
      Check.eq(c.keyz[1], 21, "copy_keyz1_21");
      Check.eq(c.stg[2], 0, "copy_stg_zeroed");
      Check.eq(c.tnt, 0, "copy_tnt_0");
      Check.eq(c.dist, 0, "copy_dist_0");
      Check.eq(c.wxz + c.wzy, 0, "copy_wxz_wzy_0");
      Check.ok(!c.elec, "copy_elec_false");
      Check.ok(!c.roted, "copy_roted_false");
      Check.ok(!c.fix, "copy_fix_false");
      Check.eq(c.fcnt, 0, "copy_fcnt_0");
   }

   /** B6(23): Plane recomputes c from oc with the CURRENT m.snap
    *  (Plane.java:136-149), so setting snap AFTER the src was built changes the
    *  copy (and only the copy). */
   private static void copySnapBlend() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      ContO src = new ContO(ascii("<p>\nc(100,50,25)\np(0,0,0)\np(10,0,0)\np(0,0,10)\n</p>\n"), m, t);
      Check.eq(src.p[0].c[0], 100, "snap_src_c_before_0");
      m.snap[0] = 10;
      m.snap[1] = 10;
      m.snap[2] = 10;
      ContO c = new ContO(src, 0, 0, 0, 0);
      Check.eq(src.p[0].c[1], 50, "snap_src_plane_not_retinted");
      Check.eq(c.p[0].c[0], 110, "snap_copy_c0_110");
      Check.eq(c.p[0].c[1], 55, "snap_copy_c1_55");
      Check.eq(c.p[0].c[2], 27, "snap_copy_c2_27");
      Check.eq(c.p[0].oc[1], 50, "snap_copy_oc1_50");
   }

   /** B6(24): the copy rotates every plane about (0,0) by arg5 before
    *  loadprojf (:461-465, Plane.java:1016-1025). 0 is a no-op; 90 truncates
    *  cos(90)=6.123234E-17 to 0, so (100,0)->(0,100) and (100,50)->(-50,100). */
   private static void copyRotation() {
      Medium m = new Medium();
      ContO src = new ContO(ascii("<p>\np(100,0,0)\np(100,10,0)\np(100,0,50)\n</p>\n"), m,
            new Trackers());
      ContO r0 = new ContO(src, 0, 0, 0, 0);
      Check.eq(r0.p[0].ox[0], 100, "rot0_ox0_100");
      Check.eq(r0.p[0].oz[0], 0, "rot0_oz0_0");
      Check.eq(r0.p[0].ox[2], 100, "rot0_ox2_100");
      Check.eq(r0.p[0].oz[2], 50, "rot0_oz2_50");

      ContO r90 = new ContO(src, 0, 0, 0, 90);
      Check.eq(r90.p[0].ox[0], 0, "rot90_p0_ox0_0");
      Check.eq(r90.p[0].oz[0], 100, "rot90_p0_oz0_100");
      Check.eq(r90.p[0].ox[2], -50, "rot90_p2_ox_minus50");
      Check.eq(r90.p[0].oz[2], 100, "rot90_p2_oz_100");
      // the source plane is untouched by the rotation
      Check.eq(src.p[0].ox[0], 100, "rot90_src_untouched_100");
   }

   /** B6(25,26): the tracker rows are appended to the SHARED Trackers at
    *  t.nt (:467-499); nothing guards duplication, so a second copy appends
    *  two more rows. Rows rotate xy/zy, place x/z/y with the copy's x/y/z, copy
    *  skd/dam, snap-blend and clamp tc (0..255) and rotate radx/radz with
    *  |a|==180 collapsing to 0 (:489-495). */
   private static void copyTrackers() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      ContO src = new ContO(ascii("tracks(2)\n<track>\nc(10,20,30)\nxy(10)\nzy(20)\nradx(30)\nrady(40)\n"
            + "radz(50)\nty(60)\ntx(70)\ntz(80)\nskid(9)\ndam\n</track>\n"
            + "<track>\nc(200,200,200)\nxy(0)\nzy(0)\nradx(1)\nrady(2)\nradz(3)\nty(4)\ntx(5)\ntz(6)\n</track>\n"),
            m, t);
      Check.eq(src.tnt, 2, "copytrack_src_tnt_2");
      Check.eq(t.nt, 0, "copytrack_byte_ctor_wrote_nothing");

      ContO c = new ContO(src, 100, 200, 300, 0);
      Check.eq(c.tnt, 0, "copytrack_dst_tnt_stays_0");
      Check.eq(t.nt, 2, "copytrack_append_2");
      Check.eq(t.xy[0], 10, "copytrack_row0_xy_10");
      Check.eq(t.zy[0], 20, "copytrack_row0_zy_20");
      Check.eq(t.c[0][0], 10, "copytrack_row0_c0_10");
      Check.eq(t.x[0], 170, "copytrack_row0_x_170");
      Check.eq(t.y[0], 260, "copytrack_row0_y_260");
      Check.eq(t.z[0], 380, "copytrack_row0_z_380");
      Check.eq(t.skd[0], 9, "copytrack_row0_skd_9");
      Check.eq(t.dam[0], 3, "copytrack_row0_dam_3");
      Check.eq(t.radx[0], 30, "copytrack_row0_radx_30");
      Check.eq(t.radz[0], 50, "copytrack_row0_radz_50");
      Check.eq(t.rady[0], 40, "copytrack_row0_rady_40");
      Check.eq(t.x[1], 105, "copytrack_row1_x_105");
      Check.eq(t.z[1], 306, "copytrack_row1_z_306");
      Check.eq(t.y[1], 204, "copytrack_row1_y_204");
      Check.eq(t.radx[1], 1, "copytrack_row1_radx_1");

      // second copy of the same source: two more rows, no guard
      ContO c2 = new ContO(src, 100, 200, 300, 0);
      Check.eq(t.nt, 4, "copytrack_second_copy_nt_4");
      Check.eq(t.xy[2], 10, "copytrack_row2_xy_10");
      Check.eq(t.xy[3], 0, "copytrack_row3_xy_0");

      // 90 degrees: xy=(int)(10*cos90-20*sin90)=-20, zy=10;
      // x=(int)(100+70*cos90-80*sin90)=20, z=370; radx=50, radz=30
      Medium m90 = new Medium();
      Trackers t90 = new Trackers();
      ContO s90 = new ContO(ascii("tracks(1)\n<track>\nc(10,10,10)\nxy(10)\nzy(20)\nradx(30)\nradz(50)\n"
            + "ty(60)\ntx(70)\ntz(80)\n</track>\n"), m90, t90);
      new ContO(s90, 100, 200, 300, 90);
      Check.eq(t90.xy[0], -20, "copytrack90_xy_minus20");
      Check.eq(t90.zy[0], 10, "copytrack90_zy_10");
      Check.eq(t90.x[0], 20, "copytrack90_x_20");
      Check.eq(t90.z[0], 370, "copytrack90_z_370");
      Check.eq(t90.radx[0], 50, "copytrack90_radx_50");
      Check.eq(t90.radz[0], 30, "copytrack90_radz_30");
      Check.eq(t90.rady[0], 0, "copytrack90_rady_0");

      // 180 degrees: radii use |a|==180 -> 0 (:489-492), so they do NOT flip
      Medium m180 = new Medium();
      Trackers t180 = new Trackers();
      ContO s180 = new ContO(ascii("tracks(1)\n<track>\nc(10,10,10)\nxy(10)\nzy(20)\nradx(30)\nradz(50)\n"
            + "tx(70)\ntz(80)\n</track>\n"), m180, t180);
      new ContO(s180, 0, 0, 0, 180);
      Check.eq(t180.radx[0], 30, "copytrack180_radx_unrotated_30");
      Check.eq(t180.radz[0], 50, "copytrack180_radz_unrotated_50");
      Check.eq(t180.xy[0], -10, "copytrack180_xy_minus10");
      Check.eq(t180.zy[0], -20, "copytrack180_zy_minus20");

      // tc snap blend clamps both ends: (-5,300,10) at snap 100 -> (0,255,20)
      Medium mc = new Medium();
      mc.snap[0] = 100;
      mc.snap[1] = 100;
      mc.snap[2] = 100;
      Trackers tc = new Trackers();
      ContO sc = new ContO(ascii("tracks(1)\n<track>\nc(-5,300,10)\n</track>\n"), mc, tc);
      new ContO(sc, 0, 0, 0, 0);
      Check.eq(tc.c[0][0], 0, "copytrack_tc_clamp_low_0");
      Check.eq(tc.c[0][1], 255, "copytrack_tc_clamp_high_255");
      Check.eq(tc.c[0][2], 20, "copytrack_tc_snap20");
   }

   /** B6(27): unlike the byte ctor, the copy ctor has no try/catch, so a
    *  src.p==null NPEs out of :438. */
   private static void copyNoCatch() {
      final ContO src = new ContO(new byte[0], new Medium(), new Trackers());
      src.npl = 1;
      src.p = null;
      Check.throwsIt(new Check.Task() {
         public void run() {
            new ContO(src, 0, 0, 0, 0);
         }
      }, NullPointerException.class, "copy_null_p_npe");
   }

   /** B6(56-62): the pure helpers, all integer-truncating. */
   private static void helpers() {
      Medium m = new Medium();
      ContO c = new ContO(new byte[0], m, new Trackers());
      // ys/xs clamp depth to 50 first (:200-204,:703-706)
      Check.eq(c.ys(100, 50), -600, "ys_100_50_minus600");
      Check.eq(c.ys(50, 100), -400, "ys_50_100_minus400");
      Check.eq(c.ys(100, 0), -600, "ys_depth0_clamps_50");
      Check.eq(c.ys(0, 49), -1400, "ys_49_clamps_50");
      Check.eq(c.ys(0, 51), -1368, "ys_51_no_clamp");
      Check.eq(c.ys(-100, -200), -2200, "ys_negative_depth_clamps_50");
      Check.eq(c.xs(275, 0), 275, "xs_275_0_275");
      Check.eq(c.xs(100, 0), -1125, "xs_100_0_minus1125");
      Check.eq(c.xs(0, 0), -1925, "xs_0_0_minus1925");
      Check.eq(c.xs(0, 49), -1925, "xs_49_clamps_50");
      Check.eq(c.xs(0, 51), -1881, "xs_51_no_clamp");
      Check.eq(c.xs(0, 100), -825, "xs_0_100_minus825");
      Check.eq(c.xs(-100, -200), -2725, "xs_negative_depth_clamps_50");

      // getpy (:645-647): /10 then square, per axis, truncating toward zero
      Check.eq(c.getpy(25, 25, 25), 12, "getpy_25_12");
      Check.eq(c.getpy(-5, 0, 0), 0, "getpy_minus5_truncates_0");
      Check.eq(c.getpy(15, 7, 3), 1, "getpy_15_7_3_1");
      c.x = 10;
      c.y = 20;
      c.z = 30;
      Check.eq(c.getpy(25, 25, 25), 1, "getpy_offsets_1");

      // py (:1213-1215) - squared distance, no abs and no sqrt
      Check.eq(c.py(3, 1, 4, 0), 20, "py_3_1_4_0_20");
      Check.eq(c.py(-3, 3, 0, 4), 52, "py_minus3_3_0_4_52");
      Check.eq(c.py(0, 0, 0, 0), 0, "py_same_0");

      // rot (:649-658): var5==0 is a no-op; 90 truncates the 6.123234E-17
      // cosine (Medium.java:115) to 0
      int[] a = new int[]{100};
      int[] b = new int[]{0};
      c.rot(a, b, 0, 0, 90, 1);
      Check.eq(a[0], 0, "rot90_x_0");
      Check.eq(b[0], 100, "rot90_y_100");
      int[] a2 = new int[]{100};
      int[] b2 = new int[]{7};
      c.rot(a2, b2, 0, 0, 0, 1);
      Check.eq(a2[0], 100, "rot0_x_untouched");
      Check.eq(b2[0], 7, "rot0_y_untouched");
      int[] a3 = new int[]{5, 6, 7};
      int[] b3 = new int[]{8, 9, 10};
      c.rot(a3, b3, 0, 0, 90, 0);
      Check.eq(a3[0], 5, "rot_n0_untouched");

      // getvalue (:683-700)
      Check.eq(c.getvalue("p", "p(10,20,30)", 0), 10, "getvalue_p0_10");
      Check.eq(c.getvalue("p", "p(10,20,30)", 1), 20, "getvalue_p1_20");
      Check.eq(c.getvalue("p", "p(10,20,30)", 2), 30, "getvalue_p2_30");
      Check.eq(c.getvalue("p", "p(-5,-6,-7)", 1), -6, "getvalue_negative_minus6");
      // an early key end is a plain text->int failure
      Check.throwsIt(new Check.Task() {
         public void run() {
            c.getvalue("xy", "txy(5)", 0);
         }
      }, NumberFormatException.class, "getvalue_keylen_mismatch_nfe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            c.getvalue("gr", "grounded(50)", 0);
         }
      }, NumberFormatException.class, "getvalue_grounded_prefix_nfe");
      // index past the LAST value: ')' advances one past the line end ->
      // charAt throws StringIndexOutOfBoundsException (measured, NOT an NFE)
      Check.throwsIt(new Check.Task() {
         public void run() {
            c.getvalue("p", "p(10,20)", 2);
         }
      }, StringIndexOutOfBoundsException.class, "getvalue_past_end_sioobe");
      // a trailing comma leaves an empty accumulator -> Integer.valueOf("") NFE
      Check.throwsIt(new Check.Task() {
         public void run() {
            c.getvalue("p", "p(1,2,)", 2);
         }
      }, NumberFormatException.class, "getvalue_empty_accumulator_nfe");
      // the ctor trims each line (:231), so only the RAW untrimmed line fails
      Check.throwsIt(new Check.Task() {
         public void run() {
            c.getvalue("p", " p(1,2,3)", 0);
         }
      }, NumberFormatException.class, "getvalue_untrimmed_nfe");
      ContO trimmed = new ContO(ascii("<p>\n  p(1,2,3)  \np(4,2,3)\np(5,2,3)\n</p>\n"), m,
            new Trackers());
      Check.eq(trimmed.npl, 1, "trimmed_line_parses");
      Check.eq(trimmed.p[0].ox[0], 1, "trimmed_line_ox0_1");
   }

   /** B6(48,49): dust spawns only when slot stg==0, |var5|+|var6| is strictly
    *  greater than 100, and the car's side does not thin this particular
    *  particle: var9>5 suppresses slots 0/2 (:662-664), var9<-5 suppresses
    *  1/3 (:666-668). All ten fields are written with (int) truncation. */
   private static void dustSpawn() {
      Medium m = new Medium();
      ContO c = new ContO(new byte[0], m, new Trackers());
      c.dust(0, 10.5F, 20.9F, -30.5F, 60.0F, 41.0F, 2.5F, true, 0);
      Check.eq(c.sx[0], 10, "dust_sx_10");
      Check.eq(c.sy[0], 20, "dust_sy_20");
      Check.eq(c.sz[0], -30, "dust_sz_minus30");
      Check.eq(c.stg[0], 1, "dust_stg_1");
      Check.eq(c.dov[0], -1, "dust_dov_minus1");
      Check.eq(bits(c.smag[0]), bits(2.5F), "dust_smag_2_5");
      Check.eq(c.scx[0], 60, "dust_scx_60");
      Check.eq(c.scz[0], 41, "dust_scz_41");
      Check.ok(c.fulls[0], "dust_fulls_true");
      Check.eq(c.stg[1], 0, "dust_other_slot_untouched");

      // strict > 100.0F
      ContO exact = new ContO(new byte[0], m, new Trackers());
      exact.dust(0, 1.5F, 2.5F, 3.5F, 60.0F, 40.0F, 2.5F, true, 0);
      Check.eq(exact.stg[0], 0, "dust_exact100_no_spawn");
      ContO over = new ContO(new byte[0], m, new Trackers());
      over.dust(0, 1.5F, 2.5F, 3.5F, 60.0F, 40.1F, 2.5F, true, 0);
      Check.eq(over.stg[0], 1, "dust_100_1_spawns");

      // a live particle blocks re-spawn
      ContO live = new ContO(new byte[0], m, new Trackers());
      live.stg[0] = 1;
      live.dust(0, 1.5F, 2.5F, 3.5F, 60.0F, 41.0F, 2.5F, true, 0);
      Check.eq(live.stg[0], 1, "dust_live_slot_no_respawn");
      Check.eq(live.sx[0], 0, "dust_live_slot_sx_untouched");

      // side thinning: var9>5 kills 0/2, var9<-5 kills 1/3
      for (int slot = 0; slot < 4; slot++) {
         ContO up = new ContO(new byte[0], m, new Trackers());
         up.dust(slot, 1.5F, 2.5F, 3.5F, 60.0F, 41.0F, 2.5F, true, 6);
         Check.eq(up.stg[slot], slot == 0 || slot == 2 ? 0 : 1, "dust_v9_6_slot" + slot);
         ContO down = new ContO(new byte[0], m, new Trackers());
         down.dust(slot, 1.5F, 2.5F, 3.5F, 60.0F, 41.0F, 2.5F, true, -6);
         Check.eq(down.stg[slot], slot == 1 || slot == 3 ? 0 : 1, "dust_v9_minus6_slot" + slot);
      }
   }

   // ---------------------------------------------------------------- helpers

   private static byte[] ascii(String s) {
      try {
         return s.getBytes("US-ASCII");
      } catch (java.io.UnsupportedEncodingException e) {
         throw new AssertionError(e);
      }
   }

   private static byte[] slurp(String path) throws Exception {
      FileInputStream in = new FileInputStream(path);
      ByteArrayOutputStream out = new ByteArrayOutputStream();
      byte[] buf = new byte[8192];
      int n;
      while ((n = in.read(buf)) > 0) {
         out.write(buf, 0, n);
      }
      in.close();
      return out.toByteArray();
   }

   private static int bits(float f) {
      return Float.floatToIntBits(f);
   }
}
