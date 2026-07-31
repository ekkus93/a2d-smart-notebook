package com.a2d.notebook.rustbridge

internal data class PolicyPagePreviewProcessingRequest(
    val encodedBytes: ByteArray,
    val format: EncodedPageFormat,
    val rotation: EncodedPageRotation,
    val storedPolicy: StoredScanPolicy,
)

internal sealed interface PolicyPagePreviewProcessingOutcome {
    data class Completed(
        val result: PolicyProcessedPagePreview,
    ) : PolicyPagePreviewProcessingOutcome

    data object Cancelled : PolicyPagePreviewProcessingOutcome
}
