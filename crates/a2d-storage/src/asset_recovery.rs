//! Non-destructive discovery of finalized asset files that have no database row.
//!
//! A finalized file may outlive a failed database transaction. This module reports that condition
//! with enough immutable evidence for a reviewed recovery flow, but it never deletes, imports, or
//! mutates an unknown file automatically.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use a2d_domain::{A2dError, AssetId, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::{Storage, map_rusqlite_error};

const ASSET_DIRECTORIES: [(AssetKind, &str); 5] = [
    (AssetKind::Original, "originals"),
    (AssetKind::Corrected, "corrected"),
    (AssetKind::Ocr, "ocr"),
    (AssetKind::Thumbnail, "thumbnails"),
    (AssetKind::Export, "exports"),
];
const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Stable Rust-owned classification for failures spanning filesystem finalization and SQLite
/// registration. The string representation is transported in `A2dError.details` for FFI callers;
/// Rust producers use this enum so recovery phase values cannot drift between workflows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetPersistenceFailureStage {
    /// The final library-owned path was never created.
    BeforeFinalization,
    /// The final file exists, but no database registration was started successfully.
    FinalizedUnregistered,
    /// Database registration began but failed and rolled back, leaving the final file unreferenced.
    DatabaseRegistrationRolledBack,
}

impl AssetPersistenceFailureStage {
    pub const fn as_detail_value(self) -> &'static str {
        match self {
            Self::BeforeFinalization => "before_finalization",
            Self::FinalizedUnregistered => "finalized_unregistered",
            Self::DatabaseRegistrationRolledBack => "database_registration_rolled_back",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrphanedFinalAsset {
    pub asset_id: AssetId,
    pub kind: AssetKind,
    pub relative_path: String,
    pub byte_length: u64,
    pub sha256: String,
}

impl Storage {
    /// Reports every regular file in a canonical final asset directory whose relative path is not
    /// referenced by any `assets` row. Results are sorted by relative path and no file is modified.
    pub fn discover_orphaned_final_assets(
        &self,
        library_root: &Path,
    ) -> Result<Vec<OrphanedFinalAsset>, A2dError> {
        discover_orphaned_final_assets(&self.conn, library_root)
    }
}

fn discover_orphaned_final_assets(
    conn: &Connection,
    library_root: &Path,
) -> Result<Vec<OrphanedFinalAsset>, A2dError> {
    let canonical_root = library_root.canonicalize().map_err(|error| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_ROOT_INVALID",
            ErrorCategory::Storage,
            format!("canonicalizing the library root failed: {error}"),
            true,
        )
        .with_detail("library_root", library_root.to_string_lossy())
    })?;
    if !canonical_root.is_dir() {
        return Err(recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_ROOT_NOT_DIRECTORY",
            ErrorCategory::Validation,
            "orphan discovery library root is not a directory",
            false,
        )
        .with_detail("library_root", library_root.to_string_lossy()));
    }

    let referenced = referenced_asset_paths(conn)?;
    let mut orphans = Vec::new();
    for (kind, directory_name) in ASSET_DIRECTORIES {
        let directory = canonical_root.join("assets").join(directory_name);
        let directory_metadata = std::fs::symlink_metadata(&directory).map_err(|error| {
            recovery_error(
                "STORAGE_ORPHAN_DISCOVERY_DIRECTORY_MISSING",
                ErrorCategory::Integrity,
                format!("required asset directory {} is unavailable: {error}", directory.display()),
                false,
            )
            .with_detail("asset_kind", format!("{kind:?}"))
            .with_detail("directory", directory.to_string_lossy())
        })?;
        if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
            return Err(recovery_error(
                "STORAGE_ORPHAN_DISCOVERY_DIRECTORY_INVALID",
                ErrorCategory::Integrity,
                "asset directory must be a real directory, not a symlink or other file type",
                false,
            )
            .with_detail("asset_kind", format!("{kind:?}"))
            .with_detail("directory", directory.to_string_lossy()));
        }
        let canonical_directory = directory.canonicalize().map_err(|error| {
            recovery_error(
                "STORAGE_ORPHAN_DISCOVERY_DIRECTORY_INVALID",
                ErrorCategory::Integrity,
                format!("canonicalizing an asset directory failed: {error}"),
                false,
            )
            .with_detail("directory", directory.to_string_lossy())
        })?;
        if !canonical_directory.starts_with(&canonical_root) {
            return Err(recovery_error(
                "STORAGE_ORPHAN_DISCOVERY_DIRECTORY_ESCAPES_ROOT",
                ErrorCategory::Integrity,
                "asset directory resolves outside the library root",
                false,
            )
            .with_detail("directory", directory.to_string_lossy()));
        }

        let entries = std::fs::read_dir(&canonical_directory).map_err(|error| {
            recovery_error(
                "STORAGE_ORPHAN_DISCOVERY_READ_FAILED",
                ErrorCategory::Storage,
                format!("reading an asset directory failed: {error}"),
                true,
            )
            .with_detail("directory", canonical_directory.to_string_lossy())
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                recovery_error(
                    "STORAGE_ORPHAN_DISCOVERY_ENTRY_FAILED",
                    ErrorCategory::Storage,
                    format!("reading an asset directory entry failed: {error}"),
                    true,
                )
                .with_detail("directory", canonical_directory.to_string_lossy())
            })?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                recovery_error(
                    "STORAGE_ORPHAN_DISCOVERY_METADATA_FAILED",
                    ErrorCategory::Storage,
                    format!("reading asset entry metadata failed: {error}"),
                    true,
                )
                .with_detail("path", path.to_string_lossy())
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(recovery_error(
                    "STORAGE_ORPHAN_DISCOVERY_ENTRY_INVALID",
                    ErrorCategory::Integrity,
                    "final asset directories may contain only regular non-symlink files",
                    false,
                )
                .with_detail("path", path.to_string_lossy()));
            }

            let canonical_path = path.canonicalize().map_err(|error| {
                recovery_error(
                    "STORAGE_ORPHAN_DISCOVERY_PATH_INVALID",
                    ErrorCategory::Integrity,
                    format!("canonicalizing a final asset path failed: {error}"),
                    false,
                )
                .with_detail("path", path.to_string_lossy())
            })?;
            if !canonical_path.starts_with(&canonical_directory) {
                return Err(recovery_error(
                    "STORAGE_ORPHAN_DISCOVERY_PATH_ESCAPES_DIRECTORY",
                    ErrorCategory::Integrity,
                    "final asset path resolves outside its expected directory",
                    false,
                )
                .with_detail("path", path.to_string_lossy()));
            }
            let relative_path = canonical_relative_path(&canonical_root, &canonical_path)?;
            if referenced.contains(&relative_path) {
                continue;
            }
            let asset_id = asset_id_from_path(&canonical_path)?;
            let (byte_length, sha256) = measure_file(&canonical_path)?;
            orphans.push(OrphanedFinalAsset {
                asset_id,
                kind,
                relative_path,
                byte_length,
                sha256,
            });
        }
    }
    orphans.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(orphans)
}

