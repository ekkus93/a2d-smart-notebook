package com.a2d.notebook.rustbridge

import com.a2d.notebook.feature.scanner.camera.CameraAnalysisFrame
import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Shared detector and quality configuration for one live luminance frame. No production defaults
 * are hidden here; the owning scanner workflow must provide a versioned policy explicitly. */
data class LivePageAnalysisPolicy(
    val maxPixels: Long,
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

typealias PageAnalysisResult = EncodedPageAnalysisResult

/** Structured Rust error returned by the live-analysis ABI. */
data class LivePageAnalysisErrorDetails(
    val code: String,
    val category: String,
    val severity: String,
    val userMessageKey: String,
    val developerMessage: String,
    val correlationId: String,
    val retryable: Boolean,
)

class LivePageAnalysisException(
    val details: LivePageAnalysisErrorDetails,
    val nativePanic: Boolean,
) : Exception(
    "${details.code} [${details.correlationId}]: ${details.developerMessage}",
)

fun interface LivePageAnalyzer {
    @Throws(Exception::class)
    fun analyze(
        frame: CameraAnalysisFrame,
        policy: LivePageAnalysisPolicy,
    ): PageAnalysisResult
}

/**
 * Synchronous borrowed-buffer adapter to the shared Rust live-analysis ABI.
 *
 * The caller must invoke this off the Android main thread. The frame's direct luminance buffer is
 * borrowed for the duration of the JNA call and is not copied into a UniFFI RustBuffer. The small
 * versioned result/error payload is copied once into Kotlin-owned memory, decoded strictly, and the
 * Rust allocation is released in `finally`.
 */
object NativeLivePageAnalyzer : LivePageAnalyzer {
    private const val STATUS_SUCCESS = 0
    private const val STATUS_ERROR = 1
    private const val STATUS_PANIC = 2

    private val library: LiveAnalysisNativeLibrary by lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        Native.load("a2d_ffi", LiveAnalysisNativeLibrary::class.java)
    }

    override fun analyze(
        frame: CameraAnalysisFrame,
        policy: LivePageAnalysisPolicy,
    ): PageAnalysisResult {
        val luminance = frame.luminanceBuffer()
        require(luminance.isDirect) { "live analysis requires a direct luminance buffer" }
        require(luminance.position() == 0) { "live luminance buffer must start at position zero" }
        require(luminance.remaining() == frame.luminanceByteCount) {
            "live luminance buffer view does not expose the complete frame"
        }

        val maxPixels = policy.maxPixels.checkedULongBits("maxPixels")
        val detectorThreadCount = policy.detectorThreadCount.checkedUIntBits("detectorThreadCount")
        val detectorBitsCorrected =
            policy.detectorBitsCorrected.checkedUIntBits("detectorBitsCorrected")
        val darkLuminanceCutoff =
            policy.darkLuminanceCutoff.checkedUIntBits("darkLuminanceCutoff")
        val highlightLuminanceCutoff =
            policy.highlightLuminanceCutoff.checkedUIntBits("highlightLuminanceCutoff")
        val qualityTileColumns = policy.qualityTileColumns.checkedUIntBits("qualityTileColumns")
        val qualityTileRows = policy.qualityTileRows.checkedUIntBits("qualityTileRows")
        val topLeftTagId = policy.markerIds.topLeft.checkedUIntBits("markerIds.topLeft")
        val topRightTagId = policy.markerIds.topRight.checkedUIntBits("markerIds.topRight")
        val bottomRightTagId =
            policy.markerIds.bottomRight.checkedUIntBits("markerIds.bottomRight")
        val bottomLeftTagId = policy.markerIds.bottomLeft.checkedUIntBits("markerIds.bottomLeft")

        val status = LiveAnalysisStatus()
        val output =
            library.a2d_live_analyze_gray_frame(
                Native.getDirectBufferPointer(luminance),
                luminance.remaining().toLong(),
                frame.width,
                frame.height,
                frame.packedRowStride.toLong(),
                frame.rotationDegrees,
                maxPixels,
                detectorThreadCount,
                policy.detectorQuadDecimate,
                policy.detectorQuadSigma,
                if (policy.detectorRefineEdges) 1 else 0,
                policy.detectorDecodeSharpening,
                detectorBitsCorrected,
                darkLuminanceCutoff,
                highlightLuminanceCutoff,
                qualityTileColumns,
                qualityTileRows,
                topLeftTagId,
                topRightTagId,
                bottomRightTagId,
                bottomLeftTagId,
                status,
            )
        status.read()
        output.read()
        status.error.read()

        try {
            return when (status.code) {
                STATUS_SUCCESS -> {
                    require(status.error.data == null && status.error.len == 0L) {
                        "live analysis reported success with an unexpected error buffer"
                    }
                    decodeResult(readRequiredBuffer(output, "live analysis result"))
                }

                STATUS_ERROR,
                STATUS_PANIC,
                -> {
                    require(output.data == null && output.len == 0L) {
                        "live analysis reported failure with an unexpected result buffer"
                    }
                    throw LivePageAnalysisException(
                        details = decodeError(readRequiredBuffer(status.error, "live analysis error")),
                        nativePanic = status.code == STATUS_PANIC,
                    )
                }

                else -> throw IllegalStateException(
                    "live analysis returned unknown status code ${status.code}",
                )
            }
        } finally {
            freeIfOwned(output)
            freeIfOwned(status.error)
        }
    }

    private fun readRequiredBuffer(
        buffer: LiveAnalysisBuffer,
        description: String,
    ): ByteArray {
        require(buffer.len > 0L) { "$description buffer is empty" }
        require(buffer.capacity >= buffer.len) {
            "$description buffer length exceeds capacity"
        }
        val pointer = requireNotNull(buffer.data) { "$description buffer pointer is null" }
        val length = Math.toIntExact(buffer.len)
        return pointer.getByteArray(0L, length)
    }

    private fun freeIfOwned(buffer: LiveAnalysisBuffer.ByValue) {
        if (buffer.data != null) {
            library.a2d_live_analysis_buffer_free(buffer)
        }
    }
}

