package com.a2d.notebook.feature.scanner.camera

import com.a2d.notebook.rustbridge.LivePageAnalysisPolicy
import com.a2d.notebook.rustbridge.LivePageAnalyzer
import com.a2d.notebook.rustbridge.NativeLivePageAnalyzer
import com.a2d.notebook.rustbridge.PageAnalysisResult
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadFactory

private fun liveAnalysisThreadFactory(): ThreadFactory = ThreadFactory { runnable ->
    Thread(runnable, "a2d-live-rust-analysis").apply { isDaemon = true }
}

enum class LiveFrameDropReason {
    SUPERSEDED_BEFORE_START,
    SUPERSEDED_IN_FLIGHT,
    CANCELLED,
    CLOSED,
}

data class LiveFrameAnalysisMetrics(
    val frameSequence: Long,
    val frameTimestampNanos: Long,
    val width: Int,
    val height: Int,
    val sourceRowStride: Int,
    val sourcePixelStride: Int,
    val packedRowStride: Int,
    val rotationDegrees: Int,
    val inputBytes: Int,
    val pixelBufferCopies: Int,
    val ffiInputCopies: Int,
    val resultPayloadCopies: Int,
    val extractionDurationNanos: Long,
    val queueDurationNanos: Long,
    val nativeBridgeDurationNanos: Long,
    val totalSchedulerDurationNanos: Long,
)

sealed interface LiveFrameAnalysisEvent {
    data class Succeeded(
        val result: PageAnalysisResult,
        val metrics: LiveFrameAnalysisMetrics,
    ) : LiveFrameAnalysisEvent

    data class Failed(
        val message: String,
        val cause: Exception,
        val metrics: LiveFrameAnalysisMetrics,
    ) : LiveFrameAnalysisEvent

    data class Dropped(
        val frameSequence: Long,
        val reason: LiveFrameDropReason,
    ) : LiveFrameAnalysisEvent

    data class StaleResultDiscarded(
        val reason: LiveFrameDropReason,
        val metrics: LiveFrameAnalysisMetrics,
    ) : LiveFrameAnalysisEvent

    data class CameraFailure(
        val message: String,
        val cause: Exception,
    ) : LiveFrameAnalysisEvent

    data class SubmissionRejected(
        val frameSequence: Long,
        val message: String,
    ) : LiveFrameAnalysisEvent

    data class InfrastructureFailure(
        val frameSequence: Long,
        val message: String,
        val cause: Exception,
    ) : LiveFrameAnalysisEvent

    data object Closed : LiveFrameAnalysisEvent
}

/**
 * Single-flight, keep-latest scheduler between CameraX and synchronous shared Rust analysis.
 *
 * At most one frame is inside the native bridge. While it runs, one pending slot retains only the
 * newest frame. Superseded pending frames are reported explicitly. A newer submission, cancellation,
 * or close invalidates any in-flight completion before it can update scanner state. The default
 * executor is a dedicated background thread; callbacks are delivered on either the submitting thread
 * (drop/rejection events) or that analysis thread (completion events), so UI owners must marshal them
 * to the main thread deliberately.
 */
