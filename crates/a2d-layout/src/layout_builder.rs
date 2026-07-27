//! Shared geometry construction for concrete layouts. Smart Page layouts (Milestone 5.2) and
//! bound-notebook layouts (Milestone 5.3) place markers, the QR code, the calibration mark, and
//! the visible page number the same way — four corners, QR/number in the bottom row,
//! calibration in the top row, content filling the space between — differing only in how far
//! each edge is inset. Extracted here once both needed it, rather than duplicated.

use a2d_domain::LayoutId;

use crate::geometry::{PhysicalRect, PhysicalSize};
use crate::page_layout::{CalibrationMark, ContentStyle, MarkerPlacement, MarkerRole, PageLayout};

pub struct BuildLayoutParams {
    pub id: LayoutId,
    pub physical_size: PhysicalSize,
    /// Left-edge inset for the left-column markers. Equal to `margin_mm` for a symmetric layout
    /// (Smart Pages); larger than `margin_mm` for a bound notebook's gutter exclusion
    /// (Milestone 5.3).
    pub left_margin_mm: f64,
    /// Inset from the top, right, and bottom edges.
    pub margin_mm: f64,
    pub quiet_zone_mm: f64,
    /// Both the Corner Marker and QR footprints share this size.
    pub marker_and_qr_size_mm: f64,
    pub content_style: ContentStyle,
    /// Setup pages have no logical page number to display; writable pages do.
    pub include_visible_page_number: bool,
}

/// Builds a [`PageLayout`] from [`BuildLayoutParams`]. Always returns an internally valid
/// layout — callers still run [`PageLayout::validate`] in their own tests rather than trusting
/// that blindly, since a hand-derived formula is exactly the kind of thing that's easy to get
/// subtly wrong for one input while looking fine for another.
pub fn build_layout(params: BuildLayoutParams) -> PageLayout {
    let w = params.physical_size.width_mm;
    let h = params.physical_size.height_mm;
    let marker = params.marker_and_qr_size_mm;
    let left = params.left_margin_mm;
    let margin = params.margin_mm;
    let quiet_zone = params.quiet_zone_mm;

    let markers = [
        MarkerPlacement {
            role: MarkerRole::TopLeft,
            rect: PhysicalRect::new(left, margin, marker, marker),
        },
        MarkerPlacement {
            role: MarkerRole::TopRight,
            rect: PhysicalRect::new(w - margin - marker, margin, marker, marker),
        },
        MarkerPlacement {
            role: MarkerRole::BottomLeft,
            rect: PhysicalRect::new(left, h - margin - marker, marker, marker),
        },
        MarkerPlacement {
            role: MarkerRole::BottomRight,
            rect: PhysicalRect::new(w - margin - marker, h - margin - marker, marker, marker),
        },
    ];

    // The writable area's horizontal center -- not the physical page's center -- so an
    // asymmetric left margin (the notebook gutter exclusion) still centers the QR, page number,
    // and calibration mark within the space that's actually usable.
    let content_left = left + marker + quiet_zone;
    let content_right = w - margin;
    let content_center_x = (content_left + content_right) / 2.0;

    let qr_rect = PhysicalRect::new(
        content_center_x - marker / 2.0,
        h - margin - marker,
        marker,
        marker,
    );

    let calibration_width = 20.0;
    let calibration_height = 2.0;
    let calibration = CalibrationMark {
        rect: PhysicalRect::new(
            content_center_x - calibration_width / 2.0,
            margin + (marker - calibration_height) / 2.0,
            calibration_width,
            calibration_height,
        ),
        reference_length_mm: calibration_width,
    };

    // The visible page number sits in its own horizontal strip directly above the marker/QR
    // row, spanning the content width -- not squeezed in beside the QR -- so it stays clear of
    // every machine-readable element's quiet zone regardless of how narrow the page is (a fixed
    // horizontal offset next to the QR can run out of room on a narrow page like the bound
    // notebook's trim size, TODO 5.3, even though it has plenty of room on a Smart Page's wider
    // paper sizes, TODO 5.2).
    let number_width = 20.0;
    let number_height = 8.0;
    // Explicit gaps rather than exactly-touching edges, so the boundary isn't sensitive to
    // floating-point rounding on non-exact physical dimensions (e.g. US Letter's 215.9mm).
    let gap = 1.0;
    let number_strip_height = if params.include_visible_page_number {
        gap + number_height + gap
    } else {
        0.0
    };

    let visible_page_number_rect = params.include_visible_page_number.then(|| {
        let bottom = h - margin - marker - quiet_zone - gap;
        PhysicalRect::new(
            content_center_x - number_width / 2.0,
            bottom - number_height,
            number_width,
            number_height,
        )
    });

    let content_top = margin + marker + quiet_zone;
    let content_bottom = h - margin - marker - quiet_zone - number_strip_height;
    let content_rect = PhysicalRect::new(
        content_left,
        content_top,
        content_right - content_left,
        content_bottom - content_top,
    );

    PageLayout {
        id: params.id,
        physical_size: params.physical_size,
        safe_margin_mm: margin.min(left),
        quiet_zone_mm: quiet_zone,
        content_rect,
        markers,
        qr_rect,
        visible_page_number_rect,
        calibration,
        content_style: params.content_style,
    }
}
