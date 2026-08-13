package com.a2d.notebook.rustbridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.a2d_ffi.ReviewItemKind
import uniffi.a2d_ffi.ReviewItemMutationResult
import uniffi.a2d_ffi.ReviewItemRecord
import uniffi.a2d_ffi.ReviewItemStatus

class NeedsReviewBridgeTest {
    @Test
    fun generatedReviewKindsCoverEveryMilestone94Category() {
        assertEquals(
            listOf(
                ReviewItemKind.UNIDENTIFIED_PAGE,
                ReviewItemKind.NOTEBOOK_SELECTION,
                ReviewItemKind.WRONG_NOTEBOOK,
                ReviewItemKind.LOW_QUALITY,
                ReviewItemKind.MANUAL_ALIGNMENT,
                ReviewItemKind.DUPLICATE,
                ReviewItemKind.REVISION,
                ReviewItemKind.PHYSICAL_COPY,
                ReviewItemKind.OCR_FAILURE,
                ReviewItemKind.PROCESSING_FAILURE,
                ReviewItemKind.IMPORT_CONFLICT,
                ReviewItemKind.RESTORE_CONFLICT,
            ),
            ReviewItemKind.entries,
        )
    }

    @Test
    fun generatedReviewStatusesIncludeDeferredAsNonterminalState() {
        assertEquals(
            listOf(
                ReviewItemStatus.OPEN,
                ReviewItemStatus.DEFERRED,
                ReviewItemStatus.RESOLVED,
                ReviewItemStatus.DISMISSED,
            ),
            ReviewItemStatus.entries,
        )
    }

    @Test
    fun resolutionRequestForwardsCodeVerbatimWithoutKotlinPolicy() {
        val request =
            buildReviewResolutionRequest(
                reviewItemId = "review",
                resolutionCode = "even free form reaches Rust unchanged",
                resolvedAtMs = 1234L,
                actor = "android-user",
            )
        assertEquals("review", request.reviewItemId)
        assertEquals("even free form reaches Rust unchanged", request.resolutionCode)
        assertEquals(1234L, request.resolvedAtMs)
        assertEquals("android-user", request.actor)
    }

    @Test
    fun mutationContractFailsClosedIfRustReportsCommittedDeletion() {
        val item =
            ReviewItemRecord(
                id = "review",
                kind = ReviewItemKind.REVISION,
                pageId = null,
                scanId = null,
                severity = "Warning",
                status = ReviewItemStatus.RESOLVED,
                details = emptyList(),
                resolutionCode = "KEEP_BOTH_VERSIONS",
                createdAtMs = 100L,
                resolvedAtMs = 200L,
            )
        val result =
            ReviewItemMutationResult(
                item = item,
                changed = true,
                auditEventId = "audit",
                committedDataDeleted = true,
            )

        assertThrows(IllegalStateException::class.java) {
            requireReviewMutationContract(
                expectedReviewItemId = "review",
                expectedStatus = ReviewItemStatus.RESOLVED,
                expectedResolutionCode = "KEEP_BOTH_VERSIONS",
                result = result,
            )
        }
    }
}
