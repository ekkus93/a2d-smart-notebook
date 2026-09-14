package com.a2d.notebook.feature.ocr

import kotlin.math.max

/**
 * Presentation-only projection of Rust-owned OCR text-region rows.
 *
 * Android never fabricates text regions for this overlay. The projection is enabled only when
 * persisted [LoadedAndroidOcrTextRegion] rows are returned by Rust readback for a detected OCR run.
 */
data class OcrRegionOverlayRegion(
    val textRegionId: String,
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
) {
    val isRenderable: Boolean
        get() =
            polygon.size >= 3 &&
                polygon.all { point ->
                    point.x.isFinite() && point.y.isFinite() && point.x >= 0f && point.y >= 0f
                }
}

data class OcrRegionOverlayState(
    val regions: List<OcrRegionOverlayRegion> = emptyList(),
) {
    val renderableRegions: List<OcrRegionOverlayRegion>
        get() = regions.filter(OcrRegionOverlayRegion::isRenderable)

    val enabled: Boolean
        get() = renderableRegions.isNotEmpty()

    val coordinateFrame: OcrRegionCoordinateFrame?
        get() = OcrRegionCoordinateFrame.from(renderableRegions)

    fun regionAt(sourceX: Float, sourceY: Float): OcrRegionOverlayRegion? =
        renderableRegions.asReversed().firstOrNull { region ->
            pointInPolygon(sourceX, sourceY, region.polygon)
        }

    companion object {
        fun fromPersisted(output: LoadedAndroidOcrOutput): OcrRegionOverlayState {
            val run = output.latestRun ?: return OcrRegionOverlayState()
            if (run.status != OcrRunStatus.Detected) return OcrRegionOverlayState()
            return OcrRegionOverlayState(
                regions =
                    run.textRegions.map { region ->
                        OcrRegionOverlayRegion(
                            textRegionId = region.textRegionId,
                            polygon = region.polygon,
                            text = region.text,
                            confidence = region.confidence,
                        )
                    },
            )
        }
    }
}

data class OcrRegionCoordinateFrame(
    val width: Float,
    val height: Float,
) {
    val aspectRatio: Float
        get() = width / height

    companion object {
        fun from(regions: List<OcrRegionOverlayRegion>): OcrRegionCoordinateFrame? {
            val points = regions.flatMap(OcrRegionOverlayRegion::polygon)
            if (points.isEmpty()) return null

            val width = max(1f, points.maxOf(OcrTextPoint::x))
            val height = max(1f, points.maxOf(OcrTextPoint::y))
            return OcrRegionCoordinateFrame(width = width, height = height)
        }
    }
}

private fun pointInPolygon(
    x: Float,
    y: Float,
    polygon: List<OcrTextPoint>,
): Boolean {
    if (polygon.size < 3) return false

    var inside = false
    var previousIndex = polygon.lastIndex
    polygon.indices.forEach { currentIndex ->
        val current = polygon[currentIndex]
        val previous = polygon[previousIndex]
        val intersects =
            (current.y > y) != (previous.y > y) &&
                x <
                (previous.x - current.x) * (y - current.y) /
                (previous.y - current.y) + current.x
        if (intersects) inside = !inside
        previousIndex = currentIndex
    }
    return inside
}
