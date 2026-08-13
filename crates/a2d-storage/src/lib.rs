//! Owns the canonical SQLite database. All SQL is private to this crate; no other crate issues SQL.
//!
//! `rusqlite` uses bundled SQLite for reproducible Android cross-compilation. Connections enable
//! foreign keys, WAL mode, `synchronous=NORMAL`, and a bounded busy timeout. WAL/NORMAL preserves
//! database consistency and application-crash durability, but the latest committed transaction may
//! be rolled back after an operating-system crash or power loss; this crate does not call such a
//! commit fully power-loss durable. Asset files complete their separate file and directory
//! synchronization contract before database registration is attempted. See
//! `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.
//!
//! Numbered migrations are applied transactionally and their exact SQL SHA-256 digests are recorded
//! and revalidated on every open; edited history, version gaps, and databases newer than the app
//! fail closed.

use std::fmt;
use std::path::Path;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

mod asset_recovery;
mod assets;
#[macro_use]
mod integrity_support;
#[allow(unused_imports)]
mod integrity;
mod json_columns;
mod migration_history;
mod migrations;
mod preferred_scan;
mod repository;
mod review_repository;
pub use review_repository::{
    MAX_REVIEW_LIST_LIMIT, MAX_REVIEW_LIST_OFFSET, ReviewItemQuery, ReviewItemRepository,
};
mod review;
pub use review::*;
mod revision;
pub use revision::*;
mod transaction_repository;
mod workflow;

pub use asset_recovery::{AssetPersistenceFailureStage, OrphanedFinalAsset};
pub use assets::AssetStore;
pub use integrity::{
    IntegrityCancellation, IntegrityCheckOptions, IntegrityCheckOutcome, IntegrityFinding,
    IntegrityFindingSeverity, IntegrityReport,
};
pub use migrations::{MIGRATIONS, Migration};
pub use preferred_scan::{ChangePreferredScanRequest, ChangePreferredScanResult};
pub use repository::{
    AssetRepository, AuditEventRepository, NotebookDesignRepository, NotebookRepository,
    OcrRunRepository, PageRepository, PageSetRepository, ScanRepository,
};
pub use workflow::{NotebookWorkflowRepository, PageLookupRepository};

/// An open library database. A single connection is serialized by the Rust core's mutex at the FFI
/// boundary; the storage crate does not expose SQL or a connection pool.
pub struct Storage {
    conn: rusqlite::Connection,
}

pub(crate) fn map_rusqlite_error(context: &str, error: rusqlite::Error) -> A2dError {
    let message = error.to_string();
    let lower = message.to_lowercase();
    let (code, category, retryable) = if lower.contains("foreign key")
        || lower.contains("unique constraint")
        || lower.contains("check constraint")
        || lower.contains("not null constraint")
    {
        (
            "STORAGE_CONSTRAINT_VIOLATION",
            ErrorCategory::Validation,
            false,
        )
    } else if lower.contains("database is locked") || lower.contains("database is busy") {
        ("STORAGE_BUSY", ErrorCategory::Storage, true)
    } else if lower.contains("malformed") || lower.contains("corrupt") {
        ("STORAGE_DATABASE_CORRUPT", ErrorCategory::Integrity, false)
    } else {
        ("STORAGE_SQL_ERROR", ErrorCategory::Storage, true)
    };
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.storage.sql",
        format!("{context}: {message}"),
        retryable,
    )
    .with_detail("context", context)
}

fn map_io_error(context: &str, error: std::io::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_IO_ERROR"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.io",
        format!("{context}: {error}"),
        true,
    )
    .with_detail("context", context)
}

/// Long enough for ordinary mobile writer serialization, bounded so a dead writer is not hidden
/// indefinitely. Real-device measurements may tune this value later.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn configure_busy_timeout(conn: &rusqlite::Connection) -> Result<(), A2dError> {
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| map_rusqlite_error("configuring SQLite busy timeout", error))
}

