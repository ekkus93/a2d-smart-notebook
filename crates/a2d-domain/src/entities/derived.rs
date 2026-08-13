//! Derived knowledge, review, automation, and audit records.

use std::collections::BTreeMap;

use super::Provenance;
use crate::error::ErrorSeverity;
use crate::id::{
    AnnotationId, AuditEventId, CollectionId, OcrRunId, PageId, PageSetId, ReviewItemId, ScanId,
    SkillId, SkillRunId, TextCorrectionId, TextRegionId,
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

/// INFERRED — spec §15.7/§19.1's `OcrResult` shape (provider, provider_version, full_text,
/// warnings), given a persisted identity and provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrRun {
    id: OcrRunId,
    pub scan_id: ScanId,
    pub provider: String,
    pub provider_version: String,
    pub full_text: String,
    pub warnings: Vec<String>,
    pub provenance: Provenance,
}

impl OcrRun {
    pub fn new(
        id: OcrRunId,
        scan_id: ScanId,
        provider: String,
        provider_version: String,
        full_text: String,
        warnings: Vec<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id,
            scan_id,
            provider,
            provider_version,
            full_text,
            warnings,
            provenance,
        }
    }

    pub fn id(&self) -> &OcrRunId {
        &self.id
    }
}

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
    pub fn id(&self) -> &TextRegionId {
        &self.id
    }
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
