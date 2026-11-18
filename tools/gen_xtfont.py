#!/usr/bin/env python3
"""Generate crates/nfm-core/src/xtfont.rs from dhepper/font8x8's basic table.

The xtGraphics port needs a stand-in for the AWT `Font("SansSerif", 1, 11/13)`
rasterizer; the original went through each era JRE's platform fonts, so there
is no ground truth to diff against (glyphs were machine-dependent in the
original too). This embeds the public-domain font8x8 bitmap font instead and
derives every metric from it, so raster and `stringWidth` stay self-consistent.

Usage: python3 tools/gen_xtfont.py <font8x8_basic.h> > crates/nfm-core/src/xtfont.rs
Fetch the header from:
  https://raw.githubusercontent.com/dhepper/font8x8/master/font8x8_basic.h
"""

import re
import sys


def main() -> None:
    src = open(sys.argv[1]).read()
    rows = re.findall(r"\{\s*(0x[0-9A-Fa-f]{2}(?:,\s*0x[0-9A-Fa-f]{2}){7})\s*\}", src)
    assert len(rows) == 128, f"expected 128 glyphs, got {len(rows)}"

    # Keep the printable ASCII slice 0x20..=0x7F; index 0 = space.
    glyphs = []
    for r in rows[0x20:]:
        vals = [int(v, 16) for v in r.split(",")]
        glyphs.append(vals)
    assert len(glyphs) == 96

    lines = []
    for i in range(0, 96, 8):
        lines.append("    " + " ".join("[%s]," % ", ".join("0x%02X" % b for b in g) for g in glyphs[i:i + 8]))
    table = "\n".join(lines)

    out = sys.stdout
    out.write(HEADER)
    out.write(table + "\n")
    out.write(FOOTER)


HEADER = '''\
//! The `Font("SansSerif", 1, 11|13)` + `FontMetrics` stand-in for the xt
//! layer's text. The original rasterized through each era JRE's platform
//! rasterizer - there is no ground truth to diff against, and glyphs were
//! machine-dependent in the original too. This port embeds the public-domain
//! 8x8 monochrome `font8x8` basic-latin font (Daniel Hepper, Marcel Sondaar,
//! IBM VGA fonts - public domain) and derives all metrics from it, so raster
//! and `stringWidth` stay self-consistent.
//!
//! Documented deviation: one fixed proportional bitmap font replaces both
//! point sizes, and `center`/`centerb` are 8px tall rather than 11/13.
//!
//! Regenerate the table with `python3 tools/gen_xtfont.py <font8x8_basic.h>`.

/// The glyphs for ASCII 0x20..=0x7F, row-major: each byte is one row, bit k
/// of the byte = column k (bit 0 = the leftmost pixel).
#[rustfmt::skip]
static FONT8X8: [[u8; 8]; 96] = [
'''

FOOTER = '''\
];

/// One glyph's cell (the row bytes, bit k = column k).
pub fn glyph(c: u8) -> &'static [u8; 8] {
    if (0x20..=0x7E).contains(&c) {
        &FONT8X8[(c - 0x20) as usize]
    } else {
        &FONT8X8[0]
    }
}

/// The advance of one glyph: the rightmost set column + 1 column of ink,
/// plus one pixel of inter-glyph space; blank glyphs (the space) advance 4.
/// The original's proportional SansSerif advances land in the same 4..8px
/// range at 11pt, so centered `drawcs` lines sit close to the source.
pub fn advance(c: u8) -> i32 {
    if !(0x21..=0x7E).contains(&c) {
        return 4;
    }
    let g = glyph(c);
    let mut last = 0usize;
    for &b in g.iter() {
        if b == 0 {
            continue;
        }
        let hi = 7 - b.leading_zeros() as usize;
        if hi > last {
            last = hi;
        }
    }
    last as i32 + 2
}

/// `FontMetrics.stringWidth` for the stand-in font.
pub fn string_width(s: &str) -> i32 {
    s.bytes().map(advance).sum()
}
'''


if __name__ == "__main__":
    main()
