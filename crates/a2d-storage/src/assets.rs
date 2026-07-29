//! The asset filesystem commit protocol (TODO 3.3, spec §16.3): create-new temp write →
//! userspace flush → file-content and metadata `sync_all` → close and verify → atomic no-replace
//! finalization → destination-directory sync → temp-link removal → temp-directory sync. A
//! successful `flush()` alone is never treated as durable. Only after this function returns an
//! `Asset` may the caller attempt the separate SQLite transaction.
//!
//! Normative terminology, WAL/`synchronous=NORMAL` interaction, supported filesystem scope, and
//! platform limitations are defined in `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.
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

#[path = "asset_platform.rs"]
mod platform;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use a2d_domain::{
    A2dError, Asset, AssetId, AssetKind, EncryptionState, ErrorCategory, ErrorCode, ErrorSeverity,
    system_now_ms,
};
use sha2::{Digest, Sha256};

use crate::AssetPersistenceFailureStage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitFault {
    None,
    FileSync,
    DestinationDirectorySync,
    TempDirectorySync,
    PermissionSet,
}

pub struct AssetStore {
    root: PathBuf,
}

fn map_io_error(context: &str, err: io::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_ASSET_IO_ERROR"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.asset_io",
        format!("{context}: {err}"),
        true,
    )
    .with_detail("context", context)
    .with_detail("io_error_kind", format!("{:?}", err.kind()))
}

fn stage_io_error(code: &'static str, context: &str, err: io::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.asset_io",
        format!("{context}: {err}"),
        true,
    )
    .with_detail("context", context)
    .with_detail("io_error_kind", format!("{:?}", err.kind()))
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

fn map_finalization_error(temp_path: &Path, final_path: &Path, error: io::Error) -> A2dError {
    let mapped = match error.kind() {
        io::ErrorKind::AlreadyExists => integrity_error(
            "STORAGE_ASSET_FINAL_PATH_COLLISION",
            "asset final path already exists; existing content was not replaced",
        ),
        io::ErrorKind::Unsupported | io::ErrorKind::CrossesDevices => A2dError::new(
            ErrorCode::new("STORAGE_ASSET_FINALIZATION_UNSUPPORTED"),
            ErrorCategory::PlatformAdapter,
            ErrorSeverity::Error,
            "error.storage.asset_finalization_unsupported",
            format!(
                "the platform or filesystem cannot provide required same-filesystem no-replace asset finalization: {error}"
            ),
            false,
        ),
        _ => stage_io_error(
            "STORAGE_ASSET_FINALIZATION_FAILED",
            "atomically finalizing the asset without replacement",
            error,
        ),
    };
    mapped
        .with_detail("temp_path", temp_path.to_string_lossy())
        .with_detail("final_path", final_path.to_string_lossy())
}

