package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel

@Composable
fun SinglePageScannerScreen(
    onBack: () -> Unit,
    viewModel: SinglePageScannerViewModel = viewModel(),
) {
    PolicyAwareSinglePageScannerRoute(onBack = onBack, viewModel = viewModel)
}
