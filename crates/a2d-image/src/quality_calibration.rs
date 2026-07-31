use crate::{QualityAssessment, QualityState};

/// Stable warning emitted whenever measured quality is classified by thresholds that do not yet
/// have the photographed physical evidence required for a production claim.
pub const QUALITY_THRESHOLDS_UNCALIBRATED: &str = "QUALITY_THRESHOLDS_UNCALIBRATED";

/// Whether a threshold policy is supported by reviewed physical calibration evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualityCalibrationState {
    Calibrated,
    Provisional,
    Unavailable,
}

/// The evidence class behind a threshold value. Measurement algorithms are independent of this
/// classification; this enum qualifies only what claims may be made from their numeric output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualityThresholdEvidence {
    PresentationOnlyProvisional,
    SyntheticFixtureRegression,
    PhysicallyCalibratedProduction,
    Unavailable,
}

/// Versioned provenance for one threshold set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualityCalibrationMetadata {
    pub threshold_policy_version: u32,
    pub calibration_state: QualityCalibrationState,
    pub threshold_evidence: QualityThresholdEvidence,
    pub physical_calibration_version: Option<u32>,
}

impl QualityCalibrationMetadata {
    pub const fn provisional(threshold_policy_version: u32) -> Self {
        Self {
            threshold_policy_version,
            calibration_state: QualityCalibrationState::Provisional,
            threshold_evidence: QualityThresholdEvidence::SyntheticFixtureRegression,
            physical_calibration_version: None,
        }
    }

    pub const fn unavailable(threshold_policy_version: u32) -> Self {
        Self {
            threshold_policy_version,
            calibration_state: QualityCalibrationState::Unavailable,
            threshold_evidence: QualityThresholdEvidence::Unavailable,
            physical_calibration_version: None,
        }
    }

    pub const fn calibrated(
        threshold_policy_version: u32,
        physical_calibration_version: u32,
    ) -> Self {
        Self {
            threshold_policy_version,
            calibration_state: QualityCalibrationState::Calibrated,
            threshold_evidence: QualityThresholdEvidence::PhysicallyCalibratedProduction,
            physical_calibration_version: Some(physical_calibration_version),
        }
    }

    pub const fn allows_production_classification(self) -> bool {
        if self.threshold_policy_version == 0 {
            return false;
        }
        match (
            self.calibration_state,
            self.threshold_evidence,
            self.physical_calibration_version,
        ) {
            (
                QualityCalibrationState::Calibrated,
                QualityThresholdEvidence::PhysicallyCalibratedProduction,
                Some(version),
            ) => version > 0,
            _ => false,
        }
    }

    /// Automatic capture is a production action and therefore requires reviewed physical evidence.
    pub const fn allows_automatic_capture(self) -> bool {
        self.allows_production_classification()
    }

    pub const fn warning_code(self) -> Option<&'static str> {
        if self.allows_production_classification() {
            None
        } else {
            Some(QUALITY_THRESHOLDS_UNCALIBRATED)
        }
    }
}

/// A measured assessment plus an honest statement about whether its classification is suitable for
/// production. Raw measurements are retained unchanged in `provisional_assessment` for every state.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationQualifiedQualityAssessment {
    pub calibration: QualityCalibrationMetadata,
    pub provisional_assessment: QualityAssessment,
    pub production_classification: Option<QualityState>,
    pub warning_code: Option<&'static str>,
}

pub fn qualify_quality_assessment(
    provisional_assessment: QualityAssessment,
    calibration: QualityCalibrationMetadata,
) -> CalibrationQualifiedQualityAssessment {
    let production_classification = calibration
        .allows_production_classification()
        .then_some(provisional_assessment.overall);
    CalibrationQualifiedQualityAssessment {
        calibration,
        provisional_assessment,
        production_classification,
        warning_code: calibration.warning_code(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MetricState, QualityMeasurements, QualityMetricStates};

    fn measured_assessment() -> QualityAssessment {
        QualityAssessment {
            policy_version: 7,
            overall: QualityState::Accepted,
            states: QualityMetricStates {
                focus: MetricState::Accepted,
                underexposure: MetricState::Accepted,
                overexposure: MetricState::Accepted,
                glare: MetricState::Accepted,
                framing: MetricState::Unavailable,
                marker_confidence: MetricState::Accepted,
                perspective: MetricState::Unavailable,
                resolution: MetricState::Unavailable,
                curvature: MetricState::Unavailable,
            },
            measurements: QualityMeasurements::empty(),
        }
    }

    #[test]
    fn provisional_assessment_preserves_measurements_without_production_acceptance() {
        let measured = measured_assessment();
        let qualified = qualify_quality_assessment(
            measured.clone(),
            QualityCalibrationMetadata::provisional(1),
        );

        assert_eq!(qualified.provisional_assessment, measured);
        assert_eq!(qualified.production_classification, None);
        assert_eq!(
            qualified.warning_code,
            Some(QUALITY_THRESHOLDS_UNCALIBRATED)
        );
    }

    #[test]
    fn uncalibrated_policy_never_allows_automatic_capture() {
        assert!(!QualityCalibrationMetadata::provisional(1).allows_automatic_capture());
        assert!(!QualityCalibrationMetadata::unavailable(1).allows_automatic_capture());
        assert!(!QualityCalibrationMetadata::calibrated(0, 1).allows_automatic_capture());
    }

    #[test]
    fn future_calibrated_policy_is_versioned_without_changing_measurements() {
        let measured = measured_assessment();
        let qualified = qualify_quality_assessment(
            measured.clone(),
            QualityCalibrationMetadata::calibrated(2, 1),
        );

        assert!(qualified.calibration.allows_automatic_capture());
        assert_eq!(
            qualified.production_classification,
            Some(QualityState::Accepted)
        );
        assert_eq!(
            qualified.provisional_assessment.measurements,
            measured.measurements
        );
        assert_eq!(qualified.warning_code, None);
    }
}
