package com.a2d.notebook.feature.scanner.camera

import com.a2d.notebook.rustbridge.AnalyzedPageQuality
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import com.a2d.notebook.rustbridge.LivePageAnalysisPolicy
import com.a2d.notebook.rustbridge.LivePageAnalyzer
import com.a2d.notebook.rustbridge.PageMarkerIds
import java.nio.ByteBuffer
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LiveFrameAnalysisSchedulerTest {
    @Test
    fun analysisRunsOffCallerThreadAndReportsCopyAndGeometryMetrics() {
        val events = Collections.synchronizedList(mutableListOf<LiveFrameAnalysisEvent>())
        val completed = CountDownLatch(1)
        var analyzerThread = ""
        val executor =
            Executors.newSingleThreadExecutor { runnable -> Thread(runnable, "test-native-analysis") }
        val scheduler =
            LatestFrameAnalysisScheduler(
                analyzer =
                    LivePageAnalyzer { _, _ ->
                        analyzerThread = Thread.currentThread().name
                        result(width = 2, height = 2)
                    },
                policy = policy(),
                onEvent = {
                    events += it
                    if (it is LiveFrameAnalysisEvent.Succeeded) completed.countDown()
                },
                executor = executor,
            )

        val callerThread = Thread.currentThread().name
        scheduler.submit(frame(timestamp = 10L, rotation = 90))

        assertTrue(completed.await(2, TimeUnit.SECONDS))
        val success = events.filterIsInstance<LiveFrameAnalysisEvent.Succeeded>().single()
        assertEquals("test-native-analysis", analyzerThread)
        assertFalse(callerThread == analyzerThread)
        assertEquals(2, success.metrics.width)
        assertEquals(2, success.metrics.height)
        assertEquals(4, success.metrics.sourceRowStride)
        assertEquals(2, success.metrics.sourcePixelStride)
        assertEquals(2, success.metrics.packedRowStride)
        assertEquals(90, success.metrics.rotationDegrees)
        assertEquals(4, success.metrics.inputBytes)
        assertEquals(1, success.metrics.pixelBufferCopies)
        assertEquals(0, success.metrics.ffiInputCopies)
        assertEquals(1, success.metrics.resultPayloadCopies)
        scheduler.close()
    }

    @Test
    fun oneInFlightFrameAndOnlyTheNewestPendingFrameAreAnalyzed() {
        val events = Collections.synchronizedList(mutableListOf<LiveFrameAnalysisEvent>())
        val firstEntered = CountDownLatch(1)
        val releaseFirst = CountDownLatch(1)
        val newestCompleted = CountDownLatch(1)
        val analyzedTimestamps = Collections.synchronizedList(mutableListOf<Long>())
        val calls = AtomicInteger(0)
        val scheduler =
            LatestFrameAnalysisScheduler(
                analyzer =
                    LivePageAnalyzer { frame, _ ->
                        analyzedTimestamps += frame.timestampNanos
                        if (calls.incrementAndGet() == 1) {
                            firstEntered.countDown()
                            assertTrue(releaseFirst.await(2, TimeUnit.SECONDS))
                        }
                        result(width = frame.width, height = frame.height)
                    },
                policy = policy(),
                onEvent = {
                    events += it
                    if (
                        it is LiveFrameAnalysisEvent.Succeeded &&
                        it.metrics.frameTimestampNanos == 3L
                    ) {
                        newestCompleted.countDown()
                    }
                },
            )

        val firstSequence = scheduler.submit(frame(timestamp = 1L))
        assertTrue(firstEntered.await(2, TimeUnit.SECONDS))
        val secondSequence = scheduler.submit(frame(timestamp = 2L))
        scheduler.submit(frame(timestamp = 3L))
        releaseFirst.countDown()

        assertTrue(newestCompleted.await(3, TimeUnit.SECONDS))
        assertEquals(listOf(1L, 3L), analyzedTimestamps.toList())
        assertTrue(
            events.any {
                it is LiveFrameAnalysisEvent.Dropped &&
                    it.frameSequence == secondSequence &&
                    it.reason == LiveFrameDropReason.SUPERSEDED_BEFORE_START
            },
        )
        assertTrue(
            events.any {
                it is LiveFrameAnalysisEvent.StaleResultDiscarded &&
                    it.metrics.frameSequence == firstSequence &&
                    it.reason == LiveFrameDropReason.SUPERSEDED_IN_FLIGHT
            },
        )
        assertFalse(
            events.filterIsInstance<LiveFrameAnalysisEvent.Succeeded>()
                .any { it.metrics.frameTimestampNanos == 1L },
        )
        scheduler.close()
    }

    @Test
    fun cancellationInvalidatesAnInFlightCompletion() {
        val events = Collections.synchronizedList(mutableListOf<LiveFrameAnalysisEvent>())
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val stale = CountDownLatch(1)
        val scheduler =
            LatestFrameAnalysisScheduler(
                analyzer =
                    LivePageAnalyzer { frame, _ ->
                        entered.countDown()
                        assertTrue(release.await(2, TimeUnit.SECONDS))
                        result(width = frame.width, height = frame.height)
                    },
                policy = policy(),
                onEvent = {
                    events += it
                    if (it is LiveFrameAnalysisEvent.StaleResultDiscarded) stale.countDown()
                },
            )

        scheduler.submit(frame(timestamp = 40L))
        assertTrue(entered.await(2, TimeUnit.SECONDS))
        scheduler.cancel()
        release.countDown()

        assertTrue(stale.await(2, TimeUnit.SECONDS))
        assertTrue(
            events.any {
                it is LiveFrameAnalysisEvent.StaleResultDiscarded &&
                    it.reason == LiveFrameDropReason.CANCELLED
            },
        )
        assertTrue(events.none { it is LiveFrameAnalysisEvent.Succeeded })
        scheduler.close()
    }

    @Test
    fun analyzerFailuresAreExplicitAndSchedulerContinuesWithTheNextFrame() {
        val events = Collections.synchronizedList(mutableListOf<LiveFrameAnalysisEvent>())
        val success = CountDownLatch(1)
        val calls = AtomicInteger(0)
        val scheduler =
            LatestFrameAnalysisScheduler(
                analyzer =
                    LivePageAnalyzer { frame, _ ->
                        if (calls.incrementAndGet() == 1) throw IllegalStateException("native failed")
                        result(width = frame.width, height = frame.height)
                    },
                policy = policy(),
                onEvent = {
                    events += it
                    if (it is LiveFrameAnalysisEvent.Succeeded) success.countDown()
                },
            )

        scheduler.submit(frame(timestamp = 50L))
        waitUntil { events.any { it is LiveFrameAnalysisEvent.Failed } }
        scheduler.submit(frame(timestamp = 51L))

        assertTrue(success.await(2, TimeUnit.SECONDS))
        val failure = events.filterIsInstance<LiveFrameAnalysisEvent.Failed>().single()
        assertEquals("native failed", failure.message)
        assertEquals(50L, failure.metrics.frameTimestampNanos)
        scheduler.close()
    }

    @Test
    fun closeRejectsLaterSubmissionsInsteadOfDroppingThemSilently() {
        val events = mutableListOf<LiveFrameAnalysisEvent>()
        val scheduler =
            LatestFrameAnalysisScheduler(
                analyzer = LivePageAnalyzer { _, _ -> result(2, 2) },
                policy = policy(),
                onEvent = events::add,
            )

        scheduler.close()
        val sequence = scheduler.submit(frame(timestamp = 60L))

        assertTrue(events.first() is LiveFrameAnalysisEvent.Closed)
        assertTrue(
            events.any {
                it is LiveFrameAnalysisEvent.SubmissionRejected &&
                    it.frameSequence == sequence
            },
        )
    }

    private fun policy() =
        LivePageAnalysisPolicy(
            maxPixels = 100,
            detectorThreadCount = 1,
            detectorQuadDecimate = 1.0,
            detectorQuadSigma = 0.0,
            detectorRefineEdges = true,
            detectorDecodeSharpening = 0.25,
            detectorBitsCorrected = 2,
            darkLuminanceCutoff = 32,
            highlightLuminanceCutoff = 245,
            qualityTileColumns = 2,
            qualityTileRows = 2,
            markerIds = PageMarkerIds(0, 1, 2, 3),
        )

    private fun frame(
        timestamp: Long,
        rotation: Int = 0,
    ): CameraAnalysisFrame {
        val luminance = ByteBuffer.allocateDirect(4)
        luminance.put(byteArrayOf(10, 20, 30, 40))
        luminance.flip()
        return CameraAnalysisFrame(
            width = 2,
            height = 2,
            sourceRowStride = 4,
            sourcePixelStride = 2,
            rotationDegrees = rotation,
            timestampNanos = timestamp,
            extractionDurationNanos = 7,
            pixelBufferCopyCount = 1,
            luminance = luminance,
        )
    }

    private fun result(
        width: Int,
        height: Int,
    ) = EncodedPageAnalysisResult(
        width = width.toLong(),
        height = height.toLong(),
        sourceRotationDegrees = 0,
        resolvedOrientationDegrees = 0,
        markers = emptyList(),
        unexpectedTagIds = emptyList(),
        quality =
            AnalyzedPageQuality(
                focusLaplacianVariance = null,
                focusInteriorSampleCount = null,
                meanLuminance = 10.0,
                luminanceStandardDeviation = 1.0,
                darkFraction = 0.0,
                highlightFraction = 0.0,
                maxTileHighlightFraction = 0.0,
                populatedTileCount = 4,
            ),
    )

    private fun waitUntil(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(2)
        while (!condition()) {
            if (System.nanoTime() >= deadline) {
                throw AssertionError("condition was not met before timeout")
            }
            Thread.sleep(5)
        }
    }
}