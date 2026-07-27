package com.a2d.notebook.feature.smartpage

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SmartPageFormTest {
    @Test
    fun acceptsSinglePagesAndPageSets() {
        assertEquals(1u, validateSmartPageForm("1", "1").getOrThrow().pageCount)
        assertEquals(20u, validateSmartPageForm("20", "5").getOrThrow().pageCount)
    }

    @Test
    fun rejectsInvalidCounts() {
        assertTrue(validateSmartPageForm("0", "1").isFailure)
        assertTrue(validateSmartPageForm("501", "1").isFailure)
        assertTrue(validateSmartPageForm("abc", "1").isFailure)
    }

    @Test
    fun rejectsInvalidStartingNumbers() {
        assertTrue(validateSmartPageForm("1", "0").isFailure)
        assertTrue(validateSmartPageForm("1", "x").isFailure)
    }
}
