//! Domain entities: the persisted records the rest of the system operates on (TODO 2.3, spec §15).
//!
//! Field lists follow spec §15 as literally as possible for the entities it enumerates in full
//! (`NotebookDesign`, `Notebook`, `Page`, `PhysicalCopy`, `Scan`, `Asset`). Spec §15.7–§15.10
//! describe the remaining entities only in prose; their fields below are inferred from that prose
//! plus TODO 11.1's `OcrRequest`/`OcrResult` and TODO 14.3's skill manifest shape, marked
//! `INFERRED` in each doc comment. This is a larger set of assumptions than usual for one task —
//! see `memory.md` for the full list, since several of these will need revisiting once their
//! owning milestones (5 layouts, 7 markers, 14 skills) pin down real requirements.
//!
//! This module enforces invariants checkable from one or two records' fields alone. Invariants
//! spanning a whole table (unique physical-copy index per page, "Smart Page requires a unique
//! Smart Page ID") belong to the storage layer (Milestone 3), which can see every row; a single
//! struct can't enforce uniqueness by construction.
//!
//! Every entity's `id` field is private with a public getter, so identity cannot change after
//! construction (TODO 2.3: "Page identity cannot change after creation" — applied to every
//! entity, not just `Page`, since the same reasoning applies to all of them).

use std::collections::BTreeMap;

use crate::error::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use crate::id::{
    AnnotationId, AssetId, AuditEventId, CollectionId, NotebookDesignId, NotebookId, OcrRunId,
    PageId, PageSetId, PhysicalCopyId, ReviewItemId, ScanId, SkillId, SkillRunId, SmartPageId,
    TextCorrectionId, TextRegionId,
};

/// A short, stable, human-readable layout registry key (e.g. `"USLETTER-LINED"`), not a random
/// 128-bit identifier. Its validation and registry live in `a2d-layout` (Milestone 5); this
/// newtype exists so `Page`/`NotebookDesign` don't carry a raw `String` for a typed reference.
/// Not part of the spec §13 core-identifier list — added here because the "opaque newtype, never
/// raw string" rule applies to it too. Flagged in `memory.md` as an addition beyond what was
/// explicitly reviewed.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LayoutId(String);

