use a11y_report_core::a11y::is_interactive;
use a11y_report_core::criteria::{ANDROID_TOUCH_TARGET, WCAG_2_5_8};
use a11y_report_core::num::px_to_dp;
use a11y_report_core::rule::{ParamMeta, RuleMeta, RuleOutput, ScreenCtx};
use a11y_report_core::{Confidence, Evidence, Finding, Node, Rule, Screen, Severity};

pub struct TargetSize;

pub const MIN_DP: f64 = 48.0;
pub const HARD_MIN_DP: f64 = 24.0;

static META: RuleMeta = RuleMeta {
    id: "target-size",
    version: 1,
    title_en: "Touch target too small",
    title_fr: "Cible tactile trop petite",
    default_severity: Severity::Major,
    criteria: &[WCAG_2_5_8, ANDROID_TOUCH_TARGET],
    needs_screenshot: false,
    params: &[
        ParamMeta {
            name: "min_dp",
            default: MIN_DP,
            description_en: "Android guideline: smaller targets are minor issues",
            description_fr: "Recommandation Android : en dessous, écart mineur",
        },
        ParamMeta {
            name: "hard_min_dp",
            default: HARD_MIN_DP,
            description_en: "WCAG 2.5.8 minimum: smaller targets are major issues",
            description_fr: "Minimum WCAG 2.5.8 : en dessous, écart majeur",
        },
    ],
};

/// The real size of a node cut by a scrolling container is unknown: uiautomator only
/// reports its visible part. A node touching an edge of a scrollable ancestor without
/// spanning it on that axis is treated as possibly cut.
fn clipped_by_scroll(screen: &Screen, node: &Node) -> bool {
    let b = node.bounds;
    screen
        .ancestors(node.id)
        .filter(|a| a.flags.scrollable)
        .any(|a| {
            let s = a.bounds;
            let spans_v = b.y1 <= s.y1 && b.y2 >= s.y2;
            let spans_h = b.x1 <= s.x1 && b.x2 >= s.x2;
            let touches_v = b.y1 <= s.y1 || b.y2 >= s.y2;
            let touches_h = b.x1 <= s.x1 || b.x2 >= s.x2;
            (touches_v && !spans_v) || (touches_h && !spans_h)
        })
}

impl Rule for TargetSize {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &ScreenCtx<'_>, out: &mut RuleOutput) {
        let min_dp = ctx.config.param("min_dp", MIN_DP);
        let hard_min_dp = ctx.config.param("hard_min_dp", HARD_MIN_DP);
        let dpi = ctx.device.density_dpi;
        for node in &ctx.screen.nodes {
            if !is_interactive(node) || node.bounds.is_empty() {
                continue;
            }
            if node.clipped_by_screen || clipped_by_scroll(ctx.screen, node) {
                continue;
            }
            out.applicable.insert(WCAG_2_5_8);
            out.applicable.insert(ANDROID_TOUCH_TARGET);
            let w = px_to_dp(f64::from(node.bounds.width()), dpi);
            let h = px_to_dp(f64::from(node.bounds.height()), dpi);
            let min_side = w.min(h);
            let (severity, criterion, threshold) = if min_side < hard_min_dp {
                (Severity::Major, WCAG_2_5_8, hard_min_dp)
            } else if min_side < min_dp {
                (Severity::Minor, ANDROID_TOUCH_TARGET, min_dp)
            } else {
                continue;
            };
            out.findings.push(Finding::new(
                META.id,
                severity,
                Confidence::High,
                &[criterion],
                ctx.screen,
                node.id,
                Evidence::TargetSize {
                    width_dp: w,
                    height_dp: h,
                    min_dp: threshold,
                },
            ));
        }
    }
}
