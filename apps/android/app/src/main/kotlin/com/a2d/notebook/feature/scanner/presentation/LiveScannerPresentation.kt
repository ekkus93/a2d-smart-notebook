package com.a2d.notebook.feature.scanner.presentation

import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.AnalyzedPagePoint
import com.a2d.notebook.rustbridge.PageAnalysisResult
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution
import kotlin.math.abs

/**
 * Explicit, versioned presentation thresholds supplied by the owning scanner workflow.
 *
 * These thresholds produce user guidance only. They are not a substitute for the authoritative
 * Rust capture-acceptance policy that will drive the Milestone 8.3 auto-capture state machine.
 * There are intentionally no hidden production defaults: physical-device calibration remains an
 * open Milestone 7 evidence gate.
 */
data class LiveScannerGuidancePolicy(
    val minimumMarkerDecisionMargin: Double,
    val minimumFocusLaplacianVariance: Double,
    val minimumMeanLuminance: Double,
    val maximumHighlightFraction: Double,
    val maximumTileHighlightFraction: Double,
    val minimumPageCoverageFraction: Double,
    val maximumPageCoverageFraction: Double,
    val minimumEdgeMarginFraction: Double,
) {
    init {
        require(minimumMarkerDecisionMargin >= 0.0 && minimumMarkerDecisionMargin.isFinite())
        require(minimumFocusLaplacianVariance >= 0.0 && minimumFocusLaplacianVariance.isFinite())
        require(minimumMeanLuminance in 0.0..255.0)
        require(maximumHighlightFraction in 0.0..1.0)
        require(maximumTileHighlightFraction in 0.0..1.0)
        require(minimumPageCoverageFraction in 0.0..1.0)
        require(maximumPageCoverageFraction in 0.0..1.0)
        require(minimumPageCoverageFraction < maximumPageCoverageFraction)
        require(minimumEdgeMarginFraction in 0.0..0.5)
    }
}

enum class ScannerGuidanceCode {
    SELECT_NOTEBOOK,
    FIND_PAGE_CODE,
    WRONG_NOTEBOOK,
    SELECT_MATCHING_NOTEBOOK,
    REGISTER_NOTEBOOK,
    SMART_PAGE_OUTSIDE_NOTEBOOK_MODE,
    UNSUPPORTED_PAGE_CODE,
    ANALYSIS_FAILED,
    FIND_PAGE,
    SHOW_ALL_CORNERS,
    USE_SUPPORTED_PAGE,
    MOVE_CLOSER,
    MOVE_FARTHER,
    HOLD_STEADY,
    ADD_LIGHT,
    REDUCE_GLARE,
    PAGE_ALIGNED,
}

enum class ScannerGuidanceSeverity {
    INFO,
    WARNING,
    BLOCKING,
    POSITIVE,
}

data class ScannerGuidance(
    val code: ScannerGuidanceCode,
    val severity: ScannerGuidanceSeverity,
    val detail: String? = null,
)

enum class IdentityCaptureBlockReason {
    NO_ACTIVE_NOTEBOOK,
    PAGE_CODE_UNRESOLVED,
    WRONG_NOTEBOOK,
    NOTEBOOK_SELECTION_REQUIRED,
    NOTEBOOK_REGISTRATION_REQUIRED,
    SMART_PAGE_REQUIRES_SMART_PAGE_MODE,
    UNSUPPORTED_PAGE_CODE,
}

/**
 * Identity-only gate consumed by the future Milestone 8.3 auto-capture state machine.
 *
 * A page is eligible only after Rust returns [PageResolution.Resolved] with a Notebook ID exactly
 * matching the prominently displayed destination. Kotlin never guesses, silently switches the
 * destination, or treats an unresolved/ambiguous result as safe.
 */
data class IdentityAutoCaptureGate(
    val allowed: Boolean,
    val blockReason: IdentityCaptureBlockReason?,
) {
    init {
        require(allowed == (blockReason == null)) {
            "an allowed identity gate cannot carry a block reason"
        }
    }
}

data class ScannerOverlayMarker(
    val role: String,
    val corners: List<AnalyzedPagePoint>,
)

data class ScannerOverlayModel(
    val frameWidth: Long,
    val frameHeight: Long,
    val sourceRotationDegrees: Int,
    val pageBoundary: List<AnalyzedPagePoint>,
    val markers: List<ScannerOverlayMarker>,
    val conflict: Boolean,
)

data class LiveScannerPresentationState(
    val activeNotebook: NotebookSummary?,
    val guidance: ScannerGuidance,
    val identityGate: IdentityAutoCaptureGate,
    val overlay: ScannerOverlayModel?,
)

