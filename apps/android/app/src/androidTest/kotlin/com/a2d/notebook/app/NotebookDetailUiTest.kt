package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.notebook.NotebookDetailContent
import com.a2d.notebook.feature.notebook.NotebookScreenTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.NotebookSummary

@RunWith(AndroidJUnit4::class)
class NotebookDetailUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun detailShowsNotebookIdentityActionsAndPagePresentationBoundary() {
        composeRule.activity.setContent {
            MaterialTheme {
                NotebookDetailContent(
                    notebookId = "notebook-a",
                    notebook =
                        NotebookSummary(
                            id = "notebook-a",
                            designId = "placeholder-design-v1",
                            displayName = "Field Notes",
                            archived = false,
                            active = true,
                        ),
                    busy = false,
                    error = null,
                    onBack = {},
                    onScanPage = {},
                    onBatchScan = {},
                )
            }
        }

        composeRule.onNodeWithTag(NotebookScreenTestTags.DETAIL_TITLE).assertIsDisplayed()
        composeRule.onNodeWithText("Field Notes").assertIsDisplayed()
        composeRule.onNodeWithTag(NotebookScreenTestTags.DETAIL_IDENTITY).assertIsDisplayed()
        composeRule.onNodeWithText("Notebook ID: notebook-a").assertIsDisplayed()
        composeRule.onNodeWithText("Design ID: placeholder-design-v1").assertIsDisplayed()
        composeRule.onNodeWithText("Status: active scan destination").assertIsDisplayed()
        composeRule.onNodeWithTag(NotebookScreenTestTags.DETAIL_ACTIONS).assertIsDisplayed()
        composeRule.onNodeWithText("Scan one page").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Batch scan this Notebook").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(NotebookScreenTestTags.DETAIL_PAGE_SLOTS).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("logical page slots", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("The UI must not renumber pages from scan order.")
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun missingNotebookShowsExplicitMissingStateInsteadOfInventingData() {
        composeRule.activity.setContent {
            MaterialTheme {
                NotebookDetailContent(
                    notebookId = "missing-notebook",
                    notebook = null,
                    busy = false,
                    error = null,
                    onBack = {},
                    onScanPage = {},
                    onBatchScan = {},
                )
            }
        }

        composeRule.onNodeWithTag(NotebookScreenTestTags.DETAIL_MISSING).assertIsDisplayed()
        composeRule.onNodeWithText("Notebook record unavailable").assertIsDisplayed()
        composeRule.onNodeWithText("missing-notebook", substring = true).assertIsDisplayed()
    }
}
