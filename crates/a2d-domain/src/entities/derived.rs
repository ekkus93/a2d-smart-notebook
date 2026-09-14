//! Derived knowledge, review, automation, and audit records.

use std::collections::BTreeMap;

use super::Provenance;
use crate::error::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use crate::id::{
    AnnotationId, AssetId, AuditEventId, CollectionId, OcrRunId, PageId, PageSetId, ReviewItemId,
    ScanId, SkillId, SkillRunId, TextCorrectionId, TextRegionId,
};

/// INFERRED — spec §15.8 describes a Page Set as "a creation relationship" without listing
/// fields. Membership lives on `PageKind::SmartPage::page_set_id`, not duplicated here.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSet {
    id: PageSetId,
    pub title: Option<String>,
    pub created_at_ms: i64,
}

impl PageSet {
    pub fn new(id: PageSetId, title: Option<String>, created_at_ms: i64) -> Self {
        Self {
            id,
            title,
            created_at_ms,
        }
    }

    pub fn id(&self) -> &PageSetId {
        &self.id
    }
}

/// INFERRED — spec §15.8 describes a Collection as "mutable organization" without listing
/// fields. Membership is a many-to-many relation owned by the storage layer, not embedded here,
/// matching "moving a page between collections MUST NOT change its QR identity."
#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    id: CollectionId,
    pub name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl Collection {
    pub fn id(&self) -> &CollectionId {
        &self.id
    }
}

/// The review-item kinds TODO 9.4 enumerates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ReviewItemKind {
    UnidentifiedPage,
    NotebookSelection,
    WrongNotebook,
    LowQuality,
    ManualAlignment,
    Duplicate,
    Revision,
    PhysicalCopy,
    OcrFailure,
    ProcessingFailure,
    ImportConflict,
    RestoreConflict,
}

/// INFERRED — TODO 9.4 requires list/filter/detail/resolve/defer APIs and audited resolutions,
/// so `Deferred` is a persisted nonterminal queue state alongside open/resolved/dismissed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ReviewItemStatus {
    Open,
    Deferred,
    Resolved,
    Dismissed,
}

/// spec §15.9.
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewItem {
    id: ReviewItemId,
    pub kind: ReviewItemKind,
    pub page_id: Option<PageId>,
    pub scan_id: Option<ScanId>,
    pub severity: ErrorSeverity,
    pub status: ReviewItemStatus,
    pub details: BTreeMap<String, String>,
    pub resolution: Option<String>,
    pub created_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

impl ReviewItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ReviewItemId,
        kind: ReviewItemKind,
        page_id: Option<PageId>,
        scan_id: Option<ScanId>,
        severity: ErrorSeverity,
        status: ReviewItemStatus,
        details: BTreeMap<String, String>,
        resolution: Option<String>,
        created_at_ms: i64,
        resolved_at_ms: Option<i64>,
    ) -> Self {
        Self {
            id,
            kind,
            page_id,
            scan_id,
            severity,
            status,
            details,
            resolution,
            created_at_ms,
            resolved_at_ms,
        }
    }

    pub fn id(&self) -> &ReviewItemId {
        &self.id
    }
}

/// Terminal OCR result persisted for a scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrRunStatus {
    /// The provider detected text and `full_text` is non-empty.
    Detected,
    /// The provider ran successfully but found no text. This is not an OCR failure.
    NoTextDetected,
    /// OCR did not produce text because the provider or required resource was unavailable/failed.
    Unavailable,
}

/// Stable reason attached only to `OcrRunStatus::Unavailable`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OcrUnavailableReason {
    ProviderUnavailable,
    ProviderFailed,
    ResourceUnavailable,
    UnsupportedInput,
    Cancelled,
}

/// INFERRED — spec §15.7/§19.1's `OcrResult` shape (provider, provider_version, full_text,
/// warnings), given a persisted identity and provenance. Milestone 11 extends the minimal v1 shape
/// with explicit terminal status so failed/unavailable OCR and successful no-text OCR are never
/// encoded as ambiguous empty text.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrRun {
    id: OcrRunId,
    pub scan_id: ScanId,
    pub input_asset_id: Option<AssetId>,
    pub provider: String,
    pub provider_version: String,
    pub model_name: Option<String>,
    pub status: OcrRunStatus,
    pub full_text: String,
    pub unavailable_reason: Option<OcrUnavailableReason>,
    pub unavailable_message: Option<String>,
    pub completed_at_ms: Option<i64>,
    pub warnings: Vec<String>,
    pub provenance: Provenance,
}

