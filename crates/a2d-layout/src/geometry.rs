//! Physical-unit geometry primitives (TODO 5.1: "use fixed physical units, not authoritative
//! screen pixels"). Every measurement is in millimeters, never device pixels — a layout is
//! defined once, physically, and rendered at whatever resolution a given output needs.
//!
//! Coordinates use the page's top-left corner as the origin, x increasing right and y increasing
//! down (natural reading order). PDF's own coordinate system is bottom-left-origin with y
//! increasing up; converting between the two is `a2d-pdf`'s job (Milestone 5.4) — this crate
//! stays renderer-agnostic and never assumes a particular output format's coordinate convention.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalSize {
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PhysicalSize {
    pub fn new(width_mm: f64, height_mm: f64) -> Self {
        Self {
            width_mm,
            height_mm,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalPoint {
    pub x_mm: f64,
    pub y_mm: f64,
}

impl PhysicalPoint {
    pub fn new(x_mm: f64, y_mm: f64) -> Self {
        Self { x_mm, y_mm }
    }
}

/// An axis-aligned rectangle in physical page space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalRect {
    pub origin: PhysicalPoint,
    pub size: PhysicalSize,
}

impl PhysicalRect {
    pub fn new(x_mm: f64, y_mm: f64, width_mm: f64, height_mm: f64) -> Self {
        Self {
            origin: PhysicalPoint::new(x_mm, y_mm),
            size: PhysicalSize::new(width_mm, height_mm),
        }
    }

    pub fn left(&self) -> f64 {
        self.origin.x_mm
    }

    pub fn top(&self) -> f64 {
        self.origin.y_mm
    }

    pub fn right(&self) -> f64 {
        self.origin.x_mm + self.size.width_mm
    }

    pub fn bottom(&self) -> f64 {
        self.origin.y_mm + self.size.height_mm
    }

    /// Grows the rect outward by `margin_mm` on every side. Used to derive a machine-readable
    /// marker's or the QR code's required quiet zone from its bare rect.
    pub fn inflated(&self, margin_mm: f64) -> Self {
        Self::new(
            self.left() - margin_mm,
            self.top() - margin_mm,
            self.size.width_mm + 2.0 * margin_mm,
            self.size.height_mm + 2.0 * margin_mm,
        )
    }

    /// True if the two rects' interiors overlap. Edges that merely touch do not count as
    /// overlapping.
    pub fn intersects(&self, other: &Self) -> bool {
        self.left() < other.right()
            && other.left() < self.right()
            && self.top() < other.bottom()
            && other.top() < self.bottom()
    }

    /// True if this rect lies entirely within `bounds`, edges inclusive.
    pub fn is_within(&self, bounds: &Self) -> bool {
        self.left() >= bounds.left()
            && self.top() >= bounds.top()
            && self.right() <= bounds.right()
            && self.bottom() <= bounds.bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflated_grows_by_the_margin_on_every_side() {
        let rect = PhysicalRect::new(10.0, 10.0, 5.0, 5.0);
        let grown = rect.inflated(2.0);
        assert_eq!(grown, PhysicalRect::new(8.0, 8.0, 9.0, 9.0));
    }

    #[test]
    fn overlapping_rects_intersect() {
        let a = PhysicalRect::new(0.0, 0.0, 10.0, 10.0);
        let b = PhysicalRect::new(5.0, 5.0, 10.0, 10.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn merely_touching_rects_do_not_intersect() {
        let a = PhysicalRect::new(0.0, 0.0, 10.0, 10.0);
        let b = PhysicalRect::new(10.0, 0.0, 10.0, 10.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn disjoint_rects_do_not_intersect() {
        let a = PhysicalRect::new(0.0, 0.0, 10.0, 10.0);
        let b = PhysicalRect::new(20.0, 20.0, 10.0, 10.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn a_rect_fully_inside_bounds_is_within() {
        let bounds = PhysicalRect::new(0.0, 0.0, 100.0, 100.0);
        let inner = PhysicalRect::new(10.0, 10.0, 20.0, 20.0);
        assert!(inner.is_within(&bounds));
    }

    #[test]
    fn a_rect_touching_the_bounds_edge_is_within() {
        let bounds = PhysicalRect::new(0.0, 0.0, 100.0, 100.0);
        let edge = PhysicalRect::new(0.0, 0.0, 100.0, 100.0);
        assert!(edge.is_within(&bounds));
    }

    #[test]
    fn a_rect_extending_past_bounds_is_not_within() {
        let bounds = PhysicalRect::new(0.0, 0.0, 100.0, 100.0);
        let over = PhysicalRect::new(90.0, 90.0, 20.0, 20.0);
        assert!(!over.is_within(&bounds));
    }
}
