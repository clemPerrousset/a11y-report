//! The report object: single source of truth for JSON, HTML, Markdown and CI.

use crate::criteria::Lang;
use crate::finding::{Finding, Severity};
use crate::model::Device;
use crate::num::ser_round2_opt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Bumped on any breaking change of the JSON layout.
pub const SCHEMA_VERSION: u32 = 1;
pub const TOOL_NAME: &str = "a11y-report";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "a11y-report report",
    description = "Automatic accessibility audit of a mobile app."
)]
pub struct Report {
    pub schema_version: u32,
    pub tool: ToolInfo,
    /// Language of the human-readable strings in this report.
    pub lang: Lang,
    pub run: RunInfo,
    pub summary: Summary,
    pub criteria: Vec<CriterionResult>,
    pub screens: Vec<ScreenSummary>,
    pub issues: Vec<Finding>,
    pub fixed: Vec<FindingRef>,
    pub not_rechecked: Vec<FindingRef>,
    pub manual_checks: Vec<ManualCheck>,
    pub known_limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
    /// Hash of rule ids, rule versions and effective configuration.
    pub rules_digest: String,
    /// Active rules, in execution order.
    pub rules: Vec<RuleInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RuleInfo {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub criteria: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunInfo {
    pub app: String,
    pub platform: String,
    pub build: String,
    /// Copied from the run manifest; the engine never generates timestamps.
    pub captured_at: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub package: Option<String>,
    pub device: Device,
    /// Successfully analysed screens, sorted by name.
    pub screens: Vec<String>,
    pub errors: Vec<RunError>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct RunError {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub screen: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScreenSummary {
    pub name: String,
    /// Foreground activity at capture time, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub activity: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub node_count: u32,
    pub interactive_count: u32,
    pub has_screenshot: bool,
    /// Rules that need a screenshot and could not run on this screen.
    pub rules_not_verified: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Summary {
    /// Share of applicable automatable criteria that pass (0–1). `null` when none applies.
    #[serde(serialize_with = "ser_round2_opt")]
    pub automatic_conformity_rate: Option<f64>,
    pub criteria: CriteriaCounts,
    pub issues: IssueCounts,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diff: Option<DiffSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct CriteriaCounts {
    pub applicable: u32,
    pub passed: u32,
    pub failed: u32,
}

/// Open (non-ignored) issues by severity, plus the ignored ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct IssueCounts {
    pub critical: u32,
    pub major: u32,
    pub minor: u32,
    pub info: u32,
    pub ignored: u32,
}

impl IssueCounts {
    pub fn get(&self, s: Severity) -> u32 {
        match s {
            Severity::Critical => self.critical,
            Severity::Major => self.major,
            Severity::Minor => self.minor,
            Severity::Info => self.info,
        }
    }

    pub fn open(&self) -> u32 {
        self.critical + self.major + self.minor + self.info
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiffSummary {
    pub baseline_build: String,
    #[serde(serialize_with = "ser_round2_opt")]
    pub baseline_rate: Option<f64>,
    pub new: u32,
    pub existing: u32,
    pub fixed: u32,
    pub not_rechecked: u32,
    /// The baseline was produced with different rules or configuration.
    pub rules_digest_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CriterionStatus {
    Pass,
    Fail,
    NotApplicable,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CriterionResult {
    pub id: String,
    /// e.g. `WCAG 1.4.3 · EN 301 549 11.1.4.3`.
    pub reference: String,
    pub level: String,
    pub title: String,
    pub automatable: bool,
    pub normative: bool,
    pub status: CriterionStatus,
    pub issue_count: u32,
    pub screens_failed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct FindingRef {
    pub id: String,
    pub rule: String,
    pub screen: String,
    pub severity: Severity,
}

impl From<&Finding> for FindingRef {
    fn from(f: &Finding) -> Self {
        FindingRef {
            id: f.id.clone(),
            rule: f.rule.clone(),
            screen: f.screen.clone(),
            severity: f.severity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ManualCheck {
    pub criterion: String,
    pub reference: String,
    pub title: String,
    pub why: String,
}
