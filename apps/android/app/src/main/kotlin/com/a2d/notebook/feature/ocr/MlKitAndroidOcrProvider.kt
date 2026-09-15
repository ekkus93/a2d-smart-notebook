package com.a2d.notebook.feature.ocr

import android.content.Context
import android.graphics.Point
import android.graphics.Rect
import android.net.Uri
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.common.MlKitException
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.Text
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.TextRecognizer
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import java.io.File
import java.util.concurrent.ExecutionException

/**
 * On-device Android OCR provider backed by ML Kit's bundled Latin text-recognition model.
 *
 * The model is packaged in the APK (`com.google.mlkit:text-recognition`) rather than downloaded
 * from Google Play services. Rust still chooses and validates the durable scan asset; this adapter
 * only resolves that Rust-returned relative path below the app library root and performs local
 * recognition.
 */
class MlKitAndroidOcrProvider internal constructor(
    private val libraryRoot: File,
    private val engine: LocalTextRecognitionEngine,
    private val nowMs: () -> Long,
) : AndroidOcrProvider,
    AutoCloseable {
    override fun recognize(input: PreparedAndroidOcrInput): AndroidOcrRecognitionOutcome {
        val resolved = resolvePreparedInput(input)
        if (resolved is PreparedInputResolution.Unavailable) {
            return resolved.outcome
        }
        resolved as PreparedInputResolution.Ready

        val recognized =
            try {
                engine.recognize(resolved.file)
            } catch (failure: LocalTextRecognitionException) {
                return failure.toOutcome()
            } catch (failure: Exception) {
                return unavailable(
                    reason = OcrUnavailableReason.ProviderFailed,
                    message = failure.message ?: "Bundled ML Kit text recognition failed",
                    retryAvailable = true,
                )
            }

        val completedAtMs = nowMs()
        val fullText = recognized.fullText.trim()
        if (fullText.isEmpty()) {
            return AndroidOcrRecognitionOutcome.NoTextDetected(
                provider = PROVIDER_NAME,
                providerVersion = PROVIDER_VERSION,
                modelName = MODEL_NAME,
                completedAtMs = completedAtMs,
                warnings = recognized.warnings,
            )
        }

        return AndroidOcrRecognitionOutcome.Detected(
            provider = PROVIDER_NAME,
            providerVersion = PROVIDER_VERSION,
            modelName = MODEL_NAME,
            fullText = fullText,
            regions =
                recognized.regions.mapNotNull { region ->
                    region.toAndroidRegion(input, completedAtMs)
                },
            completedAtMs = completedAtMs,
            warnings = recognized.warnings,
        )
    }

    override fun close() {
        engine.close()
    }

    private fun resolvePreparedInput(input: PreparedAndroidOcrInput): PreparedInputResolution {
        if (!input.mediaType.startsWith("image/")) {
            return PreparedInputResolution.Unavailable(
                unavailable(
                    reason = OcrUnavailableReason.UnsupportedInput,
                    message = "Bundled ML Kit OCR requires an image input",
                    retryAvailable = false,
                ),
            )
        }
        if (input.relativePath.isBlank() || File(input.relativePath).isAbsolute) {
            return PreparedInputResolution.Unavailable(
                unavailable(
                    reason = OcrUnavailableReason.UnsupportedInput,
                    message = "Rust returned an invalid OCR input path",
                    retryAvailable = false,
                ),
            )
        }

        val root =
            try {
                libraryRoot.canonicalFile
            } catch (failure: Exception) {
                return PreparedInputResolution.Unavailable(
                    unavailable(
                        reason = OcrUnavailableReason.ResourceUnavailable,
                        message = failure.message ?: "OCR library root is unavailable",
                        retryAvailable = true,
                    ),
                )
            }
        val file =
            try {
                root.resolve(input.relativePath).canonicalFile
            } catch (failure: Exception) {
                return PreparedInputResolution.Unavailable(
                    unavailable(
                        reason = OcrUnavailableReason.ResourceUnavailable,
                        message = failure.message ?: "OCR input path cannot be resolved",
                        retryAvailable = true,
                    ),
                )
            }

        if (!file.toPath().startsWith(root.toPath())) {
            return PreparedInputResolution.Unavailable(
                unavailable(
                    reason = OcrUnavailableReason.UnsupportedInput,
                    message = "OCR input path escapes the Rust-owned library root",
                    retryAvailable = false,
                ),
            )
        }
        if (!file.isFile) {
            return PreparedInputResolution.Unavailable(
                unavailable(
                    reason = OcrUnavailableReason.ResourceUnavailable,
                    message = "Rust-selected OCR input asset is missing",
                    retryAvailable = false,
                ),
            )
        }
        if (file.length().toULong() != input.byteLength) {
            return PreparedInputResolution.Unavailable(
                unavailable(
                    reason = OcrUnavailableReason.ResourceUnavailable,
                    message = "Rust-selected OCR input asset length changed before recognition",
                    retryAvailable = false,
                ),
            )
        }
        return PreparedInputResolution.Ready(file)
    }

    private fun LocalTextRecognitionException.toOutcome(): AndroidOcrRecognitionOutcome =
        when (kind) {
            LocalTextRecognitionFailureKind.Cancelled ->
                AndroidOcrRecognitionOutcome.Cancelled(
                    message = message ?: "OCR recognition was cancelled",
                    provider = PROVIDER_NAME,
                    providerVersion = PROVIDER_VERSION,
                    modelName = MODEL_NAME,
                    completedAtMs = nowMs(),
                )

            LocalTextRecognitionFailureKind.ProviderUnavailable ->
                unavailable(
                    reason = OcrUnavailableReason.ProviderUnavailable,
                    message = message ?: "Bundled ML Kit OCR provider is unavailable",
                    retryAvailable = retryAvailable,
                )

            LocalTextRecognitionFailureKind.ResourceUnavailable ->
                unavailable(
                    reason = OcrUnavailableReason.ResourceUnavailable,
                    message = message ?: "Bundled ML Kit OCR resource is unavailable",
                    retryAvailable = retryAvailable,
                )

            LocalTextRecognitionFailureKind.UnsupportedInput ->
                unavailable(
                    reason = OcrUnavailableReason.UnsupportedInput,
                    message = message ?: "Bundled ML Kit cannot recognize this input",
                    retryAvailable = false,
                )

            LocalTextRecognitionFailureKind.ProviderFailed ->
                unavailable(
                    reason = OcrUnavailableReason.ProviderFailed,
                    message = message ?: "Bundled ML Kit text recognition failed",
                    retryAvailable = retryAvailable,
                )
        }

    private fun unavailable(
        reason: OcrUnavailableReason,
        message: String,
        retryAvailable: Boolean,
    ): AndroidOcrRecognitionOutcome.Unavailable =
        AndroidOcrRecognitionOutcome.Unavailable(
            reason = reason,
            message = message,
            retryAvailable = retryAvailable,
            provider = PROVIDER_NAME,
            providerVersion = PROVIDER_VERSION,
            modelName = MODEL_NAME,
            completedAtMs = nowMs(),
        )

    companion object {
        fun create(
            context: Context,
            libraryRoot: File,
        ): MlKitAndroidOcrProvider =
            MlKitAndroidOcrProvider(
                libraryRoot = libraryRoot,
                engine = BundledMlKitTextRecognitionEngine(context.applicationContext),
                nowMs = System::currentTimeMillis,
            )

        const val PROVIDER_NAME = "mlkit-bundled-text-recognition"
        const val PROVIDER_VERSION = "16.0.1"
        const val MODEL_NAME = "latin-v2-bundled"
    }
}

