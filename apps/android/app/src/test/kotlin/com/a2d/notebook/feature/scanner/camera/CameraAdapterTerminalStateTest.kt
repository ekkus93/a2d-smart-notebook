package com.a2d.notebook.feature.scanner.camera

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

class CameraAdapterTerminalStateTest {
    @Test
    fun successfulCloseHasNoCleanupWarning() {
        val closed = cameraClosedState(null)

        assertNull(closed.cleanupWarning)
    }

    @Test
    fun cleanupFailureRemainsObservableInClosedState() {
        val failure = IllegalStateException("unbind failed")
        val closed = cameraClosedState(failure)

        assertEquals("unbind failed", closed.cleanupWarning?.message)
        assertSame(failure, closed.cleanupWarning?.cause)
    }

    @Test
    fun missingCleanupMessageGetsAnExplicitFallback() {
        val failure = object : Exception() {}
        val closed = cameraClosedState(failure)

        assertEquals("CameraX cleanup failed", closed.cleanupWarning?.message)
        assertSame(failure, closed.cleanupWarning?.cause)
    }
}
