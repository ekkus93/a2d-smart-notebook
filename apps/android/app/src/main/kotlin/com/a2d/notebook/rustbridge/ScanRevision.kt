package com.a2d.notebook.rustbridge

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.GetScanRevisionProposalRequest
import uniffi.a2d_ffi.ResolveScanRevisionRequest
import uniffi.a2d_ffi.ResolvedScanRevision
import uniffi.a2d_ffi.ScanRevisionDecision
import uniffi.a2d_ffi.ScanRevisionProposal

/**
 * Thin Android projection of the Rust-owned Milestone 9.3 revision workflow.
 *
 * Kotlin does not classify scans or decide which revision actions are valid. It forwards the
 * explicit threshold and user decision to Rust, then fails closed if the returned identity or
 * no-deletion contract is internally inconsistent with the request/proposal.
 */
fun A2dClient.scanRevisionProposal(
    candidateScanId: String,
    minimumCellAbsoluteDifference: UInt,
): ScanRevisionProposal {
    val proposal =
        getScanRevisionProposal(
            GetScanRevisionProposalRequest(
                candidateScanId = candidateScanId,
                minimumCellAbsoluteDifference = minimumCellAbsoluteDifference,
            ),
        )
    requireSafeRevisionProposalContract(
        requestedCandidateScanId = candidateScanId,
        returnedCandidateScanId = proposal.candidateScanId,
        defaultDecision = proposal.defaultDecision,
        allowedDecisions = proposal.allowedDecisions,
    )
    return proposal
}

fun A2dClient.applyScanRevisionDecision(
    proposal: ScanRevisionProposal,
    decision: ScanRevisionDecision,
    decidedAtMs: Long,
    actor: String,
    physicalCopyLabel: String? = null,
): ResolvedScanRevision {
    val resolved =
        resolveScanRevision(
            buildScanRevisionResolutionRequest(
                pageId = proposal.pageId,
                baselineScanId = proposal.baselineScanId,
                candidateScanId = proposal.candidateScanId,
                decision = decision,
                decidedAtMs = decidedAtMs,
                actor = actor,
                physicalCopyLabel = physicalCopyLabel,
            ),
        )
    requireResolvedRevisionContract(
        expectedPageId = proposal.pageId,
        expectedBaselineScanId = proposal.baselineScanId,
        expectedCandidateScanId = proposal.candidateScanId,
        expectedDecision = decision,
        resolved = resolved,
    )
    return resolved
}

internal fun buildScanRevisionResolutionRequest(
    pageId: String,
    baselineScanId: String,
    candidateScanId: String,
    decision: ScanRevisionDecision,
    decidedAtMs: Long,
    actor: String,
    physicalCopyLabel: String?,
): ResolveScanRevisionRequest =
    ResolveScanRevisionRequest(
        pageId = pageId,
        baselineScanId = baselineScanId,
        candidateScanId = candidateScanId,
        decision = decision,
        decidedAtMs = decidedAtMs,
        actor = actor,
        physicalCopyLabel = physicalCopyLabel,
    )

internal fun requireSafeRevisionProposalContract(
    requestedCandidateScanId: String,
    returnedCandidateScanId: String,
    defaultDecision: ScanRevisionDecision,
    allowedDecisions: List<ScanRevisionDecision>,
) {
    check(returnedCandidateScanId == requestedCandidateScanId) {
        "Rust returned a scan-revision proposal for a different candidate"
    }
    check(defaultDecision == ScanRevisionDecision.SAVE_AS_NEW_VERSION) {
        "Rust scan-revision proposal violated the preserve-by-default contract"
    }
    check(defaultDecision in allowedDecisions) {
        "Rust scan-revision proposal omitted its default decision from the allowed decisions"
    }
}

internal fun requireResolvedRevisionContract(
    expectedPageId: String,
    expectedBaselineScanId: String,
    expectedCandidateScanId: String,
    expectedDecision: ScanRevisionDecision,
    resolved: ResolvedScanRevision,
) {
    check(resolved.pageId == expectedPageId) {
        "Rust resolved a scan revision for a different page"
    }
    check(resolved.baselineScanId == expectedBaselineScanId) {
        "Rust resolved a scan revision against a different baseline"
    }
    check(resolved.candidateScanId == expectedCandidateScanId) {
        "Rust resolved a scan revision for a different candidate"
    }
    check(resolved.decision == expectedDecision) {
        "Rust resolved a different scan-revision decision than requested"
    }
    check(!resolved.committedDataDeleted) {
        "Rust violated the Milestone 9.3 no-deletion contract"
    }
}
