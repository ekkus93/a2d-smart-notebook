use a2d_core as core;

use super::{A2dClient, A2dFfiError, OcrInputKind, OcrRunStatus, OcrUnavailableReason};

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

impl From<core::CoreOcrTextPoint> for OcrTextPoint {
    fn from(value: core::CoreOcrTextPoint) -> Self {
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

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct LoadLatestOcrOutputRequest {
    pub scan_id: String,
    pub region_limit: u32,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LoadedOcrTextRegion {
    pub text_region_id: String,
    pub ocr_run_id: String,
    pub polygon: Vec<OcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: i64,
}

impl From<core::LoadedOcrTextRegion> for LoadedOcrTextRegion {
    fn from(value: core::LoadedOcrTextRegion) -> Self {
        Self {
            text_region_id: value.text_region_id,
            ocr_run_id: value.ocr_run_id,
            polygon: value.polygon.into_iter().map(Into::into).collect(),
            text: value.text,
            confidence: value.confidence,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LoadedOcrRun {
    pub ocr_run_id: String,
    pub scan_id: String,
    pub input_asset_id: Option<String>,
    pub provider: String,
    pub provider_version: String,
    pub model_name: Option<String>,
    pub status: OcrRunStatus,
    pub full_text: String,
    pub unavailable_reason: Option<OcrUnavailableReason>,
    pub unavailable_message: Option<String>,
    pub retry_available: bool,
    pub completed_at_ms: Option<i64>,
    pub warnings: Vec<String>,
    pub text_region_count: u32,
    pub text_regions: Vec<LoadedOcrTextRegion>,
}

impl From<core::LoadedOcrRun> for LoadedOcrRun {
    fn from(value: core::LoadedOcrRun) -> Self {
        Self {
            ocr_run_id: value.ocr_run_id,
            scan_id: value.scan_id,
            input_asset_id: value.input_asset_id,
            provider: value.provider,
            provider_version: value.provider_version,
            model_name: value.model_name,
            status: value.status.into(),
            full_text: value.full_text,
            unavailable_reason: value.unavailable_reason.map(Into::into),
            unavailable_message: value.unavailable_message,
            retry_available: value.retry_available,
            completed_at_ms: value.completed_at_ms,
            warnings: value.warnings,
            text_region_count: value.text_region_count,
            text_regions: value.text_regions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LoadedOcrOutput {
    pub scan_id: String,
    pub latest_run: Option<LoadedOcrRun>,
}

impl From<core::LoadedOcrOutput> for LoadedOcrOutput {
    fn from(value: core::LoadedOcrOutput) -> Self {
        Self {
            scan_id: value.scan_id,
            latest_run: value.latest_run.map(Into::into),
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

    pub fn load_latest_ocr_output(
        &self,
        request: LoadLatestOcrOutputRequest,
    ) -> Result<LoadedOcrOutput, A2dFfiError> {
        self.core
            .load_latest_ocr_output(core::LoadLatestOcrOutputRequest {
                scan_id: request.scan_id,
                region_limit: request.region_limit,
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
                regions: vec![region("orphan")],
            })
            .unwrap_err();

        assert_eq!(err.code, "STORAGE_OCR_RUN_MISSING");
    }

    #[test]
    fn load_latest_ocr_output_rejects_invalid_scan_id_at_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: "not-an-id".to_string(),
                region_limit: 10,
            })
            .unwrap_err();

        assert_eq!(err.code, "DOMAIN_ID_INVALID");
    }

    #[test]
    fn load_latest_ocr_output_rejects_unbounded_region_limit_at_ffi_boundary() {
        let client = open_test_client();
        let err = client
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: a2d_domain::ScanId::generate().to_string(),
                region_limit: 1_001,
            })
            .unwrap_err();

        assert_eq!(err.code, "CORE_OCR_READBACK_REGION_LIMIT_EXCEEDED");
    }
}
