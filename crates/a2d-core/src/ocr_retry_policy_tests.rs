use super::{CoreOcrJobStatus, EnqueueOcrJobRequest, MAX_OCR_JOB_ATTEMPTS};
use crate::{
    A2dCore, CoreOcrFinalizationResolution, CoreOcrInputKind, FinalizeOcrJobRequest,
    OpenLibraryRequest,
};
use a2d_domain::{
    Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRunStatus,
    OcrUnavailableReason, Page, PageId, PageKind, PageState, QualityStatus, Scan, ScanId,
    SmartPageId,
};
use a2d_storage::{AssetRepository, OcrJobRepository, PageRepository, ScanRepository};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct ScanFixture {
    scan_id: ScanId,
}

fn open_core(root: &Path) -> Arc<A2dCore> {
    A2dCore::open(OpenLibraryRequest {
        library_path: root.to_string_lossy().into_owned(),
    })
    .unwrap()
}

fn open_test_core(label: &str) -> (Arc<A2dCore>, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "a2d-core-ocr-retry-policy-{label}-{}",
        PageId::generate()
    ));
    (open_core(&root), root)
}

fn asset(id: AssetId) -> Asset {
    Asset::new(
        id.clone(),
        AssetKind::Original,
        format!("assets/originals/{id}.png"),
        "image/png".to_string(),
        1_024,
        "test-sha256".to_string(),
        100,
        true,
        EncryptionState::Plaintext,
    )
}

fn scan_fingerprint() -> String {
    format!(
        "scan-content-v1;corrected-sha256={};perceptual=mean-grid-16x24-v1:{}",
        "a".repeat(64),
        "b".repeat(16 * 24 * 2)
    )
}

fn insert_scan_fixture(core: &A2dCore) -> ScanFixture {
    let page_id = PageId::generate();
    let original_asset_id = AssetId::generate();
    let scan_id = ScanId::generate();
    let page = Page::new(
        page_id.clone(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: Some(1),
        },
        LayoutId::parse("PAGE").unwrap(),
        Some("OCR retry policy page".to_string()),
        PageState::Scanned,
        100,
    );
    let scan = Scan::new(
        scan_id.clone(),
        page_id,
        None,
        CaptureSource::Camera,
        125,
        original_asset_id.clone(),
        None,
        None,
        None,
        "test-pipeline".to_string(),
        QualityStatus::Accepted,
        Vec::new(),
        true,
        None,
        scan_fingerprint(),
    );
    let storage = core.lock_storage().unwrap();
    storage.insert_page(&page).unwrap();
    storage.insert_asset(&asset(original_asset_id)).unwrap();
    storage.insert_scan(&scan).unwrap();
    ScanFixture { scan_id }
}

fn enqueue(core: &A2dCore, fixture: &ScanFixture) -> String {
    core.enqueue_ocr_job(EnqueueOcrJobRequest {
        scan_id: fixture.scan_id.to_string(),
        input_kind: CoreOcrInputKind::Original,
        width_px: 1_000,
        height_px: 1_400,
    })
    .unwrap()
    .job_id
}

fn finalize_retryable_unavailable(
    core: &A2dCore,
    job_id: &str,
    attempt_count: u32,
) -> crate::FinalizedOcrJob {
    core.finalize_ocr_job(FinalizeOcrJobRequest {
        job_id: job_id.to_string(),
        attempt_count,
        provider: "retry-policy-test".to_string(),
        provider_version: "1".to_string(),
        model_name: None,
        status: OcrRunStatus::Unavailable,
        full_text: String::new(),
        unavailable_reason: Some(OcrUnavailableReason::ProviderUnavailable),
        unavailable_message: Some("provider temporarily unavailable".to_string()),
        completed_at_ms: None,
        warnings: Vec::new(),
        retryable: true,
        regions: Vec::new(),
    })
    .unwrap()
}

fn make_retry_due_now(core: &A2dCore, job_id: &str) {
    let job_id = a2d_domain::OcrJobId::parse(job_id).unwrap();
    let storage = core.lock_storage().unwrap();
    let mut job = storage.get_ocr_job(&job_id).unwrap().unwrap();
    job.next_retry_at_ms = Some(0);
    storage.update_ocr_job(&job).unwrap();
}

#[test]
fn automatic_retry_claims_exactly_three_attempts_then_exhausts_terminally() {
    let (core, root) = open_test_core("exhaustion");
    let fixture = insert_scan_fixture(&core);
    let job_id = enqueue(&core, &fixture);

    for expected_attempt in 1..=MAX_OCR_JOB_ATTEMPTS {
        let claimed = core.claim_next_ocr_job().unwrap().unwrap();
        assert_eq!(claimed.job_id, job_id);
        assert_eq!(claimed.attempt_count, expected_attempt);

        let finalized =
            finalize_retryable_unavailable(&core, &job_id, claimed.attempt_count);
        if expected_attempt < MAX_OCR_JOB_ATTEMPTS {
            assert_eq!(
                finalized.resolution,
                CoreOcrFinalizationResolution::RetryScheduled
            );
            assert_eq!(finalized.job.status, CoreOcrJobStatus::Queued);
            assert!(finalized.job.retryable);
            make_retry_due_now(&core, &job_id);
        } else {
            assert_eq!(
                finalized.resolution,
                CoreOcrFinalizationResolution::TerminalUnavailable
            );
            assert_eq!(finalized.job.status, CoreOcrJobStatus::Unavailable);
            assert!(!finalized.job.retryable);
            assert!(finalized.job.next_retry_at_ms.is_none());
        }
    }

    assert!(core.claim_next_ocr_job().unwrap().is_none());
    let exhausted = core.get_ocr_job(&job_id).unwrap();
    assert_eq!(exhausted.attempt_count, MAX_OCR_JOB_ATTEMPTS);
    assert_eq!(exhausted.status, CoreOcrJobStatus::Unavailable);
    assert!(!exhausted.retryable);

    drop(core);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn attempt_count_and_retry_eligibility_survive_library_restart() {
    let (core, root) = open_test_core("restart");
    let fixture = insert_scan_fixture(&core);
    let job_id = enqueue(&core, &fixture);
    let first = core.claim_next_ocr_job().unwrap().unwrap();
    assert_eq!(first.attempt_count, 1);
    let finalized = finalize_retryable_unavailable(&core, &job_id, first.attempt_count);
    assert_eq!(
        finalized.resolution,
        CoreOcrFinalizationResolution::RetryScheduled
    );
    make_retry_due_now(&core, &job_id);
    drop(core);

    let reopened = open_core(&root);
    let durable = reopened.get_ocr_job(&job_id).unwrap();
    assert_eq!(durable.attempt_count, 1);
    assert_eq!(durable.status, CoreOcrJobStatus::Queued);
    assert!(durable.retryable);

    let second = reopened.claim_next_ocr_job().unwrap().unwrap();
    assert_eq!(second.job_id, job_id);
    assert_eq!(second.attempt_count, 2);

    drop(reopened);
    std::fs::remove_dir_all(root).ok();
}
