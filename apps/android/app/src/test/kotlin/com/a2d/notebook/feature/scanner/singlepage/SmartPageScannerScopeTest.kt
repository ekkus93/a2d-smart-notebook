package com.a2d.notebook.feature.scanner.singlepage

import kotlin.test.Test
import kotlin.test.assertEquals

class SmartPageScannerScopeTest {
    @Test
    fun deferred_message_is_specific_to_the_notebook_page_scanner() {
        assertEquals(
            "Smart Page scanning is not available in the v0.1 Notebook Page scanner.",
            SMART_PAGE_SCANNER_DEFERRED_MESSAGE,
        )
    }
}
