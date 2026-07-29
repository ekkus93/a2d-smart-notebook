//! Verifies that legacy repository mutations cannot bypass the audited preferred-scan workflow.

use a2d_domain::{
    AssetKind, CaptureSource, LayoutId, Page, PageId, PageKind, PageState, QualityStatus, Scan,
    ScanId, SmartPageId,
};
use a2d_storage::{AssetRepository, AssetStore, PageRepository, ScanRepository, Storage};

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("a2d-preferred-gate-{}", PageId::generate()))
}

#[test]
#[allow(deprecated)]
fn legacy_preferred_scan_setters_fail_closed_for_real_changes() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir();
    let asset_store = AssetStore::open(&root).unwrap();
    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        100,
    );
    storage.insert_page(&page).unwrap();

    let first_original = asset_store
        .commit(b"first", AssetKind::Original, "image/jpeg")
        .unwrap();
    let second_original = asset_store
        .commit(b"second", AssetKind::Original, "image/jpeg")
        .unwrap();
    storage.insert_asset(&first_original).unwrap();
    storage.insert_asset(&second_original).unwrap();

    let first = Scan::new(
        ScanId::generate(),
        page.id().clone(),
        None,
        CaptureSource::Camera,
        200,
        first_original.id().clone(),
        None,
        None,
        None,
        "v1".to_string(),
        QualityStatus::Accepted,
        vec![],
        true,
        None,
        "first".to_string(),
    );
    let second = Scan::new(
        ScanId::generate(),
        page.id().clone(),
        None,
        CaptureSource::Camera,
        300,
        second_original.id().clone(),
        None,
        None,
        None,
        "v1".to_string(),
        QualityStatus::NeedsReview,
        vec![],
        false,
        None,
        "second".to_string(),
    );
    storage.insert_scan(&first).unwrap();
    storage.insert_scan(&second).unwrap();

    let inherent_error = storage
        .set_preferred_scan(page.id(), second.id())
        .unwrap_err();
    assert_eq!(
        inherent_error.code.to_string(),
        "STORAGE_PREFERRED_SCAN_WORKFLOW_REQUIRED"
    );

    let trait_error = storage
        .transaction(|tx| PageRepository::set_preferred_scan(tx, page.id(), second.id()))
        .unwrap_err();
    assert!(
        trait_error
            .to_string()
            .contains("A2D_PREFERRED_SCAN_WORKFLOW_REQUIRED")
    );

    let stored_page = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(stored_page.preferred_scan_id, Some(first.id().clone()));
    assert!(storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(!storage.get_scan(second.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}
