package com.a2d.notebook.feature.smartpage

import android.app.Application
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.a2d.notebook.rustbridge.A2dBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.GeneratedSmartPages
import uniffi.a2d_ffi.SmartPageGenerationRequest

data class SmartPagesUiState(
    val generated: GeneratedSmartPages? = null,
    val busy: Boolean = false,
    val error: String? = null,
)

/** Platform state holder; Rust owns generation, identity creation, validation, and registration. */
class SmartPagesViewModel(application: Application) : AndroidViewModel(application) {
    private val client = A2dBridge.client(application)
    private val mutableState = mutableStateOf(SmartPagesUiState())
    val state: State<SmartPagesUiState> = mutableState

    fun generate(request: SmartPageGenerationRequest) {
        if (mutableState.value.busy) return
        mutableState.value = SmartPagesUiState(busy = true)
        viewModelScope.launch {
            runCatching {
                withContext(Dispatchers.IO) { client.generateSmartPages(request) }
            }.onSuccess { generated ->
                mutableState.value = SmartPagesUiState(generated = generated)
            }.onFailure { failure ->
                mutableState.value = SmartPagesUiState(
                    error = failure.message ?: failure.toString(),
                )
            }
        }
    }
}

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
