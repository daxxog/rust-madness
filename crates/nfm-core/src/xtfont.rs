//! The real `Font("SansSerif", 1, 11|13)` + `FontMetrics` text raster, read
//! from the shared glyph atlas (`tests/fixtures/xtfont-atlas/`).
//!
//! The original rasterized text through each era JRE's platform rasterizer;
//! 2005 platform fonts no longer exist, so the pixel platform ships ONE
//! reproducible face - `tools/fonts/LiberationSans-Bold.ttf` mapped over
//! logical SansSerif bold - and pins its raster as a 95x2 grid of 20x22
//! cells (`atlas.png`) with a plain-ASCII manifest (`atlas.txt`). That
//! artifact is the port's text ground truth (notes/JAVA-BEHAVIOR/B12_pixel.md
//! §4); the earlier embedded `font8x8` stand-in is gone. The bytes come from
//! the committed fixtures on a dev filesystem and from the embedded resource
//! pack in a release binary (`pack.rs`), so the port needs no Java and no new
//! dependency.
//!
//! The atlas is black antialiased ink on opaque white: a pixel's ink
//! coverage is `255 - gray`, which is exactly the alpha the JDK's
//! `AlphaComposite.SrcOver` mask blit received (black over white reduces to
//! `255 - coverage`, see `Canvas::draw_string`). Loading validates that every
//! ink pixel really is neutral (r == g == b) and fails loudly otherwise.
//!
//! `advance` is the manifest's REAL `java.awt.FontMetrics.charWidth`
//! (the integer pen advance `stringWidth` sums), so centring and cursor
//! motion match the original exactly. Each row also carries the source's
//! ascent/descent/leading/maxAdvance/height for consumers that need line
//! metrics.

use std::path::PathBuf;
use std::sync::OnceLock;

/// One font row's line metrics: a `java.awt.FontMetrics` snapshot for a size.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub size: i32,
    pub ascent: i32,
    pub descent: i32,
    pub leading: i32,
    pub max_advance: i32,
    pub height: i32,
}

struct Row {
    metrics: Metrics,
    /// `charWidth(codepoint)` for `codepoint_min..=codepoint_max`.
    advances: Vec<i32>,
    /// `columns * cell_w * cell_h` coverage bytes, row-major per cell.
    cells: Vec<u8>,
}

/// The committed atlas, decoded and indexed by pixel size.
pub struct Atlas {
    pub format: String,
    pub png_width: usize,
    pub png_height: usize,
    pub cell_w: usize,
    pub cell_h: usize,
    /// `drawString`'s x offset inside the cell (manifest `cell_dx_px`).
    pub glyph_dx: i32,
    /// `drawString`'s baseline y inside the cell.
    pub baseline: i32,
    pub columns: usize,
    pub rows: usize,
    pub codepoint_min: u32,
    pub codepoint_max: u32,
    pub glyph_count: usize,
    /// The manifest's declared TTF / PNG / manifest digests (the actual
    /// hashing is the pixel gate's job; the port pins agreement, not SHA-256).
    pub sha256_ttf: String,
    pub ttf: String,
    pub sha256_png: String,
    row_data: Vec<Row>,
}

static ATLAS: OnceLock<Atlas> = OnceLock::new();

/// The process-wide atlas (loaded on first use).
pub fn atlas() -> &'static Atlas {
    ATLAS.get_or_init(Atlas::load)
}

/// The directory the committed atlas fixtures are read from.
pub fn fixture_dir() -> PathBuf {
    Atlas::fixture_dir()
}

impl Atlas {
    /// The row for `size`; panics for a size the original never asks for.
    fn row(&self, size: i32) -> &Row {
        self.row_data
            .iter()
            .find(|r| r.metrics.size == size)
            .unwrap_or_else(|| {
                let sizes: Vec<String> = self
                    .row_data
                    .iter()
                    .map(|r| r.metrics.size.to_string())
                    .collect();
                panic!(
                    "xtfont: no atlas row for {size}pt (the original only asks for {}pt)",
                    sizes.join("/")
                )
            })
    }

