package com.a2d.notebook.feature.library

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

object PageBrowserTestTags {
    const val TITLE = "page_browser_title"
    const val LOCAL_FIRST = "page_browser_local_first"
    const val SUMMARY = "page_browser_summary"
    const val API_BOUNDARY = "page_browser_api_boundary"
    const val EMPTY_STATE = "page_browser_empty_state"
    const val PAGE_ROW = "page_browser_page_row"
    const val OPEN_PAGE = "page_browser_open_page"
    const val OPEN_VERSIONS = "page_browser_open_versions"
}

data class PageBrowserState(
    val pageCount: Int = 0,
    val scannedPageCount: Int = 0,
    val needsReviewCount: Int = 0,
    val pages: List<PageBrowserPageSummary> = emptyList(),
    val pagePresentationApiConnected: Boolean = false,
)

data class PageBrowserPageSummary(
    val pageId: String,
    val notebookName: String,
    val visiblePageLabel: String,
    val statusLabel: String,
    val updatedSummary: String? = null,
    val needsReview: Boolean = false,
)

@Composable
fun PageBrowserScreen(
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: PageBrowserState = PageBrowserState(),
    onOpenPage: (String) -> Unit = {},
) {
    PageBrowserContent(
        state = state,
        onBack = onBack,
        onOpenPage = onOpenPage,
        onOpenVersions = onOpenVersions,
        modifier = modifier,
    )
}

@Composable
fun PageBrowserContent(
    state: PageBrowserState,
    onBack: () -> Unit,
    onOpenVersions: (String) -> Unit,
    modifier: Modifier = Modifier,
    onOpenPage: (String) -> Unit = {},
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
            text = stringResource(R.string.page_browser_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(PageBrowserTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.page_browser_local_first),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag(PageBrowserTestTags.LOCAL_FIRST),
        )
        PageBrowserSummaryCard(state)
        if (!state.pagePresentationApiConnected) {
            Card(Modifier.fillMaxWidth().testTag(PageBrowserTestTags.API_BOUNDARY)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.page_browser_boundary_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.page_browser_api_boundary))
                }
            }
        }
        if (state.pages.isEmpty()) {
            Card(Modifier.fillMaxWidth().testTag(PageBrowserTestTags.EMPTY_STATE)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.page_browser_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(stringResource(R.string.page_browser_empty_body))
                }
            }
        } else {
            state.pages.forEach { page ->
                PageBrowserRow(
                    page = page,
                    onOpenPage = onOpenPage,
                    onOpenVersions = onOpenVersions,
                )
            }
        }
    }
}

@Composable
private fun PageBrowserSummaryCard(state: PageBrowserState) {
    Card(Modifier.fillMaxWidth().testTag(PageBrowserTestTags.SUMMARY)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.page_browser_summary_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.page_browser_total_pages, state.pageCount))
            Text(stringResource(R.string.page_browser_scanned_pages, state.scannedPageCount))
            Text(stringResource(R.string.page_browser_needs_review_count, state.needsReviewCount))
        }
    }
}

@Composable
private fun PageBrowserRow(
    page: PageBrowserPageSummary,
    onOpenPage: (String) -> Unit,
    onOpenVersions: (String) -> Unit,
) {
    Card(Modifier.fillMaxWidth().testTag(PageBrowserTestTags.PAGE_ROW)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.page_browser_page, page.visiblePageLabel),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(stringResource(R.string.page_browser_notebook, page.notebookName))
            Text(stringResource(R.string.page_browser_status, page.statusLabel))
            page.updatedSummary?.let {
                Text(stringResource(R.string.page_browser_updated, it))
            }
            if (page.needsReview) {
                Text(stringResource(R.string.page_browser_needs_review_badge))
            }
            OutlinedButton(
                onClick = { onOpenPage(page.pageId) },
                modifier = Modifier.fillMaxWidth().testTag(PageBrowserTestTags.OPEN_PAGE),
            ) {
                Text(stringResource(R.string.page_browser_open_page))
            }
            OutlinedButton(
                onClick = { onOpenVersions(page.pageId) },
                modifier = Modifier.fillMaxWidth().testTag(PageBrowserTestTags.OPEN_VERSIONS),
            ) {
                Text(stringResource(R.string.page_browser_open_versions))
            }
        }
    }
}