impl LayoutId {
    /// Accepts 1–20 uppercase ASCII alphanumeric or `-` characters, matching the token shape
    /// `docs/decisions/0001-qr-v1-encoding-and-integrity.md` already specifies for layout ids
    /// embedded in QR payloads.
    pub fn parse(s: &str) -> Result<Self, A2dError> {
        let valid = !s.is_empty()
            && s.len() <= 20
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-');
        if !valid {
            return Err(A2dError::new(
                ErrorCode::new("LAYOUT_ID_INVALID"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.id.invalid",
                "LayoutId must be 1-20 uppercase alphanumeric/hyphen characters",
                false,
            )
            .with_detail("input", s));
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Metadata every derived (non-original) record MUST carry (TODO 2.3: "Derived records identify
/// source and producer"; spec §15.10). An embedded value object, not independently persisted, so
/// it gets no identifier of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Provenance {
    pub source_page_id: Option<PageId>,
    pub source_scan_id: Option<ScanId>,
    pub producing_component: String,
    pub component_version: String,
    pub created_at_ms: i64,
    pub warnings: Vec<String>,
    pub user_approved: Option<bool>,
}

/// Physical trim dimensions in millimeters. Only the *shape* is defined here — Milestone 5.3
/// picks the actual v1 numbers ("Record the first trim-size decision").
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrimSizeMm {
    pub width: u32,
    pub height: u32,
}

/// Whether a Notebook Design's manifest is known-official. Starting enumeration — spec §14.4
/// reserves room for a future signed-manifest extension, which will likely need more states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TrustState {
    Unverified,
    Trusted,
    Revoked,
}

/// spec §15.1.
#[derive(Clone, Debug, PartialEq)]
pub struct NotebookDesign {
    id: NotebookDesignId,
    pub schema_version: u32,
    pub name: String,
    pub design_version: u32,
    pub trim_size: TrimSizeMm,
    pub logical_page_count: u32,
    pub setup_layout_id: LayoutId,
    pub page_layout_id: LayoutId,
    pub marker_family: String,
    pub marker_role_ids: Vec<String>,
    pub manifest_hash: String,
    pub trust_state: TrustState,
}

impl NotebookDesign {
    pub fn id(&self) -> &NotebookDesignId {
        &self.id
    }
}

/// spec §15.2.
#[derive(Clone, Debug, PartialEq)]
pub struct Notebook {
    id: NotebookId,
    pub design_id: NotebookDesignId,
    pub display_name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub archived_at_ms: Option<i64>,
    pub active_scan_destination: bool,
    pub optional_color: Option<String>,
    pub optional_icon: Option<String>,
    pub optional_user_notes: Option<String>,
}

impl Notebook {
    pub fn id(&self) -> &NotebookId {
        &self.id
    }
}

/// The two ways a `Page` can be identified (TODO 2.3's suggested shape). Each variant requires
/// exactly the fields spec §15.3 lists for that kind — "Notebook Page requires notebook, design,
/// and logical page number" and "Smart Page requires a [...] Smart Page ID" are enforced by the
/// compiler: there is no way to construct either variant without its required fields.
#[derive(Clone, Debug, PartialEq)]
pub enum PageKind {
    NotebookPage {
        notebook_id: NotebookId,
        design_id: NotebookDesignId,
        logical_page_number: u32,
    },
    SmartPage {
        smart_page_id: SmartPageId,
        page_set_id: Option<PageSetId>,
        visible_page_number: Option<u32>,
    },
}

/// spec §12.4's example states, taken as the v1 set (spec hedges with "such as," so this may
/// need extending later).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PageState {
    GeneratedNotScanned,
    Scanned,
    NeedsReview,
    Archived,
    Trashed,
}

/// spec §15.3.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    id: PageId,
    pub kind: PageKind,
    pub layout_id: LayoutId,
    pub title: Option<String>,
    pub state: PageState,
    pub preferred_scan_id: Option<ScanId>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl Page {
    pub fn new(
        id: PageId,
        kind: PageKind,
        layout_id: LayoutId,
        title: Option<String>,
        state: PageState,
        created_at_ms: i64,
    ) -> Self {
        Self {
            id,
            kind,
            layout_id,
            title,
            state,
            preferred_scan_id: None,
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }

    pub fn id(&self) -> &PageId {
        &self.id
    }

    /// Sets the preferred scan, rejecting a scan that belongs to a different page (TODO 2.3:
    /// "Preferred scan belongs to the same page") — the one cross-record check in this module
    /// that a single call site can actually verify, since it only needs the two records
    /// involved, not the whole scan table.
    pub fn set_preferred_scan(&mut self, scan: &Scan, now_ms: i64) -> Result<(), A2dError> {
        if scan.page_id != self.id {
            return Err(A2dError::new(
                ErrorCode::new("PAGE_PREFERRED_SCAN_MISMATCH"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.page.preferred_scan_mismatch",
                "preferred scan must belong to this page",
                false,
            )
            .with_detail("page_id", self.id.to_string())
            .with_detail("scan_page_id", scan.page_id.to_string()));
        }
        self.preferred_scan_id = Some(scan.id().clone());
        self.updated_at_ms = now_ms;
        Ok(())
    }
}

/// spec §15.4.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalCopy {
    id: PhysicalCopyId,
    pub page_id: PageId,
    pub copy_index: u32,
    pub created_at_ms: i64,
    pub display_label: Option<String>,
}

impl PhysicalCopy {
    pub fn id(&self) -> &PhysicalCopyId {
        &self.id
    }
}

/// INFERRED — spec describes camera and import capture flows (§7.3, §7.8) but doesn't name this
/// enum explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CaptureSource {
    Camera,
    Import,
}

/// spec §17.6, given as the exact (non-hedged) v1 set.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum QualityStatus {
    Accepted,
    AcceptedWithWarnings,
    NeedsReview,
    Rejected,
}

/// spec §15.5.
#[derive(Clone, Debug, PartialEq)]
pub struct Scan {
    id: ScanId,
    pub page_id: PageId,
    pub physical_copy_id: Option<PhysicalCopyId>,
    pub capture_source: CaptureSource,
    pub captured_at_ms: i64,
    pub original_asset_id: AssetId,
    pub corrected_asset_id: Option<AssetId>,
    pub ocr_asset_id: Option<AssetId>,
    pub thumbnail_asset_id: Option<AssetId>,
    pub pipeline_version: String,
    pub quality_status: QualityStatus,
    pub warnings: Vec<String>,
    pub preferred: bool,
    pub supersedes_scan_id: Option<ScanId>,
    pub content_fingerprint: String,
}

impl Scan {
    pub fn id(&self) -> &ScanId {
        &self.id
    }
}

/// INFERRED — matches the asset repository layout TODO 3.3 describes
/// (`assets/{originals,corrected,ocr,thumbnails,exports}/`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AssetKind {
    Original,
    Corrected,
    Ocr,
    Thumbnail,
    Export,
}

