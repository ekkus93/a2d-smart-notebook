package com.a2d.notebook.feature.ocr

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OcrQueueJob

/**
 * Production Android boundary for the Rust-owned bounded manual OCR retry transition.
 *
 * Retry policy, eligibility, attempt accounting, durable job identity, and diagnostics remain
 * authoritative in Rust. Android only dispatches the user action off the main thread and consumes
 * the returned durable job snapshot.
 */
class AndroidOcrManualRetryGateway(
    private val client: A2dClient,
) {
    suspend fun retry(scanId: String): OcrQueueJob =
        withContext(Dispatchers.IO) {
            client.manualRetryOcrJobForScan(scanId)
        }
}
