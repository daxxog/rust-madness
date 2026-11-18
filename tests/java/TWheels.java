/**
 * Pins Wheels.make (Wheels.java:5-164), the procedural wheel generator that
 * ContO's XML loader calls nine-slots-at-a-time (ContO.java:277-289).
 *
 * STRUCTURE (source-derived): one make() call fills 9 consecutive Plane
 * slots: slot 0 is an 8-vertex hub (Wheels.java:6-44) and slots 1..8 are
 * 4-vertex rim faces (:48-162). `mast++` appears exactly ONCE, right after
 * the hub Plane (:45-46), so it counts WHEELS, not planes - only the hub
 * gets `master = mast` (:46); the eight rim planes keep master 0. B3's
 * "mast++ on each Plane creation" was wrong (see the B3 correction).
 * `ground` is a monotone max of (int)(y + 12*yScale + 1) (:118-120).
 *
 * NO SEEDING NEEDED: the constructor path Plane.java:98-223 reads only
 * m.snap / m.csky / m.cfade / m.fade and the coordinate arrays - it never
 * calls Medium.random() (contrast B5's d()-only random sites). The probes
 * below assert m.cntrn/m.trn are still 0 after make(), which is the
 * verdict, so plain fixtures are used (no TestRng.install).
 *
 * Fixture A scales are exact in float (var9=var10=10 -> 1.0F), so every
 * expected coordinate below is the source expression evaluated exactly:
 *   var16 = (float)var9/10.0F (:11), var17 = (float)var10/10.0F (:12)
 *   var11[i] = (int)((float)var5 - 4.0F*var16)              (:24-26, all 8)
 *   var12/var13 = (int)((float)var6 [var7] +/- {12,5}*var17) (:28-43,48-59)
 *   Plane(m, t, var11, var13, var12, 8|4, {75,75,75}, false, gr, var15,
 *         var6, var7, 7, 0, false)                          (:44,60,...,162)
 * (the 3rd/4th/5th Plane args are ox/oz/oy - Plane.java:81-108).
 */
public class TWheels {
   /** constants.toml [wheels] - the generator's fixed geometry. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("wheels");

   static void run() {
      Check.group("Wheels");
      K.requireKeys("slots_per_wheel", "hub_vertices", "rim_vertices", "rim_y", "ground_bias");
      defaults();
      firstMake();
      branchArms();
      repeatedMake();
   }

   /** Wheels.java:2-3 - the only two fields, both int 0. */
   private static void defaults() {
      Wheels w = new Wheels();
      Check.eq(w.ground, 0, "default_ground_0");
      Check.eq(w.mast, 0, "default_mast_0");
   }

   /** Fixture A: make(m, t, ps, 0, x=100, y=200, z=300, flag=0, s=10, s=10). */
   private static void firstMake() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      Wheels w = new Wheels();
      Plane[] ps = new Plane[16];
      w.make(m, t, ps, 0, 100, 200, 300, 0, 10, 10);

      // :118-120 - (int)(200 + 12*1.0 + 1.0)
      Check.eq(w.ground, (int) (200 + K.intv("rim_y") * 1.0F + K.intv("ground_bias")), "make_ground_213");
      // :45 - ONE increment per make() call, however many planes it built
      Check.eq(w.mast, 1, "make_mast_1");

      // :44 + eight :60..:162 writes = slots 0..8; nothing beyond
      for (int i = 0; i < K.intv("slots_per_wheel"); i++) {
         Check.ok(ps[i] != null, "make_slot" + i + "_filled");
      }
      for (int i = K.intv("slots_per_wheel"); i < 16; i++) {
         Check.eq(ps[i], null, "make_slot" + i + "_null");
      }

      // vertex counts: the hub is the only 8-vertex plane (:44 n=8), the
      // rim faces are all n=4 (:60..:162)
      Check.eq(ps[0].n, K.intv("hub_vertices"), "plane0_n_8");
      for (int i = 1; i < K.intv("slots_per_wheel"); i++) {
         Check.eq(ps[i].n, K.intv("rim_vertices"), "plane" + i + "_n_4");
      }

