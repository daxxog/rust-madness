import java.util.Arrays;
import java.util.Random;

/**
 * Pins java.util.Random's Javadoc-SPECIFIED 48-bit LCG and the Math.random()
 * injection seam. These vectors are the cross-language pin material: the
 * Rust mirror implements the same LCG and must reproduce every literal here.
 *
 * Spec (java.util.Random Javadoc, stable across JDKs forever):
 *   seed  = (initialSeed ^ 0x5DEECE66DL) & ((1L << 48) - 1)
 *   next  = (seed * 0x5DEECE66DL + 0xBL) & ((1L << 48) - 1)
 *   next(bits)        = (int)(next >>> (48 - bits))
 *   nextDouble()      = ((next(26) << 27) + next(27)) * 0x1.0p-53
 *   nextInt(bound)    = rejection loop over next(31); for a power of two,
 *                       (int)((bound * (long)next(31)) >> 31)
 *   nextBoolean()     = next(1) != 0
 *   nextLong()        = ((long)next(32) << 32) + next(32)
 *
 * Everything below was PROBED on the gate's Oracle 8u40 (scratch harness
 * under target/java-tests-probe, deleted afterwards) and then pinned as
 * literals; the Medium group additionally pins the game's own random()
 * walk, whose only entropy source is Math.random() (Medium.java:54-55;
 * GameSparker.java:616; xtGraphics.java:2289-2376 - and no `new Random`
 * anywhere in reference/vineflower/).
 *
 * ORDER SENSITIVITY (probed in TRecord, see TestRng's header): the
 * injection pins in mathInjection/mediumBridge require a COLD JIT at
 * this suite position - C2 constant-folds Math's holder field at
 * compile time, so a compile that happens BEFORE this class runs would
 * capture the stock generator and make these pins fail. Keep this class
 * EARLY in Suite (before any Math.random-heavy class); a failure here
 * after inserting earlier classes is that exact signal, not a code bug.
 *
 * Seeding is always scoped: install/uninstall wrap each group in
 * try/finally, so no other suite class ever sees a seeded generator
 * (TMedium's existing pins were built against stock Math.random()).
 */
public class TJavaRandom {
   static void run() {
      Check.group("Rng");
      lcgVectors();
      reseedRepeat();
      mathInjection();
      mediumBridge();
      uninstallContract();
   }

   /** Random(42): doubles (bit-exact), nextInt(100), the power-of-two and
    *  rejection nextInt paths, nextBoolean and nextLong. */
   private static void lcgVectors() {
      Random d = new Random(42);
      long[] doubles = {
            0x3fe74833a06ff457L, // 0.7275636800328681
            0x3fe5dcf778622e01L, // 0.6832234717598454
            0x3fd3c20f3f12bbb4L, // 0.30871945533265976
            0x3fd1bba76b52c856L, // 0.27707849007413665
            0x3fe54c2d50bb0864L, // 0.6655489517945736
            0x3fece86cf39c2cbeL, // 0.9033722646721782
            0x3fd79a23a61b35c8L, // 0.36878291341130565
            0x3fd1a5db3b0bfbe2L  // 0.2757480694417024
      };
      for (int i = 0; i < doubles.length; i++) {
         Check.eq(Double.doubleToLongBits(d.nextDouble()), doubles[i], "lcg_next_double_bits_" + i);
      }

      Random hi = new Random(42);
      int[] hundreds = {30, 63, 48, 84, 70, 25, 5, 18};
      for (int i = 0; i < hundreds.length; i++) {
         Check.eq(hi.nextInt(100), hundreds[i], "lcg_next_int_100_" + i);
      }

      // 16 is a power of two: the (bound * next(31)) >> 31 arm
      Check.eq(new Random(42).nextInt(16), 11, "lcg_next_int_16_pow2");
      // 1000000000 is not a power of two: the rejection arm
      Check.eq(new Random(42).nextInt(1000000000), 562431130, "lcg_next_int_1e9");

      Random b = new Random(42);
      boolean[] booleans = {true, false, true, false};
      for (int i = 0; i < booleans.length; i++) {
         Check.eq(b.nextBoolean(), booleans[i], "lcg_next_boolean_" + i);
      }

      Random l = new Random(42);
      Check.eq(l.nextLong(), -5025562857975149833L, "lcg_next_long_0"); // 0xba419d350dfe8af7
      Check.eq(l.nextLong(), -5843495416241995736L, "lcg_next_long_1"); // 0xaee7bbe10c45c028
   }

