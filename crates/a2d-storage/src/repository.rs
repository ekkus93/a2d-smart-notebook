//! Repository traits (TODO 3.2). SQL stays private to this module (and this crate) — every
//! method here takes and returns typed `a2d_domain` values, never raw rows or query strings.
//!
//! Implemented directly on `rusqlite::Connection` rather than a custom wrapper type, so the same
//! method call works both outside a transaction (`storage.insert_page(...)`, since `Storage`
//! re-implements each trait by delegating to its connection) and inside one
//! (`tx.insert_page(...)`, since `rusqlite::Transaction` derefs to `Connection` and Rust's
//! method resolution follows that deref automatically).
//!
//! Scoped to the entities TODO 3.2's example and TODO 3.3's asset protocol actually need
//! (notebook creation, page sets, scan registration, OCR runs, audit events, assets) rather than
//! all 18 tables — the rest get a repository when the milestone that needs them (11, 12, 13, 14)
//! arrives, matching this project's "don't build ahead of need" norm.

use a2d_domain::{
    A2dError, Asset, AssetId, AuditEvent, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity,
    LayoutId, Notebook, NotebookDesign, NotebookDesignId, NotebookId, OcrRun, OcrRunId, Page,
    PageId, PageKind, PageSet, PageSetId, PageState, Scan, ScanId, TrimSizeMm, TrustState,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::json_columns::{decode_json, encode_json};

fn map_sql_error(context: &str, err: rusqlite::Error) -> A2dError {
    if let rusqlite::Error::SqliteFailure(ffi_err, ref msg) = err
        && ffi_err.code == rusqlite::ErrorCode::ConstraintViolation
    {
        // 1555 (SQLITE_CONSTRAINT_PRIMARYKEY) is split out from the generic unique-constraint
        // case: every `id` column in this schema is the table's primary key (TODO 4.1 "detect
        // persistence collisions as hard integrity events"), and every ID is a locally generated
        // 128-bit value (`a2d_domain::id`). A primary-key collision therefore does not mean "the
        // caller supplied a duplicate business value" (that's 2067, e.g.
        // `unique_notebook_logical_page`) — it means a freshly generated ID already exists in the
        // table, which should never happen at 128 bits of entropy and indicates either a broken
        // RNG or a reused/replayed ID. That is an integrity event, not a validation error, so it
        // gets its own code/category/severity rather than being folded into
        // `STORAGE_UNIQUE_CONSTRAINT_VIOLATION`.
        if ffi_err.extended_code == 1555 {
            return A2dError::new(
                ErrorCode::new("STORAGE_ID_COLLISION"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.id_collision",
                format!("{context}: {}", msg.clone().unwrap_or_default()),
                false,
            )
            .with_detail("context", context);
        }
        let code = match ffi_err.extended_code {
            2067 /* SQLITE_CONSTRAINT_UNIQUE */ => "STORAGE_UNIQUE_CONSTRAINT_VIOLATION",
            787 /* SQLITE_CONSTRAINT_FOREIGNKEY */ => "STORAGE_FOREIGN_KEY_VIOLATION",
            1299 /* SQLITE_CONSTRAINT_NOTNULL */ => "STORAGE_NOT_NULL_VIOLATION",
            _ => "STORAGE_CONSTRAINT_VIOLATION",
        };
        return A2dError::new(
            ErrorCode::new(code),
            ErrorCategory::Validation,
            ErrorSeverity::Error,
            "error.storage.constraint",
            format!("{context}: {}", msg.clone().unwrap_or_default()),
            false,
        )
        .with_detail("context", context);
    }
    A2dError::new(
        ErrorCode::new("STORAGE_SQLITE_ERROR"),
        ErrorCategory::Storage,
        ErrorSeverity::Error,
        "error.storage.sqlite",
        format!("{context}: {err}"),
        false,
    )
    .with_detail("context", context)
}

fn layout_id(raw: String) -> Result<LayoutId, A2dError> {
    LayoutId::parse(&raw)
}

// ---------------------------------------------------------------------------------------------
// NotebookDesign (read-only here: designs are created by Milestone 4/5's manifest resolution,
// not by this crate)
// ---------------------------------------------------------------------------------------------

pub trait NotebookDesignRepository {
    fn insert_notebook_design(&self, design: &NotebookDesign) -> Result<(), A2dError>;
    fn get_notebook_design(
        &self,
        id: &NotebookDesignId,
    ) -> Result<Option<NotebookDesign>, A2dError>;
}

impl NotebookDesignRepository for Connection {
    fn insert_notebook_design(&self, design: &NotebookDesign) -> Result<(), A2dError> {
        let marker_role_ids = encode_json(&design.marker_role_ids, "marker_role_ids")?;
        self.execute(
            "INSERT INTO notebook_designs (id, schema_version, name, design_version, \
             trim_width_mm, trim_height_mm, logical_page_count, setup_layout_id, \
             page_layout_id, marker_family, marker_role_ids, manifest_hash, trust_state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                design.id().to_string(),
                design.schema_version,
                design.name,
                design.design_version,
                design.trim_size.width,
                design.trim_size.height,
                design.logical_page_count,
                design.setup_layout_id.as_str(),
                design.page_layout_id.as_str(),
                design.marker_family,
                marker_role_ids,
                design.manifest_hash,
                trust_state_to_str(design.trust_state),
            ],
        )
        .map_err(|e| map_sql_error("insert_notebook_design", e))?;
        Ok(())
    }

    fn get_notebook_design(
        &self,
        id: &NotebookDesignId,
    ) -> Result<Option<NotebookDesign>, A2dError> {
        self.query_row(
            "SELECT id, schema_version, name, design_version, trim_width_mm, trim_height_mm, \
             logical_page_count, setup_layout_id, page_layout_id, marker_family, \
             marker_role_ids, manifest_hash, trust_state FROM notebook_designs WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, u32>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_notebook_design", e))?
        .map(
            |(
                id,
                schema_version,
                name,
                design_version,
                width,
                height,
                logical_page_count,
                setup_layout_id,
                page_layout_id,
                marker_family,
                marker_role_ids,
                manifest_hash,
                trust_state,
            )| {
                Ok(NotebookDesign::new(
                    NotebookDesignId::parse(&id)?,
                    schema_version,
                    name,
                    design_version,
                    TrimSizeMm { width, height },
                    logical_page_count,
                    layout_id(setup_layout_id)?,
                    layout_id(page_layout_id)?,
                    marker_family,
                    decode_json(&marker_role_ids, "marker_role_ids")?,
                    manifest_hash,
                    trust_state_from_str(&trust_state)?,
                ))
            },
        )
        .transpose()
    }
}