@Structure.FieldOrder("capacity", "len", "data")
internal open class LiveAnalysisBuffer : Structure() {
    @JvmField var capacity: Long = 0L
    @JvmField var len: Long = 0L
    @JvmField var data: Pointer? = null

    class ByValue : LiveAnalysisBuffer(), Structure.ByValue
}

@Structure.FieldOrder("code", "error")
internal open class LiveAnalysisStatus : Structure() {
    @JvmField var code: Int = 0
    @JvmField var error: LiveAnalysisBuffer.ByValue = LiveAnalysisBuffer.ByValue()
}

internal interface LiveAnalysisNativeLibrary : Library {
    @Suppress("LongParameterList")
    fun a2d_live_analyze_gray_frame(
        bytes: Pointer?,
        bytesLen: Long,
        width: Int,
        height: Int,
        rowStride: Long,
        rotationDegrees: Int,
        maxPixels: Long,
        detectorThreadCount: Int,
        detectorQuadDecimate: Double,
        detectorQuadSigma: Double,
        detectorRefineEdges: Byte,
        detectorDecodeSharpening: Double,
        detectorBitsCorrected: Int,
        darkLuminanceCutoff: Int,
        highlightLuminanceCutoff: Int,
        qualityTileColumns: Int,
        qualityTileRows: Int,
        topLeftTagId: Int,
        topRightTagId: Int,
        bottomRightTagId: Int,
        bottomLeftTagId: Int,
        status: LiveAnalysisStatus,
    ): LiveAnalysisBuffer.ByValue

    fun a2d_live_analysis_buffer_free(buffer: LiveAnalysisBuffer.ByValue)
}

private class LiveAnalysisCodecReader(
    bytes: ByteArray,
) {
    private val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)

    fun requireHeader(expectedMagic: String) {
        require(buffer.remaining() >= 8) { "live analysis payload is shorter than its header" }
        val magic = ByteArray(4).also(buffer::get).toString(Charsets.US_ASCII)
        require(magic == expectedMagic) {
            "live analysis payload magic was $magic instead of $expectedMagic"
        }
        val version = readUInt("codec version")
        require(version == 1L) { "unsupported live analysis codec version $version" }
    }

    fun readByte(field: String): Int {
        require(buffer.remaining() >= 1) { "live analysis payload ended before $field" }
        return buffer.get().toInt() and 0xff
    }

    fun readBoolean(field: String): Boolean = when (val value = readByte(field)) {
        0 -> false
        1 -> true
        else -> throw IllegalArgumentException("$field must be encoded as 0 or 1, got $value")
    }

    fun readInt(field: String): Int {
        val value = readUInt(field)
        require(value <= Int.MAX_VALUE.toLong()) { "$field exceeds the Kotlin Int range" }
        return value.toInt()
    }

    fun readUInt(field: String): Long {
        require(buffer.remaining() >= Int.SIZE_BYTES) {
            "live analysis payload ended before $field"
        }
        return buffer.int.toLong() and 0xffff_ffffL
    }

    fun readULong(field: String): ULong {
        require(buffer.remaining() >= Long.SIZE_BYTES) {
            "live analysis payload ended before $field"
        }
        return buffer.long.toULong()
    }

    fun readDouble(field: String): Double {
        require(buffer.remaining() >= Double.SIZE_BYTES) {
            "live analysis payload ended before $field"
        }
        return buffer.double
    }

    fun readString(field: String): String {
        val length = readInt("$field length")
        require(length <= buffer.remaining()) {
            "$field length $length exceeds the remaining payload bytes"
        }
        return ByteArray(length).also(buffer::get).toString(Charsets.UTF_8)
    }

    fun readOptionalDouble(field: String): Double? = when (readByte("$field presence")) {
        0 -> null
        1 -> readDouble(field)
        else -> throw IllegalArgumentException("$field presence flag is invalid")
    }

    fun readOptionalULong(field: String): ULong? = when (readByte("$field presence")) {
        0 -> null
        1 -> readULong(field)
        else -> throw IllegalArgumentException("$field presence flag is invalid")
    }

    fun requireExhausted() {
        require(!buffer.hasRemaining()) {
            "live analysis payload has ${buffer.remaining()} unexpected trailing bytes"
        }
    }
}

