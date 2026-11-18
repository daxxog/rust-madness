//! The `cfg(test)` rig the submodule test sections share: the stage-1
//! palette, the loaded applet, the ink/dump helpers and the pre-armed
//! music voices.

use super::*;

/// Stage 1's palette, the way `loadstage` applies it: the `snap` line,
/// then the `sky` line (whose setsky blends against that snap).
pub(crate) fn stage1_m() -> Medium {
    let mut m = Medium::new();
    m.setsnap(-5, -5, 20);
    m.setsky(200, 227, 255);
    m
}

/// The applet's xt after the constructor's archive spine.
pub(crate) fn loaded_xt() -> XtGraphics {
    let mut xt = XtGraphics::new();
    xt.loadpak();
    xt
}

pub(crate) fn rect_ink(cv: &Canvas, x0: i32, y0: i32, x1: i32, y1: i32) -> usize {
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y as usize * cv.w + x as usize) * 3;
            if i + 2 < cv.px.len() && (cv.px[i] != 0 || cv.px[i + 1] != 0 || cv.px[i + 2] != 0) {
                n += 1;
            }
        }
    }
    n
}

/// The shared procedural backdrop as ARGB ints (the fleximg the
/// finish pins assign directly).
pub(crate) fn pattern_px() -> Vec<u32> {
    (0..550 * 400)
        .map(|i| {
            let x = i % 550;
            let y = i / 550;
            0xFF00_0000
                | ((((x * 7 + y * 3) & 255) as u32) << 16)
                | ((((x * 13) & 255) as u32) << 8)
                | ((y * 11) & 255) as u32
        })
        .collect()
}

/// A stage-1 voice armed by hand (avoids re-mixing per route).
pub(crate) fn armed_stage1() -> XtGraphics {
    let mut xt = XtGraphics::new();
    let raw: &[u8] = include_bytes!("../../../../assets/music/stage1.zipo");
    let z = Zipo::read(raw).unwrap();
    let mut r = RadicalMod::new(&z.entries[0].data, 350, 8400, 135);
    r.play();
    xt.loadedt[0] = true;
    xt.stracks[0] = Some(r);
    xt
}

/// JpegGt dump layout: BE i32 w, BE i32 h, then w·h BE i32 ARGB.
pub(crate) fn dump(w: i32, h: i32, px: &[u32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + px.len() * 4);
    b.extend_from_slice(&(w as u32).to_be_bytes());
    b.extend_from_slice(&(h as u32).to_be_bytes());
    for p in px {
        b.extend_from_slice(&p.to_be_bytes());
    }
    b
}

/// The masked dump for the screen compares: JpegGt layout with the
/// band rows dropped (both sides mask the text).
pub(crate) fn dump_masked(cv: &Canvas, bands: &[(i32, i32)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + cv.w * cv.h * 4);
    b.extend_from_slice(&(cv.w as u32).to_be_bytes());
    b.extend_from_slice(&(cv.h as u32).to_be_bytes());
    for y in 0..cv.h as i32 {
        if bands.iter().any(|&(lo, hi)| (lo..=hi).contains(&y)) {
            continue;
        }
        for x in 0..cv.w as i32 {
            let i = (y as usize * cv.w + x as usize) * 3;
            let argb = 0xFF00_0000
                | ((cv.px[i] as u32) << 16)
                | ((cv.px[i + 1] as u32) << 8)
                | (cv.px[i + 2] as u32);
            b.extend_from_slice(&argb.to_be_bytes());
        }
    }
    b
}

/// The rect-mask dump variant (carselect's label columns): JpegGt
/// layout with the row bands and the half-open `(x0, x1, y0, y1)`
/// rects dropped.
pub(crate) fn dump_masked_rects(
    cv: &Canvas,
    bands: &[(i32, i32)],
    rects: &[(i32, i32, i32, i32)],
) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + cv.w * cv.h * 4);
    b.extend_from_slice(&(cv.w as u32).to_be_bytes());
    b.extend_from_slice(&(cv.h as u32).to_be_bytes());
    for y in 0..cv.h as i32 {
        if bands.iter().any(|&(lo, hi)| (lo..=hi).contains(&y)) {
            continue;
        }
        for x in 0..cv.w as i32 {
            if rects
                .iter()
                .any(|&(x0, x1, y0, y1)| x >= x0 && x < x1 && y >= y0 && y < y1)
            {
                continue;
            }
            let i = (y as usize * cv.w + x as usize) * 3;
            let argb = 0xFF00_0000
                | ((cv.px[i] as u32) << 16)
                | ((cv.px[i + 1] as u32) << 8)
                | (cv.px[i + 2] as u32);
            b.extend_from_slice(&argb.to_be_bytes());
        }
    }
    b
}

pub(crate) fn hex(b: &[u8; 16]) -> String {
    b.iter().map(|v| format!("{:02x}", v)).collect()
}
