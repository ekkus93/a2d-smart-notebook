use std::mem;
use std::ptr::NonNull;
use std::slice;

use a2d_domain::A2dError;
use apriltag_sys as sys;

use crate::detection::{ImagePoint, MarkerDetection, MarkerFamily};
use crate::error::{processing_error, validation_error};
use crate::input::GrayFrame;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetectorConfig {
    pub thread_count: u8,
    pub quad_decimate: f32,
    pub quad_sigma: f32,
    pub refine_edges: bool,
    pub decode_sharpening: f64,
    pub bits_corrected: u8,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            thread_count: 1,
            quad_decimate: 1.0,
            quad_sigma: 0.0,
            refine_edges: true,
            decode_sharpening: 0.25,
            bits_corrected: 2,
        }
    }
}

trait NativeBoolean {
    fn from_bool(value: bool) -> Self;
}

impl NativeBoolean for bool {
    fn from_bool(value: bool) -> Self {
        value
    }
}

impl NativeBoolean for i32 {
    fn from_bool(value: bool) -> Self {
        i32::from(value)
    }
}

impl DetectorConfig {
    fn validate(self) -> Result<Self, A2dError> {
        if self.thread_count == 0 {
            return Err(validation_error(
                "MARKER_CONFIG_THREAD_COUNT_INVALID",
                "AprilTag detector thread count must be greater than zero",
            ));
        }
        if !self.quad_decimate.is_finite() || self.quad_decimate < 1.0 {
            return Err(validation_error(
                "MARKER_CONFIG_DECIMATION_INVALID",
                format!(
                    "quad decimation must be finite and at least 1.0, got {}",
                    self.quad_decimate
                ),
            ));
        }
        if !self.quad_sigma.is_finite() || self.quad_sigma < 0.0 {
            return Err(validation_error(
                "MARKER_CONFIG_SIGMA_INVALID",
                format!(
                    "quad sigma must be finite and non-negative, got {}",
                    self.quad_sigma
                ),
            ));
        }
        if !self.decode_sharpening.is_finite() || self.decode_sharpening < 0.0 {
            return Err(validation_error(
                "MARKER_CONFIG_SHARPENING_INVALID",
                format!(
                    "decode sharpening must be finite and non-negative, got {}",
                    self.decode_sharpening
                ),
            ));
        }
        if self.bits_corrected > 2 {
            return Err(validation_error(
                "MARKER_CONFIG_BITS_CORRECTED_INVALID",
                format!(
                    "bits_corrected must be in 0..=2 to keep the native decode table bounded, got {}",
                    self.bits_corrected
                ),
            ));
        }
        Ok(self)
    }
}

/// Safe owner for the official AprilTag 3 detector and tagStandard41h12
/// family. Native pointers never cross this module's public API.
pub struct AprilTagDetector {
    detector: NonNull<sys::apriltag_detector_t>,
    family: NonNull<sys::apriltag_family_t>,
}

impl std::fmt::Debug for AprilTagDetector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AprilTagDetector")
            .field("family", &MarkerFamily::TagStandard41h12)
            .finish_non_exhaustive()
    }
}

impl AprilTagDetector {
    pub fn new(config: DetectorConfig) -> Result<Self, A2dError> {
        let config = config.validate()?;

        // SAFETY: constructors take no borrowed pointers. Null is checked before use.
        let family = NonNull::new(unsafe { sys::tagStandard41h12_create() }).ok_or_else(|| {
            processing_error(
                "MARKER_FAMILY_CREATE_FAILED",
                "tagStandard41h12_create returned null",
                false,
            )
        })?;

        // SAFETY: constructor takes no borrowed pointers. Null is checked before use.
        let detector = match NonNull::new(unsafe { sys::apriltag_detector_create() }) {
            Some(detector) => detector,
            None => {
                // SAFETY: family came from tagStandard41h12_create and is still uniquely owned.
                unsafe { sys::tagStandard41h12_destroy(family.as_ptr()) };
                return Err(processing_error(
                    "MARKER_DETECTOR_CREATE_FAILED",
                    "apriltag_detector_create returned null",
                    false,
                ));
            }
        };

        // SAFETY: both pointers are live and uniquely owned here. The official
        // detector stores but does not own the family pointer.
        unsafe {
            sys::apriltag_detector_add_family_bits(
                detector.as_ptr(),
                family.as_ptr(),
                i32::from(config.bits_corrected),
            );
            let native = &mut *detector.as_ptr();
            native.nthreads = i32::from(config.thread_count);
            native.quad_decimate = config.quad_decimate;
            native.quad_sigma = config.quad_sigma;
            native.refine_edges = NativeBoolean::from_bool(config.refine_edges);
            native.decode_sharpening = config.decode_sharpening;
            native.debug = NativeBoolean::from_bool(false);
        }

        Ok(Self { detector, family })
    }