fn trust_state_to_str(state: TrustState) -> &'static str {
    match state {
        TrustState::Unverified => "Unverified",
        TrustState::Trusted => "Trusted",
        TrustState::Revoked => "Revoked",
    }
}

fn trust_state_from_str(raw: &str) -> Result<TrustState, A2dError> {
    match raw {
        "Unverified" => Ok(TrustState::Unverified),
        "Trusted" => Ok(TrustState::Trusted),
        "Revoked" => Ok(TrustState::Revoked),
        other => Err(corrupt_enum_error("trust_state", other)),
    }
}

fn corrupt_enum_error(column: &str, raw: &str) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_CORRUPT_ENUM_COLUMN"),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.corrupt_enum_column",
        format!("column `{column}` has value `{raw}` which is not a known enum variant"),
        false,
    )
    .with_detail("column", column)
    .with_detail("raw", raw)
}

// ---------------------------------------------------------------------------------------------
// Notebook
// ---------------------------------------------------------------------------------------------

pub trait NotebookRepository {
    fn insert_notebook(&self, notebook: &Notebook) -> Result<(), A2dError>;
    fn get_notebook(&self, id: &NotebookId) -> Result<Option<Notebook>, A2dError>;
}

impl NotebookRepository for Connection {
    fn insert_notebook(&self, notebook: &Notebook) -> Result<(), A2dError> {
        self.execute(
            "INSERT INTO notebooks (id, design_id, display_name, created_at_ms, updated_at_ms, \
             archived_at_ms, active_scan_destination, optional_color, optional_icon, \
             optional_user_notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                notebook.id().to_string(),
                notebook.design_id.to_string(),
                notebook.display_name,
                notebook.created_at_ms,
                notebook.updated_at_ms,
                notebook.archived_at_ms,
                notebook.active_scan_destination,
                notebook.optional_color,
                notebook.optional_icon,
                notebook.optional_user_notes,
            ],
        )
        .map_err(|e| map_sql_error("insert_notebook", e))?;
        Ok(())
    }

    fn get_notebook(&self, id: &NotebookId) -> Result<Option<Notebook>, A2dError> {
        self.query_row(
            "SELECT id, design_id, display_name, created_at_ms, updated_at_ms, archived_at_ms, \
             active_scan_destination, optional_color, optional_icon, optional_user_notes \
             FROM notebooks WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_notebook", e))?
        .map(
            |(
                id,
                design_id,
                display_name,
                created_at_ms,
                updated_at_ms,
                archived_at_ms,
                active_scan_destination,
                optional_color,
                optional_icon,
                optional_user_notes,
            )| {
                Ok(Notebook::new(
                    NotebookId::parse(&id)?,
                    NotebookDesignId::parse(&design_id)?,
                    display_name,
                    created_at_ms,
                    updated_at_ms,
                    archived_at_ms,
                    active_scan_destination,
                    optional_color,
                    optional_icon,
                    optional_user_notes,
                ))
            },
        )
        .transpose()
    }
}

