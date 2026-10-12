//! Core of the a11y-report engine: data model, rule contract, criteria catalog,
//! stable fingerprints, baseline diff and summary. This crate performs no I/O.

pub mod a11y;
pub mod criteria;
pub mod diff;
pub mod finding;
pub mod fingerprint;
pub mod geometry;
pub mod model;
pub mod num;
pub mod report;
pub mod rule;
pub mod summary;

pub use criteria::{Criterion, CriterionId, Lang};
pub use finding::{Confidence, ElementRef, Evidence, Finding, Rgb, Severity, SourceHint, Status};
pub use geometry::Rect;
pub use model::{Device, Image, Node, NodeFlags, NodeId, Screen};
pub use report::Report;
pub use rule::{Rule, RuleConfig, RuleMeta, RuleOutput, ScreenCtx};
