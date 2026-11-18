import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.Random;

/**
 * Deterministic seeding seam for the original game's SINGLE randomness
 * source, Math.random(): points it at a seeded java.util.Random so any
 * later AI/menu pin is reproducible. Helper only - no run(), no Check
 * calls, no dependencies.
 *
 * JDK 8u40 REALITY (probed with javap + reflection, not assumed):
 *
 *   final class java.lang.Math$RandomNumberGeneratorHolder {
 *      static final java.util.Random randomNumberGenerator = new Random();
 *   }
 *   public static double random() {
 *      return RandomNumberGeneratorHolder.randomNumberGenerator.nextDouble();
 *   }
 *
 * The "private static Random randomNumberGenerator; ... initRNG()" shape
 * belongs to JDK 7. On 8u40 the field lives in a holder class and is
 * `static final`, so a plain Field.set(null, ...) throws
 * IllegalAccessException even after setAccessible(true):
 *
 *   Can not set static final java.util.Random field
 *   java.lang.Math$RandomNumberGeneratorHolder.randomNumberGenerator
 *
 * The fix is to clear the FINAL bit on OUR Field object before its first
 * get/set (sun.reflect builds and caches the field accessor on first use,
 * and that accessor remembers finality). Writing the static slot this way
 * is observed by Math.random() in INTERPRETED code - and by COMPILED code
 * ONLY up to the moment HotSpot C2 compiles that call path: C2 constant-
 * folds java.base static finals, capturing WHICHEVER generator sits in
 * the slot at compile time (probed: a fresh-JVM 200k-call stress with the
 * injection installed before compilation stayed bit-identical, while a
 * late-suite install after warm-up was NOT observed - the compiled code
 * kept the pre-injection instance). Usage rules for the suite:
 *   - value pins via injection are reliable only EARLY in the suite
 *     (fresh JVM, before any Math.random-heavy loop gets C2-compiled) -
 *     see TJavaRandom, which runs early for exactly this reason;
 *   - for value determinism in LATE classes, do NOT rely on injection:
 *     use a Medium subclass that overrides random() with a replayed
 *     vector (the TenthsMedium pattern in TRecord) or preset state;
 *   - call-COUNT pins via injection are safe anywhere (the count does
 *     not depend on which stream is consumed).
 *
 * Save-once: install() captures the field's current value only on its
 * first call, so repeated installs can still be undone by one uninstall()
 * and the restored instance is always the true original. On 8u40 reading
 * the field initializes the holder, so that original is the stock
 * unseeded Random (never null). On a JDK-7-shaped Math the field starts
 * null and uninstall() restores null, which is also the stock behavior
 * (the next Math.random() lazily creates a fresh unseeded Random).
 */
public class TestRng {
   private static Field field;
   private static boolean saved;
   private static Object original;
   private static Random injected;

   /** Replaces Math's generator with new Random(seed), restarting the
    *  stream. Safe to call repeatedly: the saved original is the FIRST
    *  one seen, so uninstall() always restores the true stock instance. */
   static void install(long seed) {
      try {
         Field f = field();
         if (!saved) {
            original = f.get(null);
            saved = true;
         }
         injected = new Random(seed);
         f.set(null, injected);
      } catch (Exception e) {
         throw new AssertionError("TestRng.install(" + seed + ") failed", e);
      }
   }

   /** Restores the value captured by the first install(); a no-op if no
    *  install() ever ran. */
   static void uninstall() {
      if (!saved) {
         return;
      }
      try {
         field().set(null, original);
      } catch (Exception e) {
         throw new AssertionError("TestRng.uninstall() failed", e);
      }
   }

   /** The instance most recently installed (identity-check helper). */
   static Random injected() {
      return injected;
   }

   /** The first value saved by install(); null on a JDK-7-shaped Math. */
   static Object savedOriginal() {
      return original;
   }

   /** The generator Math.random() currently delegates to. */
   static Object currentGenerator() {
      try {
         return field().get(null);
      } catch (Exception e) {
         throw new AssertionError("TestRng.currentGenerator() failed", e);
      }
   }

   /** Resolves (once) and unlocks Math's generator field. Tries Math's own
    *  field first for the JDK-7 shape, then each nested holder class for
    *  the JDK-8 shape. */
   private static Field field() {
      if (field == null) {
         Field gen = null;
         try {
            gen = Math.class.getDeclaredField("randomNumberGenerator");
         } catch (NoSuchFieldException jdk7ShapeAbsent) {
            for (Class<?> nested : Math.class.getDeclaredClasses()) {
               try {
                  gen = nested.getDeclaredField("randomNumberGenerator");
                  break;
               } catch (NoSuchFieldException notTheHolder) {
                  // keep scanning
               }
            }
         }
         if (gen == null) {
            throw new AssertionError("TestRng: Math has no randomNumberGenerator field");
         }
         try {
            gen.setAccessible(true);
            if ((gen.getModifiers() & Modifier.FINAL) != 0) {
               Field modifiers = Field.class.getDeclaredField("modifiers");
               modifiers.setAccessible(true);
               modifiers.setInt(gen, gen.getModifiers() & ~Modifier.FINAL);
            }
         } catch (Exception e) {
            throw new AssertionError("TestRng: cannot unlock Math's randomNumberGenerator", e);
         }
         field = gen;
      }
      return field;
   }
}
