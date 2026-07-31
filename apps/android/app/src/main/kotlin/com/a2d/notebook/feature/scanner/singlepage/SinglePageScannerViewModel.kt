package com.a2d.notebook.feature.scanner.singlepage

import android.app.Application
import android.media.ExifInterface
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.camera.CameraCaptureResult
import com.a2d.notebook.feature.scanner.camera.LiveFrameAnalysisEvent
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeEvent
import com.a2d.notebook.feature.scanner.capture.AutoCaptureContext
import com.a2d.notebook.feature.scanner.capture.AutoCaptureEffect
import com.a2d.notebook.feature.scanner.capture.AutoCaptureFailure
import com.a2d.notebook.feature.scanner.capture.AutoCaptureFrameAssessment
import com.a2d.notebook.feature.scanner.capture.AutoCapturePhase
import com.a2d.notebook.feature.scanner.capture.AutoCaptureProcessingOutcome
import com.a2d.notebook.feature.scanner.capture.AutoCaptureRequest
import com.a2d.notebook.feature.scanner.capture.AutoCaptureStateMachine
import com.a2d.notebook.feature.scanner.capture.CaptureRequestToken
import com.a2d.notebook.feature.scanner.capture.CaptureTrigger
import com.a2d.notebook.feature.scanner.capture.CapturedImage
import com.a2d.notebook.feature.scanner.capture.ManualCaptureDeniedReason
import com.a2d.notebook.feature.scanner.presentation.buildLiveScannerPresentation
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.EncodedPageFormat
import com.a2d.notebook.rustbridge.EncodedPageRotation
import com.a2d.notebook.rustbridge.PagePreviewCancellation
import com.a2d.notebook.rustbridge.PagePreviewProcessingOutcome
import com.a2d.notebook.rustbridge.PagePreviewProcessingRequest
import com.a2d.notebook.rustbridge.ProcessedRgbImage
import com.a2d.notebook.rustbridge.decodeQrPixels
import com.a2d.notebook.rustbridge.processPagePreview
import com.a2d.notebook.rustbridge.resolveStoredScanPolicy
import com.google.zxing.NotFoundException
import java.io.File
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.BeginScannerRecoveryRequest
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution
import uniffi.a2d_ffi.ScannerRecoveryPhase
import uniffi.a2d_ffi.ScannerRecoveryRecord

internal data class FinalCaptureIdentityAssessment(
    val approvalAllowed: Boolean,
    val warning: String?,
)

internal fun assessFinalCaptureIdentity(
    resolution: PageResolution?,
    request: AutoCaptureRequest,
    decoderWarning: String? = null,
): FinalCaptureIdentityAssessment {
    if (decoderWarning != null) {
        return FinalCaptureIdentityAssessment(false, decoderWarning)
    }
    val resolved = resolution as? PageResolution.Resolved
        ?: return FinalCaptureIdentityAssessment(
            false,
            "The final Page Code did not resolve to a registered Notebook page.",
        )
    if (resolved.pageId != request.pageId) {
        return FinalCaptureIdentityAssessment(
            false,
            "The final Page Code identifies a different page than the live capture request.",
        )
    }
    if (resolved.notebookId != request.activeNotebookId) {
        return FinalCaptureIdentityAssessment(
            false,
            "The final Page Code does not belong to the selected Notebook.",
        )
    }
    return FinalCaptureIdentityAssessment(true, null)
}

class SinglePageScannerViewModel(application: Application) : AndroidViewModel(application) {
    private val policy = SinglePageScannerPolicies.V1
    private val client = A2dBridge.client(application)
    private val captureMachine = AutoCaptureStateMachine(policy.autoCapture)
    private val mutableState = mutableStateOf(SinglePageScannerUiState())
    val state: State<SinglePageScannerUiState> = mutableState

    private var latestAnalysisTimestampNanos: Long? = null
    private var latestAssessment: AutoCaptureFrameAssessment? = null
    private var analysisFailure: String? = null
    private var qrResolutionSequence = 0L
    private var recoverySequence = 0L
    private var qrResolutionJob: Job? = null
    private var processingJob: Job? = null
    private var registrationJob: Job? = null
    private var processingCancellation: PagePreviewCancellation? = null
    private var pendingStagingFile: File? = null
    private var pendingCapturedAtMs: Long? = null
    private var pendingRecoveryToken: String? = null
    private var recoveringRecord: ScannerRecoveryRecord? = null

    init {
        refreshNotebooks()
        refreshScannerRecoveries()
    }

