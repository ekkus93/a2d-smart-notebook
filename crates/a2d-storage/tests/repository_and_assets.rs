//! Integration tests for the repository layer (TODO 3.2), the asset commit protocol (TODO 3.3),
//! and the interruption/failure scenarios TODO 3.4 asks for. Exercises the public API only, the
//! same way a future consumer crate (`a2d-core`) would.

use std::collections::BTreeMap;

use a2d_domain::{
    AssetKind, AuditEvent, CaptureSource, EncryptionState, LayoutId, Notebook, NotebookDesign,
    NotebookDesignId, NotebookId, OcrRun, Page, PageId, PageKind, PageSet, PageSetId, PageState,
    Provenance, QualityStatus, Scan, ScanId, SmartPageId, TrimSizeMm, TrustState,
};
use a2d_storage::{
    AssetRepository, AssetStore, AuditEventRepository, NotebookDesignRepository,
    NotebookRepository, OcrRunRepository, PageRepository, PageSetRepository, ScanRepository,
    Storage,
};

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "a2d-storage-integration-{label}-{}",
        PageId::generate()
    ))
}

fn sample_design() -> NotebookDesign {
    NotebookDesign::new(
        NotebookDesignId::generate(),
        1,
        "Everyday Notebook".to_string(),
        1,
        TrimSizeMm {
            width: 210,
            height: 297,
        },
        100,
        LayoutId::parse("SETUP-V1").unwrap(),
        LayoutId::parse("PAGE-V1").unwrap(),
        "apriltag".to_string(),
        vec![
            "TL".to_string(),
            "TR".to_string(),
            "BL".to_string(),
            "BR".to_string(),
        ],
        "deadbeef".to_string(),
        TrustState::Trusted,
    )
}

fn sample_notebook(design_id: &NotebookDesignId) -> Notebook {
    Notebook::new(
        NotebookId::generate(),
        design_id.clone(),
        "My Notebook".to_string(),
        1_000,
        1_000,
        None,
        true,
        None,
        None,
        None,
    )
}

#[test]
fn notebook_design_round_trips_through_insert_and_get() {
    let storage = Storage::open_in_memory().unwrap();
    let design = sample_design();
    storage.insert_notebook_design(&design).unwrap();
    let fetched = storage.get_notebook_design(design.id()).unwrap().unwrap();
    assert_eq!(fetched, design);
}

#[test]
fn get_notebook_design_returns_none_for_an_unknown_id() {
    let storage = Storage::open_in_memory().unwrap();
    assert_eq!(
        storage
            .get_notebook_design(&NotebookDesignId::generate())
            .unwrap(),
        None
    );
}

#[test]
fn notebook_round_trips_and_requires_its_design_to_exist() {
    let storage = Storage::open_in_memory().unwrap();
    let design = sample_design();
    storage.insert_notebook_design(&design).unwrap();
    let notebook = sample_notebook(design.id());
    storage.insert_notebook(&notebook).unwrap();
    let fetched = storage.get_notebook(notebook.id()).unwrap().unwrap();
    assert_eq!(fetched, notebook);

    // Foreign key: a notebook referencing a design that was never inserted must fail. Mapped as
    // a Validation error, same as other constraint violations -- a bad reference is caller
    // error, not an infrastructure failure.
    let mut orphan = sample_notebook(&NotebookDesignId::generate());
    // Isolate the foreign-key behavior under test. Milestone 6 adds a separate invariant that
    // only one notebook may be the active scan destination, and `notebook` above is already active.
    orphan.active_scan_destination = false;
    let err = storage.insert_notebook(&orphan).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Validation);
    assert!(err.code.to_string().contains("FOREIGN_KEY_VIOLATION"));
}

/// TODO 4.1 "detect persistence collisions as hard integrity events": re-inserting a row under
/// an ID that already exists collides on the table's primary key, not an ordinary unique index,
/// so it must map to a distinct Integrity/Critical error rather than an everyday Validation one.
#[test]
fn reinserting_an_existing_id_is_reported_as_an_integrity_event_not_a_validation_error() {
    let storage = Storage::open_in_memory().unwrap();
    let design = sample_design();
    storage.insert_notebook_design(&design).unwrap();
    let err = storage.insert_notebook_design(&design).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Integrity);
    assert_eq!(err.severity, a2d_domain::ErrorSeverity::Critical);
    assert!(err.code.to_string().contains("ID_COLLISION"));
}

