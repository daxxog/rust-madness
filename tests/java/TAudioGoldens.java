import java.io.BufferedInputStream;
import java.io.InputStream;
import java.security.MessageDigest;
import java.util.List;

/**
 * Scoped AUDIO GOLDENS: the stage-1 fidelity proof (TModSlayer's E2E)
 * extended to ALL ELEVEN stage tracks, at the game's exact per-stage
 * parameters.
 *
 * THE CONTRACT (user-decided, quoted in B13_audio_goldens.md): live-audio
 * golden pins are "bytes before the device, md5, mirrored later through
 * the port's AudioBus". So every pin below is device-free:
 *
 * - DECODE: Mod + ModSlayer at the stage's (gain, rate, bpm) ->
 *   turnbytes() (ModSlayer.java:553-621) -> md5 + length. This is the
 *   exact byte array the original ctor stores in RadicalMod.stream
 *   (RadicalMod.java:41-43), produced by the same direct call TModSlayer's
 *   stage1 E2E uses. No AudioPlayer, no threads, no windows.
 *
 * - MIX: the per-call sequence play() would make the device see. play()
 *   calls AudioPlayer.player.start(stream) on a NON-null stream
 *   (RadicalMod.java:63-76, :70); 8u40's AudioDevice.openChannel wraps
 *   that stream in `new BufferedInputStream(in, 1024)` (bytecode offsets
 *   242-253), fails to sniff a file format, and falls back to
 *   `new AudioFormat(ULAW, 8000.0f, 8, 1, 1, 8000.0f, true)` (offsets
 *   303-318). A real start() would then open a SourceDataLine. We stop at
 *   the BufferedInputStream boundary: 1024-byte read calls, exactly the
 *   buffer size and per-call sample count (8-bit mono ULAW = 1 byte per
 *   sample, so 1024 samples = 128.0 ms at the device's hardcoded 8000 Hz).
 *   That is the "bytes before the device" seam.
 *
 *   SuperStream.read(dst, off, len) loops until it has filled exactly len
 *   bytes, calling reset() on EOF (SuperStream.java:18-31), so a
 *   1024-byte call always returns 1024 and the pass after the track end
 *   wraps mid-chunk. Pin: ceil(len/1024) calls -> ceil(len/1024)*1024
 *   bytes, i.e. ONE buffer-rounded pass over the whole track (the device
 *   itself loops forever; the original never stops it - the wrap is what
 *   makes the music loop).
 *
 * - STRUCTURAL: stream length, 4 tracks / N patterns / 31 instruments,
 *   song_length_patterns, and the startplaying bpm-derived tempo
 *   (ModSlayer.java:784 `bpm_samples = samplingrate / (24*bpm/60)`).
 *   Reused, not duplicated: TModSlayer already pins stage1's length, CRC
 *   and md5 - here they are re-derived through the all-stage table.
 *
 * ARG ORDER (verified in B1/B2, not assumed): xtGraphics passes
 * `new RadicalMod(path, gain, rate, bpm, app)`; RadicalMod.java:39
 * forwards `new ModSlayer(mod, var3, var2, var4)` and ModSlayer.java:505
 * is `(Mod, samplingrate, gain, bpm)`. The per-stage table is stored in
 * tests/fixtures/toml/audio.toml as RADICALMOD ARGS (gain, rate, bpm)
 * plus the decode/mix/structural rows, with the xtGraphics.java:1249-1324
 * line cited per row. stage1's 1191330-byte / f1f2a25e... match is the
 * standing Java==Rust mixer proof.
 *
 * POSITION IN THE SUITE IS IRRELEVANT: these goldens are pure integer
 * byte math over fixed assets (B2's "Nondeterminism: none"). No
 * Math.random, no Clock, no AWT, no AudioPlayer - unlike TJavaRandom's
 * JIT-order caveat (TestRng.java header) and the AudioPlayer class-init,
 * neither is touched here (RadicalMod's ctor only builds Mod/ModSlayer/
 * turnbytes/SuperStream; the ctor never calls start/stop). RadicalMod's
 * System.gc() (RadicalMod.java:49) is the only side effect. Hence
 * Suite.java appends this class last without perturbing any earlier pin.
 *
 * Also the doc-visible quirk: the device plays 8000 Hz ULAW regardless of
 * the render rate, so stage2/stage11 (9000) play ~12.5% fast and the
 * 7900/8100/8400/8500 tracks land between; that mismatch is original
 * behavior, not a test artifact (B13).
 */
