//! UniFFI projection for OCR text-region persistence.
//!
//! Platform OCR providers can report recognized region geometry, but they cannot write SQL-shaped
//! region rows directly. This module projects a bounded FFI-safe batch into the Rust core OCR
//! text-region API.

use a2d_core as core;

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct OcrTextPoint {
    pub x: f32,
    pub y: f32,
}

impl From<OcrTextPoint> for core::CoreOcrTextPoint {
    fn from(value: OcrTextPoint) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct RecordOcrTextRegionRequest {
    pub polygon: Vec<OcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: Option<i64>,
}

impl From<RecordOcrTextRegionRequest> for core::RecordOcrTextRegionRequest {
    fn from(value: RecordOcrTextRegionRequest) -> Self {
        Self {
            polygon: value.polygon.into_iter().map(Into::into).collect(),
            text: value.text,
            confidence: value.confidence,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct RecordOcrTextRegionsRequest {
    pub ocr_run_id: String,
    pub regions: Vec<RecordOcrTextRegionRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct RecordedOcrTextRegion {
    pub text_region_id: String,
    pub ocr_run_id: String,
    pub text: String,
}

impl From<core::RecordedOcrTextRegion> for RecordedOcrTextRegion {
    fn from(value: core::RecordedOcrTextRegion) -> Self {
        Self {
            text_region_id: value.text_region_id,
            ocr_run_id: value.ocr_run_id,
            text: value.text,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct RecordedOcrTextRegions {
    pub ocr_run_id: String,
    pub regions: Vec<RecordedOcrTextRegion>,
}

impl From<core::RecordedOcrTextRegions> for RecordedOcrTextRegions {
    fn from(value: core::RecordedOcrTextRegions) -> Self {
        Self {
            ocr_run_id: value.ocr_run_id,
            regions: value.regions.into_iter().map(Into::into).collect(),
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn record_ocr_text_regions(
        &self,
        request: RecordOcrTextRegionsRequest,
    ) -> Result<RecordedOcrTextRegions, A2dFfiError> {
        self.core
            .record_ocr_text_regions(core::RecordOcrTextRegionsRequest {
                ocr_run_id: request.ocr_run_id,
                regions: request.regions.into_iter().map(Into::into).collect(),
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
            "a2d-ffi-ocr-region-test-{}",
            a2d_domain::PageId::generate()
        ));
        A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .expect("open must succeed for a fresh directory")
    }

    fn region(text: &str) -> RecordOcrTextRegionRequest {
        RecordOcrTextRegionRequest {
            polygon: vec![
                OcrTextPoint { x: 0.0, y: 0.0 },
                OcrTextPoint { x: 10.0, y: 0.0 },
                OcrTextPoint { x: 10.0, y: 10.0 },
            ],
            text: text.to_string(),
            confidence: Some(0.75),
            created_at_ms: Some(300),
        }
    }

    #[test]
    fn record_ocr_text_region_errors_cross_the_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .record_ocr_text_regions(RecordOcrTextRegionsRequest {
                ocr_run_id: a2d_domain::OcrRunId::generate().to_string(),
                regions: vec![region("orphaned")],
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "STORAGE_TEXT_REGION_OCR_RUN_MISSING");
        assert_eq!(details.category, "Validation");
        assert!(!details.retryable);
    }

    #[test]
    fn record_ocr_text_region_empty_batch_error_is_core_owned() {
        let client = open_test_client();
        let err = client
            .record_ocr_text_regions(RecordOcrTextRegionsRequest {
                ocr_run_id: a2d_domain::OcrRunId::generate().to_string(),
                regions: Vec::new(),
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;

        assert_eq!(details.code, "CORE_OCR_TEXT_REGION_BATCH_EMPTY");
        assert_eq!(details.category, "Ocr");
    }
}
