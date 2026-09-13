package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.library.PageBrowserContent
import com.a2d.notebook.feature.library.PageBrowserPageSummary
import com.a2d.notebook.feature.library.PageBrowserState
import com.a2d.notebook.feature.library.PageBrowserTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PageBrowserUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyPageBrowserShowsLocalFirstBoundaryWithoutInventingRows() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageBrowserContent(
                    state = PageBrowserState(),
                    onBack = {},
                    onOpenVersions = {},
                )
            }
        }

        composeRule.onNodeWithTag(PageBrowserTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(PageBrowserTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("Browse local pages", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(PageBrowserTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Pages: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Scanned pages: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(PageBrowserTestTags.API_BOUNDARY).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageBrowserTestTags.EMPTY_STATE).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("does not invent page rows before Rust exposes them", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun populatedPageBrowserShowsRowsAndVersionAction() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageBrowserContent(
                    state =
                        PageBrowserState(
                            pageCount = 2,
                            scannedPageCount = 1,
                            needsReviewCount = 1,
                            pagePresentationApiConnected = true,
                            pages =
                                listOf(
                                    PageBrowserPageSummary(
                                        pageId = "page-a",
                                        notebookName = "Field Notes",
                                        visiblePageLabel = "12",
                                        statusLabel = "scanned",
                                        updatedSummary = "last scan retained locally",
                                        needsReview = true,
                                    ),
                                ),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                )
            }
        }

        composeRule.onNodeWithTag(PageBrowserTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Pages: 2").assertIsDisplayed()
        composeRule.onNodeWithText("Scanned pages: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review: 1").assertIsDisplayed()
        composeRule.onAllNodesWithTag(PageBrowserTestTags.PAGE_ROW)[0].performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Page: 12").assertIsDisplayed()
        composeRule.onNodeWithText("Notebook: Field Notes").assertIsDisplayed()
        composeRule.onNodeWithText("Status: scanned").assertIsDisplayed()
        composeRule.onNodeWithText("Updated: last scan retained locally").assertIsDisplayed()
        composeRule.onNodeWithTag(PageBrowserTestTags.OPEN_VERSIONS).performScrollTo().assertIsDisplayed()
    }
}