#[test]
fn page_set_and_smart_page_round_trip() {
    let storage = Storage::open_in_memory().unwrap();
    let page_set = PageSet::new(PageSetId::generate(), Some("Trip Notes".to_string()), 500);
    storage.insert_page_set(&page_set).unwrap();

    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: Some(page_set.id().clone()),
            visible_page_number: Some(3),
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        500,
    );
    storage.insert_page(&page).unwrap();
    let fetched = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(fetched, page);
}

#[test]
fn set_generated_pdf_asset_attaches_a_committed_asset_and_round_trips() {
    let storage = Storage::open_in_memory().unwrap();
    let asset_store = AssetStore::open(&temp_dir("generated-pdf-asset")).unwrap();
    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        500,
    );
    storage.insert_page(&page).unwrap();
    assert_eq!(
        storage
            .get_page(page.id())
            .unwrap()
            .unwrap()
            .generated_pdf_asset_id,
        None
    );

    let asset = asset_store
        .commit(b"%PDF-1.7 fake bytes", AssetKind::Export, "application/pdf")
        .unwrap();
    storage.insert_asset(&asset).unwrap();
    storage
        .set_generated_pdf_asset(page.id(), asset.id())
        .unwrap();
    // Repeating the exact assignment is idempotent and must remain safe.
    storage
        .set_generated_pdf_asset(page.id(), asset.id())
        .unwrap();

    let fetched = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(fetched.generated_pdf_asset_id, Some(asset.id().clone()));

    // A different assignment is an explicit conflict and must preserve the original association.
    let replacement = asset_store
        .commit(
            b"%PDF-1.7 replacement bytes",
            AssetKind::Export,
            "application/pdf",
        )
        .unwrap();
    storage.insert_asset(&replacement).unwrap();
    let err = storage
        .set_generated_pdf_asset(page.id(), replacement.id())
        .unwrap_err();
    assert_eq!(err.code.to_string(), "STORAGE_GENERATED_PDF_ASSET_CONFLICT");
    let fetched = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(fetched.generated_pdf_asset_id, Some(asset.id().clone()));
}

#[test]
fn set_generated_pdf_asset_rejects_an_unknown_page_id() {
    let storage = Storage::open_in_memory().unwrap();
    let asset_store = AssetStore::open(&temp_dir("generated-pdf-asset-missing-page")).unwrap();
    let asset = asset_store
        .commit(b"%PDF-1.7 fake bytes", AssetKind::Export, "application/pdf")
        .unwrap();
    storage.insert_asset(&asset).unwrap();

    let err = storage
        .set_generated_pdf_asset(&PageId::generate(), asset.id())
        .unwrap_err();
    assert!(err.code.to_string().contains("PAGE_NOT_FOUND"));
}

