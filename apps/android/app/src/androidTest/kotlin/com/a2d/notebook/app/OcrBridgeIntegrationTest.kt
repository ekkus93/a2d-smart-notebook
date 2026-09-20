package com.a2d.notebook.app

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.feature.ocr.AndroidOcrProvider
import com.a2d.notebook.feature.ocr.AndroidOcrQueueJobStatus
import com.a2d.notebook.feature.ocr.AndroidOcrQueueProcessor
import com.a2d.notebook.feature.ocr.AndroidOcrQueueStep
import com.a2d.notebook.feature.ocr.AndroidOcrRecognitionOutcome
import com.a2d.notebook.feature.ocr.AndroidRecognizedTextRegion
import com.a2d.notebook.feature.ocr.AndroidOcrWorkflow
import com.a2d.notebook.feature.ocr.FfiAndroidOcrQueueGateway
import com.a2d.notebook.feature.ocr.FfiAndroidOcrReadback
import com.a2d.notebook.feature.ocr.FfiAndroidOcrSearchGateway
import com.a2d.notebook.feature.ocr.FfiRustOcrGateway
import com.a2d.notebook.feature.ocr.AndroidOcrSearchRequest
import com.a2d.notebook.feature.ocr.OcrInputKind as AndroidOcrInputKind
import com.a2d.notebook.feature.ocr.OcrTextPoint
import java.io.File
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.A2dFfiException
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OcrRunStatus
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
class OcrBridgeIntegrationTest {
    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun prepareOcrInputCrossesGeneratedBindingAndPreservesStructuredRustErrors() {
        val root = context.filesDir.resolve("ocr-prepare-ffi-${UUID.randomUUID()}")

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val error =
                assertThrows(A2dFfiException.Failed::class.java) {
                    client.prepareOcrInput(
                        PrepareOcrInputRequest(
                            scanId = client.generatePageId(),
                            inputKind = OcrInputKind.ORIGINAL,
                            widthPx = 0u,
                            heightPx = 1_000u,
                        ),
                    )
                }

            assertEquals("CORE_OCR_IMAGE_DIMENSIONS_INVALID", error.v1.code)
            assertEquals("ocr", error.v1.category.lowercase())
            assertFalse(error.v1.retryable)
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun recordOcrRunCrossesGeneratedBindingAndDoesNotBypassRustScanValidation() {
        val root = context.filesDir.resolve("ocr-record-ffi-${UUID.randomUUID()}")

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val error =
                assertThrows(A2dFfiException.Failed::class.java) {
                    client.recordOcrRun(
                        RecordOcrRunRequest(
                            scanId = client.generatePageId(),
                            inputAssetId = client.generatePageId(),
                            provider = "mlkit",
                            providerVersion = "2026.09",
                            modelName = "latin-v1",
                            status = OcrRunStatus.NO_TEXT_DETECTED,
                            fullText = "",
                            unavailableReason = null,
                            unavailableMessage = null,
                            completedAtMs = 250,
                            warnings = emptyList(),
                        ),
                    )
                }

            assertEquals("CORE_OCR_RECORD_SCAN_MISSING", error.v1.code)
            assertEquals("ocr", error.v1.category.lowercase())
            assertFalse(error.v1.retryable)
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun productionQueueProcessorCommitsProviderResultAtomicallyAndMakesItSearchable() {
        val root = context.filesDir.resolve("ocr-production-composition-${UUID.randomUUID()}")

        try {
            val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))
            val scan = registerRealScan(client, root)
            // A freshly registered scan has an immutable Original asset. OcrOptimized is only
            // valid after the scanner's optimization pipeline has materialized that derivative.
            // This sentinel targets queue -> provider -> transactional finalizer -> search
            // composition, while scanner OcrOptimized identity is covered by its own sentinel.
            val geometry = client.resolveOcrSourceGeometry(scan.scanId, OcrInputKind.ORIGINAL)
            val queueGateway = FfiAndroidOcrQueueGateway(client)
            val queued =
                queueGateway.enqueue(
                    scanId = scan.scanId,
                    inputKind = AndroidOcrInputKind.Original,
                    widthPx = geometry.widthPx,
                    heightPx = geometry.heightPx,
                )
            val provider =
                object : AndroidOcrProvider {
                    override fun recognize(input: com.a2d.notebook.feature.ocr.PreparedAndroidOcrInput) =
                        AndroidOcrRecognitionOutcome.Detected(
                            provider = "r16-deterministic-provider",
                            providerVersion = "1",
                            modelName = "production-composition-sentinel",
                            fullText = "r16 transactional production sentinel",
                            regions =
                                listOf(
                                    AndroidRecognizedTextRegion(
                                        polygon =
                                            listOf(
                                                OcrTextPoint(1f, 1f),
                                                OcrTextPoint(20f, 1f),
                                                OcrTextPoint(20f, 20f),
                                                OcrTextPoint(1f, 20f),
                                            ),
                                        text = "transactional sentinel",
                                        confidence = 0.99f,
                                        createdAtMs = 1_000L,
                                    ),
                                ),
                            completedAtMs = 1_000L,
                            warnings = emptyList(),
                        )
                }
            val processor =
                AndroidOcrQueueProcessor(
                    gateway = queueGateway,
                    workflow = AndroidOcrWorkflow(FfiRustOcrGateway(client), provider),
                )

            val step = processor.processNext()
            assertTrue(step is AndroidOcrQueueStep.Completed)
            val completed = (step as AndroidOcrQueueStep.Completed).job
            assertEquals(queued.jobId, completed.jobId)
            assertEquals(AndroidOcrQueueJobStatus.Recognized, completed.status)
            assertNotNull(completed.lastOcrRunId)

            val loaded = FfiAndroidOcrReadback(client).loadLatestOcrOutput(scan.scanId, regionLimit = 1u)
            val run = requireNotNull(loaded.latestRun)
            assertEquals(completed.lastOcrRunId, run.ocrRunId)
            assertEquals("r16 transactional production sentinel", run.fullText)
            assertEquals(1, run.textRegionCount)
            assertEquals(1, run.textRegions.size)
            assertEquals("transactional sentinel", run.textRegions.single().text)

            val search =
                FfiAndroidOcrSearchGateway(client).searchOcrText(
                    AndroidOcrSearchRequest(query = "transactional", limit = 20u),
                )
            assertTrue(search.hits.isNotEmpty())
            assertTrue(search.hits.all { it.scanId == scan.scanId })
            assertTrue(search.hits.any { it.ocrRunId == run.ocrRunId })
        } finally {
            root.deleteRecursively()
        }
    }

