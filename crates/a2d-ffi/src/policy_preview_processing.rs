//! Policy-bound full-resolution scanner preview ABI.
//!
//! Unlike the legacy scalar ABI, this entry point accepts only a Rust-issued layout identifier and
//! processing-policy version. Marker IDs, rectification geometry, detector settings, enhancement
//! parameters, pipeline identity, and resource limits are resolved in Rust.

use std::any::Any;
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId};
use a2d_image::{
    AprilTagDetector, DerivedImagePipeline, EncodedImage, EncodedImageFormat, ImageRotation,
    ProcessingCancellation, RectificationPlan, measure_gray_quality, resolve_page_markers,
};

use crate::{
    A2dPreviewBuffer, A2dPreviewStatus, AnalyzeEncodedPageResult, AnalyzedImagePoint,
    AnalyzedMarker, GrayQualityMeasurements, PREVIEW_STATUS_CANCELLED, PREVIEW_STATUS_ERROR,
    PREVIEW_STATUS_PANIC, PREVIEW_STATUS_SUCCESS,
};

const PREVIEW_CODEC_VERSION: u32 = 1;
const RESULT_MAGIC: [u8; 4] = *b"A2DP";
const ERROR_MAGIC: [u8; 4] = *b"A2PE";

pub struct A2dPolicyPreviewCancellation {
    inner: ProcessingCancellation,
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

#[derive(Debug)]
enum PreviewOutcome {
    Completed(Box<PreviewResult>),
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
    format_code: u32,
    rotation_degrees: u32,
    layout_id: &LayoutId,
    processing_policy_version: u32,
    cancellation: &ProcessingCancellation,
) -> Result<PreviewOutcome, A2dError> {
    if cancellation.is_cancelled() {
        return Ok(PreviewOutcome::Cancelled);
    }
    let policy =
        a2d_core::resolve_bundled_scan_processing_policy(layout_id, processing_policy_version)?;
    if encoded_bytes.len() > policy.maximum_encoded_bytes() {
        return Err(request_error(
            "IMAGE_ENCODED_BYTES_LIMIT_EXCEEDED",
            "encoded capture exceeds the Rust processing-policy byte limit",
        )
        .with_detail("layout_id", layout_id.to_string())
        .with_detail("actual_bytes", encoded_bytes.len().to_string())
        .with_detail("maximum_bytes", policy.maximum_encoded_bytes().to_string()));
    }

    let image_limits = policy.image_limits()?;
    let source = EncodedImage::new(
        encoded_bytes,
        image_format(format_code)?,
        image_rotation(rotation_degrees)?,
        policy.encoded_image_limits()?,
    )?
    .decode_rgb8()?;
    let gray = source.clone().into_gray8(image_limits)?;
    let frame = gray.as_frame(image_limits)?;
    let quality = measure_gray_quality(frame, policy.quality_measurement_config()?)?;
    let mut detector = AprilTagDetector::new(policy.detector_config())?;
    let detections = detector.detect(frame)?;
    let marker_layout = policy.marker_id_layout()?;
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

    let rectification = RectificationPlan::from_page_markers(
        source.width(),
        source.height(),
        &resolved,
        &policy.page_layout,
        policy.rectified_image_size()?,
    )?;
    let derived = match DerivedImagePipeline::new(policy.derived_image_config()?).process(
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
    Ok(PreviewOutcome::Completed(Box::new(PreviewResult {
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
    })))
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
        if writer.string(value, "static error field").is_err() {
            return Vec::new();
        }
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

fn parse_layout_id(bytes: *const u8, len: u64) -> Result<LayoutId, A2dError> {
    let byte_count = to_usize(len, "layout_id_len")?;
    if byte_count == 0 {
        return Err(request_error(
            "PREVIEW_LAYOUT_ID_EMPTY",
            "layout ID must not be empty",
        ));
    }
    if bytes.is_null() {
        return Err(request_error(
            "PREVIEW_LAYOUT_ID_NULL",
            "layout ID pointer must not be null",
        ));
    }
    // SAFETY: required by the exported function's contract.
    let raw = unsafe { slice::from_raw_parts(bytes, byte_count) };
    let text = std::str::from_utf8(raw).map_err(|error| {
        request_error(
            "PREVIEW_LAYOUT_ID_UTF8_INVALID",
            format!("layout ID is not valid UTF-8: {error}"),
        )
    })?;
    LayoutId::parse(text)
}

#[unsafe(no_mangle)]
pub extern "C" fn a2d_policy_preview_cancellation_new() -> *mut A2dPolicyPreviewCancellation {
    Box::into_raw(Box::new(A2dPolicyPreviewCancellation {
        inner: ProcessingCancellation::active(),
    }))
}

/// # Safety
///
/// `cancellation` must be null or a live pointer returned by
/// [`a2d_policy_preview_cancellation_new`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_policy_preview_cancellation_cancel(
    cancellation: *const A2dPolicyPreviewCancellation,
) {
    if let Some(cancellation) = unsafe { cancellation.as_ref() } {
        cancellation.inner.cancel();
    }
}

/// # Safety
///
/// `cancellation` must be null or a live pointer returned by
/// [`a2d_policy_preview_cancellation_new`] that has not previously been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_policy_preview_cancellation_free(
    cancellation: *mut A2dPolicyPreviewCancellation,
) {
    if !cancellation.is_null() {
        // SAFETY: required by this function's contract.
        unsafe { drop(Box::from_raw(cancellation)) };
    }
}

/// Process one encoded capture using a Rust-resolved layout and versioned processing policy.
///
/// # Safety
///
/// - `status` must point to writable [`A2dPreviewStatus`] storage.
/// - `cancellation` must point to a live [`A2dPolicyPreviewCancellation`] for the full call.
/// - non-empty byte and layout-ID pointers must remain readable and unmodified until return.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn a2d_process_encoded_page_preview_v2(
    bytes: *const u8,
    bytes_len: u64,
    format_code: u32,
    rotation_degrees: u32,
    layout_id_bytes: *const u8,
    layout_id_len: u64,
    processing_policy_version: u32,
    cancellation: *const A2dPolicyPreviewCancellation,
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
        let layout_id = parse_layout_id(layout_id_bytes, layout_id_len)?;
        let cancellation = unsafe { cancellation.as_ref() }.ok_or_else(|| {
            request_error(
                "PREVIEW_CANCELLATION_NULL",
                "cancellation pointer must not be null",
            )
        })?;
        process_preview(
            encoded_bytes,
            format_code,
            rotation_degrees,
            &layout_id,
            processing_policy_version,
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
                "policy-bound preview processing panicked: {}",
                panic_message(payload.as_ref())
            ));
            let error_buffer = into_buffer(encode_error(&error));
            // SAFETY: status remains writable for the duration of the call.
            unsafe { set_status(status, PREVIEW_STATUS_PANIC, error_buffer) };
            A2dPreviewBuffer::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use a2d_layout::{PaperSize, SmartPageStyle, smart_page_layout};

    use super::*;

    #[test]
    fn cancellation_is_structurally_distinct_from_policy_failure() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let cancellation = ProcessingCancellation::active();
        cancellation.cancel();
        let outcome = process_preview(&[], 0, 0, &layout.id, 1, &cancellation).unwrap();
        assert!(matches!(outcome, PreviewOutcome::Cancelled));
    }

    #[test]
    fn unsupported_policy_version_is_rejected_before_decoding() {
        let layout = smart_page_layout(PaperSize::A4, SmartPageStyle::Blank);
        let cancellation = ProcessingCancellation::active();
        let error = process_preview(&[], 0, 0, &layout.id, 2, &cancellation).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "CORE_SCAN_PROCESSING_POLICY_VERSION_UNSUPPORTED"
        );
    }

    #[test]
    fn source_contract_has_no_kotlin_selected_processing_scalars() {
        let source = include_str!("policy_preview_processing.rs");
        let signature = source
            .split("pub unsafe extern \"C\" fn a2d_process_encoded_page_preview_v2")
            .nth(1)
            .unwrap()
            .split(") -> A2dPreviewBuffer")
            .next()
            .unwrap();
        for forbidden in [
            "top_left_tag_id",
            "corrected_width",
            "pipeline_version:",
            "derived_max_working_bytes",
            "contrast_maximum_gain",
        ] {
            assert!(
                !signature.contains(forbidden),
                "policy-bound ABI must not accept canonical scalar {forbidden}"
            );
        }
    }
}