// ---------------------------------------------------------------------------------------------
// PageSet
// ---------------------------------------------------------------------------------------------

pub trait PageSetRepository {
    fn insert_page_set(&self, page_set: &PageSet) -> Result<(), A2dError>;
    fn get_page_set(&self, id: &PageSetId) -> Result<Option<PageSet>, A2dError>;
}

impl PageSetRepository for Connection {
    fn insert_page_set(&self, page_set: &PageSet) -> Result<(), A2dError> {
        self.execute(
            "INSERT INTO page_sets (id, title, created_at_ms) VALUES (?1, ?2, ?3)",
            params![
                page_set.id().to_string(),
                page_set.title,
                page_set.created_at_ms
            ],
        )
        .map_err(|e| map_sql_error("insert_page_set", e))?;
        Ok(())
    }

    fn get_page_set(&self, id: &PageSetId) -> Result<Option<PageSet>, A2dError> {
        self.query_row(
            "SELECT id, title, created_at_ms FROM page_sets WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_page_set", e))?
        .map(|(id, title, created_at_ms)| {
            Ok(PageSet::new(PageSetId::parse(&id)?, title, created_at_ms))
        })
        .transpose()
    }
}

// ---------------------------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------------------------

pub trait PageRepository {
    fn insert_page(&self, page: &Page) -> Result<(), A2dError>;
    fn get_page(&self, id: &PageId) -> Result<Option<Page>, A2dError>;
    fn set_preferred_scan(&self, page_id: &PageId, scan_id: &ScanId) -> Result<(), A2dError>;
    fn set_generated_pdf_asset(&self, page_id: &PageId, asset_id: &AssetId)
    -> Result<(), A2dError>;
}