#[test]
fn notebook_page_round_trips_and_enforces_unique_logical_page_number() {
    let storage = Storage::open_in_memory().unwrap();
    let design = sample_design();
    storage.insert_notebook_design(&design).unwrap();
    let notebook = sample_notebook(design.id());
    storage.insert_notebook(&notebook).unwrap();

    let page = Page::new(
        PageId::generate(),
        PageKind::NotebookPage {
            notebook_id: notebook.id().clone(),
            design_id: design.id().clone(),
            logical_page_number: 1,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        Some("Page one".to_string()),
        PageState::Scanned,
        900,
    );
    storage.insert_page(&page).unwrap();
    assert_eq!(storage.get_page(page.id()).unwrap().unwrap(), page);

    let duplicate = Page::new(
        PageId::generate(),
        PageKind::NotebookPage {
            notebook_id: notebook.id().clone(),
            design_id: design.id().clone(),
            logical_page_number: 1,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        901,
    );
    let err = storage.insert_page(&duplicate).unwrap_err();
    // A business-rule unique index (not the primary key -- `duplicate` has its own fresh
    // `PageId`) stays a Validation error, distinct from the ID-collision Integrity case above.
    assert_eq!(err.category, a2d_domain::ErrorCategory::Validation);
    assert!(err.code.to_string().contains("UNIQUE_CONSTRAINT_VIOLATION"));
}

/// Mirrors TODO 3.2's own example almost verbatim: insert a page, insert a scan, set the
/// page's preferred scan, insert an audit event, all inside one transaction.
#[test]
fn scan_registration_composes_through_one_transaction_matching_the_todo_example() {
    let mut storage = Storage::open_in_memory().unwrap();
    let asset_store = AssetStore::open(&temp_dir("scan-registration")).unwrap();

    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        100,
    );
    let original = asset_store
        .commit(b"fake jpeg bytes", AssetKind::Original, "image/jpeg")
        .unwrap();
    let scan = Scan::new(
        ScanId::generate(),
        page.id().clone(),
        None,
        CaptureSource::Camera,
        200,
        original.id().clone(),
        None,
        None,
        None,
        "v1".to_string(),
        QualityStatus::Accepted,
        vec![],
        true,
        None,
        "fingerprint-1".to_string(),
    );
    let event = AuditEvent::new(
        a2d_domain::AuditEventId::generate(),
        200,
        "scan.registered".to_string(),
        "installation".to_string(),
        Some(scan.id().to_string()),
        BTreeMap::new(),
        None,
    );

    storage
        .transaction(|tx| {
            tx.insert_asset(&original)?;
            tx.insert_page(&page)?;
            tx.insert_scan(&scan)?;
            tx.set_preferred_scan(page.id(), scan.id())?;
            tx.insert_audit_event(&event)?;
            Ok(())
        })
        .unwrap();

    let stored_page = storage.get_page(page.id()).unwrap().unwrap();
    assert_eq!(stored_page.preferred_scan_id, Some(scan.id().clone()));
    assert_eq!(storage.get_scan(scan.id()).unwrap().unwrap(), scan);
    assert_eq!(storage.get_asset(original.id()).unwrap().unwrap(), original);
    assert_eq!(storage.get_audit_event(event.id()).unwrap().unwrap(), event);
}

#[test]
fn a_failing_step_rolls_back_every_earlier_write_in_the_same_transaction() {
    let mut storage = Storage::open_in_memory().unwrap();
    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        100,
    );

    let result = storage.transaction(|tx| {
        tx.insert_page(&page)?;
        // set_preferred_scan against a scan id that was never inserted -- this doesn't itself
        // fail (no FK on preferred_scan_id's *value* being a real scan at UPDATE time beyond
        // the column existing), so force a real failure: insert the same page id again.
        tx.insert_page(&page)
    });
    assert!(result.is_err());

    // The first insert_page call inside the failed transaction must not be visible either --
    // the whole transaction rolled back, not just the failing statement.
    assert_eq!(storage.get_page(page.id()).unwrap(), None);
}

#[test]
fn ocr_run_round_trips_with_provenance() {
    let storage = Storage::open_in_memory().unwrap();
    let asset_store = AssetStore::open(&temp_dir("ocr-run")).unwrap();

    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::Scanned,
        10,
    );
    storage.insert_page(&page).unwrap();
    let original = asset_store
        .commit(b"scan bytes", AssetKind::Original, "image/jpeg")
        .unwrap();
    storage.insert_asset(&original).unwrap();
    let scan = Scan::new(
        ScanId::generate(),
        page.id().clone(),
        None,
        CaptureSource::Camera,
        20,
        original.id().clone(),
        None,
        None,
        None,
        "v1".to_string(),
        QualityStatus::Accepted,
        vec![],
        true,
        None,
        "fingerprint-ocr".to_string(),
    );
    storage.insert_scan(&scan).unwrap();
    let scan_id = scan.id().clone();

    let run = OcrRun::new(
        a2d_domain::OcrRunId::generate(),
        scan_id.clone(),
        "mlkit".to_string(),
        "1.0".to_string(),
        "hello world".to_string(),
        vec!["low_confidence_region".to_string()],
        Provenance {
            source_page_id: None,
            source_scan_id: Some(scan_id),
            producing_component: "a2d-ocr".to_string(),
            component_version: "0.1.0".to_string(),
            created_at_ms: 42,
            warnings: vec![],
            user_approved: Some(false),
        },
    );
    storage.insert_ocr_run(&run).unwrap();
    assert_eq!(storage.get_ocr_run(run.id()).unwrap().unwrap(), run);
}

