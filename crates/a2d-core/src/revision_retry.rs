use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_storage::ScanRepository;

use super::{
    A2dCore, ApplyScanRevisionDecisionRequest, ApplyScanRevisionDecisionResult,
    ScanRevisionDecision,
};

const DECISION_WARNING_PREFIX: &str = "REVISION_DECISION_";

fn expected_warning(decision: ScanRevisionDecision) -> &'static str {
    match decision {
        ScanRevisionDecision::SaveAsNewVersion => "REVISION_DECISION_SAVE_AS_NEW_VERSION",
        ScanRevisionDecision::ReplacePreferred => "REVISION_DECISION_REPLACE_PREFERRED",
        ScanRevisionDecision::AnotherPhysicalCopy => "REVISION_DECISION_ANOTHER_PHYSICAL_COPY",
        ScanRevisionDecision::WrongScan => "REVISION_DECISION_WRONG_SCAN_DISCARDED",
    }
}

impl A2dCore {
    pub fn apply_revision_decision_idempotent(
        &self,
        request: ApplyScanRevisionDecisionRequest,
    ) -> Result<ApplyScanRevisionDecisionResult, A2dError> {
        let candidate = self
            .lock_storage()?
            .get_scan(&request.candidate_scan_id)?
            .ok_or_else(|| {
                A2dError::new(
                    ErrorCode::new("CORE_SCAN_REVISION_CANDIDATE_NOT_FOUND"),
                    ErrorCategory::Validation,
                    ErrorSeverity::Error,
                    "error.core.scan_revision_candidate_not_found",
                    "the revision candidate scan does not exist",
                    false,
                )
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string())
            })?;

        if let Some(existing) = candidate
            .warnings
            .iter()
            .find(|warning| warning.starts_with(DECISION_WARNING_PREFIX))
        {
            let expected = expected_warning(request.decision);
            if existing == expected {
                return Ok(ApplyScanRevisionDecisionResult {
                    page_id: candidate.page_id,
                    baseline_scan_id: request.baseline_scan_id,
                    candidate_scan_id: request.candidate_scan_id,
                    decision: request.decision,
                    candidate_physical_copy_id: candidate.physical_copy_id,
                    changed: false,
                    audit_event_id: None,
                });
            }
            return Err(A2dError::new(
                ErrorCode::new("CORE_SCAN_REVISION_DECISION_CONFLICT"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.core.scan_revision_decision_conflict",
                "the candidate scan already has a different revision decision",
                false,
            )
            .with_detail("existing_decision_warning", existing)
            .with_detail("requested_decision_warning", expected));
        }

        self.apply_revision_decision(request)
    }
}
