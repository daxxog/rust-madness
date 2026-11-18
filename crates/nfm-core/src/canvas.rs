//! The drawing surface behind the `Graphics` calls the original engine
//! makes (`fillPolygon`, `drawPolygon`, `setColor`), rendering into a plain
//! RGB pixel buffer that can be written out as a BMP.
//!
//! Fidelity note: `fillPolygon` ports the JDK 8 scan converter behind
//! `Graphics.fillPolygon` (`ShapeSpanIterator` + `FillSpans`, see the
//! private `spanfill` module), so its spans are byte-identical to Oracle
//! 8u40; `drawPolygon` still walks the integer Bresenham in `line`, whose
//! endpoints and axis-aligned edges match AWT exactly but whose diagonal
//! minor-bump rule is a stand-in. The rect and round-rect paths delegate to
//! the `roundrect` engine, byte-exact against era AWT (see its test module).

use crate::gif::Gif;
use crate::roundrect::PixelSink;

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    /// RGB triples, row-major top-down.
    pub px: Vec<u8>,
    /// The **draw-call recorder** (mirror batch 10's Rec seam): every
    /// primitive below appends the exact event string `TxtGraphicsLogic`'s
    /// `Rec extends Graphics` records (`"fillRect x,y,w,h rrggbb"`,
    /// `"str \"s\" x,y rrggbb"`, `"fillPoly [xs] [ys] rrggbb"`,
    /// `"img WxH x,y"`, ...). The mirror reads the log *after* driving the
    /// real production draw code, so its pins are observations of this
    /// canvas, not a test-side re-implementation. Test-only: production
    /// builds carry no field and no branch (the mirror suite is
    /// `cfg(test)`), so the renderer's hot paths are unchanged.
    #[cfg(test)]
    pub log: Vec<String>,
}

/// Adapter exposing the private pixel setter to the `roundrect` engine
/// through its `PixelSink` trait.
struct ShapeSink<'a> {
    cv: &'a mut Canvas,
    c: [u8; 3],
}

impl crate::roundrect::PixelSink for ShapeSink<'_> {
    fn pixel(&mut self, x: i32, y: i32) {
        self.cv.set(x, y, self.c);
    }

    fn span(&mut self, x0: i32, x1: i32, y: i32) {
        // The engine emits x0 <= x1; clip against the canvas like `set`.
        for x in x0..=x1 {
            self.cv.set(x, y, self.c);
        }
    }
}

