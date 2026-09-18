-- R7: remove the historical storage-layer OCR attempt ceiling.
--
-- Migration 0011 encoded `attempt_count <= 25` as a defensive storage bound. Durable retry policy
-- is now consolidated in a2d-core, where exactly three claimed automatic attempts are allowed.
-- Storage must persist the counter faithfully, including legacy rows created under migration 0011,
-- without acting as a second retry-policy owner. Migration 0011 is immutable, so fix forward by
-- rebuilding only ocr_jobs without an upper attempt-count CHECK.

CREATE TABLE ocr_jobs_r7 (
    id TEXT PRIMARY KEY,
    scan_id TEXT NOT NULL REFERENCES scans (id),
    input_asset_id TEXT NOT NULL REFERENCES assets (id),
    input_kind TEXT NOT NULL CHECK (input_kind IN ('Original', 'Corrected', 'OcrOptimized')),
    media_type TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    width_px INTEGER NOT NULL CHECK (width_px > 0),
    height_px INTEGER NOT NULL CHECK (height_px > 0),
    status TEXT NOT NULL CHECK (status IN ('Queued', 'Running', 'Recognized', 'Unavailable', 'Cancelled')),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
    last_started_at_ms INTEGER CHECK (last_started_at_ms IS NULL OR last_started_at_ms >= created_at_ms),
    completed_at_ms INTEGER CHECK (completed_at_ms IS NULL OR completed_at_ms >= created_at_ms),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    retryable INTEGER NOT NULL DEFAULT 1 CHECK (retryable IN (0, 1)),
    next_retry_at_ms INTEGER CHECK (next_retry_at_ms IS NULL OR next_retry_at_ms >= 0),
    last_error_code TEXT,
    last_error_message TEXT,
    provider TEXT,
    provider_version TEXT,
    model_name TEXT,
    provider_availability TEXT NOT NULL DEFAULT 'Unknown'
        CHECK (provider_availability IN ('Unknown', 'Available', 'Unavailable', 'Failed')),
    last_ocr_run_id TEXT REFERENCES ocr_runs (id),
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancellation_requested IN (0, 1)),
    CHECK (retryable = 1 OR next_retry_at_ms IS NULL),
    CHECK (
        (status = 'Queued' AND last_started_at_ms IS NULL AND completed_at_ms IS NULL)
        OR (status = 'Running' AND last_started_at_ms IS NOT NULL AND completed_at_ms IS NULL)
        OR (status IN ('Recognized', 'Unavailable', 'Cancelled') AND completed_at_ms IS NOT NULL)
    )
);

INSERT INTO ocr_jobs_r7 (
    id, scan_id, input_asset_id, input_kind, media_type, relative_path, byte_length, width_px,
    height_px, status, created_at_ms, updated_at_ms, last_started_at_ms, completed_at_ms,
    attempt_count, retryable, next_retry_at_ms, last_error_code, last_error_message, provider,
    provider_version, model_name, provider_availability, last_ocr_run_id, cancellation_requested
)
SELECT
    id, scan_id, input_asset_id, input_kind, media_type, relative_path, byte_length, width_px,
    height_px, status, created_at_ms, updated_at_ms, last_started_at_ms, completed_at_ms,
    attempt_count, retryable, next_retry_at_ms, last_error_code, last_error_message, provider,
    provider_version, model_name, provider_availability, last_ocr_run_id, cancellation_requested
FROM ocr_jobs;

DROP TABLE ocr_jobs;
ALTER TABLE ocr_jobs_r7 RENAME TO ocr_jobs;

CREATE UNIQUE INDEX index_ocr_jobs_one_active_work_item
ON ocr_jobs (scan_id, input_asset_id, input_kind)
WHERE status IN ('Queued', 'Running');

CREATE INDEX index_ocr_jobs_claim
ON ocr_jobs (status, next_retry_at_ms, created_at_ms, id);

CREATE INDEX index_ocr_jobs_scan_updated
ON ocr_jobs (scan_id, updated_at_ms DESC, id DESC);
