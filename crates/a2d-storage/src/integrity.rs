//! Bounded, cancellable, non-destructive canonical-library integrity diagnostics.
//!
//! The checker reports stable findings and never repairs, deletes, imports, or rewrites data. It
//! operates on the already-open SQLite connection and the caller-supplied library root, with
//! explicit limits for database rows, filesystem entries, and bytes hashed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};

use crate::{MIGRATIONS, Storage, map_rusqlite_error};

const SCAN_CONTENT_PREFIX: &str = "scan-content-v1;corrected-sha256=";
const PERCEPTUAL_PREFIX: &str = ";perceptual=mean-grid-16x24-v1:";
const SHA256_HEX_LENGTH: usize = 64;
const PERCEPTUAL_HEX_LENGTH: usize = 16 * 24 * 2;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IntegrityFindingSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl IntegrityFindingSeverity {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
            Self::Critical => "CRITICAL",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrityFinding {
    pub code: String,
    pub severity: IntegrityFindingSeverity,
    pub affected_id: Option<String>,
    pub details: BTreeMap<String, String>,
}

impl IntegrityFinding {
    fn new(code: &'static str, severity: IntegrityFindingSeverity) -> Self {
        Self {
            code: code.to_string(),
            severity,
            affected_id: None,
            details: BTreeMap::new(),
        }
    }

    fn affected(mut self, value: impl Into<String>) -> Self {
        self.affected_id = Some(value.into());
        self
    }

    fn detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegrityCheckOptions {
    pub verify_asset_hashes: bool,
    pub maximum_database_rows: u64,
    pub maximum_filesystem_entries: u64,
    pub maximum_hash_bytes: u64,
}

impl Default for IntegrityCheckOptions {
    fn default() -> Self {
        Self {
            verify_asset_hashes: false,
            maximum_database_rows: 100_000,
            maximum_filesystem_entries: 100_000,
            maximum_hash_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

impl IntegrityCheckOptions {
    fn validate(self) -> Result<Self, A2dError> {
        if self.maximum_database_rows == 0
            || self.maximum_filesystem_entries == 0
            || self.maximum_hash_bytes == 0
        {
            return Err(integrity_request_error(
                "STORAGE_INTEGRITY_LIMIT_INVALID",
                "integrity-check limits must all be greater than zero",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Default)]
pub struct IntegrityCancellation {
    cancelled: Arc<AtomicBool>,
}

impl IntegrityCancellation {
    pub fn active() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrityReport {
    pub schema_version: i64,
    pub verified_asset_hashes: bool,
    pub database_rows_examined: u64,
    pub filesystem_entries_examined: u64,
    pub bytes_hashed: u64,
    pub findings: Vec<IntegrityFinding>,
    /// Reserved hook for the future search milestone. No index is claimed to exist in v0.1.
    pub search_index_check: String,
}

impl IntegrityReport {
    fn new(schema_version: i64, verified_asset_hashes: bool) -> Self {
        Self {
            schema_version,
            verified_asset_hashes,
            database_rows_examined: 0,
            filesystem_entries_examined: 0,
            bytes_hashed: 0,
            findings: Vec::new(),
            search_index_check: "NOT_IMPLEMENTED".to_string(),
        }
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegrityCheckOutcome {
    Completed(IntegrityReport),
    Cancelled(IntegrityReport),
}

#[derive(Clone, Debug)]
struct AssetRow {
    id: String,
    relative_path: String,
    byte_length: u64,
    sha256: String,
}

impl Storage {
    pub fn check_integrity(
        &self,
        library_root: &Path,
        options: IntegrityCheckOptions,
        cancellation: &IntegrityCancellation,
    ) -> Result<IntegrityCheckOutcome, A2dError> {
        let options = options.validate()?;
        let canonical_root = library_root.canonicalize().map_err(|error| {
            integrity_request_error(
                "STORAGE_INTEGRITY_LIBRARY_ROOT_INVALID",
                format!("failed to canonicalize library root: {error}"),
            )
            .with_detail("library_root", library_root.to_string_lossy())
        })?;
        if !canonical_root.is_dir() {
            return Err(integrity_request_error(
                "STORAGE_INTEGRITY_LIBRARY_ROOT_NOT_DIRECTORY",
                "integrity-check library root must be a directory",
            )
            .with_detail("library_root", canonical_root.to_string_lossy()));
        }

        let schema_version = self.schema_version()?;
        let mut report = IntegrityReport::new(schema_version, options.verify_asset_hashes);
        if cancelled(cancellation, &report) {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }

        self.check_migration_identity(&mut report, options)?;
        if cancelled(cancellation, &report) {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }
        self.check_foreign_keys(&mut report, options)?;
        self.check_preferred_scan_consistency(&mut report, options)?;
        self.check_active_notebook_count(&mut report, options)?;
        self.check_page_kind_columns(&mut report, options)?;
        self.check_scan_originals(&mut report, options)?;
        self.check_scan_fingerprints(&mut report, options)?;
        self.check_generated_pdf_references(&mut report, options)?;
        if cancelled(cancellation, &report) {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }

        let assets = self.load_asset_rows(&mut report, options)?;
        let known_asset_paths = assets
            .iter()
            .map(|asset| normalize_relative_path(&asset.relative_path))
            .collect::<BTreeSet<_>>();
        check_asset_files(
            &canonical_root,
            &assets,
            options,
            cancellation,
            &mut report,
        )?;
        if cancelled(cancellation, &report) {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }
        check_orphan_final_files(
            &canonical_root,
            &known_asset_paths,
            options,
            cancellation,
            &mut report,
        )?;
        if cancelled(cancellation, &report) {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }
        check_temp_files(
            &canonical_root,
            options,
            cancellation,
            &mut report,
        )?;

        if cancellation.is_cancelled() {
            Ok(IntegrityCheckOutcome::Cancelled(report))
        } else {
            Ok(IntegrityCheckOutcome::Completed(report))
        }
    }

    fn check_migration_identity(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let mut statement = self
            .conn
            .prepare("SELECT version, name, sha256 FROM schema_migrations ORDER BY version")
            .map_err(|error| map_rusqlite_error("preparing migration integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("reading migration integrity rows", error))?;
        let mut applied = Vec::new();
        for row in rows {
            bump_database_rows(report, options)?;
            applied.push(
                row.map_err(|error| {
                    map_rusqlite_error("decoding migration integrity row", error)
                })?,
            );
        }

        if applied.len() != MIGRATIONS.len() {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_MIGRATION_COUNT_MISMATCH",
                    IntegrityFindingSeverity::Critical,
                )
                .detail("recorded_count", applied.len().to_string())
                .detail("compiled_count", MIGRATIONS.len().to_string()),
            );
        }
        for (index, (version, name, recorded_hash)) in applied.into_iter().enumerate() {
            let Some(expected) = MIGRATIONS.get(index) else {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_MIGRATION_VERSION_UNSUPPORTED",
                        IntegrityFindingSeverity::Critical,
                    )
                    .detail("version", version.to_string())
                    .detail("name", name),
                );
                continue;
            };
            let expected_hash = sha256_bytes(expected.sql.as_bytes());
            if version != expected.version || name != expected.name {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_MIGRATION_IDENTITY_MISMATCH",
                        IntegrityFindingSeverity::Critical,
                    )
                    .detail("recorded_version", version.to_string())
                    .detail("expected_version", expected.version.to_string())
                    .detail("recorded_name", name)
                    .detail("expected_name", expected.name),
                );
            }
            if recorded_hash.as_deref() != Some(expected_hash.as_str()) {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_MIGRATION_DIGEST_MISMATCH",
                        IntegrityFindingSeverity::Critical,
                    )
                    .detail("version", expected.version.to_string())
                    .detail(
                        "recorded_sha256",
                        recorded_hash.unwrap_or_else(|| "missing".to_string()),
                    )
                    .detail("expected_sha256", expected_hash),
                );
            }
        }
        Ok(())
    }

    fn check_foreign_keys(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let mut statement = self
            .conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(|error| map_rusqlite_error("preparing foreign-key integrity check", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("running foreign-key integrity check", error))?;
        for row in rows {
            bump_database_rows(report, options)?;
            let (table, row_id, parent, foreign_key_index) = row.map_err(|error| {
                map_rusqlite_error("decoding foreign-key integrity finding", error)
            })?;
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_FOREIGN_KEY_VIOLATION",
                    IntegrityFindingSeverity::Critical,
                )
                .detail("table", table)
                .detail(
                    "row_id",
                    row_id.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
                )
                .detail("parent_table", parent)
                .detail("foreign_key_index", foreign_key_index.to_string()),
            );
        }
        Ok(())
    }

    fn check_preferred_scan_consistency(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let sql =
            "SELECT p.id, p.preferred_scan_id,
                    (SELECT COUNT(*) FROM scans s WHERE s.page_id = p.id AND s.preferred = 1)
             FROM pages p
             WHERE
               (p.preferred_scan_id IS NULL AND
                (SELECT COUNT(*) FROM scans s WHERE s.page_id = p.id AND s.preferred = 1) != 0)
               OR
               (p.preferred_scan_id IS NOT NULL AND (
                  (SELECT COUNT(*) FROM scans s WHERE s.page_id = p.id AND s.preferred = 1) != 1
                  OR NOT EXISTS (
                    SELECT 1 FROM scans selected
                    WHERE selected.id = p.preferred_scan_id
                      AND selected.page_id = p.id
                      AND selected.preferred = 1
                  )
               ))";
        let mut statement = self
            .conn
            .prepare(sql)
            .map_err(|error| map_rusqlite_error("preparing preferred-scan integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("checking preferred-scan integrity", error))?;
        for row in rows {
            bump_database_rows(report, options)?;
            let (page_id, preferred_scan_id, preferred_count) = row.map_err(|error| {
                map_rusqlite_error("decoding preferred-scan integrity row", error)
            })?;
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_PREFERRED_SCAN_CONTRADICTION",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(page_id)
                .detail(
                    "preferred_scan_id",
                    preferred_scan_id.unwrap_or_else(|| "none".to_string()),
                )
                .detail("preferred_scan_count", preferred_count.to_string()),
            );
        }
        Ok(())
    }

    fn check_active_notebook_count(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        bump_database_rows(report, options)?;
        let count = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM notebooks WHERE active_scan_destination = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| map_rusqlite_error("counting active notebooks", error))?;
        if count > 1 {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_MULTIPLE_ACTIVE_NOTEBOOKS",
                    IntegrityFindingSeverity::Critical,
                )
                .detail("active_notebook_count", count.to_string()),
            );
        }
        Ok(())
    }

    fn check_page_kind_columns(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let sql =
            "SELECT id, kind FROM pages WHERE
               (kind = 'notebook_page' AND (
                    notebook_id IS NULL OR notebook_design_id IS NULL OR
                    logical_page_number IS NULL OR smart_page_id IS NOT NULL OR
                    page_set_id IS NOT NULL OR visible_page_number IS NOT NULL
               )) OR
               (kind = 'smart_page' AND (
                    notebook_id IS NOT NULL OR notebook_design_id IS NOT NULL OR
                    logical_page_number IS NOT NULL OR smart_page_id IS NULL
               )) OR
               kind NOT IN ('notebook_page', 'smart_page')";
        let mut statement = self
            .conn
            .prepare(sql)
            .map_err(|error| map_rusqlite_error("preparing page-kind integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| map_rusqlite_error("checking page-kind columns", error))?;
        for row in rows {
            bump_database_rows(report, options)?;
            let (page_id, kind) = row
                .map_err(|error| map_rusqlite_error("decoding page-kind finding", error))?;
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_PAGE_KIND_COLUMNS_INVALID",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(page_id)
                .detail("kind", kind),
            );
        }
        Ok(())
    }

    fn check_scan_originals(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let sql =
            "SELECT s.id, s.original_asset_id, a.kind, a.immutable
             FROM scans s LEFT JOIN assets a ON a.id = s.original_asset_id
             WHERE a.id IS NULL OR a.kind != 'Original' OR a.immutable != 1";
        let mut statement = self
            .conn
            .prepare(sql)
            .map_err(|error| map_rusqlite_error("preparing scan-original integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<bool>>(3)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("checking scan originals", error))?;
        for row in rows {
            bump_database_rows(report, options)?;
            let (scan_id, asset_id, kind, immutable) = row.map_err(|error| {
                map_rusqlite_error("decoding scan-original finding", error)
            })?;
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_SCAN_ORIGINAL_INVALID",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(scan_id)
                .detail("original_asset_id", asset_id)
                .detail("asset_kind", kind.unwrap_or_else(|| "missing".to_string()))
                .detail(
                    "immutable",
                    immutable.map_or_else(|| "missing".to_string(), |value| value.to_string()),
                ),
            );
        }
        Ok(())
    }

    fn check_scan_fingerprints(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let mut statement = self
            .conn
            .prepare("SELECT id, content_fingerprint FROM scans")
            .map_err(|error| map_rusqlite_error("preparing fingerprint integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| map_rusqlite_error("checking fingerprint formats", error))?;
        for row in rows {
            bump_database_rows(report, options)?;
            let (scan_id, fingerprint) = row
                .map_err(|error| map_rusqlite_error("decoding fingerprint row", error))?;
            if !valid_scan_fingerprint(&fingerprint) {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_SCAN_FINGERPRINT_INVALID",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(scan_id)
                    .detail("format", fingerprint_format_hint(&fingerprint)),
                );
            }
        }
        Ok(())
    }

    fn check_generated_pdf_references(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let sql =
            "SELECT p.id, p.generated_pdf_asset_id, a.kind
             FROM pages p LEFT JOIN assets a ON a.id = p.generated_pdf_asset_id
             WHERE p.generated_pdf_asset_id IS NOT NULL
               AND (a.id IS NULL OR a.kind != 'Export')";
        let mut statement = self
            .conn
            .prepare(sql)
            .map_err(|error| map_rusqlite_error("preparing generated-PDF integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("checking generated-PDF references", error))?;
        for row in rows {
            bump_database_rows(report, options)?;
            let (page_id, asset_id, kind) = row.map_err(|error| {
                map_rusqlite_error("decoding generated-PDF finding", error)
            })?;
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_GENERATED_PDF_REFERENCE_INVALID",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(page_id)
                .detail("generated_pdf_asset_id", asset_id)
                .detail("asset_kind", kind.unwrap_or_else(|| "missing".to_string())),
            );
        }
        Ok(())
    }

    fn load_asset_rows(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<Vec<AssetRow>, A2dError> {
        let mut statement = self
            .conn
            .prepare("SELECT id, relative_path, byte_length, sha256 FROM assets ORDER BY id")
            .map_err(|error| map_rusqlite_error("preparing asset integrity query", error))?;
        let rows = statement
            .query_map([], |row| {
                let byte_length = row.get::<_, i64>(2)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    byte_length,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("reading asset integrity rows", error))?;
        let mut assets = Vec::new();
        for row in rows {
            bump_database_rows(report, options)?;
            let (id, relative_path, byte_length, sha256) = row
                .map_err(|error| map_rusqlite_error("decoding asset integrity row", error))?;
            if byte_length < 0 {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_LENGTH_INVALID",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(id.clone())
                    .detail("byte_length", byte_length.to_string()),
                );
                continue;
            }
            assets.push(AssetRow {
                id,
                relative_path,
                byte_length: byte_length as u64,
                sha256,
            });
        }
        Ok(assets)
    }
}

fn cancelled(cancellation: &IntegrityCancellation, _report: &IntegrityReport) -> bool {
    cancellation.is_cancelled()
}

fn bump_database_rows(
    report: &mut IntegrityReport,
    options: IntegrityCheckOptions,
) -> Result<(), A2dError> {
    report.database_rows_examined = report
        .database_rows_examined
        .checked_add(1)
        .ok_or_else(|| integrity_limit_error("database row counter overflowed"))?;
    if report.database_rows_examined > options.maximum_database_rows {
        return Err(integrity_limit_error(
            "integrity check exceeded maximum_database_rows",
        )
        .with_detail(
            "maximum_database_rows",
            options.maximum_database_rows.to_string(),
        ));
    }
    Ok(())
}

fn bump_filesystem_entries(
    report: &mut IntegrityReport,
    options: IntegrityCheckOptions,
) -> Result<(), A2dError> {
    report.filesystem_entries_examined = report
        .filesystem_entries_examined
        .checked_add(1)
        .ok_or_else(|| integrity_limit_error("filesystem entry counter overflowed"))?;
    if report.filesystem_entries_examined > options.maximum_filesystem_entries {
        return Err(integrity_limit_error(
            "integrity check exceeded maximum_filesystem_entries",
        )
        .with_detail(
            "maximum_filesystem_entries",
            options.maximum_filesystem_entries.to_string(),
        ));
    }
    Ok(())
}

fn check_asset_files(
    canonical_root: &Path,
    assets: &[AssetRow],
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<(), A2dError> {
    for asset in assets {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        bump_filesystem_entries(report, options)?;
        let candidate = canonical_root.join(&asset.relative_path);
        let metadata = match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_FILE_MISSING",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(asset.id.clone())
                    .detail("relative_path", &asset.relative_path),
                );
                continue;
            }
            Err(error) => {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_METADATA_UNREADABLE",
                        IntegrityFindingSeverity::Error,
                    )
                    .affected(asset.id.clone())
                    .detail("relative_path", &asset.relative_path)
                    .detail("io_error", error.to_string()),
                );
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ASSET_PATH_NOT_REGULAR_FILE",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(asset.id.clone())
                .detail("relative_path", &asset.relative_path),
            );
            continue;
        }
        let canonical_candidate = match candidate.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_PATH_UNRESOLVABLE",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(asset.id.clone())
                    .detail("relative_path", &asset.relative_path)
                    .detail("io_error", error.to_string()),
                );
                continue;
            }
        };
        if !canonical_candidate.starts_with(canonical_root) {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ASSET_PATH_ESCAPES_LIBRARY",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(asset.id.clone())
                .detail("relative_path", &asset.relative_path)
                .detail("canonical_path", canonical_candidate.to_string_lossy()),
            );
            continue;
        }
        if metadata.len() != asset.byte_length {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ASSET_LENGTH_MISMATCH",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(asset.id.clone())
                .detail("relative_path", &asset.relative_path)
                .detail("recorded_bytes", asset.byte_length.to_string())
                .detail("actual_bytes", metadata.len().to_string()),
            );
        }
        if options.verify_asset_hashes {
            let actual = hash_file(
                &canonical_candidate,
                options,
                cancellation,
                report,
            )?;
            if let Some(actual) = actual
                && !actual.eq_ignore_ascii_case(&asset.sha256)
            {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_HASH_MISMATCH",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(asset.id.clone())
                    .detail("relative_path", &asset.relative_path)
                    .detail("recorded_sha256", &asset.sha256)
                    .detail("actual_sha256", actual),
                );
            }
        }
    }
    Ok(())
}

