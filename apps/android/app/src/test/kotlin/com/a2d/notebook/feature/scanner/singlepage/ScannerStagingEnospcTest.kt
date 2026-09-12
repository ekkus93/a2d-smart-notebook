package com.a2d.notebook.feature.scanner.singlepage

import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.lang.reflect.InvocationTargetException
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

class ScannerStagingEnospcTest {
    @Test
    fun productionReservationWriterFailsOnRealEnospcWithoutValidReservation() {
        val mountPath = System.getenv("A2D_ENOSPC_ROOT")
        assumeTrue("A2D_ENOSPC_ROOT is only provided by the dedicated CI gate", !mountPath.isNullOrBlank())
        val root = File(mountPath!!, "android-staging")
        assertTrue(root.mkdirs())
        val filler = root.resolve("filler.bin")
        val staging = root.resolve("capture.jpg")

        try {
            var sawEnospc = false
            FileOutputStream(filler).use { stream ->
                val chunk = ByteArray(256 * 1024) { 0x5a.toByte() }
                while (true) {
                    try {
                        stream.write(chunk)
                    } catch (failure: IOException) {
                        sawEnospc =
                            failure.message?.contains("No space left on device", ignoreCase = true) == true
                        if (!sawEnospc) throw failure
                        break
                    }
                }
            }
            assertTrue("dedicated filesystem must produce a real ENOSPC", sawEnospc)

            val owner =
                Class.forName(
                    "com.a2d.notebook.feature.scanner.singlepage.SinglePageScannerViewModelKt",
                )
            val writer = owner.getDeclaredMethod("writeCameraCaptureReservation", File::class.java)
            writer.isAccessible = true
            val reflectedFailure = runCatching { writer.invoke(null, staging) }.exceptionOrNull()
            assertTrue(reflectedFailure is InvocationTargetException)
            val cause = (reflectedFailure as InvocationTargetException).cause
            assertTrue(cause is IOException)
            assertTrue(
                "production staging failure must be real ENOSPC: ${cause?.message}",
                cause?.message?.contains("No space left on device", ignoreCase = true) == true,
            )
            assertFalse(
                "a failed low-storage write must not look like a valid durable reservation",
                staging.isFile && staging.readBytes().contentEquals(
                    "A2D_CAMERA_CAPTURE_RESERVED_V1\n".encodeToByteArray(),
                ),
            )
        } finally {
            filler.delete()
            staging.delete()
            root.deleteRecursively()
        }
    }
}
