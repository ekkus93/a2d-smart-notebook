//! Renders Smart Pages, Page Sets, and the development bound-notebook proof interior.

use std::fmt::Debug;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesignId, PageSetId,
    SmartPageId,
};
use a2d_identity::qr::PageCode;
use a2d_layout::notebook::{
    pdf_page_number_for_logical_page, setup_page_layout, writable_page_layout,
};
use a2d_layout::page_layout::PageLayout;
use a2d_layout::smart_page::{PaperSize, SmartPageStyle, smart_page_layout};
use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfParseOptions, PdfSaveOptions};

use crate::error::{io_error, verify_error};
use crate::render::render_page_ops;

/// Portable resource-safety ceiling for one generated Page Set. Android mirrors this value for
/// immediate form feedback, but Rust enforces it for every caller.
pub const MAX_PAGE_SET_PAGE_COUNT: u32 = 500;
/// Development proof interiors are bounded independently because every logical page adds a recto
/// and a verso in addition to the setup pair.
pub const MAX_NOTEBOOK_PROOF_LOGICAL_PAGE_COUNT: u32 = 500;
/// Maximum serialized output accepted by the in-memory standalone generator.
pub const MAX_PDF_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

const MAX_QR_V1_VISIBLE_PAGE_NUMBER: u32 = 999_999;
const MAX_PDF_PAGE_COUNT: usize = 2 + (MAX_NOTEBOOK_PROOF_LOGICAL_PAGE_COUNT as usize * 2);
const TEMP_CREATE_ATTEMPTS: u64 = 64;
static NEXT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn pdf_page_for(layout: &PageLayout, ops: Vec<Op>) -> PdfPage {
    PdfPage::new(
        Mm(layout.physical_size.width_mm as f32),
        Mm(layout.physical_size.height_mm as f32),
        ops,
    )
}

fn blank_verso_page(layout: &PageLayout) -> PdfPage {
    pdf_page_for(layout, Vec::new())
}

fn validation_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.pdf.invalid_request",
        message.into(),
        false,
    )
}

fn output_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        ErrorSeverity::Error,
        "error.pdf.output_failed",
        message.into(),
        retryable,
    )
}

fn parent_directory(output_path: &Path) -> &Path {
    output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn validate_output_target(output_path: &Path) -> Result<(), A2dError> {
    let parent = parent_directory(output_path);
    let metadata = std::fs::metadata(parent).map_err(|error| {
        io_error(format!(
            "reading standalone PDF destination directory {}: {error}",
            parent.display(),
        ))
        .with_detail("destination_directory", parent.to_string_lossy())
    })?;
    if !metadata.is_dir() {
        return Err(output_error(
            "PDF_OUTPUT_PARENT_NOT_DIRECTORY",
            ErrorCategory::Validation,
            "standalone PDF destination parent is not a directory",
            false,
        )
        .with_detail("destination_directory", parent.to_string_lossy()));
    }
    if output_path.file_name().is_none() {
        return Err(output_error(
            "PDF_OUTPUT_FILENAME_MISSING",
            ErrorCategory::Validation,
            "standalone PDF destination must include a file name",
            false,
        ));
    }
    match std::fs::symlink_metadata(output_path) {
        Ok(_) => Err(output_error(
            "PDF_OUTPUT_ALREADY_EXISTS",
            ErrorCategory::Integrity,
            "standalone PDF destination already exists and was not replaced",
            false,
        )
        .with_detail("output_path", output_path.to_string_lossy())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(format!(
            "checking standalone PDF destination {}: {error}",
            output_path.display(),
        ))
        .with_detail("output_path", output_path.to_string_lossy())),
    }
}

fn create_unique_temp(output_path: &Path) -> Result<(std::fs::File, PathBuf), A2dError> {
    let parent = parent_directory(output_path);
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            output_error(
                "PDF_OUTPUT_FILENAME_INVALID",
                ErrorCategory::Validation,
                "standalone PDF destination file name is not valid UTF-8",
                false,
            )
        })?;

    for _ in 0..TEMP_CREATE_ATTEMPTS {
        let sequence = NEXT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{file_name}.a2d-{}-{sequence}.tmp",
            std::process::id(),
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((file, temp_path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(io_error(format!(
                    "creating unique standalone PDF temp file {}: {error}",
                    temp_path.display(),
                ))
                .with_detail("temp_path", temp_path.to_string_lossy()));
            }
        }
    }

    Err(output_error(
        "PDF_TEMP_NAME_EXHAUSTED",
        ErrorCategory::Storage,
        "could not allocate a unique standalone PDF temp file",
        true,
    )
    .with_detail("output_path", output_path.to_string_lossy())
    .with_detail("attempts", TEMP_CREATE_ATTEMPTS.to_string()))
}