impl Storage {
    /// Opens or creates a library database and validates/applies every migration before returning.
    pub fn open(path: &Path) -> Result<Self, A2dError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| map_io_error("creating database parent directory", error))?;
        }
        let conn = rusqlite::Connection::open(path)
            .map_err(|error| map_rusqlite_error("opening database", error))?;
        let mut storage = Self { conn };
        storage.initialize_connection()?;
        storage.migrate()?;
        Ok(storage)
    }

    /// In-memory database with the identical pragmas and migration path used by production.
    pub fn open_in_memory() -> Result<Self, A2dError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|error| map_rusqlite_error("opening in-memory database", error))?;
        let mut storage = Self { conn };
        storage.initialize_connection()?;
        storage.migrate()?;
        Ok(storage)
    }

    fn initialize_connection(&self) -> Result<(), A2dError> {
        self.conn
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| map_rusqlite_error("enabling foreign keys", error))?;
        self.conn
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| map_rusqlite_error("enabling WAL journal mode", error))?;
        self.conn
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|error| map_rusqlite_error("setting synchronous mode", error))?;
        configure_busy_timeout(&self.conn)
    }

    fn migrate(&mut self) -> Result<(), A2dError> {
        migration_history::migrate(&mut self.conn)
    }

    pub fn schema_version(&self) -> Result<i64, A2dError> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| map_rusqlite_error("reading schema version", error))
    }

    /// Runs a closure inside one immediate transaction. An error from the closure or commit leaves
    /// no partial database mutation visible.
    pub fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, A2dError>,
    ) -> Result<T, A2dError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|error| map_rusqlite_error("starting transaction", error))?;
        let value = operation(&tx)?;
        tx.commit()
            .map_err(|error| map_rusqlite_error("committing transaction", error))?;
        Ok(value)
    }
}

macro_rules! impl_design_delegate {
    () => {
        impl NotebookDesignRepository for Storage {
            fn insert_notebook_design(
                &self,
                value: &a2d_domain::NotebookDesign,
            ) -> Result<(), A2dError> {
                NotebookDesignRepository::insert_notebook_design(&self.conn, value)
            }

            fn get_notebook_design(
                &self,
                id: &a2d_domain::NotebookDesignId,
            ) -> Result<Option<a2d_domain::NotebookDesign>, A2dError> {
                NotebookDesignRepository::get_notebook_design(&self.conn, id)
            }
        }
    };
}

macro_rules! impl_notebook_delegate {
    () => {
        impl NotebookRepository for Storage {
            fn insert_notebook(&self, value: &a2d_domain::Notebook) -> Result<(), A2dError> {
                NotebookRepository::insert_notebook(&self.conn, value)
            }

            fn get_notebook(
                &self,
                id: &a2d_domain::NotebookId,
            ) -> Result<Option<a2d_domain::Notebook>, A2dError> {
                NotebookRepository::get_notebook(&self.conn, id)
            }
        }
    };
}

macro_rules! impl_page_set_delegate {
    () => {
        impl PageSetRepository for Storage {
            fn insert_page_set(&self, value: &a2d_domain::PageSet) -> Result<(), A2dError> {
                PageSetRepository::insert_page_set(&self.conn, value)
            }

            fn get_page_set(
                &self,
                id: &a2d_domain::PageSetId,
            ) -> Result<Option<a2d_domain::PageSet>, A2dError> {
                PageSetRepository::get_page_set(&self.conn, id)
            }
        }
    };
}

macro_rules! impl_page_delegate {
    () => {
        impl PageRepository for Storage {
            fn insert_page(&self, value: &a2d_domain::Page) -> Result<(), A2dError> {
                PageRepository::insert_page(&self.conn, value)
            }

            fn get_page(
                &self,
                id: &a2d_domain::PageId,
            ) -> Result<Option<a2d_domain::Page>, A2dError> {
                PageRepository::get_page(&self.conn, id)
            }

            fn set_generated_pdf_asset(
                &self,
                page_id: &a2d_domain::PageId,
                asset_id: &a2d_domain::AssetId,
            ) -> Result<(), A2dError> {
                PageRepository::set_generated_pdf_asset(&self.conn, page_id, asset_id)
            }

            fn set_preferred_scan(
                &self,
                page_id: &a2d_domain::PageId,
                scan_id: &a2d_domain::ScanId,
            ) -> Result<(), A2dError> {
                PageRepository::set_preferred_scan(&self.conn, page_id, scan_id)
            }
        }
    };
}

