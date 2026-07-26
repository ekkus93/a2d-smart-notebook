package com.a2d.notebook.feature.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R

/** Semantics test tags used by Compose UI tests (androidTest) to find nodes without brittle
 * text matching. */
object HomeScreenTestTags {
    const val TITLE = "home_title"
}

/**
 * Placeholder empty-state Home screen (TODO 1.2, spec section 7.1's first-launch empty state).
 * Real content (Scan a Page / Add a Notebook / Create Smart Pages / Import actions, recent
 * notebooks, Needs Review count) arrives with Milestone 10 -- this only proves the Compose +
 * navigation shell renders.
 */
@Composable
fun HomeScreen() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.home_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag(HomeScreenTestTags.TITLE),
        )
        Text(
            text = stringResource(R.string.home_placeholder),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}
