use a11y_report_core::a11y::{accessible_name, is_interactive};
use a11y_report_core::criteria::WCAG_2_4_6;
use a11y_report_core::rule::{RuleMeta, RuleOutput, ScreenCtx};
use a11y_report_core::{Confidence, ElementRef, Evidence, Finding, NodeId, Rule, Severity};
use std::collections::BTreeMap;

pub struct DuplicateName;

static META: RuleMeta = RuleMeta {
    id: "duplicate-name",
    version: 1,
    title_en: "Several controls share the same name",
    title_fr: "Plusieurs contrôles portent le même nom",
    default_severity: Severity::Minor,
    criteria: &[WCAG_2_4_6],
    needs_screenshot: false,
    params: &[],
};

impl Rule for DuplicateName {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &ScreenCtx<'_>, out: &mut RuleOutput) {
        let screen = ctx.screen;
        let mut groups: BTreeMap<String, Vec<(NodeId, String)>> = BTreeMap::new();
        for node in &screen.nodes {
            if !is_interactive(node) || node.bounds.is_empty() {
                continue;
            }
            let Some(name) = accessible_name(screen, node.id) else {
                continue;
            };
            out.applicable.insert(WCAG_2_4_6);
            groups
                .entry(name.to_lowercase())
                .or_default()
                .push((node.id, name));
        }
        // Report in node order so the output does not depend on name ordering.
        let mut duplicates: Vec<Vec<(NodeId, String)>> =
            groups.into_values().filter(|g| g.len() >= 2).collect();
        duplicates.sort_by_key(|g| g[0].0);
        for group in duplicates {
            let (first, name) = &group[0];
            let others = group[1..]
                .iter()
                .map(|(id, _)| ElementRef::from_node(screen, *id))
                .collect();
            out.findings.push(Finding::new(
                META.id,
                Severity::Minor,
                Confidence::Medium,
                &[WCAG_2_4_6],
                screen,
                *first,
                Evidence::DuplicateName {
                    name: name.clone(),
                    others,
                },
            ));
        }
    }
}
