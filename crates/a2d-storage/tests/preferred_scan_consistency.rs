//! Regression tests for migration 0005 and the audited preferred-scan workflow.

use a2d_domain::{
    AssetKind, CaptureSource, LayoutId, Page, PageId, PageKind, PageState, QualityStatus, Scan,
    ScanId, SmartPageId,
};
use a2d_storage::{AssetRepository, AssetStore, PageRepository, ScanRepository, Storage};

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-preferred-scan-{label}-{}",
        PageId::generate()
    ))
}

fn page(created_at_ms: i64) -> Page {
    Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        created_at_ms,
    )
}

fn scan(
    page_id: &PageId,
    original_asset_id: a2d_domain::AssetId,
    captured_at_ms: i64,
    preferred: bool,
) -> Scan {
    Scan::new(
        ScanId::generate(),
        page_id.clone(),
        None,
        CaptureSource::Camera,
        captured_at_ms,
        original_asset_id,
        None,
        None,
        None,
        "v1".to_string(),
        if preferred {
            QualityStatus::Accepted
        } else {
            QualityStatus::NeedsReview
        },
        vec![],
        preferred,
        None,
        format!("fingerprint-{captured_at_ms}"),
    )
}

fn committed_original(store: &AssetStore, bytes: &[u8]) -> a2d_domain::Asset {
    store
        .commit(bytes, AssetKind::Original, "image/jpeg")
        .unwrap()
}

#[test]
fn audited_change_synchronizes_page_pointer_and_all_scan_flags() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("switch");
    let assets = AssetStore::open(&root).unwrap();
    let page = page(100);
    storage.insert_page(&page).unwrap();

    let original_a = committed_original(&assets, b"original-a");
    let original_b = committed_original(&assets, b"original-b");
    storage.insert_asset(&original_a).unwrap();
    storage.insert_asset(&original_b).unwrap();

    let first = scan(page.id(), original_a.id().clone(), 200, true);
    let second = scan(page.id(), original_b.id().clone(), 300, false);
    storage.insert_scan(&first).unwrap();
    storage.insert_scan(&second).unwrap();

    let before = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(before.preferred_scan_id, Some(first.id().clone()));
    assert!(storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(!storage.get_scan(second.id()).unwrap().unwrap().preferred);

    assert!(
        storage
            .change_preferred_scan(
                page.id(),
                second.id(),
                400,
                "integration-test",
                Some("preferred-scan-test"),
            )
            .unwrap()
    );

    let after = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(after.preferred_scan_id, Some(second.id().clone()));
    assert_eq!(after.updated_at_ms, 400);
    assert!(!storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(second.id()).unwrap().unwrap().preferred);

    // Repeating the exact choice is a no-op and does not create another state transition.
    assert!(
        !storage
            .change_preferred_scan(
                page.id(),
                second.id(),
                500,
                "integration-test",
                Some("preferred-scan-test-repeat"),
            )
            .unwrap()
    );
    assert_eq!(
        storage.get_page(page.id()).unwrap().unwrap().updated_at_ms,
        400
    );
    assert!(!storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(second.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn audited_change_rejects_a_scan_owned_by_another_page() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("cross-page");
    let assets = AssetStore::open(&root).unwrap();
    let page_a = page(100);
    let page_b = page(101);
    storage.insert_page(&page_a).unwrap();
    storage.insert_page(&page_b).unwrap();

    let original_a = committed_original(&assets, b"page-a");
    let original_b = committed_original(&assets, b"page-b");
    storage.insert_asset(&original_a).unwrap();
    storage.insert_asset(&original_b).unwrap();
    let scan_a = scan(page_a.id(), original_a.id().clone(), 200, true);
    let scan_b = scan(page_b.id(), original_b.id().clone(), 201, true);
    storage.insert_scan(&scan_a).unwrap();
    storage.insert_scan(&scan_b).unwrap();

    let error = storage
        .change_preferred_scan(
            page_a.id(),
            scan_b.id(),
            300,
            "integration-test",
            None,
        )
        .unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_PREFERRED_SCAN_PAGE_MISMATCH"
    );

    let stored_page_a = storage.get_page(page_a.id()).unwrap().unwrap();
    let stored_page_b = storage.get_page(page_b.id()).unwrap().unwrap();
    assert_eq!(stored_page_a.preferred_scan_id, Some(scan_a.id().clone()));
    assert_eq!(stored_page_b.preferred_scan_id, Some(scan_b.id().clone()));
    assert!(storage.get_scan(scan_a.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(scan_b.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn audited_change_rejects_unknown_records_and_invalid_audit_context() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("validation");
    let assets = AssetStore::open(&root).unwrap();
    let page = page(100);
    storage.insert_page(&page).unwrap();

    let original = committed_original(&assets, b"known");
    storage.insert_asset(&original).unwrap();
    let known = scan(page.id(), original.id().clone(), 200, true);
    storage.insert_scan(&known).unwrap();

    let unknown_scan_error = storage
        .change_preferred_scan(
            page.id(),
            &ScanId::generate(),
            300,
            "integration-test",
            None,
        )
        .unwrap_err();
    assert_eq!(unknown_scan_error.code.to_string(), "STORAGE_SCAN_NOT_FOUND");

    let unknown_page_error = storage
        .change_preferred_scan(
            &PageId::generate(),
            known.id(),
            300,
            "integration-test",
            None,
        )
        .unwrap_err();
    assert_eq!(unknown_page_error.code.to_string(), "STORAGE_PAGE_NOT_FOUND");

    let invalid_time = storage
        .change_preferred_scan(page.id(), known.id(), 0, "integration-test", None)
        .unwrap_err();
    assert_eq!(
        invalid_time.code.to_string(),
        "STORAGE_PREFERRED_SCAN_TIME_INVALID"
    );

    let invalid_actor = storage
        .change_preferred_scan(page.id(), known.id(), 300, "   ", None)
        .unwrap_err();
    assert_eq!(
        invalid_actor.code.to_string(),
        "STORAGE_PREFERRED_SCAN_ACTOR_INVALID"
    );

    assert_eq!(
        storage.get_page(page.id()).unwrap().unwrap().preferred_scan_id,
        Some(known.id().clone())
    );
    assert!(storage.get_scan(known.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn low_level_page_pointer_update_is_still_guarded_by_the_schema() {
    let storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("schema-guard");
    let assets = AssetStore::open(&root).unwrap();
    let page_a = page(100);
    let page_b = page(101);
    storage.insert_page(&page_a).unwrap();
    storage.insert_page(&page_b).unwrap();

    let original_a = committed_original(&assets, b"schema-a");
    let original_b = committed_original(&assets, b"schema-b");
    storage.insert_asset(&original_a).unwrap();
    storage.insert_asset(&original_b).unwrap();
    let scan_a = scan(page_a.id(), original_a.id().clone(), 200, true);
    let scan_b = scan(page_b.id(), original_b.id().clone(), 201, true);
    storage.insert_scan(&scan_a).unwrap();
    storage.insert_scan(&scan_b).unwrap();

    let error = storage
        .set_preferred_scan(page_a.id(), scan_b.id())
        .unwrap_err();
    assert!(error.code.to_string().contains("CONSTRAINT"));
    assert_eq!(
        storage.get_page(page_a.id()).unwrap().unwrap().preferred_scan_id,
        Some(scan_a.id().clone())
    );
    assert!(storage.get_scan(scan_a.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(scan_b.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}
