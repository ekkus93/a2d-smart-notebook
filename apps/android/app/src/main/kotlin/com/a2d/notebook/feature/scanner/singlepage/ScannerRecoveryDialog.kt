package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import uniffi.a2d_ffi.ScannerRecoveryPhase
import uniffi.a2d_ffi.ScannerRecoveryRecord

internal object ScannerRecoveryTestTags {
    const val DIALOG = "single_scanner_recovery_dialog"
    const val PRIMARY_ACTION = "single_scanner_recovery_primary"
    const val DISCARD_ACTION = "single_scanner_recovery_discard"
}

@Composable
internal fun ScannerRecoveryDialog(
    state: SinglePageScannerUiState,
    onReview: (ScannerRecoveryRecord) -> Unit,
    onReconcile: (ScannerRecoveryRecord) -> Unit,
    onAcknowledge: (ScannerRecoveryRecord) -> Unit,
    onDiscard: (ScannerRecoveryRecord) -> Unit,
) {
    val record = state.scannerRecoveries.firstOrNull() ?: return
    if (
        state.recoveryMode ||
            state.processing ||
            state.reviewArtifact != null ||
            state.registrationInProgress
    ) {
        return
    }

    AlertDialog(
        modifier = Modifier.testTag(ScannerRecoveryTestTags.DIALOG),
        onDismissRequest = {},
        title = { Text(stringResource(R.string.single_scanner_recovery_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(
                        R.string.single_scanner_recovery_count,
                        state.scannerRecoveries.size,
                    ),
                )
                Text(stringResource(R.string.single_scanner_recovery_page, record.pageId))
                Text(recoveryExplanation(record.phase))
                record.registeredScanId?.let { scanId ->
                    Text(stringResource(R.string.single_scanner_recovery_saved_id, scanId))
                }
                if (state.recoveryOperationInProgress) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        CircularProgressIndicator()
                        Text(stringResource(R.string.single_scanner_recovery_working))
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                modifier = Modifier.testTag(ScannerRecoveryTestTags.PRIMARY_ACTION),
                enabled = !state.recoveryOperationInProgress,
                onClick = {
                    when (record.primaryAction()) {
                        ScannerRecoveryPrimaryAction.REVIEW -> onReview(record)
                        ScannerRecoveryPrimaryAction.RECONCILE -> onReconcile(record)
                        ScannerRecoveryPrimaryAction.ACKNOWLEDGE -> onAcknowledge(record)
                    }
                },
            ) {
                Text(recoveryActionLabel(record.primaryAction()))
            }
        },
        dismissButton = {
            if (record.canDiscard()) {
                TextButton(
                    modifier = Modifier.testTag(ScannerRecoveryTestTags.DISCARD_ACTION),
                    enabled = !state.recoveryOperationInProgress,
                    onClick = { onDiscard(record) },
                ) {
                    Text(stringResource(R.string.single_scanner_recovery_discard))
                }
            }
        },
    )
}

@Composable
private fun recoveryExplanation(phase: ScannerRecoveryPhase): String =
    stringResource(
        when (phase) {
            ScannerRecoveryPhase.CAPTURED -> R.string.single_scanner_recovery_captured
            ScannerRecoveryPhase.PREVIEW_READY -> R.string.single_scanner_recovery_preview_ready
            ScannerRecoveryPhase.REGISTERING -> R.string.single_scanner_recovery_registering
            ScannerRecoveryPhase.COMMITTED -> R.string.single_scanner_recovery_committed
        },
    )

@Composable
private fun recoveryActionLabel(action: ScannerRecoveryPrimaryAction): String =
    stringResource(
        when (action) {
            ScannerRecoveryPrimaryAction.REVIEW -> R.string.single_scanner_recovery_review
            ScannerRecoveryPrimaryAction.RECONCILE -> R.string.single_scanner_recovery_reconcile
            ScannerRecoveryPrimaryAction.ACKNOWLEDGE ->
                R.string.single_scanner_recovery_acknowledge
        },
    )
