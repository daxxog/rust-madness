//! Port of the `java.awt.Color` bits the renderer depends on — the exact
//! JDK algorithms for RGB↔HSB plus `darker()`/`brighter()`, so colors match
//! the original engine for every shade it can produce.

/// `new Color(r, g, b).getRGB()` — the JDK 4-int constructor's packing:
/// `((a & 0xFF) << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)`.
/// Every channel is masked to 8 bits, so out-of-range components wrap into
/// their neighbor instead of saturating. The constructor's
/// `testColorValueRange` (JDK 8 source) throws for out-of-range input, but
/// all call sites here clamp first (or pass constants), so the throw is
/// unreachable and is not modeled.
pub fn rgb(r: i32, g: i32, b: i32) -> u32 {
    0xFF00_0000 | (((r & 0xFF) as u32) << 16) | (((g & 0xFF) as u32) << 8) | ((b & 0xFF) as u32)
}

pub const FACTOR: f32 = 0.7; // Color.FACTOR in JDK sources

/// Port of `Color.RGBtoHSB(int,int,int,float[])` (JDK source), including its
/// int comparisons and float divisions.
pub fn rgb_to_hsb(r: i32, g: i32, b: i32) -> [f32; 3] {
    let cmax = r.max(g).max(b);
    let cmin = r.min(g).min(b);

    let brightness = cmax as f32 / 255.0;
    let saturation = if cmax != 0 {
        (cmax - cmin) as f32 / cmax as f32
    } else {
        0.0
    };

    let mut hue = 0.0;
    if saturation != 0.0 {
        let d = (cmax - cmin) as f32;
        let redc = (cmax - r) as f32 / d;
        let greenc = (cmax - g) as f32 / d;
        let bluec = (cmax - b) as f32 / d;
        let mut h = if r == cmax {
            bluec - greenc
        } else if g == cmax {
            2.0 + redc - bluec
        } else {
            4.0 + greenc - redc
        };
        h /= 6.0;
        if h < 0.0 {
            h += 1.0;
        }
        hue = h;
    }

    [hue, saturation, brightness]
}

/// Port of `Color.getHSBColor`/`Color.HSBtoRGB(float,float,float)` (JDK
/// source, including the +0.5 rounding on every channel).
pub fn hsb_to_rgb(hue: f32, saturation: f32, brightness: f32) -> [i32; 3] {
    if saturation == 0.0 {
        let v = (brightness * 255.0 + 0.5) as i32;
        return [v, v, v];
    }

    let h = (hue - hue.floor()) * 6.0;
    let f = h - h.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * f);
    let t = brightness * (1.0 - (saturation * (1.0 - f)));

    let (r, g, b) = match h as i32 {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q), // case 5
    };
    [
        (r * 255.0 + 0.5) as i32,
        (g * 255.0 + 0.5) as i32,
        (b * 255.0 + 0.5) as i32,
    ]
}

/// `Color.darker()` — each channel `* FACTOR`, floor 0.
pub fn darker(c: [i32; 3]) -> [i32; 3] {
    c.map(|v| (v as f32 * FACTOR).max(0.0) as i32)
}

/// `Color.brighter()` — each channel `/ FACTOR + 0.5`, ceiling 255.
pub fn brighter(c: [i32; 3]) -> [i32; 3] {
    c.map(|v| ((v as f32 / FACTOR + 0.5) as i32).min(255))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsb_round_trip() {
        for c in [
            [226, 120, 56],
            [150, 150, 150],
            [0, 0, 0],
            [255, 255, 255],
            [196, 47, 30],
        ] {
            let [h, s, b] = rgb_to_hsb(c[0], c[1], c[2]);
            let rgb = hsb_to_rgb(h, s, b);
            assert_eq!(rgb, c, "round trip {c:?} via hsb {h},{s},{b}");
        }
    }

    #[test]
    fn hsb_of_primary_red() {
        // Java getHSBColor(0, 1, 1) = red
        assert_eq!(hsb_to_rgb(0.0, 1.0, 1.0), [255, 0, 0]);
        // darker/brighter JDK semantics
        assert_eq!(darker([255, 100, 10]), [178, 70, 7]);
        // Java: brighter() = (int)(v/0.7 + 0.5) per channel, ceiling 255
        assert_eq!(brighter([100, 10, 0]), [143, 14, 0]);
    }
}
