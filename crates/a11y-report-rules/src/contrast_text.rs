//! Text contrast measured on the screenshot.
//!
//! Inside the node bounds shrunk by 1 px:
//! 1. pixels are quantized to 5 bits per channel;
//! 2. the background is the dominant bucket of the 2 px border ring (mean of its real pixels);
//! 3. if that bucket covers less than 60 % of the ring, the background is busy (image,
//!    gradient): the finding is `info` with low confidence;
//! 4. text candidates are pixels with a contrast of at least 1.5 against the background;
//!    fewer than 20 pixels or 0.5 % of the zone means no text was detected;
//! 5. the text colour is the dominant bucket among candidates (anti-aliasing spreads over
//!    many buckets, the glyph bodies dominate).

use crate::color::{
    contrast_ratio, luminance, luminance_rgb, ratio_from_luminance, suggest_text_color,
};
use a11y_report_core::criteria::WCAG_1_4_3;
use a11y_report_core::num::{floor2, px_to_dp};
use a11y_report_core::rule::{RuleMeta, RuleOutput, ScreenCtx};
use a11y_report_core::{Confidence, Evidence, Finding, Image, Rect, Rgb, Rule, Severity};

pub struct ContrastText;

static META: RuleMeta = RuleMeta {
    id: "contrast-text",
    version: 1,
    title_en: "Insufficient text contrast",
    title_fr: "Contraste du texte insuffisant",
    default_severity: Severity::Major,
    criteria: &[WCAG_1_4_3],
    needs_screenshot: true,
    params: &[],
};

const MIN_AREA_PX: i64 = 64;
const RING_PX: i32 = 2;
const UNIFORMITY_MIN: f64 = 0.6;
const CANDIDATE_MIN_RATIO: f64 = 1.5;
const MIN_TEXT_PIXELS: usize = 20;
const MIN_TEXT_SHARE: f64 = 0.005;
/// Ink height ≈ 0.9 × font size.
const INK_TO_FONT: f64 = 0.9;
pub const NORMAL_RATIO: f64 = 4.5;
pub const LARGE_RATIO: f64 = 3.0;
/// 18 pt ≈ 24 dp; between 20 and 28 dp the size estimate is too uncertain to relax the threshold.
const LARGE_TEXT_DP: f64 = 28.0;
const UNCERTAIN_TEXT_DP: f64 = 20.0;

#[inline]
fn bucket(p: [u8; 3]) -> u16 {
    (u16::from(p[0] >> 3) << 10) | (u16::from(p[1] >> 3) << 5) | u16::from(p[2] >> 3)
}

/// Most frequent bucket and its count; ties go to the smallest bucket for determinism.
fn dominant(buckets: &mut [u16]) -> Option<(u16, usize)> {
    buckets.sort_unstable();
    let mut best: Option<(u16, usize)> = None;
    let mut i = 0;
    while i < buckets.len() {
        let key = buckets[i];
        let mut j = i;
        while j < buckets.len() && buckets[j] == key {
            j += 1;
        }
        let count = j - i;
        if best.is_none_or(|(_, c)| count > c) {
            best = Some((key, count));
        }
        i = j;
    }
    best
}

fn mean_color(sum: [u64; 3], n: u64) -> Rgb {
    if n == 0 {
        return Rgb::default();
    }
    let avg = |s: u64| u8::try_from((s + n / 2) / n).unwrap_or(u8::MAX);
    Rgb::new(avg(sum[0]), avg(sum[1]), avg(sum[2]))
}

/// Colours and geometry measured in a zone.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    pub fg: Rgb,
    pub bg: Rgb,
    /// Unrounded contrast ratio.
    pub ratio: f64,
    /// Share of the ring covered by the background bucket.
    pub uniformity: f64,
    /// Median height of text line bands, in pixels.
    pub ink_height_px: f64,
}