impl PageRepository for Connection {
    fn insert_page(&self, page: &Page) -> Result<(), A2dError> {
        let (
            kind,
            notebook_id,
            notebook_design_id,
            logical_page_number,
            smart_page_id,
            page_set_id,
            visible_page_number,
        ) = match &page.kind {
            PageKind::NotebookPage {
                notebook_id,
                design_id,
                logical_page_number,
            } => (
                "notebook_page",
                Some(notebook_id.to_string()),
                Some(design_id.to_string()),
                Some(*logical_page_number),
                None,
                None,
                None,
            ),
            PageKind::SmartPage {
                smart_page_id,
                page_set_id,
                visible_page_number,
            } => (
                "smart_page",
                None,
                None,
                None,
                Some(smart_page_id.to_string()),
                page_set_id.as_ref().map(ToString::to_string),
                *visible_page_number,
            ),
        };
        self.execute(
            "INSERT INTO pages (id, kind, notebook_id, notebook_design_id, \
             logical_page_number, smart_page_id, page_set_id, visible_page_number, layout_id, \
             title, state, preferred_scan_id, generated_pdf_asset_id, created_at_ms, \
             updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                page.id().to_string(),
                kind,
                notebook_id,
                notebook_design_id,
                logical_page_number,
                smart_page_id,
                page_set_id,
                visible_page_number,
                page.layout_id.as_str(),
                page.title,
                page_state_to_str(page.state),
                page.preferred_scan_id.as_ref().map(ToString::to_string),
                page.generated_pdf_asset_id
                    .as_ref()
                    .map(ToString::to_string),
                page.created_at_ms,
                page.updated_at_ms,
            ],
        )
        .map_err(|e| map_sql_error("insert_page", e))?;
        Ok(())
    }

    fn get_page(&self, id: &PageId) -> Result<Option<Page>, A2dError> {
        self.query_row(
            "SELECT id, kind, notebook_id, notebook_design_id, logical_page_number, \
             smart_page_id, page_set_id, visible_page_number, layout_id, title, state, \
             preferred_scan_id, generated_pdf_asset_id, created_at_ms, updated_at_ms FROM pages \
             WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<u32>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<u32>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, i64>(14)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_page", e))?
        .map(
            |(
                id,
                kind,
                notebook_id,
                notebook_design_id,
                logical_page_number,
                smart_page_id,
                page_set_id,
                visible_page_number,
                layout_id_raw,
                title,
                state,
                preferred_scan_id,
                generated_pdf_asset_id,
                created_at_ms,
                updated_at_ms,
            )| {
                let page_kind = match kind.as_str() {
                    "notebook_page" => PageKind::NotebookPage {
                        notebook_id: NotebookId::parse(&require(notebook_id, "notebook_id")?)?,
                        design_id: NotebookDesignId::parse(&require(
                            notebook_design_id,
                            "notebook_design_id",
                        )?)?,
                        logical_page_number: require_copy(
                            logical_page_number,
                            "logical_page_number",
                        )?,
                    },
                    "smart_page" => PageKind::SmartPage {
                        smart_page_id: SmartPageIdAlias::parse(&require(
                            smart_page_id,
                            "smart_page_id",
                        )?)?,
                        page_set_id: page_set_id.map(|s| PageSetId::parse(&s)).transpose()?,
                        visible_page_number,
                    },
                    other => return Err(corrupt_enum_error("pages.kind", other)),
                };
                Ok(Page::from_stored(
                    PageId::parse(&id)?,
                    page_kind,
                    layout_id(layout_id_raw)?,
                    title,
                    page_state_from_str(&state)?,
                    preferred_scan_id.map(|s| ScanId::parse(&s)).transpose()?,
                    generated_pdf_asset_id
                        .map(|s| AssetId::parse(&s))
                        .transpose()?,
                    created_at_ms,
                    updated_at_ms,
                ))
            },
        )
        .transpose()
    }

    fn set_preferred_scan(&self, page_id: &PageId, scan_id: &ScanId) -> Result<(), A2dError> {
        let changed = self
            .execute(
                "UPDATE pages SET preferred_scan_id = ?1 WHERE id = ?2",
                params![scan_id.to_string(), page_id.to_string()],
            )
            .map_err(|e| map_sql_error("set_preferred_scan", e))?;
        if changed == 0 {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_PAGE_NOT_FOUND"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.storage.page_not_found",
                "set_preferred_scan: no page with this id",
                false,
            )
            .with_detail("page_id", page_id.to_string()));
        }
        Ok(())
    }

    fn set_generated_pdf_asset(
        &self,
        page_id: &PageId,
        asset_id: &AssetId,
    ) -> Result<(), A2dError> {
        let requested = asset_id.to_string();
        let changed = self
            .execute(
                "UPDATE pages SET generated_pdf_asset_id = ?1 \
                 WHERE id = ?2 \
                   AND (generated_pdf_asset_id IS NULL OR generated_pdf_asset_id = ?1)",
                params![requested, page_id.to_string()],
            )
            .map_err(|e| map_sql_error("set_generated_pdf_asset", e))?;
        if changed != 0 {
            return Ok(());
        }

        let existing: Option<Option<String>> = self
            .query_row(
                "SELECT generated_pdf_asset_id FROM pages WHERE id = ?1",
                [page_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| {
                map_sql_error("reading generated_pdf_asset_id after failed assignment", e)
            })?;

        match existing {
            None => Err(A2dError::new(
                ErrorCode::new("STORAGE_PAGE_NOT_FOUND"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.storage.page_not_found",
                "set_generated_pdf_asset: no page with this id",
                false,
            )
            .with_detail("page_id", page_id.to_string())),
            Some(Some(existing_asset_id)) if existing_asset_id == asset_id.to_string() => Ok(()),
            Some(Some(existing_asset_id)) => Err(A2dError::new(
                ErrorCode::new("STORAGE_GENERATED_PDF_ASSET_CONFLICT"),
                ErrorCategory::Integrity,
                ErrorSeverity::Error,
                "error.storage.generated_pdf_asset_conflict",
                "generated PDF asset is already assigned and cannot be replaced implicitly",
                false,
            )
            .with_detail("page_id", page_id.to_string())
            .with_detail("existing_asset_id", existing_asset_id)
            .with_detail("requested_asset_id", asset_id.to_string())),
            Some(None) => Err(A2dError::internal_unknown(
                "set_generated_pdf_asset matched an existing unassigned page but SQLite reported zero changed rows",
            )
            .with_detail("page_id", page_id.to_string())),
        }
    }
}

// SmartPageId lives in a2d_domain; imported under an alias only to keep the match arm above
// readable next to the similarly-named `smart_page_id` local variable.
use a2d_domain::SmartPageId as SmartPageIdAlias;

fn require(value: Option<String>, column: &str) -> Result<String, A2dError> {
    value.ok_or_else(|| missing_column_error(column))
}

fn require_copy<T>(value: Option<T>, column: &str) -> Result<T, A2dError> {
    value.ok_or_else(|| missing_column_error(column))
}

fn missing_column_error(column: &str) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_MISSING_REQUIRED_COLUMN"),
        ErrorCategory::Integrity,
        ErrorSeverity::Critical,
        "error.storage.missing_required_column",
        format!("column `{column}` was NULL but this page kind requires it"),
        false,
    )
    .with_detail("column", column)
}

