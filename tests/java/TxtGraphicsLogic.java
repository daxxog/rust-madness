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
import java.io.PrintStream;
import java.net.URL;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Enumeration;
import java.util.Iterator;
import java.util.List;

/**
 * Pins xtGraphics - the whole screen layer: the menu state machine, the HUD,
 * the audio hub, the image-pak loaders and the draw/pure helpers the Rust
 * port mirrors 1:1. Source of truth is reference/vineflower/xtGraphics.java
 * (4110 lines); every expectation below was re-derived from that source and
 * then measured against the unmodified original on the gate's Oracle 8u40.
 *
 * Pin source: notes/JAVA-BEHAVIOR/B10_xtgraphics.md. B10 rows the source
 * contradicts are corrected here (and reported back into B10):
 *   - B10(3) "drawstat ... the (244,187,11) colour": the channel formula at
 *     :2763 is `(int)(244.0F - 233.0F * ((float)(var8 - 33) / 65.0F))`. With
 *     max=100/hit=50 var8==49, 244 - 233*16/65 == 186.646, and the (int)
 *     cast TRUNCATES: the colour is (244,186,11) == 0xf4ba0b, not 187.
 *   - B10(3) "drawstat(g,0,...) -> ArithmeticException (`98*hit/0`)": :2750 is
 *     FLOAT division, so there is no exception. `var3 > var2` clamps hit to 0
 *     first (:2746), 0.0F/0.0F == NaN and (int)NaN == 0, so the bar is simply
 *     zero-width. Pinned as the no-throw/zero-width contract.
 *   - B10(2) "sparkeng(-1) stops all 5": wrong way round. sparkeng does
 *     `var1++` first (:2386), so sparkeng(-1) is index 0 - it STARTS slot 0
 *     and stops the other four; the stop-all call is sparkeng(-2) (index -1,
 *     no slot matches) or any argument >= 4.
 *   - B10(1) "stageselect on Control.handb==true -> fase=5": the guard is
 *     `var3.handb || var3.enter` (:2513); enter is the second trigger.
 *   - B10(3) "pauseimage ... tints the band starting at index 550*106+156":
 *     the condition is `var8 > 550 * (106 + var5) + 156` (:1159) - STRICTLY
 *     greater - so 550*106+156 is still grey and 550*106+157 is the first
 *     tinted pixel.
 *   - B10(3) "stat ... always haltall=true (:1535,1598)": the
 *     `var1.dest && cntwis==8` "you were wasted" branch (:1540-1552) sets
 *     winner=false and var6=true but NEVER writes haltall. Only the
 *     wasted==4 block (:1535) and the race-finished block (:1598) do.
 *   - B10(seams) "new Madness() for stat/playsounds/carselect": Madness has
 *     exactly one constructor, `Madness(Medium, Record, xtGraphics, int)`
 *     (Madness.java:279); there is no no-arg form.
 *   - B10(2) "checkpoint plays when clear != var1.clear": in stat() the
 *     first parameter is the MADNESS (var1), the CheckPoints is var2, so
 *     the counter compared is Madness.clear (:2045) - never
 *     CheckPoints.clear[]. Pinned with a Madness.clear fixture.
 *   - B10(5) says nothing about sortcars(11), and the intuitive "stage 11
 *     leaves sc[4] alone" is wrong: :2352 skips only the sc[4] WRITE, it
 *     keeps var3==5, so :2357 still iterates var4 to 4 and sc[4] is filled
 *     like sc[1..3] (range 0..9 via 5 + (10+1)/2).
 *
 * OPEN ITEMS SETTLED (probed, see the PINS below):
 *   - scratch codebase layout: the ctor resolves the three welcome images by
 *     BARE name against getCodeBase() (:2935,2943,2951), i.e. codebase ROOT.
 *     With hello/sign/loadbar copied to the fixture root they are REAL
 *     (550x217, 75x66, 238x21); against the shipped tree - which keeps them
 *     under graphics/ - all three come back as the seam's 1x1 error image.
 *     The shipped layout works because a browser supplies a delayed Image;
 *     the ctor only ever addImage()s them (:2936,2944,2952), so the game
 *     itself never notices. The fixture therefore copies them to the root.
 *   - drawImage(null,...) on this JDK 8: SunGraphics2D does NOT throw. It
 *     returns true and draws nothing, for both the observer and the 4-arg
 *     form, so every still-null image field (e.g. rad's rpro, :3411) is a
 *     silently empty blit.
 *
 * FIXTURES (built once under target/java-tests-fixtures/xt ... dirs, marker-gated):
 *   - xtfull/    assets/graphics + assets/music + hello/sign/loadbar at ROOT
 *                (the ctor's three welcome images are real; 4 real paks)
 *   - xtsparse/  graphics/images3.zipo + images4.zipo ONLY, no welcome gifs,
 *                no music; constructed with var4==0 (the "default" sound dir)
 *   - xtno3/     xtfull minus graphics/images3.zipo  (ctor NPE arm)
 *   - xtno4/     xtfull minus graphics/images4.zipo  (ctor NPE arm)
 *
 * CTOR BUDGET: the constructor is expensive (4 zips, ~36 AudioClips, 2
 * RadicalMod decodes, MediaTracker waits, ~20 loading() repaints, System.gc())
 * so exactly FOUR constructor invocations in the whole class: xtfull (shared
 * by every group), xtsparse, and the two NPE fixtures (which never yield an
 * object). Measured cold: 749ms / 64ms / 41ms / 40ms (894ms total; the
 * first pays the AWT font/hazard init); warm inside the full suite the
 * class adds ~0.6-0.7s to the gate's wall time.
 *
 * DEVICE POLICY: every AudioClip the ctor asks for comes from the applet
 * seam as one shared SILENT stub, and every RadicalMod the class can reach
 * (stages/cars/stracks) is replaced after construction by a recording
 * RecMod whose play/resume/stop/outwithit/posit only append to a log. The
 * ctor's own two RadicalMods (music/cars.zipo, music/stages.zipo) are
 * decoded but never played by the ctor - this was verified, so the claim
 * that AudioClips alone are device-free is only half the story: stageselect
 * (:2486), carselect (:3558), loadmusic's var2==0 arm (:1328) and
 * pausedgame's resume (:1419,1428) WOULD open sun.audio if left real. They
 * are all neutralized by the RecMod seam before any test calls them.
 *
 * DETERMINISM: no TestRng and no Math.random injection. The only added
 * entropy is Medium.random(), and consumers get the TControl recipe presetted
 * explicitly (cntrn=100, rand={5,5,5}, diup={false,false,false}, trn=0 ->
 * 0.4,0.3,0.2,0.1,0.0,0.9,... for the first 100 draws). sortcars() uses
 * STOCK Math.random (xtGraphics.java:2289,2299,2325,2342,2366,2376), so only
 * its deterministic outputs (sc[4], the unlocked==stage branch) and
 * random-independent PROPERTIES (distinctness, the forced 7 on stage 10) are
 * pinned. Draw calls are STATEFUL (m.flex, aflk, holdcnt, ana, flipo), so
 * every group resets the fields it depends on explicitly; reset() below is
 * the single list.
 *
 * Every handler is driven directly, one call per pin - this class pins the
 * HANDLER-LEVEL contracts, which is a different granularity from
 * TGameSparkerLogic's run() dispatcher walk (that class already pins the
 * 111->9->10->11->10->11->7->2->1->5->176->6->0 crown walk and the four
 * ctachm mouse-latch arms; nothing here re-pins those observables in the
 * same form).
 */
public class TxtGraphicsLogic {
   /** constants.toml [xt] - one parse for the whole class. */
   static final Toml KX = Toml.load("tests/fixtures/toml/constants.toml").table("xt");

   /** 0xRRGGBB hex used by the Rec log pins. */
   static String rgbHex(int r, int g, int b) {
      return String.format("%02x%02x%02x", r, g, b);
   }

   // =====================================================================
   // entry
   // =====================================================================

   static void run() throws Throwable {
      Check.group("xtGraphics");
      KX.requireKeys("drawstat_red", "pause_band_first_tint", "kbload_newsun",
            "cntwis_dest", "starcnt_start", "starcnt_landing", "starcnt_two",
            "camera_adv_landing");
      CARS = loadCars();
      ctorPins();
      pureHelperPins();
      screenPins();
      audioPins();
      drawPins();
   }

   // =====================================================================
   // shared fixtures / seams
   // =====================================================================

   /** The silent clip (standing audio policy: no clip reaches a device). */
   static final AudioClip SILENT = new AudioClip() {
      public void play() {
      }

      public void loop() {
      }

      public void stop() {
      }
   };

   /** The applet seam, TGameSparkerLogic-shaped but WITHOUT the run() pump:
    *  no thread exists here, so no gate and no park. getCodeBase points at
    *  the fixture dir, the context's toString starts "sun." (sunytyp==2),
    *  getImage mirrors a browser (bytes or a 1x1 error Image), getAudioClip
    *  always returns SILENT, repaint is a no-op. */
   static class App extends java.applet.Applet {
      final URL base;
      final AppContext ctx = new AppContext();
      final List<String> sounds = new ArrayList<String>();

      App(URL base) {
         this.base = base;
      }

      public URL getCodeBase() {
         return base;
      }

      public URL getDocumentBase() {
         return base;
      }

      public AppletContext getAppletContext() {
         return ctx;
      }

      public Image getImage(URL u, String n) {
         try {
            return Toolkit.getDefaultToolkit().createImage(readAll(new URL(u, n)));
         } catch (Exception e) {
            return Toolkit.getDefaultToolkit().createImage(new MemoryImageSource(1, 1, new int[]{0}, 0, 1));
         }
      }

      public AudioClip getAudioClip(URL u, String n) {
         sounds.add(n);
         return SILENT;
      }

      public AudioClip getAudioClip(URL u) {
         sounds.add("" + u);
         return SILENT;
      }

      public void repaint() {
      }
   }

   /** The "sun." prefix the plugin's viewer context provides (sunytyp 2). */
   static class AppContext implements AppletContext {
      public String toString() {
         return "sun.applet.AppletViewer@beef";
      }

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

   /** A recording Graphics: every drawString/fillRect/fillPolygon/drawImage
    *  the ORIGINAL issues is appended to `log` with its coordinates and the
    *  active colour, then delegated. This is how text (drawcs/levelhigh/
    *  loading/stat) and geometry (drawstat) are pinned exactly without
    *  anti-aliasing pixel fuzz. */
   /** drawcs's centering rule (xtGraphics.java:2461-2472): the recorded
    *  x is 275 - stringWidth(s)/2 in the font the call site set. The
    *  absolute x is therefore font-dependent across JDK builds (the mac
    *  logical SansSerif composite vs the container's Liberation mapping)
    *  and CANNOT be pinned as a number on both platforms; the rule itself
    *  is. The mac-font values these pins were first recorded with are
    *  kept as comments for provenance. */
   static int csx(String s, int size) {
      BufferedImage i = new BufferedImage(1, 1, BufferedImage.TYPE_INT_RGB);
      FontMetrics fm = i.createGraphics().getFontMetrics(new Font("SansSerif", 1, size));
      return 275 - fm.stringWidth(s) / 2;
   }

   static class Rec extends Graphics {
      final Graphics d;
      final List<String> log = new ArrayList<String>();
      Color color;

      Rec(Graphics d) {
         this.d = d;
      }

      void ev(String s) {
         log.add(s);
      }

      String hex() {
         return color == null ? "null"
               : String.format("%02x%02x%02x", color.getRed(), color.getGreen(), color.getBlue());
      }

      boolean has(String needle) {
         for (int i = 0; i < log.size(); i++) {
            if (log.get(i).indexOf(needle) >= 0) {
               return true;
            }
         }
         return false;
      }

      int count(String needle) {
         int n = 0;
         for (int i = 0; i < log.size(); i++) {
            if (log.get(i).indexOf(needle) >= 0) {
               n++;
            }
         }
         return n;
      }

      String first(String needle) {
         for (int i = 0; i < log.size(); i++) {
            if (log.get(i).indexOf(needle) >= 0) {
               return log.get(i);
            }
         }
         return null;
      }

      public Graphics create() {
         return new Rec(d.create());
      }

      public Graphics create(int x, int y, int w, int h) {
         return new Rec(d.create(x, y, w, h));
      }

      public void translate(int x, int y) {
         d.translate(x, y);
      }

      public Color getColor() {
         return color == null ? Color.black : color;
      }

      public void setColor(Color c) {
         color = c;
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

      public void drawLine(int a, int b, int c, int e) {
         d.drawLine(a, b, c, e);
      }

      public void fillRect(int x, int y, int w, int h) {
         ev("fillRect " + x + "," + y + "," + w + "," + h + " " + hex());
         d.fillRect(x, y, w, h);
      }

      public void clearRect(int x, int y, int w, int h) {
         d.clearRect(x, y, w, h);
      }

      public void drawRoundRect(int x, int y, int w, int h, int aw, int ah) {
         ev("drawRR " + x + "," + y + "," + w + "," + h + " " + hex());
         d.drawRoundRect(x, y, w, h, aw, ah);
      }

      public void fillRoundRect(int x, int y, int w, int h, int aw, int ah) {
         ev("fillRR " + x + "," + y + "," + w + "," + h + " " + hex());
         d.fillRoundRect(x, y, w, h, aw, ah);
      }

      public void drawOval(int x, int y, int w, int h) {
         d.drawOval(x, y, w, h);
      }

      public void fillOval(int x, int y, int w, int h) {
         d.fillOval(x, y, w, h);
      }

      public void drawArc(int x, int y, int w, int h, int s, int a) {
         d.drawArc(x, y, w, h, s, a);
      }

      public void fillArc(int x, int y, int w, int h, int s, int a) {
         d.fillArc(x, y, w, h, s, a);
      }

      public void drawPolyline(int[] x, int[] y, int n) {
         d.drawPolyline(x, y, n);
      }

      public void drawPolygon(int[] x, int[] y, int n) {
         ev("drawPoly " + coords(x, n) + " " + coords(y, n) + " " + hex());
         d.drawPolygon(x, y, n);
      }

      public void drawPolygon(java.awt.Polygon p) {
         d.drawPolygon(p);
      }

      public void fillPolygon(int[] x, int[] y, int n) {
         ev("fillPoly " + coords(x, n) + " " + coords(y, n) + " " + hex());
         d.fillPolygon(x, y, n);
      }

      public void fillPolygon(java.awt.Polygon p) {
         d.fillPolygon(p);
      }

      public void drawString(String s, int x, int y) {
         ev("str \"" + s + "\" " + x + "," + y + " " + hex());
         d.drawString(s, x, y);
      }

      public void drawString(java.text.AttributedCharacterIterator it, int x, int y) {
         d.drawString(it, x, y);
      }

      public void drawChars(char[] a, int o, int l, int x, int y) {
         d.drawChars(a, o, l, x, y);
      }

      public void drawBytes(byte[] a, int o, int l, int x, int y) {
         d.drawBytes(a, o, l, x, y);
      }

      public boolean drawImage(Image i, int x, int y, ImageObserver o) {
         ev("img " + (i == null ? "null" : i.getWidth(null) + "x" + i.getHeight(null)) + " " + x + "," + y);
         return d.drawImage(i, x, y, o);
      }

      public boolean drawImage(Image i, int x, int y, int w, int h, ImageObserver o) {
         ev("img4 " + w + "x" + h + " " + x + "," + y);
         return d.drawImage(i, x, y, w, h, o);
      }

      public boolean drawImage(Image i, int x, int y, Color c, ImageObserver o) {
         ev("imgc " + x + "," + y);
         return d.drawImage(i, x, y, c, o);
      }

      public boolean drawImage(Image i, int x, int y, int w, int h, Color c, ImageObserver o) {
         ev("img4c " + x + "," + y);
         return d.drawImage(i, x, y, w, h, c, o);
      }

      public boolean drawImage(Image i, int a, int b, int c, int e, int f, int g, int h, int j, ImageObserver o) {
         ev("img8");
         return d.drawImage(i, a, b, c, e, f, g, h, j, o);
      }

      public boolean drawImage(Image i, int a, int b, int c, int e, int f, int g, int h, int j, Color q,
            ImageObserver o) {
         ev("img8c");
         return d.drawImage(i, a, b, c, e, f, g, h, j, q, o);
      }

      public void dispose() {
         d.dispose();
      }
   }

   static String coords(int[] a, int n) {
      StringBuilder b = new StringBuilder("[");
      for (int i = 0; i < n; i++) {
         if (i > 0) {
            b.append(',');
         }
         b.append(a[i]);
      }
      return b.append(']').toString();
   }

   /** A recording AudioClip: identity of the chosen clip + the verb. */
   static class Clip implements AudioClip {
      final String name;
      final List<String> log;

      Clip(String n, List<String> l) {
         name = n;
         log = l;
      }

      public void play() {
         log.add(name + ".play");
      }

      public void loop() {
         log.add(name + ".loop");
      }

      public void stop() {
         log.add(name + ".stop");
      }
   }

   /** A device-free RadicalMod stand-in. The super ctor's Mod/ModSlayer chain
    *  fails on the null path (Mod.java:40-62 swallows it, ModSlayer's ctor is
    *  field-only) leaving stream==null; every audio verb is overridden to
    *  append to the log, so nothing ever reaches sun.audio. */
   static class RecMod extends RadicalMod {
      final String tag;
      final List<String> log;

      RecMod(String tag, List<String> log, App app) {
         super(null, 0, 0, 0, app);
         this.tag = tag;
         this.log = log;
      }

      public void play() {
         log.add(tag + ".play");
      }

      public void resume() {
         log.add(tag + ".resume");
      }

      public void stop() {
         log.add(tag + ".stop");
      }

      protected void outwithit() {
         log.add(tag + ".outwithit");
      }

      public int posit() {
         log.add(tag + ".posit");
         return 0;
      }
   }

   /** The shared graph: one ctor-built xtGraphics plus its Medium/seams. */
   static class Graph {
      final App app;
      final Medium m;
      final Graphics base;
      final Rec rec;
      final xtGraphics xt;

      Graph(App app, Medium m, Graphics base, xtGraphics xt) {
         this.app = app;
         this.m = m;
         this.base = base;
         this.xt = xt;
         this.rec = new Rec(base);
         this.rec.setFont(new Font("SansSerif", 1, 13));
         xt.ftm = this.rec.getFontMetrics();
      }
   }

   static Graph FULL;
   static Graph SPARSE;
   static final List<String> MODLOG = new ArrayList<String>();
   static final List<String> AUDIO = new ArrayList<String>();

   // =====================================================================
   // 1 - the constructor and the four loaders (B10 group 4)
   // =====================================================================

