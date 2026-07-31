//! Bounded, cancellable, non-destructive canonical-library integrity diagnostics.
//!
//! This module reports stable findings only. It never repairs, deletes, imports, or rewrites
//! canonical data. Database rows, filesystem entries, and hashed bytes all have explicit limits.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
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
            return Err(request_error(
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
    /// Reserved for the future search milestone; v0.1 does not claim a search index exists.
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
        let root = library_root.canonicalize().map_err(|error| {
            request_error(
                "STORAGE_INTEGRITY_LIBRARY_ROOT_INVALID",
                format!("failed to canonicalize library root: {error}"),
            )
            .with_detail("library_root", library_root.to_string_lossy())
        })?;
        if !root.is_dir() {
            return Err(request_error(
                "STORAGE_INTEGRITY_LIBRARY_ROOT_NOT_DIRECTORY",
                "integrity-check library root must be a directory",
            ));
        }

        let mut report = IntegrityReport::new(self.schema_version()?, options.verify_asset_hashes);
        if cancellation.is_cancelled() {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }

        self.check_migrations(&mut report, options)?;
        self.check_foreign_keys(&mut report, options)?;
        self.check_relational_invariants(&mut report, options)?;
        if cancellation.is_cancelled() {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }

        let assets = self.load_assets(&mut report, options)?;
        let known_paths = assets
            .iter()
            .map(|asset| normalize_relative(&asset.relative_path))
            .collect::<BTreeSet<_>>();
        check_asset_rows(&root, &assets, options, cancellation, &mut report)?;
        if cancellation.is_cancelled() {
            return Ok(IntegrityCheckOutcome::Cancelled(report));
        }
        check_tree(
            &root,
            &root.join("assets"),
            Some(&known_paths),
            options,
            cancellation,
            &mut report,
        )?;
        check_tree(
            &root,
            &root.join("tmp"),
            None,
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

    fn check_migrations(
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
            bump_db(report, options)?;
            applied.push(row.map_err(|error| map_rusqlite_error("decoding migration row", error))?);
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
            .map_err(|error| map_rusqlite_error("preparing foreign-key check", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("running foreign-key check", error))?;
        for row in rows {
            bump_db(report, options)?;
            let (table, row_id, parent, index) =
                row.map_err(|error| map_rusqlite_error("decoding foreign-key finding", error))?;
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
                .detail("foreign_key_index", index.to_string()),
            );
        }
        Ok(())
    }

    fn check_relational_invariants(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        self.query_id_findings(
            "SELECT p.id FROM pages p WHERE
               (p.preferred_scan_id IS NULL AND
                (SELECT COUNT(*) FROM scans s WHERE s.page_id=p.id AND s.preferred=1) != 0)
               OR
               (p.preferred_scan_id IS NOT NULL AND (
                  (SELECT COUNT(*) FROM scans s WHERE s.page_id=p.id AND s.preferred=1) != 1
                  OR NOT EXISTS (
                    SELECT 1 FROM scans s
                    WHERE s.id=p.preferred_scan_id AND s.page_id=p.id AND s.preferred=1
                  )
               ))",
            "INTEGRITY_PREFERRED_SCAN_CONTRADICTION",
            report,
            options,
        )?;
        self.query_id_findings(
            "SELECT id FROM pages WHERE
               (kind='notebook_page' AND (
                  notebook_id IS NULL OR notebook_design_id IS NULL OR
                  logical_page_number IS NULL OR smart_page_id IS NOT NULL OR
                  page_set_id IS NOT NULL OR visible_page_number IS NOT NULL
               )) OR
               (kind='smart_page' AND (
                  notebook_id IS NOT NULL OR notebook_design_id IS NOT NULL OR
                  logical_page_number IS NOT NULL OR smart_page_id IS NULL
               )) OR kind NOT IN ('notebook_page','smart_page')",
            "INTEGRITY_PAGE_KIND_COLUMNS_INVALID",
            report,
            options,
        )?;
        self.query_id_findings(
            "SELECT s.id FROM scans s LEFT JOIN assets a ON a.id=s.original_asset_id
             WHERE a.id IS NULL OR a.kind!='Original' OR a.immutable!=1",
            "INTEGRITY_SCAN_ORIGINAL_INVALID",
            report,
            options,
        )?;
        self.query_id_findings(
            "SELECT p.id FROM pages p LEFT JOIN assets a ON a.id=p.generated_pdf_asset_id
             WHERE p.generated_pdf_asset_id IS NOT NULL
               AND (a.id IS NULL OR a.kind!='Export')",
            "INTEGRITY_GENERATED_PDF_REFERENCE_INVALID",
            report,
            options,
        )?;

        bump_db(report, options)?;
        let active_count = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM notebooks WHERE active_scan_destination=1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| map_rusqlite_error("counting active notebooks", error))?;
        if active_count > 1 {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_MULTIPLE_ACTIVE_NOTEBOOKS",
                    IntegrityFindingSeverity::Critical,
                )
                .detail("active_notebook_count", active_count.to_string()),
            );
        }

        let mut statement = self
            .conn
            .prepare("SELECT id, content_fingerprint FROM scans")
            .map_err(|error| map_rusqlite_error("preparing fingerprint check", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| map_rusqlite_error("reading fingerprints", error))?;
        for row in rows {
            bump_db(report, options)?;
            let (scan_id, fingerprint) =
                row.map_err(|error| map_rusqlite_error("decoding fingerprint row", error))?;
            if !valid_scan_fingerprint(&fingerprint) {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_SCAN_FINGERPRINT_INVALID",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(scan_id)
                    .detail("format", fingerprint_hint(&fingerprint)),
                );
            }
        }
        Ok(())
    }

    fn query_id_findings(
        &self,
        sql: &str,
        code: &'static str,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<(), A2dError> {
        let mut statement = self
            .conn
            .prepare(sql)
            .map_err(|error| map_rusqlite_error("preparing integrity query", error))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| map_rusqlite_error("running integrity query", error))?;
        for row in rows {
            bump_db(report, options)?;
            let id = row.map_err(|error| map_rusqlite_error("decoding integrity row", error))?;
            report
                .findings
                .push(IntegrityFinding::new(code, IntegrityFindingSeverity::Critical).affected(id));
        }
        Ok(())
    }

    fn load_assets(
        &self,
        report: &mut IntegrityReport,
        options: IntegrityCheckOptions,
    ) -> Result<Vec<AssetRow>, A2dError> {
        let mut statement = self
            .conn
            .prepare("SELECT id, relative_path, byte_length, sha256 FROM assets ORDER BY id")
            .map_err(|error| map_rusqlite_error("preparing asset check", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("reading asset rows", error))?;
        let mut assets = Vec::new();
        for row in rows {
            bump_db(report, options)?;
            let (id, relative_path, byte_length, sha256) =
                row.map_err(|error| map_rusqlite_error("decoding asset row", error))?;
            let Ok(byte_length) = u64::try_from(byte_length) else {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_LENGTH_INVALID",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(id),
                );
                continue;
            };
            assets.push(AssetRow {
                id,
                relative_path,
                byte_length,
                sha256,
            });
        }
        Ok(assets)
    }
}

