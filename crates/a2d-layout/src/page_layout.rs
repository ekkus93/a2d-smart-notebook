//! The canonical physical page layout model (TODO 5.1, spec §11.4/§12.3): four Corner Markers
//! at fixed semantic positions, a QR (Page Code) rectangle, a writable content rectangle, safe
//! margins, a quiet zone around every machine-readable element, and a calibration reference used
//! to detect print scaling errors (Milestone 5.6).

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId};

use crate::geometry::{PhysicalRect, PhysicalSize};

/// The four Corner Marker semantic positions (spec §11.4: "four Corner Markers with fixed
/// semantic positions"). Matches the `"TL"`/`"TR"`/`"BL"`/`"BR"` marker-role-id strings already
/// used by `NotebookDesign.marker_role_ids` (Milestone 2.3/4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkerRole {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl MarkerRole {
    pub const ALL: [MarkerRole; 4] = [
        MarkerRole::TopLeft,
        MarkerRole::TopRight,
        MarkerRole::BottomLeft,
        MarkerRole::BottomRight,
    ];

    /// The `marker_role_ids` string this role corresponds to (Milestone 2.3/4.4).
    pub fn as_id_str(&self) -> &'static str {
        match self {
            MarkerRole::TopLeft => "TL",
            MarkerRole::TopRight => "TR",
            MarkerRole::BottomLeft => "BL",
            MarkerRole::BottomRight => "BR",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerPlacement {
    pub role: MarkerRole,
    pub rect: PhysicalRect,
}

/// A printed reference of known physical length, used to detect print-scaling errors (Milestone
/// 5.6: "simulate 95%, 100%, and 105% print scaling") by comparing its measured size in a
/// captured image against `reference_length_mm`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibrationMark {
    pub rect: PhysicalRect,
    pub reference_length_mm: f64,
}

/// What, if anything, is procedurally drawn inside `content_rect` (spec §12.1: "blank, lined,
/// dot-grid, and graph styles"). Spacing is a physical measurement here, not a rendering
/// concern, so `a2d-pdf` (Milestone 5.4) can draw deterministically from the layout alone.
/// Doesn't change a layout's marker/QR/content-rect geometry — a paper size's four styles share
/// identical machine-readable geometry and differ only in what's drawn inside the writable area
/// — but each style still gets its own `LayoutId` (TODO 5.2), since the scanner and provenance
/// records identify a page's layout without needing to visually infer its ruling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentStyle {
    Blank,
    Lined { line_spacing_mm: f64 },
    DotGrid { spacing_mm: f64 },
    Graph { spacing_mm: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    pub id: LayoutId,
    pub physical_size: PhysicalSize,
    /// Printer-safe margin: the smallest inset used on any edge. `validate` checks every element
    /// against a uniform inset of this size on all four sides, so a layout with one wider edge
    /// margin than the others (e.g. a bound notebook's gutter exclusion, Milestone 5.3) still
    /// validates correctly — that edge's elements simply sit further inside the uniform floor
    /// than strictly required. The real per-edge insets are visible from the element positions
    /// themselves, not from this one field (spec §11.4 "no critical content in printer
    /// trim-risk regions").
    pub safe_margin_mm: f64,
    /// Required clear buffer around every machine-readable element (the four markers and the QR
    /// rect) — spec §11.4 "quiet zone around all machine-readable markers".
    pub quiet_zone_mm: f64,
    pub content_rect: PhysicalRect,
    pub markers: [MarkerPlacement; 4],
    pub qr_rect: PhysicalRect,
    pub visible_page_number_rect: Option<PhysicalRect>,
    pub calibration: CalibrationMark,
    pub content_style: ContentStyle,
}

fn layout_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.layout.invalid",
        message.into(),
        false,
    )
}

impl PageLayout {
    /// Validates bounds, overlap, marker roles, and quiet zones (TODO 5.1). Called by every
    /// concrete layout's own tests (Milestone 5.2/5.3) rather than trusted implicitly — a
    /// layout that fails this must never ship.
    pub fn validate(&self) -> Result<(), A2dError> {
        self.validate_marker_roles()?;
        self.validate_safe_margins()?;
        self.validate_no_unwanted_overlap()?;
        Ok(())
    }

