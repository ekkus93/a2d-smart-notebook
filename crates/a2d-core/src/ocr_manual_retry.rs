use super::{A2dCore, MAX_OCR_JOB_ATTEMPTS, OcrJobSnapshot};
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, ScanId, system_now_ms};
use a2d_storage::{OcrJobRepository, PersistedOcrJobStatus};

impl A2dCore {
    /// Re-queues the scan's latest terminal unavailable OCR job without minting a fresh job.
    ///
    /// The cumulative automatic/manual attempt budget is `MAX_OCR_JOB_ATTEMPTS`. A manual retry
    /// does not increment or reset `attempt_count`; the next durable claim increments it. Prior
    /// provider, run, and error diagnostics remain attached to the same job for audit/readback.
    pub fn manual_retry_ocr_job_for_scan(
        &self,
        scan_id: &str,
    ) -> Result<OcrJobSnapshot, A2dError> {
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