fn cleanup_temp(error: A2dError, temp_path: &Path) -> A2dError {
    match std::fs::remove_file(temp_path) {
        Ok(()) => error
            .with_detail("temp_path", temp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "true"),
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => error
            .with_detail("temp_path", temp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "true"),
        Err(cleanup_error) => error
            .with_detail("temp_path", temp_path.to_string_lossy())
            .with_detail("temp_cleanup_completed", "false")
            .with_detail("temp_cleanup_error", cleanup_error.to_string()),
    }
}

fn preserve_unverified_temp(error: A2dError, temp_path: &Path) -> A2dError {
    error
        .with_detail("temp_path", temp_path.to_string_lossy())
        .with_detail("temp_file_preserved", "true")
        .with_detail(
            "recovery_note",
            "unverified standalone output was preserved for explicit diagnostics; do not use it as a valid PDF",
        )
}

fn warning_strings<T: Debug>(warnings: &[T]) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| format!("{warning:?}"))
        .collect()
}

fn reject_warnings(context: &'static str, warnings: Vec<String>) -> Result<(), A2dError> {
    if warnings.is_empty() {
        return Ok(());
    }
    Err(verify_error(format!(
        "{context} produced parser/serializer warnings under the strict standalone output policy",
    ))
    .with_detail("warning_count", warnings.len().to_string())
    .with_detail("warnings", warnings.join(" | ")))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), A2dError> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            io_error(format!(
                "synchronizing standalone PDF destination directory {}: {error}",
                path.display(),
            ))
            .with_detail("destination_directory", path.to_string_lossy())
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), A2dError> {
    Err(output_error(
        "PDF_DIRECTORY_SYNC_UNSUPPORTED",
        ErrorCategory::PlatformAdapter,
        "standalone durable PDF finalization requires directory synchronization on this platform",
        false,
    ))
}

