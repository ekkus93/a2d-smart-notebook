package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.library.ImportItemSummary
import com.a2d.notebook.feature.library.ImportLibraryContent
import com.a2d.notebook.feature.library.ImportLibraryState
import com.a2d.notebook.feature.library.ImportLibraryTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ImportLibraryUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyImportsShowsLocalFirstBoundaryWithoutFabricatingRows() {
        composeRule.activity.setContent {
            MaterialTheme {
                ImportLibraryContent(
                    state = ImportLibraryState(),
                    onBack = {},
                )
            }
        }

        composeRule.onNodeWithTag(ImportLibraryTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("No account or A2D server", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Imports: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Imported pages: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Conflicts: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Retryable: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.API_BOUNDARY).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("Rust import APIs", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.EMPTY_STATE).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("No imports are listed yet").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun populatedImportsShowsSourceConflictConsequencesAndActions() {
        composeRule.activity.setContent {
            MaterialTheme {
                ImportLibraryContent(
                    state =
                        ImportLibraryState(
                            importApiConnected = true,
                            items =
                                listOf(
                                    ImportItemSummary(
                                        importId = "import-1",
                                        title = "Biology worksheet import",
                                        sourceLabel = "selected PDF",
                                        importedSummary = "inspected locally",
                                        statusLabel = "conflict requires review",
                                        pageCount = 3,
                                        conflictCount = 1,
                                        consequenceSummary = "requires explicit merge before any existing page ID changes",
                                        pageId = "page-12",
                                        retryAvailable = true,
                                    ),
                                ),
                        ),
                    onBack = {},
                    onOpenPage = {},
                    onRetryImport = {},
                    onMoveToReview = {},
                )
            }
        }

        composeRule.onNodeWithText("Imports: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Imported pages: 3").assertIsDisplayed()
        composeRule.onNodeWithText("Conflicts: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Retryable: 1").assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.ITEM).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Biology worksheet import").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Import ID: import-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.SOURCE).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Status: conflict requires review").performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("explicit merge before any existing page ID changes", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.OPEN_PAGE).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.RETRY).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(ImportLibraryTestTags.MOVE_TO_REVIEW).performScrollTo().assertIsDisplayed()
    }
}
