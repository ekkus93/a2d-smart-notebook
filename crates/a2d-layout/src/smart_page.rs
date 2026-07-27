//! Concrete Smart Page layouts (TODO 5.2, spec §12.1): US Letter and A4 portrait, each in Blank,
//! Lined, Dot grid, and Graph styles — eight layouts total, all sharing identical machine-
//! readable geometry within a paper size (only `content_style` differs, spec §12.1's "known
//! layout identifier" still assigns each combination its own `LayoutId`, TODO 5.2).
//!
//! Every physical measurement here (safe margin, marker size, QR size, quiet zone, ruling
//! spacing) is a starting assumption, not a measured value — spec §11.5/CLAUDE.md's "don't
//! invent thresholds" governs *capture-quality* thresholds specifically; these are physical
//! print-layout dimensions, which CLAUDE.md's "open decisions" policy explicitly allows picking
//! a sensible default for and recording here. Milestone 17's physical print validation is what
//! actually confirms or revises these numbers.

use a2d_domain::LayoutId;

use crate::geometry::{PhysicalRect, PhysicalSize};
use crate::page_layout::{CalibrationMark, ContentStyle, MarkerPlacement, MarkerRole, PageLayout};

/// mm. Printer-safe inset from every edge — most consumer printers can't reliably print full
/// bleed; 6mm sits close to the common 0.25in (6.35mm) convention.
const SAFE_MARGIN_MM: f64 = 6.0;
/// mm. Clear buffer required around every machine-readable element.
const QUIET_ZONE_MM: f64 = 3.0;
/// mm. Both the Corner Marker and QR footprints share this size for now — big enough for a
/// phone camera to resolve at typical note-taking arm's-length distance, small enough to leave
/// most of the page for writing.
const MARKER_AND_QR_SIZE_MM: f64 = 18.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperSize {
    UsLetter,
    A4,
}

impl PaperSize {
    pub fn physical_size(&self) -> PhysicalSize {
        match self {
            // 8.5in x 11in.
            PaperSize::UsLetter => PhysicalSize::new(215.9, 279.4),
            PaperSize::A4 => PhysicalSize::new(210.0, 297.0),
        }
    }

    fn id_token(&self) -> &'static str {
        match self {
            PaperSize::UsLetter => "LETTER",
            PaperSize::A4 => "A4",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmartPageStyle {
    Blank,
    Lined,
    DotGrid,
    Graph,
}

impl SmartPageStyle {
    fn id_token(&self) -> &'static str {
        match self {
            SmartPageStyle::Blank => "BLANK",
            SmartPageStyle::Lined => "LINED",
            SmartPageStyle::DotGrid => "DOTGRID",
            SmartPageStyle::Graph => "GRAPH",
        }
    }

    fn content_style(&self) -> ContentStyle {
        match self {
            SmartPageStyle::Blank => ContentStyle::Blank,
            // "College ruled"-ish spacing.
            SmartPageStyle::Lined => ContentStyle::Lined {
                line_spacing_mm: 7.0,
            },
            // Common dot-grid notebook spacing (e.g. Leuchtturm/Rhodia dot grids).
            SmartPageStyle::DotGrid => ContentStyle::DotGrid { spacing_mm: 5.0 },
            SmartPageStyle::Graph => ContentStyle::Graph { spacing_mm: 5.0 },
        }
    }
}

/// Builds one of the eight Smart Page layouts. Always returns an internally valid layout — this
/// function is exhaustively tested via [`PageLayout::validate`] rather than trusted by
/// construction, since a hand-derived formula is exactly the kind of thing that's easy to get
/// subtly wrong for one paper size while looking fine for the other.
pub fn smart_page_layout(paper: PaperSize, style: SmartPageStyle) -> PageLayout {
    let size = paper.physical_size();
    let w = size.width_mm;
    let h = size.height_mm;
    let marker = MARKER_AND_QR_SIZE_MM;

    let id = LayoutId::parse(&format!("SP-{}-{}-V1", paper.id_token(), style.id_token()))
        .expect("layout id tokens are always valid LayoutId characters within length");

    let markers = [
        MarkerPlacement {
            role: MarkerRole::TopLeft,
            rect: PhysicalRect::new(SAFE_MARGIN_MM, SAFE_MARGIN_MM, marker, marker),
        },
        MarkerPlacement {
            role: MarkerRole::TopRight,
            rect: PhysicalRect::new(w - SAFE_MARGIN_MM - marker, SAFE_MARGIN_MM, marker, marker),
        },
        MarkerPlacement {
            role: MarkerRole::BottomLeft,
            rect: PhysicalRect::new(SAFE_MARGIN_MM, h - SAFE_MARGIN_MM - marker, marker, marker),
        },
        MarkerPlacement {
            role: MarkerRole::BottomRight,
            rect: PhysicalRect::new(
                w - SAFE_MARGIN_MM - marker,
                h - SAFE_MARGIN_MM - marker,
                marker,
                marker,
            ),
        },
    ];

    // QR sits bottom-center, in the same row as the bottom markers.
    let qr_rect = PhysicalRect::new(
        w / 2.0 - marker / 2.0,
        h - SAFE_MARGIN_MM - marker,
        marker,
        marker,
    );

    // Visible page number: right of the QR, vertically centered in the bottom marker row, with
    // enough clearance before the bottom-right marker's quiet zone.
    let number_width = 20.0;
    let number_height = 8.0;
    let visible_page_number_rect = Some(PhysicalRect::new(
        w / 2.0 + marker / 2.0 + QUIET_ZONE_MM + 5.0,
        h - SAFE_MARGIN_MM - marker / 2.0 - number_height / 2.0,
        number_width,
        number_height,
    ));

    // Calibration mark: top-center, vertically centered in the top marker row.
    let calibration_width = 20.0;
    let calibration_height = 2.0;
    let calibration = CalibrationMark {
        rect: PhysicalRect::new(
            w / 2.0 - calibration_width / 2.0,
            SAFE_MARGIN_MM + (marker - calibration_height) / 2.0,
            calibration_width,
            calibration_height,
        ),
        reference_length_mm: calibration_width,
    };

    // Writable area: everything between the top and bottom marker/QR rows (each row's height is
    // `marker`, since the QR shares the marker's size), inset by the quiet zone above and below,
    // full safe-margin width -- Smart Pages are loose pages with no gutter exclusion (that's
    // Milestone 5.3's bound-notebook-only concern).
    let content_top = SAFE_MARGIN_MM + marker + QUIET_ZONE_MM;
    let content_bottom = h - SAFE_MARGIN_MM - marker - QUIET_ZONE_MM;
    let content_rect = PhysicalRect::new(
        SAFE_MARGIN_MM,
        content_top,
        w - 2.0 * SAFE_MARGIN_MM,
        content_bottom - content_top,
    );

    PageLayout {
        id,
        physical_size: size,
        safe_margin_mm: SAFE_MARGIN_MM,
        quiet_zone_mm: QUIET_ZONE_MM,
        content_rect,
        markers,
        qr_rect,
        visible_page_number_rect,
        calibration,
        content_style: style.content_style(),
    }
}

/// Every deterministic style/spacing combination this build defines. Callers rendering "the
/// four styles" (spec §12.1) iterate this rather than hand-listing each `SmartPageStyle` variant
/// again.
pub const ALL_STYLES: [SmartPageStyle; 4] = [
    SmartPageStyle::Blank,
    SmartPageStyle::Lined,
    SmartPageStyle::DotGrid,
    SmartPageStyle::Graph,
];

pub const ALL_PAPER_SIZES: [PaperSize; 2] = [PaperSize::UsLetter, PaperSize::A4];

#[cfg(test)]
mod tests {
    use super::*;

