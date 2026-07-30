from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


source_path = Path("crates/a2d-core/src/milestone9.rs")
source = source_path.read_text()

source = replace_once(
    source,
    "use a2d_storage::{AssetRepository, AuditEventRepository, PageRepository, ScanRepository};",
    "use a2d_storage::{\n    AssetPersistenceFailureStage, AssetRepository, AuditEventRepository, PageRepository,\n    ScanRepository,\n};",
    "storage import",
)

helper = r'''fn with_scan_registration_rollback_details(
    mut error: A2dError,
    journal_path: &str,
    staging_path: &Path,
    scan: &Scan,
    audit: &AuditEvent,
    assets: [&Asset; 4],
) -> A2dError {
    let committed_asset_ids = assets
        .iter()
        .map(|asset| asset.id().to_string())
        .collect::<Vec<_>>()
        .join(",");
    error = error
        .with_detail(
            "asset_commit_failure_stage",
            AssetPersistenceFailureStage::DatabaseRegistrationRolledBack.as_detail_value(),
        )
        .with_detail("asset_commit_journal", journal_path)
        .with_detail("staging_path", staging_path.to_string_lossy())
        .with_detail("scan_id", scan.id().to_string())
        .with_detail("audit_event_id", audit.id().to_string())
        .with_detail("committed_asset_ids", committed_asset_ids)
        .with_detail("orphaned_asset_count", assets.len().to_string())
        .with_detail("final_file_created", "true")
        .with_detail("file_sync_completed", "true")
        .with_detail("directory_sync_completed", "true")
        .with_detail("database_registration_started", "true")
        .with_detail("database_registration_committed", "false")
        .with_detail(
            "recovery_action",
            "inspect_orphaned_final_assets_before_any_reviewed_recovery_action",
        );

    for (index, asset) in assets.into_iter().enumerate() {
        let prefix = format!("orphaned_asset_{index}");
        error = error
            .with_detail(format!("{prefix}_id"), asset.id().to_string())
            .with_detail(format!("{prefix}_kind"), format!("{:?}", asset.kind))
            .with_detail(
                format!("{prefix}_final_relative_path"),
                &asset.relative_path,
            )
            .with_detail(format!("{prefix}_expected_sha256"), &asset.sha256)
            .with_detail(
                format!("{prefix}_byte_length"),
                asset.byte_length.to_string(),
            );
    }
    error
}

'''
source = replace_once(
    source,
    "impl A2dCore {\n    pub fn register_scan(&self, request: RegisterScanRequest) -> Result<RegisteredScan, A2dError> {",
    helper
    + "impl A2dCore {\n    pub fn register_scan(&self, request: RegisterScanRequest) -> Result<RegisteredScan, A2dError> {\n        self.register_scan_with_transaction_guard(request, || Ok(()))\n    }\n\n    fn register_scan_with_transaction_guard<F>(\n        &self,\n        request: RegisterScanRequest,\n        before_transaction_commit: F,\n    ) -> Result<RegisteredScan, A2dError>\n    where\n        F: FnOnce() -> Result<(), A2dError>,\n    {",
    "register_scan wrapper",
)

source = replace_once(
    source,
    "            }\n            Ok(())\n        });\n        transaction_result.map_err(|error| {",
    "            }\n            before_transaction_commit()?;\n            Ok(())\n        });\n        transaction_result.map_err(|error| {",
    "pre-commit transaction guard",
)

source = replace_once(
    source,
    '''        transaction_result.map_err(|error| {
            error
                .with_detail("asset_commit_journal", journal_path.clone())
                .with_detail("staging_path", staged.canonical_path.to_string_lossy())
                .with_detail(
                    "committed_asset_ids",
                    [original.id(), corrected.id(), ocr.id(), thumbnail.id()]
                        .into_iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                )
        })?;''',
    '''        transaction_result.map_err(|error| {
            with_scan_registration_rollback_details(
                error,
                &journal_path,
                &staged.canonical_path,
                &scan,
                &audit,
                [&original, &corrected, &ocr, &thumbnail],
            )
        })?;''',
    "rollback error mapping",
)

source_path.write_text(source)

tests_path = Path("crates/a2d-core/src/milestone9_tests.rs")
tests = tests_path.read_text()
tests = replace_once(
    tests,
    '''use a2d_domain::{
    CaptureSource, LayoutId, Notebook, NotebookDesign, NotebookDesignId, NotebookId, Page, PageId,
    PageKind, PageState, QualityStatus, ScanId, SmartPageId, TrimSizeMm, TrustState,
};''',
    '''use a2d_domain::{
    AssetId, AuditEventId, CaptureSource, ErrorCategory, LayoutId, Notebook, NotebookDesign,
    NotebookDesignId, NotebookId, Page, PageId, PageKind, PageState, QualityStatus, ScanId,
    SmartPageId, TrimSizeMm, TrustState,
};''',
    "domain test imports",
)
tests = replace_once(
    tests,
    "use a2d_storage::{NotebookDesignRepository, NotebookRepository, PageRepository, ScanRepository};",
    "use a2d_storage::{\n    AssetRepository, AuditEventRepository, NotebookDesignRepository, NotebookRepository,\n    PageRepository, ScanRepository,\n};",
    "storage test imports",
)

