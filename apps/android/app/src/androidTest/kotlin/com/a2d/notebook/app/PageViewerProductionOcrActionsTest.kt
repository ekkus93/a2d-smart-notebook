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
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.navigation.compose.rememberNavController
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.feature.library.PageViewerTestTags
import com.a2d.notebook.navigation.A2dDestinations
import com.a2d.notebook.navigation.A2dNavHost
import java.io.File
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.CompleteOcrJobRequest
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.EnqueueOcrJobRequest
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OcrQueueJobStatus
import uniffi.a2d_ffi.OcrRunStatus
import uniffi.a2d_ffi.OcrUnavailableReason
import uniffi.a2d_ffi.OpenLibraryRequest
import uniffi.a2d_ffi.PageResolution
import uniffi.a2d_ffi.PrepareOcrInputRequest
import uniffi.a2d_ffi.RecordOcrRunRequest
import uniffi.a2d_ffi.RegisteredScan
import uniffi.a2d_ffi.RegistrationImageFormat
import uniffi.a2d_ffi.RegistrationImageRotation
import uniffi.a2d_ffi.RegistrationMarker
import uniffi.a2d_ffi.RegisterScanRequest
import uniffi.a2d_ffi.ScanCaptureSource

@RunWith(AndroidJUnit4::class)
class PageViewerProductionOcrActionsTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun pageViewerStartAndCancelUseRustDurableQueue() {
        val root = composeRule.activity.filesDir.resolve("page-viewer-ocr-actions-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
        try {
            val scan = registerRealScan(client = client, root = root)
            openProductionPageViewer(client = client, scan = scan)
            composeRule.onNodeWithTag(PageViewerTestTags.TITLE).assertIsDisplayed()
            composeRule.onNodeWithText("Page ID: ${scan.pageId}").assertIsDisplayed()
            composeRule.onNodeWithText("Scan ID: ${scan.scanId}").assertIsDisplayed()
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_START).performScrollTo().performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText("OCR queued as durable job", substring = true).fetchSemanticsNodes().isNotEmpty()
            }
            val runningJob = requireNotNull(client.claimNextOcrJob()) { "Start OCR must create a Rust-owned durable queued job" }
            assertEquals(scan.scanId, runningJob.scanId)
            assertEquals(OcrQueueJobStatus.RUNNING, runningJob.status)
            assertEquals(1u, runningJob.attemptCount)
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_CANCEL).performScrollTo().performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText("cancellation requested", substring = true, ignoreCase = true).fetchSemanticsNodes().isNotEmpty()
            }
            val cancelledJob = client.getOcrJob(runningJob.jobId)
            assertEquals(runningJob.jobId, cancelledJob.jobId)
            assertEquals(OcrQueueJobStatus.RUNNING, cancelledJob.status)
            assertTrue(cancelledJob.cancellationRequested)
        } finally { root.deleteRecursively() }
    }

    @Test
    fun pageViewerRouteOpenHydratesQueuedOcrJobFromRustQueue() {
        val root = composeRule.activity.filesDir.resolve("page-viewer-ocr-queued-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
        try {
            val scan = registerRealScan(client = client, root = root)
            val queuedJob = client.enqueueOcrJob(EnqueueOcrJobRequest(scanId = scan.scanId, inputKind = OcrInputKind.ORIGINAL, widthPx = 1800u, heightPx = 2200u))
            assertEquals(OcrQueueJobStatus.QUEUED, queuedJob.status)
            openProductionPageViewer(client = client, scan = scan)
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText("OCR queued as durable job ${queuedJob.jobId}", substring = true).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText("OCR queued as durable job ${queuedJob.jobId}", substring = true).assertIsDisplayed()
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_CANCEL).performScrollTo().assertIsEnabled()
        } finally { root.deleteRecursively() }
    }

    @Test
    fun pageViewerHydratesDetectedOcrFromRustReadback() {
        val root = composeRule.activity.filesDir.resolve("page-viewer-ocr-detected-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
        try {
            val scan = registerRealScan(client = client, root = root)
            val recorded = recordTerminalOcr(client, scan, OcrRunStatus.DETECTED, "durable detected page viewer sentinel", null, null)
            openProductionPageViewer(client = client, scan = scan)
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText("Recognized text is available", substring = true).assertIsDisplayed()
            composeRule.onNodeWithText("OCR run: ${recorded.ocrRunId}").assertIsDisplayed()
            composeRule.onNodeWithText("Provider: instrumentation-fixture").assertIsDisplayed()
            composeRule.onNodeWithText("Model: deterministic").assertIsDisplayed()
            composeRule.onNodeWithText("durable detected page viewer sentinel", substring = true).assertIsDisplayed()
        } finally { root.deleteRecursively() }
    }

    @Test
    fun pageViewerHydratesNoTextDetectedDistinctlyFromEmptyDetectedText() {
        val root = composeRule.activity.filesDir.resolve("page-viewer-ocr-no-text-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
        try {
            val scan = registerRealScan(client = client, root = root)
            val recorded = recordTerminalOcr(client, scan, OcrRunStatus.NO_TEXT_DETECTED, "", null, null)
            openProductionPageViewer(client = client, scan = scan)
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText("detected no text", substring = true).assertIsDisplayed()
            composeRule.onNodeWithText("OCR run: ${recorded.ocrRunId}").assertIsDisplayed()
            assertTrue(composeRule.onAllNodesWithText("Text preview:", substring = true).fetchSemanticsNodes().isEmpty())
        } finally { root.deleteRecursively() }
    }

    @Test
    fun pageViewerHydratesUnavailableOcrAndRetriesThroughDurableQueue() {
        val root = composeRule.activity.filesDir.resolve("page-viewer-ocr-unavailable-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
        try {
            val scan = registerRealScan(client = client, root = root)
            val recorded = recordTerminalOcr(client, scan, OcrRunStatus.UNAVAILABLE, "", OcrUnavailableReason.PROVIDER_FAILED, "provider failed before returning text")
            openProductionPageViewer(client = client, scan = scan)
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText("OCR unavailable: provider failed", substring = true).assertIsDisplayed()
            composeRule.onNodeWithText("provider failed before returning text", substring = true).assertIsDisplayed()
            composeRule.onNodeWithText("OCR run: ${recorded.ocrRunId}").assertIsDisplayed()
            composeRule.onNodeWithTag(PageViewerTestTags.OCR_RETRY).performScrollTo().assertIsEnabled().performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) { composeRule.onAllNodesWithText("OCR queued as durable job", substring = true).fetchSemanticsNodes().isNotEmpty() }
            val retryJob = requireNotNull(client.claimNextOcrJob()) { "Retry OCR must enqueue a Rust-owned durable job" }
            assertEquals(scan.scanId, retryJob.scanId)
            assertEquals(OcrQueueJobStatus.RUNNING, retryJob.status)
        } finally { root.deleteRecursively() }
    }

    @Test
    fun routeReopenPreservesDurableRecognizedStateFromRust() {
        val root = composeRule.activity.filesDir.resolve("page-viewer-ocr-reopen-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
        try {
            val scan = registerRealScan(client = client, root = root)
            recordTerminalOcr(client, scan, OcrRunStatus.DETECTED, "route reopen durable OCR sentinel", null, null)
            openProductionPageViewer(client = client, scan = scan)
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText("route reopen durable OCR sentinel", substring = true).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithText("route reopen durable OCR sentinel", substring = true).assertIsDisplayed()
            openProductionPageViewer(client = client, scan = scan)
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithText("route reopen durable OCR sentinel", substring = true).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(PageViewerTestTags.TEXT).performScrollTo().assertIsDisplayed()
            composeRule.onNodeWithText("route reopen durable OCR sentinel", substring = true).assertIsDisplayed()
        } finally { root.deleteRecursively() }
    }

    private fun openProductionPageViewer(client: A2dClient, scan: RegisteredScan) {
        composeRule.activity.setContent {
            MaterialTheme {
                val navController = rememberNavController()
                LaunchedEffect(scan.pageId, scan.scanId) { navController.navigate(A2dDestinations.pageViewer(scan.pageId, scan.scanId)) }
                A2dNavHost(navController = navController, client = client)
            }
        }
    }

    private fun recordTerminalOcr(client: A2dClient, scan: RegisteredScan, status: OcrRunStatus, text: String, unavailableReason: OcrUnavailableReason?, unavailableMessage: String?): uniffi.a2d_ffi.RecordedOcrRun {
        val prepared = client.prepareOcrInput(PrepareOcrInputRequest(scanId = scan.scanId, inputKind = OcrInputKind.ORIGINAL, widthPx = 1800u, heightPx = 2200u))
        return client.recordOcrRun(RecordOcrRunRequest(scanId = scan.scanId, inputAssetId = prepared.inputAssetId, provider = "instrumentation-fixture", providerVersion = "1", modelName = "deterministic", status = status, fullText = text, unavailableReason = unavailableReason, unavailableMessage = unavailableMessage, completedAtMs = System.currentTimeMillis(), warnings = emptyList()))
    }

    @Suppress("unused")
    private fun completeRunningJobWithOcrRun(client: A2dClient, jobId: String, ocrRunId: String, retryable: Boolean) = client.completeOcrJob(CompleteOcrJobRequest(jobId = jobId, ocrRunId = ocrRunId, retryable = retryable))

    private fun registerRealScan(client: A2dClient, root: File): RegisteredScan {
        val setupPayload = "A2D:1:S:6DE28E53DBKPXCWWNHPC8T7QJX:0V10W2Y"
        val notebook = client.createNotebook(CreateNotebookRequest(setupPayload = setupPayload, displayName = "Page Viewer OCR actions fixture", optionalColor = "blue", optionalIcon = "notebook", optionalUserNotes = "R2 production Page Viewer durable action fixture", makeActive = true))
        val pagePayload = "A2D:1:B:6DE28E53DBKPXCWWNHPC8T7QJX:1:DEV-PAGE-V1:02V2GRM"
        val resolution = client.resolvePageCode(pagePayload, notebook.notebook.id)
        assertTrue(resolution is PageResolution.Resolved)
        val resolved = resolution as PageResolution.Resolved
        val staging = root.resolve("tmp/scanner-staging/page-viewer-ocr-actions.png")
        staging.parentFile?.mkdirs()
        writeFramedFixture(staging)
        return client.registerScan(RegisterScanRequest(stagingPath = staging.canonicalPath, pageCodePayload = pagePayload, expectedPageId = resolved.pageId, activeNotebookId = notebook.notebook.id, captureSource = ScanCaptureSource.IMPORT, imageFormat = RegistrationImageFormat.PNG, imageRotation = RegistrationImageRotation.DEGREES0, capturedAtMs = System.currentTimeMillis(), observedMarkers = listOf(RegistrationMarker(role = "TL", id = 0u), RegistrationMarker(role = "TR", id = 1u), RegistrationMarker(role = "BR", id = 2u), RegistrationMarker(role = "BL", id = 3u)), previewWarnings = listOf("A2D_POLICY_LAYOUT=DEV-PAGE-V1", "A2D_POLICY_VERSION=1", "A2D_PIPELINE_VERSION=1"), recoveryToken = null, userApproved = true))
    }

    private fun writeFramedFixture(staging: File) {
        val instrumentationAssets = InstrumentationRegistry.getInstrumentation().context.assets
        val sourceBitmap = instrumentationAssets.open("perspective-mild.png").use { input -> requireNotNull(BitmapFactory.decodeStream(input)) }
        try {
            val padX = sourceBitmap.width / 5
            val padY = sourceBitmap.height / 5
            val framed = Bitmap.createBitmap(sourceBitmap.width + (padX * 2), sourceBitmap.height + (padY * 2), Bitmap.Config.ARGB_8888)
            try {
                val canvas = Canvas(framed)
                canvas.drawColor(android.graphics.Color.WHITE)
                canvas.drawBitmap(sourceBitmap, padX.toFloat(), padY.toFloat(), null)
                staging.outputStream().use { output -> check(framed.compress(Bitmap.CompressFormat.PNG, 100, output)) }
            } finally { framed.recycle() }
        } finally { sourceBitmap.recycle() }
    }
}
