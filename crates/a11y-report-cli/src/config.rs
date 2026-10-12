//! `a11y.toml`: per-rule settings and accepted findings.

use a11y_report_core::{Rule, RuleConfig, Severity};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid a11y.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("a11y.toml: unknown section [{0}] (expected [rules.<id>] or [ignore])")]
    UnknownSection(String),
    #[error("a11y.toml: unknown rule \"{0}\" (see `a11y-report rules`)")]
    UnknownRule(String),
    #[error("a11y.toml: [rules.{rule}] has no setting \"{key}\"")]
    UnknownKey { rule: String, key: String },
    #[error("a11y.toml: [rules.{rule}] {key} must be {expected}")]
    WrongType {
        rule: String,
        key: String,
        expected: &'static str,
    },
    #[error("a11y.toml: [ignore] {0}")]
    Ignore(String),
    #[error("a11y.toml: invalid glob \"{pattern}\": {source}")]
    Glob {
        pattern: String,
        source: globset::Error,
    },
}

/// Effective configuration of an analysis.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub rules: BTreeMap<&'static str, RuleConfig>,
    pub ignore_resource_ids: Vec<String>,
    pub ignore_findings: BTreeSet<String>,
    ignore_set: GlobSet,
}

impl EngineConfig {
    /// Defaults of every registered rule, nothing ignored.
    pub fn defaults(rules: &[Box<dyn Rule>]) -> EngineConfig {
        EngineConfig {
            rules: rules
                .iter()
                .map(|r| (r.meta().id, RuleConfig::defaults(r.meta())))
                .collect(),
            ignore_resource_ids: Vec::new(),
            ignore_findings: BTreeSet::new(),
            ignore_set: GlobSet::empty(),
        }
    }

