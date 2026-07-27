//! Owns the canonical SQLite database. All SQL is private to this crate; no other crate issues SQL.
//!
//! **Crate choice (TODO's open decision):** `rusqlite` with the `bundled` feature — compiles
//! SQLite from source rather than linking a system library, which matters for reproducible
//! cross-compiled Android builds later (Milestone 1.2+) and keeps local dev independent of
//! whatever SQLite version happens to be installed. Chosen over `sqlx` because nothing here
//! needs async; adding a Tokio runtime dependency for a synchronous, single-connection mobile
//! database would be unjustified complexity.
//!
//! **Journaling mode (TODO's open decision, spec §16.2):** `WAL` with `synchronous = NORMAL` —
//! the standard modern pairing, giving concurrent readers during a writer and good crash
//! resilience without `FULL`'s fsync-per-transaction cost. Flagged for revisit once real device
//! measurements exist (spec §29: "exact device-tier thresholds MUST be measured").

use std::fmt;
use std::path::Path;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

mod assets;
mod json_columns;
mod migrations;
mod repository;
mod workflow;

pub use assets::AssetStore;
pub use migrations::{MIGRATIONS, Migration};
pub use repository::{
    AssetRepository, AuditEventRepository, NotebookDesignRepository, NotebookRepository,
    OcrRunRepository, PageRepository, PageSetRepository, ScanRepository,
};
pub use workflow::{NotebookWorkflowRepository, PageLookupRepository};

/// An open library database. Owns a single `rusqlite::Connection` — no connection pool in v1;
/// this crate is not yet responsible for concurrent multi-threaded access (TODO 3.2 will decide
/// how the FFI boundary serializes access, e.g. a `Mutex<Storage>` upstream).
pub struct Storage {
    conn: rusqlite::Connection,
}

fn map_rusqlite_error(context: &str, err: rusqlite::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_SQLITE_ERROR"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.sqlite",
        format!("{context}: {err}"),
        false,
    )
    .with_detail("context", context)
}

fn map_io_error(context: &str, err: std::io::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_IO_ERROR"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.io",
        format!("{context}: {err}"),
        true,
    )
    .with_detail("context", context)
}

/// 5 seconds — a starting default, not a measured threshold (CLAUDE.md: "don't invent
/// thresholds"). Governs how long a writer waits on `SQLITE_BUSY` from a concurrent connection
/// before failing (TODO 3.4: "test foreign-key failures and DB lock/busy handling") rather than
/// failing immediately, which rusqlite's default (0ms) would do.
const BUSY_TIMEOUT_MS: u32 = 5_000;

fn set_busy_timeout(conn: &rusqlite::Connection) -> Result<(), A2dError> {
    let applied: i64 = conn
        .pragma_update_and_check(None, "busy_timeout", BUSY_TIMEOUT_MS, |row| row.get(0))
        .map_err(|e| map_rusqlite_error("setting busy_timeout", e))?;
    if applied != BUSY_TIMEOUT_MS as i64 {
        return Err(A2dError::new(
            ErrorCode::new("STORAGE_BUSY_TIMEOUT_NOT_SET"),
            ErrorCategory::Integrity,
            ErrorSeverity::Critical,
            "error.storage.busy_timeout_not_set",
            format!("SQLite reported busy_timeout={applied}, expected {BUSY_TIMEOUT_MS}"),
            false,
        ));
    }
    Ok(())
}

