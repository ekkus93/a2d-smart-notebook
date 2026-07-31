package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeEvent
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.resolveStoredScanPolicy
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.PageResolution

internal const val SMART_PAGE_SCANNER_DEFERRED_MESSAGE =
    "Smart Page scanning is not available in the v0.1 Notebook Page scanner."

@Composable
internal fun rememberPolicyAwareQrEventHandler(
    viewModel: SinglePageScannerViewModel,
    activeNotebookId: String?,
    generation: Long,
): (LiveQrCodeEvent) -> Unit {
    val applicationContext = LocalContext.current.applicationContext
    val client = remember(applicationContext) { A2dBridge.client(applicationContext) }
    val scope = rememberCoroutineScope()
    val currentNotebookId = rememberUpdatedState(activeNotebookId)
    val currentGeneration = rememberUpdatedState(generation)
    val sequence = remember { AtomicLong(0L) }

    DisposableEffect(activeNotebookId, generation) {
        sequence.incrementAndGet()
        RustScannerPolicySession.clear()
        onDispose {
            sequence.incrementAndGet()
            RustScannerPolicySession.clear()
        }
    }

    return remember(viewModel, client, scope) {
        { event ->
            when (event) {
                is LiveQrCodeEvent.Found -> {
                    val requestSequence = sequence.incrementAndGet()
                    val notebookId = currentNotebookId.value
                    val requestGeneration = currentGeneration.value
                    if (notebookId == null) {
                        RustScannerPolicySession.clear()
                        viewModel.onQrCodeEvent(event)
                    } else {
                        scope.launch {
                            try {
                                val policy =
                                    withContext(Dispatchers.IO) {
                                        when (
                                            val resolution =
                                                client.resolvePageCode(event.payload, notebookId)
                                        ) {
                                            is PageResolution.Resolved -> {
                                                if (resolution.notebookId == null) {
                                                    throw SmartPageScannerDeferredException()
                                                }
                                                client.resolveStoredScanPolicy(resolution.pageId)
                                            }

                                            is PageResolution.ImportedUnknownSmartPage ->
                                                throw SmartPageScannerDeferredException()

                                            else -> null
                                        }
                                    }
                                if (
                                    requestSequence != sequence.get() ||
                                        requestGeneration != currentGeneration.value ||
                                        notebookId != currentNotebookId.value
                                ) {
                                    return@launch
                                }
                                if (policy == null) {
                                    RustScannerPolicySession.clear()
                                } else {
                                    RustScannerPolicySession.update(policy)
                                }
                                viewModel.onQrCodeEvent(event)
                            } catch (failure: CancellationException) {
                                throw failure
                            } catch (failure: Exception) {
                                if (requestSequence != sequence.get()) return@launch
                                RustScannerPolicySession.clear()
                                viewModel.onQrCodeEvent(
                                    LiveQrCodeEvent.Failed(
                                        frameSequence = event.frameSequence,
                                        frameTimestampNanos = event.frameTimestampNanos,
                                        message =
                                            failure.message
                                                ?: "Rust scan policy resolution failed",
                                        cause = failure,
                                    ),
                                )
                            }
                        }
                    }
                }

                is LiveQrCodeEvent.Failed,
                is LiveQrCodeEvent.SubmissionRejected,
                LiveQrCodeEvent.Closed,
                -> {
                    sequence.incrementAndGet()
                    RustScannerPolicySession.clear()
                    viewModel.onQrCodeEvent(event)
                }

                else -> viewModel.onQrCodeEvent(event)
            }
        }
    }
}

private class SmartPageScannerDeferredException : IllegalStateException(
    SMART_PAGE_SCANNER_DEFERRED_MESSAGE,
)
