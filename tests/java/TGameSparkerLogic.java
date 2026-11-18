import java.applet.AppletContext;
import java.applet.AudioClip;
import java.awt.Color;
import java.awt.Font;
import java.awt.FontMetrics;
import java.awt.Graphics;
import java.awt.Image;
import java.awt.Rectangle;
import java.awt.Shape;
import java.awt.Toolkit;
import java.awt.image.BufferedImage;
import java.awt.image.ImageObserver;
import java.awt.image.MemoryImageSource;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.Iterator;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;

/**
 * Pins GameSparker, the applet shell: the fase dispatcher, the two parsers,
 * the cookie clamps, the input machines, loadbase/loadstage/loadmusic and
 * the boot walk the Rust port reproduces as its integration crown. Source of
 * truth is reference/vineflower/GameSparker.java (1578 lines) plus the
 * xtGraphics handlers the dispatcher calls; every expectation was re-derived
 * from the source and then measured against the unmodified original on the
 * gate's Oracle 8u40.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B8_gamesparker.md. B8 rows the source
 * contradicts are corrected here (and reported back into B8):
 *   - pin 2: init() calls createImage(550,400) on an Applet that was never
 *     realized; on 8u40/macOS that returns NULL, so offImage and rd both
 *     stay null (GameSparker.java:1504-1508). B8's "offImage!=null" is a
 *     browser-peered assumption. addNotify() is not a usable workaround:
 *     Panel.addNotify() -> LWToolkit.createPanel -> LWComponentPeer
 *     .getPlatformWindow throws NullPointerException on a target-less
 *     applet (probed). The pumps therefore hand init() a createImage
 *     override (a BufferedImage-backed image) and the direct-call tests
 *     assign offImage/rd directly (both are package-private fields).
 *   - pin 6 is REVERSED in B8: the guard is
 *     `if (this.exwist && this.gamer != null)` (GameSparker.java:93), so
 *     the System.gc()/gamer.stop()/gamer=null block runs when exwist is
 *     TRUE and is skipped when exwist is false (pin 5's second half is the
 *     correct half). run() sets exwist=false at :644, which is why stop()
 *     on a LIVE game thread only raises the exit flag and leaves the
 *     reference alone.
 *   - pin 7: a bare Applet has no AppletStub, so Applet.getAppletContext()
 *     THROWS NullPointerException (Applet.java:204) - it never returns
 *     null. The sunytyp()==0 row needs a subclass whose override returns
 *     null (then ""+null == "null" fails the "sun." test).
 *   - pins 15/20: loadmusic's `var2==0 -> fase 6 / else fase 176` tail is
 *     xtGraphics.java:1326-1335, not :2514-2524 (those are stageselect's
 *     fase=5 / fase=2 writes).
 *   - pins 45/46: the "trackers tail" (trx/trz/ptr/ptcnt/hit/nrnd/trk) are
 *     Medium fields (Medium.java:73-80); Trackers.java has none of them.
 *   - pin 81: the denial path's NPE is at GameSparker.java:87
 *     (`this.gamer.stop()` with gamer==null). A null AppletContext makes
 *     `getAppletContext().showDocument(...)` throw at :83, but that sits in
 *     the inner empty catch (:81-85), so the observable throw is still :87.
 *   - pin 20's trimmed-fixture premise does not hold: with music/ absent,
 *     this.stages is a RadicalMod whose stream is null and the fase-1 arm's
 *     trackbg() calls stages.posit() = `stream.available()`
 *     (RadicalMod.java:78-80), which NPEs at xtGraphics.java:2480 - the
 *     walk dies before stage select. With music present those play() calls
 *     are NOT device-free either: probed
 *     `AudioDevice.device.openChannels()` goes 0 -> 1 after stage1.play().
 *     So the crown walk runs on the REAL assets with the RadicalMod
 *     `playing` flags pre-set (play() then returns at its `if
 *     (this.playing)` guard; stop() only walks AudioDevice.closeChannel's
 *     lookup table) and every AudioClip routed to a silent stub. The
 *     music-MISSING loader failure and the 176 play-skip are pinned in a
 *     separate graphics-only run.
 *   - pins 41/42 plus one extra behavior: every real models.zipo entry is
 *     matched by prefix and the LAST match wins (GameSparker.java:220-228),
 *     so roadend.rad lands at 23, offroadend.rad at 24, and all 43 slots
 *     end up non-null; an entry matching NO prefix lands at index 0 because
 *     var10 starts at 0 (:221).
 *   - pin 44: a missing stage does not "return normally" - the catch arm
 *     sets fase=3 and skips the promotion, but the tail still runs
 *     resetstat(...) -> sortcars(), which sets sc[4] = 4+(stage+1)/2
 *     (xtGraphics.java:2621-2652), so stage>=11 throws
 *     ArrayIndexOutOfBoundsException (models[54] for stage 99, Madness
 *     .reseto for stage 12). The clean form is a codebase that HAS
 *     graphics/ and no stages/ (graphics-only fixture): stage 1 is
 *     missing, sortcars keeps sc in 0..5, and loadstage returns with
 *     fase=3 - which is what loadstageMissingPins drives.
 *   - pin 16's countdown: the pass that runs loadmusic also runs the 176
 *     arm (the arm sits later in the same linear chain), so loadhold 80
 *     spends one tick there and then needs exactly 80 more passes - 79
 *     leaving var13 at 0 and the 80th flipping to fase 6.
 *   - pin 21's walk: opselect 1 -> 0 needs TWO keyDown(1005) presses
 *     (maini wraps 2 -> 0 at :3175-3182), and inst only needs THREE enters
 *     in fase 11 because the flipo 2 and flipo 4 blocks both advance in the
 *     same inst() call as the enter block (:1428-1444 inst's own flipo/enter
 *     ladder): 1->2, 2->3->4, 4->5->(fase=oldfase).
 *
 * DEVICE MAP (probed): with the silent AudioClip stub the only remaining
 * audio path is RadicalMod.play()/stop() -> sun.audio. Two device-free
 * facts are pinned: (a) loadmusic with loadhold!=0 sets fase=176 and never
 * calls play() (xtGraphics.java:1326-1335; playing stays false), and (b)
 * the 176 countdown's play() is guarded by loadedt (GameSparker.java:
 * 917-919) - on the graphics-only codebase Mod/ModSlayer fail on the
 * missing music file, RadicalMod's ctor swallows it (RadicalMod.java:37-50),
 * loadedt stays false and the countdown reaches fase 6 with nothing played.
 * The var2==0 -> play() branch of loadmusic is deliberately NOT driven
 * (device-gated), per the standing audio policy.
 *
 * THREADS (this class only): every run() pump is a real Thread started by
 * start(), parked deterministically inside the applet's own repaint()
 * override (run() calls this.repaint() exactly once per pass at
 * GameSparker.java:1404, so the override is a per-pass park point) and
 * always torn down by the same method that started it (exwist=true, gate
 * released, interrupt, join(10000)). No thread survives a test method; no
 * pool exists. Nothing ever becomes visible: no setVisible/show/pack, no
 * Frame. The loop's own Thread.sleep (:1492-1500) is short-circuited with
 * Thread.interrupt() while a pump waits for its next park - the catch at
 * :1497-1500 swallows it, so no state differs, only wall time.
 *
 * FIXTURES (built once under target/java-tests-fixtures/):
 *   - assets/                 the shipped tree (real music included)
 *   - gfxonly/                assets/graphics only (no stages/, no music/)
 *   - empty/                  nothing at all (loadbase failure arm)
 *   - lastmatch/graphics/models.zipo  a two-entry synthetic zip
 *
 * ENV-UNREACHABLE (documented, not burned time on): pin 25 (a runtime
 * without plugin.jar - the gate always carries it), pin 26's
 * JSObject-works sub-cases and pins 28/29/31/32's in-range cookie arms (all
 * need a live browser JSObject), sunytyp's java.version 1.3 arm, and
 * loadmusic's var2==0 play branch (device-gated).
 *
 * STRUCTURALLY DEAD for these inputs (measured with JaCoCo; 84.9% of
 * GameSparker's instructions are covered, the rest is mostly this list):
 * :30-55 (testlocation's FIRST try body needs a working
 * JSObject.getWindow, impossible outside a browser), :86-89/:154-158/
 * :1513-1516 (the same, for the second denial draw and both cookie
 * bodies), :587 (var2=15 needs sunytyp()!=0 inside run()), :612-630 (the
 * in-range cookie clamps), :1137-1153/:1247-1256 (the replay fix ladder
 * only fires on record.hfix / record.fix frames from a real race),
 * :1172-1182 (the hcaught camera needs record.hcaught, set only by a real
 * collision), :1266-1307 (the wasted-replay counters need record.wasted /
 * whenwasted from a real race), :1489 (gamer=null after Thread.stop - the
 * ThreadDeath raised at :1488 means the next statement never runs, which is
 * itself the pin), and the `cp.stage != unlocked` halves of :1327/:1334
 * (a boot always has stage==unlocked==1).
 */
public class TGameSparkerLogic {
   /** constants.toml - the xt camera ladder and the GameSparker gates. */
   static final Toml CONST = Toml.load("tests/fixtures/toml/constants.toml");
   static final Toml KXT = CONST.table("xt");
   static final Toml KGP = CONST.table("gamesparker");

   // =====================================================================
   // entry
   // =====================================================================

   static void run() throws Throwable {
      Check.group("GameSparkerLogic");
      KXT.requireKeys("camera_adv_start", "camera_adv_landing", "starcnt_start");
      KGP.requireKeys("playsounds_starcnt_gate", "loadhold", "factor_default");
      ctorDefaults();
      parserTable();
      cookiePins();
      cookieLatchScenario();
      sunytypPins();
      initPins();
      runWithoutInit();
      lifecyclePins();
      inputPins();
      loadbasePins();
      loadbaseLastMatch();
      loadbaseMissing();
      loadstagePins();
      loadstageNinePins();
      loadstageNoPromotion();
      loadstageMissingPins();
      loadstageErrorPins();
      testlocationPins();
      paintPins();
      loadmusicDirectPins();
      dispatcherMatrix();
      crownWalk();
      musicMissingLoader();
   }

   // =====================================================================
   // 1 - the ctor and the declared field initializers
   // =====================================================================

   /** B8(1) / GameSparker.java:15-26 - no ctor is declared, so only the
    *  field initializers run; no thread, no asset, no peer. */
   private static void ctorDefaults() throws Exception {
      Seam app = new Seam("assets", null, false);
      Check.ok(app.rd == null, "ctor_rd_null");
      Check.ok(app.offImage == null, "ctor_offimage_null");
      Check.ok(app.gamer == null, "ctor_gamer_null");
      Check.eq(app.u.length, 5, "ctor_u_length_5");
      Check.ok(app.u[0] == null && app.u[4] == null, "ctor_u_entries_null");
      Check.eq(app.mouses, 0, "ctor_mouses_0");
      Check.eq(app.xm, 0, "ctor_xm_0");
      Check.eq(app.ym, 0, "ctor_ym_0");
      Check.ok(!app.lostfcs, "ctor_lostfcs_false");
      Check.ok(app.exwist, "ctor_exwist_true");
      Check.eq(app.nob, 0, "ctor_nob_0");
      Check.eq(app.notb, 0, "ctor_notb_0");
      Check.eq(app.view, 0, "ctor_view_0");
      // zero statics in the original source. JaCoCo plants a synthetic
      // static $jacocoData field when the gate runs with --coverage, so
      // synthetic/JaCoCo members are not part of the count.
      int statics = 0;
      java.lang.reflect.Field[] fields = GameSparker.class.getDeclaredFields();
      for (int i = 0; i < fields.length; i++) {
         String name = fields[i].getName();
         if (java.lang.reflect.Modifier.isStatic(fields[i].getModifiers()) && !fields[i].isSynthetic()
               && !name.startsWith("$jacoco")) {
            statics++;
         }
      }
      Check.eq(statics, 0, "ctor_zero_static_fields");
   }

   // =====================================================================
   // 2 - the string/number tokenizer
   // =====================================================================

