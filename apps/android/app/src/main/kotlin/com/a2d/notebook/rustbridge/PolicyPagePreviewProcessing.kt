package com.a2d.notebook.rustbridge

import uniffi.a2d_ffi.A2dClient

private const val STATUS_SUCCESS = 0
private const val STATUS_ERROR = 1
private const val STATUS_PANIC = 2
private const val STATUS_CANCELLED = 3
private const val RESULT_ENVELOPE_LIMIT = 64L * 1024 * 1024
private const val ERROR_ENVELOPE_LIMIT = 64L * 1024

internal fun A2dClient.processPolicyPagePreview(
    request: PolicyPagePreviewProcessingRequest,
    cancellation: PolicyPagePreviewCancellation,
): PolicyPagePreviewProcessingOutcome {
    validatePolicyPreviewRequest(request)
    val layoutId = request.storedPolicy.layoutId.toByteArray(Charsets.UTF_8)
    return cancellation.withPointer { cancellationPointer ->
        val status = PreviewProcessingStatus()
        val output =
            policyPreviewNativeLibrary.a2d_process_encoded_page_preview_v2(
                bytes = request.encodedBytes,
                bytesLen = request.encodedBytes.size.toLong(),
                formatCode = if (request.format == EncodedPageFormat.JPEG) 0 else 1,
                rotationDegrees = request.rotation.degrees,
                layoutIdBytes = layoutId,
                layoutIdLen = layoutId.size.toLong(),
                processingPolicyVersion = request.storedPolicy.processingPolicyVersion,
                cancellation = cancellationPointer,
                status = status,
            )
        status.read()
        output.read()
        status.error.read()

        try {
            decodePolicyPreviewOutcome(status, output, request.storedPolicy)
        } finally {
            freePolicyPreviewBuffer(output)
            freePolicyPreviewBuffer(status.error)
        }
    }
}

private fun validatePolicyPreviewRequest(request: PolicyPagePreviewProcessingRequest) {
    require(request.encodedBytes.isNotEmpty()) { "encoded capture must not be empty" }
    require(request.storedPolicy.layoutId.isNotBlank())
    require(request.storedPolicy.processingPolicyVersion > 0)
    require(request.storedPolicy.pipelineVersion > 0)
    require(request.storedPolicy.maximumEncodedBytes > 0)
    require(request.encodedBytes.size.toLong() <= request.storedPolicy.maximumEncodedBytes) {
        "encoded capture exceeds the Rust-issued byte limit"
    }
}

private fun decodePolicyPreviewOutcome(
    status: PreviewProcessingStatus,
    output: PreviewProcessingBuffer.ByValue,
    policy: StoredScanPolicy,
): PolicyPagePreviewProcessingOutcome =
    when (status.code) {
        STATUS_SUCCESS -> {
            require(status.error.data == null && status.error.len == 0L)
            PolicyPagePreviewProcessingOutcome.Completed(
                decodePolicyPreviewResult(
                    readPolicyPreviewBuffer(output, "policy preview result", RESULT_ENVELOPE_LIMIT),
                    policy,
                ),
            )
        }

        STATUS_CANCELLED -> {
            require(output.data == null && output.len == 0L)
            require(status.error.data == null && status.error.len == 0L)
            PolicyPagePreviewProcessingOutcome.Cancelled
        }

        STATUS_ERROR,
        STATUS_PANIC,
        -> {
            require(output.data == null && output.len == 0L)
            throw PagePreviewProcessingException(
                details =
                    decodePolicyPreviewError(
                        readPolicyPreviewBuffer(
                            status.error,
                            "policy preview error",
                            ERROR_ENVELOPE_LIMIT,
                        ),
                    ),
                nativePanic = status.code == STATUS_PANIC,
            )
        }

        else -> error("policy preview returned unknown status code ${status.code}")
    }

private fun readPolicyPreviewBuffer(
    buffer: PreviewProcessingBuffer,
    description: String,
    maximumBytes: Long,
): ByteArray {
    require(buffer.len > 0L) { "$description buffer is empty" }
    require(buffer.capacity >= buffer.len) { "$description buffer length exceeds capacity" }
    require(buffer.len <= maximumBytes) { "$description exceeds its transport envelope" }
    val pointer = requireNotNull(buffer.data) { "$description pointer is null" }
    return pointer.getByteArray(0L, Math.toIntExact(buffer.len))
}

private fun freePolicyPreviewBuffer(buffer: PreviewProcessingBuffer.ByValue) {
    if (buffer.data != null) {
        policyPreviewNativeLibrary.a2d_preview_buffer_free(buffer)
    }
}
