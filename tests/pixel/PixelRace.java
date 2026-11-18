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
import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.util.Arrays;
import java.util.Enumeration;
import java.util.Iterator;

/**
 * RACE-FRAME pins: the container-pinned pixel platform extended to the fase-0
 * race loop, the fase-0 HUD/result overlays and the stage fixture graph.
 *
 * WHY A SEPARATE CLASS (and not more methods in PixelSuite): the whole suite
 * shares ONE xtGraphics/Medium/Control fixture built at handler level. The
 * race cannot use it - the race graph (Medium, Trackers, CheckPoints, Record,
 * the 43-slot model pool, the 210-slot object pool and the five Madness
 * polls) is built INSIDE GameSparker.run() as locals (GameSparker.java:583-605)
 * and the fase-0 arm draws those locals (:926-1070). The only faithful way to a
 * race frame is therefore to RUN the original loop and park it - so this class
 * owns a second, throw-away applet whose repaint() is a deterministic per-pass
 * gate. Keeping it in its own file keeps PixelSuite's "no run(), no threads"
 * policy intact: that policy is amended here, not relaxed there.
 * tools/pixel-tests.sh already compiles /w/tests/pixel/*.java, so no script
 * change is needed.
 *
 * =========================================================================
 * 1. RANDOMNESS: the main()-start injection (orchestrator-decided)
 * =========================================================================
 * A race frame consumes java.util.Random in ways no preset can reach: the AI
 * picks in sortcars (xtGraphics.java:2278-2351), Medium.random()'s 3-draw
 * refills (Medium.java:49-60) every 20th call (driving/plane-emboss reads:
 * ContO.d -> Plane.d, Madness.drive), the fleximage dither (:400-427) and the
 * playsounds air-loop / stunt-adj picks (:832,1783,2078...). A preset Medium
 * walk covers N draws and then silently falls back to Math.random(), so a
 * race "preset" would pin a window, not a race.
 *
 * The suite therefore points Math.random() at a seeded java.util.Random at the
 * VERY START of PixelSuite.main() (TestRng.install(RACE_SEED), the first
 * statement, before any game class is loaded). This is exactly the
 * "TJavaRandom slot" pattern: JDK 8u40 keeps the generator in
 * Math$RandomNumberGeneratorHolder.randomNumberGenerator (static final, the
 * final bit cleared by TestRng), and C2 constant-folds that slot at compile
 * time - so the injection MUST precede the first compilation of a
 * Math.random() call path.
 *
 * The Suite-ordering rule stated in TestRng's header ("compiled code keeps the
 * PRE-injection instance when installed late") does NOT apply here:
 *   - PixelSuite is a SEPARATE, COLD-JIT process. main() installs the seed as
 *     its first statement, i.e. before Font.createFont, before the fixture is
 *     copied, before any game class is even loaded. Nothing that can call
 *     Math.random() has run, so whichever generator C2 folds in is the
 *     injected one.
 *   - The existing handler-level screens cannot observe the change: they run
 *     on the preset Medium walk (PixelSuite.preset(): rand={5,5,5},
 *     diup=false, trn=0, cntrn=1,000,000) and never hit the refill branch of
 *     Medium.random(), so they consume ZERO draws from Math.random(). The 43
 *     pre-existing screen rows are therefore injection-invariant - verify mode
 *     proves it before and after this batch (B12 §8).
 * The seed is FIXED and documented: RACE_SEED = 20050310L (the game's release
 * month, 2005-03, + 10 for the 2005-03 era build). Changing it changes every
 * race md5; it is part of the platform contract like the font bytes.
 *
 * =========================================================================
 * 2. THE DRIVE: graph built directly, no boot walk, no device
 * =========================================================================
 * Seam (a GameSparker subclass, the pattern of TGameSparkerLogic.SEAM):
 *   getCodeBase/getDocumentBase -> the REAL assets/ tree (music present: the
 *     fase-1 flyover's trackbg() calls stages.posit() = stream.available(),
 *     RadicalMod.java:78-80, which NPEs without the file); assets/ also holds
 *     graphics/, stages/ and sounds/, so every loader in the original works.
 *   getAppletContext -> a "sun."-prefixed toString (sunytyp()==2 ->
 *     var3=2 -> kbload=514 + the newsun sound dir; GameSparker.java:268-274).
 *   getAudioClip -> the shared SILENT stub (PixelSuite.SILENT).
 *   getImage -> URL bytes -> Toolkit.createImage (Applet.getImage needs a stub
 *     and NPEs without one; TGameSparkerLogic's finding).
 *   createImage(550,400) -> a BufferedImage-backed recording image, so init()
 *     (GameSparker.java:1503-1508) has an offImage/rd on an unrealized applet
 *     and the frame we hash is the REAL persistent offImage.
 *   readcookie -> the applet's own cookie seam. Without a browser it returns
 *     -1 (GameSparker.java:150-161); here it returns the STAGE for "unlocked"
 *     and -1 for everything else. run() then sets xt.unlocked and
 *     CheckPoints.stage to the same value (:610-621) - the documented cookie
 *     contract, no reflection, no shadow graph. Stage 1 through this seam is
 *     bit-identical to a cookie-less boot (unlocked==stage==1 defaults).
 *   repaint -> the per-pass gate (see 3).
 *
 * Graph: run()'s own prologue is the graph builder - xtGraphics on the real
 * paks, loadbase on the real models.zipo (:594), the five Madness/Control
 * pairs, and Record. No loadmusic is ever called: stracks[] stay null and
 * loadedt[] false, so every mutem/track-stop/track-resume arm (:904-919, stat
 * :1645-1655) no-ops. The fase-2 load is then driven directly:
 *   pass 1   fase 111 (clicknow) - discarded, it is the first gate arm point
 *   pass 2   fx.fase = 2 -> loadingstage() draws + parks, loadstage() builds
 *            the 210-slot object pool from stages/<cookie>.txt, resetstat()
 *            sets starcnt=90/gocnt=3 and sortcars() picks the AI cars
 *   pass 3   fx.fase = 0 -> first fase-0 pass
 * The fase-1 flyover arm is deliberately NOT allowed to run: loadstage's
 * promotion is gated on `xt.fase == 2` (:527-556), so setting fase=0 while the
 * pump is parked inside loadingstage skips the promotion (trx/trz/trk stay
 * unset - they are the flyover camera only) and the fase-1 arm's
 * stageselect() -> stages.play() -> AudioPlayer.player.start(stream)
 * (RadicalMod.java:66-73) never reaches the device. This is strictly stronger
 * than TGameSparkerLogic's `playing=true` trick: no RadicalMod field is
 * written at all. NOTE the media policy: the two RadicalMods run() constructs
 * ARE built (Mod parses music/stages.zipo + music/cars.zipo) but a constructed
 * RadicalMod has playing==false, and stop()/posit() on it never open a device
 * (RadicalMod.java:8-16, 77-80).
 *
 * =========================================================================
 * 3. THREADS (amendment, same shape as TGameSparkerLogic's)
 * =========================================================================
 * The pixel platform's "no threads" rule is amended here exactly as the java
 * gate amended it: a bounded pump is allowed because run() is the only code
 * that can draw a race frame. Every pump is a daemon-less Thread started by
 * start(), parked inside OUR repaint() override (run() calls repaint() exactly
 * once per pass at GameSparker.java:1404, plus the loading/ loadingstage
 * repaints, which happen while exwist is still true and therefore do not arm
 * the gate), released one pass at a time, and torn down by shutdown()
 * (exwist=true, gate released, interrupt, join(10000)). The loop's own
 * Thread.sleep(:1493-1500) is short-circuited by interrupt()-while-waiting -
 * the catch at :1497-1500 swallows it, so no state changes, only wall time.
 * No thread survives this class; nothing is ever made visible (no
 * setVisible/show/pack, no Frame, no window).
 *
 * =========================================================================
 * 4. WHAT IS PINNED (all full 550x400 frames; ids = state they capture)
 * =========================================================================
 * The countdown ladder is driven on stage 1 with FIXED inputs (all five
 * Control flags false every pass; the AI cars drive from the injected stream),
 * pinning the frame after each named pass:
 *   starcnt 90  the reset pass: starcnt==90 -> adv=1900/zy=40/vxz=70, then the
 *               whole frame is white-filled (:1032-1037); around(cars[3],true)
 *               then runs (adv 1880, vxz 74)
 *   starcnt 89  the first decrement; first frame that shows the new camera
 *   starcnt 35  the landing: mouses=0, vert=false, adv=900, vxz=180,
 *               checkstat+follow+stat, then white-filled again (:1067-1069).
 *               Its stat call is the race's FIRST, so it takes m.flex 0 -> 1 -
 *               and the fillRect wipes the HUD it just drew (the two white
 *               frames are byte-identical; asserted).
 *   starcnt 34  the first VISIBLE follow()+stat frame: m.flex 1 -> 2, full
 *               panel (rank/lap/wasted images + drawstat + cntdn[3])
 *   starcnt 33/32  m.flex==2: the incremental rank/lap/wasted arm over the
 *               persistent offImage (Medium.d's own flex==2 csky band,
 *               :230-239); the two frames are identical (nothing changed).
 *   starcnt 0   the first pass that runs colide/drive/rec/checkstat/preform
 *               (:1000-1012) - the full race composite
 *   gocnt 2/1/0 the countdown sprite machine (:1723-1745) at starcnt 24/13/2
 * mid-race md5s at 10/50/100 physics passes prove the physics+HUD composite.
 * The secondary pins are all drawn ON a copy of the real 100-tick frame (so
 * their backdrop is the original's own sky/ground/cars, not a synthetic fill):
 *   stat result panels (wasted==4 / player-wasted / AI-first / player-first),
 *     with m.flex and holdcnt set exactly per :1515-1680 - including the
 *     flex==0 csky erase rectangle under the result image;
 *   drawstat bar variants (:2730-2860): damage full/mid/low, the newcar
 *     sky-erase polygon, and the auscnt/aflk power-bar flash;
 *   the arrow machine (missed checkpoint / cars-onscreen / wrong-way flash)
 *     via stat's own arrow block (:1690-1721);
 *   levelhigh's three titles (:744-755);
 *   pauseimage() and fleximage() transforms (:1130-1177, :371-430).
 * stage 5 and stage 10 each get a first-race-frame md5 through the same
 * pump (the countdown is short-circuited by setting starcnt=0 at the park, so
 * only the fixture graph - track file, car picks, snap/fade/ground - differs).
 * The fallback end-of-race screen the real race reaches is also pinned (the
 * -2 arm took the no-catch branch -> fase -4, trackbg + mdness).
 *
 * NOT PINNED, and why: the fase -3 replay's starcar restore. Record.starcar[]
 * IS populated for free (loadstage -> Record.reset with prepit==true,
 * Record.java:223-234), and the -2 arm zeroes the var23/var22 counters before
 * -3 (:1160-1170) - but ONLY on the `record.hcaught` branch; without a caught
 * collision the -2 arm goes to fase -4 (gameover) instead, which is the branch
 * this race actually takes (pinned). The phase -3 pass that matters
 * (var22==0 restore) is black-filled by :1261-1264 anyway, and the pass
 * counters only advance every 67th pass when whenwasted==0, so a -3 pin here
 * would be ~70 wasted passes for a screen whose first visible frame is the
 * levelhigh replay already pinned above.
 *
 * REPRODUCIBILITY NORMALISATION (the one correction the race forced, B12 §8.4):
 * loadstage's own `sky(...)` line calls xt.snap(stage), which rebuilds the HUD
 * images from the pak images through PixelGrabber. On this JDK the FIRST grab
 * right after the GIF decode race-returns a stale bottom row / transparent key
 * in about half the runs (measured: dmg/pwr/was/rank/cntdn/gameh changed by a
 * row or two, flipping whole panels), so the derived image was NOT stable.
 * loadStage() therefore re-runs the original's own public snap(stage) once the
 * sources are provably complete - same call, same stage, same m.snap - and only
 * AFTER the white-filled pass that consumes no recoloured image. With that
 * normalisation every frame here is byte-identical across runs (verified 4x).
 *
 * Wall time: see B12 §8 (measured: 1.8-2.0 s of pump inside the container, no
 * sleeps - the gate interrupts the loop's own 30 ms Thread.sleep).
 */
