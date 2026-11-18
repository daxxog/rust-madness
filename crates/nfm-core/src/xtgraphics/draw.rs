// FIDELITY: xtGraphics.drawcs()/rot() keep the Java method arity (decompiled XtGraphics.java signatures).
#![allow(clippy::too_many_arguments)]
//! The sprite pixel transforms (`loadsnap`/`loadopsnap`/`pressed`/
//! `bressed`), `drawcs`, and the pure projection helpers (`xs`/`ys`/`py`/
//! `rot`). The free functions keep their `XtGraphics`-level paths (re-
//! exported from `mod.rs`); `drawcs` is the centered snap-adjusted string
//! the whole screen layer paints with.

use super::*;

/// The magic gray `-4144960` — the HUD sprites key their sky glass with it
/// and `loadsnap` swaps it for the stage sky color.
pub const GRAY: u32 = 0xFFC0_C0C0;

/// `xtGraphics.loadsnap(Image)` (xt:2569): re-tint the HUD sprites with the
/// stage's snap palette (`m.snap`, set by the stage file's `snap` line).
///
/// The source round-trips the image through `PixelGrabber` into an `int[]`
/// and rebuilds it with `MemoryImageSource`; on ARGB values that round-trip
/// is the identity, so the transform runs on the pixels in place:
/// - the magic gray `0xFFC0C0C0` becomes the sky color `m.csky` (already
///   snap-blended and clamped 0..255 by `Medium.setsky`, so the `new
///   Color(r, g, b)` range check in the JDK source can never throw here),
/// - the LAST pixel's value is the transparency key and is kept untouched —
///   for the opaque gray sprites (lap, the digit panels, position, wasted)
///   the corner pixel IS the gray, and the gray check is evaluated FIRST in
///   the source's if/else-if, so the key never gets a chance: those sprites
///   come back fully opaque sky-glass panels,
/// - every other pixel brightens per channel and re-wraps opaque — even an
///   alpha-0 pixel not matching the key turns opaque (the 3-int `Color`
///   constructor defaults alpha to 255).
pub fn loadsnap(img: &Gif, m: &Medium) -> Gif {
    let mut px = img.px.clone();
    let key = px[img.w * img.h - 1];
    for p in px.iter_mut() {
        if *p != GRAY && *p != key {
            let r = snap_channel(((*p >> 16) & 0xFF) as i32, m.snap[0]);
            let g = snap_channel(((*p >> 8) & 0xFF) as i32, m.snap[1]);
            let b = snap_channel((*p & 0xFF) as i32, m.snap[2]);
            *p = color::rgb(r, g, b);
        } else if *p == GRAY {
            *p = color::rgb(m.csky[0], m.csky[1], m.csky[2]);
        }
    }
    Gif {
        w: img.w,
        h: img.h,
        px,
    }
}

/// One `loadsnap` channel: Java
/// `(int)((float)c + (float)c * ((float)snap / 100.0F))`, then clamped to
/// 0..225 — the HUD sprites cap at 225, not 255.
fn snap_channel(c: i32, snap: i32) -> i32 {
    let v = (c as f32 + c as f32 * (snap as f32 / 100.0f32)) as i32;
    v.clamp(0, 225)
}

/// `xtGraphics.loadopsnap(Image, int stage)` (xt:3909): the loading-music
/// and star sprites take the stage's `hipno` variant. The transparency key
/// is pixel index 76 here (the source reads it blindly — a shorter image
/// dies the same `ArrayIndexOutOfBoundsException` death as this index
/// would), and the palette adjusts per channel: when `hipno[stage - 1]` is
/// non-zero the pixel is pushed by
/// `c - c * snap[ch] / (50 * hipno[stage - 1])` (clamped 0..255), otherwise
/// red brightens ×1.25, green passes through raw, and blue darkens ×0.75 —
/// with the blue line running in `double` where the rest is `float`; kept.
pub fn loadopsnap(img: &Gif, stage: i32, hipno: &[i32], m: &Medium) -> Gif {
    let mut px = img.px.clone();
    let key = px[76];
    let hip = hipno[(stage - 1) as usize];
    for p in px.iter_mut() {
        if *p == key {
            continue;
        }
        let r = ((*p >> 16) & 0xFF) as i32;
        let g = ((*p >> 8) & 0xFF) as i32;
        let b = (*p & 0xFF) as i32;
        let (r, g, b) = if hip != 0 {
            (
                opsnap_channel(r, m.snap[0], hip),
                opsnap_channel(g, m.snap[1], hip),
                opsnap_channel(b, m.snap[2], hip),
            )
        } else {
            let r = (r as f32 + r as f32 * 0.25f32) as i32;
            let r = r.clamp(0, 255);
            // green untouched by the source (var10 = var8.getGreen())
            let b = (b as f64 - b as f64 * 0.25) as i32;
            let b = b.clamp(0, 255);
            (r, g, b)
        };
        *p = color::rgb(r, g, b);
    }
    Gif {
        w: img.w,
        h: img.h,
        px,
    }
}

