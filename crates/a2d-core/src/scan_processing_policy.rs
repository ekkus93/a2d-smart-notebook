//! Versioned Rust-owned portable processing policy shared by preview and durable registration.

use std::ops::Deref;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId};
use a2d_image::{
    ContrastNormalizationConfig, DerivedImageConfig, DerivedImageLimits, DetectorConfig,
    EncodedImageLimits, ImageLimits, LuminanceMeasurementConfig, MarkerFamily, MarkerIdLayout,
    RectificationLimits, RectifiedImageSize, ThumbnailConfig,
};
use a2d_layout::{ResolvedScanLayout, resolve_bundled_scan_layout};

/// Version of the complete portable scan-processing policy, not Android presentation guidance.
pub const SCAN_PROCESSING_POLICY_VERSION: u32 = 1;

const MAXIMUM_ENCODED_BYTES: usize = 24 * 1024 * 1024;
const MAXIMUM_DECODED_PIXELS: u64 = 32_000_000;
const MAXIMUM_DECODED_BYTES: u64 = 96_000_000;
const DETECTOR_THREAD_COUNT: u8 = 1;
const DETECTOR_QUAD_DECIMATE: f32 = 2.0;
const DETECTOR_QUAD_SIGMA: f32 = 0.0;
const DETECTOR_REFINE_EDGES: bool = true;
const DETECTOR_DECODE_SHARPENING: f64 = 0.25;
const DETECTOR_BITS_CORRECTED: u8 = 2;
const DARK_LUMINANCE_CUTOFF: u8 = 32;
const HIGHLIGHT_LUMINANCE_CUTOFF: u8 = 245;
const QUALITY_TILE_COLUMNS: u16 = 8;
const QUALITY_TILE_ROWS: u16 = 8;
const RECTIFICATION_MAXIMUM_OUTPUT_PIXELS: u64 = 2_000_000;
const RECTIFICATION_MAXIMUM_OUTPUT_BYTES: u64 = 6_000_000;
const DERIVED_PIPELINE_VERSION: u32 = 1;
const CONTRAST_LOW_PERCENTILE_PER_MILLION: u32 = 10_000;
const CONTRAST_HIGH_PERCENTILE_PER_MILLION: u32 = 990_000;
const CONTRAST_MAXIMUM_GAIN: f64 = 2.0;
const THUMBNAIL_MAXIMUM_WIDTH: u32 = 480;
const THUMBNAIL_MAXIMUM_HEIGHT: u32 = 480;
const DERIVED_MAXIMUM_PIXELS_PER_IMAGE: u64 = 2_000_000;
const DERIVED_MAXIMUM_BYTES_PER_IMAGE: u64 = 6_000_000;
const DERIVED_MAXIMUM_TOTAL_OUTPUT_BYTES: u64 = 12_000_000;
const DERIVED_MAXIMUM_WORKING_BYTES: u64 = 96_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiveAnalysisPolicyValues {
    pub maximum_encoded_bytes: u64,
    pub maximum_decoded_pixels: u64,
    pub maximum_decoded_bytes: u64,
    pub detector_thread_count: u32,
    pub detector_quad_decimate: f64,
    pub detector_quad_sigma: f64,
    pub detector_refine_edges: bool,
    pub detector_decode_sharpening: f64,
    pub detector_bits_corrected: u32,
    pub dark_luminance_cutoff: u32,
    pub highlight_luminance_cutoff: u32,
    pub quality_tile_columns: u32,
    pub quality_tile_rows: u32,
}

#[derive(Clone, Debug)]
pub struct StoredScanProcessingPolicy {
    pub layout: ResolvedScanLayout,
    pub policy_version: u32,
}

