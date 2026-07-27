//! Converts `a2d-layout`'s physical-unit geometry (millimeters, top-left origin, y increasing
//! down) into `printpdf`'s coordinate system (points, bottom-left origin, y increasing up).
//! Isolated here so every rendering function works in the layout's natural coordinates and
//! never has to reason about the flip itself.

use a2d_layout::geometry::PhysicalRect;
use printpdf::{Mm, Pt, Rect};

/// Converts a physical-space y (top-left origin, down) into PDF space (bottom-left origin, up)
/// for a page of the given height.
pub fn flip_y(page_height_mm: f64, y_mm: f64) -> f64 {
    page_height_mm - y_mm
}

pub fn mm_to_pt(mm: f64) -> Pt {
    Mm(mm as f32).into()
}

/// Converts a layout rect (top-left origin) into a `printpdf::Rect` (bottom-left origin),
/// correctly flipping the rect's vertical anchor from its top edge to its bottom edge.
pub fn rect_to_pdf(page_height_mm: f64, rect: &PhysicalRect) -> Rect {
    let bottom_y_mm = flip_y(page_height_mm, rect.bottom());
    Rect::from_xywh(
        mm_to_pt(rect.left()),
        mm_to_pt(bottom_y_mm),
        mm_to_pt(rect.size.width_mm),
        mm_to_pt(rect.size.height_mm),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_y_maps_the_top_edge_to_the_page_height() {
        assert_eq!(flip_y(297.0, 0.0), 297.0);
    }

    #[test]
    fn flip_y_maps_the_bottom_edge_to_zero() {
        assert_eq!(flip_y(297.0, 297.0), 0.0);
    }

    #[test]
    fn rect_to_pdf_preserves_width_and_height() {
        let rect = PhysicalRect::new(10.0, 20.0, 5.0, 8.0);
        let pdf_rect = rect_to_pdf(297.0, &rect);
        assert!((pdf_rect.width.0 - mm_to_pt(5.0).0).abs() < 0.001);
        assert!((pdf_rect.height.0 - mm_to_pt(8.0).0).abs() < 0.001);
    }

    #[test]
    fn rect_to_pdf_anchors_at_the_rects_bottom_edge_in_pdf_space() {
        // A rect at layout y=20..28 on a 297mm-tall page has its bottom edge (y=28 in layout
        // space) at PDF y = 297-28 = 269.
        let rect = PhysicalRect::new(10.0, 20.0, 5.0, 8.0);
        let pdf_rect = rect_to_pdf(297.0, &rect);
        assert!((pdf_rect.y.0 - mm_to_pt(269.0).0).abs() < 0.001);
    }
}
