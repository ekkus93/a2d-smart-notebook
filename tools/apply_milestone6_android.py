#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match, found {count}: {old[:100]!r}")
    target.write_text(text.replace(old, new, 1))


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


replace_once(
    "apps/android/app/build.gradle.kts",
    '''    implementation("androidx.navigation:navigation-compose:2.8.4")

    // UniFFI's generated Kotlin bindings''',
    '''    implementation("androidx.navigation:navigation-compose:2.8.4")
    implementation("com.google.zxing:core:3.5.3")

    // UniFFI's generated Kotlin bindings''',
)

replace_once(
    "apps/android/app/src/main/AndroidManifest.xml",
    '''        <activity
            android:name=".app.MainActivity"
''',
    '''        <provider
            android:name="androidx.core.content.FileProvider"
            android:authorities="${applicationId}.files"
            android:exported="false"
            android:grantUriPermissions="true">
            <meta-data
                android:name="android.support.FILE_PROVIDER_PATHS"
                android:resource="@xml/file_paths" />
        </provider>

        <activity
            android:name=".app.MainActivity"
''',
)

write(
    "apps/android/app/src/main/res/xml/file_paths.xml",
    '''<?xml version="1.0" encoding="utf-8"?>
<paths xmlns:android="http://schemas.android.com/apk/res/android">
    <files-path name="library" path="library/" />
    <cache-path name="qr_capture" path="qr-capture/" />
</paths>
''',
)

