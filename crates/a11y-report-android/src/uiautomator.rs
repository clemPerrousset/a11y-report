//! Parser of `uiautomator dump` XML into the flat [`Screen`] model.

use a11y_report_core::geometry::Rect;
use a11y_report_core::model::{Node, NodeFlags, NodeId, Screen};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("root element must be <hierarchy>, found <{0}>")]
    NotHierarchy(String),
    #[error("node #{index}: invalid bounds \"{value}\" (expected [x1,y1][x2,y2])")]
    InvalidBounds { index: usize, value: String },
    #[error("node #{index}: negative bounds \"{value}\"")]
    NegativeBounds { index: usize, value: String },
    #[error("too many nodes (more than {0})")]
    TooManyNodes(usize),
}

/// Screen size and optional app package used to drop other windows (status bar, keyboard).
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub width_px: u32,
    pub height_px: u32,
    pub package: Option<String>,
}

const MAX_NODES: usize = 200_000;

/// Parses `[x1,y1][x2,y2]`. Returns `None` on any syntax error.
pub fn parse_bounds(s: &str) -> Option<Rect> {
    let s = s.trim();
    let inner = s.strip_prefix('[')?.strip_suffix(']')?;
    let (a, b) = inner.split_once("][")?;
    let pair = |p: &str| -> Option<(i32, i32)> {
        let (x, y) = p.split_once(',')?;
        Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
    };
    let (x1, y1) = pair(a)?;
    let (x2, y2) = pair(b)?;
    Some(Rect::new(x1, y1, x2, y2))
}

fn attr_bool(n: &roxmltree::Node<'_, '_>, name: &str) -> bool {
    n.attribute(name) == Some("true")
}

