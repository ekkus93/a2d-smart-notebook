package com.a2d.notebook.feature.smartpage

import android.app.Application
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.viewModelScope
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.catchingOperationFailure
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.GeneratedSmartPages
import uniffi.a2d_ffi.SmartPageGenerationRequest

private const val PRESENTATION_MAX_PAGE_SET_PAGE_COUNT = 500
private const val PRESENTATION_MAX_QR_VISIBLE_PAGE_NUMBER = 999_999

private const val PENDING_SAVE_TOKEN_KEY = "smart_pages.pending_save.token"
private const val PENDING_SAVE_ASSET_ID_KEY = "smart_pages.pending_save.asset_id"
private const val PENDING_SAVE_PATH_KEY = "smart_pages.pending_save.path"

data class PendingSmartPageSave(
    val token: String,
    val assetId: String,
    val path: String,
)

data class SmartPagesUiState(
    val generated: GeneratedSmartPages? = null,
    val pendingSave: PendingSmartPageSave? = null,
    val busy: Boolean = false,
    val error: String? = null,
)

/**
 * Saved-state-backed pending operation store. Partial or blank restoration is treated as stale and
 * cleared rather than being guessed into a valid save request.
 */
internal class PendingSmartPageSaveStore(
    private val savedStateHandle: SavedStateHandle,
    private val tokenFactory: () -> String = { UUID.randomUUID().toString() },
) {
    init {
        val values = listOf(
            savedStateHandle.get<String>(PENDING_SAVE_TOKEN_KEY),
            savedStateHandle.get<String>(PENDING_SAVE_ASSET_ID_KEY),
            savedStateHandle.get<String>(PENDING_SAVE_PATH_KEY),
        )
        if (values.any { it == null } && values.any { it != null } || values.any { it?.isBlank() == true }) {
            clear()
        }
    }

    fun current(): PendingSmartPageSave? {
        val token = savedStateHandle.get<String>(PENDING_SAVE_TOKEN_KEY) ?: return null
        val assetId = savedStateHandle.get<String>(PENDING_SAVE_ASSET_ID_KEY) ?: return null
        val path = savedStateHandle.get<String>(PENDING_SAVE_PATH_KEY) ?: return null
        return PendingSmartPageSave(token = token, assetId = assetId, path = path)
    }

    fun begin(assetId: String, path: String): Result<PendingSmartPageSave> {
        if (current() != null) {
            return Result.failure(IllegalStateException("a Smart Page save operation is already pending"))
        }
        if (assetId.isBlank() || path.isBlank()) {
            return Result.failure(IllegalArgumentException("generated PDF identity is incomplete"))
        }
        val pending = PendingSmartPageSave(
            token = tokenFactory(),
            assetId = assetId,
            path = path,
        )
        if (pending.token.isBlank()) {
            return Result.failure(IllegalStateException("save token generator returned an empty token"))
        }
        savedStateHandle[PENDING_SAVE_TOKEN_KEY] = pending.token
        savedStateHandle[PENDING_SAVE_ASSET_ID_KEY] = pending.assetId
        savedStateHandle[PENDING_SAVE_PATH_KEY] = pending.path
        return Result.success(pending)
    }

    fun consume(): Result<PendingSmartPageSave> {
        val pending = current()
            ?: return Result.failure(IllegalStateException("no Smart Page save operation is pending"))
        clear()
        return Result.success(pending)
    }

    private fun clear() {
        savedStateHandle.remove<String>(PENDING_SAVE_TOKEN_KEY)
        savedStateHandle.remove<String>(PENDING_SAVE_ASSET_ID_KEY)
        savedStateHandle.remove<String>(PENDING_SAVE_PATH_KEY)
    }
}

/** Platform state holder; Rust owns generation, identity creation, validation, and registration. */
class SmartPagesViewModel(
    application: Application,
    savedStateHandle: SavedStateHandle,
) : AndroidViewModel(application) {
    private val client = A2dBridge.client(application)
    private val pendingSaveStore = PendingSmartPageSaveStore(savedStateHandle)
    private val mutableState = mutableStateOf(
        SmartPagesUiState(pendingSave = pendingSaveStore.current()),
    )
    val state: State<SmartPagesUiState> = mutableState

    fun generate(request: SmartPageGenerationRequest) {
        if (mutableState.value.busy || mutableState.value.pendingSave != null) return
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

    fun beginSave(generated: GeneratedSmartPages): Result<PendingSmartPageSave> {
        val result = pendingSaveStore.begin(generated.pdfAssetId, generated.pdfPath)
        result.onSuccess { pending ->
            mutableState.value = mutableState.value.copy(pendingSave = pending)
        }
        return result
    }

    fun consumePendingSave(): Result<PendingSmartPageSave> {
        val result = pendingSaveStore.consume()
        if (result.isSuccess) {
            mutableState.value = mutableState.value.copy(pendingSave = null)
        }
        return result
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
