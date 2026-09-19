//! Rust-owned OCR source asset identity and image geometry resolution.
//!
//! Page Viewer and queue callers must not manufacture OCR source dimensions. This module resolves
//! the scan-owned OCR-readable asset, validates the persisted asset row, reads the actual encoded
//! image dimensions from the committed library file, and returns the durable source identity and
//! bounds together.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use a2d_domain::{
    A2dError, Asset, AssetId, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, Scan, ScanId,
};
use a2d_storage::{AssetRepository, ScanRepository};
use image::{ImageFormat, ImageReader, Limits};

use super::{A2dCore, CoreOcrInputKind};

const MAX_OCR_SOURCE_DIMENSION_PX: u32 = 12_000;
const MAX_OCR_SOURCE_PIXELS: u64 = 80_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrSourceGeometry {
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
    /// Resolve one scan-owned OCR source asset and its real encoded image dimensions.
    pub fn resolve_ocr_source_geometry(
        &self,
        scan_id: &str,
        input_kind: CoreOcrInputKind,
    ) -> Result<OcrSourceGeometry, A2dError> {
        let scan_id = ScanId::parse(scan_id)?;
        let storage = self.lock_storage()?;
        let scan = storage.get_scan(&scan_id)?.ok_or_else(|| {
            geometry_error(
                "CORE_OCR_SOURCE_SCAN_MISSING",
                "OCR source geometry resolution requires an existing persisted scan",
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;
        let input_asset_id = select_ocr_asset_id(&scan, input_kind)?;
        let asset = storage.get_asset(&input_asset_id)?.ok_or_else(|| {
            geometry_error(
                "CORE_OCR_SOURCE_ASSET_MISSING_ROW",
                "OCR source geometry resolution requires the selected asset row to exist",
            )
            .with_detail("scan_id", scan_id.to_string())
            .with_detail("input_kind", input_kind_label(input_kind))
            .with_detail("input_asset_id", input_asset_id.to_string())
        })?;
        validate_source_asset(&asset, input_kind)?;
        drop(storage);

        let (width_px, height_px) = read_source_dimensions(&self.library_path, &asset)?;
        validate_source_dimensions(width_px, height_px)?;

        Ok(OcrSourceGeometry {
            scan_id: scan_id.to_string(),
            input_asset_id: input_asset_id.to_string(),
            input_kind,
            media_type: asset.media_type,
            relative_path: asset.relative_path,
            byte_length: asset.byte_length,
            width_px,
            height_px,
        })
    }
}

fn read_source_dimensions(library_path: &Path, asset: &Asset) -> Result<(u32, u32), A2dError> {
    let root = library_path.canonicalize().map_err(|error| {
        geometry_error(
            "CORE_OCR_SOURCE_LIBRARY_ROOT_UNAVAILABLE",
            format!("failed to resolve library root for OCR source geometry: {error}"),
        )
    })?;
    let candidate = library_path.join(&asset.relative_path);
    let canonical = candidate.canonicalize().map_err(|error| {
        geometry_error(
            "CORE_OCR_SOURCE_FILE_UNAVAILABLE",
            format!("failed to resolve OCR source file: {error}"),
        )
        .with_detail("input_asset_id", asset.id().to_string())
        .with_detail("relative_path", asset.relative_path.clone())
    })?;
    if !canonical.starts_with(&root) {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_FILE_OUTSIDE_LIBRARY",
            "OCR source asset resolved outside the open library root",
        )
        .with_detail("input_asset_id", asset.id().to_string())
        .with_detail("relative_path", asset.relative_path.clone()));
    }
    if !canonical.is_file() {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_FILE_NOT_FOUND",
            "OCR source asset file is missing",
        )
        .with_detail("input_asset_id", asset.id().to_string())
        .with_detail("relative_path", asset.relative_path.clone()));
    }

    let format = image_format_for_media_type(&asset.media_type)?;
    let file = File::open(&canonical).map_err(|error| {
        geometry_error(
            "CORE_OCR_SOURCE_FILE_OPEN_FAILED",
            format!("failed to open OCR source asset file: {error}"),
        )
        .with_detail("input_asset_id", asset.id().to_string())
        .with_detail("relative_path", asset.relative_path.clone())
    })?;
    let mut reader = ImageReader::with_format(BufReader::new(file), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_OCR_SOURCE_DIMENSION_PX);
    limits.max_image_height = Some(MAX_OCR_SOURCE_DIMENSION_PX);
    limits.max_alloc = Some(MAX_OCR_SOURCE_PIXELS.saturating_mul(4));
    reader.limits(limits);
    reader.into_dimensions().map_err(|error| {
        geometry_error(
            "CORE_OCR_SOURCE_DIMENSIONS_UNAVAILABLE",
            format!("failed to read OCR source image dimensions: {error}"),
        )
        .with_detail("input_asset_id", asset.id().to_string())
        .with_detail("relative_path", asset.relative_path.clone())
    })
}

