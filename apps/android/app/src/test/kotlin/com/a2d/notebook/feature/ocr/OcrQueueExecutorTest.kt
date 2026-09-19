package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrQueueExecutorTest {
    @Test
    fun detectedOutcomeUsesOneFinalizerRequestWithAllRegionsAndNeverSplitPersistence() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized)
        val rust = FakeRustGateway()
        val region =
            AndroidRecognizedTextRegion(
                polygon = listOf(OcrTextPoint(0f, 0f), OcrTextPoint(10f, 0f), OcrTextPoint(10f, 10f)),
                text = "sentinel text",
                confidence = 0.9f,
                createdAtMs = 300,
            )
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            fullText = "sentinel text",
                            regions = listOf(region),
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Recognized, completed.job.status)
        assertEquals(1, rust.finalized.size)
        assertEquals(OcrRunStatus.Detected, rust.finalized.single().status)
        assertEquals("sentinel text", rust.finalized.single().fullText)
        assertEquals(1, rust.finalized.single().regions.size)
        assertTrue(rust.recorded.isEmpty())
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun noTextOutcomeUsesFinalizerAndNeverSplitPersistence() {
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
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun finalizerFailureAfterDetectedProviderResultCannotFallThroughToQueueCompletion() {
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
                            regions =
                                listOf(
                                    AndroidRecognizedTextRegion(
                                        polygon = listOf(OcrTextPoint(0f, 0f), OcrTextPoint(10f, 0f), OcrTextPoint(10f, 10f)),
                                        text = "must roll back",
                                        confidence = 0.8f,
                                        createdAtMs = 300,
                                    ),
                                ),
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val failure = step as AndroidOcrQueueStep.RecoverableFailure
        assertTrue(failure.message.contains("forced region persistence failure"))
        assertTrue(rust.recorded.isEmpty())
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun cancellationBeforeCommitReturnedByRustWinsOverDetectedProviderOutput() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized)
        val rust = FakeRustGateway(finalizedStatus = AndroidOcrQueueJobStatus.Cancelled)
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            fullText = "cancelled text must not be accepted",
                            regions =
                                listOf(
                                    AndroidRecognizedTextRegion(
                                        polygon = listOf(OcrTextPoint(0f, 0f), OcrTextPoint(10f, 0f), OcrTextPoint(10f, 10f)),
                                        text = "cancelled text must not be accepted",
                                        confidence = 0.8f,
                                        createdAtMs = 300,
                                    ),
                                ),
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Cancelled, completed.job.status)
        assertEquals(null, completed.job.lastOcrRunId)
        assertEquals(OcrRunStatus.Detected, rust.finalized.single().status)
        assertEquals("cancelled text must not be accepted", rust.finalized.single().fullText)
        assertTrue(rust.recorded.isEmpty())
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun completionAfterCommitRemainsRecognizedEvenWhenCancellationWouldArriveLate() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized, cancellationRequested = true)
        val rust = FakeRustGateway(finalizedStatus = AndroidOcrQueueJobStatus.Recognized)
        val workflow =
            AndroidOcrWorkflow(
                gateway = rust,
                provider =
                    FakeProvider(
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "mlkit-bundled-text-recognition",
                            providerVersion = "16.0.1",
                            modelName = "latin-v2-bundled",
                            fullText = "accepted before late cancellation",
                            regions = emptyList(),
                            completedAtMs = 300,
                        ),
                    ),
            )

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Recognized, completed.job.status)
        assertEquals("ocr-run-1", completed.job.lastOcrRunId)
        assertEquals(OcrFinalizationResolution.Completed, rust.lastResolution)
        assertTrue(rust.recorded.isEmpty())
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun staleAttemptFinalizationFailureCannotCompleteTheQueue() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Recognized)
        val rust = FakeRustGateway(finalizeFailure = IllegalStateException("CORE_OCR_FINALIZE_STALE_ATTEMPT"))
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

        val failure = step as AndroidOcrQueueStep.RecoverableFailure
        assertTrue(failure.message.contains("CORE_OCR_FINALIZE_STALE_ATTEMPT"))
        assertTrue(rust.recorded.isEmpty())
        assertEquals(0, queue.completeCalls)
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
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun cancellationStaysNonRetryableAndCompletesAsCancelled() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Cancelled, cancellationRequested = true)
        val rust = FakeRustGateway()
        val workflow = AndroidOcrWorkflow(gateway = rust, provider = FakeProvider(AndroidOcrRecognitionOutcome.Cancelled()))

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Cancelled, completed.job.status)
        assertEquals(false, rust.finalized.single().retryable)
        assertEquals(OcrUnavailableReason.Cancelled, rust.finalized.single().unavailableReason)
        assertTrue(completed.job.cancellationRequested)
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun providerFailurePreservesProviderDiagnosticsAtQueueBoundary() {
        val queue = FakeQueueGateway(completionStatus = AndroidOcrQueueJobStatus.Unavailable, providerAvailability = AndroidOcrProviderAvailability.Failed)
        val rust = FakeRustGateway()
        val workflow = AndroidOcrWorkflow(gateway = rust, provider = ThrowingProvider(IllegalStateException("provider exploded")))

        val step = AndroidOcrQueueProcessor(queue, workflow).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Unavailable, completed.job.status)
        assertEquals(AndroidOcrProviderAvailability.Failed, completed.job.providerAvailability)
        assertEquals(true, rust.finalized.single().retryable)
        assertEquals(OcrUnavailableReason.ProviderFailed, rust.finalized.single().unavailableReason)
        assertEquals("provider exploded", rust.finalized.single().unavailableMessage)
        assertEquals(0, queue.completeCalls)
    }

    private class FakeQueueGateway(
        private val completionStatus: AndroidOcrQueueJobStatus,
        private val cancellationRequested: Boolean = false,
        private val providerAvailability: AndroidOcrProviderAvailability = AndroidOcrProviderAvailability.Unavailable,
    ) : AndroidOcrQueueGateway {
        private var claimed = false
        var completeCalls = 0

        override fun enqueue(scanId: String, inputKind: OcrInputKind, widthPx: UInt, heightPx: UInt): AndroidOcrQueueJob = runningJob()
        override fun claimNext(): AndroidOcrQueueJob? = if (claimed) null else { claimed = true; runningJob().copy(cancellationRequested = cancellationRequested) }
        override fun get(jobId: String): AndroidOcrQueueJob = runningJob()
        override fun requestCancellation(jobId: String): AndroidOcrQueueJob = runningJob().copy(cancellationRequested = true)
        override fun complete(jobId: String, ocrRunId: String, retryable: Boolean): AndroidOcrQueueJob {
            completeCalls += 1
            return runningJob().copy(
                status = completionStatus,
                retryable = completionStatus == AndroidOcrQueueJobStatus.Queued,
                nextRetryAtMs = if (completionStatus == AndroidOcrQueueJobStatus.Queued) 2_000 else null,
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
        private val finalizedStatus: AndroidOcrQueueJobStatus? = null,
    ) : RustOcrGateway {
        val recorded = mutableListOf<AndroidRecordOcrRunRequest>()
        val finalized = mutableListOf<AndroidFinalizeOcrJobRequest>()
        var lastResolution: OcrFinalizationResolution? = null

        override fun prepareOcrInput(request: AndroidOcrStartRequest): PreparedAndroidOcrInput = runningJob().preparedInput()
        override fun recordOcrRun(request: AndroidRecordOcrRunRequest): RecordedAndroidOcrRun {
            recorded += request
            return RecordedAndroidOcrRun("ocr-run-1", request.scanId, request.inputAssetId, request.status)
        }
        override fun recordOcrTextRegions(request: AndroidRecordOcrTextRegionsRequest): RecordedAndroidOcrTextRegions =
            RecordedAndroidOcrTextRegions(request.ocrRunId, emptyList())
        override fun finalizeOcrJob(request: AndroidFinalizeOcrJobRequest): FinalizedAndroidOcrJob {
            finalizeFailure?.let { throw it }
            finalized += request
            val status = finalizedStatus ?: when (request.unavailableReason) {
                OcrUnavailableReason.Cancelled -> AndroidOcrQueueJobStatus.Cancelled
                OcrUnavailableReason.ProviderFailed -> AndroidOcrQueueJobStatus.Unavailable
                OcrUnavailableReason.ProviderUnavailable -> if (request.retryable) AndroidOcrQueueJobStatus.Queued else AndroidOcrQueueJobStatus.Unavailable
                else -> AndroidOcrQueueJobStatus.Recognized
            }
            val availability = if (request.unavailableReason == OcrUnavailableReason.ProviderFailed) AndroidOcrProviderAvailability.Failed else AndroidOcrProviderAvailability.Unavailable
            val ocrRunId = if (status == AndroidOcrQueueJobStatus.Cancelled) null else "ocr-run-1"
            val resolution = when (status) {
                AndroidOcrQueueJobStatus.Queued -> OcrFinalizationResolution.RetryScheduled
                AndroidOcrQueueJobStatus.Cancelled -> OcrFinalizationResolution.CancelledBeforeCommit
                AndroidOcrQueueJobStatus.Unavailable -> OcrFinalizationResolution.TerminalUnavailable
                else -> OcrFinalizationResolution.Completed
            }
            lastResolution = resolution
            return FinalizedAndroidOcrJob(
                job = runningJob().copy(
                    status = status,
                    retryable = status == AndroidOcrQueueJobStatus.Queued,
                    nextRetryAtMs = if (status == AndroidOcrQueueJobStatus.Queued) 2_000 else null,
                    provider = request.provider,
                    providerVersion = request.providerVersion,
                    modelName = request.modelName,
                    providerAvailability = availability,
                    lastOcrRunId = ocrRunId,
                    cancellationRequested = status == AndroidOcrQueueJobStatus.Cancelled,
                ),
                ocrRunId = ocrRunId,
                recordedRegionCount = if (status == AndroidOcrQueueJobStatus.Cancelled) 0u else request.regions.size.toUInt(),
                resolution = resolution,
            )
        }
    }

    private class FakeProvider(private val outcome: AndroidOcrRecognitionOutcome) : AndroidOcrProvider {
        override fun recognize(input: PreparedAndroidOcrInput): AndroidOcrRecognitionOutcome = outcome
    }
    private class ThrowingProvider(private val failure: RuntimeException) : AndroidOcrProvider {
        override fun recognize(input: PreparedAndroidOcrInput): AndroidOcrRecognitionOutcome { throw failure }
    }

    companion object {
        private fun runningJob(): AndroidOcrQueueJob =
            AndroidOcrQueueJob(
                jobId = "job-1", scanId = "scan-1", inputAssetId = "asset-1", inputKind = OcrInputKind.OcrOptimized,
                mediaType = "image/png", relativePath = "assets/ocr/asset-1.png", byteLength = 1_024u,
                widthPx = 1_000u, heightPx = 1_400u, status = AndroidOcrQueueJobStatus.Running, attemptCount = 1u,
                retryable = true, nextRetryAtMs = null, lastErrorCode = null, lastErrorMessage = null,
                provider = null, providerVersion = null, modelName = null, providerAvailability = AndroidOcrProviderAvailability.Unknown,
                lastOcrRunId = null, cancellationRequested = false,
            )
    }
}
