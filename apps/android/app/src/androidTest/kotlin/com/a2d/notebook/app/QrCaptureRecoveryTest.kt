package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.createQrCapture
import com.a2d.notebook.rustbridge.listOrphanedQrCaptureFiles
import com.a2d.notebook.rustbridge.resolvePendingQrCapture
import java.io.File
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class QrCaptureRecoveryTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun pendingCapturePathSurvivesRecreationAndResolvesInsideApprovedRoot() {
        val capture = createQrCapture(context, "recovery-")
        capture.file.writeBytes(byteArrayOf(1, 2, 3))

        val restored = resolvePendingQrCapture(context, capture.file.absolutePath)

        assertEquals(capture.file.canonicalFile, restored)
        assertTrue(restored.isFile)
        assertTrue(restored.delete())
    }

    @Test
    fun missingAndOutsidePathsFailExplicitly() {
        val missing = context.cacheDir.resolve("qr-capture/missing.jpg")
        assertIOException { resolvePendingQrCapture(context, missing.absolutePath) }

        val outside = File.createTempFile("outside-qr-", ".jpg", context.cacheDir)
        try {
            assertIOException { resolvePendingQrCapture(context, outside.absolutePath) }
        } finally {
            outside.delete()
        }
    }

    @Test
    fun orphanDiscoveryExcludesPendingAndNeverDeletesFiles() {
        val pending = createQrCapture(context, "pending-")
        val orphan = createQrCapture(context, "orphan-")
        pending.file.writeBytes(byteArrayOf(1))
        orphan.file.writeBytes(byteArrayOf(2))

        val orphans = listOrphanedQrCaptureFiles(context, pending.file.absolutePath).getOrThrow()

        assertTrue(orphans.contains(orphan.file.canonicalFile))
        assertFalse(orphans.contains(pending.file.canonicalFile))
        assertTrue(pending.file.exists())
        assertTrue(orphan.file.exists())

        pending.file.delete()
        orphan.file.delete()
    }

    private fun assertIOException(block: () -> Unit) {
        try {
            block()
        } catch (_: IOException) {
            return
        }
        throw AssertionError("expected IOException")
    }
}
