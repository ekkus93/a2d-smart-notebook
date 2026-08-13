package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.feature.scanner.camera.CameraPreviewSurface

internal const val SCANNER_VERSION_HISTORY_TEST_TAG = "single_scanner_versions"

@Composable
internal fun ScannerGrantedContent(
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit = {},
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
    val savedPageId = state.registeredScan?.pageId
    Column(modifier = Modifier.fillMaxSize()) {
        Box(modifier = Modifier.weight(1f)) {
            SinglePageScannerContent(
                state = state,
                onBack = {
                    if (!state.navigationBlocked) {
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
        if (savedPageId != null) {
            OutlinedButton(
                onClick = {
                    if (!state.navigationBlocked) {
                        viewModel.leaveScanner()
                        onOpenVersions(savedPageId)
                    }
                },
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 8.dp)
                        .testTag(SCANNER_VERSION_HISTORY_TEST_TAG),
            ) {
                Text(stringResource(R.string.single_scanner_versions))
            }
        }
    }
}
