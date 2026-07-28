package com.a2d.notebook.rustbridge

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import java.nio.ByteBuffer
import java.nio.ByteOrder
import uniffi.a2d_ffi.A2dClient

private const val PREVIEW_STATUS_SUCCESS = 0
private const val PREVIEW_STATUS_ERROR = 1
private const val PREVIEW_STATUS_PANIC = 2
private const val PREVIEW_STATUS_CANCELLED = 3
private const val MAX_PREVIEW_METADATA_BYTES = 1L * 1024 * 1024
private const val MAX_PREVIEW_ERROR_BYTES = 64L * 1024

class PagePreviewProcessingException(
    val details: LivePageAnalysisErrorDetails,
    val nativePanic: Boolean,
) : Exception(
    "${details.code} [${details.correlationId}]: ${details.developerMessage}",
)

/**
 * Opaque Rust cancellation token with explicit borrow tracking.
 *
 * [close] cancels immediately but cannot free the native allocation until every synchronous JNA
 * call that borrowed the pointer has returned. This makes ViewModel destruction and navigation
 * cancellation safe even when Rust is in the middle of decoding or rectifying a full-resolution
 * capture.
 */
class PagePreviewCancellation : AutoCloseable {
    private val lock = Any()
    private var handle: Pointer? =
        requireNotNull(previewNativeLibrary.a2d_preview_cancellation_new()) {
            "Rust preview cancellation allocation returned null"
        }
    private var activeBorrows = 0
    private var closeRequested = false

    fun cancel() {
        synchronized(lock) {
            handle?.let(previewNativeLibrary::a2d_preview_cancellation_cancel)
        }
    }

    internal fun <T> withPointer(block: (Pointer) -> T): T {
        val pointer =
            synchronized(lock) {
                check(!closeRequested) { "preview cancellation has already been closed" }
                activeBorrows = Math.incrementExact(activeBorrows)
                requireNotNull(handle) { "preview cancellation native handle is unavailable" }
            }
        try {
            return block(pointer)
        } finally {
            val released =
                synchronized(lock) {
                    check(activeBorrows > 0) { "preview cancellation borrow count underflow" }
                    activeBorrows--
                    if (closeRequested && activeBorrows == 0) {
                        handle.also { handle = null }
                    } else {
                        null
                    }
                }
            released?.let(previewNativeLibrary::a2d_preview_cancellation_free)
        }
    }

