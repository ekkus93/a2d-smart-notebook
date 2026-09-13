package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
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
        composeRule.onNodeWithTag(LibraryHubTestTags.NOTEBOOKS).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.SMART_PAGES).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.PAGE_SETS).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.COLLECTIONS).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.IMPORTS).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.NEEDS_REVIEW).assertIsDisplayed()
        composeRule.onNodeWithTag(LibraryHubTestTags.TRASH).assertIsDisplayed()
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
        composeRule.onNodeWithText("Total visible library entries: 28").assertIsDisplayed()
        composeRule.onNodeWithText("Notebooks: 2").assertIsDisplayed()
        composeRule.onNodeWithText("Smart Pages: 3").assertIsDisplayed()
        composeRule.onNodeWithText("Page Sets: 4").assertIsDisplayed()
        composeRule.onNodeWithText("Collections: 5").assertIsDisplayed()
        composeRule.onNodeWithText("Imports: 6").assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Trash: 7").assertIsDisplayed()
        composeRule
            .onNodeWithText("Search, OCR, backup, restore, and full page browsing remain separate roadmap slices.")
            .assertIsDisplayed()
    }
}
