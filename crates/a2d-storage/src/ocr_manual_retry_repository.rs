use a2d_domain::{A2dError, OcrJobId, ScanId};
use rusqlite::OptionalExtension;

use crate::{OcrJobRepository, PersistedOcrJob, Storage, map_rusqlite_error};

impl Storage {
    /// Returns the most recently updated durable OCR job for a scan, including terminal jobs.
    ///
    /// Manual retry is intentionally based on durable queue history rather than reconstructing a
    /// fresh job from Android. This keeps input identity, attempt count, and provider diagnostics
    /// attached to the original queue record across process restart.
    pub fn find_latest_ocr_job_for_scan(
        &self,
        scan_id: &ScanId,
    ) -> Result<Option<PersistedOcrJob>, A2dError> {
        let id = self
            .conn
            .query_row(
                "SELECT id FROM ocr_jobs WHERE scan_id = ?1 \
                 ORDER BY updated_at_ms DESC, created_at_ms DESC, id DESC LIMIT 1",
                [scan_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| map_rusqlite_error("finding latest OCR job for scan", error))?;
        let Some(id) = id else {
            return Ok(None);
        };
        let id = OcrJobId::parse(&id)?;
        OcrJobRepository::get_ocr_job(self, &id)
    }
}