    override fun close() {
        val released =
            synchronized(lock) {
                if (closeRequested) return
                closeRequested = true
                handle?.let(previewNativeLibrary::a2d_preview_cancellation_cancel)
                if (activeBorrows == 0) {
                    handle.also { handle = null }
                } else {
                    null
                }
            }
        released?.let(previewNativeLibrary::a2d_preview_cancellation_free)
    }
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

/**
 * Synchronous full-resolution bridge. The caller must invoke this off the Android main thread.
 *
 * JNA copies the encoded capture once into the native call. Rust owns all decoding, marker analysis,
 * rectification, enhancement, resource limits, and result assembly. The returned native allocation
 * is copied once into Kotlin and released in `finally`; no native pointer escapes this function.
 */
@Suppress("UnusedReceiverParameter", "LongParameterList")
fun A2dClient.processPagePreview(
    request: PagePreviewProcessingRequest,
    cancellation: PagePreviewCancellation,
): PagePreviewProcessingOutcome {
    validateRequest(request)
    return cancellation.withPointer { cancellationPointer ->
        val status = PreviewProcessingStatus()
        val output =
            previewNativeLibrary.a2d_process_encoded_page_preview(
                bytes = request.encodedBytes,
                bytesLen = request.encodedBytes.size.toLong(),
                formatCode = if (request.format == EncodedPageFormat.JPEG) 0 else 1,
                rotationDegrees = request.rotation.degrees,
                maxEncodedBytes = request.maximumEncodedBytes,
                maxPixels = request.maximumPixels,
                maxDecodedBytes = request.maximumDecodedBytes,
                detectorThreadCount = request.analysisPolicy.detectorThreadCount.checkedUIntBits(
                    "detectorThreadCount",
                ),
                detectorQuadDecimate = request.analysisPolicy.detectorQuadDecimate,
                detectorQuadSigma = request.analysisPolicy.detectorQuadSigma,
                detectorRefineEdges =
                    (if (request.analysisPolicy.detectorRefineEdges) 1 else 0).toByte(),
                detectorDecodeSharpening = request.analysisPolicy.detectorDecodeSharpening,
                detectorBitsCorrected = request.analysisPolicy.detectorBitsCorrected.checkedUIntBits(
                    "detectorBitsCorrected",
                ),
                darkLuminanceCutoff = request.analysisPolicy.darkLuminanceCutoff.checkedUIntBits(
                    "darkLuminanceCutoff",
                ),
                highlightLuminanceCutoff =
                    request.analysisPolicy.highlightLuminanceCutoff.checkedUIntBits(
                        "highlightLuminanceCutoff",
                    ),
                qualityTileColumns = request.analysisPolicy.qualityTileColumns.checkedUIntBits(
                    "qualityTileColumns",
                ),
                qualityTileRows = request.analysisPolicy.qualityTileRows.checkedUIntBits(
                    "qualityTileRows",
                ),
                topLeftTagId =
                    request.analysisPolicy.markerIds.topLeft.checkedUIntBits("topLeftTagId"),
                topRightTagId =
                    request.analysisPolicy.markerIds.topRight.checkedUIntBits("topRightTagId"),
                bottomRightTagId =
                    request.analysisPolicy.markerIds.bottomRight.checkedUIntBits("bottomRightTagId"),
                bottomLeftTagId =
                    request.analysisPolicy.markerIds.bottomLeft.checkedUIntBits("bottomLeftTagId"),
                correctedWidth = request.correctedWidth.checkedUIntBits("correctedWidth"),
                correctedHeight = request.correctedHeight.checkedUIntBits("correctedHeight"),
                rectificationMaxOutputPixels = request.rectificationMaximumOutputPixels,
                rectificationMaxOutputBytes = request.rectificationMaximumOutputBytes,
                pipelineVersion = request.pipelineVersion.checkedUIntBits("pipelineVersion"),
                contrastLowPercentilePerMillion =
                    request.contrastLowPercentilePerMillion.checkedUIntBits(
                        "contrastLowPercentilePerMillion",
                    ),
                contrastHighPercentilePerMillion =
                    request.contrastHighPercentilePerMillion.checkedUIntBits(
                        "contrastHighPercentilePerMillion",
                    ),
                contrastMaximumGain = request.contrastMaximumGain,
                thumbnailMaxWidth =
                    request.thumbnailMaximumWidth.checkedUIntBits("thumbnailMaximumWidth"),
                thumbnailMaxHeight =
                    request.thumbnailMaximumHeight.checkedUIntBits("thumbnailMaximumHeight"),
                derivedMaxPixelsPerImage = request.derivedMaximumPixelsPerImage,
                derivedMaxBytesPerImage = request.derivedMaximumBytesPerImage,
                derivedMaxTotalOutputBytes = request.derivedMaximumTotalOutputBytes,
                derivedMaxWorkingBytes = request.derivedMaximumWorkingBytes,
                cancellation = cancellationPointer,
                status = status,
            )
        status.read()
        output.read()
        status.error.read()

        try {
            when (status.code) {
                PREVIEW_STATUS_SUCCESS -> {
                    require(status.error.data == null && status.error.len == 0L) {
                        "preview processing reported success with an error buffer"
                    }
                    val maximumPayload =
                        Math.addExact(
                            request.derivedMaximumTotalOutputBytes,
                            MAX_PREVIEW_METADATA_BYTES,
                        )
                    PagePreviewProcessingOutcome.Completed(
                        decodePreviewResult(
                            readRequiredBuffer(output, "preview result", maximumPayload),
                        ),
                    )
                }

                PREVIEW_STATUS_CANCELLED -> {
                    require(output.data == null && output.len == 0L) {
                        "cancelled preview processing returned a result buffer"
                    }
                    require(status.error.data == null && status.error.len == 0L) {
                        "cancelled preview processing returned an error buffer"
                    }
                    PagePreviewProcessingOutcome.Cancelled
                }

                PREVIEW_STATUS_ERROR,
                PREVIEW_STATUS_PANIC,
                -> {
                    require(output.data == null && output.len == 0L) {
                        "failed preview processing returned an unexpected result buffer"
                    }
                    throw PagePreviewProcessingException(
                        details =
                            decodePreviewError(
                                readRequiredBuffer(
                                    status.error,
                                    "preview error",
                                    MAX_PREVIEW_ERROR_BYTES,
                                ),
                            ),
                        nativePanic = status.code == PREVIEW_STATUS_PANIC,
                    )
                }

                else -> throw IllegalStateException(
                    "preview processing returned unknown status code ${status.code}",
                )
            }
        } finally {
            freeIfOwned(output)
            freeIfOwned(status.error)
        }
    }
}

private fun validateRequest(request: PagePreviewProcessingRequest) {
    require(request.encodedBytes.isNotEmpty()) { "encoded capture must not be empty" }
    require(request.encodedBytes.size.toLong() <= request.maximumEncodedBytes) {
        "encoded capture exceeds maximumEncodedBytes"
    }
    require(request.maximumEncodedBytes > 0)
    require(request.maximumPixels > 0)
    require(request.maximumDecodedBytes > 0)
    require(request.correctedWidth > 1 && request.correctedHeight > 1)
    require(request.rectificationMaximumOutputPixels > 0)
    require(request.rectificationMaximumOutputBytes > 0)
    require(request.pipelineVersion > 0)
    require(request.contrastLowPercentilePerMillion in 0..1_000_000)
    require(request.contrastHighPercentilePerMillion in 0..1_000_000)
    require(request.contrastLowPercentilePerMillion < request.contrastHighPercentilePerMillion)
    require(request.contrastMaximumGain.isFinite() && request.contrastMaximumGain >= 1.0)
    require(request.thumbnailMaximumWidth > 0 && request.thumbnailMaximumHeight > 0)
    require(request.derivedMaximumPixelsPerImage > 0)
    require(request.derivedMaximumBytesPerImage > 0)
    require(request.derivedMaximumTotalOutputBytes > 0)
    require(request.derivedMaximumWorkingBytes >= request.derivedMaximumTotalOutputBytes)
}

private fun readRequiredBuffer(
    buffer: PreviewProcessingBuffer,
    description: String,
    maximumBytes: Long,
): ByteArray {
    require(maximumBytes > 0L) { "$description maximum byte count must be positive" }
    require(buffer.len > 0L) { "$description buffer is empty" }
    require(buffer.capacity >= buffer.len) { "$description buffer length exceeds capacity" }
    require(buffer.len <= maximumBytes) {
        "$description buffer has ${buffer.len} bytes, limit is $maximumBytes"
    }
    val pointer = requireNotNull(buffer.data) { "$description buffer pointer is null" }
    return pointer.getByteArray(0L, Math.toIntExact(buffer.len))
}

private fun freeIfOwned(buffer: PreviewProcessingBuffer.ByValue) {
    if (buffer.data != null) {
        previewNativeLibrary.a2d_preview_buffer_free(buffer)
    }
}

private class PreviewCodecReader(bytes: ByteArray) {
    private val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)