fn map_directory_sync_error(
    code: &'static str,
    context: &str,
    directory: &Path,
    error: io::Error,
) -> A2dError {
    let mapped = if error.kind() == io::ErrorKind::Unsupported {
        A2dError::new(
            ErrorCode::new("STORAGE_DIRECTORY_SYNC_UNSUPPORTED"),
            ErrorCategory::PlatformAdapter,
            ErrorSeverity::Error,
            "error.storage.directory_sync_unsupported",
            format!("the platform cannot provide required asset directory synchronization: {error}"),
            false,
        )
    } else {
        stage_io_error(code, context, error)
    };
    mapped.with_detail("directory", directory.to_string_lossy())
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

fn maybe_inject_fault(
    configured: CommitFault,
    requested: CommitFault,
    operation: &'static str,
) -> io::Result<()> {
    if configured == requested {
        Err(io::Error::other(format!(
            "injected FIX-021 test failure during {operation}"
        )))
    } else {
        Ok(())
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
            if error.kind() == io::ErrorKind::NotFound {
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

    /// Runs the no-replace asset filesystem commit protocol for in-memory `data`, returning an
    /// `Asset` value the caller may insert into SQLite only after every required synchronization
    /// and verification step succeeds.
    pub fn commit(
        &self,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        self.commit_with_id_and_fault(
            AssetId::try_generate()?,
            data,
            kind,
            media_type.into(),
            CommitFault::None,
        )
    }

    /// Test-only deterministic entry point for collision and interruption coverage. Production
    /// callers cannot select an asset ID.
    #[cfg(feature = "test-util")]
    pub fn commit_with_id_for_test(
        &self,
        id: AssetId,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        self.commit_with_id_and_fault(id, data, kind, media_type.into(), CommitFault::None)
    }

    #[cfg(feature = "test-util")]
    pub fn commit_with_file_sync_failure_for_test(
        &self,
        id: AssetId,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        self.commit_with_id_and_fault(id, data, kind, media_type.into(), CommitFault::FileSync)
    }

    #[cfg(feature = "test-util")]
    pub fn commit_with_destination_directory_sync_failure_for_test(
        &self,
        id: AssetId,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        self.commit_with_id_and_fault(
            id,
            data,
            kind,
            media_type.into(),
            CommitFault::DestinationDirectorySync,
        )
    }

    #[cfg(feature = "test-util")]
    pub fn commit_with_temp_directory_sync_failure_for_test(
        &self,
        id: AssetId,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        self.commit_with_id_and_fault(
            id,
            data,
            kind,
            media_type.into(),
            CommitFault::TempDirectorySync,
        )
    }

    #[cfg(feature = "test-util")]
    pub fn commit_with_permission_failure_for_test(
        &self,
        id: AssetId,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        self.commit_with_id_and_fault(
            id,
            data,
            kind,
            media_type.into(),
            CommitFault::PermissionSet,
        )
    }

    fn commit_with_id_and_fault(
        &self,
        id: AssetId,
        data: &[u8],
        kind: AssetKind,
        media_type: String,
        fault: CommitFault,
    ) -> Result<Asset, A2dError> {
        let byte_length = u64::try_from(data.len()).map_err(|_| {
            integrity_error(
                "STORAGE_ASSET_LENGTH_UNSUPPORTED",
                "asset byte length does not fit the portable stored representation",
            )
            .with_detail("asset_id", id.to_string())
        })?;
        let expected_hash = hex_sha256(data);
        let created_at_ms = system_now_ms()?;
        let tmp_path = self.tmp_dir().join(format!("{id}.tmp"));
        let relative_path = format!("assets/{}/{id}", asset_kind_dir(kind));
        let final_path = self.root.join(&relative_path);

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|error| {
                let already_exists = error.kind() == io::ErrorKind::AlreadyExists;
                with_persistence_details(
                    A2dError::new(
                        ErrorCode::new(if already_exists {
                            "STORAGE_ASSET_TEMP_PATH_COLLISION"
                        } else {
                            "STORAGE_ASSET_TEMP_CREATE_FAILED"
                        }),
                        if already_exists {
                            ErrorCategory::Integrity
                        } else {
                            ErrorCategory::Storage
                        },
                        if already_exists {
                            ErrorSeverity::Critical
                        } else {
                            ErrorSeverity::Error
                        },
                        "error.storage.asset_temp_create_failed",
                        format!("creating the asset temp file failed: {error}"),
                        !already_exists,
                    )
                    .with_detail("temp_path", tmp_path.to_string_lossy()),
                    AssetPersistenceFailureStage::BeforeFinalization,
                    &id,
                    kind,
                    &relative_path,
                    &expected_hash,
                    byte_length,
                    false,
                    false,
                )
            })?;

        if let Err(error) = file.write_all(data) {
            drop(file);
            return Err(with_persistence_details(
                with_cleanup_result(map_io_error("writing the asset temp file", error), &tmp_path),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                false,
                false,
            ));
        }
        if let Err(error) = file.flush() {
            drop(file);
            return Err(with_persistence_details(
                with_cleanup_result(map_io_error("flushing the asset temp file", error), &tmp_path),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                false,
                false,
            ));
        }

        let immutable = kind == AssetKind::Original;
        if immutable {
            let metadata = match file.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    drop(file);
                    return Err(with_persistence_details(
                        with_cleanup_result(
                            map_io_error("reading asset temp metadata", error),
                            &tmp_path,
                        ),
                        AssetPersistenceFailureStage::BeforeFinalization,
                        &id,
                        kind,
                        &relative_path,
                        &expected_hash,
                        byte_length,
                        false,
                        false,
                    ));
                }
            };
            let mut permissions = metadata.permissions();
            permissions.set_readonly(true);
            let permission_result = maybe_inject_fault(
                fault,
                CommitFault::PermissionSet,
                "setting immutable original permissions",
            )
            .and_then(|()| file.set_permissions(permissions));
            if let Err(error) = permission_result {
                drop(file);
                return Err(with_persistence_details(
                    with_cleanup_result(
                        stage_io_error(
                            "STORAGE_ASSET_PERMISSION_SET_FAILED",
                            "marking the original asset read-only",
                            error,
                        )
                        .with_detail("final_path", final_path.to_string_lossy()),
                        &tmp_path,
                    ),
                    AssetPersistenceFailureStage::BeforeFinalization,
                    &id,
                    kind,
                    &relative_path,
                    &expected_hash,
                    byte_length,
                    false,
                    false,
                ));
            }
        }

        let file_sync_result = maybe_inject_fault(
            fault,
            CommitFault::FileSync,
            "synchronizing the asset temp file",
        )
        .and_then(|()| file.sync_all());
        if let Err(error) = file_sync_result {
            drop(file);
            return Err(with_persistence_details(
                with_cleanup_result(
                    stage_io_error(
                        "STORAGE_ASSET_FILE_SYNC_FAILED",
                        "synchronizing the asset temp file",
                        error,
                    ),
                    &tmp_path,
                ),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                false,
                false,
            ));
        }
        drop(file);

        let on_disk = std::fs::read(&tmp_path).map_err(|error| {
            with_persistence_details(
                with_cleanup_result(
                    map_io_error(
                        "re-reading the asset temp file to verify its contents",
                        error,
                    ),
                    &tmp_path,
                ),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            )
        })?;
        let actual_byte_length = u64::try_from(on_disk.len()).map_err(|_| {
            with_persistence_details(
                with_cleanup_result(
                    integrity_error(
                        "STORAGE_ASSET_LENGTH_UNSUPPORTED",
                        "written asset byte length does not fit the portable stored representation",
                    ),
                    &tmp_path,
                ),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            )
        })?;
        if actual_byte_length != byte_length {
            return Err(with_persistence_details(
                with_cleanup_result(
                    integrity_error(
                        "STORAGE_ASSET_LENGTH_MISMATCH_ON_WRITE",
                        "the temp file byte length differs from the supplied asset bytes",
                    )
                    .with_detail("expected_byte_length", byte_length.to_string())
                    .with_detail("actual_byte_length", actual_byte_length.to_string()),
                    &tmp_path,
                ),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            ));
        }
        let actual_hash = hex_sha256(&on_disk);
        if actual_hash != expected_hash {
            return Err(with_persistence_details(
                with_cleanup_result(
                    integrity_error(
                        "STORAGE_ASSET_HASH_MISMATCH_ON_WRITE",
                        "the temp file contents did not hash to the same value as the supplied data",
                    )
                    .with_detail("expected_sha256", &expected_hash)
                    .with_detail("actual_sha256", actual_hash),
                    &tmp_path,
                ),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            ));
        }

        if let Err(error) = platform::finalize_no_replace(&tmp_path, &final_path) {
            return Err(with_persistence_details(
                with_cleanup_result(
                    map_finalization_error(&tmp_path, &final_path, error),
                    &tmp_path,
                ),
                AssetPersistenceFailureStage::BeforeFinalization,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            ));
        }

        if let Err(error) = verify_finalized_metadata(&final_path, byte_length, immutable) {
            return Err(with_persistence_details(
                error
                    .with_detail("temp_path", tmp_path.to_string_lossy())
                    .with_detail("final_path", final_path.to_string_lossy()),
                AssetPersistenceFailureStage::FinalizedUnregistered,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            ));
        }

        let destination_directory = self.kind_dir(kind);
        let destination_sync_result = maybe_inject_fault(
            fault,
            CommitFault::DestinationDirectorySync,
            "synchronizing the destination asset directory",
        )
        .and_then(|()| platform::sync_directory(&destination_directory));
        if let Err(error) = destination_sync_result {
            return Err(with_persistence_details(
                map_directory_sync_error(
                    "STORAGE_ASSET_DESTINATION_DIRECTORY_SYNC_FAILED",
                    "synchronizing the destination asset directory",
                    &destination_directory,
                    error,
                )
                .with_detail("temp_path", tmp_path.to_string_lossy())
                .with_detail("final_path", final_path.to_string_lossy()),
                AssetPersistenceFailureStage::FinalizedUnregistered,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                false,
            ));
        }

        if let Err(error) = std::fs::remove_file(&tmp_path) {
            return Err(with_persistence_details(
                map_io_error("removing the finalized asset temp link", error)
                    .with_detail("temp_path", tmp_path.to_string_lossy())
                    .with_detail("final_path", final_path.to_string_lossy())
                    .with_detail("temp_cleanup_completed", "false"),
                AssetPersistenceFailureStage::FinalizedUnregistered,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                true,
            ));
        }

        let temp_directory = self.tmp_dir();
        let temp_sync_result = maybe_inject_fault(
            fault,
            CommitFault::TempDirectorySync,
            "synchronizing the temporary asset directory",
        )
        .and_then(|()| platform::sync_directory(&temp_directory));
        if let Err(error) = temp_sync_result {
            return Err(with_persistence_details(
                map_directory_sync_error(
                    "STORAGE_ASSET_TEMP_DIRECTORY_SYNC_FAILED",
                    "synchronizing the temporary asset directory",
                    &temp_directory,
                    error,
                )
                .with_detail("temp_path", tmp_path.to_string_lossy())
                .with_detail("final_path", final_path.to_string_lossy())
                .with_detail("temp_cleanup_completed", "true")
                .with_detail("temp_directory_sync_completed", "false"),
                AssetPersistenceFailureStage::FinalizedUnregistered,
                &id,
                kind,
                &relative_path,
                &expected_hash,
                byte_length,
                true,
                true,
            ));
        }

        Ok(Asset::new(
            id,
            kind,
            relative_path,
            media_type,
            byte_length,
            expected_hash,
            created_at_ms,
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
        let on_disk =
            std::fs::read(&path).map_err(|error| map_io_error("reading asset to verify", error))?;
        let actual_byte_length = u64::try_from(on_disk.len()).map_err(|_| {
            integrity_error(
                "STORAGE_ASSET_LENGTH_UNSUPPORTED",
                "asset file byte length does not fit the portable stored representation",
            )
            .with_detail("asset_id", asset.id().to_string())
        })?;
        if actual_byte_length != asset.byte_length {
            return Err(integrity_error(
                "STORAGE_ASSET_LENGTH_MISMATCH",
                "the asset file's current byte length does not match its recorded length",
            )
            .with_detail("asset_id", asset.id().to_string())
            .with_detail("expected_byte_length", asset.byte_length.to_string())
            .with_detail("actual_byte_length", actual_byte_length.to_string()));
        }
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

#[allow(clippy::too_many_arguments)]
fn with_persistence_details(
    error: A2dError,
    stage: AssetPersistenceFailureStage,
    asset_id: &AssetId,
    kind: AssetKind,
    final_relative_path: &str,
    expected_sha256: &str,
    byte_length: u64,
    file_sync_completed: bool,
    directory_sync_completed: bool,
) -> A2dError {
    error
        .with_detail("asset_commit_failure_stage", stage.as_detail_value())
        .with_detail("asset_id", asset_id.to_string())
        .with_detail("asset_kind", format!("{kind:?}"))
        .with_detail("final_relative_path", final_relative_path)
        .with_detail("expected_sha256", expected_sha256)
        .with_detail("byte_length", byte_length.to_string())
        .with_detail(
            "final_file_created",
            (stage != AssetPersistenceFailureStage::BeforeFinalization).to_string(),
        )
        .with_detail("file_sync_completed", file_sync_completed.to_string())
        .with_detail(
            "directory_sync_completed",
            directory_sync_completed.to_string(),
        )
}

fn with_cleanup_result(error: A2dError, tmp_path: &Path) -> A2dError {
    match std::fs::remove_file(tmp_path) {
        Ok(()) => error
            .with_detail("temp_path", tmp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "true"),
        Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => error
            .with_detail("temp_path", tmp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "true"),
        Err(cleanup_error) => error
            .with_detail("temp_path", tmp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "false")
            .with_detail("temp_cleanup_error", cleanup_error.to_string()),
    }
}

fn verify_finalized_metadata(
    path: &Path,
    expected_byte_length: u64,
    immutable: bool,
) -> Result<(), A2dError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| map_io_error("reading finalized asset metadata", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(integrity_error(
            "STORAGE_ASSET_FINAL_PATH_INVALID",
            "finalized asset path must identify a regular non-symlink file",
        ));
    }
    if metadata.len() != expected_byte_length {
        return Err(integrity_error(
            "STORAGE_ASSET_FINAL_LENGTH_MISMATCH",
            "finalized asset byte length differs from the synchronized temp file",
        )
        .with_detail("expected_byte_length", expected_byte_length.to_string())
        .with_detail("actual_byte_length", metadata.len().to_string()));
    }
    if immutable && !metadata.permissions().readonly() {
        return Err(integrity_error(
            "STORAGE_ASSET_FINAL_ORIGINAL_WRITABLE",
            "finalized original asset is not read-only",
        ));
    }
    Ok(())
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
