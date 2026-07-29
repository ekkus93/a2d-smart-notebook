package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.a2d.notebook.feature.scanner.capture.AutoCapturePolicy
import com.a2d.notebook.feature.scanner.presentation.LiveScannerGuidancePolicy
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import com.a2d.notebook.rustbridge.LivePageAnalysisPolicy
import com.a2d.notebook.rustbridge.PageMarkerIds
import uniffi.a2d_ffi.StoredScanLayoutPolicy

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

/**
 * One scanner-session policy. Guidance and acceptance thresholds remain Android workflow policy,
 * while physical page geometry, marker assignments, and corrected dimensions are projected from
 * Rust after the Page Code resolves to a stored page.
 */
class SinglePageScannerPolicy(
    val version: Int,
    private val baseLiveAnalysis: LivePageAnalysisPolicy,
    val guidance: LiveScannerGuidancePolicy,
    val captureThresholds: SinglePageCaptureThresholds,
    val autoCapture: AutoCapturePolicy,
    val autoCaptureEnabled: Boolean,
    val pageCodeFreshnessNanos: Long,
    private val baseFullResolution: FullResolutionPreviewPolicy,
) {
    private var storedLayoutPolicy by mutableStateOf<StoredScanLayoutPolicy?>(null)

    val liveAnalysis: LivePageAnalysisPolicy
        get() =
            storedLayoutPolicy?.let { layout ->
                baseLiveAnalysis.copy(markerIds = layout.toMarkerIds())
            } ?: baseLiveAnalysis

    /**
     * Fails visibly until Rust resolves the stored page. A capture must never be rectified using a
     * development-page fallback while canonical page geometry is unavailable.
     */
    val fullResolution: FullResolutionPreviewPolicy
        get() {
            val layout =
                checkNotNull(storedLayoutPolicy) {
                    "Rust stored scan layout policy has not been resolved for this page"
                }
            return baseFullResolution.copy(
                correctedWidth = layout.correctedWidth.toPositiveInt("correctedWidth"),
                correctedHeight = layout.correctedHeight.toPositiveInt("correctedHeight"),
            )
        }

    val resolvedLayoutId: String?
        get() = storedLayoutPolicy?.layoutId

    init {
        require(version > 0)
        require(pageCodeFreshnessNanos > 0)
    }

    fun applyStoredLayoutPolicy(layout: StoredScanLayoutPolicy) {
        require(layout.markerFamily == EXPECTED_MARKER_FAMILY) {
            "Unsupported Rust marker family ${layout.markerFamily}"
        }
        require(layout.processingPolicyVersion == EXPECTED_PROCESSING_POLICY_VERSION) {
            "Unsupported Rust scan processing policy version ${layout.processingPolicyVersion}"
        }
        layout.correctedWidth.toPositiveInt("correctedWidth")
        layout.correctedHeight.toPositiveInt("correctedHeight")
        storedLayoutPolicy = layout
    }

    fun clearStoredLayoutPolicy() {
        storedLayoutPolicy = null
    }

    private fun StoredScanLayoutPolicy.toMarkerIds(): PageMarkerIds =
        PageMarkerIds(
            topLeft = topLeftTagId.toLong(),
            topRight = topRightTagId.toLong(),
            bottomRight = bottomRightTagId.toLong(),
            bottomLeft = bottomLeftTagId.toLong(),
        )

    private fun UInt.toPositiveInt(field: String): Int {
        require(this in 1u..Int.MAX_VALUE.toUInt()) { "$field is outside the Android Int range" }
        return toInt()
    }

    private companion object {
        const val EXPECTED_MARKER_FAMILY = "tagStandard41h12"
        const val EXPECTED_PROCESSING_POLICY_VERSION: UInt = 1u
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
    /** Shared by the one active scanner ViewModel and its camera composable for this app process. */
    val V1 =
        SinglePageScannerPolicy(
            version = 1,
            baseLiveAnalysis =
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
            baseFullResolution =
                FullResolutionPreviewPolicy(
                    maximumEncodedBytes = 24L * 1024 * 1024,
                    maximumPixels = 32_000_000,
                    maximumDecodedBytes = 96_000_000,
                    // Constructor placeholders are never exposed before Rust layout resolution.
                    correctedWidth = 2,
                    correctedHeight = 2,
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
