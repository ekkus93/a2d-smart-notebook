package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.resolveGeneratedPdfAsset
import java.io.File
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class GeneratedPdfAssetValidationTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun exactLibraryExportAssetResolves() {
        val assetId = "test-export-${System.nanoTime()}"
        val exports = A2dBridge.libraryDirectory(context).resolve("assets/exports")
        assertTrue(exports.exists() || exports.mkdirs())
        val file = exports.resolve(assetId)
        file.writeBytes("%PDF-test".toByteArray())

        try {
            assertEquals(
                file.canonicalFile,
                resolveGeneratedPdfAsset(context, assetId, file.absolutePath),
            )
        } finally {
            file.delete()
        }
    }

    @Test
    fun mismatchedMissingAndOutsidePathsFailExplicitly() {
        val assetId = "test-export-${System.nanoTime()}"
        val exports = A2dBridge.libraryDirectory(context).resolve("assets/exports")
        assertTrue(exports.exists() || exports.mkdirs())
        val file = exports.resolve(assetId)
        file.writeBytes("%PDF-test".toByteArray())
        try {
            assertIOException {
                resolveGeneratedPdfAsset(context, "different-asset", file.absolutePath)
            }
            assertIOException {
                resolveGeneratedPdfAsset(context, assetId, exports.resolve("missing").absolutePath)
            }
            val outside = File.createTempFile("outside-export-", ".pdf", context.cacheDir)
            try {
                assertIOException {
                    resolveGeneratedPdfAsset(context, outside.name, outside.absolutePath)
                }
            } finally {
                outside.delete()
            }
        } finally {
            file.delete()
        }
    }

    private fun assertIOException(block: () -> Unit) {
        try {
            block()
        } catch (_: IOException) {
            return
        }
        throw AssertionError("expected IOException")
    }
}
