-- Milestone 11 Q1: durable OCR queue/retry orchestration.
--
-- Queue state is Rust-owned. Android may execute a platform OCR provider, but it cannot fabricate
-- queue rows or terminal transitions. Retry/cancellation diagnostics are persisted so process
-- restarts do not erase what happened.

CREATE TABLE ocr_jobs (
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
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0 AND attempt_count <= 25),
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

CREATE UNIQUE INDEX index_ocr_jobs_one_active_work_item
ON ocr_jobs (scan_id, input_asset_id, input_kind)
WHERE status IN ('Queued', 'Running');

CREATE INDEX index_ocr_jobs_claim
ON ocr_jobs (status, next_retry_at_ms, created_at_ms, id);

CREATE INDEX index_ocr_jobs_scan_updated
ON ocr_jobs (scan_id, updated_at_ms DESC, id DESC);
