package com.a2d.notebook.feature.scanner.singlepage

import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.a2d.notebook.R
import com.a2d.notebook.feature.ocr.AndroidOcrQueueRuntime
import com.a2d.notebook.feature.scanner.camera.CameraPermissionStatus
import com.a2d.notebook.feature.scanner.camera.rememberCameraPermissionState
import com.a2d.notebook.rustbridge.A2dBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@Composable
internal fun PolicyAwareSinglePageScannerRoute(
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit = {},
    viewModel: SinglePageScannerViewModel,
) {
    val state by viewModel.state
    val permission = rememberCameraPermissionState()
    val context = LocalContext.current
    val registeredScan = state.registeredScan

    LaunchedEffect(registeredScan?.scanId) {
        val scan = registeredScan ?: return@LaunchedEffect
        val failure =
            withContext(Dispatchers.IO) {
                AndroidOcrQueueRuntime
                    .enqueueRegisteredScan(
                        context = context.applicationContext,
                        client = A2dBridge.client(context.applicationContext),
                        libraryRoot = A2dBridge.libraryDirectory(context.applicationContext),
                        scan = scan,
                    ).exceptionOrNull()
            }
        if (failure != null) {
            Log.w(OCR_QUEUE_LOG_TAG, "Saved scan could not be enqueued for OCR", failure)
        }
    }

    BackHandler(enabled = state.navigationBlocked) {}

    if (state.recoveryMode) {
        ScannerRecoveryContent(
            onBack = onBack,
            viewModel = viewModel,
        )
    } else {
        when (permission.status) {
            CameraPermissionStatus.Granted ->
                ScannerGrantedContent(
                    onBack = onBack,
                    onOpenVersions = onOpenVersions,
                    viewModel = viewModel,
                )
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
internal fun PolicyAwareBatchScannerRoute(onBack: () -> Unit) {
    val permission = rememberCameraPermissionState()
    when (permission.status) {
        CameraPermissionStatus.Granted -> BatchScannerScreen(onBack)
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

private const val OCR_QUEUE_LOG_TAG = "A2dOcrQueue"
