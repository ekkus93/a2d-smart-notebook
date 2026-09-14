package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrSearchTest {
    @Test
    fun blankQueryStaysLocalAndDoesNotCallRust() {
        val gateway = FakeOcrSearchGateway()
        val controller = AndroidOcrSearchController(gateway)

        val state = controller.submit("   ")

        assertEquals(OcrSearchPresentationStatus.NoQuery, state.status)
        assertEquals(0, gateway.requests.size)
        assertTrue(state.hits.isEmpty())
    }

    @Test
    fun detectedFullTextAndRegionHitsMapToPresentationRows() {
        val gateway =
            FakeOcrSearchGateway(
                results =
                    AndroidOcrSearchResults(
                        query = "notebook",
                        hits =
                            listOf(
                                AndroidOcrSearchHit(
                                    pageId = "page-1",
                                    scanId = "scan-1",
                                    ocrRunId = "ocr-run-1",
                                    textRegionId = null,
                                    documentKind = AndroidOcrSearchDocumentKind.FullText,
                                    snippet = "hello [notebook]",
                                ),
                                AndroidOcrSearchHit(
                                    pageId = "page-2",
                                    scanId = "scan-2",
                                    ocrRunId = "ocr-run-2",
                                    textRegionId = "region-2",
                                    documentKind = AndroidOcrSearchDocumentKind.TextRegion,
                                    snippet = "project [notebook]",
                                ),
                            ),
                    ),
            )
        val controller = AndroidOcrSearchController(gateway, defaultLimit = 12u)

        val state = controller.submit(" notebook ")

        assertEquals(OcrSearchPresentationStatus.Results, state.status)
        assertEquals("notebook", state.query)
        assertEquals(12u, gateway.requests.single().limit)
        assertEquals(2, state.hits.size)
        assertEquals(AndroidOcrSearchDocumentKind.FullText, state.hits[0].documentKind)
        assertEquals(null, state.hits[0].textRegionId)
        assertEquals(AndroidOcrSearchDocumentKind.TextRegion, state.hits[1].documentKind)
        assertEquals("region-2", state.hits[1].textRegionId)
    }

    @Test
    fun noSearchHitsStayDistinctFromUnavailableOrFakeText() {
        val gateway = FakeOcrSearchGateway(results = AndroidOcrSearchResults("missing", emptyList()))
        val controller = AndroidOcrSearchController(gateway)

        val state = controller.submit("missing")

        assertEquals(OcrSearchPresentationStatus.NoMatches, state.status)
        assertEquals("missing", state.query)
        assertTrue(state.hits.isEmpty())
    }

    @Test
    fun rustValidationErrorIsPreservedForPresentation() {
        val gateway =
            FakeOcrSearchGateway(
                failure = IllegalArgumentException("STORAGE_OCR_SEARCH_LIMIT_INVALID"),
            )
        val controller = AndroidOcrSearchController(gateway)

        val state = controller.submit("notebook")

        assertEquals(OcrSearchPresentationStatus.Error, state.status)
        assertEquals("notebook", state.query)
        assertEquals("STORAGE_OCR_SEARCH_LIMIT_INVALID", state.errorMessage)
        assertTrue(state.hits.isEmpty())
    }

    private class FakeOcrSearchGateway(
        private val results: AndroidOcrSearchResults = AndroidOcrSearchResults("notebook", emptyList()),
        private val failure: RuntimeException? = null,
    ) : AndroidOcrSearchGateway {
        val requests = mutableListOf<AndroidOcrSearchRequest>()

        override fun searchOcrText(request: AndroidOcrSearchRequest): AndroidOcrSearchResults {
            requests += request
            failure?.let { throw it }
            return results
        }
    }
}
