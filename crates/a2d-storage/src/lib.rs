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

mod migrations;

pub use migrations::{MIGRATIONS, Migration};

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
}

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
}