    fun selectNotebook(notebook: NotebookSummary) {
        val current = mutableState.value
        if (
            current.processing ||
                current.registrationInProgress ||
                current.recoveryOperationInProgress ||
                current.scannerRecoveries.isNotEmpty() ||
                current.activeNotebook?.id == notebook.id
        ) {
            return
        }
        update { it.copy(loadingNotebooks = true, error = null) }
        viewModelScope.launch {
            try {
                val notebooks =
                    withContext(Dispatchers.IO) {
                        client.setActiveNotebook(notebook.id)
                        client.listNotebooks(false)
                    }
                val active = requireSingleActiveNotebook(notebooks)
                resetScannerSession(active, notebooks)
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        loadingNotebooks = false,
                        error = failure.message ?: "Failed to select the scan destination",
                    )
                }
            }
        }
    }

    fun onCameraStateChanged(cameraState: CameraAdapterState) {
        viewModelScope.launch {
            update {
                it.copy(
                    cameraState = cameraState,
                    error =
                        if (cameraState is CameraAdapterState.Error) {
                            cameraState.message
                        } else {
                            it.error
                        },
                )
            }
        }
    }

    fun onLiveAnalysisEvent(event: LiveFrameAnalysisEvent) {
        viewModelScope.launch {
            when (event) {
                is LiveFrameAnalysisEvent.Succeeded -> {
                    if (mutableState.value.reviewArtifact != null || mutableState.value.processing) {
                        return@launch
                    }
                    latestAnalysisTimestampNanos = event.metrics.frameTimestampNanos
                    analysisFailure = null
                    update { it.copy(latestAnalysis = event.result, error = null) }
                    rebuildPresentation(event.metrics.frameTimestampNanos)
                    recomputeAssessment(event.metrics.frameTimestampNanos, observeForAutoCapture = true)
                }

                is LiveFrameAnalysisEvent.Failed -> {
                    latestAnalysisTimestampNanos = event.metrics.frameTimestampNanos
                    latestAssessment = null
                    analysisFailure = event.message
                    update { it.copy(latestAnalysis = null) }
                    rebuildPresentation(event.metrics.frameTimestampNanos)
                }

                is LiveFrameAnalysisEvent.CameraFailure ->
                    update { it.copy(error = event.message) }

                is LiveFrameAnalysisEvent.InfrastructureFailure ->
                    update { it.copy(error = event.message) }

                is LiveFrameAnalysisEvent.SubmissionRejected ->
                    update { it.copy(error = event.message) }

                is LiveFrameAnalysisEvent.Dropped,
                is LiveFrameAnalysisEvent.StaleResultDiscarded,
                LiveFrameAnalysisEvent.Closed,
                -> Unit
            }
        }
    }

    fun onQrCodeEvent(event: LiveQrCodeEvent) {
        viewModelScope.launch {
            when (event) {
                is LiveQrCodeEvent.Found -> resolveLivePageCode(event)
                is LiveQrCodeEvent.NotFound -> handlePageCodeNotFound(event.frameTimestampNanos)
                is LiveQrCodeEvent.Failed -> {
                    qrResolutionJob?.cancel()
                    update {
                        it.copy(
                            latestPageResolution = null,
                            pageCodeStatus = PageCodeUiStatus.Failed(event.message),
                        )
                    }
                    rebuildPresentation(event.frameTimestampNanos)
                    recomputeAssessment(event.frameTimestampNanos, observeForAutoCapture = false)
                }

                is LiveQrCodeEvent.SubmissionRejected ->
                    update {
                        it.copy(pageCodeStatus = PageCodeUiStatus.Failed(event.message))
                    }

                is LiveQrCodeEvent.Dropped,
                is LiveQrCodeEvent.StaleResultDiscarded,
                LiveQrCodeEvent.Closed,
                -> Unit
            }
        }
    }

    fun requestManualCapture() {
        val current = mutableState.value
        if (!current.canCaptureManually) {
            update { it.copy(error = "The camera and scan destination must be ready before capture") }
            return
        }
        val now = System.nanoTime()
        recomputeAssessment(now, observeForAutoCapture = false)
        val assessment = latestAssessment
        if (assessment == null) {
            update { it.copy(error = "Page identity has not been verified for this capture") }
            return
        }
        applyTransition(captureMachine.observeFrame(assessment))
        applyTransition(captureMachine.requestManualCapture(now))
    }

    fun confirmManualCapture() {
        val warning = mutableState.value.pendingManualWarning ?: return
        applyTransition(captureMachine.confirmManualCapture(warning.token, System.nanoTime()))
    }

    fun dismissManualCapture() {
        val warning = mutableState.value.pendingManualWarning ?: return
        applyTransition(captureMachine.dismissManualCapture(warning.token))
    }

    /** Returns one new path and atomically consumes the matching UI request. */
    fun consumePendingCapture(request: AutoCaptureRequest): File? {
        if (mutableState.value.pendingCaptureRequest != request) return null
        update { it.copy(pendingCaptureRequest = null) }
        val directory = stagingDirectory()
        val file =
            File(
                directory,
                "scan-${request.token.generation}-${request.token.sequence}-${UUID.randomUUID()}.jpg",
            )
        check(!file.exists()) { "new scanner staging path unexpectedly already exists" }
        pendingStagingFile = file
        pendingRecoveryToken = null
        return file
    }

    fun onCameraCaptureResult(
        request: AutoCaptureRequest,
        result: CameraCaptureResult,
    ) {
        when (result) {
            is CameraCaptureResult.Captured -> {
                val file = result.file
                if (pendingStagingFile?.absolutePath != file.absolutePath) {
                    safeDelete(file)
                    update { it.copy(error = "A stale camera capture was discarded") }
                    return
                }
                val capturedAtMs = System.currentTimeMillis()
                pendingCapturedAtMs = capturedAtMs
                applyTransition(
                    captureMachine.captureSucceeded(
                        request.token,
                        CapturedImage(
                            stagingPath = file.absolutePath,
                            savedUri = result.savedUri?.toString(),
                        ),
                    ),
                )
                update { it.copy(processing = true, error = null) }
                journalCaptureAndStartProcessing(request, file, capturedAtMs)
            }

            is CameraCaptureResult.Failure -> {
                pendingStagingFile?.let(::safeDelete)
                pendingStagingFile = null
                pendingCapturedAtMs = null
                pendingRecoveryToken = null
                applyTransition(
                    captureMachine.captureFailed(
                        request.token,
                        AutoCaptureFailure(result.message, retryable = true),
                    ),
                )
                update { it.copy(processing = false, error = result.message) }
            }
        }
    }

    fun cancelProcessing() {
        processingCancellation?.cancel()
    }

    fun reviewRecovery(record: ScannerRecoveryRecord) {
        if (!isCurrentRecovery(record) || mutableState.value.recoveryOperationInProgress) return
        update { it.copy(recoveryOperationInProgress = true, error = null) }
        viewModelScope.launch {
            try {
                val prepared =
                    withContext(Dispatchers.IO) {
                        val reconciled =
                            if (record.phase == ScannerRecoveryPhase.REGISTERING) {
                                client.reconcileScannerRecovery(record.token)
                            } else {
                                record
                            }
                        if (reconciled.phase == ScannerRecoveryPhase.COMMITTED) {
                            return@withContext RecoveryPreparation.Committed(reconciled)
                        }
                        require(
                            reconciled.phase == ScannerRecoveryPhase.CAPTURED ||
                                reconciled.phase == ScannerRecoveryPhase.PREVIEW_READY,
                        ) { "only captured or preview-ready recovery can be reviewed" }
                        client.setActiveNotebook(reconciled.notebookId)
                        val notebooks = client.listNotebooks(false)
                        val active = requireSingleActiveNotebook(notebooks)
                        require(active?.id == reconciled.notebookId) {
                            "the recovered Notebook could not be selected"
                        }
                        val storedPolicy = client.resolveStoredScanPolicy(reconciled.pageId)
                        require(storedPolicy.layoutId == reconciled.layoutId) {
                            "the stored page layout changed since capture"
                        }
                        require(
                            storedPolicy.processingPolicyVersion.toUInt() ==
                                reconciled.processingPolicyVersion,
                        ) { "the scan processing policy changed since capture" }
                        RecoveryPreparation.Ready(reconciled, notebooks, active, storedPolicy)
                    }
                when (prepared) {
                    is RecoveryPreparation.Committed -> {
                        replaceRecovery(prepared.record)
                        update { it.copy(recoveryOperationInProgress = false) }
                    }

                    is RecoveryPreparation.Ready -> beginRecoveredReview(prepared)
                }
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        recoveryOperationInProgress = false,
                        recoveryMode = false,
                        error = failure.message ?: "Failed to prepare scanner recovery",
                    )
                }
                refreshScannerRecoveries()
            }
        }
    }

    fun reconcileRecovery(record: ScannerRecoveryRecord) {
        if (!isCurrentRecovery(record) || mutableState.value.recoveryOperationInProgress) return
        update { it.copy(recoveryOperationInProgress = true, error = null) }
        viewModelScope.launch {
            try {
                val reconciled =
                    withContext(Dispatchers.IO) {
                        client.reconcileScannerRecovery(record.token)
                    }
                replaceRecovery(reconciled)
                update { it.copy(recoveryOperationInProgress = false) }
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        recoveryOperationInProgress = false,
                        error = failure.message ?: "Failed to reconcile scanner recovery",
                    )
                }
            }
        }
    }

    fun acknowledgeRecovery(record: ScannerRecoveryRecord) {
        if (!isCurrentRecovery(record) || mutableState.value.recoveryOperationInProgress) return
        update { it.copy(recoveryOperationInProgress = true, error = null) }
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    val reconciled = client.reconcileScannerRecovery(record.token)
                    require(reconciled.phase == ScannerRecoveryPhase.COMMITTED) {
                        "Rust did not confirm a committed scan"
                    }
                    val scanId = requireNotNull(reconciled.registeredScanId) {
                        "committed recovery has no scan ID"
                    }
                    client.acknowledgeCommittedScannerRecovery(reconciled.token, scanId)
                }
                removeRecovery(record.token)
                update { it.copy(recoveryOperationInProgress = false) }
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        recoveryOperationInProgress = false,
                        error = failure.message ?: "Failed to acknowledge the saved scan",
                    )
                }
                refreshScannerRecoveries()
            }
        }
    }

    fun discardRecovery(record: ScannerRecoveryRecord) {
        if (
            !isCurrentRecovery(record) ||
                !record.canDiscard() ||
                mutableState.value.recoveryOperationInProgress
        ) {
            return
        }
        update { it.copy(recoveryOperationInProgress = true, error = null) }
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    client.discardScannerRecovery(record.token)
                }
                removeRecovery(record.token)
                if (pendingRecoveryToken == record.token) clearPendingCaptureReferences()
                update { it.copy(recoveryOperationInProgress = false) }
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        recoveryOperationInProgress = false,
                        error = failure.message ?: "Failed to discard scanner recovery",
                    )
                }
            }
        }
    }

    fun approveReview() {
        val current = mutableState.value
        val artifact = current.reviewArtifact
        if (!current.canApprove || artifact == null) {
            update { it.copy(error = "Final page identity must match before registration") }
            return
        }
        check(registrationJob == null) { "scan registration is already active" }
        update { it.copy(registrationInProgress = true, error = null) }
        registrationJob =
            viewModelScope.launch {
                try {
                    val registered =
                        withContext(Dispatchers.IO) {
                            client.registerScan(artifact.toRegisterScanRequest())
                        }
                    val wasRecoveryMode = mutableState.value.recoveryMode
                    clearPendingCaptureReferences()
                    if (wasRecoveryMode) {
                        update {
                            it.copy(
                                capturePhase =
                                    AutoCapturePhase.Accepted(
                                        artifact.captureRequest,
                                        registered.scanId,
                                    ),
                            )
                        }
                    } else {
                        applyTransition(
                            captureMachine.reviewRegistrationCompleted(
                                artifact.captureRequest.token,
                                registered.scanId,
                            ),
                        )
                    }

                    var acknowledgementFailure: String? = null
                    try {
                        withContext(Dispatchers.IO) {
                            val reconciled =
                                client.reconcileScannerRecovery(artifact.recoveryToken)
                            require(
                                reconciled.phase == ScannerRecoveryPhase.COMMITTED &&
                                    reconciled.registeredScanId == registered.scanId,
                            ) { "recovery record did not reconcile to the registered scan" }
                            client.acknowledgeCommittedScannerRecovery(
                                artifact.recoveryToken,
                                registered.scanId,
                            )
                        }
                        removeRecovery(artifact.recoveryToken)
                    } catch (failure: Exception) {
                        acknowledgementFailure =
                            failure.message
                                ?: "Scan was saved, but recovery acknowledgement is pending"
                        refreshScannerRecoveries()
                    }

                    update {
                        it.copy(
                            registrationInProgress = false,
                            registeredScan = registered,
                            capturePhase =
                                if (wasRecoveryMode) {
                                    AutoCapturePhase.Accepted(
                                        artifact.captureRequest,
                                        registered.scanId,
                                    )
                                } else {
                                    captureMachine.snapshot().phase
                                },
                            error = acknowledgementFailure,
                        )
                    }
                } catch (failure: CancellationException) {
                    throw failure
                } catch (failure: Exception) {
                    update {
                        it.copy(
                            registrationInProgress = false,
                            error = failure.message ?: "Rust scan registration failed",
                        )
                    }
                    refreshScannerRecoveries()
                } finally {
                    registrationJob = null
                }
            }
    }

    fun retake() {
        val current = mutableState.value
        if (current.registrationInProgress || current.recoveryOperationInProgress) return
        val artifact = current.reviewArtifact ?: return
        if (current.registeredScan != null) {
            resetAfterReview()
            return
        }
        val record = current.scannerRecoveries.firstOrNull { it.token == artifact.recoveryToken }
        if (record == null || !record.canDiscard()) {
            update { it.copy(error = "The recovery journal must be reconciled before retaking") }
            refreshScannerRecoveries()
            return
        }
        update { it.copy(recoveryOperationInProgress = true, error = null) }
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    client.discardScannerRecovery(record.token)
                }
                removeRecovery(record.token)
                clearPendingCaptureReferences()
                resetAfterReview()
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        recoveryOperationInProgress = false,
                        error = failure.message ?: "Failed to discard the captured scan",
                    )
                }
            }
        }
    }

    fun toggleDetails() {
        update { it.copy(detailsVisible = !it.detailsVisible) }
    }

    fun leaveScanner() {
        if (mutableState.value.registrationInProgress) {
            update { it.copy(error = "Wait for durable registration to finish before leaving") }
            return
        }
        cancelActiveWork(clearReview = true)
        applyTransition(captureMachine.stop())
        update {
            it.copy(
                recoveryMode = false,
                cameraGeneration = nextGeneration(it.cameraGeneration),
            )
        }
    }

    override fun onCleared() {
        cancelActiveWork(clearReview = true)
        captureMachine.stop()
        super.onCleared()
    }

    private fun journalCaptureAndStartProcessing(
        request: AutoCaptureRequest,
        file: File,
        capturedAtMs: Long,
    ) {
        viewModelScope.launch {
            try {
                val storedPolicy = RustScannerPolicySession.requireCurrentPolicy()
                val token = UUID.randomUUID().toString()
                val record =
                    withContext(Dispatchers.IO) {
                        client.beginScannerRecovery(
                            BeginScannerRecoveryRequest(
                                token = token,
                                stagingPath = file.absolutePath,
                                pageId = request.pageId,
                                notebookId = request.activeNotebookId,
                                capturedAtMs = capturedAtMs,
                                layoutId = storedPolicy.layoutId,
                                processingPolicyVersion =
                                    storedPolicy.processingPolicyVersion.toUInt(),
                            ),
                        )
                    }
                pendingRecoveryToken = record.token
                replaceRecovery(record)
                startFullResolutionProcessing(
                    request = request,
                    file = file,
                    capturedAtMs = capturedAtMs,
                    recoveryToken = record.token,
                    markPreviewReady = true,
                    recovered = false,
                )
            } catch (failure: CancellationException) {
                if (pendingRecoveryToken == null) safeDelete(file)
                throw failure
            } catch (failure: Exception) {
                if (pendingRecoveryToken == null) safeDelete(file)
                clearPendingCaptureReferences()
                applyTransition(
                    captureMachine.processingCompleted(
                        request.token,
                        AutoCaptureProcessingOutcome.Rejected(
                            failure.message ?: "Failed to create scanner recovery journal",
                        ),
                    ),
                )
                val continued = captureMachine.continueScanning(System.nanoTime(), true)
                update {
                    it.copy(
                        processing = false,
                        capturePhase = continued.snapshot.phase,
                        error = failure.message ?: "Failed to create scanner recovery journal",
                        cameraGeneration = nextGeneration(it.cameraGeneration),
                    )
                }
            }
        }
    }

    private fun beginRecoveredReview(prepared: RecoveryPreparation.Ready) {
        val record = prepared.record
        RustScannerPolicySession.update(prepared.storedPolicy)
        recoverySequence = nextGeneration(recoverySequence)
        val generation = nextGeneration(mutableState.value.cameraGeneration)
        val request =
            AutoCaptureRequest(
                token = CaptureRequestToken(generation, recoverySequence),
                pageId = record.pageId,
                activeNotebookId = record.notebookId,
                trigger = CaptureTrigger.MANUAL,
                requestedAtNanos = System.nanoTime(),
            )
        val file = File(record.stagingPath)
        captureMachine.start(AutoCaptureContext(record.notebookId))
        pendingStagingFile = file
        pendingCapturedAtMs = record.capturedAtMs
        pendingRecoveryToken = record.token
        recoveringRecord = record
        update {
            it.copy(
                notebooks = prepared.notebooks,
                activeNotebook = prepared.active,
                loadingNotebooks = false,
                recoveryOperationInProgress = false,
                recoveryMode = true,
                processing = true,
                reviewArtifact = null,
                registeredScan = null,
                capturePhase =
                    AutoCapturePhase.Processing(
                        request,
                        CapturedImage(stagingPath = record.stagingPath),
                    ),
                cameraGeneration = generation,
                error = null,
            )
        }
        startFullResolutionProcessing(
            request = request,
            file = file,
            capturedAtMs = record.capturedAtMs,
            recoveryToken = record.token,
            markPreviewReady = record.phase == ScannerRecoveryPhase.CAPTURED,
            recovered = true,
        )
    }

    private fun refreshNotebooks() {
        viewModelScope.launch {
            try {
                val notebooks = withContext(Dispatchers.IO) { client.listNotebooks(false) }
                val active = requireSingleActiveNotebook(notebooks)
                if (active == null) {
                    captureMachine.stop()
                } else {
                    captureMachine.start(AutoCaptureContext(active.id))
                }
                update {
                    it.copy(
                        notebooks = notebooks,
                        activeNotebook = active,
                        loadingNotebooks = false,
                        capturePhase = captureMachine.snapshot().phase,
                        error = null,
                    )
                }
                rebuildPresentation(System.nanoTime())
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        loadingNotebooks = false,
                        error = failure.message ?: "Failed to load Notebooks",
                    )
                }
            }
        }
    }

    private fun refreshScannerRecoveries() {
        viewModelScope.launch {
            try {
                val records =
                    withContext(Dispatchers.IO) {
                        client.listScannerRecoveries().map { record ->
                            if (record.phase == ScannerRecoveryPhase.REGISTERING) {
                                client.reconcileScannerRecovery(record.token)
                            } else {
                                record
                            }
                        }
                    }
                update {
                    it.copy(
                        scannerRecoveries = records.sortedRecoveries(),
                        recoveryLoading = false,
                        recoveryOperationInProgress = false,
                    )
                }
            } catch (failure: CancellationException) {
                throw failure
            } catch (failure: Exception) {
                update {
                    it.copy(
                        recoveryLoading = false,
                        recoveryOperationInProgress = false,
                        error = failure.message ?: "Failed to load scanner recovery records",
                    )
                }
            }
        }
    }

    private fun requireSingleActiveNotebook(notebooks: List<NotebookSummary>): NotebookSummary? {
        val active = notebooks.filter { it.active }
        check(active.size <= 1) { "Rust returned more than one active Notebook" }
        return active.singleOrNull()
    }

    private fun resetScannerSession(
        active: NotebookSummary?,
        notebooks: List<NotebookSummary>,
    ) {
        cancelActiveWork(clearReview = true)
        if (active == null) {
            captureMachine.stop()
        } else {
            captureMachine.start(AutoCaptureContext(active.id))
        }
        latestAnalysisTimestampNanos = null
        latestAssessment = null
        analysisFailure = null
        update {
            SinglePageScannerUiState(
                notebooks = notebooks,
                activeNotebook = active,
                loadingNotebooks = false,
                capturePhase = captureMachine.snapshot().phase,
                scannerRecoveries = it.scannerRecoveries,
                recoveryLoading = it.recoveryLoading,
                cameraGeneration = nextGeneration(it.cameraGeneration),
            )
        }
        rebuildPresentation(System.nanoTime())
    }

    private fun resetAfterReview() {
        val active = mutableState.value.activeNotebook
        if (active == null) {
            captureMachine.stop()
        } else {
            captureMachine.start(AutoCaptureContext(active.id))
        }
        latestAssessment = null
        latestAnalysisTimestampNanos = null
        analysisFailure = null
        recoveringRecord = null
        clearPendingCaptureReferences()
        update {
            it.copy(
                capturePhase = captureMachine.snapshot().phase,
                reviewArtifact = null,
                processing = false,
                registrationInProgress = false,
                recoveryOperationInProgress = false,
                recoveryMode = false,
                registeredScan = null,
                detailsVisible = false,
                latestAnalysis = null,
                latestPageResolution = null,
                pageCodeStatus = PageCodeUiStatus.Searching,
                presentation = null,
                error = null,
                cameraGeneration = nextGeneration(it.cameraGeneration),
            )
        }
    }

    private fun resolveLivePageCode(event: LiveQrCodeEvent.Found) {
        val active = mutableState.value.activeNotebook ?: run {
            update {
                it.copy(
                    latestPageResolution = null,
                    pageCodeStatus = PageCodeUiStatus.Blocked("Select a Notebook first"),
                )
            }
            rebuildPresentation(event.frameTimestampNanos)
            return
        }
        qrResolutionSequence = nextGeneration(qrResolutionSequence)
        val sequence = qrResolutionSequence
        val generation = mutableState.value.cameraGeneration
        qrResolutionJob?.cancel()
        qrResolutionJob =
            viewModelScope.launch {
                try {
                    val resolution =
                        withContext(Dispatchers.IO) {
                            client.resolvePageCode(event.payload, active.id)
                        }
                    if (
                        sequence != qrResolutionSequence ||
                            generation != mutableState.value.cameraGeneration ||
                            active.id != mutableState.value.activeNotebook?.id
                    ) {
                        return@launch
                    }
                    val status =
                        when (resolution) {
                            is PageResolution.Resolved ->
                                PageCodeUiStatus.Resolved(
                                    resolution.pageId,
                                    event.frameTimestampNanos,
                                )

                            else -> PageCodeUiStatus.Blocked(describeResolution(resolution))
                        }
                    update {
                        it.copy(
                            latestPageResolution = resolution,
                            pageCodeStatus = status,
                            error = null,
                        )
                    }
                    rebuildPresentation(event.frameTimestampNanos)
                    recomputeAssessment(event.frameTimestampNanos, observeForAutoCapture = true)
                } catch (failure: CancellationException) {
                    throw failure
                } catch (failure: Exception) {
                    if (sequence != qrResolutionSequence) return@launch
                    update {
                        it.copy(
                            latestPageResolution = null,
                            pageCodeStatus =
                                PageCodeUiStatus.Failed(
                                    failure.message ?: "Rust rejected the Page Code",
                                ),
                        )
                    }
                    rebuildPresentation(event.frameTimestampNanos)
                }
            }
    }

    private fun handlePageCodeNotFound(timestampNanos: Long) {
        val resolved = mutableState.value.pageCodeStatus as? PageCodeUiStatus.Resolved
        if (
            resolved == null ||
                timestampNanos < resolved.payloadObservedAtNanos ||
                timestampNanos - resolved.payloadObservedAtNanos > policy.pageCodeFreshnessNanos
        ) {
            qrResolutionJob?.cancel()
            update {
                it.copy(
                    latestPageResolution = null,
                    pageCodeStatus = PageCodeUiStatus.Searching,
                )
            }
            rebuildPresentation(timestampNanos)
            recomputeAssessment(timestampNanos, observeForAutoCapture = false)
        }
    }

    private fun recomputeAssessment(
        timestampNanos: Long,
        observeForAutoCapture: Boolean,
    ) {
        val current = mutableState.value
        val analysis = current.latestAnalysis ?: run {
            latestAssessment = null
            return
        }
        val presentation =
            buildLiveScannerPresentation(
                activeNotebook = current.activeNotebook,
                pageResolution = freshResolution(timestampNanos),
                analysis = analysis,
                analysisFailure = analysisFailure,
                policy = policy.guidance,
            )
        val captureAssessment = assessCapturePolicy(analysis, policy.captureThresholds)
        val pageId = (freshResolution(timestampNanos) as? PageResolution.Resolved)?.pageId
        val assessment =
            AutoCaptureFrameAssessment(
                timestampNanos = timestampNanos,
                pageId = pageId,
                identityGate = presentation.identityGate,
                acceptedByCapturePolicy = captureAssessment.accepted,
            )
        latestAssessment = assessment
        update { it.copy(presentation = presentation) }
        if (observeForAutoCapture && policy.autoCaptureEnabled) {
            applyTransition(captureMachine.observeFrame(assessment))
        }
    }

    private fun freshResolution(timestampNanos: Long): PageResolution? {
        val status = mutableState.value.pageCodeStatus as? PageCodeUiStatus.Resolved ?: return null
        if (timestampNanos < status.payloadObservedAtNanos) return null
        if (timestampNanos - status.payloadObservedAtNanos > policy.pageCodeFreshnessNanos) return null
        return mutableState.value.latestPageResolution
    }

    private fun rebuildPresentation(timestampNanos: Long) {
        val current = mutableState.value
        update {
            it.copy(
                presentation =
                    buildLiveScannerPresentation(
                        activeNotebook = current.activeNotebook,
                        pageResolution = freshResolution(timestampNanos),
                        analysis = current.latestAnalysis,
                        analysisFailure = analysisFailure,
                        policy = policy.guidance,
                    ),
            )
        }
    }

    private fun applyTransition(transition: com.a2d.notebook.feature.scanner.capture.AutoCaptureTransition) {
        update {
            it.copy(
                capturePhase = transition.snapshot.phase,
                pendingManualWarning = transition.snapshot.pendingManualWarning,
            )
        }
        transition.effects.forEach { effect ->
            when (effect) {
                is AutoCaptureEffect.CaptureRequested ->
                    update { it.copy(pendingCaptureRequest = effect.request, error = null) }

                is AutoCaptureEffect.ManualCaptureWarningRequired ->
                    update { it.copy(pendingManualWarning = effect.warning, error = null) }

                is AutoCaptureEffect.ManualCaptureDenied ->
                    update { it.copy(error = manualDenialMessage(effect.reason)) }

                is AutoCaptureEffect.CancelActiveWork -> processingCancellation?.cancel()
                is AutoCaptureEffect.CaptureDebounced ->
                    update { it.copy(error = "This page was captured recently") }

                is AutoCaptureEffect.StaleCallbackIgnored -> Unit
            }
        }
    }

    private fun startFullResolutionProcessing(
        request: AutoCaptureRequest,
        file: File,
        capturedAtMs: Long,
        recoveryToken: String,
        markPreviewReady: Boolean,
        recovered: Boolean,
    ) {
        check(processingJob == null) { "full-resolution processing is already active" }
        val cancellation = PagePreviewCancellation()
        processingCancellation = cancellation
        val generation = mutableState.value.cameraGeneration
        processingJob =
            viewModelScope.launch {
                try {
                    val processed =
                        withContext(Dispatchers.IO) {
                            processCapturedFile(file, cancellation)
                        }
                    val stillCurrent =
                        if (recovered) {
                            recoveringRecord?.token == recoveryToken &&
                                pendingStagingFile?.absolutePath == file.absolutePath
                        } else {
                            generation == mutableState.value.cameraGeneration &&
                                pendingStagingFile?.absolutePath == file.absolutePath
                        }
                    if (!stillCurrent) {
                        clearPendingCaptureReferences()
                        refreshScannerRecoveries()
                        return@launch
                    }
                    when (processed) {
                        PagePreviewProcessingOutcome.Cancelled ->
                            handleRetainedProcessingFailure(
                                request,
                                recoveryToken,
                                recovered,
                                "Processing cancelled; the captured file was retained for recovery",
                            )

                        is PagePreviewProcessingOutcome.Completed -> {
                            if (markPreviewReady) {
                                val updated =
                                    withContext(Dispatchers.IO) {
                                        client.markScannerRecoveryPreviewReady(recoveryToken)
                                    }
                                replaceRecovery(updated)
                            }
                            finishReview(
                                request = request,
                                file = file,
                                capturedAtMs = capturedAtMs,
                                recoveryToken = recoveryToken,
                                recovered = recovered,
                                processed = processed,
                            )
                        }
                    }
                } catch (failure: CancellationException) {
                    throw failure
                } catch (failure: Exception) {
                    handleRetainedProcessingFailure(
                        request,
                        recoveryToken,
                        recovered,
                        failure.message ?: "Full-resolution processing failed",
                    )
                } finally {
                    cancellation.close()
                    if (processingCancellation === cancellation) processingCancellation = null
                    processingJob = null
                }
            }
    }

    private fun handleRetainedProcessingFailure(
        request: AutoCaptureRequest,
        recoveryToken: String,
        recovered: Boolean,
        message: String,
    ) {
        clearPendingCaptureReferences()
        recoveringRecord = null
        if (recovered) {
            update {
                it.copy(
                    processing = false,
                    recoveryMode = false,
                    recoveryOperationInProgress = false,
                    reviewArtifact = null,
                    capturePhase = captureMachine.snapshot().phase,
                    error = message,
                )
            }
        } else {
            applyTransition(
                captureMachine.processingCompleted(
                    request.token,
                    AutoCaptureProcessingOutcome.Rejected(message),
                ),
            )
            val continued = captureMachine.continueScanning(System.nanoTime(), true)
            update {
                it.copy(
                    processing = false,
                    capturePhase = continued.snapshot.phase,
                    error = message,
                    cameraGeneration = nextGeneration(it.cameraGeneration),
                )
            }
        }
        if (mutableState.value.scannerRecoveries.none { it.token == recoveryToken }) {
            refreshScannerRecoveries()
        }
    }

    private fun processCapturedFile(
        file: File,
        cancellation: PagePreviewCancellation,
    ): PagePreviewProcessingOutcome {
        require(file.isFile) { "captured staging file does not exist" }
        val size = file.length()
        require(size > 0L) { "captured staging file is empty" }
        require(size <= policy.fullResolution.maximumEncodedBytes) {
            "captured image exceeds the full-resolution byte limit"
        }
        val bytes = file.readBytes()
        check(bytes.size.toLong() == size) { "captured file changed while it was being read" }
        val rotation = readJpegRotation(file)
        return client.processPagePreview(
            PagePreviewProcessingRequest(
                encodedBytes = bytes,
                format = EncodedPageFormat.JPEG,
                rotation = rotation,
                analysisPolicy = policy.liveAnalysis,
                maximumEncodedBytes = policy.fullResolution.maximumEncodedBytes,
                maximumPixels = policy.fullResolution.maximumPixels,
                maximumDecodedBytes = policy.fullResolution.maximumDecodedBytes,
                correctedWidth = policy.fullResolution.correctedWidth,
                correctedHeight = policy.fullResolution.correctedHeight,
                rectificationMaximumOutputPixels =
                    policy.fullResolution.rectificationMaximumOutputPixels,
                rectificationMaximumOutputBytes =
                    policy.fullResolution.rectificationMaximumOutputBytes,
                pipelineVersion = policy.fullResolution.pipelineVersion,
                contrastLowPercentilePerMillion =
                    policy.fullResolution.contrastLowPercentilePerMillion,
                contrastHighPercentilePerMillion =
                    policy.fullResolution.contrastHighPercentilePerMillion,
                contrastMaximumGain = policy.fullResolution.contrastMaximumGain,
                thumbnailMaximumWidth = policy.fullResolution.thumbnailMaximumWidth,
                thumbnailMaximumHeight = policy.fullResolution.thumbnailMaximumHeight,
                derivedMaximumPixelsPerImage =
                    policy.fullResolution.derivedMaximumPixelsPerImage,
                derivedMaximumBytesPerImage =
                    policy.fullResolution.derivedMaximumBytesPerImage,
                derivedMaximumTotalOutputBytes =
                    policy.fullResolution.derivedMaximumTotalOutputBytes,
                derivedMaximumWorkingBytes =
                    policy.fullResolution.derivedMaximumWorkingBytes,
            ),
            cancellation,
        )
    }

    private suspend fun finishReview(
        request: AutoCaptureRequest,
        file: File,
        capturedAtMs: Long,
        recoveryToken: String,
        recovered: Boolean,
        processed: PagePreviewProcessingOutcome.Completed,
    ) {
        val result = processed.result
        val finalCode = resolveFinalPageCode(result.corrected, request)
        val identity =
            assessFinalCaptureIdentity(
                resolution = finalCode.resolution,
                request = request,
                decoderWarning = finalCode.warning,
            )
        val assessment = assessCapturePolicy(result.analysis, policy.captureThresholds)
        val artifact =
            SinglePageReviewArtifact(
                captureRequest = request,
                stagingPath = file.absolutePath,
                recoveryToken = recoveryToken,
                pageCodePayload = finalCode.payload,
                imageRotation = readJpegRotation(file),
                capturedAtMs = capturedAtMs,
                analysis = result.analysis,
                finalResolution = finalCode.resolution,
                corrected = result.corrected.toScannerImage(),
                thumbnail = result.thumbnail.toScannerImage(),
                pipelineVersion = result.pipelineVersion,
                sourceToCorrectedMatrix = result.sourceToCorrectedMatrix,
                warnings = assessment.warnings,
                approvalAllowed = identity.approvalAllowed,
                identityWarning = identity.warning,
            )
        val reason = "Full-resolution capture requires explicit review before registration"
        if (recovered) {
            update {
                it.copy(
                    capturePhase = AutoCapturePhase.NeedsReview(request, reason),
                    recoveryMode = true,
                )
            }
        } else {
            applyTransition(
                captureMachine.processingCompleted(
                    request.token,
                    AutoCaptureProcessingOutcome.NeedsReview(reason),
                ),
            )
        }
        update {
            it.copy(
                processing = false,
                recoveryOperationInProgress = false,
                reviewArtifact = artifact,
                registrationInProgress = false,
                registeredScan = null,
                capturePhase =
                    if (recovered) {
                        AutoCapturePhase.NeedsReview(request, reason)
                    } else {
                        captureMachine.snapshot().phase
                    },
                error = null,
            )
        }
    }

    private data class FinalPageCodeResult(
        val payload: String?,
        val resolution: PageResolution?,
        val warning: String?,
    )

    private sealed interface RecoveryPreparation {
        data class Committed(
            val record: ScannerRecoveryRecord,
        ) : RecoveryPreparation

        data class Ready(
            val record: ScannerRecoveryRecord,
            val notebooks: List<NotebookSummary>,
            val active: NotebookSummary,
            val storedPolicy: com.a2d.notebook.rustbridge.StoredScanPolicy,
        ) : RecoveryPreparation
    }

    private suspend fun resolveFinalPageCode(
        image: ProcessedRgbImage,
        request: AutoCaptureRequest,
    ): FinalPageCodeResult =
        withContext(Dispatchers.IO) {
            try {
                val payload = decodeQrPixels(image.width, image.height, image.toArgbPixels())
                FinalPageCodeResult(
                    payload = payload,
                    resolution = client.resolvePageCode(payload, request.activeNotebookId),
                    warning = null,
                )
            } catch (_: NotFoundException) {
                FinalPageCodeResult(
                    payload = null,
                    resolution = null,
                    warning = "No Page Code was found in the corrected capture.",
                )
            } catch (failure: Exception) {
                FinalPageCodeResult(
                    payload = null,
                    resolution = null,
                    warning = "Final Page Code validation failed: ${failure.message ?: failure}",
                )
            }
        }

    private fun cancelActiveWork(clearReview: Boolean) {
        qrResolutionSequence = nextGeneration(qrResolutionSequence)
        qrResolutionJob?.cancel()
        qrResolutionJob = null
        processingCancellation?.cancel()
        processingJob?.cancel()
        processingJob = null
        processingCancellation?.close()
        processingCancellation = null
        if (!mutableState.value.registrationInProgress) {
            if (pendingRecoveryToken == null) {
                pendingStagingFile?.let(::safeDelete)
            }
            clearPendingCaptureReferences()
            recoveringRecord = null
            if (clearReview) {
                update {
                    it.copy(
                        reviewArtifact = null,
                        processing = false,
                        recoveryMode = false,
                    )
                }
            }
        }
    }

    private fun clearPendingCaptureReferences() {
        pendingStagingFile = null
        pendingCapturedAtMs = null
        pendingRecoveryToken = null
    }

    private fun isCurrentRecovery(record: ScannerRecoveryRecord): Boolean =
        mutableState.value.scannerRecoveries.any {
            it.token == record.token &&
                it.updatedAtMs == record.updatedAtMs &&
                it.phase == record.phase
        }

    private fun replaceRecovery(record: ScannerRecoveryRecord) {
        update {
            it.copy(
                scannerRecoveries =
                    (it.scannerRecoveries.filterNot { item -> item.token == record.token } + record)
                        .sortedRecoveries(),
            )
        }
    }

    private fun removeRecovery(token: String) {
        update {
            it.copy(scannerRecoveries = it.scannerRecoveries.filterNot { item -> item.token == token })
        }
    }

    private fun stagingDirectory(): File =
        A2dBridge
            .libraryDirectory(getApplication<Application>())
            .resolve("tmp/scanner-staging")
            .also { directory ->
                check(directory.isDirectory || directory.mkdirs()) {
                    "Rust-owned scanner staging directory could not be created"
                }
            }

    private fun safeDelete(file: File) {
        val root = stagingDirectory().absoluteFile.normalize()
        val candidate = file.absoluteFile.normalize()
        if (!candidate.toPath().startsWith(root.toPath())) return
        if (candidate.exists() && !candidate.delete()) {
            update { it.copy(error = "Scanner staging file could not be removed") }
        }
    }

    private fun update(transform: (SinglePageScannerUiState) -> SinglePageScannerUiState) {
        mutableState.value = transform(mutableState.value)
    }
}