/// One `loadopsnap` hipno channel: Java
/// `(int)((float)c - (float)c * ((float)snap / (float)(50 * hip)))`, clamped
/// 0..255. `hip` is non-zero on this path (the source branches on it), and
/// the `50 * hip` product happens in `int` before the float cast.
fn opsnap_channel(c: i32, snap: i32, hip: i32) -> i32 {
    let v = (c as f32 - c as f32 * (snap as f32 / (50 * hip) as f32)) as i32;
    v.clamp(0, 255)
}

/// `xtGraphics.pressed(Image)` (xt:1017): the pressed button state — every
/// pixel that is not the transparency key becomes opaque black, leaving the
/// button's silhouette (the key skips at blit time).
pub fn pressed(img: &Gif) -> Gif {
    if img.px.is_empty() {
        // The source dereferences the applet's `null` Image here
        // (xt:1017-1018: the `getWidth(null)`/PixelGrabber pair), i.e.
        // a NullPointerException; the port's unfetched sprite is the
        // empty gif, so the same death is raised with the source's name
        // (the no-pak-3 mirror pin).
        panic!("NullPointerException: pressed() on an unfetched image (xtGraphics.java:1017-1018)");
    }
    let mut px = img.px.clone();
    let key = px[px.len() - 1];
    for p in px.iter_mut() {
        if *p != key {
            *p = 0xFF00_0000;
        }
    }
    Gif {
        w: img.w,
        h: img.h,
        px,
    }
}

/// `xtGraphics.bressed(Image)` (xt:2875): the pressed variant in the pale
/// lavender `new Color(206, 214, 255)`.
pub fn bressed(img: &Gif) -> Gif {
    if img.px.is_empty() {
        // The same null-image death as `pressed` (both are called from
        // the ctor's tail on the OLD images; the pin is the no-pak-4
        // arm).
        panic!("NullPointerException: bressed() on an unfetched image (xtGraphics.java:2875)");
    }
    let mut px = img.px.clone();
    let key = px[px.len() - 1];
    for p in px.iter_mut() {
        if *p != key {
            *p = color::rgb(206, 214, 255);
        }
    }
    Gif {
        w: img.w,
        h: img.h,
        px,
    }
}

// ===========================================================================
// The xtGraphics state machine

