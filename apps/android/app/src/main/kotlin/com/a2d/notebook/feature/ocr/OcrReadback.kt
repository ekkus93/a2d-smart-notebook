package com.a2d.notebook.feature.ocr

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.LoadLatestOcrOutputRequest as FfiLoadLatestOcrOutputRequest
import uniffi.a2d_ffi.LoadOcrRegionPageRequest as FfiLoadOcrRegionPageRequest
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind
import uniffi.a2d_ffi.OcrRunStatus as FfiOcrRunStatus
import uniffi.a2d_ffi.OcrTextPoint as FfiOcrTextPoint
import uniffi.a2d_ffi.OcrUnavailableReason as FfiOcrUnavailableReason

/**
 * Android readback adapter for persisted OCR output.
 *
 * Running OCR and reading previously persisted OCR are deliberately separate flows. This adapter
 * hydrates [OcrPresentationState] from Rust-owned OCR rows so Page Viewer callers can restore OCR
 * status after app restart without manufacturing an empty detected-text result on Android.
 *
 * Run metadata is loaded independently from region data. Detected runs are then hydrated through
 * the Rust-owned bounded pagination contract until the authoritative durable count is complete.
 * No partially accumulated region list is returned to Page Viewer.
 */
class FfiAndroidOcrReadback(private val client: A2dClient) {
    fun loadLatestOcrOutput(
        scanId: String,
        regionLimit: UInt = DEFAULT_OCR_READBACK_REGION_LIMIT,
    ): LoadedAndroidOcrOutput {
        require(regionLimit in 1u..MAX_OCR_REGION_PAGE_SIZE) {
            "OCR region page size must be between 1 and $MAX_OCR_REGION_PAGE_SIZE"
        }
        val loaded =
            client.loadLatestOcrOutput(
                FfiLoadLatestOcrOutputRequest(
                    scanId = scanId,
                    regionLimit = 0u,
                ),
            )
        val latestRun = loaded.latestRun
        val hydratedRegions =
            if (latestRun?.status == FfiOcrRunStatus.DETECTED) {
                loadAllDetectedRegions(
                    scanId = loaded.scanId,
                    expectedOcrRunId = latestRun.ocrRunId,
                    expectedTotalCount = latestRun.textRegionCount,
                    pageSize = regionLimit,
                )
            } else {
                emptyList()
            }
        return LoadedAndroidOcrOutput(
            scanId = loaded.scanId,
            sourceGeometry = latestRun?.let { loadSourceGeometry(loaded.scanId) },
            latestRun =
                latestRun?.let { run ->
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
                        textRegions = hydratedRegions,
                    )
                },
        )
    }

    fun loadPresentationState(
        scanId: String,
        regionLimit: UInt = DEFAULT_OCR_READBACK_REGION_LIMIT,
    ): OcrPresentationState = loadLatestOcrOutput(scanId, regionLimit).toPresentationState()

    private fun loadAllDetectedRegions(
        scanId: String,
        expectedOcrRunId: String,
        expectedTotalCount: UInt,
        pageSize: UInt,
    ): List<LoadedAndroidOcrTextRegion> =
        AndroidOcrRegionPaginationHydrator.hydrate(
            scanId = scanId,
            expectedOcrRunId = expectedOcrRunId,
            expectedTotalCount = expectedTotalCount,
            pageSize = pageSize,
        ) { offset ->
            val page =
                client.loadOcrRegionPage(
                    FfiLoadOcrRegionPageRequest(
                        scanId = scanId,
                        offset = offset,
                        pageSize = pageSize,
                    ),
                )
            AndroidOcrRegionPage(
                scanId = page.scanId,
                ocrRunId = page.ocrRunId,
                totalCount = page.totalCount,
                returnedCount = page.returnedCount,
                offset = page.offset,
                nextOffset = page.nextOffset,
                hasMore = page.hasMore,
                complete = page.complete,
                regions =
                    page.regions.map { region ->
                        AndroidOcrRegionPageRegion(
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

    private fun loadSourceGeometry(scanId: String): AndroidOcrSourceGeometry? =
        runCatching { AndroidOcrSourceGeometryGateway(client).resolve(scanId, FfiOcrInputKind.ORIGINAL) }.getOrNull()
}

internal object AndroidOcrRegionPaginationHydrator {
    fun hydrate(
        scanId: String,
        expectedOcrRunId: String,
        expectedTotalCount: UInt,
        pageSize: UInt,
        loadPage: (offset: UInt) -> AndroidOcrRegionPage,
    ): List<LoadedAndroidOcrTextRegion> {
        val regionsById = LinkedHashMap<String, LoadedAndroidOcrTextRegion>()
        var offset = 0u
        while (true) {
            val page = loadPage(offset)
            if (page.scanId != scanId) {
                throw OcrRegionPaginationException("OCR region page belongs to an unexpected scan")
            }
            if (page.ocrRunId != expectedOcrRunId) {
                throw OcrRegionPaginationException("OCR run changed while regions were being hydrated")
            }
            if (page.totalCount != expectedTotalCount) {
                throw OcrRegionPaginationException("OCR region count changed while regions were being hydrated")
            }
            if (page.offset != offset || page.returnedCount.toInt() != page.regions.size) {
                throw OcrRegionPaginationException("OCR region page metadata is inconsistent")
            }
            page.regions.forEach { region ->
                if (region.ocrRunId != expectedOcrRunId) {
                    throw OcrRegionPaginationException("OCR region belongs to an unexpected run")
                }
                val mapped =
                    LoadedAndroidOcrTextRegion(
                        textRegionId = region.textRegionId,
                        ocrRunId = region.ocrRunId,
                        polygon = region.polygon,
                        text = region.text,
                        confidence = region.confidence,
                        createdAtMs = region.createdAtMs,
                    )
                val previous = regionsById.putIfAbsent(mapped.textRegionId, mapped)
                if (previous != null && previous != mapped) {
                    throw OcrRegionPaginationException("Conflicting duplicate OCR region ID ${mapped.textRegionId}")
                }
            }
            if (page.complete) {
                if (page.hasMore || page.nextOffset != null || regionsById.size != expectedTotalCount.toInt()) {
                    throw OcrRegionPaginationException("OCR region pagination claimed completeness before the authoritative end")
                }
                return regionsById.values.toList()
            }
            if (!page.hasMore) {
                throw OcrRegionPaginationException("OCR region pagination stopped before completion")
            }
            val nextOffset = page.nextOffset
                ?: throw OcrRegionPaginationException("OCR region pagination omitted its continuation offset")
            if (nextOffset <= offset) {
                throw OcrRegionPaginationException("OCR region pagination did not advance")
            }
            offset = nextOffset
        }
    }
}

internal data class AndroidOcrRegionPage(
    val scanId: String,
    val ocrRunId: String?,
    val totalCount: UInt,
    val returnedCount: UInt,
    val offset: UInt,
    val nextOffset: UInt?,
    val hasMore: Boolean,
    val complete: Boolean,
    val regions: List<AndroidOcrRegionPageRegion>,
)

internal data class AndroidOcrRegionPageRegion(
    val textRegionId: String,
    val ocrRunId: String,
    val polygon: List<OcrTextPoint>,
    val text: String,
    val confidence: Float?,
    val createdAtMs: Long,
)

class OcrRegionPaginationException(message: String) : IllegalStateException(message)

data class LoadedAndroidOcrOutput(
    val scanId: String,
    val latestRun: LoadedAndroidOcrRun?,
    val sourceGeometry: AndroidOcrSourceGeometry? = null,
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
private const val DEFAULT_OCR_READBACK_REGION_LIMIT: UInt = 1_000u
private const val MAX_OCR_REGION_PAGE_SIZE: UInt = 1_000u
