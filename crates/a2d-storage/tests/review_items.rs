use std::collections::BTreeMap;

use a2d_domain::{
    Asset, AssetId, AssetKind, AuditEventId, CaptureSource, EncryptionState, ErrorSeverity,
    LayoutId, Page, PageId, PageKind, PageState, QualityStatus, ReviewItem, ReviewItemId,
    ReviewItemKind, ReviewItemStatus, Scan, ScanId, SmartPageId,
};
use a2d_storage::{
    AssetRepository, AuditEventRepository, DeferReviewItemRequest, PageRepository,
    ResolveReviewItemRequest, ReviewItemQuery, ReviewItemRepository, ScanRepository, Storage,
};

fn page(id: PageId, created_at_ms: i64) -> Page {
    Page::new(
        id,
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: Some(1),
        },
        LayoutId::parse("USLETTER-LINED").unwrap(),
        None,
        PageState::NeedsReview,
        created_at_ms,
    )
}

fn original_asset(id: AssetId, suffix: &str) -> Asset {
    Asset::new(
        id,
        AssetKind::Original,
        format!("assets/{suffix}.jpg"),
        "image/jpeg".to_string(),
        4,
        format!("sha256-{suffix}"),
        100,
        true,
        EncryptionState::Plaintext,
    )
}

fn scan(id: ScanId, page_id: PageId, original_asset_id: AssetId) -> Scan {
    Scan::new(
        id,
        page_id,
        None,
        CaptureSource::Camera,
        200,
        original_asset_id,
        None,
        None,
        None,
        "pipeline-v1".to_string(),
        QualityStatus::NeedsReview,
        vec!["REVIEW_REQUIRED".to_string()],
        false,
        None,
        "fingerprint-v1".to_string(),
    )
}

fn open_review(
    kind: ReviewItemKind,
    page_id: Option<PageId>,
    scan_id: Option<ScanId>,
    created_at_ms: i64,
) -> ReviewItem {
    ReviewItem::new(
        ReviewItemId::generate(),
        kind,
        page_id,
        scan_id,
        ErrorSeverity::Warning,
        ReviewItemStatus::Open,
        BTreeMap::from([("reason_code".to_string(), "TEST_REVIEW".to_string())]),
        None,
        created_at_ms,
        None,
    )
}

#[test]
fn every_required_review_kind_round_trips_and_filters_with_stable_pagination() {
    let storage = Storage::open_in_memory().unwrap();
    let kinds = [
        ReviewItemKind::UnidentifiedPage,
        ReviewItemKind::NotebookSelection,
        ReviewItemKind::WrongNotebook,
        ReviewItemKind::LowQuality,
        ReviewItemKind::ManualAlignment,
        ReviewItemKind::Duplicate,
        ReviewItemKind::Revision,
        ReviewItemKind::PhysicalCopy,
        ReviewItemKind::OcrFailure,
        ReviewItemKind::ProcessingFailure,
        ReviewItemKind::ImportConflict,
        ReviewItemKind::RestoreConflict,
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        storage
            .insert_review_item(&open_review(kind, None, None, 100 + index as i64))
            .unwrap();
    }

    let first = storage
        .list_review_items(&ReviewItemQuery {
            kind: None,
            status: Some(ReviewItemStatus::Open),
            page_id: None,
            scan_id: None,
            limit: 5,
            offset: 0,
        })
        .unwrap();
    let second = storage
        .list_review_items(&ReviewItemQuery {
            kind: None,
            status: Some(ReviewItemStatus::Open),
            page_id: None,
            scan_id: None,
            limit: 5,
            offset: 5,
        })
        .unwrap();
    assert_eq!(first.len(), 5);
    assert_eq!(second.len(), 5);
    assert!(first[0].created_at_ms > first[4].created_at_ms);
    assert!(first.iter().all(|item| !second.contains(item)));

    let revision = storage
        .list_review_items(&ReviewItemQuery {
            kind: Some(ReviewItemKind::Revision),
            status: None,
            page_id: None,
            scan_id: None,
            limit: 10,
            offset: 0,
        })
        .unwrap();
    assert_eq!(revision.len(), 1);
    assert_eq!(revision[0].kind, ReviewItemKind::Revision);
}

