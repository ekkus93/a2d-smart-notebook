//! Milestone 6 notebook and page-resolution queries.
//!
//! These operations remain inside `a2d-storage` so SQL never leaks into the service or FFI
//! layers. They supplement the basic CRUD traits with the exact query/update shapes needed by the
//! notebook workflow and deterministic page resolver.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, Notebook, NotebookDesignId, NotebookId, Page,
    PageId, SmartPageId,
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
        PageLookupRepository::find_notebook_page(&self.conn, notebook_id, logical_page_number)
    }
}
