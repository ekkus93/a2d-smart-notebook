use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use a2d_domain::A2dError;

use crate::{
    encoded::{OwnedGrayImage, OwnedRgbImage},
    error::{processing_error, validation_error},
    input::ImageRotation,
    rectification::RectificationPlan,
};

const PERCENT_SCALE: u64 = 1_000_000;
const MAX_SHARPEN_PASSES: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContrastNormalizationConfig {
    low_percentile_per_million: u32,
    high_percentile_per_million: u32,
    maximum_gain: f64,
}

impl ContrastNormalizationConfig {
    pub fn new(
        low_percentile_per_million: u32,
        high_percentile_per_million: u32,
        maximum_gain: f64,
    ) -> Result<Self, A2dError> {
        if low_percentile_per_million >= high_percentile_per_million
            || u64::from(high_percentile_per_million) > PERCENT_SCALE
        {
            return Err(validation_error(
                "DERIVED_CONTRAST_PERCENTILES_INVALID",
                format!(
                    "contrast percentiles must satisfy 0 <= low < high <= {PERCENT_SCALE}, got {low_percentile_per_million} and {high_percentile_per_million}"
                ),
            ));
        }
        if !maximum_gain.is_finite() || maximum_gain < 1.0 {
            return Err(validation_error(
                "DERIVED_CONTRAST_GAIN_INVALID",
                format!(
                    "maximum contrast gain must be finite and at least 1.0, got {maximum_gain}"
                ),
            ));
        }
        Ok(Self {
            low_percentile_per_million,
            high_percentile_per_million,
            maximum_gain,
        })
    }

    pub const fn low_percentile_per_million(self) -> u32 {
        self.low_percentile_per_million
    }

    pub const fn high_percentile_per_million(self) -> u32 {
        self.high_percentile_per_million
    }

    pub const fn maximum_gain(self) -> f64 {
        self.maximum_gain
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SharpenConfig {
    amount: f64,
    threshold: u8,
    passes: u8,
}

impl SharpenConfig {
    pub fn new(amount: f64, threshold: u8, passes: u8) -> Result<Self, A2dError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(validation_error(
                "DERIVED_SHARPEN_AMOUNT_INVALID",
                format!("sharpen amount must be finite and positive, got {amount}"),
            ));
        }
        if passes == 0 || passes > MAX_SHARPEN_PASSES {
            return Err(validation_error(
                "DERIVED_SHARPEN_PASSES_INVALID",
                format!("sharpen passes must be between 1 and {MAX_SHARPEN_PASSES}, got {passes}"),
            ));
        }
        Ok(Self {
            amount,
            threshold,
            passes,
        })
    }

    pub const fn amount(self) -> f64 {
        self.amount
    }

    pub const fn threshold(self) -> u8 {
        self.threshold
    }

