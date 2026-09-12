package com.a2d.notebook.feature.scanner.camera

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CameraCaptureCallbackGenerationTest {
    @Test
    fun callbackFromCurrentOpenGenerationIsDelivered() {
        assertTrue(
            shouldDeliverCameraCaptureCallback(
                closeRequested = false,
                captureGeneration = 7L,
                currentGeneration = 7L,
            ),
        )
    }

    @Test
    fun callbackFromReboundGenerationIsDropped() {
        assertFalse(
            shouldDeliverCameraCaptureCallback(
                closeRequested = false,
                captureGeneration = 7L,
                currentGeneration = 8L,
            ),
        )
    }

    @Test
    fun callbackAfterCloseIsDroppedEvenWhenGenerationMatches() {
        assertFalse(
            shouldDeliverCameraCaptureCallback(
                closeRequested = true,
                captureGeneration = 7L,
                currentGeneration = 7L,
            ),
        )
    }
}