impl Deref for StoredScanProcessingPolicy {
    type Target = ResolvedScanLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl StoredScanProcessingPolicy {
    pub fn from_resolved_layout(layout: ResolvedScanLayout) -> Result<Self, A2dError> {
        if layout.processing_policy_version != SCAN_PROCESSING_POLICY_VERSION {
            return Err(policy_error(
                "CORE_SCAN_PROCESSING_POLICY_VERSION_UNSUPPORTED",
                ErrorCategory::UnsupportedFormat,
                "the resolved layout requires an unsupported scan-processing policy version",
            )
            .with_detail("layout_id", layout.layout_id.to_string())
            .with_detail(
                "resolved_policy_version",
                layout.processing_policy_version.to_string(),
            )
            .with_detail(
                "supported_policy_version",
                SCAN_PROCESSING_POLICY_VERSION.to_string(),
            ));
        }
        if layout.marker_family != MarkerFamily::TagStandard41h12.as_str() {
            return Err(policy_error(
                "CORE_SCAN_MARKER_FAMILY_UNSUPPORTED",
                ErrorCategory::UnsupportedFormat,
                "the resolved page layout requires a marker family this build cannot detect",
            )
            .with_detail("marker_family", &layout.marker_family)
            .with_detail("layout_id", layout.layout_id.to_string()));
        }
        Ok(Self {
            layout,
            policy_version: SCAN_PROCESSING_POLICY_VERSION,
        })
    }

    pub const fn live_analysis_values(&self) -> LiveAnalysisPolicyValues {
        LiveAnalysisPolicyValues {
            maximum_encoded_bytes: MAXIMUM_ENCODED_BYTES as u64,
            maximum_decoded_pixels: MAXIMUM_DECODED_PIXELS,
            maximum_decoded_bytes: MAXIMUM_DECODED_BYTES,
            detector_thread_count: DETECTOR_THREAD_COUNT as u32,
            detector_quad_decimate: DETECTOR_QUAD_DECIMATE as f64,
            detector_quad_sigma: DETECTOR_QUAD_SIGMA as f64,
            detector_refine_edges: DETECTOR_REFINE_EDGES,
            detector_decode_sharpening: DETECTOR_DECODE_SHARPENING,
            detector_bits_corrected: DETECTOR_BITS_CORRECTED as u32,
            dark_luminance_cutoff: DARK_LUMINANCE_CUTOFF as u32,
            highlight_luminance_cutoff: HIGHLIGHT_LUMINANCE_CUTOFF as u32,
            quality_tile_columns: QUALITY_TILE_COLUMNS as u32,
            quality_tile_rows: QUALITY_TILE_ROWS as u32,
        }
    }

    pub fn encoded_image_limits(&self) -> Result<EncodedImageLimits, A2dError> {
        EncodedImageLimits::new(
            MAXIMUM_ENCODED_BYTES,
            MAXIMUM_DECODED_PIXELS,
            MAXIMUM_DECODED_BYTES,
        )
    }

    pub fn maximum_encoded_bytes(&self) -> usize {
        MAXIMUM_ENCODED_BYTES
    }

    pub fn image_limits(&self) -> Result<ImageLimits, A2dError> {
        ImageLimits::new(MAXIMUM_DECODED_PIXELS)
    }

    pub fn quality_measurement_config(&self) -> Result<LuminanceMeasurementConfig, A2dError> {
        LuminanceMeasurementConfig::new(
            DARK_LUMINANCE_CUTOFF,
            HIGHLIGHT_LUMINANCE_CUTOFF,
            QUALITY_TILE_COLUMNS,
            QUALITY_TILE_ROWS,
        )
    }

    pub fn detector_config(&self) -> DetectorConfig {
        DetectorConfig {
            thread_count: DETECTOR_THREAD_COUNT,
            quad_decimate: DETECTOR_QUAD_DECIMATE,
            quad_sigma: DETECTOR_QUAD_SIGMA,
            refine_edges: DETECTOR_REFINE_EDGES,
            decode_sharpening: DETECTOR_DECODE_SHARPENING,
            bits_corrected: DETECTOR_BITS_CORRECTED,
        }
    }

    pub fn marker_id_layout(&self) -> Result<MarkerIdLayout, A2dError> {
        MarkerIdLayout::new(
            self.marker_roles
                .iter()
                .map(|marker| (marker.marker_id, marker.role)),
        )
    }

    pub fn rectified_image_size(&self) -> Result<RectifiedImageSize, A2dError> {
        RectifiedImageSize::new(
            self.corrected_width,
            self.corrected_height,
            RectificationLimits::new(
                RECTIFICATION_MAXIMUM_OUTPUT_PIXELS,
                RECTIFICATION_MAXIMUM_OUTPUT_BYTES,
            )?,
        )
    }

    pub fn derived_image_config(&self) -> Result<DerivedImageConfig, A2dError> {
        DerivedImageConfig::new(
            DERIVED_PIPELINE_VERSION,
            ContrastNormalizationConfig::new(
                CONTRAST_LOW_PERCENTILE_PER_MILLION,
                CONTRAST_HIGH_PERCENTILE_PER_MILLION,
                CONTRAST_MAXIMUM_GAIN,
            )?,
            None,
            ThumbnailConfig::new(THUMBNAIL_MAXIMUM_WIDTH, THUMBNAIL_MAXIMUM_HEIGHT)?,
            DerivedImageLimits::new(
                DERIVED_MAXIMUM_PIXELS_PER_IMAGE,
                DERIVED_MAXIMUM_BYTES_PER_IMAGE,
                DERIVED_MAXIMUM_TOTAL_OUTPUT_BYTES,
                DERIVED_MAXIMUM_WORKING_BYTES,
            )?,
        )
    }

    pub fn pipeline_version(&self) -> u32 {
        DERIVED_PIPELINE_VERSION
    }
}

pub fn resolve_bundled_scan_processing_policy(
    layout_id: &LayoutId,
    requested_policy_version: u32,
) -> Result<StoredScanProcessingPolicy, A2dError> {
    if requested_policy_version != SCAN_PROCESSING_POLICY_VERSION {
        return Err(policy_error(
            "CORE_SCAN_PROCESSING_POLICY_VERSION_UNSUPPORTED",
            ErrorCategory::UnsupportedFormat,
            "the requested preview processing-policy version is unsupported",
        )
        .with_detail("layout_id", layout_id.to_string())
        .with_detail(
            "requested_policy_version",
            requested_policy_version.to_string(),
        )
        .with_detail(
            "supported_policy_version",
            SCAN_PROCESSING_POLICY_VERSION.to_string(),
        ));
    }
    StoredScanProcessingPolicy::from_resolved_layout(resolve_bundled_scan_layout(layout_id)?)
}

fn policy_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.core.scan_processing_policy",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_layout::{PaperSize, SmartPageStyle, smart_page_layout};

