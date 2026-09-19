package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrQueueExecutorTest {
    @Test
    fun detectedOutcomeUsesOneTransactionalFinalizerWithAllRegions() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized)
        val rust = FakeRustGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            fullText = "transactional production text",
                            regions =
                                listOf(
                                    AndroidRecognizedTextRegion(
                                        polygon =
                                            listOf(
                                                OcrTextPoint(0f, 0f),
                                                OcrTextPoint(100f, 0f),
                                                OcrTextPoint(100f, 40f),
                                                OcrTextPoint(0f, 40f),
                                            ),
                                        text = "transactional production text",
                                        confidence = 0.9f,
                                        createdAtMs = 300,
                                    ),
                                ),
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Recognized, completed.job.status)
        val finalized = rust.finalized.single()
        assertEquals("job-1", finalized.jobId)
        assertEquals(1u, finalized.attemptCount)
        assertEquals(OcrRunStatus.Detected, finalized.status)
        assertEquals("transactional production text", finalized.fullText)
        assertEquals(1, finalized.regions.size)
        assertEquals("transactional production text", finalized.regions.single().text)
        assertTrue(rust.recorded.isEmpty())
        assertTrue(rust.recordedRegionBatches.isEmpty())
        assertNull(queue.lastCompletionRetryable)
    }

    @Test
    fun noTextOutcomeUsesTransactionalFinalizerWithoutSplitPersistence() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized)
        val rust = FakeRustGateway()
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.NoTextDetected(
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Recognized, completed.job.status)
        assertEquals(OcrRunStatus.NoTextDetected, rust.finalized.single().status)
        assertTrue(rust.finalized.single().regions.isEmpty())
        assertTrue(rust.recorded.isEmpty())
        assertTrue(rust.recordedRegionBatches.isEmpty())
        assertNull(queue.lastCompletionRetryable)
    }

    @Test
    fun finalizerFailureCannotFallThroughToLegacyQueueCompletion() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized)
        val rust = FakeRustGateway(finalizeFailure = IllegalStateException("forced region persistence failure"))
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            fullText = "must roll back",
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val failed = step as AndroidOcrQueueStep.RecoverableFailure
        assertEquals("forced region persistence failure", failed.message)
        assertEquals(1, rust.finalized.size)
        assertTrue(rust.recorded.isEmpty())
        assertTrue(rust.recordedRegionBatches.isEmpty())
        assertNull(queue.lastCompletionRetryable)
    }

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
        assertEquals(true, rust.finalized.single().retryable)
        assertEquals(OcrRunStatus.Unavailable, rust.finalized.single().status)
        assertEquals(OcrUnavailableReason.ProviderUnavailable, rust.finalized.single().unavailableReason)
        assertEquals("", rust.finalized.single().fullText)
        assertTrue(rust.recorded.isEmpty())
        assertTrue(rust.recordedRegionBatches.isEmpty())
        assertNull(queue.lastCompletionRetryable)
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
        assertEquals(false, rust.finalized.single().retryable)
        assertEquals(OcrUnavailableReason.Cancelled, rust.finalized.single().unavailableReason)
        assertTrue(completed.job.cancellationRequested)
        assertNull(queue.lastCompletionRetryable)
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
        assertEquals(true, rust.finalized.single().retryable)
        assertEquals(OcrUnavailableReason.ProviderFailed, rust.finalized.single().unavailableReason)
        assertEquals("provider exploded", rust.finalized.single().unavailableMessage)
        assertNull(queue.lastCompletionRetryable)
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

    private class FakeRustGateway(
        private val finalizeFailure: RuntimeException? = null,
    ) : RustOcrGateway {
        val recorded = mutableListOf<AndroidRecordOcrRunRequest>()
        val recordedRegionBatches = mutableListOf<AndroidRecordOcrTextRegionsRequest>()
        val finalized = mutableListOf<AndroidFinalizeOcrJobRequest>()

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
        ): RecordedAndroidOcrTextRegions {
            recordedRegionBatches += request
            return RecordedAndroidOcrTextRegions(ocrRunId = request.ocrRunId, regions = emptyList())
        }

        override fun finalizeOcrJob(request: AndroidFinalizeOcrJobRequest): FinalizedAndroidOcrJob {
            finalized += request
            finalizeFailure?.let { throw it }
            val status =
                when (request.unavailableReason) {
                    OcrUnavailableReason.Cancelled -> AndroidOcrQueueJobStatus.Cancelled
                    OcrUnavailableReason.ProviderFailed -> AndroidOcrQueueJobStatus.Unavailable
                    OcrUnavailableReason.ProviderUnavailable ->
                        if (request.retryable) {
                            AndroidOcrQueueJobStatus.Queued
                        } else {
                            AndroidOcrQueueJobStatus.Unavailable
                        }
                    else -> AndroidOcrQueueJobStatus.Recognized
                }
            val availability =
                if (request.unavailableReason == OcrUnavailableReason.ProviderFailed) {
                    AndroidOcrProviderAvailability.Failed
                } else {
                    AndroidOcrProviderAvailability.Unavailable
                }
            return FinalizedAndroidOcrJob(
                job =
                    runningJob().copy(
                        status = status,
                        retryable = status == AndroidOcrQueueJobStatus.Queued,
                        nextRetryAtMs = if (status == AndroidOcrQueueJobStatus.Queued) 2_000 else null,
                        provider = request.provider,
                        providerVersion = request.providerVersion,
                        modelName = request.modelName,
                        providerAvailability = availability,
                        lastOcrRunId = if (status == AndroidOcrQueueJobStatus.Cancelled) null else "ocr-run-1",
                        cancellationRequested = request.unavailableReason == OcrUnavailableReason.Cancelled,
                    ),
                ocrRunId = if (status == AndroidOcrQueueJobStatus.Cancelled) null else "ocr-run-1",
                recordedRegionCount = request.regions.size.toUInt(),
                resolution =
                    when (status) {
                        AndroidOcrQueueJobStatus.Queued -> OcrFinalizationResolution.RetryScheduled
                        AndroidOcrQueueJobStatus.Cancelled -> OcrFinalizationResolution.CancelledBeforeCommit
                        AndroidOcrQueueJobStatus.Unavailable -> OcrFinalizationResolution.TerminalUnavailable
                        else -> OcrFinalizationResolution.Completed
                    },
            )
        }
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