public class PixelRace {
   // =====================================================================
   // entry (called from PixelSuite.main, after the handler-level screens)
   // =====================================================================

   /** The documented fixed seed of the race platform: fixture data
    *  (determinism.toml [seed] pixel_main). PixelSuite.main() reads the same
    *  key as its first statement; this constant is the class-local view. */
   static final long RACE_SEED = Toml.load("tests/fixtures/toml/determinism.toml")
         .table("seed").intv("pixel_main");

   /** constants.toml [xt] - the countdown ladder and the camera resets the
    *  race pump pins (the PASS STRUCTURE stays in this class). */
   static final Toml KXT = Toml.load("tests/fixtures/toml/constants.toml").table("xt");

   /** Dev aid only: -Dpixel.race.trace=1 prints id + md5 + state per frame;
    *  -Dpixel.race.dump=<dir> writes every frame (and the derived HUD images)
    *  as raw big-endian ARGB so a cross-run diff can localise a fragile pin.
    *  Neither property is set by tools/pixel-tests.sh, so verify mode is
    *  silent. */
   static final boolean TRACE = System.getProperty("pixel.race.trace") != null;

   /** Where the real asset tree lives (the container mounts the repo at /w
    *  and the suite runs with cwd=/w). Overridable for offline probing. */
   static String assetsDir() {
      return System.getProperty("pixel.race.assets", "assets");
   }

