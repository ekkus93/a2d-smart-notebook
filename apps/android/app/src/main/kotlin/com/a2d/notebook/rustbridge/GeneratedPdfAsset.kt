package com.a2d.notebook.rustbridge

import android.content.Context
import java.io.File
import java.io.IOException
import java.nio.file.Files
import java.nio.file.LinkOption

/**
 * Resolves a Rust-returned generated PDF path as a library-owned Export asset. This is a strict
 * platform-side containment check until the generated bindings expose the Rust verified-asset
 * resolver directly.
 */
fun resolveGeneratedPdfAsset(
    context: Context,
    assetId: String,
    path: String,
): File {
    if (assetId.isBlank()) throw IOException("generated PDF asset ID is missing")
    if (path.isBlank()) throw IOException("generated PDF path is missing")

    val libraryRoot = A2dBridge.libraryDirectory(context).canonicalFile
    val exportsRoot = libraryRoot.resolve("assets/exports").canonicalFile
    val candidate = File(path)
    if (Files.isSymbolicLink(candidate.toPath())) {
        throw IOException("generated PDF path is a symbolic link")
    }
    val canonicalCandidate = try {
        candidate.canonicalFile
    } catch (failure: IOException) {
        throw IOException("generated PDF path cannot be canonicalized", failure)
    }
    if (canonicalCandidate.parentFile != exportsRoot) {
        throw IOException("generated PDF is outside the library export directory")
    }
    if (canonicalCandidate.name != assetId) {
        throw IOException("generated PDF path does not match its asset ID")
    }
    if (!Files.isRegularFile(canonicalCandidate.toPath(), LinkOption.NOFOLLOW_LINKS)) {
        throw IOException("generated PDF is missing or not a regular file")
    }
    return canonicalCandidate
}
