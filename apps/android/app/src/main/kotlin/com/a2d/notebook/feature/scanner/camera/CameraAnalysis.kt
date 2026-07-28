package com.a2d.notebook.feature.scanner.camera

import android.graphics.ImageFormat
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import java.nio.ByteBuffer

/**
 * One tightly packed luminance frame copied from CameraX's Y plane into owned direct memory.
 *
 * Source stride metadata is retained so the native boundary can be audited without assuming that
 * camera rows or pixels were packed in the source image. [luminanceBuffer] always exposes exactly
 * `width * height` bytes with a packed row stride of [width], and its lifetime is independent of the
 * CameraX [ImageProxy]. The returned buffer is read-only and each call receives an independent
 * position/limit view over the same owned bytes.
 */
class CameraAnalysisFrame(
    val width: Int,
    val height: Int,
    val sourceRowStride: Int,
    val sourcePixelStride: Int,
    val rotationDegrees: Int,
    val timestampNanos: Long,
    val extractionDurationNanos: Long,
    val pixelBufferCopyCount: Int,
    luminance: ByteBuffer,
) {
    private val ownedLuminance: ByteBuffer

    init {
        require(width > 0 && height > 0) { "analysis frame dimensions must be positive" }
        require(sourceRowStride > 0) { "source row stride must be positive" }
        require(sourcePixelStride > 0) { "source pixel stride must be positive" }
        require(rotationDegrees in setOf(0, 90, 180, 270)) {
            "rotation must be one of 0, 90, 180, or 270 degrees"
        }
        require(extractionDurationNanos >= 0L) { "extraction duration must not be negative" }
        require(pixelBufferCopyCount == 1) {
            "CameraX luminance extraction must report exactly one owned pixel-buffer copy"
        }
        require(luminance.isDirect) { "luminance buffer must use owned direct memory" }
        require(luminance.position() == 0) { "luminance buffer must start at position zero" }
        require(luminance.remaining() == Math.multiplyExact(width, height)) {
            "luminance buffer must contain exactly width * height bytes"
        }
        ownedLuminance = luminance.asReadOnlyBuffer()
    }

    val luminanceByteCount: Int
        get() = ownedLuminance.limit()

    val packedRowStride: Int
        get() = width

    fun luminanceBuffer(): ByteBuffer = ownedLuminance.asReadOnlyBuffer()
}

sealed interface CameraAnalysisEvent {
    data class Frame(val frame: CameraAnalysisFrame) : CameraAnalysisEvent

    data class Failure(
        val message: String,
        val cause: Exception,
    ) : CameraAnalysisEvent
}

internal data class LuminanceCrop(
    val left: Int,
    val top: Int,
    val width: Int,
    val height: Int,
) {
    init {
        require(left >= 0 && top >= 0) { "crop origin must not be negative" }
        require(width > 0 && height > 0) { "crop dimensions must be positive" }
    }

    val right: Int = Math.addExact(left, width)
    val bottom: Int = Math.addExact(top, height)
}

/**
 * Copies a cropped Y plane once into tightly packed direct memory while respecting row and pixel
 * stride. The source buffer is never mutated. Invalid geometry is rejected rather than truncated,
 * clamped, or represented as an empty successful frame.
 *
 * Camera plane geometry is indexed from the start of the plane buffer. The duplicate is rewound but
 * keeps the source limit, so inaccessible bytes between limit and capacity are never read.
 */
internal fun copyLuminancePlaneToDirectBuffer(
    source: ByteBuffer,
    imageWidth: Int,
    imageHeight: Int,
    crop: LuminanceCrop,
    rowStride: Int,
    pixelStride: Int,
): ByteBuffer {
    require(imageWidth > 0 && imageHeight > 0) { "image dimensions must be positive" }
    require(rowStride > 0) { "row stride must be positive" }
    require(pixelStride > 0) { "pixel stride must be positive" }
    require(crop.right <= imageWidth && crop.bottom <= imageHeight) {
        "crop rectangle must be inside the image"
    }

    val outputSize = Math.multiplyExact(crop.width, crop.height)
    val lastRow = crop.bottom - 1
    val lastColumn = crop.right - 1
    val lastIndex = Math.addExact(
        Math.multiplyExact(lastRow.toLong(), rowStride.toLong()),
        Math.multiplyExact(lastColumn.toLong(), pixelStride.toLong()),
    )

    val input = source.duplicate().apply { rewind() }
    require(lastIndex < input.limit().toLong()) {
        "Y plane buffer is too small for its declared dimensions and strides"
    }

    val output = ByteBuffer.allocateDirect(outputSize)
    for (row in crop.top until crop.bottom) {
        val rowStart = Math.multiplyExact(row.toLong(), rowStride.toLong())
        if (pixelStride == 1) {
            val sourceOffset = Math.toIntExact(Math.addExact(rowStart, crop.left.toLong()))
            val sourceEnd = Math.addExact(sourceOffset, crop.width)
            val sourceRow = input.duplicate().apply {
                position(sourceOffset)
                limit(sourceEnd)
            }
            output.put(sourceRow)
        } else {
            for (column in crop.left until crop.right) {
                val sourceOffset = Math.toIntExact(
                    Math.addExact(
                        rowStart,
                        Math.multiplyExact(column.toLong(), pixelStride.toLong()),
                    ),
                )
                output.put(input.get(sourceOffset))
            }
        }
    }
    output.flip()
    return output.asReadOnlyBuffer()
}

