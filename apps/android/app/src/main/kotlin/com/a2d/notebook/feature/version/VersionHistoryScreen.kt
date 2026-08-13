package com.a2d.notebook.feature.version

import android.graphics.BitmapFactory
import java.text.DateFormat
import java.util.Date
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.a2d.notebook.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.PageVersionComparison
import uniffi.a2d_ffi.PageVersionRecord
import uniffi.a2d_ffi.ScanRevisionDecision

object VersionHistoryTestTags {
    const val TIMELINE = "version_history_timeline"
    const val PREFERRED = "version_history_preferred"
    const val COMPARISON = "version_history_comparison"
    const val CHANGED_REGIONS = "version_history_changed_regions"
    const val KEEP_BOTH = "version_history_keep_both"
    const val SET_PREFERRED = "version_history_set_preferred"
    const val PHYSICAL_COPY = "version_history_physical_copy"
    const val WRONG_SCAN = "version_history_wrong_scan"
    const val MOVE_TO_REVIEW = "version_history_move_to_review"
}

private enum class ComparisonMode {
    SideBySide,
    Overlay,
}

@Composable
fun VersionHistoryScreen(
    pageId: String,
    onBack: () -> Unit,
    viewModel: VersionHistoryViewModel = viewModel(),
) {
    val state by viewModel.state
    LaunchedEffect(pageId) { viewModel.load(pageId) }
    VersionHistoryContent(
        pageId = pageId,
        state = state,
        onBack = onBack,
        onSelectVersion = viewModel::selectVersion,
        onLoadMore = viewModel::loadMore,
        onDecision = viewModel::applyDecision,
        onMoveToReview = viewModel::moveSelectedToReview,
    )
}

@Composable
internal fun VersionHistoryContent(
    pageId: String,
    state: VersionHistoryUiState,
    onBack: () -> Unit,
    onSelectVersion: (String) -> Unit,
    onLoadMore: () -> Unit,
    onDecision: (ScanRevisionDecision, String?) -> Unit,
    onMoveToReview: () -> Unit,
) {
    var comparisonMode by remember { mutableStateOf(ComparisonMode.SideBySide) }
    val timeline = state.timeline
    val preferred = state.preferredVersion
    val selected = state.selectedVersion

    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(16.dp).testTag(VersionHistoryTestTags.TIMELINE),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
                Text(
                    text = stringResource(R.string.version_history_title),
                    style = MaterialTheme.typography.titleLarge,
                )
            }
            Text(
                text = stringResource(R.string.version_history_page, pageId),
                style = MaterialTheme.typography.bodySmall,
            )
        }

        state.error?.let { error ->
            item { Text(error, color = MaterialTheme.colorScheme.error) }
        }

        if (state.loading && timeline == null) {
            item {
                Box(Modifier.fillMaxWidth().height(96.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            }
        } else if (timeline == null || timeline.items.isEmpty()) {
            item { Text(stringResource(R.string.version_history_empty)) }
        } else {
            items(timeline.items, key = { it.scanId }) { version ->
                VersionTimelineCard(
                    version = version,
                    selected = version.scanId == state.selectedScanId,
                    onClick = { onSelectVersion(version.scanId) },
                )
            }
            if (timeline.hasMore) {
                item {
                    OutlinedButton(onClick = onLoadMore, enabled = !state.loading) {
                        Text(stringResource(R.string.version_history_load_more))
                    }
                }
            }

            if (preferred != null && selected != null && preferred.scanId != selected.scanId) {
                item {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(onClick = { comparisonMode = ComparisonMode.SideBySide }) {
                            Text(stringResource(R.string.version_history_side_by_side))
                        }
                        OutlinedButton(onClick = { comparisonMode = ComparisonMode.Overlay }) {
                            Text(stringResource(R.string.version_history_overlay))
                        }
                    }
                }
                item {
                    VersionComparisonPanel(
                        preferred = preferred,
                        candidate = selected,
                        comparison = state.comparison,
                        mode = comparisonMode,
                    )
                }
                item {
                    VersionActions(
                        state = state,
                        onDecision = onDecision,
                        onMoveToReview = onMoveToReview,
                    )
                }
            } else if (selected?.preferred == true) {
                item {
                    Text(
                        text = stringResource(R.string.version_history_selected_preferred),
                        modifier = Modifier.testTag(VersionHistoryTestTags.PREFERRED),
                    )
                }
            }

            state.queuedReview?.let { reviewItem ->
                item { Text(stringResource(R.string.version_history_review_queued, reviewItem.id)) }
            }
        }
    }
}

