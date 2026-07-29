//! Canonical stored-page lookup for Rust-owned scan processing policy.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, NotebookDesign, PageId, PageKind,
};
use a2d_layout::{ResolvedScanLayout, resolve_scan_layout_for_page};
use a2d_storage::{NotebookDesignRepository, PageRepository};

use super::A2dCore;

impl A2dCore {
    /// Loads the canonical stored `Page` and, for Notebook Pages, its stored `NotebookDesign`, then
    /// resolves one Rust-owned scan-processing policy. Missing or contradictory canonical state
    /// fails closed before image decoding or marker detection begins.
    pub fn resolve_stored_scan_layout(
        &self,
        page_id: &PageId,
    ) -> Result<ResolvedScanLayout, A2dError> {
        let storage = self.lock_storage()?;
        let page = storage.get_page(page_id)?.ok_or_else(|| {
            policy_error(
                "CORE_SCAN_LAYOUT_PAGE_NOT_FOUND",
                ErrorCategory::Validation,
                "the requested page does not exist",
            )
            .with_detail("page_id", page_id.to_string())
        })?;
        let design = load_notebook_design(&storage, &page.kind, page_id)?;
        resolve_scan_layout_for_page(&page, design.as_ref())
    }
}

fn load_notebook_design(
    storage: &a2d_storage::Storage,
    page_kind: &PageKind,
    page_id: &PageId,
) -> Result<Option<NotebookDesign>, A2dError> {
    let PageKind::NotebookPage { design_id, .. } = page_kind else {
        return Ok(None);
    };
    let design = storage.get_notebook_design(design_id)?.ok_or_else(|| {
        policy_error(
            "CORE_SCAN_LAYOUT_DESIGN_ROW_MISSING",
            ErrorCategory::Integrity,
            "the stored Notebook Page references a Notebook Design row that does not exist",
        )
        .with_detail("page_id", page_id.to_string())
        .with_detail("design_id", design_id.to_string())
    })?;
    Ok(Some(design))
}

fn policy_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.core.scan_layout",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_domain::{
        Notebook, NotebookId, Page, PageKind, PageState, SmartPageId, system_now_ms,
    };
    use a2d_layout::{PaperSize, SmartPageStyle, bundled_placeholder_registry, smart_page_layout};
    use a2d_storage::{NotebookDesignRepository, NotebookRepository, PageRepository};

    use super::*;
    use crate::OpenLibraryRequest;

    fn open_test_core(label: &str) -> (std::sync::Arc<A2dCore>, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "a2d-scan-policy-{label}-{}",
            PageId::generate()
        ));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, root)
    }

    #[test]
    fn missing_page_fails_before_layout_resolution() {
        let (core, root) = open_test_core("missing");
        let error = core
            .resolve_stored_scan_layout(&PageId::generate())
            .unwrap_err();
        assert_eq!(error.code.to_string(), "CORE_SCAN_LAYOUT_PAGE_NOT_FOUND");
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn notebook_page_resolves_through_its_stored_design() {
        let (core, root) = open_test_core("notebook");
        let registry = bundled_placeholder_registry().unwrap();
        let design_id = a2d_domain::NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX").unwrap();
        let design = registry.resolve(&design_id).unwrap().clone();
        let notebook_id = NotebookId::generate();
        let page_id = PageId::generate();
        let now = system_now_ms().unwrap();
        let notebook = Notebook::new(
            notebook_id.clone(),
            design.id().clone(),
            "Scan policy test".to_string(),
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
                logical_page_number: 1,
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

        let resolved = core.resolve_stored_scan_layout(&page_id).unwrap();
        assert_eq!(resolved.layout_id, design.page_layout_id);
        assert_eq!(resolved.corrected_width, 900);
        assert_eq!(resolved.corrected_height, 1_356);

        drop(core);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn smart_page_resolves_without_a_notebook_design() {
        let (core, root) = open_test_core("smart");
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let page_id = PageId::generate();
        let page = Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            layout.id.clone(),
            None,
            PageState::GeneratedNotScanned,
            system_now_ms().unwrap(),
        );
        core.lock_storage().unwrap().insert_page(&page).unwrap();

        let resolved = core.resolve_stored_scan_layout(&page_id).unwrap();
        assert_eq!(resolved.layout_id, layout.id);
        assert_eq!(resolved.corrected_width, 900);
        assert_eq!(resolved.corrected_height, 1_273);

        drop(core);
        std::fs::remove_dir_all(root).ok();
    }
}