private fun List<ScannerRecoveryRecord>.sortedRecoveries(): List<ScannerRecoveryRecord> =
    sortedWith(compareBy<ScannerRecoveryRecord> { it.updatedAtMs }.thenBy { it.token })

internal fun readJpegRotation(file: File): EncodedPageRotation {
    val orientation =
        ExifInterface(file.absolutePath).getAttributeInt(
            ExifInterface.TAG_ORIENTATION,
            ExifInterface.ORIENTATION_UNDEFINED,
        )
    return when (orientation) {
        ExifInterface.ORIENTATION_UNDEFINED,
        ExifInterface.ORIENTATION_NORMAL,
        -> EncodedPageRotation.DEGREES_0

        ExifInterface.ORIENTATION_ROTATE_90 -> EncodedPageRotation.DEGREES_90
        ExifInterface.ORIENTATION_ROTATE_180 -> EncodedPageRotation.DEGREES_180
        ExifInterface.ORIENTATION_ROTATE_270 -> EncodedPageRotation.DEGREES_270
        ExifInterface.ORIENTATION_FLIP_HORIZONTAL,
        ExifInterface.ORIENTATION_FLIP_VERTICAL,
        ExifInterface.ORIENTATION_TRANSPOSE,
        ExifInterface.ORIENTATION_TRANSVERSE,
        -> throw IllegalArgumentException(
            "mirrored EXIF orientation is not supported by the Rust preview boundary",
        )

        else -> throw IllegalArgumentException("unsupported EXIF orientation value $orientation")
    }
}

