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
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(applied, MIGRATIONS.len() as i64);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn foreign_keys_are_enforced() {
    let storage = Storage::open_in_memory().unwrap();
    let error = storage
        .conn
        .execute(
            "INSERT INTO notebooks (id, design_id, display_name, created_at_ms, \
             updated_at_ms, active_scan_destination) VALUES ('n1', 'missing-design', 'x', 0, 0, 0)",
            [],
        )
        .unwrap_err();
    assert!(error.to_string().to_lowercase().contains("foreign key"));
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
    let error = conn.execute(insert_page, ["p2"]).unwrap_err();
    assert!(error.to_string().to_lowercase().contains("unique"));
}

#[test]
fn unique_smart_page_id_index_rejects_duplicates() {
    let storage = Storage::open_in_memory().unwrap();
    let conn = &storage.conn;
    let insert_page = "INSERT INTO pages (id, kind, smart_page_id, layout_id, state, \
         created_at_ms, updated_at_ms) \
         VALUES (?1, 'smart_page', 'SAME-SMART-PAGE-ID', 'PAGE', 'GeneratedNotScanned', 0, 0)";
    conn.execute(insert_page, ["p1"]).unwrap();
    let error = conn.execute(insert_page, ["p2"]).unwrap_err();
    assert!(error.to_string().to_lowercase().contains("unique"));
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
    let error = Storage::open(&db_path).unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_MIGRATION_IDENTITY_MISMATCH"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_corrupt_enum_column_fails_closed_instead_of_defaulting() {
    use crate::PageRepository;

    let storage = Storage::open_in_memory().unwrap();
    let page_id = a2d_domain::PageId::generate();
    let smart_page_id = a2d_domain::SmartPageId::generate();
    storage
        .conn
        .execute(
            "INSERT INTO pages (id, kind, smart_page_id, layout_id, state, created_at_ms, \
             updated_at_ms) VALUES (?1, 'smart_page', ?2, 'PAGE', 'NotARealState', 0, 0)",
            rusqlite::params![page_id.to_string(), smart_page_id.to_string()],
        )
        .unwrap();

    let error = storage.get_page(&page_id).unwrap_err();
    assert_eq!(error.category, a2d_domain::ErrorCategory::Integrity);
    assert!(error.code.to_string().contains("CORRUPT_ENUM_COLUMN"));
}

#[test]
fn a_second_writer_waits_for_busy_timeout_instead_of_failing_immediately() {
    let dir = std::env::temp_dir().join(format!(
        "a2d-storage-test-{}",
        a2d_domain::PageId::generate()
    ));
    let db_path = dir.join("library.sqlite");
    Storage::open(&db_path).unwrap();

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

    std::thread::sleep(std::time::Duration::from_millis(50));

    let mut writer = Storage::open(&db_path).unwrap();
    let start = std::time::Instant::now();
    writer
        .transaction(|tx| {
            tx.execute(
                "INSERT INTO settings (key, value, updated_at_ms) VALUES ('other', 'v', 0)",
                [],
            )
            .expect("second writer insert must eventually succeed");
            Ok(())
        })
        .unwrap();
    let elapsed = start.elapsed();
    holder.join().unwrap();

    assert!(
        elapsed >= std::time::Duration::from_millis(150),
        "expected the second writer to wait for the lock; elapsed={elapsed:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
