package com.a2d.notebook.app

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
import com.a2d.notebook.feature.home.HomeScreenTestTags
import com.a2d.notebook.feature.library.LibraryHubTestTags
import com.a2d.notebook.feature.library.PageViewerTestTags
import com.a2d.notebook.feature.ocr.OcrSearchTestTags
import com.a2d.notebook.navigation.A2dNavHost
import java.util.UUID
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OcrRunStatus
import uniffi.a2d_ffi.OpenLibraryRequest
import uniffi.a2d_ffi.PrepareOcrInputRequest
import uniffi.a2d_ffi.RecordOcrRunRequest
import uniffi.a2d_ffi.RegistrationImageFormat
import uniffi.a2d_ffi.RegistrationImageRotation
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
            // These are canonical v1 payloads for the bundled notebook design, including the
            // CRC-32C field required by the accepted QR wire format.
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
            val pagePayload = "A2D:1:B:6DE28E53DBKPXCWWNHPC8T7QJX:1:USLETTER-LINED:3ATFTZA"
            val staging = root.resolve("tmp/ocr-search-persisted.png")
            staging.parentFile?.mkdirs()
            composeRule.activity.resources.assets.open("base-page.png").use { source ->
                staging.outputStream().use(source::copyTo)
            }
            val registered =
                client.registerScan(
                    RegisterScanRequest(
                        stagingPath = staging.canonicalPath,
                        pageCodePayload = pagePayload,
                        expectedPageId = "",
                        activeNotebookId = notebook.notebook.id,
                        captureSource = ScanCaptureSource.IMPORT,
                        imageFormat = RegistrationImageFormat.PNG,
                        imageRotation = RegistrationImageRotation.DEGREES0,
                        capturedAtMs = System.currentTimeMillis(),
                        observedMarkers = emptyList(),
                        previewWarnings = emptyList(),
                        recoveryToken = null,
                        userApproved = true,
                    ),
                )
            val pageId = registered.pageId
            val prepared =
                client.prepareOcrInput(
                    PrepareOcrInputRequest(
                        scanId = registered.scanId,
                        inputKind = OcrInputKind.OCR_OPTIMIZED,
                        widthPx = 1080u,
                        heightPx = 1440u,
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
