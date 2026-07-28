//! UniFFI projection for Milestone 8 full-resolution scanner review processing.
//!
//! Android supplies an owned encoded capture plus explicit detector, rectification, enhancement,
//! thumbnail, and memory policies. Rust decodes, re-detects the page, performs perspective
//! correction through the shared derived-image pipeline, and returns bounded RGB review buffers.
//! This boundary does not persist or register a scan and therefore never claims that a page is
//! saved.

use std::sync::Arc;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_image::{
    AprilTagDetector, ContrastNormalizationConfig, DerivedImageConfig, DerivedImageLimits,
    DerivedImagePipeline, DetectorConfig, EncodedImage, EncodedImageLimits, ImageLimits,
    LuminanceMeasurementConfig, MarkerIdLayout, ProcessingCancellation, RectificationLimits,
    RectificationPlan, RectifiedImageSize, ResolvedPageMarkers, SharpenConfig, ThumbnailConfig,
    measure_gray_quality, resolve_page_markers,
};
use a2d_layout::{MarkerRole, writable_page_layout};

use super::{
    A2dClient, A2dFfiError, AnalyzeEncodedPageRequest, AnalyzeEncodedPageResult,
    AnalyzedImagePoint, AnalyzedMarker, GrayQualityMeasurements,
};

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum PreviewLayoutKind {
    NotebookWritableV1,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct PreviewSharpenConfig {
    pub amount: f64,
    pub threshold: u32,
    pub passes: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ProcessEncodedPagePreviewRequest {
    pub analysis: AnalyzeEncodedPageRequest,
    pub layout_kind: PreviewLayoutKind,
    pub corrected_width: u32,
    pub corrected_height: u32,
    pub rectification_max_output_pixels: u64,
    pub rectification_max_output_bytes: u64,
    pub pipeline_version: u32,
    pub contrast_low_percentile_per_million: u32,
    pub contrast_high_percentile_per_million: u32,
    pub contrast_maximum_gain: f64,
    pub sharpening: Option<PreviewSharpenConfig>,
    pub thumbnail_max_width: u32,
    pub thumbnail_max_height: u32,
    pub derived_max_pixels_per_image: u64,
    pub derived_max_bytes_per_image: u64,
    pub derived_max_total_output_bytes: u64,
    pub derived_max_working_bytes: u64,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct PreviewRgbImage {
    pub width: u32,
    pub height: u32,
    pub rgb_bytes: Vec<u8>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ProcessEncodedPagePreviewResult {
    pub analysis: AnalyzeEncodedPageResult,
    pub corrected: PreviewRgbImage,
    pub thumbnail: PreviewRgbImage,
    pub pipeline_version: u32,
    pub source_to_corrected_matrix: Vec<f64>,
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum ProcessEncodedPagePreviewOutcome {
    Completed {
        result: ProcessEncodedPagePreviewResult,
    },
    Cancelled,
}

#[derive(Debug, uniffi::Object)]
pub struct PreviewProcessingCancellation {
    inner: ProcessingCancellation,
}

#[uniffi::export]
impl PreviewProcessingCancellation {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: ProcessingCancellation::active(),
        })
    }

    pub fn cancel(&self) {
        self.inner.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
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

fn decode_and_analyze(
    request: &AnalyzeEncodedPageRequest,
) -> Result<
    (
        a2d_image::OwnedRgbImage,
        ResolvedPageMarkers,
        AnalyzeEncodedPageResult,
    ),
    A2dError,
> {
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
    .decode_rgb8()?;
    let gray = image.clone().into_gray8(image_limits)?;
    let frame = gray.as_frame(image_limits)?;

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
    let resolved = resolve_page_markers(&detections, &marker_layout)?;

    let markers = resolved
        .markers
        .iter()
        .map(|resolved_marker| {
            let detection = &resolved_marker.detection;
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

    let analysis = AnalyzeEncodedPageResult {
        width: image.width(),
        height: image.height(),
        source_rotation_degrees: u32::from(image.rotation().degrees()),
        resolved_orientation_degrees: u32::from(resolved.orientation.degrees()),
        markers,
        unexpected_tag_ids: resolved.unexpected_tag_ids.clone(),
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
    };

    Ok((image, resolved, analysis))
}

fn is_cancelled(error: &A2dError) -> bool {
    error.code.to_string() == "DERIVED_PROCESSING_CANCELLED"
}

fn process_encoded_page_preview_impl(
    request: ProcessEncodedPagePreviewRequest,
    cancellation: &ProcessingCancellation,
) -> Result<ProcessEncodedPagePreviewOutcome, A2dError> {
    if cancellation.is_cancelled() {
        return Ok(ProcessEncodedPagePreviewOutcome::Cancelled);
    }

    let (source, resolved, analysis) = decode_and_analyze(&request.analysis)?;
    if cancellation.is_cancelled() {
        return Ok(ProcessEncodedPagePreviewOutcome::Cancelled);
    }

    let layout = match request.layout_kind {
        PreviewLayoutKind::NotebookWritableV1 => writable_page_layout(),
    };
    let output_size = RectifiedImageSize::new(
        request.corrected_width,
        request.corrected_height,
        RectificationLimits::new(
            request.rectification_max_output_pixels,
            request.rectification_max_output_bytes,
        )?,
    )?;
    let rectification = RectificationPlan::from_page_markers(
        source.width(),
        source.height(),
        &resolved,
        &layout,
        output_size,
    )?;

    let sharpening = request
        .sharpening
        .map(|value| {
            Ok(SharpenConfig::new(
                value.amount,
                to_u8(value.threshold, "sharpening.threshold")?,
                to_u8(value.passes, "sharpening.passes")?,
            )?)
        })
        .transpose()?;
    let config = DerivedImageConfig::new(
        request.pipeline_version,
        ContrastNormalizationConfig::new(
            request.contrast_low_percentile_per_million,
            request.contrast_high_percentile_per_million,
            request.contrast_maximum_gain,
        )?,
        sharpening,
        ThumbnailConfig::new(request.thumbnail_max_width, request.thumbnail_max_height)?,
        DerivedImageLimits::new(
            request.derived_max_pixels_per_image,
            request.derived_max_bytes_per_image,
            request.derived_max_total_output_bytes,
            request.derived_max_working_bytes,
        )?,
    )?;

    let derived =
        match DerivedImagePipeline::new(config).process(&source, &rectification, cancellation) {
            Ok(value) => value,
            Err(error) if is_cancelled(&error) => {
                return Ok(ProcessEncodedPagePreviewOutcome::Cancelled);
            }
            Err(error) => return Err(error),
        };
    if cancellation.is_cancelled() {
        return Ok(ProcessEncodedPagePreviewOutcome::Cancelled);
    }

    let matrix = derived
        .provenance
        .source_to_corrected_matrix
        .into_iter()
        .flat_map(|row| row.into_iter())
        .collect();
    let corrected = PreviewRgbImage {
        width: derived.corrected_color.width(),
        height: derived.corrected_color.height(),
        rgb_bytes: derived.corrected_color.into_bytes(),
    };
    let thumbnail = PreviewRgbImage {
        width: derived.thumbnail.width(),
        height: derived.thumbnail.height(),
        rgb_bytes: derived.thumbnail.into_bytes(),
    };

    Ok(ProcessEncodedPagePreviewOutcome::Completed {
        result: ProcessEncodedPagePreviewResult {
            analysis,
            corrected,
            thumbnail,
            pipeline_version: derived.provenance.pipeline_version,
            source_to_corrected_matrix: matrix,
        },
    })
}

#[uniffi::export]
impl A2dClient {
    pub fn process_encoded_page_preview(
        &self,
        request: ProcessEncodedPagePreviewRequest,
        cancellation: Arc<PreviewProcessingCancellation>,
    ) -> Result<ProcessEncodedPagePreviewOutcome, A2dFfiError> {
        process_encoded_page_preview_impl(request, &cancellation.inner).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EncodedImageRotation, ImageFileFormat};

    fn request() -> ProcessEncodedPagePreviewRequest {
        ProcessEncodedPagePreviewRequest {
            analysis: AnalyzeEncodedPageRequest {
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
            },
            layout_kind: PreviewLayoutKind::NotebookWritableV1,
            corrected_width: 600,
            corrected_height: 904,
            rectification_max_output_pixels: 1_000_000,
            rectification_max_output_bytes: 3_000_000,
            pipeline_version: 1,
            contrast_low_percentile_per_million: 10_000,
            contrast_high_percentile_per_million: 990_000,
            contrast_maximum_gain: 2.0,
            sharpening: None,
            thumbnail_max_width: 300,
            thumbnail_max_height: 300,
            derived_max_pixels_per_image: 1_000_000,
            derived_max_bytes_per_image: 3_000_000,
            derived_max_total_output_bytes: 5_000_000,
            derived_max_working_bytes: 30_000_000,
        }
    }

    #[test]
    fn canonical_capture_returns_bounded_corrected_and_thumbnail_buffers() {
        let cancellation = ProcessingCancellation::active();
        let outcome = process_encoded_page_preview_impl(request(), &cancellation).unwrap();
        let ProcessEncodedPagePreviewOutcome::Completed { result } = outcome else {
            panic!("expected completed preview processing");
        };

        assert_eq!(
            (result.corrected.width, result.corrected.height),
            (600, 904)
        );
        assert_eq!(result.corrected.rgb_bytes.len(), 600 * 904 * 3);
        assert!(result.thumbnail.width <= 300);
        assert!(result.thumbnail.height <= 300);
        assert_eq!(
            result.thumbnail.rgb_bytes.len(),
            result.thumbnail.width as usize * result.thumbnail.height as usize * 3
        );
        assert_eq!(result.source_to_corrected_matrix.len(), 9);
        assert_eq!(result.pipeline_version, 1);
        assert_eq!(result.analysis.markers.len(), 4);
    }

    #[test]
    fn cancellation_is_not_reported_as_a_processing_failure() {
        let cancellation = ProcessingCancellation::active();
        cancellation.cancel();
        let outcome = process_encoded_page_preview_impl(request(), &cancellation).unwrap();
        assert!(matches!(
            outcome,
            ProcessEncodedPagePreviewOutcome::Cancelled
        ));
    }

    #[test]
    fn invalid_output_dimensions_do_not_fall_back_to_defaults() {
        let cancellation = ProcessingCancellation::active();
        let mut invalid = request();
        invalid.corrected_width = 0;
        let error = process_encoded_page_preview_impl(invalid, &cancellation).unwrap_err();
        assert_eq!(error.code.to_string(), "RECTIFICATION_DIMENSIONS_INVALID");
    }
}
