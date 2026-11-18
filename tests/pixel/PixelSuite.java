import java.applet.Applet;
import java.applet.AppletContext;
import java.applet.AudioClip;
import java.awt.Color;
import java.awt.Font;
import java.awt.FontMetrics;
import java.awt.Graphics;
import java.awt.Graphics2D;
import java.awt.Image;
import java.awt.Toolkit;
import java.awt.image.BufferedImage;
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
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Enumeration;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;

/**
 * The pixel-test platform: pins the ORIGINAL xtGraphics screens and the text
 * rasterizer at the pixel level, on a REPRODUCIBLE (not era-exact) platform.
 *
 * Contract (user-decided, see notes/JAVA-BEHAVIOR/B12_pixel.md):
 *  - The suite runs ONLY in the podman container built from
 *    tools/pixel-tests.Dockerfile (Temurin 8 + Xvfb + fontconfig), with
 *    /w read-only. Every pixel assertion is an md5 over a full 550x400
 *    BufferedImage TYPE_INT_RGB frame - no float comparisons, no window.
 *  - The logical font "SansSerif" (bold) is mapped by the committed
 *    tools/fonts/fontconfig.properties to the shipped
 *    tools/fonts/LiberationSans-Bold.ttf. Pin #1 proves the mapping by
 *    raster equality against Font.createFont(...) of the same file.
 *  - tests/fixtures/xtfont-atlas/ is the SHARED ground truth the Rust port
 *    consumes (atlas.png grid + atlas.txt manifest, format documented in the
 *    manifest header).
 *  - tests/pixel/expected.md5 is the committed baseline; --bless rewrites it,
 *    the default mode verifies against it.
 *
 * Modes:
 *   (none)   verify: every pin is compared against tests/pixel/expected.md5
 *   --bless  run every pin, write the baseline to tests/pixel/expected.md5
 *   --gen    regenerate tests/fixtures/xtfont-atlas/ (atlas.png + atlas.txt)
 *
 * Driver policy: no GameSparker run() pump, no threads, no TestRng - for the
 * HANDLER-LEVEL screens. Each screen is driven directly with the state
 * TxtGraphicsLogic established, and randomness is the explicit Medium walk
 * (rand={5,5,5}, diup=false, trn=0 -> 0.4,0.3,...; cntrn is a large
 * deterministic budget so no draw can fall back to Math.random).
 *
 * The RACE frames cannot be driven that way (their graph is built inside
 * run()), so the policy is AMENDED, not dropped: main() installs the one
 * seeded Math.random() injection as its first statement, and PixelRace owns a
 * bounded, fully torn-down run() pump plus its own applet seam. See
 * PixelRace's class comment and notes/JAVA-BEHAVIOR/B12_pixel.md §8.
 */
public class PixelSuite {
   // =====================================================================
   // the pixel manifest (tests/fixtures/toml/pixel.toml)
   // =====================================================================

   /** The ONE pixel.toml load (the README's ownership row: pixel.toml owns
    *  the atlas geometry + format + sha256s, the font-probe md5s and the
    *  [frames] reference; the frame-pin ROWS stay in expected.md5). Loaded at
    *  class init, i.e. before main()'s seed statement - Toml is harness code
    *  and consumes no Math.random, so the injected generator C2 folds is
    *  unchanged. PixelRace has no pixel-platform constant of its own and
    *  therefore no load of this file. */
   static final Toml PIXEL = Toml.load("tests/fixtures/toml/pixel.toml");
   static final Toml ATLAS = PIXEL.table("atlas");
   static final Toml PROBES = PIXEL.table("probes");
   static final Toml FRAMES = PIXEL.table("frames");
   static final Toml FRAME = PIXEL.table("frame");

   static {
      // shape assertion at load: a malformed manifest fails here, loudly,
      // before any pin runs (Toml names the file and the missing key path)
      ATLAS.requireKeys("columns", "rows", "cell_w", "cell_h", "glyph_dx", "baseline",
            "codepoint_min", "codepoint_max", "sizes", "format", "ttf_path", "sha256_ttf",
            "sha256_png", "sha256_manifest");
      PROBES.requireKeys("font_probe_13_md5", "font_probe_11_md5");
      FRAMES.requireKeys("file", "row_count");
   }

   /** Verify-mode cross-check: a pixel.toml value must equal what this run
    *  produced (the blessed copy of the same value is pinned separately in
    *  expected.md5). Skipped under --bless/--gen: those are the intentional
    *  platform-change flows (B12 §2) and must stay runnable while the manifest
    *  is being updated; verify is where agreement is enforced. */
   static void manifestCheck(String got, String manifest, String id) {
      if (!BLESS) {
         Check.eq(got, manifest, id);
      }
   }

   // =====================================================================
   // entry
   // =====================================================================

   public static void main(String[] args) throws Throwable {
      // FIRST: the seed lookup. The ONE Math.random() injection must precede
      // any game class load in this cold-JIT process (rationale: PixelRace's
      // class comment); the value itself is fixture data
      // (determinism.toml [seed] pixel_main). Toml is harness code and
      // consumes no Math.random, so the slot C2 folds is still the injected
      // generator - the read only adds class loads, never a draw.
      int seed = Toml.load("tests/fixtures/toml/determinism.toml").table("seed").intv("pixel_main");
      boolean gen = has(args, "--gen");
      BLESS = has(args, "--bless");

      // Race-frame determinism: installed as the very first game-relevant
      // statement (rationale: PixelRace's class comment). The handler-level
      // screens below never reach Math.random() (their Medium walk has a
      // 1,000,000-draw budget), so this cannot move any pre-existing pin.
      TestRng.install(seed);

      if (gen) {
         generateAtlas();
         return;
      }

      Check.group("Pixel");
      if (!BLESS) {
         loadExpected();
      }
      fontProbe();
      atlasVerify();
      screens();
      PixelRace.pins();
      if (BLESS) {
         writeExpected();
      } else {
         // a renamed/dropped pin must not silently pass: every row of the
         // committed baseline has to be produced by this run.
         Check.eq(PINNED, EXPECTED.size(), "expected_md5_row_count_matches_pins");
         // the manifest's [frames] row_count must agree with the rows the
         // loader actually read (the two-file contract: the data lives in
         // expected.md5, pixel.toml only references it)
         Check.eq(FRAMES.intv("row_count"), EXPECTED.size(), "frames_row_count_matches_expected_file");
         // the [frame] canvas geometry must be the one every md5 is rendered at
         Check.eq(FRAME.intv("w") + "x" + FRAME.intv("h"), W + "x" + H,
               "frame_geometry_matches_canvas");
      }
      int rc = Check.report();
      if (rc != 0) {
         System.exit(rc);
      }
   }

