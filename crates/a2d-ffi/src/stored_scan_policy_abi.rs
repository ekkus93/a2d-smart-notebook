//! Explicit C ABI for resolving one stored page's Rust-owned scan policy.
//!
//! This boundary deliberately avoids changing generated UniFFI bindings. Android passes the
//! already-open client's canonical library path plus a resolved Page ID. Rust reopens the same
//! local library, resolves canonical stored page/design state, and returns one bounded binary
//! policy record. The returned record is advisory input for Android presentation and live analysis;
//! full-resolution preview and durable registration still resolve policy again in Rust.

use std::any::Any;
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;

use a2d_core::{A2dCore, OpenLibraryRequest, StoredScanProcessingPolicy};
use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity, PageId};
use a2d_layout::MarkerRole;

use crate::{
    A2dPreviewBuffer, A2dPreviewStatus, PREVIEW_STATUS_ERROR, PREVIEW_STATUS_PANIC,
    PREVIEW_STATUS_SUCCESS,
};

const POLICY_CODEC_VERSION: u32 = 1;
const POLICY_MAGIC: [u8; 4] = *b"A2DS";
const ERROR_MAGIC: [u8; 4] = *b"A2PE";

fn request_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.ffi.stored_scan_policy_request",
        message.into(),
        false,
    )
}

fn codec_error(message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new("STORED_SCAN_POLICY_CODEC_ERROR"),
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
            "STORED_SCAN_POLICY_LENGTH_UNSUPPORTED",
            format!("{field} value {value} does not fit this platform"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string())
    })
}

fn parse_utf8(bytes: *const u8, len: u64, field: &'static str) -> Result<String, A2dError> {
    let byte_count = to_usize(len, field)?;
    if byte_count == 0 {
        return Err(request_error(
            "STORED_SCAN_POLICY_FIELD_EMPTY",
            format!("{field} must not be empty"),
        )
        .with_detail("field", field));
    }
    if bytes.is_null() {
        return Err(request_error(
            "STORED_SCAN_POLICY_FIELD_NULL",
            format!("{field} pointer must not be null"),
        )
        .with_detail("field", field));
    }
    // SAFETY: the exported function requires the caller to keep this byte range readable until
    // return. Copying the validated UTF-8 keeps no borrowed pointer beyond this operation.
    let raw = unsafe { slice::from_raw_parts(bytes, byte_count) };
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|error| {
            request_error(
                "STORED_SCAN_POLICY_UTF8_INVALID",
                format!("{field} is not valid UTF-8: {error}"),
            )
            .with_detail("field", field)
        })
}

fn marker_id(policy: &StoredScanProcessingPolicy, role: MarkerRole) -> Result<u32, A2dError> {
    policy
        .marker_roles
        .iter()
        .find(|marker| marker.role == role)
        .map(|marker| marker.marker_id)
        .ok_or_else(|| {
            A2dError::new(
                ErrorCode::new("FFI_SCAN_POLICY_MARKER_ROLE_MISSING"),
                ErrorCategory::Integrity,
                ErrorSeverity::Critical,
                "error.ffi.scan_policy_marker_role_missing",
                "the resolved Rust scan policy omitted a required semantic marker role",
                false,
            )
            .with_detail("layout_id", policy.layout_id.to_string())
            .with_detail("marker_role", role.as_id_str())
        })
}

struct BinaryWriter {
    bytes: Vec<u8>,
}

