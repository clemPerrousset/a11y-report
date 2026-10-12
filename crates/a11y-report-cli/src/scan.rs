//! Guided capture over adb: the user navigates on the phone, the tool captures each screen
//! (uiautomator dump + screenshot) into a run directory.

use a11y_report_android::adb::{Adb, AdbError, DeviceInfo, suggest_screen_name};
use a11y_report_android::manifest::{
    MANIFEST_SCHEMA, ManifestDevice, ManifestScreen, format_utc_timestamp, is_valid_screen_name,
    png_dimensions,
};
use a11y_report_android::project::{AndroidProject, find_adb};
use a11y_report_android::{
    MANIFEST_FILE, ManifestError, ParseOptions, RunManifest, parse_hierarchy,
};
use a11y_report_core::Lang;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(
        "adb was not found: install Android platform-tools (Android Studio > SDK Manager) or add adb to your PATH"
    )]
    NoAdb,
    #[error(transparent)]
    Adb(#[from] AdbError),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{0}")]
    Manifest(#[from] ManifestError),
    #[error(
        "existing {path} was captured on another device ({existing}); use a new output folder or drop --append"
    )]
    DeviceMismatch { path: PathBuf, existing: String },
    #[error("no screen was captured")]
    NothingCaptured,
    #[error("invalid screen name \"{0}\": use only a-z, 0-9, '_' and '-'")]
    InvalidName(String),
    #[error("{0}")]
    Capture(String),
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> ScanError + '_ {
    move |source| ScanError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub struct SessionOptions {
    pub run_dir: PathBuf,
    pub serial: Option<String>,
    /// Application id; defaults to the detected project's.
    pub package: Option<String>,
    /// Keep previous captures of the run directory.
    pub append: bool,
    pub project: Option<AndroidProject>,
}

/// A capture session bound to one device and one run directory.
pub struct Session {
    pub adb: Adb,
    pub device: DeviceInfo,
    pub manifest: RunManifest,
    pub run_dir: PathBuf,
}

