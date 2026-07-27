//! Asset, scan, OCR, and audit repository implementations.

use a2d_domain::{
    A2dError, Asset, AssetId, AuditEvent, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity,
    OcrRun, OcrRunId, PageId, Scan, ScanId,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::json_columns::{decode_json, encode_json};

use super::{corrupt_enum_error, map_sql_error};

// ---------------------------------------------------------------------------------------------
// Asset
// ---------------------------------------------------------------------------------------------

pub trait AssetRepository {
    fn insert_asset(&self, asset: &Asset) -> Result<(), A2dError>;
    fn get_asset(&self, id: &AssetId) -> Result<Option<Asset>, A2dError>;
}

impl AssetRepository for Connection {
    fn insert_asset(&self, asset: &Asset) -> Result<(), A2dError> {
        self.execute(
            "INSERT INTO assets (id, kind, relative_path, media_type, byte_length, sha256, \
             created_at_ms, immutable, encryption_state) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                asset.id().to_string(),
                asset_kind_to_str(asset.kind),
                asset.relative_path,
                asset.media_type,
                asset.byte_length as i64,
                asset.sha256,
                asset.created_at_ms,
                asset.immutable,
                encryption_state_to_str(asset.encryption_state),
            ],
        )
        .map_err(|e| map_sql_error("insert_asset", e))?;
        Ok(())
    }

    fn get_asset(&self, id: &AssetId) -> Result<Option<Asset>, A2dError> {
        self.query_row(
            "SELECT id, kind, relative_path, media_type, byte_length, sha256, created_at_ms, \
             immutable, encryption_state FROM assets WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, bool>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_asset", e))?
        .map(
            |(
                id,
                kind,
                relative_path,
                media_type,
                byte_length,
                sha256,
                created_at_ms,
                immutable,
                encryption_state,
            )| {
                Ok(Asset::new(
                    AssetId::parse(&id)?,
                    asset_kind_from_str(&kind)?,
                    relative_path,
                    media_type,
                    byte_length as u64,
                    sha256,
                    created_at_ms,
                    immutable,
                    encryption_state_from_str(&encryption_state)?,
                ))
            },
        )
        .transpose()
    }
}

fn asset_kind_to_str(kind: a2d_domain::AssetKind) -> &'static str {
    use a2d_domain::AssetKind::*;
    match kind {
        Original => "Original",
        Corrected => "Corrected",
        Ocr => "Ocr",
        Thumbnail => "Thumbnail",
        Export => "Export",
    }
}

fn asset_kind_from_str(raw: &str) -> Result<a2d_domain::AssetKind, A2dError> {
    use a2d_domain::AssetKind::*;
    match raw {
        "Original" => Ok(Original),
        "Corrected" => Ok(Corrected),
        "Ocr" => Ok(Ocr),
        "Thumbnail" => Ok(Thumbnail),
        "Export" => Ok(Export),
        other => Err(corrupt_enum_error("assets.kind", other)),
    }
}

fn encryption_state_to_str(state: a2d_domain::EncryptionState) -> &'static str {
    use a2d_domain::EncryptionState::*;
    match state {
        Plaintext => "Plaintext",
        Encrypted => "Encrypted",
    }
}

fn encryption_state_from_str(raw: &str) -> Result<a2d_domain::EncryptionState, A2dError> {
    use a2d_domain::EncryptionState::*;
    match raw {
        "Plaintext" => Ok(Plaintext),
        "Encrypted" => Ok(Encrypted),
        other => Err(corrupt_enum_error("assets.encryption_state", other)),
    }
}

// ---------------------------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------------------------

pub trait ScanRepository {
    fn insert_scan(&self, scan: &Scan) -> Result<(), A2dError>;
    fn get_scan(&self, id: &ScanId) -> Result<Option<Scan>, A2dError>;
}