    fun requireHeader(expectedMagic: String) {
        require(buffer.remaining() >= 8) { "preview payload is shorter than its header" }
        val magic = ByteArray(4).also(buffer::get).toString(Charsets.US_ASCII)
        require(magic == expectedMagic) {
            "preview payload magic was $magic instead of $expectedMagic"
        }
        val version = readUInt("codec version")
        require(version == 1L) { "unsupported preview codec version $version" }
    }

    fun readByte(field: String): Int {
        require(buffer.remaining() >= 1) { "preview payload ended before $field" }
        return buffer.get().toInt() and 0xff
    }

    fun readBoolean(field: String): Boolean =
        when (val value = readByte(field)) {
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
        require(buffer.remaining() >= Int.SIZE_BYTES) { "preview payload ended before $field" }
        return buffer.int.toLong() and 0xffff_ffffL
    }

    fun readULong(field: String): ULong {
        require(buffer.remaining() >= Long.SIZE_BYTES) { "preview payload ended before $field" }
        return buffer.long.toULong()
    }

    fun readDouble(field: String): Double {
        require(buffer.remaining() >= Double.SIZE_BYTES) { "preview payload ended before $field" }
        return buffer.double
    }

    fun readString(field: String): String {
        val bytes = readBytes(field, buffer.remaining())
        return bytes.toString(Charsets.UTF_8)
    }