impl XtGraphics {
    /// `xtGraphics.drawcs` (xt:2414-2472): a snap-adjusted centered string at
    /// the Java Graphics font `size`. `mode`: 0 plain, 1 the +1/+1 black
    /// shadow, 2 the sky-tinted shadow, 3 the raw color, 4 the snap-darkened
    /// color. The source's three color args travel as one `setColor` triple
    /// here, matching the Canvas API's shape. Java centers with
    /// `this.ftm.stringWidth(s)`, where `ftm` is the metrics of the font the
    /// caller last set (`xt:2463-2472`), so the same `size` drives both the
    /// width and the raster.
    pub fn drawcs(
        &self,
        cv: &mut Canvas,
        size: i32,
        y: i32,
        s: &str,
        c: [i32; 3],
        mode: i32,
        m: &Medium,
    ) {
        let mut c = c;
        if mode != 3 && mode != 4 {
            c = [
                lift(c[0], m.snap[0]),
                lift(c[1], m.snap[1]),
                lift(c[2], m.snap[2]),
            ];
        }
        if mode == 4 {
            c = [
                sink(c[0], m.snap[0]),
                sink(c[1], m.snap[1]),
                sink(c[2], m.snap[2]),
            ];
        }
        if mode == 1 {
            cv.draw_string(
                s,
                275 - crate::xtfont::string_width(s, size) / 2 + 1,
                y + 1,
                [0, 0, 0],
                size,
            );
        }
        if mode == 2 {
            let shadow = [
                (c[0] + m.csky[0] * 2) / 3,
                (c[1] + m.csky[1] * 2) / 3,
                (c[2] + m.csky[2] * 2) / 3,
            ];
            cv.draw_string(
                s,
                275 - crate::xtfont::string_width(s, size) / 2 + 1,
                y + 1,
                clampc(shadow),
                size,
            );
        }
        cv.draw_string(
            s,
            275 - crate::xtfont::string_width(s, size) / 2,
            y,
            clampc(c),
            size,
        );
    }
}

/// `xtGraphics.xs` (xt:3442) — the camera-space x projection toward the
/// screen center; the depth clamps at 50. The arrow's depths (650..810)
/// keep the arithmetic inside i32.
pub fn xs(v: i32, d: i32, m: &Medium) -> i32 {
    let d = if d < 50 { 50 } else { d };
    (d - m.focus_point) * (m.cx - v) / d + v
}

/// `xtGraphics.ys` (xt:999) — the y twin of `xs`.
pub fn ys(v: i32, d: i32, m: &Medium) -> i32 {
    let d = if d < 50 { 50 } else { d };
    (d - m.focus_point) * (m.cy - v) / d + v
}

/// `xtGraphics.py` (xt:2475) — the squared 100-unit-cell distance, in
/// Java's wrapping `int` arithmetic (the pure-helper overflow pin).
pub fn py(x1: i32, x2: i32, z1: i32, z2: i32) -> i32 {
    let dx = x1.wrapping_sub(x2);
    let dz = z1.wrapping_sub(z2);
    dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz))
}

