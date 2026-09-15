use a2d_domain::{
    Asset, AssetId, AssetKind, CaptureSource, EncryptionState, LayoutId, OcrJobId, Page, PageId,
    PageKind, PageState, QualityStatus, Scan, ScanId, SmartPageId,
};
use a2d_storage::{
    AssetRepository, OcrJobRepository, PageRepository, PersistedOcrInputKind, PersistedOcrJob,
    PersistedOcrJobStatus, PersistedOcrProviderAvailability, ScanRepository, Storage,
};

fn fixture(storage: &Storage) -> (ScanId, AssetId) {
    let page_id = PageId::generate();
    let scan_id = ScanId::generate();
    let asset_id = AssetId::generate();
    storage
        .insert_page(&Page::new(
            page_id.clone(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            LayoutId::parse("PAGE").unwrap(),
            Some("OCR queue page".to_string()),
            PageState::GeneratedNotScanned,
            100,
        ))
        .unwrap();
    storage
        .insert_asset(&Asset::new(
            asset_id.clone(),
            AssetKind::Original,
            format!("assets/originals/{asset_id}.png"),
            "image/png".to_string(),
            4,
            "queue-test-sha256".to_string(),
            100,
            true,
            EncryptionState::Plaintext,
        ))
        .unwrap();
    storage
        .insert_scan(&Scan::new(
            scan_id.clone(),
            page_id,
            None,
            CaptureSource::Camera,
            125,
            asset_id.clone(),
            None,
            None,
            None,
            "test-pipeline".to_string(),
            QualityStatus::Accepted,
            Vec::new(),
            true,
            None,
            format!(
                "scan-content-v1;corrected-sha256={};perceptual=mean-grid-16x24-v1:{}",
                "a".repeat(64),
                "b".repeat(16 * 24 * 2)
            ),
        ))
        .unwrap();
    (scan_id, asset_id)
}

fn queued_job(scan_id: ScanId, asset_id: AssetId, created_at_ms: i64) -> PersistedOcrJob {
    PersistedOcrJob {
        id: OcrJobId::generate(),
        scan_id,
        input_asset_id: asset_id,
        input_kind: PersistedOcrInputKind::Original,
        media_type: "image/png".to_string(),
        relative_path: "assets/originals/input.png".to_string(),
        byte_length: 4,
        width_px: 100,
        height_px: 200,
        status: PersistedOcrJobStatus::Queued,
        created_at_ms,
        updated_at_ms: created_at_ms,
        last_started_at_ms: None,
        completed_at_ms: None,
        attempt_count: 0,
        retryable: true,
        next_retry_at_ms: None,
        last_error_code: None,
        last_error_message: None,
        provider: None,
        provider_version: None,
        model_name: None,
        provider_availability: PersistedOcrProviderAvailability::Unknown,
        last_ocr_run_id: None,
        cancellation_requested: false,
    }
}

#[test]
fn queued_running_and_terminal_diagnostics_round_trip_durably() {
    let storage = Storage::open_in_memory().unwrap();
    let (scan_id, asset_id) = fixture(&storage);
    let mut job = queued_job(scan_id, asset_id, 200);
    let id = job.id.clone();

    storage.insert_ocr_job(&job).unwrap();
    assert_eq!(storage.count_active_ocr_jobs().unwrap(), 1);
    assert_eq!(storage.next_due_ocr_job(200).unwrap().unwrap().id, id);

    job.status = PersistedOcrJobStatus::Running;
    job.updated_at_ms = 220;
    job.last_started_at_ms = Some(220);
    job.attempt_count = 1;
    storage.update_ocr_job(&job).unwrap();

    let running = storage.get_ocr_job(&id).unwrap().unwrap();
    assert_eq!(running.status, PersistedOcrJobStatus::Running);
    assert_eq!(running.attempt_count, 1);

    job.status = PersistedOcrJobStatus::Cancelled;
    job.updated_at_ms = 240;
    job.completed_at_ms = Some(240);
    job.retryable = false;
    job.cancellation_requested = true;
    storage.update_ocr_job(&job).unwrap();

    let terminal = storage.get_ocr_job(&id).unwrap().unwrap();
    assert_eq!(terminal.status, PersistedOcrJobStatus::Cancelled);
    assert!(terminal.cancellation_requested);
    assert_eq!(storage.count_active_ocr_jobs().unwrap(), 0);
}

#[test]
fn retry_delay_and_fifo_claiming_are_persisted() {
    let storage = Storage::open_in_memory().unwrap();
    let (scan_one, asset_one) = fixture(&storage);
    let (scan_two, asset_two) = fixture(&storage);
    let mut first = queued_job(scan_one, asset_one, 100);
    first.next_retry_at_ms = Some(500);
    let second = queued_job(scan_two, asset_two, 200);

    storage.insert_ocr_job(&first).unwrap();
    storage.insert_ocr_job(&second).unwrap();

    assert_eq!(
        storage.next_due_ocr_job(300).unwrap().unwrap().id,
        second.id
    );
    assert_eq!(storage.next_due_ocr_job(500).unwrap().unwrap().id, first.id);
}

#[test]
fn one_active_job_per_work_key_is_enforced() {
    let storage = Storage::open_in_memory().unwrap();
    let (scan_id, asset_id) = fixture(&storage);
    let first = queued_job(scan_id.clone(), asset_id.clone(), 100);
    let second = queued_job(scan_id, asset_id, 101);

    storage.insert_ocr_job(&first).unwrap();
    let error = storage.insert_ocr_job(&second).unwrap_err();

    assert_eq!(error.code.to_string(), "STORAGE_CONSTRAINT_VIOLATION");
}
