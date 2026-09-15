package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrQueueExecutorTest {
    @Test
    fun retryableUnavailableOutcomeIsReturnedToRustQueueWithoutFakeText() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Queued)
        val rust = FakeRustGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Unavailable(
                            reason = OcrUnavailableReason.ProviderUnavailable,
                            message = "provider temporarily unavailable",
                            retryAvailable = true,
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Queued, completed.job.status)
        assertEquals(true, queue.lastCompletionRetryable)
        assertEquals(OcrRunStatus.Unavailable, rust.recorded.single().status)
        assertEquals(OcrUnavailableReason.ProviderUnavailable, rust.recorded.single().unavailableReason)
        assertEquals("", rust.recorded.single().fullText)
    }

    @Test
    fun cancellationStaysNonRetryableAndCompletesAsCancelled() {
        val queue =
            FakeQueueGateway(
                completionStatus = AndroidOcrQueueJobStatus.Cancelled,
                cancellationRequested = true,
            )
        val rust = FakeRustGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider = FakeProvider(AndroidOcrRecognitionOutcome.Cancelled()),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Cancelled, completed.job.status)
        assertEquals(false, queue.lastCompletionRetryable)
        assertEquals(OcrUnavailableReason.Cancelled, rust.recorded.single().unavailableReason)
        assertTrue(completed.job.cancellationRequested)
    }

    @Test
    fun providerFailurePreservesProviderDiagnosticsAtQueueBoundary() {
        val queue =
            FakeQueueGateway(
                completionStatus = AndroidOcrQueueJobStatus.Unavailable,
                providerAvailability = AndroidOcrProviderAvailability.Failed,
            )
        val rust = FakeRustGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider = ThrowingProvider(IllegalStateException("provider exploded")),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Unavailable, completed.job.status)
        assertEquals(AndroidOcrProviderAvailability.Failed, completed.job.providerAvailability)
        assertEquals(true, queue.lastCompletionRetryable)
        assertEquals(OcrUnavailableReason.ProviderFailed, rust.recorded.single().unavailableReason)
        assertEquals("provider exploded", rust.recorded.single().unavailableMessage)
    }

    private class FakeQueueGateway(
        private val completionStatus: AndroidOcrQueueJobStatus,
        private val cancellationRequested: Boolean = false,
        private val providerAvailability: AndroidOcrProviderAvailability =
            AndroidOcrProviderAvailability.Unavailable,
    ) : AndroidOcrQueueGateway {
        private var claimed = false
        var lastCompletionRetryable: Boolean? = null

        override fun enqueue(
            scanId: String,
            inputKind: OcrInputKind,
            widthPx: UInt,
            heightPx: UInt,
        ): AndroidOcrQueueJob = runningJob()

        override fun claimNext(): AndroidOcrQueueJob? =
            if (claimed) {
                null
            } else {
                claimed = true
                runningJob()
            }

        override fun get(jobId: String): AndroidOcrQueueJob = runningJob()

        override fun requestCancellation(jobId: String): AndroidOcrQueueJob =
            runningJob().copy(cancellationRequested = true)

        override fun complete(
            jobId: String,
            ocrRunId: String,
            retryable: Boolean,
        ): AndroidOcrQueueJob {
            lastCompletionRetryable = retryable
            return runningJob().copy(
                status = completionStatus,
                retryable = completionStatus == AndroidOcrQueueJobStatus.Queued,
                nextRetryAtMs =
                    if (completionStatus == AndroidOcrQueueJobStatus.Queued) 2_000 else null,
                provider = "mlkit-bundled-text-recognition",
                providerVersion = "16.0.1",
                modelName = "latin-v2-bundled",
                providerAvailability = providerAvailability,
                lastOcrRunId = ocrRunId,
                cancellationRequested = cancellationRequested,
            )
        }
    }

    private class FakeRustGateway : RustOcrGateway {
        val recorded = mutableListOf<AndroidRecordOcrRunRequest>()

        override fun prepareOcrInput(request: AndroidOcrStartRequest): PreparedAndroidOcrInput =
            runningJob().preparedInput()

        override fun recordOcrRun(request: AndroidRecordOcrRunRequest): RecordedAndroidOcrRun {
            recorded += request
            return RecordedAndroidOcrRun(
                ocrRunId = "ocr-run-1",
                scanId = request.scanId,
                inputAssetId = request.inputAssetId,
                status = request.status,
            )
        }

        override fun recordOcrTextRegions(
            request: AndroidRecordOcrTextRegionsRequest,
        ): RecordedAndroidOcrTextRegions =
            RecordedAndroidOcrTextRegions(ocrRunId = request.ocrRunId, regions = emptyList())
    }

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

    companion object {
        private fun runningJob(): AndroidOcrQueueJob =
            AndroidOcrQueueJob(
                jobId = "job-1",
                scanId = "scan-1",
                inputAssetId = "asset-1",
                inputKind = OcrInputKind.OcrOptimized,
                mediaType = "image/png",
                relativePath = "assets/ocr/asset-1.png",
                byteLength = 1_024u,
                widthPx = 1_000u,
                heightPx = 1_400u,
                status = AndroidOcrQueueJobStatus.Running,
                attemptCount = 1u,
                retryable = true,
                nextRetryAtMs = null,
                lastErrorCode = null,
                lastErrorMessage = null,
                provider = null,
                providerVersion = null,
                modelName = null,
                providerAvailability = AndroidOcrProviderAvailability.Unknown,
                lastOcrRunId = null,
                cancellationRequested = false,
            )
    }
}