impl ScanRepository for Connection {
    fn insert_scan(&self, scan: &Scan) -> Result<(), A2dError> {
        // TODO 2.3's "a scan always references an immutable original asset" -- the half of that
        // invariant a2d-domain's Scan type couldn't check by itself (it only has the id, not the
        // referenced row). Checked here, not at read time, so an unenforced invariant can never
        // land in the database in the first place.
        let original_immutable: Option<bool> = self
            .query_row(
                "SELECT immutable FROM assets WHERE id = ?1",
                [scan.original_asset_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| map_sql_error("insert_scan (checking original asset)", e))?;
        match original_immutable {
            None => {
                return Err(A2dError::new(
                    ErrorCode::new("STORAGE_SCAN_ORIGINAL_ASSET_MISSING"),
                    ErrorCategory::Validation,
                    ErrorSeverity::Error,
                    "error.storage.scan_original_asset_missing",
                    "a scan's original_asset_id must reference an asset that already exists",
                    false,
                )
                .with_detail("original_asset_id", scan.original_asset_id.to_string()));
            }
            Some(false) => {
                return Err(A2dError::new(
                    ErrorCode::new("STORAGE_SCAN_ORIGINAL_ASSET_NOT_IMMUTABLE"),
                    ErrorCategory::Validation,
                    ErrorSeverity::Error,
                    "error.storage.scan_original_asset_not_immutable",
                    "a scan's original_asset_id must reference an asset marked immutable \
                     (spec sections 3.3 and 16.3)",
                    false,
                )
                .with_detail("original_asset_id", scan.original_asset_id.to_string()));
            }
            Some(true) => {}
        }

        let warnings = encode_json(&scan.warnings, "warnings")?;
        self.execute(
            "INSERT INTO scans (id, page_id, physical_copy_id, capture_source, captured_at_ms, \
             original_asset_id, corrected_asset_id, ocr_asset_id, thumbnail_asset_id, \
             pipeline_version, quality_status, warnings, preferred, supersedes_scan_id, \
             content_fingerprint) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                scan.id().to_string(),
                scan.page_id.to_string(),
                scan.physical_copy_id.as_ref().map(ToString::to_string),
                capture_source_to_str(scan.capture_source),
                scan.captured_at_ms,
                scan.original_asset_id.to_string(),
                scan.corrected_asset_id.as_ref().map(ToString::to_string),
                scan.ocr_asset_id.as_ref().map(ToString::to_string),
                scan.thumbnail_asset_id.as_ref().map(ToString::to_string),
                scan.pipeline_version,
                quality_status_to_str(scan.quality_status),
                warnings,
                scan.preferred,
                scan.supersedes_scan_id.as_ref().map(ToString::to_string),
                scan.content_fingerprint,
            ],
        )
        .map_err(|e| map_sql_error("insert_scan", e))?;
        Ok(())
    }

    fn get_scan(&self, id: &ScanId) -> Result<Option<Scan>, A2dError> {
        self.query_row(
            "SELECT id, page_id, physical_copy_id, capture_source, captured_at_ms, \
             original_asset_id, corrected_asset_id, ocr_asset_id, thumbnail_asset_id, \
             pipeline_version, quality_status, warnings, preferred, supersedes_scan_id, \
             content_fingerprint FROM scans WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, bool>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, String>(14)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_scan", e))?
        .map(
            |(
                id,
                page_id,
                physical_copy_id,
                capture_source,
                captured_at_ms,
                original_asset_id,
                corrected_asset_id,
                ocr_asset_id,
                thumbnail_asset_id,
                pipeline_version,
                quality_status,
                warnings,
                preferred,
                supersedes_scan_id,
                content_fingerprint,
            )| {
                Ok(Scan::new(
                    ScanId::parse(&id)?,
                    PageId::parse(&page_id)?,
                    physical_copy_id
                        .map(|s| a2d_domain::PhysicalCopyId::parse(&s))
                        .transpose()?,
                    capture_source_from_str(&capture_source)?,
                    captured_at_ms,
                    AssetId::parse(&original_asset_id)?,
                    corrected_asset_id.map(|s| AssetId::parse(&s)).transpose()?,
                    ocr_asset_id.map(|s| AssetId::parse(&s)).transpose()?,
                    thumbnail_asset_id.map(|s| AssetId::parse(&s)).transpose()?,
                    pipeline_version,
                    quality_status_from_str(&quality_status)?,
                    decode_json(&warnings, "warnings")?,
                    preferred,
                    supersedes_scan_id.map(|s| ScanId::parse(&s)).transpose()?,
                    content_fingerprint,
                ))
            },
        )
        .transpose()
    }
}

fn capture_source_to_str(source: a2d_domain::CaptureSource) -> &'static str {
    use a2d_domain::CaptureSource::*;
    match source {
        Camera => "Camera",
        Import => "Import",
    }
}

fn capture_source_from_str(raw: &str) -> Result<a2d_domain::CaptureSource, A2dError> {
    use a2d_domain::CaptureSource::*;
    match raw {
        "Camera" => Ok(Camera),
        "Import" => Ok(Import),
        other => Err(corrupt_enum_error("scans.capture_source", other)),
    }
}

fn quality_status_to_str(status: a2d_domain::QualityStatus) -> &'static str {
    use a2d_domain::QualityStatus::*;
    match status {
        Accepted => "Accepted",
        AcceptedWithWarnings => "AcceptedWithWarnings",
        NeedsReview => "NeedsReview",
        Rejected => "Rejected",
    }
}