    fn validate_marker_roles(&self) -> Result<(), A2dError> {
        for role in MarkerRole::ALL {
            let count = self.markers.iter().filter(|m| m.role == role).count();
            if count != 1 {
                return Err(layout_error(
                    "LAYOUT_MARKER_ROLE_NOT_UNIQUE",
                    format!(
                        "layout {} must place exactly one {:?} marker, found {count}",
                        self.id, role
                    ),
                ));
            }
        }
        Ok(())
    }

    fn safe_area(&self) -> PhysicalRect {
        PhysicalRect::new(
            self.safe_margin_mm,
            self.safe_margin_mm,
            (self.physical_size.width_mm - 2.0 * self.safe_margin_mm).max(0.0),
            (self.physical_size.height_mm - 2.0 * self.safe_margin_mm).max(0.0),
        )
    }

    fn validate_safe_margins(&self) -> Result<(), A2dError> {
        let safe_area = self.safe_area();
        let mut named_rects: Vec<(&str, PhysicalRect)> = vec![
            ("content_rect", self.content_rect),
            ("qr_rect", self.qr_rect),
            ("calibration.rect", self.calibration.rect),
        ];
        for marker in &self.markers {
            named_rects.push((marker.role.as_id_str(), marker.rect));
        }
        if let Some(rect) = self.visible_page_number_rect {
            named_rects.push(("visible_page_number_rect", rect));
        }
        for (name, rect) in named_rects {
            if !rect.is_within(&safe_area) {
                return Err(layout_error(
                    "LAYOUT_OUTSIDE_SAFE_MARGIN",
                    format!(
                        "layout {}'s {name} falls outside the {}mm safe margin",
                        self.id, self.safe_margin_mm
                    ),
                ));
            }
        }
        Ok(())
    }

