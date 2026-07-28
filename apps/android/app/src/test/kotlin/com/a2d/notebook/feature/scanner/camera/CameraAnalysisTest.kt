package com.a2d.notebook.feature.scanner.camera

import java.nio.ByteBuffer
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class CameraAnalysisTest {
    @Test
    fun packedPlaneCopiesWithoutChangingSourcePosition() {
        val source = ByteBuffer.wrap(byteArrayOf(1, 2, 3, 4, 5, 6)).apply { position(2) }

        val copied = copyLuminancePlane(
            source = source,
            imageWidth = 3,
            imageHeight = 2,
            crop = LuminanceCrop(left = 0, top = 0, width = 3, height = 2),
            rowStride = 3,
            pixelStride = 1,
        )

        assertArrayEquals(byteArrayOf(1, 2, 3, 4, 5, 6), copied)
        assertEquals(2, source.position())
    }

    @Test
    fun rowPaddingIsExcluded() {
        val source = ByteBuffer.wrap(
            byteArrayOf(
                1, 2, 3, 99, 99,
                4, 5, 6, 88, 88,
            ),
        )

        val copied = copyLuminancePlane(
            source = source,
            imageWidth = 3,
            imageHeight = 2,
            crop = LuminanceCrop(left = 0, top = 0, width = 3, height = 2),
            rowStride = 5,
            pixelStride = 1,
        )

        assertArrayEquals(byteArrayOf(1, 2, 3, 4, 5, 6), copied)
    }

    @Test
    fun pixelStrideAndCropAreBothHonored() {
        val source = ByteBuffer.wrap(
            byteArrayOf(
                10, 90, 11, 90, 12, 90, 13, 90,
                20, 80, 21, 80, 22, 80, 23, 80,
                30, 70, 31, 70, 32, 70, 33, 70,
            ),
        )

        val copied = copyLuminancePlane(
            source = source,
            imageWidth = 4,
            imageHeight = 3,
            crop = LuminanceCrop(left = 1, top = 1, width = 2, height = 2),
            rowStride = 8,
            pixelStride = 2,
        )

        assertArrayEquals(byteArrayOf(21, 22, 31, 32), copied)
    }

    @Test(expected = IllegalArgumentException::class)
    fun undersizedPlaneIsRejected() {
        copyLuminancePlane(
            source = ByteBuffer.wrap(byteArrayOf(1, 2, 3)),
            imageWidth = 3,
            imageHeight = 2,
            crop = LuminanceCrop(left = 0, top = 0, width = 3, height = 2),
            rowStride = 3,
            pixelStride = 1,
        )
    }

    @Test
    fun closeAfterClosesOnSuccess() {
        val resource = FakeCloseable()

        val result = closeAfter(resource) { "ok" }

        assertTrue(result.isSuccess)
        assertEquals("ok", result.getOrThrow())
        assertTrue(resource.closed)
    }

    @Test
    fun closeAfterClosesOnProcessingFailureAndPreservesTheFailure() {
        val resource = FakeCloseable()
        val processingFailure = IllegalStateException("processing failed")

        val result = closeAfter(resource) { throw processingFailure }

        assertTrue(result.isFailure)
        assertSame(processingFailure, result.exceptionOrNull())
        assertTrue(resource.closed)
    }

    @Test
    fun closeFailureIsSuppressedBehindTheOriginalProcessingFailure() {
        val closeFailure = IllegalArgumentException("close failed")
        val resource = FakeCloseable(closeFailure)
        val processingFailure = IllegalStateException("processing failed")

        val result = closeAfter(resource) { throw processingFailure }

        assertFalse(result.isSuccess)
        assertSame(processingFailure, result.exceptionOrNull())
        assertArrayEquals(arrayOf(closeFailure), processingFailure.suppressed)
    }

    private class FakeCloseable(
        private val closeFailure: Throwable? = null,
    ) : AutoCloseable {
        var closed = false
            private set

        override fun close() {
            closed = true
            closeFailure?.let { throw it }
        }
    }
}