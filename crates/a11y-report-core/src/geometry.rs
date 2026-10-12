//! Screen-space rectangles, in physical pixels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Axis-aligned rectangle `[x1,y1][x2,y2]` with exclusive right/bottom edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Rect {
    pub const fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn width(&self) -> i32 {
        (self.x2 - self.x1).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.y2 - self.y1).max(0)
    }

    pub fn area(&self) -> i64 {
        i64::from(self.width()) * i64::from(self.height())
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    /// Clamps the rectangle to `[0, width) × [0, height)`.
    pub fn clamp_to(&self, width: u32, height: u32) -> Rect {
        let w = i32::try_from(width).unwrap_or(i32::MAX);
        let h = i32::try_from(height).unwrap_or(i32::MAX);
        Rect {
            x1: self.x1.clamp(0, w),
            y1: self.y1.clamp(0, h),
            x2: self.x2.clamp(0, w),
            y2: self.y2.clamp(0, h),
        }
    }

    /// Shrinks every side by `by` pixels (may become empty).
    pub fn inset(&self, by: i32) -> Rect {
        Rect {
            x1: self.x1 + by,
            y1: self.y1 + by,
            x2: (self.x2 - by).max(self.x1 + by),
            y2: (self.y2 - by).max(self.y1 + by),
        }
    }

    /// Translates the rectangle.
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x1 + dx, self.y1 + dy, self.x2 + dx, self.y2 + dy)
    }

    pub fn as_array(&self) -> [i32; 4] {
        [self.x1, self.y1, self.x2, self.y2]
    }
}

impl Serialize for Rect {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.as_array().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [x1, y1, x2, y2] = <[i32; 4]>::deserialize(d)?;
        Ok(Rect { x1, y1, x2, y2 })
    }
}

impl JsonSchema for Rect {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Rect".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Bounds in physical pixels: [x1, y1, x2, y2].",
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 4,
            "maxItems": 4
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_and_area() {
        let r = Rect::new(-10, 5, 1200, 50);
        let c = r.clamp_to(1080, 2400);
        assert_eq!(c, Rect::new(0, 5, 1080, 50));
        assert_eq!(c.area(), 1080 * 45);
        assert!(Rect::new(0, 0, 0, 0).is_empty());
        assert_eq!(Rect::new(10, 10, 5, 20).width(), 0);
    }

    #[test]
    fn inset_never_inverts() {
        let r = Rect::new(0, 0, 3, 3).inset(2);
        assert!(r.is_empty());
        assert_eq!(Rect::new(0, 0, 10, 10).inset(1), Rect::new(1, 1, 9, 9));
    }
}
