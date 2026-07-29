package com.a2d.notebook.feature.smartpage

import androidx.lifecycle.SavedStateHandle
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class PendingSmartPageSaveStoreTest {
    @Test
    fun beginPersistsAndARecreatedStoreRestoresTheExactOperation() {
        val handle = SavedStateHandle()
        val first = PendingSmartPageSaveStore(handle) { "token-1" }

        val pending = first.begin("asset-1", "/library/assets/exports/asset-1").getOrThrow()
        val restored = PendingSmartPageSaveStore(handle).current()

        assertEquals(pending, restored)
        assertEquals("token-1", restored?.token)
        assertEquals("asset-1", restored?.assetId)
    }

    @Test
    fun duplicateBeginFailsWithoutReplacingTheOriginalOperation() {
        val handle = SavedStateHandle()
        val store = PendingSmartPageSaveStore(handle) { "token-1" }
        val original = store.begin("asset-1", "/exports/asset-1").getOrThrow()

        val duplicate = store.begin("asset-2", "/exports/asset-2")

        assertTrue(duplicate.isFailure)
        assertEquals(original, store.current())
    }

    @Test
    fun consumeReturnsTheExactOperationAndClearsSavedState() {
        val handle = SavedStateHandle()
        val store = PendingSmartPageSaveStore(handle) { "token-1" }
        val pending = store.begin("asset-1", "/exports/asset-1").getOrThrow()

        assertEquals(pending, store.consume().getOrThrow())
        assertNull(store.current())
        assertTrue(store.consume().isFailure)
        assertNull(PendingSmartPageSaveStore(handle).current())
    }

    @Test
    fun partialOrBlankRestorationIsClearedInsteadOfGuessed() {
        val partial = SavedStateHandle(
            mapOf("smart_pages.pending_save.token" to "token-only"),
        )
        assertNull(PendingSmartPageSaveStore(partial).current())
        assertNull(partial.get<String>("smart_pages.pending_save.token"))

        val blank = SavedStateHandle(
            mapOf(
                "smart_pages.pending_save.token" to "token-1",
                "smart_pages.pending_save.asset_id" to " ",
                "smart_pages.pending_save.path" to "/exports/asset-1",
            ),
        )
        assertNull(PendingSmartPageSaveStore(blank).current())
        assertNull(blank.get<String>("smart_pages.pending_save.path"))
    }

    @Test
    fun tokenGenerationFailureReturnsFailureWithoutSavedStateMutation() {
        val handle = SavedStateHandle()
        val expected = IllegalStateException("token source failed")
        val store = PendingSmartPageSaveStore(handle) { throw expected }

        val result = store.begin("asset-1", "/exports/asset-1")

        assertTrue(result.isFailure)
        assertSame(expected, result.exceptionOrNull())
        assertNull(store.current())
        assertNull(handle.get<String>("smart_pages.pending_save.token"))
    }
}
