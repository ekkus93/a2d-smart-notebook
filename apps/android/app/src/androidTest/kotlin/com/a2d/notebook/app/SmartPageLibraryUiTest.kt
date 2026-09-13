package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.smartpage.GeneratedCollectionSummary
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
    fun emptyGeneratedContentLibraryShowsLocalFirstBoundaryAndCreateAction() {
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
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Smart Pages: 0").assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.BOUNDARY).assertIsDisplayed()
        composeRule
            .onNodeWithText("Rust generated-content APIs", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.CREATE).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.EMPTY_STATE).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun populatedGeneratedContentLibraryPreservesImmutableIdentityAcrossSections() {
        composeRule.activity.setContent {
            MaterialTheme {
                SmartPageLibraryContent(
                    state =
                        SmartPageLibraryState(
                            smartPages =
                                listOf(
                                    GeneratedSmartPageSummary(
                                        smartPageId = "smart-page-1",
                                        title = "Lecture notes",
                                        pageSetId = "page-set-1",
                                        pageCount = 3,
                                        style = "Lined",
                                        createdSummary = "2026-09-13",
                                    ),
                                ),
                            pageSets =
                                listOf(
                                    GeneratedPageSetSummary(
                                        pageSetId = "page-set-1",
                                        title = "Week 1 packet",
                                        pageCount = 3,
                                        firstVisiblePage = 1,
                                        createdSummary = "2026-09-13",
                                    ),
                                ),
                            collections =
                                listOf(
                                    GeneratedCollectionSummary(
                                        collectionId = "collection-1",
                                        title = "Biology class",
                                        itemCount = 2,
                                        ruleSummary = "Manual local collection",
                                    ),
                                ),
                            generatedContentApiConnected = true,
                        ),
                    onBack = {},
                    onCreateSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithText("Smart Pages: 3").assertIsDisplayed()
        composeRule.onNodeWithText("Page Sets: 1").assertIsDisplayed()
        composeRule.onNodeWithText("Collections: 1").assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.SMART_PAGE_ITEM).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Smart Page ID: smart-page-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Style: Lined").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.PAGE_SET_ITEM).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("First visible page: 1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(SmartPageLibraryTestTags.COLLECTION_ITEM).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Collection ID: collection-1").performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("Generated IDs must never be reused", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }
}
