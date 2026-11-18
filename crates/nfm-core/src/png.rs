//! A minimal PNG decoder for the shared glyph atlas
//! (`tests/fixtures/xtfont-atlas/atlas.png`, see
//! notes/JAVA-BEHAVIOR/B12_pixel.md §4).
//!
//! Scope is deliberately ONE variant: 8-bit truecolour RGB (color type 2),
//! non-interlaced, with a plain zlib `IDAT` stream. That is what `ImageIO`
//! emitted for the committed atlas (1900x44, `TYPE_INT_RGB`), and anything
//! else panics loudly instead of decoding approximately - the atlas is the
//! port's text ground truth, so a silently-misread PNG would move glyphs.
//!
//! The five PNG scanline filters (None/Sub/Up/Average/Paeth) are implemented
//! per the PNG specification (section 9.2); decompression uses the crate's
//! existing `flate2` dependency, so there is no new dependency.
//!
//! Only the pixels are produced: `Image::rgb` is row-major RGB triples,
//! top-down, with no alpha (the variant is opaque).

use flate2::read::ZlibDecoder;
use std::io::Read;

/// A decoded 8-bit RGB raster.
pub struct Image {
    pub w: usize,
    pub h: usize,
    /// `w * h * 3` bytes, row-major top-down, R/G/B per pixel.
    pub rgb: Vec<u8>,
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

/// Decode the one supported PNG variant; panics with a `png:` prefixed
/// message on anything else (`jpeg`/`gif` follow the same loud-failure rule).
pub fn decode(bytes: &[u8]) -> Image {
    if bytes.len() < SIGNATURE.len() || bytes[..8] != SIGNATURE {
        panic!("png: not a PNG (bad signature)");
    }
    let mut at = 8usize;
    let mut w = 0usize;
    let mut h = 0usize;
    let mut saw_ihdr = false;
    let mut saw_idat = false;
    let mut saw_iend = false;
    let mut idat: Vec<u8> = Vec::new();

    while at + 8 <= bytes.len() {
        let len = be32(&bytes[at..at + 4]);
        let kind = &bytes[at + 4..at + 8];
        let data_start = at + 8;
        let data_end = data_start + len;
        if data_end + 4 > bytes.len() {
            panic!("png: truncated {kind:?} chunk (len {len})");
        }
        let data = &bytes[data_start..data_end];
        // CRC is the last 4 bytes; a corrupt file is a loud failure on the
        // structural checks below, so the CRC itself is not compared here
        // (the committed atlas's sha256 is pinned by the manifest instead).
        match kind {
            b"IHDR" => {
                if saw_ihdr || len != 13 {
                    panic!("png: bad IHDR (repeat or len {len})");
                }
                saw_ihdr = true;
                w = be32(data);
                h = be32(&data[4..]);
                let bit_depth = data[8];
                let color_type = data[9];
                let compression = data[10];
                let filter = data[11];
                let interlace = data[12];
                if bit_depth != 8 {
                    panic!("png: unsupported bit depth {bit_depth} (want 8)");
                }
                if color_type != 2 {
                    panic!("png: unsupported color type {color_type} (want 2, RGB)");
                }
                if compression != 0 || filter != 0 {
                    panic!("png: unsupported compression/filter method {compression}/{filter}");
                }
                if interlace != 0 {
                    panic!("png: interlaced PNG is unsupported (want 0)");
                }
                if w == 0 || h == 0 {
                    panic!("png: empty image {w}x{h}");
                }
            }
            b"IDAT" => {
                saw_idat = true;
                idat.extend_from_slice(data);
            }
            b"IEND" => {
                saw_iend = true;
                break;
            }
            // Ancillary chunks (tRNS, gAMA, pHYs, ...) carry no pixels and
            // are ignorable additions per the PNG spec. Critical unknown
            // chunks would be a different variant, so reject those.
            _ => {
                let critical = kind[0].is_ascii_uppercase();
                if critical {
                    panic!(
                        "png: unsupported critical chunk {:?}",
                        String::from_utf8_lossy(kind)
                    );
                }
            }
        }
        at = data_end + 4;
    }

    if !saw_ihdr {
        panic!("png: missing IHDR");
    }
    if !saw_idat {
        panic!("png: missing IDAT");
    }
    if !saw_iend {
        panic!("png: missing IEND");
    }

    let stride = w * 3;
    let mut z = ZlibDecoder::new(&idat[..]);
    let mut raw = Vec::new();
    z.read_to_end(&mut raw)
        .unwrap_or_else(|e| panic!("png: IDAT inflate failed: {e}"));
    if raw.len() != (stride + 1) * h {
        panic!(
            "png: inflated {} bytes, want {} for {}x{} RGB",
            raw.len(),
            (stride + 1) * h,
            w,
            h
        );
    }

    let mut rgb = vec![0u8; stride * h];
    let mut prev = vec![0u8; stride];
    for y in 0..h {
        let src = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let filter = src[0];
        let scan = &src[1..];
        let out = &mut rgb[y * stride..(y + 1) * stride];
        for x in 0..stride {
            let a = if x >= 3 { out[x - 3] } else { 0 }; // left
            let b = prev[x]; // up
            let c = if x >= 3 { prev[x - 3] } else { 0 }; // up-left
            let v = match filter {
                0 => scan[x],
                1 => scan[x].wrapping_add(a),
                2 => scan[x].wrapping_add(b),
                3 => scan[x].wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => scan[x].wrapping_add(paeth(a, b, c)),
                other => panic!("png: unknown scanline filter {other} on row {y}"),
            };
            out[x] = v;
        }
        prev.copy_from_slice(out);
    }

    Image { w, h, rgb }
}

/// The PNG Paeth predictor (spec 9.4): the nearest of left/up/up-left to
/// `a + b - c`.
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let pa = (p - a as i16).abs();
    let pb = (p - b as i16).abs();
    let pc = (p - c as i16).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    /// Encode one synthetic PNG of the supported variant. `filtered` is the
    /// already-filtered scanline body (one filter byte per row + data); the
    /// test supplies it so every filter arm can be exercised.
    fn wrap(w: usize, h: usize, filtered: &[u8]) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(&SIGNATURE);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &ihdr);
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(filtered).unwrap();
        let idat = z.finish().unwrap();
        chunk(&mut png, b"IDAT", &idat);
        chunk(&mut png, b"IEND", &[]);
        png
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&[0, 0, 0, 0]); // CRC: not read by decode
    }

    /// The source pixels: 3x2 RGB.
    fn pixels() -> Vec<u8> {
        vec![
            10, 20, 30, 40, 50, 60, 70, 80, 90, // row 0
            11, 21, 31, 41, 51, 61, 71, 81, 91, // row 1
        ]
    }

    fn filtered_with(filter: u8, prev: &[u8], row: &[u8]) -> Vec<u8> {
        let mut out = vec![filter];
        for x in 0..row.len() {
            let a = if x >= 3 { row[x - 3] } else { 0 };
            let b = prev[x];
            let c = if x >= 3 { prev[x - 3] } else { 0 };
            let pred = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => paeth(a, b, c),
                _ => unreachable!(),
            };
            out.push(row[x].wrapping_sub(pred));
        }
        out
    }

    #[test]
    fn decodes_every_scanline_filter() {
        let px = pixels();
        let (w, h) = (3usize, 2usize);
        let stride = w * 3;
        for filter in 0u8..=4 {
            let mut body = Vec::new();
            for y in 0..h {
                let row = &px[y * stride..(y + 1) * stride];
                let prev = if y == 0 {
                    vec![0u8; stride]
                } else {
                    px[(y - 1) * stride..y * stride].to_vec()
                };
                body.extend_from_slice(&filtered_with(filter, &prev, row));
            }
            let img = decode(&wrap(w, h, &body));
            assert_eq!((img.w, img.h), (w, h), "filter {filter}: geometry");
            assert_eq!(img.rgb, px, "filter {filter}: round-trip");
        }
    }

    #[test]
    #[should_panic(expected = "unsupported color type")]
    fn rejects_rgba() {
        let mut png = Vec::new();
        png.extend_from_slice(&SIGNATURE);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // color type 6 (RGBA)
        chunk(&mut png, b"IHDR", &ihdr);
        decode(&png);
    }

    #[test]
    #[should_panic(expected = "bad signature")]
    fn rejects_non_png() {
        decode(b"not a png");
    }
}
