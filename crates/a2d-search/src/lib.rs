//! Full-text search over library content backed by SQLite FTS.
//!
//! This crate owns the small, platform-neutral search request/result contract. Storage owns the
//! SQLite FTS implementation, and core/FFI project these types into app-facing APIs.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

pub const MAX_OCR_SEARCH_QUERY_BYTES: usize = 256;
pub const MAX_OCR_SEARCH_RESULTS_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrSearchDocumentKind {
    FullText,
    TextRegion,
}

impl OcrSearchDocumentKind {
    pub fn as_storage_str(self) -> &'static str {
        match self {
            Self::FullText => "FullText",
            Self::TextRegion => "TextRegion",
        }
    }

    pub fn from_storage_str(raw: &str) -> Result<Self, A2dError> {
        match raw {
            "FullText" => Ok(Self::FullText),
            "TextRegion" => Ok(Self::TextRegion),
            other => Err(A2dError::new(
                ErrorCode::new("SEARCH_DOCUMENT_KIND_CORRUPT"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.search.document_kind_corrupt",
                "search index row has an unknown OCR document kind",
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
            return Err(search_error(
                "SEARCH_QUERY_EMPTY",
                "OCR search query must not be empty",
            ));
        }
        if text.len() > MAX_OCR_SEARCH_QUERY_BYTES {
            return Err(search_error(
                "SEARCH_QUERY_EXCEEDS_LIMIT",
                "OCR search query exceeds the configured limit",
            )
            .with_detail("query_bytes", text.len().to_string())
            .with_detail("max_query_bytes", MAX_OCR_SEARCH_QUERY_BYTES.to_string()));
        }
        if limit == 0 || limit > MAX_OCR_SEARCH_RESULTS_LIMIT {
            return Err(search_error(
                "SEARCH_LIMIT_INVALID",
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

    pub fn fts5_match_expression(&self) -> String {
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

fn quote_fts5_token(token: &str) -> String {
    format!("\"{}\"", token.replace('"', "\"\""))
}

fn search_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.search.query_invalid",
        message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_trims_and_quotes_fts_terms() {
        let query = OcrSearchQuery::new("  hello notebook  ", 10).unwrap();

        assert_eq!(query.text(), "hello notebook");
        assert_eq!(query.limit(), 10);
        assert_eq!(query.fts5_match_expression(), "\"hello\" \"notebook\"");
    }

    #[test]
    fn search_query_rejects_empty_and_invalid_limits() {
        let empty = OcrSearchQuery::new("   ", 10).unwrap_err();
        assert_eq!(empty.code.to_string(), "SEARCH_QUERY_EMPTY");

        let zero_limit = OcrSearchQuery::new("hello", 0).unwrap_err();
        assert_eq!(zero_limit.code.to_string(), "SEARCH_LIMIT_INVALID");

        let too_many = OcrSearchQuery::new("hello", MAX_OCR_SEARCH_RESULTS_LIMIT + 1).unwrap_err();
        assert_eq!(too_many.code.to_string(), "SEARCH_LIMIT_INVALID");
    }
}
