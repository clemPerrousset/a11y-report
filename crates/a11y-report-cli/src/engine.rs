//! The analysis pipeline: manifest → screens (in parallel) → findings → report.

use crate::config::EngineConfig;
use a11y_report_android::manifest::{ManifestScreen, png_dimensions};
use a11y_report_android::source::SourceIndex;
use a11y_report_android::{
    MANIFEST_FILE, ManifestError, ParseOptions, RunManifest, parse_hierarchy,
};
use a11y_report_core::a11y::is_interactive;
use a11y_report_core::diff::{DiffError, apply_baseline};
use a11y_report_core::fingerprint::assign_ids;
use a11y_report_core::report::{
    Report, RuleInfo, RunError, RunInfo, SCHEMA_VERSION, ScreenSummary, TOOL_NAME, ToolInfo,
};
use a11y_report_core::rule::{RuleOutput, ScreenCtx};
use a11y_report_core::summary::{evaluate_criteria, known_limitations, manual_checks, summarize};
use a11y_report_core::{Device, Finding, Image, Lang, Rule, Severity};
use a11y_report_render::Thumbnail;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
const THUMB_WIDTH: u32 = 360;
const THUMB_QUALITY: u8 = 80;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Manifest {
        path: PathBuf,
        source: ManifestError,
    },
    #[error("baseline {path}: {message}")]
    Baseline { path: PathBuf, message: String },
    #[error(transparent)]
    Diff(#[from] DiffError),
}

pub struct AnalyzeOptions {
    pub run_dir: PathBuf,
    pub config: EngineConfig,
    pub rules: Vec<Box<dyn Rule>>,
    pub baseline: Option<Report>,
    pub lang: Lang,
    /// Android project root used to locate the source of each element.
    pub project_root: Option<PathBuf>,
    /// Produce JPEG thumbnails for the HTML report.
    pub thumbnails: bool,
}

pub struct Analysis {
    pub report: Report,
    pub thumbnails: BTreeMap<String, Thumbnail>,
}

struct ScreenOutcome {
    summary: ScreenSummary,
    findings: Vec<Finding>,
    applicable: BTreeSet<&'static str>,
    unverified: BTreeSet<&'static str>,
    thumbnail: Option<Thumbnail>,
}

/// Reads a previous report, refusing other schema versions instead of guessing.
pub fn load_report(path: &Path) -> Result<Report, EngineError> {
    let text = fs::read_to_string(path).map_err(|source| EngineError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let bad = |message: String| EngineError::Baseline {
        path: path.to_path_buf(),
        message,
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| bad(format!("invalid JSON: {e}")))?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64);
    if version != Some(u64::from(SCHEMA_VERSION)) {
        return Err(bad(format!(
            "schema_version is {}, this engine reads version {SCHEMA_VERSION}: no approximate diff is made",
            version.map_or_else(|| "missing".to_string(), |v| v.to_string())
        )));
    }
    serde_json::from_value(value).map_err(|e| bad(format!("not an a11y-report report: {e}")))
}

/// Hash of the active rules (id + version) and of the effective configuration.
pub fn rules_digest(rules: &[&dyn Rule], config: &EngineConfig) -> String {
    let mut h = blake3::Hasher::new();
    for r in rules {
        let m = r.meta();
        h.update(format!("{}@{}\n", m.id, m.version).as_bytes());
    }
    h.update(config.canonical().as_bytes());
    h.finalize().to_hex().as_str()[..8].to_string()
}

fn decode(bytes: &[u8]) -> Result<image::RgbImage, String> {
    image::load_from_memory(bytes)
        .map(|img| img.to_rgb8())
        .map_err(|e| format!("cannot decode screenshot: {e}"))
}

fn thumbnail(img: &image::RgbImage) -> Option<Thumbnail> {
    let resized;
    let src = if img.width() > THUMB_WIDTH {
        let h = (u64::from(img.height()) * u64::from(THUMB_WIDTH) / u64::from(img.width())).max(1);
        resized = image::imageops::resize(
            img,
            THUMB_WIDTH,
            u32::try_from(h).unwrap_or(1),
            image::imageops::FilterType::Triangle,
        );
        &resized
    } else {
        img
    };
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, THUMB_QUALITY)
        .encode_image(src)
        .ok()?;
    Some(Thumbnail {
        jpeg,
        width: src.width(),
        height: src.height(),
    })
}