fn bump_db(report: &mut IntegrityReport, options: IntegrityCheckOptions) -> Result<(), A2dError> {
    report.database_rows_examined = report
        .database_rows_examined
        .checked_add(1)
        .ok_or_else(|| limit_error("database row counter overflowed"))?;
    if report.database_rows_examined > options.maximum_database_rows {
        return Err(
            limit_error("integrity check exceeded maximum_database_rows").with_detail(
                "maximum_database_rows",
                options.maximum_database_rows.to_string(),
            ),
        );
    }
    Ok(())
}

fn bump_fs(report: &mut IntegrityReport, options: IntegrityCheckOptions) -> Result<(), A2dError> {
    report.filesystem_entries_examined = report
        .filesystem_entries_examined
        .checked_add(1)
        .ok_or_else(|| limit_error("filesystem entry counter overflowed"))?;
    if report.filesystem_entries_examined > options.maximum_filesystem_entries {
        return Err(
            limit_error("integrity check exceeded maximum_filesystem_entries").with_detail(
                "maximum_filesystem_entries",
                options.maximum_filesystem_entries.to_string(),
            ),
        );
    }
    Ok(())
}

fn check_asset_rows(
    root: &Path,
    assets: &[AssetRow],
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<(), A2dError> {
    for asset in assets {
        if cancellation.is_cancelled() {
            break;
        }
        bump_fs(report, options)?;
        let candidate = root.join(&asset.relative_path);
        let metadata = match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_ASSET_FILE_MISSING",
                        IntegrityFindingSeverity::Critical,
                    )
                    .affected(&asset.id)
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
                    .affected(&asset.id)
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
                .affected(&asset.id),
            );
            continue;
        }
        let canonical = candidate.canonicalize().map_err(|error| {
            request_error(
                "STORAGE_INTEGRITY_ASSET_PATH_UNRESOLVABLE",
                format!("failed to canonicalize asset path: {error}"),
            )
            .with_detail("asset_id", &asset.id)
        })?;
        if !canonical.starts_with(root) {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ASSET_PATH_ESCAPES_LIBRARY",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(&asset.id),
            );
            continue;
        }
        if metadata.len() != asset.byte_length {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ASSET_LENGTH_MISMATCH",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(&asset.id)
                .detail("recorded_bytes", asset.byte_length.to_string())
                .detail("actual_bytes", metadata.len().to_string()),
            );
        }
        if options.verify_asset_hashes
            && let Some(actual) = hash_file(&canonical, options, cancellation, report)?
            && !actual.eq_ignore_ascii_case(&asset.sha256)
        {
            report.findings.push(
                IntegrityFinding::new(
                    "INTEGRITY_ASSET_HASH_MISMATCH",
                    IntegrityFindingSeverity::Critical,
                )
                .affected(&asset.id)
                .detail("recorded_sha256", &asset.sha256)
                .detail("actual_sha256", actual),
            );
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
        request_error(
            "STORAGE_INTEGRITY_HASH_OPEN_FAILED",
            format!("failed to open asset for hashing: {error}"),
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let read = file.read(&mut buffer).map_err(|error| {
            request_error(
                "STORAGE_INTEGRITY_HASH_READ_FAILED",
                format!("failed to read asset for hashing: {error}"),
            )
        })?;
        if read == 0 {
            break;
        }
        report.bytes_hashed = report
            .bytes_hashed
            .checked_add(u64::try_from(read).map_err(|_| limit_error("hash size overflowed"))?)
            .ok_or_else(|| limit_error("hash byte counter overflowed"))?;
        if report.bytes_hashed > options.maximum_hash_bytes {
            return Err(limit_error("integrity check exceeded maximum_hash_bytes")
                .with_detail("maximum_hash_bytes", options.maximum_hash_bytes.to_string()));
        }
        digest.update(&buffer[..read]);
    }
    Ok(Some(format!("{:x}", digest.finalize())))
}

