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

use crate::error::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use crate::id::{
    AssetId, NotebookDesignId, NotebookId, PageId, PageSetId, PhysicalCopyId, ScanId, SmartPageId,
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

impl std::fmt::Display for LayoutId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: NotebookDesignId,
        schema_version: u32,
        name: String,
        design_version: u32,
        trim_size: TrimSizeMm,
        logical_page_count: u32,
        setup_layout_id: LayoutId,
        page_layout_id: LayoutId,
        marker_family: String,
        marker_role_ids: Vec<String>,
        manifest_hash: String,
        trust_state: TrustState,
    ) -> Self {
        Self {
            id,
            schema_version,
            name,
            design_version,
            trim_size,
            logical_page_count,
            setup_layout_id,
            page_layout_id,
            marker_family,
            marker_role_ids,
            manifest_hash,
            trust_state,
        }
    }

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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: NotebookId,
        design_id: NotebookDesignId,
        display_name: String,
        created_at_ms: i64,
        updated_at_ms: i64,
        archived_at_ms: Option<i64>,
        active_scan_destination: bool,
        optional_color: Option<String>,
        optional_icon: Option<String>,
        optional_user_notes: Option<String>,
    ) -> Self {
        Self {
            id,
            design_id,
            display_name,
            created_at_ms,
            updated_at_ms,
            archived_at_ms,
            active_scan_destination,
            optional_color,
            optional_icon,
            optional_user_notes,
        }
    }

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
    /// The `Asset` (kind `Export`) a generated page's PDF was committed as (TODO 5.5). `None`
    /// for scanned/imported pages, which never had a PDF generated for them, and briefly for a
    /// freshly created generated page before its PDF asset is attached.
    pub generated_pdf_asset_id: Option<AssetId>,
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
            generated_pdf_asset_id: None,
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }

    /// Reconstructs a `Page` with every field explicit, including ones `new` defaults
    /// (`preferred_scan_id`, `generated_pdf_asset_id`, `updated_at_ms`) — for the storage layer
    /// rebuilding a `Page` from a database row, where those fields are already known rather than
    /// freshly defaulted.
    #[allow(clippy::too_many_arguments)]
    pub fn from_stored(
        id: PageId,
        kind: PageKind,
        layout_id: LayoutId,
        title: Option<String>,
        state: PageState,
        preferred_scan_id: Option<ScanId>,
        generated_pdf_asset_id: Option<AssetId>,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Self {
        Self {
            id,
            kind,
            layout_id,
            title,
            state,
            preferred_scan_id,
            generated_pdf_asset_id,
            created_at_ms,
            updated_at_ms,
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

    /// Records which `Asset` this generated page's PDF was committed as (TODO 5.5 "attach the
    /// PDF asset and mark success"). Assignment is single-writer and idempotent: repeating the
    /// same asset succeeds without changing timestamps, while attempting to replace it with a
    /// different asset is an explicit integrity conflict rather than a silent overwrite that
    /// could orphan the original PDF and erase provenance.
    pub fn set_generated_pdf_asset(
        &mut self,
        asset_id: AssetId,
        now_ms: i64,
    ) -> Result<(), A2dError> {
        match &self.generated_pdf_asset_id {
            None => {
                self.generated_pdf_asset_id = Some(asset_id);
                self.updated_at_ms = now_ms;
                Ok(())
            }
            Some(existing) if existing == &asset_id => Ok(()),
            Some(existing) => Err(A2dError::new(
                ErrorCode::new("PAGE_GENERATED_PDF_ASSET_CONFLICT"),
                ErrorCategory::Integrity,
                ErrorSeverity::Error,
                "error.page.generated_pdf_asset_conflict",
                "generated PDF asset is already assigned and cannot be replaced implicitly",
                false,
            )
            .with_detail("page_id", self.id.to_string())
            .with_detail("existing_asset_id", existing.to_string())
            .with_detail("requested_asset_id", asset_id.to_string())),
        }
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ScanId,
        page_id: PageId,
        physical_copy_id: Option<PhysicalCopyId>,
        capture_source: CaptureSource,
        captured_at_ms: i64,
        original_asset_id: AssetId,
        corrected_asset_id: Option<AssetId>,
        ocr_asset_id: Option<AssetId>,
        thumbnail_asset_id: Option<AssetId>,
        pipeline_version: String,
        quality_status: QualityStatus,
        warnings: Vec<String>,
        preferred: bool,
        supersedes_scan_id: Option<ScanId>,
        content_fingerprint: String,
    ) -> Self {
        Self {
            id,
            page_id,
            physical_copy_id,
            capture_source,
            captured_at_ms,
            original_asset_id,
            corrected_asset_id,
            ocr_asset_id,
            thumbnail_asset_id,
            pipeline_version,
            quality_status,
            warnings,
            preferred,
            supersedes_scan_id,
            content_fingerprint,
        }
    }

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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: AssetId,
        kind: AssetKind,
        relative_path: String,
        media_type: String,
        byte_length: u64,
        sha256: String,
        created_at_ms: i64,
        immutable: bool,
        encryption_state: EncryptionState,
    ) -> Self {
        Self {
            id,
            kind,
            relative_path,
            media_type,
            byte_length,
            sha256,
            created_at_ms,
            immutable,
            encryption_state,
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }
}

mod derived;

pub use derived::{
    Annotation, AuditEvent, Collection, OcrRun, PageSet, ReviewItem, ReviewItemKind,
    ReviewItemStatus, SkillDefinition, SkillRun, SkillRunStatus, TextCorrection, TextRegion,
};

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
    fn generated_pdf_asset_assignment_is_single_writer_and_idempotent() {
        let mut page = gen_page(PageId::generate());
        let first = AssetId::generate();
        page.set_generated_pdf_asset(first.clone(), 100).unwrap();
        assert_eq!(page.generated_pdf_asset_id, Some(first.clone()));
        assert_eq!(page.updated_at_ms, 100);

        // Repeating the exact assignment is an idempotent no-op, including the timestamp.
        page.set_generated_pdf_asset(first.clone(), 200).unwrap();
        assert_eq!(page.generated_pdf_asset_id, Some(first.clone()));
        assert_eq!(page.updated_at_ms, 100);

        let replacement = AssetId::generate();
        let err = page
            .set_generated_pdf_asset(replacement.clone(), 300)
            .unwrap_err();
        assert_eq!(err.code.to_string(), "PAGE_GENERATED_PDF_ASSET_CONFLICT");
        assert_eq!(err.category, ErrorCategory::Integrity);
        let first_string = first.to_string();
        let replacement_string = replacement.to_string();
        assert_eq!(err.details.get("existing_asset_id"), Some(&first_string));
        assert_eq!(
            err.details.get("requested_asset_id"),
            Some(&replacement_string)
        );
        assert_eq!(page.generated_pdf_asset_id, Some(first));
        assert_eq!(page.updated_at_ms, 100);
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
