//! Comparison with a previous report: new, existing, fixed, not rechecked.

use crate::finding::{Finding, Status};
use crate::report::{DiffSummary, FindingRef, Report, SCHEMA_VERSION};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DiffError {
    #[error(
        "baseline uses schema_version {found}, this engine produces version {expected}: regenerate the baseline"
    )]
    SchemaMismatch { found: u32, expected: u32 },
}

/// Result of comparing a baseline with a newer run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiffOutcome {
    pub baseline_build: String,
    pub head_build: String,
    pub rules_digest_changed: bool,
    pub new: Vec<FindingRef>,
    pub existing: Vec<FindingRef>,
    /// Gone from a screen analysed in both runs.
    pub fixed: Vec<FindingRef>,
    /// On a screen missing from the newer run: not captured is not fixed.
    pub not_rechecked: Vec<FindingRef>,
}

fn check_schema(base: &Report) -> Result<(), DiffError> {
    if base.schema_version == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(DiffError::SchemaMismatch {
            found: base.schema_version,
            expected: SCHEMA_VERSION,
        })
    }
}

fn compare(
    base: &Report,
    head_issues: &[Finding],
    head_screens: &[String],
    head_build: &str,
    head_digest: &str,
) -> DiffOutcome {
    let base_ids: BTreeSet<&str> = base.issues.iter().map(|f| f.id.as_str()).collect();
    let head_ids: BTreeSet<&str> = head_issues.iter().map(|f| f.id.as_str()).collect();
    let head_screens: BTreeSet<&str> = head_screens.iter().map(String::as_str).collect();

    let mut out = DiffOutcome {
        baseline_build: base.run.build.clone(),
        head_build: head_build.to_string(),
        rules_digest_changed: base.tool.rules_digest != head_digest,
        new: Vec::new(),
        existing: Vec::new(),
        fixed: Vec::new(),
        not_rechecked: Vec::new(),
    };
    for f in head_issues.iter().filter(|f| !f.ignored) {
        if base_ids.contains(f.id.as_str()) {
            out.existing.push(f.into());
        } else {
            out.new.push(f.into());
        }
    }
    for f in &base.issues {
        if head_ids.contains(f.id.as_str()) {
            continue;
        }
        if head_screens.contains(f.screen.as_str()) {
            out.fixed.push(f.into());
        } else {
            out.not_rechecked.push(f.into());
        }
    }
    // Findings that were already not rechecked and whose screen is still missing stay so.
    for r in &base.not_rechecked {
        if !head_screens.contains(r.screen.as_str()) && !head_ids.contains(r.id.as_str()) {
            out.not_rechecked.push(r.clone());
        }
    }
    for list in [
        &mut out.new,
        &mut out.existing,
        &mut out.fixed,
        &mut out.not_rechecked,
    ] {
        list.sort();
        list.dedup();
    }
    out
}

/// Compares two existing reports without re-analysing (`a11y-report diff`).
pub fn diff_reports(base: &Report, head: &Report) -> Result<DiffOutcome, DiffError> {
    check_schema(base)?;
    check_schema(head)?;
    Ok(compare(
        base,
        &head.issues,
        &head.run.screens,
        &head.run.build,
        &head.tool.rules_digest,
    ))
}

/// Fills statuses, `fixed`, `not_rechecked` and `summary.diff` of a fresh report.
pub fn apply_baseline(report: &mut Report, base: &Report) -> Result<(), DiffError> {
    check_schema(base)?;
    let outcome = compare(
        base,
        &report.issues,
        &report.run.screens,
        &report.run.build,
        &report.tool.rules_digest,
    );
    let base_ids: BTreeSet<&str> = base.issues.iter().map(|f| f.id.as_str()).collect();
    for f in &mut report.issues {
        f.status = if base_ids.contains(f.id.as_str()) {
            Status::Existing
        } else {
            Status::New
        };
    }
    let count = |v: &Vec<FindingRef>| u32::try_from(v.len()).unwrap_or(u32::MAX);
    report.summary.diff = Some(DiffSummary {
        baseline_build: base.run.build.clone(),
        baseline_rate: base.summary.automatic_conformity_rate,
        new: count(&outcome.new),
        existing: count(&outcome.existing),
        fixed: count(&outcome.fixed),
        not_rechecked: count(&outcome.not_rechecked),
        rules_digest_changed: outcome.rules_digest_changed,
    });
    report.fixed = outcome.fixed;
    report.not_rechecked = outcome.not_rechecked;
    Ok(())
}
