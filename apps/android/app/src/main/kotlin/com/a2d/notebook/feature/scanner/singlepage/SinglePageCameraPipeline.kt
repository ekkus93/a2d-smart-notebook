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
 * One CameraX adapter whose owned grayscale frames feed independent keep-latest Rust and ZXing
 * workers. Replacing [generation] closes both old workers, so results from a previous Notebook or
 * scan session cannot update the new destination.
 */
@Composable
fun rememberSinglePageCameraXAdapter(
    policy: SinglePageScannerPolicy,
    generation: Long,
    onLiveAnalysisEvent: (LiveFrameAnalysisEvent) -> Unit,
    onQrCodeEvent: (LiveQrCodeEvent) -> Unit,
    onStateChanged: (CameraAdapterState) -> Unit,
): CameraXAdapter {
    val currentLiveCallback = rememberUpdatedState(onLiveAnalysisEvent)
    val currentQrCallback = rememberUpdatedState(onQrCodeEvent)
    val livePipeline = remember(policy.liveAnalysis, generation) {
        LiveCameraAnalysisPipeline.native(policy.liveAnalysis) { event ->
            currentLiveCallback.value(event)
        }
    }
    val qrScheduler = remember(generation) {
        LiveQrCodeScheduler { event -> currentQrCallback.value(event) }
    }
    DisposableEffect(livePipeline, qrScheduler) {
        onDispose {
            livePipeline.close()
            qrScheduler.close()
        }
    }

    return rememberCameraXAdapter(
        onAnalysisEvent = { event ->
            livePipeline.onCameraEvent(event)
            if (event is CameraAnalysisEvent.Frame) qrScheduler.submit(event.frame)
        },
        onStateChanged = onStateChanged,
    )
}
