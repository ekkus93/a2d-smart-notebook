package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.library.PageViewerContent
import com.a2d.notebook.feature.library.PageViewerState
import com.a2d.notebook.feature.library.PageViewerTestTags
import com.a2d.notebook.feature.ocr.LoadedAndroidOcrTextRegion
import com.a2d.notebook.feature.ocr.OcrPresentationState
import com.a2d.notebook.feature.ocr.OcrPresentationStatus
import com.a2d.notebook.feature.ocr.OcrTextPoint
import org.junit.Assert.assertEquals
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
                            preferredScanId = "scan-12",
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
        composeRule.onNodeWithText("Scan ID: scan-12").assertIsDisplayed()
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

    @Test
    fun pageViewerShowsNoTextOcrAsSuccessfulDistinctState() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-no-text",
                            preferredScanId = "scan-no-text",
                            ocrState =
                                OcrPresentationState(
                                    status = OcrPresentationStatus.NoTextDetected,
                                    runId = "ocr-run-no-text",
                                ),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule
            .onNodeWithText("detected no text", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("OCR run: ocr-run-no-text").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.OCR_START).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun pageViewerShowsUnavailableOcrWithRetryWithoutClaimingEmptyText() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-unavailable",
                            preferredScanId = "scan-unavailable",
                            ocrState =
                                OcrPresentationState(
                                    status = OcrPresentationStatus.Unavailable,
                                    unavailableReason = "provider failed",
                                    message = "ML Kit model missing",
                                    retryAvailable = true,
                                ),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule
            .onNodeWithText("OCR unavailable: provider failed", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("ML Kit model missing", substring = true).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(PageViewerTestTags.OCR_RETRY).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun pageViewerKeepsOcrOverlayDisabledWithoutRustRegionRows() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-no-regions",
                            preferredScanId = "scan-no-regions",
                            ocrState =
                                OcrPresentationState(
                                    status = OcrPresentationStatus.Detected,
                                    runId = "ocr-run-no-regions",
                                    recognizedRegionCount = 0,
                                ),
                            ocrTextRegions = emptyList(),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule
            .onNodeWithTag(PageViewerTestTags.OCR_REGION_OVERLAY_DISABLED)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("overlay is disabled", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun pageViewerShowsSelectableOcrRegionOverlayFromStoredPolygons() {
        var selectedRegionId: String? = null
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-regions",
                            preferredScanId = "scan-regions",
                            ocrState =
                                OcrPresentationState(
                                    status = OcrPresentationStatus.Detected,
                                    runId = "ocr-run-regions",
                                    recognizedRegionCount = 1,
                                ),
                            ocrTextRegions = listOf(textRegion()),
                            selectedOcrTextRegionId = "text-region-1",
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                    onSelectOcrTextRegion = { selectedRegionId = it },
                )
            }
        }

        composeRule.onNodeWithTag(PageViewerTestTags.OCR_REGION_OVERLAY).performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithTag(PageViewerTestTags.OCR_REGION_OVERLAY_CANVAS)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule
            .onNodeWithTag(PageViewerTestTags.OCR_REGION_BUTTON_PREFIX + "0")
            .performScrollTo()
            .performClick()
        assertEquals("text-region-1", selectedRegionId)
        composeRule
            .onNodeWithTag(PageViewerTestTags.OCR_REGION_SELECTED)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("Selected region: text-region-1").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("hello overlay region").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Confidence: 0.91").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Polygon: (0.0, 0.0)", substring = true).performScrollTo().assertIsDisplayed()
    }

    private fun textRegion(): LoadedAndroidOcrTextRegion =
        LoadedAndroidOcrTextRegion(
            textRegionId = "text-region-1",
            ocrRunId = "ocr-run-regions",
            polygon =
                listOf(
                    OcrTextPoint(x = 0.0f, y = 0.0f),
                    OcrTextPoint(x = 10.0f, y = 0.0f),
                    OcrTextPoint(x = 10.0f, y = 10.0f),
                    OcrTextPoint(x = 0.0f, y = 10.0f),
                ),
            text = "hello overlay region",
            confidence = 0.91f,
            createdAtMs = 300L,
        )
}