fn page_state_to_str(state: PageState) -> &'static str {
    match state {
        PageState::GeneratedNotScanned => "GeneratedNotScanned",
        PageState::Scanned => "Scanned",
        PageState::NeedsReview => "NeedsReview",
        PageState::Archived => "Archived",
        PageState::Trashed => "Trashed",
    }
}

fn page_state_from_str(raw: &str) -> Result<PageState, A2dError> {
    match raw {
        "GeneratedNotScanned" => Ok(PageState::GeneratedNotScanned),
        "Scanned" => Ok(PageState::Scanned),
        "NeedsReview" => Ok(PageState::NeedsReview),
        "Archived" => Ok(PageState::Archived),
        "Trashed" => Ok(PageState::Trashed),
        other => Err(corrupt_enum_error("pages.state", other)),
    }
}

// ---------------------------------------------------------------------------------------------
// Asset
// ---------------------------------------------------------------------------------------------

pub trait AssetRepository {
    fn insert_asset(&self, asset: &Asset) -> Result<(), A2dError>;
    fn get_asset(&self, id: &AssetId) -> Result<Option<Asset>, A2dError>;
}

impl AssetRepository for Connection {
    fn insert_asset(&self, asset: &Asset) -> Result<(), A2dError> {
        self.execute(
            "INSERT INTO assets (id, kind, relative_path, media_type, byte_length, sha256, \
             created_at_ms, immutable, encryption_state) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                asset.id().to_string(),
                asset_kind_to_str(asset.kind),
                asset.relative_path,
                asset.media_type,
                asset.byte_length as i64,
                asset.sha256,
                asset.created_at_ms,
                asset.immutable,
                encryption_state_to_str(asset.encryption_state),
            ],
        )
        .map_err(|e| map_sql_error("insert_asset", e))?;
        Ok(())
    }

    fn get_asset(&self, id: &AssetId) -> Result<Option<Asset>, A2dError> {
        self.query_row(
            "SELECT id, kind, relative_path, media_type, byte_length, sha256, created_at_ms, \
             immutable, encryption_state FROM assets WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, bool>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_asset", e))?
        .map(
            |(
                id,
                kind,
                relative_path,
                media_type,
                byte_length,
                sha256,
                created_at_ms,
                immutable,
                encryption_state,
            )| {
                Ok(Asset::new(
                    AssetId::parse(&id)?,
                    asset_kind_from_str(&kind)?,
                    relative_path,
                    media_type,
                    byte_length as u64,
                    sha256,
                    created_at_ms,
                    immutable,
                    encryption_state_from_str(&encryption_state)?,
                ))
            },
        )
        .transpose()
    }
}

