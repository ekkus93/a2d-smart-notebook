//! Read-only stored-scan comparison for Milestone 9.2.
//!
//! This module loads two persisted scans, validates that they target the same page, verifies each
//! corrected asset against its recorded SHA-256, checks that the versioned fingerprint names that
//! same hash, and projects image-layer measurements into a stable core evidence record. It does not
//! classify a non-exact comparison as a duplicate, revision, or substantially different page until
//! photographed-fixture calibration exists.

use a2d_domain::{
    A2dError, Asset, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, PageId, PhysicalCopyId,
    QualityStatus, Scan, ScanId,
};
use a2d_image::{
    ScanContentComparisonConfidence, ScanContentComparisonConfig, ScanContentComparisonReason,
    ScanContentFingerprintV1,
};
use a2d_storage::{AssetRepository, AssetStore, ScanRepository};

use super::A2dCore;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompareStoredScansRequest {
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub minimum_cell_absolute_difference: u8,
}

/// Confidence in exact equality only. Non-exact duplicate/revision confidence remains unavailable
/// until photographed fixtures establish reviewed production thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredScanComparisonConfidence {
    ConclusiveExactMatch,
    UnavailableUntilFixtureCalibration,
}

impl StoredScanComparisonConfidence {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ConclusiveExactMatch => "CONCLUSIVE_EXACT_MATCH",
            Self::UnavailableUntilFixtureCalibration => "UNAVAILABLE_UNTIL_FIXTURE_CALIBRATION",
        }
    }
}

