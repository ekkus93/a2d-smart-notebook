use std::path::{Path, PathBuf};

use a2d_core::{
    A2dCore, BatchScanEntryStatus, BatchScanReviewReason, BeginBatchScanSessionRequest,
    BeginScannerRecoveryRequest, CreateNotebookRequest, OpenLibraryRequest, PageResolution,
};
use a2d_domain::{LayoutId, NotebookDesignId, PageId};
use a2d_identity::PageCode;
use a2d_layout::bundled_placeholder_registry;

const DESIGN_ID: &str = "6DE28E53DBKPXCWWNHPC8T7QJX";

fn open_core() -> (std::sync::Arc<A2dCore>, PathBuf) {
    let root = std::env::temp_dir().join(format!("a2d-batch-ordering-{}", PageId::generate()));
    let core = A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    (core, root)
}

fn placeholder_design_id() -> NotebookDesignId {
    NotebookDesignId::parse(DESIGN_ID).unwrap()
}

fn placeholder_page_layout_id() -> LayoutId {
    let design_id = placeholder_design_id();
    bundled_placeholder_registry()
        .unwrap()
        .resolve(&design_id)
        .unwrap()
        .page_layout_id
        .clone()
}

fn create_active_notebook(core: &A2dCore) -> a2d_domain::NotebookId {
    let setup_payload = PageCode::NotebookSetup {
        design_id: placeholder_design_id(),
    }
    .encode()
    .unwrap();
    core.create_notebook(CreateNotebookRequest {
        setup_payload,
        display_name: "Ordering evidence".to_string(),
        optional_color: None,
        optional_icon: None,
        optional_user_notes: None,
        make_active: true,
    })
    .unwrap()
    .notebook
    .id
}

fn resolve_page(
    core: &A2dCore,
    notebook_id: &a2d_domain::NotebookId,
    logical: u32,
    layout_id: &LayoutId,
) -> PageId {
    let payload = PageCode::NotebookPage {
        design_id: placeholder_design_id(),
        logical_page_number: logical,
        layout_id: layout_id.clone(),
    }
    .encode()
    .unwrap();
    match core.resolve_page_code(&payload, Some(notebook_id)).unwrap() {
        PageResolution::Resolved { page_id, .. } => page_id,
        other => panic!("expected resolved Notebook page, got {other:?}"),
    }
}

fn begin_recovery(
    core: &A2dCore,
    root: &Path,
    token: &str,
    notebook_id: a2d_domain::NotebookId,
    page_id: PageId,
    layout_id: LayoutId,
    captured_at_ms: i64,
) {
    let staging = root
        .join("tmp/scanner-staging")
        .join(format!("{token}.jpg"));
    std::fs::create_dir_all(staging.parent().unwrap()).unwrap();
    std::fs::write(&staging, b"captured-image").unwrap();
    core.begin_scanner_recovery(BeginScannerRecoveryRequest {
        token: token.to_string(),
        staging_path: staging.to_string_lossy().into_owned(),
        page_id,
        notebook_id,
        captured_at_ms,
        layout_id,
        processing_policy_version: 1,
    })
    .unwrap();
}

#[test]
fn out_of_order_batch_terminal_results_survive_reopen_without_cross_wiring_pages() {
    let (core, root) = open_core();
    let notebook_id = create_active_notebook(&core);
    let layout_id = placeholder_page_layout_id();
    let page_one = resolve_page(&core, &notebook_id, 1, &layout_id);
    let page_two = resolve_page(&core, &notebook_id, 2, &layout_id);

    core.begin_batch_scan_session(BeginBatchScanSessionRequest {
        session_id: "batch-ordering".to_string(),
        notebook_id: notebook_id.clone(),
    })
    .unwrap();
    begin_recovery(
        &core,
        &root,
        "capture-one",
        notebook_id.clone(),
        page_one.clone(),
        layout_id.clone(),
        1,
    );
    begin_recovery(
        &core,
        &root,
        "capture-two",
        notebook_id,
        page_two.clone(),
        layout_id,
        2,
    );
    core.queue_batch_scan_capture("batch-ordering", "capture-one")
        .unwrap();
    core.queue_batch_scan_capture("batch-ordering", "capture-two")
        .unwrap();

    let second_first = core
        .report_batch_scan_review(
            "batch-ordering",
            "capture-two",
            BatchScanReviewReason::ProcessingFailure,
            "second capture completed first".to_string(),
        )
        .unwrap();
    let one = second_first
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-one")
        .unwrap();
    let two = second_first
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-two")
        .unwrap();
    assert_eq!(one.status, BatchScanEntryStatus::Queued);
    assert_eq!(one.page_id, page_one);
    assert_eq!(two.status, BatchScanEntryStatus::NeedsReview);
    assert_eq!(two.page_id, page_two);
    assert!(two.review_item_id.is_some());

    drop(core);
    let reopened = A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    let after_reopen = reopened
        .reconcile_batch_scan_session("batch-ordering")
        .unwrap();
    let one = after_reopen
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-one")
        .unwrap();
    let two = after_reopen
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-two")
        .unwrap();
    assert_eq!(one.status, BatchScanEntryStatus::Queued);
    assert_eq!(one.page_id, page_one);
    assert_eq!(two.status, BatchScanEntryStatus::NeedsReview);
    assert_eq!(two.page_id, page_two);
    let two_review = two.review_item_id.clone().unwrap();

    let final_session = reopened
        .report_batch_scan_review(
            "batch-ordering",
            "capture-one",
            BatchScanReviewReason::IdentityFailure,
            "first capture completed after recreation".to_string(),
        )
        .unwrap();
    let one = final_session
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-one")
        .unwrap();
    let two = final_session
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-two")
        .unwrap();
    assert_eq!(one.status, BatchScanEntryStatus::NeedsReview);
    assert_eq!(one.page_id, page_one);
    assert!(one.review_item_id.is_some());
    assert_ne!(one.review_item_id.as_ref(), Some(&two_review));
    assert_eq!(two.status, BatchScanEntryStatus::NeedsReview);
    assert_eq!(two.page_id, page_two);
    assert_eq!(two.review_item_id.as_ref(), Some(&two_review));

    drop(reopened);
    std::fs::remove_dir_all(root).ok();
}