fn asset_kind_to_str(kind: a2d_domain::AssetKind) -> &'static str {
    use a2d_domain::AssetKind::*;
    match kind {
        Original => "Original",
        Corrected => "Corrected",
        Ocr => "Ocr",
        Thumbnail => "Thumbnail",
        Export => "Export",
    }
}

fn asset_kind_from_str(raw: &str) -> Result<a2d_domain::AssetKind, A2dError> {
    use a2d_domain::AssetKind::*;
    match raw {
        "Original" => Ok(Original),
        "Corrected" => Ok(Corrected),
        "Ocr" => Ok(Ocr),
        "Thumbnail" => Ok(Thumbnail),
        "Export" => Ok(Export),
        other => Err(corrupt_enum_error("assets.kind", other)),
    }
}

fn encryption_state_to_str(state: a2d_domain::EncryptionState) -> &'static str {
    use a2d_domain::EncryptionState::*;
    match state {
        Plaintext => "Plaintext",
        Encrypted => "Encrypted",
    }
}

fn encryption_state_from_str(raw: &str) -> Result<a2d_domain::EncryptionState, A2dError> {
    use a2d_domain::EncryptionState::*;
    match raw {
        "Plaintext" => Ok(Plaintext),
        "Encrypted" => Ok(Encrypted),
        other => Err(corrupt_enum_error("assets.encryption_state", other)),
    }
}

// ---------------------------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------------------------

pub trait ScanRepository {
    fn insert_scan(&self, scan: &Scan) -> Result<(), A2dError>;
    fn get_scan(&self, id: &ScanId) -> Result<Option<Scan>, A2dError>;
}

impl ScanRepository for Connection {
    fn insert_scan(&self, scan: &Scan) -> Result<(), A2dError> {
        // TODO 2.3's "a scan always references an immutable original asset" -- the half of that
        // invariant a2d-domain's Scan type couldn't check by itself (it only has the id, not the
        // referenced row). Checked here, not at read time, so an unenforced invariant can never
        // land in the database in the first place.
        let original_immutable: Option<bool> = self
            .query_row(
                "SELECT immutable FROM assets WHERE id = ?1",
                [scan.original_asset_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| map_sql_error("insert_scan (checking original asset)", e))?;
        match original_immutable {
            None => {
                return Err(A2dError::new(
                    ErrorCode::new("STORAGE_SCAN_ORIGINAL_ASSET_MISSING"),
                    ErrorCategory::Validation,
                    ErrorSeverity::Error,
                    "error.storage.scan_original_asset_missing",
                    "a scan's original_asset_id must reference an asset that already exists",
                    false,
                )
                .with_detail("original_asset_id", scan.original_asset_id.to_string()));
            }
            Some(false) => {
                return Err(A2dError::new(
                    ErrorCode::new("STORAGE_SCAN_ORIGINAL_ASSET_NOT_IMMUTABLE"),
                    ErrorCategory::Validation,
                    ErrorSeverity::Error,
                    "error.storage.scan_original_asset_not_immutable",
                    "a scan's original_asset_id must reference an asset marked immutable \
                     (spec sections 3.3 and 16.3)",
                    false,
                )
                .with_detail("original_asset_id", scan.original_asset_id.to_string()));
            }
            Some(true) => {}
        }

        let warnings = encode_json(&scan.warnings, "warnings")?;
        self.execute(
            "INSERT INTO scans (id, page_id, physical_copy_id, capture_source, captured_at_ms, \
             original_asset_id, corrected_asset_id, ocr_asset_id, thumbnail_asset_id, \
             pipeline_version, quality_status, warnings, preferred, supersedes_scan_id, \
             content_fingerprint) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                scan.id().to_string(),
                scan.page_id.to_string(),
                scan.physical_copy_id.as_ref().map(ToString::to_string),
                capture_source_to_str(scan.capture_source),
                scan.captured_at_ms,
                scan.original_asset_id.to_string(),
                scan.corrected_asset_id.as_ref().map(ToString::to_string),
                scan.ocr_asset_id.as_ref().map(ToString::to_string),
                scan.thumbnail_asset_id.as_ref().map(ToString::to_string),
                scan.pipeline_version,
                quality_status_to_str(scan.quality_status),
                warnings,
                scan.preferred,
                scan.supersedes_scan_id.as_ref().map(ToString::to_string),
                scan.content_fingerprint,
            ],
        )
        .map_err(|e| map_sql_error("insert_scan", e))?;
        Ok(())
    }

