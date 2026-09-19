package com.a2d.notebook.feature.ocr

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.LoadLatestOcrOutputRequest as FfiLoadLatestOcrOutputRequest
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind
import uniffi.a2d_ffi.OcrRunStatus as FfiOcrRunStatus
import uniffi.a2d_ffi.OcrTextPoint as FfiOcrTextPoint
import uniffi.a2d_ffi.OcrUnavailableReason as FfiOcrUnavailableReason

/** Android readback adapter for persisted OCR output and its Rust-owned source geometry. */
class FfiAndroidOcrReadback(private val client: A2dClient) {
    fun loadLatestOcrOutput(
        scanId: String,
        regionLimit: UInt = DEFAULT_OCR_READBACK_REGION_LIMIT,
    ): LoadedAndroidOcrOutput {
        val loaded =
            client.loadLatestOcrOutput(
                FfiLoadLatestOcrOutputRequest(
                    scanId = scanId,
                    regionLimit = regionLimit,
                ),
            )
        val latestRun =
            loaded.latestRun?.let { run ->
                LoadedAndroidOcrRun(
                    ocrRunId = run.ocrRunId,
                    scanId = run.scanId,
                    inputAssetId = run.inputAssetId,
                    provider = run.provider,
                    providerVersion = run.providerVersion,
                    modelName = run.modelName,
                    status = run.status.toAndroid(),
                    fullText = run.fullText,
                    unavailableReason = run.unavailableReason?.toAndroid(),
                    unavailableMessage = run.unavailableMessage,
                    completedAtMs = run.completedAtMs,
                    warnings = run.warnings,
                    textRegionCount = run.textRegionCount.toInt(),
                    textRegions =
                        run.textRegions.map { region ->
                            LoadedAndroidOcrTextRegion(
                                textRegionId = region.textRegionId,
                                ocrRunId = region.ocrRunId,
                                polygon = region.polygon.map { point -> point.toAndroid() },
                                text = region.text,
                                confidence = region.confidence,
                                createdAtMs = region.createdAtMs,
                            )
                        },
                )
            }
        return LoadedAndroidOcrOutput(
            scanId = loaded.scanId,
            latestRun = latestRun,
            sourceGeometry = latestRun?.inputAssetId?.let { assetId -> resolveSourceGeometry(scanId, assetId) },
        )
    }

    private fun resolveSourceGeometry(scanId: String, inputAssetId: String): LoadedAndroidOcrSourceGeometry? {
        for (inputKind in FfiOcrInputKind.entries) {
            val geometry =
                try {
                    client.resolveOcrSourceGeometry(scanId, inputKind)
                } catch (_: Exception) {
                    null
                }
            if (geometry != null && geometry.inputAssetId == inputAssetId) {
                return LoadedAndroidOcrSourceGeometry(
                    inputAssetId = geometry.inputAssetId,
                    inputKind = geometry.inputKind.name,
                    mediaType = geometry.mediaType,
                    relativePath = geometry.relativePath,
                    byteLength = geometry.byteLength,
                    widthPx = geometry.widthPx.toInt(),
                    heightPx = geometry.heightPx.toInt(),
                )
            }
        }
        return null
    }

    fun loadPresentationState(
        scanId: String,
        regionLimit: UInt = DEFAULT_OCR_READBACK_REGION_LIMIT,
    ): OcrPresentationState = loadLatestOcrOutput(scanId, regionLimit).toPresentationState()
}

data class LoadedAndroidOcrOutput(
    val scanId: String,
    val latestRun: LoadedAndroidOcrRun?,
    val sourceGeometry: LoadedAndroidOcrSourceGeometry? = null,
) {
    fun toPresentationState(): OcrPresentationState =
        latestRun?.toPresentationState() ?: OcrPresentationState()
}

data class LoadedAndroidOcrSourceGeometry(
    val inputAssetId: String,
    val inputKind: String,
    val mediaType: String,
    val relativePath: String,
    val byteLength: ULong,
    val widthPx: Int,
    val heightPx: Int,
)

data class LoadedAndroidOcrRun(
    val ocrRunId: String,
    val scanId: String,
    val inputAssetId: String?,
    val provider: String,
    val providerVersion: String,
    val modelName: String?,
    val status: OcrRunStatus,
    val fullText: String,
    val unavailableReason: OcrUnavailableReason?,
    val unavailableMessage: String?,
    val completedAtMs: Long?,
    val warnings: List<String>,
    val textRegionCount: Int,
    val textRegions: List<LoadedAndroidOcrTextRegion>,
) {
    fun toPresentationState(): OcrPresentationState =
        OcrPresentationState(
            status = toPresentationStatus(),
            runId = ocrRunId,
            providerLabel = provider,
            modelName = modelName,
            textPreview = if (status == OcrRunStatus.Detected) fullText.take(TEXT_PREVIEW_LIMIT) else null,
            recognizedRegionCount = if (status == OcrRunStatus.Detected) textRegionCount else 0,
            unavailableReason = unavailableReason?.label,
            message = unavailableMessage,
            retryAvailable = unavailableReason.isRetryableReadbackUnavailableReason(),
            cancelAvailable = false,
        )

    private fun toPresentationStatus(): OcrPresentationStatus =
        when (status) {
            OcrRunStatus.Detected -> OcrPresentationStatus.Detected
            OcrRunStatus.NoTextDetected -> OcrPresentationStatus.NoTextDetected
            OcrRunStatus.Unavailable ->
                if (unavailableReason == OcrUnavailableReason.Cancelled) {
                    OcrPresentationStatus.Cancelled
                } else {
                    OcrPresentationStatus.Unavailable
                }
        }
}

data class LoadedAndroidOcrTextRegion(
    val textRegionId: String,
    val ocrRunId: String,
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
    val createdAtMs: Long,
)

private fun FfiOcrTextPoint.toAndroid(): OcrTextPoint = OcrTextPoint(x = x, y = y)

private fun FfiOcrRunStatus.toAndroid(): OcrRunStatus =
    when (this) {
        FfiOcrRunStatus.DETECTED -> OcrRunStatus.Detected
        FfiOcrRunStatus.NO_TEXT_DETECTED -> OcrRunStatus.NoTextDetected
        FfiOcrRunStatus.UNAVAILABLE -> OcrRunStatus.Unavailable
    }

private fun FfiOcrUnavailableReason.toAndroid(): OcrUnavailableReason =
    when (this) {
        FfiOcrUnavailableReason.PROVIDER_UNAVAILABLE -> OcrUnavailableReason.ProviderUnavailable
        FfiOcrUnavailableReason.PROVIDER_FAILED -> OcrUnavailableReason.ProviderFailed
        FfiOcrUnavailableReason.RESOURCE_UNAVAILABLE -> OcrUnavailableReason.ResourceUnavailable
        FfiOcrUnavailableReason.UNSUPPORTED_INPUT -> OcrUnavailableReason.UnsupportedInput
        FfiOcrUnavailableReason.CANCELLED -> OcrUnavailableReason.Cancelled
    }

private fun OcrUnavailableReason?.isRetryableReadbackUnavailableReason(): Boolean =
    when (this) {
        OcrUnavailableReason.ProviderUnavailable,
        OcrUnavailableReason.ProviderFailed,
        OcrUnavailableReason.ResourceUnavailable,
        -> true
        OcrUnavailableReason.UnsupportedInput,
        OcrUnavailableReason.Cancelled,
        null,
        -> false
    }

private const val TEXT_PREVIEW_LIMIT = 240
private const val DEFAULT_OCR_READBACK_REGION_LIMIT: UInt = 1_000u
