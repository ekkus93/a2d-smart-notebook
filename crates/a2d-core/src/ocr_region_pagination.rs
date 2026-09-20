use crate::{A2dCore, CoreOcrTextPoint};
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, OcrRunStatus, ScanId};
use a2d_storage::{OcrReadbackRepository, OcrRunRepository, ScanRepository, TextRegionRepository};

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
        let total_count = u32::try_from(storage.count_text_regions_for_ocr_run(&run_id)?).map_err(|_| {
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