/// `MUL8(a, b)` — `sun/java2d/loops/AlphaMath.c`'s `mul8table[a][b]`, i.e.
/// `(a*b + 127) / 255` (round-half-up; the table is built from
/// `inc + (1 << 23)` scaled by `1 << 24`, which is this rounding).
fn mul8(a: u8, b: u8) -> u8 {
    ((a as u16 * b as u16 + 127) / 255) as u8
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            px: vec![0; w * h * 3],
            #[cfg(test)]
            log: Vec::new(),
        }
    }

    /// Append one `Rec` event (test-only, see the `log` field).
    #[cfg(test)]
    fn ev(&mut self, s: String) {
        self.log.push(s);
    }

    /// `snap_channel`-free colour spelling of the recorder: `rrggbb`.
    #[cfg(test)]
    fn hex(c: [u8; 3]) -> String {
        format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    }

    /// The recorder's bracket spelling of one polygon coordinate array.
    #[cfg(test)]
    fn coords(a: &[i32]) -> String {
        let mut b = String::from("[");
        for (i, v) in a.iter().enumerate() {
            if i > 0 {
                b.push(',');
            }
            b.push_str(&v.to_string());
        }
        b.push(']');
        b
    }

    pub fn clear(&mut self) {
        self.px.fill(0);
    }

    /// `Graphics.setColor(c); Graphics.fillRect(0, 0, w, h)` — the
    /// full-canvas flash the shell paints over a finished frame (the
    /// crash-cam fade-in and the white swings).
    pub fn fill(&mut self, c: [u8; 3]) {
        #[cfg(test)]
        self.ev(format!(
            "fillRect 0,0,{},{} {}",
            self.w,
            self.h,
            Canvas::hex(c)
        ));
        for px in self.px.chunks_exact_mut(3) {
            px[0] = c[0];
            px[1] = c[1];
            px[2] = c[2];
        }
    }

    fn set(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let at = ((y as usize) * self.w + x as usize) * 3;
        self.px[at] = c[0];
        self.px[at + 1] = c[1];
        self.px[at + 2] = c[2];
    }

    /// `Graphics.fillPolygon(xs, ys, n)` — the JDK 8 path
    /// `SunGraphics2D.fillPolygon` → `LoopPipe.fillPolygon` →
    /// `ShapeSpanIterator.appendPoly` + the `FillSpans` loop, i.e. the
    /// even-odd span tracer in this file's `spanfill` module. That engine
    /// normalizes integer vertices to the nearest (0.25, 0.25) subpixel
    /// position (`getFillSSI`'s `adjust = true` for the default
    /// `BasicStroke`) and samples every horizontal pixel center.
    ///
    /// The old stand-in even-odd scanline disagreed at vertex rows; TPlane's
    /// spark polygon pin (exact 35-pixel count, Plane.java:257-258) is the
    /// pin that forced this port.
    pub fn fill_polygon(&mut self, xs: &[i32], ys: &[i32], c: [u8; 3]) {
        let n = xs.len().min(ys.len());
        if n < 2 {
            return;
        }
        #[cfg(test)]
        self.ev(format!(
            "fillPoly {} {} {}",
            Canvas::coords(&xs[..n]),
            Canvas::coords(&ys[..n]),
            Canvas::hex(c)
        ));
        let (cw, ch) = (self.w as i32, self.h as i32);
        let mut ssi = spanfill::ShapeSpanIterator::new(0, 0, cw, ch);
        ssi.append_poly(xs, ys, n);
        let mut sink = ShapeSink { cv: self, c };
        while let Some((x0, y, x1)) = ssi.next_span() {
            sink.span(x0, x1 - 1, y);
        }
    }

    /// Integer Bresenham line with both endpoints (1.1-era `drawLine` did).
    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 3]) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.set(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// `Graphics.drawPolygon(xs, ys, n)` — closed outline through all points.
    pub fn draw_polygon(&mut self, xs: &[i32], ys: &[i32], c: [u8; 3]) {
        let n = xs.len().min(ys.len());
        if n < 2 {
            if n == 1 {
                self.set(xs[0], ys[0], c);
            }
            return;
        }
        #[cfg(test)]
        self.ev(format!(
            "drawPoly {} {} {}",
            Canvas::coords(&xs[..n]),
            Canvas::coords(&ys[..n]),
            Canvas::hex(c)
        ));
        for e in 0..n {
            let a = e;
            let b = (e + 1) % n;
            self.line(xs[a], ys[a], xs[b], ys[b], c);
        }
    }

    /// `Graphics.drawImage(img, x, y, null)` — the applet's sprite blit.
    /// Every pixel the engine ever blits is fully opaque or fully
    /// transparent (GIF palettes and the loadsnap/`MemoryImageSource`
    /// round-trip only produce alpha 0 or 255), so the 1.1-era source-over
    /// composite reduces to: skip transparent pixels, overwrite opaque
    /// ones. Pixels outside the canvas are clipped.
    pub fn draw_image(&mut self, img: &Gif, x: i32, y: i32) {
        // The recorder's spelling of a Java `drawImage(null, ...)`: an
        // unfetched sprite is the port's empty gif (w == h == 0), which
        // draws nothing exactly like the null image's silent blit.
        #[cfg(test)]
        {
            let dim = if img.w == 0 && img.h == 0 {
                "null".to_string()
            } else {
                format!("{}x{}", img.w, img.h)
            };
            self.ev(format!("img {dim} {x},{y}"));
        }
        for sy in 0..img.h {
            let dy = y + sy as i32;
            if dy < 0 || dy >= self.h as i32 {
                continue;
            }
            for sx in 0..img.w {
                let a = img.px[sy * img.w + sx];
                if a >> 24 == 0 {
                    continue;
                }
                let dx = x + sx as i32;
                if dx < 0 || dx >= self.w as i32 {
                    continue;
                }
                let at = (dy as usize * self.w + dx as usize) * 3;
                self.px[at] = (a >> 16) as u8;
                self.px[at + 1] = (a >> 8) as u8;
                self.px[at + 2] = a as u8;
            }
        }
    }

    /// `Graphics.fillRect(x, y, w, h)` — w×h pixels.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        #[cfg(test)]
        self.ev(format!("fillRect {x},{y},{w},{h} {}", Canvas::hex(c)));
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    /// `Graphics.drawRect(x, y, w, h)` — the AWT outline is `w+1` × `h+1`
    /// pixels (the right/bottom edges land on x+w / y+h, inclusive).
    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for xx in x..=x + w {
            self.set(xx, y, c);
            self.set(xx, y + h, c);
        }
        for yy in y..=y + h {
            self.set(x, yy, c);
            self.set(x + w, yy, c);
        }
    }

    /// `Graphics.fillRect` is routed through below; these routines are the
    /// JDK pipeline behind `fillRoundRect` / `drawRoundRect` (the menu
    /// buttons and dialogs). They go through the full JDK `ProcessPath`
    /// engine in `roundrect`, byte-identical to an untransformed
    /// JDK 1.8.0_40 `BufferedImage.TYPE_INT_RGB` target.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_round_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        aw: i32,
        ah: i32,
        c: [u8; 3],
    ) {
        #[cfg(test)]
        self.ev(format!("fillRR {x},{y},{w},{h} {}", Canvas::hex(c)));
        let (cw, ch) = (self.w as i32, self.h as i32);
        if let Some(path) = crate::roundrect::round_rect_path(x, y, w, h, aw, ah) {
            let mut sink = ShapeSink { cv: self, c };
            crate::roundrect::fill_path(&mut sink, &path, cw, ch);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_round_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        aw: i32,
        ah: i32,
        c: [u8; 3],
    ) {
        #[cfg(test)]
        self.ev(format!("drawRR {x},{y},{w},{h} {}", Canvas::hex(c)));
        let (cw, ch) = (self.w as i32, self.h as i32);
        if let Some(path) = crate::roundrect::round_rect_path(x, y, w, h, aw, ah) {
            let mut sink = ShapeSink { cv: self, c };
            crate::roundrect::draw_path(&mut sink, &path, cw, ch);
        }
    }

    /// `Graphics.drawLine(x1, y1, x2, y2)` — the JDK `DrawLine` native
    /// (outcode-clipped Bresenham with the fused minor bump), straight
    /// to the untransformed INT_RGB surface.
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, c: [u8; 3]) {
        let (cw, ch) = (self.w as i32, self.h as i32);
        let mut sink = ShapeSink { cv: self, c };
        crate::roundrect::draw_line(&mut sink, x1, y1, x2, y2, cw, ch);
    }

    /// `Graphics.drawString(s, x, y)` at the Java font `size` (11 or 13 —
    /// the only two the original asks for). `(x, y)` is the AWT baseline;
    /// the atlas cell is `xtfont::atlas().cell(size, cp)` and was rendered
    /// with `drawString(char, cell_x + glyph_dx, cell_y + baseline)`, so the
    /// cell's left edge lands at `x - glyph_dx` and its top at `y - baseline`.
    ///
    /// Compositing is Java's exact opaque-destination SRC_OVER for a solid
    /// colour through an 8-bit coverage mask. `sun/java2d/loops/
    /// AlphaMacros.h` (`DEFINE_ALPHA_MASKBLIT`, SRC_OVER) reduces to
    /// `res = MUL8(mask, src); res += MUL8(MaxVal-mask, dst)` (`MUL8` from
    /// `AlphaMath.c`'s `initAlphaTables`: `(a*b + 127) / 255`, round-half-up
    /// — the table's `inc + (1 << 23)` construction is that rounding), and
    /// the coverage mask for black ink on white is exactly `255 - gray`.
    pub fn draw_string(&mut self, s: &str, x: i32, y: i32, c: [u8; 3], size: i32) {
        #[cfg(test)]
        self.ev(format!("str \"{s}\" {x},{y} {}", Canvas::hex(c)));
        let a = crate::xtfont::atlas();
        let mut pen = x;
        for ch in s.chars() {
            if ch == '\n' {
                break; // the source never feeds multi-line strings here
            }
            let cp = ch as u32;
            let cell = a.cell(size, cp);
            let ox = pen - a.glyph_dx;
            let oy = y - a.baseline;
            for ry in 0..a.cell_h {
                let dy = oy + ry as i32;
                if dy < 0 || dy >= self.h as i32 {
                    continue;
                }
                for rx in 0..a.cell_w {
                    let cov = cell[ry * a.cell_w + rx];
                    if cov == 0 {
                        continue;
                    }
                    let dx = ox + rx as i32;
                    if dx < 0 || dx >= self.w as i32 {
                        continue;
                    }
                    let at = (dy as usize * self.w + dx as usize) * 3;
                    for (k, &ink) in c.iter().enumerate() {
                        let bg = self.px[at + k];
                        let mixed = mul8(cov, ink) as u16 + mul8(255 - cov, bg) as u16;
                        self.px[at + k] = mixed.min(255) as u8;
                    }
                }
            }
            pen += a.advance(size, cp);
        }
    }

    /// Writes the buffer as a 24-bit BMP (bottom-up BGR rows, 4-byte padded).
    pub fn save_bmp(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        let row_bytes = (self.w * 3).div_ceil(4) * 4;
        let data_size = row_bytes * self.h;
        let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);

        // BITMAPFILEHEADER (14 bytes)
        f.write_all(b"BM")?;
        f.write_all(&((54 + data_size) as u32).to_le_bytes())?;
        f.write_all(&0u32.to_le_bytes())?;
        f.write_all(&54u32.to_le_bytes())?;

        // BITMAPINFOHEADER (40 bytes)
        f.write_all(&40u32.to_le_bytes())?;
        f.write_all(&(self.w as i32).to_le_bytes())?;
        f.write_all(&(self.h as i32).to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?;
        f.write_all(&24u16.to_le_bytes())?;
        f.write_all(&0u32.to_le_bytes())?; // uncompressed
        f.write_all(&(data_size as u32).to_le_bytes())?;
        f.write_all(&2835u32.to_le_bytes())?;
        f.write_all(&2835u32.to_le_bytes())?;
        f.write_all(&0u32.to_le_bytes())?;
        f.write_all(&0u32.to_le_bytes())?;

        // pixel rows, bottom-up, BGR
        let pad = row_bytes - self.w * 3;
        for y in (0..self.h).rev() {
            for x in 0..self.w {
                let at = (y * self.w + x) * 3;
                f.write_all(&[self.px[at + 2], self.px[at + 1], self.px[at]])?;
            }
            f.write_all(&vec![0u8; pad])?;
        }
        Ok(())
    }
}

