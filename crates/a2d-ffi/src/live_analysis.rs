//! Borrowed grayscale live-analysis ABI for Android camera frames.
//!
//! This narrow C ABI intentionally stays outside UniFFI metadata. Camera frames are already owned
//! by Android in a direct `ByteBuffer`; accepting a borrowed pointer avoids serializing the full
//! luminance plane into a second RustBuffer on every preview frame. Rust validates every scalar and
//! borrows the bytes only for the duration of the synchronous call. Result and error buffers remain
//! Rust-owned until the caller releases them with `a2d_live_analysis_buffer_free`.

use std::any::Any;
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};
use a2d_image::{
    AprilTagDetector, DetectorConfig, GrayFrame, ImageLimits, ImageRotation,
    LuminanceMeasurementConfig, MarkerIdLayout, ResolvedPageMarkers, measure_gray_quality,
    resolve_page_markers,
};
use a2d_layout::MarkerRole;

use crate::{
    AnalyzeEncodedPageResult, AnalyzedImagePoint, AnalyzedMarker, GrayQualityMeasurements,
};

const LIVE_ANALYSIS_CODEC_VERSION: u32 = 1;
const RESULT_MAGIC: [u8; 4] = *b"A2DR";
const ERROR_MAGIC: [u8; 4] = *b"A2DE";

pub const LIVE_ANALYSIS_STATUS_SUCCESS: i32 = 0;
pub const LIVE_ANALYSIS_STATUS_ERROR: i32 = 1;
pub const LIVE_ANALYSIS_STATUS_PANIC: i32 = 2;

/// Rust-owned bytes returned by the live-analysis ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct A2dLiveAnalysisBuffer {
    pub capacity: u64,
    pub len: u64,
    pub data: *mut u8,
}

impl Default for A2dLiveAnalysisBuffer {
    fn default() -> Self {
        Self {
            capacity: 0,
            len: 0,
            data: ptr::null_mut(),
        }
    }
}

/// Explicit status for the live-analysis ABI. `error` is populated for non-success codes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A2dLiveAnalysisStatus {
    pub code: i32,
    pub error: A2dLiveAnalysisBuffer,
}