impl BinaryWriter {
    fn new(magic: [u8; 4], capacity: usize) -> Self {
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(&magic);
        bytes.extend_from_slice(&POLICY_CODEC_VERSION.to_be_bytes());
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
                .map_err(|_| codec_error(format!("{field} length exceeds the policy codec")))?,
        );
        Ok(())
    }

    fn string(&mut self, value: &str, field: &'static str) -> Result<(), A2dError> {
        self.length(value.len(), field)?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn optional_string(
        &mut self,
        value: Option<&str>,
        field: &'static str,
    ) -> Result<(), A2dError> {
        match value {
            Some(value) => {
                self.u8(1);
                self.string(value, field)?;
            }
            None => self.u8(0),
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn encode_policy(policy: &StoredScanProcessingPolicy) -> Result<Vec<u8>, A2dError> {
    let live = policy.live_analysis_values();
    let mut writer = BinaryWriter::new(POLICY_MAGIC, 512);
    writer.string(&policy.layout_id.to_string(), "layout_id")?;
    writer.f64(policy.physical_width_mm);
    writer.f64(policy.physical_height_mm);
    writer.string(&policy.marker_family, "marker_family")?;
    writer.optional_string(
        policy.declared_marker_family.as_deref(),
        "declared_marker_family",
    )?;
    writer.u32(marker_id(policy, MarkerRole::TopLeft)?);
    writer.u32(marker_id(policy, MarkerRole::TopRight)?);
    writer.u32(marker_id(policy, MarkerRole::BottomRight)?);
    writer.u32(marker_id(policy, MarkerRole::BottomLeft)?);
    writer.u32(policy.corrected_width);
    writer.u32(policy.corrected_height);
    writer.string(&policy.layout_version, "layout_version")?;
    writer.u32(policy.policy_version);
    writer.u32(policy.pipeline_version());
    writer.u64(live.maximum_encoded_bytes);
    writer.u64(live.maximum_decoded_pixels);
    writer.u64(live.maximum_decoded_bytes);
    writer.u32(live.detector_thread_count);
    writer.f64(live.detector_quad_decimate);
    writer.f64(live.detector_quad_sigma);
    writer.u8(u8::from(live.detector_refine_edges));
    writer.f64(live.detector_decode_sharpening);
    writer.u32(live.detector_bits_corrected);
    writer.u32(live.dark_luminance_cutoff);
    writer.u32(live.highlight_luminance_cutoff);
    writer.u32(live.quality_tile_columns);
    writer.u32(live.quality_tile_rows);
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
        "STORED_SCAN_POLICY_ERROR_ENCODING_FAILED",
        "Internal",
        "Critical",
        "error.internal_unknown",
        "stored scan policy error fields exceeded the codec limit",
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

/// Resolve one stored page's canonical layout and portable live-analysis policy.
///
/// # Safety
///
/// - `status` must point to writable [`A2dPreviewStatus`] storage.
/// - non-empty library-path and Page-ID byte ranges must remain readable and unmodified until
///   return.
/// - the returned buffer must be freed with `a2d_preview_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn a2d_resolve_stored_scan_policy_v1(
    library_path_bytes: *const u8,
    library_path_len: u64,
    page_id_bytes: *const u8,
    page_id_len: u64,
    status: *mut A2dPreviewStatus,
) -> A2dPreviewBuffer {
    if status.is_null() {
        return A2dPreviewBuffer::default();
    }
    // SAFETY: status is non-null and writable by contract.
    unsafe { set_status(status, PREVIEW_STATUS_SUCCESS, A2dPreviewBuffer::default()) };

    let execution = catch_unwind(AssertUnwindSafe(|| {
        let library_path = parse_utf8(library_path_bytes, library_path_len, "library_path")?;
        let page_id = PageId::parse(&parse_utf8(page_id_bytes, page_id_len, "page_id")?)?;
        let core = A2dCore::open(OpenLibraryRequest { library_path })?;
        let policy = core.resolve_stored_scan_processing_policy(&page_id)?;
        encode_policy(&policy)
    }));

    match execution {
        Ok(Ok(encoded)) => into_buffer(encoded),
        Ok(Err(error)) => {
            let error_buffer = into_buffer(encode_error(&error));
            // SAFETY: status remains writable for the duration of the call.
            unsafe { set_status(status, PREVIEW_STATUS_ERROR, error_buffer) };
            A2dPreviewBuffer::default()
        }
        Err(payload) => {
            let error = A2dError::internal_unknown(format!(
                "stored scan policy resolution panicked: {}",
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
    use a2d_domain::PageId;

    use super::*;
    use crate::{A2dClient, SmartPageContentStyle, SmartPageGenerationRequest, SmartPagePaperSize};

    #[test]
    fn policy_codec_starts_with_stable_magic_and_version() {
        let layout = a2d_layout::smart_page_layout(
            a2d_layout::PaperSize::A4,
            a2d_layout::SmartPageStyle::Blank,
        );
        let policy = a2d_core::resolve_bundled_scan_processing_policy(&layout.id, 1).unwrap();
        let encoded = encode_policy(&policy).unwrap();
        assert_eq!(&encoded[..4], b"A2DS");
        assert_eq!(u32::from_be_bytes(encoded[4..8].try_into().unwrap()), 1);
    }

    #[test]
    fn stored_page_resolution_reports_the_same_policy_identity() {
        let root =
            std::env::temp_dir().join(format!("a2d-stored-policy-abi-{}", PageId::generate()));
        let client = A2dClient::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let generated = client
            .generate_smart_pages(SmartPageGenerationRequest {
                paper_size: SmartPagePaperSize::A4,
                style: SmartPageContentStyle::Blank,
                page_count: 1,
                starting_visible_page: 1,
            })
            .unwrap();
        let page_id = PageId::parse(&generated.page_ids[0]).unwrap();
        let policy = client
            .core
            .resolve_stored_scan_processing_policy(&page_id)
            .unwrap();
        assert_eq!(policy.layout_id.to_string(), "SP-A4-BLANK-V1");
        assert_eq!(policy.policy_version, 1);
        assert_eq!(policy.pipeline_version(), 1);

        drop(client);
        std::fs::remove_dir_all(root).ok();
    }
}
