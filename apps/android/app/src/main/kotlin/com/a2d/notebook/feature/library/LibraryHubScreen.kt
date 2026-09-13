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

object LibraryHubTestTags {
    const val TITLE = "library_title"
    const val LOCAL_FIRST = "library_local_first"
    const val STATUS = "library_status"
    const val EMPTY_STATE = "library_empty_state"
    const val NOTEBOOKS = "library_notebooks"
    const val SMART_PAGES = "library_smart_pages"
    const val PAGE_SETS = "library_page_sets"
    const val COLLECTIONS = "library_collections"
    const val IMPORTS = "library_imports"
    const val NEEDS_REVIEW = "library_needs_review"
    const val TRASH = "library_trash"
}

data class LibraryHubState(
    val notebookCount: Int = 0,
    val smartPageCount: Int = 0,
    val pageSetCount: Int = 0,
    val collectionCount: Int = 0,
    val importCount: Int = 0,
    val needsReviewCount: Int = 0,
    val trashCount: Int = 0,
) {
    val totalVisibleEntries: Int
        get() =
            notebookCount +
                smartPageCount +
                pageSetCount +
                collectionCount +
                importCount +
                needsReviewCount +
                trashCount

    val hasContent: Boolean
        get() = totalVisibleEntries > 0
}

@Composable
fun LibraryHubScreen(
    onBack: () -> Unit,
    onOpenNotebooks: () -> Unit,
    onOpenSmartPages: () -> Unit,
    modifier: Modifier = Modifier,
    state: LibraryHubState = LibraryHubState(),
    onOpenPageSets: () -> Unit = {},
    onOpenCollections: () -> Unit = {},
    onOpenImports: () -> Unit = {},
    onOpenNeedsReview: () -> Unit = {},
    onOpenTrash: () -> Unit = {},
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
            text = stringResource(R.string.library_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(LibraryHubTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.library_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(LibraryHubTestTags.LOCAL_FIRST),
        )

        LibraryStatusCard(state)

        if (!state.hasContent) {
            Card(modifier = Modifier.fillMaxWidth().testTag(LibraryHubTestTags.EMPTY_STATE)) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        text = stringResource(R.string.library_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.library_empty_body))
                }
            }
        }

        LibraryDestinationButton(
            title = stringResource(R.string.library_notebooks),
            detail = stringResource(R.string.library_notebooks_detail, state.notebookCount),
            testTag = LibraryHubTestTags.NOTEBOOKS,
            onClick = onOpenNotebooks,
        )
        LibraryDestinationButton(
            title = stringResource(R.string.library_smart_pages),
            detail = stringResource(R.string.library_smart_pages_detail, state.smartPageCount),
            testTag = LibraryHubTestTags.SMART_PAGES,
            onClick = onOpenSmartPages,
        )
        LibraryDestinationButton(
            title = stringResource(R.string.library_page_sets),
            detail = stringResource(R.string.library_page_sets_detail, state.pageSetCount),
            testTag = LibraryHubTestTags.PAGE_SETS,
            onClick = onOpenPageSets,
        )
        LibraryDestinationButton(
            title = stringResource(R.string.library_collections),
            detail = stringResource(R.string.library_collections_detail, state.collectionCount),
            testTag = LibraryHubTestTags.COLLECTIONS,
            onClick = onOpenCollections,
        )
        LibraryDestinationButton(
            title = stringResource(R.string.library_imports),
            detail = stringResource(R.string.library_imports_detail, state.importCount),
            testTag = LibraryHubTestTags.IMPORTS,
            onClick = onOpenImports,
        )
        LibraryDestinationButton(
            title = stringResource(R.string.library_needs_review),
            detail = stringResource(R.string.library_needs_review_detail, state.needsReviewCount),
            testTag = LibraryHubTestTags.NEEDS_REVIEW,
            onClick = onOpenNeedsReview,
        )
        LibraryDestinationButton(
            title = stringResource(R.string.library_trash),
            detail = stringResource(R.string.library_trash_detail, state.trashCount),
            testTag = LibraryHubTestTags.TRASH,
            onClick = onOpenTrash,
        )
    }
}

@Composable
private fun LibraryStatusCard(state: LibraryHubState) {
    Card(modifier = Modifier.fillMaxWidth().testTag(LibraryHubTestTags.STATUS)) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = stringResource(R.string.library_status_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.library_total_entries, state.totalVisibleEntries))
            Text(stringResource(R.string.library_status_boundary))
        }
    }
}

@Composable
private fun LibraryDestinationButton(
    title: String,
    detail: String,
    testTag: String,
    onClick: () -> Unit,
) {
    Button(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth().testTag(testTag),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(detail, style = MaterialTheme.typography.bodySmall)
        }
    }
}
