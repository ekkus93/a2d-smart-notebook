use std::{
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
};

use a2d_domain::A2dError;
use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbImage};

use crate::{
    error::{processing_error, validation_error},
    input::{GrayFrame, ImageLimits, ImageRotation, PixelFormat},
};

/// Encoded image formats accepted by the v0.1 shared full-resolution boundary.
/// The caller declares the format and the decoder verifies the matching file
/// signature rather than silently guessing or falling back to another format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodedImageFormat {
    Jpeg,
    Png,
}

impl EncodedImageFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
        }
    }

    const fn image_format(self) -> ImageFormat {
        match self {
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Png => ImageFormat::Png,
        }
    }

    fn has_matching_signature(self, bytes: &[u8]) -> bool {
        match self {
            Self::Jpeg => bytes.starts_with(&[0xff, 0xd8]),
            Self::Png => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        }
    }
}

/// Caller-selected resource limits for encoded full-resolution inputs.
///
/// No production threshold is embedded here. Camera capture, import, and other
/// call sites must select explicit limits appropriate to their operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodedImageLimits {
    max_encoded_bytes: usize,
    max_pixels: u64,
    max_decoded_bytes: u64,
}

impl EncodedImageLimits {
    pub fn new(
        max_encoded_bytes: usize,
        max_pixels: u64,
        max_decoded_bytes: u64,
    ) -> Result<Self, A2dError> {
        if max_encoded_bytes == 0 {
            return Err(validation_error(
                "IMAGE_ENCODED_BYTE_LIMIT_INVALID",
                "maximum encoded image bytes must be greater than zero",
            ));
        }
        if max_pixels == 0 {
            return Err(validation_error(
                "IMAGE_PIXEL_LIMIT_INVALID",
                "maximum decoded pixel count must be greater than zero",
            ));
        }
        if max_decoded_bytes == 0 {
            return Err(validation_error(
                "IMAGE_DECODED_BYTE_LIMIT_INVALID",
                "maximum decoded image bytes must be greater than zero",
            ));
        }

        Ok(Self {
            max_encoded_bytes,
            max_pixels,
            max_decoded_bytes,
        })
    }

    pub const fn max_encoded_bytes(self) -> usize {
        self.max_encoded_bytes
    }

    pub const fn max_pixels(self) -> u64 {
        self.max_pixels
    }

    pub const fn max_decoded_bytes(self) -> u64 {
        self.max_decoded_bytes
    }

    fn decoder_limits(self) -> Limits {
        let max_dimension = self.max_pixels.min(u64::from(u32::MAX)) as u32;
        let mut limits = Limits::default();
        limits.max_image_width = Some(max_dimension);
        limits.max_image_height = Some(max_dimension);
        limits.max_alloc = Some(self.max_decoded_bytes);
        limits
    }
}

/// Borrowed encoded full-resolution image input. Decoding produces an owned RGB
/// buffer so platform-owned file or camera bytes never outlive this call.
#[derive(Clone, Copy, Debug)]
pub struct EncodedImage<'a> {
    bytes: &'a [u8],
    format: EncodedImageFormat,
    rotation: ImageRotation,
    limits: EncodedImageLimits,
}

impl<'a> EncodedImage<'a> {
    pub fn new(
        bytes: &'a [u8],
        format: EncodedImageFormat,
        rotation: ImageRotation,
        limits: EncodedImageLimits,
    ) -> Result<Self, A2dError> {
        if bytes.is_empty() {
            return Err(validation_error(
                "IMAGE_ENCODED_EMPTY",
                "encoded image buffer must not be empty",
            ));
        }
        if bytes.len() > limits.max_encoded_bytes() {
            return Err(validation_error(
                "IMAGE_ENCODED_BYTE_LIMIT_EXCEEDED",
                format!(
                    "encoded image has {} bytes, limit is {}",
                    bytes.len(),
                    limits.max_encoded_bytes()
                ),
            )
            .with_detail("actual_bytes", bytes.len().to_string())
            .with_detail("max_encoded_bytes", limits.max_encoded_bytes().to_string()));
        }
        if !format.has_matching_signature(bytes) {
            return Err(validation_error(
                "IMAGE_FORMAT_MISMATCH",
                format!(
                    "encoded bytes do not match declared {} format",
                    format.as_str()
                ),
            )
            .with_detail("declared_format", format.as_str()));
        }

        Ok(Self {
            bytes,
            format,
            rotation,
            limits,
        })
    }

