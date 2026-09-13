package com.a2d.notebook.feature.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
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

object ImportLibraryTestTags {
    const val TITLE = "imports_title"
    const val LOCAL_FIRST = "imports_local_first"
    const val SUMMARY = "imports_summary"
    const val API_BOUNDARY = "imports_api_boundary"
    const val EMPTY_STATE = "imports_empty_state"
    const val ITEM = "imports_item"
    const val SOURCE = "imports_source"
    const val OPEN_PAGE = "imports_open_page"
    const val RETRY = "imports_retry"
    const val MOVE_TO_REVIEW = "imports_move_to_review"
}

data class ImportLibraryState(
    val items: List<ImportItemSummary> = emptyList(),
    val importApiConnected: Boolean = false,
) {
    val totalCount: Int
        get() = items.size

    val importedPageCount: Int
        get() = items.sumOf { item -> item.pageCount }

    val conflictCount: Int
        get() = items.sumOf { item -> item.conflictCount }

    val retryableCount: Int
        get() = items.count { item -> item.retryAvailable }
}

data class ImportItemSummary(
    val importId: String,
    val title: String,
    val sourceLabel: String,
    val importedSummary: String,
    val statusLabel: String,
    val pageCount: Int,
    val conflictCount: Int,
    val consequenceSummary: String,
    val pageId: String? = null,
    val retryAvailable: Boolean = false,
    val moveToReviewAvailable: Boolean = true,
)

@Composable
fun ImportLibraryScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    state: ImportLibraryState = ImportLibraryState(),
    onOpenPage: (String) -> Unit = {},
    onRetryImport: (String) -> Unit = {},
    onMoveToReview: (String) -> Unit = {},
) {
    ImportLibraryContent(
        state = state,
        onBack = onBack,
        onOpenPage = onOpenPage,
        onRetryImport = onRetryImport,
        onMoveToReview = onMoveToReview,
        modifier = modifier,
    )
}

@Composable
fun ImportLibraryContent(
    state: ImportLibraryState,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    onOpenPage: (String) -> Unit = {},
    onRetryImport: (String) -> Unit = {},
    onMoveToReview: (String) -> Unit = {},
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
            text = stringResource(R.string.imports_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(ImportLibraryTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.imports_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(ImportLibraryTestTags.LOCAL_FIRST),
        )
        ImportSummaryCard(state)
        ImportBoundaryCard(state.importApiConnected)
        if (state.items.isEmpty()) {
            Card(Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.EMPTY_STATE)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.imports_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.imports_empty_body))
                }
            }
        } else {
            state.items.forEach { item ->
                ImportItemCard(
                    item = item,
                    onOpenPage = onOpenPage,
                    onRetryImport = onRetryImport,
                    onMoveToReview = onMoveToReview,
                )
            }
        }
    }
}

@Composable
private fun ImportSummaryCard(state: ImportLibraryState) {
    Card(Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.SUMMARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.imports_summary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.imports_total_count, state.totalCount))
            Text(stringResource(R.string.imports_page_count, state.importedPageCount))
            Text(stringResource(R.string.imports_conflict_count, state.conflictCount))
            Text(stringResource(R.string.imports_retryable_count, state.retryableCount))
        }
    }
}

@Composable
private fun ImportBoundaryCard(apiConnected: Boolean) {
    Card(Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.API_BOUNDARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.imports_boundary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                if (apiConnected) {
                    stringResource(R.string.imports_api_connected)
                } else {
                    stringResource(R.string.imports_api_boundary)
                },
            )
            Text(stringResource(R.string.imports_no_silent_merge))
        }
    }
}

@Composable
private fun ImportItemCard(
    item: ImportItemSummary,
    onOpenPage: (String) -> Unit,
    onRetryImport: (String) -> Unit,
    onMoveToReview: (String) -> Unit,
) {
    Card(Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.ITEM)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(item.title, style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.imports_item_id, item.importId))
            Text(
                text = stringResource(R.string.imports_source, item.sourceLabel),
                modifier = Modifier.testTag(ImportLibraryTestTags.SOURCE),
            )
            Text(stringResource(R.string.imports_item_status, item.statusLabel))
            Text(stringResource(R.string.imports_item_imported, item.importedSummary))
            Text(stringResource(R.string.imports_item_pages, item.pageCount))
            Text(stringResource(R.string.imports_item_conflicts, item.conflictCount))
            Text(stringResource(R.string.imports_item_consequence, item.consequenceSummary))
            item.pageId?.let { pageId ->
                OutlinedButton(
                    onClick = { onOpenPage(pageId) },
                    modifier = Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.OPEN_PAGE),
                ) {
                    Text(stringResource(R.string.imports_open_page))
                }
            }
            OutlinedButton(
                onClick = { onRetryImport(item.importId) },
                enabled = item.retryAvailable,
                modifier = Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.RETRY),
            ) {
                Text(stringResource(R.string.imports_retry))
            }
            Button(
                onClick = { onMoveToReview(item.importId) },
                enabled = item.moveToReviewAvailable,
                modifier = Modifier.fillMaxWidth().testTag(ImportLibraryTestTags.MOVE_TO_REVIEW),
            ) {
                Text(stringResource(R.string.imports_move_to_review))
            }
        }
    }
}