fn sample_scan(page_id: PageId, original_asset_id: a2d_domain::AssetId) -> Scan {
    Scan::new(
        ScanId::generate(),
        page_id,
        None,
        CaptureSource::Camera,
        20,
        original_asset_id,
        None,
        None,
        None,
        "v1".to_string(),
        QualityStatus::Accepted,
        vec![],
        true,
        None,
        "fingerprint".to_string(),
    )
}

/// TODO 2.3's "a scan always references an immutable original asset", closed now that storage
/// exists to check it -- deferred at the time that invariant was written.
#[test]
fn insert_scan_rejects_an_original_asset_that_is_not_immutable() {
    let storage = Storage::open_in_memory().unwrap();
    let asset_store = AssetStore::open(&temp_dir("scan-not-immutable")).unwrap();
    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        0,
    );
    storage.insert_page(&page).unwrap();

    // Corrected assets are not marked immutable -- using one as a scan's "original" is exactly
    // the mistake this check exists to catch.
    let not_original = asset_store
        .commit(b"not an original", AssetKind::Corrected, "image/jpeg")
        .unwrap();
    storage.insert_asset(&not_original).unwrap();

    let scan = sample_scan(page.id().clone(), not_original.id().clone());
    let err = storage.insert_scan(&scan).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Validation);
    assert!(
        err.code
            .to_string()
            .contains("ORIGINAL_ASSET_NOT_IMMUTABLE")
    );
    assert_eq!(storage.get_scan(scan.id()).unwrap(), None);
}

#[test]
fn insert_scan_rejects_an_original_asset_that_does_not_exist() {
    let storage = Storage::open_in_memory().unwrap();
    let page = Page::new(
        PageId::generate(),
        PageKind::SmartPage {
            smart_page_id: SmartPageId::generate(),
            page_set_id: None,
            visible_page_number: None,
        },
        LayoutId::parse("PAGE-V1").unwrap(),
        None,
        PageState::GeneratedNotScanned,
        0,
    );
    storage.insert_page(&page).unwrap();

    let scan = sample_scan(page.id().clone(), a2d_domain::AssetId::generate());
    let err = storage.insert_scan(&scan).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Validation);
    assert!(err.code.to_string().contains("ORIGINAL_ASSET_MISSING"));
}

