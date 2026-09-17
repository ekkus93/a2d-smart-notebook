use a2d_core as core;

use super::{A2dClient, A2dFfiError, OcrInputKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum OcrQueueJobStatus {
    Queued,
    Running,
    Recognized,
    Unavailable,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum OcrProviderAvailability {
    Unknown,
    Available,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct EnqueueOcrJobRequest {
    pub scan_id: String,
    pub input_kind: OcrInputKind,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct CompleteOcrJobRequest {
    pub job_id: String,
    pub ocr_run_id: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct OcrQueueJob {
    pub job_id: String,
    pub scan_id: String,
    pub input_asset_id: String,
    pub input_kind: OcrInputKind,
    pub media_type: String,
    pub relative_path: String,
    pub byte_length: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub status: OcrQueueJobStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub last_started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub attempt_count: u32,
    pub retryable: bool,
    pub next_retry_at_ms: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub provider: Option<String>,
    pub provider_version: Option<String>,
    pub model_name: Option<String>,
    pub provider_availability: OcrProviderAvailability,
    pub last_ocr_run_id: Option<String>,
    pub cancellation_requested: bool,
}

impl From<core::OcrJobSnapshot> for OcrQueueJob {
    fn from(value: core::OcrJobSnapshot) -> Self {
        Self {
            job_id: value.job_id,
            scan_id: value.scan_id,
            input_asset_id: value.input_asset_id,
            input_kind: value.input_kind.into(),
            media_type: value.media_type,
            relative_path: value.relative_path,
            byte_length: value.byte_length,
            width_px: value.width_px,
            height_px: value.height_px,
            status: match value.status {
                core::CoreOcrJobStatus::Queued => OcrQueueJobStatus::Queued,
                core::CoreOcrJobStatus::Running => OcrQueueJobStatus::Running,
                core::CoreOcrJobStatus::Recognized => OcrQueueJobStatus::Recognized,
                core::CoreOcrJobStatus::Unavailable => OcrQueueJobStatus::Unavailable,
                core::CoreOcrJobStatus::Cancelled => OcrQueueJobStatus::Cancelled,
            },
            created_at_ms: value.created_at_ms,
            updated_at_ms: value.updated_at_ms,
            last_started_at_ms: value.last_started_at_ms,
            completed_at_ms: value.completed_at_ms,
            attempt_count: value.attempt_count,
            retryable: value.retryable,
            next_retry_at_ms: value.next_retry_at_ms,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            provider: value.provider,
            provider_version: value.provider_version,
            model_name: value.model_name,
            provider_availability: match value.provider_availability {
                core::CoreOcrProviderAvailability::Unknown => OcrProviderAvailability::Unknown,
                core::CoreOcrProviderAvailability::Available => OcrProviderAvailability::Available,
                core::CoreOcrProviderAvailability::Unavailable => {
                    OcrProviderAvailability::Unavailable
                }
                core::CoreOcrProviderAvailability::Failed => OcrProviderAvailability::Failed,
            },
            last_ocr_run_id: value.last_ocr_run_id,
            cancellation_requested: value.cancellation_requested,
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn enqueue_ocr_job(
        &self,
        request: EnqueueOcrJobRequest,
    ) -> Result<OcrQueueJob, A2dFfiError> {
        self.core
            .enqueue_ocr_job(core::EnqueueOcrJobRequest {
                scan_id: request.scan_id,
                input_kind: request.input_kind.into(),
                width_px: request.width_px,
                height_px: request.height_px,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn find_active_ocr_job_for_scan(
        &self,
        request: EnqueueOcrJobRequest,
    ) -> Result<Option<OcrQueueJob>, A2dFfiError> {
        self.core
            .find_active_ocr_job_for_scan(core::EnqueueOcrJobRequest {
                scan_id: request.scan_id,
                input_kind: request.input_kind.into(),
                width_px: request.width_px,
                height_px: request.height_px,
            })
            .map(|job| job.map(Into::into))
            .map_err(Into::into)
    }

    pub fn claim_next_ocr_job(&self) -> Result<Option<OcrQueueJob>, A2dFfiError> {
        self.core
            .claim_next_ocr_job()
            .map(|job| job.map(Into::into))
            .map_err(Into::into)
    }

    pub fn get_ocr_job(&self, job_id: String) -> Result<OcrQueueJob, A2dFfiError> {
        self.core
            .get_ocr_job(&job_id)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn request_ocr_job_cancellation(&self, job_id: String) -> Result<OcrQueueJob, A2dFfiError> {
        self.core
            .request_ocr_job_cancellation(&job_id)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn complete_ocr_job(
        &self,
        request: CompleteOcrJobRequest,
    ) -> Result<OcrQueueJob, A2dFfiError> {
        self.core
            .complete_ocr_job(core::CompleteOcrJobRequest {
                job_id: request.job_id,
                ocr_run_id: request.ocr_run_id,
                retryable: request.retryable,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::OpenLibraryRequest;

    fn open_test_client() -> Arc<A2dClient> {
        let dir = std::env::temp_dir().join(format!(
            "a2d-ffi-ocr-queue-test-{}",
            a2d_domain::PageId::generate()
        ));
        A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .expect("open must succeed for a fresh directory")
    }

    #[test]
    fn missing_queue_job_preserves_typed_rust_error() {
        let client = open_test_client();
        let err = client
            .get_ocr_job(a2d_domain::OcrJobId::generate().to_string())
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;
        assert_eq!(details.code, "CORE_OCR_JOB_MISSING");
        assert_eq!(details.category, "Ocr");
        assert!(!details.retryable);
    }
}
