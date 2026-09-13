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
import com.a2d.notebook.feature.smartpage.GeneratedContentCollectionSummary
import com.a2d.notebook.feature.smartpage.GeneratedPageSetSummary
import com.a2d.notebook.feature.smartpage.GeneratedSmartPageSummary
import com.a2d.notebook.feature.smartpage.SmartPageLibraryContent
import com.a2d.notebook.feature.smartpage.SmartPageLibraryState
import com.a2d.notebook.feature.smartpage.SmartPageLibraryTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SmartPageLibraryUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyGeneratedContentLibraryShowsLocalFirstBoundaryWithoutInventingRows() {
        composeRule.activity.setContent {
            MaterialTheme {
                SmartPageLibraryContent(
                    state = SmartPageLibraryState(),
                    onBack = {},
                    onCreateSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithTag(SmartPageLibraryTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("stay on this device", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Smart Pages: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Page Sets: 0").assertIsDisplayed()
        composeRule.onNodeWithText("Collections: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.API_BOUNDARY).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.EMPTY_STATE).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("does not fabricate generated-content rows", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.CREATE).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun populatedGeneratedContentLibraryPreservesImmutableIdentityAcrossSections() {
        composeRule.activity.setContent {
            MaterialTheme {
                SmartPageLibraryContent(
                    state =
                        SmartPageLibraryState(
                            generatedContentApiConnected = true,
                            smartPages =
                                listOf(
                                    GeneratedSmartPageSummary(
                                        pageId = "smart-page-1",
                                        pageSetId = "page-set-1",
                                        visiblePageLabel = "101",
                                        updatedSummary = "generated locally",
                                    ),
                                ),
                            pageSets =
                                listOf(
                                    GeneratedPageSetSummary(
                                        pageSetId = "page-set-1",
                                        pageCount = 5,
                                        updatedSummary = "print-ready PDF retained",
                                    ),
                                ),
                            collections =
                                listOf(
                                    GeneratedContentCollectionSummary(
                                        collectionId = "collection-1",
                                        title = "Lab templates",
                                        pageCount = 5,
                                        updatedSummary = "manual grouping",
                                    ),
                                ),
                        ),
                    onBack = {},
                    onCreateSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithText("Smart Pages: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Page Sets: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Collections: 1").assertIsDisplayed()
        composeRule.onAllNodesWithTag(SmartPageLibraryTestTags.SMART_PAGE_ROW)[0]
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("Smart Page 101").assertIsDisplayed()
        composeRule.onNodeWithText("ID: smart-page-1").assertIsDisplayed()
        composeRule.onAllNodesWithTag(SmartPageLibraryTestTags.PAGE_SET_ROW)[0]
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("Page Set page-set-1").assertIsDisplayed()
        composeRule.onNodeWithText("Pages: 5").assertIsDisplayed()
        composeRule.onAllNodesWithTag(SmartPageLibraryTestTags.COLLECTION_ROW)[0]
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("Collection Lab templates").assertIsDisplayed()
        composeRule
            .onNodeWithText("rows are never renumbered or reused", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }
}
