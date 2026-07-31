//! Typed UniFFI projection for Rust-owned scanner recovery records.

use a2d_core as core;
use a2d_domain::{LayoutId, NotebookId, PageId, ScanId};

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ScannerRecoveryPhase {
    Captured,
    PreviewReady,
    Registering,
    Committed,
}

impl From<core::ScannerRecoveryPhase> for ScannerRecoveryPhase {
    fn from(value: core::ScannerRecoveryPhase) -> Self {
        match value {
            core::ScannerRecoveryPhase::Captured => Self::Captured,
            core::ScannerRecoveryPhase::PreviewReady => Self::PreviewReady,
            core::ScannerRecoveryPhase::Registering => Self::Registering,
            core::ScannerRecoveryPhase::Committed => Self::Committed,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BeginScannerRecoveryRequest {
    pub token: String,
    pub staging_path: String,
    pub page_id: String,
    pub notebook_id: String,
    pub captured_at_ms: i64,
    pub layout_id: String,
    pub processing_policy_version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ScannerRecoveryRecord {
    pub token: String,
    pub staging_path: String,
    pub page_id: String,
    pub notebook_id: String,
    pub captured_at_ms: i64,
    pub layout_id: String,
    pub processing_policy_version: u32,
    pub phase: ScannerRecoveryPhase,
    pub registered_scan_id: Option<String>,
    pub updated_at_ms: i64,
}

impl From<core::ScannerRecoveryRecord> for ScannerRecoveryRecord {
    fn from(value: core::ScannerRecoveryRecord) -> Self {
        Self {
            token: value.token,
            staging_path: value.staging_path,
            page_id: value.page_id.to_string(),
            notebook_id: value.notebook_id.to_string(),
            captured_at_ms: value.captured_at_ms,
            layout_id: value.layout_id.to_string(),
            processing_policy_version: value.processing_policy_version,
            phase: value.phase.into(),
            registered_scan_id: value.registered_scan_id.map(|id| id.to_string()),
            updated_at_ms: value.updated_at_ms,
        }
    }
}

#[uniffi::export]
impl A2dClient {
    pub fn stored_page_code_payload(&self, page_id: String) -> Result<String, A2dFfiError> {
        self.core
            .stored_page_code_payload(&PageId::parse(&page_id)?)
            .map_err(Into::into)
    }

    pub fn begin_scanner_recovery(
        &self,
        request: BeginScannerRecoveryRequest,
    ) -> Result<ScannerRecoveryRecord, A2dFfiError> {
        self.core
            .begin_scanner_recovery(core::BeginScannerRecoveryRequest {
                token: request.token,
                staging_path: request.staging_path,
                page_id: PageId::parse(&request.page_id)?,
                notebook_id: NotebookId::parse(&request.notebook_id)?,
                captured_at_ms: request.captured_at_ms,
                layout_id: LayoutId::parse(&request.layout_id)?,
                processing_policy_version: request.processing_policy_version,
            })
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn list_scanner_recoveries(&self) -> Result<Vec<ScannerRecoveryRecord>, A2dFfiError> {
        self.core
            .list_scanner_recoveries()
            .map(|records| records.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub fn mark_scanner_recovery_preview_ready(
        &self,
        token: String,
    ) -> Result<ScannerRecoveryRecord, A2dFfiError> {
        self.core
            .mark_scanner_recovery_preview_ready(&token)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn reconcile_scanner_recovery(
        &self,
        token: String,
    ) -> Result<ScannerRecoveryRecord, A2dFfiError> {
        self.core
            .reconcile_scanner_recovery(&token)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn discard_scanner_recovery(&self, token: String) -> Result<(), A2dFfiError> {
        self.core
            .discard_scanner_recovery(&token)
            .map_err(Into::into)
    }

    pub fn acknowledge_committed_scanner_recovery(
        &self,
        token: String,
        scan_id: String,
    ) -> Result<(), A2dFfiError> {
        self.core
            .acknowledge_committed_scanner_recovery(&token, &ScanId::parse(&scan_id)?)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::OpenLibraryRequest;

    #[test]
    fn ffi_recovery_records_round_trip_without_platform_business_rules() {
        let root =
            std::env::temp_dir().join(format!("a2d-ffi-scanner-recovery-{}", PageId::generate()));
        let client = A2dClient::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let staging = root.join("tmp/scanner-staging/recovery.jpg");
        std::fs::create_dir_all(staging.parent().unwrap()).unwrap();
        std::fs::write(&staging, b"capture").unwrap();
        let page_id = PageId::generate().to_string();
        let notebook_id = NotebookId::generate().to_string();
        let created = client
            .begin_scanner_recovery(BeginScannerRecoveryRequest {
                token: "ffi-recovery".to_string(),
                staging_path: staging.to_string_lossy().into_owned(),
                page_id: page_id.clone(),
                notebook_id: notebook_id.clone(),
                captured_at_ms: 1,
                layout_id: "USLETTER-LINED".to_string(),
                processing_policy_version: 1,
            })
            .unwrap();
        assert_eq!(created.page_id, page_id);
        assert_eq!(created.notebook_id, notebook_id);
        assert_eq!(created.phase, ScannerRecoveryPhase::Captured);
        assert_eq!(client.list_scanner_recoveries().unwrap(), vec![created]);
        client
            .mark_scanner_recovery_preview_ready("ffi-recovery".to_string())
            .unwrap();
        client
            .discard_scanner_recovery("ffi-recovery".to_string())
            .unwrap();
        assert!(!Path::new(&staging).exists());
        assert!(client.list_scanner_recoveries().unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }
}
