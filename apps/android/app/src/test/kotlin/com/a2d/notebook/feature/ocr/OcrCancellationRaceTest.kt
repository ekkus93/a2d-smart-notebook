package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Sentinels for the production queue-processor/finalizer cancellation contract. */
class OcrCancellationRaceTest {
    @Test
    fun cancellationAfterProviderBeforeFinalizerLetsRustCommitPointWin() {
        val state = DurableRaceState()
        val queue = RaceQueueGateway(state)
        val rust = RaceRustGateway(state)
        val provider = CallbackProvider {
            queue.requestCancellation(JOB_ID)
            detectedOutcome()
        }

        val step = AndroidOcrQueueProcessor(queue, AndroidOcrWorkflow(rust, provider)).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Cancelled, completed.job.status)
        assertEquals(OcrFinalizationResolution.CancelledBeforeCommit, rust.lastResolution)
        assertNull(completed.job.lastOcrRunId)
        assertEquals(1, rust.finalizeCalls)
        assertEquals(0, queue.completeCalls)
    }

    @Test
    fun cancellationAfterSuccessfulFinalizerCannotRewriteCompletion() {
        val state = DurableRaceState()
        val queue = RaceQueueGateway(state)
        val rust = RaceRustGateway(state)

        val step = AndroidOcrQueueProcessor(queue, AndroidOcrWorkflow(rust, CallbackProvider { detectedOutcome() })).processNext()

        val completed = step as AndroidOcrQueueStep.Completed
        assertEquals(AndroidOcrQueueJobStatus.Recognized, completed.job.status)
        assertEquals(OcrFinalizationResolution.Completed, rust.lastResolution)
        val error = runCatching { queue.requestCancellation(JOB_ID) }.exceptionOrNull()
        assertTrue(error is IllegalStateException)
        assertEquals(AndroidOcrQueueJobStatus.Recognized, state.status)
        assertEquals(OCR_RUN_ID, state.ocrRunId)
    }

    @Test
    fun staleClaimedAttemptIsRejectedWithoutAndroidInventingAResolution() {
        val state = DurableRaceState(currentAttempt = 2u)
        val queue = RaceQueueGateway(state, claimedAttempt = 1u)
        val rust = RaceRustGateway(state)

        val step = AndroidOcrQueueProcessor(queue, AndroidOcrWorkflow(rust, CallbackProvider { detectedOutcome() })).processNext()

        val failure = step as AndroidOcrQueueStep.RecoverableFailure
        assertTrue(failure.message.contains("stale OCR attempt"))
        assertEquals(AndroidOcrQueueJobStatus.Running, state.status)
        assertNull(state.ocrRunId)
        assertEquals(0, queue.completeCalls)
    }

    private class DurableRaceState(
        var status: AndroidOcrQueueJobStatus = AndroidOcrQueueJobStatus.Running,
        var currentAttempt: UInt = 1u,
        var cancellationRequested: Boolean = false,
        var ocrRunId: String? = null,
    )

    private class RaceQueueGateway(
        private val state: DurableRaceState,
        private val claimedAttempt: UInt = state.currentAttempt,
    ) : AndroidOcrQueueGateway {
        private var claimed = false
        var completeCalls = 0

        override fun enqueue(scanId: String, inputKind: OcrInputKind, widthPx: UInt, heightPx: UInt) = job()

        override fun claimNext(): AndroidOcrQueueJob? = if (claimed) null else job().also { claimed = true }

        override fun get(jobId: String) = job()

        override fun requestCancellation(jobId: String): AndroidOcrQueueJob {
            if (state.status == AndroidOcrQueueJobStatus.Recognized || state.status == AndroidOcrQueueJobStatus.Unavailable) {
                throw IllegalStateException("completed OCR jobs cannot be cancelled")
            }
            state.cancellationRequested = true
            if (state.status == AndroidOcrQueueJobStatus.Queued) state.status = AndroidOcrQueueJobStatus.Cancelled
            return job()
        }

        override fun complete(jobId: String, ocrRunId: String, retryable: Boolean): AndroidOcrQueueJob {
            completeCalls += 1
            error("legacy split completion must not be called")
        }

        private fun job() = AndroidOcrQueueJob(
            jobId = JOB_ID,
            scanId = SCAN_ID,
            inputAssetId = ASSET_ID,
            inputKind = OcrInputKind.OcrOptimized,
            mediaType = "image/png",
            relativePath = "assets/ocr/$ASSET_ID.png",
            byteLength = 1024u,
            widthPx = 1000u,
            heightPx = 1400u,
            status = state.status,
            attemptCount = claimedAttempt,
            retryable = state.status == AndroidOcrQueueJobStatus.Running,
            nextRetryAtMs = null,
            lastErrorCode = null,
            lastErrorMessage = null,
            provider = null,
            providerVersion = null,
            modelName = null,
            providerAvailability = AndroidOcrProviderAvailability.Unknown,
            lastOcrRunId = state.ocrRunId,
            cancellationRequested = state.cancellationRequested,
        )
    }

    private class RaceRustGateway(private val state: DurableRaceState) : RustOcrGateway {
        var finalizeCalls = 0
        var lastResolution: OcrFinalizationResolution? = null

        override fun prepareOcrInput(request: AndroidOcrStartRequest) = PreparedAndroidOcrInput(
            scanId = SCAN_ID,
            inputAssetId = ASSET_ID,
            inputKind = OcrInputKind.OcrOptimized,
            mediaType = "image/png",
            relativePath = "assets/ocr/$ASSET_ID.png",
            byteLength = 1024u,
            widthPx = 1000u,
            heightPx = 1400u,
        )

        override fun recordOcrRun(request: AndroidRecordOcrRunRequest): RecordedAndroidOcrRun =
            error("legacy split record must not be called")

        override fun recordOcrTextRegions(request: AndroidRecordOcrTextRegionsRequest): RecordedAndroidOcrTextRegions =
            error("legacy split region record must not be called")

        override fun finalizeOcrJob(request: AndroidFinalizeOcrJobRequest): FinalizedAndroidOcrJob {
            finalizeCalls += 1
            if (request.attemptCount != state.currentAttempt) throw IllegalStateException("stale OCR attempt")
            val cancelled = state.cancellationRequested || request.unavailableReason == OcrUnavailableReason.Cancelled
            state.status = if (cancelled) AndroidOcrQueueJobStatus.Cancelled else AndroidOcrQueueJobStatus.Recognized
            state.ocrRunId = if (cancelled) null else OCR_RUN_ID
            val resolution = if (cancelled) OcrFinalizationResolution.CancelledBeforeCommit else OcrFinalizationResolution.Completed
            lastResolution = resolution
            return FinalizedAndroidOcrJob(
                job = AndroidOcrQueueJob(
                    jobId = JOB_ID,
                    scanId = SCAN_ID,
                    inputAssetId = ASSET_ID,
                    inputKind = OcrInputKind.OcrOptimized,
                    mediaType = "image/png",
                    relativePath = "assets/ocr/$ASSET_ID.png",
                    byteLength = 1024u,
                    widthPx = 1000u,
                    heightPx = 1400u,
                    status = state.status,
                    attemptCount = state.currentAttempt,
                    retryable = false,
                    nextRetryAtMs = null,
                    lastErrorCode = if (cancelled) "OCR_CANCELLED" else null,
                    lastErrorMessage = null,
                    provider = request.provider,
                    providerVersion = request.providerVersion,
                    modelName = request.modelName,
                    providerAvailability = AndroidOcrProviderAvailability.Available,
                    lastOcrRunId = state.ocrRunId,
                    cancellationRequested = state.cancellationRequested,
                ),
                ocrRunId = state.ocrRunId,
                recordedRegionCount = if (cancelled) 0u else request.regions.size.toUInt(),
                resolution = resolution,
            )
        }
    }

    private class CallbackProvider(private val callback: () -> AndroidOcrRecognitionOutcome) : AndroidOcrProvider {
        override fun recognize(input: PreparedAndroidOcrInput) = callback()
    }

    companion object {
        private const val JOB_ID = "job-race"
        private const val SCAN_ID = "scan-race"
        private const val ASSET_ID = "asset-race"
        private const val OCR_RUN_ID = "run-race"

        private fun detectedOutcome() = AndroidOcrRecognitionOutcome.Detected(
            provider = "mlkit-bundled-text-recognition",
            providerVersion = "16.0.1",
            modelName = "latin-v2-bundled",
            fullText = "race sentinel",
            regions = emptyList(),
            completedAtMs = 300,
        )
    }
}
