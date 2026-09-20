//! Repository for persisted OCR text-region rows.
//!
//! The v1 schema already has `text_regions`; this module is the first public typed repository for
//! it. SQL remains private to storage, and callers can only attach regions to detected OCR runs so
//! unavailable/no-text runs cannot be misrepresented as recognized empty text.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, OcrRunId, TextRegion, TextRegionId,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::json_columns::{decode_json, encode_json};
use crate::{Storage, map_rusqlite_error};

/// Authoritative durable maximum for OCR regions belonging to one OCR run.
///
/// This mirrors the Rust product contract exposed by `a2d-core`; enforcing it at the storage write
/// boundary makes the limit cumulative across repeated batches and keeps every caller fail-closed.
const MAX_OCR_REGIONS_PER_RUN: i64 = 20_000;

pub trait TextRegionRepository {
    fn insert_text_region(&self, region: &TextRegion) -> Result<(), A2dError>;
    fn get_text_region(&self, id: &TextRegionId) -> Result<Option<TextRegion>, A2dError>;
    fn list_text_regions_for_ocr_run(
        &self,
        ocr_run_id: &OcrRunId,
    ) -> Result<Vec<TextRegion>, A2dError>;
}

impl TextRegionRepository for Storage {
    fn insert_text_region(&self, region: &TextRegion) -> Result<(), A2dError> {
        TextRegionRepository::insert_text_region(&self.conn, region)
    }

    fn get_text_region(&self, id: &TextRegionId) -> Result<Option<TextRegion>, A2dError> {
        TextRegionRepository::get_text_region(&self.conn, id)
    }

    fn list_text_regions_for_ocr_run(
        &self,
        ocr_run_id: &OcrRunId,
    ) -> Result<Vec<TextRegion>, A2dError> {
        TextRegionRepository::list_text_regions_for_ocr_run(&self.conn, ocr_run_id)
    }
}

