//! GIF decoder — GIF87a/GIF89a to per-pixel ARGB (`u32`, `0xAARRGGBB`), the
//! pixel form the JDK hands `PixelGrabber` and the applet's image
//! transforms consume (`xtGraphics.loadsnap`/`pressed` — the next xt
//! milestone slice).
//!
//! The LZW core follows the JDK's native GIF decoder (`gifdecoder.c`,
//! `sun.awt.image.GifImageDecoder.parseImage` — the code the applet's
//! pixels actually come from): variable-width codes read LSB-first, the
//! table grows to 12-bit codes exactly when the next code to read needs
//! the wider width (`++freeCode >= maxCode`), the first code after a
//! clear is plain data, and every malformed stream is *quietly accepted*
//! — ran-dry, a code beyond the table, or chain overflow all stop
//! decoding and return the pixels produced so far, with the undecoded
//! remainder left transparent (those pixels are never sent to the image
//! consumer). A clear code arriving after the frame is complete is
//! treated as end-of-data (:290-318).
//!
//! Scope matches the applet's assets (`images1-4.zipo`, all single-frame
//! sprites): global/local color tables, optional interlace, and a
//! transparent index stored as alpha 0 with the palette color kept under
//! the mask — what the JDK's `IndexColorModel` produces and what
//! `loadsnap`'s corner key compares against.

use std::fmt;

#[derive(Debug)]
pub struct GifError(pub String);

impl fmt::Display for GifError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for GifError {}

/// A decoded GIF: frame 0 composited on the logical screen.
pub struct Gif {
    pub w: usize,
    pub h: usize,
    /// ARGB pixels, row-major (`0xAARRGGBB`; alpha 0 = transparent, with
    /// the palette color kept under the mask).
    pub px: Vec<u32>,
}

impl Gif {
    /// The not-yet-loaded stand-in for the `Image` fields the applet leaves
    /// `null` until the archive loads: zero size, no pixels. Drawing it is
    /// a no-op and every pixel transform over it is empty, which keeps the
    /// xt struct's 40 sprite sites free of `Option` plumbing.
    pub fn empty() -> Gif {
        Gif {
            w: 0,
            h: 0,
            px: Vec::new(),
        }
    }
}

impl fmt::Debug for Gif {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gif")
            .field("w", &self.w)
            .field("h", &self.h)
            .field("px", &format_args!("[{} pixels]", self.px.len()))
            .finish()
    }
}

struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, GifError> {
        let b = self.d.get(self.pos).copied();
        self.pos += 1;
        b.ok_or_else(|| GifError("truncated gif".to_string()))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], GifError> {
        if self.pos + n > self.d.len() {
            return Err(GifError("truncated gif".to_string()));
        }
        let s = &self.d[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u16le(&mut self) -> Result<u16, GifError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    /// One sub-block chunk: length byte + payload; `Ok(None)` at the zero
    /// terminator.
    fn chunk(&mut self) -> Result<Option<&'a [u8]>, GifError> {
        let n = self.byte()? as usize;
        if n == 0 {
            Ok(None)
        } else {
            Ok(Some(self.take(n)?))
        }
    }

    /// A color table of `2^(size+1)` entries when the high bit is set.
    fn palette(&mut self, packed: u8) -> Result<Vec<[u8; 3]>, GifError> {
        if packed & 0x80 == 0 {
            return Ok(Vec::new());
        }
        let n = 2usize << (packed & 7);
        let raw = self.take(n * 3)?;
        Ok(raw.as_chunks::<3>().0.to_vec())
    }
}

