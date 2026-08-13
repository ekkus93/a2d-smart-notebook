//! Durable Needs Review repository for Milestone 9.4.
//!
//! Review items are canonical Rust-owned state. SQL stays private to storage, list filters are
//! fixed/parameterized, and stored enum corruption fails closed rather than becoming an unknown
//! or default UI value.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, ReviewItem, ReviewItemId,
    ReviewItemKind, ReviewItemStatus, ScanId,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::json_columns::{decode_json, encode_json};
use crate::repository::map_sql_error;

pub const MAX_REVIEW_LIST_LIMIT: u32 = 101;
pub const MAX_REVIEW_LIST_OFFSET: u32 = 1_000_000;
const MAX_REVIEW_DETAILS: usize = 32;
const MAX_REVIEW_DETAIL_KEY_BYTES: usize = 64;
const MAX_REVIEW_DETAIL_VALUE_BYTES: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewItemQuery {
    pub kind: Option<ReviewItemKind>,
    pub status: Option<ReviewItemStatus>,
    pub page_id: Option<PageId>,
    pub scan_id: Option<ScanId>,
    pub limit: u32,
    pub offset: u32,
}

pub trait ReviewItemRepository {
    fn insert_review_item(&self, item: &ReviewItem) -> Result<(), A2dError>;
    fn get_review_item(&self, id: &ReviewItemId) -> Result<Option<ReviewItem>, A2dError>;
    fn list_review_items(&self, query: &ReviewItemQuery) -> Result<Vec<ReviewItem>, A2dError>;
}

impl ReviewItemRepository for Connection {
    fn insert_review_item(&self, item: &ReviewItem) -> Result<(), A2dError> {
        validate_review_item(item)?;
        validate_page_scan_relation(self, item.page_id.as_ref(), item.scan_id.as_ref())?;
        let details = encode_json(&item.details, "review_items.details")?;
        self.execute(
            "INSERT INTO review_items (id, kind, page_id, scan_id, severity, status, details, \
             resolution, created_at_ms, resolved_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item.id().to_string(),
                review_kind_to_str(item.kind),
                item.page_id.as_ref().map(ToString::to_string),
                item.scan_id.as_ref().map(ToString::to_string),
                severity_to_str(item.severity),
                review_status_to_str(item.status),
                details,
                item.resolution,
                item.created_at_ms,
                item.resolved_at_ms,
            ],
        )
        .map_err(|error| map_sql_error("insert_review_item", error))?;
        Ok(())
    }

    fn get_review_item(&self, id: &ReviewItemId) -> Result<Option<ReviewItem>, A2dError> {
        self.query_row(
            "SELECT id, kind, page_id, scan_id, severity, status, details, resolution, \
             created_at_ms, resolved_at_ms FROM review_items WHERE id = ?1",
            [id.to_string()],
            review_item_row,
        )
        .optional()
        .map_err(|error| map_sql_error("get_review_item", error))?
        .map(decode_review_item_row)
        .transpose()
    }

    fn list_review_items(&self, query: &ReviewItemQuery) -> Result<Vec<ReviewItem>, A2dError> {
        validate_query(query)?;
        let kind = query.kind.map(review_kind_to_str);
        let status = query.status.map(review_status_to_str);
        let page_id = query.page_id.as_ref().map(ToString::to_string);
        let scan_id = query.scan_id.as_ref().map(ToString::to_string);
        let mut statement = self
            .prepare(
                "SELECT id, kind, page_id, scan_id, severity, status, details, resolution, \
                 created_at_ms, resolved_at_ms FROM review_items \
                 WHERE (?1 IS NULL OR kind = ?1) \
                   AND (?2 IS NULL OR status = ?2) \
                   AND (?3 IS NULL OR page_id = ?3) \
                   AND (?4 IS NULL OR scan_id = ?4) \
                 ORDER BY created_at_ms DESC, id DESC LIMIT ?5 OFFSET ?6",
            )
            .map_err(|error| map_sql_error("prepare_list_review_items", error))?;
        let rows = statement
            .query_map(
                params![kind, status, page_id, scan_id, query.limit, query.offset],
                review_item_row,
            )
            .map_err(|error| map_sql_error("list_review_items", error))?;
        let mut items = Vec::with_capacity(query.limit as usize);
        for row in rows {
            let raw = row.map_err(|error| map_sql_error("list_review_items_row", error))?;
            items.push(decode_review_item_row(raw)?);
        }
        Ok(items)
    }
}

type RawReviewItemRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    i64,
    Option<i64>,
);

fn review_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawReviewItemRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

fn decode_review_item_row(raw: RawReviewItemRow) -> Result<ReviewItem, A2dError> {
    let (id, kind, page_id, scan_id, severity, status, details, resolution, created_at_ms, resolved_at_ms) = raw;
    let item = ReviewItem::new(
        ReviewItemId::parse(&id)?,
        review_kind_from_str(&kind)?,
        page_id.map(|value| PageId::parse(&value)).transpose()?,
        scan_id.map(|value| ScanId::parse(&value)).transpose()?,
        severity_from_str(&severity)?,
        review_status_from_str(&status)?,
        decode_json(&details, "review_items.details")?,
        resolution,
        created_at_ms,
        resolved_at_ms,
    );
    validate_review_item(&item)?;
    Ok(item)
}

fn validate_query(query: &ReviewItemQuery) -> Result<(), A2dError> {
    if query.limit == 0 || query.limit > MAX_REVIEW_LIST_LIMIT {
        return Err(review_validation_error(
            "STORAGE_REVIEW_LIST_LIMIT_INVALID",
            "review-item list limit is outside the supported range",
        )
        .with_detail("limit", query.limit.to_string()));
    }
    if query.offset > MAX_REVIEW_LIST_OFFSET {
        return Err(review_validation_error(
            "STORAGE_REVIEW_LIST_OFFSET_INVALID",
            "review-item list offset is outside the supported range",
        )
        .with_detail("offset", query.offset.to_string()));
    }
    Ok(())
}

fn validate_review_item(item: &ReviewItem) -> Result<(), A2dError> {
    if item.created_at_ms <= 0 {
        return Err(review_validation_error(
            "STORAGE_REVIEW_CREATED_TIME_INVALID",
            "review-item created_at_ms must be positive",
        ));
    }
    if item.details.len() > MAX_REVIEW_DETAILS {
        return Err(review_validation_error(
            "STORAGE_REVIEW_DETAILS_TOO_MANY",
            "review item has too many detail entries",
        ));
    }
    for (key, value) in &item.details {
        if key.is_empty() || key.len() > MAX_REVIEW_DETAIL_KEY_BYTES {
            return Err(review_validation_error(
                "STORAGE_REVIEW_DETAIL_KEY_INVALID",
                "review-item detail key is empty or too long",
            ));
        }
        if value.len() > MAX_REVIEW_DETAIL_VALUE_BYTES {
            return Err(review_validation_error(
                "STORAGE_REVIEW_DETAIL_VALUE_TOO_LONG",
                "review-item detail value is too long",
            )
            .with_detail("key", key.clone()));
        }
    }
    match item.status {
        ReviewItemStatus::Open | ReviewItemStatus::Deferred => {
            if item.resolution.is_some() || item.resolved_at_ms.is_some() {
                return Err(review_integrity_error(
                    "STORAGE_REVIEW_NONTERMINAL_HAS_RESOLUTION",
                    "nonterminal review item cannot carry terminal resolution fields",
                ));
            }
        }
        ReviewItemStatus::Resolved | ReviewItemStatus::Dismissed => {
            if item.resolution.as_deref().is_none_or(str::is_empty) || item.resolved_at_ms.is_none() {
                return Err(review_integrity_error(
                    "STORAGE_REVIEW_TERMINAL_MISSING_RESOLUTION",
                    "terminal review item must carry resolution and resolved timestamp",
                ));
            }
            if item.resolved_at_ms.is_some_and(|time| time < item.created_at_ms) {
                return Err(review_integrity_error(
                    "STORAGE_REVIEW_RESOLVED_BEFORE_CREATED",
                    "review item cannot resolve before it was created",
                ));
            }
        }
    }
    Ok(())
}

