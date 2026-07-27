#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str) -> None:
    file = ROOT / path
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match, found {count}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1))


def write(path: str, content: str) -> None:
    file = ROOT / path
    file.parent.mkdir(parents=True, exist_ok=True)
    file.write_text(content)


replace_once(
    "crates/a2d-storage/src/migrations.rs",
    '''    Migration {
        version: 2,
        name: "page_generated_pdf_asset",
        sql: include_str!("migrations/0002_page_generated_pdf_asset.sql"),
    },
];
''',
    '''    Migration {
        version: 2,
        name: "page_generated_pdf_asset",
        sql: include_str!("migrations/0002_page_generated_pdf_asset.sql"),
    },
    Migration {
        version: 3,
        name: "milestone6_notebook_workflows",
        sql: include_str!("migrations/0003_milestone6_notebook_workflows.sql"),
    },
];
''',
)

write(
    "crates/a2d-storage/src/migrations/0003_milestone6_notebook_workflows.sql",
    '''-- Migration 0003: Milestone 6 notebook workflow indexes and active-notebook invariant.
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
''',
)

replace_once(
    "crates/a2d-domain/src/entities.rs",
    '''    pub fn id(&self) -> &NotebookId {
        &self.id
    }
}

/// The two ways a `Page` can be identified''',
    '''    pub fn rename(&mut self, display_name: String, now_ms: i64) -> Result<(), A2dError> {
        let normalized = display_name.trim();
        if normalized.is_empty() || normalized.len() > 200 {
            return Err(A2dError::new(
                ErrorCode::new("NOTEBOOK_DISPLAY_NAME_INVALID"),
                ErrorCategory::Validation,
                ErrorSeverity::Error,
                "error.notebook.display_name_invalid",
                "notebook display name must contain 1-200 non-whitespace UTF-8 bytes",
                false,
            )
            .with_detail("length", normalized.len().to_string()));
        }
        self.display_name = normalized.to_string();
        self.updated_at_ms = now_ms;
        Ok(())
    }

    pub fn archive(&mut self, now_ms: i64) {
        if self.archived_at_ms.is_none() {
            self.archived_at_ms = Some(now_ms);
            self.active_scan_destination = false;
            self.updated_at_ms = now_ms;
        }
    }

    pub fn id(&self) -> &NotebookId {
        &self.id
    }
}

/// The two ways a `Page` can be identified''',
)

replace_once(
    "crates/a2d-domain/src/entities.rs",
    '''pub enum PageState {
    GeneratedNotScanned,
''',
    '''pub enum PageState {
    Unscanned,
    GeneratedNotScanned,
''',
)

replace_once(
    "crates/a2d-storage/src/repository.rs",
    "fn map_sql_error(context: &str, err: rusqlite::Error) -> A2dError {",
    "pub(crate) fn map_sql_error(context: &str, err: rusqlite::Error) -> A2dError {",
)
replace_once(
    "crates/a2d-storage/src/repository.rs",
    '''    match state {
        PageState::GeneratedNotScanned => "GeneratedNotScanned",
''',
    '''    match state {
        PageState::Unscanned => "Unscanned",
        PageState::GeneratedNotScanned => "GeneratedNotScanned",
''',
)
replace_once(
    "crates/a2d-storage/src/repository.rs",
    '''    match raw {
        "GeneratedNotScanned" => Ok(PageState::GeneratedNotScanned),
''',
    '''    match raw {
        "Unscanned" => Ok(PageState::Unscanned),
        "GeneratedNotScanned" => Ok(PageState::GeneratedNotScanned),
''',
)

replace_once(
    "crates/a2d-storage/src/lib.rs",
    '''mod repository;

pub use assets::AssetStore;
''',
    '''mod repository;
mod workflow;

pub use assets::AssetStore;
''',
)
replace_once(
    "crates/a2d-storage/src/lib.rs",
    '''    OcrRunRepository, PageRepository, PageSetRepository, ScanRepository,
};
''',
    '''    OcrRunRepository, PageRepository, PageSetRepository, ScanRepository,
};
pub use workflow::{NotebookWorkflowRepository, PageLookupRepository};
''',
)

