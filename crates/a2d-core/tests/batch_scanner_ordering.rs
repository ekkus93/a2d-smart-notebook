use std::path::{Path, PathBuf};

use a2d_core::{
    A2dCore, BatchScanEntryStatus, BeginBatchScanSessionRequest, BeginScannerRecoveryRequest,
    CreateNotebookRequest, OpenLibraryRequest, PageResolution,
};
use a2d_domain::{LayoutId, NotebookDesignId, PageId, ScanId};
use a2d_identity::PageCode;

const DESIGN_ID: &str = "6DE28E53DBKPXCWWNHPC8T7QJX";
const LAYOUT_ID: &str = "USLETTER-LINED";

fn open_core() -> (std::sync::Arc<A2dCore>, PathBuf) {
    let root = std::env::temp_dir().join(format!("a2d-batch-ordering-{}", PageId::generate()));
    let core = A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    (core, root)
}

fn create_active_notebook(core: &A2dCore) -> a2d_domain::NotebookId {
    let setup_payload = PageCode::NotebookSetup {
        design_id: NotebookDesignId::parse(DESIGN_ID).unwrap(),
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

fn resolve_page(core: &A2dCore, notebook_id: &a2d_domain::NotebookId, logical: u32) -> PageId {
    let payload = PageCode::NotebookPage {
        design_id: NotebookDesignId::parse(DESIGN_ID).unwrap(),
        logical_page_number: logical,
        layout_id: LayoutId::parse(LAYOUT_ID).unwrap(),
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
        layout_id: LayoutId::parse(LAYOUT_ID).unwrap(),
        processing_policy_version: 1,
    })
    .unwrap();
}

fn mark_committed(core: &A2dCore, token: &str) -> ScanId {
    let recovery = core
        .list_scanner_recoveries()
        .unwrap()
        .into_iter()
        .find(|record| record.token == token)
        .unwrap();
    core.mark_scanner_recovery_preview_ready(token).unwrap();
    core.mark_scanner_recovery_registering(
        token,
        Path::new(&recovery.staging_path),
        &recovery.page_id,
        &recovery.notebook_id,
        &recovery.layout_id,
        recovery.processing_policy_version,
    )
    .unwrap();
    let scan_id = ScanId::generate();
    core.mark_scanner_recovery_committed(token, &scan_id)
        .unwrap();
    scan_id
}

#[test]
fn out_of_order_recovery_completion_survives_reopen_without_cross_wiring_pages() {
    let (core, root) = open_core();
    let notebook_id = create_active_notebook(&core);
    let page_one = resolve_page(&core, &notebook_id, 1);
    let page_two = resolve_page(&core, &notebook_id, 2);

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
        1,
    );
    begin_recovery(
        &core,
        &root,
        "capture-two",
        notebook_id,
        page_two.clone(),
        2,
    );
    core.queue_batch_scan_capture("batch-ordering", "capture-one")
        .unwrap();
    core.queue_batch_scan_capture("batch-ordering", "capture-two")
        .unwrap();

    let scan_two = mark_committed(&core, "capture-two");
    let first_reconcile = core.reconcile_batch_scan_session("batch-ordering").unwrap();
    let one = first_reconcile
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-one")
        .unwrap();
    let two = first_reconcile
        .entries
        .iter()
        .find(|entry| entry.recovery_token == "capture-two")
        .unwrap();
    assert_eq!(one.status, BatchScanEntryStatus::Queued);
    assert_eq!(one.page_id, page_one);
    assert_eq!(two.status, BatchScanEntryStatus::Saved);
    assert_eq!(two.page_id, page_two);
    assert_eq!(two.registered_scan_id, Some(scan_two.clone()));

    drop(core);
    let reopened = A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    let after_reopen = reopened.reconcile_batch_scan_session("batch-ordering").unwrap();
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
    assert_eq!(two.status, BatchScanEntryStatus::Saved);
    assert_eq!(two.page_id, page_two);
    assert_eq!(two.registered_scan_id, Some(scan_two));

    let scan_one = mark_committed(&reopened, "capture-one");
    let final_session = reopened.reconcile_batch_scan_session("batch-ordering").unwrap();
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
    assert_eq!(one.status, BatchScanEntryStatus::Saved);
    assert_eq!(one.page_id, page_one);
    assert_eq!(one.registered_scan_id, Some(scan_one));
    assert_eq!(two.status, BatchScanEntryStatus::Saved);
    assert_eq!(two.page_id, page_two);

    drop(reopened);
    std::fs::remove_dir_all(root).ok();
}