    fn validate_no_unwanted_overlap(&self) -> Result<(), A2dError> {
        // Every machine-readable element (markers + QR) needs a clear quiet zone: its inflated
        // rect must not overlap the writable content, the visible page number, or any other
        // machine-readable element's own footprint.
        let mut machine_readable: Vec<(&str, PhysicalRect)> = self
            .markers
            .iter()
            .map(|m| (m.role.as_id_str(), m.rect))
            .collect();
        machine_readable.push(("qr_rect", self.qr_rect));

        for i in 0..machine_readable.len() {
            let (name_i, rect_i) = machine_readable[i];
            let inflated = rect_i.inflated(self.quiet_zone_mm);
            if inflated.intersects(&self.content_rect) {
                return Err(layout_error(
                    "LAYOUT_QUIET_ZONE_VIOLATION",
                    format!(
                        "layout {}'s {name_i} quiet zone overlaps content_rect",
                        self.id
                    ),
                ));
            }
            if let Some(number_rect) = self.visible_page_number_rect
                && inflated.intersects(&number_rect)
            {
                return Err(layout_error(
                    "LAYOUT_QUIET_ZONE_VIOLATION",
                    format!(
                        "layout {}'s {name_i} quiet zone overlaps visible_page_number_rect",
                        self.id
                    ),
                ));
            }
            for (name_j, rect_j) in machine_readable.iter().skip(i + 1) {
                if inflated.intersects(rect_j) {
                    return Err(layout_error(
                        "LAYOUT_QUIET_ZONE_VIOLATION",
                        format!(
                            "layout {}'s {name_i} and {name_j} do not leave a clear quiet zone \
                             between them",
                            self.id
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deliberately generous, obviously-valid layout to use as a baseline that individual
    /// tests mutate one field at a time.
    fn valid_layout() -> PageLayout {
        let marker_size = 10.0;
        let page = PhysicalSize::new(216.0, 279.0); // US Letter, mm
        PageLayout {
            id: LayoutId::parse("TEST-LAYOUT").unwrap(),
            physical_size: page,
            safe_margin_mm: 5.0,
            quiet_zone_mm: 2.0,
            content_rect: PhysicalRect::new(
                30.0,
                30.0,
                page.width_mm - 60.0,
                page.height_mm - 80.0,
            ),
            markers: [
                MarkerPlacement {
                    role: MarkerRole::TopLeft,
                    rect: PhysicalRect::new(6.0, 6.0, marker_size, marker_size),
                },
                MarkerPlacement {
                    role: MarkerRole::TopRight,
                    rect: PhysicalRect::new(
                        page.width_mm - 6.0 - marker_size,
                        6.0,
                        marker_size,
                        marker_size,
                    ),
                },
                MarkerPlacement {
                    role: MarkerRole::BottomLeft,
                    rect: PhysicalRect::new(
                        6.0,
                        page.height_mm - 6.0 - marker_size,
                        marker_size,
                        marker_size,
                    ),
                },
                MarkerPlacement {
                    role: MarkerRole::BottomRight,
                    rect: PhysicalRect::new(
                        page.width_mm - 6.0 - marker_size,
                        page.height_mm - 6.0 - marker_size,
                        marker_size,
                        marker_size,
                    ),
                },
            ],
            qr_rect: PhysicalRect::new(
                page.width_mm / 2.0 - 7.5,
                page.height_mm - 20.0,
                15.0,
                15.0,
            ),
            // Bottom-left-of-center, clear of the BL marker's and QR's quiet zones.
            visible_page_number_rect: Some(PhysicalRect::new(
                25.0,
                page.height_mm - 15.0,
                20.0,
                8.0,
            )),
            // Top-center, clear of the TL/TR markers' quiet zones.
            calibration: CalibrationMark {
                rect: PhysicalRect::new(page.width_mm / 2.0 - 10.0, 6.0, 20.0, 2.0),
                reference_length_mm: 20.0,
            },
            content_style: ContentStyle::Blank,
        }
    }

    #[test]
    fn a_well_formed_layout_validates() {
        valid_layout().validate().unwrap();
    }

    #[test]
    fn rejects_a_missing_marker_role() {
        let mut layout = valid_layout();
        layout.markers[3].role = MarkerRole::TopLeft; // duplicate TL, no BR
        let err = layout.validate().unwrap_err();
        assert!(err.code.to_string().contains("MARKER_ROLE_NOT_UNIQUE"));
    }

    #[test]
    fn rejects_a_marker_outside_the_safe_margin() {
        let mut layout = valid_layout();
        layout.markers[0].rect = PhysicalRect::new(0.0, 0.0, 10.0, 10.0);
        let err = layout.validate().unwrap_err();
        assert!(err.code.to_string().contains("OUTSIDE_SAFE_MARGIN"));
    }

    #[test]
    fn rejects_content_rect_extending_past_the_safe_margin() {
        let mut layout = valid_layout();
        layout.content_rect = PhysicalRect::new(
            0.0,
            0.0,
            layout.physical_size.width_mm,
            layout.physical_size.height_mm,
        );
        let err = layout.validate().unwrap_err();
        assert!(err.code.to_string().contains("OUTSIDE_SAFE_MARGIN"));
    }

    #[test]
    fn rejects_a_marker_overlapping_the_content_rect_quiet_zone() {
        let mut layout = valid_layout();
        // Push the top-left marker deep into the content area.
        layout.markers[0].rect = PhysicalRect::new(35.0, 35.0, 10.0, 10.0);
        let err = layout.validate().unwrap_err();
        assert!(err.code.to_string().contains("QUIET_ZONE_VIOLATION"));
    }

    #[test]
    fn rejects_two_markers_without_a_clear_quiet_zone_between_them() {
        let mut layout = valid_layout();
        layout.markers[1].rect = layout.markers[0].rect;
        // Also move it off the content rect so this test isolates the marker-vs-marker case.
        layout.markers[1].rect.origin.x_mm += 1.0;
        let err = layout.validate().unwrap_err();
        assert!(err.code.to_string().contains("QUIET_ZONE_VIOLATION"));
    }

    #[test]
    fn rejects_the_qr_rect_overlapping_the_visible_page_number_rect_quiet_zone() {
        let mut layout = valid_layout();
        layout.qr_rect = layout.visible_page_number_rect.unwrap();
        let err = layout.validate().unwrap_err();
        assert!(err.code.to_string().contains("QUIET_ZONE_VIOLATION"));
    }
}
