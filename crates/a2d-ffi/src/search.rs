//! UniFFI projection for local OCR search and OCR correction review.
//!
//! The actual full-text index is SQLite-owned storage state. This module exposes bounded search
//! DTOs and append-only text-correction DTOs through the same Rust-owned FFI boundary as OCR
//! record/readback APIs.

use a2d_core as core;

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum OcrSearchDocumentKind {
    FullText,
    TextRegion,
}

impl From<core::CoreOcrSearchDocumentKind> for OcrSearchDocumentKind {
    fn from(value: core::CoreOcrSearchDocumentKind) -> Self {
        match value {
            core::CoreOcrSearchDocumentKind::FullText => Self::FullText,
            core::CoreOcrSearchDocumentKind::TextRegion => Self::TextRegion,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct SearchOcrTextRequest {
    pub query: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct OcrTextSearchHit {
    pub page_id: String,
    pub scan_id: String,
    pub ocr_run_id: String,
    pub text_region_id: Option<String>,
    pub document_kind: OcrSearchDocumentKind,
    pub snippet: String,
}

impl From<core::OcrTextSearchHit> for OcrTextSearchHit {
    fn from(value: core::OcrTextSearchHit) -> Self {
        Self {
            page_id: value.page_id,
            scan_id: value.scan_id,
            ocr_run_id: value.ocr_run_id,
            text_region_id: value.text_region_id,
            document_kind: value.document_kind.into(),
            snippet: value.snippet,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct SearchOcrTextResults {
    pub query: String,
    pub hits: Vec<OcrTextSearchHit>,
}

impl From<core::SearchOcrTextResults> for SearchOcrTextResults {
    fn from(value: core::SearchOcrTextResults) -> Self {
        Self {
            query: value.query,
            hits: value.hits.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct RecordOcrCorrectionRequest {
    pub scan_id: String,
    pub text_region_id: Option<String>,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub created_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct RecordedOcrCorrection {
    pub text_correction_id: String,
    pub scan_id: String,
    pub text_region_id: Option<String>,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub created_at_ms: i64,
}

impl From<core::RecordedOcrCorrection> for RecordedOcrCorrection {
    fn from(value: core::RecordedOcrCorrection) -> Self {
        Self {
            text_correction_id: value.text_correction_id,
            scan_id: value.scan_id,
            text_region_id: value.text_region_id,
            corrected_text: value.corrected_text,
            previous_text: value.previous_text,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ListOcrCorrectionsForScanRequest {
    pub scan_id: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct OcrTextCorrection {
    pub text_correction_id: String,
    pub scan_id: String,
    pub text_region_id: Option<String>,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub created_at_ms: i64,
}

impl From<core::OcrTextCorrection> for OcrTextCorrection {
    fn from(value: core::OcrTextCorrection) -> Self {
        Self {
            text_correction_id: value.text_correction_id,
            scan_id: value.scan_id,
            text_region_id: value.text_region_id,
            corrected_text: value.corrected_text,
            previous_text: value.previous_text,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ListOcrCorrectionsForScanResult {
    pub scan_id: String,
    pub corrections: Vec<OcrTextCorrection>,
}

impl From<core::ListOcrCorrectionsForScanResult> for ListOcrCorrectionsForScanResult {
    fn from(value: core::ListOcrCorrectionsForScanResult) -> Self {
        Self {
            scan_id: value.scan_id,
            corrections: value.corrections.into_iter().map(Into::into).collect(),
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn search_ocr_text(
        &self,
        request: SearchOcrTextRequest,
    ) -> Result<SearchOcrTextResults, A2dFfiError> {
        self.core
            .search_ocr_text(core::SearchOcrTextRequest {
                query: request.query,
                limit: request.limit,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn record_ocr_correction(
        &self,
        request: RecordOcrCorrectionRequest,
    ) -> Result<RecordedOcrCorrection, A2dFfiError> {
        self.core
            .record_ocr_correction(core::RecordOcrCorrectionRequest {
                scan_id: request.scan_id,
                text_region_id: request.text_region_id,
                corrected_text: request.corrected_text,
                previous_text: request.previous_text,
                created_at_ms: request.created_at_ms,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn list_ocr_corrections_for_scan(
        &self,
        request: ListOcrCorrectionsForScanRequest,
    ) -> Result<ListOcrCorrectionsForScanResult, A2dFfiError> {
        self.core
            .list_ocr_corrections_for_scan(core::ListOcrCorrectionsForScanRequest {
                scan_id: request.scan_id,
                limit: request.limit,
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
            "a2d-ffi-ocr-search-test-{}",
            a2d_domain::PageId::generate()
        ));
        A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .expect("open must succeed for a fresh directory")
    }

    #[test]
    fn search_ocr_text_validation_errors_cross_the_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .search_ocr_text(SearchOcrTextRequest {
                query: "   ".to_string(),
                limit: 10,
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "STORAGE_OCR_SEARCH_QUERY_EMPTY");
        assert_eq!(details.category, "Validation");
    }

    #[test]
    fn record_ocr_correction_validation_errors_cross_the_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .record_ocr_correction(RecordOcrCorrectionRequest {
                scan_id: a2d_domain::ScanId::generate().to_string(),
                text_region_id: None,
                corrected_text: "   ".to_string(),
                previous_text: None,
                created_at_ms: Some(300),
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "CORE_OCR_CORRECTION_TEXT_EMPTY");
        assert_eq!(details.category, "Ocr");
    }

    #[test]
    fn list_ocr_corrections_validation_errors_cross_the_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .list_ocr_corrections_for_scan(ListOcrCorrectionsForScanRequest {
                scan_id: a2d_domain::ScanId::generate().to_string(),
                limit: 0,
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "CORE_OCR_CORRECTION_LIST_LIMIT_INVALID");
        assert_eq!(details.category, "Ocr");
    }
}