/// `Home Screen!` -> `home_screen`.
pub fn sanitize_name(input: &str) -> String {
    let mut out = String::new();
    for c in input.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' {
            out.push(c);
        } else if (c.is_whitespace() || c == '.' || c == '/') && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn unique_name(base: &str, manifest: &RunManifest) -> String {
    let taken = |n: &str| manifest.screens.iter().any(|s| s.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base}_{i}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

/// What a capture produced.
pub struct Captured {
    pub name: String,
    pub nodes: usize,
    pub activity: Option<String>,
}

impl Session {
    pub fn open(opts: SessionOptions) -> Result<Session, ScanError> {
        let adb_path = find_adb(opts.project.as_ref()).ok_or(ScanError::NoAdb)?;
        let mut adb = Adb::new(adb_path, opts.serial.clone());
        adb.select_device()?;
        let device = adb.device_info()?;
        let package = opts
            .package
            .clone()
            .or_else(|| opts.project.as_ref().and_then(|p| p.application_id.clone()));
        let build = package
            .as_deref()
            .and_then(|p| adb.package_version(p))
            .or_else(|| opts.project.as_ref().and_then(|p| p.version_name.clone()))
            .unwrap_or_else(|| "unknown".into());
        let app = opts
            .project
            .as_ref()
            .and_then(|p| p.app_name.clone())
            .or_else(|| package.clone())
            .unwrap_or_else(|| "android-app".into());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let manifest_path = opts.run_dir.join(MANIFEST_FILE);
        let mut manifest = None;
        if opts.append && manifest_path.is_file() {
            let text = fs::read_to_string(&manifest_path).map_err(io_err(&manifest_path))?;
            let existing: RunManifest = serde_json::from_str(&text).map_err(ManifestError::from)?;
            if (
                existing.device.width_px,
                existing.device.height_px,
                existing.device.density_dpi,
            ) != (device.width_px, device.height_px, Some(device.density_dpi))
            {
                return Err(ScanError::DeviceMismatch {
                    path: manifest_path,
                    existing: existing.device.model,
                });
            }
            manifest = Some(existing);
        } else if manifest_path.is_file() {
            // A fresh session replaces the previous captures of this run directory only.
            let screens = opts.run_dir.join("screens");
            if screens.is_dir() {
                fs::remove_dir_all(&screens).map_err(io_err(&screens))?;
            }
            fs::remove_file(&manifest_path).map_err(io_err(&manifest_path))?;
        }
        let manifest = manifest.unwrap_or_else(|| RunManifest {
            schema: MANIFEST_SCHEMA,
            app,
            platform: "android".into(),
            build,
            captured_at: format_utc_timestamp(now),
            package,
            device: ManifestDevice {
                model: device.model.clone(),
                density_dpi: Some(device.density_dpi),
                width_px: device.width_px,
                height_px: device.height_px,
                font_scale: device.font_scale,
            },
            screens: Vec::new(),
        });
        let screens_dir = opts.run_dir.join("screens");
        fs::create_dir_all(&screens_dir).map_err(io_err(&screens_dir))?;
        Ok(Session {
            adb,
            device,
            manifest,
            run_dir: opts.run_dir,
        })
    }

    /// Suggested name for the screen currently shown.
    pub fn suggestion(&self) -> (String, Option<String>) {
        let activity = self.adb.foreground_activity();
        let base = activity
            .as_deref()
            .map(suggest_screen_name)
            .unwrap_or_else(|| "screen".into());
        (unique_name(&base, &self.manifest), activity)
    }

    /// Captures the current screen as `name` (replacing a previous capture with that name).
    pub fn capture(&mut self, name: &str) -> Result<Captured, ScanError> {
        if !is_valid_screen_name(name) {
            return Err(ScanError::InvalidName(name.to_string()));
        }
        let activity = self.adb.foreground_activity();
        let xml = self.adb.dump_hierarchy()?;
        let png = self.adb.screenshot_png()?;
        let (w, h) = png_dimensions(&png)
            .ok_or_else(|| ScanError::Capture("the screenshot is not a PNG image".into()))?;
        if (w, h) != (self.device.width_px, self.device.height_px) {
            return Err(ScanError::Capture(format!(
                "the screenshot is {w}×{h} px but the device reports {}×{} px: rotate the phone to portrait and retry",
                self.device.width_px, self.device.height_px
            )));
        }
        let parsed = parse_hierarchy(
            name,
            &xml,
            &ParseOptions {
                width_px: w,
                height_px: h,
                package: self.manifest.package.clone(),
            },
        )
        .map_err(|e| ScanError::Capture(format!("the hierarchy dump is invalid: {e}")))?;
        if parsed.nodes.is_empty() {
            return Err(ScanError::Capture(format!(
                "{} is not on screen (foreground: {}): not captured",
                self.manifest.package.as_deref().unwrap_or("the app"),
                activity.as_deref().unwrap_or("unknown")
            )));
        }
        let xml_rel = format!("screens/{name}.xml");
        let png_rel = format!("screens/{name}.png");
        let xml_path = self.run_dir.join(&xml_rel);
        let png_path = self.run_dir.join(&png_rel);
        fs::write(&xml_path, &xml).map_err(io_err(&xml_path))?;
        fs::write(&png_path, &png).map_err(io_err(&png_path))?;
        self.manifest.screens.retain(|s| s.name != name);
        self.manifest.screens.push(ManifestScreen {
            name: name.to_string(),
            hierarchy: xml_rel,
            screenshot: Some(png_rel),
            activity: activity.clone(),
        });
        self.save()?;
        Ok(Captured {
            name: name.to_string(),
            nodes: parsed.nodes.len(),
            activity,
        })
    }

    pub fn save(&self) -> Result<(), ScanError> {
        let path = self.run_dir.join(MANIFEST_FILE);
        fs::write(&path, self.manifest.to_json()).map_err(io_err(&path))
    }
}

/// Interactive loop: Enter captures, a typed name names the capture, `q` finishes.
/// Prompts go to `ui` (stderr in the CLI) so stdout stays clean.
pub fn interactive(
    session: &mut Session,
    input: &mut impl BufRead,
    ui: &mut impl Write,
    lang: Lang,
) -> Result<usize, ScanError> {
    let d = &session.device;
    let _ = writeln!(
        ui,
        "{} {} ({}×{} px, {} dpi) — {} {}",
        lang.pick("Device:", "Appareil :"),
        d.model,
        d.width_px,
        d.height_px,
        d.density_dpi,
        session.manifest.package.as_deref().unwrap_or("?"),
        session.manifest.build,
    );
    let _ = writeln!(
        ui,
        "{}",
        lang.pick(
            "Open a screen of the app on the phone, then press Enter to capture it.\nType a name before Enter to choose it. Type q then Enter when you are done.",
            "Ouvrez un écran de l'application sur le téléphone, puis appuyez sur Entrée pour le capturer.\nTapez un nom avant Entrée pour le choisir. Tapez q puis Entrée quand vous avez fini."
        )
    );
    let mut captured = 0usize;
    loop {
        let (suggested, _) = session.suggestion();
        let _ = write!(
            ui,
            "[{}] {} [{suggested}]: ",
            session.manifest.screens.len() + 1,
            lang.pick("screen name", "nom de l'écran")
        );
        let _ = ui.flush();
        let mut line = String::new();
        if input
            .read_line(&mut line)
            .map_err(io_err(Path::new("stdin")))?
            == 0
        {
            break;
        }
        let typed = line.trim();
        if matches!(typed, "q" | "quit" | "exit" | "done" | "fin") {
            break;
        }
        let name = if typed.is_empty() {
            suggested
        } else {
            sanitize_name(typed)
        };
        if !is_valid_screen_name(&name) {
            let _ = writeln!(ui, "  ✗ {}", ScanError::InvalidName(typed.to_string()));
            continue;
        }
        match session.capture(&name) {
            Ok(c) => {
                captured += 1;
                let _ = writeln!(
                    ui,
                    "  ✓ {} ({} {}{})",
                    c.name,
                    c.nodes,
                    lang.pick("elements", "éléments"),
                    c.activity.map(|a| format!(", {a}")).unwrap_or_default()
                );
            }
            Err(e) => {
                let _ = writeln!(ui, "  ✗ {e}");
            }
        }
    }
    if session.manifest.screens.is_empty() {
        return Err(ScanError::NothingCaptured);
    }
    Ok(captured)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(sanitize_name("  Home Screen "), "home_screen");
        assert_eq!(sanitize_name("Plant/Detail.v2"), "plant_detail_v2");
        assert_eq!(sanitize_name("€€"), "");
    }
}
