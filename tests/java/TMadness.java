/**
 * Pins Madness's zero-fixture surface: py/rpy (the squared-distance
 * helpers with int-truncation and overflow behavior), rot (the table
 * rotation), and the constructor's field defaults including the
 * power==75 vs reseto's 98 distinction.
 * Pin source: notes/JAVA-BEHAVIOR/B9_madness.md (verified lines).
 */
public class TMadness {
   /** constants.toml [madness] - the ctor power and the regression constants. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("madness");

   static void run() {
      Check.group("Madness");
      K.requireKeys("power_default", "power_reset", "power_decay_divisor");

      // py: (a-b)^2 + (c-d)^2, INT arithmetic - overflow wraps
      // (Madness.java:118-120)
      Check.eq(new Madness(null, null, null, 0).py(3, 1, 5, 2), 13, "py_basic");
      Check.eq(new Madness(null, null, null, 0).py(0, -100000, 0, 0), 1410065408, "py_int_overflow_wraps");
      Check.eq(new Madness(null, null, null, 0).py(-1, 0, -1, 0), 2, "py_negatives");

      // rpy: the 3-axis float version, (int)-truncated
      // (Madness.java:286-288)
      Madness m = new Madness(null, null, null, 0);
      Check.eq(m.rpy(0, 1.5F, 0, 0, 0, 0), 2, "rpy_truncates_toward_zero");
      Check.eq(m.rpy(10, 0, 0, 0, 0, 0), 100, "rpy_symmetric_a");
      Check.eq(m.rpy(0, 10, 0, 0, 0, 0), 100, "rpy_symmetric_b");
      Check.eq(m.rpy(0.5F, 0, 0, 0, 0, 0), 0, "rpy_fraction_drops");
      Check.eq(m.rpy(3, 0, 4, 0, 0, 0), 25, "rpy_3_4_5");

      // rot: angle 0 is a no-op even with a null Medium; with a Medium
      // the points rotate through the sin/cos tables (Madness.java:372-381)
      Madness mnull = new Madness(null, null, null, 0);
      float[] a = new float[]{10.0F};
      float[] b = new float[]{0.0F};
      mnull.rot(a, b, 0, 0, 0, 1);
      Check.eq(a[0], 10.0F, "rot_angle0_noop_a");
      Check.eq(b[0], 0.0F, "rot_angle0_noop_b");

      Medium med = new Medium();
      Madness mrot = new Madness(med, null, null, 0);
      float[] ra = new float[]{10.0F, 0.0F};
      float[] rb = new float[]{0.0F, 10.0F};
      mrot.rot(ra, rb, 0, 0, 90, 2);
      Check.eq(ra[0], 10.0F * med.tcos[90], "rot_90_a_uses_tcos");
      Check.eq(rb[0], 10.0F * med.tsin[90], "rot_90_b_uses_tsin");
      // second point: (0,10) rotated 90deg -> (-10*sin, 10*cos)
      Check.eq(ra[1], -10.0F * med.tsin[90], "rot_90_a_point2");
      Check.eq(rb[1], 10.0F * med.tcos[90], "rot_90_b_point2");

      // ctor defaults (Madness.java:279-284, fields :7-116): power is
      // 75.0F - reseto installs 98 later (Madness.java:639), and drive
      // handicaps power!=98 by 0.76 for im==0 (:934)
      Madness md = new Madness(null, null, null, 4);
      Check.eq(md.im, 4, "ctor_im");
      Check.eq(md.cn, 0, "ctor_cn");
      Check.eq(md.power, K.floatv("power_default"), "ctor_power_75_not_98");
      Check.eq(md.focus, -1, "ctor_focus");
      Check.eq(md.speed, 0.0F, "ctor_speed");
      Check.ok(!md.mtouch, "ctor_mtouch");
      Check.ok(!md.wtouch, "ctor_wtouch");
      Check.eq(md.loop, 0, "ctor_loop");
      Check.eq(md.drag[0], 0.5F, "ctor_drag");
      Check.eq(md.point, 0, "ctor_point");
      Check.eq(md.nlaps, 0, "ctor_nlaps");
   }
}