/// Measures text and background colours inside `zone`. `None` when no text is detected.
pub fn measure(image: &Image, zone: Rect) -> Option<Measure> {
    let zone = zone.clamp_to(image.width, image.height);
    if zone.width() <= RING_PX * 2 || zone.height() <= RING_PX * 2 {
        return None;
    }
    let (x1, y1, x2, y2) = (
        zone.x1 as u32,
        zone.y1 as u32,
        zone.x2 as u32,
        zone.y2 as u32,
    );
    let ring = RING_PX as u32;
    let in_ring =
        |x: u32, y: u32| x < x1 + ring || x >= x2 - ring || y < y1 + ring || y >= y2 - ring;

    // Background: dominant bucket of the border ring.
    let mut ring_buckets = Vec::new();
    for y in y1..y2 {
        for x in x1..x2 {
            if in_ring(x, y) {
                ring_buckets.push(bucket(image.pixel(x, y)));
            }
        }
    }
    let ring_total = ring_buckets.len();
    let (bg_bucket, bg_count) = dominant(&mut ring_buckets)?;
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for y in y1..y2 {
        for x in x1..x2 {
            let p = image.pixel(x, y);
            if in_ring(x, y) && bucket(p) == bg_bucket {
                for (s, v) in sum.iter_mut().zip(p) {
                    *s += u64::from(v);
                }
                n += 1;
            }
        }
    }
    let bg = mean_color(sum, n);
    let bg_lum = luminance(bg);

    // Text candidates.
    let height = (y2 - y1) as usize;
    let mut rows = vec![false; height];
    let mut candidates = Vec::new();
    for y in y1..y2 {
        for x in x1..x2 {
            let p = image.pixel(x, y);
            if ratio_from_luminance(luminance_rgb(p[0], p[1], p[2]), bg_lum) >= CANDIDATE_MIN_RATIO
            {
                candidates.push(bucket(p));
                rows[(y - y1) as usize] = true;
            }
        }
    }
    let area = zone.area() as f64;
    if candidates.len() < MIN_TEXT_PIXELS || (candidates.len() as f64) < area * MIN_TEXT_SHARE {
        return None;
    }
    let (fg_bucket, _) = dominant(&mut candidates)?;
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for y in y1..y2 {
        for x in x1..x2 {
            let p = image.pixel(x, y);
            if bucket(p) == fg_bucket
                && ratio_from_luminance(luminance_rgb(p[0], p[1], p[2]), bg_lum)
                    >= CANDIDATE_MIN_RATIO
            {
                for (s, v) in sum.iter_mut().zip(p) {
                    *s += u64::from(v);
                }
                n += 1;
            }
        }
    }
    let fg = mean_color(sum, n);

    // Line bands: runs of consecutive rows containing text pixels.
    let mut bands = Vec::new();
    let mut run = 0usize;
    for &has_text in rows.iter().chain(std::iter::once(&false)) {
        if has_text {
            run += 1;
        } else if run > 0 {
            bands.push(run);
            run = 0;
        }
    }
    bands.sort_unstable();
    let ink_height_px = match bands.len() {
        0 => 0.0,
        len if len % 2 == 1 => bands[len / 2] as f64,
        len => (bands[len / 2 - 1] + bands[len / 2]) as f64 / 2.0,
    };

    Some(Measure {
        fg,
        bg,
        ratio: contrast_ratio(fg, bg),
        uniformity: bg_count as f64 / ring_total as f64,
        ink_height_px,
    })
}

