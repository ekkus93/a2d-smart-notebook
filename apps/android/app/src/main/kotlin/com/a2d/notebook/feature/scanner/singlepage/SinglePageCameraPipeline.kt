package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import com.a2d.notebook.feature.scanner.camera.CameraAnalysisEvent
import com.a2d.notebook.feature.scanner.camera.CameraPreviewSurface
import com.a2d.notebook.feature.scanner.camera.CameraXAdapter
import com.a2d.notebook.feature.scanner.camera.LiveCameraAnalysisPipeline
import com.a2d.notebook.feature.scanner.camera.LiveFrameAnalysisEvent
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeEvent
import com.a2d.notebook.feature.scanner.camera.LiveQrCodeScheduler
import com.a2d.notebook.feature.scanner.camera.rememberCameraXAdapter
import java.io.File

object BatchScannerTestTags {
    const val ROOT = "batch_scanner_root"
    const val DESTINATION = "batch_scanner_destination"
    const val COUNTS = "batch_scanner_counts"
    const val CAPTURE = "batch_scanner_capture"
    const val FINISH = "batch_scanner_finish"
    const val SUMMARY = "batch_scanner_summary"
}

/** Runs QR decoding before Rust has resolved the stored page policy. */
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
    val livePipeline = remember(livePolicy, generation) {
        livePolicy?.let { resolved ->
            LiveCameraAnalysisPipeline.native(resolved) { currentLiveCallback.value(it) }
        }
    }
    val qrScheduler = remember(generation) {
        LiveQrCodeScheduler(onEvent = { currentQrCallback.value(it) })
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

@Composable
internal fun BatchScannerScreen(
    onBack: () -> Unit,
    viewModel: BatchScannerViewModel = viewModel(),
) {
    val state by viewModel.state
    LaunchedEffect(state.storedPolicy) {
        state.storedPolicy?.let(RustScannerPolicySession::update)
    }
    val adapter = rememberSinglePageCameraXAdapter(
        policy = SinglePageScannerPolicies.V1,
        generation = state.cameraGeneration,
        onLiveAnalysisEvent = {},
        onQrCodeEvent = viewModel::onQrCodeEvent,
        onStateChanged = viewModel::onCameraStateChanged,
    )
    LaunchedEffect(adapter, state.pendingCapture) {
        state.pendingCapture?.let { pending ->
            adapter.capture(File(pending.stagingPath)) { viewModel.onCameraCaptureResult(pending, it) }
        }
    }
    Column(
        Modifier.fillMaxSize().padding(16.dp).testTag(BatchScannerTestTags.ROOT),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            state.lockedNotebook?.let { "Batch Notebook: ${it.displayName ?: it.id}" } ?: "No active Notebook",
            Modifier.testTag(BatchScannerTestTags.DESTINATION),
        )
        Text("Destination stays fixed until this batch is finished.")
        CameraPreviewSurface(adapter, Modifier.fillMaxWidth().height(360.dp))
        Text(state.pageCodeMessage)
        val session = state.session
        Text(
            "Saved ${session?.savedCount ?: 0u}; queued ${session?.queuedCount ?: 0u}; " +
                "review ${session?.reviewCount ?: 0u}; duplicates ${session?.entries?.count { it.duplicatePage } ?: 0}",
            Modifier.testTag(BatchScannerTestTags.COUNTS),
        )
        state.notice?.let { Text(it) }
        state.error?.let { Text(it) }
        Button(
            onClick = viewModel::requestCapture,
            enabled = state.canCapture,
            modifier = Modifier.fillMaxWidth().testTag(BatchScannerTestTags.CAPTURE),
        ) { Text("Capture next page") }
        Button(
            onClick = viewModel::finishBatch,
            enabled = state.canFinish,
            modifier = Modifier.fillMaxWidth().testTag(BatchScannerTestTags.FINISH),
        ) { Text("Finish batch") }
        state.completedSummary?.let { summary ->
            Text(
                "Batch complete: ${summary.savedCount} saved, ${summary.reviewCount} review, " +
                    "${summary.entries.count { it.duplicatePage }} duplicates.",
                Modifier.testTag(BatchScannerTestTags.SUMMARY),
            )
            Button(onClick = { viewModel.acknowledgeCompleted(onBack) }) { Text("Close") }
        }
        Button(onClick = onBack, enabled = !state.captureInProgress) { Text("Back") }
    }
}
