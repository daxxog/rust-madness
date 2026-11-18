//! JPEG decoder — baseline sequential JFIF (SOI…SOF0…DQT/DHT…DRI…SOS…EOI)
//! to a per-pixel ARGB sprite (`gif::Gif`, opaque `0xFFRRGGBB`), feeding
//! the same draw paths as the GIF sprites. The applet's two JPEG archive
//! entries (`images2: bgmain.jpg` → `xt.bgmain`, `images3: stages.jpg` →
//! `xt.trackbg`) decodes through `Toolkit.createImage`'s IJG-derived native
//! decoder behind `loadimage`'s MediaTracker wait, and this port is
//! bit-exact against that JDK 1.8 decode (ground truth captured by
//! pixel-grabbing the exact `createImage(byte[])` chain the source calls —
//! the md5 pins in the test section).
//!
//! The pipeline follows the IJG defaults those streams decodes through: the
//! fixed-point `jpeg_idct_islow` inverse DCT (Loeffler-Ligtenberg-Moshovitz
//! 13-bit integer kernel, dequantizing inline with the raw quant values),
//! fancy h2v2 chroma upsampling (3/4-nearer + 1/4-farther horizontally and
//! vertically, with the alternating +8/+7 rounding dither), and the
//! 2^16-scaled YCbCr→RGB table conversion clamped through the post-IDCT
//! range-limit table (`prepare_range_limit_table`: the descaled sample,
//! masked to 10 bits, indexes a table that both clamps and applies the
//! +128 level shift the encoder folded away).

use std::fmt;

use crate::gif::Gif;

#[derive(Debug)]
pub struct JpegError(pub String);

impl fmt::Display for JpegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for JpegError {}

/// Canonical zigzag-to-raster coefficient order (JPEG spec figure A.6):
/// DQT value lists walk blocks in this order, and the IDCT dequantizes at
/// the natural positions they name.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, //
    17, 24, 32, 25, 18, 11, 4, 5, //
    12, 19, 26, 33, 40, 48, 41, 34, //
    27, 20, 13, 6, 7, 14, 21, 28, //
    35, 42, 49, 56, 57, 50, 43, 36, //
    29, 22, 15, 23, 30, 37, 44, 51, //
    58, 59, 52, 45, 38, 31, 39, 46, //
    53, 60, 61, 54, 47, 55, 62, 63,
];

/// The post-IDCT range-limit table — `jdmaster.c prepare_range_limit_table`'s
/// 10-bit masked table, laid out as the source comment describes: 128..255
/// (the +CENTERJSAMPLE identity window), then 255 repeated, then 0 repeated,
/// then 0..127 for the wrapped negatives.
fn range_limit() -> [u8; 1024] {
    let mut t = [0u8; 1024];
    for (i, v) in t.iter_mut().enumerate().take(128) {
        *v = (128 + i) as u8;
    }
    for v in t.iter_mut().take(512).skip(128) {
        *v = 255;
    }
    for (i, v) in t.iter_mut().enumerate().skip(896) {
        *v = (i - 896) as u8;
    }
    t
}

/// One Huffman table in `jdhuff.c`'s precomputed mincode/maxcode/valptr
/// form (symbol codes assigned canonically MSB-first, shortest first).
struct Huff {
    mincode: [i32; 17],
    maxcode: [i32; 17],
    valptr: [usize; 17],
    vals: Vec<u8>,
}

impl Huff {
    fn build(bits: &[u8; 17], vals: Vec<u8>) -> Result<Huff, JpegError> {
        let n = bits.iter().skip(1).fold(0usize, |a, &b| a + usize::from(b));
        if n != vals.len() {
            return Err(JpegError(format!(
                "huffman count sum {n} vs {} values",
                vals.len()
            )));
        }
        let mut t = Huff {
            mincode: [0; 17],
            maxcode: [-1; 17],
            valptr: [0; 17],
            vals,
        };
        let mut code = 0i32;
        let mut p = 0usize;
        for (l, &len) in bits.iter().enumerate().skip(1) {
            if len != 0 {
                t.valptr[l] = p;
                t.mincode[l] = code;
                p += usize::from(len);
                code += i32::from(len);
                t.maxcode[l] = code - 1;
            }
            code <<= 1;
        }
        Ok(t)
    }

