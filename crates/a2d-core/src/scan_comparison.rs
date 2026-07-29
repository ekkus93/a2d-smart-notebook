use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, Scan, ScanId,
};
use a2d_image::{
    AlignedChangeRegionComparison, AlignedChangeRegionConfig, PerceptualFingerprintV1,
};
use a2d_storage::ScanRepository;

use super::A2dCore;

const CONTENT_FINGERPRINT_PREFIX: &str = "scan-content-v1;corrected-sha256=";
const PERCEPTUAL_FINGERPRINT_SEPARATOR: &str = ";perceptual=";
const SHA256_HEX_LENGTH: usize = 64;

#[derive(Clone, Debug)]
pub struct CompareExistingPageScansRequest {
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    /// Inclusive absolute-luminance difference used only to segment the canonical fingerprint grid.
    /// Production callers must supply a value calibrated from photographed fixtures.
    pub minimum_cell_absolute_difference: u8,
}

/// Confidence in the comparison result itself, not a fabricated duplicate/revision probability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanComparisonConfidence {
    /// Both the corrected-asset SHA-256 and every perceptual fingerprint cell match exactly.
    ConclusiveExactMatch,
    /// The evidence is preserved, but no production duplicate/revision classification is allowed
    /// until its thresholds are calibrated from photographed fixtures.
    UnavailableUntilFixtureCalibration,
}

