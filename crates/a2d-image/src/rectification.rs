use a2d_domain::A2dError;
use a2d_layout::{MarkerRole, PageLayout};

use crate::{
    detection::{ImagePoint, ResolvedPageMarkers},
    encoded::{OwnedGrayImage, OwnedRgbImage},
    error::{processing_error, validation_error},
    input::{GrayFrame, ImageRotation},
};

const GEOMETRY_EPSILON: f64 = 1.0e-9;
const MIN_PIVOT_RATIO: f64 = 1.0e-12;
const BOUNDS_EPSILON: f64 = 1.0e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageQuad {
    pub top_left: ImagePoint,
    pub top_right: ImagePoint,
    pub bottom_right: ImagePoint,
    pub bottom_left: ImagePoint,
}

impl ImageQuad {
    pub const fn new(
        top_left: ImagePoint,
        top_right: ImagePoint,
        bottom_right: ImagePoint,
        bottom_left: ImagePoint,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn points(self) -> [ImagePoint; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    pub fn signed_area(self) -> f64 {
        let points = self.points();
        let mut twice_area = 0.0;
        for index in 0..4 {
            let current = points[index];
            let next = points[(index + 1) % 4];
            twice_area += current.x * next.y - current.y * next.x;
        }
        twice_area * 0.5
    }

    pub fn validate(self, label: &'static str) -> Result<(), A2dError> {
        let points = self.points();
        if points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(validation_error(
                "HOMOGRAPHY_QUAD_NON_FINITE",
                format!("{label} quadrilateral contains a non-finite point"),
            ));
        }

        for index in 0..4 {
            let edge_length_squared = distance_squared(points[index], points[(index + 1) % 4]);
            if edge_length_squared <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
                return Err(validation_error(
                    "HOMOGRAPHY_QUAD_DEGENERATE",
                    format!("{label} quadrilateral has a zero-length edge"),
                ));
            }
        }

        if segments_intersect(points[0], points[1], points[2], points[3])
            || segments_intersect(points[1], points[2], points[3], points[0])
        {
            return Err(validation_error(
                "HOMOGRAPHY_QUAD_SELF_INTERSECTING",
                format!("{label} quadrilateral is self-intersecting"),
            ));
        }

        let mut expected_sign = 0.0;
        for index in 0..4 {
            let cross = cross_product(
                points[index],
                points[(index + 1) % 4],
                points[(index + 2) % 4],
            );
            if cross.abs() <= GEOMETRY_EPSILON {
                return Err(validation_error(
                    "HOMOGRAPHY_QUAD_DEGENERATE",
                    format!("{label} quadrilateral has collinear adjacent edges"),
                ));
            }
            if expected_sign == 0.0 {
                expected_sign = cross.signum();
            } else if cross.signum() != expected_sign {
                return Err(validation_error(
                    "HOMOGRAPHY_QUAD_NON_CONVEX",
                    format!("{label} quadrilateral is not convex"),
                ));
            }
        }

        if self.signed_area().abs() <= GEOMETRY_EPSILON {
            return Err(validation_error(
                "HOMOGRAPHY_QUAD_DEGENERATE",
                format!("{label} quadrilateral has negligible area"),
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RectificationLimits {
    max_output_pixels: u64,
    max_output_bytes: u64,
}

impl RectificationLimits {
    pub fn new(max_output_pixels: u64, max_output_bytes: u64) -> Result<Self, A2dError> {
        if max_output_pixels == 0 {
            return Err(validation_error(
                "RECTIFICATION_PIXEL_LIMIT_INVALID",
                "maximum rectified pixel count must be greater than zero",
            ));
        }
        if max_output_bytes == 0 {
            return Err(validation_error(
                "RECTIFICATION_BYTE_LIMIT_INVALID",
                "maximum rectified byte count must be greater than zero",
            ));
        }
        Ok(Self {
            max_output_pixels,
            max_output_bytes,
        })
    }

    pub const fn max_output_pixels(self) -> u64 {
        self.max_output_pixels
    }

    pub const fn max_output_bytes(self) -> u64 {
        self.max_output_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RectifiedImageSize {
    width: u32,
    height: u32,
    pixel_count: u64,
    rgb_byte_count: u64,
}

impl RectifiedImageSize {
    pub fn new(width: u32, height: u32, limits: RectificationLimits) -> Result<Self, A2dError> {
        if width < 2 || height < 2 {
            return Err(validation_error(
                "RECTIFICATION_DIMENSIONS_INVALID",
                format!("rectified dimensions must be at least 2x2, got {width}x{height}"),
            ));
        }
        let pixel_count = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                validation_error(
                    "RECTIFICATION_PIXEL_COUNT_OVERFLOW",
                    format!("rectified pixel count overflow for {width}x{height}"),
                )
            })?;
        if pixel_count > limits.max_output_pixels() {
            return Err(validation_error(
                "RECTIFICATION_PIXEL_LIMIT_EXCEEDED",
                format!(
                    "rectified output has {pixel_count} pixels, limit is {}",
                    limits.max_output_pixels()
                ),
            ));
        }
        let rgb_byte_count = pixel_count.checked_mul(3).ok_or_else(|| {
            validation_error(
                "RECTIFICATION_BYTE_COUNT_OVERFLOW",
                format!("rectified RGB byte count overflow for {width}x{height}"),
            )
        })?;
        if rgb_byte_count > limits.max_output_bytes() {
            return Err(validation_error(
                "RECTIFICATION_BYTE_LIMIT_EXCEEDED",
                format!(
                    "rectified RGB output requires {rgb_byte_count} bytes, limit is {}",
                    limits.max_output_bytes()
                ),
            ));
        }
        usize::try_from(rgb_byte_count).map_err(|_| {
            validation_error(
                "RECTIFICATION_OUTPUT_UNSUPPORTED",
                "rectified output does not fit this platform's address space",
            )
        })?;

        Ok(Self {
            width,
            height,
            pixel_count,
            rgb_byte_count,
        })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub const fn pixel_count(self) -> u64 {
        self.pixel_count
    }

    pub const fn rgb_byte_count(self) -> u64 {
        self.rgb_byte_count
    }

    fn destination_quad(self) -> ImageQuad {
        let max_x = f64::from(self.width - 1);
        let max_y = f64::from(self.height - 1);
        ImageQuad::new(
            ImagePoint { x: 0.0, y: 0.0 },
            ImagePoint { x: max_x, y: 0.0 },
            ImagePoint { x: max_x, y: max_y },
            ImagePoint { x: 0.0, y: max_y },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectiveTransform {
    source_to_destination: [[f64; 3]; 3],
    destination_to_source: [[f64; 3]; 3],
    pivot_ratio: f64,
}

impl ProjectiveTransform {
    pub fn from_quads(source: ImageQuad, destination: ImageQuad) -> Result<Self, A2dError> {
        source.validate("source")?;
        destination.validate("destination")?;

        let source_normalization = normalize_points(source.points())?;
        let destination_normalization = normalize_points(destination.points())?;
        let (normalized, pivot_ratio) = solve_homography(
            source_normalization.points,
            destination_normalization.points,
        )?;
        if !pivot_ratio.is_finite() || pivot_ratio < MIN_PIVOT_RATIO {
            return Err(processing_error(
                "HOMOGRAPHY_ILL_CONDITIONED",
                format!(
                    "homography solve pivot ratio {pivot_ratio:e} is below {MIN_PIVOT_RATIO:e}"
                ),
                true,
            ));
        }

        let denormalized = multiply_3x3(
            destination_normalization.inverse,
            multiply_3x3(normalized, source_normalization.matrix),
        );
        let source_to_destination = normalize_matrix_scale(denormalized)?;
        let destination_to_source = invert_3x3(source_to_destination)?;

        let transform = Self {
            source_to_destination,
            destination_to_source,
            pivot_ratio,
        };
        for (source_point, destination_point) in source
            .points()
            .into_iter()
            .zip(destination.points().into_iter())
        {
            let mapped = transform.map_source_to_destination(source_point)?;
            if distance_squared(mapped, destination_point) > 1.0e-10 {
                return Err(processing_error(
                    "HOMOGRAPHY_CORRESPONDENCE_MISMATCH",
                    "solved homography does not reproduce its input correspondences",
                    false,
                ));
            }
        }
        Ok(transform)
    }

    pub const fn source_to_destination_matrix(self) -> [[f64; 3]; 3] {
        self.source_to_destination
    }

    pub const fn destination_to_source_matrix(self) -> [[f64; 3]; 3] {
        self.destination_to_source
    }

    pub const fn pivot_ratio(self) -> f64 {
        self.pivot_ratio
    }

    pub fn map_source_to_destination(self, point: ImagePoint) -> Result<ImagePoint, A2dError> {
        project(self.source_to_destination, point)
    }

    pub fn map_destination_to_source(self, point: ImagePoint) -> Result<ImagePoint, A2dError> {
        project(self.destination_to_source, point)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RectificationPlan {
    source_width: u32,
    source_height: u32,
    output_size: RectifiedImageSize,
    source_page_corners: ImageQuad,
    destination_page_corners: ImageQuad,
    source_marker_centers: Option<ImageQuad>,
    destination_marker_centers: Option<ImageQuad>,
    transform: ProjectiveTransform,
}

impl RectificationPlan {
    pub fn from_page_corners(
        source_width: u32,
        source_height: u32,
        source_page_corners: ImageQuad,
        output_size: RectifiedImageSize,
    ) -> Result<Self, A2dError> {
        validate_source_dimensions(source_width, source_height)?;
        validate_quad_within_image(
            source_page_corners,
            source_width,
            source_height,
            "source page corners",
        )?;
        let destination_page_corners = output_size.destination_quad();
        let transform =
            ProjectiveTransform::from_quads(source_page_corners, destination_page_corners)?;
        Ok(Self {
            source_width,
            source_height,
            output_size,
            source_page_corners,
            destination_page_corners,
            source_marker_centers: None,
            destination_marker_centers: None,
            transform,
        })
    }

    pub fn from_page_markers(
        source_width: u32,
        source_height: u32,
        markers: &ResolvedPageMarkers,
        layout: &PageLayout,
        output_size: RectifiedImageSize,
    ) -> Result<Self, A2dError> {
        validate_source_dimensions(source_width, source_height)?;
        layout.validate()?;
        validate_physical_page(layout)?;

        let source_marker_centers = ImageQuad::new(
            markers.marker(MarkerRole::TopLeft).center,
            markers.marker(MarkerRole::TopRight).center,
            markers.marker(MarkerRole::BottomRight).center,
            markers.marker(MarkerRole::BottomLeft).center,
        );
        validate_quad_within_image(
            source_marker_centers,
            source_width,
            source_height,
            "source marker centers",
        )?;
        let destination_marker_centers = marker_destination_quad(layout, output_size)?;
        let transform =
            ProjectiveTransform::from_quads(source_marker_centers, destination_marker_centers)?;
        let destination_page_corners = output_size.destination_quad();
        let destination_points = destination_page_corners.points();
        let source_points = destination_points
            .map(|point| transform.map_destination_to_source(point))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let source_page_corners = ImageQuad::new(
            source_points[0],
            source_points[1],
            source_points[2],
            source_points[3],
        );
        validate_quad_within_image(
            source_page_corners,
            source_width,
            source_height,
            "extrapolated source page corners",
        )?;

        Ok(Self {
            source_width,
            source_height,
            output_size,
            source_page_corners,
            destination_page_corners,
            source_marker_centers: Some(source_marker_centers),
            destination_marker_centers: Some(destination_marker_centers),
            transform,
        })
    }

    pub const fn source_width(&self) -> u32 {
        self.source_width
    }

    pub const fn source_height(&self) -> u32 {
        self.source_height
    }

    pub const fn output_size(&self) -> RectifiedImageSize {
        self.output_size
    }

    pub const fn source_page_corners(&self) -> ImageQuad {
        self.source_page_corners
    }

    pub const fn destination_page_corners(&self) -> ImageQuad {
        self.destination_page_corners
    }

    pub const fn source_marker_centers(&self) -> Option<ImageQuad> {
        self.source_marker_centers
    }

    pub const fn destination_marker_centers(&self) -> Option<ImageQuad> {
        self.destination_marker_centers
    }

    pub const fn transform(&self) -> ProjectiveTransform {
        self.transform
    }

    pub fn rectify_gray8(&self, source: GrayFrame<'_>) -> Result<OwnedGrayImage, A2dError> {
        self.validate_source_match(source.width(), source.height())?;
        let output_len = usize::try_from(self.output_size.pixel_count()).map_err(|_| {
            validation_error(
                "RECTIFICATION_OUTPUT_UNSUPPORTED",
                "rectified Gray8 output does not fit this platform's address space",
            )
        })?;
        let mut output = Vec::with_capacity(output_len);
        for y in 0..self.output_size.height() {
            for x in 0..self.output_size.width() {
                let source_point = self.transform.map_destination_to_source(ImagePoint {
                    x: f64::from(x),
                    y: f64::from(y),
                })?;
                output.push(sample_gray8(source, source_point)?);
            }
        }
        OwnedGrayImage::from_tight(
            self.output_size.width(),
            self.output_size.height(),
            ImageRotation::Degrees0,
            output,
        )
    }

    pub fn rectify_rgb8(&self, source: &OwnedRgbImage) -> Result<OwnedRgbImage, A2dError> {
        self.validate_source_match(source.width(), source.height())?;
        let output_len = usize::try_from(self.output_size.rgb_byte_count()).map_err(|_| {
            validation_error(
                "RECTIFICATION_OUTPUT_UNSUPPORTED",
                "rectified RGB8 output does not fit this platform's address space",
            )
        })?;
        let mut output = Vec::with_capacity(output_len);
        for y in 0..self.output_size.height() {
            for x in 0..self.output_size.width() {
                let source_point = self.transform.map_destination_to_source(ImagePoint {
                    x: f64::from(x),
                    y: f64::from(y),
                })?;
                output.extend_from_slice(&sample_rgb8(source, source_point)?);
            }
        }
        OwnedRgbImage::from_tight(
            self.output_size.width(),
            self.output_size.height(),
            ImageRotation::Degrees0,
            output,
        )
    }

    fn validate_source_match(&self, width: u32, height: u32) -> Result<(), A2dError> {
        if width != self.source_width || height != self.source_height {
            return Err(validation_error(
                "RECTIFICATION_SOURCE_DIMENSIONS_MISMATCH",
                format!(
                    "rectification plan expects {}x{} source pixels, got {width}x{height}",
                    self.source_width, self.source_height
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct NormalizedPoints {
    points: [ImagePoint; 4],
    matrix: [[f64; 3]; 3],
    inverse: [[f64; 3]; 3],
}

fn normalize_points(points: [ImagePoint; 4]) -> Result<NormalizedPoints, A2dError> {
    let centroid = ImagePoint {
        x: points.iter().map(|point| point.x).sum::<f64>() / 4.0,
        y: points.iter().map(|point| point.y).sum::<f64>() / 4.0,
    };
    let mean_distance = points
        .iter()
        .map(|point| distance_squared(*point, centroid).sqrt())
        .sum::<f64>()
        / 4.0;
    if !mean_distance.is_finite() || mean_distance <= GEOMETRY_EPSILON {
        return Err(processing_error(
            "HOMOGRAPHY_NORMALIZATION_FAILED",
            "quadrilateral points cannot be normalized",
            true,
        ));
    }
    let scale = 2.0_f64.sqrt() / mean_distance;
    let matrix = [
        [scale, 0.0, -scale * centroid.x],
        [0.0, scale, -scale * centroid.y],
        [0.0, 0.0, 1.0],
    ];
    let inverse = [
        [1.0 / scale, 0.0, centroid.x],
        [0.0, 1.0 / scale, centroid.y],
        [0.0, 0.0, 1.0],
    ];
    let normalized = points.map(|point| ImagePoint {
        x: scale * (point.x - centroid.x),
        y: scale * (point.y - centroid.y),
    });
    Ok(NormalizedPoints {
        points: normalized,
        matrix,
        inverse,
    })
}

fn solve_homography(
    source: [ImagePoint; 4],
    destination: [ImagePoint; 4],
) -> Result<([[f64; 3]; 3], f64), A2dError> {
    let mut augmented = [[0.0_f64; 9]; 8];
    for index in 0..4 {
        let source_point = source[index];
        let destination_point = destination[index];
        let row = index * 2;
        augmented[row] = [
            source_point.x,
            source_point.y,
            1.0,
            0.0,
            0.0,
            0.0,
            -destination_point.x * source_point.x,
            -destination_point.x * source_point.y,
            destination_point.x,
        ];
        augmented[row + 1] = [
            0.0,
            0.0,
            0.0,
            source_point.x,
            source_point.y,
            1.0,
            -destination_point.y * source_point.x,
            -destination_point.y * source_point.y,
            destination_point.y,
        ];
    }

    let mut minimum_pivot = f64::INFINITY;
    let mut maximum_pivot: f64 = 0.0;
    for column in 0..8 {
        let pivot_row = (column..8)
            .max_by(|left, right| {
                augmented[*left][column]
                    .abs()
                    .total_cmp(&augmented[*right][column].abs())
            })
            .expect("column range is never empty");
        let pivot = augmented[pivot_row][column].abs();
        if !pivot.is_finite() || pivot <= GEOMETRY_EPSILON {
            return Err(processing_error(
                "HOMOGRAPHY_SOLVE_SINGULAR",
                format!("homography solve has a singular pivot in column {column}"),
                true,
            ));
        }
        minimum_pivot = minimum_pivot.min(pivot);
        maximum_pivot = maximum_pivot.max(pivot);
        augmented.swap(column, pivot_row);

        let divisor = augmented[column][column];
        for value in &mut augmented[column][column..] {
            *value /= divisor;
        }
        let pivot_values = augmented[column];
        for (row, target_row) in augmented.iter_mut().enumerate() {
            if row == column {
                continue;
            }
            let factor = target_row[column];
            if factor == 0.0 {
                continue;
            }
            for (target, pivot) in target_row[column..].iter_mut().zip(&pivot_values[column..]) {
                *target -= factor * pivot;
            }
        }
    }

    let solution = augmented.map(|row| row[8]);
    if solution.iter().any(|value| !value.is_finite()) {
        return Err(processing_error(
            "HOMOGRAPHY_SOLVE_NON_FINITE",
            "homography solve produced a non-finite coefficient",
            false,
        ));
    }
    Ok((
        [
            [solution[0], solution[1], solution[2]],
            [solution[3], solution[4], solution[5]],
            [solution[6], solution[7], 1.0],
        ],
        minimum_pivot / maximum_pivot,
    ))
}

fn normalize_matrix_scale(mut matrix: [[f64; 3]; 3]) -> Result<[[f64; 3]; 3], A2dError> {
    let maximum = matrix
        .iter()
        .flat_map(|row| row.iter())
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if !maximum.is_finite() || maximum <= GEOMETRY_EPSILON {
        return Err(processing_error(
            "HOMOGRAPHY_MATRIX_INVALID",
            "homography matrix has no finite non-zero scale",
            false,
        ));
    }
    for row in &mut matrix {
        for value in row {
            *value /= maximum;
        }
    }
    Ok(matrix)
}

fn invert_3x3(matrix: [[f64; 3]; 3]) -> Result<[[f64; 3]; 3], A2dError> {
    let determinant = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);
    if !determinant.is_finite() || determinant.abs() <= GEOMETRY_EPSILON {
        return Err(processing_error(
            "HOMOGRAPHY_MATRIX_SINGULAR",
            "homography matrix is singular",
            true,
        ));
    }
    let inverse_determinant = 1.0 / determinant;
    let inverse = [
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * inverse_determinant,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * inverse_determinant,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * inverse_determinant,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * inverse_determinant,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * inverse_determinant,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * inverse_determinant,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * inverse_determinant,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * inverse_determinant,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * inverse_determinant,
        ],
    ];
    if inverse
        .iter()
        .flat_map(|row| row.iter())
        .any(|value| !value.is_finite())
    {
        return Err(processing_error(
            "HOMOGRAPHY_MATRIX_NON_FINITE",
            "inverse homography contains a non-finite coefficient",
            false,
        ));
    }
    Ok(inverse)
}

fn multiply_3x3(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut output = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            output[row][column] = (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum();
        }
    }
    output
}

fn project(matrix: [[f64; 3]; 3], point: ImagePoint) -> Result<ImagePoint, A2dError> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(validation_error(
            "HOMOGRAPHY_POINT_NON_FINITE",
            "projective transform input point is non-finite",
        ));
    }
    let denominator = matrix[2][0] * point.x + matrix[2][1] * point.y + matrix[2][2];
    if !denominator.is_finite() || denominator.abs() <= GEOMETRY_EPSILON {
        return Err(processing_error(
            "HOMOGRAPHY_PROJECTION_INVALID",
            "projective transform maps a point to infinity",
            true,
        ));
    }
    let x = (matrix[0][0] * point.x + matrix[0][1] * point.y + matrix[0][2]) / denominator;
    let y = (matrix[1][0] * point.x + matrix[1][1] * point.y + matrix[1][2]) / denominator;
    if !x.is_finite() || !y.is_finite() {
        return Err(processing_error(
            "HOMOGRAPHY_PROJECTION_NON_FINITE",
            "projective transform produced a non-finite point",
            false,
        ));
    }
    Ok(ImagePoint { x, y })
}

fn marker_destination_quad(
    layout: &PageLayout,
    output_size: RectifiedImageSize,
) -> Result<ImageQuad, A2dError> {
    let point_for = |role: MarkerRole| -> Result<ImagePoint, A2dError> {
        let marker = layout
            .markers
            .iter()
            .find(|marker| marker.role == role)
            .ok_or_else(|| {
                validation_error(
                    "RECTIFICATION_LAYOUT_MARKER_MISSING",
                    format!("layout is missing {} marker", role.as_id_str()),
                )
            })?;
        let center_x = marker.rect.origin.x_mm + marker.rect.size.width_mm * 0.5;
        let center_y = marker.rect.origin.y_mm + marker.rect.size.height_mm * 0.5;
        if !center_x.is_finite() || !center_y.is_finite() {
            return Err(validation_error(
                "RECTIFICATION_LAYOUT_GEOMETRY_INVALID",
                format!("layout {} marker center is non-finite", role.as_id_str()),
            ));
        }
        Ok(ImagePoint {
            x: center_x / layout.physical_size.width_mm * f64::from(output_size.width() - 1),
            y: center_y / layout.physical_size.height_mm * f64::from(output_size.height() - 1),
        })
    };

    Ok(ImageQuad::new(
        point_for(MarkerRole::TopLeft)?,
        point_for(MarkerRole::TopRight)?,
        point_for(MarkerRole::BottomRight)?,
        point_for(MarkerRole::BottomLeft)?,
    ))
}

fn validate_physical_page(layout: &PageLayout) -> Result<(), A2dError> {
    if !layout.physical_size.width_mm.is_finite()
        || !layout.physical_size.height_mm.is_finite()
        || layout.physical_size.width_mm <= 0.0
        || layout.physical_size.height_mm <= 0.0
    {
        return Err(validation_error(
            "RECTIFICATION_LAYOUT_SIZE_INVALID",
            format!(
                "layout physical size must be finite and positive, got {}x{}mm",
                layout.physical_size.width_mm, layout.physical_size.height_mm
            ),
        ));
    }
    Ok(())
}

fn validate_source_dimensions(width: u32, height: u32) -> Result<(), A2dError> {
    if width < 2 || height < 2 {
        return Err(validation_error(
            "RECTIFICATION_SOURCE_DIMENSIONS_INVALID",
            format!("source dimensions must be at least 2x2, got {width}x{height}"),
        ));
    }
    Ok(())
}

fn validate_quad_within_image(
    quad: ImageQuad,
    width: u32,
    height: u32,
    label: &'static str,
) -> Result<(), A2dError> {
    quad.validate(label)?;
    let max_x = f64::from(width - 1);
    let max_y = f64::from(height - 1);
    if quad.points().iter().any(|point| {
        point.x < -BOUNDS_EPSILON
            || point.y < -BOUNDS_EPSILON
            || point.x > max_x + BOUNDS_EPSILON
            || point.y > max_y + BOUNDS_EPSILON
    }) {
        return Err(validation_error(
            "RECTIFICATION_SOURCE_CORNERS_OUT_OF_BOUNDS",
            format!("{label} extend outside the {width}x{height} source image"),
        ));
    }
    Ok(())
}

fn sample_gray8(source: GrayFrame<'_>, point: ImagePoint) -> Result<u8, A2dError> {
    let (x0, y0, x1, y1, x_fraction, y_fraction) =
        sampling_coordinates(point, source.width(), source.height())?;
    let bytes = source.bytes();
    let stride = source.row_stride();
    let value = bilinear(
        f64::from(bytes[y0 * stride + x0]),
        f64::from(bytes[y0 * stride + x1]),
        f64::from(bytes[y1 * stride + x0]),
        f64::from(bytes[y1 * stride + x1]),
        x_fraction,
        y_fraction,
    );
    Ok(value.round().clamp(0.0, 255.0) as u8)
}

fn sample_rgb8(source: &OwnedRgbImage, point: ImagePoint) -> Result<[u8; 3], A2dError> {
    let (x0, y0, x1, y1, x_fraction, y_fraction) =
        sampling_coordinates(point, source.width(), source.height())?;
    let bytes = source.bytes();
    let stride = source.row_stride();
    let pixel = |x: usize, y: usize, channel: usize| -> f64 {
        f64::from(bytes[y * stride + x * 3 + channel])
    };
    let mut output = [0_u8; 3];
    for (channel, output_channel) in output.iter_mut().enumerate() {
        let value = bilinear(
            pixel(x0, y0, channel),
            pixel(x1, y0, channel),
            pixel(x0, y1, channel),
            pixel(x1, y1, channel),
            x_fraction,
            y_fraction,
        );
        *output_channel = value.round().clamp(0.0, 255.0) as u8;
    }
    Ok(output)
}

fn sampling_coordinates(
    point: ImagePoint,
    width: u32,
    height: u32,
) -> Result<(usize, usize, usize, usize, f64, f64), A2dError> {
    let max_x = f64::from(width - 1);
    let max_y = f64::from(height - 1);
    if !point.x.is_finite()
        || !point.y.is_finite()
        || point.x < -BOUNDS_EPSILON
        || point.y < -BOUNDS_EPSILON
        || point.x > max_x + BOUNDS_EPSILON
        || point.y > max_y + BOUNDS_EPSILON
    {
        return Err(processing_error(
            "RECTIFICATION_SAMPLE_OUT_OF_BOUNDS",
            format!(
                "rectification sample ({}, {}) is outside {width}x{height} source bounds",
                point.x, point.y
            ),
            true,
        ));
    }
    let x = point.x.clamp(0.0, max_x);
    let y = point.y.clamp(0.0, max_y);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width as usize - 1);
    let y1 = (y0 + 1).min(height as usize - 1);
    Ok((x0, y0, x1, y1, x - x0 as f64, y - y0 as f64))
}

fn bilinear(
    top_left: f64,
    top_right: f64,
    bottom_left: f64,
    bottom_right: f64,
    x_fraction: f64,
    y_fraction: f64,
) -> f64 {
    let top = top_left + (top_right - top_left) * x_fraction;
    let bottom = bottom_left + (bottom_right - bottom_left) * x_fraction;
    top + (bottom - top) * y_fraction
}

fn cross_product(origin: ImagePoint, first: ImagePoint, second: ImagePoint) -> f64 {
    (first.x - origin.x) * (second.y - origin.y) - (first.y - origin.y) * (second.x - origin.x)
}

fn distance_squared(first: ImagePoint, second: ImagePoint) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    dx * dx + dy * dy
}

fn segments_intersect(
    first_start: ImagePoint,
    first_end: ImagePoint,
    second_start: ImagePoint,
    second_end: ImagePoint,
) -> bool {
    let first_a = cross_product(first_start, first_end, second_start);
    let first_b = cross_product(first_start, first_end, second_end);
    let second_a = cross_product(second_start, second_end, first_start);
    let second_b = cross_product(second_start, second_end, first_end);
    first_a * first_b < -GEOMETRY_EPSILON && second_a * second_b < -GEOMETRY_EPSILON
}

#[cfg(test)]
mod tests {
    use a2d_domain::LayoutId;
    use a2d_layout::{
        CalibrationMark, ContentStyle, MarkerPlacement,
        geometry::{PhysicalPoint, PhysicalRect, PhysicalSize},
    };

    use crate::{
        MarkerDetection, MarkerFamily, PageOrientation, ResolvedMarker, input::ImageLimits,
    };

    use super::*;

    fn limits() -> RectificationLimits {
        RectificationLimits::new(1_000_000, 3_000_000).unwrap()
    }

    fn point(x: f64, y: f64) -> ImagePoint {
        ImagePoint { x, y }
    }

    fn full_quad(width: u32, height: u32) -> ImageQuad {
        ImageQuad::new(
            point(0.0, 0.0),
            point(f64::from(width - 1), 0.0),
            point(f64::from(width - 1), f64::from(height - 1)),
            point(0.0, f64::from(height - 1)),
        )
    }

    #[test]
    fn identity_homography_preserves_points() {
        let quad = full_quad(10, 20);
        let transform = ProjectiveTransform::from_quads(quad, quad).unwrap();
        for candidate in [point(0.0, 0.0), point(3.25, 7.5), point(9.0, 19.0)] {
            let mapped = transform.map_source_to_destination(candidate).unwrap();
            assert!((mapped.x - candidate.x).abs() < 1.0e-9);
            assert!((mapped.y - candidate.y).abs() < 1.0e-9);
        }
    }

    #[test]
    fn perspective_homography_reproduces_all_correspondences() {
        let source = ImageQuad::new(
            point(12.0, 8.0),
            point(90.0, 4.0),
            point(96.0, 110.0),
            point(5.0, 100.0),
        );
        let destination = full_quad(80, 100);
        let transform = ProjectiveTransform::from_quads(source, destination).unwrap();
        for (source_point, destination_point) in source
            .points()
            .into_iter()
            .zip(destination.points().into_iter())
        {
            let mapped = transform.map_source_to_destination(source_point).unwrap();
            assert!((mapped.x - destination_point.x).abs() < 1.0e-7);
            assert!((mapped.y - destination_point.y).abs() < 1.0e-7);
        }
    }

    #[test]
    fn rejects_self_intersecting_and_concave_quadrilaterals() {
        let bow_tie = ImageQuad::new(
            point(0.0, 0.0),
            point(10.0, 10.0),
            point(10.0, 0.0),
            point(0.0, 10.0),
        );
        let err = ProjectiveTransform::from_quads(bow_tie, full_quad(10, 10)).unwrap_err();
        assert_eq!(err.code.to_string(), "HOMOGRAPHY_QUAD_SELF_INTERSECTING");

        let concave = ImageQuad::new(
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(4.0, 4.0),
            point(0.0, 10.0),
        );
        let err = ProjectiveTransform::from_quads(concave, full_quad(10, 10)).unwrap_err();
        assert_eq!(err.code.to_string(), "HOMOGRAPHY_QUAD_NON_CONVEX");
    }

    #[test]
    fn rejects_degenerate_quadrilateral() {
        let degenerate = ImageQuad::new(
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(20.0, 0.0),
            point(0.0, 10.0),
        );
        let err = ProjectiveTransform::from_quads(degenerate, full_quad(10, 10)).unwrap_err();
        assert_eq!(err.code.to_string(), "HOMOGRAPHY_QUAD_DEGENERATE");
    }

    #[test]
    fn identity_gray_warp_matches_reference_bytes() {
        let bytes: Vec<u8> = (0_u8..16).collect();
        let frame = GrayFrame::new(
            4,
            4,
            4,
            ImageRotation::Degrees270,
            &bytes,
            ImageLimits::new(16).unwrap(),
        )
        .unwrap();
        let plan = RectificationPlan::from_page_corners(
            4,
            4,
            full_quad(4, 4),
            RectifiedImageSize::new(4, 4, limits()).unwrap(),
        )
        .unwrap();
        let output = plan.rectify_gray8(frame).unwrap();

        assert_eq!(output.bytes(), bytes);
        assert_eq!(output.rotation(), ImageRotation::Degrees0);
    }

    #[test]
    fn identity_rgb_warp_matches_reference_bytes() {
        let bytes: Vec<u8> = (0_u8..48).collect();
        let source =
            OwnedRgbImage::from_tight(4, 4, ImageRotation::Degrees180, bytes.clone()).unwrap();
        let plan = RectificationPlan::from_page_corners(
            4,
            4,
            full_quad(4, 4),
            RectifiedImageSize::new(4, 4, limits()).unwrap(),
        )
        .unwrap();
        let output = plan.rectify_rgb8(&source).unwrap();

        assert_eq!(output.bytes(), bytes);
        assert_eq!(output.rotation(), ImageRotation::Degrees0);
    }

    #[test]
    fn rejects_source_page_corners_outside_the_image() {
        let source = ImageQuad::new(
            point(-1.0, 0.0),
            point(9.0, 0.0),
            point(9.0, 9.0),
            point(0.0, 9.0),
        );
        let err = RectificationPlan::from_page_corners(
            10,
            10,
            source,
            RectifiedImageSize::new(10, 10, limits()).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err.code.to_string(),
            "RECTIFICATION_SOURCE_CORNERS_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn rejects_source_dimensions_that_do_not_match_the_plan() {
        let bytes = [0_u8; 25];
        let frame = GrayFrame::new(
            5,
            5,
            5,
            ImageRotation::Degrees0,
            &bytes,
            ImageLimits::new(25).unwrap(),
        )
        .unwrap();
        let plan = RectificationPlan::from_page_corners(
            4,
            4,
            full_quad(4, 4),
            RectifiedImageSize::new(4, 4, limits()).unwrap(),
        )
        .unwrap();
        let err = plan.rectify_gray8(frame).unwrap_err();
        assert_eq!(
            err.code.to_string(),
            "RECTIFICATION_SOURCE_DIMENSIONS_MISMATCH"
        );
    }

    #[test]
    fn enforces_output_memory_limits() {
        let err =
            RectifiedImageSize::new(100, 100, RectificationLimits::new(10_000, 29_999).unwrap())
                .unwrap_err();
        assert_eq!(err.code.to_string(), "RECTIFICATION_BYTE_LIMIT_EXCEEDED");
    }

    fn layout() -> PageLayout {
        let marker = |role, x, y| MarkerPlacement {
            role,
            rect: PhysicalRect::new(x, y, 10.0, 10.0),
        };
        PageLayout {
            id: LayoutId::parse("RECTIFY-TEST").unwrap(),
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

    fn detection(id: u32, center: ImagePoint) -> MarkerDetection {
        MarkerDetection {
            family: MarkerFamily::TagStandard41h12,
            id,
            hamming_errors: 0,
            decision_margin: 100.0,
            center,
            corners: [center; 4],
        }
    }

    #[test]
    fn page_marker_plan_preserves_correspondence_and_source_page_corners() {
        let resolved = ResolvedPageMarkers {
            markers: [
                ResolvedMarker {
                    role: MarkerRole::TopLeft,
                    detection: detection(1, point(20.0, 20.0)),
                },
                ResolvedMarker {
                    role: MarkerRole::TopRight,
                    detection: detection(2, point(180.0, 20.0)),
                },
                ResolvedMarker {
                    role: MarkerRole::BottomLeft,
                    detection: detection(3, point(20.0, 380.0)),
                },
                ResolvedMarker {
                    role: MarkerRole::BottomRight,
                    detection: detection(4, point(180.0, 380.0)),
                },
            ],
            orientation: PageOrientation::Degrees0,
            unexpected_tag_ids: Vec::new(),
        };
        let output_size = RectifiedImageSize::new(100, 200, limits()).unwrap();
        let plan =
            RectificationPlan::from_page_markers(201, 401, &resolved, &layout(), output_size)
                .unwrap();

        let source_markers = plan.source_marker_centers().unwrap().points();
        let destination_markers = plan.destination_marker_centers().unwrap().points();
        for (source, expected) in source_markers
            .into_iter()
            .zip(destination_markers.into_iter())
        {
            let actual = plan.transform().map_source_to_destination(source).unwrap();
            assert!((actual.x - expected.x).abs() < 1.0e-7);
            assert!((actual.y - expected.y).abs() < 1.0e-7);
        }
        let source_page = plan.source_page_corners();
        assert!((source_page.top_left.x - 0.0).abs() < 1.0e-7);
        assert!((source_page.top_left.y - 0.0).abs() < 1.0e-7);
        assert!((source_page.bottom_right.x - 200.0).abs() < 1.0e-7);
        assert!((source_page.bottom_right.y - 400.0).abs() < 1.0e-7);
    }
}
