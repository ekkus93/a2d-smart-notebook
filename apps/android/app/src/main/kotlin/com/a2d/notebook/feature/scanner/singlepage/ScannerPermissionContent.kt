package com.a2d.notebook.feature.scanner.singlepage

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
internal fun ScannerPermissionContent(
    explanation: String,
    actionLabel: String,
    backLabel: String,
    onAction: () -> Unit,
    onBack: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(explanation, style = MaterialTheme.typography.titleLarge)
        Button(onClick = onAction, modifier = Modifier.padding(top = 16.dp)) {
            Text(actionLabel)
        }
        TextButton(onClick = onBack, modifier = Modifier.padding(top = 8.dp)) {
            Text(backLabel)
        }
    }
}
