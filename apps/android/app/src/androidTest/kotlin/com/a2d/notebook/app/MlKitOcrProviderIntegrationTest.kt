package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.feature.ocr.AndroidOcrRecognitionOutcome
import com.a2d.notebook.feature.ocr.MlKitAndroidOcrProvider
import com.a2d.notebook.feature.ocr.OcrInputKind
import com.a2d.notebook.feature.ocr.PreparedAndroidOcrInput
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MlKitOcrProviderIntegrationTest {
    @Test
    fun bundledProviderRecognizesLocalFixtureWithoutNetworkModelDownload() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val targetContext = instrumentation.targetContext
        val testContext = instrumentation.context
        val libraryRoot = targetContext.filesDir.resolve("mlkit-provider-integration").apply {
            deleteRecursively()
            mkdirs()
        }
        val relativePath = "assets/ocr/base-page.png"
        val inputFile = libraryRoot.resolve(relativePath)
        inputFile.parentFile?.mkdirs()
        testContext.assets.open("base-page.png").use { source ->
            inputFile.outputStream().use(source::copyTo)
        }

        val outcome =
            MlKitAndroidOcrProvider.create(targetContext, libraryRoot).use { provider ->
                provider.recognize(
                    PreparedAndroidOcrInput(
                        scanId = "integration-scan",
                        inputAssetId = "integration-asset",
                        inputKind = OcrInputKind.Original,
                        mediaType = "image/png",
                        relativePath = relativePath,
                        byteLength = inputFile.length().toULong(),
                        widthPx = 1080u,
                        heightPx = 1440u,
                    ),
                )
            }

        assertTrue(
            "Bundled ML Kit should detect fixture text, got $outcome",
            outcome is AndroidOcrRecognitionOutcome.Detected,
        )
        val detected = outcome as AndroidOcrRecognitionOutcome.Detected
        assertTrue(detected.fullText.contains("Tasks", ignoreCase = true))
        assertTrue(detected.regions.isNotEmpty())
        libraryRoot.deleteRecursively()
    }
}