/// The JDK 8 scan converter behind `Graphics.fillPolygon`:
/// `sun.java2d.pipe.ShapeSpanIterator`'s native core
/// (`jdk/src/share/native/sun/java2d/pipe/ShapeSpanIterator.c`), specialized
/// to the `appendPoly` entry point the fill-polygon loop uses.
///
/// The algorithm (all of it transliterated from that C file, preserving its
/// f32 coordinate flow and i32/jlong error arithmetic):
/// - `appendPoly` adds the (0.25, 0.25) normalization offset
///   (`getFillSSI(sg2d)`'s `adjust == true` for the default `BasicStroke`),
///   drops horizontal edges (tracking their x movement), clips each edge's
///   outcode pair, and calls `appendSegment`.
/// - `appendSegment` traces the segment at every horizontal pixel-center
///   (HPC) row: `istarty = ceil(y0 - 0.5)`, a start x at that HPC, and the
///   `bumpx`/`bumperr`/`error` fixed-point stepper.
/// - `initSegmentTable` sorts segments by `(cury, curx, lasty)` and skips
///   those ending at/above the top clip edge.
/// - `ShapeSINextSpan` walks scanlines, retires/accepts segments, steps the
///   active x coordinates, insertion-sorts by x, and emits even-odd
///   `[x0, x1)` spans (`appendPoly` sets `evenodd = true`).
///
/// This is deliberately separate from `roundrect`'s `ProcessPath` engine:
/// `fillRoundRect` and `fillPolygon` take different JDK pipelines, and the
/// TPlane spark pin (35 pixels) only matches this one.
mod spanfill {
    const ERR_STEP_MAX: i32 = 0x7fff_ffff;

