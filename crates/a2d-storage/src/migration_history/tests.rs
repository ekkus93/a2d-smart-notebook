use super::*;

fn connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    conn
}

fn assert_recorded_hash(row: &AppliedMigration, migration: &Migration) {
    let expected = migration_sha256(migration);
    assert_eq!(row.sha256.as_deref(), Some(expected.as_str()));
}

#[test]
fn fresh_database_records_every_exact_sql_digest() {
    let mut conn = connection();
    migrate(&mut conn).unwrap();

    let rows = load_applied(&conn, true).unwrap();
    assert_eq!(rows.len(), MIGRATIONS.len());
    for (row, migration) in rows.iter().zip(MIGRATIONS) {
        assert_eq!(row.version, migration.version);
        assert_eq!(row.name, migration.name);
        assert_recorded_hash(row, migration);
    }
}

#[test]
fn legacy_tracking_table_is_validated_then_backfilled() {
    let mut conn = connection();
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at_ms INTEGER NOT NULL
        );",
    )
    .unwrap();
    conn.execute_batch(MIGRATIONS[0].sql).unwrap();
    conn.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (1, ?1, 123)",
        [MIGRATIONS[0].name],
    )
    .unwrap();

    migrate(&mut conn).unwrap();
    let rows = load_applied(&conn, true).unwrap();
    assert_eq!(rows.len(), MIGRATIONS.len());
    assert_recorded_hash(&rows[0], &MIGRATIONS[0]);
    let original_time: i64 = conn
        .query_row(
            "SELECT applied_at_ms FROM schema_migrations WHERE version = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(original_time, 123);
}

#[test]
fn digest_tampering_is_rejected() {
    let mut conn = connection();
    migrate(&mut conn).unwrap();
    conn.execute(
        "UPDATE schema_migrations SET sha256 = ?1 WHERE version = 1",
        ["0".repeat(64)],
    )
    .unwrap();

    let error = migrate(&mut conn).unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_MIGRATION_HASH_MISMATCH");
}

#[test]
fn a_future_database_version_is_rejected_before_pending_work() {
    let mut conn = connection();
    create_tracking_table(&conn).unwrap();
    let future = MIGRATIONS.last().unwrap().version + 1;
    conn.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms, sha256) \
         VALUES (?1, 'future', 1, ?2)",
        params![future, "0".repeat(64)],
    )
    .unwrap();

    let error = migrate(&mut conn).unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_DATABASE_SCHEMA_NEWER_THAN_APP"
    );
}

#[test]
fn a_history_gap_is_rejected() {
    let mut conn = connection();
    create_tracking_table(&conn).unwrap();
    for migration in [&MIGRATIONS[0], &MIGRATIONS[2]] {
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms, sha256) \
             VALUES (?1, ?2, 1, ?3)",
            params![
                migration.version,
                migration.name,
                migration_sha256(migration),
            ],
        )
        .unwrap();
    }

    let error = migrate(&mut conn).unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_MIGRATION_HISTORY_GAP");
}

#[test]
fn a_name_mismatch_is_rejected_before_hash_backfill() {
    let mut conn = connection();
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at_ms INTEGER NOT NULL
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms) \
         VALUES (1, 'tampered', 1)",
        [],
    )
    .unwrap();

    let error = migrate(&mut conn).unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_MIGRATION_IDENTITY_MISMATCH"
    );
    assert!(!tracking_columns(&conn).unwrap().contains(HASH_COLUMN));
}

#[test]
fn migration_sql_and_tracking_row_are_atomic() {
    let mut conn = connection();
    create_tracking_table(&conn).unwrap();
    let broken = Migration {
        version: 1,
        name: "broken",
        sql: "CREATE TABLE should_roll_back (id INTEGER); THIS IS NOT SQL;",
    };

    assert!(apply_one(&mut conn, &broken).is_err());
    assert!(!table_named(&conn, "should_roll_back"));
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

fn table_named(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .unwrap()
    .is_some()
}