#[derive(Clone, Copy, Debug)]
struct LiveAnalysisConfig {
    max_pixels: u64,
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
        ErrorCode::new("LIVE_ANALYSIS_CODEC_ERROR"),
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

fn rotation_from_degrees(value: u32) -> Result<ImageRotation, A2dError> {
    match value {
        0 => Ok(ImageRotation::Degrees0),
        90 => Ok(ImageRotation::Degrees90),
        180 => Ok(ImageRotation::Degrees180),
        270 => Ok(ImageRotation::Degrees270),
        _ => Err(request_error(
            "IMAGE_ROTATION_INVALID",
            format!("rotation_degrees must be 0, 90, 180, or 270, got {value}"),
        )
        .with_detail("rotation_degrees", value.to_string())),
    }
}

fn point(value: a2d_image::ImagePoint) -> AnalyzedImagePoint {
    AnalyzedImagePoint {
        x: value.x,
        y: value.y,
    }
}

fn analyze_gray_frame_impl(
    width: u32,
    height: u32,
    row_stride: u64,
    rotation_degrees: u32,
    bytes: &[u8],
    config: LiveAnalysisConfig,
) -> Result<AnalyzeEncodedPageResult, A2dError> {
    let image_limits = ImageLimits::new(config.max_pixels)?;
    let frame = GrayFrame::new(
        width,
        height,
        to_usize(row_stride, "row_stride")?,
        rotation_from_degrees(rotation_degrees)?,
        bytes,
        image_limits,
    )?;

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
        width: frame.width(),
        height: frame.height(),
        source_rotation_degrees: u32::from(frame.rotation().degrees()),
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

struct BinaryWriter {
    bytes: Vec<u8>,
}

impl BinaryWriter {
    fn new(magic: [u8; 4]) -> Self {
        let mut bytes = Vec::with_capacity(512);
        bytes.extend_from_slice(&magic);
        bytes.extend_from_slice(&LIVE_ANALYSIS_CODEC_VERSION.to_be_bytes());
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

    fn count(&mut self, value: usize, field: &'static str) -> Result<(), A2dError> {
        let value = u32::try_from(value).map_err(|_| {
            codec_error(format!(
                "{field} count does not fit the live-analysis codec"
            ))
        })?;
        self.u32(value);
        Ok(())
    }

    fn string(&mut self, value: &str, field: &'static str) -> Result<(), A2dError> {
        self.count(value.len(), field)?;
        self.bytes.extend_from_slice(value.as_bytes());
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

fn encode_result(result: &AnalyzeEncodedPageResult) -> Result<Vec<u8>, A2dError> {
    let mut writer = BinaryWriter::new(RESULT_MAGIC);
    writer.u32(result.width);
    writer.u32(result.height);
    writer.u32(result.source_rotation_degrees);
    writer.u32(result.resolved_orientation_degrees);
    writer.count(result.markers.len(), "markers")?;
    for marker in &result.markers {
        writer.string(&marker.role, "marker.role")?;
        writer.string(&marker.family, "marker.family")?;
        writer.u32(marker.id);
        writer.u32(marker.hamming_errors);
        writer.f64(marker.decision_margin);
        writer.f64(marker.center.x);
        writer.f64(marker.center.y);
        writer.count(marker.corners.len(), "marker.corners")?;
        for corner in &marker.corners {
            writer.f64(corner.x);
            writer.f64(corner.y);
        }
    }
    writer.count(result.unexpected_tag_ids.len(), "unexpected_tag_ids")?;
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
    Ok(writer.finish())
}

fn encode_error(error: &A2dError) -> Vec<u8> {
    let category = format!("{:?}", error.category);
    let severity = format!("{:?}", error.severity);
    let fields = [
        error.code.to_string(),
        category,
        severity,
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

    let mut writer = BinaryWriter::new(ERROR_MAGIC);
    for (index, value) in fields.iter().enumerate() {
        writer
            .string(value, "error field")
            .unwrap_or_else(|_| panic!("prevalidated error field {index} failed to encode"));
    }
    writer.u8(u8::from(error.retryable));
    writer.finish()
}

fn encode_static_error() -> Vec<u8> {
    let mut writer = BinaryWriter::new(ERROR_MAGIC);
    for value in [
        "LIVE_ANALYSIS_ERROR_ENCODING_FAILED",
        "Internal",
        "Critical",
        "error.internal_unknown",
        "live-analysis error fields exceeded the codec limit",
        "unavailable",
    ] {
        writer
            .string(value, "static error field")
            .expect("static live-analysis error fields fit u32");
    }
    writer.u8(0);
    writer.finish()
}

fn into_buffer(mut bytes: Vec<u8>) -> A2dLiveAnalysisBuffer {
    if bytes.is_empty() {
        return A2dLiveAnalysisBuffer::default();
    }
    let buffer = A2dLiveAnalysisBuffer {
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

unsafe fn set_status(status: *mut A2dLiveAnalysisStatus, code: i32, error: A2dLiveAnalysisBuffer) {
    // SAFETY: the exported function validates `status` is non-null before calling this helper, and
    // the caller promises it points to writable `A2dLiveAnalysisStatus` storage for the call.
    unsafe { ptr::write(status, A2dLiveAnalysisStatus { code, error }) };
}

#[allow(clippy::too_many_arguments)]
fn execute_live_analysis(
    width: u32,
    height: u32,
    row_stride: u64,
    rotation_degrees: u32,
    bytes: &[u8],
    config: LiveAnalysisConfig,
) -> Result<Vec<u8>, A2dError> {
    let result =
        analyze_gray_frame_impl(width, height, row_stride, rotation_degrees, bytes, config)?;
    encode_result(&result)
}

/// Analyze one borrowed Gray8 frame synchronously.
///
/// The input pointer is never retained. Successful and error output buffers are owned by Rust and
/// must be released exactly once with [`a2d_live_analysis_buffer_free`]. A panic is caught and
/// reported as `LIVE_ANALYSIS_STATUS_PANIC`; it never unwinds across the ABI.
///
/// # Safety
///
/// - `status` must point to writable `A2dLiveAnalysisStatus` storage for the duration of this call.
/// - when `bytes_len` is non-zero, `bytes` must point to at least `bytes_len` readable bytes that
///   remain alive and unmodified until this function returns.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn a2d_live_analyze_gray_frame(
    bytes: *const u8,
    bytes_len: u64,
    width: u32,
    height: u32,
    row_stride: u64,
    rotation_degrees: u32,
    max_pixels: u64,
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
    status: *mut A2dLiveAnalysisStatus,
) -> A2dLiveAnalysisBuffer {
    if status.is_null() {
        return A2dLiveAnalysisBuffer::default();
    }
    // SAFETY: non-null was checked and the function's safety contract requires writable storage.
    unsafe {
        set_status(
            status,
            LIVE_ANALYSIS_STATUS_SUCCESS,
            A2dLiveAnalysisBuffer::default(),
        )
    };

    let execution = catch_unwind(AssertUnwindSafe(|| {
        let byte_count = to_usize(bytes_len, "bytes_len")?;
        let borrowed = if byte_count == 0 {
            &[][..]
        } else {
            if bytes.is_null() {
                return Err(request_error(
                    "IMAGE_FFI_NULL_BUFFER",
                    "bytes pointer must not be null when bytes_len is non-zero",
                ));
            }
            // SAFETY: required by this function's contract and bounded by the checked byte count.
            unsafe { slice::from_raw_parts(bytes, byte_count) }
        };
        execute_live_analysis(
            width,
            height,
            row_stride,
            rotation_degrees,
            borrowed,
            LiveAnalysisConfig {
                max_pixels,
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
            },
        )
    }));

    match execution {
        Ok(Ok(encoded_result)) => into_buffer(encoded_result),
        Ok(Err(error)) => {
            let error_buffer = into_buffer(encode_error(&error));
            // SAFETY: validated at function entry and still live for the duration of the call.
            unsafe { set_status(status, LIVE_ANALYSIS_STATUS_ERROR, error_buffer) };
            A2dLiveAnalysisBuffer::default()
        }
        Err(payload) => {
            let error = A2dError::internal_unknown(format!(
                "live grayscale analysis panicked: {}",
                panic_message(payload.as_ref())
            ));
            let error_buffer = into_buffer(encode_error(&error));
            // SAFETY: validated at function entry and still live for the duration of the call.
            unsafe { set_status(status, LIVE_ANALYSIS_STATUS_PANIC, error_buffer) };
            A2dLiveAnalysisBuffer::default()
        }
    }
}

/// Release a Rust-owned buffer returned by the live-analysis ABI.
///
/// # Safety
///
/// `buffer` must either be the zero/default buffer or an unmodified buffer returned by
/// [`a2d_live_analyze_gray_frame`] that has not previously been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_live_analysis_buffer_free(buffer: A2dLiveAnalysisBuffer) {
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
    // SAFETY: guaranteed by the function contract; the pointer/capacity came from `into_buffer`.
    unsafe { drop(Vec::from_raw_parts(buffer.data, len, capacity)) };
}

#[cfg(test)]
mod tests {
    use a2d_image::{EncodedImage, EncodedImageFormat, EncodedImageLimits};

    use super::*;

    fn config() -> LiveAnalysisConfig {
        LiveAnalysisConfig {
            max_pixels: 3_000_000,
            detector_thread_count: 1,
            detector_quad_decimate: 1.0,
            detector_quad_sigma: 0.0,
            detector_refine_edges: 1,
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

    fn canonical_gray_image() -> a2d_image::OwnedGrayImage {
        EncodedImage::new(
            include_bytes!("../../../fixtures/scans/generated/base-page.png"),
            EncodedImageFormat::Png,
            ImageRotation::Degrees0,
            EncodedImageLimits::new(1_000_000, 3_000_000, 9_000_000).unwrap(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap()
        .into_gray8(ImageLimits::new(3_000_000).unwrap())
        .unwrap()
    }

    #[test]
    fn borrowed_gray_path_runs_the_complete_detector_and_quality_projection() {
        let image = canonical_gray_image();
        let result = analyze_gray_frame_impl(
            image.width(),
            image.height(),
            image.row_stride() as u64,
            u32::from(image.rotation().degrees()),
            image.bytes(),
            config(),
        )
        .unwrap();

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
    }

    #[test]
    fn result_codec_is_versioned_and_nonempty() {
        let image = canonical_gray_image();
        let encoded = execute_live_analysis(
            image.width(),
            image.height(),
            image.row_stride() as u64,
            0,
            image.bytes(),
            config(),
        )
        .unwrap();

        assert_eq!(&encoded[..4], &RESULT_MAGIC);
        assert_eq!(u32::from_be_bytes(encoded[4..8].try_into().unwrap()), 1);
        assert!(encoded.len() > 100);
    }

    #[test]
    fn null_input_pointer_is_an_explicit_error_not_undefined_behavior() {
        let mut status = A2dLiveAnalysisStatus::default();
        // SAFETY: status is writable and the deliberately null data pointer is paired with a
        // non-zero length so the boundary's validation path is exercised without dereferencing it.
        let result = unsafe {
            a2d_live_analyze_gray_frame(
                ptr::null(),
                1,
                1,
                1,
                1,
                0,
                1,
                1,
                1.0,
                0.0,
                1,
                0.25,
                2,
                32,
                245,
                1,
                1,
                0,
                1,
                2,
                3,
                &mut status,
            )
        };

        assert!(result.data.is_null());
        assert_eq!(status.code, LIVE_ANALYSIS_STATUS_ERROR);
        assert!(!status.error.data.is_null());
        // SAFETY: this is the unmodified buffer returned in status and it is freed exactly once.
        unsafe { a2d_live_analysis_buffer_free(status.error) };
    }

    #[test]
    fn panic_is_contained_and_encoded_as_a_critical_error() {
        let execution = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<u8>, A2dError> {
            panic!("test panic")
        }));
        let error = match execution {
            Err(payload) => A2dError::internal_unknown(format!(
                "live grayscale analysis panicked: {}",
                panic_message(payload.as_ref())
            )),
            Ok(_) => panic!("test closure must panic"),
        };
        let encoded = encode_error(&error);

        assert_eq!(&encoded[..4], &ERROR_MAGIC);
        assert_eq!(u32::from_be_bytes(encoded[4..8].try_into().unwrap()), 1);
        assert!(String::from_utf8_lossy(&encoded).contains("INTERNAL_UNKNOWN"));
    }

    #[test]
    fn invalid_boolean_configuration_never_falls_back() {
        let image = canonical_gray_image();
        let mut invalid = config();
        invalid.detector_refine_edges = 2;
        let error = analyze_gray_frame_impl(
            image.width(),
            image.height(),
            image.row_stride() as u64,
            0,
            image.bytes(),
            invalid,
        )
        .unwrap_err();

        assert_eq!(error.code.to_string(), "IMAGE_FFI_PARAMETER_OUT_OF_RANGE");
    }
}
