use crate::{A2dCore, CoreOcrTextPoint, OcrJobSnapshot, MAX_OCR_JOB_ATTEMPTS};
use a2d_domain::{
    A2dError, Asset, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, OcrJobId, OcrRun,
    OcrRunId, OcrRunStatus, OcrUnavailableReason, Provenance, Scan, ScanId, TextRegion,
    TextRegionId, system_now_ms,
};
use a2d_storage::{
    AssetRepository, OcrJobRepository, OcrRunRepository, PersistedOcrJob,
    PersistedOcrJobStatus, PersistedOcrProviderAvailability, ScanRepository, TextRegionRepository,
};

const MAX_OCR_LABEL_BYTES: usize = 120;
const MAX_OCR_WARNING_COUNT: usize = 64;
const MAX_OCR_WARNING_TEXT_BYTES: usize = 1_000;
const MAX_FINALIZE_OCR_TEXT_REGION_BATCH_SIZE: usize = 20_000;
const BASE_RETRY_DELAY_MS: i64 = 2_000;
const MAX_RETRY_DELAY_MS: i64 = 30_000;

#[derive(Clone, Debug, PartialEq)]
pub struct FinalizeOcrTextRegionRequest {
    pub polygon: Vec<CoreOcrTextPoint>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinalizeOcrJobRequest {
    pub job_id: String,
    pub attempt_count: u32,
    pub provider: String,
    pub provider_version: String,
    pub model_name: Option<String>,
    pub status: OcrRunStatus,
    pub full_text: String,
    pub unavailable_reason: Option<OcrUnavailableReason>,
    pub unavailable_message: Option<String>,
    pub completed_at_ms: Option<i64>,
    pub warnings: Vec<String>,
    pub retryable: bool,
    pub regions: Vec<FinalizeOcrTextRegionRequest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CoreOcrFinalizationResolution {
    Completed,
    RetryScheduled,
    TerminalUnavailable,
    CancelledBeforeCommit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinalizedOcrJob {
    pub job: OcrJobSnapshot,
    pub ocr_run_id: Option<String>,
    pub recorded_region_count: u32,
    pub resolution: CoreOcrFinalizationResolution,
}

impl A2dCore {
    /// Atomically commits one claimed OCR job's terminal provider outcome.
    ///
    /// This is the production commit point for OCR queue workers. For accepted detected output,
    /// the OCR run, required text regions, FTS trigger effects, and queue success transition are
    /// one SQLite transaction. If any required write fails, none of those records become visible.
    /// A durable cancellation request observed before this transaction commits wins and prevents
    /// detected text from becoming accepted/searchable OCR.
    pub fn finalize_ocr_job(
        &self,
        request: FinalizeOcrJobRequest,
    ) -> Result<FinalizedOcrJob, A2dError> {
        validate_finalize_request(&request)?;
        let job_id = OcrJobId::parse(&request.job_id)?;
        let now_ms = system_now_ms()?;
        let completed_at_ms = request.completed_at_ms.unwrap_or(now_ms);
        let mut storage = self.lock_storage()?;

        storage.transaction(|tx| {
            let mut job = tx
                .get_ocr_job(&job_id)?
                .ok_or_else(|| missing_job_error(&job_id))?;
            validate_running_claim(&job, request.attempt_count)?;

            if job.cancellation_requested
                || request.unavailable_reason == Some(OcrUnavailableReason::Cancelled)
            {
                job.updated_at_ms = now_ms;
                job.status = PersistedOcrJobStatus::Cancelled;
                job.completed_at_ms = Some(now_ms);
                job.retryable = false;
                job.next_retry_at_ms = None;
                job.provider = Some(request.provider.clone());
                job.provider_version = Some(request.provider_version.clone());
                job.model_name = request.model_name.clone();
                job.provider_availability = PersistedOcrProviderAvailability::Available;
                job.last_error_code = Some("OCR_CANCELLED".to_string());
                job.last_error_message = request
                    .unavailable_message
                    .clone()
                    .or_else(|| Some("OCR execution was cancelled before commit".to_string()));
                tx.update_ocr_job(&job)?;
                return Ok(FinalizedOcrJob {
                    job: job.into(),
                    ocr_run_id: None,
                    recorded_region_count: 0,
                    resolution: CoreOcrFinalizationResolution::CancelledBeforeCommit,
                });
            }

            let scan = validate_job_input(tx, &job)?;
            let run_id = OcrRunId::try_generate()?;
            let provenance = Provenance {
                source_page_id: Some(scan.page_id),
                source_scan_id: Some(job.scan_id.clone()),
                producing_component: request.provider.clone(),
                component_version: request.provider_version.clone(),
                created_at_ms: completed_at_ms,
                warnings: request.warnings.clone(),
                user_approved: None,
            };
            let run = OcrRun::from_stored(
                run_id.clone(),
                job.scan_id.clone(),
                Some(job.input_asset_id.clone()),
                request.provider.clone(),
                request.provider_version.clone(),
                request.model_name.clone(),
                request.status,
                request.full_text.clone(),
                request.unavailable_reason,
                request.unavailable_message.clone(),
                Some(completed_at_ms),
                request.warnings.clone(),
                provenance,
            )?;
            let regions = build_text_regions(&run_id, &request.regions, completed_at_ms)?;

            tx.insert_ocr_run(&run)?;
            for region in &regions {
                tx.insert_text_region(region)?;
            }

            apply_terminal_queue_transition(&mut job, &run, request.retryable, now_ms)?;
            tx.update_ocr_job(&job)?;

            let resolution = match job.status {
                PersistedOcrJobStatus::Recognized => CoreOcrFinalizationResolution::Completed,
                PersistedOcrJobStatus::Queued => CoreOcrFinalizationResolution::RetryScheduled,
                PersistedOcrJobStatus::Unavailable => {
                    CoreOcrFinalizationResolution::TerminalUnavailable
                }
                PersistedOcrJobStatus::Cancelled => {
                    CoreOcrFinalizationResolution::CancelledBeforeCommit
                }
                PersistedOcrJobStatus::Running => {
                    return Err(finalize_error(
                        "CORE_OCR_FINALIZE_STATE_INVALID",
                        "OCR finalization left a job running after a terminal provider outcome",
                        false,
                    ));
                }
            };
            Ok(FinalizedOcrJob {
                job: job.into(),
                ocr_run_id: Some(run_id.to_string()),
                recorded_region_count: regions.len() as u32,
                resolution,
            })
        })
    }
}

fn validate_finalize_request(request: &FinalizeOcrJobRequest) -> Result<(), A2dError> {
    validate_label(&request.provider, "provider")?;
    validate_label(&request.provider_version, "provider_version")?;
    if let Some(model_name) = &request.model_name {
        validate_label(model_name, "model_name")?;
    }
    if let Some(completed_at_ms) = request.completed_at_ms {
        validate_non_negative_timestamp(completed_at_ms, "completed_at_ms")?;
    }
    if let Some(message) = &request.unavailable_message
        && (message.is_empty() || message.len() > MAX_OCR_WARNING_TEXT_BYTES)
    {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_UNAVAILABLE_MESSAGE_INVALID",
            "OCR unavailable message must be non-empty and bounded when present",
            false,
        )
        .with_detail("max_bytes", MAX_OCR_WARNING_TEXT_BYTES.to_string()));
    }
    if request.warnings.len() > MAX_OCR_WARNING_COUNT {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_WARNING_COUNT_EXCEEDS_LIMIT",
            "OCR warning count exceeds the configured OCR limit",
            false,
        )
        .with_detail("warning_count", request.warnings.len().to_string())
        .with_detail("max_warning_count", MAX_OCR_WARNING_COUNT.to_string()));
    }
    for warning in &request.warnings {
        if warning.is_empty() || warning.len() > MAX_OCR_WARNING_TEXT_BYTES {
            return Err(finalize_error(
                "CORE_OCR_FINALIZE_WARNING_INVALID",
                "OCR warnings must be non-empty and bounded",
                false,
            )
            .with_detail("max_bytes", MAX_OCR_WARNING_TEXT_BYTES.to_string()));
        }
    }
    if request.regions.len() > MAX_FINALIZE_OCR_TEXT_REGION_BATCH_SIZE {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_TEXT_REGION_BATCH_EXCEEDS_LIMIT",
            "OCR finalization region batch exceeds the configured limit",
            false,
        )
        .with_detail("region_count", request.regions.len().to_string())
        .with_detail(
            "max_region_count",
            MAX_FINALIZE_OCR_TEXT_REGION_BATCH_SIZE.to_string(),
        ));
    }
    if request.status != OcrRunStatus::Detected && !request.regions.is_empty() {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_REGIONS_FOR_NON_DETECTED_RESULT",
            "OCR text regions can only be finalized with detected OCR text",
            false,
        ));
    }
    Ok(())
}