/// INFERRED — spec names the field but not its value set.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EncryptionState {
    Plaintext,
    Encrypted,
}

/// spec §15.6. `immutable` MUST be `true` for every original asset once committed (spec §3.3,
/// §16.3) — the asset-commit protocol that guarantees this lives in Milestone 3, not here.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    id: AssetId,
    pub kind: AssetKind,
    pub relative_path: String,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: String,
    pub created_at_ms: i64,
    pub immutable: bool,
    pub encryption_state: EncryptionState,
}

impl Asset {
    pub fn id(&self) -> &AssetId {
        &self.id
    }
}

/// INFERRED — spec §15.8 describes a Page Set as "a creation relationship" without listing
/// fields. Membership lives on `PageKind::SmartPage::page_set_id`, not duplicated here.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSet {
    id: PageSetId,
    pub title: Option<String>,
    pub created_at_ms: i64,
}

impl PageSet {
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

/// INFERRED — TODO 9.4 requires "list/filter/detail/resolve" APIs and audited resolutions,
/// implying an open/resolved lifecycle, but doesn't name the states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ReviewItemStatus {
    Open,
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
    pub fn id(&self) -> &AuditEventId {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{AssetId, NotebookDesignId, NotebookId, PageId, ScanId};

    fn gen_page(id: PageId) -> Page {
        Page::new(
            id,
            PageKind::NotebookPage {
                notebook_id: NotebookId::generate(),
                design_id: NotebookDesignId::generate(),
                logical_page_number: 1,
            },
            LayoutId::parse("USLETTER-LINED").unwrap(),
            None,
            PageState::GeneratedNotScanned,
            0,
        )
    }

    fn gen_scan(page_id: PageId) -> Scan {
        Scan {
            id: ScanId::generate(),
            page_id,
            physical_copy_id: None,
            capture_source: CaptureSource::Camera,
            captured_at_ms: 0,
            original_asset_id: AssetId::generate(),
            corrected_asset_id: None,
            ocr_asset_id: None,
            thumbnail_asset_id: None,
            pipeline_version: "v1".to_string(),
            quality_status: QualityStatus::Accepted,
            warnings: vec![],
            preferred: true,
            supersedes_scan_id: None,
            content_fingerprint: "fingerprint".to_string(),
        }
    }

    #[test]
    fn layout_id_rejects_lowercase_and_overlong_input() {
        assert!(LayoutId::parse("USLETTER-LINED").is_ok());
        assert!(LayoutId::parse("usletter-lined").is_err());
        assert!(LayoutId::parse(&"A".repeat(21)).is_err());
        assert!(LayoutId::parse("").is_err());
    }

    #[test]
    fn page_identity_is_read_only() {
        let page = gen_page(PageId::generate());
        // `page.id()` returns a reference; there is no setter, so identity cannot change after
        // construction other than by constructing a whole new Page.
        let id_before = page.id().clone();
        assert_eq!(&id_before, page.id());
    }

    #[test]
    fn preferred_scan_must_belong_to_the_same_page() {
        let mut page = gen_page(PageId::generate());
        let own_scan = gen_scan(page.id().clone());
        assert!(page.set_preferred_scan(&own_scan, 100).is_ok());
        assert_eq!(page.preferred_scan_id, Some(own_scan.id().clone()));

        let other_scan = gen_scan(PageId::generate());
        let err = page.set_preferred_scan(&other_scan, 200).unwrap_err();
        assert!(
            err.code
                .to_string()
                .contains("PAGE_PREFERRED_SCAN_MISMATCH")
        );
        // Rejected assignment must not have mutated state.
        assert_eq!(page.preferred_scan_id, Some(own_scan.id().clone()));
    }

    #[test]
    fn page_kind_variants_carry_their_required_fields() {
        let notebook_page = PageKind::NotebookPage {
            notebook_id: NotebookId::generate(),
            design_id: NotebookDesignId::generate(),
            logical_page_number: 5,
        };
        match notebook_page {
            PageKind::NotebookPage {
                logical_page_number,
                ..
            } => assert_eq!(logical_page_number, 5),
            PageKind::SmartPage { .. } => panic!("wrong variant"),
        }

        let smart_page = PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: Some(3),
        };
        match smart_page {
            PageKind::SmartPage {
                visible_page_number,
                ..
            } => assert_eq!(visible_page_number, Some(3)),
            PageKind::NotebookPage { .. } => panic!("wrong variant"),
        }
    }
}