/// Decode a GIF's frame 0. Multi-frame files are rejected — the applet's
/// assets are all single-frame; extend if one ever shows up.
pub fn decode(data: &[u8]) -> Result<Gif, GifError> {
    let mut r = Reader { d: data, pos: 0 };
    let sig = r.take(6)?;
    if sig != b"GIF87a" && sig != b"GIF89a" {
        return Err(GifError("not a gif signature".to_string()));
    }
    let w = r.u16le()? as usize;
    let h = r.u16le()? as usize;
    if w == 0 || h == 0 {
        return Err(GifError("zero-size logical screen".to_string()));
    }
    let packed = r.byte()?;
    let _bg = r.byte()?;
    let _aspect = r.byte()?;
    let gct = r.palette(packed)?;

    let mut px = vec![0u32; w * h]; // transparent until painted
    let mut frames = 0usize;
    let mut transparent: Option<u8> = None;

    loop {
        match r.byte()? {
            0x21 => {
                // extension: label byte + sub-block chunks
                let label = r.byte()?;
                if label == 0xF9 {
                    // graphic control: one 4-byte sub-block; bit 0 of the
                    // flags turns the last byte into the transparent index
                    let blk = r
                        .chunk()?
                        .ok_or_else(|| GifError("empty graphic control".to_string()))?;
                    if blk.len() != 4 {
                        return Err(GifError("bad graphic control size".to_string()));
                    }
                    if blk[0] & 1 != 0 {
                        transparent = Some(blk[3]);
                    }
                }
                while r.chunk()?.is_some() {}
            }
            0x2C => {
                frames += 1;
                if frames > 1 {
                    return Err(GifError("multi-frame gif".to_string()));
                }
                let left = r.u16le()? as usize;
                let top = r.u16le()? as usize;
                let fw = r.u16le()? as usize;
                let fh = r.u16le()? as usize;
                let ipacked = r.byte()?;
                let lct = r.palette(ipacked)?;
                let pal: &[[u8; 3]] = if lct.is_empty() { &gct } else { &lct };
                if pal.is_empty() {
                    return Err(GifError("image without color table".to_string()));
                }
                if left + fw > w || top + fh > h {
                    return Err(GifError("frame overflows logical screen".to_string()));
                }
                let mcs = r.byte()?;
                let mut bits = Vec::new();
                while let Some(c) = r.chunk()? {
                    bits.extend_from_slice(c);
                }
                let idx = lzw(&bits, mcs, fw * fh)?;
                let tkey = transparent;
                let mut si = 0usize;
                for dr in frame_rows(fh, ipacked & 0x40 != 0) {
                    let base = (top + dr) * w + left;
                    for c in 0..fw {
                        if si >= idx.len() {
                            // the stream ran short: the undecoded remainder
                            // was never sent to the image consumer — it
                            // stays transparent
                            break;
                        }
                        let i = idx[si] as usize;
                        si += 1;
                        if i >= pal.len() {
                            return Err(GifError(format!("pixel index {i} outside palette")));
                        }
                        let p = pal[i];
                        px[base + c] = if Some(i as u8) == tkey {
                            (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32
                        } else {
                            0xFF00_0000 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32
                        };
                    }
                }
                // a graphic control applies to the following image only
                transparent = None;
            }
            0x3B => break,
            b => return Err(GifError(format!("unknown block {b:02x}"))),
        }
    }
    if frames == 0 {
        return Err(GifError("no image block".to_string()));
    }
    Ok(Gif { w, h, px })
}

/// The frame's rows in destination order: plain top-down, or the four
/// interlace passes (row starts 0, 4, 2, 3 with steps 8, 8, 4, 2).
fn frame_rows(fh: usize, interlaced: bool) -> Vec<usize> {
    if !interlaced {
        return (0..fh).collect();
    }
    let starts = [0usize, 4, 2, 3];
    let steps = [8usize, 8, 4, 2];
    let mut rows = Vec::with_capacity(fh);
    for p in 0..4 {
        let mut row = starts[p];
        while row < fh {
            rows.push(row);
            row += steps[p];
        }
    }
    rows
}

/// The GIF LZW variant, line-faithful to the JDK's native decoder
/// (`gifdecoder.c`): a uniform loop with no special first-code branch —
/// every data code goes through the same interpret/emit/define path,
/// where the first code after a clear defines a throwaway entry at the
/// eof slot (`freeCode` is deliberately one less there, so it is
/// incremented right after) that still participates in the code-width
/// schedule. No error path once the header is sane: a dry stream, a code
/// beyond the table, or a chain overflow all break out and return the
/// output so far (the caller paints the remainder transparent).
fn lzw(src: &[u8], mcs: u8, expected: usize) -> Result<Vec<u8>, GifError> {
    if mcs > 11 {
        // the JDK's java layer rejects initCodeSize >= 12 before calling
        return Err(GifError(format!("lzw min code size {mcs} out of range")));
    }
    let clear: u16 = 1 << mcs;
    let eof: u16 = clear + 1;
    let bitmask: u16 = clear - 1; // gifdecoder.c:219 (animated-GIF fix)
    let mut width: u32 = (mcs as u32) + 1; // codeSize
    let mut maxcode: u16 = 1 << width; // maxCode
    let mut next: u16 = eof; // freeCode: the slot the next define writes
    let mut oldcode: u16 = 0; // oldCode (stale across clears, as in the C)
    let mut prevroot: u8 = 0; // prevChar (stale across clears, as in the C)
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut pos = 0usize;
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let mut stack = vec![0u8; 4097];
    let mut out = Vec::with_capacity(expected);

    loop {
        while nbits < width {
            if pos >= src.len() {
                // "quietly accept truncated GIF images" (:241)
                return Ok(out);
            }
            bits |= (src[pos] as u32) << nbits;
            pos += 1;
            nbits += 8;
        }
        let code = (bits & ((1 << width) - 1)) as u16;
        bits >>= width;
        nbits -= width;

        if code == clear {
            // Clear sets everything back, then reads the immediately
            // subsequent code as uncompressed data (:290-318); a clear
            // after the frame is complete is treated as eof.
            width = (mcs as u32) + 1;
            maxcode = 1 << width;
            next = eof;
            if out.len() >= expected {
                break;
            }
            continue;
        }
        if code == eof {
            break;
        }

        // Interpret the code (:343-361): a code at or beyond freeCode is
        // not in the table yet — equal means the entry being defined
        // right now (repeat the previous expansion), greater means the
        // stream is truncated and is quietly accepted.
        let mut cur = code as usize;
        if code >= next {
            if code > next {
                break;
            }
            cur = oldcode as usize;
        }
        // Pursue the chain to its root, stacking suffixes LIFO (:363-389);
        // a code equal to freeCode starts from the previous expansion and
        // ends with prevChar, the previous expansion's root. Overflow
        // flushes the rest of the data and quietly accepts the GIF as
        // truncated (:370-383).
        let mut sp = 0usize;
        if code == next {
            stack[sp] = prevroot;
            sp += 1;
        }
        while cur > bitmask as usize {
            stack[sp] = suffix[cur];
            sp += 1;
            if sp == 4097 {
                return Ok(out);
            }
            cur = prefix[cur] as usize;
        }
        prevroot = cur as u8; // the root character (:388)
        stack[sp] = prevroot;
        sp += 1;
        for i in (0..sp).rev() {
            out.push(stack[i]);
        }
        if out.len() >= expected {
            // the frame is complete; the port clamps here where the C
            // would keep wrapping leftovers through the raster line
            break;
        }

        // Build the hash table on the fly (:455-474)
        if (next as usize) < 4096 {
            prefix[next as usize] = oldcode;
            suffix[next as usize] = prevroot;
        }
        oldcode = code;
        next += 1;
        if next >= maxcode {
            if width < 12 {
                width += 1;
                maxcode <<= 1;
            } else {
                next = maxcode - 1; // frozen until the next clear
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zipo::Zipo;

    /// Textbook LZW encoder for round-trip tests. Its code-width schedule
    /// mirrors the decoder's define schedule (gifdecoder.c defines an
    /// entry for every data code, the first after a clear being a
    /// throwaway at the eof slot): the counter starts at the eof code and
    /// advances once per data code emitted, growing the width whenever it
    /// reaches 2^width. The string table's own slots start at clear+2.
    fn lzw_encode(symbols: &[u8], mcs: u8) -> Vec<u8> {
        let clear: u16 = 1 << mcs;
        let eof: u16 = clear + 1;
        let mut width: u32 = (mcs as u32) + 1;
        let mut maxcode: u16 = 1 << width;
        let mut wfree: u16 = eof;
        let mut bits: Vec<bool> = Vec::new();
        let emit = |code: u16, width: u32, bits: &mut Vec<bool>| {
            for i in 0..width {
                bits.push((code >> i) & 1 == 1);
            }
        };
        use std::collections::HashMap;
        let mut dict: HashMap<Vec<u8>, u16> = HashMap::new();
        let mut dnext: u16 = clear + 2;
        emit(clear, width, &mut bits);
        let mut cur: Vec<u8> = Vec::new();
        for &s in symbols {
            if cur.is_empty() {
                cur.push(s);
                continue;
            }
            let mut cand = cur.clone();
            cand.push(s);
            if dict.contains_key(&cand) {
                cur = cand;
                continue;
            }
            if cur.len() == 1 {
                emit(cur[0] as u16, width, &mut bits);
            } else {
                emit(dict[&cur], width, &mut bits);
            }
            if dnext < 4096 {
                dict.insert(cand, dnext);
                dnext += 1;
            } else {
                // table full: emit a clear and start the table over
                emit(clear, width, &mut bits);
                dict.clear();
                dnext = clear + 2;
                width = (mcs as u32) + 1;
                maxcode = 1 << width;
                wfree = eof;
                cur.clear();
                cur.push(s);
                continue;
            }
            // the decoder defines an entry for every data code it reads
            // (the first after a clear being a throwaway) — mirror that
            wfree += 1;
            if wfree >= maxcode && width < 12 {
                width += 1;
                maxcode <<= 1;
            }
            cur.clear();
            cur.push(s);
        }
        // the tail code: the decoder defines for it too
        if cur.len() == 1 {
            emit(cur[0] as u16, width, &mut bits);
        } else if cur.len() > 1 {
            emit(dict[&cur], width, &mut bits);
        }
        wfree += 1;
        if wfree >= maxcode && width < 12 {
            width += 1;
        }
        emit(eof, width, &mut bits);
        pack_bits(&bits)
    }

    /// Pack an LSB-first bit list into bytes.
    fn pack_bits(bits: &[bool]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut acc = 0u32;
        let mut n = 0u32;
        for &b in bits {
            acc |= (b as u32) << n;
            n += 1;
            if n == 8 {
                out.push(acc as u8);
                acc = 0;
                n = 0;
            }
        }
        if n > 0 {
            out.push(acc as u8);
        }
        out
    }

    fn xorshift(seed: u32) -> impl Iterator<Item = u8> {
        let mut s = seed;
        std::iter::from_fn(move || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            Some((s >> 24) as u8 & 15)
        })
    }

    #[test]
    fn round_trips_lzw_patterns() {
        // (pattern, mcs) pairs — constant, alternating, ramp, long
        // xorshift, and a table-filling run past 4096 entries
        let cases: Vec<(Vec<u8>, u8)> = vec![
            (vec![3; 64], 2),
            ((0..64).map(|i| (i & 1) as u8).collect(), 1),
            ((0..64).map(|i| i as u8 & 15).collect(), 4),
            (xorshift(1).take(300).collect(), 4),
            (xorshift(7).take(9000).collect(), 8),
        ];
        for (i, (sym, mcs)) in cases.iter().enumerate() {
            let enc = lzw_encode(sym, *mcs);
            let dec = lzw(&enc, *mcs, sym.len()).unwrap_or_else(|e| panic!("case {i}: {e}"));
            assert_eq!(&dec, sym, "case {i}");
        }
    }

    #[test]
    fn decodes_a_hand_built_gif() {
        // 2x1, 4-entry GCT, mcs 1: stream clear(2), 0, 1, end(3) at 2-bit
        // codes, LSB-first — one byte 0b11010010 (210). The graphic
        // control marks index 1 transparent.
        let data = [
            b"GIF89a"[..].to_vec(),
            vec![2, 0, 1, 0], // logical screen 2x1
            vec![0x81, 0, 0], // GCT of 4
            vec![255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0],
            vec![0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x01, 0x00],
            // image descriptor + mcs 1, then one data sub-block. The
            // decoder's throwaway define grows the code width to 3 after
            // the first data code, so the stream packs at widths
            // 2,2,3,3: clear(2), 0(2), 1(3), end(3) = 10 bits → 0x92, 0x01
            vec![
                0x2C, 0, 0, 0, 0, 2, 0, 1, 0, 0x00, 0x01, 0x02, 0x92, 0x01, 0x00,
            ],
            vec![0x3B],
        ]
        .concat();
        let g = decode(&data).unwrap();
        assert_eq!((g.w, g.h), (2, 1));
        assert_eq!(g.px[0], 0xFFFF_0000, "index 0 = red opaque");
        assert_eq!(g.px[1], 0x0000_00FF, "index 1 = blue, alpha 0");
    }

    #[test]
    fn quietly_accepts_truncated_streams() {
        assert!(decode(b"PNG...").is_err(), "bad signature");
        assert!(decode(b"GIF89a\x02").is_err(), "truncated screen");

        // a stream that dries up mid-frame: the decoded pixels land and
        // the remainder stays transparent
        let data = [
            b"GIF87a"[..].to_vec(),
            vec![4, 0, 1, 0],
            vec![0x80, 0, 0],
            vec![10, 20, 30, 40, 50, 60],
            vec![0x2C, 0, 0, 0, 0, 4, 0, 1, 0, 0x00, 0x02],
            // mcs 2: clear(4), symbol 0 — then dry, no end code
            // 3-bit LSB-first: 4=001 0=000 → 6 bits: 0x04
            vec![0x01, 0x04, 0x00],
            vec![0x3B],
        ]
        .concat();
        let g = decode(&data).unwrap();
        assert_eq!(g.px[0], 0xFF0A_141E, "pixel 0 = palette 0 opaque");
        assert_eq!(g.px[1], 0, "stream dried before pixel 1");
        assert_eq!(g.px[2], 0, "pixel 2 undecoded = transparent");
        assert_eq!(g.px[3], 0, "pixel 3 undecoded = transparent");

        // a code beyond the table: the JDK flushes and quietly accepts
        let data = [
            b"GIF89a"[..].to_vec(),
            vec![2, 0, 1, 0],
            vec![0x80, 0, 0],
            vec![1, 2, 3, 4, 5, 6],
            vec![0x2C, 0, 0, 0, 0, 2, 0, 1, 0, 0x00, 0x02],
            // mcs 2 (width 3, symbols 0..3, clear 4, end 5, free 6):
            // clear(4), 0, then 7 — beyond the table
            // 3-bit LSB-first: 4=001 0=000 7=111 → 9 bits: 0xC4, 0x01
            vec![0x02, 0xC4, 0x01, 0x00],
            vec![0x3B],
        ]
        .concat();
        let g = decode(&data).unwrap();
        assert_eq!(g.px[0], 0xFF01_0203, "decoded before the bad code");
        assert_eq!(g.px[1], 0, "nothing after the bad code");
    }

    const IMAGES: [&str; 4] = [
        "../../../assets/graphics/images1.zipo",
        "../../../assets/graphics/images2.zipo",
        "../../../assets/graphics/images3.zipo",
        "../../../assets/graphics/images4.zipo",
    ];

    /// The applet's sprite inventory: every archive gif decodes, the
    /// known dimensions hold, GIF87a carries no transparency, and every
    /// pixel's alpha is exactly 0 or 255.
    #[test]
    fn decodes_every_applet_sprite() {
        // (name, (w, h)) — the full 47-sprite inventory, sizes verified
        // against an independent decoder (macOS sips)
        let dims: &[(&str, (usize, usize))] = &[
            ("0c.gif", (73, 39)),
            ("1.gif", (33, 17)),
            ("1c.gif", (30, 39)),
            ("2.gif", (33, 17)),
            ("2c.gif", (30, 39)),
            ("3.gif", (33, 17)),
            ("3c.gif", (30, 39)),
            ("4.gif", (33, 17)),
            ("5.gif", (35, 18)),
            ("bb.gif", (550, 43)),
            ("bl.gif", (41, 400)),
            ("back.gif", (60, 21)),
            ("br.gif", (41, 400)),
            ("bt.gif", (550, 36)),
            ("cars.gif", (550, 400)),
            ("congrad.gif", (269, 38)),
            ("continue1.gif", (86, 21)),
            ("continue2.gif", (90, 22)),
            ("damage.gif", (176, 16)),
            ("gameh.gif", (197, 24)),
            ("gameov.gif", (170, 33)),
            ("inst1.gif", (550, 400)),
            ("inst2.gif", (550, 400)),
            ("inst3.gif", (550, 400)),
            ("lap.gif", (27, 12)),
            ("loadingmusic.gif", (221, 131)),
            ("madness.gif", (234, 45)),
            ("main.gif", (550, 400)),
            ("next.gif", (60, 21)),
            ("nfmcom.gif", (412, 30)),
            ("options.gif", (182, 75)),
            ("paused.gif", (237, 188)),
            ("pgate.gif", (367, 147)),
            ("position.gif", (65, 12)),
            ("power.gif", (176, 16)),
            ("rpro.gif", (250, 19)),
            ("radicalplay.gif", (375, 59)),
            ("select.gif", (148, 21)),
            ("selectcar.gif", (181, 24)),
            ("start1.gif", (82, 26)),
            ("start2.gif", (82, 26)),
            ("statb.gif", (156, 7)),
            ("youlost.gif", (129, 27)),
            ("youwastedem.gif", (218, 26)),
            ("yourwasted.gif", (206, 25)),
            ("wasted.gif", (53, 12)),
            ("youwon.gif", (134, 25)),
        ];
        let mut count = 0usize;
        for path in IMAGES {
            let raw: &[u8] = match path {
                "../../../assets/graphics/images1.zipo" => {
                    include_bytes!("../../../assets/graphics/images1.zipo")
                }
                "../../../assets/graphics/images2.zipo" => {
                    include_bytes!("../../../assets/graphics/images2.zipo")
                }
                "../../../assets/graphics/images3.zipo" => {
                    include_bytes!("../../../assets/graphics/images3.zipo")
                }
                "../../../assets/graphics/images4.zipo" => {
                    include_bytes!("../../../assets/graphics/images4.zipo")
                }
                _ => unreachable!(),
            };
            let zipo = Zipo::read(raw).unwrap();
            for entry in &zipo.entries {
                if !entry.name.ends_with(".gif") {
                    continue;
                }
                let g = decode(&entry.data).unwrap_or_else(|e| panic!("{}: {e}", entry.name));
                assert!(g.w > 0 && g.h > 0);
                assert_eq!(g.px.len(), g.w * g.h);
                let opaque87a = entry.name == "cars.gif" || entry.name == "4.gif";
                for (i, &p) in g.px.iter().enumerate() {
                    let a = (p >> 24) as u8;
                    assert!(a == 0 || a == 255, "{} px {i} alpha {a}", entry.name);
                    if opaque87a {
                        assert_eq!(a, 255, "{} px {i} must be opaque", entry.name);
                    }
                }
                if let Some(&(_, (ew, eh))) = dims.iter().find(|(n, _)| *n == entry.name) {
                    assert_eq!((g.w, g.h), (ew, eh), "{}", entry.name);
                }
                count += 1;
            }
        }
        assert_eq!(count, 47, "unexpected sprite count");
    }
}
