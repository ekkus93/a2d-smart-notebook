//! Renders one page's content stream from a resolved [`PageLayout`] (TODO 5.4). Every element is
//! vector drawing (filled polygons, stroked lines, and standard-font text) — nothing here embeds
//! a raster image or an external font file, so there is no interpolation blur to worry about and
//! no font-licensing question to answer (spec §12.3's "legally distributable fonts" requirement
//! is satisfied by construction: `BuiltinFont` uses the 14 standard PDF fonts, which every PDF
//! viewer/printer already has and which require no embedded font program).
//!
//! **Corner Markers are a placeholder shape** (a bordered black square), not a real AprilTag bit
//! pattern — the actual tag family isn't decided until Milestone 7 accepts
//! `docs/decisions/0002-apriltag-detector-selection.md`. Swapping in the real pattern only
//! touches [`marker_ops`]; every other rendering function and the layout geometry itself is
//! unaffected.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_layout::geometry::PhysicalRect;
use a2d_layout::page_layout::{CalibrationMark, ContentStyle, PageLayout};
use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PdfFontHandle, Point, Pt, Rgb, TextItem,
};
use qrcode::EcLevel;
use qrcode::types::Color as QrModuleColor;

use crate::coordinates::{flip_y, mm_to_pt, rect_to_pdf};
use crate::error::qr_encode_error;

fn black() -> Color {
    Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    })
}

fn white() -> Color {
    Color::Rgb(Rgb {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        icc_profile: None,
    })
}

fn ruling_gray() -> Color {
    Color::Rgb(Rgb {
        r: 0.6,
        g: 0.6,
        b: 0.6,
        icc_profile: None,
    })
}

fn filled_rect_ops(page_height_mm: f64, rect: &PhysicalRect, color: Color) -> Vec<Op> {
    let pdf_rect = rect_to_pdf(page_height_mm, rect);
    vec![
        Op::SaveGraphicsState,
        Op::SetFillColor { col: color },
        Op::DrawPolygon {
            polygon: pdf_rect.to_polygon(),
        },
        Op::RestoreGraphicsState,
    ]
}

fn horizontal_line_ops(page_height_mm: f64, left_mm: f64, right_mm: f64, y_mm: f64) -> Op {
    let pdf_y = flip_y(page_height_mm, y_mm) as f32;
    Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(left_mm as f32), Mm(pdf_y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(right_mm as f32), Mm(pdf_y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    }
}

fn vertical_line_ops(page_height_mm: f64, top_mm: f64, bottom_mm: f64, x_mm: f64) -> Op {
    let pdf_top = flip_y(page_height_mm, top_mm) as f32;
    let pdf_bottom = flip_y(page_height_mm, bottom_mm) as f32;
    Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x_mm as f32), Mm(pdf_top)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x_mm as f32), Mm(pdf_bottom)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    }
}

/// A placeholder Corner Marker: a black square with an inset white square, giving a bordered
/// "there is a marker here" shape without claiming to be a decodable AprilTag. See module docs.
fn marker_ops(page_height_mm: f64, marker_rect: &PhysicalRect) -> Vec<Op> {
    let border_fraction = 0.15;
    let inset = marker_rect.size.width_mm.min(marker_rect.size.height_mm) * border_fraction;
    let inner = PhysicalRect::new(
        marker_rect.left() + inset,
        marker_rect.top() + inset,
        marker_rect.size.width_mm - 2.0 * inset,
        marker_rect.size.height_mm - 2.0 * inset,
    );
    let mut ops = filled_rect_ops(page_height_mm, marker_rect, black());
    ops.extend(filled_rect_ops(page_height_mm, &inner, white()));
    ops
}

