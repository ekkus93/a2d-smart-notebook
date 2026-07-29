//! Regression tests for preferred-scan schema invariants and the audited workflow.

use std::collections::BTreeMap;

use a2d_domain::{
    AssetKind, AuditEvent, AuditEventId, CaptureSource, LayoutId, Page, PageId, PageKind,
    PageState, QualityStatus, Scan, ScanId, SmartPageId,
};
use a2d_storage::{
    AssetRepository, AssetStore, AuditEventRepository, ChangePreferredScanRequest, PageRepository,
    ScanRepository, Storage,
};

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("a2d-preferred-scan-{label}-{}", PageId::generate()))
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

fn change_request(
    page_id: &PageId,
    scan_id: &ScanId,
    changed_at_ms: i64,
    actor: &str,
    operation_id: AuditEventId,
) -> ChangePreferredScanRequest {
    ChangePreferredScanRequest {
        page_id: page_id.clone(),
        scan_id: scan_id.clone(),
        changed_at_ms,
        actor: actor.to_string(),
        operation_id,
    }
}

fn committed_original(store: &AssetStore, bytes: &[u8]) -> a2d_domain::Asset {
    store
        .commit(bytes, AssetKind::Original, "image/jpeg")
        .unwrap()
}

#[test]
fn audited_change_synchronizes_page_pointer_scan_flags_and_audit_event() {
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

    let operation_id = AuditEventId::generate();
    let result = storage
        .change_preferred_scan(change_request(
            page.id(),
            second.id(),
            400,
            "integration-test",
            operation_id.clone(),
        ))
        .unwrap();
    assert_eq!(result.page_id, page.id().clone());
    assert_eq!(result.previous_preferred_scan_id, Some(first.id().clone()));
    assert_eq!(result.preferred_scan_id, second.id().clone());
    assert!(result.changed);
    assert_eq!(result.audit_event_id, Some(operation_id.clone()));

    let after = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(after.preferred_scan_id, Some(second.id().clone()));
    assert_eq!(after.updated_at_ms, 400);
    assert!(!storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(storage.get_scan(second.id()).unwrap().unwrap().preferred);

    let audit = storage
        .get_audit_event(&operation_id)
        .unwrap()
        .expect("changed preference must be audited");
    assert_eq!(audit.occurred_at_ms, 400);
    assert_eq!(audit.event_kind, "scan.preferred_changed");
    assert_eq!(audit.actor, "integration-test");
    let expected_page_subject = page.id().to_string();
    assert_eq!(
        audit.subject.as_deref(),
        Some(expected_page_subject.as_str())
    );
    assert_eq!(audit.correlation_id, Some(operation_id.to_string()));
    assert_eq!(
        audit.details.get("previous_preferred_scan_id"),
        Some(&first.id().to_string())
    );
    assert_eq!(
        audit.details.get("preferred_scan_id"),
        Some(&second.id().to_string())
    );

    // Repeating the exact choice is a no-op: no timestamp change and no audit noise.
    let no_op_id = AuditEventId::generate();
    let no_op = storage
        .change_preferred_scan(change_request(
            page.id(),
            second.id(),
            500,
            "integration-test",
            no_op_id.clone(),
        ))
        .unwrap();
    assert!(!no_op.changed);
    assert_eq!(no_op.audit_event_id, None);
    assert_eq!(
        storage.get_page(page.id()).unwrap().unwrap().updated_at_ms,
        400
    );
    assert_eq!(storage.get_audit_event(&no_op_id).unwrap(), None);

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
        .change_preferred_scan(change_request(
            page_a.id(),
            scan_b.id(),
            300,
            "integration-test",
            AuditEventId::generate(),
        ))
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
        .change_preferred_scan(change_request(
            page.id(),
            &ScanId::generate(),
            300,
            "integration-test",
            AuditEventId::generate(),
        ))
        .unwrap_err();
    assert_eq!(
        unknown_scan_error.code.to_string(),
        "STORAGE_SCAN_NOT_FOUND"
    );

    let unknown_page_error = storage
        .change_preferred_scan(change_request(
            &PageId::generate(),
            known.id(),
            300,
            "integration-test",
            AuditEventId::generate(),
        ))
        .unwrap_err();
    assert_eq!(
        unknown_page_error.code.to_string(),
        "STORAGE_PAGE_NOT_FOUND"
    );

    let invalid_time = storage
        .change_preferred_scan(change_request(
            page.id(),
            known.id(),
            0,
            "integration-test",
            AuditEventId::generate(),
        ))
        .unwrap_err();
    assert_eq!(
        invalid_time.code.to_string(),
        "STORAGE_PREFERRED_SCAN_TIME_INVALID"
    );

    let invalid_actor = storage
        .change_preferred_scan(change_request(
            page.id(),
            known.id(),
            300,
            "   ",
            AuditEventId::generate(),
        ))
        .unwrap_err();
    assert_eq!(
        invalid_actor.code.to_string(),
        "STORAGE_PREFERRED_SCAN_ACTOR_INVALID"
    );

    assert_eq!(
        storage
            .get_page(page.id())
            .unwrap()
            .unwrap()
            .preferred_scan_id,
        Some(known.id().clone())
    );
    assert!(storage.get_scan(known.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn candidate_referenced_assets_must_exist_and_match_their_roles() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("asset-role");
    let assets = AssetStore::open(&root).unwrap();
    let page = page(100);
    storage.insert_page(&page).unwrap();

    let first_original = committed_original(&assets, b"first");
    let candidate_original = committed_original(&assets, b"candidate");
    let wrong_corrected_kind = committed_original(&assets, b"not-corrected");
    storage.insert_asset(&first_original).unwrap();
    storage.insert_asset(&candidate_original).unwrap();
    storage.insert_asset(&wrong_corrected_kind).unwrap();

    let first = scan(page.id(), first_original.id().clone(), 200, true);
    storage.insert_scan(&first).unwrap();
    let candidate = Scan::new(
        ScanId::generate(),
        page.id().clone(),
        None,
        CaptureSource::Camera,
        300,
        candidate_original.id().clone(),
        Some(wrong_corrected_kind.id().clone()),
        None,
        None,
        "v1".to_string(),
        QualityStatus::NeedsReview,
        vec![],
        false,
        None,
        "fingerprint-candidate".to_string(),
    );
    storage.insert_scan(&candidate).unwrap();

    let error = storage
        .change_preferred_scan(change_request(
            page.id(),
            candidate.id(),
            400,
            "integration-test",
            AuditEventId::generate(),
        ))
        .unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_PREFERRED_SCAN_ASSET_KIND_INVALID"
    );
    assert_eq!(
        error.details.get("asset_role").map(String::as_str),
        Some("corrected")
    );
    assert_eq!(
        storage
            .get_page(page.id())
            .unwrap()
            .unwrap()
            .preferred_scan_id,
        Some(first.id().clone())
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn audit_insertion_failure_rolls_back_page_and_scan_mutations() {
    let mut storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("audit-rollback");
    let assets = AssetStore::open(&root).unwrap();
    let page = page(100);
    storage.insert_page(&page).unwrap();

    let original_a = committed_original(&assets, b"rollback-a");
    let original_b = committed_original(&assets, b"rollback-b");
    storage.insert_asset(&original_a).unwrap();
    storage.insert_asset(&original_b).unwrap();
    let first = scan(page.id(), original_a.id().clone(), 200, true);
    let second = scan(page.id(), original_b.id().clone(), 300, false);
    storage.insert_scan(&first).unwrap();
    storage.insert_scan(&second).unwrap();

    let duplicate_operation_id = AuditEventId::generate();
    storage
        .insert_audit_event(&AuditEvent::new(
            duplicate_operation_id.clone(),
            250,
            "test.preexisting".to_string(),
            "integration-test".to_string(),
            None,
            BTreeMap::new(),
            Some(duplicate_operation_id.to_string()),
        ))
        .unwrap();

    let error = storage
        .change_preferred_scan(change_request(
            page.id(),
            second.id(),
            400,
            "integration-test",
            duplicate_operation_id,
        ))
        .unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_ID_COLLISION");

    let stored_page = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(stored_page.preferred_scan_id, Some(first.id().clone()));
    assert_eq!(stored_page.updated_at_ms, 300);
    assert!(storage.get_scan(first.id()).unwrap().unwrap().preferred);
    assert!(!storage.get_scan(second.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn unique_index_rejects_two_preferred_scans_for_one_page() {
    let storage = Storage::open_in_memory().unwrap();
    let root = temp_dir("unique-index");
    let assets = AssetStore::open(&root).unwrap();
    let page = page(100);
    storage.insert_page(&page).unwrap();

    let original_a = committed_original(&assets, b"unique-a");
    let original_b = committed_original(&assets, b"unique-b");
    storage.insert_asset(&original_a).unwrap();
    storage.insert_asset(&original_b).unwrap();
    let first = scan(page.id(), original_a.id().clone(), 200, true);
    let second = scan(page.id(), original_b.id().clone(), 300, true);
    storage.insert_scan(&first).unwrap();

    let error = storage.insert_scan(&second).unwrap_err();
    assert!(error.code.to_string().contains("UNIQUE_CONSTRAINT"));
    assert_eq!(storage.get_scan(second.id()).unwrap(), None);
    assert_eq!(
        storage
            .get_page(page.id())
            .unwrap()
            .unwrap()
            .preferred_scan_id,
        Some(first.id().clone())
    );
    assert!(storage.get_scan(first.id()).unwrap().unwrap().preferred);

    std::fs::remove_dir_all(root).ok();
}
