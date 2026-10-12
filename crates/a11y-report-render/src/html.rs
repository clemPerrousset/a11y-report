//! Self-contained HTML report, rendered from the report object only.

use crate::guidance;
use crate::i18n::{self, Labels, labels, number, percent};
use crate::{RenderError, Thumbnail};
use a11y_report_core::report::{CriterionStatus, FindingRef, Report};
use a11y_report_core::{Finding, Lang, Severity};
use askama::Template;
use base64::Engine as _;
use std::collections::{BTreeMap, BTreeSet};

struct Count {
    label: &'static str,
    class: &'static str,
    n: u32,
}

struct RuleRow {
    title: String,
    severity_label: &'static str,
    severity_class: &'static str,
    open: usize,
    screens: usize,
}

struct CriterionRow {
    reference: String,
    title: String,
    level: String,
    normative: bool,
    status_label: &'static str,
    status_class: &'static str,
    issues: u32,
}

struct Mark {
    n: usize,
    class: &'static str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    cx: i32,
    cy: i32,
}

struct Figure {
    width: u32,
    height: u32,
    data_uri: String,
    stroke: u32,
    radius: u32,
    font: u32,
    marks: Vec<Mark>,
}

struct Swatch {
    label: &'static str,
    hex: String,
}

struct IssueView {
    n: usize,
    anchor: String,
    title: String,
    severity_label: &'static str,
    severity_class: &'static str,
    confidence: &'static str,
    status: &'static str,
    ignored: bool,
    element: String,
    measure: String,
    fix: String,
    done_when: String,
    id: String,
    swatches: Vec<Swatch>,
    sources: Vec<String>,
}

struct ScreenView {
    name: String,
    anchor: String,
    activity: Option<String>,
    open: usize,
    figure: Option<Figure>,
    not_verified: Option<String>,
    issues: Vec<IssueView>,
}

struct RefRow {
    id: String,
    title: String,
    screen: String,
    severity_label: &'static str,
    severity_class: &'static str,
}

struct ManualRow {
    reference: String,
    title: String,
    why: String,
}

struct RuleLine {
    id: String,
    title: String,
    version: u32,
}

#[derive(Template)]
#[template(path = "report.html")]
struct ReportPage<'a> {
    l: &'a Labels,
    lang: &'static str,
    app: String,
    build: String,
    captured_at: String,
    device: String,
    screen_count: usize,
    rate: String,
    rate_delta: Option<String>,
    open_total: u32,
    counts: Vec<Count>,
    ignored: u32,
    new_count: Option<u32>,
    fixed_count: Option<u32>,
    has_baseline: bool,
    digest_changed: bool,
    rule_rows: Vec<RuleRow>,
    criteria: Vec<CriterionRow>,
    errors: Vec<String>,
    screens: Vec<ScreenView>,
    fixed: Vec<RefRow>,
    not_rechecked: Vec<RefRow>,
    manual: Vec<ManualRow>,
    limitations: Vec<String>,
    version: String,
    digest: String,
    rules: Vec<RuleLine>,
}

fn severity_class(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "critical",
        Severity::Major => "major",
        Severity::Minor => "minor",
        Severity::Info => "info",
    }
}

fn status_class(s: CriterionStatus) -> &'static str {
    match s {
        CriterionStatus::Pass => "pass",
        CriterionStatus::Fail => "fail",
        CriterionStatus::NotApplicable => "na",
        CriterionStatus::Manual => "manual",
    }
}

fn anchor(prefix: &str, s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{prefix}-{clean}")
}

fn rule_titles(report: &Report) -> BTreeMap<&str, &str> {
    report
        .tool
        .rules
        .iter()
        .map(|r| (r.id.as_str(), r.title.as_str()))
        .collect()
}

