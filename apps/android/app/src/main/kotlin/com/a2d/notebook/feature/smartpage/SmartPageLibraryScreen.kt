package com.a2d.notebook.feature.smartpage

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

object SmartPageLibraryTestTags {
    const val TITLE = "smart_page_library_title"
    const val LOCAL_FIRST = "smart_page_library_local_first"
    const val SUMMARY = "smart_page_library_summary"
    const val BOUNDARY = "smart_page_library_boundary"
    const val EMPTY_STATE = "smart_page_library_empty_state"
    const val SMART_PAGE_ITEM = "smart_page_library_smart_page_item"
    const val PAGE_SET_ITEM = "smart_page_library_page_set_item"
    const val COLLECTION_ITEM = "smart_page_library_collection_item"
    const val CREATE = "smart_page_library_create"
}

data class SmartPageLibraryState(
    val smartPages: List<GeneratedSmartPageSummary> = emptyList(),
    val pageSets: List<GeneratedPageSetSummary> = emptyList(),
    val collections: List<GeneratedCollectionSummary> = emptyList(),
    val generatedContentApiConnected: Boolean = false,
) {
    val smartPageCount: Int
        get() = smartPages.sumOf { item -> item.pageCount }

    val pageSetCount: Int
        get() = pageSets.size

    val collectionCount: Int
        get() = collections.size

    val hasContent: Boolean
        get() = smartPages.isNotEmpty() || pageSets.isNotEmpty() || collections.isNotEmpty()
}

data class GeneratedSmartPageSummary(
    val smartPageId: String,
    val title: String,
    val pageSetId: String,
    val pageCount: Int,
    val style: String,
    val createdSummary: String,
)

data class GeneratedPageSetSummary(
    val pageSetId: String,
    val title: String,
    val pageCount: Int,
    val firstVisiblePage: Int,
    val createdSummary: String,
)

data class GeneratedCollectionSummary(
    val collectionId: String,
    val title: String,
    val itemCount: Int,
    val ruleSummary: String,
)

@Composable
fun SmartPageLibraryScreen(
    onBack: () -> Unit,
    onCreateSmartPages: () -> Unit,
    modifier: Modifier = Modifier,
    state: SmartPageLibraryState = SmartPageLibraryState(),
) {
    SmartPageLibraryContent(
        state = state,
        onBack = onBack,
        onCreateSmartPages = onCreateSmartPages,
        modifier = modifier,
    )
}

@Composable
fun SmartPageLibraryContent(
    state: SmartPageLibraryState,
    onBack: () -> Unit,
    onCreateSmartPages: () -> Unit,
    modifier: Modifier = Modifier,
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
            text = stringResource(R.string.smart_page_library_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(SmartPageLibraryTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.smart_page_library_local_first),
            modifier = Modifier.testTag(SmartPageLibraryTestTags.LOCAL_FIRST),
        )
        SmartPageLibrarySummaryCard(state)
        SmartPageLibraryBoundaryCard(state.generatedContentApiConnected)
        Button(
            onClick = onCreateSmartPages,
            modifier = Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.CREATE),
        ) {
            Text(stringResource(R.string.smart_page_library_create))
        }

        if (!state.hasContent) {
            Card(Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.EMPTY_STATE)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.smart_page_library_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.smart_page_library_empty_body))
                }
            }
        }

        if (state.smartPages.isNotEmpty()) {
            Text(
                text = stringResource(R.string.smart_page_library_smart_pages_section),
                style = MaterialTheme.typography.titleMedium,
            )
            state.smartPages.forEach { item -> SmartPageItemCard(item) }
        }
        if (state.pageSets.isNotEmpty()) {
            Text(
                text = stringResource(R.string.smart_page_library_page_sets_section),
                style = MaterialTheme.typography.titleMedium,
            )
            state.pageSets.forEach { item -> PageSetItemCard(item) }
        }
        if (state.collections.isNotEmpty()) {
            Text(
                text = stringResource(R.string.smart_page_library_collections_section),
                style = MaterialTheme.typography.titleMedium,
            )
            state.collections.forEach { item -> CollectionItemCard(item) }
        }
    }
}

@Composable
private fun SmartPageLibrarySummaryCard(state: SmartPageLibraryState) {
    Card(Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.SUMMARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.smart_page_library_summary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.smart_page_library_smart_pages_count, state.smartPageCount))
            Text(stringResource(R.string.smart_page_library_page_sets_count, state.pageSetCount))
            Text(stringResource(R.string.smart_page_library_collections_count, state.collectionCount))
        }
    }
}

@Composable
private fun SmartPageLibraryBoundaryCard(apiConnected: Boolean) {
    Card(Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.BOUNDARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.smart_page_library_boundary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                if (apiConnected) {
                    stringResource(R.string.smart_page_library_api_connected)
                } else {
                    stringResource(R.string.smart_page_library_api_boundary)
                },
            )
            Text(stringResource(R.string.smart_page_library_no_id_reuse))
        }
    }
}

@Composable
private fun SmartPageItemCard(item: GeneratedSmartPageSummary) {
    Card(Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.SMART_PAGE_ITEM)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(item.title, style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.smart_page_library_smart_page_id, item.smartPageId))
            Text(stringResource(R.string.smart_page_library_page_set_id, item.pageSetId))
            Text(stringResource(R.string.smart_page_library_page_count, item.pageCount))
            Text(stringResource(R.string.smart_page_library_style, item.style))
            Text(stringResource(R.string.smart_page_library_created, item.createdSummary))
        }
    }
}

@Composable
private fun PageSetItemCard(item: GeneratedPageSetSummary) {
    Card(Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.PAGE_SET_ITEM)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(item.title, style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.smart_page_library_page_set_id, item.pageSetId))
            Text(stringResource(R.string.smart_page_library_page_count, item.pageCount))
            Text(stringResource(R.string.smart_page_library_first_visible_page, item.firstVisiblePage))
            Text(stringResource(R.string.smart_page_library_created, item.createdSummary))
        }
    }
}

@Composable
private fun CollectionItemCard(item: GeneratedCollectionSummary) {
    Card(Modifier.fillMaxWidth().testTag(SmartPageLibraryTestTags.COLLECTION_ITEM)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(item.title, style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.smart_page_library_collection_id, item.collectionId))
            Text(stringResource(R.string.smart_page_library_collection_items, item.itemCount))
            Text(stringResource(R.string.smart_page_library_collection_rule, item.ruleSummary))
        }
    }
}
