//! JSON encode/decode for the list/map-shaped columns `migrations/0001_initial.sql` documents as
//! JSON-encoded `TEXT` (marker_role_ids, warnings, details, polygon, permission lists).
//!
//! Deliberately fallible in both directions and never silently substitutes a default on failure
//! (CLAUDE.md: "Errors MUST NOT be reduced to `null`, empty lists, or `false`") — a corrupt
//! column reads back as an `Integrity` error, not a quietly-empty `Vec`/`BTreeMap`.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use serde::{Serialize, de::DeserializeOwned};

pub fn encode_json<T: Serialize>(value: &T, column: &str) -> Result<String, A2dError> {
    serde_json::to_string(value).map_err(|e| {
        A2dError::new(
            ErrorCode::new("STORAGE_JSON_ENCODE_FAILED"),
            ErrorCategory::Storage,
            ErrorSeverity::Error,
            "error.storage.json_encode_failed",
            format!("failed to encode column `{column}`: {e}"),
            false,
        )
        .with_detail("column", column)
    })
}

pub fn decode_json<T: DeserializeOwned>(raw: &str, column: &str) -> Result<T, A2dError> {
    serde_json::from_str(raw).map_err(|e| {
        A2dError::new(
            ErrorCode::new("STORAGE_CORRUPT_JSON_COLUMN"),
            ErrorCategory::Integrity,
            ErrorSeverity::Critical,
            "error.storage.corrupt_json_column",
            format!("column `{column}` does not contain valid JSON for its expected shape: {e}"),
            false,
        )
        .with_detail("column", column)
        .with_detail("raw", raw)
    })
}