   static void pins() throws Throwable {
      Check.group("Pixel");
      long t0 = System.currentTimeMillis();

      Race one = Race.open(1);
      try {
         one.loadStage();
         dumpImages(one);
         countdownLadder(one);
         midRace(one);
         variants(one);        // copies of the 100-tick frame: stat/drawstat/arrow/...
         transforms(one);      // pauseimage/fleximage (last: they burn m.random())
         afterRace(one);       // the -2 branch (replay vs gameover)
      } finally {
         one.shutdown();
      }

      Race five = Race.open(5);
      try {
         five.loadStage();
         five.toRaceStart();
         pinFrame("race_stage5_starcnt0_first_race_frame", five);
      } finally {
         five.shutdown();
      }

      Race ten = Race.open(10);
      try {
         ten.loadStage();
         ten.toRaceStart();
         pinFrame("race_stage10_starcnt0_first_race_frame", ten);
      } finally {
         ten.shutdown();
      }

      long ms = System.currentTimeMillis() - t0;
      System.out.println("PixelRace: pump wall time " + ms + " ms");
   }

   // =====================================================================
   // the countdown ladder (stage 1)
   // =====================================================================

   static void countdownLadder(Race r) {
      // pass 1 of the loop was fase 111; loadStage() used pass 2 (loadingstage
      // park + loadstage) and pass 3 (the starcnt==90 reset pass).
      Check.eq(r.xt.starcnt, KXT.intv("starcnt_start") - 1, "race_stage1_starcnt90_reset_exits_89");
      Check.eq(r.xt.m.adv, KXT.intv("camera_adv_start") - KXT.intv("camera_around_adv_step"),
            "race_stage1_starcnt90_adv_1900_then_around_minus20");
      Check.eq(r.xt.m.vxz, KXT.intv("camera_vxz_start") + KXT.intv("camera_around_vxz_step"),
            "race_stage1_starcnt90_vxz_70_then_around_plus4");
      Check.ok(uniformly(r.frame(), 0xffffff), "race_stage1_starcnt90_reset_frame_is_white");
      String resetWhite = PixelSuite.md5(r.frame());
      pinFrame("race_stage1_starcnt90_reset_white", r);

      r.step(); // enters 89 -> 88: the first pass that draws the reset camera
      Check.eq(r.xt.starcnt, KXT.intv("starcnt_start") - 2, "race_stage1_starcnt89_first_decrement_exits_88");
      pinFrame("race_stage1_starcnt89_first_decrement", r);

      stepUntil(r, KXT.intv("starcnt_landing"));
      // the landing: decremented 36 -> 35, then the `== 35` block ran. It
      // calls stat() (which bumps m.flex 0 -> 1) and THEN white-fills the
      // frame, so the landing's HUD is painted but never seen.
      Check.eq(r.xt.starcnt, KXT.intv("starcnt_landing"), "race_stage1_starcnt35_landing_observed_35");
      Check.eq(r.xt.m.adv, KXT.intv("camera_adv_landing"), "race_stage1_starcnt35_landing_adv_900");
      Check.eq(r.xt.m.vxz, KXT.intv("camera_vxz_landing"), "race_stage1_starcnt35_landing_vxz_180");
      Check.ok(!r.xt.m.vert, "race_stage1_starcnt35_landing_vert_false");
      Check.eq(r.app.mouses, 0, "race_stage1_starcnt35_landing_mouses_0");
      // the landing pass is the FIRST stat() call of the race: it takes the
      // flex!=2 full-panel arm and leaves flex=1 behind for the next pass
      Check.eq(r.xt.m.flex, 1, "race_stage1_starcnt35_landing_flex_0_to_1");
      Check.ok(uniformly(r.frame(), 0xffffff), "race_stage1_starcnt35_landing_frame_is_white");
      Check.eq(PixelSuite.md5(r.frame()), resetWhite, "race_stage1_reset_and_landing_white_frames_identical");
      pinFrame("race_stage1_starcnt35_landing_white", r);

      r.step(); // enters 35 -> 34: the first VISIBLE follow()+stat frame
      Check.eq(r.xt.starcnt, KXT.intv("starcnt_visible"), "race_stage1_starcnt34_first_visible_hud_observed_34");
      Check.eq(r.xt.m.flex, 2, "race_stage1_starcnt34_full_panel_flex_1_to_2");
      Check.eq(r.xt.gocnt, KXT.intv("gocnt_start"), "race_stage1_starcnt34_gocnt_3");
      pinFrame("race_stage1_starcnt34_first_visible_hud_full_panel", r);

      r.step(); // enters 34 -> 33: m.flex==2, the incremental rank/lap arm
      Check.eq(r.xt.m.flex, 2, "race_stage1_incremental_first_flex_stays_2");
      String incremental = PixelSuite.md5(r.frame());
      pinFrame("race_stage1_incremental_hud_first", r);

      r.step(); // enters 33 -> 32: a second incremental pass
      Check.eq(r.xt.m.flex, 2, "race_stage1_incremental_second_flex_stays_2");
      Check.eq(PixelSuite.md5(r.frame()), incremental, "race_stage1_incremental_pass_is_idempotent");
      pinFrame("race_stage1_incremental_hud_second", r);

      stepUntil(r, KXT.intv("starcnt_two"));
      Check.eq(r.xt.gocnt, KXT.intv("gocnt_two"), "race_stage1_gocnt2_at_starcnt24");
      pinFrame("race_stage1_gocnt2_starcnt24", r);

      stepUntil(r, KXT.intv("starcnt_one"));
      Check.eq(r.xt.gocnt, KXT.intv("gocnt_one"), "race_stage1_gocnt1_at_starcnt13");
      pinFrame("race_stage1_gocnt1_starcnt13", r);

      stepUntil(r, KXT.intv("starcnt_go"));
      Check.eq(r.xt.gocnt, KXT.intv("gocnt_go"), "race_stage1_gocnt0_at_starcnt2");
      pinFrame("race_stage1_gocnt0_go_sprite", r);

      stepUntil(r, KXT.intv("starcnt_race"));
      // the pass that reaches 0 has NOT yet run the physics block (the
      // decrement and the starcnt==0 test are the same two branches)
      Check.eq(r.xt.starcnt, KXT.intv("starcnt_race"), "race_stage1_countdown_reaches_0");

      r.step(); // first physics pass: colide/drive/rec/checkstat/preform
      Check.eq(r.xt.starcnt, KXT.intv("starcnt_race"), "race_stage1_first_physics_frame_starcnt_0");
      Check.eq(r.xt.holdcnt, 0, "race_stage1_no_result_panel_holdcnt_0");
      Check.ok(!r.xt.winner, "race_stage1_no_result_panel_winner_false");
      r.physics = 1;
      pinFrame("race_stage1_starcnt0_full_race_frame", r);
   }

