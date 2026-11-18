import java.util.zip.CRC32;

/**
 * Pins UlawUtils against the JDK: the s2u_lut table properties, the
 * clamping encoder, and the two DISTINCT linear2ulaw overloads.
 * Pin source: notes/JAVA-BEHAVIOR/B11_ulawutils.md (values verified
 * against the table literals before authoring).
 */
public class TUlawUtils {
   static void run() {
      Check.group("UlawUtils");

      // linear2ulawclip: clamp to [-32767, 32767] then s2u_lut[v>>3&8191]
      Check.eq(UlawUtils.linear2ulawclip(0), -1, "clip_0");
      Check.eq(UlawUtils.linear2ulawclip(7), -1, "clip_7_floor");
      Check.eq(UlawUtils.linear2ulawclip(8), -2, "clip_8");
      Check.eq(UlawUtils.linear2ulawclip(1000), -50, "clip_1000");
      Check.eq(UlawUtils.linear2ulawclip(1024), -51, "clip_1024");
      Check.eq(UlawUtils.linear2ulawclip(31744), -128, "clip_31744");
      Check.eq(UlawUtils.linear2ulawclip(32767), -128, "clip_32767");
      // unclamped, 32776>>3&8191 would index 4097 (0x00): the clamp pins it
      Check.eq(UlawUtils.linear2ulawclip(32776), -128, "clip_upper_clamp");
      // the clamp bound is -32767, NOT -32768 (UlawUtils.java:8198-8199)
      Check.eq(UlawUtils.linear2ulawclip(-32776), 0, "clip_lower_clamp");
      Check.eq(UlawUtils.linear2ulawclip(-1), 126, "clip_neg1");
      Check.eq(UlawUtils.linear2ulawclip(-8), 126, "clip_neg8_floor");
      Check.eq(UlawUtils.linear2ulawclip(-9), 125, "clip_neg9");
      Check.eq(UlawUtils.linear2ulawclip(-31744), 1, "clip_neg31744");
      Check.eq(UlawUtils.linear2ulawclip(-32767), 0, "clip_neg32767");
      // the 128-entry zero plateau at indices 4096..4223
      Check.eq(UlawUtils.linear2ulawclip(-32760), 0, "clip_zero_plateau");

      // table properties
      Check.eq(UlawUtils.s2u_lut[0], -1, "lut_0");
      Check.eq(UlawUtils.s2u_lut[4095], -128, "lut_4095");
      Check.eq(UlawUtils.s2u_lut[4096], 0, "lut_4096");
      Check.eq(UlawUtils.s2u_lut[4223], 0, "lut_4223");
      Check.eq(UlawUtils.s2u_lut[4224], 1, "lut_4224");
      Check.eq(UlawUtils.s2u_lut[8191], 126, "lut_8191");
      Check.eq(UlawUtils.s2u_lut.length, 8192, "lut_len");
      int bad = 0;
      for (int i = 0; i < UlawUtils.s2u_lut.length; i++) {
         if (UlawUtils.s2u_lut[i] == 127) {
            bad++;
         }
      }
      Check.eq(bad, 0, "lut_no_0x7F");

      // the byte overload: s2u_lut[((b & 255) << 5) + 15] - an unrelated
      // index formula that never reads [0..14] or [8176..8191]
      Check.eq(UlawUtils.linear2ulaw((byte)0), -16, "byte_0");
      Check.eq(UlawUtils.linear2ulaw((byte)1), -32, "byte_1");
      Check.eq(UlawUtils.linear2ulaw((byte)15), -80, "byte_15");
      Check.eq(UlawUtils.linear2ulaw((byte)127), -128, "byte_127");
      Check.eq(UlawUtils.linear2ulaw((byte)-128), 0, "byte_neg128");
      Check.eq(UlawUtils.linear2ulaw((byte)-1), 111, "byte_neg1");
      Check.ok(UlawUtils.linear2ulaw((byte)0) != UlawUtils.linear2ulaw(0), "overloads_differ");

      // the int overload: s2u_lut[v >> 3] with NO clamp
      Check.eq(UlawUtils.linear2ulaw(0), -1, "int_0");
      Check.eq(UlawUtils.linear2ulaw(7), -1, "int_7_floor");
      Check.eq(UlawUtils.linear2ulaw(8), -2, "int_8");
      Check.eq(UlawUtils.linear2ulaw(32767), -128, "int_32767");
      Check.eq(UlawUtils.linear2ulaw(65535), 126, "int_65535");
      Check.throwsIt(new Check.Task() {
         public void run() {
            UlawUtils.linear2ulaw(65536);
         }
      }, ArrayIndexOutOfBoundsException.class, "int_65536_throws");
      Check.throwsIt(new Check.Task() {
         public void run() {
            UlawUtils.linear2ulaw(-1);
         }
      }, ArrayIndexOutOfBoundsException.class, "int_neg1_throws");

      // the table's overall shape: descending -1..-128 half, 128-entry
      // zero plateau, ascending 1..126 half (NOT monotone as signed
      // bytes - the gate caught that pin and the shape below is the
      // verified structure)
      Check.ok(isShape(), "lut_shape");
      Check.eq(distinctCount(), 255, "lut_255_distinct");
      Check.eq(tableCrc(), 2352583155L, "lut_crc32");
   }

   private static boolean isShape() {
      for (int i = 1; i < 4096; i++) {
         if (UlawUtils.s2u_lut[i] > UlawUtils.s2u_lut[i - 1]) {
            return false;
         }
      }
      for (int i = 4096; i < 4224; i++) {
         if (UlawUtils.s2u_lut[i] != 0) {
            return false;
         }
      }
      for (int i = 4225; i < 8192; i++) {
         if (UlawUtils.s2u_lut[i] < UlawUtils.s2u_lut[i - 1]) {
            return false;
         }
      }
      return true;
   }

   private static int distinctCount() {
      java.util.Set<Byte> seen = new java.util.HashSet<Byte>();
      for (byte b : UlawUtils.s2u_lut) {
         seen.add(b);
      }
      return seen.size();
   }

   /** Pins the WHOLE table through a digest, not just sample entries. */
   private static long tableCrc() {
      CRC32 crc = new CRC32();
      crc.update(UlawUtils.s2u_lut);
      return crc.getValue();
   }
}
