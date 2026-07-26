-- Migration 0001: initial schema.
--
-- Golden-fixture-style rule for migrations (TODO 3.1): numbered and immutable. Once this file
-- ships, it MUST NOT be edited -- fix forward with a new numbered migration instead.
--
-- Column shapes mirror crates/a2d-domain/src/entities.rs as closely as SQL allows. List/map
-- fields (marker_role_ids, warnings, details, polygon coordinates, permission lists) are stored
-- as JSON-encoded TEXT rather than normalized into join tables -- a deliberate v1 simplification
-- for values that are read/written as a whole and not queried relationally; revisit if a query
-- ever needs to filter on their contents. `provenance_*` columns flatten the embedded
-- a2d_domain::Provenance value object (spec section 15.10) inline rather than a shared table,
-- since Provenance has no independent identity of its own.
--
-- All ids are TEXT (26-char canonical Crockford Base32, a2d-domain::id). All timestamps are
-- INTEGER milliseconds (CLAUDE.md: "Timestamps are stored as *_at_ms INTEGER").

CREATE TABLE notebook_designs (
    id TEXT PRIMARY KEY NOT NULL,
    schema_version INTEGER NOT NULL,
    name TEXT NOT NULL,
    design_version INTEGER NOT NULL,
    trim_width_mm INTEGER NOT NULL,
    trim_height_mm INTEGER NOT NULL,
    logical_page_count INTEGER NOT NULL,
    setup_layout_id TEXT NOT NULL,
    page_layout_id TEXT NOT NULL,
    marker_family TEXT NOT NULL,
    marker_role_ids TEXT NOT NULL,
    manifest_hash TEXT NOT NULL,
    trust_state TEXT NOT NULL
);

CREATE TABLE notebooks (
    id TEXT PRIMARY KEY NOT NULL,
    design_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER,
    active_scan_destination INTEGER NOT NULL,
    optional_color TEXT,
    optional_icon TEXT,
    optional_user_notes TEXT,
    FOREIGN KEY (design_id) REFERENCES notebook_designs (id)
);