internal data class LocalTextRecognitionResult(
    val fullText: String,
    val regions: List<LocalTextRecognitionRegion>,
    val warnings: List<String> = emptyList(),
)

internal data class LocalTextRecognitionRegion(
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
)

internal enum class LocalTextRecognitionFailureKind {
    Cancelled,
    ProviderUnavailable,
    ResourceUnavailable,
    UnsupportedInput,
    ProviderFailed,
}

internal class LocalTextRecognitionException(
    val kind: LocalTextRecognitionFailureKind,
    message: String,
    val retryAvailable: Boolean,
    cause: Throwable? = null,
) : Exception(message, cause)

internal interface LocalTextRecognitionEngine : AutoCloseable {
    fun recognize(imageFile: File): LocalTextRecognitionResult

    override fun close() = Unit
}

private sealed interface PreparedInputResolution {
    data class Ready(val file: File) : PreparedInputResolution

    data class Unavailable(val outcome: AndroidOcrRecognitionOutcome.Unavailable) :
        PreparedInputResolution
}

private class BundledMlKitTextRecognitionEngine(
    private val context: Context,
    private val recognizer: TextRecognizer =
        TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS),
) : LocalTextRecognitionEngine {
    override fun recognize(imageFile: File): LocalTextRecognitionResult {
        val image =
            try {
                InputImage.fromFilePath(context, Uri.fromFile(imageFile))
            } catch (failure: Exception) {
                throw LocalTextRecognitionException(
                    kind = LocalTextRecognitionFailureKind.UnsupportedInput,
                    message = failure.message ?: "ML Kit could not decode the OCR input image",
                    retryAvailable = false,
                    cause = failure,
                )
            }

        val result =
            try {
                Tasks.await(recognizer.process(image))
            } catch (failure: InterruptedException) {
                Thread.currentThread().interrupt()
                throw LocalTextRecognitionException(
                    kind = LocalTextRecognitionFailureKind.Cancelled,
                    message = "ML Kit OCR wait was interrupted",
                    retryAvailable = false,
                    cause = failure,
                )
            } catch (failure: ExecutionException) {
                throw classifyMlKitFailure(failure.cause ?: failure)
            } catch (failure: Exception) {
                throw classifyMlKitFailure(failure)
            }

        return LocalTextRecognitionResult(
            fullText = result.text,
            regions =
                result.textBlocks.flatMap { block ->
                    block.lines.mapNotNull { line -> line.toLocalRegion() }
                },
        )
    }

    override fun close() {
        recognizer.close()
    }
}