write(
    "apps/android/app/src/main/res/values/strings.xml",
    '''<resources>
    <string name="app_name">A2D Smart Notebook</string>
    <string name="home_title">A2D Smart Notebook</string>
    <string name="home_placeholder">Local-first. No account required.</string>
    <string name="home_rust_generated_id_prefix">Rust-generated Page ID: %1$s</string>
    <string name="home_notebooks">Notebooks</string>
    <string name="home_smart_pages">Create Smart Pages</string>

    <string name="common_back">Back</string>
    <string name="common_retry">Retry</string>
    <string name="common_cancel">Cancel</string>
    <string name="common_error">Error</string>
    <string name="common_loading">Loading…</string>

    <string name="notebooks_title">Notebooks</string>
    <string name="notebooks_add">Add Notebook</string>
    <string name="notebooks_empty">No Notebooks have been registered.</string>
    <string name="notebooks_active">Active scan destination</string>
    <string name="notebooks_make_active">Make active</string>
    <string name="notebooks_clear_active">Clear active selection</string>
    <string name="notebooks_rename">Rename</string>
    <string name="notebooks_archive">Archive</string>
    <string name="notebooks_rename_title">Rename Notebook</string>
    <string name="notebooks_name">Notebook name</string>

    <string name="setup_title">Add a Notebook</string>
    <string name="setup_scan">Photograph Setup Code</string>
    <string name="setup_payload">Notebook Setup Code payload</string>
    <string name="setup_recognize">Recognize Setup Code</string>
    <string name="setup_recognized">Notebook Design recognized</string>
    <string name="setup_design_version">Design version %1$d · %2$d logical pages</string>
    <string name="setup_untrusted">This design is not trusted by the current build.</string>
    <string name="setup_color">Optional color label</string>
    <string name="setup_icon">Optional icon label</string>
    <string name="setup_notes">Optional notes</string>
    <string name="setup_multiple_copies">You may register multiple physical copies of the same Notebook Design. Each copy receives its own identity, so scans never get silently mixed together.</string>
    <string name="setup_create">Create Notebook</string>
    <string name="setup_created">Notebook created</string>
    <string name="setup_created_pages">Created %1$d persistent page slots.</string>
    <string name="setup_scan_first_page">Scan First Page</string>
    <string name="setup_add_another_copy">Add another copy</string>
    <string name="setup_capture_failed">The camera did not return a Setup Code image.</string>
    <string name="setup_decode_failed">No readable QR code was found in the captured image.</string>

    <string name="page_code_title">Resolve a Page Code</string>
    <string name="page_code_destination">Destination Notebook: %1$s</string>
    <string name="page_code_scan">Photograph Page Code</string>
    <string name="page_code_payload">Page Code payload</string>
    <string name="page_code_resolve">Resolve Page Code</string>
    <string name="page_code_result">Rust resolution result</string>
    <string name="page_code_explanation">This confirms identity and destination only. Full-page capture and durable scan registration arrive in Milestones 8 and 9.</string>

    <string name="smart_pages_title">Create Smart Pages</string>
    <string name="smart_pages_paper">Paper: %1$s</string>
    <string name="smart_pages_style">Style: %1$s</string>
    <string name="smart_pages_page_count">Page count</string>
    <string name="smart_pages_start_number">Starting visible page number</string>
    <string name="smart_pages_generate">Generate PDF</string>
    <string name="smart_pages_single_hint">Use page count 1 for a standalone Smart Page; use a larger count for a Page Set.</string>
    <string name="smart_pages_invalid_count">Page count must be between 1 and 500.</string>
    <string name="smart_pages_invalid_start">Starting page number must be at least 1.</string>
    <string name="smart_pages_generated">Smart Pages generated</string>
    <string name="smart_pages_set_id">Page Set: %1$s</string>
    <string name="smart_pages_page_total">%1$d unique page identities</string>
    <string name="smart_pages_save">Save a copy</string>
    <string name="smart_pages_share">Share</string>
    <string name="smart_pages_print">Print</string>
    <string name="smart_pages_saved">PDF copy saved.</string>
    <string name="smart_pages_save_failed">The PDF could not be copied to the selected destination.</string>
    <string name="smart_pages_preview_unavailable">PDF preview unavailable.</string>
</resources>
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/rustbridge/QrCapture.kt",
    r'''package com.a2d.notebook.rustbridge

import android.content.Context
import android.graphics.BitmapFactory
import androidx.core.content.FileProvider
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.io.File

/** A temporary camera destination and its grantable content URI. */
data class QrCapture(
    val file: File,
    val uri: android.net.Uri,
)

fun createQrCapture(context: Context, prefix: String): QrCapture {
    val directory = context.cacheDir.resolve("qr-capture").apply { mkdirs() }
    val file = File.createTempFile(prefix, ".jpg", directory)
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    return QrCapture(file, uri)
}

/**
 * Decodes one QR code from a captured image entirely on-device. The decoded text is never trusted
 * here: feature code immediately sends it to Rust for the canonical A2D grammar, checksum, layout,
 * and identity checks.
 */
fun decodeQrImage(file: File): String {
    val bitmap = BitmapFactory.decodeFile(file.absolutePath)
        ?: error("captured image could not be decoded")
    val pixels = IntArray(bitmap.width * bitmap.height)
    bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
    val source = RGBLuminanceSource(bitmap.width, bitmap.height, pixels)
    val binary = BinaryBitmap(HybridBinarizer(source))
    val hints = mapOf(
        DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
        DecodeHintType.TRY_HARDER to true,
    )
    return MultiFormatReader().decode(binary, hints).text
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/rustbridge/QrCaptureButton.kt",
    r'''package com.a2d.notebook.rustbridge

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
fun QrCaptureButton(
    label: String,
    prefix: String,
    onDecoded: (String) -> Unit,
    onFailure: (Throwable?) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var pending by remember { mutableStateOf<QrCapture?>(null) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture()) { ok ->
        val capture = pending
        pending = null
        if (!ok || capture == null) {
            capture?.file?.delete()
            onFailure(null)
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val decoded = runCatching {
                withContext(Dispatchers.IO) { decodeQrImage(capture.file) }
            }
            capture.file.delete()
            decoded.onSuccess(onDecoded).onFailure { onFailure(it) }
        }
    }

    Button(
        onClick = {
            runCatching { createQrCapture(context, prefix) }
                .onSuccess {
                    pending = it
                    launcher.launch(it.uri)
                }
                .onFailure { onFailure(it) }
        },
    ) {
        Text(label)
    }
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/home/HomeScreen.kt",
    r'''package com.a2d.notebook.feature.home

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
    const val NOTEBOOKS = "home_notebooks"
    const val SMART_PAGES = "home_smart_pages"
}

@Composable
fun HomeScreen(
    onOpenNotebooks: () -> Unit,
    onCreateSmartPages: () -> Unit,
) {
    val context = LocalContext.current
    val rustGeneratedPageId = remember { A2dBridge.client(context).generatePageId() }

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
        Spacer(Modifier.height(24.dp))
        Button(
            onClick = onOpenNotebooks,
            modifier = Modifier.testTag(HomeScreenTestTags.NOTEBOOKS),
        ) {
            Text(stringResource(R.string.home_notebooks))
        }
        Spacer(Modifier.height(12.dp))
        Button(
            onClick = onCreateSmartPages,
            modifier = Modifier.testTag(HomeScreenTestTags.SMART_PAGES),
        ) {
            Text(stringResource(R.string.home_smart_pages))
        }
        Spacer(Modifier.height(24.dp))
        Text(
            text = stringResource(R.string.home_rust_generated_id_prefix, rustGeneratedPageId),
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.testTag(HomeScreenTestTags.RUST_GENERATED_ID),
        )
    }
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/notebook/NotebookSetupScreen.kt",
    r'''package com.a2d.notebook.feature.notebook

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.QrCaptureButton
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.CreatedNotebook
import uniffi.a2d_ffi.NotebookDesignSummary

@Composable
fun NotebookSetupScreen(
    onBack: () -> Unit,
    onScanFirstPage: (String) -> Unit,
) {
    val context = LocalContext.current
    val client = remember { A2dBridge.client(context) }
    val scope = rememberCoroutineScope()
    var payload by rememberSaveable { mutableStateOf("") }
    var design by remember { mutableStateOf<NotebookDesignSummary?>(null) }
    var displayName by rememberSaveable { mutableStateOf("") }
    var color by rememberSaveable { mutableStateOf("") }
    var icon by rememberSaveable { mutableStateOf("") }
    var notes by rememberSaveable { mutableStateOf("") }
    var makeActive by rememberSaveable { mutableStateOf(true) }
    var created by remember { mutableStateOf<CreatedNotebook?>(null) }
    var busy by remember { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }

    fun resolve(candidate: String) {
        payload = candidate
        design = null
        created = null
        error = null
        busy = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) { client.resolveNotebookSetupCode(candidate) }
            }.onSuccess {
                design = it
                if (displayName.isBlank()) displayName = it.name
            }.onFailure { error = it.message ?: it.toString() }
            busy = false
        }
    }

    fun createNotebook() {
        error = null
        busy = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    client.createNotebook(
                        CreateNotebookRequest(
                            setupPayload = payload,
                            displayName = displayName,
                            optionalColor = color.trim().takeIf(String::isNotEmpty),
                            optionalIcon = icon.trim().takeIf(String::isNotEmpty),
                            optionalUserNotes = notes.trim().takeIf(String::isNotEmpty),
                            makeActive = makeActive,
                        ),
                    )
                }
            }.onSuccess { created = it }
                .onFailure { error = it.message ?: it.toString() }
            busy = false
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
        Text(stringResource(R.string.setup_title), style = MaterialTheme.typography.headlineMedium)

        created?.let { result ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        stringResource(R.string.setup_created),
                        style = MaterialTheme.typography.titleLarge,
                    )
                    Text(result.notebook.displayName)
                    Text(stringResource(R.string.setup_created_pages, result.createdPageCount))
                    Button(onClick = { onScanFirstPage(result.notebook.id) }) {
                        Text(stringResource(R.string.setup_scan_first_page))
                    }
                    TextButton(
                        onClick = {
                            created = null
                            design = null
                            payload = ""
                            displayName = ""
                        },
                    ) { Text(stringResource(R.string.setup_add_another_copy)) }
                }
            }
            return@Column
        }

        QrCaptureButton(
            label = stringResource(R.string.setup_scan),
            prefix = "setup-",
            onDecoded =(::resolve),
            onFailure = { throwable ->
                error = throwable?.message ?: context.getString(R.string.setup_capture_failed)
            },
        )
        OutlinedTextField(
            value = payload,
            onValueChange = {
                payload = it
                design = null
            },
            label = { Text(stringResource(R.string.setup_payload)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
        )
        Button(onClick = { resolve(payload.trim()) }, enabled = payload.isNotBlank() && !busy) {
            Text(stringResource(R.string.setup_recognize))
        }

        if (busy) Text(stringResource(R.string.common_loading))
        error?.let {
            Text(
                text = "${stringResource(R.string.common_error)}: $it",
                color = MaterialTheme.colorScheme.error,
            )
        }

        design?.let { recognized ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        stringResource(R.string.setup_recognized),
                        style = MaterialTheme.typography.titleLarge,
                    )
                    Text(recognized.name)
                    Text(
                        stringResource(
                            R.string.setup_design_version,
                            recognized.designVersion,
                            recognized.logicalPageCount,
                        ),
                    )
                    if (!recognized.trusted) {
                        Text(
                            stringResource(R.string.setup_untrusted),
                            color = MaterialTheme.colorScheme.error,
                        )
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
            Spacer(Modifier.height(4.dp))
            Button(
                onClick =(::createNotebook),
                enabled = displayName.isNotBlank() && !busy,
            ) { Text(stringResource(R.string.setup_create)) }
        }
    }
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/notebook/NotebookLibraryScreen.kt",
    r'''package com.a2d.notebook.feature.notebook

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.rustbridge.A2dBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.NotebookSummary

@Composable
fun NotebookLibraryScreen(
    onBack: () -> Unit,
    onAddNotebook: () -> Unit,
) {
    val context = LocalContext.current
    val client = remember { A2dBridge.client(context) }
    val scope = rememberCoroutineScope()
    var notebooks by remember { mutableStateOf<List<NotebookSummary>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    var renaming by remember { mutableStateOf<NotebookSummary?>(null) }
    var renameValue by rememberSaveable { mutableStateOf("") }

    fun refresh() {
        loading = true
        scope.launch {
            runCatching { withContext(Dispatchers.IO) { client.listNotebooks(false) } }
                .onSuccess { notebooks = it }
                .onFailure { error = it.message ?: it.toString() }
            loading = false
        }
    }

    fun perform(action: () -> Unit) {
        error = null
        scope.launch {
            runCatching { withContext(Dispatchers.IO) { action() } }
                .onFailure { error = it.message ?: it.toString() }
            refresh()
        }
    }

    LaunchedEffect(Unit) { refresh() }

    Column(Modifier.fillMaxSize().padding(20.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
            Button(onClick = onAddNotebook) { Text(stringResource(R.string.notebooks_add)) }
        }
        Text(stringResource(R.string.notebooks_title), style = MaterialTheme.typography.headlineMedium)
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        when {
            loading -> Text(stringResource(R.string.common_loading))
            notebooks.isEmpty() -> Text(stringResource(R.string.notebooks_empty))
            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                items(notebooks, key = { it.id }) { notebook ->
                    Card(Modifier.fillMaxWidth()) {
                        Column(
                            Modifier.padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Text(notebook.displayName, style = MaterialTheme.typography.titleMedium)
                            if (notebook.active) {
                                Text(
                                    stringResource(R.string.notebooks_active),
                                    color = MaterialTheme.colorScheme.primary,
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                if (!notebook.active) {
                                    TextButton(
                                        onClick = {
                                            perform { client.setActiveNotebook(notebook.id) }
                                        },
                                    ) { Text(stringResource(R.string.notebooks_make_active)) }
                                }
                                TextButton(
                                    onClick = {
                                        renaming = notebook
                                        renameValue = notebook.displayName
                                    },
                                ) { Text(stringResource(R.string.notebooks_rename)) }
                                TextButton(
                                    onClick = {
                                        perform { client.archiveNotebook(notebook.id) }
                                    },
                                ) { Text(stringResource(R.string.notebooks_archive)) }
                            }
                        }
                    }
                }
            }
        }
        if (notebooks.any { it.active }) {
            TextButton(onClick = { perform { client.setActiveNotebook(null) } }) {
                Text(stringResource(R.string.notebooks_clear_active))
            }
        }
    }

    renaming?.let { notebook ->
        AlertDialog(
            onDismissRequest = { renaming = null },
            title = { Text(stringResource(R.string.notebooks_rename_title)) },
            text = {
                OutlinedTextField(
                    value = renameValue,
                    onValueChange = { renameValue = it },
                    label = { Text(stringResource(R.string.notebooks_name)) },
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        renaming = null
                        perform { client.renameNotebook(notebook.id, renameValue) }
                    },
                    enabled = renameValue.isNotBlank(),
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
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/notebook/PageCodeScreen.kt",
    r'''package com.a2d.notebook.feature.notebook

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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.QrCaptureButton
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.PageResolution

@Composable
fun PageCodeScreen(
    notebookId: String,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val client = remember { A2dBridge.client(context) }
    val scope = rememberCoroutineScope()
    var payload by rememberSaveable { mutableStateOf("") }
    var resolution by remember { mutableStateOf<PageResolution?>(null) }
    var busy by remember { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }

    fun resolve(candidate: String) {
        payload = candidate
        resolution = null
        error = null
        busy = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    client.resolvePageCode(candidate, notebookId)
                }
            }.onSuccess { resolution = it }
                .onFailure { error = it.message ?: it.toString() }
            busy = false
        }
    }

    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
        Text(stringResource(R.string.page_code_title), style = MaterialTheme.typography.headlineMedium)
        Text(stringResource(R.string.page_code_destination, notebookId))
        Text(stringResource(R.string.page_code_explanation))
        QrCaptureButton(
            label = stringResource(R.string.page_code_scan),
            prefix = "page-code-",
            onDecoded =(::resolve),
            onFailure = { error = it?.message ?: context.getString(R.string.setup_capture_failed) },
        )
        OutlinedTextField(
            value = payload,
            onValueChange = { payload = it },
            label = { Text(stringResource(R.string.page_code_payload)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
        )
        Button(onClick = { resolve(payload.trim()) }, enabled = payload.isNotBlank() && !busy) {
            Text(stringResource(R.string.page_code_resolve))
        }
        if (busy) Text(stringResource(R.string.common_loading))
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        resolution?.let {
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        stringResource(R.string.page_code_result),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(it.toString())
                }
            }
        }
    }
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/SmartPageForm.kt",
    r'''package com.a2d.notebook.feature.smartpage

data class ValidatedSmartPageForm(
    val pageCount: UInt,
    val startingVisiblePage: UInt,
)

fun validateSmartPageForm(
    pageCountText: String,
    startingVisiblePageText: String,
): Result<ValidatedSmartPageForm> {
    val pageCount = pageCountText.toUIntOrNull()
        ?: return Result.failure(IllegalArgumentException("page_count"))
    if (pageCount !in 1u..500u) {
        return Result.failure(IllegalArgumentException("page_count"))
    }
    val startingPage = startingVisiblePageText.toUIntOrNull()
        ?: return Result.failure(IllegalArgumentException("starting_page"))
    if (startingPage == 0u) {
        return Result.failure(IllegalArgumentException("starting_page"))
    }
    return Result.success(ValidatedSmartPageForm(pageCount, startingPage))
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/PdfSupport.kt",
    r'''package com.a2d.notebook.feature.smartpage

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.pdf.PdfRenderer
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.print.PageRange
import android.print.PrintAttributes
import android.print.PrintDocumentAdapter
import android.print.PrintDocumentInfo
import android.print.PrintManager
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import kotlin.math.roundToInt

fun renderFirstPdfPage(path: String): Bitmap {
    val file = File(path)
    ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY).use { descriptor ->
        PdfRenderer(descriptor).use { renderer ->
            require(renderer.pageCount > 0) { "PDF has no pages" }
            renderer.openPage(0).use { page ->
                val width = 900
                val height = (width.toFloat() * page.height / page.width).roundToInt()
                return Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).also { bitmap ->
                    bitmap.eraseColor(Color.WHITE)
                    page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
                }
            }
        }
    }
}

fun sharePdf(context: Context, path: String) {
    val file = File(path)
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = "application/pdf"
        putExtra(Intent.EXTRA_STREAM, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(Intent.createChooser(intent, null))
}

fun printPdf(context: Context, path: String, jobName: String) {
    val manager = context.getSystemService(PrintManager::class.java)
    manager.print(jobName, PdfFilePrintAdapter(File(path)), null)
}

class PdfFilePrintAdapter(private val source: File) : PrintDocumentAdapter() {
    override fun onLayout(
        oldAttributes: PrintAttributes?,
        newAttributes: PrintAttributes?,
        cancellationSignal: CancellationSignal?,
        callback: LayoutResultCallback,
        extras: android.os.Bundle?,
    ) {
        if (cancellationSignal?.isCanceled == true) {
            callback.onLayoutCancelled()
            return
        }
        callback.onLayoutFinished(
            PrintDocumentInfo.Builder(source.name)
                .setContentType(PrintDocumentInfo.CONTENT_TYPE_DOCUMENT)
                .build(),
            false,
        )
    }

    override fun onWrite(
        pages: Array<out PageRange>?,
        destination: ParcelFileDescriptor,
        cancellationSignal: CancellationSignal?,
        callback: WriteResultCallback,
    ) {
        Thread {
            try {
                if (cancellationSignal?.isCanceled == true) {
                    callback.onWriteCancelled()
                    return@Thread
                }
                FileInputStream(source).use { input ->
                    FileOutputStream(destination.fileDescriptor).use { output ->
                        input.copyTo(output)
                    }
                }
                callback.onWriteFinished(arrayOf(PageRange.ALL_PAGES))
            } catch (error: Throwable) {
                callback.onWriteFailed(error.message)
            }
        }.start()
    }
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/SmartPagesScreen.kt",
    r'''package com.a2d.notebook.feature.smartpage

import android.graphics.Bitmap
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.rustbridge.A2dBridge
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.GeneratedSmartPages
import uniffi.a2d_ffi.SmartPageContentStyle
import uniffi.a2d_ffi.SmartPageGenerationRequest
import uniffi.a2d_ffi.SmartPagePaperSize

private enum class UiPaper(val label: String) { LETTER("US Letter"), A4("A4") }
private enum class UiStyle(val label: String) { BLANK("Blank"), LINED("Lined"), DOT_GRID("Dot grid"), GRAPH("Graph") }

@Composable
fun SmartPagesScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val client = remember { A2dBridge.client(context) }
    val scope = rememberCoroutineScope()
    var paper by rememberSaveable { mutableStateOf(UiPaper.LETTER) }
    var style by rememberSaveable { mutableStateOf(UiStyle.LINED) }
    var pageCount by rememberSaveable { mutableStateOf("1") }
    var startingPage by rememberSaveable { mutableStateOf("1") }
    var generated by remember { mutableStateOf<GeneratedSmartPages?>(null) }
    var preview by remember { mutableStateOf<Bitmap?>(null) }
    var busy by remember { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    var message by rememberSaveable { mutableStateOf<String?>(null) }
    var pendingSavePath by remember { mutableStateOf<String?>(null) }

    val saveLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/pdf"),
    ) { uri ->
        val path = pendingSavePath
        pendingSavePath = null
        if (uri == null || path == null) return@rememberLauncherForActivityResult
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    context.contentResolver.openOutputStream(uri)?.use { output ->
                        File(path).inputStream().use { input -> input.copyTo(output) }
                    } ?: error("destination could not be opened")
                }
            }.onSuccess { message = context.getString(R.string.smart_pages_saved) }
                .onFailure { error = context.getString(R.string.smart_pages_save_failed) }
        }
    }

    fun generate() {
        val validated = validateSmartPageForm(pageCount, startingPage)
        if (validated.isFailure) {
            error = when (validated.exceptionOrNull()?.message) {
                "starting_page" -> context.getString(R.string.smart_pages_invalid_start)
                else -> context.getString(R.string.smart_pages_invalid_count)
            }
            return
        }
        val values = validated.getOrThrow()
        error = null
        message = null
        generated = null
        preview = null
        busy = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    client.generateSmartPages(
                        SmartPageGenerationRequest(
                            paperSize = when (paper) {
                                UiPaper.LETTER -> SmartPagePaperSize.US_LETTER
                                UiPaper.A4 -> SmartPagePaperSize.A4
                            },
                            style = when (style) {
                                UiStyle.BLANK -> SmartPageContentStyle.BLANK
                                UiStyle.LINED -> SmartPageContentStyle.LINED
                                UiStyle.DOT_GRID -> SmartPageContentStyle.DOT_GRID
                                UiStyle.GRAPH -> SmartPageContentStyle.GRAPH
                            },
                            pageCount = values.pageCount,
                            startingVisiblePage = values.startingVisiblePage,
                        ),
                    )
                }
            }.onSuccess { generated = it }
                .onFailure { error = it.message ?: it.toString() }
            busy = false
        }
    }

    LaunchedEffect(generated?.pdfPath) {
        val path = generated?.pdfPath ?: return@LaunchedEffect
        preview = runCatching { withContext(Dispatchers.IO) { renderFirstPdfPage(path) } }.getOrNull()
    }

    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
        Text(stringResource(R.string.smart_pages_title), style = MaterialTheme.typography.headlineMedium)
        Text(stringResource(R.string.smart_pages_single_hint))
        Button(onClick = { paper = if (paper == UiPaper.LETTER) UiPaper.A4 else UiPaper.LETTER }) {
            Text(stringResource(R.string.smart_pages_paper, paper.label))
        }
        Button(onClick = { style = UiStyle.entries[(style.ordinal + 1) % UiStyle.entries.size] }) {
            Text(stringResource(R.string.smart_pages_style, style.label))
        }
        OutlinedTextField(
            value = pageCount,
            onValueChange = { pageCount = it.filter(Char::isDigit) },
            label = { Text(stringResource(R.string.smart_pages_page_count)) },
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = startingPage,
            onValueChange = { startingPage = it.filter(Char::isDigit) },
            label = { Text(stringResource(R.string.smart_pages_start_number)) },
            modifier = Modifier.fillMaxWidth(),
        )
        Button(onClick =(::generate), enabled = !busy) {
            Text(stringResource(R.string.smart_pages_generate))
        }
        if (busy) Text(stringResource(R.string.common_loading))
        error?.let {
            Text(it, color = MaterialTheme.colorScheme.error)
            Button(onClick =(::generate)) { Text(stringResource(R.string.common_retry)) }
        }
        message?.let { Text(it, color = MaterialTheme.colorScheme.primary) }

        generated?.let { result ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        stringResource(R.string.smart_pages_generated),
                        style = MaterialTheme.typography.titleLarge,
                    )
                    Text(stringResource(R.string.smart_pages_set_id, result.pageSetId))
                    Text(stringResource(R.string.smart_pages_page_total, result.pageIds.size))
                    preview?.let { bitmap ->
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = stringResource(R.string.smart_pages_generated),
                            modifier = Modifier.fillMaxWidth().heightIn(max = 520.dp),
                        )
                    } ?: Text(stringResource(R.string.smart_pages_preview_unavailable))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = {
                                pendingSavePath = result.pdfPath
                                saveLauncher.launch("a2d-smart-pages-${result.pageSetId}.pdf")
                            },
                        ) { Text(stringResource(R.string.smart_pages_save)) }
                        Button(onClick = { sharePdf(context, result.pdfPath) }) {
                            Text(stringResource(R.string.smart_pages_share))
                        }
                        Button(
                            onClick = {
                                printPdf(
                                    context,
                                    result.pdfPath,
                                    "A2D Smart Pages ${result.pageSetId}",
                                )
                            },
                        ) { Text(stringResource(R.string.smart_pages_print)) }
                    }
                }
            }
        }
    }
}
''',
)

write(
    "apps/android/app/src/main/kotlin/com/a2d/notebook/navigation/A2dNavHost.kt",
    r'''package com.a2d.notebook.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.a2d.notebook.feature.home.HomeScreen
