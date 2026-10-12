//! Findings (issues) produced by rules, with their measured evidence.

use crate::a11y::accessible_name;
use crate::geometry::Rect;
use crate::model::{NodeId, Screen};
use crate::num::{ser_round2, ser_round2_opt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Ordered from most to least severe (`Critical < Info` in `Ord`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Major,
    Minor,
    Info,
}

impl Severity {
    pub const ALL: [Severity; 4] = [
        Severity::Critical,
        Severity::Major,
        Severity::Minor,
        Severity::Info,
    ];

    /// True when `self` is at least as severe as `threshold`.
    pub fn at_least(self, threshold: Severity) -> bool {
        self <= threshold
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Major => "major",
            Severity::Minor => "minor",
            Severity::Info => "info",
        }
    }

    pub fn parse(s: &str) -> Option<Severity> {
        Severity::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        }
    }
}

/// Lifecycle of a finding relative to the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Existing,
}

/// sRGB colour, serialized as `#RRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn parse_hex(s: &str) -> Option<Rgb> {
        let s = s.strip_prefix('#')?;
        if s.len() != 6 || !s.is_ascii() {
            return None;
        }
        let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        Some(Rgb::new(c(0)?, c(2)?, c(4)?))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgb::parse_hex(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid colour {s}")))
    }
}

impl JsonSchema for Rgb {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Rgb".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^#[0-9A-F]{6}$"
        })
    }
}

/// Identifies the element a finding is about, for humans and agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ElementRef {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub resource_id: Option<String>,
    pub class: String,
    /// Visible text, truncated to 80 characters.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub content_desc: Option<String>,
    /// Accessible name as TalkBack would announce it (normalized).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub accessible_name: Option<String>,
    pub bounds: Rect,
}

pub const ELEMENT_TEXT_MAX: usize = 80;

pub fn truncate_chars(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

impl ElementRef {
    pub fn from_node(screen: &Screen, id: NodeId) -> ElementRef {
        let Some(node) = screen.node(id) else {
            return ElementRef {
                resource_id: None,
                class: String::new(),
                text: None,
                content_desc: None,
                accessible_name: None,
                bounds: Rect::default(),
            };
        };
        let clip = |s: &Option<String>| {
            s.as_deref()
                .filter(|t| !t.trim().is_empty())
                .map(|t| truncate_chars(t, ELEMENT_TEXT_MAX))
        };
        ElementRef {
            resource_id: node.short_resource_id().map(str::to_string),
            class: node.class.clone(),
            text: clip(&node.text),
            content_desc: clip(&node.content_desc),
            accessible_name: accessible_name(screen, id)
                .map(|n| truncate_chars(&n, ELEMENT_TEXT_MAX)),
            bounds: node.bounds,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MissingNameKind {
    /// ImageView / ImageButton without description.
    Image,
    /// EditText: a hint or labelFor may exist but is not visible in the dump.
    EditText,
    /// Any other interactive element.
    Control,
}

/// Measured proof attached to every finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Evidence {
    MissingName {
        element_kind: MissingNameKind,
    },
    TargetSize {
        #[serde(serialize_with = "ser_round2")]
        width_dp: f64,
        #[serde(serialize_with = "ser_round2")]
        height_dp: f64,
        #[serde(serialize_with = "ser_round2")]
        min_dp: f64,
    },
    Contrast {
        /// Measured ratio, truncated (not rounded) to 2 decimals.
        #[serde(serialize_with = "ser_round2")]
        ratio: f64,
        #[serde(serialize_with = "ser_round2")]
        required: f64,
        fg: Rgb,
        bg: Rgb,
        #[serde(serialize_with = "ser_round2")]
        text_height_dp: f64,
        large_text: bool,
        /// Share of the border ring covered by the background colour (0–1).
        #[serde(serialize_with = "ser_round2")]
        background_uniformity: f64,
        /// Nearest text colour (same hue, adjusted lightness) that meets `required`.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        suggested_fg: Option<Rgb>,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default,
            serialize_with = "ser_round2_opt"
        )]
        suggested_ratio: Option<f64>,
    },
    DuplicateName {
        name: String,
        others: Vec<ElementRef>,
    },
}

/// Pointer to the project source that probably declares the element.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct SourceHint {
    /// Path relative to the project root, with `/` separators.
    pub file: String,
    pub line: u32,
    /// How the match was found (`layout_id`, `compose_test_tag`, `string_resource`, …).
    pub matched_by: String,
    /// The matching line, trimmed.
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// Stable fingerprint `{rule}:{screen}:{label}#{hash}`.
    pub id: String,
    pub rule: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub criteria: Vec<String>,
    pub screen: String,
    pub node: NodeId,
    pub element: ElementRef,
    pub evidence: Evidence,
    pub status: Status,
    pub ignored: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub source_hints: Vec<SourceHint>,
}

impl Finding {
    /// Builds a finding with an empty id; ids are assigned by [`crate::fingerprint`].
    pub fn new(
        rule: &str,
        severity: Severity,
        confidence: Confidence,
        criteria: &[&str],
        screen: &Screen,
        node: NodeId,
        evidence: Evidence,
    ) -> Finding {
        Finding {
            id: String::new(),
            rule: rule.to_string(),
            severity,
            confidence,
            criteria: criteria.iter().map(|c| (*c).to_string()).collect(),
            screen: screen.name.clone(),
            node,
            element: ElementRef::from_node(screen, node),
            evidence,
            status: Status::New,
            ignored: false,
            source_hints: Vec::new(),
        }
    }

    /// True when this finding can make a criterion fail (and count for `--fail-on`).
    pub fn is_blocking(&self) -> bool {
        !self.ignored && self.severity != Severity::Info && self.confidence != Confidence::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_order() {
        assert!(Severity::Critical.at_least(Severity::Major));
        assert!(Severity::Major.at_least(Severity::Major));
        assert!(!Severity::Minor.at_least(Severity::Major));
        assert_eq!(Severity::parse("minor"), Some(Severity::Minor));
        assert_eq!(Severity::parse("nope"), None);
    }

    #[test]
    fn rgb_hex_roundtrip() {
        let c = Rgb::new(0xFF, 0x8A, 0x5B);
        assert_eq!(c.hex(), "#FF8A5B");
        assert_eq!(Rgb::parse_hex("#FF8A5B"), Some(c));
        assert_eq!(Rgb::parse_hex("FF8A5B"), None);
        assert_eq!(Rgb::parse_hex("#é8A5B0"), None);
    }

    #[test]
    fn truncation_is_char_based() {
        assert_eq!(truncate_chars("héllo", 3), "hél…");
        assert_eq!(truncate_chars("abc", 3), "abc");
    }
}
