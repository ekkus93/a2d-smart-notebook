//! Portable UniFFI projection for the Rust-owned Needs Review queue (Milestone 9.4).

use a2d_core as core;
use a2d_domain::{
    PageId, ReviewItem as DomainReviewItem, ReviewItemKind as DomainReviewItemKind,
    ReviewItemStatus as DomainReviewItemStatus, ScanId,
};

use crate::{A2dClient, A2dFfiError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ReviewItemKind {
    UnidentifiedPage,
    NotebookSelection,
    WrongNotebook,
    LowQuality,
    ManualAlignment,
    Duplicate,
    Revision,
    PhysicalCopy,
    OcrFailure,
    ProcessingFailure,
    ImportConflict,
    RestoreConflict,
}

impl From<DomainReviewItemKind> for ReviewItemKind {
    fn from(value: DomainReviewItemKind) -> Self {
        match value {
            DomainReviewItemKind::UnidentifiedPage => Self::UnidentifiedPage,
            DomainReviewItemKind::NotebookSelection => Self::NotebookSelection,
            DomainReviewItemKind::WrongNotebook => Self::WrongNotebook,
            DomainReviewItemKind::LowQuality => Self::LowQuality,
            DomainReviewItemKind::ManualAlignment => Self::ManualAlignment,
            DomainReviewItemKind::Duplicate => Self::Duplicate,
            DomainReviewItemKind::Revision => Self::Revision,
            DomainReviewItemKind::PhysicalCopy => Self::PhysicalCopy,
            DomainReviewItemKind::OcrFailure => Self::OcrFailure,
            DomainReviewItemKind::ProcessingFailure => Self::ProcessingFailure,
            DomainReviewItemKind::ImportConflict => Self::ImportConflict,
            DomainReviewItemKind::RestoreConflict => Self::RestoreConflict,
        }
    }
}

impl From<ReviewItemKind> for DomainReviewItemKind {
    fn from(value: ReviewItemKind) -> Self {
        match value {
            ReviewItemKind::UnidentifiedPage => Self::UnidentifiedPage,
            ReviewItemKind::NotebookSelection => Self::NotebookSelection,
            ReviewItemKind::WrongNotebook => Self::WrongNotebook,
            ReviewItemKind::LowQuality => Self::LowQuality,
            ReviewItemKind::ManualAlignment => Self::ManualAlignment,
            ReviewItemKind::Duplicate => Self::Duplicate,
            ReviewItemKind::Revision => Self::Revision,
            ReviewItemKind::PhysicalCopy => Self::PhysicalCopy,
            ReviewItemKind::OcrFailure => Self::OcrFailure,
            ReviewItemKind::ProcessingFailure => Self::ProcessingFailure,
            ReviewItemKind::ImportConflict => Self::ImportConflict,
            ReviewItemKind::RestoreConflict => Self::RestoreConflict,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ReviewItemStatus {
    Open,
    Deferred,
    Resolved,
    Dismissed,
}

impl From<DomainReviewItemStatus> for ReviewItemStatus {
    fn from(value: DomainReviewItemStatus) -> Self {
        match value {
            DomainReviewItemStatus::Open => Self::Open,
            DomainReviewItemStatus::Deferred => Self::Deferred,
            DomainReviewItemStatus::Resolved => Self::Resolved,
            DomainReviewItemStatus::Dismissed => Self::Dismissed,
        }
    }
}

impl From<ReviewItemStatus> for DomainReviewItemStatus {
    fn from(value: ReviewItemStatus) -> Self {
        match value {
            ReviewItemStatus::Open => Self::Open,
            ReviewItemStatus::Deferred => Self::Deferred,
            ReviewItemStatus::Resolved => Self::Resolved,
            ReviewItemStatus::Dismissed => Self::Dismissed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ReviewItemDetail {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewItemRecord {
    pub id: String,
    pub kind: ReviewItemKind,
    pub page_id: Option<String>,
    pub scan_id: Option<String>,
    pub severity: String,
    pub status: ReviewItemStatus,
    pub details: Vec<ReviewItemDetail>,
    pub resolution_code: Option<String>,
    pub created_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

impl From<DomainReviewItem> for ReviewItemRecord {
    fn from(value: DomainReviewItem) -> Self {
        Self {
            id: value.id().to_string(),
            kind: value.kind.into(),
            page_id: value.page_id.map(|id| id.to_string()),
            scan_id: value.scan_id.map(|id| id.to_string()),
            severity: format!("{:?}", value.severity),
            status: value.status.into(),
            details: value
                .details
                .into_iter()
                .map(|(key, value)| ReviewItemDetail { key, value })
                .collect(),
            resolution_code: value.resolution,
            created_at_ms: value.created_at_ms,
            resolved_at_ms: value.resolved_at_ms,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ListReviewItemsRequest {
    pub kind: Option<ReviewItemKind>,
    pub status: Option<ReviewItemStatus>,
    pub page_id: Option<String>,
    pub scan_id: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewItemPage {
    pub items: Vec<ReviewItemRecord>,
    pub has_more: bool,
    pub next_offset: Option<u32>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct GetReviewItemRequest {
    pub review_item_id: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ResolveReviewItemRequest {
    pub review_item_id: String,
    pub resolution_code: String,
    pub resolved_at_ms: i64,
    pub actor: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct DeferReviewItemRequest {
    pub review_item_id: String,
    pub deferred_at_ms: i64,
    pub actor: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewItemMutationResult {
    pub item: ReviewItemRecord,
    pub changed: bool,
    pub audit_event_id: Option<String>,
    pub committed_data_deleted: bool,
}

impl From<core::ReviewItemMutationResult> for ReviewItemMutationResult {
    fn from(value: core::ReviewItemMutationResult) -> Self {
        Self {
            item: value.item.into(),
            changed: value.changed,
            audit_event_id: value.audit_event_id.map(|id| id.to_string()),
            committed_data_deleted: value.committed_data_deleted,
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn list_review_items(
        &self,
        request: ListReviewItemsRequest,
    ) -> Result<ReviewItemPage, A2dFfiError> {
        let page = self.core.list_review_items(core::ListReviewItemsRequest {
            kind: request.kind.map(Into::into),
            status: request.status.map(Into::into),
            page_id: request.page_id.map(|id| PageId::parse(&id)).transpose()?,
            scan_id: request.scan_id.map(|id| ScanId::parse(&id)).transpose()?,
            limit: request.limit,
            offset: request.offset,
        })?;
        Ok(ReviewItemPage {
            items: page.items.into_iter().map(Into::into).collect(),
            has_more: page.has_more,
            next_offset: page.next_offset,
        })
    }

    pub fn get_review_item(
        &self,
        request: GetReviewItemRequest,
    ) -> Result<ReviewItemRecord, A2dFfiError> {
        Ok(self
            .core
            .get_review_item(&a2d_domain::ReviewItemId::parse(&request.review_item_id)?)?
            .into())
    }

    pub fn resolve_review_item(
        &self,
        request: ResolveReviewItemRequest,
    ) -> Result<ReviewItemMutationResult, A2dFfiError> {
        self.core
            .resolve_review_item(core::ResolveReviewItemRequest {
                review_item_id: a2d_domain::ReviewItemId::parse(&request.review_item_id)?,
                resolution_code: request.resolution_code,
                resolved_at_ms: request.resolved_at_ms,
                actor: request.actor,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn defer_review_item(
        &self,
        request: DeferReviewItemRequest,
    ) -> Result<ReviewItemMutationResult, A2dFfiError> {
        self.core
            .defer_review_item(core::DeferReviewItemRequest {
                review_item_id: a2d_domain::ReviewItemId::parse(&request.review_item_id)?,
                deferred_at_ms: request.deferred_at_ms,
                actor: request.actor,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use a2d_domain::{ErrorSeverity, ReviewItemId};

    use super::*;

    #[test]
    fn review_projection_preserves_every_kind_status_and_ordered_details() {
        let item = DomainReviewItem::new(
            ReviewItemId::generate(),
            DomainReviewItemKind::RestoreConflict,
            None,
            None,
            ErrorSeverity::Critical,
            DomainReviewItemStatus::Deferred,
            BTreeMap::from([
                ("a".to_string(), "first".to_string()),
                ("b".to_string(), "second".to_string()),
            ]),
            None,
            100,
            None,
        );
        let projected: ReviewItemRecord = item.into();
        assert_eq!(projected.kind, ReviewItemKind::RestoreConflict);
        assert_eq!(projected.status, ReviewItemStatus::Deferred);
        assert_eq!(projected.severity, "Critical");
        assert_eq!(projected.details[0].key, "a");
        assert_eq!(projected.details[1].key, "b");
    }

    #[test]
    fn ffi_methods_list_defer_and_resolve_rust_owned_review_state() {
        let root = std::env::temp_dir().join(format!(
            "a2d-ffi-review-{}",
            a2d_domain::PageId::generate()
        ));
        let core_handle = core::A2dCore::open(core::OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let item = core_handle
            .create_review_item(core::CreateReviewItemRequest {
                kind: DomainReviewItemKind::Revision,
                page_id: None,
                scan_id: None,
                severity: ErrorSeverity::Warning,
                details: BTreeMap::from([(
                    "reason_code".to_string(),
                    "FFI_REVIEW_TEST".to_string(),
                )]),
                created_at_ms: 100,
            })
            .unwrap();
        let client = A2dClient { core: core_handle };

        let listed = client
            .list_review_items(ListReviewItemsRequest {
                kind: Some(ReviewItemKind::Revision),
                status: Some(ReviewItemStatus::Open),
                page_id: None,
                scan_id: None,
                limit: 10,
                offset: 0,
            })
            .unwrap();
        assert_eq!(listed.items.len(), 1);
        assert_eq!(listed.items[0].id, item.id().to_string());

        let deferred = client
            .defer_review_item(DeferReviewItemRequest {
                review_item_id: item.id().to_string(),
                deferred_at_ms: 200,
                actor: "ffi-test".to_string(),
            })
            .unwrap();
        assert_eq!(deferred.item.status, ReviewItemStatus::Deferred);
        assert!(!deferred.committed_data_deleted);

        let resolved = client
            .resolve_review_item(ResolveReviewItemRequest {
                review_item_id: item.id().to_string(),
                resolution_code: "KEEP_BOTH_VERSIONS".to_string(),
                resolved_at_ms: 300,
                actor: "ffi-test".to_string(),
            })
            .unwrap();
        assert_eq!(resolved.item.status, ReviewItemStatus::Resolved);
        assert_eq!(
            resolved.item.resolution_code.as_deref(),
            Some("KEEP_BOTH_VERSIONS")
        );
        assert!(!resolved.committed_data_deleted);

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn mutation_projection_keeps_no_deletion_contract_explicit() {
        let item = DomainReviewItem::new(
            ReviewItemId::generate(),
            DomainReviewItemKind::Revision,
            None,
            None,
            ErrorSeverity::Warning,
            DomainReviewItemStatus::Resolved,
            BTreeMap::new(),
            Some("KEEP_BOTH_VERSIONS".to_string()),
            100,
            Some(200),
        );
        let projected: ReviewItemMutationResult = core::ReviewItemMutationResult {
            item,
            changed: true,
            audit_event_id: None,
            committed_data_deleted: false,
        }
        .into();
        assert!(!projected.committed_data_deleted);
        assert_eq!(projected.item.status, ReviewItemStatus::Resolved);
    }
}
