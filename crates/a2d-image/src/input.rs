use a2d_domain::A2dError;

use crate::error::validation_error;

/// The only pixel format accepted by the initial shared detector boundary.
/// Camera adapters must extract the luminance plane without Base64 or JSON.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Gray8,
}

/// Clockwise rotation required to display the supplied pixels upright.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageRotation {
    Degrees0,
    Degrees90,
    Degrees180,
    Degrees270,
}

impl ImageRotation {
    pub const fn degrees(self) -> u16 {
        match self {
            Self::Degrees0 => 0,
            Self::Degrees90 => 90,
            Self::Degrees180 => 180,
            Self::Degrees270 => 270,
        }
    }
}

/// Caller-selected memory safety limit. The shared core does not invent a
/// quality threshold; the platform/use case must provide its allowed size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageLimits {
    max_pixels: u64,
}

impl ImageLimits {
    pub fn new(max_pixels: u64) -> Result<Self, A2dError> {
        if max_pixels == 0 {
            return Err(validation_error(
                "IMAGE_PIXEL_LIMIT_INVALID",
                "maximum decoded pixel count must be greater than zero",
            ));
        }
        Ok(Self { max_pixels })
    }

    pub const fn max_pixels(self) -> u64 {
        self.max_pixels
    }
}

/// Borrowed, validated grayscale image input. `bytes` must contain complete
/// rows including any stride padding; trailing bytes are permitted and ignored.
#[derive(Clone, Copy, Debug)]
pub struct GrayFrame<'a> {
    width: u32,
    height: u32,
    row_stride: usize,
    pixel_format: PixelFormat,
    rotation: ImageRotation,
    bytes: &'a [u8],
}

impl<'a> GrayFrame<'a> {
    pub fn new(
        width: u32,
        height: u32,
        row_stride: usize,
        rotation: ImageRotation,
        bytes: &'a [u8],
        limits: ImageLimits,
    ) -> Result<Self, A2dError> {
        if width == 0 || height == 0 {
            return Err(validation_error(
                "IMAGE_DIMENSIONS_INVALID",
                format!("image dimensions must be non-zero, got {width}x{height}"),
            ));
        }
        if width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(validation_error(
                "IMAGE_DIMENSIONS_UNSUPPORTED",
                format!("image dimensions exceed native detector limits: {width}x{height}"),
            ));
        }

        let width_usize = width as usize;
        if row_stride < width_usize || row_stride > i32::MAX as usize {
            return Err(validation_error(
                "IMAGE_STRIDE_INVALID",
                format!(
                    "row stride must be between width ({width_usize}) and {}, got {row_stride}",
                    i32::MAX
                ),
            ));
        }

        let pixel_count = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                validation_error(
                    "IMAGE_PIXEL_COUNT_OVERFLOW",
                    format!("pixel count overflow for {width}x{height}"),
                )
            })?;
        if pixel_count > limits.max_pixels() {
            return Err(validation_error(
                "IMAGE_PIXEL_LIMIT_EXCEEDED",
                format!(
                    "decoded image has {pixel_count} pixels, limit is {}",
                    limits.max_pixels()
                ),
            )
            .with_detail("width", width.to_string())
            .with_detail("height", height.to_string())
            .with_detail("max_pixels", limits.max_pixels().to_string()));
        }

        let required_bytes = row_stride.checked_mul(height as usize).ok_or_else(|| {
            validation_error(
                "IMAGE_BUFFER_SIZE_OVERFLOW",
                format!("buffer size overflow for stride {row_stride} and height {height}"),
            )
        })?;
        if bytes.len() < required_bytes {
            return Err(validation_error(
                "IMAGE_BUFFER_TOO_SMALL",
                format!(
                    "grayscale buffer has {} bytes but {required_bytes} are required",
                    bytes.len()
                ),
            )
            .with_detail("required_bytes", required_bytes.to_string())
            .with_detail("actual_bytes", bytes.len().to_string()));
        }

        Ok(Self {
            width,
            height,
            row_stride,
            pixel_format: PixelFormat::Gray8,
            rotation,
            bytes,
        })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub const fn row_stride(self) -> usize {
        self.row_stride
    }

    pub const fn pixel_format(self) -> PixelFormat {
        self.pixel_format
    }

    pub const fn rotation(self) -> ImageRotation {
        self.rotation
    }

    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub fn required_bytes(self) -> usize {
        self.row_stride * self.height as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> ImageLimits {
        ImageLimits::new(1_000_000).unwrap()
    }

    #[test]
    fn accepts_padded_grayscale_rows_without_copying_at_validation_time() {
        let bytes = [0_u8; 24];
        let frame = GrayFrame::new(
            5,
            3,
            8,
            ImageRotation::Degrees90,
            &bytes,
            limits(),
        )
        .unwrap();

        assert_eq!(frame.width(), 5);
        assert_eq!(frame.height(), 3);
        assert_eq!(frame.row_stride(), 8);
        assert_eq!(frame.required_bytes(), 24);
        assert_eq!(frame.rotation().degrees(), 90);
        assert_eq!(frame.pixel_format(), PixelFormat::Gray8);
    }

    #[test]
    fn rejects_zero_dimensions() {
        let err = GrayFrame::new(
            0,
            10,
            10,
            ImageRotation::Degrees0,
            &[0; 100],
            limits(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "IMAGE_DIMENSIONS_INVALID");
    }

    #[test]
    fn rejects_stride_smaller_than_width() {
        let err = GrayFrame::new(
            10,
            10,
            9,
            ImageRotation::Degrees0,
            &[0; 100],
            limits(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "IMAGE_STRIDE_INVALID");
    }

    #[test]
    fn rejects_truncated_rows() {
        let err = GrayFrame::new(
            10,
            10,
            12,
            ImageRotation::Degrees0,
            &[0; 119],
            limits(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "IMAGE_BUFFER_TOO_SMALL");
    }

    #[test]
    fn rejects_frames_above_the_explicit_pixel_limit() {
        let err = GrayFrame::new(
            11,
            10,
            11,
            ImageRotation::Degrees0,
            &[0; 110],
            ImageLimits::new(100).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "IMAGE_PIXEL_LIMIT_EXCEEDED");
    }

    #[test]
    fn rejects_a_zero_pixel_limit() {
        let err = ImageLimits::new(0).unwrap_err();
        assert_eq!(err.code.to_string(), "IMAGE_PIXEL_LIMIT_INVALID");
    }
}
