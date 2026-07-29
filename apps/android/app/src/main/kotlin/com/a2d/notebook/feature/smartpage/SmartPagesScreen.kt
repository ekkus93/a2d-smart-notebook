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
import androidx.compose.runtime.DisposableEffect
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
import com.a2d.notebook.rustbridge.catchingOperationFailure
import com.a2d.notebook.rustbridge.resolveGeneratedPdfAsset
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
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

private sealed interface SmartPagePreviewState {
    data object Idle : SmartPagePreviewState
    data object Loading : SmartPagePreviewState
    data class Ready(val bitmap: Bitmap) : SmartPagePreviewState
    data class Failed(val message: String) : SmartPagePreviewState
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
    var platformError by rememberSaveable { mutableStateOf<String?>(null) }
    var preview by remember { mutableStateOf<SmartPagePreviewState>(SmartPagePreviewState.Idle) }

    fun replacePreview(next: SmartPagePreviewState) {
        val oldBitmap = (preview as? SmartPagePreviewState.Ready)?.bitmap
        val nextBitmap = (next as? SmartPagePreviewState.Ready)?.bitmap
        preview = next
        if (oldBitmap != null && oldBitmap !== nextBitmap && !oldBitmap.isRecycled) {
            oldBitmap.recycle()
        }
    }

    DisposableEffect(Unit) {
        onDispose {
            (preview as? SmartPagePreviewState.Ready)?.bitmap
                ?.takeUnless(Bitmap::isRecycled)
                ?.recycle()
        }
    }