   static boolean has(String[] a, String want) {
      for (int i = 0; i < a.length; i++) {
         if (want.equals(a[i])) {
            return true;
         }
      }
      return false;
   }

   // =====================================================================
   // pin plumbing (expected.md5)
   // =====================================================================

   /** Verify mode rows, id -> canonical value (md5 / sha256 / count). */
   static final Map<String, String> EXPECTED = new LinkedHashMap<String, String>();
   /** Bless mode rows, in emission order. */
   static final Map<String, String> ACTUAL = new LinkedHashMap<String, String>();
   static boolean BLESS = false;
   static int PINNED = 0;

   /** One pin: an md5 hex of a frame, or a canonical scalar (count/hash). */
   static void pin(String id, String got) {
      PINNED++;
      if (BLESS) {
         ACTUAL.put(id, got);
         return;
      }
      String want = EXPECTED.get(id);
      if (want == null) {
         Check.ok(false, id + "_missing_from_expected_md5");
      } else {
         Check.eq(got, want, id);
      }
   }

   static String expectedPath(boolean write) {
      // the default IS the manifest's [frames] file; --bless/--verify keep
      // their -Dexpected.out / -Dexpected.file overrides unchanged
      return System.getProperty(write ? "expected.out" : "expected.file", FRAMES.str("file"));
   }

   static void loadExpected() throws IOException {
      File f = new File(expectedPath(false));
      if (!f.isFile()) {
         Check.ok(false, "expected_md5_file_missing");
         return;
      }
      List<String> lines = readLines(f);
      for (int i = 0; i < lines.size(); i++) {
         String line = lines.get(i).trim();
         if (line.length() == 0 || line.startsWith("#")) {
            continue;
         }
         int sp = line.indexOf(' ');
         if (sp < 0) {
            continue;
         }
         EXPECTED.put(line.substring(0, sp), line.substring(sp + 1).trim());
      }
   }

   static void writeExpected() throws IOException {
      File f = new File(expectedPath(true));
      f.getParentFile().mkdirs();
      StringBuilder b = new StringBuilder();
      b.append("# Pixel-suite pin baseline - the committed artifact is the pin.\n");
      b.append("#\n");
      b.append("# Regenerate (ONLY after an intentional platform change) with:\n");
      b.append("#   tools/pixel-tests.sh --bless\n");
      b.append("# then review the diff: a screen id whose md5 moved is a visual\n");
      b.append("# behavior change in the ORIGINAL, not a cosmetic edit.\n");
      b.append("#\n");
      b.append("# Format: <stable_snake_case_id> <md5-or-scalar>\n");
      b.append("#   screen_*  md5 of the full 550x400 TYPE_INT_RGB ARGB pixel array\n");
      b.append("#   font_*    md5 of the named probe frame (same 550x400 rule)\n");
      b.append("#   atlas_*   canonical scalar: sha256 hex, or a decimal count\n");
      for (Map.Entry<String, String> e : ACTUAL.entrySet()) {
         b.append(e.getKey()).append(' ').append(e.getValue()).append('\n');
      }
      writeFile(f, b.toString());
      System.out.println("blessed " + ACTUAL.size() + " pins -> " + f.getPath());
   }

   // =====================================================================
   // the platform seam (device-free, TxtGraphicsLogic-shaped)
   // =====================================================================

   /** The silent clip (standing policy: no clip ever reaches a device). */
   static final AudioClip SILENT = new AudioClip() {
      public void play() {
      }

      public void loop() {
      }

      public void stop() {
      }
   };

   /** A recording RadicalMod: the ctor's stages/cars are replaced with these
    *  before any screen runs, so carselect's cars.play() and stageselect's
    *  stages.play()/stop() append to MODLOG instead of opening sun.audio. */
   static class RecMod extends RadicalMod {
      final String tag;

      RecMod(String tag, App app) {
         super(null, 0, 0, 0, app);
         this.tag = tag;
      }

      public void play() {
         MODLOG.add(tag + ".play");
      }

      public void resume() {
         MODLOG.add(tag + ".resume");
      }

      public void stop() {
         MODLOG.add(tag + ".stop");
      }

      protected void outwithit() {
         MODLOG.add(tag + ".outwithit");
      }

      public int posit() {
         MODLOG.add(tag + ".posit");
         return 0;
      }
   }

   static final List<String> MODLOG = new ArrayList<String>();

   /** The applet seam WITHOUT any run()/repaint pump. getCodeBase points at
    *  the fixture dir; getImage mirrors a browser; getAudioClip returns the
    *  shared silent stub; repaint is a no-op (xtGraphics.loading() calls it). */
   static class App extends Applet {
      final URL base;
      final AppContext ctx = new AppContext();

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
         return SILENT;
      }

      public AudioClip getAudioClip(URL u) {
         return SILENT;
      }

