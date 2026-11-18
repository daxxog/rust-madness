import java.util.ArrayList;
import java.util.List;

/**
 * Minimal dependency-free test harness for documenting the ORIGINAL
 * code's behavior (reference/vineflower is compiled unmodified and
 * exercised, never edited).
 *
 * Design contract:
 * - One JVM run; an explicit Suite lists every test class in order.
 * - Every assertion carries a stable ID (group + id) that the later
 *   Rust mirror maps 1:1 by name, so a green Rust mirror is a direct
 *   faithfulness proof against the original.
 * - No assertion framework dependency: this file is the whole harness.
 */
public class Check {
   /** Allows checked exceptions from test bodies. */
   interface Task {
      void run() throws Throwable;
   }

   private static final List<String> failures = new ArrayList<String>();
   private static int passed = 0;
   private static int failed = 0;
   private static String group = "?";

   /** Sets the group name prefixed onto every assertion ID. */
   static void group(String g) {
      group = g;
   }

   static void eq(Object got, Object want, String id) {
      if (want == null ? got == null : want.equals(got)) {
         ++passed;
      } else {
         ++failed;
         failures.add(group + "." + id + " got <" + got + "> want <" + want + ">");
      }
   }

   static void eq(int got, int want, String id) {
      eq(Integer.valueOf(got), Integer.valueOf(want), id);
   }

   static void eq(long got, long want, String id) {
      eq(Long.valueOf(got), Long.valueOf(want), id);
   }

   static void ok(boolean cond, String id) {
      eq(Boolean.valueOf(cond), Boolean.valueOf(true), id);
   }

   /** Documents that a call throws exactly `type` (or a subclass). */
   static void throwsIt(Task task, Class<? extends Throwable> type, String id) {
      try {
         task.run();
         ++failed;
         failures.add(group + "." + id + " threw nothing, wanted " + type.getSimpleName());
      } catch (Throwable t) {
         if (type.isInstance(t)) {
            ++passed;
         } else {
            ++failed;
            failures.add(group + "." + id + " threw " + t + ", wanted " + type.getSimpleName());
         }
      }
   }

   /** Prints the counters and every failure; returns the exit code. */
   static int report() {
      System.out.println("== " + passed + " passed, " + failed + " failed ==");
      for (String f : failures) {
         System.out.println("FAIL " + f);
      }
      return failed == 0 ? 0 : 1;
   }
}
