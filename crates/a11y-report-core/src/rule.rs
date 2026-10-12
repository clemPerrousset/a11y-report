//! The rule contract. Rules are pure: the context provides everything, rules do no I/O.

use crate::criteria::{CriterionId, Lang};
use crate::finding::{Finding, Severity};
use crate::model::{Device, Image, Screen};
use std::collections::{BTreeMap, BTreeSet};

/// Numeric parameter a rule accepts from `a11y.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamMeta {
    pub name: &'static str,
    pub default: f64,
    pub description_en: &'static str,
    pub description_fr: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleMeta {
    /// Stable id, e.g. `contrast-text`.
    pub id: &'static str,
    /// Bumped whenever the rule's behaviour changes (part of `rules_digest`).
    pub version: u32,
    pub title_en: &'static str,
    pub title_fr: &'static str,
    pub default_severity: Severity,
    pub criteria: &'static [CriterionId],
    pub needs_screenshot: bool,
    pub params: &'static [ParamMeta],
}

impl RuleMeta {
    pub fn title(&self, lang: Lang) -> &'static str {
        lang.pick(self.title_en, self.title_fr)
    }
}

/// Effective configuration of one rule (defaults merged with `a11y.toml`).
#[derive(Debug, Clone, PartialEq)]
pub struct RuleConfig {
    pub enabled: bool,
    pub severity: Option<Severity>,
    pub params: BTreeMap<String, f64>,
}

impl RuleConfig {
    /// Defaults declared by the rule's metadata.
    pub fn defaults(meta: &RuleMeta) -> RuleConfig {
        RuleConfig {
            enabled: true,
            severity: None,
            params: meta
                .params
                .iter()
                .map(|p| (p.name.to_string(), p.default))
                .collect(),
        }
    }

    pub fn param(&self, name: &str, default: f64) -> f64 {
        self.params.get(name).copied().unwrap_or(default)
    }
}

/// Everything a rule may look at for one screen.
pub struct ScreenCtx<'a> {
    pub screen: &'a Screen,
    pub device: &'a Device,
    /// Decoded once per screen, only when an active rule needs it.
    pub image: Option<&'a Image>,
    pub config: &'a RuleConfig,
}

/// What a rule produces on one screen.
#[derive(Debug, Default)]
pub struct RuleOutput {
    pub findings: Vec<Finding>,
    /// Criteria for which at least one concerned element was evaluated.
    pub applicable: BTreeSet<CriterionId>,
}

pub trait Rule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, ctx: &ScreenCtx<'_>, out: &mut RuleOutput);
}