    /// The row sizes in manifest order (row 0 first).
    pub fn sizes(&self) -> Vec<i32> {
        self.row_data.iter().map(|r| r.metrics.size).collect()
    }

    /// `java.awt.FontMetrics` for `size`.
    pub fn metrics(&self, size: i32) -> Metrics {
        self.row(size).metrics
    }

    /// `FontMetrics.charWidth(codepoint)` for `size`.
    pub fn advance(&self, size: i32, cp: u32) -> i32 {
        let cp = self.known(cp);
        self.row(size).advances[(cp - self.codepoint_min) as usize]
    }

    /// One glyph cell's `cell_w * cell_h` coverage map (0 transparent,
    /// 255 fully inked), row-major top-down. The cell is the raster of
    /// `drawString(char, cell_x + glyph_dx, cell_y + baseline)`.
    pub fn cell(&self, size: i32, cp: u32) -> &[u8] {
        let cp = self.known(cp);
        let row = self.row(size);
        let col = (cp - self.codepoint_min) as usize;
        let stride = self.cell_w * self.cell_h;
        &row.cells[col * stride..(col + 1) * stride]
    }

    /// Clamp a codepoint into the atlas range. The game's strings are ASCII;
    /// a stray non-ASCII char falls back to `?` (the old stand-in did the
    /// same) instead of silently dropping the pen movement.
    fn known(&self, cp: u32) -> u32 {
        if (self.codepoint_min..=self.codepoint_max).contains(&cp) {
            cp
        } else {
            b'?' as u32
        }
    }

