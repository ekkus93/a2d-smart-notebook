use crate::A2dCore;
use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, OcrRun, OcrRunId, OcrRunStatus,
    OcrUnavailableReason, ScanId, TextRegion, TextRegionId, system_now_ms,
};
use a2d_storage::{OcrReadbackRepository, ScanRepository, TextRegionRepository};

const MAX_OCR_TEXT_REGION_BATCH_SIZE: usize = 20_000;
const MAX_OCR_READBACK_REGION_LIMIT: u32 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoreOcrTextPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordOcrTextRegionRequest {
    pub polygon: Vec<CoreOcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordOcrTextRegionsRequest {
    pub ocr_run_id: String,
    pub regions: Vec<RecordOcrTextRegionRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedOcrTextRegion {
    pub text_region_id: String,
    pub ocr_run_id: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedOcrTextRegions {
    pub ocr_run_id: String,
    pub regions: Vec<RecordedOcrTextRegion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadLatestOcrOutputRequest {
    pub scan_id: String,
    pub region_limit: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedOcrTextRegion {
    pub text_region_id: String,
    pub ocr_run_id: String,
    pub polygon: Vec<CoreOcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedOcrRun {
    pub ocr_run_id: String,
    pub scan_id: String,
    pub input_asset_id: Option<String>,
    pub provider: String,
    pub provider_version: String,
    pub model_name: Option<String>,
    pub status: OcrRunStatus,
    pub full_text: String,
    pub unavailable_reason: Option<OcrUnavailableReason>,
    pub unavailable_message: Option<String>,
    pub completed_at_ms: Option<i64>,
    pub warnings: Vec<String>,
    pub text_region_count: u32,
    pub text_regions: Vec<LoadedOcrTextRegion>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedOcrOutput {
    pub scan_id: String,
    pub latest_run: Option<LoadedOcrRun>,
}

impl A2dCore {
    /// Persists OCR text regions for an existing detected OCR run.
    ///
    /// Android/platform providers may detect block/line/element geometry, but Rust still owns the
    /// durable region rows. This method creates typed `TextRegion` values, bounds the batch, and
    /// inserts the batch transactionally through storage. The storage repository independently
    /// rejects regions for `NoTextDetected` and `Unavailable` OCR runs so failures cannot grow
    /// fabricated text rows.
    pub fn record_ocr_text_regions(
        &self,
        request: RecordOcrTextRegionsRequest,
    ) -> Result<RecordedOcrTextRegions, A2dError> {
        if request.regions.is_empty() {
            return Err(ocr_region_error(
                "CORE_OCR_TEXT_REGION_BATCH_EMPTY",
                "OCR text-region recording requires at least one region",
                false,
            ));
        }
        if request.regions.len() > MAX_OCR_TEXT_REGION_BATCH_SIZE {
            return Err(ocr_region_error(
                "CORE_OCR_TEXT_REGION_BATCH_EXCEEDS_LIMIT",
                "OCR text-region batch exceeds the configured limit",
                false,
            )
            .with_detail("region_count", request.regions.len().to_string())
            .with_detail(
                "max_region_count",
                MAX_OCR_TEXT_REGION_BATCH_SIZE.to_string(),
            ));
        }

        let ocr_run_id = OcrRunId::parse(&request.ocr_run_id)?;
        let fallback_created_at_ms = system_now_ms()?;
        let mut typed_regions = Vec::with_capacity(request.regions.len());
        for (index, region) in request.regions.into_iter().enumerate() {
            let created_at_ms = region.created_at_ms.unwrap_or(fallback_created_at_ms);
            if created_at_ms < 0 {
                return Err(ocr_region_error(
                    "CORE_OCR_TEXT_REGION_TIMESTAMP_INVALID",
                    "OCR text-region timestamps must be non-negative milliseconds",
                    false,
                )
                .with_detail("region_index", index.to_string()));
            }
            let polygon = region
                .polygon
                .into_iter()
                .map(|point| (point.x, point.y))
                .collect::<Vec<_>>();
            let text_region = TextRegion::new(
                TextRegionId::try_generate()?,
                ocr_run_id.clone(),
                polygon,
                region.text,
                region.confidence,
                created_at_ms,
            )
            .map_err(|error| error.with_detail("region_index", index.to_string()))?;
            typed_regions.push(text_region);
        }

        let mut storage = self.lock_storage()?;
        storage.transaction(|tx| {
            for region in &typed_regions {
                tx.insert_text_region(region)?;
            }
            Ok(())
        })?;

        let regions = typed_regions
            .into_iter()
            .map(|region| RecordedOcrTextRegion {
                text_region_id: region.id().to_string(),
                ocr_run_id: region.ocr_run_id.to_string(),
                text: region.text,
            })
            .collect();

        Ok(RecordedOcrTextRegions {
            ocr_run_id: ocr_run_id.to_string(),
            regions,
        })
    }

    /// Loads the latest persisted OCR outcome for one scan, including a bounded region preview.
    ///
    /// This is the readback half of the Android-facing OCR integration: after app restart, Android
    /// can hydrate Page Viewer state from Rust-owned OCR rows instead of relying on transient
    /// in-memory workflow results. The scan must exist, but a scan with no OCR run returns
    /// `latest_run = None` rather than fabricating empty text.
    pub fn load_latest_ocr_output(
        &self,
        request: LoadLatestOcrOutputRequest,
    ) -> Result<LoadedOcrOutput, A2dError> {
        if request.region_limit > MAX_OCR_READBACK_REGION_LIMIT {
            return Err(ocr_region_error(
                "CORE_OCR_READBACK_REGION_LIMIT_EXCEEDED",
                "OCR readback region limit exceeds the configured maximum",
                false,
            )
            .with_detail("region_limit", request.region_limit.to_string())
            .with_detail(
                "max_region_limit",
                MAX_OCR_READBACK_REGION_LIMIT.to_string(),
            ));
        }

        let scan_id = ScanId::parse(&request.scan_id)?;
        let storage = self.lock_storage()?;
        storage.get_scan(&scan_id)?.ok_or_else(|| {
            ocr_region_error(
                "CORE_OCR_READBACK_SCAN_MISSING",
                "OCR readback requires an existing persisted scan",
                false,
            )
            .with_detail("scan_id", scan_id.to_string())
        })?;

        let Some(run) = storage.latest_ocr_run_for_scan(&scan_id)? else {
            return Ok(LoadedOcrOutput {
                scan_id: scan_id.to_string(),
                latest_run: None,
            });
        };
        let run_id = run.id().clone();
        let text_region_count = storage.count_text_regions_for_ocr_run(&run_id)? as u32;
        let text_regions = if request.region_limit == 0 || run.status != OcrRunStatus::Detected {
            Vec::new()
        } else {
            storage
                .list_text_regions_for_ocr_run(&run_id)?
                .into_iter()
                .take(request.region_limit as usize)
                .map(loaded_text_region)
                .collect()
        };

        Ok(LoadedOcrOutput {
            scan_id: scan_id.to_string(),
            latest_run: Some(loaded_ocr_run(run, text_region_count, text_regions)),
        })
    }
}

fn loaded_ocr_run(
    run: OcrRun,
    text_region_count: u32,
    text_regions: Vec<LoadedOcrTextRegion>,
) -> LoadedOcrRun {
    LoadedOcrRun {
        ocr_run_id: run.id().to_string(),
        scan_id: run.scan_id.to_string(),
        input_asset_id: run.input_asset_id.map(|id| id.to_string()),
        provider: run.provider,
        provider_version: run.provider_version,
        model_name: run.model_name,
        status: run.status,
        full_text: run.full_text,
        unavailable_reason: run.unavailable_reason,
        unavailable_message: run.unavailable_message,
        completed_at_ms: run.completed_at_ms,
        warnings: run.warnings,
        text_region_count,
        text_regions,
    }
}

fn loaded_text_region(region: TextRegion) -> LoadedOcrTextRegion {
    LoadedOcrTextRegion {
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
    }
}

fn ocr_region_error(
    code: &'static str,
    developer_message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.text_region",
        developer_message,
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;
    use a2d_domain::{
        Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrRun, OcrRunStatus,
        Page, PageId, PageKind, PageState, Provenance, QualityStatus, Scan, ScanId, SmartPageId,
    };
    use a2d_storage::{
        AssetRepository, OcrRunRepository, PageRepository, ScanRepository, TextRegionRepository,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    struct ScanFixture {
        scan_id: ScanId,
        page_id: PageId,
        original_asset_id: AssetId,
    }

    fn open_test_core() -> (Arc<A2dCore>, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("a2d-core-ocr-region-test-{}", PageId::generate()));
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
            Some("OCR text-region core page".to_string()),
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

    fn insert_ocr_run(core: &A2dCore, fixture: &ScanFixture, status: OcrRunStatus) -> OcrRunId {
        let run = match status {
            OcrRunStatus::Detected => OcrRun::detected(
                OcrRunId::generate(),
                fixture.scan_id.clone(),
                Some(fixture.original_asset_id.clone()),
                "mlkit".to_string(),
                "2026.09".to_string(),
                Some("latin-v1".to_string()),
                "first line\nsecond line".to_string(),
                Some(250),
                Vec::new(),
                provenance(&fixture.page_id, &fixture.scan_id),
            )
            .unwrap(),
            OcrRunStatus::NoTextDetected => OcrRun::no_text_detected(
                OcrRunId::generate(),
                fixture.scan_id.clone(),
                Some(fixture.original_asset_id.clone()),
                "mlkit".to_string(),
                "2026.09".to_string(),
                None,
                Some(250),
                Vec::new(),
                provenance(&fixture.page_id, &fixture.scan_id),
            )
            .unwrap(),
            OcrRunStatus::Unavailable => unreachable!("not needed in these tests"),
        };
        let run_id = run.id().clone();
        let storage = core.lock_storage().unwrap();
        storage.insert_ocr_run(&run).unwrap();
        run_id
    }

    fn region(text: &str, created_at_ms: Option<i64>) -> RecordOcrTextRegionRequest {
        RecordOcrTextRegionRequest {
            polygon: vec![
                CoreOcrTextPoint { x: 0.0, y: 0.0 },
                CoreOcrTextPoint { x: 120.0, y: 0.0 },
                CoreOcrTextPoint { x: 120.0, y: 32.0 },
                CoreOcrTextPoint { x: 0.0, y: 32.0 },
            ],
            text: text.to_string(),
            confidence: Some(0.85),
            created_at_ms,
        }
    }

    #[test]
    fn record_ocr_text_regions_persists_batch_for_detected_run() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_ocr_run(&core, &fixture, OcrRunStatus::Detected);

        let recorded = core
            .record_ocr_text_regions(RecordOcrTextRegionsRequest {
                ocr_run_id: run_id.to_string(),
                regions: vec![
                    region("first line", Some(300)),
                    region("second line", Some(301)),
                ],
            })
            .unwrap();

        assert_eq!(recorded.ocr_run_id, run_id.to_string());
        assert_eq!(recorded.regions.len(), 2);
        let storage = core.lock_storage().unwrap();
        let loaded = storage.list_text_regions_for_ocr_run(&run_id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "first line");
        assert_eq!(loaded[1].text, "second line");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_latest_ocr_output_returns_none_when_scan_has_no_ocr_run() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);

        let loaded = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 10,
            })
            .unwrap();

        assert_eq!(loaded.scan_id, fixture.scan_id.to_string());
        assert!(loaded.latest_run.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_latest_ocr_output_returns_detected_run_with_bounded_regions() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_ocr_run(&core, &fixture, OcrRunStatus::Detected);
        core.record_ocr_text_regions(RecordOcrTextRegionsRequest {
            ocr_run_id: run_id.to_string(),
            regions: vec![region("first line", Some(300)), region("second line", Some(301))],
        })
        .unwrap();

        let loaded = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 1,
            })
            .unwrap();
        let latest = loaded.latest_run.unwrap();

        assert_eq!(latest.ocr_run_id, run_id.to_string());
        assert_eq!(latest.status, OcrRunStatus::Detected);
        assert_eq!(latest.full_text, "first line\nsecond line");
        assert_eq!(latest.text_region_count, 2);
        assert_eq!(latest.text_regions.len(), 1);
        assert_eq!(latest.text_regions[0].text, "first line");
        assert_eq!(latest.text_regions[0].polygon.len(), 4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_latest_ocr_output_keeps_no_text_separate_from_detected_empty_text() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_ocr_run(&core, &fixture, OcrRunStatus::NoTextDetected);

        let loaded = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 10,
            })
            .unwrap();
        let latest = loaded.latest_run.unwrap();

        assert_eq!(latest.ocr_run_id, run_id.to_string());
        assert_eq!(latest.status, OcrRunStatus::NoTextDetected);
        assert_eq!(latest.full_text, "");
        assert_eq!(latest.text_region_count, 0);
        assert!(latest.text_regions.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_latest_ocr_output_rejects_unbounded_region_requests_before_storage() {
        let (core, dir) = open_test_core();
        let err = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: ScanId::generate().to_string(),
                region_limit: MAX_OCR_READBACK_REGION_LIMIT + 1,
            })
            .unwrap_err();

        assert_eq!(
            err.code.to_string(),
            "CORE_OCR_READBACK_REGION_LIMIT_EXCEEDED"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_text_regions_rejects_empty_batch() {
        let (core, dir) = open_test_core();
        let err = core
            .record_ocr_text_regions(RecordOcrTextRegionsRequest {
                ocr_run_id: OcrRunId::generate().to_string(),
                regions: Vec::new(),
            })
            .unwrap_err();
        assert_eq!(err.code.to_string(), "CORE_OCR_TEXT_REGION_BATCH_EMPTY");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_text_regions_rejects_invalid_region_before_storage() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_ocr_run(&core, &fixture, OcrRunStatus::Detected);
        let err = core
            .record_ocr_text_regions(RecordOcrTextRegionsRequest {
                ocr_run_id: run_id.to_string(),
                regions: vec![RecordOcrTextRegionRequest {
                    polygon: vec![CoreOcrTextPoint { x: 0.0, y: 0.0 }],
                    text: "bad polygon".to_string(),
                    confidence: Some(0.5),
                    created_at_ms: Some(300),
                }],
            })
            .unwrap_err();
        assert_eq!(
            err.code.to_string(),
            "TEXT_REGION_POLYGON_POINT_COUNT_INVALID"
        );
        assert_eq!(err.details.get("region_index"), Some(&"0".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_ocr_text_regions_cannot_attach_to_no_text_run() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let run_id = insert_ocr_run(&core, &fixture, OcrRunStatus::NoTextDetected);

        let err = core
            .record_ocr_text_regions(RecordOcrTextRegionsRequest {
                ocr_run_id: run_id.to_string(),
                regions: vec![region("fabricated", Some(300))],
            })
            .unwrap_err();

        assert_eq!(
            err.code.to_string(),
            "STORAGE_TEXT_REGION_OCR_RUN_NOT_DETECTED"
        );
        let storage = core.lock_storage().unwrap();
        assert!(
            storage
                .list_text_regions_for_ocr_run(&run_id)
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