/** Builds one immutable scanner presentation snapshot from typed Rust and native-analysis results. */
fun buildLiveScannerPresentation(
    activeNotebook: NotebookSummary?,
    pageResolution: PageResolution?,
    analysis: PageAnalysisResult?,
    analysisFailure: String?,
    policy: LiveScannerGuidancePolicy,
): LiveScannerPresentationState {
    val identity = assessIdentity(activeNotebook, pageResolution)
    val overlay = analysis?.toOverlayModel(conflict = !identity.gate.allowed)
    val guidance =
        identity.blockingGuidance
            ?: analysisFailure?.let {
                ScannerGuidance(
                    code = ScannerGuidanceCode.ANALYSIS_FAILED,
                    severity = ScannerGuidanceSeverity.WARNING,
                    detail = it,
                )
            }
            ?: analysis?.guidance(policy)
            ?: ScannerGuidance(
                code = ScannerGuidanceCode.FIND_PAGE,
                severity = ScannerGuidanceSeverity.INFO,
            )

    return LiveScannerPresentationState(
        activeNotebook = activeNotebook,
        guidance = guidance,
        identityGate = identity.gate,
        overlay = overlay,
    )
}

private data class IdentityAssessment(
    val gate: IdentityAutoCaptureGate,
    val blockingGuidance: ScannerGuidance?,
)

private fun assessIdentity(
    activeNotebook: NotebookSummary?,
    resolution: PageResolution?,
): IdentityAssessment {
    if (activeNotebook == null) {
        return blockedIdentity(
            IdentityCaptureBlockReason.NO_ACTIVE_NOTEBOOK,
            ScannerGuidanceCode.SELECT_NOTEBOOK,
        )
    }

    return when (resolution) {
        null ->
            blockedIdentity(
                IdentityCaptureBlockReason.PAGE_CODE_UNRESOLVED,
                ScannerGuidanceCode.FIND_PAGE_CODE,
            )

        is PageResolution.Resolved -> {
            if (resolution.notebookId == activeNotebook.id) {
                IdentityAssessment(
                    gate = IdentityAutoCaptureGate(allowed = true, blockReason = null),
                    blockingGuidance = null,
                )
            } else {
                blockedIdentity(
                    IdentityCaptureBlockReason.WRONG_NOTEBOOK,
                    ScannerGuidanceCode.WRONG_NOTEBOOK,
                    detail = resolution.notebookId,
                )
            }
        }

        is PageResolution.ConflictingActiveNotebook ->
            blockedIdentity(
                IdentityCaptureBlockReason.WRONG_NOTEBOOK,
                ScannerGuidanceCode.WRONG_NOTEBOOK,
                detail = resolution.detectedDesign,
            )

        is PageResolution.RequiresNotebookSelection ->
            blockedIdentity(
                IdentityCaptureBlockReason.NOTEBOOK_SELECTION_REQUIRED,
                ScannerGuidanceCode.SELECT_MATCHING_NOTEBOOK,
                detail = resolution.candidates.joinToString { it.displayName },
            )

        is PageResolution.RequiresNotebookRegistration ->
            blockedIdentity(
                IdentityCaptureBlockReason.NOTEBOOK_REGISTRATION_REQUIRED,
                ScannerGuidanceCode.REGISTER_NOTEBOOK,
                detail = resolution.design.name,
            )

        is PageResolution.ImportedUnknownSmartPage ->
            blockedIdentity(
                IdentityCaptureBlockReason.SMART_PAGE_REQUIRES_SMART_PAGE_MODE,
                ScannerGuidanceCode.SMART_PAGE_OUTSIDE_NOTEBOOK_MODE,
                detail = resolution.smartPageId,
            )

        is PageResolution.UnsupportedCode ->
            blockedIdentity(
                IdentityCaptureBlockReason.UNSUPPORTED_PAGE_CODE,
                ScannerGuidanceCode.UNSUPPORTED_PAGE_CODE,
                detail = resolution.reason,
            )
    }
}

private fun blockedIdentity(
    reason: IdentityCaptureBlockReason,
    code: ScannerGuidanceCode,
    detail: String? = null,
): IdentityAssessment =
    IdentityAssessment(
        gate = IdentityAutoCaptureGate(allowed = false, blockReason = reason),
        blockingGuidance =
            ScannerGuidance(
                code = code,
                severity = ScannerGuidanceSeverity.BLOCKING,
                detail = detail,
            ),
    )

private fun PageAnalysisResult.toOverlayModel(conflict: Boolean): ScannerOverlayModel? {
    if (width <= 0L || height <= 0L) return null
    val byRole = markers.associateBy { it.role.uppercase() }
    val pageBoundary =
        listOf("TL", "TR", "BR", "BL")
            .mapNotNull { role -> byRole[role]?.center?.takeIf(AnalyzedPagePoint::isFinite) }
    val overlayMarkers =
        markers.mapNotNull { marker ->
            marker.corners
                .takeIf { corners -> corners.size == 4 && corners.all(AnalyzedPagePoint::isFinite) }
                ?.let { corners -> ScannerOverlayMarker(marker.role, corners) }
        }
    if (pageBoundary.isEmpty() && overlayMarkers.isEmpty()) return null
    return ScannerOverlayModel(
        frameWidth = width,
        frameHeight = height,
        sourceRotationDegrees = sourceRotationDegrees,
        pageBoundary = pageBoundary,
        markers = overlayMarkers,
        conflict = conflict,
    )
}

