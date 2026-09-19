//! Durable OCR queue state for Milestone 11 Q1.
//!
//! Android executes the platform OCR provider, but queue identity, work ordering, retries,
//! cancellation intent, and provider diagnostics are canonical Rust-owned state.

use a2d_domain::{A2dError, AssetId, OcrJobId, OcrRunId, ScanId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Storage, map_rusqlite_error};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedOcrInputKind {
    Original,
    Corrected,
    OcrOptimized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedOcrJobStatus {
    Queued,
    Running,
    Recognized,
    Unavailable,
    Cancelled,
}

impl PersistedOcrJobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Recognized | Self::Unavailable | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedOcrProviderAvailability {
    Unknown,
    Available,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedOcrJob {
    pub id: OcrJobId,
    pub scan_id: ScanId,
    pub input_asset_id: AssetId,
    pub input_kind: PersistedOcrInputKind,
    pub media_type: String,
    pub relative_path: String,
    pub byte_length: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub status: PersistedOcrJobStatus,
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
    pub provider_availability: PersistedOcrProviderAvailability,
    pub last_ocr_run_id: Option<OcrRunId>,
    pub cancellation_requested: bool,
}

pub trait OcrJobRepository {
    fn insert_ocr_job(&self, job: &PersistedOcrJob) -> Result<(), A2dError>;
    fn update_ocr_job(&self, job: &PersistedOcrJob) -> Result<(), A2dError>;
    fn get_ocr_job(&self, id: &OcrJobId) -> Result<Option<PersistedOcrJob>, A2dError>;
    fn find_active_ocr_job(
        &self,
        scan_id: &ScanId,
        input_asset_id: &AssetId,
        input_kind: PersistedOcrInputKind,
    ) -> Result<Option<PersistedOcrJob>, A2dError>;
    fn find_active_ocr_job_for_scan(
        &self,
        scan_id: &ScanId,
    ) -> Result<Option<PersistedOcrJob>, A2dError>;
    fn count_active_ocr_jobs(&self) -> Result<usize, A2dError>;
    fn next_due_ocr_job(&self, now_ms: i64) -> Result<Option<PersistedOcrJob>, A2dError>;
    fn list_running_ocr_jobs(&self) -> Result<Vec<PersistedOcrJob>, A2dError>;
}

impl OcrJobRepository for Storage {
    fn insert_ocr_job(&self, job: &PersistedOcrJob) -> Result<(), A2dError> {
        OcrJobRepository::insert_ocr_job(&self.conn, job)
    }

    fn update_ocr_job(&self, job: &PersistedOcrJob) -> Result<(), A2dError> {
        OcrJobRepository::update_ocr_job(&self.conn, job)
    }

    fn get_ocr_job(&self, id: &OcrJobId) -> Result<Option<PersistedOcrJob>, A2dError> {
        OcrJobRepository::get_ocr_job(&self.conn, id)
    }

    fn find_active_ocr_job(
        &self,
        scan_id: &ScanId,
        input_asset_id: &AssetId,
        input_kind: PersistedOcrInputKind,
    ) -> Result<Option<PersistedOcrJob>, A2dError> {
        OcrJobRepository::find_active_ocr_job(&self.conn, scan_id, input_asset_id, input_kind)
    }

    fn find_active_ocr_job_for_scan(
        &self,
        scan_id: &ScanId,
    ) -> Result<Option<PersistedOcrJob>, A2dError> {
        OcrJobRepository::find_active_ocr_job_for_scan(&self.conn, scan_id)
    }

    fn count_active_ocr_jobs(&self) -> Result<usize, A2dError> {
        OcrJobRepository::count_active_ocr_jobs(&self.conn)
    }

    fn next_due_ocr_job(&self, now_ms: i64) -> Result<Option<PersistedOcrJob>, A2dError> {
        OcrJobRepository::next_due_ocr_job(&self.conn, now_ms)
    }

    fn list_running_ocr_jobs(&self) -> Result<Vec<PersistedOcrJob>, A2dError> {
        OcrJobRepository::list_running_ocr_jobs(&self.conn)
    }
}

impl OcrJobRepository for Connection {
    fn insert_ocr_job(&self, job: &PersistedOcrJob) -> Result<(), A2dError> {
        self.execute(
            "INSERT INTO ocr_jobs (id, scan_id, input_asset_id, input_kind, media_type, \
             relative_path, byte_length, width_px, height_px, status, created_at_ms, updated_at_ms, \
             last_started_at_ms, completed_at_ms, attempt_count, retryable, next_retry_at_ms, \
             last_error_code, last_error_message, provider, provider_version, model_name, \
             provider_availability, last_ocr_run_id, cancellation_requested) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                     ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            params![
                job.id.to_string(),
                job.scan_id.to_string(),
                job.input_asset_id.to_string(),
                input_kind_to_str(job.input_kind),
                &job.media_type,
                &job.relative_path,
                job.byte_length as i64,
                i64::from(job.width_px),
                i64::from(job.height_px),
                status_to_str(job.status),
                job.created_at_ms,
                job.updated_at_ms,
                job.last_started_at_ms,
                job.completed_at_ms,
                i64::from(job.attempt_count),
                job.retryable,
                job.next_retry_at_ms,
                &job.last_error_code,
                &job.last_error_message,
                &job.provider,
                &job.provider_version,
                &job.model_name,
                provider_availability_to_str(job.provider_availability),
                job.last_ocr_run_id.as_ref().map(ToString::to_string),
                job.cancellation_requested,
            ],
        )
        .map_err(|error| map_rusqlite_error("inserting OCR job", error))?;
        Ok(())
    }

    fn update_ocr_job(&self, job: &PersistedOcrJob) -> Result<(), A2dError> {
        let changed = self
            .execute(
                "UPDATE ocr_jobs SET scan_id = ?2, input_asset_id = ?3, input_kind = ?4, \
                 media_type = ?5, relative_path = ?6, byte_length = ?7, width_px = ?8, \
                 height_px = ?9, status = ?10, created_at_ms = ?11, updated_at_ms = ?12, \
                 last_started_at_ms = ?13, completed_at_ms = ?14, attempt_count = ?15, \
                 retryable = ?16, next_retry_at_ms = ?17, last_error_code = ?18, \
                 last_error_message = ?19, provider = ?20, provider_version = ?21, model_name = ?22, \
                 provider_availability = ?23, last_ocr_run_id = ?24, cancellation_requested = ?25 \
                 WHERE id = ?1",
                params![
                    job.id.to_string(),
                    job.scan_id.to_string(),
                    job.input_asset_id.to_string(),
                    input_kind_to_str(job.input_kind),
                    &job.media_type,
                    &job.relative_path,
                    job.byte_length as i64,
                    i64::from(job.width_px),
                    i64::from(job.height_px),
                    status_to_str(job.status),
                    job.created_at_ms,
                    job.updated_at_ms,
                    job.last_started_at_ms,
                    job.completed_at_ms,
                    i64::from(job.attempt_count),
                    job.retryable,
                    job.next_retry_at_ms,
                    &job.last_error_code,
                    &job.last_error_message,
                    &job.provider,
                    &job.provider_version,
                    &job.model_name,
                    provider_availability_to_str(job.provider_availability),
                    job.last_ocr_run_id.as_ref().map(ToString::to_string),
                    job.cancellation_requested,
                ],
            )
            .map_err(|error| map_rusqlite_error("updating OCR job", error))?;
        if changed != 1 {
            return Err(storage_integrity_error(
                "STORAGE_OCR_JOB_UPDATE_MISSING",
                "OCR job disappeared before its durable transition could be written",
            ));
        }
        Ok(())
    }

    fn get_ocr_job(&self, id: &OcrJobId) -> Result<Option<PersistedOcrJob>, A2dError> {
        self.query_row(
            &format!("{} WHERE id = ?1", OCR_JOB_SELECT),
            [id.to_string()],
            decode_job_row,
        )
        .optional()
        .map_err(|error| map_rusqlite_error("reading OCR job", error))?
        .map(decode_job)
        .transpose()
    }

    fn find_active_ocr_job(
        &self,
        scan_id: &ScanId,
        input_asset_id: &AssetId,
        input_kind: PersistedOcrInputKind,
    ) -> Result<Option<PersistedOcrJob>, A2dError> {
        self.query_row(
            &format!(
                "{} WHERE scan_id = ?1 AND input_asset_id = ?2 AND input_kind = ?3 \
                 AND status IN ('Queued', 'Running') ORDER BY created_at_ms ASC, id ASC LIMIT 1",
                OCR_JOB_SELECT
            ),
            params![
                scan_id.to_string(),
                input_asset_id.to_string(),
                input_kind_to_str(input_kind),
            ],
            decode_job_row,
        )
        .optional()
        .map_err(|error| map_rusqlite_error("finding active OCR job", error))?
        .map(decode_job)
        .transpose()
    }

    fn find_active_ocr_job_for_scan(
        &self,
        scan_id: &ScanId,
    ) -> Result<Option<PersistedOcrJob>, A2dError> {
        self.query_row(
            &format!(
                "{} WHERE scan_id = ?1 AND status IN ('Queued', 'Running') \
                 ORDER BY created_at_ms ASC, id ASC LIMIT 1",
                OCR_JOB_SELECT
            ),
            params![scan_id.to_string()],
            decode_job_row,
        )
        .optional()
        .map_err(|error| map_rusqlite_error("finding active OCR job for scan", error))?
        .map(decode_job)
        .transpose()
    }

    fn count_active_ocr_jobs(&self) -> Result<usize, A2dError> {
        let count = self
            .query_row(
                "SELECT COUNT(*) FROM ocr_jobs WHERE status IN ('Queued', 'Running')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| map_rusqlite_error("counting active OCR jobs", error))?;
        usize::try_from(count).map_err(|_| {
            storage_integrity_error(
                "STORAGE_OCR_JOB_COUNT_INVALID",
                "OCR job count was outside the portable usize representation",
            )
        })
    }

    fn next_due_ocr_job(&self, now_ms: i64) -> Result<Option<PersistedOcrJob>, A2dError> {
        self.query_row(
            &format!(
                "{} WHERE status = 'Queued' AND cancellation_requested = 0 \
                 AND (next_retry_at_ms IS NULL OR next_retry_at_ms <= ?1) \
                 ORDER BY created_at_ms ASC, id ASC LIMIT 1",
                OCR_JOB_SELECT
            ),
            [now_ms],
            decode_job_row,
        )
        .optional()
        .map_err(|error| map_rusqlite_error("claiming next due OCR job", error))?
        .map(decode_job)
        .transpose()
    }

    fn list_running_ocr_jobs(&self) -> Result<Vec<PersistedOcrJob>, A2dError> {
        let mut statement = self
            .prepare(&format!(
                "{} WHERE status = 'Running' ORDER BY created_at_ms ASC, id ASC",
                OCR_JOB_SELECT
            ))
            .map_err(|error| map_rusqlite_error("preparing running OCR job query", error))?;
        let rows = statement
            .query_map([], decode_job_row)
            .map_err(|error| map_rusqlite_error("querying running OCR jobs", error))?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(decode_job(row.map_err(|error| {
                map_rusqlite_error("decoding running OCR job row", error)
            })?)?);
        }
        Ok(jobs)
    }
}

const OCR_JOB_SELECT: &str = "SELECT id, scan_id, input_asset_id, input_kind, media_type, relative_path, byte_length, \
     width_px, height_px, status, created_at_ms, updated_at_ms, last_started_at_ms, completed_at_ms, \
     attempt_count, retryable, next_retry_at_ms, last_error_code, last_error_message, provider, \
     provider_version, model_name, provider_availability, last_ocr_run_id, cancellation_requested \
     FROM ocr_jobs";

#[allow(clippy::type_complexity)]
type OcrJobRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    String,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
    i64,
    bool,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    bool,
);

