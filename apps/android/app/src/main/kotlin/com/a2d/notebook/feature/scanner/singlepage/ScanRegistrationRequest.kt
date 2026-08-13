package com.a2d.notebook.feature.scanner.singlepage

import android.app.Application
import android.graphics.BitmapFactory
import android.media.ExifInterface
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.camera.CameraCaptureResult
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeEvent
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.EncodedPageFormat
import com.a2d.notebook.rustbridge.EncodedPageRotation
import com.a2d.notebook.rustbridge.PolicyPagePreviewCancellation
import com.a2d.notebook.rustbridge.PolicyPagePreviewProcessingOutcome
import com.a2d.notebook.rustbridge.PolicyPagePreviewProcessingRequest
import com.a2d.notebook.rustbridge.StoredScanPolicy
import com.a2d.notebook.rustbridge.processPolicyPagePreview
import com.a2d.notebook.rustbridge.resolveStoredScanPolicy
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.io.File
import java.util.EnumMap
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.BatchScanEntry
import uniffi.a2d_ffi.BatchScanEntryStatus
import uniffi.a2d_ffi.BatchScanReviewReason
import uniffi.a2d_ffi.BatchScanSession
import uniffi.a2d_ffi.BeginBatchScanSessionRequest
import uniffi.a2d_ffi.BeginScannerRecoveryRequest
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution
import uniffi.a2d_ffi.RegisterScanRequest
import uniffi.a2d_ffi.RegistrationImageFormat
import uniffi.a2d_ffi.RegistrationImageRotation
import uniffi.a2d_ffi.RegistrationMarker
import uniffi.a2d_ffi.ScanCaptureSource
import uniffi.a2d_ffi.ScannerRecoveryPhase

internal fun SinglePageReviewArtifact.toRegisterScanRequest(): RegisterScanRequest {
    require(approvalAllowed) { "blocked review artifacts cannot be registered" }
    val payload = requireNotNull(pageCodePayload?.takeIf(String::isNotBlank)) {
        "validated Page Code payload is required for registration"
    }
    return RegisterScanRequest(
        stagingPath = stagingPath,
        pageCodePayload = payload,
        expectedPageId = captureRequest.pageId,
        activeNotebookId = captureRequest.activeNotebookId,
        captureSource = ScanCaptureSource.CAMERA,
        imageFormat = RegistrationImageFormat.JPEG,
        imageRotation = imageRotation.toRegistrationImageRotation(),
        capturedAtMs = capturedAtMs,
        observedMarkers = analysis.markers.toRegistrationMarkers(),
        previewWarnings = warnings.toRegistrationWarningCodes() + RustScannerPolicySession.registrationEvidence(),
        recoveryToken = recoveryToken,
        userApproved = true,
    )
}

internal fun EncodedPageRotation.toRegistrationImageRotation(): RegistrationImageRotation =
    when (this) {
        EncodedPageRotation.DEGREES_0 -> RegistrationImageRotation.DEGREES0
        EncodedPageRotation.DEGREES_90 -> RegistrationImageRotation.DEGREES90
        EncodedPageRotation.DEGREES_180 -> RegistrationImageRotation.DEGREES180
        EncodedPageRotation.DEGREES_270 -> RegistrationImageRotation.DEGREES270
    }

internal fun List<AnalyzedPageMarker>.toRegistrationMarkers(): List<RegistrationMarker> {
    val markers =
        map { marker ->
            require(marker.id in 0L..UInt.MAX_VALUE.toLong()) {
                "marker ID must fit an unsigned 32-bit integer"
            }
            RegistrationMarker(role = marker.role, id = marker.id.toUInt())
        }
    val roles = markers.map { it.role.trim().uppercase() }
    require(roles.size == 4 && roles.toSet() == setOf("TL", "TR", "BR", "BL")) {
        "registration requires exactly one TL, TR, BR, and BL marker"
    }
    return markers
}

internal fun Set<CapturePolicyWarning>.toRegistrationWarningCodes(): List<String> {
    require(CapturePolicyWarning.MISSING_MARKERS !in this) {
        "a capture with missing markers cannot be approved for registration"
    }
    return map { it.name }.sorted()
}