internal fun ProcessedRgbImage.toArgbPixels(): IntArray {
    val expected = Math.multiplyExact(Math.multiplyExact(width, height), 3)
    require(bytes.size == expected) { "RGB byte count does not match its dimensions" }
    val pixels = IntArray(Math.multiplyExact(width, height))
    var source = 0
    for (index in pixels.indices) {
        val red = bytes[source].toInt() and 0xff
        val green = bytes[source + 1].toInt() and 0xff
        val blue = bytes[source + 2].toInt() and 0xff
        pixels[index] = (0xff shl 24) or (red shl 16) or (green shl 8) or blue
        source += 3
    }
    return pixels
}

private fun ProcessedRgbImage.toScannerImage(): ScannerRgbImage =
    ScannerRgbImage(width, height, bytes)

private fun describeResolution(resolution: PageResolution): String =
    when (resolution) {
        is PageResolution.Resolved -> "Page identity resolved"
        is PageResolution.RequiresNotebookSelection -> "Choose which matching Notebook owns this page"
        is PageResolution.RequiresNotebookRegistration -> "Register this Notebook before scanning"
        is PageResolution.ConflictingActiveNotebook -> "The page belongs to a different Notebook"
        is PageResolution.ImportedUnknownSmartPage -> "This is a Smart Page, not a Notebook page"
        is PageResolution.UnsupportedCode -> resolution.reason
    }

private fun manualDenialMessage(reason: ManualCaptureDeniedReason): String =
    when (reason) {
        ManualCaptureDeniedReason.NOT_RUNNING -> "Select a Notebook before capturing"
        ManualCaptureDeniedReason.NO_CURRENT_FRAME -> "No analyzed camera frame is available"
        ManualCaptureDeniedReason.IDENTITY_NOT_VERIFIED -> "Page identity has not been verified"
        ManualCaptureDeniedReason.CAPTURE_ALREADY_ACTIVE -> "A capture is already active"
        ManualCaptureDeniedReason.PAUSED -> "The scanner is paused"
    }

private fun nextGeneration(value: Long): Long =
    if (value == Long.MAX_VALUE) 1L else value + 1L