fn validate_page_scan_relation(
    connection: &Connection,
    page_id: Option<&PageId>,
    scan_id: Option<&ScanId>,
) -> Result<(), A2dError> {
    let (Some(page_id), Some(scan_id)) = (page_id, scan_id) else {
        return Ok(());
    };
    let scan_page_id = connection
        .query_row(
            "SELECT page_id FROM scans WHERE id = ?1",
            [scan_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| map_sql_error("validate_review_item_scan_page", error))?;
    if let Some(scan_page_id) = scan_page_id
        && scan_page_id != page_id.to_string()
    {
        return Err(review_validation_error(
            "STORAGE_REVIEW_PAGE_SCAN_MISMATCH",
            "review-item page and scan references do not identify the same page",
        )
        .with_detail("page_id", page_id.to_string())
        .with_detail("scan_id", scan_id.to_string())
        .with_detail("scan_page_id", scan_page_id));
    }
    Ok(())
}

pub(crate) const fn review_kind_to_str(kind: ReviewItemKind) -> &'static str {
    match kind {
        ReviewItemKind::UnidentifiedPage => "UnidentifiedPage",
        ReviewItemKind::NotebookSelection => "NotebookSelection",
        ReviewItemKind::WrongNotebook => "WrongNotebook",
        ReviewItemKind::LowQuality => "LowQuality",
        ReviewItemKind::ManualAlignment => "ManualAlignment",
        ReviewItemKind::Duplicate => "Duplicate",
        ReviewItemKind::Revision => "Revision",
        ReviewItemKind::PhysicalCopy => "PhysicalCopy",
        ReviewItemKind::OcrFailure => "OcrFailure",
        ReviewItemKind::ProcessingFailure => "ProcessingFailure",
        ReviewItemKind::ImportConflict => "ImportConflict",
        ReviewItemKind::RestoreConflict => "RestoreConflict",
    }
}

fn review_kind_from_str(raw: &str) -> Result<ReviewItemKind, A2dError> {
    match raw {
        "UnidentifiedPage" => Ok(ReviewItemKind::UnidentifiedPage),
        "NotebookSelection" => Ok(ReviewItemKind::NotebookSelection),
        "WrongNotebook" => Ok(ReviewItemKind::WrongNotebook),
        "LowQuality" => Ok(ReviewItemKind::LowQuality),
        "ManualAlignment" => Ok(ReviewItemKind::ManualAlignment),
        "Duplicate" => Ok(ReviewItemKind::Duplicate),
        "Revision" => Ok(ReviewItemKind::Revision),
        "PhysicalCopy" => Ok(ReviewItemKind::PhysicalCopy),
        "OcrFailure" => Ok(ReviewItemKind::OcrFailure),
        "ProcessingFailure" => Ok(ReviewItemKind::ProcessingFailure),
        "ImportConflict" => Ok(ReviewItemKind::ImportConflict),
        "RestoreConflict" => Ok(ReviewItemKind::RestoreConflict),
        other => Err(review_corrupt_enum_error("review_items.kind", other)),
    }
}

pub(crate) const fn review_status_to_str(status: ReviewItemStatus) -> &'static str {
    match status {
        ReviewItemStatus::Open => "Open",
        ReviewItemStatus::Deferred => "Deferred",
        ReviewItemStatus::Resolved => "Resolved",
        ReviewItemStatus::Dismissed => "Dismissed",
    }
}

fn review_status_from_str(raw: &str) -> Result<ReviewItemStatus, A2dError> {
    match raw {
        "Open" => Ok(ReviewItemStatus::Open),
        "Deferred" => Ok(ReviewItemStatus::Deferred),
        "Resolved" => Ok(ReviewItemStatus::Resolved),
        "Dismissed" => Ok(ReviewItemStatus::Dismissed),
        other => Err(review_corrupt_enum_error("review_items.status", other)),
    }
}

const fn severity_to_str(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Info => "Info",
        ErrorSeverity::Warning => "Warning",
        ErrorSeverity::Error => "Error",
        ErrorSeverity::Critical => "Critical",
    }
}

fn severity_from_str(raw: &str) -> Result<ErrorSeverity, A2dError> {
    match raw {
        "Info" => Ok(ErrorSeverity::Info),
        "Warning" => Ok(ErrorSeverity::Warning),
        "Error" => Ok(ErrorSeverity::Error),
        "Critical" => Ok(ErrorSeverity::Critical),
        other => Err(review_corrupt_enum_error("review_items.severity", other)),
    }
}

fn review_validation_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.review_item",
        message,
        false,
    )
}

fn review_integrity_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.review_item_integrity",
        message,
        false,
    )
}

fn review_corrupt_enum_error(column: &str, raw: &str) -> A2dError {
    review_integrity_error(
        "STORAGE_REVIEW_CORRUPT_ENUM",
        "review item contains an unknown persisted enum value",
    )
    .with_detail("column", column)
    .with_detail("raw", raw)
}
