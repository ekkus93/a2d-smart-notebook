package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.capture.AutoCaptureRequest
import com.a2d.notebook.feature.scanner.capture.CaptureRequestToken
import com.a2d.notebook.feature.scanner.capture.CaptureTrigger
import com.a2d.notebook.rustbridge.ProcessedRgbImage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution

class SinglePageScannerSafetyTest {
    private val request =
        AutoCaptureRequest(
            token = CaptureRequestToken(generation = 2L, sequence = 3L),
            pageId = "page-a",
            activeNotebookId = "notebook-a",
            trigger = CaptureTrigger.MANUAL,
            requestedAtNanos = 10L,
        )

    @Test
    fun finalIdentityAllowsApprovalOnlyForExactPageAndNotebook() {
        val matching =
            assessFinalCaptureIdentity(
                PageResolution.Resolved(pageId = "page-a", notebookId = "notebook-a"),
                request,
            )
        val wrongPage =
            assessFinalCaptureIdentity(
                PageResolution.Resolved(pageId = "page-b", notebookId = "notebook-a"),
                request,
            )
        val wrongNotebook =
            assessFinalCaptureIdentity(
                PageResolution.Resolved(pageId = "page-a", notebookId = "notebook-b"),
                request,
            )

        assertTrue(matching.approvalAllowed)
        assertEquals(null, matching.warning)
        assertFalse(wrongPage.approvalAllowed)
        assertNotNull(wrongPage.warning)
        assertFalse(wrongNotebook.approvalAllowed)
        assertNotNull(wrongNotebook.warning)
    }

    @Test
    fun decoderFailureCanNeverBeOverriddenByAResolution() {
        val result =
            assessFinalCaptureIdentity(
                resolution = PageResolution.Resolved("page-a", "notebook-a"),
                request = request,
                decoderWarning = "decoder failed",
            )

        assertFalse(result.approvalAllowed)
        assertEquals("decoder failed", result.warning)
    }

    @Test
    fun manualCaptureRequiresABoundCameraAndActiveNotebook() {
        val notebook =
            NotebookSummary(
                id = "notebook-a",
                designId = "design-a",
                displayName = "Field Notes",
                archived = false,
                active = true,
            )

        assertFalse(SinglePageScannerUiState(activeNotebook = notebook).canCaptureManually)
        assertTrue(
            SinglePageScannerUiState(
                activeNotebook = notebook,
                cameraState = CameraAdapterState.Bound(torchAvailable = true, torchEnabled = false),
            ).canCaptureManually,
        )
    }

    @Test
    fun rgbConversionPreservesUnsignedChannelsAndOpaqueAlpha() {
        val pixels =
            ProcessedRgbImage(
                width = 2,
                height = 1,
                bytes = byteArrayOf(0, 127, -1, -1, 0, 64),
            ).toArgbPixels()

        assertEquals(0xff007fff.toInt(), pixels[0])
        assertEquals(0xffff0040.toInt(), pixels[1])
    }
}
