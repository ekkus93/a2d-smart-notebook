package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
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
import com.a2d.notebook.feature.ocr.AndroidOcrProvider
import com.a2d.notebook.feature.ocr.AndroidOcrQueueProcessor
import com.a2d.notebook.feature.ocr.AndroidOcrQueueStep
import com.a2d.notebook.feature.ocr.AndroidOcrQueueJobStatus
import com.a2d.notebook.feature.ocr.AndroidOcrWorkflow
import com.a2d.notebook.feature.ocr.AndroidOcrRecognitionOutcome
import com.a2d.notebook.feature.ocr.AndroidRecognizedTextRegion
import com.a2d.notebook.feature.ocr.FfiAndroidOcrQueueGateway
import com.a2d.notebook.feature.ocr.FfiAndroidOcrReadback
import com.a2d.notebook.feature.ocr.FfiRustOcrGateway
import com.a2d.notebook.feature.ocr.OcrTextPoint
import com.a2d.notebook.feature.ocr.OcrRegionOverlayTestTags
import com.a2d.notebook.feature.ocr.OcrInputKind as AndroidOcrInputKind
import com.a2d.notebook.navigation.A2dDestinations
import com.a2d.notebook.navigation.A2dNavHost
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.ListOcrCorrectionsForScanRequest
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OpenLibraryRequest
import uniffi.a2d_ffi.PageResolution
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

            // DEV-PAGE-V1's gutter-side marker inset is wider than this photographed fixture's
            // symmetric crop. Preserve every photographed pixel and AprilTag at native resolution,
            // but add a bounded 20%-of-source camera margin on each side. That is enough room for
            // the layout's roughly 9% additional gutter extrapolation without the much larger
            // coordinate translation and decoder working set produced by the earlier 50% frame.
            val staging = root.resolve("tmp/scanner-staging/ocr-search-persisted.png")
            staging.parentFile?.mkdirs()
            val instrumentationAssets = InstrumentationRegistry.getInstrumentation().context.assets
            val sourceBitmap =
                instrumentationAssets.open("perspective-mild.png").use { input ->
                    requireNotNull(BitmapFactory.decodeStream(input))
                }
            try {
                val padX = sourceBitmap.width / 5
                val padY = sourceBitmap.height / 5
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
            val geometry = client.resolveOcrSourceGeometry(registered.scanId, OcrInputKind.ORIGINAL)
            val queue = FfiAndroidOcrQueueGateway(client)
            val queued = queue.enqueue(registered.scanId, AndroidOcrInputKind.Original, geometry.widthPx, geometry.heightPx)
            val provider = object : AndroidOcrProvider {
                override fun recognize(input: com.a2d.notebook.feature.ocr.PreparedAndroidOcrInput) =
                    AndroidOcrRecognitionOutcome.Detected(
                        provider = "production-navigation-sentinel", providerVersion = "1",
                        modelName = "deterministic", fullText = "persisted production notebook sentinel",
                        completedAtMs = System.currentTimeMillis(),
                        regions = listOf(AndroidRecognizedTextRegion(
                            polygon = listOf(OcrTextPoint(1f, 1f), OcrTextPoint(20f, 1f), OcrTextPoint(20f, 20f)),
                            text = "overlay region", confidence = 0.9f, createdAtMs = System.currentTimeMillis(),
                        )),
                    )
            }
            val step = AndroidOcrQueueProcessor(queue, AndroidOcrWorkflow(FfiRustOcrGateway(client), provider)).processNext()
            assertTrue(step is AndroidOcrQueueStep.Completed)
            val job = (step as AndroidOcrQueueStep.Completed).job
            assertEquals(queued.jobId, job.jobId)
            assertEquals(AndroidOcrQueueJobStatus.Recognized, job.status)
            val output = FfiAndroidOcrReadback(client).loadLatestOcrOutput(registered.scanId)
            val recorded = requireNotNull(output.latestRun)
            assertEquals(job.lastOcrRunId, recorded.ocrRunId)
            assertEquals(1, recorded.textRegions.size)
            assertEquals(geometry.inputAssetId, requireNotNull(output.sourceGeometry).inputAssetId)

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
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText(
                    "persisted production notebook sentinel",
                    substring = true,
                ).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText(
                "persisted production notebook sentinel",
                substring = true,
            ).assertIsDisplayed()

            composeRule.onNodeWithTag(PageViewerTestTags.OCR_SOURCE_GEOMETRY).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText("OCR source: ${geometry.widthPx}×${geometry.heightPx} px (Original)").assertIsDisplayed()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithTag(OcrRegionOverlayTestTags.CANVAS).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(OcrRegionOverlayTestTags.CANVAS).performScrollTo().assertIsDisplayed()

            // Continue through the production Page Viewer correction seam instead of switching to
            // a test-only controller. This makes one bounded scenario sentinel the Search controller,
            // hit navigation, persisted OCR readback, correction wiring, and durable correction history.
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_CORRECTION)
                .performScrollTo()
                .assertIsDisplayed()
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_CORRECTION_INPUT)
                .performScrollTo()
                .performTextInput("corrected production notebook sentinel")
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_CORRECTION_SUBMIT)
                .performScrollTo()
                .assertIsEnabled()
                .performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText(
                    "Corrected text: corrected production notebook sentinel",
                    substring = true,
                ).fetchSemanticsNodes().isNotEmpty()
            }

            val corrections =
                client.listOcrCorrectionsForScan(
                    ListOcrCorrectionsForScanRequest(scanId = registered.scanId, limit = 10u),
                )
            assertEquals(1, corrections.corrections.size)
            assertEquals(
                "corrected production notebook sentinel",
                corrections.corrections.single().correctedText,
            )
            assertEquals(
                "persisted production notebook sentinel",
                corrections.corrections.single().previousText,
            )

            // Recreate the production route and prove correction history is hydrated from Rust, not
            // retained only in the previous composition's Kotlin state.
            composeRule.activity.setContent {
                MaterialTheme {
                    val navController = rememberNavController()
                    LaunchedEffect(pageId, registered.scanId) {
                        navController.navigate(A2dDestinations.pageViewer(pageId, registered.scanId))
                    }
                    A2dNavHost(navController = navController, client = client)
                }
            }
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText(
                    "Corrected text: corrected production notebook sentinel",
                    substring = true,
                ).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText(
                "Text preview: persisted production notebook sentinel",
                substring = true,
            ).assertIsDisplayed()
        } finally {
            root.deleteRecursively()
        }
    }
}
