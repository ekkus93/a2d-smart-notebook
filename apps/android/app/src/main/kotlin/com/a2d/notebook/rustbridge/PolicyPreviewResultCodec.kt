package com.a2d.notebook.rustbridge

internal fun decodePolicyPreviewResult(
    bytes: ByteArray,
    expectedPolicy: StoredScanPolicy,
): PolicyProcessedPagePreview {
    val reader = PolicyPreviewCodecReader(bytes)
    reader.requireHeader("A2DP")
    val pipelineVersion = reader.readInt("pipeline version")
    val matrix =
        List(reader.readInt("matrix element count")) { index ->
            reader.readDouble("matrix[$index]")
        }
    require(matrix.size == 9) { "source-to-corrected matrix must contain 9 values" }

    val analysis = decodePolicyPreviewAnalysis(reader)
    val corrected = decodePolicyPreviewRgb(reader, "corrected")
    val thumbnail = decodePolicyPreviewRgb(reader, "thumbnail")
    reader.requireExhausted()

    requirePolicyPreviewIdentity(
        policy = expectedPolicy,
        pipelineVersion = pipelineVersion,
        corrected = corrected,
    )
    return PolicyProcessedPagePreview(
        analysis = analysis,
        corrected = corrected,
        thumbnail = thumbnail,
        layoutId = expectedPolicy.layoutId,
        processingPolicyVersion = expectedPolicy.processingPolicyVersion,
        pipelineVersion = pipelineVersion,
        sourceToCorrectedMatrix = matrix,
    )
}

private fun requirePolicyPreviewIdentity(
    policy: StoredScanPolicy,
    pipelineVersion: Int,
    corrected: ProcessedRgbImage,
) {
    if (pipelineVersion != policy.pipelineVersion) {
        throw policyPreviewMismatch("pipeline version", policy.pipelineVersion, pipelineVersion)
    }
    if (corrected.width != policy.correctedWidth || corrected.height != policy.correctedHeight) {
        throw policyPreviewMismatch(
            "corrected dimensions",
            "${policy.correctedWidth}x${policy.correctedHeight}",
            "${corrected.width}x${corrected.height}",
        )
    }
}
