package com.a2d.notebook.feature.notebook

import android.app.Application
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.catchingOperationFailure
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.CreateNotebookRequest
import uniffi.a2d_ffi.CreatedNotebook
import uniffi.a2d_ffi.NotebookDesignSummary
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution

data class NotebookUiState(
    val notebooks: List<NotebookSummary> = emptyList(),
    val recognizedDesign: NotebookDesignSummary? = null,
    val createdNotebook: CreatedNotebook? = null,
    val pageResolution: PageResolution? = null,
    val busy: Boolean = false,
    val error: String? = null,
)

/**
 * Platform state holder only. All identity parsing, persistence, ambiguity handling, and notebook
 * business rules are delegated to the typed Rust A2dClient.
 */
class NotebookViewModel(application: Application) : AndroidViewModel(application) {
    private val client = A2dBridge.client(application)
    private val mutableState = mutableStateOf(NotebookUiState())
    val state: State<NotebookUiState> = mutableState

    fun clearTransientResult() {
        update {
            it.copy(
                recognizedDesign = null,
                createdNotebook = null,
                pageResolution = null,
                error = null,
            )
        }
    }

    fun reportPlatformError(message: String) {
        update { it.copy(error = message, busy = false) }
    }

    fun refreshNotebooks() = runOperation {
        val notebooks = withContext(Dispatchers.IO) { client.listNotebooks(false) }
        updateAfterSuspension { it.copy(notebooks = notebooks) }
    }

    fun resolveSetupCode(
        payload: String,
        onRecognized: (NotebookDesignSummary) -> Unit = {},
    ) = runOperation {
        val design = withContext(Dispatchers.IO) { client.resolveNotebookSetupCode(payload) }
        currentCoroutineContext().ensureActive()
        update { it.copy(recognizedDesign = design, createdNotebook = null) }
        onRecognized(design)
    }

    fun createNotebook(request: CreateNotebookRequest) = runOperation {
        val created = withContext(Dispatchers.IO) { client.createNotebook(request) }
        updateAfterSuspension { it.copy(createdNotebook = created, recognizedDesign = null) }
    }

    fun renameNotebook(id: String, displayName: String) = runOperation {
        withContext(Dispatchers.IO) { client.renameNotebook(id, displayName) }
        val notebooks = withContext(Dispatchers.IO) { client.listNotebooks(false) }
        updateAfterSuspension { it.copy(notebooks = notebooks) }
    }

    fun archiveNotebook(id: String) = runOperation {
        withContext(Dispatchers.IO) { client.archiveNotebook(id) }
        val notebooks = withContext(Dispatchers.IO) { client.listNotebooks(false) }
        updateAfterSuspension { it.copy(notebooks = notebooks) }
    }

    fun setActiveNotebook(id: String?) = runOperation {
        withContext(Dispatchers.IO) { client.setActiveNotebook(id) }
        val notebooks = withContext(Dispatchers.IO) { client.listNotebooks(false) }
        updateAfterSuspension { it.copy(notebooks = notebooks) }
    }

    fun resolvePageCode(payload: String, confirmedNotebookId: String?) = runOperation {
        val resolution = withContext(Dispatchers.IO) {
            client.resolvePageCode(payload, confirmedNotebookId)
        }
        updateAfterSuspension { it.copy(pageResolution = resolution) }
    }

    private fun runOperation(block: suspend () -> Unit) {
        if (mutableState.value.busy) return
        update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            val result = catchingOperationFailure { block() }
            currentCoroutineContext().ensureActive()
            result.exceptionOrNull()?.let { failure ->
                update { it.copy(error = failure.message ?: failure.toString()) }
            }
            update { it.copy(busy = false) }
        }
    }

    private suspend fun updateAfterSuspension(
        transform: (NotebookUiState) -> NotebookUiState,
    ) {
        currentCoroutineContext().ensureActive()
        update(transform)
    }

    private fun update(transform: (NotebookUiState) -> NotebookUiState) {
        mutableState.value = transform(mutableState.value)
    }
}
