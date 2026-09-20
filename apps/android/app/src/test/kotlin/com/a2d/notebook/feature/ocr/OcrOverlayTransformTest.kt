package com.a2d.notebook.feature.ocr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OcrOverlayTransformTest {
    @Test
    fun squareSourceFitsSquareViewportWithoutOffsets() {
        val transform = OcrOverlayTransform.contentFit(100f, 100f, 400f, 400f)
        assertEquals(4f, transform.scale, 0.0001f)
        assertEquals(0f, transform.offsetX, 0.0001f)
        assertEquals(0f, transform.offsetY, 0.0001f)
        assertEquals(OcrOverlayPoint(400f, 400f), transform.sourceToViewport(OcrOverlayPoint(100f, 100f)))
    }

    @Test
    fun portraitSourceIsPillarboxedAndRoundTrips() {
        val transform = OcrOverlayTransform.contentFit(100f, 200f, 400f, 400f)
        assertEquals(2f, transform.scale, 0.0001f)
        assertEquals(100f, transform.offsetX, 0.0001f)
        assertEquals(0f, transform.offsetY, 0.0001f)
        val source = OcrOverlayPoint(25f, 150f)
        assertEquals(source, transform.viewportToSource(transform.sourceToViewport(source)))
    }

    @Test
    fun landscapeSourceIsLetterboxedAndRoundTrips() {
        val transform = OcrOverlayTransform.contentFit(200f, 100f, 400f, 400f)
        assertEquals(2f, transform.scale, 0.0001f)
        assertEquals(0f, transform.offsetX, 0.0001f)
        assertEquals(100f, transform.offsetY, 0.0001f)
        val source = OcrOverlayPoint(175f, 25f)
        assertEquals(source, transform.viewportToSource(transform.sourceToViewport(source)))
    }

    @Test
    fun portraitSourceInLandscapeViewportUsesHorizontalPillarboxOffsets() {
        val transform = OcrOverlayTransform.contentFit(100f, 200f, 600f, 300f)
        assertEquals(1.5f, transform.scale, 0.0001f)
        assertEquals(225f, transform.offsetX, 0.0001f)
        assertEquals(0f, transform.offsetY, 0.0001f)
        assertNull(transform.viewportToSource(OcrOverlayPoint(224f, 150f)))
        assertEquals(OcrOverlayPoint(0f, 100f), transform.viewportToSource(OcrOverlayPoint(225f, 150f)))
    }

    @Test
    fun landscapeSourceInPortraitViewportUsesVerticalLetterboxOffsets() {
        val transform = OcrOverlayTransform.contentFit(200f, 100f, 300f, 600f)
        assertEquals(1.5f, transform.scale, 0.0001f)
        assertEquals(0f, transform.offsetX, 0.0001f)
        assertEquals(225f, transform.offsetY, 0.0001f)
        assertNull(transform.viewportToSource(OcrOverlayPoint(150f, 224f)))
        assertEquals(OcrOverlayPoint(100f, 0f), transform.viewportToSource(OcrOverlayPoint(150f, 225f)))
    }

    @Test
    fun allRenderedEdgesAreIncludedAndMapToAllSourceEdges() {
        val transform = OcrOverlayTransform.contentFit(200f, 100f, 400f, 400f)
        assertTrue(transform.containsViewportPoint(OcrOverlayPoint(0f, 100f)))
        assertTrue(transform.containsViewportPoint(OcrOverlayPoint(400f, 100f)))
        assertTrue(transform.containsViewportPoint(OcrOverlayPoint(0f, 300f)))
        assertTrue(transform.containsViewportPoint(OcrOverlayPoint(400f, 300f)))
        assertEquals(OcrOverlayPoint(0f, 0f), transform.viewportToSource(OcrOverlayPoint(0f, 100f)))
        assertEquals(OcrOverlayPoint(200f, 0f), transform.viewportToSource(OcrOverlayPoint(400f, 100f)))
        assertEquals(OcrOverlayPoint(0f, 100f), transform.viewportToSource(OcrOverlayPoint(0f, 300f)))
        assertEquals(OcrOverlayPoint(200f, 100f), transform.viewportToSource(OcrOverlayPoint(400f, 300f)))
    }

    @Test
    fun touchesInLetterboxAreRejected() {
        val transform = OcrOverlayTransform.contentFit(200f, 100f, 400f, 400f)
        assertFalse(transform.containsViewportPoint(OcrOverlayPoint(200f, 99f)))
        assertFalse(transform.containsViewportPoint(OcrOverlayPoint(200f, 301f)))
        assertNull(transform.viewportToSource(OcrOverlayPoint(200f, 50f)))
    }

    @Test
    fun sourceOrientationIsNotRotatedOrAxisSwappedByContentFit() {
        val transform = OcrOverlayTransform.contentFit(300f, 100f, 600f, 400f)
        assertEquals(OcrOverlayPoint(600f, 300f), transform.sourceToViewport(OcrOverlayPoint(300f, 100f)))
        assertEquals(OcrOverlayPoint(300f, 100f), transform.viewportToSource(OcrOverlayPoint(600f, 300f)))
    }

    @Test(expected = IllegalArgumentException::class)
    fun invalidSourceDimensionsAreRejected() {
        OcrOverlayTransform.contentFit(0f, 100f, 400f, 400f)
    }

    @Test(expected = IllegalArgumentException::class)
    fun invalidViewportDimensionsAreRejected() {
        OcrOverlayTransform.contentFit(100f, 100f, Float.NaN, 400f)
    }
}
