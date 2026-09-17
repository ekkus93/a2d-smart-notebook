use super::{A2dClient, A2dFfiError};

#[derive(Clone, Debug, uniffi::Record)]
pub struct PageViewerAsset {
    pub asset_id: String,
    pub kind: String,
    pub relative_path: String,
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

impl From<a2d_core::PageViewerAsset> for PageViewerAsset {
    fn from(value: a2d_core::PageViewerAsset) -> Self {
        Self {
            asset_id: value.asset_id,
            kind: value.kind,
            relative_path: value.relative_path,
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
}