/// Renders `payload` as a QR code filling `qr_rect`, one filled vector square per dark module —
/// never a scaled raster image — so every module renders with crisp vector edges regardless of
/// the output device's resolution (TODO 5.4 "render QR at an integral module scale").
fn qr_ops(page_height_mm: f64, qr_rect: &PhysicalRect, payload: &str) -> Result<Vec<Op>, A2dError> {
    let code = qrcode::QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)
        .map_err(|e| qr_encode_error(format!("failed to encode QR payload: {e}")))?;
    let modules = code.width();
    let module_size_mm = qr_rect.size.width_mm / modules as f64;

    let mut ops = filled_rect_ops(page_height_mm, qr_rect, white());
    let colors = code.to_colors();
    for row in 0..modules {
        for col in 0..modules {
            if colors[row * modules + col] == QrModuleColor::Dark {
                let module_rect = PhysicalRect::new(
                    qr_rect.left() + col as f64 * module_size_mm,
                    qr_rect.top() + row as f64 * module_size_mm,
                    module_size_mm,
                    module_size_mm,
                );
                ops.extend(filled_rect_ops(page_height_mm, &module_rect, black()));
            }
        }
    }
    Ok(ops)
}

/// Maximum number of procedural ruling elements one page may generate. This prevents a finite
/// but pathologically small spacing value from exhausting memory or making PDF generation
/// effectively unbounded.
const MAX_RULING_ELEMENTS: usize = 100_000;

fn ruling_limit_error(style: &str, requested: usize) -> A2dError {
    A2dError::new(
        ErrorCode::new("PDF_CONTENT_STYLE_ELEMENT_LIMIT_EXCEEDED"),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.pdf.content_style_too_dense",
        format!(
            "{style} ruling would create {requested} elements, exceeding the defensive limit of {MAX_RULING_ELEMENTS}"
        ),
        false,
    )
    .with_detail("style", style)
    .with_detail("requested_elements", requested.to_string())
    .with_detail("maximum_elements", MAX_RULING_ELEMENTS.to_string())
}

fn interior_step_count(length_mm: f64, spacing_mm: f64) -> usize {
    ((length_mm / spacing_mm).ceil() - 1.0).max(0.0) as usize
}

/// Draws whatever `style` calls for inside `content_rect` (TODO 5.4 "render line/grid styles
/// deterministically" — same inputs always produce the same ops, no randomness, no
/// device-dependent layout).
///
/// The style is validated again here even when the layout builder already validated it. This is
/// intentional defense in depth: `PageLayout` is a public value and a caller can construct or
/// mutate one without calling `PageLayout::validate`. Integer-bounded iteration avoids the
/// non-progressing floating-point loops that previously allowed zero/negative spacing to hang PDF
/// generation, and the element ceiling prevents tiny positive spacing from exhausting memory.
fn content_style_ops(
    page_height_mm: f64,
    content_rect: &PhysicalRect,
    style: ContentStyle,
) -> Result<Vec<Op>, A2dError> {
    style.validate()?;

    let width_mm = content_rect.size.width_mm;
    let height_mm = content_rect.size.height_mm;
    let mut ops = match style {
        ContentStyle::Blank => return Ok(Vec::new()),
        ContentStyle::Lined { line_spacing_mm } => {
            let rows = interior_step_count(height_mm, line_spacing_mm);
            if rows > MAX_RULING_ELEMENTS {
                return Err(ruling_limit_error("lined", rows));
            }
            (1..=rows)
                .map(|row| {
                    horizontal_line_ops(
                        page_height_mm,
                        content_rect.left(),
                        content_rect.right(),
                        content_rect.top() + row as f64 * line_spacing_mm,
                    )
                })
                .collect()
        }
        ContentStyle::Graph { spacing_mm } => {
            let rows = interior_step_count(height_mm, spacing_mm);
            let columns = interior_step_count(width_mm, spacing_mm);
            let total = rows.saturating_add(columns);
            if total > MAX_RULING_ELEMENTS {
                return Err(ruling_limit_error("graph", total));
            }
            let mut lines = Vec::with_capacity(total);
            lines.extend((1..=rows).map(|row| {
                horizontal_line_ops(
                    page_height_mm,
                    content_rect.left(),
                    content_rect.right(),
                    content_rect.top() + row as f64 * spacing_mm,
                )
            }));
            lines.extend((1..=columns).map(|column| {
                vertical_line_ops(
                    page_height_mm,
                    content_rect.top(),
                    content_rect.bottom(),
                    content_rect.left() + column as f64 * spacing_mm,
                )
            }));
            lines
        }
        ContentStyle::DotGrid { spacing_mm } => {
            let rows = interior_step_count(height_mm, spacing_mm);
            let columns = interior_step_count(width_mm, spacing_mm);
            let total = rows.saturating_mul(columns);
            if total > MAX_RULING_ELEMENTS {
                return Err(ruling_limit_error("dot-grid", total));
            }

            // Rendered as small filled squares rather than true circles -- a deliberate
            // simplification for this first pass; revisit at Milestone 17's physical print
            // validation if a rounder dot matters visually.
            let dot_size_mm = 0.4;
            let mut dots = Vec::with_capacity(total.saturating_mul(4));
            for row in 1..=rows {
                let y = content_rect.top() + row as f64 * spacing_mm;
                for column in 1..=columns {
                    let x = content_rect.left() + column as f64 * spacing_mm;
                    let dot_rect = PhysicalRect::new(
                        x - dot_size_mm / 2.0,
                        y - dot_size_mm / 2.0,
                        dot_size_mm,
                        dot_size_mm,
                    );
                    dots.extend(filled_rect_ops(page_height_mm, &dot_rect, ruling_gray()));
                }
            }
            return Ok(dots);
        }
    };
    let mut wrapped = vec![
        Op::SaveGraphicsState,
        Op::SetOutlineColor { col: ruling_gray() },
        Op::SetOutlineThickness { pt: Pt(0.5) },
    ];
    wrapped.append(&mut ops);
    wrapped.push(Op::RestoreGraphicsState);
    Ok(wrapped)
}