private val BATCH_CAPTURE_RESERVATION_BYTES =
    "A2D_BATCH_CAMERA_CAPTURE_RESERVED_V1\n".encodeToByteArray()

internal data class PreparedBatchCapture(
    val recoveryToken: String,
    val stagingPath: String,
)

internal data class BatchScannerUiState(
    val loading: Boolean = true,
    val session: BatchScanSession? = null,
    val lockedNotebook: NotebookSummary? = null,
    val cameraState: CameraAdapterState = CameraAdapterState.Idle,
    val cameraGeneration: Long = 0,
    val resolvedPageId: String? = null,
    val pageCodePayload: String? = null,
    val storedPolicy: StoredScanPolicy? = null,
    val pageCodeMessage: String = "Keep the Page Code visible",
    val captureInProgress: Boolean = false,
    val processing: Boolean = false,
    val pendingCapture: PreparedBatchCapture? = null,
    val notice: String? = null,
    val completedSummary: BatchScanSession? = null,
    val error: String? = null,
) {
    val canCapture: Boolean
        get() = !loading && session?.completedAtMs == null && lockedNotebook != null &&
            cameraState is CameraAdapterState.Bound && resolvedPageId != null &&
            pageCodePayload != null && storedPolicy != null && !captureInProgress && pendingCapture == null

    val canFinish: Boolean
        get() = !captureInProgress && pendingCapture == null && !processing &&
            (session?.queuedCount ?: 0u) == 0u && session != null
}

internal class BatchScannerViewModel(application: Application) : AndroidViewModel(application) {
    private val client = A2dBridge.client(application)
    private val mutableState = mutableStateOf(BatchScannerUiState())
    val state: State<BatchScannerUiState> = mutableState
    private var worker: Job? = null
    private var qrSequence = 0L

    init {
        viewModelScope.launch { startOrResume() }
    }

    fun onCameraStateChanged(value: CameraAdapterState) {
        update { state ->
            state.copy(
                cameraState = value,
                error = if (value is CameraAdapterState.Error) value.message else state.error,
            )
        }
    }

    fun onQrCodeEvent(event: LiveQrCodeEvent) {
        when (event) {
            is LiveQrCodeEvent.Found -> resolveLivePage(event.payload)
            is LiveQrCodeEvent.NotFound -> clearLivePage("Keep the Page Code visible")
            is LiveQrCodeEvent.Failed -> clearLivePage(event.message)
            is LiveQrCodeEvent.SubmissionRejected -> update { it.copy(pageCodeMessage = event.message) }
            is LiveQrCodeEvent.Dropped,
            is LiveQrCodeEvent.StaleResultDiscarded,
            LiveQrCodeEvent.Closed,
            -> Unit
        }
    }

    fun requestCapture() {
        val state = mutableState.value
        val session = state.session ?: return
        val notebook = state.lockedNotebook ?: return
        val pageId = state.resolvedPageId ?: return
        val policy = state.storedPolicy ?: return
        if (!state.canCapture) return
        update { it.copy(captureInProgress = true, error = null, notice = null) }
        viewModelScope.launch {
            try {
                val prepared = withContext(Dispatchers.IO) {
                    val file = newStagingFile()
                    writeReservation(file)
                    val token = UUID.randomUUID().toString()
                    try {
                        client.beginScannerRecovery(
                            BeginScannerRecoveryRequest(
                                token = token,
                                stagingPath = file.canonicalPath,
                                pageId = pageId,
                                notebookId = notebook.id,
                                capturedAtMs = System.currentTimeMillis(),
                                layoutId = policy.layoutId,
                                processingPolicyVersion = policy.processingPolicyVersion.toUInt(),
                            ),
                        )
                        val queued = client.queueBatchScanCapture(session.sessionId, token)
                        require(queued.notebookId == notebook.id) { "batch Notebook changed" }
                        require(deleteReservation(file)) { "could not release CameraX staging reservation" }
                        update { it.copy(session = queued) }
                        PreparedBatchCapture(token, file.canonicalPath)
                    } catch (failure: Exception) {
                        runCatching { client.discardScannerRecovery(token) }
                        file.delete()
                        throw failure
                    }
                }
                update { it.copy(pendingCapture = prepared) }
            } catch (failure: Exception) {
                update { it.copy(captureInProgress = false, error = failure.message ?: "capture setup failed") }
            }
        }
    }