    fun readBytes(field: String, maximumBytes: Int): ByteArray {
        val length = readInt("$field length")
        require(length <= maximumBytes) { "$field length $length exceeds limit $maximumBytes" }
        require(length <= buffer.remaining()) {
            "$field length $length exceeds the remaining payload bytes"
        }
        return ByteArray(length).also(buffer::get)
    }

    fun readOptionalDouble(field: String): Double? =
        when (readByte("$field presence")) {
            0 -> null
            1 -> readDouble(field)
            else -> throw IllegalArgumentException("$field presence flag is invalid")
        }

    fun readOptionalULong(field: String): ULong? =
        when (readByte("$field presence")) {
            0 -> null
            1 -> readULong(field)
            else -> throw IllegalArgumentException("$field presence flag is invalid")
        }

    fun requireExhausted() {
        require(!buffer.hasRemaining()) {
            "preview payload has ${buffer.remaining()} unexpected trailing bytes"
        }
    }
}

private fun decodePreviewResult(bytes: ByteArray): ProcessedPagePreview {
    val reader = PreviewCodecReader(bytes)
    reader.requireHeader("A2DP")
    val pipelineVersion = reader.readInt("pipeline version")
    require(pipelineVersion > 0) { "preview pipeline version must be positive" }
    val matrix =
        List(reader.readInt("matrix element count")) { index ->
            reader.readDouble("matrix[$index]")
        }
    require(matrix.size == 9) { "source-to-corrected matrix must contain 9 values" }

    val analysis = decodeAnalysis(reader)
    val corrected = decodeRgbImage(reader, "corrected")
    val thumbnail = decodeRgbImage(reader, "thumbnail")
    reader.requireExhausted()
    return ProcessedPagePreview(
        analysis = analysis,
        corrected = corrected,
        thumbnail = thumbnail,
        pipelineVersion = pipelineVersion,
        sourceToCorrectedMatrix = matrix,
    )
}