fn issue_view(f: &Finding, n: usize, titles: &BTreeMap<&str, &str>, lang: Lang) -> IssueView {
    let mut swatches = Vec::new();
    if let a11y_report_core::Evidence::Contrast {
        fg,
        bg,
        suggested_fg,
        ..
    } = &f.evidence
    {
        let l = labels(lang);
        swatches.push(Swatch {
            label: l.colour_text,
            hex: fg.hex(),
        });
        swatches.push(Swatch {
            label: l.colour_background,
            hex: bg.hex(),
        });
        if let Some(s) = suggested_fg {
            swatches.push(Swatch {
                label: l.colour_suggested,
                hex: s.hex(),
            });
        }
    }
    IssueView {
        n,
        anchor: anchor("issue", &f.id),
        title: titles
            .get(f.rule.as_str())
            .copied()
            .unwrap_or(&f.rule)
            .to_string(),
        severity_label: i18n::severity(f.severity, lang),
        severity_class: severity_class(f.severity),
        confidence: i18n::confidence(f.confidence, lang),
        status: i18n::status(f.status, lang),
        ignored: f.ignored,
        element: guidance::element(&f.element),
        measure: guidance::measure(f, lang),
        fix: guidance::fix(f, lang),
        done_when: guidance::done_when(f, lang),
        id: f.id.clone(),
        swatches,
        sources: f
            .source_hints
            .iter()
            .map(|h| format!("{}:{} ({})", h.file, h.line, h.matched_by))
            .collect(),
    }
}

fn ref_row(r: &FindingRef, titles: &BTreeMap<&str, &str>, lang: Lang) -> RefRow {
    RefRow {
        id: r.id.clone(),
        title: titles
            .get(r.rule.as_str())
            .copied()
            .unwrap_or(&r.rule)
            .to_string(),
        screen: r.screen.clone(),
        severity_label: i18n::severity(r.severity, lang),
        severity_class: severity_class(r.severity),
    }
}