      // :46 master is written ONLY for the hub; the rim planes keep 0
      Check.eq(ps[0].master, 1, "plane0_master_1");
      Check.eq(ps[1].master, 0, "plane1_master_0");
      Check.eq(ps[8].master, 0, "plane8_master_0");

      // gr: hub 0 (:44), three var18 faces (:60,:74,:88), three hard -1
      // faces (:102,:116,:134), two var18 faces (:148,:162); var18==1 here
      // because |100| != 73 (:17-20)
      Check.eq(ps[0].gr, 0, "plane0_gr_0");
      Check.eq(ps[1].gr, 1, "plane1_gr_var18_1");
      Check.eq(ps[2].gr, 1, "plane2_gr_var18_1");
      Check.eq(ps[3].gr, 1, "plane3_gr_var18_1");
      Check.eq(ps[4].gr, -1, "plane4_gr_hard_minus1");
      Check.eq(ps[5].gr, -1, "plane5_gr_hard_minus1");
      Check.eq(ps[6].gr, -1, "plane6_gr_hard_minus1");
      Check.eq(ps[7].gr, 1, "plane7_gr_var18_1");
      Check.eq(ps[8].gr, 1, "plane8_gr_var18_1");

      // hub geometry (:24-26 + :28-43), all exact with var16=var17=1.0F
      int[] wantOx0 = {96, 96, 96, 96, 96, 96, 96, 96};
      int[] wantOy0 = {188, 188, 195, 205, 212, 212, 205, 195};
      int[] wantOz0 = {305, 295, 288, 288, 295, 305, 312, 312};
      for (int i = 0; i < 8; i++) {
         Check.eq(ps[0].ox[i], wantOx0[i], "plane0_ox" + i + "_96");
         Check.eq(ps[0].oy[i], wantOy0[i], "plane0_oy" + i);
         Check.eq(ps[0].oz[i], wantOz0[i], "plane0_oz" + i);
      }
      Check.eq(ps[0].ox.length, K.intv("hub_vertices"), "plane0_ox_len_8");
      Check.eq(ps[0].oy.length, K.intv("hub_vertices"), "plane0_oy_len_8");
      Check.eq(ps[0].oz.length, K.intv("hub_vertices"), "plane0_oz_len_8");

      // first rim face (:48-60): x = var5 +/- 4.0F*var16, y = var6 - 12/12,
      // z = var7 + 5/-5
      int[] wantOx1 = {96, 96, 104, 104};
      int[] wantOy1 = {188, 188, 188, 188};
      int[] wantOz1 = {305, 295, 295, 305};
      for (int i = 0; i < 4; i++) {
         Check.eq(ps[1].ox[i], wantOx1[i], "plane1_ox" + i);
         Check.eq(ps[1].oy[i], wantOy1[i], "plane1_oy" + i + "_188");
         Check.eq(ps[1].oz[i], wantOz1[i], "plane1_oz" + i);
      }
      // Plane copies the shared scratch arrays (:101-109), so the planes do
      // not alias each other
      Check.ok(ps[1].ox != ps[2].ox, "plane_ox_arrays_distinct");

      // Plane.java:111-124 uses only vertices 1 and 2: the hub's ox span is
      // 0 -> typ 1; a rim face has ox 8 vs oy/oz 0 -> typ 2 then 3 (the last
      // if wins), the documented tie order
      Check.eq(ps[0].typ, 1, "plane0_typ_1");
      Check.eq(ps[1].typ, 3, "plane1_typ_3_tie_overwrite");

      // colours: color {75,75,75} (:9), Medium.snap default {0,0,0} ->
      // c = color + color*snap/100 = color (Plane.java:136-149); equal
      // channels -> nocol (Plane.java:132-134); RGBtoHSB of grey is
      // {0,0,75/255} and bfase 0 skips every clamp (Plane.java:162-194)
      Check.eq(ps[0].c[0], 75, "plane0_c0_75");
      Check.eq(ps[0].c[1], 75, "plane0_c1_75");
      Check.eq(ps[0].c[2], 75, "plane0_c2_75");
      Check.eq(ps[0].oc[0], 75, "plane0_oc0_75");
      Check.eq(ps[0].oc[2], 75, "plane0_oc2_75");
      Check.eq(ps[0].nocol, true, "plane0_nocol_true");
      Check.eq(ps[0].hsb[0], 0.0F, "plane0_hsb_hue_0");
      Check.eq(ps[0].hsb[1], 0.0F, "plane0_hsb_sat_0");
      Check.eq(ps[0].hsb[2], 75 / 255.0F, "plane0_hsb_bri_75_255");
      Check.eq(ps[1].c[0], 75, "plane1_c0_75");
      Check.eq(ps[1].nocol, true, "plane1_nocol_true");