fn check_tree(
    library_root: &Path,
    tree_root: &Path,
    known_assets: Option<&BTreeSet<String>>,
    options: IntegrityCheckOptions,
    cancellation: &IntegrityCancellation,
    report: &mut IntegrityReport,
) -> Result<(), A2dError> {
    if !tree_root.exists() {
        return Ok(());
    }
    let mut queue = VecDeque::from([tree_root.to_path_buf()]);
    while let Some(directory) = queue.pop_front() {
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
            bump_fs(report, options)?;
            let entry = entry.map_err(|error| {
                request_error(
                    "STORAGE_INTEGRITY_READ_DIRECTORY_FAILED",
                    format!("failed to read filesystem entry: {error}"),
                )
            })?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                request_error(
                    "STORAGE_INTEGRITY_METADATA_FAILED",
                    format!("failed to read filesystem metadata: {error}"),
                )
            })?;
            if metadata.file_type().is_symlink() {
                report.findings.push(
                    IntegrityFinding::new(
                        "INTEGRITY_FILESYSTEM_SYMLINK_UNEXPECTED",
                        IntegrityFindingSeverity::Critical,
                    )
                    .detail("path", path.to_string_lossy()),
                );
            } else if metadata.is_dir() {
                queue.push_back(path);
            } else if metadata.is_file() {
                let relative = normalize_path(path.strip_prefix(library_root).map_err(|_| {
                    request_error(
                        "STORAGE_INTEGRITY_PATH_RELATIVIZE_FAILED",
                        "filesystem path escaped the library root",
                    )
                })?);
                if let Some(known_assets) = known_assets {
                    if !known_assets.contains(&relative) {
                        report.findings.push(
                            IntegrityFinding::new(
                                "INTEGRITY_ORPHAN_FINAL_ASSET",
                                IntegrityFindingSeverity::Warning,
                            )
                            .detail("relative_path", relative),
                        );
                    }
                } else {
                    let code = if relative.starts_with("tmp/scanner-staging/") {
                        "INTEGRITY_RECOVERABLE_SCANNER_STAGING_FILE"
                    } else if relative.starts_with("tmp/asset-commit-journals/") {
                        "INTEGRITY_INCOMPLETE_ASSET_COMMIT_JOURNAL"
                    } else {
                        "INTEGRITY_ORPHAN_TEMP_FILE"
                    };
                    report.findings.push(
                        IntegrityFinding::new(code, IntegrityFindingSeverity::Warning)
                            .detail("relative_path", relative),
                    );
                }
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
    Ok(())
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

fn fingerprint_hint(value: &str) -> String {
    if value.is_empty() {
        "empty".to_string()
    } else if value.chars().count() > 128 {
        let prefix = value.chars().take(64).collect::<String>();
        format!("prefix={prefix:?};chars={}", value.chars().count())
    } else {
        value.to_string()
    }
}

fn normalize_relative(value: &str) -> String {
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

fn request_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.integrity_check",
        message.into(),
        false,
    )
}

