package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

@Composable
internal fun ScannerRecoveryContent(
    onBack: () -> Unit,
    viewModel: SinglePageScannerViewModel,
) {
    val state by viewModel.state
    SinglePageScannerContent(
        state = state,
        onBack = {
            if (!state.registrationInProgress && !state.recoveryOperationInProgress) {
                viewModel.leaveScanner()
                onBack()
            }
        },
        onSelectNotebook = viewModel::selectNotebook,
        onManualCapture = {},
        onConfirmManualCapture = {},
        onDismissManualCapture = {},
        onToggleTorch = {},
        onCancelProcessing = viewModel::cancelProcessing,
        onApprove = viewModel::approveReview,
        onRetake = viewModel::retake,
        onToggleDetails = viewModel::toggleDetails,
        preview = {
            Box(Modifier.fillMaxSize().background(Color.Black))
        },
    )
}