impl Rule for ContrastText {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &ScreenCtx<'_>, out: &mut RuleOutput) {
        let Some(image) = ctx.image else { return };
        let screen = ctx.screen;
        for node in &screen.nodes {
            let has_text = node.text.as_deref().is_some_and(|t| !t.trim().is_empty());
            if !node.children.is_empty()
                || !has_text
                || !node.flags.enabled
                || node.bounds.area() < MIN_AREA_PX
            {
                continue;
            }
            let Some(m) = measure(image, node.bounds.inset(1)) else {
                tracing::debug!(screen = %screen.name, node = node.id.0, "no text pixels detected");
                continue;
            };
            out.applicable.insert(WCAG_1_4_3);
            let text_height_dp = px_to_dp(m.ink_height_px / INK_TO_FONT, ctx.device.density_dpi);
            let (required, large_text, mut confidence) = if text_height_dp >= LARGE_TEXT_DP {
                (LARGE_RATIO, true, Confidence::High)
            } else if text_height_dp >= UNCERTAIN_TEXT_DP {
                (NORMAL_RATIO, false, Confidence::Medium)
            } else {
                (NORMAL_RATIO, false, Confidence::High)
            };
            if m.ratio >= required {
                continue;
            }
            if m.uniformity < UNIFORMITY_MIN {
                confidence = Confidence::Low;
            }
            let severity = if confidence == Confidence::Low {
                Severity::Info
            } else {
                Severity::Major
            };
            let suggestion = suggest_text_color(m.fg, m.bg, required);
            out.findings.push(Finding::new(
                META.id,
                severity,
                confidence,
                &[WCAG_1_4_3],
                screen,
                node.id,
                Evidence::Contrast {
                    ratio: floor2(m.ratio),
                    required,
                    fg: m.fg,
                    bg: m.bg,
                    text_height_dp,
                    large_text,
                    background_uniformity: m.uniformity,
                    suggested_fg: suggestion.map(|s| s.0),
                    suggested_ratio: suggestion.map(|s| floor2(s.1)),
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: u32, h: u32, bg: [u8; 3]) -> Image {
        let data = (0..w * h).flat_map(|_| bg).collect();
        Image::new(w, h, data).expect("valid size")
    }

    fn fill(img: &mut Image, r: Rect, c: [u8; 3]) {
        for y in r.y1..r.y2 {
            for x in r.x1..r.x2 {
                let i = (y as usize * img.width as usize + x as usize) * 3;
                img.data[i..i + 3].copy_from_slice(&c);
            }
        }
    }

    #[test]
    fn grey_on_white_is_measured_exactly() {
        let mut img = canvas(200, 60, [255, 255, 255]);
        // Two "lines" of glyph strokes, 12 px high each.
        for x in (20..180).step_by(6) {
            fill(&mut img, Rect::new(x, 10, x + 3, 22), [0x77, 0x77, 0x77]);
            fill(&mut img, Rect::new(x, 30, x + 3, 42), [0x77, 0x77, 0x77]);
        }
        let m = measure(&img, Rect::new(0, 0, 200, 60)).expect("text detected");
        assert_eq!(m.bg, Rgb::new(255, 255, 255));
        assert_eq!(m.fg, Rgb::new(0x77, 0x77, 0x77));
        assert!((m.ratio - 4.478).abs() < 0.001);
        assert_eq!(floor2(m.ratio), 4.47);
        assert_eq!(m.uniformity, 1.0);
        assert_eq!(m.ink_height_px, 12.0);
    }

    #[test]
    fn blank_zone_has_no_text() {
        let img = canvas(50, 50, [10, 20, 30]);
        assert!(measure(&img, Rect::new(0, 0, 50, 50)).is_none());
        assert!(measure(&img, Rect::new(0, 0, 4, 4)).is_none());
    }

    #[test]
    fn busy_background_lowers_uniformity() {
        let mut img = canvas(100, 40, [255, 255, 255]);
        for x in (0..100).step_by(2) {
            fill(&mut img, Rect::new(x, 0, x + 1, 40), [200, 30, 30]);
        }
        fill(&mut img, Rect::new(20, 12, 80, 28), [240, 240, 240]);
        let m = measure(&img, Rect::new(0, 0, 100, 40)).expect("text detected");
        assert!(m.uniformity < UNIFORMITY_MIN);
    }
}
