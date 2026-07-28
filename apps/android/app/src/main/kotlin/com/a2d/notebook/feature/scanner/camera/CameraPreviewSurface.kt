package com.a2d.notebook.feature.scanner.camera

import android.view.Surface
import androidx.camera.view.PreviewView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.doOnAttach
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.a2d.notebook.rustbridge.LivePageAnalysisPolicy

/** Creates one adapter per lifecycle owner and closes its executor when composition is disposed. */
@Composable
fun rememberCameraXAdapter(
    onAnalysisEvent: (CameraAnalysisEvent) -> Unit,
    onStateChanged: (CameraAdapterState) -> Unit,
): CameraXAdapter {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val currentAnalysisCallback = rememberUpdatedState(onAnalysisEvent)
    val currentStateCallback = rememberUpdatedState(onStateChanged)
    val adapter = remember(context.applicationContext, lifecycleOwner) {
        CameraXAdapter(
            context = context.applicationContext,
            lifecycleOwner = lifecycleOwner,
            onAnalysisEvent = { currentAnalysisCallback.value(it) },
            onStateChanged = { currentStateCallback.value(it) },
        )
    }

    DisposableEffect(adapter) {
        onDispose { adapter.close() }
    }
    return adapter
}

/**
 * Creates the complete CameraX -> keep-latest scheduler -> borrowed Rust analysis path.
 *
 * A policy change closes and replaces the old scheduler so an in-flight result from the previous
 * policy cannot update current scanner state. The underlying CameraX adapter remains lifecycle-owned
 * and switches to the new pipeline callback through [rememberUpdatedState].
 */
@Composable
fun rememberLiveAnalysisCameraXAdapter(
    policy: LivePageAnalysisPolicy,
    onLiveAnalysisEvent: (LiveFrameAnalysisEvent) -> Unit,
    onStateChanged: (CameraAdapterState) -> Unit,
): CameraXAdapter {
    val currentLiveAnalysisCallback = rememberUpdatedState(onLiveAnalysisEvent)
    val pipeline = remember(policy) {
        LiveCameraAnalysisPipeline.native(policy) { event ->
            currentLiveAnalysisCallback.value(event)
        }
    }
    DisposableEffect(pipeline) {
        onDispose { pipeline.close() }
    }
    return rememberCameraXAdapter(
        onAnalysisEvent = pipeline::onCameraEvent,
        onStateChanged = onStateChanged,
    )
}

/**
 * Hosts CameraX's [PreviewView] inside Compose. Binding is performed once after each platform view
 * is attached, so the initial display rotation is authoritative without leaving a delayed posted
 * bind that can outlive disposal. Recomposition only updates target rotation. Lifecycle stop/start
 * behavior is owned by CameraX's lifecycle binding, and disposal explicitly unbinds the adapter.
 */
@Composable
fun CameraPreviewSurface(
    adapter: CameraXAdapter,
    modifier: Modifier = Modifier,
) {
    DisposableEffect(adapter) {
        onDispose { adapter.unbind() }
    }

    AndroidView(
        modifier = modifier,
        factory = { context ->
            PreviewView(context).apply {
                implementationMode = PreviewView.ImplementationMode.COMPATIBLE
                scaleType = PreviewView.ScaleType.FILL_CENTER
                doOnAttach { attachedView ->
                    adapter.bind(
                        surfaceProvider = surfaceProvider,
                        targetRotation = attachedView.display?.rotation ?: Surface.ROTATION_0,
                    )
                }
            }
        },
        update = { previewView ->
            adapter.updateTargetRotation(
                previewView.display?.rotation ?: Surface.ROTATION_0,
            )
        },
    )
}