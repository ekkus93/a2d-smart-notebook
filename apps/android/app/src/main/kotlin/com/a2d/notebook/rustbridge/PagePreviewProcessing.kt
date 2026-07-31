package com.a2d.notebook.rustbridge

import com.a2d.notebook.feature.scanner.singlepage.RustScannerPolicySession
import uniffi.a2d_ffi.A2dClient

/**
 * Compatibility adapter for the original scanner call site.
 *
 * The scalar fields retained in [PagePreviewProcessingRequest] are no longer authoritative and are
 * not forwarded across the native boundary. They remain temporarily so the scanner workflow can be
 * migrated without generated-binding churn. Rust resolves the exact layout and processing policy
 * from the stored-policy identity issued for the current scanner session.
 */
@Suppress("UnusedReceiverParameter")
internal fun A2dClient.processPagePreview(
    request: PagePreviewProcessingRequest,
    cancellation: PagePreviewCancellation,
): PagePreviewProcessingOutcome {
    val storedPolicy = RustScannerPolicySession.requireCurrentPolicy()
    val outcome =
        processPolicyPagePreview(
            request =
                PolicyPagePreviewProcessingRequest(
                    encodedBytes = request.encodedBytes,
                    format = request.format,
                    rotation = request.rotation,
                    storedPolicy = storedPolicy,
                ),
            cancellation = cancellation,
        )
    return when (outcome) {
        PolicyPagePreviewProcessingOutcome.Cancelled -> PagePreviewProcessingOutcome.Cancelled
        is PolicyPagePreviewProcessingOutcome.Completed -> {
            RustScannerPolicySession.markReviewed(storedPolicy)
            val result = outcome.result
            PagePreviewProcessingOutcome.Completed(
                ProcessedPagePreview(
                    analysis = result.analysis,
                    corrected = result.corrected,
                    thumbnail = result.thumbnail,
                    pipelineVersion = result.pipelineVersion,
                    sourceToCorrectedMatrix = result.sourceToCorrectedMatrix,
                ),
            )
        }
    }
}