    /// Outcode bits (`ShapeSpanIterator.c` `OUT_*`).
    const OUT_XLO: i32 = 1;
    const OUT_XHI: i32 = 2;
    const OUT_YLO: i32 = 4;
    const OUT_YHI: i32 = 8;

    /// `segmentData`.
    #[derive(Clone, Copy)]
    struct Segment {
        curx: i32,
        cury: i32,
        lasty: i32,
        error: i32,
        bumpx: i32,
        bumperr: i32,
    }

    /// `pathData`'s span-iteration half.
    pub(super) struct ShapeSpanIterator {
        segs: Vec<Segment>,
        table: Vec<usize>,
        lo: usize,
        cur: usize,
        hi: usize,
        loy: i32,
        started: bool,
        lox: i32,
        loy_clip: i32,
        hix: i32,
        hiy: i32,
    }

    /// `FRACTTOJINT(f)` — `(jint)((f) * (double) ERRSTEP_MAX)`.
    fn fracttojint(v: f64) -> i32 {
        (v * ERR_STEP_MAX as f64) as i32
    }

    impl ShapeSpanIterator {
        pub(super) fn new(lox: i32, loy: i32, hix: i32, hiy: i32) -> ShapeSpanIterator {
            ShapeSpanIterator {
                segs: Vec::new(),
                table: Vec::new(),
                lo: 0,
                cur: 0,
                hi: 0,
                loy: loy - 1,
                started: false,
                lox,
                loy_clip: loy,
                hix,
                hiy,
            }
        }

        /// `CALCULATE_OUTCODES`.
        fn outcode(&self, x: f32, y: f32) -> i32 {
            let mut outc = if y <= self.loy_clip as f32 {
                OUT_YLO
            } else if y >= self.hiy as f32 {
                OUT_YHI
            } else {
                0
            };
            if x <= self.lox as f32 {
                outc |= OUT_XLO;
            } else if x >= self.hix as f32 {
                outc |= OUT_XHI;
            }
            outc
        }

        /// `appendPoly(xPoints, yPoints, nPoints, xoff=0, yoff=0)` with the
        /// JDK's normalization adjustment (`adjust == true`).
        pub(super) fn append_poly(&mut self, xs: &[i32], ys: &[i32], n: usize) {
            if n == 0 {
                return;
            }
            // `if (pd->adjust) { xoff += 0.25f; yoff += 0.25f; }`
            let xoff = 0.25f32;
            let yoff = 0.25f32;

            let mut curx = xs[0] as f32 + xoff;
            let mut cury = ys[0] as f32 + yoff;
            let movx = curx;
            let movy = cury;
            let mut outc0 = self.outcode(curx, cury);

            for i in 1..n {
                let x = xs[i] as f32 + xoff;
                let y = ys[i] as f32 + yoff;
                if y == cury {
                    // Horizontal segment - do not append; track x movement.
                    if x != curx {
                        outc0 = self.outcode(x, y);
                        curx = x;
                    }
                    continue;
                }
                let outc1 = self.outcode(x, y);
                outc0 &= outc1;
                if outc0 == 0 {
                    self.append_segment(curx, cury, x, y);
                } else if outc0 == OUT_XLO {
                    self.append_segment(self.lox as f32, cury, self.lox as f32, y);
                }
                outc0 = outc1;
                curx = x;
                cury = y;
            }
            // HANDLEENDPATH -> HANDLECLOSE -> subdivideLine
            if curx != movx || cury != movy {
                self.subdivide_line(curx, cury, movx, movy);
            }
        }