write(
    "crates/a2d-storage/src/workflow.rs",
    r'''//! Milestone 6 notebook and page-resolution queries.
//!
//! These operations remain inside `a2d-storage` so SQL never leaks into the service or FFI
//! layers. They supplement the basic CRUD traits with the exact query/update shapes needed by the
//! notebook workflow and deterministic page resolver.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, Notebook, NotebookDesignId, NotebookId,
    Page, PageId, SmartPageId,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::Storage;
use crate::repository::{NotebookRepository, PageRepository, map_sql_error};

pub trait NotebookWorkflowRepository {
    fn update_notebook(&self, notebook: &Notebook) -> Result<(), A2dError>;
    fn list_notebooks(&self, include_archived: bool) -> Result<Vec<Notebook>, A2dError>;
    fn list_notebooks_by_design(
        &self,
        design_id: &NotebookDesignId,
        include_archived: bool,
    ) -> Result<Vec<Notebook>, A2dError>;
    fn set_active_notebook(
        &self,
        notebook_id: Option<&NotebookId>,
        now_ms: i64,
    ) -> Result<(), A2dError>;
    fn get_active_notebook(&self) -> Result<Option<Notebook>, A2dError>;
}

pub trait PageLookupRepository {
    fn find_smart_page(&self, smart_page_id: &SmartPageId) -> Result<Option<Page>, A2dError>;
    fn find_notebook_page(
        &self,
        notebook_id: &NotebookId,
        logical_page_number: u32,
    ) -> Result<Option<Page>, A2dError>;
}

fn notebook_not_found(id: &NotebookId, operation: &str) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORAGE_NOTEBOOK_NOT_FOUND"),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.storage.notebook_not_found",
        format!("{operation}: no notebook with this id"),
        false,
    )
    .with_detail("notebook_id", id.to_string())
}

fn load_notebooks(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    context: &str,
) -> Result<Vec<Notebook>, A2dError> {
    let mut statement = conn.prepare(sql).map_err(|e| map_sql_error(context, e))?;
    let ids = statement
        .query_map(params, |row| row.get::<_, String>(0))
        .map_err(|e| map_sql_error(context, e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sql_error(context, e))?;
    ids.into_iter()
        .map(|raw| {
            let id = NotebookId::parse(&raw)?;
            NotebookRepository::get_notebook(conn, &id)?.ok_or_else(|| {
                A2dError::internal_unknown("notebook disappeared between list and typed load")
                    .with_detail("notebook_id", raw)
            })
        })
        .collect()
}

impl NotebookWorkflowRepository for Connection {
    fn update_notebook(&self, notebook: &Notebook) -> Result<(), A2dError> {
        let changed = self
            .execute(
                "UPDATE notebooks SET display_name = ?1, updated_at_ms = ?2, archived_at_ms = ?3, \
                 active_scan_destination = ?4, optional_color = ?5, optional_icon = ?6, \
                 optional_user_notes = ?7 WHERE id = ?8",
                params![
                    notebook.display_name,
                    notebook.updated_at_ms,
                    notebook.archived_at_ms,
                    notebook.active_scan_destination,
                    notebook.optional_color,
                    notebook.optional_icon,
                    notebook.optional_user_notes,
                    notebook.id().to_string(),
                ],
            )
            .map_err(|e| map_sql_error("update_notebook", e))?;
        if changed == 0 {
            return Err(notebook_not_found(notebook.id(), "update_notebook"));
        }
        Ok(())
    }

    fn list_notebooks(&self, include_archived: bool) -> Result<Vec<Notebook>, A2dError> {
        if include_archived {
            load_notebooks(
                self,
                "SELECT id FROM notebooks ORDER BY active_scan_destination DESC, \
                 (archived_at_ms IS NOT NULL), created_at_ms, id",
                [],
                "list_notebooks",
            )
        } else {
            load_notebooks(
                self,
                "SELECT id FROM notebooks WHERE archived_at_ms IS NULL \
                 ORDER BY active_scan_destination DESC, created_at_ms, id",
                [],
                "list_notebooks",
            )
        }
    }

    fn list_notebooks_by_design(
        &self,
        design_id: &NotebookDesignId,
        include_archived: bool,
    ) -> Result<Vec<Notebook>, A2dError> {
        if include_archived {
            load_notebooks(
                self,
                "SELECT id FROM notebooks WHERE design_id = ?1 \
                 ORDER BY active_scan_destination DESC, (archived_at_ms IS NOT NULL), \
                 created_at_ms, id",
                [design_id.to_string()],
                "list_notebooks_by_design",
            )
        } else {
            load_notebooks(
                self,
                "SELECT id FROM notebooks WHERE design_id = ?1 AND archived_at_ms IS NULL \
                 ORDER BY active_scan_destination DESC, created_at_ms, id",
                [design_id.to_string()],
                "list_notebooks_by_design",
            )
        }
    }

    fn set_active_notebook(
        &self,
        notebook_id: Option<&NotebookId>,
        now_ms: i64,
    ) -> Result<(), A2dError> {
        let Some(notebook_id) = notebook_id else {
            self.execute(
                "UPDATE notebooks SET active_scan_destination = 0, updated_at_ms = CASE \
                 WHEN active_scan_destination = 1 THEN ?1 ELSE updated_at_ms END",
                [now_ms],
            )
            .map_err(|e| map_sql_error("clear_active_notebook", e))?;
            return Ok(());
        };

        let archived_at: Option<Option<i64>> = self
            .query_row(
                "SELECT archived_at_ms FROM notebooks WHERE id = ?1",
                [notebook_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| map_sql_error("checking active notebook candidate", e))?;
        match archived_at {
            None => return Err(notebook_not_found(notebook_id, "set_active_notebook")),
            Some(Some(_)) => {
                return Err(A2dError::new(
                    ErrorCode::new("STORAGE_ARCHIVED_NOTEBOOK_CANNOT_BE_ACTIVE"),
                    ErrorCategory::Validation,
                    ErrorSeverity::Error,
                    "error.storage.archived_notebook_cannot_be_active",
                    "an archived notebook cannot be the active scan destination",
                    false,
                )
                .with_detail("notebook_id", notebook_id.to_string()));
            }
            Some(None) => {}
        }

        self.execute(
            "UPDATE notebooks SET \
             active_scan_destination = CASE WHEN id = ?1 THEN 1 ELSE 0 END, \
             updated_at_ms = CASE \
               WHEN active_scan_destination != CASE WHEN id = ?1 THEN 1 ELSE 0 END \
               THEN ?2 ELSE updated_at_ms END",
            params![notebook_id.to_string(), now_ms],
        )
        .map_err(|e| map_sql_error("set_active_notebook", e))?;
        Ok(())
    }

    fn get_active_notebook(&self) -> Result<Option<Notebook>, A2dError> {
        let mut statement = self
            .prepare("SELECT id FROM notebooks WHERE active_scan_destination = 1 ORDER BY id")
            .map_err(|e| map_sql_error("get_active_notebook", e))?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| map_sql_error("get_active_notebook", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| map_sql_error("get_active_notebook", e))?;
        if ids.len() > 1 {
            return Err(A2dError::new(
                ErrorCode::new("STORAGE_MULTIPLE_ACTIVE_NOTEBOOKS"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.storage.multiple_active_notebooks",
                "more than one notebook is marked as the active scan destination",
                false,
            ));
        }
        ids.into_iter()
            .next()
            .map(|raw| {
                let id = NotebookId::parse(&raw)?;
                NotebookRepository::get_notebook(self, &id)?.ok_or_else(|| {
                    A2dError::internal_unknown("active notebook disappeared during typed load")
                        .with_detail("notebook_id", raw)
                })
            })
            .transpose()
    }
}

impl PageLookupRepository for Connection {
    fn find_smart_page(&self, smart_page_id: &SmartPageId) -> Result<Option<Page>, A2dError> {
        let raw: Option<String> = self
            .query_row(
                "SELECT id FROM pages WHERE smart_page_id = ?1",
                [smart_page_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| map_sql_error("find_smart_page", e))?;
        raw.map(|raw| {
            let id = PageId::parse(&raw)?;
            PageRepository::get_page(self, &id)?.ok_or_else(|| {
                A2dError::internal_unknown("smart page disappeared during typed load")
                    .with_detail("page_id", raw)
            })
        })
        .transpose()
    }

    fn find_notebook_page(
        &self,
        notebook_id: &NotebookId,
        logical_page_number: u32,
    ) -> Result<Option<Page>, A2dError> {
        let raw: Option<String> = self
            .query_row(
                "SELECT id FROM pages WHERE notebook_id = ?1 AND logical_page_number = ?2",
                params![notebook_id.to_string(), logical_page_number],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| map_sql_error("find_notebook_page", e))?;
        raw.map(|raw| {
            let id = PageId::parse(&raw)?;
            PageRepository::get_page(self, &id)?.ok_or_else(|| {
                A2dError::internal_unknown("notebook page disappeared during typed load")
                    .with_detail("page_id", raw)
            })
        })
        .transpose()
    }
}

impl NotebookWorkflowRepository for Storage {
    fn update_notebook(&self, notebook: &Notebook) -> Result<(), A2dError> {
        NotebookWorkflowRepository::update_notebook(&self.conn, notebook)
    }

    fn list_notebooks(&self, include_archived: bool) -> Result<Vec<Notebook>, A2dError> {
        NotebookWorkflowRepository::list_notebooks(&self.conn, include_archived)
    }

    fn list_notebooks_by_design(
        &self,
        design_id: &NotebookDesignId,
        include_archived: bool,
    ) -> Result<Vec<Notebook>, A2dError> {
        NotebookWorkflowRepository::list_notebooks_by_design(
            &self.conn,
            design_id,
            include_archived,
        )
    }

    fn set_active_notebook(
        &self,
        notebook_id: Option<&NotebookId>,
        now_ms: i64,
    ) -> Result<(), A2dError> {
        NotebookWorkflowRepository::set_active_notebook(&self.conn, notebook_id, now_ms)
    }

    fn get_active_notebook(&self) -> Result<Option<Notebook>, A2dError> {
        NotebookWorkflowRepository::get_active_notebook(&self.conn)
    }
}

impl PageLookupRepository for Storage {
    fn find_smart_page(&self, smart_page_id: &SmartPageId) -> Result<Option<Page>, A2dError> {
        PageLookupRepository::find_smart_page(&self.conn, smart_page_id)
    }

    fn find_notebook_page(
        &self,
        notebook_id: &NotebookId,
        logical_page_number: u32,
    ) -> Result<Option<Page>, A2dError> {
        PageLookupRepository::find_notebook_page(
            &self.conn,
            notebook_id,
            logical_page_number,
        )
    }
}
''',
)

