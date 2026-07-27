use a2d_domain::A2dError;
use a2d_layout::PageLayout;

use crate::{
    detection::{ImagePoint, ResolvedPageMarkers},
    error::{capture_quality_error, validation_error},
    input::GrayFrame,
    rectification::{ImageQuad, RectifiedImageSize},
};

const MAX_QUALITY_TILES: usize = 4_096;
const BOUNDS_EPSILON: f64 = 1.0e-6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualityState {
    Accepted,
    AcceptedWithWarnings,
    NeedsReview,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricState {
    Accepted,
    Warning,
    NeedsReview,
    Rejected,
    Unavailable,
}

impl MetricState {
    const fn rank(self) -> u8 {
        match self {
            Self::Unavailable => 0,
            Self::Accepted => 1,
            Self::Warning => 2,
            Self::NeedsReview => 3,
            Self::Rejected => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LuminanceMeasurementConfig {
    dark_cutoff: u8,
    highlight_cutoff: u8,
    tile_columns: u16,
    tile_rows: u16,
}

impl LuminanceMeasurementConfig {
    pub fn new(
        dark_cutoff: u8,
        highlight_cutoff: u8,
        tile_columns: u16,
        tile_rows: u16,
    ) -> Result<Self, A2dError> {
        if dark_cutoff >= highlight_cutoff {
            return Err(validation_error(
                "QUALITY_LUMINANCE_CUTOFFS_INVALID",
                format!(
                    "dark cutoff {dark_cutoff} must be below highlight cutoff {highlight_cutoff}"
                ),
            ));
        }
        if tile_columns == 0 || tile_rows == 0 {
            return Err(validation_error(
                "QUALITY_TILE_GRID_INVALID",
                "quality tile grid dimensions must be non-zero",
            ));
        }
        let tile_count = usize::from(tile_columns)
            .checked_mul(usize::from(tile_rows))
            .ok_or_else(|| {
                validation_error(
                    "QUALITY_TILE_GRID_OVERFLOW",
                    "quality tile grid size overflowed",
                )
            })?;
        if tile_count > MAX_QUALITY_TILES {
            return Err(validation_error(
                "QUALITY_TILE_GRID_TOO_LARGE",
                format!(
                    "quality tile grid contains {tile_count} tiles, safety limit is {MAX_QUALITY_TILES}"
                ),
            ));
        }
        Ok(Self {
            dark_cutoff,
            highlight_cutoff,
            tile_columns,
            tile_rows,
        })
    }

    pub const fn dark_cutoff(self) -> u8 {
        self.dark_cutoff
    }

    pub const fn highlight_cutoff(self) -> u8 {
        self.highlight_cutoff
    }

    pub const fn tile_columns(self) -> u16 {
        self.tile_columns
    }

    pub const fn tile_rows(self) -> u16 {
        self.tile_rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusMetrics {
    pub laplacian_variance: f64,
    pub interior_sample_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureMetrics {
    pub mean_luminance: f64,
    pub luminance_standard_deviation: f64,
    pub dark_fraction: f64,
    pub highlight_fraction: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlareMetrics {
    pub highlight_fraction: f64,
    pub max_tile_highlight_fraction: f64,
    pub populated_tile_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrayQualityMetrics {
    pub focus: Option<FocusMetrics>,
    pub exposure: ExposureMetrics,
    pub glare: GlareMetrics,
}

pub fn measure_gray_quality(
    frame: GrayFrame<'_>,
    config: LuminanceMeasurementConfig,
) -> Result<GrayQualityMetrics, A2dError> {
    let width = usize::try_from(frame.width()).map_err(|_| {
        validation_error(
            "QUALITY_IMAGE_DIMENSIONS_UNSUPPORTED",
            "quality image width does not fit this platform",
        )
    })?;
    let height = usize::try_from(frame.height()).map_err(|_| {
        validation_error(
            "QUALITY_IMAGE_DIMENSIONS_UNSUPPORTED",
            "quality image height does not fit this platform",
        )
    })?;
    let pixel_count = width.checked_mul(height).ok_or_else(|| {
        validation_error(
            "QUALITY_PIXEL_COUNT_OVERFLOW",
            "quality image pixel count overflowed",
        )
    })?;
    let tile_columns = usize::from(config.tile_columns());
    let tile_rows = usize::from(config.tile_rows());
    let tile_count = tile_columns * tile_rows;
    let mut tile_pixels = vec![0_u64; tile_count];
    let mut tile_highlights = vec![0_u64; tile_count];
    let mut sum = 0.0;
    let mut sum_of_squares = 0.0;
    let mut dark_count = 0_u64;
    let mut highlight_count = 0_u64;
    let bytes = frame.bytes();
    let stride = frame.row_stride();

    for y in 0..height {
        let tile_y = y * tile_rows / height;
        let row = &bytes[y * stride..y * stride + width];
        for (x, value) in row.iter().copied().enumerate() {
            let numeric = f64::from(value);
            sum += numeric;
            sum_of_squares += numeric * numeric;
            if value <= config.dark_cutoff() {
                dark_count += 1;
            }
            let highlighted = value >= config.highlight_cutoff();
            if highlighted {
                highlight_count += 1;
            }
            let tile_x = x * tile_columns / width;
            let tile_index = tile_y * tile_columns + tile_x;
            tile_pixels[tile_index] += 1;
            if highlighted {
                tile_highlights[tile_index] += 1;
            }
        }
    }

    let pixel_count_f64 = pixel_count as f64;
    let mean = sum / pixel_count_f64;
    let variance = (sum_of_squares / pixel_count_f64 - mean * mean).max(0.0);
    let max_tile_highlight_fraction = tile_pixels
        .iter()
        .zip(&tile_highlights)
        .filter(|(count, _)| **count > 0)
        .map(|(count, highlights)| *highlights as f64 / *count as f64)
        .fold(0.0_f64, f64::max);
    let populated_tile_count = tile_pixels.iter().filter(|count| **count > 0).count();
    let populated_tile_count = u32::try_from(populated_tile_count).map_err(|_| {
        validation_error(
            "QUALITY_TILE_COUNT_UNSUPPORTED",
            "populated quality tile count does not fit u32",
        )
    })?;

    Ok(GrayQualityMetrics {
        focus: measure_focus(frame),
        exposure: ExposureMetrics {
            mean_luminance: mean,
            luminance_standard_deviation: variance.sqrt(),
            dark_fraction: dark_count as f64 / pixel_count_f64,
            highlight_fraction: highlight_count as f64 / pixel_count_f64,
        },
        glare: GlareMetrics {
            highlight_fraction: highlight_count as f64 / pixel_count_f64,
            max_tile_highlight_fraction,
            populated_tile_count,
        },
    })
}

fn measure_focus(frame: GrayFrame<'_>) -> Option<FocusMetrics> {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width < 3 || height < 3 {
        return None;
    }
    let bytes = frame.bytes();
    let stride = frame.row_stride();
    let mut count = 0_u64;
    let mut mean = 0.0;
    let mut sum_squared_delta = 0.0;

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let center = f64::from(bytes[y * stride + x]);
            let laplacian = 4.0 * center
                - f64::from(bytes[y * stride + x - 1])
                - f64::from(bytes[y * stride + x + 1])
                - f64::from(bytes[(y - 1) * stride + x])
                - f64::from(bytes[(y + 1) * stride + x]);
            count += 1;
            let delta = laplacian - mean;
            mean += delta / count as f64;
            sum_squared_delta += delta * (laplacian - mean);
        }
    }

    Some(FocusMetrics {
        laplacian_variance: sum_squared_delta / count as f64,
        interior_sample_count: count,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramingMetrics {
    pub page_area_fraction: f64,
    pub minimum_border_margin_fraction: f64,
    pub center_offset_fraction: f64,
    pub left_margin_fraction: f64,
    pub right_margin_fraction: f64,
    pub top_margin_fraction: f64,
    pub bottom_margin_fraction: f64,
}

pub fn measure_framing(
    source_width: u32,
    source_height: u32,
    page_quad: ImageQuad,
) -> Result<FramingMetrics, A2dError> {
    validate_source_and_quad(source_width, source_height, page_quad)?;
    let max_x = f64::from(source_width - 1);
    let max_y = f64::from(source_height - 1);
    let points = page_quad.points();
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_page_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_page_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let left_margin_fraction = min_x / max_x;
    let right_margin_fraction = (max_x - max_page_x) / max_x;
    let top_margin_fraction = min_y / max_y;
    let bottom_margin_fraction = (max_y - max_page_y) / max_y;
    let minimum_border_margin_fraction = [
        left_margin_fraction,
        right_margin_fraction,
        top_margin_fraction,
        bottom_margin_fraction,
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min);
    let page_center = ImagePoint {
        x: points.iter().map(|point| point.x).sum::<f64>() / 4.0,
        y: points.iter().map(|point| point.y).sum::<f64>() / 4.0,
    };
    let image_center = ImagePoint {
        x: max_x * 0.5,
        y: max_y * 0.5,
    };
    let half_diagonal = (max_x * max_x + max_y * max_y).sqrt() * 0.5;

    Ok(FramingMetrics {
        page_area_fraction: page_quad.signed_area().abs() / (max_x * max_y),
        minimum_border_margin_fraction,
        center_offset_fraction: distance(page_center, image_center) / half_diagonal,
        left_margin_fraction,
        right_margin_fraction,
        top_margin_fraction,
        bottom_margin_fraction,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerConfidenceMetrics {
    pub minimum_decision_margin: f64,
    pub mean_decision_margin: f64,
    pub maximum_hamming_errors: u8,
    pub unexpected_tag_count: u32,
}

pub fn measure_marker_confidence(
    markers: &ResolvedPageMarkers,
) -> Result<MarkerConfidenceMetrics, A2dError> {
    let mut minimum_decision_margin = f64::INFINITY;
    let mut decision_margin_sum = 0.0;
    let mut maximum_hamming_errors = 0_u8;
    for marker in &markers.markers {
        let margin = f64::from(marker.detection.decision_margin);
        if !margin.is_finite() || margin < 0.0 {
            return Err(capture_quality_error(
                "QUALITY_MARKER_MARGIN_INVALID",
                format!(
                    "marker {} has invalid decision margin {margin}",
                    marker.detection.id
                ),
                true,
            ));
        }
        minimum_decision_margin = minimum_decision_margin.min(margin);
        decision_margin_sum += margin;
        maximum_hamming_errors = maximum_hamming_errors.max(marker.detection.hamming_errors);
    }
    let unexpected_tag_count = u32::try_from(markers.unexpected_tag_ids.len()).map_err(|_| {
        validation_error(
            "QUALITY_UNEXPECTED_TAG_COUNT_UNSUPPORTED",
            "unexpected tag count does not fit u32",
        )
    })?;
    Ok(MarkerConfidenceMetrics {
        minimum_decision_margin,
        mean_decision_margin: decision_margin_sum / markers.markers.len() as f64,
        maximum_hamming_errors,
        unexpected_tag_count,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveMetrics {
    pub maximum_to_minimum_edge_ratio: f64,
    pub opposing_edge_imbalance_ratio: f64,
    pub diagonal_imbalance_ratio: f64,
    pub quad_to_bounding_box_area_ratio: f64,
}

pub fn measure_perspective(page_quad: ImageQuad) -> Result<PerspectiveMetrics, A2dError> {
    page_quad.validate("quality page")?;
    let points = page_quad.points();
    let edges = [
        distance(points[0], points[1]),
        distance(points[1], points[2]),
        distance(points[2], points[3]),
        distance(points[3], points[0]),
    ];
    let minimum_edge = edges.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum_edge = edges.iter().copied().fold(0.0_f64, f64::max);
    let diagonals = [
        distance(points[0], points[2]),
        distance(points[1], points[3]),
    ];
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let bounding_box_area = (max_x - min_x) * (max_y - min_y);

    Ok(PerspectiveMetrics {
        maximum_to_minimum_edge_ratio: maximum_edge / minimum_edge,
        opposing_edge_imbalance_ratio: symmetric_ratio(edges[0], edges[2])
            .max(symmetric_ratio(edges[1], edges[3])),
        diagonal_imbalance_ratio: symmetric_ratio(diagonals[0], diagonals[1]),
        quad_to_bounding_box_area_ratio: page_quad.signed_area().abs() / bounding_box_area,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolutionMetrics {
    pub source_minimum_pixels_per_mm: f64,
    pub output_minimum_pixels_per_mm: f64,
}

pub fn measure_effective_resolution(
    source_page_quad: ImageQuad,
    output_size: RectifiedImageSize,
    layout: &PageLayout,
) -> Result<ResolutionMetrics, A2dError> {
    source_page_quad.validate("resolution page")?;
    layout.validate()?;
    let page_width_mm = layout.physical_size.width_mm;
    let page_height_mm = layout.physical_size.height_mm;
    if !page_width_mm.is_finite()
        || !page_height_mm.is_finite()
        || page_width_mm <= 0.0
        || page_height_mm <= 0.0
    {
        return Err(validation_error(
            "QUALITY_LAYOUT_SIZE_INVALID",
            format!(
                "layout physical size must be finite and positive, got {page_width_mm}x{page_height_mm}mm"
            ),
        ));
    }
    let points = source_page_quad.points();
    let source_values = [
        distance(points[0], points[1]) / page_width_mm,
        distance(points[2], points[3]) / page_width_mm,
        distance(points[1], points[2]) / page_height_mm,
        distance(points[3], points[0]) / page_height_mm,
    ];
    let output_values = [
        f64::from(output_size.width() - 1) / page_width_mm,
        f64::from(output_size.height() - 1) / page_height_mm,
    ];
    Ok(ResolutionMetrics {
        source_minimum_pixels_per_mm: source_values.into_iter().fold(f64::INFINITY, f64::min),
        output_minimum_pixels_per_mm: output_values.into_iter().fold(f64::INFINITY, f64::min),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageEdge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeProbe {
    pub edge: PageEdge,
    pub normalized_position: f64,
    pub observed: ImagePoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvatureMetrics {
    pub maximum_normalized_deviation: f64,
    pub rms_normalized_deviation: f64,
    pub sample_count: u32,
}

pub fn measure_curvature(
    page_quad: ImageQuad,
    probes: &[EdgeProbe],
) -> Result<Option<CurvatureMetrics>, A2dError> {
    page_quad.validate("curvature page")?;
    if probes.is_empty() {
        return Ok(None);
    }
    let mut maximum = 0.0_f64;
    let mut squared_sum = 0.0;
    for probe in probes {
        if !probe.normalized_position.is_finite()
            || !(0.0..=1.0).contains(&probe.normalized_position)
            || !probe.observed.x.is_finite()
            || !probe.observed.y.is_finite()
        {
            return Err(validation_error(
                "QUALITY_CURVATURE_PROBE_INVALID",
                "curvature probes require finite positions in [0, 1] and finite observed points",
            ));
        }
        let (start, end) = edge_points(page_quad, probe.edge);
        let expected = interpolate(start, end, probe.normalized_position);
        let edge_dx = end.x - start.x;
        let edge_dy = end.y - start.y;
        let edge_length_squared = edge_dx * edge_dx + edge_dy * edge_dy;
        let perpendicular_cross =
            edge_dx * (probe.observed.y - expected.y) - edge_dy * (probe.observed.x - expected.x);
        let normalized_deviation = perpendicular_cross.abs() / edge_length_squared;
        maximum = maximum.max(normalized_deviation);
        squared_sum += normalized_deviation * normalized_deviation;
    }
    let sample_count = u32::try_from(probes.len()).map_err(|_| {
        validation_error(
            "QUALITY_CURVATURE_SAMPLE_COUNT_UNSUPPORTED",
            "curvature probe count does not fit u32",
        )
    })?;
    Ok(Some(CurvatureMetrics {
        maximum_normalized_deviation: maximum,
        rms_normalized_deviation: (squared_sum / probes.len() as f64).sqrt(),
        sample_count,
    }))
}

#[derive(Clone, Debug, PartialEq)]
pub struct QualityMeasurements {
    pub focus: Option<FocusMetrics>,
    pub exposure: Option<ExposureMetrics>,
    pub glare: Option<GlareMetrics>,
    pub framing: Option<FramingMetrics>,
    pub marker_confidence: Option<MarkerConfidenceMetrics>,
    pub perspective: Option<PerspectiveMetrics>,
    pub resolution: Option<ResolutionMetrics>,
    pub curvature: Option<CurvatureMetrics>,
}

impl QualityMeasurements {
    pub const fn empty() -> Self {
        Self {
            focus: None,
            exposure: None,
            glare: None,
            framing: None,
            marker_confidence: None,
            perspective: None,
            resolution: None,
            curvature: None,
        }
    }

    pub fn with_gray(mut self, metrics: GrayQualityMetrics) -> Self {
        self.focus = metrics.focus;
        self.exposure = Some(metrics.exposure);
        self.glare = Some(metrics.glare);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThresholdDirection {
    HigherIsBetter,
    LowerIsBetter,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalarThresholds {
    direction: ThresholdDirection,
    accepted_boundary: f64,
    warning_boundary: f64,
    review_boundary: f64,
}

impl ScalarThresholds {
    pub fn higher_is_better(
        accepted_minimum: f64,
        warning_minimum: f64,
        review_minimum: f64,
    ) -> Result<Self, A2dError> {
        validate_finite_thresholds([accepted_minimum, warning_minimum, review_minimum])?;
        if !(accepted_minimum > warning_minimum && warning_minimum > review_minimum) {
            return Err(validation_error(
                "QUALITY_HIGHER_THRESHOLDS_INVALID",
                "higher-is-better thresholds must satisfy accepted > warning > review",
            ));
        }
        Ok(Self {
            direction: ThresholdDirection::HigherIsBetter,
            accepted_boundary: accepted_minimum,
            warning_boundary: warning_minimum,
            review_boundary: review_minimum,
        })
    }

    pub fn lower_is_better(
        accepted_maximum: f64,
        warning_maximum: f64,
        review_maximum: f64,
    ) -> Result<Self, A2dError> {
        validate_finite_thresholds([accepted_maximum, warning_maximum, review_maximum])?;
        if !(accepted_maximum < warning_maximum && warning_maximum < review_maximum) {
            return Err(validation_error(
                "QUALITY_LOWER_THRESHOLDS_INVALID",
                "lower-is-better thresholds must satisfy accepted < warning < review",
            ));
        }
        Ok(Self {
            direction: ThresholdDirection::LowerIsBetter,
            accepted_boundary: accepted_maximum,
            warning_boundary: warning_maximum,
            review_boundary: review_maximum,
        })
    }

    pub const fn direction(self) -> ThresholdDirection {
        self.direction
    }

    pub fn classify(self, value: f64) -> Result<MetricState, A2dError> {
        if !value.is_finite() {
            return Err(validation_error(
                "QUALITY_METRIC_NON_FINITE",
                "quality metric value must be finite",
            ));
        }
        Ok(match self.direction {
            ThresholdDirection::HigherIsBetter => {
                if value >= self.accepted_boundary {
                    MetricState::Accepted
                } else if value >= self.warning_boundary {
                    MetricState::Warning
                } else if value >= self.review_boundary {
                    MetricState::NeedsReview
                } else {
                    MetricState::Rejected
                }
            }
            ThresholdDirection::LowerIsBetter => {
                if value <= self.accepted_boundary {
                    MetricState::Accepted
                } else if value <= self.warning_boundary {
                    MetricState::Warning
                } else if value <= self.review_boundary {
                    MetricState::NeedsReview
                } else {
                    MetricState::Rejected
                }
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandThresholds {
    accepted_minimum: f64,
    accepted_maximum: f64,
    warning_minimum: f64,
    warning_maximum: f64,
    review_minimum: f64,
    review_maximum: f64,
}

impl BandThresholds {
    pub fn new(
        accepted_range: [f64; 2],
        warning_range: [f64; 2],
        review_range: [f64; 2],
    ) -> Result<Self, A2dError> {
        let [accepted_minimum, accepted_maximum] = accepted_range;
        let [warning_minimum, warning_maximum] = warning_range;
        let [review_minimum, review_maximum] = review_range;
        validate_finite_thresholds([
            accepted_minimum,
            accepted_maximum,
            warning_minimum,
            warning_maximum,
            review_minimum,
            review_maximum,
        ])?;
        if !(review_minimum <= warning_minimum
            && warning_minimum <= accepted_minimum
            && accepted_minimum < accepted_maximum
            && accepted_maximum <= warning_maximum
            && warning_maximum <= review_maximum)
        {
            return Err(validation_error(
                "QUALITY_BAND_THRESHOLDS_INVALID",
                "band thresholds must be nested review ⊇ warning ⊇ accepted",
            ));
        }
        Ok(Self {
            accepted_minimum,
            accepted_maximum,
            warning_minimum,
            warning_maximum,
            review_minimum,
            review_maximum,
        })
    }

    pub fn classify(self, value: f64) -> Result<MetricState, A2dError> {
        if !value.is_finite() {
            return Err(validation_error(
                "QUALITY_METRIC_NON_FINITE",
                "quality metric value must be finite",
            ));
        }
        Ok(
            if (self.accepted_minimum..=self.accepted_maximum).contains(&value) {
                MetricState::Accepted
            } else if (self.warning_minimum..=self.warning_maximum).contains(&value) {
                MetricState::Warning
            } else if (self.review_minimum..=self.review_maximum).contains(&value) {
                MetricState::NeedsReview
            } else {
                MetricState::Rejected
            },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnderexposurePolicy {
    pub mean_luminance: ScalarThresholds,
    pub dark_fraction: ScalarThresholds,
}

impl UnderexposurePolicy {
    pub fn new(
        mean_luminance: ScalarThresholds,
        dark_fraction: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(mean_luminance, ThresholdDirection::HigherIsBetter)?;
        require_direction(dark_fraction, ThresholdDirection::LowerIsBetter)?;
        Ok(Self {
            mean_luminance,
            dark_fraction,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverexposurePolicy {
    pub mean_luminance: ScalarThresholds,
    pub highlight_fraction: ScalarThresholds,
}

impl OverexposurePolicy {
    pub fn new(
        mean_luminance: ScalarThresholds,
        highlight_fraction: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(mean_luminance, ThresholdDirection::LowerIsBetter)?;
        require_direction(highlight_fraction, ThresholdDirection::LowerIsBetter)?;
        Ok(Self {
            mean_luminance,
            highlight_fraction,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlarePolicy {
    pub global_highlight_fraction: ScalarThresholds,
    pub maximum_tile_highlight_fraction: ScalarThresholds,
}

impl GlarePolicy {
    pub fn new(
        global_highlight_fraction: ScalarThresholds,
        maximum_tile_highlight_fraction: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(global_highlight_fraction, ThresholdDirection::LowerIsBetter)?;
        require_direction(
            maximum_tile_highlight_fraction,
            ThresholdDirection::LowerIsBetter,
        )?;
        Ok(Self {
            global_highlight_fraction,
            maximum_tile_highlight_fraction,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramingPolicy {
    pub page_area_fraction: BandThresholds,
    pub minimum_border_margin_fraction: ScalarThresholds,
    pub center_offset_fraction: ScalarThresholds,
}

impl FramingPolicy {
    pub fn new(
        page_area_fraction: BandThresholds,
        minimum_border_margin_fraction: ScalarThresholds,
        center_offset_fraction: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(
            minimum_border_margin_fraction,
            ThresholdDirection::HigherIsBetter,
        )?;
        require_direction(center_offset_fraction, ThresholdDirection::LowerIsBetter)?;
        Ok(Self {
            page_area_fraction,
            minimum_border_margin_fraction,
            center_offset_fraction,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerConfidencePolicy {
    pub minimum_decision_margin: ScalarThresholds,
    pub maximum_hamming_errors: ScalarThresholds,
    pub unexpected_tag_count: ScalarThresholds,
}

impl MarkerConfidencePolicy {
    pub fn new(
        minimum_decision_margin: ScalarThresholds,
        maximum_hamming_errors: ScalarThresholds,
        unexpected_tag_count: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(minimum_decision_margin, ThresholdDirection::HigherIsBetter)?;
        require_direction(maximum_hamming_errors, ThresholdDirection::LowerIsBetter)?;
        require_direction(unexpected_tag_count, ThresholdDirection::LowerIsBetter)?;
        Ok(Self {
            minimum_decision_margin,
            maximum_hamming_errors,
            unexpected_tag_count,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectivePolicy {
    pub maximum_to_minimum_edge_ratio: ScalarThresholds,
    pub opposing_edge_imbalance_ratio: ScalarThresholds,
    pub diagonal_imbalance_ratio: ScalarThresholds,
    pub quad_to_bounding_box_area_ratio: ScalarThresholds,
}

impl PerspectivePolicy {
    pub fn new(
        maximum_to_minimum_edge_ratio: ScalarThresholds,
        opposing_edge_imbalance_ratio: ScalarThresholds,
        diagonal_imbalance_ratio: ScalarThresholds,
        quad_to_bounding_box_area_ratio: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        for threshold in [
            maximum_to_minimum_edge_ratio,
            opposing_edge_imbalance_ratio,
            diagonal_imbalance_ratio,
        ] {
            require_direction(threshold, ThresholdDirection::LowerIsBetter)?;
        }
        require_direction(
            quad_to_bounding_box_area_ratio,
            ThresholdDirection::HigherIsBetter,
        )?;
        Ok(Self {
            maximum_to_minimum_edge_ratio,
            opposing_edge_imbalance_ratio,
            diagonal_imbalance_ratio,
            quad_to_bounding_box_area_ratio,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolutionPolicy {
    pub source_minimum_pixels_per_mm: ScalarThresholds,
    pub output_minimum_pixels_per_mm: ScalarThresholds,
}

impl ResolutionPolicy {
    pub fn new(
        source_minimum_pixels_per_mm: ScalarThresholds,
        output_minimum_pixels_per_mm: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(
            source_minimum_pixels_per_mm,
            ThresholdDirection::HigherIsBetter,
        )?;
        require_direction(
            output_minimum_pixels_per_mm,
            ThresholdDirection::HigherIsBetter,
        )?;
        Ok(Self {
            source_minimum_pixels_per_mm,
            output_minimum_pixels_per_mm,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvaturePolicy {
    pub maximum_normalized_deviation: ScalarThresholds,
    pub rms_normalized_deviation: ScalarThresholds,
}

impl CurvaturePolicy {
    pub fn new(
        maximum_normalized_deviation: ScalarThresholds,
        rms_normalized_deviation: ScalarThresholds,
    ) -> Result<Self, A2dError> {
        require_direction(
            maximum_normalized_deviation,
            ThresholdDirection::LowerIsBetter,
        )?;
        require_direction(rms_normalized_deviation, ThresholdDirection::LowerIsBetter)?;
        Ok(Self {
            maximum_normalized_deviation,
            rms_normalized_deviation,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QualityThresholdSet {
    pub focus: ScalarThresholds,
    pub underexposure: UnderexposurePolicy,
    pub overexposure: OverexposurePolicy,
    pub glare: GlarePolicy,
    pub framing: FramingPolicy,
    pub marker_confidence: MarkerConfidencePolicy,
    pub perspective: PerspectivePolicy,
    pub resolution: ResolutionPolicy,
    pub curvature: CurvaturePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualityRequirements {
    pub focus: bool,
    pub exposure: bool,
    pub glare: bool,
    pub framing: bool,
    pub marker_confidence: bool,
    pub perspective: bool,
    pub resolution: bool,
    pub curvature: bool,
}

impl QualityRequirements {
    pub const fn all_required() -> Self {
        Self {
            focus: true,
            exposure: true,
            glare: true,
            framing: true,
            marker_confidence: true,
            perspective: true,
            resolution: true,
            curvature: true,
        }
    }

    pub const fn none_required() -> Self {
        Self {
            focus: false,
            exposure: false,
            glare: false,
            framing: false,
            marker_confidence: false,
            perspective: false,
            resolution: false,
            curvature: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QualityPolicy {
    version: u32,
    luminance_measurement: LuminanceMeasurementConfig,
    thresholds: QualityThresholdSet,
    requirements: QualityRequirements,
}

impl QualityPolicy {
    pub fn new(
        version: u32,
        luminance_measurement: LuminanceMeasurementConfig,
        thresholds: QualityThresholdSet,
        requirements: QualityRequirements,
    ) -> Result<Self, A2dError> {
        if version == 0 {
            return Err(validation_error(
                "QUALITY_POLICY_VERSION_INVALID",
                "quality policy version must be greater than zero",
            ));
        }
        require_direction(thresholds.focus, ThresholdDirection::HigherIsBetter)?;
        Ok(Self {
            version,
            luminance_measurement,
            thresholds,
            requirements,
        })
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn luminance_measurement(self) -> LuminanceMeasurementConfig {
        self.luminance_measurement
    }

    pub const fn requirements(self) -> QualityRequirements {
        self.requirements
    }

    pub fn assess(self, measurements: &QualityMeasurements) -> Result<QualityAssessment, A2dError> {
        let focus = classify_optional(
            measurements.focus.as_ref(),
            self.requirements.focus,
            |metric| self.thresholds.focus.classify(metric.laplacian_variance),
        )?;
        let underexposure = classify_optional(
            measurements.exposure.as_ref(),
            self.requirements.exposure,
            |metric| {
                combine([
                    self.thresholds
                        .underexposure
                        .mean_luminance
                        .classify(metric.mean_luminance)?,
                    self.thresholds
                        .underexposure
                        .dark_fraction
                        .classify(metric.dark_fraction)?,
                ])
            },
        )?;
        let overexposure = classify_optional(
            measurements.exposure.as_ref(),
            self.requirements.exposure,
            |metric| {
                combine([
                    self.thresholds
                        .overexposure
                        .mean_luminance
                        .classify(metric.mean_luminance)?,
                    self.thresholds
                        .overexposure
                        .highlight_fraction
                        .classify(metric.highlight_fraction)?,
                ])
            },
        )?;
        let glare = classify_optional(
            measurements.glare.as_ref(),
            self.requirements.glare,
            |metric| {
                combine([
                    self.thresholds
                        .glare
                        .global_highlight_fraction
                        .classify(metric.highlight_fraction)?,
                    self.thresholds
                        .glare
                        .maximum_tile_highlight_fraction
                        .classify(metric.max_tile_highlight_fraction)?,
                ])
            },
        )?;
        let framing = classify_optional(
            measurements.framing.as_ref(),
            self.requirements.framing,
            |metric| {
                combine([
                    self.thresholds
                        .framing
                        .page_area_fraction
                        .classify(metric.page_area_fraction)?,
                    self.thresholds
                        .framing
                        .minimum_border_margin_fraction
                        .classify(metric.minimum_border_margin_fraction)?,
                    self.thresholds
                        .framing
                        .center_offset_fraction
                        .classify(metric.center_offset_fraction)?,
                ])
            },
        )?;
        let marker_confidence = classify_optional(
            measurements.marker_confidence.as_ref(),
            self.requirements.marker_confidence,
            |metric| {
                combine([
                    self.thresholds
                        .marker_confidence
                        .minimum_decision_margin
                        .classify(metric.minimum_decision_margin)?,
                    self.thresholds
                        .marker_confidence
                        .maximum_hamming_errors
                        .classify(f64::from(metric.maximum_hamming_errors))?,
                    self.thresholds
                        .marker_confidence
                        .unexpected_tag_count
                        .classify(f64::from(metric.unexpected_tag_count))?,
                ])
            },
        )?;
        let perspective = classify_optional(
            measurements.perspective.as_ref(),
            self.requirements.perspective,
            |metric| {
                combine([
                    self.thresholds
                        .perspective
                        .maximum_to_minimum_edge_ratio
                        .classify(metric.maximum_to_minimum_edge_ratio)?,
                    self.thresholds
                        .perspective
                        .opposing_edge_imbalance_ratio
                        .classify(metric.opposing_edge_imbalance_ratio)?,
                    self.thresholds
                        .perspective
                        .diagonal_imbalance_ratio
                        .classify(metric.diagonal_imbalance_ratio)?,
                    self.thresholds
                        .perspective
                        .quad_to_bounding_box_area_ratio
                        .classify(metric.quad_to_bounding_box_area_ratio)?,
                ])
            },
        )?;
        let resolution = classify_optional(
            measurements.resolution.as_ref(),
            self.requirements.resolution,
            |metric| {
                combine([
                    self.thresholds
                        .resolution
                        .source_minimum_pixels_per_mm
                        .classify(metric.source_minimum_pixels_per_mm)?,
                    self.thresholds
                        .resolution
                        .output_minimum_pixels_per_mm
                        .classify(metric.output_minimum_pixels_per_mm)?,
                ])
            },
        )?;
        let curvature = classify_optional(
            measurements.curvature.as_ref(),
            self.requirements.curvature,
            |metric| {
                combine([
                    self.thresholds
                        .curvature
                        .maximum_normalized_deviation
                        .classify(metric.maximum_normalized_deviation)?,
                    self.thresholds
                        .curvature
                        .rms_normalized_deviation
                        .classify(metric.rms_normalized_deviation)?,
                ])
            },
        )?;
        let states = QualityMetricStates {
            focus,
            underexposure,
            overexposure,
            glare,
            framing,
            marker_confidence,
            perspective,
            resolution,
            curvature,
        };
        Ok(QualityAssessment {
            policy_version: self.version,
            overall: overall_state(states),
            states,
            measurements: measurements.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualityMetricStates {
    pub focus: MetricState,
    pub underexposure: MetricState,
    pub overexposure: MetricState,
    pub glare: MetricState,
    pub framing: MetricState,
    pub marker_confidence: MetricState,
    pub perspective: MetricState,
    pub resolution: MetricState,
    pub curvature: MetricState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QualityAssessment {
    pub policy_version: u32,
    pub overall: QualityState,
    pub states: QualityMetricStates,
    pub measurements: QualityMeasurements,
}

fn classify_optional<T>(
    value: Option<&T>,
    required: bool,
    classify: impl FnOnce(&T) -> Result<MetricState, A2dError>,
) -> Result<MetricState, A2dError> {
    match value {
        Some(value) => classify(value),
        None if required => Ok(MetricState::NeedsReview),
        None => Ok(MetricState::Unavailable),
    }
}

fn combine<const N: usize>(states: [MetricState; N]) -> Result<MetricState, A2dError> {
    states
        .into_iter()
        .max_by_key(|state| state.rank())
        .ok_or_else(|| {
            validation_error(
                "QUALITY_STATE_SET_EMPTY",
                "quality classification requires at least one state",
            )
        })
}

fn overall_state(states: QualityMetricStates) -> QualityState {
    let values = [
        states.focus,
        states.underexposure,
        states.overexposure,
        states.glare,
        states.framing,
        states.marker_confidence,
        states.perspective,
        states.resolution,
        states.curvature,
    ];
    let mut evaluated = false;
    let mut worst = MetricState::Accepted;
    for state in values {
        if state == MetricState::Unavailable {
            continue;
        }
        evaluated = true;
        if state.rank() > worst.rank() {
            worst = state;
        }
    }
    if !evaluated {
        return QualityState::NeedsReview;
    }
    match worst {
        MetricState::Accepted => QualityState::Accepted,
        MetricState::Warning => QualityState::AcceptedWithWarnings,
        MetricState::NeedsReview | MetricState::Unavailable => QualityState::NeedsReview,
        MetricState::Rejected => QualityState::Rejected,
    }
}

fn require_direction(
    thresholds: ScalarThresholds,
    expected: ThresholdDirection,
) -> Result<(), A2dError> {
    if thresholds.direction() != expected {
        return Err(validation_error(
            "QUALITY_THRESHOLD_DIRECTION_INVALID",
            format!(
                "quality threshold direction {:?} does not match required {:?}",
                thresholds.direction(),
                expected
            ),
        ));
    }
    Ok(())
}

fn validate_finite_thresholds<const N: usize>(values: [f64; N]) -> Result<(), A2dError> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(validation_error(
            "QUALITY_THRESHOLDS_NON_FINITE",
            "quality thresholds must be finite",
        ));
    }
    Ok(())
}

fn validate_source_and_quad(
    source_width: u32,
    source_height: u32,
    quad: ImageQuad,
) -> Result<(), A2dError> {
    if source_width < 2 || source_height < 2 {
        return Err(validation_error(
            "QUALITY_SOURCE_DIMENSIONS_INVALID",
            format!(
                "quality source dimensions must be at least 2x2, got {source_width}x{source_height}"
            ),
        ));
    }
    quad.validate("quality page")?;
    let max_x = f64::from(source_width - 1);
    let max_y = f64::from(source_height - 1);
    if quad.points().iter().any(|point| {
        point.x < -BOUNDS_EPSILON
            || point.y < -BOUNDS_EPSILON
            || point.x > max_x + BOUNDS_EPSILON
            || point.y > max_y + BOUNDS_EPSILON
    }) {
        return Err(validation_error(
            "QUALITY_PAGE_QUAD_OUT_OF_BOUNDS",
            format!("quality page quadrilateral extends outside {source_width}x{source_height}"),
        ));
    }
    Ok(())
}

fn edge_points(quad: ImageQuad, edge: PageEdge) -> (ImagePoint, ImagePoint) {
    match edge {
        PageEdge::Top => (quad.top_left, quad.top_right),
        PageEdge::Right => (quad.top_right, quad.bottom_right),
        PageEdge::Bottom => (quad.bottom_right, quad.bottom_left),
        PageEdge::Left => (quad.bottom_left, quad.top_left),
    }
}

fn interpolate(start: ImagePoint, end: ImagePoint, position: f64) -> ImagePoint {
    ImagePoint {
        x: start.x + (end.x - start.x) * position,
        y: start.y + (end.y - start.y) * position,
    }
}

fn distance(first: ImagePoint, second: ImagePoint) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    (dx * dx + dy * dy).sqrt()
}

fn symmetric_ratio(first: f64, second: f64) -> f64 {
    first.max(second) / first.min(second)
}

#[cfg(test)]
mod tests {
    use a2d_domain::LayoutId;
    use a2d_layout::{
        CalibrationMark, ContentStyle, MarkerPlacement, MarkerRole,
        geometry::{PhysicalPoint, PhysicalRect, PhysicalSize},
    };

    use crate::{
        ImageLimits, ImageRotation, MarkerDetection, MarkerFamily, PageOrientation, ResolvedMarker,
    };

    use super::*;

    fn gray_frame<'a>(width: u32, height: u32, bytes: &'a [u8]) -> GrayFrame<'a> {
        GrayFrame::new(
            width,
            height,
            width as usize,
            ImageRotation::Degrees0,
            bytes,
            ImageLimits::new(u64::from(width) * u64::from(height)).unwrap(),
        )
        .unwrap()
    }

    fn measurement_config() -> LuminanceMeasurementConfig {
        LuminanceMeasurementConfig::new(20, 240, 2, 2).unwrap()
    }

    fn point(x: f64, y: f64) -> ImagePoint {
        ImagePoint { x, y }
    }

    fn quad(width: f64, height: f64) -> ImageQuad {
        ImageQuad::new(
            point(0.0, 0.0),
            point(width, 0.0),
            point(width, height),
            point(0.0, height),
        )
    }

    #[test]
    fn flat_frame_has_zero_focus_variance_and_uniform_exposure() {
        let bytes = vec![100_u8; 25];
        let metrics = measure_gray_quality(gray_frame(5, 5, &bytes), measurement_config()).unwrap();
        assert_eq!(metrics.focus.unwrap().laplacian_variance, 0.0);
        assert_eq!(metrics.exposure.mean_luminance, 100.0);
        assert_eq!(metrics.exposure.dark_fraction, 0.0);
        assert_eq!(metrics.exposure.highlight_fraction, 0.0);
    }

    #[test]
    fn edge_pattern_has_nonzero_focus_variance() {
        let bytes = [
            0, 0, 0, 255, 255, 0, 0, 0, 255, 255, 0, 0, 0, 255, 255, 0, 0, 0, 255, 255, 0, 0, 0,
            255, 255,
        ];
        let metrics = measure_gray_quality(gray_frame(5, 5, &bytes), measurement_config()).unwrap();
        assert!(metrics.focus.unwrap().laplacian_variance > 0.0);
    }

    #[test]
    fn small_frame_reports_focus_unavailable_without_fabricating_zero() {
        let bytes = [50_u8; 4];
        let metrics = measure_gray_quality(gray_frame(2, 2, &bytes), measurement_config()).unwrap();
        assert_eq!(metrics.focus, None);
        assert_eq!(metrics.exposure.mean_luminance, 50.0);
    }

    #[test]
    fn localized_highlights_are_visible_in_tile_metric() {
        let bytes = [255, 255, 0, 0, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let metrics = measure_gray_quality(gray_frame(4, 4, &bytes), measurement_config()).unwrap();
        assert_eq!(metrics.exposure.highlight_fraction, 0.25);
        assert_eq!(metrics.glare.max_tile_highlight_fraction, 1.0);
    }

    #[test]
    fn framing_and_perspective_metrics_are_deterministic() {
        let page = ImageQuad::new(
            point(10.0, 20.0),
            point(90.0, 20.0),
            point(90.0, 180.0),
            point(10.0, 180.0),
        );
        let framing = measure_framing(101, 201, page).unwrap();
        assert!((framing.page_area_fraction - 0.64).abs() < 1.0e-12);
        assert!((framing.minimum_border_margin_fraction - 0.1).abs() < 1.0e-12);
        assert!(framing.center_offset_fraction < 1.0e-12);
        let perspective = measure_perspective(page).unwrap();
        assert_eq!(perspective.opposing_edge_imbalance_ratio, 1.0);
        assert_eq!(perspective.diagonal_imbalance_ratio, 1.0);
        assert_eq!(perspective.quad_to_bounding_box_area_ratio, 1.0);
    }

    fn marker(id: u32, role: MarkerRole, margin: f32, hamming: u8) -> ResolvedMarker {
        ResolvedMarker {
            role,
            detection: MarkerDetection {
                family: MarkerFamily::TagStandard41h12,
                id,
                hamming_errors: hamming,
                decision_margin: margin,
                center: point(f64::from(id), f64::from(id)),
                corners: [point(0.0, 0.0); 4],
            },
        }
    }

    #[test]
    fn marker_confidence_preserves_worst_and_mean_values() {
        let markers = ResolvedPageMarkers {
            markers: [
                marker(1, MarkerRole::TopLeft, 40.0, 0),
                marker(2, MarkerRole::TopRight, 30.0, 1),
                marker(3, MarkerRole::BottomLeft, 20.0, 0),
                marker(4, MarkerRole::BottomRight, 10.0, 2),
            ],
            orientation: PageOrientation::Degrees0,
            unexpected_tag_ids: vec![99],
        };
        let metrics = measure_marker_confidence(&markers).unwrap();
        assert_eq!(metrics.minimum_decision_margin, 10.0);
        assert_eq!(metrics.mean_decision_margin, 25.0);
        assert_eq!(metrics.maximum_hamming_errors, 2);
        assert_eq!(metrics.unexpected_tag_count, 1);
    }

    fn layout() -> PageLayout {
        let marker = |role, x, y| MarkerPlacement {
            role,
            rect: PhysicalRect::new(x, y, 10.0, 10.0),
        };
        PageLayout {
            id: LayoutId::parse("QUALITY-TEST").unwrap(),
            physical_size: PhysicalSize::new(100.0, 200.0),
            safe_margin_mm: 0.0,
            quiet_zone_mm: 1.0,
            content_rect: PhysicalRect::new(20.0, 20.0, 60.0, 140.0),
            markers: [
                marker(MarkerRole::TopLeft, 5.0, 5.0),
                marker(MarkerRole::TopRight, 85.0, 5.0),
                marker(MarkerRole::BottomLeft, 5.0, 185.0),
                marker(MarkerRole::BottomRight, 85.0, 185.0),
            ],
            qr_rect: PhysicalRect::new(42.5, 170.0, 15.0, 15.0),
            visible_page_number_rect: None,
            calibration: CalibrationMark {
                rect: PhysicalRect {
                    origin: PhysicalPoint::new(40.0, 2.0),
                    size: PhysicalSize::new(20.0, 1.0),
                },
                reference_length_mm: 20.0,
            },
            content_style: ContentStyle::Blank,
        }
    }

    #[test]
    fn resolution_reports_conservative_source_and_output_density() {
        let output = RectifiedImageSize::new(
            1_001,
            2_001,
            crate::RectificationLimits::new(3_000_000, 9_000_000).unwrap(),
        )
        .unwrap();
        let metrics =
            measure_effective_resolution(quad(500.0, 1_000.0), output, &layout()).unwrap();
        assert_eq!(metrics.source_minimum_pixels_per_mm, 5.0);
        assert_eq!(metrics.output_minimum_pixels_per_mm, 10.0);
    }

    #[test]
    fn curvature_is_unavailable_without_probes_and_measured_with_probes() {
        assert_eq!(measure_curvature(quad(100.0, 200.0), &[]).unwrap(), None);
        let probes = [
            EdgeProbe {
                edge: PageEdge::Top,
                normalized_position: 0.5,
                observed: point(50.0, 2.0),
            },
            EdgeProbe {
                edge: PageEdge::Bottom,
                normalized_position: 0.5,
                observed: point(50.0, 198.0),
            },
        ];
        let metrics = measure_curvature(quad(100.0, 200.0), &probes)
            .unwrap()
            .unwrap();
        assert!((metrics.maximum_normalized_deviation - 0.02).abs() < 1.0e-12);
        assert_eq!(metrics.sample_count, 2);
    }

    fn scalar_higher() -> ScalarThresholds {
        ScalarThresholds::higher_is_better(10.0, 5.0, 1.0).unwrap()
    }

    fn scalar_lower() -> ScalarThresholds {
        ScalarThresholds::lower_is_better(0.1, 0.2, 0.4).unwrap()
    }

    fn policy(requirements: QualityRequirements) -> QualityPolicy {
        let thresholds = QualityThresholdSet {
            focus: scalar_higher(),
            underexposure: UnderexposurePolicy::new(scalar_higher(), scalar_lower()).unwrap(),
            overexposure: OverexposurePolicy::new(
                ScalarThresholds::lower_is_better(200.0, 220.0, 240.0).unwrap(),
                scalar_lower(),
            )
            .unwrap(),
            glare: GlarePolicy::new(scalar_lower(), scalar_lower()).unwrap(),
            framing: FramingPolicy::new(
                BandThresholds::new([0.5, 0.8], [0.4, 0.9], [0.2, 0.98]).unwrap(),
                ScalarThresholds::higher_is_better(0.05, 0.02, 0.005).unwrap(),
                scalar_lower(),
            )
            .unwrap(),
            marker_confidence: MarkerConfidencePolicy::new(
                scalar_higher(),
                ScalarThresholds::lower_is_better(0.0, 1.0, 2.0).unwrap(),
                ScalarThresholds::lower_is_better(0.0, 1.0, 2.0).unwrap(),
            )
            .unwrap(),
            perspective: PerspectivePolicy::new(
                ScalarThresholds::lower_is_better(1.2, 1.5, 2.0).unwrap(),
                ScalarThresholds::lower_is_better(1.1, 1.3, 1.8).unwrap(),
                ScalarThresholds::lower_is_better(1.1, 1.3, 1.8).unwrap(),
                ScalarThresholds::higher_is_better(0.9, 0.8, 0.6).unwrap(),
            )
            .unwrap(),
            resolution: ResolutionPolicy::new(scalar_higher(), scalar_higher()).unwrap(),
            curvature: CurvaturePolicy::new(scalar_lower(), scalar_lower()).unwrap(),
        };
        QualityPolicy::new(7, measurement_config(), thresholds, requirements).unwrap()
    }

    #[test]
    fn missing_required_metric_needs_review_and_optional_metric_stays_unavailable() {
        let requirements = QualityRequirements {
            focus: true,
            ..QualityRequirements::none_required()
        };
        let assessment = policy(requirements)
            .assess(&QualityMeasurements::empty())
            .unwrap();
        assert_eq!(assessment.states.focus, MetricState::NeedsReview);
        assert_eq!(assessment.states.curvature, MetricState::Unavailable);
        assert_eq!(assessment.overall, QualityState::NeedsReview);
    }

    #[test]
    fn empty_optional_measurements_never_fabricate_accepted_state() {
        let assessment = policy(QualityRequirements::none_required())
            .assess(&QualityMeasurements::empty())
            .unwrap();
        assert_eq!(assessment.overall, QualityState::NeedsReview);
    }

    #[test]
    fn assessment_uses_worst_metric_and_preserves_policy_version() {
        let measurements = QualityMeasurements {
            focus: Some(FocusMetrics {
                laplacian_variance: 7.0,
                interior_sample_count: 9,
            }),
            exposure: Some(ExposureMetrics {
                mean_luminance: 100.0,
                luminance_standard_deviation: 20.0,
                dark_fraction: 0.05,
                highlight_fraction: 0.05,
            }),
            glare: None,
            framing: None,
            marker_confidence: None,
            perspective: None,
            resolution: None,
            curvature: None,
        };
        let assessment = policy(QualityRequirements::none_required())
            .assess(&measurements)
            .unwrap();
        assert_eq!(assessment.policy_version, 7);
        assert_eq!(assessment.states.focus, MetricState::Warning);
        assert_eq!(assessment.overall, QualityState::AcceptedWithWarnings);
    }

    #[test]
    fn threshold_constructors_reject_invalid_order_and_direction() {
        let err = ScalarThresholds::higher_is_better(1.0, 2.0, 0.0).unwrap_err();
        assert_eq!(err.code.to_string(), "QUALITY_HIGHER_THRESHOLDS_INVALID");
        let err = UnderexposurePolicy::new(scalar_lower(), scalar_lower()).unwrap_err();
        assert_eq!(err.code.to_string(), "QUALITY_THRESHOLD_DIRECTION_INVALID");
        let err = QualityPolicy::new(
            0,
            measurement_config(),
            QualityThresholdSet {
                focus: scalar_higher(),
                underexposure: UnderexposurePolicy::new(scalar_higher(), scalar_lower()).unwrap(),
                overexposure: OverexposurePolicy::new(
                    ScalarThresholds::lower_is_better(200.0, 220.0, 240.0).unwrap(),
                    scalar_lower(),
                )
                .unwrap(),
                glare: GlarePolicy::new(scalar_lower(), scalar_lower()).unwrap(),
                framing: FramingPolicy::new(
                    BandThresholds::new([0.5, 0.8], [0.4, 0.9], [0.2, 0.98]).unwrap(),
                    ScalarThresholds::higher_is_better(0.05, 0.02, 0.005).unwrap(),
                    scalar_lower(),
                )
                .unwrap(),
                marker_confidence: MarkerConfidencePolicy::new(
                    scalar_higher(),
                    ScalarThresholds::lower_is_better(0.0, 1.0, 2.0).unwrap(),
                    ScalarThresholds::lower_is_better(0.0, 1.0, 2.0).unwrap(),
                )
                .unwrap(),
                perspective: PerspectivePolicy::new(
                    ScalarThresholds::lower_is_better(1.2, 1.5, 2.0).unwrap(),
                    ScalarThresholds::lower_is_better(1.1, 1.3, 1.8).unwrap(),
                    ScalarThresholds::lower_is_better(1.1, 1.3, 1.8).unwrap(),
                    ScalarThresholds::higher_is_better(0.9, 0.8, 0.6).unwrap(),
                )
                .unwrap(),
                resolution: ResolutionPolicy::new(scalar_higher(), scalar_higher()).unwrap(),
                curvature: CurvaturePolicy::new(scalar_lower(), scalar_lower()).unwrap(),
            },
            QualityRequirements::none_required(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "QUALITY_POLICY_VERSION_INVALID");
    }
}
