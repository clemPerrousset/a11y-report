//! Library behind the `a11y-report` binary: the analysis pipeline, configuration,
//! adb capture sessions and output writing. The binary is a thin clap layer on top,
//! so another front end (e.g. an MCP server) can reuse all of it.

pub mod config;
pub mod engine;
pub mod scan;
pub mod synth;

use a11y_report_core::{Lang, Report, Severity, Status};
use a11y_report_render::i18n::{self, labels, number, percent};
use engine::Analysis;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Which files to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formats {
    pub json: bool,
    pub html: bool,
    pub ai: bool,
}

impl Default for Formats {
    fn default() -> Self {
        Formats {
            json: true,
            html: true,
            ai: true,
        }
    }
}

impl Formats {
    /// `json,html,md` (`ai` is an alias of `md`).
    pub fn parse(s: &str) -> Result<Formats, String> {
        let mut f = Formats {
            json: false,
            html: false,
            ai: false,
        };
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "json" => f.json = true,
                "html" => f.html = true,
                "md" | "ai" | "markdown" => f.ai = true,
                other => {
                    return Err(format!(
                        "unknown format \"{other}\" (expected json, html, md)"
                    ));
                }
            }
        }
        if !(f.json || f.html || f.ai) {
            return Err("no output format selected".into());
        }
        Ok(f)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Render(#[from] a11y_report_render::RenderError),
}

fn write_file(path: &Path, content: &[u8]) -> Result<(), OutputError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| OutputError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| OutputError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn relative_link(from_dir: &Path, target: &Path) -> String {
    // Both paths are built from the same base by the CLI; fall back to an absolute path.
    let from: Vec<_> = from_dir.components().collect();
    let to: Vec<_> = target.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return target.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = vec!["..".into(); from.len() - common];
    parts.extend(
        to[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// Writes the selected outputs into `out_dir`; returns the main file (HTML if written).
pub fn write_outputs(
    analysis: &Analysis,
    run_dir: &Path,
    out_dir: &Path,
    formats: Formats,
) -> Result<PathBuf, OutputError> {
    let report = &analysis.report;
    let mut main = out_dir.join("report.json");
    if formats.json {
        write_file(&main, a11y_report_render::render_json(report)?.as_bytes())?;
    }
    if formats.ai {
        let ai_dir = out_dir.join("ai");
        // Remove briefs of screens that no longer exist.
        if ai_dir.is_dir()
            && let Ok(entries) = fs::read_dir(&ai_dir)
        {
            for e in entries.filter_map(Result::ok) {
                if e.path().extension().is_some_and(|x| x == "md") {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
        let shots: BTreeMap<String, String> = report
            .screens
            .iter()
            .filter(|s| s.has_screenshot)
            .map(|s| {
                let png = run_dir.join("screens").join(format!("{}.png", s.name));
                (s.name.clone(), relative_link(&ai_dir, &png))
            })
            .collect();
        for f in a11y_report_render::render_ai_briefs(report, &shots) {
            write_file(&ai_dir.join(&f.path), f.content.as_bytes())?;
        }
        main = ai_dir.join("README.md");
    }
    if formats.html {
        main = out_dir.join("report.html");
        let html = a11y_report_render::render_html(report, &analysis.thumbnails)?;
        write_file(&main, html.as_bytes())?;
    }
    Ok(main)
}

/// 0: below the threshold, 1: threshold exceeded, 2: some input was invalid.
pub fn exit_code(report: &Report, fail_on: Option<Severity>, new_only: bool) -> i32 {
    if !report.run.errors.is_empty() {
        return 2;
    }
    let Some(threshold) = fail_on else { return 0 };
    let failing = report.issues.iter().any(|f| {
        f.is_blocking() && f.severity.at_least(threshold) && (!new_only || f.status == Status::New)
    });
    i32::from(failing)
}

/// The 3-line stdout summary: rate and trend, counts by severity, report path.
pub fn summary_lines(report: &Report, main: &Path, lang: Lang) -> [String; 3] {
    let l = labels(lang);
    let s = &report.summary;
    let mut rate = format!(
        "{}{} {}",
        l.summary_line_rate,
        l.colon,
        s.automatic_conformity_rate
            .map(|r| percent(r, lang))
            .unwrap_or_else(|| l.not_available.to_string())
    );
    if let Some(d) = &s.diff
        && let (Some(now), Some(before)) = (s.automatic_conformity_rate, d.baseline_rate)
    {
        let pts = ((now - before) * 100.0).round();
        rate.push_str(&format!(
            " ({}{} {} {})",
            if pts > 0.0 { "+" } else { "" },
            number(pts, lang),
            l.points_vs,
            d.baseline_build
        ));
    }
    let mut issues = format!(
        "{}{} {} {} · {} {} · {} {} · {} {}",
        l.summary_line_issues,
        l.colon,
        s.issues.critical,
        i18n::severity(Severity::Critical, lang).to_lowercase(),
        s.issues.major,
        i18n::severity(Severity::Major, lang).to_lowercase(),
        s.issues.minor,
        i18n::severity(Severity::Minor, lang).to_lowercase(),
        s.issues.info,
        i18n::severity(Severity::Info, lang).to_lowercase(),
    );
    if s.issues.ignored > 0 {
        issues.push_str(&format!(" ({} {})", s.issues.ignored, l.ignored_issues));
    }
    if let Some(d) = &s.diff {
        issues.push_str(&format!(
            " — {} {}, {} {}",
            d.new,
            l.new_issues.to_lowercase(),
            d.fixed,
            l.fixed_issues.to_lowercase()
        ));
    }
    let mut path = format!("{}{} {}", l.summary_line_report, l.colon, main.display());
    if !report.run.errors.is_empty() {
        path.push_str(&format!(
            " — {} {}",
            report.run.errors.len(),
            lang.pick("screen(s) in error", "écran(s) en erreur")
        ));
    }
    [rate, issues, path]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(
            Formats::parse("json,html").map(|f| (f.json, f.html, f.ai)),
            Ok((true, true, false))
        );
        assert!(Formats::parse("pdf").is_err());
        assert!(Formats::parse("").is_err());
    }

    #[test]
    fn links() {
        assert_eq!(
            relative_link(Path::new("out/ai"), Path::new("out/run/screens/home.png")),
            "../run/screens/home.png"
        );
    }
}
