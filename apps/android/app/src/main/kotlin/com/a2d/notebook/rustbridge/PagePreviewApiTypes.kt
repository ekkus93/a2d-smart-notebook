package com.a2d.notebook.rustbridge

class PagePreviewProcessingException(
    val details: LivePageAnalysisErrorDetails,
    val nativePanic: Boolean,
) : Exception(
    "${details.code} [${details.correlationId}]: ${details.developerMessage}",
)

internal typealias PagePreviewCancellation = PolicyPagePreviewCancellation

data class PagePreviewProcessingRequest(
    val encodedBytes: ByteArray,
    val format: EncodedPageFormat,
    val rotation: EncodedPageRotation,
    val analysisPolicy: LivePageAnalysisPolicy,
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
)

data class ProcessedRgbImage(
    val width: Int,
    val height: Int,
    val bytes: ByteArray,
)

data class ProcessedPagePreview(
    val analysis: EncodedPageAnalysisResult,
    val corrected: ProcessedRgbImage,
    val thumbnail: ProcessedRgbImage,
    val pipelineVersion: Int,
    val sourceToCorrectedMatrix: List<Double>,
)

sealed interface PagePreviewProcessingOutcome {
    data class Completed(
        val result: ProcessedPagePreview,
    ) : PagePreviewProcessingOutcome

    data object Cancelled : PagePreviewProcessingOutcome
}
