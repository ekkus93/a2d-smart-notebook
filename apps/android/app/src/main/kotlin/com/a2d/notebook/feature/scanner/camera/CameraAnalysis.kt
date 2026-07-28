package com.a2d.notebook.feature.scanner.camera

import android.graphics.ImageFormat
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import java.nio.ByteBuffer

/**
 * One tightly packed luminance frame copied from CameraX's Y plane.
 *
 * Source stride metadata is retained so the Rust/native boundary can be audited without assuming
 * that camera rows or pixels were packed in the source image. [luminance] itself is always
 * `width * height` bytes and is independent of the lifetime of the CameraX [ImageProxy].
 */
data class CameraAnalysisFrame(
    val width: Int,
    val height: Int,
    val sourceRowStride: Int,
    val sourcePixelStride: Int,
    val rotationDegrees: Int,
    val timestampNanos: Long,
    val luminance: ByteArray,
) {
    init {
        require(width > 0 && height > 0) { "analysis frame dimensions must be positive" }
        require(sourceRowStride > 0) { "source row stride must be positive" }
        require(sourcePixelStride > 0) { "source pixel stride must be positive" }
        require(rotationDegrees in setOf(0, 90, 180, 270)) {
            "rotation must be one of 0, 90, 180, or 270 degrees"
        }
        require(luminance.size == Math.multiplyExact(width, height)) {
            "luminance buffer must contain exactly width * height bytes"
        }
    }
}

sealed interface CameraAnalysisEvent {
    data class Frame(val frame: CameraAnalysisFrame) : CameraAnalysisEvent

    data class Failure(
        val message: String,
        val cause: Throwable,
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
 * Copies a cropped Y plane while respecting row and pixel stride. The source buffer is never
 * mutated. Invalid geometry is rejected rather than truncated, clamped, or represented as an empty
 * successful frame.
 */
internal fun copyLuminancePlane(
    source: ByteBuffer,
    imageWidth: Int,
    imageHeight: Int,
    crop: LuminanceCrop,
    rowStride: Int,
    pixelStride: Int,
): ByteArray {
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
    require(lastIndex < source.capacity().toLong()) {
        "Y plane buffer is too small for its declared dimensions and strides"
    }

    val input = source.duplicate().apply { clear() }
    val output = ByteArray(outputSize)
    var destination = 0

    for (row in crop.top until crop.bottom) {
        val rowStart = row.toLong() * rowStride.toLong()
        if (pixelStride == 1) {
            val sourceOffset = Math.addExact(rowStart, crop.left.toLong()).toInt()
            input.position(sourceOffset)
            input.get(output, destination, crop.width)
            destination += crop.width
        } else {
            for (column in crop.left until crop.right) {
                val sourceOffset = Math.addExact(
                    rowStart,
                    column.toLong() * pixelStride.toLong(),
                ).toInt()
                output[destination++] = input.get(sourceOffset)
            }
        }
    }
    return output
}

/** Always closes [resource], preserving a processing failure and suppressing any close failure. */
internal fun <T : AutoCloseable, R> closeAfter(
    resource: T,
    block: (T) -> R,
): Result<R> {
    val result = runCatching { block(resource) }
    return try {
        resource.close()
        result
    } catch (closeFailure: Throwable) {
        result.exceptionOrNull()?.let { original ->
            original.addSuppressed(closeFailure)
            Result.failure(original)
        } ?: Result.failure(closeFailure)
    }
}

/**
 * CameraX analyzer that owns no frame after [analyze] returns. Every success and failure is surfaced
 * through [onEvent], and every [ImageProxy] is closed even if validation or copying fails.
 */
class CameraFrameAnalyzer(
    private val onEvent: (CameraAnalysisEvent) -> Unit,
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
            CameraAnalysisFrame(
                width = crop.width,
                height = crop.height,
                sourceRowStride = plane.rowStride,
                sourcePixelStride = plane.pixelStride,
                rotationDegrees = proxy.imageInfo.rotationDegrees,
                timestampNanos = proxy.imageInfo.timestamp,
                luminance = copyLuminancePlane(
                    source = plane.buffer,
                    imageWidth = proxy.width,
                    imageHeight = proxy.height,
                    crop = crop,
                    rowStride = plane.rowStride,
                    pixelStride = plane.pixelStride,
                ),
            )
        }
        result.fold(
            onSuccess = { onEvent(CameraAnalysisEvent.Frame(it)) },
            onFailure = {
                onEvent(
                    CameraAnalysisEvent.Failure(
                        message = it.message ?: "CameraX analysis failed",
                        cause = it,
                    ),
                )
            },
        )
    }
}