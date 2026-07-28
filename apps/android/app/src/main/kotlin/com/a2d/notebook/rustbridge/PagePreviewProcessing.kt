package com.a2d.notebook.rustbridge

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.AnalyzeEncodedPageRequest
import uniffi.a2d_ffi.EncodedImageRotation
import uniffi.a2d_ffi.ImageFileFormat
import uniffi.a2d_ffi.PreviewLayoutKind
import uniffi.a2d_ffi.PreviewProcessingCancellation
import uniffi.a2d_ffi.ProcessEncodedPagePreviewRequest
import uniffi.a2d_ffi.ProcessEncodedPagePreviewStatus

class PagePreviewCancellation internal constructor(
    internal val ffi: PreviewProcessingCancellation,
) {
    constructor() : this(PreviewProcessingCancellation())

    fun cancel() = ffi.cancel()
    fun isCancelled(): Boolean = ffi.isCancelled()
}

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

fun A2dClient.processPagePreview(
    request: PagePreviewProcessingRequest,
    cancellation: PagePreviewCancellation,
): PagePreviewProcessingOutcome {
    require(request.encodedBytes.isNotEmpty()) { "encoded capture must not be empty" }
    require(request.maximumEncodedBytes > 0)
    require(request.maximumPixels > 0)
    require(request.maximumDecodedBytes > 0)
    require(request.correctedWidth > 1 && request.correctedHeight > 1)
    require(request.pipelineVersion > 0)

    val raw =
        processEncodedPagePreview(
            ProcessEncodedPagePreviewRequest(
                analysis =
                    AnalyzeEncodedPageRequest(
                        encodedBytes = request.encodedBytes,
                        format = request.format.toFfi(),
                        rotation = request.rotation.toFfi(),
                        maxEncodedBytes = request.maximumEncodedBytes.toULong(),
                        maxPixels = request.maximumPixels.toULong(),
                        maxDecodedBytes = request.maximumDecodedBytes.toULong(),
                        detectorThreadCount = request.analysisPolicy.detectorThreadCount.toUInt(),
                        detectorQuadDecimate = request.analysisPolicy.detectorQuadDecimate,
                        detectorQuadSigma = request.analysisPolicy.detectorQuadSigma,
                        detectorRefineEdges = request.analysisPolicy.detectorRefineEdges,
                        detectorDecodeSharpening = request.analysisPolicy.detectorDecodeSharpening,
                        detectorBitsCorrected = request.analysisPolicy.detectorBitsCorrected.toUInt(),
                        darkLuminanceCutoff = request.analysisPolicy.darkLuminanceCutoff.toUInt(),
                        highlightLuminanceCutoff =
                            request.analysisPolicy.highlightLuminanceCutoff.toUInt(),
                        qualityTileColumns = request.analysisPolicy.qualityTileColumns.toUInt(),
                        qualityTileRows = request.analysisPolicy.qualityTileRows.toUInt(),
                        topLeftTagId = request.analysisPolicy.markerIds.topLeft.toUInt(),
                        topRightTagId = request.analysisPolicy.markerIds.topRight.toUInt(),
                        bottomRightTagId = request.analysisPolicy.markerIds.bottomRight.toUInt(),
                        bottomLeftTagId = request.analysisPolicy.markerIds.bottomLeft.toUInt(),
                    ),
                layoutKind = PreviewLayoutKind.NOTEBOOKWRITABLEV1,
                correctedWidth = request.correctedWidth.toUInt(),
                correctedHeight = request.correctedHeight.toUInt(),
                rectificationMaxOutputPixels = request.rectificationMaximumOutputPixels.toULong(),
                rectificationMaxOutputBytes = request.rectificationMaximumOutputBytes.toULong(),
                pipelineVersion = request.pipelineVersion.toUInt(),
                contrastLowPercentilePerMillion =
                    request.contrastLowPercentilePerMillion.toUInt(),
                contrastHighPercentilePerMillion =
                    request.contrastHighPercentilePerMillion.toUInt(),
                contrastMaximumGain = request.contrastMaximumGain,
                sharpening = null,
                thumbnailMaxWidth = request.thumbnailMaximumWidth.toUInt(),
                thumbnailMaxHeight = request.thumbnailMaximumHeight.toUInt(),
                derivedMaxPixelsPerImage = request.derivedMaximumPixelsPerImage.toULong(),
                derivedMaxBytesPerImage = request.derivedMaximumBytesPerImage.toULong(),
                derivedMaxTotalOutputBytes = request.derivedMaximumTotalOutputBytes.toULong(),
                derivedMaxWorkingBytes = request.derivedMaximumWorkingBytes.toULong(),
            ),
            cancellation.ffi,
        )
    if (raw.status == ProcessEncodedPagePreviewStatus.CANCELLED) {
        check(raw.result == null) { "cancelled Rust preview processing returned a result" }
        return PagePreviewProcessingOutcome.Cancelled
    }
    val result = requireNotNull(raw.result) { "completed Rust preview processing omitted its result" }
    return PagePreviewProcessingOutcome.Completed(
        ProcessedPagePreview(
            analysis = result.analysis.toKotlin(),
            corrected =
                ProcessedRgbImage(
                    width = result.corrected.width.toInt(),
                    height = result.corrected.height.toInt(),
                    bytes = result.corrected.rgbBytes,
                ).validated(),
            thumbnail =
                ProcessedRgbImage(
                    width = result.thumbnail.width.toInt(),
                    height = result.thumbnail.height.toInt(),
                    bytes = result.thumbnail.rgbBytes,
                ).validated(),
            pipelineVersion = result.pipelineVersion.toInt(),
            sourceToCorrectedMatrix = result.sourceToCorrectedMatrix,
        ),
    )
}

