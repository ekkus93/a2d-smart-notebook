package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.test.assertIsDisplayed
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
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.OcrQueueJobStatus
import uniffi.a2d_ffi.PageResolution
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
        val client = A2dClient.open(uniffi.a2d_ffi.OpenLibraryRequest(libraryPath = root.absolutePath))

        try {
            val scan = registerRealScan(client = client, root = root)

            composeRule.activity.setContent {
                MaterialTheme {
                    val navController = rememberNavController()
                    LaunchedEffect(scan.pageId, scan.scanId) {
                        navController.navigate(A2dDestinations.pageViewer(scan.pageId, scan.scanId))
                    }
                    A2dNavHost(navController = navController, client = client)
                }
            }

            composeRule.onNodeWithTag(PageViewerTestTags.TITLE).assertIsDisplayed()
            composeRule.onNodeWithText("Page ID: ${scan.pageId}").assertIsDisplayed()
            composeRule.onNodeWithText("Scan ID: ${scan.scanId}").assertIsDisplayed()

            composeRule.onNodeWithTag(PageViewerTestTags.OCR_START).performScrollTo().performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule
                    .onAllNodesWithText("OCR queued as durable job", substring = true)
                    .fetchSemanticsNodes()
                    .isNotEmpty()
            }

            val runningJob = requireNotNull(client.claimNextOcrJob()) {
                "Start OCR must create a Rust-owned durable queued job"
            }
            assertEquals(scan.scanId, runningJob.scanId)
            assertEquals(OcrQueueJobStatus.RUNNING, runningJob.status)
            assertEquals(1u, runningJob.attemptCount)

            composeRule.onNodeWithTag(PageViewerTestTags.OCR_CANCEL).performScrollTo().performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule
                    .onAllNodesWithText("cancellation requested", substring = true, ignoreCase = true)
                    .fetchSemanticsNodes()
                    .isNotEmpty()
            }

            val cancelledJob = client.getOcrJob(runningJob.jobId)
            assertEquals(runningJob.jobId, cancelledJob.jobId)
            assertEquals(OcrQueueJobStatus.RUNNING, cancelledJob.status)
            assertTrue(cancelledJob.cancellationRequested)
        } finally {
            root.deleteRecursively()
        }
    }

    private fun registerRealScan(client: A2dClient, root: File): RegisteredScan {
        val setupPayload = "A2D:1:S:6DE28E53DBKPXCWWNHPC8T7QJX:0V10W2Y"
        val notebook =
            client.createNotebook(
                CreateNotebookRequest(
                    setupPayload = setupPayload,
                    displayName = "Page Viewer OCR actions fixture",
                    optionalColor = "blue",
                    optionalIcon = "notebook",
                    optionalUserNotes = "R2 production Page Viewer durable action fixture",
                    makeActive = true,
                ),
            )
        val pagePayload = "A2D:1:B:6DE28E53DBKPXCWWNHPC8T7QJX:1:DEV-PAGE-V1:02V2GRM"
        val resolution = client.resolvePageCode(pagePayload, notebook.notebook.id)
        assertTrue(resolution is PageResolution.Resolved)
        val resolved = resolution as PageResolution.Resolved

        val staging = root.resolve("tmp/scanner-staging/page-viewer-ocr-actions.png")
        staging.parentFile?.mkdirs()
        writeFramedFixture(staging)

        return client.registerScan(
            RegisterScanRequest(
                stagingPath = staging.canonicalPath,
                pageCodePayload = pagePayload,
                expectedPageId = resolved.pageId,
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
    }

    private fun writeFramedFixture(staging: File) {
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
    }
}
