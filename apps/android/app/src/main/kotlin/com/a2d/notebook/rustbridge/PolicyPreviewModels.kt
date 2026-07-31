package com.a2d.notebook.rustbridge

internal data class PolicyProcessedPagePreview(
    val analysis: EncodedPageAnalysisResult,
    val corrected: ProcessedRgbImage,
    val thumbnail: ProcessedRgbImage,
    val layoutId: String,
    val processingPolicyVersion: Int,
    val pipelineVersion: Int,
    val sourceToCorrectedMatrix: List<Double>,
)

internal fun policyPreviewMismatch(
    field: String,
    expected: Any,
    actual: Any,
): PagePreviewProcessingException =
    PagePreviewProcessingException(
        details =
            LivePageAnalysisErrorDetails(
                code = "ANDROID_PREVIEW_POLICY_RESULT_MISMATCH",
                category = "Integrity",
                severity = "Critical",
                userMessageKey = "error.preview.policy_result_mismatch",
                developerMessage = "$field mismatch: expected $expected, got $actual",
                correlationId = "android-policy-preview",
                retryable = false,
            ),
        nativePanic = false,
    )