public class TAudioGoldens {
   /** The AudioDevice boundary, constants.toml [audio_device] (B13 §1.1):
    *  BufferedInputStream(in, 1024), ULAW fallback at 8000 Hz, so one call
    *  is 1024 samples = 128 ms. */
   static final Toml KAD = Toml.load("tests/fixtures/toml/constants.toml").table("audio_device");
   static final int DEVICE_CALL_BYTES = KAD.intv("frame_bytes");
   static final int DEVICE_RATE = KAD.intv("rate");
   static final int DEVICE_MS_PER_CALL = KAD.intv("ms_per_call");

   /** The fixture is the data source (README: audio.toml owns the 11 stage
    *  rows). One row per stage, stage1..stage11 in order. */
   static final String FIXTURE = "tests/fixtures/toml/audio.toml";
   static final int STAGES = 11;

   /** Every key a stage row must carry (shape pin at load). */
   static final String[] STAGE_KEYS = {
      "id", "path", "gain", "rate", "bpm",
      "decode_bytes", "decode_md5", "mix_calls", "mix_samples", "mix_md5",
      "num_tracks", "num_patterns", "song_length_patterns",
      "song_repeat_patterns", "instruments", "bpm_samples"
   };

   static void run() throws Exception {
      Check.group("AudioGolden");
      KAD.requireKeys("rate", "frame_bytes", "ms_per_call");
      Check.eq(DEVICE_CALL_BYTES * 1000 / DEVICE_RATE, DEVICE_MS_PER_CALL,
            "audio_device_ms_per_call_128");
      Toml audio = Toml.load(FIXTURE);
      // shape pins: the table has one row per stage, in stage order, and
      // every row carries the full key set (requireKeys throws loudly)
      Check.eq(audio.tableCount("stage"), STAGES, "audio_stage_rows_11");
      List<Toml> rows = audio.tables("stage");
      StringBuilder ids = new StringBuilder();
      for (int i = 0; i < rows.size(); i++) {
         Toml s = rows.get(i);
         s.requireKeys(STAGE_KEYS);
         if (i > 0) {
            ids.append(',');
         }
         ids.append(s.str("id"));
      }
      Check.eq(ids.toString(), "stage1,stage2,stage3,stage4,stage5,stage6,"
            + "stage7,stage8,stage9,stage10,stage11", "audio_stage_ids_in_order");

      TestApplet music = new TestApplet("assets/music");
      TestApplet assets = new TestApplet("assets");
      for (int i = 0; i < rows.size(); i++) {
         stage(music, assets, i, rows.get(i));
      }
   }

