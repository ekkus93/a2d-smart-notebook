package com.a2d.notebook.feature.review

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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

object NeedsReviewTestTags {
    const val TITLE = "needs_review_title"
    const val LOCAL_FIRST = "needs_review_local_first"
    const val SUMMARY = "needs_review_summary"
    const val API_BOUNDARY = "needs_review_api_boundary"
    const val EMPTY_STATE = "needs_review_empty_state"
    const val ITEM = "needs_review_item"
    const val OPEN_VERSIONS = "needs_review_open_versions"
    const val DEFER = "needs_review_defer"
    const val RESOLVE = "needs_review_resolve"
}

data class NeedsReviewQueueState(
    val totalCount: Int = 0,
    val unresolvedCount: Int = 0,
    val deferredCount: Int = 0,
    val items: List<NeedsReviewItemSummary> = emptyList(),
    val reviewApiConnected: Boolean = false,
)

data class NeedsReviewItemSummary(
    val reviewItemId: String,
    val pageId: String,
    val notebookName: String,
    val reason: String,
    val createdSummary: String,
    val resolutionOptions: List<String> = emptyList(),
)

@Composable
fun NeedsReviewScreen(
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: NeedsReviewQueueState = NeedsReviewQueueState(),
    onDefer: (String) -> Unit = {},
    onResolve: (String, String) -> Unit = { _, _ -> },
) {
    NeedsReviewContent(
        state = state,
        onBack = onBack,
        onOpenVersions = onOpenVersions,
        onDefer = onDefer,
        onResolve = onResolve,
        modifier = modifier,
    )
}

@Composable
fun NeedsReviewContent(
    state: NeedsReviewQueueState,
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    modifier: Modifier = Modifier,
    onDefer: (String) -> Unit = {},
    onResolve: (String, String) -> Unit = { _, _ -> },
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
            text = stringResource(R.string.needs_review_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(NeedsReviewTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.needs_review_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(NeedsReviewTestTags.LOCAL_FIRST),
        )
        NeedsReviewSummaryCard(state)
        if (!state.reviewApiConnected) {
            Card(Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.API_BOUNDARY)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.needs_review_boundary_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.needs_review_api_boundary))
                }
            }
        }
        if (state.items.isEmpty()) {
            Card(Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.EMPTY_STATE)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.needs_review_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.needs_review_empty_body))
                }
            }
        } else {
            state.items.forEach { item ->
                NeedsReviewItemCard(
                    item = item,
                    onOpenVersions = onOpenVersions,
                    onDefer = onDefer,
                    onResolve = onResolve,
                )
            }
        }
    }
}

@Composable
private fun NeedsReviewSummaryCard(state: NeedsReviewQueueState) {
    Card(Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.SUMMARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.needs_review_summary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.needs_review_total_count, state.totalCount))
            Text(stringResource(R.string.needs_review_unresolved_count, state.unresolvedCount))
            Text(stringResource(R.string.needs_review_deferred_count, state.deferredCount))
        }
    }
}

@Composable
private fun NeedsReviewItemCard(
    item: NeedsReviewItemSummary,
    onOpenVersions: (String) -> Unit,
    onDefer: (String) -> Unit,
    onResolve: (String, String) -> Unit,
) {
    Card(Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.ITEM)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.needs_review_item_title, item.notebookName),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.needs_review_page_id, item.pageId))
            Text(stringResource(R.string.needs_review_reason, item.reason))
            Text(stringResource(R.string.needs_review_created, item.createdSummary))
            OutlinedButton(
                onClick = { onOpenVersions(item.pageId) },
                modifier = Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.OPEN_VERSIONS),
            ) {
                Text(stringResource(R.string.needs_review_open_versions))
            }
            OutlinedButton(
                onClick = { onDefer(item.reviewItemId) },
                modifier = Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.DEFER),
            ) {
                Text(stringResource(R.string.needs_review_defer))
            }
            item.resolutionOptions.firstOrNull()?.let { resolution ->
                OutlinedButton(
                    onClick = { onResolve(item.reviewItemId, resolution) },
                    modifier = Modifier.fillMaxWidth().testTag(NeedsReviewTestTags.RESOLVE),
                ) {
                    Text(stringResource(R.string.needs_review_resolve_as, resolution))
                }
            } ?: Text(stringResource(R.string.needs_review_no_resolution_options))
        }
    }
}
