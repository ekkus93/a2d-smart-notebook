//! Regression tests for migration 0005's preferred-scan ownership and synchronization invariants.

use a2d_domain::{
    AssetKind, CaptureSource, LayoutId, Page, PageId, PageKind, PageState, QualityStatus, Scan,
    ScanId, SmartPageId,
};
use a2d_storage::{
    AssetRepository, AssetStore, PageRepository, ScanRepository, Storage,
};

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
fn changing_the_page_pointer_synchronizes_all_scan_preferred_flags() {
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

    storage.set_preferred_scan(page.id(), second.id()).unwrap();

    let after = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(after.preferred_scan_id, Some(second.id().clone()));
    assert!(!storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(second.id()).unwrap().unwrap().preferred);

    // Repeating the exact choice remains idempotent and cannot create duplicate preferred flags.
    storage.set_preferred_scan(page.id(), second.id()).unwrap();
    assert!(!storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(second.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_page_cannot_prefer_a_scan_owned_by_another_page() {
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
        .set_preferred_scan(page_a.id(), scan_b.id())
        .unwrap_err();
    assert!(error.code.to_string().contains("CONSTRAINT"));

    let stored_page_a = storage.get_page(page_a.id()).unwrap().unwrap();
    let stored_page_b = storage.get_page(page_b.id()).unwrap().unwrap();
    assert_eq!(stored_page_a.preferred_scan_id, Some(scan_a.id().clone()));
    assert_eq!(stored_page_b.preferred_scan_id, Some(scan_b.id().clone()));
    assert!(storage.get_scan(scan_a.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(scan_b.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn an_unknown_scan_cannot_become_a_page_preference() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("unknown");
    let assets = AssetStore::open(&root).unwrap();
    let page = page(100);
    storage.insert_page(&page).unwrap();

    let original = committed_original(&assets, b"known");
    storage.insert_asset(&original).unwrap();
    let known = scan(page.id(), original.id().clone(), 200, true);
    storage.insert_scan(&known).unwrap();

    let error = storage
        .set_preferred_scan(page.id(), &ScanId::generate())
        .unwrap_err();
    assert!(error.code.to_string().contains("CONSTRAINT"));
    assert_eq!(
        storage.get_page(page.id()).unwrap().unwrap().preferred_scan_id,
        Some(known.id().clone())
    );
    assert!(storage.get_scan(known.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}
