-- Migration 0007: preserve the selected preferred flag while synchronizing a page pointer.
--
-- Migration 0006 correctly split preferred-flag synchronization into two UPDATE statements to avoid
-- a transient partial-unique-index collision. Its first UPDATE cleared every preferred flag,
-- including the scan now referenced by pages.preferred_scan_id. The guard installed by migration
-- 0005 correctly rejected clearing that selected scan, so first-scan registration and preference
-- changes failed closed with A2D_PREFERRED_SCAN_FLAG_MISMATCH.
--
-- Fix forward: after the page pointer changes, clear only non-selected scans first. Then set the
-- selected scan. This ordering avoids the unique-index collision and keeps every individual flag
-- mutation consistent with the already-authoritative page pointer.

DROP TRIGGER preferred_scan_flags_follow_page_after_update;

CREATE TRIGGER preferred_scan_flags_follow_page_after_update
AFTER UPDATE OF preferred_scan_id ON pages
BEGIN
    UPDATE scans
    SET preferred = 0
    WHERE page_id = NEW.id
      AND preferred <> 0
      AND (
          NEW.preferred_scan_id IS NULL
          OR id <> NEW.preferred_scan_id
      );

    UPDATE scans
    SET preferred = 1
    WHERE NEW.preferred_scan_id IS NOT NULL
      AND id = NEW.preferred_scan_id
      AND page_id = NEW.id
      AND preferred <> 1;
END;
