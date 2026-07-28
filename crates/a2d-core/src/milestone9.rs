//! Milestone 9.1 durable scan registration.
//!
//! Rust reopens the platform staging file, reparses the Page Code, re-runs marker detection and
//! bounded image processing, commits immutable/derived assets, and inserts every database reference
//! in one SQLite transaction. A filesystem journal is created before the first asset commit and is
//! removed only after the database transaction succeeds. If a commit is interrupted, the journal,
//! staging file, and any already-committed assets remain visible for explicit recovery; nothing is
//! silently deleted or reported as saved.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use a2d_domain::{
    A2dError, Asset, AssetKind, AuditEvent, AuditEventId, CaptureSource, ErrorCategory, ErrorCode,
    ErrorSeverity, NotebookId, Page, PageId, PageKind, PageState, QualityStatus, Scan, ScanId,
};
use a2d_identity::PageCode;
use a2d_image::{
    AprilTagDetector, ContrastNormalizationConfig, DerivedImageConfig, DerivedImageLimits,
    DerivedImagePipeline, DetectorConfig, EncodedImage, EncodedImageFormat, EncodedImageLimits,
    GrayQualityMetrics, ImageLimits, ImageRotation, LuminanceMeasurementConfig, MarkerIdLayout,
    OwnedGrayImage, OwnedRgbImage, ProcessingCancellation, RectificationLimits, RectificationPlan,
    RectifiedImageSize, ResolvedPageMarkers, ThumbnailConfig, measure_gray_quality,
    resolve_page_markers,
};
use a2d_layout::{MarkerRole, writable_page_layout};
use a2d_storage::{AssetRepository, AuditEventRepository, PageRepository, ScanRepository};
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage};
use serde_json::json;

use super::{A2dCore, PageResolution, now_ms};

const MAX_ENCODED_BYTES: usize = 24 * 1024 * 1024;
const MAX_DECODED_PIXELS: u64 = 32_000_000;
const MAX_DECODED_BYTES: u64 = 96_000_000;
const CORRECTED_WIDTH: u32 = 900;
const CORRECTED_HEIGHT: u32 = 1_356;
const PIPELINE_VERSION: u32 = 1;
const JOURNAL_DIRECTORY: &str = "asset-commit-journals";
const SCANNER_STAGING_DIRECTORY: &str = "scanner-staging";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanImageFormat {
    Jpeg,
    Png,
}

