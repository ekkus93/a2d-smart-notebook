package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.capture.AutoCapturePolicy
import com.a2d.notebook.feature.scanner.presentation.LiveScannerGuidancePolicy

/**
 * Versioned Android presentation and capture-guidance policy.
 *
 * Portable marker layouts, processing limits, corrected dimensions, and pipeline versions are
 * resolved from Rust per stored page and are not duplicated here.
 */
object SinglePageScannerPolicies {
    val V1 =
        SinglePageScannerPolicy(
            version = 1,
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
        )
}
