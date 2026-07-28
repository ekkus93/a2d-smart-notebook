from pathlib import Path

ROOT = Path(".")
fingerprint_path = ROOT / "crates/a2d-image/src/fingerprint.rs"
lib_path = ROOT / "crates/a2d-image/src/lib.rs"
milestone_path = ROOT / "crates/a2d-core/src/milestone9.rs"
tests_path = ROOT / "crates/a2d-core/src/milestone9_tests.rs"
todo_path = ROOT / "docs/A2D_SMART_NOTEBOOK_V01_TODO.md"


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match, found {count}: {old[:120]!r}")
    path.write_text(text.replace(old, new))


fingerprint_path.write_text(r'''use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

use crate::OwnedGrayImage;

pub const PERCEPTUAL_FINGERPRINT_V1_WIDTH: usize = 16;
pub const PERCEPTUAL_FINGERPRINT_V1_HEIGHT: usize = 24;
pub const PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT: usize =
    PERCEPTUAL_FINGERPRINT_V1_WIDTH * PERCEPTUAL_FINGERPRINT_V1_HEIGHT;
const SERIALIZATION_PREFIX: &str = "mean-grid-16x24-v1:";

/// Versioned, compact representation of an already-rectified and contrast-normalized page.
///
/// Each byte is the mean luminance of one proportional cell in a 16x24 grid. This intentionally
/// stores measurements rather than a duplicate/revision classification: production thresholds must
/// be tuned from photographed fixtures before they can become policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerceptualFingerprintV1 {
    cells: Vec<u8>,
}

impl PerceptualFingerprintV1 {
    pub fn from_gray_image(image: &OwnedGrayImage) -> Result<Self, A2dError> {
        let width = usize::try_from(image.width()).map_err(|_| {
            fingerprint_error(
                "IMAGE_FINGERPRINT_DIMENSION_UNSUPPORTED",
                "fingerprint source width does not fit this platform",
            )
        })?;
        let height = usize::try_from(image.height()).map_err(|_| {
            fingerprint_error(
                "IMAGE_FINGERPRINT_DIMENSION_UNSUPPORTED",
                "fingerprint source height does not fit this platform",
            )
        })?;
        if width < PERCEPTUAL_FINGERPRINT_V1_WIDTH
            || height < PERCEPTUAL_FINGERPRINT_V1_HEIGHT
        {
            return Err(fingerprint_error(
                "IMAGE_FINGERPRINT_SOURCE_TOO_SMALL",
                format!(
                    "fingerprint source must be at least {}x{}, got {}x{}",
                    PERCEPTUAL_FINGERPRINT_V1_WIDTH,
                    PERCEPTUAL_FINGERPRINT_V1_HEIGHT,
                    width,
                    height
                ),
            ));
        }

        let mut cells = Vec::with_capacity(PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT);
        for grid_y in 0..PERCEPTUAL_FINGERPRINT_V1_HEIGHT {
            let y0 = grid_y * height / PERCEPTUAL_FINGERPRINT_V1_HEIGHT;
            let y1 = (grid_y + 1) * height / PERCEPTUAL_FINGERPRINT_V1_HEIGHT;
            for grid_x in 0..PERCEPTUAL_FINGERPRINT_V1_WIDTH {
                let x0 = grid_x * width / PERCEPTUAL_FINGERPRINT_V1_WIDTH;
                let x1 = (grid_x + 1) * width / PERCEPTUAL_FINGERPRINT_V1_WIDTH;
                let mut sum = 0_u64;
                let mut count = 0_u64;
                for y in y0..y1 {
                    let row = y.checked_mul(image.row_stride()).ok_or_else(|| {
                        fingerprint_error(
                            "IMAGE_FINGERPRINT_INDEX_OVERFLOW",
                            "fingerprint source row offset overflowed",
                        )
                    })?;
                    for x in x0..x1 {
                        let index = row.checked_add(x).ok_or_else(|| {
                            fingerprint_error(
                                "IMAGE_FINGERPRINT_INDEX_OVERFLOW",
                                "fingerprint source index overflowed",
                            )
                        })?;
                        sum += u64::from(*image.bytes().get(index).ok_or_else(|| {
                            fingerprint_error(
                                "IMAGE_FINGERPRINT_BUFFER_INVALID",
                                "fingerprint source dimensions exceed its byte buffer",
                            )
                        })?);
                        count += 1;
                    }
                }
                if count == 0 {
                    return Err(fingerprint_error(
                        "IMAGE_FINGERPRINT_CELL_EMPTY",
                        "fingerprint grid produced an empty cell",
                    ));
                }
                cells.push(((sum + count / 2) / count) as u8);
            }
        }
        Self::from_cells(cells)
    }

    pub fn parse(value: &str) -> Result<Self, A2dError> {
        let encoded = value.strip_prefix(SERIALIZATION_PREFIX).ok_or_else(|| {
            fingerprint_error(
                "IMAGE_FINGERPRINT_VERSION_UNSUPPORTED",
                "perceptual fingerprint has an unsupported version or algorithm",
            )
        })?;
        if encoded.len() != PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT * 2 {
            return Err(fingerprint_error(
                "IMAGE_FINGERPRINT_LENGTH_INVALID",
                format!(
                    "perceptual fingerprint payload must contain {} hexadecimal characters",
                    PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT * 2
                ),
            ));
        }
        let mut cells = Vec::with_capacity(PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT);
        for pair in encoded.as_bytes().chunks_exact(2) {
            let high = decode_hex(pair[0])?;
            let low = decode_hex(pair[1])?;
            cells.push((high << 4) | low);
        }
        Self::from_cells(cells)
    }

    pub fn encode(&self) -> String {
        let mut encoded =
            String::with_capacity(SERIALIZATION_PREFIX.len() + self.cells.len() * 2);
        encoded.push_str(SERIALIZATION_PREFIX);
        for cell in &self.cells {
            use std::fmt::Write as _;
            write!(&mut encoded, "{cell:02x}")
                .expect("writing hexadecimal bytes into a String cannot fail");
        }
        encoded
    }

    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    pub fn difference(&self, other: &Self) -> PerceptualFingerprintDifference {
        let cell_absolute_differences = self
            .cells
            .iter()
            .zip(&other.cells)
            .map(|(left, right)| left.abs_diff(*right))
            .collect::<Vec<_>>();
        let total = cell_absolute_differences
            .iter()
            .map(|value| u64::from(*value))
            .sum::<u64>();
        let maximum_absolute_difference = cell_absolute_differences
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        PerceptualFingerprintDifference {
            mean_absolute_difference: total as f32 / cell_absolute_differences.len() as f32,
            maximum_absolute_difference,
            cell_absolute_differences,
        }
    }

    fn from_cells(cells: Vec<u8>) -> Result<Self, A2dError> {
        if cells.len() != PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT {
            return Err(fingerprint_error(
                "IMAGE_FINGERPRINT_CELL_COUNT_INVALID",
                format!(
                    "perceptual fingerprint requires {} cells, got {}",
                    PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT,
                    cells.len()
                ),
            ));
        }
        Ok(Self { cells })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PerceptualFingerprintDifference {
    pub mean_absolute_difference: f32,
    pub maximum_absolute_difference: u8,
    pub cell_absolute_differences: Vec<u8>,
}

fn decode_hex(value: u8) -> Result<u8, A2dError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(fingerprint_error(
            "IMAGE_FINGERPRINT_HEX_INVALID",
            "perceptual fingerprint contains a non-hexadecimal character",
        )),
    }
}

fn fingerprint_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.image.fingerprint",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageRotation;

    fn image_with_cells(cells: impl Fn(usize, usize) -> u8) -> OwnedGrayImage {
        let cell_width = 10;
        let cell_height = 10;
        let width = PERCEPTUAL_FINGERPRINT_V1_WIDTH * cell_width;
        let height = PERCEPTUAL_FINGERPRINT_V1_HEIGHT * cell_height;
        let mut bytes = vec![0_u8; width * height];
        for grid_y in 0..PERCEPTUAL_FINGERPRINT_V1_HEIGHT {
            for grid_x in 0..PERCEPTUAL_FINGERPRINT_V1_WIDTH {
                let value = cells(grid_x, grid_y);
                for y in grid_y * cell_height..(grid_y + 1) * cell_height {
                    for x in grid_x * cell_width..(grid_x + 1) * cell_width {
                        bytes[y * width + x] = value;
                    }
                }
            }
        }
        OwnedGrayImage::from_tight(
            width as u32,
            height as u32,
            ImageRotation::Degrees0,
            bytes,
        )
        .unwrap()
    }

    #[test]
    fn uniform_rectified_page_has_deterministic_grid() {
        let fingerprint =
            PerceptualFingerprintV1::from_gray_image(&image_with_cells(|_, _| 180)).unwrap();
        assert_eq!(
            fingerprint.cells(),
            vec![180; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT]
        );
    }

    #[test]
    fn serialization_is_versioned_strict_and_round_trips() {
        let fingerprint = PerceptualFingerprintV1::from_gray_image(&image_with_cells(|x, y| {
            ((x + y * PERCEPTUAL_FINGERPRINT_V1_WIDTH) % 256) as u8
        }))
        .unwrap();
        let encoded = fingerprint.encode();
        assert!(encoded.starts_with(SERIALIZATION_PREFIX));
        assert_eq!(PerceptualFingerprintV1::parse(&encoded).unwrap(), fingerprint);
        assert!(PerceptualFingerprintV1::parse("legacy:00").is_err());
        assert!(
            PerceptualFingerprintV1::parse(&format!("{SERIALIZATION_PREFIX}zz")).is_err()
        );
    }

    #[test]
    fn localized_edit_remains_localized_in_difference_grid() {
        let baseline =
            PerceptualFingerprintV1::from_gray_image(&image_with_cells(|_, _| 220)).unwrap();
        let edited = PerceptualFingerprintV1::from_gray_image(&image_with_cells(|x, y| {
            if x == 5 && y == 7 { 20 } else { 220 }
        }))
        .unwrap();
        let difference = baseline.difference(&edited);
        assert_eq!(difference.maximum_absolute_difference, 200);
        assert_eq!(
            difference
                .cell_absolute_differences
                .iter()
                .filter(|value| **value != 0)
                .count(),
            1
        );
        assert!(
            (difference.mean_absolute_difference
                - 200.0 / PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT as f32)
                .abs()
                < f32::EPSILON
        );
    }
}
''')

