//! Full-resolution scanner preview ABI for Android.
//!
//! Android owns the encoded capture bytes and passes them synchronously through JNA. Rust borrows
//! those bytes only for the duration of the call, re-runs marker and quality analysis, performs
//! perspective correction through the shared derived-image pipeline, and returns one versioned,
//! Rust-owned result buffer. This boundary never persists a scan or reports that a page was saved.

use std::any::Any;
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_image::{
    AprilTagDetector, ContrastNormalizationConfig, DerivedImageConfig, DerivedImageLimits,
    DerivedImagePipeline, DetectorConfig, EncodedImage, EncodedImageFormat, EncodedImageLimits,
    ImageLimits, ImageRotation, LuminanceMeasurementConfig, MarkerIdLayout, ProcessingCancellation,
    RectificationLimits, RectificationPlan, RectifiedImageSize, ThumbnailConfig,
    measure_gray_quality, resolve_page_markers,
};
use a2d_layout::{MarkerRole, writable_page_layout};

use crate::{
    AnalyzeEncodedPageResult, AnalyzedImagePoint, AnalyzedMarker, GrayQualityMeasurements,
};

const PREVIEW_CODEC_VERSION: u32 = 1;
const RESULT_MAGIC: [u8; 4] = *b"A2DP";
const ERROR_MAGIC: [u8; 4] = *b"A2PE";

pub const PREVIEW_STATUS_SUCCESS: i32 = 0;
pub const PREVIEW_STATUS_ERROR: i32 = 1;
pub const PREVIEW_STATUS_PANIC: i32 = 2;
pub const PREVIEW_STATUS_CANCELLED: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct A2dPreviewBuffer {
    pub capacity: u64,
    pub len: u64,
    pub data: *mut u8,
}

