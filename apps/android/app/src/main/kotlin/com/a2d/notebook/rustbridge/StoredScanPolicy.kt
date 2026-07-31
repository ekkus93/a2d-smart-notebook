package com.a2d.notebook.rustbridge

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import java.nio.ByteBuffer
import java.nio.ByteOrder
import uniffi.a2d_ffi.A2dClient

private const val STORED_POLICY_STATUS_SUCCESS = 0
private const val STORED_POLICY_STATUS_ERROR = 1
private const val STORED_POLICY_STATUS_PANIC = 2
private const val MAX_STORED_POLICY_BYTES = 64L * 1024
private const val MAX_STORED_POLICY_ERROR_BYTES = 64L * 1024

data class StoredScanPolicy(
    val layoutId: String,
    val physicalWidthMm: Double,
    val physicalHeightMm: Double,
    val markerFamily: String,
    val declaredMarkerFamily: String?,
    val markerIds: PageMarkerIds,
    val correctedWidth: Int,
    val correctedHeight: Int,
    val layoutVersion: String,
    val processingPolicyVersion: Int,
    val pipelineVersion: Int,
    val maximumEncodedBytes: Long,
    val maximumDecodedPixels: Long,
    val maximumDecodedBytes: Long,
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
) {
    val liveAnalysisPolicy: LivePageAnalysisPolicy
        get() =
            LivePageAnalysisPolicy(
                maxPixels = maximumDecodedPixels,
                detectorThreadCount = detectorThreadCount,
                detectorQuadDecimate = detectorQuadDecimate,
                detectorQuadSigma = detectorQuadSigma,
                detectorRefineEdges = detectorRefineEdges,
                detectorDecodeSharpening = detectorDecodeSharpening,
                detectorBitsCorrected = detectorBitsCorrected,
                darkLuminanceCutoff = darkLuminanceCutoff,
                highlightLuminanceCutoff = highlightLuminanceCutoff,
                qualityTileColumns = qualityTileColumns,
                qualityTileRows = qualityTileRows,
                markerIds = markerIds,
            )
}

class StoredScanPolicyException(
    val details: LivePageAnalysisErrorDetails,
    val nativePanic: Boolean,
) : Exception(
    "${details.code} [${details.correlationId}]: ${details.developerMessage}",
)

/**
 * Resolves the canonical stored page/design policy through the explicit Rust C ABI.
 *
 * This call is synchronous and opens the same application-private library path for a bounded policy
 * read. Call it from a background dispatcher. Full-resolution preview and registration resolve the
 * policy again in Rust and do not trust this presentation-side copy as authority.
 */
fun A2dClient.resolveStoredScanPolicy(pageId: String): StoredScanPolicy {
    require(pageId.isNotBlank()) { "pageId must not be blank" }
    val libraryPathBytes = libraryPath().toByteArray(Charsets.UTF_8)
    val pageIdBytes = pageId.toByteArray(Charsets.UTF_8)
    val status = StoredScanPolicyStatus()
    val output =
        storedScanPolicyNativeLibrary.a2d_resolve_stored_scan_policy_v1(
            libraryPathBytes = libraryPathBytes,
            libraryPathLen = libraryPathBytes.size.toLong(),
            pageIdBytes = pageIdBytes,
            pageIdLen = pageIdBytes.size.toLong(),
            status = status,
        )
    status.read()
    output.read()
    status.error.read()

    try {
        return when (status.code) {
            STORED_POLICY_STATUS_SUCCESS -> {
                require(status.error.data == null && status.error.len == 0L) {
                    "stored scan policy reported success with an error buffer"
                }
                decodeStoredScanPolicy(
                    readStoredPolicyBuffer(
                        output,
                        description = "stored scan policy",
                        maximumBytes = MAX_STORED_POLICY_BYTES,
                    ),
                )
            }

            STORED_POLICY_STATUS_ERROR,
            STORED_POLICY_STATUS_PANIC,
            -> {
                require(output.data == null && output.len == 0L) {
                    "stored scan policy failure returned an unexpected result buffer"
                }
                throw StoredScanPolicyException(
                    details =
                        decodeStoredPolicyError(
                            readStoredPolicyBuffer(
                                status.error,
                                description = "stored scan policy error",
                                maximumBytes = MAX_STORED_POLICY_ERROR_BYTES,
                            ),
                        ),
                    nativePanic = status.code == STORED_POLICY_STATUS_PANIC,
                )
            }

            else -> throw IllegalStateException(
                "stored scan policy returned unknown status code ${status.code}",
            )
        }
    } finally {
        freeStoredPolicyBuffer(output)
        freeStoredPolicyBuffer(status.error)
    }
}

