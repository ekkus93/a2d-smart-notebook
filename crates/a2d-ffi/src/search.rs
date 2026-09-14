//! UniFFI projection for local OCR search.
//!
//! The actual full-text index is SQLite-owned storage state. This module exposes a bounded search
//! DTO through the same Rust-owned FFI boundary as OCR record/readback APIs.

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
}