   /** setSeed(42) on an already-advanced Random(42) restarts the stream:
    *  the next doubles are again lcg_next_double_bits_0/1. */
   private static void reseedRepeat() {
      Random r = new Random(42);
      for (int i = 0; i < 20; i++) {
         r.nextDouble();
      }
      r.setSeed(42);
      Check.eq(Double.doubleToLongBits(r.nextDouble()), 0x3fe74833a06ff457L, "reseed_next_double_bits_0");
      Check.eq(Double.doubleToLongBits(r.nextDouble()), 0x3fe5dcf778622e01L, "reseed_next_double_bits_1");
   }

   /** Replacing Math's generator with Random(7) makes Math.random() emit
    *  exactly Random(7).nextDouble(); two values are also pinned as
    *  literals so the seam is strong without the local reference. */
   private static void mathInjection() {
      TestRng.install(7);
      try {
         Random ref = new Random(7);
         double[] got = new double[5];
         for (int i = 0; i < got.length; i++) {
            got[i] = Math.random();
            Check.eq(Double.doubleToLongBits(got[i]), Double.doubleToLongBits(ref.nextDouble()),
                  "math_injection_call_" + i);
         }
         Check.eq(Double.doubleToLongBits(got[0]), 0x3fe761e2f51bb9a0L, "math_injection_first_bits_pin");
         Check.eq(Double.doubleToLongBits(got[1]), 0x3fe7f9328813b859L, "math_injection_second_bits_pin");
      } finally {
         TestRng.uninstall();
      }
   }

   /** Medium.random() (Medium.java:49-89) draws ONLY from Math.random(), so
    *  its 5-call output vector is a pure function of the injected seed.
    *  install(11) twice must replay it; install(12) must not. */
   private static void mediumBridge() {
      float[] seed11 = new float[5];
      TestRng.install(11);
      try {
         Medium m1 = new Medium();
         for (int i = 0; i < seed11.length; i++) {
            seed11[i] = m1.random();
         }
      } finally {
         TestRng.uninstall();
      }

      // reseed: a fresh Random(11) restarts the stream, a fresh Medium
      // starts with cntrn == 0, so the second vector must equal the first
      float[] seed11Again = new float[5];
      TestRng.install(11);
      try {
         Medium m2 = new Medium();
         for (int i = 0; i < seed11Again.length; i++) {
            seed11Again[i] = m2.random();
         }
      } finally {
         TestRng.uninstall();
      }
      for (int i = 0; i < seed11.length; i++) {
         Check.eq(Float.floatToIntBits(seed11Again[i]), Float.floatToIntBits(seed11[i]),
               "medium_bridge_reseed_same_" + i);
      }

      // literal pin of the seed-11 vector (the Rust mirror's target)
      int[] want11 = {1063675494, 1045220557, 0, 1058642330, 1063675494};
      for (int i = 0; i < want11.length; i++) {
         Check.eq(Float.floatToIntBits(seed11[i]), want11[i], "medium_bridge_seed11_bits_" + i);
      }

      // a different seed must give a different vector (seed sensitivity)
      float[] seed12 = new float[5];
      TestRng.install(12);
      try {
         Medium m3 = new Medium();
         for (int i = 0; i < seed12.length; i++) {
            seed12[i] = m3.random();
         }
      } finally {
         TestRng.uninstall();
      }
      int[] want12 = {0, 1056964608, 0, 1050253722, 1061997773};
      for (int i = 0; i < want12.length; i++) {
         Check.eq(Float.floatToIntBits(seed12[i]), want12[i], "medium_bridge_seed12_bits_" + i);
      }
      Check.ok(!Arrays.equals(seed11, seed12), "medium_bridge_seed12_vector_differs");
   }

   /** After uninstall() the field must hold the FIRST saved original again,
    *  not the injected instance - an identity check, no value flakiness.
    *  On 8u40 that original is the stock Random created by the holder's
    *  <clinit> (never null). */
   private static void uninstallContract() {
      TestRng.install(12345);
      Random injected = TestRng.injected();
      TestRng.uninstall();
      Check.ok(TestRng.currentGenerator() != injected, "uninstall_field_no_longer_injected");
      Check.ok(TestRng.currentGenerator() == TestRng.savedOriginal(), "uninstall_restores_saved_original");
      Check.ok(TestRng.savedOriginal() instanceof Random, "uninstall_saved_original_is_random");
      double live = Math.random();
      Check.ok(live >= 0.0 && live < 1.0, "uninstall_math_random_still_live");
   }
}
