package com.a2d.notebook.rustbridge

import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer

private const val MAX_QR_DECODE_PIXELS = 4_194_304L

/**
 * Decodes exactly one QR code from a bounded row-major ARGB pixel buffer.
 *
 * This function deliberately owns only image decoding. Its returned text is untrusted and must be
 * sent to Rust for canonical A2D grammar, version, bounds, layout, CRC, identity, and workflow
 * validation before any success state is shown or persisted.
 */
fun decodeQrPixels(width: Int, height: Int, argbPixels: IntArray): String {
    require(width > 0 && height > 0) { "QR pixel dimensions must be positive" }

    val pixelCount = width.toLong() * height.toLong()
    require(pixelCount <= MAX_QR_DECODE_PIXELS) {
        "QR pixel buffer exceeds the decode safety limit"
    }
    require(pixelCount <= Int.MAX_VALUE.toLong()) {
        "QR pixel buffer is not addressable on this runtime"
    }
    require(argbPixels.size == pixelCount.toInt()) {
        "QR pixel buffer length does not match its declared dimensions"
    }

    val source = RGBLuminanceSource(width, height, argbPixels)
    val binary = BinaryBitmap(HybridBinarizer(source))
    val hints = mapOf(
        DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
        DecodeHintType.TRY_HARDER to true,
    )
    return MultiFormatReader().decode(binary, hints).text
}
