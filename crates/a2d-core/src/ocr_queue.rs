use crate::{A2dCore, CoreOcrInputKind, PrepareOcrInputRequest, PreparedOcrInput};
use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, OcrJobId, OcrRunStatus,
    OcrUnavailableReason, system_now_ms,
};
use a2d_storage::{
    OcrJobRepository, OcrRunRepository, PersistedOcrInputKind, PersistedOcrJob,
    PersistedOcrJobStatus, PersistedOcrProviderAvailability, Storage,
};

pub const MAX_ACTIVE_OCR_JOBS: usize = 64;
pub const MAX_OCR_JOB_ATTEMPTS: u32 = 3;
const BASE_RETRY_DELAY_MS: i64 = 2_000;
const MAX_RETRY_DELAY_MS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CoreOcrJobStatus {
    Queued,
    Running,
    Recognized,
    Unavailable,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CoreOcrProviderAvailability {
    Unknown,
    Available,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnqueueOcrJobRequest {
    pub scan_id: String,
    pub input_kind: CoreOcrInputKind,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteOcrJobRequest {
    pub job_id: String,
    pub ocr_run_id: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrJobSnapshot {
    pub job_id: String,
    pub scan_id: String,
    pub input_asset_id: String,
    pub input_kind: CoreOcrInputKind,
    pub media_type: String,
    pub relative_path: String,
    pub byte_length: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub status: CoreOcrJobStatus,
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
    pub provider_availability: CoreOcrProviderAvailability,
    pub last_ocr_run_id: Option<String>,
    pub cancellation_requested: bool,
}

impl OcrJobSnapshot {
    pub fn prepared_input(&self) -> PreparedOcrInput {
        PreparedOcrInput {
            scan_id: self.scan_id.clone(),
            input_asset_id: self.input_asset_id.clone(),
            input_kind: self.input_kind,
            media_type: self.media_type.clone(),
            relative_path: self.relative_path.clone(),
            byte_length: self.byte_length,
            width_px: self.width_px,
            height_px: self.height_px,
        }
    }
}

impl A2dCore {
    pub fn enqueue_ocr_job(
        &self,
        request: EnqueueOcrJobRequest,
    ) -> Result<OcrJobSnapshot, A2dError> {
        let prepared = self.prepare_ocr_input(PrepareOcrInputRequest {
            scan_id: request.scan_id,
            input_kind: request.input_kind,
            width_px: request.width_px,
            height_px: request.height_px,
        })?;
        let now_ms = system_now_ms()?;
        let scan_id = a2d_domain::ScanId::parse(&prepared.scan_id)?;
        let input_asset_id = a2d_domain::AssetId::parse(&prepared.input_asset_id)?;
        let input_kind = to_persisted_input_kind(prepared.input_kind);
        let storage = self.lock_storage()?;

        if let Some(existing) =
            storage.find_active_ocr_job(&scan_id, &input_asset_id, input_kind)?
        {
            return Ok(existing.into());
        }
        let active_count = storage.count_active_ocr_jobs()?;
        if active_count >= MAX_ACTIVE_OCR_JOBS {
            return Err(queue_error(
                "CORE_OCR_QUEUE_CAPACITY_EXCEEDED",
                "OCR queue has reached its bounded active-job capacity",
                true,
            )
            .with_detail("active_count", active_count.to_string())
            .with_detail("max_active_jobs", MAX_ACTIVE_OCR_JOBS.to_string()));
        }

        let job = PersistedOcrJob {
            id: OcrJobId::try_generate()?,
            scan_id,
            input_asset_id,
            input_kind,
            media_type: prepared.media_type,
            relative_path: prepared.relative_path,
            byte_length: prepared.byte_length,
            width_px: prepared.width_px,
            height_px: prepared.height_px,
            status: PersistedOcrJobStatus::Queued,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            last_started_at_ms: None,
            completed_at_ms: None,
            attempt_count: 0,
            retryable: true,
            next_retry_at_ms: None,
            last_error_code: None,
            last_error_message: None,
            provider: None,
            provider_version: None,
            model_name: None,
            provider_availability: PersistedOcrProviderAvailability::Unknown,
            last_ocr_run_id: None,
            cancellation_requested: false,
        };
        storage.insert_ocr_job(&job)?;
        Ok(job.into())
    }

    pub fn claim_next_ocr_job(&self) -> Result<Option<OcrJobSnapshot>, A2dError> {
        let now_ms = system_now_ms()?;
        let storage = self.lock_storage()?;
        let Some(mut job) = storage.next_due_ocr_job(now_ms)? else {
            return Ok(None);
        };
        if job.attempt_count >= MAX_OCR_JOB_ATTEMPTS {
            return Err(queue_error(
                "CORE_OCR_JOB_ATTEMPT_LIMIT_STATE_INVALID",
                "queued OCR job already reached the orchestration attempt limit",
                false,
            )
            .with_detail("job_id", job.id.to_string())
            .with_detail("attempt_count", job.attempt_count.to_string()));
        }
        job.status = PersistedOcrJobStatus::Running;
        job.updated_at_ms = now_ms;
        job.last_started_at_ms = Some(now_ms);
        job.completed_at_ms = None;
        job.attempt_count = job.attempt_count.checked_add(1).ok_or_else(|| {
            queue_error(
                "CORE_OCR_JOB_ATTEMPT_COUNT_OVERFLOW",
                "OCR job attempt count overflowed",
                false,
            )
        })?;
        job.retryable = true;
        job.next_retry_at_ms = None;
        job.cancellation_requested = false;
        storage.update_ocr_job(&job)?;
        Ok(Some(job.into()))
    }

    pub fn get_ocr_job(&self, job_id: &str) -> Result<OcrJobSnapshot, A2dError> {
        let job_id = OcrJobId::parse(job_id)?;
        let storage = self.lock_storage()?;
        storage
            .get_ocr_job(&job_id)?
            .map(Into::into)
            .ok_or_else(|| missing_job_error(&job_id))
    }

    pub fn request_ocr_job_cancellation(&self, job_id: &str) -> Result<OcrJobSnapshot, A2dError> {
        let job_id = OcrJobId::parse(job_id)?;
        let now_ms = system_now_ms()?;
        let storage = self.lock_storage()?;
        let mut job = storage
            .get_ocr_job(&job_id)?
            .ok_or_else(|| missing_job_error(&job_id))?;
        match job.status {
            PersistedOcrJobStatus::Queued => {
                job.status = PersistedOcrJobStatus::Cancelled;
                job.updated_at_ms = now_ms;
                job.completed_at_ms = Some(now_ms);
                job.retryable = false;
                job.next_retry_at_ms = None;
                job.cancellation_requested = true;
                job.last_error_code = Some("OCR_CANCELLED".to_string());
                job.last_error_message =
                    Some("OCR job cancelled before provider execution".to_string());
                storage.update_ocr_job(&job)?;
            }
            PersistedOcrJobStatus::Running => {
                job.updated_at_ms = now_ms;
                job.cancellation_requested = true;
                storage.update_ocr_job(&job)?;
            }
            PersistedOcrJobStatus::Cancelled => {}
            PersistedOcrJobStatus::Recognized | PersistedOcrJobStatus::Unavailable => {
                return Err(queue_error(
                    "CORE_OCR_JOB_TERMINAL_TRANSITION_INVALID",
                    "completed OCR jobs cannot be cancelled",
                    false,
                )
                .with_detail("job_id", job.id.to_string()));
            }
        }
        Ok(job.into())
    }

    pub fn complete_ocr_job(
        &self,
        request: CompleteOcrJobRequest,
    ) -> Result<OcrJobSnapshot, A2dError> {
        let job_id = OcrJobId::parse(&request.job_id)?;
        let ocr_run_id = a2d_domain::OcrRunId::parse(&request.ocr_run_id)?;
        let now_ms = system_now_ms()?;
        let storage = self.lock_storage()?;
        let mut job = storage
            .get_ocr_job(&job_id)?
            .ok_or_else(|| missing_job_error(&job_id))?;
        if job.status != PersistedOcrJobStatus::Running {
            return Err(queue_error(
                "CORE_OCR_JOB_NOT_RUNNING",
                "only a running OCR job can accept a provider result",
                false,
            )
            .with_detail("job_id", job.id.to_string())
            .with_detail("status", format!("{:?}", job.status)));
        }
        let run = storage.get_ocr_run(&ocr_run_id)?.ok_or_else(|| {
            queue_error(
                "CORE_OCR_JOB_RUN_MISSING",
                "OCR queue completion requires the persisted OCR run",
                false,
            )
            .with_detail("ocr_run_id", ocr_run_id.to_string())
        })?;
        if run.scan_id != job.scan_id || run.input_asset_id.as_ref() != Some(&job.input_asset_id) {
            return Err(queue_error(
                "CORE_OCR_JOB_RUN_MISMATCH",
                "OCR run does not belong to the queued scan/input asset",
                false,
            )
            .with_detail("job_id", job.id.to_string())
            .with_detail("ocr_run_id", ocr_run_id.to_string()));
        }

        job.updated_at_ms = now_ms;
        job.provider = Some(run.provider.clone());
        job.provider_version = Some(run.provider_version.clone());
        job.model_name = run.model_name.clone();
        job.last_ocr_run_id = Some(ocr_run_id);

        if job.cancellation_requested
            || run.unavailable_reason == Some(OcrUnavailableReason::Cancelled)
        {
            job.status = PersistedOcrJobStatus::Cancelled;
            job.completed_at_ms = Some(now_ms);
            job.retryable = false;
            job.next_retry_at_ms = None;
            job.provider_availability = PersistedOcrProviderAvailability::Available;
            job.last_error_code = Some("OCR_CANCELLED".to_string());
            job.last_error_message = Some(
                run.unavailable_message
                    .unwrap_or_else(|| "OCR execution was cancelled".to_string()),
            );
            storage.update_ocr_job(&job)?;
            return Ok(job.into());
        }

        match run.status {
            OcrRunStatus::Detected | OcrRunStatus::NoTextDetected => {
                job.status = PersistedOcrJobStatus::Recognized;
                job.completed_at_ms = Some(now_ms);
                job.retryable = false;
                job.next_retry_at_ms = None;
                job.provider_availability = PersistedOcrProviderAvailability::Available;
                job.last_error_code = None;
                job.last_error_message = None;
            }
            OcrRunStatus::Unavailable => {
                let reason = run.unavailable_reason.ok_or_else(|| {
                    queue_error(
                        "CORE_OCR_JOB_UNAVAILABLE_REASON_MISSING",
                        "persisted unavailable OCR run is missing its reason",
                        false,
                    )
                })?;
                job.provider_availability = provider_availability_for(reason);
                job.last_error_code = Some(unavailable_error_code(reason).to_string());
                job.last_error_message = run.unavailable_message.clone().or_else(|| {
                    Some("OCR provider did not produce a terminal text result".to_string())
                });
                let retryable = request.retryable
                    && retryable_reason(reason)
                    && job.attempt_count < MAX_OCR_JOB_ATTEMPTS;
                if retryable {
                    job.status = PersistedOcrJobStatus::Queued;
                    job.last_started_at_ms = None;
                    job.completed_at_ms = None;
                    job.retryable = true;
                    job.next_retry_at_ms =
                        Some(now_ms.saturating_add(retry_delay_ms(job.attempt_count)));
                } else {
                    job.status = PersistedOcrJobStatus::Unavailable;
                    job.completed_at_ms = Some(now_ms);
                    job.retryable = false;
                    job.next_retry_at_ms = None;
                }
            }
        }
        storage.update_ocr_job(&job)?;
        Ok(job.into())
    }
}

pub(crate) fn recover_interrupted_ocr_jobs(storage: &Storage) -> Result<(), A2dError> {
    let now_ms = system_now_ms()?;
    for mut job in storage.list_running_ocr_jobs()? {
        job.updated_at_ms = now_ms;
        job.last_error_code = Some("OCR_EXECUTION_INTERRUPTED".to_string());
        job.last_error_message = Some(
            "OCR execution was interrupted before a terminal provider result was committed"
                .to_string(),
        );
        job.provider_availability = PersistedOcrProviderAvailability::Unknown;
        if job.cancellation_requested {
            job.status = PersistedOcrJobStatus::Cancelled;
            job.completed_at_ms = Some(now_ms);
            job.retryable = false;
            job.next_retry_at_ms = None;
        } else if job.attempt_count < MAX_OCR_JOB_ATTEMPTS {
            job.status = PersistedOcrJobStatus::Queued;
            job.last_started_at_ms = None;
            job.completed_at_ms = None;
            job.retryable = true;
            job.next_retry_at_ms = Some(now_ms);
        } else {
            job.status = PersistedOcrJobStatus::Unavailable;
            job.completed_at_ms = Some(now_ms);
            job.retryable = false;
            job.next_retry_at_ms = None;
        }
        storage.update_ocr_job(&job)?;
    }
    Ok(())
}

fn retry_delay_ms(attempt_count: u32) -> i64 {
    let shift = attempt_count.saturating_sub(1).min(4);
    BASE_RETRY_DELAY_MS
        .saturating_mul(1_i64 << shift)
        .min(MAX_RETRY_DELAY_MS)
}

fn retryable_reason(reason: OcrUnavailableReason) -> bool {
    matches!(
        reason,
        OcrUnavailableReason::ProviderUnavailable
            | OcrUnavailableReason::ProviderFailed
            | OcrUnavailableReason::ResourceUnavailable
    )
}

fn provider_availability_for(reason: OcrUnavailableReason) -> PersistedOcrProviderAvailability {
    match reason {
        OcrUnavailableReason::ProviderFailed => PersistedOcrProviderAvailability::Failed,
        OcrUnavailableReason::ProviderUnavailable | OcrUnavailableReason::ResourceUnavailable => {
            PersistedOcrProviderAvailability::Unavailable
        }
        OcrUnavailableReason::UnsupportedInput | OcrUnavailableReason::Cancelled => {
            PersistedOcrProviderAvailability::Available
        }
    }
}

fn unavailable_error_code(reason: OcrUnavailableReason) -> &'static str {
    match reason {
        OcrUnavailableReason::ProviderUnavailable => "OCR_PROVIDER_UNAVAILABLE",
        OcrUnavailableReason::ProviderFailed => "OCR_PROVIDER_FAILED",
        OcrUnavailableReason::ResourceUnavailable => "OCR_RESOURCE_UNAVAILABLE",
        OcrUnavailableReason::UnsupportedInput => "OCR_UNSUPPORTED_INPUT",
        OcrUnavailableReason::Cancelled => "OCR_CANCELLED",
    }
}

fn missing_job_error(job_id: &OcrJobId) -> A2dError {
    queue_error(
        "CORE_OCR_JOB_MISSING",
        "OCR queue operation requires an existing persisted job",
        false,
    )
    .with_detail("job_id", job_id.to_string())
}

fn queue_error(code: &'static str, message: impl Into<String>, retryable: bool) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.queue",
        message,
        retryable,
    )
}

fn to_persisted_input_kind(value: CoreOcrInputKind) -> PersistedOcrInputKind {
    match value {
        CoreOcrInputKind::Original => PersistedOcrInputKind::Original,
        CoreOcrInputKind::Corrected => PersistedOcrInputKind::Corrected,
        CoreOcrInputKind::OcrOptimized => PersistedOcrInputKind::OcrOptimized,
    }
}

fn from_persisted_input_kind(value: PersistedOcrInputKind) -> CoreOcrInputKind {
    match value {
        PersistedOcrInputKind::Original => CoreOcrInputKind::Original,
        PersistedOcrInputKind::Corrected => CoreOcrInputKind::Corrected,
        PersistedOcrInputKind::OcrOptimized => CoreOcrInputKind::OcrOptimized,
    }
}

impl From<PersistedOcrJob> for OcrJobSnapshot {
    fn from(job: PersistedOcrJob) -> Self {
        Self {
            job_id: job.id.to_string(),
            scan_id: job.scan_id.to_string(),
            input_asset_id: job.input_asset_id.to_string(),
            input_kind: from_persisted_input_kind(job.input_kind),
            media_type: job.media_type,
            relative_path: job.relative_path,
            byte_length: job.byte_length,
            width_px: job.width_px,
            height_px: job.height_px,
            status: match job.status {
                PersistedOcrJobStatus::Queued => CoreOcrJobStatus::Queued,
                PersistedOcrJobStatus::Running => CoreOcrJobStatus::Running,
                PersistedOcrJobStatus::Recognized => CoreOcrJobStatus::Recognized,
                PersistedOcrJobStatus::Unavailable => CoreOcrJobStatus::Unavailable,
                PersistedOcrJobStatus::Cancelled => CoreOcrJobStatus::Cancelled,
            },
            created_at_ms: job.created_at_ms,
            updated_at_ms: job.updated_at_ms,
            last_started_at_ms: job.last_started_at_ms,
            completed_at_ms: job.completed_at_ms,
            attempt_count: job.attempt_count,
            retryable: job.retryable,
            next_retry_at_ms: job.next_retry_at_ms,
            last_error_code: job.last_error_code,
            last_error_message: job.last_error_message,
            provider: job.provider,
            provider_version: job.provider_version,
            model_name: job.model_name,
            provider_availability: match job.provider_availability {
                PersistedOcrProviderAvailability::Unknown => CoreOcrProviderAvailability::Unknown,
                PersistedOcrProviderAvailability::Available => {
                    CoreOcrProviderAvailability::Available
                }
                PersistedOcrProviderAvailability::Unavailable => {
                    CoreOcrProviderAvailability::Unavailable
                }
                PersistedOcrProviderAvailability::Failed => CoreOcrProviderAvailability::Failed,
            },
            last_ocr_run_id: job.last_ocr_run_id.map(|id| id.to_string()),
            cancellation_requested: job.cancellation_requested,
        }
    }
}
