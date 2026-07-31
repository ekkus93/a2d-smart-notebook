//! Durable, report-first recovery records for the single-page scanner.
//!
//! The record is created only after a camera capture exists in Rust's scanner staging directory.
//! It is advanced before preview/registration boundaries and remains after a committed scan until
//! the platform explicitly acknowledges the result. Recovery never auto-registers, auto-deletes,
//! or guesses around malformed state.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookId, PageId, ScanId,
    system_now_ms,
};
use serde_json::{Value, json};

use super::A2dCore;

const RECOVERY_DIRECTORY: &str = "scanner-recovery";
const STAGING_DIRECTORY: &str = "scanner-staging";
const RECORD_SUFFIX: &str = ".json";
const MAX_RECORDS: usize = 128;
const MAX_RECORD_BYTES: u64 = 64 * 1024;
const MAX_TOKEN_BYTES: usize = 64;
static NEXT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScannerRecoveryPhase {
    Captured,
    PreviewReady,
    Registering,
    Committed,
}

impl ScannerRecoveryPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::PreviewReady => "preview_ready",
            Self::Registering => "registering",
            Self::Committed => "committed",
        }
    }

    fn parse(value: &str) -> Result<Self, A2dError> {
        match value {
            "captured" => Ok(Self::Captured),
            "preview_ready" => Ok(Self::PreviewReady),
            "registering" => Ok(Self::Registering),
            "committed" => Ok(Self::Committed),
            _ => Err(recovery_error(
                "CORE_SCANNER_RECOVERY_PHASE_INVALID",
                ErrorCategory::Integrity,
                format!("unknown scanner recovery phase {value:?}"),
                false,
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BeginScannerRecoveryRequest {
    pub token: String,
    pub staging_path: String,
    pub page_id: PageId,
    pub notebook_id: NotebookId,
    pub captured_at_ms: i64,
    pub layout_id: LayoutId,
    pub processing_policy_version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScannerRecoveryRecord {
    pub token: String,
    pub staging_path: String,
    pub page_id: PageId,
    pub notebook_id: NotebookId,
    pub captured_at_ms: i64,
    pub layout_id: LayoutId,
    pub processing_policy_version: u32,
    pub phase: ScannerRecoveryPhase,
    pub registered_scan_id: Option<ScanId>,
    pub updated_at_ms: i64,
}

fn recovery_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        ErrorSeverity::Error,
        "error.core.scanner_recovery",
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
        return Err(recovery_error(
            "CORE_SCANNER_RECOVERY_TOKEN_INVALID",
            ErrorCategory::Validation,
            "scanner recovery token must contain 1..=64 ASCII letters, digits, '-' or '_'",
            false,
        ));
    }
    Ok(())
}

fn required_string(value: &Value, field: &'static str) -> Result<String, A2dError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_INVALID",
                ErrorCategory::Integrity,
                format!("scanner recovery record has no valid {field}"),
                false,
            )
            .with_detail("field", field)
        })
}

fn required_u32(value: &Value, field: &'static str) -> Result<u32, A2dError> {
    let raw = value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        recovery_error(
            "CORE_SCANNER_RECOVERY_RECORD_INVALID",
            ErrorCategory::Integrity,
            format!("scanner recovery record has no valid {field}"),
            false,
        )
        .with_detail("field", field)
    })?;
    u32::try_from(raw).map_err(|_| {
        recovery_error(
            "CORE_SCANNER_RECOVERY_RECORD_INVALID",
            ErrorCategory::Integrity,
            format!("scanner recovery record {field} exceeds u32"),
            false,
        )
        .with_detail("field", field)
    })
}

fn required_i64(value: &Value, field: &'static str) -> Result<i64, A2dError> {
    value.get(field).and_then(Value::as_i64).ok_or_else(|| {
        recovery_error(
            "CORE_SCANNER_RECOVERY_RECORD_INVALID",
            ErrorCategory::Integrity,
            format!("scanner recovery record has no valid {field}"),
            false,
        )
        .with_detail("field", field)
    })
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), A2dError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_DIRECTORY_SYNC_FAILED",
                ErrorCategory::Storage,
                format!("failed to synchronize scanner recovery directory: {error}"),
                true,
            )
            .with_detail("directory", path.to_string_lossy())
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), A2dError> {
    Err(recovery_error(
        "CORE_SCANNER_RECOVERY_DIRECTORY_SYNC_UNSUPPORTED",
        ErrorCategory::PlatformAdapter,
        "durable scanner recovery requires directory synchronization on this platform",
        false,
    ))
}