impl ScanComparisonConfidence {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ConclusiveExactMatch => "CONCLUSIVE_EXACT_MATCH",
            Self::UnavailableUntilFixtureCalibration => {
                "UNAVAILABLE_UNTIL_FIXTURE_CALIBRATION"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScanComparisonReason {
    CorrectedAssetHashMatch,
    CorrectedAssetHashDiffers,
    PerceptualFingerprintMatch,
    PerceptualDifferencesBelowConfiguredThreshold,
    PerceptualChangeRegionsDetected,
    FixtureCalibrationRequired,
}

impl ScanComparisonReason {
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

#[derive(Clone, Debug, PartialEq)]
pub struct ExistingPageScanComparison {
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub page_id: PageId,
    pub exact_content_match: bool,
    pub confidence: ScanComparisonConfidence,
    pub reasons: Vec<ScanComparisonReason>,
    pub mean_absolute_difference: f32,
    pub maximum_absolute_difference: u8,
    pub change_regions: AlignedChangeRegionComparison,
}

impl A2dCore {
    /// Compares two durably stored scans of the same page without silently classifying an
    /// uncalibrated difference as a duplicate or revision.
    pub fn compare_existing_page_scans(
        &self,
        request: CompareExistingPageScansRequest,
    ) -> Result<ExistingPageScanComparison, A2dError> {
        let region_config =
            AlignedChangeRegionConfig::new(request.minimum_cell_absolute_difference)?;
        let (baseline, candidate) = {
            let storage = self.lock_storage()?;
            let baseline = load_scan(
                &storage,
                &request.baseline_scan_id,
                "baseline",
            )?;
            let candidate = load_scan(
                &storage,
                &request.candidate_scan_id,
                "candidate",
            )?;
            (baseline, candidate)
        };

        if baseline.page_id != candidate.page_id {
            return Err(comparison_error(
                "CORE_SCAN_COMPARISON_PAGE_MISMATCH",
                ErrorCategory::Validation,
                "stored scans must belong to the same page before they can be compared",
            )
            .with_detail("baseline_scan_id", baseline.id().to_string())
            .with_detail("baseline_page_id", baseline.page_id.to_string())
            .with_detail("candidate_scan_id", candidate.id().to_string())
            .with_detail("candidate_page_id", candidate.page_id.to_string()));
        }

        let baseline_fingerprint =
            ScanContentFingerprintV1::parse(&baseline.content_fingerprint).map_err(|error| {
                error
                    .with_detail("scan_role", "baseline")
                    .with_detail("scan_id", baseline.id().to_string())
            })?;
        let candidate_fingerprint =
            ScanContentFingerprintV1::parse(&candidate.content_fingerprint).map_err(|error| {
                error
                    .with_detail("scan_role", "candidate")
                    .with_detail("scan_id", candidate.id().to_string())
            })?;
        let difference = baseline_fingerprint
            .perceptual()
            .difference(candidate_fingerprint.perceptual());
        let mean_absolute_difference = difference.mean_absolute_difference;
        let maximum_absolute_difference = difference.maximum_absolute_difference;
        let change_regions = difference.aligned_change_regions(region_config)?;
        let corrected_asset_hash_match = baseline_fingerprint.corrected_sha256()
            == candidate_fingerprint.corrected_sha256();
        let perceptual_fingerprint_match = maximum_absolute_difference == 0;
        let exact_content_match = corrected_asset_hash_match && perceptual_fingerprint_match;

        let mut reasons = Vec::with_capacity(3);
        reasons.push(if corrected_asset_hash_match {
            ScanComparisonReason::CorrectedAssetHashMatch
        } else {
            ScanComparisonReason::CorrectedAssetHashDiffers
        });
        reasons.push(if perceptual_fingerprint_match {
            ScanComparisonReason::PerceptualFingerprintMatch
        } else if change_regions.is_empty() {
            ScanComparisonReason::PerceptualDifferencesBelowConfiguredThreshold
        } else {
            ScanComparisonReason::PerceptualChangeRegionsDetected
        });
        let confidence = if exact_content_match {
            ScanComparisonConfidence::ConclusiveExactMatch
        } else {
            reasons.push(ScanComparisonReason::FixtureCalibrationRequired);
            ScanComparisonConfidence::UnavailableUntilFixtureCalibration
        };

        Ok(ExistingPageScanComparison {
            baseline_scan_id: request.baseline_scan_id,
            candidate_scan_id: request.candidate_scan_id,
            page_id: baseline.page_id,
            exact_content_match,
            confidence,
            reasons,
            mean_absolute_difference,
            maximum_absolute_difference,
            change_regions,
        })
    }
}

fn load_scan(
    storage: &a2d_storage::Storage,
    scan_id: &ScanId,
    role: &'static str,
) -> Result<Scan, A2dError> {
    storage.get_scan(scan_id)?.ok_or_else(|| {
        comparison_error(
            "CORE_SCAN_COMPARISON_SCAN_NOT_FOUND",
            ErrorCategory::NotFound,
            "a requested scan does not exist in the local library",
        )
        .with_detail("scan_role", role)
        .with_detail("scan_id", scan_id.to_string())
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScanContentFingerprintV1 {
    corrected_sha256: String,
    perceptual: PerceptualFingerprintV1,
}

impl ScanContentFingerprintV1 {
    pub(crate) fn new(
        corrected_sha256: impl Into<String>,
        perceptual: PerceptualFingerprintV1,
    ) -> Result<Self, A2dError> {
        let corrected_sha256 = canonical_sha256(corrected_sha256.into())?;
        Ok(Self {
            corrected_sha256,
            perceptual,
        })
    }

    fn parse(value: &str) -> Result<Self, A2dError> {
        let body = value
            .strip_prefix(CONTENT_FINGERPRINT_PREFIX)
            .ok_or_else(|| {
                comparison_error(
                    "CORE_SCAN_CONTENT_FINGERPRINT_VERSION_UNSUPPORTED",
                    ErrorCategory::Validation,
                    "scan content fingerprint has an unsupported version or format",
                )
            })?;
        let (corrected_sha256, perceptual) = body
            .split_once(PERCEPTUAL_FINGERPRINT_SEPARATOR)
            .ok_or_else(|| {
                comparison_error(
                    "CORE_SCAN_CONTENT_FINGERPRINT_FORMAT_INVALID",
                    ErrorCategory::Validation,
                    "scan content fingerprint is missing its perceptual component",
                )
            })?;
        Self::new(
            corrected_sha256,
            PerceptualFingerprintV1::parse(perceptual)?,
        )
    }

    pub(crate) fn encode(&self) -> String {
        format!(
            "{CONTENT_FINGERPRINT_PREFIX}{}{PERCEPTUAL_FINGERPRINT_SEPARATOR}{}",
            self.corrected_sha256,
            self.perceptual.encode()
        )
    }

    fn corrected_sha256(&self) -> &str {
        &self.corrected_sha256
    }

    fn perceptual(&self) -> &PerceptualFingerprintV1 {
        &self.perceptual
    }
}

fn canonical_sha256(value: String) -> Result<String, A2dError> {
    if value.len() != SHA256_HEX_LENGTH
        || !value.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(comparison_error(
            "CORE_SCAN_CONTENT_FINGERPRINT_SHA256_INVALID",
            ErrorCategory::Validation,
            "corrected asset SHA-256 must contain exactly 64 hexadecimal characters",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn comparison_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        ErrorSeverity::Error,
        "error.core.scan_comparison",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2d_image::PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT;

    fn perceptual_with_changes(changes: &[(usize, u8)]) -> PerceptualFingerprintV1 {
        let mut cells = vec![180_u8; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT];
        for &(index, value) in changes {
            cells[index] = value;
        }
        let payload = cells
            .iter()
            .map(|cell| format!("{cell:02x}"))
            .collect::<String>();
        PerceptualFingerprintV1::parse(&format!("mean-grid-16x24-v1:{payload}")).unwrap()
    }

    fn content_fingerprint(sha256: &str, changes: &[(usize, u8)]) -> String {
        ScanContentFingerprintV1::new(sha256, perceptual_with_changes(changes))
            .unwrap()
            .encode()
    }

    fn compare_encoded(
        baseline: &str,
        candidate: &str,
        minimum_cell_absolute_difference: u8,
    ) -> ExistingPageScanComparison {
        let baseline_fingerprint = ScanContentFingerprintV1::parse(baseline).unwrap();
        let candidate_fingerprint = ScanContentFingerprintV1::parse(candidate).unwrap();
        let difference = baseline_fingerprint
            .perceptual()
            .difference(candidate_fingerprint.perceptual());
        let mean_absolute_difference = difference.mean_absolute_difference;
        let maximum_absolute_difference = difference.maximum_absolute_difference;
        let change_regions = difference
            .aligned_change_regions(
                AlignedChangeRegionConfig::new(minimum_cell_absolute_difference).unwrap(),
            )
            .unwrap();
        let corrected_asset_hash_match = baseline_fingerprint.corrected_sha256()
            == candidate_fingerprint.corrected_sha256();
        let perceptual_fingerprint_match = maximum_absolute_difference == 0;
        let exact_content_match = corrected_asset_hash_match && perceptual_fingerprint_match;
        let mut reasons = vec![if corrected_asset_hash_match {
            ScanComparisonReason::CorrectedAssetHashMatch
        } else {
            ScanComparisonReason::CorrectedAssetHashDiffers
        }];
        reasons.push(if perceptual_fingerprint_match {
            ScanComparisonReason::PerceptualFingerprintMatch
        } else if change_regions.is_empty() {
            ScanComparisonReason::PerceptualDifferencesBelowConfiguredThreshold
        } else {
            ScanComparisonReason::PerceptualChangeRegionsDetected
        });
        let confidence = if exact_content_match {
            ScanComparisonConfidence::ConclusiveExactMatch
        } else {
            reasons.push(ScanComparisonReason::FixtureCalibrationRequired);
            ScanComparisonConfidence::UnavailableUntilFixtureCalibration
        };
        ExistingPageScanComparison {
            baseline_scan_id: ScanId::generate(),
            candidate_scan_id: ScanId::generate(),
            page_id: PageId::generate(),
            exact_content_match,
            confidence,
            reasons,
            mean_absolute_difference,
            maximum_absolute_difference,
            change_regions,
        }
    }

    #[test]
    fn content_fingerprint_round_trip_is_versioned_strict_and_canonical() {
        let encoded = content_fingerprint(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            &[],
        );
        assert!(encoded.starts_with(
            "scan-content-v1;corrected-sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;perceptual=mean-grid-16x24-v1:"
        ));
        assert_eq!(ScanContentFingerprintV1::parse(&encoded).unwrap().encode(), encoded);
        assert!(ScanContentFingerprintV1::parse("exact-sha256-v1:00").is_err());
        assert!(ScanContentFingerprintV1::new("not-a-sha256", perceptual_with_changes(&[])).is_err());
    }

    #[test]
    fn exact_hash_and_perceptual_match_are_conclusive() {
        let baseline = content_fingerprint(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &[],
        );
        let comparison = compare_encoded(&baseline, &baseline, 1);

        assert!(comparison.exact_content_match);
        assert_eq!(
            comparison.confidence,
            ScanComparisonConfidence::ConclusiveExactMatch
        );
        assert_eq!(
            comparison.reasons,
            vec![
                ScanComparisonReason::CorrectedAssetHashMatch,
                ScanComparisonReason::PerceptualFingerprintMatch,
            ]
        );
        assert!(comparison.change_regions.is_empty());
    }

    #[test]
    fn changed_content_reports_regions_but_remains_uncalibrated() {
        let baseline = content_fingerprint(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &[],
        );
        let candidate = content_fingerprint(
            "2222222222222222222222222222222222222222222222222222222222222222",
            &[(17, 40)],
        );
        let comparison = compare_encoded(&baseline, &candidate, 20);

        assert!(!comparison.exact_content_match);
        assert_eq!(
            comparison.confidence,
            ScanComparisonConfidence::UnavailableUntilFixtureCalibration
        );
        assert_eq!(comparison.maximum_absolute_difference, 140);
        assert_eq!(comparison.change_regions.changed_cell_count(), 1);
        assert_eq!(
            comparison.reasons,
            vec![
                ScanComparisonReason::CorrectedAssetHashDiffers,
                ScanComparisonReason::PerceptualChangeRegionsDetected,
                ScanComparisonReason::FixtureCalibrationRequired,
            ]
        );
    }

    #[test]
    fn subthreshold_differences_are_not_silently_called_equal() {
        let baseline = content_fingerprint(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &[],
        );
        let candidate = content_fingerprint(
            "2222222222222222222222222222222222222222222222222222222222222222",
            &[(17, 175)],
        );
        let comparison = compare_encoded(&baseline, &candidate, 10);

        assert!(!comparison.exact_content_match);
        assert_eq!(comparison.maximum_absolute_difference, 5);
        assert!(comparison.mean_absolute_difference > 0.0);
        assert!(comparison.change_regions.is_empty());
        assert_eq!(
            comparison.reasons,
            vec![
                ScanComparisonReason::CorrectedAssetHashDiffers,
                ScanComparisonReason::PerceptualDifferencesBelowConfiguredThreshold,
                ScanComparisonReason::FixtureCalibrationRequired,
            ]
        );
    }
}
