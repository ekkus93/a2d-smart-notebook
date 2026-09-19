use super::{A2dClient, A2dFfiError};
use crate::ocr::OcrInputKind;

#[derive(Clone, Debug, uniffi::Record)]
pub struct PageViewerAsset {
    pub asset_id: String,
    pub kind: String,
    pub media_type: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct PageViewerSnapshot {
    pub page_id: String,
    pub page_status: String,
    pub updated_at_ms: i64,
    pub selected_scan_id: Option<String>,
    pub original_asset: Option<PageViewerAsset>,
    pub corrected_asset: Option<PageViewerAsset>,
    pub display_asset: Option<PageViewerAsset>,
    pub needs_review: bool,
}

/// Authoritative OCR source identity and encoded image geometry for Page Viewer.
///
/// `relative_path` is library-relative and must be resolved beneath `A2dClient.library_path()`;
/// Android must not manufacture dimensions or infer them from OCR polygons.
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

impl From<a2d_core::OcrSourceGeometry> for OcrSourceGeometry {
    fn from(value: a2d_core::OcrSourceGeometry) -> Self {
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

impl From<a2d_core::PageViewerAsset> for PageViewerAsset {
    fn from(value: a2d_core::PageViewerAsset) -> Self {
        Self {
            asset_id: value.asset_id,
            kind: value.kind,
            media_type: value.media_type,
            byte_length: value.byte_length,
        }
    }
}

impl From<a2d_core::PageViewerSnapshot> for PageViewerSnapshot {
    fn from(value: a2d_core::PageViewerSnapshot) -> Self {
        Self {
            page_id: value.page_id,
            page_status: value.page_status,
            updated_at_ms: value.updated_at_ms,
            selected_scan_id: value.selected_scan_id,
            original_asset: value.original_asset.map(Into::into),
            corrected_asset: value.corrected_asset.map(Into::into),
            display_asset: value.display_asset.map(Into::into),
            needs_review: value.needs_review,
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn load_page_viewer_snapshot(
        &self,
        page_id: String,
        requested_scan_id: Option<String>,
    ) -> Result<PageViewerSnapshot, A2dFfiError> {
        self.core
            .load_page_viewer_snapshot(&page_id, requested_scan_id.as_deref())
            .map(Into::into)
            .map_err(Into::into)
    }

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
