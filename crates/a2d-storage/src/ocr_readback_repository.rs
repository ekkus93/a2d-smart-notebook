//! Readback repository for persisted OCR output summaries.
//!
//! This module keeps SQL private to `a2d-storage` while exposing the scan-scoped query Milestone 11
//! needs to hydrate Android/Page Viewer state after app restart. It deliberately returns typed
//! domain rows rather than SQL-shaped DTOs.

use a2d_domain::{
    A2dError, AssetId, OcrRun, OcrRunId, OcrRunStatus, OcrUnavailableReason, PageId, Provenance,
    ScanId,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::json_columns::decode_json;
use crate::{Storage, map_rusqlite_error};

pub trait OcrReadbackRepository {
    fn latest_ocr_run_for_scan(&self, scan_id: &ScanId) -> Result<Option<OcrRun>, A2dError>;
    fn count_text_regions_for_ocr_run(&self, ocr_run_id: &OcrRunId) -> Result<usize, A2dError>;
}

impl OcrReadbackRepository for Storage {
    fn latest_ocr_run_for_scan(&self, scan_id: &ScanId) -> Result<Option<OcrRun>, A2dError> {
        OcrReadbackRepository::latest_ocr_run_for_scan(&self.conn, scan_id)
    }

    fn count_text_regions_for_ocr_run(&self, ocr_run_id: &OcrRunId) -> Result<usize, A2dError> {
        OcrReadbackRepository::count_text_regions_for_ocr_run(&self.conn, ocr_run_id)
    }
}

impl OcrReadbackRepository for Connection {
    fn latest_ocr_run_for_scan(&self, scan_id: &ScanId) -> Result<Option<OcrRun>, A2dError> {
        self.query_row(
            "SELECT id, scan_id, input_asset_id, provider, provider_version, model_name, \
             status, full_text, unavailable_reason, unavailable_message, completed_at_ms, \
             warnings, provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved \
             FROM ocr_runs WHERE scan_id = ?1 \
             ORDER BY COALESCE(completed_at_ms, provenance_created_at_ms) DESC, \
                      provenance_created_at_ms DESC, id DESC \
             LIMIT 1",
            [scan_id.to_string()],
            ocr_run_row,
        )
        .optional()
        .map_err(|error| map_rusqlite_error("latest_ocr_run_for_scan", error))?
        .map(ocr_run_from_row)
        .transpose()
    }

    fn count_text_regions_for_ocr_run(&self, ocr_run_id: &OcrRunId) -> Result<usize, A2dError> {
        let count = self
            .query_row(
                "SELECT COUNT(*) FROM text_regions WHERE ocr_run_id = ?1",
                [ocr_run_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| map_rusqlite_error("count_text_regions_for_ocr_run", error))?;
        Ok(count as usize)
    }
}

#[allow(clippy::type_complexity)]
fn ocr_run_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    i64,
    String,
    Option<bool>,
)> {
    Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, Option<String>>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, Option<String>>(5)?,
        row.get::<_, String>(6)?,
        row.get::<_, String>(7)?,
        row.get::<_, Option<String>>(8)?,
        row.get::<_, Option<String>>(9)?,
        row.get::<_, Option<i64>>(10)?,
        row.get::<_, String>(11)?,
        row.get::<_, Option<String>>(12)?,
        row.get::<_, Option<String>>(13)?,
        row.get::<_, String>(14)?,
        row.get::<_, String>(15)?,
        row.get::<_, i64>(16)?,
        row.get::<_, String>(17)?,
        row.get::<_, Option<bool>>(18)?,
    ))
}

#[allow(clippy::type_complexity)]
fn ocr_run_from_row(
    row: (
        String,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        i64,
        String,
        Option<bool>,
    ),
) -> Result<OcrRun, A2dError> {
    let (
        id,
        scan_id,
        input_asset_id,
        provider,
        provider_version,
        model_name,
        status,
        full_text,
        unavailable_reason,
        unavailable_message,
        completed_at_ms,
        warnings,
        prov_source_page_id,
        prov_source_scan_id,
        prov_producing_component,
        prov_component_version,
        prov_created_at_ms,
        prov_warnings,
        prov_user_approved,
    ) = row;
    OcrRun::from_stored(
        OcrRunId::parse(&id)?,
        ScanId::parse(&scan_id)?,
        input_asset_id
            .map(|value| AssetId::parse(&value))
            .transpose()?,
        provider,
        provider_version,
        model_name,
        ocr_run_status_from_str(&status)?,
        full_text,
        unavailable_reason
            .map(|value| ocr_unavailable_reason_from_str(&value))
            .transpose()?,
        unavailable_message,
        completed_at_ms,
        decode_json(&warnings, "warnings")?,
        Provenance {
            source_page_id: prov_source_page_id
                .map(|value| PageId::parse(&value))
                .transpose()?,
            source_scan_id: prov_source_scan_id
                .map(|value| ScanId::parse(&value))
                .transpose()?,
            producing_component: prov_producing_component,
            component_version: prov_component_version,
            created_at_ms: prov_created_at_ms,
            warnings: decode_json(&prov_warnings, "provenance_warnings")?,
            user_approved: prov_user_approved,
        },
    )
}

fn ocr_run_status_from_str(raw: &str) -> Result<OcrRunStatus, A2dError> {
    match raw {
        "Detected" => Ok(OcrRunStatus::Detected),
        "NoTextDetected" => Ok(OcrRunStatus::NoTextDetected),
        "Unavailable" => Ok(OcrRunStatus::Unavailable),
        other => Err(a2d_domain::A2dError::new(
            a2d_domain::ErrorCode::new("STORAGE_OCR_RUN_STATUS_CORRUPT"),
            a2d_domain::ErrorCategory::Integrity,
            a2d_domain::ErrorSeverity::Critical,
            "error.storage.ocr_run_status_corrupt",
            "OCR run has an unknown status during readback",
            false,
        )
        .with_detail("ocr_run_status", other)),
    }
}

fn ocr_unavailable_reason_from_str(raw: &str) -> Result<OcrUnavailableReason, A2dError> {
    match raw {
        "ProviderUnavailable" => Ok(OcrUnavailableReason::ProviderUnavailable),
        "ProviderFailed" => Ok(OcrUnavailableReason::ProviderFailed),
        "ResourceUnavailable" => Ok(OcrUnavailableReason::ResourceUnavailable),
        "UnsupportedInput" => Ok(OcrUnavailableReason::UnsupportedInput),
        "Cancelled" => Ok(OcrUnavailableReason::Cancelled),
        other => Err(a2d_domain::A2dError::new(
            a2d_domain::ErrorCode::new("STORAGE_OCR_UNAVAILABLE_REASON_CORRUPT"),
            a2d_domain::ErrorCategory::Integrity,
            a2d_domain::ErrorSeverity::Critical,
            "error.storage.ocr_unavailable_reason_corrupt",
            "OCR run has an unknown unavailable reason during readback",
            false,
        )
        .with_detail("ocr_unavailable_reason", other)),
    }
}
