package com.a2d.notebook.feature.ocr

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.LoadLatestOcrOutputRequest as FfiLoadLatestOcrOutputRequest
import uniffi.a2d_ffi.OcrRunStatus as FfiOcrRunStatus
import uniffi.a2d_ffi.OcrTextPoint as FfiOcrTextPoint
import uniffi.a2d_ffi.OcrUnavailableReason as FfiOcrUnavailableReason

/**
 * Android readback adapter for persisted OCR output.
 *
 * Running OCR and reading previously persisted OCR are deliberately separate flows. This adapter
 * hydrates [OcrPresentationState] from Rust-owned OCR rows so Page Viewer callers can restore OCR
 * status after app restart without manufacturing an empty detected-text result on Android.
 */
class FfiAndroidOcrReadback(private val client: A2dClient) {
    fun loadLatestOcrOutput(
        scanId: String,
        regionLimit: UInt = 50u,
    ): LoadedAndroidOcrOutput {
        val loaded =
            client.loadLatestOcrOutput(
                FfiLoadLatestOcrOutputRequest(
                    scanId = scanId,
                    regionLimit = regionLimit,
                ),
            )
        return LoadedAndroidOcrOutput(
            scanId = loaded.scanId,
            latestRun =
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
                },
        )
    }

    fun loadPresentationState(
        scanId: String,
        regionLimit: UInt = 50u,
    ): OcrPresentationState = loadLatestOcrOutput(scanId, regionLimit).toPresentationState()
}

data class LoadedAndroidOcrOutput(
    val scanId: String,
    val latestRun: LoadedAndroidOcrRun?,
) {
    fun toPresentationState(): OcrPresentationState =
        latestRun?.toPresentationState() ?: OcrPresentationState()
}

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
            textPreview =
                if (status == OcrRunStatus.Detected) {
                    fullText.take(TEXT_PREVIEW_LIMIT)
                } else {
                    null
                },
            recognizedRegionCount =
                if (status == OcrRunStatus.Detected) {
                    textRegionCount
                } else {
                    0
                },
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

private fun FfiOcrTextPoint.toAndroid(): OcrTextPoint =
    OcrTextPoint(
        x = x,
        y = y,
    )

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
