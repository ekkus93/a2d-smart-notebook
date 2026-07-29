//! The asset commit protocol (TODO 3.3, spec §16.3): create-new temp write → flush and sync →
//! compute and verify SHA-256 → atomic no-replace finalization → sync directories → caller commits
//! the DB row. This module owns only the filesystem half; the DB half is
//! `AssetRepository::insert_asset` (repository.rs).
//!
//! ```text
//! library/
//! ├── library.sqlite
//! ├── assets/
//! │   ├── originals/
//! │   ├── corrected/
//! │   ├── ocr/
//! │   ├── thumbnails/
//! │   └── exports/
//! └── tmp/
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};

use a2d_domain::A2dError;
use a2d_domain::{
    Asset, AssetId, AssetKind, EncryptionState, ErrorCategory, ErrorCode, ErrorSeverity,
};
use sha2::{Digest, Sha256};

pub struct AssetStore {
    root: PathBuf,
}

fn map_io_error(context: &str, err: std::io::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_ASSET_IO_ERROR"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.asset_io",
        format!("{context}: {err}"),
        true,
    )
    .with_detail("context", context)
}

fn integrity_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.asset_integrity",
        message.into(),
        false,
    )
}

fn asset_kind_dir(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Original => "originals",
        AssetKind::Corrected => "corrected",
        AssetKind::Ocr => "ocr",
        AssetKind::Thumbnail => "thumbnails",
        AssetKind::Export => "exports",
    }
}

impl AssetStore {
    /// `root` is the library directory (containing `library.sqlite`), not the `assets/`
    /// subdirectory itself. Creates `assets/{originals,corrected,ocr,thumbnails,exports}/` and
    /// `tmp/` if they don't already exist.
    pub fn open(root: &Path) -> Result<Self, A2dError> {
        let store = Self {
            root: root.to_path_buf(),
        };
        std::fs::create_dir_all(store.tmp_dir()).map_err(|e| map_io_error("creating tmp/", e))?;
        for kind in [
            AssetKind::Original,
            AssetKind::Corrected,
            AssetKind::Ocr,
            AssetKind::Thumbnail,
            AssetKind::Export,
        ] {
            std::fs::create_dir_all(store.kind_dir(kind))
                .map_err(|e| map_io_error("creating an assets/ subdirectory", e))?;
        }
        Ok(store)
    }

    fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    fn kind_dir(&self, kind: AssetKind) -> PathBuf {
        self.root.join("assets").join(asset_kind_dir(kind))
    }

    /// Resolves a relative path stored in the database back to the canonical absolute path,
    /// rejecting missing files, symlinks, and anything that escapes `root`.
    pub fn resolve(&self, relative_path: &str) -> Result<PathBuf, A2dError> {
        let candidate = self.root.join(relative_path);
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                A2dError::new(
                    ErrorCode::new("STORAGE_ASSET_MISSING"),
                    ErrorCategory::Integrity,
                    ErrorSeverity::Critical,
                    "error.storage.asset_missing",
                    "the database references an asset whose file no longer exists",
                    false,
                )
                .with_detail("relative_path", relative_path)
            } else {
                map_io_error("reading asset path metadata", error)
                    .with_detail("relative_path", relative_path)
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(integrity_error(
                "STORAGE_ASSET_PATH_IS_SYMLINK",
                "asset relative_path must not identify a symbolic link",
            )
            .with_detail("relative_path", relative_path));
        }
        if !metadata.is_file() {
            return Err(integrity_error(
                "STORAGE_ASSET_PATH_NOT_FILE",
                "asset relative_path must identify a regular file",
            )
            .with_detail("relative_path", relative_path));
        }