    pub const fn format(self) -> EncodedImageFormat {
        self.format
    }

    pub const fn rotation(self) -> ImageRotation {
        self.rotation
    }

    pub const fn encoded_len(self) -> usize {
        self.bytes.len()
    }

    /// Decode to owned RGB8 while converting decoder panics and ordinary decode
    /// failures into structured image-processing errors.
    pub fn decode_rgb8(self) -> Result<OwnedRgbImage, A2dError> {
        catch_unwind(AssertUnwindSafe(|| self.decode_rgb8_inner())).map_err(|_| {
            processing_error(
                "IMAGE_DECODER_PANIC",
                format!("{} decoder panicked", self.format.as_str()),
                false,
            )
            .with_detail("format", self.format.as_str())
        })?
    }

    fn decode_rgb8_inner(self) -> Result<OwnedRgbImage, A2dError> {
        let (width, height) = self.read_dimensions()?;
        let pixel_count = checked_pixel_count(width, height)?;
        if pixel_count > self.limits.max_pixels() {
            return Err(validation_error(
                "IMAGE_PIXEL_LIMIT_EXCEEDED",
                format!(
                    "decoded image has {pixel_count} pixels, limit is {}",
                    self.limits.max_pixels()
                ),
            )
            .with_detail("width", width.to_string())
            .with_detail("height", height.to_string())
            .with_detail("max_pixels", self.limits.max_pixels().to_string()));
        }

        let required_rgb_bytes = pixel_count.checked_mul(3).ok_or_else(|| {
            validation_error(
                "IMAGE_BUFFER_SIZE_OVERFLOW",
                format!("RGB buffer size overflow for {width}x{height}"),
            )
        })?;
        if required_rgb_bytes > self.limits.max_decoded_bytes() {
            return Err(validation_error(
                "IMAGE_DECODED_BYTE_LIMIT_EXCEEDED",
                format!(
                    "RGB output requires {required_rgb_bytes} bytes, limit is {}",
                    self.limits.max_decoded_bytes()
                ),
            )
            .with_detail("required_bytes", required_rgb_bytes.to_string())
            .with_detail(
                "max_decoded_bytes",
                self.limits.max_decoded_bytes().to_string(),
            ));
        }
        let required_rgb_bytes = usize::try_from(required_rgb_bytes).map_err(|_| {
            validation_error(
                "IMAGE_BUFFER_SIZE_UNSUPPORTED",
                "RGB output does not fit this platform's address space",
            )
        })?;

        let mut reader =
            ImageReader::with_format(Cursor::new(self.bytes), self.format.image_format());
        reader.limits(self.limits.decoder_limits());
        let decoded = reader.decode().map_err(|error| {
            processing_error(
                "IMAGE_DECODE_FAILED",
                format!("failed to decode {} image: {error}", self.format.as_str()),
                false,
            )
            .with_detail("format", self.format.as_str())
        })?;

        if decoded.width() != width || decoded.height() != height {
            return Err(processing_error(
                "IMAGE_DECODE_DIMENSIONS_CHANGED",
                format!(
                    "{} decoder reported {width}x{height} before decoding but produced {}x{}",
                    self.format.as_str(),
                    decoded.width(),
                    decoded.height()
                ),
                false,
            ));
        }

        let rgb = decoded.into_rgb8();
        let bytes = rgb.into_raw();
        if bytes.len() != required_rgb_bytes {
            return Err(processing_error(
                "IMAGE_DECODE_OUTPUT_INVALID",
                format!(
                    "RGB output has {} bytes but {required_rgb_bytes} were required",
                    bytes.len()
                ),
                false,
            ));
        }

        OwnedRgbImage::from_tight(width, height, self.rotation, bytes)
    }

    fn read_dimensions(self) -> Result<(u32, u32), A2dError> {
        let mut reader =
            ImageReader::with_format(Cursor::new(self.bytes), self.format.image_format());
        reader.limits(self.limits.decoder_limits());
        reader.into_dimensions().map_err(|error| {
            processing_error(
                "IMAGE_DIMENSION_READ_FAILED",
                format!(
                    "failed to read {} image dimensions: {error}",
                    self.format.as_str()
                ),
                false,
            )
            .with_detail("format", self.format.as_str())
        })
    }
}

/// Owned, tightly packed RGB8 full-resolution image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedRgbImage {
    width: u32,
    height: u32,
    row_stride: usize,
    rotation: ImageRotation,
    bytes: Vec<u8>,
}

