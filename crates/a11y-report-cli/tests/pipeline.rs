//! End-to-end tests of the analysis pipeline on synthetic runs with known colours.

use a11y_report_cli::config::EngineConfig;
use a11y_report_cli::engine::{AnalyzeOptions, analyze, load_report};
use a11y_report_cli::synth::{self, SynthScreen};
use a11y_report_cli::{Formats, exit_code, write_outputs};
use a11y_report_core::report::CriterionStatus;
use a11y_report_core::{Evidence, Lang, Report, Severity, Status};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn options(run_dir: &Path, config: Option<&str>, baseline: Option<Report>) -> AnalyzeOptions {
    let rules = a11y_report_rules::all_rules();
    let config = match config {
        Some(text) => EngineConfig::parse(text, &rules).expect("valid config"),
        None => EngineConfig::defaults(&rules),
    };
    AnalyzeOptions {
        run_dir: run_dir.to_path_buf(),
        config,
        rules,
        baseline,
        lang: Lang::En,
        project_root: None,
        thumbnails: false,
    }
}

fn demo_run(dir: &Path) {
    let screens = synth::demo_screens().expect("draw");
    synth::write_run(dir, &screens, "2.4.0 (240)").expect("write run");
}

fn json(report: &Report) -> String {
    a11y_report_render::render_json(report).expect("json")
}

#[test]
fn synthetic_contrast_is_exact() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;

    // #777777 on #FFFFFF: 4.478 → shown as 4.47, fails 4.5.
    let grey: Vec<_> = report
        .issues
        .iter()
        .filter(|f| {
            f.rule == "contrast-text" && f.element.text.as_deref() == Some("Water every 7 days")
        })
        .collect();
    assert_eq!(grey.len(), 3);
    match &grey[0].evidence {
        Evidence::Contrast {
            ratio,
            required,
            fg,
            bg,
            large_text,
            ..
        } => {
            assert_eq!(*ratio, 4.47);
            assert_eq!(*required, 4.5);
            assert_eq!(fg.hex(), "#777777");
            assert_eq!(bg.hex(), "#FFFFFF");
            assert!(!large_text);
        }
        other => panic!("unexpected evidence {other:?}"),
    }
    assert_eq!(grey[0].severity, Severity::Major);
    // Same element key in three cards: collisions get ~2 and ~3 in node order.
    assert!(
        grey[1].id.ends_with("~2") && grey[2].id.ends_with("~3"),
        "{:?}",
        grey.iter().map(|f| &f.id).collect::<Vec<_>>()
    );

    // 30 sp title in #949494 (≈3.03:1) is large text: passes at 3:1.
    assert!(
        !report
            .issues
            .iter()
            .any(|f| f.element.resource_id.as_deref() == Some("plant_title"))
    );
    // 14 sp #9E9E9E (≈2.68:1) fails.
    assert!(
        report.issues.iter().any(|f| f.rule == "contrast-text"
            && f.element.resource_id.as_deref() == Some("plant_light"))
    );
    // Text over stripes: info with low confidence, never failing.
    let banner = report
        .issues
        .iter()
        .find(|f| f.element.resource_id.as_deref() == Some("seasonal_banner"))
        .expect("banner finding");
    assert_eq!(banner.severity, Severity::Info);
    assert!(!banner.is_blocking());
}

#[test]
fn every_rule_fires_on_the_demo() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    let find = |rule: &str, rid: Option<&str>| {
        report
            .issues
            .iter()
            .find(|f| f.rule == rule && f.element.resource_id.as_deref() == rid)
            .unwrap_or_else(|| panic!("{rule} on {rid:?}"))
    };
    assert_eq!(
        find("missing-name", Some("menu")).severity,
        Severity::Critical
    );
    assert_eq!(
        find("missing-name", Some("search_box")).severity,
        Severity::Minor
    );
    assert_eq!(find("missing-name", None).severity, Severity::Major);
    assert_eq!(
        find("target-size", Some("btn_tips")).severity,
        Severity::Major
    );
    assert_eq!(
        find("target-size", Some("btn_water")).severity,
        Severity::Minor
    );
    assert_eq!(
        find("target-size", Some("btn_delete")).severity,
        Severity::Major
    );
    let dup = find("duplicate-name", Some("btn_water"));
    assert!(matches!(&dup.evidence, Evidence::DuplicateName { others, .. } if others.len() == 2));
    // Named through its child: no missing-name on the share row.
    assert!(
        !report
            .issues
            .iter()
            .any(|f| f.element.resource_id.as_deref() == Some("row_share"))
    );
    // The settings screen is clean.
    assert!(!report.issues.iter().any(|f| f.screen == "settings"));
    let c = |id: &str| {
        report
            .criteria
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.status)
    };
    assert_eq!(c("wcag-1.4.3"), Some(CriterionStatus::Fail));
    assert_eq!(c("wcag-1.3.2"), Some(CriterionStatus::Manual));
}

