package com.a2d.notebook.feature.scanner.singlepage

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.a2d.notebook.R
import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.camera.CameraPermissionStatus
import com.a2d.notebook.feature.scanner.camera.CameraPreviewSurface
import com.a2d.notebook.feature.scanner.camera.rememberCameraPermissionState
import com.a2d.notebook.rustbridge.A2dBridge
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.PageResolution

@Composable
fun SinglePageScannerScreen(
    onBack: () -> Unit,
    viewModel: SinglePageScannerViewModel = viewModel(),
) {
    val state by viewModel.state
    val permission = rememberCameraPermissionState()
    BackHandler(enabled = state.registrationInProgress) {}

    when (permission.status) {
        CameraPermissionStatus.Granted -> {
            val applicationContext = LocalContext.current.applicationContext
            val policy = remember { SinglePageScannerPolicies.V1 }
            val resolvedPageId =
                (state.latestPageResolution as? PageResolution.Resolved)?.pageId

            LaunchedEffect(resolvedPageId, state.cameraGeneration, policy) {
                policy.clearStoredLayoutPolicy()
                if (resolvedPageId != null) {
                    try {
                        val storedLayout =
                            withContext(Dispatchers.IO) {
                                A2dBridge.client(applicationContext)
                                    .resolveStoredScanLayoutPolicy(resolvedPageId)
                            }
                        policy.applyStoredLayoutPolicy(storedLayout)
                    } catch (failure: CancellationException) {
                        throw failure
                    } catch (failure: Exception) {
                        policy.clearStoredLayoutPolicy()
                        viewModel.onCameraStateChanged(
                            CameraAdapterState.Error(
                                message =
                                    failure.message
                                        ?: "Rust failed to resolve the stored page scan layout",
                                cause = failure,
                            ),
                        )
                    }
                }
            }
            DisposableEffect(policy) {
                onDispose { policy.clearStoredLayoutPolicy() }
            }

            val adapter =
                rememberSinglePageCameraXAdapter(
                    policy = policy,
                    generation = state.cameraGeneration,
                    onLiveAnalysisEvent = viewModel::onLiveAnalysisEvent,
                    onQrCodeEvent = viewModel::onQrCodeEvent,
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
                    CameraPreviewSurface(
                        adapter = adapter,
                        modifier = Modifier.fillMaxSize(),
                    )
                },
            )
        }

        CameraPermissionStatus.NotRequested ->
            CameraPermissionContent(
                explanation = stringResource(R.string.single_scanner_permission_title),
                actionLabel = stringResource(R.string.single_scanner_permission_request),
                onAction = permission.requestPermission,
                onBack = onBack,
            )

        CameraPermissionStatus.Denied ->
            CameraPermissionContent(
                explanation = stringResource(R.string.single_scanner_permission_title),
                actionLabel = stringResource(R.string.common_retry),
                onAction = permission.requestPermission,
                onBack = onBack,
            )

        CameraPermissionStatus.PermanentlyDenied ->
            CameraPermissionContent(
                explanation = stringResource(R.string.single_scanner_camera_unavailable),
                actionLabel = stringResource(R.string.single_scanner_permission_settings),
                onAction = permission.openApplicationSettings,
                onBack = onBack,
            )
    }
}

@Composable
private fun CameraPermissionContent(
    explanation: String,
    actionLabel: String,
    onAction: () -> Unit,
    onBack: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(explanation, style = MaterialTheme.typography.titleLarge)
        Button(
            onClick = onAction,
            modifier = Modifier.padding(top = 16.dp),
        ) {
            Text(actionLabel)
        }
        TextButton(
            onClick = onBack,
            modifier = Modifier.padding(top = 8.dp),
        ) {
            Text(stringResource(R.string.common_back))
        }
    }
}