    /// The fixture directory the atlas was read from.
    pub fn fixture_dir() -> PathBuf {
        if let Ok(d) = std::env::var("NFM_XTFONT_ATLAS") {
            return PathBuf::from(d);
        }
        let cands = [
            PathBuf::from("tests/fixtures/xtfont-atlas"),
            PathBuf::from("../../tests/fixtures/xtfont-atlas"),
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/xtfont-atlas"
            )),
        ];
        for c in cands {
            if c.join("atlas.txt").is_file() && c.join("atlas.png").is_file() {
                return c;
            }
        }
        panic!(
            "xtfont: glyph atlas not found (tests/fixtures/xtfont-atlas/atlas.txt+.png); \
             run from the repo root or set NFM_XTFONT_ATLAS"
        )
    }

    /// The manifest text and PNG bytes, in the D2a pack contract's order:
    ///
    /// - **Debug**: the `NFM_XTFONT_ATLAS` override (if set), then the three
    ///   historic filesystem candidates (cwd, crate-dir, compiled-in repo
    ///   path), then the embedded resource pack's
    ///   `tests/fixtures/xtfont-atlas/atlas.{txt,png}`. Without
    ///   `tools/pack.sh` that archive is empty, so the pack step is a no-op.
    /// - **Release**: the pack ONLY — a shipped binary carries its glyph
    ///   atlas and reads no fixture from disk.
    ///
    /// Missing sources panic with the cause (the atlas is process-wide and
    /// load-time fatal, like the rest of this module).
    fn read_sources() -> (String, Vec<u8>) {
        const TXT: &str = "tests/fixtures/xtfont-atlas/atlas.txt";
        const PNG: &str = "tests/fixtures/xtfont-atlas/atlas.png";

        #[cfg(debug_assertions)]
        {
            if let Ok(d) = std::env::var("NFM_XTFONT_ATLAS") {
                let dir = PathBuf::from(d);
                let txt = dir.join("atlas.txt");
                let png = dir.join("atlas.png");
                let text = std::fs::read_to_string(&txt)
                    .unwrap_or_else(|e| panic!("xtfont: cannot read {}: {e}", txt.display()));
                let png_bytes = std::fs::read(&png)
                    .unwrap_or_else(|e| panic!("xtfont: cannot read {}: {e}", png.display()));
                return (text, png_bytes);
            }
            let cands = [
                PathBuf::from("tests/fixtures/xtfont-atlas"),
                PathBuf::from("../../tests/fixtures/xtfont-atlas"),
                PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/xtfont-atlas"
                )),
            ];
            for c in cands {
                let (txt, png) = (c.join("atlas.txt"), c.join("atlas.png"));
                if txt.is_file() && png.is_file() {
                    let text = std::fs::read_to_string(&txt)
                        .unwrap_or_else(|e| panic!("xtfont: cannot read {}: {e}", txt.display()));
                    let png_bytes = std::fs::read(&png)
                        .unwrap_or_else(|e| panic!("xtfont: cannot read {}: {e}", png.display()));
                    return (text, png_bytes);
                }
            }
            if let (Some(text), Some(png_bytes)) = (crate::pack::get(TXT), crate::pack::get(PNG)) {
                let text = String::from_utf8(text.to_vec())
                    .unwrap_or_else(|e| panic!("xtfont: pack manifest is not UTF-8: {e}"));
                return (text, png_bytes.to_vec());
            }
            panic!(
                "xtfont: glyph atlas not found (tests/fixtures/xtfont-atlas/atlas.txt+.png); \
                 run from the repo root, set NFM_XTFONT_ATLAS, or run tools/pack.sh"
            )
        }
        #[cfg(not(debug_assertions))]
        {
            let (Some(text), Some(png_bytes)) = (crate::pack::get(TXT), crate::pack::get(PNG))
            else {
                panic!(
                    "xtfont: the embedded resource pack is missing {TXT} / {PNG}; \
                     rebuild after tools/pack.sh"
                );
            };
            let text = String::from_utf8(text.to_vec())
                .unwrap_or_else(|e| panic!("xtfont: pack manifest is not UTF-8: {e}"));
            (text, png_bytes.to_vec())
        }
    }

    /// Read, validate and index the committed atlas. Panics loudly on any
    /// deviation from the `xtfont-atlas/1` variant or internal disagreement.
    pub fn load() -> Atlas {
        let (text, png_bytes) = Atlas::read_sources();
        let mut keys: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut glyph_rows: Vec<(i32, u32, i32, i32)> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("glyph ") {
                let f: Vec<&str> = rest.split_ascii_whitespace().collect();
                if f.len() != 4 {
                    panic!("xtfont: bad glyph row {line:?} (want 4 fields)");
                }
                let pt: i32 = f[0]
                    .parse()
                    .unwrap_or_else(|_| panic!("xtfont: bad size in {line:?}"));
                let cp: u32 = f[1]
                    .parse()
                    .unwrap_or_else(|_| panic!("xtfont: bad codepoint in {line:?}"));
                let adv: i32 = f[2]
                    .parse()
                    .unwrap_or_else(|_| panic!("xtfont: bad advance in {line:?}"));
                let dx: i32 = f[3]
                    .parse()
                    .unwrap_or_else(|_| panic!("xtfont: bad cell_dx in {line:?}"));
                glyph_rows.push((pt, cp, adv, dx));
            } else if let Some((k, v)) = line.split_once('=') {
                keys.insert(k.to_string(), v.to_string());
            } else {
                panic!("xtfont: unknown manifest line {line:?}");
            }
        }

        let get = |k: &str| -> &str {
            keys.get(k)
                .map(String::as_str)
                .unwrap_or_else(|| panic!("xtfont: manifest is missing {k}"))
        };
        let int = |k: &str| -> i32 {
            get(k)
                .parse()
                .unwrap_or_else(|_| panic!("xtfont: {k} is not an integer: {:?}", get(k)))
        };

        let format = get("format").to_string();
        if format != "xtfont-atlas/1" {
            panic!("xtfont: unsupported manifest format {format:?} (want xtfont-atlas/1)");
        }
        let png_width = int("png_width");
        let png_height = int("png_height");
        let cell_w = int("cell_width");
        let cell_h = int("cell_height");
        let grid_x = int("grid_x");
        let grid_y = int("grid_y");
        let columns = int("columns");
        let rows = int("rows");
        let cp_min = int("codepoint_min");
        let cp_max = int("codepoint_max");
        let baseline = int("baseline");
        let glyph_count = int("glyph_count");
        if columns <= 0 || rows <= 0 || cell_w <= 0 || cell_h <= 0 {
            panic!(
                "xtfont: non-positive atlas geometry {columns}x{rows} cells of {cell_w}x{cell_h}"
            );
        }
        if cp_max - cp_min + 1 != columns {
            panic!("xtfont: codepoints {cp_min}..={cp_max} do not fill {columns} columns");
        }
        if glyph_count != columns * rows {
            panic!("xtfont: glyph_count {glyph_count} != {columns} columns x {rows} rows");
        }
        if glyph_rows.len() != glyph_count as usize {
            panic!(
                "xtfont: {} glyph rows in the manifest, want {glyph_count}",
                glyph_rows.len()
            );
        }
        if grid_x < 0 || grid_y < 0 {
            panic!("xtfont: negative grid origin {grid_x},{grid_y}");
        }
        let need_w = grid_x + columns * cell_w;
        let need_h = grid_y + rows * cell_h;
        if png_width != need_w || png_height != need_h {
            panic!("xtfont: png is {png_width}x{png_height}, geometry needs {need_w}x{need_h}");
        }

        // The PNG: the one supported variant (8-bit RGB, non-interlaced).
        let img = crate::png::decode(&png_bytes);
        if img.w != png_width as usize || img.h != png_height as usize {
            panic!(
                "xtfont: atlas.png decodes to {}x{}, manifest says {png_width}x{png_height}",
                img.w, img.h
            );
        }

        // `cell_dx` is uniform (the manifest repeats it on every glyph row).
        let glyph_dx = glyph_rows[0].3;
        for g in &glyph_rows {
            if g.3 != glyph_dx {
                panic!(
                    "xtfont: glyph rows disagree on cell_dx ({} vs {glyph_dx})",
                    g.3
                );
            }
            if g.1 < cp_min as u32 || g.1 > cp_max as u32 {
                panic!("xtfont: codepoint {} outside the atlas range", g.1);
            }
        }

        let mut row_data: Vec<Row> = Vec::new();
        let mut sizes: Vec<i32> = Vec::new();
        for r in 0..rows as usize {
            let size = int(&format!("row{r}_size"));
            let metrics = Metrics {
                size,
                ascent: int(&format!("row{r}_ascent")),
                descent: int(&format!("row{r}_descent")),
                leading: int(&format!("row{r}_leading")),
                max_advance: int(&format!("row{r}_max_advance")),
                height: int(&format!("row{r}_height")),
            };
            if sizes.contains(&size) {
                panic!("xtfont: duplicate size {size}pt in the manifest");
            }
            let mut advances = vec![i32::MIN; columns as usize];
            for &(pt, cp, adv, _dx) in &glyph_rows {
                if pt == size {
                    let col = (cp - cp_min as u32) as usize;
                    if advances[col] != i32::MIN {
                        panic!("xtfont: duplicate glyph {size}pt cp {cp}");
                    }
                    if adv < 0 {
                        panic!("xtfont: negative advance {adv} for {size}pt cp {cp}");
                    }
                    advances[col] = adv;
                }
            }
            if advances.contains(&i32::MIN) {
                panic!("xtfont: size {size}pt is missing a glyph advance");
            }
            // Cut every cell out of the decoded raster as ink coverage.
            let mut cells = vec![0u8; (columns * cell_w * cell_h) as usize];
            for col in 0..columns as usize {
                let ox = grid_x as usize + col * cell_w as usize;
                for cy in 0..cell_h as usize {
                    let oy = grid_y as usize + r * cell_h as usize + cy;
                    for cx in 0..cell_w as usize {
                        let p = (oy * img.w + ox + cx) * 3;
                        let (rr, gg, bb) = (img.rgb[p], img.rgb[p + 1], img.rgb[p + 2]);
                        if rr != gg || rr != bb {
                            panic!(
                                "xtfont: atlas ink is not neutral at ({},{}) = ({rr},{gg},{bb}); \
                                 the atlas must be black-on-white antialiasing",
                                ox + cx,
                                oy
                            );
                        }
                        let cov = 255 - rr;
                        let at = (col * cell_h as usize + cy) * cell_w as usize + cx;
                        cells[at] = cov;
                    }
                }
            }
            sizes.push(size);
            row_data.push(Row {
                metrics,
                advances,
                cells,
            });
        }

        Atlas {
            format,
            png_width: png_width as usize,
            png_height: png_height as usize,
            cell_w: cell_w as usize,
            cell_h: cell_h as usize,
            glyph_dx,
            baseline,
            columns: columns as usize,
            rows: rows as usize,
            codepoint_min: cp_min as u32,
            codepoint_max: cp_max as u32,
            glyph_count: glyph_count as usize,
            sha256_ttf: get("sha256_ttf").to_string(),
            ttf: get("ttf").to_string(),
            sha256_png: get("sha256_png").to_string(),
            row_data,
        }
    }
}