      public void repaint() {
      }
   }

   /** The plugin viewer's "sun." toString (sunytyp==2), kept for parity with
    *  the gate's seam even though no pixel path reads it. */
   static class AppContext implements AppletContext {
      public String toString() {
         return "sun.applet.AppletViewer@pixel";
      }

      public AudioClip getAudioClip(URL u) {
         return SILENT;
      }

      public Image getImage(URL u) {
         return null;
      }

      public Applet getApplet(String n) {
         return null;
      }

      public Enumeration<Applet> getApplets() {
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

   // =====================================================================
   // the one-constructor fixture
   // =====================================================================

   static xtGraphics X;
   static Medium M;
   static App APP;
   static Control CTL;
   static ContO[] CARS;

   /** The graphics-only fixture (assets/graphics 240K + the three welcome
    *  gifs at the codebase ROOT, which is where xtGraphics.java:2935/2943/
    *  2951 asks for them). music/ is deliberately absent: RadicalMod's ctor
    *  swallows the missing file (RadicalMod.java:41-47), the two music
    *  objects are replaced by RecMod right after construction, and the
    *  ctor's dnload arithmetic is independent of the file bytes. */
   static String fixture() throws IOException {
      String rootProp = System.getProperty("pixel.fixtures.dir", "/tmp/nfm-pixel-fixtures");
      File root = new File(rootProp, "pixelfull");
      deleteTree(root);
      copyTree(new File("assets/graphics"), new File(root, "graphics"));
      copyTree(new File("assets/graphics/hello.gif"), new File(root, "hello.gif"));
      copyTree(new File("assets/graphics/sign.gif"), new File(root, "sign.gif"));
      copyTree(new File("assets/graphics/loadbar.gif"), new File(root, "loadbar.gif"));
      return root.getPath();
   }

   /** Constructs the ONE xtGraphics (var4==2 -> the newsun sound dir and the
    *  ctor's 485/514 progress pair). The ctor's prints are captured: with no
    *  music the two RadicalMods legitimately print "Error Loading Mod: ...". */
   static void setup() throws Exception {
      String dir = fixture();
      APP = new App(dirUrl(dir));
      M = new Medium();
      APP_CANVAS = fresh();
      PrintStream real = System.out;
      ByteArrayOutputStream buf = new ByteArrayOutputStream();
      System.setOut(new PrintStream(buf, true));
      try {
         X = new xtGraphics(M, APP_CANVAS.getGraphics(), APP, 2);
         // the RecMod super ctors also probe their null music path and print
         X.stages = new RecMod("stages", APP);
         X.cars = new RecMod("cars", APP);
      } finally {
         System.setOut(real);
      }
      CTL = new Control(M);
      CARS = loadCars();
      // snap(1) is the mandatory predecessor for loadingmusic/star
      // (xtGraphics.java:2564-2566); ctor does not call it.
      X.snap(1);
   }

   static BufferedImage APP_CANVAS;

   static final int W = FRAME.intv("w");
   static final int H = FRAME.intv("h");

   static BufferedImage fresh() {
      return new BufferedImage(W, H, BufferedImage.TYPE_INT_RGB);
   }

   // =====================================================================
   // (a) the fontconfig raster-equality probe - pin #1
   // =====================================================================

   // the shipped face and its digest: pixel.toml [atlas] ttf_path / sha256_ttf
   static final String TTF = ATLAS.str("ttf_path");
   static final String TTF_SHA256 = ATLAS.str("sha256_ttf");

   /** Renders `s` black-on-white at (10,40) into a 550x400 INT_RGB frame. */
   static BufferedImage probeFrame(Font f, String s) {
      BufferedImage img = fresh();
      Graphics2D g = img.createGraphics();
      g.setColor(Color.WHITE);
      g.fillRect(0, 0, W, H);
      g.setColor(Color.BLACK);
      g.setFont(f);
      g.drawString(s, 10, 40);
      g.dispose();
      return img;
   }

   /** Prove the mapping, not just the presence: the logical SansSerif bold
    *  must rasterize EXACTLY like a physical Font.createFont() of the shipped
    *  TTF at the same size/style. Both sizes the original asks for are
    *  probed, because screens use both (xtGraphics.java:1109,3398,3570,...). */
   static void fontProbe() throws Exception {
      String sample = "The Introductory Stage";
      Font physical13 = Font.createFont(Font.TRUETYPE_FONT, new File(TTF)).deriveFont(13.0F).deriveFont(Font.BOLD);
      Font logical13 = new Font("SansSerif", Font.BOLD, 13);
      BufferedImage l13 = probeFrame(logical13, sample);
      BufferedImage p13 = probeFrame(physical13, sample);
      Check.ok(pixelEqual(l13, p13), "font_sansserif_bold_13_raster_equals_shipped_ttf");
      String probe13 = md5(l13);
      manifestCheck(probe13, PROBES.str("font_probe_13_md5"), "font_probe_13_md5_matches_manifest");
      pin("font_probe_13_md5", probe13);

      Font physical11 = Font.createFont(Font.TRUETYPE_FONT, new File(TTF)).deriveFont(11.0F).deriveFont(Font.BOLD);
      Font logical11 = new Font("SansSerif", Font.BOLD, 11);
      BufferedImage l11 = probeFrame(logical11, sample);
      BufferedImage p11 = probeFrame(physical11, sample);
      Check.ok(pixelEqual(l11, p11), "font_sansserif_bold_11_raster_equals_shipped_ttf");
      String probe11 = md5(l11);
      manifestCheck(probe11, PROBES.str("font_probe_11_md5"), "font_probe_11_md5_matches_manifest");
      pin("font_probe_11_md5", probe11);

      // the physical font's identity must be the shipped family, not a
      // fallback that happens to raster the same
      Check.eq(logical13.getFontName(), "SansSerif.bold", "font_logical13_name_composite");
      Check.eq(physical13.getFontName(), "Liberation Sans Bold", "font_physical13_name");
      Check.eq(physical13.getFamily(), "Liberation Sans", "font_physical13_family");

      // line metrics equality is a second, coarser proof of the mapping
      Metrics lm13 = metrics(logical13);
      Metrics pm13 = metrics(physical13);
      Check.eq(lm13.toString(), pm13.toString(), "font_metrics_13_logical_equals_physical");
      Metrics lm11 = metrics(logical11);
      Metrics pm11 = metrics(physical11);
      Check.eq(lm11.toString(), pm11.toString(), "font_metrics_11_logical_equals_physical");
      // the concrete numbers the atlas records (13pt/11pt bold)
      Check.eq(lm13.toString(), "ascent=12 descent=3 leading=1 maxAdvance=17 height=16",
            "font_metrics_13_values");
      Check.eq(lm11.toString(), "ascent=10 descent=3 leading=0 maxAdvance=15 height=13",
            "font_metrics_11_values");

      Check.eq(sha256(new File(TTF)), TTF_SHA256, "font_source_ttf_sha256");
   }

   static class Metrics {
      final int ascent, descent, leading, maxAdvance, height;

      Metrics(int a, int d, int l, int m, int h) {
         ascent = a;
         descent = d;
         leading = l;
         maxAdvance = m;
         height = h;
      }

      public String toString() {
         return "ascent=" + ascent + " descent=" + descent + " leading=" + leading + " maxAdvance=" + maxAdvance
               + " height=" + height;
      }
   }

   static Metrics metrics(Font f) {
      BufferedImage img = new BufferedImage(1, 1, BufferedImage.TYPE_INT_RGB);
      Graphics2D g = img.createGraphics();
      FontMetrics fm = g.getFontMetrics(f);
      Metrics m = new Metrics(fm.getAscent(), fm.getDescent(), fm.getLeading(), fm.getMaxAdvance(), fm.getHeight());
      g.dispose();
      return m;
   }

   // =====================================================================
   // (b) the glyph atlas - shared ground truth for the Rust port
   // =====================================================================

   // Grid geometry, from pixel.toml [atlas]. Every cell is CELL_W x CELL_H,
   // glyph ink is drawn with drawString(cellX + GLYPH_DX, cellY + BASELINE);
   // the atlas.txt header repeats these numbers so the port can parse the PNG
   // without Java.
   static final int COLUMNS = ATLAS.intv("columns");   // 95
   static final int ROWS = ATLAS.intv("rows");         // 2
   static final int CELL_W = ATLAS.intv("cell_w");     // 20
   static final int CELL_H = ATLAS.intv("cell_h");     // 22
   static final int GLYPH_DX = ATLAS.intv("glyph_dx"); // 1
   static final int BASELINE = ATLAS.intv("baseline"); // 16
   static final int CP_MIN = ATLAS.intv("codepoint_min"); // 0x20
   static final int CP_MAX = ATLAS.intv("codepoint_max"); // 0x7e
   static final int GLYPHS = COLUMNS; // 95
   static final int[] SIZES = ATLAS.intArr("sizes"); // row 0 = 13pt bold, row 1 = 11pt bold

   static File atlasDir() {
      return new File(System.getProperty("atlas.dir", "tests/fixtures/xtfont-atlas"));
   }

   static Font atlasFont(int row) {
      return new Font("SansSerif", Font.BOLD, SIZES[row]);
   }

   /** --gen: render the committed atlas. Only run when the platform (font
    *  bytes + JDK rasterizer) changes intentionally; the manifest is then
    *  pinned by atlas_manifest_sha256 in expected.md5. */
   static void generateAtlas() throws Exception {
      File dir = atlasDir();
      dir.mkdirs();
      BufferedImage img = new BufferedImage(GLYPHS * CELL_W, ROWS * CELL_H, BufferedImage.TYPE_INT_RGB);
      Graphics2D g = img.createGraphics();
      g.setColor(Color.WHITE);
      g.fillRect(0, 0, img.getWidth(), img.getHeight());
      g.setColor(Color.BLACK);

      Metrics[] metrics = new Metrics[SIZES.length];
      StringBuilder rows = new StringBuilder();
      int[][] advances = new int[SIZES.length][GLYPHS];
      for (int row = 0; row < SIZES.length; row++) {
         Font f = atlasFont(row);
         g.setFont(f);
         FontMetrics fm = g.getFontMetrics(f);
         metrics[row] = new Metrics(fm.getAscent(), fm.getDescent(), fm.getLeading(), fm.getMaxAdvance(),
               fm.getHeight());
         for (int cp = CP_MIN; cp <= CP_MAX; cp++) {
            int col = cp - CP_MIN;
            int cx = col * CELL_W;
            int cy = row * CELL_H;
            g.drawString(new String(new char[]{(char) cp}), cx + GLYPH_DX, cy + BASELINE);
            advances[row][col] = fm.charWidth(cp);
            rows.append("glyph ").append(SIZES[row]).append(' ').append(cp).append(' ')
                  .append(advances[row][col]).append(' ').append(GLYPH_DX).append('\n');
         }
      }

      // no ink may touch a cell border: a glyph that overflows would bleed
      // into its neighbour and the port could not cut cells safely.
      int bleed = 0;
      for (int y = 0; y < img.getHeight(); y++) {
         for (int x = 0; x < img.getWidth(); x++) {
            boolean border = (x % CELL_W == 0) || (x % CELL_W == CELL_W - 1) || (y % CELL_H == 0)
                  || (y % CELL_H == CELL_H - 1);
            if (border && (img.getRGB(x, y) & 0xffffff) != 0xffffff) {
               bleed++;
            }
         }
      }
      if (bleed != 0) {
         throw new IllegalStateException("atlas glyph bleed: " + bleed + " non-white border pixels; enlarge CELL_W/CELL_H");
      }
      g.dispose();

      File png = new File(dir, "atlas.png");
      ImageIO.write(img, "png", png);

      StringBuilder b = new StringBuilder();
      b.append("# xtfont-atlas - the shared text ground truth of the pixel platform.\n");
      b.append("#\n");
      b.append("# WHY: the original asks only for new Font(\"SansSerif\", 1, 13) and\n");
      b.append("# new Font(\"SansSerif\", 1, 11) (every site in xtGraphics.java). The\n");
      b.append("# container maps logical SansSerif bold onto the shipped\n");
      b.append("# tools/fonts/LiberationSans-Bold.ttf via\n");
      b.append("# tools/fonts/fontconfig.properties, so THIS raster is reproducible\n");
      b.append("# inside tools/pixel-tests.Dockerfile and nowhere else guaranteed.\n");
      b.append("# Era-exactness is approximate (2005 JRE platform fonts are gone);\n");
      b.append("# reproducibility is exact and is what the pin protects.\n");
      b.append("#\n");
      b.append("# FORMAT ").append(ATLAS.str("format")).append(" (plain ASCII, parse without Java):\n");
      b.append("#   - '#' lines are comments.\n");
      b.append("#   - 'key=value' lines below are the manifest; unknown keys are\n");
      b.append("#     ignorable additions.\n");
      b.append("#   - 'glyph <pt> <codepoint_dec> <advance_px> <cell_dx_px>' rows:\n");
      b.append("#     one per codepoint per size. The grid cell is\n");
      b.append("#     col = codepoint - 32, row = 0 for 13pt, 1 for 11pt;\n");
      b.append("#     its pixel origin is (grid_x + col*cell_width,\n");
      b.append("#     grid_y + row*cell_height). The glyph was drawn at\n");
      b.append("#     (cell_x + cell_dx, cell_y + baseline) with drawString.\n");
      b.append("#   - advance is the REAL java.awt.FontMetrics.charWidth(codepoint),\n");
      b.append("#     i.e. the integer pen advance used by stringWidth.\n");
      b.append("#   - pixels are black glyphs on an opaque white TYPE_INT_RGB\n");
      b.append("#     canvas; text antialiasing is the toolkit default.\n");
      b.append("format=").append(ATLAS.str("format")).append('\n');
      b.append("png=atlas.png\n");
      b.append("png_width=").append(img.getWidth()).append('\n');
      b.append("png_height=").append(img.getHeight()).append('\n');
      b.append("png_type=INT_RGB\n");
      b.append("cell_width=").append(CELL_W).append('\n');
      b.append("cell_height=").append(CELL_H).append('\n');
      b.append("grid_x=0\n");
      b.append("grid_y=0\n");
      b.append("columns=").append(GLYPHS).append('\n');
      b.append("rows=").append(ROWS).append('\n');
      b.append("codepoint_min=").append(CP_MIN).append('\n');
      b.append("codepoint_max=").append(CP_MAX).append('\n');
      b.append("baseline=").append(BASELINE).append('\n');
      b.append("glyph_count=").append(GLYPHS * ROWS).append('\n');
      for (int row = 0; row < SIZES.length; row++) {
         b.append("row").append(row).append("_size=").append(SIZES[row]).append('\n');
         b.append("row").append(row).append("_ascent=").append(metrics[row].ascent).append('\n');
         b.append("row").append(row).append("_descent=").append(metrics[row].descent).append('\n');
         b.append("row").append(row).append("_leading=").append(metrics[row].leading).append('\n');
         b.append("row").append(row).append("_max_advance=").append(metrics[row].maxAdvance).append('\n');
         b.append("row").append(row).append("_height=").append(metrics[row].height).append('\n');
      }
      b.append("sha256_ttf=").append(TTF_SHA256).append('\n');
      b.append("ttf=").append(TTF).append('\n');
      b.append("sha256_png=").append(sha256(png)).append('\n');
      b.append(rows);
      File txt = new File(dir, "atlas.txt");
      writeFile(txt, b.toString());
      System.out.println("atlas: wrote " + png.getPath() + " (" + img.getWidth() + "x" + img.getHeight() + ")");
      System.out.println("atlas: wrote " + txt.getPath() + " glyphs=" + (GLYPHS * ROWS));
      System.out.println("atlas: sha256_png=" + sha256(png));
      System.out.println("atlas: sha256_txt=" + sha256(txt));
   }

   /** Verify: the committed manifest is pinned whole by its sha256, and a
    *  documented SAMPLE of cells (space + 10 digits + 26 upper + 26 lower at
    *  both sizes = 126) is re-rendered and compared pixel-for-pixel. All 190
    *  advances are re-checked (cheap), so the sample is only about raster. */
   static void atlasVerify() throws Exception {
      File dir = atlasDir();
      File txt = new File(dir, "atlas.txt");
      File png = new File(dir, "atlas.png");
      if (!txt.isFile() || !png.isFile()) {
         Check.ok(false, "atlas_artifacts_present");
         return;
      }
      Map<String, String> man = parseManifest(txt);
      String manifestSha = sha256(txt);
      pin("atlas_manifest_sha256", manifestSha);
      // pixel.toml carries the same digest as the shared checklist; the
      // blessed pin row above stays the copy the --bless artifact owns
      manifestCheck(manifestSha, ATLAS.str("sha256_manifest"), "atlas_txt_sha256_matches_manifest");
      Check.eq(man.get("format"), ATLAS.str("format"), "atlas_format_tag");
      Check.eq(man.get("cell_width"), String.valueOf(CELL_W), "atlas_cell_width_matches_suite");
      Check.eq(man.get("cell_height"), String.valueOf(CELL_H), "atlas_cell_height_matches_suite");
      Check.eq(man.get("baseline"), String.valueOf(BASELINE), "atlas_baseline_matches_suite");
      Check.eq(man.get("sha256_ttf"), TTF_SHA256, "atlas_manifest_ttf_sha256");
      Check.eq(sha256(new File(TTF)), man.get("sha256_ttf"), "atlas_ttf_file_matches_manifest");
      // the PNG must be the digests pixel.toml publishes for the Rust mirror;
      // the atlas.txt copy of that digest is itself pinned transitively by
      // atlas_manifest_sha256
      Check.eq(sha256(png), ATLAS.str("sha256_png"), "atlas_png_matches_manifest");
      int count = 0;
      List<String> lines = readLines(txt);
      List<String> glyphLines = new ArrayList<String>();
      for (int i = 0; i < lines.size(); i++) {
         if (lines.get(i).startsWith("glyph ")) {
            glyphLines.add(lines.get(i));
            count++;
         }
      }
      Check.eq(count, GLYPHS * ROWS, "atlas_glyph_record_count_190");
      pin("atlas_glyph_count", "" + count);

      BufferedImage atlas = ImageIO.read(png);
      Check.ok(atlas != null, "atlas_png_readable");
      if (atlas == null) {
         return;
      }
      Check.eq(atlas.getWidth(), GLYPHS * CELL_W, "atlas_png_width");
      Check.eq(atlas.getHeight(), ROWS * CELL_H, "atlas_png_height");

      // advances: every glyph, both sizes, against a fresh FontMetrics
      int advMismatch = 0;
      int[][] wantAdv = new int[SIZES.length][GLYPHS];
      for (int i = 0; i < glyphLines.size(); i++) {
         String[] t = glyphLines.get(i).split(" ");
         int size = Integer.parseInt(t[1]);
         int cp = Integer.parseInt(t[2]);
         int adv = Integer.parseInt(t[3]);
         int row = size == 13 ? 0 : 1;
         wantAdv[row][cp - CP_MIN] = adv;
      }
      for (int row = 0; row < SIZES.length; row++) {
         Font f = atlasFont(row);
         FontMetrics fm = freshMetrics(f);
         for (int cp = CP_MIN; cp <= CP_MAX; cp++) {
            if (wantAdv[row][cp - CP_MIN] != fm.charWidth(cp)) {
               advMismatch++;
            }
         }
      }
      Check.eq(advMismatch, 0, "atlas_advance_mismatch_count_0");

      // raster sample: space + digits + upper + lower, both sizes
      String sample = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
      int sampled = 0;
      int cellMismatch = 0;
      for (int row = 0; row < SIZES.length; row++) {
         Font f = atlasFont(row);
         for (int i = 0; i < sample.length(); i++) {
            int cp = sample.charAt(i);
            BufferedImage cell = glyphCell(f, cp);
            int[] got = cell.getRGB(0, 0, CELL_W, CELL_H, null, 0, CELL_W);
            int[] want = atlas.getRGB((cp - CP_MIN) * CELL_W, row * CELL_H, CELL_W, CELL_H, null, 0, CELL_W);
            if (!Arrays.equals(got, want)) {
               cellMismatch++;
               System.out.println("ATLAS CELL MISMATCH pt=" + SIZES[row] + " cp=" + cp);
            }
            sampled++;
         }
      }
      Check.eq(cellMismatch, 0, "atlas_sampled_cell_mismatch_count_0");
      // space + 10 digits + 26 upper + 26 lower = 63 per size, 126 total:
      // a superset of the required 37-per-size sample.
      Check.eq(sampled, 126, "atlas_sampled_cell_count_126");
      pin("atlas_sampled_cell_count", "" + sampled);
   }

   static Map<String, String> parseManifest(File f) throws IOException {
      Map<String, String> out = new LinkedHashMap<String, String>();
      List<String> lines = readLines(f);
      for (int i = 0; i < lines.size(); i++) {
         String line = lines.get(i).trim();
         if (line.length() == 0 || line.startsWith("#") || line.startsWith("glyph ")) {
            continue;
         }
         int eq = line.indexOf('=');
         if (eq > 0) {
            out.put(line.substring(0, eq), line.substring(eq + 1));
         }
      }
      return out;
   }

   /** One atlas cell re-rendered with the platform default hints. */
   static BufferedImage glyphCell(Font f, int cp) {
      BufferedImage img = new BufferedImage(CELL_W, CELL_H, BufferedImage.TYPE_INT_RGB);
      Graphics2D g = img.createGraphics();
      g.setColor(Color.WHITE);
      g.fillRect(0, 0, CELL_W, CELL_H);
      g.setColor(Color.BLACK);
      g.setFont(f);
      g.drawString(new String(new char[]{(char) cp}), GLYPH_DX, BASELINE);
      g.dispose();
      return img;
   }

   static FontMetrics freshMetrics(Font f) {
      BufferedImage img = new BufferedImage(1, 1, BufferedImage.TYPE_INT_RGB);
      Graphics2D g = img.createGraphics();
      FontMetrics fm = g.getFontMetrics(f);
      g.dispose();
      return fm;
   }

   // =====================================================================
   // (c) the screen pins
   // =====================================================================

   /** Every screen is drawn into a FRESH 550x400 TYPE_INT_RGB canvas. State
    *  is set explicitly per pin (the draw methods are stateful: aflk, flipo,
    *  sc, gatey, pstar); the references in the comments are the original
    *  lines whose branch the pin exercises. */
   static void screens() throws Exception {
      setup();

      // ---- nofocus (xtGraphics.java:1101-1113). drawcs var7==3 never reads
      // Medium, so the pin runs with m==null: the lost-focus overlay must
      // not depend on the physics object.
      {
         BufferedImage img = fresh();
         Medium keep = X.m;
         X.m = null;
         X.aflk = false;
         X.nofocus(img.getGraphics());
         X.m = keep;
         pin("screen_nofocus", md5(img));
      }

      // ---- loading (xtGraphics.java:2897-2928). The ctor ends at
      // dnload=485 / kbload=514 (var4==2) => "95 % loaded | 29 KB remaining"
      // and the 26+188px bar. Also the 0 KB head of the same bar.
      pin("screen_loading_final_95pct", md5(renderLoading(485)));
      pin("screen_loading_start_0kb", md5(renderLoading(0)));

      // ---- clicknow (xtGraphics.java:3364-3376) is the fase-111 overlay
      // GameSparker.java:649-655 paints ON TOP of the ctor's loading canvas
      // for var23 0..799. It never sets ftm (:2472 drawcs), so the loading
      // call's 11pt FontMetrics is the one it centers with - B10's caveat.
      pin("screen_clicknow_over_loading_aflk_false", md5(renderClicknow(false)));
      pin("screen_clicknow_over_loading_aflk_true", md5(renderClicknow(true)));

      // ---- rad = fase 9 (xtGraphics.java:3390-3412). var2 is the
      // GameSparker var23 tick: 0 plays powerup and is the first frame.
      // m.random() picks the 13pt x offset from the preset walk.
      for (int tick = 0; tick <= 1; tick++) {
         pin("screen_rad_tick" + tick + "_aflk_false", md5(renderRad(tick, false)));
         pin("screen_rad_tick" + tick + "_aflk_true", md5(renderRad(tick, true)));
      }
      // var2 is GameSparker's var23 tick (0..199). It only gates the
      // powerup.play() at :3391-3393; it is PAINT-NEUTRAL, so the splash's
      // only on-screen motion is the aflk colour alternation of the second
      // line. Pin that reading rather than 200 identical md5s.
      Check.eq(md5(renderRad(0, false)), md5(renderRad(199, false)), "screen_rad_tick_paint_neutral");

      // ---- maini = fase 10 menu (xtGraphics.java:3160-3272). flipo 0 draws
      // bgmain+maini then the three option boxes; opselect 0/1/2 moves the
      // blue (or locked 32,48,160 when shaded) outline.
      for (int op = 0; op <= 2; op++) {
         pin("screen_maini_opselect" + op, md5(renderMaini(op, false)));
      }
      pin("screen_maini_opselect1_shaded", md5(renderMaini(1, true)));

      // ---- inst = fase 11 instruction pages (xtGraphics.java:320-369).
      // flipo 0/2/4 are the transition draws that paint a page background
      // AND its button in one call; flipo 1/3/5 only repaint the button, so
      // those pins pre-roll the canvas exactly as the real persistent
      // offImage would (a fresh buffer at flipo 5 is just black+contin2).
      for (int f = 0; f <= 5; f++) {
         pin("screen_inst_flipo" + f, md5(renderInst(f)));
      }

      // ---- credits = fase 8 (xtGraphics.java:1452-1513), pages 0..3.
      // flipo 0 is the entry tick: powerup.play(), then the flipo==1 page.
      for (int f = 0; f <= 3; f++) {
         pin("screen_credits_flipo" + f, md5(renderCredits(f)));
      }

      // ---- carselect = fase 7 (xtGraphics.java:3557-3704). GameSparker.java:748
      // passes the 43-slot MODEL table (loadbase) and the game calls this every
      // tick: the first call draws the pristine model, then re-seats it
      // (:3580-3585) for the next frame. Pins are therefore the SECOND tick's
      // canvas (the steady state the player sees), and the countdown pins are
      // the second tick of the flipo>10 run (:3649-3673).
      pin("screen_carselect_car0_stats", md5(renderCarselect(0, 0, 1, 0, false, 2)));
      // car 4 = Nimi, the user's named repro state: selectable at
      // unlocked=1 (the gate arm (4-4)*2 >= 1 is false, verified by the T2
      // capture), label clearing the bar exactly as car0's.
      pin("screen_carselect_car4_stats", md5(renderCarselect(4, 0, 1, 0, false, 2)));
      pin("screen_carselect_locked_gatey0", md5(renderCarselect(5, 0, 1, 0, false, 2)));
      pin("screen_carselect_locked_gatey300", md5(renderCarselect(5, 0, 1, 300, false, 2)));
      pin("screen_carselect_countdown_flipo20", md5(renderCarselect(0, 20, 1, 300, true, 2)));
      pin("screen_carselect_car9_stats", md5(renderCarselect(9, 0, 11, 0, false, 2)));

      // ---- stageselect = fase 1 (xtGraphics.java:2485-2538). Stage 1 has
      // no back button, stage 11 prints "Final Party Stage" and has no next
      // button; both pback/pnext variants are pinned.
      pin("screen_stageselect_stage1", md5(renderStageselect(1, 0, 0)));
      pin("screen_stageselect_stage5", md5(renderStageselect(5, 0, 0)));
      pin("screen_stageselect_stage5_buttons_pressed", md5(renderStageselect(5, 1, 1)));
      pin("screen_stageselect_stage11", md5(renderStageselect(11, 0, 0)));

      // ---- loadingfailed = fase 3 (xtGraphics.java:3982-4000).
      pin("screen_loadingfailed_stage1", md5(renderLoadingfailed(1)));
      pin("screen_loadingfailed_stage10", md5(renderLoadingfailed(10)));

      // ---- cantgo = fase 4 (xtGraphics.java:263-294). lockcnt only gates
      // the exit, it is paint-neutral; the flicker state changes the locked
      // line colour (red when aflk==false, black when true).
      pin("screen_cantgo_aflk_false_red", md5(renderCantgo(1, 70, false)));
      pin("screen_cantgo_aflk_true_black", md5(renderCantgo(1, 70, true)));
      pin("screen_cantgo_unlocked10", md5(renderCantgo(10, 70, false)));
      Check.eq(md5(renderCantgo(1, 70, false)), md5(renderCantgo(1, 7, false)),
            "screen_cantgo_lockcnt_paint_neutral");

      // ---- musicomp = fase 6 (xtGraphics.java:3274-3362). pstar selects
      // star[pstar]; stage 1 prints the extra hint because stage==unlocked,
      // stage 10 prints the guidance-arrow note.
      for (int p = 0; p <= 2; p++) {
         pin("screen_musicomp_stage1_pstar" + p, md5(renderMusicomp(1, 1, p)));
      }
      pin("screen_musicomp_stage10_pstar0", md5(renderMusicomp(10, 1, 0)));
      pin("screen_musicomp_stage2_pstar0", md5(renderMusicomp(2, 1, 0)));
   }

   static BufferedImage renderLoading(int dnload) {
      BufferedImage img = fresh();
      X.dnload = dnload;
      X.loading(img.getGraphics(), APP);
      return img;
   }

   static BufferedImage renderClicknow(boolean aflk) {
      BufferedImage img = fresh();
      // GAME TRUTH: the applet paints loading and clicknow on the SAME
      // persistent canvas Graphics - GameSparker.java:1507 sets
      // this.rd = offImage.getGraphics() once in init(), :592 hands that rd to
      // the xtGraphics ctor, :597 calls loading(this.rd, this) - which installs
      // the 11pt SansSerif bold font (:2897-2928) - and :655 calls
      // clicknow(this.rd) on that same surface, so drawcs' drawString
      // rasterises at 11pt. A fresh Graphics per call would drop the font back
      // to the toolkit default (Dialog.plain 12 in the container) while the
      // caption is still centred with the 11pt ftm - not a real frame.
      Graphics g = img.getGraphics();
      X.dnload = 485;
      X.loading(g, APP);
      X.aflk = aflk;
      X.clicknow(g);
      return img;
   }

   static BufferedImage renderRad(int tick, boolean aflk) {
      BufferedImage img = fresh();
      preset();
      X.aflk = aflk;
      X.rad(img.getGraphics(), tick);
      return img;
   }

   static BufferedImage renderMaini(int opselect, boolean shaded) {
      BufferedImage img = fresh();
      X.flipo = 0;
      X.opselect = opselect;
      X.shaded = shaded;
      X.aflk = false;
      X.maini(img.getGraphics(), CTL);
      return img;
   }

   static BufferedImage renderInst(int target) {
      BufferedImage img = fresh();
      Graphics g = img.getGraphics();
      Control c = new Control(M);
      X.flipo = 0;
      c.enter = false;
      X.inst(g, c); // 0 -> 1, paints inst1 + next
      if (target == 0 || target == 1) {
         if (target == 1) {
            X.inst(g, c);
         }
         return img;
      }
      c.enter = true;
      X.inst(g, c); // 1 -> 2
      c.enter = false;
      X.inst(g, c); // 2 -> 3, paints inst2 + next
      if (target == 2 || target == 3) {
         if (target == 3) {
            X.inst(g, c);
         }
         return img;
      }
      c.enter = true;
      X.inst(g, c); // 3 -> 4
      c.enter = false;
      X.inst(g, c); // 4 -> 5, paints inst3 + contin2
      if (target == 4) {
         return img;
      }
      X.inst(g, c); // 5 stays 5, repaints contin2
      return img;
   }

   static BufferedImage renderCredits(int target) {
      BufferedImage img = fresh();
      preset();
      X.flipo = target;
      X.aflk = false;
      X.credits(img.getGraphics(), new Control(M));
      return img;
   }

   static BufferedImage renderCarselect(int car, int flipo, int unlocked, int gatey, boolean nextc, int ticks) {
      BufferedImage img = fresh();
      Graphics g = img.getGraphics();
      X.sc[0] = car;
      X.unlocked = unlocked;
      X.gatey = gatey;
      X.nextc = nextc;
      X.pback = 0;
      X.pnext = 0;
      X.pcontin = 0;
      ContO[] models = freshCars();
      Madness mad = new Madness(M, new Record(M), X, 0);
      Control c = new Control(M);
      // The real flipo>0 countdown begins from the seated idle frame (the
      // player pressed left/right while flipo==0), so seat the car first.
      if (flipo > 0) {
         X.flipo = 0;
         preset();
         X.aflk = false;
         X.carselect(c, models, mad, g);
      }
      X.flipo = flipo;
      for (int i = 0; i < ticks; i++) {
         preset();
         X.aflk = false;
         X.carselect(c, models, mad, g);
      }
      return img;
   }

   static BufferedImage renderStageselect(int stage, int pback, int pnext) {
      BufferedImage img = fresh();
      CheckPoints cp = new CheckPoints();
      cp.stage = stage;
      cp.name = "The Introductory Stage";
      X.pback = pback;
      X.pnext = pnext;
      X.pcontin = 0;
      X.stageselect(img.getGraphics(), cp, new Control(M));
      return img;
   }

   static BufferedImage renderLoadingfailed(int stage) {
      BufferedImage img = fresh();
      X.pcontin = 0;
      X.loadingfailed(stage, new Control(M), img.getGraphics());
      return img;
   }

   static BufferedImage renderCantgo(int unlocked, int lockcnt, boolean aflk) {
      BufferedImage img = fresh();
      X.unlocked = unlocked;
      X.lockcnt = lockcnt;
      X.aflk = aflk;
      X.pback = 0;
      X.cantgo(img.getGraphics(), new Control(M));
      return img;
   }

   static BufferedImage renderMusicomp(int stage, int unlocked, int pstar) {
      BufferedImage img = fresh();
      X.pstar = pstar;
      X.unlocked = unlocked;
      X.musicomp(stage, img.getGraphics(), new Control(M));
      return img;
   }

   // =====================================================================
   // shared fixture helpers
   // =====================================================================

   /** The TControl recipe: rand=5s and diup all false, trn=0 ->
    *  0.4,0.3,0.2,0.1,0.0,0.9,... (B3/B7). cntrn is the deterministic-walk
    *  budget: Medium.random() refills from Math.random the moment it hits 0
    *  (Medium.java:49-60), and a carselect frame runs ContO.d -> Plane.d,
    *  whose embossed planes consume m.random() per plane - far more than the
    *  100 a gameplay recipe needs. A large budget keeps the whole frame on
    *  the SAME deterministic walk without changing any drawn value (the walk
    *  is identical for every cntrn>0). */
   static void preset() {
      M.cntrn = 1000000;
      M.rand[0] = 5;
      M.rand[1] = 5;
      M.rand[2] = 5;
      M.diup[0] = false;
      M.diup[1] = false;
      M.diup[2] = false;
      M.trn = 0;
   }

   static ContO[] freshCars() {
      ContO[] out = new ContO[10];
      for (int i = 0; i < 10; i++) {
         out[i] = new ContO(CARS[i], 0, 0, 0, 0);
      }
      return out;
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
   // pixel / hash / file helpers
   // =====================================================================

   static int[] pixels(BufferedImage img) {
      return img.getRGB(0, 0, img.getWidth(), img.getHeight(), null, 0, img.getWidth());
   }

   static boolean pixelEqual(BufferedImage a, BufferedImage b) {
      if (a.getWidth() != b.getWidth() || a.getHeight() != b.getHeight()) {
         return false;
      }
      return Arrays.equals(pixels(a), pixels(b));
   }

   /** md5 over the ARGB ints serialized big-endian (channel order fixed in
    *  the bytes so a port does not have to guess the host endianness). */
   static String md5(BufferedImage img) {
      return hash("MD5", pixels(img));
   }

   static String md5(int[] px) {
      return hash("MD5", px);
   }

   static String hash(String algo, int[] px) {
      try {
         MessageDigest d = MessageDigest.getInstance(algo);
         byte[] b = new byte[px.length * 4];
         for (int i = 0; i < px.length; i++) {
            b[i * 4] = (byte) (px[i] >>> 24);
            b[i * 4 + 1] = (byte) (px[i] >>> 16);
            b[i * 4 + 2] = (byte) (px[i] >>> 8);
            b[i * 4 + 3] = (byte) px[i];
         }
         return hex(d.digest(b));
      } catch (Exception e) {
         throw new RuntimeException(e);
      }
   }

   static String sha256(File f) throws IOException {
      try {
         MessageDigest d = MessageDigest.getInstance("SHA-256");
         InputStream in = new FileInputStream(f);
         byte[] buf = new byte[65536];
         int n;
         while ((n = in.read(buf)) > 0) {
            d.update(buf, 0, n);
         }
         in.close();
         return hex(d.digest());
      } catch (IOException e) {
         throw e;
      } catch (Exception e) {
         throw new RuntimeException(e);
      }
   }

   static String hex(byte[] h) {
      StringBuilder s = new StringBuilder(h.length * 2);
      for (int i = 0; i < h.length; i++) {
         s.append(String.format("%02x", h[i] & 255));
      }
      return s.toString();
   }

   static List<String> readLines(File f) throws IOException {
      List<String> out = new ArrayList<String>();
      FileInputStream in = new FileInputStream(f);
      ByteArrayOutputStream b = new ByteArrayOutputStream();
      byte[] buf = new byte[8192];
      int n;
      while ((n = in.read(buf)) > 0) {
         b.write(buf, 0, n);
      }
      in.close();
      String[] parts = b.toString("UTF-8").split("\n", -1);
      for (int i = 0; i < parts.length; i++) {
         out.add(parts[i].endsWith("\r") ? parts[i].substring(0, parts[i].length() - 1) : parts[i]);
      }
      return out;
   }

   static void writeFile(File f, String s) throws IOException {
      OutputStream out = new FileOutputStream(f);
      out.write(s.getBytes("UTF-8"));
      out.close();
   }

   static URL dirUrl(String dir) throws Exception {
      String u = new File(dir).toURI().toURL().toExternalForm();
      if (!u.endsWith("/")) {
         u = u + "/";
      }
      return new URL(u);
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
}