    private fun registerRealScan(client: A2dClient, root: File): RegisteredScan {
        val notebook =
            client.createNotebook(
                CreateNotebookRequest(
                    setupPayload = "A2D:1:S:6DE28E53DBKPXCWWNHPC8T7QJX:0V10W2Y",
                    displayName = "R16 production composition fixture",
                    optionalColor = "blue",
                    optionalIcon = "notebook",
                    optionalUserNotes = "queue-provider-finalizer-search sentinel",
                    makeActive = true,
                ),
            )
        val pagePayload = "A2D:1:B:6DE28E53DBKPXCWWNHPC8T7QJX:1:DEV-PAGE-V1:02V2GRM"
        val resolution = client.resolvePageCode(pagePayload, notebook.notebook.id)
        assertTrue(resolution is PageResolution.Resolved)
        val resolved = resolution as PageResolution.Resolved
        val staging = root.resolve("tmp/scanner-staging/r16-production-composition.png")
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
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val source = assets.open("perspective-mild.png").use { requireNotNull(BitmapFactory.decodeStream(it)) }
        try {
            val padX = source.width / 5
            val padY = source.height / 5
            val framed =
                Bitmap.createBitmap(
                    source.width + (padX * 2),
                    source.height + (padY * 2),
                    Bitmap.Config.ARGB_8888,
                )
            try {
                val canvas = Canvas(framed)
                canvas.drawColor(android.graphics.Color.WHITE)
                canvas.drawBitmap(source, padX.toFloat(), padY.toFloat(), null)
                staging.outputStream().use { output ->
                    check(framed.compress(Bitmap.CompressFormat.PNG, 100, output))
                }
            } finally {
                framed.recycle()
            }
        } finally {
            source.recycle()
        }
    }
}