    pub const fn passes(self) -> u8 {
        self.passes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThumbnailConfig {
    max_width: u32,
    max_height: u32,
}

impl ThumbnailConfig {
    pub fn new(max_width: u32, max_height: u32) -> Result<Self, A2dError> {
        if max_width == 0 || max_height == 0 {
            return Err(validation_error(
                "DERIVED_THUMBNAIL_DIMENSIONS_INVALID",
                "thumbnail maximum dimensions must be non-zero",
            ));
        }
        Ok(Self {
            max_width,
            max_height,
        })
    }

    pub const fn max_width(self) -> u32 {
        self.max_width
    }

    pub const fn max_height(self) -> u32 {
        self.max_height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedImageLimits {
    max_pixels_per_image: u64,
    max_bytes_per_image: u64,
    max_total_output_bytes: u64,
    max_working_bytes: u64,
}

impl DerivedImageLimits {
    pub fn new(
        max_pixels_per_image: u64,
        max_bytes_per_image: u64,
        max_total_output_bytes: u64,
        max_working_bytes: u64,
    ) -> Result<Self, A2dError> {
        if max_pixels_per_image == 0
            || max_bytes_per_image == 0
            || max_total_output_bytes == 0
            || max_working_bytes == 0
        {
            return Err(validation_error(
                "DERIVED_MEMORY_LIMIT_INVALID",
                "all derived-image memory limits must be greater than zero",
            ));
        }
        if max_total_output_bytes > max_working_bytes {
            return Err(validation_error(
                "DERIVED_MEMORY_LIMIT_ORDER_INVALID",
                "maximum total output bytes must not exceed maximum working bytes",
            ));
        }
        Ok(Self {
            max_pixels_per_image,
            max_bytes_per_image,
            max_total_output_bytes,
            max_working_bytes,
        })
    }

    pub const fn max_pixels_per_image(self) -> u64 {
        self.max_pixels_per_image
    }

    pub const fn max_bytes_per_image(self) -> u64 {
        self.max_bytes_per_image
    }

    pub const fn max_total_output_bytes(self) -> u64 {
        self.max_total_output_bytes
    }

    pub const fn max_working_bytes(self) -> u64 {
        self.max_working_bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DerivedImageConfig {
    pipeline_version: u32,
    contrast: ContrastNormalizationConfig,
    sharpening: Option<SharpenConfig>,
    thumbnail: ThumbnailConfig,
    limits: DerivedImageLimits,
}

impl DerivedImageConfig {
    pub fn new(
        pipeline_version: u32,
        contrast: ContrastNormalizationConfig,
        sharpening: Option<SharpenConfig>,
        thumbnail: ThumbnailConfig,
        limits: DerivedImageLimits,
    ) -> Result<Self, A2dError> {
        if pipeline_version == 0 {
            return Err(validation_error(
                "DERIVED_PIPELINE_VERSION_INVALID",
                "derived-image pipeline version must be greater than zero",
            ));
        }
        Ok(Self {
            pipeline_version,
            contrast,
            sharpening,
            thumbnail,
            limits,
        })
    }

    pub const fn pipeline_version(self) -> u32 {
        self.pipeline_version
    }

    pub const fn contrast(self) -> ContrastNormalizationConfig {
        self.contrast
    }

    pub const fn sharpening(self) -> Option<SharpenConfig> {
        self.sharpening
    }

    pub const fn thumbnail(self) -> ThumbnailConfig {
        self.thumbnail
    }

    pub const fn limits(self) -> DerivedImageLimits {
        self.limits
    }
}

#[derive(Clone, Debug)]
pub struct ProcessingCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ProcessingCancellation {
    pub fn active() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn check(&self, stage: &'static str) -> Result<(), A2dError> {
        if self.is_cancelled() {
            return Err(processing_error(
                "DERIVED_PROCESSING_CANCELLED",
                format!("derived-image processing was cancelled before {stage}"),
                true,
            )
            .with_detail("stage", stage));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContrastNormalizationProvenance {
    pub low_luminance: u8,
    pub high_luminance: u8,
    pub requested_maximum_gain: f64,
    pub applied_gain: f64,
    pub changed_pixels: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedImageProvenance {
    pub pipeline_version: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub source_rotation: ImageRotation,
    pub corrected_width: u32,
    pub corrected_height: u32,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub source_to_corrected_matrix: [[f64; 3]; 3],
    pub contrast: ContrastNormalizationProvenance,
    pub sharpening: Option<SharpenConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedImages {
    pub corrected_color: OwnedRgbImage,
    pub ocr_optimized: OwnedGrayImage,
    pub thumbnail: OwnedRgbImage,
    pub provenance: DerivedImageProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DerivedImagePipeline {
    config: DerivedImageConfig,
}

impl DerivedImagePipeline {
    pub const fn new(config: DerivedImageConfig) -> Self {
        Self { config }
    }

    pub const fn config(self) -> DerivedImageConfig {
        self.config
    }

    pub fn process(
        self,
        source: &OwnedRgbImage,
        rectification: &RectificationPlan,
        cancellation: &ProcessingCancellation,
    ) -> Result<DerivedImages, A2dError> {
        cancellation.check("memory preflight")?;
        let dimensions = PipelineDimensions::new(rectification, self.config)?;
        dimensions.validate_limits(self.config.limits())?;

        cancellation.check("color rectification")?;
        let corrected_color = rectification.rectify_rgb8(source)?;

        cancellation.check("OCR grayscale conversion")?;
        let mut ocr_bytes = rgb_to_gray(&corrected_color);

        cancellation.check("contrast normalization")?;
        let contrast = normalize_contrast(&mut ocr_bytes, self.config.contrast());

        if let Some(sharpening) = self.config.sharpening() {
            for pass in 0..sharpening.passes() {
                cancellation.check("OCR sharpening")?;
                sharpen_gray_in_place(
                    &mut ocr_bytes,
                    corrected_color.width(),
                    corrected_color.height(),
                    sharpening,
                )?;
                if pass + 1 < sharpening.passes() {
                    cancellation.check("next OCR sharpening pass")?;
                }
            }
        }
        let ocr_optimized = OwnedGrayImage::from_tight(
            corrected_color.width(),
            corrected_color.height(),
            ImageRotation::Degrees0,
            ocr_bytes,
        )?;

        cancellation.check("thumbnail generation")?;
        let thumbnail = resize_rgb8(
            &corrected_color,
            dimensions.thumbnail_width,
            dimensions.thumbnail_height,
        )?;

        cancellation.check("atomic result assembly")?;
        let provenance = DerivedImageProvenance {
            pipeline_version: self.config.pipeline_version(),
            source_width: source.width(),
            source_height: source.height(),
            source_rotation: source.rotation(),
            corrected_width: corrected_color.width(),
            corrected_height: corrected_color.height(),
            thumbnail_width: thumbnail.width(),
            thumbnail_height: thumbnail.height(),
            source_to_corrected_matrix: rectification.transform().source_to_destination_matrix(),
            contrast,
            sharpening: self.config.sharpening(),
        };

        Ok(DerivedImages {
            corrected_color,
            ocr_optimized,
            thumbnail,
            provenance,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PipelineDimensions {
    corrected_pixels: u64,
    corrected_rgb_bytes: u64,
    ocr_bytes: u64,
    thumbnail_width: u32,
    thumbnail_height: u32,
    thumbnail_pixels: u64,
    thumbnail_rgb_bytes: u64,
}

impl PipelineDimensions {
    fn new(
        rectification: &RectificationPlan,
        config: DerivedImageConfig,
    ) -> Result<Self, A2dError> {
        let output = rectification.output_size();
        let corrected_pixels = output.pixel_count();
        let corrected_rgb_bytes = output.rgb_byte_count();
        let ocr_bytes = corrected_pixels;
        let (thumbnail_width, thumbnail_height) =
            thumbnail_dimensions(output.width(), output.height(), config.thumbnail())?;
        let thumbnail_pixels = u64::from(thumbnail_width)
            .checked_mul(u64::from(thumbnail_height))
            .ok_or_else(|| {
                validation_error(
                    "DERIVED_THUMBNAIL_PIXEL_OVERFLOW",
                    "thumbnail pixel count overflowed",
                )
            })?;
        let thumbnail_rgb_bytes = thumbnail_pixels.checked_mul(3).ok_or_else(|| {
            validation_error(
                "DERIVED_THUMBNAIL_BYTE_OVERFLOW",
                "thumbnail RGB byte count overflowed",
            )
        })?;
        Ok(Self {
            corrected_pixels,
            corrected_rgb_bytes,
            ocr_bytes,
            thumbnail_width,
            thumbnail_height,
            thumbnail_pixels,
            thumbnail_rgb_bytes,
        })
    }

    fn validate_limits(self, limits: DerivedImageLimits) -> Result<(), A2dError> {
        for (name, pixels, bytes) in [
            (
                "corrected color",
                self.corrected_pixels,
                self.corrected_rgb_bytes,
            ),
            ("OCR optimized", self.corrected_pixels, self.ocr_bytes),
            ("thumbnail", self.thumbnail_pixels, self.thumbnail_rgb_bytes),
        ] {
            if pixels > limits.max_pixels_per_image() {
                return Err(validation_error(
                    "DERIVED_PIXEL_LIMIT_EXCEEDED",
                    format!(
                        "{name} output has {pixels} pixels, limit is {}",
                        limits.max_pixels_per_image()
                    ),
                )
                .with_detail("output", name)
                .with_detail("pixels", pixels.to_string()));
            }
            if bytes > limits.max_bytes_per_image() {
                return Err(validation_error(
                    "DERIVED_IMAGE_BYTE_LIMIT_EXCEEDED",
                    format!(
                        "{name} output requires {bytes} bytes, limit is {}",
                        limits.max_bytes_per_image()
                    ),
                )
                .with_detail("output", name)
                .with_detail("bytes", bytes.to_string()));
            }
            usize::try_from(bytes).map_err(|_| {
                validation_error(
                    "DERIVED_IMAGE_SIZE_UNSUPPORTED",
                    format!("{name} output does not fit this platform's address space"),
                )
            })?;
        }

        let total_output_bytes = self
            .corrected_rgb_bytes
            .checked_add(self.ocr_bytes)
            .and_then(|value| value.checked_add(self.thumbnail_rgb_bytes))
            .ok_or_else(|| {
                validation_error(
                    "DERIVED_TOTAL_OUTPUT_OVERFLOW",
                    "derived-image total output size overflowed",
                )
            })?;
        if total_output_bytes > limits.max_total_output_bytes() {
            return Err(validation_error(
                "DERIVED_TOTAL_OUTPUT_LIMIT_EXCEEDED",
                format!(
                    "derived outputs require {total_output_bytes} bytes, limit is {}",
                    limits.max_total_output_bytes()
                ),
            ));
        }

        let scratch_bytes = self.ocr_bytes;
        let peak_working_bytes =
            total_output_bytes
                .checked_add(scratch_bytes)
                .ok_or_else(|| {
                    validation_error(
                        "DERIVED_WORKING_SET_OVERFLOW",
                        "derived-image working-set estimate overflowed",
                    )
                })?;
        if peak_working_bytes > limits.max_working_bytes() {
            return Err(validation_error(
                "DERIVED_WORKING_SET_LIMIT_EXCEEDED",
                format!(
                    "derived-image working set requires up to {peak_working_bytes} bytes, limit is {}",
                    limits.max_working_bytes()
                ),
            ));
        }
        Ok(())
    }
}

fn rgb_to_gray(source: &OwnedRgbImage) -> Vec<u8> {
    source
        .bytes()
        .chunks_exact(3)
        .map(|pixel| {
            let weighted = 77_u32 * u32::from(pixel[0])
                + 150_u32 * u32::from(pixel[1])
                + 29_u32 * u32::from(pixel[2])
                + 128;
            (weighted >> 8) as u8
        })
        .collect()
}

fn normalize_contrast(
    bytes: &mut [u8],
    config: ContrastNormalizationConfig,
) -> ContrastNormalizationProvenance {
    let mut histogram = [0_u64; 256];
    for value in bytes.iter().copied() {
        histogram[usize::from(value)] += 1;
    }
    let pixel_count = bytes.len() as u64;
    let low_rank = pixel_count * u64::from(config.low_percentile_per_million()) / PERCENT_SCALE;
    let high_rank = pixel_count * u64::from(config.high_percentile_per_million()) / PERCENT_SCALE;
    let low = histogram_value_at_rank(&histogram, low_rank);
    let high = histogram_value_at_rank(&histogram, high_rank.min(pixel_count - 1));
    if high <= low {
        return ContrastNormalizationProvenance {
            low_luminance: low,
            high_luminance: high,
            requested_maximum_gain: config.maximum_gain(),
            applied_gain: 1.0,
            changed_pixels: false,
        };
    }

    let raw_gain = 255.0 / f64::from(high - low);
    let applied_gain = raw_gain.min(config.maximum_gain());
    if applied_gain <= 1.0 {
        return ContrastNormalizationProvenance {
            low_luminance: low,
            high_luminance: high,
            requested_maximum_gain: config.maximum_gain(),
            applied_gain: 1.0,
            changed_pixels: false,
        };
    }
    let midpoint = (f64::from(low) + f64::from(high)) * 0.5;
    let mut changed_pixels = false;
    for value in bytes {
        let normalized = (midpoint + (f64::from(*value) - midpoint) * applied_gain)
            .round()
            .clamp(0.0, 255.0) as u8;
        changed_pixels |= normalized != *value;
        *value = normalized;
    }
    ContrastNormalizationProvenance {
        low_luminance: low,
        high_luminance: high,
        requested_maximum_gain: config.maximum_gain(),
        applied_gain,
        changed_pixels,
    }
}

fn histogram_value_at_rank(histogram: &[u64; 256], rank: u64) -> u8 {
    let mut cumulative = 0_u64;
    for (value, count) in histogram.iter().copied().enumerate() {
        cumulative += count;
        if cumulative > rank {
            return value as u8;
        }
    }
    u8::MAX
}

fn sharpen_gray_in_place(
    bytes: &mut [u8],
    width: u32,
    height: u32,
    config: SharpenConfig,
) -> Result<(), A2dError> {
    let width = width as usize;
    let height = height as usize;
    if width < 3 || height < 3 {
        return Ok(());
    }
    let original = bytes.to_vec();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let index = y * width + x;
            let center = f64::from(original[index]);
            let cross_blur = (center
                + f64::from(original[index - 1])
                + f64::from(original[index + 1])
                + f64::from(original[index - width])
                + f64::from(original[index + width]))
                / 5.0;
            let detail = center - cross_blur;
            if detail.abs() >= f64::from(config.threshold()) {
                bytes[index] = (center + config.amount() * detail)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
    Ok(())
}

fn thumbnail_dimensions(
    width: u32,
    height: u32,
    config: ThumbnailConfig,
) -> Result<(u32, u32), A2dError> {
    if width == 0 || height == 0 {
        return Err(validation_error(
            "DERIVED_SOURCE_DIMENSIONS_INVALID",
            "thumbnail source dimensions must be non-zero",
        ));
    }
    if width <= config.max_width() && height <= config.max_height() {
        return Ok((width, height));
    }
    let width_limited = u64::from(config.max_width()) * u64::from(height)
        <= u64::from(config.max_height()) * u64::from(width);
    if width_limited {
        let thumbnail_height =
            (u64::from(height) * u64::from(config.max_width()) / u64::from(width)).max(1);
        Ok((config.max_width(), thumbnail_height as u32))
    } else {
        let thumbnail_width =
            (u64::from(width) * u64::from(config.max_height()) / u64::from(height)).max(1);
        Ok((thumbnail_width as u32, config.max_height()))
    }
}

fn resize_rgb8(
    source: &OwnedRgbImage,
    destination_width: u32,
    destination_height: u32,
) -> Result<OwnedRgbImage, A2dError> {
    let output_len = u64::from(destination_width)
        .checked_mul(u64::from(destination_height))
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            validation_error(
                "DERIVED_THUMBNAIL_SIZE_UNSUPPORTED",
                "thumbnail output does not fit this platform's address space",
            )
        })?;
    let mut output = Vec::with_capacity(output_len);
    let source_max_x = f64::from(source.width() - 1);
    let source_max_y = f64::from(source.height() - 1);
    let destination_max_x = f64::from(destination_width.saturating_sub(1));
    let destination_max_y = f64::from(destination_height.saturating_sub(1));
    for y in 0..destination_height {
        let source_y = if destination_height == 1 {
            source_max_y * 0.5
        } else {
            f64::from(y) * source_max_y / destination_max_y
        };
        for x in 0..destination_width {
            let source_x = if destination_width == 1 {
                source_max_x * 0.5
            } else {
                f64::from(x) * source_max_x / destination_max_x
            };
            output.extend_from_slice(&sample_rgb(source, source_x, source_y));
        }
    }
    OwnedRgbImage::from_tight(
        destination_width,
        destination_height,
        ImageRotation::Degrees0,
        output,
    )
}

fn sample_rgb(source: &OwnedRgbImage, x: f64, y: f64) -> [u8; 3] {
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(source.width() as usize - 1);
    let y1 = (y0 + 1).min(source.height() as usize - 1);
    let x_fraction = x - x0 as f64;
    let y_fraction = y - y0 as f64;
    let stride = source.row_stride();
    let bytes = source.bytes();
    let pixel = |column: usize, row: usize, channel: usize| -> f64 {
        f64::from(bytes[row * stride + column * 3 + channel])
    };
    let mut result = [0_u8; 3];
    for (channel, output) in result.iter_mut().enumerate() {
        let top =
            pixel(x0, y0, channel) + (pixel(x1, y0, channel) - pixel(x0, y0, channel)) * x_fraction;
        let bottom =
            pixel(x0, y1, channel) + (pixel(x1, y1, channel) - pixel(x0, y1, channel)) * x_fraction;
        *output = (top + (bottom - top) * y_fraction)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::{ImagePoint, ImageQuad, RectificationLimits, RectifiedImageSize};

    use super::*;

    fn point(x: f64, y: f64) -> ImagePoint {
        ImagePoint { x, y }
    }

    fn source_image(width: u32, height: u32) -> OwnedRgbImage {
        let mut bytes = Vec::new();
        for y in 0..height {
            for x in 0..width {
                bytes.extend_from_slice(&[(x * 20) as u8, (y * 20) as u8, ((x + y) * 10) as u8]);
            }
        }
        OwnedRgbImage::from_tight(width, height, ImageRotation::Degrees90, bytes).unwrap()
    }

    fn identity_plan(width: u32, height: u32) -> RectificationPlan {
        let quad = ImageQuad::new(
            point(0.0, 0.0),
            point(f64::from(width - 1), 0.0),
            point(f64::from(width - 1), f64::from(height - 1)),
            point(0.0, f64::from(height - 1)),
        );
        RectificationPlan::from_page_corners(
            width,
            height,
            quad,
            RectifiedImageSize::new(
                width,
                height,
                RectificationLimits::new(1_000_000, 3_000_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn config(thumbnail_width: u32, thumbnail_height: u32) -> DerivedImageConfig {
        DerivedImageConfig::new(
            3,
            ContrastNormalizationConfig::new(0, 1_000_000, 1.0).unwrap(),
            None,
            ThumbnailConfig::new(thumbnail_width, thumbnail_height).unwrap(),
            DerivedImageLimits::new(1_000_000, 3_000_000, 7_000_000, 8_000_000).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn identity_pipeline_returns_new_outputs_and_preserves_original() {
        let source = source_image(4, 4);
        let original = source.bytes().to_vec();
        let result = DerivedImagePipeline::new(config(2, 2))
            .process(
                &source,
                &identity_plan(4, 4),
                &ProcessingCancellation::active(),
            )
            .unwrap();

        assert_eq!(source.bytes(), original);
        assert_eq!(result.corrected_color.bytes(), original);
        assert_eq!(
            (result.thumbnail.width(), result.thumbnail.height()),
            (2, 2)
        );
        assert_eq!(result.ocr_optimized.bytes().len(), 16);
        assert_eq!(result.provenance.pipeline_version, 3);
        assert_eq!(result.provenance.source_rotation, ImageRotation::Degrees90);
        assert_eq!(result.corrected_color.rotation(), ImageRotation::Degrees0);
    }

    #[test]
    fn thumbnail_preserves_aspect_ratio_and_never_upscales() {
        assert_eq!(
            thumbnail_dimensions(400, 200, ThumbnailConfig::new(100, 100).unwrap()).unwrap(),
            (100, 50)
        );
        assert_eq!(
            thumbnail_dimensions(40, 20, ThumbnailConfig::new(100, 100).unwrap()).unwrap(),
            (40, 20)
        );
    }

    #[test]
    fn contrast_normalization_is_bounded_by_requested_gain() {
        let mut bytes = [100_u8, 110, 120, 130];
        let provenance = normalize_contrast(
            &mut bytes,
            ContrastNormalizationConfig::new(0, 1_000_000, 2.0).unwrap(),
        );
        assert_eq!(provenance.applied_gain, 2.0);
        assert_eq!(bytes, [85, 105, 125, 145]);
        assert!(provenance.changed_pixels);
    }

    #[test]
    fn optional_sharpening_changes_only_eligible_interior_pixels() {
        let mut bytes = vec![100_u8; 25];
        bytes[12] = 150;
        sharpen_gray_in_place(&mut bytes, 5, 5, SharpenConfig::new(1.0, 1, 1).unwrap()).unwrap();
        assert!(bytes[12] > 150);
        assert_eq!(bytes[0], 100);
    }

    #[test]
    fn cancelled_pipeline_returns_no_partial_result() {
        let cancellation = ProcessingCancellation::active();
        cancellation.cancel();
        let err = DerivedImagePipeline::new(config(2, 2))
            .process(&source_image(4, 4), &identity_plan(4, 4), &cancellation)
            .unwrap_err();
        assert_eq!(err.code.to_string(), "DERIVED_PROCESSING_CANCELLED");
    }

    #[test]
    fn preflight_rejects_output_before_image_processing() {
        let limited = DerivedImageConfig::new(
            1,
            ContrastNormalizationConfig::new(0, 1_000_000, 1.0).unwrap(),
            None,
            ThumbnailConfig::new(2, 2).unwrap(),
            DerivedImageLimits::new(15, 100, 1_000, 1_000).unwrap(),
        )
        .unwrap();
        let err = DerivedImagePipeline::new(limited)
            .process(
                &source_image(4, 4),
                &identity_plan(4, 4),
                &ProcessingCancellation::active(),
            )
            .unwrap_err();
        assert_eq!(err.code.to_string(), "DERIVED_PIXEL_LIMIT_EXCEEDED");
    }

    #[test]
    fn invalid_pipeline_configuration_is_rejected() {
        let err = DerivedImageConfig::new(
            0,
            ContrastNormalizationConfig::new(0, 1_000_000, 1.0).unwrap(),
            None,
            ThumbnailConfig::new(1, 1).unwrap(),
            DerivedImageLimits::new(1, 3, 4, 4).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "DERIVED_PIPELINE_VERSION_INVALID");

        let err = DerivedImageLimits::new(1, 3, 5, 4).unwrap_err();
        assert_eq!(err.code.to_string(), "DERIVED_MEMORY_LIMIT_ORDER_INVALID");
    }
}