impl Storage {
    /// Opens (creating if necessary) the SQLite database at `db_path`, verifies pragma
    /// settings, and applies any migrations not yet recorded as applied. A migration failure
    /// leaves the database at its last successfully committed state and returns an error — it
    /// never deletes or recreates the file (TODO 3.1: "Never recreate an empty database after
    /// migration failure").
    pub fn open(db_path: &Path) -> Result<Self, A2dError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| map_io_error("creating the database's parent directory", e))?;
        }

        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| map_rusqlite_error("opening the database", e))?;

        conn.pragma_update(None, "foreign_keys", true)
            .map_err(|e| map_rusqlite_error("enabling foreign_keys", e))?;
        let foreign_keys_on: i64 = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .map_err(|e| map_rusqlite_error("verifying foreign_keys", e))?;
        if foreign_keys_on != 1 {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_FOREIGN_KEYS_NOT_ENFORCED"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.foreign_keys_not_enforced",
                "SQLite did not report foreign_keys=ON after enabling it",
                false,
            ));
        }

        let journal_mode: String = conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .map_err(|e| map_rusqlite_error("setting journal_mode=WAL", e))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_JOURNAL_MODE_NOT_WAL"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.journal_mode_not_wal",
                format!("SQLite reported journal_mode={journal_mode}, expected WAL"),
                false,
            ));
        }

        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| map_rusqlite_error("setting synchronous=NORMAL", e))?;

        set_busy_timeout(&conn)?;

        let mut storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    /// Opens an in-memory database. Test/desktop-tooling use only — an app library is always a
    /// real file.
    pub fn open_in_memory() -> Result<Self, A2dError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| map_rusqlite_error("opening an in-memory database", e))?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(|e| map_rusqlite_error("enabling foreign_keys", e))?;
        set_busy_timeout(&conn)?;
        let mut storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&mut self) -> Result<(), A2dError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL,
                    applied_at_ms INTEGER NOT NULL
                );",
            )
            .map_err(|e| map_rusqlite_error("creating schema_migrations", e))?;

        for migration in MIGRATIONS {
            let recorded_name: Option<String> = self
                .conn
                .query_row(
                    "SELECT name FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| map_rusqlite_error("reading schema_migrations", e))?;

            match recorded_name {
                Some(name) if name == migration.name => continue,
                Some(name) => {
                    return Err(A2dError::new(
                        ErrorCode::new("STORAGE_MIGRATION_IDENTITY_MISMATCH"),
                        ErrorCategory::Integrity,
                        ErrorSeverity::Critical,
                        "error.storage.migration_identity_mismatch",
                        format!(
                            "migration {} is recorded as '{name}' but code names it '{}' -- \
                             migrations MUST be immutable once applied",
                            migration.version, migration.name
                        ),
                        false,
                    ));
                }
                None => self.apply_migration(migration)?,
            }
        }
        Ok(())
    }

    fn apply_migration(&mut self, migration: &Migration) -> Result<(), A2dError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| map_rusqlite_error("starting migration transaction", e))?;
        tx.execute_batch(migration.sql).map_err(|e| {
            map_rusqlite_error(&format!("applying migration {}", migration.version), e)
        })?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![migration.version, migration.name, now_ms()],
        )
        .map_err(|e| map_rusqlite_error("recording migration", e))?;
        tx.commit()
            .map_err(|e| map_rusqlite_error("committing migration", e))?;
        Ok(())
    }

    /// The highest migration version applied so far.
    pub fn schema_version(&self) -> Result<i64, A2dError> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|e| map_rusqlite_error("reading schema_version", e))
    }

    /// Runs `f` inside a transaction, committing only if `f` returns `Ok`. TODO 3.2 requires
    /// transactions for notebook creation, generated page sets, scan registration, OCR
    /// replacement, and restore merge — none of those are single-repository-call operations, so
    /// this is the API each of them composes multiple repository calls through, e.g.:
    ///
    /// Illustrative shape only, not a compiling example (hence `ignore`, not `no_run`) —
    /// `page`/`scan`/`event` would need a full valid `Page`/`Scan`/`AuditEvent`, and `Scan`
    /// alone needs an already-committed immutable `Asset`, which would bloat this into a
    /// multi-page setup rather than a quick illustration of the pattern. The real, fully
    /// constructed version of exactly this composition is exercised for real by
    /// `scan_registration_composes_through_one_transaction_matching_the_todo_example` in
    /// `tests/repository_and_assets.rs`.
    ///
    /// ```ignore
    /// storage.transaction(|tx| {
    ///     tx.insert_page(&page)?;
    ///     tx.insert_scan(&scan)?;
    ///     tx.set_preferred_scan(page.id(), scan.id())?;
    ///     tx.insert_audit_event(&event)?;
    ///     Ok(())
    /// })
    /// ```
    ///
    /// `f` returning `Err` (including a repository call failing) rolls the transaction back —
    /// `rusqlite::Transaction` rolls back on drop unless explicitly committed, so an early `?`
    /// return is enough; nothing here has to reimplement rollback.
    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&rusqlite::Transaction) -> Result<T, A2dError>,
    ) -> Result<T, A2dError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| map_rusqlite_error("starting transaction", e))?;
        let result = f(&tx)?;
        tx.commit()
            .map_err(|e| map_rusqlite_error("committing transaction", e))?;
        Ok(result)
    }
}

