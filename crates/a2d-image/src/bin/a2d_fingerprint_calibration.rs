use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{Error as IoError, ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

use a2d_image::{
    EncodedImage, EncodedImageFormat, EncodedImageLimits, ImageLimits, ImageRotation,
    PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT, PerceptualFingerprintV1,
};

const MAX_ENCODED_BYTES: usize = 24 * 1024 * 1024;
const MAX_DECODED_PIXELS: u64 = 32_000_000;
const MAX_DECODED_BYTES: u64 = 96_000_000;
const CALIBRATION_INPUT_HEADER: &str = "pair_id\texpected_relation\tbaseline_fixture_id\tbaseline_normalized_ocr_path\tbaseline_pipeline_version\tcandidate_fixture_id\tcandidate_normalized_ocr_path\tcandidate_pipeline_version";
const EXPECTED_RELATIONS: &[&str] = &[
    "near_duplicate",
    "revision",
    "substantially_different",
];

type DynError = Box<dyn Error>;

#[derive(Debug, Eq, PartialEq)]
struct FixtureSource {
    path: PathBuf,
    pipeline_version: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct ComparisonPair {
    id: String,
    baseline_fixture_id: String,
    candidate_fixture_id: String,
    expected_relation: String,
}

struct CalibrationInput {
    fixtures: BTreeMap<String, FixtureSource>,
    pairs: Vec<ComparisonPair>,
}

fn usage() -> ! {
    eprintln!(
        "usage: a2d-fingerprint-calibration <photographed-root> <validated-input.tsv> <output.tsv>"
    );
    std::process::exit(2)
}

fn invalid(message: impl Into<String>) -> DynError {
    Box::new(IoError::new(ErrorKind::InvalidData, message.into()))
}

fn validate_field(value: &str, context: &str) -> Result<(), DynError> {
    if value.is_empty() {
        return Err(invalid(format!("{context}: value must not be empty")));
    }
    if value
        .bytes()
        .any(|byte| matches!(byte, b'\t' | b'\r' | b'\n'))
    {
        return Err(invalid(format!(
            "{context}: value contains control whitespace"
        )));
    }
    Ok(())
}

fn resolve_confined_file(
    root: &Path,
    relative_path: &str,
    context: &str,
) -> Result<PathBuf, DynError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(format!(
            "{context}: path must contain only relative normal components"
        )));
    }
    let unresolved = root.join(relative);
    let resolved = unresolved.canonicalize().map_err(|error| {
        invalid(format!(
            "{context}: failed to resolve {}: {error}",
            unresolved.display()
        ))
    })?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(invalid(format!(
            "{context}: path is not a file confined to {}",
            root.display()
        )));
    }
    Ok(resolved)
}

fn insert_fixture_source(
    fixtures: &mut BTreeMap<String, FixtureSource>,
    fixture_id: &str,
    relative_path: &str,
    pipeline_version: u64,
    root: &Path,
    context: &str,
) -> Result<(), DynError> {
    validate_field(fixture_id, context)?;
    validate_field(relative_path, context)?;
    if pipeline_version == 0 {
        return Err(invalid(format!(
            "{context}: pipeline version must be positive"
        )));
    }
    let source = FixtureSource {
        path: resolve_confined_file(root, relative_path, context)?,
        pipeline_version,
    };
    match fixtures.entry(fixture_id.to_string()) {
        Entry::Vacant(entry) => {
            entry.insert(source);
            Ok(())
        }
        Entry::Occupied(entry) if entry.get() == &source => Ok(()),
        Entry::Occupied(_) => Err(invalid(format!(
            "{context}: fixture {fixture_id} maps to inconsistent evidence"
        ))),
    }
}

fn parse_pipeline_version(value: &str, context: &str) -> Result<u64, DynError> {
    let parsed = value.parse::<u64>().map_err(|error| {
        invalid(format!(
            "{context}: invalid pipeline version {value:?}: {error}"
        ))
    })?;
    if parsed == 0 {
        return Err(invalid(format!(
            "{context}: pipeline version must be positive"
        )));
    }
    Ok(parsed)
}

