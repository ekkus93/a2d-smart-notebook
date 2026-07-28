use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use a2d_image::{
    AprilTagDetector, DetectorConfig, EncodedImage, EncodedImageFormat, EncodedImageLimits,
    ImageLimits, ImageRotation, LuminanceMeasurementConfig, MarkerIdLayout, measure_gray_quality,
    resolve_page_markers,
};
use a2d_layout::MarkerRole;

const EXPECTED_HEADER: &str = "path\twidth\theight\tfocus_min\tfocus_max\tmean_luminance_min\tmean_luminance_max\tluminance_std_min\tluminance_std_max\tdark_fraction_min\tdark_fraction_max\thighlight_fraction_min\thighlight_fraction_max\tmax_tile_highlight_min\tmax_tile_highlight_max\tdetection_count\tdetected_ids\tminimum_decision_margin_min\tminimum_decision_margin_max\tmaximum_hamming_errors\tresolution";

#[derive(Debug)]
struct NumericRange {
    minimum: f64,
    maximum: f64,
}

#[derive(Debug)]
struct ProcessingExpectation {
    path: String,
    width: u32,
    height: u32,
    focus: NumericRange,
    mean_luminance: NumericRange,
    luminance_standard_deviation: NumericRange,
    dark_fraction: NumericRange,
    highlight_fraction: NumericRange,
    max_tile_highlight_fraction: NumericRange,
    detection_count: usize,
    detected_ids: BTreeSet<u32>,
    minimum_decision_margin: Option<NumericRange>,
    maximum_hamming_errors: Option<u8>,
    resolution: String,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scans")
}

fn parse_number<T>(value: &str, field: &str, line_number: usize) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.parse::<T>().unwrap_or_else(|error| {
        panic!("invalid {field} on expectations line {line_number}: {value}: {error}")
    })
}

fn parse_range(
    minimum: &str,
    maximum: &str,
    field: &str,
    line_number: usize,
) -> NumericRange {
    let range = NumericRange {
        minimum: parse_number(minimum, &format!("{field}_minimum"), line_number),
        maximum: parse_number(maximum, &format!("{field}_maximum"), line_number),
    };
    assert!(
        range.minimum.is_finite()
            && range.maximum.is_finite()
            && range.minimum <= range.maximum,
        "invalid {field} range on expectations line {line_number}: {range:?}"
    );
    range
}

fn parse_optional_range(
    minimum: &str,
    maximum: &str,
    field: &str,
    line_number: usize,
) -> Option<NumericRange> {
    match (minimum, maximum) {
        ("-", "-") => None,
        ("-", _) | (_, "-") => {
            panic!("partial optional {field} range on expectations line {line_number}")
        }
        _ => Some(parse_range(minimum, maximum, field, line_number)),
    }
}