CREATE TABLE page_sets (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE assets (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    media_type TEXT NOT NULL,
    byte_length INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    immutable INTEGER NOT NULL,
    encryption_state TEXT NOT NULL
);

-- kind is 'notebook_page' or 'smart_page'; exactly one of the two field groups below is
-- populated, matching a2d_domain::PageKind. preferred_scan_id references scans(id), defined
-- later in this file -- SQLite resolves foreign keys by name at DML time, not at CREATE TABLE
-- time, so the forward reference is fine.
CREATE TABLE pages (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    notebook_id TEXT,
    notebook_design_id TEXT,
    logical_page_number INTEGER,
    smart_page_id TEXT,
    page_set_id TEXT,
    visible_page_number INTEGER,
    layout_id TEXT NOT NULL,
    title TEXT,
    state TEXT NOT NULL,
    preferred_scan_id TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY (notebook_id) REFERENCES notebooks (id),
    FOREIGN KEY (notebook_design_id) REFERENCES notebook_designs (id),
    FOREIGN KEY (page_set_id) REFERENCES page_sets (id),
    FOREIGN KEY (preferred_scan_id) REFERENCES scans (id)
);

-- Enforces TODO 2.3's "Notebook Page requires ... logical page number [uniqueness]" at the only
-- layer that can see the whole table.
CREATE UNIQUE INDEX unique_notebook_logical_page
ON pages (notebook_id, logical_page_number)
WHERE notebook_id IS NOT NULL;

-- Enforces TODO 2.3's "Smart Page requires a UNIQUE Smart Page ID" -- the half of that invariant
-- a2d-domain's Page type could not check by itself (see entities.rs).
CREATE UNIQUE INDEX unique_smart_page_id
ON pages (smart_page_id)
WHERE smart_page_id IS NOT NULL;

CREATE TABLE physical_copies (
    id TEXT PRIMARY KEY NOT NULL,
    page_id TEXT NOT NULL,
    copy_index INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    display_label TEXT,
    FOREIGN KEY (page_id) REFERENCES pages (id)
);

-- Enforces TODO 2.3's "Physical-copy index is unique per page."
CREATE UNIQUE INDEX unique_physical_copy_index
ON physical_copies (page_id, copy_index);

CREATE TABLE scans (
    id TEXT PRIMARY KEY NOT NULL,
    page_id TEXT NOT NULL,
    physical_copy_id TEXT,
    capture_source TEXT NOT NULL,
    captured_at_ms INTEGER NOT NULL,
    original_asset_id TEXT NOT NULL,
    corrected_asset_id TEXT,
    ocr_asset_id TEXT,
    thumbnail_asset_id TEXT,
    pipeline_version TEXT NOT NULL,
    quality_status TEXT NOT NULL,
    warnings TEXT NOT NULL,
    preferred INTEGER NOT NULL,
    supersedes_scan_id TEXT,
    content_fingerprint TEXT NOT NULL,
    FOREIGN KEY (page_id) REFERENCES pages (id),
    FOREIGN KEY (physical_copy_id) REFERENCES physical_copies (id),
    FOREIGN KEY (original_asset_id) REFERENCES assets (id),
    FOREIGN KEY (corrected_asset_id) REFERENCES assets (id),
    FOREIGN KEY (ocr_asset_id) REFERENCES assets (id),
    FOREIGN KEY (thumbnail_asset_id) REFERENCES assets (id),
    FOREIGN KEY (supersedes_scan_id) REFERENCES scans (id)
);

CREATE TABLE collections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

-- Membership is a many-to-many relation, not embedded in `collections` (spec section 15.8:
-- "moving a page between collections MUST NOT change its QR identity").
CREATE TABLE collection_pages (
    collection_id TEXT NOT NULL,
    page_id TEXT NOT NULL,
    added_at_ms INTEGER NOT NULL,
    PRIMARY KEY (collection_id, page_id),
    FOREIGN KEY (collection_id) REFERENCES collections (id),
    FOREIGN KEY (page_id) REFERENCES pages (id)
);

CREATE TABLE review_items (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    page_id TEXT,
    scan_id TEXT,
    severity TEXT NOT NULL,
    status TEXT NOT NULL,
    details TEXT NOT NULL,
    resolution TEXT,
    created_at_ms INTEGER NOT NULL,
    resolved_at_ms INTEGER,
    FOREIGN KEY (page_id) REFERENCES pages (id),
    FOREIGN KEY (scan_id) REFERENCES scans (id)
);

CREATE TABLE ocr_runs (
    id TEXT PRIMARY KEY NOT NULL,
    scan_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    full_text TEXT NOT NULL,
    warnings TEXT NOT NULL,
    provenance_source_page_id TEXT,
    provenance_source_scan_id TEXT,
    provenance_producing_component TEXT NOT NULL,
    provenance_component_version TEXT NOT NULL,
    provenance_created_at_ms INTEGER NOT NULL,
    provenance_warnings TEXT NOT NULL,
    provenance_user_approved INTEGER,
    FOREIGN KEY (scan_id) REFERENCES scans (id)
);

CREATE TABLE text_regions (
    id TEXT PRIMARY KEY NOT NULL,
    ocr_run_id TEXT NOT NULL,
    polygon TEXT NOT NULL,
    text TEXT NOT NULL,
    confidence REAL,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY (ocr_run_id) REFERENCES ocr_runs (id)
);

CREATE TABLE text_corrections (
    id TEXT PRIMARY KEY NOT NULL,
    text_region_id TEXT,
    scan_id TEXT NOT NULL,
    corrected_text TEXT NOT NULL,
    previous_text TEXT,
    provenance_source_page_id TEXT,
    provenance_source_scan_id TEXT,
    provenance_producing_component TEXT NOT NULL,
    provenance_component_version TEXT NOT NULL,
    provenance_created_at_ms INTEGER NOT NULL,
    provenance_warnings TEXT NOT NULL,
    provenance_user_approved INTEGER,
    FOREIGN KEY (text_region_id) REFERENCES text_regions (id),
    FOREIGN KEY (scan_id) REFERENCES scans (id)
);

CREATE TABLE annotations (
    id TEXT PRIMARY KEY NOT NULL,
    page_id TEXT NOT NULL,
    body TEXT NOT NULL,
    region TEXT,
    provenance_source_page_id TEXT,
    provenance_source_scan_id TEXT,
    provenance_producing_component TEXT NOT NULL,
    provenance_component_version TEXT NOT NULL,
    provenance_created_at_ms INTEGER NOT NULL,
    provenance_warnings TEXT NOT NULL,
    provenance_user_approved INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY (page_id) REFERENCES pages (id)
);

CREATE TABLE skill_definitions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    runtime TEXT NOT NULL,
    permissions TEXT NOT NULL,
    model_requirements TEXT NOT NULL,
    network TEXT NOT NULL,
    mutation_policy TEXT NOT NULL,
    manifest_hash TEXT NOT NULL
);

CREATE TABLE skill_runs (
    id TEXT PRIMARY KEY NOT NULL,
    skill_id TEXT NOT NULL,
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    status TEXT NOT NULL,
    granted_permissions TEXT NOT NULL,
    scope_description TEXT NOT NULL,
    provenance_source_page_id TEXT,
    provenance_source_scan_id TEXT,
    provenance_producing_component TEXT NOT NULL,
    provenance_component_version TEXT NOT NULL,
    provenance_created_at_ms INTEGER NOT NULL,
    provenance_warnings TEXT NOT NULL,
    provenance_user_approved INTEGER,
    warnings TEXT NOT NULL,
    FOREIGN KEY (skill_id) REFERENCES skill_definitions (id)
);

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    occurred_at_ms INTEGER NOT NULL,
    event_kind TEXT NOT NULL,
    actor TEXT NOT NULL,
    subject TEXT,
    details TEXT NOT NULL,
    correlation_id TEXT
);

-- No Rust entity exists for this yet (Milestone 2.3's entity list didn't include one; real
-- backup domain logic is Milestone 13's job). Included now, minimally, only because TODO 3.1
-- explicitly lists "backup history" among the initial tables.
CREATE TABLE backup_history (
    id TEXT PRIMARY KEY NOT NULL,
    created_at_ms INTEGER NOT NULL,
    format_version INTEGER NOT NULL,
    encrypted INTEGER NOT NULL,
    verified INTEGER NOT NULL
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
