//! Comparison with a previous report: new, existing, fixed, not rechecked.

use crate::finding::Status;
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

/// Ids known from the baseline: its issues plus those it carried as not rechecked.
fn known_ids(base: &Report) -> BTreeSet<&str> {
    base.issues
        .iter()
        .map(|f| f.id.as_str())
        .chain(base.not_rechecked.iter().map(|r| r.id.as_str()))
        .collect()
}

fn compare(base: &Report, head: &Report) -> DiffOutcome {
    let base_ids = known_ids(base);
    let head_ids: BTreeSet<&str> = head.issues.iter().map(|f| f.id.as_str()).collect();
    let head_screens: BTreeSet<&str> = head.run.screens.iter().map(String::as_str).collect();
    let head_rules: BTreeSet<&str> = head.tool.rules.iter().map(|r| r.id.as_str()).collect();
    // (screen, rule) pairs that could not run in the head (e.g. no screenshot).
    let unverified: BTreeSet<(&str, &str)> = head
        .screens
        .iter()
        .flat_map(|s| {
            s.rules_not_verified
                .iter()
                .map(move |r| (s.name.as_str(), r.as_str()))
        })
        .collect();
    // A baseline finding is rechecked only if its screen was analysed and its rule ran there.
    let rechecked = |screen: &str, rule: &str| {
        head_screens.contains(screen)
            && head_rules.contains(rule)
            && !unverified.contains(&(screen, rule))
    };

    let mut out = DiffOutcome {
        baseline_build: base.run.build.clone(),
        head_build: head.run.build.clone(),
        rules_digest_changed: base.tool.rules_digest != head.tool.rules_digest,
        new: Vec::new(),
        existing: Vec::new(),
        fixed: Vec::new(),
        not_rechecked: Vec::new(),
    };
    for f in head.issues.iter().filter(|f| !f.ignored) {
        if base_ids.contains(f.id.as_str()) {
            out.existing.push(f.into());
        } else {
            out.new.push(f.into());
        }
    }
    let previous = base
        .issues
        .iter()
        .map(FindingRef::from)
        .chain(base.not_rechecked.iter().cloned());
    for r in previous {
        if head_ids.contains(r.id.as_str()) {
            continue;
        }
        if rechecked(&r.screen, &r.rule) {
            out.fixed.push(r);
        } else {
            out.not_rechecked.push(r);
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
    Ok(compare(base, head))
}

/// Fills statuses, `fixed`, `not_rechecked` and `summary.diff` of a fresh report.
pub fn apply_baseline(report: &mut Report, base: &Report) -> Result<(), DiffError> {
    check_schema(base)?;
    let outcome = compare(base, report);
    let base_ids = known_ids(base);
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
