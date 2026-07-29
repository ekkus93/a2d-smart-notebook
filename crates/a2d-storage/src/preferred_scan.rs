//! Atomic preferred-scan selection with cross-record validation and auditability.
//!
//! This is the only production workflow for an explicit user preference change. The page pointer is
//! updated inside one immediate SQLite transaction; schema triggers synchronize scan flags, and the
//! workflow validates both the pre-state and post-state before committing an audit event.

use std::collections::BTreeMap;

use a2d_domain::{
    A2dError, AssetId, AssetKind, AuditEvent, AuditEventId, ErrorCategory, ErrorCode,
    ErrorSeverity, Page, PageId, Scan, ScanId,
};
use rusqlite::{Connection, params};

use crate::Storage;
use crate::repository::{
    AssetRepository, AuditEventRepository, PageRepository, ScanRepository, map_sql_error,
};

/// Complete input for one explicit preferred-scan selection.
///
/// `operation_id` is caller-generated and doubles as the audit-event ID and correlation ID. This
/// makes retries traceable and lets a duplicate operation fail transactionally instead of creating
/// an unaudited state change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangePreferredScanRequest {
    pub page_id: PageId,
    pub scan_id: ScanId,
    pub changed_at_ms: i64,
    pub actor: String,
    pub operation_id: AuditEventId,
}

/// Typed outcome of a preferred-scan request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangePreferredScanResult {
    pub page_id: PageId,
    pub previous_preferred_scan_id: Option<ScanId>,
    pub preferred_scan_id: ScanId,
    pub changed: bool,
    pub audit_event_id: Option<AuditEventId>,
}

fn preferred_scan_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.storage.preferred_scan",
        message.into(),
        false,
    )
}

fn preferred_count(conn: &Connection, page_id: &PageId) -> Result<i64, A2dError> {
    conn.query_row(
        "SELECT COUNT(*) FROM scans WHERE page_id = ?1 AND preferred = 1",
        [page_id.to_string()],
        |row| row.get(0),
    )
    .map_err(|error| map_sql_error("counting preferred scans", error))
}

fn validate_current_state(conn: &Connection, page: &Page) -> Result<(), A2dError> {
    let count = preferred_count(conn, page.id())?;
    match page.preferred_scan_id.as_ref() {
        None if count == 0 => Ok(()),
        None => Err(preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_STATE_INCONSISTENT",
            ErrorCategory::Integrity,
            "page has no preferred-scan pointer but one or more scans are flagged preferred",
        )
        .with_detail("page_id", page.id().to_string())
        .with_detail("preferred_scan_count", count.to_string())),
        Some(scan_id) => {
            let scan = ScanRepository::get_scan(conn, scan_id)?.ok_or_else(|| {
                preferred_scan_error(
                    "STORAGE_PREFERRED_SCAN_STATE_INCONSISTENT",
                    ErrorCategory::Integrity,
                    "page references a preferred scan row that does not exist",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("preferred_scan_id", scan_id.to_string())
            })?;
            if &scan.page_id != page.id() || !scan.preferred || count != 1 {
                return Err(preferred_scan_error(
                    "STORAGE_PREFERRED_SCAN_STATE_INCONSISTENT",
                    ErrorCategory::Integrity,
                    "page pointer and scan preferred flags are internally inconsistent",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("preferred_scan_id", scan_id.to_string())
                .with_detail("preferred_scan_count", count.to_string())
                .with_detail("scan_page_id", scan.page_id.to_string())
                .with_detail("scan_preferred", scan.preferred.to_string()));
            }
            Ok(())
        }
    }
}

fn validate_asset(
    conn: &Connection,
    scan: &Scan,
    asset_id: &AssetId,
    expected_kind: AssetKind,
    role: &'static str,
    require_immutable: bool,
) -> Result<(), A2dError> {
    let asset = AssetRepository::get_asset(conn, asset_id)?.ok_or_else(|| {
        preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_ASSET_MISSING",
            ErrorCategory::Integrity,
            "preferred-scan candidate references a missing asset",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("asset_id", asset_id.to_string())
        .with_detail("asset_role", role)
    })?;
    if asset.kind != expected_kind {
        return Err(preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_ASSET_KIND_INVALID",
            ErrorCategory::Integrity,
            "preferred-scan candidate references an asset with the wrong kind",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("asset_id", asset_id.to_string())
        .with_detail("asset_role", role)
        .with_detail("actual_kind", format!("{:?}", asset.kind))
        .with_detail("expected_kind", format!("{expected_kind:?}")));
    }
    if require_immutable && !asset.immutable {
        return Err(preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_ORIGINAL_INVALID",
            ErrorCategory::Integrity,
            "preferred-scan candidate does not reference an immutable original asset",
        )
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("original_asset_id", asset_id.to_string()));
    }
    Ok(())
}

