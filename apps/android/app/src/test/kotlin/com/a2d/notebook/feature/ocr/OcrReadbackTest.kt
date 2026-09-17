package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrReadbackTest {
    @Test
    fun missingPersistedRunHydratesNotStartedState() {
        val output = LoadedAndroidOcrOutput(scanId = "scan-1", latestRun = null)

        val state = output.toPresentationState()

        assertEquals(OcrPresentationStatus.NotStarted, state.status)
        assertNull(state.runId)
        assertNull(state.textPreview)
        assertEquals(0, state.recognizedRegionCount)
        assertFalse(state.retryAvailable)
        assertFalse(state.cancelAvailable)
    }

    @Test
    fun detectedRunHydratesPageViewerStateWithBoundedRegionSummary() {
        val output =
            LoadedAndroidOcrOutput(
                scanId = "scan-1",
                latestRun =
                    loadedRun(
                        status = OcrRunStatus.Detected,
                        fullText = "hello notebook",
                        textRegionCount = 12,
                        textRegions = List(12) { loadedRegion("hello-$it", "text-region-$it") },
                    ),
            )

        val state = output.toPresentationState()

        assertEquals(OcrPresentationStatus.Detected, state.status)
        assertEquals("ocr-run-1", state.runId)
        assertEquals("mlkit", state.providerLabel)
        assertEquals("latin-v1", state.modelName)
        assertEquals("hello notebook", state.textPreview)
        assertEquals(12, state.recognizedRegionCount)
        assertNull(state.message)
        assertFalse(state.retryAvailable)
        assertFalse(state.cancelAvailable)
    }

    @Test
    fun detectedRunDisclosesPartialOverlayInsteadOfPresentingTruncationAsComplete() {
        val output =
            LoadedAndroidOcrOutput(
                scanId = "scan-1",
                latestRun =
                    loadedRun(
                        status = OcrRunStatus.Detected,
                        fullText = "many regions",
                        textRegionCount = 75,
                        textRegions = List(60) { loadedRegion("region-$it", "text-region-$it") },
                    ),
            )

        val run = requireNotNull(output.latestRun)
        val state = output.toPresentationState()

        assertTrue(run.regionsTruncated)
        assertEquals(75, state.recognizedRegionCount)
        assertEquals("OCR overlay is partial: loaded 60 of 75 persisted regions", state.message)
    }

    @Test
    fun noTextRunHydratesSeparatelyFromDetectedEmptyText() {
        val output =
            LoadedAndroidOcrOutput(
                scanId = "scan-1",
                latestRun =
                    loadedRun(
                        status = OcrRunStatus.NoTextDetected,
                        fullText = "",
                        textRegionCount = 0,
                        textRegions = emptyList(),
                    ),
            )

        val state = output.toPresentationState()

        assertEquals(OcrPresentationStatus.NoTextDetected, state.status)
        assertNull(state.textPreview)
        assertEquals(0, state.recognizedRegionCount)
        assertFalse(state.retryAvailable)
    }

    @Test
    fun unavailableProviderFailureHydratesRetryableUnavailableState() {
        val output =
            LoadedAndroidOcrOutput(
                scanId = "scan-1",
                latestRun =
                    loadedRun(
                        status = OcrRunStatus.Unavailable,
                        fullText = "",
                        unavailableReason = OcrUnavailableReason.ProviderFailed,
                        unavailableMessage = "provider failed before returning text",
                    ),
            )

        val state = output.toPresentationState()

        assertEquals(OcrPresentationStatus.Unavailable, state.status)
        assertEquals("provider failed", state.unavailableReason)
        assertEquals("provider failed before returning text", state.message)
        assertTrue(state.retryAvailable)
        assertEquals(0, state.recognizedRegionCount)
    }

    @Test
    fun cancelledUnavailableRunHydratesCancelledStateWithoutRetry() {
        val output =
            LoadedAndroidOcrOutput(
                scanId = "scan-1",
                latestRun =
                    loadedRun(
                        status = OcrRunStatus.Unavailable,
                        fullText = "",
                        unavailableReason = OcrUnavailableReason.Cancelled,
                        unavailableMessage = "OCR was cancelled before text was returned",
                    ),
            )

        val state = output.toPresentationState()

        assertEquals(OcrPresentationStatus.Cancelled, state.status)
        assertEquals("cancelled", state.unavailableReason)
        assertEquals("OCR was cancelled before text was returned", state.message)
        assertFalse(state.retryAvailable)
        assertFalse(state.cancelAvailable)
    }

    private fun loadedRun(
        status: OcrRunStatus,
        fullText: String,
        unavailableReason: OcrUnavailableReason? = null,
        unavailableMessage: String? = null,
        textRegionCount: Int = 0,
        textRegions: List<LoadedAndroidOcrTextRegion> = emptyList(),
    ): LoadedAndroidOcrRun =
        LoadedAndroidOcrRun(
            ocrRunId = "ocr-run-1",
            scanId = "scan-1",
            inputAssetId = "asset-1",
            provider = "mlkit",
            providerVersion = "2026.09",
            modelName = "latin-v1",
            status = status,
            fullText = fullText,
            unavailableReason = unavailableReason,
            unavailableMessage = unavailableMessage,
            completedAtMs = 300,
            warnings = emptyList(),
            textRegionCount = textRegionCount,
            textRegions = textRegions,
        )

    private fun loadedRegion(
        text: String,
        textRegionId: String = "text-region-1",
    ): LoadedAndroidOcrTextRegion =
        LoadedAndroidOcrTextRegion(
            textRegionId = textRegionId,
            ocrRunId = "ocr-run-1",
            polygon =
                listOf(
                    OcrTextPoint(x = 0.0f, y = 0.0f),
                    OcrTextPoint(x = 10.0f, y = 0.0f),
                    OcrTextPoint(x = 10.0f, y = 10.0f),
                ),
            text = text,
            confidence = 0.91f,
            createdAtMs = 300,
        )
}
