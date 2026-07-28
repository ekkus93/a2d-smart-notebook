-- Migration 0004: durable scan registration invariants.
--
-- Numbered migrations are immutable once shipped. This migration makes the scan row the atomic
-- registration event: inserting a preferred first scan updates the owning page to Scanned and
-- records its preferred scan; inserting a nonpreferred NeedsReview version preserves the existing
-- preferred scan while moving the page into NeedsReview. The partial unique index prevents two
-- scans from ever claiming preferred status for the same page.

CREATE UNIQUE INDEX one_preferred_scan_per_page
ON scans (page_id)
WHERE preferred = 1;

CREATE TRIGGER register_scan_updates_page
AFTER INSERT ON scans
BEGIN
    UPDATE pages
    SET state = CASE
            WHEN NEW.quality_status = 'NeedsReview' THEN 'NeedsReview'
            ELSE 'Scanned'
        END,
        preferred_scan_id = CASE
            WHEN NEW.preferred = 1 THEN NEW.id
            ELSE preferred_scan_id
        END,
        updated_at_ms = CASE
            WHEN updated_at_ms < NEW.captured_at_ms THEN NEW.captured_at_ms
            ELSE updated_at_ms
        END
    WHERE id = NEW.page_id;
END;
