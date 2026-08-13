use std::collections::BTreeMap;

use a2d_domain::{ErrorSeverity, PageId, ReviewItemKind, ReviewItemStatus};

use crate::{
    A2dCore, CreateReviewItemRequest, DeferReviewItemRequest, ListReviewItemsRequest,
    OpenLibraryRequest, ResolveReviewItemRequest,
};

fn open_core() -> (std::sync::Arc<A2dCore>, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("a2d-core-review-{}", PageId::generate()));
    let core = A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    (core, root)
}

fn create(core: &A2dCore, kind: ReviewItemKind, created_at_ms: i64) -> a2d_domain::ReviewItem {
    core.create_review_item(CreateReviewItemRequest {
        kind,
        page_id: None,
        scan_id: None,
        severity: ErrorSeverity::Warning,
        details: BTreeMap::from([("reason_code".to_string(), "TEST".to_string())]),
        created_at_ms,
    })
    .unwrap()
}

#[test]
fn list_detail_defer_and_resolve_use_typed_rust_state() {
    let (core, root) = open_core();
    let older = create(&core, ReviewItemKind::LowQuality, 100);
    let newer = create(&core, ReviewItemKind::Revision, 200);

    let page = core
        .list_review_items(ListReviewItemsRequest {
            kind: None,
            status: Some(ReviewItemStatus::Open),
            page_id: None,
            scan_id: None,
            limit: 1,
            offset: 0,
        })
        .unwrap();
    assert_eq!(page.items, vec![newer.clone()]);
    assert!(page.has_more);
    assert_eq!(page.next_offset, Some(1));

    assert_eq!(core.get_review_item(older.id()).unwrap(), older);

    let deferred = core
        .defer_review_item(DeferReviewItemRequest {
            review_item_id: newer.id().clone(),
            deferred_at_ms: 300,
            actor: "android-user".to_string(),
        })
        .unwrap();
    assert!(deferred.changed);
    assert_eq!(deferred.item.status, ReviewItemStatus::Deferred);
    assert!(!deferred.committed_data_deleted);

    let resolved = core
        .resolve_review_item(ResolveReviewItemRequest {
            review_item_id: newer.id().clone(),
            resolution_code: "KEEP_BOTH_VERSIONS".to_string(),
            resolved_at_ms: 400,
            actor: "android-user".to_string(),
        })
        .unwrap();
    assert!(resolved.changed);
    assert_eq!(resolved.item.status, ReviewItemStatus::Resolved);
    assert_eq!(resolved.item.resolution.as_deref(), Some("KEEP_BOTH_VERSIONS"));
    assert!(!resolved.committed_data_deleted);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn list_bounds_and_resolution_codes_fail_closed() {
    let (core, root) = open_core();
    let item = create(&core, ReviewItemKind::ProcessingFailure, 100);

    let list_error = core
        .list_review_items(ListReviewItemsRequest {
            kind: None,
            status: None,
            page_id: None,
            scan_id: None,
            limit: 101,
            offset: 0,
        })
        .unwrap_err();
    assert_eq!(list_error.code.to_string(), "CORE_REVIEW_LIST_LIMIT_INVALID");

    let resolution_error = core
        .resolve_review_item(ResolveReviewItemRequest {
            review_item_id: item.id().clone(),
            resolution_code: "contains raw free form text".to_string(),
            resolved_at_ms: 200,
            actor: "android-user".to_string(),
        })
        .unwrap_err();
    assert_eq!(
        resolution_error.code.to_string(),
        "STORAGE_REVIEW_RESOLUTION_CODE_INVALID"
    );
    assert_eq!(
        core.get_review_item(item.id()).unwrap().status,
        ReviewItemStatus::Open
    );

    std::fs::remove_dir_all(root).ok();
}