replace_once(
    lib_path,
    "mod error;\nmod input;\n",
    "mod error;\nmod fingerprint;\nmod input;\n",
)
replace_once(
    lib_path,
    "pub use encoded::{\n    EncodedImage, EncodedImageFormat, EncodedImageLimits, OwnedGrayImage, OwnedRgbImage,\n};\n",
    "pub use encoded::{\n    EncodedImage, EncodedImageFormat, EncodedImageLimits, OwnedGrayImage, OwnedRgbImage,\n};\npub use fingerprint::{\n    PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT, PERCEPTUAL_FINGERPRINT_V1_HEIGHT,\n    PERCEPTUAL_FINGERPRINT_V1_WIDTH, PerceptualFingerprintDifference,\n    PerceptualFingerprintV1,\n};\n",
)

replace_once(
    milestone_path,
    "    OwnedGrayImage, OwnedRgbImage, ProcessingCancellation, RectificationLimits, RectificationPlan,\n",
    "    OwnedGrayImage, OwnedRgbImage, PerceptualFingerprintV1, ProcessingCancellation,\n    RectificationLimits, RectificationPlan,\n",
)
replace_once(
    milestone_path,
    "    quality: GrayQualityMetrics,\n}\n",
    "    quality: GrayQualityMetrics,\n    perceptual_fingerprint: PerceptualFingerprintV1,\n}\n",
)
replace_once(
    milestone_path,
    '            format!("exact-sha256-v1:{}", corrected.sha256),\n',
    '''            format!(
                "scan-content-v1;corrected-sha256={};perceptual={}",
                corrected.sha256,
                processed.perceptual_fingerprint.encode()
            ),
''',
)
replace_once(
    milestone_path,
    '''    )?)
    .process(&source, &rectification, &ProcessingCancellation::active())?;

    Ok(ProcessedCapture {
''',
    '''    )?)
    .process(&source, &rectification, &ProcessingCancellation::active())?;
    let perceptual_fingerprint =
        PerceptualFingerprintV1::from_gray_image(&derived.ocr_optimized)?;

    Ok(ProcessedCapture {
''',
)
replace_once(
    milestone_path,
    '''        resolved_markers,
        quality,
    })
''',
    '''        resolved_markers,
        quality,
        perceptual_fingerprint,
    })
''',
)

replace_once(
    tests_path,
    '''    let scan = storage.get_scan(&registered.scan_id).unwrap().unwrap();
    assert!(scan.preferred);
''',
    '''    let scan = storage.get_scan(&registered.scan_id).unwrap().unwrap();
    assert!(scan.preferred);
    assert!(
        scan.content_fingerprint
            .starts_with("scan-content-v1;corrected-sha256=")
    );
    assert!(
        scan.content_fingerprint
            .contains(";perceptual=mean-grid-16x24-v1:")
    );
''',
)

replace_once(
    todo_path,
    "- [ ] Cryptographic asset hash.\n- [ ] Versioned perceptual fingerprint.\n",
    '''- [x] Cryptographic asset hash. The immutable corrected asset's verified SHA-256 is embedded in the
      versioned scan content fingerprint.
- [x] Versioned perceptual fingerprint. Rust stores a deterministic `mean-grid-16x24-v1` luminance
      signature derived from the aligned, contrast-normalized OCR image. The representation exposes
      raw per-cell differences only; it does not invent duplicate/revision thresholds.
''',
)
