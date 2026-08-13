package com.a2d.notebook.rustbridge

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.ComparePageVersionsRequest
import uniffi.a2d_ffi.GetPageVersionTimelineRequest
import uniffi.a2d_ffi.MovePageVersionToReviewRequest
import uniffi.a2d_ffi.PageVersionComparison
import uniffi.a2d_ffi.PageVersionReviewResult
import uniffi.a2d_ffi.PageVersionTimeline

/** Thin Android projection of Rust-owned Milestone 9.5 version history. */
fun A2dClient.loadPageVersionTimeline(
    pageId: String,
    limit: UInt = 50u,
    offset: UInt = 0u,
): PageVersionTimeline {
    val timeline =
        getPageVersionTimeline(
            GetPageVersionTimelineRequest(
                pageId = pageId,
                limit = limit,
                offset = offset,
            ),
        )
    requireVersionTimelineContract(pageId, timeline)
    return timeline
}

fun A2dClient.comparePageVersionsForDisplay(
    baselineScanId: String,
    candidateScanId: String,
    minimumCellAbsoluteDifference: UInt,
): PageVersionComparison {
    val comparison =
        comparePageVersions(
            ComparePageVersionsRequest(
                baselineScanId = baselineScanId,
                candidateScanId = candidateScanId,
                minimumCellAbsoluteDifference = minimumCellAbsoluteDifference,
            ),
        )
    requirePageVersionComparisonContract(baselineScanId, candidateScanId, comparison)
    return comparison
}

fun A2dClient.enqueuePageVersionReview(
    pageId: String,
    scanId: String,
    createdAtMs: Long,
): PageVersionReviewResult {
    val result =
        movePageVersionToReview(
            MovePageVersionToReviewRequest(
                pageId = pageId,
                scanId = scanId,
                createdAtMs = createdAtMs,
            ),
        )
    requirePageVersionReviewContract(pageId, scanId, result)
    return result
}

internal fun requireVersionTimelineContract(
    requestedPageId: String,
    timeline: PageVersionTimeline,
) {
    check(timeline.pageId == requestedPageId) { "Rust returned a different page-version timeline" }
    check(!timeline.hasMore || timeline.nextOffset != null) {
        "Rust page-version pagination reported more items without a next offset"
    }
    check(timeline.hasMore || timeline.nextOffset == null) {
        "Rust page-version pagination returned a next offset without more items"
    }
    val preferred = timeline.items.filter { it.preferred }
    check(preferred.size <= 1) { "Rust returned multiple preferred page versions" }
    timeline.preferredVersion?.let { preferredVersion ->
        check(preferredVersion.preferred) { "Rust projected its preferred page version as non-preferred" }
        check(timeline.preferredScanId == preferredVersion.scanId) {
            "Rust preferred page-version pointer disagrees with the projected preferred version"
        }
    }
    check((timeline.preferredScanId == null) == (timeline.preferredVersion == null)) {
        "Rust preferred page-version identity and record availability disagree"
    }
    preferred.singleOrNull()?.let { returnedPreferred ->
        check(timeline.preferredScanId == returnedPreferred.scanId) {
            "Rust preferred page-version pointer disagrees with a paginated timeline item"
        }
    }
}

internal fun requirePageVersionComparisonContract(
    baselineScanId: String,
    candidateScanId: String,
    comparison: PageVersionComparison,
) {
    check(comparison.evidence.baselineScanId == baselineScanId) {
        "Rust compared a different baseline page version"
    }
    check(comparison.evidence.candidateScanId == candidateScanId) {
        "Rust compared a different candidate page version"
    }
    check(comparison.gridColumns > 0u && comparison.gridRows > 0u) {
        "Rust returned invalid page-version comparison grid dimensions"
    }
}

internal fun requirePageVersionReviewContract(
    pageId: String,
    scanId: String,
    result: PageVersionReviewResult,
) {
    check(result.reviewItem.pageId == pageId) { "Rust queued review for a different page" }
    check(result.reviewItem.scanId == scanId) { "Rust queued review for a different scan" }
    check(!result.reviewItem.id.isBlank()) { "Rust returned an empty review-item identity" }
}