/// `xtGraphics.rot` (xt:1115): rotate n (x, z) points around (cx, cy) by
/// ang degrees, through `Medium.cos`/`sin`.
pub fn rot(xs: &mut [i32], ys: &mut [i32], cx: i32, cy: i32, ang: i32, n: usize, m: &Medium) {
    if ang != 0 {
        for i in 0..n {
            let x = xs[i];
            let z = ys[i];
            xs[i] = cx + ((x - cx) as f32 * m.cos(ang) - (z - cy) as f32 * m.sin(ang)) as i32;
            ys[i] = cy + ((x - cx) as f32 * m.sin(ang) + (z - cy) as f32 * m.cos(ang)) as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gif(w: usize, h: usize, px: &[u32]) -> Gif {
        assert_eq!(px.len(), w * h);
        Gif {
            w,
            h,
            px: px.to_vec(),
        }
    }

    /// A `Medium` the way `snap(stage)` sees one: fresh, then the stage
    /// file's `snap` line and the (already snap-blended, clamped) `csky`.
    fn m(snap: [i32; 3], csky: [i32; 3]) -> Medium {
        let mut med = Medium::new();
        med.snap = snap;
        med.csky = csky;
        med
    }

    #[test]
    fn loadsnap_brightens_gray_becomes_sky_key_is_kept() {
        let med = m([25, -50, 100], [1, 2, 3]);
        let img = gif(
            1,
            4,
            &[
                0xFF10_1010, // (16,16,16) color pixel
                0xFFC0_C0C0, // the magic gray
                0x0011_2233, // alpha 0, but NOT the key
                0x0099_AABB, // the key (last pixel)
            ],
        );
        let out = loadsnap(&img, &med);
        assert_eq!(
            out.px,
            vec![
                0xFF14_0820, // r 16+16*0.25=20, g 16-16*0.5=8, b 16+16=32
                0xFF01_0203, // gray -> csky
                0xFF15_1166, // r 17+17*0.25=21, g 34-17=17, b 51+51=102 —
                // and alpha 0 turns opaque (3-int Color)
                0x0099_AABB, // key pixel untouched
            ]
        );
        assert_eq!((out.w, out.h), (1, 4));
    }

    #[test]
    fn loadsnap_clamps_channels_to_225() {
        let med = m([100, -100, -1000], [0, 0, 0]);
        let img = gif(2, 1, &[0xFFC8_C8C8, 0x0000_0001]);
        let out = loadsnap(&img, &med);
        // r 200+200*1.0=400 -> 225; g 200-200=0; b 200-2000 -> 0
        assert_eq!(out.px, vec![0xFFE1_0000, 0x0000_0001]);
    }

    #[test]
    fn loadopsnap_hipno_push_and_zero_hipno_split() {
        // 10x10 so pixel index 76 exists; everything is the key except the
        // probe pixels at 0 and 1.
        let key = 0x0012_3456;
        let mut px = [key; 100];
        px[0] = 0xFF64_6464; // (100,100,100)
        px[1] = 0xFFFF_FFFF; // (255,255,255)
        px[76] = key;
        let img = gif(10, 10, &px);
        let hipno = [0, 1]; // stage 1 -> 0 (else branch), stage 2 -> 1

        // stage 2: denom 50, snap (-100,100,0): r 100-100*(-2)=300->255,
        // g 100-100*2 -> 0, b 100-0=100; caps: r 255+510 -> 255, g -> 0
        let out = loadopsnap(&img, 2, &hipno, &m([-100, 100, 0], [0, 0, 0]));
        assert_eq!(out.px[0], 0xFFFF_0064);
        assert_eq!(out.px[1], 0xFFFF_00FF);
        assert_eq!(out.px[76], key);
        assert_eq!(out.px[99], key);

        // stage 1: hipno 0 — red x1.25, green raw, blue x0.75 in f64:
        // (100,100,100) -> (125,100,75); (255,255,255) -> r 318.75->255,
        // g 255, b 191.25->191
        let out = loadopsnap(&img, 1, &hipno, &m([0, 0, 0], [0, 0, 0]));
        assert_eq!(out.px[0], 0xFF7D_644B);
        assert_eq!(out.px[1], 0xFFFF_FFBF);
        assert_eq!(out.px[76], key);
        assert_eq!(out.px[99], key);
    }

    #[test]
    fn pressed_and_bressed_silhouettes() {
        let img = gif(
            2,
            2,
            &[
                0xFF12_3456,
                0x00AB_CDEF,
                0x0099_AABB, // the key
                0x0099_AABB, // ...also the key (corner)
            ],
        );
        assert_eq!(
            pressed(&img).px,
            vec![0xFF00_0000, 0xFF00_0000, 0x0099_AABB, 0x0099_AABB]
        );
        assert_eq!(
            bressed(&img).px,
            vec![0xFFCE_D6FF, 0xFFCE_D6FF, 0x0099_AABB, 0x0099_AABB]
        );
    }

    #[test]
    fn loadsnap_on_real_damage_sprite() {
        // images4.zipo carries the HUD meters; damage.gif measured (scratch
        // scan): 318 magic-gray px, 1135 transparent (key-colored) px, the
        // corner transparent — the transparent-framed meter shape.
        let raw: &[u8] = include_bytes!("../../../../assets/graphics/images4.zipo");
        let zipo = Zipo::read(raw).unwrap();
        let entry = zipo.get("damage.gif").unwrap();
        let img = crate::gif::decode(&entry.data).unwrap();
        let med = m([25, -50, 100], [1, 2, 3]);

        let out = loadsnap(&img, &med);
        assert_eq!((out.w, out.h), (img.w, img.h));
        assert_eq!(
            out.px.iter().filter(|&&p| p == 0xFF01_0203).count(),
            318,
            "every gray pixel becomes csky"
        );
        assert_eq!(
            out.px.iter().filter(|&&p| p >> 24 == 0).count(),
            1135,
            "the keyed (transparent) pixels stay transparent"
        );
        assert_eq!(
            out.px[out.px.len() - 1],
            img.px[img.px.len() - 1],
            "the corner key is untouched"
        );
    }

    // ===================================================================
    // The xtGraphics machine
}
