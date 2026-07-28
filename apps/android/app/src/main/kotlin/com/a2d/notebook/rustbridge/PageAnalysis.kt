package com.a2d.notebook.rustbridge

import android.content.Context
import uniffi.a2d_ffi.AnalyzeEncodedPageRequest
import uniffi.a2d_ffi.EncodedImageRotation
import uniffi.a2d_ffi.ImageFileFormat

enum class EncodedPageFormat {
    JPEG,
    PNG,
}

enum class EncodedPageRotation(val degrees: Int) {
    DEGREES_0(0),
    DEGREES_90(90),
    DEGREES_180(180),
    DEGREES_270(270),
}

data class PageMarkerIds(
    val topLeft: Long,
    val topRight: Long,
    val bottomRight: Long,
    val bottomLeft: Long,
)

/**
 * Explicit Android request for the shared Rust decode, AprilTag, semantic-role, and quality path.
 *
 * There are intentionally no production defaults here. The caller must supply the safety limits,
 * detector configuration, measurement configuration, and page marker IDs selected by the owning
 * workflow or versioned policy. Kotlin only rejects signed values that cannot be represented by
 * the generated unsigned UniFFI types; Rust remains authoritative for all image-domain validation.
 */
data class EncodedPageAnalysisRequest(
    val encodedBytes: ByteArray,
    val format: EncodedPageFormat,
    val rotation: EncodedPageRotation,
    val maxEncodedBytes: Long,
    val maxPixels: Long,
    val maxDecodedBytes: Long,
    val detectorThreadCount: Int,
    val detectorQuadDecimate: Double,
    val detectorQuadSigma: Double,
    val detectorRefineEdges: Boolean,
    val detectorDecodeSharpening: Double,
    val detectorBitsCorrected: Int,
    val darkLuminanceCutoff: Int,
    val highlightLuminanceCutoff: Int,
    val qualityTileColumns: Int,
    val qualityTileRows: Int,
    val markerIds: PageMarkerIds,
)

data class AnalyzedPagePoint(
    val x: Double,
    val y: Double,
)

data class AnalyzedPageMarker(
    val role: String,
    val family: String,
    val id: Long,
    val hammingErrors: Long,
    val decisionMargin: Double,
    val center: AnalyzedPagePoint,
    val corners: List<AnalyzedPagePoint>,
)

data class AnalyzedPageQuality(
    val focusLaplacianVariance: Double?,
    val focusInteriorSampleCount: ULong?,
    val meanLuminance: Double,
    val luminanceStandardDeviation: Double,
    val darkFraction: Double,
    val highlightFraction: Double,
    val maxTileHighlightFraction: Double,
    val populatedTileCount: Long,
)

data class EncodedPageAnalysisResult(
    val width: Long,
    val height: Long,
    val sourceRotationDegrees: Int,
    val resolvedOrientationDegrees: Int,
    val markers: List<AnalyzedPageMarker>,
    val unexpectedTagIds: List<Long>,
    val quality: AnalyzedPageQuality,
)

fun A2dBridge.analyzeEncodedPage(
    context: Context,
    request: EncodedPageAnalysisRequest,
): EncodedPageAnalysisResult {
    val ffiRequest =
        AnalyzeEncodedPageRequest(
            encodedBytes = request.encodedBytes,
            format =
                when (request.format) {
                    EncodedPageFormat.JPEG -> ImageFileFormat.JPEG
                    EncodedPageFormat.PNG -> ImageFileFormat.PNG
                },
            rotation =
                when (request.rotation) {
                    EncodedPageRotation.DEGREES_0 -> EncodedImageRotation.DEGREES0
                    EncodedPageRotation.DEGREES_90 -> EncodedImageRotation.DEGREES90
                    EncodedPageRotation.DEGREES_180 -> EncodedImageRotation.DEGREES180
                    EncodedPageRotation.DEGREES_270 -> EncodedImageRotation.DEGREES270
                },
            maxEncodedBytes = request.maxEncodedBytes.checkedULong("maxEncodedBytes"),
            maxPixels = request.maxPixels.checkedULong("maxPixels"),
            maxDecodedBytes = request.maxDecodedBytes.checkedULong("maxDecodedBytes"),
            detectorThreadCount = request.detectorThreadCount.checkedUInt("detectorThreadCount"),
            detectorQuadDecimate = request.detectorQuadDecimate,
            detectorQuadSigma = request.detectorQuadSigma,
            detectorRefineEdges = request.detectorRefineEdges,
            detectorDecodeSharpening = request.detectorDecodeSharpening,
            detectorBitsCorrected =
                request.detectorBitsCorrected.checkedUInt("detectorBitsCorrected"),
            darkLuminanceCutoff =
                request.darkLuminanceCutoff.checkedUInt("darkLuminanceCutoff"),
            highlightLuminanceCutoff =
                request.highlightLuminanceCutoff.checkedUInt("highlightLuminanceCutoff"),
            qualityTileColumns = request.qualityTileColumns.checkedUInt("qualityTileColumns"),
            qualityTileRows = request.qualityTileRows.checkedUInt("qualityTileRows"),
            topLeftTagId = request.markerIds.topLeft.checkedUInt("markerIds.topLeft"),
            topRightTagId = request.markerIds.topRight.checkedUInt("markerIds.topRight"),
            bottomRightTagId =
                request.markerIds.bottomRight.checkedUInt("markerIds.bottomRight"),
            bottomLeftTagId = request.markerIds.bottomLeft.checkedUInt("markerIds.bottomLeft"),
        )

    val result = client(context).analyzeEncodedPage(ffiRequest)
    return EncodedPageAnalysisResult(
        width = result.width.toLong(),
        height = result.height.toLong(),
        sourceRotationDegrees = result.sourceRotationDegrees.toInt(),
        resolvedOrientationDegrees = result.resolvedOrientationDegrees.toInt(),
        markers =
            result.markers.map { marker ->
                AnalyzedPageMarker(
                    role = marker.role,
                    family = marker.family,
                    id = marker.id.toLong(),
                    hammingErrors = marker.hammingErrors.toLong(),
                    decisionMargin = marker.decisionMargin,
                    center = AnalyzedPagePoint(marker.center.x, marker.center.y),
                    corners = marker.corners.map { AnalyzedPagePoint(it.x, it.y) },
                )
            },
        unexpectedTagIds = result.unexpectedTagIds.map { it.toLong() },
        quality =
            AnalyzedPageQuality(
                focusLaplacianVariance = result.quality.focusLaplacianVariance,
                focusInteriorSampleCount = result.quality.focusInteriorSampleCount,
                meanLuminance = result.quality.meanLuminance,
                luminanceStandardDeviation = result.quality.luminanceStandardDeviation,
                darkFraction = result.quality.darkFraction,
                highlightFraction = result.quality.highlightFraction,
                maxTileHighlightFraction = result.quality.maxTileHighlightFraction,
                populatedTileCount = result.quality.populatedTileCount.toLong(),
            ),
    )
}

private fun Long.checkedULong(field: String): ULong {
    require(this >= 0L) { "$field must not be negative" }
    return toULong()
}

private fun Int.checkedUInt(field: String): UInt {
    require(this >= 0) { "$field must not be negative" }
    return toUInt()
}

private fun Long.checkedUInt(field: String): UInt {
    require(this in 0L..UInt.MAX_VALUE.toLong()) { "$field must fit an unsigned 32-bit integer" }
    return toUInt()
}
