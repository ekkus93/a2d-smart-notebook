package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.loadReviewItems
import java.util.UUID
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OpenLibraryRequest

@RunWith(AndroidJUnit4::class)
class NeedsReviewBridgeIntegrationTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun emptyReviewQueueCrossesThePackagedGeneratedBinding() {
        val root = context.filesDir.resolve("needs-review-ffi-${UUID.randomUUID()}")
        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val page = client.loadReviewItems(limit = 10u)
            assertTrue(page.items.isEmpty())
            assertFalse(page.hasMore)
            assertNull(page.nextOffset)
        } finally {
            root.deleteRecursively()
        }
    }
}
