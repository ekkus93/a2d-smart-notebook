//! Rust-owned page-version timeline and review handoff for Milestone 9.5.

use std::collections::BTreeMap;

use a2d_domain::{
    A2dError, AssetId, ErrorCategory, ErrorCode, ErrorSeverity, PageId, ReviewItem, ReviewItemId,
    ReviewItemKind, ReviewItemStatus, Scan, ScanId,
};
use a2d_image::{PERCEPTUAL_FINGERPRINT_V1_HEIGHT, PERCEPTUAL_FINGERPRINT_V1_WIDTH};
use a2d_storage::{
    AssetRepository, MAX_PAGE_VERSION_LIST_OFFSET, PageRepository, PageVersionQuery,
    PageVersionRepository, ReviewItemQuery, ReviewItemRepository, ScanRepository,
};

use crate::{A2dCore, CompareStoredScansRequest, StoredScanComparisonEvidence};

pub const MAX_PAGE_VERSION_PAGE_SIZE: u32 = 100;
const REVISION_DECISION_WARNING_PREFIX: &str = "REVISION_DECISION_";
const VERSION_UI_REVIEW_REASON: &str = "UNRESOLVED_PAGE_VERSION";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetPageVersionTimelineRequest {
    pub page_id: PageId,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageVersionRecord {
    pub scan_id: ScanId,
    pub captured_at_ms: i64,
    pub preferred: bool,
    pub physical_copy_id: Option<a2d_domain::PhysicalCopyId>,
    pub supersedes_scan_id: Option<ScanId>,
    pub quality_status: a2d_domain::QualityStatus,
    pub pipeline_version: String,
    pub decision_code: Option<String>,
    pub original_asset_path: String,
    pub corrected_asset_path: Option<String>,
    pub thumbnail_asset_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageVersionTimeline {
    pub page_id: PageId,
    pub preferred_scan_id: Option<ScanId>,
    pub preferred_version: Option<PageVersionRecord>,
    pub items: Vec<PageVersionRecord>,
    pub has_more: bool,
    pub next_offset: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparePageVersionsRequest {
    pub baseline_scan_id: ScanId,
    pub candidate_scan_id: ScanId,
    pub minimum_cell_absolute_difference: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageVersionComparison {
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub evidence: StoredScanComparisonEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovePageVersionToReviewRequest {
    pub page_id: PageId,
    pub scan_id: ScanId,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageVersionReviewResult {
    pub review_item: ReviewItem,
    pub created: bool,
}

impl A2dCore {
    pub fn get_page_version_timeline(
        &self,
        request: GetPageVersionTimelineRequest,
    ) -> Result<PageVersionTimeline, A2dError> {
        if request.limit == 0 || request.limit > MAX_PAGE_VERSION_PAGE_SIZE {
            return Err(version_error(
                "CORE_PAGE_VERSION_LIST_LIMIT_INVALID",
                ErrorCategory::Validation,
                "page-version list limit is outside the supported range",
            )
            .with_detail("limit", request.limit.to_string()));
        }
        if request.offset > MAX_PAGE_VERSION_LIST_OFFSET
            || request
                .offset
                .checked_add(request.limit)
                .is_none_or(|end| end > MAX_PAGE_VERSION_LIST_OFFSET)
        {
            return Err(version_error(
                "CORE_PAGE_VERSION_LIST_OFFSET_INVALID",
                ErrorCategory::Validation,
                "page-version list offset is outside the supported range",
            )
            .with_detail("offset", request.offset.to_string()));
        }
        let storage_limit = request.limit.checked_add(1).ok_or_else(|| {
            version_error(
                "CORE_PAGE_VERSION_LIST_LIMIT_OVERFLOW",
                ErrorCategory::Validation,
                "page-version list limit overflowed",
            )
        })?;
        let storage = self.lock_storage()?;
        let page = storage.get_page(&request.page_id)?.ok_or_else(|| {
            version_error(
                "CORE_PAGE_VERSION_PAGE_NOT_FOUND",
                ErrorCategory::Validation,
                "the requested page does not exist",
            )
            .with_detail("page_id", request.page_id.to_string())
        })?;
        let preferred_version = page
            .preferred_scan_id
            .as_ref()
            .map(|preferred_scan_id| {
                let scan = storage.get_scan(preferred_scan_id)?.ok_or_else(|| {
                    version_error(
                        "CORE_PAGE_VERSION_PREFERRED_SCAN_NOT_FOUND",
                        ErrorCategory::Integrity,
                        "the page preferred-scan pointer references a missing scan",
                    )
                    .with_detail("page_id", request.page_id.to_string())
                    .with_detail("preferred_scan_id", preferred_scan_id.to_string())
                })?;
                if scan.page_id != request.page_id || !scan.preferred {
                    return Err(version_error(
                        "CORE_PAGE_VERSION_PREFERRED_SCAN_INVALID",
                        ErrorCategory::Integrity,
                        "the page preferred-scan pointer disagrees with stored scan ownership or preference",
                    )
                    .with_detail("page_id", request.page_id.to_string())
                    .with_detail("preferred_scan_id", preferred_scan_id.to_string())
                    .with_detail("scan_page_id", scan.page_id.to_string())
                    .with_detail("scan_preferred", scan.preferred.to_string()));
                }
                self.project_page_version(&storage, scan)
            })
            .transpose()?;
        let mut scans = storage.list_page_versions(&PageVersionQuery {
            page_id: request.page_id.clone(),
            limit: storage_limit,
            offset: request.offset,
        })?;
        let has_more = scans.len() > request.limit as usize;
        if has_more {
            scans.truncate(request.limit as usize);
        }
        let mut items = Vec::with_capacity(scans.len());
        for scan in scans {
            items.push(self.project_page_version(&storage, scan)?);
        }
        let next_offset = if has_more {
            Some(request.offset.checked_add(request.limit).ok_or_else(|| {
                version_error(
                    "CORE_PAGE_VERSION_NEXT_OFFSET_OVERFLOW",
                    ErrorCategory::Integrity,
                    "page-version pagination overflowed its next offset",
                )
            })?)
        } else {
            None
        };
        Ok(PageVersionTimeline {
            page_id: request.page_id,
            preferred_scan_id: page.preferred_scan_id,
            preferred_version,
            items,
            has_more,
            next_offset,
        })
    }

    pub fn compare_page_versions(
        &self,
        request: ComparePageVersionsRequest,
    ) -> Result<PageVersionComparison, A2dError> {
        let evidence = self.compare_stored_scans(CompareStoredScansRequest {
            baseline_scan_id: request.baseline_scan_id,
            candidate_scan_id: request.candidate_scan_id,
            minimum_cell_absolute_difference: request.minimum_cell_absolute_difference,
        })?;
        Ok(PageVersionComparison {
            grid_columns: u32::try_from(PERCEPTUAL_FINGERPRINT_V1_WIDTH).map_err(|_| {
                version_error(
                    "CORE_PAGE_VERSION_GRID_WIDTH_INVALID",
                    ErrorCategory::Internal,
                    "page-version comparison grid width is outside the portable range",
                )
            })?,
            grid_rows: u32::try_from(PERCEPTUAL_FINGERPRINT_V1_HEIGHT).map_err(|_| {
                version_error(
                    "CORE_PAGE_VERSION_GRID_HEIGHT_INVALID",
                    ErrorCategory::Internal,
                    "page-version comparison grid height is outside the portable range",
                )
            })?,
            evidence,
        })
    }

    pub fn move_page_version_to_review(
        &self,
        request: MovePageVersionToReviewRequest,
    ) -> Result<PageVersionReviewResult, A2dError> {
        if request.created_at_ms <= 0 {
            return Err(version_error(
                "CORE_PAGE_VERSION_REVIEW_TIME_INVALID",
                ErrorCategory::Validation,
                "created_at_ms must be a positive Unix timestamp",
            ));
        }
        let new_id = ReviewItemId::try_generate()?;
        let mut storage = self.lock_storage()?;
        storage.transaction(|tx| {
            let page = PageRepository::get_page(tx, &request.page_id)?.ok_or_else(|| {
                version_error(
                    "CORE_PAGE_VERSION_PAGE_NOT_FOUND",
                    ErrorCategory::Validation,
                    "the requested page does not exist",
                )
                .with_detail("page_id", request.page_id.to_string())
            })?;
            let scan = ScanRepository::get_scan(tx, &request.scan_id)?.ok_or_else(|| {
                version_error(
                    "CORE_PAGE_VERSION_SCAN_NOT_FOUND",
                    ErrorCategory::Validation,
                    "the requested page version does not exist",
                )
                .with_detail("scan_id", request.scan_id.to_string())
            })?;
            if scan.page_id != request.page_id {
                return Err(version_error(
                    "CORE_PAGE_VERSION_PAGE_SCAN_MISMATCH",
                    ErrorCategory::Validation,
                    "the requested scan does not belong to the requested page",
                )
                .with_detail("page_id", request.page_id.to_string())
                .with_detail("scan_id", request.scan_id.to_string())
                .with_detail("scan_page_id", scan.page_id.to_string()));
            }
            for status in [ReviewItemStatus::Open, ReviewItemStatus::Deferred] {
                let existing = ReviewItemRepository::list_review_items(
                    tx,
                    &ReviewItemQuery {
                        kind: Some(ReviewItemKind::Revision),
                        status: Some(status),
                        page_id: Some(request.page_id.clone()),
                        scan_id: Some(request.scan_id.clone()),
                        limit: 1,
                        offset: 0,
                    },
                )?;
                if let Some(item) = existing.into_iter().next() {
                    return Ok(PageVersionReviewResult {
                        review_item: item,
                        created: false,
                    });
                }
            }
            let details = BTreeMap::from([
                (
                    "reason_code".to_string(),
                    VERSION_UI_REVIEW_REASON.to_string(),
                ),
                (
                    "preferred_scan_id".to_string(),
                    page.preferred_scan_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "NONE".to_string()),
                ),
            ]);
            let item = ReviewItem::new(
                new_id.clone(),
                ReviewItemKind::Revision,
                Some(request.page_id.clone()),
                Some(request.scan_id.clone()),
                ErrorSeverity::Warning,
                ReviewItemStatus::Open,
                details,
                None,
                request.created_at_ms,
                None,
            );
            ReviewItemRepository::insert_review_item(tx, &item)?;
            Ok(PageVersionReviewResult {
                review_item: item,
                created: true,
            })
        })
    }

    fn project_page_version(
        &self,
        storage: &a2d_storage::Storage,
        scan: Scan,
    ) -> Result<PageVersionRecord, A2dError> {
        let scan_id_string = scan.id().to_string();
        let original_asset_path =
            self.resolve_page_version_asset(storage, &scan.original_asset_id, &scan_id_string)?;
        let corrected_asset_path = scan
            .corrected_asset_id
            .as_ref()
            .map(|asset_id| self.resolve_page_version_asset(storage, asset_id, &scan_id_string))
            .transpose()?;
        let thumbnail_asset_path = scan
            .thumbnail_asset_id
            .as_ref()
            .map(|asset_id| self.resolve_page_version_asset(storage, asset_id, &scan_id_string))
            .transpose()?;
        let decision_code = scan
            .warnings
            .iter()
            .find_map(|warning| warning.strip_prefix(REVISION_DECISION_WARNING_PREFIX))
            .map(ToString::to_string);
        Ok(PageVersionRecord {
            scan_id: scan.id().clone(),
            captured_at_ms: scan.captured_at_ms,
            preferred: scan.preferred,
            physical_copy_id: scan.physical_copy_id,
            supersedes_scan_id: scan.supersedes_scan_id,
            quality_status: scan.quality_status,
            pipeline_version: scan.pipeline_version,
            decision_code,
            original_asset_path,
            corrected_asset_path,
            thumbnail_asset_path,
        })
    }

    fn resolve_page_version_asset(
        &self,
        storage: &a2d_storage::Storage,
        asset_id: &AssetId,
        scan_id: &str,
    ) -> Result<String, A2dError> {
        let asset = storage.get_asset(asset_id)?.ok_or_else(|| {
            version_error(
                "CORE_PAGE_VERSION_ASSET_NOT_FOUND",
                ErrorCategory::Integrity,
                "a page version references an asset row that does not exist",
            )
            .with_detail("scan_id", scan_id)
            .with_detail("asset_id", asset_id.to_string())
        })?;
        let path = self.asset_store.resolve(&asset.relative_path)?;
        path.to_str().map(ToString::to_string).ok_or_else(|| {
            version_error(
                "CORE_PAGE_VERSION_ASSET_PATH_NOT_UTF8",
                ErrorCategory::Storage,
                "a page-version asset path cannot cross the portable string boundary",
            )
            .with_detail("scan_id", scan_id)
            .with_detail("asset_id", asset_id.to_string())
        })
    }
}

fn version_error(
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
        "error.core.page_versions",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_domain::{
        AssetKind, AuditEventId, CaptureSource, LayoutId, Page, PageKind, PageState, QualityStatus,
        SmartPageId,
    };
    use a2d_storage::{
        AssetRepository, ChangePreferredScanRequest, PageRepository, ReviewItemRepository,
        ScanRepository,
    };

    use super::*;
    use crate::OpenLibraryRequest;

    fn open_core() -> (std::sync::Arc<A2dCore>, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("a2d-core-version-history-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, root)
    }

    fn add_page(core: &A2dCore, page_id: &PageId) {
        core.lock_storage()
            .unwrap()
            .insert_page(&Page::new(
                page_id.clone(),
                PageKind::SmartPage {
                    smart_page_id: SmartPageId::generate(),
                    page_set_id: None,
                    visible_page_number: Some(1),
                },
                LayoutId::parse("USLETTER-LINED").unwrap(),
                None,
                PageState::NeedsReview,
                10,
            ))
            .unwrap();
    }

    fn add_scan(core: &A2dCore, page_id: &PageId, captured_at_ms: i64) -> ScanId {
        let asset = core
            .asset_store
            .commit(
                b"durable-version-original",
                AssetKind::Original,
                "image/jpeg",
            )
            .unwrap();
        let scan_id = ScanId::generate();
        let storage = core.lock_storage().unwrap();
        storage.insert_asset(&asset).unwrap();
        storage
            .insert_scan(&Scan::new(
                scan_id.clone(),
                page_id.clone(),
                None,
                CaptureSource::Camera,
                captured_at_ms,
                asset.id().clone(),
                None,
                None,
                None,
                "pipeline-v1".to_string(),
                QualityStatus::NeedsReview,
                vec![],
                false,
                None,
                "fingerprint-v1".to_string(),
            ))
            .unwrap();
        scan_id
    }

    #[test]
    fn timeline_projects_stable_versions_and_preferred_record() {
        let (core, root) = open_core();
        let page_id = PageId::generate();
        add_page(&core, &page_id);
        let preferred_scan_id = add_scan(&core, &page_id, 100);
        let newest_scan_id = add_scan(&core, &page_id, 200);
        core.lock_storage()
            .unwrap()
            .change_preferred_scan(ChangePreferredScanRequest {
                page_id: page_id.clone(),
                scan_id: preferred_scan_id.clone(),
                changed_at_ms: 300,
                actor: "version-history-test".to_string(),
                operation_id: AuditEventId::generate(),
            })
            .unwrap();

        let timeline = core
            .get_page_version_timeline(GetPageVersionTimelineRequest {
                page_id: page_id.clone(),
                limit: 1,
                offset: 0,
            })
            .unwrap();
        assert_eq!(timeline.page_id, page_id);
        assert_eq!(timeline.preferred_scan_id, Some(preferred_scan_id.clone()));
        assert_eq!(
            timeline
                .preferred_version
                .as_ref()
                .map(|item| &item.scan_id),
            Some(&preferred_scan_id)
        );
        assert_eq!(timeline.items.len(), 1);
        assert_eq!(timeline.items[0].scan_id, newest_scan_id);
        assert!(!timeline.items[0].preferred);
        assert!(timeline.has_more);
        assert_eq!(timeline.next_offset, Some(1));
        assert!(std::path::Path::new(&timeline.items[0].original_asset_path).is_file());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn move_to_review_is_idempotent_and_does_not_duplicate_queue_state() {
        let (core, root) = open_core();
        let page_id = PageId::generate();
        add_page(&core, &page_id);
        let scan_id = add_scan(&core, &page_id, 100);

        let first = core
            .move_page_version_to_review(MovePageVersionToReviewRequest {
                page_id: page_id.clone(),
                scan_id: scan_id.clone(),
                created_at_ms: 200,
            })
            .unwrap();
        assert!(first.created);
        assert_eq!(first.review_item.kind, ReviewItemKind::Revision);
        assert_eq!(first.review_item.page_id.as_ref(), Some(&page_id));
        assert_eq!(first.review_item.scan_id.as_ref(), Some(&scan_id));

        let repeated = core
            .move_page_version_to_review(MovePageVersionToReviewRequest {
                page_id: page_id.clone(),
                scan_id: scan_id.clone(),
                created_at_ms: 300,
            })
            .unwrap();
        assert!(!repeated.created);
        assert_eq!(repeated.review_item.id(), first.review_item.id());

        let stored = core
            .lock_storage()
            .unwrap()
            .list_review_items(&ReviewItemQuery {
                kind: Some(ReviewItemKind::Revision),
                status: None,
                page_id: Some(page_id),
                scan_id: Some(scan_id),
                limit: 10,
                offset: 0,
            })
            .unwrap();
        assert_eq!(stored.len(), 1);

        std::fs::remove_dir_all(root).ok();
    }
}