    fn all_layouts() -> Vec<PageLayout> {
        ALL_PAPER_SIZES
            .into_iter()
            .flat_map(|paper| ALL_STYLES.into_iter().map(move |style| (paper, style)))
            .map(|(paper, style)| smart_page_layout(paper, style))
            .collect()
    }

    #[test]
    fn every_paper_size_and_style_combination_produces_a_valid_layout() {
        for layout in all_layouts() {
            layout
                .validate()
                .unwrap_or_else(|e| panic!("layout {} failed validation: {e}", layout.id));
        }
    }

    #[test]
    fn eight_distinct_layouts_are_produced() {
        let ids: std::collections::HashSet<String> =
            all_layouts().iter().map(|l| l.id.to_string()).collect();
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn dimensions_and_spacing_are_deterministic_across_calls() {
        let a = smart_page_layout(PaperSize::UsLetter, SmartPageStyle::Lined);
        let b = smart_page_layout(PaperSize::UsLetter, SmartPageStyle::Lined);
        assert_eq!(a, b);
    }

    #[test]
    fn markers_and_qr_stay_within_the_printer_safe_margin() {
        for layout in all_layouts() {
            let safe_area = PhysicalRect::new(
                layout.safe_margin_mm,
                layout.safe_margin_mm,
                layout.physical_size.width_mm - 2.0 * layout.safe_margin_mm,
                layout.physical_size.height_mm - 2.0 * layout.safe_margin_mm,
            );
            for marker in &layout.markers {
                assert!(
                    marker.rect.is_within(&safe_area),
                    "{} marker {:?} escapes the safe margin",
                    layout.id,
                    marker.role
                );
            }
            assert!(
                layout.qr_rect.is_within(&safe_area),
                "{} qr_rect escapes the safe margin",
                layout.id
            );
        }
    }

    #[test]
    fn content_rect_never_overlaps_any_marker_or_the_qr_code() {
        for layout in all_layouts() {
            for marker in &layout.markers {
                assert!(
                    !layout.content_rect.intersects(&marker.rect),
                    "{} content_rect overlaps {:?} marker",
                    layout.id,
                    marker.role
                );
            }
            assert!(
                !layout.content_rect.intersects(&layout.qr_rect),
                "{} content_rect overlaps qr_rect",
                layout.id
            );
        }
    }

    #[test]
    fn each_style_carries_its_own_content_style_metadata() {
        let blank = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        assert_eq!(blank.content_style, ContentStyle::Blank);

        let lined = smart_page_layout(PaperSize::A4, SmartPageStyle::Lined);
        assert_eq!(
            lined.content_style,
            ContentStyle::Lined {
                line_spacing_mm: 7.0
            }
        );

        let dots = smart_page_layout(PaperSize::A4, SmartPageStyle::DotGrid);
        assert_eq!(
            dots.content_style,
            ContentStyle::DotGrid { spacing_mm: 5.0 }
        );

        let graph = smart_page_layout(PaperSize::A4, SmartPageStyle::Graph);
        assert_eq!(graph.content_style, ContentStyle::Graph { spacing_mm: 5.0 });
    }

    #[test]
    fn letter_and_a4_layouts_use_their_respective_physical_dimensions() {
        let letter = smart_page_layout(PaperSize::UsLetter, SmartPageStyle::Blank);
        assert_eq!(letter.physical_size, PhysicalSize::new(215.9, 279.4));

        let a4 = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        assert_eq!(a4.physical_size, PhysicalSize::new(210.0, 297.0));
    }
}
