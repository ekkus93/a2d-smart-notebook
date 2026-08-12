//! Atomic, audited Milestone 9.3 decisions for scans already committed by registration.

use std::collections::BTreeMap;

use a2d_domain::{
    A2dError, AssetKind, AuditEvent, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity, PageId,
    PageKind, PhysicalCopyId, QualityStatus, Scan, ScanId,
};
use rusqlite::params;

use crate::Storage;
use crate::json_columns::encode_json;
use crate::repository::{
    AssetRepository, AuditEventRepository, PageRepository, ScanRepository, map_sql_error,
};

const EXISTING_PAGE_REVIEW_WARNING: &str = "EXISTING_PAGE_SCAN_REQUIRES_REVIEW";
const DECISION_WARNING_PREFIX: &str = "REVISION_DECISION_";
const MAX_ACTOR_CHARACTERS: usize = 128;
const MAX_PHYSICAL_COPY_LABEL_CHARACTERS: usize = 80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredScanRevisionDecision {
    SaveAsNewVersion,
    AnotherPhysicalCopy,
    WrongScan,
}

impl StoredScanRevisionDecision {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SaveAsNewVersion => "SAVE_AS_NEW_VERSION",
            Self::AnotherPhysicalCopy => "ANOTHER_PHYSICAL_COPY",
            Self::WrongScan => "WRONG_SCAN_DISCARDED",
        }
    }

    fn audit_kind(self) -> &'static str {
        match self {
            Self::SaveAsNewVersion => "scan.revision_saved",
            Self::AnotherPhysicalCopy => "scan.physical_copy_assigned",
            Self::WrongScan => "scan.wrong_scan_discarded",
        }
    }

    fn warning(self) -> String {
        std::format!("{DECISION_WARNING_PREFIX}{}", self.code())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordScanRevisionDecisionRequest {
    pub page_id: PageId,
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub decision: StoredScanRevisionDecision,
    pub decided_at_ms: i64,
    pub actor: String,
    pub operation_id: AuditEventId,
    pub physical_copy_label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordScanRevisionDecisionResult {
    pub page_id: PageId,
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub decision: StoredScanRevisionDecision,
    pub candidate_physical_copy_id: Option<PhysicalCopyId>,
    pub changed: bool,
    pub audit_event_id: Option<AuditEventId>,
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
        "error.storage.scan_revision",
        message.into(),
        false,
    )
}

fn validate_actor(actor: &str) -> Result<String, A2dError> {
    let trimmed = actor.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_ACTOR_CHARACTERS {
        return Err(revision_error(
            "STORAGE_SCAN_REVISION_ACTOR_INVALID",
            ErrorCategory::Validation,
            "scan revision decisions require a non-empty bounded audit actor",
        )
        .with_detail("maximum_characters", MAX_ACTOR_CHARACTERS.to_string()));
    }
    Ok(trimmed.to_string())
}

fn validate_label(label: Option<&str>) -> Result<Option<String>, A2dError> {
    let Some(label) = label else {
        return Ok(None);
    };
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_PHYSICAL_COPY_LABEL_CHARACTERS {
        return Err(revision_error(
            "STORAGE_SCAN_REVISION_PHYSICAL_COPY_LABEL_INVALID",
            ErrorCategory::Validation,
            "physical-copy labels must be non-empty and within the portable character limit",
        )
        .with_detail(
            "maximum_characters",
            MAX_PHYSICAL_COPY_LABEL_CHARACTERS.to_string(),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

fn validate_original_asset(
    conn: &rusqlite::Connection,
    scan: &Scan,
    role: &'static str,
) -> Result<(), A2dError> {
    let asset = AssetRepository::get_asset(conn, &scan.original_asset_id)?.ok_or_else(|| {
        revision_error(
            "STORAGE_SCAN_REVISION_ORIGINAL_MISSING",
            ErrorCategory::Integrity,
            "revision decision references a scan whose original asset row is missing",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("scan_role", role)
        .with_detail("original_asset_id", scan.original_asset_id.to_string())
    })?;
    if asset.kind != AssetKind::Original || !asset.immutable {
        return Err(revision_error(
            "STORAGE_SCAN_REVISION_ORIGINAL_INVALID",
            ErrorCategory::Integrity,
            "revision decision requires each scan to retain an immutable original asset",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("scan_role", role)
        .with_detail("original_asset_id", asset.id().to_string())
        .with_detail("asset_kind", std::format!("{:?}", asset.kind))
        .with_detail("asset_immutable", asset.immutable.to_string()));
    }
    Ok(())
}

fn prior_decision(scan: &Scan) -> Option<&str> {
    scan.warnings
        .iter()
        .find(|warning| warning.starts_with(DECISION_WARNING_PREFIX))
        .map(String::as_str)
}

fn decision_warnings(scan: &Scan, decision: StoredScanRevisionDecision) -> Vec<String> {
    let mut warnings = scan
        .warnings
        .iter()
        .filter(|warning| {
            warning.as_str() != EXISTING_PAGE_REVIEW_WARNING
                && !warning.starts_with(DECISION_WARNING_PREFIX)
        })
        .cloned()
        .collect::<Vec<_>>();
    warnings.push(decision.warning());
    warnings.sort();
    warnings.dedup();
    warnings
}

fn next_copy_index(conn: &rusqlite::Connection, page_id: &PageId) -> Result<u32, A2dError> {
    let maximum = conn
        .query_row(
            "SELECT MAX(copy_index) FROM physical_copies WHERE page_id = ?1",
            [page_id.to_string()],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|error| map_sql_error("reading maximum physical-copy index", error))?
        .unwrap_or(0);
    let maximum = u32::try_from(maximum).map_err(|_| {
        revision_error(
            "STORAGE_SCAN_REVISION_PHYSICAL_COPY_INDEX_INVALID",
            ErrorCategory::Integrity,
            "stored physical-copy index is outside the portable unsigned range",
        )
        .with_detail("page_id", page_id.to_string())
        .with_detail("maximum_copy_index", maximum.to_string())
    })?;
    maximum.checked_add(1).ok_or_else(|| {
        revision_error(
            "STORAGE_SCAN_REVISION_PHYSICAL_COPY_INDEX_EXHAUSTED",
            ErrorCategory::Validation,
            "no additional physical-copy index is available for this page",
        )
        .with_detail("page_id", page_id.to_string())
    })
}

fn insert_physical_copy(
    conn: &rusqlite::Connection,
    page_id: &PageId,
    copy_index: u32,
    created_at_ms: i64,
    display_label: Option<&str>,
) -> Result<PhysicalCopyId, A2dError> {
    let id = PhysicalCopyId::try_generate()?;
    conn.execute(
        "INSERT INTO physical_copies (id, page_id, copy_index, created_at_ms, display_label) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.to_string(),
            page_id.to_string(),
            i64::from(copy_index),
            created_at_ms,
            display_label,
        ],
    )
    .map_err(|error| map_sql_error("inserting physical copy", error))?;
    Ok(id)
}

fn update_page_review_state(
    conn: &rusqlite::Connection,
    page_id: &PageId,
    decided_at_ms: i64,
) -> Result<(), A2dError> {
    let needs_review = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM scans \
             WHERE page_id = ?1 AND quality_status = 'NeedsReview')",
            [page_id.to_string()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| map_sql_error("checking unresolved scan review state", error))?;
    let state_text = if needs_review {
        "NeedsReview"
    } else {
        "Scanned"
    };
    let changed = conn
        .execute(
            "UPDATE pages SET state = ?1, updated_at_ms = ?2 WHERE id = ?3",
            params![state_text, decided_at_ms, page_id.to_string()],
        )
        .map_err(|error| map_sql_error("updating page revision state", error))?;
    if changed != 1 {
        return Err(A2dError::internal_unknown(
            "page disappeared while finalizing its scan revision decision",
        )
        .with_detail("page_id", page_id.to_string()));
    }
    Ok(())
}

impl Storage {
    /// Records one non-preference Milestone 9.3 decision in a single immediate transaction.
    ///
    /// The candidate scan and all asset rows remain present for every decision. `WrongScan` is a
    /// logical discard only; this workflow contains no file or row deletion operation.
    pub fn record_scan_revision_decision(
        &mut self,
        request: RecordScanRevisionDecisionRequest,
    ) -> Result<RecordScanRevisionDecisionResult, A2dError> {
        if request.decided_at_ms <= 0 {
            return Err(revision_error(
                "STORAGE_SCAN_REVISION_TIME_INVALID",
                ErrorCategory::Validation,
                "decided_at_ms must be a positive Unix timestamp",
            ));
        }
        if request.baseline_scan_id == request.candidate_scan_id {
            return Err(revision_error(
                "STORAGE_SCAN_REVISION_SELF_REFERENCE",
                ErrorCategory::Validation,
                "baseline and candidate must be different stored scans",
            ));
        }
        let actor = validate_actor(&request.actor)?;
        let label = validate_label(request.physical_copy_label.as_deref())?;
        if request.decision != StoredScanRevisionDecision::AnotherPhysicalCopy && label.is_some() {
            return Err(revision_error(
                "STORAGE_SCAN_REVISION_LABEL_NOT_APPLICABLE",
                ErrorCategory::Validation,
                "a physical-copy label is valid only for Another Physical Copy",
            ));
        }

        self.transaction(|tx| {
            let page = PageRepository::get_page(tx, &request.page_id)?.ok_or_else(|| {
                revision_error(
                    "STORAGE_SCAN_REVISION_PAGE_NOT_FOUND",
                    ErrorCategory::Validation,
                    "the revision decision page does not exist",
                )
                .with_detail("page_id", request.page_id.to_string())
            })?;
            let baseline = ScanRepository::get_scan(tx, &request.baseline_scan_id)?.ok_or_else(|| {
                revision_error(
                    "STORAGE_SCAN_REVISION_BASELINE_NOT_FOUND",
                    ErrorCategory::Validation,
                    "the revision baseline scan does not exist",
                )
                .with_detail("baseline_scan_id", request.baseline_scan_id.to_string())
            })?;
            let candidate = ScanRepository::get_scan(tx, &request.candidate_scan_id)?.ok_or_else(|| {
                revision_error(
                    "STORAGE_SCAN_REVISION_CANDIDATE_NOT_FOUND",
                    ErrorCategory::Validation,
                    "the revision candidate scan does not exist",
                )
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string())
            })?;
            if baseline.page_id != request.page_id || candidate.page_id != request.page_id {
                return Err(revision_error(
                    "STORAGE_SCAN_REVISION_PAGE_MISMATCH",
                    ErrorCategory::Validation,
                    "baseline and candidate must both belong to the requested page",
                )
                .with_detail("page_id", request.page_id.to_string())
                .with_detail("baseline_page_id", baseline.page_id.to_string())
                .with_detail("candidate_page_id", candidate.page_id.to_string()));
            }
            if page.preferred_scan_id.as_ref() != Some(&request.baseline_scan_id)
                || !baseline.preferred
                || candidate.preferred
            {
                return Err(revision_error(
                    "STORAGE_SCAN_REVISION_STALE_PROPOSAL",
                    ErrorCategory::Validation,
                    "the page preference changed after this revision proposal was created",
                )
                .with_detail(
                    "current_preferred_scan_id",
                    page.preferred_scan_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "none".to_string()),
                )
                .with_detail("baseline_scan_id", request.baseline_scan_id.to_string())
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string()));
            }
            validate_original_asset(tx, &baseline, "baseline")?;
            validate_original_asset(tx, &candidate, "candidate")?;

            let decision_warning = request.decision.warning();
            if let Some(existing) = prior_decision(&candidate) {
                if existing == decision_warning.as_str() {
                    return Ok(RecordScanRevisionDecisionResult {
                        page_id: request.page_id.clone(),
                        baseline_scan_id: request.baseline_scan_id.clone(),
                        candidate_scan_id: request.candidate_scan_id.clone(),
                        decision: request.decision,
                        candidate_physical_copy_id: candidate.physical_copy_id.clone(),
                        changed: false,
                        audit_event_id: None,
                    });
                }
                return Err(revision_error(
                    "STORAGE_SCAN_REVISION_DECISION_CONFLICT",
                    ErrorCategory::Validation,
                    "this candidate scan already has a different revision decision",
                )
                .with_detail("existing_decision_warning", existing)
                .with_detail("requested_decision_warning", decision_warning));
            }

            let warnings = encode_json(
                &decision_warnings(&candidate, request.decision),
                "scans.warnings",
            )?;
            let (candidate_physical_copy_id, supersedes_scan_id) = match request.decision {
                StoredScanRevisionDecision::SaveAsNewVersion => (
                    baseline.physical_copy_id.clone(),
                    Some(request.baseline_scan_id.clone()),
                ),
                StoredScanRevisionDecision::AnotherPhysicalCopy => {
                    if !matches!(page.kind, PageKind::SmartPage { .. }) {
                        return Err(revision_error(
                            "STORAGE_SCAN_REVISION_PHYSICAL_COPY_NOT_SUPPORTED",
                            ErrorCategory::Validation,
                            "Another Physical Copy is valid only for a Smart Page identity",
                        )
                        .with_detail("page_id", request.page_id.to_string()));
                    }
                    let mut copy_index = next_copy_index(tx, &request.page_id)?;
                    if baseline.physical_copy_id.is_none() {
                        let baseline_copy = insert_physical_copy(
                            tx,
                            &request.page_id,
                            copy_index,
                            request.decided_at_ms,
                            None,
                        )?;
                        let changed = tx
                            .execute(
                                "UPDATE scans SET physical_copy_id = ?1 WHERE id = ?2 AND page_id = ?3",
                                params![
                                    baseline_copy.to_string(),
                                    request.baseline_scan_id.to_string(),
                                    request.page_id.to_string(),
                                ],
                            )
                            .map_err(|error| map_sql_error("assigning baseline physical copy", error))?;
                        if changed != 1 {
                            return Err(A2dError::internal_unknown(
                                "baseline scan disappeared while assigning its physical copy",
                            ));
                        }
                        copy_index = copy_index.checked_add(1).ok_or_else(|| {
                            revision_error(
                                "STORAGE_SCAN_REVISION_PHYSICAL_COPY_INDEX_EXHAUSTED",
                                ErrorCategory::Validation,
                                "no candidate physical-copy index remains after assigning the baseline",
                            )
                        })?;
                    }
                    let candidate_copy = insert_physical_copy(
                        tx,
                        &request.page_id,
                        copy_index,
                        request.decided_at_ms,
                        label.as_deref(),
                    )?;
                    (Some(candidate_copy), None)
                }
                StoredScanRevisionDecision::WrongScan => (None, None),
            };
            let quality_status = if request.decision == StoredScanRevisionDecision::WrongScan {
                QualityStatus::Rejected
            } else {
                candidate.quality_status
            };
            let quality_text = match quality_status {
                QualityStatus::Accepted => "Accepted",
                QualityStatus::AcceptedWithWarnings => "AcceptedWithWarnings",
                QualityStatus::NeedsReview => "NeedsReview",
                QualityStatus::Rejected => "Rejected",
            };
            let changed = tx
                .execute(
                    "UPDATE scans SET physical_copy_id = ?1, quality_status = ?2, warnings = ?3, \
                     supersedes_scan_id = ?4 WHERE id = ?5 AND page_id = ?6 AND preferred = 0",
                    params![
                        candidate_physical_copy_id.as_ref().map(ToString::to_string),
                        quality_text,
                        warnings,
                        supersedes_scan_id.as_ref().map(ToString::to_string),
                        request.candidate_scan_id.to_string(),
                        request.page_id.to_string(),
                    ],
                )
                .map_err(|error| map_sql_error("recording scan revision decision", error))?;
            if changed != 1 {
                return Err(A2dError::internal_unknown(
                    "candidate scan disappeared while recording its revision decision",
                )
                .with_detail("candidate_scan_id", request.candidate_scan_id.to_string()));
            }
            update_page_review_state(tx, &request.page_id, request.decided_at_ms)?;

            let mut details = BTreeMap::new();
            details.insert("page_id".to_string(), request.page_id.to_string());
            details.insert(
                "baseline_scan_id".to_string(),
                request.baseline_scan_id.to_string(),
            );
            details.insert(
                "candidate_scan_id".to_string(),
                request.candidate_scan_id.to_string(),
            );
            details.insert("decision".to_string(), request.decision.code().to_string());
            details.insert(
                "candidate_original_asset_id".to_string(),
                candidate.original_asset_id.to_string(),
            );
            details.insert("committed_data_deleted".to_string(), "false".to_string());
            if let Some(copy_id) = candidate_physical_copy_id.as_ref() {
                details.insert("candidate_physical_copy_id".to_string(), copy_id.to_string());
            }
            let event = AuditEvent::new(
                request.operation_id.clone(),
                request.decided_at_ms,
                request.decision.audit_kind().to_string(),
                actor.clone(),
                Some(request.page_id.to_string()),
                details,
                Some(request.operation_id.to_string()),
            );
            AuditEventRepository::insert_audit_event(tx, &event)?;

            Ok(RecordScanRevisionDecisionResult {
                page_id: request.page_id.clone(),
                baseline_scan_id: request.baseline_scan_id.clone(),
                candidate_scan_id: request.candidate_scan_id.clone(),
                decision: request.decision,
                candidate_physical_copy_id,
                changed: true,
                audit_event_id: Some(request.operation_id.clone()),
            })
        })
    }
}