/// Strict standalone output protocol. It never replaces an existing destination. Any corrupt or
/// warning-bearing temp output is intentionally preserved with its path in the returned error.
fn write_and_verify(bytes: &[u8], output_path: &Path) -> Result<(), A2dError> {
    if bytes.is_empty() || bytes.len() > MAX_PDF_OUTPUT_BYTES {
        return Err(validation_error(
            "PDF_OUTPUT_BYTE_LIMIT_INVALID",
            format!(
                "standalone PDF byte length must be within 1..={MAX_PDF_OUTPUT_BYTES}, got {}",
                bytes.len(),
            ),
        )
        .with_detail("byte_length", bytes.len().to_string())
        .with_detail("max_byte_length", MAX_PDF_OUTPUT_BYTES.to_string()));
    }
    validate_output_target(output_path)?;
    let parent = parent_directory(output_path);
    #[cfg(not(unix))]
    sync_directory(parent)?;

    let (mut file, temp_path) = create_unique_temp(output_path)?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        return Err(cleanup_temp(
            io_error(format!(
                "writing standalone PDF temp file {}: {error}",
                temp_path.display(),
            )),
            &temp_path,
        ));
    }
    if let Err(error) = file.flush() {
        drop(file);
        return Err(cleanup_temp(
            io_error(format!(
                "flushing standalone PDF temp file {}: {error}",
                temp_path.display(),
            )),
            &temp_path,
        ));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        return Err(cleanup_temp(
            io_error(format!(
                "synchronizing standalone PDF temp file {}: {error}",
                temp_path.display(),
            )),
            &temp_path,
        ));
    }
    drop(file);

    let reread = std::fs::read(&temp_path).map_err(|error| {
        cleanup_temp(
            io_error(format!(
                "re-reading standalone PDF temp file {}: {error}",
                temp_path.display(),
            )),
            &temp_path,
        )
    })?;
    if reread != bytes {
        return Err(preserve_unverified_temp(
            verify_error("standalone PDF temp bytes changed between write and verification"),
            &temp_path,
        ));
    }

    let mut warnings = Vec::new();
    let document = PdfDocument::parse(
        &reread,
        &PdfParseOptions {
            fail_on_error: true,
        },
        &mut warnings,
    )
    .map_err(|error| {
        preserve_unverified_temp(
            verify_error(format!(
                "generated PDF at {} failed to re-parse: {error}",
                temp_path.display(),
            )),
            &temp_path,
        )
    })?;
    if let Err(error) = reject_warnings("generated PDF parse", warning_strings(&warnings)) {
        return Err(preserve_unverified_temp(error, &temp_path));
    }
    let page_count = document.page_count();
    if page_count == 0 || page_count > MAX_PDF_PAGE_COUNT {
        return Err(preserve_unverified_temp(
            verify_error(format!(
                "verified standalone PDF page count {page_count} is outside 1..={MAX_PDF_PAGE_COUNT}",
            )),
            &temp_path,
        ));
    }

    if let Err(error) = std::fs::hard_link(&temp_path, output_path) {
        let mapped = if error.kind() == std::io::ErrorKind::AlreadyExists {
            output_error(
                "PDF_OUTPUT_ALREADY_EXISTS",
                ErrorCategory::Integrity,
                "standalone PDF destination appeared during finalization and was not replaced",
                false,
            )
        } else {
            io_error(format!(
                "atomically finalizing standalone PDF {} without replacement: {error}",
                output_path.display(),
            ))
        };
        return Err(cleanup_temp(mapped, &temp_path)
            .with_detail("output_path", output_path.to_string_lossy()));
    }
    if let Err(error) = sync_directory(parent) {
        return Err(error
            .with_detail("output_path", output_path.to_string_lossy())
            .with_detail("temp_path", temp_path.to_string_lossy())
            .with_detail("output_file_created", "true")
            .with_detail("destination_directory_sync_completed", "false"));
    }
    if let Err(error) = std::fs::remove_file(&temp_path) {
        return Err(io_error(format!(
            "removing finalized standalone PDF temp file {}: {error}",
            temp_path.display(),
        ))
        .with_detail("output_path", output_path.to_string_lossy())
        .with_detail("temp_path", temp_path.to_string_lossy())
        .with_detail("output_file_created", "true")
        .with_detail("destination_directory_sync_completed", "true")
        .with_detail("temp_cleanup_completed", "false"));
    }
    if let Err(error) = sync_directory(parent) {
        return Err(error
            .with_detail("output_path", output_path.to_string_lossy())
            .with_detail("temp_path", temp_path.to_string_lossy())
            .with_detail("output_file_created", "true")
            .with_detail("temp_cleanup_completed", "true")
            .with_detail("temp_cleanup_directory_sync_completed", "false"));
    }
    Ok(())
}

fn save_document(title: &str, pages: Vec<PdfPage>) -> Result<Vec<u8>, A2dError> {
    if pages.is_empty() || pages.len() > MAX_PDF_PAGE_COUNT {
        return Err(validation_error(
            "PDF_PAGE_COUNT_LIMIT_INVALID",
            format!(
                "generated PDF page count must be within 1..={MAX_PDF_PAGE_COUNT}, got {}",
                pages.len(),
            ),
        )
        .with_detail("page_count", pages.len().to_string())
        .with_detail("max_page_count", MAX_PDF_PAGE_COUNT.to_string()));
    }
    let mut document = PdfDocument::new(title);
    document.with_pages(pages);
    let mut warnings = Vec::new();
    let bytes = document.save(&PdfSaveOptions::default(), &mut warnings);
    reject_warnings("generated PDF serialization", warning_strings(&warnings))?;
    if bytes.is_empty() || bytes.len() > MAX_PDF_OUTPUT_BYTES {
        return Err(validation_error(
            "PDF_OUTPUT_BYTE_LIMIT_INVALID",
            format!(
                "generated PDF byte length must be within 1..={MAX_PDF_OUTPUT_BYTES}, got {}",
                bytes.len(),
            ),
        )
        .with_detail("byte_length", bytes.len().to_string())
        .with_detail("max_byte_length", MAX_PDF_OUTPUT_BYTES.to_string()));
    }
    Ok(bytes)
}

#[derive(Debug)]
pub struct GeneratedSmartPageBytes {
    pub smart_page_id: SmartPageId,
    pub layout_id: LayoutId,
    pub pdf_bytes: Vec<u8>,
}

