//! Shared image capture analysis and processing.
//!
//! Milestone 7 establishes validated borrowed grayscale and bounded encoded-image
//! input boundaries, projective page rectification, explicit versioned quality
//! measurement/classification, bounded atomic derived-image processing, and a
//! small reviewed unsafe wrapper around the official AprilTag 3 C detector.
//! Native pointers, ownership, and allocation never cross the public Rust API.

mod derived;
mod detection;
mod detector;
mod encoded;
mod error;
mod input;
mod quality;
mod rectification;

pub use derived::{
    ContrastNormalizationConfig, ContrastNormalizationProvenance, DerivedImageConfig,
    DerivedImageLimits, DerivedImagePipeline, DerivedImageProvenance, DerivedImages,
    ProcessingCancellation, SharpenConfig, ThumbnailConfig,
};
pub use detection::{
    ImagePoint, MarkerDetection, MarkerFamily, MarkerIdLayout, PageOrientation, ResolvedMarker,
    ResolvedPageMarkers, resolve_page_markers,
};
pub use detector::{AprilTagDetector, DetectorConfig, RenderedTag};
pub use encoded::{
    EncodedImage, EncodedImageFormat, EncodedImageLimits, OwnedGrayImage, OwnedRgbImage,
};
pub use input::{GrayFrame, ImageLimits, ImageRotation, PixelFormat};
pub use quality::{
    BandThresholds, CurvatureMetrics, CurvaturePolicy, EdgeProbe, ExposureMetrics, FocusMetrics,
    FramingMetrics, FramingPolicy, GlareMetrics, GlarePolicy, GrayQualityMetrics,
    LuminanceMeasurementConfig, MarkerConfidenceMetrics, MarkerConfidencePolicy, MetricState,
    OverexposurePolicy, PageEdge, PerspectiveMetrics, PerspectivePolicy, QualityAssessment,
    QualityMeasurements, QualityMetricStates, QualityPolicy, QualityRequirements, QualityState,
    QualityThresholdSet, ResolutionMetrics, ResolutionPolicy, ScalarThresholds, ThresholdDirection,
    UnderexposurePolicy, measure_curvature, measure_effective_resolution, measure_framing,
    measure_gray_quality, measure_marker_confidence, measure_perspective,
};
pub use rectification::{
    ImageQuad, ProjectiveTransform, RectificationLimits, RectificationPlan, RectifiedImageSize,
};