macro_rules! delegate_repository {
    ($trait_name:ident { $(fn $method:ident(&self $(, $arg:ident : $arg_ty:ty)*) -> Result<$ret:ty, A2dError>;)+ }) => {
        impl repository::$trait_name for Storage {
            $(
                fn $method(&self $(, $arg: $arg_ty)*) -> Result<$ret, A2dError> {
                    repository::$trait_name::$method(&self.conn $(, $arg)*)
                }
            )+
        }
    };
}

delegate_repository!(NotebookDesignRepository {
    fn insert_notebook_design(&self, design: &a2d_domain::NotebookDesign) -> Result<(), A2dError>;
    fn get_notebook_design(&self, id: &a2d_domain::NotebookDesignId) -> Result<Option<a2d_domain::NotebookDesign>, A2dError>;
});

delegate_repository!(NotebookRepository {
    fn insert_notebook(&self, notebook: &a2d_domain::Notebook) -> Result<(), A2dError>;
    fn get_notebook(&self, id: &a2d_domain::NotebookId) -> Result<Option<a2d_domain::Notebook>, A2dError>;
});

delegate_repository!(PageSetRepository {
    fn insert_page_set(&self, page_set: &a2d_domain::PageSet) -> Result<(), A2dError>;
    fn get_page_set(&self, id: &a2d_domain::PageSetId) -> Result<Option<a2d_domain::PageSet>, A2dError>;
});

delegate_repository!(PageRepository {
    fn insert_page(&self, page: &a2d_domain::Page) -> Result<(), A2dError>;
    fn get_page(&self, id: &a2d_domain::PageId) -> Result<Option<a2d_domain::Page>, A2dError>;
    fn set_preferred_scan(&self, page_id: &a2d_domain::PageId, scan_id: &a2d_domain::ScanId) -> Result<(), A2dError>;
    fn set_generated_pdf_asset(&self, page_id: &a2d_domain::PageId, asset_id: &a2d_domain::AssetId) -> Result<(), A2dError>;
});

delegate_repository!(AssetRepository {
    fn insert_asset(&self, asset: &a2d_domain::Asset) -> Result<(), A2dError>;
    fn get_asset(&self, id: &a2d_domain::AssetId) -> Result<Option<a2d_domain::Asset>, A2dError>;
});

delegate_repository!(ScanRepository {
    fn insert_scan(&self, scan: &a2d_domain::Scan) -> Result<(), A2dError>;
    fn get_scan(&self, id: &a2d_domain::ScanId) -> Result<Option<a2d_domain::Scan>, A2dError>;
});

delegate_repository!(OcrRunRepository {
    fn insert_ocr_run(&self, run: &a2d_domain::OcrRun) -> Result<(), A2dError>;
    fn get_ocr_run(&self, id: &a2d_domain::OcrRunId) -> Result<Option<a2d_domain::OcrRun>, A2dError>;
});