    fun onCameraCaptureResult(prepared: PreparedBatchCapture, result: CameraCaptureResult) {
        if (mutableState.value.pendingCapture?.recoveryToken != prepared.recoveryToken) return
        when (result) {
            is CameraCaptureResult.Captured -> {
                if (result.file.canonicalPath != prepared.stagingPath || !isFinalized(result.file)) {
                    failCapture(prepared, "CameraX did not finalize the expected staged image")
                    return
                }
                update {
                    it.copy(
                        captureInProgress = false,
                        pendingCapture = null,
                        resolvedPageId = null,
                        pageCodePayload = null,
                        storedPolicy = null,
                        notice = "Captured and queued",
                        cameraGeneration = if (it.cameraGeneration == Long.MAX_VALUE) 0 else it.cameraGeneration + 1,
                    )
                }
                startWorker()
            }
            is CameraCaptureResult.Failure -> failCapture(prepared, result.message)
        }
    }

    fun finishBatch() {
        val state = mutableState.value
        val session = state.session ?: return
        if (!state.canFinish) {
            update { it.copy(error = "Queued scans must finish before the batch can be completed") }
            return
        }
        viewModelScope.launch {
            try {
                val completed = withContext(Dispatchers.IO) { client.completeBatchScanSession(session.sessionId) }
                update { it.copy(session = completed, completedSummary = completed, notice = null) }
            } catch (failure: Exception) {
                update { it.copy(error = failure.message ?: "failed to finish batch") }
            }
        }
    }