    val saveLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/pdf"),
    ) { uri ->
        val pending = viewModel.consumePendingSave().getOrElse {
            platformMessage = null
            platformError = context.getString(R.string.smart_pages_save_stale)
            return@rememberLauncherForActivityResult
        }
        if (uri == null) {
            platformMessage = null
            platformError = null
            return@rememberLauncherForActivityResult
        }

        scope.launch {
            val result = catchingOperationFailure {
                withContext(Dispatchers.IO) {
                    val source = resolveGeneratedPdfAsset(
                        context = context,
                        assetId = pending.assetId,
                        path = pending.path,
                    )
                    context.contentResolver.openOutputStream(uri, "w")?.use { output ->
                        source.inputStream().use { input -> input.copyTo(output) }
                        output.flush()
                    } ?: throw IOException("selected destination could not be opened")
                }
            }
            currentCoroutineContext().ensureActive()
            result.fold(
                onSuccess = {
                    platformError = null
                    platformMessage = context.getString(R.string.smart_pages_saved)
                },
                onFailure = {
                    platformMessage = null
                    platformError = context.getString(R.string.smart_pages_save_failed)
                },
            )
        }
    }

    fun generate() {
        val countText = if (mode == SmartPageMode.SINGLE) "1" else pageCount
        val validated = validateSmartPageForm(countText, startingPage)
        if (validated.isFailure) {
            formError = when (validated.exceptionOrNull()?.message) {
                "starting_page" -> context.getString(R.string.smart_pages_invalid_start)
                "visible_page_range" -> context.getString(R.string.smart_pages_invalid_range)
                else -> context.getString(R.string.smart_pages_invalid_count)
            }
            return
        }
        val values = validated.getOrThrow()
        formError = null
        platformMessage = null
        platformError = null
        replacePreview(SmartPagePreviewState.Idle)
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

    LaunchedEffect(state.generated?.pdfAssetId, state.generated?.pdfPath) {
        val generated = state.generated
        if (generated == null) {
            replacePreview(SmartPagePreviewState.Idle)
            return@LaunchedEffect
        }

        replacePreview(SmartPagePreviewState.Loading)
        var renderedBitmap: Bitmap? = null
        try {
            val result = catchingOperationFailure {
                withContext(Dispatchers.IO) {
                    val source = resolveGeneratedPdfAsset(
                        context = context,
                        assetId = generated.pdfAssetId,
                        path = generated.pdfPath,
                    )
                    renderFirstPdfPage(source.absolutePath).also { renderedBitmap = it }
                }
            }
            currentCoroutineContext().ensureActive()
            result.fold(
                onSuccess = { bitmap ->
                    replacePreview(SmartPagePreviewState.Ready(bitmap))
                    renderedBitmap = null
                },
                onFailure = {
                    replacePreview(
                        SmartPagePreviewState.Failed(
                            context.getString(R.string.smart_pages_preview_failed),
                        ),
                    )
                },
            )
        } finally {
            renderedBitmap
                ?.takeUnless(Bitmap::isRecycled)
                ?.recycle()
        }
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
        Button(
            onClick = ::generate,
            enabled = !state.busy && state.pendingSave == null,
        ) {
            Text(stringResource(R.string.smart_pages_generate))
        }
        if (state.busy) Text(stringResource(R.string.common_loading))
        (formError ?: state.error)?.let { error ->
            Text(stringResource(R.string.common_error_prefix, error), color = MaterialTheme.colorScheme.error)
            Button(
                onClick = ::generate,
                enabled = !state.busy && state.pendingSave == null,
            ) {
                Text(stringResource(R.string.common_retry))
            }
        }
        platformMessage?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
        platformError?.let {
            Text(stringResource(R.string.common_error_prefix, it), color = MaterialTheme.colorScheme.error)
        }

        state.generated?.let { result ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.smart_pages_generated), style = MaterialTheme.typography.titleLarge)
                    Text(stringResource(R.string.smart_pages_set_id, result.pageSetId))
                    Text(stringResource(R.string.smart_pages_page_total, result.pageIds.size))
                    when (val currentPreview = preview) {
                        SmartPagePreviewState.Idle -> {
                            Text(stringResource(R.string.smart_pages_preview_unavailable))
                        }
                        SmartPagePreviewState.Loading -> {
                            Text(stringResource(R.string.smart_pages_preview_loading))
                        }
                        is SmartPagePreviewState.Failed -> {
                            Text(
                                currentPreview.message,
                                color = MaterialTheme.colorScheme.error,
                            )
                        }
                        is SmartPagePreviewState.Ready -> {
                            Image(
                                bitmap = currentPreview.bitmap.asImageBitmap(),
                                contentDescription = stringResource(R.string.smart_pages_preview_description),
                                modifier = Modifier.fillMaxWidth().heightIn(max = 520.dp),
                            )
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            enabled = state.pendingSave == null,
                            onClick = {
                                platformMessage = null
                                platformError = null
                                val pending = viewModel.beginSave(result).getOrElse {
                                    platformError = context.getString(R.string.smart_pages_save_pending)
                                    return@Button
                                }
                                try {
                                    saveLauncher.launch("a2d-smart-pages-${result.pageSetId}.pdf")
                                } catch (failure: Exception) {
                                    val consumed = viewModel.consumePendingSave()
                                    if (consumed.isFailure || consumed.getOrNull()?.token != pending.token) {
                                        platformError = context.getString(R.string.smart_pages_save_stale)
                                    } else {
                                        platformError = context.getString(R.string.smart_pages_save_failed)
                                    }
                                }
                            },
                        ) { Text(stringResource(R.string.smart_pages_save)) }
                        Button(
                            onClick = {
                                platformMessage = null
                                platformError = try {
                                    val source = resolveGeneratedPdfAsset(
                                        context = context,
                                        assetId = result.pdfAssetId,
                                        path = result.pdfPath,
                                    )
                                    sharePdf(context, source.absolutePath)
                                    null
                                } catch (failure: Exception) {
                                    context.getString(R.string.smart_pages_share_failed)
                                }
                            },
                        ) { Text(stringResource(R.string.smart_pages_share)) }
                        Button(
                            onClick = {
                                platformMessage = null
                                platformError = try {
                                    val source = resolveGeneratedPdfAsset(
                                        context = context,
                                        assetId = result.pdfAssetId,
                                        path = result.pdfPath,
                                    )
                                    printPdf(
                                        context,
                                        source.absolutePath,
                                        "A2D Smart Pages ${result.pageSetId}",
                                    )
                                    null
                                } catch (failure: Exception) {
                                    context.getString(R.string.smart_pages_print_failed)
                                }
                            },
                        ) { Text(stringResource(R.string.smart_pages_print)) }
                    }
                }
            }
        }
    }
}
