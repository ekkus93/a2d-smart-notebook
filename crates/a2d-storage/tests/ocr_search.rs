use std::path::PathBuf;

use a2d_domain::{
    Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRun, OcrRunId,
    OcrRunStatus, OcrUnavailableReason, Page, PageId, PageKind, PageState, Provenance,
    QualityStatus, Scan, ScanId, SmartPageId, TextRegion, TextRegionId,
};
use a2d_storage::{
    AssetRepository, OcrRunRepository, OcrSearchDocumentKind, OcrSearchQuery, OcrSearchRepository,
    PageRepository, ScanRepository, Storage, TextRegionRepository,
};

struct ScanFixture {
    page_id: PageId,
    scan_id: ScanId,
    original_asset_id: AssetId,
}

fn open_reopenable_storage() -> (Storage, PathBuf) {
    let dir = std::env::temp_dir().join(format!("a2d-storage-ocr-search-{}", PageId::generate()));
    std::fs::create_dir_all(&dir).unwrap();
    let storage = Storage::open(&dir.join("library.sqlite")).unwrap();
    (storage, dir)
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
        Some("OCR search page".to_string()),
        PageState::Scanned,
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
        page_id,
        scan_id,
        original_asset_id,
    }
}

fn insert_detected_run(storage: &Storage, fixture: &ScanFixture, full_text: &str) -> OcrRunId {
    let run = OcrRun::detected(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        Some("latin-v1".to_string()),
        full_text.to_string(),
        Some(250),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    let run_id = run.id().clone();
    storage.insert_ocr_run(&run).unwrap();
    run_id
}

fn insert_no_text_run(storage: &Storage, fixture: &ScanFixture) {
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
    storage.insert_ocr_run(&run).unwrap();
}

fn insert_unavailable_run(storage: &Storage, fixture: &ScanFixture) {
    let run = OcrRun::unavailable(
        OcrRunId::generate(),
        fixture.scan_id.clone(),
        Some(fixture.original_asset_id.clone()),
        "mlkit".to_string(),
        "2026.09".to_string(),
        None,
        OcrUnavailableReason::ProviderFailed,
        Some("phantom provider failure".to_string()),
        Some(270),
        Vec::new(),
        provenance(&fixture.page_id, &fixture.scan_id),
    )
    .unwrap();
    storage.insert_ocr_run(&run).unwrap();
}

fn text_region(ocr_run_id: OcrRunId, text: &str) -> TextRegion {
    TextRegion::new(
        TextRegionId::generate(),
        ocr_run_id,
        vec![(0.0, 0.0), (120.0, 0.0), (120.0, 32.0), (0.0, 32.0)],
        text.to_string(),
        Some(0.91),
        300,
    )
    .unwrap()
}

#[test]
fn detected_ocr_full_text_is_searchable_after_reopen() {
    let (storage, dir) = open_reopenable_storage();
    let fixture = insert_scan_fixture(&storage);
    let run_id = insert_detected_run(
        &storage,
        &fixture,
        "rainbow notebook measurements and calibration notes",
    );
    drop(storage);

    let storage = Storage::open(&dir.join("library.sqlite")).unwrap();
    let hits = storage
        .search_ocr_text(&OcrSearchQuery::new("rainbow", 10).unwrap())
        .unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].page_id, fixture.page_id.to_string());
    assert_eq!(hits[0].scan_id, fixture.scan_id.to_string());
    assert_eq!(hits[0].ocr_run_id, run_id.to_string());
    assert_eq!(hits[0].text_region_id, None);
    assert_eq!(hits[0].document_kind, OcrSearchDocumentKind::FullText);
    assert!(hits[0].snippet.to_lowercase().contains("rainbow"));

    drop(storage);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn text_region_rows_are_searchable_as_region_documents() {
    let (storage, dir) = open_reopenable_storage();
    let fixture = insert_scan_fixture(&storage);
    let run_id = insert_detected_run(&storage, &fixture, "top level full OCR text only");
    let region = text_region(run_id.clone(), "margin triangle annotation");
    let region_id = region.id().clone();
    storage.insert_text_region(&region).unwrap();

    let hits = storage
        .search_ocr_text(&OcrSearchQuery::new("triangle", 10).unwrap())
        .unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].ocr_run_id, run_id.to_string());
    assert_eq!(hits[0].text_region_id, Some(region_id.to_string()));
    assert_eq!(hits[0].document_kind, OcrSearchDocumentKind::TextRegion);

    drop(storage);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_text_and_unavailable_runs_do_not_create_search_documents() {
    let (storage, dir) = open_reopenable_storage();
    let fixture = insert_scan_fixture(&storage);
    insert_no_text_run(&storage, &fixture);
    insert_unavailable_run(&storage, &fixture);

    let hits = storage
        .search_ocr_text(&OcrSearchQuery::new("phantom", 10).unwrap())
        .unwrap();

    assert!(hits.is_empty());

    drop(storage);
    std::fs::remove_dir_all(&dir).ok();
}
