use a2d_domain::{
    Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRun, OcrRunId,
    OcrRunStatus, OcrUnavailableReason, Page, PageId, PageKind, PageState, Provenance,
    QualityStatus, Scan, ScanId, SmartPageId,
};
use a2d_storage::{AssetRepository, OcrRunRepository, PageRepository, ScanRepository, Storage};

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
        warnings: vec!["provider-warning".to_string()],
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
        Some("OCR persistence page".to_string()),
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

#[test]
fn detected_ocr_result_round_trips_with_input_and_model_metadata() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);
    let run = OcrRun::detected(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        Some("latin-v1".to_string()),
        "hello notebook".to_string(),
        Some(250),
        vec!["low-confidence-line".to_string()],
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    let run_id = run.id().clone();

    storage.insert_ocr_run(&run).unwrap();
    let loaded = storage.get_ocr_run(&run_id).unwrap().unwrap();

    assert_eq!(loaded.status, OcrRunStatus::Detected);
    assert_eq!(loaded.input_asset_id, Some(fixture.original_asset_id));
    assert_eq!(loaded.model_name.as_deref(), Some("latin-v1"));
    assert_eq!(loaded.full_text, "hello notebook");
    assert_eq!(loaded.completed_at_ms, Some(250));
    assert_eq!(loaded.unavailable_reason, None);
    assert_eq!(loaded.unavailable_message, None);
    assert_eq!(loaded.warnings, vec!["low-confidence-line".to_string()]);
    assert_eq!(loaded.provenance.source_scan_id, Some(fixture.scan_id));
}

#[test]
fn no_text_ocr_result_is_explicit_not_empty_detected_text() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);
    let run = OcrRun::no_text_detected(
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
    let run_id = run.id().clone();

    storage.insert_ocr_run(&run).unwrap();
    let loaded = storage.get_ocr_run(&run_id).unwrap().unwrap();

    assert_eq!(loaded.status, OcrRunStatus::NoTextDetected);
    assert_eq!(loaded.full_text, "");
    assert_eq!(loaded.unavailable_reason, None);
    assert_eq!(loaded.completed_at_ms, Some(260));
}

#[test]
fn unavailable_ocr_result_persists_reason_without_text() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);
    let run = OcrRun::unavailable(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        Some("latin-v1".to_string()),
        OcrUnavailableReason::ProviderFailed,
        Some("provider returned a non-retryable failure".to_string()),
        Some(270),
        vec!["no text asset committed".to_string()],
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    let run_id = run.id().clone();

    storage.insert_ocr_run(&run).unwrap();
    let loaded = storage.get_ocr_run(&run_id).unwrap().unwrap();

    assert_eq!(loaded.status, OcrRunStatus::Unavailable);
    assert_eq!(loaded.full_text, "");
    assert_eq!(
        loaded.unavailable_reason,
        Some(OcrUnavailableReason::ProviderFailed)
    );
    assert_eq!(
        loaded.unavailable_message.as_deref(),
        Some("provider returned a non-retryable failure")
    );
}

#[test]
fn legacy_empty_text_constructor_normalizes_to_no_text_detected() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);
    let run = OcrRun::new(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        "legacy".to_string(),
        "1".to_string(),
        String::new(),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    );
    let run_id = run.id().clone();

    storage.insert_ocr_run(&run).unwrap();
    let loaded = storage.get_ocr_run(&run_id).unwrap().unwrap();

    assert_eq!(loaded.status, OcrRunStatus::NoTextDetected);
    assert_eq!(loaded.full_text, "");
    assert_eq!(loaded.unavailable_reason, None);
}

#[test]
fn detected_result_rejects_empty_text_before_storage() {
    let storage = Storage::open_in_memory().unwrap();
    let fixture = insert_scan_fixture(&storage);

    let err = OcrRun::detected(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id),
        "mlkit".to_string(),
        "2026.09".to_string(),
        None,
        String::new(),
        Some(280),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap_err();

    assert_eq!(err.code.to_string(), "OCR_RUN_DETECTED_TEXT_EMPTY");
}
