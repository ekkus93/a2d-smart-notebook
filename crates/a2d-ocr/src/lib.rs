//! Provider-adapter and queue-status contracts for OCR.
//!
//! Rust owns the normalized OCR request/result shape. Platform adapters such as Android ML Kit
//! may recognize text, but they must report bounded, explicit outcomes through this contract:
//! successful recognition, successful no-text detection, OCR unavailability/failure, or
//! cancellation. A failed OCR run must never be converted into a fabricated empty successful
//! transcription.

use std::collections::BTreeMap;

const MAX_REF_BYTES: usize = 200;
const MAX_LANGUAGE_TAG_BYTES: usize = 35;
const MAX_LABEL_BYTES: usize = 120;
const MAX_WARNING_CODE_BYTES: usize = 80;
const MIN_POLYGON_POINTS: usize = 3;
const MAX_POLYGON_POINTS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrContractError {
    pub code: String,
    pub developer_message: String,
    pub retryable: bool,
    pub details: BTreeMap<String, String>,
}

impl OcrContractError {
    fn new(code: &'static str, developer_message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.to_string(),
            developer_message: developer_message.into(),
            retryable,
            details: BTreeMap::new(),
        }
    }

    #[must_use]
    fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OcrScanRef(String);

impl OcrScanRef {
    pub fn new(value: impl Into<String>) -> Result<Self, OcrContractError> {
        let value = value.into();
        validate_reference(&value, "scan_ref")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OcrAssetRef(String);

impl OcrAssetRef {
    pub fn new(value: impl Into<String>) -> Result<Self, OcrContractError> {
        let value = value.into();
        validate_reference(&value, "asset_ref")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OcrAdapterOutcome<T> {
    Completed(T),
    Cancelled,
    Failed(OcrContractError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrLimits {
    pub max_image_dimension_px: u32,
    pub max_image_pixels: u64,
    pub max_full_text_bytes: usize,
    pub max_regions: usize,
    pub max_region_text_bytes: usize,
    pub max_languages_per_region: usize,
    pub max_warning_count: usize,
    pub max_warning_text_bytes: usize,
}

impl Default for OcrLimits {
    fn default() -> Self {
        Self {
            max_image_dimension_px: 12_000,
            max_image_pixels: 80_000_000,
            max_full_text_bytes: 2_000_000,
            max_regions: 20_000,
            max_region_text_bytes: 20_000,
            max_languages_per_region: 8,
            max_warning_count: 64,
            max_warning_text_bytes: 1_000,
        }
    }
}

impl OcrLimits {
    fn validate(&self) -> Result<(), OcrContractError> {
        if self.max_image_dimension_px == 0
            || self.max_image_pixels == 0
            || self.max_full_text_bytes == 0
            || self.max_regions == 0
            || self.max_region_text_bytes == 0
            || self.max_languages_per_region == 0
            || self.max_warning_text_bytes == 0
        {
            return Err(ocr_contract_error(
                "OCR_LIMIT_INVALID",
                "OCR limits must be non-zero",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrQueueLimits {
    pub max_attempt_count: u32,
    pub max_last_error_bytes: usize,
}

impl Default for OcrQueueLimits {
    fn default() -> Self {
        Self {
            max_attempt_count: 25,
            max_last_error_bytes: 1_000,
        }
    }
}

impl OcrQueueLimits {
    fn validate(&self) -> Result<(), OcrContractError> {
        if self.max_attempt_count == 0 || self.max_last_error_bytes == 0 {
            return Err(ocr_contract_error(
                "OCR_QUEUE_LIMIT_INVALID",
                "OCR queue limits must be non-zero",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrInputKind {
    Original,
    Corrected,
    OcrOptimized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrImageSource {
    pub scan_ref: OcrScanRef,
    pub input_asset_ref: OcrAssetRef,
    pub input_kind: OcrInputKind,
    pub width_px: u32,
    pub height_px: u32,
}

impl OcrImageSource {
    fn validate(&self, limits: &OcrLimits) -> Result<(), OcrContractError> {
        if self.width_px == 0 || self.height_px == 0 {
            return Err(ocr_contract_error(
                "OCR_IMAGE_DIMENSIONS_INVALID",
                "OCR image dimensions must be non-zero",
                false,
            ));
        }
        if self.width_px > limits.max_image_dimension_px
            || self.height_px > limits.max_image_dimension_px
        {
            return Err(ocr_contract_error(
                "OCR_IMAGE_DIMENSIONS_EXCEED_LIMIT",
                "OCR image dimensions exceed the configured OCR limit",
                false,
            )
            .with_detail("width_px", self.width_px.to_string())
            .with_detail("height_px", self.height_px.to_string())
            .with_detail(
                "max_image_dimension_px",
                limits.max_image_dimension_px.to_string(),
            ));
        }
        let pixels = u64::from(self.width_px) * u64::from(self.height_px);
        if pixels > limits.max_image_pixels {
            return Err(ocr_contract_error(
                "OCR_IMAGE_PIXELS_EXCEED_LIMIT",
                "OCR image pixel count exceeds the configured OCR limit",
                false,
            )
            .with_detail("pixels", pixels.to_string())
            .with_detail("max_image_pixels", limits.max_image_pixels.to_string()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrRequest {
    pub source: OcrImageSource,
    pub provider_hint: Option<String>,
    pub language_hints: Vec<String>,
    pub limits: OcrLimits,
}

impl OcrRequest {
    pub fn new(source: OcrImageSource) -> Self {
        Self {
            source,
            provider_hint: None,
            language_hints: Vec::new(),
            limits: OcrLimits::default(),
        }
    }

    pub fn validate(&self) -> Result<(), OcrContractError> {
        self.limits.validate()?;
        self.source.validate(&self.limits)?;
        if let Some(provider_hint) = &self.provider_hint {
            validate_bounded_label(provider_hint, "provider_hint")?;
        }
        for language in &self.language_hints {
            validate_language_tag(language)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OcrWorkKey {
    pub scan_ref: OcrScanRef,
    pub input_asset_ref: OcrAssetRef,
    pub input_kind: OcrInputKind,
}

impl OcrWorkKey {
    pub fn from_request(request: &OcrRequest) -> Result<Self, OcrContractError> {
        request.validate()?;
        Ok(Self {
            scan_ref: request.source.scan_ref.clone(),
            input_asset_ref: request.source.input_asset_ref.clone(),
            input_kind: request.source.input_kind,
        })
    }

    fn matches_result(&self, result: &OcrResult) -> bool {
        self.scan_ref == result.scan_ref
            && self.input_asset_ref == result.input_asset_ref
            && self.input_kind == result.input_kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrJobStatus {
    Queued,
    Running,
    Recognized,
    Unavailable,
    Cancelled,
}

impl OcrJobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            OcrJobStatus::Recognized | OcrJobStatus::Unavailable | OcrJobStatus::Cancelled
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrRetryState {
    pub attempt_count: u32,
    pub retryable: bool,
    pub next_retry_at_ms: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
}

impl OcrRetryState {
    fn validate(&self, limits: &OcrQueueLimits) -> Result<(), OcrContractError> {
        limits.validate()?;
        if self.attempt_count > limits.max_attempt_count {
            return Err(ocr_contract_error(
                "OCR_ATTEMPT_COUNT_EXCEEDS_LIMIT",
                "OCR attempt count exceeds the configured OCR queue limit",
                false,
            )
            .with_detail("attempt_count", self.attempt_count.to_string())
            .with_detail("max_attempt_count", limits.max_attempt_count.to_string()));
        }
        if let Some(next_retry_at_ms) = self.next_retry_at_ms
            && next_retry_at_ms < 0
        {
            return Err(ocr_contract_error(
                "OCR_RETRY_TIMESTAMP_INVALID",
                "OCR retry timestamp must be non-negative milliseconds",
                false,
            ));
        }
        if let Some(code) = &self.last_error_code {
            validate_warning_code(code)?;
        }
        if let Some(message) = &self.last_error_message
            && (message.is_empty() || message.len() > limits.max_last_error_bytes)
        {
            return Err(ocr_contract_error(
                "OCR_LAST_ERROR_MESSAGE_INVALID",
                "OCR last error message must be non-empty and bounded when present",
                false,
            )
            .with_detail("max_bytes", limits.max_last_error_bytes.to_string()));
        }
        if !self.retryable && self.next_retry_at_ms.is_some() {
            return Err(ocr_contract_error(
                "OCR_RETRY_STATE_CONFLICT",
                "non-retryable OCR jobs must not carry a next retry timestamp",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrJobRecord {
    pub work_key: OcrWorkKey,
    pub status: OcrJobStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub last_started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub retry_state: OcrRetryState,
    pub result: Option<OcrResult>,
}

impl OcrJobRecord {
    pub fn queued(request: &OcrRequest, now_ms: i64) -> Result<Self, OcrContractError> {
        validate_timestamp(now_ms, "now_ms")?;
        Ok(Self {
            work_key: OcrWorkKey::from_request(request)?,
            status: OcrJobStatus::Queued,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            last_started_at_ms: None,
            completed_at_ms: None,
            retry_state: OcrRetryState {
                attempt_count: 0,
                retryable: true,
                next_retry_at_ms: None,
                last_error_code: None,
                last_error_message: None,
            },
            result: None,
        })
    }

    pub fn mark_running(&self, now_ms: i64) -> Result<Self, OcrContractError> {
        validate_timestamp(now_ms, "now_ms")?;
        self.validate(&OcrQueueLimits::default())?;
        if self.status.is_terminal() {
            return Err(ocr_contract_error(
                "OCR_JOB_TERMINAL_TRANSITION_INVALID",
                "terminal OCR jobs cannot be marked running",
                false,
            ));
        }
        if now_ms < self.created_at_ms {
            return Err(ocr_contract_error(
                "OCR_JOB_TIMESTAMP_ORDER_INVALID",
                "OCR job cannot start before it was created",
                false,
            ));
        }
        let mut next = self.clone();
        next.status = OcrJobStatus::Running;
        next.updated_at_ms = now_ms;
        next.last_started_at_ms = Some(now_ms);
        next.retry_state.attempt_count =
            next.retry_state
                .attempt_count
                .checked_add(1)
                .ok_or_else(|| {
                    ocr_contract_error(
                        "OCR_ATTEMPT_COUNT_OVERFLOW",
                        "OCR attempt count overflowed",
                        false,
                    )
                })?;
        next.validate(&OcrQueueLimits::default())?;
        Ok(next)
    }

    pub fn complete_with_result(
        &self,
        result: OcrResult,
        now_ms: i64,
    ) -> Result<Self, OcrContractError> {
        validate_timestamp(now_ms, "now_ms")?;
        self.validate(&OcrQueueLimits::default())?;
        if !self.work_key.matches_result(&result) {
            return Err(ocr_contract_error(
                "OCR_RESULT_WORK_KEY_MISMATCH",
                "OCR result must match the queued scan, asset, and input kind",
                false,
            ));
        }
        if now_ms < self.created_at_ms {
            return Err(ocr_contract_error(
                "OCR_JOB_TIMESTAMP_ORDER_INVALID",
                "OCR job cannot complete before it was created",
                false,
            ));
        }
        let mut next = self.clone();
        next.status = match &result.body {
            OcrAdapterOutput::Recognized(_) => OcrJobStatus::Recognized,
            OcrAdapterOutput::Unavailable(_) => OcrJobStatus::Unavailable,
        };
        next.updated_at_ms = now_ms;
        next.completed_at_ms = Some(now_ms);
        next.retry_state.retryable = matches!(
            &result.body,
            OcrAdapterOutput::Unavailable(unavailable) if unavailable.retryable
        );
        next.retry_state.next_retry_at_ms = None;
        next.retry_state.last_error_code = match &result.body {
            OcrAdapterOutput::Recognized(_) => None,
            OcrAdapterOutput::Unavailable(unavailable) => {
                Some(format!("OCR_{:?}", unavailable.reason).to_uppercase())
            }
        };
        next.retry_state.last_error_message = match &result.body {
            OcrAdapterOutput::Recognized(_) => None,
            OcrAdapterOutput::Unavailable(unavailable) => Some(unavailable.developer_message.clone()),
        };
        next.result = Some(result);
        next.validate(&OcrQueueLimits::default())?;
        Ok(next)
    }

    pub fn cancel(&self, now_ms: i64) -> Result<Self, OcrContractError> {
        validate_timestamp(now_ms, "now_ms")?;
        self.validate(&OcrQueueLimits::default())?;
        if self.status.is_terminal() {
            return Err(ocr_contract_error(
                "OCR_JOB_TERMINAL_TRANSITION_INVALID",
                "terminal OCR jobs cannot be cancelled again",
                false,
            ));
        }
        let mut next = self.clone();
        next.status = OcrJobStatus::Cancelled;
        next.updated_at_ms = now_ms;
        next.completed_at_ms = Some(now_ms);
        next.retry_state.retryable = false;
        next.retry_state.next_retry_at_ms = None;
        next.result = None;
        next.validate(&OcrQueueLimits::default())?;
        Ok(next)
    }

    pub fn validate(&self, limits: &OcrQueueLimits) -> Result<(), OcrContractError> {
        limits.validate()?;
        validate_timestamp(self.created_at_ms, "created_at_ms")?;
        validate_timestamp(self.updated_at_ms, "updated_at_ms")?;
        if self.updated_at_ms < self.created_at_ms {
            return Err(ocr_contract_error(
                "OCR_JOB_TIMESTAMP_ORDER_INVALID",
                "OCR job updated timestamp cannot precede creation",
                false,
            ));
        }
        if let Some(started_at) = self.last_started_at_ms {
            validate_timestamp(started_at, "last_started_at_ms")?;
            if started_at < self.created_at_ms {
                return Err(ocr_contract_error(
                    "OCR_JOB_TIMESTAMP_ORDER_INVALID",
                    "OCR job start timestamp cannot precede creation",
                    false,
                ));
            }
        }
        if let Some(completed_at) = self.completed_at_ms {
            validate_timestamp(completed_at, "completed_at_ms")?;
            if completed_at < self.created_at_ms {
                return Err(ocr_contract_error(
                    "OCR_JOB_TIMESTAMP_ORDER_INVALID",
                    "OCR job completion timestamp cannot precede creation",
                    false,
                ));
            }
        }
        self.retry_state.validate(limits)?;
        match self.status {
            OcrJobStatus::Queued => {
                if self.last_started_at_ms.is_some()
                    || self.completed_at_ms.is_some()
                    || self.result.is_some()
                {
                    return Err(ocr_contract_error(
                        "OCR_JOB_QUEUED_STATE_INVALID",
                        "queued OCR jobs must not carry started/completed/result fields",
                        false,
                    ));
                }
            }
            OcrJobStatus::Running => {
                if self.last_started_at_ms.is_none()
                    || self.completed_at_ms.is_some()
                    || self.result.is_some()
                {
                    return Err(ocr_contract_error(
                        "OCR_JOB_RUNNING_STATE_INVALID",
                        "running OCR jobs require a start timestamp and no terminal result",
                        false,
                    ));
                }
            }
            OcrJobStatus::Recognized => match &self.result {
                Some(result) if result.is_recognized() && self.work_key.matches_result(result) => {}
                _ => {
                    return Err(ocr_contract_error(
                        "OCR_JOB_RECOGNIZED_STATE_INVALID",
                        "recognized OCR jobs must carry a matching recognized result",
                        false,
                    ));
                }
            },
            OcrJobStatus::Unavailable => match &self.result {
                Some(result) if result.is_unavailable() && self.work_key.matches_result(result) => {
                }
                _ => {
                    return Err(ocr_contract_error(
                        "OCR_JOB_UNAVAILABLE_STATE_INVALID",
                        "unavailable OCR jobs must carry a matching unavailable result",
                        false,
                    ));
                }
            },
            OcrJobStatus::Cancelled => {
                if self.completed_at_ms.is_none()
                    || self.result.is_some()
                    || self.retry_state.retryable
                {
                    return Err(ocr_contract_error(
                        "OCR_JOB_CANCELLED_STATE_INVALID",
                        "cancelled OCR jobs must be terminal, non-retryable, and result-free",
                        false,
                    ));
                }
            }
        }
        if self.status.is_terminal() && self.completed_at_ms.is_none() {
            return Err(ocr_contract_error(
                "OCR_JOB_TERMINAL_TIMESTAMP_MISSING",
                "terminal OCR jobs must carry a completion timestamp",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrProviderInfo {
    pub provider: String,
    pub provider_version: String,
    pub model_version: Option<String>,
}

impl OcrProviderInfo {
    fn validate(&self) -> Result<(), OcrContractError> {
        validate_bounded_label(&self.provider, "provider")?;
        validate_bounded_label(&self.provider_version, "provider_version")?;
        if let Some(model_version) = &self.model_version {
            validate_bounded_label(model_version, "model_version")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrTextPresence {
    Detected,
    NoTextDetected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrWarning {
    pub code: String,
    pub message_key: String,
    pub developer_message: String,
    pub retryable: bool,
}

impl OcrWarning {
    fn validate(&self, limits: &OcrLimits) -> Result<(), OcrContractError> {
        validate_warning_code(&self.code)?;
        validate_bounded_label(&self.message_key, "message_key")?;
        if self.developer_message.is_empty()
            || self.developer_message.len() > limits.max_warning_text_bytes
        {
            return Err(ocr_contract_error(
                "OCR_WARNING_TEXT_INVALID",
                "OCR warning text must be non-empty and bounded",
                false,
            )
            .with_detail("warning_code", self.code.clone())
            .with_detail("max_bytes", limits.max_warning_text_bytes.to_string()));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcrPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcrRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl OcrRect {
    fn validate(&self, source: &OcrImageSource) -> Result<(), OcrContractError> {
        validate_coordinate(self.left, source.width_px, "left")?;
        validate_coordinate(self.right, source.width_px, "right")?;
        validate_coordinate(self.top, source.height_px, "top")?;
        validate_coordinate(self.bottom, source.height_px, "bottom")?;
        if self.left > self.right || self.top > self.bottom {
            return Err(ocr_contract_error(
                "OCR_BOUNDING_BOX_INVALID",
                "OCR bounding box edges must be ordered",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrPolygon {
    pub points: Vec<OcrPoint>,
}

impl OcrPolygon {
    fn validate(&self, source: &OcrImageSource) -> Result<(), OcrContractError> {
        if self.points.len() < MIN_POLYGON_POINTS || self.points.len() > MAX_POLYGON_POINTS {
            return Err(ocr_contract_error(
                "OCR_POLYGON_POINT_COUNT_INVALID",
                "OCR polygon point count must be bounded",
                false,
            )
            .with_detail("point_count", self.points.len().to_string()));
        }
        for point in &self.points {
            validate_coordinate(point.x, source.width_px, "x")?;
            validate_coordinate(point.y, source.height_px, "y")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OcrConfidence {
    Available(f32),
    Unavailable { reason: String },
}

impl OcrConfidence {
    fn validate(&self) -> Result<(), OcrContractError> {
        match self {
            OcrConfidence::Available(value) => {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(ocr_contract_error(
                        "OCR_CONFIDENCE_INVALID",
                        "OCR confidence must be finite and between 0.0 and 1.0",
                        false,
                    ));
                }
            }
            OcrConfidence::Unavailable { reason } => validate_bounded_label(reason, "reason")?,
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrRegionKind {
    Block,
    Line,
    Element,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrRegion {
    pub kind: OcrRegionKind,
    pub source_order: u32,
    pub text: String,
    pub polygon: OcrPolygon,
    pub bounding_box: Option<OcrRect>,
    pub confidence: OcrConfidence,
    pub language_tags: Vec<String>,
}

impl OcrRegion {
    fn validate(
        &self,
        source: &OcrImageSource,
        limits: &OcrLimits,
    ) -> Result<(), OcrContractError> {
        if self.text.len() > limits.max_region_text_bytes {
            return Err(ocr_contract_error(
                "OCR_REGION_TEXT_EXCEEDS_LIMIT",
                "OCR region text exceeds the configured OCR limit",
                false,
            )
            .with_detail("source_order", self.source_order.to_string())
            .with_detail("max_bytes", limits.max_region_text_bytes.to_string()));
        }
        self.polygon.validate(source)?;
        if let Some(bounding_box) = &self.bounding_box {
            bounding_box.validate(source)?;
        }
        self.confidence.validate()?;
        if self.language_tags.len() > limits.max_languages_per_region {
            return Err(ocr_contract_error(
                "OCR_REGION_LANGUAGES_EXCEED_LIMIT",
                "OCR region language count exceeds the configured OCR limit",
                false,
            )
            .with_detail("source_order", self.source_order.to_string())
            .with_detail(
                "max_languages_per_region",
                limits.max_languages_per_region.to_string(),
            ));
        }
        for language in &self.language_tags {
            validate_language_tag(language)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrRecognizedText {
    pub provider: OcrProviderInfo,
    pub text_presence: OcrTextPresence,
    pub full_text: String,
    pub regions: Vec<OcrRegion>,
    pub language_tags: Vec<String>,
    pub warnings: Vec<OcrWarning>,
}

impl OcrRecognizedText {
    fn validate(
        &self,
        source: &OcrImageSource,
        limits: &OcrLimits,
    ) -> Result<(), OcrContractError> {
        self.provider.validate()?;
        if self.full_text.len() > limits.max_full_text_bytes {
            return Err(ocr_contract_error(
                "OCR_FULL_TEXT_EXCEEDS_LIMIT",
                "OCR full text exceeds the configured OCR limit",
                false,
            )
            .with_detail("max_bytes", limits.max_full_text_bytes.to_string()));
        }
        if self.regions.len() > limits.max_regions {
            return Err(ocr_contract_error(
                "OCR_REGION_COUNT_EXCEEDS_LIMIT",
                "OCR region count exceeds the configured OCR limit",
                false,
            )
            .with_detail("region_count", self.regions.len().to_string())
            .with_detail("max_regions", limits.max_regions.to_string()));
        }
        validate_warnings(&self.warnings, limits)?;
        for language in &self.language_tags {
            validate_language_tag(language)?;
        }
        for region in &self.regions {
            region.validate(source, limits)?;
        }
        match self.text_presence {
            OcrTextPresence::Detected => {
                let has_region_text = self.regions.iter().any(|region| !region.text.is_empty());
                if self.full_text.is_empty() && !has_region_text {
                    return Err(ocr_contract_error(
                        "OCR_DETECTED_TEXT_EMPTY",
                        "OCR detected-text result must contain text in full_text or at least one region",
                        false,
                    ));
                }
            }
            OcrTextPresence::NoTextDetected => {
                if !self.full_text.is_empty()
                    || self.regions.iter().any(|region| !region.text.is_empty())
                {
                    return Err(ocr_contract_error(
                        "OCR_NO_TEXT_RESULT_CONTAINS_TEXT",
                        "OCR no-text result must not contain recognized text",
                        false,
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrUnavailableReason {
    ProviderUnavailable,
    AdapterFailure,
    ResourceExhausted,
    UnsupportedImage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrUnavailable {
    pub provider: Option<OcrProviderInfo>,
    pub reason: OcrUnavailableReason,
    pub retryable: bool,
    pub developer_message: String,
    pub warnings: Vec<OcrWarning>,
}

impl OcrUnavailable {
    fn validate(&self, limits: &OcrLimits) -> Result<(), OcrContractError> {
        if let Some(provider) = &self.provider {
            provider.validate()?;
        }
        if self.developer_message.is_empty()
            || self.developer_message.len() > limits.max_warning_text_bytes
        {
            return Err(ocr_contract_error(
                "OCR_UNAVAILABLE_MESSAGE_INVALID",
                "OCR unavailable message must be non-empty and bounded",
                self.retryable,
            ));
        }
        validate_warnings(&self.warnings, limits)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OcrAdapterOutput {
    Recognized(OcrRecognizedText),
    Unavailable(OcrUnavailable),
}

impl OcrAdapterOutput {
    fn validate(
        &self,
        source: &OcrImageSource,
        limits: &OcrLimits,
    ) -> Result<(), OcrContractError> {
        match self {
            OcrAdapterOutput::Recognized(text) => text.validate(source, limits),
            OcrAdapterOutput::Unavailable(unavailable) => unavailable.validate(limits),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub scan_ref: OcrScanRef,
    pub input_asset_ref: OcrAssetRef,
    pub input_kind: OcrInputKind,
    pub body: OcrAdapterOutput,
}

impl OcrResult {
    pub fn is_recognized(&self) -> bool {
        matches!(self.body, OcrAdapterOutput::Recognized(_))
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self.body, OcrAdapterOutput::Unavailable(_))
    }
}

pub trait OcrProviderAdapter {
    fn recognize(&self, request: &OcrRequest) -> OcrAdapterOutcome<OcrAdapterOutput>;
}

pub fn normalize_ocr_output(
    request: &OcrRequest,
    output: OcrAdapterOutput,
) -> Result<OcrResult, OcrContractError> {
    request.validate()?;
    output.validate(&request.source, &request.limits)?;
    Ok(OcrResult {
        scan_ref: request.source.scan_ref.clone(),
        input_asset_ref: request.source.input_asset_ref.clone(),
        input_kind: request.source.input_kind,
        body: output,
    })
}

fn validate_warnings(warnings: &[OcrWarning], limits: &OcrLimits) -> Result<(), OcrContractError> {
    if warnings.len() > limits.max_warning_count {
        return Err(ocr_contract_error(
            "OCR_WARNING_COUNT_EXCEEDS_LIMIT",
            "OCR warning count exceeds the configured OCR limit",
            false,
        )
        .with_detail("warning_count", warnings.len().to_string())
        .with_detail("max_warning_count", limits.max_warning_count.to_string()));
    }
    for warning in warnings {
        warning.validate(limits)?;
    }
    Ok(())
}

fn validate_coordinate(
    value: f32,
    max_inclusive: u32,
    field: &'static str,
) -> Result<(), OcrContractError> {
    if !value.is_finite() || value < 0.0 || value > max_inclusive as f32 {
        return Err(ocr_contract_error(
            "OCR_COORDINATE_OUT_OF_BOUNDS",
            "OCR coordinates must be finite and inside the image bounds",
            false,
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string())
        .with_detail("max_inclusive", max_inclusive.to_string()));
    }
    Ok(())
}

fn validate_reference(value: &str, field: &'static str) -> Result<(), OcrContractError> {
    if value.is_empty() || value.len() > MAX_REF_BYTES {
        return Err(ocr_contract_error(
            "OCR_REFERENCE_INVALID",
            "OCR scan and asset references must be non-empty and bounded",
            false,
        )
        .with_detail("field", field)
        .with_detail("max_bytes", MAX_REF_BYTES.to_string()));
    }
    Ok(())
}

fn validate_bounded_label(value: &str, field: &'static str) -> Result<(), OcrContractError> {
    if value.is_empty() || value.len() > MAX_LABEL_BYTES {
        return Err(ocr_contract_error(
            "OCR_LABEL_INVALID",
            "OCR labels must be non-empty and bounded",
            false,
        )
        .with_detail("field", field)
        .with_detail("max_bytes", MAX_LABEL_BYTES.to_string()));
    }
    Ok(())
}

fn validate_language_tag(value: &str) -> Result<(), OcrContractError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_LANGUAGE_TAG_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if !valid {
        return Err(ocr_contract_error(
            "OCR_LANGUAGE_TAG_INVALID",
            "OCR language tags must be bounded ASCII BCP-47-like tokens",
            false,
        )
        .with_detail("max_bytes", MAX_LANGUAGE_TAG_BYTES.to_string()));
    }
    Ok(())
}

fn validate_warning_code(value: &str) -> Result<(), OcrContractError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_WARNING_CODE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
    if !valid {
        return Err(ocr_contract_error(
            "OCR_WARNING_CODE_INVALID",
            "OCR warning codes must be bounded uppercase ASCII tokens",
            false,
        )
        .with_detail("max_bytes", MAX_WARNING_CODE_BYTES.to_string()));
    }
    Ok(())
}

fn validate_timestamp(value: i64, field: &'static str) -> Result<(), OcrContractError> {
    if value < 0 {
        return Err(ocr_contract_error(
            "OCR_TIMESTAMP_INVALID",
            "OCR timestamps must be non-negative milliseconds",
            false,
        )
        .with_detail("field", field));
    }
    Ok(())
}

fn ocr_contract_error(
    code: &'static str,
    developer_message: impl Into<String>,
    retryable: bool,
) -> OcrContractError {
    OcrContractError::new(code, developer_message, retryable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> OcrRequest {
        OcrRequest {
            source: OcrImageSource {
                scan_ref: OcrScanRef::new("scan-1").unwrap(),
                input_asset_ref: OcrAssetRef::new("asset-1").unwrap(),
                input_kind: OcrInputKind::OcrOptimized,
                width_px: 1_000,
                height_px: 1_400,
            },
            provider_hint: Some("android-ml-kit".to_string()),
            language_hints: vec!["en-US".to_string()],
            limits: OcrLimits::default(),
        }
    }

    fn provider() -> OcrProviderInfo {
        OcrProviderInfo {
            provider: "android-ml-kit".to_string(),
            provider_version: "v1".to_string(),
            model_version: Some("latin-v1".to_string()),
        }
    }

    fn polygon() -> OcrPolygon {
        OcrPolygon {
            points: vec![
                OcrPoint { x: 10.0, y: 10.0 },
                OcrPoint { x: 90.0, y: 10.0 },
                OcrPoint { x: 90.0, y: 40.0 },
                OcrPoint { x: 10.0, y: 40.0 },
            ],
        }
    }

    fn warning(code: &str) -> OcrWarning {
        OcrWarning {
            code: code.to_string(),
            message_key: "ocr.warning".to_string(),
            developer_message: "confidence unavailable from provider".to_string(),
            retryable: false,
        }
    }

    fn recognized_output() -> OcrAdapterOutput {
        OcrAdapterOutput::Recognized(OcrRecognizedText {
            provider: provider(),
            text_presence: OcrTextPresence::Detected,
            full_text: "hello notebook".to_string(),
            regions: vec![OcrRegion {
                kind: OcrRegionKind::Line,
                source_order: 0,
                text: "hello notebook".to_string(),
                polygon: polygon(),
                bounding_box: Some(OcrRect {
                    left: 10.0,
                    top: 10.0,
                    right: 90.0,
                    bottom: 40.0,
                }),
                confidence: OcrConfidence::Unavailable {
                    reason: "provider did not expose confidence".to_string(),
                },
                language_tags: vec!["en-US".to_string()],
            }],
            language_tags: vec!["en-US".to_string()],
            warnings: vec![warning("OCR_CONFIDENCE_UNAVAILABLE")],
        })
    }

    fn unavailable_output(retryable: bool) -> OcrAdapterOutput {
        OcrAdapterOutput::Unavailable(OcrUnavailable {
            provider: Some(provider()),
            reason: OcrUnavailableReason::AdapterFailure,
            retryable,
            developer_message: "text recognizer crashed before producing a result".to_string(),
            warnings: vec![warning("OCR_PROVIDER_FAILED")],
        })
    }

    #[test]
    fn request_rejects_oversized_images_before_provider_work() {
        let mut request = request();
        request.source.width_px = 12_001;

        let err = request.validate().unwrap_err();

        assert_eq!(err.code, "OCR_IMAGE_DIMENSIONS_EXCEED_LIMIT");
    }

    #[test]
    fn typed_references_are_non_empty_and_bounded() {
        let err = OcrScanRef::new("").unwrap_err();
        assert_eq!(err.code, "OCR_REFERENCE_INVALID");

        let asset = OcrAssetRef::new("asset-1").unwrap();
        assert_eq!(asset.as_str(), "asset-1");
    }

    #[test]
    fn recognized_text_preserves_regions_languages_and_confidence_state() {
        let request = request();
        let result = normalize_ocr_output(&request, recognized_output()).unwrap();

        assert!(result.is_recognized());
        assert_eq!(result.scan_ref, request.source.scan_ref);
        match result.body {
            OcrAdapterOutput::Recognized(text) => {
                assert_eq!(text.full_text, "hello notebook");
                assert_eq!(text.regions[0].language_tags, vec!["en-US".to_string()]);
                assert_eq!(text.warnings[0].code, "OCR_CONFIDENCE_UNAVAILABLE");
            }
            OcrAdapterOutput::Unavailable(_) => panic!("expected recognized OCR text"),
        }
    }

    #[test]
    fn failed_ocr_is_unavailable_not_empty_success() {
        let request = request();
        let result = normalize_ocr_output(&request, unavailable_output(true)).unwrap();

        assert!(result.is_unavailable());
        match result.body {
            OcrAdapterOutput::Recognized(_) => panic!("failure must not become empty text"),
            OcrAdapterOutput::Unavailable(unavailable) => {
                assert_eq!(unavailable.reason, OcrUnavailableReason::AdapterFailure);
                assert!(unavailable.retryable);
            }
        }
    }

    #[test]
    fn detected_text_result_cannot_be_empty() {
        let request = request();
        let output = OcrAdapterOutput::Recognized(OcrRecognizedText {
            provider: provider(),
            text_presence: OcrTextPresence::Detected,
            full_text: String::new(),
            regions: Vec::new(),
            language_tags: Vec::new(),
            warnings: Vec::new(),
        });

        let err = normalize_ocr_output(&request, output).unwrap_err();

        assert_eq!(err.code, "OCR_DETECTED_TEXT_EMPTY");
    }

    #[test]
    fn no_text_detected_is_explicit_and_valid() {
        let request = request();
        let output = OcrAdapterOutput::Recognized(OcrRecognizedText {
            provider: provider(),
            text_presence: OcrTextPresence::NoTextDetected,
            full_text: String::new(),
            regions: Vec::new(),
            language_tags: Vec::new(),
            warnings: vec![warning("OCR_NO_TEXT_DETECTED")],
        });

        let result = normalize_ocr_output(&request, output).unwrap();

        assert!(result.is_recognized());
    }

    #[test]
    fn invalid_region_geometry_and_confidence_fail_closed() {
        let request = request();
        let output = OcrAdapterOutput::Recognized(OcrRecognizedText {
            provider: provider(),
            text_presence: OcrTextPresence::Detected,
            full_text: "hello".to_string(),
            regions: vec![OcrRegion {
                kind: OcrRegionKind::Element,
                source_order: 0,
                text: "hello".to_string(),
                polygon: OcrPolygon {
                    points: vec![
                        OcrPoint { x: 10.0, y: 10.0 },
                        OcrPoint { x: 20.0, y: 10.0 },
                        OcrPoint {
                            x: 2_000.0,
                            y: 10.0,
                        },
                    ],
                },
                bounding_box: None,
                confidence: OcrConfidence::Available(1.2),
                language_tags: Vec::new(),
            }],
            language_tags: Vec::new(),
            warnings: Vec::new(),
        });

        let err = normalize_ocr_output(&request, output).unwrap_err();

        assert_eq!(err.code, "OCR_COORDINATE_OUT_OF_BOUNDS");
    }

    #[test]
    fn warning_count_is_bounded() {
        let mut request = request();
        request.limits.max_warning_count = 1;
        let output = OcrAdapterOutput::Unavailable(OcrUnavailable {
            provider: Some(provider()),
            reason: OcrUnavailableReason::ProviderUnavailable,
            retryable: true,
            developer_message: "model is not downloaded".to_string(),
            warnings: vec![warning("OCR_MODEL_MISSING"), warning("OCR_RETRY_LATER")],
        });

        let err = normalize_ocr_output(&request, output).unwrap_err();

        assert_eq!(err.code, "OCR_WARNING_COUNT_EXCEEDS_LIMIT");
    }

    #[test]
    fn queued_job_records_exact_work_key_without_result() {
        let request = request();
        let job = OcrJobRecord::queued(&request, 100).unwrap();

        assert_eq!(job.status, OcrJobStatus::Queued);
        assert_eq!(job.work_key.scan_ref, request.source.scan_ref);
        assert!(job.result.is_none());
        job.validate(&OcrQueueLimits::default()).unwrap();
    }

    #[test]
    fn running_job_increments_attempt_count_without_terminal_result() {
        let job = OcrJobRecord::queued(&request(), 100).unwrap();
        let running = job.mark_running(125).unwrap();

        assert_eq!(running.status, OcrJobStatus::Running);
        assert_eq!(running.retry_state.attempt_count, 1);
        assert_eq!(running.last_started_at_ms, Some(125));
        assert!(running.completed_at_ms.is_none());
    }

    #[test]
    fn recognized_job_completion_binds_to_same_scan_asset_and_kind() {
        let request = request();
        let job = OcrJobRecord::queued(&request, 100)
            .unwrap()
            .mark_running(125)
            .unwrap();
        let result = normalize_ocr_output(&request, recognized_output()).unwrap();
        let completed = job.complete_with_result(result, 150).unwrap();

        assert_eq!(completed.status, OcrJobStatus::Recognized);
        assert_eq!(completed.completed_at_ms, Some(150));
        assert!(completed.result.unwrap().is_recognized());
    }

    #[test]
    fn unavailable_job_completion_remains_retryable_when_provider_says_retryable() {
        let request = request();
        let job = OcrJobRecord::queued(&request, 100)
            .unwrap()
            .mark_running(125)
            .unwrap();
        let result = normalize_ocr_output(&request, unavailable_output(true)).unwrap();
        let completed = job.complete_with_result(result, 150).unwrap();

        assert_eq!(completed.status, OcrJobStatus::Unavailable);
        assert!(completed.retry_state.retryable);
        assert_eq!(
            completed.retry_state.last_error_code.as_deref(),
            Some("OCR_ADAPTERFAILURE")
        );
        assert!(completed.result.unwrap().is_unavailable());
    }

    #[test]
    fn cancellation_is_terminal_without_result_or_retry() {
        let job = OcrJobRecord::queued(&request(), 100)
            .unwrap()
            .mark_running(125)
            .unwrap()
            .cancel(150)
            .unwrap();

        assert_eq!(job.status, OcrJobStatus::Cancelled);
        assert!(job.status.is_terminal());
        assert!(!job.retry_state.retryable);
        assert!(job.result.is_none());
    }

    #[test]
    fn mismatched_result_cannot_complete_a_different_job() {
        let request = request();
        let mut other = request();
        other.source.scan_ref = OcrScanRef::new("scan-2").unwrap();
        let job = OcrJobRecord::queued(&request, 100).unwrap();
        let result = normalize_ocr_output(&other, recognized_output()).unwrap();

        let err = job.complete_with_result(result, 150).unwrap_err();

        assert_eq!(err.code, "OCR_RESULT_WORK_KEY_MISMATCH");
    }

    #[test]
    fn queued_state_rejects_fabricated_terminal_result() {
        let mut job = OcrJobRecord::queued(&request(), 100).unwrap();
        job.result = Some(OcrResult {
            scan_ref: OcrScanRef::new("scan-1").unwrap(),
            input_asset_ref: OcrAssetRef::new("asset-1").unwrap(),
            input_kind: OcrInputKind::OcrOptimized,
            body: unavailable_output(false),
        });

        let err = job.validate(&OcrQueueLimits::default()).unwrap_err();

        assert_eq!(err.code, "OCR_JOB_QUEUED_STATE_INVALID");
    }
}
