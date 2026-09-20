package com.a2d.notebook.feature.ocr

import kotlin.math.min

/**
 * One authoritative content-fit mapping between OCR source pixels and the displayed image.
 *
 * The transform preserves source aspect ratio and centers the rendered image inside its viewport.
 * Polygon drawing and hit testing must share this same mapping so letterbox/pillarbox offsets can
 * never diverge between presentation and interaction.
 */
data class OcrOverlayTransform(
    val sourceWidthPx: Float,
    val sourceHeightPx: Float,
    val viewportWidthPx: Float,
    val viewportHeightPx: Float,
    val scale: Float,
    val offsetX: Float,
    val offsetY: Float,
) {
    val renderedWidthPx: Float get() = sourceWidthPx * scale
    val renderedHeightPx: Float get() = sourceHeightPx * scale

    fun sourceToViewport(point: OcrOverlayPoint): OcrOverlayPoint =
        OcrOverlayPoint(
            x = offsetX + point.x * scale,
            y = offsetY + point.y * scale,
        )

    fun viewportToSource(point: OcrOverlayPoint): OcrOverlayPoint? {
        if (!containsViewportPoint(point)) return null
        return OcrOverlayPoint(
            x = (point.x - offsetX) / scale,
            y = (point.y - offsetY) / scale,
        )
    }

    fun containsViewportPoint(point: OcrOverlayPoint): Boolean =
        point.x >= offsetX &&
            point.x <= offsetX + renderedWidthPx &&
            point.y >= offsetY &&
            point.y <= offsetY + renderedHeightPx

    companion object {
        fun contentFit(
            sourceWidthPx: Float,
            sourceHeightPx: Float,
            viewportWidthPx: Float,
            viewportHeightPx: Float,
        ): OcrOverlayTransform {
            require(sourceWidthPx.isFinite() && sourceWidthPx > 0f) { "source width must be finite and positive" }
            require(sourceHeightPx.isFinite() && sourceHeightPx > 0f) { "source height must be finite and positive" }
            require(viewportWidthPx.isFinite() && viewportWidthPx > 0f) { "viewport width must be finite and positive" }
            require(viewportHeightPx.isFinite() && viewportHeightPx > 0f) { "viewport height must be finite and positive" }
            val scale = min(viewportWidthPx / sourceWidthPx, viewportHeightPx / sourceHeightPx)
            val renderedWidth = sourceWidthPx * scale
            val renderedHeight = sourceHeightPx * scale
            return OcrOverlayTransform(
                sourceWidthPx = sourceWidthPx,
                sourceHeightPx = sourceHeightPx,
                viewportWidthPx = viewportWidthPx,
                viewportHeightPx = viewportHeightPx,
                scale = scale,
                offsetX = (viewportWidthPx - renderedWidth) / 2f,
                offsetY = (viewportHeightPx - renderedHeight) / 2f,
            )
        }
    }
}

data class OcrOverlayPoint(val x: Float, val y: Float)
