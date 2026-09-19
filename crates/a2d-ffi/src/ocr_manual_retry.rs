use a2d_core as core;

use super::{A2dClient, A2dFfiError, OcrInputKind, OcrQueueJob};

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct OcrSourceGeometry {
    pub scan_id: String,
    pub input_asset_id: String,
    pub input_kind: OcrInputKind,
    pub media_type: String,
    pub relative_path: String,
    pub byte_length: u64,
    pub width_px: u32,
    pub height_px: u32,
}

impl From<core::OcrSourceGeometry> for OcrSourceGeometry {
    fn from(value: core::OcrSourceGeometry) -> Self {
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

#[uniffi::export]
impl A2dClient {
    /// Requests the Rust-owned bounded manual retry transition for a scan's latest OCR job.
    pub fn manual_retry_ocr_job_for_scan(
        &self,
        scan_id: String,
    ) -> Result<OcrQueueJob, A2dFfiError> {
        self.core
            .manual_retry_ocr_job_for_scan(&scan_id)
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Resolves the persisted OCR source asset and its actual encoded image dimensions.
    pub fn resolve_ocr_source_geometry(
        &self,
        scan_id: String,
        input_kind: OcrInputKind,
    ) -> Result<OcrSourceGeometry, A2dFfiError> {
        self.core
            .resolve_ocr_source_geometry(&scan_id, input_kind.into())
            .map(Into::into)
            .map_err(Into::into)
    }
}
