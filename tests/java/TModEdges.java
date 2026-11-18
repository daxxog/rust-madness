import java.io.ByteArrayInputStream;

/**
 * Pins the LOADER EDGES of Mod.java and the remaining effect/pattern
 * edges of ModSlayer.java (the arms the stage*.zipo goldens never take).
 *
 * Position in Suite: APPENDED after TAudioGoldens. Safe because this
 * class is randomness-free (Mod/ModSlayer are pure integer parsers -
 * B2's "Nondeterminism: none" for both) and carries no shared slot or
 * stream state: every fixture is built from local byte[]/field writes,
 * so no earlier test can influence its outcome.
 *
 * Synthetic MOD layout used below (Mod.java:101-147):
 *   0..19        name (readText 20)
 *   20..         instruments, 30 bytes each (readInstrument :156-174)
 *   seqOff       song_length u8, song_repeat u8 (readSequence :81-99)
 *   seqOff+2     128 position bytes
 *   seqOff+130   4-byte magic (skipped at :135)
 *   patternOff   numpatterns * numtracks*4*64 pattern bytes (:188-196)
 *   patternOff+  sample data (:149-153)
 * The MAGIC IS ALWAYS DETECTED AT OFFSET 1080 (:106-108: mark(1068),
 * skip(1060), readInt, reset) - even for the 15-instrument fallback,
 * whose sequence header therefore starts at 470. build() writes the
 * magic there in both cases.
 */
public class TModEdges {
   /** constants.toml [modslayer] - the turnbytes safety cap. */
   static final Toml KM = Toml.load("tests/fixtures/toml/constants.toml").table("modslayer");

