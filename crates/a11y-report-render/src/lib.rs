//! Renderers: JSON (source of truth) and its JSON Schema, a self-contained accessible
//! HTML report for humans, and per-screen Markdown briefs for AI coding agents.
//! Everything is derived from the [`Report`] object, without extra business logic.

pub mod guidance;
pub mod html;
pub mod i18n;
pub mod markdown;

use a11y_report_core::Report;

pub use html::render_html;
pub use markdown::{MdFile, render_ai_briefs};

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("HTML template error: {0}")]
    Template(#[from] askama::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// JPEG thumbnail of a screen (≤ 360 px wide), embedded in the HTML report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Pretty JSON with a trailing newline.
pub fn render_json(report: &Report) -> Result<String, RenderError> {
    let mut s = serde_json::to_string_pretty(report)?;
    s.push('\n');
    Ok(s)
}

/// JSON Schema of the report (published as `schema/report.v1.json`).
pub fn render_schema() -> Result<String, RenderError> {
    let schema = schemars::schema_for!(Report);
    let mut s = serde_json::to_string_pretty(&schema)?;
    s.push('\n');
    Ok(s)
}
