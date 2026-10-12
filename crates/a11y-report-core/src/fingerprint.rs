//! Stable finding ids that survive small layout shifts and changing numbers in texts.

use crate::a11y::accessible_name;
use crate::finding::Finding;
use crate::model::{NodeId, Screen};
use std::collections::BTreeMap;

const NAME_KEY_MAX: usize = 40;

/// Normalized name used in element keys: lowercase, every run of digits replaced by a
/// single `#` (so "9 items" and "10 items" match), 40 chars max.
pub fn key_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_digit() {
            if !out.ends_with('#') {
                out.push('#');
            }
        } else {
            out.push(c);
        }
    }
    out.chars().take(NAME_KEY_MAX).collect()
}

/// Element key, never based on bounds:
/// - the short resource id when the node has one;
/// - else the nearest ancestor's short resource id followed by the path of
///   `SimpleClass[index_in_parent]` segments down to the node;
/// - in both cases suffixed with the normalized accessible name.
pub fn element_key(screen: &Screen, id: NodeId) -> String {
    let Some(node) = screen.node(id) else {
        return String::new();
    };
    let base = match node.short_resource_id() {
        Some(rid) => rid.to_string(),
        None => {
            let mut segments = vec![format!("{}[{}]", node.simple_class(), node.index_in_parent)];
            let mut anchor = String::new();
            for ancestor in screen.ancestors(id) {
                if let Some(rid) = ancestor.short_resource_id() {
                    anchor = rid.to_string();
                    break;
                }
                segments.push(format!(
                    "{}[{}]",
                    ancestor.simple_class(),
                    ancestor.index_in_parent
                ));
            }
            segments.reverse();
            if anchor.is_empty() {
                segments.join("/")
            } else {
                format!("{anchor}/{}", segments.join("/"))
            }
        }
    };
    let name = accessible_name(screen, id)
        .map(|n| key_name(&n))
        .unwrap_or_default();
    format!("{base}|{name}")
}

/// Short label shown in ids: the short resource id or the simple class.
pub fn element_label(screen: &Screen, id: NodeId) -> String {
    screen
        .node(id)
        .map(|n| {
            n.short_resource_id()
                .map(str::to_string)
                .unwrap_or_else(|| n.simple_class().to_string())
        })
        .unwrap_or_default()
}

/// `{rule}:{screen}:{label}#{8 hex chars of blake3(rule \0 screen \0 key)}`.
pub fn fingerprint(rule: &str, screen: &str, label: &str, key: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(rule.as_bytes());
    hasher.update(b"\0");
    hasher.update(screen.as_bytes());
    hasher.update(b"\0");
    hasher.update(key.as_bytes());
    let hex = hasher.finalize().to_hex();
    format!("{rule}:{screen}:{label}#{}", &hex.as_str()[..8])
}

/// Assigns ids to the findings of one screen. Findings must be sorted by node id;
/// collisions get `~2`, `~3`… in that order.
pub fn assign_ids(screen: &Screen, findings: &mut [Finding]) {
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for f in findings.iter_mut() {
        let base = fingerprint(
            &f.rule,
            &screen.name,
            &element_label(screen, f.node),
            &element_key(screen, f.node),
        );
        let count = seen.entry(base.clone()).or_insert(0);
        *count += 1;
        f.id = if *count == 1 {
            base
        } else {
            format!("{base}~{count}")
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::model::{Node, NodeFlags};

    fn node(id: u32, parent: Option<u32>, idx: u16, class: &str, rid: Option<&str>) -> Node {
        Node {
            id: NodeId(id),
            parent: parent.map(NodeId),
            children: Vec::new(),
            depth: 0,
            index_in_parent: idx,
            class: class.into(),
            package: None,
            resource_id: rid.map(str::to_string),
            text: None,
            content_desc: None,
            bounds: Rect::new(0, 0, 10, 10),
            clipped_by_screen: false,
            flags: NodeFlags::default(),
        }
    }

    fn sample() -> Screen {
        let mut root = node(
            0,
            None,
            0,
            "android.widget.FrameLayout",
            Some("com.app:id/card_list"),
        );
        root.children = vec![NodeId(1)];
        let mut row = node(1, Some(0), 2, "android.widget.LinearLayout", None);
        row.children = vec![NodeId(2)];
        let mut button = node(2, Some(1), 0, "android.widget.Button", None);
        button.text = Some("3 participants".into());
        Screen {
            name: "feed".into(),
            width_px: 100,
            height_px: 100,
            nodes: vec![root, row, button],
        }
    }

    #[test]
    fn key_uses_anchor_path_and_name() {
        let s = sample();
        assert_eq!(
            element_key(&s, NodeId(2)),
            "card_list/LinearLayout[2]/Button[0]|# participants"
        );
        assert_eq!(element_key(&s, NodeId(0)), "card_list|# participants");
        assert_eq!(element_label(&s, NodeId(2)), "Button");
    }

    #[test]
    fn numbers_and_bounds_do_not_change_the_key() {
        let a = sample();
        let mut b = sample();
        b.nodes[2].text = Some("4 participants".into());
        for n in &mut b.nodes {
            n.bounds = n.bounds.offset(10, 10);
        }
        assert_eq!(element_key(&a, NodeId(2)), element_key(&b, NodeId(2)));
    }

    #[test]
    fn fingerprint_format() {
        let id = fingerprint("contrast-text", "feed", "btn_join", "btn_join|join");
        assert!(id.starts_with("contrast-text:feed:btn_join#"));
        assert_eq!(id.len(), "contrast-text:feed:btn_join#".len() + 8);
        assert_eq!(
            id,
            fingerprint("contrast-text", "feed", "btn_join", "btn_join|join")
        );
    }

    #[test]
    fn key_name_truncates() {
        assert_eq!(key_name(&"A".repeat(50)).len(), 40);
        assert_eq!(key_name("Room 12"), "room #");
        assert_eq!(key_name("9 items"), key_name("10 items"));
    }
}
