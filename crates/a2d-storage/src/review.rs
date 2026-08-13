//! Atomic Needs Review state transitions for Milestone 9.4.
//!
//! Resolving or deferring a review item changes only the review row and inserts an audit event in
//! the same transaction. No page, scan, or asset row is deleted or mutated by these generic queue
//! transitions; kind-specific business workflows may do their own explicit mutation before they
//! resolve a review item.

use std::collections::BTreeMap;

use a2d_domain::{
    A2dError, AuditEvent, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity, ReviewItem,
    ReviewItemId, ReviewItemStatus,
};
use rusqlite::params;

use crate::repository::{AuditEventRepository, map_sql_error};
use crate::{ReviewItemRepository, Storage};

pub const MAX_REVIEW_ACTOR_BYTES: usize = 128;
pub const MAX_REVIEW_RESOLUTION_CODE_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveReviewItemRequest {
    pub review_item_id: ReviewItemId,
    pub resolution_code: String,
    pub resolved_at_ms: i64,
    pub actor: String,
    pub operation_id: AuditEventId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeferReviewItemRequest {
    pub review_item_id: ReviewItemId,
    pub deferred_at_ms: i64,
    pub actor: String,
    pub operation_id: AuditEventId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewItemMutationResult {
    pub item: ReviewItem,
    pub changed: bool,
    pub audit_event_id: Option<AuditEventId>,
    pub committed_data_deleted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReviewStatusTransition {
    previous: ReviewItemStatus,
    next: ReviewItemStatus,
}

impl Storage {
    pub fn resolve_review_item(
        &mut self,
        request: ResolveReviewItemRequest,
    ) -> Result<ReviewItemMutationResult, A2dError> {
        validate_actor(&request.actor)?;
        validate_resolution_code(&request.resolution_code)?;
        validate_positive_time("resolved_at_ms", request.resolved_at_ms)?;

        self.transaction(|tx| {
            let current = ReviewItemRepository::get_review_item(tx, &request.review_item_id)?
                .ok_or_else(|| review_not_found(&request.review_item_id))?;
            if request.resolved_at_ms < current.created_at_ms {
                return Err(review_validation_error(
                    "STORAGE_REVIEW_RESOLUTION_BEFORE_CREATION",
                    "review item cannot resolve before it was created",
                ));
            }
            match current.status {
                ReviewItemStatus::Resolved => {
                    if current.resolution.as_deref() == Some(request.resolution_code.as_str()) {
                        return Ok(no_change(current));
                    }
                    return Err(review_conflict_error(
                        "STORAGE_REVIEW_ALREADY_RESOLVED_DIFFERENTLY",
                        "review item was already resolved with a different resolution code",
                        &request.review_item_id,
                    ));
                }
                ReviewItemStatus::Dismissed => {
                    return Err(review_conflict_error(
                        "STORAGE_REVIEW_ALREADY_DISMISSED",
                        "dismissed review item cannot be resolved again",
                        &request.review_item_id,
                    ));
                }
                ReviewItemStatus::Open | ReviewItemStatus::Deferred => {}
            }

            let previous_status = current.status;
            let changed = tx
                .execute(
                    "UPDATE review_items SET status = 'Resolved', resolution = ?1, \
                     resolved_at_ms = ?2 WHERE id = ?3 AND status IN ('Open', 'Deferred')",
                    params![
                        request.resolution_code,
                        request.resolved_at_ms,
                        request.review_item_id.to_string(),
                    ],
                )
                .map_err(|error| map_sql_error("resolve_review_item", error))?;
            if changed != 1 {
                return Err(review_integrity_error(
                    "STORAGE_REVIEW_RESOLVE_RACE",
                    "review item changed unexpectedly during atomic resolution",
                )
                .with_detail("review_item_id", request.review_item_id.to_string()));
            }

            let event = review_audit_event(
                request.operation_id.clone(),
                request.resolved_at_ms,
                "review_item.resolved",
                request.actor.clone(),
                &current,
                ReviewStatusTransition {
                    previous: previous_status,
                    next: ReviewItemStatus::Resolved,
                },
                Some(request.resolution_code.clone()),
            );
            AuditEventRepository::insert_audit_event(tx, &event)?;
            let item = ReviewItemRepository::get_review_item(tx, &request.review_item_id)?
                .ok_or_else(|| {
                    review_integrity_error(
                        "STORAGE_REVIEW_DISAPPEARED_AFTER_RESOLVE",
                        "review item disappeared after successful resolution update",
                    )
                })?;
            Ok(ReviewItemMutationResult {
                item,
                changed: true,
                audit_event_id: Some(request.operation_id.clone()),
                committed_data_deleted: false,
            })
        })
    }

    pub fn defer_review_item(
        &mut self,
        request: DeferReviewItemRequest,
    ) -> Result<ReviewItemMutationResult, A2dError> {
        validate_actor(&request.actor)?;
        validate_positive_time("deferred_at_ms", request.deferred_at_ms)?;

        self.transaction(|tx| {
            let current = ReviewItemRepository::get_review_item(tx, &request.review_item_id)?
                .ok_or_else(|| review_not_found(&request.review_item_id))?;
            if request.deferred_at_ms < current.created_at_ms {
                return Err(review_validation_error(
                    "STORAGE_REVIEW_DEFER_BEFORE_CREATION",
                    "review item cannot be deferred before it was created",
                ));
            }
            match current.status {
                ReviewItemStatus::Deferred => return Ok(no_change(current)),
                ReviewItemStatus::Resolved | ReviewItemStatus::Dismissed => {
                    return Err(review_conflict_error(
                        "STORAGE_REVIEW_TERMINAL_CANNOT_DEFER",
                        "terminal review item cannot be deferred",
                        &request.review_item_id,
                    ));
                }
                ReviewItemStatus::Open => {}
            }

            let changed = tx
                .execute(
                    "UPDATE review_items SET status = 'Deferred' WHERE id = ?1 AND status = 'Open'",
                    [request.review_item_id.to_string()],
                )
                .map_err(|error| map_sql_error("defer_review_item", error))?;
            if changed != 1 {
                return Err(review_integrity_error(
                    "STORAGE_REVIEW_DEFER_RACE",
                    "review item changed unexpectedly during atomic deferral",
                )
                .with_detail("review_item_id", request.review_item_id.to_string()));
            }

            let event = review_audit_event(
                request.operation_id.clone(),
                request.deferred_at_ms,
                "review_item.deferred",
                request.actor.clone(),
                &current,
                ReviewStatusTransition {
                    previous: ReviewItemStatus::Open,
                    next: ReviewItemStatus::Deferred,
                },
                None,
            );
            AuditEventRepository::insert_audit_event(tx, &event)?;
            let item = ReviewItemRepository::get_review_item(tx, &request.review_item_id)?
                .ok_or_else(|| {
                    review_integrity_error(
                        "STORAGE_REVIEW_DISAPPEARED_AFTER_DEFER",
                        "review item disappeared after successful deferral update",
                    )
                })?;
            Ok(ReviewItemMutationResult {
                item,
                changed: true,
                audit_event_id: Some(request.operation_id.clone()),
                committed_data_deleted: false,
            })
        })
    }
}

fn no_change(item: ReviewItem) -> ReviewItemMutationResult {
    ReviewItemMutationResult {
        item,
        changed: false,
        audit_event_id: None,
        committed_data_deleted: false,
    }
}

fn review_audit_event(
    id: AuditEventId,
    occurred_at_ms: i64,
    event_kind: &str,
    actor: String,
    item: &ReviewItem,
    transition: ReviewStatusTransition,
    resolution_code: Option<String>,
) -> AuditEvent {
    let mut details = BTreeMap::new();
    details.insert("review_item_id".to_string(), item.id().to_string());
    details.insert("kind".to_string(), std::format!("{:?}", item.kind));
    details.insert(
        "previous_status".to_string(),
        std::format!("{:?}", transition.previous),
    );
    details.insert(
        "new_status".to_string(),
        std::format!("{:?}", transition.next),
    );
    details.insert("committed_data_deleted".to_string(), "false".to_string());
    if let Some(page_id) = item.page_id.as_ref() {
        details.insert("page_id".to_string(), page_id.to_string());
    }
    if let Some(scan_id) = item.scan_id.as_ref() {
        details.insert("scan_id".to_string(), scan_id.to_string());
    }
    if let Some(resolution_code) = resolution_code {
        details.insert("resolution_code".to_string(), resolution_code);
    }
    AuditEvent::new(
        id.clone(),
        occurred_at_ms,
        event_kind.to_string(),
        actor,
        Some(item.id().to_string()),
        details,
        Some(id.to_string()),
    )
}

pub fn validate_resolution_code(value: &str) -> Result<(), A2dError> {
    if value.is_empty()
        || value.len() > MAX_REVIEW_RESOLUTION_CODE_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
        })
    {
        return Err(review_validation_error(
            "STORAGE_REVIEW_RESOLUTION_CODE_INVALID",
            "resolution_code must be a bounded stable ASCII code using A-Z, 0-9, underscore, dot, or hyphen",
        ));
    }
    Ok(())
}

fn validate_actor(actor: &str) -> Result<(), A2dError> {
    if actor.is_empty()
        || actor.len() > MAX_REVIEW_ACTOR_BYTES
        || actor.trim() != actor
        || actor.chars().any(char::is_control)
    {
        return Err(review_validation_error(
            "STORAGE_REVIEW_ACTOR_INVALID",
            "review actor must be nonempty, bounded, trimmed, and contain no control characters",
        ));
    }
    Ok(())
}

fn validate_positive_time(field: &str, value: i64) -> Result<(), A2dError> {
    if value <= 0 {
        return Err(review_validation_error(
            "STORAGE_REVIEW_TIME_INVALID",
            "review transition timestamp must be positive",
        )
        .with_detail("field", field));
    }
    Ok(())
}

fn review_not_found(id: &ReviewItemId) -> A2dError {
    review_validation_error(
        "STORAGE_REVIEW_ITEM_NOT_FOUND",
        "requested review item does not exist",
    )
    .with_detail("review_item_id", id.to_string())
}

fn review_conflict_error(code: &'static str, message: &'static str, id: &ReviewItemId) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.review_transition_conflict",
        message,
        false,
    )
    .with_detail("review_item_id", id.to_string())
}

fn review_validation_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.review_transition",
        message,
        false,
    )
}

fn review_integrity_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.review_transition_integrity",
        message,
        false,
    )
}

#[path = "version_history.rs"]
mod version_history;
pub use version_history::*;
