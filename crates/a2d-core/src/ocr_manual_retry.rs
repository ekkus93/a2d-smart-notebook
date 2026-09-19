use super::{A2dCore, MAX_OCR_JOB_ATTEMPTS, OcrJobSnapshot};
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, ScanId, system_now_ms};
use a2d_storage::{OcrJobRepository, PersistedOcrJobStatus};

impl A2dCore {
    /// Re-queues the scan's latest terminal unavailable OCR job without minting a fresh job.
    ///
    /// The cumulative automatic/manual attempt budget is `MAX_OCR_JOB_ATTEMPTS`. A manual retry
    /// does not increment or reset `attempt_count`; the next durable claim increments it. Prior
    /// provider, run, and error diagnostics remain attached to the same job for audit/readback.
    pub fn manual_retry_ocr_job_for_scan(&self, scan_id: &str) -> Result<OcrJobSnapshot, A2dError> {
        let scan_id = ScanId::parse(scan_id)?;
        let now_ms = system_now_ms()?;
        let storage = self.lock_storage()?;

        if let Some(active) = storage.find_active_ocr_job_for_scan(&scan_id)? {
            return Err(manual_retry_error(
                "CORE_OCR_MANUAL_RETRY_ACTIVE_JOB",
                "manual retry is ineligible while the scan already has an active OCR job",
            )
            .with_detail("job_id", active.id.to_string())
            .with_detail("status", format!("{:?}", active.status)));
        }

        let mut job = storage
            .find_latest_ocr_job_for_scan(&scan_id)?
            .ok_or_else(|| {
                manual_retry_error(
                    "CORE_OCR_MANUAL_RETRY_JOB_MISSING",
                    "manual retry requires durable OCR job history for the scan",
                )
                .with_detail("scan_id", scan_id.to_string())
            })?;

        if job.status != PersistedOcrJobStatus::Unavailable {
            return Err(manual_retry_error(
                "CORE_OCR_MANUAL_RETRY_STATUS_INELIGIBLE",
                "only a terminal unavailable OCR job can be manually retried",
            )
            .with_detail("job_id", job.id.to_string())
            .with_detail("status", format!("{:?}", job.status)));
        }
        if job.attempt_count >= MAX_OCR_JOB_ATTEMPTS {
            return Err(manual_retry_error(
                "CORE_OCR_MANUAL_RETRY_ATTEMPTS_EXHAUSTED",
                "manual retry cannot exceed the Rust-owned OCR attempt budget",
            )
            .with_detail("job_id", job.id.to_string())
            .with_detail("attempt_count", job.attempt_count.to_string())
            .with_detail("max_attempts", MAX_OCR_JOB_ATTEMPTS.to_string()));
        }
        if !matches!(
            job.last_error_code.as_deref(),
            Some("OCR_PROVIDER_UNAVAILABLE" | "OCR_PROVIDER_FAILED" | "OCR_RESOURCE_UNAVAILABLE")
        ) {
            return Err(manual_retry_error(
                "CORE_OCR_MANUAL_RETRY_REASON_INELIGIBLE",
                "manual retry is limited to transient provider or resource unavailability",
            )
            .with_detail("job_id", job.id.to_string())
            .with_detail(
                "last_error_code",
                job.last_error_code
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
            ));
        }

        job.status = PersistedOcrJobStatus::Queued;
        job.updated_at_ms = now_ms;
        job.last_started_at_ms = None;
        job.completed_at_ms = None;
        job.retryable = true;
        job.next_retry_at_ms = Some(now_ms);
        job.cancellation_requested = false;
        storage.update_ocr_job(&job)?;
        Ok(job.into())
    }
}

