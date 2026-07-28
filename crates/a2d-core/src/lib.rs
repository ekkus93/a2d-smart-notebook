//! Composes the other crates into the typed use cases exposed across the FFI boundary.
//!
//! `open_library` now opens the real SQLite database and asset store (Milestone 3), not just a
//! bare directory. `Storage::transaction` needs `&mut Storage`, but `A2dCore` is shared behind
//! `Arc` and its methods take `&self` (mirroring how `a2d-ffi`'s `A2dClient` holds it) — so
//! `storage` is wrapped in a `Mutex`, locked for the duration of each use case's transaction.
//! `AssetStore`'s own methods only need `&self` (each asset commit uses its own fresh temp
//! filename), so it needs no such wrapper.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use a2d_domain::{
    A2dError, AssetKind, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesignId, Page,
    PageId, PageKind, PageSet, PageSetId, SmartPageId,
};
use a2d_identity::PageCode;
use a2d_storage::{AssetRepository, AssetStore, PageRepository, PageSetRepository, Storage};

mod milestone6;
pub use milestone6::*;
mod milestone9;
pub use milestone9::*;

pub struct OpenLibraryRequest {
    pub library_path: String,
}

pub struct A2dCore {
    library_path: PathBuf,
    storage: Mutex<Storage>,
    asset_store: AssetStore,
}

// Manual impl: `Storage`/`AssetStore` don't derive `Debug` (rusqlite's `Connection` doesn't),
// but `Result::unwrap_err` in tests needs *some* `Debug` bound on the `Ok` type. Prints only the
// library path, never connection/file-handle internals.
impl std::fmt::Debug for A2dCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2dCore")
            .field("library_path", &self.library_path)
            .finish_non_exhaustive()
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl A2dCore {
    /// Opens (creating if necessary) the local library directory, its SQLite database
    /// (`library.sqlite`), and its asset store (`assets/`, `tmp/`).
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
        let storage = Storage::open(&path.join("library.sqlite"))?;
        let asset_store = AssetStore::open(&path)?;
        Ok(Arc::new(Self {
            library_path: path,
            storage: Mutex::new(storage),
            asset_store,
        }))
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

    fn lock_storage(&self) -> Result<std::sync::MutexGuard<'_, Storage>, A2dError> {
        self.storage.lock().map_err(|_| {
            A2dError::new(
                ErrorCode::new("CORE_STORAGE_LOCK_POISONED"),
                ErrorCategory::Internal,
                ErrorSeverity::Critical,
                "error.core.storage_lock_poisoned",
                "the storage mutex was poisoned by a panic in another operation",
                false,
            )
        })
    }

    /// Generates a Smart Page Set PDF and registers it durably (TODO 5.5, spec §7.6): the PDF is
    /// committed through the asset commit protocol (spec §16.3) first, since that step is itself
    /// a write-then-verify-then-rename sequence with nothing to roll back; the `PageSet`, every
    /// `Page`, and the `Asset` row are then inserted together in a single `Storage::transaction`
    /// so they become visible atomically. A transaction failure rolls back every row from this
    /// attempt automatically (`Storage::transaction`'s rollback-on-drop) — nothing partial is
    /// ever left behind to collide with a retry, and every ID here (`PageSetId`, each
    /// `SmartPageId`, each `PageId`, the `AssetId`) is freshly random regardless of how many
    /// times this is called, so a retry can never produce a duplicate logical record even though
    /// nothing here is idempotent by request identity (spec §12.2: every generation is supposed
    /// to mint new identities, not reuse them).
    ///
    /// **Known gap**: if the DB transaction fails *after* the asset was already durably
    /// committed to `assets/exports/`, that asset file is orphaned — there is no database row
    /// referencing it. This function does not silently hide that: the returned error carries the
    /// orphaned `AssetId` in its `details` so it's diagnosable, but there is no automated
    /// orphan-cleanup or review-item mechanism yet (that needs Milestone 9.4/16's broader Needs
    /// Review and integrity-check infrastructure, neither of which exists yet) — matching how
    /// Milestone 3.3 left its own asset-commit orphan cleanup as documented future work rather
    /// than building it ahead of need.
    pub fn generate_and_register_page_set(
        &self,
        request: a2d_pdf::GeneratePageSetRequest,
    ) -> Result<RegisteredPageSet, A2dError> {
        let starting_visible_page = request.starting_visible_page;
        let generated = a2d_pdf::render_page_set_pdf_bytes(request)?;

        let asset =
            self.asset_store
                .commit(&generated.pdf_bytes, AssetKind::Export, "application/pdf")?;

        let created_at = now_ms();
        let page_set = PageSet::new(generated.page_set_id.clone(), None, created_at);

        let mut pages = Vec::with_capacity(generated.smart_page_ids.len());
        let mut registered_pages = Vec::with_capacity(generated.smart_page_ids.len());
        for (offset, smart_page_id) in generated.smart_page_ids.into_iter().enumerate() {
            let visible_number = starting_visible_page + offset as u32;
            let page_id = PageId::generate();
            let mut page = Page::new(
                page_id.clone(),
                PageKind::SmartPage {
                    smart_page_id: smart_page_id.clone(),
                    page_set_id: Some(generated.page_set_id.clone()),
                    visible_page_number: Some(visible_number),
                },
                generated.layout_id.clone(),
                None,
                a2d_domain::PageState::GeneratedNotScanned,
                created_at,
            );
            page.set_generated_pdf_asset(asset.id().clone(), created_at)?;
            registered_pages.push(RegisteredPage {
                page_id,
                smart_page_id,
            });
            pages.push(page);
        }

        let asset_id = asset.id().clone();
        let mut storage = self.lock_storage()?;
        storage
            .transaction(|tx| {
                tx.insert_page_set(&page_set)?;
                tx.insert_asset(&asset)?;
                for page in &pages {
                    tx.insert_page(page)?;
                }
                Ok(())
            })
            .map_err(|e| {
                e.with_detail("orphaned_asset_id", asset_id.to_string())
                    .with_detail(
                        "note",
                        "the PDF asset was durably committed to disk before this transaction \
                     failed; no database row references it",
                    )
            })?;

        Ok(RegisteredPageSet {
            page_set_id: generated.page_set_id,
            pages: registered_pages,
            pdf_asset_id: asset_id,
        })
    }
}

