package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.capture.AutoCapturePolicy
import com.a2d.notebook.feature.scanner.presentation.LiveScannerGuidancePolicy
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import com.a2d.notebook.rustbridge.LivePageAnalysisPolicy
import com.a2d.notebook.rustbridge.PageMarkerIds

/** Independent capture-acceptance thresholds. These are not presentation guidance thresholds. */
data class SinglePageCaptureThresholds(
    val minimumMarkerDecisionMargin: Double,
    val minimumFocusLaplacianVariance: Double,
    val minimumMeanLuminance: Double,
    val maximumDarkFraction: Double,
    val maximumHighlightFraction: Double,
    val maximumTileHighlightFraction: Double,
) {
    init {
        require(minimumMarkerDecisionMargin >= 0.0 && minimumMarkerDecisionMargin.isFinite())
        require(minimumFocusLaplacianVariance >= 0.0 && minimumFocusLaplacianVariance.isFinite())
        require(minimumMeanLuminance in 0.0..255.0)
        require(maximumDarkFraction in 0.0..1.0)
        require(maximumHighlightFraction in 0.0..1.0)
        require(maximumTileHighlightFraction in 0.0..1.0)
    }
}

enum class CapturePolicyWarning {
    MISSING_MARKERS,
    UNEXPECTED_MARKERS,
    LOW_MARKER_CONFIDENCE,
    LOW_FOCUS,
    TOO_DARK,
    TOO_MUCH_DARK_AREA,
    TOO_MUCH_HIGHLIGHT,
    LOCALIZED_GLARE,
}

data class CapturePolicyAssessment(
    val accepted: Boolean,
    val warnings: Set<CapturePolicyWarning>,
)

fun assessCapturePolicy(
    analysis: EncodedPageAnalysisResult?,
    thresholds: SinglePageCaptureThresholds,
): CapturePolicyAssessment {
    if (analysis == null) {
        return CapturePolicyAssessment(false, setOf(CapturePolicyWarning.MISSING_MARKERS))
    }
    val warnings = buildSet {
        val roles = analysis.markers.map { it.role.uppercase() }.toSet()
        if (!roles.containsAll(setOf("TL", "TR", "BR", "BL"))) {
            add(CapturePolicyWarning.MISSING_MARKERS)
        }
        if (analysis.unexpectedTagIds.isNotEmpty()) add(CapturePolicyWarning.UNEXPECTED_MARKERS)
        if (analysis.markers.any { it.decisionMargin < thresholds.minimumMarkerDecisionMargin }) {
            add(CapturePolicyWarning.LOW_MARKER_CONFIDENCE)
        }
        val focus = analysis.quality.focusLaplacianVariance
        if (focus == null || focus < thresholds.minimumFocusLaplacianVariance) {
            add(CapturePolicyWarning.LOW_FOCUS)
        }
        if (analysis.quality.meanLuminance < thresholds.minimumMeanLuminance) {
            add(CapturePolicyWarning.TOO_DARK)
        }
        if (analysis.quality.darkFraction > thresholds.maximumDarkFraction) {
            add(CapturePolicyWarning.TOO_MUCH_DARK_AREA)
        }
        if (analysis.quality.highlightFraction > thresholds.maximumHighlightFraction) {
            add(CapturePolicyWarning.TOO_MUCH_HIGHLIGHT)
        }
        if (analysis.quality.maxTileHighlightFraction > thresholds.maximumTileHighlightFraction) {
            add(CapturePolicyWarning.LOCALIZED_GLARE)
        }
    }
    return CapturePolicyAssessment(warnings.isEmpty(), warnings)
}

