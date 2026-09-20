use a2d_core as core;

use super::{A2dClient, A2dFfiError, OcrTextPoint};

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct LoadOcrRegionPageRequest {
    pub scan_id: String,
    pub offset: u32,
    pub page_size: u32,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct OcrRegionPageItem {
    pub text_region_id: String,
    pub ocr_run_id: String,
    pub polygon: Vec<OcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: i64,
}

impl From<core::OcrRegionPageItem> for OcrRegionPageItem {
    fn from(value: core::OcrRegionPageItem) -> Self {
        Self {
            text_region_id: value.text_region_id,
            ocr_run_id: value.ocr_run_id,
            polygon: value
                .polygon
                .into_iter()
                .map(|point| OcrTextPoint { x: point.x, y: point.y })
                .collect(),
            text: value.text,
            confidence: value.confidence,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct OcrRegionPage {
    pub scan_id: String,
    pub ocr_run_id: Option<String>,
    pub total_count: u32,
    pub returned_count: u32,
    pub offset: u32,
    pub next_offset: Option<u32>,
    pub has_more: bool,
    pub complete: bool,
    pub regions: Vec<OcrRegionPageItem>,
}

impl From<core::OcrRegionPage> for OcrRegionPage {
    fn from(value: core::OcrRegionPage) -> Self {
        Self {
            scan_id: value.scan_id,
            ocr_run_id: value.ocr_run_id,
            total_count: value.total_count,
            returned_count: value.returned_count,
            offset: value.offset,
            next_offset: value.next_offset,
            has_more: value.has_more,
            complete: value.complete,
            regions: value.regions.into_iter().map(Into::into).collect(),
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn load_ocr_region_page(
        &self,
        request: LoadOcrRegionPageRequest,
    ) -> Result<OcrRegionPage, A2dFfiError> {
        self.core
            .load_ocr_region_page(core::LoadOcrRegionPageRequest {
                scan_id: request.scan_id,
                offset: request.offset,
                page_size: request.page_size,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}