replace_once(
    "crates/a2d-core/src/lib.rs",
    '''use a2d_storage::{AssetRepository, AssetStore, PageRepository, PageSetRepository, Storage};

pub struct OpenLibraryRequest {
''',
    '''use a2d_storage::{AssetRepository, AssetStore, PageRepository, PageSetRepository, Storage};

mod milestone6;
pub use milestone6::*;

pub struct OpenLibraryRequest {
''',
)

write(
    "crates/a2d-core/src/milestone6.rs",
    r'''//! Milestone 6 notebook, page-resolution, and Smart Page workflows.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, Notebook, NotebookDesign,
    NotebookDesignId, NotebookId, Page, PageId, PageKind, PageState, SmartPageId, TrustState,
};
use a2d_identity::PageCode;
use a2d_layout::{
    ALL_PAPER_SIZES, ALL_STYLES, PaperSize, SmartPageStyle, bundled_placeholder_registry,
    smart_page_layout,
};
use a2d_storage::{
    AssetRepository, NotebookDesignRepository, NotebookRepository, NotebookWorkflowRepository,
    PageLookupRepository, PageRepository,
};

use super::{A2dCore, now_ms};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotebookSummary {
    pub id: NotebookId,
    pub design_id: NotebookDesignId,
    pub display_name: String,
    pub archived: bool,
    pub active: bool,
}

impl From<&Notebook> for NotebookSummary {
    fn from(value: &Notebook) -> Self {
        Self {
            id: value.id().clone(),
            design_id: value.design_id.clone(),
            display_name: value.display_name.clone(),
            archived: value.archived_at_ms.is_some(),
            active: value.active_scan_destination,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotebookDesignSummary {
    pub id: NotebookDesignId,
    pub name: String,
    pub design_version: u32,
    pub logical_page_count: u32,
    pub trusted: bool,
}

impl From<&NotebookDesign> for NotebookDesignSummary {
    fn from(value: &NotebookDesign) -> Self {
        Self {
            id: value.id().clone(),
            name: value.name.clone(),
            design_version: value.design_version,
            logical_page_count: value.logical_page_count,
            trusted: value.trust_state == TrustState::Trusted,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CreateNotebookRequest {
    pub setup_payload: String,
    pub display_name: String,
    pub optional_color: Option<String>,
    pub optional_icon: Option<String>,
    pub optional_user_notes: Option<String>,
    pub make_active: bool,
}

#[derive(Clone, Debug)]
pub struct CreatedNotebook {
    pub notebook: NotebookSummary,
    pub created_page_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageResolution {
    Resolved {
        page_id: PageId,
        notebook_id: Option<NotebookId>,
    },
    RequiresNotebookSelection {
        candidates: Vec<NotebookSummary>,
    },
    RequiresNotebookRegistration {
        design: NotebookDesignSummary,
    },
    ConflictingActiveNotebook {
        active: NotebookSummary,
        detected_design: NotebookDesignId,
    },
    ImportedUnknownSmartPage {
        smart_page_id: SmartPageId,
        layout_id: LayoutId,
        visible_page_number: Option<u32>,
        page_set_id: Option<a2d_domain::PageSetId>,
    },
    UnsupportedCode {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartPagePaperSize {
    UsLetter,
    A4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartPageContentStyle {
    Blank,
    Lined,
    DotGrid,
    Graph,
}

#[derive(Clone, Debug)]
pub struct SmartPageGenerationRequest {
    pub paper_size: SmartPagePaperSize,
    pub style: SmartPageContentStyle,
    pub page_count: u32,
    pub starting_visible_page: u32,
}

#[derive(Clone, Debug)]
pub struct GeneratedSmartPages {
    pub page_set_id: a2d_domain::PageSetId,
    pub page_ids: Vec<PageId>,
    pub smart_page_ids: Vec<SmartPageId>,
    pub pdf_asset_id: a2d_domain::AssetId,
    pub pdf_path: String,
}

fn workflow_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.core.notebook_workflow",
        message.into(),
        false,
    )
}

fn is_known_layout(layout_id: &LayoutId) -> bool {
    if let Ok(registry) = bundled_placeholder_registry()
        && let Ok(id) = NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX")
        && let Some(design) = registry.resolve(&id)
        && (design.setup_layout_id == *layout_id || design.page_layout_id == *layout_id)
    {
        return true;
    }
    ALL_PAPER_SIZES.into_iter().any(|paper| {
        ALL_STYLES
            .into_iter()
            .any(|style| smart_page_layout(paper, style).id == *layout_id)
    })
}

impl A2dCore {
    fn available_design(&self, id: &NotebookDesignId) -> Result<Option<NotebookDesign>, A2dError> {
        if let Some(stored) = self.lock_storage()?.get_notebook_design(id)? {
            return Ok(Some(stored));
        }
        let registry = bundled_placeholder_registry()?;
        Ok(registry.resolve(id).cloned())
    }

    fn setup_design_from_payload(&self, payload: &str) -> Result<NotebookDesign, A2dError> {
        let parsed = a2d_identity::qr::parse(payload, is_known_layout)?;
        let PageCode::NotebookSetup { design_id } = parsed else {
            return Err(workflow_error(
                "CORE_EXPECTED_NOTEBOOK_SETUP_CODE",
                "the supplied payload is valid A2D data but is not a Notebook Setup Code",
            ));
        };
        let design = self.available_design(&design_id)?.ok_or_else(|| {
            A2dError::new(
                ErrorCode::new("CORE_NOTEBOOK_DESIGN_UNAVAILABLE"),
                ErrorCategory::UnsupportedFormat,
                ErrorSeverity::Error,
                "error.core.notebook_design_unavailable",
                "this build cannot resolve the Notebook Design referenced by the Setup Code",
                false,
            )
            .with_detail("design_id", design_id.to_string())
        })?;
        if design.trust_state == TrustState::Revoked {
            return Err(A2dError::new(
                ErrorCode::new("CORE_NOTEBOOK_DESIGN_REVOKED"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.core.notebook_design_revoked",
                "the referenced Notebook Design is revoked and cannot be registered",
                false,
            )
            .with_detail("design_id", design_id.to_string()));
        }
        Ok(design)
    }

    pub fn resolve_notebook_setup_code(
        &self,
        payload: &str,
    ) -> Result<NotebookDesignSummary, A2dError> {
        let design = self.setup_design_from_payload(payload)?;
        Ok(NotebookDesignSummary::from(&design))
    }

    pub fn create_notebook(
        &self,
        request: CreateNotebookRequest,
    ) -> Result<CreatedNotebook, A2dError> {
        let design = self.setup_design_from_payload(&request.setup_payload)?;
        let created_at = now_ms();
        let mut notebook = Notebook::new(
            NotebookId::generate(),
            design.id().clone(),
            request.display_name,
            created_at,
            created_at,
            None,
            false,
            request.optional_color,
            request.optional_icon,
            request.optional_user_notes,
        );
        notebook.rename(notebook.display_name.clone(), created_at)?;

        let pages: Vec<Page> = (1..=design.logical_page_count)
            .map(|logical_page_number| {
                Page::new(
                    PageId::generate(),
                    PageKind::NotebookPage {
                        notebook_id: notebook.id().clone(),
                        design_id: design.id().clone(),
                        logical_page_number,
                    },
                    design.page_layout_id.clone(),
                    None,
                    PageState::Unscanned,
                    created_at,
                )
            })
            .collect();

        let mut storage = self.lock_storage()?;
        storage.transaction(|tx| {
            if let Some(existing) = tx.get_notebook_design(design.id())? {
                if existing.manifest_hash != design.manifest_hash {
                    return Err(A2dError::new(
                        ErrorCode::new("CORE_NOTEBOOK_DESIGN_CONTENT_CONFLICT"),
                        ErrorCategory::Integrity,
                        ErrorSeverity::Critical,
                        "error.core.notebook_design_content_conflict",
                        "a stored Notebook Design reuses this id with different manifest content",
                        false,
                    )
                    .with_detail("design_id", design.id().to_string()));
                }
            } else {
                tx.insert_notebook_design(&design)?;
            }
            tx.insert_notebook(&notebook)?;
            for page in &pages {
                tx.insert_page(page)?;
            }
            if request.make_active {
                tx.set_active_notebook(Some(notebook.id()), created_at)?;
            }
            Ok(())
        })?;

        if request.make_active {
            notebook.active_scan_destination = true;
        }
        Ok(CreatedNotebook {
            notebook: NotebookSummary::from(&notebook),
            created_page_count: design.logical_page_count,
        })
    }

    pub fn rename_notebook(
        &self,
        notebook_id: &NotebookId,
        display_name: String,
    ) -> Result<NotebookSummary, A2dError> {
        let mut storage = self.lock_storage()?;
        let mut notebook = storage
            .get_notebook(notebook_id)?
            .ok_or_else(|| workflow_error("CORE_NOTEBOOK_NOT_FOUND", "notebook was not found"))?;
        notebook.rename(display_name, now_ms())?;
        storage.update_notebook(&notebook)?;
        Ok(NotebookSummary::from(&notebook))
    }

    pub fn archive_notebook(
        &self,
        notebook_id: &NotebookId,
    ) -> Result<NotebookSummary, A2dError> {
        let mut storage = self.lock_storage()?;
        let mut notebook = storage
            .get_notebook(notebook_id)?
            .ok_or_else(|| workflow_error("CORE_NOTEBOOK_NOT_FOUND", "notebook was not found"))?;
        notebook.archive(now_ms());
        storage.update_notebook(&notebook)?;
        Ok(NotebookSummary::from(&notebook))
    }

    pub fn list_notebooks(&self, include_archived: bool) -> Result<Vec<NotebookSummary>, A2dError> {
        Ok(self
            .lock_storage()?
            .list_notebooks(include_archived)?
            .iter()
            .map(NotebookSummary::from)
            .collect())
    }

    pub fn get_notebook(
        &self,
        notebook_id: &NotebookId,
    ) -> Result<Option<NotebookSummary>, A2dError> {
        Ok(self
            .lock_storage()?
            .get_notebook(notebook_id)?
            .as_ref()
            .map(NotebookSummary::from))
    }

    pub fn set_active_notebook(
        &self,
        notebook_id: Option<&NotebookId>,
    ) -> Result<Option<NotebookSummary>, A2dError> {
        let storage = self.lock_storage()?;
        storage.set_active_notebook(notebook_id, now_ms())?;
        Ok(storage
            .get_active_notebook()?
            .as_ref()
            .map(NotebookSummary::from))
    }

    pub fn get_active_notebook(&self) -> Result<Option<NotebookSummary>, A2dError> {
        Ok(self
            .lock_storage()?
            .get_active_notebook()?
            .as_ref()
            .map(NotebookSummary::from))
    }

    pub fn resolve_page_code(
        &self,
        payload: &str,
        confirmed_notebook_id: Option<&NotebookId>,
    ) -> Result<PageResolution, A2dError> {
        let code = a2d_identity::qr::parse(payload, is_known_layout)?;
        let storage = self.lock_storage()?;
        match code {
            PageCode::NotebookSetup { .. } => Ok(PageResolution::UnsupportedCode {
                reason: "Notebook Setup Codes register notebooks; they do not identify a page"
                    .to_string(),
            }),
            PageCode::SmartPage {
                smart_page_id,
                layout_id,
                visible_page_number,
                page_set_id,
            } => {
                if let Some(page) = storage.find_smart_page(&smart_page_id)? {
                    return Ok(PageResolution::Resolved {
                        page_id: page.id().clone(),
                        notebook_id: None,
                    });
                }
                Ok(PageResolution::ImportedUnknownSmartPage {
                    smart_page_id,
                    layout_id,
                    visible_page_number,
                    page_set_id,
                })
            }
            PageCode::NotebookPage {
                design_id,
                logical_page_number,
                layout_id,
            } => {
                let design = storage
                    .get_notebook_design(&design_id)?
                    .or_else(|| {
                        bundled_placeholder_registry()
                            .ok()
                            .and_then(|registry| registry.resolve(&design_id).cloned())
                    });
                let Some(design) = design else {
                    return Ok(PageResolution::UnsupportedCode {
                        reason: format!("Notebook Design {design_id} is not available in this build"),
                    });
                };
                if logical_page_number == 0 || logical_page_number > design.logical_page_count {
                    return Ok(PageResolution::UnsupportedCode {
                        reason: format!(
                            "logical page {logical_page_number} is outside design range 1-{}",
                            design.logical_page_count
                        ),
                    });
                }
                if layout_id != design.page_layout_id {
                    return Ok(PageResolution::UnsupportedCode {
                        reason: format!(
                            "page layout {layout_id} does not match design layout {}",
                            design.page_layout_id
                        ),
                    });
                }

                let selected = if let Some(confirmed) = confirmed_notebook_id {
                    storage.get_notebook(confirmed)?.ok_or_else(|| {
                        workflow_error(
                            "CORE_CONFIRMED_NOTEBOOK_NOT_FOUND",
                            "the explicitly confirmed notebook no longer exists",
                        )
                        .with_detail("notebook_id", confirmed.to_string())
                    })?
                    .into()
                } else {
                    storage.get_active_notebook()?
                };

                if let Some(selected) = selected {
                    if selected.design_id != design_id {
                        return Ok(PageResolution::ConflictingActiveNotebook {
                            active: NotebookSummary::from(&selected),
                            detected_design: design_id,
                        });
                    }
                    if selected.archived_at_ms.is_some() {
                        return Err(workflow_error(
                            "CORE_ARCHIVED_NOTEBOOK_CANNOT_RECEIVE_SCAN",
                            "an archived notebook cannot receive a page scan",
                        )
                        .with_detail("notebook_id", selected.id().to_string()));
                    }
                    let page = storage
                        .find_notebook_page(selected.id(), logical_page_number)?
                        .ok_or_else(|| {
                            A2dError::new(
                                ErrorCode::new("CORE_NOTEBOOK_PAGE_SLOT_MISSING"),
                                ErrorCategory::Integrity,
                                ErrorSeverity::Critical,
                                "error.core.notebook_page_slot_missing",
                                "registered notebook is missing an expected logical page record",
                                false,
                            )
                            .with_detail("notebook_id", selected.id().to_string())
                            .with_detail("logical_page_number", logical_page_number.to_string())
                        })?;
                    return Ok(PageResolution::Resolved {
                        page_id: page.id().clone(),
                        notebook_id: Some(selected.id().clone()),
                    });
                }

                let candidates = storage.list_notebooks_by_design(&design_id, false)?;
                match candidates.as_slice() {
                    [] => Ok(PageResolution::RequiresNotebookRegistration {
                        design: NotebookDesignSummary::from(&design),
                    }),
                    [only] => {
                        let page = storage
                            .find_notebook_page(only.id(), logical_page_number)?
                            .ok_or_else(|| {
                                A2dError::new(
                                    ErrorCode::new("CORE_NOTEBOOK_PAGE_SLOT_MISSING"),
                                    ErrorCategory::Integrity,
                                    ErrorSeverity::Critical,
                                    "error.core.notebook_page_slot_missing",
                                    "registered notebook is missing an expected logical page record",
                                    false,
                                )
                                .with_detail("notebook_id", only.id().to_string())
                            })?;
                        Ok(PageResolution::Resolved {
                            page_id: page.id().clone(),
                            notebook_id: Some(only.id().clone()),
                        })
                    }
                    _ => Ok(PageResolution::RequiresNotebookSelection {
                        candidates: candidates.iter().map(NotebookSummary::from).collect(),
                    }),
                }
            }
        }
    }

    pub fn generate_smart_pages(
        &self,
        request: SmartPageGenerationRequest,
    ) -> Result<GeneratedSmartPages, A2dError> {
        let paper_size = match request.paper_size {
            SmartPagePaperSize::UsLetter => PaperSize::UsLetter,
            SmartPagePaperSize::A4 => PaperSize::A4,
        };
        let style = match request.style {
            SmartPageContentStyle::Blank => SmartPageStyle::Blank,
            SmartPageContentStyle::Lined => SmartPageStyle::Lined,
            SmartPageContentStyle::DotGrid => SmartPageStyle::DotGrid,
            SmartPageContentStyle::Graph => SmartPageStyle::Graph,
        };
        let registered = self.generate_and_register_page_set(a2d_pdf::GeneratePageSetRequest {
            paper_size,
            style,
            page_count: request.page_count,
            starting_visible_page: request.starting_visible_page,
        })?;
        let asset = self
            .lock_storage()?
            .get_asset(&registered.pdf_asset_id)?
            .ok_or_else(|| {
                A2dError::internal_unknown(
                    "generated PDF asset row disappeared before its path could be returned",
                )
            })?;
        let pdf_path = self
            .asset_store
            .resolve(&asset.relative_path)?
            .to_string_lossy()
            .into_owned();
        Ok(GeneratedSmartPages {
            page_set_id: registered.page_set_id,
            page_ids: registered.pages.iter().map(|page| page.page_id.clone()).collect(),
            smart_page_ids: registered
                .pages
                .iter()
                .map(|page| page.smart_page_id.clone())
                .collect(),
            pdf_asset_id: registered.pdf_asset_id,
            pdf_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;

    const PLACEHOLDER_DESIGN_ID: &str = "6DE28E53DBKPXCWWNHPC8T7QJX";

    fn setup_payload(design_id: NotebookDesignId) -> String {
        PageCode::NotebookSetup { design_id }.encode().unwrap()
    }

    fn notebook_page_payload(
        design_id: NotebookDesignId,
        logical_page_number: u32,
        layout_id: LayoutId,
    ) -> String {
        PageCode::NotebookPage {
            design_id,
            logical_page_number,
            layout_id,
        }
        .encode()
        .unwrap()
    }

    fn open_core() -> (std::sync::Arc<A2dCore>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "a2d-milestone6-test-{}",
            a2d_domain::PageId::generate()
        ));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, dir)
    }

    fn create_request(name: &str, make_active: bool) -> CreateNotebookRequest {
        CreateNotebookRequest {
            setup_payload: setup_payload(NotebookDesignId::parse(PLACEHOLDER_DESIGN_ID).unwrap()),
            display_name: name.to_string(),
            optional_color: None,
            optional_icon: None,
            optional_user_notes: None,
            make_active,
        }
    }

    #[test]
    fn setup_code_resolves_offline_and_notebook_creation_is_transactional() {
        let (core, dir) = open_core();
        let summary = core
            .resolve_notebook_setup_code(&create_request("First", false).setup_payload)
            .unwrap();
        assert_eq!(summary.logical_page_count, 100);

        let created = core.create_notebook(create_request(" First ", true)).unwrap();
        assert_eq!(created.notebook.display_name, "First");
        assert_eq!(created.created_page_count, 100);
        assert!(created.notebook.active);

        let storage = core.lock_storage().unwrap();
        assert!(storage
            .find_notebook_page(&created.notebook.id, 1)
            .unwrap()
            .is_some());
        assert!(storage
            .find_notebook_page(&created.notebook.id, 100)
            .unwrap()
            .is_some());
        drop(storage);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn identical_physical_notebooks_remain_distinct_and_require_selection_without_active_state() {
        let (core, dir) = open_core();
        let first = core.create_notebook(create_request("Copy A", false)).unwrap();
        let second = core.create_notebook(create_request("Copy B", false)).unwrap();
        assert_ne!(first.notebook.id, second.notebook.id);
        assert_eq!(first.notebook.design_id, second.notebook.design_id);

        let design = core
            .available_design(&first.notebook.design_id)
            .unwrap()
            .unwrap();
        let payload = notebook_page_payload(
            design.id().clone(),
            7,
            design.page_layout_id.clone(),
        );
        match core.resolve_page_code(&payload, None).unwrap() {
            PageResolution::RequiresNotebookSelection { candidates } => {
                assert_eq!(candidates.len(), 2);
            }
            other => panic!("expected selection, got {other:?}"),
        }

        core.set_active_notebook(Some(&first.notebook.id)).unwrap();
        match core.resolve_page_code(&payload, None).unwrap() {
            PageResolution::Resolved {
                notebook_id: Some(id),
                ..
            } => assert_eq!(id, first.notebook.id),
            other => panic!("expected active resolution, got {other:?}"),
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn active_notebook_persists_and_archiving_it_clears_the_destination() {
        let (core, dir) = open_core();
        let created = core.create_notebook(create_request("Persistent", true)).unwrap();
        drop(core);
        let reopened = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        assert_eq!(
            reopened.get_active_notebook().unwrap().unwrap().id,
            created.notebook.id
        );
        let archived = reopened.archive_notebook(&created.notebook.id).unwrap();
        assert!(archived.archived);
        assert!(!archived.active);
        assert!(reopened.get_active_notebook().unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn unknown_smart_page_is_explicit_and_generated_smart_page_resolves() {
        let (core, dir) = open_core();
        let unknown_id = SmartPageId::generate();
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank).id;
        let unknown_payload = PageCode::SmartPage {
            smart_page_id: unknown_id.clone(),
            layout_id: layout,
            visible_page_number: Some(1),
            page_set_id: None,
        }
        .encode()
        .unwrap();
        assert!(matches!(
            core.resolve_page_code(&unknown_payload, None).unwrap(),
            PageResolution::ImportedUnknownSmartPage { smart_page_id, .. } if smart_page_id == unknown_id
        ));

        let generated = core
            .generate_smart_pages(SmartPageGenerationRequest {
                paper_size: SmartPagePaperSize::A4,
                style: SmartPageContentStyle::Lined,
                page_count: 2,
                starting_visible_page: 1,
            })
            .unwrap();
        assert!(std::path::Path::new(&generated.pdf_path).is_file());
        let generated_payload = PageCode::SmartPage {
            smart_page_id: generated.smart_page_ids[0].clone(),
            layout_id: smart_page_layout(PaperSize::A4, SmartPageStyle::Lined).id,
            visible_page_number: Some(1),
            page_set_id: Some(generated.page_set_id.clone()),
        }
        .encode()
        .unwrap();
        assert!(matches!(
            core.resolve_page_code(&generated_payload, None).unwrap(),
            PageResolution::Resolved { notebook_id: None, .. }
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rename_validation_and_archived_activation_fail_visibly() {
        let (core, dir) = open_core();
        let created = core.create_notebook(create_request("Notebook", false)).unwrap();
        assert!(core
            .rename_notebook(&created.notebook.id, "   ".to_string())
            .is_err());
        core.archive_notebook(&created.notebook.id).unwrap();
        let err = core
            .set_active_notebook(Some(&created.notebook.id))
            .unwrap_err();
        assert!(err.code.to_string().contains("ARCHIVED_NOTEBOOK"));
        std::fs::remove_dir_all(dir).ok();
    }
}
''',
)