/// `FontMetrics.stringWidth(s)` for the atlas row `size`.
pub fn string_width(s: &str, size: i32) -> i32 {
    let a = atlas();
    s.chars().map(|ch| a.advance(size, ch as u32)).sum()
}

/// The atlas row's `java.awt.FontMetrics` values for `size`.
pub fn metrics(size: i32) -> Metrics {
    atlas().metrics(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_committed_atlas() {
        let a = atlas();
        assert_eq!(a.format, "xtfont-atlas/1");
        assert_eq!((a.png_width, a.png_height), (1900, 44));
        assert_eq!((a.cell_w, a.cell_h), (20, 22));
        assert_eq!((a.columns, a.rows), (95, 2));
        assert_eq!((a.codepoint_min, a.codepoint_max), (32, 126));
        assert_eq!(a.glyph_count, 190);
        assert_eq!((a.glyph_dx, a.baseline), (1, 16));
        assert_eq!(
            a.cell(13, b' ' as u32).iter().filter(|&&c| c != 0).count(),
            0
        );
        assert!(a.cell(13, b'A' as u32).iter().any(|&c| c > 0), "A inks");
        // the manifest's real charWidth values
        assert_eq!(a.advance(13, b'A' as u32), 9);
        assert_eq!(a.advance(13, b' ' as u32), 4);
        assert_eq!(a.advance(11, b'A' as u32), 8);
        assert_eq!(a.advance(11, b' ' as u32), 3);
        assert_eq!(string_width("AB", 13), 18);
        assert_eq!(string_width("AB", 11), 15);
    }

    #[test]
    fn row_metrics_are_the_java_fontmetrics() {
        let a = atlas();
        let m13 = a.metrics(13);
        assert_eq!(
            (
                m13.ascent,
                m13.descent,
                m13.leading,
                m13.max_advance,
                m13.height
            ),
            (12, 3, 1, 17, 16)
        );
        let m11 = a.metrics(11);
        assert_eq!(
            (
                m11.ascent,
                m11.descent,
                m11.leading,
                m11.max_advance,
                m11.height
            ),
            (10, 3, 0, 15, 13)
        );
    }

    #[test]
    fn space_and_ink_are_coverage_maps() {
        let a = atlas();
        // a fully inked pixel (coverage 255) exists somewhere in 'A'
        let cell = a.cell(13, b'A' as u32);
        assert!(cell.contains(&255), "A has an opaque core");
        // and glyph borders are empty: the generator rejects bleed
        let w = a.cell_w;
        for cp in a.codepoint_min..=a.codepoint_max {
            let cell = a.cell(13, cp);
            for x in 0..w {
                assert_eq!(cell[x], 0, "top border cp {cp} x {x}");
                assert_eq!(cell[(a.cell_h - 1) * w + x], 0, "bottom border cp {cp}");
            }
            for y in 0..a.cell_h {
                assert_eq!(cell[y * w], 0, "left border cp {cp} y {y}");
                assert_eq!(cell[y * w + w - 1], 0, "right border cp {cp} y {y}");
            }
        }
    }
}
