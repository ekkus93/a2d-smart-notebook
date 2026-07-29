use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

use crate::{
    AlignedChangeRegionComparison, AlignedChangeRegionConfig, PerceptualFingerprintV1,
};

const CONTENT_FINGERPRINT_PREFIX: &str = "scan-content-v1;corrected-sha256=";
const PERCEPTUAL_FINGERPRINT_SEPARATOR: &str = ";perceptual=";
const SHA256_HEX_LENGTH: usize = 64;

/// Versioned scan-content evidence combining the corrected asset hash and perceptual signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanContentFingerprintV1 {
    corrected_sha256: String,
    perceptual: PerceptualFingerprintV1,
}

impl ScanContentFingerprintV1 {
    pub fn new(
        corrected_sha256: impl Into<String>,
        perceptual: PerceptualFingerprintV1,
    ) -> Result<Self, A2dError> {
        let corrected_sha256 = canonical_sha256(corrected_sha256.into())?;
        Ok(Self {
            corrected_sha256,
            perceptual,
        })
    }

    pub fn parse(value: &str) -> Result<Self, A2dError> {
        let body = value
            .strip_prefix(CONTENT_FINGERPRINT_PREFIX)
            .ok_or_else(|| {
                content_comparison_error(
                    "IMAGE_SCAN_CONTENT_FINGERPRINT_VERSION_UNSUPPORTED",
                    "scan content fingerprint has an unsupported version or format",
                )
            })?;
        let (corrected_sha256, perceptual) = body
            .split_once(PERCEPTUAL_FINGERPRINT_SEPARATOR)
            .ok_or_else(|| {
                content_comparison_error(
                    "IMAGE_SCAN_CONTENT_FINGERPRINT_FORMAT_INVALID",
                    "scan content fingerprint is missing its perceptual component",
                )
            })?;
        Self::new(
            corrected_sha256,
            PerceptualFingerprintV1::parse(perceptual)?,
        )
    }

    pub fn encode(&self) -> String {
        format!(
            "{CONTENT_FINGERPRINT_PREFIX}{}{PERCEPTUAL_FINGERPRINT_SEPARATOR}{}",
            self.corrected_sha256,
            self.perceptual.encode()
        )
    }

    pub fn corrected_sha256(&self) -> &str {
        &self.corrected_sha256
    }

    pub fn perceptual(&self) -> &PerceptualFingerprintV1 {
        &self.perceptual
    }

