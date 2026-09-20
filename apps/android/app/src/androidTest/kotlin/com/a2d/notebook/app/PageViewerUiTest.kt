package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.Color
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
import com.a2d.notebook.feature.ocr.OcrPresentationState
import com.a2d.notebook.feature.ocr.OcrPresentationStatus
import com.a2d.notebook.feature.ocr.OcrRegionCoordinateFrame
import com.a2d.notebook.feature.ocr.OcrRegionOverlayRegion
import com.a2d.notebook.feature.ocr.OcrRegionOverlayState
import com.a2d.notebook.feature.ocr.OcrRegionOverlayTestTags
import com.a2d.notebook.feature.ocr.OcrTextPoint
import java.io.File
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
        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.CARD).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.DISABLED).assertIsDisplayed()
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
    fun pageViewerShowsPersistedOcrRegionOverlayWhenSourceGeometryAndRowsExist() {
        val sourceImagePath = writeTestOcrSourceImage(width = 200, height = 100)

        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-regions",
                            ocrRegionOverlay =
                                OcrRegionOverlayState(
                                    regions =
                                        listOf(
                                            OcrRegionOverlayRegion(
                                                textRegionId = "region-1",
                                                polygon =
                                                    listOf(
                                                        OcrTextPoint(0f, 0f),
                                                        OcrTextPoint(100f, 0f),
                                                        OcrTextPoint(100f, 50f),
                                                        OcrTextPoint(0f, 50f),
                                                    ),
                                                text = "persisted region text",
                                                confidence = 0.92f,
                                            ),
                                        ),
                                    sourceFrame = OcrRegionCoordinateFrame(width = 200f, height = 100f),
                                    sourceImagePath = sourceImagePath,
                                ),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.CARD).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.CANVAS).assertIsDisplayed()
        composeRule.onNodeWithText("1 selectable regions", substring = true).assertIsDisplayed()
    }

    @Test
    fun pageViewerDoesNotInferOcrRegionOverlayFrameFromPolygonExtrema() {
        composeRule.activity.setContent {
            MaterialTheme {
                PageViewerContent(
                    state =
                        PageViewerState(
                            pageId = "page-regions-no-source-frame",
                            ocrRegionOverlay =
                                OcrRegionOverlayState(
                                    regions =
                                        listOf(
                                            OcrRegionOverlayRegion(
                                                textRegionId = "region-1",
                                                polygon =
                                                    listOf(
                                                        OcrTextPoint(0f, 0f),
                                                        OcrTextPoint(100f, 0f),
                                                        OcrTextPoint(100f, 50f),
                                                        OcrTextPoint(0f, 50f),
                                                    ),
                                                text = "persisted region text",
                                                confidence = 0.92f,
                                            ),
                                        ),
                                ),
                        ),
                    onBack = {},
                    onOpenVersions = {},
                    onOpenNeedsReview = {},
                )
            }
        }

        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.CARD).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.DISABLED).assertIsDisplayed()
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

    private fun writeTestOcrSourceImage(width: Int, height: Int): String {
        val file = File(composeRule.activity.cacheDir, "ocr-source-${System.nanoTime()}.png")
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(Color.WHITE)
        file.outputStream().use { output ->
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) {
                "failed to write OCR source fixture image"
            }
        }
        bitmap.recycle()
        return file.absolutePath
    }
}