private fun PageAnalysisResult.guidance(policy: LiveScannerGuidancePolicy): ScannerGuidance {
    val expectedRoles = setOf("TL", "TR", "BR", "BL")
    val detectedRoles = markers.map { it.role.uppercase() }.toSet()
    if (markers.isEmpty()) {
        return ScannerGuidance(ScannerGuidanceCode.FIND_PAGE, ScannerGuidanceSeverity.INFO)
    }
    if (!detectedRoles.containsAll(expectedRoles)) {
        return ScannerGuidance(
            ScannerGuidanceCode.SHOW_ALL_CORNERS,
            ScannerGuidanceSeverity.INFO,
            detail = "${detectedRoles.size}/4",
        )
    }
    if (unexpectedTagIds.isNotEmpty()) {
        return ScannerGuidance(
            ScannerGuidanceCode.USE_SUPPORTED_PAGE,
            ScannerGuidanceSeverity.WARNING,
            detail = unexpectedTagIds.joinToString(),
        )
    }
    if (markers.any { it.decisionMargin < policy.minimumMarkerDecisionMargin }) {
        return ScannerGuidance(ScannerGuidanceCode.HOLD_STEADY, ScannerGuidanceSeverity.INFO)
    }

    val boundary = orderedBoundary(markers)
    if (boundary != null) {
        val coverage = polygonArea(boundary) / (width.toDouble() * height.toDouble())
        if (coverage < policy.minimumPageCoverageFraction) {
            return ScannerGuidance(ScannerGuidanceCode.MOVE_CLOSER, ScannerGuidanceSeverity.INFO)
        }
        val minimumEdgeMargin = minimumEdgeMarginFraction(boundary, width, height)
        if (
            coverage > policy.maximumPageCoverageFraction ||
                minimumEdgeMargin < policy.minimumEdgeMarginFraction
        ) {
            return ScannerGuidance(ScannerGuidanceCode.MOVE_FARTHER, ScannerGuidanceSeverity.INFO)
        }
    }

    val focus = quality.focusLaplacianVariance
    if (focus == null || focus < policy.minimumFocusLaplacianVariance) {
        return ScannerGuidance(ScannerGuidanceCode.HOLD_STEADY, ScannerGuidanceSeverity.INFO)
    }
    if (quality.meanLuminance < policy.minimumMeanLuminance) {
        return ScannerGuidance(ScannerGuidanceCode.ADD_LIGHT, ScannerGuidanceSeverity.INFO)
    }
    if (
        quality.highlightFraction > policy.maximumHighlightFraction ||
            quality.maxTileHighlightFraction > policy.maximumTileHighlightFraction
    ) {
        return ScannerGuidance(ScannerGuidanceCode.REDUCE_GLARE, ScannerGuidanceSeverity.INFO)
    }
    return ScannerGuidance(ScannerGuidanceCode.PAGE_ALIGNED, ScannerGuidanceSeverity.POSITIVE)
}

private fun orderedBoundary(markers: List<AnalyzedPageMarker>): List<AnalyzedPagePoint>? {
    val byRole = markers.associateBy { it.role.uppercase() }
    return listOf("TL", "TR", "BR", "BL")
        .map { role -> byRole[role]?.center ?: return null }
        .takeIf { points -> points.all(AnalyzedPagePoint::isFinite) }
}

private fun polygonArea(points: List<AnalyzedPagePoint>): Double {
    if (points.size < 3) return 0.0
    var doubledArea = 0.0
    points.indices.forEach { index ->
        val current = points[index]
        val next = points[(index + 1) % points.size]
        doubledArea += current.x * next.y - next.x * current.y
    }
    return abs(doubledArea) / 2.0
}

private fun minimumEdgeMarginFraction(
    points: List<AnalyzedPagePoint>,
    width: Long,
    height: Long,
): Double {
    val minX = points.minOf { it.x }
    val maxX = points.maxOf { it.x }
    val minY = points.minOf { it.y }
    val maxY = points.maxOf { it.y }
    return minOf(
        minX / width.toDouble(),
        (width.toDouble() - maxX) / width.toDouble(),
        minY / height.toDouble(),
        (height.toDouble() - maxY) / height.toDouble(),
    )
}

private fun AnalyzedPagePoint.isFinite(): Boolean = x.isFinite() && y.isFinite()