    pub fn compare(
        &self,
        other: &Self,
        config: ScanContentComparisonConfig,
    ) -> Result<ScanContentComparison, A2dError> {
        let difference = self.perceptual.difference(&other.perceptual);
        let mean_absolute_difference = difference.mean_absolute_difference;
        let maximum_absolute_difference = difference.maximum_absolute_difference;
        let change_regions = difference.aligned_change_regions(config.region_config)?;
        let corrected_asset_hash_match = self.corrected_sha256 == other.corrected_sha256;
        let perceptual_fingerprint_match = maximum_absolute_difference == 0;
        let exact_content_match = corrected_asset_hash_match && perceptual_fingerprint_match;

        let mut reasons = Vec::with_capacity(3);
        reasons.push(if corrected_asset_hash_match {
            ScanContentComparisonReason::CorrectedAssetHashMatch
        } else {
            ScanContentComparisonReason::CorrectedAssetHashDiffers
        });
        reasons.push(if perceptual_fingerprint_match {
            ScanContentComparisonReason::PerceptualFingerprintMatch
        } else if change_regions.is_empty() {
            ScanContentComparisonReason::PerceptualDifferencesBelowConfiguredThreshold
        } else {
            ScanContentComparisonReason::PerceptualChangeRegionsDetected
        });
        let confidence = if exact_content_match {
            ScanContentComparisonConfidence::ConclusiveExactMatch
        } else {
            reasons.push(ScanContentComparisonReason::FixtureCalibrationRequired);
            ScanContentComparisonConfidence::UnavailableUntilFixtureCalibration
        };

        Ok(ScanContentComparison {
            corrected_asset_hash_match,
            exact_content_match,
            confidence,
            reasons,
            mean_absolute_difference,
            maximum_absolute_difference,
            change_regions,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanContentComparisonConfig {
    region_config: AlignedChangeRegionConfig,
}

impl ScanContentComparisonConfig {
    pub fn new(minimum_cell_absolute_difference: u8) -> Result<Self, A2dError> {
        Ok(Self {
            region_config: AlignedChangeRegionConfig::new(minimum_cell_absolute_difference)?,
        })
    }

    pub fn minimum_cell_absolute_difference(self) -> u8 {
        self.region_config.minimum_cell_absolute_difference()
    }
}

/// Confidence in measured content equality, not a fabricated duplicate/revision probability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanContentComparisonConfidence {
    /// Both the corrected-asset SHA-256 and every perceptual fingerprint cell match exactly.
    ConclusiveExactMatch,
    /// Evidence is returned, but duplicate/revision classification is unavailable until production
    /// thresholds are calibrated from photographed fixtures.
    UnavailableUntilFixtureCalibration,
}

impl ScanContentComparisonConfidence {
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
pub enum ScanContentComparisonReason {
    CorrectedAssetHashMatch,
    CorrectedAssetHashDiffers,
    PerceptualFingerprintMatch,
    PerceptualDifferencesBelowConfiguredThreshold,
    PerceptualChangeRegionsDetected,
    FixtureCalibrationRequired,
}

impl ScanContentComparisonReason {
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
pub struct ScanContentComparison {
    pub corrected_asset_hash_match: bool,
    pub exact_content_match: bool,
    pub confidence: ScanContentComparisonConfidence,
    pub reasons: Vec<ScanContentComparisonReason>,
    pub mean_absolute_difference: f32,
    pub maximum_absolute_difference: u8,
    pub change_regions: AlignedChangeRegionComparison,
}

fn canonical_sha256(value: String) -> Result<String, A2dError> {
    if value.len() != SHA256_HEX_LENGTH
        || !value.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(content_comparison_error(
            "IMAGE_SCAN_CONTENT_FINGERPRINT_SHA256_INVALID",
            "corrected asset SHA-256 must contain exactly 64 hexadecimal characters",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn content_comparison_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.image.scan_content_comparison",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT;

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

    fn content_fingerprint(sha256: &str, changes: &[(usize, u8)]) -> ScanContentFingerprintV1 {
        ScanContentFingerprintV1::new(sha256, perceptual_with_changes(changes)).unwrap()
    }

    #[test]
    fn round_trip_is_versioned_strict_and_canonical() {
        let fingerprint = content_fingerprint(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            &[],
        );
        let encoded = fingerprint.encode();

        assert!(encoded.starts_with(
            "scan-content-v1;corrected-sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;perceptual=mean-grid-16x24-v1:"
        ));
        assert_eq!(ScanContentFingerprintV1::parse(&encoded).unwrap(), fingerprint);
        assert!(ScanContentFingerprintV1::parse("exact-sha256-v1:00").is_err());
        assert!(ScanContentFingerprintV1::new("not-a-sha256", perceptual_with_changes(&[])).is_err());
    }

    #[test]
    fn exact_hash_and_perceptual_match_are_conclusive() {
        let baseline = content_fingerprint(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &[],
        );
        let comparison = baseline
            .compare(&baseline, ScanContentComparisonConfig::new(1).unwrap())
            .unwrap();

        assert!(comparison.corrected_asset_hash_match);
        assert!(comparison.exact_content_match);
        assert_eq!(
            comparison.confidence,
            ScanContentComparisonConfidence::ConclusiveExactMatch
        );
        assert_eq!(
            comparison.reasons,
            vec![
                ScanContentComparisonReason::CorrectedAssetHashMatch,
                ScanContentComparisonReason::PerceptualFingerprintMatch,
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
        let comparison = baseline
            .compare(&candidate, ScanContentComparisonConfig::new(20).unwrap())
            .unwrap();

        assert!(!comparison.corrected_asset_hash_match);
        assert!(!comparison.exact_content_match);
        assert_eq!(
            comparison.confidence,
            ScanContentComparisonConfidence::UnavailableUntilFixtureCalibration
        );
        assert_eq!(comparison.maximum_absolute_difference, 140);
        assert_eq!(comparison.change_regions.changed_cell_count(), 1);
        assert_eq!(
            comparison.reasons,
            vec![
                ScanContentComparisonReason::CorrectedAssetHashDiffers,
                ScanContentComparisonReason::PerceptualChangeRegionsDetected,
                ScanContentComparisonReason::FixtureCalibrationRequired,
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
        let comparison = baseline
            .compare(&candidate, ScanContentComparisonConfig::new(10).unwrap())
            .unwrap();

        assert!(!comparison.exact_content_match);
        assert_eq!(comparison.maximum_absolute_difference, 5);
        assert!(comparison.mean_absolute_difference > 0.0);
        assert!(comparison.change_regions.is_empty());
        assert_eq!(
            comparison.reasons,
            vec![
                ScanContentComparisonReason::CorrectedAssetHashDiffers,
                ScanContentComparisonReason::PerceptualDifferencesBelowConfiguredThreshold,
                ScanContentComparisonReason::FixtureCalibrationRequired,
            ]
        );
    }
}
