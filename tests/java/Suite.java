/**
 * The explicit suite: one JVM, every test class in order. Must run
 * from the repo root (the script does) so asset paths resolve.
 */
public class Suite {
   public static void main(String[] args) {
      int rc = 0;
      try {
         TMod.run();
         TUlawUtils.run();
         TMedium.run();
         TModSlayer.run();
         TMadness.run();
         TMadnessWorld.run();
         TSmall.run();
         TJavaRandom.run();
         TRadicalMod.run();
         TWheels.run();
         TRecord.run();
         TContO.run();
         TContODraw.run();
         TPlane.run();
         TControl.run();
         TGameSparkerLogic.run();
         TxtGraphicsLogic.run();
         TAudioGoldens.run();
         TMadnessDrive.run();
         TModEdges.run();
      } catch (Throwable t) {
         t.printStackTrace();
         rc = 1;
      }
      rc |= Check.report();
      System.exit(rc);
   }
}
