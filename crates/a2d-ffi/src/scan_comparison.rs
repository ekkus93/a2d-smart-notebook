//! Typed UniFFI projection for read-only stored-scan comparison.
//!
//! Identifier parsing and portable integer conversion happen at this boundary. Stored scan lookup,
//! same-page validation, fingerprint parsing, aligned comparison, and calibration policy remain in
//! the shared Rust core.

use a2d_core as core;
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, QualityStatus, ScanId};

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Debug, uniffi::Record)]
pub struct CompareStoredScansRequest {
    pub baseline_scan_id: String,
    pub candidate_scan_id: String,
    /// Explicit aligned-cell segmentation threshold. Valid values are 1 through 255.
    pub minimum_cell_absolute_difference: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum StoredScanComparisonConfidence {
    ConclusiveExactMatch,
    UnavailableUntilFixtureCalibration,
}

impl From<core::StoredScanComparisonConfidence> for StoredScanComparisonConfidence {
    fn from(value: core::StoredScanComparisonConfidence) -> Self {
        match value {
            core::StoredScanComparisonConfidence::ConclusiveExactMatch => {
                Self::ConclusiveExactMatch
            }
            core::StoredScanComparisonConfidence::UnavailableUntilFixtureCalibration => {
                Self::UnavailableUntilFixtureCalibration
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum StoredScanComparisonReason {
    CorrectedAssetHashMatch,
    CorrectedAssetHashDiffers,
    PerceptualFingerprintMatch,
    PerceptualDifferencesBelowConfiguredThreshold,
    PerceptualChangeRegionsDetected,
    FixtureCalibrationRequired,
}

impl From<core::StoredScanComparisonReason> for StoredScanComparisonReason {
    fn from(value: core::StoredScanComparisonReason) -> Self {
        match value {
            core::StoredScanComparisonReason::CorrectedAssetHashMatch => {
                Self::CorrectedAssetHashMatch
            }
            core::StoredScanComparisonReason::CorrectedAssetHashDiffers => {
                Self::CorrectedAssetHashDiffers
            }
            core::StoredScanComparisonReason::PerceptualFingerprintMatch => {
                Self::PerceptualFingerprintMatch
            }
            core::StoredScanComparisonReason::PerceptualDifferencesBelowConfiguredThreshold => {
                Self::PerceptualDifferencesBelowConfiguredThreshold
            }
            core::StoredScanComparisonReason::PerceptualChangeRegionsDetected => {
                Self::PerceptualChangeRegionsDetected
            }
            core::StoredScanComparisonReason::FixtureCalibrationRequired => {
                Self::FixtureCalibrationRequired
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum StoredScanQualityStatus {
    Accepted,
    AcceptedWithWarnings,
    NeedsReview,
    Rejected,
}

impl From<QualityStatus> for StoredScanQualityStatus {
    fn from(value: QualityStatus) -> Self {
        match value {
            QualityStatus::Accepted => Self::Accepted,
            QualityStatus::AcceptedWithWarnings => Self::AcceptedWithWarnings,
            QualityStatus::NeedsReview => Self::NeedsReview,
            QualityStatus::Rejected => Self::Rejected,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct StoredScanChangedCell {
    pub column: u32,
    pub row: u32,
    pub absolute_difference: u32,
}

impl From<core::StoredScanChangedCell> for StoredScanChangedCell {
    fn from(value: core::StoredScanChangedCell) -> Self {
        Self {
            column: value.column,
            row: value.row,
            absolute_difference: u32::from(value.absolute_difference),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct StoredScanChangeRegion {
    pub left_column: u32,
    pub top_row: u32,
    pub right_column_exclusive: u32,
    pub bottom_row_exclusive: u32,
    pub changed_cell_count: u32,
    pub mean_absolute_difference: f64,
    pub maximum_absolute_difference: u32,
    pub cells: Vec<StoredScanChangedCell>,
}

impl From<core::StoredScanChangeRegion> for StoredScanChangeRegion {
    fn from(value: core::StoredScanChangeRegion) -> Self {
        Self {
            left_column: value.left_column,
            top_row: value.top_row,
            right_column_exclusive: value.right_column_exclusive,
            bottom_row_exclusive: value.bottom_row_exclusive,
            changed_cell_count: value.changed_cell_count,
            mean_absolute_difference: f64::from(value.mean_absolute_difference),
            maximum_absolute_difference: u32::from(value.maximum_absolute_difference),
            cells: value.cells.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct StoredScanComparisonEvidence {
    pub baseline_scan_id: String,
    pub candidate_scan_id: String,
    pub page_id: String,
    pub baseline_pipeline_version: String,
    pub candidate_pipeline_version: String,
    pub pipeline_versions_match: bool,
    pub baseline_quality_status: StoredScanQualityStatus,
    pub candidate_quality_status: StoredScanQualityStatus,
    pub baseline_preferred: bool,
    pub candidate_preferred: bool,
    pub baseline_physical_copy_id: Option<String>,
    pub candidate_physical_copy_id: Option<String>,
    /// `None` means at least one scan has no known physical-copy assignment.
    pub same_physical_copy: Option<bool>,
    pub minimum_cell_absolute_difference: u32,
    pub corrected_asset_hash_match: bool,
    pub exact_content_match: bool,
    pub confidence: StoredScanComparisonConfidence,
    pub reasons: Vec<StoredScanComparisonReason>,
    pub mean_absolute_difference: f64,
    pub maximum_absolute_difference: u32,
    pub changed_cell_count: u32,
    pub change_regions: Vec<StoredScanChangeRegion>,
}

impl From<core::StoredScanComparisonEvidence> for StoredScanComparisonEvidence {
    fn from(value: core::StoredScanComparisonEvidence) -> Self {
        Self {
            baseline_scan_id: value.baseline_scan_id.to_string(),
            candidate_scan_id: value.candidate_scan_id.to_string(),
            page_id: value.page_id.to_string(),
            baseline_pipeline_version: value.baseline_pipeline_version,
            candidate_pipeline_version: value.candidate_pipeline_version,
            pipeline_versions_match: value.pipeline_versions_match,
            baseline_quality_status: value.baseline_quality_status.into(),
            candidate_quality_status: value.candidate_quality_status.into(),
            baseline_preferred: value.baseline_preferred,
            candidate_preferred: value.candidate_preferred,
            baseline_physical_copy_id: value.baseline_physical_copy_id.map(|id| id.to_string()),
            candidate_physical_copy_id: value.candidate_physical_copy_id.map(|id| id.to_string()),
            same_physical_copy: value.same_physical_copy,
            minimum_cell_absolute_difference: u32::from(value.minimum_cell_absolute_difference),
            corrected_asset_hash_match: value.corrected_asset_hash_match,
            exact_content_match: value.exact_content_match,
            confidence: value.confidence.into(),
            reasons: value.reasons.into_iter().map(Into::into).collect(),
            mean_absolute_difference: f64::from(value.mean_absolute_difference),
            maximum_absolute_difference: u32::from(value.maximum_absolute_difference),
            changed_cell_count: value.changed_cell_count,
            change_regions: value.change_regions.into_iter().map(Into::into).collect(),
        }
    }
}

fn portable_threshold(value: u32) -> Result<u8, A2dFfiError> {
    u8::try_from(value).map_err(|_| {
        A2dError::new(
            ErrorCode::new("FFI_SCAN_COMPARISON_THRESHOLD_OUT_OF_RANGE"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.ffi.scan_comparison_threshold_out_of_range",
            "minimum_cell_absolute_difference must be representable as an unsigned byte",
            false,
        )
        .with_detail("minimum_cell_absolute_difference", value.to_string())
        .into()
    })
}

#[uniffi::export]
impl A2dClient {
    pub fn compare_stored_scans(
        &self,
        request: CompareStoredScansRequest,
    ) -> Result<StoredScanComparisonEvidence, A2dFfiError> {
        let baseline_scan_id = ScanId::parse(&request.baseline_scan_id)?;
        let candidate_scan_id = ScanId::parse(&request.candidate_scan_id)?;
        let minimum_cell_absolute_difference =
            portable_threshold(request.minimum_cell_absolute_difference)?;

        self.core
            .compare_stored_scans(core::CompareStoredScansRequest {
                baseline_scan_id,
                candidate_scan_id,
                minimum_cell_absolute_difference,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use a2d_domain::{PageId, PhysicalCopyId};

    use super::*;
    use crate::OpenLibraryRequest;

    #[test]
    fn projection_preserves_all_comparison_evidence() {
        let baseline_scan_id = ScanId::generate();
        let candidate_scan_id = ScanId::generate();
        let page_id = PageId::generate();
        let physical_copy_id = PhysicalCopyId::generate();
        let projected: StoredScanComparisonEvidence = core::StoredScanComparisonEvidence {
            baseline_scan_id: baseline_scan_id.clone(),
            candidate_scan_id: candidate_scan_id.clone(),
            page_id: page_id.clone(),
            baseline_pipeline_version: "1".to_string(),
            candidate_pipeline_version: "2".to_string(),
            pipeline_versions_match: false,
            baseline_quality_status: QualityStatus::Accepted,
            candidate_quality_status: QualityStatus::NeedsReview,
            baseline_preferred: true,
            candidate_preferred: false,
            baseline_physical_copy_id: Some(physical_copy_id.clone()),
            candidate_physical_copy_id: Some(physical_copy_id.clone()),
            same_physical_copy: Some(true),
            minimum_cell_absolute_difference: 20,
            corrected_asset_hash_match: false,
            exact_content_match: false,
            confidence: core::StoredScanComparisonConfidence::UnavailableUntilFixtureCalibration,
            reasons: vec![
                core::StoredScanComparisonReason::CorrectedAssetHashDiffers,
                core::StoredScanComparisonReason::PerceptualChangeRegionsDetected,
                core::StoredScanComparisonReason::FixtureCalibrationRequired,
            ],
            mean_absolute_difference: 1.25,
            maximum_absolute_difference: 140,
            changed_cell_count: 1,
            change_regions: vec![core::StoredScanChangeRegion {
                left_column: 1,
                top_row: 2,
                right_column_exclusive: 2,
                bottom_row_exclusive: 3,
                changed_cell_count: 1,
                mean_absolute_difference: 140.0,
                maximum_absolute_difference: 140,
                cells: vec![core::StoredScanChangedCell {
                    column: 1,
                    row: 2,
                    absolute_difference: 140,
                }],
            }],
        }
        .into();

        assert_eq!(projected.baseline_scan_id, baseline_scan_id.to_string());
        assert_eq!(projected.candidate_scan_id, candidate_scan_id.to_string());
        assert_eq!(projected.page_id, page_id.to_string());
        assert_eq!(
            projected.baseline_physical_copy_id,
            Some(physical_copy_id.to_string())
        );
        assert_eq!(projected.same_physical_copy, Some(true));
        assert_eq!(projected.minimum_cell_absolute_difference, 20);
        assert_eq!(
            projected.confidence,
            StoredScanComparisonConfidence::UnavailableUntilFixtureCalibration
        );
        assert_eq!(
            projected.reasons,
            vec![
                StoredScanComparisonReason::CorrectedAssetHashDiffers,
                StoredScanComparisonReason::PerceptualChangeRegionsDetected,
                StoredScanComparisonReason::FixtureCalibrationRequired,
            ]
        );
        assert_eq!(
            projected.change_regions[0].cells[0].absolute_difference,
            140
        );
    }

    #[test]
    fn malformed_ids_are_rejected_before_storage_lookup() {
        let root = std::env::temp_dir().join(format!(
            "a2d-ffi-scan-comparison-test-{}",
            PageId::generate()
        ));
        let client = A2dClient::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let error = client
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: "not-a-scan-id".to_string(),
                candidate_scan_id: ScanId::generate().to_string(),
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = error;
        assert!(details.code.contains("SCAN_ID"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn thresholds_above_byte_range_are_rejected_without_truncation() {
        let root = std::env::temp_dir().join(format!(
            "a2d-ffi-scan-comparison-test-{}",
            PageId::generate()
        ));
        let client = A2dClient::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let error = client
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: ScanId::generate().to_string(),
                candidate_scan_id: ScanId::generate().to_string(),
                minimum_cell_absolute_difference: 256,
            })
            .unwrap_err();
        let A2dFfiError::Failed(details) = error;
        assert_eq!(details.code, "FFI_SCAN_COMPARISON_THRESHOLD_OUT_OF_RANGE");
        std::fs::remove_dir_all(root).ok();
    }
}
