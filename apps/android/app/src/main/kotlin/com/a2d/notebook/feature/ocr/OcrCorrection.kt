package com.a2d.notebook.feature.ocr

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.ListOcrCorrectionsForScanRequest as FfiListOcrCorrectionsForScanRequest
import uniffi.a2d_ffi.RecordOcrCorrectionRequest as FfiRecordOcrCorrectionRequest

/** Android-facing adapter for Rust-owned OCR text corrections. */
interface AndroidOcrCorrectionGateway {
    fun recordOcrCorrection(request: AndroidRecordOcrCorrectionRequest): RecordedAndroidOcrCorrection

    fun listOcrCorrectionsForScan(
        request: AndroidListOcrCorrectionsForScanRequest,
    ): AndroidOcrCorrectionReview
}

class FfiAndroidOcrCorrectionGateway(private val client: A2dClient) : AndroidOcrCorrectionGateway {
    override fun recordOcrCorrection(
        request: AndroidRecordOcrCorrectionRequest,
    ): RecordedAndroidOcrCorrection {
        val recorded =
            client.recordOcrCorrection(
                FfiRecordOcrCorrectionRequest(
                    scanId = request.scanId,
                    textRegionId = request.textRegionId,
                    correctedText = request.correctedText,
                    previousText = request.previousText,
                    createdAtMs = request.createdAtMs,
                ),
            )
        return RecordedAndroidOcrCorrection(
            textCorrectionId = recorded.textCorrectionId,
            scanId = recorded.scanId,
            textRegionId = recorded.textRegionId,
            correctedText = recorded.correctedText,
            previousText = recorded.previousText,
            createdAtMs = recorded.createdAtMs,
        )
    }

    override fun listOcrCorrectionsForScan(
        request: AndroidListOcrCorrectionsForScanRequest,
    ): AndroidOcrCorrectionReview {
        val loaded =
            client.listOcrCorrectionsForScan(
                FfiListOcrCorrectionsForScanRequest(
                    scanId = request.scanId,
                    limit = request.limit,
                ),
            )
        return AndroidOcrCorrectionReview(
            scanId = loaded.scanId,
            corrections =
                loaded.corrections.map { correction ->
                    AndroidOcrTextCorrection(
                        textCorrectionId = correction.textCorrectionId,
                        scanId = correction.scanId,
                        textRegionId = correction.textRegionId,
                        correctedText = correction.correctedText,
                        previousText = correction.previousText,
                        createdAtMs = correction.createdAtMs,
                    )
                },
        )
    }
}

class AndroidOcrCorrectionController(
    private val gateway: AndroidOcrCorrectionGateway,
    private val defaultReviewLimit: UInt = 50u,
) {
    fun submitCorrection(
        scanId: String,
        textRegionId: String?,
        correctedText: String,
        previousText: String? = null,
    ): OcrCorrectionPresentationState {
        val trimmedCorrection = correctedText.trim()
        if (trimmedCorrection.isEmpty()) {
            return OcrCorrectionPresentationState.error(
                scanId = scanId,
                message = "Correction text must not be empty",
            )
        }
        return try {
            val recorded =
                gateway.recordOcrCorrection(
                    AndroidRecordOcrCorrectionRequest(
                        scanId = scanId,
                        textRegionId = textRegionId,
                        correctedText = trimmedCorrection,
                        previousText = previousText,
                        createdAtMs = null,
                    ),
                )
            OcrCorrectionPresentationState.saved(recorded)
        } catch (failure: Exception) {
            OcrCorrectionPresentationState.error(
                scanId = scanId,
                message = failure.message ?: failure::class.java.simpleName,
            )
        }
    }

    fun loadReview(scanId: String): OcrCorrectionPresentationState =
        try {
            val review =
                gateway.listOcrCorrectionsForScan(
                    AndroidListOcrCorrectionsForScanRequest(
                        scanId = scanId,
                        limit = defaultReviewLimit,
                    ),
                )
            OcrCorrectionPresentationState.review(review)
        } catch (failure: Exception) {
            OcrCorrectionPresentationState.error(
                scanId = scanId,
                message = failure.message ?: failure::class.java.simpleName,
            )
        }
}

data class AndroidRecordOcrCorrectionRequest(
    val scanId: String,
    val textRegionId: String?,
    val correctedText: String,
    val previousText: String?,
    val createdAtMs: Long?,
)

data class RecordedAndroidOcrCorrection(
    val textCorrectionId: String,
    val scanId: String,
    val textRegionId: String?,
    val correctedText: String,
    val previousText: String?,
    val createdAtMs: Long,
)

data class AndroidListOcrCorrectionsForScanRequest(
    val scanId: String,
    val limit: UInt,
)

data class AndroidOcrTextCorrection(
    val textCorrectionId: String,
    val scanId: String,
    val textRegionId: String?,
    val correctedText: String,
    val previousText: String?,
    val createdAtMs: Long,
)

data class AndroidOcrCorrectionReview(
    val scanId: String,
    val corrections: List<AndroidOcrTextCorrection>,
)

data class OcrCorrectionPresentationState(
    val scanId: String,
    val status: OcrCorrectionPresentationStatus,
    val corrections: List<AndroidOcrTextCorrection> = emptyList(),
    val lastSaved: RecordedAndroidOcrCorrection? = null,
    val errorMessage: String? = null,
) {
    companion object {
        fun empty(scanId: String): OcrCorrectionPresentationState =
            OcrCorrectionPresentationState(
                scanId = scanId,
                status = OcrCorrectionPresentationStatus.Empty,
            )

        fun saved(recorded: RecordedAndroidOcrCorrection): OcrCorrectionPresentationState =
            OcrCorrectionPresentationState(
                scanId = recorded.scanId,
                status = OcrCorrectionPresentationStatus.Saved,
                lastSaved = recorded,
            )

        fun review(review: AndroidOcrCorrectionReview): OcrCorrectionPresentationState =
            OcrCorrectionPresentationState(
                scanId = review.scanId,
                status =
                    if (review.corrections.isEmpty()) {
                        OcrCorrectionPresentationStatus.Empty
                    } else {
                        OcrCorrectionPresentationStatus.Review
                    },
                corrections = review.corrections,
            )

        fun error(scanId: String, message: String): OcrCorrectionPresentationState =
            OcrCorrectionPresentationState(
                scanId = scanId,
                status = OcrCorrectionPresentationStatus.Error,
                errorMessage = message,
            )
    }
}

enum class OcrCorrectionPresentationStatus {
    Empty,
    Saved,
    Review,
    Error,
}