        let canonical_root = self
            .root
            .canonicalize()
            .map_err(|e| map_io_error("canonicalizing the library root", e))?;
        let canonical_candidate = candidate
            .canonicalize()
            .map_err(|e| map_io_error("canonicalizing an asset path", e))?;
        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(integrity_error(
                "STORAGE_ASSET_PATH_ESCAPES_ROOT",
                "asset relative_path resolves outside the library root",
            )
            .with_detail("relative_path", relative_path));
        }
        Ok(canonical_candidate)
    }

    /// Runs the durable no-replace asset commit protocol for in-memory `data`, returning an
    /// `Asset` value the caller may insert into SQLite only after this function succeeds.
    ///
    /// The final path is created with a hard link from the synchronized temp inode. Hard-link
    /// creation is atomic, remains on the same filesystem, and fails rather than replacing an
    /// existing destination. Android and Apple targets are Unix platforms; on Unix, both the
    /// destination and temp directories are synchronized before success is returned.
    pub fn commit(
        &self,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        let id = AssetId::generate();
        let tmp_path = self.tmp_dir().join(format!("{id}.tmp"));
        let relative_path = format!("assets/{}/{id}", asset_kind_dir(kind));
        let final_path = self.root.join(&relative_path);

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|error| {
                let code = if error.kind() == std::io::ErrorKind::AlreadyExists {
                    "STORAGE_ASSET_TEMP_PATH_COLLISION"
                } else {
                    "STORAGE_ASSET_TEMP_CREATE_FAILED"
                };
                A2dError::new(
                    ErrorCode::new(code),
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        ErrorCategory::Integrity
                    } else {
                        ErrorCategory::Storage
                    },
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        ErrorSeverity::Critical
                    } else {
                        ErrorSeverity::Error
                    },
                    "error.storage.asset_temp_create_failed",
                    format!("creating the asset temp file failed: {error}"),
                    error.kind() != std::io::ErrorKind::AlreadyExists,
                )
                .with_detail("asset_id", id.to_string())
                .with_detail("temp_path", tmp_path.to_string_lossy())
            })?;

        if let Err(error) = file.write_all(data) {
            drop(file);
            return Err(with_cleanup_result(
                map_io_error("writing the asset temp file", error),
                &tmp_path,
            )
            .with_detail("asset_id", id.to_string()));
        }
        if let Err(error) = file.flush() {
            drop(file);
            return Err(with_cleanup_result(
                map_io_error("flushing the asset temp file", error),
                &tmp_path,
            )
            .with_detail("asset_id", id.to_string()));
        }

        let immutable = kind == AssetKind::Original;
        if immutable {
            let mut permissions = file
                .metadata()
                .map_err(|error| map_io_error("reading asset temp metadata", error))?
                .permissions();
            permissions.set_readonly(true);
            if let Err(error) = std::fs::set_permissions(&tmp_path, permissions) {
                drop(file);
                return Err(with_cleanup_result(
                    map_io_error("marking the original asset read-only", error),
                    &tmp_path,
                )
                .with_detail("asset_id", id.to_string()));
            }
        }
        if let Err(error) = file.sync_all() {
            drop(file);
            return Err(with_cleanup_result(
                map_io_error("synchronizing the asset temp file", error),
                &tmp_path,
            )
            .with_detail("asset_id", id.to_string()));
        }
        drop(file);

        let expected_hash = hex_sha256(data);
        let on_disk = std::fs::read(&tmp_path).map_err(|error| {
            with_cleanup_result(
                map_io_error("re-reading the asset temp file to verify its hash", error),
                &tmp_path,
            )
            .with_detail("asset_id", id.to_string())
        })?;
        let actual_hash = hex_sha256(&on_disk);
        if actual_hash != expected_hash {
            return Err(with_cleanup_result(
                integrity_error(
                    "STORAGE_ASSET_HASH_MISMATCH_ON_WRITE",
                    "the temp file contents did not hash to the same value as the supplied data",
                )
                .with_detail("expected_sha256", &expected_hash)
                .with_detail("actual_sha256", actual_hash),
                &tmp_path,
            )
            .with_detail("asset_id", id.to_string()));
        }

        if let Err(error) = std::fs::hard_link(&tmp_path, &final_path) {
            let mapped = if error.kind() == std::io::ErrorKind::AlreadyExists {
                integrity_error(
                    "STORAGE_ASSET_FINAL_PATH_COLLISION",
                    "asset final path already exists; existing content was not replaced",
                )
            } else {
                map_io_error("atomically finalizing the asset without replacement", error)
            };
            return Err(with_cleanup_result(mapped, &tmp_path)
                .with_detail("asset_id", id.to_string())
                .with_detail("final_path", final_path.to_string_lossy()));
        }

        if let Err(error) = sync_directory(&self.kind_dir(kind)) {
            return Err(error
                .with_detail("asset_id", id.to_string())
                .with_detail("temp_path", tmp_path.to_string_lossy())
                .with_detail("final_path", final_path.to_string_lossy())
                .with_detail("final_file_created", "true")
                .with_detail("directory_sync_completed", "false"));
        }

        if let Err(error) = std::fs::remove_file(&tmp_path) {
            return Err(map_io_error("removing the finalized asset temp link", error)
                .with_detail("asset_id", id.to_string())
                .with_detail("temp_path", tmp_path.to_string_lossy())
                .with_detail("final_path", final_path.to_string_lossy())
                .with_detail("final_file_created", "true")
                .with_detail("directory_sync_completed", "true")
                .with_detail("temp_cleanup_completed", "false"));
        }
        if let Err(error) = sync_directory(&self.tmp_dir()) {
            return Err(error
                .with_detail("asset_id", id.to_string())
                .with_detail("temp_path", tmp_path.to_string_lossy())
                .with_detail("final_path", final_path.to_string_lossy())
                .with_detail("final_file_created", "true")
                .with_detail("directory_sync_completed", "true")
                .with_detail("temp_cleanup_completed", "true")
                .with_detail("temp_directory_sync_completed", "false"));
        }

        Ok(Asset::new(
            id,
            kind,
            relative_path,
            media_type.into(),
            data.len() as u64,
            expected_hash,
            now_ms(),
            immutable,
            EncryptionState::Plaintext,
        ))
    }

    /// Re-verifies a previously committed asset against the filesystem.
    pub fn verify(&self, asset: &Asset) -> Result<(), A2dError> {
        let path = self.resolve(&asset.relative_path).map_err(|error| {
            error
                .with_detail("asset_id", asset.id().to_string())
                .with_detail("relative_path", &asset.relative_path)
        })?;
        let on_disk = std::fs::read(&path)
            .map_err(|e| map_io_error("reading asset to verify", e))?;
        let actual_hash = hex_sha256(&on_disk);
        if actual_hash != asset.sha256 {
            return Err(integrity_error(
                "STORAGE_ASSET_HASH_MISMATCH",
                "the asset file's current contents do not match its recorded SHA-256",
            )
            .with_detail("asset_id", asset.id().to_string())
            .with_detail("expected_sha256", &asset.sha256)
            .with_detail("actual_sha256", &actual_hash));
        }
        Ok(())
    }

    /// Lists files under `tmp/` without deleting anything. Results are sorted for deterministic
    /// diagnostics and tests.
    pub fn list_orphaned_temp_files(&self) -> Result<Vec<PathBuf>, A2dError> {
        let mut orphans = Vec::new();
        let entries =
            std::fs::read_dir(self.tmp_dir()).map_err(|e| map_io_error("listing tmp/", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| map_io_error("reading a tmp/ directory entry", e))?;
            if entry.path().is_file() {
                orphans.push(entry.path());
            }
        }
        orphans.sort();
        Ok(orphans)
    }
}

fn with_cleanup_result(error: A2dError, tmp_path: &Path) -> A2dError {
    match std::fs::remove_file(tmp_path) {
        Ok(()) => error
            .with_detail("temp_path", tmp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "true"),
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => error
            .with_detail("temp_path", tmp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "true"),
        Err(cleanup_error) => error
            .with_detail("temp_path", tmp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "false")
            .with_detail("temp_cleanup_error", cleanup_error.to_string()),
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), A2dError> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| map_io_error("synchronizing an asset directory", error))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), A2dError> {
    // Android and Apple production targets are Unix. Rust's standard library does not expose a
    // portable directory-sync operation on every host; non-Unix desktop builds retain compile and
    // test feasibility but do not claim the mobile durability guarantee.
    Ok(())
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