        /// `subdivideLine` — the trivial rejections / left-edge collapse
        /// that precede `appendSegment`.
        fn subdivide_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
            let (minx, maxx) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
            let (miny, maxy) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            if maxy <= self.loy_clip as f32 || miny >= self.hiy as f32 || minx >= self.hix as f32 {
                return;
            }
            if maxx <= self.lox as f32 {
                self.append_segment(maxx, y0, maxx, y1);
            } else {
                self.append_segment(x0, y0, x1, y1);
            }
        }

        /// `appendSegment` — one HPC-stepped segment.
        fn append_segment(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
            let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
            if y0 > y1 {
                std::mem::swap(&mut x0, &mut x1);
                std::mem::swap(&mut y0, &mut y1);
            }
            // `istarty = (jint) ceil(y0 - 0.5f)` — the f32 subtraction is
            // promoted to double for ceil.
            let istarty = ((y0 - 0.5f32) as f64).ceil() as i32;
            let ilasty = ((y1 - 0.5f32) as f64).ceil() as i32;
            if istarty >= ilasty || istarty >= self.hiy || ilasty <= self.loy_clip {
                return;
            }

            let dx = x1 - x0;
            let dy = y1 - y0;
            let slope = dx / dy;

            // First HPC bump and its x sample.
            let ystartbump = istarty as f32 + 0.5f32 - y0;
            x0 += ystartbump * dx / dy;
            let istartx = ((x0 - 0.5f32) as f64).ceil() as i32;
            let bumpx = (slope as f64).floor() as i32;
            let bumperr = fracttojint(slope as f64 - (slope as f64).floor());
            let error = fracttojint((x0 - (istartx as f32 - 0.5f32)) as f64);

            self.segs.push(Segment {
                curx: istartx,
                cury: istarty,
                lasty: ilasty,
                error,
                bumpx,
                bumperr,
            });
        }

        /// `initSegmentTable`.
        fn init(&mut self) {
            let mut table: Vec<usize> = (0..self.segs.len()).collect();
            table.sort_unstable_by(|&a, &b| {
                let sa = &self.segs[a];
                let sb = &self.segs[b];
                sa.cury
                    .cmp(&sb.cury)
                    .then(sa.curx.cmp(&sb.curx))
                    .then(sa.lasty.cmp(&sb.lasty))
            });
            let mut c = 0;
            while c < table.len() && self.segs[table[c]].lasty <= self.loy_clip {
                c += 1;
            }
            self.table = table;
            self.lo = c;
            self.cur = c;
            self.hi = c;
            self.loy = self.loy_clip - 1;
            self.started = true;
        }

