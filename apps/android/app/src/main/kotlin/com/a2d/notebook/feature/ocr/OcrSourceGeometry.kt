package com.a2d.notebook.feature.ocr

import java.io.File
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind
import uniffi.a2d_ffi.OcrSourceGeometry as FfiOcrSourceGeometry

/** Android projection of Rust-owned OCR source identity and image bounds. */
data class AndroidOcrSourceGeometry(
    val scanId: String,
    val inputAssetId: String,
    val inputKind: FfiOcrInputKind,
    val mediaType: String,
    val relativePath: String,
    val absolutePath: String,
    val byteLength: ULong,
    val widthPx: UInt,
    val heightPx: UInt,
) {
    val inputKindLabel: String
        get() =
            when (inputKind) {
                FfiOcrInputKind.ORIGINAL -> "Original"
                FfiOcrInputKind.CORRECTED -> "Corrected"
                FfiOcrInputKind.OCR_OPTIMIZED -> "OcrOptimized"
            }
}

/**
 * Thin Android gateway for Page Viewer's authoritative OCR source geometry.
 *
 * Page Viewer callers must use this instead of manufacturing OCR dimensions. Rust selects and
 * validates the scan-owned source asset, resolves the library-relative path, and reads the encoded
 * image dimensions from the committed immutable file.
 */
class AndroidOcrSourceGeometryGateway(private val client: A2dClient) {
    fun resolveForViewer(scanId: String): AndroidOcrSourceGeometry =
        client.resolveOcrSourceGeometryForViewer(scanId).toAndroid(File(client.libraryPath()).canonicalFile)

    fun resolveRun(ocrRunId: String): AndroidOcrSourceGeometry =
        client.resolveOcrRunSourceGeometry(ocrRunId).toAndroid(File(client.libraryPath()).canonicalFile)

    fun resolve(
        scanId: String,
        inputKind: FfiOcrInputKind,
    ): AndroidOcrSourceGeometry {
        val libraryRoot = File(client.libraryPath()).canonicalFile
        return client.resolveOcrSourceGeometry(scanId, inputKind).toAndroid(libraryRoot)
    }
}

private fun FfiOcrSourceGeometry.toAndroid(libraryRoot: File): AndroidOcrSourceGeometry {
    val sourceFile = libraryRoot.resolve(relativePath).canonicalFile
    require(sourceFile.path == libraryRoot.path || sourceFile.path.startsWith(libraryRoot.path + File.separator)) {
        "OCR source path must stay under the local library root"
    }
    return AndroidOcrSourceGeometry(
        scanId = scanId,
        inputAssetId = inputAssetId,
        inputKind = inputKind,
        mediaType = mediaType,
        relativePath = relativePath,
        absolutePath = sourceFile.absolutePath,
        byteLength = byteLength,
        widthPx = widthPx,
        heightPx = heightPx,
    )
}
