import java.io.BufferedReader;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * A deliberately minimal TOML-subset reader for the behavior-test fixtures.
 *
 * WHY: the fixture TOMLs under tests/fixtures/toml/ are the single source of
 * truth for the values the Java tests and the future Rust mirror tests both
 * consume (tests/fixtures/toml/README.md is the contract). Java 8 ships no
 * TOML parser and the project stays dependency-free, so this reader covers
 * exactly the subset the fixture files use - no more:
 *
 *   [table]  [table.sub]  [[array.of.tables]]
 *   key = 123 | -4 | 1.5 | 1.5e-3 | "text" | true | false | [1, 2, 3] | ["a"]
 *   # comments to end of line; blank lines ignored
 *
 * One TOML rule matters for the fixtures and is implemented structurally: a
 * header component that names an array-of-tables binds into that array's
 * MOST RECENT row ([[fruits]] then [[fruits.varieties]] then
 * [fruits.varieties.physical] nests inside the last varieties row), which is
 * what lets [[scenario]] rows carry nested [[scenario.phase]] rows.
 *
 * Explicitly NOT supported (banned in the fixtures by the README): dotted
 * keys, inline tables, multi-line arrays, multi-line strings, dates/times,
 * hex/octal/binary/underscore/leading-plus number forms, NaN/Infinity.
 * Anything outside the subset fails with the file, line and key path in the
 * message.
 *
 * Every lookup failure throws IllegalStateException naming the file, the key
 * path and the defining line; consumers assert shapes up front (requireKeys,
 * tableCount) so a malformed fixture fails loudly at load, never silently.
 * Loads are not cached - each is a parse of a few KB, negligible next to the
 * suites.
 */
public final class Toml {

   /** One parsed value plus the line it was defined on (for error text). */
   private static final class Val {
      final Object v;
      final int line;
      Val(Object v, int line) { this.v = v; this.line = line; }
      public String toString() { return String.valueOf(v); }
   }

   private final String file;
   private final String path;              // dotted key path of this view ("" = root)
   private final Map<String, Object> map;  // String key -> Val | Map (subtable) | List<Val> ([[rows]], Val.v = Map)

   private Toml(String file, String path, Map<String, Object> map) {
      this.file = file;
      this.path = path;
      this.map = map;
   }

   // ── loading ─────────────────────────────────────────────────────────────

   public static Toml load(String file) {
      List<String> lines = new ArrayList<String>();
      try {
         BufferedReader r = Files.newBufferedReader(Paths.get(file), StandardCharsets.UTF_8);
         try {
            for (String line; (line = r.readLine()) != null; ) lines.add(line);
         } finally {
            r.close();
         }
      } catch (IOException e) {
         throw new IllegalStateException("toml: cannot read " + file + ": " + e, e);
      }
      if (!lines.isEmpty() && !lines.get(0).isEmpty() && lines.get(0).charAt(0) == '\uFEFF') {
         lines.set(0, lines.get(0).substring(1)); // strip a UTF-8 BOM defensively
      }
      Map<String, Object> root = new TreeMap<String, Object>();
      Map<String, Object> cur = root;            // the table key-value lines land in
      for (int i = 0; i < lines.size(); i++) {
         int ln = i + 1;
         String line = stripComment(lines.get(i)).trim();
         if (line.isEmpty()) continue;
         if (line.startsWith("[")) {
            boolean isArr = line.startsWith("[[");
            if (isArr && (!line.endsWith("]]") || line.length() < 5))
               die(file, ln, "bad [[table]] header: " + line);
            if (!isArr && (!line.endsWith("]") || line.length() < 3))
               die(file, ln, "bad [table] header: " + line);
            List<String> comps = comps(file, ln, line.substring(isArr ? 2 : 1,
                  line.length() - (isArr ? 2 : 1)).trim());
            // walk the intermediates; an array-of-tables binds into its
            // most recent row (the TOML rule the fixtures' nesting needs)
            Map<String, Object> parent = root;
            for (int k = 0; k < comps.size() - 1; k++) {
               String c = comps.get(k);
               Object u = unwrap(parent.get(c), file, ln, "table '" + c + "' of [" + join(comps) + "]");
               if (u == null) {
                  Map<String, Object> n = new TreeMap<String, Object>();
                  parent.put(c, new Val(n, ln));
                  parent = n;
               } else if (u instanceof Map) {
                  parent = type(Map.class, u);
               } else if (u instanceof List) {
                  List<Val> rows = type(List.class, u);
                  if (rows.isEmpty())
                     die(file, ln, "empty array '" + c + "' cannot hold [" + line + "]");
                  Val last = rows.get(rows.size() - 1);
                  if (!(last.v instanceof Map))
                     die(file, ln, "array '" + c + "' rows are not tables");
                  parent = type(Map.class, last.v);
               } else {
                  die(file, ln, "key '" + c + "' of [" + join(comps) + "] is already a value");
               }
            }
            String name = comps.get(comps.size() - 1);
            Object u = unwrap(parent.get(name), file, ln, "key '" + name + "' of [" + join(comps) + "]");
            if (isArr) {
               List<Val> rows;
               if (u == null) {
                  rows = new ArrayList<Val>();
                  parent.put(name, new Val(rows, ln));
               } else if (u instanceof List) {
                  rows = type(List.class, u);
               } else {
                  die(file, ln, "'" + name + "' of [[" + join(comps) + "]] is already a value/table");
                  return null; // unreachable
               }
               Map<String, Object> row = new TreeMap<String, Object>();
               rows.add(new Val(row, ln));
               cur = row;
            } else {
               if (u == null) {
                  Map<String, Object> n = new TreeMap<String, Object>();
                  parent.put(name, new Val(n, ln));
                  cur = n;
               } else if (u instanceof Map) {
                  cur = type(Map.class, u);
               } else {
                  die(file, ln, "'" + name + "' of [" + join(comps) + "] is already a value/array");
                  return null; // unreachable
               }
            }
         } else {
            kv(cur, file, ln, line);
         }
      }
      return new Toml(file, "", root);
   }

