package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.library.LibraryHubScreen
import com.a2d.notebook.feature.library.LibraryHubState
import com.a2d.notebook.feature.library.LibraryHubTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class LibraryHubUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyLibraryHubShowsLocalFirstBoundaryAndEveryLibraryDestination() {
        composeRule.activity.setContent {
            MaterialTheme {
                LibraryHubScreen(
                    onBack = {},
                    onOpenNotebooks = {},
                    onOpenSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithTag(LibraryHubTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("No account or A2D server", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.STATUS).assertIsDisplayed()
        composeRule.onNodeWithText("Total visible library entries: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.EMPTY_STATE).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.NOTEBOOKS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.SMART_PAGES).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.PAGES).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.OCR_SEARCH).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.PAGE_SETS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.COLLECTIONS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.IMPORTS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.NEEDS_REVIEW).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.TRASH).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun populatedLibraryHubShowsCountsWithoutClaimingFutureBackendsExist() {
        composeRule.activity.setContent {
            MaterialTheme {
                LibraryHubScreen(
                    state =
                        LibraryHubState(
                            notebookCount = 2,
                            smartPageCount = 3,
                            pageCount = 8,
                            pageSetCount = 4,
                            collectionCount = 5,
                            importCount = 6,
                            needsReviewCount = 1,
                            trashCount = 7,
                        ),
                    onBack = {},
                    onOpenNotebooks = {},
                    onOpenSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithTag(LibraryHubTestTags.STATUS).assertIsDisplayed()
        composeRule.onNodeWithText("Total visible library entries: 36").assertIsDisplayed()
        composeRule.onNodeWithText("Notebooks: 2").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Smart Pages: 3").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Pages: 8").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Search locally persisted OCR text").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Page Sets: 4").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Collections: 5").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Imports: 6").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review: 1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Trash: 7").performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("Backup, restore, page-row persistence", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }
}