fn load_expectations() -> Vec<ProcessingExpectation> {
    let path = fixture_root().join("processing-expectations.tsv");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.starts_with('#'));
    let (header_line_number, header) = lines.next().expect("expectations file must contain a header");
    assert_eq!(
        header,
        EXPECTED_HEADER,
        "unexpected processing expectations header on line {}",
        header_line_number + 1
    );

    let expectations = lines
        .map(|(zero_based_line_number, line)| {
            let line_number = zero_based_line_number + 1;
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                21,
                "processing expectations line {line_number} must contain 21 fields"
            );
            let detected_ids = if fields[16] == "-" {
                BTreeSet::new()
            } else {
                fields[16]
                    .split(',')
                    .map(|value| parse_number(value, "detected_id", line_number))
                    .collect()
            };
            let maximum_hamming_errors = if fields[19] == "-" {
                None
            } else {
                Some(parse_number(
                    fields[19],
                    "maximum_hamming_errors",
                    line_number,
                ))
            };

            ProcessingExpectation {
                path: fields[0].to_string(),
                width: parse_number(fields[1], "width", line_number),
                height: parse_number(fields[2], "height", line_number),
                focus: parse_range(fields[3], fields[4], "focus", line_number),
                mean_luminance: parse_range(
                    fields[5],
                    fields[6],
                    "mean_luminance",
                    line_number,
                ),
                luminance_standard_deviation: parse_range(
                    fields[7],
                    fields[8],
                    "luminance_standard_deviation",
                    line_number,
                ),
                dark_fraction: parse_range(
                    fields[9],
                    fields[10],
                    "dark_fraction",
                    line_number,
                ),
                highlight_fraction: parse_range(
                    fields[11],
                    fields[12],
                    "highlight_fraction",
                    line_number,
                ),
                max_tile_highlight_fraction: parse_range(
                    fields[13],
                    fields[14],
                    "max_tile_highlight_fraction",
                    line_number,
                ),
                detection_count: parse_number(fields[15], "detection_count", line_number),
                detected_ids,
                minimum_decision_margin: parse_optional_range(
                    fields[17],
                    fields[18],
                    "minimum_decision_margin",
                    line_number,
                ),
                maximum_hamming_errors,
                resolution: fields[20].to_string(),
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(
        expectations.len(),
        17,
        "every committed decodable synthetic fixture must have one expectation row"
    );
    let unique_paths = expectations
        .iter()
        .map(|expectation| expectation.path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        unique_paths.len(),
        expectations.len(),
        "processing expectation paths must be unique"
    );
    expectations
}

fn decode_png(relative_path: &str) -> a2d_image::OwnedGrayImage {
    let path = fixture_root().join(relative_path);
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let encoded_limits =
        EncodedImageLimits::new(bytes.len().saturating_add(1), 5_000_000, 15_000_000).unwrap();
    EncodedImage::new(
        &bytes,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        encoded_limits,
    )
    .unwrap_or_else(|error| panic!("failed to validate {relative_path}: {error}"))
    .decode_rgb8()
    .unwrap_or_else(|error| panic!("failed to decode {relative_path}: {error}"))
    .into_gray8(ImageLimits::new(5_000_000).unwrap())
    .unwrap_or_else(|error| panic!("failed to convert {relative_path} to Gray8: {error}"))
}

fn assert_in_range(path: &str, field: &str, value: f64, range: &NumericRange) {
    assert!(
        value.is_finite() && value >= range.minimum && value <= range.maximum,
        "{path} {field} value {value} is outside synthetic regression envelope [{}, {}]",
        range.minimum,
        range.maximum
    );
}

#[test]
fn synthetic_fixture_processing_stays_inside_committed_regression_envelopes() {
    let image_limits = ImageLimits::new(5_000_000).unwrap();
    let quality_config = LuminanceMeasurementConfig::new(40, 245, 8, 8).unwrap();
    let marker_layout = MarkerIdLayout::new([
        (0, MarkerRole::TopLeft),
        (1, MarkerRole::TopRight),
        (2, MarkerRole::BottomRight),
        (3, MarkerRole::BottomLeft),
    ])
    .unwrap();
    let mut detector = AprilTagDetector::new(DetectorConfig::default()).unwrap();

    for expectation in load_expectations() {
        let gray = decode_png(&expectation.path);
        assert_eq!(gray.width(), expectation.width, "{} width", expectation.path);
        assert_eq!(gray.height(), expectation.height, "{} height", expectation.path);
        let frame = gray.as_frame(image_limits).unwrap();
        let quality = measure_gray_quality(frame, quality_config).unwrap();
        let focus = quality
            .focus
            .expect("all synthetic fixtures must remain large enough for focus measurement");
        assert_in_range(
            &expectation.path,
            "focus_laplacian_variance",
            focus.laplacian_variance,
            &expectation.focus,
        );
        assert_in_range(
            &expectation.path,
            "mean_luminance",
            quality.exposure.mean_luminance,
            &expectation.mean_luminance,
        );
        assert_in_range(
            &expectation.path,
            "luminance_standard_deviation",
            quality.exposure.luminance_standard_deviation,
            &expectation.luminance_standard_deviation,
        );
        assert_in_range(
            &expectation.path,
            "dark_fraction",
            quality.exposure.dark_fraction,
            &expectation.dark_fraction,
        );
        assert_in_range(
            &expectation.path,
            "highlight_fraction",
            quality.exposure.highlight_fraction,
            &expectation.highlight_fraction,
        );
        assert_in_range(
            &expectation.path,
            "max_tile_highlight_fraction",
            quality.glare.max_tile_highlight_fraction,
            &expectation.max_tile_highlight_fraction,
        );

        let detections = detector.detect(frame).unwrap();
        assert_eq!(
            detections.len(),
            expectation.detection_count,
            "{} detection count",
            expectation.path
        );
        let detected_ids = detections
            .iter()
            .map(|detection| detection.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            detected_ids, expectation.detected_ids,
            "{} detected IDs",
            expectation.path
        );
        let minimum_decision_margin = detections
            .iter()
            .map(|detection| f64::from(detection.decision_margin))
            .reduce(f64::min);
        match (
            minimum_decision_margin,
            expectation.minimum_decision_margin.as_ref(),
        ) {
            (Some(value), Some(range)) => assert_in_range(
                &expectation.path,
                "minimum_decision_margin",
                value,
                range,
            ),
            (None, None) => {}
            (actual, expected) => panic!(
                "{} minimum decision margin availability mismatch: actual={actual:?}, expected={expected:?}",
                expectation.path
            ),
        }
        let maximum_hamming_errors = detections
            .iter()
            .map(|detection| detection.hamming_errors)
            .max();
        assert_eq!(
            maximum_hamming_errors, expectation.maximum_hamming_errors,
            "{} maximum Hamming errors",
            expectation.path
        );
        let resolution = match resolve_page_markers(&detections, &marker_layout) {
            Ok(resolved) => format!("resolved:{}", resolved.orientation.degrees()),
            Err(error) => format!("error:{}", error.code),
        };
        assert_eq!(
            resolution, expectation.resolution,
            "{} semantic marker resolution",
            expectation.path
        );
    }
}