    /// Canonical MSB-first decode step by step (`jdhuff.c`'s slow path).
    fn decode(&self, r: &mut BitReader) -> Result<u8, JpegError> {
        let mut code = 0i32;
        let mut l = 1usize;
        loop {
            if l > 16 {
                return Err(JpegError("huffman code overflow".to_string()));
            }
            code = (code << 1) | i32::from(r.next_bit()?);
            if code <= self.maxcode[l] {
                let idx = self.valptr[l] + (code - self.mincode[l]) as usize;
                return Ok(self.vals[idx]);
            }
            l += 1;
        }
    }
}

/// Entropy-coded-segment bit reader: byte stuffing (FF 00 → FF), padding
/// runs of FF, and marker discovery. Once a marker appears, entropy reads
/// fail and `marker` holds its code; the scan loop checks it or resyncs to
/// the next restart marker.
struct BitReader<'a> {
    d: &'a [u8],
    pos: usize,
    cur: u8,
    /// bits already consumed from `cur` (8 = nothing fetched)
    taken: u32,
    marker: Option<u8>,
}

impl<'a> BitReader<'a> {
    fn new(d: &'a [u8], pos: usize) -> Result<BitReader<'a>, JpegError> {
        if pos >= d.len() {
            return Err(JpegError("scan data missing".to_string()));
        }
        Ok(BitReader {
            d,
            pos,
            cur: 0,
            taken: 8,
            marker: None,
        })
    }

    /// Next byte of entropy data, un-stuffing FF 00 and stopping at every
    /// marker (FF xx ≠ FF 00; a run of FF is padding and steps over).
    fn data_byte(&mut self) -> Result<Option<u8>, JpegError> {
        loop {
            if self.marker.is_some() {
                return Ok(None);
            }
            let Some(&b) = self.d.get(self.pos) else {
                return Ok(None); // ran past the data: treat as scan end
            };
            if b != 0xFF {
                self.pos += 1;
                return Ok(Some(b));
            }
            match self.d.get(self.pos + 1).copied().unwrap_or(0) {
                0x00 => {
                    self.pos += 2;
                    return Ok(Some(0xFF));
                }
                0xFF => self.pos += 1, // padding byte — try the next one
                marker => {
                    // hold the marker code; the position moves past it
                    self.pos += 2;
                    self.marker = Some(marker);
                    return Ok(None);
                }
            }
        }
    }

    fn next_bit(&mut self) -> Result<u8, JpegError> {
        if self.taken == 8 {
            match self.data_byte()? {
                Some(b) => self.cur = b,
                None => return Err(JpegError("scan ran dry".to_string())),
            }
            self.taken = 0;
        }
        let b = (self.cur >> (7 - self.taken)) & 1;
        self.taken += 1;
        Ok(b)
    }

    fn bits(&mut self, n: u32) -> Result<u32, JpegError> {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | u32::from(self.next_bit()?);
        }
        Ok(v)
    }

    /// At a restart boundary the entropy data is byte-aligned, so discard
    /// any in-flight partial byte and expect this cycle's RSTn marker:
    /// either it already surfaced in the reader or it is the next thing in
    /// the data.
    fn resync(&mut self, cycle: usize) -> Result<(), JpegError> {
        self.taken = 8;
        let code = match self.marker.take() {
            Some(c) => c,
            None => {
                while self.pos < self.d.len() && self.d.get(self.pos) != Some(&0xFF) {
                    self.pos += 1;
                }
                while self.d.get(self.pos) == Some(&0xFF) {
                    self.pos += 1;
                }
                let c = self.d.get(self.pos).copied().unwrap_or(0);
                if c != 0 {
                    self.pos += 1;
                }
                c
            }
        };
        if !(0xD0..=0xD7).contains(&code) {
            return Err(JpegError(format!(
                "expected restart marker, found {code:02X}"
            )));
        }
        if code != 0xD0 + (cycle % 8) as u8 {
            return Err(JpegError("restart marker out of sequence".to_string()));
        }
        Ok(())
    }
}

/// One SOF component: sampling factors, quant table select, and the
/// per-scan Huffman table selects filled in at SOS.
struct Comp {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    tdc: usize,
    tac: usize,
}

