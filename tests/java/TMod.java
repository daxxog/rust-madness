import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.FileInputStream;
import java.util.zip.ZipInputStream;

/**
 * Documents Mod's behavior: the zipo-loading constructor (the game's
 * real path), the public LoadMod(InputStream) seam, and the
 * broken-object state a failed constructor leaves behind (fields at
 * their defaults, insts == null).
 *
 * Pins verified against JDK 1.8.0_40 and mirrored in
 * crates/nfm-core/src/tracker.rs (load_mod tests).
 */
public class TMod {
   static void run() throws Exception {
      Check.group("Mod");
      TestApplet app = new TestApplet("assets/music");

      // the game's real path: zipo container -> first entry -> LoadMod
      Mod s1 = new Mod("stage1.zipo", app);
      Check.eq(s1.toString(), "Stage1 (4 tracks, 20 patterns, 31 samples)", "stage1_toString");
      Check.eq(s1.getNumPatterns(), 20, "stage1_numpatterns");

      Mod s2 = new Mod("stage2.zipo", app);
      Check.eq(s2.getNumPatterns(), 44, "stage2_numpatterns");

      // a missing file leaves a broken object: fields at their defaults
      // (getNumPatterns reads the zero-valued field), while toString
      // NPEs at insts.length - the original never guards this
      Mod broken = new Mod("nonexistent.zipo", app);
      Check.eq(broken.getNumPatterns(), 0, "failed_ctor_numpatterns_default");
      Check.throwsIt(new Check.Task() {
         public void run() {
            broken.toString();
         }
      }, NullPointerException.class, "failed_ctor_toString_npes");

      // the public LoadMod seam parses raw mod bytes with no zipo
      // wrapper - and revives the broken object (all fields reset)
      broken.LoadMod(new ByteArrayInputStream(firstZipoEntry("assets/music/stage1.zipo")));
      Check.eq(broken.toString(), "Stage1 (4 tracks, 20 patterns, 31 samples)", "loadmod_revives_broken");

      // a truncated stream throws (EOFException is an IOException) -
      // the constructor catches it, direct callers see it raw
      Check.throwsIt(new Check.Task() {
         public void run() throws Exception {
            broken.LoadMod(new ByteArrayInputStream(new byte[40]));
         }
      }, java.io.EOFException.class, "loadmod_truncated_throws");
   }

   /** Extracts the first zip entry, mirroring the constructor's read. */
   static byte[] firstZipoEntry(String path) throws Exception {
      DataInputStream in = new DataInputStream(new FileInputStream(path));
      ZipInputStream zip = new ZipInputStream(in);
      zip.getNextEntry();
      ByteArrayOutputStream out = new ByteArrayOutputStream();
      byte[] buf = new byte[4096];
      int n;
      while ((n = zip.read(buf)) > 0) {
         out.write(buf, 0, n);
      }
      in.close();
      zip.close();
      return out.toByteArray();
   }
}
