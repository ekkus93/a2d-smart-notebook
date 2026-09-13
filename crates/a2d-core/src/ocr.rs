use crate::A2dCore;
use a2d_domain::{
    A2dError, Asset, AssetId, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, OcrRun, OcrRunId,
    OcrRunStatus, OcrUnavailableReason, Provenance, Scan, ScanId, system_now_ms,
};
use a2d_storage::{AssetRepository, OcrRunRepository, ScanRepository};

const MAX_OCR_IMAGE_DIMENSION_PX: u32 = 12_000;
const MAX_OCR_IMAGE_PIXELS: u64 = 80_000_000;
const MAX_OCR_LABEL_BYTES: usize = 120;
const MAX_OCR_WARNING_COUNT: usize = 64;
const MAX_OCR_WARNING_TEXT_BYTES: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CoreOcrInputKind {
    Original,
    Corrected,
    OcrOptimized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareOcrInputRequest {
    pub scan_id: String,
    pub input_kind: CoreOcrInputKind,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedOcrInput {
    pub scan_id: String,
    pub input_asset_id: String,
    pub input_kind: CoreOcrInputKind,
    pub media_type: String,
    pub relative_path: String,
    pub byte_length: u64,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordOcrRunRequest {
    pub scan_id: String,
    pub input_asset_id: String,
    pub provider: String,
    pub provider_version: String,
    pub model_name: Option<String>,
    pub status: OcrRunStatus,
    pub full_text: String,
    pub unavailable_reason: Option<OcrUnavailableReason>,
    pub unavailable_message: Option<String>,
    pub completed_at_ms: Option<i64>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedOcrRun {
    pub ocr_run_id: String,
    pub scan_id: String,
    pub input_asset_id: String,
    pub status: OcrRunStatus,
}

impl A2dCore {
    /// Resolves the exact immutable scan asset that a platform OCR provider may read.
    ///
    /// This is the storage/core bridge for the Milestone 11 OCR contract: Android may choose an
    /// OCR provider, but Rust chooses the scan-owned asset identity, verifies it exists, verifies
    /// it is immutable, and rejects missing corrected/OCR-optimized assets explicitly instead of
    /// silently falling back to the original image.
    pub fn prepare_ocr_input(
        &self,
        request: PrepareOcrInputRequest,
    ) -> Result<PreparedOcrInput, A2dError> {
        validate_ocr_dimensions(request.width_px, request.height_px)?;
        let scan_id = ScanId::parse(&request.scan_id)?;
        let storage = self.lock_storage()?;
        let scan = storage.get_scan(&scan_id)?.ok_or_else(|| {
            ocr_core_error(
                "CORE_OCR_SCAN_MISSING",
                "OCR input preparation requires an existing persisted scan",
                false,
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;
        let input_asset_id = select_ocr_asset_id(&scan, request.input_kind)?;
        let asset = storage.get_asset(&input_asset_id)?.ok_or_else(|| {
            ocr_core_error(
                "CORE_OCR_INPUT_ASSET_MISSING_ROW",
                "OCR input preparation requires the selected asset row to exist",
                false,
            )
            .with_detail("scan_id", scan_id.to_string())
            .with_detail("input_kind", input_kind_label(request.input_kind))
            .with_detail("input_asset_id", input_asset_id.to_string())
        })?;
        validate_input_asset(&asset, request.input_kind)?;

        Ok(PreparedOcrInput {
            scan_id: scan_id.to_string(),
            input_asset_id: input_asset_id.to_string(),
            input_kind: request.input_kind,
            media_type: asset.media_type,
            relative_path: asset.relative_path,
            byte_length: asset.byte_length,
            width_px: request.width_px,
            height_px: request.height_px,
        })
    }

    /// Persists one terminal OCR outcome after Rust revalidates the scan-owned input asset.
    ///
    /// Platform code may run OCR, but it does not get to manufacture SQL-shaped OCR rows. This
    /// method verifies that the scan exists, the input asset belongs to that scan, the asset row is
    /// immutable and kind-correct, and the terminal outcome is explicit (`Detected`,
    /// `NoTextDetected`, or `Unavailable`) before storage sees it.
    pub fn record_ocr_run(&self, request: RecordOcrRunRequest) -> Result<RecordedOcrRun, A2dError> {
        validate_record_ocr_request(&request)?;
        let scan_id = ScanId::parse(&request.scan_id)?;
        let input_asset_id = AssetId::parse(&request.input_asset_id)?;
        let completed_at_ms = request.completed_at_ms;
        if let Some(completed_at_ms) = completed_at_ms {
            validate_non_negative_timestamp(completed_at_ms, "completed_at_ms")?;
        }

        let storage = self.lock_storage()?;
        let scan = storage.get_scan(&scan_id)?.ok_or_else(|| {
            ocr_record_error(
                "CORE_OCR_RECORD_SCAN_MISSING",
                "OCR result recording requires an existing persisted scan",
                false,
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;
        let expected_asset_kind = expected_scan_asset_kind(&scan, &input_asset_id)?;
        let asset = storage.get_asset(&input_asset_id)?.ok_or_else(|| {
            ocr_record_error(
                "CORE_OCR_RECORD_INPUT_ASSET_MISSING_ROW",
                "OCR result recording requires the selected input asset row to exist",
                false,
            )
            .with_detail("scan_id", scan_id.to_string())
            .with_detail("input_asset_id", input_asset_id.to_string())
        })?;
        validate_record_input_asset(&asset, expected_asset_kind)?;

        let run_id = OcrRunId::try_generate()?;
        let provenance_created_at_ms = completed_at_ms.unwrap_or(system_now_ms()?);
        let provenance = Provenance {
            source_page_id: Some(scan.page_id),
            source_scan_id: Some(scan_id.clone()),
            producing_component: request.provider.clone(),
            component_version: request.provider_version.clone(),
            created_at_ms: provenance_created_at_ms,
            warnings: request.warnings.clone(),
            user_approved: None,
        };
        let status = request.status;
        let run = OcrRun::from_stored(
            run_id.clone(),
            scan_id.clone(),
            Some(input_asset_id.clone()),
            request.provider,
            request.provider_version,
            request.model_name,
            request.status,
            request.full_text,
            request.unavailable_reason,
            request.unavailable_message,
            completed_at_ms,
            request.warnings,
            provenance,
        )?;
        storage.insert_ocr_run(&run)?;

        Ok(RecordedOcrRun {
            ocr_run_id: run_id.to_string(),
            scan_id: scan_id.to_string(),
            input_asset_id: input_asset_id.to_string(),
            status,
        })
    }
}

fn validate_ocr_dimensions(width_px: u32, height_px: u32) -> Result<(), A2dError> {
    if width_px == 0 || height_px == 0 {
        return Err(ocr_core_error(
            "CORE_OCR_IMAGE_DIMENSIONS_INVALID",
            "OCR image dimensions must be non-zero",
            false,
        )
        .with_detail("width_px", width_px.to_string())
        .with_detail("height_px", height_px.to_string()));
    }
    if width_px > MAX_OCR_IMAGE_DIMENSION_PX || height_px > MAX_OCR_IMAGE_DIMENSION_PX {
        return Err(ocr_core_error(
            "CORE_OCR_IMAGE_DIMENSIONS_EXCEED_LIMIT",
            "OCR image dimensions exceed the configured OCR limit",
            false,
        )
        .with_detail("width_px", width_px.to_string())
        .with_detail("height_px", height_px.to_string())
        .with_detail(
            "max_image_dimension_px",
            MAX_OCR_IMAGE_DIMENSION_PX.to_string(),
        ));
    }
    let pixels = u64::from(width_px) * u64::from(height_px);
    if pixels > MAX_OCR_IMAGE_PIXELS {
        return Err(ocr_core_error(
            "CORE_OCR_IMAGE_PIXELS_EXCEED_LIMIT",
            "OCR image pixel count exceeds the configured OCR limit",
            false,
        )
        .with_detail("pixels", pixels.to_string())
        .with_detail("max_image_pixels", MAX_OCR_IMAGE_PIXELS.to_string()));
    }
    Ok(())
}

fn validate_record_ocr_request(request: &RecordOcrRunRequest) -> Result<(), A2dError> {
    validate_ocr_label(&request.provider, "provider")?;
    validate_ocr_label(&request.provider_version, "provider_version")?;
    if let Some(model_name) = &request.model_name {
        validate_ocr_label(model_name, "model_name")?;
    }
    if let Some(message) = &request.unavailable_message
        && (message.is_empty() || message.len() > MAX_OCR_WARNING_TEXT_BYTES)
    {
        return Err(ocr_record_error(
            "CORE_OCR_RECORD_UNAVAILABLE_MESSAGE_INVALID",
            "OCR unavailable message must be non-empty and bounded when present",
            false,
        )
        .with_detail("max_bytes", MAX_OCR_WARNING_TEXT_BYTES.to_string()));
    }
    if request.warnings.len() > MAX_OCR_WARNING_COUNT {
        return Err(ocr_record_error(
            "CORE_OCR_RECORD_WARNING_COUNT_EXCEEDS_LIMIT",
            "OCR warning count exceeds the configured OCR limit",
            false,
        )
        .with_detail("warning_count", request.warnings.len().to_string())
        .with_detail("max_warning_count", MAX_OCR_WARNING_COUNT.to_string()));
    }
    for warning in &request.warnings {
        if warning.is_empty() || warning.len() > MAX_OCR_WARNING_TEXT_BYTES {
            return Err(ocr_record_error(
                "CORE_OCR_RECORD_WARNING_INVALID",
                "OCR warnings must be non-empty and bounded",
                false,
            )
            .with_detail("max_bytes", MAX_OCR_WARNING_TEXT_BYTES.to_string()));
        }
    }
    Ok(())
}

fn validate_ocr_label(value: &str, field: &'static str) -> Result<(), A2dError> {
    if value.is_empty() || value.len() > MAX_OCR_LABEL_BYTES {
        return Err(ocr_record_error(
            "CORE_OCR_RECORD_LABEL_INVALID",
            "OCR provider labels must be non-empty and bounded",
            false,
        )
        .with_detail("field", field)
        .with_detail("max_bytes", MAX_OCR_LABEL_BYTES.to_string()));
    }
    Ok(())
}

fn validate_non_negative_timestamp(value: i64, field: &'static str) -> Result<(), A2dError> {
    if value < 0 {
        return Err(ocr_record_error(
            "CORE_OCR_RECORD_TIMESTAMP_INVALID",
            "OCR timestamps must be non-negative milliseconds",
            false,
        )
        .with_detail("field", field));
    }
    Ok(())
}

fn select_ocr_asset_id(scan: &Scan, input_kind: CoreOcrInputKind) -> Result<AssetId, A2dError> {
    match input_kind {
        CoreOcrInputKind::Original => Ok(scan.original_asset_id.clone()),
        CoreOcrInputKind::Corrected => scan.corrected_asset_id.clone().ok_or_else(|| {
            ocr_core_error(
                "CORE_OCR_INPUT_ASSET_MISSING",
                "requested corrected OCR input is not available for this scan",
                false,
            )
            .with_detail("scan_id", scan.id().to_string())
            .with_detail("input_kind", input_kind_label(input_kind))
        }),
        CoreOcrInputKind::OcrOptimized => scan.ocr_asset_id.clone().ok_or_else(|| {
            ocr_core_error(
                "CORE_OCR_INPUT_ASSET_MISSING",
                "requested OCR-optimized input is not available for this scan",
                false,
            )
            .with_detail("scan_id", scan.id().to_string())
            .with_detail("input_kind", input_kind_label(input_kind))
        }),
    }
}

fn validate_input_asset(asset: &Asset, input_kind: CoreOcrInputKind) -> Result<(), A2dError> {
    let expected = expected_asset_kind(input_kind);
    if asset.kind != expected {
        return Err(ocr_core_error(
            "CORE_OCR_INPUT_ASSET_KIND_MISMATCH",
            "OCR input asset kind does not match the requested OCR input kind",
            false,
        )
        .with_detail("input_kind", input_kind_label(input_kind))
        .with_detail("expected_asset_kind", asset_kind_label(expected))
        .with_detail("actual_asset_kind", asset_kind_label(asset.kind))
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    if !asset.immutable {
        return Err(ocr_core_error(
            "CORE_OCR_INPUT_ASSET_NOT_IMMUTABLE",
            "OCR input must reference an immutable asset",
            false,
        )
        .with_detail("input_kind", input_kind_label(input_kind))
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    Ok(())
}

fn expected_scan_asset_kind(scan: &Scan, input_asset_id: &AssetId) -> Result<AssetKind, A2dError> {
    if input_asset_id == &scan.original_asset_id {
        return Ok(AssetKind::Original);
    }
    if scan
        .corrected_asset_id
        .as_ref()
        .is_some_and(|asset_id| asset_id == input_asset_id)
    {
        return Ok(AssetKind::Corrected);
    }
    if scan
        .ocr_asset_id
        .as_ref()
        .is_some_and(|asset_id| asset_id == input_asset_id)
    {
        return Ok(AssetKind::Ocr);
    }
    Err(ocr_record_error(
        "CORE_OCR_RECORD_INPUT_ASSET_NOT_OWNED_BY_SCAN",
        "OCR result input asset must be one of the scan's persisted OCR-readable assets",
        false,
    )
    .with_detail("scan_id", scan.id().to_string())
    .with_detail("input_asset_id", input_asset_id.to_string()))
}

fn validate_record_input_asset(asset: &Asset, expected: AssetKind) -> Result<(), A2dError> {
    if asset.kind != expected {
        return Err(ocr_record_error(
            "CORE_OCR_RECORD_INPUT_ASSET_KIND_MISMATCH",
            "OCR result input asset kind does not match the scan-owned asset slot",
            false,
        )
        .with_detail("expected_asset_kind", asset_kind_label(expected))
        .with_detail("actual_asset_kind", asset_kind_label(asset.kind))
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    if !asset.immutable {
        return Err(ocr_record_error(
            "CORE_OCR_RECORD_INPUT_ASSET_NOT_IMMUTABLE",
            "OCR result input asset must be immutable",
            false,
        )
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    Ok(())
}

fn expected_asset_kind(input_kind: CoreOcrInputKind) -> AssetKind {
    match input_kind {
        CoreOcrInputKind::Original => AssetKind::Original,
        CoreOcrInputKind::Corrected => AssetKind::Corrected,
        CoreOcrInputKind::OcrOptimized => AssetKind::Ocr,
    }
}

fn input_kind_label(input_kind: CoreOcrInputKind) -> &'static str {
    match input_kind {
        CoreOcrInputKind::Original => "Original",
        CoreOcrInputKind::Corrected => "Corrected",
        CoreOcrInputKind::OcrOptimized => "OcrOptimized",
    }
}

fn asset_kind_label(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Original => "Original",
        AssetKind::Corrected => "Corrected",
        AssetKind::Ocr => "Ocr",
        AssetKind::Thumbnail => "Thumbnail",
        AssetKind::Export => "Export",
    }
}

fn ocr_core_error(
    code: &'static str,
    developer_message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.input_preparation",
        developer_message,
        retryable,
    )
}

fn ocr_record_error(
    code: &'static str,
    developer_message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.record_result",
        developer_message,
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;
    use a2d_domain::{
        Asset, CaptureSource, EncryptionState, LayoutId, OcrRunId, Page, PageId, PageKind,
        PageState, QualityStatus, Scan, SmartPageId,
    };
    use a2d_storage::{AssetRepository, OcrRunRepository, PageRepository, ScanRepository};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct ScanFixture {
        scan_id: ScanId,
        original_asset_id: AssetId,
    }

    fn open_test_core() -> (Arc<A2dCore>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("a2d-core-ocr-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, dir)
    }

    fn asset(id: AssetId, kind: AssetKind, immutable: bool) -> Asset {
        Asset::new(
            id.clone(),
            kind,
            format!(
                "assets/{}/{}.png",
                asset_kind_label(kind).to_lowercase(),
                id
            ),
            "image/png".to_string(),
            1_024,
            "test-sha256".to_string(),
            100,
            immutable,
            EncryptionState::Plaintext,
        )
    }

    fn insert_scan_fixture(
        core: &A2dCore,
        corrected: Option<(AssetKind, bool)>,
        ocr_optimized: Option<(AssetKind, bool)>,
    ) -> ScanFixture {
        let page_id = PageId::generate();
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("PAGE").unwrap(),
            Some("OCR test page".to_string()),
            PageState::Scanned,
            100,
        );
        let original_asset_id = AssetId::generate();
        let original_asset = asset(original_asset_id.clone(), AssetKind::Original, true);
        let corrected_asset =
            corrected.map(|(kind, immutable)| asset(AssetId::generate(), kind, immutable));
        let ocr_asset =
            ocr_optimized.map(|(kind, immutable)| asset(AssetId::generate(), kind, immutable));
        let scan_id = ScanId::generate();
        let scan = Scan::new(
            scan_id.clone(),
            page_id,
            None,
            CaptureSource::Camera,
            125,
            original_asset_id.clone(),
            corrected_asset.as_ref().map(|asset| asset.id().clone()),
            ocr_asset.as_ref().map(|asset| asset.id().clone()),
            None,
            "test-pipeline".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            true,
            None,
            "fingerprint".to_string(),
        );

        let mut storage = core.lock_storage().unwrap();
        storage
            .transaction(|tx| {
                tx.insert_page(&page)?;
                tx.insert_asset(&original_asset)?;
                if let Some(asset) = &corrected_asset {
                    tx.insert_asset(asset)?;
                }
                if let Some(asset) = &ocr_asset {
                    tx.insert_asset(asset)?;
                }
                tx.insert_scan(&scan)?;
                Ok(())
            })
            .unwrap();

        ScanFixture {
            scan_id,
            original_asset_id,
        }
    }

    fn prepare_request(scan_id: &ScanId, input_kind: CoreOcrInputKind) -> PrepareOcrInputRequest {
        PrepareOcrInputRequest {
            scan_id: scan_id.to_string(),
            input_kind,
            width_px: 1_000,
            height_px: 1_400,
        }
    }

    fn record_request(
        fixture: &ScanFixture,
        status: OcrRunStatus,
        full_text: impl Into<String>,
    ) -> RecordOcrRunRequest {
        RecordOcrRunRequest {
            scan_id: fixture.scan_id.to_string(),
            input_asset_id: fixture.original_asset_id.to_string(),
            provider: "mlkit".to_string(),
            provider_version: "2026.09".to_string(),
            model_name: Some("latin-v1".to_string()),
            status,
            full_text: full_text.into(),
            unavailable_reason: None,
            unavailable_message: None,
            completed_at_ms: Some(250),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn prepare_ocr_input_resolves_original_asset_from_persisted_scan() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);

        let prepared = core
            .prepare_ocr_input(prepare_request(
                &fixture.scan_id,
                CoreOcrInputKind::Original,
            ))
            .unwrap();

        assert_eq!(prepared.scan_id, fixture.scan_id.to_string());
        assert_eq!(
            prepared.input_asset_id,
            fixture.original_asset_id.to_string()
        );
        assert_eq!(prepared.input_kind, CoreOcrInputKind::Original);
        assert_eq!(prepared.media_type, "image/png");
        assert_eq!(prepared.byte_length, 1_024);
        assert_eq!(prepared.width_px, 1_000);
        assert_eq!(prepared.height_px, 1_400);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prepare_ocr_input_does_not_fall_back_when_corrected_asset_is_missing() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);

        let err = core
            .prepare_ocr_input(prepare_request(
                &fixture.scan_id,
                CoreOcrInputKind::Corrected,
            ))
            .unwrap_err();

        assert_eq!(err.code.to_string(), "CORE_OCR_INPUT_ASSET_MISSING");
        assert_eq!(
            err.details.get("input_kind").map(String::as_str),
            Some("Corrected")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prepare_ocr_input_rejects_wrong_asset_kind() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, Some((AssetKind::Ocr, true)), None);

        let err = core
            .prepare_ocr_input(prepare_request(
                &fixture.scan_id,
                CoreOcrInputKind::Corrected,
            ))
            .unwrap_err();

        assert_eq!(err.code.to_string(), "CORE_OCR_INPUT_ASSET_KIND_MISMATCH");
        assert_eq!(
            err.details.get("expected_asset_kind").map(String::as_str),
            Some("Corrected")
        );
        assert_eq!(
            err.details.get("actual_asset_kind").map(String::as_str),
            Some("Ocr")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prepare_ocr_input_rejects_mutable_derived_asset() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, Some((AssetKind::Corrected, false)), None);

        let err = core
            .prepare_ocr_input(prepare_request(
                &fixture.scan_id,
                CoreOcrInputKind::Corrected,
            ))
            .unwrap_err();

        assert_eq!(err.code.to_string(), "CORE_OCR_INPUT_ASSET_NOT_IMMUTABLE");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prepare_ocr_input_validates_dimensions_before_storage_lookup() {
        let (core, dir) = open_test_core();
        let mut request = prepare_request(&ScanId::generate(), CoreOcrInputKind::Original);
        request.width_px = 0;

        let err = core.prepare_ocr_input(request).unwrap_err();

        assert_eq!(err.code.to_string(), "CORE_OCR_IMAGE_DIMENSIONS_INVALID");
        assert_eq!(err.category, ErrorCategory::Ocr);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_run_persists_detected_text_for_scan_owned_asset() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);

        let recorded = core
            .record_ocr_run(record_request(
                &fixture,
                OcrRunStatus::Detected,
                "hello OCR",
            ))
            .unwrap();
        let run_id = OcrRunId::parse(&recorded.ocr_run_id).unwrap();
        let storage = core.lock_storage().unwrap();
        let loaded = storage.get_ocr_run(&run_id).unwrap().unwrap();

        assert_eq!(recorded.status, OcrRunStatus::Detected);
        assert_eq!(loaded.scan_id, fixture.scan_id);
        assert_eq!(loaded.input_asset_id, Some(fixture.original_asset_id));
        assert_eq!(loaded.status, OcrRunStatus::Detected);
        assert_eq!(loaded.full_text, "hello OCR");
        assert_eq!(loaded.provider, "mlkit");
        assert_eq!(loaded.model_name.as_deref(), Some("latin-v1"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_run_persists_unavailable_result_without_text() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);
        let mut request = record_request(&fixture, OcrRunStatus::Unavailable, "");
        request.unavailable_reason = Some(OcrUnavailableReason::ProviderFailed);
        request.unavailable_message = Some("provider failed before returning text".to_string());

        let recorded = core.record_ocr_run(request).unwrap();
        let run_id = OcrRunId::parse(&recorded.ocr_run_id).unwrap();
        let storage = core.lock_storage().unwrap();
        let loaded = storage.get_ocr_run(&run_id).unwrap().unwrap();

        assert_eq!(loaded.status, OcrRunStatus::Unavailable);
        assert_eq!(loaded.full_text, "");
        assert_eq!(
            loaded.unavailable_reason,
            Some(OcrUnavailableReason::ProviderFailed)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_run_rejects_input_asset_not_owned_by_scan() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);
        let rogue_asset_id = AssetId::generate();
        {
            let storage = core.lock_storage().unwrap();
            storage
                .insert_asset(&asset(rogue_asset_id.clone(), AssetKind::Original, true))
                .unwrap();
        }
        let mut request = record_request(&fixture, OcrRunStatus::Detected, "hello OCR");
        request.input_asset_id = rogue_asset_id.to_string();

        let err = core.record_ocr_run(request).unwrap_err();

        assert_eq!(
            err.code.to_string(),
            "CORE_OCR_RECORD_INPUT_ASSET_NOT_OWNED_BY_SCAN"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_run_rejects_empty_detected_text_before_storage() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);

        let err = core
            .record_ocr_run(record_request(&fixture, OcrRunStatus::Detected, ""))
            .unwrap_err();

        assert_eq!(err.code.to_string(), "OCR_RUN_DETECTED_TEXT_EMPTY");
        std::fs::remove_dir_all(&dir).ok();
    }
}
