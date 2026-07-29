//! Upgrade regression tests for contradictory preferred-scan state.

use a2d_domain::{AssetId, PageId, ScanId, SmartPageId};
use a2d_storage::{MIGRATIONS, Storage};
use rusqlite::{Connection, params};

fn temp_database() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-preferred-migration-{}.sqlite3",
        PageId::generate()
    ))
}

#[test]
fn migration_rejects_contradictory_legacy_state_without_recording_version_five() {
    let path = temp_database();
    let page_id = PageId::generate();
    let scan_id = ScanId::generate();
    let asset_id = AssetId::generate();
    let smart_page_id = SmartPageId::generate();

    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                applied_at_ms INTEGER NOT NULL,
                sha256 TEXT
            );",
        )
        .unwrap();

        for migration in MIGRATIONS.iter().take(4) {
            conn.execute_batch(migration.sql).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations (version, name, applied_at_ms, sha256)
                 VALUES (?1, ?2, 1, NULL)",
                params![migration.version, migration.name],
            )
            .unwrap();
        }

        conn.execute(
            "INSERT INTO assets (
                id, kind, relative_path, media_type, byte_length, sha256,
                created_at_ms, immutable, encryption_state
             ) VALUES (?1, 'Original', 'assets/original', 'image/jpeg', 1, '00', 100, 1, 'Plaintext')",
            [asset_id.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pages (
                id, kind, notebook_id, notebook_design_id, logical_page_number,
                smart_page_id, page_set_id, visible_page_number, layout_id, title,
                state, preferred_scan_id, generated_pdf_asset_id, created_at_ms, updated_at_ms
             ) VALUES (
                ?1, 'smart_page', NULL, NULL, NULL, ?2, NULL, NULL, 'PAGE-V1', NULL,
                'GeneratedNotScanned', NULL, NULL, 100, 100
             )",
            params![page_id.to_string(), smart_page_id.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scans (
                id, page_id, physical_copy_id, capture_source, captured_at_ms,
                original_asset_id, corrected_asset_id, ocr_asset_id, thumbnail_asset_id,
                pipeline_version, quality_status, warnings, preferred, supersedes_scan_id,
                content_fingerprint
             ) VALUES (
                ?1, ?2, NULL, 'Camera', 200, ?3, NULL, NULL, NULL,
                'v1', 'Accepted', '[]', 0, NULL, 'legacy-fingerprint'
             )",
            params![
                scan_id.to_string(),
                page_id.to_string(),
                asset_id.to_string()
            ],
        )
        .unwrap();

        // Migration 0005 has not run yet, so this deliberately injects the legacy contradiction:
        // one scan is flagged preferred while the owning page has no preferred-scan pointer.
        conn.execute(
            "UPDATE scans SET preferred = 1 WHERE id = ?1",
            [scan_id.to_string()],
        )
        .unwrap();
    }

    let error = Storage::open(&path).unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_CONSTRAINT_VIOLATION");

    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row(
            "SELECT MAX(version) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 4);
    let page_pointer: Option<String> = conn
        .query_row(
            "SELECT preferred_scan_id FROM pages WHERE id = ?1",
            [page_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    let preferred: bool = conn
        .query_row(
            "SELECT preferred FROM scans WHERE id = ?1",
            [scan_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(page_pointer, None);
    assert!(preferred);
    drop(conn);

    std::fs::remove_file(&path).ok();
    std::fs::remove_file(path.with_extension("sqlite3-wal")).ok();
    std::fs::remove_file(path.with_extension("sqlite3-shm")).ok();
}