fn calibration_ops(page_height_mm: f64, mark: &CalibrationMark) -> Vec<Op> {
    let y_mid = mark.rect.top() + mark.rect.size.height_mm / 2.0;
    vec![
        Op::SaveGraphicsState,
        Op::SetOutlineColor { col: black() },
        Op::SetOutlineThickness { pt: Pt(0.75) },
        horizontal_line_ops(
            page_height_mm,
            mark.rect.left(),
            mark.rect.left() + mark.reference_length_mm,
            y_mid,
        ),
        Op::RestoreGraphicsState,
    ]
}

fn page_number_ops(page_height_mm: f64, rect: &PhysicalRect, number: u32) -> Vec<Op> {
    let font_size = mm_to_pt(rect.size.height_mm * 0.8);
    let baseline_y_mm = rect.bottom() - rect.size.height_mm * 0.2;
    let baseline_pdf_y = flip_y(page_height_mm, baseline_y_mm) as f32;
    vec![
        Op::SaveGraphicsState,
        Op::SetFillColor { col: black() },
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: font_size,
        },
        Op::SetTextCursor {
            pos: Point::new(Mm(rect.left() as f32), Mm(baseline_pdf_y)),
        },
        Op::ShowText {
            items: vec![TextItem::Text(number.to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]
}

/// Renders one full page's content stream: markers, QR (encoding `qr_payload`), the content
/// style's ruling, the calibration mark, and the visible page number if the layout has a slot
/// for one and the caller supplied a number.
pub fn render_page_ops(
    layout: &PageLayout,
    qr_payload: &str,
    visible_page_number: Option<u32>,
) -> Result<Vec<Op>, A2dError> {
    let h = layout.physical_size.height_mm;
    // Validate and bound content-style work before doing any other rendering. This ensures a
    // malformed hand-constructed layout fails immediately rather than allocating QR/marker ops
    // first or entering an unbounded ruling loop.
    let content_ops = content_style_ops(h, &layout.content_rect, layout.content_style)?;
    let mut ops = Vec::new();
    for marker in &layout.markers {
        ops.extend(marker_ops(h, &marker.rect));
    }
    ops.extend(qr_ops(h, &layout.qr_rect, qr_payload)?);
    ops.extend(content_ops);
    ops.extend(calibration_ops(h, &layout.calibration));
    if let (Some(rect), Some(number)) = (layout.visible_page_number_rect, visible_page_number) {
        ops.extend(page_number_ops(h, &rect, number));
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2d_layout::page_layout::MarkerRole;
    use a2d_layout::smart_page::{PaperSize, SmartPageStyle, smart_page_layout};

    fn count_polygons(ops: &[Op]) -> usize {
        ops.iter()
            .filter(|op| matches!(op, Op::DrawPolygon { .. }))
            .count()
    }

    #[test]
    fn render_page_ops_draws_a_filled_polygon_pair_for_every_marker() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let ops = render_page_ops(&layout, "A2D:1:S:X:1234567", None).unwrap();
        // Each marker is an outer + inner square (2 polygons), 4 markers -> 8, plus the QR's
        // white backing square -- at least that many even before counting dark QR modules.
        assert!(
            count_polygons(&ops) > layout.markers.len() * 2,
            "expected at least one polygon pair per marker plus the QR backing"
        );
    }

    #[test]
    fn render_page_ops_renders_the_qr_as_many_small_filled_squares() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let payload = a2d_identity::qr::PageCode::NotebookSetup {
            design_id: a2d_domain::NotebookDesignId::generate(),
        }
        .encode()
        .unwrap();
        let code =
            qrcode::QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M).unwrap();
        let dark_modules = code
            .to_colors()
            .iter()
            .filter(|c| **c == QrModuleColor::Dark)
            .count();

        let ops = render_page_ops(&layout, &payload, None).unwrap();
        // 8 marker polygons + 1 QR backing + one polygon per dark module.
        assert_eq!(count_polygons(&ops), 8 + 1 + dark_modules);
    }

    #[test]
    fn qr_encoding_failure_surfaces_as_a_typed_validation_error() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        // Far beyond QR's absolute maximum alphanumeric capacity (~4296 chars at version 40).
        let oversized_payload = "A".repeat(10_000);
        let err = render_page_ops(&layout, &oversized_payload, None).unwrap_err();
        assert_eq!(err.category, a2d_domain::ErrorCategory::Validation);
        assert!(err.code.to_string().contains("QR_ENCODE_FAILED"));
    }

    #[test]
    fn visible_page_number_is_only_rendered_when_the_layout_has_a_slot_and_a_number_is_given() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        assert!(layout.visible_page_number_rect.is_some());

        let with_number = render_page_ops(&layout, "A2D:1:S:X:1234567", Some(3)).unwrap();
        let without_number = render_page_ops(&layout, "A2D:1:S:X:1234567", None).unwrap();
        assert!(
            with_number
                .iter()
                .any(|op| matches!(op, Op::ShowText { .. }))
        );
        assert!(
            !without_number
                .iter()
                .any(|op| matches!(op, Op::ShowText { .. }))
        );
    }

    #[test]
    fn blank_style_draws_no_ruling_but_lined_style_does() {
        let blank = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let lined = smart_page_layout(PaperSize::A4, SmartPageStyle::Lined);
        let blank_ops = render_page_ops(&blank, "A2D:1:S:X:1234567", None).unwrap();
        let lined_ops = render_page_ops(&lined, "A2D:1:S:X:1234567", None).unwrap();
        let blank_lines = blank_ops
            .iter()
            .filter(|op| matches!(op, Op::DrawLine { .. }))
            .count();
        let lined_lines = lined_ops
            .iter()
            .filter(|op| matches!(op, Op::DrawLine { .. }))
            .count();
        // The calibration mark itself is one line even for Blank, so Lined must draw more.
        assert!(lined_lines > blank_lines);
    }

    #[test]
    fn every_marker_role_gets_a_placeholder_shape() {
        let layout = smart_page_layout(PaperSize::UsLetter, SmartPageStyle::Blank);
        let roles: std::collections::HashSet<MarkerRole> =
            layout.markers.iter().map(|m| m.role).collect();
        assert_eq!(roles.len(), 4);
    }

    #[test]
    fn renderer_rejects_invalid_spacing_even_when_layout_validation_is_bypassed() {
        for spacing in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
            layout.content_style = ContentStyle::Lined {
                line_spacing_mm: spacing,
            };
            let err = render_page_ops(&layout, "A2D:1:S:X:1234567", None).unwrap_err();
            assert_eq!(err.code.to_string(), "LAYOUT_CONTENT_STYLE_SPACING_INVALID");
        }
    }

    #[test]
    fn renderer_rejects_pathologically_dense_ruling_before_allocating_it() {
        let mut layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        layout.content_style = ContentStyle::DotGrid { spacing_mm: 0.001 };
        let err = render_page_ops(&layout, "A2D:1:S:X:1234567", None).unwrap_err();
        assert_eq!(
            err.code.to_string(),
            "PDF_CONTENT_STYLE_ELEMENT_LIMIT_EXCEEDED"
        );
        assert!(err.details.contains_key("requested_elements"));
    }
}