/// Decode a baseline sequential JPEG. Only the stream class the applet's
/// assets use is accepted — baseline Huffman, 8-bit samples, YCbCr,
/// interleaved single scan — anything else is rejected the same way a bad
/// GIF is (the sprite stays empty, `loadpak` skips it).
pub fn decode(data: &[u8]) -> Result<Gif, JpegError> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(JpegError("not a JPEG (no SOI)".to_string()));
    }
    let mut pos = 2usize;
    let mut w = 0usize;
    let mut h = 0usize;
    let mut hmax = 0usize;
    let mut vmax = 0usize;
    let mut comps: Vec<Comp> = Vec::new();
    let mut qt: [[u16; 64]; 4] = [[0; 64]; 4];
    let mut htdc: [Option<Huff>; 4] = [None, None, None, None];
    let mut htac: [Option<Huff>; 4] = [None, None, None, None];
    let mut restart_interval = 0usize;

    // Marker walk to the first SOS; it leaves `scan_pos` at the entropy
    // data.
    let scan_pos: usize = loop {
        // land on the next marker: first FF, then the code, skipping fill
        while pos < data.len() && data[pos] != 0xFF {
            pos += 1;
        }
        if pos + 1 >= data.len() {
            return Err(JpegError("EOF before scan".to_string()));
        }
        while data.get(pos) == Some(&0xFF) {
            pos += 1;
        }
        let Some(&m) = data.get(pos) else {
            return Err(JpegError("EOF before scan".to_string()));
        };
        pos += 1;
        match m {
            // SOI / TEM / RSTn are standalone
            0xD8 | 0x01 | 0xD0..=0xD7 => {}
            0xD9 => return Err(JpegError("EOI before scan".to_string())),
            0xC0 => {
                // SOF0 baseline: prec(1) height(2) width(2) count(1) + 3/comps
                let len = seg_len(data, pos)?;
                let np = usize::from(data[pos + 7]);
                if len != 8 + 3 * np {
                    return Err(JpegError("bad SOF length".to_string()));
                }
                let dim = |at: usize| usize::from(u16::from_be_bytes([data[at], data[at + 1]]));
                h = dim(pos + 3);
                w = dim(pos + 5);
                if data[pos + 2] != 8 {
                    return Err(JpegError("non-8-bit samples".to_string()));
                }
                hmax = 0;
                vmax = 0;
                comps.clear();
                for c in 0..np {
                    let base = pos + 8 + 3 * c;
                    let ch = usize::from(data[base + 1] >> 4);
                    let cv = usize::from(data[base + 1] & 15);
                    comps.push(Comp {
                        id: data[base],
                        h: ch,
                        v: cv,
                        tq: usize::from(data[base + 2] & 3),
                        tdc: 0,
                        tac: 0,
                    });
                    hmax = hmax.max(ch);
                    vmax = vmax.max(cv);
                }
                if w == 0 || h == 0 || hmax == 0 || vmax == 0 {
                    return Err(JpegError("degenerate SOF".to_string()));
                }
                pos += len;
            }
            0xC1 | 0xC2 | 0xC3 | 0xC5..=0xCB | 0xCD..=0xCF => {
                return Err(JpegError(format!("unsupported SOF marker {m:02X}")));
            }
            0xC4 => {
                // DHT: several tables may ride inside one segment
                let len = seg_len(data, pos)?;
                let mut p = pos + 2;
                while p < pos + len {
                    let t = data[p];
                    let (class, id) = (usize::from(t >> 4), usize::from(t & 15));
                    if class > 1 || id > 3 {
                        return Err(JpegError(format!("bad DHT table {t:02X}")));
                    }
                    let mut bits = [0u8; 17];
                    for b in &mut bits[1..] {
                        *b = data[p + 1];
                        p += 1;
                    }
                    let n = bits.iter().skip(1).fold(0usize, |a, &b| a + usize::from(b));
                    let vals = data[p + 1..p + 1 + n].to_vec();
                    p += n + 1;
                    let table = Huff::build(&bits, vals)?;
                    if class == 0 {
                        htdc[id] = Some(table);
                    } else {
                        htac[id] = Some(table);
                    }
                }
                pos += len;
            }
            0xCC => return Err(JpegError("arithmetic coding unsupported".to_string())),
            0xDB => {
                // DQT: only 8-bit tables (the applet's assets) accepted
                let len = seg_len(data, pos)?;
                let mut p = pos + 2;
                while p < pos + len {
                    let pq = usize::from(data[p] >> 4);
                    let tq = usize::from(data[p] & 15);
                    if pq != 0 || tq > 3 {
                        return Err(JpegError(format!("bad DQT table {pq:X} {tq}")));
                    }
                    // values arrive in zigzag order; the IDCT consumes them
                    // at natural coefficient positions
                    let mut natural = [0u16; 64];
                    for i in 0..64 {
                        natural[ZIGZAG[i]] = u16::from(data[p + 1 + i]);
                    }
                    qt[tq] = natural;
                    p += 65;
                }
                pos += len;
            }
            0xDD => {
                let len = seg_len(data, pos)?;
                restart_interval = usize::from(u16::from_be_bytes([data[pos + 2], data[pos + 3]]));
                pos += len;
            }
            0xDA => {
                // SOS: per-component table selects; bounds must be 0..63
                let len = seg_len(data, pos)?;
                let ns = usize::from(data[pos + 2]);
                if len != 6 + 2 * ns {
                    return Err(JpegError("bad SOS length".to_string()));
                }
                let (ss, se, aa) = (
                    data[pos + 3 + 2 * ns],
                    data[pos + 4 + 2 * ns],
                    data[pos + 5 + 2 * ns],
                );
                if ss != 0 || se != 63 || aa != 0 {
                    return Err(JpegError("non-baseline scan bounds".to_string()));
                }
                if ns != comps.len() {
                    return Err(JpegError("scan does not cover all components".to_string()));
                }
                for s in 0..ns {
                    let base = pos + 3 + 2 * s;
                    let id = data[base];
                    let (tdc, tac) = (
                        usize::from(data[base + 1] >> 4),
                        usize::from(data[base + 1] & 15),
                    );
                    if tdc > 3 || tac > 3 {
                        return Err(JpegError("bad scan table select".to_string()));
                    }
                    let Some(c) = comps.iter_mut().find(|c| c.id == id) else {
                        return Err(JpegError("scan names an unknown component".to_string()));
                    };
                    c.tdc = tdc;
                    c.tac = tac;
                }
                pos += len;
                break pos;
            }
            m if (0xE0..=0xEF).contains(&m) || m == 0xFE => {
                // APPn / COM: skipped whole
                pos += seg_len(data, pos)?;
            }
            _ => return Err(JpegError(format!("unsupported marker {m:02X}"))),
        }
    };

    // Sample planes, block-raster layout, one per component.
    let mcu_cols = w.div_ceil(8 * hmax);
    let mcu_rows = h.div_ceil(8 * vmax);
    let strides: Vec<usize> = comps.iter().map(|c| mcu_cols * c.h * 8).collect();
    let rowcounts: Vec<usize> = comps.iter().map(|c| mcu_rows * c.v * 8).collect();
    let mut planes: Vec<Vec<u8>> = strides
        .iter()
        .zip(&rowcounts)
        .map(|(&s, &n)| vec![0u8; s * n])
        .collect();
    let rl = range_limit();

    // Entropy decode of the interleaved scan.
    let mut r = BitReader::new(data, scan_pos)?;
    let mut preds = vec![0i32; comps.len()];
    let mut coef = [0i32; 64];
    let mut mcu_index = 0usize;
    for row in 0..mcu_rows {
        for col in 0..mcu_cols {
            if restart_interval > 0
                && mcu_index != 0
                && usize::is_multiple_of(mcu_index, restart_interval)
            {
                r.resync(mcu_index / restart_interval - 1)?;
                preds.fill(0);
            }
            for (ci, c) in comps.iter().enumerate() {
                for vy in 0..c.v {
                    for hx in 0..c.h {
                        coef.fill(0);
                        // DC coefficient: category, then the diff bits
                        let dc = htdc[c.tdc]
                            .as_ref()
                            .ok_or_else(|| JpegError("missing DC table".to_string()))?;
                        let s = usize::from(dc.decode(&mut r)?);
                        if s > 11 {
                            return Err(JpegError(format!("DC size {s}")));
                        }
                        let diff = extend(r.bits(s as u32)?, s as u32);
                        preds[ci] += diff;
                        coef[0] = preds[ci];
                        // AC coefficients: (run,size) symbols
                        let ac = htac[c.tac]
                            .as_ref()
                            .ok_or_else(|| JpegError("missing AC table".to_string()))?;
                        let mut k = 1usize;
                        while k < 64 {
                            let rs = usize::from(ac.decode(&mut r)?);
                            let run = rs >> 4;
                            let sz = rs & 15;
                            if sz == 0 {
                                if run == 15 {
                                    k += 16; // ZRL: 15 zeros + this position
                                    continue;
                                }
                                break; // EOB
                            }
                            if k + run > 63 {
                                return Err(JpegError("AC run past the block".to_string()));
                            }
                            k += run;
                            coef[ZIGZAG[k]] = extend(r.bits(sz as u32)?, sz as u32);
                            k += 1;
                        }
                        let quant = &qt[c.tq];
                        let bx = col * c.h + hx;
                        let by = row * c.v + vy;
                        idct_block(&coef, quant, &rl, &mut planes[ci], strides[ci], bx, by);
                    }
                }
            }
            mcu_index += 1;
        }
    }

    // YCbCr → RGB through the fixed-point tables.
    if comps.len() != 3 {
        return Err(JpegError("not YCbCr".to_string()));
    }
    colorize(&planes, &strides, &comps, hmax, vmax, h, w)
}