impl OcrRun {
    /// Legacy constructor retained for older call sites. Empty text is normalized to the explicit
    /// `NoTextDetected` outcome rather than being stored as a detected-text success with no text.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: OcrRunId,
        scan_id: ScanId,
        provider: String,
        provider_version: String,
        full_text: String,
        warnings: Vec<String>,
        provenance: Provenance,
    ) -> Self {
        let status = if full_text.is_empty() {
            OcrRunStatus::NoTextDetected
        } else {
            OcrRunStatus::Detected
        };
        Self {
            id,
            scan_id,
            input_asset_id: None,
            provider,
            provider_version,
            model_name: None,
            status,
            full_text,
            unavailable_reason: None,
            unavailable_message: None,
            completed_at_ms: None,
            warnings,
            provenance,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn detected(
        id: OcrRunId,
        scan_id: ScanId,
        input_asset_id: Option<AssetId>,
        provider: String,
        provider_version: String,
        model_name: Option<String>,
        full_text: String,
        completed_at_ms: Option<i64>,
        warnings: Vec<String>,
        provenance: Provenance,
    ) -> Result<Self, A2dError> {
        Self::from_stored(
            id,
            scan_id,
            input_asset_id,
            provider,
            provider_version,
            model_name,
            OcrRunStatus::Detected,
            full_text,
            None,
            None,
            completed_at_ms,
            warnings,
            provenance,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn no_text_detected(
        id: OcrRunId,
        scan_id: ScanId,
        input_asset_id: Option<AssetId>,
        provider: String,
        provider_version: String,
        model_name: Option<String>,
        completed_at_ms: Option<i64>,
        warnings: Vec<String>,
        provenance: Provenance,
    ) -> Result<Self, A2dError> {
        Self::from_stored(
            id,
            scan_id,
            input_asset_id,
            provider,
            provider_version,
            model_name,
            OcrRunStatus::NoTextDetected,
            String::new(),
            None,
            None,
            completed_at_ms,
            warnings,
            provenance,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn unavailable(
        id: OcrRunId,
        scan_id: ScanId,
        input_asset_id: Option<AssetId>,
        provider: String,
        provider_version: String,
        model_name: Option<String>,
        reason: OcrUnavailableReason,
        message: Option<String>,
        completed_at_ms: Option<i64>,
        warnings: Vec<String>,
        provenance: Provenance,
    ) -> Result<Self, A2dError> {
        Self::from_stored(
            id,
            scan_id,
            input_asset_id,
            provider,
            provider_version,
            model_name,
            OcrRunStatus::Unavailable,
            String::new(),
            Some(reason),
            message,
            completed_at_ms,
            warnings,
            provenance,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_stored(
        id: OcrRunId,
        scan_id: ScanId,
        input_asset_id: Option<AssetId>,
        provider: String,
        provider_version: String,
        model_name: Option<String>,
        status: OcrRunStatus,
        full_text: String,
        unavailable_reason: Option<OcrUnavailableReason>,
        unavailable_message: Option<String>,
        completed_at_ms: Option<i64>,
        warnings: Vec<String>,
        provenance: Provenance,
    ) -> Result<Self, A2dError> {
        validate_ocr_terminal_state(status, &full_text, unavailable_reason, &unavailable_message)?;
        Ok(Self {
            id,
            scan_id,
            input_asset_id,
            provider,
            provider_version,
            model_name,
            status,
            full_text,
            unavailable_reason,
            unavailable_message,
            completed_at_ms,
            warnings,
            provenance,
        })
    }

    pub fn id(&self) -> &OcrRunId {
        &self.id
    }
}

fn validate_ocr_terminal_state(
    status: OcrRunStatus,
    full_text: &str,
    unavailable_reason: Option<OcrUnavailableReason>,
    unavailable_message: &Option<String>,
) -> Result<(), A2dError> {
    match status {
        OcrRunStatus::Detected => {
            if full_text.is_empty() {
                return Err(ocr_state_error(
                    "OCR_RUN_DETECTED_TEXT_EMPTY",
                    "detected OCR runs must carry non-empty full_text",
                ));
            }
            if unavailable_reason.is_some() || unavailable_message.is_some() {
                return Err(ocr_state_error(
                    "OCR_RUN_DETECTED_HAS_UNAVAILABLE_DETAIL",
                    "detected OCR runs cannot carry unavailable reason/message details",
                ));
            }
        }
        OcrRunStatus::NoTextDetected => {
            if !full_text.is_empty() {
                return Err(ocr_state_error(
                    "OCR_RUN_NO_TEXT_HAS_TEXT",
                    "no-text OCR runs must not carry full_text",
                ));
            }
            if unavailable_reason.is_some() || unavailable_message.is_some() {
                return Err(ocr_state_error(
                    "OCR_RUN_NO_TEXT_HAS_UNAVAILABLE_DETAIL",
                    "no-text OCR runs cannot carry unavailable reason/message details",
                ));
            }
        }
        OcrRunStatus::Unavailable => {
            if !full_text.is_empty() {
                return Err(ocr_state_error(
                    "OCR_RUN_UNAVAILABLE_HAS_TEXT",
                    "unavailable OCR runs must not carry full_text",
                ));
            }
            if unavailable_reason.is_none() {
                return Err(ocr_state_error(
                    "OCR_RUN_UNAVAILABLE_REASON_MISSING",
                    "unavailable OCR runs must carry an unavailable reason",
                ));
            }
        }
    }
    Ok(())
}

fn ocr_state_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.run_state_invalid",
        message,
        false,
    )
}

const MIN_TEXT_REGION_POLYGON_POINTS: usize = 3;
const MAX_TEXT_REGION_POLYGON_POINTS: usize = 8;
const MAX_TEXT_REGION_TEXT_BYTES: usize = 20_000;

/// INFERRED — spec §15.7: "polygons, confidence where available, source region."
#[derive(Clone, Debug, PartialEq)]
pub struct TextRegion {
    id: TextRegionId,
    pub ocr_run_id: OcrRunId,
    pub polygon: Vec<(f32, f32)>,
    pub text: String,
    pub confidence: Option<f32>,
    pub created_at_ms: i64,
}

impl TextRegion {
    pub fn new(
        id: TextRegionId,
        ocr_run_id: OcrRunId,
        polygon: Vec<(f32, f32)>,
        text: String,
        confidence: Option<f32>,
        created_at_ms: i64,
    ) -> Result<Self, A2dError> {
        Self::from_stored(id, ocr_run_id, polygon, text, confidence, created_at_ms)
    }

    pub fn from_stored(
        id: TextRegionId,
        ocr_run_id: OcrRunId,
        polygon: Vec<(f32, f32)>,
        text: String,
        confidence: Option<f32>,
        created_at_ms: i64,
    ) -> Result<Self, A2dError> {
        validate_text_region(&polygon, &text, confidence, created_at_ms)?;
        Ok(Self {
            id,
            ocr_run_id,
            polygon,
            text,
            confidence,
            created_at_ms,
        })
    }

    pub fn id(&self) -> &TextRegionId {
        &self.id
    }
}

fn validate_text_region(
    polygon: &[(f32, f32)],
    text: &str,
    confidence: Option<f32>,
    created_at_ms: i64,
) -> Result<(), A2dError> {
    if polygon.len() < MIN_TEXT_REGION_POLYGON_POINTS
        || polygon.len() > MAX_TEXT_REGION_POLYGON_POINTS
    {
        return Err(text_region_state_error(
            "TEXT_REGION_POLYGON_POINT_COUNT_INVALID",
            "text-region polygon point count must be bounded",
        )
        .with_detail("point_count", polygon.len().to_string())
        .with_detail(
            "min_polygon_points",
            MIN_TEXT_REGION_POLYGON_POINTS.to_string(),
        )
        .with_detail(
            "max_polygon_points",
            MAX_TEXT_REGION_POLYGON_POINTS.to_string(),
        ));
    }
    for (point_index, (x, y)) in polygon.iter().copied().enumerate() {
        validate_text_region_coordinate(x, "x", point_index)?;
        validate_text_region_coordinate(y, "y", point_index)?;
    }
    if text.is_empty() {
        return Err(text_region_state_error(
            "TEXT_REGION_TEXT_EMPTY",
            "text regions must carry non-empty recognized text",
        ));
    }
    if text.len() > MAX_TEXT_REGION_TEXT_BYTES {
        return Err(text_region_state_error(
            "TEXT_REGION_TEXT_EXCEEDS_LIMIT",
            "text-region text exceeds the configured OCR region limit",
        )
        .with_detail("text_bytes", text.len().to_string())
        .with_detail("max_text_bytes", MAX_TEXT_REGION_TEXT_BYTES.to_string()));
    }
    if let Some(confidence) = confidence
        && (!confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
    {
        return Err(text_region_state_error(
            "TEXT_REGION_CONFIDENCE_INVALID",
            "text-region confidence must be finite and between 0.0 and 1.0",
        ));
    }
    if created_at_ms < 0 {
        return Err(text_region_state_error(
            "TEXT_REGION_TIMESTAMP_INVALID",
            "text-region timestamp must be non-negative milliseconds",
        ));
    }
    Ok(())
}

fn validate_text_region_coordinate(
    value: f32,
    coordinate: &'static str,
    point_index: usize,
) -> Result<(), A2dError> {
    if !value.is_finite() || value < 0.0 {
        return Err(text_region_state_error(
            "TEXT_REGION_POLYGON_COORDINATE_INVALID",
            "text-region polygon coordinates must be finite and non-negative",
        )
        .with_detail("coordinate", coordinate)
        .with_detail("point_index", point_index.to_string()));
    }
    Ok(())
}

fn text_region_state_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Ocr,
        ErrorSeverity::Error,
        "error.ocr.text_region_invalid",
        message,
        false,
    )
}

/// INFERRED — spec §15.7: "correction history" implies each correction is its own record rather
/// than an in-place edit, so prior text is preserved.
#[derive(Clone, Debug, PartialEq)]
pub struct TextCorrection {
    id: TextCorrectionId,
    pub text_region_id: Option<TextRegionId>,
    pub scan_id: ScanId,
    pub corrected_text: String,
    pub previous_text: Option<String>,
    pub provenance: Provenance,
}

impl TextCorrection {
    pub fn id(&self) -> &TextCorrectionId {
        &self.id
    }
}

/// INFERRED — spec mentions annotations only via capabilities (§21.3 `pages.create_annotation`)
/// and the page viewer (§10.5), never with a field list.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    id: AnnotationId,
    pub page_id: PageId,
    pub body: String,
    pub region: Option<Vec<(f32, f32)>>,
    pub provenance: Provenance,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl Annotation {
    pub fn id(&self) -> &AnnotationId {
        &self.id
    }
}

/// INFERRED — the registered form of TODO 14.3's skill manifest YAML. Permission/network/
/// mutation-policy values stay as strings here rather than dedicated enums: Milestone 14 owns
/// the real permission model, and duplicating it ahead of that milestone risks diverging from it.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillDefinition {
    id: SkillId,
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub permissions: Vec<String>,
    pub model_requirements: Vec<String>,
    pub network: String,
    pub mutation_policy: String,
    pub manifest_hash: String,
}

impl SkillDefinition {
    pub fn id(&self) -> &SkillId {
        &self.id
    }
}

/// INFERRED.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SkillRunStatus {
    Running,
    Completed,
    Denied,
    Failed,
    Cancelled,
}

/// INFERRED — a skill execution record (spec §6: "Skill execution record" -> "Skill History"),
/// carrying the per-run effective permission snapshot TODO 14.4 requires.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillRun {
    id: SkillRunId,
    pub skill_id: SkillId,
    pub started_at_ms: i64,
    pub completed_at_ms: Option<i64>,
    pub status: SkillRunStatus,
    pub granted_permissions: Vec<String>,
    pub scope_description: String,
    pub provenance: Provenance,
    pub warnings: Vec<String>,
}

impl SkillRun {
    pub fn id(&self) -> &SkillRunId {
        &self.id
    }
}

/// INFERRED — spec §9.1 requires "audit records" and §21.3 "every run is audited," without a
/// field list. `subject`/`details` stay generic strings since an audit event can reference any
/// kind of entity, not just one.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditEvent {
    id: AuditEventId,
    pub occurred_at_ms: i64,
    pub event_kind: String,
    pub actor: String,
    pub subject: Option<String>,
    pub details: BTreeMap<String, String>,
    pub correlation_id: Option<String>,
}

impl AuditEvent {
    pub fn new(
        id: AuditEventId,
        occurred_at_ms: i64,
        event_kind: String,
        actor: String,
        subject: Option<String>,
        details: BTreeMap<String, String>,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            id,
            occurred_at_ms,
            event_kind,
            actor,
            subject,
            details,
            correlation_id,
        }
    }

    pub fn id(&self) -> &AuditEventId {
        &self.id
    }
}
