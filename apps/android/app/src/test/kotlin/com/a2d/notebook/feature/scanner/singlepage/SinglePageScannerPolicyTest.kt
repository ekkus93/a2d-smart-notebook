package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.AnalyzedPagePoint
import com.a2d.notebook.rustbridge.AnalyzedPageQuality
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SinglePageScannerPolicyTest {
    private val thresholds = SinglePageScannerPolicies.V1.captureThresholds

    @Test
    fun completeHighQualityAnalysisIsAccepted() {
        val assessment = assessCapturePolicy(goodAnalysis(), thresholds)
        assertTrue(assessment.accepted)
        assertTrue(assessment.warnings.isEmpty())
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
        assertFalse(SinglePageScannerPolicies.V1.autoCaptureEnabled)
    }

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
