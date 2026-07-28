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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.rustbridge.A2dBridge

object HomeScreenTestTags {
    const val TITLE = "home_title"
    const val RUST_GENERATED_ID = "home_rust_generated_id"
    const val SCAN_PAGE = "home_scan_page"
    const val NOTEBOOKS = "home_notebooks"
    const val SMART_PAGES = "home_smart_pages"
}

@Composable
fun HomeScreen(
    onScanPage: () -> Unit,
    onOpenNotebooks: () -> Unit,
    onCreateSmartPages: () -> Unit,
) {
    val context = LocalContext.current
    val rustGeneratedPageId = remember { A2dBridge.client(context).generatePageId() }

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
            onClick = onOpenNotebooks,
            modifier = Modifier.testTag(HomeScreenTestTags.NOTEBOOKS),
        ) { Text(stringResource(R.string.home_notebooks)) }
        Spacer(Modifier.height(12.dp))
        Button(
            onClick = onCreateSmartPages,
            modifier = Modifier.testTag(HomeScreenTestTags.SMART_PAGES),
        ) { Text(stringResource(R.string.home_smart_pages)) }
        Spacer(Modifier.height(24.dp))
        Text(
            text = stringResource(R.string.home_rust_generated_id_prefix, rustGeneratedPageId),
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.testTag(HomeScreenTestTags.RUST_GENERATED_ID),
        )
    }
}
