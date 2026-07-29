//! Renders one page's content stream from a resolved [`PageLayout`] (TODO 5.4). Every element is
//! vector drawing (filled polygons, stroked lines, and standard-font text) — nothing here embeds
//! a raster image or an external font file, so there is no interpolation blur to worry about and
//! no font-licensing question to answer (spec §12.3's "legally distributable fonts" requirement
//! is satisfied by construction: `BuiltinFont` uses the 14 standard PDF fonts, which every PDF
//! viewer/printer already has and which require no embedded font program).
//!
//! Corner Markers are vector renderings of official `tagStandard41h12` tags. The marker pixels
//! come through `a2d-image`'s reviewed native ownership boundary and are immediately converted to
//! PDF vector rectangles; native pointers and raster interpolation never cross into this crate.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_image::{AprilTagDetector, DetectorConfig, RenderedTag};
use a2d_layout::geometry::PhysicalRect;
use a2d_layout::marker_id_for_role;
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

/// Converts an official marker image into vector PDF rectangles. The white backing guarantees a
/// deterministic marker field; each dark source pixel becomes one crisp vector module.
fn marker_ops(page_height_mm: f64, marker_rect: &PhysicalRect, rendered: &RenderedTag) -> Vec<Op> {
    let module_width_mm = marker_rect.size.width_mm / rendered.width() as f64;
    let module_height_mm = marker_rect.size.height_mm / rendered.height() as f64;
    let mut ops = filled_rect_ops(page_height_mm, marker_rect, white());

    for row in 0..rendered.height() {
        for column in 0..rendered.width() {
            let value = rendered
                .pixel(column, row)
                .expect("loop coordinates are bounded by the rendered marker dimensions");
            if value >= 128 {
                continue;
            }
            let module_rect = PhysicalRect::new(
                marker_rect.left() + column as f64 * module_width_mm,
                marker_rect.top() + row as f64 * module_height_mm,
                module_width_mm,
                module_height_mm,
            );
            ops.extend(filled_rect_ops(page_height_mm, &module_rect, black()));
        }
    }
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

fn qr_label_ops(page_height_mm: f64, rect: &PhysicalRect, label: &str) -> Vec<Op> {
    let font_size = Pt(7.0);
    let baseline_y_mm = rect.bottom() + 3.0;
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
            items: vec![TextItem::Text(label.to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]
}

pub(crate) fn render_page_ops(
    layout: &PageLayout,
    qr_payload: &str,
    visible_page_number: Option<u32>,
) -> Result<Vec<Op>, A2dError> {
    layout.validate()?;
    let mut detector = AprilTagDetector::new(DetectorConfig::default())?;
    let page_rect = PhysicalRect::new(
        0.0,
        0.0,
        layout.physical_size.width_mm,
        layout.physical_size.height_mm,
    );
    let mut ops = filled_rect_ops(layout.physical_size.height_mm, &page_rect, white());

    for placement in &layout.markers {
        let marker_id = marker_id_for_role(placement.role);
        let rendered = detector.render_tag(marker_id)?;
        ops.extend(marker_ops(
            layout.physical_size.height_mm,
            &placement.rect,
            &rendered,
        ));
    }

    ops.extend(qr_ops(
        layout.physical_size.height_mm,
        &layout.qr_rect,
        qr_payload,
    )?);
    ops.extend(qr_label_ops(
        layout.physical_size.height_mm,
        &layout.qr_rect,
        qr_payload,
    ));
    ops.extend(content_style_ops(
        layout.physical_size.height_mm,
        &layout.content_rect,
        layout.content_style,
    )?);
    ops.extend(calibration_ops(
        layout.physical_size.height_mm,
        &layout.calibration,
    ));
    if let (Some(rect), Some(number)) = (&layout.visible_page_number_rect, visible_page_number) {
        ops.extend(page_number_ops(
            layout.physical_size.height_mm,
            rect,
            number,
        ));
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use a2d_layout::smart_page::{PaperSize, SmartPageStyle, smart_page_layout};

    use super::*;

    #[test]
    fn invalid_content_style_is_rejected_before_iteration() {
        let mut layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        layout.content_style = ContentStyle::Lined {
            line_spacing_mm: 0.0,
        };
        let error = render_page_ops(&layout, "A2D:1:M:test", None).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "LAYOUT_CONTENT_STYLE_SPACING_INVALID"
        );
    }

    #[test]
    fn pathologically_dense_content_style_is_bounded() {
        let mut layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        layout.content_style = ContentStyle::Graph { spacing_mm: 0.0001 };
        let error = render_page_ops(&layout, "A2D:1:M:test", None).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "PDF_CONTENT_STYLE_ELEMENT_LIMIT_EXCEEDED"
        );
    }
}
