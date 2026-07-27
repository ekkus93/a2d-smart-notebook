//! Page-layout definitions and geometry: corner marker placement, content rectangles, margins, and calibration.

pub mod geometry;
pub mod manifest;
pub mod page_layout;

pub use manifest::{ManifestRegistry, bundled_placeholder_registry, parse_manifest};
pub use page_layout::{CalibrationMark, MarkerPlacement, MarkerRole, PageLayout};