delegate_repository!(AuditEventRepository {
    fn insert_audit_event(&self, event: &a2d_domain::AuditEvent) -> Result<(), A2dError>;
    fn get_audit_event(&self, id: &a2d_domain::AuditEventId) -> Result<Option<a2d_domain::AuditEvent>, A2dError>;
});

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Storage")
            .field("path", &self.conn.path())
            .finish()
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_a_fresh_database_and_applies_migrations() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-storage-test-{}",
            a2d_domain::PageId::generate()
        ));
        let db_path = dir.join("library.sqlite");
        let storage = Storage::open(&db_path).expect("open must succeed for a fresh path");
        assert!(db_path.is_file());
        assert_eq!(
            storage.schema_version().unwrap(),
            MIGRATIONS.last().unwrap().version
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reopening_an_existing_database_does_not_reapply_migrations() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-storage-test-{}",
            a2d_domain::PageId::generate()
        ));
        let db_path = dir.join("library.sqlite");
        {
            let _first = Storage::open(&db_path).unwrap();
        }
        let second = Storage::open(&db_path).expect("reopen must succeed");
        let applied: i64 = second
            .conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, MIGRATIONS.len() as i64);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn table_has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
        let mut statement = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table-info query must prepare");
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("table-info query must run")
            .any(|name| name.expect("column name must decode") == column)
    }

    #[test]
    fn opening_a_partially_migrated_database_applies_only_the_missing_migrations() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-storage-incremental-migration-test-{}",
            a2d_domain::PageId::generate()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("library.sqlite");
        const ORIGINAL_V1_APPLIED_AT: i64 = 123_456_789;

        // Build the exact durable state an installation had when migration 0001 was the newest
        // version: schema_migrations exists, 0001 is committed and recorded, and 0002's column is
        // absent. Tests live in this child module so they can use the real private migration
        // machinery rather than duplicating migration SQL or approximating its transaction rules.
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "foreign_keys", true).unwrap();
            let mut storage = Storage { conn };
            storage
                .conn
                .execute_batch(
                    "CREATE TABLE schema_migrations (\
                         version INTEGER PRIMARY KEY NOT NULL,\
                         name TEXT NOT NULL,\
                         applied_at_ms INTEGER NOT NULL\
                     );",
                )
                .unwrap();
            storage.apply_migration(&MIGRATIONS[0]).unwrap();
            storage
                .conn
                .execute(
                    "UPDATE schema_migrations SET applied_at_ms = ?1 WHERE version = 1",
                    [ORIGINAL_V1_APPLIED_AT],
                )
                .unwrap();
            assert!(!table_has_column(
                &storage.conn,
                "pages",
                "generated_pdf_asset_id"
            ));
        }

        let upgraded = Storage::open(&db_path).expect("partial database must upgrade in place");
        assert_eq!(
            upgraded.schema_version().unwrap(),
            MIGRATIONS.last().unwrap().version
        );
        assert!(table_has_column(
            &upgraded.conn,
            "pages",
            "generated_pdf_asset_id"
        ));

        let rows: Vec<(i64, String, i64)> = {
            let mut statement = upgraded
                .conn
                .prepare(
                    "SELECT version, name, applied_at_ms FROM schema_migrations ORDER BY version",
                )
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], (1, "initial".to_string(), ORIGINAL_V1_APPLIED_AT));
        assert_eq!(rows[1].0, 2);
        assert_eq!(rows[1].1, "page_generated_pdf_asset");
        assert_eq!(rows[2].0, 3);
        assert_eq!(rows[2].1, "milestone6_notebook_workflows");

        // Exercise the new column through the real typed repositories, not merely PRAGMA output.
        let page = a2d_domain::Page::new(
            a2d_domain::PageId::generate(),
            a2d_domain::PageKind::SmartPage {
                smart_page_id: a2d_domain::SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: None,
            },
            a2d_domain::LayoutId::parse("PAGE-V1").unwrap(),
            None,
            a2d_domain::PageState::GeneratedNotScanned,
            500,
        );
        upgraded.insert_page(&page).unwrap();
        let asset_store = AssetStore::open(&dir).unwrap();
        let asset = asset_store
            .commit(
                b"%PDF-1.7 incremental migration fixture",
                a2d_domain::AssetKind::Export,
                "application/pdf",
            )
            .unwrap();
        upgraded.insert_asset(&asset).unwrap();
        upgraded
            .set_generated_pdf_asset(page.id(), asset.id())
            .unwrap();
        let fetched = upgraded.get_page(page.id()).unwrap().unwrap();
        assert_eq!(fetched.generated_pdf_asset_id, Some(asset.id().clone()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let storage = Storage::open_in_memory().unwrap();
        let err = storage
            .conn
            .execute(
                "INSERT INTO notebooks (id, design_id, display_name, created_at_ms, \
                 updated_at_ms, active_scan_destination) VALUES ('n1', 'missing-design', 'x', 0, 0, 0)",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("foreign key"));
    }

    #[test]
    fn unique_notebook_logical_page_index_rejects_duplicates() {
        let storage = Storage::open_in_memory().unwrap();
        let conn = &storage.conn;
        conn.execute(
            "INSERT INTO notebook_designs (id, schema_version, name, design_version, \
             trim_width_mm, trim_height_mm, logical_page_count, setup_layout_id, page_layout_id, \
             marker_family, marker_role_ids, manifest_hash, trust_state) \
             VALUES ('d1', 1, 'Design', 1, 210, 297, 100, 'SETUP', 'PAGE', 'apriltag', '[]', 'hash', 'Unverified')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notebooks (id, design_id, display_name, created_at_ms, updated_at_ms, \
             active_scan_destination) VALUES ('n1', 'd1', 'Notebook', 0, 0, 0)",
            [],
        )
        .unwrap();
        let insert_page = "INSERT INTO pages (id, kind, notebook_id, notebook_design_id, \
             logical_page_number, layout_id, state, created_at_ms, updated_at_ms) \
             VALUES (?1, 'notebook_page', 'n1', 'd1', 1, 'PAGE', 'GeneratedNotScanned', 0, 0)";
        conn.execute(insert_page, ["p1"]).unwrap();
        let err = conn.execute(insert_page, ["p2"]).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unique"));
    }

    #[test]
    fn unique_smart_page_id_index_rejects_duplicates() {
        // Closes the gap TODO 2.3 deferred to storage: a2d-domain's Page type can require a
        // Page::SmartPage to carry a SmartPageId, but only this index can enforce it's unique.
        let storage = Storage::open_in_memory().unwrap();
        let conn = &storage.conn;
        let insert_page = "INSERT INTO pages (id, kind, smart_page_id, layout_id, state, \
             created_at_ms, updated_at_ms) \
             VALUES (?1, 'smart_page', 'SAME-SMART-PAGE-ID', 'PAGE', 'GeneratedNotScanned', 0, 0)";
        conn.execute(insert_page, ["p1"]).unwrap();
        let err = conn.execute(insert_page, ["p2"]).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unique"));
    }

    #[test]
    fn a_tampered_migration_name_is_detected_as_an_integrity_error() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-storage-test-{}",
            a2d_domain::PageId::generate()
        ));
        let db_path = dir.join("library.sqlite");
        {
            let storage = Storage::open(&db_path).unwrap();
            storage
                .conn
                .execute(
                    "UPDATE schema_migrations SET name = 'tampered' WHERE version = 1",
                    [],
                )
                .unwrap();
        }
        let err = Storage::open(&db_path).unwrap_err();
        assert!(err.code.to_string().contains("MIGRATION_IDENTITY_MISMATCH"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_enum_column_fails_closed_instead_of_defaulting() {
        use crate::PageRepository;

        let storage = Storage::open_in_memory().unwrap();
        let page_id = a2d_domain::PageId::generate();
        let smart_page_id = a2d_domain::SmartPageId::generate();
        // Bypass the repository layer entirely to write a `state` value its enum mapper doesn't
        // recognize -- simulates the kind of corruption a disk fault or a bug in a future writer
        // could cause. A real Page can never carry this value; a raw statement can.
        storage
            .conn
            .execute(
                "INSERT INTO pages (id, kind, smart_page_id, layout_id, state, created_at_ms, \
                 updated_at_ms) VALUES (?1, 'smart_page', ?2, 'PAGE', 'NotARealState', 0, 0)",
                rusqlite::params![page_id.to_string(), smart_page_id.to_string()],
            )
            .unwrap();

        let err = storage.get_page(&page_id).unwrap_err();
        assert_eq!(err.category, a2d_domain::ErrorCategory::Integrity);
        assert!(err.code.to_string().contains("CORRUPT_ENUM_COLUMN"));
    }

    #[test]
    fn a_second_writer_waits_for_busy_timeout_instead_of_failing_immediately() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-storage-test-{}",
            a2d_domain::PageId::generate()
        ));
        let db_path = dir.join("library.sqlite");
        {
            // Ensure the schema/file exist before the two threads race to open it.
            Storage::open(&db_path).unwrap();
        }

        let holder_path = db_path.clone();
        let holder = std::thread::spawn(move || {
            let mut storage = Storage::open(&holder_path).unwrap();
            storage
                .transaction(|tx| {
                    tx.execute(
                        "INSERT INTO settings (key, value, updated_at_ms) VALUES ('holder', 'v', 0)",
                        [],
                    )
                    .expect("holder insert must succeed");
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    Ok(())
                })
                .unwrap();
        });

        // Give the holder thread time to acquire the write lock before this thread tries too.
        std::thread::sleep(std::time::Duration::from_millis(50));

        let mut writer = Storage::open(&db_path).unwrap();
        let start = std::time::Instant::now();
        writer
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO settings (key, value, updated_at_ms) VALUES ('other', 'v', 0)",
                    [],
                )
                .expect("second writer insert must eventually succeed, not fail immediately");
                Ok(())
            })
            .unwrap();
        let elapsed = start.elapsed();
        holder.join().unwrap();

        assert!(
            elapsed >= std::time::Duration::from_millis(150),
            "expected the second writer to block waiting for the lock rather than fail \
             immediately with SQLITE_BUSY; elapsed={elapsed:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
