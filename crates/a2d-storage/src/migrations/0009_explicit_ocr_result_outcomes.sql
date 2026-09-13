-- Migration 0009: persist explicit OCR result outcomes.
--
-- Earlier OCR rows could only distinguish provider/version/full_text/warnings. Empty full_text
-- was ambiguous: it could mean a successful no-text result, an unavailable provider, or a failed
-- provider path. Keep the original column for successful detected text, but add terminal outcome
-- metadata so no-text and unavailable OCR are explicit durable states.

ALTER TABLE ocr_runs ADD COLUMN input_asset_id TEXT REFERENCES assets (id);
ALTER TABLE ocr_runs ADD COLUMN status TEXT NOT NULL DEFAULT 'Detected'
    CHECK (status IN ('Detected', 'NoTextDetected', 'Unavailable'));
ALTER TABLE ocr_runs ADD COLUMN model_name TEXT;
ALTER TABLE ocr_runs ADD COLUMN completed_at_ms INTEGER;
ALTER TABLE ocr_runs ADD COLUMN unavailable_reason TEXT
    CHECK (
        unavailable_reason IS NULL OR unavailable_reason IN (
            'ProviderUnavailable',
            'ProviderFailed',
            'ResourceUnavailable',
            'UnsupportedInput',
            'Cancelled'
        )
    );
ALTER TABLE ocr_runs ADD COLUMN unavailable_message TEXT;

UPDATE ocr_runs
SET status = 'NoTextDetected'
WHERE full_text = '';

CREATE INDEX index_ocr_runs_scan_status
ON ocr_runs (scan_id, status);

CREATE TRIGGER ocr_runs_terminal_state_valid_insert
BEFORE INSERT ON ocr_runs
BEGIN
    SELECT RAISE(ABORT, 'A2D_OCR_DETECTED_REQUIRES_TEXT')
    WHERE NEW.status = 'Detected'
      AND length(NEW.full_text) = 0;

    SELECT RAISE(ABORT, 'A2D_OCR_DETECTED_FORBIDS_UNAVAILABLE_DETAIL')
    WHERE NEW.status = 'Detected'
      AND (NEW.unavailable_reason IS NOT NULL OR NEW.unavailable_message IS NOT NULL);

    SELECT RAISE(ABORT, 'A2D_OCR_NO_TEXT_FORBIDS_TEXT')
    WHERE NEW.status = 'NoTextDetected'
      AND length(NEW.full_text) != 0;

    SELECT RAISE(ABORT, 'A2D_OCR_NO_TEXT_FORBIDS_UNAVAILABLE_DETAIL')
    WHERE NEW.status = 'NoTextDetected'
      AND (NEW.unavailable_reason IS NOT NULL OR NEW.unavailable_message IS NOT NULL);

    SELECT RAISE(ABORT, 'A2D_OCR_UNAVAILABLE_FORBIDS_TEXT')
    WHERE NEW.status = 'Unavailable'
      AND length(NEW.full_text) != 0;

    SELECT RAISE(ABORT, 'A2D_OCR_UNAVAILABLE_REQUIRES_REASON')
    WHERE NEW.status = 'Unavailable'
      AND NEW.unavailable_reason IS NULL;
END;

CREATE TRIGGER ocr_runs_terminal_state_valid_update
BEFORE UPDATE OF status, full_text, unavailable_reason, unavailable_message ON ocr_runs
BEGIN
    SELECT RAISE(ABORT, 'A2D_OCR_DETECTED_REQUIRES_TEXT')
    WHERE NEW.status = 'Detected'
      AND length(NEW.full_text) = 0;

    SELECT RAISE(ABORT, 'A2D_OCR_DETECTED_FORBIDS_UNAVAILABLE_DETAIL')
    WHERE NEW.status = 'Detected'
      AND (NEW.unavailable_reason IS NOT NULL OR NEW.unavailable_message IS NOT NULL);

    SELECT RAISE(ABORT, 'A2D_OCR_NO_TEXT_FORBIDS_TEXT')
    WHERE NEW.status = 'NoTextDetected'
      AND length(NEW.full_text) != 0;

    SELECT RAISE(ABORT, 'A2D_OCR_NO_TEXT_FORBIDS_UNAVAILABLE_DETAIL')
    WHERE NEW.status = 'NoTextDetected'
      AND (NEW.unavailable_reason IS NOT NULL OR NEW.unavailable_message IS NOT NULL);

    SELECT RAISE(ABORT, 'A2D_OCR_UNAVAILABLE_FORBIDS_TEXT')
    WHERE NEW.status = 'Unavailable'
      AND length(NEW.full_text) != 0;

    SELECT RAISE(ABORT, 'A2D_OCR_UNAVAILABLE_REQUIRES_REASON')
    WHERE NEW.status = 'Unavailable'
      AND NEW.unavailable_reason IS NULL;
END;