pub fn render_smart_page_pdf_bytes(
    paper: PaperSize,
    style: SmartPageStyle,
) -> Result<GeneratedSmartPageBytes, A2dError> {
    let layout = smart_page_layout(paper, style);
    let smart_page_id = SmartPageId::generate();
    let qr_payload = PageCode::SmartPage {
        smart_page_id: smart_page_id.clone(),
        layout_id: layout.id.clone(),
        visible_page_number: None,
        page_set_id: None,
    }
    .encode()?;
    let ops = render_page_ops(&layout, &qr_payload, None)?;
    Ok(GeneratedSmartPageBytes {
        smart_page_id,
        layout_id: layout.id.clone(),
        pdf_bytes: save_document("A2D Smart Page", vec![pdf_page_for(&layout, ops)])?,
    })
}

pub fn generate_smart_page_pdf(
    paper: PaperSize,
    style: SmartPageStyle,
    output_path: &Path,
) -> Result<SmartPageId, A2dError> {
    let generated = render_smart_page_pdf_bytes(paper, style)?;
    write_and_verify(&generated.pdf_bytes, output_path)?;
    Ok(generated.smart_page_id)
}

#[derive(Debug)]
pub struct GeneratePageSetRequest {
    pub paper_size: PaperSize,
    pub style: SmartPageStyle,
    pub page_count: u32,
    pub starting_visible_page: u32,
}

impl GeneratePageSetRequest {
    fn validated_capacity_and_last_page(&self) -> Result<(usize, u32), A2dError> {
        if self.page_count == 0 {
            return Err(validation_error(
                "PDF_PAGE_SET_EMPTY",
                "page_count must be at least 1",
            ));
        }
        if self.page_count > MAX_PAGE_SET_PAGE_COUNT {
            return Err(validation_error(
                "PDF_PAGE_SET_PAGE_COUNT_LIMIT_EXCEEDED",
                format!(
                    "page_count {} exceeds the portable limit {MAX_PAGE_SET_PAGE_COUNT}",
                    self.page_count
                ),
            )
            .with_detail("page_count", self.page_count.to_string())
            .with_detail("max_page_count", MAX_PAGE_SET_PAGE_COUNT.to_string()));
        }
        if self.starting_visible_page == 0 {
            return Err(validation_error(
                "PDF_PAGE_SET_STARTING_PAGE_INVALID",
                "starting_visible_page must be at least 1",
            ));
        }

        let last_visible_page = self
            .starting_visible_page
            .checked_add(self.page_count - 1)
            .ok_or_else(|| {
                validation_error(
                    "PDF_PAGE_SET_VISIBLE_PAGE_OVERFLOW",
                    "visible page range overflows the portable u32 representation",
                )
            })?;
        if last_visible_page > MAX_QR_V1_VISIBLE_PAGE_NUMBER {
            return Err(validation_error(
                "PDF_PAGE_SET_VISIBLE_PAGE_OUT_OF_RANGE",
                format!(
                    "last visible page {last_visible_page} exceeds QR v1 maximum {MAX_QR_V1_VISIBLE_PAGE_NUMBER}"
                ),
            )
            .with_detail("starting_visible_page", self.starting_visible_page.to_string())
            .with_detail("page_count", self.page_count.to_string())
            .with_detail("last_visible_page", last_visible_page.to_string())
            .with_detail(
                "max_visible_page",
                MAX_QR_V1_VISIBLE_PAGE_NUMBER.to_string(),
            ));
        }

        let capacity = usize::try_from(self.page_count).map_err(|_| {
            validation_error(
                "PDF_PAGE_SET_PAGE_COUNT_UNSUPPORTED",
                "page_count does not fit this platform's address space",
            )
        })?;
        Ok((capacity, last_visible_page))
    }
}

#[derive(Debug)]
pub struct GeneratedPageSet {
    pub page_set_id: PageSetId,
    pub layout_id: LayoutId,
    pub smart_page_ids: Vec<SmartPageId>,
}

#[derive(Debug)]
pub struct GeneratedPageSetBytes {
    pub page_set_id: PageSetId,
    pub layout_id: LayoutId,
    pub smart_page_ids: Vec<SmartPageId>,
    pub pdf_bytes: Vec<u8>,
}