fn validate_scan_assets(conn: &Connection, scan: &Scan) -> Result<(), A2dError> {
    validate_asset(
        conn,
        scan,
        &scan.original_asset_id,
        AssetKind::Original,
        "original",
        true,
    )?;
    if let Some(asset_id) = scan.corrected_asset_id.as_ref() {
        validate_asset(
            conn,
            scan,
            asset_id,
            AssetKind::Corrected,
            "corrected",
            false,
        )?;
    }
    if let Some(asset_id) = scan.ocr_asset_id.as_ref() {
        validate_asset(conn, scan, asset_id, AssetKind::Ocr, "ocr", false)?;
    }
    if let Some(asset_id) = scan.thumbnail_asset_id.as_ref() {
        validate_asset(
            conn,
            scan,
            asset_id,
            AssetKind::Thumbnail,
            "thumbnail",
            false,
        )?;
    }
    Ok(())
}

fn map_preferred_update_error(error: rusqlite::Error) -> A2dError {
    let message = error.to_string();
    if message.contains("A2D_PREFERRED_SCAN_WORKFLOW_REQUIRED") {
        return preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_WORKFLOW_REQUIRED",
            ErrorCategory::Integrity,
            "schema rejected a preferred-scan pointer change without an authorized workflow context",
        )
        .with_detail(
            "sqlite_trigger",
            "preferred_scan_pointer_update_requires_workflow",
        );
    }
    if message.contains("A2D_PREFERRED_SCAN_PAGE_MISMATCH") {
        return preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_PAGE_MISMATCH",
            ErrorCategory::Integrity,
            "schema rejected a preferred scan that belongs to another page",
        )
        .with_detail(
            "sqlite_trigger",
            "preferred_scan_page_ownership_before_update",
        );
    }
    if message.contains("A2D_PREFERRED_SCAN_FLAG_MISMATCH") {
        return preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_FLAG_MISMATCH",
            ErrorCategory::Integrity,
            "schema rejected a preferred-scan flag mutation inconsistent with the page pointer",
        )
        .with_detail(
            "sqlite_trigger",
            "preferred_scan_flag_update_requires_page_pointer",
        );
    }
    map_sql_error("change_preferred_scan", error)
}

fn enter_workflow_context(
    conn: &Connection,
    request: &ChangePreferredScanRequest,
) -> Result<(), A2dError> {
    conn.execute(
        "INSERT INTO preferred_scan_mutation_context \
         (page_id, scan_id, operation_id, source) VALUES (?1, ?2, ?3, 'explicit_change')",
        params![
            request.page_id.to_string(),
            request.scan_id.to_string(),
            request.operation_id.to_string(),
        ],
    )
    .map_err(|error| map_sql_error("entering preferred-scan workflow context", error))?;
    Ok(())
}

fn leave_workflow_context(
    conn: &Connection,
    request: &ChangePreferredScanRequest,
) -> Result<(), A2dError> {
    let changed = conn
        .execute(
            "DELETE FROM preferred_scan_mutation_context \
             WHERE page_id = ?1 AND scan_id = ?2 AND operation_id = ?3 \
               AND source = 'explicit_change'",
            params![
                request.page_id.to_string(),
                request.scan_id.to_string(),
                request.operation_id.to_string(),
            ],
        )
        .map_err(|error| map_sql_error("leaving preferred-scan workflow context", error))?;
    if changed != 1 {
        return Err(preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_CONTEXT_CLEANUP_FAILED",
            ErrorCategory::Integrity,
            "preferred-scan workflow context disappeared before transaction completion",
        )
        .with_detail("page_id", request.page_id.to_string())
        .with_detail("scan_id", request.scan_id.to_string())
        .with_detail("operation_id", request.operation_id.to_string()));
    }
    Ok(())
}