   /** One stage: decode golden + structural pins + device-free mix golden.
    *  Every expected value is the TOML row's (audio.toml); the pin ids and
    *  their meanings are unchanged since the transcription. */
   static void stage(TestApplet music, TestApplet assets, int i, Toml s) throws Exception {
      String track = s.str("id");
      String path = s.str("path");
      int gain = s.intv("gain");
      int rate = s.intv("rate");
      int bpm = s.intv("bpm");
      int decodeBytes = s.intv("decode_bytes");

      // ---- decode golden (ModSlayer.java:553) ----
      Mod mod = new Mod(track + ".zipo", music);
      Check.eq(mod.numtracks, s.intv("num_tracks"), track + "_tracks");
      Check.eq(mod.getNumPatterns(), s.intv("num_patterns"), track + "_numpatterns");
      Check.eq(mod.patterns.length, s.intv("num_patterns"), track + "_pattern_rows");
      Check.eq(mod.song_length_patterns, s.intv("song_length_patterns"), track + "_song_length");
      Check.eq(mod.song_repeat_patterns, s.intv("song_repeat_patterns"), track + "_song_repeat");
      Check.eq(mod.insts.length, s.intv("instruments"), track + "_instruments");

      ModSlayer slayer = new ModSlayer(mod, rate, gain, bpm);
      // capture the tempo before the mixer runs: Fxx effects may retune
      // bpm_samples mid-song (get_track case 15, ModSlayer.java:759-769)
      slayer.startplaying(false);
      Check.eq(slayer.bpm_samples, s.intv("bpm_samples"), track + "_bpm_samples");
      Check.eq(slayer.numtracks, s.intv("num_tracks"), track + "_slayer_tracks");

      byte[] decoded = slayer.turnbytes();
      Check.eq(decoded.length, decodeBytes, track + "_decode_len");
      String decodeMd5 = md5(decoded);
      Check.eq(decodeMd5, s.str("decode_md5"), track + "_decode_md5");

      // ---- the game's ctor path (RadicalMod.java:37-50) ----
      // proves the (gain, rate) order again: the reversed order would
      // make turnbytes throw and leave stream == null (TRadicalMod pin)
      RadicalMod rm = new RadicalMod(path, gain, rate, bpm, assets);
      Check.ok(rm.stream != null, track + "_stream_not_null");
      Check.eq(rm.playing, false, track + "_ctor_playing_false");
      // available() BEFORE any read: the full turnbytes() array
      // (SuperStream over ByteArrayInputStream, SuperStream.java:4-6)
      Check.eq(rm.stream.available(), decodeBytes, track + "_stream_available");

      // ---- mix golden: the device-free per-call pump ----
      int calls = s.intv("mix_calls");
      byte[] mixed = pump(rm.stream, calls);
      int mixLen = s.intv("mix_samples");
      Check.eq(mixed.length, mixLen, track + "_mix_samples");
      Check.eq(mixLen, (decodeBytes + DEVICE_CALL_BYTES - 1) / DEVICE_CALL_BYTES * DEVICE_CALL_BYTES,
         track + "_mix_len_is_ceiling");
      Check.eq(md5(mixed), s.str("mix_md5"), track + "_mix_md5");
      // the pump's first DECODE_LEN bytes are the whole decoded track:
      // a second independent turnbytes() run (inside RadicalMod) is
      // byte-identical, and chunking only appends the wrap-in head
      Check.ok(samePrefix(mixed, decoded, decodeBytes), track + "_mix_prefix_matches_decode");

      if (i == 0) {
         // THE standing Java==Rust fidelity proof (B13 contract)
         Check.eq(decodeMd5, "f1f2a25eb468517faa2218cf1712395f", "stage1_matches_rust_port_digest");
      }
   }

   /** Reads `calls` device-sized frames. Every call must return exactly
    *  DEVICE_CALL_BYTES (SuperStream.java:18-31 fills to len, resetting on
    *  EOF), so the concatenation is one buffer-rounded pass. */
   static byte[] pump(InputStream stream, int calls) throws Exception {
      InputStream in = new BufferedInputStream(stream, DEVICE_CALL_BYTES);
      byte[] out = new byte[calls * DEVICE_CALL_BYTES];
      byte[] one = new byte[DEVICE_CALL_BYTES];
      for (int c = 0; c < calls; c++) {
         int n = in.read(one, 0, DEVICE_CALL_BYTES);
         if (n != DEVICE_CALL_BYTES) {
            // cannot happen with SuperStream; keep the golden honest
            throw new IllegalStateException("short device read " + n + " at call " + c);
         }
         System.arraycopy(one, 0, out, c * DEVICE_CALL_BYTES, DEVICE_CALL_BYTES);
      }
      return out;
   }

   static boolean samePrefix(byte[] a, byte[] b, int n) {
      if (a.length < n || b.length < n) {
         return false;
      }
      for (int i = 0; i < n; i++) {
         if (a[i] != b[i]) {
            return false;
         }
      }
      return true;
   }

   static String md5(byte[] bytes) throws Exception {
      MessageDigest md5 = MessageDigest.getInstance("MD5");
      md5.update(bytes);
      StringBuilder hex = new StringBuilder();
      for (byte b : md5.digest()) {
         hex.append(String.format("%02x", b));
      }
      return hex.toString();
   }
}
