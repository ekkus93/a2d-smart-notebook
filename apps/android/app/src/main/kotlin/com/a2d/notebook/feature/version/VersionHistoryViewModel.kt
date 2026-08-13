package com.a2d.notebook.feature.version

import android.app.Application
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.a2d.notebook.rustbridge.A2dBridge
import com.a2d.notebook.rustbridge.applyScanRevisionDecision
import com.a2d.notebook.rustbridge.catchingOperationFailure
import com.a2d.notebook.rustbridge.comparePageVersionsForDisplay
import com.a2d.notebook.rustbridge.enqueuePageVersionReview
import com.a2d.notebook.rustbridge.loadPageVersionTimeline
import com.a2d.notebook.rustbridge.scanRevisionProposal
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.PageVersionComparison
import uniffi.a2d_ffi.PageVersionRecord
import uniffi.a2d_ffi.PageVersionTimeline
import uniffi.a2d_ffi.ReviewItemRecord
import uniffi.a2d_ffi.ScanRevisionDecision
import uniffi.a2d_ffi.ScanRevisionProposal

private const val VISUAL_DIFFERENCE_THRESHOLD: UInt = 16u
private const val VERSION_UI_ACTOR = "android-version-ui"

data class VersionHistoryUiState(
    val timeline: PageVersionTimeline? = null,
    val selectedScanId: String? = null,
    val comparison: PageVersionComparison? = null,
    val proposal: ScanRevisionProposal? = null,
    val queuedReview: ReviewItemRecord? = null,
    val loading: Boolean = false,
    val mutating: Boolean = false,
    val error: String? = null,
) {
    val selectedVersion: PageVersionRecord?
        get() = timeline?.items?.firstOrNull { it.scanId == selectedScanId }

    val preferredVersion: PageVersionRecord?
        get() = timeline?.preferredVersion
}

/** Android presentation state only; page/version policy and mutations remain in Rust. */
class VersionHistoryViewModel(application: Application) : AndroidViewModel(application) {
    private val client = A2dBridge.client(application)
    private val mutableState = mutableStateOf(VersionHistoryUiState())
    val state: State<VersionHistoryUiState> = mutableState

    private var pageId: String? = null

    fun load(pageId: String) {
        this.pageId = pageId
        runRead {
            val timeline = withContext(Dispatchers.IO) { client.loadPageVersionTimeline(pageId) }
            currentCoroutineContext().ensureActive()
            val previousSelection = mutableState.value.selectedScanId
            val selected =
                timeline.items.firstOrNull { it.scanId == previousSelection }?.scanId
                    ?: timeline.items.firstOrNull { !it.preferred }?.scanId
                    ?: timeline.items.firstOrNull()?.scanId
            update {
                it.copy(
                    timeline = timeline,
                    selectedScanId = selected,
                    comparison = null,
                    proposal = null,
                    queuedReview = null,
                )
            }
            refreshSelectionDetails(timeline, selected)
        }
    }

    fun loadMore() {
        val current = mutableState.value
        val timeline = current.timeline ?: return
        val nextOffset = timeline.nextOffset ?: return
        val currentPageId = pageId ?: return
        runRead {
            val next =
                withContext(Dispatchers.IO) {
                    client.loadPageVersionTimeline(
                        pageId = currentPageId,
                        offset = nextOffset,
                    )
                }
            currentCoroutineContext().ensureActive()
            val existingIds = timeline.items.mapTo(mutableSetOf()) { it.scanId }
            val appended = next.items.filter { existingIds.add(it.scanId) }
            update {
                it.copy(
                    timeline =
                        timeline.copy(
                            items = timeline.items + appended,
                            hasMore = next.hasMore,
                            nextOffset = next.nextOffset,
                        ),
                )
            }
        }
    }

    fun selectVersion(scanId: String) {
        val timeline = mutableState.value.timeline ?: return
        if (timeline.items.none { it.scanId == scanId }) return
        update {
            it.copy(
                selectedScanId = scanId,
                comparison = null,
                proposal = null,
                queuedReview = null,
                error = null,
            )
        }
        runRead { refreshSelectionDetails(timeline, scanId) }
    }

