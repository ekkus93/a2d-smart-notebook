package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.BeginScannerRecoveryRequest
import uniffi.a2d_ffi.OpenLibraryRequest
import uniffi.a2d_ffi.ScannerRecoveryPhase

@RunWith(AndroidJUnit4::class)
class ScannerRecoveryBridgeTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun scannerJournalSurvivesClientRecreationAndDiscardRemovesOnlyStagingFile() {
        val root = context.filesDir.resolve("scanner-recovery-test-${UUID.randomUUID()}")
        val staging = root.resolve("tmp/scanner-staging/capture.jpg")
        assertTrue(staging.parentFile?.mkdirs() == true)
        staging.writeBytes(byteArrayOf(1, 2, 3, 4))
        val token = "android-${UUID.randomUUID()}"

        try {
            val first = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val pageId = first.generatePageId()
            val created =
                first.beginScannerRecovery(
                    BeginScannerRecoveryRequest(
                        token = token,
                        stagingPath = staging.canonicalPath,
                        pageId = pageId,
                        notebookId = "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                        capturedAtMs = System.currentTimeMillis(),
                        layoutId = "USLETTER-LINED",
                        processingPolicyVersion = 1u,
                    ),
                )
            assertEquals(ScannerRecoveryPhase.CAPTURED, created.phase)
            assertTrue(staging.isFile)

            val recreated = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val restored = recreated.listScannerRecoveries().single { it.token == token }
            assertEquals(pageId, restored.pageId)
            assertEquals(ScannerRecoveryPhase.CAPTURED, restored.phase)

            val previewReady = recreated.markScannerRecoveryPreviewReady(token)
            assertEquals(ScannerRecoveryPhase.PREVIEW_READY, previewReady.phase)
            recreated.discardScannerRecovery(token)

            assertFalse(staging.exists())
            assertTrue(recreated.listScannerRecoveries().none { it.token == token })
        } finally {
            root.deleteRecursively()
        }
    }
}