        /// `ShapeSINextSpan` — one `[x0, x1)` span on scanline `y`.
        pub(super) fn next_span(&mut self) -> Option<(i32, i32, i32)> {
            if !self.started {
                self.init();
            }
            let num = self.segs.len();
            let mut lo = self.lo;
            let mut cur = self.cur;
            let mut hi = self.hi;
            let mut loy = self.loy;

            while lo < num {
                if cur < hi {
                    let mut x0 = self.segs[self.table[cur]].curx;
                    if x0 >= self.hix {
                        cur = hi;
                        continue;
                    }
                    if x0 < self.lox {
                        x0 = self.lox;
                    }
                    // appendPoly always sets the even-odd rule.
                    cur += 2;
                    let mut x1 = if cur <= hi {
                        self.segs[self.table[cur - 1]].curx
                    } else {
                        self.hix
                    };
                    if x1 > self.hix {
                        x1 = self.hix;
                    }
                    if x1 <= x0 {
                        continue;
                    }
                    self.lo = lo;
                    self.cur = cur;
                    self.hi = hi;
                    self.loy = loy;
                    return Some((x0, loy, x1));
                }

                loy += 1;
                if loy >= self.hiy {
                    lo = num;
                    cur = num;
                    hi = num;
                    break;
                }

                // Retire segments whose x range ends on/above this line.
                cur = hi;
                let mut new = hi;
                while cur > lo {
                    cur -= 1;
                    let idx = self.table[cur];
                    if self.segs[idx].lasty > loy {
                        new -= 1;
                        self.table[new] = idx;
                    }
                }
                lo = new;
                if lo == hi && lo < num {
                    // Empty active list: jump to the next segment set, but
                    // never backwards.
                    let next_y = self.segs[self.table[lo]].cury;
                    if loy < next_y {
                        loy = next_y;
                    }
                }

                // Accept segments starting on this line.
                while hi < num && self.segs[self.table[hi]].cury <= loy {
                    hi += 1;
                }

                // Step every active segment's x to this scanline and sort.
                for c in lo..hi {
                    let idx = self.table[c];
                    let mut x0 = self.segs[idx].curx;
                    let mut y0 = self.segs[idx].cury;
                    let err0 = self.segs[idx].error;
                    let bumpx = self.segs[idx].bumpx;
                    let bumperr = self.segs[idx].bumperr;
                    let mut err = err0;
                    y0 += 1;
                    if y0 == loy {
                        x0 = x0.wrapping_add(bumpx);
                        err = err.wrapping_add(bumperr);
                        x0 = x0.wrapping_sub(err >> 31);
                        err &= ERR_STEP_MAX;
                    } else {
                        // `steps` is a jlong in the C; `y0` was already
                        // incremented, so `steps == loy - cury`.
                        let steps = (loy - (y0 - 1)) as i64;
                        y0 = loy;
                        x0 = x0.wrapping_add((steps * bumpx as i64) as i32);
                        let st = err as i64 + steps * bumperr as i64;
                        x0 = x0.wrapping_add((st >> 31) as i32);
                        err = (st as i32) & ERR_STEP_MAX;
                    }
                    self.segs[idx].curx = x0;
                    self.segs[idx].cury = y0;
                    self.segs[idx].error = err;

                    // Insertion sort by x0.
                    let mut n = c;
                    while n > lo {
                        let prev = self.table[n - 1];
                        if self.segs[prev].curx <= x0 {
                            break;
                        }
                        self.table[n] = prev;
                        n -= 1;
                    }
                    self.table[n] = idx;
                }
                cur = lo;
            }

            self.lo = lo;
            self.cur = cur;
            self.hi = hi;
            self.loy = loy;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_square() {
        let mut cv = Canvas::new(10, 10);
        // axis-aligned square (1,1),(1,5),(5,5),(5,1): the JDK FillPath
        // lane fills x 1..=4, y 1..=4 (a filled quad's right/bottom integer
        // edges are exclusive; measured on Oracle 8u40, TPlane batch).
        cv.fill_polygon(&[1, 1, 5, 5], &[1, 5, 5, 1], [255, 0, 0]);
        for y in 1..=4 {
            for x in 1..=4 {
                let at = (y * 10 + x) * 3;
                assert_eq!(&cv.px[at..at + 3], &[255, 0, 0], "at {x},{y}");
            }
        }
        // the edge columns/rows and the corner outside stay untouched:
        // (x=5,y=1) at 1*10+5, (x=4,y=5) at 5*10+4
        assert_eq!(&cv.px[45..48], &[0, 0, 0]);
        assert_eq!(&cv.px[162..165], &[0, 0, 0]);
        assert_eq!(&cv.px[0..3], &[0, 0, 0]);
    }

    #[test]
    fn fills_triangle_span() {
        let mut cv = Canvas::new(6, 6);
        cv.fill_polygon(&[1, 4, 1], &[1, 4, 4], [0, 255, 0]);
        // Oracle 8u40 fills exactly (1,2) and (1,3),(2,3): 3 pixels.
        let count = cv
            .px
            .chunks_exact(3)
            .filter(|p| *p == [0u8, 255, 0])
            .count();
        assert_eq!(count, 3, "count {count}");
        assert_eq!(px_at(&cv, 1, 2), [0, 255, 0]);
        assert_eq!(px_at(&cv, 1, 3), [0, 255, 0]);
        assert_eq!(px_at(&cv, 2, 3), [0, 255, 0]);
        assert_eq!(px_at(&cv, 0, 3), [0, 0, 0]);
        assert_eq!(px_at(&cv, 3, 3), [0, 0, 0]);
    }

    #[test]
    fn fill_polygon_matches_the_jdk_span_iterator() {
        // Oracle 8u40 `ShapeSpanIterator` span dumps for the five cases
        // below (TPlane batch validation), including clipping and the
        // even-odd self-intersection arm.
        fn spans(cv: &Canvas) -> Vec<(i32, i32, i32)> {
            let mut out = Vec::new();
            for y in 0..cv.h as i32 {
                let mut x = 0;
                while x < cv.w as i32 {
                    let at = ((y as usize) * cv.w + x as usize) * 3;
                    if cv.px[at] != 1 {
                        x += 1;
                        continue;
                    }
                    let x0 = x;
                    while x < cv.w as i32 {
                        let at = ((y as usize) * cv.w + x as usize) * 3;
                        if cv.px[at] != 1 {
                            break;
                        }
                        x += 1;
                    }
                    out.push((x0, y, x));
                }
            }
            out
        }

        type Case<'a> = (i32, i32, &'a [i32], &'a [i32], &'a [(i32, i32, i32)]);
        let cases: [Case; 5] = [
            (
                40,
                40,
                &[-20, 30, 5],
                &[-10, 60, 20],
                &[
                    (0, 15, 1),
                    (0, 16, 2),
                    (0, 17, 3),
                    (0, 18, 4),
                    (1, 19, 5),
                    (2, 20, 5),
                    (3, 21, 6),
                    (3, 22, 7),
                    (4, 23, 7),
                    (5, 24, 8),
                    (5, 25, 9),
                    (6, 26, 9),
                    (7, 27, 10),
                    (8, 28, 10),
                    (8, 29, 11),
                    (9, 30, 12),
                    (10, 31, 12),
                    (10, 32, 13),
                    (11, 33, 14),
                    (12, 34, 14),
                    (13, 35, 15),
                    (13, 36, 15),
                    (14, 37, 16),
                    (15, 38, 17),
                    (15, 39, 17),
                ],
            ),
            (
                40,
                40,
                &[-50, 10, 10],
                &[5, 5, 30],
                &[
                    (0, 5, 10),
                    (0, 6, 10),
                    (0, 7, 10),
                    (0, 8, 10),
                    (0, 9, 10),
                    (0, 10, 10),
                    (0, 11, 10),
                    (0, 12, 10),
                    (0, 13, 10),
                    (0, 14, 10),
                    (0, 15, 10),
                    (0, 16, 10),
                    (0, 17, 10),
                    (0, 18, 10),
                    (0, 19, 10),
                    (0, 20, 10),
                    (0, 21, 10),
                    (0, 22, 10),
                    (0, 23, 10),
                    (0, 24, 10),
                    (0, 25, 10),
                    (1, 26, 10),
                    (4, 27, 10),
                    (6, 28, 10),
                    (8, 29, 10),
                ],
            ),
            (
                20,
                20,
                &[-5, 15, 5],
                &[-5, 15, 15],
                &[
                    (0, 1, 1),
                    (0, 2, 2),
                    (0, 3, 3),
                    (0, 4, 4),
                    (0, 5, 5),
                    (1, 6, 6),
                    (1, 7, 7),
                    (2, 8, 8),
                    (2, 9, 9),
                    (3, 10, 10),
                    (3, 11, 11),
                    (4, 12, 12),
                    (4, 13, 13),
                    (5, 14, 14),
                ],
            ),
            (
                30,
                30,
                &[2, 25, 20, 6],
                &[3, 5, 27, 20],
                &[
                    (2, 3, 5),
                    (3, 4, 17),
                    (3, 5, 25),
                    (3, 6, 25),
                    (3, 7, 25),
                    (3, 8, 25),
                    (4, 9, 24),
                    (4, 10, 24),
                    (4, 11, 24),
                    (4, 12, 24),
                    (5, 13, 23),
                    (5, 14, 23),
                    (5, 15, 23),
                    (5, 16, 23),
                    (6, 17, 22),
                    (6, 18, 22),
                    (6, 19, 22),
                    (7, 20, 22),
                    (9, 21, 22),
                    (11, 22, 21),
                    (13, 23, 21),
                    (15, 24, 21),
                    (17, 25, 21),
                    (19, 26, 20),
                ],
            ),
            (
                30,
                30,
                &[2, 25, 2, 25],
                &[3, 25, 25, 3],
                &[
                    (3, 3, 25),
                    (4, 4, 24),
                    (5, 5, 23),
                    (6, 6, 22),
                    (7, 7, 21),
                    (8, 8, 20),
                    (9, 9, 19),
                    (10, 10, 18),
                    (11, 11, 17),
                    (12, 12, 16),
                    (13, 13, 15),
                    (13, 14, 14),
                    (12, 15, 15),
                    (11, 16, 16),
                    (10, 17, 17),
                    (9, 18, 18),
                    (8, 19, 19),
                    (7, 20, 20),
                    (6, 21, 21),
                    (5, 22, 22),
                    (4, 23, 23),
                    (3, 24, 24),
                ],
            ),
        ];

        for (i, (w, h, xs, ys, want)) in cases.iter().enumerate() {
            let mut cv = Canvas::new(*w as usize, *h as usize);
            cv.fill_polygon(xs, ys, [1, 0, 0]);
            assert_eq!(spans(&cv), want.to_vec(), "case {i}");
        }
    }