impl ScanImageFormat {
    fn encoded(self) -> EncodedImageFormat {
        match self {
            Self::Jpeg => EncodedImageFormat::Jpeg,
            Self::Png => EncodedImageFormat::Png,
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanImageRotation {
    Degrees0,
    Degrees90,
    Degrees180,
    Degrees270,
}

impl ScanImageRotation {
    fn image(self) -> ImageRotation {
        match self {
            Self::Degrees0 => ImageRotation::Degrees0,
            Self::Degrees90 => ImageRotation::Degrees90,
            Self::Degrees180 => ImageRotation::Degrees180,
            Self::Degrees270 => ImageRotation::Degrees270,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationMarker {
    pub role: String,
    pub id: u32,
}

#[derive(Clone, Debug)]
pub struct RegisterScanRequest {
    pub staging_path: String,
    pub page_code_payload: String,
    pub expected_page_id: PageId,
    pub active_notebook_id: Option<NotebookId>,
    pub capture_source: CaptureSource,
    pub image_format: ScanImageFormat,
    pub image_rotation: ScanImageRotation,
    pub captured_at_ms: i64,
    pub observed_markers: Vec<RegistrationMarker>,
    pub preview_warnings: Vec<String>,
    pub user_approved: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistrationWarning {
    UnexpectedMarkers,
    LowMarkerConfidence,
    LowFocus,
    TooDark,
    TooMuchDarkArea,
    TooMuchHighlight,
    LocalizedGlare,
    ExistingPageScanRequiresReview,
    AssetCommitJournalCleanupPending,
    StagingCleanupPending,
}

impl RegistrationWarning {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnexpectedMarkers => "UNEXPECTED_MARKERS",
            Self::LowMarkerConfidence => "LOW_MARKER_CONFIDENCE",
            Self::LowFocus => "LOW_FOCUS",
            Self::TooDark => "TOO_DARK",
            Self::TooMuchDarkArea => "TOO_MUCH_DARK_AREA",
            Self::TooMuchHighlight => "TOO_MUCH_HIGHLIGHT",
            Self::LocalizedGlare => "LOCALIZED_GLARE",
            Self::ExistingPageScanRequiresReview => "EXISTING_PAGE_SCAN_REQUIRES_REVIEW",
            Self::AssetCommitJournalCleanupPending => "ASSET_COMMIT_JOURNAL_CLEANUP_PENDING",
            Self::StagingCleanupPending => "STAGING_CLEANUP_PENDING",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationRequiredAction {
    ReviewExistingPage,
    InspectIncompleteAssetCommit,
    RemoveStagingFile,
}

#[derive(Clone, Debug)]
pub struct RegisteredScan {
    pub scan_id: ScanId,
    pub page_id: PageId,
    pub original_asset_id: a2d_domain::AssetId,
    pub corrected_asset_id: a2d_domain::AssetId,
    pub ocr_asset_id: a2d_domain::AssetId,
    pub thumbnail_asset_id: a2d_domain::AssetId,
    pub original_path: String,
    pub corrected_path: String,
    pub ocr_path: String,
    pub thumbnail_path: String,
    pub quality_status: QualityStatus,
    pub preferred: bool,
    pub warnings: Vec<RegistrationWarning>,
    pub required_actions: Vec<RegistrationRequiredAction>,
}

struct StagedCapture {
    canonical_path: PathBuf,
    bytes: Vec<u8>,
}

struct ProcessedCapture {
    corrected_png: Vec<u8>,
    ocr_png: Vec<u8>,
    thumbnail_png: Vec<u8>,
    pipeline_version: u32,
    resolved_markers: ResolvedPageMarkers,
    quality: GrayQualityMetrics,
}

struct RegistrationJournal {
    path: PathBuf,
    file: Option<File>,
}

impl RegistrationJournal {
    fn begin(root: &Path, scan_id: &ScanId, staging_path: &Path) -> Result<Self, A2dError> {
        let directory = root.join("tmp").join(JOURNAL_DIRECTORY);
        std::fs::create_dir_all(&directory)
            .map_err(|error| journal_io_error("creating asset commit journal directory", error))?;
        let path = directory.join(format!("scan-{scan_id}.jsonl"));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| journal_io_error("creating asset commit journal", error))?;
        let mut journal = Self {
            path,
            file: Some(file),
        };
        journal.record(json!({
            "schema_version": 1,
            "operation": "scan_registration",
            "scan_id": scan_id.to_string(),
            "staging_path": staging_path.to_string_lossy(),
            "started_at_ms": now_ms(),
            "phase": "started"
        }))?;
        Ok(journal)
    }

    fn path_string(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    fn record_phase(&mut self, phase: &'static str) -> Result<(), A2dError> {
        self.record(json!({"phase": phase, "at_ms": now_ms()}))
    }

    fn record_asset(&mut self, asset: &Asset) -> Result<(), A2dError> {
        self.record(json!({
            "phase": "asset_committed",
            "asset_id": asset.id().to_string(),
            "kind": format!("{:?}", asset.kind),
            "relative_path": asset.relative_path,
            "sha256": asset.sha256,
            "byte_length": asset.byte_length,
            "at_ms": now_ms()
        }))
    }

    fn record(&mut self, value: serde_json::Value) -> Result<(), A2dError> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| A2dError::internal_unknown("asset commit journal was already closed"))?;
        serde_json::to_writer(&mut *file, &value).map_err(|error| {
            A2dError::new(
                ErrorCode::new("CORE_SCAN_JOURNAL_ENCODING_FAILED"),
                ErrorCategory::Internal,
                ErrorSeverity::Critical,
                "error.core.scan_journal_encoding_failed",
                format!("failed to encode asset commit journal record: {error}"),
                false,
            )
        })?;
        file.write_all(b"\n")
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_data())
            .map_err(|error| journal_io_error("persisting asset commit journal", error))
    }

    fn complete(mut self) -> Result<(), A2dError> {
        self.record(json!({"phase": "database_committed", "at_ms": now_ms()}))?;
        self.file.take();
        std::fs::remove_file(&self.path)
            .map_err(|error| journal_io_error("removing completed asset commit journal", error))
    }
}

fn journal_io_error(context: &'static str, error: std::io::Error) -> A2dError {
    A2dError::new(
        ErrorCode::new("CORE_SCAN_JOURNAL_IO_FAILED"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.core.scan_journal_io_failed",
        format!("{context}: {error}"),
        true,
    )
    .with_detail("context", context)
}

fn registration_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        ErrorSeverity::Error,
        "error.core.scan_registration",
        message.into(),
        false,
    )
}

impl A2dCore {
    pub fn register_scan(&self, request: RegisterScanRequest) -> Result<RegisteredScan, A2dError> {
        if !request.user_approved {
            return Err(registration_error(
                "CORE_SCAN_APPROVAL_REQUIRED",
                ErrorCategory::Validation,
                "single-page registration requires explicit review approval",
            ));
        }
        if request.captured_at_ms <= 0 {
            return Err(registration_error(
                "CORE_SCAN_CAPTURE_TIME_INVALID",
                ErrorCategory::Validation,
                "captured_at_ms must be a positive Unix timestamp",
            ));
        }

        let staged = self.read_staged_capture(
            &request.staging_path,
            request.image_format,
            request.image_rotation,
        )?;
        let page_code = a2d_identity::qr::parse(&request.page_code_payload, |_| true)?;
        let resolution = self.resolve_page_code(
            &request.page_code_payload,
            request.active_notebook_id.as_ref(),
        )?;
        let PageResolution::Resolved {
            page_id,
            notebook_id,
        } = resolution
        else {
            return Err(registration_error(
                "CORE_SCAN_PAGE_IDENTITY_UNRESOLVED",
                ErrorCategory::Identity,
                "the Page Code did not resolve to one existing page",
            ));
        };
        if page_id != request.expected_page_id || notebook_id != request.active_notebook_id {
            return Err(registration_error(
                "CORE_SCAN_PAGE_IDENTITY_CONFLICT",
                ErrorCategory::Identity,
                "the reparsed Page Code does not match the approved page and Notebook",
            )
            .with_detail("expected_page_id", request.expected_page_id.to_string())
            .with_detail("resolved_page_id", page_id.to_string())
            .with_detail(
                "expected_notebook_id",
                request
                    .active_notebook_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "none".to_string()),
            )
            .with_detail(
                "resolved_notebook_id",
                notebook_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "none".to_string()),
            ));
        }

        let processed =
            process_capture(&staged.bytes, request.image_format, request.image_rotation)?;
        validate_observed_markers(&request.observed_markers, &processed.resolved_markers)?;
        let mut warnings = quality_warnings(&processed);
        merge_preview_warnings(&request.preview_warnings, &mut warnings)?;

        let mut storage = self.lock_storage()?;
        let page = storage
            .get_page(&request.expected_page_id)?
            .ok_or_else(|| {
                registration_error(
                    "CORE_SCAN_PAGE_NOT_FOUND",
                    ErrorCategory::Integrity,
                    "the resolved page disappeared before registration",
                )
            })?;
        validate_page_target(&page, &page_code, request.active_notebook_id.as_ref())?;
        if matches!(page.state, PageState::Archived | PageState::Trashed) {
            return Err(registration_error(
                "CORE_SCAN_PAGE_NOT_WRITABLE",
                ErrorCategory::Validation,
                "archived or trashed pages cannot receive a scan",
            )
            .with_detail("page_id", page.id().to_string()));
        }

        let existing_preferred = page.preferred_scan_id.clone();
        if let Some(preferred_scan_id) = existing_preferred.as_ref() {
            let preferred_scan = storage.get_scan(preferred_scan_id)?.ok_or_else(|| {
                registration_error(
                    "CORE_SCAN_PREFERRED_ROW_MISSING",
                    ErrorCategory::Integrity,
                    "page references a preferred scan row that does not exist",
                )
                .with_detail("preferred_scan_id", preferred_scan_id.to_string())
            })?;
            if preferred_scan.page_id != request.expected_page_id || !preferred_scan.preferred {
                return Err(registration_error(
                    "CORE_SCAN_PREFERRED_ROW_INVALID",
                    ErrorCategory::Integrity,
                    "the page's preferred scan reference is internally inconsistent",
                ));
            }
        } else if matches!(page.state, PageState::Scanned | PageState::NeedsReview) {
            return Err(registration_error(
                "CORE_SCAN_PAGE_STATE_INCONSISTENT",
                ErrorCategory::Integrity,
                "page is marked scanned/review without a preferred scan",
            ));
        }

        let preferred = existing_preferred.is_none();
        let quality_status = if preferred {
            if warnings.is_empty() {
                QualityStatus::Accepted
            } else {
                QualityStatus::AcceptedWithWarnings
            }
        } else {
            warnings.insert(RegistrationWarning::ExistingPageScanRequiresReview);
            QualityStatus::NeedsReview
        };

        let scan_id = ScanId::generate();
        let mut journal =
            RegistrationJournal::begin(&self.library_path, &scan_id, &staged.canonical_path)?;
        let journal_path = journal.path_string();

        let asset_result = (|| {
            journal.record_phase("committing_original")?;
            let original = self.asset_store.commit(
                &staged.bytes,
                AssetKind::Original,
                request.image_format.media_type(),
            )?;
            journal.record_asset(&original)?;

            journal.record_phase("committing_corrected")?;
            let corrected = self.asset_store.commit(
                &processed.corrected_png,
                AssetKind::Corrected,
                "image/png",
            )?;
            journal.record_asset(&corrected)?;

            journal.record_phase("committing_ocr_image")?;
            let ocr = self
                .asset_store
                .commit(&processed.ocr_png, AssetKind::Ocr, "image/png")?;
            journal.record_asset(&ocr)?;

            journal.record_phase("committing_thumbnail")?;
            let thumbnail = self.asset_store.commit(
                &processed.thumbnail_png,
                AssetKind::Thumbnail,
                "image/png",
            )?;
            journal.record_asset(&thumbnail)?;
            Ok::<_, A2dError>((original, corrected, ocr, thumbnail))
        })();
        let (original, corrected, ocr, thumbnail) = asset_result.map_err(|error| {
            error
                .with_detail("asset_commit_journal", journal_path.clone())
                .with_detail("staging_path", staged.canonical_path.to_string_lossy())
        })?;
        let resolve_path = |asset: &Asset| {
            self.resolve_asset_path(asset).map_err(|error| {
                error
                    .with_detail("asset_commit_journal", journal_path.clone())
                    .with_detail("staging_path", staged.canonical_path.to_string_lossy())
            })
        };
        let original_path = resolve_path(&original)?;
        let corrected_path = resolve_path(&corrected)?;
        let ocr_path = resolve_path(&ocr)?;
        let thumbnail_path = resolve_path(&thumbnail)?;

        let scan = Scan::new(
            scan_id.clone(),
            request.expected_page_id.clone(),
            None,
            request.capture_source,
            request.captured_at_ms,
            original.id().clone(),
            Some(corrected.id().clone()),
            Some(ocr.id().clone()),
            Some(thumbnail.id().clone()),
            processed.pipeline_version.to_string(),
            quality_status,
            warnings
                .iter()
                .map(|warning| warning.code().to_string())
                .collect(),
            preferred,
            None,
            format!("exact-sha256-v1:{}", corrected.sha256),
        );
        let audit = scan_audit_event(&scan, request.active_notebook_id.as_ref());

        let transaction_result = storage.transaction(|tx| {
            let current_page = tx.get_page(&request.expected_page_id)?.ok_or_else(|| {
                registration_error(
                    "CORE_SCAN_PAGE_NOT_FOUND_IN_TRANSACTION",
                    ErrorCategory::Integrity,
                    "the page disappeared before the registration transaction",
                )
            })?;
            validate_page_target(
                &current_page,
                &page_code,
                request.active_notebook_id.as_ref(),
            )?;
            if current_page.preferred_scan_id != existing_preferred {
                return Err(registration_error(
                    "CORE_SCAN_PAGE_VERSION_CHANGED",
                    ErrorCategory::Integrity,
                    "the preferred scan changed while registration was in progress",
                ));
            }

            tx.insert_asset(&original)?;
            tx.insert_asset(&corrected)?;
            tx.insert_asset(&ocr)?;
            tx.insert_asset(&thumbnail)?;
            tx.insert_scan(&scan)?;
            tx.insert_audit_event(&audit)?;

            let updated_page = tx.get_page(&request.expected_page_id)?.ok_or_else(|| {
                A2dError::internal_unknown("scan registration trigger removed its owning page")
            })?;
            if preferred {
                if updated_page.state != PageState::Scanned
                    || updated_page.preferred_scan_id.as_ref() != Some(scan.id())
                {
                    return Err(A2dError::internal_unknown(
                        "scan registration trigger did not establish the preferred scan",
                    ));
                }
            } else if updated_page.state != PageState::NeedsReview
                || updated_page.preferred_scan_id != existing_preferred
            {
                return Err(A2dError::internal_unknown(
                    "scan registration trigger did not preserve the existing preferred scan",
                ));
            }
            Ok(())
        });
        transaction_result.map_err(|error| {
            error
                .with_detail("asset_commit_journal", journal_path.clone())
                .with_detail("staging_path", staged.canonical_path.to_string_lossy())
                .with_detail(
                    "committed_asset_ids",
                    [original.id(), corrected.id(), ocr.id(), thumbnail.id()]
                        .into_iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                )
        })?;
        drop(storage);

        let mut required_actions = Vec::new();
        if !preferred {
            required_actions.push(RegistrationRequiredAction::ReviewExistingPage);
        }
        if journal.complete().is_err() {
            warnings.insert(RegistrationWarning::AssetCommitJournalCleanupPending);
            required_actions.push(RegistrationRequiredAction::InspectIncompleteAssetCommit);
        }
        if std::fs::remove_file(&staged.canonical_path).is_err() {
            warnings.insert(RegistrationWarning::StagingCleanupPending);
            required_actions.push(RegistrationRequiredAction::RemoveStagingFile);
        }

        Ok(RegisteredScan {
            scan_id,
            page_id: request.expected_page_id,
            original_asset_id: original.id().clone(),
            corrected_asset_id: corrected.id().clone(),
            ocr_asset_id: ocr.id().clone(),
            thumbnail_asset_id: thumbnail.id().clone(),
            original_path,
            corrected_path,
            ocr_path,
            thumbnail_path,
            quality_status,
            preferred,
            warnings: warnings.into_iter().collect(),
            required_actions,
        })
    }

    fn read_staged_capture(
        &self,
        staging_path: &str,
        format: ScanImageFormat,
        rotation: ScanImageRotation,
    ) -> Result<StagedCapture, A2dError> {
        if staging_path.trim().is_empty() {
            return Err(registration_error(
                "CORE_SCAN_STAGING_PATH_EMPTY",
                ErrorCategory::Validation,
                "staging path must not be empty",
            ));
        }
        let staging_root = self
            .library_path
            .join("tmp")
            .join(SCANNER_STAGING_DIRECTORY);
        std::fs::create_dir_all(&staging_root).map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_DIRECTORY_FAILED",
                ErrorCategory::Storage,
                format!("failed to create scanner staging directory: {error}"),
            )
        })?;
        let canonical_root = staging_root.canonicalize().map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_ROOT_INVALID",
                ErrorCategory::Storage,
                format!("failed to canonicalize scanner staging directory: {error}"),
            )
        })?;
        let supplied = PathBuf::from(staging_path);
        let symlink_metadata = std::fs::symlink_metadata(&supplied).map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_FILE_UNAVAILABLE",
                ErrorCategory::Storage,
                format!("staging file is unavailable: {error}"),
            )
            .with_detail("staging_path", staging_path)
        })?;
        if symlink_metadata.file_type().is_symlink() || !symlink_metadata.is_file() {
            return Err(registration_error(
                "CORE_SCAN_STAGING_FILE_INVALID",
                ErrorCategory::Validation,
                "staging path must reference a regular non-symlink file",
            ));
        }
        let canonical_path = supplied.canonicalize().map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_PATH_INVALID",
                ErrorCategory::Storage,
                format!("failed to canonicalize staging path: {error}"),
            )
        })?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(registration_error(
                "CORE_SCAN_STAGING_PATH_ESCAPES_LIBRARY",
                ErrorCategory::Integrity,
                "staging file is outside the Rust-owned scanner staging directory",
            )
            .with_detail("staging_path", canonical_path.to_string_lossy())
            .with_detail("allowed_root", canonical_root.to_string_lossy()));
        }

        let mut file = File::open(&canonical_path).map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_OPEN_FAILED",
                ErrorCategory::Storage,
                format!("failed to open staging file: {error}"),
            )
        })?;
        let before = file.metadata().map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_METADATA_FAILED",
                ErrorCategory::Storage,
                format!("failed to read staging metadata: {error}"),
            )
        })?;
        if before.len() == 0 || before.len() > MAX_ENCODED_BYTES as u64 {
            return Err(registration_error(
                "CORE_SCAN_STAGING_SIZE_INVALID",
                ErrorCategory::Validation,
                format!(
                    "staging image must contain 1..={MAX_ENCODED_BYTES} bytes, got {}",
                    before.len()
                ),
            ));
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        file.read_to_end(&mut bytes).map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_READ_FAILED",
                ErrorCategory::Storage,
                format!("failed to read staging image: {error}"),
            )
        })?;
        let after = file.metadata().map_err(|error| {
            registration_error(
                "CORE_SCAN_STAGING_METADATA_FAILED",
                ErrorCategory::Storage,
                format!("failed to re-read staging metadata: {error}"),
            )
        })?;
        if before.len() != after.len() || bytes.len() as u64 != before.len() {
            return Err(registration_error(
                "CORE_SCAN_STAGING_CHANGED_DURING_READ",
                ErrorCategory::Integrity,
                "staging file changed while Rust was reading it",
            ));
        }
        if let (Ok(before_modified), Ok(after_modified)) = (before.modified(), after.modified())
            && before_modified != after_modified
        {
            return Err(registration_error(
                "CORE_SCAN_STAGING_CHANGED_DURING_READ",
                ErrorCategory::Integrity,
                "staging file modification time changed while Rust was reading it",
            ));
        }
        EncodedImage::new(
            &bytes,
            format.encoded(),
            rotation.image(),
            EncodedImageLimits::new(MAX_ENCODED_BYTES, MAX_DECODED_PIXELS, MAX_DECODED_BYTES)?,
        )?;
        Ok(StagedCapture {
            canonical_path,
            bytes,
        })
    }

    fn resolve_asset_path(&self, asset: &Asset) -> Result<String, A2dError> {
        Ok(self
            .asset_store
            .resolve(&asset.relative_path)?
            .to_string_lossy()
            .into_owned())
    }
}

