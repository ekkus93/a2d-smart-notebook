package com.a2d.notebook.feature.ocr

import com.google.mlkit.common.MlKitException
import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MlKitAndroidOcrProviderTest {
    @Test
    fun detectedTextAndRegionsPreserveRealProviderSemantics() {
        withProvider(
            LocalTextRecognitionResult(
                fullText = "hello notebook",
                regions =
                    listOf(
                        LocalTextRecognitionRegion(
                            polygon =
                                listOf(
                                    OcrTextPoint(-2f, 3f),
                                    OcrTextPoint(110f, 3f),
                                    OcrTextPoint(110f, 25f),
                                    OcrTextPoint(-2f, 25f),
                                ),
                            text = "hello notebook",
                            confidence = 0.91f,
                        ),
                    ),
            ),
        ) { provider, input, _ ->
            val outcome = provider.recognize(input) as AndroidOcrRecognitionOutcome.Detected

            assertEquals(MlKitAndroidOcrProvider.PROVIDER_NAME, outcome.provider)
            assertEquals(MlKitAndroidOcrProvider.PROVIDER_VERSION, outcome.providerVersion)
            assertEquals("hello notebook", outcome.fullText)
            assertEquals(1, outcome.regions.size)
            assertEquals(0f, outcome.regions.single().polygon.first().x)
            assertEquals(100f, outcome.regions.single().polygon[1].x)
            assertEquals(0.91f, outcome.regions.single().confidence)
            assertEquals(1234L, outcome.completedAtMs)
        }
    }

    @Test
    fun blankRecognitionIsNoTextDetectedNotDetectedEmptyText() {
        withProvider(LocalTextRecognitionResult(fullText = "  \n ", regions = emptyList())) { provider, input, _ ->
            val outcome = provider.recognize(input)

            assertTrue(outcome is AndroidOcrRecognitionOutcome.NoTextDetected)
            assertFalse(outcome is AndroidOcrRecognitionOutcome.Detected)
        }
    }

    @Test
    fun providerFailureMapsToUnavailableRatherThanFakeEmptyDetection() {
        withFailingProvider(
            LocalTextRecognitionException(
                kind = LocalTextRecognitionFailureKind.ProviderFailed,
                message = "recognizer crashed",
                retryAvailable = true,
            ),
        ) { outcome ->
            val unavailable = outcome as AndroidOcrRecognitionOutcome.Unavailable
            assertEquals(OcrUnavailableReason.ProviderFailed, unavailable.reason)
            assertTrue(unavailable.retryAvailable)
        }
    }

    @Test
    fun cancellationRemainsCancelled() {
        withFailingProvider(
            LocalTextRecognitionException(
                kind = LocalTextRecognitionFailureKind.Cancelled,
                message = "cancelled",
                retryAvailable = false,
            ),
        ) { outcome ->
            assertTrue(outcome is AndroidOcrRecognitionOutcome.Cancelled)
        }
    }

    @Test
    fun providerAndResourceAvailabilityFailuresStayExplicit() {
        listOf(
            LocalTextRecognitionFailureKind.ProviderUnavailable to OcrUnavailableReason.ProviderUnavailable,
            LocalTextRecognitionFailureKind.ResourceUnavailable to OcrUnavailableReason.ResourceUnavailable,
        ).forEach { (kind, expectedReason) ->
            withFailingProvider(
                LocalTextRecognitionException(
                    kind = kind,
                    message = kind.name,
                    retryAvailable = true,
                ),
            ) { outcome ->
                val unavailable = outcome as AndroidOcrRecognitionOutcome.Unavailable
                assertEquals(expectedReason, unavailable.reason)
            }
        }
    }

    @Test
    fun missingRustSelectedAssetIsResourceUnavailableWithoutCallingProvider() {
        val root = Files.createTempDirectory("a2d-mlkit-provider-").toFile()
        val engine = RecordingEngine(LocalTextRecognitionResult("should not run", emptyList()))
        val provider = MlKitAndroidOcrProvider(root, engine) { 1234L }
        val input = preparedInput(relativePath = "assets/missing.png", byteLength = 10u)

        val unavailable = provider.recognize(input) as AndroidOcrRecognitionOutcome.Unavailable

        assertEquals(OcrUnavailableReason.ResourceUnavailable, unavailable.reason)
        assertFalse(engine.called)
        root.deleteRecursively()
    }

    @Test
    fun pathTraversalIsUnsupportedWithoutCallingProvider() {
        val root = Files.createTempDirectory("a2d-mlkit-provider-").toFile()
        val engine = RecordingEngine(LocalTextRecognitionResult("should not run", emptyList()))
        val provider = MlKitAndroidOcrProvider(root, engine) { 1234L }
        val input = preparedInput(relativePath = "../outside.png", byteLength = 10u)

        val unavailable = provider.recognize(input) as AndroidOcrRecognitionOutcome.Unavailable

        assertEquals(OcrUnavailableReason.UnsupportedInput, unavailable.reason)
        assertFalse(engine.called)
        root.deleteRecursively()
    }

    @Test
    fun mlKitErrorCodesPreserveTerminalCategories() {
        assertEquals(
            LocalTextRecognitionFailureKind.Cancelled,
            classifyMlKitErrorCode(MlKitException.CANCELLED).kind,
        )
        assertEquals(
            LocalTextRecognitionFailureKind.ProviderUnavailable,
            classifyMlKitErrorCode(MlKitException.UNAVAILABLE).kind,
        )
        assertEquals(
            LocalTextRecognitionFailureKind.ResourceUnavailable,
            classifyMlKitErrorCode(MlKitException.RESOURCE_EXHAUSTED).kind,
        )
        assertEquals(
            LocalTextRecognitionFailureKind.UnsupportedInput,
            classifyMlKitErrorCode(MlKitException.UNSUPPORTED).kind,
        )
    }

    private fun withFailingProvider(
        failure: LocalTextRecognitionException,
        assertion: (AndroidOcrRecognitionOutcome) -> Unit,
    ) {
        val root = Files.createTempDirectory("a2d-mlkit-provider-").toFile()
        val asset = root.resolve("assets/ocr/input.png").apply {
            parentFile?.mkdirs()
            writeBytes(byteArrayOf(1, 2, 3, 4))
        }
        val provider =
            MlKitAndroidOcrProvider(
                libraryRoot = root,
                engine = ThrowingEngine(failure),
                nowMs = { 1234L },
            )
        assertion(provider.recognize(preparedInput(byteLength = asset.length().toULong())))
        root.deleteRecursively()
    }

    private fun withProvider(
        result: LocalTextRecognitionResult,
        assertion: (MlKitAndroidOcrProvider, PreparedAndroidOcrInput, File) -> Unit,
    ) {
        val root = Files.createTempDirectory("a2d-mlkit-provider-").toFile()
        val asset = root.resolve("assets/ocr/input.png").apply {
            parentFile?.mkdirs()
            writeBytes(byteArrayOf(1, 2, 3, 4))
        }
        val provider =
            MlKitAndroidOcrProvider(
                libraryRoot = root,
                engine = RecordingEngine(result),
                nowMs = { 1234L },
            )
        assertion(provider, preparedInput(byteLength = asset.length().toULong()), asset)
        root.deleteRecursively()
    }

    private fun preparedInput(
        relativePath: String = "assets/ocr/input.png",
        byteLength: ULong = 4u,
    ): PreparedAndroidOcrInput =
        PreparedAndroidOcrInput(
            scanId = "scan-1",
            inputAssetId = "asset-1",
            inputKind = OcrInputKind.OcrOptimized,
            mediaType = "image/png",
            relativePath = relativePath,
            byteLength = byteLength,
            widthPx = 100u,
            heightPx = 100u,
        )

    private class RecordingEngine(
        private val result: LocalTextRecognitionResult,
    ) : LocalTextRecognitionEngine {
        var called = false

        override fun recognize(imageFile: File): LocalTextRecognitionResult {
            called = true
            return result
        }
    }

    private class ThrowingEngine(
        private val failure: LocalTextRecognitionException,
    ) : LocalTextRecognitionEngine {
        override fun recognize(imageFile: File): LocalTextRecognitionResult = throw failure
    }
}
