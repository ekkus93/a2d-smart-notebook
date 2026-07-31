package com.a2d.notebook.rustbridge

internal fun decodePolicyPreviewError(bytes: ByteArray): LivePageAnalysisErrorDetails {
    val reader = PolicyPreviewCodecReader(bytes)
    reader.requireHeader("A2PE")
    val details =
        LivePageAnalysisErrorDetails(
            code = reader.readString("error code"),
            category = reader.readString("error category"),
            severity = reader.readString("error severity"),
            userMessageKey = reader.readString("error user message key"),
            developerMessage = reader.readString("error developer message"),
            correlationId = reader.readString("error correlation ID"),
            retryable = reader.readBoolean("error retryable"),
        )
    reader.requireExhausted()
    return details
}