fn decode_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OcrJobRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
        row.get(18)?,
        row.get(19)?,
        row.get(20)?,
        row.get(21)?,
        row.get(22)?,
        row.get(23)?,
        row.get(24)?,
    ))
}

fn decode_job(row: OcrJobRow) -> Result<PersistedOcrJob, A2dError> {
    let (
        id,
        scan_id,
        input_asset_id,
        input_kind,
        media_type,
        relative_path,
        byte_length,
        width_px,
        height_px,
        status,
        created_at_ms,
        updated_at_ms,
        last_started_at_ms,
        completed_at_ms,
        attempt_count,
        retryable,
        next_retry_at_ms,
        last_error_code,
        last_error_message,
        provider,
        provider_version,
        model_name,
        provider_availability,
        last_ocr_run_id,
        cancellation_requested,
    ) = row;
    Ok(PersistedOcrJob {
        id: OcrJobId::parse(&id)?,
        scan_id: ScanId::parse(&scan_id)?,
        input_asset_id: AssetId::parse(&input_asset_id)?,
        input_kind: input_kind_from_str(&input_kind)?,
        media_type,
        relative_path,
        byte_length: non_negative_u64(byte_length, "ocr_jobs.byte_length")?,
        width_px: positive_u32(width_px, "ocr_jobs.width_px")?,
        height_px: positive_u32(height_px, "ocr_jobs.height_px")?,
        status: status_from_str(&status)?,
        created_at_ms,
        updated_at_ms,
        last_started_at_ms,
        completed_at_ms,
        attempt_count: non_negative_u32(attempt_count, "ocr_jobs.attempt_count")?,
        retryable,
        next_retry_at_ms,
        last_error_code,
        last_error_message,
        provider,
        provider_version,
        model_name,
        provider_availability: provider_availability_from_str(&provider_availability)?,
        last_ocr_run_id: last_ocr_run_id
            .map(|value| OcrRunId::parse(&value))
            .transpose()?,
        cancellation_requested,
    })
}

