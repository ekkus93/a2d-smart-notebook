//! Durable Rust-owned batch scanner session state for Milestone 8.5.
//!
//! Scanner recovery remains the source of truth for capture and registration durability. This
//! module adds bounded batch-session orchestration, a locked Notebook destination, duplicate-page
//! reporting, canonical Needs Review integration, and explicit summary cleanup.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, NotebookId, PageId, QualityStatus,
    ReviewItemId, ReviewItemKind, ScanId, system_now_ms,
};
use serde_json::{Value, json};

use crate::{
    A2dCore, CreateReviewItemRequest, RegisterScanRequest, RegisteredScan, ScannerRecoveryPhase,
};

const BATCH_DIRECTORY: &str = "scanner-batches";
const RECORD_SUFFIX: &str = ".json";
const MAX_SESSIONS: usize = 32;
const MAX_ENTRIES: usize = 128;
const MAX_RECORD_BYTES: u64 = 256 * 1024;
const MAX_TOKEN_BYTES: usize = 64;
static NEXT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchScanEntryStatus {
    Queued,
    Saved,
    NeedsReview,
}

impl BatchScanEntryStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Saved => "saved",
            Self::NeedsReview => "needs_review",
        }
    }

    fn parse(raw: &str) -> Result<Self, A2dError> {
        match raw {
            "queued" => Ok(Self::Queued),
            "saved" => Ok(Self::Saved),
            "needs_review" => Ok(Self::NeedsReview),
            _ => Err(batch_error(
                "CORE_BATCH_SCAN_STATUS_INVALID",
                ErrorCategory::Integrity,
                format!("unknown batch scan entry status {raw:?}"),
                false,
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchScanEntry {
    pub recovery_token: String,
    pub page_id: PageId,
    pub captured_at_ms: i64,
    pub status: BatchScanEntryStatus,
    pub registered_scan_id: Option<ScanId>,
    pub duplicate_page: bool,
    pub review_item_id: Option<ReviewItemId>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchScanSession {
    pub session_id: String,
    pub notebook_id: NotebookId,
    pub started_at_ms: i64,
    pub completed_at_ms: Option<i64>,
    pub entries: Vec<BatchScanEntry>,
}

impl BatchScanSession {
    pub fn queued_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|entry| entry.status == BatchScanEntryStatus::Queued)
            .count() as u32
    }

    pub fn saved_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|entry| entry.status == BatchScanEntryStatus::Saved)
            .count() as u32
    }

    pub fn review_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|entry| {
                entry.status == BatchScanEntryStatus::NeedsReview || entry.review_item_id.is_some()
            })
            .count() as u32
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginBatchScanSessionRequest {
    pub session_id: String,
    pub notebook_id: NotebookId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchScanReviewReason {
    IdentityFailure,
    ProcessingFailure,
    RegistrationFailure,
}

fn batch_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.core.batch_scanner",
        message.into(),
        retryable,
    )
}

fn validate_token(token: &str) -> Result<(), A2dError> {
    if token.is_empty()
        || token.len() > MAX_TOKEN_BYTES
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(batch_error(
            "CORE_BATCH_SCAN_TOKEN_INVALID",
            ErrorCategory::Validation,
            "batch session id must contain 1..=64 ASCII letters, digits, '-' or '_'",
            false,
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), A2dError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_DIRECTORY_SYNC_FAILED",
                ErrorCategory::Storage,
                format!("failed to synchronize batch scanner directory: {error}"),
                true,
            )
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), A2dError> {
    Err(batch_error(
        "CORE_BATCH_SCAN_DIRECTORY_SYNC_UNSUPPORTED",
        ErrorCategory::PlatformAdapter,
        "durable batch scanner sessions require directory synchronization",
        false,
    ))
}

fn required_string(value: &Value, field: &'static str) -> Result<String, A2dError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            batch_error(
                "CORE_BATCH_SCAN_RECORD_INVALID",
                ErrorCategory::Integrity,
                format!("batch scanner record has no valid {field}"),
                false,
            )
        })
}

fn required_i64(value: &Value, field: &'static str) -> Result<i64, A2dError> {
    value.get(field).and_then(Value::as_i64).ok_or_else(|| {
        batch_error(
            "CORE_BATCH_SCAN_RECORD_INVALID",
            ErrorCategory::Integrity,
            format!("batch scanner record has no valid {field}"),
            false,
        )
    })
}