    fn get_scan(&self, id: &ScanId) -> Result<Option<Scan>, A2dError> {
        self.query_row(
            "SELECT id, page_id, physical_copy_id, capture_source, captured_at_ms, \
             original_asset_id, corrected_asset_id, ocr_asset_id, thumbnail_asset_id, \
             pipeline_version, quality_status, warnings, preferred, supersedes_scan_id, \
             content_fingerprint FROM scans WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, bool>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, String>(14)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_scan", e))?
        .map(
            |(
                id,
                page_id,
                physical_copy_id,
                capture_source,
                captured_at_ms,
                original_asset_id,
                corrected_asset_id,
                ocr_asset_id,
                thumbnail_asset_id,
                pipeline_version,
                quality_status,
                warnings,
                preferred,
                supersedes_scan_id,
                content_fingerprint,
            )| {
                Ok(Scan::new(
                    ScanId::parse(&id)?,
                    PageId::parse(&page_id)?,
                    physical_copy_id
                        .map(|s| a2d_domain::PhysicalCopyId::parse(&s))
                        .transpose()?,
                    capture_source_from_str(&capture_source)?,
                    captured_at_ms,
                    AssetId::parse(&original_asset_id)?,
                    corrected_asset_id.map(|s| AssetId::parse(&s)).transpose()?,
                    ocr_asset_id.map(|s| AssetId::parse(&s)).transpose()?,
                    thumbnail_asset_id.map(|s| AssetId::parse(&s)).transpose()?,
                    pipeline_version,
                    quality_status_from_str(&quality_status)?,
                    decode_json(&warnings, "warnings")?,
                    preferred,
                    supersedes_scan_id.map(|s| ScanId::parse(&s)).transpose()?,
                    content_fingerprint,
                ))
            },
        )
        .transpose()
    }
}

fn capture_source_to_str(source: a2d_domain::CaptureSource) -> &'static str {
    use a2d_domain::CaptureSource::*;
    match source {
        Camera => "Camera",
        Import => "Import",
    }
}

fn capture_source_from_str(raw: &str) -> Result<a2d_domain::CaptureSource, A2dError> {
    use a2d_domain::CaptureSource::*;
    match raw {
        "Camera" => Ok(Camera),
        "Import" => Ok(Import),
        other => Err(corrupt_enum_error("scans.capture_source", other)),
    }
}

fn quality_status_to_str(status: a2d_domain::QualityStatus) -> &'static str {
    use a2d_domain::QualityStatus::*;
    match status {
        Accepted => "Accepted",
        AcceptedWithWarnings => "AcceptedWithWarnings",
        NeedsReview => "NeedsReview",
        Rejected => "Rejected",
    }
}

fn quality_status_from_str(raw: &str) -> Result<a2d_domain::QualityStatus, A2dError> {
    use a2d_domain::QualityStatus::*;
    match raw {
        "Accepted" => Ok(Accepted),
        "AcceptedWithWarnings" => Ok(AcceptedWithWarnings),
        "NeedsReview" => Ok(NeedsReview),
        "Rejected" => Ok(Rejected),
        other => Err(corrupt_enum_error("scans.quality_status", other)),
    }
}

// ---------------------------------------------------------------------------------------------
// OcrRun
// ---------------------------------------------------------------------------------------------

pub trait OcrRunRepository {
    fn insert_ocr_run(&self, run: &OcrRun) -> Result<(), A2dError>;
    fn get_ocr_run(&self, id: &OcrRunId) -> Result<Option<OcrRun>, A2dError>;
}