/** Test-friendly heap projection of [copyLuminancePlaneToDirectBuffer]. Production analysis uses the
 * direct-buffer function and does not incur this second copy. */
internal fun copyLuminancePlane(
    source: ByteBuffer,
    imageWidth: Int,
    imageHeight: Int,
    crop: LuminanceCrop,
    rowStride: Int,
    pixelStride: Int,
): ByteArray {
    val direct = copyLuminancePlaneToDirectBuffer(
        source = source,
        imageWidth = imageWidth,
        imageHeight = imageHeight,
        crop = crop,
        rowStride = rowStride,
        pixelStride = pixelStride,
    )
    return ByteArray(direct.remaining()).also(direct::get)
}

/**
 * Always closes [resource]. Ordinary processing/close exceptions become [Result] failures; fatal
 * JVM errors are never converted into recoverable camera events and are rethrown after closure.
 */
internal fun <T : AutoCloseable, R> closeAfter(
    resource: T,
    block: (T) -> R,
): Result<R> {
    var result: Result<R>? = null
    var fatalFailure: Throwable? = null

    try {
        result = try {
            Result.success(block(resource))
        } catch (failure: Exception) {
            Result.failure(failure)
        } catch (failure: Throwable) {
            fatalFailure = failure
            throw failure
        }
    } finally {
        try {
            resource.close()
        } catch (closeFailure: Throwable) {
            val processingFailure = fatalFailure ?: result?.exceptionOrNull()
            when {
                closeFailure !is Exception -> {
                    processingFailure?.let(closeFailure::addSuppressed)
                    throw closeFailure
                }

                processingFailure != null -> {
                    processingFailure.addSuppressed(closeFailure)
                    if (fatalFailure == null) {
                        result = Result.failure(processingFailure)
                    }
                }

                else -> result = Result.failure(closeFailure)
            }
        }
    }

    return checkNotNull(result) { "camera frame processing completed without a result" }
}

/**
 * CameraX analyzer that owns no frame after [analyze] returns. Every recoverable success and failure
 * is surfaced through [onEvent], and every [ImageProxy] is closed even if validation or copying
 * fails. Fatal JVM errors propagate after the frame is closed.
 */
class CameraFrameAnalyzer(
    private val onEvent: (CameraAnalysisEvent) -> Unit,
    private val clockNanos: () -> Long = System::nanoTime,
) : ImageAnalysis.Analyzer {
    override fun analyze(image: ImageProxy) {
        val result = closeAfter(image) { proxy ->
            require(proxy.format == ImageFormat.YUV_420_888) {
                "CameraX analysis frame must use YUV_420_888"
            }
            val plane = proxy.planes.firstOrNull()
                ?: error("CameraX analysis frame has no luminance plane")
            val cropRect = proxy.cropRect
            val crop = LuminanceCrop(
                left = cropRect.left,
                top = cropRect.top,
                width = cropRect.width(),
                height = cropRect.height(),
            )
            val extractionStartedNanos = clockNanos()
            val luminance = copyLuminancePlaneToDirectBuffer(
                source = plane.buffer,
                imageWidth = proxy.width,
                imageHeight = proxy.height,
                crop = crop,
                rowStride = plane.rowStride,
                pixelStride = plane.pixelStride,
            )
            val extractionCompletedNanos = clockNanos()
            CameraAnalysisFrame(
                width = crop.width,
                height = crop.height,
                sourceRowStride = plane.rowStride,
                sourcePixelStride = plane.pixelStride,
                rotationDegrees = proxy.imageInfo.rotationDegrees,
                timestampNanos = proxy.imageInfo.timestamp,
                extractionDurationNanos = Math.subtractExact(
                    extractionCompletedNanos,
                    extractionStartedNanos,
                ),
                pixelBufferCopyCount = 1,
                luminance = luminance,
            )
        }
        result.fold(
            onSuccess = { onEvent(CameraAnalysisEvent.Frame(it)) },
            onFailure = { failure ->
                val exception = failure as? Exception
                    ?: throw failure
                onEvent(
                    CameraAnalysisEvent.Failure(
                        message = exception.message ?: "CameraX analysis failed",
                        cause = exception,
                    ),
                )
            },
        )
    }
}