   static void run() throws Exception {
      Check.group("ModEdges");
      Check.eq(KM.intv("turnbytes_cap"), 3500000, "turnbytes_cap_key_3500000");
      TestApplet app = new TestApplet("assets/music");

      // ---- voice detection (Mod.java:111-126) ----
      // every listed FOURCC sets var3=31; only 8CHN/6CHN/28CH change
      // numtracks. FLT8 is the documented quirk: 31 instruments but 4
      // tracks, because :119-124 only tests 8CHN/6CHN/28CH.
      String[] magics = {"M.K.", "M!K!", "M&K!", "FLT4", "FLT8", "8CHN", "6CHN", "28CH"};
      int[] tracks = {4, 4, 4, 4, 4, 8, 6, 28};
      for (int i = 0; i < magics.length; i++) {
         Mod mod = load(build(magics[i], 3, 2, positions(0, 1, 2), 3, tracks[i],
               insts(31, 64, 0, 0), false));
         Check.eq(mod.getNumTracks(), tracks[i], "voice_" + key(magics[i]) + "_tracks");
         Check.eq(mod.insts.length, 31, "voice_" + key(magics[i]) + "_31_instruments");
         // max(positions)+1 == 3 (readSequence :90-98)
         Check.eq(mod.getNumPatterns(), 3, "voice_" + key(magics[i]) + "_patterns");
      }

      // an unlisted FOURCC leaves var3==15 (Mod.java:103,118) - the
      // 15-instrument ProTracker fallback, numtracks still 4 (:104)
      Mod fifteen = load(build("ZZZZ", 2, 1, positions(0, 1), 2, 4,
            insts(15, 64, 0, 0), false));
      Check.eq(fifteen.insts.length, 15, "voice_unknown_15_instruments");
      Check.eq(fifteen.getNumTracks(), 4, "voice_unknown_tracks_4");
      Check.eq(fifteen.getNumPatterns(), 2, "voice_unknown_patterns_2");
      Check.eq(fifteen.getName(), "edge", "voice_unknown_name_trimmed");

      // ---- readSequence repeat clamp (Mod.java:86-88) ----
      // repeat > length -> clamp; repeat <= length -> untouched
      Mod clamped = load(build("M.K.", 5, 200, positions(0, 3), 4, 4,
            insts(31, 64, 0, 0), false));
      Check.eq(clamped.song_length_patterns, 5, "seq_song_length_5");
      Check.eq(clamped.song_repeat_patterns, 5, "seq_repeat_clamped_to_length");
      Check.eq(clamped.getNumPatterns(), 4, "seq_numpatterns_max_pos_plus_1");
      Mod unclamped = load(build("M.K.", 5, 2, positions(0), 1, 4,
            insts(31, 64, 0, 0), false));
      Check.eq(unclamped.song_repeat_patterns, 2, "seq_repeat_kept_when_smaller");
      // the all-zero position table is the "empty section" edge:
      // numpatterns == 1 (max 0 + 1, Mod.java:90-98)
      Mod empty = load(build("M.K.", 0, 0, positions(), 1, 4,
            insts(31, 64, 0, 0), false));
      Check.eq(empty.getNumPatterns(), 1, "seq_empty_positions_one_pattern");

      // ---- readInstrument clamps (Mod.java:159-171) ----
      // words are doubled: sample_length = words*2 (:159)
      // instrument 0: repeat_point 200 words -> 400 > 128 -> clamp to 128
      //               repeat_length 100 words -> 200; 128+200 > 128 ->
      //               clamp repeat_length to 128-128 == 0
      Inst[] cl = insts(31, 64, 0, 0);
      cl[0] = new Inst(64, 200, 100);
      cl[1] = new Inst(64, 0, 200);
      cl[2] = new Inst(10, 2, 3);
      Mod clamps = load(build("M.K.", 1, 0, positions(0), 1, 4, cl, false));
      Check.eq(clamps.insts[0].sample_length, 128, "inst_sample_length_words_x2");
      Check.eq(clamps.insts[0].repeat_point, 128, "inst_repeat_point_clamped");
      Check.eq(clamps.insts[0].repeat_length, 0, "inst_repeat_length_clamped_zero");
      Check.eq(clamps.insts[1].repeat_length, 128, "inst_repeat_length_clamped_to_sample");
      // instrument 2: point 2 words (->4), length 3 words (->6) into a
      // 20-byte sample: 4+6 <= 20 so neither clamp fires
      Check.eq(clamps.insts[2].repeat_point, 4, "inst_repeat_point_kept");
      Check.eq(clamps.insts[2].repeat_length, 6, "inst_repeat_length_kept");

      // ---- readSampleData repeat copy (Mod.java:149-153) ----
      // repeat_length > 3 copies 8 bytes from repeat_point to
      // samples[sample_length .. +8]. Instrument 0: len 4 words -> 8
      // bytes of sample; repeat point 0, repeat length 4 -> copy
      // samples[0..8] into samples[8..16].
      Inst[] repInst = insts(31, 64, 0, 0);
      repInst[0] = new Inst(4, 0, 4);
      Mod rep = load(build("M.K.", 1, 0, positions(0), 1, 4, repInst, false));
      Check.eq(rep.insts[0].samples[0], (byte) 1, "sample_first_byte");
      Check.eq(rep.insts[0].samples[7], (byte) 8, "sample_last_byte");
      Check.eq(rep.insts[0].samples[8], (byte) 1, "sample_repeat_copy_first");
      Check.eq(rep.insts[0].samples[15], (byte) 8, "sample_repeat_copy_last");
      // repeat_length <= 3 skips the copy: the tail stays zero
      Inst[] norepInst = insts(31, 64, 0, 0);
      norepInst[0] = new Inst(4, 0, 1);
      Mod norep = load(build("M.K.", 1, 0, positions(0), 1, 4, norepInst, false));
      Check.eq(norep.insts[0].samples[8], (byte) 0, "sample_short_repeat_no_copy");

      // ---- truncated sample data (Mod.java:138-144) ----
      // the EOF on the sample section is CAUGHT and only prints a
      // warning, so the object stays fully parsed
      Inst[] eofInst = insts(31, 64, 0, 0);
      eofInst[0] = new Inst(64, 0, 0);
      Mod eof = load(build("M.K.", 1, 0, positions(0), 1, 4, eofInst, true));
      Check.eq(eof.getNumPatterns(), 1, "eof_samples_patterns_kept");
      Check.eq(eof.getNumTracks(), 4, "eof_samples_tracks_kept");

      // ---- ModSlayer: get_track edge parses (ModSlayer.java:623-773) ----
      ModSlayer sl = slayer(app, 4);

      // effect 0 == arpeggio: the period walk runs 12..47 and breaks at
      // 48 when period(0) is below every threshold (:657-661)
      ModTrackInfo t = row(sl, 0, 0, 0, 5);
      Check.eq(t.effect, 16, "track_arp_effect_bit");
      Check.eq(t.arp[0], 107, "track_arp_period_at_48");
      Check.eq(t.arp[1], 80, "track_arp_plus_low_nibble_5");
      Check.eq(t.arpindex, 0, "track_arp_index_reset");

      // case 1 with a non-zero param sets port_up (:671-673)
      t = row(sl, 0, 0, 1, 7);
      Check.eq(t.effect, 4, "track_port_up_effect_bit");
      Check.eq(t.port_up, 7, "track_port_up_value");
      // param 0 leaves port_up at its default
      t = row(sl, 0, 0, 1, 0);
      Check.eq(t.port_up, 0, "track_port_up_zero_kept");

      // case 9 param 0 replays oldsampofs (:717-723)
      t = row(sl, 0, 0, 9, 5);
      Check.eq(t.position, 1280, "track_sample_offset_param_5");
      ModTrackInfo old = new ModTrackInfo();
      old.oldsampofs = 6;
      t = row2(sl, old, 0, 0, 9, 0);
      Check.eq(t.position, 1536, "track_sample_offset_old_6_shl8");
      Check.eq(t.oldsampofs, 6, "track_sample_offset_old_kept");

      // case 12 volume: in-range keeps the param, out-of-range clamps 64
      t = row(sl, 0, 0, 12, 30);
      Check.eq(t.volume, 30, "track_set_volume_30");
      t = row(sl, 0, 0, 12, 70);
      Check.eq(t.volume, 64, "track_set_volume_clamps_64");

      // case 14: the E1/E2 sub-switch is DEAD ORIGINAL CODE. var9 is
      // `var7 & 240` (:737), i.e. always a multiple of 16 (0,16,...,240),
      // while the cases test 1 and 2 (:740,:748), so only the default
      // arm can ever run. The period is therefore untouched.
      t = rowPeriod(sl, 1, 855, 14, 0x15);
      Check.eq(t.period, 855, "track_e14_param_1x_noop_dead_switch");
      t = rowPeriod(sl, 1, 115, 14, 0x25);
      Check.eq(t.period, 115, "track_e14_param_2x_noop_dead_switch");
      t = rowPeriod(sl, 1, 300, 14, 0x05);
      Check.eq(t.period, 300, "track_e14_param_0x_noop");

      // ---- beattrack remaining arms ----
      // PORT_TO both fall-short directions (:400-407)
      ModSlayer slb = slayer(app, 4);
      // approaching from BELOW: the else-if branch subtracts and then
      // pins portto (period 100-100=0 -> 500)
      t = TModSlayer.mti(32, 0, 100);
      t.portto = 500;
      t.port_inc = 100;
      slb.beattrack(t);
      Check.eq(t.period, 500, "port_to_under_lands_at_portto");
      // portto == period: the else-if guard `portto > period` is false,
      // so the period is left alone (the second false arm, :405)
      t = TModSlayer.mti(32, 0, 100);
      t.portto = 100;
      t.port_inc = 100;
      slb.beattrack(t);
      Check.eq(t.period, 100, "port_to_equal_leaves_period");
      // VIBRATO high-limit clamp (:420-428)
      t = TModSlayer.mti(8, 0, 855);
      t.vib_rate = 1;
      t.vib_depth = 128;
      t.period_low_limit = 113;
      t.period_high_limit = 856;
      t.finetune_rate = 8000;
      slb.beattrack(t);
      Check.eq(t.period, 855, "vibrato_period_untouched");
      Check.eq(t.pitch, 8000 / (855 + ModSlayer.sintable[1]), "vibrato_under_high_limit");

      // ---- updatetracks pattern-jump (DEAD) / nloops arms ----
      // positions is byte[] (Mod.java:18), so a 0xFF byte reads as -1;
      // `positions[order_pos] == 255` (:536) can never be true and the
      // jump/phrase-reset body :537-539 is dead original code. The -1
      // falls through and indexes patterns[-1] at :541 (original bug).
      final Mod mJump = TModSlayer.fixture(app, 4, false);
      mJump.positions[1] = (byte) 255;
      final ModSlayer sJump = new ModSlayer(mJump, 8000, 350, 125);
      sJump.startplaying(false);
      sJump.order_pos = 1;
      Check.throwsIt(new Check.Task() {
         public void run() {
            sJump.updatetracks();
         }
      }, ArrayIndexOutOfBoundsException.class, "jump_0xff_indexes_minus1_aioobe");

      // nloops 2 -> one end-of-song wrap leaves mod_done false (:526-531)
      Mod mTwo = TModSlayer.fixture(app, 4, false);
      mTwo.song_length_patterns = 0;
      ModSlayer sTwo = new ModSlayer(mTwo, 8000, 350, 125);
      sTwo.startplaying(false);
      sTwo.nloops = 2;
      sTwo.updatetracks();
      Check.eq(sTwo.nloops, 1, "nloops_decrements_to_1");
      Check.ok(!sTwo.mod_done, "nloops_two_not_done");

      // ---- turnbytes oversampling (ModSlayer.java:576-596) ----
      // oversample==2 -> pairwise average; oversample==3 -> integer mean
      Mod mOs = TModSlayer.fixture(app, 4, false);
      ModSlayer sOs = new ModSlayer(mOs, 8000, 350, 125);
      sOs.oversample = 2;
      Check.eq(sOs.turnbytes().length, 123840, "oversample2_len_123840");
      Mod mOs3 = TModSlayer.fixture(app, 4, false);
      ModSlayer sOs3 = new ModSlayer(mOs3, 8000, 350, 125);
      sOs3.oversample = 3;
      Check.eq(sOs3.turnbytes().length, 123840, "oversample3_len_123840");

      // loud startplaying arm (:777) - loud_vol_adj instead of normal
      Mod mLoud = TModSlayer.fixture(app, 4, false);
      ModSlayer sLoud = new ModSlayer(mLoud, 8000, 350, 125);
      sLoud.startplaying(true);
      Check.eq(sLoud.vol_adj, ModSlayer.loud_vol_adj, "startplaying_loud_vol_adj");

      // ---- turnbytes 3.5M cap (:598-603) ----
      // a long silent song with bpm 60 at 44.1k yields 1837 samples per
      // tick; 8 patterns x 64 rows x 6 ticks = 3072 ticks > 3.5M samples,
      // so var6 stops at the cap and the output length is exactly 3500000.
      Mod mLong = TModSlayer.fixture(app, 4, false);
      for (int i = 0; i < 128; i++) {
         mLong.positions[i] = 0;
      }
      mLong.song_length_patterns = 8;
      mLong.song_repeat_patterns = 8;
      mLong.patterns = new byte[64][4 * 4 * 64];
      ModSlayer sLong = new ModSlayer(mLong, 44100, 350, 60);
      byte[] capped = sLong.turnbytes();
      Check.eq(capped.length, KM.intv("turnbytes_cap"), "turnbytes_caps_at_3500000");
      Check.eq(sLong.mod_done, true, "turnbytes_long_song_done");
   }

