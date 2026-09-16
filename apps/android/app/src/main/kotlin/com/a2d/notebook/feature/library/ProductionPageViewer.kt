package com.a2d.notebook.feature.library

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.a2d.notebook.feature.ocr.FfiAndroidOcrReadback
import com.a2d.notebook.feature.ocr.OcrPresentationState
import com.a2d.notebook.feature.ocr.OcrRegionOverlayState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.SearchOcrTextRequest

/**
 * Production Page Viewer composition. Durable OCR identity and output are read from the active
 * Rust-owned library; the presentation-only [PageViewerScreen] remains injectable for previews
 * and focused UI tests.
 */
@Composable
fun ProductionPageViewerScreen(
    pageId: String,
    client: A2dClient?,
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    onOpenNeedsReview: () -> Unit,
) {
    var state by remember(pageId, client) { mutableStateOf(PageViewerState(pageId = pageId)) }

    LaunchedEffect(pageId, client) {
        state =
            if (client == null) {
                PageViewerState(pageId = pageId)
            } else {
                withContext(Dispatchers.IO) {
                    loadProductionPageViewerState(client = client, pageId = pageId)
                }
            }
    }

    PageViewerScreen(
        pageId = pageId,
        onBack = onBack,
        onOpenVersions = onOpenVersions,
        onOpenNeedsReview = onOpenNeedsReview,
        state = state,
    )
}

internal fun loadProductionPageViewerState(
    client: A2dClient,
    pageId: String,
): PageViewerState {
    val scanId = findPersistedScanIdForPage(client = client, pageId = pageId)
        ?: return PageViewerState(pageId = pageId, viewerApiConnected = true)
    val output = FfiAndroidOcrReadback(client).loadLatestOcrOutput(scanId = scanId)
    return PageViewerState(
        pageId = pageId,
        preferredScanId = scanId,
        hasRecognizedText = output.latestRun != null,
        ocrState = output.toPresentationState(),
        ocrRegionOverlay = OcrRegionOverlayState.fromPersisted(output),
        viewerApiConnected = true,
    )
}

/**
 * Resolve the page's persisted scan through the existing Rust search index. This is deliberately
 * bounded and read-only; R2's subsequent state-owner slice will replace this bridge with a direct
 * page/preferred-scan readback API while preserving the production composition established here.
 */
private fun findPersistedScanIdForPage(client: A2dClient, pageId: String): String? {
    val probes = listOf("a", "e", "i", "o", "u", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9")
    probes.forEach { query ->
        val hit =
            client
                .searchOcrText(SearchOcrTextRequest(query = query, limit = PAGE_SCAN_PROBE_LIMIT))
                .hits
                .firstOrNull { it.pageId == pageId }
        if (hit != null) return hit.scanId
    }
    return null
}

private const val PAGE_SCAN_PROBE_LIMIT: UInt = 200u