fn hash_file(
    path: &Path,
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<Option<String>, A2dError> {
    let mut file = File::open(path).map_err(|error| {
        integrity_request_error(
            "STORAGE_INTEGRITY_HASH_OPEN_FAILED",
            format!("failed to open asset for hashing: {error}"),
        )
        .with_detail("path", path.to_string_lossy())
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let read = file.read(&mut buffer).map_err(|error| {
            integrity_request_error(
                "STORAGE_INTEGRITY_HASH_READ_FAILED",
                format!("failed to read asset for hashing: {error}"),
            )
            .with_detail("path", path.to_string_lossy())
        })?;
        if read == 0 {
            break;
        }
        let read_u64 = u64::try_from(read)
            .map_err(|_| integrity_limit_error("hash read size did not fit u64"))?;
        report.bytes_hashed = report
            .bytes_hashed
            .checked_add(read_u64)
            .ok_or_else(|| integrity_limit_error("hash byte counter overflowed"))?;
        if report.bytes_hashed > options.maximum_hash_bytes {
            return Err(integrity_limit_error(
                "integrity check exceeded maximum_hash_bytes",
            )
            .with_detail(
                "maximum_hash_bytes",
                options.maximum_hash_bytes.to_string(),
            ));
        }
        digest.update(&buffer[..read]);
    }
    Ok(Some(format!("{:x}", digest.finalize())))
}

fn check_orphan_final_files(
    canonical_root: &Path,
    known_asset_paths: &BTreeSet<String>,
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<(), A2dError> {
    let assets_root = canonical_root.join("assets");
    for file in bounded_files(&assets_root, options, cancellation, report)? {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        let relative = file.strip_prefix(canonical_root).map_err(|_| {
            integrity_request_error(
                "STORAGE_INTEGRITY_PATH_RELATIVIZE_FAILED",
                "asset path could not be made relative to the library root",
            )
            .with_detail("path", file.to_string_lossy())
        })?;
        let normalized = normalize_path(relative);
        if !known_asset_paths.contains(&normalized) {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ORPHAN_FINAL_ASSET",
                    IntegrityFindingSeverity::Warning,
                )
                .detail("relative_path", normalized),
            );
        }
    }
    Ok(())
}

fn check_temp_files(
    canonical_root: &Path,
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<(), A2dError> {
    let tmp_root = canonical_root.join("tmp");
    for file in bounded_files(&tmp_root, options, cancellation, report)? {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        let relative = file.strip_prefix(canonical_root).map_err(|_| {
            integrity_request_error(
                "STORAGE_INTEGRITY_PATH_RELATIVIZE_FAILED",
                "temp path could not be made relative to the library root",
            )
            .with_detail("path", file.to_string_lossy())
        })?;
        let normalized = normalize_path(relative);
        let code = if normalized.starts_with("tmp/scanner-staging/") {
            "INTEGRITY_RECOVERABLE_SCANNER_STAGING_FILE"
        } else if normalized.starts_with("tmp/asset-commit-journals/") {
            "INTEGRITY_INCOMPLETE_ASSET_COMMIT_JOURNAL"
        } else {
            "INTEGRITY_ORPHAN_TEMP_FILE"
        };
        report.findings.push(
            IntegrityFinding::new(code, IntegrityFindingSeverity::Warning)
                .detail("relative_path", normalized),
        );
    }
    Ok(())
}

fn bounded_files(
    root: &Path,
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<Vec<PathBuf>, A2dError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = stack.pop() {
        if cancellation.is_cancelled() {
            break;
        }
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_FILESYSTEM_DIRECTORY_UNREADABLE",
                        IntegrityFindingSeverity::Error,
                    )
                    .detail("directory", directory.to_string_lossy())
                    .detail("io_error", error.to_string()),
                );
                continue;
            }
        };
        for entry in entries {
            if cancellation.is_cancelled() {
                break;
            }
            bump_filesystem_entries(report, options)?;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report.findings.push(
                        IntegrityFinding::new(
                            "INTEGRITY_FILESYSTEM_ENTRY_UNREADABLE",
                            IntegrityFindingSeverity::Error,
                        )
                        .detail("directory", directory.to_string_lossy())
                        .detail("io_error", error.to_string()),
                    );
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    report.findings.push(
                        IntegrityFinding::new(
                            "INTEGRITY_FILESYSTEM_METADATA_UNREADABLE",
                            IntegrityFindingSeverity::Error,
                        )
                        .detail("path", path.to_string_lossy())
                        .detail("io_error", error.to_string()),
                    );
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_FILESYSTEM_SYMLINK_UNEXPECTED",
                        IntegrityFindingSeverity::Critical,
                    )
                    .detail("path", path.to_string_lossy()),
                );
            } else if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() {
                files.push(path);
            } else {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_FILESYSTEM_SPECIAL_FILE_UNEXPECTED",
                        IntegrityFindingSeverity::Critical,
                    )
                    .detail("path", path.to_string_lossy()),
                );
            }
        }
    }
    Ok(files)
}