// --- AssetStore (TODO 3.3) ---------------------------------------------------------------
#[test]
fn commit_writes_a_durable_file_with_a_verified_hash() {
    let dir = temp_dir("commit-basic");
    let store = AssetStore::open(&dir).unwrap();
    let data = b"hello, a2d";
    let asset = store
        .commit(data, AssetKind::Corrected, "image/png")
        .unwrap();

    let expected_sha256 = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(asset.sha256, expected_sha256);
    assert_eq!(asset.byte_length, data.len() as u64);
    assert!(!asset.immutable);

    let resolved = store.resolve(&asset.relative_path).unwrap();
    assert_eq!(std::fs::read(&resolved).unwrap(), data);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn original_assets_are_marked_immutable_and_read_only_on_disk() {
    let dir = temp_dir("commit-original");
    let store = AssetStore::open(&dir).unwrap();
    let asset = store
        .commit(b"original bytes", AssetKind::Original, "image/jpeg")
        .unwrap();
    assert!(asset.immutable);
    assert_eq!(asset.encryption_state, EncryptionState::Plaintext);

    let resolved = store.resolve(&asset.relative_path).unwrap();
    let perms = std::fs::metadata(&resolved).unwrap().permissions();
    assert!(perms.readonly());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn commit_never_leaves_a_temp_file_behind_on_success() {
    let dir = temp_dir("commit-no-orphan");
    let store = AssetStore::open(&dir).unwrap();
    store
        .commit(b"data", AssetKind::Thumbnail, "image/png")
        .unwrap();
    assert!(store.list_orphaned_temp_files().unwrap().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// Simulates the "interrupted after temp write" scenario TODO 3.4 asks for: a temp file exists
/// (as if a process died between the write and the atomic rename) but was never committed to
/// the database. `list_orphaned_temp_files` must find it, and finding it must not delete it.
#[test]
fn an_interrupted_write_leaves_a_detectable_orphan_that_is_not_deleted() {
    let dir = temp_dir("commit-orphan");
    let store = AssetStore::open(&dir).unwrap();
    let orphan_path = dir.join("tmp").join("interrupted-write.tmp");
    std::fs::write(&orphan_path, b"partial data").unwrap();

    let orphans = store.list_orphaned_temp_files().unwrap();
    assert_eq!(orphans, vec![orphan_path.clone()]);
    assert!(orphan_path.exists(), "detection must not delete the orphan");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn resolve_rejects_a_relative_path_that_escapes_the_library_root() {
    let dir = temp_dir("path-escape");
    let store = AssetStore::open(&dir).unwrap();
    // A file that genuinely exists outside root, referenced via a traversal-shaped relative
    // path, so canonicalize() succeeds and the containment check is what has to catch it.
    let outside = std::env::temp_dir().join(format!("a2d-outside-{}", PageId::generate()));
    std::fs::write(&outside, b"should not be reachable").unwrap();
    let traversal = format!("../{}", outside.file_name().unwrap().to_str().unwrap());

    let err = store.resolve(&traversal).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Integrity);
    assert!(err.code.to_string().contains("ESCAPES_ROOT"));

    std::fs::remove_file(&outside).ok();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn verify_succeeds_for_a_freshly_committed_asset() {
    let dir = temp_dir("verify-ok");
    let store = AssetStore::open(&dir).unwrap();
    let asset = store
        .commit(b"trustworthy bytes", AssetKind::Corrected, "image/png")
        .unwrap();
    store.verify(&asset).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn verify_detects_a_missing_asset_file() {
    let dir = temp_dir("verify-missing");
    let store = AssetStore::open(&dir).unwrap();
    let asset = store
        .commit(b"will be deleted", AssetKind::Corrected, "image/png")
        .unwrap();
    std::fs::remove_file(store.resolve(&asset.relative_path).unwrap()).unwrap();

    let err = store.verify(&asset).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Integrity);
    assert!(err.code.to_string().contains("ASSET_MISSING"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn verify_detects_a_hash_mismatch() {
    let dir = temp_dir("verify-mismatch");
    let store = AssetStore::open(&dir).unwrap();
    let asset = store
        .commit(b"original content", AssetKind::Corrected, "image/png")
        .unwrap();
    // Simulate on-disk corruption after the fact -- the immutable/read-only bit only applies to
    // Original assets, so a Corrected asset's file is writable here.
    std::fs::write(
        store.resolve(&asset.relative_path).unwrap(),
        b"tampered content",
    )
    .unwrap();

    let err = store.verify(&asset).unwrap_err();
    assert_eq!(err.category, a2d_domain::ErrorCategory::Integrity);
    assert!(err.code.to_string().contains("ASSET_HASH_MISMATCH"));
    std::fs::remove_dir_all(&dir).ok();
}

/// TODO 3.4: "A committed scan can never reference an original file that was never durably
/// written" -- proven structurally here, not just asserted: `commit` returns the `Asset` only
/// after the atomic rename succeeds, and the DB row is only inserted afterward using that
/// returned value. There is no code path that constructs an `Asset` (and therefore no path that
/// could insert its row) before the rename happens.
#[test]
fn asset_row_is_only_insertable_after_the_file_is_durably_renamed_into_place() {
    let dir = temp_dir("commit-then-insert");
    let store = AssetStore::open(&dir).unwrap();
    let storage = Storage::open_in_memory().unwrap();

    let asset = store
        .commit(b"durable bytes", AssetKind::Export, "text/plain")
        .unwrap();
    // By the time `asset` exists at all, the file is already at its final path.
    let final_path = store.resolve(&asset.relative_path).unwrap();
    assert!(final_path.exists());

    storage.insert_asset(&asset).unwrap();
    assert_eq!(storage.get_asset(asset.id()).unwrap().unwrap(), asset);
    std::fs::remove_dir_all(&dir).ok();
}
