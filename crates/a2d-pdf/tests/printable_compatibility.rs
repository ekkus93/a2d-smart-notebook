use std::collections::BTreeSet;

use a2d_domain::{LayoutId, PageSetId, SmartPageId};
use a2d_identity::qr::PageCode;
use a2d_image::{
    AprilTagDetector, DetectorConfig, GrayFrame, ImageLimits, ImageRotation, MarkerIdLayout,
    PageOrientation, resolve_page_markers,
};
use a2d_layout::MarkerRole;
use a2d_layout::smart_page::{PaperSize, SmartPageStyle, smart_page_layout};
use a2d_pdf::render_page_ops;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings, render};
use printpdf::{Mm, PdfDocument, PdfPage, PdfSaveOptions};

const DPI: f64 = 300.0;
const MAX_LAYOUT_ID: &str = "ABCDEFGHIJKLMNOPQRST";

fn printable_pdf() -> (Vec<u8>, String, a2d_layout::PageLayout) {
    let mut layout = smart_page_layout(PaperSize::UsLetter, SmartPageStyle::Graph);
    layout.id = LayoutId::parse(MAX_LAYOUT_ID).unwrap();
    let payload = PageCode::SmartPage {
        smart_page_id: SmartPageId::parse("00000000000000000000000002").unwrap(),
        layout_id: layout.id.clone(),
        visible_page_number: Some(999_999),
        page_set_id: Some(PageSetId::parse("00000000000000000000000003").unwrap()),
    }
    .encode()
    .unwrap();
    let ops = render_page_ops(&layout, &payload, Some(999_999)).unwrap();
    let page = PdfPage::new(
        Mm(layout.physical_size.width_mm as f32),
        Mm(layout.physical_size.height_mm as f32),
        ops,
    );
    let mut document = PdfDocument::new("A2D Printable Compatibility Fixture");
    document.with_pages(vec![page]);
    let mut warnings = Vec::new();
    let bytes = document.save(&PdfSaveOptions::default(), &mut warnings);
    assert!(warnings.is_empty(), "PDF save warnings: {warnings:?}");
    (bytes, payload, layout)
}

fn rasterize(pdf_bytes: &[u8], print_scale: f64) -> image::GrayImage {
    let pdf = Pdf::new(pdf_bytes.to_vec()).unwrap();
    let page = pdf.pages().iter().next().unwrap();
    let render_scale = (DPI / 72.0 * print_scale) as f32;
    let settings = RenderSettings {
        x_scale: render_scale,
        y_scale: render_scale,
        bg_color: WHITE,
        ..Default::default()
    };
    let pixmap = render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &settings,
    );
    image::load_from_memory(&pixmap.into_png().unwrap())
        .unwrap()
        .to_luma8()
}

fn decode_qr(image: &image::GrayImage) -> String {
    let mut prepared = rqrr::PreparedImage::prepare(image.clone());
    let grids = prepared.detect_grids();
    assert_eq!(
        grids.len(),
        1,
        "full printable page must contain exactly one QR"
    );
    grids[0].decode().unwrap().1
}

fn marker_role_for_id(id: u32) -> MarkerRole {
    match id {
        0 => MarkerRole::TopLeft,
        1 => MarkerRole::TopRight,
        2 => MarkerRole::BottomRight,
        3 => MarkerRole::BottomLeft,
        other => panic!("unexpected marker ID {other}"),
    }
}

#[test]
fn worst_case_page_code_and_all_markers_survive_real_layout_rasterization_at_print_scales() {
    let (pdf_bytes, payload, layout) = printable_pdf();

    for print_scale in [0.95, 1.0, 1.05] {
        let image = rasterize(&pdf_bytes, print_scale);
        assert_eq!(decode_qr(&image), payload, "print scale {print_scale}");

        let bytes = image.as_raw();
        let frame = GrayFrame::new(
            image.width(),
            image.height(),
            image.width() as usize,
            ImageRotation::Degrees0,
            bytes,
            ImageLimits::new(bytes.len() as u64).unwrap(),
        )
        .unwrap();
        let mut detector = AprilTagDetector::new(DetectorConfig::default()).unwrap();
        let detections = detector.detect(frame).unwrap();
        let ids: BTreeSet<u32> = detections.iter().map(|detection| detection.id).collect();
        assert_eq!(
            ids,
            BTreeSet::from([0, 1, 2, 3]),
            "print scale {print_scale}"
        );

        let id_layout = MarkerIdLayout::new([
            (0, MarkerRole::TopLeft),
            (1, MarkerRole::TopRight),
            (2, MarkerRole::BottomRight),
            (3, MarkerRole::BottomLeft),
        ])
        .unwrap();
        let resolved = resolve_page_markers(&detections, &id_layout).unwrap();
        assert_eq!(resolved.orientation, PageOrientation::Degrees0);
        assert!(resolved.unexpected_tag_ids.is_empty());

        let pixels_per_mm = DPI * print_scale / 25.4;
        let tolerance_px = 2.0 * pixels_per_mm;
        for detection in &detections {
            let role = marker_role_for_id(detection.id);
            let marker = layout
                .markers
                .iter()
                .find(|marker| marker.role == role)
                .unwrap();
            let expected_x = (marker.rect.left() + marker.rect.size.width_mm / 2.0) * pixels_per_mm;
            let expected_y = (marker.rect.top() + marker.rect.size.height_mm / 2.0) * pixels_per_mm;
            let dx = detection.center.x - expected_x;
            let dy = detection.center.y - expected_y;
            let distance = (dx * dx + dy * dy).sqrt();
            assert!(
                distance <= tolerance_px,
                "marker {:?} center drifted {distance:.2}px at print scale {print_scale}",
                role
            );
        }
    }
}
