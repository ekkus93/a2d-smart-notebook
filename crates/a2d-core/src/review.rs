//! Rust-owned Needs Review use cases for Milestone 9.4.

use std::collections::BTreeMap;

use a2d_domain::{
    A2dError, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity, PageId, ReviewItem,
    ReviewItemId, ReviewItemKind, ReviewItemStatus, ScanId,
};
use a2d_storage::{
    DeferReviewItemRequest as StorageDeferReviewItemRequest, MAX_REVIEW_LIST_OFFSET,
    ResolveReviewItemRequest as StorageResolveReviewItemRequest,
    ReviewItemMutationResult as StorageReviewItemMutationResult, ReviewItemQuery,
    ReviewItemRepository,
};

use crate::A2dCore;

pub const MAX_REVIEW_PAGE_SIZE: u32 = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateReviewItemRequest {
    pub kind: ReviewItemKind,
    pub page_id: Option<PageId>,
    pub scan_id: Option<ScanId>,
    pub severity: ErrorSeverity,
    pub details: BTreeMap<String, String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListReviewItemsRequest {
    pub kind: Option<ReviewItemKind>,
    pub status: Option<ReviewItemStatus>,
    pub page_id: Option<PageId>,
    pub scan_id: Option<ScanId>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewItemPage {
    pub items: Vec<ReviewItem>,
    pub has_more: bool,
    pub next_offset: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveReviewItemRequest {
    pub review_item_id: ReviewItemId,
    pub resolution_code: String,
    pub resolved_at_ms: i64,
    pub actor: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeferReviewItemRequest {
    pub review_item_id: ReviewItemId,
    pub deferred_at_ms: i64,
    pub actor: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewItemMutationResult {
    pub item: ReviewItem,
    pub changed: bool,
    pub audit_event_id: Option<AuditEventId>,
    pub committed_data_deleted: bool,
}

impl From<StorageReviewItemMutationResult> for ReviewItemMutationResult {
    fn from(value: StorageReviewItemMutationResult) -> Self {
        Self {
            item: value.item,
            changed: value.changed,
            audit_event_id: value.audit_event_id,
            committed_data_deleted: value.committed_data_deleted,
        }
    }
}

impl A2dCore {
    /// Creates canonical review state for Rust-owned producers. This deliberately is not exported
    /// through UniFFI: Android may consume/resolve review items but may not invent canonical queue
    /// entries or their classification.
    pub fn create_review_item(
        &self,
        request: CreateReviewItemRequest,
    ) -> Result<ReviewItem, A2dError> {
        if request.created_at_ms <= 0 {
            return Err(review_error(
                "CORE_REVIEW_CREATED_TIME_INVALID",
                "review-item created_at_ms must be positive",
            ));
        }
        let item = ReviewItem::new(
            ReviewItemId::try_generate()?,
            request.kind,
            request.page_id,
            request.scan_id,
            request.severity,
            ReviewItemStatus::Open,
            request.details,
            None,
            request.created_at_ms,
            None,
        );
        self.lock_storage()?.insert_review_item(&item)?;
        Ok(item)
    }

    pub fn list_review_items(
        &self,
        request: ListReviewItemsRequest,
    ) -> Result<ReviewItemPage, A2dError> {
        validate_page_request(&request)?;
        let fetch_limit = request.limit.checked_add(1).ok_or_else(|| {
            review_error(
                "CORE_REVIEW_LIST_LIMIT_OVERFLOW",
                "review-item page size overflowed",
            )
        })?;
        let mut items = self.lock_storage()?.list_review_items(&ReviewItemQuery {
            kind: request.kind,
            status: request.status,
            page_id: request.page_id,
            scan_id: request.scan_id,
            limit: fetch_limit,
            offset: request.offset,
        })?;
        let has_more = items.len() > request.limit as usize;
        if has_more {
            items.truncate(request.limit as usize);
        }
        let next_offset = if has_more {
            Some(request.offset.checked_add(request.limit).ok_or_else(|| {
                review_error(
                    "CORE_REVIEW_LIST_OFFSET_OVERFLOW",
                    "review-item next offset overflowed",
                )
            })?)
        } else {
            None
        };
        Ok(ReviewItemPage {
            items,
            has_more,
            next_offset,
        })
    }

    pub fn get_review_item(&self, id: &ReviewItemId) -> Result<ReviewItem, A2dError> {
        self.lock_storage()?.get_review_item(id)?.ok_or_else(|| {
            review_error(
                "CORE_REVIEW_ITEM_NOT_FOUND",
                "requested review item does not exist",
            )
            .with_detail("review_item_id", id.to_string())
        })
    }

    pub fn resolve_review_item(
        &self,
        request: ResolveReviewItemRequest,
    ) -> Result<ReviewItemMutationResult, A2dError> {
        let operation_id = AuditEventId::try_generate()?;
        self.lock_storage()?
            .resolve_review_item(StorageResolveReviewItemRequest {
                review_item_id: request.review_item_id,
                resolution_code: request.resolution_code,
                resolved_at_ms: request.resolved_at_ms,
                actor: request.actor,
                operation_id,
            })
            .map(Into::into)
    }

    pub fn defer_review_item(
        &self,
        request: DeferReviewItemRequest,
    ) -> Result<ReviewItemMutationResult, A2dError> {
        let operation_id = AuditEventId::try_generate()?;
        self.lock_storage()?
            .defer_review_item(StorageDeferReviewItemRequest {
                review_item_id: request.review_item_id,
                deferred_at_ms: request.deferred_at_ms,
                actor: request.actor,
                operation_id,
            })
            .map(Into::into)
    }
}

fn validate_page_request(request: &ListReviewItemsRequest) -> Result<(), A2dError> {
    if request.limit == 0 || request.limit > MAX_REVIEW_PAGE_SIZE {
        return Err(review_error(
            "CORE_REVIEW_LIST_LIMIT_INVALID",
            "review-item page size must be between 1 and 100",
        )
        .with_detail("limit", request.limit.to_string()));
    }
    if request.offset > MAX_REVIEW_LIST_OFFSET
        || request
            .offset
            .checked_add(request.limit)
            .is_none_or(|end| end > MAX_REVIEW_LIST_OFFSET)
    {
        return Err(review_error(
            "CORE_REVIEW_LIST_OFFSET_INVALID",
            "review-item page window exceeds the supported offset bound",
        )
        .with_detail("offset", request.offset.to_string())
        .with_detail("limit", request.limit.to_string()));
    }
    Ok(())
}

fn review_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.core.review_item",
        message,
        false,
    )
}
