package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.capture.AutoCapturePolicy
import com.a2d.notebook.feature.scanner.presentation.LiveScannerGuidancePolicy
import com.a2d.notebook.rustbridge.LivePageAnalysisPolicy
import com.a2d.notebook.rustbridge.StoredScanPolicy

/**
 * Transitional shape consumed by the existing ViewModel request builder.
 *
 * Only encoded/decode limits, dimensions, and pipeline identity are read from the Rust-issued
 * policy. The remaining fields are non-authoritative compatibility sentinels; the policy-bound
 * native preview ABI does not accept or consume them.
 */
data class FullResolutionPreviewPolicy(
    val maximumEncodedBytes: Long,
    val maximumPixels: Long,
    val maximumDecodedBytes: Long,
    val correctedWidth: Int,
    val correctedHeight: Int,
    val rectificationMaximumOutputPixels: Long = 1,
    val rectificationMaximumOutputBytes: Long = 1,
    val pipelineVersion: Int,
    val contrastLowPercentilePerMillion: Int = 0,
    val contrastHighPercentilePerMillion: Int = 1,
    val contrastMaximumGain: Double = 1.0,
    val thumbnailMaximumWidth: Int = 1,
    val thumbnailMaximumHeight: Int = 1,
    val derivedMaximumPixelsPerImage: Long = 1,
    val derivedMaximumBytesPerImage: Long = 1,
    val derivedMaximumTotalOutputBytes: Long = 1,
    val derivedMaximumWorkingBytes: Long = 1,
)

data class SinglePageScannerPolicy(
    val version: Int,
    val guidance: LiveScannerGuidancePolicy,
    val captureThresholds: SinglePageCaptureThresholds,
    val autoCapture: AutoCapturePolicy,
    val autoCaptureEnabled: Boolean,
    val pageCodeFreshnessNanos: Long,
) {
    init {
        require(version > 0)
        require(pageCodeFreshnessNanos > 0)
    }

    val liveAnalysis: LivePageAnalysisPolicy
        get() = RustScannerPolicySession.requireCurrentPolicy().liveAnalysisPolicy

    val fullResolution: FullResolutionPreviewPolicy
        get() = RustScannerPolicySession.requireCurrentPolicy().toLegacyPreviewPolicy()
}

private fun StoredScanPolicy.toLegacyPreviewPolicy(): FullResolutionPreviewPolicy =
    FullResolutionPreviewPolicy(
        maximumEncodedBytes = maximumEncodedBytes,
        maximumPixels = maximumDecodedPixels,
        maximumDecodedBytes = maximumDecodedBytes,
        correctedWidth = correctedWidth,
        correctedHeight = correctedHeight,
        pipelineVersion = pipelineVersion,
    )