    fn px_at(cv: &Canvas, x: usize, y: usize) -> [u8; 3] {
        let at = (y * cv.w + x) * 3;
        [cv.px[at], cv.px[at + 1], cv.px[at + 2]]
    }

    /// 2x2 sprite: red top-left, blue bottom-right, transparent off-diagonal.
    fn sprite() -> Gif {
        Gif {
            w: 2,
            h: 2,
            px: vec![0xFFFF_0000, 0, 0, 0xFF00_00FF],
        }
    }

    #[test]
    fn draw_image_blits_opaque_skips_transparent() {
        let mut cv = Canvas::new(4, 3);
        cv.fill([10, 20, 30]);
        cv.draw_image(&sprite(), 1, 1);
        assert_eq!(px_at(&cv, 1, 1), [255, 0, 0]);
        assert_eq!(px_at(&cv, 2, 2), [0, 0, 255]);
        // the transparent off-diagonal left the destination alone
        assert_eq!(px_at(&cv, 2, 1), [10, 20, 30]);
        assert_eq!(px_at(&cv, 1, 2), [10, 20, 30]);
    }

    #[test]
    fn draw_image_clips_at_edges() {
        let mut cv = Canvas::new(4, 3);
        cv.fill([10, 20, 30]);
        cv.draw_image(&sprite(), -1, -1);
        // only the sprite's bottom-right lands, at (0,0)
        assert_eq!(px_at(&cv, 0, 0), [0, 0, 255]);
        assert_eq!(px_at(&cv, 1, 0), [10, 20, 30]);

        cv.draw_image(&sprite(), 3, 2);
        // only the sprite's top-left lands, at (3,2); the rest clips
        assert_eq!(px_at(&cv, 3, 2), [255, 0, 0]);
        assert_eq!(px_at(&cv, 3, 1), [10, 20, 30]);
    }

