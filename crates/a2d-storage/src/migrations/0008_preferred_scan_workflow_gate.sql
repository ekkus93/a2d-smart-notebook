-- Migration 0008: require an authorized workflow context for preferred-scan pointer changes.
--
-- The schema already enforces ownership, one preferred scan per page, and pointer/flag agreement.
-- This migration closes the audit-bypass gap left by the legacy PageRepository setter: a statement
-- may change pages.preferred_scan_id only while the audited preferred-scan workflow or the atomic
-- first-scan registration trigger owns an exact page/scan operation context.
--
-- The context table is normally empty. Context insertion, pointer mutation, audit insertion, and
-- context removal occur in one transaction, so any failure rolls everything back. A stale context
-- is therefore an integrity defect rather than a reusable authorization token.

CREATE TABLE preferred_scan_mutation_context (
    page_id TEXT PRIMARY KEY NOT NULL,
    scan_id TEXT,
    operation_id TEXT NOT NULL UNIQUE,
    source TEXT NOT NULL CHECK (source IN ('explicit_change', 'scan_registration'))
);

CREATE TRIGGER preferred_scan_pointer_update_requires_workflow
BEFORE UPDATE OF preferred_scan_id ON pages
WHEN NEW.preferred_scan_id IS NOT OLD.preferred_scan_id
BEGIN
    SELECT RAISE(ABORT, 'A2D_PREFERRED_SCAN_WORKFLOW_REQUIRED')
    WHERE NOT EXISTS (
        SELECT 1
        FROM preferred_scan_mutation_context AS context
        WHERE context.page_id = NEW.id
          AND context.scan_id IS NEW.preferred_scan_id
    );
END;

-- First-scan registration is already one Rust-owned transaction containing the scan row, assets,
-- and audit event. Recreate its trigger so the pointer change carries a narrowly scoped context.
DROP TRIGGER register_scan_updates_page;

CREATE TRIGGER register_scan_updates_page
AFTER INSERT ON scans
BEGIN
    INSERT INTO preferred_scan_mutation_context (page_id, scan_id, operation_id, source)
    SELECT NEW.page_id, NEW.id, 'scan-registration:' || NEW.id, 'scan_registration'
    WHERE NEW.preferred = 1;

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

    DELETE FROM preferred_scan_mutation_context
    WHERE page_id = NEW.page_id
      AND operation_id = 'scan-registration:' || NEW.id;
END;
