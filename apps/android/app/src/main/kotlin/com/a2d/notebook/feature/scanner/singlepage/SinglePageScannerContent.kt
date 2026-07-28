package com.a2d.notebook.feature.scanner.singlepage

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.feature.scanner.capture.ManualCaptureWarningCode
import com.a2d.notebook.feature.scanner.presentation.LiveScannerChrome
import uniffi.a2d_ffi.NotebookSummary

object SinglePageScannerTestTags {
    const val NOTEBOOK_SELECTOR = "single_scanner_notebook_selector"
    const val MARKER_STATUS = "single_scanner_marker_status"
    const val PAGE_CODE_STATUS = "single_scanner_page_code_status"
    const val MANUAL_CAPTURE = "single_scanner_manual_capture"
    const val TORCH = "single_scanner_torch"
    const val PROCESSING = "single_scanner_processing"
    const val CORRECTED_PREVIEW = "single_scanner_corrected_preview"
    const val REGISTRATION_RESULT = "single_scanner_registration_result"
    const val IDENTITY_WARNING = "single_scanner_identity_warning"
}

@Composable
fun SinglePageScannerContent(
    state: SinglePageScannerUiState,
    onBack: () -> Unit,
    onSelectNotebook: (NotebookSummary) -> Unit,
    onManualCapture: () -> Unit,
    onConfirmManualCapture: () -> Unit,
    onDismissManualCapture: () -> Unit,
    onToggleTorch: () -> Unit,
    onCancelProcessing: () -> Unit,
    onApprove: () -> Unit,
    onRetake: () -> Unit,
    onToggleDetails: () -> Unit,
    preview: @Composable () -> Unit,
) {
    var notebookMenuExpanded by remember { mutableStateOf(false) }
    Column(
        modifier = Modifier.fillMaxSize().padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TextButton(
                onClick = onBack,
                enabled = !state.registrationInProgress,
            ) {
                Text(stringResource(R.string.common_back))
            }
            Text(
                text = stringResource(R.string.single_scanner_title),
                style = MaterialTheme.typography.titleLarge,
            )
        }

        Box {
            OutlinedButton(
                onClick = { notebookMenuExpanded = true },
                enabled =
                    !state.loadingNotebooks &&
                        !state.processing &&
                        state.reviewArtifact == null,
                modifier = Modifier.fillMaxWidth().testTag(SinglePageScannerTestTags.NOTEBOOK_SELECTOR),
            ) {
                Text(
                    state.activeNotebook?.displayName
                        ?: stringResource(R.string.single_scanner_choose_notebook),
                )
            }
            DropdownMenu(
                expanded = notebookMenuExpanded,
                onDismissRequest = { notebookMenuExpanded = false },
            ) {
                state.notebooks.forEach { notebook ->
                    DropdownMenuItem(
                        text = { Text(notebook.displayName) },
                        onClick = {
                            notebookMenuExpanded = false
                            onSelectNotebook(notebook)
                        },
                    )
                }
            }
        }

        if (!state.loadingNotebooks && state.notebooks.isEmpty()) {
            Text(
                text = stringResource(R.string.single_scanner_no_notebooks),
                color = MaterialTheme.colorScheme.error,
            )
        }
        state.error?.let {
            Text(
                text = stringResource(R.string.common_error_prefix, it),
                color = MaterialTheme.colorScheme.error,
            )
        }

        if (state.reviewArtifact == null) {
            Box(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .heightIn(min = 240.dp)
                        .background(Color.Black),
            ) {
                val presentation = state.presentation
                if (presentation == null) {
                    preview()
                } else {
                    LiveScannerChrome(
                        state = presentation,
                        modifier = Modifier.fillMaxSize(),
                        preview = { preview() },
                    )
                }
            }
            ScannerStatusRow(state)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(
                    onClick = onToggleTorch,
                    enabled = state.torchAvailable && !state.processing,
                    modifier = Modifier.weight(1f).testTag(SinglePageScannerTestTags.TORCH),
                ) {
                    Text(
                        stringResource(
                            if (state.torchEnabled) {
                                R.string.single_scanner_torch_off
                            } else {
                                R.string.single_scanner_torch_on
                            },
                        ),
                    )
                }
                Button(
                    onClick = onManualCapture,
                    enabled = state.canCaptureManually,
                    modifier = Modifier.weight(1f).testTag(SinglePageScannerTestTags.MANUAL_CAPTURE),
                ) {
                    Text(stringResource(R.string.single_scanner_manual_capture))
                }
            }
            Text(
                text = stringResource(R.string.single_scanner_auto_capture_calibration),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            ReviewContent(
                state = state,
                onApprove = onApprove,
                onRetake = onRetake,
                onToggleDetails = onToggleDetails,
            )
        }
    }

    if (state.processing || state.registrationInProgress) {
        AlertDialog(
            modifier = Modifier.testTag(SinglePageScannerTestTags.PROCESSING),
            onDismissRequest = {},
            title = {
                Text(
                    stringResource(
                        if (state.registrationInProgress) {
                            R.string.single_scanner_registering
                        } else {
                            R.string.single_scanner_processing
                        },
                    ),
                )
            },
            text = {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    CircularProgressIndicator()
                    Text(
                        stringResource(
                            if (state.registrationInProgress) {
                                R.string.single_scanner_registering_explanation
                            } else {
                                R.string.single_scanner_review_explanation
                            },
                        ),
                    )
                }
            },
            confirmButton = {
                if (state.processing) {
                    TextButton(onClick = onCancelProcessing) {
                        Text(stringResource(R.string.single_scanner_cancel_processing))
                    }
                }
            },
        )
    }

    state.pendingManualWarning?.let { warning ->
        AlertDialog(
            onDismissRequest = onDismissManualCapture,
            title = { Text(stringResource(R.string.single_scanner_warning_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.single_scanner_manual_warning))
                    warning.warningCodes.forEach { code ->
                        Text("• ${manualWarningText(code)}")
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = onConfirmManualCapture) {
                    Text(stringResource(R.string.single_scanner_continue_capture))
                }
            },
            dismissButton = {
                TextButton(onClick = onDismissManualCapture) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }

    if (state.detailsVisible) {
        ScannerDetailsDialog(state = state, onDismiss = onToggleDetails)
    }
}

@Composable
private fun ScannerStatusRow(state: SinglePageScannerUiState) {
    val markerCount = state.latestAnalysis?.markers?.map { it.role.uppercase() }?.toSet()?.size ?: 0
    val markerText =
        if (markerCount == 0) {
            stringResource(R.string.single_scanner_markers_searching)
        } else {
            stringResource(R.string.single_scanner_markers_found, markerCount)
        }
    val pageCodeText =
        when (state.pageCodeStatus) {
            PageCodeUiStatus.Searching -> stringResource(R.string.single_scanner_page_code_searching)
            is PageCodeUiStatus.Resolved -> stringResource(R.string.single_scanner_page_code_resolved)
            is PageCodeUiStatus.Blocked -> stringResource(R.string.single_scanner_page_code_blocked)
            is PageCodeUiStatus.Failed -> stringResource(R.string.single_scanner_page_code_failed)
        }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(markerText, modifier = Modifier.testTag(SinglePageScannerTestTags.MARKER_STATUS))
        Text(pageCodeText, modifier = Modifier.testTag(SinglePageScannerTestTags.PAGE_CODE_STATUS))
    }
}

