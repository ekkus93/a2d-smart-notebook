//! Thin UniFFI projection for Milestone 11 OCR input preparation and result recording.
//!
//! Platform adapters may perform recognition, but they only pass bounded request/result data
//! across this boundary. Rust core remains responsible for scan lookup, scan-owned asset checks,
//! terminal OCR outcome validation, and durable storage.

use a2d_core as core;

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum OcrInputKind {
    Original,
    Corrected,
    OcrOptimized,
}

impl From<OcrInputKind> for core::CoreOcrInputKind {
    fn from(value: OcrInputKind) -> Self {
        match value {
            OcrInputKind::Original => Self::Original,
            OcrInputKind::Corrected => Self::Corrected,
            OcrInputKind::OcrOptimized => Self::OcrOptimized,
        }
    }
}

impl From<core::CoreOcrInputKind> for OcrInputKind {
    fn from(value: core::CoreOcrInputKind) -> Self {
        match value {
            core::CoreOcrInputKind::Original => Self::Original,
            core::CoreOcrInputKind::Corrected => Self::Corrected,
            core::CoreOcrInputKind::OcrOptimized => Self::OcrOptimized,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum OcrRunStatus {
    Detected,
    NoTextDetected,
    Unavailable,
}

impl From<OcrRunStatus> for a2d_domain::OcrRunStatus {
    fn from(value: OcrRunStatus) -> Self {
        match value {
            OcrRunStatus::Detected => Self::Detected,
            OcrRunStatus::NoTextDetected => Self::NoTextDetected,
            OcrRunStatus::Unavailable => Self::Unavailable,
        }
    }
}

impl From<a2d_domain::OcrRunStatus> for OcrRunStatus {
    fn from(value: a2d_domain::OcrRunStatus) -> Self {
        match value {
            a2d_domain::OcrRunStatus::Detected => Self::Detected,
            a2d_domain::OcrRunStatus::NoTextDetected => Self::NoTextDetected,
            a2d_domain::OcrRunStatus::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum OcrUnavailableReason {
    ProviderUnavailable,
    ProviderFailed,
    ResourceUnavailable,
    UnsupportedInput,
    Cancelled,
}

impl From<OcrUnavailableReason> for a2d_domain::OcrUnavailableReason {
    fn from(value: OcrUnavailableReason) -> Self {
        match value {
            OcrUnavailableReason::ProviderUnavailable => Self::ProviderUnavailable,
            OcrUnavailableReason::ProviderFailed => Self::ProviderFailed,
            OcrUnavailableReason::ResourceUnavailable => Self::ResourceUnavailable,
            OcrUnavailableReason::UnsupportedInput => Self::UnsupportedInput,
            OcrUnavailableReason::Cancelled => Self::Cancelled,
        }
    }
}

impl From<a2d_domain::OcrUnavailableReason> for OcrUnavailableReason {
    fn from(value: a2d_domain::OcrUnavailableReason) -> Self {
        match value {
            a2d_domain::OcrUnavailableReason::ProviderUnavailable => Self::ProviderUnavailable,
            a2d_domain::OcrUnavailableReason::ProviderFailed => Self::ProviderFailed,
            a2d_domain::OcrUnavailableReason::ResourceUnavailable => Self::ResourceUnavailable,
            a2d_domain::OcrUnavailableReason::UnsupportedInput => Self::UnsupportedInput,
            a2d_domain::OcrUnavailableReason::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct PrepareOcrInputRequest {
    pub scan_id: String,
    pub input_kind: OcrInputKind,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct PreparedOcrInput {
    pub scan_id: String,
    pub input_asset_id: String,
    pub input_kind: OcrInputKind,
    pub media_type: String,
    pub relative_path: String,
    pub byte_length: u64,
    pub width_px: u32,
    pub height_px: u32,
}

impl From<core::PreparedOcrInput> for PreparedOcrInput {
    fn from(value: core::PreparedOcrInput) -> Self {
        Self {
            scan_id: value.scan_id,
            input_asset_id: value.input_asset_id,
            input_kind: value.input_kind.into(),
            media_type: value.media_type,
            relative_path: value.relative_path,
            byte_length: value.byte_length,
            width_px: value.width_px,
            height_px: value.height_px,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct RecordOcrRunRequest {
    pub scan_id: String,
    pub input_asset_id: String,
    pub provider: String,
    pub provider_version: String,
    pub model_name: Option<String>,
    pub status: OcrRunStatus,
    pub full_text: String,
    pub unavailable_reason: Option<OcrUnavailableReason>,
    pub unavailable_message: Option<String>,
    pub completed_at_ms: Option<i64>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct RecordedOcrRun {
    pub ocr_run_id: String,
    pub scan_id: String,
    pub input_asset_id: String,
    pub status: OcrRunStatus,
}

impl From<core::RecordedOcrRun> for RecordedOcrRun {
    fn from(value: core::RecordedOcrRun) -> Self {
        Self {
            ocr_run_id: value.ocr_run_id,
            scan_id: value.scan_id,
            input_asset_id: value.input_asset_id,
            status: value.status.into(),
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn prepare_ocr_input(
        &self,
        request: PrepareOcrInputRequest,
    ) -> Result<PreparedOcrInput, A2dFfiError> {
        self.core
            .prepare_ocr_input(core::PrepareOcrInputRequest {
                scan_id: request.scan_id,
                input_kind: request.input_kind.into(),
                width_px: request.width_px,
                height_px: request.height_px,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn record_ocr_run(
        &self,
        request: RecordOcrRunRequest,
    ) -> Result<RecordedOcrRun, A2dFfiError> {
        self.core
            .record_ocr_run(core::RecordOcrRunRequest {
                scan_id: request.scan_id,
                input_asset_id: request.input_asset_id,
                provider: request.provider,
                provider_version: request.provider_version,
                model_name: request.model_name,
                status: request.status.into(),
                full_text: request.full_text,
                unavailable_reason: request.unavailable_reason.map(Into::into),
                unavailable_message: request.unavailable_message,
                completed_at_ms: request.completed_at_ms,
                warnings: request.warnings,
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
            "a2d-ffi-ocr-test-{}",
            a2d_domain::PageId::generate()
        ));
        A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .expect("open must succeed for a fresh directory")
    }

    #[test]
    fn prepare_ocr_input_errors_cross_the_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .prepare_ocr_input(PrepareOcrInputRequest {
                scan_id: a2d_domain::ScanId::generate().to_string(),
                input_kind: OcrInputKind::Original,
                width_px: 0,
                height_px: 1_000,
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "CORE_OCR_IMAGE_DIMENSIONS_INVALID");
        assert_eq!(details.category, "Ocr");
        assert_eq!(
            details
                .details
                .iter()
                .map(|detail| detail.key.as_str())
                .collect::<Vec<_>>(),
            vec!["height_px", "width_px"]
        );
    }

    #[test]
    fn record_ocr_run_errors_cross_the_ffi_boundary_without_storage_details() {
        let client = open_test_client();
        let err = client
            .record_ocr_run(RecordOcrRunRequest {
                scan_id: a2d_domain::ScanId::generate().to_string(),
                input_asset_id: a2d_domain::AssetId::generate().to_string(),
                provider: "mlkit".to_string(),
                provider_version: "2026.09".to_string(),
                model_name: Some("latin-v1".to_string()),
                status: OcrRunStatus::NoTextDetected,
                full_text: String::new(),
                unavailable_reason: None,
                unavailable_message: None,
                completed_at_ms: Some(250),
                warnings: Vec::new(),
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "CORE_OCR_RECORD_SCAN_MISSING");
        assert_eq!(details.category, "Ocr");
        assert!(!details.retryable);
    }
}