    fun acknowledgeCompleted(onDone: () -> Unit) {
        val summary = mutableState.value.completedSummary ?: return
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) { client.acknowledgeBatchScanSession(summary.sessionId) }
                update { it.copy(completedSummary = null) }
                onDone()
            } catch (failure: Exception) {
                update { it.copy(error = failure.message ?: "failed to acknowledge batch") }
            }
        }
    }

    private suspend fun startOrResume() {
        try {
            val (session, notebook) = withContext(Dispatchers.IO) {
                val active = client.listBatchScanSessions(false)
                require(active.size <= 1) { "more than one active batch session exists" }
                val session = active.singleOrNull()?.let { client.reconcileBatchScanSession(it.sessionId) }
                    ?: run {
                        val notebook = client.getActiveNotebook()
                            ?: error("Select an active Notebook before starting Batch Scan")
                        client.beginBatchScanSession(
                            BeginBatchScanSessionRequest(
                                sessionId = "batch-${UUID.randomUUID()}",
                                notebookId = notebook.id,
                            ),
                        )
                    }
                val notebook = client.getNotebook(session.notebookId)
                    ?: error("The batch Notebook no longer exists")
                require(!notebook.archived) { "The batch Notebook is archived" }
                session to notebook
            }
            update { it.copy(loading = false, session = session, lockedNotebook = notebook, error = null) }
            startWorker()
        } catch (failure: Exception) {
            update { it.copy(loading = false, error = failure.message ?: "failed to start batch") }
        }
    }

    private fun resolveLivePage(payload: String) {
        val session = mutableState.value.session ?: return
        val sequence = ++qrSequence
        viewModelScope.launch {
            try {
                val pair = withContext(Dispatchers.IO) {
                    val resolution = client.resolvePageCode(payload, session.notebookId)
                    val page = resolution as? PageResolution.Resolved
                        ?: error("Page Code is not a registered Notebook page")
                    require(page.notebookId == session.notebookId) { "Page belongs to a different Notebook" }
                    page.pageId to client.resolveStoredScanPolicy(page.pageId)
                }
                if (sequence == qrSequence) {
                    update {
                        it.copy(
                            resolvedPageId = pair.first,
                            pageCodePayload = payload,
                            storedPolicy = pair.second,
                            pageCodeMessage = "Page identity verified",
                        )
                    }
                }
            } catch (failure: Exception) {
                if (sequence == qrSequence) clearLivePage(failure.message ?: "Page Code resolution failed")
            }
        }
    }

    private fun clearLivePage(message: String) {
        update {
            it.copy(
                resolvedPageId = null,
                pageCodePayload = null,
                storedPolicy = null,
                pageCodeMessage = message,
            )
        }
    }

    private fun failCapture(prepared: PreparedBatchCapture, message: String) {
        update { it.copy(captureInProgress = false, pendingCapture = null, error = message) }
        viewModelScope.launch {
            val session = mutableState.value.session ?: return@launch
            try {
                val changed = withContext(Dispatchers.IO) {
                    client.reportBatchScanReview(
                        session.sessionId,
                        prepared.recoveryToken,
                        BatchScanReviewReason.PROCESSING_FAILURE,
                        message,
                    ).also { runCatching { client.discardScannerRecovery(prepared.recoveryToken) } }
                }
                update { it.copy(session = changed) }
            } catch (failure: Exception) {
                update { it.copy(error = "$message; ${failure.message ?: "review handoff failed"}") }
            }
        }
    }

    private fun startWorker() {
        if (worker?.isActive == true) return
        worker = viewModelScope.launch {
            update { it.copy(processing = true) }
            try {
                while (true) {
                    val current = mutableState.value.session ?: break
                    val session = withContext(Dispatchers.IO) { client.reconcileBatchScanSession(current.sessionId) }
                    update { it.copy(session = session) }
                    val entry = session.entries.firstOrNull { it.status == BatchScanEntryStatus.QUEUED } ?: break
                    processEntry(session, entry)
                }
            } finally {
                update { it.copy(processing = false) }
            }
        }
    }

    private suspend fun processEntry(session: BatchScanSession, entry: BatchScanEntry) {
        try {
            withContext(Dispatchers.IO) {
                val recovery = client.listScannerRecoveries().singleOrNull { it.token == entry.recoveryToken }
                    ?: error("Queued capture has no scanner recovery record")
                if (recovery.phase == ScannerRecoveryPhase.REGISTERING) {
                    client.reconcileScannerRecovery(recovery.token)
                    return@withContext
                }
                require(recovery.phase == ScannerRecoveryPhase.CAPTURED || recovery.phase == ScannerRecoveryPhase.PREVIEW_READY)
                val file = File(recovery.stagingPath)
                require(isFinalized(file)) { "Queued capture image is unavailable or incomplete" }
                val policy = client.resolveStoredScanPolicy(entry.pageId)
                require(policy.layoutId == recovery.layoutId) { "Stored page layout changed after capture" }
                require(policy.processingPolicyVersion.toUInt() == recovery.processingPolicyVersion) {
                    "Stored processing policy changed after capture"
                }
                val rotation = imageRotation(file)
                val bytes = readBounded(file, policy)
                val preview = PolicyPagePreviewCancellation().use { cancellation ->
                    when (
                        val outcome = client.processPolicyPagePreview(
                            PolicyPagePreviewProcessingRequest(bytes, EncodedPageFormat.JPEG, rotation.first, policy),
                            cancellation,
                        )
                    ) {
                        is PolicyPagePreviewProcessingOutcome.Completed -> outcome.result
                        PolicyPagePreviewProcessingOutcome.Cancelled -> error("Batch preview processing was cancelled")
                    }
                }
                val payload = decodeQr(file, policy)
                val resolution = client.resolvePageCode(payload, session.notebookId)
                val resolved = resolution as? PageResolution.Resolved
                    ?: error("Final captured Page Code no longer resolves")
                require(resolved.pageId == entry.pageId && resolved.notebookId == session.notebookId) {
                    "Final captured Page Code conflicts with queued identity"
                }
                if (recovery.phase == ScannerRecoveryPhase.CAPTURED) {
                    client.markScannerRecoveryPreviewReady(recovery.token)
                }
                client.registerBatchScan(
                    session.sessionId,
                    RegisterScanRequest(
                        stagingPath = recovery.stagingPath,
                        pageCodePayload = payload,
                        expectedPageId = entry.pageId,
                        activeNotebookId = session.notebookId,
                        captureSource = ScanCaptureSource.CAMERA,
                        imageFormat = RegistrationImageFormat.JPEG,
                        imageRotation = rotation.second,
                        capturedAtMs = recovery.capturedAtMs,
                        observedMarkers = preview.analysis.markers.toRegistrationMarkers(),
                        previewWarnings = listOf(
                            "A2D_POLICY_LAYOUT=${policy.layoutId}",
                            "A2D_POLICY_VERSION=${policy.processingPolicyVersion}",
                            "A2D_PIPELINE_VERSION=${policy.pipelineVersion}",
                        ),
                        recoveryToken = recovery.token,
                        userApproved = false,
                    ),
                )
            }
            val changed = withContext(Dispatchers.IO) { client.reconcileBatchScanSession(session.sessionId) }
            update { it.copy(session = changed, notice = "Saved", error = null) }
        } catch (failure: CancellationException) {
            throw failure
        } catch (failure: Exception) {
            val changed = withContext(Dispatchers.IO) {
                client.reportBatchScanReview(
                    session.sessionId,
                    entry.recoveryToken,
                    BatchScanReviewReason.PROCESSING_FAILURE,
                    failure.message ?: "Batch processing failed",
                )
            }
            update { it.copy(session = changed, notice = "Capture moved to Needs Review", error = failure.message) }
        }
    }

    private fun newStagingFile(): File {
        val directory = A2dBridge.libraryDirectory(getApplication()).resolve("tmp").resolve("scanner-staging")
        check(directory.exists() || directory.mkdirs()) { "Could not create scanner staging directory" }
        return directory.resolve("batch-${UUID.randomUUID()}.jpg")
    }

    private fun writeReservation(file: File) {
        check(!file.exists())
        file.outputStream().use { stream ->
            stream.write(BATCH_CAPTURE_RESERVATION_BYTES)
            stream.flush()
            stream.fd.sync()
        }
    }

    private fun deleteReservation(file: File): Boolean =
        file.isFile && file.readBytes().contentEquals(BATCH_CAPTURE_RESERVATION_BYTES) && file.delete()

    private fun isFinalized(file: File): Boolean =
        file.isFile && file.length() > BATCH_CAPTURE_RESERVATION_BYTES.size

    private fun readBounded(file: File, policy: StoredScanPolicy): ByteArray {
        require(file.length() in 1..policy.maximumEncodedBytes)
        return file.readBytes().also { require(it.size.toLong() <= policy.maximumEncodedBytes) }
    }

    private fun decodeQr(file: File, policy: StoredScanPolicy): String {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.absolutePath, bounds)
        require(bounds.outWidth > 0 && bounds.outHeight > 0)
        require(Math.multiplyExact(bounds.outWidth.toLong(), bounds.outHeight.toLong()) <= policy.maximumDecodedPixels)
        val bitmap = requireNotNull(BitmapFactory.decodeFile(file.absolutePath))
        try {
            val pixels = IntArray(Math.multiplyExact(bitmap.width, bitmap.height))
            bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
            val source = RGBLuminanceSource(bitmap.width, bitmap.height, pixels)
            val hints = EnumMap<DecodeHintType, Any>(DecodeHintType::class.java)
            hints[DecodeHintType.POSSIBLE_FORMATS] = listOf(BarcodeFormat.QR_CODE)
            return MultiFormatReader().decode(BinaryBitmap(HybridBinarizer(source)), hints).text
                .takeIf(String::isNotBlank) ?: error("Final captured Page Code is empty")
        } finally {
            bitmap.recycle()
        }
    }

    private fun imageRotation(file: File): Pair<EncodedPageRotation, RegistrationImageRotation> {
        val value = ExifInterface(file.absolutePath).getAttributeInt(
            ExifInterface.TAG_ORIENTATION,
            ExifInterface.ORIENTATION_NORMAL,
        )
        return when (value) {
            ExifInterface.ORIENTATION_ROTATE_90 -> EncodedPageRotation.DEGREES_90 to RegistrationImageRotation.DEGREES90
            ExifInterface.ORIENTATION_ROTATE_180 -> EncodedPageRotation.DEGREES_180 to RegistrationImageRotation.DEGREES180
            ExifInterface.ORIENTATION_ROTATE_270 -> EncodedPageRotation.DEGREES_270 to RegistrationImageRotation.DEGREES270
            else -> EncodedPageRotation.DEGREES_0 to RegistrationImageRotation.DEGREES0
        }
    }

    private fun update(transform: (BatchScannerUiState) -> BatchScannerUiState) {
        mutableState.value = transform(mutableState.value)
    }
}
