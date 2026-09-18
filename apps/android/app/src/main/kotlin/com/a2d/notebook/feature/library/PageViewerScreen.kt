package com.a2d.notebook.feature.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.feature.ocr.OcrPresentationState
import com.a2d.notebook.feature.ocr.OcrPresentationStatus
import com.a2d.notebook.feature.ocr.OcrRegionOverlayCard
import com.a2d.notebook.feature.ocr.OcrRegionOverlayState

object PageViewerTestTags {
    const val TITLE = "page_viewer_title"
    const val LOCAL_FIRST = "page_viewer_local_first"
    const val SUMMARY = "page_viewer_summary"
    const val API_BOUNDARY = "page_viewer_api_boundary"
    const val ORIGINAL = "page_viewer_original"
    const val CORRECTED = "page_viewer_corrected"
    const val TEXT = "page_viewer_text"
    const val OCR_START = "page_viewer_ocr_start"
    const val OCR_RETRY = "page_viewer_ocr_retry"
    const val OCR_CANCEL = "page_viewer_ocr_cancel"
    const val SPLIT = "page_viewer_split"
    const val METADATA = "page_viewer_metadata"
    const val VERSIONS = "page_viewer_versions"
    const val ANNOTATIONS = "page_viewer_annotations"
    const val RELATED = "page_viewer_related"
    const val SKILL_RESULTS = "page_viewer_skill_results"
    const val OPEN_VERSIONS = "page_viewer_open_versions"
    const val OPEN_NEEDS_REVIEW = "page_viewer_open_needs_review"
}

data class PageViewerState(
    val pageId: String,
    val notebookName: String? = null,
    val visiblePageLabel: String? = null,
    val statusLabel: String = "unknown",
    val updatedSummary: String? = null,
    val preferredScanId: String? = null,
    val activeOcrJobId: String? = null,
    val hasOriginalImage: Boolean = false,
    val hasCorrectedImage: Boolean = false,
    val displayAssetId: String? = null,
    val displayAssetRelativePath: String? = null,
    val displayAssetMediaType: String? = null,
    val displayAssetByteLength: ULong? = null,
    val hasRecognizedText: Boolean = false,
    val ocrState: OcrPresentationState = OcrPresentationState(),
    val ocrRegionOverlay: OcrRegionOverlayState = OcrRegionOverlayState(),
    val annotationCount: Int = 0,
    val relatedPageCount: Int = 0,
    val skillResultCount: Int = 0,
    val needsReview: Boolean = false,
    val viewerApiConnected: Boolean = false,
)

@Composable
fun PageViewerScreen(
    pageId: String,
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    onOpenNeedsReview: () -> Unit,
    modifier: Modifier = Modifier,
    state: PageViewerState = PageViewerState(pageId = pageId),
    onStartOcr: (String) -> Unit = {},
    onRetryOcr: (String) -> Unit = {},
    onCancelOcr: (String) -> Unit = {},
) {
    PageViewerContent(
        state = state,
        onBack = onBack,
        onOpenVersions = onOpenVersions,
        onOpenNeedsReview = onOpenNeedsReview,
        modifier = modifier,
        onStartOcr = onStartOcr,
        onRetryOcr = onRetryOcr,
        onCancelOcr = onCancelOcr,
    )
}