   /** B10(4) / xtGraphics.java:2930-3158. kbload/dnload are the ctor's only
    *  progress arithmetic: the four pak loads add 47+44+47+44 and the 36
    *  sound/newsun clips + two music decodes bring the var4==2 total to
    *  exactly 485 (re-derived by summing :2966-3152; B10's 485 is right).
    *  Nothing in the ctor writes fase/oldfase/unlocked/opselect/flipo/sc/
    *  pstar, so the field initializers survive. */
   private static void ctorPins() throws Exception {
      String dir = fixtureFull();
      App app = new App(dirUrl(dir));
      Medium m = new Medium();
      Graphics g = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB).getGraphics();

      captureStart();
      xtGraphics xt = null;
      Throwable boom = null;
      try {
         xt = new xtGraphics(m, g, app, 2);
      } catch (Throwable t) {
         boom = t;
      } finally {
         ctorOut = captureEnd();
      }
      Check.ok(boom == null, "ctor_full_no_exception");
      Check.eq(app.sounds.size(), 36, "ctor_full_sound_clip_count_36");
      Check.ok(!ctorOut.contains("Error Reading Images Pak") && !ctorOut.contains("Error Loading Images Pak"),
            "ctor_full_no_pak_errors_printed");
      FULL = new Graph(app, m, g, xt);
      // neutralize the two REAL RadicalMods the ctor decoded: every later
      // stageselect/carselect call would otherwise reach sun.audio
      // (RadicalMod.play -> AudioPlayer.player.start). They are never
      // replaced by the ctor itself, only by this test seam.
      xt.stages = new RecMod("stages", MODLOG, app);
      xt.cars = new RecMod("cars", MODLOG, app);

      Check.eq(xt.kbload, KX.intv("kbload_newsun"), "ctor_kbload_514_var4_2");
      Check.eq(xt.dnload, KX.intv("dnload_newsun"), "ctor_dnload_485_var4_2");
      Check.eq(xt.fase, 111, "ctor_fase_111_untouched");
      Check.eq(xt.oldfase, 0, "ctor_oldfase_0");
      Check.eq(xt.unlocked, 1, "ctor_unlocked_1");
      Check.eq(xt.opselect, 1, "ctor_opselect_1");
      Check.eq(xt.flipo, 0, "ctor_flipo_0");
      Check.eq(Arrays.toString(xt.sc), "[0, 0, 0, 0, 0]", "ctor_sc_all_zero");
      Check.eq(xt.pstar, 0, "ctor_pstar_0");
      Check.eq(xt.pcontin, 0, "ctor_pcontin_0");
      Check.eq(xt.pnext, 0, "ctor_pnext_0");
      Check.eq(xt.pback, 0, "ctor_pback_0");
      Check.eq(xt.gocnt, 0, "ctor_gocnt_0");
      Check.eq(xt.holdcnt, 0, "ctor_holdcnt_0");
      Check.ok(!xt.winner, "ctor_winner_false");
      Check.ok(!xt.shaded, "ctor_shaded_false");
      Check.ok(!xt.nextc, "ctor_nextc_false");
      Check.eq(xt.gatey, 0, "ctor_gatey_0");
      Check.eq(xt.looped, 1, "ctor_looped_1");
      Check.eq(xt.pin, 60, "ctor_pin_60");
      Check.eq(xt.pwait, 7, "ctor_pwait_7");
      Check.eq(xt.tcnt, 30, "ctor_tcnt_30");
      Check.eq(xt.auscnt, 45, "ctor_auscnt_45");
      Check.eq(xt.stopcnt, 0, "ctor_stopcnt_0");
      Check.eq(xt.cntwis, 0, "ctor_cntwis_0");
      Check.eq(xt.crshturn, 0, "ctor_crshturn_0");
      Check.eq(xt.bfcrash, 0, "ctor_bfcrash_0");
      Check.eq(xt.bfskid, 0, "ctor_bfskid_0");
      Check.eq(xt.skflg, 0, "ctor_skflg_0");
      Check.eq(xt.dskflg, 0, "ctor_dskflg_0");
      Check.ok(!xt.crashup, "ctor_crashup_false");
      Check.ok(!xt.mutes && !xt.mutem, "ctor_mutes_mutem_false");
      Check.ok(!xt.arrace, "ctor_arrace_false");
      Check.eq(xt.ana, 0, "ctor_ana_0");
      Check.eq(xt.cntan, 0, "ctor_cntan_0");
      Check.eq(xt.cntovn, 0, "ctor_cntovn_0");
      Check.ok(!xt.flk && !xt.tflk && !xt.wasay, "ctor_flk_tflk_wasay_false");
      Check.eq(xt.say, "", "ctor_say_empty");
      Check.eq(xt.clear, 0, "ctor_clear_0");
      Check.eq(xt.posit, 0, "ctor_posit_0");
      Check.eq(xt.wasted, 0, "ctor_wasted_0");
      Check.eq(xt.laps, 0, "ctor_laps_0");
      Check.eq(Arrays.toString(xt.dested), "[0, 0, 0, 0, 0]", "ctor_dested_all_zero");
      Check.eq(xt.dmcnt, 0, "ctor_dmcnt_0");
      Check.ok(!xt.dmflk && !xt.pwflk, "ctor_dmflk_pwflk_false");
      Check.eq(xt.pwcnt, 0, "ctor_pwcnt_0");
      Check.ok(!xt.grrd && !xt.aird, "ctor_grrd_aird_false");
      Check.ok(!xt.pwastd, "ctor_pwastd_false");
      // the ctor re-clears loadedt[0..9] at :3155-3157; [10] keeps the
      // field initializer. All eleven are false either way.
      String loaded = "";
      for (int i = 0; i < 11; i++) {
         loaded += xt.loadedt[i] ? "1" : "0";
      }
      Check.eq(loaded, "00000000000", "ctor_loadedt_all_false");

      // the static tables and the two welcome-image name rules
      Check.eq(xt.names.length, 10, "ctor_names_length_10");
      Check.eq(xt.names[0], "Tornado Shark", "ctor_names_0_tornado_shark");
      Check.eq(xt.names[9], "DR Monstaa", "ctor_names_9_dr_monstaa");
      Check.eq(Arrays.toString(xt.hipno), "[0, 2, 2, 2, 2, 0, 2, 50, 2, 2, 6]", "ctor_hipno_table");
      Check.eq(Arrays.toString(xt.sndsize), "[39, 128, 23, 58, 106, 140, 81, 135, 38, 141, 80]",
            "ctor_sndsize_table");
      Check.eq(Arrays.toString(xt.xstart), "[0, -350, 350, 0, 0]", "ctor_xstart_table");
      Check.eq(Arrays.toString(xt.zstart), "[-100, 500, 500, 500, 1100]", "ctor_zstart_table");
      Check.eq(Arrays.toString(xt.enginsignature[0]), "[0, 0, 1, 1, 0]", "ctor_enginsignature_0");
      Check.eq(Arrays.toString(xt.enginsignature[1]), "[0, 1, 1, 0, 1]", "ctor_enginsignature_1");
      Check.eq(Arrays.toString(xt.enginsignature[2]), "[0, 0, 0, 0, 0]", "ctor_enginsignature_2");
      Check.eq(Arrays.toString(xt.enginsignature[9]), "[1, 1, 1, 1, 1]", "ctor_enginsignature_9");
      Check.eq(xt.proba.length, 10, "ctor_proba_length_10");
      Check.eq(xt.dishandle.length, 10, "ctor_dishandle_length_10");
      Check.eq(xt.adj.length, 5, "ctor_adj_rows_5");
      Check.eq(xt.adj[4][2], "bounce back", "ctor_adj_4_2");
      Check.eq(Arrays.toString(xt.exlm), "[!, !!, !!!]", "ctor_exlm_three");
      Check.eq(xt.flexpix.length, 220000, "ctor_flexpix_220000");
      Check.eq(xt.ob, null, "ctor_ob_never_assigned_null");

      // welcome images: the ctor asks for them at the codebase ROOT, so the
      // fixture's root copies make them REAL (the shipped tree's graphics/
      // copies give the seam's 1x1 error Image instead - see SPARSE)
      Check.eq(xt.hello.getWidth(null), 550, "ctor_hello_root_width_550");
      Check.eq(xt.hello.getHeight(null), 217, "ctor_hello_root_height_217");
      Check.eq(xt.sign.getWidth(null), 75, "ctor_sign_root_width_75");
      Check.eq(xt.sign.getHeight(null), 66, "ctor_sign_root_height_66");
      Check.eq(xt.loadbar.getWidth(null), 238, "ctor_loadbar_root_width_238");
      Check.eq(xt.loadbar.getHeight(null), 21, "ctor_loadbar_root_height_21");

      // pak 1 (:1055-1089)
      Check.ok(xt.carsbg != null && xt.select != null && xt.paused != null, "ctor_pak1_cars_select_paused");
      Check.ok(xt.orank[0] != null && xt.ogameh != null && xt.gameov != null, "ctor_pak1_rank0_gameh_gameov");
      Check.ok(xt.olap != null && xt.oyourwasted != null && xt.oyouwastedem != null, "ctor_pak1_lap_wasted_images");
      // pak 2 (:2672-2718)
      Check.ok(xt.bgmain != null && xt.br != null && xt.oloadingmusic != null, "ctor_pak2_bgmain_br_loadingmusic");
      Check.ok(xt.maini != null && xt.radicalplay != null, "ctor_pak2_maini_radicalplay");
      Check.ok(xt.orank[1] != null && xt.orank[4] != null, "ctor_pak2_rank1_rank4");
      Check.ok(xt.ocntdn[1] != null && xt.ocntdn[3] != null, "ctor_pak2_countdown_1_3");
      // pak 3 (:3476-3518)
      Check.ok(xt.back[0] != null && xt.next[0] != null, "ctor_pak3_back_next_0");
      Check.ok(xt.contin1[0] != null && xt.contin2[0] != null, "ctor_pak3_contin_pairs_0");
      Check.ok(xt.inst3 != null && xt.pgate != null && xt.rpro != null, "ctor_pak3_inst3_pgate_rpro");
      Check.ok(xt.selectcar != null && xt.trackbg != null, "ctor_pak3_selectcar_trackbg");
      Check.ok(xt.oyoulost != null && xt.oyouwon != null, "ctor_pak3_lost_won");
      // pak 4 (:4031-4097)
      Check.ok(xt.ocntdn[0] != null && xt.odmg != null && xt.opwr != null, "ctor_pak4_countdown0_dmg_pwr");
      Check.ok(xt.opos != null && xt.owas != null, "ctor_pak4_pos_was");
      Check.ok(xt.bl != null && xt.bt != null && xt.bb != null, "ctor_pak4_borders");
      Check.ok(xt.ostar[0] != null && xt.ostar[1] != null, "ctor_pak4_ostar_pair");
      Check.ok(xt.congrd != null && xt.statb != null && xt.omdness != null, "ctor_pak4_congrd_statb_mdness");
      Check.ok(xt.opti != null && xt.inst1 != null && xt.inst2 != null && xt.nfmcom != null,
            "ctor_pak4_opti_inst_nfmcom");
      // derived by pressed()/bressed() at :2977-2985
      Check.ok(xt.next[1] != null && xt.next[2] != null, "ctor_pressed_next_derived");
      Check.ok(xt.back[1] != null && xt.back[2] != null, "ctor_pressed_back_derived");
      Check.ok(xt.contin1[1] != null && xt.contin2[1] != null, "ctor_pressed_contin_derived");
      Check.ok(xt.star[2] != null, "ctor_pressed_star2_from_ostar1");
      // snap() has NOT run, so the snapshot set is still null
      Check.ok(xt.star[0] == null && xt.star[1] == null, "ctor_star_0_1_null_before_snap");
      Check.ok(xt.rank[0] == null && xt.dmg == null && xt.pwr == null, "ctor_rank_dmg_pwr_null_before_snap");
      Check.ok(xt.gameh == null && xt.mdness == null && xt.loadingmusic == null, "ctor_hud_images_null_before_snap");
      Check.ok(xt.fleximg == null, "ctor_fleximg_null");

      // every clip is the seam's SILENT instance (no device object exists)
      Check.ok(xt.crash[0] == SILENT && xt.crash[2] == SILENT, "ctor_crash_clips_are_silent_stub");
      Check.ok(xt.air[0] == SILENT && xt.air[5] == SILENT, "ctor_air_clips_are_silent_stub");
      Check.ok(xt.engs[0][0] == SILENT && xt.engs[1][4] == SILENT, "ctor_engine_clips_are_silent_stub");
      Check.ok(xt.wastd == SILENT && xt.firewasted == SILENT && xt.powerup == SILENT, "ctor_named_clips_are_silent_stub");
      // and the sound dir rule: var4==2 -> sounds/newsun/
      Check.ok(allPrefix(app.sounds, "sounds/newsun/"), "ctor_sound_dir_newsun_when_var4_2");
      Check.ok(app.sounds.contains("sounds/newsun/a0.au"), "ctor_sound_a0_newsun");
      Check.ok(app.sounds.contains("sounds/newsun/b4.au"), "ctor_sound_b4_newsun");
      Check.ok(app.sounds.contains("sounds/newsun/air5.au"), "ctor_sound_air5_newsun");
      Check.ok(app.sounds.contains("sounds/newsun/crash3.au"), "ctor_sound_crash3_newsun");
      Check.ok(app.sounds.contains("sounds/newsun/lowcrash3.au"), "ctor_sound_lowcrash3_newsun");
      Check.ok(app.sounds.contains("sounds/newsun/dustskid2.au"), "ctor_sound_dustskid2_newsun");
      Check.ok(app.sounds.contains("sounds/newsun/firewasted.au"), "ctor_sound_firewasted_newsun");
      Check.eq(app.sounds.get(0), "sounds/newsun/a0.au", "ctor_sound_first_a0");

      // snap() has NOT run yet - the HUD set is still null; it is the
      // mandatory predecessor for rank/cntdn/star/loadingmusic (:2540-2567)
      xt.snap(1);
      Check.ok(xt.rank[0] != null && xt.star[0] != null, "ctor_snap_after_makes_rank_star");

      // the sparse ctor: only images3/images4, var4==0
      sparseCtorPins();

      // the two missing-pak NPE arms
      missingImages3();
      missingImages4();
   }

   /** The var4!=2 arm plus the loaders' swallow-everything contract:
    *  loadpak1/2 fail on the absent paks and print, loadpak3/4 succeed, and
    *  dnload is STILL 387 because the +47/+44 are unconditional (:2966-2975).
    *  Welcome images fall back to the seam's 1x1 error Image, which is also
    *  the shipped-tree verdict for B10's codebase-layout OPEN item. */
   private static void sparseCtorPins() throws Exception {
      App app = new App(dirUrl(fixtureSparse()));
      Medium m = new Medium();
      Graphics g = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB).getGraphics();
      captureStart();
      xtGraphics xt = null;
      Throwable boom = null;
      try {
         xt = new xtGraphics(m, g, app, 0);
      } catch (Throwable t) {
         boom = t;
      } finally {
         ctorOut = captureEnd();
      }
      Check.ok(boom == null, "sparse_ctor_no_exception");
      SPARSE = new Graph(app, m, g, xt);
      Check.eq(xt.kbload, KX.intv("kbload_default"), "sparse_kbload_416_var4_0");
      Check.eq(xt.dnload, KX.intv("dnload_default"), "sparse_dnload_387_var4_0");
      Check.eq(app.sounds.size(), 36, "sparse_sound_clip_count_36");
      Check.ok(allPrefix(app.sounds, "sounds/default/"), "sparse_sound_dir_default_when_var4_0");

      // loadpak1 / loadpak2 printed and returned; their fields are null
      Check.ok(ctorOut.contains("Error Loading Images Pak 1:"), "sparse_pak1_missing_printed");
      Check.ok(ctorOut.contains("Error Reading Images Pak 2:"), "sparse_pak2_missing_printed");
      Check.ok(!ctorOut.contains("Error Reading Images Pak 3:"), "sparse_pak3_present_no_error");
      Check.ok(!ctorOut.contains("Error Reading Images Pak 4:"), "sparse_pak4_present_no_error");
      Check.eq(xt.carsbg, null, "sparse_pak1_missing_carsbg_null");
      Check.eq(xt.select, null, "sparse_pak1_missing_select_null");
      Check.eq(xt.olap, null, "sparse_pak1_missing_olap_null");
      Check.eq(xt.maini, null, "sparse_pak2_missing_maini_null");
      Check.eq(xt.bgmain, null, "sparse_pak2_missing_bgmain_null");
      Check.eq(xt.radicalplay, null, "sparse_pak2_missing_radicalplay_null");
      // pak 3 / 4 still land
      Check.ok(xt.next[0] != null && xt.back[0] != null && xt.trackbg != null, "sparse_pak3_fields_present");
      Check.ok(xt.odmg != null && xt.ostar[1] != null && xt.bl != null && xt.inst1 != null,
            "sparse_pak4_fields_present");
      Check.ok(xt.next[1] != null && xt.star[2] != null, "sparse_derived_images_present");
      // the missing welcome gifs
      Check.eq(xt.hello.getWidth(null), 1, "sparse_hello_missing_1x1_error_image");
      Check.eq(xt.sign.getWidth(null), 1, "sparse_sign_missing_1x1_error_image");
      Check.eq(xt.loadbar.getWidth(null), 1, "sparse_loadbar_missing_1x1_error_image");
   }

