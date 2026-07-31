//! Typed UniFFI projection for Milestone 9.1 durable scan registration.
//!
//! This boundary parses identifiers and maps enums only. Staging validation, image processing,
//! asset durability, page/version policy, SQLite transactions, and recovery journals remain in the
//! shared Rust core.

use a2d_core as core;
use a2d_domain::{CaptureSource, NotebookId, PageId, QualityStatus};

use super::{A2dClient, A2dFfiError};
use registration_policy_evidence::validate_and_strip_registration_policy_evidence;

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ScanCaptureSource {
    Camera,
    Import,
}

impl From<ScanCaptureSource> for CaptureSource {
    fn from(value: ScanCaptureSource) -> Self {
        match value {
            ScanCaptureSource::Camera => Self::Camera,
            ScanCaptureSource::Import => Self::Import,
        }
    }
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum RegistrationImageFormat {
    Jpeg,
    Png,
}

impl From<RegistrationImageFormat> for core::ScanImageFormat {
    fn from(value: RegistrationImageFormat) -> Self {
        match value {
            RegistrationImageFormat::Jpeg => Self::Jpeg,
            RegistrationImageFormat::Png => Self::Png,
        }
    }
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum RegistrationImageRotation {
    Degrees0,
    Degrees90,
    Degrees180,
    Degrees270,
}

impl From<RegistrationImageRotation> for core::ScanImageRotation {
    fn from(value: RegistrationImageRotation) -> Self {
        match value {
            RegistrationImageRotation::Degrees0 => Self::Degrees0,
            RegistrationImageRotation::Degrees90 => Self::Degrees90,
            RegistrationImageRotation::Degrees180 => Self::Degrees180,
            RegistrationImageRotation::Degrees270 => Self::Degrees270,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct RegistrationMarker {
    pub role: String,
    pub id: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct RegisterScanRequest {
    pub staging_path: String,
    pub page_code_payload: String,
    pub expected_page_id: String,
    pub active_notebook_id: Option<String>,
    pub capture_source: ScanCaptureSource,
    pub image_format: RegistrationImageFormat,
    pub image_rotation: RegistrationImageRotation,
    pub captured_at_ms: i64,
    pub observed_markers: Vec<RegistrationMarker>,
    pub preview_warnings: Vec<String>,
    pub user_approved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum RegisteredScanQualityStatus {
    Accepted,
    AcceptedWithWarnings,
    NeedsReview,
    Rejected,
}

impl From<QualityStatus> for RegisteredScanQualityStatus {
    fn from(value: QualityStatus) -> Self {
        match value {
            QualityStatus::Accepted => Self::Accepted,
            QualityStatus::AcceptedWithWarnings => Self::AcceptedWithWarnings,
            QualityStatus::NeedsReview => Self::NeedsReview,
            QualityStatus::Rejected => Self::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum RegisteredScanWarning {
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

impl From<core::RegistrationWarning> for RegisteredScanWarning {
    fn from(value: core::RegistrationWarning) -> Self {
        match value {
            core::RegistrationWarning::UnexpectedMarkers => Self::UnexpectedMarkers,
            core::RegistrationWarning::LowMarkerConfidence => Self::LowMarkerConfidence,
            core::RegistrationWarning::LowFocus => Self::LowFocus,
            core::RegistrationWarning::TooDark => Self::TooDark,
            core::RegistrationWarning::TooMuchDarkArea => Self::TooMuchDarkArea,
            core::RegistrationWarning::TooMuchHighlight => Self::TooMuchHighlight,
            core::RegistrationWarning::LocalizedGlare => Self::LocalizedGlare,
            core::RegistrationWarning::ExistingPageScanRequiresReview => {
                Self::ExistingPageScanRequiresReview
            }
            core::RegistrationWarning::AssetCommitJournalCleanupPending => {
                Self::AssetCommitJournalCleanupPending
            }
            core::RegistrationWarning::StagingCleanupPending => Self::StagingCleanupPending,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum RegisteredScanRequiredAction {
    ReviewExistingPage,
    InspectIncompleteAssetCommit,
    RemoveStagingFile,
}

impl From<core::RegistrationRequiredAction> for RegisteredScanRequiredAction {
    fn from(value: core::RegistrationRequiredAction) -> Self {
        match value {
            core::RegistrationRequiredAction::ReviewExistingPage => Self::ReviewExistingPage,
            core::RegistrationRequiredAction::InspectIncompleteAssetCommit => {
                Self::InspectIncompleteAssetCommit
            }
            core::RegistrationRequiredAction::RemoveStagingFile => Self::RemoveStagingFile,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct RegisteredScan {
    pub scan_id: String,
    pub page_id: String,
    pub original_asset_id: String,
    pub corrected_asset_id: String,
    pub ocr_asset_id: String,
    pub thumbnail_asset_id: String,
    pub original_path: String,
    pub corrected_path: String,
    pub ocr_path: String,
    pub thumbnail_path: String,
    pub quality_status: RegisteredScanQualityStatus,
    pub preferred: bool,
    pub warnings: Vec<RegisteredScanWarning>,
    pub required_actions: Vec<RegisteredScanRequiredAction>,
}

impl From<core::RegisteredScan> for RegisteredScan {
    fn from(value: core::RegisteredScan) -> Self {
        Self {
            scan_id: value.scan_id.to_string(),
            page_id: value.page_id.to_string(),
            original_asset_id: value.original_asset_id.to_string(),
            corrected_asset_id: value.corrected_asset_id.to_string(),
            ocr_asset_id: value.ocr_asset_id.to_string(),
            thumbnail_asset_id: value.thumbnail_asset_id.to_string(),
            original_path: value.original_path,
            corrected_path: value.corrected_path,
            ocr_path: value.ocr_path,
            thumbnail_path: value.thumbnail_path,
            quality_status: value.quality_status.into(),
            preferred: value.preferred,
            warnings: value.warnings.into_iter().map(Into::into).collect(),
            required_actions: value.required_actions.into_iter().map(Into::into).collect(),
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn register_scan(
        &self,
        request: RegisterScanRequest,
    ) -> Result<RegisteredScan, A2dFfiError> {
        let expected_page_id = PageId::parse(&request.expected_page_id)?;
        let active_notebook_id = request
            .active_notebook_id
            .as_deref()
            .map(NotebookId::parse)
            .transpose()?;
        let preview_warnings = validate_and_strip_registration_policy_evidence(
            &self.core,
            &expected_page_id,
            request.preview_warnings,
        )?;
        self.core
            .register_scan(core::RegisterScanRequest {
                staging_path: request.staging_path,
                page_code_payload: request.page_code_payload,
                expected_page_id,
                active_notebook_id,
                capture_source: request.capture_source.into(),
                image_format: request.image_format.into(),
                image_rotation: request.image_rotation.into(),
                captured_at_ms: request.captured_at_ms,
                observed_markers: request
                    .observed_markers
                    .into_iter()
                    .map(|marker| core::RegistrationMarker {
                        role: marker.role,
                        id: marker.id,
                    })
                    .collect(),
                preview_warnings,
                user_approved: request.user_approved,
            })
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[path = "scan_comparison.rs"]
mod scan_comparison;
pub use scan_comparison::*;

#[path = "policy_preview_processing.rs"]
mod policy_preview_processing;
pub use policy_preview_processing::*;

#[path = "stored_scan_policy_abi.rs"]
mod stored_scan_policy_abi;
pub use stored_scan_policy_abi::*;

#[path = "scanner_recovery.rs"]
mod scanner_recovery;
pub use scanner_recovery::*;

#[path = "registration_policy_evidence.rs"]
mod registration_policy_evidence;
