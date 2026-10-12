//! Rules of the a11y-report engine. Adding a rule = one file + one line in [`all_rules`].

pub mod color;
pub mod contrast_text;
pub mod duplicate_name;
pub mod missing_name;
pub mod target_size;

use a11y_report_core::Rule;

/// Every rule, in the fixed execution order.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(missing_name::MissingName),
        Box::new(target_size::TargetSize),
        Box::new(contrast_text::ContrastText),
        Box::new(duplicate_name::DuplicateName),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use a11y_report_core::criteria::criterion;
    use a11y_report_core::geometry::Rect;
    use a11y_report_core::model::{Device, Node, NodeFlags, NodeId, Screen};
    use a11y_report_core::rule::{RuleConfig, RuleOutput, ScreenCtx};
    use a11y_report_core::{Evidence, Severity};
    use std::collections::BTreeSet;

    #[test]
    fn rule_ids_unique_and_criteria_known() {
        let rules = all_rules();
        let ids: BTreeSet<_> = rules.iter().map(|r| r.meta().id).collect();
        assert_eq!(ids.len(), rules.len());
        for r in &rules {
            for c in r.meta().criteria {
                assert!(criterion(c).is_some(), "{c}");
            }
        }
    }

    fn device() -> Device {
        Device {
            model: "Test".into(),
            density_dpi: 160,
            width_px: 400,
            height_px: 800,
            font_scale: 1.0,
        }
    }

    fn node(id: u32, class: &str, bounds: Rect) -> Node {
        Node {
            id: NodeId(id),
            parent: if id == 0 { None } else { Some(NodeId(0)) },
            children: Vec::new(),
            depth: u16::from(id != 0),
            index_in_parent: u16::try_from(id.saturating_sub(1)).unwrap_or(0),
            class: class.into(),
            package: None,
            resource_id: None,
            text: None,
            content_desc: None,
            bounds,
            clipped_by_screen: false,
            flags: NodeFlags {
                enabled: true,
                clickable: id != 0,
                ..NodeFlags::default()
            },
        }
    }

    fn run(rule: &dyn Rule, nodes: Vec<Node>) -> RuleOutput {
        let mut nodes = nodes;
        let children = (1..nodes.len() as u32).map(NodeId).collect();
        nodes[0].children = children;
        let screen = Screen {
            name: "s".into(),
            width_px: 400,
            height_px: 800,
            nodes,
        };
        let device = device();
        let config = RuleConfig::defaults(rule.meta());
        let ctx = ScreenCtx {
            screen: &screen,
            device: &device,
            image: None,
            config: &config,
        };
        let mut out = RuleOutput::default();
        rule.check(&ctx, &mut out);
        out
    }

    fn root() -> Node {
        node(0, "android.widget.FrameLayout", Rect::new(0, 0, 400, 800))
    }

    #[test]
    fn missing_name_severities() {
        let img = node(1, "android.widget.ImageButton", Rect::new(0, 0, 60, 60));
        let edit = node(2, "android.widget.EditText", Rect::new(0, 100, 300, 160));
        let btn = node(3, "android.widget.Button", Rect::new(0, 200, 300, 260));
        let mut named = node(4, "android.widget.Button", Rect::new(0, 300, 300, 360));
        named.text = Some("OK".into());
        let out = run(
            &missing_name::MissingName,
            vec![root(), img, edit, btn, named],
        );
        let sev: Vec<_> = out
            .findings
            .iter()
            .map(|f| (f.node.0, f.severity))
            .collect();
        assert_eq!(
            sev,
            vec![
                (1, Severity::Critical),
                (2, Severity::Minor),
                (3, Severity::Major)
            ]
        );
        assert_eq!(out.findings[0].criteria, vec!["wcag-4.1.2", "wcag-1.1.1"]);
    }

    #[test]
    fn target_size_thresholds() {
        // density 160: 1 px = 1 dp.
        let mut tiny = node(1, "android.widget.Button", Rect::new(10, 10, 30, 30));
        tiny.text = Some("x".into());
        let mut small = node(2, "android.widget.Button", Rect::new(10, 100, 50, 140));
        small.text = Some("y".into());
        let mut ok = node(3, "android.widget.Button", Rect::new(10, 200, 60, 250));
        ok.text = Some("z".into());
        let mut cut = node(4, "android.widget.Button", Rect::new(10, 300, 20, 310));
        cut.clipped_by_screen = true;
        let out = run(&target_size::TargetSize, vec![root(), tiny, small, ok, cut]);
        assert_eq!(out.findings.len(), 2);
        assert_eq!(out.findings[0].severity, Severity::Major);
        assert_eq!(out.findings[0].criteria, vec!["wcag-2.5.8"]);
        assert_eq!(out.findings[1].severity, Severity::Minor);
        assert!(matches!(
            out.findings[1].evidence,
            Evidence::TargetSize { width_dp, height_dp, min_dp } if width_dp == 40.0 && height_dp == 40.0 && min_dp == 48.0
        ));
    }

    #[test]
    fn duplicate_names_grouped_case_insensitively() {
        let mut a = node(1, "android.widget.Button", Rect::new(0, 0, 100, 60));
        a.text = Some("Join".into());
        let mut b = node(2, "android.widget.Button", Rect::new(0, 100, 100, 160));
        b.text = Some("join".into());
        let mut c = node(3, "android.widget.Button", Rect::new(0, 200, 100, 260));
        c.text = Some("Leave".into());
        let out = run(&duplicate_name::DuplicateName, vec![root(), a, b, c]);
        assert_eq!(out.findings.len(), 1);
        assert_eq!(out.findings[0].node, NodeId(1));
        match &out.findings[0].evidence {
            Evidence::DuplicateName { name, others } => {
                assert_eq!(name, "Join");
                assert_eq!(others.len(), 1);
            }
            other => panic!("unexpected evidence {other:?}"),
        }
    }

    #[test]
    fn contrast_without_image_does_nothing() {
        let mut t = node(1, "android.widget.TextView", Rect::new(0, 0, 100, 40));
        t.text = Some("Hello".into());
        let out = run(&contrast_text::ContrastText, vec![root(), t]);
        assert!(out.findings.is_empty());
        assert!(out.applicable.is_empty());
    }
}