fn attr_string(n: &roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    n.attribute(name)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn is_node(n: &roxmltree::Node<'_, '_>) -> bool {
    n.is_element() && n.tag_name().name() == "node"
}

/// Parses a dump. Nodes with an empty area once clamped to the screen are dropped with
/// their subtree; bounds exceeding the screen are clamped and flagged.
pub fn parse_hierarchy(name: &str, xml: &str, opts: &ParseOptions) -> Result<Screen, ParseError> {
    let doc = roxmltree::Document::parse(xml)?;
    let root = doc.root_element();
    if root.tag_name().name() != "hierarchy" {
        return Err(ParseError::NotHierarchy(root.tag_name().name().to_string()));
    }

    let mut nodes: Vec<Node> = Vec::new();
    // (xml node, parent, depth, index among siblings), processed depth-first in document order.
    let mut stack: Vec<(roxmltree::Node<'_, '_>, Option<NodeId>, u16, u16)> = Vec::new();
    let top: Vec<_> = root.children().filter(is_node).collect();
    for (i, n) in top.into_iter().enumerate().rev() {
        let keep = match (&opts.package, n.attribute("package")) {
            (Some(pkg), Some(p)) if !p.is_empty() => p == pkg,
            _ => true,
        };
        if keep {
            stack.push((n, None, 0, u16::try_from(i).unwrap_or(u16::MAX)));
        }
    }

    let mut seen = 0usize;
    while let Some((xml_node, parent, depth, index_in_parent)) = stack.pop() {
        seen += 1;
        if seen > MAX_NODES {
            return Err(ParseError::TooManyNodes(MAX_NODES));
        }
        let raw = xml_node.attribute("bounds").unwrap_or("[0,0][0,0]");
        let bounds = parse_bounds(raw).ok_or_else(|| ParseError::InvalidBounds {
            index: seen,
            value: raw.to_string(),
        })?;
        if bounds.x1 < 0 || bounds.y1 < 0 || bounds.x2 < 0 || bounds.y2 < 0 {
            return Err(ParseError::NegativeBounds {
                index: seen,
                value: raw.to_string(),
            });
        }
        let clamped = bounds.clamp_to(opts.width_px, opts.height_px);
        if clamped.is_empty() {
            continue;
        }
        let id = NodeId(u32::try_from(nodes.len()).unwrap_or(u32::MAX));
        nodes.push(Node {
            id,
            parent,
            children: Vec::new(),
            depth,
            index_in_parent,
            class: xml_node.attribute("class").unwrap_or_default().to_string(),
            package: attr_string(&xml_node, "package"),
            resource_id: attr_string(&xml_node, "resource-id"),
            text: attr_string(&xml_node, "text"),
            content_desc: attr_string(&xml_node, "content-desc"),
            bounds: clamped,
            clipped_by_screen: clamped != bounds,
            flags: NodeFlags {
                clickable: attr_bool(&xml_node, "clickable"),
                long_clickable: attr_bool(&xml_node, "long-clickable"),
                checkable: attr_bool(&xml_node, "checkable"),
                focusable: attr_bool(&xml_node, "focusable"),
                enabled: attr_bool(&xml_node, "enabled"),
                scrollable: attr_bool(&xml_node, "scrollable"),
                password: attr_bool(&xml_node, "password"),
                selected: attr_bool(&xml_node, "selected"),
            },
        });
        if let Some(p) = parent {
            nodes[p.index()].children.push(id);
        }
        let children: Vec<_> = xml_node.children().filter(is_node).collect();
        for (i, c) in children.into_iter().enumerate().rev() {
            stack.push((
                c,
                Some(id),
                depth.saturating_add(1),
                u16::try_from(i).unwrap_or(u16::MAX),
            ));
        }
    }

    Ok(Screen {
        name: name.to_string(),
        width_px: opts.width_px,
        height_px: opts.height_px,
        nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<hierarchy rotation="0">
  <node index="0" text="" resource-id="" class="android.widget.FrameLayout" package="com.example.app" content-desc="" clickable="false" enabled="true" focusable="false" bounds="[0,0][1080,2400]">
    <node index="0" text="Join &amp; walk 🌱" resource-id="com.example.app:id/btn_join" class="android.widget.Button" package="com.example.app" content-desc="" checkable="false" clickable="true" enabled="true" focusable="true" long-clickable="false" password="false" scrollable="false" selected="false" unknown-attr="x" bounds="[48,1210][312,1288]" />
    <node index="1" text="" class="android.view.View" package="com.example.app" bounds="[0,0][0,0]">
      <node text="hidden" class="android.widget.TextView" bounds="[0,0][0,0]" />
    </node>
    <node index="2" text="" class="android.widget.ImageView" package="com.example.app" clickable="true" enabled="true" bounds="[1000,100][1200,200]" />
  </node>
  <node index="1" class="android.widget.FrameLayout" package="com.android.systemui" bounds="[0,0][1080,80]" />
</hierarchy>"#;

    fn opts(package: Option<&str>) -> ParseOptions {
        ParseOptions {
            width_px: 1080,
            height_px: 2400,
            package: package.map(str::to_string),
        }
    }

    #[test]
    fn bounds_parsing() {
        assert_eq!(
            parse_bounds("[48,1210][312,1288]"),
            Some(Rect::new(48, 1210, 312, 1288))
        );
        assert_eq!(
            parse_bounds("[-5,0][10,10]"),
            Some(Rect::new(-5, 0, 10, 10))
        );
        assert_eq!(parse_bounds("48,1210,312,1288"), None);
        assert_eq!(parse_bounds("[a,b][c,d]"), None);
        assert_eq!(parse_bounds("[1,2][3]"), None);
    }

    #[test]
    fn parses_tree_and_decodes_entities() {
        let s = parse_hierarchy("feed", DUMP, &opts(Some("com.example.app"))).expect("parse");
        assert_eq!(
            s.nodes.len(),
            3,
            "zero-area subtree and other packages dropped"
        );
        let button = &s.nodes[1];
        assert_eq!(button.text.as_deref(), Some("Join & walk 🌱"));
        assert_eq!(button.short_resource_id(), Some("btn_join"));
        assert!(button.flags.clickable && button.flags.focusable && button.flags.enabled);
        assert_eq!(button.parent, Some(NodeId(0)));
        assert_eq!(button.depth, 1);
        let image = &s.nodes[2];
        assert_eq!(image.index_in_parent, 2);
        assert!(image.clipped_by_screen);
        assert_eq!(image.bounds, Rect::new(1000, 100, 1080, 200));
        assert_eq!(s.nodes[0].children, vec![NodeId(1), NodeId(2)]);
    }

    #[test]
    fn keeps_other_windows_without_package_filter() {
        let s = parse_hierarchy("feed", DUMP, &opts(None)).expect("parse");
        assert_eq!(s.nodes.len(), 4);
    }

    #[test]
    fn errors_are_typed() {
        assert!(matches!(
            parse_hierarchy("x", "<hierarchy><node", &opts(None)),
            Err(ParseError::Xml(_))
        ));
        assert!(matches!(
            parse_hierarchy("x", "<root/>", &opts(None)),
            Err(ParseError::NotHierarchy(_))
        ));
        assert!(matches!(
            parse_hierarchy(
                "x",
                r#"<hierarchy><node bounds="[-1,0][10,10]"/></hierarchy>"#,
                &opts(None)
            ),
            Err(ParseError::NegativeBounds { .. })
        ));
        assert!(matches!(
            parse_hierarchy(
                "x",
                r#"<hierarchy><node bounds="oops"/></hierarchy>"#,
                &opts(None)
            ),
            Err(ParseError::InvalidBounds { .. })
        ));
    }
}
