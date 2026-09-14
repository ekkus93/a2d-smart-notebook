use a2d_domain::{
    Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRun, OcrRunId,
    OcrUnavailableReason, Page, PageId, PageKind, PageState, Provenance, QualityStatus, Scan,
    ScanId, SmartPageId, TextRegion, TextRegionId,
};
use a2d_storage::{
    AssetRepository, OcrRunRepository, PageRepository, ScanRepository, Storage,
    TextRegionRepository,
};

struct ScanFixture {
    scan_id: ScanId,
    page_id: PageId,
    original_asset_id: AssetId,
}

fn provenance(page_id: &PageId, scan_id: &ScanId) -> Provenance {
    Provenance {
        source_page_id: Some(page_id.clone()),
        source_scan_id: Some(scan_id.clone()),
        producing_component: "test-ocr-provider".to_string(),
        component_version: "1.0.0".to_string(),
        created_at_ms: 200,
        warnings: Vec::new(),
        user_approved: None,
    }
}

fn original_asset(id: AssetId) -> Asset {
    Asset::new(
        id.clone(),
        AssetKind::Original,
        format!("assets/originals/{id}.png"),
        "image/png".to_string(),
        1_024,
        "test-sha256".to_string(),
        100,
        true,
        EncryptionState::Plaintext,
    )
}

fn scan_fingerprint() -> String {
    format!(
        "scan-content-v1;corrected-sha256={};perceptual=mean-grid-16x24-v1:{}",
        "a".repeat(64),
        "b".repeat(16 * 24 * 2)
    )
}

fn insert_scan_fixture(storage: &Storage) -> ScanFixture {
    let page_id = PageId::generate();
    let original_asset_id = AssetId::generate();
    let scan_id = ScanId::generate();
    let page = Page::new(
        page_id.clone(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: Some(1),
        },
        LayoutId::parse("PAGE").unwrap(),
        Some("OCR text-region page".to_string()),
        PageState::GeneratedNotScanned,
        100,
    );
    let scan = Scan::new(
        scan_id.clone(),
        page_id.clone(),
        None,
        CaptureSource::Camera,
        125,
        original_asset_id.clone(),
        None,
        None,
        None,
        "test-pipeline".to_string(),
        QualityStatus::Accepted,
        Vec::new(),
        true,
        None,
        scan_fingerprint(),
    );

    storage.insert_page(&page).unwrap();
    storage
        .insert_asset(&original_asset(original_asset_id.clone()))
        .unwrap();
    storage.insert_scan(&scan).unwrap();

    ScanFixture {
        scan_id,
        page_id,
        original_asset_id,
    }
}

fn insert_detected_run(storage: &Storage, fixture: &ScanFixture) -> OcrRunId {
    let run = OcrRun::detected(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        Some("latin-v1".to_string()),
        "first line\nsecond line".to_string(),
        Some(250),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    let run_id = run.id().clone();
    storage.insert_ocr_run(&run).unwrap();
    run_id
}

fn region(ocr_run_id: OcrRunId, text: &str, created_at_ms: i64) -> TextRegion {
    TextRegion::new(
        TextRegionId::generate(),
        ocr_run_id,
        vec![(0.0, 0.0), (120.0, 0.0), (120.0, 32.0), (0.0, 32.0)],
        text.to_string(),
        Some(0.85),
        created_at_ms,
    )
    .unwrap()
}

#[test]
fn text_region_round_trips_and_lists_for_detected_ocr_run() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);
    let run_id = insert_detected_run(&storage, &fixture);
    let first = region(run_id.clone(), "first line", 300);
    let second = region(run_id.clone(), "second line", 301);
    let first_id = first.id().clone();

    storage.insert_text_region(&second).unwrap();
    storage.insert_text_region(&first).unwrap();

    let loaded = storage.get_text_region(&first_id).unwrap().unwrap();
    assert_eq!(loaded.ocr_run_id, run_id);
    assert_eq!(loaded.text, "first line");
    assert_eq!(loaded.confidence, Some(0.85));
    assert_eq!(
        loaded.polygon,
        vec![(0.0, 0.0), (120.0, 0.0), (120.0, 32.0), (0.0, 32.0)]
    );

    let listed = storage
        .list_text_regions_for_ocr_run(&loaded.ocr_run_id)
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].text, "first line");
    assert_eq!(listed[1].text, "second line");
}

#[test]
fn text_region_constructor_rejects_invalid_geometry_confidence_and_text() {
    let ocr_run_id = OcrRunId::generate();
    let invalid_polygon = TextRegion::new(
        TextRegionId::generate(),
        ocr_run_id.clone(),
        vec![(0.0, 0.0), (10.0, 10.0)],
        "text".to_string(),
        Some(0.5),
        300,
    )
    .unwrap_err();
    assert_eq!(
        invalid_polygon.code.to_string(),
        "TEXT_REGION_POLYGON_POINT_COUNT_INVALID"
    );

    let invalid_confidence = TextRegion::new(
        TextRegionId::generate(),
        ocr_run_id.clone(),
        vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
        "text".to_string(),
        Some(1.5),
        300,
    )
    .unwrap_err();
    assert_eq!(
        invalid_confidence.code.to_string(),
        "TEXT_REGION_CONFIDENCE_INVALID"
    );

    let empty_text = TextRegion::new(
        TextRegionId::generate(),
        ocr_run_id,
        vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
        String::new(),
        Some(0.5),
        300,
    )
    .unwrap_err();
    assert_eq!(empty_text.code.to_string(), "TEXT_REGION_TEXT_EMPTY");
}

#[test]
fn text_regions_cannot_attach_to_no_text_or_unavailable_ocr_runs() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);
    let no_text = OcrRun::no_text_detected(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        None,
        Some(260),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    let unavailable = OcrRun::unavailable(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        None,
        OcrUnavailableReason::ProviderFailed,
        Some("provider failed".to_string()),
        Some(270),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    storage.insert_ocr_run(&no_text).unwrap();
    storage.insert_ocr_run(&unavailable).unwrap();

    let no_text_error = storage
        .insert_text_region(&region(no_text.id().clone(), "fabricated", 300))
        .unwrap_err();
    let unavailable_error = storage
        .insert_text_region(&region(unavailable.id().clone(), "fabricated", 301))
        .unwrap_err();

    assert_eq!(
        no_text_error.code.to_string(),
        "STORAGE_TEXT_REGION_OCR_RUN_NOT_DETECTED"
    );
    assert_eq!(
        unavailable_error.code.to_string(),
        "STORAGE_TEXT_REGION_OCR_RUN_NOT_DETECTED"
    );
    assert!(
        storage
            .list_text_regions_for_ocr_run(no_text.id())
            .unwrap()
            .is_empty()
    );
    assert!(
        storage
            .list_text_regions_for_ocr_run(unavailable.id())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn missing_text_region_reads_as_none() {
    let storage = Storage::open_in_memory().unwrap();
    assert!(
        storage
            .get_text_region(&TextRegionId::generate())
            .unwrap()
            .is_none()
    );
}