impl A2dCore {
    fn scanner_recovery_root(&self) -> Result<PathBuf, A2dError> {
        let root = self.library_path.join("tmp").join(RECOVERY_DIRECTORY);
        std::fs::create_dir_all(&root).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_DIRECTORY_FAILED",
                ErrorCategory::Storage,
                format!("failed to create scanner recovery directory: {error}"),
                true,
            )
            .with_detail("directory", root.to_string_lossy())
        })?;
        root.canonicalize().map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_DIRECTORY_INVALID",
                ErrorCategory::Storage,
                format!("failed to canonicalize scanner recovery directory: {error}"),
                true,
            )
        })
    }

    fn scanner_staging_root(&self) -> Result<PathBuf, A2dError> {
        let root = self.library_path.join("tmp").join(STAGING_DIRECTORY);
        std::fs::create_dir_all(&root).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_STAGING_DIRECTORY_FAILED",
                ErrorCategory::Storage,
                format!("failed to create scanner staging directory: {error}"),
                true,
            )
        })?;
        root.canonicalize().map_err(|error| {
            recovery_error(
                "CORE_SCANNER_STAGING_DIRECTORY_INVALID",
                ErrorCategory::Storage,
                format!("failed to canonicalize scanner staging directory: {error}"),
                true,
            )
        })
    }

    fn validate_recovery_staging_file(&self, raw_path: &str) -> Result<PathBuf, A2dError> {
        if raw_path.trim().is_empty() {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_STAGING_PATH_EMPTY",
                ErrorCategory::Validation,
                "scanner recovery staging path must not be empty",
                false,
            ));
        }
        let supplied = PathBuf::from(raw_path);
        let metadata = std::fs::symlink_metadata(&supplied).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_STAGING_UNAVAILABLE",
                ErrorCategory::Storage,
                format!("scanner recovery staging file is unavailable: {error}"),
                true,
            )
            .with_detail("staging_path", raw_path)
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_STAGING_INVALID",
                ErrorCategory::Integrity,
                "scanner recovery staging path must be a non-empty regular non-symlink file",
                false,
            )
            .with_detail("staging_path", raw_path));
        }
        let canonical = supplied.canonicalize().map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_STAGING_INVALID",
                ErrorCategory::Storage,
                format!("failed to canonicalize scanner recovery staging file: {error}"),
                true,
            )
        })?;
        let root = self.scanner_staging_root()?;
        if canonical.parent() != Some(root.as_path()) {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_STAGING_ESCAPES_LIBRARY",
                ErrorCategory::Integrity,
                "scanner recovery staging file is outside the approved staging root",
                false,
            )
            .with_detail("staging_path", canonical.to_string_lossy())
            .with_detail("approved_root", root.to_string_lossy()));
        }
        Ok(canonical)
    }

    fn scanner_recovery_path(&self, token: &str) -> Result<PathBuf, A2dError> {
        validate_token(token)?;
        Ok(self
            .scanner_recovery_root()?
            .join(format!("{token}{RECORD_SUFFIX}")))
    }

    fn encode_scanner_recovery(record: &ScannerRecoveryRecord) -> Result<Vec<u8>, A2dError> {
        let value = json!({
            "schema_version": 1,
            "token": record.token,
            "staging_path": record.staging_path,
            "page_id": record.page_id.to_string(),
            "notebook_id": record.notebook_id.to_string(),
            "captured_at_ms": record.captured_at_ms,
            "layout_id": record.layout_id.to_string(),
            "processing_policy_version": record.processing_policy_version,
            "phase": record.phase.as_str(),
            "registered_scan_id": record.registered_scan_id.as_ref().map(ToString::to_string),
            "updated_at_ms": record.updated_at_ms,
        });
        let bytes = serde_json::to_vec(&value).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_ENCODING_FAILED",
                ErrorCategory::Internal,
                format!("failed to encode scanner recovery record: {error}"),
                false,
            )
        })?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_TOO_LARGE",
                ErrorCategory::Internal,
                "scanner recovery record exceeded its bounded representation",
                false,
            ));
        }
        Ok(bytes)
    }

    fn parse_scanner_recovery(&self, bytes: &[u8]) -> Result<ScannerRecoveryRecord, A2dError> {
        let value: Value = serde_json::from_slice(bytes).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_CORRUPT",
                ErrorCategory::Integrity,
                format!("failed to parse scanner recovery record: {error}"),
                false,
            )
        })?;
        if value.get("schema_version").and_then(Value::as_u64) != Some(1) {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_SCHEMA_UNSUPPORTED",
                ErrorCategory::Integrity,
                "scanner recovery record schema is unsupported",
                false,
            ));
        }
        let token = required_string(&value, "token")?;
        validate_token(&token)?;
        let staging_path = required_string(&value, "staging_path")?;
        let page_id = PageId::parse(&required_string(&value, "page_id")?)?;
        let notebook_id = NotebookId::parse(&required_string(&value, "notebook_id")?)?;
        let captured_at_ms = required_i64(&value, "captured_at_ms")?;
        if captured_at_ms <= 0 {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_INVALID",
                ErrorCategory::Integrity,
                "scanner recovery captured_at_ms must be positive",
                false,
            ));
        }
        let layout_id = LayoutId::parse(&required_string(&value, "layout_id")?)?;
        let processing_policy_version = required_u32(&value, "processing_policy_version")?;
        if processing_policy_version == 0 {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_INVALID",
                ErrorCategory::Integrity,
                "scanner recovery processing policy version must be non-zero",
                false,
            ));
        }
        let phase = ScannerRecoveryPhase::parse(&required_string(&value, "phase")?)?;
        let registered_scan_id = value
            .get("registered_scan_id")
            .and_then(Value::as_str)
            .map(ScanId::parse)
            .transpose()?;
        if (phase == ScannerRecoveryPhase::Committed) != registered_scan_id.is_some() {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_INVALID",
                ErrorCategory::Integrity,
                "only a committed scanner recovery may contain a registered scan id",
                false,
            ));
        }
        let updated_at_ms = required_i64(&value, "updated_at_ms")?;
        if updated_at_ms <= 0 {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_INVALID",
                ErrorCategory::Integrity,
                "scanner recovery updated_at_ms must be positive",
                false,
            ));
        }
        if phase != ScannerRecoveryPhase::Committed {
            let canonical = self.validate_recovery_staging_file(&staging_path)?;
            if canonical.to_string_lossy() != staging_path {
                return Err(recovery_error(
                    "CORE_SCANNER_RECOVERY_STAGING_IDENTITY_CHANGED",
                    ErrorCategory::Integrity,
                    "scanner recovery staging path is not canonical",
                    false,
                ));
            }
        }
        Ok(ScannerRecoveryRecord {
            token,
            staging_path,
            page_id,
            notebook_id,
            captured_at_ms,
            layout_id,
            processing_policy_version,
            phase,
            registered_scan_id,
            updated_at_ms,
        })
    }

    fn read_scanner_recovery(&self, token: &str) -> Result<ScannerRecoveryRecord, A2dError> {
        let path = self.scanner_recovery_path(token)?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_NOT_FOUND",
                ErrorCategory::Storage,
                format!("scanner recovery record is unavailable: {error}"),
                false,
            )
            .with_detail("token", token)
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_RECORD_INVALID",
                ErrorCategory::Integrity,
                "scanner recovery record must be a bounded regular non-symlink file",
                false,
            )
            .with_detail("token", token));
        }
        let mut file = File::open(&path).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_READ_FAILED",
                ErrorCategory::Storage,
                format!("failed to open scanner recovery record: {error}"),
                true,
            )
        })?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_READ_FAILED",
                ErrorCategory::Storage,
                format!("failed to read scanner recovery record: {error}"),
                true,
            )
        })?;
        if bytes.len() as u64 != metadata.len() {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_CHANGED_DURING_READ",
                ErrorCategory::Integrity,
                "scanner recovery record changed while it was read",
                false,
            ));
        }
        let record = self.parse_scanner_recovery(&bytes)?;
        if record.token != token {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_TOKEN_MISMATCH",
                ErrorCategory::Integrity,
                "scanner recovery filename token does not match record content",
                false,
            ));
        }
        Ok(record)
    }

    fn write_new_scanner_recovery(&self, record: &ScannerRecoveryRecord) -> Result<(), A2dError> {
        let root = self.scanner_recovery_root()?;
        let final_path = self.scanner_recovery_path(&record.token)?;
        let temp_path = root.join(format!(
            ".{}.{}-{}.tmp",
            record.token,
            std::process::id(),
            NEXT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        let bytes = Self::encode_scanner_recovery(record)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| {
                recovery_error(
                    "CORE_SCANNER_RECOVERY_TEMP_CREATE_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to create scanner recovery temp file: {error}"),
                    true,
                )
            })?;
        let write_result = file
            .write_all(&bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temp_path);
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_WRITE_FAILED",
                ErrorCategory::Storage,
                format!("failed to persist scanner recovery temp file: {error}"),
                true,
            ));
        }
        if let Err(error) = std::fs::hard_link(&temp_path, &final_path) {
            let _ = std::fs::remove_file(&temp_path);
            let code = if error.kind() == std::io::ErrorKind::AlreadyExists {
                "CORE_SCANNER_RECOVERY_ALREADY_EXISTS"
            } else {
                "CORE_SCANNER_RECOVERY_FINALIZE_FAILED"
            };
            return Err(recovery_error(
                code,
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    ErrorCategory::Validation
                } else {
                    ErrorCategory::Storage
                },
                format!("failed to finalize scanner recovery record: {error}"),
                error.kind() != std::io::ErrorKind::AlreadyExists,
            )
            .with_detail("token", &record.token));
        }
        sync_directory(&root)?;
        std::fs::remove_file(&temp_path).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_TEMP_CLEANUP_FAILED",
                ErrorCategory::Storage,
                format!("failed to remove finalized scanner recovery temp file: {error}"),
                true,
            )
            .with_detail("temp_path", temp_path.to_string_lossy())
        })?;
        sync_directory(&root)
    }

    fn replace_scanner_recovery(&self, record: &ScannerRecoveryRecord) -> Result<(), A2dError> {
        let root = self.scanner_recovery_root()?;
        let final_path = self.scanner_recovery_path(&record.token)?;
        if !final_path.is_file() {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_NOT_FOUND",
                ErrorCategory::Storage,
                "scanner recovery record disappeared before update",
                false,
            )
            .with_detail("token", &record.token));
        }
        let temp_path = root.join(format!(
            ".{}.{}-{}.tmp",
            record.token,
            std::process::id(),
            NEXT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        let bytes = Self::encode_scanner_recovery(record)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| {
                recovery_error(
                    "CORE_SCANNER_RECOVERY_TEMP_CREATE_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to create scanner recovery update file: {error}"),
                    true,
                )
            })?;
        file.write_all(&bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|error| {
                recovery_error(
                    "CORE_SCANNER_RECOVERY_WRITE_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to persist scanner recovery update: {error}"),
                    true,
                )
            })?;
        drop(file);
        std::fs::rename(&temp_path, &final_path).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_REPLACE_FAILED",
                ErrorCategory::Storage,
                format!("failed to atomically replace scanner recovery record: {error}"),
                true,
            )
            .with_detail("temp_path", temp_path.to_string_lossy())
            .with_detail("record_path", final_path.to_string_lossy())
        })?;
        sync_directory(&root)
    }

    pub fn begin_scanner_recovery(
        &self,
        request: BeginScannerRecoveryRequest,
    ) -> Result<ScannerRecoveryRecord, A2dError> {
        validate_token(&request.token)?;
        if request.captured_at_ms <= 0 || request.processing_policy_version == 0 {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_REQUEST_INVALID",
                ErrorCategory::Validation,
                "captured_at_ms and processing_policy_version must be positive",
                false,
            ));
        }
        if self.list_scanner_recoveries()?.len() >= MAX_RECORDS {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_LIMIT_EXCEEDED",
                ErrorCategory::Storage,
                format!("scanner recovery record limit {MAX_RECORDS} was reached"),
                false,
            ));
        }
        let staging = self.validate_recovery_staging_file(&request.staging_path)?;
        let record = ScannerRecoveryRecord {
            token: request.token,
            staging_path: staging.to_string_lossy().into_owned(),
            page_id: request.page_id,
            notebook_id: request.notebook_id,
            captured_at_ms: request.captured_at_ms,
            layout_id: request.layout_id,
            processing_policy_version: request.processing_policy_version,
            phase: ScannerRecoveryPhase::Captured,
            registered_scan_id: None,
            updated_at_ms: system_now_ms()?,
        };
        self.write_new_scanner_recovery(&record)?;
        Ok(record)
    }

    pub fn list_scanner_recoveries(&self) -> Result<Vec<ScannerRecoveryRecord>, A2dError> {
        let root = self.scanner_recovery_root()?;
        let mut paths = std::fs::read_dir(&root)
            .map_err(|error| {
                recovery_error(
                    "CORE_SCANNER_RECOVERY_LIST_FAILED",
                    ErrorCategory::Storage,
                    format!("failed to list scanner recovery directory: {error}"),
                    true,
                )
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();
        if paths.len() > MAX_RECORDS {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_LIMIT_EXCEEDED",
                ErrorCategory::Integrity,
                format!("scanner recovery directory contains more than {MAX_RECORDS} records"),
                false,
            ));
        }
        let mut records = Vec::with_capacity(paths.len());
        for path in paths {
            let token = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    recovery_error(
                        "CORE_SCANNER_RECOVERY_FILENAME_INVALID",
                        ErrorCategory::Integrity,
                        "scanner recovery filename is not valid UTF-8",
                        false,
                    )
                })?;
            records.push(self.read_scanner_recovery(token)?);
        }
        records.sort_by(|left, right| {
            left.updated_at_ms
                .cmp(&right.updated_at_ms)
                .then_with(|| left.token.cmp(&right.token))
        });
        Ok(records)
    }

    pub fn mark_scanner_recovery_preview_ready(
        &self,
        token: &str,
    ) -> Result<ScannerRecoveryRecord, A2dError> {
        self.advance_scanner_recovery(
            token,
            ScannerRecoveryPhase::Captured,
            ScannerRecoveryPhase::PreviewReady,
            None,
        )
    }

    pub(crate) fn mark_scanner_recovery_registering(
        &self,
        token: &str,
        staging_path: &Path,
        page_id: &PageId,
        notebook_id: &NotebookId,
        layout_id: &LayoutId,
        processing_policy_version: u32,
    ) -> Result<(), A2dError> {
        let record = self.read_scanner_recovery(token)?;
        if record.staging_path != staging_path.to_string_lossy()
            || &record.page_id != page_id
            || &record.notebook_id != notebook_id
            || &record.layout_id != layout_id
            || record.processing_policy_version != processing_policy_version
        {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_REGISTRATION_MISMATCH",
                ErrorCategory::Integrity,
                "scanner recovery identity does not match the registration request",
                false,
            )
            .with_detail("token", token));
        }
        self.advance_scanner_recovery(
            token,
            ScannerRecoveryPhase::PreviewReady,
            ScannerRecoveryPhase::Registering,
            None,
        )?;
        Ok(())
    }

    pub(crate) fn mark_scanner_recovery_committed(
        &self,
        token: &str,
        scan_id: &ScanId,
    ) -> Result<(), A2dError> {
        self.advance_scanner_recovery(
            token,
            ScannerRecoveryPhase::Registering,
            ScannerRecoveryPhase::Committed,
            Some(scan_id.clone()),
        )?;
        Ok(())
    }

    fn advance_scanner_recovery(
        &self,
        token: &str,
        expected: ScannerRecoveryPhase,
        next: ScannerRecoveryPhase,
        registered_scan_id: Option<ScanId>,
    ) -> Result<ScannerRecoveryRecord, A2dError> {
        let mut record = self.read_scanner_recovery(token)?;
        if record.phase != expected {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_PHASE_CONFLICT",
                ErrorCategory::Integrity,
                format!(
                    "scanner recovery phase is {}, expected {}",
                    record.phase.as_str(),
                    expected.as_str()
                ),
                false,
            )
            .with_detail("token", token));
        }
        record.phase = next;
        record.registered_scan_id = registered_scan_id;
        record.updated_at_ms = system_now_ms()?;
        self.replace_scanner_recovery(&record)?;
        Ok(record)
    }

    pub fn discard_scanner_recovery(&self, token: &str) -> Result<(), A2dError> {
        let record = self.read_scanner_recovery(token)?;
        if !matches!(
            record.phase,
            ScannerRecoveryPhase::Captured | ScannerRecoveryPhase::PreviewReady
        ) {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_DISCARD_NOT_ALLOWED",
                ErrorCategory::Integrity,
                "registering or committed scanner recovery cannot be discarded",
                false,
            )
            .with_detail("token", token)
            .with_detail("phase", record.phase.as_str()));
        }
        let staging = self.validate_recovery_staging_file(&record.staging_path)?;
        std::fs::remove_file(&staging).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_STAGING_DELETE_FAILED",
                ErrorCategory::Storage,
                format!("failed to remove discarded scanner staging file: {error}"),
                true,
            )
            .with_detail("staging_path", staging.to_string_lossy())
        })?;
        self.remove_scanner_recovery_record(token)
    }

    pub fn acknowledge_committed_scanner_recovery(
        &self,
        token: &str,
        scan_id: &ScanId,
    ) -> Result<(), A2dError> {
        let record = self.read_scanner_recovery(token)?;
        if record.phase != ScannerRecoveryPhase::Committed
            || record.registered_scan_id.as_ref() != Some(scan_id)
        {
            return Err(recovery_error(
                "CORE_SCANNER_RECOVERY_ACKNOWLEDGEMENT_MISMATCH",
                ErrorCategory::Integrity,
                "scanner recovery acknowledgement does not match the committed scan",
                false,
            )
            .with_detail("token", token)
            .with_detail("scan_id", scan_id.to_string()));
        }
        self.remove_scanner_recovery_record(token)
    }

    fn remove_scanner_recovery_record(&self, token: &str) -> Result<(), A2dError> {
        let root = self.scanner_recovery_root()?;
        let path = self.scanner_recovery_path(token)?;
        std::fs::remove_file(&path).map_err(|error| {
            recovery_error(
                "CORE_SCANNER_RECOVERY_REMOVE_FAILED",
                ErrorCategory::Storage,
                format!("failed to remove scanner recovery record: {error}"),
                true,
            )
            .with_detail("token", token)
        })?;
        sync_directory(&root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;

    fn open_core() -> (std::sync::Arc<A2dCore>, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("a2d-scanner-recovery-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, root)
    }

    fn begin(core: &A2dCore, root: &Path, token: &str) -> ScannerRecoveryRecord {
        let staging = root
            .join("tmp")
            .join(STAGING_DIRECTORY)
            .join(format!("{token}.jpg"));
        std::fs::create_dir_all(staging.parent().unwrap()).unwrap();
        std::fs::write(&staging, b"capture").unwrap();
        core.begin_scanner_recovery(BeginScannerRecoveryRequest {
            token: token.to_string(),
            staging_path: staging.to_string_lossy().into_owned(),
            page_id: PageId::generate(),
            notebook_id: NotebookId::generate(),
            captured_at_ms: 1,
            layout_id: LayoutId::parse("USLETTER-LINED").unwrap(),
            processing_policy_version: 1,
        })
        .unwrap()
    }

    #[test]
    fn captured_preview_and_discard_are_durable_and_explicit() {
        let (core, root) = open_core();
        let captured = begin(&core, &root, "recover-one");
        assert_eq!(captured.phase, ScannerRecoveryPhase::Captured);
        let preview = core
            .mark_scanner_recovery_preview_ready("recover-one")
            .unwrap();
        assert_eq!(preview.phase, ScannerRecoveryPhase::PreviewReady);
        drop(core);

        let reopened = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        assert_eq!(reopened.list_scanner_recoveries().unwrap(), vec![preview]);
        reopened.discard_scanner_recovery("recover-one").unwrap();
        assert!(reopened.list_scanner_recoveries().unwrap().is_empty());
        assert!(!Path::new(&captured.staging_path).exists());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn committed_recovery_cannot_be_discarded_or_acknowledged_with_another_scan() {
        let (core, root) = open_core();
        let captured = begin(&core, &root, "recover-two");
        core.mark_scanner_recovery_preview_ready("recover-two")
            .unwrap();
        core.mark_scanner_recovery_registering(
            "recover-two",
            Path::new(&captured.staging_path),
            &captured.page_id,
            &captured.notebook_id,
            &captured.layout_id,
            captured.processing_policy_version,
        )
        .unwrap();
        let scan_id = ScanId::generate();
        core.mark_scanner_recovery_committed("recover-two", &scan_id)
            .unwrap();
        assert!(core.discard_scanner_recovery("recover-two").is_err());
        assert!(
            core.acknowledge_committed_scanner_recovery("recover-two", &ScanId::generate())
                .is_err()
        );
        core.acknowledge_committed_scanner_recovery("recover-two", &scan_id)
            .unwrap();
        assert!(core.list_scanner_recoveries().unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn corrupt_record_fails_closed_instead_of_disappearing() {
        let (core, root) = open_core();
        let recovery_root = root.join("tmp").join(RECOVERY_DIRECTORY);
        std::fs::create_dir_all(&recovery_root).unwrap();
        std::fs::write(recovery_root.join("corrupt.json"), b"not-json").unwrap();
        let error = core.list_scanner_recoveries().unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "CORE_SCANNER_RECOVERY_RECORD_CORRUPT"
        );
        std::fs::remove_dir_all(root).ok();
    }
}
