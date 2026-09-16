package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.navigation.compose.rememberNavController
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.feature.home.HomeScreenTestTags
import com.a2d.notebook.feature.library.LibraryHubTestTags
import com.a2d.notebook.feature.library.PageViewerTestTags
import com.a2d.notebook.feature.ocr.OcrSearchTestTags
import com.a2d.notebook.navigation.A2dNavHost
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OcrRunStatus
import uniffi.a2d_ffi.OpenLibraryRequest
import uniffi.a2d_ffi.PageResolution
import uniffi.a2d_ffi.PrepareOcrInputRequest
import uniffi.a2d_ffi.RecordOcrRunRequest
import uniffi.a2d_ffi.RegistrationImageFormat
import uniffi.a2d_ffi.RegistrationImageRotation
import uniffi.a2d_ffi.RegistrationMarker
import uniffi.a2d_ffi.RegisterScanRequest
import uniffi.a2d_ffi.ScanCaptureSource

@RunWith(AndroidJUnit4::class)
class OcrSearchProductionNavigationTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun realNavGraphUsesProductionController() {
        val root = composeRule.activity.filesDir.resolve("ocr-search-nav-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))

        try {
            composeRule.activity.setContent {
                MaterialTheme {
                    A2dNavHost(
                        navController = rememberNavController(),
                        client = client,
                    )
                }
            }

            composeRule.onNodeWithTag(HomeScreenTestTags.LIBRARY).performScrollTo().performClick()
            composeRule.onNodeWithTag(LibraryHubTestTags.OCR_SEARCH).performScrollTo().performClick()
            composeRule.onNodeWithTag(OcrSearchTestTags.TITLE).assertIsDisplayed()

            // A valid query against a newly opened empty library deterministically returns no
            // matches through Rust. The nullable fallback instead renders an error card containing
            // "not connected", so this distinguishes the real production controller without
            // depending on SQLite FTS accepting or rejecting a particular punctuation query.
            composeRule.onNodeWithTag(OcrSearchTestTags.QUERY_FIELD).performTextInput("notebook")
            composeRule.onNodeWithTag(OcrSearchTestTags.SUBMIT).performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithTag(OcrSearchTestTags.NO_MATCHES).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(OcrSearchTestTags.NO_MATCHES).assertIsDisplayed()
            composeRule.onNodeWithText("not connected", substring = true, ignoreCase = true).assertDoesNotExist()
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun persistedRustOcrSearchHitUsesProductionNavGraphToOpenPageViewer() {
        val root = composeRule.activity.filesDir.resolve("ocr-search-persisted-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))

        try {
            val setupPayload = "A2D:1:S:6DE28E53DBKPXCWWNHPC8T7QJX:0V10W2Y"
            val notebook =
                client.createNotebook(
                    CreateNotebookRequest(
                        setupPayload = setupPayload,
                        displayName = "OCR production navigation fixture",
                        optionalColor = "blue",
                        optionalIcon = "notebook",
                        optionalUserNotes = "R1 persisted OCR search fixture",
                        makeActive = true,
                    ),
                )
            val pagePayload = "A2D:1:B:6DE28E53DBKPXCWWNHPC8T7QJX:1:DEV-PAGE-V1:02V2GRM"
            val resolution = client.resolvePageCode(pagePayload, notebook.notebook.id)
            assertTrue(resolution is PageResolution.Resolved)
            val resolved = resolution as PageResolution.Resolved
            assertEquals(notebook.notebook.id, resolved.notebookId)
            val pageId = resolved.pageId

            // The photographed-analysis fixture is intentionally tight around its synthetic page.
            // DEV-PAGE-V1 has an asymmetric notebook gutter, so production rectification can
            // legitimately extrapolate beyond that crop. Embed the unchanged photographed fixture
            // in a generous white camera-frame margin: marker pixels and perspective stay real,
            // while the source image now contains the physical-page extent implied by the layout.
            val staging = root.resolve("tmp/scanner-staging/ocr-search-persisted.png")
            staging.parentFile?.mkdirs()
            val instrumentationAssets = InstrumentationRegistry.getInstrumentation().context.assets
            val sourceBitmap =
                instrumentationAssets.open("perspective-mild.png").use { input ->
                    requireNotNull(BitmapFactory.decodeStream(input))
                }
            try {
                val padX = sourceBitmap.width
                val padY = sourceBitmap.height
                val framed =
                    Bitmap.createBitmap(
                        sourceBitmap.width + (padX * 2),
                        sourceBitmap.height + (padY * 2),
                        Bitmap.Config.ARGB_8888,
                    )
                try {
                    val canvas = Canvas(framed)
                    canvas.drawColor(android.graphics.Color.WHITE)
                    canvas.drawBitmap(sourceBitmap, padX.toFloat(), padY.toFloat(), null)
                    staging.outputStream().use { output ->
                        check(framed.compress(Bitmap.CompressFormat.PNG, 100, output))
                    }
                } finally {
                    framed.recycle()
                }
            } finally {
                sourceBitmap.recycle()
            }
            val registered =
                client.registerScan(
                    RegisterScanRequest(
                        stagingPath = staging.canonicalPath,
                        pageCodePayload = pagePayload,
                        expectedPageId = pageId,
                        activeNotebookId = notebook.notebook.id,
                        captureSource = ScanCaptureSource.IMPORT,
                        imageFormat = RegistrationImageFormat.PNG,
                        imageRotation = RegistrationImageRotation.DEGREES0,
                        capturedAtMs = System.currentTimeMillis(),
                        observedMarkers =
                            listOf(
                                RegistrationMarker(role = "TL", id = 0u),
                                RegistrationMarker(role = "TR", id = 1u),
                                RegistrationMarker(role = "BR", id = 2u),
                                RegistrationMarker(role = "BL", id = 3u),
                            ),
                        previewWarnings =
                            listOf(
                                "A2D_POLICY_LAYOUT=DEV-PAGE-V1",
                                "A2D_POLICY_VERSION=1",
                                "A2D_PIPELINE_VERSION=1",
                            ),
                        recoveryToken = null,
                        userApproved = true,
                    ),
                )
            assertEquals(pageId, registered.pageId)
            val prepared =
                client.prepareOcrInput(
                    PrepareOcrInputRequest(
                        scanId = registered.scanId,
                        inputKind = OcrInputKind.ORIGINAL,
                        widthPx = 1800u,
                        heightPx = 2200u,
                    ),
                )
            val recorded =
                client.recordOcrRun(
                    RecordOcrRunRequest(
                        scanId = registered.scanId,
                        inputAssetId = prepared.inputAssetId,
                        provider = "instrumentation-fixture",
                        providerVersion = "1",
                        modelName = "deterministic",
                        status = OcrRunStatus.DETECTED,
                        fullText = "persisted production notebook sentinel",
                        unavailableReason = null,
                        unavailableMessage = null,
                        completedAtMs = System.currentTimeMillis(),
                        warnings = emptyList(),
                    ),
                )

            composeRule.activity.setContent {
                MaterialTheme {
                    A2dNavHost(
                        navController = rememberNavController(),
                        client = client,
                    )
                }
            }
            composeRule.onNodeWithTag(HomeScreenTestTags.LIBRARY).performScrollTo().performClick()
            composeRule.onNodeWithTag(LibraryHubTestTags.OCR_SEARCH).performScrollTo().performClick()
            composeRule.onNodeWithTag(OcrSearchTestTags.QUERY_FIELD).performTextInput("sentinel")
            composeRule.onNodeWithTag(OcrSearchTestTags.SUBMIT).performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithTag(OcrSearchTestTags.OPEN_PAGE).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithText(registered.scanId, substring = true).assertIsDisplayed()
            composeRule.onNodeWithText(recorded.ocrRunId, substring = true).assertIsDisplayed()
            composeRule.onNodeWithTag(OcrSearchTestTags.OPEN_PAGE).performScrollTo().performClick()
            composeRule.onNodeWithTag(PageViewerTestTags.TITLE).assertIsDisplayed()
            composeRule.onNodeWithText("Page ID: $pageId").assertIsDisplayed()
        } finally {
            root.deleteRecursively()
        }
    }
}
