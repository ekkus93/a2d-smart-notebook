//! The structured error envelope every fallible domain operation returns (TODO 2.2, spec §27).
//!
//! `ErrorCategory::Cancellation` exists so callers can *describe* a cancellation-flavored error
//! when they must (spec §27 lists cancellation among the error categories). Operations that
//! support user-initiated cancellation SHOULD instead return [`Outcome`], which keeps a cancelled
//! run structurally distinct from a failed one (TODO 2.2: "map cancellation separately from
//! failure") rather than forcing every cancellable call site to build a full `A2dError`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use crate::id::generate_correlation_id;

/// A stable, callsite-defined error code, e.g. `ErrorCode::new("ID_INVALID_LENGTH")`.
///
/// Deliberately not a single central enum: codes are defined next to the code that raises them,
/// so adding a new failure mode never requires editing a shared type. Holds `Cow` so most call
/// sites pay nothing (a `&'static str` literal) while a few callers that need to compose a code
/// at runtime (e.g. per-identifier-type codes) can still produce one without leaking memory.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ErrorCode(pub Cow<'static, str>);

impl ErrorCode {
    pub fn new(code: impl Into<Cow<'static, str>>) -> Self {
        ErrorCode(code.into())
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error categories from spec §27. `Cancellation` is included for completeness but SHOULD NOT be
/// the primary way cancellation is represented — prefer [`Outcome::Cancelled`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ErrorCategory {
    Validation,
    Identity,
    UnsupportedFormat,
    CaptureQuality,
    ImageProcessing,
    Storage,
    Integrity,
    Migration,
    Backup,
    Restore,
    Ocr,
    Search,
    SkillPermission,
    ModelProvider,
    PlatformAdapter,
    Cancellation,
    Internal,
}

/// Severity is not enumerated in the spec; this is a starting assumption (recorded in
/// `memory.md`) covering the levels the UI and logs need to distinguish.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// The fields of [`A2dError`]. Kept separate and boxed inside `A2dError` so that `Result<T,
/// A2dError>` stays cheap to return by value everywhere in this codebase (clippy's
/// `result_large_err`); field access on `A2dError` works exactly as if the fields lived there
/// directly, via `Deref`.
///
/// `details` MUST NOT contain secrets, API keys, passwords, or raw note content (spec §16.1,
/// §28) — enforcing that is each producing module's responsibility, since only that module knows
/// what its own values mean.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct A2dErrorFields {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub user_message_key: String,
    pub developer_message: String,
    pub retryable: bool,
    pub correlation_id: String,
    pub details: BTreeMap<String, String>,
}

/// The structured error envelope every fallible domain operation returns. See TODO §2.2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct A2dError(Box<A2dErrorFields>);

impl std::ops::Deref for A2dError {
    type Target = A2dErrorFields;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for A2dError {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl A2dError {
    /// Builds a new error with a fresh, random correlation ID. `user_message_key` is a lookup
    /// key into platform-localized strings, never end-user-facing text itself.
    pub fn new(
        code: ErrorCode,
        category: ErrorCategory,
        severity: ErrorSeverity,
        user_message_key: impl Into<String>,
        developer_message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self(Box::new(A2dErrorFields {
            code,
            category,
            severity,
            user_message_key: user_message_key.into(),
            developer_message: developer_message.into(),
            retryable,
            correlation_id: generate_correlation_id(),
            details: BTreeMap::new(),
        }))
    }

    /// Attaches a detail key/value pair. Callers MUST NOT pass secrets or raw note content.
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.details.insert(key.into(), value.into());
        self
    }

    /// A stable "this should never happen" error for defects caught at a boundary (e.g. a
    /// panic converted at the FFI edge). Always carries a correlation ID for support/log
    /// correlation even though the cause is otherwise unknown.
    pub fn internal_unknown(developer_message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::new("INTERNAL_UNKNOWN"),
            ErrorCategory::Internal,
            ErrorSeverity::Critical,
            "error.internal_unknown",
            developer_message,
            false,
        )
    }
}

impl fmt::Display for A2dError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] ({:?}/{:?}, retryable={}): {}",
            self.code,
            self.correlation_id,
            self.category,
            self.severity,
            self.retryable,
            self.developer_message
        )
    }
}

impl std::error::Error for A2dError {}

/// The result of an operation that MAY be cancelled. Cancellation is structurally distinct from
/// [`A2dError`] — a cancelled run is not a failure and MUST NOT be coerced into one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome<T> {
    Completed(T),
    Cancelled,
    Failed(A2dError),
}

impl<T> Outcome<T> {
    pub fn is_completed(&self) -> bool {
        matches!(self, Outcome::Completed(_))
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, Outcome::Cancelled)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Outcome::Failed(_))
    }
}

impl<T> From<Result<T, A2dError>> for Outcome<T> {
    fn from(result: Result<T, A2dError>) -> Self {
        match result {
            Ok(value) => Outcome::Completed(value),
            Err(error) => Outcome::Failed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_code_and_correlation_id() {
        let err = A2dError::new(
            ErrorCode::new("VALIDATION_BAD_INPUT"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.bad_input",
            "field 'x' was negative",
            false,
        );
        let rendered = err.to_string();
        assert!(rendered.contains("VALIDATION_BAD_INPUT"));
        assert!(rendered.contains(&err.correlation_id));
        assert!(rendered.contains("field 'x' was negative"));
    }

    #[test]
    fn each_error_gets_a_distinct_correlation_id() {
        let a = A2dError::internal_unknown("boom");
        let b = A2dError::internal_unknown("boom");
        assert_ne!(a.correlation_id, b.correlation_id);
    }

    #[test]
    fn with_detail_is_additive_and_does_not_mutate_in_place() {
        let err = A2dError::new(
            ErrorCode::new("STORAGE_IO"),
            ErrorCategory::Storage,
            ErrorSeverity::Error,
            "error.storage_io",
            "disk write failed",
            true,
        )
        .with_detail("path", "assets/originals/x.jpg")
        .with_detail("errno", "28");
        assert_eq!(
            err.details.get("path").map(String::as_str),
            Some("assets/originals/x.jpg")
        );
        assert_eq!(err.details.get("errno").map(String::as_str), Some("28"));
    }

    #[test]
    fn outcome_never_conflates_cancellation_with_failure() {
        let cancelled: Outcome<u32> = Outcome::Cancelled;
        let failed: Outcome<u32> = Outcome::Failed(A2dError::internal_unknown("x"));
        assert!(cancelled.is_cancelled());
        assert!(!cancelled.is_failed());
        assert!(failed.is_failed());
        assert!(!failed.is_cancelled());
    }

    #[test]
    fn outcome_from_result_maps_ok_and_err() {
        let ok: Outcome<u32> = Ok::<u32, A2dError>(7).into();
        assert_eq!(ok, Outcome::Completed(7));

        let err = A2dError::internal_unknown("x");
        let mapped: Outcome<u32> = Err::<u32, A2dError>(err.clone()).into();
        assert_eq!(mapped, Outcome::Failed(err));
    }
}
