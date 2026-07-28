//! UniFFI projections for the Milestone 7 shared image-analysis foundation.
//!
//! Android supplies owned encoded bytes and explicit limits/configuration. All decoding, grayscale
//! conversion, AprilTag detection, semantic marker resolution, and quality measurement remains in
//! `a2d-image`. This boundary returns measurements only; it does not persist files or claim that a
//! page has been accepted or saved.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_image::{
    AprilTagDetector, DetectorConfig, EncodedImage, EncodedImageFormat, EncodedImageLimits,
    ImageLimits, ImageRotation, LuminanceMeasurementConfig, MarkerIdLayout, ResolvedPageMarkers,
    measure_gray_quality, resolve_page_markers,
};
use a2d_layout::MarkerRole;

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ImageFileFormat {
    Jpeg,
    Png,
}

impl From<ImageFileFormat> for EncodedImageFormat {
    fn from(value: ImageFileFormat) -> Self {
        match value {
            ImageFileFormat::Jpeg => Self::Jpeg,
            ImageFileFormat::Png => Self::Png,
        }
    }
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum EncodedImageRotation {
    Degrees0,
    Degrees90,
    Degrees180,
    Degrees270,
}

impl From<EncodedImageRotation> for ImageRotation {
    fn from(value: EncodedImageRotation) -> Self {
        match value {
            EncodedImageRotation::Degrees0 => Self::Degrees0,
            EncodedImageRotation::Degrees90 => Self::Degrees90,
            EncodedImageRotation::Degrees180 => Self::Degrees180,
            EncodedImageRotation::Degrees270 => Self::Degrees270,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct AnalyzeEncodedPageRequest {
    pub encoded_bytes: Vec<u8>,
    pub format: ImageFileFormat,
    pub rotation: EncodedImageRotation,
    pub max_encoded_bytes: u64,
    pub max_pixels: u64,
    pub max_decoded_bytes: u64,
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
    pub top_left_tag_id: u32,
    pub top_right_tag_id: u32,
    pub bottom_right_tag_id: u32,
    pub bottom_left_tag_id: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct AnalyzedImagePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct AnalyzedMarker {
    pub role: String,
    pub family: String,
    pub id: u32,
    pub hamming_errors: u32,
    pub decision_margin: f64,
    pub center: AnalyzedImagePoint,
    pub corners: Vec<AnalyzedImagePoint>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct GrayQualityMeasurements {
    pub focus_laplacian_variance: Option<f64>,
    pub focus_interior_sample_count: Option<u64>,
    pub mean_luminance: f64,
    pub luminance_standard_deviation: f64,
    pub dark_fraction: f64,
    pub highlight_fraction: f64,
    pub max_tile_highlight_fraction: f64,
    pub populated_tile_count: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct AnalyzeEncodedPageResult {
    pub width: u32,
    pub height: u32,
    pub source_rotation_degrees: u32,
    pub resolved_orientation_degrees: u32,
    pub markers: Vec<AnalyzedMarker>,
    pub unexpected_tag_ids: Vec<u32>,
    pub quality: GrayQualityMeasurements,
}

fn request_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.image.request_invalid",
        message.into(),
        false,
    )
}

fn to_usize(value: u64, field: &'static str) -> Result<usize, A2dError> {
    usize::try_from(value).map_err(|_| {
        request_error(
            "IMAGE_FFI_LIMIT_UNSUPPORTED",
            format!("{field} value {value} does not fit this platform"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string())
    })
}

fn to_u8(value: u32, field: &'static str) -> Result<u8, A2dError> {
    u8::try_from(value).map_err(|_| {
        request_error(
            "IMAGE_FFI_PARAMETER_OUT_OF_RANGE",
            format!("{field} value {value} does not fit an unsigned byte"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string())
    })
}

fn to_u16(value: u32, field: &'static str) -> Result<u16, A2dError> {
    u16::try_from(value).map_err(|_| {
        request_error(
            "IMAGE_FFI_PARAMETER_OUT_OF_RANGE",
            format!("{field} value {value} does not fit an unsigned 16-bit integer"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string())
    })
}

fn to_f32(value: f64, field: &'static str) -> Result<f32, A2dError> {
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err(request_error(
            "IMAGE_FFI_PARAMETER_OUT_OF_RANGE",
            format!("{field} value {value} is not representable as a finite 32-bit float"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string()));
    }
    Ok(value as f32)
}

fn point(value: a2d_image::ImagePoint) -> AnalyzedImagePoint {
    AnalyzedImagePoint {
        x: value.x,
        y: value.y,
    }
}

fn analyze_encoded_page_impl(
    request: AnalyzeEncodedPageRequest,
) -> Result<AnalyzeEncodedPageResult, A2dError> {
    let encoded_limits = EncodedImageLimits::new(
        to_usize(request.max_encoded_bytes, "max_encoded_bytes")?,
        request.max_pixels,
        request.max_decoded_bytes,
    )?;
    let image_limits = ImageLimits::new(request.max_pixels)?;
    let image = EncodedImage::new(
        &request.encoded_bytes,
        request.format.into(),
        request.rotation.into(),
        encoded_limits,
    )?
    .decode_rgb8()?
    .into_gray8(image_limits)?;
    let frame = image.as_frame(image_limits)?;

    let quality = measure_gray_quality(
        frame,
        LuminanceMeasurementConfig::new(
            to_u8(request.dark_luminance_cutoff, "dark_luminance_cutoff")?,
            to_u8(
                request.highlight_luminance_cutoff,
                "highlight_luminance_cutoff",
            )?,
            to_u16(request.quality_tile_columns, "quality_tile_columns")?,
            to_u16(request.quality_tile_rows, "quality_tile_rows")?,
        )?,
    )?;

    let mut detector = AprilTagDetector::new(DetectorConfig {
        thread_count: to_u8(request.detector_thread_count, "detector_thread_count")?,
        quad_decimate: to_f32(request.detector_quad_decimate, "detector_quad_decimate")?,
        quad_sigma: to_f32(request.detector_quad_sigma, "detector_quad_sigma")?,
        refine_edges: request.detector_refine_edges,
        decode_sharpening: request.detector_decode_sharpening,
        bits_corrected: to_u8(request.detector_bits_corrected, "detector_bits_corrected")?,
    })?;
    let detections = detector.detect(frame)?;
    let marker_layout = MarkerIdLayout::new([
        (request.top_left_tag_id, MarkerRole::TopLeft),
        (request.top_right_tag_id, MarkerRole::TopRight),
        (request.bottom_right_tag_id, MarkerRole::BottomRight),
        (request.bottom_left_tag_id, MarkerRole::BottomLeft),
    ])?;
    let ResolvedPageMarkers {
        markers: resolved_markers,
        orientation,
        unexpected_tag_ids,
    } = resolve_page_markers(&detections, &marker_layout)?;

    let markers = resolved_markers
        .into_iter()
        .map(|resolved_marker| {
            let detection = resolved_marker.detection;
            AnalyzedMarker {
                role: resolved_marker.role.as_id_str().to_string(),
                family: detection.family.as_str().to_string(),
                id: detection.id,
                hamming_errors: u32::from(detection.hamming_errors),
                decision_margin: f64::from(detection.decision_margin),
                center: point(detection.center),
                corners: detection.corners.into_iter().map(point).collect(),
            }
        })
        .collect();

    Ok(AnalyzeEncodedPageResult {
        width: image.width(),
        height: image.height(),
        source_rotation_degrees: u32::from(image.rotation().degrees()),
        resolved_orientation_degrees: u32::from(orientation.degrees()),
        markers,
        unexpected_tag_ids,
        quality: GrayQualityMeasurements {
            focus_laplacian_variance: quality.focus.map(|value| value.laplacian_variance),
            focus_interior_sample_count: quality.focus.map(|value| value.interior_sample_count),
            mean_luminance: quality.exposure.mean_luminance,
            luminance_standard_deviation: quality.exposure.luminance_standard_deviation,
            dark_fraction: quality.exposure.dark_fraction,
            highlight_fraction: quality.exposure.highlight_fraction,
            max_tile_highlight_fraction: quality.glare.max_tile_highlight_fraction,
            populated_tile_count: quality.glare.populated_tile_count,
        },
    })
}

#[uniffi::export]
impl A2dClient {
    pub fn analyze_encoded_page(
        &self,
        request: AnalyzeEncodedPageRequest,
    ) -> Result<AnalyzeEncodedPageResult, A2dFfiError> {
        analyze_encoded_page_impl(request).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_request() -> AnalyzeEncodedPageRequest {
        AnalyzeEncodedPageRequest {
            encoded_bytes: include_bytes!("../../../fixtures/scans/generated/base-page.png")
                .to_vec(),
            format: ImageFileFormat::Png,
            rotation: EncodedImageRotation::Degrees0,
            max_encoded_bytes: 1_000_000,
            max_pixels: 3_000_000,
            max_decoded_bytes: 9_000_000,
            detector_thread_count: 1,
            detector_quad_decimate: 1.0,
            detector_quad_sigma: 0.0,
            detector_refine_edges: true,
            detector_decode_sharpening: 0.25,
            detector_bits_corrected: 2,
            dark_luminance_cutoff: 32,
            highlight_luminance_cutoff: 245,
            quality_tile_columns: 8,
            quality_tile_rows: 8,
            top_left_tag_id: 0,
            top_right_tag_id: 1,
            bottom_right_tag_id: 2,
            bottom_left_tag_id: 3,
        }
    }

    #[test]
    fn canonical_fixture_crosses_the_complete_shared_analysis_projection() {
        let result = analyze_encoded_page_impl(canonical_request()).unwrap();

        assert_eq!((result.width, result.height), (1400, 1900));
        assert_eq!(result.source_rotation_degrees, 0);
        assert_eq!(result.resolved_orientation_degrees, 0);
        assert_eq!(
            result
                .markers
                .iter()
                .map(|marker| (marker.role.as_str(), marker.id))
                .collect::<Vec<_>>(),
            [("TL", 0), ("TR", 1), ("BL", 3), ("BR", 2)]
        );
        assert!(result.unexpected_tag_ids.is_empty());
        assert!(result.quality.focus_laplacian_variance.unwrap() > 0.0);
        assert!(result.quality.mean_luminance.is_finite());
    }

    #[test]
    fn invalid_ffi_parameter_does_not_fall_back_to_a_default() {
        let mut request = canonical_request();
        request.detector_thread_count = 256;

        let error = analyze_encoded_page_impl(request).unwrap_err();
        assert_eq!(error.code.to_string(), "IMAGE_FFI_PARAMETER_OUT_OF_RANGE");
    }
}
