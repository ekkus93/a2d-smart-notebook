//! Immutable migration-history validation and application.
//!
//! Every applied migration is identified by its version, name, and SHA-256 of the exact compiled
//! SQL bytes. A database with a future version, a history gap, a renamed migration, or a digest
//! mismatch is rejected before any pending migration runs. Databases created before digest storage
//! are upgraded by validating version/name continuity first and then sealing each existing row with
//! the digest from the current reviewed build. That legacy backfill cannot retroactively prove what
//! SQL an older build executed, but it prevents subsequent undetected edits.

use std::collections::BTreeSet;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, system_now_ms};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::map_rusqlite_error;
use crate::migrations::{MIGRATIONS, Migration};

const TRACKING_TABLE: &str = "schema_migrations";
const HASH_COLUMN: &str = "sha256";

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppliedMigration {
    version: i64,
    name: String,
    sha256: Option<String>,
}

pub(crate) fn migrate(conn: &mut Connection) -> Result<(), A2dError> {
    validate_compiled_catalog()?;

    if !tracking_table_exists(conn)? {
        create_tracking_table(conn)?;
    }

    let columns = tracking_columns(conn)?;
    validate_tracking_columns(&columns)?;
    let has_hash_column = columns.contains(HASH_COLUMN);
    let applied = load_applied(conn, has_hash_column)?;
    validate_applied_history(&applied, has_hash_column)?;

    if !has_hash_column {
        add_hash_column(conn)?;
    }
    backfill_missing_hashes(conn, &applied)?;

    for migration in MIGRATIONS.iter().skip(applied.len()) {
        apply_one(conn, migration)?;
    }

    // Re-read and verify the final durable state rather than trusting successful statements alone.
    let final_columns = tracking_columns(conn)?;
    if !final_columns.contains(HASH_COLUMN) {
        return Err(migration_integrity_error(
            "STORAGE_MIGRATION_HASH_COLUMN_MISSING",
            "schema_migrations does not contain the required SHA-256 column after migration",
        ));
    }
    let final_rows = load_applied(conn, true)?;
    validate_applied_history(&final_rows, true)?;
    if final_rows.len() != MIGRATIONS.len() {
        return Err(migration_integrity_error(
            "STORAGE_MIGRATION_HISTORY_INCOMPLETE",
            format!(
                "database records {} migrations but this build requires {}",
                final_rows.len(),
                MIGRATIONS.len(),
            ),
        ));
    }
    Ok(())
}

fn validate_compiled_catalog() -> Result<(), A2dError> {
    if MIGRATIONS.is_empty() {
        return Err(migration_integrity_error(
            "STORAGE_MIGRATION_CATALOG_EMPTY",
            "the compiled migration catalog is empty",
        ));
    }
    for (index, migration) in MIGRATIONS.iter().enumerate() {
        let expected = i64::try_from(index + 1).map_err(|_| {
            migration_integrity_error(
                "STORAGE_MIGRATION_CATALOG_OVERFLOW",
                "compiled migration count exceeds the portable version representation",
            )
        })?;
        if migration.version != expected {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_CATALOG_GAP",
                format!(
                    "compiled migration at index {index} has version {}, expected {expected}",
                    migration.version,
                ),
            )
            .with_detail("expected_version", expected.to_string())
            .with_detail("actual_version", migration.version.to_string()));
        }
        if migration.name.trim().is_empty() || migration.name != migration.name.trim() {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_CATALOG_NAME_INVALID",
                format!("compiled migration {expected} has an empty or noncanonical name"),
            )
            .with_detail("version", expected.to_string()));
        }
        if migration.sql.trim().is_empty() {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_CATALOG_SQL_EMPTY",
                format!("compiled migration {expected} has no SQL"),
            )
            .with_detail("version", expected.to_string()));
        }
    }
    Ok(())
}

fn tracking_table_exists(conn: &Connection) -> Result<bool, A2dError> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [TRACKING_TABLE],
        |_| Ok(()),
    )
    .optional()
    .map(|row| row.is_some())
    .map_err(|error| map_rusqlite_error("checking schema_migrations existence", error))
}

fn create_tracking_table(conn: &Connection) -> Result<(), A2dError> {
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at_ms INTEGER NOT NULL,
            sha256 TEXT
        );",
    )
    .map_err(|error| map_rusqlite_error("creating schema_migrations", error))
}

fn tracking_columns(conn: &Connection) -> Result<BTreeSet<String>, A2dError> {
    let mut statement = conn
        .prepare("PRAGMA table_info(schema_migrations)")
        .map_err(|error| map_rusqlite_error("preparing schema_migrations table info", error))?;
    statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| map_rusqlite_error("reading schema_migrations table info", error))?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|error| map_rusqlite_error("decoding schema_migrations table info", error))
}

fn validate_tracking_columns(columns: &BTreeSet<String>) -> Result<(), A2dError> {
    for required in ["version", "name", "applied_at_ms"] {
        if !columns.contains(required) {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_TRACKING_SCHEMA_INVALID",
                format!("schema_migrations is missing required column `{required}`"),
            )
            .with_detail("missing_column", required));
        }
    }
    Ok(())
}

fn add_hash_column(conn: &Connection) -> Result<(), A2dError> {
    conn.execute_batch("ALTER TABLE schema_migrations ADD COLUMN sha256 TEXT;")
        .map_err(|error| map_rusqlite_error("adding migration SHA-256 column", error))
}