fn valid_scan_fingerprint(value: &str) -> bool {
    let Some(body) = value.strip_prefix(SCAN_CONTENT_PREFIX) else {
        return false;
    };
    let Some((sha256, perceptual)) = body.split_once(PERCEPTUAL_PREFIX) else {
        return false;
    };
    sha256.len() == SHA256_HEX_LENGTH
        && sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        && perceptual.len() == PERCEPTUAL_HEX_LENGTH
        && perceptual.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn fingerprint_format_hint(value: &str) -> String {
    if value.is_empty() {
        "empty".to_string()
    } else if value.len() > 128 {
        format!("prefix={:?};length={}", &value[..64], value.len())
    } else {
        value.to_string()
    }
}

fn normalize_relative_path(value: &str) -> String {
    value.replace('\\', "/")
}

fn normalize_path(value: &Path) -> String {
    value
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn integrity_request_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.integrity_check",
        message.into(),
        false,
    )
}

fn integrity_limit_error(message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_INTEGRITY_LIMIT_EXCEEDED"),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.integrity_check_limit",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_domain::{
        Asset, AssetId, AssetKind, EncryptionState, PageId, system_now_ms,
    };

    use super::*;
    use crate::AssetRepository;

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "a2d-integrity-{name}-{}",
            PageId::generate()
        ))
    }

    fn open_storage(name: &str) -> (Storage, PathBuf) {
        let root = test_root(name);
        std::fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(&root.join("library.sqlite")).unwrap();
        (storage, root)
    }

    fn asset_row(root: &Path, bytes: &[u8], recorded_hash: String) -> Asset {
        let id = AssetId::generate();
        let relative_path = format!("assets/originals/{id}");
        let path = root.join(&relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        Asset::new(
            id,
            AssetKind::Original,
            relative_path,
            "image/jpeg".to_string(),
            bytes.len() as u64,
            recorded_hash,
            system_now_ms().unwrap(),
            true,
            EncryptionState::Plaintext,
        )
    }

    fn run(
        storage: &Storage,
        root: &Path,
        verify_hashes: bool,
    ) -> IntegrityCheckOutcome {
        storage
            .check_integrity(
                root,
                IntegrityCheckOptions {
                    verify_asset_hashes: verify_hashes,
                    ..IntegrityCheckOptions::default()
                },
                &IntegrityCancellation::active(),
            )
            .unwrap()
    }

    #[test]
    fn clean_library_reports_no_findings() {
        let (storage, root) = open_storage("clean");
        let IntegrityCheckOutcome::Completed(report) = run(&storage, &root, true) else {
            panic!("clean check was unexpectedly cancelled")
        };
        assert!(report.is_clean(), "findings: {:?}", report.findings);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_asset_file_is_reported() {
        let (storage, root) = open_storage("missing");
        let id = AssetId::generate();
        storage
            .insert_asset(&Asset::new(
                id.clone(),
                AssetKind::Original,
                format!("assets/originals/{id}"),
                "image/jpeg".to_string(),
                1,
                sha256_bytes(b"x"),
                system_now_ms().unwrap(),
                true,
                EncryptionState::Plaintext,
            ))
            .unwrap();
        let IntegrityCheckOutcome::Completed(report) = run(&storage, &root, false) else {
            panic!("check was unexpectedly cancelled")
        };
        assert!(report.findings.iter().any(|finding| {
            finding.code == "INTEGRITY_ASSET_FILE_MISSING"
                && finding.affected_id.as_deref() == Some(id.to_string().as_str())
        }));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hash_mismatch_is_reported_only_when_requested() {
        let (storage, root) = open_storage("hash");
        let asset = asset_row(&root, b"actual", sha256_bytes(b"different"));
        storage.insert_asset(&asset).unwrap();
        let IntegrityCheckOutcome::Completed(report) = run(&storage, &root, true) else {
            panic!("check was unexpectedly cancelled")
        };
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "INTEGRITY_ASSET_HASH_MISMATCH")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn orphan_final_asset_is_reported_without_deletion() {
        let (storage, root) = open_storage("orphan-final");
        let path = root.join("assets/originals/orphan");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"orphan").unwrap();
        let IntegrityCheckOutcome::Completed(report) = run(&storage, &root, false) else {
            panic!("check was unexpectedly cancelled")
        };
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "INTEGRITY_ORPHAN_FINAL_ASSET")
        );
        assert!(path.is_file());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn foreign_key_violation_fixture_is_reported() {
        let (storage, root) = open_storage("foreign-key");
        storage.conn.pragma_update(None, "foreign_keys", false).unwrap();
        storage
            .conn
            .execute(
                "INSERT INTO pages (
                    id, kind, notebook_id, notebook_design_id, logical_page_number,
                    smart_page_id, page_set_id, visible_page_number, layout_id, title,
                    state, preferred_scan_id, generated_pdf_asset_id, created_at_ms, updated_at_ms
                 ) VALUES (?1, 'notebook_page', ?2, ?3, 1, NULL, NULL, NULL,
                           'USLETTER-LINED', NULL, 'Unscanned', NULL, NULL, 1, 1)",
                rusqlite::params![
                    PageId::generate().to_string(),
                    "00000000000000000000000000",
                    "00000000000000000000000001",
                ],
            )
            .unwrap();
        storage.conn.pragma_update(None, "foreign_keys", true).unwrap();
        let IntegrityCheckOutcome::Completed(report) = run(&storage, &root, false) else {
            panic!("check was unexpectedly cancelled")
        };
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "INTEGRITY_FOREIGN_KEY_VIOLATION")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cancellation_is_a_distinct_outcome() {
        let (storage, root) = open_storage("cancel");
        let cancellation = IntegrityCancellation::active();
        cancellation.cancel();
        let outcome = storage
            .check_integrity(
                &root,
                IntegrityCheckOptions::default(),
                &cancellation,
            )
            .unwrap();
        assert!(matches!(outcome, IntegrityCheckOutcome::Cancelled(_)));
        std::fs::remove_dir_all(root).ok();
    }
}
