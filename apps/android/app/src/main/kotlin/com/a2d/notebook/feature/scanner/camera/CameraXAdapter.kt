package com.a2d.notebook.feature.scanner.camera

import android.content.Context
import android.net.Uri
import android.view.Surface
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.core.UseCase
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.core.content.ContextCompat
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import java.io.File
import java.util.concurrent.Executor
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.ThreadFactory
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

sealed interface CameraAdapterState {
    data object Idle : CameraAdapterState
    data object Initializing : CameraAdapterState

    data class Bound(
        val torchAvailable: Boolean,
        val torchEnabled: Boolean,
    ) : CameraAdapterState

    data object Unbound : CameraAdapterState

    data class Error(
        val message: String,
        val cause: Exception,
    ) : CameraAdapterState

    data object Closed : CameraAdapterState
}

sealed interface CameraCaptureResult {
    data class Captured(
        val file: File,
        val savedUri: Uri?,
    ) : CameraCaptureResult

    data class Failure(
        val message: String,
        val cause: Exception?,
    ) : CameraCaptureResult
}

private fun cameraThreadFactory(name: String): ThreadFactory = ThreadFactory { runnable ->
    Thread(runnable, name).apply { isDaemon = true }
}

/**
 * Owns CameraX Preview, ImageAnalysis, and ImageCapture as one lifecycle-bound adapter.
 *
 * The adapter never performs canonical identity or save registration. Analysis frames are copied
 * into owned luminance buffers and delivered to the caller, while full-resolution capture writes
 * only to a caller-selected staging file. Rust remains responsible for final validation and durable
 * registration in the later scanning milestone.
 */