impl OwnedRgbImage {
    pub(crate) fn from_tight(
        width: u32,
        height: u32,
        rotation: ImageRotation,
        bytes: Vec<u8>,
    ) -> Result<Self, A2dError> {
        let pixel_count = checked_pixel_count(width, height)?;
        let required_bytes = pixel_count.checked_mul(3).ok_or_else(|| {
            validation_error(
                "IMAGE_BUFFER_SIZE_OVERFLOW",
                format!("RGB buffer size overflow for {width}x{height}"),
            )
        })?;
        let required_bytes = usize::try_from(required_bytes).map_err(|_| {
            validation_error(
                "IMAGE_BUFFER_SIZE_UNSUPPORTED",
                "RGB buffer does not fit this platform's address space",
            )
        })?;
        if bytes.len() != required_bytes {
            return Err(validation_error(
                "IMAGE_RGB_BUFFER_LENGTH_INVALID",
                format!(
                    "RGB buffer has {} bytes but {required_bytes} are required",
                    bytes.len()
                ),
            ));
        }
        let row_stride = usize::try_from(width)
            .ok()
            .and_then(|value| value.checked_mul(3))
            .ok_or_else(|| {
                validation_error(
                    "IMAGE_STRIDE_INVALID",
                    format!("RGB row stride overflow for width {width}"),
                )
            })?;
        Ok(Self {
            width,
            height,
            row_stride,
            rotation,
            bytes,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn row_stride(&self) -> usize {
        self.row_stride
    }

    pub const fn rotation(&self) -> ImageRotation {
        self.rotation
    }

    pub const fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgb8
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Consume the RGB image and create a tightly packed owned Gray8 image that
    /// can be borrowed by the existing marker detector boundary.
    pub fn into_gray8(self, limits: ImageLimits) -> Result<OwnedGrayImage, A2dError> {
        let rgb = RgbImage::from_raw(self.width, self.height, self.bytes).ok_or_else(|| {
            processing_error(
                "IMAGE_RGB_BUFFER_INVALID",
                "validated RGB buffer could not be reconstructed",
                false,
            )
        })?;
        let gray = DynamicImage::ImageRgb8(rgb).into_luma8();
        let image = OwnedGrayImage::from_tight(
            self.width,
            self.height,
            self.rotation,
            gray.into_raw(),
        )?;
        image.as_frame(limits)?;
        Ok(image)
    }
}

/// Owned, tightly packed Gray8 image suitable for detector input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedGrayImage {
    width: u32,
    height: u32,
    row_stride: usize,
    rotation: ImageRotation,
    bytes: Vec<u8>,
}

impl OwnedGrayImage {
    pub(crate) fn from_tight(
        width: u32,
        height: u32,
        rotation: ImageRotation,
        bytes: Vec<u8>,
    ) -> Result<Self, A2dError> {
        let required_bytes = checked_pixel_count(width, height)?;
        let required_bytes = usize::try_from(required_bytes).map_err(|_| {
            validation_error(
                "IMAGE_BUFFER_SIZE_UNSUPPORTED",
                "Gray8 buffer does not fit this platform's address space",
            )
        })?;
        if bytes.len() != required_bytes {
            return Err(validation_error(
                "IMAGE_GRAY_BUFFER_LENGTH_INVALID",
                format!(
                    "Gray8 buffer has {} bytes but {required_bytes} are required",
                    bytes.len()
                ),
            ));
        }
        let row_stride = usize::try_from(width).map_err(|_| {
            validation_error(
                "IMAGE_STRIDE_INVALID",
                format!("Gray8 row stride overflow for width {width}"),
            )
        })?;
        Ok(Self {
            width,
            height,
            row_stride,
            rotation,
            bytes,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn row_stride(&self) -> usize {
        self.row_stride
    }

    pub const fn rotation(&self) -> ImageRotation {
        self.rotation
    }

    pub const fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Gray8
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_frame(&self, limits: ImageLimits) -> Result<GrayFrame<'_>, A2dError> {
        GrayFrame::new(
            self.width,
            self.height,
            self.row_stride,
            self.rotation,
            &self.bytes,
            limits,
        )
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn checked_pixel_count(width: u32, height: u32) -> Result<u64, A2dError> {
    if width == 0 || height == 0 {
        return Err(validation_error(
            "IMAGE_DIMENSIONS_INVALID",
            format!("image dimensions must be non-zero, got {width}x{height}"),
        ));
    }
    u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            validation_error(
                "IMAGE_PIXEL_COUNT_OVERFLOW",
                format!("pixel count overflow for {width}x{height}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

    use super::*;

    fn limits() -> EncodedImageLimits {
        EncodedImageLimits::new(1_000_000, 1_000_000, 3_000_000).unwrap()
    }

    fn encode_test_image(format: ImageFormat) -> Vec<u8> {
        let image = RgbImage::from_fn(2, 2, |x, y| match (x, y) {
            (0, 0) => Rgb([255, 0, 0]),
            (1, 0) => Rgb([0, 255, 0]),
            (0, 1) => Rgb([0, 0, 255]),
            _ => Rgb([255, 255, 255]),
        });
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut output, format)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn decodes_png_to_owned_rgb8_and_preserves_rotation() {
        let bytes = encode_test_image(ImageFormat::Png);
        let decoded = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Png,
            ImageRotation::Degrees90,
            limits(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap();

        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);
        assert_eq!(decoded.row_stride(), 6);
        assert_eq!(decoded.rotation(), ImageRotation::Degrees90);
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(
            decoded.bytes(),
            &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
        );
    }

    #[test]
    fn decodes_jpeg_with_the_same_bounded_boundary() {
        let bytes = encode_test_image(ImageFormat::Jpeg);
        let decoded = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Jpeg,
            ImageRotation::Degrees0,
            limits(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap();

        assert_eq!((decoded.width(), decoded.height()), (2, 2));
        assert_eq!(decoded.bytes().len(), 12);
    }

    #[test]
    fn converts_owned_rgb_to_a_valid_borrowed_gray_frame() {
        let bytes = encode_test_image(ImageFormat::Png);
        let rgb = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Png,
            ImageRotation::Degrees180,
            limits(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap();
        let gray = rgb.into_gray8(ImageLimits::new(4).unwrap()).unwrap();
        let frame = gray.as_frame(ImageLimits::new(4).unwrap()).unwrap();

        assert_eq!(gray.pixel_format(), PixelFormat::Gray8);
        assert_eq!(frame.width(), 2);
        assert_eq!(frame.height(), 2);
        assert_eq!(frame.row_stride(), 2);
        assert_eq!(frame.rotation(), ImageRotation::Degrees180);
        assert_eq!(frame.bytes().len(), 4);
    }

    #[test]
    fn rejects_encoded_input_above_the_explicit_byte_limit() {
        let bytes = encode_test_image(ImageFormat::Png);
        let err = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Png,
            ImageRotation::Degrees0,
            EncodedImageLimits::new(bytes.len() - 1, 100, 1_000).unwrap(),
        )
        .unwrap_err();

        assert_eq!(err.code.to_string(), "IMAGE_ENCODED_BYTE_LIMIT_EXCEEDED");
    }

    #[test]
    fn rejects_declared_format_that_does_not_match_the_signature() {
        let bytes = encode_test_image(ImageFormat::Png);
        let err = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Jpeg,
            ImageRotation::Degrees0,
            limits(),
        )
        .unwrap_err();

        assert_eq!(err.code.to_string(), "IMAGE_FORMAT_MISMATCH");
    }

    #[test]
    fn rejects_decoded_pixel_count_above_the_explicit_limit() {
        let bytes = encode_test_image(ImageFormat::Png);
        let err = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Png,
            ImageRotation::Degrees0,
            EncodedImageLimits::new(bytes.len(), 3, 1_000).unwrap(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap_err();

        assert_eq!(err.code.to_string(), "IMAGE_PIXEL_LIMIT_EXCEEDED");
    }

    #[test]
    fn rejects_rgb_output_above_the_explicit_decoded_byte_limit() {
        let bytes = encode_test_image(ImageFormat::Png);
        let err = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Png,
            ImageRotation::Degrees0,
            EncodedImageLimits::new(bytes.len(), 4, 11).unwrap(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap_err();

        assert_eq!(err.code.to_string(), "IMAGE_DECODED_BYTE_LIMIT_EXCEEDED");
    }

    #[test]
    fn rejects_corrupted_data_as_a_structured_processing_error() {
        let bytes = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 1, 2, 3];
        let err = EncodedImage::new(
            &bytes,
            EncodedImageFormat::Png,
            ImageRotation::Degrees0,
            limits(),
        )
        .unwrap()
        .decode_rgb8()
        .unwrap_err();

        assert_eq!(err.code.to_string(), "IMAGE_DIMENSION_READ_FAILED");
    }
}