pub fn render_html(
    report: &Report,
    thumbnails: &BTreeMap<String, Thumbnail>,
) -> Result<String, RenderError> {
    let lang = report.lang;
    let l = labels(lang);
    let titles = rule_titles(report);
    let s = &report.summary;

    let rate = s
        .automatic_conformity_rate
        .map(|r| percent(r, lang))
        .unwrap_or_else(|| l.not_available.to_string());
    let rate_delta = s.diff.as_ref().and_then(|d| {
        let (now, before) = (s.automatic_conformity_rate?, d.baseline_rate?);
        let pts = ((now - before) * 100.0).round();
        let sign = if pts > 0.0 { "+" } else { "" };
        Some(format!(
            "{sign}{} {} {}",
            number(pts, lang),
            l.points_vs,
            d.baseline_build
        ))
    });

    let counts = Severity::ALL
        .into_iter()
        .map(|sev| Count {
            label: i18n::severity(sev, lang),
            class: severity_class(sev),
            n: s.issues.get(sev),
        })
        .collect();

    // What to fix first: open issues grouped by rule, worst severity first.
    let mut by_rule: BTreeMap<&str, (Severity, usize, BTreeSet<&str>)> = BTreeMap::new();
    for f in report.issues.iter().filter(|f| !f.ignored) {
        let e = by_rule
            .entry(f.rule.as_str())
            .or_insert((Severity::Info, 0, BTreeSet::new()));
        e.0 = e.0.min(f.severity);
        e.1 += 1;
        e.2.insert(f.screen.as_str());
    }
    let mut rule_rows: Vec<(Severity, usize, &str, usize)> = by_rule
        .iter()
        .map(|(rule, (sev, n, screens))| (*sev, *n, *rule, screens.len()))
        .collect();
    rule_rows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(b.2)));
    let rule_rows = rule_rows
        .into_iter()
        .map(|(sev, open, rule, screens)| RuleRow {
            title: titles.get(rule).copied().unwrap_or(rule).to_string(),
            severity_label: i18n::severity(sev, lang),
            severity_class: severity_class(sev),
            open,
            screens,
        })
        .collect();

    let criteria = report
        .criteria
        .iter()
        .map(|c| CriterionRow {
            reference: c.reference.clone(),
            title: c.title.clone(),
            level: c.level.clone(),
            normative: c.normative,
            status_label: i18n::criterion_status(c.status, lang),
            status_class: status_class(c.status),
            issues: c.issue_count,
        })
        .collect();

    let mut screens = Vec::new();
    for sc in &report.screens {
        let issues: Vec<&Finding> = report
            .issues
            .iter()
            .filter(|f| f.screen == sc.name)
            .collect();
        let figure = thumbnails.get(&sc.name).map(|t| {
            let w = sc.width_px.max(1);
            let radius = (w / 30).max(12);
            let r = i32::try_from(radius).unwrap_or(12);
            // Several issues on the same element: shift their number badges sideways.
            let mut per_origin: BTreeMap<(i32, i32), i32> = BTreeMap::new();
            Figure {
                width: sc.width_px,
                height: sc.height_px,
                data_uri: format!(
                    "data:image/jpeg;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&t.jpeg)
                ),
                stroke: (w / 180).max(3),
                radius,
                font: (w / 28).max(14),
                marks: issues
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| !f.ignored)
                    .map(|(i, f)| {
                        let b = f.element.bounds;
                        let k = per_origin.entry((b.x1, b.y1)).or_insert(0);
                        let shift = *k * r * 2;
                        *k += 1;
                        let max_x = i32::try_from(sc.width_px).unwrap_or(i32::MAX) - r;
                        Mark {
                            n: i + 1,
                            class: severity_class(f.severity),
                            x: b.x1,
                            y: b.y1,
                            w: b.width(),
                            h: b.height(),
                            cx: (b.x1 + shift).max(r).min(max_x.max(r)),
                            cy: b.y1.max(r),
                        }
                    })
                    .collect(),
            }
        });
        screens.push(ScreenView {
            name: sc.name.clone(),
            anchor: anchor("screen", &sc.name),
            activity: sc.activity.clone(),
            open: issues.iter().filter(|f| !f.ignored).count(),
            figure,
            not_verified: (!sc.rules_not_verified.is_empty())
                .then(|| sc.rules_not_verified.join(", ")),
            issues: issues
                .iter()
                .enumerate()
                .map(|(i, f)| issue_view(f, i + 1, &titles, lang))
                .collect(),
        });
    }

    let page = ReportPage {
        l,
        lang: lang.code(),
        app: report.run.app.clone(),
        build: report.run.build.clone(),
        captured_at: report.run.captured_at.clone(),
        device: format!(
            "{} · {} dpi · {}×{} px",
            report.run.device.model,
            report.run.device.density_dpi,
            report.run.device.width_px,
            report.run.device.height_px
        ),
        screen_count: report.run.screens.len(),
        rate,
        rate_delta,
        open_total: s.issues.open(),
        counts,
        ignored: s.issues.ignored,
        new_count: s.diff.as_ref().map(|d| d.new),
        fixed_count: s.diff.as_ref().map(|d| d.fixed),
        has_baseline: s.diff.is_some(),
        digest_changed: s.diff.as_ref().is_some_and(|d| d.rules_digest_changed),
        rule_rows,
        criteria,
        errors: report
            .run
            .errors
            .iter()
            .map(|e| match &e.screen {
                Some(name) => format!("{name}: {}", e.message),
                None => e.message.clone(),
            })
            .collect(),
        screens,
        fixed: report
            .fixed
            .iter()
            .map(|r| ref_row(r, &titles, lang))
            .collect(),
        not_rechecked: report
            .not_rechecked
            .iter()
            .map(|r| ref_row(r, &titles, lang))
            .collect(),
        manual: report
            .manual_checks
            .iter()
            .map(|m| ManualRow {
                reference: m.reference.clone(),
                title: m.title.clone(),
                why: m.why.clone(),
            })
            .collect(),
        limitations: report.known_limitations.clone(),
        version: format!("{} {}", report.tool.name, report.tool.version),
        digest: report.tool.rules_digest.clone(),
        rules: report
            .tool
            .rules
            .iter()
            .map(|r| RuleLine {
                id: r.id.clone(),
                title: r.title.clone(),
                version: r.version,
            })
            .collect(),
    };
    Ok(page.render()?)
}