   /** Header components; each must be a bare key (subset rule). */
   private static List<String> comps(String file, int ln, String name) {
      List<String> out = new ArrayList<String>();
      for (String c : name.split("\\.")) {
         String comp = c.trim();
         if (!comp.matches("[A-Za-z0-9_-]+"))
            die(file, ln, "bad table name component '" + comp + "' in [" + name + "]");
         out.add(comp);
      }
      if (out.isEmpty()) die(file, ln, "empty table header");
      return out;
   }

   /** One key = value line into the current table. */
   private static void kv(Map<String, Object> cur, String file, int ln, String line) {
      int eq = line.indexOf('=');
      if (eq <= 0) die(file, ln, "expected key = value, got: " + line);
      String key = line.substring(0, eq).trim();
      if (!key.matches("[A-Za-z0-9_-]+"))
         die(file, ln, "bad bare key '" + key + "' (subset: A-Za-z0-9_- only)");
      Object v = parseValue(line.substring(eq + 1).trim(), file, ln, key);
      if (cur.containsKey(key)) die(file, ln, "duplicate key '" + key + "'");
      cur.put(key, new Val(v, ln));
   }

   /** Parse one value; the input is already trimmed. */
   private static Object parseValue(String s, String file, int ln, String key) {
      if (s.isEmpty()) die(file, ln, "missing value for '" + key + "'");
      if (s.charAt(0) == '"') return parseString(s, file, ln, key);
      if (s.charAt(0) == '[') return parseArray(s, file, ln, key);
      if (s.contains(" ")) die(file, ln, "bare value '" + s + "' for '" + key
            + "' must be one token (quote strings)");
      if (s.equals("true")) return Boolean.TRUE;
      if (s.equals("false")) return Boolean.FALSE;
      if (s.matches("-?[0-9]+")) return Long.valueOf(s);
      if (s.matches("-?[0-9]+(\\.[0-9]+)?([eE][+-]?[0-9]+)?") && (s.contains(".") || s.contains("e")
            || s.contains("E")))
         return Double.valueOf(s);
      die(file, ln, "unsupported value '" + s + "' for '" + key
            + "' (subset: int, decimal float, bool, quoted string, one-line array)");
      return null; // unreachable
   }

