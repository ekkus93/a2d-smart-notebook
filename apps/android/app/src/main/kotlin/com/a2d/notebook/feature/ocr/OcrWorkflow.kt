package com.a2d.notebook.feature.ocr

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind
import uniffi.a2d_ffi.OcrRunStatus as FfiOcrRunStatus
import uniffi.a2d_ffi.OcrTextPoint as FfiOcrTextPoint
import uniffi.a2d_ffi.OcrUnavailableReason as FfiOcrUnavailableReason
import uniffi.a2d_ffi.PrepareOcrInputRequest as FfiPrepareOcrInputRequest
import uniffi.a2d_ffi.RecordOcrRunRequest as FfiRecordOcrRunRequest
import uniffi.a2d_ffi.RecordOcrTextRegionRequest as FfiRecordOcrTextRegionRequest
import uniffi.a2d_ffi.RecordOcrTextRegionsRequest as FfiRecordOcrTextRegionsRequest

/**
 * Android-side OCR orchestration around the Rust-owned Milestone 11 OCR APIs.
 *
 * Platform OCR providers may recognize text from the prepared image path, but they do not choose
 * scan identity, asset identity, or SQL-shaped result rows. Those remain delegated to Rust through
 * [RustOcrGateway].
 */
class AndroidOcrWorkflow(
    private val gateway: RustOcrGateway,
    private val provider: AndroidOcrProvider,
) {
    fun run(request: AndroidOcrStartRequest): AndroidOcrWorkflowResult {
        val prepared =
            try {
                gateway.prepareOcrInput(request)
            } catch (failure: Exception) {
                return AndroidOcrWorkflowResult.failed(
                    status = OcrPresentationStatus.Failed,
                    message = failure.message ?: "Rust could not prepare an OCR input",
                )
            }

        val outcome =
            try {
                provider.recognize(prepared)
            } catch (failure: Exception) {
                AndroidOcrRecognitionOutcome.Unavailable(
                    reason = OcrUnavailableReason.ProviderFailed,
                    message = failure.message ?: "OCR provider failed before returning text",
                    retryAvailable = true,
                )
            }

        val recordRequest = outcome.toRecordRequest(request.scanId, prepared)
        val recorded =
            try {
                gateway.recordOcrRun(recordRequest)
            } catch (failure: Exception) {
                return AndroidOcrWorkflowResult.failed(
                    status = OcrPresentationStatus.Failed,
                    preparedInput = prepared,
                    message = failure.message ?: "Rust rejected the OCR result",
                )
            }

        val recordedRegions =
            if (outcome is AndroidOcrRecognitionOutcome.Detected && outcome.regions.isNotEmpty()) {
                try {
                    gateway.recordOcrTextRegions(
                        AndroidRecordOcrTextRegionsRequest(
                            ocrRunId = recorded.ocrRunId,
                            sourceImageWidth = prepared.widthPx,
                            sourceImageHeight = prepared.heightPx,
                            regions = outcome.regions.map { it.toRecordRequest() },
                        ),
                    )
                } catch (failure: Exception) {
                    return AndroidOcrWorkflowResult.failed(
                        status = OcrPresentationStatus.Failed,
                        preparedInput = prepared,
                        recordedRun = recorded,
                        message = failure.message ?: "Rust rejected the OCR text regions",
                    )
                }
            } else {
                null
            }

        return when (outcome) {
            is AndroidOcrRecognitionOutcome.Detected ->
                AndroidOcrWorkflowResult(
                    status = OcrPresentationStatus.Detected,
                    preparedInput = prepared,
                    recordedRun = recorded,
                    recordedTextRegions = recordedRegions,
                    textPreview = outcome.fullText.take(TEXT_PREVIEW_LIMIT),
                    providerLabel = outcome.provider,
                    modelName = outcome.modelName,
                    recognizedRegionCount = recordedRegions?.regions?.size ?: 0,
                )

            is AndroidOcrRecognitionOutcome.NoTextDetected ->
                AndroidOcrWorkflowResult(
                    status = OcrPresentationStatus.NoTextDetected,
                    preparedInput = prepared,
                    recordedRun = recorded,
                    providerLabel = outcome.provider,
                    modelName = outcome.modelName,
                )

            is AndroidOcrRecognitionOutcome.Unavailable ->
                AndroidOcrWorkflowResult(
                    status = OcrPresentationStatus.Unavailable,
                    preparedInput = prepared,
                    recordedRun = recorded,
                    unavailableReason = outcome.reason,
                    message = outcome.message,
                    retryAvailable = outcome.retryAvailable,
                )

            is AndroidOcrRecognitionOutcome.Cancelled ->
                AndroidOcrWorkflowResult(
                    status = OcrPresentationStatus.Cancelled,
                    preparedInput = prepared,
                    recordedRun = recorded,
                    unavailableReason = OcrUnavailableReason.Cancelled,
                    message = outcome.message,
                    retryAvailable = false,
                )
        }
    }
}