@Composable
private fun ReviewContent(
    state: SinglePageScannerUiState,
    onApprove: () -> Unit,
    onRetake: () -> Unit,
    onToggleDetails: () -> Unit,
) {
    val artifact = requireNotNull(state.reviewArtifact)
    val bitmap = remember(artifact.corrected) { artifact.corrected.toBitmap() }
    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(
            text = stringResource(R.string.single_scanner_review_title),
            style = MaterialTheme.typography.headlineSmall,
        )
        Image(
            bitmap = bitmap.asImageBitmap(),
            contentDescription = stringResource(R.string.single_scanner_review_title),
            modifier =
                Modifier
                    .fillMaxWidth()
                    .heightIn(max = 540.dp)
                    .testTag(SinglePageScannerTestTags.CORRECTED_PREVIEW),
        )
        Text(stringResource(R.string.single_scanner_review_explanation))
        artifact.identityWarning?.let { warning ->
            Text(
                text = warning,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.testTag(SinglePageScannerTestTags.IDENTITY_WARNING),
            )
        }
        artifact.warnings.forEach { warning ->
            Text(
                text = stringResource(R.string.single_scanner_detail_warning, warning.name),
                color = MaterialTheme.colorScheme.tertiary,
            )
        }
        state.registeredScan?.let { registered ->
            Card(
                modifier = Modifier.fillMaxWidth().testTag(SinglePageScannerTestTags.REGISTRATION_RESULT),
            ) {
                Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(
                        stringResource(R.string.single_scanner_saved),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.single_scanner_saved_scan_id, registered.scanId))
                    Text(
                        stringResource(
                            R.string.single_scanner_saved_status,
                            registered.qualityStatus.name,
                        ),
                    )
                    registered.warnings.forEach { warning ->
                        Text(stringResource(R.string.single_scanner_detail_warning, warning.name))
                    }
                    registered.requiredActions.forEach { action ->
                        Text(stringResource(R.string.single_scanner_required_action, action.name))
                    }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (state.registeredScan == null) {
                Button(
                    onClick = onApprove,
                    enabled = state.canApprove,
                ) {
                    Text(stringResource(R.string.single_scanner_save_scan))
                }
            }
            OutlinedButton(
                onClick = onRetake,
                enabled = !state.registrationInProgress,
            ) {
                Text(
                    stringResource(
                        if (state.registeredScan == null) {
                            R.string.single_scanner_retake
                        } else {
                            R.string.single_scanner_scan_another
                        },
                    ),
                )
            }
            TextButton(onClick = onToggleDetails) {
                Text(stringResource(R.string.common_details))
            }
        }
    }
}