/// Sign-extend a raw `sz`-bit category value (JPEG spec figure F.5).
fn extend(v: u32, sz: u32) -> i32 {
    if sz == 0 {
        0
    } else if v < (1 << (sz - 1)) {
        v as i32 - (1 << sz) + 1
    } else {
        v as i32
    }
}

/// Marker segment length: bytes at `at..at+2`, INCLUDING those two bytes.
fn seg_len(data: &[u8], at: usize) -> Result<usize, JpegError> {
    let n = usize::from(u16::from_be_bytes([data[at], data[at + 1]]));
    if n < 2 || at + n > data.len() {
        return Err(JpegError("bad marker length".to_string()));
    }
    Ok(n)
}

/// Dequantization + inverse DCT for one 8×8 block — `jpeg_idct_islow`
/// (jidctint.c, 13-bit constant set), writing the samples into the
/// component plane at the block's raster offset.
#[allow(clippy::too_many_lines)]
fn idct_block(
    coef: &[i32; 64],
    qt: &[u16; 64],
    rl: &[u8; 1024],
    plane: &mut [u8],
    stride: usize,
    bx: usize,
    by: usize,
) {
    const CONST_BITS: i32 = 13;
    const PASS1_BITS: i32 = 2;
    let de = |i: usize| i64::from(coef[i]) * i64::from(qt[i]);
    let mut wz = [0i64; 64];

    // Pass 1: columns (results carry a sqrt(8)·2^PASS1_BITS scale)
    for j in 0..8usize {
        if (1..8).all(|i| coef[8 * i + j] == 0) {
            // AC terms all zero: every sample is the dequantized DC
            let dcval = de(j) << PASS1_BITS;
            for wzj in wz.iter_mut().skip(j).step_by(8) {
                *wzj = dcval;
            }
            continue;
        }
        // even part — the rotator is sqrt(2)·c(-6)
        let z2 = de(8 * 2 + j);
        let z3 = de(8 * 6 + j);
        let z1 = (z2 + z3) * M0_541196100;
        let tmp2 = z1 + z3 * -M1_847759065;
        let tmp3 = z1 + z2 * M0_765366865;
        let z2 = de(j);
        let z3 = de(8 * 4 + j);
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;
        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;
        // odd part — figure 8: i0..i3 are y7,y5,y3,y1 respectively
        let mut t0 = de(8 * 7 + j);
        let mut t1 = de(8 * 5 + j);
        let mut t2 = de(8 * 3 + j);
        let mut t3 = de(8 + j);
        let q1 = (t0 + t3) * -M0_899976223;
        let q2 = (t1 + t2) * -M2_562915447;
        let q3 = (t0 + t2) * -M1_961570560;
        let q4 = (t1 + t3) * -M0_390180644;
        let q5 = ((t0 + t2) + (t1 + t3)) * M1_175875602;
        t0 *= M0_298631336;
        t1 *= M2_053119869;
        t2 *= M3_072711026;
        t3 *= M1_501321110;
        let q3 = q3 + q5;
        let q4 = q4 + q5;
        let t0 = t0 + q1 + q3;
        let t1 = t1 + q2 + q4;
        let t2 = t2 + q2 + q3;
        let t3 = t3 + q1 + q4;
        wz[j] = descale(tmp10 + t3, CONST_BITS - PASS1_BITS);
        wz[8 * 7 + j] = descale(tmp10 - t3, CONST_BITS - PASS1_BITS);
        wz[8 + j] = descale(tmp11 + t2, CONST_BITS - PASS1_BITS);
        wz[8 * 6 + j] = descale(tmp11 - t2, CONST_BITS - PASS1_BITS);
        wz[8 * 2 + j] = descale(tmp12 + t1, CONST_BITS - PASS1_BITS);
        wz[8 * 5 + j] = descale(tmp12 - t1, CONST_BITS - PASS1_BITS);
        wz[8 * 3 + j] = descale(tmp13 + t0, CONST_BITS - PASS1_BITS);
        wz[8 * 4 + j] = descale(tmp13 - t0, CONST_BITS - PASS1_BITS);
    }

    // Pass 2: rows, out through the masked range-limit table
    for i in 0..8usize {
        let row = &mut plane[8 * by * stride + 8 * bx + i * stride..];
        if (1..8).all(|j| wz[8 * i + j] == 0) {
            let v = rl[(descale(wz[8 * i], PASS1_BITS + 3) & 1023) as usize];
            row[..8].copy_from_slice(&[v; 8]);
            continue;
        }
        let z2 = wz[8 * i + 2];
        let z3 = wz[8 * i + 6];
        let z1 = (z2 + z3) * M0_541196100;
        let tmp2 = z1 + z3 * -M1_847759065;
        let tmp3 = z1 + z2 * M0_765366865;
        let tmp0 = (wz[8 * i] + wz[8 * i + 4]) << CONST_BITS;
        let tmp1 = (wz[8 * i] - wz[8 * i + 4]) << CONST_BITS;
        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;
        let mut t0 = wz[8 * i + 7];
        let mut t1 = wz[8 * i + 5];
        let mut t2 = wz[8 * i + 3];
        let mut t3 = wz[8 * i + 1];
        let q1 = (t0 + t3) * -M0_899976223;
        let q2 = (t1 + t2) * -M2_562915447;
        let q3 = (t0 + t2) * -M1_961570560;
        let q4 = (t1 + t3) * -M0_390180644;
        let q5 = ((t0 + t2) + (t1 + t3)) * M1_175875602;
        t0 *= M0_298631336;
        t1 *= M2_053119869;
        t2 *= M3_072711026;
        t3 *= M1_501321110;
        let q3 = q3 + q5;
        let q4 = q4 + q5;
        let t0 = t0 + q1 + q3;
        let t1 = t1 + q2 + q4;
        let t2 = t2 + q2 + q3;
        let t3 = t3 + q1 + q4;
        row[0] = limit(rl, descale(tmp10 + t3, CONST_BITS + PASS1_BITS + 3));
        row[7] = limit(rl, descale(tmp10 - t3, CONST_BITS + PASS1_BITS + 3));
        row[1] = limit(rl, descale(tmp11 + t2, CONST_BITS + PASS1_BITS + 3));
        row[6] = limit(rl, descale(tmp11 - t2, CONST_BITS + PASS1_BITS + 3));
        row[2] = limit(rl, descale(tmp12 + t1, CONST_BITS + PASS1_BITS + 3));
        row[5] = limit(rl, descale(tmp12 - t1, CONST_BITS + PASS1_BITS + 3));
        row[3] = limit(rl, descale(tmp13 + t0, CONST_BITS + PASS1_BITS + 3));
        row[4] = limit(rl, descale(tmp13 - t0, CONST_BITS + PASS1_BITS + 3));
    }
}