private fun Text.Line.toLocalRegion(): LocalTextRecognitionRegion? {
    val regionText = text.trim()
    if (regionText.isEmpty()) return null
    val points = cornerPoints?.toList() ?: boundingBox?.toCornerPoints() ?: return null
    if (points.size < 3) return null
    return LocalTextRecognitionRegion(
        polygon = points.map { point -> OcrTextPoint(point.x.toFloat(), point.y.toFloat()) },
        text = regionText,
        confidence = confidence.takeIf { it.isFinite() }?.coerceIn(0f, 1f),
    )
}

private fun Rect.toCornerPoints(): List<Point> =
    listOf(
        Point(left, top),
        Point(right, top),
        Point(right, bottom),
        Point(left, bottom),
    )

private fun LocalTextRecognitionRegion.toAndroidRegion(
    input: PreparedAndroidOcrInput,
    createdAtMs: Long,
): AndroidRecognizedTextRegion? {
    val regionText = text.trim()
    if (regionText.isEmpty() || polygon.size !in 3..8) return null
    val maxX = input.widthPx.toFloat()
    val maxY = input.heightPx.toFloat()
    val normalized =
        polygon.mapNotNull { point ->
            if (!point.x.isFinite() || !point.y.isFinite()) {
                null
            } else {
                OcrTextPoint(
                    x = point.x.coerceIn(0f, maxX),
                    y = point.y.coerceIn(0f, maxY),
                )
            }
        }
    if (normalized.size != polygon.size) return null
    return AndroidRecognizedTextRegion(
        polygon = normalized,
        text = regionText,
        confidence = confidence?.takeIf { it.isFinite() }?.coerceIn(0f, 1f),
        createdAtMs = createdAtMs,
    )
}

internal fun classifyMlKitFailure(failure: Throwable): LocalTextRecognitionException {
    if (failure is LocalTextRecognitionException) return failure
    if (failure !is MlKitException) {
        return LocalTextRecognitionException(
            kind = LocalTextRecognitionFailureKind.ProviderFailed,
            message = failure.message ?: "Bundled ML Kit text recognition failed",
            retryAvailable = true,
            cause = failure,
        )
    }
    return classifyMlKitErrorCode(
        errorCode = failure.errorCode,
        message = failure.message ?: "Bundled ML Kit text recognition failed",
        cause = failure,
    )
}

internal fun classifyMlKitErrorCode(
    errorCode: Int,
    message: String = "Bundled ML Kit text recognition failed",
    cause: Throwable? = null,
): LocalTextRecognitionException {
    val kind =
        when (errorCode) {
            MlKitException.CANCELLED -> LocalTextRecognitionFailureKind.Cancelled
            MlKitException.UNAVAILABLE -> LocalTextRecognitionFailureKind.ProviderUnavailable
            MlKitException.UNSUPPORTED,
            MlKitException.UNIMPLEMENTED,
            MlKitException.INVALID_ARGUMENT,
            -> LocalTextRecognitionFailureKind.UnsupportedInput

            MlKitException.RESOURCE_EXHAUSTED,
            MlKitException.NOT_ENOUGH_SPACE,
            MlKitException.NOT_FOUND,
            -> LocalTextRecognitionFailureKind.ResourceUnavailable

            else -> LocalTextRecognitionFailureKind.ProviderFailed
        }
    val retryAvailable =
        when (kind) {
            LocalTextRecognitionFailureKind.Cancelled,
            LocalTextRecognitionFailureKind.UnsupportedInput,
            -> false

            LocalTextRecognitionFailureKind.ProviderUnavailable,
            LocalTextRecognitionFailureKind.ResourceUnavailable,
            LocalTextRecognitionFailureKind.ProviderFailed,
            -> true
        }
    return LocalTextRecognitionException(
        kind = kind,
        message = message,
        retryAvailable = retryAvailable,
        cause = cause,
    )
}
