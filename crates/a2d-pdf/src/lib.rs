//! Generates Smart Page and Notebook Design PDFs, including Corner Markers, Page Codes, and print-safe layout.

pub mod coordinates;
pub mod error;
pub mod generate;
pub mod render;

pub use generate::{
    GeneratePageSetRequest, GeneratedPageSet, GeneratedPageSetBytes, GeneratedSmartPageBytes,
    generate_notebook_proof_interior_pdf, generate_page_set_pdf, generate_smart_page_pdf,
    render_notebook_proof_interior_pdf_bytes, render_page_set_pdf_bytes,
    render_smart_page_pdf_bytes,
};

pub use render::render_page_ops;
