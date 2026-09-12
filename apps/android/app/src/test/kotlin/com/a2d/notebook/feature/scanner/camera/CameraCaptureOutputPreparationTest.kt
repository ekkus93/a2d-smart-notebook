package com.a2d.notebook.feature.scanner.camera

import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CameraCaptureOutputPreparationTest {
    @Test
    fun missingOutputPathIsReady() {
        val root = Files.createTempDirectory("a2d-camera-output").toFile()
        try {
            val output = root.resolve("capture.jpg")
            assertEquals(CameraCaptureOutputPreparation.READY, prepareCameraCaptureOutputFile(output))
            assertFalse(output.exists())
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun singlePageReservationIsVerifiedAndReleased() {
        assertReservationReleased("A2D_CAMERA_CAPTURE_RESERVED_V1\n")
    }

    @Test
    fun batchReservationIsVerifiedAndReleased() {
        assertReservationReleased("A2D_BATCH_CAMERA_CAPTURE_RESERVED_V1\n")
    }

    @Test
    fun arbitraryExistingContentIsNeverDeleted() {
        val root = Files.createTempDirectory("a2d-camera-output").toFile()
        try {
            val output = root.resolve("capture.jpg")
            output.writeText("user image bytes")

            assertEquals(
                CameraCaptureOutputPreparation.EXISTING_NON_RESERVATION,
                prepareCameraCaptureOutputFile(output),
            )
            assertTrue(output.isFile)
            assertEquals("user image bytes", output.readText())
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun existingDirectoryIsNeverTreatedAsReservation() {
        val root = Files.createTempDirectory("a2d-camera-output").toFile()
        try {
            val output = root.resolve("capture.jpg")
            assertTrue(output.mkdir())

            assertEquals(
                CameraCaptureOutputPreparation.EXISTING_NON_RESERVATION,
                prepareCameraCaptureOutputFile(output),
            )
            assertTrue(output.isDirectory)
        } finally {
            root.deleteRecursively()
        }
    }

    private fun assertReservationReleased(contents: String) {
        val root = Files.createTempDirectory("a2d-camera-output").toFile()
        try {
            val output = root.resolve("capture.jpg")
            output.writeText(contents)

            assertEquals(CameraCaptureOutputPreparation.READY, prepareCameraCaptureOutputFile(output))
            assertFalse(output.exists())
        } finally {
            root.deleteRecursively()
        }
    }
}
