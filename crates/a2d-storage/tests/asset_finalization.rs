#![cfg(feature = "test-util")]

use a2d_domain::{AssetId, AssetKind, ErrorCategory, ErrorSeverity};
use a2d_storage::{AssetRepository, AssetStore, Storage};

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-asset-finalization-{label}-{}",
        AssetId::generate()
    ))
}

fn final_path(root: &std::path::Path, kind: &str, id: &AssetId) -> std::path::PathBuf {
    root.join("assets").join(kind).join(id.to_string())
}

#[test]
fn an_existing_destination_is_never_overwritten_and_reports_a_critical_collision() {
    let root = temp_dir("collision");
    let store = AssetStore::open(&root).unwrap();
    let id = AssetId::generate();

    let first = store
        .commit_with_id_for_test(
            id.clone(),
            b"authoritative first bytes",
            AssetKind::Corrected,
            "image/png",
        )
        .unwrap();
    let destination = root.join(&first.relative_path);
    let original_bytes = std::fs::read(&destination).unwrap();
    let expected_id = id.to_string();
    let expected_destination = destination.to_string_lossy().into_owned();

    let error = store
        .commit_with_id_for_test(
            id.clone(),
            b"different collision bytes",
            AssetKind::Corrected,
            "image/png",
        )
        .unwrap_err();

    assert_eq!(error.code.to_string(), "STORAGE_ASSET_FINAL_PATH_COLLISION");
    assert_eq!(error.category, ErrorCategory::Integrity);
    assert_eq!(error.severity, ErrorSeverity::Critical);
    assert_eq!(error.details.get("asset_id"), Some(&expected_id));
    assert_eq!(error.details.get("final_path"), Some(&expected_destination));
    assert_eq!(std::fs::read(&destination).unwrap(), original_bytes);
    assert!(!root.join("tmp").join(format!("{id}.tmp")).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_forced_file_sync_failure_returns_before_finalization_and_creates_no_database_row() {
    let root = temp_dir("file-sync-failure");
    let store = AssetStore::open(&root).unwrap();
    let storage = Storage::open_in_memory().unwrap();
    let id = AssetId::generate();

    let error = store
        .commit_with_file_sync_failure_for_test(
            id.clone(),
            b"bytes that must not finalize",
            AssetKind::Corrected,
            "image/png",
        )
        .unwrap_err();

    assert_eq!(error.code.to_string(), "STORAGE_ASSET_FILE_SYNC_FAILED");
    assert_eq!(
        error
            .details
            .get("asset_commit_failure_stage")
            .map(String::as_str),
        Some("before_finalization")
    );
    assert_eq!(
        error.details.get("file_sync_completed").map(String::as_str),
        Some("false")
    );
    assert_eq!(storage.get_asset(&id).unwrap(), None);
    assert!(!final_path(&root, "corrected", &id).exists());
    assert!(!root.join("tmp").join(format!("{id}.tmp")).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_forced_destination_directory_sync_failure_reports_a_finalized_unregistered_asset() {
    let root = temp_dir("destination-directory-sync-failure");
    let store = AssetStore::open(&root).unwrap();
    let id = AssetId::generate();
    let expected = b"synchronized file awaiting directory sync";

    let error = store
        .commit_with_destination_directory_sync_failure_for_test(
            id.clone(),
            expected,
            AssetKind::Thumbnail,
            "image/png",
        )
        .unwrap_err();

    let destination = final_path(&root, "thumbnails", &id);
    assert_eq!(
        error.code.to_string(),
        "STORAGE_ASSET_DESTINATION_DIRECTORY_SYNC_FAILED"
    );
    assert_eq!(
        error
            .details
            .get("asset_commit_failure_stage")
            .map(String::as_str),
        Some("finalized_unregistered")
    );
    assert_eq!(
        error.details.get("file_sync_completed").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        error
            .details
            .get("directory_sync_completed")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(std::fs::read(&destination).unwrap(), expected);
    assert!(root.join("tmp").join(format!("{id}.tmp")).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_forced_temp_directory_sync_failure_preserves_the_final_asset_and_reports_cleanup_state() {
    let root = temp_dir("temp-directory-sync-failure");
    let store = AssetStore::open(&root).unwrap();
    let id = AssetId::generate();
    let expected = b"finalized bytes after temp unlink";

    let error = store
        .commit_with_temp_directory_sync_failure_for_test(
            id.clone(),
            expected,
            AssetKind::Export,
            "application/octet-stream",
        )
        .unwrap_err();

    assert_eq!(
        error.code.to_string(),
        "STORAGE_ASSET_TEMP_DIRECTORY_SYNC_FAILED"
    );
    assert_eq!(
        error
            .details
            .get("asset_commit_failure_stage")
            .map(String::as_str),
        Some("finalized_unregistered")
    );
    assert_eq!(
        error
            .details
            .get("temp_cleanup_completed")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        error
            .details
            .get("temp_directory_sync_completed")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        std::fs::read(final_path(&root, "exports", &id)).unwrap(),
        expected
    );
    assert!(!root.join("tmp").join(format!("{id}.tmp")).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_forced_permission_failure_reports_the_asset_id_and_planned_final_path() {
    let root = temp_dir("permission-failure");
    let store = AssetStore::open(&root).unwrap();
    let id = AssetId::generate();
    let destination = final_path(&root, "originals", &id);
    let expected_id = id.to_string();
    let expected_destination = destination.to_string_lossy().into_owned();
    let expected_relative_path = format!("assets/originals/{id}");

    let error = store
        .commit_with_permission_failure_for_test(
            id.clone(),
            b"immutable original",
            AssetKind::Original,
            "image/jpeg",
        )
        .unwrap_err();

    assert_eq!(
        error.code.to_string(),
        "STORAGE_ASSET_PERMISSION_SET_FAILED"
    );
    assert_eq!(error.details.get("asset_id"), Some(&expected_id));
    assert_eq!(error.details.get("final_path"), Some(&expected_destination));
    assert_eq!(
        error.details.get("final_relative_path"),
        Some(&expected_relative_path)
    );
    assert!(!destination.exists());
    assert!(!root.join("tmp").join(format!("{id}.tmp")).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn successful_original_commit_meets_hash_file_and_read_only_postconditions() {
    let root = temp_dir("success-postconditions");
    let store = AssetStore::open(&root).unwrap();
    let bytes = b"verified original bytes";

    let asset = store
        .commit(bytes, AssetKind::Original, "image/jpeg")
        .unwrap();
    let path = store.resolve(&asset.relative_path).unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), bytes.len() as u64);
    assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
    store.verify(&asset).unwrap();
    assert!(store.list_orphaned_temp_files().unwrap().is_empty());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn platform_adapter_is_explicit_and_never_falls_back_to_rename() {
    let source = include_str!("../src/asset_platform.rs");

    assert!(source.contains("target_os = \"android\""));
    assert!(source.contains("target_os = \"linux\""));
    assert!(source.contains("ErrorKind::Unsupported"));
    assert!(source.contains("std::fs::hard_link(temp_path, final_path)"));
    assert!(!source.contains("std::fs::rename"));
}
