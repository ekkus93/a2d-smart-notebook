package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult

/** Independent capture-acceptance thresholds. These are Android review/presentation thresholds. */
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
