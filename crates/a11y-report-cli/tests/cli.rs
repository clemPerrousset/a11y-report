//! Exit codes and stdout of the binary.

use a11y_report_cli::synth;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn bin(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_a11y-report"))
        .args(args)
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .output()
        .expect("run binary")
}

fn demo_run(dir: &Path) {
    let screens = synth::demo_screens().expect("draw");
    synth::write_run(&dir.join("run"), &screens, "2.4.0 (240)").expect("write");
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

#[test]
fn exit_codes_follow_fail_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    demo_run(dir.path());
    let failing = bin(&["analyze", "run", "--out", "out"], dir.path());
    assert_eq!(
        code(&failing),
        1,
        "{}",
        String::from_utf8_lossy(&failing.stderr)
    );
    let stdout = String::from_utf8_lossy(&failing.stdout);
    assert_eq!(stdout.lines().count(), 3, "{stdout}");
    assert!(stdout.starts_with("Automatic conformity rate: "));
    assert!(dir.path().join("out/report.html").is_file());
    assert!(dir.path().join("out/ai/README.md").is_file());

    let lenient = bin(
        &[
            "analyze",
            "run",
            "--out",
            "out",
            "--fail-on",
            "none",
            "--stdout",
            "none",
        ],
        dir.path(),
    );
    assert_eq!(code(&lenient), 0);
    assert!(lenient.stdout.is_empty());

    // With itself as baseline, nothing is new.
    let new_only = bin(
        &[
            "analyze",
            "run",
            "--out",
            "out2",
            "--baseline",
            "out/report.json",
            "--new-only",
            "--stdout",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(code(&new_only), 0);
    let report: serde_json::Value =
        serde_json::from_slice(&new_only.stdout).expect("json on stdout");
    assert_eq!(report["summary"]["diff"]["new"], 0);
}

#[test]
fn invalid_inputs_exit_with_2() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = bin(&["analyze", "nowhere"], dir.path());
    assert_eq!(code(&missing), 2);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("run.json"));

    demo_run(dir.path());
    fs::write(
        dir.path().join("a11y.toml"),
        "[rules.unknown]\nenabled = true\n",
    )
    .expect("write");
    let bad_config = bin(&["analyze", "run", "--out", "out"], dir.path());
    assert_eq!(code(&bad_config), 2);
    assert!(String::from_utf8_lossy(&bad_config.stderr).contains("unknown rule"));
    fs::remove_file(dir.path().join("a11y.toml")).expect("rm");

    let xml = dir.path().join("run/screens/home.xml");
    fs::write(&xml, "<hierarchy><node").expect("write");
    let broken = bin(
        &["analyze", "run", "--out", "out", "--fail-on", "none"],
        dir.path(),
    );
    assert_eq!(
        code(&broken),
        2,
        "a broken screen does not stop the run but sets code 2"
    );
    assert!(dir.path().join("out/report.json").is_file());
}

#[test]
fn schema_rules_and_diff_commands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let schema = bin(&["schema"], dir.path());
    assert_eq!(code(&schema), 0);
    let committed = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/report.v1.json"
    ))
    .expect("schema/report.v1.json is committed");
    assert_eq!(
        String::from_utf8_lossy(&schema.stdout).replace("\r\n", "\n"),
        committed.replace("\r\n", "\n"),
        "schema/report.v1.json is out of date: run `a11y-report schema > schema/report.v1.json`"
    );

    let rules = bin(&["--lang", "fr", "rules"], dir.path());
    assert!(
        String::from_utf8_lossy(&rules.stdout)
            .contains("contrast-text (v1) — Contraste du texte insuffisant")
    );

    demo_run(dir.path());
    assert_eq!(
        code(&bin(
            &["analyze", "run", "--out", "a", "--fail-on", "none"],
            dir.path()
        )),
        0
    );
    assert_eq!(
        code(&bin(
            &["analyze", "run", "--out", "b", "--fail-on", "none"],
            dir.path()
        )),
        0
    );
    let diff = bin(&["diff", "a/report.json", "b/report.json"], dir.path());
    assert_eq!(code(&diff), 0);
    assert!(String::from_utf8_lossy(&diff.stdout).contains("0 new"));
}
