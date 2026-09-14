package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrWorkflowTest {
    @Test
    fun detectedTextIsRecordedThroughRustGateway() {
        val gateway = FakeRustOcrGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = gateway,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit",
                            providerVersion = "2026.09",
                            modelName = "latin-v1",
                            fullText = "hello notebook",
                            completedAtMs = 250,
                        ),
                    ),
            )

        val result = workflow.run(startRequest())

        assertEquals(OcrPresentationStatus.Detected, result.status)
        assertEquals("hello notebook", result.textPreview)
        assertEquals(OcrRunStatus.Detected, gateway.recorded.single().status)
        assertEquals("hello notebook", gateway.recorded.single().fullText)
        assertEquals(null, gateway.recorded.single().unavailableReason)
    }

    @Test
    fun noTextDetectedIsRecordedSeparatelyFromDetectedEmptyText() {
        val gateway = FakeRustOcrGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = gateway,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.NoTextDetected(
                            provider = "mlkit",
                            providerVersion = "2026.09",
                            modelName = null,
                            completedAtMs = 260,
                        ),
                    ),
            )

        val result = workflow.run(startRequest())

        assertEquals(OcrPresentationStatus.NoTextDetected, result.status)
        assertEquals(OcrRunStatus.NoTextDetected, gateway.recorded.single().status)
        assertEquals("", gateway.recorded.single().fullText)
        assertNotEquals(OcrRunStatus.Detected, gateway.recorded.single().status)
    }

    @Test
    fun providerExceptionIsRecordedAsUnavailableNotEmptyDetectedText() {
        val gateway = FakeRustOcrGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = gateway,
                provider = ThrowingProvider(IllegalStateException("mlkit model missing")),
            )

        val result = workflow.run(startRequest())

        assertEquals(OcrPresentationStatus.Unavailable, result.status)
        assertEquals(OcrUnavailableReason.ProviderFailed, result.unavailableReason)
        assertEquals(OcrRunStatus.Unavailable, gateway.recorded.single().status)
        assertEquals("", gateway.recorded.single().fullText)
        assertEquals(OcrUnavailableReason.ProviderFailed, gateway.recorded.single().unavailableReason)
        assertTrue(result.retryAvailable)
    }

    @Test
    fun cancellationIsRecordedSeparatelyFromProviderFailure() {
        val gateway = FakeRustOcrGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = gateway,
                provider = FakeProvider(AndroidOcrRecognitionOutcome.Cancelled()),
            )

        val result = workflow.run(startRequest())

        assertEquals(OcrPresentationStatus.Cancelled, result.status)
        assertEquals(OcrUnavailableReason.Cancelled, result.unavailableReason)
        assertEquals(OcrRunStatus.Unavailable, gateway.recorded.single().status)
        assertEquals(OcrUnavailableReason.Cancelled, gateway.recorded.single().unavailableReason)
    }

    @Test
    fun rustRecordRejectionDoesNotAppearAsSuccessfulOcr() {
        val gateway = FakeRustOcrGateway(recordFailure = IllegalArgumentException("empty detected text"))
        val workflow =
            AndroidOcrWorkflow(
                gateway = gateway,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit",
                            providerVersion = "2026.09",
                            modelName = null,
                            fullText = "",
                            completedAtMs = 270,
                        ),
                    ),
            )

        val result = workflow.run(startRequest())

        assertEquals(OcrPresentationStatus.Failed, result.status)
        assertEquals("empty detected text", result.message)
        assertTrue(result.retryAvailable)
    }

    private fun startRequest(): AndroidOcrStartRequest =
        AndroidOcrStartRequest(
            scanId = "scan-1",
            inputKind = OcrInputKind.OcrOptimized,
            widthPx = 1_000u,
            heightPx = 1_400u,
        )

    private class FakeProvider(
        private val outcome: AndroidOcrRecognitionOutcome,
    ) : AndroidOcrProvider {
        override fun recognize(input: PreparedAndroidOcrInput): AndroidOcrRecognitionOutcome = outcome
    }

    private class ThrowingProvider(
        private val failure: RuntimeException,
    ) : AndroidOcrProvider {
        override fun recognize(input: PreparedAndroidOcrInput): AndroidOcrRecognitionOutcome {
            throw failure
        }
    }

    private class FakeRustOcrGateway(
        private val recordFailure: RuntimeException? = null,
    ) : RustOcrGateway {
        val recorded = mutableListOf<AndroidRecordOcrRunRequest>()

        override fun prepareOcrInput(request: AndroidOcrStartRequest): PreparedAndroidOcrInput =
            PreparedAndroidOcrInput(
                scanId = request.scanId,
                inputAssetId = "asset-1",
                inputKind = request.inputKind,
                mediaType = "image/png",
                relativePath = "assets/ocr/asset-1.png",
                byteLength = 1_024u,
                widthPx = request.widthPx,
                heightPx = request.heightPx,
            )

        override fun recordOcrRun(request: AndroidRecordOcrRunRequest): RecordedAndroidOcrRun {
            recorded += request
            recordFailure?.let { throw it }
            return RecordedAndroidOcrRun(
                ocrRunId = "ocr-run-1",
                scanId = request.scanId,
                inputAssetId = request.inputAssetId,
                status = request.status,
            )
        }
    }
}
