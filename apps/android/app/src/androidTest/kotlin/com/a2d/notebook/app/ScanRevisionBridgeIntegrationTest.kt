package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.scanRevisionProposal
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.A2dFfiException
import uniffi.a2d_ffi.OpenLibraryRequest

@RunWith(AndroidJUnit4::class)
class ScanRevisionBridgeIntegrationTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun revisionProposalCrossesGeneratedBindingAndPreservesStructuredRustErrors() {
        val root = context.filesDir.resolve("scan-revision-ffi-${UUID.randomUUID()}")

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val syntacticallyValidCandidateId = client.generatePageId()

            val error =
                assertThrows(A2dFfiException.Failed::class.java) {
                    client.scanRevisionProposal(
                        candidateScanId = syntacticallyValidCandidateId,
                        minimumCellAbsoluteDifference = 256u,
                    )
                }

            assertEquals("FFI_SCAN_REVISION_THRESHOLD_OUT_OF_RANGE", error.v1.code)
            assertEquals("validation", error.v1.category.lowercase())
        } finally {
            root.deleteRecursively()
        }
    }
}
