//! Typed UniFFI projection for Milestone 8.5 durable batch scanning.
//!
//! Rust owns batch authorization, fixed Notebook identity, duplicate-page detection, recovery
//! reconciliation, review-item production, and session completion semantics.

use a2d_core as core;
use a2d_domain::{NotebookId, PageId};

use super::{
    registration_policy_evidence::validate_and_strip_registration_policy_evidence, A2dClient,
    A2dFfiError, RegisterScanRequest, RegisteredScan,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum BatchScanEntryStatus {
    Queued,
    Saved,
    NeedsReview,
}

impl From<core::BatchScanEntryStatus> for BatchScanEntryStatus {
    fn from(value: core::BatchScanEntryStatus) -> Self {
        match value {
            core::BatchScanEntryStatus::Queued => Self::Queued,
            core::BatchScanEntryStatus::Saved => Self::Saved,
            core::BatchScanEntryStatus::NeedsReview => Self::NeedsReview,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum BatchScanReviewReason {
    IdentityFailure,
    ProcessingFailure,
    RegistrationFailure,
}

impl From<BatchScanReviewReason> for core::BatchScanReviewReason {
    fn from(value: BatchScanReviewReason) -> Self {
        match value {
            BatchScanReviewReason::IdentityFailure => Self::IdentityFailure,
            BatchScanReviewReason::ProcessingFailure => Self::ProcessingFailure,
            BatchScanReviewReason::RegistrationFailure => Self::RegistrationFailure,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BeginBatchScanSessionRequest {
    pub session_id: String,
    pub notebook_id: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BatchScanEntry {
    pub recovery_token: String,
    pub page_id: String,
    pub captured_at_ms: i64,
    pub status: BatchScanEntryStatus,
    pub registered_scan_id: Option<String>,
    pub duplicate_page: bool,
    pub review_item_id: Option<String>,
    pub message: Option<String>,
}

impl From<core::BatchScanEntry> for BatchScanEntry {
    fn from(value: core::BatchScanEntry) -> Self {
        Self {
            recovery_token: value.recovery_token,
            page_id: value.page_id.to_string(),
            captured_at_ms: value.captured_at_ms,
            status: value.status.into(),
            registered_scan_id: value.registered_scan_id.map(|id| id.to_string()),
            duplicate_page: value.duplicate_page,
            review_item_id: value.review_item_id.map(|id| id.to_string()),
            message: value.message,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BatchScanSession {
    pub session_id: String,
    pub notebook_id: String,
    pub started_at_ms: i64,
    pub completed_at_ms: Option<i64>,
    pub entries: Vec<BatchScanEntry>,
    pub queued_count: u32,
    pub saved_count: u32,
    pub review_count: u32,
}

impl From<core::BatchScanSession> for BatchScanSession {
    fn from(value: core::BatchScanSession) -> Self {
        let queued_count = value.queued_count();
        let saved_count = value.saved_count();
        let review_count = value.review_count();
        Self {
            session_id: value.session_id,
            notebook_id: value.notebook_id.to_string(),
            started_at_ms: value.started_at_ms,
            completed_at_ms: value.completed_at_ms,
            entries: value.entries.into_iter().map(Into::into).collect(),
            queued_count,
            saved_count,
            review_count,
        }
    }
}

fn to_core_register_scan_request(
    client: &A2dClient,
    request: RegisterScanRequest,
) -> Result<core::RegisterScanRequest, A2dFfiError> {
    let expected_page_id = PageId::parse(&request.expected_page_id)?;
    let active_notebook_id = request
        .active_notebook_id
        .as_deref()
        .map(NotebookId::parse)
        .transpose()?;
    let preview_warnings = validate_and_strip_registration_policy_evidence(
        &client.core,
        &expected_page_id,
        request.preview_warnings,
    )?;
    Ok(core::RegisterScanRequest {
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
        recovery_token: request.recovery_token,
        user_approved: request.user_approved,
    })
}

#[uniffi::export]
impl A2dClient {
    pub fn begin_batch_scan_session(
        &self,
        request: BeginBatchScanSessionRequest,
    ) -> Result<BatchScanSession, A2dFfiError> {
        self.core
            .begin_batch_scan_session(core::BeginBatchScanSessionRequest {
                session_id: request.session_id,
                notebook_id: NotebookId::parse(&request.notebook_id)?,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn list_batch_scan_sessions(
        &self,
        include_completed: bool,
    ) -> Result<Vec<BatchScanSession>, A2dFfiError> {
        self.core
            .list_batch_scan_sessions(include_completed)
            .map(|sessions| sessions.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub fn queue_batch_scan_capture(
        &self,
        session_id: String,
        recovery_token: String,
    ) -> Result<BatchScanSession, A2dFfiError> {
        self.core
            .queue_batch_scan_capture(&session_id, &recovery_token)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn register_batch_scan(
        &self,
        session_id: String,
        request: RegisterScanRequest,
    ) -> Result<RegisteredScan, A2dFfiError> {
        let request = to_core_register_scan_request(self, request)?;
        self.core
            .register_batch_scan(&session_id, request)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn report_batch_scan_review(
        &self,
        session_id: String,
        recovery_token: String,
        reason: BatchScanReviewReason,
        message: String,
    ) -> Result<BatchScanSession, A2dFfiError> {
        self.core
            .report_batch_scan_review(&session_id, &recovery_token, reason.into(), message)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn reconcile_batch_scan_session(
        &self,
        session_id: String,
    ) -> Result<BatchScanSession, A2dFfiError> {
        self.core
            .reconcile_batch_scan_session(&session_id)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn complete_batch_scan_session(
        &self,
        session_id: String,
    ) -> Result<BatchScanSession, A2dFfiError> {
        self.core
            .complete_batch_scan_session(&session_id)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn acknowledge_batch_scan_session(
        &self,
        session_id: String,
    ) -> Result<(), A2dFfiError> {
        self.core
            .acknowledge_batch_scan_session(&session_id)
            .map_err(Into::into)
    }
}
