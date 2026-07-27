-- Migration 0003: Milestone 6 notebook workflow indexes and active-notebook invariant.
--
-- This migration fails closed if a pre-existing library somehow contains more than one active
-- notebook. It does not silently select a winner and erase the ambiguity.

CREATE TABLE milestone6_active_notebook_guard (
    active_count INTEGER NOT NULL CHECK (active_count <= 1)
);
INSERT INTO milestone6_active_notebook_guard (active_count)
SELECT COUNT(*) FROM notebooks WHERE active_scan_destination = 1;
DROP TABLE milestone6_active_notebook_guard;

CREATE UNIQUE INDEX unique_active_scan_destination
ON notebooks (active_scan_destination)
WHERE active_scan_destination = 1;

CREATE INDEX notebooks_by_design_and_archive
ON notebooks (design_id, archived_at_ms, created_at_ms, id);

CREATE INDEX pages_by_notebook_and_logical_number
ON pages (notebook_id, logical_page_number);
