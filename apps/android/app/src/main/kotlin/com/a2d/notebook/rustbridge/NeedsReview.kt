package com.a2d.notebook.rustbridge

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.DeferReviewItemRequest
import uniffi.a2d_ffi.GetReviewItemRequest
import uniffi.a2d_ffi.ListReviewItemsRequest
import uniffi.a2d_ffi.ResolveReviewItemRequest
import uniffi.a2d_ffi.ReviewItemKind
import uniffi.a2d_ffi.ReviewItemMutationResult
import uniffi.a2d_ffi.ReviewItemPage
import uniffi.a2d_ffi.ReviewItemRecord
import uniffi.a2d_ffi.ReviewItemStatus

/** Thin Android projection of the Rust-owned Milestone 9.4 Needs Review queue. */
fun A2dClient.loadReviewItems(
    kind: ReviewItemKind? = null,
    status: ReviewItemStatus? = null,
    pageId: String? = null,
    scanId: String? = null,
    limit: UInt = 50u,
    offset: UInt = 0u,
): ReviewItemPage {
    val page =
        listReviewItems(
            ListReviewItemsRequest(
                kind = kind,
                status = status,
                pageId = pageId,
                scanId = scanId,
                limit = limit,
                offset = offset,
            ),
        )
    requireReviewPageContract(page, kind, status, pageId, scanId)
    return page
}

fun A2dClient.loadReviewItem(reviewItemId: String): ReviewItemRecord {
    val item = getReviewItem(GetReviewItemRequest(reviewItemId = reviewItemId))
    check(item.id == reviewItemId) { "Rust returned a different review item than requested" }
    return item
}

fun A2dClient.applyReviewResolution(
    reviewItemId: String,
    resolutionCode: String,
    resolvedAtMs: Long,
    actor: String,
): ReviewItemMutationResult {
    val result =
        resolveReviewItem(
            buildReviewResolutionRequest(
                reviewItemId = reviewItemId,
                resolutionCode = resolutionCode,
                resolvedAtMs = resolvedAtMs,
                actor = actor,
            ),
        )
    requireReviewMutationContract(
        expectedReviewItemId = reviewItemId,
        expectedStatus = ReviewItemStatus.RESOLVED,
        expectedResolutionCode = resolutionCode,
        result = result,
    )
    return result
}

fun A2dClient.applyReviewDeferral(
    reviewItemId: String,
    deferredAtMs: Long,
    actor: String,
): ReviewItemMutationResult {
    val result =
        deferReviewItem(
            buildReviewDeferralRequest(
                reviewItemId = reviewItemId,
                deferredAtMs = deferredAtMs,
                actor = actor,
            ),
        )
    requireReviewMutationContract(
        expectedReviewItemId = reviewItemId,
        expectedStatus = ReviewItemStatus.DEFERRED,
        expectedResolutionCode = null,
        result = result,
    )
    return result
}

internal fun buildReviewResolutionRequest(
    reviewItemId: String,
    resolutionCode: String,
    resolvedAtMs: Long,
    actor: String,
): ResolveReviewItemRequest =
    ResolveReviewItemRequest(
        reviewItemId = reviewItemId,
        resolutionCode = resolutionCode,
        resolvedAtMs = resolvedAtMs,
        actor = actor,
    )

internal fun buildReviewDeferralRequest(
    reviewItemId: String,
    deferredAtMs: Long,
    actor: String,
): DeferReviewItemRequest =
    DeferReviewItemRequest(
        reviewItemId = reviewItemId,
        deferredAtMs = deferredAtMs,
        actor = actor,
    )

internal fun requireReviewPageContract(
    page: ReviewItemPage,
    kind: ReviewItemKind?,
    status: ReviewItemStatus?,
    pageId: String?,
    scanId: String?,
) {
    check(!page.hasMore || page.nextOffset != null) {
        "Rust review pagination reported more items without a next offset"
    }
    check(page.hasMore || page.nextOffset == null) {
        "Rust review pagination returned a next offset without more items"
    }
    page.items.forEach { item ->
        check(kind == null || item.kind == kind) { "Rust violated the requested review-kind filter" }
        check(status == null || item.status == status) { "Rust violated the requested review-status filter" }
        check(pageId == null || item.pageId == pageId) { "Rust violated the requested review page filter" }
        check(scanId == null || item.scanId == scanId) { "Rust violated the requested review scan filter" }
    }
}

internal fun requireReviewMutationContract(
    expectedReviewItemId: String,
    expectedStatus: ReviewItemStatus,
    expectedResolutionCode: String?,
    result: ReviewItemMutationResult,
) {
    check(result.item.id == expectedReviewItemId) { "Rust mutated a different review item" }
    check(result.item.status == expectedStatus) { "Rust returned an unexpected review-item status" }
    check(result.item.resolutionCode == expectedResolutionCode) {
        "Rust returned an unexpected review-item resolution code"
    }
    check(!result.committedDataDeleted) { "Rust violated the Needs Review no-data-loss contract" }
}