fn load_applied(
    conn: &Connection,
    has_hash_column: bool,
) -> Result<Vec<AppliedMigration>, A2dError> {
    let sql = if has_hash_column {
        "SELECT version, name, sha256 FROM schema_migrations ORDER BY version"
    } else {
        "SELECT version, name, NULL FROM schema_migrations ORDER BY version"
    };
    let mut statement = conn
        .prepare(sql)
        .map_err(|error| map_rusqlite_error("preparing migration history query", error))?;
    statement
        .query_map([], |row| {
            Ok(AppliedMigration {
                version: row.get(0)?,
                name: row.get(1)?,
                sha256: row.get(2)?,
            })
        })
        .map_err(|error| map_rusqlite_error("reading migration history", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_rusqlite_error("decoding migration history", error))
}

fn validate_applied_history(
    applied: &[AppliedMigration],
    hash_column_exists: bool,
) -> Result<(), A2dError> {
    let current_version = MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .ok_or_else(|| {
            migration_integrity_error(
                "STORAGE_MIGRATION_CATALOG_EMPTY",
                "the compiled migration catalog is empty",
            )
        })?;

    for (index, row) in applied.iter().enumerate() {
        if row.version > current_version {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_DATABASE_SCHEMA_NEWER_THAN_APP"),
                ErrorCategory::Migration,
                ErrorSeverity::Critical,
                "error.storage.database_newer_than_app",
                format!(
                    "database contains migration version {} but this build supports only through {current_version}",
                    row.version,
                ),
                false,
            )
            .with_detail("database_version", row.version.to_string())
            .with_detail("supported_version", current_version.to_string()));
        }

        let expected_version = i64::try_from(index + 1).map_err(|_| {
            migration_integrity_error(
                "STORAGE_MIGRATION_HISTORY_OVERFLOW",
                "migration history length exceeds the portable version representation",
            )
        })?;
        if row.version != expected_version {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_HISTORY_GAP",
                format!(
                    "migration history expected version {expected_version} but found {}",
                    row.version,
                ),
            )
            .with_detail("expected_version", expected_version.to_string())
            .with_detail("actual_version", row.version.to_string()));
        }

        let migration = MIGRATIONS.get(index).ok_or_else(|| {
            migration_integrity_error(
                "STORAGE_DATABASE_SCHEMA_NEWER_THAN_APP",
                "database migration history exceeds the compiled catalog",
            )
        })?;
        if row.name != migration.name {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_IDENTITY_MISMATCH",
                format!(
                    "migration {} is recorded as '{}' but this build names it '{}'",
                    row.version, row.name, migration.name,
                ),
            )
            .with_detail("version", row.version.to_string())
            .with_detail("recorded_name", &row.name)
            .with_detail("compiled_name", migration.name));
        }

        if hash_column_exists && let Some(recorded_hash) = row.sha256.as_deref() {
            let expected_hash = migration_sha256(migration);
            if recorded_hash != expected_hash {
                return Err(migration_integrity_error(
                    "STORAGE_MIGRATION_HASH_MISMATCH",
                    format!(
                        "migration {} SQL digest differs from the immutable recorded digest",
                        row.version,
                    ),
                )
                .with_detail("version", row.version.to_string())
                .with_detail("recorded_sha256", recorded_hash)
                .with_detail("compiled_sha256", expected_hash));
            }
        }
    }
    Ok(())
}

fn backfill_missing_hashes(
    conn: &mut Connection,
    previously_applied: &[AppliedMigration],
) -> Result<(), A2dError> {
    let missing_versions = previously_applied
        .iter()
        .filter(|row| row.sha256.is_none())
        .map(|row| row.version)
        .collect::<Vec<_>>();
    if missing_versions.is_empty() {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(|error| map_rusqlite_error("starting migration hash backfill", error))?;
    for version in missing_versions {
        let index = usize::try_from(version - 1).map_err(|_| {
            migration_integrity_error(
                "STORAGE_MIGRATION_VERSION_INVALID",
                "migration version cannot be represented as a catalog index",
            )
            .with_detail("version", version.to_string())
        })?;
        let migration = MIGRATIONS.get(index).ok_or_else(|| {
            migration_integrity_error(
                "STORAGE_DATABASE_SCHEMA_NEWER_THAN_APP",
                "migration hash backfill references an unsupported version",
            )
            .with_detail("version", version.to_string())
        })?;
        let changed = tx
            .execute(
                "UPDATE schema_migrations SET sha256 = ?1 WHERE version = ?2 AND sha256 IS NULL",
                params![migration_sha256(migration), version],
            )
            .map_err(|error| map_rusqlite_error("backfilling migration SHA-256", error))?;
        if changed != 1 {
            return Err(migration_integrity_error(
                "STORAGE_MIGRATION_HASH_BACKFILL_RACE",
                "migration hash row changed unexpectedly during backfill",
            )
            .with_detail("version", version.to_string())
            .with_detail("changed_rows", changed.to_string()));
        }
    }
    tx.commit()
        .map_err(|error| map_rusqlite_error("committing migration hash backfill", error))
}

fn apply_one(conn: &mut Connection, migration: &Migration) -> Result<(), A2dError> {
    // Clock failure occurs inside the transaction and therefore cannot leave migration SQL or a
    // tracking row committed independently.
    let tx = conn
        .transaction()
        .map_err(|error| map_rusqlite_error("starting migration transaction", error))?;
    let applied_at_ms = system_now_ms()?;
    tx.execute_batch(migration.sql).map_err(|error| {
        map_rusqlite_error(&format!("applying migration {}", migration.version), error)
    })?;
    tx.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms, sha256) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            migration.version,
            migration.name,
            applied_at_ms,
            migration_sha256(migration),
        ],
    )
    .map_err(|error| map_rusqlite_error("recording migration", error))?;
    tx.commit()
        .map_err(|error| map_rusqlite_error("committing migration", error))
}

fn migration_sha256(migration: &Migration) -> String {
    let mut hasher = Sha256::new();
    hasher.update(migration.sql.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn migration_integrity_error(
    code: &'static str,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.migration_integrity",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests;
