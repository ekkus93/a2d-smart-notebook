use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use a2d_image::{
    AprilTagDetector, DetectorConfig, EncodedImage, EncodedImageFormat, EncodedImageLimits,
    ImageLimits, ImageRotation, LuminanceMeasurementConfig, MarkerIdLayout, measure_gray_quality,
    resolve_page_markers,
};
use a2d_layout::MarkerRole;

const PNG_FIXTURES: &[&str] = &[
    "generated/base-page.png",
    "generated/rotated-7-5-degrees.png",
    "generated/rotated-90-degrees.png",
    "generated/perspective-mild.png",
    "generated/perspective-severe.png",
    "generated/underexposed.png",
    "generated/overexposed.png",
    "blur/gaussian-radius-2.png",
    "blur/gaussian-radius-7.png",
    "glare/partial-glare.png",
    "glare/strong-glare.png",
    "missing-marker/missing-bottom-right.png",
    "wrong-layout/wrong-layout-qr.png",
    "wrong-layout/wrong-tag-set.png",
    "duplicate/duplicate-top-left.png",
    "revisions/revision-original.png",
    "revisions/revision-updated.png",
];

fn usage() -> ! {
    eprintln!("usage: a2d-fixture-measurements <fixture-root> <output.tsv>");
    std::process::exit(2);
}

fn decode_png(path: &Path) -> Result<a2d_image::OwnedGrayImage, a2d_domain::A2dError> {
    let bytes = fs::read(path).map_err(|error| {
        a2d_domain::A2dError::new(
            a2d_domain::ErrorCode::new("FIXTURE_MEASUREMENT_READ_FAILED"),
            a2d_domain::ErrorCategory::Storage,
            a2d_domain::ErrorSeverity::Error,
            "error.fixture.read_failed",
            format!("failed to read {}: {error}", path.display()),
            false,
        )
    })?;
    let limits = EncodedImageLimits::new(bytes.len().saturating_add(1), 5_000_000, 15_000_000)?;
    EncodedImage::new(
        &bytes,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        limits,
    )?
    .decode_rgb8()?
    .into_gray8(ImageLimits::new(5_000_000)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let fixture_root = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    let output_path = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    if args.next().is_some() {
        usage();
    }

    let image_limits = ImageLimits::new(5_000_000)?;
    let quality_config = LuminanceMeasurementConfig::new(40, 245, 8, 8)?;
    let marker_layout = MarkerIdLayout::new([
        (0, MarkerRole::TopLeft),
        (1, MarkerRole::TopRight),
        (2, MarkerRole::BottomRight),
        (3, MarkerRole::BottomLeft),
    ])?;
    let mut detector = AprilTagDetector::new(DetectorConfig::default())?;

    let mut report = String::from(
        "path\twidth\theight\tfocus_laplacian_variance\tmean_luminance\t"
    );
    report.push_str(
        "luminance_standard_deviation\tdark_fraction\thighlight_fraction\t"
    );
    report.push_str(
        "max_tile_highlight_fraction\tdetection_count\tdetected_ids\t"
    );
    report.push_str(
        "minimum_decision_margin\tmean_decision_margin\tmaximum_hamming_errors\tresolution\n",
    );

    for relative_path in PNG_FIXTURES {
        let gray = decode_png(&fixture_root.join(relative_path))?;
        let frame = gray.as_frame(image_limits)?;
        let quality = measure_gray_quality(frame, quality_config)?;
        let detections = detector.detect(frame)?;
        let ids = detections
            .iter()
            .map(|detection| detection.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let minimum_decision_margin = detections
            .iter()
            .map(|detection| f64::from(detection.decision_margin))
            .reduce(f64::min);
        let mean_decision_margin = if detections.is_empty() {
            None
        } else {
            Some(
                detections
                    .iter()
                    .map(|detection| f64::from(detection.decision_margin))
                    .sum::<f64>()
                    / detections.len() as f64,
            )
        };
        let maximum_hamming_errors = detections
            .iter()
            .map(|detection| detection.hamming_errors)
            .max();
        let resolution = match resolve_page_markers(&detections, &marker_layout) {
            Ok(resolved) => format!("resolved:{}", resolved.orientation.degrees()),
            Err(error) => format!("error:{}", error.code),
        };
        let focus = quality
            .focus
            .expect("all committed PNG fixtures are large enough for focus measurement");

        writeln!(
            report,
            "{relative_path}\t{}\t{}\t{:.12}\t{:.12}\t{:.12}\t{:.12}\t{:.12}\t{:.12}\t{}\t{}\t{}\t{}\t{}\t{}",
            gray.width(),
            gray.height(),
            focus.laplacian_variance,
            quality.exposure.mean_luminance,
            quality.exposure.luminance_standard_deviation,
            quality.exposure.dark_fraction,
            quality.exposure.highlight_fraction,
            quality.glare.max_tile_highlight_fraction,
            detections.len(),
            ids,
            minimum_decision_margin
                .map(|value| format!("{value:.12}"))
                .unwrap_or_default(),
            mean_decision_margin
                .map(|value| format!("{value:.12}"))
                .unwrap_or_default(),
            maximum_hamming_errors
                .map(|value| value.to_string())
                .unwrap_or_default(),
            resolution,
        )?;
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, report)?;
    println!("wrote synthetic fixture measurements to {}", output_path.display());
    Ok(())
}
