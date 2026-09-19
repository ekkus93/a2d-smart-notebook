use super::{A2dClient, A2dFfiError, OcrQueueJob};

#[uniffi::export]
impl A2dClient {
    /// Requests the Rust-owned bounded manual retry transition for a scan's latest OCR job.
    pub fn manual_retry_ocr_job_for_scan(
        &self,
        scan_id: String,
    ) -> Result<OcrQueueJob, A2dFfiError> {
        self.core
            .manual_retry_ocr_job_for_scan(&scan_id)
            .map(Into::into)
            .map_err(Into::into)
    }
}