@Composable
private fun VersionTimelineCard(
    version: PageVersionRecord,
    selected: Boolean,
    onClick: () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(version.scanId, style = MaterialTheme.typography.labelLarge)
                if (version.preferred) {
                    Text(
                        stringResource(R.string.version_history_preferred),
                        modifier = Modifier.testTag(VersionHistoryTestTags.PREFERRED),
                    )
                }
            }
            val capturedAt = remember(version.capturedAtMs) {
                DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT)
                    .format(Date(version.capturedAtMs))
            }
            Text(stringResource(R.string.version_history_captured_at, capturedAt))
            Text(stringResource(R.string.version_history_quality, version.qualityStatus.name))
            version.physicalCopyId?.let {
                Text(stringResource(R.string.version_history_physical_copy_id, it))
            }
            version.supersedesScanId?.let {
                Text(stringResource(R.string.version_history_supersedes, it))
            }
            version.decisionCode?.let {
                Text(stringResource(R.string.version_history_decision, it))
            }
            if (selected) {
                Text(stringResource(R.string.version_history_selected))
            }
        }
    }
}

@Composable
private fun VersionComparisonPanel(
    preferred: PageVersionRecord,
    candidate: PageVersionRecord,
    comparison: PageVersionComparison?,
    mode: ComparisonMode,
) {
    Card(modifier = Modifier.fillMaxWidth().testTag(VersionHistoryTestTags.COMPARISON)) {
        Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (mode == ComparisonMode.SideBySide) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    VersionImage(
                        path = preferred.displayAssetPath(),
                        modifier = Modifier.weight(1f).height(220.dp),
                    )
                    VersionImage(
                        path = candidate.displayAssetPath(),
                        modifier = Modifier.weight(1f).height(220.dp),
                    )
                }
            } else {
                OverlayVersionImage(
                    candidate = candidate,
                    comparison = comparison,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (comparison == null) {
                CircularProgressIndicator()
            } else {
                val evidence = comparison.evidence
                Text(
                    stringResource(
                        R.string.version_history_difference_summary,
                        evidence.changedCellCount.toInt(),
                        evidence.changeRegions.size,
                    ),
                    modifier = Modifier.testTag(VersionHistoryTestTags.CHANGED_REGIONS),
                )
                Text(stringResource(R.string.version_history_confidence, evidence.confidence.name))
                Text(
                    stringResource(R.string.version_history_calibration_warning),
                    color = MaterialTheme.colorScheme.tertiary,
                )
            }
        }
    }
}