#[test]
fn report_json_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    insta::assert_snapshot!("demo_report_json", json(&report));
}

#[test]
fn output_is_deterministic_byte_for_byte() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let mut opts = options(dir.path(), None, None);
    opts.thumbnails = true;
    let a = analyze(&opts).expect("analyze");
    let b = analyze(&opts).expect("analyze");
    assert_eq!(json(&a.report), json(&b.report));
    assert_eq!(
        a11y_report_render::render_html(&a.report, &a.thumbnails).expect("html"),
        a11y_report_render::render_html(&b.report, &b.thumbnails).expect("html")
    );

    // Same screens listed in another order in run.json.
    let path = dir.path().join("run.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
    manifest["screens"]
        .as_array_mut()
        .expect("screens")
        .reverse();
    fs::write(&path, serde_json::to_string(&manifest).expect("ser")).expect("write");
    let c = analyze(&opts).expect("analyze");
    assert_eq!(json(&a.report), json(&c.report));
    assert_eq!(
        a11y_report_render::render_html(&a.report, &a.thumbnails).expect("html"),
        a11y_report_render::render_html(&c.report, &c.thumbnails).expect("html")
    );
}

fn shift_bounds(xml: &str, d: i32) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find("bounds=\"") {
        out.push_str(&rest[..i + 8]);
        rest = &rest[i + 8..];
        let end = rest.find('"').expect("closing quote");
        let nums: Vec<i32> = rest[..end]
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().expect("int"))
            .collect();
        out.push_str(&format!(
            "[{},{}][{},{}]",
            nums[0] + d,
            nums[1] + d,
            nums[2] + d,
            nums[3] + d
        ));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn ids_survive_moves_and_changing_numbers() {
    // A number inside an interactive element's name, changed between the two runs.
    let without_png = |s: SynthScreen| SynthScreen {
        png: None,
        xml: s.xml.replace("text=\"Delete\"", "text=\"Delete 2 plants\""),
        ..s
    };
    let base: Vec<SynthScreen> = synth::demo_screens()
        .expect("draw")
        .into_iter()
        .map(without_png)
        .collect();
    let moved: Vec<SynthScreen> = base
        .iter()
        .map(|s| SynthScreen {
            name: s.name.clone(),
            activity: s.activity.clone(),
            xml: shift_bounds(&s.xml, 10)
                .replace("Water every 7 days", "Water every 12 days")
                .replace("text=\"Delete 2 plants\"", "text=\"Delete 15 plants\""),
            png: None,
        })
        .collect();
    let a_dir = tempfile::tempdir().expect("tempdir");
    let b_dir = tempfile::tempdir().expect("tempdir");
    synth::write_run(a_dir.path(), &base, "1").expect("write");
    synth::write_run(b_dir.path(), &moved, "2").expect("write");
    let ids = |dir: &Path| -> BTreeSet<String> {
        analyze(&options(dir, None, None))
            .expect("analyze")
            .report
            .issues
            .into_iter()
            .map(|f| f.id)
            .collect()
    };
    let a = ids(a_dir.path());
    assert!(!a.is_empty());
    assert_eq!(a, ids(b_dir.path()));
}

