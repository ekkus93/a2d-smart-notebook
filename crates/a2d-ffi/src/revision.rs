//! Typed UniFFI projection for Milestone 9.3 scan-revision decisions.

use a2d_core as core;
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, ScanId};

use crate::{A2dClient, A2dFfiError, StoredScanComparisonEvidence};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ScanRevisionDecision {
    SaveAsNewVersion,
    ReplacePreferred,
    AnotherPhysicalCopy,
    WrongScan,
}

impl From<ScanRevisionDecision> for core::ScanRevisionDecision {
    fn from(value: ScanRevisionDecision) -> Self {
        match value {
            ScanRevisionDecision::SaveAsNewVersion => Self::SaveAsNewVersion,
            ScanRevisionDecision::ReplacePreferred => Self::ReplacePreferred,
            ScanRevisionDecision::AnotherPhysicalCopy => Self::AnotherPhysicalCopy,
            ScanRevisionDecision::WrongScan => Self::WrongScan,
        }
    }
}

impl From<core::ScanRevisionDecision> for ScanRevisionDecision {
    fn from(value: core::ScanRevisionDecision) -> Self {
        match value {
            core::ScanRevisionDecision::SaveAsNewVersion => Self::SaveAsNewVersion,
            core::ScanRevisionDecision::ReplacePreferred => Self::ReplacePreferred,
            core::ScanRevisionDecision::AnotherPhysicalCopy => Self::AnotherPhysicalCopy,
            core::ScanRevisionDecision::WrongScan => Self::WrongScan,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct GetScanRevisionProposalRequest {
    pub candidate_scan_id: String,
    /// Explicit aligned-cell segmentation threshold. Valid values are 1 through 255.
    pub minimum_cell_absolute_difference: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ScanRevisionProposal {
    pub page_id: String,
    pub baseline_scan_id: String,
    pub candidate_scan_id: String,
    pub default_decision: ScanRevisionDecision,
    pub allowed_decisions: Vec<ScanRevisionDecision>,
    pub comparison: StoredScanComparisonEvidence,
}

impl From<core::ScanRevisionProposal> for ScanRevisionProposal {
    fn from(value: core::ScanRevisionProposal) -> Self {
        Self {
            page_id: value.page_id.to_string(),
            baseline_scan_id: value.baseline_scan_id.to_string(),
            candidate_scan_id: value.candidate_scan_id.to_string(),
            default_decision: value.default_decision.into(),
            allowed_decisions: value
                .allowed_decisions
                .into_iter()
                .map(Into::into)
                .collect(),
            comparison: value.comparison.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ResolveScanRevisionRequest {
    pub page_id: String,
    pub baseline_scan_id: String,
    pub candidate_scan_id: String,
    pub decision: ScanRevisionDecision,
    pub decided_at_ms: i64,
    pub actor: String,
    pub physical_copy_label: Option<String>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ResolvedScanRevision {
    pub page_id: String,
    pub baseline_scan_id: String,
    pub candidate_scan_id: String,
    pub decision: ScanRevisionDecision,
    pub preferred_scan_id: String,
    pub candidate_physical_copy_id: Option<String>,
    pub changed: bool,
    pub audit_event_id: Option<String>,
    /// Always false. Wrong Scan is a logical discard that retains every committed row and asset.
    pub committed_data_deleted: bool,
}

impl From<core::ResolvedScanRevision> for ResolvedScanRevision {
    fn from(value: core::ResolvedScanRevision) -> Self {
        Self {
            page_id: value.page_id.to_string(),
            baseline_scan_id: value.baseline_scan_id.to_string(),
            candidate_scan_id: value.candidate_scan_id.to_string(),
            decision: value.decision.into(),
            preferred_scan_id: value.preferred_scan_id.to_string(),
            candidate_physical_copy_id: value.candidate_physical_copy_id.map(|id| id.to_string()),
            changed: value.changed,
            audit_event_id: value.audit_event_id.map(|id| id.to_string()),
            committed_data_deleted: value.committed_data_deleted,
        }
    }
}

fn portable_threshold(value: u32) -> Result<u8, A2dFfiError> {
    u8::try_from(value).map_err(|_| {
        A2dError::new(
            ErrorCode::new("FFI_SCAN_REVISION_THRESHOLD_OUT_OF_RANGE"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.ffi.scan_revision_threshold_out_of_range",
            "minimum_cell_absolute_difference must be representable as an unsigned byte",
            false,
        )
        .with_detail("minimum_cell_absolute_difference", value.to_string())
        .into()
    })
}

#[uniffi::export]
impl A2dClient {
    pub fn get_scan_revision_proposal(
        &self,
        request: GetScanRevisionProposalRequest,
    ) -> Result<ScanRevisionProposal, A2dFfiError> {
        self.core
            .get_scan_revision_proposal(core::GetScanRevisionProposalRequest {
                candidate_scan_id: ScanId::parse(&request.candidate_scan_id)?,
                minimum_cell_absolute_difference: portable_threshold(
                    request.minimum_cell_absolute_difference,
                )?,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn resolve_scan_revision(
        &self,
        request: ResolveScanRevisionRequest,
    ) -> Result<ResolvedScanRevision, A2dFfiError> {
        self.core
            .resolve_scan_revision(core::ResolveScanRevisionRequest {
                page_id: PageId::parse(&request.page_id)?,
                baseline_scan_id: ScanId::parse(&request.baseline_scan_id)?,
                candidate_scan_id: ScanId::parse(&request.candidate_scan_id)?,
                decision: request.decision.into(),
                decided_at_ms: request.decided_at_ms,
                actor: request.actor,
                physical_copy_label: request.physical_copy_label,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use a2d_domain::{AuditEventId, PhysicalCopyId};

    use super::*;

    #[test]
    fn resolved_projection_preserves_the_no_deletion_contract() {
        let page_id = PageId::generate();
        let baseline_scan_id = ScanId::generate();
        let candidate_scan_id = ScanId::generate();
        let copy_id = PhysicalCopyId::generate();
        let audit_id = AuditEventId::generate();
        let projected: ResolvedScanRevision = core::ResolvedScanRevision {
            page_id: page_id.clone(),
            baseline_scan_id: baseline_scan_id.clone(),
            candidate_scan_id: candidate_scan_id.clone(),
            decision: core::ScanRevisionDecision::AnotherPhysicalCopy,
            preferred_scan_id: baseline_scan_id.clone(),
            candidate_physical_copy_id: Some(copy_id.clone()),
            changed: true,
            audit_event_id: Some(audit_id.clone()),
            committed_data_deleted: false,
        }
        .into();

        assert_eq!(projected.page_id, page_id.to_string());
        assert_eq!(projected.baseline_scan_id, baseline_scan_id.to_string());
        assert_eq!(projected.candidate_scan_id, candidate_scan_id.to_string());
        assert_eq!(projected.preferred_scan_id, baseline_scan_id.to_string());
        assert_eq!(
            projected.candidate_physical_copy_id,
            Some(copy_id.to_string())
        );
        assert_eq!(projected.audit_event_id, Some(audit_id.to_string()));
        assert!(!projected.committed_data_deleted);
    }

    #[test]
    fn thresholds_above_byte_range_are_rejected_without_truncation() {
        let error = portable_threshold(256).unwrap_err();
        let A2dFfiError::Failed(details) = error;
        assert_eq!(details.code, "FFI_SCAN_REVISION_THRESHOLD_OUT_OF_RANGE");
    }
}