import com.a2d.notebook.feature.notebook.NotebookLibraryScreen
import com.a2d.notebook.feature.notebook.NotebookSetupScreen
import com.a2d.notebook.feature.notebook.PageCodeScreen
import com.a2d.notebook.feature.smartpage.SmartPagesScreen

object A2dDestinations {
    const val HOME = "home"
    const val NOTEBOOKS = "notebooks"
    const val ADD_NOTEBOOK = "notebooks/add"
    const val SMART_PAGES = "smart-pages"
    const val PAGE_CODE_PATTERN = "page-code/{notebookId}"

    fun pageCode(notebookId: String) = "page-code/$notebookId"
}

@Composable
fun A2dNavHost(navController: NavHostController) {
    NavHost(navController = navController, startDestination = A2dDestinations.HOME) {
        composable(A2dDestinations.HOME) {
            HomeScreen(
                onOpenNotebooks = { navController.navigate(A2dDestinations.NOTEBOOKS) },
                onCreateSmartPages = { navController.navigate(A2dDestinations.SMART_PAGES) },
            )
        }
        composable(A2dDestinations.NOTEBOOKS) {
            NotebookLibraryScreen(
                onBack = { navController.navigateUp() },
                onAddNotebook = { navController.navigate(A2dDestinations.ADD_NOTEBOOK) },
            )
        }
        composable(A2dDestinations.ADD_NOTEBOOK) {
            NotebookSetupScreen(
                onBack = { navController.navigateUp() },
                onScanFirstPage = { notebookId ->
                    navController.navigate(A2dDestinations.pageCode(notebookId))
                },
            )
        }
        composable(A2dDestinations.SMART_PAGES) {
            SmartPagesScreen(onBack = { navController.navigateUp() })
        }
        composable(
            route = A2dDestinations.PAGE_CODE_PATTERN,
            arguments = listOf(navArgument("notebookId") { type = NavType.StringType }),
        ) { entry ->
            PageCodeScreen(
                notebookId = requireNotNull(entry.arguments?.getString("notebookId")),
                onBack = { navController.navigateUp() },
            )
        }
    }
}
''',
)

write(
    "apps/android/app/src/test/kotlin/com/a2d/notebook/feature/smartpage/SmartPageFormTest.kt",
    r'''package com.a2d.notebook.feature.smartpage

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SmartPageFormTest {
    @Test
    fun accepts_single_pages_and_page_sets() {
        assertEquals(1u, validateSmartPageForm("1", "1").getOrThrow().pageCount)
        assertEquals(20u, validateSmartPageForm("20", "5").getOrThrow().pageCount)
    }

    @Test
    fun rejects_zero_excessive_and_malformed_counts() {
        assertTrue(validateSmartPageForm("0", "1").isFailure)
        assertTrue(validateSmartPageForm("501", "1").isFailure)
        assertTrue(validateSmartPageForm("abc", "1").isFailure)
    }

    @Test
    fun rejects_zero_or_malformed_starting_numbers() {
        assertTrue(validateSmartPageForm("1", "0").isFailure)
        assertTrue(validateSmartPageForm("1", "x").isFailure)
    }
}
''',
)

print("Milestone 6 Android transformations applied")
