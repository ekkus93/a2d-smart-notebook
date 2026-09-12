package com.a2d.notebook.feature.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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

object HomeScreenTestTags {
    const val TITLE = "home_title"
    const val LOCAL_FIRST = "home_local_first"
    const val PRIMARY_ACTIONS = "home_primary_actions"
    const val EMPTY_STATE = "home_empty_state"
    const val POPULATED_STATE = "home_populated_state"
    const val RECENT_NOTEBOOK = "home_recent_notebook"
    const val REVIEW_COUNT = "home_review_count"
    const val UNFINISHED_COUNT = "home_unfinished_count"
    const val SMART_PAGE_COUNT = "home_smart_page_count"
    const val BACKUP_STATUS = "home_backup_status"
    const val SCAN_PAGE = "home_scan_page"
    const val BATCH_SCAN = "home_batch_scan"
    const val NOTEBOOKS = "home_notebooks"
    const val SMART_PAGES = "home_smart_pages"
    const val IMPORT = "home_import"
}

data class HomeDashboardState(
    val hasLocalLibrary: Boolean = false,
    val recentNotebooks: List<HomeNotebookSummary> = emptyList(),
    val unfinishedScanCount: Int = 0,
    val needsReviewCount: Int = 0,
    val generatedSmartPageCount: Int = 0,
    val backupStatus: HomeBackupStatus = HomeBackupStatus.NotConfigured,
) {
    val hasContent: Boolean
        get() =
            hasLocalLibrary ||
                recentNotebooks.isNotEmpty() ||
                unfinishedScanCount > 0 ||
                needsReviewCount > 0 ||
                generatedSmartPageCount > 0 ||
                backupStatus != HomeBackupStatus.NotConfigured
}

data class HomeNotebookSummary(
    val id: String,
    val displayName: String,
    val pageSummary: String,
    val needsReviewCount: Int = 0,
)

enum class HomeBackupStatus {
    NotConfigured,
    Current,
    AttentionNeeded,
}

@Composable
fun HomeScreen(
    onScanPage: () -> Unit,
    onBatchScan: () -> Unit,
    onOpenNotebooks: () -> Unit,
    onCreateSmartPages: () -> Unit,
    modifier: Modifier = Modifier,
    state: HomeDashboardState = HomeDashboardState(),
    onOpenNotebook: (String) -> Unit = {},
    onOpenNeedsReview: () -> Unit = {},
    onOpenBackup: () -> Unit = {},
    onImport: () -> Unit = {},
) {
    Column(
        modifier =
            modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            text = stringResource(R.string.home_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(HomeScreenTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.home_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(HomeScreenTestTags.LOCAL_FIRST),
        )

        HomeStatusCard(
            state = state,
            onOpenNeedsReview = onOpenNeedsReview,
            onOpenBackup = onOpenBackup,
        )
        HomePrimaryActions(
            onScanPage = onScanPage,
            onBatchScan = onBatchScan,
            onOpenNotebooks = onOpenNotebooks,
            onCreateSmartPages = onCreateSmartPages,
            onImport = onImport,
        )

        if (state.hasContent) {
            PopulatedHomeState(state = state, onOpenNotebook = onOpenNotebook)
        } else {
            EmptyHomeState()
        }
    }
}

@Composable
private fun HomeStatusCard(
    state: HomeDashboardState,
    onOpenNeedsReview: () -> Unit,
    onOpenBackup: () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = stringResource(R.string.home_library_status),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                text = stringResource(R.string.home_unfinished_scans_count, state.unfinishedScanCount),
                modifier = Modifier.testTag(HomeScreenTestTags.UNFINISHED_COUNT),
            )
            Text(
                text = stringResource(R.string.home_needs_review_count, state.needsReviewCount),
                modifier = Modifier.testTag(HomeScreenTestTags.REVIEW_COUNT),
            )
            Text(
                text =
                    stringResource(
                        R.string.home_smart_page_count,
                        state.generatedSmartPageCount,
                    ),
                modifier = Modifier.testTag(HomeScreenTestTags.SMART_PAGE_COUNT),
            )
            Text(
                text = stringResource(R.string.home_backup_status, backupStatusLabel(state.backupStatus)),
                modifier = Modifier.testTag(HomeScreenTestTags.BACKUP_STATUS),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedButton(onClick = onOpenNeedsReview) {
                    Text(stringResource(R.string.home_open_needs_review))
                }
                OutlinedButton(onClick = onOpenBackup) {
                    Text(stringResource(R.string.home_open_backup))
                }
            }
        }
    }
}

