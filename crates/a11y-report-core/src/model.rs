//! Flat accessibility tree of one captured screen.

use crate::geometry::Rect;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Index of a node in [`Screen::nodes`] (depth-first order of the XML dump).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Boolean attributes of a uiautomator node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeFlags {
    pub clickable: bool,
    pub long_clickable: bool,
    pub checkable: bool,
    pub focusable: bool,
    pub enabled: bool,
    pub scrollable: bool,
    pub password: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub depth: u16,
    pub index_in_parent: u16,
    pub class: String,
    pub package: Option<String>,
    pub resource_id: Option<String>,
    pub text: Option<String>,
    pub content_desc: Option<String>,
    /// Bounds clamped to the screen.
    pub bounds: Rect,
    /// True when the raw bounds exceeded the screen (the element is cut).
    pub clipped_by_screen: bool,
    pub flags: NodeFlags,
}

impl Node {
    /// Simple class name: `android.widget.Button` -> `Button`.
    pub fn simple_class(&self) -> &str {
        simple_class(&self.class)
    }

    /// Resource id without the package prefix: `com.app:id/btn_join` -> `btn_join`.
    pub fn short_resource_id(&self) -> Option<&str> {
        self.resource_id.as_deref().map(short_resource_id)
    }
}

pub fn simple_class(class: &str) -> &str {
    class.rsplit(['.', '$']).next().unwrap_or(class)
}

pub fn short_resource_id(id: &str) -> &str {
    match id.find(":id/") {
        Some(pos) => &id[pos + 4..],
        None => id,
    }
}

/// One analysed screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    pub width_px: u32,
    pub height_px: u32,
    pub nodes: Vec<Node>,
}

impl Screen {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// Iterates over the ancestors of `id`, nearest first.
    pub fn ancestors(&self, id: NodeId) -> Ancestors<'_> {
        Ancestors {
            screen: self,
            next: self.node(id).and_then(|n| n.parent),
        }
    }
}

pub struct Ancestors<'a> {
    screen: &'a Screen,
    next: Option<NodeId>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        let node = self.screen.node(self.next?)?;
        self.next = node.parent;
        Some(node)
    }
}

/// Capture device characteristics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Device {
    pub model: String,
    pub density_dpi: u32,
    pub width_px: u32,
    pub height_px: u32,
    #[serde(serialize_with = "crate::num::ser_round2")]
    pub font_scale: f64,
}

/// Decoded RGB8 screenshot, row-major. Kept in core so rules stay free of codecs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes.
    pub data: Vec<u8>,
}

impl Image {
    /// Returns `None` if the buffer size does not match the dimensions.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(3)?;
        (data.len() == expected).then_some(Self {
            width,
            height,
            data,
        })
    }

    /// Pixel at (x, y); callers must stay in bounds.
    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_and_id_helpers() {
        assert_eq!(simple_class("android.widget.ImageButton"), "ImageButton");
        assert_eq!(simple_class("View"), "View");
        assert_eq!(simple_class("a.b.Outer$Inner"), "Inner");
        assert_eq!(short_resource_id("com.app:id/btn_join"), "btn_join");
        assert_eq!(short_resource_id("search_box"), "search_box");
    }

    #[test]
    fn image_size_check() {
        assert!(Image::new(2, 2, vec![0; 12]).is_some());
        assert!(Image::new(2, 2, vec![0; 11]).is_none());
    }
}
