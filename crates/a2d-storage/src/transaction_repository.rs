//! Repository trait adapters for SQLite transactions.
//!
//! `rusqlite::Transaction` dereferences to `Connection`, so ordinary method-call syntax can use the
//! `Connection` repository implementations automatically. UFCS calls require the trait to be
//! implemented for the transaction type itself. These adapters preserve the active transaction and
//! delegate every operation to the underlying connection.

use a2d_domain::{A2dError, Asset, AssetId, AuditEvent, AuditEventId, Page, PageId, Scan, ScanId};
use rusqlite::Transaction;

use crate::repository::{AssetRepository, AuditEventRepository, PageRepository, ScanRepository};

impl PageRepository for Transaction<'_> {
    fn insert_page(&self, value: &Page) -> Result<(), A2dError> {
        PageRepository::insert_page(&**self, value)
    }

    fn get_page(&self, id: &PageId) -> Result<Option<Page>, A2dError> {
        PageRepository::get_page(&**self, id)
    }

    fn set_generated_pdf_asset(
        &self,
        page_id: &PageId,
        asset_id: &AssetId,
    ) -> Result<(), A2dError> {
        PageRepository::set_generated_pdf_asset(&**self, page_id, asset_id)
    }

    fn set_preferred_scan(&self, page_id: &PageId, scan_id: &ScanId) -> Result<(), A2dError> {
        PageRepository::set_preferred_scan(&**self, page_id, scan_id)
    }
}

impl AssetRepository for Transaction<'_> {
    fn insert_asset(&self, value: &Asset) -> Result<(), A2dError> {
        AssetRepository::insert_asset(&**self, value)
    }

    fn get_asset(&self, id: &AssetId) -> Result<Option<Asset>, A2dError> {
        AssetRepository::get_asset(&**self, id)
    }
}

impl ScanRepository for Transaction<'_> {
    fn insert_scan(&self, value: &Scan) -> Result<(), A2dError> {
        ScanRepository::insert_scan(&**self, value)
    }

    fn get_scan(&self, id: &ScanId) -> Result<Option<Scan>, A2dError> {
        ScanRepository::get_scan(&**self, id)
    }
}

impl AuditEventRepository for Transaction<'_> {
    fn insert_audit_event(&self, value: &AuditEvent) -> Result<(), A2dError> {
        AuditEventRepository::insert_audit_event(&**self, value)
    }

    fn get_audit_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>, A2dError> {
        AuditEventRepository::get_audit_event(&**self, id)
    }
}
