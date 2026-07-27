use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use a2d_image::{
    AprilTagDetector, DetectorConfig, EncodedImage, EncodedImageFormat, EncodedImageLimits,
    ImageLimits, ImageRotation, LuminanceMeasurementConfig, MarkerIdLayout, PageOrientation,
    measure_gray_quality, resolve_page_markers,
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

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scans")
}

fn read_fixture(relative_path: &str) -> Vec<u8> {
    let path = fixture_root().join(relative_path);
    fs::read(&path).unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn encoded_limits(encoded_len: usize) -> EncodedImageLimits {
    EncodedImageLimits::new(encoded_len.saturating_add(1), 5_000_000, 15_000_000).unwrap()
}

fn image_limits() -> ImageLimits {
    ImageLimits::new(5_000_000).unwrap()
}

fn decode_png(relative_path: &str) -> a2d_image::OwnedGrayImage {
    let bytes = read_fixture(relative_path);
    EncodedImage::new(
        &bytes,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        encoded_limits(bytes.len()),
    )
    .unwrap_or_else(|error| panic!("failed to validate {relative_path}: {error}"))
    .decode_rgb8()
    .unwrap_or_else(|error| panic!("failed to decode {relative_path}: {error}"))
    .into_gray8(image_limits())
    .unwrap_or_else(|error| panic!("failed to convert {relative_path} to Gray8: {error}"))
}

#[test]
fn every_generated_png_runs_through_shared_decode_and_quality_measurement() {
    let measurement_config = LuminanceMeasurementConfig::new(40, 245, 8, 8).unwrap();

    for relative_path in PNG_FIXTURES {
        let gray = decode_png(relative_path);
        let frame = gray
            .as_frame(image_limits())
            .unwrap_or_else(|error| panic!("failed to borrow {relative_path}: {error}"));
        let metrics = measure_gray_quality(frame, measurement_config)
            .unwrap_or_else(|error| panic!("failed to measure {relative_path}: {error}"));

        assert!(metrics.exposure.mean_luminance.is_finite(), "{relative_path}");
        assert!(
            metrics.exposure.luminance_standard_deviation.is_finite(),
            "{relative_path}"
        );
        assert!(metrics.exposure.dark_fraction.is_finite(), "{relative_path}");
        assert!(
            metrics.exposure.highlight_fraction.is_finite(),
            "{relative_path}"
        );
        assert!(metrics.glare.max_tile_highlight_fraction.is_finite(), "{relative_path}");
        assert!(metrics.focus.is_some(), "{relative_path}");
    }
}

#[test]
fn canonical_page_detects_and_resolves_the_expected_official_markers() {
    let gray = decode_png("generated/base-page.png");
    let frame = gray.as_frame(image_limits()).unwrap();
    let mut detector = AprilTagDetector::new(DetectorConfig::default()).unwrap();
    let detections = detector.detect(frame).unwrap();

    let ids: BTreeSet<_> = detections.iter().map(|detection| detection.id).collect();
    assert_eq!(ids, BTreeSet::from([0, 1, 2, 3]));
    assert!(detections.iter().all(|detection| detection.hamming_errors == 0));

    let layout = MarkerIdLayout::new([
        (0, MarkerRole::TopLeft),
        (1, MarkerRole::TopRight),
        (2, MarkerRole::BottomRight),
        (3, MarkerRole::BottomLeft),
    ])
    .unwrap();
    let resolved = resolve_page_markers(&detections, &layout).unwrap();
    assert_eq!(resolved.orientation, PageOrientation::Degrees0);
    assert!(resolved.unexpected_tag_ids.is_empty());
}

#[test]
fn corrupted_controls_are_rejected_without_fabricated_success() {
    let truncated = read_fixture("corrupted/truncated.png");
    let error = EncodedImage::new(
        &truncated,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        encoded_limits(truncated.len()),
    )
    .unwrap()
    .decode_rgb8()
    .unwrap_err();
    assert!(
        matches!(
            error.code.to_string().as_str(),
            "IMAGE_DIMENSION_READ_FAILED" | "IMAGE_DECODE_FAILED"
        ),
        "unexpected truncated PNG error: {error}"
    );

    let invalid = read_fixture("corrupted/not-an-image.bin");
    let error = EncodedImage::new(
        &invalid,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        encoded_limits(invalid.len()),
    )
    .unwrap_err();
    assert_eq!(error.code.to_string(), "IMAGE_FORMAT_MISMATCH");
}