class LatestFrameAnalysisScheduler(
    private val analyzer: LivePageAnalyzer,
    private val policy: LivePageAnalysisPolicy,
    private val onEvent: (LiveFrameAnalysisEvent) -> Unit,
    private val executor: ExecutorService =
        Executors.newSingleThreadExecutor(liveAnalysisThreadFactory()),
    private val clockNanos: () -> Long = System::nanoTime,
) : AutoCloseable {
    private data class PendingFrame(
        val sequence: Long,
        val generation: Long,
        val submittedAtNanos: Long,
        val frame: CameraAnalysisFrame,
    )

    private val lock = Any()
    private var nextSequence = 1L
    private var latestSequence = 0L
    private var generation = 0L
    private var pending: PendingFrame? = null
    private var workerScheduled = false
    private var closed = false

    fun submit(frame: CameraAnalysisFrame): Long {
        val submittedAtNanos = clockNanos()
        var dropped: PendingFrame? = null
        var shouldScheduleWorker = false
        var rejected = false
        val sequence: Long

        synchronized(lock) {
            sequence = nextSequence
            nextSequence = Math.incrementExact(nextSequence)
            if (closed) {
                rejected = true
            } else {
                latestSequence = sequence
                dropped = pending
                pending =
                    PendingFrame(
                        sequence = sequence,
                        generation = generation,
                        submittedAtNanos = submittedAtNanos,
                        frame = frame,
                    )
                if (!workerScheduled) {
                    workerScheduled = true
                    shouldScheduleWorker = true
                }
            }
        }

        if (rejected) {
            publish(
                LiveFrameAnalysisEvent.SubmissionRejected(
                    frameSequence = sequence,
                    message = "live frame analysis scheduler is closed",
                ),
            )
            return sequence
        }

        dropped?.let {
            publish(
                LiveFrameAnalysisEvent.Dropped(
                    frameSequence = it.sequence,
                    reason = LiveFrameDropReason.SUPERSEDED_BEFORE_START,
                ),
            )
        }

        if (shouldScheduleWorker) {
            try {
                executor.execute(::drain)
            } catch (failure: RejectedExecutionException) {
                val rejectedPending =
                    synchronized(lock) {
                        workerScheduled = false
                        generation = Math.incrementExact(generation)
                        pending.also { pending = null }
                    }
                val failedSequence = rejectedPending?.sequence ?: sequence
                publish(
                    LiveFrameAnalysisEvent.InfrastructureFailure(
                        frameSequence = failedSequence,
                        message = "live analysis executor rejected its worker",
                        cause = failure,
                    ),
                )
            }
        }
        return sequence
    }

    /** Invalidates queued and in-flight work without closing the scheduler. */
    fun cancel() {
        val dropped =
            synchronized(lock) {
                if (closed) return
                generation = Math.incrementExact(generation)
                pending.also { pending = null }
            }
        dropped?.let {
            publish(
                LiveFrameAnalysisEvent.Dropped(
                    frameSequence = it.sequence,
                    reason = LiveFrameDropReason.CANCELLED,
                ),
            )
        }
    }

    fun reportCameraFailure(
        message: String,
        cause: Exception,
    ) {
        publish(LiveFrameAnalysisEvent.CameraFailure(message, cause))
    }

    override fun close() {
        val dropped =
            synchronized(lock) {
                if (closed) return
                closed = true
                generation = Math.incrementExact(generation)
                pending.also { pending = null }
            }
        dropped?.let {
            publish(
                LiveFrameAnalysisEvent.Dropped(
                    frameSequence = it.sequence,
                    reason = LiveFrameDropReason.CLOSED,
                ),
            )
        }
        executor.shutdownNow()
        publish(LiveFrameAnalysisEvent.Closed)
    }

    private fun drain() {
        while (true) {
            val work =
                synchronized(lock) {
                    val next = pending
                    if (next == null) {
                        workerScheduled = false
                    } else {
                        pending = null
                    }
                    next
                } ?: return

            val startedAtNanos = clockNanos()
            val outcome: Result<PageAnalysisResult> =
                try {
                    Result.success(analyzer.analyze(work.frame, policy))
                } catch (failure: Exception) {
                    Result.failure(failure)
                } catch (fatal: Throwable) {
                    synchronized(lock) { workerScheduled = false }
                    throw fatal
                }
            val completedAtNanos = clockNanos()
            val metrics =
                metrics(
                    work = work,
                    startedAtNanos = startedAtNanos,
                    completedAtNanos = completedAtNanos,
                )
            val staleReason =
                synchronized(lock) {
                    when {
                        closed -> LiveFrameDropReason.CLOSED
                        work.generation != generation -> LiveFrameDropReason.CANCELLED
                        work.sequence != latestSequence ->
                            LiveFrameDropReason.SUPERSEDED_IN_FLIGHT
                        else -> null
                    }
                }

            if (staleReason != null) {
                publish(
                    LiveFrameAnalysisEvent.StaleResultDiscarded(
                        reason = staleReason,
                        metrics = metrics,
                    ),
                )
            } else {
                outcome.fold(
                    onSuccess = { result ->
                        publish(LiveFrameAnalysisEvent.Succeeded(result, metrics))
                    },
                    onFailure = { failure ->
                        val exception = failure as? Exception ?: throw failure
                        publish(
                            LiveFrameAnalysisEvent.Failed(
                                message = exception.message ?: "live Rust analysis failed",
                                cause = exception,
                                metrics = metrics,
                            ),
                        )
                    },
                )
            }
        }
    }

    private fun metrics(
        work: PendingFrame,
        startedAtNanos: Long,
        completedAtNanos: Long,
    ): LiveFrameAnalysisMetrics =
        LiveFrameAnalysisMetrics(
            frameSequence = work.sequence,
            frameTimestampNanos = work.frame.timestampNanos,
            width = work.frame.width,
            height = work.frame.height,
            sourceRowStride = work.frame.sourceRowStride,
            sourcePixelStride = work.frame.sourcePixelStride,
            packedRowStride = work.frame.packedRowStride,
            rotationDegrees = work.frame.rotationDegrees,
            inputBytes = work.frame.luminanceByteCount,
            pixelBufferCopies = work.frame.pixelBufferCopyCount,
            ffiInputCopies = 0,
            resultPayloadCopies = 1,
            extractionDurationNanos = work.frame.extractionDurationNanos,
            queueDurationNanos = elapsed(work.submittedAtNanos, startedAtNanos, "queue"),
            nativeBridgeDurationNanos = elapsed(startedAtNanos, completedAtNanos, "native bridge"),
            totalSchedulerDurationNanos =
                elapsed(work.submittedAtNanos, completedAtNanos, "total scheduler"),
        )

    private fun elapsed(
        start: Long,
        end: Long,
        label: String,
    ): Long {
        require(end >= start) { "$label clock moved backwards: start=$start end=$end" }
        return Math.subtractExact(end, start)
    }

    private fun publish(event: LiveFrameAnalysisEvent) {
        try {
            onEvent(event)
        } catch (failure: Exception) {
            synchronized(lock) {
                closed = true
                generation = Math.incrementExact(generation)
                pending = null
                workerScheduled = false
            }
            executor.shutdownNow()
            throw IllegalStateException(
                "live analysis event callback failed; scheduler was closed",
                failure,
            )
        }
    }
}

/** CameraX event adapter suitable for passing directly as `onAnalysisEvent`. */
class LiveCameraAnalysisPipeline(
    private val scheduler: LatestFrameAnalysisScheduler,
) : AutoCloseable {
    fun onCameraEvent(event: CameraAnalysisEvent) {
        when (event) {
            is CameraAnalysisEvent.Frame -> scheduler.submit(event.frame)
            is CameraAnalysisEvent.Failure ->
                scheduler.reportCameraFailure(event.message, event.cause)
        }
    }

    fun cancel() = scheduler.cancel()

    override fun close() = scheduler.close()

    companion object {
        fun native(
            policy: LivePageAnalysisPolicy,
            onEvent: (LiveFrameAnalysisEvent) -> Unit,
        ): LiveCameraAnalysisPipeline =
            LiveCameraAnalysisPipeline(
                LatestFrameAnalysisScheduler(
                    analyzer = NativeLivePageAnalyzer,
                    policy = policy,
                    onEvent = onEvent,
                ),
            )
    }
}