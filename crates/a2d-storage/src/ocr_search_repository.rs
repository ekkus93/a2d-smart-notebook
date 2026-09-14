//! Search repository for persisted OCR text.
//!
//! The SQLite FTS table is maintained by migration-owned triggers. This repository only validates
//! bounded query input and maps FTS rows back to typed search hits; callers never issue SQL.
//! This module also owns the first typed repository for OCR text corrections because corrections
//! share the OCR-text persistence boundary and must stay separate from the original OCR rows.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, Provenance, ScanId,
    TextCorrectionId, TextRegionId,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::json_columns::{decode_json, encode_json};
use crate::{Storage, map_rusqlite_error};

pub const MAX_OCR_SEARCH_QUERY_BYTES: usize = 256;
pub const MAX_OCR_SEARCH_RESULTS_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrSearchDocumentKind {
    FullText,
    TextRegion,
}

impl OcrSearchDocumentKind {
    fn from_storage_str(raw: &str) -> Result<Self, A2dError> {
        match raw {
            "FullText" => Ok(Self::FullText),
            "TextRegion" => Ok(Self::TextRegion),
            other => Err(A2dError::new(
                ErrorCode::new("STORAGE_OCR_SEARCH_DOCUMENT_KIND_CORRUPT"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.ocr_search_document_kind_corrupt",
                "OCR search index row has an unknown document kind",
                false,
            )
            .with_detail("document_kind", other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrSearchQuery {
    text: String,
    limit: usize,
}

impl OcrSearchQuery {
    pub fn new(text: impl Into<String>, limit: usize) -> Result<Self, A2dError> {
        let text = text.into();
        let text = text.trim();
        if text.is_empty() {
            return Err(search_query_error(
                "STORAGE_OCR_SEARCH_QUERY_EMPTY",
                "OCR search query must not be empty",
            ));
        }
        if text.len() > MAX_OCR_SEARCH_QUERY_BYTES {
            return Err(search_query_error(
                "STORAGE_OCR_SEARCH_QUERY_EXCEEDS_LIMIT",
                "OCR search query exceeds the configured limit",
            )
            .with_detail("query_bytes", text.len().to_string())
            .with_detail("max_query_bytes", MAX_OCR_SEARCH_QUERY_BYTES.to_string()));
        }
        if limit == 0 || limit > MAX_OCR_SEARCH_RESULTS_LIMIT {
            return Err(search_query_error(
                "STORAGE_OCR_SEARCH_LIMIT_INVALID",
                "OCR search limit must be between 1 and the configured maximum",
            )
            .with_detail("limit", limit.to_string())
            .with_detail("max_limit", MAX_OCR_SEARCH_RESULTS_LIMIT.to_string()));
        }
        Ok(Self {
            text: text.to_string(),
            limit,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    fn fts5_match_expression(&self) -> String {
        self.text
            .split_whitespace()
            .map(quote_fts5_token)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrSearchResult {
    pub page_id: String,
    pub scan_id: String,
    pub ocr_run_id: String,
    pub text_region_id: Option<String>,
    pub document_kind: OcrSearchDocumentKind,
    pub snippet: String,
}

pub trait OcrSearchRepository {
    fn search_ocr_text(&self, query: &OcrSearchQuery) -> Result<Vec<OcrSearchResult>, A2dError>;
}

impl OcrSearchRepository for Storage {
    fn search_ocr_text(&self, query: &OcrSearchQuery) -> Result<Vec<OcrSearchResult>, A2dError> {
        OcrSearchRepository::search_ocr_text(&self.conn, query)
    }
}

impl OcrSearchRepository for Connection {
    fn search_ocr_text(&self, query: &OcrSearchQuery) -> Result<Vec<OcrSearchResult>, A2dError> {
        let match_expression = query.fts5_match_expression();
        let mut statement = self
            .prepare(
                "SELECT page_id, scan_id, ocr_run_id, text_region_id, document_kind, \
                 snippet(ocr_search_index, 5, '[', ']', '...', 16) \
                 FROM ocr_search_index \
                 WHERE ocr_search_index MATCH ?1 \
                 ORDER BY rank \
                 LIMIT ?2",
            )
            .map_err(|error| map_rusqlite_error("search_ocr_text.prepare", error))?;
        let rows = statement
            .query_map(params![match_expression, query.limit() as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|error| map_rusqlite_error("search_ocr_text.query", error))?;

        let mut results = Vec::new();
        for row in rows {
            let (page_id, scan_id, ocr_run_id, text_region_id, document_kind, snippet) =
                row.map_err(|error| map_rusqlite_error("search_ocr_text.row", error))?;
            results.push(OcrSearchResult {
                page_id,
                scan_id,
                ocr_run_id,
                text_region_id,
                document_kind: OcrSearchDocumentKind::from_storage_str(&document_kind)?,
                snippet,
            });
        }
        Ok(results)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextCorrectionRecord {
    pub id: TextCorrectionId,
    pub text_region_id: Option<TextRegionId>,
    pub scan_id: ScanId,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub provenance: Provenance,
}

impl TextCorrectionRecord {
    pub fn id(&self) -> &TextCorrectionId {
        &self.id
    }
}

pub trait TextCorrectionRepository {
    fn insert_text_correction(&self, correction: &TextCorrectionRecord) -> Result<(), A2dError>;
    fn get_text_correction(
        &self,
        id: &TextCorrectionId,
    ) -> Result<Option<TextCorrectionRecord>, A2dError>;
    fn list_text_corrections_for_scan(
        &self,
        scan_id: &ScanId,
        limit: usize,
    ) -> Result<Vec<TextCorrectionRecord>, A2dError>;
}

impl TextCorrectionRepository for Storage {
    fn insert_text_correction(&self, correction: &TextCorrectionRecord) -> Result<(), A2dError> {
        TextCorrectionRepository::insert_text_correction(&self.conn, correction)
    }

    fn get_text_correction(
        &self,
        id: &TextCorrectionId,
    ) -> Result<Option<TextCorrectionRecord>, A2dError> {
        TextCorrectionRepository::get_text_correction(&self.conn, id)
    }

    fn list_text_corrections_for_scan(
        &self,
        scan_id: &ScanId,
        limit: usize,
    ) -> Result<Vec<TextCorrectionRecord>, A2dError> {
        TextCorrectionRepository::list_text_corrections_for_scan(&self.conn, scan_id, limit)
    }
}

impl TextCorrectionRepository for Connection {
    fn insert_text_correction(&self, correction: &TextCorrectionRecord) -> Result<(), A2dError> {
        let provenance_warnings = encode_json(
            &correction.provenance.warnings,
            "text_corrections.provenance_warnings",
        )?;
        self.execute(
            "INSERT INTO text_corrections (id, text_region_id, scan_id, corrected_text, \
             previous_text, provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                correction.id.to_string(),
                correction.text_region_id.as_ref().map(ToString::to_string),
                correction.scan_id.to_string(),
                correction.corrected_text,
                correction.previous_text,
                correction
                    .provenance
                    .source_page_id
                    .as_ref()
                    .map(ToString::to_string),
                correction
                    .provenance
                    .source_scan_id
                    .as_ref()
                    .map(ToString::to_string),
                correction.provenance.producing_component,
                correction.provenance.component_version,
                correction.provenance.created_at_ms,
                provenance_warnings,
                correction.provenance.user_approved,
            ],
        )
        .map_err(|error| map_rusqlite_error("insert_text_correction", error))?;
        Ok(())
    }

    fn get_text_correction(
        &self,
        id: &TextCorrectionId,
    ) -> Result<Option<TextCorrectionRecord>, A2dError> {
        self.query_row(
            "SELECT id, text_region_id, scan_id, corrected_text, previous_text, \
             provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved \
             FROM text_corrections WHERE id = ?1",
            [id.to_string()],
            text_correction_row,
        )
        .optional()
        .map_err(|error| map_rusqlite_error("get_text_correction", error))?
        .map(text_correction_from_row)
        .transpose()
    }

    fn list_text_corrections_for_scan(
        &self,
        scan_id: &ScanId,
        limit: usize,
    ) -> Result<Vec<TextCorrectionRecord>, A2dError> {
        let mut statement = self
            .prepare(
                "SELECT id, text_region_id, scan_id, corrected_text, previous_text, \
                 provenance_source_page_id, provenance_source_scan_id, \
                 provenance_producing_component, provenance_component_version, \
                 provenance_created_at_ms, provenance_warnings, provenance_user_approved \
                 FROM text_corrections WHERE scan_id = ?1 \
                 ORDER BY provenance_created_at_ms ASC, id ASC LIMIT ?2",
            )
            .map_err(|error| map_rusqlite_error("list_text_corrections_for_scan.prepare", error))?;
        let rows = statement
            .query_map(
                params![scan_id.to_string(), limit as i64],
                text_correction_row,
            )
            .map_err(|error| map_rusqlite_error("list_text_corrections_for_scan.query", error))?;

        let mut corrections = Vec::new();
        for row in rows {
            corrections.push(text_correction_from_row(row.map_err(|error| {
                map_rusqlite_error("list_text_corrections_for_scan.row", error)
            })?)?);
        }
        Ok(corrections)
    }
}

type TextCorrectionRow = (
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    i64,
    String,
    Option<bool>,
);

fn text_correction_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TextCorrectionRow> {
    Ok((
        row.get::<_, String>(0)?,
        row.get::<_, Option<String>>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, Option<String>>(4)?,
        row.get::<_, Option<String>>(5)?,
        row.get::<_, Option<String>>(6)?,
        row.get::<_, String>(7)?,
        row.get::<_, String>(8)?,
        row.get::<_, i64>(9)?,
        row.get::<_, String>(10)?,
        row.get::<_, Option<bool>>(11)?,
    ))
}

fn text_correction_from_row(row: TextCorrectionRow) -> Result<TextCorrectionRecord, A2dError> {
    let (
        id,
        text_region_id,
        scan_id,
        corrected_text,
        previous_text,
        source_page_id,
        source_scan_id,
        producing_component,
        component_version,
        created_at_ms,
        provenance_warnings,
        user_approved,
    ) = row;
    Ok(TextCorrectionRecord {
        id: TextCorrectionId::parse(&id)?,
        text_region_id: text_region_id
            .map(|value| TextRegionId::parse(&value))
            .transpose()?,
        scan_id: ScanId::parse(&scan_id)?,
        corrected_text,
        previous_text,
        provenance: Provenance {
            source_page_id: source_page_id
                .map(|value| PageId::parse(&value))
                .transpose()?,
            source_scan_id: source_scan_id
                .map(|value| ScanId::parse(&value))
                .transpose()?,
            producing_component,
            component_version,
            created_at_ms,
            warnings: decode_json(&provenance_warnings, "text_corrections.provenance_warnings")?,
            user_approved,
        },
    })
}

fn quote_fts5_token(token: &str) -> String {
    format!("\"{}\"", token.replace('"', "\"\""))
}

fn search_query_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.ocr_search_query_invalid",
        message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_trims_terms_and_rejects_invalid_limits() {
        let query = OcrSearchQuery::new("  hello notebook  ", 10).unwrap();
        assert_eq!(query.text(), "hello notebook");
        assert_eq!(query.fts5_match_expression(), "\"hello\" \"notebook\"");

        assert_eq!(
            OcrSearchQuery::new("", 10).unwrap_err().code.to_string(),
            "STORAGE_OCR_SEARCH_QUERY_EMPTY"
        );
        assert_eq!(
            OcrSearchQuery::new("hello", 0)
                .unwrap_err()
                .code
                .to_string(),
            "STORAGE_OCR_SEARCH_LIMIT_INVALID"
        );
        assert_eq!(
            OcrSearchQuery::new("hello", MAX_OCR_SEARCH_RESULTS_LIMIT + 1)
                .unwrap_err()
                .code
                .to_string(),
            "STORAGE_OCR_SEARCH_LIMIT_INVALID"
        );
    }
}