/// jidctint.c's DESCALE: arithmetic right shift with half-up rounding.
fn descale(x: i64, n: i32) -> i64 {
    (x + (1 << (n - 1))) >> n
}

fn limit(rl: &[u8; 1024], v: i64) -> u8 {
    rl[(v & 1023) as usize]
}

/// jidctint.c's 13-bit constant set (FIX(cos) pre-scaled by 2^13).
const M0_541196100: i64 = 4433;
const M1_847759065: i64 = 15_137;
const M0_765366865: i64 = 6270;
const M1_175875602: i64 = 9633;
const M0_298631336: i64 = 2446;
const M2_053119869: i64 = 16_819;
const M3_072711026: i64 = 25_172;
const M1_501321110: i64 = 12_299;
const M0_899976223: i64 = 7373;
const M2_562915447: i64 = 20_995;
const M1_961570560: i64 = 16_069;
const M0_390180644: i64 = 3196;

/// YCbCr → RGB fixed-point constants, 2^16-scaled (`jdcolor.c` FIX set).
const FIX_1_40200: i64 = 91_881;
const FIX_1_77200: i64 = 116_130;
const FIX_0_34414: i64 = 22_554;
const FIX_0_71414: i64 = 46_802;

/// Convert the YCbCr planes into an opaque ARGB sprite: a full-size
/// component rides as-is (`fullsize_upsample`), a 2h2v downsampled one
/// goes through the fancy upsample (`h2v2_fancy_upsample`), and any other
/// sampling is rejected; then `ycc_rgb_convert_internal`'s table math
/// per pixel.
fn colorize(
    planes: &[Vec<u8>],
    strides: &[usize],
    comps: &[Comp],
    hmax: usize,
    vmax: usize,
    h: usize,
    w: usize,
) -> Result<Gif, JpegError> {
    assert_eq!(comps.len(), 3);
    for c in comps {
        let full = c.h == hmax && c.v == vmax;
        let half = c.h * 2 == hmax && c.v * 2 == vmax;
        if !full && !half {
            return Err(JpegError("unsupported sampling factors".to_string()));
        }
    }
    let yplane = fullsize(&planes[0], strides[0], h, w);
    let cbplane = if comps[1].h * 2 == hmax {
        upsample2(&planes[1], strides[1], h, w)
    } else {
        fullsize(&planes[1], strides[1], h, w)
    };
    let crplane = if comps[2].h * 2 == hmax {
        upsample2(&planes[2], strides[2], h, w)
    } else {
        fullsize(&planes[2], strides[2], h, w)
    };

    // build_ycc_rgb_table
    let mut crr = [0i32; 256];
    let mut cbb = [0i32; 256];
    let mut crg = [0i64; 256];
    let mut cbg = [0i64; 256];
    for i in 0..256usize {
        let x = i as i64 - 128;
        crr[i] = ((FIX_1_40200 * x + 32_768) >> 16) as i32;
        cbb[i] = ((FIX_1_77200 * x + 32_768) >> 16) as i32;
        crg[i] = -FIX_0_71414 * x;
        cbg[i] = -FIX_0_34414 * x + 32_768;
    }

    // ycc_rgb_convert
    let mut px = vec![0u32; w * h];
    for row in 0..h {
        for col in 0..w {
            let y = i32::from(yplane[row * w + col]);
            let cb = i32::from(cbplane[row * w + col]);
            let cr = i32::from(crplane[row * w + col]);
            let r = (y + crr[cr as usize]).clamp(0, 255) as u32;
            let g = (y + ((cbg[cb as usize] + crg[cr as usize]) >> 16) as i32).clamp(0, 255) as u32;
            let b = (y + cbb[cb as usize]).clamp(0, 255) as u32;
            px[row * w + col] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
        }
    }
    Ok(Gif { w, h, px })
}

