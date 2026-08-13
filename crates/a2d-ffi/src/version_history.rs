//! Portable UniFFI projection for Milestone 9.5 page-version history and comparison.

use a2d_core as core;
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, ScanId};

use crate::{
    A2dClient, A2dFfiError, ReviewItemRecord, StoredScanComparisonEvidence, StoredScanQualityStatus,
};

#[derive(Clone, Debug, uniffi::Record)]
pub struct GetPageVersionTimelineRequest {
    pub page_id: String,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PageVersionRecord {
    pub scan_id: String,
    pub captured_at_ms: i64,
    pub preferred: bool,
    pub physical_copy_id: Option<String>,
    pub supersedes_scan_id: Option<String>,
    pub quality_status: StoredScanQualityStatus,
    pub pipeline_version: String,
    pub decision_code: Option<String>,
    pub original_asset_path: String,
    pub corrected_asset_path: Option<String>,
    pub thumbnail_asset_path: Option<String>,
}

impl From<core::PageVersionRecord> for PageVersionRecord {
    fn from(value: core::PageVersionRecord) -> Self {
        Self {
            scan_id: value.scan_id.to_string(),
            captured_at_ms: value.captured_at_ms,
            preferred: value.preferred,
            physical_copy_id: value.physical_copy_id.map(|id| id.to_string()),
            supersedes_scan_id: value.supersedes_scan_id.map(|id| id.to_string()),
            quality_status: value.quality_status.into(),
            pipeline_version: value.pipeline_version,
            decision_code: value.decision_code,
            original_asset_path: value.original_asset_path,
            corrected_asset_path: value.corrected_asset_path,
            thumbnail_asset_path: value.thumbnail_asset_path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PageVersionTimeline {
    pub page_id: String,
    pub preferred_scan_id: Option<String>,
    pub preferred_version: Option<PageVersionRecord>,
    pub items: Vec<PageVersionRecord>,
    pub has_more: bool,
    pub next_offset: Option<u32>,
}

impl From<core::PageVersionTimeline> for PageVersionTimeline {
    fn from(value: core::PageVersionTimeline) -> Self {
        Self {
            page_id: value.page_id.to_string(),
            preferred_scan_id: value.preferred_scan_id.map(|id| id.to_string()),
            preferred_version: value.preferred_version.map(Into::into),
            items: value.items.into_iter().map(Into::into).collect(),
            has_more: value.has_more,
            next_offset: value.next_offset,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ComparePageVersionsRequest {
    pub baseline_scan_id: String,
    pub candidate_scan_id: String,
    pub minimum_cell_absolute_difference: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct PageVersionComparison {
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub evidence: StoredScanComparisonEvidence,
}

impl From<core::PageVersionComparison> for PageVersionComparison {
    fn from(value: core::PageVersionComparison) -> Self {
        Self {
            grid_columns: value.grid_columns,
            grid_rows: value.grid_rows,
            evidence: value.evidence.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct MovePageVersionToReviewRequest {
    pub page_id: String,
    pub scan_id: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PageVersionReviewResult {
    pub review_item: ReviewItemRecord,
    pub created: bool,
}

impl From<core::PageVersionReviewResult> for PageVersionReviewResult {
    fn from(value: core::PageVersionReviewResult) -> Self {
        Self {
            review_item: value.review_item.into(),
            created: value.created,
        }
    }
}

fn portable_threshold(value: u32) -> Result<u8, A2dFfiError> {
    let threshold = u8::try_from(value).map_err(|_| {
        A2dError::new(
            ErrorCode::new("FFI_PAGE_VERSION_THRESHOLD_OUT_OF_RANGE"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.ffi.page_version_threshold_out_of_range",
            "minimum_cell_absolute_difference must be representable as an unsigned byte",
            false,
        )
        .with_detail("minimum_cell_absolute_difference", value.to_string())
    })?;
    if threshold == 0 {
        return Err(A2dError::new(
            ErrorCode::new("FFI_PAGE_VERSION_THRESHOLD_ZERO"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.ffi.page_version_threshold_zero",
            "minimum_cell_absolute_difference must be at least one",
            false,
        )
        .into());
    }
    Ok(threshold)
}

#[uniffi::export]
impl A2dClient {
    pub fn get_page_version_timeline(
        &self,
        request: GetPageVersionTimelineRequest,
    ) -> Result<PageVersionTimeline, A2dFfiError> {
        self.core
            .get_page_version_timeline(core::GetPageVersionTimelineRequest {
                page_id: PageId::parse(&request.page_id)?,
                limit: request.limit,
                offset: request.offset,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn compare_page_versions(
        &self,
        request: ComparePageVersionsRequest,
    ) -> Result<PageVersionComparison, A2dFfiError> {
        self.core
            .compare_page_versions(core::ComparePageVersionsRequest {
                baseline_scan_id: ScanId::parse(&request.baseline_scan_id)?,
                candidate_scan_id: ScanId::parse(&request.candidate_scan_id)?,
                minimum_cell_absolute_difference: portable_threshold(
                    request.minimum_cell_absolute_difference,
                )?,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn move_page_version_to_review(
        &self,
        request: MovePageVersionToReviewRequest,
    ) -> Result<PageVersionReviewResult, A2dFfiError> {
        self.core
            .move_page_version_to_review(core::MovePageVersionToReviewRequest {
                page_id: PageId::parse(&request.page_id)?,
                scan_id: ScanId::parse(&request.scan_id)?,
                created_at_ms: request.created_at_ms,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_version_threshold_zero_and_overflow_fail_closed() {
        assert!(portable_threshold(0).is_err());
        assert!(portable_threshold(256).is_err());
        assert_eq!(portable_threshold(1).unwrap(), 1);
        assert_eq!(portable_threshold(255).unwrap(), 255);
    }
}