fn parse_calibration_input(root: &Path, input_path: &Path) -> Result<CalibrationInput, DynError> {
    let input = fs::read_to_string(input_path)?;
    let mut lines = input.lines();
    let header = lines
        .next()
        .ok_or_else(|| invalid("calibration input is empty"))?;
    if header != CALIBRATION_INPUT_HEADER {
        return Err(invalid("unsupported calibration input header or schema"));
    }

    let mut fixtures = BTreeMap::new();
    let mut pairs = Vec::new();
    let mut pair_ids = BTreeSet::new();
    let mut unordered_pairs = BTreeSet::new();
    for (offset, line) in lines.enumerate() {
        let line_number = offset + 2;
        if line.is_empty() {
            return Err(invalid(format!(
                "calibration input line {line_number} is empty"
            )));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 8 {
            return Err(invalid(format!(
                "calibration input line {line_number} must contain exactly 8 fields"
            )));
        }
        let pair_id = fields[0];
        let expected_relation = fields[1];
        let baseline_fixture_id = fields[2];
        let baseline_path = fields[3];
        let baseline_pipeline_version =
            parse_pipeline_version(fields[4], &format!("line {line_number} baseline"))?;
        let candidate_fixture_id = fields[5];
        let candidate_path = fields[6];
        let candidate_pipeline_version =
            parse_pipeline_version(fields[7], &format!("line {line_number} candidate"))?;

        validate_field(pair_id, &format!("line {line_number} pair id"))?;
        validate_field(
            expected_relation,
            &format!("line {line_number} expected relation"),
        )?;
        if !EXPECTED_RELATIONS.contains(&expected_relation) {
            return Err(invalid(format!(
                "line {line_number}: unsupported expected relation {expected_relation:?}"
            )));
        }
        if baseline_fixture_id == candidate_fixture_id {
            return Err(invalid(format!(
                "line {line_number}: a fixture cannot be compared with itself"
            )));
        }
        if !pair_ids.insert(pair_id.to_string()) {
            return Err(invalid(format!("duplicate comparison pair id: {pair_id}")));
        }
        let mut unordered_key = [
            baseline_fixture_id.to_string(),
            candidate_fixture_id.to_string(),
        ];
        unordered_key.sort();
        if !unordered_pairs.insert(unordered_key) {
            return Err(invalid(format!(
                "duplicate unordered comparison pair: {baseline_fixture_id} / {candidate_fixture_id}"
            )));
        }

        insert_fixture_source(
            &mut fixtures,
            baseline_fixture_id,
            baseline_path,
            baseline_pipeline_version,
            root,
            &format!("line {line_number} baseline"),
        )?;
        insert_fixture_source(
            &mut fixtures,
            candidate_fixture_id,
            candidate_path,
            candidate_pipeline_version,
            root,
            &format!("line {line_number} candidate"),
        )?;
        pairs.push(ComparisonPair {
            id: pair_id.to_string(),
            baseline_fixture_id: baseline_fixture_id.to_string(),
            candidate_fixture_id: candidate_fixture_id.to_string(),
            expected_relation: expected_relation.to_string(),
        });
    }

    if pairs.is_empty() {
        return Err(invalid(
            "photographed calibration requires at least one labeled pair",
        ));
    }
    Ok(CalibrationInput { fixtures, pairs })
}

fn decode_normalized_fingerprint(path: &Path) -> Result<PerceptualFingerprintV1, DynError> {
    let bytes = fs::read(path)?;
    let image = EncodedImage::new(
        &bytes,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        EncodedImageLimits::new(
            MAX_ENCODED_BYTES,
            MAX_DECODED_PIXELS,
            MAX_DECODED_BYTES,
        )?,
    )?
    .decode_rgb8()?
    .into_gray8(ImageLimits::new(MAX_DECODED_PIXELS)?)?;
    PerceptualFingerprintV1::from_gray_image(&image)
        .map_err(|error| Box::new(error) as DynError)
}

fn nearest_rank_percentile(sorted_values: &[u8], percentile: usize) -> u8 {
    let rank = (percentile * sorted_values.len()).div_ceil(100).max(1);
    sorted_values[rank - 1]
}

fn histogram_string(values: &[u8]) -> String {
    let mut counts = [0_u32; 256];
    for value in values {
        counts[usize::from(*value)] += 1;
    }
    let mut encoded = String::new();
    for (value, count) in counts
        .into_iter()
        .enumerate()
        .filter(|(_, count)| *count > 0)
    {
        if !encoded.is_empty() {
            encoded.push(',');
        }
        write!(&mut encoded, "{value}:{count}")
            .expect("writing a histogram into a String cannot fail");
    }
    encoded
}

fn cell_differences_hex(values: &[u8]) -> String {
    let mut encoded = String::with_capacity(values.len() * 2);
    for value in values {
        write!(&mut encoded, "{value:02x}")
            .expect("writing hexadecimal bytes into a String cannot fail");
    }
    encoded
}

fn write_report(
    output_path: &Path,
    fixture_sources: &BTreeMap<String, FixtureSource>,
    pairs: &[ComparisonPair],
) -> Result<(), DynError> {
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut fingerprints = BTreeMap::new();
    for (fixture_id, source) in fixture_sources {
        fingerprints.insert(
            fixture_id.clone(),
            decode_normalized_fingerprint(&source.path)?,
        );
    }

    let mut report = String::from(
        "pair_id\texpected_relation\tbaseline_fixture_id\tcandidate_fixture_id\tbaseline_pipeline_version\tcandidate_pipeline_version\tmean_absolute_difference\tmaximum_absolute_difference\tnonzero_cell_count\tp50_absolute_difference\tp90_absolute_difference\tp95_absolute_difference\tp99_absolute_difference\tdifference_histogram\tcell_absolute_differences_hex\n",
    );
    for pair in pairs {
        let baseline = fingerprints.get(&pair.baseline_fixture_id).ok_or_else(|| {
            invalid(format!(
                "comparison pair {} references unknown baseline fingerprint {}",
                pair.id, pair.baseline_fixture_id
            ))
        })?;
        let candidate = fingerprints.get(&pair.candidate_fixture_id).ok_or_else(|| {
            invalid(format!(
                "comparison pair {} references unknown candidate fingerprint {}",
                pair.id, pair.candidate_fixture_id
            ))
        })?;
        let baseline_source = fixture_sources
            .get(&pair.baseline_fixture_id)
            .ok_or_else(|| {
                invalid(format!(
                    "comparison pair {} references unknown baseline fixture {}",
                    pair.id, pair.baseline_fixture_id
                ))
            })?;
        let candidate_source = fixture_sources
            .get(&pair.candidate_fixture_id)
            .ok_or_else(|| {
                invalid(format!(
                    "comparison pair {} references unknown candidate fixture {}",
                    pair.id, pair.candidate_fixture_id
                ))
            })?;
        let difference = baseline.difference(candidate);
        if difference.cell_absolute_differences.len() != PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT {
            return Err(invalid(format!(
                "comparison pair {} produced an invalid difference grid",
                pair.id
            )));
        }
        let nonzero_cell_count = difference
            .cell_absolute_differences
            .iter()
            .filter(|value| **value != 0)
            .count();
        let mut sorted = difference.cell_absolute_differences.clone();
        sorted.sort_unstable();
        writeln!(
            report,
            "{}\t{}\t{}\t{}\t{}\t{}\t{:.12}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            pair.id,
            pair.expected_relation,
            pair.baseline_fixture_id,
            pair.candidate_fixture_id,
            baseline_source.pipeline_version,
            candidate_source.pipeline_version,
            difference.mean_absolute_difference,
            difference.maximum_absolute_difference,
            nonzero_cell_count,
            nearest_rank_percentile(&sorted, 50),
            nearest_rank_percentile(&sorted, 90),
            nearest_rank_percentile(&sorted, 95),
            nearest_rank_percentile(&sorted, 99),
            histogram_string(&difference.cell_absolute_differences),
            cell_differences_hex(&difference.cell_absolute_differences),
        )?;
    }

    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    output.write_all(report.as_bytes())?;
    output.flush()?;
    output.sync_all()?;
    println!(
        "wrote {} photographed fingerprint comparisons to {}",
        pairs.len(),
        output_path.display()
    );
    Ok(())
}

fn run(root: &Path, input_path: &Path, output_path: &Path) -> Result<(), DynError> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(invalid(format!(
            "photographed root is not a directory: {}",
            root.display()
        )));
    }
    let input_path = input_path.canonicalize()?;
    let input = parse_calibration_input(&root, &input_path)?;
    write_report(output_path, &input.fixtures, &input.pairs)
}

fn main() -> Result<(), DynError> {
    let mut args = env::args_os().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    let input_path = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    let output_path = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    if args.next().is_some() {
        usage();
    }
    run(&root, &input_path, &output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles_are_deterministic() {
        let values = (0_u8..=99).collect::<Vec<_>>();
        assert_eq!(nearest_rank_percentile(&values, 50), 49);
        assert_eq!(nearest_rank_percentile(&values, 90), 89);
        assert_eq!(nearest_rank_percentile(&values, 99), 98);
    }

    #[test]
    fn histogram_and_hex_preserve_every_difference() {
        let values = [0_u8, 0, 1, 16, 255];
        assert_eq!(histogram_string(&values), "0:2,1:1,16:1,255:1");
        assert_eq!(cell_differences_hex(&values), "00000110ff");
    }

    #[test]
    fn invalid_pipeline_versions_are_rejected() {
        assert!(parse_pipeline_version("0", "test").is_err());
        assert!(parse_pipeline_version("not-a-number", "test").is_err());
    }
}
