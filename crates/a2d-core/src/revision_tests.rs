use std::path::PathBuf;
use std::sync::Arc;

use a2d_domain::{
    AssetKind, CaptureSource, LayoutId, Page, PageId, PageKind, PageState, QualityStatus, Scan,
    ScanId, SmartPageId,
};
use a2d_image::PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT;
use a2d_storage::{AssetRepository, PageRepository, ScanRepository};

use super::{
    A2dCore, GetScanRevisionProposalRequest, OpenLibraryRequest, ResolveScanRevisionRequest,
    ScanRevisionDecision,
};

struct Fixture {
    core: Arc<A2dCore>,
    root: PathBuf,
    page_id: PageId,
    baseline_id: ScanId,
    candidate_id: ScanId,
}

fn fingerprint(corrected_sha256: &str, changed_cell: Option<(usize, u8)>) -> String {
    let mut cells = vec![180_u8; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT];
    if let Some((index, value)) = changed_cell {
        cells[index] = value;
    }
    let payload = cells
        .iter()
        .map(|cell| format!("{cell:02x}"))
        .collect::<String>();
    format!(
        "scan-content-v1;corrected-sha256={corrected_sha256};perceptual=mean-grid-16x24-v1:{payload}"
    )
}

fn fixture(candidate_changed_cell: Option<(usize, u8)>) -> Fixture {
    let root = std::env::temp_dir().join(format!("a2d-core-revision-{}", PageId::generate()));
    let core = A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap();
    let page_id = PageId::generate();
    core.lock_storage()
        .unwrap()
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

    let baseline_original = core
        .asset_store
        .commit(b"baseline-original", AssetKind::Original, "image/jpeg")
        .unwrap();
    let baseline_corrected = core
        .asset_store
        .commit(b"baseline-corrected", AssetKind::Corrected, "image/png")
        .unwrap();
    let candidate_original = core
        .asset_store
        .commit(b"candidate-original", AssetKind::Original, "image/jpeg")
        .unwrap();
    let candidate_corrected_bytes: &[u8] = if candidate_changed_cell.is_none() {
        b"baseline-corrected"
    } else {
        b"candidate-corrected"
    };
    let candidate_corrected = core
        .asset_store
        .commit(candidate_corrected_bytes, AssetKind::Corrected, "image/png")
        .unwrap();
    let baseline_id = ScanId::generate();
    let candidate_id = ScanId::generate();
    let baseline = Scan::new(
        baseline_id.clone(),
        page_id.clone(),
        None,
        CaptureSource::Camera,
        200,
        baseline_original.id().clone(),
        Some(baseline_corrected.id().clone()),
        None,
        None,
        "pipeline-v1".to_string(),
        QualityStatus::Accepted,
        Vec::new(),
        true,
        None,
        fingerprint(&baseline_corrected.sha256, None),
    );
    let candidate = Scan::new(
        candidate_id.clone(),
        page_id.clone(),
        None,
        CaptureSource::Camera,
        300,
        candidate_original.id().clone(),
        Some(candidate_corrected.id().clone()),
        None,
        None,
        "pipeline-v1".to_string(),
        QualityStatus::NeedsReview,
        vec!["EXISTING_PAGE_SCAN_REQUIRES_REVIEW".to_string()],
        false,
        None,
        fingerprint(&candidate_corrected.sha256, candidate_changed_cell),
    );
    core.lock_storage()
        .unwrap()
        .transaction(|tx| {
            tx.insert_asset(&baseline_original)?;
            tx.insert_asset(&baseline_corrected)?;
            tx.insert_asset(&candidate_original)?;
            tx.insert_asset(&candidate_corrected)?;
            tx.insert_scan(&baseline)?;
            tx.insert_scan(&candidate)?;
            Ok(())
        })
        .unwrap();

    Fixture {
        core,
        root,
        page_id,
        baseline_id,
        candidate_id,
    }
}

fn proposal(fixture: &Fixture) -> super::ScanRevisionProposal {
    fixture
        .core
        .get_scan_revision_proposal(GetScanRevisionProposalRequest {
            candidate_scan_id: fixture.candidate_id.clone(),
            minimum_cell_absolute_difference: 20,
        })
        .unwrap()
}

#[test]
fn proposal_defaults_to_preserving_a_new_version_and_exposes_all_smart_page_choices() {
    let fixture = fixture(Some((17, 40)));
    let proposal = proposal(&fixture);

    assert_eq!(proposal.page_id, fixture.page_id);
    assert_eq!(proposal.baseline_scan_id, fixture.baseline_id);
    assert_eq!(proposal.candidate_scan_id, fixture.candidate_id);
    assert_eq!(
        proposal.default_decision,
        ScanRevisionDecision::SaveAsNewVersion
    );
    assert_eq!(
        proposal.allowed_decisions,
        vec![
            ScanRevisionDecision::SaveAsNewVersion,
            ScanRevisionDecision::ReplacePreferred,
            ScanRevisionDecision::AnotherPhysicalCopy,
            ScanRevisionDecision::WrongScan,
        ]
    );
    assert!(!proposal.comparison.exact_content_match);

    std::fs::remove_dir_all(fixture.root).ok();
}

#[test]
fn save_as_new_version_keeps_the_baseline_preferred_and_links_the_candidate() {
    let fixture = fixture(None);
    let proposal = proposal(&fixture);
    let resolved = fixture
        .core
        .resolve_scan_revision(ResolveScanRevisionRequest {
            page_id: proposal.page_id,
            baseline_scan_id: proposal.baseline_scan_id,
            candidate_scan_id: proposal.candidate_scan_id,
            decision: ScanRevisionDecision::SaveAsNewVersion,
            decided_at_ms: 400,
            actor: "core-test".to_string(),
            physical_copy_label: None,
        })
        .unwrap();

    assert!(resolved.changed);
    assert_eq!(resolved.preferred_scan_id, fixture.baseline_id);
    assert!(!resolved.committed_data_deleted);
    let candidate = fixture
        .core
        .lock_storage()
        .unwrap()
        .get_scan(&fixture.candidate_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        candidate.supersedes_scan_id,
        Some(fixture.baseline_id.clone())
    );
    assert!(!candidate.preferred);

    std::fs::remove_dir_all(fixture.root).ok();
}

#[test]
fn replace_preferred_routes_through_the_atomic_preference_workflow() {
    let fixture = fixture(None);
    let proposal = proposal(&fixture);
    let resolved = fixture
        .core
        .resolve_scan_revision(ResolveScanRevisionRequest {
            page_id: proposal.page_id,
            baseline_scan_id: proposal.baseline_scan_id,
            candidate_scan_id: proposal.candidate_scan_id,
            decision: ScanRevisionDecision::ReplacePreferred,
            decided_at_ms: 400,
            actor: "core-test".to_string(),
            physical_copy_label: None,
        })
        .unwrap();

    assert!(resolved.changed);
    assert_eq!(resolved.preferred_scan_id, fixture.candidate_id);
    assert!(resolved.audit_event_id.is_some());
    let storage = fixture.core.lock_storage().unwrap();
    let page = storage.get_page(&fixture.page_id).unwrap().unwrap();
    let baseline = storage.get_scan(&fixture.baseline_id).unwrap().unwrap();
    let candidate = storage.get_scan(&fixture.candidate_id).unwrap().unwrap();
    assert_eq!(page.preferred_scan_id, Some(fixture.candidate_id.clone()));
    assert!(!baseline.preferred);
    assert!(candidate.preferred);
    drop(storage);

    std::fs::remove_dir_all(fixture.root).ok();
}