fn validate_page_target(
    page: &Page,
    code: &PageCode,
    active_notebook_id: Option<&NotebookId>,
) -> Result<(), A2dError> {
    match (&page.kind, code) {
        (
            PageKind::NotebookPage {
                notebook_id,
                design_id,
                logical_page_number,
            },
            PageCode::NotebookPage {
                design_id: code_design,
                logical_page_number: code_number,
                layout_id,
            },
        ) if notebook_id
            == active_notebook_id.ok_or_else(|| {
                registration_error(
                    "CORE_SCAN_NOTEBOOK_REQUIRED",
                    ErrorCategory::Identity,
                    "Notebook page registration requires the confirmed Notebook",
                )
            })?
            && design_id == code_design
            && logical_page_number == code_number
            && &page.layout_id == layout_id =>
        {
            Ok(())
        }
        (
            PageKind::SmartPage {
                smart_page_id,
                page_set_id,
                visible_page_number,
            },
            PageCode::SmartPage {
                smart_page_id: code_id,
                page_set_id: code_set,
                visible_page_number: code_visible,
                layout_id,
            },
        ) if active_notebook_id.is_none()
            && smart_page_id == code_id
            && page_set_id == code_set
            && visible_page_number == code_visible
            && &page.layout_id == layout_id =>
        {
            Ok(())
        }
        _ => Err(registration_error(
            "CORE_SCAN_PAGE_RECORD_CONFLICT",
            ErrorCategory::Integrity,
            "the stored page record does not match the reparsed Page Code",
        )
        .with_detail("page_id", page.id().to_string())),
    }
}

