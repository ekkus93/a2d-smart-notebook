package com.a2d.notebook.feature.smartpage

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
import uniffi.a2d_ffi.GeneratedSmartPages
import uniffi.a2d_ffi.SmartPageGenerationRequest

private const val PRESENTATION_MAX_PAGE_SET_PAGE_COUNT = 500
private const val PRESENTATION_MAX_QR_VISIBLE_PAGE_NUMBER = 999_999

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
            val result = catchingOperationFailure {
                withContext(Dispatchers.IO) { client.generateSmartPages(request) }
            }
            currentCoroutineContext().ensureActive()
            result.fold(
                onSuccess = { generated ->
                    mutableState.value = SmartPagesUiState(generated = generated)
                },
                onFailure = { failure ->
                    mutableState.value = SmartPagesUiState(
                        error = failure.message ?: failure.toString(),
                    )
                },
            )
        }
    }
}

data class ValidatedSmartPageForm(
    val pageCount: UInt,
    val startingVisiblePage: UInt,
)

/**
 * Presentation-only mirror of the Rust validation policy for immediate form feedback. Rust repeats
 * every check and remains authoritative for direct FFI and future platform callers.
 */
fun validateSmartPageForm(
    pageCountText: String,
    startingVisiblePageText: String,
): Result<ValidatedSmartPageForm> {
    val pageCount = pageCountText.toUIntOrNull()
        ?: return Result.failure(IllegalArgumentException("page_count"))
    if (pageCount == 0u || pageCount > PRESENTATION_MAX_PAGE_SET_PAGE_COUNT.toUInt()) {
        return Result.failure(IllegalArgumentException("page_count"))
    }

    val startingPage = startingVisiblePageText.toUIntOrNull()
        ?: return Result.failure(IllegalArgumentException("starting_page"))
    if (startingPage == 0u || startingPage > PRESENTATION_MAX_QR_VISIBLE_PAGE_NUMBER.toUInt()) {
        return Result.failure(IllegalArgumentException("starting_page"))
    }

    val lastVisiblePage = startingPage.toLong() + pageCount.toLong() - 1L
    if (lastVisiblePage > PRESENTATION_MAX_QR_VISIBLE_PAGE_NUMBER.toLong()) {
        return Result.failure(IllegalArgumentException("visible_page_range"))
    }
    return Result.success(ValidatedSmartPageForm(pageCount, startingPage))
}
