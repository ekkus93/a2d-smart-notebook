package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.A2dFfiException
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OcrRunStatus
import uniffi.a2d_ffi.OpenLibraryRequest
import uniffi.a2d_ffi.PrepareOcrInputRequest
import uniffi.a2d_ffi.RecordOcrRunRequest

@RunWith(AndroidJUnit4::class)
class OcrBridgeIntegrationTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun prepareOcrInputCrossesGeneratedBindingAndPreservesStructuredRustErrors() {
        val root = context.filesDir.resolve("ocr-prepare-ffi-${UUID.randomUUID()}")

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val error =
                assertThrows(A2dFfiException.Failed::class.java) {
                    client.prepareOcrInput(
                        PrepareOcrInputRequest(
                            scanId = client.generatePageId(),
                            inputKind = OcrInputKind.ORIGINAL,
                            widthPx = 0u,
                            heightPx = 1_000u,
                        ),
                    )
                }

            assertEquals("CORE_OCR_IMAGE_DIMENSIONS_INVALID", error.v1.code)
            assertEquals("ocr", error.v1.category.lowercase())
            assertFalse(error.v1.retryable)
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun recordOcrRunCrossesGeneratedBindingAndDoesNotBypassRustScanValidation() {
        val root = context.filesDir.resolve("ocr-record-ffi-${UUID.randomUUID()}")

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val error =
                assertThrows(A2dFfiException.Failed::class.java) {
                    client.recordOcrRun(
                        RecordOcrRunRequest(
                            scanId = client.generatePageId(),
                            inputAssetId = client.generatePageId(),
                            provider = "mlkit",
                            providerVersion = "2026.09",
                            modelName = "latin-v1",
                            status = OcrRunStatus.NO_TEXT_DETECTED,
                            fullText = "",
                            unavailableReason = null,
                            unavailableMessage = null,
                            completedAtMs = 250,
                            warnings = emptyList(),
                        ),
                    )
                }

            assertEquals("CORE_OCR_RECORD_SCAN_MISSING", error.v1.code)
            assertEquals("ocr", error.v1.category.lowercase())
            assertFalse(error.v1.retryable)
        } finally {
            root.deleteRecursively()
        }
    }
}