macro_rules! impl_asset_delegate {
    () => {
        impl AssetRepository for Storage {
            fn insert_asset(&self, value: &a2d_domain::Asset) -> Result<(), A2dError> {
                AssetRepository::insert_asset(&self.conn, value)
            }

            fn get_asset(
                &self,
                id: &a2d_domain::AssetId,
            ) -> Result<Option<a2d_domain::Asset>, A2dError> {
                AssetRepository::get_asset(&self.conn, id)
            }
        }
    };
}

macro_rules! impl_scan_delegate {
    () => {
        impl ScanRepository for Storage {
            fn insert_scan(&self, value: &a2d_domain::Scan) -> Result<(), A2dError> {
                ScanRepository::insert_scan(&self.conn, value)
            }

            fn get_scan(
                &self,
                id: &a2d_domain::ScanId,
            ) -> Result<Option<a2d_domain::Scan>, A2dError> {
                ScanRepository::get_scan(&self.conn, id)
            }

            fn find_scan_by_recovery_token(
                &self,
                page_id: &a2d_domain::PageId,
                token: &str,
            ) -> Result<Option<a2d_domain::Scan>, A2dError> {
                ScanRepository::find_scan_by_recovery_token(&self.conn, page_id, token)
            }
        }
    };
}

macro_rules! impl_ocr_delegate {
    () => {
        impl OcrRunRepository for Storage {
            fn insert_ocr_run(&self, value: &a2d_domain::OcrRun) -> Result<(), A2dError> {
                OcrRunRepository::insert_ocr_run(&self.conn, value)
            }

            fn get_ocr_run(
                &self,
                id: &a2d_domain::OcrRunId,
            ) -> Result<Option<a2d_domain::OcrRun>, A2dError> {
                OcrRunRepository::get_ocr_run(&self.conn, id)
            }
        }
    };
}

macro_rules! impl_review_delegate {
    () => {
        impl ReviewItemRepository for Storage {
            fn insert_review_item(&self, value: &a2d_domain::ReviewItem) -> Result<(), A2dError> {
                ReviewItemRepository::insert_review_item(&self.conn, value)
            }

            fn get_review_item(
                &self,
                id: &a2d_domain::ReviewItemId,
            ) -> Result<Option<a2d_domain::ReviewItem>, A2dError> {
                ReviewItemRepository::get_review_item(&self.conn, id)
            }

            fn list_review_items(
                &self,
                query: &ReviewItemQuery,
            ) -> Result<Vec<a2d_domain::ReviewItem>, A2dError> {
                ReviewItemRepository::list_review_items(&self.conn, query)
            }
        }
    };
}

macro_rules! impl_audit_delegate {
    () => {
        impl AuditEventRepository for Storage {
            fn insert_audit_event(&self, value: &a2d_domain::AuditEvent) -> Result<(), A2dError> {
                AuditEventRepository::insert_audit_event(&self.conn, value)
            }

            fn get_audit_event(
                &self,
                id: &a2d_domain::AuditEventId,
            ) -> Result<Option<a2d_domain::AuditEvent>, A2dError> {
                AuditEventRepository::get_audit_event(&self.conn, id)
            }
        }
    };
}

impl_design_delegate!();
impl_notebook_delegate!();
impl_page_set_delegate!();
impl_page_delegate!();
impl_asset_delegate!();
impl_scan_delegate!();
impl_ocr_delegate!();
impl_review_delegate!();
impl_audit_delegate!();

impl fmt::Debug for Storage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Storage")
            .field("schema_version", &self.schema_version())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
