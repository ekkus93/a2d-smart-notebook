//! Core domain entities, invariants, and typed identifiers shared by every other crate.

pub mod error;
pub mod id;

pub use error::{A2dError, A2dErrorFields, ErrorCategory, ErrorCode, ErrorSeverity, Outcome};
pub use id::{
    AnnotationId, AssetId, AuditEventId, BackupId, CollectionId, InstallationId, NotebookDesignId,
    NotebookId, OcrRunId, PageId, PageSetId, PhysicalCopyId, ReviewItemId, ScanId, SkillId,
    SkillRunId, SmartPageId, TextCorrectionId, TextRegionId,
};