rollback_test = r'''#[test]
fn database_failure_rolls_back_all_scan_rows_and_reports_each_finalized_asset() {
    let (core, root, page_id, notebook_id, payload) = test_core();
    let failed_request = request(
        &root,
        page_id.clone(),
        notebook_id.clone(),
        payload.clone(),
        "rollback.png",
    );
    let staging_path = PathBuf::from(&failed_request.staging_path);

    let error = core
        .register_scan_with_transaction_guard(failed_request, || {
            Err(registration_error(
                "CORE_SCAN_TEST_TRANSACTION_FAILURE",
                ErrorCategory::Storage,
                "forced failure immediately before the generic scan-registration transaction commit",
            ))
        })
        .unwrap_err();

    assert_eq!(error.code.to_string(), "CORE_SCAN_TEST_TRANSACTION_FAILURE");
    assert_eq!(
        error
            .details
            .get("asset_commit_failure_stage")
            .map(String::as_str),
        Some("database_registration_rolled_back")
    );
    assert_eq!(
        error
            .details
            .get("database_registration_started")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        error
            .details
            .get("database_registration_committed")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        error.details.get("file_sync_completed").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        error
            .details
            .get("directory_sync_completed")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        error.details.get("orphaned_asset_count").map(String::as_str),
        Some("4")
    );
    assert!(staging_path.is_file());

    let journal_path = PathBuf::from(error.details.get("asset_commit_journal").unwrap());
    assert!(journal_path.is_file());
    let journal = std::fs::read_to_string(&journal_path).unwrap();
    assert_eq!(
        journal
            .lines()
            .filter(|line| line.contains("\"phase\":\"asset_committed\""))
            .count(),
        4
    );
    assert!(!journal.contains("\"phase\":\"database_committed\""));

    let scan_id = ScanId::parse(error.details.get("scan_id").unwrap()).unwrap();
    let audit_event_id =
        AuditEventId::parse(error.details.get("audit_event_id").unwrap()).unwrap();
    let mut evidence = Vec::new();
    for index in 0..4 {
        let prefix = format!("orphaned_asset_{index}");
        evidence.push((
            AssetId::parse(error.details.get(&format!("{prefix}_id")).unwrap()).unwrap(),
            error
                .details
                .get(&format!("{prefix}_kind"))
                .unwrap()
                .clone(),
            error
                .details
                .get(&format!("{prefix}_final_relative_path"))
                .unwrap()
                .clone(),
            error
                .details
                .get(&format!("{prefix}_expected_sha256"))
                .unwrap()
                .clone(),
            error
                .details
                .get(&format!("{prefix}_byte_length"))
                .unwrap()
                .parse::<u64>()
                .unwrap(),
        ));
    }

    {
        let storage = core.lock_storage().unwrap();
        assert_eq!(storage.get_scan(&scan_id).unwrap(), None);
        assert_eq!(storage.get_audit_event(&audit_event_id).unwrap(), None);
        let page = storage.get_page(&page_id).unwrap().unwrap();
        assert_eq!(page.state, PageState::Unscanned);
        assert_eq!(page.preferred_scan_id, None);
        for (asset_id, _, relative_path, _, _) in &evidence {
            assert_eq!(storage.get_asset(asset_id).unwrap(), None);
            assert!(root.join(relative_path).is_file());
        }

        let orphans = storage.discover_orphaned_final_assets(&root).unwrap();
        assert_eq!(orphans.len(), 4);
        for orphan in &orphans {
            let (_, expected_kind, expected_path, expected_sha256, expected_length) = evidence
                .iter()
                .find(|(asset_id, _, _, _, _)| asset_id == &orphan.asset_id)
                .expect("every discovered orphan must have immutable rollback evidence");
            assert_eq!(&format!("{:?}", orphan.kind), expected_kind);
            assert_eq!(&orphan.relative_path, expected_path);
            assert_eq!(&orphan.sha256, expected_sha256);
            assert_eq!(&orphan.byte_length, expected_length);
        }
    }

    let retried = core
        .register_scan(request(
            &root,
            page_id,
            notebook_id,
            payload,
            "rollback-retry.png",
        ))
        .unwrap();
    let retry_asset_ids = [
        retried.original_asset_id,
        retried.corrected_asset_id,
        retried.ocr_asset_id,
        retried.thumbnail_asset_id,
    ];
    for (orphaned_id, _, _, _, _) in &evidence {
        assert!(!retry_asset_ids.contains(orphaned_id));
    }

    let storage = core.lock_storage().unwrap();
    let remaining_orphans = storage.discover_orphaned_final_assets(&root).unwrap();
    assert_eq!(remaining_orphans.len(), 4);
    for orphan in &remaining_orphans {
        let (_, _, expected_path, expected_sha256, expected_length) = evidence
            .iter()
            .find(|(asset_id, _, _, _, _)| asset_id == &orphan.asset_id)
            .expect("retry must preserve every prior orphan without replacement");
        assert_eq!(&orphan.relative_path, expected_path);
        assert_eq!(&orphan.sha256, expected_sha256);
        assert_eq!(&orphan.byte_length, expected_length);
    }
    assert!(staging_path.is_file());
    assert!(journal_path.is_file());

    drop(storage);
    drop(core);
    std::fs::remove_dir_all(root).ok();
}

'''
tests = replace_once(
    tests,
    "#[test]\nfn incomplete_journal_is_retained_until_explicit_completion() {",
    rollback_test + "#[test]\nfn incomplete_journal_is_retained_until_explicit_completion() {",
    "rollback regression insertion",
)
tests_path.write_text(tests)
