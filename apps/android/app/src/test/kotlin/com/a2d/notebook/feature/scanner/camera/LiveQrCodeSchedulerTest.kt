package com.a2d.notebook.feature.scanner.camera

import java.nio.ByteBuffer
import java.util.concurrent.AbstractExecutorService
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LiveQrCodeSchedulerTest {
    @Test
    fun rotationNormalizesAllSupportedOrientations() {
        val source = byteArrayOf(1, 2, 3, 4, 5, 6)

        assertArrayEquals(source, rotateGray8(source, 3, 2, 0).bytes)
        assertArrayEquals(
            byteArrayOf(4, 1, 5, 2, 6, 3),
            rotateGray8(source, 3, 2, 90).bytes,
        )
        assertArrayEquals(
            byteArrayOf(6, 5, 4, 3, 2, 1),
            rotateGray8(source, 3, 2, 180).bytes,
        )
        assertArrayEquals(
            byteArrayOf(3, 6, 2, 5, 1, 4),
            rotateGray8(source, 3, 2, 270).bytes,
        )
        assertEquals(2, rotateGray8(source, 3, 2, 90).width)
        assertEquals(3, rotateGray8(source, 3, 2, 90).height)
    }

    @Test
    fun keepLatestDropsPendingAndDiscardsSupersededInFlightResult() {
        val executor = ManualExecutor()
        val events = mutableListOf<LiveQrCodeEvent>()
        val scheduler =
            LiveQrCodeScheduler(
                decoder = GrayQrCodeDecoder { frame -> "page-${frame.timestampNanos}" },
                onEvent = events::add,
                executor = executor,
            )

        val first = scheduler.submit(frame(timestamp = 1))
        val second = scheduler.submit(frame(timestamp = 2))
        val third = scheduler.submit(frame(timestamp = 3))
        executor.runAll()

        assertEquals(1L, first)
        assertEquals(2L, second)
        assertEquals(3L, third)
        assertTrue(
            events.any {
                it == LiveQrCodeEvent.Dropped(
                    frameSequence = second,
                    reason = LiveQrDropReason.SUPERSEDED_BEFORE_START,
                )
            },
        )
        val found = events.filterIsInstance<LiveQrCodeEvent.Found>().single()
        assertEquals(third, found.frameSequence)
        assertEquals("page-3", found.payload)
        assertEquals(1, found.pixelBufferCopies)
    }

    @Test
    fun noCodeAndDecoderFailureRemainDistinct() {
        val noCode = mutableListOf<LiveQrCodeEvent>()
        LiveQrCodeScheduler(
            decoder = GrayQrCodeDecoder { null },
            onEvent = noCode::add,
            executor = ImmediateExecutor(),
        ).submit(frame(timestamp = 10))
        assertTrue(noCode.single() is LiveQrCodeEvent.NotFound)

        val failed = mutableListOf<LiveQrCodeEvent>()
        LiveQrCodeScheduler(
            decoder = GrayQrCodeDecoder { throw IllegalStateException("decoder broke") },
            onEvent = failed::add,
            executor = ImmediateExecutor(),
        ).submit(frame(timestamp = 11))
        val failure = failed.single() as LiveQrCodeEvent.Failed
        assertEquals("decoder broke", failure.message)
    }

    @Test
    fun cancelInvalidatesQueuedWorkAndCloseRejectsNewFrames() {
        val executor = ManualExecutor()
        val events = mutableListOf<LiveQrCodeEvent>()
        val scheduler =
            LiveQrCodeScheduler(
                decoder = GrayQrCodeDecoder { "payload" },
                onEvent = events::add,
                executor = executor,
            )
        val queued = scheduler.submit(frame(timestamp = 20))
        scheduler.cancel()
        executor.runAll()
        assertTrue(
            events.any {
                it == LiveQrCodeEvent.Dropped(queued, LiveQrDropReason.CANCELLED)
            },
        )

        scheduler.close()
        val rejected = scheduler.submit(frame(timestamp = 21))
        assertTrue(
            events.any {
                it == LiveQrCodeEvent.SubmissionRejected(rejected, "live QR scheduler is closed")
            },
        )
    }

    private fun frame(timestamp: Long): CameraAnalysisFrame {
        val buffer = ByteBuffer.allocateDirect(4).apply {
            put(byteArrayOf(1, 2, 3, 4))
            flip()
        }
        return CameraAnalysisFrame(
            width = 2,
            height = 2,
            sourceRowStride = 2,
            sourcePixelStride = 1,
            rotationDegrees = 0,
            timestampNanos = timestamp,
            extractionDurationNanos = 1,
            pixelBufferCopyCount = 1,
            luminance = buffer,
        )
    }
}

private class ImmediateExecutor : AbstractExecutorService() {
    private var shutdown = false

    override fun execute(command: Runnable) = command.run()
    override fun shutdown() {
        shutdown = true
    }
    override fun shutdownNow(): MutableList<Runnable> {
        shutdown = true
        return mutableListOf()
    }
    override fun isShutdown(): Boolean = shutdown
    override fun isTerminated(): Boolean = shutdown
    override fun awaitTermination(timeout: Long, unit: TimeUnit): Boolean = true
}

private class ManualExecutor : AbstractExecutorService() {
    private val tasks = ArrayDeque<Runnable>()
    private var shutdown = false

    override fun execute(command: Runnable) {
        check(!shutdown)
        tasks.addLast(command)
    }

    fun runAll() {
        while (tasks.isNotEmpty()) tasks.removeFirst().run()
    }

    override fun shutdown() {
        shutdown = true
    }
    override fun shutdownNow(): MutableList<Runnable> {
        shutdown = true
        return tasks.toMutableList().also { tasks.clear() }
    }
    override fun isShutdown(): Boolean = shutdown
    override fun isTerminated(): Boolean = shutdown && tasks.isEmpty()
    override fun awaitTermination(timeout: Long, unit: TimeUnit): Boolean = isTerminated
}
