//! Accessible name and interactivity, shared by every rule.

use crate::model::{Node, NodeId, Screen};

/// Trims and collapses runs of whitespace to a single space.
pub fn normalize_name(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn own_label(node: &Node) -> Option<String> {
    [&node.content_desc, &node.text]
        .into_iter()
        .flatten()
        .map(|s| normalize_name(s))
        .find(|s| !s.is_empty())
}

fn collect_descendant_names(screen: &Screen, id: NodeId, out: &mut Vec<String>) {
    let Some(node) = screen.node(id) else { return };
    for &child_id in &node.children {
        let Some(child) = screen.node(child_id) else {
            continue;
        };
        // A focusable descendant is its own TalkBack stop: it is not merged into the parent.
        if child.flags.focusable {
            continue;
        }
        match own_label(child) {
            Some(label) => out.push(label),
            None => collect_descendant_names(screen, child_id, out),
        }
    }
}

/// Name announced by TalkBack:
/// 1. non-empty `content-desc`, else
/// 2. non-empty `text`, else
/// 3. names of non-focusable descendants joined with spaces.
///
/// The result is normalized; `None` when nothing is announced.
pub fn accessible_name(screen: &Screen, id: NodeId) -> Option<String> {
    let node = screen.node(id)?;
    if let Some(label) = own_label(node) {
        return Some(label);
    }
    let mut parts = Vec::new();
    collect_descendant_names(screen, id, &mut parts);
    let joined = normalize_name(&parts.join(" "));
    (!joined.is_empty()).then_some(joined)
}

/// Enabled, and clickable / long-clickable / checkable, or a text field.
pub fn is_interactive(node: &Node) -> bool {
    let f = &node.flags;
    f.enabled
        && (f.clickable || f.long_clickable || f.checkable || node.class.ends_with("EditText"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::model::NodeFlags;

    fn node(id: u32, parent: Option<u32>, children: &[u32]) -> Node {
        Node {
            id: NodeId(id),
            parent: parent.map(NodeId),
            children: children.iter().copied().map(NodeId).collect(),
            depth: 0,
            index_in_parent: 0,
            class: "android.view.View".into(),
            package: None,
            resource_id: None,
            text: None,
            content_desc: None,
            bounds: Rect::new(0, 0, 10, 10),
            clipped_by_screen: false,
            flags: NodeFlags {
                enabled: true,
                ..NodeFlags::default()
            },
        }
    }

    fn screen(nodes: Vec<Node>) -> Screen {
        Screen {
            name: "s".into(),
            width_px: 100,
            height_px: 100,
            nodes,
        }
    }

    #[test]
    fn content_desc_wins_then_text() {
        let mut n = node(0, None, &[]);
        n.text = Some("Text".into());
        n.content_desc = Some("  Desc  ".into());
        assert_eq!(
            accessible_name(&screen(vec![n.clone()]), NodeId(0)).as_deref(),
            Some("Desc")
        );
        n.content_desc = Some("   ".into());
        assert_eq!(
            accessible_name(&screen(vec![n]), NodeId(0)).as_deref(),
            Some("Text")
        );
    }

    #[test]
    fn descendants_are_merged_unless_focusable() {
        let parent = node(0, None, &[1, 2, 3]);
        let mut a = node(1, Some(0), &[]);
        a.text = Some("Join".into());
        let mut b = node(2, Some(0), &[]);
        b.text = Some("the   walk".into());
        let mut c = node(3, Some(0), &[]);
        c.text = Some("Other".into());
        c.flags.focusable = true;
        let s = screen(vec![parent, a, b, c]);
        assert_eq!(
            accessible_name(&s, NodeId(0)).as_deref(),
            Some("Join the walk")
        );
    }

    #[test]
    fn nested_unnamed_containers_are_traversed() {
        let parent = node(0, None, &[1]);
        let wrapper = node(1, Some(0), &[2]);
        let mut leaf = node(2, Some(1), &[]);
        leaf.content_desc = Some("Close".into());
        let s = screen(vec![parent, wrapper, leaf]);
        assert_eq!(accessible_name(&s, NodeId(0)).as_deref(), Some("Close"));
        let empty = screen(vec![node(0, None, &[])]);
        assert_eq!(accessible_name(&empty, NodeId(0)), None);
    }

    #[test]
    fn interactivity() {
        let mut n = node(0, None, &[]);
        assert!(!is_interactive(&n));
        n.flags.clickable = true;
        assert!(is_interactive(&n));
        n.flags.enabled = false;
        assert!(!is_interactive(&n));
        let mut e = node(0, None, &[]);
        e.class = "android.widget.EditText".into();
        assert!(is_interactive(&e));
        e.flags.checkable = true;
        assert!(is_interactive(&e));
    }

    #[test]
    fn normalization() {
        assert_eq!(normalize_name("  a \n\t b  "), "a b");
    }
}
