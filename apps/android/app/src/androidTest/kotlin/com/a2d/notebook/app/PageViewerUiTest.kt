package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.library.PageViewerContent
import com.a2d.notebook.feature.library.PageViewerState
import com.a2d.notebook.feature.library.PageViewerTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PageViewerUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyPageViewerShowsEverySectionWithoutInventingData() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state = PageViewerState(pageId = "page-empty"),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule.onNodeWithTag(PageViewerTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("No account or A2D server", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.SUMMARY).assertIsDisplayed()
        composeRule.onNodeWithText("Page ID: page-empty").assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.API_BOUNDARY).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.ORIGINAL).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.CORRECTED).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.SPLIT).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.METADATA).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.ANNOTATIONS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.RELATED).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.SKILL_RESULTS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.VERSIONS).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("does not fabricate images, OCR text, annotations", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun populatedPageViewerShowsPageMetadataAndNavigationActions() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-12",
                            notebookName = "Field Notes",
                            visiblePageLabel = "12",
                            statusLabel = "needs review",
                            updatedSummary = "captured locally today",
                            hasOriginalImage = true,
                            hasCorrectedImage = true,
                            hasRecognizedText = true,
                            annotationCount = 2,
                            relatedPageCount = 3,
                            skillResultCount = 4,
                            needsReview = true,
                            viewerApiConnected = true,
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule.onNodeWithText("Page ID: page-12").assertIsDisplayed()
        composeRule.onNodeWithText("Notebook: Field Notes").assertIsDisplayed()
        composeRule.onNodeWithText("Visible page: 12").assertIsDisplayed()
        composeRule.onNodeWithText("Status: needs review").assertIsDisplayed()
        composeRule.onNodeWithText("Updated: captured locally today").assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review").assertIsDisplayed()
        composeRule.onNodeWithText("Original scan is available", substring = true).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Corrected image is available", substring = true).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Recognized text is available", substring = true).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Annotations: 2").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Related pages: 3").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Skill results: 4").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.OPEN_VERSIONS).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.OPEN_NEEDS_REVIEW).performScrollTo().assertIsDisplayed()
    }
}