#[test]
fn diff_new_existing_fixed_and_not_rechecked() {
    let base_dir = tempfile::tempdir().expect("tempdir");
    demo_run(base_dir.path());
    let base = analyze(&options(base_dir.path(), None, None))
        .expect("analyze")
        .report;

    // Head: the menu gets a description (fixed), the tips button loses its own
    // (new), and plant_detail is not captured (not rechecked).
    let mut screens = synth::demo_screens().expect("draw");
    screens.retain(|s| s.name != "plant_detail");
    let home = screens.iter_mut().find(|s| s.name == "home").expect("home");
    home.xml = home
        .xml
        .replacen("resource-id=\"com.example.pocketgarden:id/menu\" class=\"android.widget.ImageButton\" package=\"com.example.pocketgarden\" content-desc=\"\"", "resource-id=\"com.example.pocketgarden:id/menu\" class=\"android.widget.ImageButton\" package=\"com.example.pocketgarden\" content-desc=\"Open menu\"", 1)
        .replace("content-desc=\"Plant care tips\"", "content-desc=\"\"");
    let head_dir = tempfile::tempdir().expect("tempdir");
    synth::write_run(head_dir.path(), &screens, "2.5.0 (250)").expect("write");
    let head = analyze(&options(head_dir.path(), None, Some(base.clone())))
        .expect("analyze")
        .report;

    let fixed: Vec<_> = head.fixed.iter().map(|r| r.id.as_str()).collect();
    assert!(
        fixed
            .iter()
            .any(|id| id.starts_with("missing-name:home:menu#")),
        "{fixed:?}"
    );
    assert!(
        head.not_rechecked
            .iter()
            .all(|r| r.screen == "plant_detail")
    );
    assert!(!head.not_rechecked.is_empty());
    assert!(
        !head.fixed.iter().any(|r| r.screen == "plant_detail"),
        "not captured ≠ fixed"
    );
    let tips = head
        .issues
        .iter()
        .find(|f| f.rule == "missing-name" && f.element.resource_id.as_deref() == Some("btn_tips"))
        .expect("new issue");
    assert_eq!(tips.status, Status::New);
    assert!(head.issues.iter().any(|f| f.status == Status::Existing));
    let d = head.summary.diff.as_ref().expect("diff summary");
    assert_eq!(d.baseline_build, "2.4.0 (240)");
    assert!(!d.rules_digest_changed);
    assert_eq!(d.fixed as usize, head.fixed.len());

    // --new-only only fails on the new critical issue.
    assert_eq!(exit_code(&head, Some(Severity::Critical), true), 1);

    // The `diff` logic on two stored reports agrees.
    let d2 = a11y_report_core::diff::diff_reports(&base, &head).expect("diff");
    assert_eq!(d2.fixed, head.fixed);
    assert_eq!(d2.not_rechecked, head.not_rechecked);
}

#[test]
fn config_ignores_and_overrides() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let first = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    let menu_id = first
        .issues
        .iter()
        .find(|f| f.element.resource_id.as_deref() == Some("menu"))
        .map(|f| f.id.clone())
        .expect("menu issue");
    let cfg = format!(
        "[rules.contrast-text]\nseverity = \"minor\"\n[rules.duplicate-name]\nenabled = false\n[ignore]\nresource_ids = [\"*:id/btn_tips\"]\nfindings = [\"{menu_id}\"]\n"
    );
    let report = analyze(&options(dir.path(), Some(&cfg), None))
        .expect("analyze")
        .report;
    assert_ne!(report.tool.rules_digest, first.tool.rules_digest);
    assert!(!report.issues.iter().any(|f| f.rule == "duplicate-name"));
    let menu = report
        .issues
        .iter()
        .find(|f| f.id == menu_id)
        .expect("still listed");
    assert!(menu.ignored, "ignored findings stay in the JSON");
    assert!(
        report
            .issues
            .iter()
            .filter(|f| f.element.resource_id.as_deref() == Some("btn_tips"))
            .all(|f| f.ignored)
    );
    assert!(
        report
            .issues
            .iter()
            .filter(|f| f.rule == "contrast-text" && f.severity != Severity::Info)
            .all(|f| f.severity == Severity::Minor)
    );
    assert_eq!(report.summary.issues.critical, 0);
    assert!(report.summary.issues.ignored >= 2);
}

