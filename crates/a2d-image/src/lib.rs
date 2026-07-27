//! Shared image capture analysis and processing.
//!
//! Milestone 7 establishes validated borrowed grayscale and bounded encoded-image
//! input boundaries plus a small, reviewed unsafe wrapper around the official
//! AprilTag 3 C detector. Native pointers, ownership, and allocation never cross
//! the public Rust API.

mod detection;
mod detector;
mod encoded;
mod error;
mod input;

pub use detection::{
    ImagePoint, MarkerDetection, MarkerFamily, MarkerIdLayout, PageOrientation,
    ResolvedMarker, ResolvedPageMarkers, resolve_page_markers,
};
pub use detector::{AprilTagDetector, DetectorConfig};
pub use encoded::{
    EncodedImage, EncodedImageFormat, EncodedImageLimits, OwnedGrayImage, OwnedRgbImage,
};
pub use input::{GrayFrame, ImageLimits, ImageRotation, PixelFormat};