   /** "text" with \\ \" \n \t \r escapes; nothing may follow the quote. */
   private static String parseString(String s, String file, int ln, String key) {
      StringBuilder out = new StringBuilder();
      int i = 1;
      while (i < s.length()) {
         char c = s.charAt(i);
         if (c == '"') {
            if (!s.substring(i + 1).trim().isEmpty())
               die(file, ln, "trailing garbage after string for '" + key + "': " + s);
            return out.toString();
         }
         if (c == '\\') {
            if (i + 1 >= s.length()) die(file, ln, "unterminated escape in '" + key + "'");
            char e = s.charAt(++i);
            if (e == 'n') out.append('\n');
            else if (e == 't') out.append('\t');
            else if (e == 'r') out.append('\r');
            else if (e == '"') out.append('"');
            else if (e == '\\') out.append('\\');
            else die(file, ln, "unsupported escape \\" + e + " in '" + key + "' (subset: \\ \\\" \\n \\t \\r)");
         } else {
            out.append(c);
         }
         i++;
      }
      die(file, ln, "unterminated string for '" + key + "': " + s);
      return null; // unreachable
   }

   /** One-line [a, b, c] of scalars; nested arrays are not in the subset. */
   private static List<Val> parseArray(String s, String file, int ln, String key) {
      if (!s.endsWith("]"))
         die(file, ln, "unterminated array for '" + key + "' (subset: one-line arrays)");
      String inner = s.substring(1, s.length() - 1).trim();
      List<Val> out = new ArrayList<Val>();
      if (inner.isEmpty()) return out;
      int depth = 0;
      boolean in = false;
      boolean esc = false;
      int start = 0;
      List<String> parts = new ArrayList<String>();
      for (int i = 0; i < inner.length(); i++) {
         char c = inner.charAt(i);
         if (esc) { esc = false; }
         else if (c == '\\' && in) { esc = true; }
         else if (c == '"') { in = !in; }
         else if (c == ',' && !in) {
            parts.add(inner.substring(start, i));
            start = i + 1;
         }
      }
      parts.add(inner.substring(start));
      for (String p : parts) {
         String e = p.trim();
         if (e.isEmpty()) die(file, ln, "empty array element in '" + key + "'");
         if (e.charAt(0) == '[')
            die(file, ln, "nested arrays are not in the subset ('" + key + "')");
         out.add(new Val(parseValue(e, file, ln, key), ln));
      }
      return out;
   }

   /** Every stored value is a Val; unwrap it (null passes through). */
   private static Object unwrap(Object stored, String file, int ln, String ctx) {
      if (stored == null) return null;
      if (!(stored instanceof Val))
         die(file, ln, ctx + ": internal: unwrapped storage");
      return ((Val) stored).v;
   }

   /** Cut a # comment, honoring quotes. */
   private static String stripComment(String s) {
      boolean in = false;
      boolean esc = false;
      for (int i = 0; i < s.length(); i++) {
         char c = s.charAt(i);
         if (esc) { esc = false; continue; }
         if (c == '\\' && in) { esc = true; continue; }
         if (c == '"') in = !in;
         if (c == '#' && !in) return s.substring(0, i);
      }
      return s;
   }

   private static String join(List<String> comps) {
      StringBuilder b = new StringBuilder();
      for (String c : comps) {
         if (b.length() > 0) b.append('.');
         b.append(c);
      }
      return b.toString();
   }

   private static void die(String file, int ln, String msg) {
      throw new IllegalStateException("toml " + file + ":" + ln + ": " + msg);
   }

   @SuppressWarnings("unchecked")
   private static <T> T type(Class<T> t, Object o) {
      return (T) o;
   }

   // ── lookup API ──────────────────────────────────────────────────────────

   /** True when the key is present in this table (value type unchecked). */
   public boolean has(String key) {
      return map.containsKey(key);
   }

   /** The keys of this table, sorted (stable for shape/count pins). */
   public List<String> keys() {
      return new ArrayList<String>(map.keySet());
   }

   /** A subtable; throws unless present and a [table]. */
   public Toml table(String key) {
      Val v = val(key);
      if (!(v.v instanceof Map)) fail(key, v, "table");
      return new Toml(file, join(key), type(Map.class, v.v));
   }

   /** The rows of an [[array.of.tables]]; throws unless present and an array. */
   public List<Toml> tables(String key) {
      Val v = val(key);
      if (!(v.v instanceof List)) fail(key, v, "array of tables");
      List<Val> rows = type(List.class, v.v);
      List<Toml> out = new ArrayList<Toml>();
      for (Val e : rows) {
         if (!(e.v instanceof Map)) fail(key, e, "table row");
         out.add(new Toml(file, join(key) + "[" + out.size() + "]", type(Map.class, e.v)));
      }
      return out;
   }

   public int tableCount(String key) {
      return tables(key).size();
   }

   public Toml tableAt(String key, int i) {
      return tables(key).get(i);
   }

