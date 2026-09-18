use std::{fs, path::PathBuf};

use a2d_domain::OcrJobId;
use a2d_storage::{MIGRATIONS, OcrJobRepository, Storage};
use rusqlite::{Connection, params};

fn database_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-ocr-r7-migration-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn create_version_11_database(path: &PathBuf, attempt_count: u32) {
    let mut conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE schema_migrations (
             version INTEGER PRIMARY KEY,
             name TEXT NOT NULL,
             applied_at_ms INTEGER NOT NULL,
             sha256 TEXT
         );",
    )
    .unwrap();

    for migration in MIGRATIONS.iter().take(11) {
        conn.execute_batch(migration.sql).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms, sha256)
             VALUES (?1, ?2, 1, NULL)",
            params![migration.version, migration.name],
        )
        .unwrap();
    }

    conn.execute(
        "INSERT INTO assets
         (id, kind, relative_path, media_type, byte_length, sha256, created_at_ms, immutable, encryption_state)
         VALUES ('asset-r7', 'Original', 'assets/originals/r7.png', 'image/png', 4, 'r7-sha', 10, 1, 'Plaintext')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO pages
         (id, kind, layout_id, title, state, created_at_ms, updated_at_ms)
         VALUES ('page-r7', 'SmartPage', 'PAGE', 'R7 migration', 'GeneratedNotScanned', 10, 10)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scans
         (id, page_id, capture_source, captured_at_ms, original_asset_id, pipeline_version,
          quality_status, warnings, preferred, content_fingerprint)
         VALUES ('scan-r7', 'page-r7', 'Camera', 20, 'asset-r7', 'r7-test',
                 'Accepted', '[]', 0, 'r7-fingerprint')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ocr_jobs
         (id, scan_id, input_asset_id, input_kind, media_type, relative_path, byte_length,
          width_px, height_px, status, created_at_ms, updated_at_ms, attempt_count, retryable,
          provider_availability, cancellation_requested)
         VALUES ('ocrjob_r7legacy', 'scan-r7', 'asset-r7', 'Original', 'image/png',
                 'assets/originals/r7.png', 4, 100, 200, 'Queued', 30, 30, ?1, 1, 'Unknown', 0)",
        [i64::from(attempt_count)],
    )
    .unwrap();
}

#[test]
fn version_11_database_upgrades_forward_without_rewriting_migration_0011() {
    assert!(MIGRATIONS[10].sql.contains("attempt_count <= 25"));
    assert_eq!(MIGRATIONS[11].version, 12);
    assert!(!MIGRATIONS[11].sql.contains("attempt_count <= 25"));

    let path = database_path();
    create_version_11_database(&path, 3);

    let storage = Storage::open(&path).unwrap();
    let job = storage
        .get_ocr_job(&OcrJobId::parse("ocrjob_r7legacy").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(job.attempt_count, 3);

    drop(storage);
    fs::remove_file(path).ok();
}

#[test]
fn legacy_row_above_canonical_retry_limit_survives_schema_upgrade_for_core_resolution() {
    let path = database_path();
    create_version_11_database(&path, 25);

    let storage = Storage::open(&path).unwrap();
    let job = storage
        .get_ocr_job(&OcrJobId::parse("ocrjob_r7legacy").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(job.attempt_count, 25);

    drop(storage);
    fs::remove_file(path).ok();
}
