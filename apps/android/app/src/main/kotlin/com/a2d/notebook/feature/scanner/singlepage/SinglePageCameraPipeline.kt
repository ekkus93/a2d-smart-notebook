package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.camera.CameraAnalysisEvent
import com.a2d.notebook.feature.scanner.camera.CameraXAdapter
import com.a2d.notebook.feature.scanner.camera.LiveCameraAnalysisPipeline
import com.a2d.notebook.feature.scanner.camera.LiveFrameAnalysisEvent
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeEvent
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeScheduler
import com.a2d.notebook.feature.scanner.camera.rememberCameraXAdapter

/**
 * Runs QR decoding immediately, but starts Rust marker/quality analysis only after Rust has resolved
 * the stored page's layout and processing policy for this scanner session.
 */
@Composable
@Suppress("UNUSED_PARAMETER")
fun rememberSinglePageCameraXAdapter(
    policy: SinglePageScannerPolicy,
    generation: Long,
    onLiveAnalysisEvent: (LiveFrameAnalysisEvent) -> Unit,
    onQrCodeEvent: (LiveQrCodeEvent) -> Unit,
    onStateChanged: (CameraAdapterState) -> Unit,
): CameraXAdapter {
    val currentLiveCallback = rememberUpdatedState(onLiveAnalysisEvent)
    val currentQrCallback = rememberUpdatedState(onQrCodeEvent)
    val livePolicy = RustScannerPolicySession.currentPolicy()?.liveAnalysisPolicy
    val livePipeline =
        remember(livePolicy, generation) {
            livePolicy?.let { resolved ->
                LiveCameraAnalysisPipeline.native(resolved) { event ->
                    currentLiveCallback.value(event)
                }
            }
        }
    val qrScheduler = remember(generation) {
        LiveQrCodeScheduler(onEvent = { event -> currentQrCallback.value(event) })
    }
    DisposableEffect(livePipeline, qrScheduler) {
        onDispose {
            livePipeline?.close()
            qrScheduler.close()
        }
    }

    return rememberCameraXAdapter(
        onAnalysisEvent = { event ->
            livePipeline?.onCameraEvent(event)
            if (event is CameraAnalysisEvent.Frame) qrScheduler.submit(event.frame)
        },
        onStateChanged = onStateChanged,
    )
}