#[test]
fn missing_screenshot_marks_contrast_unverified() {
    let dir = tempfile::tempdir().expect("tempdir");
    let screens: Vec<SynthScreen> = synth::demo_screens()
        .expect("draw")
        .into_iter()
        .map(|s| SynthScreen { png: None, ..s })
        .collect();
    synth::write_run(dir.path(), &screens, "1").expect("write");
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    assert!(
        report
            .screens
            .iter()
            .all(|s| s.rules_not_verified == vec!["contrast-text".to_string()])
    );
    let status = report
        .criteria
        .iter()
        .find(|c| c.id == "wcag-1.4.3")
        .map(|c| c.status);
    assert_eq!(status, Some(CriterionStatus::Manual));
    assert!(!report.issues.iter().any(|f| f.rule == "contrast-text"));
}

#[test]
fn invalid_screens_are_reported_not_fatal() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let screens = dir.path().join("screens");
    // Truncated XML.
    let xml = fs::read_to_string(screens.join("settings.xml")).expect("read");
    fs::write(screens.join("settings.xml"), &xml[..xml.len() / 2]).expect("write");
    // Screenshot of the wrong size.
    let small = synth::Canvas::new(10, 10, [0, 0, 0]).to_png().expect("png");
    fs::write(screens.join("plant_detail.png"), small).expect("write");
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    assert_eq!(report.run.screens, vec!["home".to_string()]);
    let errors: Vec<_> = report
        .run
        .errors
        .iter()
        .map(|e| e.screen.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        errors,
        vec!["plant_detail".to_string(), "settings".to_string()]
    );
    assert!(report.run.errors[0].message.contains("10×10"));
    assert!(report.run.errors[1].message.contains("invalid XML"));
    assert_eq!(exit_code(&report, None, false), 2);
}

#[test]
fn negative_bounds_are_an_input_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let path = dir.path().join("screens/home.xml");
    let xml =
        fs::read_to_string(&path)
            .expect("read")
            .replacen("bounds=\"[0,0]", "bounds=\"[-20,0]", 1);
    fs::write(&path, xml).expect("write");
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    assert!(
        report
            .run
            .errors
            .iter()
            .any(|e| e.message.contains("negative bounds"))
    );
}

#[test]
fn baseline_with_other_schema_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let report = analyze(&options(dir.path(), None, None))
        .expect("analyze")
        .report;
    let path = dir.path().join("old.json");
    fs::write(
        &path,
        json(&report).replace("\"schema_version\": 1", "\"schema_version\": 0"),
    )
    .expect("write");
    let Err(err) = load_report(&path) else {
        panic!("a baseline with another schema_version must be refused");
    };
    assert!(err.to_string().contains("schema_version is 0"), "{err}");
}

#[test]
fn writes_all_outputs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = dir.path().join("out/run");
    demo_run(&run);
    let mut opts = options(&run, None, None);
    opts.thumbnails = true;
    opts.lang = Lang::Fr;
    let analysis = analyze(&opts).expect("analyze");
    let out = dir.path().join("out");
    let main = write_outputs(&analysis, &run, &out, Formats::default()).expect("write");
    assert_eq!(main, out.join("report.html"));
    let html = fs::read_to_string(&main).expect("html");
    assert!(html.starts_with("<!doctype html>\n<html lang=\"fr\">"));
    assert!(html.contains("Taux de conformité automatique"));
    assert!(html.contains("data:image/jpeg;base64,"));
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "no external resource"
    );
    let brief = fs::read_to_string(out.join("ai/home.md")).expect("brief");
    assert!(brief.contains("](../run/screens/home.png)"));
    assert!(
        fs::read_to_string(out.join("ai/README.md"))
            .expect("readme")
            .contains("| `plant_detail` |")
    );
    // The JSON round-trips: parsing and re-serializing gives the same bytes.
    let written = fs::read_to_string(out.join("report.json")).expect("json");
    let parsed = load_report(&out.join("report.json")).expect("reload");
    assert_eq!(
        a11y_report_render::render_json(&parsed).expect("json"),
        written
    );
}