interface AndroidOcrProvider {
    fun recognize(input: PreparedAndroidOcrInput): AndroidOcrRecognitionOutcome
}

interface RustOcrGateway {
    fun prepareOcrInput(request: AndroidOcrStartRequest): PreparedAndroidOcrInput

    fun recordOcrRun(request: AndroidRecordOcrRunRequest): RecordedAndroidOcrRun

    fun recordOcrTextRegions(request: AndroidRecordOcrTextRegionsRequest): RecordedAndroidOcrTextRegions
}

class FfiRustOcrGateway(private val client: A2dClient) : RustOcrGateway {
    override fun prepareOcrInput(request: AndroidOcrStartRequest): PreparedAndroidOcrInput {
        val prepared =
            client.prepareOcrInput(
                FfiPrepareOcrInputRequest(
                    scanId = request.scanId,
                    inputKind = request.inputKind.toFfi(),
                    widthPx = request.widthPx,
                    heightPx = request.heightPx,
                ),
            )
        return PreparedAndroidOcrInput(
            scanId = prepared.scanId,
            inputAssetId = prepared.inputAssetId,
            inputKind = prepared.inputKind.toAndroid(),
            mediaType = prepared.mediaType,
            relativePath = prepared.relativePath,
            byteLength = prepared.byteLength,
            widthPx = prepared.widthPx,
            heightPx = prepared.heightPx,
        )
    }

    override fun recordOcrRun(request: AndroidRecordOcrRunRequest): RecordedAndroidOcrRun {
        val recorded =
            client.recordOcrRun(
                FfiRecordOcrRunRequest(
                    scanId = request.scanId,
                    inputAssetId = request.inputAssetId,
                    provider = request.provider,
                    providerVersion = request.providerVersion,
                    modelName = request.modelName,
                    status = request.status.toFfi(),
                    fullText = request.fullText,
                    unavailableReason = request.unavailableReason?.toFfi(),
                    unavailableMessage = request.unavailableMessage,
                    completedAtMs = request.completedAtMs,
                    warnings = request.warnings,
                ),
            )
        return RecordedAndroidOcrRun(
            ocrRunId = recorded.ocrRunId,
            scanId = recorded.scanId,
            inputAssetId = recorded.inputAssetId,
            status = recorded.status.toAndroid(),
        )
    }

    override fun recordOcrTextRegions(
        request: AndroidRecordOcrTextRegionsRequest,
    ): RecordedAndroidOcrTextRegions {
        val recorded =
            client.recordOcrTextRegions(
                FfiRecordOcrTextRegionsRequest(
                    ocrRunId = request.ocrRunId,
                    sourceImageWidth = request.sourceImageWidth,
                    sourceImageHeight = request.sourceImageHeight,
                    regions = request.regions.map { it.toFfi() },
                ),
            )
        return RecordedAndroidOcrTextRegions(
            ocrRunId = recorded.ocrRunId,
            regions =
                recorded.regions.map { region ->
                    RecordedAndroidOcrTextRegion(
                        textRegionId = region.textRegionId,
                        ocrRunId = region.ocrRunId,
                        text = region.text,
                    )
                },
        )
    }
}

data class AndroidOcrStartRequest(
    val scanId: String,
    val inputKind: OcrInputKind,
    val widthPx: UInt,
    val heightPx: UInt,
)

data class PreparedAndroidOcrInput(
    val scanId: String,
    val inputAssetId: String,
    val inputKind: OcrInputKind,
    val mediaType: String,
    val relativePath: String,
    val byteLength: ULong,
    val widthPx: UInt,
    val heightPx: UInt,
)

data class AndroidRecordOcrRunRequest(
    val scanId: String,
    val inputAssetId: String,
    val provider: String,
    val providerVersion: String,
    val modelName: String?,
    val status: OcrRunStatus,
    val fullText: String,
    val unavailableReason: OcrUnavailableReason?,
    val unavailableMessage: String?,
    val completedAtMs: Long?,
    val warnings: List<String>,
)

data class RecordedAndroidOcrRun(
    val ocrRunId: String,
    val scanId: String,
    val inputAssetId: String,
    val status: OcrRunStatus,
)

data class OcrTextPoint(
    val x: Float,
    val y: Float,
)

data class AndroidRecognizedTextRegion(
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
    val createdAtMs: Long?,
)

data class AndroidRecordOcrTextRegionRequest(
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
    val createdAtMs: Long?,
)

data class AndroidRecordOcrTextRegionsRequest(
    val ocrRunId: String,
    val sourceImageWidth: UInt,
    val sourceImageHeight: UInt,
    val regions: List<AndroidRecordOcrTextRegionRequest>,
)