   /** Runs one get_track row with the given effect/param on track 0. */
   private static ModTrackInfo row(ModSlayer s, int sample, int period, int effect, int param) {
      return row2(s, null, sample, period, effect, param);
   }

   /** Same, but starting from a caller-supplied track (oldsampofs etc). */
   private static ModTrackInfo row2(ModSlayer s, ModTrackInfo prev, int sample, int period, int effect, int param) {
      byte[] patt = new byte[16];
      patt[0] = (byte) ((sample & 0xF0) | ((period >> 8) & 0x0F));
      patt[1] = (byte) (period & 0xFF);
      patt[2] = (byte) (((sample & 0x0F) << 4) | (effect & 0x0F));
      patt[3] = (byte) param;
      s.patt = patt;
      s.pattofs = 0;
      s.row = 0;
      if (prev != null) {
         s.tracks[0] = prev;
      } else {
         s.tracks[0] = new ModTrackInfo();
      }
      s.updatetracks();
      return s.tracks[0];
   }

   /** row() but the note field is the period (sample 1 = insts[0]). */
   private static ModTrackInfo rowPeriod(ModSlayer s, int sample, int period, int effect, int param) {
      return row(s, sample, period, effect, param);
   }

   /** A ready ModSlayer with MOD limits installed on instrument 0. */
   private static ModSlayer slayer(TestApplet app, int numtracks) throws Exception {
      Mod m = TModSlayer.fixture(app, numtracks, false);
      ModSlayer s = new ModSlayer(m, 8000, 350, 125);
      s.startplaying(false);
      return s;
   }