private fun decodeStoredScanPolicy(bytes: ByteArray): StoredScanPolicy {
    val reader = StoredPolicyCodecReader(bytes)
    reader.requireHeader("A2DS")
    val policy =
        StoredScanPolicy(
            layoutId = reader.readString("layout ID"),
            physicalWidthMm = reader.readDouble("physical width"),
            physicalHeightMm = reader.readDouble("physical height"),
            markerFamily = reader.readString("marker family"),
            declaredMarkerFamily = reader.readOptionalString("declared marker family"),
            markerIds =
                PageMarkerIds(
                    topLeft = reader.readInt("top-left marker ID").toLong(),
                    topRight = reader.readInt("top-right marker ID").toLong(),
                    bottomRight = reader.readInt("bottom-right marker ID").toLong(),
                    bottomLeft = reader.readInt("bottom-left marker ID").toLong(),
                ),
            correctedWidth = reader.readInt("corrected width"),
            correctedHeight = reader.readInt("corrected height"),
            layoutVersion = reader.readString("layout version"),
            processingPolicyVersion = reader.readInt("processing policy version"),
            pipelineVersion = reader.readInt("pipeline version"),
            maximumEncodedBytes = reader.readLong("maximum encoded bytes"),
            maximumDecodedPixels = reader.readLong("maximum decoded pixels"),
            maximumDecodedBytes = reader.readLong("maximum decoded bytes"),
            detectorThreadCount = reader.readInt("detector thread count"),
            detectorQuadDecimate = reader.readDouble("detector quad decimate"),
            detectorQuadSigma = reader.readDouble("detector quad sigma"),
            detectorRefineEdges = reader.readBoolean("detector refine edges"),
            detectorDecodeSharpening = reader.readDouble("detector decode sharpening"),
            detectorBitsCorrected = reader.readInt("detector bits corrected"),
            darkLuminanceCutoff = reader.readInt("dark luminance cutoff"),
            highlightLuminanceCutoff = reader.readInt("highlight luminance cutoff"),
            qualityTileColumns = reader.readInt("quality tile columns"),
            qualityTileRows = reader.readInt("quality tile rows"),
        )
    reader.requireExhausted()

    require(policy.layoutId.isNotBlank())
    require(policy.markerFamily.isNotBlank())
    require(policy.correctedWidth > 1 && policy.correctedHeight > 1)
    require(policy.processingPolicyVersion > 0)
    require(policy.pipelineVersion > 0)
    require(policy.maximumEncodedBytes > 0)
    require(policy.maximumDecodedPixels > 0)
    require(policy.maximumDecodedBytes > 0)
    return policy
}

