//! Core domain entities, invariants, and typed identifiers shared by every other crate.

pub mod clock;
pub mod entities;
pub mod error;
pub mod id;

pub use clock::{Clock, SystemClock, system_now_ms, unix_millis};
pub use entities::{
    Annotation, Asset, AssetKind, AuditEvent, CaptureSource, Collection, EncryptionState, LayoutId,
    Notebook, NotebookDesign, OcrRun, Page, PageKind, PageSet, PageState, PhysicalCopy, Provenance,
    QualityStatus, ReviewItem, ReviewItemKind, ReviewItemStatus, Scan, SkillDefinition, SkillRun,
    SkillRunStatus, TextCorrection, TextRegion, TrimSizeMm, TrustState,
};
pub use error::{A2dError, A2dErrorFields, ErrorCategory, ErrorCode, ErrorSeverity, Outcome};
pub use id::{
    AnnotationId, AssetId, AuditEventId, BackupId, CollectionId, InstallationId, NotebookDesignId,
    NotebookId, OcrRunId, PageId, PageSetId, PhysicalCopyId, ReviewItemId, ScanId, SkillId,
    SkillRunId, SmartPageId, TextCorrectionId, TextRegionId,
};