fn process_capture(
    encoded_bytes: &[u8],
    format: ScanImageFormat,
    rotation: ScanImageRotation,
) -> Result<ProcessedCapture, A2dError> {
    let image_limits = ImageLimits::new(MAX_DECODED_PIXELS)?;
    let source = EncodedImage::new(
        encoded_bytes,
        format.encoded(),
        rotation.image(),
        EncodedImageLimits::new(MAX_ENCODED_BYTES, MAX_DECODED_PIXELS, MAX_DECODED_BYTES)?,
    )?
    .decode_rgb8()?;
    let gray = source.clone().into_gray8(image_limits)?;
    let frame = gray.as_frame(image_limits)?;
    let quality = measure_gray_quality(frame, LuminanceMeasurementConfig::new(32, 245, 8, 8)?)?;
    let mut detector = AprilTagDetector::new(DetectorConfig {
        thread_count: 1,
        quad_decimate: 2.0,
        quad_sigma: 0.0,
        refine_edges: true,
        decode_sharpening: 0.25,
        bits_corrected: 2,
    })?;
    let detections = detector.detect(frame)?;
    let marker_layout = MarkerIdLayout::new([
        (0, MarkerRole::TopLeft),
        (1, MarkerRole::TopRight),
        (2, MarkerRole::BottomRight),
        (3, MarkerRole::BottomLeft),
    ])?;
    let resolved_markers = resolve_page_markers(&detections, &marker_layout)?;
    let rectification = RectificationPlan::from_page_markers(
        source.width(),
        source.height(),
        &resolved_markers,
        &writable_page_layout(),
        RectifiedImageSize::new(
            CORRECTED_WIDTH,
            CORRECTED_HEIGHT,
            RectificationLimits::new(2_000_000, 6_000_000)?,
        )?,
    )?;
    let derived = DerivedImagePipeline::new(DerivedImageConfig::new(
        PIPELINE_VERSION,
        ContrastNormalizationConfig::new(10_000, 990_000, 2.0)?,
        None,
        ThumbnailConfig::new(480, 480)?,
        DerivedImageLimits::new(2_000_000, 6_000_000, 12_000_000, 96_000_000)?,
    )?)
    .process(&source, &rectification, &ProcessingCancellation::active())?;

    Ok(ProcessedCapture {
        corrected_png: encode_rgb_png(&derived.corrected_color)?,
        ocr_png: encode_gray_png(&derived.ocr_optimized)?,
        thumbnail_png: encode_rgb_png(&derived.thumbnail)?,
        pipeline_version: derived.provenance.pipeline_version,
        resolved_markers,
        quality,
    })
}

