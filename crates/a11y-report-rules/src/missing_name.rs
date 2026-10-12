use a11y_report_core::a11y::{accessible_name, is_interactive};
use a11y_report_core::criteria::{WCAG_1_1_1, WCAG_4_1_2};
use a11y_report_core::finding::MissingNameKind;
use a11y_report_core::rule::{RuleMeta, RuleOutput, ScreenCtx};
use a11y_report_core::{Confidence, Evidence, Finding, Rule, Severity};

pub struct MissingName;

static META: RuleMeta = RuleMeta {
    id: "missing-name",
    version: 1,
    title_en: "Interactive element without an accessible name",
    title_fr: "Élément interactif sans nom accessible",
    default_severity: Severity::Major,
    criteria: &[WCAG_4_1_2, WCAG_1_1_1],
    needs_screenshot: false,
    params: &[],
};

fn is_image_class(class: &str) -> bool {
    class.ends_with("ImageView") || class.ends_with("ImageButton")
}

impl Rule for MissingName {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &ScreenCtx<'_>, out: &mut RuleOutput) {
        let screen = ctx.screen;
        for node in &screen.nodes {
            if !is_interactive(node) || node.bounds.is_empty() {
                continue;
            }
            let image = is_image_class(&node.class);
            out.applicable.insert(WCAG_4_1_2);
            if image {
                out.applicable.insert(WCAG_1_1_1);
            }
            if accessible_name(screen, node.id).is_some() {
                continue;
            }
            let (severity, confidence, criteria, kind): (_, _, &[&str], _) = if image {
                (
                    Severity::Critical,
                    Confidence::High,
                    &[WCAG_4_1_2, WCAG_1_1_1],
                    MissingNameKind::Image,
                )
            } else if node.class.ends_with("EditText") {
                (
                    Severity::Minor,
                    Confidence::Low,
                    &[WCAG_4_1_2],
                    MissingNameKind::EditText,
                )
            } else {
                (
                    Severity::Major,
                    Confidence::High,
                    &[WCAG_4_1_2],
                    MissingNameKind::Control,
                )
            };
            out.findings.push(Finding::new(
                META.id,
                severity,
                confidence,
                criteria,
                screen,
                node.id,
                Evidence::MissingName { element_kind: kind },
            ));
        }
    }
}
