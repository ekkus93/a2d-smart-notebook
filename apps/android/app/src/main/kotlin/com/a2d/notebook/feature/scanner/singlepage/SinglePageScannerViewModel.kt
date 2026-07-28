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
import com.google.zxing.NotFoundException
import java.io.File
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution

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
    private var qrResolutionJob: Job? = null
    private var processingJob: Job? = null
    private var processingCancellation: PagePreviewCancellation? = null
    private var pendingStagingFile: File? = null

    init {
        refreshNotebooks()
    }

    fun selectNotebook(notebook: NotebookSummary) {
        if (mutableState.value.processing || mutableState.value.activeNotebook?.id == notebook.id) {
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
                startFullResolutionProcessing(request, file)
            }

            is CameraCaptureResult.Failure -> {
                pendingStagingFile?.let(::safeDelete)
                pendingStagingFile = null
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

    fun approveReview() {
        val current = mutableState.value
        if (!current.canApprove) {
            update { it.copy(error = "Final page identity must match before approval") }
            return
        }
        update { it.copy(approvedForRegistration = true, error = null) }
    }

    fun retake() {
        val artifact = mutableState.value.reviewArtifact ?: return
        safeDelete(File(artifact.stagingPath))
        pendingStagingFile = null
        val transition = captureMachine.continueScanning(System.nanoTime(), true)
        latestAssessment = null
        update {
            it.copy(
                capturePhase = transition.snapshot.phase,
                reviewArtifact = null,
                approvedForRegistration = false,
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

    fun toggleDetails() {
        update { it.copy(detailsVisible = !it.detailsVisible) }
    }

    fun leaveScanner() {
        cancelActiveWork(deleteReview = true)
        applyTransition(captureMachine.stop())
        update { it.copy(cameraGeneration = nextGeneration(it.cameraGeneration)) }
    }

    override fun onCleared() {
        cancelActiveWork(deleteReview = true)
        captureMachine.stop()
        super.onCleared()
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

    private fun requireSingleActiveNotebook(notebooks: List<NotebookSummary>): NotebookSummary? {
        val active = notebooks.filter { it.active }
        check(active.size <= 1) { "Rust returned more than one active Notebook" }
        return active.singleOrNull()
    }

    private fun resetScannerSession(
        active: NotebookSummary?,
        notebooks: List<NotebookSummary>,
    ) {
        cancelActiveWork(deleteReview = true)
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
                cameraGeneration = nextGeneration(it.cameraGeneration),
            )
        }
        rebuildPresentation(System.nanoTime())
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
                    if (
                        generation != mutableState.value.cameraGeneration ||
                            pendingStagingFile?.absolutePath != file.absolutePath
                    ) {
                        safeDelete(file)
                        return@launch
                    }
                    when (processed) {
                        PagePreviewProcessingOutcome.Cancelled -> {
                            safeDelete(file)
                            pendingStagingFile = null
                            applyTransition(
                                captureMachine.processingCompleted(
                                    request.token,
                                    AutoCaptureProcessingOutcome.Rejected("Processing cancelled"),
                                ),
                            )
                            val continued = captureMachine.continueScanning(System.nanoTime(), true)
                            update {
                                it.copy(
                                    processing = false,
                                    capturePhase = continued.snapshot.phase,
                                    error = null,
                                    cameraGeneration = nextGeneration(it.cameraGeneration),
                                )
                            }
                        }

                        is PagePreviewProcessingOutcome.Completed ->
                            finishReview(request, file, processed)
                    }
                } catch (failure: CancellationException) {
                    safeDelete(file)
                    throw failure
                } catch (failure: Exception) {
                    safeDelete(file)
                    pendingStagingFile = null
                    applyTransition(
                        captureMachine.processingCompleted(
                            request.token,
                            AutoCaptureProcessingOutcome.Rejected(
                                failure.message ?: "Full-resolution processing failed",
                            ),
                        ),
                    )
                    val continued = captureMachine.continueScanning(System.nanoTime(), true)
                    update {
                        it.copy(
                            processing = false,
                            capturePhase = continued.snapshot.phase,
                            error = failure.message ?: "Full-resolution processing failed",
                            cameraGeneration = nextGeneration(it.cameraGeneration),
                        )
                    }
                } finally {
                    cancellation.close()
                    if (processingCancellation === cancellation) processingCancellation = null
                    processingJob = null
                }
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
        applyTransition(
            captureMachine.processingCompleted(
                request.token,
                AutoCaptureProcessingOutcome.NeedsReview(
                    "Full-resolution capture requires explicit review before registration",
                ),
            ),
        )
        update {
            it.copy(
                processing = false,
                reviewArtifact = artifact,
                approvedForRegistration = false,
                capturePhase = captureMachine.snapshot().phase,
                error = null,
            )
        }
    }

    private data class FinalPageCodeResult(
        val resolution: PageResolution?,
        val warning: String?,
    )

    private suspend fun resolveFinalPageCode(
        image: ProcessedRgbImage,
        request: AutoCaptureRequest,
    ): FinalPageCodeResult =
        withContext(Dispatchers.IO) {
            try {
                val payload = decodeQrPixels(image.width, image.height, image.toArgbPixels())
                FinalPageCodeResult(
                    resolution = client.resolvePageCode(payload, request.activeNotebookId),
                    warning = null,
                )
            } catch (_: NotFoundException) {
                FinalPageCodeResult(null, "No Page Code was found in the corrected capture.")
            } catch (failure: Exception) {
                FinalPageCodeResult(
                    null,
                    "Final Page Code validation failed: ${failure.message ?: failure}",
                )
            }
        }

    private fun cancelActiveWork(deleteReview: Boolean) {
        qrResolutionSequence = nextGeneration(qrResolutionSequence)
        qrResolutionJob?.cancel()
        qrResolutionJob = null
        processingCancellation?.cancel()
        processingJob?.cancel()
        processingJob = null
        processingCancellation?.close()
        processingCancellation = null
        pendingStagingFile?.let(::safeDelete)
        pendingStagingFile = null
        if (deleteReview) {
            mutableState.value.reviewArtifact?.let { safeDelete(File(it.stagingPath)) }
        }
    }

    private fun stagingDirectory(): File =
        File(getApplication<Application>().cacheDir, "a2d-scanner-staging")

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
