//! Assembles rendered pages into complete PDF documents and commits them to disk (TODO 5.4/5.5):
//! single Smart Pages, multi-page Page Sets, and the bound-notebook proof interior.
//!
//! Every generator writes to a temporary path first, re-parses the bytes it just wrote to
//! confirm they're a well-formed PDF, and only then renames into place at `output_path` (TODO
//! 5.4 "write to a temp path and verify before returning success" — the same
//! write-then-verify-then-commit discipline spec §16.3 requires for asset commits, applied here
//! to generated PDFs).

use std::io::Write;
use std::path::Path;

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

/// Portable resource-safety ceiling for one generated Page Set. Android uses the same value for
/// immediate form feedback, but Rust enforces it for every caller, including direct FFI and future
/// Swift clients.
pub const MAX_PAGE_SET_PAGE_COUNT: u32 = 500;

/// QR v1 numeric fields are bounded to 0..=999999. Kept explicit here so visible-page arithmetic
/// is checked before allocation or identity generation rather than failing partway through a set.
const MAX_QR_V1_VISIBLE_PAGE_NUMBER: u32 = 999_999;

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

/// Writes `bytes` to a temp path beside `output_path`, re-parses them to confirm the file is a
/// well-formed PDF, then atomically renames into place. Never leaves `output_path` pointing at
/// content that wasn't verified.
fn write_and_verify(bytes: &[u8], output_path: &Path) -> Result<(), A2dError> {
    let tmp_path = output_path.with_extension("pdf.tmp");
    {
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| io_error(format!("creating temp file {}: {e}", tmp_path.display())))?;
        file.write_all(bytes)
            .map_err(|e| io_error(format!("writing temp file {}: {e}", tmp_path.display())))?;
        file.flush()
            .map_err(|e| io_error(format!("flushing temp file {}: {e}", tmp_path.display())))?;
    }

    let reread = std::fs::read(&tmp_path)
        .map_err(|e| io_error(format!("re-reading temp file {}: {e}", tmp_path.display())))?;
    let mut warnings = Vec::new();
    let parse_opts = PdfParseOptions {
        fail_on_error: true,
    };
    PdfDocument::parse(&reread, &parse_opts, &mut warnings).map_err(|e| {
        verify_error(format!(
            "generated PDF at {} failed to re-parse: {e}",
            tmp_path.display()
        ))
    })?;

    std::fs::rename(&tmp_path, output_path).map_err(|e| {
        io_error(format!(
            "renaming {} to {}: {e}",
            tmp_path.display(),
            output_path.display()
        ))
    })?;
    Ok(())
}

fn save_document(title: &str, pages: Vec<PdfPage>) -> Vec<u8> {
    let mut doc = PdfDocument::new(title);
    doc.with_pages(pages);
    let mut warnings = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings)
}

/// A generated Smart Page's PDF bytes plus the identities generating it minted. Kept separate
/// from disk I/O so a caller that owns its own asset-commit protocol (TODO 5.5, `a2d-core`'s
/// `AssetStore`) can commit these bytes itself rather than going through [`write_and_verify`]'s
/// arbitrary-output-path flow, which is for standalone/CLI-style generation instead.
pub struct GeneratedSmartPageBytes {
    pub smart_page_id: SmartPageId,
    pub layout_id: LayoutId,
    pub pdf_bytes: Vec<u8>,
}

/// Renders a single unique A2D Smart Page PDF (spec §7.5) to bytes, without writing anything to
/// disk. Returns the freshly generated, globally unique `SmartPageId` -- generated entirely
/// offline, no server allocation.
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
    let pdf_bytes = save_document("A2D Smart Page", vec![pdf_page_for(&layout, ops)]);
    Ok(GeneratedSmartPageBytes {
        smart_page_id,
        layout_id: layout.id,
        pdf_bytes,
    })
}

