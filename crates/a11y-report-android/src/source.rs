//! Best-effort mapping from a captured element to the project source that declares it.
//!
//! The index is built once per analysis by scanning layouts, string resources and
//! Kotlin/Java files, so lookups do not rescan the project for every finding.

use a11y_report_core::finding::SourceHint;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const SKIP_DIRS: [&str; 8] = [
    "build",
    "node_modules",
    "a11y-report",
    "out",
    "intermediates",
    "generated",
    "target",
    "Pods",
];
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FILES: usize = 50_000;
const MAX_HINTS: usize = 5;

/// How a hint was found, from most to least reliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    LayoutId,
    ComposeTestTag,
    ViewBinding,
    CodeIdReference,
    LayoutString,
    StringResource,
    LiteralText,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::LayoutId => "layout_id",
            Kind::ComposeTestTag => "compose_test_tag",
            Kind::ViewBinding => "view_binding",
            Kind::CodeIdReference => "code_id_reference",
            Kind::LayoutString => "layout_string",
            Kind::StringResource => "string_resource",
            Kind::LiteralText => "literal_text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Location {
    file: u32,
    line: u32,
}

#[derive(Debug, Default)]
pub struct SourceIndex {
    files: Vec<String>,
    /// Trimmed line texts, for snippets.
    lines: BTreeMap<Location, String>,
    occurrences: BTreeMap<(Kind, String), Vec<Location>>,
    /// Normalized string value -> resource names.
    strings_by_value: BTreeMap<String, Vec<String>>,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Identifiers following each occurrence of `prefix` in `line`.
fn idents_after<'a>(line: &'a str, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    line.match_indices(prefix).filter_map(move |(i, _)| {
        let rest = &line[i + prefix.len()..];
        let end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
        (end > 0).then(|| &rest[..end])
    })
}

/// Double-quoted literals of a line (naive, handles `\"`).
fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut lit = String::new();
        let mut escaped = false;
        let mut closed = false;
        for c in chars.by_ref() {
            if escaped {
                lit.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                closed = true;
                break;
            } else {
                lit.push(c);
            }
        }
        if closed && !lit.trim().is_empty() && lit.chars().count() <= 120 {
            out.push(lit);
        }
    }
    out
}

fn normalize_text(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `btn_join` -> `btnJoin` (ViewBinding field name).
fn snake_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in s.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![(root.to_path_buf(), false)];
    while let Some((dir, in_sources)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                // Inside java/ or kotlin/ source roots, `build` or `out` are package names.
                let skipped = !in_sources && SKIP_DIRS.contains(&name.as_str());
                if !name.starts_with('.') && !skipped {
                    let sources = in_sources || name == "java" || name == "kotlin";
                    stack.push((path, sources));
                }
            } else if ft.is_file() {
                let wanted = name.ends_with(".kt")
                    || name.ends_with(".java")
                    || (name.ends_with(".xml")
                        && path.components().any(|c| c.as_os_str() == "res"));
                let small = e
                    .metadata()
                    .map(|m| m.len() <= MAX_FILE_BYTES)
                    .unwrap_or(false);
                if wanted && small {
                    out.push(path);
                    if out.len() >= MAX_FILES {
                        return;
                    }
                }
            }
        }
    }
}

impl SourceIndex {
    /// Scans `root` (an Android project). Unreadable files are skipped silently.
    pub fn build(root: &Path) -> SourceIndex {
        let mut paths = Vec::new();
        collect_files(root, &mut paths);
        paths.sort();
        let mut index = SourceIndex::default();
        for path in paths {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            index.add_file(&rel, &text);
        }
        for names in index.strings_by_value.values_mut() {
            names.sort();
            names.dedup();
        }
        index
    }

    /// Indexes one file given its project-relative path (public for tests).
    pub fn add_file(&mut self, rel: &str, text: &str) {
        let file = u32::try_from(self.files.len()).unwrap_or(u32::MAX);
        self.files.push(rel.to_string());
        let is_xml = rel.ends_with(".xml");
        let is_strings = is_xml && rel.contains("/values") && text.contains("<string");
        if is_strings
            && !crate::uiautomator::xml_too_deep(text, crate::uiautomator::MAX_XML_DEPTH)
            && let Ok(doc) = roxmltree::Document::parse(text)
        {
            for n in doc.descendants().filter(|n| n.has_tag_name("string")) {
                if let (Some(name), Some(value)) = (n.attribute("name"), n.text()) {
                    let value = value.replace("\\'", "'").replace("\\\"", "\"");
                    self.strings_by_value
                        .entry(normalize_text(&value))
                        .or_default()
                        .push(name.to_string());
                }
            }
        }
        for (i, line) in text.lines().enumerate() {
            let loc = Location {
                file,
                line: u32::try_from(i + 1).unwrap_or(u32::MAX),
            };
            let mut hits: Vec<(Kind, String)> = Vec::new();
            if is_xml {
                for id in idents_after(line, "@+id/").chain(idents_after(line, "@id/")) {
                    if line.contains("android:id") || line.contains(":id=") {
                        hits.push((Kind::LayoutId, id.to_string()));
                    }
                }
                for name in idents_after(line, "@string/") {
                    hits.push((Kind::LayoutString, name.to_string()));
                }
                if !is_strings {
                    for lit in string_literals(line) {
                        if !lit.starts_with('@') && !lit.starts_with('?') {
                            hits.push((Kind::LiteralText, normalize_text(&lit)));
                        }
                    }
                }
            } else {
                for id in idents_after(line, "R.id.") {
                    hits.push((Kind::CodeIdReference, id.to_string()));
                }
                for name in idents_after(line, "R.string.") {
                    hits.push((Kind::StringResource, name.to_string()));
                }
                if line.contains("testTag") {
                    for lit in string_literals(line) {
                        hits.push((Kind::ComposeTestTag, lit));
                    }
                }
                for field in idents_after(line, ".") {
                    if field.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                        && field.chars().any(|c| c.is_ascii_uppercase())
                    {
                        hits.push((Kind::ViewBinding, field.to_string()));
                    }
                }
                for lit in string_literals(line) {
                    hits.push((Kind::LiteralText, normalize_text(&lit)));
                    // `const val TAG_JOIN = "btn_join"` used through a constant.
                    hits.push((Kind::ComposeTestTag, lit));
                }
            }
            if hits.is_empty() {
                continue;
            }
            self.lines
                .insert(loc, line.trim().chars().take(160).collect());
            for key in hits {
                self.occurrences.entry(key).or_default().push(loc);
            }
        }
    }