    use super::*;

    #[test]
    fn one_policy_constructs_all_portable_registration_configs() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let policy =
            resolve_bundled_scan_processing_policy(&layout.id, SCAN_PROCESSING_POLICY_VERSION)
                .unwrap();

        assert_eq!(policy.layout_id, layout.id);
        assert_eq!(
            (policy.corrected_width, policy.corrected_height),
            (900, 1_273)
        );
        assert_eq!(policy.maximum_encoded_bytes(), 24 * 1024 * 1024);
        assert_eq!(policy.detector_config().thread_count, 1);
        assert_eq!(policy.pipeline_version(), 1);
        assert_eq!(
            policy.live_analysis_values(),
            LiveAnalysisPolicyValues {
                maximum_encoded_bytes: 24 * 1024 * 1024,
                maximum_decoded_pixels: 32_000_000,
                maximum_decoded_bytes: 96_000_000,
                detector_thread_count: 1,
                detector_quad_decimate: 2.0,
                detector_quad_sigma: 0.0,
                detector_refine_edges: true,
                detector_decode_sharpening: 0.25,
                detector_bits_corrected: 2,
                dark_luminance_cutoff: 32,
                highlight_luminance_cutoff: 245,
                quality_tile_columns: 8,
                quality_tile_rows: 8,
            }
        );
        policy.encoded_image_limits().unwrap();
        policy.image_limits().unwrap();
        policy.quality_measurement_config().unwrap();
        policy.marker_id_layout().unwrap();
        policy.rectified_image_size().unwrap();
        policy.derived_image_config().unwrap();
    }

    #[test]
    fn unsupported_policy_version_fails_closed() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let error = resolve_bundled_scan_processing_policy(&layout.id, 2).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "CORE_SCAN_PROCESSING_POLICY_VERSION_UNSUPPORTED"
        );
    }
}
