package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import com.a2d.notebook.feature.ocr.OcrRegionCoordinateFrame
import com.a2d.notebook.feature.ocr.OcrRegionOverlayCard
import com.a2d.notebook.feature.ocr.OcrRegionOverlayRegion
import com.a2d.notebook.feature.ocr.OcrRegionOverlayState
import com.a2d.notebook.feature.ocr.OcrRegionOverlayTestTags
import com.a2d.notebook.feature.ocr.OcrTextPoint
import org.junit.Rule
import org.junit.Test

class OcrRegionOverlayCardImageTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun overlayCanvasRequiresRealSourceImageAndSharedFrame() {
        val sourceImagePath = writeSourceImage(width = 200, height = 100)

        composeRule.activity.setContent {
            MaterialTheme {
                OcrRegionOverlayCard(
                    overlay =
                        OcrRegionOverlayState(
                            regions = listOf(region()),
                            sourceFrame = OcrRegionCoordinateFrame(width = 200f, height = 100f),
                            sourceImagePath = sourceImagePath,
                        ),
                )
            }
        }

        composeRule.waitUntil(timeoutMillis = 10_000) {
            composeRule.onAllNodesWithTag(OcrRegionOverlayTestTags.CANVAS).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.CANVAS).assertIsDisplayed()
    }

    @Test
    fun overlayStaysDisabledWhenSourceImageIsMissing() {
        composeRule.activity.setContent {
            MaterialTheme {
                OcrRegionOverlayCard(
                    overlay =
                        OcrRegionOverlayState(
                            regions = listOf(region()),
                            sourceFrame = OcrRegionCoordinateFrame(width = 200f, height = 100f),
                        ),
                )
            }
        }

        composeRule.onNodeWithTag(OcrRegionOverlayTestTags.DISABLED).assertIsDisplayed()
    }

    private fun writeSourceImage(width: Int, height: Int): String {
        val file = composeRule.activity.cacheDir.resolve("ocr-overlay-source-$width-$height.png")
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        try {
            Canvas(bitmap).drawColor(Color.WHITE)
            file.outputStream().use { output -> check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) }
        } finally {
            bitmap.recycle()
        }
        return file.absolutePath
    }

    private fun region(): OcrRegionOverlayRegion =
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
        )
}