fn validate_label(value: &str, field: &'static str) -> Result<(), A2dError> {
    if value.is_empty() || value.len() > MAX_OCR_LABEL_BYTES {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_LABEL_INVALID",
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
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_TIMESTAMP_INVALID",
            "OCR timestamps must be non-negative milliseconds",
            false,
        )
        .with_detail("field", field));
    }
    Ok(())
}

fn validate_running_claim(job: &PersistedOcrJob, attempt_count: u32) -> Result<(), A2dError> {
    if job.status != PersistedOcrJobStatus::Running {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_JOB_NOT_RUNNING",
            "only a running OCR job can accept a provider finalization result",
            false,
        )
        .with_detail("job_id", job.id.to_string())
        .with_detail("status", format!("{:?}", job.status)));
    }
    if job.attempt_count != attempt_count {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_STALE_ATTEMPT",
            "OCR finalization attempt does not match the current durable job attempt",
            false,
        )
        .with_detail("job_id", job.id.to_string())
        .with_detail("expected_attempt_count", job.attempt_count.to_string())
        .with_detail("actual_attempt_count", attempt_count.to_string()));
    }
    Ok(())
}

fn validate_job_input(conn: &rusqlite::Connection, job: &PersistedOcrJob) -> Result<Scan, A2dError> {
    let scan = conn.get_scan(&job.scan_id)?.ok_or_else(|| {
        finalize_error(
            "CORE_OCR_FINALIZE_SCAN_MISSING",
            "OCR finalization requires the queued scan to still exist",
            false,
        )
        .with_detail("scan_id", job.scan_id.to_string())
    })?;
    let expected_asset_kind = expected_scan_asset_kind(&scan, &job.input_asset_id)?;
    let asset = conn.get_asset(&job.input_asset_id)?.ok_or_else(|| {
        finalize_error(
            "CORE_OCR_FINALIZE_INPUT_ASSET_MISSING_ROW",
            "OCR finalization requires the queued input asset row to still exist",
            false,
        )
        .with_detail("scan_id", job.scan_id.to_string())
        .with_detail("input_asset_id", job.input_asset_id.to_string())
    })?;
    validate_record_input_asset(&asset, expected_asset_kind)?;
    Ok(scan)
}

