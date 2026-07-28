package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.capture.AutoCapturePhase
import com.a2d.notebook.feature.scanner.capture.AutoCaptureRequest
import com.a2d.notebook.feature.scanner.capture.ManualCaptureWarning
import com.a2d.notebook.feature.scanner.presentation.LiveScannerPresentationState
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import com.a2d.notebook.rustbridge.EncodedPageRotation
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution
import uniffi.a2d_ffi.RegisteredScan

sealed interface PageCodeUiStatus {
    data object Searching : PageCodeUiStatus

    data class Resolved(
        val pageId: String,
        val payloadObservedAtNanos: Long,
    ) : PageCodeUiStatus

    data class Blocked(
        val explanation: String,
    ) : PageCodeUiStatus

    data class Failed(
        val explanation: String,
    ) : PageCodeUiStatus
}

data class ScannerRgbImage(
    val width: Int,
    val height: Int,
    val bytes: ByteArray,
) {
    init {
        require(width > 0 && height > 0)
        require(bytes.size == Math.multiplyExact(Math.multiplyExact(width, height), 3))
    }
}

data class SinglePageReviewArtifact(
    val captureRequest: AutoCaptureRequest,
    val stagingPath: String,
    val pageCodePayload: String?,
    val imageRotation: EncodedPageRotation,
    val capturedAtMs: Long,
    val analysis: EncodedPageAnalysisResult,
    val finalResolution: PageResolution?,
    val corrected: ScannerRgbImage,
    val thumbnail: ScannerRgbImage,
    val pipelineVersion: Int,
    val sourceToCorrectedMatrix: List<Double>,
    val warnings: Set<CapturePolicyWarning>,
    val approvalAllowed: Boolean,
    val identityWarning: String?,
) {
    init {
        require(stagingPath.isNotBlank())
        require(capturedAtMs > 0L)
        require(!approvalAllowed || !pageCodePayload.isNullOrBlank()) {
            "an approvable review artifact must retain its validated Page Code payload"
        }
        require(pipelineVersion > 0)
        require(sourceToCorrectedMatrix.size == 9)
        require(approvalAllowed || !identityWarning.isNullOrBlank()) {
            "a blocked review artifact must explain its identity conflict"
        }
    }
}

data class SinglePageScannerUiState(
    val notebooks: List<NotebookSummary> = emptyList(),
    val activeNotebook: NotebookSummary? = null,
    val loadingNotebooks: Boolean = true,
    val cameraState: CameraAdapterState = CameraAdapterState.Idle,
    val presentation: LiveScannerPresentationState? = null,
    val latestAnalysis: EncodedPageAnalysisResult? = null,
    val latestPageResolution: PageResolution? = null,
    val pageCodeStatus: PageCodeUiStatus = PageCodeUiStatus.Searching,
    val capturePhase: AutoCapturePhase = AutoCapturePhase.Idle,
    val pendingCaptureRequest: AutoCaptureRequest? = null,
    val pendingManualWarning: ManualCaptureWarning? = null,
    val processing: Boolean = false,
    val reviewArtifact: SinglePageReviewArtifact? = null,
    val registrationInProgress: Boolean = false,
    val registeredScan: RegisteredScan? = null,
    val detailsVisible: Boolean = false,
    val cameraGeneration: Long = 0,
    val error: String? = null,
) {
    val torchAvailable: Boolean
        get() = (cameraState as? CameraAdapterState.Bound)?.torchAvailable == true

    val torchEnabled: Boolean
        get() = (cameraState as? CameraAdapterState.Bound)?.torchEnabled == true

    val canCaptureManually: Boolean
        get() =
            activeNotebook != null &&
                cameraState is CameraAdapterState.Bound &&
                !processing &&
                reviewArtifact == null

    val canApprove: Boolean
        get() =
            reviewArtifact?.approvalAllowed == true &&
                !processing &&
                !registrationInProgress &&
                registeredScan == null
}
