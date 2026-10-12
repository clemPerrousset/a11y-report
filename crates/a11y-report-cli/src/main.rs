//! `a11y-report`: run it at the root of an Android project.

use a11y_report_android::project::{self, AndroidProject};
use a11y_report_cli::config::{ConfigError, EngineConfig};
use a11y_report_cli::engine::{self, AnalyzeOptions, EngineError};
use a11y_report_cli::scan::{self, ScanError, Session, SessionOptions};
use a11y_report_cli::{Formats, exit_code, summary_lines, synth, write_outputs};
use a11y_report_core::diff::diff_reports;
use a11y_report_core::{Lang, Report, Severity};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "a11y-report",
    version,
    about = "Accessibility audit of Android apps: capture screens over adb, get an HTML report for humans and Markdown briefs for AI agents.",
    long_about = "Run `a11y-report` at the root of an Android project with a phone or emulator connected:\nit guides you through capturing screens, then writes ./a11y-report/ (report.html, report.json, ai/*.md)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Language of the reports and messages.
    #[arg(long, global = true, value_enum, default_value_t = LangArg::En)]
    lang: LangArg,
    /// More logs on stderr (-v debug, -vv trace).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(flatten)]
    scan: ScanArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Capture screens interactively from a connected device, then analyse (default).
    Scan(ScanArgs),
    /// Capture the screen currently shown into a run directory (for scripts and UI tests).
    Capture(CaptureArgs),
    /// Analyse an existing run directory (run.json + screens/).
    Analyze(AnalyzeArgs),
    /// Compare two reports without re-analysing.
    Diff(DiffArgs),
    /// List the rules, their criteria and parameters.
    Rules,
    /// Print the JSON Schema of report.json.
    Schema,
    /// Generate a demo project and report without any device.
    Demo(DemoArgs),
    /// Check adb, connected devices and the Android project.
    Doctor,
}

#[derive(Clone, Copy, ValueEnum)]
enum LangArg {
    En,
    Fr,
}