    pub fn detect(&mut self, frame: GrayFrame<'_>) -> Result<Vec<MarkerDetection>, A2dError> {
        let mut image = NativeGrayImage::copy_from(frame)?;

        // SAFETY: detector and image are live for the duration of the call.
        // The result is immediately wrapped in a guard and copied into owned
        // Rust values before native memory is destroyed.
        let detections_ptr =
            unsafe { sys::apriltag_detector_detect(self.detector.as_ptr(), image.as_mut_ptr()) };
        let detections = NativeDetections::new(detections_ptr)?;

        // SAFETY: the guard owns the native array for the remainder of this scope.
        let raw_array = unsafe { detections.as_ref() };
        if raw_array.el_sz != mem::size_of::<*mut sys::apriltag_detection_t>() {
            return Err(processing_error(
                "MARKER_DETECTION_ARRAY_LAYOUT_INVALID",
                format!(
                    "native detection array element size is {}, expected {}",
                    raw_array.el_sz,
                    mem::size_of::<*mut sys::apriltag_detection_t>()
                ),
                false,
            ));
        }
        if raw_array.size < 0 || raw_array.size > raw_array.alloc {
            return Err(processing_error(
                "MARKER_DETECTION_ARRAY_SIZE_INVALID",
                format!(
                    "native detection array has size {} and allocation {}",
                    raw_array.size, raw_array.alloc
                ),
                false,
            ));
        }
        if raw_array.size > 0 && raw_array.data.is_null() {
            return Err(processing_error(
                "MARKER_DETECTION_ARRAY_DATA_NULL",
                "native detection array contains elements but has a null data pointer",
                false,
            ));
        }

        let raw_detections = if raw_array.size == 0 {
            &[][..]
        } else {
            // SAFETY: zarray reports pointer-sized elements, a valid non-null
            // data pointer, and a non-negative size no larger than allocation.
            unsafe {
                slice::from_raw_parts(
                    raw_array.data.cast::<*mut sys::apriltag_detection_t>(),
                    raw_array.size as usize,
                )
            }
        };

        raw_detections
            .iter()
            .map(|raw| copy_detection(*raw, self.family))
            .collect()
    }

    /// Renders one official `tagStandard41h12` marker into Rust-owned grayscale pixels.
    ///
    /// The native image is copied before returning, so callers never receive a C pointer or a
    /// buffer tied to the detector lifetime. This is used by both printable PDF generation and
    /// deterministic fixture generation.
    pub fn render_tag(&self, id: u32) -> Result<RenderedTag, A2dError> {
        // SAFETY: self.family is live. Reading ncodes is immutable.
        let ncodes = unsafe { self.family.as_ref().ncodes };
        if id >= ncodes {
            return Err(validation_error(
                "MARKER_TAG_ID_OUT_OF_RANGE",
                format!("tag ID {id} is outside tagStandard41h12's {ncodes} codes"),
            ));
        }
        // SAFETY: id is in range and family remains live while the returned
        // image is copied into Rust-owned memory.
        let image = unsafe { sys::apriltag_to_image(self.family.as_ptr(), id) };
        let image = NonNull::new(image).ok_or_else(|| {
            processing_error(
                "MARKER_RENDER_FAILED",
                format!("apriltag_to_image returned null for tag ID {id}"),
                false,
            )
        })?;

        // SAFETY: image is live and owned by this function.
        let native = unsafe { image.as_ref() };
        if native.width <= 0
            || native.height <= 0
            || native.stride < native.width
            || native.buf.is_null()
        {
            // SAFETY: image came from apriltag_to_image.
            unsafe { sys::image_u8_destroy(image.as_ptr()) };
            return Err(processing_error(
                "MARKER_RENDER_INVALID",
                "apriltag_to_image returned an invalid image",
                false,
            ));
        }

        let len = (native.stride as usize)
            .checked_mul(native.height as usize)
            .ok_or_else(|| {
                processing_error(
                    "MARKER_RENDER_SIZE_OVERFLOW",
                    "rendered tag buffer size overflowed",
                    false,
                )
            })?;
        // SAFETY: native image owns at least stride * height bytes.
        let bytes = unsafe { slice::from_raw_parts(native.buf, len) }.to_vec();
        let rendered = RenderedTag {
            width: native.width as usize,
            height: native.height as usize,
            stride: native.stride as usize,
            bytes,
        };
        // SAFETY: image came from apriltag_to_image and has not been freed.
        unsafe { sys::image_u8_destroy(image.as_ptr()) };
        Ok(rendered)
    }
}

