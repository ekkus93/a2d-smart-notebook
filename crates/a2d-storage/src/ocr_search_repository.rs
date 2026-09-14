//! Search repository for persisted OCR text.
//!
//! The SQLite FTS table is maintained by migration-owned triggers. This repository only validates
//! bounded query input and maps FTS rows back to typed search hits; callers never issue SQL.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use rusqlite::{Connection, params};

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