fn limit_error(message: impl Into<String>) -> A2dError {
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
    use a2d_domain::{Asset, AssetId, AssetKind, EncryptionState, PageId, system_now_ms};

    use super::*;
    use crate::AssetRepository;

    fn open_storage(name: &str) -> (Storage, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("a2d-integrity-{name}-{}", PageId::generate()));
        std::fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(&root.join("library.sqlite")).unwrap();
        (storage, root)
    }

    fn run(storage: &Storage, root: &Path, verify_hashes: bool) -> IntegrityCheckOutcome {
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

    fn asset(root: &Path, bytes: &[u8], hash: String) -> Asset {
        let id = AssetId::generate();
        let relative_path = format!("assets/originals/{id}");
        let path = root.join(&relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        Asset::new(
            id,
            AssetKind::Original,
            relative_path,
            "image/jpeg".to_string(),
            bytes.len() as u64,
            hash,
            system_now_ms().unwrap(),
            true,
            EncryptionState::Plaintext,
        )
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
        let expected_id = id.to_string();
        storage
            .insert_asset(&Asset::new(
                id,
                AssetKind::Original,
                format!("assets/originals/{expected_id}"),
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
                && finding.affected_id.as_deref() == Some(expected_id.as_str())
        }));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hash_mismatch_is_reported_only_when_requested() {
        let (storage, root) = open_storage("hash");
        let asset = asset(&root, b"actual", sha256_bytes(b"different"));
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
        let (storage, root) = open_storage("orphan");
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
    fn foreign_key_violation_is_reported() {
        let (storage, root) = open_storage("foreign-key");
        storage
            .conn
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        storage
            .conn
            .execute(
                "INSERT INTO pages (
                    id,kind,notebook_id,notebook_design_id,logical_page_number,
                    smart_page_id,page_set_id,visible_page_number,layout_id,title,state,
                    preferred_scan_id,generated_pdf_asset_id,created_at_ms,updated_at_ms
                 ) VALUES (?1,'notebook_page',?2,?3,1,NULL,NULL,NULL,
                           'USLETTER-LINED',NULL,'Unscanned',NULL,NULL,1,1)",
                rusqlite::params![
                    PageId::generate().to_string(),
                    "00000000000000000000000000",
                    "00000000000000000000000001",
                ],
            )
            .unwrap();
        storage
            .conn
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
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
    fn cancellation_is_distinct() {
        let (storage, root) = open_storage("cancel");
        let cancellation = IntegrityCancellation::active();
        cancellation.cancel();
        let outcome = storage
            .check_integrity(&root, IntegrityCheckOptions::default(), &cancellation)
            .unwrap();
        assert!(matches!(outcome, IntegrityCheckOutcome::Cancelled(_)));
        std::fs::remove_dir_all(root).ok();
    }
}
