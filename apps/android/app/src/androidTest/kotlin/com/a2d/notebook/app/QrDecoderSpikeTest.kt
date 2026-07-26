package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.Color
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.A2dBridge
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.EncodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.MultiFormatWriter
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.BitMatrix
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The ADR 0001 QR decoder spike ("docs/decisions/0001-qr-v1-encoding-and-integrity.md"'s
 * Validation Evidence checklist). Proves the v1 grammar survives a real render/decode round
 * trip: Rust generates the canonical payload text (a2d-identity's `PageCode::encode`, crossing
 * the UniFFI/JNA boundary, not a hand-typed fixture) -> ZXing renders it as an actual QR image ->
 * ZXing decodes that image back to text -> assert byte-for-byte equality.
 *
 * ZXing here stands in for "a real Android QR decoder" per the ADR -- it's what most Android QR
 * scanning is built on, directly or via ML Kit. This is NOT the final production decoder
 * library choice (that's Milestone 7.4/12); it's the spike's proxy for "a decoder a phone would
 * plausibly use," which is what the ADR needs evidence against.
 *
 * This test does NOT by itself move the ADR to Accepted -- see that file's own checklist. It
 * provides the "render/decode round trip survives" evidence; the module-size/damage-tolerance
 * evidence still needs a physical layout (Milestone 5) to test against.
 */
@RunWith(AndroidJUnit4::class)
class QrDecoderSpikeTest {

    private fun client() =
        A2dBridge.client(InstrumentationRegistry.getInstrumentation().targetContext)

    /** Renders [text] as a QR code and decodes it straight back, asserting equality. */
    private fun assertRoundTripsThroughARenderedQrImage(text: String, sizePx: Int = 300) {
        val hints = mapOf(
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.MARGIN to 2,
        )
        val matrix: BitMatrix =
            MultiFormatWriter().encode(text, BarcodeFormat.QR_CODE, sizePx, sizePx, hints)

        val bitmap = Bitmap.createBitmap(matrix.width, matrix.height, Bitmap.Config.ARGB_8888)
        for (x in 0 until matrix.width) {
            for (y in 0 until matrix.height) {
                bitmap.setPixel(x, y, if (matrix.get(x, y)) Color.BLACK else Color.WHITE)
            }
        }

        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        val source = RGBLuminanceSource(bitmap.width, bitmap.height, pixels)
        val binaryBitmap = BinaryBitmap(HybridBinarizer(source))

        val decoded = MultiFormatReader().decode(binaryBitmap)

        assertEqualsWithContext(text, decoded.text)
    }

    private fun assertEqualsWithContext(expected: String, actual: String) {
        if (expected != actual) {
            throw AssertionError(
                "QR round trip did not preserve the canonical payload.\n" +
                    "  expected: $expected\n" +
                    "  actual:   $actual"
            )
        }
    }

    @Test
    fun notebookSetupPayloadRoundTripsThroughARealQrImage() {
        val payload = client().generateExampleNotebookSetupQrPayload()
        assertPayloadShape(payload, "S")
        assertRoundTripsThroughARenderedQrImage(payload)
    }

    @Test
    fun notebookPagePayloadRoundTripsThroughARealQrImage() {
        val payload = client().generateExampleNotebookPageQrPayload()
        assertPayloadShape(payload, "B")
        assertRoundTripsThroughARenderedQrImage(payload)
    }

    /** The longest of the three variants (ADR 0001 calls this out as the worst case). */
    @Test
    fun smartPagePayloadRoundTripsThroughARealQrImage() {
        val payload = client().generateExampleSmartPageQrPayload()
        assertPayloadShape(payload, "M")
        assertRoundTripsThroughARenderedQrImage(payload)
    }

    /** Same worst-case payload, rendered small -- a rough proxy for a printed page's QR size
     * until Milestone 5 defines the real physical layout to test against. */
    @Test
    fun smartPagePayloadRoundTripsAtASmallRenderSize() {
        val payload = client().generateExampleSmartPageQrPayload()
        assertRoundTripsThroughARenderedQrImage(payload, sizePx = 120)
    }

    @Test
    fun eachCallGeneratesADifferentPayload() {
        val a = client().generateExampleSmartPageQrPayload()
        val b = client().generateExampleSmartPageQrPayload()
        assert(a != b) { "expected fresh random ids each call, got the same payload twice" }
    }

    private fun assertPayloadShape(payload: String, expectedTypeCode: String) {
        val parts = payload.split(":")
        assert(parts[0] == "A2D") { "expected magic prefix A2D, got: $payload" }
        assert(parts[1] == "1") { "expected version 1, got: $payload" }
        assert(parts[2] == expectedTypeCode) {
            "expected type code $expectedTypeCode, got: $payload"
        }
        assert(payload == payload.uppercase()) { "payload must be canonical uppercase: $payload" }
    }
}