impl Drop for AprilTagDetector {
    fn drop(&mut self) {
        // SAFETY: both pointers are live and owned by self. The detector does
        // not deallocate families, so unregister/destroy detector first and
        // then destroy the family exactly once.
        unsafe {
            sys::apriltag_detector_clear_families(self.detector.as_ptr());
            sys::apriltag_detector_destroy(self.detector.as_ptr());
            sys::tagStandard41h12_destroy(self.family.as_ptr());
        }
    }
}

struct NativeGrayImage(NonNull<sys::image_u8_t>);

impl NativeGrayImage {
    fn copy_from(frame: GrayFrame<'_>) -> Result<Self, A2dError> {
        // SAFETY: validated dimensions and stride fit the C integer range.
        let image = unsafe {
            sys::image_u8_create_stride(frame.width(), frame.height(), frame.row_stride() as u32)
        };
        let image = NonNull::new(image).ok_or_else(|| {
            processing_error(
                "IMAGE_NATIVE_ALLOCATION_FAILED",
                format!(
                    "failed to allocate {}x{} native grayscale image",
                    frame.width(),
                    frame.height()
                ),
                true,
            )
        })?;

        // SAFETY: image_u8_create_stride returns a writable buffer of at
        // least stride * height bytes. The source has already been validated
        // to contain that many bytes.
        unsafe {
            let native = image.as_ref();
            if native.buf.is_null() {
                sys::image_u8_destroy(image.as_ptr());
                return Err(processing_error(
                    "IMAGE_NATIVE_BUFFER_NULL",
                    "native grayscale image allocation returned a null buffer",
                    false,
                ));
            }
            std::ptr::copy_nonoverlapping(
                frame.bytes().as_ptr(),
                native.buf,
                frame.required_bytes(),
            );
        }

        Ok(Self(image))
    }

    fn as_mut_ptr(&mut self) -> *mut sys::image_u8_t {
        self.0.as_ptr()
    }
}

impl Drop for NativeGrayImage {
    fn drop(&mut self) {
        // SAFETY: pointer came from image_u8_create_stride and is owned by self.
        unsafe { sys::image_u8_destroy(self.0.as_ptr()) };
    }
}

struct NativeDetections(NonNull<sys::zarray_t>);

impl NativeDetections {
    fn new(raw: *mut sys::zarray_t) -> Result<Self, A2dError> {
        NonNull::new(raw).map(Self).ok_or_else(|| {
            processing_error(
                "MARKER_DETECTION_FAILED",
                "apriltag_detector_detect returned null",
                true,
            )
        })
    }

    unsafe fn as_ref(&self) -> &sys::zarray_t {
        // SAFETY: callers keep the NativeDetections guard alive.
        unsafe { self.0.as_ref() }
    }
}

impl Drop for NativeDetections {
    fn drop(&mut self) {
        // SAFETY: pointer came from apriltag_detector_detect and is owned by self.
        unsafe { sys::apriltag_detections_destroy(self.0.as_ptr()) };
    }
}

fn copy_detection(
    raw: *mut sys::apriltag_detection_t,
    expected_family: NonNull<sys::apriltag_family_t>,
) -> Result<MarkerDetection, A2dError> {
    let raw = NonNull::new(raw).ok_or_else(|| {
        processing_error(
            "MARKER_DETECTION_NULL",
            "native detection array contained a null detection pointer",
            false,
        )
    })?;
    // SAFETY: the pointer is owned by the live NativeDetections guard.
    let native = unsafe { raw.as_ref() };

    if native.family != expected_family.as_ptr() {
        return Err(processing_error(
            "MARKER_FAMILY_UNEXPECTED",
            "native detector returned a detection from an unregistered family",
            false,
        ));
    }
    let id = u32::try_from(native.id).map_err(|_| {
        processing_error(
            "MARKER_DETECTION_ID_INVALID",
            format!("native detector returned negative tag ID {}", native.id),
            false,
        )
    })?;
    let hamming_errors = u8::try_from(native.hamming).map_err(|_| {
        processing_error(
            "MARKER_DETECTION_HAMMING_INVALID",
            format!(
                "native detector returned out-of-range hamming distance {}",
                native.hamming
            ),
            false,
        )
    })?;
    if !native.decision_margin.is_finite() {
        return Err(processing_error(
            "MARKER_DETECTION_MARGIN_INVALID",
            "native detector returned a non-finite decision margin",
            false,
        ));
    }

    let center = ImagePoint::from_array(native.c)?;
    let corners = [
        ImagePoint::from_array(native.p[0])?,
        ImagePoint::from_array(native.p[1])?,
        ImagePoint::from_array(native.p[2])?,
        ImagePoint::from_array(native.p[3])?,
    ];

    Ok(MarkerDetection {
        family: MarkerFamily::TagStandard41h12,
        id,
        hamming_errors,
        decision_margin: native.decision_margin,
        center,
        corners,
    })
}