   private static int key(String fourcc) {
      return fourcc.charAt(0) * 1000 + fourcc.charAt(1) * 100 + fourcc.charAt(2) * 10 + fourcc.charAt(3);
   }

   /** 128-byte position table with the listed slots set. */
   private static byte[] positions(int... slots) {
      byte[] p = new byte[128];
      for (int i = 0; i < slots.length; i++) {
         p[i] = (byte) slots[i];
      }
      return p;
   }

   /** n identical instrument descriptors. */
   private static Inst[] insts(int n, int words, int pointWords, int lengthWords) {
      Inst[] a = new Inst[n];
      for (int i = 0; i < n; i++) {
         a[i] = new Inst(words, pointWords, lengthWords);
      }
      return a;
   }

   /** One instrument descriptor: u16 word counts as stored on disk. */
   private static class Inst {
      final int words;
      final int pointWords;
      final int lengthWords;

      Inst(int words, int pointWords, int lengthWords) {
         this.words = words;
         this.pointWords = pointWords;
         this.lengthWords = lengthWords;
      }
   }

   /**
    * Builds one synthetic MOD. nInstr == insts.length decides the
    * instrument count the parser uses (31 or 15); the magic is always
    * written at the fixed 1080 read offset.
    */
   private static byte[] build(String magic, int songlen, int repeat, byte[] pos,
         int numpat, int numtracks, Inst[] insts, boolean truncate) throws Exception {
      int nInstr = insts.length;
      int seqOff = 20 + nInstr * 30;
      int patternOff = seqOff + 2 + 128 + 4;
      int patternBytes = numpat * numtracks * 4 * 64;
      int[] len = new int[nInstr];
      int sampleBytes = 0;
      for (int i = 0; i < nInstr; i++) {
         len[i] = insts[i].words * 2;
         sampleBytes += len[i];
      }
      int size = patternOff + patternBytes + sampleBytes;
      if (size < 1084) {
         size = 1084;
      }
      byte[] out = new byte[size];
      byte[] name = "edge".getBytes("US-ASCII");
      System.arraycopy(name, 0, out, 0, name.length);
      for (int i = 0; i < nInstr; i++) {
         int o = 20 + i * 30;
         out[o] = (byte) ('a' + i % 26);
         putU16(out, o + 22, insts[i].words);
         out[o + 24] = 0; // finetune
         out[o + 25] = 64; // volume
         putU16(out, o + 26, insts[i].pointWords);
         putU16(out, o + 28, insts[i].lengthWords);
      }
      out[seqOff] = (byte) songlen;
      out[seqOff + 1] = (byte) repeat;
      System.arraycopy(pos, 0, out, seqOff + 2, 128);
      for (int i = 0; i < 4; i++) {
         out[1080 + i] = (byte) magic.charAt(i);
      }
      for (int p = 0; p < numpat; p++) {
         out[patternOff + p * numtracks * 256] = (byte) (p + 1);
      }
      int so = patternOff + patternBytes;
      for (int i = 0; i < sampleBytes; i++) {
         out[so + i] = (byte) ((i % 8) + 1);
      }
      if (truncate && sampleBytes > 0) {
         byte[] cut = new byte[size - sampleBytes / 2];
         System.arraycopy(out, 0, cut, 0, cut.length);
         return cut;
      }
      return out;
   }

   private static void putU16(byte[] b, int off, int v) {
      b[off] = (byte) ((v >> 8) & 0xFF);
      b[off + 1] = (byte) (v & 0xFF);
   }

   /** LoadMod is the only public entrance; it throws IOException. */
   private static Mod load(byte[] bytes) throws Exception {
      Mod m = new Mod("edges.zipo", new TestApplet("assets/music"));
      m.LoadMod(new ByteArrayInputStream(bytes));
      return m;
   }
}