impl A2dCore {
    fn batch_scan_root(&self) -> Result<PathBuf, A2dError> {
        let root = self.library_path.join("tmp").join(BATCH_DIRECTORY);
        std::fs::create_dir_all(&root).map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_DIRECTORY_FAILED",
                ErrorCategory::Storage,
                format!("failed to create batch scanner directory: {error}"),
                true,
            )
        })?;
        root.canonicalize().map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_DIRECTORY_INVALID",
                ErrorCategory::Storage,
                format!("failed to canonicalize batch scanner directory: {error}"),
                true,
            )
        })
    }

    fn batch_scan_path(&self, session_id: &str) -> Result<PathBuf, A2dError> {
        validate_token(session_id)?;
        Ok(self
            .batch_scan_root()?
            .join(format!("{session_id}{RECORD_SUFFIX}")))
    }

    fn encode_batch_scan_session(session: &BatchScanSession) -> Result<Vec<u8>, A2dError> {
        let entries = session
            .entries
            .iter()
            .map(|entry| {
                json!({
                    "recovery_token": entry.recovery_token,
                    "page_id": entry.page_id.to_string(),
                    "captured_at_ms": entry.captured_at_ms,
                    "status": entry.status.as_str(),
                    "registered_scan_id": entry.registered_scan_id.as_ref().map(ToString::to_string),
                    "duplicate_page": entry.duplicate_page,
                    "review_item_id": entry.review_item_id.as_ref().map(ToString::to_string),
                    "message": entry.message,
                })
            })
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&json!({
            "schema_version": 1,
            "session_id": session.session_id,
            "notebook_id": session.notebook_id.to_string(),
            "started_at_ms": session.started_at_ms,
            "completed_at_ms": session.completed_at_ms,
            "entries": entries,
        }))
        .map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_ENCODING_FAILED",
                ErrorCategory::Internal,
                format!("failed to encode batch scanner session: {error}"),
                false,
            )
        })?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(batch_error(
                "CORE_BATCH_SCAN_RECORD_TOO_LARGE",
                ErrorCategory::Storage,
                "batch scanner session exceeded its bounded representation",
                false,
            ));
        }
        Ok(bytes)
    }

    fn parse_batch_scan_session(&self, bytes: &[u8]) -> Result<BatchScanSession, A2dError> {
        let value: Value = serde_json::from_slice(bytes).map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_RECORD_CORRUPT",
                ErrorCategory::Integrity,
                format!("failed to parse batch scanner session: {error}"),
                false,
            )
        })?;
        if value.get("schema_version").and_then(Value::as_u64) != Some(1) {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SCHEMA_UNSUPPORTED",
                ErrorCategory::Integrity,
                "batch scanner session schema is unsupported",
                false,
            ));
        }
        let session_id = required_string(&value, "session_id")?;
        validate_token(&session_id)?;
        let notebook_id = NotebookId::parse(&required_string(&value, "notebook_id")?)?;
        let started_at_ms = required_i64(&value, "started_at_ms")?;
        if started_at_ms <= 0 {
            return Err(batch_error(
                "CORE_BATCH_SCAN_RECORD_INVALID",
                ErrorCategory::Integrity,
                "batch scanner started_at_ms must be positive",
                false,
            ));
        }
        let completed_at_ms = value.get("completed_at_ms").and_then(Value::as_i64);
        if completed_at_ms.is_some_and(|completed| completed < started_at_ms) {
            return Err(batch_error(
                "CORE_BATCH_SCAN_RECORD_INVALID",
                ErrorCategory::Integrity,
                "batch scanner completion time precedes its start time",
                false,
            ));
        }
        let raw_entries = value
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                batch_error(
                    "CORE_BATCH_SCAN_RECORD_INVALID",
                    ErrorCategory::Integrity,
                    "batch scanner record has no entries array",
                    false,
                )
            })?;
        if raw_entries.len() > MAX_ENTRIES {
            return Err(batch_error(
                "CORE_BATCH_SCAN_ENTRY_LIMIT_EXCEEDED",
                ErrorCategory::Integrity,
                "batch scanner session contains too many entries",
                false,
            ));
        }
        let mut tokens = BTreeSet::new();
        let mut entries = Vec::with_capacity(raw_entries.len());
        for raw in raw_entries {
            let recovery_token = required_string(raw, "recovery_token")?;
            validate_token(&recovery_token)?;
            if !tokens.insert(recovery_token.clone()) {
                return Err(batch_error(
                    "CORE_BATCH_SCAN_DUPLICATE_RECOVERY_TOKEN",
                    ErrorCategory::Integrity,
                    "batch scanner record repeats a recovery token",
                    false,
                ));
            }
            let page_id = PageId::parse(&required_string(raw, "page_id")?)?;
            let captured_at_ms = required_i64(raw, "captured_at_ms")?;
            let status = BatchScanEntryStatus::parse(&required_string(raw, "status")?)?;
            let registered_scan_id = raw
                .get("registered_scan_id")
                .and_then(Value::as_str)
                .map(ScanId::parse)
                .transpose()?;
            if (status == BatchScanEntryStatus::Saved) != registered_scan_id.is_some() {
                return Err(batch_error(
                    "CORE_BATCH_SCAN_RECORD_INVALID",
                    ErrorCategory::Integrity,
                    "only a saved batch entry may contain a registered scan id",
                    false,
                ));
            }
            let review_item_id = raw
                .get("review_item_id")
                .and_then(Value::as_str)
                .map(ReviewItemId::parse)
                .transpose()?;
            entries.push(BatchScanEntry {
                recovery_token,
                page_id,
                captured_at_ms,
                status,
                registered_scan_id,
                duplicate_page: raw
                    .get("duplicate_page")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                review_item_id,
                message: raw
                    .get("message")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            });
        }
        Ok(BatchScanSession {
            session_id,
            notebook_id,
            started_at_ms,
            completed_at_ms,
            entries,
        })
    }

    fn read_batch_scan_session(&self, session_id: &str) -> Result<BatchScanSession, A2dError> {
        let path = self.batch_scan_path(session_id)?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_SESSION_NOT_FOUND",
                ErrorCategory::Storage,
                format!("batch scanner session is unavailable: {error}"),
                false,
            )
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err(batch_error(
                "CORE_BATCH_SCAN_RECORD_INVALID",
                ErrorCategory::Integrity,
                "batch scanner session must be a bounded regular non-symlink file",
                false,
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        File::open(&path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|error| {
                batch_error(
                    "CORE_BATCH_SCAN_READ_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to read batch scanner session: {error}"),
                    true,
                )
            })?;
        if bytes.len() as u64 != metadata.len() {
            return Err(batch_error(
                "CORE_BATCH_SCAN_CHANGED_DURING_READ",
                ErrorCategory::Integrity,
                "batch scanner session changed while it was read",
                false,
            ));
        }
        let session = self.parse_batch_scan_session(&bytes)?;
        if session.session_id != session_id {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_ID_MISMATCH",
                ErrorCategory::Integrity,
                "batch scanner filename does not match record content",
                false,
            ));
        }
        Ok(session)
    }

    fn persist_batch_scan_session(
        &self,
        session: &BatchScanSession,
        create_new: bool,
    ) -> Result<(), A2dError> {
        let root = self.batch_scan_root()?;
        let final_path = self.batch_scan_path(&session.session_id)?;
        if !create_new && !final_path.is_file() {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_NOT_FOUND",
                ErrorCategory::Storage,
                "batch scanner session disappeared before update",
                false,
            ));
        }
        if create_new && final_path.exists() {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_ALREADY_EXISTS",
                ErrorCategory::Validation,
                "batch scanner session id already exists",
                false,
            ));
        }
        let temp_path = root.join(format!(
            ".{}.{}-{}.tmp",
            session.session_id,
            std::process::id(),
            NEXT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        let bytes = Self::encode_batch_scan_session(session)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| {
                batch_error(
                    "CORE_BATCH_SCAN_TEMP_CREATE_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to create batch scanner temp file: {error}"),
                    true,
                )
            })?;
        if let Err(error) = file
            .write_all(&bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
        {
            let _ = std::fs::remove_file(&temp_path);
            return Err(batch_error(
                "CORE_BATCH_SCAN_WRITE_FAILED",
                ErrorCategory::Storage,
                format!("failed to persist batch scanner session: {error}"),
                true,
            ));
        }
        drop(file);
        if create_new {
            let reservation = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&final_path);
            if let Err(error) = reservation {
                let _ = std::fs::remove_file(&temp_path);
                return Err(batch_error(
                    "CORE_BATCH_SCAN_SESSION_ALREADY_EXISTS",
                    ErrorCategory::Validation,
                    format!("failed to reserve batch scanner session path: {error}"),
                    false,
                ));
            }
        }
        std::fs::rename(&temp_path, &final_path).map_err(|error| {
            let _ = std::fs::remove_file(&temp_path);
            batch_error(
                "CORE_BATCH_SCAN_FINALIZE_FAILED",
                ErrorCategory::Storage,
                format!("failed to atomically finalize batch scanner session: {error}"),
                true,
            )
        })?;
        sync_directory(&root)
    }

    pub fn begin_batch_scan_session(
        &self,
        request: BeginBatchScanSessionRequest,
    ) -> Result<BatchScanSession, A2dError> {
        validate_token(&request.session_id)?;
        let notebook = self.get_notebook(&request.notebook_id)?.ok_or_else(|| {
            batch_error(
                "CORE_BATCH_SCAN_NOTEBOOK_NOT_FOUND",
                ErrorCategory::Validation,
                "batch scan Notebook does not exist",
                false,
            )
        })?;
        if notebook.archived {
            return Err(batch_error(
                "CORE_BATCH_SCAN_NOTEBOOK_ARCHIVED",
                ErrorCategory::Validation,
                "an archived Notebook cannot receive a batch scan",
                false,
            ));
        }
        let active = self.get_active_notebook()?;
        if active.as_ref().map(|item| &item.id) != Some(&request.notebook_id) {
            return Err(batch_error(
                "CORE_BATCH_SCAN_NOTEBOOK_NOT_ACTIVE",
                ErrorCategory::Validation,
                "batch scanning must lock the currently active Notebook",
                false,
            ));
        }
        if self
            .list_batch_scan_sessions(false)?
            .iter()
            .any(|session| session.completed_at_ms.is_none())
        {
            return Err(batch_error(
                "CORE_BATCH_SCAN_ACTIVE_SESSION_EXISTS",
                ErrorCategory::Validation,
                "finish or resume the active batch session before starting another",
                false,
            ));
        }
        if self.list_batch_scan_sessions(true)?.len() >= MAX_SESSIONS {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_LIMIT_EXCEEDED",
                ErrorCategory::Storage,
                "acknowledge completed batch summaries before starting another session",
                false,
            ));
        }
        let session = BatchScanSession {
            session_id: request.session_id,
            notebook_id: request.notebook_id,
            started_at_ms: system_now_ms()?,
            completed_at_ms: None,
            entries: Vec::new(),
        };
        self.persist_batch_scan_session(&session, true)?;
        Ok(session)
    }

    pub fn list_batch_scan_sessions(
        &self,
        include_completed: bool,
    ) -> Result<Vec<BatchScanSession>, A2dError> {
        let root = self.batch_scan_root()?;
        let mut paths = std::fs::read_dir(&root)
            .map_err(|error| {
                batch_error(
                    "CORE_BATCH_SCAN_LIST_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to list batch scanner sessions: {error}"),
                    true,
                )
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();
        if paths.len() > MAX_SESSIONS {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_LIMIT_EXCEEDED",
                ErrorCategory::Integrity,
                "batch scanner directory contains too many sessions",
                false,
            ));
        }
        let mut sessions = Vec::with_capacity(paths.len());
        for path in paths {
            let id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    batch_error(
                        "CORE_BATCH_SCAN_FILENAME_INVALID",
                        ErrorCategory::Integrity,
                        "batch scanner filename is not valid UTF-8",
                        false,
                    )
                })?;
            let session = self.read_batch_scan_session(id)?;
            if include_completed || session.completed_at_ms.is_none() {
                sessions.push(session);
            }
        }
        sessions.sort_by(|left, right| {
            left.started_at_ms
                .cmp(&right.started_at_ms)
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
        Ok(sessions)
    }

    pub fn queue_batch_scan_capture(
        &self,
        session_id: &str,
        recovery_token: &str,
    ) -> Result<BatchScanSession, A2dError> {
        let mut session = self.read_batch_scan_session(session_id)?;
        if session.completed_at_ms.is_some() {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_COMPLETED",
                ErrorCategory::Validation,
                "a completed batch session cannot accept captures",
                false,
            ));
        }
        if let Some(existing) = session
            .entries
            .iter()
            .find(|entry| entry.recovery_token == recovery_token)
        {
            let recovery = self
                .list_scanner_recoveries()?
                .into_iter()
                .find(|record| record.token == recovery_token);
            if recovery.as_ref().is_some_and(|record| {
                record.page_id != existing.page_id || record.notebook_id != session.notebook_id
            }) {
                return Err(batch_error(
                    "CORE_BATCH_SCAN_RECOVERY_IDENTITY_CONFLICT",
                    ErrorCategory::Integrity,
                    "an existing batch recovery token changed identity",
                    false,
                ));
            }
            return Ok(session);
        }
        if session.entries.len() >= MAX_ENTRIES {
            return Err(batch_error(
                "CORE_BATCH_SCAN_ENTRY_LIMIT_EXCEEDED",
                ErrorCategory::Storage,
                "batch session reached its capture limit",
                false,
            ));
        }
        let recovery = self
            .list_scanner_recoveries()?
            .into_iter()
            .find(|record| record.token == recovery_token)
            .ok_or_else(|| {
                batch_error(
                    "CORE_BATCH_SCAN_RECOVERY_NOT_FOUND",
                    ErrorCategory::Integrity,
                    "batch capture has no durable scanner recovery record",
                    false,
                )
            })?;
        if recovery.notebook_id != session.notebook_id {
            return Err(batch_error(
                "CORE_BATCH_SCAN_NOTEBOOK_CHANGED",
                ErrorCategory::Identity,
                "batch capture does not belong to the session's locked Notebook",
                false,
            ));
        }
        if !matches!(
            recovery.phase,
            ScannerRecoveryPhase::Captured | ScannerRecoveryPhase::PreviewReady
        ) {
            return Err(batch_error(
                "CORE_BATCH_SCAN_RECOVERY_PHASE_INVALID",
                ErrorCategory::Integrity,
                "new batch capture must be captured or preview-ready",
                false,
            ));
        }
        let duplicate_page = session
            .entries
            .iter()
            .any(|entry| entry.page_id == recovery.page_id);
        session.entries.push(BatchScanEntry {
            recovery_token: recovery.token,
            page_id: recovery.page_id,
            captured_at_ms: recovery.captured_at_ms,
            status: BatchScanEntryStatus::Queued,
            registered_scan_id: None,
            duplicate_page,
            review_item_id: None,
            message: if duplicate_page {
                Some("This page identity already appears in the current batch.".to_string())
            } else {
                None
            },
        });
        self.persist_batch_scan_session(&session, false)?;
        Ok(session)
    }

    pub fn register_batch_scan(
        &self,
        session_id: &str,
        mut request: RegisterScanRequest,
    ) -> Result<RegisteredScan, A2dError> {
        let session = self.reconcile_batch_scan_session(session_id)?;
        if session.completed_at_ms.is_some() {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_COMPLETED",
                ErrorCategory::Validation,
                "a completed batch session cannot register another scan",
                false,
            ));
        }
        let recovery_token = request.recovery_token.clone().ok_or_else(|| {
            batch_error(
                "CORE_BATCH_SCAN_RECOVERY_TOKEN_REQUIRED",
                ErrorCategory::Validation,
                "batch registration requires a scanner recovery token",
                false,
            )
        })?;
        let entry = session
            .entries
            .iter()
            .find(|entry| entry.recovery_token == recovery_token)
            .ok_or_else(|| {
                batch_error(
                    "CORE_BATCH_SCAN_CAPTURE_NOT_QUEUED",
                    ErrorCategory::Integrity,
                    "batch registration is not authorized by this session",
                    false,
                )
            })?;
        if entry.status != BatchScanEntryStatus::Queued {
            return Err(batch_error(
                "CORE_BATCH_SCAN_CAPTURE_NOT_QUEUED",
                ErrorCategory::Validation,
                "only a queued batch capture may be registered",
                false,
            ));
        }
        if entry.page_id != request.expected_page_id
            || request.active_notebook_id.as_ref() != Some(&session.notebook_id)
        {
            return Err(batch_error(
                "CORE_BATCH_SCAN_REGISTRATION_IDENTITY_CONFLICT",
                ErrorCategory::Identity,
                "batch registration changed the locked page or Notebook identity",
                false,
            ));
        }
        // A durable queued batch entry is the Rust-owned alternative to single-page review approval.
        request.user_approved = true;
        let registered = self.register_scan(request)?;
        self.record_batch_scan_saved(session_id, &recovery_token, &registered)?;
        Ok(registered)
    }

    pub fn report_batch_scan_review(
        &self,
        session_id: &str,
        recovery_token: &str,
        reason: BatchScanReviewReason,
        message: String,
    ) -> Result<BatchScanSession, A2dError> {
        if message.trim().is_empty() || message.len() > 512 {
            return Err(batch_error(
                "CORE_BATCH_SCAN_REVIEW_MESSAGE_INVALID",
                ErrorCategory::Validation,
                "batch review message must contain 1..=512 characters",
                false,
            ));
        }
        let mut session = self.read_batch_scan_session(session_id)?;
        let session_id_owned = session.session_id.clone();
        let entry = session
            .entries
            .iter_mut()
            .find(|entry| entry.recovery_token == recovery_token)
            .ok_or_else(|| {
                batch_error(
                    "CORE_BATCH_SCAN_CAPTURE_NOT_QUEUED",
                    ErrorCategory::Validation,
                    "batch review target is not part of this session",
                    false,
                )
            })?;
        if entry.status == BatchScanEntryStatus::Saved {
            return Err(batch_error(
                "CORE_BATCH_SCAN_ALREADY_SAVED",
                ErrorCategory::Validation,
                "a saved batch scan cannot be converted back into a capture failure",
                false,
            ));
        }
        if entry.review_item_id.is_none() {
            let kind = match reason {
                BatchScanReviewReason::IdentityFailure => ReviewItemKind::UnidentifiedPage,
                BatchScanReviewReason::ProcessingFailure
                | BatchScanReviewReason::RegistrationFailure => ReviewItemKind::ProcessingFailure,
            };
            let mut details = BTreeMap::new();
            details.insert("producer".to_string(), "batch_scanner".to_string());
            details.insert("session_id".to_string(), session_id_owned);
            details.insert("recovery_token".to_string(), recovery_token.to_string());
            details.insert("reason".to_string(), format!("{reason:?}"));
            details.insert("message".to_string(), message.clone());
            let review = self.create_review_item(CreateReviewItemRequest {
                kind,
                page_id: Some(entry.page_id.clone()),
                scan_id: None,
                severity: ErrorSeverity::Warning,
                details,
                created_at_ms: system_now_ms()?,
            })?;
            entry.review_item_id = Some(review.id().clone());
        }
        entry.status = BatchScanEntryStatus::NeedsReview;
        entry.message = Some(message);
        self.persist_batch_scan_session(&session, false)?;
        Ok(session)
    }

    fn record_batch_scan_saved(
        &self,
        session_id: &str,
        recovery_token: &str,
        registered: &RegisteredScan,
    ) -> Result<(), A2dError> {
        let mut session = self.read_batch_scan_session(session_id)?;
        let session_id_owned = session.session_id.clone();
        let entry = session
            .entries
            .iter_mut()
            .find(|entry| entry.recovery_token == recovery_token)
            .ok_or_else(|| {
                batch_error(
                    "CORE_BATCH_SCAN_CAPTURE_NOT_QUEUED",
                    ErrorCategory::Integrity,
                    "registered scan has no matching batch entry",
                    false,
                )
            })?;
        if entry.page_id != registered.page_id {
            return Err(batch_error(
                "CORE_BATCH_SCAN_REGISTERED_PAGE_MISMATCH",
                ErrorCategory::Integrity,
                "registered scan page does not match its batch entry",
                false,
            ));
        }
        entry.status = BatchScanEntryStatus::Saved;
        entry.registered_scan_id = Some(registered.scan_id.clone());

        let needs_review = entry.duplicate_page
            || registered.quality_status == QualityStatus::NeedsReview
            || !registered.required_actions.is_empty();
        if needs_review && entry.review_item_id.is_none() {
            let kind = if entry.duplicate_page {
                ReviewItemKind::Duplicate
            } else if !registered.required_actions.is_empty() {
                ReviewItemKind::Revision
            } else {
                ReviewItemKind::LowQuality
            };
            let mut details = BTreeMap::new();
            details.insert("producer".to_string(), "batch_scanner".to_string());
            details.insert("session_id".to_string(), session_id_owned);
            details.insert("recovery_token".to_string(), recovery_token.to_string());
            details.insert(
                "duplicate_page".to_string(),
                entry.duplicate_page.to_string(),
            );
            details.insert(
                "quality_status".to_string(),
                format!("{:?}", registered.quality_status),
            );
            details.insert(
                "required_actions".to_string(),
                format!("{:?}", registered.required_actions),
            );
            let review = self.create_review_item(CreateReviewItemRequest {
                kind,
                page_id: Some(entry.page_id.clone()),
                scan_id: Some(registered.scan_id.clone()),
                severity: ErrorSeverity::Warning,
                details,
                created_at_ms: system_now_ms()?,
            })?;
            entry.review_item_id = Some(review.id().clone());
            entry.message = Some(
                "Saved; review is required before treating this scan as resolved.".to_string(),
            );
        } else if entry.message.is_none() {
            entry.message =
                Some("Saved. OCR remains queued until the OCR milestone is available.".to_string());
        }
        self.persist_batch_scan_session(&session, false)
    }

    pub fn reconcile_batch_scan_session(
        &self,
        session_id: &str,
    ) -> Result<BatchScanSession, A2dError> {
        let mut session = self.read_batch_scan_session(session_id)?;
        let mut recoveries = self.list_scanner_recoveries()?;
        for recovery in &mut recoveries {
            if recovery.phase == ScannerRecoveryPhase::Registering {
                *recovery = self.reconcile_scanner_recovery(&recovery.token)?;
            }
        }
        let by_token = recoveries
            .into_iter()
            .map(|record| (record.token.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let mut changed = false;
        for entry in &mut session.entries {
            if entry.status != BatchScanEntryStatus::Queued {
                continue;
            }
            match by_token.get(&entry.recovery_token) {
                Some(recovery) if recovery.phase == ScannerRecoveryPhase::Committed => {
                    let scan_id = recovery.registered_scan_id.clone().ok_or_else(|| {
                        batch_error(
                            "CORE_BATCH_SCAN_COMMITTED_RECOVERY_INVALID",
                            ErrorCategory::Integrity,
                            "committed scanner recovery has no scan id",
                            false,
                        )
                    })?;
                    entry.status = BatchScanEntryStatus::Saved;
                    entry.registered_scan_id = Some(scan_id);
                    entry.message = Some(
                        "Saved scan recovered after interruption; review metadata may still need reconciliation."
                            .to_string(),
                    );
                    changed = true;
                }
                Some(_) => {}
                None => {
                    entry.status = BatchScanEntryStatus::NeedsReview;
                    entry.message = Some(
                        "The batch capture lost its scanner recovery record; no registration was retried."
                            .to_string(),
                    );
                    changed = true;
                }
            }
        }
        if changed {
            self.persist_batch_scan_session(&session, false)?;
        }
        Ok(session)
    }

    pub fn complete_batch_scan_session(
        &self,
        session_id: &str,
    ) -> Result<BatchScanSession, A2dError> {
        let mut session = self.reconcile_batch_scan_session(session_id)?;
        if session
            .entries
            .iter()
            .any(|entry| entry.status == BatchScanEntryStatus::Queued)
        {
            return Err(batch_error(
                "CORE_BATCH_SCAN_PENDING_CAPTURES",
                ErrorCategory::Validation,
                "wait for queued batch captures to finish before completing the session",
                true,
            ));
        }
        if session.completed_at_ms.is_none() {
            session.completed_at_ms = Some(system_now_ms()?);
            self.persist_batch_scan_session(&session, false)?;
        }
        // Persist completion before acknowledgements. Interruption can only leave redundant
        // committed recovery metadata; it cannot erase the batch summary.
        for entry in &session.entries {
            let Some(scan_id) = entry.registered_scan_id.as_ref() else {
                continue;
            };
            let recovery = self
                .list_scanner_recoveries()?
                .into_iter()
                .find(|record| record.token == entry.recovery_token);
            if let Some(recovery) = recovery {
                let committed = if recovery.phase == ScannerRecoveryPhase::Registering {
                    self.reconcile_scanner_recovery(&entry.recovery_token)?
                } else {
                    recovery
                };
                if committed.phase != ScannerRecoveryPhase::Committed
                    || committed.registered_scan_id.as_ref() != Some(scan_id)
                {
                    return Err(batch_error(
                        "CORE_BATCH_SCAN_RECOVERY_NOT_COMMITTED",
                        ErrorCategory::Integrity,
                        "batch completion could not reconcile a saved scan",
                        false,
                    ));
                }
                self.acknowledge_committed_scanner_recovery(&entry.recovery_token, scan_id)?;
            }
        }
        Ok(session)
    }

    pub fn acknowledge_batch_scan_session(&self, session_id: &str) -> Result<(), A2dError> {
        let session = self.read_batch_scan_session(session_id)?;
        if session.completed_at_ms.is_none() {
            return Err(batch_error(
                "CORE_BATCH_SCAN_SESSION_ACTIVE",
                ErrorCategory::Validation,
                "an active batch session cannot be acknowledged",
                false,
            ));
        }
        let root = self.batch_scan_root()?;
        let path = self.batch_scan_path(session_id)?;
        std::fs::remove_file(&path).map_err(|error| {
            batch_error(
                "CORE_BATCH_SCAN_REMOVE_FAILED",
                ErrorCategory::Storage,
                format!("failed to remove acknowledged batch summary: {error}"),
                true,
            )
        })?;
        sync_directory(&root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginScannerRecoveryRequest, OpenLibraryRequest};
    use a2d_domain::{LayoutId, ScanId};

    fn open_core() -> (std::sync::Arc<A2dCore>, PathBuf) {
        let root = std::env::temp_dir().join(format!("a2d-batch-scan-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, root)
    }

    fn seed_session(core: &A2dCore, notebook_id: NotebookId) {
        let session = BatchScanSession {
            session_id: "batch-test".to_string(),
            notebook_id,
            started_at_ms: 1,
            completed_at_ms: None,
            entries: Vec::new(),
        };
        core.persist_batch_scan_session(&session, true).unwrap();
    }

    fn begin_recovery(
        core: &A2dCore,
        root: &Path,
        token: &str,
        notebook_id: NotebookId,
        page_id: PageId,
        captured_at_ms: i64,
    ) {
        let staging = root
            .join("tmp/scanner-staging")
            .join(format!("{token}.jpg"));
        std::fs::create_dir_all(staging.parent().unwrap()).unwrap();
        std::fs::write(&staging, b"capture").unwrap();
        core.begin_scanner_recovery(BeginScannerRecoveryRequest {
            token: token.to_string(),
            staging_path: staging.to_string_lossy().into_owned(),
            page_id,
            notebook_id,
            captured_at_ms,
            layout_id: LayoutId::parse("USLETTER-LINED").unwrap(),
            processing_policy_version: 1,
        })
        .unwrap();
    }

    #[test]
    fn duplicate_page_is_detected_without_replacing_either_capture() {
        let (core, root) = open_core();
        let notebook_id = NotebookId::generate();
        let page_id = PageId::generate();
        seed_session(&core, notebook_id.clone());
        begin_recovery(&core, &root, "one", notebook_id.clone(), page_id.clone(), 1);
        begin_recovery(&core, &root, "two", notebook_id, page_id, 2);
        let first = core.queue_batch_scan_capture("batch-test", "one").unwrap();
        assert!(!first.entries[0].duplicate_page);
        let second = core.queue_batch_scan_capture("batch-test", "two").unwrap();
        assert_eq!(second.entries.len(), 2);
        assert!(second.entries[1].duplicate_page);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn requeueing_same_recovery_token_is_idempotent() {
        let (core, root) = open_core();
        let notebook_id = NotebookId::generate();
        let page_id = PageId::generate();
        seed_session(&core, notebook_id.clone());
        begin_recovery(&core, &root, "same", notebook_id, page_id, 1);
        core.queue_batch_scan_capture("batch-test", "same").unwrap();
        let repeated = core.queue_batch_scan_capture("batch-test", "same").unwrap();
        assert_eq!(repeated.entries.len(), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn committed_recovery_reconciles_after_reopen_without_reregistering() {
        let (core, root) = open_core();
        let notebook_id = NotebookId::generate();
        let page_id = PageId::generate();
        seed_session(&core, notebook_id.clone());
        begin_recovery(&core, &root, "resume", notebook_id, page_id.clone(), 1);
        core.queue_batch_scan_capture("batch-test", "resume")
            .unwrap();
        let recovery = core
            .list_scanner_recoveries()
            .unwrap()
            .into_iter()
            .find(|record| record.token == "resume")
            .unwrap();
        core.mark_scanner_recovery_preview_ready("resume").unwrap();
        core.mark_scanner_recovery_registering(
            "resume",
            Path::new(&recovery.staging_path),
            &recovery.page_id,
            &recovery.notebook_id,
            &recovery.layout_id,
            recovery.processing_policy_version,
        )
        .unwrap();
        let scan_id = ScanId::generate();
        core.mark_scanner_recovery_committed("resume", &scan_id)
            .unwrap();
        drop(core);

        let reopened = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let session = reopened.reconcile_batch_scan_session("batch-test").unwrap();
        assert_eq!(session.entries[0].status, BatchScanEntryStatus::Saved);
        assert_eq!(session.entries[0].registered_scan_id, Some(scan_id));
        assert_eq!(session.entries[0].page_id, page_id);
        std::fs::remove_dir_all(root).ok();
    }
}
