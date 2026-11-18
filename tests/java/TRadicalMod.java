/**
 * Pins RadicalMod, the music-file loader (B1_audio_tiny.md "RadicalMod"):
 * the ctor's Mod -> ModSlayer -> turnbytes -> SuperStream chain
 * (RadicalMod.java:37-50), its failure path (stream==null,
 * "Error Loading Mod"), the stop() playing guard (:8-17), outwithit's
 * null-guarded close (:19-35), and the null-stream transitions of
 * stop/resume/play/posit.
 *
 * ARG ORDER (verified against the source, not assumed): xtGraphics.java:1250
 * calls `new RadicalMod("music/stage1.zipo", 350, 8400, 135, this.app)` and
 * RadicalMod.java:39 forwards `new ModSlayer(var6, var3, var2, var4)`, whose
 * signature is `ModSlayer(Mod, samplingrate, gain, bpm)`
 * (ModSlayer.java:505-512). So RadicalMod's 2nd arg is the ModSlayer GAIN
 * (350) and its 3rd arg is the SAMPLING RATE (8400) - the reverse of what
 * B1/B10 prose implies for stage1 ("gain=8400, rate=350"). The probe proved
 * the order is observable: swapping them makes turnbytes() fail with
 * ArrayIndexOutOfBoundsException: 44 and the ctor falls into its catch.
 * The path resolves through Applet.getCodeBase() (Mod.java:42), so the seam
 * is TestApplet("assets") (same base the applet tag supplies).
 *
 * AUDIO VERDICT (probed on the gate's Oracle 8u40, see the audioNullStream
 * comment): touching sun.audio.AudioPlayer.player IS safe here - its
 * class-init starts ONE JDK-internal daemon thread ("Audio Player", prio
 * 10) whose run() loops AudioDevice.play() (a no-op: DEBUG check then
 * return) + sleep(5000), and AudioDevice.open()/close() are likewise
 * no-ops. No mixer/line is ever opened - AudioDevice.device.openChannels()
 * stayed 0 through every null-stream call - so no device I/O occurs. The
 * ctor's System.gc() (:49) is fine. Nothing below ever hands a NON-null
 * stream to start()/stop() (that is the only path that would touch audio),
 * and no method is called while a real stream is playing.
 */
public class TRadicalMod {
   static void run() throws Exception {
      Check.group("RadicalMod");
      TestApplet app = new TestApplet("assets");
      RadicalMod rm = ctorSuccess(app);
      stopGuard(rm);
      outwithitPaths(rm);
      failurePaths(app);
      argOrder(app);
   }

   /** RadicalMod.java:37-50 with the game's stage1 parameters. The ctor
    *  builds Mod + ModSlayer, calls turnbytes(), wraps the bytes in a
    *  SuperStream and leaves playing at its field default (false, :6). */
   private static RadicalMod ctorSuccess(TestApplet app) {
      RadicalMod rm = new RadicalMod("music/stage1.zipo", 350, 8400, 135, app);
      Check.ok(rm.stream != null, "ctor_stream_not_null");
      Check.eq(rm.stream.getClass().getName(), "SuperStream", "ctor_stream_superstream");
      Check.eq(rm.playing, false, "ctor_playing_false");
      // SuperStream delegates to ByteArrayInputStream over the turnbytes()
      // array (SuperStream.java:4-6), so available() is the FULL byte count;
      // 1191330 is TModSlayer's independent stage1_len pin, which proves the
      // gain/rate order reaches the mixer as samplingrate=8400, gain=350.
      Check.eq(rm.stream.available(), 1191330, "ctor_stream_available_1191330");
      // RadicalMod.java:78-80 is a bare `return this.stream.available();`
      Check.eq(rm.posit(), 1191330, "ctor_posit_1191330");
      return rm;
   }

   /** RadicalMod.java:8-17 - stop() is guarded by `if (this.playing)`;
    *  with playing==false it touches neither AudioPlayer nor the stream. */
   private static void stopGuard(RadicalMod rm) {
      int before = rm.posit();
      rm.stop();
      Check.eq(rm.playing, false, "stop_not_playing_stays_false");
      Check.eq(rm.posit(), before, "stop_not_playing_posit_unchanged");
      // calling it twice is equally inert
      rm.stop();
      Check.eq(rm.playing, false, "stop_twice_stays_false");
   }