impl From<LangArg> for Lang {
    fn from(l: LangArg) -> Lang {
        match l {
            LangArg::En => Lang::En,
            LangArg::Fr => Lang::Fr,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum FailOn {
    Critical,
    Major,
    Minor,
    None,
}

impl FailOn {
    fn severity(self) -> Option<Severity> {
        match self {
            FailOn::Critical => Some(Severity::Critical),
            FailOn::Major => Some(Severity::Major),
            FailOn::Minor => Some(Severity::Minor),
            FailOn::None => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum StdoutMode {
    Summary,
    Json,
    None,
}

#[derive(Args, Clone)]
struct ReportArgs {
    /// Output directory.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Files to write: json, html, md (AI briefs).
    #[arg(long, default_value = "json,html,md")]
    format: String,
    /// Previous report.json to compare with.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Configuration file (default: ./a11y.toml when it exists).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Exit with code 1 when an issue at or above this severity is open.
    #[arg(long, value_enum, default_value_t = FailOn::Major)]
    fail_on: FailOn,
    /// Only fail on issues that are new compared to the baseline.
    #[arg(long)]
    new_only: bool,
    /// What to print on stdout.
    #[arg(long, value_enum, default_value_t = StdoutMode::Summary)]
    stdout: StdoutMode,
    /// Android project root used to locate the source of each element (default: auto).
    #[arg(long)]
    project: Option<PathBuf>,
}

#[derive(Args, Clone)]
struct ScanArgs {
    /// Device serial (see `adb devices`) when several are connected.
    #[arg(long)]
    serial: Option<String>,
    /// Application id to audit (default: read from the Gradle project).
    #[arg(long)]
    package: Option<String>,
    /// Launch the app before capturing.
    #[arg(long)]
    launch: bool,
    /// Keep the previous captures and add new ones.
    #[arg(long)]
    append: bool,
    #[command(flatten)]
    report: ReportArgs,
}

#[derive(Args)]
struct CaptureArgs {
    /// Screen name: a-z, 0-9, '_' and '-'.
    name: String,
    /// Run directory to add the capture to.
    #[arg(long, default_value = "a11y-report/run")]
    run: PathBuf,
    #[arg(long)]
    serial: Option<String>,
    #[arg(long)]
    package: Option<String>,
}

#[derive(Args)]
struct AnalyzeArgs {
    /// Run directory containing run.json (default: ./a11y-report/run).
    run_dir: Option<PathBuf>,
    #[command(flatten)]
    report: ReportArgs,
}

#[derive(Args)]
struct DiffArgs {
    base: PathBuf,
    head: PathBuf,
    #[arg(long, value_enum, default_value_t = StdoutMode::Summary)]
    stdout: StdoutMode,
}

#[derive(Args)]
struct DemoArgs {
    /// Folder to create the demo project in.
    #[arg(default_value = "a11y-demo")]
    dir: PathBuf,
}

/// Errors caused by the user's input map to exit code 2; anything else is 3.
fn is_input_error(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.is::<EngineError>()
            || c.is::<ConfigError>()
            || c.is::<ScanError>()
            || c.is::<InputError>()
    })
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct InputError(String);

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

fn load_config(
    explicit: Option<&Path>,
    root: &Path,
    rules: &[Box<dyn a11y_report_core::Rule>],
) -> Result<EngineConfig> {
    let path = match explicit {
        Some(p) => Some(p.to_path_buf()),
        None => Some(root.join("a11y.toml")).filter(|p| p.is_file()),
    };
    match path {
        Some(p) => {
            let text = std::fs::read_to_string(&p)
                .map_err(|e| InputError(format!("cannot read {}: {e}", p.display())))?;
            Ok(EngineConfig::parse(&text, rules)?)
        }
        None => Ok(EngineConfig::defaults(rules)),
    }
}

/// Runs the analysis of `run_dir` and writes the outputs; returns the exit code.
fn analyze_and_write(
    run_dir: &Path,
    args: &ReportArgs,
    default_out: PathBuf,
    auto_baseline: bool,
    project: Option<&AndroidProject>,
    lang: Lang,
) -> Result<i32> {
    let cwd = std::env::current_dir().context("current directory")?;
    let rules = a11y_report_rules::all_rules();
    let config = load_config(
        args.config.as_deref(),
        project.map_or(&cwd, |p| &p.root),
        &rules,
    )?;
    let formats = Formats::parse(&args.format).map_err(InputError)?;
    let out = absolute(args.out.as_deref().unwrap_or(&default_out));
    let baseline = match &args.baseline {
        Some(p) => Some(engine::load_report(p)?),
        None if auto_baseline && out.join("report.json").is_file() => {
            match engine::load_report(&out.join("report.json")) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!("previous report ignored: {e}");
                    None
                }
            }
        }
        None => None,
    };
    let project_root = args
        .project
        .clone()
        .or_else(|| project.map(|p| p.root.clone()));
    let analysis = engine::analyze(&AnalyzeOptions {
        run_dir: absolute(run_dir),
        config,
        rules,
        baseline,
        lang,
        project_root,
        thumbnails: formats.html,
    })?;
    let main = write_outputs(&analysis, &absolute(run_dir), &out, formats)?;
    match args.stdout {
        StdoutMode::Summary => {
            for line in summary_lines(&analysis.report, &main, lang) {
                println!("{line}");
            }
        }
        StdoutMode::Json => print!("{}", a11y_report_render::render_json(&analysis.report)?),
        StdoutMode::None => {}
    }
    Ok(exit_code(
        &analysis.report,
        args.fail_on.severity(),
        args.new_only,
    ))
}

fn cmd_scan(args: ScanArgs, lang: Lang) -> Result<i32> {
    let cwd = std::env::current_dir().context("current directory")?;
    let report_args = args.report.clone();
    let project = report_args
        .project
        .as_deref()
        .and_then(project::detect)
        .or_else(|| project::detect(&cwd));
    match &project {
        Some(p) => eprintln!(
            "{} {} ({})",
            lang.pick("Android project:", "Projet Android :"),
            p.root.display(),
            p.application_id.as_deref().unwrap_or("?")
        ),
        None => eprintln!(
            "{}",
            lang.pick(
                "No Gradle project here: source hints are disabled (run from the project root, or pass --package).",
                "Pas de projet Gradle ici : les indices de source sont désactivés (lancer depuis la racine du projet, ou passer --package)."
            )
        ),
    }
    let root = project.as_ref().map_or(cwd.clone(), |p| p.root.clone());
    let out = absolute(
        report_args
            .out
            .as_deref()
            .unwrap_or(&root.join("a11y-report")),
    );
    let run_dir = out.join("run");
    let mut session = Session::open(SessionOptions {
        run_dir: run_dir.clone(),
        serial: args.serial,
        package: args.package,
        append: args.append,
        project: project.clone(),
    })?;
    if args.launch
        && let Some(pkg) = session.manifest.package.clone()
    {
        session.adb.launch(&pkg)?;
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut ui = std::io::stderr();
    scan::interactive(&mut session, &mut input, &mut ui, lang)?;
    let mut report_args = report_args;
    report_args.out = Some(out.clone());
    analyze_and_write(&run_dir, &report_args, out, true, project.as_ref(), lang)
}

fn cmd_capture(args: CaptureArgs) -> Result<i32> {
    let cwd = std::env::current_dir().context("current directory")?;
    let project = project::detect(&cwd);
    let mut session = Session::open(SessionOptions {
        run_dir: absolute(&args.run),
        serial: args.serial,
        package: args.package,
        append: true,
        project,
    })?;
    let c = session.capture(&scan::sanitize_name(&args.name))?;
    eprintln!(
        "{} ({} elements) -> {}",
        c.name,
        c.nodes,
        args.run.display()
    );
    Ok(0)
}

fn cmd_analyze(args: AnalyzeArgs, lang: Lang) -> Result<i32> {
    let cwd = std::env::current_dir().context("current directory")?;
    let project = args
        .report
        .project
        .as_deref()
        .and_then(project::detect)
        .or_else(|| project::detect(&cwd));
    let run_dir = args
        .run_dir
        .clone()
        .unwrap_or_else(|| cwd.join("a11y-report").join("run"));
    analyze_and_write(
        &run_dir,
        &args.report,
        cwd.join("a11y-report"),
        false,
        project.as_ref(),
        lang,
    )
}

fn cmd_diff(args: DiffArgs, lang: Lang) -> Result<i32> {
    let base: Report = engine::load_report(&args.base)?;
    let head: Report = engine::load_report(&args.head)?;
    let d = diff_reports(&base, &head)?;
    match args.stdout {
        StdoutMode::Json => println!("{}", serde_json::to_string_pretty(&d)?),
        StdoutMode::Summary => {
            println!(
                "{} -> {}: {} {}, {} {}, {} {}, {} {}",
                d.baseline_build,
                d.head_build,
                d.new.len(),
                lang.pick("new", "nouveaux"),
                d.existing.len(),
                lang.pick("existing", "existants"),
                d.fixed.len(),
                lang.pick("fixed", "corrigés"),
                d.not_rechecked.len(),
                lang.pick("not rechecked", "non revérifiés"),
            );
            if d.rules_digest_changed {
                println!("{}", lang.pick("warning: the reports were produced with different rules or settings", "attention : les rapports ont été produits avec des règles ou réglages différents"));
            }
            for (label, list) in [("+", &d.new), ("✓", &d.fixed), ("?", &d.not_rechecked)] {
                for r in list {
                    println!("{label} {} [{}]", r.id, r.severity);
                }
            }
        }
        StdoutMode::None => {}
    }
    Ok(0)
}

fn cmd_rules(lang: Lang) -> Result<i32> {
    for r in a11y_report_rules::all_rules() {
        let m = r.meta();
        println!("{} (v{}) — {}", m.id, m.version, m.title(lang));
        println!(
            "  {}: {}",
            lang.pick("criteria", "critères"),
            m.criteria.join(", ")
        );
        println!(
            "  {}: {}",
            lang.pick("default severity", "sévérité par défaut"),
            m.default_severity
        );
        if m.needs_screenshot {
            println!(
                "  {}",
                lang.pick("needs a screenshot", "nécessite une capture")
            );
        }
        for p in m.params {
            println!(
                "  [rules.{}] {} = {} — {}",
                m.id,
                p.name,
                p.default,
                lang.pick(p.description_en, p.description_fr)
            );
        }
    }
    Ok(0)
}

fn cmd_demo(args: DemoArgs, lang: Lang) -> Result<i32> {
    let root = absolute(&args.dir);
    synth::write_demo_project(&root).with_context(|| format!("writing {}", root.display()))?;
    let run_dir = root.join("a11y-report").join("run");
    let screens = synth::demo_screens().context("drawing demo screens")?;
    synth::write_run(&run_dir, &screens, "2.4.0 (240)").context("writing demo run")?;
    eprintln!(
        "{} {}",
        lang.pick(
            "Demo project written to",
            "Projet de démonstration écrit dans"
        ),
        root.display()
    );
    let project = project::detect(&root);
    let args = ReportArgs {
        out: Some(root.join("a11y-report")),
        format: "json,html,md".into(),
        baseline: None,
        config: None,
        fail_on: FailOn::None,
        new_only: false,
        stdout: StdoutMode::Summary,
        project: Some(root.clone()),
    };
    analyze_and_write(
        &run_dir,
        &args,
        root.join("a11y-report"),
        false,
        project.as_ref(),
        lang,
    )
}

fn cmd_doctor(lang: Lang) -> Result<i32> {
    let cwd = std::env::current_dir().context("current directory")?;
    let project = project::detect(&cwd);
    match &project {
        Some(p) => {
            println!(
                "✓ {} {}",
                lang.pick("Android project:", "Projet Android :"),
                p.root.display()
            );
            println!(
                "  applicationId: {}",
                p.application_id.as_deref().unwrap_or("?")
            );
            println!("  app_name: {}", p.app_name.as_deref().unwrap_or("?"));
        }
        None => println!(
            "✗ {}",
            lang.pick(
                "not in an Android project root",
                "pas à la racine d'un projet Android"
            )
        ),
    }
    let Some(adb_path) = project::find_adb(project.as_ref()) else {
        println!("✗ {}", ScanError::NoAdb);
        return Ok(2);
    };
    println!("✓ adb: {}", adb_path.display());
    let adb = a11y_report_android::adb::Adb::new(adb_path, None);
    let devices = adb.devices().map_err(ScanError::from)?;
    if devices.is_empty() {
        println!("✗ {}", a11y_report_android::adb::AdbError::NoDevice);
        return Ok(2);
    }
    for d in devices {
        println!(
            "{} {} ({})",
            if d.state == "device" { "✓" } else { "✗" },
            d.serial,
            d.state
        );
    }
    Ok(0)
}

fn run(cli: Cli) -> Result<i32> {
    let lang: Lang = cli.lang.into();
    match cli.command {
        None => cmd_scan(cli.scan, lang),
        Some(Command::Scan(args)) => cmd_scan(args, lang),
        Some(Command::Capture(args)) => cmd_capture(args),
        Some(Command::Analyze(args)) => cmd_analyze(args, lang),
        Some(Command::Diff(args)) => cmd_diff(args, lang),
        Some(Command::Rules) => cmd_rules(lang),
        Some(Command::Schema) => {
            print!("{}", a11y_report_render::render_schema()?);
            Ok(0)
        }
        Some(Command::Demo(args)) => cmd_demo(args, lang),
        Some(Command::Doctor) => cmd_doctor(lang),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let level = match cli.verbose {
        0 => "warn",
        1 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("A11Y_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .with_target(false)
        .without_time()
        .init();
    match std::panic::catch_unwind(|| run(cli)) {
        Ok(Ok(code)) => ExitCode::from(u8::try_from(code).unwrap_or(3)),
        Ok(Err(e)) => {
            eprintln!("error: {e:#}");
            if is_input_error(&e) {
                ExitCode::from(2)
            } else {
                ExitCode::from(3)
            }
        }
        Err(_) => ExitCode::from(3),
    }
}
