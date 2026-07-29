-- Migration 0006: make preferred-scan flag synchronization independent of SQLite row update order.
--
-- Migration 0005 used one UPDATE with CASE. With the partial unique index from migration 0004,
-- SQLite could attempt to set the new preferred row to 1 before clearing the old row, producing a
-- transient uniqueness violation. Fix forward: clear all flags first, then set exactly one flag.

DROP TRIGGER preferred_scan_flags_follow_page_after_update;

CREATE TRIGGER preferred_scan_flags_follow_page_after_update
AFTER UPDATE OF preferred_scan_id ON pages
BEGIN
    UPDATE scans
    SET preferred = 0
    WHERE page_id = NEW.id
      AND preferred <> 0;

    UPDATE scans
    SET preferred = 1
    WHERE NEW.preferred_scan_id IS NOT NULL
      AND id = NEW.preferred_scan_id
      AND page_id = NEW.id
      AND preferred <> 1;
END;
