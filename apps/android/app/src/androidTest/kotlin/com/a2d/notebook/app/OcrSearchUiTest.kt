package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.ocr.AndroidOcrSearchDocumentKind
import com.a2d.notebook.feature.ocr.OcrSearchContent
import com.a2d.notebook.feature.ocr.OcrSearchHitState
import com.a2d.notebook.feature.ocr.OcrSearchPresentationState
import com.a2d.notebook.feature.ocr.OcrSearchTestTags
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class OcrSearchUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun noQueryStateShowsLocalFirstSearchBoundary() {
        composeRule.activity.setContent {
            MaterialTheme {
                OcrSearchContent(
                    state = OcrSearchPresentationState.noQuery(),
                    onBack = {},
                    onQueryChange = {},
                    onSubmitSearch = {},
                    onOpenPage = {},
                )
            }
        }

        composeRule.onNodeWithTag(OcrSearchTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(OcrSearchTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("locally persisted OCR text", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(OcrSearchTestTags.QUERY_FIELD).assertIsDisplayed()
        composeRule.onNodeWithTag(OcrSearchTestTags.NO_QUERY).assertIsDisplayed()
    }

    @Test
    fun resultStateShowsSnippetMetadataAndCanOpenPage() {
        var openedPageId: String? = null
        composeRule.activity.setContent {
            MaterialTheme {
                OcrSearchContent(
                    state =
                        OcrSearchPresentationState.results(
                            query = "notebook",
                            hits =
                                listOf(
                                    OcrSearchHitState(
                                        pageId = "page-1",
                                        scanId = "scan-1",
                                        ocrRunId = "ocr-run-1",
                                        textRegionId = "region-1",
                                        documentKind = AndroidOcrSearchDocumentKind.TextRegion,
                                        snippet = "hello [notebook]",
                                    ),
                                ),
                        ),
                    onBack = {},
                    onQueryChange = {},
                    onSubmitSearch = {},
                    onOpenPage = { pageId -> openedPageId = pageId },
                )
            }
        }

        composeRule.onNodeWithTag(OcrSearchTestTags.RESULTS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(OcrSearchTestTags.RESULT_ROW).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("hello [notebook]").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Page ID: page-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Scan ID: scan-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("OCR run: ocr-run-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Text region: region-1").performScrollTo().assertIsDisplayed()
        composeRule.onAllNodesWithTag(OcrSearchTestTags.OPEN_PAGE)[0].performScrollTo().performClick()

        assertEquals("page-1", openedPageId)
    }

    @Test
    fun errorStateShowsRustErrorWithoutFakeResults() {
        composeRule.activity.setContent {
            MaterialTheme {
                OcrSearchContent(
                    state =
                        OcrSearchPresentationState.error(
                            query = "notebook",
                            message = "STORAGE_OCR_SEARCH_QUERY_EXCEEDS_LIMIT",
                        ),
                    onBack = {},
                    onQueryChange = {},
                    onSubmitSearch = {},
                    onOpenPage = {},
                )
            }
        }

        composeRule.onNodeWithTag(OcrSearchTestTags.ERROR).assertIsDisplayed()
        composeRule
            .onNodeWithText("STORAGE_OCR_SEARCH_QUERY_EXCEEDS_LIMIT", substring = true)
            .assertIsDisplayed()
        assertEquals(0, composeRule.onAllNodesWithTag(OcrSearchTestTags.RESULT_ROW).fetchSemanticsNodes().size)
    }
}