    /// Parses `a11y.toml` on top of the defaults.
    pub fn parse(text: &str, rules: &[Box<dyn Rule>]) -> Result<EngineConfig, ConfigError> {
        let mut cfg = EngineConfig::defaults(rules);
        let table: toml::Table = text.parse()?;
        for (section, value) in &table {
            match section.as_str() {
                "rules" => {
                    let Some(rules_table) = value.as_table() else {
                        return Err(ConfigError::UnknownSection(section.clone()));
                    };
                    for (rule_id, settings) in rules_table {
                        let Some(rule) = rules.iter().find(|r| r.meta().id == rule_id) else {
                            return Err(ConfigError::UnknownRule(rule_id.clone()));
                        };
                        let meta = rule.meta();
                        let entry = cfg
                            .rules
                            .entry(meta.id)
                            .or_insert_with(|| RuleConfig::defaults(meta));
                        let Some(settings) = settings.as_table() else {
                            return Err(ConfigError::UnknownRule(rule_id.clone()));
                        };
                        for (key, v) in settings {
                            let wrong = |expected| ConfigError::WrongType {
                                rule: rule_id.clone(),
                                key: key.clone(),
                                expected,
                            };
                            match key.as_str() {
                                "enabled" => {
                                    entry.enabled =
                                        v.as_bool().ok_or_else(|| wrong("true or false"))?
                                }
                                "severity" => {
                                    let s =
                                        v.as_str().and_then(Severity::parse).ok_or_else(|| {
                                            wrong("\"critical\", \"major\", \"minor\" or \"info\"")
                                        })?;
                                    entry.severity = Some(s);
                                }
                                param if meta.params.iter().any(|p| p.name == param) => {
                                    let n = v
                                        .as_float()
                                        .or_else(|| v.as_integer().map(|i| i as f64))
                                        .filter(|n| n.is_finite() && *n >= 0.0)
                                        .ok_or_else(|| wrong("a positive number"))?;
                                    entry.params.insert(param.to_string(), n);
                                }
                                _ => {
                                    return Err(ConfigError::UnknownKey {
                                        rule: rule_id.clone(),
                                        key: key.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                "ignore" => {
                    let Some(t) = value.as_table() else {
                        return Err(ConfigError::Ignore("must be a table".into()));
                    };
                    for (key, v) in t {
                        let list: Vec<String> = v
                            .as_array()
                            .and_then(|a| {
                                a.iter().map(|x| x.as_str().map(str::to_string)).collect()
                            })
                            .ok_or_else(|| {
                                ConfigError::Ignore(format!("{key} must be a list of strings"))
                            })?;
                        match key.as_str() {
                            "resource_ids" => cfg.ignore_resource_ids = list,
                            "findings" => cfg.ignore_findings = list.into_iter().collect(),
                            other => {
                                return Err(ConfigError::Ignore(format!("unknown key {other}")));
                            }
                        }
                    }
                }
                other => return Err(ConfigError::UnknownSection(other.to_string())),
            }
        }
        let mut builder = GlobSetBuilder::new();
        for pattern in &cfg.ignore_resource_ids {
            let glob = Glob::new(pattern).map_err(|source| ConfigError::Glob {
                pattern: pattern.clone(),
                source,
            })?;
            builder.add(glob);
        }
        cfg.ignore_set = builder.build().map_err(|source| ConfigError::Glob {
            pattern: cfg.ignore_resource_ids.join(", "),
            source,
        })?;
        Ok(cfg)
    }

    pub fn rule(&self, id: &str) -> Option<&RuleConfig> {
        self.rules.get(id)
    }

    pub fn ignores_resource_id(&self, resource_id: &str) -> bool {
        !self.ignore_resource_ids.is_empty() && self.ignore_set.is_match(resource_id)
    }

    /// Stable text form of the effective configuration, hashed into `rules_digest`.
    pub fn canonical(&self) -> String {
        let mut s = String::new();
        for (id, rc) in &self.rules {
            s.push_str(&format!(
                "{id}:enabled={}:severity={}",
                rc.enabled,
                rc.severity.map(|s| s.as_str()).unwrap_or("default")
            ));
            for (k, v) in &rc.params {
                s.push_str(&format!(":{k}={v}"));
            }
            s.push('\n');
        }
        s.push_str(&format!(
            "ignore.resource_ids={:?}\n",
            self.ignore_resource_ids
        ));
        s.push_str(&format!("ignore.findings={:?}\n", self.ignore_findings));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a11y_report_rules::all_rules;

    #[test]
    fn parses_spec_example() {
        let rules = all_rules();
        let cfg = EngineConfig::parse(
            r#"
[rules.target-size]
enabled = true
min_dp = 48
hard_min_dp = 24.5

[rules.contrast-text]
severity = "minor"

[ignore]
resource_ids = ["*:id/debug_*"]
findings = ["contrast-text:feed:btn_join#a41f09c2"]
"#,
            &rules,
        )
        .expect("valid");
        assert_eq!(
            cfg.rule("target-size").map(|r| r.param("hard_min_dp", 0.0)),
            Some(24.5)
        );
        assert_eq!(
            cfg.rule("contrast-text").and_then(|r| r.severity),
            Some(Severity::Minor)
        );
        assert!(cfg.ignores_resource_id("com.example:id/debug_panel"));
        assert!(!cfg.ignores_resource_id("com.example:id/btn_join"));
        assert!(
            cfg.ignore_findings
                .contains("contrast-text:feed:btn_join#a41f09c2")
        );
    }

    #[test]
    fn rejects_unknown_things() {
        let rules = all_rules();
        assert!(matches!(
            EngineConfig::parse("[rules.nope]\nenabled = true", &rules),
            Err(ConfigError::UnknownRule(_))
        ));
        assert!(matches!(
            EngineConfig::parse("[rules.target-size]\nmin = 3", &rules),
            Err(ConfigError::UnknownKey { .. })
        ));
        assert!(matches!(
            EngineConfig::parse("[rules.target-size]\nmin_dp = \"x\"", &rules),
            Err(ConfigError::WrongType { .. })
        ));
        assert!(matches!(
            EngineConfig::parse("[other]", &rules),
            Err(ConfigError::UnknownSection(_))
        ));
        assert!(matches!(
            EngineConfig::parse("not toml =", &rules),
            Err(ConfigError::Toml(_))
        ));
    }

    #[test]
    fn canonical_changes_with_settings() {
        let rules = all_rules();
        let a = EngineConfig::defaults(&rules).canonical();
        let b = EngineConfig::parse("[rules.target-size]\nmin_dp = 44", &rules)
            .expect("valid")
            .canonical();
        assert_ne!(a, b);
    }
}