fn input_kind_to_str(value: PersistedOcrInputKind) -> &'static str {
    match value {
        PersistedOcrInputKind::Original => "Original",
        PersistedOcrInputKind::Corrected => "Corrected",
        PersistedOcrInputKind::OcrOptimized => "OcrOptimized",
    }
}

fn input_kind_from_str(value: &str) -> Result<PersistedOcrInputKind, A2dError> {
    match value {
        "Original" => Ok(PersistedOcrInputKind::Original),
        "Corrected" => Ok(PersistedOcrInputKind::Corrected),
        "OcrOptimized" => Ok(PersistedOcrInputKind::OcrOptimized),
        other => Err(corrupt_enum("ocr_jobs.input_kind", other)),
    }
}

fn status_to_str(value: PersistedOcrJobStatus) -> &'static str {
    match value {
        PersistedOcrJobStatus::Queued => "Queued",
        PersistedOcrJobStatus::Running => "Running",
        PersistedOcrJobStatus::Recognized => "Recognized",
        PersistedOcrJobStatus::Unavailable => "Unavailable",
        PersistedOcrJobStatus::Cancelled => "Cancelled",
    }
}

fn status_from_str(value: &str) -> Result<PersistedOcrJobStatus, A2dError> {
    match value {
        "Queued" => Ok(PersistedOcrJobStatus::Queued),
        "Running" => Ok(PersistedOcrJobStatus::Running),
        "Recognized" => Ok(PersistedOcrJobStatus::Recognized),
        "Unavailable" => Ok(PersistedOcrJobStatus::Unavailable),
        "Cancelled" => Ok(PersistedOcrJobStatus::Cancelled),
        other => Err(corrupt_enum("ocr_jobs.status", other)),
    }
}