      // Plane arg mapping at :44: p8 glass=false, p9 gr, p10 wx, p11 wy,
      // p12 wz, p13 disline=7, p14 bfase=0, p15 road=false
      Check.eq(ps[0].glass, false, "plane0_glass_false");
      Check.eq(ps[0].road, false, "plane0_road_false");
      Check.eq(ps[0].wx, 0, "plane0_wx_0_when_var8_not_11");
      Check.eq(ps[0].wy, 200, "plane0_wy_200");
      Check.eq(ps[0].wz, 300, "plane0_wz_300");
      Check.eq(ps[0].disline, 7, "plane0_disline_7");
      Check.eq(ps[0].bfase, 0, "plane0_bfase_0");
      Check.eq(ps[8].wy, 200, "plane8_wy_200");
      Check.eq(ps[8].wz, 300, "plane8_wz_300");
      Check.eq(ps[1].disline, 7, "plane1_disline_7");
      Check.eq(ps[0].projf, 1.0F, "plane0_projf_untouched_1");

      // deltaf (Plane.java:201-223): float accumulator starting at 1.0F,
      // multiplied by (sqrt(int squared-dist)/100.0 cast to float) for the
      // six ORDERED index pairs, then /3.0F. Hub pairs:
      //   (1,0)=10 -> 0.1        (2,0)=sqrt(0+49+289)/100
      //   (2,1)=sqrt(0+49+49)/100
      // so 1.0F*a*b*a*c*b*c/3.0F; the int bits are the cross-language pin.
      float a = (float) (Math.sqrt(0.0 + 0.0 + 10.0 * 10.0) / 100.0);
      float b = (float) (Math.sqrt(0.0 + 7.0 * 7.0 + 17.0 * 17.0) / 100.0);
      float c = (float) (Math.sqrt(0.0 + 7.0 * 7.0 + 7.0 * 7.0) / 100.0);
      Check.eq(Float.floatToIntBits(ps[0].deltaf), 898904508, "plane0_deltaf_bits");
      Check.eq(ps[0].deltaf, 1.0F * a * b * a * c * b * c / 3.0F, "plane0_deltaf_formula");
      // rim face (:48-60): (1,0)=10, (2,0)=sqrt(64+100), (2,1)=8
      float r0 = (float) (Math.sqrt(0.0 + 0.0 + 10.0 * 10.0) / 100.0);
      float r1 = (float) (Math.sqrt(8.0 * 8.0 + 0.0 + 10.0 * 10.0) / 100.0);
      float r2 = (float) (Math.sqrt(8.0 * 8.0 + 0.0 + 0.0) / 100.0);
      Check.eq(Float.floatToIntBits(ps[1].deltaf), 884725071, "plane1_deltaf_bits");
      Check.eq(ps[1].deltaf, 1.0F * r0 * r1 * r0 * r2 * r1 * r2 / 3.0F, "plane1_deltaf_formula");
      // rim face 2 (:62-74): (1,0)=sqrt(98), (2,0)=sqrt(64+49+49), (2,1)=8
      float s0 = (float) (Math.sqrt(0.0 + 7.0 * 7.0 + 7.0 * 7.0) / 100.0);
      float s1 = (float) (Math.sqrt(8.0 * 8.0 + 7.0 * 7.0 + 7.0 * 7.0) / 100.0);
      float s2 = (float) (Math.sqrt(8.0 * 8.0 + 0.0 + 0.0) / 100.0);
      Check.eq(Float.floatToIntBits(ps[2].deltaf), 884331756, "plane2_deltaf_bits");
      Check.eq(ps[2].deltaf, 1.0F * s0 * s1 * s0 * s2 * s1 * s2 / 3.0F, "plane2_deltaf_formula");