#[test]
fn defer_then_resolve_is_audited_idempotent_and_does_not_mutate_scan_or_asset() {
    let mut storage = Storage::open_in_memory().unwrap();
    let page_id = PageId::generate();
    storage.insert_page(&page(page_id.clone(), 100)).unwrap();
    let asset = original_asset(AssetId::generate(), "review-original");
    storage.insert_asset(&asset).unwrap();
    let scan_id = ScanId::generate();
    let scan = scan(scan_id.clone(), page_id.clone(), asset.id().clone());
    storage.insert_scan(&scan).unwrap();
    let item = open_review(
        ReviewItemKind::Revision,
        Some(page_id.clone()),
        Some(scan_id.clone()),
        300,
    );
    storage.insert_review_item(&item).unwrap();

    let defer_audit = AuditEventId::generate();
    let deferred = storage
        .defer_review_item(DeferReviewItemRequest {
            review_item_id: item.id().clone(),
            deferred_at_ms: 400,
            actor: "android-user".to_string(),
            operation_id: defer_audit.clone(),
        })
        .unwrap();
    assert!(deferred.changed);
    assert_eq!(deferred.item.status, ReviewItemStatus::Deferred);
    assert!(!deferred.committed_data_deleted);
    assert_eq!(deferred.audit_event_id, Some(defer_audit.clone()));
    assert_eq!(
        storage
            .get_audit_event(&defer_audit)
            .unwrap()
            .unwrap()
            .event_kind,
        "review_item.deferred"
    );

    let repeated_defer = storage
        .defer_review_item(DeferReviewItemRequest {
            review_item_id: item.id().clone(),
            deferred_at_ms: 450,
            actor: "android-user".to_string(),
            operation_id: AuditEventId::generate(),
        })
        .unwrap();
    assert!(!repeated_defer.changed);
    assert_eq!(repeated_defer.audit_event_id, None);

    let resolve_audit = AuditEventId::generate();
    let resolved = storage
        .resolve_review_item(ResolveReviewItemRequest {
            review_item_id: item.id().clone(),
            resolution_code: "KEEP_BOTH_VERSIONS".to_string(),
            resolved_at_ms: 500,
            actor: "android-user".to_string(),
            operation_id: resolve_audit.clone(),
        })
        .unwrap();
    assert!(resolved.changed);
    assert_eq!(resolved.item.status, ReviewItemStatus::Resolved);
    assert_eq!(
        resolved.item.resolution.as_deref(),
        Some("KEEP_BOTH_VERSIONS")
    );
    assert_eq!(resolved.item.resolved_at_ms, Some(500));
    assert!(!resolved.committed_data_deleted);
    let audit = storage.get_audit_event(&resolve_audit).unwrap().unwrap();
    assert_eq!(audit.event_kind, "review_item.resolved");
    assert_eq!(
        audit
            .details
            .get("committed_data_deleted")
            .map(String::as_str),
        Some("false")
    );

    assert_eq!(storage.get_scan(&scan_id).unwrap().unwrap(), scan);
    assert_eq!(storage.get_asset(asset.id()).unwrap().unwrap(), asset);

    let repeated_resolve = storage
        .resolve_review_item(ResolveReviewItemRequest {
            review_item_id: item.id().clone(),
            resolution_code: "KEEP_BOTH_VERSIONS".to_string(),
            resolved_at_ms: 600,
            actor: "android-user".to_string(),
            operation_id: AuditEventId::generate(),
        })
        .unwrap();
    assert!(!repeated_resolve.changed);
    assert_eq!(repeated_resolve.audit_event_id, None);

    let conflicting = storage
        .resolve_review_item(ResolveReviewItemRequest {
            review_item_id: item.id().clone(),
            resolution_code: "SET_PREFERRED".to_string(),
            resolved_at_ms: 700,
            actor: "android-user".to_string(),
            operation_id: AuditEventId::generate(),
        })
        .unwrap_err();
    assert_eq!(
        conflicting.code.to_string(),
        "STORAGE_REVIEW_ALREADY_RESOLVED_DIFFERENTLY"
    );
}

#[test]
fn insertion_rejects_a_page_scan_reference_mismatch() {
    let storage = Storage::open_in_memory().unwrap();
    let first_page = PageId::generate();
    let second_page = PageId::generate();
    storage.insert_page(&page(first_page.clone(), 100)).unwrap();
    storage
        .insert_page(&page(second_page.clone(), 101))
        .unwrap();
    let asset = original_asset(AssetId::generate(), "mismatch");
    storage.insert_asset(&asset).unwrap();
    let scan_id = ScanId::generate();
    storage
        .insert_scan(&scan(scan_id.clone(), first_page, asset.id().clone()))
        .unwrap();

    let error = storage
        .insert_review_item(&open_review(
            ReviewItemKind::LowQuality,
            Some(second_page),
            Some(scan_id),
            300,
        ))
        .unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_REVIEW_PAGE_SCAN_MISMATCH");
}