   // =====================================================================
   // mid-race determinism (the injected stream, not a preset window)
   // =====================================================================

   static void midRace(Race r) {
      // the player car is sc[0] (no usercar cookie): 0 = 2000tornados
      Check.eq(r.xt.sc[0], 0, "race_stage1_player_car_0");
      // sortcars picked four DISTINCT rivals from the injected stream
      int distinct = 1;
      for (int i = 1; i < 5; i++) {
         boolean seen = false;
         for (int j = 0; j < i; j++) {
            if (r.xt.sc[i] == r.xt.sc[j]) {
               seen = true;
            }
         }
         if (!seen) {
            distinct++;
         }
      }
      Check.eq(distinct, 5, "race_stage1_five_distinct_cars");
      Check.eq(r.xt.sc[4], 5, "race_stage1_stage1_rival_car_4_plus_half");

      // 10, 50, 100 physics passes after the first one
      stepTo(r, 10);
      pinFrame("race_stage1_tick10_midrace", r);
      stepTo(r, 50);
      pinFrame("race_stage1_tick50_midrace", r);
      stepTo(r, 100);
      pinFrame("race_stage1_tick100_midrace", r);
      // the race is still live: no result panel, no phase change
      Check.eq(r.xt.fase, 0, "race_stage1_tick100_still_fase_0");
      Check.eq(r.xt.cntwis, 0, "race_stage1_tick100_cntwis_0");
      // remember the canonical backdrop for the composite pins
      Race.BACKDROP = copy(r.frame());
   }

   static void stepTo(Race r, int tick) {
      while (r.physics < tick) {
         r.step();
         r.physics++;
      }
   }

   /** Step until the countdown field reaches `starcnt` (bounded: a stuck
    *  countdown must fail loudly, not hang the container). */
   static void stepUntil(Race r, int starcnt) {
      int guard = 0;
      while (r.xt.starcnt > starcnt) {
         if (++guard > 400) {
            throw new IllegalStateException(
                  "countdown stuck at " + r.xt.starcnt + " waiting for " + starcnt + " (guard=" + guard + ")");
         }
         r.step();
      }
   }

   // =====================================================================
   // stat / drawstat / arrow / levelhigh over the REAL race frame
   // =====================================================================

   /** The stat machine (:1515-1680) is a composite: the panel images and the
    *  HUD are drawn over whatever the caller's Graphics holds. Every pin here
    *  starts from a PIXEL COPY of the real 100-tick frame and from the
    *  handler-level fields TxtGraphicsLogic established (resetStat), so the
    *  only difference between two pins is the forced arm. */
   static void variants(Race r) {
      xtGraphics x = r.xt;
      Font f = frameFont(r);
      Check.eq(f.getFamily(), "SansSerif", "race_frame_font_family_sansserif");
      Check.eq(f.getFontName(), "SansSerif.bold", "race_frame_font_name_sansserif_bold");
      Check.eq(f.getStyle(), Font.BOLD, "race_frame_font_style_bold");
      Check.eq(f.getSize(), 11, "race_frame_font_size_11");
      // stat()/levelhigh()/arrow() draw with the Graphics' current font while
      // ftm (already computed) supplies the centring widths, so the composite
      // pins must reuse this exact font object.

      // ---- stat: wasted==4 (the winner panel). flex==2 is the steady state
      // (Medium.d resets flex to 0 each around(), the HUD bumps it to 2 and
      // stat then takes the incremental arm); flex 0 is the full-redraw arm
      // whose csky fillRect sits UNDER the result image (:1535-1541).
      resetStat(r);
      x.wasted = 4;
      x.m.flex = 2;
      x.holdcnt = 0;
      BufferedImage base = Race.BACKDROP;
      BufferedImage img = copy(base);
      Graphics g = img.getGraphics();
      g.setFont(f);
      r.ctl.enter = false;
      CheckPoints cp = freshCp();
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      Check.ok(x.winner, "race_stat_wasted4_winner_true");
      Check.ok(cp.haltall, "race_stat_wasted4_haltall_true");
      Check.eq(x.holdcnt, 1, "race_stat_wasted4_holdcnt_1");
      pinFrame("race_stat_wasted4_flex2_winner_panel", img);

      // the same panel with flex==0: the csky erase rectangle is painted
      // first, so the result image sits on a flat sky patch instead of the
      // race backdrop - a different full frame, same text.
      resetStat(r);
      x.wasted = 4;
      x.m.flex = 0;
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, freshCp(), r.ctl, true, g);
      g.dispose();
      Check.eq(x.m.flex, 1, "race_stat_wasted4_flex0_bumped_to_1");
      pinFrame("race_stat_wasted4_flex0_csky_erase", img);

