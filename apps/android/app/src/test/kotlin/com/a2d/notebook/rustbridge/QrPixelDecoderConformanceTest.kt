package com.a2d.notebook.rustbridge

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.MultiFormatWriter
import com.google.zxing.common.BitMatrix
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

private const val BLACK = -0x1000000
private const val WHITE = -0x1

/**
 * Deterministic, project-generated conformance controls for the shipped ZXing pixel decoder.
 *
 * The payload below is the canonical Rust-generated fixture payload produced by
 * `a2d_fixture_support`; this test does not reimplement A2D grammar or CRC generation in Kotlin.
 * Pixel transformations are test-only and make no claim about physical-camera tolerances.
 */
class QrPixelDecoderConformanceTest {
    private val canonicalPayload =
        "A2D:1:B:00000000000000000000000001:42:FIXTURE-MAIN-V1:0QV1AKZ"

    @Test
    fun canonicalPayloadDecodesAtExplicitScaleCases() {
        listOf(128, 192, 384).forEach { side ->
            val image = renderQr(canonicalPayload, side)
            assertEquals(
                "failed at ${side}x$side",
                canonicalPayload,
                decode(image),
            )
        }
    }

    @Test
    fun canonicalPayloadDecodesAfterQuarterTurnRotations() {
        val source = renderQr(canonicalPayload, 256)
        listOf(1, 2, 3).forEach { quarterTurns ->
            assertEquals(
                "failed after $quarterTurns quarter turns",
                canonicalPayload,
                decode(rotateQuarterTurns(source, quarterTurns)),
            )
        }
    }

    @Test
    fun canonicalPayloadDecodesAfterMildDeterministicBlur() {
        val source = renderQr(canonicalPayload, 384)
        assertEquals(canonicalPayload, decode(boxBlur(source, radius = 1)))
    }

    @Test
    fun canonicalPayloadDecodesThroughLocalizedGlare() {
        val source = renderQr(canonicalPayload, 384)
        val glare = applyCenteredGlare(source, radiusFraction = 0.07, alpha = 0.72)
        assertEquals(canonicalPayload, decode(glare))
    }

    @Test
    fun controlledMinorDamageRemainsDecodable() {
        val source = renderQr(canonicalPayload, 384)
        val damaged = coverCenteredSquare(source, sideFraction = 0.07)
        assertEquals(canonicalPayload, decode(damaged))
    }

    @Test
    fun controlledSevereDamageFailsExplicitly() {
        val source = renderQr(canonicalPayload, 384)
        val damaged = coverCenteredSquare(source, sideFraction = 0.60)
        assertTrue(
            "severely damaged QR unexpectedly decoded",
            runCatching { decode(damaged) }.isFailure,
        )
    }

    @Test
    fun malformedPixelGeometryIsRejectedBeforeZxing() {
        assertThrows(IllegalArgumentException::class.java) {
            decodeQrPixels(0, 10, IntArray(0))
        }
        assertThrows(IllegalArgumentException::class.java) {
            decodeQrPixels(10, 10, IntArray(99))
        }
        assertThrows(IllegalArgumentException::class.java) {
            decodeQrPixels(2_049, 2_049, IntArray(1))
        }
    }

    private fun decode(image: PixelImage): String =
        decodeQrPixels(image.width, image.height, image.pixels)

    private fun renderQr(text: String, side: Int): PixelImage {
        val hints = mapOf(
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.MARGIN to 4,
        )
        val matrix: BitMatrix =
            MultiFormatWriter().encode(text, BarcodeFormat.QR_CODE, side, side, hints)
        val pixels = IntArray(matrix.width * matrix.height)
        for (y in 0 until matrix.height) {
            for (x in 0 until matrix.width) {
                pixels[y * matrix.width + x] = if (matrix.get(x, y)) BLACK else WHITE
            }
        }
        return PixelImage(matrix.width, matrix.height, pixels)
    }

    private fun rotateQuarterTurns(source: PixelImage, turns: Int): PixelImage {
        var result = source
        repeat(Math.floorMod(turns, 4)) {
            val destination = IntArray(result.width * result.height)
            val newWidth = result.height
            val newHeight = result.width
            for (y in 0 until result.height) {
                for (x in 0 until result.width) {
                    val newX = result.height - 1 - y
                    val newY = x
                    destination[newY * newWidth + newX] = result.pixels[y * result.width + x]
                }
            }
            result = PixelImage(newWidth, newHeight, destination)
        }
        return result
    }

    private fun boxBlur(source: PixelImage, radius: Int): PixelImage {
        require(radius > 0)
        val destination = IntArray(source.pixels.size)
        for (y in 0 until source.height) {
            for (x in 0 until source.width) {
                var total = 0
                var count = 0
                for (sampleY in (y - radius).coerceAtLeast(0)..(y + radius).coerceAtMost(source.height - 1)) {
                    for (sampleX in (x - radius).coerceAtLeast(0)..(x + radius).coerceAtMost(source.width - 1)) {
                        total += source.pixels[sampleY * source.width + sampleX] and 0xFF
                        count += 1
                    }
                }
                destination[y * source.width + x] = gray(total / count)
            }
        }
        return source.copy(pixels = destination)
    }

    private fun applyCenteredGlare(
        source: PixelImage,
        radiusFraction: Double,
        alpha: Double,
    ): PixelImage {
        require(radiusFraction > 0.0 && radiusFraction < 0.5)
        require(alpha in 0.0..1.0)
        val destination = source.pixels.copyOf()
        val centerX = (source.width - 1) / 2.0
        val centerY = (source.height - 1) / 2.0
        val radius = minOf(source.width, source.height) * radiusFraction
        for (y in 0 until source.height) {
            for (x in 0 until source.width) {
                val dx = x - centerX
                val dy = y - centerY
                val distance = kotlin.math.sqrt(dx * dx + dy * dy)
                if (distance > radius) continue
                val edgeFade = 1.0 - distance / radius
                val localAlpha = alpha * edgeFade
                val original = destination[y * source.width + x] and 0xFF
                val value = (original * (1.0 - localAlpha) + 255.0 * localAlpha)
                    .toInt()
                    .coerceIn(0, 255)
                destination[y * source.width + x] = gray(value)
            }
        }
        return source.copy(pixels = destination)
    }

    private fun coverCenteredSquare(source: PixelImage, sideFraction: Double): PixelImage {
        require(sideFraction > 0.0 && sideFraction < 1.0)
        val destination = source.pixels.copyOf()
        val side = (minOf(source.width, source.height) * sideFraction).toInt().coerceAtLeast(1)
        val left = (source.width - side) / 2
        val top = (source.height - side) / 2
        for (y in top until top + side) {
            for (x in left until left + side) {
                destination[y * source.width + x] = WHITE
            }
        }
        return source.copy(pixels = destination)
    }

    private fun gray(value: Int): Int =
        (0xFF shl 24) or (value shl 16) or (value shl 8) or value

    private data class PixelImage(
        val width: Int,
        val height: Int,
        val pixels: IntArray,
    )
}