impl TextRegionRepository for Connection {
    fn insert_text_region(&self, region: &TextRegion) -> Result<(), A2dError> {
        require_detected_ocr_run(self, &region.ocr_run_id)?;
        require_region_capacity(self, &region.ocr_run_id)?;
        let polygon = encode_json(&region.polygon, "text_regions.polygon")?;
        self.execute(
            "INSERT INTO text_regions (id, ocr_run_id, polygon, text, confidence, created_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                region.id().to_string(),
                region.ocr_run_id.to_string(),
                polygon,
                region.text,
                region.confidence,
                region.created_at_ms,
            ],
        )
        .map_err(|error| map_rusqlite_error("insert_text_region", error))?;
        Ok(())
    }

    fn get_text_region(&self, id: &TextRegionId) -> Result<Option<TextRegion>, A2dError> {
        self.query_row(
            "SELECT id, ocr_run_id, polygon, text, confidence, created_at_ms \
             FROM text_regions WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<f32>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_rusqlite_error("get_text_region", error))?
        .map(text_region_from_row)
        .transpose()
    }

    fn list_text_regions_for_ocr_run(
        &self,
        ocr_run_id: &OcrRunId,
    ) -> Result<Vec<TextRegion>, A2dError> {
        let mut statement = self
            .prepare(
                "SELECT id, ocr_run_id, polygon, text, confidence, created_at_ms \
                 FROM text_regions WHERE ocr_run_id = ?1 ORDER BY created_at_ms ASC, id ASC",
            )
            .map_err(|error| map_rusqlite_error("list_text_regions_for_ocr_run.prepare", error))?;
        let rows = statement
            .query_map([ocr_run_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<f32>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("list_text_regions_for_ocr_run.query", error))?;

        let mut regions = Vec::new();
        for row in rows {
            regions.push(text_region_from_row(row.map_err(|error| {
                map_rusqlite_error("list_text_regions_for_ocr_run.row", error)
            })?)?);
        }
        Ok(regions)
    }
}

fn require_detected_ocr_run(conn: &Connection, ocr_run_id: &OcrRunId) -> Result<(), A2dError> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM ocr_runs WHERE id = ?1",
            [ocr_run_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_rusqlite_error("insert_text_region.check_ocr_run_status", error))?;
    match status.as_deref() {
        Some("Detected") => Ok(()),
        Some("NoTextDetected" | "Unavailable") => Err(A2dError::new(
            ErrorCode::new("STORAGE_TEXT_REGION_OCR_RUN_NOT_DETECTED"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.storage.text_region_ocr_run_not_detected",
            "text regions can only be attached to detected OCR runs",
            false,
        )
        .with_detail("ocr_run_id", ocr_run_id.to_string())
        .with_detail("ocr_run_status", status.unwrap())),
        Some(other) => Err(A2dError::new(
            ErrorCode::new("STORAGE_TEXT_REGION_OCR_RUN_STATUS_CORRUPT"),
            ErrorCategory::Integrity,
            ErrorSeverity::Critical,
            "error.storage.text_region_ocr_run_status_corrupt",
            "text-region parent OCR run has an unknown status",
            false,
        )
        .with_detail("ocr_run_id", ocr_run_id.to_string())
        .with_detail("ocr_run_status", other)),
        None => Err(A2dError::new(
            ErrorCode::new("STORAGE_TEXT_REGION_OCR_RUN_MISSING"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.storage.text_region_ocr_run_missing",
            "text region requires an existing OCR run",
            false,
        )
        .with_detail("ocr_run_id", ocr_run_id.to_string())),
    }
}

fn require_region_capacity(conn: &Connection, ocr_run_id: &OcrRunId) -> Result<(), A2dError> {
    let region_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM text_regions WHERE ocr_run_id = ?1",
            [ocr_run_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|error| map_rusqlite_error("insert_text_region.count_for_ocr_run", error))?;
    if region_count >= MAX_OCR_REGIONS_PER_RUN {
        return Err(A2dError::new(
            ErrorCode::new("STORAGE_TEXT_REGION_RUN_LIMIT_EXCEEDED"),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.storage.text_region_run_limit_exceeded",
            "OCR run already contains the maximum supported number of text regions",
            false,
        )
        .with_detail("ocr_run_id", ocr_run_id.to_string())
        .with_detail("region_count", region_count.to_string())
        .with_detail("max_region_count", MAX_OCR_REGIONS_PER_RUN.to_string()));
    }
    Ok(())
}

fn text_region_from_row(
    row: (String, String, String, String, Option<f32>, i64),
) -> Result<TextRegion, A2dError> {
    let (id, ocr_run_id, polygon, text, confidence, created_at_ms) = row;
    TextRegion::from_stored(
        TextRegionId::parse(&id)?,
        OcrRunId::parse(&ocr_run_id)?,
        decode_json(&polygon, "text_regions.polygon")?,
        text,
        confidence,
        created_at_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_capacity_allows_exact_maximum_and_rejects_next_region() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE text_regions (ocr_run_id TEXT NOT NULL)",
            [],
        )
        .unwrap();
        let run_id = OcrRunId::generate();
        conn.execute(
            "WITH RECURSIVE n(value) AS (\
                 SELECT 1 UNION ALL SELECT value + 1 FROM n WHERE value < ?2\
             ) INSERT INTO text_regions (ocr_run_id) SELECT ?1 FROM n",
            params![run_id.to_string(), MAX_OCR_REGIONS_PER_RUN - 1],
        )
        .unwrap();

        require_region_capacity(&conn, &run_id).unwrap();
        conn.execute(
            "INSERT INTO text_regions (ocr_run_id) VALUES (?1)",
            [run_id.to_string()],
        )
        .unwrap();

        let error = require_region_capacity(&conn, &run_id).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "STORAGE_TEXT_REGION_RUN_LIMIT_EXCEEDED"
        );
        assert_eq!(
            error.details.get("region_count"),
            Some(&MAX_OCR_REGIONS_PER_RUN.to_string())
        );
        assert_eq!(
            error.details.get("max_region_count"),
            Some(&MAX_OCR_REGIONS_PER_RUN.to_string())
        );
    }
}
