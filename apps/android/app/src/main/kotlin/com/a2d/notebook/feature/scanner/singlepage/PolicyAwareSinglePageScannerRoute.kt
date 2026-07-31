package com.a2d.notebook.feature.scanner.singlepage

import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.res.stringResource
import com.a2d.notebook.R
import com.a2d.notebook.feature.scanner.camera.CameraPermissionStatus
import com.a2d.notebook.feature.scanner.camera.rememberCameraPermissionState

@Composable
internal fun PolicyAwareSinglePageScannerRoute(
    onBack: () -> Unit,
    viewModel: SinglePageScannerViewModel,
) {
    val state by viewModel.state
    val permission = rememberCameraPermissionState()
    BackHandler(enabled = state.navigationBlocked) {}

    if (state.recoveryMode) {
        ScannerRecoveryContent(onBack = onBack, viewModel = viewModel)
    } else {
        when (permission.status) {
            CameraPermissionStatus.Granted -> ScannerGrantedContent(onBack, viewModel)
            CameraPermissionStatus.NotRequested ->
                scannerPermission(
                    explanation = stringResource(R.string.single_scanner_permission_title),
                    actionLabel = stringResource(R.string.single_scanner_permission_request),
                    onAction = permission.requestPermission,
                    onBack = onBack,
                )
            CameraPermissionStatus.Denied ->
                scannerPermission(
                    explanation = stringResource(R.string.single_scanner_permission_title),
                    actionLabel = stringResource(R.string.common_retry),
                    onAction = permission.requestPermission,
                    onBack = onBack,
                )
            CameraPermissionStatus.PermanentlyDenied ->
                scannerPermission(
                    explanation = stringResource(R.string.single_scanner_camera_unavailable),
                    actionLabel = stringResource(R.string.single_scanner_permission_settings),
                    onAction = permission.openApplicationSettings,
                    onBack = onBack,
                )
        }
    }

    ScannerRecoveryDialog(
        state = state,
        onReview = viewModel::reviewRecovery,
        onReconcile = viewModel::reconcileRecovery,
        onAcknowledge = viewModel::acknowledgeRecovery,
        onDiscard = viewModel::discardRecovery,
    )
}

@Composable
private fun scannerPermission(
    explanation: String,
    actionLabel: String,
    onAction: () -> Unit,
    onBack: () -> Unit,
) {
    ScannerPermissionContent(
        explanation = explanation,
        actionLabel = actionLabel,
        backLabel = stringResource(R.string.common_back),
        onAction = onAction,
        onBack = onBack,
    )
}