   /** RadicalMod.java:19-35 - outwithit() with playing==false skips the
    *  AudioPlayer branch, then closes the stream only if non-null (:29-31).
    *  ByteArrayInputStream.close() is a no-op, so posit() still works and
    *  still reports the whole buffer. */
   private static void outwithitPaths(RadicalMod rm) {
      rm.outwithit();
      Check.eq(rm.playing, false, "outwithit_not_playing_stays_false");
      Check.ok(rm.stream != null, "outwithit_keeps_stream_reference");
      Check.eq(rm.posit(), 1191330, "outwithit_posit_after_close_1191330");
      // idempotent: the second call closes an already-closed stream
      rm.outwithit();
      Check.eq(rm.playing, false, "outwithit_twice_stays_false");
      Check.eq(rm.posit(), 1191330, "outwithit_twice_posit_unchanged");
   }

   /** A missing file makes Mod's own ctor print "Mod: <path> <exception>"
    *  and leave insts==null (Mod.java:40-62); ModSlayer.turnbytes() then
    *  throws NullPointerException at ModSlayer.java:800, which
    *  RadicalMod.java:44-46 catches: stream=null, playing stays false, and
    *  it prints "Error Loading Mod: java.lang.NullPointerException". */
   private static void failurePaths(TestApplet app) {
      RadicalMod bad = new RadicalMod("music/nope.zipo", 350, 8400, 135, app);
      Check.eq(bad.stream, null, "fail_stream_null");
      Check.eq(bad.playing, false, "fail_playing_false");

      // RadicalMod.java:78-80 has NO null guard: posit() dereferences the
      // null stream and throws (B1 said "returns 0" - wrong, see B1 fix)
      final RadicalMod fbad = bad;
      Check.throwsIt(new Check.Task() {
         public void run() {
            fbad.posit();
         }
      }, NullPointerException.class, "fail_posit_npe");

      // :29-31 null-guards the close, so outwithit() on the failed instance
      // calls NOTHING (no exception at all) and leaves playing false
      bad.outwithit();
      Check.eq(bad.stream, null, "fail_outwithit_stream_stays_null");
      Check.eq(bad.playing, false, "fail_outwithit_playing_stays_false");

      // stop() with playing==false again: the guard wins over the null stream
      bad.stop();
      Check.eq(bad.playing, false, "fail_stop_not_playing");

      // ---- null-stream AudioPlayer paths (:8-17, :52-76) ----
      // The playing field is package-private (:6), so the test can set it.
      // 8u40 bytecode facts (javap): AudioDevice.closeChannel(null) returns
      // at its `if (in == null) return` and openChannel(null) fails to parse
      // its null-backed BufferedInputStream and also returns; both are
      // swallowed-or-not silently. AudioDevice.open()/close()/play() are
      // no-ops. So the calls below neither throw nor open a line; what they
      // do prove is that the playing writes at :15/:59/:74 happen
      // unconditionally outside the try blocks.
      bad.playing = true;
      bad.stop();
      Check.eq(bad.playing, false, "audio_stop_null_stream_resets_playing");
      bad.stop();
      Check.eq(bad.playing, false, "audio_stop_guard_when_already_false");

      bad.playing = false;
      bad.resume();
      Check.eq(bad.playing, true, "audio_resume_null_stream_sets_playing");
      bad.resume();
      Check.eq(bad.playing, true, "audio_resume_guard_when_playing");

      bad.stop();
      Check.eq(bad.playing, false, "audio_stop_after_resume_resets");
      bad.play();
      Check.eq(bad.playing, true, "audio_play_null_stream_sets_playing");
      bad.play();
      Check.eq(bad.playing, true, "audio_play_guard_when_playing");

      // outwithit while playing==true on the null-stream instance: the
      // playing branch runs (:20-27), the close is skipped (:30), no throw
      bad.outwithit();
      Check.eq(bad.playing, false, "audio_outwithit_playing_null_stream");
      Check.eq(bad.stream, null, "audio_outwithit_stream_stays_null");
   }

   /** Proves the ctor's int order: the game's (gain=350, rate=8400) order
    *  yields a stream; the reversed (8400, 350) order makes the mixer read
    *  out of bounds and the ctor catches it (RadicalMod.java:41-47). */
   private static void argOrder(TestApplet app) {
      RadicalMod swapped = new RadicalMod("music/stage1.zipo", 8400, 350, 135, app);
      Check.eq(swapped.stream, null, "argorder_swapped_stream_null");
      Check.eq(swapped.playing, false, "argorder_swapped_playing_false");
   }
}
