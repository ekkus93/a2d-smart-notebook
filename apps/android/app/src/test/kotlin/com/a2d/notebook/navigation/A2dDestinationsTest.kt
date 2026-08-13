package com.a2d.notebook.navigation

import org.junit.Assert.assertEquals
import org.junit.Test

/** JVM unit test (TODO 1.2's unit test source set) -- no Android framework needed. */
class A2dDestinationsTest {
    @Test
    fun homeRouteIsStable() {
        // The nav graph's start destination is referenced by this literal in A2dNavHost; a
        // change here without updating the graph would be a real regression, so pin it.
        assertEquals("home", A2dDestinations.HOME)
    }

    @Test
    fun singlePageScannerRouteIsStable() {
        assertEquals("scanner/single", A2dDestinations.SINGLE_PAGE_SCANNER)
    }

    @Test
    fun versionHistoryRouteCarriesTheCanonicalPageId() {
        assertEquals("versions/{pageId}", A2dDestinations.VERSION_HISTORY_PATTERN)
        assertEquals("versions/01ABCDEF", A2dDestinations.versionHistory("01ABCDEF"))
    }
}
