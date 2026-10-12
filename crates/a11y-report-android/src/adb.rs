//! Thin wrapper around `adb` to capture screens from a device or emulator.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum AdbError {
    #[error("could not run {path}: {source}")]
    Spawn {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("`adb {command}` failed: {message}")]
    Failed { command: String, message: String },
    #[error("no device connected: plug a phone with USB debugging enabled, or start an emulator")]
    NoDevice,
    #[error("several devices are connected ({0}): choose one with --serial")]
    MultipleDevices(String),
    #[error("device {0} is {1}: accept the debugging prompt on the phone")]
    NotReady(String, String),
    #[error("unexpected adb output for {what}: {output}")]
    Parse { what: &'static str, output: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    pub serial: String,
    pub state: String,
}

/// Device characteristics needed by the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub model: String,
    pub density_dpi: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub font_scale: f64,
}

#[derive(Debug, Clone)]
pub struct Adb {
    path: PathBuf,
    serial: Option<String>,
}

const REMOTE_DUMP: &str = "/sdcard/a11y-report-window.xml";

impl Adb {
    pub fn new(path: impl Into<PathBuf>, serial: Option<String>) -> Adb {
        Adb {
            path: path.into(),
            serial,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
    }

    fn raw(&self, with_serial: bool, args: &[&str]) -> Result<Vec<u8>, AdbError> {
        let mut cmd = Command::new(&self.path);
        if with_serial && let Some(s) = &self.serial {
            cmd.args(["-s", s]);
        }
        cmd.args(args);
        let out = cmd.output().map_err(|source| AdbError::Spawn {
            path: self.path.clone(),
            source,
        })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return Err(AdbError::Failed {
                command: args.join(" "),
                message: if stderr.is_empty() { stdout } else { stderr },
            });
        }
        Ok(out.stdout)
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>, AdbError> {
        self.raw(true, args)
    }

    pub fn shell(&self, command: &str) -> Result<String, AdbError> {
        let out = self.run(&["shell", command])?;
        Ok(String::from_utf8_lossy(&out).replace("\r\n", "\n"))
    }

    pub fn devices(&self) -> Result<Vec<DeviceEntry>, AdbError> {
        let out = self.raw(false, &["devices"])?;
        Ok(parse_devices(&String::from_utf8_lossy(&out)))
    }

    /// Picks the device: the `--serial` one, or the only one connected.
    pub fn select_device(&mut self) -> Result<String, AdbError> {
        let devices = self.devices()?;
        let chosen = match &self.serial {
            Some(s) => devices
                .into_iter()
                .find(|d| &d.serial == s)
                .ok_or(AdbError::NoDevice)?,
            None => {
                let mut ready: Vec<_> = devices.into_iter().collect();
                match ready.len() {
                    0 => return Err(AdbError::NoDevice),
                    1 => ready.remove(0),
                    _ => {
                        let list: Vec<_> = ready.iter().map(|d| d.serial.clone()).collect();
                        return Err(AdbError::MultipleDevices(list.join(", ")));
                    }
                }
            }
        };
        if chosen.state != "device" {
            return Err(AdbError::NotReady(chosen.serial, chosen.state));
        }
        self.serial = Some(chosen.serial.clone());
        Ok(chosen.serial)
    }

    pub fn device_info(&self) -> Result<DeviceInfo, AdbError> {
        let model = self.shell("getprop ro.product.model")?.trim().to_string();
        let size_out = self.shell("wm size")?;
        let (width_px, height_px) = parse_wm_size(&size_out).ok_or(AdbError::Parse {
            what: "wm size",
            output: size_out.clone(),
        })?;
        let density_out = self.shell("wm density")?;
        let density_dpi = parse_wm_density(&density_out).ok_or(AdbError::Parse {
            what: "wm density",
            output: density_out.clone(),
        })?;
        let font_scale = self
            .shell("settings get system font_scale")
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(1.0);
        Ok(DeviceInfo {
            model: if model.is_empty() {
                "unknown".into()
            } else {
                model
            },
            density_dpi,
            width_px,
            height_px,
            font_scale,
        })
    }

    /// Dumps the current accessibility hierarchy. Retries when the UI is not idle.
    pub fn dump_hierarchy(&self) -> Result<String, AdbError> {
        let mut last = String::new();
        for _ in 0..3 {
            let out = self.shell(&format!("uiautomator dump {REMOTE_DUMP}"))?;
            if out.contains("dumped to") {
                let xml = self.run(&["exec-out", "cat", REMOTE_DUMP])?;
                let _ = self.shell(&format!("rm -f {REMOTE_DUMP}"));
                return Ok(String::from_utf8_lossy(&xml).into_owned());
            }
            last = out;
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        Err(AdbError::Failed {
            command: "shell uiautomator dump".into(),
            message: format!(
                "{} (stop animations or videos on screen and retry)",
                last.trim()
            ),
        })
    }

    pub fn screenshot_png(&self) -> Result<Vec<u8>, AdbError> {
        self.run(&["exec-out", "screencap", "-p"])
    }

    /// `package/.Activity` currently resumed, when it can be determined.
    pub fn foreground_activity(&self) -> Option<String> {
        let out = self.shell("dumpsys activity activities").ok()?;
        parse_resumed_activity(&out)
    }

    /// `versionName (versionCode)` of an installed package.
    pub fn package_version(&self, package: &str) -> Option<String> {
        let out = self.shell(&format!("dumpsys package {package}")).ok()?;
        parse_package_version(&out)
    }

    pub fn launch(&self, package: &str) -> Result<(), AdbError> {
        self.shell(&format!(
            "monkey -p {package} -c android.intent.category.LAUNCHER 1"
        ))
        .map(|_| ())
    }
}

pub fn parse_devices(out: &str) -> Vec<DeviceEntry> {
    out.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let serial = parts.next()?;
            let state = parts.next()?;
            Some(DeviceEntry {
                serial: serial.to_string(),
                state: state.to_string(),
            })
        })
        .collect()
}