data class RecordedAndroidOcrTextRegion(
    val textRegionId: String,
    val ocrRunId: String,
    val text: String,
)

data class RecordedAndroidOcrTextRegions(
    val ocrRunId: String,
    val regions: List<RecordedAndroidOcrTextRegion>,
)

data class AndroidOcrWorkflowResult(
    val status: OcrPresentationStatus,
    val preparedInput: PreparedAndroidOcrInput? = null,
    val recordedRun: RecordedAndroidOcrRun? = null,
    val recordedTextRegions: RecordedAndroidOcrTextRegions? = null,
    val textPreview: String? = null,
    val providerLabel: String? = null,
    val modelName: String? = null,
    val recognizedRegionCount: Int = 0,
    val unavailableReason: OcrUnavailableReason? = null,
    val message: String? = null,
    val retryAvailable: Boolean = false,
) {
    fun toPresentationState(): OcrPresentationState =
        OcrPresentationState(
            status = status,
            runId = recordedRun?.ocrRunId,
            providerLabel = providerLabel,
            modelName = modelName,
            textPreview = textPreview,
            recognizedRegionCount = recognizedRegionCount,
            unavailableReason = unavailableReason?.label,
            message = message,
            retryAvailable = retryAvailable,
            cancelAvailable = status == OcrPresentationStatus.Preparing ||
                status == OcrPresentationStatus.Recognizing ||
                status == OcrPresentationStatus.Recording,
        )

    companion object {
        fun failed(
            status: OcrPresentationStatus,
            message: String,
            preparedInput: PreparedAndroidOcrInput? = null,
            recordedRun: RecordedAndroidOcrRun? = null,
        ): AndroidOcrWorkflowResult =
            AndroidOcrWorkflowResult(
                status = status,
                preparedInput = preparedInput,
                recordedRun = recordedRun,
                message = message,
                retryAvailable = true,
            )
    }
}

data class OcrPresentationState(
    val status: OcrPresentationStatus = OcrPresentationStatus.NotStarted,
    val runId: String? = null,
    val providerLabel: String? = null,
    val modelName: String? = null,
    val textPreview: String? = null,
    val recognizedRegionCount: Int = 0,
    val unavailableReason: String? = null,
    val message: String? = null,
    val retryAvailable: Boolean = false,
    val cancelAvailable: Boolean = false,
)

enum class OcrPresentationStatus {
    NotStarted,
    Preparing,
    Recognizing,
    Recording,
    Detected,
    NoTextDetected,
    Unavailable,
    Failed,
    Cancelled,
}

enum class OcrInputKind {
    Original,
    Corrected,
    OcrOptimized,
}

enum class OcrRunStatus {
    Detected,
    NoTextDetected,
    Unavailable,
}

enum class OcrUnavailableReason(val label: String) {
    ProviderUnavailable("provider unavailable"),
    ProviderFailed("provider failed"),
    ResourceUnavailable("resource unavailable"),
    UnsupportedInput("unsupported input"),
    Cancelled("cancelled"),
}

sealed class AndroidOcrRecognitionOutcome {
    abstract val provider: String
    abstract val providerVersion: String
    abstract val modelName: String?
    abstract val completedAtMs: Long?
    abstract val warnings: List<String>

    data class Detected(
        override val provider: String,
        override val providerVersion: String,
        override val modelName: String?,
        val fullText: String,
        val regions: List<AndroidRecognizedTextRegion> = emptyList(),
        override val completedAtMs: Long?,
        override val warnings: List<String> = emptyList(),
    ) : AndroidOcrRecognitionOutcome()

    data class NoTextDetected(
        override val provider: String,
        override val providerVersion: String,
        override val modelName: String?,
        override val completedAtMs: Long?,
        override val warnings: List<String> = emptyList(),
    ) : AndroidOcrRecognitionOutcome()

    data class Unavailable(
        val reason: OcrUnavailableReason,
        val message: String,
        val retryAvailable: Boolean,
        override val provider: String = "android-ocr-provider",
        override val providerVersion: String = "unavailable",
        override val modelName: String? = null,
        override val completedAtMs: Long? = null,
        override val warnings: List<String> = emptyList(),
    ) : AndroidOcrRecognitionOutcome()

    data class Cancelled(
        val message: String = "OCR was cancelled before text was returned",
        override val provider: String = "android-ocr-provider",
        override val providerVersion: String = "cancelled",
        override val modelName: String? = null,
        override val completedAtMs: Long? = null,
        override val warnings: List<String> = emptyList(),
    ) : AndroidOcrRecognitionOutcome()
}