      // make() mutates neither the Medium's random walk nor the Trackers
      Check.eq(m.cntrn, 0, "make_medium_cntrn_untouched");
      Check.eq(m.trn, 0, "make_medium_trn_untouched");
      Check.eq(m.rand[0], 0, "make_medium_rand0_untouched");
      Check.eq(t.nt, 0, "make_trackers_nt_untouched");
      Check.ok(ps[0].m == m, "plane0_medium_identity");
      Check.ok(ps[0].t == t, "plane0_trackers_identity");
   }

   /** Fixture B exercises both make() branches at once:
    *  var8==11 -> var15 = (int)(var5 + 4.0F*var16) (:13-15), and
    *  |var5|==73 -> var18 = -1 (:17-20). x=-73, s=20 (var16=2.0F),
    *  y=200, z=300 (var17=0.5F). */
   private static void branchArms() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      Wheels w = new Wheels();
      Plane[] ps = new Plane[16];
      w.make(m, t, ps, 0, -73, 200, 300, 11, 20, 5);

      Check.eq(w.mast, 1, "branch_mast_1");
      // :118-120 - (int)(200 + 12*0.5 + 1.0)
      Check.eq(w.ground, 207, "branch_ground_207");
      // :13-15 - (int)(-73 + 4*2.0) = -65, passed as wx to every plane
      Check.eq(ps[0].wx, -65, "branch_var8_11_wx_minus65");
      Check.eq(ps[5].wx, -65, "branch_var8_11_wx_all_planes");
      // :17-20 - Math.abs(-73)==73, so var18 flips to -1
      Check.eq(ps[1].gr, -1, "branch_abs73_gr_minus1");
      Check.eq(ps[0].gr, 0, "branch_hub_gr_still_0");
      // :25 - (int)(-73 - 4*2.0) = -81 for every hub x
      Check.eq(ps[0].ox[0], -81, "branch_ox0_minus81");
      Check.eq(ps[0].ox[7], -81, "branch_ox7_minus81");
      // :28-29 - (int)(200 - 12*0.5), (int)(300 + 5*0.5)
      Check.eq(ps[0].oy[0], 194, "branch_oy0_194");
      Check.eq(ps[0].oz[0], 302, "branch_oz0_302_truncated_302_5");
      Check.eq(ps[1].oz[1], 297, "branch_oz1_297_truncated_297_5");
      Check.eq(ps[0].n, 8, "branch_hub_n_8");
   }

   /** Two make() calls on one Wheels: mast counts calls (:45), ground is a
    *  monotone max (:118-120), and each 9-slot block starts fresh. */
   private static void repeatedMake() {
      Medium m = new Medium();
      Trackers t = new Trackers();
      Wheels w = new Wheels();
      Plane[] ps = new Plane[24];
      w.make(m, t, ps, 0, 73, 100, 100, 0, 10, 10);
      Check.eq(w.mast, 1, "repeat_mast_after_first");
      Check.eq(w.ground, 113, "repeat_ground_113");
      // |73|==73 -> -1 as well: the abs() arm does not care about the sign
      Check.eq(ps[1].gr, -1, "repeat_abs73_positive_gr_minus1");

      w.make(m, t, ps, 9, 73, 50, 50, 0, 10, 10);
      Check.eq(w.mast, 2, "repeat_mast_after_second");
      // candidate (int)(50 + 12 + 1) = 63 < 113, so ground keeps the max
      Check.eq(w.ground, 113, "repeat_ground_keeps_max");
      Check.eq(ps[9].master, 2, "repeat_slot9_master_2");
      Check.eq(ps[10].master, 0, "repeat_slot10_master_0");
      Check.eq(ps[17].master, 0, "repeat_slot17_master_0");
      Check.eq(ps[17].n, 4, "repeat_slot17_n_4");
      Check.eq(ps[9].n, 8, "repeat_slot9_n_8");
      Check.eq(ps[18], null, "repeat_slot18_null");
      // the first block is untouched by the second call
      Check.eq(ps[0].master, 1, "repeat_slot0_master_still_1");
      Check.eq(ps[0].wx, 0, "repeat_slot0_wx_still_0");
   }
}
