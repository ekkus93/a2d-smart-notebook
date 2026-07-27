package com.a2d.notebook.feature.notebook

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.a2d.notebook.R
import com.a2d.notebook.rustbridge.QrCaptureButton
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.NotebookSummary

@Composable
fun NotebookLibraryScreen(
    onBack: () -> Unit,
    onAddNotebook: () -> Unit,
    viewModel: NotebookViewModel = viewModel(),
) {
    val state by viewModel.state
    var renaming by remember { mutableStateOf<NotebookSummary?>(null) }
    var renameValue by rememberSaveable { mutableStateOf("") }

    LaunchedEffect(Unit) { viewModel.refreshNotebooks() }

    Column(Modifier.fillMaxSize().padding(20.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
            Button(onClick = onAddNotebook, enabled = !state.busy) {
                Text(stringResource(R.string.notebooks_add))
            }
        }
        Text(stringResource(R.string.notebooks_title), style = MaterialTheme.typography.headlineMedium)
        state.error?.let {
            Text(stringResource(R.string.common_error_prefix, it), color = MaterialTheme.colorScheme.error)
        }
        when {
            state.busy && state.notebooks.isEmpty() -> Text(stringResource(R.string.common_loading))
            state.notebooks.isEmpty() -> Text(stringResource(R.string.notebooks_empty))
            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                items(state.notebooks, key = { it.id }) { notebook ->
                    Card(Modifier.fillMaxWidth()) {
                        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            Text(notebook.displayName, style = MaterialTheme.typography.titleMedium)
                            Text(stringResource(R.string.notebooks_design, notebook.designId))
                            if (notebook.active) {
                                Text(
                                    stringResource(R.string.notebooks_active),
                                    color = MaterialTheme.colorScheme.primary,
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                if (!notebook.active) {
                                    TextButton(
                                        enabled = !state.busy,
                                        onClick = { viewModel.setActiveNotebook(notebook.id) },
                                    ) { Text(stringResource(R.string.notebooks_make_active)) }
                                }
                                TextButton(
                                    enabled = !state.busy,
                                    onClick = {
                                        renaming = notebook
                                        renameValue = notebook.displayName
                                    },
                                ) { Text(stringResource(R.string.notebooks_rename)) }
                                TextButton(
                                    enabled = !state.busy,
                                    onClick = { viewModel.archiveNotebook(notebook.id) },
                                ) { Text(stringResource(R.string.notebooks_archive)) }
                            }
                        }
                    }
                }
            }
        }
        if (state.notebooks.any { it.active }) {
            TextButton(
                enabled = !state.busy,
                onClick = { viewModel.setActiveNotebook(null) },
            ) { Text(stringResource(R.string.notebooks_clear_active)) }
        }
    }

    renaming?.let { notebook ->
        AlertDialog(
            onDismissRequest = { renaming = null },
            title = { Text(stringResource(R.string.notebooks_rename)) },
            text = {
                OutlinedTextField(
                    value = renameValue,
                    onValueChange = { renameValue = it },
                    label = { Text(stringResource(R.string.notebooks_name)) },
                )
            },
            confirmButton = {
                TextButton(
                    enabled = renameValue.isNotBlank() && !state.busy,
                    onClick = {
                        viewModel.renameNotebook(notebook.id, renameValue.trim())
                        renaming = null
                    },
                ) { Text(stringResource(R.string.notebooks_rename)) }
            },
            dismissButton = {
                TextButton(onClick = { renaming = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}

@Composable
fun NotebookSetupScreen(
    onBack: () -> Unit,
    onResolveFirstPage: (String) -> Unit,
    viewModel: NotebookViewModel = viewModel(),
) {
    val context = LocalContext.current
    val state by viewModel.state
    var payload by rememberSaveable { mutableStateOf("") }
    var displayName by rememberSaveable { mutableStateOf("") }
    var color by rememberSaveable { mutableStateOf("") }
    var icon by rememberSaveable { mutableStateOf("") }
    var notes by rememberSaveable { mutableStateOf("") }
    var makeActive by rememberSaveable { mutableStateOf(true) }

    fun recognize(candidate: String) {
        payload = candidate
        viewModel.resolveSetupCode(candidate.trim()) { design ->
            if (displayName.isBlank()) displayName = design.name
        }
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
        Text(stringResource(R.string.setup_title), style = MaterialTheme.typography.headlineMedium)

        state.createdNotebook?.let { created ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.setup_created), style = MaterialTheme.typography.titleLarge)
                    Text(created.notebook.displayName)
                    Text(
                        stringResource(
                            R.string.setup_created_pages,
                            created.createdPageCount.toInt(),
                        ),
                    )
                    Button(onClick = { onResolveFirstPage(created.notebook.id) }) {
                        Text(stringResource(R.string.setup_resolve_first_page))
                    }
                    TextButton(
                        onClick = {
                            payload = ""
                            displayName = ""
                            color = ""
                            icon = ""
                            notes = ""
                            viewModel.clearTransientResult()
                        },
                    ) { Text(stringResource(R.string.setup_add_another_copy)) }
                }
            }
            return@Column
        }

        QrCaptureButton(
            label = stringResource(R.string.setup_scan),
            prefix = "setup-",
            onDecoded = ::recognize,
            onFailure = { failure ->
                val message = failure?.message ?: context.getString(R.string.setup_capture_failed)
                payload = ""
                viewModel.resolveSetupCode(message)
            },
        )
        OutlinedTextField(
            value = payload,
            onValueChange = {
                payload = it
                viewModel.clearTransientResult()
            },
            label = { Text(stringResource(R.string.setup_payload)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
        )
        Button(
            enabled = payload.isNotBlank() && !state.busy,
            onClick = { recognize(payload) },
        ) { Text(stringResource(R.string.setup_recognize)) }

        if (state.busy) Text(stringResource(R.string.common_loading))
        state.error?.let {
            Text(stringResource(R.string.common_error_prefix, it), color = MaterialTheme.colorScheme.error)
        }

        state.recognizedDesign?.let { design ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.setup_recognized), style = MaterialTheme.typography.titleLarge)
                    Text(design.name)
                    Text(
                        stringResource(
                            R.string.setup_design_version,
                            design.designVersion.toInt(),
                            design.logicalPageCount.toInt(),
                        ),
                    )
                    if (!design.trusted) {
                        Text(stringResource(R.string.setup_untrusted), color = MaterialTheme.colorScheme.error)
                    }
                }
            }
            OutlinedTextField(
                value = displayName,
                onValueChange = { displayName = it },
                label = { Text(stringResource(R.string.notebooks_name)) },
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = color,
                onValueChange = { color = it },
                label = { Text(stringResource(R.string.setup_color)) },
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = icon,
                onValueChange = { icon = it },
                label = { Text(stringResource(R.string.setup_icon)) },
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text(stringResource(R.string.setup_notes)) },
                modifier = Modifier.fillMaxWidth(),
                minLines = 2,
            )
            Text(stringResource(R.string.setup_multiple_copies))
            Row {
                Checkbox(checked = makeActive, onCheckedChange = { makeActive = it })
                Text(stringResource(R.string.notebooks_make_active), Modifier.padding(top = 12.dp))
            }
            Button(
                enabled = displayName.isNotBlank() && !state.busy,
                onClick = {
                    viewModel.createNotebook(
                        CreateNotebookRequest(
                            setupPayload = payload.trim(),
                            displayName = displayName.trim(),
                            optionalColor = color.trim().takeIf { it.isNotEmpty() },
                            optionalIcon = icon.trim().takeIf { it.isNotEmpty() },
                            optionalUserNotes = notes.trim().takeIf { it.isNotEmpty() },
                            makeActive = makeActive,
                        ),
                    )
                },
            ) { Text(stringResource(R.string.setup_create)) }
        }
    }
}