@Composable
fun PageViewerContent(
    state: PageViewerState,
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    onOpenNeedsReview: () -> Unit,
    modifier: Modifier = Modifier,
    onStartOcr: (String) -> Unit = {},
    onRetryOcr: (String) -> Unit = {},
    onCancelOcr: (String) -> Unit = {},
) {
    Column(
        modifier =
            modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        OutlinedButton(onClick = onBack) {
            Text(stringResource(R.string.common_back))
        }
        Text(
            text = stringResource(R.string.page_viewer_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(PageViewerTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.page_viewer_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(PageViewerTestTags.LOCAL_FIRST),
        )

        PageViewerSummaryCard(state)

        if (!state.viewerApiConnected) {
            Card(Modifier.fillMaxWidth().testTag(PageViewerTestTags.API_BOUNDARY)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.page_viewer_boundary_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.page_viewer_api_boundary))
                }
            }
        }

        PageViewerSection(
            title = stringResource(R.string.page_viewer_original_title),
            detail =
                if (state.hasOriginalImage) {
                    stringResource(R.string.page_viewer_original_available)
                } else {
                    stringResource(R.string.page_viewer_original_unavailable)
                },
            testTag = PageViewerTestTags.ORIGINAL,
        )
        PageViewerSection(
            title = stringResource(R.string.page_viewer_corrected_title),
            detail =
                if (state.hasCorrectedImage) {
                    stringResource(R.string.page_viewer_corrected_available)
                } else {
                    stringResource(R.string.page_viewer_corrected_unavailable)
                },
            testTag = PageViewerTestTags.CORRECTED,
        )
        PageViewerOcrTextSection(
            state = state,
            onStartOcr = onStartOcr,
            onRetryOcr = onRetryOcr,
            onCancelOcr = onCancelOcr,
        )
        OcrRegionOverlayCard(state.ocrRegionOverlay)
        PageViewerSection(
            title = stringResource(R.string.page_viewer_split_title),
            detail = stringResource(R.string.page_viewer_split_boundary),
            testTag = PageViewerTestTags.SPLIT,
        )
        PageViewerSection(
            title = stringResource(R.string.page_viewer_metadata_title),
            detail = stringResource(R.string.page_viewer_metadata_detail, state.pageId, state.statusLabel),
            testTag = PageViewerTestTags.METADATA,
        )
        PageViewerSection(
            title = stringResource(R.string.page_viewer_annotations_title),
            detail = stringResource(R.string.page_viewer_annotations_detail, state.annotationCount),
            testTag = PageViewerTestTags.ANNOTATIONS,
        )
        PageViewerSection(
            title = stringResource(R.string.page_viewer_related_title),
            detail = stringResource(R.string.page_viewer_related_detail, state.relatedPageCount),
            testTag = PageViewerTestTags.RELATED,
        )
        PageViewerSection(
            title = stringResource(R.string.page_viewer_skill_results_title),
            detail = stringResource(R.string.page_viewer_skill_results_detail, state.skillResultCount),
            testTag = PageViewerTestTags.SKILL_RESULTS,
        )

        Card(Modifier.fillMaxWidth().testTag(PageViewerTestTags.VERSIONS)) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = stringResource(R.string.page_viewer_versions_title),
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(stringResource(R.string.page_viewer_versions_detail))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(
                        onClick = { onOpenVersions(state.pageId) },
                        modifier = Modifier.testTag(PageViewerTestTags.OPEN_VERSIONS),
                    ) {
                        Text(stringResource(R.string.page_viewer_open_versions))
                    }
                    OutlinedButton(
                        onClick = onOpenNeedsReview,
                        modifier = Modifier.testTag(PageViewerTestTags.OPEN_NEEDS_REVIEW),
                    ) {
                        Text(stringResource(R.string.page_viewer_open_needs_review))
                    }
                }
            }
        }
    }
}

@Composable
private fun PageViewerSummaryCard(state: PageViewerState) {
    Card(Modifier.fillMaxWidth().testTag(PageViewerTestTags.SUMMARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.page_viewer_summary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.page_viewer_page_id, state.pageId))
            state.notebookName?.let { notebook ->
                Text(stringResource(R.string.page_viewer_notebook, notebook))
            }
            state.visiblePageLabel?.let { label ->
                Text(stringResource(R.string.page_viewer_visible_page, label))
            }
            state.preferredScanId?.let { scanId ->
                Text(stringResource(R.string.page_viewer_scan_id, scanId))
            }
            Text(stringResource(R.string.page_viewer_status, state.statusLabel))
            state.updatedSummary?.let { updated ->
                Text(stringResource(R.string.page_viewer_updated, updated))
            }
            if (state.needsReview) {
                Text(stringResource(R.string.page_viewer_needs_review_badge))
            }
        }
    }
}

