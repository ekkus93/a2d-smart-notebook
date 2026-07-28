package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.EncodedPageAnalysisRequest
import com.a2d.notebook.rustbridge.EncodedPageFormat
import com.a2d.notebook.rustbridge.EncodedPageRotation
import com.a2d.notebook.rustbridge.PageMarkerIds
import com.a2d.notebook.rustbridge.analyzeEncodedPage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Executes the canonical synthetic page through the actual Android APK boundary:
 * Android test asset bytes -> typed Kotlin façade -> generated UniFFI/JNA binding -> packaged
 * x86_64 Rust cdylib -> shared encoded-image decoder -> Gray8 conversion -> official AprilTag
 * detector -> semantic marker resolution -> quality measurement -> typed Kotlin result.
 *
 * This is synthetic integration evidence only. It does not claim physical-camera performance or
 * establish production capture thresholds.
 */
@RunWith(AndroidJUnit4::class)
class SharedPageAnalysisTest {

    @Test
    fun canonicalSyntheticPageRunsThroughThePackagedSharedRustAnalysisPath() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val targetContext = instrumentation.targetContext
        val encodedBytes =
            instrumentation.context.assets.open("base-page.png").use { it.readBytes() }

        val result =
            A2dBridge.analyzeEncodedPage(
                targetContext,
                EncodedPageAnalysisRequest(
                    encodedBytes = encodedBytes,
                    format = EncodedPageFormat.PNG,
                    rotation = EncodedPageRotation.DEGREES_0,
                    maxEncodedBytes = 1_000_000,
                    maxPixels = 3_000_000,
                    maxDecodedBytes = 9_000_000,
                    detectorThreadCount = 1,
                    detectorQuadDecimate = 1.0,
                    detectorQuadSigma = 0.0,
                    detectorRefineEdges = true,
                    detectorDecodeSharpening = 0.25,
                    detectorBitsCorrected = 2,
                    darkLuminanceCutoff = 32,
                    highlightLuminanceCutoff = 245,
                    qualityTileColumns = 8,
                    qualityTileRows = 8,
                    markerIds =
                        PageMarkerIds(
                            topLeft = 0,
                            topRight = 1,
                            bottomRight = 2,
                            bottomLeft = 3,
                        ),
                ),
            )

        assertEquals(1400L, result.width)
        assertEquals(1900L, result.height)
        assertEquals(0, result.sourceRotationDegrees)
        assertEquals(0, result.resolvedOrientationDegrees)
        assertEquals(
            mapOf("TL" to 0L, "TR" to 1L, "BR" to 2L, "BL" to 3L),
            result.markers.associate { marker -> marker.role to marker.id },
        )
        assertTrue(result.unexpectedTagIds.isEmpty())
        assertTrue(result.markers.all { marker -> marker.family == "tagStandard41h12" })
        assertTrue(result.markers.all { marker -> marker.hammingErrors == 0L })
        assertTrue(result.markers.all { marker -> marker.decisionMargin > 0.0 })
        assertTrue(result.markers.all { marker -> marker.corners.size == 4 })
        assertTrue(result.quality.focusLaplacianVariance?.isFinite() == true)
        assertTrue(result.quality.meanLuminance.isFinite())
        assertTrue(result.quality.luminanceStandardDeviation.isFinite())
        assertTrue(result.quality.darkFraction in 0.0..1.0)
        assertTrue(result.quality.highlightFraction in 0.0..1.0)
        assertTrue(result.quality.maxTileHighlightFraction in 0.0..1.0)
    }
}
