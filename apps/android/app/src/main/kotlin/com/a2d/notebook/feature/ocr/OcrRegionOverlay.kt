package com.a2d.notebook.feature.ocr

/**
 * Presentation-only projection of Rust-owned OCR text-region rows.
 *
 * Android never fabricates text regions for this overlay. The projection is enabled only when
 * persisted [LoadedAndroidOcrTextRegion] rows are returned by Rust readback for a detected OCR run
 * and an explicit source image coordinate frame is available. The overlay must never derive the
 * image frame from OCR polygon extrema because that changes the coordinate system when OCR only
 * covers part of the page.
 */
data class OcrRegionOverlayRegion(
    val textRegionId: String,
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
) {
    fun isRenderableIn(frame: OcrRegionCoordinateFrame): Boolean =
        polygon.size >= 3 &&
            polygon.all { point ->
                point.x.isFinite() &&
                    point.y.isFinite() &&
                    point.x >= 0f &&
                    point.y >= 0f &&
                    point.x <= frame.width &&
                    point.y <= frame.height
            }
}

data class OcrRegionOverlayState(
    val regions: List<OcrRegionOverlayRegion> = emptyList(),
    val sourceFrame: OcrRegionCoordinateFrame? = null,
    val completeRegionHydration: Boolean = true,
) {
    val renderableRegions: List<OcrRegionOverlayRegion>
        get() {
            val frame = sourceFrame ?: return emptyList()
            return regions.filter { region -> region.isRenderableIn(frame) }
        }

    val enabled: Boolean
        get() = sourceFrame != null && renderableRegions.isNotEmpty()

    val coordinateFrame: OcrRegionCoordinateFrame?
        get() = sourceFrame

    fun regionAt(sourceX: Float, sourceY: Float): OcrRegionOverlayRegion? {
        val frame = sourceFrame ?: return null
        if (!sourceX.isFinite() || !sourceY.isFinite()) return null
        if (sourceX < 0f || sourceY < 0f || sourceX > frame.width || sourceY > frame.height) {
            return null
        }
        return renderableRegions.asReversed().firstOrNull { region ->
            pointInPolygon(sourceX, sourceY, region.polygon)
        }
    }

    companion object {
        fun withSourceGeometry(
            sourceImageWidth: Int,
            sourceImageHeight: Int,
            regions: List<OcrRegionOverlayRegion>,
            completeRegionHydration: Boolean = true,
        ): OcrRegionOverlayState =
            OcrRegionOverlayState(
                regions = regions,
                sourceFrame = OcrRegionCoordinateFrame.fromDimensions(sourceImageWidth, sourceImageHeight),
                completeRegionHydration = completeRegionHydration,
            )

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
                sourceFrame = null,
                completeRegionHydration = run.textRegionCount == run.textRegions.size,
            )
        }
    }
}

data class OcrRegionCoordinateFrame(
    val width: Float,
    val height: Float,
) {
    init {
        require(width.isFinite() && width > 0f) { "source image width must be finite and positive" }
        require(height.isFinite() && height > 0f) { "source image height must be finite and positive" }
    }

    val aspectRatio: Float
        get() = width / height

    companion object {
        fun fromDimensions(width: Int, height: Int): OcrRegionCoordinateFrame? {
            if (width <= 0 || height <= 0) return null
            return OcrRegionCoordinateFrame(width = width.toFloat(), height = height.toFloat())
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