fn expected_scan_asset_kind(
    scan: &Scan,
    input_asset_id: &a2d_domain::AssetId,
) -> Result<AssetKind, A2dError> {
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
    Err(finalize_error(
        "CORE_OCR_FINALIZE_INPUT_ASSET_NOT_OWNED_BY_SCAN",
        "OCR finalization input asset must be one of the scan's persisted OCR-readable assets",
        false,
    )
    .with_detail("scan_id", scan.id().to_string())
    .with_detail("input_asset_id", input_asset_id.to_string()))
}

fn validate_record_input_asset(asset: &Asset, expected: AssetKind) -> Result<(), A2dError> {
    if asset.kind != expected {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_INPUT_ASSET_KIND_MISMATCH",
            "OCR finalization input asset kind does not match the scan-owned asset slot",
            false,
        )
        .with_detail("expected_asset_kind", asset_kind_label(expected))
        .with_detail("actual_asset_kind", asset_kind_label(asset.kind))
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    if !asset.immutable {
        return Err(finalize_error(
            "CORE_OCR_FINALIZE_INPUT_ASSET_NOT_IMMUTABLE",
            "OCR finalization input asset must be immutable",
            false,
        )
        .with_detail("input_asset_id", asset.id().to_string()));
    }
    Ok(())
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

fn build_text_regions(
    run_id: &OcrRunId,
    regions: &[FinalizeOcrTextRegionRequest],
    fallback_created_at_ms: i64,
) -> Result<Vec<TextRegion>, A2dError> {
    let mut typed_regions = Vec::with_capacity(regions.len());
    for (index, region) in regions.iter().enumerate() {
        let created_at_ms = region.created_at_ms.unwrap_or(fallback_created_at_ms);
        if created_at_ms < 0 {
            return Err(finalize_error(
                "CORE_OCR_FINALIZE_TEXT_REGION_TIMESTAMP_INVALID",
                "OCR text-region timestamps must be non-negative milliseconds",
                false,
            )
            .with_detail("region_index", index.to_string()));
        }
        let polygon = region
            .polygon
            .iter()
            .map(|point| (point.x, point.y))
            .collect::<Vec<_>>();
        typed_regions.push(
            TextRegion::new(
                TextRegionId::try_generate()?,
                run_id.clone(),
                polygon,
                region.text.clone(),
                region.confidence,
                created_at_ms,
            )
            .map_err(|error| error.with_detail("region_index", index.to_string()))?,
        );
    }
    Ok(typed_regions)
}

fn apply_terminal_queue_transition(
    job: &mut PersistedOcrJob,
    run: &OcrRun,
    request_retryable: bool,
    now_ms: i64,
) -> Result<(), A2dError> {
    job.updated_at_ms = now_ms;
    job.provider = Some(run.provider.clone());
    job.provider_version = Some(run.provider_version.clone());
    job.model_name = run.model_name.clone();
    job.last_ocr_run_id = Some(run.id().clone());

    match run.status {
        OcrRunStatus::Detected | OcrRunStatus::NoTextDetected => {
            job.status = PersistedOcrJobStatus::Recognized;
            job.completed_at_ms = Some(now_ms);
            job.retryable = false;
            job.next_retry_at_ms = None;
            job.provider_availability = PersistedOcrProviderAvailability::Available;
            job.last_error_code = None;
            job.last_error_message = None;
        }
        OcrRunStatus::Unavailable => {
            let reason = run.unavailable_reason.ok_or_else(|| {
                finalize_error(
                    "CORE_OCR_FINALIZE_UNAVAILABLE_REASON_MISSING",
                    "persisted unavailable OCR run is missing its reason",
                    false,
                )
            })?;
            job.provider_availability = provider_availability_for(reason);
            job.last_error_code = Some(unavailable_error_code(reason).to_string());
            job.last_error_message = run.unavailable_message.clone().or_else(|| {
                Some("OCR provider did not produce a terminal text result".to_string())
            });
            let retryable = request_retryable
                && retryable_reason(reason)
                && job.attempt_count < MAX_OCR_JOB_ATTEMPTS;
            if retryable {
                job.status = PersistedOcrJobStatus::Queued;
                job.last_started_at_ms = None;
                job.completed_at_ms = None;
                job.retryable = true;
                job.next_retry_at_ms = Some(now_ms.saturating_add(retry_delay_ms(job.attempt_count)));
            } else {
                job.status = PersistedOcrJobStatus::Unavailable;
                job.completed_at_ms = Some(now_ms);
                job.retryable = false;
                job.next_retry_at_ms = None;
            }
        }
    }
    Ok(())
}

fn retry_delay_ms(attempt_count: u32) -> i64 {
    let shift = attempt_count.saturating_sub(1).min(4);
    BASE_RETRY_DELAY_MS
        .saturating_mul(1_i64 << shift)
        .min(MAX_RETRY_DELAY_MS)
}

fn retryable_reason(reason: OcrUnavailableReason) -> bool {
    matches!(
        reason,
        OcrUnavailableReason::ProviderUnavailable
            | OcrUnavailableReason::ProviderFailed
            | OcrUnavailableReason::ResourceUnavailable
    )
}

fn provider_availability_for(reason: OcrUnavailableReason) -> PersistedOcrProviderAvailability {
    match reason {
        OcrUnavailableReason::ProviderFailed => PersistedOcrProviderAvailability::Failed,
        OcrUnavailableReason::ProviderUnavailable | OcrUnavailableReason::ResourceUnavailable => {
            PersistedOcrProviderAvailability::Unavailable
        }
        OcrUnavailableReason::UnsupportedInput | OcrUnavailableReason::Cancelled => {
            PersistedOcrProviderAvailability::Available
        }
    }
}

fn unavailable_error_code(reason: OcrUnavailableReason) -> &'static str {
    match reason {
        OcrUnavailableReason::ProviderUnavailable => "OCR_PROVIDER_UNAVAILABLE",
        OcrUnavailableReason::ProviderFailed => "OCR_PROVIDER_FAILED",
        OcrUnavailableReason::ResourceUnavailable => "OCR_RESOURCE_UNAVAILABLE",
        OcrUnavailableReason::UnsupportedInput => "OCR_UNSUPPORTED_INPUT",
        OcrUnavailableReason::Cancelled => "OCR_CANCELLED",
    }
}