private fun decodeStoredPolicyError(bytes: ByteArray): LivePageAnalysisErrorDetails {
    val reader = StoredPolicyCodecReader(bytes)
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

private class StoredPolicyCodecReader(bytes: ByteArray) {
    private val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)

    fun requireHeader(expectedMagic: String) {
        require(buffer.remaining() >= 8) { "stored policy payload is shorter than its header" }
        val magic = ByteArray(4).also(buffer::get).toString(Charsets.US_ASCII)
        require(magic == expectedMagic) {
            "stored policy payload magic was $magic instead of $expectedMagic"
        }
        require(readInt("codec version") == 1) { "unsupported stored policy codec version" }
    }

    fun readBoolean(field: String): Boolean =
        when (val value = readByte(field)) {
            0 -> false
            1 -> true
            else -> throw IllegalArgumentException("$field must be encoded as 0 or 1, got $value")
        }

    fun readByte(field: String): Int {
        require(buffer.remaining() >= 1) { "stored policy payload ended before $field" }
        return buffer.get().toInt() and 0xff
    }

    fun readInt(field: String): Int {
        require(buffer.remaining() >= Int.SIZE_BYTES) { "stored policy payload ended before $field" }
        val value = buffer.int.toLong() and 0xffff_ffffL
        require(value <= Int.MAX_VALUE.toLong()) { "$field exceeds the Kotlin Int range" }
        return value.toInt()
    }

    fun readLong(field: String): Long {
        require(buffer.remaining() >= Long.SIZE_BYTES) { "stored policy payload ended before $field" }
        val value = buffer.long.toULong()
        require(value <= Long.MAX_VALUE.toULong()) { "$field exceeds the Kotlin Long range" }
        return value.toLong()
    }

    fun readDouble(field: String): Double {
        require(buffer.remaining() >= Double.SIZE_BYTES) { "stored policy payload ended before $field" }
        return buffer.double
    }

    fun readString(field: String): String {
        val length = readInt("$field length")
        require(length <= buffer.remaining()) { "$field length exceeds the remaining payload" }
        return ByteArray(length).also(buffer::get).toString(Charsets.UTF_8)
    }

    fun readOptionalString(field: String): String? =
        when (readByte("$field presence")) {
            0 -> null
            1 -> readString(field)
            else -> throw IllegalArgumentException("$field presence flag is invalid")
        }

    fun requireExhausted() {
        require(!buffer.hasRemaining()) {
            "stored policy payload has ${buffer.remaining()} unexpected trailing bytes"
        }
    }
}

private fun readStoredPolicyBuffer(
    buffer: StoredScanPolicyBuffer,
    description: String,
    maximumBytes: Long,
): ByteArray {
    require(buffer.len > 0L) { "$description buffer is empty" }
    require(buffer.capacity >= buffer.len) { "$description buffer length exceeds capacity" }
    require(buffer.len <= maximumBytes) { "$description buffer exceeds $maximumBytes bytes" }
    val pointer = requireNotNull(buffer.data) { "$description buffer pointer is null" }
    return pointer.getByteArray(0L, Math.toIntExact(buffer.len))
}

private fun freeStoredPolicyBuffer(buffer: StoredScanPolicyBuffer.ByValue) {
    if (buffer.data != null) {
        storedScanPolicyNativeLibrary.a2d_preview_buffer_free(buffer)
    }
}

@Structure.FieldOrder("capacity", "len", "data")
private open class StoredScanPolicyBuffer : Structure() {
    @JvmField var capacity: Long = 0L
    @JvmField var len: Long = 0L
    @JvmField var data: Pointer? = null

    class ByValue : StoredScanPolicyBuffer(), Structure.ByValue
}

@Structure.FieldOrder("code", "error")
private open class StoredScanPolicyStatus : Structure() {
    @JvmField var code: Int = 0
    @JvmField var error: StoredScanPolicyBuffer.ByValue = StoredScanPolicyBuffer.ByValue()
}

private interface StoredScanPolicyNativeLibrary : Library {
    fun a2d_resolve_stored_scan_policy_v1(
        libraryPathBytes: ByteArray,
        libraryPathLen: Long,
        pageIdBytes: ByteArray,
        pageIdLen: Long,
        status: StoredScanPolicyStatus,
    ): StoredScanPolicyBuffer.ByValue

    fun a2d_preview_buffer_free(buffer: StoredScanPolicyBuffer.ByValue)
}

private val storedScanPolicyNativeLibrary: StoredScanPolicyNativeLibrary by lazy(
    LazyThreadSafetyMode.SYNCHRONIZED,
) {
    Native.load("a2d_ffi", StoredScanPolicyNativeLibrary::class.java)
}
