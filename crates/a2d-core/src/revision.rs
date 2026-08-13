//! Safe scan-revision proposals and explicit user decisions for Milestone 9.3.

use a2d_domain::{
    A2dError, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity, PageId, PageKind,
    PhysicalCopyId, ScanId,
};
use a2d_storage::{
    ChangePreferredScanRequest, PageRepository, RecordScanRevisionDecisionRequest, ScanRepository,
    StoredScanRevisionDecision,
};

use crate::{A2dCore, CompareStoredScansRequest, StoredScanComparisonEvidence};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanRevisionDecision {
    SaveAsNewVersion,
    ReplacePreferred,
    AnotherPhysicalCopy,
    WrongScan,
}

impl ScanRevisionDecision {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SaveAsNewVersion => "SAVE_AS_NEW_VERSION",
            Self::ReplacePreferred => "REPLACE_PREFERRED",
            Self::AnotherPhysicalCopy => "ANOTHER_PHYSICAL_COPY",
            Self::WrongScan => "WRONG_SCAN_DISCARDED",
        }
    }
}

impl TryFrom<ScanRevisionDecision> for StoredScanRevisionDecision {
    type Error = A2dError;

    fn try_from(value: ScanRevisionDecision) -> Result<Self, Self::Error> {
        match value {
            ScanRevisionDecision::SaveAsNewVersion => Ok(Self::SaveAsNewVersion),
            ScanRevisionDecision::AnotherPhysicalCopy => Ok(Self::AnotherPhysicalCopy),
            ScanRevisionDecision::WrongScan => Ok(Self::WrongScan),
            ScanRevisionDecision::ReplacePreferred => Err(revision_error(
                "CORE_SCAN_REVISION_INTERNAL_DECISION_ROUTE_INVALID",
                ErrorCategory::Internal,
                "Replace Preferred must use the dedicated atomic preference workflow",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetScanRevisionProposalRequest {
    pub candidate_scan_id: ScanId,
    pub minimum_cell_absolute_difference: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScanRevisionProposal {
    pub page_id: PageId,
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub default_decision: ScanRevisionDecision,
    pub allowed_decisions: Vec<ScanRevisionDecision>,
    pub comparison: StoredScanComparisonEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveScanRevisionRequest {
    pub page_id: PageId,
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub decision: ScanRevisionDecision,
    pub decided_at_ms: i64,
    pub actor: String,
    pub physical_copy_label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedScanRevision {
    pub page_id: PageId,
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub decision: ScanRevisionDecision,
    pub preferred_scan_id: ScanId,
    pub candidate_physical_copy_id: Option<PhysicalCopyId>,
    pub changed: bool,
    pub audit_event_id: Option<AuditEventId>,
    pub committed_data_deleted: bool,
}

fn revision_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if matches!(category, ErrorCategory::Integrity | ErrorCategory::Internal) {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.core.scan_revision",
        message.into(),
        false,
    )
}

impl A2dCore {
    /// Builds a proposal only after the candidate scan and immutable original have already been
    /// committed by `register_scan`. No decision is inferred from uncalibrated non-exact evidence.
    pub fn get_scan_revision_proposal(
        &self,
        request: GetScanRevisionProposalRequest,
    ) -> Result<ScanRevisionProposal, A2dError> {
        let (page_id, baseline_scan_id, physical_copy_allowed) = {
            let storage = self.lock_storage()?;
            let candidate = storage
                .get_scan(&request.candidate_scan_id)?
                .ok_or_else(|| {
                    revision_error(
                        "CORE_SCAN_REVISION_CANDIDATE_NOT_FOUND",
                        ErrorCategory::Validation,
                        "the requested revision candidate does not exist",
                    )
                    .with_detail("candidate_scan_id", request.candidate_scan_id.to_string())
                })?;
            if candidate.preferred {
                return Err(revision_error(
                    "CORE_SCAN_REVISION_CANDIDATE_ALREADY_PREFERRED",
                    ErrorCategory::Validation,
                    "a preferred scan does not require an existing-page revision proposal",
                )
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string()));
            }
            let page = storage.get_page(&candidate.page_id)?.ok_or_else(|| {
                revision_error(
                    "CORE_SCAN_REVISION_PAGE_NOT_FOUND",
                    ErrorCategory::Integrity,
                    "the revision candidate references a missing page",
                )
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string())
                .with_detail("page_id", candidate.page_id.to_string())
            })?;
            let baseline_scan_id = page.preferred_scan_id.clone().ok_or_else(|| {
                revision_error(
                    "CORE_SCAN_REVISION_BASELINE_UNAVAILABLE",
                    ErrorCategory::Integrity,
                    "an existing-page revision candidate requires one current preferred scan",
                )
                .with_detail("page_id", page.id().to_string())
            })?;
            if baseline_scan_id == request.candidate_scan_id {
                return Err(A2dError::internal_unknown(
                    "non-preferred revision candidate unexpectedly equals the preferred scan",
                ));
            }
            let baseline = storage.get_scan(&baseline_scan_id)?.ok_or_else(|| {
                revision_error(
                    "CORE_SCAN_REVISION_BASELINE_NOT_FOUND",
                    ErrorCategory::Integrity,
                    "the page's preferred revision baseline row is missing",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("baseline_scan_id", baseline_scan_id.to_string())
            })?;
            if baseline.page_id != candidate.page_id || !baseline.preferred {
                return Err(revision_error(
                    "CORE_SCAN_REVISION_BASELINE_INVALID",
                    ErrorCategory::Integrity,
                    "the preferred revision baseline is internally inconsistent",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("baseline_scan_id", baseline_scan_id.to_string()));
            }
            (
                candidate.page_id,
                baseline_scan_id,
                matches!(page.kind, PageKind::SmartPage { .. }),
            )
        };

        let comparison = self.compare_stored_scans(CompareStoredScansRequest {
            baseline_scan_id: baseline_scan_id.clone(),
            candidate_scan_id: request.candidate_scan_id.clone(),
            minimum_cell_absolute_difference: request.minimum_cell_absolute_difference,
        })?;
        let mut allowed_decisions = vec![
            ScanRevisionDecision::SaveAsNewVersion,
            ScanRevisionDecision::ReplacePreferred,
        ];
        if physical_copy_allowed {
            allowed_decisions.push(ScanRevisionDecision::AnotherPhysicalCopy);
        }
        allowed_decisions.push(ScanRevisionDecision::WrongScan);

        Ok(ScanRevisionProposal {
            page_id,
            baseline_scan_id,
            candidate_scan_id: request.candidate_scan_id,
            default_decision: ScanRevisionDecision::SaveAsNewVersion,
            allowed_decisions,
            comparison,
        })
    }

    /// Applies only an explicit user decision. The workflow never deletes a scan row or asset.
    pub fn resolve_scan_revision(
        &self,
        request: ResolveScanRevisionRequest,
    ) -> Result<ResolvedScanRevision, A2dError> {
        if request.decided_at_ms <= 0 {
            return Err(revision_error(
                "CORE_SCAN_REVISION_TIME_INVALID",
                ErrorCategory::Validation,
                "decided_at_ms must be a positive Unix timestamp",
            ));
        }
        let operation_id = AuditEventId::try_generate()?;
        let mut storage = self.lock_storage()?;
        let candidate = storage
            .get_scan(&request.candidate_scan_id)?
            .ok_or_else(|| {
                revision_error(
                    "CORE_SCAN_REVISION_CANDIDATE_NOT_FOUND",
                    ErrorCategory::Validation,
                    "the requested revision candidate does not exist",
                )
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string())
            })?;
        if candidate.page_id != request.page_id {
            return Err(revision_error(
                "CORE_SCAN_REVISION_PAGE_MISMATCH",
                ErrorCategory::Validation,
                "the revision candidate does not belong to the requested page",
            )
            .with_detail("page_id", request.page_id.to_string())
            .with_detail("candidate_page_id", candidate.page_id.to_string()));
        }
        let page = storage.get_page(&request.page_id)?.ok_or_else(|| {
            revision_error(
                "CORE_SCAN_REVISION_PAGE_NOT_FOUND",
                ErrorCategory::Validation,
                "the requested revision page does not exist",
            )
            .with_detail("page_id", request.page_id.to_string())
        })?;
        if request.decision == ScanRevisionDecision::ReplacePreferred
            && page.preferred_scan_id.as_ref() == Some(&request.candidate_scan_id)
            && candidate.preferred
        {
            return Ok(ResolvedScanRevision {
                page_id: request.page_id,
                baseline_scan_id: request.baseline_scan_id,
                candidate_scan_id: request.candidate_scan_id.clone(),
                decision: request.decision,
                preferred_scan_id: request.candidate_scan_id,
                candidate_physical_copy_id: candidate.physical_copy_id,
                changed: false,
                audit_event_id: None,
                committed_data_deleted: false,
            });
        }
        if page.preferred_scan_id.as_ref() != Some(&request.baseline_scan_id) {
            return Err(revision_error(
                "CORE_SCAN_REVISION_STALE_PROPOSAL",
                ErrorCategory::Validation,
                "the preferred scan changed after the revision proposal was created",
            )
            .with_detail(
                "current_preferred_scan_id",
                page.preferred_scan_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "none".to_string()),
            )
            .with_detail("baseline_scan_id", request.baseline_scan_id.to_string()));
        }
        if request.decision != ScanRevisionDecision::AnotherPhysicalCopy
            && request.physical_copy_label.is_some()
        {
            return Err(revision_error(
                "CORE_SCAN_REVISION_LABEL_NOT_APPLICABLE",
                ErrorCategory::Validation,
                "a physical-copy label is valid only for Another Physical Copy",
            ));
        }

        if request.decision == ScanRevisionDecision::ReplacePreferred {
            let changed = storage.change_preferred_scan(ChangePreferredScanRequest {
                page_id: request.page_id.clone(),
                scan_id: request.candidate_scan_id.clone(),
                changed_at_ms: request.decided_at_ms,
                actor: request.actor,
                operation_id,
            })?;
            return Ok(ResolvedScanRevision {
                page_id: request.page_id,
                baseline_scan_id: request.baseline_scan_id,
                candidate_scan_id: request.candidate_scan_id.clone(),
                decision: request.decision,
                preferred_scan_id: request.candidate_scan_id,
                candidate_physical_copy_id: candidate.physical_copy_id,
                changed: changed.changed,
                audit_event_id: changed.audit_event_id,
                committed_data_deleted: false,
            });
        }

        let result = storage.record_scan_revision_decision(RecordScanRevisionDecisionRequest {
            page_id: request.page_id.clone(),
            baseline_scan_id: request.baseline_scan_id.clone(),
            candidate_scan_id: request.candidate_scan_id.clone(),
            decision: request.decision.try_into()?,
            decided_at_ms: request.decided_at_ms,
            actor: request.actor,
            operation_id,
            physical_copy_label: request.physical_copy_label,
        })?;
        Ok(ResolvedScanRevision {
            page_id: result.page_id,
            baseline_scan_id: result.baseline_scan_id.clone(),
            candidate_scan_id: result.candidate_scan_id,
            decision: request.decision,
            preferred_scan_id: result.baseline_scan_id,
            candidate_physical_copy_id: result.candidate_physical_copy_id,
            changed: result.changed,
            audit_event_id: result.audit_event_id,
            committed_data_deleted: false,
        })
    }
}
#[path = "review.rs"]
mod review;
pub use review::*;

#[path = "batch_scanner.rs"]
mod batch_scanner;
pub use batch_scanner::*;

#[cfg(test)]
#[path = "review_tests.rs"]
mod review_tests;