private fun AndroidOcrRecognitionOutcome.toRecordRequest(
    scanId: String,
    prepared: PreparedAndroidOcrInput,
): AndroidRecordOcrRunRequest =
    when (this) {
        is AndroidOcrRecognitionOutcome.Detected ->
            AndroidRecordOcrRunRequest(
                scanId = scanId,
                inputAssetId = prepared.inputAssetId,
                provider = provider,
                providerVersion = providerVersion,
                modelName = modelName,
                status = OcrRunStatus.Detected,
                fullText = fullText,
                unavailableReason = null,
                unavailableMessage = null,
                completedAtMs = completedAtMs,
                warnings = warnings,
            )

        is AndroidOcrRecognitionOutcome.NoTextDetected ->
            AndroidRecordOcrRunRequest(
                scanId = scanId,
                inputAssetId = prepared.inputAssetId,
                provider = provider,
                providerVersion = providerVersion,
                modelName = modelName,
                status = OcrRunStatus.NoTextDetected,
                fullText = "",
                unavailableReason = null,
                unavailableMessage = null,
                completedAtMs = completedAtMs,
                warnings = warnings,
            )

        is AndroidOcrRecognitionOutcome.Unavailable ->
            AndroidRecordOcrRunRequest(
                scanId = scanId,
                inputAssetId = prepared.inputAssetId,
                provider = provider,
                providerVersion = providerVersion,
                modelName = modelName,
                status = OcrRunStatus.Unavailable,
                fullText = "",
                unavailableReason = reason,
                unavailableMessage = message,
                completedAtMs = completedAtMs,
                warnings = warnings,
            )

        is AndroidOcrRecognitionOutcome.Cancelled ->
            AndroidRecordOcrRunRequest(
                scanId = scanId,
                inputAssetId = prepared.inputAssetId,
                provider = provider,
                providerVersion = providerVersion,
                modelName = modelName,
                status = OcrRunStatus.Unavailable,
                fullText = "",
                unavailableReason = OcrUnavailableReason.Cancelled,
                unavailableMessage = message,
                completedAtMs = completedAtMs,
                warnings = warnings,
            )
    }

private fun AndroidRecognizedTextRegion.toRecordRequest(): AndroidRecordOcrTextRegionRequest =
    AndroidRecordOcrTextRegionRequest(
        polygon = polygon,
        text = text,
        confidence = confidence,
        createdAtMs = createdAtMs,
    )

private fun AndroidRecordOcrTextRegionRequest.toFfi(): FfiRecordOcrTextRegionRequest =
    FfiRecordOcrTextRegionRequest(
        polygon = polygon.map { point -> FfiOcrTextPoint(x = point.x, y = point.y) },
        text = text,
        confidence = confidence,
        createdAtMs = createdAtMs,
    )

private fun OcrInputKind.toFfi(): FfiOcrInputKind =
    when (this) {
        OcrInputKind.Original -> FfiOcrInputKind.ORIGINAL
        OcrInputKind.Corrected -> FfiOcrInputKind.CORRECTED
        OcrInputKind.OcrOptimized -> FfiOcrInputKind.OCR_OPTIMIZED
    }

private fun FfiOcrInputKind.toAndroid(): OcrInputKind =
    when (this) {
        FfiOcrInputKind.ORIGINAL -> OcrInputKind.Original
        FfiOcrInputKind.CORRECTED -> OcrInputKind.Corrected
        FfiOcrInputKind.OCR_OPTIMIZED -> OcrInputKind.OcrOptimized
    }

private fun OcrRunStatus.toFfi(): FfiOcrRunStatus =
    when (this) {
        OcrRunStatus.Detected -> FfiOcrRunStatus.DETECTED
        OcrRunStatus.NoTextDetected -> FfiOcrRunStatus.NO_TEXT_DETECTED
        OcrRunStatus.Unavailable -> FfiOcrRunStatus.UNAVAILABLE
    }

private fun FfiOcrRunStatus.toAndroid(): OcrRunStatus =
    when (this) {
        FfiOcrRunStatus.DETECTED -> OcrRunStatus.Detected
        FfiOcrRunStatus.NO_TEXT_DETECTED -> OcrRunStatus.NoTextDetected
        FfiOcrRunStatus.UNAVAILABLE -> OcrRunStatus.Unavailable
    }

private fun OcrUnavailableReason.toFfi(): FfiOcrUnavailableReason =
    when (this) {
        OcrUnavailableReason.ProviderUnavailable -> FfiOcrUnavailableReason.PROVIDER_UNAVAILABLE
        OcrUnavailableReason.ProviderFailed -> FfiOcrUnavailableReason.PROVIDER_FAILED
        OcrUnavailableReason.ResourceUnavailable -> FfiOcrUnavailableReason.RESOURCE_UNAVAILABLE
        OcrUnavailableReason.UnsupportedInput -> FfiOcrUnavailableReason.UNSUPPORTED_INPUT
        OcrUnavailableReason.Cancelled -> FfiOcrUnavailableReason.CANCELLED
    }

private const val TEXT_PREVIEW_LIMIT = 240