/// Generates a single unique A2D Smart Page PDF (spec §7.5), writing it to `output_path`.
/// Returns the freshly generated, globally unique `SmartPageId`.
pub fn generate_smart_page_pdf(
    paper: PaperSize,
    style: SmartPageStyle,
    output_path: &Path,
) -> Result<SmartPageId, A2dError> {
    let generated = render_smart_page_pdf_bytes(paper, style)?;
    write_and_verify(&generated.pdf_bytes, output_path)?;
    Ok(generated.smart_page_id)
}

/// Mirrors TODO 5.4's suggested request shape.
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
        let last_offset = self.page_count - 1;
        let last_visible_page = self
            .starting_visible_page
            .checked_add(last_offset)
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

pub struct GeneratedPageSetBytes {
    pub page_set_id: PageSetId,
    pub layout_id: LayoutId,
    pub smart_page_ids: Vec<SmartPageId>,
    pub pdf_bytes: Vec<u8>,
}

/// Renders a multipage Page Set PDF (spec §7.6) to bytes, without writing anything to disk: one
/// `PageSetId`, one unique `SmartPageId` per page, ascending visible page numbers starting at
/// `starting_visible_page`.
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

    let pdf_bytes = save_document("A2D Smart Page Set", pdf_pages);
    Ok(GeneratedPageSetBytes {
        page_set_id,
        layout_id: layout.id,
        smart_page_ids,
        pdf_bytes,
    })
}

/// Generates a multipage Page Set PDF (spec §7.6), writing it to `output_path`.
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

