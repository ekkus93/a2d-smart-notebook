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

object TrashWorkflowTestTags {
    const val TITLE = "trash_title"
    const val LOCAL_FIRST = "trash_local_first"
    const val SUMMARY = "trash_summary"
    const val API_BOUNDARY = "trash_api_boundary"
    const val EMPTY_STATE = "trash_empty_state"
    const val ITEM = "trash_item"
    const val CONSEQUENCE = "trash_consequence"
    const val OPEN_PAGE = "trash_open_page"
    const val RESTORE = "trash_restore"
    const val PERMANENT_DELETE = "trash_permanent_delete"
}

data class TrashState(
    val items: List<TrashItemSummary> = emptyList(),
    val trashApiConnected: Boolean = false,
) {
    val totalCount: Int
        get() = items.size

    val restorableCount: Int
        get() = items.count { item -> item.restoreAvailable }

    val permanentDeleteEligibleCount: Int
        get() = items.count { item -> item.permanentDeleteAvailable }
}

data class TrashItemSummary(
    val itemId: String,
    val kindLabel: String,
    val title: String,
    val locationSummary: String,
    val deletedSummary: String,
    val retainedAssetSummary: String,
    val restoreConsequence: String,
    val permanentDeleteConsequence: String,
    val pageId: String? = null,
    val restoreAvailable: Boolean = true,
    val permanentDeleteAvailable: Boolean = true,
)

@Composable
fun TrashScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    state: TrashState = TrashState(),
    onOpenPage: (String) -> Unit = {},
    onRestore: (String) -> Unit = {},
    onPermanentDelete: (String) -> Unit = {},
) {
    TrashContent(
        state = state,
        onBack = onBack,
        onOpenPage = onOpenPage,
        onRestore = onRestore,
        onPermanentDelete = onPermanentDelete,
        modifier = modifier,
    )
}

@Composable
fun TrashContent(
    state: TrashState,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    onOpenPage: (String) -> Unit = {},
    onRestore: (String) -> Unit = {},
    onPermanentDelete: (String) -> Unit = {},
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
            text = stringResource(R.string.trash_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(TrashWorkflowTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.trash_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(TrashWorkflowTestTags.LOCAL_FIRST),
        )
        TrashSummaryCard(state)
        TrashBoundaryCard(state.trashApiConnected)
        if (state.items.isEmpty()) {
            Card(Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.EMPTY_STATE)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.trash_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.trash_empty_body))
                }
            }
        } else {
            state.items.forEach { item ->
                TrashItemCard(
                    item = item,
                    onOpenPage = onOpenPage,
                    onRestore = onRestore,
                    onPermanentDelete = onPermanentDelete,
                )
            }
        }
    }
}

@Composable
private fun TrashSummaryCard(state: TrashState) {
    Card(Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.SUMMARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.trash_summary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.trash_total_count, state.totalCount))
            Text(stringResource(R.string.trash_restorable_count, state.restorableCount))
            Text(stringResource(R.string.trash_permanent_delete_count, state.permanentDeleteEligibleCount))
        }
    }
}

@Composable
private fun TrashBoundaryCard(apiConnected: Boolean) {
    Card(Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.API_BOUNDARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.trash_boundary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                if (apiConnected) {
                    stringResource(R.string.trash_api_connected)
                } else {
                    stringResource(R.string.trash_api_boundary)
                },
            )
            Text(stringResource(R.string.trash_no_id_reuse))
        }
    }
}

@Composable
private fun TrashItemCard(
    item: TrashItemSummary,
    onOpenPage: (String) -> Unit,
    onRestore: (String) -> Unit,
    onPermanentDelete: (String) -> Unit,
) {
    Card(Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.ITEM)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(item.title, style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.trash_item_kind, item.kindLabel))
            Text(stringResource(R.string.trash_item_id, item.itemId))
            Text(stringResource(R.string.trash_item_location, item.locationSummary))
            Text(stringResource(R.string.trash_item_deleted, item.deletedSummary))
            Text(stringResource(R.string.trash_item_retained_assets, item.retainedAssetSummary))
            Column(Modifier.testTag(TrashWorkflowTestTags.CONSEQUENCE)) {
                Text(stringResource(R.string.trash_restore_consequence, item.restoreConsequence))
                Text(stringResource(R.string.trash_permanent_delete_consequence, item.permanentDeleteConsequence))
            }
            item.pageId?.let { pageId ->
                OutlinedButton(
                    onClick = { onOpenPage(pageId) },
                    modifier = Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.OPEN_PAGE),
                ) {
                    Text(stringResource(R.string.trash_open_page))
                }
            }
            Button(
                onClick = { onRestore(item.itemId) },
                enabled = item.restoreAvailable,
                modifier = Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.RESTORE),
            ) {
                Text(stringResource(R.string.trash_restore))
            }
            OutlinedButton(
                onClick = { onPermanentDelete(item.itemId) },
                enabled = item.permanentDeleteAvailable,
                modifier = Modifier.fillMaxWidth().testTag(TrashWorkflowTestTags.PERMANENT_DELETE),
            ) {
                Text(stringResource(R.string.trash_permanent_delete))
            }
        }
    }
}
