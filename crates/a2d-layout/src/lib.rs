//! Page-layout definitions and geometry: corner marker placement, content rectangles, margins, and calibration.

pub mod geometry;
pub mod layout_builder;
pub mod manifest;
pub mod notebook;
pub mod page_layout;
pub mod scan_layout;
pub mod smart_page;

pub use manifest::{ManifestRegistry, bundled_placeholder_registry, parse_manifest};
pub use notebook::{
    NOTEBOOK_TRIM_SIZE_MM, pdf_page_number_for_logical_page, setup_page_layout,
    writable_page_layout,
};
pub use page_layout::{CalibrationMark, ContentStyle, MarkerPlacement, MarkerRole, PageLayout};
pub use scan_layout::{
    ResolvedMarkerRole, ResolvedScanLayout, SCAN_PROCESSING_POLICY_VERSION,
    V1_CORRECTED_WIDTH_PX, V1_MARKER_FAMILY, V1_MARKER_ID_LAYOUT, marker_id_for_role,
    resolve_bundled_scan_layout, resolve_scan_layout_for_page,
};
pub use smart_page::{PaperSize, SmartPageStyle, smart_page_layout};
