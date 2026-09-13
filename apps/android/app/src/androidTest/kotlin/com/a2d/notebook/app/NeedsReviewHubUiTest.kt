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
import com.a2d.notebook.feature.review.NeedsReviewContent
import com.a2d.notebook.feature.review.NeedsReviewItemSummary
import com.a2d.notebook.feature.review.NeedsReviewQueueState
import com.a2d.notebook.feature.review.NeedsReviewTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NeedsReviewHubUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyNeedsReviewQueueShowsLocalFirstRustBoundary() {
        composeRule.activity.setContent {
            MaterialTheme {
                NeedsReviewContent(
                    state = NeedsReviewQueueState(),
                    onBack = {},
                    onOpenVersions = {},
                )
            }
        }

        composeRule.onNodeWithTag(NeedsReviewTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("stored locally", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Review items: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Unresolved: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Deferred: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.API_BOUNDARY).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.EMPTY_STATE).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("does not fabricate review items", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun populatedNeedsReviewQueueShowsItemActionsWithoutMutatingRust() {
        composeRule.activity.setContent {
            MaterialTheme {
                NeedsReviewContent(
                    state =
                        NeedsReviewQueueState(
                            totalCount = 2,
                            unresolvedCount = 1,
                            deferredCount = 1,
                            reviewApiConnected = true,
                            items =
                                listOf(
                                    NeedsReviewItemSummary(
                                        reviewItemId = "review-a",
                                        pageId = "page-a",
                                        notebookName = "Field Notes",
                                        reason = "duplicate or revision requires user choice",
                                        createdSummary = "created from local scan evidence",
                                        resolutionOptions = listOf("KEEP_BOTH_VERSIONS"),
                                    ),
                                ),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                )
            }
        }

        composeRule.onNodeWithTag(NeedsReviewTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Review items: 2").assertIsDisplayed()
        composeRule.onNodeWithText("Unresolved: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Deferred: 1").assertIsDisplayed()
        composeRule.onAllNodesWithTag(NeedsReviewTestTags.ITEM)[0].performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Review Field Notes").assertIsDisplayed()
        composeRule.onNodeWithText("Page ID: page-a").assertIsDisplayed()
        composeRule
            .onNodeWithText("duplicate or revision requires user choice", substring = true)
            .assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.OPEN_VERSIONS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.DEFER).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(NeedsReviewTestTags.RESOLVE).performScrollTo().assertIsDisplayed()
    }
}
