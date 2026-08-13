//! Bounded, stable read access to every durable scan version for one logical page.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, Scan, ScanId};
use rusqlite::{Connection, params};

use crate::repository::ScanRepository;
use crate::{Storage, map_rusqlite_error};

pub const MAX_PAGE_VERSION_LIST_LIMIT: u32 = 101;
pub const MAX_PAGE_VERSION_LIST_OFFSET: u32 = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageVersionQuery {
    pub page_id: PageId,
    pub limit: u32,
    pub offset: u32,
}

pub trait PageVersionRepository {
    fn list_page_versions(&self, query: &PageVersionQuery) -> Result<Vec<Scan>, A2dError>;
}

impl PageVersionRepository for Connection {
    fn list_page_versions(&self, query: &PageVersionQuery) -> Result<Vec<Scan>, A2dError> {
        validate_query(query)?;
        let mut statement = self
            .prepare(
                "SELECT id FROM scans WHERE page_id = ?1 \
                 ORDER BY captured_at_ms DESC, id DESC LIMIT ?2 OFFSET ?3",
            )
            .map_err(|error| map_rusqlite_error("preparing page-version list", error))?;
        let rows = statement
            .query_map(
                params![query.page_id.to_string(), query.limit, query.offset],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| map_rusqlite_error("listing page-version ids", error))?;
        let mut versions = Vec::with_capacity(query.limit as usize);
        for row in rows {
            let id = ScanId::parse(
                &row.map_err(|error| map_rusqlite_error("decoding page-version id", error))?,
            )?;
            let scan = ScanRepository::get_scan(self, &id)?.ok_or_else(|| {
                version_integrity_error(
                    "STORAGE_PAGE_VERSION_SCAN_DISAPPEARED",
                    "a page-version row disappeared while the stable version list was being read",
                )
                .with_detail("scan_id", id.to_string())
                .with_detail("page_id", query.page_id.to_string())
            })?;
            if scan.page_id != query.page_id {
                return Err(version_integrity_error(
                    "STORAGE_PAGE_VERSION_PAGE_MISMATCH",
                    "a page-version query returned a scan owned by another page",
                )
                .with_detail("scan_id", id.to_string())
                .with_detail("requested_page_id", query.page_id.to_string())
                .with_detail("scan_page_id", scan.page_id.to_string()));
            }
            versions.push(scan);
        }
        Ok(versions)
    }
}

impl PageVersionRepository for Storage {
    fn list_page_versions(&self, query: &PageVersionQuery) -> Result<Vec<Scan>, A2dError> {
        PageVersionRepository::list_page_versions(&self.conn, query)
    }
}

fn validate_query(query: &PageVersionQuery) -> Result<(), A2dError> {
    if query.limit == 0 || query.limit > MAX_PAGE_VERSION_LIST_LIMIT {
        return Err(version_validation_error(
            "STORAGE_PAGE_VERSION_LIST_LIMIT_INVALID",
            "page-version list limit is outside the supported range",
        )
        .with_detail("limit", query.limit.to_string()));
    }
    if query.offset > MAX_PAGE_VERSION_LIST_OFFSET {
        return Err(version_validation_error(
            "STORAGE_PAGE_VERSION_LIST_OFFSET_INVALID",
            "page-version list offset is outside the supported range",
        )
        .with_detail("offset", query.offset.to_string()));
    }
    Ok(())
}

fn version_validation_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.page_versions",
        message,
        false,
    )
}

fn version_integrity_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.page_versions_integrity",
        message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_domain::{
        Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, Page, PageId, PageKind,
        PageState, QualityStatus, Scan, ScanId, SmartPageId,
    };

    use super::*;
    use crate::{AssetRepository, PageRepository, ScanRepository};

    fn page(id: PageId) -> Page {
        Page::new(
            id,
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("USLETTER-LINED").unwrap(),
            None,
            PageState::NeedsReview,
            10,
        )
    }

    fn add_scan(storage: &Storage, page_id: &PageId, captured_at_ms: i64) -> ScanId {
        let asset_id = AssetId::generate();
        storage
            .insert_asset(&Asset::new(
                asset_id.clone(),
                AssetKind::Original,
                format!("assets/originals/{asset_id}.jpg"),
                "image/jpeg".to_string(),
                1,
                format!("sha256-{asset_id}"),
                captured_at_ms,
                true,
                EncryptionState::Plaintext,
            ))
            .unwrap();
        let scan_id = ScanId::generate();
        storage
            .insert_scan(&Scan::new(
                scan_id.clone(),
                page_id.clone(),
                None,
                CaptureSource::Camera,
                captured_at_ms,
                asset_id,
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
    fn page_versions_are_stably_newest_first_and_bounded() {
        let storage = Storage::open_in_memory().unwrap();
        let page_id = PageId::generate();
        storage.insert_page(&page(page_id.clone())).unwrap();
        let oldest = add_scan(&storage, &page_id, 100);
        let middle = add_scan(&storage, &page_id, 200);
        let newest = add_scan(&storage, &page_id, 300);

        let first = storage
            .list_page_versions(&PageVersionQuery {
                page_id: page_id.clone(),
                limit: 2,
                offset: 0,
            })
            .unwrap();
        assert_eq!(
            first.iter().map(|scan| scan.id()).collect::<Vec<_>>(),
            vec![&newest, &middle]
        );
        let second = storage
            .list_page_versions(&PageVersionQuery {
                page_id,
                limit: 2,
                offset: 2,
            })
            .unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].id(), &oldest);
    }

    #[test]
    fn page_version_query_rejects_unbounded_requests() {
        let storage = Storage::open_in_memory().unwrap();
        let error = storage
            .list_page_versions(&PageVersionQuery {
                page_id: PageId::generate(),
                limit: MAX_PAGE_VERSION_LIST_LIMIT + 1,
                offset: 0,
            })
            .unwrap_err();
        assert_eq!(error.code.to_string(), "STORAGE_PAGE_VERSION_LIST_LIMIT_INVALID");
    }
}