fn encode_rgb_png(image: &OwnedRgbImage) -> Result<Vec<u8>, A2dError> {
    let rgb = RgbImage::from_raw(image.width(), image.height(), image.bytes().to_vec())
        .ok_or_else(|| {
            A2dError::internal_unknown("validated corrected RGB image could not be reconstructed")
        })?;
    encode_dynamic_png(DynamicImage::ImageRgb8(rgb))
}

fn encode_gray_png(image: &OwnedGrayImage) -> Result<Vec<u8>, A2dError> {
    let gray = GrayImage::from_raw(image.width(), image.height(), image.bytes().to_vec())
        .ok_or_else(|| {
            A2dError::internal_unknown("validated OCR grayscale image could not be reconstructed")
        })?;
    encode_dynamic_png(DynamicImage::ImageLuma8(gray))
}

fn encode_dynamic_png(image: DynamicImage) -> Result<Vec<u8>, A2dError> {
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| {
            registration_error(
                "CORE_SCAN_DERIVED_ENCODE_FAILED",
                ErrorCategory::ImageProcessing,
                format!("failed to encode a derived PNG: {error}"),
            )
        })?;
    Ok(output.into_inner())
}

fn validate_observed_markers(
    observed: &[RegistrationMarker],
    resolved: &ResolvedPageMarkers,
) -> Result<(), A2dError> {
    if observed.len() != 4 {
        return Err(registration_error(
            "CORE_SCAN_OBSERVED_MARKERS_INCOMPLETE",
            ErrorCategory::Validation,
            "approved preview must contain exactly four semantic markers",
        ));
    }
    let mut observed_map = BTreeMap::new();
    for marker in observed {
        let role = normalize_marker_role(&marker.role)?;
        if observed_map.insert(role, marker.id).is_some() {
            return Err(registration_error(
                "CORE_SCAN_OBSERVED_MARKER_DUPLICATE",
                ErrorCategory::Validation,
                "approved preview repeats a marker role",
            ));
        }
    }
    let resolved_map = resolved
        .markers
        .iter()
        .map(|marker| (marker.role.as_id_str().to_string(), marker.detection.id))
        .collect::<BTreeMap<_, _>>();
    if observed_map != resolved_map {
        return Err(registration_error(
            "CORE_SCAN_MARKERS_CHANGED_SINCE_REVIEW",
            ErrorCategory::Integrity,
            "Rust marker identities no longer match the approved preview",
        )
        .with_detail("observed", format!("{observed_map:?}"))
        .with_detail("reprocessed", format!("{resolved_map:?}")));
    }
    Ok(())
}