fn referenced_asset_paths(conn: &Connection) -> Result<BTreeSet<String>, A2dError> {
    let mut statement = conn
        .prepare("SELECT relative_path FROM assets ORDER BY relative_path")
        .map_err(|error| map_rusqlite_error("preparing referenced asset path query", error))?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| map_rusqlite_error("reading referenced asset paths", error))?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|error| map_rusqlite_error("decoding referenced asset paths", error))
}

fn canonical_relative_path(root: &Path, path: &Path) -> Result<String, A2dError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_PATH_ESCAPES_ROOT",
            ErrorCategory::Integrity,
            "final asset path is not contained by the canonical library root",
            false,
        )
        .with_detail("path", path.to_string_lossy())
    })?;
    let text = relative.to_str().ok_or_else(|| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_PATH_NOT_UTF8",
            ErrorCategory::UnsupportedFormat,
            "final asset relative path is not valid UTF-8",
            false,
        )
        .with_detail("path", path.to_string_lossy())
    })?;
    Ok(text.replace('\\', "/"))
}

fn asset_id_from_path(path: &Path) -> Result<AssetId, A2dError> {
    let file_name = path.file_name().and_then(|value| value.to_str()).ok_or_else(|| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_ASSET_ID_MISSING",
            ErrorCategory::Integrity,
            "final asset path does not have a valid UTF-8 asset ID filename",
            false,
        )
        .with_detail("path", path.to_string_lossy())
    })?;
    AssetId::parse(file_name).map_err(|error| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_ASSET_ID_INVALID",
            ErrorCategory::Integrity,
            "final asset filename is not a canonical AssetId",
            false,
        )
        .with_detail("path", path.to_string_lossy())
        .with_detail("file_name", file_name)
        .with_detail("cause_code", error.code.to_string())
    })
}