@Composable
private fun VersionActions(
    state: VersionHistoryUiState,
    onDecision: (ScanRevisionDecision, String?) -> Unit,
    onMoveToReview: () -> Unit,
) {
    val proposal = state.proposal
    val selected = state.selectedVersion
    val decisionEnabled = !state.loading && !state.mutating && proposal != null
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (proposal?.allowedDecisions?.contains(ScanRevisionDecision.SAVE_AS_NEW_VERSION) == true) {
            Button(
                onClick = { onDecision(ScanRevisionDecision.SAVE_AS_NEW_VERSION, null) },
                enabled = decisionEnabled,
                modifier = Modifier.fillMaxWidth().testTag(VersionHistoryTestTags.KEEP_BOTH),
            ) { Text(stringResource(R.string.version_history_keep_both)) }
        }
        if (proposal?.allowedDecisions?.contains(ScanRevisionDecision.REPLACE_PREFERRED) == true) {
            OutlinedButton(
                onClick = { onDecision(ScanRevisionDecision.REPLACE_PREFERRED, null) },
                enabled = decisionEnabled,
                modifier = Modifier.fillMaxWidth().testTag(VersionHistoryTestTags.SET_PREFERRED),
            ) { Text(stringResource(R.string.version_history_set_preferred)) }
        }
        if (proposal?.allowedDecisions?.contains(ScanRevisionDecision.ANOTHER_PHYSICAL_COPY) == true) {
            OutlinedButton(
                onClick = { onDecision(ScanRevisionDecision.ANOTHER_PHYSICAL_COPY, null) },
                enabled = decisionEnabled,
                modifier = Modifier.fillMaxWidth().testTag(VersionHistoryTestTags.PHYSICAL_COPY),
            ) { Text(stringResource(R.string.version_history_mark_physical_copy)) }
        }
        if (proposal?.allowedDecisions?.contains(ScanRevisionDecision.WRONG_SCAN) == true) {
            OutlinedButton(
                onClick = { onDecision(ScanRevisionDecision.WRONG_SCAN, null) },
                enabled = decisionEnabled,
                modifier = Modifier.fillMaxWidth().testTag(VersionHistoryTestTags.WRONG_SCAN),
            ) { Text(stringResource(R.string.version_history_wrong_scan)) }
        }
        if (selected != null && selected.decisionCode == null) {
            OutlinedButton(
                onClick = onMoveToReview,
                enabled = !state.loading && !state.mutating,
                modifier = Modifier.fillMaxWidth().testTag(VersionHistoryTestTags.MOVE_TO_REVIEW),
            ) { Text(stringResource(R.string.version_history_move_to_review)) }
        }
    }
}

private fun PageVersionRecord.displayAssetPath(): String =
    thumbnailAssetPath ?: correctedAssetPath ?: originalAssetPath

@Composable
private fun VersionImage(path: String, modifier: Modifier = Modifier) {
    val bitmap by versionBitmap(path)
    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        val current = bitmap
        if (current == null) {
            Text(stringResource(R.string.version_history_preview_unavailable))
        } else {
            Image(
                bitmap = current,
                contentDescription = stringResource(R.string.version_history_preview_content_description),
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Fit,
            )
        }
    }
}

@Composable
private fun OverlayVersionImage(
    candidate: PageVersionRecord,
    comparison: PageVersionComparison?,
    modifier: Modifier = Modifier,
) {
    val bitmap by versionBitmap(candidate.displayAssetPath())
    val overlayColor = MaterialTheme.colorScheme.error
    val current = bitmap
    if (current == null) {
        Box(modifier = modifier.height(220.dp), contentAlignment = Alignment.Center) {
            Text(stringResource(R.string.version_history_preview_unavailable))
        }
        return
    }
    val aspectRatio = current.width.toFloat() / current.height.toFloat()
    Box(modifier = modifier.aspectRatio(aspectRatio), contentAlignment = Alignment.Center) {
        Image(
            bitmap = current,
            contentDescription = stringResource(R.string.version_history_overlay_content_description),
            modifier = Modifier.fillMaxSize(),
            contentScale = ContentScale.FillBounds,
        )
        val currentComparison = comparison
        if (currentComparison != null) {
            Canvas(Modifier.fillMaxSize()) {
                val columns = currentComparison.gridColumns.toFloat()
                val rows = currentComparison.gridRows.toFloat()
                currentComparison.evidence.changeRegions.forEach { region ->
                    val left = size.width * region.leftColumn.toFloat() / columns
                    val top = size.height * region.topRow.toFloat() / rows
                    val right = size.width * region.rightColumnExclusive.toFloat() / columns
                    val bottom = size.height * region.bottomRowExclusive.toFloat() / rows
                    drawRect(
                        color = overlayColor,
                        topLeft = Offset(left, top),
                        size = Size(right - left, bottom - top),
                        style = Stroke(width = 3.dp.toPx()),
                    )
                }
            }
        }
    }
}

@Composable
private fun versionBitmap(path: String): androidx.compose.runtime.State<ImageBitmap?> {
    val bitmap = remember(path) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(path) {
        bitmap.value = withContext(Dispatchers.IO) { BitmapFactory.decodeFile(path)?.asImageBitmap() }
    }
    return bitmap
}
