package com.a2d.notebook.rustbridge

internal fun decodePolicyPreviewRgb(
    reader: PolicyPreviewCodecReader,
    name: String,
): ProcessedRgbImage {
    val width = reader.readInt("$name width")
    val height = reader.readInt("$name height")
    require(width > 0 && height > 0) { "$name dimensions must be positive" }
    val expectedBytes = Math.multiplyExact(Math.multiplyExact(width, height), 3)
    val bytes = reader.readBytes("$name bytes", expectedBytes)
    require(bytes.size == expectedBytes) {
        "$name byte count does not match its dimensions"
    }
    return ProcessedRgbImage(width = width, height = height, bytes = bytes)
}