fn measure_file(path: &Path) -> Result<(u64, String), A2dError> {
    // Opening after a no-symlink metadata check leaves a small path-substitution window on
    // platforms without a standard-library O_NOFOLLOW API. The open handle is then authoritative
    // for both metadata and hashing, minimizing that window without adding platform-specific code.
    let mut file = std::fs::File::open(path).map_err(|error| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_OPEN_FAILED",
            ErrorCategory::Storage,
            format!("opening a final asset for measurement failed: {error}"),
            true,
        )
        .with_detail("path", path.to_string_lossy())
    })?;
    let metadata = file.metadata().map_err(|error| {
        recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_METADATA_FAILED",
            ErrorCategory::Storage,
            format!("reading opened asset metadata failed: {error}"),
            true,
        )
        .with_detail("path", path.to_string_lossy())
    })?;
    if !metadata.is_file() {
        return Err(recovery_error(
            "STORAGE_ORPHAN_DISCOVERY_ENTRY_CHANGED",
            ErrorCategory::Integrity,
            "final asset stopped being a regular file during discovery",
            false,
        )
        .with_detail("path", path.to_string_lossy()));
    }

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            recovery_error(
                "STORAGE_ORPHAN_DISCOVERY_READ_FAILED",
                ErrorCategory::Storage,
                format!("reading a final asset for hashing failed: {error}"),
                true,
            )
            .with_detail("path", path.to_string_lossy())
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((metadata.len(), sha256))
}

fn recovery_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.storage.orphan_discovery",
        message.into(),
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use a2d_domain::PageId;

    use super::*;
    use crate::{AssetRepository, AssetStore};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "a2d-orphan-final-{label}-{}",
            PageId::generate()
        ))
    }

    #[test]
    fn failure_stage_detail_values_are_stable() {
        assert_eq!(
            AssetPersistenceFailureStage::BeforeFinalization.as_detail_value(),
            "before_finalization"
        );
        assert_eq!(
            AssetPersistenceFailureStage::FinalizedUnregistered.as_detail_value(),
            "finalized_unregistered"
        );
        assert_eq!(
            AssetPersistenceFailureStage::DatabaseRegistrationRolledBack.as_detail_value(),
            "database_registration_rolled_back"
        );
    }

    #[test]
    fn unreferenced_final_asset_is_reported_and_never_deleted() {
        let root = temp_root("report");
        let storage = Storage::open_in_memory().unwrap();
        let store = AssetStore::open(&root).unwrap();
        let referenced = store
            .commit(b"referenced bytes", AssetKind::Corrected, "image/png")
            .unwrap();
        let orphan = store
            .commit(b"orphan bytes", AssetKind::Export, "application/pdf")
            .unwrap();
        storage.insert_asset(&referenced).unwrap();

        let discovered = storage.discover_orphaned_final_assets(&root).unwrap();
        assert_eq!(discovered.len(), 1);
        assert_eq!(&discovered[0].asset_id, orphan.id());
        assert_eq!(discovered[0].kind, AssetKind::Export);
        assert_eq!(discovered[0].relative_path, orphan.relative_path);
        assert_eq!(discovered[0].byte_length, orphan.byte_length);
        assert_eq!(discovered[0].sha256, orphan.sha256);
        assert!(store.resolve(&orphan.relative_path).unwrap().is_file());

        let repeated = storage.discover_orphaned_final_assets(&root).unwrap();
        assert_eq!(repeated, discovered);
        assert!(store.resolve(&orphan.relative_path).unwrap().is_file());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn invalid_final_asset_filename_fails_closed_without_deleting_it() {
        let root = temp_root("invalid-name");
        let storage = Storage::open_in_memory().unwrap();
        let _store = AssetStore::open(&root).unwrap();
        let invalid_path = root.join("assets").join("exports").join("not-an-asset-id");
        std::fs::write(&invalid_path, b"unknown bytes").unwrap();

        let error = storage
            .discover_orphaned_final_assets(&root)
            .unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "STORAGE_ORPHAN_DISCOVERY_ASSET_ID_INVALID"
        );
        assert!(invalid_path.is_file());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn referenced_assets_are_not_reported() {
        let root = temp_root("referenced");
        let storage = Storage::open_in_memory().unwrap();
        let store = AssetStore::open(&root).unwrap();
        let asset = store
            .commit(b"registered bytes", AssetKind::Original, "image/jpeg")
            .unwrap();
        storage.insert_asset(&asset).unwrap();

        assert!(
            storage
                .discover_orphaned_final_assets(&root)
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(root).ok();
    }
}