/// Owned grayscale marker image returned by [`AprilTagDetector::render_tag`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedTag {
    width: usize,
    height: usize,
    stride: usize,
    bytes: Vec<u8>,
}

impl RenderedTag {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn row_stride(&self) -> usize {
        self.stride
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.bytes
            .get(y.checked_mul(self.stride)?.checked_add(x)?)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::{MarkerIdLayout, PageOrientation, resolve_page_markers};
    use crate::input::{ImageLimits, ImageRotation};
    use a2d_layout::MarkerRole;
    use std::collections::BTreeSet;
    use std::time::Instant;

    const WIDTH: usize = 640;
    const HEIGHT: usize = 480;
    const SCALE: usize = 12;

    fn paste_scaled(canvas: &mut [u8], tag: &RenderedTag, left: usize, top: usize) {
        for source_y in 0..tag.height {
            for source_x in 0..tag.width {
                let value = tag.bytes[source_y * tag.stride + source_x];
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        let x = left + source_x * SCALE + dx;
                        let y = top + source_y * SCALE + dy;
                        canvas[y * WIDTH + x] = value;
                    }
                }
            }
        }
    }

    fn four_tag_frame(detector: &AprilTagDetector) -> (Vec<u8>, MarkerIdLayout) {
        let mut canvas = vec![255_u8; WIDTH * HEIGHT];
        let placements = [
            (0, MarkerRole::TopLeft, 24, 24),
            (1, MarkerRole::TopRight, 500, 24),
            (2, MarkerRole::BottomRight, 500, 340),
            (3, MarkerRole::BottomLeft, 24, 340),
        ];

        for (id, _, x, y) in placements {
            let tag = detector.render_tag(id).unwrap();
            assert!(x + tag.width * SCALE <= WIDTH);
            assert!(y + tag.height * SCALE <= HEIGHT);
            paste_scaled(&mut canvas, &tag, x, y);
        }

        let layout = MarkerIdLayout::new([
            (0, MarkerRole::TopLeft),
            (1, MarkerRole::TopRight),
            (2, MarkerRole::BottomRight),
            (3, MarkerRole::BottomLeft),
        ])
        .unwrap();
        (canvas, layout)
    }

    #[test]
    fn official_detector_finds_and_resolves_four_generated_tags() {
        let mut detector = AprilTagDetector::new(DetectorConfig::default()).unwrap();
        let (canvas, layout) = four_tag_frame(&detector);
        let frame = GrayFrame::new(
            WIDTH as u32,
            HEIGHT as u32,
            WIDTH,
            ImageRotation::Degrees0,
            &canvas,
            ImageLimits::new((WIDTH * HEIGHT) as u64).unwrap(),
        )
        .unwrap();

        let started = Instant::now();
        let detections = detector.detect(frame).unwrap();
        let elapsed = started.elapsed();

        let ids: BTreeSet<_> = detections.iter().map(|d| d.id).collect();
        assert_eq!(ids, BTreeSet::from([0, 1, 2, 3]));
        assert!(detections.iter().all(|d| d.hamming_errors == 0));
        assert!(
            detections
                .iter()
                .all(|d| d.decision_margin.is_finite() && d.decision_margin > 0.0)
        );

        let resolved = resolve_page_markers(&detections, &layout).unwrap();
        assert_eq!(resolved.orientation, PageOrientation::Degrees0);
        assert!(resolved.unexpected_tag_ids.is_empty());

        eprintln!(
            "AprilTag spike: detected {} tagStandard41h12 markers in {:?}",
            detections.len(),
            elapsed
        );
    }

    #[test]
    fn native_boolean_mapping_is_portable_across_generated_binding_types() {
        assert!(<bool as NativeBoolean>::from_bool(true));
        assert!(!<bool as NativeBoolean>::from_bool(false));
        assert_eq!(<i32 as NativeBoolean>::from_bool(true), 1);
        assert_eq!(<i32 as NativeBoolean>::from_bool(false), 0);
    }

    #[test]
    fn rejects_invalid_native_detector_configuration_before_allocation() {
        let config = DetectorConfig {
            thread_count: 0,
            ..DetectorConfig::default()
        };
        let err = AprilTagDetector::new(config).unwrap_err();
        assert_eq!(err.code.to_string(), "MARKER_CONFIG_THREAD_COUNT_INVALID");

        let config = DetectorConfig {
            bits_corrected: 3,
            ..DetectorConfig::default()
        };
        let err = AprilTagDetector::new(config).unwrap_err();
        assert_eq!(err.code.to_string(), "MARKER_CONFIG_BITS_CORRECTED_INVALID");
    }
}