fn normalize_marker_role(value: &str) -> Result<String, A2dError> {
    let normalized = value.trim().to_ascii_uppercase();
    if matches!(normalized.as_str(), "TL" | "TR" | "BR" | "BL") {
        Ok(normalized)
    } else {
        Err(registration_error(
            "CORE_SCAN_MARKER_ROLE_INVALID",
            ErrorCategory::Validation,
            format!("unknown marker role {value:?}"),
        ))
    }
}

fn quality_warnings(processed: &ProcessedCapture) -> BTreeSet<RegistrationWarning> {
    let mut warnings = BTreeSet::new();
    if !processed.resolved_markers.unexpected_tag_ids.is_empty() {
        warnings.insert(RegistrationWarning::UnexpectedMarkers);
    }
    if processed
        .resolved_markers
        .markers
        .iter()
        .any(|marker| marker.detection.decision_margin < 20.0)
    {
        warnings.insert(RegistrationWarning::LowMarkerConfidence);
    }
    let quality = &processed.quality;
    if quality
        .focus
        .is_none_or(|focus| focus.laplacian_variance < 40.0)
    {
        warnings.insert(RegistrationWarning::LowFocus);
    }
    if quality.exposure.mean_luminance < 50.0 {
        warnings.insert(RegistrationWarning::TooDark);
    }
    if quality.exposure.dark_fraction > 0.45 {
        warnings.insert(RegistrationWarning::TooMuchDarkArea);
    }
    if quality.exposure.highlight_fraction > 0.15 {
        warnings.insert(RegistrationWarning::TooMuchHighlight);
    }
    if quality.glare.max_tile_highlight_fraction > 0.35 {
        warnings.insert(RegistrationWarning::LocalizedGlare);
    }
    warnings
}