private fun ProcessedRgbImage.validated(): ProcessedRgbImage {
    require(width > 0 && height > 0)
    require(bytes.size == Math.multiplyExact(Math.multiplyExact(width, height), 3)) {
        "Rust RGB preview byte count does not match its dimensions"
    }
    return this
}

private fun uniffi.a2d_ffi.AnalyzeEncodedPageResult.toKotlin(): EncodedPageAnalysisResult =
    EncodedPageAnalysisResult(
        width = width.toLong(),
        height = height.toLong(),
        sourceRotationDegrees = sourceRotationDegrees.toInt(),
        resolvedOrientationDegrees = resolvedOrientationDegrees.toInt(),
        markers =
            markers.map { marker ->
                AnalyzedPageMarker(
                    role = marker.role,
                    family = marker.family,
                    id = marker.id.toLong(),
                    hammingErrors = marker.hammingErrors.toInt(),
                    decisionMargin = marker.decisionMargin,
                    center = AnalyzedPagePoint(marker.center.x, marker.center.y),
                    corners = marker.corners.map { AnalyzedPagePoint(it.x, it.y) },
                )
            },
        unexpectedTagIds = unexpectedTagIds.map(ULong::toLong),
        quality =
            AnalyzedPageQuality(
                focusLaplacianVariance = quality.focusLaplacianVariance,
                focusInteriorSampleCount = quality.focusInteriorSampleCount,
                meanLuminance = quality.meanLuminance,
                luminanceStandardDeviation = quality.luminanceStandardDeviation,
                darkFraction = quality.darkFraction,
                highlightFraction = quality.highlightFraction,
                maxTileHighlightFraction = quality.maxTileHighlightFraction,
                populatedTileCount = quality.populatedTileCount.toLong(),
            ),
    )

private fun EncodedPageFormat.toFfi(): ImageFileFormat =
    when (this) {
        EncodedPageFormat.JPEG -> ImageFileFormat.JPEG
        EncodedPageFormat.PNG -> ImageFileFormat.PNG
    }

private fun EncodedPageRotation.toFfi(): EncodedImageRotation =
    when (this) {
        EncodedPageRotation.DEGREES_0 -> EncodedImageRotation.DEGREES0
        EncodedPageRotation.DEGREES_90 -> EncodedImageRotation.DEGREES90
        EncodedPageRotation.DEGREES_180 -> EncodedImageRotation.DEGREES180
        EncodedPageRotation.DEGREES_270 -> EncodedImageRotation.DEGREES270
    }
