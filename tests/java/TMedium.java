/**
 * Pins Medium's math core against the JDK: field defaults, the sin/cos
 * table build and normalization, ys()'s int projection, random()'s
 * 3-digit random-walk structure (deterministic parts only - the digits
 * themselves are Math.random-fed), the fade machine, the snap-blended
 * setfade, and addsp's 5-slot guard.
 * Pin source: notes/JAVA-BEHAVIOR/B3_wheels_medium.md (verified lines).
 */
public class TMedium {
   /** constants.toml [medium] - the camera/palette/fade defaults. */
   static final Toml K = Toml.load("tests/fixtures/toml/constants.toml").table("medium");

   static void run() {
      Check.group("Medium");
      K.requireKeys("focus_point", "ground", "fade0", "fade7", "addsp_slots");
      Medium m = new Medium();

      // field defaults (Medium.java:5-47)
      Check.eq(m.focus_point, K.intv("focus_point"), "default_focus_point");
      Check.eq(m.ground, K.intv("ground"), "default_ground");
      Check.eq(m.skyline, K.intv("skyline"), "default_skyline");
      Check.eq(m.origfade, K.intv("fade0"), "default_origfade");
      Check.eq(m.cx, K.intv("cx"), "default_cx");
      Check.eq(m.cy, K.intv("cy"), "default_cy");
      Check.eq(m.cz, K.intv("cz"), "default_cz");
      Check.eq(m.vxz, K.intv("vxz"), "default_vxz");
      Check.eq(m.adv, K.intv("adv"), "default_adv");
      Check.eq(m.hit, K.intv("hit"), "default_hit");
      Check.eq(m.ptcnt, K.intv("ptcnt"), "default_ptcnt");
      Check.eq(m.w, K.intv("w"), "default_w");
      Check.eq(m.h, K.intv("h"), "default_h");
      Check.eq(m.nsp, 0, "default_nsp");
      Check.eq(m.cntrn, 0, "default_cntrn");
      Check.eq(m.trn, 0, "default_trn");
      Check.eq(m.fade.length, 8, "default_fade_len");
      Check.eq(m.fade[0], K.intv("fade0"), "default_fade0");
      Check.eq(m.fade[7], K.intv("fade7"), "default_fade7");
      Check.eq(m.csky[0], K.intv("csky0"), "default_csky0");
      Check.eq(m.csky[2], K.intv("csky2"), "default_csky2");
      Check.eq(m.cgrnd[0], K.intv("cgrnd0"), "default_cgrnd0");
      Check.eq(m.cgrnd[2], K.intv("cgrnd2"), "default_cgrnd2");
      Check.eq(m.cfade[0], K.intv("cfade0"), "default_cfade0");
      Check.eq(m.cfade[1], K.intv("cfade1"), "default_cfade1");

      // the ctor's tables are the double-precision degree formula,
      // float-cast (Medium.java:111-123) - exact quarter-turns and the
      // formula pin for the inexact ones
      Check.eq(m.tcos[0], 1.0F, "tcos_0");
      Check.eq(m.tsin[0], 0.0F, "tsin_0");
      Check.eq(m.tsin[90], 1.0F, "tsin_90");
      Check.eq(m.tsin[270], -1.0F, "tsin_270");
      Check.eq(m.tcos[180], -1.0F, "tcos_180");
      Check.eq(m.tcos[90], (float)Math.cos(90 * (Math.PI / 180.0)), "tcos_90_formula");
      Check.eq(m.tsin[180], (float)Math.sin(180 * (Math.PI / 180.0)), "tsin_180_formula");

      // sin()/cos() normalize with while loops, not modulo
      // (Medium.java:99-109, 554-564)
      Check.eq(m.sin(390), m.sin(30), "sin_390_wraps");
      Check.eq(m.sin(-30), m.sin(330), "sin_neg30_wraps");
      Check.eq(m.cos(-90), m.cos(270), "cos_neg90_wraps");
      Check.eq(m.sin(720 + 45), m.sin(45), "sin_765_wraps");

      // ys(): int arithmetic, truncating division, d<10 clamp
      // (Medium.java:91-97)
      Check.eq(m.ys(100, 50), -600, "ys_100_50");
      Check.eq(m.ys(0, 50), -1400, "ys_0_50");
      Check.eq(m.ys(5, 10), -7600, "ys_5_10");
      Check.eq(m.ys(0, 9), -7800, "ys_d_clamped");
      // truncation toward zero: -34900/51 = -684.31 -> -684
      Check.eq(m.ys(100, 51), -584, "ys_truncates_toward_zero");

      // random(): the 3-digit random-walk. Deterministic structure
      // only (Medium.java:49-89): the first call refills (cntrn 0->20),
      // every call decrements cntrn, advances trn cyclically and walks
      // ALL THREE rand[] slots by +-1 (mod 10); the domain is exactly
      // {0.0 .. 0.9} at k/10.0F.
      m.random();
      // call 1 refills: cntrn 0 -> 20, trn 0 -> 1
      Check.eq(m.cntrn, 20, "random_first_call_refills");
      Check.eq(m.trn, 1, "random_trn_1");
      m.random();
      // call 2 only decrements
      Check.eq(m.cntrn, 19, "random_second_call_decrements");
      Check.eq(m.trn, 2, "random_trn_2");
      m.random();
      Check.eq(m.trn, 0, "random_trn_cycles");
      int[] before = new int[]{m.rand[0], m.rand[1], m.rand[2]};
      m.random();
      for (int i = 0; i < 3; i++) {
         int delta = (m.rand[i] - before[i] + 10) % 10;
         Check.ok(delta == 1 || delta == 9, "random_walks_slot" + i);
      }
      boolean domain = true;
      for (int c = 0; c < 25; c++) {
         float v = m.random();
         boolean inDomain = false;
         for (int k = 0; k < 10; k++) {
            if (v == (float)k / 10.0F) {
               inDomain = true;
            }
         }
         if (!inDomain) {
            domain = false;
         }
      }
      Check.ok(domain, "random_domain_10_values");
      // cadence (probe-verified): refill at call 1 and every 21 calls
      // after - 29 calls in, the last refill was call 22 -> cntrn 13;
      // the next refill lands at call 43
      Check.eq(m.cntrn, 13, "random_cntrn_after_29_calls");
      for (int c = 0; c < 14; c++) {
         m.random();
      }
      Check.eq(m.cntrn, 20, "random_refills_every_21_calls");

      // adjstfade: the fade[0] != origfade guard makes the growth arm a
      // no-op when already at rest (Medium.java:427-443)
      m.adjstfade(20.0F);
      Check.eq(m.fade[0], K.intv("fade_floor"), "adjstfade_guard_noop");
      Check.eq(m.fade[7], K.intv("fade7"), "adjstfade_guard_noop_fade7");
      m.fade[0] = 2000;
      m.adjstfade(20.0F);
      Check.eq(m.fade[0], 2000 + K.intv("fade_step"), "adjstfade_growth_500");
      Check.eq(m.fade[1], 5000, "adjstfade_growth_fadfrom");
      Check.eq(m.fade[7], 20000, "adjstfade_growth_fadfrom7");
      m.origfade = 6000;
      m.adjstfade(10.0F);
      // 6000 - 1000*(15-10) = 1000 -> clamped to 3000, then fadfrom
      Check.eq(m.fade[0], K.intv("fade_floor"), "adjstfade_low_clamped");
      m.adjstfade(14.9F);
      // float artifact: 14.9F = 14.8999996..., so 15-14.9F > 0.1 and
      // 6000 - 1000.0F*0.10000004 = 5899.9999 -> (int) truncates to
      // 5899, NOT 5900 (the gate caught the naive expectation)
      Check.eq(m.fade[0], 5899, "adjstfade_low_float_truncation");
      Check.eq(m.fade[1], 11798, "adjstfade_low_fadfrom");

      // fadfrom sets fade[i] = ARGUMENT * (i+1) (Medium.java:535-541)
      m.fadfrom(100);
      Check.eq(m.fade[0], 100, "fadfrom_0");
      Check.eq(m.fade[3], 400, "fadfrom_3");
      Check.eq(m.fade[7], 800, "fadfrom_7");

      // setfade blends each channel with snap[i]/100 then clamps
      // (Medium.java:125-152); snap defaults 0 -> exact copy
      m.setfade(10, 20, 30);
      Check.eq(m.cfade[0], 10, "setfade_snap0");
      Check.eq(m.cfade[1], 20, "setfade_snap1");
      Check.eq(m.cfade[2], 30, "setfade_snap2");
      m.snap[0] = 50;
      m.snap[1] = 200;
      m.snap[2] = 0;
      m.setfade(100, 100, 100);
      Check.eq(m.cfade[0], 150, "setfade_snap50");
      Check.eq(m.cfade[1], 255, "setfade_snap200_clamped");
      Check.eq(m.cfade[2], 100, "setfade_snap0b");

      // addsp: 5 slots, the 6th add is a no-op (Medium.java:445-452)
      m.addsp(1, 2, 3);
      m.addsp(4, 5, 6);
      m.addsp(7, 8, 9);
      m.addsp(10, 11, 12);
      m.addsp(13, 14, 15);
      Check.eq(m.nsp, K.intv("addsp_slots"), "addsp_full");
      m.addsp(99, 99, 99);
      Check.eq(m.nsp, K.intv("addsp_slots"), "addsp_guard");
      Check.eq(m.spx[4], 13, "addsp_slot4_untouched");
      Check.eq(m.spx[0], 1, "addsp_slot0");
   }
}