/// Renders the bound-notebook proof interior PDF (TODO 5.3/5.4) to bytes, without writing
/// anything to disk: the Setup Page, then `logical_page_count` writable pages, each followed by
/// a blank verso (spec §11.2's alternating recto/verso interior). Uses `a2d_layout::notebook`'s
/// fixed development-placeholder layouts (Milestone 5.3) -- there is no real official Notebook
/// Design to generate from yet.
pub fn render_notebook_proof_interior_pdf_bytes(
    design_id: &NotebookDesignId,
    logical_page_count: u32,
) -> Result<Vec<u8>, A2dError> {
    if logical_page_count == 0 {
        return Err(validation_error(
            "PDF_NOTEBOOK_INTERIOR_EMPTY",
            "logical_page_count must be at least 1",
        ));
    }

    let setup_layout = setup_page_layout();
    let writable_layout = writable_page_layout();

    let mut pdf_pages = Vec::new();

    let setup_payload = PageCode::NotebookSetup {
        design_id: design_id.clone(),
    }
    .encode()?;
    let setup_ops = render_page_ops(&setup_layout, &setup_payload, None)?;
    pdf_pages.push(pdf_page_for(&setup_layout, setup_ops));
    pdf_pages.push(blank_verso_page(&setup_layout));

    for logical_page_number in 1..=logical_page_count {
        // Ties this construction loop to Milestone 5.3's independently defined page-number
        // mapping: if the two ever drift apart, this catches it immediately rather than only
        // showing up as a subtle numbering bug once Milestone 6 wires up real scanning.
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

    Ok(save_document(
        "A2D Smart Notebook Proof Interior",
        pdf_pages,
    ))
}

/// Generates the bound-notebook proof interior PDF (TODO 5.3/5.4), writing it to `output_path`.
pub fn generate_notebook_proof_interior_pdf(
    design_id: &NotebookDesignId,
    logical_page_count: u32,
    output_path: &Path,
) -> Result<(), A2dError> {
    let bytes = render_notebook_proof_interior_pdf_bytes(design_id, logical_page_count)?;
    write_and_verify(&bytes, output_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "a2d-pdf-generate-{label}-{}.pdf",
            SmartPageId::generate()
        ))
    }

    #[test]
    fn write_and_verify_rejects_truncated_bytes_and_never_creates_the_output_path() {
        let generated = render_smart_page_pdf_bytes(PaperSize::A4, SmartPageStyle::Blank).unwrap();
        let truncated = &generated.pdf_bytes[..generated.pdf_bytes.len() / 2];

        let path = temp_path("truncated");
        let err = write_and_verify(truncated, &path).unwrap_err();
        assert!(err.code.to_string().contains("VERIFY_FAILED"));
        assert!(
            !path.exists(),
            "a failed verify must never leave a file at output_path"
        );

        let tmp_path = path.with_extension("pdf.tmp");
        assert!(tmp_path.exists());
        std::fs::remove_file(&tmp_path).ok();
    }

    #[test]
    fn generate_smart_page_pdf_writes_a_single_page_pdf_that_re_parses() {
        let path = temp_path("single");
        generate_smart_page_pdf(PaperSize::UsLetter, SmartPageStyle::Blank, &path).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty());
        let mut warnings = Vec::new();
        let doc = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings).unwrap();
        assert_eq!(doc.page_count(), 1);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn generate_smart_page_pdf_never_leaves_a_temp_file_behind_on_success() {
        let path = temp_path("no-leftover-tmp");
        generate_smart_page_pdf(PaperSize::A4, SmartPageStyle::Lined, &path).unwrap();
        assert!(!path.with_extension("pdf.tmp").exists());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn generate_smart_page_pdf_produces_a_fresh_id_every_call() {
        let a = generate_smart_page_pdf(PaperSize::A4, SmartPageStyle::Blank, &temp_path("id-a"))
            .unwrap();
        let b = generate_smart_page_pdf(PaperSize::A4, SmartPageStyle::Blank, &temp_path("id-b"))
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn generate_page_set_pdf_rejects_zero_pages() {
        let request = GeneratePageSetRequest {
            paper_size: PaperSize::A4,
            style: SmartPageStyle::Blank,
            page_count: 0,
            starting_visible_page: 1,
        };
        let err = generate_page_set_pdf(request, &temp_path("empty")).unwrap_err();
        assert!(err.code.to_string().contains("PAGE_SET_EMPTY"));
    }

    #[test]
    fn page_set_limits_are_enforced_before_allocation_or_identity_generation() {
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
    fn generate_page_set_pdf_produces_one_pdf_page_and_one_unique_smart_page_id_per_page() {
        let path = temp_path("set");
        let request = GeneratePageSetRequest {
            paper_size: PaperSize::UsLetter,
            style: SmartPageStyle::Graph,
            page_count: 5,
            starting_visible_page: 10,
        };
        let generated = generate_page_set_pdf(request, &path).unwrap();

        assert_eq!(generated.smart_page_ids.len(), 5);
        let unique: std::collections::HashSet<_> = generated.smart_page_ids.iter().collect();
        assert_eq!(
            unique.len(),
            5,
            "every page must get a distinct SmartPageId"
        );

        let bytes = std::fs::read(&path).unwrap();
        let mut warnings = Vec::new();
        let doc = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings).unwrap();
        assert_eq!(doc.page_count(), 5);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn generate_notebook_proof_interior_pdf_rejects_zero_logical_pages() {
        let err = generate_notebook_proof_interior_pdf(
            &NotebookDesignId::generate(),
            0,
            &temp_path("notebook-empty"),
        )
        .unwrap_err();
        assert!(err.code.to_string().contains("NOTEBOOK_INTERIOR_EMPTY"));
    }

    #[test]
    fn generate_notebook_proof_interior_pdf_alternates_recto_and_blank_verso() {
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
        let doc = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings).unwrap();

        assert_eq!(doc.page_count(), 2 + 2 * logical_page_count as usize);
        for (index, page) in doc.pages.iter().enumerate() {
            let is_verso = index % 2 == 1;
            if is_verso {
                assert!(
                    page.ops.is_empty(),
                    "page {index} should be a blank verso but has {} ops",
                    page.ops.len()
                );
            } else {
                assert!(
                    !page.ops.is_empty(),
                    "page {index} should be a non-blank recto page"
                );
            }
        }

        std::fs::remove_file(&path).ok();
    }
}
