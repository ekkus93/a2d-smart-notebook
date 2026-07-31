package com.a2d.notebook.rustbridge

internal fun decodePolicyPreviewAnalysis(
    reader: PolicyPreviewCodecReader,
): EncodedPageAnalysisResult {
    val width = reader.readUInt("analysis width")
    val height = reader.readUInt("analysis height")
    val sourceRotationDegrees = reader.readInt("source rotation")
    val resolvedOrientationDegrees = reader.readInt("resolved orientation")
    val markers =
        List(reader.readInt("marker count")) { index ->
            decodePolicyPreviewMarker(reader, index)
        }
    val unexpectedTagIds =
        List(reader.readInt("unexpected tag count")) { index ->
            reader.readUInt("unexpected tag[$index]")
        }
    return EncodedPageAnalysisResult(
        width = width,
        height = height,
        sourceRotationDegrees = sourceRotationDegrees,
        resolvedOrientationDegrees = resolvedOrientationDegrees,
        markers = markers,
        unexpectedTagIds = unexpectedTagIds,
        quality =
            AnalyzedPageQuality(
                focusLaplacianVariance = reader.readOptionalDouble("focus Laplacian variance"),
                focusInteriorSampleCount = reader.readOptionalULong("focus interior sample count"),
                meanLuminance = reader.readDouble("mean luminance"),
                luminanceStandardDeviation = reader.readDouble("luminance standard deviation"),
                darkFraction = reader.readDouble("dark fraction"),
                highlightFraction = reader.readDouble("highlight fraction"),
                maxTileHighlightFraction = reader.readDouble("maximum tile highlight fraction"),
                populatedTileCount = reader.readUInt("populated tile count"),
            ),
    )
}

private fun decodePolicyPreviewMarker(
    reader: PolicyPreviewCodecReader,
    index: Int,
): AnalyzedPageMarker {
    val prefix = "marker[$index]"
    return AnalyzedPageMarker(
        role = reader.readString("$prefix role"),
        family = reader.readString("$prefix family"),
        id = reader.readUInt("$prefix id"),
        hammingErrors = reader.readUInt("$prefix hamming errors"),
        decisionMargin = reader.readDouble("$prefix decision margin"),
        center =
            AnalyzedPagePoint(
                x = reader.readDouble("$prefix center x"),
                y = reader.readDouble("$prefix center y"),
            ),
        corners =
            List(reader.readInt("$prefix corner count")) { cornerIndex ->
                AnalyzedPagePoint(
                    x = reader.readDouble("$prefix corner[$cornerIndex] x"),
                    y = reader.readDouble("$prefix corner[$cornerIndex] y"),
                )
            },
    )
}