@Composable
private fun ScannerDetailsDialog(
    state: SinglePageScannerUiState,
    onDismiss: () -> Unit,
) {
    val artifact = state.reviewArtifact
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.common_details)) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                state.activeNotebook?.let {
                    Text(stringResource(R.string.single_scanner_detail_notebook, it.displayName))
                }
                artifact?.let {
                    Text(stringResource(R.string.single_scanner_detail_page, it.captureRequest.pageId))
                    Text(stringResource(R.string.single_scanner_detail_pipeline, it.pipelineVersion))
                    val registered = state.registeredScan
                    if (registered == null) {
                        Text(stringResource(R.string.single_scanner_awaiting_registration))
                    } else {
                        Text(stringResource(R.string.single_scanner_saved_scan_id, registered.scanId))
                        Text(
                            stringResource(
                                R.string.single_scanner_saved_status,
                                registered.qualityStatus.name,
                            ),
                        )
                    }
                    it.identityWarning?.let { warning ->
                        Text(warning, color = MaterialTheme.colorScheme.error)
                    }
                    it.warnings.forEach { warning ->
                        Text(stringResource(R.string.single_scanner_detail_warning, warning.name))
                    }
                }
                when (val code = state.pageCodeStatus) {
                    is PageCodeUiStatus.Blocked -> Text(code.explanation)
                    is PageCodeUiStatus.Failed -> Text(code.explanation)
                    else -> Unit
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_close)) }
        },
    )
}

private fun manualWarningText(code: ManualCaptureWarningCode): String =
    when (code) {
        ManualCaptureWarningCode.BYPASSES_STABILITY_CHECK -> "Bypasses stable-frame timing"
        ManualCaptureWarningCode.CAPTURE_POLICY_NOT_ACCEPTED -> "Current capture quality is not accepted"
        ManualCaptureWarningCode.REPEATS_RECENT_PAGE -> "Repeats a recently captured page"
    }

private fun ScannerRgbImage.toBitmap(): Bitmap {
    val pixels = IntArray(Math.multiplyExact(width, height))
    var sourceIndex = 0
    for (index in pixels.indices) {
        val red = bytes[sourceIndex].toInt() and 0xff
        val green = bytes[sourceIndex + 1].toInt() and 0xff
        val blue = bytes[sourceIndex + 2].toInt() and 0xff
        pixels[index] = (0xff shl 24) or (red shl 16) or (green shl 8) or blue
        sourceIndex += 3
    }
    return Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888)
}
