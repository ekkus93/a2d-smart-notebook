use crate::A2dCore;
use a2d_domain::A2dError;
use a2d_storage::{OcrSearchDocumentKind, OcrSearchQuery, OcrSearchRepository};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CoreOcrSearchDocumentKind {
    FullText,
    TextRegion,
}

impl From<OcrSearchDocumentKind> for CoreOcrSearchDocumentKind {
    fn from(value: OcrSearchDocumentKind) -> Self {
        match value {
            OcrSearchDocumentKind::FullText => Self::FullText,
            OcrSearchDocumentKind::TextRegion => Self::TextRegion,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchOcrTextRequest {
    pub query: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrTextSearchHit {
    pub page_id: String,
    pub scan_id: String,
    pub ocr_run_id: String,
    pub text_region_id: Option<String>,
    pub document_kind: CoreOcrSearchDocumentKind,
    pub snippet: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchOcrTextResults {
    pub query: String,
    pub hits: Vec<OcrTextSearchHit>,
}

impl A2dCore {
    /// Searches persisted detected OCR text through the local SQLite FTS index.
    ///
    /// The index is derived from Rust-owned OCR rows. No-text, unavailable, and cancelled outcomes
    /// do not create search documents, so search cannot accidentally present provider failures as
    /// recognized empty text.
    pub fn search_ocr_text(
        &self,
        request: SearchOcrTextRequest,
    ) -> Result<SearchOcrTextResults, A2dError> {
        let query = OcrSearchQuery::new(request.query, request.limit as usize)?;
        let storage = self.lock_storage()?;
        let hits = storage
            .search_ocr_text(&query)?
            .into_iter()
            .map(|hit| OcrTextSearchHit {
                page_id: hit.page_id,
                scan_id: hit.scan_id,
                ocr_run_id: hit.ocr_run_id,
                text_region_id: hit.text_region_id,
                document_kind: hit.document_kind.into(),
                snippet: hit.snippet,
            })
            .collect();
        Ok(SearchOcrTextResults {
            query: query.text().to_string(),
            hits,
        })
    }
}
