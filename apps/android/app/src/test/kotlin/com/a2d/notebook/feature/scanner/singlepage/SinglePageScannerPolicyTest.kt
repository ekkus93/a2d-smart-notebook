package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.capture.AutoCapturePolicy
import com.a2d.notebook.feature.scanner.presentation.LiveScannerGuidancePolicy
import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.AnalyzedPagePoint
import com.a2d.notebook.rustbridge.AnalyzedPageQuality
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class SinglePageScannerPolicyTest {
    private val thresholds = SinglePageScannerPolicies.V1.captureThresholds

    @Test
    fun completeHighQualityAnalysisIsAcceptedOnlyByProvisionalCaptureAssessment() {
        val assessment = assessCapturePolicy(goodAnalysis(), thresholds)
        assertTrue(assessment.accepted)
        assertTrue(assessment.warnings.isEmpty())
        assertFalse(SinglePageScannerPolicies.V1.qualityCalibration.allowsProductionClassification)
        assertEquals(
            QUALITY_THRESHOLDS_UNCALIBRATED,
            SinglePageScannerPolicies.V1.qualityWarningCode,
        )
    }

    @Test
    fun everyCaptureFailureRemainsVisible() {
        val bad =
            goodAnalysis().copy(
                markers = goodAnalysis().markers.dropLast(1).map { it.copy(decisionMargin = 5.0) },
                unexpectedTagIds = listOf(99),
                quality =
                    goodAnalysis().quality.copy(
                        focusLaplacianVariance = 1.0,
                        meanLuminance = 10.0,
                        darkFraction = 0.9,
                        highlightFraction = 0.8,
                        maxTileHighlightFraction = 0.9,
                    ),
            )
        val assessment = assessCapturePolicy(bad, thresholds)

        assertFalse(assessment.accepted)
        assertEquals(
            setOf(
                CapturePolicyWarning.MISSING_MARKERS,
                CapturePolicyWarning.UNEXPECTED_MARKERS,
                CapturePolicyWarning.LOW_MARKER_CONFIDENCE,
                CapturePolicyWarning.LOW_FOCUS,
                CapturePolicyWarning.TOO_DARK,
                CapturePolicyWarning.TOO_MUCH_DARK_AREA,
                CapturePolicyWarning.TOO_MUCH_HIGHLIGHT,
                CapturePolicyWarning.LOCALIZED_GLARE,
            ),
            assessment.warnings,
        )
    }

    @Test
    fun productionAutoCaptureIsExplicitlyDisabledPendingPhysicalCalibration() {
        val policy = SinglePageScannerPolicies.V1

        assertEquals(QualityCalibrationState.PROVISIONAL, policy.qualityCalibration.state)
        assertEquals(
            QualityThresholdEvidence.SYNTHETIC_FIXTURE_REGRESSION,
            policy.qualityCalibration.evidence,
        )
        assertFalse(policy.qualityCalibration.allowsAutomaticCapture)
        assertFalse(policy.autoCaptureEnabled)
    }

    @Test
    fun policyRejectsAutomaticCaptureWithUncalibratedThresholds() {
        assertThrows(IllegalArgumentException::class.java) {
            scannerPolicy(
                autoCaptureEnabled = true,
                calibration =
                    ScannerQualityCalibration(
                        thresholdPolicyVersion = 1,
                        state = QualityCalibrationState.PROVISIONAL,
                        evidence = QualityThresholdEvidence.SYNTHETIC_FIXTURE_REGRESSION,
                    ),
            )
        }
    }

    @Test
    fun futureCalibratedPolicyCanEnableAutomaticCaptureWithVersionedEvidence() {
        val policy =
            scannerPolicy(
                autoCaptureEnabled = true,
                calibration =
                    ScannerQualityCalibration(
                        thresholdPolicyVersion = 2,
                        state = QualityCalibrationState.CALIBRATED,
                        evidence = QualityThresholdEvidence.PHYSICALLY_CALIBRATED_PRODUCTION,
                        physicalCalibrationVersion = 1,
                    ),
            )

        assertTrue(policy.qualityCalibration.allowsProductionClassification)
        assertTrue(policy.qualityCalibration.allowsAutomaticCapture)
        assertTrue(policy.autoCaptureEnabled)
        assertNull(policy.qualityWarningCode)
    }

    private fun scannerPolicy(
        autoCaptureEnabled: Boolean,
        calibration: ScannerQualityCalibration,
    ): SinglePageScannerPolicy =
        SinglePageScannerPolicy(
            version = 99,
            guidance =
                LiveScannerGuidancePolicy(
                    minimumMarkerDecisionMargin = 20.0,
                    minimumFocusLaplacianVariance = 40.0,
                    minimumMeanLuminance = 50.0,
                    maximumHighlightFraction = 0.15,
                    maximumTileHighlightFraction = 0.35,
                    minimumPageCoverageFraction = 0.20,
                    maximumPageCoverageFraction = 0.85,
                    minimumEdgeMarginFraction = 0.03,
                ),
            captureThresholds = thresholds,
            autoCapture =
                AutoCapturePolicy(
                    stableIntervalNanos = 1_000_000_000,
                    maximumInterFrameGapNanos = 500_000_000,
                    repeatDebounceNanos = 5_000_000_000,
                ),
            autoCaptureEnabled = autoCaptureEnabled,
            qualityCalibration = calibration,
            pageCodeFreshnessNanos = 1_000_000_000,
        )

    private fun goodAnalysis(): EncodedPageAnalysisResult =
        EncodedPageAnalysisResult(
            width = 1_000,
            height = 1_000,
            sourceRotationDegrees = 0,
            resolvedOrientationDegrees = 0,
            markers =
                listOf("TL", "TR", "BR", "BL").mapIndexed { index, role ->
                    AnalyzedPageMarker(
                        role = role,
                        family = "tagStandard41h12",
                        id = index.toLong(),
                        hammingErrors = 0,
                        decisionMargin = 50.0,
                        center = AnalyzedPagePoint(100.0 + index, 100.0 + index),
                        corners =
                            listOf(
                                AnalyzedPagePoint(90.0, 90.0),
                                AnalyzedPagePoint(110.0, 90.0),
                                AnalyzedPagePoint(110.0, 110.0),
                                AnalyzedPagePoint(90.0, 110.0),
                            ),
                    )
                },
            unexpectedTagIds = emptyList(),
            quality =
                AnalyzedPageQuality(
                    focusLaplacianVariance = 100.0,
                    focusInteriorSampleCount = 100u,
                    meanLuminance = 120.0,
                    luminanceStandardDeviation = 30.0,
                    darkFraction = 0.01,
                    highlightFraction = 0.01,
                    maxTileHighlightFraction = 0.02,
                    populatedTileCount = 64,
                ),
        )
}