fn image_format_for_media_type(media_type: &str) -> Result<ImageFormat, A2dError> {
    match media_type {
        "image/png" => Ok(ImageFormat::Png),
        unsupported => Err(geometry_error(
            "CORE_OCR_SOURCE_MEDIA_TYPE_UNSUPPORTED",
            "OCR source geometry resolution only supports reviewed image media types",
        )
        .with_detail("media_type", unsupported.to_string())),
    }
}

fn validate_source_dimensions(width_px: u32, height_px: u32) -> Result<(), A2dError> {
    if width_px == 0 || height_px == 0 {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_DIMENSIONS_INVALID",
            "OCR source image dimensions must be non-zero",
        )
        .with_detail("width_px", width_px.to_string())
        .with_detail("height_px", height_px.to_string()));
    }
    if width_px > MAX_OCR_SOURCE_DIMENSION_PX || height_px > MAX_OCR_SOURCE_DIMENSION_PX {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_DIMENSIONS_EXCEED_LIMIT",
            "OCR source image dimensions exceed the configured OCR limit",
        )
        .with_detail("width_px", width_px.to_string())
        .with_detail("height_px", height_px.to_string())
        .with_detail(
            "max_image_dimension_px",
            MAX_OCR_SOURCE_DIMENSION_PX.to_string(),
        ));
    }
    let pixels = u64::from(width_px) * u64::from(height_px);
    if pixels > MAX_OCR_SOURCE_PIXELS {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_PIXELS_EXCEED_LIMIT",
            "OCR source image pixel count exceeds the configured OCR limit",
        )
        .with_detail("pixels", pixels.to_string())
        .with_detail("max_image_pixels", MAX_OCR_SOURCE_PIXELS.to_string()));
    }
    Ok(())
}

fn select_ocr_asset_id(scan: &Scan, input_kind: CoreOcrInputKind) -> Result<AssetId, A2dError> {
    match input_kind {
        CoreOcrInputKind::Original => Ok(scan.original_asset_id.clone()),
        CoreOcrInputKind::Corrected => scan.corrected_asset_id.clone().ok_or_else(|| {
            geometry_error(
                "CORE_OCR_SOURCE_ASSET_MISSING",
                "requested corrected OCR source is not available for this scan",
            )
            .with_detail("scan_id", scan.id().to_string())
            .with_detail("input_kind", input_kind_label(input_kind))
        }),
        CoreOcrInputKind::OcrOptimized => scan.ocr_asset_id.clone().ok_or_else(|| {
            geometry_error(
                "CORE_OCR_SOURCE_ASSET_MISSING",
                "requested OCR-optimized source is not available for this scan",
            )
            .with_detail("scan_id", scan.id().to_string())
            .with_detail("input_kind", input_kind_label(input_kind))
        }),
    }
}

