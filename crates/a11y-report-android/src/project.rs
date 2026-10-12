//! Detection of the Android (Gradle) project the tool is launched in, and of `adb`.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AndroidProject {
    pub root: PathBuf,
    /// Module applying `com.android.application`, e.g. `<root>/app`.
    pub app_module: Option<PathBuf>,
    pub application_id: Option<String>,
    pub version_name: Option<String>,
    /// `app_name` string resource, when found.
    pub app_name: Option<String>,
    /// `sdk.dir` from `local.properties`.
    pub sdk_dir: Option<PathBuf>,
}

const GRADLE_FILES: [&str; 2] = ["build.gradle.kts", "build.gradle"];
const SETTINGS_FILES: [&str; 2] = ["settings.gradle.kts", "settings.gradle"];

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn gradle_file(dir: &Path) -> Option<(PathBuf, String)> {
    GRADLE_FILES
        .iter()
        .map(|f| dir.join(f))
        .find_map(|p| read(&p).map(|s| (p, s)))
}

fn is_application_module(gradle: &str) -> bool {
    gradle.contains("com.android.application") || gradle.contains("android.application")
}

/// Extracts the first quoted value assigned to `key` (`key = "v"`, `key "v"`, `key("v")`).
pub fn gradle_string_value(gradle: &str, key: &str) -> Option<String> {
    for line in gradle.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let Some(rest) = line.match_indices(key).find_map(|(i, _)| {
            let before_ok = line[..i].chars().next_back().is_none_or(|c| !is_ident(c));
            let rest = &line[i + key.len()..];
            let after_ok = rest.chars().next().is_none_or(|c| !is_ident(c));
            (before_ok && after_ok).then_some(rest)
        }) else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest
            .strip_prefix('=')
            .or_else(|| rest.strip_prefix('('))
            .unwrap_or(rest)
            .trim_start();
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let value: String = rest[1..].chars().take_while(|c| *c != quote).collect();
        if !value.is_empty() && !value.contains('$') {
            return Some(value);
        }
    }
    None
}

/// Reads `sdk.dir` from `local.properties` (handles `C\:\\Users\\…` escapes).
pub fn parse_local_properties_sdk_dir(text: &str) -> Option<PathBuf> {
    text.lines().find_map(|l| {
        let v = l
            .trim()
            .strip_prefix("sdk.dir")?
            .trim_start()
            .strip_prefix('=')?;
        let unescaped = v.trim().replace("\\:", ":").replace("\\\\", "\\");
        (!unescaped.is_empty()).then(|| PathBuf::from(unescaped))
    })
}

fn find_app_name(module: &Path) -> Option<String> {
    let strings = read(&module.join("src/main/res/values/strings.xml"))?;
    let doc = roxmltree::Document::parse(&strings).ok()?;
    doc.descendants()
        .find(|n| n.has_tag_name("string") && n.attribute("name") == Some("app_name"))
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Returns the project when `root` looks like a Gradle Android project.
pub fn detect(root: &Path) -> Option<AndroidProject> {
    let has_settings = SETTINGS_FILES.iter().any(|f| root.join(f).is_file());
    if !has_settings && gradle_file(root).is_none() {
        return None;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if gradle_file(root).is_some_and(|(_, g)| is_application_module(&g)) {
        candidates.push(root.to_path_buf());
    }
    if let Ok(entries) = fs::read_dir(root) {
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for d in dirs {
            if gradle_file(&d).is_some_and(|(_, g)| is_application_module(&g)) {
                candidates.push(d);
            }
        }
    }
    // Prefer the conventional `app` module.
    candidates.sort_by_key(|p| p.file_name().is_none_or(|n| n != "app"));
    let app_module = candidates.into_iter().next();
    let gradle = app_module.as_deref().and_then(gradle_file).map(|(_, g)| g);
    let application_id = gradle.as_deref().and_then(|g| {
        gradle_string_value(g, "applicationId").or_else(|| gradle_string_value(g, "namespace"))
    });
    let version_name = gradle
        .as_deref()
        .and_then(|g| gradle_string_value(g, "versionName"));
    Some(AndroidProject {
        root: root.to_path_buf(),
        app_name: app_module.as_deref().and_then(find_app_name),
        app_module,
        application_id,
        version_name,
        sdk_dir: read(&root.join("local.properties"))
            .as_deref()
            .and_then(parse_local_properties_sdk_dir),
    })
}

fn adb_name() -> &'static str {
    if cfg!(windows) { "adb.exe" } else { "adb" }
}

/// Finds `adb`: `PATH`, then `ANDROID_HOME`, `ANDROID_SDK_ROOT`, the project's
/// `local.properties`, then the default Android Studio SDK locations.
pub fn find_adb(project: Option<&AndroidProject>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(adb_name());
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let mut sdks: Vec<PathBuf> = Vec::new();
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(v) = std::env::var_os(var) {
            sdks.push(PathBuf::from(v));
        }
    }
    if let Some(dir) = project.and_then(|p| p.sdk_dir.clone()) {
        sdks.push(dir);
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        sdks.push(PathBuf::from(local).join("Android").join("Sdk"));
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        sdks.push(home.join("Library/Android/sdk"));
        sdks.push(home.join("Android/Sdk"));
    }
    sdks.into_iter()
        .map(|sdk| sdk.join("platform-tools").join(adb_name()))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradle_values() {
        let kts = r#"
            android {
                namespace = "com.example.pocketgarden"
                defaultConfig {
                    // applicationId = "commented.out"
                    applicationId = "com.example.pocketgarden.app"
                    versionName = "2.4.0"
                }
            }"#;
        assert_eq!(
            gradle_string_value(kts, "applicationId").as_deref(),
            Some("com.example.pocketgarden.app")
        );
        assert_eq!(
            gradle_string_value(kts, "versionName").as_deref(),
            Some("2.4.0")
        );
        let groovy =
            "defaultConfig {\n applicationId 'com.example.groovy'\n versionName \"1.${x}\"\n}";
        assert_eq!(
            gradle_string_value(groovy, "applicationId").as_deref(),
            Some("com.example.groovy")
        );
        assert_eq!(gradle_string_value(groovy, "versionName"), None);
    }

    #[test]
    fn local_properties() {
        let text =
            "## comment\nsdk.dir=C\\:\\\\Users\\\\me\\\\AppData\\\\Local\\\\Android\\\\Sdk\n";
        assert_eq!(
            parse_local_properties_sdk_dir(text),
            Some(PathBuf::from("C:\\Users\\me\\AppData\\Local\\Android\\Sdk"))
        );
        assert_eq!(
            parse_local_properties_sdk_dir("sdk.dir=/opt/android"),
            Some(PathBuf::from("/opt/android"))
        );
    }

    #[test]
    fn detects_project() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(root.join("settings.gradle.kts"), "include(\":app\")").expect("write");
        fs::create_dir_all(root.join("app/src/main/res/values")).expect("mkdir");
        fs::write(
            root.join("app/build.gradle.kts"),
            "plugins { id(\"com.android.application\") }\nandroid { defaultConfig { applicationId = \"com.example.pocketgarden\" } }",
        )
        .expect("write");
        fs::write(
            root.join("app/src/main/res/values/strings.xml"),
            "<resources><string name=\"app_name\">Pocket Garden</string></resources>",
        )
        .expect("write");
        let p = detect(root).expect("project");
        assert_eq!(
            p.application_id.as_deref(),
            Some("com.example.pocketgarden")
        );
        assert_eq!(p.app_name.as_deref(), Some("Pocket Garden"));
        assert!(detect(&root.join("app/src")).is_none());
    }
}
