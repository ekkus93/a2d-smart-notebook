package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.library.TrashContent
import com.a2d.notebook.feature.library.TrashItemSummary
import com.a2d.notebook.feature.library.TrashState
import com.a2d.notebook.feature.library.TrashWorkflowTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TrashWorkflowUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyTrashShowsLocalFirstBoundaryWithoutFabricatingItems() {
        composeRule.activity.setContent {
            MaterialTheme {
                TrashContent(
                    state = TrashState(),
                    onBack = {},
                )
            }
        }

        composeRule.onNodeWithTag(TrashWorkflowTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("No account or A2D server", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Trash items: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Restorable: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Permanent-delete eligible: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.API_BOUNDARY).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("Rust trash APIs", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.EMPTY_STATE).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("does not invent deleted rows", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun populatedTrashShowsConsequencesAndDestructiveActions() {
        composeRule.activity.setContent {
            MaterialTheme {
                TrashContent(
                    state =
                        TrashState(
                            trashApiConnected = true,
                            items =
                                listOf(
                                    TrashItemSummary(
                                        itemId = "trash-page-1",
                                        kindLabel = "Page",
                                        title = "Lab notebook page 12",
                                        locationSummary = "Field Notes / Page 12",
                                        deletedSummary = "moved to trash locally",
                                        retainedAssetSummary = "original and corrected assets retained",
                                        restoreConsequence = "returns Page 12 to Field Notes with the same page ID",
                                        permanentDeleteConsequence = "requires Rust to retire the ID and prevent reuse",
                                        pageId = "page-12",
                                    ),
                                ),
                        ),
                    onBack = {},
                    onOpenPage = {},
                    onRestore = {},
                    onPermanentDelete = {},
                )
            }
        }

        composeRule.onNodeWithText("Trash items: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Restorable: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Permanent-delete eligible: 1").assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.ITEM).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Kind: Page").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("ID: trash-page-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Location: Field Notes / Page 12").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Assets: original and corrected assets retained").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.CONSEQUENCE).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("same page ID", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("retire the ID and prevent reuse", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.OPEN_PAGE).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.RESTORE).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(TrashWorkflowTestTags.PERMANENT_DELETE).performScrollTo().assertIsDisplayed()
    }
}
