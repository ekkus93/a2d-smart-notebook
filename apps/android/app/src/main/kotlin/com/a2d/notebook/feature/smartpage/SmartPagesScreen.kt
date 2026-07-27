package com.a2d.notebook.feature.smartpage

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
import androidx.lifecycle.viewmodel.compose.viewModel
import com.a2d.notebook.R
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.SmartPageContentStyle
import uniffi.a2d_ffi.SmartPageGenerationRequest
import uniffi.a2d_ffi.SmartPagePaperSize

private enum class SmartPageMode { SINGLE, SET }
private enum class UiPaper(val label: String) { LETTER("US Letter"), A4("A4") }
private enum class UiStyle(val label: String) {
    BLANK("Blank"), LINED("Lined"), DOT_GRID("Dot grid"), GRAPH("Graph")
}

@Composable
fun SmartPagesScreen(
    onBack: () -> Unit,
    viewModel: SmartPagesViewModel = viewModel(),
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by viewModel.state
    var mode by rememberSaveable { mutableStateOf(SmartPageMode.SINGLE) }
    var paper by rememberSaveable { mutableStateOf(UiPaper.LETTER) }
    var style by rememberSaveable { mutableStateOf(UiStyle.LINED) }
    var pageCount by rememberSaveable { mutableStateOf("5") }
    var startingPage by rememberSaveable { mutableStateOf("1") }
    var formError by rememberSaveable { mutableStateOf<String?>(null) }
    var platformMessage by rememberSaveable { mutableStateOf<String?>(null) }
    var preview by remember { mutableStateOf<Bitmap?>(null) }
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
            }.onSuccess {
                platformMessage = context.getString(R.string.smart_pages_saved)
            }.onFailure {
                platformMessage = context.getString(R.string.smart_pages_save_failed)
            }
        }
    }

    fun generate() {
        val countText = if (mode == SmartPageMode.SINGLE) "1" else pageCount
        val validated = validateSmartPageForm(countText, startingPage)
        if (validated.isFailure) {
            formError = when (validated.exceptionOrNull()?.message) {
                "starting_page" -> context.getString(R.string.smart_pages_invalid_start)
                else -> context.getString(R.string.smart_pages_invalid_count)
            }
            return
        }
        val values = validated.getOrThrow()
        formError = null
        platformMessage = null
        preview = null
        viewModel.generate(
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

    LaunchedEffect(state.generated?.pdfPath) {
        val path = state.generated?.pdfPath ?: return@LaunchedEffect
        preview = runCatching {
            withContext(Dispatchers.IO) { renderFirstPdfPage(path) }
        }.getOrNull()
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        TextButton(onClick = onBack) { Text(stringResource(R.string.common_back)) }
        Text(stringResource(R.string.smart_pages_title), style = MaterialTheme.typography.headlineMedium)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = { mode = SmartPageMode.SINGLE }) {
                Text(stringResource(R.string.smart_pages_single))
            }
            Button(onClick = { mode = SmartPageMode.SET }) {
                Text(stringResource(R.string.smart_pages_set))
            }
        }
        Button(onClick = { paper = if (paper == UiPaper.LETTER) UiPaper.A4 else UiPaper.LETTER }) {
            Text(stringResource(R.string.smart_pages_paper, paper.label))
        }
        Button(onClick = { style = UiStyle.entries[(style.ordinal + 1) % UiStyle.entries.size] }) {
            Text(stringResource(R.string.smart_pages_style, style.label))
        }
        if (mode == SmartPageMode.SET) {
            OutlinedTextField(
                value = pageCount,
                onValueChange = { pageCount = it.filter(Char::isDigit) },
                label = { Text(stringResource(R.string.smart_pages_page_count)) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
        OutlinedTextField(
            value = startingPage,
            onValueChange = { startingPage = it.filter(Char::isDigit) },
            label = { Text(stringResource(R.string.smart_pages_start_number)) },
            modifier = Modifier.fillMaxWidth(),
        )
        Button(onClick = ::generate, enabled = !state.busy) {
            Text(stringResource(R.string.smart_pages_generate))
        }
        if (state.busy) Text(stringResource(R.string.common_loading))
        (formError ?: state.error)?.let { error ->
            Text(stringResource(R.string.common_error_prefix, error), color = MaterialTheme.colorScheme.error)
            Button(onClick = ::generate, enabled = !state.busy) {
                Text(stringResource(R.string.common_retry))
            }
        }
        platformMessage?.let { Text(it, color = MaterialTheme.colorScheme.primary) }

        state.generated?.let { result ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.smart_pages_generated), style = MaterialTheme.typography.titleLarge)
                    Text(stringResource(R.string.smart_pages_set_id, result.pageSetId))
                    Text(stringResource(R.string.smart_pages_page_total, result.pageIds.size))
                    preview?.let { bitmap ->
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = stringResource(R.string.smart_pages_preview_description),
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
                        Button(
                            onClick = {
                                runCatching { sharePdf(context, result.pdfPath) }
                                    .onFailure { platformMessage = it.message }
                            },
                        ) { Text(stringResource(R.string.smart_pages_share)) }
                        Button(
                            onClick = {
                                runCatching {
                                    printPdf(context, result.pdfPath, "A2D Smart Pages ${result.pageSetId}")
                                }.onFailure { platformMessage = it.message }
                            },
                        ) { Text(stringResource(R.string.smart_pages_print)) }
                    }
                }
            }
        }
    }
}
