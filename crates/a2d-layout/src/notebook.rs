//! Bound-notebook layout (TODO 5.3, spec §11.1-11.4): the physical A2D Smart Notebook's Setup
//! Page and writable page layouts.
//!
//! Recto-only by construction (spec §11.2 "only right-hand/recto pages are usable and
//! scannable"): this module defines geometry for usable recto pages only. There is no verso
//! layout type at all — a verso page carries no markers, no QR, no content geometry, it's simply
//! a blank page in the interior PDF (spec: "Verso pages remain blank in v0.1"). Emitting that
//! blank page into the actual PDF bytes is Milestone 5.4's job; this module only defines what a
//! *usable* page looks like.
//!
//! The layout IDs here (`DEV-SETUP-V1`, `DEV-PAGE-V1`) intentionally match
//! `crates/a2d-layout/manifests/dev-placeholder.json`'s `setup_layout_id`/`page_layout_id`
//! (Milestone 4.4) — that manifest's layout references now resolve to real geometry rather than
//! arbitrary placeholder strings, even though the manifest and this trim size remain development
//! placeholders, not an official released design (no real physical Notebook Design exists until
//! Milestone 5 is followed by physical print validation, Milestone 17).

use a2d_domain::LayoutId;

use crate::geometry::PhysicalSize;
use crate::layout_builder::{BuildLayoutParams, build_layout};
use crate::page_layout::{ContentStyle, PageLayout};

/// TODO 5.3 "record the first trim-size decision". 6in x 9in (152.4mm x 228.6mm, rounded to
/// whole millimeters), a common print-on-demand journal/notebook trim size -- a reasonable
/// balance between portability and writable area. Recorded as a starting assumption per
/// CLAUDE.md's open-decisions policy, not a measured/validated value; Milestone 17's physical
/// print validation is what actually confirms or revises it.
pub const NOTEBOOK_TRIM_SIZE_MM: PhysicalSize = PhysicalSize {
    width_mm: 152.0,
    height_mm: 229.0,
};

const OUTER_MARGIN_MM: f64 = 6.0;
/// TODO 5.3 "define a larger left/gutter exclusion" (spec §11.2: "the gutter-side writable
/// exclusion zone is larger than outer margins"). Substantially larger than `OUTER_MARGIN_MM` to
/// keep both the left-column Corner Markers and the writable content clear of spine-binding
/// curvature (spec §11.2: "Important writing areas MUST NOT extend into the expected
/// spine-curvature zone").
const GUTTER_MARGIN_MM: f64 = 20.0;
const QUIET_ZONE_MM: f64 = 3.0;
const MARKER_AND_QR_SIZE_MM: f64 = 18.0;

fn notebook_layout_id(token: &str) -> LayoutId {
    LayoutId::parse(token).expect("notebook layout id tokens are always valid LayoutId strings")
}

/// The notebook's Setup Page (spec §11.3): branding, design name/version, a Setup Code, and
/// registration instructions. No logical page number -- the setup page isn't part of
/// `NotebookDesign.logical_page_count`.
pub fn setup_page_layout() -> PageLayout {
    build_layout(BuildLayoutParams {
        id: notebook_layout_id("DEV-SETUP-V1"),
        physical_size: NOTEBOOK_TRIM_SIZE_MM,
        left_margin_mm: GUTTER_MARGIN_MM,
        margin_mm: OUTER_MARGIN_MM,
        quiet_zone_mm: QUIET_ZONE_MM,
        marker_and_qr_size_mm: MARKER_AND_QR_SIZE_MM,
        content_style: ContentStyle::Blank,
        include_visible_page_number: false,
    })
}

