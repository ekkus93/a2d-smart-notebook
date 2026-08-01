use std::path::PathBuf;

use a2d_domain::{
    AssetKind, AuditEventId, CaptureSource, LayoutId, Page, PageId, PageKind, PageState,
    QualityStatus, Scan, ScanId, SmartPageId,
};
use a2d_storage::{
    AssetRepository, AssetStore, AuditEventRepository, PageRepository,
    RecordScanRevisionDecisionRequest, ScanRepository, Storage, StoredScanRevisionDecision,
};

struct Fixture {
    storage: Storage,
    assets: AssetStore,
    root: PathBuf,
    page_id: PageId,
    baseline_id: ScanId,
    candidate_id: ScanId,
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!("a2d-scan-revision-{}", PageId::generate()));
    let assets = AssetStore::open(&root).unwrap();
    let storage = Storage::open_in_memory().unwrap();
    let page_id = PageId::generate();
    storage
        .insert_page(&Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("USLETTER-LINED").unwrap(),
            None,
            PageState::Scanned,
            100,
        ))
        .unwrap();

    let baseline_original = assets
        .commit(b"baseline-original", AssetKind::Original, "image/jpeg")
        .unwrap();
    let candidate_original = assets
        .commit(b"candidate-original", AssetKind::Original, "image/jpeg")
        .unwrap();
    storage.insert_asset(&baseline_original).unwrap();
    storage.insert_asset(&candidate_original).unwrap();

    let baseline_id = ScanId::generate();
    storage
        .insert_scan(&Scan::new(
            baseline_id.clone(),
            page_id.clone(),
            None,
            CaptureSource::Camera,
            200,
            baseline_original.id().clone(),
            None,
            None,
            None,
            "pipeline-v1".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            true,
            None,
            "baseline-fingerprint".to_string(),
        ))
        .unwrap();
    let candidate_id = ScanId::generate();
    storage
        .insert_scan(&Scan::new(
            candidate_id.clone(),
            page_id.clone(),
            None,
            CaptureSource::Camera,
            300,
            candidate_original.id().clone(),
            None,
            None,
            None,
            "pipeline-v1".to_string(),
            QualityStatus::NeedsReview,
            vec!["EXISTING_PAGE_SCAN_REQUIRES_REVIEW".to_string()],
            false,
            None,
            "candidate-fingerprint".to_string(),
        ))
        .unwrap();

    Fixture {
        storage,
        assets,
        root,
        page_id,
        baseline_id,
        candidate_id,
    }
}

fn request(
    fixture: &Fixture,
    decision: StoredScanRevisionDecision,
) -> RecordScanRevisionDecisionRequest {
    RecordScanRevisionDecisionRequest {
        page_id: fixture.page_id.clone(),
        baseline_scan_id: fixture.baseline_id.clone(),
        candidate_scan_id: fixture.candidate_id.clone(),
        decision,
        decided_at_ms: 400,
        actor: "integration-test".to_string(),
        operation_id: AuditEventId::generate(),
        physical_copy_label: None,
    }
}