    #[test]
    fn draw_image_overwrites_opaque_pixels() {
        let mut cv = Canvas::new(4, 3);
        cv.fill([10, 20, 30]);
        cv.draw_image(&sprite(), 1, 1);
        cv.draw_image(&sprite(), 1, 1);
        assert_eq!(px_at(&cv, 1, 1), [255, 0, 0]);

        // a later opaque blit replaces an earlier one
        let one = Gif {
            w: 1,
            h: 1,
            px: vec![0xFF00_FF00],
        };
        cv.draw_image(&one, 1, 1);
        assert_eq!(px_at(&cv, 1, 1), [0, 255, 0]);
    }

    #[test]
    fn fill_rect_is_whw_and_draw_rect_is_whp1() {
        let mut cv = Canvas::new(6, 6);
        cv.fill_rect(1, 1, 3, 2, [1, 2, 3]);
        // w=3 h=2 exactly: (1..3, 1..2)
        assert_eq!(px_at(&cv, 1, 1), [1, 2, 3]);
        assert_eq!(px_at(&cv, 3, 2), [1, 2, 3]);
        assert_eq!(px_at(&cv, 4, 1), [0, 0, 0]);
        assert_eq!(px_at(&cv, 3, 3), [0, 0, 0]);

        cv.draw_rect(0, 0, 4, 4, [9, 9, 9]);
        // AWT outline: x..=x+w, y..=y+h
        assert_eq!(px_at(&cv, 4, 4), [9, 9, 9]);
        assert_eq!(px_at(&cv, 4, 2), [9, 9, 9]);
        assert_eq!(px_at(&cv, 2, 4), [9, 9, 9]);
        assert_eq!(px_at(&cv, 5, 0), [0, 0, 0]);
    }

    #[test]
    fn draw_string_uses_the_atlas_cell_and_baseline() {
        let a = crate::xtfont::atlas();
        let (x, y, size) = (3, 17, 13);
        let mut cv = Canvas::new(40, 26);
        cv.draw_string("!", x, y, [255, 255, 255], size);
        let cell = a.cell(size, b'!' as u32);
        let ox = x - a.glyph_dx;
        let oy = y - a.baseline;
        let mut ink = 0;
        for cy in 0..a.cell_h {
            for cx in 0..a.cell_w {
                let cov = cell[cy * a.cell_w + cx];
                let got = px_at(&cv, (ox + cx as i32) as usize, (oy + cy as i32) as usize);
                // white ink over a black canvas: out = MUL8(cov, 255) = cov
                assert_eq!(got, [cov, cov, cov], "cell ({cx},{cy}) coverage");
                if cov > 0 {
                    ink += 1;
                }
            }
        }
        assert!(ink > 0, "the glyph inked");
        // the cell's frame is blank (the generator rejects bleed), so the
        // canvas's only ink is the glyph's
        assert_eq!(px_at(&cv, 0, 0), [0, 0, 0]);
        assert_eq!(px_at(&cv, 39, 25), [0, 0, 0]);

        // an out-of-atlas char falls back to '?'s cell and advance
        let mut cv2 = Canvas::new(40, 26);
        cv2.draw_string("\u{e9}!", x, y, [255, 255, 255], size);
        let mut cv3 = Canvas::new(40, 26);
        cv3.draw_string("?!", x, y, [255, 255, 255], size);
        assert_eq!(cv2.px, cv3.px, "non-ASCII falls back to '?'");
    }

    #[test]
    fn draw_string_advances_by_the_manifest_char_width() {
        use crate::xtfont;
        let a = xtfont::atlas();
        // " A": the space's cell is blank, so the A lands exactly where the
        // pen advance puts it — a clean read of the advance loop.
        let (x, y, size) = (4, 18, 11);
        let mut cv = Canvas::new(40, 24);
        cv.draw_string(" A", x, y, [255, 255, 255], size);
        let mut solo = Canvas::new(40, 24);
        solo.draw_string(
            "A",
            x + a.advance(size, b' ' as u32),
            y,
            [255, 255, 255],
            size,
        );
        assert_eq!(cv.px, solo.px, "the space advanced the pen by charWidth");

        // `stringWidth` is the sum of the pen advances
        let s = "Power low, perform stunt!";
        let w = xtfont::string_width(s, 11);
        let sum: i32 = s.chars().map(|c| a.advance(11, c as u32)).sum();
        assert_eq!(w, sum);
    }
}