#[allow(clippy::too_many_arguments)]
fn analyze_screen(
    run_dir: &Path,
    entry: &ManifestScreen,
    manifest: &RunManifest,
    device: &Device,
    rules: &[&dyn Rule],
    config: &EngineConfig,
    sources: Option<&SourceIndex>,
    want_thumbnail: bool,
) -> Result<ScreenOutcome, String> {
    let xml_path = run_dir.join(&entry.hierarchy);
    let xml = fs::read_to_string(&xml_path)
        .map_err(|e| format!("cannot read {}: {e}", xml_path.display()))?;
    let screen = parse_hierarchy(
        &entry.name,
        &xml,
        &ParseOptions {
            width_px: device.width_px,
            height_px: device.height_px,
            package: manifest.package.clone(),
        },
    )
    .map_err(|e| format!("{}: {e}", entry.hierarchy))?;

    let needs_image = rules.iter().any(|r| r.meta().needs_screenshot);
    let mut image: Option<Image> = None;
    let mut thumb = None;
    let has_screenshot = entry.screenshot.is_some();
    if let Some(rel) = &entry.screenshot {
        let path = run_dir.join(rel);
        let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if let Some((w, h)) = png_dimensions(&bytes)
            && (w, h) != (device.width_px, device.height_px)
        {
            return Err(format!(
                "{rel} is {w}×{h} px but the device is {}×{} px (rotated screen or wrong file)",
                device.width_px, device.height_px
            ));
        }
        if needs_image || want_thumbnail {
            let rgb = decode(&bytes)?;
            if (rgb.width(), rgb.height()) != (device.width_px, device.height_px) {
                return Err(format!(
                    "{rel} is {}×{} px but the device is {}×{} px",
                    rgb.width(),
                    rgb.height(),
                    device.width_px,
                    device.height_px
                ));
            }
            if want_thumbnail {
                thumb = thumbnail(&rgb);
            }
            if needs_image {
                let (w, h) = rgb.dimensions();
                image = Image::new(w, h, rgb.into_raw());
            }
        }
    }

    let mut findings = Vec::new();
    let mut applicable = BTreeSet::new();
    let mut unverified = BTreeSet::new();
    let mut not_verified_rules = Vec::new();
    for rule in rules {
        let meta = rule.meta();
        let Some(rc) = config.rule(meta.id) else {
            continue;
        };
        if meta.needs_screenshot && image.is_none() {
            not_verified_rules.push(meta.id.to_string());
            unverified.extend(meta.criteria.iter().copied());
            continue;
        }
        let ctx = ScreenCtx {
            screen: &screen,
            device,
            image: image.as_ref(),
            config: rc,
        };
        let mut out = RuleOutput::default();
        rule.check(&ctx, &mut out);
        if let Some(sev) = rc.severity {
            for f in &mut out.findings {
                if f.severity != Severity::Info {
                    f.severity = sev;
                }
            }
        }
        findings.extend(out.findings);
        applicable.extend(out.applicable);
    }

    findings.sort_by(|a, b| a.node.cmp(&b.node).then_with(|| a.rule.cmp(&b.rule)));
    assign_ids(&screen, &mut findings);
    for f in &mut findings {
        let full_id = screen.node(f.node).and_then(|n| n.resource_id.as_deref());
        if full_id.is_some_and(|id| config.ignores_resource_id(id))
            || config.ignore_findings.contains(&f.id)
        {
            f.ignored = true;
        }
        if let Some(idx) = sources {
            f.source_hints = idx.hints(
                f.element.resource_id.as_deref(),
                screen.node(f.node).and_then(|n| n.text.as_deref()),
                screen.node(f.node).and_then(|n| n.content_desc.as_deref()),
            );
        }
    }

    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Ok(ScreenOutcome {
        summary: ScreenSummary {
            name: entry.name.clone(),
            activity: entry.activity.clone(),
            width_px: device.width_px,
            height_px: device.height_px,
            node_count: count(screen.nodes.len()),
            interactive_count: count(screen.nodes.iter().filter(|n| is_interactive(n)).count()),
            has_screenshot,
            rules_not_verified: not_verified_rules,
        },
        findings,
        applicable,
        unverified,
        thumbnail: thumb,
    })
}