/// Copy the useful (image-sized) rows of a full-size component.
fn fullsize(plane: &[u8], stride: usize, h: usize, w: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h];
    for row in 0..h {
        out[row * w..(row + 1) * w].copy_from_slice(&plane[row * stride..row * stride + w]);
    }
    out
}

/// Fancy 2×2 upsampling of a downsampled plane (`h2v2_fancy_upsample`):
/// the vertical 3:1 blend toward the current row, then the horizontal
/// neighborhood of that blend, evaluated with the source's +8/+7 rounding
/// dither and edge rows replicated.
fn upsample2(plane: &[u8], plane_w: usize, h: usize, w: usize) -> Vec<u8> {
    let dw = w.div_ceil(2);
    let dh = h.div_ceil(2);
    let mut out = vec![0u8; w * h];
    let mut colsum = vec![0i32; dw];
    for j in 0..dh {
        let base = j * plane_w;
        let cur = &plane[base..base + dw];
        let prev: &[u8] = if j == 0 {
            cur
        } else {
            let b = (j - 1) * plane_w;
            &plane[b..b + dw]
        };
        let next: &[u8] = if j + 1 >= dh {
            cur
        } else {
            let b = (j + 1) * plane_w;
            &plane[b..b + dw]
        };
        for v in 0..2usize {
            let other = if v == 0 { prev } else { next };
            for (c, cs) in colsum.iter_mut().enumerate().take(dw) {
                *cs = 3 * i32::from(cur[c]) + i32::from(other[c]);
            }
            let orow = 2 * j + v;
            if orow >= h {
                break;
            }
            let row = &mut out[orow * w..];
            for (c, &this) in colsum.iter().enumerate().take(dw) {
                let leftc = colsum[if c == 0 { 0 } else { c - 1 }];
                let rightc = colsum[if c + 1 >= dw { dw - 1 } else { c + 1 }];
                if 2 * c < w {
                    row[2 * c] = ((this * 3 + leftc + 8) >> 4) as u8;
                }
                if 2 * c + 1 < w {
                    row[2 * c + 1] = ((this * 3 + rightc + 7) >> 4) as u8;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::md5;
    use crate::zipo::Zipo;

    /// The PixelGrabber dump layout: BE u32 width, BE u32 height, then one
    /// BE u32 ARGB int per pixel — exactly what the JDK ground truth run
    /// hashed.
    fn dump_bytes(g: &Gif) -> Vec<u8> {
        let mut b = Vec::with_capacity(8 + g.px.len() * 4);
        b.extend_from_slice(&(g.w as u32).to_be_bytes());
        b.extend_from_slice(&(g.h as u32).to_be_bytes());
        for &p in &g.px {
            b.extend_from_slice(&p.to_be_bytes());
        }
        b
    }

    /// Both archive JPEGs decode pixel-exact to the JDK 1.8 Toolkit
    /// decode (550×400, fully opaque, one md5 each was recorded from the
    /// pixel-grabbed `createImage` result).
    #[test]
    fn decodes_both_archives_to_the_jdk_ground_truth() {
        // JFIF marker sanity of the test's md5 helper first
        assert_eq!(md5(b"abc"), md5(b"abc"));
        let abc: [u8; 16] = [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
            0x7f, 0x72,
        ];
        assert_eq!(md5(b"abc"), abc, "md5 helper is wrong");

        let raw: &[u8] = include_bytes!("../../../assets/graphics/images2.zipo");
        let z = Zipo::read(raw).unwrap();
        let entry = z.get("bgmain.jpg").unwrap();
        let g = decode(&entry.data).unwrap();
        assert_eq!((g.w, g.h), (550, 400));
        assert!(g.px.iter().all(|&p| p & 0xFF00_0000 != 0), "opaque ARGB");
        // eye "JpegGt bgmain.jpg bgmain.gt" under JDK 1.8.0_40
        let want: [u8; 16] = [
            0x61, 0xc1, 0xc8, 0x93, 0xa7, 0x95, 0x87, 0x0b, 0x31, 0x9c, 0x1d, 0x84, 0x70, 0xa7,
            0x47, 0xad,
        ];
        assert_eq!(md5(&dump_bytes(&g)), want, "bgmain.jpg pixel mismatch");

        let raw: &[u8] = include_bytes!("../../../assets/graphics/images3.zipo");
        let z = Zipo::read(raw).unwrap();
        let entry = z.get("stages.jpg").unwrap();
        let g = decode(&entry.data).unwrap();
        assert_eq!((g.w, g.h), (550, 400));
        assert!(g.px.iter().all(|&p| p & 0xFF00_0000 != 0), "opaque ARGB");
        // stages.gt
        let want: [u8; 16] = [
            0xa4, 0x95, 0xc9, 0xc8, 0xf0, 0xfa, 0x9f, 0x7c, 0x7d, 0xc3, 0xb4, 0x4c, 0xca, 0x2d,
            0x63, 0x3a,
        ];
        assert_eq!(md5(&dump_bytes(&g)), want, "stages.jpg pixel mismatch");
    }
}