/// A regular writable page (spec §11.4): four Corner Markers, a Page Code, a visible logical
/// page number, and a writable content rectangle inset by the gutter exclusion on the left.
pub fn writable_page_layout() -> PageLayout {
    build_layout(BuildLayoutParams {
        id: notebook_layout_id("DEV-PAGE-V1"),
        physical_size: NOTEBOOK_TRIM_SIZE_MM,
        left_margin_mm: GUTTER_MARGIN_MM,
        margin_mm: OUTER_MARGIN_MM,
        quiet_zone_mm: QUIET_ZONE_MM,
        marker_and_qr_size_mm: MARKER_AND_QR_SIZE_MM,
        content_style: ContentStyle::Blank,
        include_visible_page_number: true,
    })
}

/// Maps a 1-based logical (recto) page number to its 1-based position in the interior PDF (TODO
/// 5.3 "define logical numbering independent of manuscript PDF page number"; CLAUDE.md: "logical
/// page numbers != PDF page numbers"). The interior alternates recto/verso starting with the
/// Setup Page: PDF page 1 is the Setup Page, page 2 is its blank verso, logical page 1 is PDF
/// page 3, logical page 2 is PDF page 5, and so on -- each logical page consumes one recto PDF
/// page plus the blank verso PDF page immediately before it.
pub fn pdf_page_number_for_logical_page(logical_page_number: u32) -> u32 {
    assert!(logical_page_number >= 1, "logical page numbers are 1-based");
    2 * logical_page_number + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_page_layout_validates() {
        setup_page_layout().validate().unwrap();
    }

    #[test]
    fn writable_page_layout_validates() {
        writable_page_layout().validate().unwrap();
    }

    #[test]
    fn setup_page_has_no_visible_page_number() {
        assert_eq!(setup_page_layout().visible_page_number_rect, None);
    }

    #[test]
    fn writable_page_has_a_visible_page_number() {
        assert!(writable_page_layout().visible_page_number_rect.is_some());
    }

    #[test]
    fn the_gutter_exclusion_is_wider_than_the_outer_margin() {
        for layout in [setup_page_layout(), writable_page_layout()] {
            let top_left = layout
                .markers
                .iter()
                .find(|m| m.role == crate::page_layout::MarkerRole::TopLeft)
                .unwrap();
            let top_right = layout
                .markers
                .iter()
                .find(|m| m.role == crate::page_layout::MarkerRole::TopRight)
                .unwrap();
            let left_inset = top_left.rect.left();
            let right_inset = layout.physical_size.width_mm
                - (top_right.rect.left() + top_right.rect.size.width_mm);
            assert_eq!(left_inset, GUTTER_MARGIN_MM);
            assert_eq!(right_inset, OUTER_MARGIN_MM);
            assert!(
                left_inset > right_inset,
                "gutter exclusion must be wider than the outer margin"
            );
        }
    }

    #[test]
    fn important_content_never_extends_into_the_gutter_exclusion_zone() {
        for layout in [setup_page_layout(), writable_page_layout()] {
            assert!(layout.content_rect.left() >= GUTTER_MARGIN_MM);
        }
    }

    #[test]
    fn setup_and_writable_layouts_use_the_same_trim_size() {
        assert_eq!(
            setup_page_layout().physical_size,
            writable_page_layout().physical_size
        );
        assert_eq!(setup_page_layout().physical_size, NOTEBOOK_TRIM_SIZE_MM);
    }

    #[test]
    fn logical_page_one_starts_at_pdf_page_three() {
        assert_eq!(pdf_page_number_for_logical_page(1), 3);
    }

    #[test]
    fn logical_page_numbering_advances_by_two_pdf_pages_per_logical_page() {
        assert_eq!(pdf_page_number_for_logical_page(2), 5);
        assert_eq!(pdf_page_number_for_logical_page(3), 7);
        assert_eq!(pdf_page_number_for_logical_page(100), 201);
    }

    #[test]
    fn pdf_page_number_is_never_equal_to_the_logical_page_number() {
        for logical in 1..=50u32 {
            assert_ne!(pdf_page_number_for_logical_page(logical), logical);
        }
    }

    #[test]
    #[should_panic(expected = "1-based")]
    fn logical_page_zero_is_rejected() {
        pdf_page_number_for_logical_page(0);
    }
}