fn missing_job_error(job_id: &OcrJobId) -> A2dError {
    finalize_error(
        "CORE_OCR_JOB_MISSING",
        "OCR finalization requires an existing persisted job",
        false,
    )
    .with_detail("job_id", job_id.to_string())
}

fn finalize_error(code: &'static str, message: impl Into<String>, retryable: bool) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.finalize",
        message,
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CoreOcrInputKind, EnqueueOcrJobRequest, LoadLatestOcrOutputRequest, OpenLibraryRequest,
        SearchOcrTextRequest,
    };
    use a2d_domain::{
        Asset, AssetId, CaptureSource, EncryptionState, LayoutId, Page, PageId, PageKind,
        PageState, QualityStatus, SmartPageId,
    };
    use a2d_storage::{AssetRepository, PageRepository, ScanRepository};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct ScanFixture {
        scan_id: ScanId,
        original_asset_id: AssetId,
    }

    fn open_test_core() -> (Arc<A2dCore>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "a2d-core-ocr-finalize-test-{}",
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
            Some("OCR finalization core page".to_string()),
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
            original_asset_id,
        }
    }

    fn claim_job(core: &A2dCore, fixture: &ScanFixture) -> OcrJobSnapshot {
        core.enqueue_ocr_job(EnqueueOcrJobRequest {
            scan_id: fixture.scan_id.to_string(),
            input_kind: CoreOcrInputKind::Original,
            width_px: 1_000,
            height_px: 1_400,
        })
        .unwrap();
        core.claim_next_ocr_job().unwrap().unwrap()
    }

    fn region(text: &str) -> FinalizeOcrTextRegionRequest {
        FinalizeOcrTextRegionRequest {
            polygon: vec![
                CoreOcrTextPoint { x: 0.0, y: 0.0 },
                CoreOcrTextPoint { x: 120.0, y: 0.0 },
                CoreOcrTextPoint { x: 120.0, y: 32.0 },
                CoreOcrTextPoint { x: 0.0, y: 32.0 },
            ],
            text: text.to_string(),
            confidence: Some(0.85),
            created_at_ms: Some(300),
        }
    }

    fn detected_request(job: &OcrJobSnapshot) -> FinalizeOcrJobRequest {
        FinalizeOcrJobRequest {
            job_id: job.job_id.clone(),
            attempt_count: job.attempt_count,
            provider: "mlkit".to_string(),
            provider_version: "2026.09".to_string(),
            model_name: Some("latin-v1".to_string()),
            status: OcrRunStatus::Detected,
            full_text: "transactional rollback text".to_string(),
            unavailable_reason: None,
            unavailable_message: None,
            completed_at_ms: Some(250),
            warnings: Vec::new(),
            retryable: false,
            regions: vec![region("transactional rollback text")],
        }
    }

    #[test]
    fn finalize_detected_ocr_commits_run_regions_search_and_queue_together() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let claimed = claim_job(&core, &fixture);

        let finalized = core.finalize_ocr_job(detected_request(&claimed)).unwrap();

        assert_eq!(finalized.resolution, CoreOcrFinalizationResolution::Completed);
        assert_eq!(finalized.job.status, crate::CoreOcrJobStatus::Recognized);
        assert_eq!(finalized.recorded_region_count, 1);
        assert!(finalized.ocr_run_id.is_some());

        let output = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 10,
            })
            .unwrap();
        let latest = output.latest_run.unwrap();
        assert_eq!(latest.status, OcrRunStatus::Detected);
        assert_eq!(latest.text_regions.len(), 1);
        assert_eq!(latest.text_regions[0].text, "transactional rollback text");

        let search = core
            .search_ocr_text(SearchOcrTextRequest {
                query: "transactional".to_string(),
                limit: 10,
            })
            .unwrap();
        assert!(!search.hits.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_detected_ocr_rolls_back_run_search_and_queue_when_region_insert_fails() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let claimed = claim_job(&core, &fixture);
        {
            let mut storage = core.lock_storage().unwrap();
            storage
                .transaction(|tx| {
                    tx.execute_batch(
                        "CREATE TRIGGER fail_text_region_insert_for_test \
                         BEFORE INSERT ON text_regions \
                         BEGIN \
                           SELECT RAISE(ABORT, 'forced text-region failure'); \
                         END;",
                    )
                    .expect("failure-injection trigger must be created");
                    Ok(())
                })
                .unwrap();
        }

        let err = core.finalize_ocr_job(detected_request(&claimed)).unwrap_err();
        assert!(
            err.developer_message.contains("forced text-region failure")
                || err.code.to_string().contains("STORAGE")
        );

        let output = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 10,
            })
            .unwrap();
        assert!(output.latest_run.is_none());
        let job = core.get_ocr_job(&claimed.job_id).unwrap();
        assert_eq!(job.status, crate::CoreOcrJobStatus::Running);
        assert!(job.last_ocr_run_id.is_none());
        let search = core
            .search_ocr_text(SearchOcrTextRequest {
                query: "transactional".to_string(),
                limit: 10,
            })
            .unwrap();
        assert!(search.hits.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_rejects_stale_attempt_without_mutating_running_job() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let claimed = claim_job(&core, &fixture);
        let mut request = detected_request(&claimed);
        request.attempt_count = claimed.attempt_count + 1;

        let err = core.finalize_ocr_job(request).unwrap_err();

        assert_eq!(err.code.to_string(), "CORE_OCR_FINALIZE_STALE_ATTEMPT");
        let job = core.get_ocr_job(&claimed.job_id).unwrap();
        assert_eq!(job.status, crate::CoreOcrJobStatus::Running);
        assert!(job.last_ocr_run_id.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_honors_cancellation_requested_before_commit() {
        let (core, dir) = open_test_core();
        let fixture = insert_scan_fixture(&core);
        let claimed = claim_job(&core, &fixture);
        core.request_ocr_job_cancellation(&claimed.job_id).unwrap();

        let finalized = core.finalize_ocr_job(detected_request(&claimed)).unwrap();

        assert_eq!(
            finalized.resolution,
            CoreOcrFinalizationResolution::CancelledBeforeCommit
        );
        assert_eq!(finalized.job.status, crate::CoreOcrJobStatus::Cancelled);
        assert!(finalized.ocr_run_id.is_none());
        let output = core
            .load_latest_ocr_output(LoadLatestOcrOutputRequest {
                scan_id: fixture.scan_id.to_string(),
                region_limit: 10,
            })
            .unwrap();
        assert!(output.latest_run.is_none());
        let search = core
            .search_ocr_text(SearchOcrTextRequest {
                query: "transactional".to_string(),
                limit: 10,
            })
            .unwrap();
        assert!(search.hits.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