fn manual_retry_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.manual_retry",
        message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoreOcrInputKind, EnqueueOcrJobRequest, OpenLibraryRequest};
    use a2d_domain::{
        Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, Page, PageId,
        PageKind, PageState, QualityStatus, Scan, ScanId, SmartPageId,
    };
    use a2d_storage::{AssetRepository, PageRepository, ScanRepository};
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
            "a2d-core-manual-retry-{label}-{}",
            PageId::generate()
        ));
        (open_core(&root), root)
    }

    fn insert_scan_fixture(core: &A2dCore) -> ScanFixture {
        let page_id = PageId::generate();
        let asset_id = AssetId::generate();
        let scan_id = ScanId::generate();
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("PAGE").unwrap(),
            Some("manual retry test page".to_string()),
            PageState::Scanned,
            100,
        );
        let asset = Asset::new(
            asset_id.clone(),
            AssetKind::Original,
            format!("assets/originals/{asset_id}.png"),
            "image/png".to_string(),
            1_024,
            "test-sha256".to_string(),
            100,
            true,
            EncryptionState::Plaintext,
        );
        let scan = Scan::new(
            scan_id.clone(),
            page_id,
            None,
            CaptureSource::Camera,
            125,
            asset_id,
            None,
            None,
            None,
            "test-pipeline".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            true,
            None,
            format!(
                "scan-content-v1;corrected-sha256={};perceptual=mean-grid-16x24-v1:{}",
                "a".repeat(64),
                "b".repeat(16 * 24 * 2)
            ),
        );
        let storage = core.lock_storage().unwrap();
        storage.insert_page(&page).unwrap();
        storage.insert_asset(&asset).unwrap();
        storage.insert_scan(&scan).unwrap();
        ScanFixture { scan_id }
    }

    fn enqueue_and_claim(core: &A2dCore, fixture: &ScanFixture) -> OcrJobSnapshot {
        core.enqueue_ocr_job(EnqueueOcrJobRequest {
            scan_id: fixture.scan_id.to_string(),
            input_kind: CoreOcrInputKind::Original,
            width_px: 1_000,
            height_px: 1_400,
        })
        .unwrap();
        core.claim_next_ocr_job().unwrap().unwrap()
    }

    fn make_terminal_unavailable(core: &A2dCore, job_id: &str, attempt_count: u32) {
        let job_id = a2d_domain::OcrJobId::parse(job_id).unwrap();
        let storage = core.lock_storage().unwrap();
        let mut job = storage.get_ocr_job(&job_id).unwrap().unwrap();
        job.status = PersistedOcrJobStatus::Unavailable;
        job.attempt_count = attempt_count;
        job.completed_at_ms = Some(job.created_at_ms);
        job.retryable = false;
        job.next_retry_at_ms = None;
        job.provider = Some("manual-retry-provider".to_string());
        job.provider_version = Some("7".to_string());
        job.last_error_code = Some("OCR_PROVIDER_FAILED".to_string());
        job.last_error_message = Some("transient provider failure".to_string());
        storage.update_ocr_job(&job).unwrap();
    }

    #[test]
    fn manual_retry_reuses_job_preserves_diagnostics_and_survives_restart() {
        let (core, root) = open_test_core("restart");
        let fixture = insert_scan_fixture(&core);
        let claimed = enqueue_and_claim(&core, &fixture);
        assert_eq!(claimed.attempt_count, 1);
        make_terminal_unavailable(&core, &claimed.job_id, 1);

        let retried = core
            .manual_retry_ocr_job_for_scan(&fixture.scan_id.to_string())
            .unwrap();
        assert_eq!(retried.job_id, claimed.job_id);
        assert_eq!(retried.status, super::super::CoreOcrJobStatus::Queued);
        assert_eq!(retried.attempt_count, 1);
        assert_eq!(retried.provider.as_deref(), Some("manual-retry-provider"));
        assert_eq!(retried.provider_version.as_deref(), Some("7"));
        assert_eq!(
            retried.last_error_code.as_deref(),
            Some("OCR_PROVIDER_FAILED")
        );
        assert_eq!(
            retried.last_error_message.as_deref(),
            Some("transient provider failure")
        );
        drop(core);

        let reopened = open_core(&root);
        let durable = reopened.get_ocr_job(&claimed.job_id).unwrap();
        assert_eq!(durable.status, super::super::CoreOcrJobStatus::Queued);
        assert_eq!(durable.attempt_count, 1);
        let second = reopened.claim_next_ocr_job().unwrap().unwrap();
        assert_eq!(second.job_id, claimed.job_id);
        assert_eq!(second.attempt_count, 2);

        drop(reopened);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn manual_retry_rejects_active_job() {
        let (core, root) = open_test_core("active");
        let fixture = insert_scan_fixture(&core);
        core.enqueue_ocr_job(EnqueueOcrJobRequest {
            scan_id: fixture.scan_id.to_string(),
            input_kind: CoreOcrInputKind::Original,
            width_px: 1_000,
            height_px: 1_400,
        })
        .unwrap();

        assert!(
            core.manual_retry_ocr_job_for_scan(&fixture.scan_id.to_string())
                .is_err()
        );
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn manual_retry_rejects_exhausted_attempt_budget() {
        let (core, root) = open_test_core("exhausted");
        let fixture = insert_scan_fixture(&core);
        let claimed = enqueue_and_claim(&core, &fixture);
        make_terminal_unavailable(&core, &claimed.job_id, MAX_OCR_JOB_ATTEMPTS);

        assert!(
            core.manual_retry_ocr_job_for_scan(&fixture.scan_id.to_string())
                .is_err()
        );
        let durable = core.get_ocr_job(&claimed.job_id).unwrap();
        assert_eq!(durable.status, super::super::CoreOcrJobStatus::Unavailable);
        assert_eq!(durable.attempt_count, MAX_OCR_JOB_ATTEMPTS);

        drop(core);
        std::fs::remove_dir_all(root).ok();
    }
}