   /** B8(33-39,77) / GameSparker.java:112-148 - both scanners start at
    *  key.length()+1, so the delimiter right after the key is always eaten;
    *  a token starts after every ","/")" (the index then advances past it)
    *  and runs to the next delimiter or the end. */
   private static void parserTable() throws Exception {
      final Seam app = new Seam("assets", null, false);
      String snap = "snap 1,2,3)";
      Check.eq(app.getstring("snap", snap, 0), "1", "getstring_snap_0_is_1");
      Check.eq(app.getstring("snap", snap, 1), "2", "getstring_snap_1_is_2");
      Check.eq(app.getstring("snap", snap, 2), "3", "getstring_snap_2_is_3");
      // index 3: the trailing ')' bumps var5 to 3 and var4 to len, and the
      // append at :124 runs BEFORE the loop condition is re-tested, so the
      // tokenizer overruns the string instead of returning "" (B8(34) fix)
      Check.throwsIt(new Check.Task() {
         public void run() {
            app.getstring("snap", snap, 3);
         }
      }, StringIndexOutOfBoundsException.class, "getstring_snap_3_overruns_string");
      // without a trailing delimiter the same index is simply empty
      Check.eq(app.getstring("snap", "snap 1,2,3", 3), "", "getstring_no_trailing_delimiter_empty");
      // the '(' after the key is skipped by the same +1 (:116)
      Check.eq(app.getstring("name", "name(The Introductory Stage)", 0), "The Introductory Stage",
            "getstring_name_skips_paren");
      // '|' is NOT special to the parser - only loadstage replaces it (:437)
      Check.eq(app.getstring("name", "name(A|B)", 0), "A|B", "getstring_pipe_kept");
      // a 1-char key starts at 2, so the char at index 1 ('b') is eaten and
      // index 0 of "ab,cd)" therefore never sees a delimiter
      Check.eq(app.getstring("a", "ab,cd)", 0), "", "getstring_shortkey_0_empty");
      Check.eq(app.getstring("a", "ab,cd)", 1), "cd", "getstring_shortkey_1_cd");

      String ints = "snap(-5,-5,20)";
      Check.eq(app.getint("snap", ints, 0), -5, "getint_snap_0_minus5");
      Check.eq(app.getint("snap", ints, 1), -5, "getint_snap_1_minus5");
      Check.eq(app.getint("snap", ints, 2), 20, "getint_snap_2_20");
      Check.eq(app.getint("snap", "snap(1,2)", 1), 2, "getint_snap_index1_2");

      // a non-numeric token dies in Integer.valueOf (:147)
      Check.throwsIt(new Check.Task() {
         public void run() {
            app.getint("snap", "snap(a,1,2)", 0);
         }
      }, NumberFormatException.class, "getint_non_numeric_nfe");
      // an index no token reaches hands Integer.valueOf("") the empty string
      Check.throwsIt(new Check.Task() {
         public void run() {
            app.getint("snap", "snap(1,2)", 9);
         }
      }, NumberFormatException.class, "getint_out_of_range_nfe");
      // the null key dies at var1.length() (:116 / :135)
      Check.throwsIt(new Check.Task() {
         public void run() {
            app.getstring(null, snap, 0);
         }
      }, NullPointerException.class, "getstring_null_key_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            app.getint(null, snap, 0);
         }
      }, NullPointerException.class, "getint_null_key_npe");
   }

   // =====================================================================
   // 3 - cookies without a browser
   // =====================================================================

   /** B8(24,26-partial,27,30,31,32) / GameSparker.java:150-161,1511-1517.
    *  With no live JSObject every read falls into the empty catch and keeps
    *  the -1 initializer; a write is swallowed whole. */
   private static void cookiePins() throws Exception {
      Direct d = directAssets();
      Seam app = d.app;
      Check.eq(app.readcookie("unlocked"), -1, "readcookie_no_browser_minus1");
      Check.eq(app.readcookie("usercar"), -1, "readcookie_usercar_minus1");
      Check.eq(app.readcookie("gameprfact"), -1, "readcookie_gameprfact_minus1");
      Check.eq(app.readcookie("any"), -1, "readcookie_any_minus1");
      // savecookie swallows its Exception and writes nothing (:1511-1517)
      app.savecookie("unlocked", "2");
      Check.eq(app.readcookie("unlocked"), -1, "savecookie_then_read_still_minus1");

      // B8(30): -1 is outside 1..11, so the three boot clamps write
      // nothing - xtGraphics defaults survive and CheckPoints.stage stays 1
      Check.eq(d.xt.unlocked, 1, "cookie_absent_unlocked_stays_1");
      Check.eq(d.xt.opselect, 1, "cookie_absent_opselect_stays_1");
      Check.eq(d.cp.stage, 1, "cookie_absent_stage_stays_1");
      Check.eq(d.xt.sc[0], 0, "cookie_absent_sc0_stays_0");
      // B8(32)'s ==-1 arm: factor 35 / loadhold 80 are run() locals, so the
      // observable contract is the -1 that keeps both writes away (:627-631)
      Check.eq(app.readcookie("gameprfact"), -1, "cookie_absent_loadhold_arm_minus1");
      // and readcookie works before init() (it touches only this+JSObject)
      Check.ok(app.rd != null, "cookie_direct_graph_has_rd");
   }

   // =====================================================================
   // 3b - the var37 cookie latch (fase 5 arms, fase -5 fires)
   // =====================================================================

   /** T1 / GameSparker.java:726-741,771-777: the one-shot latch the play
    *  loop uses to persist progress. `var37` is a run() local (:633), so the
    *  real loop must be pumped: the first fase-5 pass arms it and saves
    *  usercar, the first fase -5 pass saves the set and clears it, and every
    *  later -5 pass saves nothing. The Seam's savecookie override records
    *  the keys/values in call order (there is no browser jar to read back:
    *  the real body swallows its no-JSObject exception, :1511-1517).
    *
    *  Setup: one stage load (fase 2) so the fase-5 pass's fallthrough 176
    *  arm has a filled ContO pool (var9[0] would NPE on a bare boot) and
    *  CheckPoints.stage names a real stage. The unlocks/gates are then all
    *  reachable through xtGraphics fields; cp.stage stays 1 (it is a run()
    *  local, reachable only through loadstage/finish). */
   private static void cookieLatchScenario() throws Throwable {
      Boot b = Boot.start("assets");
      try {
         Seam app = b.app;
         b.xt.fase = 2;
         b.app.mouses = 0;
         b.step(); // loadingstage parks
         b.step(); // loadstage fills the pool, fase 1
         Check.eq(b.xt.fase, 1, "latch_setup_stage_fase_1");

         // ---- qualifying: stage 1 == unlocked 1 and a win
         b.xt.unlocked = 1;
         b.xt.winner = true;
         b.xt.sc[0] = 3;
         app.clearSaved();
         b.xt.fase = 5;
         b.step(); // loadmusic parks
         b.step(); // loadmusic tail, then the arm (:773-776)
         Check.eq(app.savedTrace(), "usercar=3", "latch_fase5_arm_usercar");
         Check.eq(app.savedCount(), 1, "latch_fase5_arm_once");
         Check.eq(b.xt.fase, 176, "latch_fase5_arm_fase_176");

         app.clearSaved();
         b.xt.fase = -5;
         b.step(); // finish, then the fire (:727-735)
         Check.eq(app.savedTrace(), "unlocked=2,gameprfact=35,usercar=3",
               "latch_fire_qualifying_set");
         Check.eq(app.savedCount(), 3, "latch_fire_qualifying_count");
         // the fire itself never bumps the unlock: only finish's enter block
         // (:2268-2271) does, so the save value is the pre-bump one
         Check.eq(b.xt.unlocked, 1, "latch_fire_qualifying_unlocked_kept");

         // ---- one shot: a second -5 pass saves nothing at all
         String fired = app.savedTrace();
         b.step();
         Check.eq(app.savedTrace(), fired, "latch_one_shot_second_neg5_no_saves");
         Check.eq(app.savedCount(), 3, "latch_one_shot_second_neg5_count");

         // ---- non-qualifying: replay of an already-unlocked stage
         // (stage 1 < unlocked 5). gameprfact/usercar still save.
         b.xt.unlocked = 5;
         b.xt.winner = true;
         b.xt.sc[0] = 4;
         app.clearSaved();
         b.xt.fase = 5;
         b.step();
         b.step();
         Check.eq(app.savedTrace(), "usercar=4", "latch_replay_arm_usercar");
         app.clearSaved();
         b.xt.fase = -5;
         b.step();
         Check.eq(app.savedTrace(), "gameprfact=35,usercar=4",
               "latch_replay_fire_no_unlocked");
         Check.eq(app.savedCount(), 2, "latch_replay_fire_count");

         // ---- non-qualifying: a loss on the newest stage (winner false)
         b.xt.unlocked = 1;
         b.xt.winner = false;
         b.xt.sc[0] = 2;
         app.clearSaved();
         b.xt.fase = 5;
         b.step();
         b.step();
         Check.eq(app.savedTrace(), "usercar=2", "latch_loss_arm_usercar");
         app.clearSaved();
         b.xt.fase = -5;
         b.step();
         Check.eq(app.savedTrace(), "gameprfact=35,usercar=2",
               "latch_loss_fire_no_unlocked");
         Check.eq(app.savedCount(), 2, "latch_loss_fire_count");

         // ---- (int) var12: the factor save is the truncation, not the
         // rounded float. var12 is a run() local (35.0F at :608; only the
         // wall-clock fps window can make it fractional), so the machine
         // pins its initializer end-to-end above while the cast itself is
         // pinned at the expression. The port's Shell.factor is a field, so
         // the mirror twin drives 41.7 through the same fire end-to-end.
         Check.eq("" + (int) 41.7F, "41", "latch_gameprfact_truncates");
      } finally {
         b.shutdown();
      }
   }

   // =====================================================================
   // 4 - sunytyp
   // =====================================================================

   /** B8(7,8) / GameSparker.java:256-264: the "sun." test reads
    *  `"" + getAppletContext()` (not java.version); java.version only picks
    *  the 1.3 arm, which is unreachable on 8u40. */
   private static void sunytypPins() throws Exception {
      Seam nul = new Seam("assets", null, false);
      Check.eq(nul.sunytyp(), 0, "sunytyp_null_context_0");
      Check.eq("" + (Object) null, "null", "sunytyp_null_string_is_null");

      Seam sun = new Seam("assets", new SunContext(), false);
      Check.eq(sun.sunytyp(), 2, "sunytyp_sun_context_2");
      Check.eq(System.getProperty("java.version").startsWith("1.3") ? 1 : 0, 0,
            "sunytyp_java13_arm_unreachable_on_8u40");

      Seam other = new Seam("assets", new StubContext(), false);
      Check.eq(other.sunytyp(), 0, "sunytyp_other_context_0");

      // the stub-less applet throws instead of returning null (B8(7) fix)
      final Bare bare = new Bare("assets");
      Check.throwsIt(new Check.Task() {
         public void run() {
            bare.getAppletContext();
         }
      }, NullPointerException.class, "sunytyp_bare_no_stub_npe");
   }

   // =====================================================================
   // 5 - init() and the platform's createImage
   // =====================================================================

   /** B8(2) corrected: init() assigns offImage=createImage(550,400) and rd
    *  only when that is non-null (GameSparker.java:1504-1508). On the
    *  gate's 8u40/macOS an unrealized Applet gets null back, so the real
    *  path leaves BOTH null; the recording seam proves the assignments. */
   private static void initPins() throws Exception {
      Bare bare = new Bare("assets");
      Image real = bare.createImage(550, 400);
      bare.init();
      Check.eq(bare.offImage == null, real == null, "init_offimage_follows_createimage");
      Check.eq(bare.rd == null, bare.offImage == null, "init_rd_null_iff_offimage_null");
      Check.eq(real == null, true, "init_createimage_null_on_unrealized_8u40_mac");

      Seam seam = new Seam("assets", null, true);
      seam.init();
      Check.ok(seam.offImage != null, "init_seam_offimage_not_null");
      Check.eq(seam.offImage.getWidth(null), 550, "init_seam_offimage_width_550");
      Check.eq(seam.offImage.getHeight(null), 400, "init_seam_offimage_height_400");
      Check.eq(seam.createW, 550, "init_createimage_arg_width_550");
      Check.eq(seam.createH, 400, "init_createimage_arg_height_400");
      Check.ok(seam.rd != null, "init_seam_rd_not_null");
      Check.ok(seam.rd == seam.offImage.getGraphics(), "init_rd_is_offimage_graphics");
   }

   // =====================================================================
   // 6 - run() with no init()
   // =====================================================================

   /** B8(3) / GameSparker.java:578-580 - the first loop statement is
    *  rd.setColor, so a direct run() throws before the first sleep. */
   private static void runWithoutInit() throws Exception {
      final Bare bare = new Bare("assets");
      Check.throwsIt(new Check.Task() {
         public void run() {
            bare.run();
         }
      }, NullPointerException.class, "run_without_init_npe");
      try {
         bare.run();
      } catch (NullPointerException e) {
         Check.eq(e.getStackTrace()[0].getMethodName(), "run", "run_without_init_top_frame_run");
         Check.eq(e.getStackTrace()[0].getClassName(), "GameSparker", "run_without_init_top_frame_class");
      }
      Check.ok(bare.gamer == null, "run_without_init_no_thread");
   }

   // =====================================================================
   // 7 - start()/stop()/exwist
   // =====================================================================

   /** B8(4,5,6-corrected,22) / GameSparker.java:92-100,292-298,1484-1490. */
   private static void lifecyclePins() throws Throwable {
      // B8(4): the guard only tests gamer==null, so a second start() reaches
      // Thread.start on a live thread.
      Seam app = new Seam("assets", null, true);
      app.init();
      app.autoGate = true;
      app.mouses = 1;
      app.start();
      Check.ok(app.gamer != null, "start_creates_gamer");
      Check.throwsIt(new Check.Task() {
         public void run() {
            app.start();
         }
      }, IllegalThreadStateException.class, "start_twice_illegal_thread_state");
      app.awaitPark();
      Check.eq(app.xt().fase, 9, "start_first_pass_reaches_fase_9");

      // B8(5)+(6) corrected: run() cleared exwist at :644, so stop() skips
      // the whole block (the guard needs exwist==true) and only re-raises
      // the flag; the thread exits on its own at :1484.
      Thread live = app.gamer;
      app.stop();
      Check.ok(app.exwist, "stop_live_sets_exwist_true");
      Check.ok(app.gamer == live, "stop_live_keeps_gamer_reference");
      app.releaseGate();
      live.join(10000);
      Check.ok(!live.isAlive(), "stop_live_thread_exits_within_join");
      Check.ok(app.gamer == null || app.gamer == live, "stop_live_gamer_post_mortem_loose");

      // stop() while exwist is still true nulls the reference (:93-97)
      Seam app2 = new Seam("assets", null, true);
      app2.init();
      app2.gamer = new Thread(app2);
      app2.exwist = true;
      app2.stop();
      Check.ok(app2.gamer == null, "stop_exwist_true_nulls_gamer");
      Check.ok(app2.exwist, "stop_exwist_true_keeps_exwist");

      // exwist==false keeps the reference (only the flag is written)
      Seam app3 = new Seam("assets", null, false);
      Thread dummy = new Thread(app3);
      app3.gamer = dummy;
      app3.exwist = false;
      app3.stop();
      Check.ok(app3.gamer == dummy, "stop_exwist_false_keeps_gamer");
      Check.ok(app3.exwist, "stop_exwist_false_forces_true");
   }

   // =====================================================================
   // 8 - keyDown / keyUp / mouseDown / lostFocus
   // =====================================================================

   /** B8(71-76) / GameSparker.java:102-110,266-290,300-308,1519-1578. All
    *  of these are pure field machines gated by `!exwist`; the Event
    *  argument is never dereferenced. */
   private static void inputPins() throws Exception {
      Seam app = new Seam("assets", null, false);
      app.u[0] = new Control(new Medium());
      app.exwist = false;
      Control c = app.u[0];

      // keyDown drives: 1004 up, 1005 down, 1007 right, 1006 left, 32 handb
      int[] codes = {1004, 1005, 1007, 1006, 32};
      String[] keys = {"1004_up", "1005_down", "1007_right", "1006_left", "32_handb"};
      for (int i = 0; i < codes.length; i++) {
         c.falseo();
         Check.eq(app.keyDown(null, codes[i]), false, "keydown_" + keys[i] + "_returns_false");
         Check.ok(c.up || c.down || c.right || c.left || c.handb, "keydown_" + keys[i] + "_sets_a_drive_flag");
      }
      Check.ok(c.handb, "keydown_32_sets_handb");
      // 10/80/112/27 all set enter and nothing else
      for (int k : new int[]{10, 80, 112, 27}) {
         c.falseo();
         app.keyDown(null, k);
         Check.ok(c.enter, "keydown_" + k + "_sets_enter");
         Check.ok(!c.up && !c.down && !c.left && !c.right && !c.handb, "keydown_" + k + "_only_enter");
      }
      // 97/65 toggle arrace
      c.falseo();
      app.keyDown(null, 97);
      Check.ok(c.arrace, "keydown_97_arrace_on");
      app.keyDown(null, 65);
      Check.ok(!c.arrace, "keydown_65_arrace_off");
      // 77/109 toggle mutem
      c.falseo();
      app.keyDown(null, 77);
      Check.ok(c.mutem, "keydown_77_mutem_on");
      app.keyDown(null, 109);
      Check.ok(!c.mutem, "keydown_109_mutem_off");
      // 78/110 toggle mutes
      c.falseo();
      app.keyDown(null, 78);
      Check.ok(c.mutes, "keydown_78_mutes_on");
      app.keyDown(null, 110);
      Check.ok(!c.mutes, "keydown_110_mutes_off");
      // 118/86 cycle the view 0 -> 1 -> 2 -> 0
      app.view = 0;
      app.keyDown(null, 118);
      Check.eq(app.view, 1, "keydown_118_view_1");
      app.keyDown(null, 86);
      Check.eq(app.view, 2, "keydown_86_view_2");
      app.keyDown(null, 118);
      Check.eq(app.view, 0, "keydown_118_view_wraps_0");

      // B8(72): exwist==true is a total no-op
      app.exwist = true;
      c.falseo();
      app.view = 1;
      app.keyDown(null, 1004);
      app.keyDown(null, 97);
      app.keyDown(null, 118);
      Check.ok(!c.up && !c.arrace, "keydown_exwist_no_writes");
      Check.eq(app.view, 1, "keydown_exwist_view_unchanged");
      app.exwist = false;

      // B8(73): keyUp clears the four drives + handb and NEVER enter
      c.up = c.down = c.left = c.right = c.handb = c.enter = true;
      app.keyUp(null, 1004);
      Check.ok(!c.up, "keyup_1004_up_false");
      app.keyUp(null, 1005);
      Check.ok(!c.down, "keyup_1005_down_false");
      app.keyUp(null, 1007);
      Check.ok(!c.right, "keyup_1007_right_false");
      app.keyUp(null, 1006);
      Check.ok(!c.left, "keyup_1006_left_false");
      app.keyUp(null, 32);
      Check.ok(!c.handb, "keyup_32_handb_false");
      Check.ok(c.enter, "keyup_never_clears_enter");
      Check.eq(app.keyUp(null, 999), false, "keyup_other_returns_false");
      app.keyUp(null, 10);
      Check.ok(c.enter, "keyup_10_does_not_clear_enter");

      // B8(74): mouseDown latches xm/ym/mouses exactly once
      app.mouses = 0;
      Check.eq(app.mouseDown(null, 12, 34), false, "mousedown_returns_false");
      Check.eq(app.xm, 12, "mousedown_xm_12");
      Check.eq(app.ym, 34, "mousedown_ym_34");
      Check.eq(app.mouses, 1, "mousedown_mouses_1");
      app.mouseDown(null, 99, 99);
      Check.eq(app.xm, 12, "mousedown_second_keeps_xm");
      Check.eq(app.ym, 34, "mousedown_second_keeps_ym");
      Check.eq(app.mouses, 1, "mousedown_second_keeps_mouses");
      app.mouses = 0;
      app.exwist = true;
      app.mouseDown(null, 5, 6);
      Check.eq(app.mouses, 0, "mousedown_exwist_no_latch");
      app.exwist = false;

      // B8(75): lostFocus sets lostfcs, drops mouses and clears the flags
      c.up = c.down = c.left = c.right = c.handb = c.enter = true;
      app.lostfcs = false;
      app.mouses = 1;
      Check.eq(app.lostFocus(null, null), false, "lostfocus_returns_false");
      Check.ok(app.lostfcs, "lostfocus_sets_lostfcs");
      Check.eq(app.mouses, 0, "lostfocus_mouses_0");
      Check.ok(!c.up && !c.down && !c.left && !c.right && !c.handb && !c.enter, "lostfocus_u0_falseo");
      app.lostfcs = false;
      app.exwist = true;
      app.lostFocus(null, null);
      Check.ok(!app.lostfcs, "lostfocus_exwist_no_write");
      app.exwist = false;

      // B8(76): a null u[0] NPEs at :106/:269/:1522 when exwist==false and
      // never for mouseDown (:300-308 dereferences no Control)
      Seam nul = new Seam("assets", null, false);
      nul.exwist = false;
      Check.throwsIt(new Check.Task() {
         public void run() {
            nul.keyDown(null, 1004);
         }
      }, NullPointerException.class, "nul_u0_keydown_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            nul.keyUp(null, 1004);
         }
      }, NullPointerException.class, "nul_u0_keyup_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            nul.lostFocus(null, null);
         }
      }, NullPointerException.class, "nul_u0_lostfocus_npe");
      Check.eq(nul.mouseDown(null, 1, 2), false, "nul_u0_mousedown_no_npe");
      Check.eq(nul.mouses, 1, "nul_u0_mousedown_latches");
      // exwist==true short-circuits before the dereference at :103/:267/:1520
      nul.exwist = true;
      Check.eq(nul.keyDown(null, 1004), false, "nul_u0_keydown_exwist_no_npe");
      Check.eq(nul.keyUp(null, 1004), false, "nul_u0_keyup_exwist_no_npe");
      Check.eq(nul.lostFocus(null, null), false, "nul_u0_lostfocus_exwist_no_npe");
   }

   // =====================================================================
   // 9 - loadbase
   // =====================================================================

   /** B8(41,42) / GameSparker.java:167-250: the 43-name table, the
    *  startsWith scan that keeps the LAST match, the byte read loop and
    *  `new ContO(bytes, medium, trackers)` per entry. On the real
    *  models.zipo every one of the 43 slots ends up non-null. */
   private static void loadbasePins() throws Exception {
      Holder h = new Holder("assets");
      h.app.loadbase(h.models, h.medium, h.trackers);
      int nonNull = 0;
      for (int i = 0; i < 43; i++) {
         if (h.models[i] != null) {
            nonNull++;
         }
      }
      Check.eq(nonNull, 43, "loadbase_real_all_43_slots_filled");
      Check.ok(h.models[10] != null, "loadbase_slot_10_road");
      Check.ok(h.models[15] != null, "loadbase_slot_15_offroad");
      Check.ok(h.models[23] != null, "loadbase_slot_23_roadend_last_match");
      Check.ok(h.models[24] != null, "loadbase_slot_24_offroadend_last_match");
      Check.ok(h.models[39] != null, "loadbase_slot_39_thewall");
      Check.ok(h.models[42] != null, "loadbase_slot_42_offcheckpoint");
      Check.eq(h.models[0].getClass().getName(), "ContO", "loadbase_entries_are_ContO");
      // the copy ctor works off a loaded slot (loadstage's tail needs this)
      ContO copy = new ContO(h.models[0], 0, 0, 0, 0);
      Check.ok(copy != null, "loadbase_slot_copy_ctor");
   }

   /** B8(41)'s rule, isolated: a synthetic zipo with "offroadendx.rad"
    *  (matches offroad AND offroadend, so the LAST match is 24) plus a name
    *  that matches nothing (var10's 0 initializer sends it to slot 0). */
   private static void loadbaseLastMatch() throws Exception {
      Holder h = new Holder(fixtureLastMatch());
      h.app.loadbase(h.models, h.medium, h.trackers);
      Check.ok(h.models[24] != null, "lastmatch_offroadendx_lands_24");
      Check.ok(h.models[15] == null, "lastmatch_offroad_15_untouched");
      Check.ok(h.models[10] == null, "lastmatch_road_10_untouched");
      Check.ok(h.models[0] != null, "lastmatch_unmatched_entry_lands_0");
      int nonNull = 0;
      for (int i = 0; i < 43; i++) {
         if (h.models[i] != null) {
            nonNull++;
         }
      }
      Check.eq(nonNull, 2, "lastmatch_only_two_slots_filled");
   }

   /** B8(40) / GameSparker.java:245-249: a codebase without
    *  graphics/models.zipo prints "Error Reading Models: ..." and returns
    *  normally with every slot still null. */
   private static void loadbaseMissing() throws Exception {
      Holder h = new Holder(fixtureEmpty());
      h.app.loadbase(h.models, h.medium, h.trackers);
      int nonNull = 0;
      for (int i = 0; i < 43; i++) {
         if (h.models[i] != null) {
            nonNull++;
         }
      }
      Check.eq(nonNull, 0, "loadbase_missing_all_slots_null");
   }

   // =====================================================================
   // 10 - loadstage
   // =====================================================================

   /** B8(43,45-53) / GameSparker.java:310-576 on the real stages/1.txt. */
   private static void loadstagePins() throws Exception {
      Direct d = directAssets();
      // B8(43): the prelude overwrites everything BEFORE the file is
      // touched, so pre-set sentinels are gone even on a successful parse.
      d.trackers.nt = 77;
      d.app.nob = 99;
      d.app.notb = 88;
      d.cp.n = 55;
      d.cp.nsp = 44;
      d.cp.fn = 33;
      d.cp.haltall = true;
      d.cp.wasted = 22;
      d.medium.ground = 999;
      d.app.view = 2;
      d.xt.fase = 2;
      d.app.loadstage(d.obj, d.models, d.medium, d.trackers, d.cp, d.xt, d.madness, d.record);

      // B8(45): the promotion ran (fase was 2), then the promotion's tail
      Check.eq(d.xt.fase, 1, "loadstage_fase_2_promotes_to_1");
      Check.eq(d.app.mouses, 0, "loadstage_promotion_clears_mouses");
      Check.eq(d.medium.trx, -5600L, "loadstage_trx_mean_of_last_four_minus5600");
      Check.eq(d.medium.trz, 15600L, "loadstage_trz_mean_of_last_four_15600");
      Check.eq(d.medium.ptr, 0, "loadstage_ptr_0");
      Check.eq(d.medium.ptcnt, -10, "loadstage_ptcnt_minus10");
      Check.eq(d.medium.hit, 60000, "loadstage_hit_60000");
      Check.eq(d.medium.nrnd, 0, "loadstage_nrnd_0");
      Check.ok(d.medium.trk, "loadstage_trk_true");
      Check.eq(d.trackers.nt, 85, "loadstage_trackers_nt_85");

      // B8(47)
      Check.eq(d.cp.name, "The Introductory Stage", "loadstage_name");
      Check.eq(d.cp.nlaps, 4, "loadstage_nlaps_4");

      // B8(48): pcs is the index of the LAST chk line, nsp counts chk
      // lines, n counts chk+")p" set lines, notb follows nob (:379-403)
      Check.eq(d.app.nob, 66, "loadstage_nob_66");
      Check.eq(d.app.notb, 28, "loadstage_notb_28");
      Check.eq(d.cp.n, 20, "loadstage_cp_n_20");
      Check.eq(d.cp.nsp, 2, "loadstage_cp_nsp_2");
      Check.eq(d.cp.pcs, 18, "loadstage_cp_pcs_last_chk_18");
      Check.eq(d.cp.fn, 1, "loadstage_cp_fn_1");
      Check.ok(!d.cp.haltall, "loadstage_haltall_false");
      Check.eq(d.cp.wasted, 0, "loadstage_wasted_0");
      Check.eq(d.medium.ground, 250, "loadstage_ground_250");
      Check.eq(d.app.view, 0, "loadstage_view_0");

      // B8(49): the set-line marker table )p )pt )pr with cp coords, y=0
      Check.eq(d.cp.typ[0], 0, "loadstage_marker_p_typ0");
      Check.eq(d.cp.z[1], 5600, "loadstage_set_z1_5600");
      Check.eq(d.cp.typ[2], -2, "loadstage_marker_pr_typ_minus2");
      Check.eq(d.cp.x[8], -4550, "loadstage_set_x8_minus4550");
      Check.eq(d.cp.typ[7], -1, "loadstage_marker_pt_typ_minus1");
      Check.eq(d.cp.y[7], 0, "loadstage_set_y0");
      Check.eq(d.cp.typ[19], -1, "loadstage_last_line_pt");
      Check.eq(d.cp.nsp, 2, "loadstage_nsp_counts_chk_only");

      // B8(50): chk rot==0 -> typ 1, else typ 2; y = 250 - model.grat
      Check.eq(d.cp.typ[6], 1, "loadstage_chk_rot0_typ1");
      Check.eq(d.cp.y[6], 250, "loadstage_chk_y_250_minus_grat");
      Check.eq(d.cp.z[6], 28000, "loadstage_chk_z6_28000");
      Check.eq(d.cp.typ[18], 2, "loadstage_chk_rot_nonzero_typ2");
      Check.eq(d.cp.x[18], -4550, "loadstage_chk_x18_minus4550");

      // B8(51): resetstat(stage) ran (GameSparker.java:566)
      Check.eq(d.xt.starcnt, 90, "loadstage_resetstat_starcnt_90");
      Check.eq(d.xt.gocnt, 3, "loadstage_resetstat_gocnt_3");
      Check.ok(!d.xt.winner, "loadstage_resetstat_winner_false");
      Check.eq(d.xt.wasted, 0, "loadstage_resetstat_wasted_0");

      // B8(52): every u[i].reset(cp) ran (the same walk a fresh Control
      // performs on the same cp proves it; the value itself is stable)
      Control fresh = new Control(d.medium);
      fresh.reset(d.cp);
      for (int i = 0; i < 5; i++) {
         Check.eq(d.app.u[i].fpnt[0], fresh.fpnt[0], "loadstage_u" + i + "_reset_fpnt");
      }
      Check.eq(d.app.u[0].fpnt[0], 10, "loadstage_u0_fpnt_10");

      // the car row is the pristine model copy at xstart/zstart (B8(53))
      Check.eq(d.obj[0].x, 0, "loadstage_car0_xstart_0");
      Check.eq(d.obj[0].z, -100, "loadstage_car0_zstart_minus100");
      Check.eq(d.obj[0].y, 206, "loadstage_car0_y_250_minus_grat");
      Check.eq(d.obj[0].xz, 0, "loadstage_car0_xz_0");
      Check.eq(d.xt.sc[0], 0, "loadstage_car0_sc0_0");

      // the file's snap / fog / fadefrom lines
      Check.eq(d.medium.snap[0], -5, "loadstage_snap0_minus5");
      Check.eq(d.medium.snap[1], -5, "loadstage_snap1_minus5");
      Check.eq(d.medium.snap[2], 20, "loadstage_snap2_20");
      Check.eq(d.medium.origfade, d.medium.fade[0], "loadstage_fadefrom_sets_origfade");
   }

   /** B8(49) continued on stages/9.txt - the first stage carrying )po
    *  (typ -3) and )ph (typ -4) - plus a second independent parse. */
   private static void loadstageNinePins() throws Exception {
      Direct d = directAssets();
      CheckPoints cp = new CheckPoints();
      cp.stage = 9;
      d.xt.fase = 2;
      d.app.loadstage(d.obj, d.models, d.medium, d.trackers, cp, d.xt, d.madness, d.record);
      Check.eq(d.xt.fase, 1, "stage9_fase_1");
      Check.eq(cp.name, "The Beach Arcade Dream", "stage9_name");
      Check.eq(cp.nlaps, 3, "stage9_nlaps_3");
      Check.eq(cp.n, 83, "stage9_n_83");
      Check.eq(cp.nsp, 7, "stage9_nsp_7");
      Check.eq(cp.pcs, 76, "stage9_pcs_76");
      Check.eq(cp.fn, 2, "stage9_fn_2");
      Check.eq(d.app.nob, 205, "stage9_nob_205");
      Check.eq(d.app.notb, 147, "stage9_notb_147");
      Check.eq(d.trackers.nt, 468, "stage9_nt_468");
      Check.eq(d.medium.trx, 16200L, "stage9_trx_16200");
      Check.eq(d.medium.trz, 18000L, "stage9_trz_18000");
      Check.eq(d.medium.snap[0], 20, "stage9_snap0_20");
      Check.eq(d.medium.snap[1], -10, "stage9_snap1_minus10");
      Check.eq(d.medium.snap[2], -20, "stage9_snap2_minus20");
      // marker table counts: )ph -4, )po -3, )pr -2, )pt -1, )p 0,
      // chk-rot0 1, chk-rot-nonzero 2
      int[] counts = new int[7];
      for (int i = 0; i < cp.n; i++) {
         counts[cp.typ[i] + 4]++;
      }
      Check.eq(counts[0], 5, "stage9_typ_minus4_count_5");
      Check.eq(counts[1], 1, "stage9_typ_minus3_count_1");
      Check.eq(counts[2], 5, "stage9_typ_minus2_count_5");
      Check.eq(counts[3], 11, "stage9_typ_minus1_count_11");
      Check.eq(counts[4], 54, "stage9_typ_0_count_54");
      Check.eq(counts[5], 2, "stage9_typ_1_count_2");
      Check.eq(counts[6], 5, "stage9_typ_2_count_5");
      // the two marker lines with their coordinates
      Check.eq(cp.typ[5], -4, "stage9_ph_marker_typ_minus4");
      Check.eq(cp.x[5], -1810, "stage9_ph_x_minus1810");
      Check.eq(cp.z[5], 19000, "stage9_ph_z_19000");
      Check.eq(cp.typ[78], -3, "stage9_po_marker_typ_minus3");
      Check.eq(cp.x[78], 5000, "stage9_po_x_5000");
      Check.eq(cp.z[78], 1400, "stage9_po_z_1400");
      // fix lines: coords, roted and the )s special flag
      Check.eq(cp.fx[0], 23520, "stage9_fix_x_23520");
      Check.eq(cp.fz[0], 30000, "stage9_fix_z_30000");
      Check.eq(cp.fy[0], -3500, "stage9_fix_y_minus3500");
      Check.ok(cp.special[0], "stage9_fix_special_s");
      Check.ok(!cp.roted[0], "stage9_fix_roted_false");

      // stages/2.txt is the smallest stage carrying a fix with a non-zero
      // rot argument (:414-419) - the only place )roted is set
      CheckPoints cp2 = new CheckPoints();
      cp2.stage = 2;
      d.xt.fase = 2;
      d.app.loadstage(d.obj, d.models, d.medium, d.trackers, cp2, d.xt, d.madness, d.record);
      Check.eq(d.xt.fase, 1, "stage2_fase_1");
      Check.eq(cp2.fn, 1, "stage2_fn_1");
      Check.ok(cp2.roted[0], "stage2_fix_rot90_roted_true");
      Check.ok(!cp2.special[0], "stage2_fix_no_s_special_false");
      Check.ok(!d.cp.roted[0], "stage2_stage1_fix_stays_unroted");
   }

   /** B8(46) / GameSparker.java:537-558 - the promotion is gated on
    *  fase==2 AFTER the parse, so a pre-set fase leaves the medium tail and
    *  mouses untouched while the table is still replaced. */
   private static void loadstageNoPromotion() throws Exception {
      Direct d = directAssets();
      CheckPoints cp = new CheckPoints();
      d.medium.trx = 111L;
      d.medium.trz = 222L;
      d.medium.ptr = 33;
      d.medium.ptcnt = 44;
      d.medium.hit = 555;
      d.medium.nrnd = 66;
      d.medium.trk = false;
      d.app.mouses = 1;
      d.xt.fase = 5;
      d.app.loadstage(d.obj, d.models, d.medium, d.trackers, cp, d.xt, d.madness, d.record);
      Check.eq(d.xt.fase, 5, "loadstage_fase_5_kept");
      Check.eq(d.medium.trx, 111L, "loadstage_fase5_trx_untouched");
      Check.eq(d.medium.trz, 222L, "loadstage_fase5_trz_untouched");
      Check.eq(d.medium.ptr, 33, "loadstage_fase5_ptr_untouched");
      Check.eq(d.medium.ptcnt, 44, "loadstage_fase5_ptcnt_untouched");
      Check.eq(d.medium.hit, 555, "loadstage_fase5_hit_untouched");
      Check.eq(d.medium.nrnd, 66, "loadstage_fase5_nrnd_untouched");
      Check.ok(!d.medium.trk, "loadstage_fase5_trk_untouched");
      Check.eq(d.app.mouses, 1, "loadstage_fase5_mouses_untouched");
      Check.eq(cp.n, 20, "loadstage_fase5_still_parses");
   }

   /** B8(43,44) on a codebase that has models but no stages/: stage 1 is
    *  missing, the catch arm sets fase=3, the promotion is skipped and -
    *  unlike a stage>=11 - the tail finishes because sortcars keeps sc in
    *  0..5 and every model slot is loaded. */
   private static void loadstageMissingPins() throws Exception {
      Direct d = directGfx();
      CheckPoints cp = new CheckPoints();
      d.medium.trx = 111L;
      d.medium.trk = true;
      d.app.nob = 99;
      d.app.notb = 88;
      cp.n = 55;
      cp.nsp = 44;
      cp.fn = 33;
      cp.haltall = true;
      cp.wasted = 22;
      d.medium.ground = 999;
      d.app.view = 2;
      d.xt.fase = 2;
      cp.stage = 1;
      d.app.loadstage(d.obj, d.models, d.medium, d.trackers, cp, d.xt, d.madness, d.record);
      Check.eq(d.xt.fase, 3, "loadstage_missing_fase_3");
      Check.eq(d.app.nob, 5, "loadstage_missing_nob_5");
      Check.eq(d.app.notb, 0, "loadstage_missing_notb_0");
      Check.eq(cp.n, 0, "loadstage_missing_cp_n_0");
      Check.eq(cp.nsp, 0, "loadstage_missing_cp_nsp_0");
      Check.eq(cp.fn, 0, "loadstage_missing_cp_fn_0");
      Check.ok(!cp.haltall, "loadstage_missing_haltall_false");
      Check.eq(cp.wasted, 0, "loadstage_missing_wasted_0");
      Check.eq(d.medium.ground, 250, "loadstage_missing_ground_250");
      Check.eq(d.app.view, 0, "loadstage_missing_view_0");
      // the promotion is skipped: the medium tail keeps its sentinels
      Check.eq(d.medium.trx, 111L, "loadstage_missing_trx_untouched");
      Check.ok(d.medium.trk, "loadstage_missing_trk_untouched");
      // everything after the catch still ran
      Check.eq(d.xt.starcnt, 90, "loadstage_missing_resetstat_ran");
   }

   /** B8(52,53,78) / GameSparker.java:531,537,563,570 - the two null-xt
    *  variants, the null-u[i] resume walk and the models-row dependency. */
   private static void loadstageErrorPins() throws Exception {
      final Direct gfx = directGfx();
      final CheckPoints cpMissing = new CheckPoints();
      cpMissing.stage = 1;
      Check.throwsIt(new Check.Task() {
         public void run() {
            gfx.app.loadstage(gfx.obj, gfx.models, gfx.medium, gfx.trackers, cpMissing, null, gfx.madness,
                  gfx.record);
         }
      }, NullPointerException.class, "loadstage_xt_null_missing_stage_npe");
      try {
         gfx.app.loadstage(gfx.obj, gfx.models, gfx.medium, gfx.trackers, cpMissing, null, gfx.madness, gfx.record);
      } catch (NullPointerException e) {
         Check.eq(e.getStackTrace()[0].getMethodName(), "loadstage", "loadstage_xt_null_top_frame");
      }

      final Direct d = directAssets();
      Check.throwsIt(new Check.Task() {
         public void run() {
            d.app.loadstage(d.obj, d.models, d.medium, d.trackers, new CheckPoints(), null, d.madness, d.record);
         }
      }, NullPointerException.class, "loadstage_xt_null_parsed_stage_npe");

      // B8(52): the resume loop needs all five Controls
      d.app.u[3] = null;
      Check.throwsIt(new Check.Task() {
         public void run() {
            d.app.loadstage(d.obj, d.models, d.medium, d.trackers, new CheckPoints(), d.xt, d.madness, d.record);
         }
      }, NullPointerException.class, "loadstage_null_u3_npe");
      d.app.u[3] = new Control(d.medium);

      // B8(53): the car row copies models[xt.sc[i]]; a null slot kills it
      // at :570 after the parse succeeded
      d.xt.sc[0] = 39;
      ContO saved = d.models[39];
      d.models[39] = null;
      Check.throwsIt(new Check.Task() {
         public void run() {
            d.app.loadstage(d.obj, d.models, d.medium, d.trackers, new CheckPoints(), d.xt, d.madness, d.record);
         }
      }, NullPointerException.class, "loadstage_null_model_row_npe");
      d.models[39] = saved;
      d.xt.sc[0] = 0;
   }

   // =====================================================================
   // 11 - testlocation
   // =====================================================================

   /** B8(80,81-corrected) / GameSparker.java:28-90. The first try needs a
    *  live JSObject, so a file: codebase always lands in the catch arm;
    *  there var1 is true for any non-http: document base, so nothing is
    *  drawn and nothing is stopped. An http: base that is not
    *  radicalplay.com takes the denial arm. */
   private static void testlocationPins() throws Exception {
      // file: document base -> catch arm, var1 true, no drawing at all
      StubContext fileCtx = new StubContext();
      Seam app = new Seam("assets", fileCtx, true);
      app.init();
      app.testlocation();
      Check.eq(nonZeroPixels(app.img), 0, "testlocation_file_base_draws_nothing");
      Check.ok(fileCtx.calls.isEmpty(), "testlocation_file_base_no_showdocument");
      Check.ok(app.gamer == null, "testlocation_file_base_no_gamer");

      // http: document base, not radicalplay.com -> "Access Denied !"
      StubContext ctx = new StubContext();
      Seam deny = new Seam("assets", ctx, true);
      deny.docbase = new URL("http://example.com/game/");
      deny.init();
      try {
         deny.testlocation();
         Check.ok(false, "testlocation_denial_gamer_stop_npe");
      } catch (NullPointerException e) {
         Check.ok(true, "testlocation_denial_gamer_stop_npe");
         Check.eq(e.getStackTrace()[0].getMethodName(), "testlocation", "testlocation_denial_npe_frame");
      }
      Check.ok(nonZeroPixels(deny.img) > 0, "testlocation_denial_draws_access_denied");
      Check.eq(ctx.calls.size(), 1, "testlocation_denial_one_showdocument");
      Check.eq(ctx.calls.get(0), "http://www.radicalplay.com/madness/ target=olen",
            "testlocation_denial_showdocument_args");

      // the same denial arm with a null AppletContext: :83's NPE is eaten
      // by the empty catch at :81-85, so the observable throw is still :87
      Seam nul = new Seam("assets", null, true);
      nul.docbase = new URL("http://example.com/game/");
      nul.init();
      try {
         nul.testlocation();
         Check.ok(false, "testlocation_denial_null_ctx_npe");
      } catch (NullPointerException e) {
         Check.ok(true, "testlocation_denial_null_ctx_npe");
         Check.eq(e.getStackTrace()[0].getMethodName(), "testlocation", "testlocation_denial_null_ctx_frame");
      }
      Check.ok(nonZeroPixels(nul.img) > 0, "testlocation_denial_null_ctx_still_draws");

      // the allowed-host arm: the "radicalplay.com" scan flips var1
      StubContext okCtx = new StubContext();
      Seam ok = new Seam("assets", okCtx, true);
      ok.docbase = new URL("http://www.radicalplay.com/madness/");
      ok.init();
      ok.testlocation();
      Check.eq(nonZeroPixels(ok.img), 0, "testlocation_allowed_host_no_draw");
      Check.ok(okCtx.calls.isEmpty(), "testlocation_allowed_host_no_showdocument");
   }

   // =====================================================================
   // 12 - paint / update
   // =====================================================================

   /** B8(79) / GameSparker.java:163-165,252-254 - paint blits offImage at
    *  (0,0) with `this` as observer, update is a bare delegate. */
   private static void paintPins() throws Exception {
      Seam app = new Seam("assets", null, false);
      BufferedImage src = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      src.setRGB(10, 10, 0x00ff0000);
      src.setRGB(300, 200, 0x0000ff00);
      app.offImage = src;
      BufferedImage dst = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      RecG dg = new RecG(dst.getGraphics());
      app.paint(dg);
      Check.eq(dst.getRGB(10, 10) & 0xffffff, 0x00ff0000, "paint_blits_pixel_at_origin");
      Check.eq(dst.getRGB(300, 200) & 0xffffff, 0x0000ff00, "paint_blits_second_pixel");
      Check.ok(dg.lastObserver == app, "paint_observer_is_the_applet");
      BufferedImage dst2 = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      app.update(dst2.getGraphics());
      Check.eq(dst2.getRGB(10, 10) & 0xffffff, 0x00ff0000, "update_delegates_to_paint");
   }

   // =====================================================================
   // 13 - loadmusic (the device-free half)
   // =====================================================================

   /** B8(15,16) / xtGraphics.java:1181-1340: with loadhold!=0 (the
    *  cookie-less 80) the machine loads the real stage1 mod, sets loadedt
    *  and fase=176 and never calls play(). The var2==0 -> play() branch is
    *  device-gated and deliberately not driven. */
   private static void loadmusicDirectPins() throws Exception {
      Direct d = directAssets();
      d.xt.loadedt[0] = false;
      d.xt.stracks[0] = null;
      d.xt.mutem = true;
      d.xt.mutes = true;
      d.xt.pcontin = 7;
      d.xt.loadmusic(1, KGP.intv("loadhold"), d.app.rd);
      Check.eq(d.xt.fase, 176, "loadmusic_var2_80_fase_176");
      Check.ok(d.xt.loadedt[0], "loadmusic_real_stage1_loadedt_true");
      Check.ok(d.xt.stracks[0] != null, "loadmusic_stracks0_constructed");
      Check.ok(d.xt.stracks[0].stream != null, "loadmusic_stracks0_stream_not_null");
      Check.ok(!d.xt.stracks[0].playing, "loadmusic_var2_80_never_plays");
      Check.eq(d.xt.pcontin, 0, "loadmusic_resets_pcontin_0");
      Check.eq(d.xt.mutem, false, "loadmusic_resets_mutem");
      Check.eq(d.xt.mutes, false, "loadmusic_resets_mutes");
      // a second call for the same stage does not rebuild the track
      RadicalMod first = d.xt.stracks[0];
      d.xt.loadmusic(1, KGP.intv("loadhold"), d.app.rd);
      Check.ok(d.xt.stracks[0] == first, "loadmusic_loadedt_short_circuits_reload");
   }

   // =====================================================================
   // 14 - the dispatcher matrix
   // =====================================================================

   /** B8(54-69,82) / GameSparker.java:646-1380. One run() thread, one pass
    *  per forced fase, each row asserting the observable that arm writes.
    *  Rows whose handler calls app.repaint() itself (2 -> loadingstage:317,
    *  5 -> loadmusic:1248) park twice; those step through the intermediate
    *  park explicitly. */
   private static void dispatcherMatrix() throws Throwable {
      Boot b = Boot.start("assets");
      try {
         // boot: fase 111 with a pre-latched click -> 9 in ONE pass
         Check.eq(b.xt.fase, 9, "matrix_boot_fase_9");
         Check.eq(b.app.mouses, 0, "matrix_boot_mouses_0");
         Check.eq(b.parks(), 1, "matrix_boot_one_park");
         // the 9 arm needs exactly 200 more passes (var23 1 -> 200)
         b.steps(200);
         Check.eq(b.xt.fase, 10, "matrix_9_arm_200_passes_to_10");

         // ---- fase -9: five white fills then fase 7 (:677-687)
         b.xt.fase = -9;
         b.app.mouses = 1;
         b.steps(5);
         Check.eq(b.xt.fase, -9, "matrix_neg9_five_passes_still_neg9");
         b.step();
         Check.eq(b.xt.fase, 7, "matrix_neg9_sixth_pass_fase_7");
         Check.eq(b.app.mouses, 0, "matrix_neg9_clears_mouses");

         // ---- fase 8: credits + ctachm + the mouses latch (:689-699)
         b.xt.fase = 8;
         b.xt.flipo = 0;
         b.app.mouses = 1;
         b.step();
         Check.eq(b.xt.fase, 8, "matrix_8_fase_kept");
         Check.eq(b.xt.flipo, 1, "matrix_8_credits_flipo_1");
         Check.eq(b.app.mouses, 2, "matrix_8_mouses_latch_1_to_2");
         // credits' own exit: enter with flipo 3 -> flipo 0 + fase 10
         b.xt.flipo = 3;
         b.app.u[0].enter = true;
         b.step();
         Check.eq(b.xt.fase, 10, "matrix_8_enter_sets_fase_10");
         // the 10 arm sits later in the same chain, so maini's flipo==0
         // block immediately rewrites flipo 0 -> 1
         Check.eq(b.xt.flipo, 1, "matrix_8_enter_then_maini_flipo_1");
         Check.ok(!b.app.u[0].enter, "matrix_8_enter_cleared");

         // ---- fase 10: maini + ctachm (:701-711)
         b.xt.fase = 10;
         b.xt.opselect = 1;
         b.app.u[0].down = true;
         b.step();
         Check.eq(b.xt.opselect, 2, "matrix_10_maini_down_opselect_2");
         Check.ok(!b.app.u[0].down, "matrix_10_maini_clears_down");
         // pin 21: opselect==1 records oldfase=10 ...
         b.xt.fase = 10;
         b.xt.opselect = 1;
         b.app.u[0].enter = true;
         b.step();
         Check.eq(b.xt.fase, 11, "matrix_10_enter_fase_11");
         Check.eq(b.xt.oldfase, 10, "matrix_10_enter_opselect1_oldfase_10");
         Check.eq(b.xt.flipo, 1, "matrix_10_enter_flipo_1");
         // ... and opselect==0 with unlocked==1/oldfase==0 records 7
         b.xt.opselect = 0;
         b.xt.oldfase = 0;
         b.xt.fase = 10;
         b.app.u[0].enter = true;
         b.step();
         Check.eq(b.xt.oldfase, 7, "matrix_10_enter_opselect0_oldfase_7");
         Check.eq(b.xt.fase, 11, "matrix_10_enter_opselect0_fase_11");

         // ---- fase 11: inst + ctachm (:713-723)
         b.xt.flipo = 0;
         b.app.u[0].enter = true;
         b.xt.fase = 11;
         b.step();
         Check.eq(b.xt.fase, 11, "matrix_11_fase_kept");
         // inst draws block 0 (flipo 0 -> 1) and then the enter block
         // advances 1 -> 2 inside the SAME call (:1426-1444)
         Check.eq(b.xt.flipo, 2, "matrix_11_inst_same_call_flipo_2");
         Check.ok(!b.app.u[0].enter, "matrix_11_enter_cleared");

         // ---- fase -5: finish + the one-time savecookie gate (:725-745)
         b.app.u[0].enter = true;
         b.xt.fase = -5;
         b.step();
         Check.eq(b.xt.fase, 10, "matrix_neg5_finish_fase_10");
         Check.ok(!b.app.u[0].enter, "matrix_neg5_enter_cleared");

         // ---- fase 7: carselect (:747-757) - medium.crs and the poser
         b.xt.flipo = 0;
         b.xt.fase = 7;
         b.step();
         Check.eq(b.xt.fase, 7, "matrix_7_fase_kept");
         Check.ok(b.medium().crs, "matrix_7_carselect_crs_true");
         Check.eq(b.medium().x, -275, "matrix_7_carselect_x_minus275");
         Check.eq(b.medium().y, -500, "matrix_7_carselect_y_minus500");
         Check.eq(b.medium().z, -50, "matrix_7_carselect_z_minus50");
         Check.eq(b.medium().xz, 0, "matrix_7_carselect_xz_0");
         Check.eq(b.medium().zy, 10, "matrix_7_carselect_zy_10");
         Check.eq(b.medium().ground, 470, "matrix_7_carselect_ground_470");

         // ---- fase 6: musicomp without enter -> pstar flicker (:759-769)
         b.xt.pstar = 0;
         b.xt.fase = 6;
         b.step();
         Check.eq(b.xt.fase, 6, "matrix_6_fase_kept");
         Check.eq(b.xt.pstar, 1, "matrix_6_musicomp_pstar_1");

         // ---- fase 3: loadingfailed's enter -> fase 2 (:791-801), then the
         // 2 arm runs in the SAME pass and parks inside loadingstage
         b.app.u[0].enter = true;
         b.xt.fase = 3;
         b.step();
         Check.eq(b.xt.fase, 2, "matrix_3_loadingfailed_fase_2");
         Check.ok(!b.app.u[0].enter, "matrix_3_enter_cleared");
         b.step();
         Check.eq(b.xt.fase, 1, "matrix_2_promotes_to_1_same_pass");
         Check.eq(b.xt.starcnt, 90, "matrix_2_resetstat_starcnt_90");
         Check.eq(b.app.nob, 66, "matrix_2_nob_66");
         Check.ok(!b.app.u[0].enter, "matrix_2_falseo_clears_enter");

         // every ctachm arm repeats the same two-line latch (mouses 2 -> 0,
         // then 1 -> 2: :704-710, :738-744, :750-756, :762-768, ...). Four
         // representative arms are driven; the rest repeat the identical
         // code with the identical Control writes.
         int[] latchArms = {7, 6, 1, -7};
         String[] latchNames = {"7", "6", "1", "neg7"};
         for (int i = 0; i < latchArms.length; i++) {
            b.xt.fase = latchArms[i];
            b.xt.pnext = 0;
            b.xt.pback = 0;
            b.xt.pcontin = 0;
            b.xt.shaded = false;
            b.xt.flipo = 0;
            b.app.u[0].enter = false;
            b.app.u[0].handb = false;
            b.app.u[0].right = false;
            b.app.u[0].left = false;
            b.app.mouses = 2;
            b.step();
            Check.eq(b.app.mouses, 0, "matrix_mouses_2_to_0_arm" + latchNames[i]);
            b.app.u[0].enter = false;
            b.app.u[0].handb = false;
            b.app.mouses = 1;
            b.step();
            Check.eq(b.app.mouses, 2, "matrix_mouses_1_to_2_arm" + latchNames[i]);
         }
         b.app.mouses = 0;

         // ---- fase 5: loadmusic + savecookie gate (:771-777); parks once
         // inside loadmusic, then lands on 176 in the same pass
         b.xt.fase = 5;
         b.app.mouses = 0;
         b.step();
         Check.eq(b.xt.fase, 5, "matrix_5_park_inside_loadmusic");
         b.step();
         Check.eq(b.xt.fase, 176, "matrix_5_loadmusic_fase_176");
         Check.ok(b.xt.loadedt[0], "matrix_5_loadedt_0_true");
         Check.ok(b.xt.stracks[0] != null, "matrix_5_stracks0_built");
         b.neutralizeTracks();
         // pin the play() guard itself: with loadedt cleared the countdown
         // still reaches fase 6 but :917-919 never plays (the device-free
         // form; the crown walk keeps the loadedt-true form, where the
         // RadicalMod playing flag neutralizes it)
         b.xt.loadedt[0] = false;

         // ---- fase 176: loadhold 80. The 5 pass spent one tick, so 79
         // passes leave var13 at 0 and the 80th flips to 6 (:866-924)
         b.steps(79);
         Check.eq(b.xt.fase, 176, "matrix_176_countdown_79_passes_still_176");
         b.step();
         Check.eq(b.xt.fase, 6, "matrix_176_countdown_eighty_passes_fase_6");
         Check.ok(!b.app.u[0].enter && !b.app.u[0].handb, "matrix_176_clears_enter_handb");
         Check.ok(!b.xt.loadedt[0], "matrix_176_loadedt_false_still_false");

         // ---- fase 6 -> 0 with enter: musicomp (:3356-3361) and the 0 arm
         // in the SAME pass (:926)
         b.app.u[0].enter = true;
         b.step();
         Check.eq(b.xt.fase, 0, "matrix_6_enter_fase_0_same_pass");
         Check.ok(!b.app.u[0].enter, "matrix_6_enter_cleared_by_musicomp");
         Check.eq(b.xt.starcnt, 89, "matrix_0_first_frame_starcnt_89");
         Check.eq(b.medium().adv, KXT.intv("camera_adv_start") - KXT.intv("camera_around_adv_step"),
               "matrix_0_starcnt90_adv_1900_then_around_1880");
         Check.eq(b.medium().vxz, KXT.intv("camera_vxz_start") + KXT.intv("camera_around_vxz_step"),
               "matrix_0_starcnt90_vxz_70_then_around_74");

         // ---- fase 0, starcnt != 0: no colide/drive/checkstat/preform
         b.step();
         Check.eq(b.xt.starcnt, 88, "matrix_0_starcnt_decrements_88");
         // starcnt >= 35 keeps medium.around(cars[3], true): adv -20, vxz +4
         Check.eq(b.medium().adv, KXT.intv("camera_adv_start") - 2 * KXT.intv("camera_around_adv_step"),
               "matrix_0_ge35_around_adv_minus20");
         Check.eq(b.medium().vxz, KXT.intv("camera_vxz_start") + 2 * KXT.intv("camera_around_vxz_step"),
               "matrix_0_ge35_around_vxz_plus4");
         Check.eq(b.app.u[1].stcnt, 0, "matrix_0_no_preform_while_starcnt_nonzero");

         // ---- fase 0, starcnt==50 + enter: the >=35 camera branch clamps to
         // 35 and zeroes mouses/vert/adv/vxz (:1050-1069)
         b.xt.starcnt = 50;
         b.app.mouses = 1;
         b.app.u[0].enter = true;
         b.step();
         Check.eq(b.xt.starcnt, KGP.intv("playsounds_starcnt_gate"), "matrix_0_35_clamp_starcnt_35");
         Check.eq(b.medium().adv, KXT.intv("camera_adv_landing"), "matrix_0_35_clamp_adv_900");
         Check.eq(b.medium().vxz, KXT.intv("camera_vxz_landing"), "matrix_0_35_clamp_vxz_180");
         Check.ok(!b.medium().vert, "matrix_0_35_clamp_vert_false");
         Check.eq(b.app.mouses, 0, "matrix_0_35_clamp_mouses_0");
         Check.ok(!b.app.u[0].enter && !b.app.u[0].handb, "matrix_0_35_clamp_clears_enter_handb");

         // ---- fase 0, starcnt<35 + mouses==1: view 0 follow + stat, then
         // the click becomes enter (:1030-1049); the countdown still spends
         // one tick (34 -> 33) before the camera test
         b.xt.starcnt = KGP.intv("playsounds_starcnt_gate") - 1;
         b.app.view = 0;
         b.app.mouses = 1;
         b.step();
         Check.eq(b.xt.starcnt, KGP.intv("playsounds_starcnt_gate") - 2,
               "matrix_0_below35_one_more_decrement_33");
         Check.eq(b.app.mouses, 0, "matrix_0_below35_click_to_enter_mouses_0");
         Check.ok(b.app.u[0].enter, "matrix_0_below35_click_sets_enter");
         Check.eq(b.medium().zy, 10, "matrix_0_below35_follow_zy_10");
         Check.eq(b.app.u[1].stcnt, 0, "matrix_0_below35_still_no_preform");
         // view 1 -> around(cars[0], false) sets trns 5 (:1036-1039)
         b.app.view = 1;
         b.app.u[0].enter = false;
         b.medium().trns = 1;
         b.step();
         Check.eq(b.medium().trns, 5, "matrix_0_view1_around_trns_5");
         // view 2 -> watch decrements trns 5 -> 4 (:1041-1044)
         b.app.view = 2;
         b.step();
         Check.eq(b.medium().trns, 4, "matrix_0_view2_watch_trns_4");
         b.app.view = 0;

         // ---- fase 0, starcnt==0: the physics block runs and u[1].preform
         // advances its plan gate 0 -> 1 (Control.java:74-76, plan gate)
         b.xt.starcnt = 0;
         b.app.u[1].stcnt = 0;
         b.app.u[0].enter = false;
         b.app.mouses = 0;
         b.step();
         Check.eq(b.app.u[1].stcnt, 1, "matrix_0_starcnt0_physics_preform_ran");

         // ---- fase 1: trackbg + aroundtrack + stageselect (:809-864)
         b.xt.fase = 1;
         b.medium().hit = 60000;
         b.medium().ptr = 0;
         b.medium().ptcnt = -10;
         b.step();
         Check.eq(b.xt.fase, 1, "matrix_1_fase_kept");
         Check.eq(b.medium().hit, 57000, "matrix_1_aroundtrack_hit_minus_3000");
         Check.eq(b.medium().ptr, 0, "matrix_1_aroundtrack_ptr_kept");
         Check.eq(b.medium().ptcnt, -10, "matrix_1_aroundtrack_ptcnt_kept");

         // ---- fase 4: cantgo decrements lockcnt (:779-789)
         b.xt.lockcnt = 5;
         b.xt.fase = 4;
         b.step();
         Check.eq(b.xt.lockcnt, 4, "matrix_4_cantgo_lockcnt_4");
         Check.eq(b.xt.fase, 4, "matrix_4_fase_kept");

         // ---- B8(70) / :1382-1402: the lostfcs overlay runs for every fase
         // except 176/111; at fase 0 it also raises u[0].enter, and a mouses
         // latch clears lostfcs (rewinding flipo for 10/11)
         b.xt.fase = 0;
         b.xt.starcnt = KGP.intv("playsounds_starcnt_gate") - 1;
         b.app.lostfcs = true;
         b.app.u[0].enter = false;
         b.app.mouses = 0;
         b.step();
         Check.ok(b.app.u[0].enter, "matrix_lostfcs_overlay_fase0_sets_enter");
         b.xt.fase = 10;
         b.xt.flipo = 7;
         b.app.mouses = 1;
         b.step();
         Check.ok(!b.app.lostfcs, "matrix_lostfcs_overlay_mouses_clears_flag");
         Check.eq(b.xt.flipo, 0, "matrix_lostfcs_overlay_fase10_flipo_0");
         b.xt.fase = 11;
         b.xt.flipo = 3;
         b.app.lostfcs = true;
         b.app.mouses = 1;
         b.step();
         Check.ok(!b.app.lostfcs, "matrix_lostfcs_overlay_fase11_clears_flag");
         Check.eq(b.xt.flipo, 2, "matrix_lostfcs_overlay_fase11_flipo_minus1");
         b.app.mouses = 0;

         // ---- fase 12: no arm exists at all (:646-1501)
         b.app.u[0].enter = true;
         b.xt.fase = 12;
         b.step();
         Check.eq(b.xt.fase, 12, "matrix_12_no_arm_fase_kept");
         Check.ok(b.app.u[0].enter, "matrix_12_no_arm_no_flag_write");
         b.app.u[0].enter = false;

         // ---- fase -6 -> -7 in the same pass (:1347-1351)
         b.xt.fase = -6;
         b.app.mouses = 1;
         b.step();
         Check.eq(b.xt.fase, -7, "matrix_neg6_sets_fase_neg7");
         Check.eq(b.app.mouses, 0, "matrix_neg6_clears_mouses");

         // ---- fase -7: pausedgame, opselect 1 + enter -> -8 (:1342-1450)
         b.xt.opselect = 1;
         b.app.u[0].enter = true;
         b.xt.fase = -7;
         b.step();
         Check.eq(b.xt.fase, -8, "matrix_neg7_enter_fase_neg8");
         Check.ok(!b.app.u[0].enter, "matrix_neg7_enter_cleared");

         // ---- fase -8: enter -> -7 and everything cleared (:1369-1380)
         b.app.mouses = 1;
         b.app.u[0].enter = true;
         b.xt.fase = -8;
         b.step();
         Check.eq(b.xt.fase, -7, "matrix_neg8_enter_fase_neg7");
         Check.eq(b.app.mouses, 0, "matrix_neg8_clears_mouses");
         Check.ok(!b.app.u[0].enter && !b.app.u[0].handb, "matrix_neg8_clears_flags");

         // ---- fase -2 -> -4 when record.hcaught is false (:1169-1188)
         b.xt.fase = -2;
         b.step();
         Check.eq(b.xt.fase, -4, "matrix_neg2_no_hcaught_fase_neg4");

         // ---- fase -3: enter -> -4 and var22 = -7 (:1310-1315). Record
         // .playh indexes its replay row by the frame counter, so var22
         // must be back in range first (the -7 arm zeroes it).
         b.xt.fase = -7;
         b.step();
         b.app.u[0].enter = true;
         b.xt.fase = -3;
         b.step();
         Check.eq(b.xt.fase, -4, "matrix_neg3_enter_fase_neg4");
         Check.ok(!b.app.u[0].enter && !b.app.u[0].handb, "matrix_neg3_clears_flags");

         // ---- fase -1: the var22==0 pass captures record.ocar, then enter
         // pushes var22 to 299, the wrap stops the loaded track and falls
         // to -6/-7 inside the same pass (:1072-1167)
         b.xt.fase = -7;
         b.step(); // the -7 arm zeroes var22
         b.app.u[0].enter = true;
         b.xt.fase = -1;
         b.step();
         Check.eq(b.xt.fase, -7, "matrix_neg1_wrap_falls_to_neg7");
         Check.ok(!b.app.u[0].enter, "matrix_neg1_enter_cleared");

         // ---- B8(82) cadence invariance + B8(66) ( ++var22 == 6 -> -5 ).
         // From var22==0 the -4 arm needs exactly six passes.
         b.xt.fase = -4;
         b.steps(5);
         Check.eq(b.xt.fase, -4, "matrix_neg4_five_passes_still_neg4");
         b.step();
         Check.eq(b.xt.fase, -5, "matrix_neg4_sixth_pass_fase_neg5");
         // three -8 passes then two -4 passes must equal five -4 passes:
         // both leave var22 at 5, so exactly one more -4 pass flips
         b.xt.fase = -7;
         b.step(); // var22 -> 0
         b.xt.fase = -8;
         b.steps(3);
         b.xt.fase = -4;
         b.steps(2);
         Check.eq(b.xt.fase, -4, "pacing_three_plus_two_passes_still_neg4");
         b.step();
         Check.eq(b.xt.fase, -5, "pacing_three_plus_two_equals_five");

         // ---- fase -4 with var22 >= 0: fleximage + the winner block
         // (:1318-1345). stage 1 == unlocked 1, so the 140,30 congrat draws.
         b.xt.winner = true;
         b.xt.fase = -4;
         b.step();
         Check.eq(b.xt.fase, -4, "matrix_neg4_winner_fase_kept");
         // var22 0 = the PixelGrabber/HSB pass; winner false = gameov
         b.xt.fase = -7;
         b.step();
         b.xt.winner = false;
         b.xt.fase = -4;
         b.step();
         Check.eq(b.xt.fase, -4, "matrix_neg4_fleximage_zero_fase_kept");
         b.step();
         Check.eq(b.xt.fase, -4, "matrix_neg4_gameov_fase_kept");
      } finally {
         b.shutdown();
      }
      Check.ok(b.threadDead(), "matrix_exit_thread_terminated");
   }

   // =====================================================================
   // 15 - the crown walk
   // =====================================================================

   /** B8(20,21) - the no-cookie boot walk
    *  111 -> 9 -> 10 -> 11 -> 10 -> 11 -> 7 -> 2 -> 1 -> 5 -> 176 -> 6 -> 0
    * on the real asset graph (the class comment explains why the trimmed
    * fixture cannot walk). Input script, every step through the real
    * handlers: the 111 arm's click is pre-latched as mouses=1 (mouseDown
    * itself is exwist-gated, pinned in inputPins); two keyDown(1005)
    * presses wrap maini's opselect 1 -> 2 -> 0; Enter (keyDown 10) in fase
    * 10, fase 11 (three times), fase 7 and fase 1. */
   private static void crownWalk() throws Throwable {
      // The canonical station trace and its pass bound are fixture data
      // (determinism.toml [walks], citing B8 pin 20 / ARCHITECTURE.md:1502).
      // Only the expected values move; the pump below is untouched.
      Toml walks = Toml.load("tests/fixtures/toml/determinism.toml").table("walks");
      int[] crown = walks.intArr("crown");
      int crownPasses = walks.intv("crown_passes");
      Check.eq(crown.length, 10, "walk_crown_stations_10");

      Boot b = Boot.start("assets");
      try {
         Check.eq(b.xt.fase, crown[1], "walk_111_click_fast_path_fase_9");
         Check.eq(b.app.mouses, 0, "walk_111_clears_mouses");
         Check.ok(!b.app.lostfcs, "walk_111_clears_lostfcs");
         Check.eq(b.parks(), 1, "walk_111_one_pass");

         b.steps(200);
         Check.eq(b.xt.fase, crown[2], "walk_9_two_hundred_rad_ticks_fase_10");
         Check.ok(!b.app.u[0].enter && !b.app.u[0].up && !b.app.u[0].down && !b.app.u[0].right && !b.app.u[0].left
               && !b.app.u[0].handb, "walk_9_falseo_all_false");

         // 10 -> 11 with opselect==1: oldfase becomes 10 (pin 21 - this is
         // why a no-input, cookie-less boot loops back to 10)
         b.xt.opselect = 1;
         b.key(10);
         Check.eq(b.xt.fase, crown[3], "walk_10_enter_fase_11");
         Check.eq(b.xt.oldfase, 10, "walk_10_opselect1_oldfase_10");
         Check.eq(b.xt.flipo, 1, "walk_10_inst_same_pass_flipo_1");
         b.key(10);
         Check.eq(b.xt.flipo, 2, "walk_11_flipo_2");
         b.key(10);
         Check.eq(b.xt.flipo, 4, "walk_11_flipo_4_same_call_double_step");
         b.key(10);
         Check.eq(b.xt.fase, crown[2], "walk_11_enter_returns_to_oldfase_10");
         Check.eq(b.xt.flipo, 0, "walk_11_enter_flipo_0");

         // two arrow-down presses wrap opselect 1 -> 2 -> 0 (pin 21)
         b.xt.opselect = 1;
         b.xt.oldfase = 0;
         b.key(1005);
         Check.eq(b.xt.opselect, 2, "walk_opselect_down_1_to_2");
         b.key(1005);
         Check.eq(b.xt.opselect, 0, "walk_opselect_down_2_to_0");
         b.key(10);
         Check.eq(b.xt.fase, crown[3], "walk_10_opselect0_fase_11");
         Check.eq(b.xt.oldfase, 7, "walk_10_opselect0_oldfase_7");
         // three enters: 1 -> 2, 2 -> 4, 4 -> fase 7
         b.key(10);
         Check.eq(b.xt.flipo, 2, "walk_11b_flipo_2");
         b.key(10);
         Check.eq(b.xt.flipo, 4, "walk_11b_flipo_4");
         b.key(10);
         Check.eq(b.xt.fase, crown[4], "walk_11_returns_to_carselect_7");

         // 7 -> 2 -> 1 in one pass (carselect :3695-3703 then the 2 arm
         // :803-807); loadingstage parks once inside the pass
         b.key(10);
         Check.eq(b.xt.fase, 2, "walk_7_carselect_fase_2_mid_pass");
         b.step();
         Check.eq(b.xt.fase, crown[5], "walk_2_loadstage_fase_1_same_pass");
         Check.eq(b.app.mouses, 0, "walk_7_to_1_clears_mouses");
         Check.eq(b.xt.starcnt, 90, "walk_loadstage_resetstat_90");
         Check.eq(b.app.nob, 66, "walk_loadstage_nob_66");
         Check.eq(b.app.notb, 28, "walk_loadstage_notb_28");
         Check.eq(b.medium().trx, -5600L, "walk_loadstage_trx");
         Check.eq(b.medium().trz, 15600L, "walk_loadstage_trz");
         // the promotion's same-pass fallthrough into the fase-1 arm already
         // ran trackbg/aroundtrack, which subtracts 3000 from loadstage's
         // 60000 (:454-457 aroundtrack)
         Check.eq(b.medium().hit, 57000, "walk_loadstage_hit_60000_then_aroundtrack");
         Check.eq(b.medium().ptcnt, -10, "walk_loadstage_ptcnt_minus10");
         Check.ok(b.medium().trk, "walk_loadstage_trk_true");

         // 1 -> 5 via stageselect's enter (:2513-2519)
         b.key(10);
         Check.eq(b.xt.fase, crown[6], "walk_1_stageselect_fase_5");
         Check.ok(!b.medium().trk, "walk_stageselect_trk_false");
         // 5 -> 176 (loadmusic, one park at its own repaint)
         b.step();
         Check.eq(b.xt.fase, crown[6], "walk_5_park_inside_loadmusic");
         b.step();
         Check.eq(b.xt.fase, crown[7], "walk_5_loadhold_80_fase_176");
         Check.ok(b.xt.loadedt[0], "walk_5_loadedt_0_true");
         b.neutralizeTracks();
         // the 176 countdown: 79 decrements to zero, then the 0 arm
         b.steps(79);
         Check.eq(b.xt.fase, crown[7], "walk_176_countdown_still_176");
         // the documented pass bound: the 295th parked pass is the last one
         // that still sees fase 176 (the countdown's own budget is 80)
         Check.eq(b.parks(), crownPasses, "walk_crown_passes_295");
         b.step();
         Check.eq(b.xt.fase, crown[8], "walk_176_countdown_fase_6");
         Check.ok(!b.app.u[0].enter && !b.app.u[0].handb, "walk_176_clears_enter_handb");
         // 6 -> 0 with Enter (musicomp) and the first race frame in the
         // same pass (:926-1070)
         b.key(10);
         Check.eq(b.xt.fase, crown[9], "walk_6_musicomp_fase_0");
         Check.eq(b.xt.starcnt, 89, "walk_0_first_frame_starcnt_89");
         Check.eq(b.medium().adv, 1880, "walk_0_adv_1880");
         Check.eq(b.medium().vxz, 74, "walk_0_vxz_74");
      } finally {
         b.shutdown();
      }
      // B8(22): exwist=true on a live thread ends run() through the exit
      // block (:1484-1490); gamer is nulled only if :1489 survives
      // Thread.stop, so the post-mortem reference is asserted loosely.
      Check.ok(b.threadDead(), "walk_exit_thread_terminated");
      Check.ok(b.app.gamer == null || !b.app.gamer.isAlive(), "walk_exit_gamer_post_mortem");
   }

   // =====================================================================
   // 16 - the music-missing 176 skip
   // =====================================================================

   /** B8(15/16) device-free half on the graphics-only codebase: loadmusic
    *  builds a RadicalMod whose Mod/ModSlayer chain fails on the absent
    *  music file (Mod.java:40-62, RadicalMod.java:37-50), so the ctor's
    *  catch leaves stream==null, loadedt stays false and the phase machine
    *  still lands on fase 176 without ever calling play(). The 176
    *  countdown's skip is pinned in the matrix above. */
   private static void musicMissingLoader() throws Exception {
      Direct d = directGfx();
      d.xt.loadedt[0] = false;
      d.xt.stracks[0] = null;
      d.xt.loadmusic(1, KGP.intv("loadhold"), d.app.rd);
      Check.eq(d.xt.fase, 176, "musicless_loadmusic_still_fase_176");
      Check.ok(!d.xt.loadedt[0], "musicless_loadedt_0_false");
      Check.ok(d.xt.stracks[0] != null, "musicless_stracks0_constructed");
      Check.ok(d.xt.stracks[0].stream == null, "musicless_stracks0_stream_null");
      Check.ok(!d.xt.stracks[0].playing, "musicless_stracks0_never_plays");
   }

   // =====================================================================
   // pumps
   // =====================================================================

   /** One pass = one repaint() call at the end of the dispatch section
    *  (GameSparker.java:1404). Park there, release exactly one pass, park
    *  again. The gate arms itself on the first repaint after run() flips
    *  exwist=false (:644), so the boot is bounded without racing pass 1. */
   static class Boot {
      final Seam app;
      final xtGraphics xt;

      private Boot(Seam app) {
         this.app = app;
         this.xt = app.xt();
      }

      static Boot start(String dir) throws Exception {
         Seam app = new Seam(dir, null, true);
         app.init();
         app.autoGate = true;
         // the 111 arm's click, latched before the loop starts (mouseDown
         // itself is exwist-gated; that machine is pinned in inputPins)
         app.mouses = 1;
         app.start();
         app.awaitPark();
         Boot b = new Boot(app);
         if (b.xt == null) {
            throw new IllegalStateException("xtGraphics was not captured");
         }
         // real music must never reach sun.audio: the three play() sites
         // this walk can hit are neutralized at their RadicalMod guard
         b.xt.stages.playing = true;
         b.xt.cars.playing = true;
         return b;
      }

      void neutralizeTracks() {
         for (int i = 0; i < xt.loadedt.length; i++) {
            if (xt.loadedt[i] && xt.stracks[i] != null) {
               xt.stracks[i].playing = true;
            }
         }
      }

      void step() {
         app.step();
      }

      void steps(int n) {
         for (int i = 0; i < n; i++) {
            app.step();
         }
      }

      /** keyDown + one pass (the handlers ignore the Event argument). */
      void key(int code) {
         app.exwist = false;
         app.keyDown(null, code);
         app.step();
      }

      int parks() {
         return app.parks;
      }

      Medium medium() {
         return xt.m;
      }

      void shutdown() throws Exception {
         app.shutdown();
      }

      boolean threadDead() {
         return app.threadDead();
      }
   }

   /** The pumping/observing subclass: codebase, context and asset seams. */
   static class Seam extends GameSparker {
      URL base;
      URL docbase;
      AppletContext ctx;
      final boolean recording;
      RecImage img;
      int createW = -1;
      int createH = -1;
      // the savecookie recorder (T1: the var37 latch's keys/values in call
      // order; there is no browser jar to read the values back from)
      final List<String> cookieKeys = new ArrayList<>();
      final List<String> cookieVals = new ArrayList<>();
      // pump gate
      final Object gate = new Object();
      volatile int passes;
      int parks;
      volatile boolean gateOn;
      volatile boolean parked;
      int permits;
      volatile boolean autoGate;

      Seam(String dir, AppletContext ctx, boolean recording) throws Exception {
         this.base = dirUrl(dir);
         this.docbase = base;
         this.ctx = ctx;
         this.recording = recording;
      }

      public URL getCodeBase() {
         return base;
      }

      public URL getDocumentBase() {
         return docbase;
      }

      public AppletContext getAppletContext() {
         return ctx;
      }

      /** Image seam: Applet.getImage needs an AppletStub (it NPEs at
       *  Applet.java:276 without one), so the bytes are fetched through the
       *  URL directly and a missing file becomes a 1x1 error image - the
       *  same shape a browser gives a missing gif. */
      public Image getImage(URL u, String n) {
         try {
            return Toolkit.getDefaultToolkit().createImage(readAll(new URL(u, n)));
         } catch (Exception e) {
            return Toolkit.getDefaultToolkit().createImage(new MemoryImageSource(1, 1, new int[]{0}, 0, 1));
         }
      }

      /** Audio seam (standing audio policy): no clip reaches a device. */
      public AudioClip getAudioClip(URL u, String n) {
         return SILENT;
      }

      public AudioClip getAudioClip(URL u) {
         return SILENT;
      }

      /** Records every savecookie call in order, then runs the real body
       *  (which swallows the no-JSObject exception, :1511-1517). */
      public void savecookie(String var1, String var2) {
         cookieKeys.add(var1);
         cookieVals.add(var2);
         super.savecookie(var1, var2);
      }

      void clearSaved() {
         cookieKeys.clear();
         cookieVals.clear();
      }

      int savedCount() {
         return cookieKeys.size();
      }

      /** "key=value,key=value,..." in call order - the latch's exact set. */
      String savedTrace() {
         StringBuilder sb = new StringBuilder();
         for (int i = 0; i < cookieKeys.size(); i++) {
            if (i > 0) {
               sb.append(',');
            }
            sb.append(cookieKeys.get(i)).append('=').append(cookieVals.get(i));
         }
         return sb.toString();
      }

      public Image createImage(int w, int h) {
         createW = w;
         createH = h;
         if (!recording) {
            return super.createImage(w, h);
         }
         img = new RecImage(w, h);
         return img;
      }

      public void repaint() {
         synchronized (gate) {
            passes++;
            if (autoGate && !gateOn && !exwist) {
               gateOn = true; // the first loop pass: arm the pump
            }
            if (!gateOn) {
               return;
            }
            parks++;
            parked = true;
            gate.notifyAll();
            while (permits == 0 && gateOn) {
               try {
                  gate.wait(100);
               } catch (InterruptedException e) {
                  // the pump's own interrupts land here; keep parked
               }
            }
            if (permits > 0) {
               permits--;
            }
            parked = false;
         }
      }

      xtGraphics xt() {
         return img == null ? null : img.g.seen;
      }

      void awaitPark() {
         synchronized (gate) {
            long deadline = System.currentTimeMillis() + 10000;
            while (!parked) {
               if (System.currentTimeMillis() > deadline) {
                  throw new RuntimeException("pump never parked (passes=" + passes + ")");
               }
               try {
                  gate.wait(5);
               } catch (InterruptedException e) {
               }
            }
         }
      }

      void step() {
         synchronized (gate) {
            int seen = parks;
            permits++;
            gate.notifyAll();
            long deadline = System.currentTimeMillis() + 10000;
            while (!(parked && parks > seen)) {
               if (System.currentTimeMillis() > deadline) {
                  throw new RuntimeException("pump step timeout (parks=" + parks + ")");
               }
               // only Thread.sleep (:1497-1500) sees this; it is swallowed
               gamer.interrupt();
               try {
                  gate.wait(1);
               } catch (InterruptedException e) {
               }
            }
         }
      }

      void releaseGate() {
         synchronized (gate) {
            gateOn = false;
            parked = false;
            permits = 0;
            gate.notifyAll();
         }
      }

      void shutdown() throws Exception {
         exwist = true;
         Thread t = gamer;
         releaseGate();
         if (t != null) {
            t.interrupt();
            t.join(10000);
         }
      }

      boolean threadDead() {
         Thread t = gamer;
         return t == null || !t.isAlive();
      }
   }

   /** The un-overridden sibling: the real createImage path, no
    *  AppletContext stub (so Applet's own NPE is observable) and no image
    *  or audio seam. */
   static class Bare extends GameSparker {
      final URL base;

      Bare(String dir) throws Exception {
         base = dirUrl(dir);
      }

      public URL getCodeBase() {
         return base;
      }

      public URL getDocumentBase() {
         return base;
      }
   }

   /** offImage stand-in that hands out a recording Graphics and captures
    *  the ImageObserver xtGraphics passes itself as
    *  (xtGraphics.java:2900-2906 draws with `this`). */
   static class RecImage extends BufferedImage {
      RecG g;

      RecImage(int w, int h) {
         super(w, h, TYPE_INT_RGB);
      }

      public Graphics getGraphics() {
         if (g == null) {
            g = new RecG(super.getGraphics());
         }
         return g;
      }
   }

   /** Delegating Graphics that records the last observer it was handed. */
   static class RecG extends Graphics {
      final Graphics d;
      volatile ImageObserver lastObserver;
      volatile xtGraphics seen;

      RecG(Graphics d) {
         this.d = d;
      }

      private void note(ImageObserver o) {
         lastObserver = o;
         if (o instanceof xtGraphics) {
            seen = (xtGraphics) o;
         }
      }

      public Graphics create() {
         return new RecG(d.create());
      }

      public Graphics create(int x, int y, int w, int h) {
         return new RecG(d.create(x, y, w, h));
      }

      public void translate(int x, int y) {
         d.translate(x, y);
      }

      public Color getColor() {
         return d.getColor();
      }

      public void setColor(Color c) {
         d.setColor(c);
      }

      public void setPaintMode() {
         d.setPaintMode();
      }

      public void setXORMode(Color c) {
         d.setXORMode(c);
      }

      public Font getFont() {
         return d.getFont();
      }

      public void setFont(Font f) {
         d.setFont(f);
      }

      public FontMetrics getFontMetrics(Font f) {
         return d.getFontMetrics(f);
      }

      public Rectangle getClipBounds() {
         return d.getClipBounds();
      }

      public void clipRect(int x, int y, int w, int h) {
         d.clipRect(x, y, w, h);
      }

      public void setClip(int x, int y, int w, int h) {
         d.setClip(x, y, w, h);
      }

      public Shape getClip() {
         return d.getClip();
      }

      public void setClip(Shape s) {
         d.setClip(s);
      }

      public void copyArea(int x, int y, int w, int h, int dx, int dy) {
         d.copyArea(x, y, w, h, dx, dy);
      }

      public void drawLine(int x1, int y1, int x2, int y2) {
         d.drawLine(x1, y1, x2, y2);
      }

      public void fillRect(int x, int y, int w, int h) {
         d.fillRect(x, y, w, h);
      }

      public void clearRect(int x, int y, int w, int h) {
         d.clearRect(x, y, w, h);
      }

      public void drawRoundRect(int x, int y, int w, int h, int aw, int ah) {
         d.drawRoundRect(x, y, w, h, aw, ah);
      }

      public void fillRoundRect(int x, int y, int w, int h, int aw, int ah) {
         d.fillRoundRect(x, y, w, h, aw, ah);
      }

      public void drawOval(int x, int y, int w, int h) {
         d.drawOval(x, y, w, h);
      }

      public void fillOval(int x, int y, int w, int h) {
         d.fillOval(x, y, w, h);
      }

      public void drawArc(int x, int y, int w, int h, int sa, int aa) {
         d.drawArc(x, y, w, h, sa, aa);
      }

      public void fillArc(int x, int y, int w, int h, int sa, int aa) {
         d.fillArc(x, y, w, h, sa, aa);
      }

      public void drawPolyline(int[] xs, int[] ys, int n) {
         d.drawPolyline(xs, ys, n);
      }

      public void drawPolygon(int[] xs, int[] ys, int n) {
         d.drawPolygon(xs, ys, n);
      }

      public void drawPolygon(java.awt.Polygon p) {
         d.drawPolygon(p);
      }

      public void fillPolygon(int[] xs, int[] ys, int n) {
         d.fillPolygon(xs, ys, n);
      }

      public void fillPolygon(java.awt.Polygon p) {
         d.fillPolygon(p);
      }

      public void drawString(String s, int x, int y) {
         d.drawString(s, x, y);
      }

      public void drawString(java.text.AttributedCharacterIterator it, int x, int y) {
         d.drawString(it, x, y);
      }

      public void drawChars(char[] data, int off, int len, int x, int y) {
         d.drawChars(data, off, len, x, y);
      }

      public void drawBytes(byte[] data, int off, int len, int x, int y) {
         d.drawBytes(data, off, len, x, y);
      }

      public boolean drawImage(Image i, int x, int y, ImageObserver o) {
         note(o);
         return d.drawImage(i, x, y, o);
      }

      public boolean drawImage(Image i, int x, int y, int w, int h, ImageObserver o) {
         note(o);
         return d.drawImage(i, x, y, w, h, o);
      }

      public boolean drawImage(Image i, int x, int y, Color c, ImageObserver o) {
         note(o);
         return d.drawImage(i, x, y, c, o);
      }

      public boolean drawImage(Image i, int x, int y, int w, int h, Color c, ImageObserver o) {
         note(o);
         return d.drawImage(i, x, y, w, h, c, o);
      }

      public boolean drawImage(Image i, int dx1, int dy1, int dx2, int dy2, int sx1, int sy1, int sx2, int sy2,
            ImageObserver o) {
         note(o);
         return d.drawImage(i, dx1, dy1, dx2, dy2, sx1, sy1, sx2, sy2, o);
      }

      public boolean drawImage(Image i, int dx1, int dy1, int dx2, int dy2, int sx1, int sy1, int sx2, int sy2,
            Color c, ImageObserver o) {
         note(o);
         return d.drawImage(i, dx1, dy1, dx2, dy2, sx1, sy1, sx2, sy2, c, o);
      }

      public void dispose() {
         d.dispose();
      }
   }

   /** The direct-call graph. run() builds exactly these objects
    *  (GameSparker.java:583-605); the tests build them by hand so
    *  loadbase/loadstage/loadmusic can be driven without a thread. */
   static class Direct {
      final Seam app;
      final Medium medium = new Medium();
      final Trackers trackers = new Trackers();
      final CheckPoints cp = new CheckPoints();
      final xtGraphics xt;
      final Record record;
      final ContO[] models = new ContO[43];
      final ContO[] obj = new ContO[210];
      final Madness[] madness = new Madness[5];
      final Control[] u = new Control[5];

      Direct(String dir) throws Exception {
         app = new Seam(dir, null, false);
         app.rd = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB).getGraphics();
         xt = new xtGraphics(medium, app.rd, app, 0);
         record = new Record(medium);
         for (int i = 0; i < 5; i++) {
            madness[i] = new Madness(medium, record, xt, i);
            u[i] = new Control(medium);
            app.u[i] = u[i];
         }
         app.loadbase(models, medium, trackers);
      }
   }

   /** A lightweight holder for loadbase-only tests (no xtGraphics cost). */
   static class Holder {
      final Seam app;
      final Medium medium = new Medium();
      final Trackers trackers = new Trackers();
      final ContO[] models = new ContO[43];

      Holder(String dir) throws Exception {
         app = new Seam(dir, null, false);
      }
   }

   private static Direct directAssets;
   private static Direct directGfx;

   static Direct directAssets() throws Exception {
      if (directAssets == null) {
         directAssets = new Direct("assets");
      }
      return directAssets;
   }

   static Direct directGfx() throws Exception {
      if (directGfx == null) {
         directGfx = new Direct(fixtureGfxOnly());
      }
      return directGfx;
   }

   /** The silent clip (audio policy). */
   static final AudioClip SILENT = new AudioClip() {
      public void play() {
      }

      public void loop() {
      }

      public void stop() {
      }
   };

   /** A stub AppletContext that records showDocument calls. */
   static class StubContext implements AppletContext {
      final List<String> calls = new ArrayList<String>();

      public AudioClip getAudioClip(URL u) {
         return SILENT;
      }

      public Image getImage(URL u) {
         return null;
      }

      public java.applet.Applet getApplet(String n) {
         return null;
      }

      public Enumeration<java.applet.Applet> getApplets() {
         return null;
      }

      public void showDocument(URL u) {
         calls.add(u.toString());
      }

      public void showDocument(URL u, String target) {
         calls.add(u + " target=" + target);
      }

      public void showStatus(String s) {
      }

      public void setStream(String k, InputStream s) {
      }

      public InputStream getStream(String k) {
         return null;
      }

      public Iterator<String> getStreamKeys() {
         return null;
      }
   }

   /** The "sun." prefix the plugin's viewer context provides. */
   static class SunContext extends StubContext {
      public String toString() {
         return "sun.applet.AppletViewer@deadbeef";
      }
   }

   // =====================================================================
   // fixture directories
   // =====================================================================

   static URL dirUrl(String dir) throws Exception {
      String u = new File(dir).toURI().toURL().toExternalForm();
      if (!u.endsWith("/")) {
         u = u + "/"; // without the slash relative URLs resolve as siblings
      }
      return new URL(u);
   }

   /** assets/graphics only: the models row loads, stages/ and music/ are
    *  absent (the clean missing-stage arm and the music-missing 176 skip). */
   static String fixtureGfxOnly() throws IOException {
      File root = new File("target/java-tests-fixtures/gfxonly");
      File marker = new File(root, "graphics/models.zipo");
      if (!marker.isFile()) {
         deleteTree(root);
         copyTree(new File("assets/graphics"), new File(root, "graphics"));
      }
      return root.getPath();
   }

   /** Nothing at all: loadbase falls into its printed-error arm. */
   static String fixtureEmpty() throws IOException {
      File root = new File("target/java-tests-fixtures/empty");
      root.mkdirs();
      return root.getPath();
   }

   /** A two-entry models.zipo that isolates the LAST-prefix-match rule and
    *  the var10==0 fallback for an entry that matches no prefix. */
   static String fixtureLastMatch() throws IOException {
      File root = new File("target/java-tests-fixtures/lastmatch");
      File zipFile = new File(root, "graphics/models.zipo");
      new File(root, "graphics").mkdirs();
      zipFile.delete();
      byte[] payload = modelEntry("offroadend.rad");
      ZipOutputStream z = new ZipOutputStream(new FileOutputStream(zipFile));
      // STORED, not deflated: the real models.zipo entries carry a real
      // getSize() and loadbase allocates `new byte[size]` from it (:230), so
      // a -1 (deflated, size unknown) throws NegativeArraySizeException and
      // lands in the printed-error arm instead.
      z.putNextEntry(storedEntry("offroadendx.rad", payload));
      z.write(payload);
      z.closeEntry();
      z.putNextEntry(storedEntry("zzz.rad", payload));
      z.write(payload);
      z.closeEntry();
      z.close();
      return root.getPath();
   }

   static ZipEntry storedEntry(String name, byte[] payload) {
      java.util.zip.CRC32 crc = new java.util.zip.CRC32();
      crc.update(payload);
      ZipEntry e = new ZipEntry(name);
      e.setMethod(ZipEntry.STORED);
      e.setSize(payload.length);
      e.setCompressedSize(payload.length);
      e.setCrc(crc.getValue());
      return e;
   }

   static byte[] modelEntry(String name) throws IOException {
      ZipInputStream z = new ZipInputStream(new FileInputStream("assets/graphics/models.zipo"));
      try {
         for (ZipEntry e = z.getNextEntry(); e != null; e = z.getNextEntry()) {
            if (e.getName().equals(name)) {
               ByteArrayOutputStream b = new ByteArrayOutputStream();
               byte[] buf = new byte[65536];
               int n;
               while ((n = z.read(buf)) > 0) {
                  b.write(buf, 0, n);
               }
               return b.toByteArray();
            }
         }
      } finally {
         z.close();
      }
      throw new IOException("no " + name + " in models.zipo");
   }

   static byte[] readAll(URL u) throws IOException {
      InputStream in = u.openStream();
      ByteArrayOutputStream b = new ByteArrayOutputStream();
      byte[] buf = new byte[65536];
      int n;
      while ((n = in.read(buf)) > 0) {
         b.write(buf, 0, n);
      }
      in.close();
      return b.toByteArray();
   }

   static void copyTree(File from, File to) throws IOException {
      if (from.isDirectory()) {
         to.mkdirs();
         File[] kids = from.listFiles();
         for (int i = 0; i < kids.length; i++) {
            copyTree(kids[i], new File(to, kids[i].getName()));
         }
      } else {
         InputStream in = new FileInputStream(from);
         OutputStream out = new FileOutputStream(to);
         byte[] buf = new byte[65536];
         int n;
         while ((n = in.read(buf)) > 0) {
            out.write(buf, 0, n);
         }
         in.close();
         out.close();
      }
   }

   static void deleteTree(File f) {
      if (f.isDirectory()) {
         File[] kids = f.listFiles();
         for (int i = 0; i < kids.length; i++) {
            deleteTree(kids[i]);
         }
      }
      f.delete();
   }

   static int nonZeroPixels(BufferedImage img) {
      int n = 0;
      for (int y = 0; y < img.getHeight(); y++) {
         for (int x = 0; x < img.getWidth(); x++) {
            if ((img.getRGB(x, y) & 0xffffff) != 0) {
               n++;
            }
         }
      }
      return n;
   }
}
