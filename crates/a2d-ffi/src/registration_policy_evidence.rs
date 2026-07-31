//! Validation for reviewed-preview policy identity carried through the existing warning vector.

use a2d_core::A2dCore;
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId};

const LAYOUT_PREFIX: &str = "A2D_POLICY_LAYOUT=";
const POLICY_PREFIX: &str = "A2D_POLICY_VERSION=";
const PIPELINE_PREFIX: &str = "A2D_PIPELINE_VERSION=";

pub fn validate_and_strip_registration_policy_evidence(
    core: &A2dCore,
    page_id: &PageId,
    values: Vec<String>,
) -> Result<Vec<String>, A2dError> {
    let mut layout_id = None;
    let mut policy_version = None;
    let mut pipeline_version = None;
    let mut warnings = Vec::new();

    for value in values {
        if let Some(raw) = value.strip_prefix(LAYOUT_PREFIX) {
            set_once(&mut layout_id, raw.to_string(), "layout")?;
        } else if let Some(raw) = value.strip_prefix(POLICY_PREFIX) {
            set_once(
                &mut policy_version,
                parse_version(raw, "processing policy")?,
                "processing policy",
            )?;
        } else if let Some(raw) = value.strip_prefix(PIPELINE_PREFIX) {
            set_once(
                &mut pipeline_version,
                parse_version(raw, "pipeline")?,
                "pipeline",
            )?;
        } else {
            warnings.push(value);
        }
    }

    let layout_id = required(layout_id, "layout")?;
    let policy_version = required(policy_version, "processing policy")?;
    let pipeline_version = required(pipeline_version, "pipeline")?;
    let current = core.resolve_stored_scan_processing_policy(page_id)?;

    if layout_id != current.layout_id.to_string()
        || policy_version != current.policy_version
        || pipeline_version != current.pipeline_version()
    {
        return Err(evidence_error(
            "FFI_SCAN_PREVIEW_POLICY_MISMATCH",
            ErrorCategory::Integrity,
            "the reviewed preview policy no longer matches the stored page policy",
        )
        .with_detail("page_id", page_id.to_string())
        .with_detail("reviewed_layout_id", layout_id)
        .with_detail("current_layout_id", current.layout_id.to_string())
        .with_detail("reviewed_policy_version", policy_version.to_string())
        .with_detail("current_policy_version", current.policy_version.to_string())
        .with_detail("reviewed_pipeline_version", pipeline_version.to_string())
        .with_detail(
            "current_pipeline_version",
            current.pipeline_version().to_string(),
        ));
    }

    Ok(warnings)
}

fn parse_version(raw: &str, field: &'static str) -> Result<u32, A2dError> {
    raw.parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            evidence_error(
                "FFI_SCAN_PREVIEW_POLICY_EVIDENCE_INVALID",
                ErrorCategory::Validation,
                format!("reviewed preview {field} version is invalid"),
            )
            .with_detail("field", field)
            .with_detail("value", raw)
        })
}

fn set_once<T>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), A2dError> {
    if slot.replace(value).is_some() {
        return Err(evidence_error(
            "FFI_SCAN_PREVIEW_POLICY_EVIDENCE_DUPLICATE",
            ErrorCategory::Validation,
            format!("reviewed preview {field} evidence was repeated"),
        )
        .with_detail("field", field));
    }
    Ok(())
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, A2dError> {
    value.ok_or_else(|| {
        evidence_error(
            "FFI_SCAN_PREVIEW_POLICY_EVIDENCE_MISSING",
            ErrorCategory::Integrity,
            format!("reviewed preview {field} evidence is required for registration"),
        )
        .with_detail("field", field)
    })
}

fn evidence_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.ffi.scan_preview_policy",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_versions_are_rejected() {
        let error = parse_version("0", "pipeline").unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "FFI_SCAN_PREVIEW_POLICY_EVIDENCE_INVALID"
        );
    }

    #[test]
    fn duplicate_evidence_is_rejected() {
        let mut slot = Some(1_u32);
        let error = set_once(&mut slot, 2, "pipeline").unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "FFI_SCAN_PREVIEW_POLICY_EVIDENCE_DUPLICATE"
        );
    }
}
