package com.a2d.notebook.feature.smartpage

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.a2d_ffi.SmartPageGenerationPolicy

class SmartPageFormTest {
    private val policy = SmartPageGenerationPolicy(
        policyVersion = 1u,
        maximumPageCount = 500u,
        maximumStartingVisiblePage = 999_999u,
        maximumPdfOutputBytes = 128uL * 1024uL * 1024uL,
    )

    @Test
    fun acceptsSinglePagesPageSetsAndTheQrBoundary() {
        assertEquals(1u, validateSmartPageForm("1", "1", policy).getOrThrow().pageCount)
        assertEquals(20u, validateSmartPageForm("20", "5", policy).getOrThrow().pageCount)
        assertEquals(
            2u,
            validateSmartPageForm("2", "999998", policy).getOrThrow().pageCount,
        )
    }

    @Test
    fun rejectsInvalidCounts() {
        assertTrue(validateSmartPageForm("0", "1", policy).isFailure)
        assertTrue(validateSmartPageForm("501", "1", policy).isFailure)
        assertTrue(validateSmartPageForm("abc", "1", policy).isFailure)
    }

    @Test
    fun rejectsInvalidStartingNumbersAndFinalPageOverflow() {
        assertTrue(validateSmartPageForm("1", "0", policy).isFailure)
        assertTrue(validateSmartPageForm("1", "x", policy).isFailure)
        assertTrue(validateSmartPageForm("1", "1000000", policy).isFailure)
        assertTrue(validateSmartPageForm("2", "999999", policy).isFailure)
        assertTrue(validateSmartPageForm("500", UInt.MAX_VALUE.toString(), policy).isFailure)
    }

    @Test
    fun honors_a_changed_rust_policy_without_android_constant_edits() {
        val restricted = policy.copy(maximumPageCount = 2u, maximumStartingVisiblePage = 10u)
        assertTrue(validateSmartPageForm("3", "1", restricted).isFailure)
        assertTrue(validateSmartPageForm("2", "10", restricted).isFailure)
        assertEquals(2u, validateSmartPageForm("2", "9", restricted).getOrThrow().pageCount)
    }
}