replace_once(
    "crates/a2d-ffi/src/lib.rs",
    '''uniffi::setup_scaffolding!();

#[derive(uniffi::Record)]
''',
    '''uniffi::setup_scaffolding!();

mod milestone6;
pub use milestone6::*;

#[derive(uniffi::Record)]
''',
)

write(
    "crates/a2d-ffi/src/milestone6.rs",
    r'''//! UniFFI projections for Milestone 6. No business rules live here.

use std::sync::Arc;

use a2d_core as core;
use a2d_domain::NotebookId;

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Debug, uniffi::Record)]
pub struct NotebookSummary {
    pub id: String,
    pub design_id: String,
    pub display_name: String,
    pub archived: bool,
    pub active: bool,
}

impl From<core::NotebookSummary> for NotebookSummary {
    fn from(value: core::NotebookSummary) -> Self {
        Self {
            id: value.id.to_string(),
            design_id: value.design_id.to_string(),
            display_name: value.display_name,
            archived: value.archived,
            active: value.active,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct NotebookDesignSummary {
    pub id: String,
    pub name: String,
    pub design_version: u32,
    pub logical_page_count: u32,
    pub trusted: bool,
}

impl From<core::NotebookDesignSummary> for NotebookDesignSummary {
    fn from(value: core::NotebookDesignSummary) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            design_version: value.design_version,
            logical_page_count: value.logical_page_count,
            trusted: value.trusted,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct CreateNotebookRequest {
    pub setup_payload: String,
    pub display_name: String,
    pub optional_color: Option<String>,
    pub optional_icon: Option<String>,
    pub optional_user_notes: Option<String>,
    pub make_active: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct CreatedNotebook {
    pub notebook: NotebookSummary,
    pub created_page_count: u32,
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum PageResolution {
    Resolved {
        page_id: String,
        notebook_id: Option<String>,
    },
    RequiresNotebookSelection {
        candidates: Vec<NotebookSummary>,
    },
    RequiresNotebookRegistration {
        design: NotebookDesignSummary,
    },
    ConflictingActiveNotebook {
        active: NotebookSummary,
        detected_design: String,
    },
    ImportedUnknownSmartPage {
        smart_page_id: String,
        layout_id: String,
        visible_page_number: Option<u32>,
        page_set_id: Option<String>,
    },
    UnsupportedCode {
        reason: String,
    },
}

impl From<core::PageResolution> for PageResolution {
    fn from(value: core::PageResolution) -> Self {
        match value {
            core::PageResolution::Resolved {
                page_id,
                notebook_id,
            } => Self::Resolved {
                page_id: page_id.to_string(),
                notebook_id: notebook_id.map(|id| id.to_string()),
            },
            core::PageResolution::RequiresNotebookSelection { candidates } => {
                Self::RequiresNotebookSelection {
                    candidates: candidates.into_iter().map(Into::into).collect(),
                }
            }
            core::PageResolution::RequiresNotebookRegistration { design } => {
                Self::RequiresNotebookRegistration {
                    design: design.into(),
                }
            }
            core::PageResolution::ConflictingActiveNotebook {
                active,
                detected_design,
            } => Self::ConflictingActiveNotebook {
                active: active.into(),
                detected_design: detected_design.to_string(),
            },
            core::PageResolution::ImportedUnknownSmartPage {
                smart_page_id,
                layout_id,
                visible_page_number,
                page_set_id,
            } => Self::ImportedUnknownSmartPage {
                smart_page_id: smart_page_id.to_string(),
                layout_id: layout_id.to_string(),
                visible_page_number,
                page_set_id: page_set_id.map(|id| id.to_string()),
            },
            core::PageResolution::UnsupportedCode { reason } => Self::UnsupportedCode { reason },
        }
    }
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum SmartPagePaperSize {
    UsLetter,
    A4,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum SmartPageContentStyle {
    Blank,
    Lined,
    DotGrid,
    Graph,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SmartPageGenerationRequest {
    pub paper_size: SmartPagePaperSize,
    pub style: SmartPageContentStyle,
    pub page_count: u32,
    pub starting_visible_page: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct GeneratedSmartPages {
    pub page_set_id: String,
    pub page_ids: Vec<String>,
    pub smart_page_ids: Vec<String>,
    pub pdf_asset_id: String,
    pub pdf_path: String,
}

fn parse_notebook_id(raw: String) -> Result<NotebookId, A2dFfiError> {
    NotebookId::parse(&raw).map_err(Into::into)
}

#[uniffi::export]
impl A2dClient {
    pub fn resolve_notebook_setup_code(
        &self,
        payload: String,
    ) -> Result<NotebookDesignSummary, A2dFfiError> {
        self.core
            .resolve_notebook_setup_code(&payload)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn create_notebook(
        &self,
        request: CreateNotebookRequest,
    ) -> Result<CreatedNotebook, A2dFfiError> {
        let created = self.core.create_notebook(core::CreateNotebookRequest {
            setup_payload: request.setup_payload,
            display_name: request.display_name,
            optional_color: request.optional_color,
            optional_icon: request.optional_icon,
            optional_user_notes: request.optional_user_notes,
            make_active: request.make_active,
        })?;
        Ok(CreatedNotebook {
            notebook: created.notebook.into(),
            created_page_count: created.created_page_count,
        })
    }

    pub fn rename_notebook(
        &self,
        notebook_id: String,
        display_name: String,
    ) -> Result<NotebookSummary, A2dFfiError> {
        self.core
            .rename_notebook(&parse_notebook_id(notebook_id)?, display_name)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn archive_notebook(
        &self,
        notebook_id: String,
    ) -> Result<NotebookSummary, A2dFfiError> {
        self.core
            .archive_notebook(&parse_notebook_id(notebook_id)?)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn list_notebooks(
        &self,
        include_archived: bool,
    ) -> Result<Vec<NotebookSummary>, A2dFfiError> {
        self.core
            .list_notebooks(include_archived)
            .map(|items| items.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub fn get_notebook(
        &self,
        notebook_id: String,
    ) -> Result<Option<NotebookSummary>, A2dFfiError> {
        self.core
            .get_notebook(&parse_notebook_id(notebook_id)?)
            .map(|item| item.map(Into::into))
            .map_err(Into::into)
    }

    pub fn set_active_notebook(
        &self,
        notebook_id: Option<String>,
    ) -> Result<Option<NotebookSummary>, A2dFfiError> {
        let parsed = notebook_id.map(parse_notebook_id).transpose()?;
        self.core
            .set_active_notebook(parsed.as_ref())
            .map(|item| item.map(Into::into))
            .map_err(Into::into)
    }

    pub fn get_active_notebook(&self) -> Result<Option<NotebookSummary>, A2dFfiError> {
        self.core
            .get_active_notebook()
            .map(|item| item.map(Into::into))
            .map_err(Into::into)
    }

    pub fn resolve_page_code(
        &self,
        payload: String,
        confirmed_notebook_id: Option<String>,
    ) -> Result<PageResolution, A2dFfiError> {
        let confirmed = confirmed_notebook_id.map(parse_notebook_id).transpose()?;
        self.core
            .resolve_page_code(&payload, confirmed.as_ref())
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn generate_smart_pages(
        &self,
        request: SmartPageGenerationRequest,
    ) -> Result<GeneratedSmartPages, A2dFfiError> {
        let paper_size = match request.paper_size {
            SmartPagePaperSize::UsLetter => core::SmartPagePaperSize::UsLetter,
            SmartPagePaperSize::A4 => core::SmartPagePaperSize::A4,
        };
        let style = match request.style {
            SmartPageContentStyle::Blank => core::SmartPageContentStyle::Blank,
            SmartPageContentStyle::Lined => core::SmartPageContentStyle::Lined,
            SmartPageContentStyle::DotGrid => core::SmartPageContentStyle::DotGrid,
            SmartPageContentStyle::Graph => core::SmartPageContentStyle::Graph,
        };
        let generated = self.core.generate_smart_pages(core::SmartPageGenerationRequest {
            paper_size,
            style,
            page_count: request.page_count,
            starting_visible_page: request.starting_visible_page,
        })?;
        Ok(GeneratedSmartPages {
            page_set_id: generated.page_set_id.to_string(),
            page_ids: generated.page_ids.into_iter().map(|id| id.to_string()).collect(),
            smart_page_ids: generated
                .smart_page_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            pdf_asset_id: generated.pdf_asset_id.to_string(),
            pdf_path: generated.pdf_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenLibraryRequest;

    #[test]
    fn notebook_and_generation_workflows_delegate_through_the_ffi_boundary() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-ffi-milestone6-test-{}",
            a2d_domain::PageId::generate()
        ));
        let client = A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        let setup_payload = a2d_identity::PageCode::NotebookSetup {
            design_id: a2d_domain::NotebookDesignId::parse(
                "6DE28E53DBKPXCWWNHPC8T7QJX",
            )
            .unwrap(),
        }
        .encode()
        .unwrap();
        let design = client
            .resolve_notebook_setup_code(setup_payload.clone())
            .unwrap();
        assert_eq!(design.logical_page_count, 100);
        let created = client
            .create_notebook(CreateNotebookRequest {
                setup_payload,
                display_name: "FFI Notebook".to_string(),
                optional_color: None,
                optional_icon: None,
                optional_user_notes: None,
                make_active: true,
            })
            .unwrap();
        assert!(created.notebook.active);
        assert_eq!(client.list_notebooks(false).unwrap().len(), 1);

        let generated = client
            .generate_smart_pages(SmartPageGenerationRequest {
                paper_size: SmartPagePaperSize::A4,
                style: SmartPageContentStyle::Blank,
                page_count: 1,
                starting_visible_page: 1,
            })
            .unwrap();
        assert!(std::path::Path::new(&generated.pdf_path).is_file());
        std::fs::remove_dir_all(dir).ok();
    }
}
''',
)

print("Milestone 6 Rust/FFI transformations applied")
