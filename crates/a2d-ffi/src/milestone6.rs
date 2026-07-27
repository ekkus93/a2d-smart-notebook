//! UniFFI projections for Milestone 6. No business rules live here.

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

    pub fn archive_notebook(&self, notebook_id: String) -> Result<NotebookSummary, A2dFfiError> {
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
        let generated = self
            .core
            .generate_smart_pages(core::SmartPageGenerationRequest {
                paper_size,
                style,
                page_count: request.page_count,
                starting_visible_page: request.starting_visible_page,
            })?;
        Ok(GeneratedSmartPages {
            page_set_id: generated.page_set_id.to_string(),
            page_ids: generated
                .page_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
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
            design_id: a2d_domain::NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX").unwrap(),
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
