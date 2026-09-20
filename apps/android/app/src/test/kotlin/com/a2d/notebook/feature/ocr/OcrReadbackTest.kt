package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind

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
                        textRegions = listOf(loadedRegion("hello")),
                    ),
            )

        val state = output.toPresentationState()

        assertEquals(OcrPresentationStatus.Detected, state.status)
        assertEquals("ocr-run-1", state.runId)
        assertEquals("mlkit", state.providerLabel)
        assertEquals("latin-v1", state.modelName)
        assertEquals("hello notebook", state.textPreview)
        assertEquals(12, state.recognizedRegionCount)
        assertFalse(state.retryAvailable)
        assertFalse(state.cancelAvailable)
    }

    @Test
    fun androidPaginationHydratesMoreThanFiftyRegionsBeforeOverlayIsComplete() {
        val pages =
            mapOf(
                0u to regionPage(offset = 0u, nextOffset = 50u, hasMore = true, complete = false, range = 0 until 50),
                50u to regionPage(offset = 50u, nextOffset = null, hasMore = false, complete = true, range = 50 until 64),
            )

        val hydrated =
            AndroidOcrRegionPaginationHydrator.hydrate(
                scanId = "scan-1",
                expectedOcrRunId = "ocr-run-1",
                expectedTotalCount = 64u,
                pageSize = 50u,
            ) { offset -> pages.getValue(offset) }

        assertEquals(64, hydrated.size)
        assertEquals("text-region-000", hydrated.first().textRegionId)
        assertEquals("text-region-063", hydrated.last().textRegionId)
        assertEquals(64, hydrated.map { it.textRegionId }.toSet().size)

        val output =
            LoadedAndroidOcrOutput(
                scanId = "scan-1",
                latestRun =
                    loadedRun(
                        status = OcrRunStatus.Detected,
                        fullText = hydrated.joinToString("\n") { it.text },
                        textRegionCount = 64,
                        textRegions = hydrated,
                    ),
                sourceGeometry = resolvedSourceGeometry(),
            )
        val overlay = OcrRegionOverlayState.fromPersisted(output, resolvedSourceGeometry())
        val presentation = output.toPresentationState()

        assertTrue(overlay.enabled)
        assertEquals(64, overlay.renderableRegions.size)
        assertEquals(64, presentation.recognizedRegionCount)
    }

    @Test
    fun androidPaginationRejectsConflictingDuplicateRegionsDuringRecreationOverlap() {
        val pages =
            mapOf(
                0u to regionPage(offset = 0u, nextOffset = 2u, hasMore = true, complete = false, range = 0 until 2),
                2u to
                    regionPage(
                        offset = 2u,
                        nextOffset = null,
                        hasMore = false,
                        complete = true,
                        regions = listOf(regionPageRegion(1, text = "conflicting duplicate")),
                    ),
            )

        val error =
            expectPaginationError {
                AndroidOcrRegionPaginationHydrator.hydrate(
                    scanId = "scan-1",
                    expectedOcrRunId = "ocr-run-1",
                    expectedTotalCount = 2u,
                    pageSize = 2u,
                ) { offset -> pages.getValue(offset) }
            }

        assertEquals("Conflicting duplicate OCR region ID text-region-001", error.message)
    }

    @Test
    fun androidPaginationSurfacesLaterPageFailuresInsteadOfPublishingPartialOverlay() {
        val first = regionPage(offset = 0u, nextOffset = 2u, hasMore = true, complete = false, range = 0 until 2)

        val error =
            expectPaginationError {
                AndroidOcrRegionPaginationHydrator.hydrate(
                    scanId = "scan-1",
                    expectedOcrRunId = "ocr-run-1",
                    expectedTotalCount = 3u,
                    pageSize = 2u,
                ) { offset ->
                    if (offset == 0u) {
                        first
                    } else {
                        throw OcrRegionPaginationException("simulated later page failure")
                    }
                }
            }

        assertEquals("simulated later page failure", error.message)
    }

    @Test
    fun androidPaginationRejectsCompletenessBeforeAuthoritativeEnd() {
        val premature =
            regionPage(
                offset = 0u,
                nextOffset = null,
                hasMore = false,
                complete = true,
                range = 0 until 2,
                totalCount = 3u,
            )

        val error =
            expectPaginationError {
                AndroidOcrRegionPaginationHydrator.hydrate(
                    scanId = "scan-1",
                    expectedOcrRunId = "ocr-run-1",
                    expectedTotalCount = 3u,
                    pageSize = 2u,
                ) { premature }
            }

        assertEquals("OCR region pagination claimed completeness before the authoritative end", error.message)
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

    private fun loadedRegion(text: String): LoadedAndroidOcrTextRegion =
        LoadedAndroidOcrTextRegion(
            textRegionId = "text-region-1",
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

    private fun regionPage(
        offset: UInt,
        nextOffset: UInt?,
        hasMore: Boolean,
        complete: Boolean,
        range: IntRange,
        totalCount: UInt = 64u,
    ): AndroidOcrRegionPage =
        regionPage(
            offset = offset,
            nextOffset = nextOffset,
            hasMore = hasMore,
            complete = complete,
            regions = range.map { regionPageRegion(it) },
            totalCount = totalCount,
        )

    private fun regionPage(
        offset: UInt,
        nextOffset: UInt?,
        hasMore: Boolean,
        complete: Boolean,
        regions: List<AndroidOcrRegionPageRegion>,
        totalCount: UInt = regions.size.toUInt(),
    ): AndroidOcrRegionPage =
        AndroidOcrRegionPage(
            scanId = "scan-1",
            ocrRunId = "ocr-run-1",
            totalCount = totalCount,
            returnedCount = regions.size.toUInt(),
            offset = offset,
            nextOffset = nextOffset,
            hasMore = hasMore,
            complete = complete,
            regions = regions,
        )

    private fun regionPageRegion(
        index: Int,
        text: String = "region-$index",
    ): AndroidOcrRegionPageRegion =
        AndroidOcrRegionPageRegion(
            textRegionId = "text-region-${index.toString().padStart(3, '0')}",
            ocrRunId = "ocr-run-1",
            polygon =
                listOf(
                    OcrTextPoint(x = 0.0f, y = 0.0f),
                    OcrTextPoint(x = 10.0f, y = 0.0f),
                    OcrTextPoint(x = 10.0f, y = 10.0f),
                ),
            text = text,
            confidence = 0.91f,
            createdAtMs = 300L + index,
        )

    private fun expectPaginationError(block: () -> Unit): OcrRegionPaginationException =
        try {
            block()
            fail("expected OcrRegionPaginationException")
            throw AssertionError("unreachable")
        } catch (error: OcrRegionPaginationException) {
            error
        }
}