#[test]
fn save_as_new_version_links_scans_without_replacing_or_deleting_evidence() {
    let mut fixture = fixture();
    let candidate_before = fixture
        .storage
        .get_scan(&fixture.candidate_id)
        .unwrap()
        .unwrap();
    let original_before = fixture
        .storage
        .get_asset(&candidate_before.original_asset_id)
        .unwrap()
        .unwrap();

    let decision = request(&fixture, StoredScanRevisionDecision::SaveAsNewVersion);
    let result = fixture
        .storage
        .record_scan_revision_decision(decision)
        .unwrap();

    assert!(result.changed);
    assert!(result.audit_event_id.is_some());
    let page = fixture.storage.get_page(&fixture.page_id).unwrap().unwrap();
    assert_eq!(page.preferred_scan_id, Some(fixture.baseline_id.clone()));
    let candidate = fixture
        .storage
        .get_scan(&fixture.candidate_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        candidate.supersedes_scan_id,
        Some(fixture.baseline_id.clone())
    );
    assert!(!candidate.preferred);
    assert!(
        candidate
            .warnings
            .iter()
            .any(|warning| warning == "REVISION_DECISION_SAVE_AS_NEW_VERSION")
    );
    assert!(
        !candidate
            .warnings
            .iter()
            .any(|warning| warning == "EXISTING_PAGE_SCAN_REQUIRES_REVIEW")
    );
    assert_eq!(candidate.original_asset_id, original_before.id().clone());
    fixture.assets.verify(&original_before).unwrap();
    let audit = fixture
        .storage
        .get_audit_event(result.audit_event_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(audit.event_kind, "scan.revision_saved");
    assert_eq!(
        audit
            .details
            .get("committed_data_deleted")
            .map(String::as_str),
        Some("false")
    );

    std::fs::remove_dir_all(fixture.root).ok();
}

#[test]
fn another_physical_copy_assigns_distinct_explicit_copy_ids() {
    let mut fixture = fixture();
    let mut decision = request(&fixture, StoredScanRevisionDecision::AnotherPhysicalCopy);
    decision.physical_copy_label = Some("Second printout".to_string());
    let result = fixture
        .storage
        .record_scan_revision_decision(decision)
        .unwrap();

    let baseline = fixture
        .storage
        .get_scan(&fixture.baseline_id)
        .unwrap()
        .unwrap();
    let candidate = fixture
        .storage
        .get_scan(&fixture.candidate_id)
        .unwrap()
        .unwrap();
    assert!(baseline.physical_copy_id.is_some());
    assert_eq!(
        candidate.physical_copy_id,
        result.candidate_physical_copy_id
    );
    assert_ne!(baseline.physical_copy_id, candidate.physical_copy_id);
    assert_eq!(candidate.supersedes_scan_id, None);
    assert_eq!(
        fixture
            .storage
            .get_page(&fixture.page_id)
            .unwrap()
            .unwrap()
            .preferred_scan_id,
        Some(fixture.baseline_id.clone())
    );

    std::fs::remove_dir_all(fixture.root).ok();
}

#[test]
fn wrong_scan_is_a_logical_discard_and_keeps_the_committed_original() {
    let mut fixture = fixture();
    let candidate_before = fixture
        .storage
        .get_scan(&fixture.candidate_id)
        .unwrap()
        .unwrap();
    let original = fixture
        .storage
        .get_asset(&candidate_before.original_asset_id)
        .unwrap()
        .unwrap();

    let decision = request(&fixture, StoredScanRevisionDecision::WrongScan);
    let result = fixture
        .storage
        .record_scan_revision_decision(decision)
        .unwrap();

    assert!(result.changed);
    let candidate = fixture
        .storage
        .get_scan(&fixture.candidate_id)
        .unwrap()
        .unwrap();
    assert_eq!(candidate.quality_status, QualityStatus::Rejected);
    assert!(
        candidate
            .warnings
            .iter()
            .any(|warning| warning == "REVISION_DECISION_WRONG_SCAN_DISCARDED")
    );
    assert_eq!(candidate.original_asset_id, original.id().clone());
    fixture.assets.verify(&original).unwrap();
    assert_eq!(
        fixture
            .storage
            .get_page(&fixture.page_id)
            .unwrap()
            .unwrap()
            .preferred_scan_id,
        Some(fixture.baseline_id.clone())
    );

    std::fs::remove_dir_all(fixture.root).ok();
}

#[test]
fn repeated_matching_decision_is_an_idempotent_no_op() {
    let mut fixture = fixture();
    let first_request = request(&fixture, StoredScanRevisionDecision::SaveAsNewVersion);
    let first = fixture
        .storage
        .record_scan_revision_decision(first_request)
        .unwrap();
    let second_request = request(&fixture, StoredScanRevisionDecision::SaveAsNewVersion);
    let second = fixture
        .storage
        .record_scan_revision_decision(second_request)
        .unwrap();
    assert!(first.changed);
    assert!(!second.changed);
    assert_eq!(second.audit_event_id, None);

    std::fs::remove_dir_all(fixture.root).ok();
}
