//! FIX-024 regression coverage for canonical asset path resolution.

use a2d_domain::PageId;
use a2d_storage::AssetStore;

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-asset-path-resolution-{label}-{}",
        PageId::generate()
    ))
}

#[test]
fn resolve_rejects_relative_traversal_to_an_existing_regular_file() {
    let root = temp_dir("traversal-root");
    let store = AssetStore::open(&root).unwrap();
    let outside = root.parent().unwrap().join(format!(
        "a2d-asset-path-resolution-outside-{}",
        PageId::generate()
    ));
    std::fs::write(&outside, b"outside bytes must remain untouched").unwrap();
    let outside_name = outside
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let relative_path = format!("../{outside_name}");

    let error = store.resolve(&relative_path).unwrap_err();

    assert_eq!(error.code.to_string(), "STORAGE_ASSET_PATH_ESCAPES_ROOT");
    assert_eq!(
        error.details.get("relative_path").map(String::as_str),
        Some(relative_path.as_str()),
    );
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        b"outside bytes must remain untouched",
    );

    std::fs::remove_file(outside).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resolve_rejects_a_directory_where_an_asset_file_is_required() {
    let root = temp_dir("directory");
    let store = AssetStore::open(&root).unwrap();
    let relative_path = "assets/corrected/not-a-file";
    std::fs::create_dir_all(root.join(relative_path)).unwrap();

    let error = store.resolve(relative_path).unwrap_err();

    assert_eq!(error.code.to_string(), "STORAGE_ASSET_PATH_NOT_FILE");
    assert_eq!(
        error.details.get("relative_path").map(String::as_str),
        Some(relative_path),
    );

    std::fs::remove_dir_all(root).unwrap();
}