@Composable
private fun PageViewerOcrTextSection(
    state: PageViewerState,
    onStartOcr: (String) -> Unit,
    onRetryOcr: (String) -> Unit,
    onCancelOcr: (String) -> Unit,
) {
    Card(Modifier.fillMaxWidth().testTag(PageViewerTestTags.TEXT)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.page_viewer_text_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(pageViewerOcrDetail(state))
            state.ocrState.runId?.let { runId ->
                Text(stringResource(R.string.page_viewer_text_run_id, runId))
            }
            state.ocrState.providerLabel?.let { provider ->
                Text(stringResource(R.string.page_viewer_text_provider, provider))
            }
            state.ocrState.modelName?.let { model ->
                Text(stringResource(R.string.page_viewer_text_model, model))
            }
            if (state.ocrState.recognizedRegionCount > 0) {
                Text(
                    stringResource(
                        R.string.page_viewer_text_region_count,
                        state.ocrState.recognizedRegionCount,
                    ),
                )
            }
            state.ocrState.textPreview?.let { preview ->
                Text(stringResource(R.string.page_viewer_text_preview, preview))
            }
            state.ocrState.message?.let { message ->
                Text(stringResource(R.string.page_viewer_text_message, message))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(
                    enabled = state.preferredScanId != null &&
                        state.ocrState.status == OcrPresentationStatus.NotStarted,
                    onClick = { state.preferredScanId?.let(onStartOcr) },
                    modifier = Modifier.testTag(PageViewerTestTags.OCR_START),
                ) {
                    Text(stringResource(R.string.page_viewer_start_ocr))
                }
                if (state.ocrState.retryAvailable) {
                    OutlinedButton(
                        enabled = state.preferredScanId != null,
                        onClick = { state.preferredScanId?.let(onRetryOcr) },
                        modifier = Modifier.testTag(PageViewerTestTags.OCR_RETRY),
                    ) {
                        Text(stringResource(R.string.common_retry))
                    }
                }
                if (state.ocrState.cancelAvailable) {
                    OutlinedButton(
                        enabled = state.preferredScanId != null && state.activeOcrJobId != null,
                        onClick = { state.preferredScanId?.let(onCancelOcr) },
                        modifier = Modifier.testTag(PageViewerTestTags.OCR_CANCEL),
                    ) {
                        Text(stringResource(R.string.common_cancel))
                    }
                }
            }
        }
    }
}

@Composable
private fun pageViewerOcrDetail(state: PageViewerState): String =
    when (state.ocrState.status) {
        OcrPresentationStatus.NotStarted ->
            if (state.hasRecognizedText) {
                stringResource(R.string.page_viewer_text_available)
            } else {
                stringResource(R.string.page_viewer_text_not_started)
            }

        OcrPresentationStatus.Preparing -> stringResource(R.string.page_viewer_text_preparing)
        OcrPresentationStatus.Recognizing -> stringResource(R.string.page_viewer_text_recognizing)
        OcrPresentationStatus.Recording -> stringResource(R.string.page_viewer_text_recording)
        OcrPresentationStatus.Detected -> stringResource(R.string.page_viewer_text_available)
        OcrPresentationStatus.NoTextDetected -> stringResource(R.string.page_viewer_text_no_text_detected)
        OcrPresentationStatus.Unavailable ->
            stringResource(
                R.string.page_viewer_text_unavailable_reason,
                state.ocrState.unavailableReason ?: stringResource(R.string.common_unknown),
            )

        OcrPresentationStatus.Failed -> stringResource(R.string.page_viewer_text_failed)
        OcrPresentationStatus.Cancelled -> stringResource(R.string.page_viewer_text_cancelled)
    }

@Composable
private fun PageViewerSection(
    title: String,
    detail: String,
    testTag: String,
) {
    Card(Modifier.fillMaxWidth().testTag(testTag)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(detail)
        }
    }
}