/// Runs a full analysis of a run directory.
pub fn analyze(opts: &AnalyzeOptions) -> Result<Analysis, EngineError> {
    let manifest_path = opts.run_dir.join(MANIFEST_FILE);
    let text = fs::read_to_string(&manifest_path).map_err(|source| EngineError::Read {
        path: manifest_path.clone(),
        source,
    })?;
    let manifest = RunManifest::parse(&text).map_err(|source| EngineError::Manifest {
        path: manifest_path.clone(),
        source,
    })?;
    let device = manifest.device();

    let active: Vec<&dyn Rule> = opts
        .rules
        .iter()
        .filter(|r| opts.config.rule(r.meta().id).is_some_and(|c| c.enabled))
        .map(Box::as_ref)
        .collect();
    let digest = rules_digest(&active, &opts.config);

    let sources = opts.project_root.as_deref().map(SourceIndex::build);
    if let Some(idx) = &sources {
        tracing::debug!(files = idx.file_count(), "source index built");
    }

    let mut entries: Vec<&ManifestScreen> = manifest.screens.iter().collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let outcomes: Vec<(String, Result<ScreenOutcome, String>)> = entries
        .par_iter()
        .map(|e| {
            (
                e.name.clone(),
                analyze_screen(
                    &opts.run_dir,
                    e,
                    &manifest,
                    &device,
                    &active,
                    &opts.config,
                    sources.as_ref(),
                    opts.thumbnails,
                ),
            )
        })
        .collect();

    let mut screens = Vec::new();
    let mut names = Vec::new();
    let mut issues = Vec::new();
    let mut applicable = BTreeSet::new();
    let mut unverified = BTreeSet::new();
    let mut errors = Vec::new();
    let mut thumbnails = BTreeMap::new();
    for (name, outcome) in outcomes {
        match outcome {
            Ok(o) => {
                names.push(name.clone());
                screens.push(o.summary);
                issues.extend(o.findings);
                applicable.extend(o.applicable.into_iter().map(str::to_string));
                unverified.extend(o.unverified.into_iter().map(str::to_string));
                if let Some(t) = o.thumbnail {
                    thumbnails.insert(name, t);
                }
            }
            Err(message) => {
                tracing::warn!(screen = %name, "{message}");
                errors.push(RunError {
                    screen: Some(name),
                    message,
                });
            }
        }
    }

    let criteria = evaluate_criteria(&issues, &applicable, &unverified, opts.lang);
    let summary = summarize(&criteria, &issues);
    let mut report = Report {
        schema_version: SCHEMA_VERSION,
        tool: ToolInfo {
            name: TOOL_NAME.to_string(),
            version: TOOL_VERSION.to_string(),
            rules_digest: digest,
            rules: active
                .iter()
                .map(|r| {
                    let m = r.meta();
                    RuleInfo {
                        id: m.id.to_string(),
                        version: m.version,
                        title: m.title(opts.lang).to_string(),
                        criteria: m.criteria.iter().map(|c| (*c).to_string()).collect(),
                    }
                })
                .collect(),
        },
        lang: opts.lang,
        run: RunInfo {
            app: manifest.app.clone(),
            platform: manifest.platform.clone(),
            build: manifest.build.clone(),
            captured_at: manifest.captured_at.clone(),
            package: manifest.package.clone(),
            device,
            screens: names,
            errors,
        },
        summary,
        criteria,
        screens,
        issues,
        fixed: Vec::new(),
        not_rechecked: Vec::new(),
        manual_checks: manual_checks(opts.lang),
        known_limitations: known_limitations(opts.lang),
    };
    if let Some(base) = &opts.baseline {
        apply_baseline(&mut report, base)?;
    }
    Ok(Analysis { report, thumbnails })
}
