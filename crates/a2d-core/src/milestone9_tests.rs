use std::io::Cursor;
use std::path::{Path, PathBuf};

use a2d_domain::{
    CaptureSource, LayoutId, Notebook, NotebookDesign, NotebookDesignId, NotebookId, Page, PageId,
    PageKind, PageState, QualityStatus, ScanId, SmartPageId, TrimSizeMm, TrustState,
};
use a2d_identity::PageCode;
use a2d_image::{AprilTagDetector, DetectorConfig};
use a2d_layout::{
    MarkerRole, PageLayout, PaperSize, SmartPageStyle, smart_page_layout, writable_page_layout,
};
use a2d_storage::{NotebookDesignRepository, NotebookRepository, PageRepository, ScanRepository};
use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, RgbImage};

use super::*;

const DESIGN_ID: &str = "00000000000000000000000001";

fn test_core() -> (std::sync::Arc<A2dCore>, PathBuf, PageId, NotebookId, String) {
    let root = std::env::temp_dir().join(format!("a2d-m9-test-{}", PageId::generate()));
    let core = A2dCore::open(crate::OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    let layout_id = LayoutId::parse(writable_page_layout().id.as_str()).unwrap();
    let design_id = NotebookDesignId::parse(DESIGN_ID).unwrap();
    let design = NotebookDesign::new(
        design_id.clone(),
        1,
        "Test Design".to_string(),
        1,
        TrimSizeMm {
            width: 152,
            height: 229,
        },
        100,
        layout_id.clone(),
        layout_id.clone(),
        "apriltag-placeholder".to_string(),
        vec![
            "TL".to_string(),
            "TR".to_string(),
            "BL".to_string(),
            "BR".to_string(),
        ],
        "test-manifest".to_string(),
        TrustState::Trusted,
    );
    let notebook_id = NotebookId::generate();
    let notebook = Notebook::new(
        notebook_id.clone(),
        design_id.clone(),
        "Test Notebook".to_string(),
        1,
        1,
        None,
        true,
        None,
        None,
        None,
    );
    let page_id = PageId::generate();
    let page = Page::new(
        page_id.clone(),
        PageKind::NotebookPage {
            notebook_id: notebook_id.clone(),
            design_id: design_id.clone(),
            logical_page_number: 42,
        },
        layout_id.clone(),
        None,
        PageState::Unscanned,
        1,
    );
    core.lock_storage()
        .unwrap()
        .transaction(|tx| {
            tx.insert_notebook_design(&design)?;
            tx.insert_notebook(&notebook)?;
            tx.insert_page(&page)?;
            Ok(())
        })
        .unwrap();
    let payload = PageCode::NotebookPage {
        design_id,
        logical_page_number: 42,
        layout_id,
    }
    .encode()
    .unwrap();
    (core, root, page_id, notebook_id, payload)
}

fn rendered_layout_page(layout: &PageLayout) -> Vec<u8> {
    const PAGE_WIDTH: u32 = 1_216;
    const BORDER: u32 = 64;
    let page_height = (f64::from(PAGE_WIDTH) * layout.physical_size.height_mm
        / layout.physical_size.width_mm)
        .round() as u32;
    let source_width = PAGE_WIDTH + 2 * BORDER;
    let source_height = page_height + 2 * BORDER;
    let mut image = RgbImage::from_pixel(source_width, source_height, Rgb([45, 48, 56]));
    for y in BORDER..BORDER + page_height {
        for x in BORDER..BORDER + PAGE_WIDTH {
            image.put_pixel(x, y, Rgb([250, 249, 246]));
        }
    }
    let detector = AprilTagDetector::new(DetectorConfig::default()).unwrap();
    for (role, id) in [
        (MarkerRole::TopLeft, 0),
        (MarkerRole::TopRight, 1),
        (MarkerRole::BottomRight, 2),
        (MarkerRole::BottomLeft, 3),
    ] {
        let placement = layout
            .markers
            .iter()
            .find(|placement| placement.role == role)
            .unwrap();
        let tag = detector.render_tag(id).unwrap();
        let left = BORDER
            + (placement.rect.left() / layout.physical_size.width_mm * f64::from(PAGE_WIDTH - 1))
                .round() as u32;
        let top = BORDER
            + (placement.rect.top() / layout.physical_size.height_mm * f64::from(page_height - 1))
                .round() as u32;
        let target_width = (placement.rect.size.width_mm / layout.physical_size.width_mm
            * f64::from(PAGE_WIDTH - 1))
        .round() as u32;
        let target_height = (placement.rect.size.height_mm / layout.physical_size.height_mm
            * f64::from(page_height - 1))
        .round() as u32;
        for y in 0..target_height {
            let source_y = y as usize * tag.height() / target_height as usize;
            for x in 0..target_width {
                let source_x = x as usize * tag.width() / target_width as usize;
                let value = tag.pixel(source_x, source_y).unwrap();
                image.put_pixel(left + x, top + y, Rgb([value, value, value]));
            }
        }
    }
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut output, ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn production_layout_page() -> Vec<u8> {
    rendered_layout_page(&writable_page_layout())
}

fn approved_markers() -> Vec<RegistrationMarker> {
    vec![
        RegistrationMarker {
            role: "TL".to_string(),
            id: 0,
        },
        RegistrationMarker {
            role: "TR".to_string(),
            id: 1,
        },
        RegistrationMarker {
            role: "BR".to_string(),
            id: 2,
        },
        RegistrationMarker {
            role: "BL".to_string(),
            id: 3,
        },
    ]
}

fn request(
    root: &Path,
    page_id: PageId,
    notebook_id: NotebookId,
    payload: String,
    name: &str,
) -> RegisterScanRequest {
    let staging = root.join("tmp").join(SCANNER_STAGING_DIRECTORY);
    std::fs::create_dir_all(&staging).unwrap();
    let path = staging.join(name);
    std::fs::write(&path, production_layout_page()).unwrap();
    RegisterScanRequest {
        staging_path: path.to_string_lossy().into_owned(),
        page_code_payload: payload,
        expected_page_id: page_id,
        active_notebook_id: Some(notebook_id),
        capture_source: CaptureSource::Camera,
        image_format: ScanImageFormat::Png,
        image_rotation: ScanImageRotation::Degrees0,
        captured_at_ms: 1_000,
        observed_markers: approved_markers(),
        preview_warnings: Vec::new(),
        user_approved: true,
    }
}

#[test]
fn first_registration_commits_assets_scan_and_preferred_page_atomically() {
    let (core, root, page_id, notebook_id, payload) = test_core();
    let request = request(&root, page_id.clone(), notebook_id, payload, "first.png");
    let staging = PathBuf::from(&request.staging_path);
    let registered = core.register_scan(request).unwrap();
    assert!(registered.preferred);
    assert!(matches!(
        registered.quality_status,
        QualityStatus::Accepted | QualityStatus::AcceptedWithWarnings
    ));
    assert!(!staging.exists());
    let storage = core.lock_storage().unwrap();
    let page = storage.get_page(&page_id).unwrap().unwrap();
    assert_eq!(page.state, PageState::Scanned);
    assert_eq!(page.preferred_scan_id.as_ref(), Some(&registered.scan_id));
    let scan = storage.get_scan(&registered.scan_id).unwrap().unwrap();
    assert!(scan.preferred);
    assert!(
        scan.pipeline_version
            .starts_with("image-pipeline-v1;scan-policy-v1;layout=notebook-design:")
    );
    assert!(
        scan.pipeline_version
            .ends_with(";marker-family=tagStandard41h12")
    );
    assert!(
        scan.content_fingerprint
            .starts_with("scan-content-v1;corrected-sha256=")
    );
    assert!(
        scan.content_fingerprint
            .contains(";perceptual=mean-grid-16x24-v1:")
    );
    for path in [
        registered.original_path,
        registered.corrected_path,
        registered.ocr_path,
        registered.thumbnail_path,
    ] {
        assert!(Path::new(&path).is_file());
    }
    drop(storage);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a4_smart_page_registration_uses_a4_rectification_dimensions() {
    let root = std::env::temp_dir().join(format!("a2d-m9-smart-{}", PageId::generate()));
    let core = A2dCore::open(crate::OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
    let smart_page_id = SmartPageId::generate();
    let page_id = PageId::generate();
    let page = Page::new(
        page_id.clone(),
        PageKind::SmartPage {
            smart_page_id: smart_page_id.clone(),
            page_set_id: None,
            visible_page_number: Some(7),
        },
        layout.id.clone(),
        None,
        PageState::GeneratedNotScanned,
        1,
    );
    core.lock_storage().unwrap().insert_page(&page).unwrap();
    let payload = PageCode::SmartPage {
        smart_page_id,
        layout_id: layout.id.clone(),
        visible_page_number: Some(7),
        page_set_id: None,
    }
    .encode()
    .unwrap();
    let staging_root = root.join("tmp").join(SCANNER_STAGING_DIRECTORY);
    std::fs::create_dir_all(&staging_root).unwrap();
    let staging_path = staging_root.join("smart-a4.png");
    std::fs::write(&staging_path, rendered_layout_page(&layout)).unwrap();

    let registered = core
        .register_scan(RegisterScanRequest {
            staging_path: staging_path.to_string_lossy().into_owned(),
            page_code_payload: payload,
            expected_page_id: page_id,
            active_notebook_id: None,
            capture_source: CaptureSource::Camera,
            image_format: ScanImageFormat::Png,
            image_rotation: ScanImageRotation::Degrees0,
            captured_at_ms: 1_000,
            observed_markers: approved_markers(),
            preview_warnings: Vec::new(),
            user_approved: true,
        })
        .unwrap();
    let corrected_bytes = std::fs::read(&registered.corrected_path).unwrap();
    let corrected = image::load_from_memory(&corrected_bytes).unwrap();
    assert_eq!(corrected.dimensions(), (900, 1_273));
    let scan = core
        .lock_storage()
        .unwrap()
        .get_scan(&registered.scan_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        scan.pipeline_version,
        "image-pipeline-v1;scan-policy-v1;layout=SP-A4-BLANK-V1;marker-family=tagStandard41h12"
    );

    drop(core);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn rescan_preserves_preferred_original_and_requires_review() {
    let (core, root, page_id, notebook_id, payload) = test_core();
    let first = core
        .register_scan(request(
            &root,
            page_id.clone(),
            notebook_id.clone(),
            payload.clone(),
            "first.png",
        ))
        .unwrap();
    let second = core
        .register_scan(request(
            &root,
            page_id.clone(),
            notebook_id,
            payload,
            "second.png",
        ))
        .unwrap();
    assert!(!second.preferred);
    assert_eq!(second.quality_status, QualityStatus::NeedsReview);
    assert!(
        second
            .required_actions
            .contains(&RegistrationRequiredAction::ReviewExistingPage)
    );
    let page = core
        .lock_storage()
        .unwrap()
        .get_page(&page_id)
        .unwrap()
        .unwrap();
    assert_eq!(page.state, PageState::NeedsReview);
    assert_eq!(page.preferred_scan_id, Some(first.scan_id));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn staging_path_outside_library_is_rejected_without_deleting_source() {
    let (core, root, page_id, notebook_id, payload) = test_core();
    let outside = std::env::temp_dir().join(format!("outside-{}.png", PageId::generate()));
    std::fs::write(&outside, production_layout_page()).unwrap();
    let mut request = request(&root, page_id, notebook_id, payload, "inside.png");
    request.staging_path = outside.to_string_lossy().into_owned();
    let error = core.register_scan(request).unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "CORE_SCAN_STAGING_PATH_ESCAPES_LIBRARY"
    );
    assert!(outside.exists());
    std::fs::remove_file(outside).ok();
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn changed_marker_identity_is_a_hard_registration_error() {
    let (core, root, page_id, notebook_id, payload) = test_core();
    let mut request = request(&root, page_id, notebook_id, payload, "marker.png");
    request.observed_markers[0].id = 99;
    let error = core.register_scan(request).unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "CORE_SCAN_MARKERS_CHANGED_SINCE_REVIEW"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn incomplete_journal_is_retained_until_explicit_completion() {
    let root = std::env::temp_dir().join(format!("a2d-journal-test-{}", ScanId::generate()));
    std::fs::create_dir_all(root.join("tmp")).unwrap();
    let scan_id = ScanId::generate();
    let staging = root.join("tmp").join("capture.png");
    std::fs::write(&staging, b"capture").unwrap();
    let mut journal = RegistrationJournal::begin(&root, &scan_id, &staging).unwrap();
    journal.record_phase("simulated_interruption").unwrap();
    let path = journal.path.clone();
    drop(journal);
    assert!(path.is_file());
    std::fs::remove_dir_all(root).ok();
}
