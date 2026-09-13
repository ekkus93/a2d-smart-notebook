use crate::A2dCore;
use a2d_domain::{
    A2dError, Asset, AssetId, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, Scan, ScanId,
};
use a2d_storage::{AssetRepository, ScanRepository};

const MAX_OCR_IMAGE_DIMENSION_PX: u32 = 12_000;
const MAX_OCR_IMAGE_PIXELS: u64 = 80_000_000;

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

fn ocr_core_error(code: &'static str, developer_message: impl Into<String>, retryable: bool) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.input_preparation",
        developer_message,
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;
    use a2d_domain::{
        Asset, CaptureSource, EncryptionState, LayoutId, Page, PageId, PageKind, PageState,
        QualityStatus, Scan, SmartPageId,
    };
    use a2d_storage::{AssetRepository, PageRepository, ScanRepository};
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
            format!("assets/{}/{}.png", asset_kind_label(kind).to_lowercase(), id),
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
        let corrected_asset = corrected.map(|(kind, immutable)| asset(AssetId::generate(), kind, immutable));
        let ocr_asset = ocr_optimized.map(|(kind, immutable)| asset(AssetId::generate(), kind, immutable));
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

    #[test]
    fn prepare_ocr_input_resolves_original_asset_from_persisted_scan() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core, None, None);

        let prepared = core
            .prepare_ocr_input(prepare_request(&fixture.scan_id, CoreOcrInputKind::Original))
            .unwrap();

        assert_eq!(prepared.scan_id, fixture.scan_id.to_string());
        assert_eq!(prepared.input_asset_id, fixture.original_asset_id.to_string());
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
            .prepare_ocr_input(prepare_request(&fixture.scan_id, CoreOcrInputKind::Corrected))
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
            .prepare_ocr_input(prepare_request(&fixture.scan_id, CoreOcrInputKind::Corrected))
            .unwrap_err();

        assert_eq!(
            err.code.to_string(),
            "CORE_OCR_INPUT_ASSET_KIND_MISMATCH"
        );
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
            .prepare_ocr_input(prepare_request(&fixture.scan_id, CoreOcrInputKind::Corrected))
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
}
