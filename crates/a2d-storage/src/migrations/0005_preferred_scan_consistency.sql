-- Migration 0005: preferred-scan ownership and synchronization.
--
-- Migration 0004 established the first-scan registration trigger and the partial unique index that
-- permits at most one preferred scan per page. This migration closes the remaining invariant gap:
-- pages.preferred_scan_id must identify a preferred scan owned by that same page, and changing the
-- page pointer must update every scan.preferred flag for the page in the same SQLite statement.
--
-- Existing contradictory state is an integrity failure. The migration deliberately fails closed
-- rather than selecting a winner or silently rewriting user history.

CREATE TABLE milestone9_preferred_scan_consistency_guard (
    invalid_count INTEGER NOT NULL CHECK (invalid_count = 0)
);

INSERT INTO milestone9_preferred_scan_consistency_guard (invalid_count)
SELECT
    (
        SELECT COUNT(*)
        FROM pages AS page
        WHERE page.preferred_scan_id IS NOT NULL
          AND NOT EXISTS (
              SELECT 1
              FROM scans AS scan
              WHERE scan.id = page.preferred_scan_id
                AND scan.page_id = page.id
                AND scan.preferred = 1
          )
    )
    +
    (
        SELECT COUNT(*)
        FROM scans AS scan
        WHERE scan.preferred = 1
          AND NOT EXISTS (
              SELECT 1
              FROM pages AS page
              WHERE page.id = scan.page_id
                AND page.preferred_scan_id = scan.id
          )
    );

DROP TABLE milestone9_preferred_scan_consistency_guard;

CREATE TRIGGER preferred_scan_page_ownership_before_update
BEFORE UPDATE OF preferred_scan_id ON pages
WHEN NEW.preferred_scan_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'A2D_PREFERRED_SCAN_PAGE_MISMATCH')
    WHERE NOT EXISTS (
        SELECT 1
        FROM scans AS scan
        WHERE scan.id = NEW.preferred_scan_id
          AND scan.page_id = NEW.id
    );
END;

CREATE TRIGGER preferred_scan_flags_follow_page_after_update
AFTER UPDATE OF preferred_scan_id ON pages
BEGIN
    UPDATE scans
    SET preferred = CASE
            WHEN NEW.preferred_scan_id IS NOT NULL AND id = NEW.preferred_scan_id THEN 1
            ELSE 0
        END
    WHERE page_id = NEW.id;
END;

-- The page pointer is authoritative for an explicit preference change. Prevent internal callers
-- from changing one scan flag independently and reintroducing contradictory state. The AFTER UPDATE
-- trigger above remains allowed because the page pointer has already changed before it updates the
-- corresponding scan rows.
CREATE TRIGGER preferred_scan_flag_update_requires_page_pointer
BEFORE UPDATE OF preferred ON scans
WHEN NEW.preferred <> OLD.preferred
BEGIN
    SELECT RAISE(ABORT, 'A2D_PREFERRED_SCAN_FLAG_MISMATCH')
    WHERE (
        NEW.preferred = 1
        AND NOT EXISTS (
            SELECT 1
            FROM pages AS page
            WHERE page.id = NEW.page_id
              AND page.preferred_scan_id = NEW.id
        )
    ) OR (
        NEW.preferred = 0
        AND EXISTS (
            SELECT 1
            FROM pages AS page
            WHERE page.id = OLD.page_id
              AND page.preferred_scan_id = OLD.id
        )
    );
END;
