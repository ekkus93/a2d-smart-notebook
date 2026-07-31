package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.a2d.notebook.feature.scanner.camera.CameraPreviewSurface

@Composable
internal fun ScannerGrantedContent(
    onBack: () -> Unit,
    viewModel: SinglePageScannerViewModel,
) {
    val state by viewModel.state
    val qrHandler =
        rememberPolicyAwareQrEventHandler(
            viewModel = viewModel,
            activeNotebookId = state.activeNotebook?.id,
            generation = state.cameraGeneration,
        )
    val adapter =
        rememberSinglePageCameraXAdapter(
            policy = SinglePageScannerPolicies.V1,
            generation = state.cameraGeneration,
            onLiveAnalysisEvent = viewModel::onLiveAnalysisEvent,
            onQrCodeEvent = qrHandler,
            onStateChanged = viewModel::onCameraStateChanged,
        )
    val pendingCapture = state.pendingCaptureRequest
    LaunchedEffect(pendingCapture, adapter) {
        if (pendingCapture != null) {
            viewModel.consumePendingCapture(pendingCapture)?.let { stagingFile ->
                adapter.capture(stagingFile) { result ->
                    viewModel.onCameraCaptureResult(pendingCapture, result)
                }
            }
        }
    }
    SinglePageScannerContent(
        state = state,
        onBack = {
            if (!state.registrationInProgress) {
                viewModel.leaveScanner()
                onBack()
            }
        },
        onSelectNotebook = viewModel::selectNotebook,
        onManualCapture = viewModel::requestManualCapture,
        onConfirmManualCapture = viewModel::confirmManualCapture,
        onDismissManualCapture = viewModel::dismissManualCapture,
        onToggleTorch = { adapter.setTorch(!state.torchEnabled) },
        onCancelProcessing = viewModel::cancelProcessing,
        onApprove = viewModel::approveReview,
        onRetake = viewModel::retake,
        onToggleDetails = viewModel::toggleDetails,
        preview = {
            CameraPreviewSurface(adapter = adapter, modifier = Modifier.fillMaxSize())
        },
    )
}