@Composable
private fun backupStatusLabel(status: HomeBackupStatus): String =
    when (status) {
        HomeBackupStatus.NotConfigured -> stringResource(R.string.home_backup_not_configured)
        HomeBackupStatus.Current -> stringResource(R.string.home_backup_current)
        HomeBackupStatus.AttentionNeeded -> stringResource(R.string.home_backup_attention_needed)
    }

@Composable
private fun HomePrimaryActions(
    onScanPage: () -> Unit,
    onBatchScan: () -> Unit,
    onOpenNotebooks: () -> Unit,
    onCreateSmartPages: () -> Unit,
    onImport: () -> Unit,
) {
    Column(
        modifier = Modifier.testTag(HomeScreenTestTags.PRIMARY_ACTIONS),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = stringResource(R.string.home_primary_actions),
            style = MaterialTheme.typography.titleMedium,
        )
        Button(
            onClick = onScanPage,
            modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.SCAN_PAGE),
        ) { Text(stringResource(R.string.home_scan_page)) }
        OutlinedButton(
            onClick = onBatchScan,
            modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.BATCH_SCAN),
        ) { Text(stringResource(R.string.home_batch_scan)) }
        OutlinedButton(
            onClick = onOpenNotebooks,
            modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.NOTEBOOKS),
        ) { Text(stringResource(R.string.home_notebooks)) }
        OutlinedButton(
            onClick = onCreateSmartPages,
            modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.SMART_PAGES),
        ) { Text(stringResource(R.string.home_smart_pages)) }
        OutlinedButton(
            onClick = onImport,
            modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.IMPORT),
        ) { Text(stringResource(R.string.home_import)) }
    }
}

@Composable
private fun EmptyHomeState() {
    Card(modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.EMPTY_STATE)) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = stringResource(R.string.home_empty_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.home_empty_body))
        }
    }
}

@Composable
private fun PopulatedHomeState(
    state: HomeDashboardState,
    onOpenNotebook: (String) -> Unit,
) {
    Column(
        modifier = Modifier.testTag(HomeScreenTestTags.POPULATED_STATE),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = stringResource(R.string.home_populated_title),
            style = MaterialTheme.typography.titleMedium,
        )
        Text(
            text = stringResource(R.string.home_recent_notebooks),
            style = MaterialTheme.typography.titleSmall,
        )
        if (state.recentNotebooks.isEmpty()) {
            Text(stringResource(R.string.home_no_recent_notebooks))
        } else {
            state.recentNotebooks.forEach { notebook ->
                OutlinedButton(
                    onClick = { onOpenNotebook(notebook.id) },
                    modifier = Modifier.fillMaxWidth().testTag(HomeScreenTestTags.RECENT_NOTEBOOK),
                ) {
                    Column(modifier = Modifier.fillMaxWidth()) {
                        Text(notebook.displayName, style = MaterialTheme.typography.bodyLarge)
                        Text(notebook.pageSummary, style = MaterialTheme.typography.bodySmall)
                        if (notebook.needsReviewCount > 0) {
                            Spacer(Modifier.height(4.dp))
                            Text(
                                text =
                                    stringResource(
                                        R.string.home_notebook_needs_review_count,
                                        notebook.needsReviewCount,
                                    ),
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }
                }
            }
        }
    }
}