    fun applyDecision(decision: ScanRevisionDecision, physicalCopyLabel: String? = null) {
        val current = mutableState.value
        val proposal = current.proposal ?: return
        if (decision !in proposal.allowedDecisions) return
        runMutation {
            withContext(Dispatchers.IO) {
                client.applyScanRevisionDecision(
                    proposal = proposal,
                    decision = decision,
                    decidedAtMs = System.currentTimeMillis(),
                    actor = VERSION_UI_ACTOR,
                    physicalCopyLabel = physicalCopyLabel,
                )
            }
            reloadAfterMutation()
        }
    }

    fun moveSelectedToReview() {
        val current = mutableState.value
        val selected = current.selectedVersion ?: return
        val currentPageId = pageId ?: return
        runMutation {
            val result =
                withContext(Dispatchers.IO) {
                    client.enqueuePageVersionReview(
                        pageId = currentPageId,
                        scanId = selected.scanId,
                        createdAtMs = System.currentTimeMillis(),
                    )
                }
            currentCoroutineContext().ensureActive()
            update { it.copy(queuedReview = result.reviewItem) }
        }
    }

    fun clearError() {
        update { it.copy(error = null) }
    }

    private suspend fun reloadAfterMutation() {
        val currentPageId = pageId ?: return
        val timeline = withContext(Dispatchers.IO) { client.loadPageVersionTimeline(currentPageId) }
        currentCoroutineContext().ensureActive()
        val selected =
            timeline.items.firstOrNull { it.scanId == mutableState.value.selectedScanId }?.scanId
                ?: timeline.items.firstOrNull { !it.preferred }?.scanId
                ?: timeline.items.firstOrNull()?.scanId
        update {
            it.copy(
                timeline = timeline,
                selectedScanId = selected,
                comparison = null,
                proposal = null,
                queuedReview = null,
            )
        }
        refreshSelectionDetails(timeline, selected)
    }

    private suspend fun refreshSelectionDetails(
        timeline: PageVersionTimeline,
        selectedScanId: String?,
    ) {
        val preferredScanId = timeline.preferredScanId ?: return
        val selected = selectedScanId ?: return
        if (selected == preferredScanId) return
        val comparison =
            withContext(Dispatchers.IO) {
                client.comparePageVersionsForDisplay(
                    baselineScanId = preferredScanId,
                    candidateScanId = selected,
                    minimumCellAbsoluteDifference = VISUAL_DIFFERENCE_THRESHOLD,
                )
            }
        val proposal =
            withContext(Dispatchers.IO) {
                client.scanRevisionProposal(
                    candidateScanId = selected,
                    minimumCellAbsoluteDifference = VISUAL_DIFFERENCE_THRESHOLD,
                )
            }
        currentCoroutineContext().ensureActive()
        update { it.copy(comparison = comparison, proposal = proposal) }
    }

    private fun runRead(block: suspend () -> Unit) {
        if (mutableState.value.loading || mutableState.value.mutating) return
        update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            val result = catchingOperationFailure { block() }
            currentCoroutineContext().ensureActive()
            result.exceptionOrNull()?.let { failure ->
                update { it.copy(error = failure.message ?: failure.toString()) }
            }
            update { it.copy(loading = false) }
        }
    }

    private fun runMutation(block: suspend () -> Unit) {
        if (mutableState.value.loading || mutableState.value.mutating) return
        update { it.copy(mutating = true, error = null) }
        viewModelScope.launch {
            val result = catchingOperationFailure { block() }
            currentCoroutineContext().ensureActive()
            result.exceptionOrNull()?.let { failure ->
                update { it.copy(error = failure.message ?: failure.toString()) }
            }
            update { it.copy(mutating = false) }
        }
    }

    private fun update(transform: (VersionHistoryUiState) -> VersionHistoryUiState) {
        mutableState.value = transform(mutableState.value)
    }
}