impl Storage {
    /// Compatibility shim for the removed unaudited mutation contract.
    ///
    /// Explicit preference changes require actor, timestamp, and operation identity, so this method
    /// always fails closed. Use [`Storage::change_preferred_scan`] instead.
    #[deprecated(
        since = "0.1.0",
        note = "use change_preferred_scan(ChangePreferredScanRequest)"
    )]
    pub fn set_preferred_scan(&self, page_id: &PageId, scan_id: &ScanId) -> Result<(), A2dError> {
        Err(preferred_scan_error(
            "STORAGE_PREFERRED_SCAN_WORKFLOW_REQUIRED",
            ErrorCategory::Integrity,
            "unaudited preferred-scan mutation is disabled; use change_preferred_scan",
        )
        .with_detail("page_id", page_id.to_string())
        .with_detail("scan_id", scan_id.to_string()))
    }

    /// Atomically selects a page's preferred scan and records the mutation.
    ///
    /// Re-selecting the already-consistent preferred scan is a no-op: the timestamp is preserved and
    /// no audit event is inserted. Any validation, schema, postcondition, or audit failure rolls back
    /// the page pointer and every trigger-driven scan flag change.
    pub fn change_preferred_scan(
        &mut self,
        request: ChangePreferredScanRequest,
    ) -> Result<ChangePreferredScanResult, A2dError> {
        if request.changed_at_ms <= 0 {
            return Err(preferred_scan_error(
                "STORAGE_PREFERRED_SCAN_TIME_INVALID",
                ErrorCategory::Validation,
                "changed_at_ms must be a positive Unix timestamp",
            ));
        }
        let actor = request.actor.trim();
        if actor.is_empty() {
            return Err(preferred_scan_error(
                "STORAGE_PREFERRED_SCAN_ACTOR_INVALID",
                ErrorCategory::Validation,
                "preferred-scan changes require a non-empty audit actor",
            ));
        }
        let actor = actor.to_string();

        self.transaction(|tx| {
            let page = PageRepository::get_page(tx, &request.page_id)?.ok_or_else(|| {
                preferred_scan_error(
                    "STORAGE_PAGE_NOT_FOUND",
                    ErrorCategory::Validation,
                    "change_preferred_scan: no page with this id",
                )
                .with_detail("page_id", request.page_id.to_string())
            })?;
            let scan = ScanRepository::get_scan(tx, &request.scan_id)?.ok_or_else(|| {
                preferred_scan_error(
                    "STORAGE_SCAN_NOT_FOUND",
                    ErrorCategory::Validation,
                    "change_preferred_scan: no scan with this id",
                )
                .with_detail("scan_id", request.scan_id.to_string())
            })?;
            if scan.page_id != request.page_id {
                return Err(preferred_scan_error(
                    "STORAGE_PREFERRED_SCAN_PAGE_MISMATCH",
                    ErrorCategory::Validation,
                    "preferred scan must belong to the requested page",
                )
                .with_detail("page_id", request.page_id.to_string())
                .with_detail("scan_id", request.scan_id.to_string())
                .with_detail("scan_page_id", scan.page_id.to_string()));
            }

            validate_current_state(tx, &page)?;
            validate_scan_assets(tx, &scan)?;

            let previous_preferred_scan_id = page.preferred_scan_id.clone();
            if previous_preferred_scan_id.as_ref() == Some(&request.scan_id) {
                return Ok(ChangePreferredScanResult {
                    page_id: request.page_id.clone(),
                    previous_preferred_scan_id,
                    preferred_scan_id: request.scan_id.clone(),
                    changed: false,
                    audit_event_id: None,
                });
            }

            enter_workflow_context(tx, &request)?;
            let changed = tx
                .execute(
                    "UPDATE pages SET preferred_scan_id = ?1, updated_at_ms = ?2 WHERE id = ?3",
                    params![
                        request.scan_id.to_string(),
                        request.changed_at_ms,
                        request.page_id.to_string()
                    ],
                )
                .map_err(map_preferred_update_error)?;
            if changed != 1 {
                return Err(A2dError::internal_unknown(
                    "preferred-scan page disappeared during its transaction",
                )
                .with_detail("page_id", request.page_id.to_string()));
            }

            let updated_page = PageRepository::get_page(tx, &request.page_id)?.ok_or_else(|| {
                A2dError::internal_unknown(
                    "preferred-scan page disappeared after its update",
                )
                .with_detail("page_id", request.page_id.to_string())
            })?;
            let updated_scan = ScanRepository::get_scan(tx, &request.scan_id)?.ok_or_else(|| {
                A2dError::internal_unknown(
                    "preferred-scan candidate disappeared after its update",
                )
                .with_detail("scan_id", request.scan_id.to_string())
            })?;
            if updated_page.preferred_scan_id.as_ref() != Some(&request.scan_id)
                || !updated_scan.preferred
                || preferred_count(tx, &request.page_id)? != 1
            {
                return Err(preferred_scan_error(
                    "STORAGE_PREFERRED_SCAN_POSTCONDITION_FAILED",
                    ErrorCategory::Integrity,
                    "preferred-scan transaction did not establish exactly one consistent preference",
                )
                .with_detail("page_id", request.page_id.to_string())
                .with_detail("scan_id", request.scan_id.to_string()));
            }

            let mut details = BTreeMap::new();
            details.insert("page_id".to_string(), request.page_id.to_string());
            details.insert(
                "previous_preferred_scan_id".to_string(),
                previous_preferred_scan_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "none".to_string()),
            );
            details.insert(
                "preferred_scan_id".to_string(),
                request.scan_id.to_string(),
            );
            let event = AuditEvent::new(
                request.operation_id.clone(),
                request.changed_at_ms,
                "scan.preferred_changed".to_string(),
                actor.clone(),
                Some(request.page_id.to_string()),
                details,
                Some(request.operation_id.to_string()),
            );
            AuditEventRepository::insert_audit_event(tx, &event)?;
            leave_workflow_context(tx, &request)?;

            Ok(ChangePreferredScanResult {
                page_id: request.page_id.clone(),
                previous_preferred_scan_id,
                preferred_scan_id: request.scan_id.clone(),
                changed: true,
                audit_event_id: Some(request.operation_id.clone()),
            })
        })
    }
}