fn merge_preview_warnings(
    preview_warnings: &[String],
    warnings: &mut BTreeSet<RegistrationWarning>,
) -> Result<(), A2dError> {
    for warning in preview_warnings {
        let typed = match warning.as_str() {
            "UNEXPECTED_MARKERS" => RegistrationWarning::UnexpectedMarkers,
            "LOW_MARKER_CONFIDENCE" => RegistrationWarning::LowMarkerConfidence,
            "LOW_FOCUS" => RegistrationWarning::LowFocus,
            "TOO_DARK" => RegistrationWarning::TooDark,
            "TOO_MUCH_DARK_AREA" => RegistrationWarning::TooMuchDarkArea,
            "TOO_MUCH_HIGHLIGHT" => RegistrationWarning::TooMuchHighlight,
            "LOCALIZED_GLARE" => RegistrationWarning::LocalizedGlare,
            "MISSING_MARKERS" => {
                return Err(registration_error(
                    "CORE_SCAN_PREVIEW_MARKERS_MISSING",
                    ErrorCategory::Integrity,
                    "an approved preview cannot report missing markers",
                ));
            }
            other => {
                return Err(registration_error(
                    "CORE_SCAN_PREVIEW_WARNING_UNKNOWN",
                    ErrorCategory::Validation,
                    format!("unknown preview warning code {other:?}"),
                ));
            }
        };
        warnings.insert(typed);
    }
    Ok(())
}

fn scan_audit_event(scan: &Scan, notebook_id: Option<&NotebookId>) -> AuditEvent {
    let mut details = BTreeMap::new();
    details.insert("page_id".to_string(), scan.page_id.to_string());
    details.insert("scan_id".to_string(), scan.id().to_string());
    details.insert(
        "notebook_id".to_string(),
        notebook_id
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_string()),
    );
    details.insert("preferred".to_string(), scan.preferred.to_string());
    details.insert(
        "quality_status".to_string(),
        format!("{:?}", scan.quality_status),
    );
    AuditEvent::new(
        AuditEventId::generate(),
        now_ms(),
        "scan.registered".to_string(),
        "rust-core".to_string(),
        Some(scan.id().to_string()),
        details,
        None,
    )
}

#[cfg(test)]
#[path = "milestone9_tests.rs"]
mod tests;
