//! Composes the other crates into the typed use cases exposed across the FFI boundary.
//!
//! Milestone 2.4's job is proving the FFI plumbing works end-to-end, not implementing real
//! library use cases — those need storage (Milestone 3) and workflows (Milestone 6+), neither of
//! which exist yet. `open_library` is genuinely complete (path validation has no storage
//! dependency); `generate_page_id`/`parse_page_id` re-expose already-complete Milestone 2.1
//! functionality specifically so there is a real, non-stub operation to prove the FFI round-trip
//! with, rather than a placeholder that returns a fabricated empty result.

use std::path::PathBuf;
use std::sync::Arc;

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesignId, PageId,
    SmartPageId,
};
use a2d_identity::PageCode;

pub struct OpenLibraryRequest {
    pub library_path: String,
}

#[derive(Debug)]
pub struct A2dCore {
    library_path: PathBuf,
}

impl A2dCore {
    /// Opens (creating if necessary) the local library directory. Does not touch SQLite —
    /// that's Milestone 3.
    pub fn open(request: OpenLibraryRequest) -> Result<Arc<Self>, A2dError> {
        let path = PathBuf::from(&request.library_path);
        std::fs::create_dir_all(&path).map_err(|e| {
            A2dError::new(
                ErrorCode::new("CORE_OPEN_LIBRARY_IO"),
                ErrorCategory::Storage,
                ErrorSeverity::Error,
                "error.core.open_library_io",
                format!("failed to create or access the library directory: {e}"),
                true,
            )
            .with_detail("path", request.library_path.clone())
        })?;
        if !path.is_dir() {
            return Err(A2dError::new(
                ErrorCode::new("CORE_OPEN_LIBRARY_NOT_A_DIRECTORY"),
                ErrorCategory::Storage,
                ErrorSeverity::Error,
                "error.core.open_library_not_a_directory",
                "library path exists but is not a directory",
                false,
            )
            .with_detail("path", request.library_path));
        }
        Ok(Arc::new(Self { library_path: path }))
    }

    pub fn library_path(&self) -> String {
        self.library_path.to_string_lossy().into_owned()
    }

    pub fn generate_page_id(&self) -> String {
        PageId::generate().to_string()
    }

    pub fn parse_page_id(&self, candidate: &str) -> Result<String, A2dError> {
        PageId::parse(candidate).map(|id| id.to_string())
    }

    /// Generates a real (freshly random) example QR payload for each of the three v1 code types
    /// (ADR 0001), so a caller can exercise a genuine render/decode round trip rather than a
    /// hand-typed fixture string. Encoding only, per that ADR's Proposed (not yet Accepted)
    /// status — see `a2d_identity::qr`'s module doc.
    pub fn generate_example_notebook_setup_qr_payload(&self) -> Result<String, A2dError> {
        PageCode::NotebookSetup {
            design_id: NotebookDesignId::generate(),
        }
        .encode()
    }

    pub fn generate_example_notebook_page_qr_payload(&self) -> Result<String, A2dError> {
        PageCode::NotebookPage {
            design_id: NotebookDesignId::generate(),
            logical_page_number: 12,
            layout_id: example_layout_id(),
        }
        .encode()
    }

    pub fn generate_example_smart_page_qr_payload(&self) -> Result<String, A2dError> {
        PageCode::SmartPage {
            smart_page_id: SmartPageId::generate(),
            layout_id: example_layout_id(),
            visible_page_number: Some(3),
            page_set_id: None,
        }
        .encode()
    }
}

fn example_layout_id() -> LayoutId {
    LayoutId::parse("USLETTER-LINED").expect("static layout token is a valid LayoutId")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_the_library_directory() {
        let dir = std::env::temp_dir().join(format!("a2d-core-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .expect("open must succeed for a fresh directory");
        assert!(dir.is_dir());
        assert_eq!(core.library_path(), dir.to_string_lossy());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_rejects_a_path_that_is_an_existing_file() {
        let file = std::env::temp_dir().join(format!("a2d-core-test-file-{}", PageId::generate()));
        std::fs::write(&file, b"not a directory").expect("test setup must be able to write");
        let err = A2dCore::open(OpenLibraryRequest {
            library_path: file.to_string_lossy().into_owned(),
        })
        .unwrap_err();
        assert_eq!(err.category, ErrorCategory::Storage);
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn generate_then_parse_round_trips_through_core() {
        let dir = std::env::temp_dir().join(format!("a2d-core-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        let generated = core.generate_page_id();
        let parsed = core
            .parse_page_id(&generated)
            .expect("must parse its own output");
        assert_eq!(generated, parsed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_page_id_rejects_garbage() {
        let dir = std::env::temp_dir().join(format!("a2d-core-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        assert!(core.parse_page_id("not-a-valid-id").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn example_qr_payloads_are_well_formed_and_fresh_each_call() {
        let dir = std::env::temp_dir().join(format!("a2d-core-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();

        let setup_a = core.generate_example_notebook_setup_qr_payload().unwrap();
        let setup_b = core.generate_example_notebook_setup_qr_payload().unwrap();
        assert!(setup_a.starts_with("A2D:1:S:"));
        assert_ne!(
            setup_a, setup_b,
            "each call must generate a fresh random id"
        );

        let page = core.generate_example_notebook_page_qr_payload().unwrap();
        assert!(page.starts_with("A2D:1:B:"));

        let smart = core.generate_example_smart_page_qr_payload().unwrap();
        assert!(smart.starts_with("A2D:1:M:"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