    fn push(&self, kind: Kind, key: &str, out: &mut Vec<(Kind, Location)>) {
        if let Some(locs) = self.occurrences.get(&(kind, key.to_string())) {
            out.extend(locs.iter().map(|l| (kind, *l)));
        }
    }

    /// Up to 5 likely source locations for an element, most reliable first.
    pub fn hints(
        &self,
        resource_id: Option<&str>,
        text: Option<&str>,
        content_desc: Option<&str>,
    ) -> Vec<SourceHint> {
        let mut found: Vec<(Kind, Location)> = Vec::new();
        if let Some(id) = resource_id.filter(|s| !s.is_empty()) {
            self.push(Kind::LayoutId, id, &mut found);
            self.push(Kind::ComposeTestTag, id, &mut found);
            let camel = snake_to_camel(id);
            if camel != id {
                self.push(Kind::ViewBinding, &camel, &mut found);
            }
            self.push(Kind::CodeIdReference, id, &mut found);
        }
        for label in [text, content_desc].into_iter().flatten() {
            let norm = normalize_text(label);
            if norm.is_empty() {
                continue;
            }
            if let Some(names) = self.strings_by_value.get(&norm) {
                for name in names {
                    self.push(Kind::LayoutString, name, &mut found);
                    self.push(Kind::StringResource, name, &mut found);
                }
            }
            self.push(Kind::LiteralText, &norm, &mut found);
        }
        // Most reliable kind first; one hint per file:line.
        found.sort();
        let mut seen = std::collections::BTreeSet::new();
        found.retain(|(_, loc)| seen.insert(*loc));
        found
            .into_iter()
            .take(MAX_HINTS)
            .map(|(kind, loc)| SourceHint {
                file: self
                    .files
                    .get(loc.file as usize)
                    .cloned()
                    .unwrap_or_default(),
                line: loc.line,
                matched_by: kind.label().to_string(),
                snippet: self.lines.get(&loc).cloned().unwrap_or_default(),
            })
            .collect()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> SourceIndex {
        let mut idx = SourceIndex::default();
        idx.add_file(
            "app/src/main/res/values/strings.xml",
            "<resources>\n  <string name=\"water_now\">Water now</string>\n</resources>",
        );
        idx.add_file(
            "app/src/main/res/layout/item_plant.xml",
            "<LinearLayout>\n  <Button\n    android:id=\"@+id/btn_water\"\n    android:text=\"@string/water_now\" />\n</LinearLayout>",
        );
        idx.add_file(
            "app/src/main/java/com/example/PlantAdapter.kt",
            "binding.btnWater.setOnClickListener { }\nval x = R.id.btn_water\nText(\"Hard coded\")\nModifier.testTag(\"search_box\")",
        );
        idx
    }

    #[test]
    fn finds_layout_and_code_by_id() {
        let hints = index().hints(Some("btn_water"), None, None);
        let kinds: Vec<_> = hints
            .iter()
            .map(|h| (h.matched_by.as_str(), h.line))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("layout_id", 3),
                ("view_binding", 1),
                ("code_id_reference", 2)
            ]
        );
        assert_eq!(hints[0].file, "app/src/main/res/layout/item_plant.xml");
        assert_eq!(hints[0].snippet, "android:id=\"@+id/btn_water\"");
    }

    #[test]
    fn finds_text_through_string_resources_and_literals() {
        let idx = index();
        let hints = idx.hints(None, Some("Water  now"), None);
        assert_eq!(hints[0].matched_by, "layout_string");
        assert_eq!(hints[0].line, 4);
        let hints = idx.hints(None, Some("Hard coded"), None);
        assert_eq!(hints[0].matched_by, "literal_text");
        let hints = idx.hints(Some("search_box"), None, None);
        assert_eq!(hints[0].matched_by, "compose_test_tag");
        assert!(idx.hints(Some("nothing"), Some("nope"), None).is_empty());
    }

    #[test]
    fn helpers() {
        assert_eq!(snake_to_camel("btn_join_now"), "btnJoinNow");
        assert_eq!(
            string_literals(r#"a("x", "y \"z\"")"#),
            vec!["x", "y \"z\""]
        );
        assert_eq!(
            idents_after("R.id.a + R.id.b_c", "R.id.").collect::<Vec<_>>(),
            vec!["a", "b_c"]
        );
    }
}
