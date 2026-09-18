use crate::A2dCore;
use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, OcrRunStatus, Provenance, ScanId,
    TextCorrectionId, TextRegionId, system_now_ms,
};
use a2d_storage::{
    OcrReadbackRepository, OcrRunRepository, OcrSearchDocumentKind, OcrSearchQuery,
    OcrSearchRepository, ScanRepository, TextCorrectionRecord, TextCorrectionRepository,
    TextRegionRepository,
};

const MAX_OCR_CORRECTION_TEXT_BYTES: usize = 2_000_000;
const MAX_OCR_CORRECTION_LIST_LIMIT: u32 = 1_000;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordOcrCorrectionRequest {
    pub scan_id: String,
    pub text_region_id: Option<String>,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub created_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedOcrCorrection {
    pub text_correction_id: String,
    pub scan_id: String,
    pub text_region_id: Option<String>,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListOcrCorrectionsForScanRequest {
    pub scan_id: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrTextCorrection {
    pub text_correction_id: String,
    pub scan_id: String,
    pub text_region_id: Option<String>,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListOcrCorrectionsForScanResult {
    pub scan_id: String,
    pub corrections: Vec<OcrTextCorrection>,
}

impl A2dCore {
    /// Searches persisted detected OCR text through the local SQLite FTS index.
    ///
    /// The index is derived from Rust-owned OCR rows. No-text, unavailable, and cancelled outcomes
    /// do not create search documents, so search cannot accidentally present provider failures as
    /// recognized empty text. OCR corrections are intentionally stored separately in M11-C1 and do
    /// not alter the original OCR search index; corrected-text search precedence remains explicitly
    /// documented as original OCR only for this milestone slice.
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

    /// Records a user correction for the latest detected OCR text on a scan or one text region.
    ///
    /// Corrections are append-only rows in `text_corrections`: the original `ocr_runs.full_text`
    /// and `text_regions.text` values are preserved as immutable OCR provenance. If `text_region_id`
    /// is provided, the region must belong to a detected OCR run for the same scan. If it is absent,
    /// the scan must already have a latest detected OCR run. The previous text is always derived
    /// from durable source OCR text so the correction record carries reviewable before/after
    /// evidence without trusting caller-supplied original text.
    pub fn record_ocr_correction(
        &self,
        request: RecordOcrCorrectionRequest,
    ) -> Result<RecordedOcrCorrection, A2dError> {
        validate_correction_text("corrected_text", &request.corrected_text)?;
        // `previous_text` remains on the request for FFI/API compatibility, but Rust derives the
        // stored previous text from the durable OCR source below. Android callers must not be able
        // to spoof original OCR provenance.
        if let Some(previous_text) = &request.previous_text {
            validate_optional_previous_text(previous_text)?;
        }
        let created_at_ms = request.created_at_ms.unwrap_or(system_now_ms()?);
        if created_at_ms < 0 {
            return Err(ocr_correction_error(
                "CORE_OCR_CORRECTION_TIMESTAMP_INVALID",
                "OCR correction timestamp must be non-negative milliseconds",
            ));
        }

        let scan_id = ScanId::parse(&request.scan_id)?;
        let text_region_id = request
            .text_region_id
            .as_deref()
            .map(TextRegionId::parse)
            .transpose()?;
        let storage = self.lock_storage()?;
        let scan = storage.get_scan(&scan_id)?.ok_or_else(|| {
            ocr_correction_error(
                "CORE_OCR_CORRECTION_SCAN_MISSING",
                "OCR correction requires an existing persisted scan",
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;

        let previous_text = match &text_region_id {
            Some(region_id) => {
                let region = storage.get_text_region(region_id)?.ok_or_else(|| {
                    ocr_correction_error(
                        "CORE_OCR_CORRECTION_TEXT_REGION_MISSING",
                        "OCR correction text_region_id must reference an existing text region",
                    )
                    .with_detail("text_region_id", region_id.to_string())
                })?;
                let run = storage.get_ocr_run(&region.ocr_run_id)?.ok_or_else(|| {
                    ocr_correction_error(
                        "CORE_OCR_CORRECTION_REGION_OCR_RUN_MISSING",
                        "OCR correction text region must reference an existing OCR run",
                    )
                    .with_detail("ocr_run_id", region.ocr_run_id.to_string())
                })?;
                if run.scan_id != scan_id {
                    return Err(ocr_correction_error(
                        "CORE_OCR_CORRECTION_TEXT_REGION_SCAN_MISMATCH",
                        "OCR correction text region belongs to a different scan",
                    )
                    .with_detail("requested_scan_id", scan_id.to_string())
                    .with_detail("region_scan_id", run.scan_id.to_string()));
                }
                if run.status != OcrRunStatus::Detected {
                    return Err(ocr_correction_error(
                        "CORE_OCR_CORRECTION_REGION_OCR_RUN_NOT_DETECTED",
                        "OCR correction text region must belong to a detected OCR run",
                    )
                    .with_detail("ocr_run_id", run.id().to_string()));
                }
                Some(region.text)
            }
            None => {
                let run = storage
                    .latest_ocr_run_for_scan(&scan_id)?
                    .ok_or_else(|| detected_run_missing_error(&scan_id))?;
                if run.status != OcrRunStatus::Detected {
                    return Err(detected_run_missing_error(&scan_id)
                        .with_detail("latest_ocr_run_id", run.id().to_string()));
                }
                Some(run.full_text)
            }
        };

        let provenance = Provenance {
            source_page_id: Some(scan.page_id),
            source_scan_id: Some(scan_id.clone()),
            producing_component: "a2d-core-ocr-correction".to_string(),
            component_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at_ms,
            warnings: Vec::new(),
            user_approved: Some(true),
        };
        let correction = TextCorrectionRecord {
            id: TextCorrectionId::try_generate()?,
            text_region_id,
            scan_id: scan_id.clone(),
            corrected_text: request.corrected_text,
            previous_text,
            provenance,
        };
        storage.insert_text_correction(&correction)?;

        Ok(recorded_correction(correction))
    }

    /// Lists recorded user corrections for a scan without mutating OCR output.
    pub fn list_ocr_corrections_for_scan(
        &self,
        request: ListOcrCorrectionsForScanRequest,
    ) -> Result<ListOcrCorrectionsForScanResult, A2dError> {
        if request.limit == 0 || request.limit > MAX_OCR_CORRECTION_LIST_LIMIT {
            return Err(ocr_correction_error(
                "CORE_OCR_CORRECTION_LIST_LIMIT_INVALID",
                "OCR correction list limit must be between 1 and the configured maximum",
            )
            .with_detail("limit", request.limit.to_string())
            .with_detail("max_limit", MAX_OCR_CORRECTION_LIST_LIMIT.to_string()));
        }
        let scan_id = ScanId::parse(&request.scan_id)?;
        let storage = self.lock_storage()?;
        storage.get_scan(&scan_id)?.ok_or_else(|| {
            ocr_correction_error(
                "CORE_OCR_CORRECTION_SCAN_MISSING",
                "OCR correction listing requires an existing persisted scan",
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;
        let corrections = storage
            .list_text_corrections_for_scan(&scan_id, request.limit as usize)?
            .into_iter()
            .map(loaded_correction)
            .collect();
        Ok(ListOcrCorrectionsForScanResult {
            scan_id: scan_id.to_string(),
            corrections,
        })
    }
}

fn recorded_correction(correction: TextCorrectionRecord) -> RecordedOcrCorrection {
    RecordedOcrCorrection {
        text_correction_id: correction.id.to_string(),
        scan_id: correction.scan_id.to_string(),
        text_region_id: correction.text_region_id.map(|id| id.to_string()),
        corrected_text: correction.corrected_text,
        previous_text: correction.previous_text,
        created_at_ms: correction.provenance.created_at_ms,
    }
}

fn loaded_correction(correction: TextCorrectionRecord) -> OcrTextCorrection {
    OcrTextCorrection {
        text_correction_id: correction.id.to_string(),
        scan_id: correction.scan_id.to_string(),
        text_region_id: correction.text_region_id.map(|id| id.to_string()),
        corrected_text: correction.corrected_text,
        previous_text: correction.previous_text,
        created_at_ms: correction.provenance.created_at_ms,
    }
}

fn detected_run_missing_error(scan_id: &ScanId) -> A2dError {
    ocr_correction_error(
        "CORE_OCR_CORRECTION_DETECTED_RUN_MISSING",
        "OCR correction requires an existing detected OCR run for the scan",
    )
    .with_detail("scan_id", scan_id.to_string())
}

fn validate_correction_text(field: &'static str, text: &str) -> Result<(), A2dError> {
    if text.trim().is_empty() {
        return Err(ocr_correction_error(
            "CORE_OCR_CORRECTION_TEXT_EMPTY",
            "OCR correction text must not be empty",
        )
        .with_detail("field", field));
    }
    if text.len() > MAX_OCR_CORRECTION_TEXT_BYTES {
        return Err(ocr_correction_error(
            "CORE_OCR_CORRECTION_TEXT_EXCEEDS_LIMIT",
            "OCR correction text exceeds the configured limit",
        )
        .with_detail("field", field)
        .with_detail("text_bytes", text.len().to_string())
        .with_detail("max_text_bytes", MAX_OCR_CORRECTION_TEXT_BYTES.to_string()));
    }
    Ok(())
}

fn validate_optional_previous_text(text: &str) -> Result<(), A2dError> {
    if text.len() > MAX_OCR_CORRECTION_TEXT_BYTES {
        return Err(ocr_correction_error(
            "CORE_OCR_CORRECTION_PREVIOUS_TEXT_EXCEEDS_LIMIT",
            "OCR correction previous text exceeds the configured limit",
        )
        .with_detail("text_bytes", text.len().to_string())
        .with_detail("max_text_bytes", MAX_OCR_CORRECTION_TEXT_BYTES.to_string()));
    }
    Ok(())
}

fn ocr_correction_error(code: &'static str, developer_message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.correction",
        developer_message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;
    use a2d_domain::{
        Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRun, OcrRunId,
        Page, PageId, PageKind, PageState, QualityStatus, Scan, SmartPageId,
    };
    use a2d_storage::{AssetRepository, PageRepository, ScanRepository};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct ScanFixture {
        scan_id: ScanId,
        page_id: PageId,
        original_asset_id: AssetId,
    }

    fn open_test_core() -> (Arc<A2dCore>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "a2d-core-ocr-correction-test-{}",
            PageId::generate()
        ));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, dir)
    }

    fn asset(id: AssetId) -> Asset {
        Asset::new(
            id.clone(),
            AssetKind::Original,
            format!("assets/originals/{id}.png"),
            "image/png".to_string(),
            1_024,
            "test-sha256".to_string(),
            100,
            true,
            EncryptionState::Plaintext,
        )
    }

    fn scan_fingerprint() -> String {
        format!(
            "scan-content-v1;corrected-sha256={};perceptual=mean-grid-16x24-v1:{}",
            "a".repeat(64),
            "b".repeat(16 * 24 * 2)
        )
    }

    fn provenance(page_id: &PageId, scan_id: &ScanId) -> Provenance {
        Provenance {
            source_page_id: Some(page_id.clone()),
            source_scan_id: Some(scan_id.clone()),
            producing_component: "test-ocr-provider".to_string(),
            component_version: "1.0.0".to_string(),
            created_at_ms: 200,
            warnings: Vec::new(),
            user_approved: None,
        }
    }

    fn insert_scan_fixture(core: &A2dCore) -> ScanFixture {
        let page_id = PageId::generate();
        let original_asset_id = AssetId::generate();
        let scan_id = ScanId::generate();
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("PAGE").unwrap(),
            Some("OCR correction core page".to_string()),
            PageState::Scanned,
            100,
        );
        let scan = Scan::new(
            scan_id.clone(),
            page_id.clone(),
            None,
            CaptureSource::Camera,
            125,
            original_asset_id.clone(),
            None,
            None,
            None,
            "test-pipeline".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            true,
            None,
            scan_fingerprint(),
        );
        let storage = core.lock_storage().unwrap();
        storage.insert_page(&page).unwrap();
        storage
            .insert_asset(&asset(original_asset_id.clone()))
            .unwrap();
        storage.insert_scan(&scan).unwrap();
        ScanFixture {
            scan_id,
            page_id,
            original_asset_id,
        }
    }

    fn insert_detected_ocr_run(core: &A2dCore, fixture: &ScanFixture, full_text: &str) -> OcrRunId {
        let run = OcrRun::detected(
            OcrRunId::generate(),
            fixture.scan_id.clone(),
            Some(fixture.original_asset_id.clone()),
            "mlkit".to_string(),
            "2026.09".to_string(),
            Some("latin-v1".to_string()),
            full_text.to_string(),
            Some(250),
            Vec::new(),
            provenance(&fixture.page_id, &fixture.scan_id),
        )
        .unwrap();
        let run_id = run.id().clone();
        let storage = core.lock_storage().unwrap();
        storage.insert_ocr_run(&run).unwrap();
        run_id
    }

    #[test]
    fn record_ocr_correction_persists_scan_level_correction_without_mutating_ocr() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_detected_ocr_run(&core, &fixture, "helo notebook");

        let recorded = core
            .record_ocr_correction(RecordOcrCorrectionRequest {
                scan_id: fixture.scan_id.to_string(),
                text_region_id: None,
                corrected_text: "hello notebook".to_string(),
                previous_text: None,
                created_at_ms: Some(300),
            })
            .unwrap();

        assert_eq!(recorded.scan_id, fixture.scan_id.to_string());
        assert_eq!(recorded.text_region_id, None);
        assert_eq!(recorded.corrected_text, "hello notebook");
        assert_eq!(recorded.previous_text.as_deref(), Some("helo notebook"));
        assert_eq!(recorded.created_at_ms, 300);

        let storage = core.lock_storage().unwrap();
        let original = storage.get_ocr_run(&run_id).unwrap().unwrap();
        assert_eq!(original.full_text, "helo notebook");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_correction_ignores_spoofed_scan_previous_text() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        insert_detected_ocr_run(&core, &fixture, "helo notebook");

        let recorded = core
            .record_ocr_correction(RecordOcrCorrectionRequest {
                scan_id: fixture.scan_id.to_string(),
                text_region_id: None,
                corrected_text: "hello notebook".to_string(),
                previous_text: Some("caller supplied spoofed previous text".to_string()),
                created_at_ms: Some(300),
            })
            .unwrap();

        assert_eq!(recorded.corrected_text, "hello notebook");
        assert_eq!(recorded.previous_text.as_deref(), Some("helo notebook"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_ocr_corrections_returns_persisted_review_history() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        insert_detected_ocr_run(&core, &fixture, "helo notebook");
        core.record_ocr_correction(RecordOcrCorrectionRequest {
            scan_id: fixture.scan_id.to_string(),
            text_region_id: None,
            corrected_text: "hello notebook".to_string(),
            previous_text: None,
            created_at_ms: Some(300),
        })
        .unwrap();

        let corrections = core
            .list_ocr_corrections_for_scan(ListOcrCorrectionsForScanRequest {
                scan_id: fixture.scan_id.to_string(),
                limit: 10,
            })
            .unwrap();

        assert_eq!(corrections.scan_id, fixture.scan_id.to_string());
        assert_eq!(corrections.corrections.len(), 1);
        assert_eq!(corrections.corrections[0].corrected_text, "hello notebook");
        assert_eq!(
            corrections.corrections[0].previous_text.as_deref(),
            Some("helo notebook")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_correction_requires_detected_ocr_text() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);

        let err = core
            .record_ocr_correction(RecordOcrCorrectionRequest {
                scan_id: fixture.scan_id.to_string(),
                text_region_id: None,
                corrected_text: "hello notebook".to_string(),
                previous_text: None,
                created_at_ms: Some(300),
            })
            .unwrap_err();

        assert_eq!(
            err.code.to_string(),
            "CORE_OCR_CORRECTION_DETECTED_RUN_MISSING"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_correction_rejects_empty_correction_text_before_storage() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        insert_detected_ocr_run(&core, &fixture, "helo notebook");

        let err = core
            .record_ocr_correction(RecordOcrCorrectionRequest {
                scan_id: fixture.scan_id.to_string(),
                text_region_id: None,
                corrected_text: "   ".to_string(),
                previous_text: None,
                created_at_ms: Some(300),
            })
            .unwrap_err();

        assert_eq!(err.code.to_string(), "CORE_OCR_CORRECTION_TEXT_EMPTY");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_ocr_corrections_bounds_result_limit() {
        let (core, dir) = open_test_core();
        let err = core
            .list_ocr_corrections_for_scan(ListOcrCorrectionsForScanRequest {
                scan_id: ScanId::generate().to_string(),
                limit: MAX_OCR_CORRECTION_LIST_LIMIT + 1,
            })
            .unwrap_err();

        assert_eq!(
            err.code.to_string(),
            "CORE_OCR_CORRECTION_LIST_LIMIT_INVALID"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