#[derive(Debug)]
pub struct RegisteredPage {
    pub page_id: PageId,
    pub smart_page_id: SmartPageId,
}

#[derive(Debug)]
pub struct RegisteredPageSet {
    pub page_set_id: PageSetId,
    pub pages: Vec<RegisteredPage>,
    pub pdf_asset_id: a2d_domain::AssetId,
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

    fn open_test_core() -> (Arc<A2dCore>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("a2d-core-test-{}", PageId::generate()));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        (core, dir)
    }

    fn sample_request() -> a2d_pdf::GeneratePageSetRequest {
        a2d_pdf::GeneratePageSetRequest {
            paper_size: a2d_layout::PaperSize::A4,
            style: a2d_layout::SmartPageStyle::Blank,
            page_count: 3,
            starting_visible_page: 1,
        }
    }

    #[test]
    fn generate_and_register_page_set_persists_the_page_set_pages_and_asset() {
        let (core, dir) = open_test_core();
        let registered = core
            .generate_and_register_page_set(sample_request())
            .unwrap();
        assert_eq!(registered.pages.len(), 3);

        // Inspect the library directly through a second, independent Storage/AssetStore handle,
        // the same way a real second process (or the next app launch) would.
        let storage = Storage::open(&dir.join("library.sqlite")).unwrap();
        let asset_store = AssetStore::open(&dir).unwrap();

        let page_set = storage.get_page_set(&registered.page_set_id).unwrap();
        assert!(page_set.is_some());

        for registered_page in &registered.pages {
            let page = storage.get_page(&registered_page.page_id).unwrap().unwrap();
            assert_eq!(page.state, a2d_domain::PageState::GeneratedNotScanned);
            assert_eq!(
                page.generated_pdf_asset_id,
                Some(registered.pdf_asset_id.clone())
            );
            match page.kind {
                PageKind::SmartPage { smart_page_id, .. } => {
                    assert_eq!(smart_page_id, registered_page.smart_page_id);
                }
                other => panic!("expected a SmartPage, got {other:?}"),
            }
        }

        // The asset the pages reference actually exists on disk with a verifiable hash -- not
        // just a database row with a dangling path.
        let asset = storage
            .get_asset(&registered.pdf_asset_id)
            .unwrap()
            .unwrap();
        asset_store.verify(&asset).unwrap();

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generate_and_register_page_set_rejects_zero_pages_before_touching_storage() {
        let (core, dir) = open_test_core();
        let mut request = sample_request();
        request.page_count = 0;
        let err = core.generate_and_register_page_set(request).unwrap_err();
        assert!(err.code.to_string().contains("PAGE_SET_EMPTY"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn transaction_failure_after_asset_commit_reports_a_real_orphan_and_rolls_back_rows() {
        let (core, dir) = open_test_core();

        // Deterministic real-SQL fault injection: the trigger lets the asset commit complete, then
        // aborts the first page_sets INSERT inside the registration transaction. This is reliable
        // across CI/container privilege models, unlike changing file permissions on an already
        // open WAL database, and it does not add a production-only mock seam.
        {
            let mut storage = core.lock_storage().unwrap();
            storage
                .transaction(|tx| {
                    tx.execute_batch(
                        "CREATE TRIGGER fail_page_set_registration_for_test \
                         BEFORE INSERT ON page_sets \
                         BEGIN \
                           SELECT RAISE(ABORT, 'forced page-set registration failure'); \
                         END;",
                    )
                    .expect("failure-injection trigger must be created");
                    Ok(())
                })
                .unwrap();
        }

        let err = core
            .generate_and_register_page_set(sample_request())
            .unwrap_err();
        let orphaned_asset_id = err
            .details
            .get("orphaned_asset_id")
            .expect("transaction failure after asset commit must identify the orphan")
            .clone();
        assert!(err.details.contains_key("note"));

        let orphaned_path = dir.join("assets").join("exports").join(&orphaned_asset_id);
        assert!(
            orphaned_path.is_file(),
            "the diagnostic must refer to the PDF file that was durably committed before SQL failed"
        );

        // The database transaction must have rolled back all attempted rows, including the Asset
        // row; only the deliberately orphaned filesystem object remains.
        let mut storage = Storage::open(&dir.join("library.sqlite")).unwrap();
        let (page_sets, pages, assets): (i64, i64, i64) = storage
            .transaction(|tx| {
                let page_sets = tx
                    .query_row("SELECT COUNT(*) FROM page_sets", [], |row| row.get(0))
                    .expect("page_sets count must be readable");
                let pages = tx
                    .query_row("SELECT COUNT(*) FROM pages", [], |row| row.get(0))
                    .expect("pages count must be readable");
                let assets = tx
                    .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
                    .expect("assets count must be readable");
                Ok((page_sets, pages, assets))
            })
            .unwrap();
        assert_eq!((page_sets, pages, assets), (0, 0, 0));

        drop(storage);
        drop(core);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repeated_generation_produces_fully_independent_page_sets() {
        let (core, dir) = open_test_core();
        let first = core
            .generate_and_register_page_set(sample_request())
            .unwrap();
        let second = core
            .generate_and_register_page_set(sample_request())
            .unwrap();

        assert_ne!(first.page_set_id, second.page_set_id);
        assert_ne!(first.pdf_asset_id, second.pdf_asset_id);
        let first_page_ids: std::collections::HashSet<_> =
            first.pages.iter().map(|p| p.page_id.clone()).collect();
        let second_page_ids: std::collections::HashSet<_> =
            second.pages.iter().map(|p| p.page_id.clone()).collect();
        assert!(first_page_ids.is_disjoint(&second_page_ids));

        std::fs::remove_dir_all(&dir).ok();
    }
}
