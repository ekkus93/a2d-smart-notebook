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
    fun scannerJournalExistsBeforeCameraWriteAndSurvivesCallbackLoss() {
        val root = context.filesDir.resolve("scanner-recovery-test-${UUID.randomUUID()}")
        val staging = root.resolve("tmp/scanner-staging/capture.jpg")
        assertTrue(staging.parentFile?.mkdirs() == true)
        staging.writeText("A2D_CAMERA_CAPTURE_RESERVED_V1\n")
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

            val beforeCameraCallback =
                A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val reserved = beforeCameraCallback.listScannerRecoveries().single { it.token == token }
            assertEquals(pageId, reserved.pageId)
            assertEquals(ScannerRecoveryPhase.CAPTURED, reserved.phase)

            // Simulate CameraX replacing the reservation and the process dying before its callback.
            staging.writeBytes(byteArrayOf(1, 2, 3, 4))
            val afterCallbackLoss =
                A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val restored = afterCallbackLoss.listScannerRecoveries().single { it.token == token }
            assertEquals(ScannerRecoveryPhase.CAPTURED, restored.phase)

            val previewReady = afterCallbackLoss.markScannerRecoveryPreviewReady(token)
            assertEquals(ScannerRecoveryPhase.PREVIEW_READY, previewReady.phase)
            afterCallbackLoss.discardScannerRecovery(token)

            assertFalse(staging.exists())
            assertTrue(afterCallbackLoss.listScannerRecoveries().none { it.token == token })
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun cameraFailureDiscardsPreparedJournalAndReservation() {
        val root = context.filesDir.resolve("scanner-recovery-abort-${UUID.randomUUID()}")
        val staging = root.resolve("tmp/scanner-staging/capture.jpg")
        assertTrue(staging.parentFile?.mkdirs() == true)
        staging.writeText("A2D_CAMERA_CAPTURE_RESERVED_V1\n")
        val token = "android-${UUID.randomUUID()}"

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            client.beginScannerRecovery(
                BeginScannerRecoveryRequest(
                    token = token,
                    stagingPath = staging.canonicalPath,
                    pageId = client.generatePageId(),
                    notebookId = "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                    capturedAtMs = System.currentTimeMillis(),
                    layoutId = "USLETTER-LINED",
                    processingPolicyVersion = 1u,
                ),
            )

            client.discardScannerRecovery(token)

            assertFalse(staging.exists())
            assertTrue(client.listScannerRecoveries().none { it.token == token })
        } finally {
            root.deleteRecursively()
        }
    }
}
