//! Criterion statuses, issue counts and the automatic conformity rate.

use crate::criteria::{CATALOG, KNOWN_LIMITATIONS, Lang};
use crate::finding::{Finding, Severity};
use crate::report::{
    CriteriaCounts, CriterionResult, CriterionStatus, IssueCounts, ManualCheck, Summary,
};
use std::collections::BTreeSet;

/// Per-criterion results, in catalog order.
///
/// - `applicable`: criteria with at least one evaluated element on some screen;
/// - `unverified`: criteria whose rule could not run on some screen (no screenshot).
pub fn evaluate_criteria(
    findings: &[Finding],
    applicable: &BTreeSet<String>,
    unverified: &BTreeSet<String>,
    lang: Lang,
) -> Vec<CriterionResult> {
    CATALOG
        .iter()
        .map(|c| {
            let concerned: Vec<&Finding> = findings
                .iter()
                .filter(|f| !f.ignored && f.criteria.iter().any(|id| id == c.id))
                .collect();
            let screens_failed: BTreeSet<String> = concerned
                .iter()
                .filter(|f| f.is_blocking())
                .map(|f| f.screen.clone())
                .collect();
            let status = if !c.automatable {
                CriterionStatus::Manual
            } else if !screens_failed.is_empty() {
                CriterionStatus::Fail
            } else if applicable.contains(c.id) {
                CriterionStatus::Pass
            } else if unverified.contains(c.id) {
                CriterionStatus::Manual
            } else {
                CriterionStatus::NotApplicable
            };
            CriterionResult {
                id: c.id.to_string(),
                reference: c.reference(),
                level: c.level.to_string(),
                title: c.title(lang).to_string(),
                automatable: c.automatable,
                normative: c.normative,
                status,
                issue_count: u32::try_from(concerned.len()).unwrap_or(u32::MAX),
                screens_failed: screens_failed.into_iter().collect(),
            }
        })
        .collect()
}

pub fn issue_counts(findings: &[Finding]) -> IssueCounts {
    let mut counts = IssueCounts::default();
    for f in findings {
        if f.ignored {
            counts.ignored += 1;
            continue;
        }
        match f.severity {
            Severity::Critical => counts.critical += 1,
            Severity::Major => counts.major += 1,
            Severity::Minor => counts.minor += 1,
            Severity::Info => counts.info += 1,
        }
    }
    counts
}

/// Only normative automatable criteria count in the rate.
pub fn summarize(criteria: &[CriterionResult], findings: &[Finding]) -> Summary {
    let mut counts = CriteriaCounts::default();
    for c in criteria.iter().filter(|c| c.automatable && c.normative) {
        match c.status {
            CriterionStatus::Pass => counts.passed += 1,
            CriterionStatus::Fail => counts.failed += 1,
            CriterionStatus::NotApplicable | CriterionStatus::Manual => continue,
        }
        counts.applicable += 1;
    }
    let rate =
        (counts.applicable > 0).then(|| f64::from(counts.passed) / f64::from(counts.applicable));
    Summary {
        automatic_conformity_rate: rate,
        criteria: counts,
        issues: issue_counts(findings),
        diff: None,
    }
}

pub fn manual_checks(lang: Lang) -> Vec<ManualCheck> {
    CATALOG
        .iter()
        .filter_map(|c| {
            c.manual_note(lang).map(|why| ManualCheck {
                criterion: c.id.to_string(),
                reference: c.reference(),
                title: c.title(lang).to_string(),
                why: why.to_string(),
            })
        })
        .collect()
}

pub fn known_limitations(lang: Lang) -> Vec<String> {
    KNOWN_LIMITATIONS
        .iter()
        .map(|(en, fr)| lang.pick(en, fr).to_string())
        .collect()
}