   public String str(String key) {
      Val v = val(key);
      if (!(v.v instanceof String)) fail(key, v, "string");
      return (String) v.v;
   }

   /** An integer, range-checked into int (the game's integer width). */
   public int intv(String key) {
      Val v = val(key);
      if (!(v.v instanceof Long)) fail(key, v, "integer");
      long l = (Long) v.v;
      if (l < Integer.MIN_VALUE || l > Integer.MAX_VALUE) fail(key, v, "int-range integer");
      return (int) l;
   }

   public long longv(String key) {
      Val v = val(key);
      if (!(v.v instanceof Long)) fail(key, v, "integer");
      return (Long) v.v;
   }

   /** A number as float; an integer-valued TOML number is accepted. */
   public float floatv(String key) {
      Val v = val(key);
      if (v.v instanceof Double) return (float) (double) (Double) v.v;
      if (v.v instanceof Long) return (float) (long) (Long) v.v;
      fail(key, v, "number");
      return 0; // unreachable
   }

   public boolean boolv(String key) {
      Val v = val(key);
      if (!(v.v instanceof Boolean)) fail(key, v, "boolean");
      return (Boolean) v.v;
   }

   public int[] intArr(String key) {
      List<Val> list = arr(key);
      int[] out = new int[list.size()];
      for (int i = 0; i < out.length; i++) {
         Val e = list.get(i);
         if (!(e.v instanceof Long)) fail(key, e, "integer element", i);
         long l = (Long) e.v;
         if (l < Integer.MIN_VALUE || l > Integer.MAX_VALUE) fail(key, e, "int-range integer", i);
         out[i] = (int) l;
      }
      return out;
   }

   public long[] longArr(String key) {
      List<Val> list = arr(key);
      long[] out = new long[list.size()];
      for (int i = 0; i < out.length; i++) {
         Val e = list.get(i);
         if (!(e.v instanceof Long)) fail(key, e, "integer element", i);
         out[i] = (Long) e.v;
      }
      return out;
   }

   /** Floats; integer-valued elements are accepted. */
   public float[] floatArr(String key) {
      List<Val> list = arr(key);
      float[] out = new float[list.size()];
      for (int i = 0; i < out.length; i++) {
         Val e = list.get(i);
         if (e.v instanceof Double) out[i] = (float) (double) (Double) e.v;
         else if (e.v instanceof Long) out[i] = (float) (long) (Long) e.v;
         else fail(key, e, "number element", i);
      }
      return out;
   }

   public String[] strArr(String key) {
      List<Val> list = arr(key);
      String[] out = new String[list.size()];
      for (int i = 0; i < out.length; i++) {
         Val e = list.get(i);
         if (!(e.v instanceof String)) fail(key, e, "string element", i);
         out[i] = (String) e.v;
      }
      return out;
   }

   /** Throws unless every named key is present in this table. */
   public void requireKeys(String... keys) {
      for (String k : keys)
         if (!map.containsKey(k))
            throw new IllegalStateException("toml " + file + ": "
                  + (path.isEmpty() ? k : path + "." + k) + ": missing required key");
   }

   public String file() {
      return file;
   }

   // ── internals ───────────────────────────────────────────────────────────

   private Val val(String key) {
      Object o = map.get(key);
      if (o == null)
         throw new IllegalStateException("toml " + file + ": "
               + (path.isEmpty() ? key : path + "." + key) + ": missing key");
      return (Val) o;
   }

   private List<Val> arr(String key) {
      Val v = val(key);
      if (!(v.v instanceof List)) fail(key, v, "array");
      return type(List.class, v.v);
   }

   private String join(String key) {
      return path.isEmpty() ? key : path + "." + key;
   }

   private void fail(String key, Val v, String want) {
      throw new IllegalStateException("toml " + file + ": " + join(key) + " (line " + v.line
            + "): expected " + want + ", got " + kind(v.v));
   }

   private void fail(String key, Val v, String want, int i) {
      throw new IllegalStateException("toml " + file + ": " + join(key) + "[" + i + "] (line "
            + v.line + "): expected " + want + ", got " + kind(v.v));
   }

   private static String kind(Object v) {
      if (v instanceof String) return "string";
      if (v instanceof Long) return "integer";
      if (v instanceof Double) return "float";
      if (v instanceof Boolean) return "boolean";
      if (v instanceof Map) return "table";
      if (v instanceof List) return "array";
      return String.valueOf(v);
   }
}