pub fn render_page_set_pdf_bytes(
    request: GeneratePageSetRequest,
) -> Result<GeneratedPageSetBytes, A2dError> {
    let (capacity, validated_last_page) = request.validated_capacity_and_last_page()?;
    let layout = smart_page_layout(request.paper_size, request.style);
    let page_set_id = PageSetId::generate();
    let mut smart_page_ids = Vec::with_capacity(capacity);
    let mut pdf_pages = Vec::with_capacity(capacity);

    for offset in 0..request.page_count {
        let smart_page_id = SmartPageId::generate();
        let visible_number = request
            .starting_visible_page
            .checked_add(offset)
            .ok_or_else(|| {
                validation_error(
                    "PDF_PAGE_SET_VISIBLE_PAGE_OVERFLOW",
                    "visible page arithmetic changed after request validation",
                )
            })?;
        debug_assert!(visible_number <= validated_last_page);
        let qr_payload = PageCode::SmartPage {
            smart_page_id: smart_page_id.clone(),
            layout_id: layout.id.clone(),
            visible_page_number: Some(visible_number),
            page_set_id: Some(page_set_id.clone()),
        }
        .encode()?;
        let ops = render_page_ops(&layout, &qr_payload, Some(visible_number))?;
        pdf_pages.push(pdf_page_for(&layout, ops));
        smart_page_ids.push(smart_page_id);
    }

    Ok(GeneratedPageSetBytes {
        page_set_id,
        layout_id: layout.id,
        smart_page_ids,
        pdf_bytes: save_document("A2D Smart Page Set", pdf_pages)?,
    })
}

pub fn generate_page_set_pdf(
    request: GeneratePageSetRequest,
    output_path: &Path,
) -> Result<GeneratedPageSet, A2dError> {
    let generated = render_page_set_pdf_bytes(request)?;
    write_and_verify(&generated.pdf_bytes, output_path)?;
    Ok(GeneratedPageSet {
        page_set_id: generated.page_set_id,
        layout_id: generated.layout_id,
        smart_page_ids: generated.smart_page_ids,
    })
}

pub fn render_notebook_proof_interior_pdf_bytes(
    design_id: &NotebookDesignId,
    logical_page_count: u32,
) -> Result<Vec<u8>, A2dError> {
    if logical_page_count == 0 || logical_page_count > MAX_NOTEBOOK_PROOF_LOGICAL_PAGE_COUNT {
        return Err(validation_error(
            "PDF_NOTEBOOK_INTERIOR_PAGE_COUNT_INVALID",
            format!(
                "logical_page_count must be within 1..={MAX_NOTEBOOK_PROOF_LOGICAL_PAGE_COUNT}",
            ),
        )
        .with_detail("logical_page_count", logical_page_count.to_string())
        .with_detail(
            "max_logical_page_count",
            MAX_NOTEBOOK_PROOF_LOGICAL_PAGE_COUNT.to_string(),
        ));
    }

    let setup_layout = setup_page_layout();
    let writable_layout = writable_page_layout();
    let page_capacity = usize::try_from(logical_page_count)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| {
            validation_error(
                "PDF_NOTEBOOK_INTERIOR_CAPACITY_OVERFLOW",
                "notebook proof page capacity does not fit this platform",
            )
        })?;
    let mut pdf_pages = Vec::with_capacity(page_capacity);

    let setup_payload = PageCode::NotebookSetup {
        design_id: design_id.clone(),
    }
    .encode()?;
    let setup_ops = render_page_ops(&setup_layout, &setup_payload, None)?;
    pdf_pages.push(pdf_page_for(&setup_layout, setup_ops));
    pdf_pages.push(blank_verso_page(&setup_layout));

    for logical_page_number in 1..=logical_page_count {
        debug_assert_eq!(
            pdf_pages.len() as u32 + 1,
            pdf_page_number_for_logical_page(logical_page_number)
        );
        let payload = PageCode::NotebookPage {
            design_id: design_id.clone(),
            logical_page_number,
            layout_id: writable_layout.id.clone(),
        }
        .encode()?;
        let ops = render_page_ops(&writable_layout, &payload, Some(logical_page_number))?;
        pdf_pages.push(pdf_page_for(&writable_layout, ops));
        pdf_pages.push(blank_verso_page(&writable_layout));
    }

    save_document("A2D Smart Notebook Proof Interior", pdf_pages)
}

