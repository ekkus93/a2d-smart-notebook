package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrRegionOverlayTest {
    @Test
    fun noPersistedRegionsKeepsOverlayDisabled() {
        val state =
            OcrRegionOverlayState.fromPersisted(
                LoadedAndroidOcrOutput(
                    scanId = "scan-1",
                    latestRun = loadedRun(textRegions = emptyList()),
                ),
            )

        assertFalse(state.enabled)
        assertTrue(state.renderableRegions.isEmpty())
        assertNull(state.coordinateFrame)
    }

    @Test
    fun persistedDetectedRegionsPreserveTextConfidenceAndPolygon() {
        val region = loadedRegion()

        val state =
            OcrRegionOverlayState.fromPersisted(
                LoadedAndroidOcrOutput(
                    scanId = "scan-1",
                    latestRun = loadedRun(textRegions = listOf(region)),
                ),
            )

        assertTrue(state.enabled)
        assertEquals("text-region-1", state.renderableRegions.single().textRegionId)
        assertEquals("stored text", state.renderableRegions.single().text)
        assertEquals(0.91f, state.renderableRegions.single().confidence)
        assertEquals(region.polygon, state.renderableRegions.single().polygon)
        assertEquals(120f, state.coordinateFrame?.width)
        assertEquals(40f, state.coordinateFrame?.height)
    }

    @Test
    fun unavailableRunCannotExposeTextRegionOverlay() {
        val state =
            OcrRegionOverlayState.fromPersisted(
                LoadedAndroidOcrOutput(
                    scanId = "scan-1",
                    latestRun =
                        loadedRun(
                            status = OcrRunStatus.Unavailable,
                            textRegions = listOf(loadedRegion()),
                        ),
                ),
            )

        assertFalse(state.enabled)
        assertTrue(state.regions.isEmpty())
    }

    @Test
    fun selectionUsesStoredPolygonGeometry() {
        val state = OcrRegionOverlayState(regions = listOf(loadedRegion().toOverlayRegion()))

        assertEquals("text-region-1", state.regionAt(sourceX = 60f, sourceY = 20f)?.textRegionId)
        assertNull(state.regionAt(sourceX = 160f, sourceY = 20f))
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
            completedAtMs = 10,
            warnings = emptyList(),
            textRegionCount = textRegions.size,
            textRegions = textRegions,
        )

    private fun loadedRegion(): LoadedAndroidOcrTextRegion =
        LoadedAndroidOcrTextRegion(
            textRegionId = "text-region-1",
            ocrRunId = "ocr-run-1",
            polygon =
                listOf(
                    OcrTextPoint(0f, 0f),
                    OcrTextPoint(120f, 0f),
                    OcrTextPoint(120f, 40f),
                    OcrTextPoint(0f, 40f),
                ),
            text = "stored text",
            confidence = 0.91f,
            createdAtMs = 10,
        )

    private fun LoadedAndroidOcrTextRegion.toOverlayRegion(): OcrRegionOverlayRegion =
        OcrRegionOverlayRegion(
            textRegionId = textRegionId,
            polygon = polygon,
            text = text,
            confidence = confidence,
        )
}
