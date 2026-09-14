use std::path::PathBuf;

use a2d_domain::{
    Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, Page, PageId, PageKind,
    PageState, Provenance, QualityStatus, Scan, ScanId, SmartPageId, TextCorrectionId,
    TextRegionId,
};
use a2d_storage::{
    AssetRepository, PageRepository, ScanRepository, Storage, TextCorrectionRecord,
    TextCorrectionRepository,
};

struct ScanFixture {
    page_id: PageId,
    scan_id: ScanId,
}

fn open_reopenable_storage() -> (Storage, PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("a2d-storage-ocr-correction-{}", PageId::generate()));
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

fn provenance(page_id: &PageId, scan_id: &ScanId, created_at_ms: i64) -> Provenance {
    Provenance {
        source_page_id: Some(page_id.clone()),
        source_scan_id: Some(scan_id.clone()),
        producing_component: "a2d-core-ocr-correction".to_string(),
        component_version: "0.1.0".to_string(),
        created_at_ms,
        warnings: Vec::new(),
        user_approved: Some(true),
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
        Some("OCR correction page".to_string()),
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
        .insert_asset(&original_asset(original_asset_id))
        .unwrap();
    storage.insert_scan(&scan).unwrap();

    ScanFixture { page_id, scan_id }
}

fn correction(
    fixture: &ScanFixture,
    text_region_id: Option<TextRegionId>,
    corrected_text: &str,
    previous_text: Option<&str>,
    created_at_ms: i64,
) -> TextCorrectionRecord {
    TextCorrectionRecord {
        id: TextCorrectionId::generate(),
        text_region_id,
        scan_id: fixture.scan_id.clone(),
        corrected_text: corrected_text.to_string(),
        previous_text: previous_text.map(ToString::to_string),
        provenance: provenance(&fixture.page_id, &fixture.scan_id, created_at_ms),
    }
}

#[test]
fn text_correction_round_trips_after_reopen_without_mutating_ocr_rows() {
    let (storage, dir) = open_reopenable_storage();
    let fixture = insert_scan_fixture(&storage);
    let correction = correction(
        &fixture,
        None,
        "corrected notebook text",
        Some("ocr notebook text"),
        300,
    );
    let correction_id = correction.id.clone();

    storage.insert_text_correction(&correction).unwrap();
    drop(storage);

    let storage = Storage::open(&dir.join("library.sqlite")).unwrap();
    let loaded = storage
        .get_text_correction(&correction_id)
        .unwrap()
        .expect("correction must round-trip");

    assert_eq!(loaded.id, correction_id);
    assert_eq!(loaded.text_region_id, None);
    assert_eq!(loaded.scan_id, fixture.scan_id);
    assert_eq!(loaded.corrected_text, "corrected notebook text");
    assert_eq!(loaded.previous_text.as_deref(), Some("ocr notebook text"));
    assert_eq!(loaded.provenance.source_page_id, Some(fixture.page_id));
    assert_eq!(loaded.provenance.source_scan_id, Some(fixture.scan_id));
    assert_eq!(loaded.provenance.user_approved, Some(true));

    drop(storage);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn text_corrections_list_by_scan_in_provenance_order() {
    let (storage, dir) = open_reopenable_storage();
    let fixture = insert_scan_fixture(&storage);
    let first = correction(&fixture, None, "first correction", Some("first"), 300);
    let second = correction(&fixture, None, "second correction", Some("second"), 301);

    storage.insert_text_correction(&first).unwrap();
    storage.insert_text_correction(&second).unwrap();

    let corrections = storage
        .list_text_corrections_for_scan(&fixture.scan_id, 10)
        .unwrap();

    assert_eq!(corrections.len(), 2);
    assert_eq!(corrections[0].corrected_text, "first correction");
    assert_eq!(corrections[1].corrected_text, "second correction");

    drop(storage);
    std::fs::remove_dir_all(&dir).ok();
}