impl From<ScanContentComparisonConfidence> for StoredScanComparisonConfidence {
    fn from(value: ScanContentComparisonConfidence) -> Self {
        match value {
            ScanContentComparisonConfidence::ConclusiveExactMatch => Self::ConclusiveExactMatch,
            ScanContentComparisonConfidence::UnavailableUntilFixtureCalibration => {
                Self::UnavailableUntilFixtureCalibration
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredScanComparisonReason {
    CorrectedAssetHashMatch,
    CorrectedAssetHashDiffers,
    PerceptualFingerprintMatch,
    PerceptualDifferencesBelowConfiguredThreshold,
    PerceptualChangeRegionsDetected,
    FixtureCalibrationRequired,
}

impl StoredScanComparisonReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::CorrectedAssetHashMatch => "CORRECTED_ASSET_HASH_MATCH",
            Self::CorrectedAssetHashDiffers => "CORRECTED_ASSET_HASH_DIFFERS",
            Self::PerceptualFingerprintMatch => "PERCEPTUAL_FINGERPRINT_MATCH",
            Self::PerceptualDifferencesBelowConfiguredThreshold => {
                "PERCEPTUAL_DIFFERENCES_BELOW_CONFIGURED_THRESHOLD"
            }
            Self::PerceptualChangeRegionsDetected => "PERCEPTUAL_CHANGE_REGIONS_DETECTED",
            Self::FixtureCalibrationRequired => "FIXTURE_CALIBRATION_REQUIRED",
        }
    }
}

impl From<ScanContentComparisonReason> for StoredScanComparisonReason {
    fn from(value: ScanContentComparisonReason) -> Self {
        match value {
            ScanContentComparisonReason::CorrectedAssetHashMatch => Self::CorrectedAssetHashMatch,
            ScanContentComparisonReason::CorrectedAssetHashDiffers => {
                Self::CorrectedAssetHashDiffers
            }
            ScanContentComparisonReason::PerceptualFingerprintMatch => {
                Self::PerceptualFingerprintMatch
            }
            ScanContentComparisonReason::PerceptualDifferencesBelowConfiguredThreshold => {
                Self::PerceptualDifferencesBelowConfiguredThreshold
            }
            ScanContentComparisonReason::PerceptualChangeRegionsDetected => {
                Self::PerceptualChangeRegionsDetected
            }
            ScanContentComparisonReason::FixtureCalibrationRequired => {
                Self::FixtureCalibrationRequired
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredScanChangedCell {
    pub column: u32,
    pub row: u32,
    pub absolute_difference: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredScanChangeRegion {
    pub left_column: u32,
    pub top_row: u32,
    pub right_column_exclusive: u32,
    pub bottom_row_exclusive: u32,
    pub changed_cell_count: u32,
    pub mean_absolute_difference: f32,
    pub maximum_absolute_difference: u8,
    pub cells: Vec<StoredScanChangedCell>,
}

/// Immutable evidence for a comparison between two persisted scans of one page.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredScanComparisonEvidence {
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub page_id: PageId,
    pub baseline_pipeline_version: String,
    pub candidate_pipeline_version: String,
    pub pipeline_versions_match: bool,
    pub baseline_quality_status: QualityStatus,
    pub candidate_quality_status: QualityStatus,
    pub baseline_preferred: bool,
    pub candidate_preferred: bool,
    pub baseline_physical_copy_id: Option<PhysicalCopyId>,
    pub candidate_physical_copy_id: Option<PhysicalCopyId>,
    /// `None` means at least one scan has not yet been assigned to a known physical copy.
    pub same_physical_copy: Option<bool>,
    pub minimum_cell_absolute_difference: u8,
    pub corrected_asset_hash_match: bool,
    pub exact_content_match: bool,
    pub confidence: StoredScanComparisonConfidence,
    pub reasons: Vec<StoredScanComparisonReason>,
    pub mean_absolute_difference: f32,
    pub maximum_absolute_difference: u8,
    pub changed_cell_count: u32,
    pub change_regions: Vec<StoredScanChangeRegion>,
}

impl A2dCore {
    pub fn compare_stored_scans(
        &self,
        request: CompareStoredScansRequest,
    ) -> Result<StoredScanComparisonEvidence, A2dError> {
        if request.baseline_scan_id == request.candidate_scan_id {
            return Err(comparison_error(
                "CORE_SCAN_COMPARISON_SELF_REFERENCE",
                ErrorCategory::Validation,
                "baseline and candidate scan IDs must identify two different stored scans",
            )
            .with_detail("scan_id", request.baseline_scan_id.to_string()));
        }

        let config = ScanContentComparisonConfig::new(request.minimum_cell_absolute_difference)?;
        let storage = self.lock_storage()?;
        let baseline = load_required_scan(
            &storage,
            &request.baseline_scan_id,
            "CORE_SCAN_COMPARISON_BASELINE_NOT_FOUND",
            "baseline",
        )?;
        let candidate = load_required_scan(
            &storage,
            &request.candidate_scan_id,
            "CORE_SCAN_COMPARISON_CANDIDATE_NOT_FOUND",
            "candidate",
        )?;

        if baseline.page_id != candidate.page_id {
            return Err(comparison_error(
                "CORE_SCAN_COMPARISON_PAGE_MISMATCH",
                ErrorCategory::Validation,
                "stored scan comparison requires both scans to belong to the same page",
            )
            .with_detail("baseline_scan_id", baseline.id().to_string())
            .with_detail("baseline_page_id", baseline.page_id.to_string())
            .with_detail("candidate_scan_id", candidate.id().to_string())
            .with_detail("candidate_page_id", candidate.page_id.to_string()));
        }
        validate_pipeline_version(&baseline, "baseline")?;
        validate_pipeline_version(&candidate, "candidate")?;

        let baseline_fingerprint = parse_stored_fingerprint(&baseline, "baseline")?;
        let candidate_fingerprint = parse_stored_fingerprint(&candidate, "candidate")?;
        let baseline_asset = load_required_corrected_asset(&storage, &baseline, "baseline")?;
        let candidate_asset = load_required_corrected_asset(&storage, &candidate, "candidate")?;
        drop(storage);

        verify_corrected_asset(
            &self.asset_store,
            &baseline,
            "baseline",
            &baseline_asset,
            &baseline_fingerprint,
        )?;
        verify_corrected_asset(
            &self.asset_store,
            &candidate,
            "candidate",
            &candidate_asset,
            &candidate_fingerprint,
        )?;

        let comparison = baseline_fingerprint.compare(&candidate_fingerprint, config)?;
        let same_physical_copy = match (
            baseline.physical_copy_id.as_ref(),
            candidate.physical_copy_id.as_ref(),
        ) {
            (Some(left), Some(right)) => Some(left == right),
            _ => None,
        };
        let mut change_regions = Vec::with_capacity(comparison.change_regions.regions().len());
        for region in comparison.change_regions.regions() {
            let mut cells = Vec::with_capacity(region.cells().len());
            for cell in region.cells() {
                cells.push(StoredScanChangedCell {
                    column: grid_coordinate(cell.column(), "change cell column")?,
                    row: grid_coordinate(cell.row(), "change cell row")?,
                    absolute_difference: cell.absolute_difference(),
                });
            }
            change_regions.push(StoredScanChangeRegion {
                left_column: grid_coordinate(region.left_column(), "region left column")?,
                top_row: grid_coordinate(region.top_row(), "region top row")?,
                right_column_exclusive: grid_coordinate(
                    region.right_column_exclusive(),
                    "region right column",
                )?,
                bottom_row_exclusive: grid_coordinate(
                    region.bottom_row_exclusive(),
                    "region bottom row",
                )?,
                changed_cell_count: grid_coordinate(
                    region.changed_cell_count(),
                    "region changed cell count",
                )?,
                mean_absolute_difference: region.mean_absolute_difference(),
                maximum_absolute_difference: region.maximum_absolute_difference(),
                cells,
            });
        }

        Ok(StoredScanComparisonEvidence {
            baseline_scan_id: baseline.id().clone(),
            candidate_scan_id: candidate.id().clone(),
            page_id: baseline.page_id.clone(),
            baseline_pipeline_version: baseline.pipeline_version.clone(),
            candidate_pipeline_version: candidate.pipeline_version.clone(),
            pipeline_versions_match: baseline.pipeline_version == candidate.pipeline_version,
            baseline_quality_status: baseline.quality_status,
            candidate_quality_status: candidate.quality_status,
            baseline_preferred: baseline.preferred,
            candidate_preferred: candidate.preferred,
            baseline_physical_copy_id: baseline.physical_copy_id.clone(),
            candidate_physical_copy_id: candidate.physical_copy_id.clone(),
            same_physical_copy,
            minimum_cell_absolute_difference: comparison
                .change_regions
                .minimum_cell_absolute_difference(),
            corrected_asset_hash_match: comparison.corrected_asset_hash_match,
            exact_content_match: comparison.exact_content_match,
            confidence: comparison.confidence.into(),
            reasons: comparison.reasons.into_iter().map(Into::into).collect(),
            mean_absolute_difference: comparison.mean_absolute_difference,
            maximum_absolute_difference: comparison.maximum_absolute_difference,
            changed_cell_count: grid_coordinate(
                comparison.change_regions.changed_cell_count(),
                "comparison changed cell count",
            )?,
            change_regions,
        })
    }
}

fn load_required_scan(
    storage: &a2d_storage::Storage,
    scan_id: &ScanId,
    missing_code: &'static str,
    role: &'static str,
) -> Result<Scan, A2dError> {
    storage.get_scan(scan_id)?.ok_or_else(|| {
        comparison_error(
            missing_code,
            ErrorCategory::Validation,
            format!("the requested {role} scan does not exist"),
        )
        .with_detail("scan_id", scan_id.to_string())
        .with_detail("comparison_role", role)
    })
}

fn load_required_corrected_asset(
    storage: &a2d_storage::Storage,
    scan: &Scan,
    role: &'static str,
) -> Result<Asset, A2dError> {
    let asset_id = scan.corrected_asset_id.as_ref().ok_or_else(|| {
        comparison_error(
            "CORE_SCAN_COMPARISON_CORRECTED_ASSET_MISSING",
            ErrorCategory::Integrity,
            "stored scan comparison requires a corrected asset",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("comparison_role", role)
    })?;
    let asset = storage.get_asset(asset_id)?.ok_or_else(|| {
        comparison_error(
            "CORE_SCAN_COMPARISON_CORRECTED_ASSET_ROW_MISSING",
            ErrorCategory::Integrity,
            "stored scan references a corrected asset row that does not exist",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("asset_id", asset_id.to_string())
        .with_detail("comparison_role", role)
    })?;
    if asset.kind != AssetKind::Corrected {
        return Err(comparison_error(
            "CORE_SCAN_COMPARISON_CORRECTED_ASSET_KIND_INVALID",
            ErrorCategory::Integrity,
            "stored scan corrected_asset_id does not identify a corrected asset",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("asset_id", asset.id().to_string())
        .with_detail("comparison_role", role));
    }
    Ok(asset)
}

fn verify_corrected_asset(
    asset_store: &AssetStore,
    scan: &Scan,
    role: &'static str,
    asset: &Asset,
    fingerprint: &ScanContentFingerprintV1,
) -> Result<(), A2dError> {
    if asset.sha256.as_str() != fingerprint.corrected_sha256() {
        return Err(comparison_error(
            "CORE_SCAN_COMPARISON_FINGERPRINT_ASSET_HASH_MISMATCH",
            ErrorCategory::Integrity,
            "stored content fingerprint does not name the corrected asset's recorded SHA-256",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("asset_id", asset.id().to_string())
        .with_detail("comparison_role", role)
        .with_detail("asset_sha256", &asset.sha256)
        .with_detail("fingerprint_sha256", fingerprint.corrected_sha256()));
    }
    asset_store.verify(asset).map_err(|error| {
        error
            .with_detail("scan_id", scan.id().to_string())
            .with_detail("asset_id", asset.id().to_string())
            .with_detail("comparison_role", role)
    })
}

fn validate_pipeline_version(scan: &Scan, role: &'static str) -> Result<(), A2dError> {
    if scan.pipeline_version.trim().is_empty() {
        return Err(comparison_error(
            "CORE_SCAN_COMPARISON_PIPELINE_VERSION_MISSING",
            ErrorCategory::Integrity,
            "a stored scan is missing required processing-pipeline provenance",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("comparison_role", role));
    }
    Ok(())
}

fn parse_stored_fingerprint(
    scan: &Scan,
    role: &'static str,
) -> Result<ScanContentFingerprintV1, A2dError> {
    ScanContentFingerprintV1::parse(&scan.content_fingerprint).map_err(|error| {
        error
            .with_detail("scan_id", scan.id().to_string())
            .with_detail("comparison_role", role)
            .with_detail("pipeline_version", scan.pipeline_version.clone())
    })
}

fn grid_coordinate(value: usize, context: &'static str) -> Result<u32, A2dError> {
    u32::try_from(value).map_err(|_| {
        comparison_error(
            "CORE_SCAN_COMPARISON_GRID_VALUE_OVERFLOW",
            ErrorCategory::Internal,
            "an aligned fingerprint grid value exceeded the portable result representation",
        )
        .with_detail("context", context)
        .with_detail("value", value.to_string())
    })
}

fn comparison_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    let severity = match category {
        ErrorCategory::Integrity | ErrorCategory::Internal => ErrorSeverity::Critical,
        _ => ErrorSeverity::Error,
    };
    A2dError::new(
        ErrorCode::new(code),
        category,
        severity,
        "error.core.scan_comparison",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use a2d_domain::{
        AssetKind, CaptureSource, LayoutId, Page, PageId, PageKind, PageState, QualityStatus, Scan,
        ScanId, SmartPageId,
    };
    use a2d_image::PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT;
    use a2d_storage::{AssetRepository, PageRepository, ScanRepository};

    use super::*;
    use crate::OpenLibraryRequest;

    struct InsertedScan {
        id: ScanId,
        corrected: Asset,
    }

    fn test_core() -> (Arc<A2dCore>, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("a2d-scan-comparison-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, root)
    }

    fn insert_page(core: &A2dCore) -> PageId {
        let page_id = PageId::generate();
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("USLETTER-LINED").unwrap(),
            None,
            PageState::Scanned,
            1,
        );
        core.lock_storage().unwrap().insert_page(&page).unwrap();
        page_id
    }

    fn fingerprint(corrected_sha256: &str, changes: &[(usize, u8)]) -> String {
        let mut cells = vec![180_u8; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT];
        for &(index, value) in changes {
            cells[index] = value;
        }
        let payload = cells
            .iter()
            .map(|cell| format!("{cell:02x}"))
            .collect::<String>();
        format!(
            "scan-content-v1;corrected-sha256={corrected_sha256};perceptual=mean-grid-16x24-v1:{payload}"
        )
    }

    fn insert_scan(
        core: &A2dCore,
        page_id: PageId,
        corrected_bytes: &[u8],
        changes: &[(usize, u8)],
        fingerprint_override: Option<String>,
        pipeline_version: &str,
        preferred: bool,
    ) -> InsertedScan {
        let scan_id = ScanId::generate();
        let original_bytes = format!("original-{scan_id}");
        let original = core
            .asset_store
            .commit(original_bytes.as_bytes(), AssetKind::Original, "image/jpeg")
            .unwrap();
        let corrected = core
            .asset_store
            .commit(corrected_bytes, AssetKind::Corrected, "image/png")
            .unwrap();
        let content_fingerprint =
            fingerprint_override.unwrap_or_else(|| fingerprint(&corrected.sha256, changes));
        let scan = Scan::new(
            scan_id.clone(),
            page_id,
            None,
            CaptureSource::Camera,
            1,
            original.id().clone(),
            Some(corrected.id().clone()),
            None,
            None,
            pipeline_version.to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            preferred,
            None,
            content_fingerprint,
        );
        core.lock_storage()
            .unwrap()
            .transaction(|tx| {
                tx.insert_asset(&original)?;
                tx.insert_asset(&corrected)?;
                tx.insert_scan(&scan)?;
                Ok(())
            })
            .unwrap();
        InsertedScan {
            id: scan_id,
            corrected,
        }
    }

    fn insert_scan_without_corrected(core: &A2dCore, page_id: PageId, preferred: bool) -> ScanId {
        let scan_id = ScanId::generate();
        let original = core
            .asset_store
            .commit(b"original-only", AssetKind::Original, "image/jpeg")
            .unwrap();
        let scan = Scan::new(
            scan_id.clone(),
            page_id,
            None,
            CaptureSource::Camera,
            1,
            original.id().clone(),
            None,
            None,
            None,
            "1".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            preferred,
            None,
            fingerprint(&"1".repeat(64), &[]),
        );
        core.lock_storage()
            .unwrap()
            .transaction(|tx| {
                tx.insert_asset(&original)?;
                tx.insert_scan(&scan)?;
                Ok(())
            })
            .unwrap();
        scan_id
    }

    #[test]
    fn exact_stored_content_is_conclusive_only_after_both_assets_verify() {
        let (core, root) = test_core();
        let page_id = insert_page(&core);
        let baseline = insert_scan(
            &core,
            page_id.clone(),
            b"same corrected bytes",
            &[],
            None,
            "1",
            true,
        );
        let candidate = insert_scan(
            &core,
            page_id.clone(),
            b"same corrected bytes",
            &[],
            None,
            "1",
            false,
        );

        let evidence = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap();

        assert_eq!(evidence.page_id, page_id);
        assert!(evidence.corrected_asset_hash_match);
        assert!(evidence.exact_content_match);
        assert_eq!(
            evidence.confidence,
            StoredScanComparisonConfidence::ConclusiveExactMatch
        );
        assert_eq!(
            evidence.reasons,
            vec![
                StoredScanComparisonReason::CorrectedAssetHashMatch,
                StoredScanComparisonReason::PerceptualFingerprintMatch,
            ]
        );
        assert_eq!(evidence.changed_cell_count, 0);
        assert!(evidence.change_regions.is_empty());
        assert!(evidence.pipeline_versions_match);
        assert_eq!(evidence.same_physical_copy, None);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn changed_stored_content_returns_regions_but_not_a_revision_classification() {
        let (core, root) = test_core();
        let page_id = insert_page(&core);
        let baseline = insert_scan(
            &core,
            page_id.clone(),
            b"baseline corrected bytes",
            &[],
            None,
            "1",
            true,
        );
        let candidate = insert_scan(
            &core,
            page_id,
            b"candidate corrected bytes",
            &[(17, 40)],
            None,
            "2",
            false,
        );

        let evidence = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 20,
            })
            .unwrap();

        assert!(!evidence.corrected_asset_hash_match);
        assert!(!evidence.exact_content_match);
        assert_eq!(
            evidence.confidence,
            StoredScanComparisonConfidence::UnavailableUntilFixtureCalibration
        );
        assert!(
            evidence
                .reasons
                .contains(&StoredScanComparisonReason::FixtureCalibrationRequired)
        );
        assert_eq!(evidence.minimum_cell_absolute_difference, 20);
        assert_eq!(evidence.maximum_absolute_difference, 140);
        assert_eq!(evidence.changed_cell_count, 1);
        assert_eq!(evidence.change_regions.len(), 1);
        assert_eq!(evidence.change_regions[0].cells.len(), 1);
        assert!(!evidence.pipeline_versions_match);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scans_from_different_pages_are_rejected_before_asset_comparison() {
        let (core, root) = test_core();
        let baseline = insert_scan(&core, insert_page(&core), b"baseline", &[], None, "1", true);
        let candidate = insert_scan(
            &core,
            insert_page(&core),
            b"candidate",
            &[],
            None,
            "1",
            true,
        );

        let error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(error.code.to_string(), "CORE_SCAN_COMPARISON_PAGE_MISMATCH");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_and_malformed_evidence_fail_explicitly() {
        let (core, root) = test_core();
        let page_id = insert_page(&core);
        let baseline = insert_scan(
            &core,
            page_id.clone(),
            b"baseline",
            &[],
            Some("legacy-fingerprint".to_string()),
            "1",
            true,
        );
        let missing = ScanId::generate();

        let missing_error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id.clone(),
                candidate_scan_id: missing,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(
            missing_error.code.to_string(),
            "CORE_SCAN_COMPARISON_CANDIDATE_NOT_FOUND"
        );

        let candidate = insert_scan(&core, page_id, b"candidate", &[], None, "1", false);
        let malformed_error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(
            malformed_error.code.to_string(),
            "IMAGE_SCAN_CONTENT_FINGERPRINT_VERSION_UNSUPPORTED"
        );
        assert_eq!(
            malformed_error
                .details
                .get("comparison_role")
                .map(String::as_str),
            Some("baseline")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_corrected_asset_is_rejected() {
        let (core, root) = test_core();
        let page_id = insert_page(&core);
        let baseline = insert_scan_without_corrected(&core, page_id.clone(), true);
        let candidate = insert_scan(&core, page_id, b"candidate", &[], None, "1", false);

        let error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "CORE_SCAN_COMPARISON_CORRECTED_ASSET_MISSING"
        );
        assert_eq!(
            error.details.get("comparison_role").map(String::as_str),
            Some("baseline")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fingerprint_hash_must_match_the_corrected_asset_row() {
        let (core, root) = test_core();
        let page_id = insert_page(&core);
        let baseline = insert_scan(
            &core,
            page_id.clone(),
            b"baseline",
            &[],
            Some(fingerprint(&"9".repeat(64), &[])),
            "1",
            true,
        );
        let candidate = insert_scan(&core, page_id, b"candidate", &[], None, "1", false);

        let error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "CORE_SCAN_COMPARISON_FINGERPRINT_ASSET_HASH_MISMATCH"
        );
        assert_eq!(
            error.details.get("comparison_role").map(String::as_str),
            Some("baseline")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_or_tampered_corrected_files_block_conclusive_evidence() {
        let (core, root) = test_core();
        let page_id = insert_page(&core);
        let baseline = insert_scan(&core, page_id.clone(), b"baseline", &[], None, "1", true);
        let candidate = insert_scan(&core, page_id.clone(), b"candidate", &[], None, "1", false);
        let baseline_path = core
            .asset_store
            .resolve(&baseline.corrected.relative_path)
            .unwrap();
        std::fs::remove_file(baseline_path).unwrap();

        let missing_error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: baseline.id,
                candidate_scan_id: candidate.id.clone(),
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(missing_error.code.to_string(), "STORAGE_ASSET_MISSING");
        assert_eq!(
            missing_error
                .details
                .get("comparison_role")
                .map(String::as_str),
            Some("baseline")
        );

        let replacement_baseline = insert_scan(
            &core,
            page_id,
            b"replacement baseline",
            &[],
            None,
            "1",
            false,
        );
        let candidate_path = core
            .asset_store
            .resolve(&candidate.corrected.relative_path)
            .unwrap();
        std::fs::write(candidate_path, b"tampered").unwrap();
        let tampered_error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: replacement_baseline.id,
                candidate_scan_id: candidate.id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(
            tampered_error.code.to_string(),
            "STORAGE_ASSET_HASH_MISMATCH"
        );
        assert_eq!(
            tampered_error
                .details
                .get("comparison_role")
                .map(String::as_str),
            Some("candidate")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn self_comparison_and_zero_threshold_are_rejected() {
        let (core, root) = test_core();
        let scan_id = ScanId::generate();
        let self_error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: scan_id.clone(),
                candidate_scan_id: scan_id,
                minimum_cell_absolute_difference: 1,
            })
            .unwrap_err();
        assert_eq!(
            self_error.code.to_string(),
            "CORE_SCAN_COMPARISON_SELF_REFERENCE"
        );

        let threshold_error = core
            .compare_stored_scans(CompareStoredScansRequest {
                baseline_scan_id: ScanId::generate(),
                candidate_scan_id: ScanId::generate(),
                minimum_cell_absolute_difference: 0,
            })
            .unwrap_err();
        assert_eq!(
            threshold_error.code.to_string(),
            "IMAGE_FINGERPRINT_CHANGE_THRESHOLD_INVALID"
        );
        std::fs::remove_dir_all(root).ok();
    }
}
