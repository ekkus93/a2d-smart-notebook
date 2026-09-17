use a2d_domain::{A2dError, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, PageId, PageState, ScanId};
use a2d_storage::{AssetRepository, PageRepository, ScanRepository};

use crate::A2dCore;

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct PageViewerAsset {
    pub asset_id: String,
    pub kind: String,
    pub relative_path: String,
    pub media_type: String,
    pub byte_length: u64,
}

impl A2dCore {
    pub fn load_page_viewer_snapshot(
        &self,
        page_id: &str,
        requested_scan_id: Option<&str>,
    ) -> Result<PageViewerSnapshot, A2dError> {
        let page_id = PageId::parse(page_id)?;
        let storage = self.lock_storage()?;
        let page = storage.get_page(&page_id)?.ok_or_else(|| {
            A2dError::new(
                ErrorCode::new("CORE_PAGE_VIEWER_PAGE_MISSING"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.core.page_viewer_page_missing",
                "Page Viewer page does not exist",
                false,
            )
            .with_detail("page_id", page_id.to_string())
        })?;

        let selected_scan_id = requested_scan_id
            .map(ScanId::parse)
            .transpose()?
            .or_else(|| page.preferred_scan_id.clone());

        let scan = match selected_scan_id {
            Some(ref scan_id) => {
                let scan = storage.get_scan(scan_id)?.ok_or_else(|| {
                    A2dError::new(
                        ErrorCode::new("CORE_PAGE_VIEWER_SCAN_MISSING"),
                        ErrorCategory::Validation,
                        ErrorSeverity::Error,
                        "error.core.page_viewer_scan_missing",
                        "Page Viewer scan does not exist",
                        false,
                    )
                    .with_detail("scan_id", scan_id.to_string())
                })?;
                if scan.page_id != page_id {
                    return Err(A2dError::new(
                        ErrorCode::new("CORE_PAGE_VIEWER_SCAN_PAGE_MISMATCH"),
                        ErrorCategory::Validation,
                        ErrorSeverity::Error,
                        "error.core.page_viewer_scan_page_mismatch",
                        "Page Viewer scan does not belong to the requested page",
                        false,
                    )
                    .with_detail("page_id", page_id.to_string())
                    .with_detail("scan_id", scan_id.to_string()));
                }
                Some(scan)
            }
            None => None,
        };

        let load_asset = |id: &a2d_domain::AssetId| -> Result<PageViewerAsset, A2dError> {
            let asset = storage.get_asset(id)?.ok_or_else(|| {
                A2dError::new(
                    ErrorCode::new("CORE_PAGE_VIEWER_ASSET_MISSING"),
                    ErrorCategory::Integrity,
                    ErrorSeverity::Error,
                    "error.core.page_viewer_asset_missing",
                    "Page Viewer scan references a missing asset",
                    false,
                )
                .with_detail("asset_id", id.to_string())
            })?;
            Ok(PageViewerAsset {
                asset_id: asset.id().to_string(),
                kind: match asset.kind {
                    AssetKind::Original => "original",
                    AssetKind::Corrected => "corrected",
                    AssetKind::Ocr => "ocr",
                    AssetKind::Thumbnail => "thumbnail",
                    AssetKind::Export => "export",
                }.to_string(),
                relative_path: asset.relative_path,
                media_type: asset.media_type,
                byte_length: asset.byte_length,
            })
        };

        let (original_asset, corrected_asset) = if let Some(scan) = scan.as_ref() {
            (
                Some(load_asset(&scan.original_asset_id)?),
                scan.corrected_asset_id.as_ref().map(load_asset).transpose()?,
            )
        } else {
            (None, None)
        };
        let display_asset = corrected_asset.clone().or_else(|| original_asset.clone());

        Ok(PageViewerSnapshot {
            page_id: page_id.to_string(),
            page_status: match page.state {
                PageState::Unscanned => "unscanned",
                PageState::GeneratedNotScanned => "generated_not_scanned",
                PageState::Scanned => "scanned",
                PageState::NeedsReview => "needs_review",
                PageState::Archived => "archived",
                PageState::Trashed => "trashed",
            }.to_string(),
            updated_at_ms: page.updated_at_ms,
            selected_scan_id: scan.as_ref().map(|scan| scan.id().to_string()),
            original_asset,
            corrected_asset,
            display_asset,
            needs_review: page.state == PageState::NeedsReview
                || scan.as_ref().is_some_and(|scan| matches!(scan.quality_status, a2d_domain::QualityStatus::NeedsReview)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_page_is_rejected() {
        let dir = std::env::temp_dir().join(format!("a2d-page-viewer-{}", PageId::generate()));
        let core = A2dCore::open(crate::OpenLibraryRequest { library_path: dir.to_string_lossy().into_owned() }).unwrap();
        let err = core.load_page_viewer_snapshot(&PageId::generate().to_string(), None).unwrap_err();
        assert_eq!(err.code.to_string(), "CORE_PAGE_VIEWER_PAGE_MISSING");
        std::fs::remove_dir_all(dir).ok();
    }
}
