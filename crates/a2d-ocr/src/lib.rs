//! Provider-adapter contract for OCR.
//!
//! Rust owns the normalized OCR request/result shape. Platform adapters such as Android ML Kit
//! can recognize text, but they must report bounded, explicit outcomes through this contract:
//! successful recognition, successful no-text detection, or OCR unavailability/failure. A failed
//! OCR run must never be converted into a fabricated empty successful transcription.

use a2d_domain::{A2dError, AssetId, ErrorCategory, ErrorCode, ErrorSeverity, Outcome, ScanId};

const MAX_LANGUAGE_TAG_BYTES: usize = 35;
const MAX_LABEL_BYTES: usize = 120;
const MAX_WARNING_CODE_BYTES: usize = 80;
const MIN_POLYGON_POINTS: usize = 3;
const MAX_POLYGON_POINTS: usize = 8;

/// Resource limits Rust applies to every platform OCR adapter result before it may be persisted.
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
    fn validate(&self) -> Result<(), A2dError> {
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

/// Which immutable Rust-owned asset is being submitted to a platform OCR adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrInputKind {
    Original,
    Corrected,
    OcrOptimized,
}

/// Bounded descriptor for the image supplied to OCR. The image bytes remain asset-owned; adapters
/// receive or resolve bytes through platform code, then return normalized OCR results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrImageSource {
    pub scan_id: ScanId,
    pub input_asset_id: AssetId,
    pub input_kind: OcrInputKind,
    pub width_px: u32,
    pub height_px: u32,
}

impl OcrImageSource {
    fn validate(&self, limits: &OcrLimits) -> Result<(), A2dError> {
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

/// A normalized OCR request. Provider hints are hints only; Rust remains authoritative for the
/// source scan/asset identity and resource limits.
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

    pub fn validate(&self) -> Result<(), A2dError> {
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

/// The OCR engine and model/adapter versions that produced a result or failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrProviderInfo {
    pub provider: String,
    pub provider_version: String,
    pub model_version: Option<String>,
}

impl OcrProviderInfo {
    fn validate(&self) -> Result<(), A2dError> {
        validate_bounded_label(&self.provider, "provider")?;
        validate_bounded_label(&self.provider_version, "provider_version")?;
        if let Some(model_version) = &self.model_version {
            validate_bounded_label(model_version, "model_version")?;
        }
        Ok(())
    }
}

/// OCR output must distinguish actual detected text from a valid no-text outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrTextPresence {
    Detected,
    NoTextDetected,
}

/// OCR warning codes are persisted/audited metadata. Warning text must describe adapter state, not
/// raw note content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrWarning {
    pub code: String,
    pub message_key: String,
    pub developer_message: String,
    pub retryable: bool,
}

impl OcrWarning {
    fn validate(&self, limits: &OcrLimits) -> Result<(), A2dError> {
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
    fn validate(&self, source: &OcrImageSource) -> Result<(), A2dError> {
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
    fn validate(&self, source: &OcrImageSource) -> Result<(), A2dError> {
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

/// Provider confidence is optional, but unavailability must be explicit and explainable.
#[derive(Clone, Debug, PartialEq)]
pub enum OcrConfidence {
    Available(f32),
    Unavailable { reason: String },
}

impl OcrConfidence {
    fn validate(&self) -> Result<(), A2dError> {
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
    fn validate(&self, source: &OcrImageSource, limits: &OcrLimits) -> Result<(), A2dError> {
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
    fn validate(&self, source: &OcrImageSource, limits: &OcrLimits) -> Result<(), A2dError> {
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
    fn validate(&self, limits: &OcrLimits) -> Result<(), A2dError> {
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
    fn validate(&self, source: &OcrImageSource, limits: &OcrLimits) -> Result<(), A2dError> {
        match self {
            OcrAdapterOutput::Recognized(text) => text.validate(source, limits),
            OcrAdapterOutput::Unavailable(unavailable) => unavailable.validate(limits),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub scan_id: ScanId,
    pub input_asset_id: AssetId,
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

/// Platform adapters implement OCR recognition and return a cancellable outcome. Cancellation is
/// not failure, and adapter failures are returned as [`OcrAdapterOutput::Unavailable`].
pub trait OcrProviderAdapter {
    fn recognize(&self, request: &OcrRequest) -> Outcome<OcrAdapterOutput>;
}

/// Validate and bind an adapter output to the exact scan/asset request Rust submitted.
pub fn normalize_ocr_output(
    request: &OcrRequest,
    output: OcrAdapterOutput,
) -> Result<OcrResult, A2dError> {
    request.validate()?;
    output.validate(&request.source, &request.limits)?;
    Ok(OcrResult {
        scan_id: request.source.scan_id.clone(),
        input_asset_id: request.source.input_asset_id.clone(),
        input_kind: request.source.input_kind,
        body: output,
    })
}

fn validate_warnings(warnings: &[OcrWarning], limits: &OcrLimits) -> Result<(), A2dError> {
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

fn validate_coordinate(value: f32, max_inclusive: u32, field: &'static str) -> Result<(), A2dError> {
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

fn validate_bounded_label(value: &str, field: &'static str) -> Result<(), A2dError> {
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

fn validate_language_tag(value: &str) -> Result<(), A2dError> {
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

fn validate_warning_code(value: &str) -> Result<(), A2dError> {
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

fn ocr_contract_error(
    code: &'static str,
    developer_message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.contract",
        developer_message,
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> OcrRequest {
        OcrRequest {
            source: OcrImageSource {
                scan_id: ScanId::generate(),
                input_asset_id: AssetId::generate(),
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

    #[test]
    fn request_rejects_oversized_images_before_provider_work() {
        let mut request = request();
        request.source.width_px = 12_001;

        let err = request.validate().unwrap_err();

        assert_eq!(err.code.to_string(), "OCR_IMAGE_DIMENSIONS_EXCEED_LIMIT");
        assert_eq!(err.category, ErrorCategory::Ocr);
    }

    #[test]
    fn recognized_text_preserves_regions_languages_and_confidence_state() {
        let request = request();
        let output = OcrAdapterOutput::Recognized(OcrRecognizedText {
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
        });

        let result = normalize_ocr_output(&request, output).unwrap();

        assert!(result.is_recognized());
        assert_eq!(result.scan_id, request.source.scan_id);
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
        let output = OcrAdapterOutput::Unavailable(OcrUnavailable {
            provider: Some(provider()),
            reason: OcrUnavailableReason::AdapterFailure,
            retryable: true,
            developer_message: "text recognizer crashed before producing a result".to_string(),
            warnings: vec![warning("OCR_PROVIDER_FAILED")],
        });

        let result = normalize_ocr_output(&request, output).unwrap();

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

        assert_eq!(err.code.to_string(), "OCR_DETECTED_TEXT_EMPTY");
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
                        OcrPoint { x: 2_000.0, y: 10.0 },
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

        assert_eq!(err.code.to_string(), "OCR_COORDINATE_OUT_OF_BOUNDS");
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

        assert_eq!(err.code.to_string(), "OCR_WARNING_COUNT_EXCEEDS_LIMIT");
    }
}