fn last_value_after<'a>(out: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    // "Override …" wins over "Physical …" when present.
    let mut found = None;
    for prefix in prefixes {
        if let Some(v) = out
            .lines()
            .find_map(|l| l.trim().strip_prefix(prefix).map(str::trim))
        {
            found = Some(v);
        }
    }
    found
}

pub fn parse_wm_size(out: &str) -> Option<(u32, u32)> {
    let v = last_value_after(out, &["Physical size:", "Override size:"])?;
    let (w, h) = v.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

pub fn parse_wm_density(out: &str) -> Option<u32> {
    last_value_after(out, &["Physical density:", "Override density:"])?
        .parse()
        .ok()
}

pub fn parse_resumed_activity(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let l = l.trim();
        if !(l.starts_with("mResumedActivity")
            || l.starts_with("topResumedActivity")
            || l.starts_with("ResumedActivity"))
        {
            return None;
        }
        l.split_whitespace()
            .find(|t| t.contains('/'))
            .map(|t| t.trim_end_matches('}').to_string())
    })
}

pub fn parse_package_version(out: &str) -> Option<String> {
    let name = out
        .lines()
        .find_map(|l| l.trim().strip_prefix("versionName="))?
        .trim()
        .to_string();
    let code = out.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("versionCode=")?;
        rest.split_whitespace().next().map(str::to_string)
    });
    Some(match code {
        Some(c) => format!("{name} ({c})"),
        None => name,
    })
}

/// Turns `com.example/.ui.PlantDetailActivity` into a screen name suggestion: `plant_detail`.
pub fn suggest_screen_name(activity: &str) -> String {
    let class = activity.rsplit(['/', '.']).next().unwrap_or(activity);
    let class = class
        .strip_suffix("Activity")
        .or_else(|| class.strip_suffix("Fragment"))
        .unwrap_or(class);
    let mut out = String::new();
    for (i, c) in class.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "screen".into()
    } else if crate::manifest::is_reserved_screen_name(&out) {
        format!("{out}_screen")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices() {
        let out = "* daemon started successfully\nList of devices attached\nemulator-5554\tdevice\nR58M\tunauthorized\n\n";
        assert_eq!(
            parse_devices(out),
            vec![
                DeviceEntry {
                    serial: "emulator-5554".into(),
                    state: "device".into()
                },
                DeviceEntry {
                    serial: "R58M".into(),
                    state: "unauthorized".into()
                },
            ]
        );
    }

    #[test]
    fn wm_outputs() {
        assert_eq!(
            parse_wm_size("Physical size: 1080x2400\n"),
            Some((1080, 2400))
        );
        assert_eq!(
            parse_wm_size("Physical size: 1080x2400\nOverride size: 720x1600\n"),
            Some((720, 1600))
        );
        assert_eq!(parse_wm_density("Physical density: 420\n"), Some(420));
        assert_eq!(
            parse_wm_density("Physical density: 420\nOverride density: 480"),
            Some(480)
        );
        assert_eq!(parse_wm_size("garbage"), None);
    }

    #[test]
    fn activity_and_version() {
        let out = "  mResumedActivity: ActivityRecord{e1c u0 com.example.pocketgarden/.ui.PlantDetailActivity t42}\n";
        assert_eq!(
            parse_resumed_activity(out).as_deref(),
            Some("com.example.pocketgarden/.ui.PlantDetailActivity")
        );
        let pkg = "    versionCode=240 minSdk=24 targetSdk=35\n    versionName=2.4.0\n";
        assert_eq!(parse_package_version(pkg).as_deref(), Some("2.4.0 (240)"));
    }

    #[test]
    fn screen_name_suggestions() {
        assert_eq!(
            suggest_screen_name("com.x/.ui.PlantDetailActivity"),
            "plant_detail"
        );
        assert_eq!(suggest_screen_name("com.x/com.x.MainActivity"), "main");
        assert_eq!(suggest_screen_name("Activity"), "screen");
    }
}
