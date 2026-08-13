package com.a2d.notebook.feature.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R

object HomeScreenTestTags {
    const val TITLE = "home_title"
    const val SCAN_PAGE = "home_scan_page"
    const val BATCH_SCAN = "home_batch_scan"
    const val NOTEBOOKS = "home_notebooks"
    const val SMART_PAGES = "home_smart_pages"
}

@Composable
fun HomeScreen(
    onScanPage: () -> Unit,
    onBatchScan: () -> Unit,
    onOpenNotebooks: () -> Unit,
    onCreateSmartPages: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.home_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(HomeScreenTestTags.TITLE),
        )
        Text(stringResource(R.string.home_placeholder), style = MaterialTheme.typography.bodyMedium)
        Spacer(Modifier.height(24.dp))
        Button(
            onClick = onScanPage,
            modifier = Modifier.testTag(HomeScreenTestTags.SCAN_PAGE),
        ) { Text(stringResource(R.string.home_scan_page)) }
        Spacer(Modifier.height(12.dp))
        Button(
            onClick = onBatchScan,
            modifier = Modifier.testTag(HomeScreenTestTags.BATCH_SCAN),
        ) { Text("Batch Scan") }
        Spacer(Modifier.height(12.dp))
        Button(
            onClick = onOpenNotebooks,
            modifier = Modifier.testTag(HomeScreenTestTags.NOTEBOOKS),
        ) { Text(stringResource(R.string.home_notebooks)) }
        Spacer(Modifier.height(12.dp))
        Button(
            onClick = onCreateSmartPages,
            modifier = Modifier.testTag(HomeScreenTestTags.SMART_PAGES),
        ) { Text(stringResource(R.string.home_smart_pages)) }
    }
}
