//! Android input layer of a11y-report: uiautomator dumps, run manifests, adb capture,
//! Gradle project detection and source hints.

pub mod adb;
pub mod manifest;
pub mod project;
pub mod source;
pub mod uiautomator;

pub use manifest::{MANIFEST_FILE, ManifestError, RunManifest};
pub use uiautomator::{ParseError, ParseOptions, parse_hierarchy};