@Composable
fun PageCodeScreen(
    notebookId: String,
    onBack: () -> Unit,
    viewModel: NotebookViewModel = viewModel(),
) {
    val context = LocalContext.current
    val state by viewModel.state
    var payload by rememberSaveable { mutableStateOf("") }

    fun resolve(candidate: String) {
        payload = candidate
        viewModel.resolvePageCode(candidate.trim(), notebookId)
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
        Text(stringResource(R.string.page_code_title), style = MaterialTheme.typography.headlineMedium)
        Text(stringResource(R.string.page_code_destination, notebookId))
        Text(stringResource(R.string.page_code_explanation))
        QrCaptureButton(
            label = stringResource(R.string.page_code_scan),
            prefix = "page-code-",
            onDecoded = ::resolve,
            onFailure = { failure ->
                payload = ""
                val message = failure?.message ?: context.getString(R.string.setup_capture_failed)
                viewModel.resolvePageCode(message, notebookId)
            },
        )
        OutlinedTextField(
            value = payload,
            onValueChange = { payload = it },
            label = { Text(stringResource(R.string.page_code_payload)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
        )
        Button(
            enabled = payload.isNotBlank() && !state.busy,
            onClick = { resolve(payload) },
        ) { Text(stringResource(R.string.page_code_resolve)) }
        if (state.busy) Text(stringResource(R.string.common_loading))
        state.error?.let {
            Text(stringResource(R.string.common_error_prefix, it), color = MaterialTheme.colorScheme.error)
        }
        state.pageResolution?.let { resolution ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.page_code_result), style = MaterialTheme.typography.titleMedium)
                    Text(resolution.toString())
                }
            }
        }
    }
}
