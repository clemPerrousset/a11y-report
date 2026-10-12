//! `run.json`: the manifest of a capture run.

use a11y_report_core::model::Device;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MANIFEST_FILE: &str = "run.json";
pub const MANIFEST_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema: u32,
    pub app: String,
    #[serde(default = "default_platform")]
    pub platform: String,
    pub build: String,
    /// ISO 8601 UTC timestamp, e.g. `2026-10-12T09:30:00Z`.
    pub captured_at: String,
    /// Application id; when set, windows of other packages are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    pub device: ManifestDevice,
    pub screens: Vec<ManifestScreen>,
}

fn default_platform() -> String {
    "android".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDevice {
    #[serde(default = "unknown")]
    pub model: String,
    /// Required: without it no px → dp conversion is possible.
    pub density_dpi: Option<u32>,
    pub width_px: u32,
    pub height_px: u32,
    #[serde(default = "one")]
    pub font_scale: f64,
}

fn unknown() -> String {
    "unknown".to_string()
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestScreen {
    pub name: String,
    pub hierarchy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid run.json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported run.json schema {0} (expected {MANIFEST_SCHEMA})")]
    UnsupportedSchema(u32),
    #[error("device.density_dpi is required (dp = px × 160 / density_dpi)")]
    MissingDensity,
    #[error("device width_px and height_px must be positive")]
    InvalidSize,
    #[error("screen name \"{0}\" is invalid: use only a-z, 0-9, '_' and '-'")]
    InvalidScreenName(String),
    #[error("screen name \"{0}\" is used twice")]
    DuplicateScreen(String),
    #[error("run.json lists no screens")]
    NoScreens,
}

/// `[a-z0-9_-]+`.
pub fn is_valid_screen_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

impl RunManifest {
    pub fn parse(json: &str) -> Result<RunManifest, ManifestError> {
        let m: RunManifest = serde_json::from_str(json)?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema != MANIFEST_SCHEMA {
            return Err(ManifestError::UnsupportedSchema(self.schema));
        }
        match self.device.density_dpi {
            None | Some(0) => return Err(ManifestError::MissingDensity),
            Some(_) => {}
        }
        if self.device.width_px == 0 || self.device.height_px == 0 {
            return Err(ManifestError::InvalidSize);
        }
        if self.screens.is_empty() {
            return Err(ManifestError::NoScreens);
        }
        let mut names = BTreeSet::new();
        for s in &self.screens {
            if !is_valid_screen_name(&s.name) {
                return Err(ManifestError::InvalidScreenName(s.name.clone()));
            }
            if !names.insert(s.name.as_str()) {
                return Err(ManifestError::DuplicateScreen(s.name.clone()));
            }
        }
        Ok(())
    }

    /// Device model; call after [`RunManifest::validate`].
    pub fn device(&self) -> Device {
        Device {
            model: self.device.model.clone(),
            density_dpi: self.device.density_dpi.unwrap_or(160),
            width_px: self.device.width_px,
            height_px: self.device.height_px,
            font_scale: self.device.font_scale,
        }
    }

    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }
}

/// Formats seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_utc_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

/// Width and height read from a PNG header, without decoding.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = r#"{
      "schema": 1,
      "app": "pocket-garden",
      "platform": "android",
      "build": "2.4.0 (240)",
      "captured_at": "2026-10-12T09:30:00Z",
      "device": { "model": "Pixel 7", "density_dpi": 420, "width_px": 1080, "height_px": 2400, "font_scale": 1.0 },
      "screens": [ { "name": "home", "hierarchy": "screens/home.xml", "screenshot": "screens/home.png" } ]
    }"#;

    #[test]
    fn parses_and_validates() {
        let m = RunManifest::parse(RUN).expect("valid");
        assert_eq!(m.device().density_dpi, 420);
        assert_eq!(m.screens[0].screenshot.as_deref(), Some("screens/home.png"));
        let back = RunManifest::parse(&m.to_json()).expect("roundtrip");
        assert_eq!(back, m);
    }

    #[test]
    fn validation_errors() {
        let no_density = RUN.replace("\"density_dpi\": 420, ", "");
        assert!(matches!(
            RunManifest::parse(&no_density),
            Err(ManifestError::MissingDensity)
        ));
        let bad_name = RUN.replace("\"name\": \"home\"", "\"name\": \"Home Screen\"");
        assert!(matches!(
            RunManifest::parse(&bad_name),
            Err(ManifestError::InvalidScreenName(_))
        ));
        let v2 = RUN.replace("\"schema\": 1", "\"schema\": 2");
        assert!(matches!(
            RunManifest::parse(&v2),
            Err(ManifestError::UnsupportedSchema(2))
        ));
        assert!(matches!(
            RunManifest::parse("{"),
            Err(ManifestError::Json(_))
        ));
    }

    #[test]
    fn screen_names() {
        assert!(is_valid_screen_name("feed_2-b"));
        assert!(!is_valid_screen_name(""));
        assert!(!is_valid_screen_name("Feed"));
        assert!(!is_valid_screen_name("a/b"));
    }

    #[test]
    fn timestamps() {
        assert_eq!(format_utc_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc_timestamp(1_791_797_400), "2026-10-12T09:30:00Z");
        assert_eq!(format_utc_timestamp(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn png_header() {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&1080u32.to_be_bytes());
        bytes.extend_from_slice(&2400u32.to_be_bytes());
        assert_eq!(png_dimensions(&bytes), Some((1080, 2400)));
        assert_eq!(png_dimensions(b"not a png"), None);
    }
}
