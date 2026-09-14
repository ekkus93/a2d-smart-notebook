-- Migration 0010: index detected OCR text for local search.
--
-- Search is local-first and derived from persisted OCR rows. Only explicit Detected outcomes are
-- indexed; successful no-text OCR and unavailable/cancelled provider outcomes intentionally do
-- not create searchable fake text.

CREATE VIRTUAL TABLE ocr_search_index USING fts5(
    page_id UNINDEXED,
    scan_id UNINDEXED,
    ocr_run_id UNINDEXED,
    text_region_id UNINDEXED,
    document_kind UNINDEXED,
    body
);

INSERT INTO ocr_search_index (page_id, scan_id, ocr_run_id, text_region_id, document_kind, body)
SELECT scans.page_id, ocr_runs.scan_id, ocr_runs.id, NULL, 'FullText', ocr_runs.full_text
FROM ocr_runs
JOIN scans ON scans.id = ocr_runs.scan_id
WHERE ocr_runs.status = 'Detected'
  AND length(ocr_runs.full_text) > 0;

INSERT INTO ocr_search_index (page_id, scan_id, ocr_run_id, text_region_id, document_kind, body)
SELECT scans.page_id, ocr_runs.scan_id, ocr_runs.id, text_regions.id, 'TextRegion', text_regions.text
FROM text_regions
JOIN ocr_runs ON ocr_runs.id = text_regions.ocr_run_id
JOIN scans ON scans.id = ocr_runs.scan_id
WHERE ocr_runs.status = 'Detected'
  AND length(text_regions.text) > 0;

CREATE TRIGGER ocr_search_index_ocr_runs_after_insert
AFTER INSERT ON ocr_runs
WHEN NEW.status = 'Detected' AND length(NEW.full_text) > 0
BEGIN
    INSERT INTO ocr_search_index (page_id, scan_id, ocr_run_id, text_region_id, document_kind, body)
    SELECT scans.page_id, NEW.scan_id, NEW.id, NULL, 'FullText', NEW.full_text
    FROM scans
    WHERE scans.id = NEW.scan_id;
END;

CREATE TRIGGER ocr_search_index_text_regions_after_insert
AFTER INSERT ON text_regions
WHEN length(NEW.text) > 0
BEGIN
    INSERT INTO ocr_search_index (page_id, scan_id, ocr_run_id, text_region_id, document_kind, body)
    SELECT scans.page_id, ocr_runs.scan_id, ocr_runs.id, NEW.id, 'TextRegion', NEW.text
    FROM ocr_runs
    JOIN scans ON scans.id = ocr_runs.scan_id
    WHERE ocr_runs.id = NEW.ocr_run_id
      AND ocr_runs.status = 'Detected';
END;

CREATE TRIGGER ocr_search_index_ocr_runs_after_delete
AFTER DELETE ON ocr_runs
BEGIN
    DELETE FROM ocr_search_index WHERE ocr_run_id = OLD.id;
END;

CREATE TRIGGER ocr_search_index_text_regions_after_delete
AFTER DELETE ON text_regions
BEGIN
    DELETE FROM ocr_search_index WHERE text_region_id = OLD.id;
END;