data class FullResolutionPreviewPolicy(
    val maximumEncodedBytes: Long,
    val maximumPixels: Long,
    val maximumDecodedBytes: Long,
    val correctedWidth: Int,
    val correctedHeight: Int,
    val rectificationMaximumOutputPixels: Long,
    val rectificationMaximumOutputBytes: Long,
    val pipelineVersion: Int,
    val contrastLowPercentilePerMillion: Int,
    val contrastHighPercentilePerMillion: Int,
    val contrastMaximumGain: Double,
    val thumbnailMaximumWidth: Int,
    val thumbnailMaximumHeight: Int,
    val derivedMaximumPixelsPerImage: Long,
    val derivedMaximumBytesPerImage: Long,
    val derivedMaximumTotalOutputBytes: Long,
    val derivedMaximumWorkingBytes: Long,
) {
    init {
        require(maximumEncodedBytes > 0)
        require(maximumPixels > 0)
        require(maximumDecodedBytes > 0)
        require(correctedWidth >= 2 && correctedHeight >= 2)
        require(rectificationMaximumOutputPixels > 0)
        require(rectificationMaximumOutputBytes > 0)
        require(pipelineVersion > 0)
        require(contrastLowPercentilePerMillion in 0..1_000_000)
        require(contrastHighPercentilePerMillion in 0..1_000_000)
        require(contrastLowPercentilePerMillion < contrastHighPercentilePerMillion)
        require(contrastMaximumGain >= 1.0 && contrastMaximumGain.isFinite())
        require(thumbnailMaximumWidth > 0 && thumbnailMaximumHeight > 0)
        require(derivedMaximumPixelsPerImage > 0)
        require(derivedMaximumBytesPerImage > 0)
        require(derivedMaximumTotalOutputBytes > 0)
        require(derivedMaximumWorkingBytes >= derivedMaximumTotalOutputBytes)
    }
}

data class SinglePageScannerPolicy(
    val version: Int,
    val liveAnalysis: LivePageAnalysisPolicy,
    val guidance: LiveScannerGuidancePolicy,
    val captureThresholds: SinglePageCaptureThresholds,
    val autoCapture: AutoCapturePolicy,
    val autoCaptureEnabled: Boolean,
    val pageCodeFreshnessNanos: Long,
    val fullResolution: FullResolutionPreviewPolicy,
) {
    init {
        require(version > 0)
        require(pageCodeFreshnessNanos > 0)
    }
}

/**
 * Versioned v0.1 scanner configuration.
 *
 * Automatic capture is deliberately disabled: Milestone 7 still requires photographed fixtures and
 * physical Android performance/quality calibration. The provisional thresholds support guidance,
 * manual-capture warnings, deterministic tests, and final review only; they are not claimed as
 * calibrated production acceptance limits.
 */
object SinglePageScannerPolicies {
    val V1 =
        SinglePageScannerPolicy(
            version = 1,
            liveAnalysis =
                LivePageAnalysisPolicy(
                    maxPixels = 4_194_304,
                    detectorThreadCount = 1,
                    detectorQuadDecimate = 2.0,
                    detectorQuadSigma = 0.0,
                    detectorRefineEdges = true,
                    detectorDecodeSharpening = 0.25,
                    detectorBitsCorrected = 2,
                    darkLuminanceCutoff = 32,
                    highlightLuminanceCutoff = 245,
                    qualityTileColumns = 8,
                    qualityTileRows = 8,
                    markerIds = PageMarkerIds(0, 1, 2, 3),
                ),
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
            captureThresholds =
                SinglePageCaptureThresholds(
                    minimumMarkerDecisionMargin = 20.0,
                    minimumFocusLaplacianVariance = 40.0,
                    minimumMeanLuminance = 50.0,
                    maximumDarkFraction = 0.45,
                    maximumHighlightFraction = 0.15,
                    maximumTileHighlightFraction = 0.35,
                ),
            autoCapture =
                AutoCapturePolicy(
                    stableIntervalNanos = 1_000_000_000,
                    maximumInterFrameGapNanos = 500_000_000,
                    repeatDebounceNanos = 5_000_000_000,
                ),
            autoCaptureEnabled = false,
            pageCodeFreshnessNanos = 1_000_000_000,
            fullResolution =
                FullResolutionPreviewPolicy(
                    maximumEncodedBytes = 24L * 1024 * 1024,
                    maximumPixels = 32_000_000,
                    maximumDecodedBytes = 96_000_000,
                    correctedWidth = 900,
                    correctedHeight = 1_356,
                    rectificationMaximumOutputPixels = 2_000_000,
                    rectificationMaximumOutputBytes = 6_000_000,
                    pipelineVersion = 1,
                    contrastLowPercentilePerMillion = 10_000,
                    contrastHighPercentilePerMillion = 990_000,
                    contrastMaximumGain = 2.0,
                    thumbnailMaximumWidth = 480,
                    thumbnailMaximumHeight = 480,
                    derivedMaximumPixelsPerImage = 2_000_000,
                    derivedMaximumBytesPerImage = 6_000_000,
                    derivedMaximumTotalOutputBytes = 12_000_000,
                    derivedMaximumWorkingBytes = 96_000_000,
                ),
        )
}
