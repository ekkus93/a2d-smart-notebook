package com.a2d.notebook.feature.ocr

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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R

object OcrSearchTestTags {
    const val TITLE = "ocr_search_title"
    const val LOCAL_FIRST = "ocr_search_local_first"
    const val QUERY_FIELD = "ocr_search_query_field"
    const val SUBMIT = "ocr_search_submit"
    const val NO_QUERY = "ocr_search_no_query"
    const val NO_MATCHES = "ocr_search_no_matches"
    const val ERROR = "ocr_search_error"
    const val RESULTS = "ocr_search_results"
    const val RESULT_ROW = "ocr_search_result_row"
    const val OPEN_PAGE = "ocr_search_open_page"
}

@Composable
fun OcrSearchScreen(
    onBack: () -> Unit,
    onOpenPage: (String) -> Unit,
    modifier: Modifier = Modifier,
    searchController: AndroidOcrSearchController? = null,
) {
    var state by remember { mutableStateOf(OcrSearchPresentationState.noQuery()) }
    val notConnectedMessage = stringResource(R.string.ocr_search_not_connected)
    OcrSearchContent(
        state = state,
        onBack = onBack,
        onQueryChange = { query -> state = OcrSearchPresentationState.noQuery(query) },
        onSubmitSearch = { query ->
            state =
                searchController?.submit(query)
                    ?: OcrSearchPresentationState.error(
                        query = query.trim(),
                        message = notConnectedMessage,
                    )
        },
        onOpenPage = onOpenPage,
        modifier = modifier,
    )
}

@Composable
fun OcrSearchContent(
    state: OcrSearchPresentationState,
    onBack: () -> Unit,
    onQueryChange: (String) -> Unit,
    onSubmitSearch: (String) -> Unit,
    onOpenPage: (String) -> Unit,
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
            text = stringResource(R.string.ocr_search_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(OcrSearchTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.ocr_search_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(OcrSearchTestTags.LOCAL_FIRST),
        )
        OcrSearchBoundaryCard()
        OutlinedTextField(
            value = state.query,
            onValueChange = onQueryChange,
            label = { Text(stringResource(R.string.ocr_search_query_label)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.QUERY_FIELD),
        )
        Button(
            onClick = { onSubmitSearch(state.query) },
            modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.SUBMIT),
        ) {
            Text(stringResource(R.string.ocr_search_submit))
        }

        when (state.status) {
            OcrSearchPresentationStatus.NoQuery -> OcrSearchNoQueryCard()
            OcrSearchPresentationStatus.NoMatches -> OcrSearchNoMatchesCard(state.query)
            OcrSearchPresentationStatus.Error -> OcrSearchErrorCard(state.errorMessage)
            OcrSearchPresentationStatus.Results -> OcrSearchResultsList(state, onOpenPage)
        }
    }
}

@Composable
private fun OcrSearchBoundaryCard() {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.ocr_search_boundary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.ocr_search_boundary_body))
        }
    }
}

@Composable
private fun OcrSearchNoQueryCard() {
    Card(modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.NO_QUERY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.ocr_search_no_query_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.ocr_search_no_query_body))
        }
    }
}

@Composable
private fun OcrSearchNoMatchesCard(query: String) {
    Card(modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.NO_MATCHES)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.ocr_search_no_matches_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.ocr_search_no_matches_body, query))
        }
    }
}

@Composable
private fun OcrSearchErrorCard(errorMessage: String?) {
    Card(modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.ERROR)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.ocr_search_error_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                stringResource(
                    R.string.common_error_prefix,
                    errorMessage ?: stringResource(R.string.common_unknown),
                ),
            )
        }
    }
}

@Composable
private fun OcrSearchResultsList(
    state: OcrSearchPresentationState,
    onOpenPage: (String) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.RESULTS),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(stringResource(R.string.ocr_search_result_count, state.hits.size))
        state.hits.forEach { hit ->
            OcrSearchResultRow(hit, onOpenPage)
        }
    }
}

@Composable
private fun OcrSearchResultRow(
    hit: OcrSearchHitState,
    onOpenPage: (String) -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.RESULT_ROW)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(hit.snippet, style = MaterialTheme.typography.bodyLarge)
            Text(stringResource(R.string.ocr_search_hit_source, hit.documentKind.displayLabel))
            Text(stringResource(R.string.ocr_search_hit_page, hit.pageId))
            Text(stringResource(R.string.ocr_search_hit_scan, hit.scanId))
            Text(stringResource(R.string.ocr_search_hit_run, hit.ocrRunId))
            hit.textRegionId?.let { regionId ->
                Text(stringResource(R.string.ocr_search_hit_region, regionId))
            }
            OutlinedButton(
                onClick = { onOpenPage(hit.pageId) },
                modifier = Modifier.fillMaxWidth().testTag(OcrSearchTestTags.OPEN_PAGE),
            ) {
                Text(stringResource(R.string.ocr_search_open_page))
            }
        }
    }
}