fn validate_source_asset(asset: &Asset, input_kind: CoreOcrInputKind) -> Result<(), A2dError> {
    let expected = expected_asset_kind(input_kind);
    if asset.kind != expected {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_ASSET_KIND_MISMATCH",
            "OCR source asset kind does not match the requested OCR input kind",
        )
        .with_detail("input_kind", input_kind_label(input_kind))
        .with_detail("expected_asset_kind", asset_kind_label(expected))
        .with_detail("actual_asset_kind", asset_kind_label(asset.kind))
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    if !asset.immutable {
        return Err(geometry_error(
            "CORE_OCR_SOURCE_ASSET_NOT_IMMUTABLE",
            "OCR source must reference an immutable asset",
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

fn geometry_error(code: &'static str, developer_message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.source_geometry",
        developer_message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;
    use a2d_domain::{
        CaptureSource, EncryptionState, LayoutId, Page, PageId, PageKind, PageState,
        QualityStatus, SmartPageId,
    };
    use a2d_storage::{AssetRepository, PageRepository, ScanRepository};
    use image::{ImageBuffer, Rgba};
    use std::sync::Arc;

    struct ScanFixture {
        scan_id: ScanId,
        original_asset_id: AssetId,
        ocr_asset_id: AssetId,
    }

    fn open_test_core(label: &str) -> (Arc<A2dCore>, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "a2d-ocr-source-geometry-{label}-{}",
            PageId::generate()
        ));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, root)
    }

    fn write_png(root: &Path, relative_path: &str, width: u32, height: u32) -> u64 {
        let path = root.join(relative_path);
        path.parent().unwrap().mkdirs_or_create();
        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(width, height, Rgba([255, 255, 255, 255]));
        image.save(&path).unwrap();
        path.metadata().unwrap().len()
    }

    trait CreateDirs {
        fn mkdirs_or_create(&self);
    }

    impl CreateDirs for Path {
        fn mkdirs_or_create(&self) {
            std::fs::create_dir_all(self).unwrap();
        }
    }

    fn asset(id: AssetId, kind: AssetKind, relative_path: String, byte_length: u64) -> Asset {
        Asset::new(
            id,
            kind,
            relative_path,
            "image/png".to_string(),
            byte_length,
            "test-sha256".to_string(),
            100,
            true,
            EncryptionState::Plaintext,
        )
    }

    fn insert_scan_fixture(core: &A2dCore, root: &Path) -> ScanFixture {
        let page_id = PageId::generate();
        let original_asset_id = AssetId::generate();
        let ocr_asset_id = AssetId::generate();
        let original_relative_path = format!("assets/originals/{original_asset_id}.png");
        let ocr_relative_path = format!("assets/ocr/{ocr_asset_id}.png");
        let original_bytes = write_png(root, &original_relative_path, 31, 43);
        let ocr_bytes = write_png(root, &ocr_relative_path, 13, 17);
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("PAGE").unwrap(),
            Some("OCR source geometry test".to_string()),
            PageState::Scanned,
            100,
        );
        let scan_id = ScanId::generate();
        let scan = Scan::new(
            scan_id.clone(),
            page_id,
            None,
            CaptureSource::Camera,
            125,
            original_asset_id.clone(),
            None,
            Some(ocr_asset_id.clone()),
            None,
            "test-pipeline".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            true,
            None,
            "fingerprint".to_string(),
        );
        let original_asset = asset(
            original_asset_id.clone(),
            AssetKind::Original,
            original_relative_path,
            original_bytes,
        );
        let ocr_asset = asset(
            ocr_asset_id.clone(),
            AssetKind::Ocr,
            ocr_relative_path,
            ocr_bytes,
        );
        let storage = core.lock_storage().unwrap();
        storage.insert_page(&page).unwrap();
        storage.insert_asset(&original_asset).unwrap();
        storage.insert_asset(&ocr_asset).unwrap();
        storage.insert_scan(&scan).unwrap();
        ScanFixture {
            scan_id,
            original_asset_id,
            ocr_asset_id,
        }
    }

    #[test]
    fn resolves_actual_original_source_geometry_from_committed_asset() {
        let (core, root) = open_test_core("original");
        let fixture = insert_scan_fixture(&core, &root);

        let source = core
            .resolve_ocr_source_geometry(&fixture.scan_id.to_string(), CoreOcrInputKind::Original)
            .unwrap();

        assert_eq!(source.scan_id, fixture.scan_id.to_string());
        assert_eq!(source.input_asset_id, fixture.original_asset_id.to_string());
        assert_eq!(source.input_kind, CoreOcrInputKind::Original);
        assert_eq!(source.media_type, "image/png");
        assert_eq!(source.width_px, 31);
        assert_eq!(source.height_px, 43);
        assert!(source.relative_path.starts_with("assets/originals/"));
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resolves_actual_ocr_optimized_source_geometry_from_committed_asset() {
        let (core, root) = open_test_core("ocr");
        let fixture = insert_scan_fixture(&core, &root);

        let source = core
            .resolve_ocr_source_geometry(
                &fixture.scan_id.to_string(),
                CoreOcrInputKind::OcrOptimized,
            )
            .unwrap();

        assert_eq!(source.input_asset_id, fixture.ocr_asset_id.to_string());
        assert_eq!(source.input_kind, CoreOcrInputKind::OcrOptimized);
        assert_eq!(source.width_px, 13);
        assert_eq!(source.height_px, 17);
        assert!(source.relative_path.starts_with("assets/ocr/"));
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_source_file_fails_closed() {
        let (core, root) = open_test_core("missing");
        let fixture = insert_scan_fixture(&core, &root);
        let missing_file = root.join(format!("assets/originals/{}.png", fixture.original_asset_id));
        std::fs::remove_file(missing_file).unwrap();

        let error = core
            .resolve_ocr_source_geometry(&fixture.scan_id.to_string(), CoreOcrInputKind::Original)
            .unwrap_err();

        assert_eq!(error.code.to_string(), "CORE_OCR_SOURCE_FILE_UNAVAILABLE");
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }
}