impl OcrRunRepository for Connection {
    fn insert_ocr_run(&self, run: &OcrRun) -> Result<(), A2dError> {
        let warnings = encode_json(&run.warnings, "warnings")?;
        let provenance_warnings = encode_json(&run.provenance.warnings, "provenance_warnings")?;
        self.execute(
            "INSERT INTO ocr_runs (id, scan_id, provider, provider_version, full_text, \
             warnings, provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                run.id().to_string(),
                run.scan_id.to_string(),
                run.provider,
                run.provider_version,
                run.full_text,
                warnings,
                run.provenance
                    .source_page_id
                    .as_ref()
                    .map(ToString::to_string),
                run.provenance
                    .source_scan_id
                    .as_ref()
                    .map(ToString::to_string),
                run.provenance.producing_component,
                run.provenance.component_version,
                run.provenance.created_at_ms,
                provenance_warnings,
                run.provenance.user_approved,
            ],
        )
        .map_err(|e| map_sql_error("insert_ocr_run", e))?;
        Ok(())
    }

    fn get_ocr_run(&self, id: &OcrRunId) -> Result<Option<OcrRun>, A2dError> {
        self.query_row(
            "SELECT id, scan_id, provider, provider_version, full_text, warnings, \
             provenance_source_page_id, provenance_source_scan_id, \
             provenance_producing_component, provenance_component_version, \
             provenance_created_at_ms, provenance_warnings, provenance_user_approved \
             FROM ocr_runs WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<bool>>(12)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_ocr_run", e))?
        .map(
            |(
                id,
                scan_id,
                provider,
                provider_version,
                full_text,
                warnings,
                prov_source_page_id,
                prov_source_scan_id,
                prov_producing_component,
                prov_component_version,
                prov_created_at_ms,
                prov_warnings,
                prov_user_approved,
            )| {
                Ok(OcrRun::new(
                    OcrRunId::parse(&id)?,
                    ScanId::parse(&scan_id)?,
                    provider,
                    provider_version,
                    full_text,
                    decode_json(&warnings, "warnings")?,
                    a2d_domain::Provenance {
                        source_page_id: prov_source_page_id
                            .map(|s| PageId::parse(&s))
                            .transpose()?,
                        source_scan_id: prov_source_scan_id
                            .map(|s| ScanId::parse(&s))
                            .transpose()?,
                        producing_component: prov_producing_component,
                        component_version: prov_component_version,
                        created_at_ms: prov_created_at_ms,
                        warnings: decode_json(&prov_warnings, "provenance_warnings")?,
                        user_approved: prov_user_approved,
                    },
                ))
            },
        )
        .transpose()
    }
}

// ---------------------------------------------------------------------------------------------
// AuditEvent
// ---------------------------------------------------------------------------------------------

pub trait AuditEventRepository {
    fn insert_audit_event(&self, event: &AuditEvent) -> Result<(), A2dError>;
    fn get_audit_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>, A2dError>;
}

impl AuditEventRepository for Connection {
    fn insert_audit_event(&self, event: &AuditEvent) -> Result<(), A2dError> {
        let details = encode_json(&event.details, "details")?;
        self.execute(
            "INSERT INTO audit_events (id, occurred_at_ms, event_kind, actor, subject, \
             details, correlation_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.id().to_string(),
                event.occurred_at_ms,
                event.event_kind,
                event.actor,
                event.subject,
                details,
                event.correlation_id,
            ],
        )
        .map_err(|e| map_sql_error("insert_audit_event", e))?;
        Ok(())
    }

    fn get_audit_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>, A2dError> {
        self.query_row(
            "SELECT id, occurred_at_ms, event_kind, actor, subject, details, correlation_id \
             FROM audit_events WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sql_error("get_audit_event", e))?
        .map(
            |(id, occurred_at_ms, event_kind, actor, subject, details, correlation_id)| {
                Ok(AuditEvent::new(
                    AuditEventId::parse(&id)?,
                    occurred_at_ms,
                    event_kind,
                    actor,
                    subject,
                    decode_json(&details, "details")?,
                    correlation_id,
                ))
            },
        )
        .transpose()
    }
}