private fun decodeAnalysis(reader: PreviewCodecReader): EncodedPageAnalysisResult {
    val width = reader.readUInt("analysis width")
    val height = reader.readUInt("analysis height")
    val sourceRotationDegrees = reader.readInt("source rotation")
    val resolvedOrientationDegrees = reader.readInt("resolved orientation")
    val markers =
        List(reader.readInt("marker count")) { index ->
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
    return EncodedPageAnalysisResult(
        width = width,
        height = height,
        sourceRotationDegrees = sourceRotationDegrees,
        resolvedOrientationDegrees = resolvedOrientationDegrees,
        markers = markers,
        unexpectedTagIds = unexpectedTagIds,
        quality =
            AnalyzedPageQuality(
                focusLaplacianVariance = reader.readOptionalDouble("focus Laplacian variance"),
                focusInteriorSampleCount = reader.readOptionalULong("focus interior sample count"),
                meanLuminance = reader.readDouble("mean luminance"),
                luminanceStandardDeviation = reader.readDouble("luminance standard deviation"),
                darkFraction = reader.readDouble("dark fraction"),
                highlightFraction = reader.readDouble("highlight fraction"),
                maxTileHighlightFraction = reader.readDouble("maximum tile highlight fraction"),
                populatedTileCount = reader.readUInt("populated tile count"),
            ),
    )
}

private fun decodeRgbImage(
    reader: PreviewCodecReader,
    name: String,
): ProcessedRgbImage {
    val width = reader.readInt("$name width")
    val height = reader.readInt("$name height")
    require(width > 0 && height > 0) { "$name dimensions must be positive" }
    val expectedBytes = Math.multiplyExact(Math.multiplyExact(width, height), 3)
    return ProcessedRgbImage(
        width = width,
        height = height,
        bytes = reader.readBytes("$name RGB", expectedBytes),
    ).also { image ->
        require(image.bytes.size == expectedBytes) {
            "$name RGB byte count does not match its dimensions"
        }
    }
}

private fun decodePreviewError(bytes: ByteArray): LivePageAnalysisErrorDetails {
    val reader = PreviewCodecReader(bytes)
    reader.requireHeader("A2PE")
    val details =
        LivePageAnalysisErrorDetails(
            code = reader.readString("error code"),
            category = reader.readString("error category"),
            severity = reader.readString("error severity"),
            userMessageKey = reader.readString("error user message key"),
            developerMessage = reader.readString("error developer message"),
            correlationId = reader.readString("error correlation ID"),
            retryable = reader.readBoolean("error retryable"),
        )
    reader.requireExhausted()
    return details
}

private fun Int.checkedUIntBits(field: String): Int {
    require(this >= 0) { "$field must not be negative" }
    return toUInt().toInt()
}

private fun Long.checkedUIntBits(field: String): Int {
    require(this in 0L..UInt.MAX_VALUE.toLong()) { "$field must fit an unsigned 32-bit integer" }
    return toUInt().toInt()
}

@Structure.FieldOrder("capacity", "len", "data")
internal open class PreviewProcessingBuffer : Structure() {
    @JvmField var capacity: Long = 0L
    @JvmField var len: Long = 0L
    @JvmField var data: Pointer? = null

    class ByValue : PreviewProcessingBuffer(), Structure.ByValue
}

@Structure.FieldOrder("code", "error")
internal open class PreviewProcessingStatus : Structure() {
    @JvmField var code: Int = 0
    @JvmField var error: PreviewProcessingBuffer.ByValue = PreviewProcessingBuffer.ByValue()
}

@Suppress("LongParameterList")
internal interface PreviewProcessingNativeLibrary : Library {
    fun a2d_preview_cancellation_new(): Pointer?

    fun a2d_preview_cancellation_cancel(cancellation: Pointer?)

    fun a2d_preview_cancellation_free(cancellation: Pointer?)

    fun a2d_process_encoded_page_preview(
        bytes: ByteArray,
        bytesLen: Long,
        formatCode: Int,
        rotationDegrees: Int,
        maxEncodedBytes: Long,
        maxPixels: Long,
        maxDecodedBytes: Long,
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
        correctedWidth: Int,
        correctedHeight: Int,
        rectificationMaxOutputPixels: Long,
        rectificationMaxOutputBytes: Long,
        pipelineVersion: Int,
        contrastLowPercentilePerMillion: Int,
        contrastHighPercentilePerMillion: Int,
        contrastMaximumGain: Double,
        thumbnailMaxWidth: Int,
        thumbnailMaxHeight: Int,
        derivedMaxPixelsPerImage: Long,
        derivedMaxBytesPerImage: Long,
        derivedMaxTotalOutputBytes: Long,
        derivedMaxWorkingBytes: Long,
        cancellation: Pointer?,
        status: PreviewProcessingStatus,
    ): PreviewProcessingBuffer.ByValue

    fun a2d_preview_buffer_free(buffer: PreviewProcessingBuffer.ByValue)
}

private val previewNativeLibrary: PreviewProcessingNativeLibrary by lazy(
    LazyThreadSafetyMode.SYNCHRONIZED,
) {
    Native.load("a2d_ffi", PreviewProcessingNativeLibrary::class.java)
}
