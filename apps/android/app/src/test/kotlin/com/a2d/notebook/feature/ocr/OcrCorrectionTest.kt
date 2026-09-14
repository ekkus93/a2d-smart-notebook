package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrCorrectionTest {
    @Test
    fun blankCorrectionStaysLocalAndDoesNotCallRust() {
        val gateway = FakeCorrectionGateway()
        val controller = AndroidOcrCorrectionController(gateway)

        val state = controller.submitCorrection(
            scanId = "scan-1",
            textRegionId = null,
            correctedText = "   ",
        )

        assertEquals(OcrCorrectionPresentationStatus.Error, state.status)
        assertEquals("Correction text must not be empty", state.errorMessage)
        assertTrue(gateway.recordRequests.isEmpty())
    }

    @Test
    fun correctionSubmissionMapsToSavedPresentationState() {
        val gateway = FakeCorrectionGateway()
        val controller = AndroidOcrCorrectionController(gateway)

        val state = controller.submitCorrection(
            scanId = "scan-1",
            textRegionId = "region-1",
            correctedText = " corrected text ",
            previousText = "ocr text",
        )

        assertEquals(OcrCorrectionPresentationStatus.Saved, state.status)
        assertEquals("corrected text", gateway.recordRequests.single().correctedText)
        assertEquals("region-1", gateway.recordRequests.single().textRegionId)
        assertEquals("correction-1", state.lastSaved?.textCorrectionId)
        assertEquals("ocr text", state.lastSaved?.previousText)
    }

    @Test
    fun correctionReviewMapsPersistedRowsToPresentationState() {
        val gateway =
            FakeCorrectionGateway(
                review =
                    AndroidOcrCorrectionReview(
                        scanId = "scan-1",
                        corrections =
                            listOf(
                                AndroidOcrTextCorrection(
                                    textCorrectionId = "correction-1",
                                    scanId = "scan-1",
                                    textRegionId = null,
                                    correctedText = "hello notebook",
                                    previousText = "helo notebook",
                                    createdAtMs = 300,
                                ),
                            ),
                    ),
            )
        val controller = AndroidOcrCorrectionController(gateway, defaultReviewLimit = 12u)

        val state = controller.loadReview("scan-1")

        assertEquals(OcrCorrectionPresentationStatus.Review, state.status)
        assertEquals(12u, gateway.listRequests.single().limit)
        assertEquals(1, state.corrections.size)
        assertEquals("hello notebook", state.corrections[0].correctedText)
        assertEquals("helo notebook", state.corrections[0].previousText)
    }

    @Test
    fun rustCorrectionErrorIsPreservedForPresentation() {
        val gateway = FakeCorrectionGateway(failure = IllegalStateException("CORE_OCR_CORRECTION_SCAN_MISSING"))
        val controller = AndroidOcrCorrectionController(gateway)

        val state = controller.submitCorrection(
            scanId = "scan-1",
            textRegionId = null,
            correctedText = "hello notebook",
        )

        assertEquals(OcrCorrectionPresentationStatus.Error, state.status)
        assertEquals("CORE_OCR_CORRECTION_SCAN_MISSING", state.errorMessage)
    }

    private class FakeCorrectionGateway(
        private val review: AndroidOcrCorrectionReview = AndroidOcrCorrectionReview("scan-1", emptyList()),
        private val failure: RuntimeException? = null,
    ) : AndroidOcrCorrectionGateway {
        val recordRequests = mutableListOf<AndroidRecordOcrCorrectionRequest>()
        val listRequests = mutableListOf<AndroidListOcrCorrectionsForScanRequest>()

        override fun recordOcrCorrection(
            request: AndroidRecordOcrCorrectionRequest,
        ): RecordedAndroidOcrCorrection {
            recordRequests += request
            failure?.let { throw it }
            return RecordedAndroidOcrCorrection(
                textCorrectionId = "correction-1",
                scanId = request.scanId,
                textRegionId = request.textRegionId,
                correctedText = request.correctedText,
                previousText = request.previousText,
                createdAtMs = 300,
            )
        }

        override fun listOcrCorrectionsForScan(
            request: AndroidListOcrCorrectionsForScanRequest,
        ): AndroidOcrCorrectionReview {
            listRequests += request
            failure?.let { throw it }
            return review
        }
    }
}