fn quality_status_from_str(raw: &str) -> Result<a2d_domain::QualityStatus, A2dError> {
    use a2d_domain::QualityStatus::*;
    match raw {
        "Accepted" => Ok(Accepted),
        "AcceptedWithWarnings" => Ok(AcceptedWithWarnings),
        "NeedsReview" => Ok(NeedsReview),
        "Rejected" => Ok(Rejected),
        other => Err(corrupt_enum_error("scans.quality_status", other)),
    }
}

// ---------------------------------------------------------------------------------------------
// OcrRun
// ---------------------------------------------------------------------------------------------

pub trait OcrRunRepository {
    fn insert_ocr_run(&self, run: &OcrRun) -> Result<(), A2dError>;
    fn get_ocr_run(&self, id: &OcrRunId) -> Result<Option<OcrRun>, A2dError>;
}

impl OcrRunRepository for Connection {
    fn insert_ocr_run(&self, run: &OcrRun) -> Result<(), A2dError> {
        let warnings = encode_json(&run.warnings, "warnings")?;
        let provenance_warnings = encode_json(&run.provenance.warnings, "provenance_warnings")?;
        self.execute(
            "INSERT INTO ocr_runs (id, scan_id, provider, provider_version, full_text, \
             warnings, provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                run.id().to_string(),
                run.scan_id.to_string(),
                run.provider,
                run.provider_version,
                run.full_text,
                warnings,
                run.provenance
                    .source_page_id
                    .as_ref()
                    .map(ToString::to_string),
                run.provenance
                    .source_scan_id
                    .as_ref()
                    .map(ToString::to_string),
                run.provenance.producing_component,
                run.provenance.component_version,
                run.provenance.created_at_ms,
                provenance_warnings,
                run.provenance.user_approved,
            ],
        )
        .map_err(|e| map_sql_error("insert_ocr_run", e))?;
        Ok(())
    }

    fn get_ocr_run(&self, id: &OcrRunId) -> Result<Option<OcrRun>, A2dError> {
        self.query_row(
            "SELECT id, scan_id, provider, provider_version, full_text, warnings, \
             provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved \
             FROM ocr_runs WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<bool>>(12)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_ocr_run", e))?
        .map(
            |(
                id,
                scan_id,
                provider,
                provider_version,
                full_text,
                warnings,
                prov_source_page_id,
                prov_source_scan_id,
                prov_producing_component,
                prov_component_version,
                prov_created_at_ms,
                prov_warnings,
                prov_user_approved,
            )| {
                Ok(OcrRun::new(
                    OcrRunId::parse(&id)?,
                    ScanId::parse(&scan_id)?,
                    provider,
                    provider_version,
                    full_text,
                    decode_json(&warnings, "warnings")?,
                    a2d_domain::Provenance {
                        source_page_id: prov_source_page_id
                            .map(|s| PageId::parse(&s))
                            .transpose()?,
                        source_scan_id: prov_source_scan_id
                            .map(|s| ScanId::parse(&s))
                            .transpose()?,
                        producing_component: prov_producing_component,
                        component_version: prov_component_version,
                        created_at_ms: prov_created_at_ms,
                        warnings: decode_json(&prov_warnings, "provenance_warnings")?,
                        user_approved: prov_user_approved,
                    },
                ))
            },
        )
        .transpose()
    }
}

// ---------------------------------------------------------------------------------------------
// AuditEvent
// ---------------------------------------------------------------------------------------------

pub trait AuditEventRepository {
    fn insert_audit_event(&self, event: &AuditEvent) -> Result<(), A2dError>;
    fn get_audit_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>, A2dError>;
}

impl AuditEventRepository for Connection {
    fn insert_audit_event(&self, event: &AuditEvent) -> Result<(), A2dError> {
        let details = encode_json(&event.details, "details")?;
        self.execute(
            "INSERT INTO audit_events (id, occurred_at_ms, event_kind, actor, subject, \
             details, correlation_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.id().to_string(),
                event.occurred_at_ms,
                event.event_kind,
                event.actor,
                event.subject,
                details,
                event.correlation_id,
            ],
        )
        .map_err(|e| map_sql_error("insert_audit_event", e))?;
        Ok(())
    }

    fn get_audit_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>, A2dError> {
        self.query_row(
            "SELECT id, occurred_at_ms, event_kind, actor, subject, details, correlation_id \
             FROM audit_events WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_audit_event", e))?
        .map(
            |(id, occurred_at_ms, event_kind, actor, subject, details, correlation_id)| {
                Ok(AuditEvent::new(
                    AuditEventId::parse(&id)?,
                    occurred_at_ms,
                    event_kind,
                    actor,
                    subject,
                    decode_json(&details, "details")?,
                    correlation_id,
                ))
            },
        )
        .transpose()
    }
}