   /** B10(4) / :2977 -> :1018: next[0] comes from images3, so a codebase
    *  without graphics/images3.zipo dies inside pressed() when the ctor
    *  makes next[1] - AFTER loadpak3 printed its swallowed error. */
   private static void missingImages3() throws Exception {
      App app = new App(dirUrl(fixtureNo3()));
      Medium m = new Medium();
      Graphics g = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB).getGraphics();
      captureStart();
      Throwable boom = null;
      try {
         new xtGraphics(m, g, app, 2);
      } catch (Throwable t) {
         boom = t;
      } finally {
         ctorOut = captureEnd();
      }
      Check.ok(boom instanceof NullPointerException, "no3_ctor_npe");
      Check.eq(boom.getStackTrace()[0].getMethodName(), "pressed", "no3_ctor_npe_top_frame_pressed");
      Check.eq(boom.getStackTrace()[0].getLineNumber(), 1018, "no3_ctor_npe_line_1018");
      Check.eq(boom.getStackTrace()[1].getMethodName(), "<init>", "no3_ctor_npe_caller_ctor");
      Check.ok(ctorOut.contains("Error Reading Images Pak 3:"), "no3_pak3_missing_printed");
      Check.ok(!ctorOut.contains("Error Reading Images Pak 4:"), "no3_pak4_present_no_error");
   }

   /** B10(4) / :2985 -> :1018: ostar[1] comes from images4, so a codebase
    *  without graphics/images4.zipo dies on `pressed(ostar[1])`. */
   private static void missingImages4() throws Exception {
      App app = new App(dirUrl(fixtureNo4()));
      Medium m = new Medium();
      Graphics g = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB).getGraphics();
      captureStart();
      Throwable boom = null;
      try {
         new xtGraphics(m, g, app, 2);
      } catch (Throwable t) {
         boom = t;
      } finally {
         ctorOut = captureEnd();
      }
      Check.ok(boom instanceof NullPointerException, "no4_ctor_npe");
      Check.eq(boom.getStackTrace()[0].getMethodName(), "pressed", "no4_ctor_npe_top_frame_pressed");
      Check.eq(boom.getStackTrace()[0].getLineNumber(), 1018, "no4_ctor_npe_line_1018");
      Check.ok(ctorOut.contains("Error Reading Images Pak 4:"), "no4_pak4_missing_printed");
      Check.ok(!ctorOut.contains("Error Reading Images Pak 3:"), "no4_pak3_present_no_error");
   }

   // =====================================================================
   // 2 - pure helpers (B10 group 5)
   // =====================================================================

   /** B10(5) / :2475 py, :999 ys, :3442 xs, :1115 rot, :1126 overon). */
   private static void pureHelperPins() {
      xtGraphics x = FULL.xt;
      Check.eq(x.py(3, 1, 5, 2), 13, "pure_py_3_1_5_2_13");
      Check.eq(x.py(-1, 0, -1, 0), 2, "pure_py_four_square_sum_2");
      Check.eq(x.py(0, 100000, 0, 0), 1410065408, "pure_py_int_overflow_wraps");
      Check.eq(x.py(1, 1, 1, 1), 0, "pure_py_same_point_0");

      // defaults: focus_point 400, cx 275, cy 200
      Check.eq(x.xs(0, 50), -1925, "pure_xs_0_50_minus1925");
      Check.eq(x.ys(100, 50), -600, "pure_ys_100_50_minus600");
      Check.eq(x.xs(0, 10), x.xs(0, 50), "pure_xs_d_under_50_clamped");
      Check.eq(x.ys(100, 0), x.ys(100, 50), "pure_ys_d_under_50_clamped");
      Check.eq(x.xs(10, 100), (100 - 400) * (275 - 10) / 100 + 10, "pure_xs_formula");
      Check.eq(x.ys(10, 100), (100 - 400) * (200 - 10) / 100 + 10, "pure_ys_formula");

      // rot: ang==0 is a strict no-op, else m.cos/m.sin
      int[] ax = {100, 0};
      int[] ay = {11, 0};
      x.rot(ax, ay, 0, 0, 0, 2);
      Check.eq(Arrays.toString(ax), "[100, 0]", "pure_rot_ang0_x_untouched");
      Check.eq(Arrays.toString(ay), "[11, 0]", "pure_rot_ang0_y_untouched");
      ax[0] = 100;
      ax[1] = 0;
      ay[0] = 11;
      ay[1] = 0;
      x.rot(ax, ay, 0, 0, 90, 2);
      Check.eq(Arrays.toString(ax), "[-11, 0]", "pure_rot_90_x_minus11");
      Check.eq(Arrays.toString(ay), "[100, 0]", "pure_rot_90_y_100");
      ax[0] = 100;
      ay[0] = 11;
      x.rot(ax, ay, 0, 0, 45, 1);
      Check.eq(ax[0], 62, "pure_rot_45_x_62");
      Check.eq(ay[0], 78, "pure_rot_45_y_78");

      // overon: strict on all four edges
      Check.ok(x.overon(204, 143, 137, 22, 205, 144), "pure_overon_inside_true");
      Check.ok(!x.overon(204, 143, 137, 22, 204, 144), "pure_overon_left_edge_false");
      Check.ok(!x.overon(204, 143, 137, 22, 205, 143), "pure_overon_top_edge_false");
      Check.ok(!x.overon(204, 143, 137, 22, 341, 144), "pure_overon_right_edge_false");
      Check.ok(!x.overon(204, 143, 137, 22, 205, 165), "pure_overon_bottom_edge_false");
      Check.ok(x.overon(204, 143, 137, 22, 340, 164), "pure_overon_far_corner_inside_true");

      // over: +-5 tolerance around dx/dy + image size (:296-300)
      BufferedImage oi = new BufferedImage(100, 50, BufferedImage.TYPE_INT_RGB);
      Check.ok(x.over(oi, 196, 111, 200, 115), "pure_over_inside_true");
      Check.ok(!x.over(oi, 195, 111, 200, 115), "pure_over_left_minus5_strict_false");
      Check.ok(x.over(oi, 304, 169, 200, 115), "pure_over_bottom_plus5_inside_true");
      Check.ok(!x.over(oi, 305, 164, 200, 115), "pure_over_right_plus5_strict_false");
      Check.ok(!x.over(oi, 304, 170, 200, 115), "pure_over_below_plus5_false");
      Check.ok(!x.over(oi, 200, 110, 200, 115), "pure_over_top_edge_false");

      sortcarsPins();
      resetstatPins();
   }

   /** B10(5) / :2278-2383. sortcars uses STOCK Math.random, so only the
    *  deterministic outputs and the random-independent properties are
    *  pinned: sc[4], sc[1..3] distinctness/bounds, the stage-10 forced 7. */
   private static void sortcarsPins() {
      xtGraphics x = FULL.xt;

      resetAll();
      x.sortcars(0);
      Check.eq(Arrays.toString(x.sc), "[0, 0, 0, 0, 0]", "pure_sortcars_0_noop");

      resetAll();
      x.sortcars(1);
      Check.eq(x.sc[4], 5, "pure_sortcars_1_sc4_5");
      Check.eq(distinctNonZero(x.sc, 1, 3), true, "pure_sortcars_1_middle_distinct");
      Check.ok(x.sc[1] <= 4 && x.sc[2] <= 4 && x.sc[3] <= 4, "pure_sortcars_1_middle_le4");
      Check.ok(x.sc[1] >= 1 && x.sc[2] >= 1 && x.sc[3] >= 1, "pure_sortcars_1_middle_ge1");

      resetAll();
      x.sortcars(2);
      Check.eq(x.sc[4], 5, "pure_sortcars_2_sc4_5");

      resetAll();
      x.sortcars(4);
      Check.eq(x.sc[4], 6, "pure_sortcars_4_sc4_6");
      Check.ok(x.sc[1] <= 5 && x.sc[2] <= 5 && x.sc[3] <= 5, "pure_sortcars_4_middle_le5");

      resetAll();
      x.unlocked = 10;
      x.sortcars(10);
      Check.eq(x.sc[4], 9, "pure_sortcars_10_sc4_9");
      boolean has7 = false;
      for (int i = 0; i < 5; i++) {
         if (x.sc[i] == 7) {
            has7 = true;
         }
      }
      Check.ok(has7, "pure_sortcars_10_forced_7_somewhere");

      // stage 11: the sc[4] write is skipped entirely (:2352)
      // stage 11 takes the else branch and KEEPS var3==5, so the loop
      // fills sc[1..4] (the sc[4] write at :2352 is skipped because
      // var1==11, but :2357 still iterates to 4). Range is 0..9 via
      // `5 + (unlocked==11 -> 10 + 1)/2`.
      resetAll();
      x.unlocked = 11;
      x.sortcars(11);
      Check.ok(x.sc[4] >= 0 && x.sc[4] <= 9, "pure_sortcars_11_sc4_in_0_9");
      Check.eq(distinctNonZero(x.sc, 1, 4), true, "pure_sortcars_11_all_four_distinct");
   }

   /** B10(5) / :2621-2652 - exactly 24 field writes then sortcars. */
   private static void resetstatPins() {
      xtGraphics x = FULL.xt;
      resetAll();
      x.arrace = true;
      x.ana = 9;
      x.cntan = 9;
      x.cntovn = 9;
      x.tcnt = 9;
      x.wasay = true;
      x.clear = 9;
      x.dmcnt = 9;
      x.pwcnt = 9;
      x.auscnt = 9;
      x.pnext = 9;
      x.pback = 9;
      x.starcnt = 9;
      x.gocnt = 9;
      x.grrd = false;
      x.aird = false;
      x.bfcrash = 9;
      x.cntwis = 9;
      x.bfskid = 9;
      x.pwait = 9;
      x.holdcnt = 9;
      x.winner = true;
      x.wasted = 3;
      for (int i = 0; i < 5; i++) {
         x.dested[i] = 1;
      }
      x.say = "keep";
      x.tflk = true;
      x.flk = true;
      x.aflk = true;
      x.pin = 7;
      x.stopcnt = 7;
      x.crshturn = 2;
      x.shaded = true;
      x.opselect = 2;
      x.looped = 3;
      x.oldfase = 5;
      x.fase = 8;
      x.sortcars(0); // keep sc deterministic before the reset
      x.resetstat(0);
      Check.ok(!x.arrace, "pure_resetstat_arrace_false");
      Check.eq(x.ana, 0, "pure_resetstat_ana_0");
      Check.eq(x.cntan, 0, "pure_resetstat_cntan_0");
      Check.eq(x.cntovn, 0, "pure_resetstat_cntovn_0");
      Check.eq(x.tcnt, 30, "pure_resetstat_tcnt_30");
      Check.ok(!x.wasay, "pure_resetstat_wasay_false");
      Check.eq(x.clear, 0, "pure_resetstat_clear_0");
      Check.eq(x.dmcnt, 0, "pure_resetstat_dmcnt_0");
      Check.eq(x.pwcnt, 0, "pure_resetstat_pwcnt_0");
      Check.eq(x.auscnt, 45, "pure_resetstat_auscnt_45");
      Check.eq(x.pnext, 0, "pure_resetstat_pnext_0");
      Check.eq(x.pback, 0, "pure_resetstat_pback_0");
      Check.eq(x.starcnt, KX.intv("starcnt_start"), "pure_resetstat_starcnt_90");
      Check.eq(x.gocnt, KX.intv("gocnt_start"), "pure_resetstat_gocnt_3");
      Check.ok(x.grrd, "pure_resetstat_grrd_true");
      Check.ok(x.aird, "pure_resetstat_aird_true");
      Check.eq(x.bfcrash, 0, "pure_resetstat_bfcrash_0");
      Check.eq(x.cntwis, 0, "pure_resetstat_cntwis_0");
      Check.eq(x.bfskid, 0, "pure_resetstat_bfskid_0");
      Check.eq(x.pwait, 7, "pure_resetstat_pwait_7");
      Check.eq(x.holdcnt, 0, "pure_resetstat_holdcnt_0");
      Check.ok(!x.winner, "pure_resetstat_winner_false");
      Check.eq(x.wasted, 0, "pure_resetstat_wasted_0");
      Check.eq(Arrays.toString(x.dested), "[0, 0, 0, 0, 0]", "pure_resetstat_dested_all_zero");
      // outside the list: untouched
      Check.eq(x.say, "keep", "pure_resetstat_say_untouched");
      Check.ok(x.tflk && x.flk && x.aflk, "pure_resetstat_flk_triplet_untouched");
      Check.eq(x.pin, 7, "pure_resetstat_pin_untouched");
      Check.eq(x.stopcnt, 7, "pure_resetstat_stopcnt_untouched");
      Check.eq(x.crshturn, 2, "pure_resetstat_crshturn_untouched");
      Check.ok(x.shaded, "pure_resetstat_shaded_untouched");
      Check.eq(x.opselect, 2, "pure_resetstat_opselect_untouched");
      Check.eq(x.looped, 3, "pure_resetstat_looped_untouched");
      Check.eq(x.oldfase, 5, "pure_resetstat_oldfase_untouched");
      Check.eq(x.fase, 8, "pure_resetstat_fase_untouched");

      // B10(99): resetstat(90) -> sc[4] = 4 + 91/2 = 49, so the next stat()
      // that sees CheckPoints.dested[4] change indexes names[49] (:2069).
      resetAll();
      x.unlocked = 1;
      x.resetstat(90);
      Check.eq(x.sc[4], 49, "pure_resetstat_90_sc4_49");
      Madness mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      Control ctl = new Control(FULL.m);
      CheckPoints cp = new CheckPoints();
      cp.dested[4] = 1;
      final xtGraphics fx = x;
      final Madness fm = mad;
      final Control fc = ctl;
      final CheckPoints fcp = cp;
      final Graphics fg = FULL.rec;
      Check.throwsIt(new Check.Task() {
         public void run() {
            fx.stat(fm, fcp, fc, false, fg);
         }
      }, ArrayIndexOutOfBoundsException.class, "pure_resetstat_90_stat_names49_aioobe");
      try {
         x.stat(mad, cp, ctl, false, FULL.rec);
      } catch (ArrayIndexOutOfBoundsException e) {
         Check.eq(e.getStackTrace()[0].getMethodName(), "stat", "pure_resetstat_90_aioobe_in_stat");
         Check.eq(e.getStackTrace()[0].getLineNumber(), 2069, "pure_resetstat_90_aioobe_line_2069");
      }
   }

   // =====================================================================
   // 3 - the screen state machine (B10 group 1)
   // =====================================================================

   /** B10(1) - every menu transition, one handler call per pin. */
   private static void screenPins() throws Exception {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;

      // ---- stageselect (:2485-2538)
      resetAll();
      Control c = new Control(FULL.m);
      CheckPoints cp = new CheckPoints();
      MODLOG.clear();
      c.handb = true;
      x.stageselect(g, cp, c);
      Check.eq(x.fase, 5, "screen_stageselect_handb_fase_5");
      Check.ok(!FULL.m.trk, "screen_stageselect_handb_m_trk_false");
      Check.ok(!c.handb, "screen_stageselect_handb_cleared");
      Check.ok(MODLOG.contains("stages.stop"), "screen_stageselect_handb_stages_stop");
      Check.ok(MODLOG.contains("stages.play"), "screen_stageselect_always_stages_play");

      resetAll();
      c = new Control(FULL.m);
      c.enter = true;
      x.stageselect(g, cp, c);
      Check.eq(x.fase, 5, "screen_stageselect_enter_fase_5");
      Check.ok(!c.enter, "screen_stageselect_enter_cleared");

      resetAll();
      c = new Control(FULL.m);
      cp.stage = 2;
      x.unlocked = 1;
      c.right = true;
      x.stageselect(g, cp, c);
      Check.eq(cp.stage, 3, "screen_stageselect_right_stage_plus1");
      Check.eq(x.fase, 2, "screen_stageselect_right_fase_2");
      Check.ok(!c.right, "screen_stageselect_right_cleared");

      resetAll();
      c = new Control(FULL.m);
      cp.stage = 1;
      x.unlocked = 1;
      c.right = true;
      x.stageselect(g, cp, c);
      Check.eq(cp.stage, 1, "screen_stageselect_right_locked_stage_kept");
      Check.eq(x.fase, 4, "screen_stageselect_right_locked_fase_4");
      Check.eq(x.lockcnt, 70, "screen_stageselect_right_locked_lockcnt_70");
      Check.ok(!c.right, "screen_stageselect_right_locked_cleared");

      resetAll();
      c = new Control(FULL.m);
      cp.stage = 11;
      x.unlocked = 11;
      c.right = true;
      x.stageselect(g, cp, c);
      Check.eq(cp.stage, 11, "screen_stageselect_stage11_right_noop_stage");
      Check.ok(c.right, "screen_stageselect_stage11_right_flag_kept");
      Check.eq(x.fase, 111, "screen_stageselect_stage11_right_fase_kept");

      resetAll();
      c = new Control(FULL.m);
      cp.stage = 3;
      c.left = true;
      x.stageselect(g, cp, c);
      Check.eq(cp.stage, 2, "screen_stageselect_left_stage_minus1");
      Check.eq(x.fase, 2, "screen_stageselect_left_fase_2");
      Check.ok(!c.left, "screen_stageselect_left_cleared");

      resetAll();
      c = new Control(FULL.m);
      cp.stage = 1;
      c.left = true;
      x.stageselect(g, cp, c);
      Check.eq(cp.stage, 1, "screen_stageselect_left_stage1_noop_stage");
      Check.eq(x.fase, 111, "screen_stageselect_left_stage1_fase_kept");
      Check.ok(c.left, "screen_stageselect_left_stage1_flag_kept");

      // ---- cantgo (:263-294)
      resetAll();
      c = new Control(FULL.m);
      x.fase = 4;
      x.lockcnt = 5;
      x.pnext = 3;
      x.aflk = false;
      g.log.clear();
      x.cantgo(g, c);
      Check.eq(x.lockcnt, 4, "screen_cantgo_lockcnt_decrements");
      Check.eq(x.fase, 4, "screen_cantgo_fase_kept_while_counting");
      Check.eq(x.pnext, 0, "screen_cantgo_pnext_zeroed");
      Check.ok(x.aflk, "screen_cantgo_aflk_false_to_true");
      Check.ok(g.has("[ Stage 2 Locked ]"), "screen_cantgo_locked_text_unlocked_plus1");
      Check.ok(g.first("[ Stage 2 Locked ]").endsWith("ff0000"), "screen_cantgo_first_draw_red");
      g.log.clear();
      x.cantgo(g, c);
      Check.ok(!x.aflk, "screen_cantgo_aflk_true_to_false");
      Check.ok(g.first("[ Stage 2 Locked ]").endsWith("000000"), "screen_cantgo_second_draw_black");

      resetAll();
      c = new Control(FULL.m);
      x.fase = 4;
      x.lockcnt = 1;
      x.cantgo(g, c);
      Check.eq(x.lockcnt, 0, "screen_cantgo_lockcnt_1_to_0");
      Check.eq(x.fase, 1, "screen_cantgo_lockcnt_zero_fase_1");

      resetAll();
      c = new Control(FULL.m);
      x.fase = 4;
      x.lockcnt = 9;
      c.enter = true;
      c.handb = true;
      c.left = true;
      x.cantgo(g, c);
      Check.eq(x.fase, 1, "screen_cantgo_enter_fase_1");
      Check.ok(!c.enter && !c.handb && !c.left, "screen_cantgo_three_flags_cleared");

      // ---- maini (:3160-3272)
      resetAll();
      c = new Control(FULL.m);
      x.maini(g, c);
      Check.eq(x.flipo, 1, "screen_maini_first_call_flipo_1");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 1;
      c.up = true;
      x.maini(g, c);
      Check.eq(x.opselect, 0, "screen_maini_up_1_to_0");
      Check.ok(!c.up, "screen_maini_up_cleared");
      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      c.up = true;
      x.maini(g, c);
      Check.eq(x.opselect, 2, "screen_maini_up_0_wraps_2");
      resetAll();
      c = new Control(FULL.m);
      x.opselect = 1;
      c.down = true;
      x.maini(g, c);
      Check.eq(x.opselect, 2, "screen_maini_down_1_to_2");
      Check.ok(!c.down, "screen_maini_down_cleared");
      resetAll();
      c = new Control(FULL.m);
      x.opselect = 2;
      c.down = true;
      x.maini(g, c);
      Check.eq(x.opselect, 0, "screen_maini_down_2_wraps_0");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      x.unlocked = 1;
      x.oldfase = 0;
      c.enter = true;
      x.maini(g, c);
      Check.eq(x.oldfase, 7, "screen_maini_op0_oldfase_7");
      Check.eq(x.fase, 11, "screen_maini_op0_fase_11");
      Check.ok(!c.enter, "screen_maini_op0_enter_cleared");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      x.unlocked = 2;
      x.oldfase = 0;
      c.enter = true;
      x.maini(g, c);
      Check.eq(x.fase, -9, "screen_maini_op0_unlocked2_fase_minus9");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      x.unlocked = 1;
      x.oldfase = 5;
      c.enter = true;
      x.maini(g, c);
      Check.eq(x.fase, -9, "screen_maini_op0_oldfase_nonzero_fase_minus9");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 1;
      c.enter = true;
      x.flipo = 6;
      x.maini(g, c);
      Check.eq(x.oldfase, 10, "screen_maini_op1_oldfase_10");
      Check.eq(x.fase, 11, "screen_maini_op1_fase_11");
      Check.eq(x.flipo, 0, "screen_maini_enter_flipo_reset_0");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 2;
      c.handb = true;
      x.maini(g, c);
      Check.eq(x.fase, 8, "screen_maini_op2_fase_8");
      Check.ok(!c.handb, "screen_maini_op2_handb_cleared");

      // ---- inst (:320-369)
      resetAll();
      c = new Control(FULL.m);
      x.flipo = 0;
      x.inst(g, c);
      Check.eq(x.flipo, 1, "screen_inst_0_to_1_no_enter");
      c.enter = true;
      x.inst(g, c);
      Check.eq(x.flipo, 2, "screen_inst_enter_1_to_2");
      Check.ok(!c.enter, "screen_inst_enter_consumed");
      c.enter = true;
      x.inst(g, c);
      Check.eq(x.flipo, 4, "screen_inst_enter_2_to_4_same_call");
      // from 4 the SAME call's flipo==4 block (->5) then the enter block
      // (5 -> 0 + fase=oldfase) both run
      c.enter = true;
      x.oldfase = 0;
      x.inst(g, c);
      Check.eq(x.flipo, 0, "screen_inst_enter_4_exits_same_call");
      Check.eq(x.fase, 0, "screen_inst_enter_4_restores_oldfase_0");
      Check.ok(!c.enter, "screen_inst_enter_consumed_again");
      // a bare flipo==5 without enter just draws contin2 and stays
      resetAll();
      c = new Control(FULL.m);
      x.flipo = 5;
      x.inst(g, c);
      Check.eq(x.flipo, 5, "screen_inst_5_without_enter_stays_5");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 5;
      x.oldfase = 7;
      c.enter = true;
      x.inst(g, c);
      Check.eq(x.flipo, 0, "screen_inst_5_resets_flipo_0");
      Check.eq(x.fase, 7, "screen_inst_5_restores_oldfase");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 5;
      x.oldfase = -7;
      x.fleximg = new BufferedImage(7, 9, BufferedImage.TYPE_INT_RGB);
      c.enter = true;
      g.log.clear();
      x.inst(g, c);
      Check.eq(x.fase, -7, "screen_inst_oldfase_minus7_exit");
      Check.ok(g.has("img 7x9 0,0"), "screen_inst_oldfase_minus7_draws_fleximg");

      // ---- credits (:1452-1513)
      resetAll();
      installAudio(x);
      c = new Control(FULL.m);
      AUDIO.clear();
      x.credits(g, c);
      Check.eq(x.flipo, 1, "screen_credits_first_call_flipo_1");
      Check.ok(AUDIO.contains("powerup.play"), "screen_credits_first_call_powerup");
      resetAll();
      c = new Control(FULL.m);
      x.credits(g, c);
      c.enter = true;
      x.credits(g, c);
      Check.eq(x.flipo, 2, "screen_credits_enter_1_to_2");
      Check.ok(!c.enter, "screen_credits_enter_cleared");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 2;
      c.right = true;
      x.credits(g, c);
      Check.eq(x.flipo, 3, "screen_credits_right_2_to_3");
      c.right = false;
      c.handb = true;
      x.credits(g, c);
      Check.eq(x.flipo, 0, "screen_credits_handb_3_wraps_0");
      Check.eq(x.fase, 10, "screen_credits_handb_3_fase_10");
      Check.ok(!c.handb, "screen_credits_handb_cleared");

      // ---- pausedgame (:1342-1450)
      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      c.up = true;
      x.pausedgame(g, 1, c, new Record(FULL.m));
      Check.eq(x.opselect, 3, "screen_paused_up_0_wraps_3");
      Check.ok(!c.up, "screen_paused_up_cleared");
      c.down = true;
      x.pausedgame(g, 1, c, new Record(FULL.m));
      Check.eq(x.opselect, 0, "screen_paused_down_3_wraps_0");
      Check.ok(!c.down, "screen_paused_down_cleared");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      MODLOG.clear();
      c.enter = true;
      x.pausedgame(g, 1, c, new Record(FULL.m));
      Check.eq(x.fase, 0, "screen_paused_op0_fase_0");
      Check.ok(MODLOG.contains("track0.resume"), "screen_paused_op0_resumes_loaded_track");
      Check.ok(!c.enter, "screen_paused_op0_enter_cleared");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 0;
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      x.mutem = true;
      MODLOG.clear();
      c.enter = true;
      x.pausedgame(g, 1, c, new Record(FULL.m));
      Check.eq(x.fase, 0, "screen_paused_op0_mutem_still_fase_0");
      Check.ok(!MODLOG.contains("track0.resume"), "screen_paused_op0_mutem_no_resume");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 1;
      Record r = new Record(FULL.m);
      r.caught = 299;
      c.enter = true;
      x.pausedgame(g, 1, c, r);
      Check.eq(x.fase, -8, "screen_paused_op1_caught299_fase_minus8");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 1;
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      r = new Record(FULL.m);
      r.caught = 300;
      MODLOG.clear();
      c.enter = true;
      x.pausedgame(g, 1, c, r);
      Check.eq(x.fase, -1, "screen_paused_op1_caught300_fase_minus1");
      Check.ok(MODLOG.contains("track0.resume"), "screen_paused_op1_caught300_resume");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 2;
      c.enter = true;
      x.pausedgame(g, 1, c, new Record(FULL.m));
      Check.eq(x.oldfase, -7, "screen_paused_op2_oldfase_minus7");
      Check.eq(x.fase, 11, "screen_paused_op2_fase_11");

      resetAll();
      c = new Control(FULL.m);
      x.opselect = 3;
      c.handb = true;
      x.pausedgame(g, 1, c, new Record(FULL.m));
      Check.eq(x.fase, 10, "screen_paused_op3_fase_10");
      Check.eq(x.opselect, 0, "screen_paused_op3_opselect_reset_0");
      Check.ok(!c.handb, "screen_paused_op3_handb_cleared");

      // ---- carselect (:3557-3713)
      ContO[] cars = freshCars();
      resetAll();
      c = new Control(FULL.m);
      c.right = true;
      x.flipo = 0;
      x.carselect(c, cars, new Madness(FULL.m, new Record(FULL.m), x, 0), g);
      Check.eq(x.flipo, 20, "screen_carselect_right_flipo_20");
      Check.ok(x.nextc, "screen_carselect_right_nextc_true");
      Check.ok(!c.right, "screen_carselect_right_cleared");
      Check.eq(x.sc[0], 0, "screen_carselect_right_sc0_kept");
      Check.eq(cars[0].y, -78, "screen_carselect_first_pass_y_minus34_grat");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 0;
      x.sc[0] = 9;
      c.right = true;
      x.carselect(c, cars, new Madness(FULL.m, new Record(FULL.m), x, 0), g);
      Check.eq(x.flipo, 0, "screen_carselect_right_at_9_noop_flipo");
      Check.ok(x.nextc == false, "screen_carselect_right_at_9_nextc_false");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 0;
      x.sc[0] = 0;
      c.left = true;
      x.carselect(c, cars, new Madness(FULL.m, new Record(FULL.m), x, 0), g);
      Check.eq(x.flipo, 0, "screen_carselect_left_at_0_noop_flipo");
      Check.ok(!x.nextc, "screen_carselect_left_at_0_nextc_false");

      // the flipo>0 countdown: 20 -> 11 shifts y/zy, 10 applies sc[0]±1
      resetAll();
      c = new Control(FULL.m);
      Madness mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.flipo = 20;
      x.nextc = true;
      x.sc[0] = 0;
      cars = freshCars();
      for (int i = 0; i < 10; i++) {
         x.carselect(c, cars, mad, g);
      }
      Check.eq(x.flipo, 10, "screen_carselect_countdown_ten_calls_flipo_10");
      Check.eq(x.sc[0], 0, "screen_carselect_countdown_sc0_still_0");
      Check.eq(cars[0].y, -1000, "screen_carselect_countdown_y_minus100_each");
      Check.eq(cars[0].zy, 200, "screen_carselect_countdown_zy_plus20_each");
      Check.eq(x.gatey, 300, "screen_carselect_countdown_gatey_300");
      x.carselect(c, cars, mad, g);
      Check.eq(x.flipo, 9, "screen_carselect_flipo10_step_flipo_9");
      Check.eq(x.sc[0], 1, "screen_carselect_flipo10_nextc_sc0_1");
      Check.eq(cars[1].z, 950, "screen_carselect_flipo10_new_car_z_950");
      Check.eq(cars[1].y, -1051, "screen_carselect_flipo10_new_car_y_minus34_grat_1100_100");
      Check.eq(cars[1].zy, 0, "screen_carselect_flipo10_new_car_zy_0");

      // the pgate gate: (sc[0]-4)*2 >= unlocked
      resetAll();
      c = new Control(FULL.m);
      x.flipo = 0;
      x.sc[0] = 5;
      x.gatey = 300;
      g.log.clear();
      x.carselect(c, cars = freshCars(), new Madness(FULL.m, new Record(FULL.m), x, 0), g);
      Check.eq(x.gatey, 200, "screen_carselect_locked_gatey_minus100");
      Check.ok(g.has("[ Car Locked ]"), "screen_carselect_locked_text");
      Check.ok(g.has("img 367x147 96,-155"), "screen_carselect_locked_pgate_above_view");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 0;
      x.sc[0] = 0;
      x.unlocked = 1;
      c.enter = true;
      x.carselect(c, freshCars(), new Madness(FULL.m, new Record(FULL.m), x, 0), g);
      Check.eq(x.fase, 2, "screen_carselect_enter_fase_2");
      Check.ok(!FULL.m.crs, "screen_carselect_enter_crs_false");
      Check.ok(!c.enter, "screen_carselect_enter_cleared");

      resetAll();
      c = new Control(FULL.m);
      x.flipo = 0;
      x.sc[0] = 0;
      x.unlocked = -8; // (0-4)*2 == -8, NOT < -8
      c.enter = true;
      x.carselect(c, freshCars(), new Madness(FULL.m, new Record(FULL.m), x, 0), g);
      Check.eq(x.fase, 111, "screen_carselect_enter_gate_strict_no_fase_write");

      // ---- musicomp (:3274-3362)
      resetAll();
      c = new Control(FULL.m);
      x.pstar = 0;
      x.musicomp(1, g, c);
      Check.eq(x.pstar, 1, "screen_musicomp_pstar_0_to_1");
      Check.eq(x.fase, 111, "screen_musicomp_no_enter_fase_kept");
      x.pstar = 1;
      x.musicomp(1, g, c);
      Check.eq(x.pstar, 0, "screen_musicomp_pstar_1_to_0");
      x.pstar = 2;
      x.musicomp(1, g, c);
      Check.eq(x.pstar, 2, "screen_musicomp_pstar_2_frozen");

      resetAll();
      c = new Control(FULL.m);
      c.enter = true;
      x.musicomp(1, g, c);
      Check.eq(x.fase, 0, "screen_musicomp_enter_fase_0");
      Check.ok(!c.enter, "screen_musicomp_enter_cleared");
      resetAll();
      c = new Control(FULL.m);
      c.handb = true;
      x.musicomp(1, g, c);
      Check.eq(x.fase, 0, "screen_musicomp_handb_fase_0");
      Check.ok(!c.handb, "screen_musicomp_handb_cleared");

      // ---- finish (:2083-2276)
      finishPins();

      // ---- stat's result arms (:1515-1623)
      statPins();

      // ---- loadmusic's phase tail (:1326-1339)
      resetAll();
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      x.loadedt[0] = true;
      x.pcontin = 7;
      x.mutem = true;
      x.mutes = true;
      MODLOG.clear();
      x.loadmusic(1, 0, g);
      Check.eq(x.fase, 6, "screen_loadmusic_var0_fase_6");
      Check.eq(x.pcontin, 0, "screen_loadmusic_var0_pcontin_0");
      Check.ok(!x.mutem, "screen_loadmusic_var0_mutem_false");
      Check.ok(!x.mutes, "screen_loadmusic_var0_mutes_false");
      Check.ok(MODLOG.contains("track0.play"), "screen_loadmusic_var0_plays_loaded_track");

      resetAll();
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      x.loadedt[0] = true;
      x.pcontin = 7;
      x.mutem = true;
      x.mutes = true;
      MODLOG.clear();
      x.loadmusic(1, 80, g);
      Check.eq(x.fase, 176, "screen_loadmusic_var80_fase_176");
      Check.eq(x.pcontin, 0, "screen_loadmusic_var80_pcontin_0");
      Check.ok(!x.mutem && !x.mutes, "screen_loadmusic_var80_mutes_reset");
      Check.ok(MODLOG.isEmpty(), "screen_loadmusic_var80_never_plays");

      // ---- loadingfailed (:3982-4000)
      resetAll();
      c = new Control(FULL.m);
      c.enter = true;
      x.loadingfailed(1, c, g);
      Check.eq(x.fase, 2, "screen_loadingfailed_enter_fase_2");
      Check.ok(!c.enter, "screen_loadingfailed_enter_cleared");
      resetAll();
      c = new Control(FULL.m);
      c.handb = true;
      x.loadingfailed(1, c, g);
      Check.eq(x.fase, 2, "screen_loadingfailed_handb_fase_2");

      // ---- ctachm (:3706-3899)
      ctachmPins();
   }

   private static void finishPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      // finish does NOT set a font before the "You completed stage" banner
      // (:2088-2089), so the banner x is whatever the caller left current;
      // pin it with an explicit SansSerif 13 so the x is stable.
      g.setFont(new Font("SansSerif", 1, 13));
      x.ftm = g.getFontMetrics();
      Control c;
      CheckPoints cp;

      // winner at the unlocked, non-final stage: pin 0, sc[0] and the car
      // table for stage 2
      resetAll();
      x.winner = true;
      x.unlocked = 2;
      cp = new CheckPoints();
      cp.stage = 2;
      cp.name = "N";
      ContO[] cars = freshCars();
      g.log.clear();
      x.finish(cp, cars, new Control(FULL.m), g);
      Check.eq(x.pin, 0, "screen_finish_stage2_pin_0");
      Check.eq(x.sc[0], 5, "screen_finish_stage2_sc0_5");
      Check.eq(cars[5].y, 60, "screen_finish_stage2_car_y_60");
      Check.eq(cars[5].z, 1500, "screen_finish_stage2_car_z_1500");
      Check.eq(cars[5].x, 0, "screen_finish_stage2_car_x_0");
      Check.eq(cars[5].xz, 5, "screen_finish_stage2_car_xz_plus5");
      Check.eq(cars[5].zy, 0, "screen_finish_stage2_car_zy_0");
      Check.eq(cars[5].wzy, -10, "screen_finish_stage2_car_wzy_minus10");
      Check.eq(FULL.m.ground, 2470, "screen_finish_stage2_m_ground_2470");
      Check.eq(FULL.m.x, -275, "screen_finish_stage2_m_x_minus275");
      Check.eq(FULL.m.y, -500, "screen_finish_stage2_m_y_minus500");
      Check.eq(FULL.m.z, -50, "screen_finish_stage2_m_z_minus50");
      Check.eq(FULL.m.xz, 0, "screen_finish_stage2_m_xz_0");
      Check.eq(FULL.m.zy, 10, "screen_finish_stage2_m_zy_10");
      Check.ok(FULL.m.crs, "screen_finish_stage2_m_crs_true");
      Check.ok(g.has("img 269x38 140,30"), "screen_finish_stage2_congrd_at_140_30");
      Check.ok(g.has("str \"You completed stage 2 : N !\" " + csx("You completed stage 2 : N !", 13) + ",80 90a7ff"),
            "screen_finish_stage2_banner"); // mac-font x was 183
      Check.ok(g.has("str \"Stage 3 unlocked!\" " + csx("Stage 3 unlocked!", 13) + ",120 d0f0ff"),
            "screen_finish_stage2_unlock_text"); // mac-font x was 215
      Check.ok(g.has("str \"MAX Revenge has been unlocked!\" " + csx("MAX Revenge has been unlocked!", 13) + ",300 d0f0ff"),
            "screen_finish_stage2_car_name_text"); // mac-font x was 160
      Check.ok(g.has("str \"( Game Saved )\" " + csx("( Game Saved )", 11) + ",335 969696"),
            "screen_finish_stage2_saved_text"); // mac-font x was 234
      Check.ok(g.has("fillRect 200,170,150,100 d0f0ff"), "screen_finish_stage2_car_frame_fill");
      Check.ok(g.has("img 90x22 230,350"), "screen_finish_stage2_contin_at_350_minus_pin");

      // stages 4/6/8/10: the rest of the table
      int[] stages = {4, 6, 8, 10};
      int[] pins = {0, 0, 0, 0};
      int[] sc0 = {6, 7, 8, 9};
      int[] ys = {40, 25, 70, 80};
      for (int i = 0; i < stages.length; i++) {
         resetAll();
         x.winner = true;
         x.unlocked = stages[i];
         cp = new CheckPoints();
         cp.stage = stages[i];
         cp.name = "N";
         cars = freshCars();
         x.finish(cp, cars, new Control(FULL.m), g);
         Check.eq(x.pin, pins[i], "screen_finish_stage" + stages[i] + "_pin");
         Check.eq(x.sc[0], sc0[i], "screen_finish_stage" + stages[i] + "_sc0");
         Check.eq(cars[sc0[i]].y, ys[i], "screen_finish_stage" + stages[i] + "_car_y");
      }

      // default pin 60 for a stage with no unlock entry (1)
      resetAll();
      x.winner = true;
      x.unlocked = 1;
      cp = new CheckPoints();
      cp.stage = 1;
      cp.name = "N";
      x.finish(cp, freshCars(), new Control(FULL.m), g);
      Check.eq(x.pin, 60, "screen_finish_stage1_pin_60");

      // final stage: pin 40, no progression
      resetAll();
      x.winner = true;
      x.unlocked = 11;
      cp = new CheckPoints();
      cp.stage = 11;
      cp.name = "N";
      x.finish(cp, freshCars(), new Control(FULL.m), g);
      Check.eq(x.pin, 40, "screen_finish_stage11_pin_40");

      // loss and winner-at-locked both pin 100
      resetAll();
      x.winner = false;
      x.unlocked = 1;
      cp = new CheckPoints();
      cp.stage = 2;
      cp.name = "N";
      x.finish(cp, freshCars(), new Control(FULL.m), g);
      Check.eq(x.pin, 100, "screen_finish_loss_pin_100");
      resetAll();
      x.winner = true;
      x.unlocked = 1;
      cp = new CheckPoints();
      cp.stage = 2;
      cp.name = "N";
      x.finish(cp, freshCars(), new Control(FULL.m), g);
      Check.eq(x.pin, 100, "screen_finish_winner_at_locked_pin_100");

      // enter: fase 10, track stop, and the stage/unlocked promotion
      resetAll();
      x.winner = true;
      x.unlocked = 1;
      cp = new CheckPoints();
      cp.stage = 1;
      cp.name = "N";
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      c = new Control(FULL.m);
      c.enter = true;
      MODLOG.clear();
      x.finish(cp, freshCars(), c, g);
      Check.eq(x.fase, 10, "screen_finish_enter_fase_10");
      Check.ok(MODLOG.contains("track0.stop"), "screen_finish_enter_stops_loaded_track");
      Check.eq(cp.stage, 2, "screen_finish_enter_stage_plus1");
      Check.eq(x.unlocked, 2, "screen_finish_enter_unlocked_plus1");
      Check.ok(!c.enter, "screen_finish_enter_cleared");

      // final stage: no promotion
      resetAll();
      x.winner = true;
      x.unlocked = 11;
      cp = new CheckPoints();
      cp.stage = 11;
      cp.name = "N";
      c = new Control(FULL.m);
      c.handb = true;
      x.finish(cp, freshCars(), c, g);
      Check.eq(cp.stage, 11, "screen_finish_stage11_no_stage_advance");
      Check.eq(x.unlocked, 11, "screen_finish_stage11_no_unlocked_advance");
      Check.eq(x.fase, 10, "screen_finish_stage11_handb_fase_10");

      // winner==false: enter still returns to 10 but no promotion
      resetAll();
      x.winner = false;
      x.unlocked = 1;
      cp = new CheckPoints();
      cp.stage = 1;
      cp.name = "N";
      c = new Control(FULL.m);
      c.enter = true;
      x.finish(cp, freshCars(), c, g);
      Check.eq(cp.stage, 1, "screen_finish_loss_no_stage_advance");
      Check.eq(x.unlocked, 1, "screen_finish_loss_no_unlocked_advance");
      Check.eq(x.fase, 10, "screen_finish_loss_enter_fase_10");
   }

   /** B10(1) / :1515-1623 - the five result arms, holdcnt and the
    *  no-result enter. */
   private static void statPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      Control c;
      CheckPoints cp;
      Madness mad;

      // wasted==4 -> winner, haltall, holdcnt++ ; holdcnt>250 -> fase -2
      resetAll();
      x.wasted = 4;
      x.holdcnt = 251;
      x.winner = false;
      x.fase = 0;
      cp = new CheckPoints();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      c = new Control(FULL.m);
      x.stat(mad, cp, c, false, g);
      Check.ok(x.winner, "screen_stat_wasted4_winner_true");
      Check.ok(cp.haltall, "screen_stat_wasted4_haltall_true");
      Check.eq(x.fase, -2, "screen_stat_holdcnt_251_fase_minus2");
      Check.eq(x.holdcnt, 252, "screen_stat_wasted4_holdcnt_incremented");

      // enter on any result screen -> fase -2 and enter cleared
      resetAll();
      x.wasted = 4;
      x.fase = 0;
      cp = new CheckPoints();
      c = new Control(FULL.m);
      c.enter = true;
      x.stat(new Madness(FULL.m, new Record(FULL.m), x, 0), cp, c, false, g);
      Check.eq(x.fase, -2, "screen_stat_result_enter_fase_minus2");
      Check.ok(!c.enter, "screen_stat_result_enter_cleared");

      // player first -> winner true, haltall true
      resetAll();
      cp = new CheckPoints();
      cp.pos[0] = 0;
      x.stat(new Madness(FULL.m, new Record(FULL.m), x, 0), cp, new Control(FULL.m), false, g);
      Check.ok(x.winner, "screen_stat_player_first_winner_true");
      Check.ok(cp.haltall, "screen_stat_player_first_haltall_true");

      // AI first -> winner false, haltall true
      resetAll();
      x.winner = true;
      cp = new CheckPoints();
      cp.pos[1] = 0;
      x.stat(new Madness(FULL.m, new Record(FULL.m), x, 0), cp, new Control(FULL.m), false, g);
      Check.ok(!x.winner, "screen_stat_ai_first_winner_false");
      Check.ok(cp.haltall, "screen_stat_ai_first_haltall_true");

      // "you were wasted" (dest && cntwis==8): winner false, var6 true,
      // but haltall is NEVER written in this branch (B10 correction)
      resetAll();
      x.winner = true;
      x.cntwis = KX.intv("cntwis_dest");
      cp = new CheckPoints();
      cp.haltall = false;
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      mad.dest = true;
      x.stat(mad, cp, new Control(FULL.m), false, g);
      Check.ok(!x.winner, "screen_stat_player_wasted_winner_false");
      Check.ok(!cp.haltall, "screen_stat_player_wasted_haltall_untouched_false");
      Check.eq(x.holdcnt, 1, "screen_stat_player_wasted_holdcnt_1");

      // no result + enter -> stop track, fase -6
      resetAll();
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      MODLOG.clear();
      cp = new CheckPoints();
      cp.stage = 1;
      c = new Control(FULL.m);
      c.enter = true;
      x.stat(new Madness(FULL.m, new Record(FULL.m), x, 0), cp, c, false, g);
      Check.eq(x.fase, -6, "screen_stat_noresult_enter_fase_minus6");
      Check.ok(MODLOG.contains("track0.stop"), "screen_stat_noresult_enter_stops_track");
      Check.ok(!c.enter, "screen_stat_noresult_enter_cleared");

      // no result clears a stale holdcnt
      resetAll();
      x.holdcnt = 40;
      x.stat(new Madness(FULL.m, new Record(FULL.m), x, 0), new CheckPoints(), new Control(FULL.m), false, g);
      Check.eq(x.holdcnt, 0, "screen_stat_noresult_holdcnt_reset_0");
   }

   /** B10(1) / :3706-3899 - the hitbox machines that convert a mouse press
    *  and release into Control flags. */
   private static void ctachmPins() {
      xtGraphics x = FULL.xt;
      Control c;

      // fase 1: next / back / contin1
      resetAll();
      x.fase = 1;
      c = new Control(FULL.m);
      int nx = 440 + x.next[0].getWidth(null) / 2;
      int ny = 110 + x.next[0].getHeight(null) / 2;
      x.ctachm(nx, ny, 1, c);
      Check.eq(x.pnext, 1, "screen_ctachm_f1_next_press");
      x.ctachm(nx, ny, 2, c);
      Check.ok(c.right, "screen_ctachm_f1_next_release_sets_right");
      Check.eq(x.pnext, 1, "screen_ctachm_f1_next_flag_not_cleared_by_release");

      int bx = 50 + x.back[0].getWidth(null) / 2;
      int by = 110 + x.back[0].getHeight(null) / 2;
      x.ctachm(bx, by, 1, c);
      Check.eq(x.pback, 1, "screen_ctachm_f1_back_press");
      x.ctachm(bx, by, 2, c);
      Check.ok(c.left, "screen_ctachm_f1_back_release_sets_left");

      int cx = 232 + x.contin1[0].getWidth(null) / 2;
      int cy = 170 + x.contin1[0].getHeight(null) / 2;
      x.ctachm(cx, cy, 1, c);
      Check.eq(x.pcontin, 1, "screen_ctachm_f1_contin_press");
      x.ctachm(cx, cy, 2, c);
      Check.ok(c.enter, "screen_ctachm_f1_contin_release_sets_enter");
      // a miss latches nothing
      resetAll();
      x.fase = 1;
      c = new Control(FULL.m);
      x.ctachm(0, 0, 1, c);
      Check.eq(x.pnext + x.pback + x.pcontin, 0, "screen_ctachm_f1_miss_no_latch");

      // fase 10: press selects, release with shaded turns into enter
      int[][] boxes = {{218, 246, 110, 22}, {174, 275, 196, 22}, {230, 306, 85, 22}};
      for (int i = 0; i < boxes.length; i++) {
         resetAll();
         x.fase = 10;
         x.opselect = 1;
         x.shaded = false;
         c = new Control(FULL.m);
         x.ctachm(boxes[i][0] + 2, boxes[i][1] + 2, 1, c);
         Check.eq(x.opselect, i, "screen_ctachm_f10_press_opselect_" + i);
         Check.ok(x.shaded, "screen_ctachm_f10_press_shaded_" + i);
         x.ctachm(0, 0, 2, c);
         Check.ok(c.enter, "screen_ctachm_f10_release_enter_" + i);
         Check.ok(!x.shaded, "screen_ctachm_f10_release_unshades_" + i);
      }
      // release without a press does nothing
      resetAll();
      x.fase = 10;
      x.shaded = false;
      c = new Control(FULL.m);
      x.ctachm(0, 0, 2, c);
      Check.ok(!c.enter, "screen_ctachm_f10_release_without_shaded_no_enter");

      // fase -7: the four pause boxes
      int[][] pause = {{204, 143, 137, 22}, {195, 171, 155, 22}, {178, 197, 190, 22}, {216, 223, 109, 22}};
      for (int i = 0; i < pause.length; i++) {
         resetAll();
         x.fase = -7;
         x.opselect = 7;
         x.shaded = false;
         c = new Control(FULL.m);
         x.ctachm(pause[i][0] + 2, pause[i][1] + 2, 1, c);
         Check.eq(x.opselect, i, "screen_ctachm_neg7_press_opselect_" + i);
         Check.ok(x.shaded, "screen_ctachm_neg7_press_shaded_" + i);
         x.ctachm(0, 0, 2, c);
         Check.ok(c.enter, "screen_ctachm_neg7_release_enter_" + i);
         Check.ok(!x.shaded, "screen_ctachm_neg7_release_unshades_" + i);
      }

      // fase 6: the star toggle (pstar 2 on press, 1 on release)
      resetAll();
      x.fase = 6;
      x.pstar = 0;
      c = new Control(FULL.m);
      int sx = 234 + x.star[0].getWidth(null) / 2;
      int sy = 280 + x.star[0].getHeight(null) / 2;
      x.ctachm(sx, sy, 1, c);
      Check.eq(x.pstar, 2, "screen_ctachm_f6_star_press_pstar_2");
      x.ctachm(sx, sy, 2, c);
      Check.ok(c.enter, "screen_ctachm_f6_star_release_enter");
      Check.eq(x.pstar, 1, "screen_ctachm_f6_star_release_pstar_1");

      // fase 8: the credits next box
      resetAll();
      x.fase = 8;
      c = new Control(FULL.m);
      int ex = 460 + x.next[0].getWidth(null) / 2;
      int ey = 370 + x.next[0].getHeight(null) / 2;
      x.ctachm(ex, ey, 1, c);
      Check.eq(x.pnext, 1, "screen_ctachm_f8_next_press");
      x.ctachm(ex, ey, 2, c);
      Check.ok(c.enter, "screen_ctachm_f8_release_enter");

      // a different fase latches nothing at all
      resetAll();
      x.fase = 2;
      c = new Control(FULL.m);
      x.ctachm(nx, ny, 1, c);
      Check.eq(x.pnext + x.pcontin + x.pback, 0, "screen_ctachm_other_fase_no_latch");
   }

   // =====================================================================
   // 4 - the audio hub (B10 group 2, FLAGS ONLY)
   // =====================================================================

   /** B10(2) - selection indices via recording clips, gates via flags. No
    *  assertion here ever inspects device state: the clips are the seam's
    *  SILENT stub during the ctor and named recording stubs afterwards. */
   private static void audioPins() {
      xtGraphics x = FULL.xt;
      installAudio(x);

      sparkengPins();
      crashPins();
      skidPins();
      playsoundsPins();
      countdownPins();
      checkpointPins();
      powerupPins();
      wastdPins();
      copyGatePins();
      stopallnowPins();
   }

   /** B10(2) / :2385-2400. The index is `enginsignature[sc[0]][slot]`, the
    *  slot is the post-increment argument. sparkeng(-2) cannot NPE because
    *  the loop only dereferences a slot whose pengs[] is true. */
   private static void sparkengPins() {
      xtGraphics x = FULL.xt;
      resetAll();
      // a fully null engine table + all-false pengs: -2 dereferences nothing
      x.engs = new AudioClip[2][5];
      for (int i = 0; i < 5; i++) {
         x.pengs[i] = false;
      }
      AUDIO.clear();
      x.sparkeng(-2);
      Check.ok(true, "audio_sparkeng_minus2_null_engs_no_npe");
      Check.eq(AUDIO.size(), 0, "audio_sparkeng_minus2_plays_nothing");
      Check.eq(Arrays.toString(x.pengs), "[false, false, false, false, false]", "audio_sparkeng_minus2_pengs_false");

      installAudio(x);
      resetAll();
      AUDIO.clear();
      x.sparkeng(-1);
      Check.eq(AUDIO.toString(), "[eng00.loop]", "audio_sparkeng_minus1_starts_slot0");
      Check.eq(Arrays.toString(x.pengs), "[true, false, false, false, false]", "audio_sparkeng_minus1_pengs0");

      resetAll();
      AUDIO.clear();
      x.sparkeng(2);
      Check.eq(AUDIO.toString(), "[eng13.loop]", "audio_sparkeng_2_slot3_via_enginsignature");
      Check.eq(Arrays.toString(x.pengs), "[false, false, false, true, false]", "audio_sparkeng_2_pengs3");

      resetAll();
      AUDIO.clear();
      x.sparkeng(-1);
      AUDIO.clear();
      x.sparkeng(0);
      Check.eq(AUDIO.toString(), "[eng00.stop, eng01.loop]", "audio_sparkeng_0_stops_slot0_then_starts_slot1");

      resetAll();
      x.sc[0] = 5;
      AUDIO.clear();
      x.sparkeng(-1);
      x.sparkeng(0);
      // stop of the previously playing slot 0 comes first (the loop is
      // ascending), then the start of slot 1 via row 5's signature
      Check.eq(AUDIO.toString(), "[eng00.loop, eng00.stop, eng11.loop]",
            "audio_sparkeng_signature_row5_slot1_is_row1");

      resetAll();
      AUDIO.clear();
      x.sparkeng(-1);
      AUDIO.clear();
      x.sparkeng(4);
      Check.eq(AUDIO.toString(), "[eng00.stop]", "audio_sparkeng_4_stops_all");
      Check.eq(Arrays.toString(x.pengs), "[false, false, false, false, false]", "audio_sparkeng_4_pengs_all_false");
   }

   /** B10(2) / :935-997. */
   private static void crashPins() {
      xtGraphics x = FULL.xt;

      // var2==0, 25 < |v| < 170: lowcrash[crshturn], bfcrash 2, rotate up
      resetAll();
      AUDIO.clear();
      x.crash(100.0F, 0);
      Check.eq(AUDIO.toString(), "[lowcrash1.play]", "audio_crash_100_plays_lowcrash_0");
      Check.eq(x.bfcrash, 2, "audio_crash_100_bfcrash_2");
      Check.eq(x.crshturn, 1, "audio_crash_100_crshturn_rotates_to_1");
      // a non-zero bfcrash swallows the whole call, rotation included
      AUDIO.clear();
      x.crash(200.0F, 0);
      Check.eq(AUDIO.size(), 0, "audio_crash_bfcrash_swallows_play");
      Check.eq(x.bfcrash, 2, "audio_crash_bfcrash_swallowed_keeps_bfcrash");
      Check.eq(x.crshturn, 1, "audio_crash_bfcrash_swallowed_no_rotation");

      // |v| > 170: crash[crshturn]
      resetAll();
      AUDIO.clear();
      x.crash(200.0F, 0);
      Check.eq(AUDIO.toString(), "[crash1.play]", "audio_crash_200_plays_crash_0");
      Check.eq(x.bfcrash, 2, "audio_crash_200_bfcrash_2");
      Check.eq(x.crshturn, 1, "audio_crash_200_crshturn_1");

      // strict thresholds
      resetAll();
      AUDIO.clear();
      x.crash(25.0F, 0);
      Check.eq(AUDIO.size(), 0, "audio_crash_25_strict_no_play");
      Check.eq(x.bfcrash, 0, "audio_crash_25_bfcrash_0");
      Check.eq(x.crshturn, 0, "audio_crash_25_no_rotation");
      resetAll();
      AUDIO.clear();
      x.crash(170.0F, 0);
      Check.eq(AUDIO.size(), 0, "audio_crash_170_strict_no_play");
      Check.eq(x.bfcrash, 0, "audio_crash_170_bfcrash_0");
      Check.eq(x.crshturn, 1, "audio_crash_170_rotation_still_runs");

      // crshturn cycle: up 0 -> -1 -> 2, down 2 -> 3 -> 0
      resetAll();
      x.crashup = true;
      x.crshturn = 0;
      x.crash(100.0F, 0);
      Check.eq(x.crshturn, 2, "audio_crash_up_0_wraps_to_2");
      resetAll();
      x.crashup = false;
      x.crshturn = 2;
      x.crash(100.0F, 0);
      Check.eq(x.crshturn, 0, "audio_crash_down_2_wraps_to_0");
      resetAll();
      x.crashup = false;
      x.crshturn = 0;
      x.crash(100.0F, 0);
      Check.eq(x.crshturn, 1, "audio_crash_down_0_to_1");

      // mutes silences the clip but keeps the flags
      resetAll();
      x.mutes = true;
      AUDIO.clear();
      x.crash(100.0F, 0);
      Check.eq(AUDIO.size(), 0, "audio_crash_mutes_no_play");
      Check.eq(x.bfcrash, 2, "audio_crash_mutes_bfcrash_2");
      Check.eq(x.crshturn, 1, "audio_crash_mutes_still_rotates");

      // var2==-1: always index 2, never rotates
      resetAll();
      AUDIO.clear();
      x.crshturn = 1;
      x.crash(100.0F, -1);
      Check.eq(AUDIO.toString(), "[lowcrash3.play]", "audio_crash_minus1_lowcrash_index2");
      Check.eq(x.bfcrash, 2, "audio_crash_minus1_bfcrash_2");
      Check.eq(x.crshturn, 1, "audio_crash_minus1_no_rotation");
      resetAll();
      AUDIO.clear();
      x.crash(200.0F, -1);
      Check.eq(AUDIO.toString(), "[crash3.play]", "audio_crash_minus1_high_index2");
      Check.eq(x.crshturn, 0, "audio_crash_minus1_high_no_rotation");

      // var2==1: tires, bfcrash 3, no magnitude test
      resetAll();
      AUDIO.clear();
      x.crash(0.0F, 1);
      Check.eq(AUDIO.toString(), "[tires.play]", "audio_crash_tires_any_speed");
      Check.eq(x.bfcrash, 3, "audio_crash_tires_bfcrash_3");
      Check.eq(x.crshturn, 0, "audio_crash_tires_no_rotation");
   }

   /** B10(2) / :3414-3440. */
   private static void skidPins() {
      xtGraphics x = FULL.xt;

      resetAll();
      AUDIO.clear();
      x.skid(0, 151.0F);
      Check.eq(AUDIO.toString(), "[skid1.play]", "audio_skid_0_skid1");
      Check.eq(x.skflg, 1, "audio_skid_0_skflg_1");
      Check.eq(x.bfskid, 35, "audio_skid_0_bfskid_35");
      // second call is swallowed by bfskid
      AUDIO.clear();
      x.skid(1, 151.0F);
      Check.eq(AUDIO.size(), 0, "audio_skid_bfskid_swallows");
      Check.eq(x.dskflg, 0, "audio_skid_bfskid_swallow_dskflg_0");

      resetAll();
      AUDIO.clear();
      x.skid(1, 151.0F);
      Check.eq(AUDIO.toString(), "[dustskid1.play]", "audio_skid_1_dustskid1");
      Check.eq(x.dskflg, 1, "audio_skid_1_dskflg_1");
      Check.eq(x.bfskid, 35, "audio_skid_1_bfskid_35");

      resetAll();
      x.skid(0, 150.0F);
      Check.eq(x.bfskid, 0, "audio_skid_150_strict_no_write");
      resetAll();
      x.bfcrash = 1;
      x.skid(0, 151.0F);
      Check.eq(x.bfskid, 0, "audio_skid_bfcrash_blocks");

      // alternation 0 -> 1 -> 0 (bfskid cleared by hand between calls)
      resetAll();
      AUDIO.clear();
      x.skid(0, 151.0F);
      x.bfskid = 0;
      x.skid(0, 151.0F);
      Check.eq(AUDIO.toString(), "[skid1.play, skid2.play]", "audio_skid_skflg_alternates_0_1_0");
      Check.eq(x.skflg, 0, "audio_skid_skflg_back_to_0");
      resetAll();
      AUDIO.clear();
      x.skid(1, 151.0F);
      x.bfskid = 0;
      x.skid(1, 151.0F);
      Check.eq(AUDIO.toString(), "[dustskid1.play, dustskid2.play]", "audio_skid_dskflg_alternates_0_1_0");
      Check.eq(x.dskflg, 0, "audio_skid_dskflg_back_to_0");

      // mutes: no clip, flags still written
      resetAll();
      x.mutes = true;
      AUDIO.clear();
      x.skid(0, 151.0F);
      Check.eq(AUDIO.size(), 0, "audio_skid_mutes_no_play");
      Check.eq(x.skflg, 1, "audio_skid_mutes_skflg_still_1");
      Check.eq(x.bfskid, 35, "audio_skid_mutes_bfskid_35");
   }

   /** B10(2) / :757-933 - the gated body, the air/engine selection and the
    *  stopcnt/bfcrash/bfskid tails. */
   private static void playsoundsPins() {
      xtGraphics x = FULL.xt;
      Control c;
      Madness mad;

      // ---- the no-throttle branch: two air draws and air[5] fixed
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      x.cntwis = 0;
      mad.mtouch = false;
      mad.wtouch = false;
      x.grrd = false;
      x.aird = false;
      preset(9, 9, 5); // 0.8, 0.3, 0.6, ...
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      // draw1 0.8 > 0.4 -> air[(int)(0.3*4)=1], grrd, stopcnt 5;
      // then !wtouch && !aird -> stopairs (6 stops) + air[(int)(0.6*4)=2],
      // stopcnt 10, aird; then sparkeng(-1) starts slot 0
      Check.ok(AUDIO.contains("air1.loop"), "audio_playsounds_air_random_index_1");
      Check.ok(AUDIO.contains("air2.loop"), "audio_playsounds_air_random_index_2");
      Check.eq(count(AUDIO, ".stop"), 6, "audio_playsounds_stopairs_six_stops");
      Check.eq(x.stopcnt, 9, "audio_playsounds_aird_stopcnt_10_then_tail_decrement");
      Check.ok(x.grrd, "audio_playsounds_grrd_true");
      Check.ok(x.aird, "audio_playsounds_aird_true");
      Check.ok(AUDIO.contains("eng00.loop"), "audio_playsounds_no_throttle_sparkeng_minus1_slot0");

      // second call: grrd/aird already true -> no air draws; sparkeng(-1)
      // is also silent because slot 0 is already the running engine
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(AUDIO.toString(), "[]", "audio_playsounds_second_call_no_air_no_reengine");

      // ---- throttle up below 10 with mtouch: air[5] fixed and var6
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      mad.mtouch = true;
      mad.speed = 5.0F;
      mad.power = 50.0F;
      mad.swits[0] = new int[]{50, 180, 280};
      c.up = true;
      preset11();
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("air5.loop"), "audio_playsounds_var6_up_starts_air5");
      Check.eq(x.stopcnt, 9, "audio_playsounds_var6_stopcnt_10_then_tail_decrement");

      // ---- skid branch without var6: air[2 + (int)(random*3)]
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      x.stopcnt = -2;
      mad.mtouch = true;
      mad.speed = 100.0F;
      mad.skid = 1;
      c.up = true;
      c.handb = true;
      preset11();
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("air4.loop"), "audio_playsounds_skid_air_2_plus_random3");
      Check.eq(x.stopcnt, 6, "audio_playsounds_skid_stopcnt_7_then_tail_decrement");

      // ---- capped car: sparkeng(3) -> slot 4
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      mad.mtouch = true;
      mad.speed = 10.0F;
      mad.capsized = true;
      c.up = true;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("eng04.loop"), "audio_playsounds_capsized_sparkeng3_slot4");

      // ---- the engine ladder: speed 40 in [0, swits[0]) -> var7 2, so
      // pwait ticks 7->6 and sparkeng(2) picks slot 3
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      mad.mtouch = true;
      mad.power = 50.0F;
      mad.speed = 40.0F;
      mad.swits[0] = new int[]{50, 180, 280};
      c.up = true;
      x.pwait = 7;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(x.pwait, 6, "audio_playsounds_speed40_pwait_7_to_6");
      Check.ok(AUDIO.contains("eng13.loop"), "audio_playsounds_speed40_sparkeng2_slot3");
      // pwait==0 turns the 2 into a 0 -> slot 1
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      mad.mtouch = true;
      mad.power = 50.0F;
      mad.speed = 40.0F;
      mad.swits[0] = new int[]{50, 180, 280};
      c.up = true;
      x.pwait = 0;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("eng01.loop"), "audio_playsounds_pwait0_var7_becomes_0_slot1");

      // ---- power 98: var10 2 normally, 3 when pwait==0 and speed is past
      // swits[0][1]
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      mad.mtouch = true;
      mad.power = 98.0F;
      mad.speed = 200.0F;
      mad.swits[0] = new int[]{50, 180, 280};
      c.up = true;
      x.pwait = 5;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("eng13.loop"), "audio_playsounds_power98_pwait5_var10_2_slot3");
      Check.eq(x.pwait, 4, "audio_playsounds_power98_pwait_decrements");
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      mad.mtouch = true;
      mad.power = 98.0F;
      mad.speed = 200.0F;
      mad.swits[0] = new int[]{50, 180, 280};
      c.up = true;
      x.pwait = 0;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("eng04.loop"), "audio_playsounds_power98_pwait0_fast_var10_3_slot4");

      // ---- the tail: bfcrash/bfskid decrement, cntwis reset on newcar
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 111;
      x.bfcrash = 2;
      x.bfskid = 3;
      x.cntwis = 4;
      mad.newcar = true;
      x.playsounds(mad, c, 1);
      Check.eq(x.bfcrash, 1, "audio_playsounds_tail_bfcrash_decrements");
      Check.eq(x.bfskid, 2, "audio_playsounds_tail_bfskid_decrements");
      Check.eq(x.cntwis, 0, "audio_playsounds_tail_newcar_cntwis_0");

      // the gated-off fase does sparkeng(-2) (stop all) only
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 111;
      AUDIO.clear();
      x.sparkeng(-1); // enter with slot 0 playing
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(AUDIO.toString(), "[eng00.stop]", "audio_playsounds_off_fase_sparkeng_minus2_stops_all");

      // stopcnt: 1 -> stopairs + 0; then -1; -19 -> -20; -20 freezes
      resetAll();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      c = new Control(FULL.m);
      x.fase = 111;
      x.stopcnt = 1;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(count(AUDIO, ".stop"), 6, "audio_stopcnt_1_fires_stopairs_six");
      Check.eq(x.stopcnt, 0, "audio_stopcnt_1_to_0");
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(AUDIO.size(), 0, "audio_stopcnt_0_no_stopairs");
      Check.eq(x.stopcnt, -1, "audio_stopcnt_0_to_minus1");
      x.stopcnt = -19;
      x.playsounds(mad, c, 1);
      Check.eq(x.stopcnt, -20, "audio_stopcnt_minus19_to_minus20");
      x.playsounds(mad, c, 1);
      Check.eq(x.stopcnt, -20, "audio_stopcnt_minus20_frozen");
   }

   /** B10(2) / stat's countdown block :1718-1750. */
   private static void countdownPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      Control c;
      CheckPoints cp;
      Madness mad;

      resetAll();
      x.fase = 0;
      c = new Control(FULL.m);
      cp = new CheckPoints();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      AUDIO.clear();
      x.starcnt = KX.intv("starcnt_landing");
      x.gocnt = KX.intv("gocnt_start");
      x.mutes = false;
      x.stat(mad, cp, c, false, g);
      Check.ok(AUDIO.contains("three.play"), "audio_countdown_35_three");
      Check.eq(x.gocnt, KX.intv("gocnt_start"), "audio_countdown_35_gocnt_untouched_3");

      x.starcnt = KX.intv("starcnt_two");
      AUDIO.clear();
      x.stat(mad, cp, c, false, g);
      Check.ok(AUDIO.contains("two.play"), "audio_countdown_24_two");
      Check.eq(x.gocnt, KX.intv("gocnt_two"), "audio_countdown_24_gocnt_2");

      x.starcnt = KX.intv("starcnt_one");
      AUDIO.clear();
      x.stat(mad, cp, c, false, g);
      Check.ok(AUDIO.contains("one.play"), "audio_countdown_13_one");
      Check.eq(x.gocnt, KX.intv("gocnt_one"), "audio_countdown_13_gocnt_1");

      x.starcnt = KX.intv("starcnt_go");
      AUDIO.clear();
      x.stat(mad, cp, c, false, g);
      Check.ok(AUDIO.contains("go.play"), "audio_countdown_2_go");
      Check.eq(x.gocnt, KX.intv("gocnt_go"), "audio_countdown_2_gocnt_0");

      // mutes: the gocnt writes still happen, the clips do not
      resetAll();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      c = new Control(FULL.m);
      cp = new CheckPoints();
      x.mutes = true;
      x.starcnt = KX.intv("starcnt_two");
      AUDIO.clear();
      x.stat(mad, cp, c, false, g);
      Check.eq(AUDIO.size(), 0, "audio_countdown_mutes_no_clip");
      Check.eq(x.gocnt, KX.intv("gocnt_two"), "audio_countdown_mutes_gocnt_2");
   }

   /** B10(2) / stat's checkpoint block :2045-2059. */
   private static void checkpointPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      CheckPoints cp;
      Madness mad;

      // stat(Madness var1, CheckPoints var2, ...) - :2045 reads VAR1.clear,
      // i.e. Madness.clear (the car's own counter), not CheckPoints.clear
      resetAll();
      x.clear = 0;
      x.cntovn = 5;
      x.cntan = 5;
      cp = new CheckPoints();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      mad.clear = 5;
      AUDIO.clear();
      x.stat(mad, cp, new Control(FULL.m), false, g);
      Check.ok(AUDIO.contains("checkpoint.play"), "audio_checkpoint_new_value_plays");
      Check.eq(x.clear, 5, "audio_checkpoint_clear_cached_5");
      Check.eq(x.cntovn, 0, "audio_checkpoint_cntovn_reset_0");
      Check.eq(x.cntan, 0, "audio_checkpoint_cntan_reset_0");

      // same Madness.clear -> no play
      AUDIO.clear();
      x.stat(mad, cp, new Control(FULL.m), false, g);
      Check.eq(AUDIO.size(), 0, "audio_checkpoint_same_value_no_play");

      // cp value 0 -> no play and no cache write
      resetAll();
      x.clear = 9;
      cp = new CheckPoints();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      mad.clear = 0;
      AUDIO.clear();
      x.stat(new Madness(FULL.m, new Record(FULL.m), x, 0), cp, new Control(FULL.m), false, g);
      Check.eq(AUDIO.size(), 0, "audio_checkpoint_zero_value_no_play");
      Check.eq(x.clear, 9, "audio_checkpoint_zero_value_no_cache_write");

      // mutes: no clip, cache + counters still updated
      resetAll();
      x.mutes = true;
      x.cntovn = 5;
      cp = new CheckPoints();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      mad.clear = 5;
      AUDIO.clear();
      x.stat(mad, cp, new Control(FULL.m), false, g);
      Check.eq(AUDIO.size(), 0, "audio_checkpoint_mutes_no_clip");
      Check.eq(x.clear, 5, "audio_checkpoint_mutes_still_caches");
      Check.eq(x.cntovn, 0, "audio_checkpoint_mutes_cntovn_reset");
   }

   /** B10(2) / the three powerup trigger sites (:1454 credits, :1990 stunt,
    *  :3391 rad). */
   private static void powerupPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      Madness mad;

      resetAll();
      AUDIO.clear();
      x.credits(g, new Control(FULL.m));
      Check.eq(count(AUDIO, "powerup.play"), 1, "audio_powerup_credits_entry");

      resetAll();
      AUDIO.clear();
      x.rad(g, 0);
      Check.eq(count(AUDIO, "powerup.play"), 1, "audio_powerup_rad_zero");
      AUDIO.clear();
      x.rad(g, 1);
      Check.eq(count(AUDIO, "powerup.play"), 0, "audio_powerup_rad_nonzero_silent");

      resetAll();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      mad.trcnt = 10;
      mad.travzy = 0;
      mad.travxy = 0;
      mad.travxz = 0;
      mad.powerup = 0.0F;
      x.auscnt = 40;
      AUDIO.clear();
      x.stat(mad, new CheckPoints(), new Control(FULL.m), false, g);
      Check.eq(count(AUDIO, "powerup.play"), 1, "audio_powerup_stunt_trcnt10_auscnt_lt45");

      // trcnt != 10 -> no powerup
      resetAll();
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      mad.trcnt = 9;
      x.auscnt = 40;
      AUDIO.clear();
      x.stat(mad, new CheckPoints(), new Control(FULL.m), false, g);
      Check.eq(count(AUDIO, "powerup.play"), 0, "audio_powerup_trcnt_not_10_silent");
   }

   /** B10(2) / playsounds' destroyed-car block :860-874 and the cntwis tail
    *  :920-932. */
   private static void wastdPins() {
      xtGraphics x = FULL.xt;
      Control c;
      Madness mad;

      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      x.cntwis = 0;
      mad.cntdest = 1;
      mad.dest = false;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(count(AUDIO, "wastd.loop"), 1, "audio_wastd_loops_once");
      Check.ok(x.pwastd, "audio_wastd_pwastd_true");
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(count(AUDIO, "wastd.loop"), 0, "audio_wastd_pwastd_guard_no_second_loop");

      // cntdest 0 -> stop and clear the guard
      mad.cntdest = 0;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("wastd.stop"), "audio_wastd_stops_when_cntdest_0");
      Check.eq(count(AUDIO, "wastd.loop"), 0, "audio_wastd_no_loop_when_cntdest_0");
      Check.ok(!x.pwastd, "audio_wastd_pwastd_false_after_stop");

      // cntwis==7 with the guard still set -> stop then fire
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      x.cntwis = 7;
      x.pwastd = true;
      mad.cntdest = 1; // the 7->8 tail write only happens when cntdest != 0
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.ok(AUDIO.contains("wastd.stop"), "audio_wastd_cntwis7_stops_first");
      Check.eq(count(AUDIO, "firewasted.play"), 1, "audio_wastd_cntwis7_firewasted_once");
      Check.eq(x.cntwis, 8, "audio_wastd_cntwis_7_to_8_tail");

      // mutes suppresses firewasted
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      x.cntwis = 7;
      x.mutes = true;
      AUDIO.clear();
      x.playsounds(mad, c, 1);
      Check.eq(count(AUDIO, "firewasted.play"), 0, "audio_wastd_mutes_no_firewasted");

      // cntwis increment while dest is set
      resetAll();
      c = new Control(FULL.m);
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 0;
      x.cntwis = 2;
      mad.cntdest = 1;
      mad.dest = true;
      x.playsounds(mad, c, 1);
      Check.eq(x.cntwis, 3, "audio_wastd_dest_increments_cntwis");
   }

   /** B10(2) / :903-918 - the fase-gated mutes/mutem copy. */
   private static void copyGatePins() {
      xtGraphics x = FULL.xt;
      Control c;
      Madness mad;

      resetAll();
      c = new Control(FULL.m);
      c.mutes = true;
      c.mutem = true;
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 111;
      x.playsounds(mad, c, 1);
      Check.ok(!x.mutes, "audio_copygate_off_fase_mutes_not_copied");
      Check.ok(!x.mutem, "audio_copygate_off_fase_mutem_not_copied");

      resetAll();
      c = new Control(FULL.m);
      c.mutes = true;
      c.mutem = true;
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 100;
      x.playsounds(mad, c, 1);
      Check.ok(x.mutes, "audio_copygate_fase0_mutes_copied");
      Check.ok(x.mutem, "audio_copygate_fase0_mutem_copied");

      int[] gate = {6, -1, -2, -3, -4, -5};
      for (int i = 0; i < gate.length; i++) {
         resetAll();
         Control cc = new Control(FULL.m);
         cc.mutes = true;
         x.fase = gate[i];
         x.playsounds(new Madness(FULL.m, new Record(FULL.m), x, 0), cc, 1);
         Check.ok(x.mutes, "audio_copygate_fase" + gate[i] + "_mutes_copied");
      }

      // mutem transition drives the loaded track
      resetAll();
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      c = new Control(FULL.m);
      c.mutem = true;
      mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      x.fase = 0;
      x.starcnt = 100;
      MODLOG.clear();
      x.playsounds(mad, c, 1);
      Check.ok(MODLOG.contains("track0.stop"), "audio_copygate_mutem_true_stops_track");
      MODLOG.clear();
      c.mutem = false;
      x.playsounds(mad, c, 1);
      Check.ok(MODLOG.contains("track0.resume"), "audio_copygate_mutem_false_resumes_track");
      // no transition -> no call
      MODLOG.clear();
      x.playsounds(mad, c, 1);
      Check.ok(MODLOG.isEmpty(), "audio_copygate_mutem_unchanged_no_track_call");
   }

   /** B10(2) / :3530-3555. */
   private static void stopallnowPins() {
      xtGraphics x = FULL.xt;
      resetAll();
      for (int i = 0; i < 5; i++) {
         x.engs[0][i] = new Clip("a" + i, AUDIO);
         x.engs[1][i] = new Clip("b" + i, AUDIO);
      }
      for (int i = 0; i < 6; i++) {
         x.air[i] = new Clip("air" + i, AUDIO);
      }
      x.wastd = new Clip("wastd", AUDIO);
      x.loadedt[0] = true;
      x.stracks[0] = new RecMod("track0", MODLOG, FULL.app);
      MODLOG.clear();
      x.cars = new RecMod("cars", MODLOG, FULL.app);
      x.stages = new RecMod("stages", MODLOG, FULL.app);
      AUDIO.clear();
      x.stopallnow();
      Check.eq(count(AUDIO, ".stop"), 17, "audio_stopallnow_seventeen_clip_stops");
      Check.ok(AUDIO.contains("a0.stop") && AUDIO.contains("b4.stop"), "audio_stopallnow_both_engine_rows");
      Check.ok(AUDIO.contains("air5.stop"), "audio_stopallnow_all_six_air");
      Check.ok(AUDIO.contains("wastd.stop"), "audio_stopallnow_wastd");
      Check.eq(MODLOG.toString(), "[track0.outwithit, cars.outwithit, stages.outwithit]",
            "audio_stopallnow_track_and_three_mods_outwithit");
      // restore the shared RecMods for the remaining tests
      FULL.xt.cars = new RecMod("cars", MODLOG, FULL.app);
      FULL.xt.stages = new RecMod("stages", MODLOG, FULL.app);
   }

   // =====================================================================
   // 5 - draw methods (B10 group 3)
   // =====================================================================

   /** B10(3) - exact pixels where the original paints a solid colour, exact
    *  draw-call geometry/text where it paints text or polygons. */
   private static void drawPins() {
      nofocusPins();
      clicknowPins();
      drawcsPins();
      drawstatPins();
      overPins();
      levelhighPins();
      fleximagePins();
      pauseimagePins();
      loadingPins();
      statHudPins();
      arrowPins();
      drawImageNullPins();
   }

   /** B10(3) / :1101-1113, callable with a bare Graphics and m==null. */
   private static void nofocusPins() {
      xtGraphics x = FULL.xt;
      BufferedImage img = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      Graphics g = img.getGraphics();
      Medium saved = x.m;
      x.m = null; // nofocus never reads m (drawcs var7==3 skips the snap math)
      x.ftm = null;
      x.nofocus(g);
      x.m = saved;
      Check.eq(img.getRGB(0, 0) & 0xffffff, 0xffffff, "draw_nofocus_pixel_0_0_white");
      Check.eq(img.getRGB(20, 20) & 0xffffff, 0xc0c0c0, "draw_nofocus_pixel_20_20_gray");
      Check.eq(img.getRGB(19, 20) & 0xffffff, 0xffffff, "draw_nofocus_border_edge_19_20_white");
      Check.eq(img.getRGB(20, 19) & 0xffffff, 0xffffff, "draw_nofocus_border_edge_20_19_white");
      Check.eq(img.getRGB(549, 200) & 0xffffff, 0xffffff, "draw_nofocus_right_border_white");
      // drawRect(20,20,510,360) puts its right edge exactly on x=530, on
      // top of the white right border fill -> gray
      Check.eq(img.getRGB(530, 200) & 0xffffff, 0xc0c0c0, "draw_nofocus_right_outline_on_border_gray");
      Check.eq(img.getRGB(275, 200) & 0xffffff, 0x000000, "draw_nofocus_interior_untouched_black");
      Check.ok(x.ftm != null, "draw_nofocus_sets_ftm");
   }

   /** B10(3) / :3364-3376 -> :2472 - clicknow never sets ftm. */
   private static void clicknowPins() {
      xtGraphics x = FULL.xt;
      final Graphics g2 = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB).getGraphics();
      x.ftm = null;
      Check.throwsIt(new Check.Task() {
         public void run() {
            FULL.xt.clicknow(g2);
         }
      }, NullPointerException.class, "draw_clicknow_null_ftm_npe");
      try {
         x.clicknow(g2);
      } catch (NullPointerException e) {
         Check.eq(e.getStackTrace()[0].getMethodName(), "drawcs", "draw_clicknow_npe_top_frame_drawcs");
         Check.eq(e.getStackTrace()[0].getLineNumber(), 2472, "draw_clicknow_npe_line_2472");
      }
      x.ftm = FULL.rec.getFontMetrics();
      x.clicknow(g2);
      Check.ok(true, "draw_clicknow_with_ftm_no_exception");
   }

   /** B10(3) / :2402-2473 - the colour transform, the centering formula and
    *  the var7 1/2 shadow copies. */
   private static void drawcsPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      Medium m = FULL.m;
      resetAll();
      g.setFont(new Font("SansSerif", 1, 13));
      x.ftm = g.getFontMetrics();
      int half = x.ftm.stringWidth("AB") / 2;
      int cx = 275 - half;

      m.snap[0] = 50;
      m.snap[1] = 0;
      m.snap[2] = -50;
      g.log.clear();
      x.drawcs(g, 10, "AB", 100, 50, 50, 0);
      Check.eq(g.log.size(), 1, "draw_drawcs_var7_0_one_string");
      Check.eq(g.log.get(0), "str \"AB\" " + cx + ",10 963219", "draw_drawcs_var7_0_transform_up");
      g.log.clear();
      x.drawcs(g, 10, "AB", 100, 50, 50, 1);
      Check.eq(g.log.size(), 2, "draw_drawcs_var7_1_two_strings");
      Check.eq(g.log.get(0), "str \"AB\" " + (cx + 1) + ",11 000000", "draw_drawcs_var7_1_black_shadow");
      Check.eq(g.log.get(1), "str \"AB\" " + cx + ",10 963219", "draw_drawcs_var7_1_main_copy");
      g.log.clear();
      x.drawcs(g, 10, "AB", 100, 50, 50, 2);
      Check.eq(g.log.size(), 2, "draw_drawcs_var7_2_two_strings");
      Check.eq(g.log.get(0), "str \"AB\" " + (cx + 1) + ",11 c2a6b2", "draw_drawcs_var7_2_csky_blend");
      g.log.clear();
      x.drawcs(g, 10, "AB", 100, 50, 50, 3);
      Check.eq(g.log.size(), 1, "draw_drawcs_var7_3_one_string");
      Check.eq(g.log.get(0), "str \"AB\" " + cx + ",10 643232", "draw_drawcs_var7_3_no_transform");
      g.log.clear();
      x.drawcs(g, 10, "AB", 100, 50, 50, 4);
      Check.eq(g.log.size(), 1, "draw_drawcs_var7_4_one_string");
      Check.eq(g.log.get(0), "str \"AB\" " + cx + ",10 32324b", "draw_drawcs_var7_4_transform_down");

      // clamping: 255 on both ends (:2405-2458)
      m.snap[0] = 1000;
      m.snap[1] = -1000;
      m.snap[2] = 0;
      g.log.clear();
      x.drawcs(g, 10, "AB", 200, 200, 0, 0);
      Check.eq(g.log.get(0), "str \"AB\" " + cx + ",10 ff0000", "draw_drawcs_channel_clamped_255_0");
      m.snap[0] = 0;
      m.snap[1] = 0;
      m.snap[2] = 0;

      // the NPE matrix: var7 0 reads m.snap at :2404, var7 4 at :2433,
      // var7 3 never reads m and only ftm at :2472
      Medium saved = x.m;
      x.m = null;
      final Rec rg = g;
      g.log.clear();
      x.drawcs(rg, 10, "AB", 0, 0, 0, 3);
      Check.eq(g.log.size(), 1, "draw_drawcs_var7_3_never_reads_m");
      final Medium keep = saved;
      x.ftm = null;
      Check.throwsIt(new Check.Task() {
         public void run() {
            FULL.xt.drawcs(rg, 10, "AB", 0, 0, 0, 3);
         }
      }, NullPointerException.class, "draw_drawcs_null_ftm_npe");
      x.m = saved;
      x.ftm = g.getFontMetrics();
      Check.throwsIt(new Check.Task() {
         public void run() {
            FULL.xt.m = null;
            FULL.xt.drawcs(rg, 10, "AB", 0, 0, 0, 0);
         }
      }, NullPointerException.class, "draw_drawcs_null_m_var7_0_npe");
      Check.throwsIt(new Check.Task() {
         public void run() {
            FULL.xt.m = null;
            FULL.xt.drawcs(rg, 10, "AB", 0, 0, 0, 4);
         }
      }, NullPointerException.class, "draw_drawcs_null_m_var7_4_npe");
      x.m = keep;
   }

   /** B10(3) / :2730-2873 - the bar geometry, the truncated colour and the
    *  corrected no-ArithmeticException contract. */
   private static void drawstatPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      Medium m = FULL.m;
      resetAll();
      m.snap[0] = 0;
      m.snap[1] = 0;
      m.snap[2] = 0;

      g.log.clear();
      x.drawstat(g, 100, 50, false, 0.0F);
      Check.eq(g.log.size(), 2, "draw_drawstat_two_polygons");
      Check.ok(g.log.get(0).startsWith("fillPoly [422,422,471,471] [11,20,20,11]"),
            "draw_drawstat_bar_geometry_w49");
      Check.eq(g.log.get(0).substring(g.log.get(0).length() - 6),
            rgbHex(KX.intv("drawstat_red"), KX.intv("drawstat_green_mid"), KX.intv("drawstat_blue")),
            "draw_drawstat_colour_244_186_11");
      Check.ok(g.log.get(1).startsWith("fillPoly [422,422,422,422] [31,40,40,31]"),
            "draw_drawstat_power_bar_power0");
      Check.eq(g.log.get(1).substring(g.log.get(1).length() - 6), "80bef4", "draw_drawstat_power_colour");

      // hit clamped to max -> full 98px bar
      g.log.clear();
      x.drawstat(g, 100, 200, false, 0.0F);
      Check.ok(g.log.get(0).startsWith("fillPoly [422,422,520,520] [11,20,20,11]"),
            "draw_drawstat_hit_clamped_to_max");
      Check.ok(g.log.get(0).endsWith(rgbHex(KX.intv("drawstat_red"), KX.intv("drawstat_green_low"),
            KX.intv("drawstat_blue"))), "draw_drawstat_full_bar_colour_244_11_11");

      // power bar width is (int)(422 + power)
      g.log.clear();
      x.drawstat(g, 100, 50, false, 30.0F);
      Check.ok(g.log.get(1).startsWith("fillPoly [422,422,452,452] [31,40,40,31]"),
            "draw_drawstat_power_width_422_plus_power");

      // var4 (newcar) paints the csky backing polygon first
      g.log.clear();
      x.drawstat(g, 100, 50, true, 0.0F);
      Check.eq(g.log.size(), 3, "draw_drawstat_newcar_three_polygons");
      Check.ok(g.log.get(0).startsWith("fillPoly [423,423,520,520] [11,19,19,11]"),
            "draw_drawstat_newcar_backing_polygon");
      Check.ok(g.log.get(0).endsWith("d9e0ff"), "draw_drawstat_newcar_backing_csky");

      // B10 correction: max==0 is float 0/0 -> NaN -> (int)0, NOT an
      // ArithmeticException; the hit clamp makes the bar zero width.
      g.log.clear();
      x.drawstat(g, 0, 0, false, 0.0F);
      Check.ok(true, "draw_drawstat_max0_no_arithmetic_exception");
      Check.eq(g.log.size(), 2, "draw_drawstat_max0_still_paints_two_polygons");
      Check.ok(g.log.get(0).startsWith("fillPoly [422,422,422,422] [11,20,20,11]"),
            "draw_drawstat_max0_zero_width");
      Check.ok(g.log.get(0).endsWith(rgbHex(KX.intv("drawstat_red"), KX.intv("drawstat_green_high"),
            KX.intv("drawstat_blue"))), "draw_drawstat_max0_colour_244_244_11");

      // m.flex==2 tail paints the empty power-bar region with csky
      resetAll();
      m.flex = 2;
      g.log.clear();
      x.drawstat(g, 100, 50, false, 75.0F);
      Check.eq(g.log.size(), 3, "draw_drawstat_flex2_three_polygons");
      Check.ok(g.log.get(2).startsWith("fillPoly [497,497,520,520] [31,39,39,31]"),
            "draw_drawstat_flex2_tail_422_plus_power_to_520");
   }

   private static void overPins() {
      xtGraphics x = FULL.xt;
      BufferedImage img = new BufferedImage(137, 22, BufferedImage.TYPE_INT_RGB);
      Check.ok(x.over(img, 204 + 68, 143 + 11, 204, 143), "draw_over_center_true");
      Check.ok(x.over(img, 204 + 68, 169, 204, 143), "draw_over_bottom_minus1_true");
      Check.ok(!x.over(img, 204 + 68, 170, 204, 143), "draw_over_bottom_edge_170_false");
      Check.ok(!x.over(img, 198, 143 + 11, 204, 143), "draw_over_left_minus6_false");
   }

   /** B10(3) / :744-755 - the three titles, pinned through drawString. */
   private static void levelhighPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      resetAll();
      g.setFont(new Font("SansSerif", 1, 13));
      x.ftm = g.getFontMetrics();
      g.log.clear();
      x.levelhigh(g, 0, 229);
      Check.ok(g.has("str \"Wasted!\" " + csx("Wasted!", 13) + ",60 103060"), "draw_levelhigh_wasted_title"); // mac-font x was 249
      g.log.clear();
      x.levelhigh(g, 1, 0);
      Check.ok(g.has("str \"You wasted 'em!\" " + csx("You wasted 'em!", 13) + ",60 103060"), "draw_levelhigh_you_wasted_em_title"); // mac-font x was 220
      g.log.clear();
      x.levelhigh(g, 0, 0);
      Check.ok(g.has("str \"Stunts!\" " + csx("Stunts!", 13) + ",60 103060"), "draw_levelhigh_stunts_title"); // mac-font x was 252
      Check.ok(g.has("str \"Press  [ Enter ]  to continue\" " + csx("Press  [ Enter ]  to continue", 13) + ",380 000000"), "draw_levelhigh_enter_prompt"); // mac-font x was 184
      Check.ok(g.has("img 197x24 176,20"), "draw_levelhigh_gameh_at_176_20");
   }

   /** B10(3) / :371-430 - the fixed 550x400 PixelGrabber, the in-place
    *  flexpix rewrite and the MemoryImageSource result. */
   private static void fleximagePins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      resetAll();
      BufferedImage src = gradient();
      preset(5, 5, 5); // the TControl recipe: 0.4,0.3,0.2,0.1,0.0,0.9,...
      g.log.clear();
      x.fleximage(src, g, 0);
      Check.ok(x.fleximg != null, "draw_fleximage_fleximg_created");
      Check.eq(x.fleximg.getWidth(null), 550, "draw_fleximage_fleximg_width_550");
      Check.eq(x.fleximg.getHeight(null), 400, "draw_fleximage_fleximg_height_400");
      Check.ok(g.has("img 550x400 0,0"), "draw_fleximage_blits_at_origin");
      // loop 1 rewrites every pixel toward hue 0.7; loop 2 darkens when
      // m.random() > 0.2 (the preset walk: 0.4,0.3 keep, 0.2/0.1/0.0 skip)
      Check.eq(Integer.toHexString(x.flexpix[0]), "ff2e2960", "draw_fleximage_flexpix_0");
      Check.eq(Integer.toHexString(x.flexpix[1]), "ff2e2965", "draw_fleximage_flexpix_1");
      Check.eq(Integer.toHexString(x.flexpix[2]), "ff30296a", "draw_fleximage_flexpix_2_skipped_at_0_2");
      Check.eq(Integer.toHexString(x.flexpix[3]), "ff514095", "draw_fleximage_flexpix_3_skipped_at_0_1");
      Check.eq(Integer.toHexString(x.flexpix[4]), "ff52409c", "draw_fleximage_flexpix_4_skipped_at_0_0");
      Check.eq(Integer.toHexString(x.flexpix[5]), "ff32297a", "draw_fleximage_flexpix_5_darkened_at_0_9");

      // var3 scales the blue subtraction (0.25 + var3/30)
      resetAll();
      preset(5, 5, 5);
      x.fleximage(src, g, 15);
      Check.eq(Integer.toHexString(x.flexpix[0]), "ff1b1a18", "draw_fleximage_var3_15_flexpix_0");
      Check.eq(Integer.toHexString(x.flexpix[5]), "ff1e1a1e", "draw_fleximage_var3_15_flexpix_5");

      // in place: the SAME array is reused and overwritten
      Check.ok(x.flexpix == x.flexpix, "draw_fleximage_flexpix_reused");
   }

   /** B10(3) / :1130-1179 - greying, the band tint and m.flex=0. */
   private static void pauseimagePins() {
      xtGraphics x = FULL.xt;
      BufferedImage src = gradient();
      BufferedImage dst = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      resetAll();
      FULL.m.flex = 9;
      x.pauseimage(src, dst.getGraphics());
      Check.eq(FULL.m.flex, 0, "draw_pauseimage_m_flex_reset_0");
      Check.eq(x.flexpix[0] & 0xffffff, 0x606060, "draw_pauseimage_grey_of_406080");
      // :1159 is strictly greater, so 550*106+156 is still grey and +157 is
      // the first tinted pixel
      Check.eq(x.flexpix[550 * KX.intv("pause_band_row") + KX.intv("pause_band_grey_index")] & 0xffffff,
            0x575757, "draw_pauseimage_index_156_still_grey");
      Check.eq(x.flexpix[550 * KX.intv("pause_band_row") + KX.intv("pause_band_first_tint")] & 0xffffff,
            0x304965, "draw_pauseimage_index_157_first_tinted");
      Check.ok(x.fleximg != null, "draw_pauseimage_fleximg_created");
      Check.eq(x.fleximg.getWidth(null), 550, "draw_pauseimage_fleximg_width_550");
   }

   /** B10(3) / :2897-2927 - the bar width formula with the ctor's 485/514. */
   private static void loadingPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      resetAll();
      g.setFont(new Font("SansSerif", 1, 11));
      x.ftm = g.getFontMetrics();
      g.log.clear();
      x.loading(g, FULL.app);
      Check.ok(g.has("fillRect 162,346,214,10 204080"), "draw_loading_bar_26_plus_188");
      Check.ok(g.has("str \"95 % loaded    |    29 KB remaining\" " + csx("95 % loaded    |    29 KB remaining", 11) + ",385 204080"), "draw_loading_percent_text_95"); // mac-font x was 176
      Check.ok(g.has("fillRect 170,373,210,17 ffffff"), "draw_loading_white_text_backdrop");
      Check.ok(g.has("img 75x66 237,10"), "draw_loading_sign_at_237_10");
      Check.ok(g.has("img 550x217 0,80"), "draw_loading_hello_at_0_80");
      Check.ok(g.has("img 238x21 156,340"), "draw_loading_loadbar_at_156_340");
      Check.ok(g.has("str \"Loading game, please wait.\" " + csx("Loading game, please wait.", 11) + ",333 000000"), "draw_loading_caption"); // mac-font x was 200
   }

   /** B10(3) / :1515-1716 - the full HUD, the m.flex bump and the
    *  changed-field-only repaint. */
   private static void statHudPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      resetAll();
      Check.ok(x.rank[0] != null && x.cntdn[0] != null, "draw_snap_rank_and_countdown_loaded");
      Check.ok(x.gameh != null && x.star[0] != null && x.loadingmusic != null, "draw_snap_hud_images_loaded");

      FULL.m.flex = 0;
      x.posit = 0;
      x.wasted = 0;
      x.laps = 0;
      Madness mad = new Madness(FULL.m, new Record(FULL.m), x, 0);
      CheckPoints cp = new CheckPoints();
      Control c = new Control(FULL.m);
      g.setFont(new Font("SansSerif", 1, 13));
      x.ftm = g.getFontMetrics();
      g.log.clear();
      x.stat(mad, cp, c, true, g);
      Check.eq(FULL.m.flex, 1, "draw_stat_hud_flex_0_to_1");
      Check.ok(g.has("img 176x16 360,7"), "draw_stat_hud_dmg_at_360_7");
      Check.ok(g.has("img 176x16 360,27"), "draw_stat_hud_pwr_at_360_27");
      Check.ok(g.has("img 27x12 19,7"), "draw_stat_hud_lap_at_19_7");
      Check.ok(g.has("str \"1 / 0\" 51,18 000064"), "draw_stat_hud_lap_text");
      Check.ok(g.has("img 53x12 92,7"), "draw_stat_hud_wasted_image");
      Check.ok(g.has("str \"0 / 4\" 150,18 000064"), "draw_stat_hud_wasted_text");
      Check.ok(g.has("img 65x12 42,27"), "draw_stat_hud_pos_at_42_27");
      Check.ok(g.has("img 35x18 110,28"), "draw_stat_hud_rank_at_110_28");
      Check.ok(g.has("fillPoly [422,422,497,497] [31,40,40,31]"), "draw_stat_hud_drawstat_power_bar");

      g.log.clear();
      x.stat(mad, cp, c, true, g);
      Check.eq(FULL.m.flex, 2, "draw_stat_hud_flex_1_to_2");

      // flex==2: only the changed fields are repainted; here pos changed
      // once (posit was 0, cp.pos[im]=4) so the rank is redrawn once, then
      // the next frame draws nothing but drawstat
      g.log.clear();
      x.stat(mad, cp, c, true, g);
      Check.eq(FULL.m.flex, 2, "draw_stat_hud_flex_stays_2");
      Check.ok(g.has("img 35x18 110,28"), "draw_stat_hud_changed_pos_repaints_rank");
      Check.ok(!g.has("img 176x16 360,7"), "draw_stat_hud_incremental_skips_dmg");
      Check.eq(x.posit, 4, "draw_stat_hud_posit_cached_4");
      g.log.clear();
      x.stat(mad, cp, c, true, g);
      Check.ok(!g.has("img 35x18 110,28"), "draw_stat_hud_unchanged_pos_no_repaint");
      Check.ok(g.has("fillPoly [422,422,497,497] [31,40,40,31]"), "draw_stat_hud_drawstat_every_frame");

      // a wasted change repaints that one field with csky + the black text
      cp.wasted = 2;
      g.log.clear();
      x.stat(mad, cp, c, true, g);
      Check.ok(g.has("fillRect 150,8,30,10 d9e0ff"), "draw_stat_hud_wasted_change_fill_csky");
      Check.ok(g.has("str \"2 / 4\" 150,18 000064"), "draw_stat_hud_wasted_change_text");
      Check.eq(x.wasted, 2, "draw_stat_hud_wasted_cached_2");
   }

   /** B10(3) / :432-742 - the angle formula, the normalisation, the
    *  smoothing and both clamps. */
   private static void arrowPins() {
      xtGraphics x = FULL.xt;
      Rec g = FULL.rec;
      resetAll();
      g.setFont(new Font("SansSerif", 1, 13));
      x.ftm = g.getFontMetrics();
      CheckPoints cp = new CheckPoints();
      cp.x[0] = 1000;
      cp.z[0] = 0;
      cp.opx[0] = 0;
      cp.opz[0] = 0;

      x.ana = 0;
      x.arrow(g, 0, 0, cp, false);
      Check.eq(x.ana, -10, "draw_arrow_track_first_step_minus10");
      for (int i = 0; i < 40; i++) {
         x.arrow(g, 0, 0, cp, false);
      }
      Check.eq(x.ana, -90, "draw_arrow_track_converges_minus90");

      // m.xz is added to the raw angle before normalisation
      resetAll();
      FULL.m.xz = 40;
      x.ana = 0;
      x.arrow(g, 0, 0, cp, false);
      Check.eq(x.ana, -10, "draw_arrow_m_xz_40_raw_minus50_step_minus10");
      FULL.m.xz = 0;

      // a steep track target clamps at -130
      resetAll();
      cp.z[0] = -10000;
      x.ana = 0;
      for (int i = 0; i < 40; i++) {
         x.arrow(g, 0, 0, cp, false);
      }
      Check.eq(x.ana, -130, "draw_arrow_track_clamp_minus130");

      // cars branch: nearest on-screen not-dested car, clamp +-100, and the
      // names[sc[..]] banner
      resetAll();
      CheckPoints cp2 = new CheckPoints();
      cp2.opx[0] = 0;
      cp2.opz[0] = 0;
      for (int i = 1; i < 5; i++) {
         cp2.opx[i] = 1000 * i;
         cp2.opz[i] = -10000;
         cp2.onscreen[i] = 1;
      }
      x.ana = 0;
      g.log.clear();
      for (int i = 0; i < 40; i++) {
         x.arrow(g, 1, 0, cp2, true);
      }
      Check.eq(x.ana, -100, "draw_arrow_cars_clamp_minus100");
      Check.ok(g.has("Tornado Shark"), "draw_arrow_cars_banner_name");

      // a wasted (dested) car is skipped by the nearest-car scan: only car 2
      // is eligible, so the banner is its name and the angle is its vector
      resetAll();
      CheckPoints cp3 = new CheckPoints();
      cp3.opx[0] = 0;
      cp3.opz[0] = 0;
      cp3.opx[1] = 100;
      cp3.opz[1] = 0;
      cp3.dested[1] = 1;
      cp3.opx[2] = 100;
      cp3.opz[2] = 0;
      cp3.onscreen[2] = 1;
      cp3.dested[2] = 0;
      cp3.opx[3] = 9000;
      cp3.opz[3] = 9000;
      cp3.dested[3] = 1;
      cp3.opx[4] = 9000;
      cp3.opz[4] = 9000;
      cp3.dested[4] = 1;
      x.sc[2] = 3;
      x.ana = 0;
      g.log.clear();
      x.arrow(g, 3, 0, cp3, true);
      Check.ok(g.has("La vite Crab"), "draw_arrow_cars_skips_dested_nearest");
   }

   /** The other OPEN item: drawImage(null,...) on this JDK never throws, so
    *  every still-null image field is a silent no-op (:3411 for rpro). */
   private static void drawImageNullPins() {
      BufferedImage img = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      Graphics g = img.getGraphics();
      Check.eq(g.drawImage((Image) null, 0, 0, null), true, "draw_drawimage_null_returns_true");
      Check.eq(g.drawImage((Image) null, 0, 0, 10, 10, null), true, "draw_drawimage_null_4arg_returns_true");

      xtGraphics x = FULL.xt;
      resetAll();
      Image saved = x.rpro;
      x.rpro = null;
      Rec r = FULL.rec;
      r.setFont(new Font("SansSerif", 1, 13));
      x.ftm = r.getFontMetrics();
      r.log.clear();
      x.rad(r, 1);
      Check.ok(r.has("img null 150,240"), "draw_rad_null_rpro_silent_blit");
      Check.ok(r.has(" 87,110") && !r.has("img null 87,110"), "draw_rad_radicalplay_still_drawn");
      Check.ok(r.has("str \"www.radicalplay.com\""), "draw_rad_still_draws_text");
      x.rpro = saved;
   }

   // =====================================================================
   // state reset
   // =====================================================================

   /** Restores every field the groups above mutate back to the ctor state
    *  (the ctor's own list) plus the Medium fields draw/stat mutate. Fields
    *  NOT reset here (and asserted for it): the image/clip tables, names/
    *  hipno/sndsize and the RecMod seams installed once. */
   static void resetAll() {
      xtGraphics x = FULL.xt;
      Medium m = FULL.m;
      x.fase = 111;
      x.oldfase = 0;
      x.starcnt = 0;
      x.unlocked = 1;
      x.lockcnt = 0;
      x.opselect = 1;
      x.shaded = false;
      x.flipo = 0;
      x.nextc = false;
      x.gatey = 0;
      x.looped = 1;
      Arrays.fill(x.sc, 0);
      x.holdcnt = 0;
      x.winner = false;
      x.pcontin = 0;
      x.pnext = 0;
      x.pback = 0;
      x.pstar = 0;
      x.gocnt = 0;
      x.pwastd = false;
      x.mutes = false;
      x.mutem = false;
      x.arrace = false;
      x.ana = 0;
      x.cntan = 0;
      x.cntovn = 0;
      x.flk = false;
      x.tcnt = 30;
      x.tflk = false;
      x.wasay = false;
      x.say = "";
      x.clear = 0;
      x.posit = 0;
      x.wasted = 0;
      x.laps = 0;
      Arrays.fill(x.dested, 0);
      x.dmcnt = 0;
      x.dmflk = false;
      x.pwcnt = 0;
      x.pwflk = false;
      x.auscnt = 45;
      x.aflk = false;
      x.pin = 60;
      x.pwait = 7;
      x.stopcnt = 0;
      x.cntwis = 0;
      x.crshturn = 0;
      x.bfcrash = 0;
      x.bfskid = 0;
      x.crashup = false;
      x.skflg = 0;
      x.dskflg = 0;
      x.grrd = false;
      x.aird = false;
      x.loop = "";
      x.spin = "";
      x.asay = "";
      Arrays.fill(x.loadedt, false);
      Arrays.fill(x.stracks, null);
      Arrays.fill(x.pengs, false);
      m.snap[0] = 0;
      m.snap[1] = 0;
      m.snap[2] = 0;
      m.flex = 0;
      m.crs = false;
      m.trk = false;
      m.x = 0;
      m.y = 0;
      m.z = 0;
      m.xz = 0;
      m.zy = 0;
      m.ground = 250;
      m.cx = 275;
      m.cy = 200;
      m.focus_point = 400;
      AUDIO.clear();
      MODLOG.clear();
      FULL.rec.log.clear();
   }

   /** A fresh copy of the ten car models (finish/carselect mutate x/y/z/xz/
    *  zy/wzy in place). */
   static ContO[] freshCars() {
      ContO[] out = new ContO[10];
      for (int i = 0; i < 10; i++) {
         out[i] = new ContO(CARS[i], 0, 0, 0, 0);
      }
      return out;
   }

   static ContO[] CARS;

   /** Installs a named recording Clip into every audio slot. */
   static void installAudio(xtGraphics x) {
      for (int a = 0; a < 2; a++) {
         for (int b2 = 0; b2 < 5; b2++) {
            x.engs[a][b2] = new Clip("eng" + a + b2, AUDIO);
         }
      }
      for (int i = 0; i < 6; i++) {
         x.air[i] = new Clip("air" + i, AUDIO);
      }
      for (int i = 0; i < 3; i++) {
         x.crash[i] = new Clip("crash" + (i + 1), AUDIO);
         x.lowcrash[i] = new Clip("lowcrash" + (i + 1), AUDIO);
      }
      for (int i = 0; i < 2; i++) {
         x.skid[i] = new Clip("skid" + (i + 1), AUDIO);
         x.dustskid[i] = new Clip("dustskid" + (i + 1), AUDIO);
      }
      x.tires = new Clip("tires", AUDIO);
      x.checkpoint = new Clip("checkpoint", AUDIO);
      x.carfixed = new Clip("carfixed", AUDIO);
      x.powerup = new Clip("powerup", AUDIO);
      x.three = new Clip("three", AUDIO);
      x.two = new Clip("two", AUDIO);
      x.one = new Clip("one", AUDIO);
      x.go = new Clip("go", AUDIO);
      x.wastd = new Clip("wastd", AUDIO);
      x.firewasted = new Clip("firewasted", AUDIO);
      for (int i = 0; i < 5; i++) {
         x.pengs[i] = false;
      }
   }

   /** The TControl recipe with a chosen rand triple: cntrn=100 (no refill),
    *  trn=0 (first return reads rand[1]) and diup all false (every entry
    *  decrements). preset(9,9,9) yields 0.8, 0.7, 0.6, 0.5, ... and
    *  preset(9,9,5) yields 0.8, 0.3, 0.6, 0.5, ... - the explicit form is
    *  used wherever a later draw must land on a different index. */
   static void preset(int r0, int r1, int r2) {
      Medium m = FULL.m;
      m.cntrn = 100;
      m.rand[0] = r0;
      m.rand[1] = r1;
      m.rand[2] = r2;
      m.diup[0] = false;
      m.diup[1] = false;
      m.diup[2] = false;
      m.trn = 0;
   }

   static void preset11() {
      preset(9, 9, 9);
   }

   static BufferedImage gradient() {
      BufferedImage src = new BufferedImage(550, 400, BufferedImage.TYPE_INT_RGB);
      for (int y = 0; y < 400; y++) {
         for (int x = 0; x < 550; x++) {
            src.setRGB(x, y, 0x406080 + (x * 7 + y));
         }
      }
      return src;
   }

   static int count(List<String> list, String needle) {
      int n = 0;
      for (int i = 0; i < list.size(); i++) {
         if (list.get(i).indexOf(needle) >= 0) {
            n++;
         }
      }
      return n;
   }

   static boolean allPrefix(List<String> list, String prefix) {
      for (int i = 0; i < list.size(); i++) {
         if (!list.get(i).startsWith(prefix)) {
            return false;
         }
      }
      return true;
   }

   static boolean distinctNonZero(int[] a, int from, int to) {
      for (int i = from; i <= to; i++) {
         for (int j = i + 1; j <= to; j++) {
            if (a[i] == a[j]) {
               return false;
            }
         }
      }
      return true;
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

   static String fixtureRoot(String name) {
      return "target/java-tests-fixtures/" + name;
   }

   /** assets/graphics + assets/music + the three welcome gifs at the ROOT
    *  (the exact paths the ctor asks for: hello.gif, sign.gif, loadbar.gif
    *  against getCodeBase()). */
   static String fixtureFull() throws IOException {
      File root = new File(fixtureRoot("xtfull"));
      File marker = new File(root, "graphics/images4.zipo");
      if (!marker.isFile()) {
         deleteTree(root);
         copyTree(new File("assets/graphics"), new File(root, "graphics"));
         copyTree(new File("assets/music"), new File(root, "music"));
         copyTree(new File("assets/graphics/hello.gif"), new File(root, "hello.gif"));
         copyTree(new File("assets/graphics/sign.gif"), new File(root, "sign.gif"));
         copyTree(new File("assets/graphics/loadbar.gif"), new File(root, "loadbar.gif"));
      }
      return root.getPath();
   }

   /** Only the two paks the ctor's pressed() calls need; no welcome gifs, no
    *  music (the loadpak1/2 swallow arm + the default sound dir + the
    *  shipped-tree welcome-image shape). */
   static String fixtureSparse() throws IOException {
      File root = new File(fixtureRoot("xtsparse"));
      File marker = new File(root, "graphics/images4.zipo");
      if (!marker.isFile()) {
         deleteTree(root);
         new File(root, "graphics").mkdirs();
         copyTree(new File("assets/graphics/images3.zipo"), new File(root, "graphics/images3.zipo"));
         copyTree(new File("assets/graphics/images4.zipo"), new File(root, "graphics/images4.zipo"));
      }
      return root.getPath();
   }

   static String fixtureNo3() throws IOException {
      File root = new File(fixtureRoot("xtno3"));
      File marker = new File(root, "graphics/images4.zipo");
      if (!marker.isFile()) {
         deleteTree(root);
         copyTree(new File(fixtureFull()), root);
         new File(root, "graphics/images3.zipo").delete();
      }
      return root.getPath();
   }

   static String fixtureNo4() throws IOException {
      File root = new File(fixtureRoot("xtno4"));
      File marker = new File(root, "graphics/images3.zipo");
      if (!marker.isFile()) {
         deleteTree(root);
         copyTree(new File(fixtureFull()), root);
         new File(root, "graphics/images4.zipo").delete();
      }
      return root.getPath();
   }

   static ContO[] loadCars() throws Exception {
      final Medium mm = new Medium();
      final Trackers tr = new Trackers();
      ContO[] out = new ContO[10];
      java.util.zip.ZipInputStream z = new java.util.zip.ZipInputStream(
            new FileInputStream("assets/graphics/models.zipo"));
      String[] names = {"2000tornados", "formula7", "canyenaro", "lescrab", "nimi", "maxrevenge", "leadoxide", "king",
            "radicalone", "drmonster"};
      try {
         for (java.util.zip.ZipEntry e = z.getNextEntry(); e != null; e = z.getNextEntry()) {
            byte[] payload = new byte[(int) e.getSize()];
            int off = 0;
            while (off < payload.length) {
               int r = z.read(payload, off, payload.length - off);
               if (r < 0) {
                  break;
               }
               off += r;
            }
            for (int i = 0; i < 10; i++) {
               if (e.getName().startsWith(names[i])) {
                  out[i] = new ContO(payload, mm, tr);
               }
            }
         }
      } finally {
         z.close();
      }
      return out;
   }

   // =====================================================================
   // stdout capture (the loaders only PRINT)
   // =====================================================================

   static PrintStream realOut;
   static ByteArrayOutputStream outBuf;
   static String ctorOut;

   static void captureStart() {
      outBuf = new ByteArrayOutputStream();
      realOut = System.out;
      System.setOut(new PrintStream(outBuf, true));
   }

   static String captureEnd() {
      System.setOut(realOut);
      return outBuf.toString();
   }

   // =====================================================================
   // file helpers
   // =====================================================================

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
}
