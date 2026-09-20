use crate::{A2dCore, CoreOcrTextPoint};
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, OcrRunStatus, ScanId};
use a2d_storage::{OcrReadbackRepository, ScanRepository, TextRegionRepository};

pub const MAX_OCR_REGIONS_PER_RUN: u32 = 20_000;
pub const MAX_OCR_REGION_PAGE_SIZE: u32 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadOcrRegionPageRequest {
    pub scan_id: String,
    pub offset: u32,
    pub page_size: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrRegionPageItem {
    pub text_region_id: String,
    pub ocr_run_id: String,
    pub polygon: Vec<CoreOcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrRegionPage {
    pub scan_id: String,
    pub ocr_run_id: Option<String>,
    pub total_count: u32,
    pub returned_count: u32,
    pub offset: u32,
    pub next_offset: Option<u32>,
    pub has_more: bool,
    pub complete: bool,
    pub regions: Vec<OcrRegionPageItem>,
}

impl A2dCore {
    /// Loads one bounded, deterministic page of regions for the latest OCR run on a scan.
    ///
    /// The 20,000-region product maximum and 1,000-row transport page maximum are Rust-owned.
    /// A metadata-only request uses page_size=0. Offsets beyond the authoritative durable count
    /// fail closed instead of being normalized, and a durable count beyond the supported maximum
    /// is reported as an integrity error rather than silently truncated.
    pub fn load_ocr_region_page(
        &self,
        request: LoadOcrRegionPageRequest,
    ) -> Result<OcrRegionPage, A2dError> {
        if request.page_size > MAX_OCR_REGION_PAGE_SIZE {
            return Err(pagination_error(
                "CORE_OCR_REGION_PAGE_SIZE_EXCEEDED",
                ErrorCategory::Ocr,
                "OCR region page size exceeds the configured maximum",
            )
            .with_detail("page_size", request.page_size.to_string())
            .with_detail("max_page_size", MAX_OCR_REGION_PAGE_SIZE.to_string()));
        }

        let scan_id = ScanId::parse(&request.scan_id)?;
        let storage = self.lock_storage()?;
        storage.get_scan(&scan_id)?.ok_or_else(|| {
            pagination_error(
                "CORE_OCR_REGION_PAGE_SCAN_MISSING",
                ErrorCategory::Ocr,
                "OCR region pagination requires an existing persisted scan",
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;

        let Some(run) = storage.latest_ocr_run_for_scan(&scan_id)? else {
            if request.offset != 0 {
                return Err(offset_error(request.offset, 0));
            }
            return Ok(OcrRegionPage {
                scan_id: scan_id.to_string(),
                ocr_run_id: None,
                total_count: 0,
                returned_count: 0,
                offset: 0,
                next_offset: None,
                has_more: false,
                complete: true,
                regions: Vec::new(),
            });
        };

        let run_id = run.id().clone();
        let total_count =
            u32::try_from(storage.count_text_regions_for_ocr_run(&run_id)?).map_err(|_| {
                pagination_error(
                    "CORE_OCR_REGION_COUNT_OVERFLOW",
                    ErrorCategory::Integrity,
                    "OCR region count exceeds the portable u32 representation",
                )
                .with_detail("ocr_run_id", run_id.to_string())
            })?;
        if total_count > MAX_OCR_REGIONS_PER_RUN {
            return Err(pagination_error(
                "CORE_OCR_REGION_COUNT_EXCEEDS_SUPPORTED_MAXIMUM",
                ErrorCategory::Integrity,
                "durable OCR region count exceeds the supported per-run maximum",
            )
            .with_detail("ocr_run_id", run_id.to_string())
            .with_detail("region_count", total_count.to_string())
            .with_detail("max_region_count", MAX_OCR_REGIONS_PER_RUN.to_string()));
        }
        if request.offset > total_count {
            return Err(offset_error(request.offset, total_count));
        }

        let regions = if request.page_size == 0 || run.status != OcrRunStatus::Detected {
            Vec::new()
        } else {
            storage
                .list_text_regions_for_ocr_run(&run_id)?
                .into_iter()
                .skip(request.offset as usize)
                .take(request.page_size as usize)
                .map(|region| OcrRegionPageItem {
                    text_region_id: region.id().to_string(),
                    ocr_run_id: region.ocr_run_id.to_string(),
                    polygon: region
                        .polygon
                        .into_iter()
                        .map(|(x, y)| CoreOcrTextPoint { x, y })
                        .collect(),
                    text: region.text,
                    confidence: region.confidence,
                    created_at_ms: region.created_at_ms,
                })
                .collect::<Vec<_>>()
        };
        let returned_count = u32::try_from(regions.len()).expect("page size is bounded to u32");
        let consumed = request.offset.checked_add(returned_count).ok_or_else(|| {
            pagination_error(
                "CORE_OCR_REGION_PAGE_OFFSET_OVERFLOW",
                ErrorCategory::Integrity,
                "OCR region page offset arithmetic overflowed",
            )
        })?;
        let has_more = consumed < total_count;
        if has_more && returned_count == 0 && request.page_size != 0 {
            return Err(pagination_error(
                "CORE_OCR_REGION_PAGE_NO_PROGRESS",
                ErrorCategory::Integrity,
                "OCR region pagination returned no rows before the authoritative end",
            )
            .with_detail("ocr_run_id", run_id.to_string())
            .with_detail("offset", request.offset.to_string())
            .with_detail("total_count", total_count.to_string()));
        }
        let next_offset = has_more.then_some(consumed);

        Ok(OcrRegionPage {
            scan_id: scan_id.to_string(),
            ocr_run_id: Some(run_id.to_string()),
            total_count,
            returned_count,
            offset: request.offset,
            next_offset,
            has_more,
            complete: !has_more,
            regions,
        })
    }
}

fn offset_error(offset: u32, total_count: u32) -> A2dError {
    pagination_error(
        "CORE_OCR_REGION_PAGE_OFFSET_INVALID",
        ErrorCategory::Ocr,
        "OCR region page offset exceeds the authoritative durable count",
    )
    .with_detail("offset", offset.to_string())
    .with_detail("total_count", total_count.to_string())
}

fn pagination_error(
    code: &'static str,
    category: ErrorCategory,
    message: &'static str,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.ocr.region_pagination",
        message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CoreOcrInputKind, LoadLatestOcrOutputRequest, OpenLibraryRequest,
        RecordOcrTextRegionRequest, RecordOcrTextRegionsRequest,
    };
    use a2d_domain::{
        Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRun, OcrRunId,
        Page, PageId, PageKind, PageState, Provenance, QualityStatus, Scan, SmartPageId,
    };
    use a2d_storage::{AssetRepository, OcrRunRepository, PageRepository, ScanRepository};
    use image::{ImageBuffer, Rgba};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct ScanFixture {
        scan_id: ScanId,
        page_id: PageId,
        original_asset_id: AssetId,
    }

    fn open_test_core() -> (Arc<A2dCore>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "a2d-core-ocr-region-pagination-test-{}",
            PageId::generate()
        ));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, dir)
    }

    fn asset(id: AssetId, byte_length: u64) -> Asset {
        Asset::new(
            id.clone(),
            AssetKind::Original,
            format!("assets/originals/{id}.png"),
            "image/png".to_string(),
            byte_length,
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
        let relative_path = format!("assets/originals/{original_asset_id}.png");
        let source_path = PathBuf::from(core.library_path()).join(&relative_path);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(200, 100, Rgba([255, 255, 255, 255]))
            .save(&source_path)
            .unwrap();
        let byte_length = source_path.metadata().unwrap().len();
        let scan_id = ScanId::generate();
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("PAGE").unwrap(),
            Some("OCR pagination qualification page".to_string()),
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
            .insert_asset(&asset(original_asset_id.clone(), byte_length))
            .unwrap();
        storage.insert_scan(&scan).unwrap();
        ScanFixture {
            scan_id,
            page_id,
            original_asset_id,
        }
    }

    fn insert_detected_run(core: &A2dCore, fixture: &ScanFixture) -> OcrRunId {
        let run = OcrRun::detected(
            OcrRunId::generate(),
            fixture.scan_id.clone(),
            Some(fixture.original_asset_id.clone()),
            "mlkit".to_string(),
            "2026.09".to_string(),
            Some("latin-v1".to_string()),
            "many regions".to_string(),
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

    fn region(index: u32) -> RecordOcrTextRegionRequest {
        RecordOcrTextRegionRequest {
            polygon: vec![
                CoreOcrTextPoint { x: 0.0, y: 0.0 },
                CoreOcrTextPoint { x: 120.0, y: 0.0 },
                CoreOcrTextPoint { x: 120.0, y: 32.0 },
                CoreOcrTextPoint { x: 0.0, y: 32.0 },
            ],
            text: format!("region-{index:04}"),
            confidence: Some(0.85),
            created_at_ms: Some(1_000 + i64::from(index)),
        }
    }

    #[test]
    fn pagination_reads_more_than_previous_readback_ceiling_across_stable_pages() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_detected_run(&core, &fixture);
        core.prepare_ocr_input(crate::PrepareOcrInputRequest {
            scan_id: fixture.scan_id.to_string(),
            input_kind: CoreOcrInputKind::Original,
            width_px: 200,
            height_px: 100,
        })
        .unwrap();
        core.record_ocr_text_regions(RecordOcrTextRegionsRequest {
            ocr_run_id: run_id.to_string(),
            regions: (0..1_001).map(region).collect(),
        })
        .unwrap();

        let bounded_preview = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 1_000,
            })
            .unwrap()
            .latest_run
            .unwrap();
        assert_eq!(bounded_preview.text_region_count, 1_001);
        assert_eq!(bounded_preview.text_regions.len(), 1_000);

        let first = core
            .load_ocr_region_page(LoadOcrRegionPageRequest {
                scan_id: fixture.scan_id.to_string(),
                offset: 0,
                page_size: 1_000,
            })
            .unwrap();
        assert_eq!(
            first.ocr_run_id.as_deref(),
            Some(run_id.to_string().as_str())
        );
        assert_eq!(first.total_count, 1_001);
        assert_eq!(first.returned_count, 1_000);
        assert_eq!(first.offset, 0);
        assert_eq!(first.next_offset, Some(1_000));
        assert!(first.has_more);
        assert!(!first.complete);
        assert_eq!(first.regions.first().unwrap().text, "region-0000");
        assert_eq!(first.regions.last().unwrap().text, "region-0999");

        let second = core
            .load_ocr_region_page(LoadOcrRegionPageRequest {
                scan_id: fixture.scan_id.to_string(),
                offset: first.next_offset.unwrap(),
                page_size: 1_000,
            })
            .unwrap();
        assert_eq!(second.total_count, 1_001);
        assert_eq!(second.returned_count, 1);
        assert_eq!(second.offset, 1_000);
        assert_eq!(second.next_offset, None);
        assert!(!second.has_more);
        assert!(second.complete);
        assert_eq!(second.regions[0].text, "region-1000");

        let unique_ids: std::collections::HashSet<_> = first
            .regions
            .iter()
            .chain(second.regions.iter())
            .map(|region| region.text_region_id.clone())
            .collect();
        assert_eq!(unique_ids.len(), 1_001);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pagination_rejects_offsets_past_authoritative_region_count() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_detected_run(&core, &fixture);
        core.prepare_ocr_input(crate::PrepareOcrInputRequest {
            scan_id: fixture.scan_id.to_string(),
            input_kind: CoreOcrInputKind::Original,
            width_px: 200,
            height_px: 100,
        })
        .unwrap();
        core.record_ocr_text_regions(RecordOcrTextRegionsRequest {
            ocr_run_id: run_id.to_string(),
            regions: (0..64).map(region).collect(),
        })
        .unwrap();

        let error = core
            .load_ocr_region_page(LoadOcrRegionPageRequest {
                scan_id: fixture.scan_id.to_string(),
                offset: 65,
                page_size: 1,
            })
            .unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "CORE_OCR_REGION_PAGE_OFFSET_INVALID"
        );
        assert_eq!(error.details.get("offset"), Some(&"65".to_string()));
        assert_eq!(error.details.get("total_count"), Some(&"64".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
