package com.a2d.notebook.rustbridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.a2d_ffi.ResolvedScanRevision
import uniffi.a2d_ffi.ScanRevisionDecision

class ScanRevisionBridgeTest {
    @Test
    fun generatedDecisionSetContainsEveryRustOwnedRevisionChoice() {
        assertEquals(
            listOf(
                ScanRevisionDecision.SAVE_AS_NEW_VERSION,
                ScanRevisionDecision.REPLACE_PREFERRED,
                ScanRevisionDecision.ANOTHER_PHYSICAL_COPY,
                ScanRevisionDecision.WRONG_SCAN,
            ),
            ScanRevisionDecision.entries,
        )
    }

    @Test
    fun resolutionRequestForwardsTheExplicitDecisionWithoutKotlinPolicy() {
        val request =
            buildScanRevisionResolutionRequest(
                pageId = "page",
                baselineScanId = "baseline",
                candidateScanId = "candidate",
                decision = ScanRevisionDecision.WRONG_SCAN,
                decidedAtMs = 1234L,
                actor = "android-user",
                physicalCopyLabel = "forwarded-verbatim",
            )

        assertEquals("page", request.pageId)
        assertEquals("baseline", request.baselineScanId)
        assertEquals("candidate", request.candidateScanId)
        assertEquals(ScanRevisionDecision.WRONG_SCAN, request.decision)
        assertEquals(1234L, request.decidedAtMs)
        assertEquals("android-user", request.actor)
        assertEquals("forwarded-verbatim", request.physicalCopyLabel)
    }

    @Test
    fun proposalContractFailsClosedIfRustChangesTheSafeDefault() {
        assertThrows(IllegalStateException::class.java) {
            requireSafeRevisionProposalContract(
                requestedCandidateScanId = "candidate",
                returnedCandidateScanId = "candidate",
                defaultDecision = ScanRevisionDecision.REPLACE_PREFERRED,
                allowedDecisions =
                    listOf(
                        ScanRevisionDecision.SAVE_AS_NEW_VERSION,
                        ScanRevisionDecision.REPLACE_PREFERRED,
                    ),
            )
        }
    }

    @Test
    fun resolvedContractFailsClosedIfRustEverReportsCommittedDeletion() {
        val resolved =
            ResolvedScanRevision(
                pageId = "page",
                baselineScanId = "baseline",
                candidateScanId = "candidate",
                decision = ScanRevisionDecision.WRONG_SCAN,
                preferredScanId = "baseline",
                candidatePhysicalCopyId = null,
                changed = true,
                auditEventId = "audit",
                committedDataDeleted = true,
            )

        assertThrows(IllegalStateException::class.java) {
            requireResolvedRevisionContract(
                expectedPageId = "page",
                expectedBaselineScanId = "baseline",
                expectedCandidateScanId = "candidate",
                expectedDecision = ScanRevisionDecision.WRONG_SCAN,
                resolved = resolved,
            )
        }
    }
}