fn provider_availability_to_str(value: PersistedOcrProviderAvailability) -> &'static str {
    match value {
        PersistedOcrProviderAvailability::Unknown => "Unknown",
        PersistedOcrProviderAvailability::Available => "Available",
        PersistedOcrProviderAvailability::Unavailable => "Unavailable",
        PersistedOcrProviderAvailability::Failed => "Failed",
    }
}

fn provider_availability_from_str(
    value: &str,
) -> Result<PersistedOcrProviderAvailability, A2dError> {
    match value {
        "Unknown" => Ok(PersistedOcrProviderAvailability::Unknown),
        "Available" => Ok(PersistedOcrProviderAvailability::Available),
        "Unavailable" => Ok(PersistedOcrProviderAvailability::Unavailable),
        "Failed" => Ok(PersistedOcrProviderAvailability::Failed),
        other => Err(corrupt_enum("ocr_jobs.provider_availability", other)),
    }
}

fn non_negative_u64(value: i64, field: &'static str) -> Result<u64, A2dError> {
    u64::try_from(value).map_err(|_| {
        storage_integrity_error(
            "STORAGE_OCR_JOB_INTEGER_CORRUPT",
            format!("{field} contains a negative value"),
        )
    })
}

fn positive_u32(value: i64, field: &'static str) -> Result<u32, A2dError> {
    let value = u32::try_from(value).map_err(|_| {
        storage_integrity_error(
            "STORAGE_OCR_JOB_INTEGER_CORRUPT",
            format!("{field} is outside the portable u32 representation"),
        )
    })?;
    if value == 0 {
        return Err(storage_integrity_error(
            "STORAGE_OCR_JOB_INTEGER_CORRUPT",
            format!("{field} must be positive"),
        ));
    }
    Ok(value)
}

fn non_negative_u32(value: i64, field: &'static str) -> Result<u32, A2dError> {
    u32::try_from(value).map_err(|_| {
        storage_integrity_error(
            "STORAGE_OCR_JOB_INTEGER_CORRUPT",
            format!("{field} is outside the portable u32 representation"),
        )
    })
}

fn corrupt_enum(field: &'static str, value: &str) -> A2dError {
    storage_integrity_error(
        "STORAGE_OCR_JOB_ENUM_CORRUPT",
        format!("{field} contains unknown value {value:?}"),
    )
    .with_detail("field", field)
    .with_detail("value", value)
}

fn storage_integrity_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        a2d_domain::ErrorCode::new(code),
        a2d_domain::ErrorCategory::Integrity,
        a2d_domain::ErrorSeverity::Critical,
        "error.storage.ocr_job_integrity",
        message,
        false,
    )
}