pub fn generate_notebook_proof_interior_pdf(
    design_id: &NotebookDesignId,
    logical_page_count: u32,
    output_path: &Path,
) -> Result<(), A2dError> {
    let bytes = render_notebook_proof_interior_pdf_bytes(design_id, logical_page_count)?;
    write_and_verify(&bytes, output_path)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "a2d-pdf-generate-{label}-{}.pdf",
            SmartPageId::generate()
        ))
    }

    fn remove_if_present(path: &Path) {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("test cleanup failed for {}: {error}", path.display()),
        }
    }

    #[test]
    fn corrupt_output_is_rejected_and_preserved_with_a_unique_recovery_path() {
        let generated = render_smart_page_pdf_bytes(PaperSize::A4, SmartPageStyle::Blank).unwrap();
        let truncated = &generated.pdf_bytes[..generated.pdf_bytes.len() / 2];
        let output = temp_path("truncated");
        let error = write_and_verify(truncated, &output).unwrap_err();
        assert_eq!(error.code.to_string(), "PDF_VERIFY_FAILED");
        assert!(!output.exists());
        assert_eq!(
            error.details.get("temp_file_preserved").map(String::as_str),
            Some("true"),
        );
        let temp = PathBuf::from(error.details.get("temp_path").unwrap());
        assert!(temp.exists());
        remove_if_present(&temp);
    }

    #[test]
    fn existing_destination_is_not_replaced() {
        let output = temp_path("no-replace");
        std::fs::write(&output, b"existing").unwrap();
        let error =
            generate_smart_page_pdf(PaperSize::A4, SmartPageStyle::Blank, &output).unwrap_err();
        assert_eq!(error.code.to_string(), "PDF_OUTPUT_ALREADY_EXISTS");
        assert_eq!(std::fs::read(&output).unwrap(), b"existing");
        remove_if_present(&output);
    }

    #[test]
    fn concurrent_failed_generators_use_independent_temp_files() {
        let generated = render_smart_page_pdf_bytes(PaperSize::A4, SmartPageStyle::Blank).unwrap();
        let truncated = Arc::new(generated.pdf_bytes[..generated.pdf_bytes.len() / 2].to_vec());
        let output = temp_path("concurrent-temp");
        let handles = (0..2)
            .map(|_| {
                let bytes = Arc::clone(&truncated);
                let output = output.clone();
                std::thread::spawn(move || write_and_verify(&bytes, &output).unwrap_err())
            })
            .collect::<Vec<_>>();
        let errors = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        let temp_paths = errors
            .iter()
            .map(|error| PathBuf::from(error.details.get("temp_path").unwrap()))
            .collect::<Vec<_>>();
        assert_ne!(temp_paths[0], temp_paths[1]);
        for path in temp_paths {
            assert!(path.exists());
            remove_if_present(&path);
        }
        assert!(!output.exists());
    }

    #[test]
    fn warning_policy_rejects_any_warning() {
        let error = reject_warnings(
            "synthetic warning test",
            vec!["malformed object warning".to_string()],
        )
        .unwrap_err();
        assert_eq!(error.code.to_string(), "PDF_VERIFY_FAILED");
        assert_eq!(
            error.details.get("warning_count").map(String::as_str),
            Some("1"),
        );
    }

    #[test]
    fn generated_smart_page_re_parses_and_leaves_no_temp_file() {
        let output = temp_path("single");
        generate_smart_page_pdf(PaperSize::UsLetter, SmartPageStyle::Blank, &output).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        let mut warnings = Vec::new();
        let document =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(document.page_count(), 1);
        let parent = parent_directory(&output);
        let file_name = output.file_name().unwrap().to_string_lossy();
        let leftovers = std::fs::read_dir(parent)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!(".{file_name}.a2d-"))
            })
            .count();
        assert_eq!(leftovers, 0);
        remove_if_present(&output);
    }

    #[test]
    fn generated_smart_pages_always_have_fresh_ids() {
        let path_a = temp_path("id-a");
        let path_b = temp_path("id-b");
        let first = generate_smart_page_pdf(PaperSize::A4, SmartPageStyle::Blank, &path_a).unwrap();
        let second =
            generate_smart_page_pdf(PaperSize::A4, SmartPageStyle::Blank, &path_b).unwrap();
        assert_ne!(first, second);
        remove_if_present(&path_a);
        remove_if_present(&path_b);
    }

    #[test]
    fn page_set_limits_fail_before_generation() {
        let empty = render_page_set_pdf_bytes(GeneratePageSetRequest {
            paper_size: PaperSize::A4,
            style: SmartPageStyle::Blank,
            page_count: 0,
            starting_visible_page: 1,
        })
        .unwrap_err();
        assert_eq!(empty.code.to_string(), "PDF_PAGE_SET_EMPTY");

        let too_many = render_page_set_pdf_bytes(GeneratePageSetRequest {
            paper_size: PaperSize::A4,
            style: SmartPageStyle::Blank,
            page_count: MAX_PAGE_SET_PAGE_COUNT + 1,
            starting_visible_page: 1,
        })
        .unwrap_err();
        assert_eq!(
            too_many.code.to_string(),
            "PDF_PAGE_SET_PAGE_COUNT_LIMIT_EXCEEDED"
        );

        let zero_start = render_page_set_pdf_bytes(GeneratePageSetRequest {
            paper_size: PaperSize::A4,
            style: SmartPageStyle::Blank,
            page_count: 1,
            starting_visible_page: 0,
        })
        .unwrap_err();
        assert_eq!(
            zero_start.code.to_string(),
            "PDF_PAGE_SET_STARTING_PAGE_INVALID"
        );

        let out_of_range = render_page_set_pdf_bytes(GeneratePageSetRequest {
            paper_size: PaperSize::A4,
            style: SmartPageStyle::Blank,
            page_count: 2,
            starting_visible_page: MAX_QR_V1_VISIBLE_PAGE_NUMBER,
        })
        .unwrap_err();
        assert_eq!(
            out_of_range.code.to_string(),
            "PDF_PAGE_SET_VISIBLE_PAGE_OUT_OF_RANGE"
        );
    }

    #[test]
    fn page_set_accepts_the_qr_v1_visible_page_boundary() {
        let generated = render_page_set_pdf_bytes(GeneratePageSetRequest {
            paper_size: PaperSize::A4,
            style: SmartPageStyle::Blank,
            page_count: 2,
            starting_visible_page: MAX_QR_V1_VISIBLE_PAGE_NUMBER - 1,
        })
        .unwrap();
        assert_eq!(generated.smart_page_ids.len(), 2);
    }

    #[test]
    fn generated_page_set_has_one_unique_identity_per_pdf_page() {
        let path = temp_path("set");
        let generated = generate_page_set_pdf(
            GeneratePageSetRequest {
                paper_size: PaperSize::UsLetter,
                style: SmartPageStyle::Graph,
                page_count: 5,
                starting_visible_page: 10,
            },
            &path,
        )
        .unwrap();
        assert_eq!(generated.smart_page_ids.len(), 5);
        let unique: std::collections::HashSet<_> = generated.smart_page_ids.iter().collect();
        assert_eq!(unique.len(), 5);
        let bytes = std::fs::read(&path).unwrap();
        let mut warnings = Vec::new();
        let document =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(document.page_count(), 5);
        remove_if_present(&path);
    }

    #[test]
    fn notebook_proof_page_limit_is_enforced_before_generation() {
        let error = render_notebook_proof_interior_pdf_bytes(
            &NotebookDesignId::generate(),
            MAX_NOTEBOOK_PROOF_LOGICAL_PAGE_COUNT + 1,
        )
        .unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "PDF_NOTEBOOK_INTERIOR_PAGE_COUNT_INVALID"
        );
    }

    #[test]
    fn notebook_proof_rejects_zero_and_alternates_recto_verso() {
        let empty_error =
            render_notebook_proof_interior_pdf_bytes(&NotebookDesignId::generate(), 0).unwrap_err();
        assert_eq!(
            empty_error.code.to_string(),
            "PDF_NOTEBOOK_INTERIOR_PAGE_COUNT_INVALID"
        );

        let path = temp_path("notebook");
        let logical_page_count = 3;
        generate_notebook_proof_interior_pdf(
            &NotebookDesignId::generate(),
            logical_page_count,
            &path,
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mut warnings = Vec::new();
        let document =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(document.page_count(), 2 + 2 * logical_page_count as usize);
        for (index, page) in document.pages.iter().enumerate() {
            assert_eq!(page.ops.is_empty(), index % 2 == 1);
        }
        remove_if_present(&path);
    }
}
