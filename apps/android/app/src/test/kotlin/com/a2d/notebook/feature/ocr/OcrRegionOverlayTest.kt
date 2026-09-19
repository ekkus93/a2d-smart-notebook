package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrRegionOverlayTest {
    @Test
    fun fitTransformUsesAuthoritativeSourceDimensionsWithLetterboxOffsets() {
        val transform = OcrImageFitTransform.fit(sourceWidth = 400f, sourceHeight = 200f, viewportWidth = 300f, viewportHeight = 300f)

        assertEquals(0.75f, transform.scale, 0.0001f)
        assertEquals(0f, transform.offsetX, 0.0001f)
        assertEquals(75f, transform.offsetY, 0.0001f)
        assertEquals(OcrOverlayPoint(0f, 75f), transform.sourceToViewport(OcrOverlayPoint(0f, 0f)))
        assertEquals(OcrOverlayPoint(300f, 225f), transform.sourceToViewport(OcrOverlayPoint(400f, 200f)))
    }

    @Test
    fun fitTransformPreservesNonSquareGeometryAndMapsSelectionBackToSource() {
        val transform = OcrImageFitTransform.fit(sourceWidth = 200f, sourceHeight = 400f, viewportWidth = 400f, viewportHeight = 300f)
        val viewportPoint = transform.sourceToViewport(OcrOverlayPoint(100f, 200f))

        assertEquals(0.75f, transform.scale, 0.0001f)
        assertEquals(125f, transform.offsetX, 0.0001f)
        assertEquals(0f, transform.offsetY, 0.0001f)
        assertEquals(OcrOverlayPoint(200f, 150f), viewportPoint)
        assertEquals(OcrOverlayPoint(100f, 200f), transform.viewportToSource(viewportPoint.x, viewportPoint.y))
    }

    @Test
    fun viewportToSourceRejectsLetterboxAndOutOfImageTouches() {
        val transform = OcrImageFitTransform.fit(sourceWidth = 400f, sourceHeight = 200f, viewportWidth = 300f, viewportHeight = 300f)

        assertNull(transform.viewportToSource(150f, 20f))
        assertNull(transform.viewportToSource(150f, 260f))
        assertNull(transform.viewportToSource(-1f, 100f))
        assertEquals(OcrOverlayPoint(200f, 100f), transform.viewportToSource(150f, 150f))
    }

    @Test
    fun persistedRegionsRetainSourceGeometryAndSupportEdgeSelection() {
        val state =
            OcrRegionOverlayState.fromPersisted(
                LoadedAndroidOcrOutput(
                    scanId = "scan-1",
                    latestRun =
                        loadedRun(
                            textRegions =
                                listOf(
                                    loadedRegion(
                                        polygon =
                                            listOf(
                                                OcrTextPoint(0f, 0f),
                                                OcrTextPoint(40f, 0f),
                                                OcrTextPoint(40f, 40f),
                                                OcrTextPoint(0f, 40f),
                                            ),
                                    ),
                                ),
                        ),
                ),
                sourceWidth = 400f,
                sourceHeight = 200f,
            )

        assertEquals(400f, state.sourceWidth, 0f)
        assertEquals(200f, state.sourceHeight, 0f)
        assertEquals("text-region-1", state.regionAt(sourceX = 0f, sourceY = 0f)?.textRegionId)
        assertEquals("text-region-1", state.regionAt(sourceX = 40f, sourceY = 40f)?.textRegionId)
        assertNull(state.regionAt(sourceX = 400f, sourceY = 200f))
    }

    @Test
    fun regionHitTestingUsesPolygonNotBoundingExtrema() {
        val diamond =
            loadedRegion(
                polygon =
                    listOf(
                        OcrTextPoint(60f, 0f),
                        OcrTextPoint(120f, 60f),
                        OcrTextPoint(60f, 120f),
                        OcrTextPoint(0f, 60f),
                    ),
            )
        val state =
            OcrRegionOverlayState(
                sourceWidth = 240f,
                sourceHeight = 160f,
                regions = listOf(diamond.toOverlayRegion()),
            )

        assertEquals("text-region-1", state.regionAt(sourceX = 60f, sourceY = 20f)?.textRegionId)
        assertNull(state.regionAt(sourceX = 160f, sourceY = 20f))
        assertNull(state.regionAt(sourceX = 260f, sourceY = 20f))
    }

    private fun loadedRun(
        status: OcrRunStatus = OcrRunStatus.Detected,
        textRegions: List<LoadedAndroidOcrTextRegion>,
    ): LoadedAndroidOcrRun =
        LoadedAndroidOcrRun(
            ocrRunId = "ocr-run-1",
            scanId = "scan-1",
            inputAssetId = "asset-1",
            provider = "provider",
            providerVersion = "1",
            modelName = null,
            status = status,
            fullText = "stored text",
            unavailableReason = null,
            unavailableMessage = null,
            retryAvailable = false,
            completedAtMs = 10,
            warnings = emptyList(),
            textRegionCount = textRegions.size,
            textRegions = textRegions,
        )

    private fun loadedRegion(
        polygon: List<OcrTextPoint> =
            listOf(
                OcrTextPoint(0f, 0f),
                OcrTextPoint(20f, 0f),
                OcrTextPoint(20f, 20f),
                OcrTextPoint(0f, 20f),
            ),
    ): LoadedAndroidOcrTextRegion =
        LoadedAndroidOcrTextRegion(
            textRegionId = "text-region-1",
            ocrRunId = "ocr-run-1",
            polygon = polygon,
            text = "region",
            confidence = 0.8f,
            createdAtMs = 10,
        )
}
