//! Milestone 6 notebook, page-resolution, and Smart Page workflows.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, Notebook, NotebookDesign,
    NotebookDesignId, NotebookId, Page, PageId, PageKind, PageState, SmartPageId, TrustState,
    system_now_ms,
};
use a2d_identity::PageCode;
use a2d_layout::smart_page::{ALL_PAPER_SIZES, ALL_STYLES};
use a2d_layout::{
    ManifestRegistry, PaperSize, SmartPageStyle, bundled_placeholder_registry, smart_page_layout,
};
use a2d_storage::{
    AssetRepository, NotebookDesignRepository, NotebookRepository, NotebookWorkflowRepository,
    PageLookupRepository, PageRepository,
};

use super::A2dCore;

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

fn is_known_layout(registry: &ManifestRegistry, layout_id: &LayoutId) -> bool {
    if let Ok(id) = NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX")
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
    fn available_design_with_registry(
        &self,
        id: &NotebookDesignId,
        registry: &ManifestRegistry,
    ) -> Result<Option<NotebookDesign>, A2dError> {
        if let Some(stored) = self.lock_storage()?.get_notebook_design(id)? {
            return Ok(Some(stored));
        }
        Ok(registry.resolve(id).cloned())
    }

    #[cfg(test)]
    fn available_design(&self, id: &NotebookDesignId) -> Result<Option<NotebookDesign>, A2dError> {
        let registry = bundled_placeholder_registry()?;
        self.available_design_with_registry(id, &registry)
    }

    fn setup_design_from_payload(&self, payload: &str) -> Result<NotebookDesign, A2dError> {
        let registry = bundled_placeholder_registry()?;
        let parsed =
            a2d_identity::qr::parse(payload, |layout_id| is_known_layout(&registry, layout_id))?;
        let PageCode::NotebookSetup { design_id } = parsed else {
            return Err(workflow_error(
                "CORE_EXPECTED_NOTEBOOK_SETUP_CODE",
                "the supplied payload is valid A2D data but is not a Notebook Setup Code",
            ));
        };
        let design = self
            .available_design_with_registry(&design_id, &registry)?
            .ok_or_else(|| {
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
        let created_at = system_now_ms()?;
        let mut notebook = Notebook::new(
            NotebookId::try_generate()?,
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

        let pages = (1..=design.logical_page_count)
            .map(|logical_page_number| {
                Ok(Page::new(
                    PageId::try_generate()?,
                    PageKind::NotebookPage {
                        notebook_id: notebook.id().clone(),
                        design_id: design.id().clone(),
                        logical_page_number,
                    },
                    design.page_layout_id.clone(),
                    None,
                    PageState::Unscanned,
                    created_at,
                ))
            })
            .collect::<Result<Vec<_>, A2dError>>()?;

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
        let storage = self.lock_storage()?;
        let mut notebook = storage
            .get_notebook(notebook_id)?
            .ok_or_else(|| workflow_error("CORE_NOTEBOOK_NOT_FOUND", "notebook was not found"))?;
        notebook.rename(display_name, system_now_ms()?)?;
        storage.update_notebook(&notebook)?;
        Ok(NotebookSummary::from(&notebook))
    }

    pub fn archive_notebook(&self, notebook_id: &NotebookId) -> Result<NotebookSummary, A2dError> {
        let storage = self.lock_storage()?;
        let mut notebook = storage
            .get_notebook(notebook_id)?
            .ok_or_else(|| workflow_error("CORE_NOTEBOOK_NOT_FOUND", "notebook was not found"))?;
        notebook.archive(system_now_ms()?);
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
        storage.set_active_notebook(notebook_id, system_now_ms()?)?;
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
        let registry = bundled_placeholder_registry()?;
        let code =
            a2d_identity::qr::parse(payload, |layout_id| is_known_layout(&registry, layout_id))?;
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
                    .or_else(|| registry.resolve(&design_id).cloned());
                let Some(design) = design else {
                    return Ok(PageResolution::UnsupportedCode {
                        reason: format!(
                            "Notebook Design {design_id} is not available in this build"
                        ),
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
                    storage
                        .get_notebook(confirmed)?
                        .ok_or_else(|| {
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
            page_ids: registered
                .pages
                .iter()
                .map(|page| page.page_id.clone())
                .collect(),
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

        let created = core
            .create_notebook(create_request(" First ", true))
            .unwrap();
        assert_eq!(created.notebook.display_name, "First");
        assert_eq!(created.created_page_count, 100);
        assert!(created.notebook.active);

        let storage = core.lock_storage().unwrap();
        assert!(
            storage
                .find_notebook_page(&created.notebook.id, 1)
                .unwrap()
                .is_some()
        );
        assert!(
            storage
                .find_notebook_page(&created.notebook.id, 100)
                .unwrap()
                .is_some()
        );
        drop(storage);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn identical_physical_notebooks_remain_distinct_and_require_selection_without_active_state() {
        let (core, dir) = open_core();
        let first = core
            .create_notebook(create_request("Copy A", false))
            .unwrap();
        let second = core
            .create_notebook(create_request("Copy B", false))
            .unwrap();
        assert_ne!(first.notebook.id, second.notebook.id);
        assert_eq!(first.notebook.design_id, second.notebook.design_id);

        let design = core
            .available_design(&first.notebook.design_id)
            .unwrap()
            .unwrap();
        let payload = notebook_page_payload(design.id().clone(), 7, design.page_layout_id.clone());
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
        let created = core
            .create_notebook(create_request("Persistent", true))
            .unwrap();
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
            PageResolution::Resolved {
                notebook_id: None,
                ..
            }
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rename_validation_and_archived_activation_fail_visibly() {
        let (core, dir) = open_core();
        let created = core
            .create_notebook(create_request("Notebook", false))
            .unwrap();
        assert!(
            core.rename_notebook(&created.notebook.id, "   ".to_string())
                .is_err()
        );
        core.archive_notebook(&created.notebook.id).unwrap();
        let err = core
            .set_active_notebook(Some(&created.notebook.id))
            .unwrap_err();
        assert!(err.code.to_string().contains("ARCHIVED_NOTEBOOK"));
        std::fs::remove_dir_all(dir).ok();
    }
}
