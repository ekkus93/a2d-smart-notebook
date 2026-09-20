package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind

class OcrRegionOverlaySourceGeometryTest {
    @Test
    fun persistedDetectedRegionsRenderWithResolvedSourceGeometryAndImagePath() {
        val state =
            OcrRegionOverlayState.fromPersisted(
                output =
                    LoadedAndroidOcrOutput(
                        scanId = "scan-1",
                        latestRun = loadedRun(textRegions = listOf(loadedRegion())),
                    ),
                sourceGeometry = resolvedSourceGeometry(),
            )

        assertTrue(state.enabled)
        assertEquals("/library/assets/ocr/source.png", state.sourceImagePath)
        assertEquals(240f, state.coordinateFrame?.width)
        assertEquals(120f, state.coordinateFrame?.height)
        assertEquals("text-region-1", state.renderableRegions.single().textRegionId)
    }

    @Test
    fun sharedTransformRejectsTapsOutsideRenderedImageBeforeRegionHitTesting() {
        val state =
            OcrRegionOverlayState.withSourceGeometry(
                sourceImageWidth = 200,
                sourceImageHeight = 100,
                sourceImagePath = "/library/assets/ocr/source.png",
                regions = listOf(loadedRegion().toOverlayRegion()),
            )
        val transform = OcrOverlayTransform.contentFit(
            sourceWidthPx = 200f,
            sourceHeightPx = 100f,
            viewportWidthPx = 400f,
            viewportHeightPx = 400f,
        )

        val inside = requireNotNull(transform.viewportToSource(transform.sourceToViewport(OcrOverlayPoint(60f, 20f))))
        assertEquals("text-region-1", state.regionAt(inside.x, inside.y)?.textRegionId)
        assertNull(transform.viewportToSource(OcrOverlayPoint(200f, 50f)))
    }

    private fun resolvedSourceGeometry(): AndroidOcrSourceGeometry =
        AndroidOcrSourceGeometry(
            scanId = "scan-1",
            inputAssetId = "asset-1",
            inputKind = FfiOcrInputKind.ORIGINAL,
            mediaType = "image/png",
            relativePath = "assets/ocr/source.png",
            absolutePath = "/library/assets/ocr/source.png",
            byteLength = 42uL,
            widthPx = 240u,
            heightPx = 120u,
        )

    private fun loadedRun(textRegions: List<LoadedAndroidOcrTextRegion>): LoadedAndroidOcrRun =
        LoadedAndroidOcrRun(
            ocrRunId = "ocr-run-1",
            scanId = "scan-1",
            inputAssetId = "asset-1",
            provider = "provider",
            providerVersion = "1",
            modelName = null,
            status = OcrRunStatus.Detected,
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
