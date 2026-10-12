//! WCAG relative luminance, contrast ratio and HSL-based colour suggestions.

use a11y_report_core::Rgb;
use std::sync::OnceLock;

fn linear_table() -> &'static [f64; 256] {
    static TABLE: OnceLock<[f64; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0.0; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f64 / 255.0;
            *v = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        t
    })
}

/// Linearized sRGB channel value.
#[inline]
pub fn linearize(c: u8) -> f64 {
    linear_table()[c as usize]
}

/// WCAG relative luminance, in [0, 1].
#[inline]
pub fn luminance_rgb(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

pub fn luminance(c: Rgb) -> f64 {
    luminance_rgb(c.r, c.g, c.b)
}

/// Contrast between two luminances, in [1, 21].
#[inline]
pub fn ratio_from_luminance(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    ratio_from_luminance(luminance(a), luminance(b))
}

/// (hue in [0, 360), saturation and lightness in [0, 1]).
pub fn rgb_to_hsl(c: Rgb) -> (f64, f64, f64) {
    let r = f64::from(c.r) / 255.0;
    let g = f64::from(c.g) / 255.0;
    let b = f64::from(c.b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s.clamp(0.0, 1.0), l)
}

pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to8 = |v: f64| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgb::new(to8(r1), to8(g1), to8(b1))
}

/// Nearest text colour with the same hue and saturation whose contrast with `bg`
/// reaches `required`, found by moving lightness in 0.5 % steps in both directions.
pub fn suggest_text_color(fg: Rgb, bg: Rgb, required: f64) -> Option<(Rgb, f64)> {
    let (h, s, l) = rgb_to_hsl(fg);
    let darker_first = luminance(bg) >= luminance(fg);
    for step in 1..=200 {
        let delta = f64::from(step) * 0.005;
        let candidates = if darker_first {
            [l - delta, l + delta]
        } else {
            [l + delta, l - delta]
        };
        for cand in candidates {
            if !(0.0..=1.0).contains(&cand) {
                continue;
            }
            let c = hsl_to_rgb(h, s, cand);
            let r = contrast_ratio(c, bg);
            if r >= required {
                return Some((c, r));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const BLACK: Rgb = Rgb::new(0, 0, 0);
    const WHITE: Rgb = Rgb::new(255, 255, 255);

    #[test]
    fn reference_values() {
        assert!((contrast_ratio(BLACK, WHITE) - 21.0).abs() < 1e-9);
        let grey = contrast_ratio(Rgb::new(0x77, 0x77, 0x77), WHITE);
        assert!((grey - 4.478).abs() < 0.001, "{grey}");
        assert!(grey < 4.5);
    }

    #[test]
    fn hsl_roundtrip() {
        for c in [
            Rgb::new(0xFF, 0x8A, 0x5B),
            Rgb::new(12, 200, 99),
            WHITE,
            BLACK,
        ] {
            let (h, s, l) = rgb_to_hsl(c);
            let back = hsl_to_rgb(h, s, l);
            assert!((i16::from(back.r) - i16::from(c.r)).abs() <= 1);
            assert!((i16::from(back.g) - i16::from(c.g)).abs() <= 1);
            assert!((i16::from(back.b) - i16::from(c.b)).abs() <= 1);
        }
    }

    #[test]
    fn suggestion_meets_threshold() {
        let fg = Rgb::new(0xFF, 0x8A, 0x5B);
        let (c, r) = suggest_text_color(fg, WHITE, 4.5).expect("suggestion");
        assert!(r >= 4.5);
        assert!(luminance(c) < luminance(fg), "should darken on white");
        assert!(suggest_text_color(fg, Rgb::new(0x10, 0x10, 0x10), 4.5).is_some());
    }

    fn rgb() -> impl Strategy<Value = Rgb> {
        (any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(r, g, b)| Rgb::new(r, g, b))
    }

    proptest! {
        #[test]
        fn ratio_in_range(a in rgb(), b in rgb()) {
            let r = contrast_ratio(a, b);
            prop_assert!((1.0..=21.0 + 1e-9).contains(&r));
        }

        #[test]
        fn ratio_symmetric(a in rgb(), b in rgb()) {
            prop_assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-12);
        }

        #[test]
        fn ratio_self_is_one(a in rgb()) {
            prop_assert!((contrast_ratio(a, a) - 1.0).abs() < 1e-12);
        }
    }
}