private fun decodeResult(bytes: ByteArray): PageAnalysisResult {
    val reader = LiveAnalysisCodecReader(bytes)
    reader.requireHeader("A2DR")
    val width = reader.readUInt("width")
    val height = reader.readUInt("height")
    val sourceRotationDegrees = reader.readInt("source rotation")
    val resolvedOrientationDegrees = reader.readInt("resolved orientation")
    val markerCount = reader.readInt("marker count")
    val markers =
        List(markerCount) { index ->
            val prefix = "marker[$index]"
            AnalyzedPageMarker(
                role = reader.readString("$prefix role"),
                family = reader.readString("$prefix family"),
                id = reader.readUInt("$prefix id"),
                hammingErrors = reader.readUInt("$prefix hamming errors"),
                decisionMargin = reader.readDouble("$prefix decision margin"),
                center =
                    AnalyzedPagePoint(
                        x = reader.readDouble("$prefix center x"),
                        y = reader.readDouble("$prefix center y"),
                    ),
                corners =
                    List(reader.readInt("$prefix corner count")) { cornerIndex ->
                        AnalyzedPagePoint(
                            x = reader.readDouble("$prefix corner[$cornerIndex] x"),
                            y = reader.readDouble("$prefix corner[$cornerIndex] y"),
                        )
                    },
            )
        }
    val unexpectedTagIds =
        List(reader.readInt("unexpected tag count")) { index ->
            reader.readUInt("unexpected tag[$index]")
        }
    val quality =
        AnalyzedPageQuality(
            focusLaplacianVariance = reader.readOptionalDouble("focus Laplacian variance"),
            focusInteriorSampleCount = reader.readOptionalULong("focus interior sample count"),
            meanLuminance = reader.readDouble("mean luminance"),
            luminanceStandardDeviation = reader.readDouble("luminance standard deviation"),
            darkFraction = reader.readDouble("dark fraction"),
            highlightFraction = reader.readDouble("highlight fraction"),
            maxTileHighlightFraction = reader.readDouble("maximum tile highlight fraction"),
            populatedTileCount = reader.readUInt("populated tile count"),
        )
    reader.requireExhausted()
    return EncodedPageAnalysisResult(
        width = width,
        height = height,
        sourceRotationDegrees = sourceRotationDegrees,
        resolvedOrientationDegrees = resolvedOrientationDegrees,
        markers = markers,
        unexpectedTagIds = unexpectedTagIds,
        quality = quality,
    )
}

private fun decodeError(bytes: ByteArray): LivePageAnalysisErrorDetails {
    val reader = LiveAnalysisCodecReader(bytes)
    reader.requireHeader("A2DE")
    val details =
        LivePageAnalysisErrorDetails(
            code = reader.readString("error code"),
            category = reader.readString("error category"),
            severity = reader.readString("error severity"),
            userMessageKey = reader.readString("user message key"),
            developerMessage = reader.readString("developer message"),
            correlationId = reader.readString("correlation id"),
            retryable = reader.readBoolean("retryable"),
        )
    reader.requireExhausted()
    return details
}

private fun Long.checkedULongBits(field: String): Long {
    require(this >= 0L) { "$field must not be negative" }
    return this
}

private fun Int.checkedUIntBits(field: String): Int {
    require(this >= 0) { "$field must not be negative" }
    return this
}

private fun Long.checkedUIntBits(field: String): Int {
    require(this in 0L..UInt.MAX_VALUE.toLong()) {
        "$field must fit an unsigned 32-bit integer"
    }
    return toUInt().toInt()
}