//! The asset commit protocol (TODO 3.3, spec §16.3): temp write → flush/close → compute (and
//! verify) SHA-256 → atomic rename → caller commits the DB row. This module owns only the
//! filesystem half; the DB half is `AssetRepository::insert_asset` (repository.rs). They're kept
//! separate on purpose — a caller composing a larger transaction (e.g. Milestone 9's scan
//! registration: commit an asset, insert a scan, update a page, all as one commit) calls
//! `AssetStore::commit` first, then folds `Storage::transaction`'s repository calls around its
//! result, rather than this module reaching into the database itself.
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

    /// Resolves a relative path stored in the database back to an absolute path, rejecting
    /// anything that would escape `root` (TODO 3.3: "Validate paths remain inside library
    /// root"). Writes never need this — this crate always generates the relative path itself
    /// from an `AssetId` — but reads defend against a corrupted or tampered database value.
    pub fn resolve(&self, relative_path: &str) -> Result<PathBuf, A2dError> {
        let candidate = self.root.join(relative_path);
        let canonical_root = self
            .root
            .canonicalize()
            .map_err(|e| map_io_error("canonicalizing the library root", e))?;
        let canonical_candidate = candidate
            .canonicalize()
            .map_err(|e| map_io_error("canonicalizing an asset path", e))?;
        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_ASSET_PATH_ESCAPES_ROOT"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.asset_path_escapes_root",
                "asset relative_path resolves outside the library root",
                false,
            )
            .with_detail("relative_path", relative_path));
        }
        Ok(candidate)
    }

    /// Runs the asset commit protocol (spec §16.3) for in-memory `data`, returning an `Asset`
    /// value the caller then inserts into the database (typically inside a
    /// `Storage::transaction`, so the DB row and the durably-written file become visible
    /// together). Does **not** touch the database itself — see this module's doc comment.
    ///
    /// 1. Write to a temp file under `tmp/`.
    /// 2. Flush and close it.
    /// 3. Compute SHA-256 of what was written, then re-read the file and verify the on-disk
    ///    bytes hash the same (catches a write bug or disk corruption, not just trusting the
    ///    in-memory bytes we started with).
    /// 4. Atomically rename into `assets/<kind>/`.
    /// 5. For `AssetKind::Original`, mark the file read-only (best-effort OS-level immutability
    ///    signal, alongside the `immutable` DB flag `AssetRepository` sets).
    pub fn commit(
        &self,
        data: &[u8],
        kind: AssetKind,
        media_type: impl Into<String>,
    ) -> Result<Asset, A2dError> {
        let id = AssetId::generate();
        let tmp_path = self.tmp_dir().join(format!("{id}.tmp"));

        // 1 + 2: write to temp, then flush/close (the File is dropped at the end of this block,
        // closing it; flush() is still called explicitly first so a flush error is caught by
        // name rather than silently surfacing as a close-time error).
        {
            let mut file = std::fs::File::create(&tmp_path)
                .map_err(|e| map_io_error("creating the temp file", e))?;
            file.write_all(data)
                .map_err(|e| map_io_error("writing the temp file", e))?;
            file.flush()
                .map_err(|e| map_io_error("flushing the temp file", e))?;
        }

        // 3: compute, then verify against what's actually on disk.
        let expected_hash = hex_sha256(data);
        let on_disk = std::fs::read(&tmp_path).map_err(|e| {
            cleanup_tmp(&tmp_path);
            map_io_error("re-reading the temp file to verify its hash", e)
        })?;
        let actual_hash = hex_sha256(&on_disk);
        if actual_hash != expected_hash {
            cleanup_tmp(&tmp_path);
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_ASSET_HASH_MISMATCH_ON_WRITE"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.asset_hash_mismatch_on_write",
                "the temp file's on-disk contents did not hash to the same value as the data written",
                true,
            ));
        }

        // 4: atomic rename.
        let relative_path = format!("assets/{}/{id}", asset_kind_dir(kind));
        let final_path = self.root.join(&relative_path);
        std::fs::rename(&tmp_path, &final_path).map_err(|e| {
            cleanup_tmp(&tmp_path);
            map_io_error("atomically renaming the temp file into place", e)
        })?;

        // 5: mark originals read-only.
        let immutable = kind == AssetKind::Original;
        if immutable {
            let mut perms = std::fs::metadata(&final_path)
                .map_err(|e| map_io_error("reading final asset metadata", e))?
                .permissions();
            perms.set_readonly(true);
            std::fs::set_permissions(&final_path, perms)
                .map_err(|e| map_io_error("marking the original asset read-only", e))?;
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

    /// Re-verifies a previously committed asset against the filesystem (TODO 3.4: "Detect
    /// missing assets and hash mismatches"). Distinct from the write-time check inside
    /// [`commit`](Self::commit) — this is for auditing an asset some time after it was
    /// committed, e.g. a library integrity check (spec §16.4) or before trusting a scan's
    /// original during export.
    pub fn verify(&self, asset: &Asset) -> Result<(), A2dError> {
        // Check existence via the plain joined path first: `resolve`'s root-containment check
        // canonicalizes the path, which itself requires the path to exist, so calling `resolve`
        // before checking existence would report a missing file as a generic I/O error instead
        // of the specific "missing" error below.
        if !self.root.join(&asset.relative_path).is_file() {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_ASSET_MISSING"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.asset_missing",
                "the database references an asset whose file no longer exists",
                false,
            )
            .with_detail("asset_id", asset.id().to_string())
            .with_detail("relative_path", &asset.relative_path));
        }
        let path = self.resolve(&asset.relative_path)?;
        let on_disk =
            std::fs::read(&path).map_err(|e| map_io_error("reading asset to verify", e))?;
        let actual_hash = hex_sha256(&on_disk);
        if actual_hash != asset.sha256 {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_ASSET_HASH_MISMATCH"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.asset_hash_mismatch",
                "the asset file's current contents do not match its recorded SHA-256",
                false,
            )
            .with_detail("asset_id", asset.id().to_string())
            .with_detail("expected_sha256", &asset.sha256)
            .with_detail("actual_sha256", &actual_hash));
        }
        Ok(())
    }

    /// Lists files under `tmp/` without deleting anything (TODO 3.3: "Detect orphan temporary
    /// files without deleting unknown files silently"). A file appears here only if a commit was
    /// interrupted between the temp write and the atomic rename — successful commits always
    /// remove their temp file via rename. Cleanup policy (when it's safe to delete an orphan) is
    /// a follow-up; this only reports.
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
        Ok(orphans)
    }
}

fn cleanup_tmp(tmp_path: &Path) {
    // Best-effort: if this fails, list_orphaned_temp_files will surface it later rather than
    // this losing the original error by trying to also report a cleanup failure.
    std::fs::remove_file(tmp_path).ok();
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