      // holdcnt>250 is the auto-continue gate (:1616-1622): fase -> -2
      resetStat(r);
      x.wasted = 4;
      x.m.flex = 2;
      x.holdcnt = 251;
      x.fase = 0;
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, freshCp(), r.ctl, false, g);
      g.dispose();
      Check.eq(x.fase, -2, "race_stat_holdcnt_251_auto_continue_fase_minus2");
      Check.eq(x.holdcnt, 252, "race_stat_holdcnt_251_incremented_252");

      // ---- stat: player wasted (var1.dest && cntwis==8, :1540-1552)
      resetStat(r);
      x.cntwis = 8;
      r.mad.dest = true;
      x.m.flex = 2;
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      cp = freshCp();
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      Check.ok(!x.winner, "race_stat_player_wasted_winner_false");
      Check.ok(!cp.haltall, "race_stat_player_wasted_haltall_untouched");
      pinFrame("race_stat_player_wasted_panel", img);

      // ---- stat: AI first (pos[1]==0 and clear==nlaps*nsp for rival 1)
      resetStat(r);
      x.m.flex = 2;
      cp = freshCp();
      cp.pos[0] = 4;
      cp.pos[1] = 0;
      cp.clear[0] = 0;
      cp.clear[1] = 0; // == nlaps(0) * nsp(0)
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      Check.ok(!x.winner, "race_stat_ai_first_winner_false");
      Check.ok(cp.haltall, "race_stat_ai_first_haltall_true");
      pinFrame("race_stat_ai_first_panel", img);

      // ---- stat: player finished first (pos[0]==0)
      resetStat(r);
      x.m.flex = 2;
      cp = freshCp();
      cp.pos[0] = 0;
      cp.clear[0] = 0;
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      Check.ok(x.winner, "race_stat_player_first_winner_true");
      pinFrame("race_stat_player_first_panel", img);

      // ---- the arrow machine (:1690-1721) through stat's own call path.
      // CheckPoints opx/opz/onscreen/dested are the values checkstat() would
      // have written from the car table; they are set explicitly so the pin
      // does not depend on where the pack happened to be.
      resetStat(r);
      x.m.flex = 2;
      x.starcnt = KXT.intv("starcnt_race");
      x.arrace = false;
      r.ctl.arrace = false;
      x.ana = 0;
      x.flk = false;
      r.mad.missedcp = 20;  // 15 < missedcp < 50 -> "CheckPoint Missed!"
      r.mad.point = 1;
      cp = arrowCp();
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      Check.eq(x.flk, true, "race_arrow_missedcp_flk_toggles_to_true");
      Check.eq(r.mad.missedcp, 21, "race_arrow_missedcp_incremented");
      pinFrame("race_arrow_missedcp_flash_orange", img);

      // second call: the arrow polygon's flash machine (:575-625) has now
      // flipped flk, so the same text draws with the other colour
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, arrowCp(), r.ctl, true, g);
      g.dispose();
      Check.eq(x.flk, false, "race_arrow_missedcp_flk_toggles_back");
      pinFrame("race_arrow_missedcp_flash_red", img);

      // ---- cars-onscreen arrow (arrace==true): the "[ name ]" box + the
      // nearest not-dested rival (:470-500)
      resetStat(r);
      x.m.flex = 2;
      x.starcnt = KXT.intv("starcnt_race");
      x.arrace = true;
      r.ctl.arrace = true;
      r.mad.missedcp = 0;
      cp = arrowCp();
      cp.onscreen[1] = 40; // rival 1 is on screen -> preferred
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      pinFrame("race_arrow_cars_onscreen", img);

      // ---- wrong way (:1700-1717): arrow() runs FIRST and smooths ana, then
      // the block needs |ana|>100 to raise cntan past 40 while mtouch holds
      // and cntovn<70. ana starts at 170 so one arrow step cannot drop it
      // under the threshold.
      resetStat(r);
      x.m.flex = 2;
      x.starcnt = KXT.intv("starcnt_race");
      x.arrace = false;
      x.ana = 170;
      x.cntan = 50;
      x.cntovn = 0;
      x.flk = false;
      r.ctl.arrace = false;
      r.mad.missedcp = 0;
      r.mad.mtouch = true;
      cp = arrowCp();
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, cp, r.ctl, true, g);
      g.dispose();
      Check.eq(x.cntovn, 1, "race_arrow_wrongway_cntovn_1");
      Check.eq(x.cntan, 40, "race_arrow_wrongway_cntan_clamped_40");
      Check.eq(x.flk, true, "race_arrow_wrongway_flk_true");
      pinFrame("race_arrow_wrongway_flash_red", img);

      resetStat(r);
      x.m.flex = 2;
      x.starcnt = KXT.intv("starcnt_race");
      x.ana = 170;
      x.cntan = 50;
      x.cntovn = 0;
      x.flk = true;   // the other half of the flash machine
      r.mad.missedcp = 0;
      r.mad.mtouch = true;
      img = copy(base);
      g = img.getGraphics();
      g.setFont(f);
      x.stat(r.mad, arrowCp(), r.ctl, true, g);
      g.dispose();
      Check.eq(x.flk, false, "race_arrow_wrongway_flk_false");
      pinFrame("race_arrow_wrongway_flash_orange", img);

      // ---- drawstat (:2730-2860): damage bar + power bar over the HUD.
      // var4 (newcar) selects the sky-erase polygon (:2733-2744); var2==0 is
      // the ArithmeticException already pinned at handler level, so it is not
      // re-driven here.
      drawstat(r, "race_drawstat_dmg_full_pwr_max", 100, 100, false, 98.0F, 45, false);
      drawstat(r, "race_drawstat_dmg_mid_pwr_half", 100, 60, false, 50.0F, 45, false);
      drawstat(r, "race_drawstat_dmg_low_newcar_erase", 100, 10, true, 20.0F, 45, false);
      drawstat(r, "race_drawstat_pwr_low_flash", 100, 60, false, 30.0F, 44, true);

      // ---- levelhigh's three titles (:744-755): wasted!=0 -> "You wasted
      // 'em!", wasted==0 && whenwasted==229 -> "Wasted!", else "Stunts!"
      levelhigh(r, "race_levelhigh_you_wasted_em", 4, 0);
      levelhigh(r, "race_levelhigh_wasted", 0, 229);
      levelhigh(r, "race_levelhigh_stunts", 0, 0);

      // ---- the flex machine's two full-panel entries (0 and 1) are the SAME
      // code path; prove it paints identically so a "flex 0/1 full panels"
      // pin would be a duplicate. (The pumped race never *shows* flex==0:
      // the white-filled landing pass takes it, :1535-1541 + :1067-1069.)
      resetStat(r);
      x.m.flex = 0;
      x.starcnt = KXT.intv("starcnt_race");
      BufferedImage a0 = copy(Race.BACKDROP);
      Graphics gg = a0.getGraphics();
      gg.setFont(f);
      x.stat(r.mad, freshCp(), r.ctl, true, gg);
      gg.dispose();
      String flex0 = PixelSuite.md5(a0);
      resetStat(r);
      x.m.flex = 1;
      x.starcnt = KXT.intv("starcnt_race");
      BufferedImage a1 = copy(Race.BACKDROP);
      gg = a1.getGraphics();
      gg.setFont(f);
      x.stat(r.mad, freshCp(), r.ctl, true, gg);
      gg.dispose();
      Check.eq(PixelSuite.md5(a1), flex0, "race_stat_flex0_and_flex1_arms_paint_identically");
   }

   static void drawstat(Race r, String id, int max, int hit, boolean newcar, float power, int auscnt,
         boolean aflk) {
      resetStat(r);
      r.xt.m.flex = 2;
      r.xt.auscnt = auscnt;
      r.xt.aflk = aflk;
      BufferedImage img = copy(Race.BACKDROP);
      Graphics g = img.getGraphics();
      g.setFont(frameFont(r));
      r.xt.drawstat(g, max, hit, newcar, power);
      g.dispose();
      pinFrame(id, img);
   }

   static void levelhigh(Race r, String id, int wasted, int whenwasted) {
      BufferedImage img = copy(Race.BACKDROP);
      Graphics g = img.getGraphics();
      g.setFont(frameFont(r));
      r.xt.levelhigh(g, wasted, whenwasted);
      g.dispose();
      pinFrame(id, img);
   }

   // =====================================================================
   // the transforms (pauseimage / fleximage) on a real race frame
   // =====================================================================

   static void transforms(Race r) {
      // pauseimage (:1130-1177): PixelGrabber over the persistent offImage ->
      // per-pixel luma + the 188-row blue band -> a new MemoryImageSource
      // frame drawn over the target. It also writes m.flex=0.
      BufferedImage img = copy(Race.BACKDROP);
      Graphics g = img.getGraphics();
      r.xt.pauseimage(r.frame(), g);
      g.dispose();
      Check.eq(r.xt.m.flex, 0, "race_pauseimage_sets_m_flex_0");
      pinFrame("race_pauseimage_over_race_frame", img);

      // fleximage phase 0 (:371-430): the HSB hue-rotate pass (0.7 hue) then
      // the dithered darken; phases >0 skip the HSB pass.
      img = copy(Race.BACKDROP);
      g = img.getGraphics();
      r.xt.fleximage(r.frame(), g, 0);
      g.dispose();
      pinFrame("race_fleximage_phase0_hsb_hue_rotate", img);

      img = copy(Race.BACKDROP);
      g = img.getGraphics();
      r.xt.fleximage(r.frame(), g, 3);
      g.dispose();
      pinFrame("race_fleximage_phase3_dither_only", img);
   }

   // =====================================================================
   // after the race: the -2 branch (replay -3 vs gameover -4)
   // =====================================================================

   static void afterRace(Race r) {
      r.xt.fase = -2;
      r.ctl.enter = false;
      r.ctl.handb = false;
      r.app.mouses = 0;
      r.step();
      int fase = r.xt.fase;
      Check.ok(fase == -3 || fase == -4, "race_neg2_branch_is_minus3_or_minus4");
      if (TRACE) {
         trace("neg2", "fase=" + fase + " (hcaught=" + (fase == -3) + ")");
      }
      if (fase == -3) {
         // var22==0 restore of record.starcar[] + levelhigh, then the black
         // fill at :1261-1264. whenwasted==0 makes var23 climb to 67 before
         // var22 advances, so the first VISIBLE frame is 67 passes later.
         pinFrame("race_neg3_starcar_restore_black_hold", r);
         int guard = 0;
         while (guard++ < 80) {
            r.step();
            if (!uniformly(r.frame(), 0x000000)) {
               break;
            }
         }
         Check.ok(!uniformly(r.frame(), 0x000000), "race_neg3_first_visible_frame_reached");
         pinFrame("race_neg3_levelhigh_stunts_replay", r);
      } else {
         // the -4 arm: trackbg + the mdness logo at (164,330) (:1318-1321);
         // var22==-2 there, so fleximage() is NOT called on this screen.
         pinFrame("race_neg4_gameover_mdness_over_trackbg", r);
      }
   }

   // =====================================================================
   // pin plumbing
   // =====================================================================

   static void pinFrame(String id, Race r) {
      BufferedImage img = r.frame();
      String md5 = PixelSuite.md5(img);
      if (TRACE) {
         trace(id, md5 + " fase=" + r.xt.fase + " starcnt=" + r.xt.starcnt + " gocnt=" + r.xt.gocnt
               + " flex=" + r.xt.m.flex + " passes=" + r.app.passes + " tick=" + r.physics);
      }
      dump(id, img);
      PixelSuite.pin(id, md5);
   }

   static void pinFrame(String id, BufferedImage img) {
      String md5 = PixelSuite.md5(img);
      if (TRACE) {
         trace(id, md5 + " composite");
      }
      dump(id, img);
      PixelSuite.pin(id, md5);
   }

   /** Dev aid only: -Dpixel.race.dump=<dir> also writes the derived HUD
    *  images as raw ARGB, so a cross-run diff can separate "the image bytes
    *  differ" from "the blit is racy". */
   static void dumpImages(Race r) {
      String dir = System.getProperty("pixel.race.dump");
      if (dir == null) {
         return;
      }
      xtGraphics x = r.xt;
      dumpImage(dir, "img_dmg", x.dmg);
      dumpImage(dir, "img_pwr", x.pwr);
      dumpImage(dir, "img_was", x.was);
      dumpImage(dir, "img_lap", x.lap);
      dumpImage(dir, "img_pos", x.pos);
      dumpImage(dir, "img_gameh", x.gameh);
      dumpImage(dir, "img_odmg", x.odmg);
      dumpImage(dir, "img_opwr", x.opwr);
      dumpImage(dir, "img_rank0", x.rank[0]);
      dumpImage(dir, "img_cntdn3", x.cntdn[3]);
   }

   static void dumpImage(String dir, String id, Image im) {
      if (im == null) {
         System.out.println("IMGDUMP " + id + " null");
         return;
      }
      int w = im.getWidth(null);
      int h = im.getHeight(null);
      int[] px = new int[w * h];
      java.awt.image.PixelGrabber pg = new java.awt.image.PixelGrabber(im, 0, 0, w, h, px, 0, w);
      try {
         pg.grabPixels();
      } catch (InterruptedException e) {
      }
      System.out.println("IMGDUMP " + id + " " + w + "x" + h + " md5=" + PixelSuite.md5(px));
      try {
         byte[] b = new byte[px.length * 4];
         for (int i = 0; i < px.length; i++) {
            b[i * 4] = (byte) (px[i] >>> 24);
            b[i * 4 + 1] = (byte) (px[i] >>> 16);
            b[i * 4 + 2] = (byte) (px[i] >>> 8);
            b[i * 4 + 3] = (byte) px[i];
         }
         java.io.OutputStream out = new java.io.BufferedOutputStream(
               new java.io.FileOutputStream(new java.io.File(dir, id + ".argb")));
         out.write(b);
         out.close();
      } catch (IOException e) {
         throw new RuntimeException(e);
      }
   }

   /** Dev aid only: -Dpixel.race.dump=<dir> writes each frame as raw
    *  big-endian ARGB bytes (no ImageIO in the pinned path). */
   static void dump(String id, BufferedImage img) {      String dir = System.getProperty("pixel.race.dump");
      if (dir == null) {
         return;
      }
      try {
         int[] px = PixelSuite.pixels(img);
         byte[] b = new byte[px.length * 4];
         for (int i = 0; i < px.length; i++) {
            b[i * 4] = (byte) (px[i] >>> 24);
            b[i * 4 + 1] = (byte) (px[i] >>> 16);
            b[i * 4 + 2] = (byte) (px[i] >>> 8);
            b[i * 4 + 3] = (byte) px[i];
         }
         java.io.OutputStream out = new java.io.BufferedOutputStream(
               new java.io.FileOutputStream(new java.io.File(dir, id + ".argb")));
         out.write(b);
         out.close();
      } catch (IOException e) {
         throw new RuntimeException(e);
      }
   }

   static void trace(String id, String state) {
      System.out.println("TRACE " + id + " " + state);
   }

   /** The font the race Graphics currently carries: stat/levelhigh/arrow call
    *  drawcs() without setting a font, so the glyphs rasterize with this one
    *  while ftm (already computed) supplies the widths. It must be the same
    *  11pt SansSerif.bold the loadingstage() draw left behind. */
   static Font frameFont(Race r) {
      return r.frame().getGraphics().getFont();
   }

   static boolean uniformly(BufferedImage img, int rgb) {
      int[] px = PixelSuite.pixels(img);
      for (int i = 0; i < px.length; i++) {
         if ((px[i] & 0xffffff) != rgb) {
            return false;
         }
      }
      return true;
   }

   static BufferedImage copy(BufferedImage src) {
      BufferedImage out = new BufferedImage(src.getWidth(), src.getHeight(), BufferedImage.TYPE_INT_RGB);
      out.setRGB(0, 0, src.getWidth(), src.getHeight(), PixelSuite.pixels(src), 0, src.getWidth());
      return out;
   }

   /** handler-level reset of every field stat/drawstat/arrow/levelhigh read -
    *  the same field set TxtGraphicsLogic.resetAll uses (fase/starcnt/flex are
    *  set per pin afterwards). */
   static void resetStat(Race r) {
      xtGraphics x = r.xt;
      x.fase = 0;
      x.starcnt = KXT.intv("starcnt_race");
      x.gocnt = 0;
      x.holdcnt = 0;
      x.winner = false;
      x.wasted = 0;
      x.cntwis = 0;
      x.posit = 0;
      x.laps = 0;
      x.clear = 0;
      x.arrace = false;
      x.ana = 0;
      x.cntan = 0;
      x.cntovn = 0;
      x.flk = false;
      x.tcnt = 30;
      x.tflk = false;
      x.wasay = false;
      x.say = "";
      x.dmcnt = 0;
      x.dmflk = false;
      x.pwcnt = 0;
      x.pwflk = false;
      x.auscnt = 45;
      x.aflk = false;
      x.crashup = false;
      x.skflg = 0;
      x.dskflg = 0;
      x.loop = "";
      x.spin = "";
      x.asay = "";
      Arrays.fill(x.dested, 0);

      Madness mad = r.mad;
      mad.dest = false;
      mad.missedcp = 0;
      mad.point = 0;
      mad.mtouch = false;
      mad.hitmag = 0;
      mad.newcar = false;
      mad.nlaps = 0;
      mad.clear = 0;
      mad.power = 75.0F;
      mad.im = 0;
      mad.cn = 0;

      r.ctl.enter = false;
      r.ctl.handb = false;
      r.ctl.arrace = false;
      r.ctl.up = false;
      r.ctl.down = false;
      r.ctl.left = false;
      r.ctl.right = false;
      r.ctl.mutes = false;
      r.ctl.mutem = false;
   }

   static CheckPoints freshCp() {
      CheckPoints cp = new CheckPoints();
      cp.nlaps = 0;
      cp.nsp = 0;
      cp.stage = 1;
      cp.wasted = 0;
      cp.haltall = false;
      return cp;
   }

   /** The opacity state checkstat() would have written: the player at the
    *  origin and one rival ahead-right and on screen. */
   static CheckPoints arrowCp() {
      CheckPoints cp = freshCp();
      cp.n = 8;
      cp.opx[0] = 0;
      cp.opz[0] = 0;
      cp.opx[1] = 900;
      cp.opz[1] = 1800;
      cp.onscreen[0] = 40;
      cp.onscreen[1] = 40;
      cp.onscreen[2] = 0;
      cp.onscreen[3] = 0;
      cp.onscreen[4] = 0;
      cp.x[0] = 1200;
      cp.z[0] = 2400;
      cp.x[1] = 900;
      cp.z[1] = 1800;
      for (int i = 0; i < 5; i++) {
         cp.dested[i] = 0;
      }
      return cp;
   }

   // =====================================================================
   // the pump seam (game-side)
   // =====================================================================

   static class Race {
      /** The real 100-tick frame, reused as the composite backdrop. */
      static BufferedImage BACKDROP;

      final App app;
      final xtGraphics xt;
      final Madness mad;
      final Control ctl;
      final int stage;
      int physics;

      private Race(App app, int stage) {
         this.app = app;
         this.xt = app.xt;
         this.stage = stage;
         this.mad = new Madness(xt.m, new Record(xt.m), xt, 0);
         this.ctl = new Control(xt.m);
      }

      static Race open(int stage) throws Exception {
         App a = new App(PixelSuite.dirUrl(assetsDir()), stage);
         a.init();
         a.autoGate = true;
         a.start();
         a.awaitPark();
         if (a.xt == null) {
            throw new IllegalStateException("run() never handed xtGraphics to the recording Graphics");
         }
         return new Race(a, stage);
      }

      /** pass 2 parks inside loadingstage(); setting fase=0 there skips
       *  loadstage's `fase==2` promotion, so the fase-1 flyover arm (and its
       *  stages.play()) never runs. pass 3 is then the starcnt==90 reset. */
      void loadStage() {
         xt.fase = 2;
         app.step();
         xt.fase = 0;
         app.step();
         // The stage's `sky(...)` line made loadstage call xt.snap(stage),
         // which re-derives the HUD images from the raw pak images with
         // PixelGrabber. On this JDK/Toolkit the very first grab after the
         // GIF decode races the image producer (measured: the bottom row /
         // the transparent key came back stale in half the runs, changing
         // dmg/pwr/was/rank/cntdn/gameh by rows), so the derived image is
         // NOT reproducible. Re-running the original's own snap() once the
         // sources are provably complete (the same public call, the same
         // stage, the same m.snap) is the pixel platform's normalisation -
         // see B12 §8.4. No pixel drawn before this point contains a
         // recoloured image, so the countdown's white frames are unaffected.
         xt.snap(stage);
      }

      /** Jump the countdown: the next pass enters starcnt==0 and runs the
       *  physics block. Used for the stage-5/10 fixture pins only. */
      void toRaceStart() {
         xt.starcnt = KXT.intv("starcnt_race");
         app.step();
      }

      void step() {
         app.step();
      }

      BufferedImage frame() {
         return app.frame;
      }

      void shutdown() throws Exception {
         app.shutdown();
      }
   }

   /** The pumping GameSparker: codebase/context/audio/image seams + the
    *  per-pass park gate. Pattern: TGameSparkerLogic.Seam (proven there). */
   static class App extends GameSparker {
      final URL base;
      final int stageCookie;
      xtGraphics xt;
      RecImg frame;
      final Object gate = new Object();
      volatile int passes;
      int parks;
      volatile boolean gateOn;
      volatile boolean parked;
      int permits;
      volatile boolean autoGate;

      App(URL base, int stage) {
         this.base = base;
         this.stageCookie = stage;
      }

      public URL getCodeBase() {
         return base;
      }

      public URL getDocumentBase() {
         return base;
      }

      public AppletContext getAppletContext() {
         return CTX;
      }

      public Image getImage(URL u, String n) {
         try {
            return Toolkit.getDefaultToolkit().createImage(PixelSuite.readAll(new URL(u, n)));
         } catch (Exception e) {
            return Toolkit.getDefaultToolkit().createImage(new MemoryImageSource(1, 1, new int[]{0}, 0, 1));
         }
      }

      public AudioClip getAudioClip(URL u, String n) {
         return PixelSuite.SILENT;
      }

      public AudioClip getAudioClip(URL u) {
         return PixelSuite.SILENT;
      }

      /** testlocation() seam (:28-90). JSObject.getWindow needs the Oracle
       *  deploy runtime: plugin.jar's implementation links
       *  com/sun/deploy/appcontext/AppContext, and on this image that call
       *  dies with NoClassDefFoundError - a LinkageError, NOT the JSException
       *  the source's catch (:91) expects - so run() would lose its thread
       *  before the first pass. For a file: document base the real method's
       *  catch arm is a documented no-op (it draws nothing and never calls
       *  gamer.stop(); TGameSparkerLogic's
       *  testlocation_file_base_draws_nothing/no_gamer pins), which is
       *  exactly this override. See B12 §8.5. */
      public void testlocation() {
      }

      /** init()'s 550x400 offImage is the recording one; any later request
       *  (none in the original) would get a plain image. */
      public Image createImage(int w, int h) {
         if (frame == null) {
            frame = new RecImg(this, w, h);
            return frame;
         }
         return new BufferedImage(w, h, BufferedImage.TYPE_INT_RGB);
      }

      /** The browser cookie seam: the applet's own readcookie() (:150-161)
       *  returns -1 with no JSObject; here the run() prologue sees the stage
       *  in "unlocked" exactly as a fresh browser cookie would show it, so
       *  xt.unlocked and CheckPoints.stage are the same value. */
      public int readcookie(String name) {
         return "unlocked".equals(name) ? stageCookie : -1;
      }

      public void repaint() {
         synchronized (gate) {
            passes++;
            if (autoGate && !gateOn && !exwist) {
               gateOn = true;
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
                  // step()'s interrupts land here; stay parked
               }
            }
            if (permits > 0) {
               permits--;
            }
            parked = false;
         }
      }

      void awaitPark() {
         synchronized (gate) {
            long deadline = System.currentTimeMillis() + 60000;
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
            long deadline = System.currentTimeMillis() + 60000;
            while (!(parked && parks > seen)) {
               if (System.currentTimeMillis() > deadline) {
                  throw new RuntimeException("pump step timeout (parks=" + parks + ")");
               }
               // only Thread.sleep(:1493-1500) sees this
               gamer.interrupt();
               try {
                  gate.wait(1);
               } catch (InterruptedException e) {
               }
            }
         }
      }

      void shutdown() throws Exception {
         exwist = true;
         Thread t = gamer;
         synchronized (gate) {
            gateOn = false;
            parked = false;
            permits = 0;
            gate.notifyAll();
         }
         if (t != null) {
            t.interrupt();
            t.join(10000);
         }
      }
   }

   static final AppletContext CTX = new Ctx();

   static class Ctx implements AppletContext {
      public String toString() {
         return "sun.applet.AppletViewer@pixelrace";
      }

      public AudioClip getAudioClip(URL u) {
         return PixelSuite.SILENT;
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
      }

      public void showDocument(URL u, String t) {
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

   /** The persistent offImage: its pixels are the race frame, and its
    *  getGraphics() hands out the observer-recording delegate so run()'s own
    *  xtGraphics (created locally at GameSparker.java:591) is captured on the
    *  first drawImage(..., this) inside xtGraphics.loading(). */
   static class RecImg extends BufferedImage {
      RecG g;

      RecImg(App app, int w, int h) {
         super(w, h, TYPE_INT_RGB);
         g = new RecG(super.getGraphics(), app);
      }

      public Graphics getGraphics() {
         return g;
      }
   }

   /** Delegating Graphics that records the observer it was handed. */
   static class RecG extends Graphics {
      final Graphics d;
      final App app;
      volatile xtGraphics seen;

      RecG(Graphics d, App app) {
         this.d = d;
         this.app = app;
      }

      private void note(ImageObserver o) {
         if (o instanceof xtGraphics) {
            xtGraphics x = (xtGraphics) o;
            if (seen == null) {
               seen = x;
               app.xt = x;
            }
         }
      }

      public Graphics create() {
         return new RecG(d.create(), app);
      }

      public Graphics create(int x, int y, int w, int h) {
         return new RecG(d.create(x, y, w, h), app);
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
}
