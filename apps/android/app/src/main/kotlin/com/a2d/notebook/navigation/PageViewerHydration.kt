package com.a2d.notebook.navigation

import com.a2d.notebook.feature.library.PageViewerState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.A2dClient

internal suspend fun hydratePageViewerMetadata(
    current: PageViewerState,
    pageId: String,
    requestedScanId: String?,
    client: A2dClient,
): PageViewerState {
    val snapshot = withContext(Dispatchers.IO) {
        client.loadPageViewerSnapshot(pageId, requestedScanId)
    }
    return current.copy(
        pageId = snapshot.pageId,
        statusLabel = snapshot.pageStatus,
        updatedSummary = snapshot.updatedAtMs.toString(),
        preferredScanId = snapshot.selectedScanId,
        hasOriginalImage = snapshot.originalAsset != null,
        hasCorrectedImage = snapshot.correctedAsset != null,
        displayAssetId = snapshot.displayAsset?.assetId,
        displayAssetMediaType = snapshot.displayAsset?.mediaType,
        displayAssetByteLength = snapshot.displayAsset?.byteLength,
        needsReview = snapshot.needsReview,
        viewerApiConnected = true,
    )
}