class CameraXAdapter(
    context: Context,
    private val lifecycleOwner: LifecycleOwner,
    private val cameraSelector: CameraSelector = CameraSelector.DEFAULT_BACK_CAMERA,
    private val onAnalysisEvent: (CameraAnalysisEvent) -> Unit,
    private val onStateChanged: (CameraAdapterState) -> Unit,
    private val mainExecutor: Executor = ContextCompat.getMainExecutor(context),
    private val analysisExecutor: ExecutorService = Executors.newSingleThreadExecutor(
        cameraThreadFactory("a2d-camera-analysis"),
    ),
) : AutoCloseable {
    private val applicationContext = context.applicationContext
    private val closeRequested = AtomicBoolean(false)
    private val bindGeneration = AtomicLong(0)
    private val lifecycleObserver = object : DefaultLifecycleObserver {
        override fun onDestroy(owner: LifecycleOwner) {
            close()
        }
    }

    private var provider: ProcessCameraProvider? = null
    private var camera: Camera? = null
    private var preview: Preview? = null
    private var imageAnalysis: ImageAnalysis? = null
    private var imageCapture: ImageCapture? = null
    private var torchEnabled = false

    init {
        lifecycleOwner.lifecycle.addObserver(lifecycleObserver)
        publish(CameraAdapterState.Idle)
    }

    fun bind(
        surfaceProvider: Preview.SurfaceProvider,
        targetRotation: Int,
    ) {
        mainExecutor.execute {
            if (closeRequested.get()) {
                publishClosedFailure("cannot bind a closed CameraX adapter")
                return@execute
            }
            if (!isValidRotation(targetRotation)) {
                publishFailure(
                    IllegalArgumentException("invalid CameraX target rotation: $targetRotation"),
                )
                return@execute
            }

            val generation = bindGeneration.incrementAndGet()
            publish(CameraAdapterState.Initializing)
            val providerFuture = ProcessCameraProvider.getInstance(applicationContext)
            providerFuture.addListener(
                {
                    if (closeRequested.get() || generation != bindGeneration.get()) {
                        return@addListener
                    }

                    var pendingAnalysis: ImageAnalysis? = null
                    try {
                        val resolvedProvider = providerFuture.get()
                        val newPreview = Preview.Builder()
                            .setTargetRotation(targetRotation)
                            .build()
                            .also { it.setSurfaceProvider(surfaceProvider) }
                        val newAnalysis = ImageAnalysis.Builder()
                            .setTargetRotation(targetRotation)
                            .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_YUV_420_888)
                            .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                            .build()
                            .also { analysis ->
                                pendingAnalysis = analysis
                                analysis.setAnalyzer(
                                    analysisExecutor,
                                    CameraFrameAnalyzer { event ->
                                        if (
                                            !closeRequested.get() &&
                                            generation == bindGeneration.get()
                                        ) {
                                            onAnalysisEvent(event)
                                        }
                                    },
                                )
                            }
                        val newCapture = ImageCapture.Builder()
                            .setTargetRotation(targetRotation)
                            .setCaptureMode(ImageCapture.CAPTURE_MODE_MAXIMIZE_QUALITY)
                            .build()

                        imageAnalysis?.clearAnalyzer()
                        unbindUseCases(resolvedProvider)
                        clearBoundReferences()

                        val boundCamera = resolvedProvider.bindToLifecycle(
                            lifecycleOwner,
                            cameraSelector,
                            newPreview,
                            newAnalysis,
                            newCapture,
                        )
                        provider = resolvedProvider
                        camera = boundCamera
                        preview = newPreview
                        imageAnalysis = newAnalysis
                        imageCapture = newCapture
                        torchEnabled = false

                        if (closeRequested.get() || generation != bindGeneration.get()) {
                            return@addListener
                        }
                        publish(
                            CameraAdapterState.Bound(
                                torchAvailable = boundCamera.cameraInfo.hasFlashUnit(),
                                torchEnabled = false,
                            ),
                        )
                    } catch (failure: Exception) {
                        pendingAnalysis?.clearAnalyzer()
                        publishFailure(failure)
                    }
                },
                mainExecutor,
            )
        }
    }

    fun updateTargetRotation(targetRotation: Int) {
        mainExecutor.execute {
            if (closeRequested.get()) {
                publishClosedFailure("cannot update rotation on a closed CameraX adapter")
                return@execute
            }
            if (!isValidRotation(targetRotation)) {
                publishFailure(
                    IllegalArgumentException("invalid CameraX target rotation: $targetRotation"),
                )
                return@execute
            }
            preview?.targetRotation = targetRotation
            imageAnalysis?.targetRotation = targetRotation
            imageCapture?.targetRotation = targetRotation
        }
    }

    fun setTorch(enabled: Boolean) {
        mainExecutor.execute {
            if (closeRequested.get()) {
                publishClosedFailure("cannot control torch on a closed CameraX adapter")
                return@execute
            }
            val boundCamera = camera
            if (boundCamera == null) {
                publishFailure(IllegalStateException("camera is not bound"))
                return@execute
            }
            if (enabled && !boundCamera.cameraInfo.hasFlashUnit()) {
                publishFailure(UnsupportedOperationException("selected camera has no torch"))
                return@execute
            }

            val generation = bindGeneration.get()
            val request = boundCamera.cameraControl.enableTorch(enabled)
            request.addListener(
                {
                    if (
                        closeRequested.get() ||
                        generation != bindGeneration.get() ||
                        camera !== boundCamera
                    ) {
                        return@addListener
                    }
                    try {
                        request.get()
                        torchEnabled = enabled
                        publish(
                            CameraAdapterState.Bound(
                                torchAvailable = boundCamera.cameraInfo.hasFlashUnit(),
                                torchEnabled = enabled,
                            ),
                        )
                    } catch (failure: Exception) {
                        publishFailure(failure)
                    }
                },
                mainExecutor,
            )
        }
    }

    /**
     * Captures a full-resolution image to a new staging file. Existing files are rejected so a
     * capture can never silently overwrite an original or prior staged capture.
     */
    fun capture(
        outputFile: File,
        callback: (CameraCaptureResult) -> Unit,
    ) {
        mainExecutor.execute {
            if (closeRequested.get()) {
                callback(
                    CameraCaptureResult.Failure(
                        "cannot capture from a closed CameraX adapter",
                        null,
                    ),
                )
                return@execute
            }
            val capture = imageCapture
            if (capture == null) {
                callback(CameraCaptureResult.Failure("camera is not bound", null))
                return@execute
            }
            if (outputFile.exists()) {
                callback(
                    CameraCaptureResult.Failure(
                        "capture staging file already exists",
                        null,
                    ),
                )
                return@execute
            }
            val parent = outputFile.parentFile
            if (parent == null || (!parent.exists() && !parent.mkdirs())) {
                callback(
                    CameraCaptureResult.Failure(
                        "capture staging directory could not be created",
                        null,
                    ),
                )
                return@execute
            }

            val options = ImageCapture.OutputFileOptions.Builder(outputFile).build()
            try {
                capture.takePicture(
                    options,
                    mainExecutor,
                    object : ImageCapture.OnImageSavedCallback {
                        override fun onImageSaved(output: ImageCapture.OutputFileResults) {
                            callback(CameraCaptureResult.Captured(outputFile, output.savedUri))
                        }

                        override fun onError(exception: ImageCaptureException) {
                            callback(
                                CameraCaptureResult.Failure(
                                    exception.message ?: "CameraX image capture failed",
                                    exception,
                                ),
                            )
                        }
                    },
                )
            } catch (failure: Exception) {
                callback(
                    CameraCaptureResult.Failure(
                        failure.message ?: "CameraX image capture failed",
                        failure,
                    ),
                )
            }
        }
    }

    fun unbind() {
        mainExecutor.execute { unbindOnMain() }
    }

    override fun close() {
        if (!closeRequested.compareAndSet(false, true)) {
            return
        }
        bindGeneration.incrementAndGet()
        mainExecutor.execute {
            lifecycleOwner.lifecycle.removeObserver(lifecycleObserver)

            var cleanupFailure: Exception? = null
            try {
                imageAnalysis?.clearAnalyzer()
                provider?.let(::unbindUseCases)
            } catch (failure: Exception) {
                cleanupFailure = failure
            }

            provider = null
            clearBoundReferences()
            analysisExecutor.shutdownNow()

            cleanupFailure?.let(::publishFailure)
            publish(CameraAdapterState.Closed)
        }
    }

    private fun unbindOnMain() {
        if (closeRequested.get()) return
        bindGeneration.incrementAndGet()
        try {
            imageAnalysis?.clearAnalyzer()
            provider?.let(::unbindUseCases)
            clearBoundReferences()
            publish(CameraAdapterState.Unbound)
        } catch (failure: Exception) {
            publishFailure(failure)
        }
    }

    private fun clearBoundReferences() {
        camera = null
        preview = null
        imageAnalysis = null
        imageCapture = null
        torchEnabled = false
    }

    private fun unbindUseCases(cameraProvider: ProcessCameraProvider) {
        val useCases = listOfNotNull<UseCase>(preview, imageAnalysis, imageCapture)
        if (useCases.isNotEmpty()) {
            cameraProvider.unbind(*useCases.toTypedArray())
        }
    }

    private fun publishFailure(cause: Exception) {
        publish(
            CameraAdapterState.Error(
                message = cause.message ?: "CameraX operation failed",
                cause = cause,
            ),
        )
    }

    private fun publishClosedFailure(message: String) {
        publishFailure(IllegalStateException(message))
    }

    private fun publish(state: CameraAdapterState) {
        onStateChanged(state)
    }

    private fun isValidRotation(rotation: Int): Boolean = rotation == Surface.ROTATION_0 ||
        rotation == Surface.ROTATION_90 ||
        rotation == Surface.ROTATION_180 ||
        rotation == Surface.ROTATION_270
}