impl Default for A2dPreviewBuffer {
    fn default() -> Self {
        Self {
            capacity: 0,
            len: 0,
            data: ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A2dPreviewStatus {
    pub code: i32,
    pub error: A2dPreviewBuffer,
}

pub struct A2dPreviewCancellation {
    inner: ProcessingCancellation,
}

#[derive(Clone, Copy, Debug)]
struct PreviewConfig {
    format_code: u32,
    rotation_degrees: u32,
    max_encoded_bytes: u64,
    max_pixels: u64,
    max_decoded_bytes: u64,
    detector_thread_count: u32,
    detector_quad_decimate: f64,
    detector_quad_sigma: f64,
    detector_refine_edges: u8,
    detector_decode_sharpening: f64,
    detector_bits_corrected: u32,
    dark_luminance_cutoff: u32,
    highlight_luminance_cutoff: u32,
    quality_tile_columns: u32,
    quality_tile_rows: u32,
    top_left_tag_id: u32,
    top_right_tag_id: u32,
    bottom_right_tag_id: u32,
    bottom_left_tag_id: u32,
    corrected_width: u32,
    corrected_height: u32,
    rectification_max_output_pixels: u64,
    rectification_max_output_bytes: u64,
    pipeline_version: u32,
    contrast_low_percentile_per_million: u32,
    contrast_high_percentile_per_million: u32,
    contrast_maximum_gain: f64,
    thumbnail_max_width: u32,
    thumbnail_max_height: u32,
    derived_max_pixels_per_image: u64,
    derived_max_bytes_per_image: u64,
    derived_max_total_output_bytes: u64,
    derived_max_working_bytes: u64,
}

#[derive(Debug)]
struct PreviewRgbImage {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct PreviewResult {
    analysis: AnalyzeEncodedPageResult,
    corrected: PreviewRgbImage,
    thumbnail: PreviewRgbImage,
    pipeline_version: u32,
    source_to_corrected_matrix: [f64; 9],
}

enum PreviewOutcome {
    Completed(PreviewResult),
    Cancelled,
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

fn codec_error(message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new("PREVIEW_CODEC_ERROR"),
        ErrorCategory::Internal,
        ErrorSeverity::Critical,
        "error.internal_unknown",
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

fn to_bool(value: u8, field: &'static str) -> Result<bool, A2dError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(request_error(
            "IMAGE_FFI_PARAMETER_OUT_OF_RANGE",
            format!("{field} must be encoded as 0 or 1, got {value}"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string())),
    }
}

fn image_format(value: u32) -> Result<EncodedImageFormat, A2dError> {
    match value {
        0 => Ok(EncodedImageFormat::Jpeg),
        1 => Ok(EncodedImageFormat::Png),
        _ => Err(request_error(
            "IMAGE_FORMAT_INVALID",
            format!("format_code must be 0 (JPEG) or 1 (PNG), got {value}"),
        )),
    }
}

fn image_rotation(value: u32) -> Result<ImageRotation, A2dError> {
    match value {
        0 => Ok(ImageRotation::Degrees0),
        90 => Ok(ImageRotation::Degrees90),
        180 => Ok(ImageRotation::Degrees180),
        270 => Ok(ImageRotation::Degrees270),
        _ => Err(request_error(
            "IMAGE_ROTATION_INVALID",
            format!("rotation_degrees must be 0, 90, 180, or 270, got {value}"),
        )),
    }
}

fn point(value: a2d_image::ImagePoint) -> AnalyzedImagePoint {
    AnalyzedImagePoint {
        x: value.x,
        y: value.y,
    }
}

fn cancelled(error: &A2dError) -> bool {
    error.code.to_string() == "DERIVED_PROCESSING_CANCELLED"
}

fn process_preview(
    encoded_bytes: &[u8],
    config: PreviewConfig,
    cancellation: &ProcessingCancellation,
) -> Result<PreviewOutcome, A2dError> {
    if cancellation.is_cancelled() {
        return Ok(PreviewOutcome::Cancelled);
    }

    let encoded_limits = EncodedImageLimits::new(
        to_usize(config.max_encoded_bytes, "max_encoded_bytes")?,
        config.max_pixels,
        config.max_decoded_bytes,
    )?;
    let image_limits = ImageLimits::new(config.max_pixels)?;
    let source = EncodedImage::new(
        encoded_bytes,
        image_format(config.format_code)?,
        image_rotation(config.rotation_degrees)?,
        encoded_limits,
    )?
    .decode_rgb8()?;
    let gray = source.clone().into_gray8(image_limits)?;
    let frame = gray.as_frame(image_limits)?;

    let quality = measure_gray_quality(
        frame,
        LuminanceMeasurementConfig::new(
            to_u8(config.dark_luminance_cutoff, "dark_luminance_cutoff")?,
            to_u8(
                config.highlight_luminance_cutoff,
                "highlight_luminance_cutoff",
            )?,
            to_u16(config.quality_tile_columns, "quality_tile_columns")?,
            to_u16(config.quality_tile_rows, "quality_tile_rows")?,
        )?,
    )?;
    let mut detector = AprilTagDetector::new(DetectorConfig {
        thread_count: to_u8(config.detector_thread_count, "detector_thread_count")?,
        quad_decimate: to_f32(config.detector_quad_decimate, "detector_quad_decimate")?,
        quad_sigma: to_f32(config.detector_quad_sigma, "detector_quad_sigma")?,
        refine_edges: to_bool(config.detector_refine_edges, "detector_refine_edges")?,
        decode_sharpening: config.detector_decode_sharpening,
        bits_corrected: to_u8(config.detector_bits_corrected, "detector_bits_corrected")?,
    })?;
    let detections = detector.detect(frame)?;
    let marker_layout = MarkerIdLayout::new([
        (config.top_left_tag_id, MarkerRole::TopLeft),
        (config.top_right_tag_id, MarkerRole::TopRight),
        (config.bottom_right_tag_id, MarkerRole::BottomRight),
        (config.bottom_left_tag_id, MarkerRole::BottomLeft),
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
        width: source.width(),
        height: source.height(),
        source_rotation_degrees: u32::from(source.rotation().degrees()),
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
    if cancellation.is_cancelled() {
        return Ok(PreviewOutcome::Cancelled);
    }

    let output_size = RectifiedImageSize::new(
        config.corrected_width,
        config.corrected_height,
        RectificationLimits::new(
            config.rectification_max_output_pixels,
            config.rectification_max_output_bytes,
        )?,
    )?;
    let layout = writable_page_layout();
    let rectification = RectificationPlan::from_page_markers(
        source.width(),
        source.height(),
        &resolved,
        &layout,
        output_size,
    )?;
    let derived_config = DerivedImageConfig::new(
        config.pipeline_version,
        ContrastNormalizationConfig::new(
            config.contrast_low_percentile_per_million,
            config.contrast_high_percentile_per_million,
            config.contrast_maximum_gain,
        )?,
        None,
        ThumbnailConfig::new(config.thumbnail_max_width, config.thumbnail_max_height)?,
        DerivedImageLimits::new(
            config.derived_max_pixels_per_image,
            config.derived_max_bytes_per_image,
            config.derived_max_total_output_bytes,
            config.derived_max_working_bytes,
        )?,
    )?;
    let derived = match DerivedImagePipeline::new(derived_config).process(
        &source,
        &rectification,
        cancellation,
    ) {
        Ok(value) => value,
        Err(error) if cancelled(&error) => return Ok(PreviewOutcome::Cancelled),
        Err(error) => return Err(error),
    };
    if cancellation.is_cancelled() {
        return Ok(PreviewOutcome::Cancelled);
    }

    let matrix_rows = derived.provenance.source_to_corrected_matrix;
    let source_to_corrected_matrix = [
        matrix_rows[0][0],
        matrix_rows[0][1],
        matrix_rows[0][2],
        matrix_rows[1][0],
        matrix_rows[1][1],
        matrix_rows[1][2],
        matrix_rows[2][0],
        matrix_rows[2][1],
        matrix_rows[2][2],
    ];
    Ok(PreviewOutcome::Completed(PreviewResult {
        analysis,
        corrected: PreviewRgbImage {
            width: derived.corrected_color.width(),
            height: derived.corrected_color.height(),
            bytes: derived.corrected_color.into_bytes(),
        },
        thumbnail: PreviewRgbImage {
            width: derived.thumbnail.width(),
            height: derived.thumbnail.height(),
            bytes: derived.thumbnail.into_bytes(),
        },
        pipeline_version: derived.provenance.pipeline_version,
        source_to_corrected_matrix,
    }))
}

struct BinaryWriter {
    bytes: Vec<u8>,
}

impl BinaryWriter {
    fn new(magic: [u8; 4], capacity: usize) -> Self {
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(&magic);
        bytes.extend_from_slice(&PREVIEW_CODEC_VERSION.to_be_bytes());
        Self { bytes }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn length(&mut self, value: usize, field: &'static str) -> Result<(), A2dError> {
        self.u32(
            u32::try_from(value)
                .map_err(|_| codec_error(format!("{field} length exceeds the preview codec")))?,
        );
        Ok(())
    }

    fn string(&mut self, value: &str, field: &'static str) -> Result<(), A2dError> {
        self.length(value.len(), field)?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn raw_bytes(&mut self, value: &[u8], field: &'static str) -> Result<(), A2dError> {
        self.length(value.len(), field)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn optional_f64(&mut self, value: Option<f64>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.f64(value);
            }
            None => self.u8(0),
        }
    }

    fn optional_u64(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.u64(value);
            }
            None => self.u8(0),
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn encode_analysis(
    writer: &mut BinaryWriter,
    result: &AnalyzeEncodedPageResult,
) -> Result<(), A2dError> {
    writer.u32(result.width);
    writer.u32(result.height);
    writer.u32(result.source_rotation_degrees);
    writer.u32(result.resolved_orientation_degrees);
    writer.length(result.markers.len(), "markers")?;
    for marker in &result.markers {
        writer.string(&marker.role, "marker.role")?;
        writer.string(&marker.family, "marker.family")?;
        writer.u32(marker.id);
        writer.u32(marker.hamming_errors);
        writer.f64(marker.decision_margin);
        writer.f64(marker.center.x);
        writer.f64(marker.center.y);
        writer.length(marker.corners.len(), "marker.corners")?;
        for corner in &marker.corners {
            writer.f64(corner.x);
            writer.f64(corner.y);
        }
    }
    writer.length(result.unexpected_tag_ids.len(), "unexpected_tag_ids")?;
    for tag_id in &result.unexpected_tag_ids {
        writer.u32(*tag_id);
    }
    writer.optional_f64(result.quality.focus_laplacian_variance);
    writer.optional_u64(result.quality.focus_interior_sample_count);
    writer.f64(result.quality.mean_luminance);
    writer.f64(result.quality.luminance_standard_deviation);
    writer.f64(result.quality.dark_fraction);
    writer.f64(result.quality.highlight_fraction);
    writer.f64(result.quality.max_tile_highlight_fraction);
    writer.u32(result.quality.populated_tile_count);
    Ok(())
}

fn encode_result(result: &PreviewResult) -> Result<Vec<u8>, A2dError> {
    let image_bytes = result
        .corrected
        .bytes
        .len()
        .checked_add(result.thumbnail.bytes.len())
        .ok_or_else(|| codec_error("preview result byte count overflowed"))?;
    let capacity = image_bytes
        .checked_add(2_048)
        .ok_or_else(|| codec_error("preview result capacity overflowed"))?;
    let mut writer = BinaryWriter::new(RESULT_MAGIC, capacity);
    writer.u32(result.pipeline_version);
    writer.length(
        result.source_to_corrected_matrix.len(),
        "source_to_corrected_matrix",
    )?;
    for value in result.source_to_corrected_matrix {
        writer.f64(value);
    }
    encode_analysis(&mut writer, &result.analysis)?;
    writer.u32(result.corrected.width);
    writer.u32(result.corrected.height);
    writer.raw_bytes(&result.corrected.bytes, "corrected.rgb_bytes")?;
    writer.u32(result.thumbnail.width);
    writer.u32(result.thumbnail.height);
    writer.raw_bytes(&result.thumbnail.bytes, "thumbnail.rgb_bytes")?;
    Ok(writer.finish())
}

fn encode_error(error: &A2dError) -> Vec<u8> {
    let fields = [
        error.code.to_string(),
        format!("{:?}", error.category),
        format!("{:?}", error.severity),
        error.user_message_key.clone(),
        error.developer_message.clone(),
        error.correlation_id.clone(),
    ];
    if fields
        .iter()
        .any(|value| u32::try_from(value.len()).is_err())
    {
        return encode_static_error();
    }
    let mut writer = BinaryWriter::new(ERROR_MAGIC, 512);
    for value in fields {
        if writer.string(&value, "error field").is_err() {
            return encode_static_error();
        }
    }
    writer.u8(u8::from(error.retryable));
    writer.finish()
}

fn encode_static_error() -> Vec<u8> {
    let mut writer = BinaryWriter::new(ERROR_MAGIC, 256);
    for value in [
        "PREVIEW_ERROR_ENCODING_FAILED",
        "Internal",
        "Critical",
        "error.internal_unknown",
        "preview-processing error fields exceeded the codec limit",
        "unavailable",
    ] {
        writer
            .string(value, "static error field")
            .expect("static preview error fields fit u32");
    }
    writer.u8(0);
    writer.finish()
}

fn into_buffer(mut bytes: Vec<u8>) -> A2dPreviewBuffer {
    if bytes.is_empty() {
        return A2dPreviewBuffer::default();
    }
    let buffer = A2dPreviewBuffer {
        capacity: bytes.capacity() as u64,
        len: bytes.len() as u64,
        data: bytes.as_mut_ptr(),
    };
    mem::forget(bytes);
    buffer
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

unsafe fn set_status(status: *mut A2dPreviewStatus, code: i32, error: A2dPreviewBuffer) {
    // SAFETY: callers validate `status` before this helper and promise writable storage.
    unsafe { ptr::write(status, A2dPreviewStatus { code, error }) };
}

#[unsafe(no_mangle)]
pub extern "C" fn a2d_preview_cancellation_new() -> *mut A2dPreviewCancellation {
    Box::into_raw(Box::new(A2dPreviewCancellation {
        inner: ProcessingCancellation::active(),
    }))
}

/// # Safety
///
/// `cancellation` must be null or a live pointer returned by
/// [`a2d_preview_cancellation_new`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_preview_cancellation_cancel(
    cancellation: *const A2dPreviewCancellation,
) {
    if let Some(cancellation) = unsafe { cancellation.as_ref() } {
        cancellation.inner.cancel();
    }
}

/// # Safety
///
/// `cancellation` must be null or a live pointer returned by
/// [`a2d_preview_cancellation_new`] that has not previously been freed. It must not be freed while a
/// processing call is still borrowing it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_preview_cancellation_free(cancellation: *mut A2dPreviewCancellation) {
    if !cancellation.is_null() {
        // SAFETY: required by this function's contract.
        unsafe { drop(Box::from_raw(cancellation)) };
    }
}

/// Process one borrowed encoded capture synchronously.
///
/// Result and error buffers are Rust-owned and must be released exactly once with
/// [`a2d_preview_buffer_free`]. Cancellation is returned as an explicit status with empty buffers.
/// Panics are caught and never unwind across the ABI.
///
/// # Safety
///
/// - `status` must point to writable [`A2dPreviewStatus`] storage.
/// - `cancellation` must point to a live [`A2dPreviewCancellation`] for the full call.
/// - when `bytes_len` is non-zero, `bytes` must point to at least `bytes_len` readable bytes that
///   remain alive and unmodified until this function returns.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn a2d_process_encoded_page_preview(
    bytes: *const u8,
    bytes_len: u64,
    format_code: u32,
    rotation_degrees: u32,
    max_encoded_bytes: u64,
    max_pixels: u64,
    max_decoded_bytes: u64,
    detector_thread_count: u32,
    detector_quad_decimate: f64,
    detector_quad_sigma: f64,
    detector_refine_edges: u8,
    detector_decode_sharpening: f64,
    detector_bits_corrected: u32,
    dark_luminance_cutoff: u32,
    highlight_luminance_cutoff: u32,
    quality_tile_columns: u32,
    quality_tile_rows: u32,
    top_left_tag_id: u32,
    top_right_tag_id: u32,
    bottom_right_tag_id: u32,
    bottom_left_tag_id: u32,
    corrected_width: u32,
    corrected_height: u32,
    rectification_max_output_pixels: u64,
    rectification_max_output_bytes: u64,
    pipeline_version: u32,
    contrast_low_percentile_per_million: u32,
    contrast_high_percentile_per_million: u32,
    contrast_maximum_gain: f64,
    thumbnail_max_width: u32,
    thumbnail_max_height: u32,
    derived_max_pixels_per_image: u64,
    derived_max_bytes_per_image: u64,
    derived_max_total_output_bytes: u64,
    derived_max_working_bytes: u64,
    cancellation: *const A2dPreviewCancellation,
    status: *mut A2dPreviewStatus,
) -> A2dPreviewBuffer {
    if status.is_null() {
        return A2dPreviewBuffer::default();
    }
    // SAFETY: status is non-null and writable by contract.
    unsafe { set_status(status, PREVIEW_STATUS_SUCCESS, A2dPreviewBuffer::default()) };

    let execution = catch_unwind(AssertUnwindSafe(|| {
        let byte_count = to_usize(bytes_len, "bytes_len")?;
        let encoded_bytes = if byte_count == 0 {
            &[][..]
        } else {
            if bytes.is_null() {
                return Err(request_error(
                    "IMAGE_FFI_NULL_BUFFER",
                    "bytes pointer must not be null when bytes_len is non-zero",
                ));
            }
            // SAFETY: required by the exported function's contract.
            unsafe { slice::from_raw_parts(bytes, byte_count) }
        };
        let cancellation = unsafe { cancellation.as_ref() }.ok_or_else(|| {
            request_error(
                "PREVIEW_CANCELLATION_NULL",
                "cancellation pointer must not be null",
            )
        })?;
        process_preview(
            encoded_bytes,
            PreviewConfig {
                format_code,
                rotation_degrees,
                max_encoded_bytes,
                max_pixels,
                max_decoded_bytes,
                detector_thread_count,
                detector_quad_decimate,
                detector_quad_sigma,
                detector_refine_edges,
                detector_decode_sharpening,
                detector_bits_corrected,
                dark_luminance_cutoff,
                highlight_luminance_cutoff,
                quality_tile_columns,
                quality_tile_rows,
                top_left_tag_id,
                top_right_tag_id,
                bottom_right_tag_id,
                bottom_left_tag_id,
                corrected_width,
                corrected_height,
                rectification_max_output_pixels,
                rectification_max_output_bytes,
                pipeline_version,
                contrast_low_percentile_per_million,
                contrast_high_percentile_per_million,
                contrast_maximum_gain,
                thumbnail_max_width,
                thumbnail_max_height,
                derived_max_pixels_per_image,
                derived_max_bytes_per_image,
                derived_max_total_output_bytes,
                derived_max_working_bytes,
            },
            &cancellation.inner,
        )
    }));

    match execution {
        Ok(Ok(PreviewOutcome::Completed(result))) => match encode_result(&result) {
            Ok(encoded) => into_buffer(encoded),
            Err(error) => {
                let error_buffer = into_buffer(encode_error(&error));
                // SAFETY: status remains writable for the duration of the call.
                unsafe { set_status(status, PREVIEW_STATUS_ERROR, error_buffer) };
                A2dPreviewBuffer::default()
            }
        },
        Ok(Ok(PreviewOutcome::Cancelled)) => {
            // SAFETY: status remains writable for the duration of the call.
            unsafe {
                set_status(
                    status,
                    PREVIEW_STATUS_CANCELLED,
                    A2dPreviewBuffer::default(),
                )
            };
            A2dPreviewBuffer::default()
        }
        Ok(Err(error)) => {
            let error_buffer = into_buffer(encode_error(&error));
            // SAFETY: status remains writable for the duration of the call.
            unsafe { set_status(status, PREVIEW_STATUS_ERROR, error_buffer) };
            A2dPreviewBuffer::default()
        }
        Err(payload) => {
            let error = A2dError::internal_unknown(format!(
                "full-resolution preview processing panicked: {}",
                panic_message(payload.as_ref())
            ));
            let error_buffer = into_buffer(encode_error(&error));
            // SAFETY: status remains writable for the duration of the call.
            unsafe { set_status(status, PREVIEW_STATUS_PANIC, error_buffer) };
            A2dPreviewBuffer::default()
        }
    }
}

/// # Safety
///
/// `buffer` must be the zero/default buffer or an unmodified buffer returned by
/// [`a2d_process_encoded_page_preview`] that has not previously been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_preview_buffer_free(buffer: A2dPreviewBuffer) {
    if buffer.data.is_null() {
        return;
    }
    let Ok(capacity) = usize::try_from(buffer.capacity) else {
        return;
    };
    let Ok(len) = usize::try_from(buffer.len) else {
        return;
    };
    if capacity == 0 || len > capacity {
        return;
    }
    // SAFETY: required by this function's contract; pointer/capacity came from `into_buffer`.
    unsafe { drop(Vec::from_raw_parts(buffer.data, len, capacity)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_processing_is_structurally_distinct_from_failure() {
        let cancellation = ProcessingCancellation::active();
        cancellation.cancel();
        let outcome = process_preview(
            &[0xff, 0xd8, 0xff, 0xd9],
            PreviewConfig {
                format_code: 0,
                rotation_degrees: 0,
                max_encoded_bytes: 1024,
                max_pixels: 1024,
                max_decoded_bytes: 3072,
                detector_thread_count: 1,
                detector_quad_decimate: 1.0,
                detector_quad_sigma: 0.0,
                detector_refine_edges: 1,
                detector_decode_sharpening: 0.25,
                detector_bits_corrected: 2,
                dark_luminance_cutoff: 32,
                highlight_luminance_cutoff: 245,
                quality_tile_columns: 1,
                quality_tile_rows: 1,
                top_left_tag_id: 0,
                top_right_tag_id: 1,
                bottom_right_tag_id: 2,
                bottom_left_tag_id: 3,
                corrected_width: 2,
                corrected_height: 2,
                rectification_max_output_pixels: 4,
                rectification_max_output_bytes: 12,
                pipeline_version: 1,
                contrast_low_percentile_per_million: 10_000,
                contrast_high_percentile_per_million: 990_000,
                contrast_maximum_gain: 2.0,
                thumbnail_max_width: 2,
                thumbnail_max_height: 2,
                derived_max_pixels_per_image: 4,
                derived_max_bytes_per_image: 12,
                derived_max_total_output_bytes: 20,
                derived_max_working_bytes: 64,
            },
            &cancellation,
        )
        .unwrap();
        assert!(matches!(outcome, PreviewOutcome::Cancelled));
    }

    #[test]
    fn result_codec_has_magic_version_and_bounded_image_lengths() {
        let analysis = AnalyzeEncodedPageResult {
            width: 10,
            height: 20,
            source_rotation_degrees: 0,
            resolved_orientation_degrees: 0,
            markers: Vec::new(),
            unexpected_tag_ids: Vec::new(),
            quality: GrayQualityMeasurements {
                focus_laplacian_variance: None,
                focus_interior_sample_count: None,
                mean_luminance: 128.0,
                luminance_standard_deviation: 0.0,
                dark_fraction: 0.0,
                highlight_fraction: 0.0,
                max_tile_highlight_fraction: 0.0,
                populated_tile_count: 1,
            },
        };
        let encoded = encode_result(&PreviewResult {
            analysis,
            corrected: PreviewRgbImage {
                width: 1,
                height: 1,
                bytes: vec![1, 2, 3],
            },
            thumbnail: PreviewRgbImage {
                width: 1,
                height: 1,
                bytes: vec![4, 5, 6],
            },
            pipeline_version: 1,
            source_to_corrected_matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        })
        .unwrap();
        assert_eq!(&encoded[..4], &RESULT_MAGIC);
        assert_eq!(u32::from_be_bytes(encoded[4..8].try_into().unwrap()), 1);
        assert!(encoded.len() < 1024);
    }
}
