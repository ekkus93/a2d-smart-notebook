//! Reconstructs the canonical Page Code for an existing stored page.
//!
//! This is used by process-death recovery so platform code never has to persist or guess identity
//! payloads outside the Rust-owned page model.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId, PageKind};
use a2d_identity::PageCode;
use a2d_storage::PageRepository;

use super::A2dCore;

impl A2dCore {
    pub fn stored_page_code_payload(&self, page_id: &PageId) -> Result<String, A2dError> {
        let storage = self.lock_storage()?;
        let page = storage.get_page(page_id)?.ok_or_else(|| {
            projection_error(
                "CORE_STORED_PAGE_CODE_PAGE_NOT_FOUND",
                ErrorCategory::Validation,
                "the requested stored page does not exist",
            )
            .with_detail("page_id", page_id.to_string())
        })?;
        let code = match page.kind {
            PageKind::NotebookPage {
                design_id,
                logical_page_number,
                ..
            } => PageCode::NotebookPage {
                design_id,
                logical_page_number,
                layout_id: page.layout_id,
            },
            PageKind::SmartPage {
                smart_page_id,
                page_set_id,
                visible_page_number,
            } => PageCode::SmartPage {
                smart_page_id,
                layout_id: page.layout_id,
                visible_page_number,
                page_set_id,
            },
        };
        code.encode()
    }
}

fn projection_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        ErrorSeverity::Error,
        "error.core.stored_page_code",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_domain::{Notebook, NotebookDesignId, NotebookId, Page, PageState, system_now_ms};
    use a2d_layout::bundled_placeholder_registry;
    use a2d_storage::{NotebookDesignRepository, NotebookRepository};

    use super::*;
    use crate::OpenLibraryRequest;

    #[test]
    fn stored_notebook_page_round_trips_through_the_identity_codec() {
        let root =
            std::env::temp_dir().join(format!("a2d-page-code-projection-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let registry = bundled_placeholder_registry().unwrap();
        let design_id = NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX").unwrap();
        let design = registry.resolve(&design_id).unwrap().clone();
        let notebook_id = NotebookId::generate();
        let page_id = PageId::generate();
        let now = system_now_ms().unwrap();
        let notebook = Notebook::new(
            notebook_id.clone(),
            design.id().clone(),
            "Projection test".to_string(),
            now,
            now,
            None,
            false,
            None,
            None,
            None,
        );
        let page = Page::new(
            page_id.clone(),
            PageKind::NotebookPage {
                notebook_id,
                design_id: design.id().clone(),
                logical_page_number: 7,
            },
            design.page_layout_id.clone(),
            None,
            PageState::Unscanned,
            now,
        );
        {
            let storage = core.lock_storage().unwrap();
            storage.insert_notebook_design(&design).unwrap();
            storage.insert_notebook(&notebook).unwrap();
            storage.insert_page(&page).unwrap();
        }
        let payload = core.stored_page_code_payload(&page_id).unwrap();
        let parsed = a2d_identity::qr::parse(&payload, |_| true).unwrap();
        assert!(matches!(
            parsed,
            PageCode::NotebookPage {
                logical_page_number: 7,
                ..
            }
        ));
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }
}
