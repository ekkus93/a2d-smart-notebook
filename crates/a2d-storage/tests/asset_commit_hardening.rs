//! Regression coverage for durable, no-replace asset finalization and canonical path resolution.

use a2d_domain::{AssetId, AssetKind, PageId};
use a2d_storage::AssetStore;

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-asset-hardening-{label}-{}",
        PageId::generate()
    ))
}

#[test]
fn resolve_returns_the_canonical_file_path() {
    let root = temp_dir("canonical");
    let store = AssetStore::open(&root).unwrap();
    let asset = store
        .commit(b"canonical bytes", AssetKind::Corrected, "image/png")
        .unwrap();

    let resolved = store.resolve(&asset.relative_path).unwrap();
    assert_eq!(resolved, resolved.canonicalize().unwrap());
    assert!(resolved.starts_with(root.canonicalize().unwrap()));
    assert_eq!(std::fs::read(resolved).unwrap(), b"canonical bytes");

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn resolve_reports_a_missing_asset_with_the_dedicated_code() {
    let root = temp_dir("missing");
    let store = AssetStore::open(&root).unwrap();

    let error = store
        .resolve("assets/corrected/00000000000000000000000000")
        .unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_ASSET_MISSING");

    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn resolve_rejects_a_symlink_even_when_its_target_is_inside_the_library() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("symlink");
    let store = AssetStore::open(&root).unwrap();
    let asset = store
        .commit(b"real bytes", AssetKind::Corrected, "image/png")
        .unwrap();
    let real = store.resolve(&asset.relative_path).unwrap();
    let link_relative = "assets/corrected/symlinked-asset";
    let link = root.join(link_relative);
    symlink(real, &link).unwrap();

    let error = store.resolve(link_relative).unwrap_err();
    assert_eq!(error.code.to_string(), "STORAGE_ASSET_PATH_IS_SYMLINK");

    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "test-util")]
#[test]
fn final_path_collision_never_replaces_existing_asset_bytes() {
    let root = temp_dir("final-collision");
    let store = AssetStore::open(&root).unwrap();
    let id = AssetId::parse("00000000000000000000000000").unwrap();

    let first = store
        .commit_with_id_for_test(
            id.clone(),
            b"first immutable content",
            AssetKind::Corrected,
            "image/png",
        )
        .unwrap();
    let final_path = store.resolve(&first.relative_path).unwrap();

    let error = store
        .commit_with_id_for_test(
            id.clone(),
            b"replacement content",
            AssetKind::Corrected,
            "image/png",
        )
        .unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_ASSET_FINAL_PATH_COLLISION"
    );
    assert_eq!(std::fs::read(&final_path).unwrap(), b"first immutable content");
    assert_eq!(
        error.details.get("asset_id").map(String::as_str),
        Some(id.as_str())
    );
    assert_eq!(
        error
            .details
            .get("temp_cleanup_completed")
            .map(String::as_str),
        Some("true")
    );
    assert!(store.list_orphaned_temp_files().unwrap().is_empty());

    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "test-util")]
#[test]
fn temp_path_collision_preserves_the_existing_temp_file() {
    let root = temp_dir("temp-collision");
    let store = AssetStore::open(&root).unwrap();
    let id = AssetId::parse("00000000000000000000000001").unwrap();
    let temp_path = root.join("tmp").join(format!("{id}.tmp"));
    std::fs::write(&temp_path, b"pre-existing recovery data").unwrap();

    let error = store
        .commit_with_id_for_test(
            id.clone(),
            b"new bytes",
            AssetKind::Thumbnail,
            "image/png",
        )
        .unwrap_err();
    assert_eq!(
        error.code.to_string(),
        "STORAGE_ASSET_TEMP_PATH_COLLISION"
    );
    assert_eq!(
        std::fs::read(&temp_path).unwrap(),
        b"pre-existing recovery data"
    );
    assert_eq!(
        error.details.get("asset_id").map(String::as_str),
        Some(id.as_str())
    );

    std::fs::remove_dir_all(root).ok();
}
