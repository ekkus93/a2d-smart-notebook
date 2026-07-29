use std::collections::{BTreeMap, BTreeSet};
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
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const MAX_ENCODED_BYTES: usize = 24 * 1024 * 1024;
const MAX_DECODED_PIXELS: u64 = 32_000_000;
const MAX_DECODED_BYTES: u64 = 96_000_000;
const EXPECTED_RELATIONS: &[&str] = &[
    "near_duplicate",
    "revision",
    "substantially_different",
];

type DynError = Box<dyn Error>;

struct FixtureEvidence {
    pipeline_version: u64,
    fingerprint: PerceptualFingerprintV1,
}

struct ComparisonPair {
    id: String,
    baseline_fixture_id: String,
    candidate_fixture_id: String,
    expected_relation: String,
}

fn usage() -> ! {
    eprintln!(
        "usage: a2d-fingerprint-calibration <photographed-manifest.json> <output.tsv>"
    );
    std::process::exit(2)
}

fn invalid(message: impl Into<String>) -> DynError {
    Box::new(IoError::new(ErrorKind::InvalidData, message.into()))
}

fn required_object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, DynError> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{context}: expected an object")))
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a [Value], DynError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(format!("{context}: {field} must be an array")))
}

fn required_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, DynError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid(format!("{context}: missing non-empty {field}")))?;
    if value.contains(['\t', '\r', '\n']) {
        return Err(invalid(format!(
            "{context}: {field} contains control whitespace"
        )));
    }
    Ok(value)
}

fn required_u64(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<u64, DynError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid(format!("{context}: {field} must be a positive integer")))
}

fn required_true(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<(), DynError> {
    if object.get(field).and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(invalid(format!("{context}: {field} must be true")))
    }
}

fn resolve_confined_file(root: &Path, relative_path: &str, context: &str) -> Result<PathBuf, DynError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            !matches!(component, Component::Normal(_))
        })
    {
        return Err(invalid(format!(
            "{context}: path must contain only relative normal components"
        )));
    }
    let resolved = root.join(relative).canonicalize().map_err(|error| {
        invalid(format!(
            "{context}: failed to resolve {}: {error}",
            root.join(relative).display()
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

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn decode_normalized_fingerprint(path: &Path, bytes: &[u8]) -> Result<PerceptualFingerprintV1, DynError> {
    let limits = EncodedImageLimits::new(
        MAX_ENCODED_BYTES,
        MAX_DECODED_PIXELS,
        MAX_DECODED_BYTES,
    )?;
    let image = EncodedImage::new(
        bytes,
        EncodedImageFormat::Png,
        ImageRotation::Degrees0,
        limits,
    )?
    .decode_rgb8()?
    .into_gray8(ImageLimits::new(MAX_DECODED_PIXELS)?)?;
    PerceptualFingerprintV1::from_gray_image(&image).map_err(|error| {
        invalid(format!(
            "failed to fingerprint normalized OCR image {}: {error}",
            path.display()
        ))
    })
}

fn load_fixture(
    root: &Path,
    value: &Value,
) -> Result<(String, FixtureEvidence), DynError> {
    let fixture = required_object(value, "fixture")?;
    let fixture_id = required_text(fixture, "id", "fixture")?.to_string();
    let context = format!("fixture {fixture_id}");
    required_true(fixture, "photographed", &context)?;
    if required_text(fixture, "normalized_ocr_format", &context)? != "png" {
        return Err(invalid(format!(
            "{context}: normalized_ocr_format must be png"
        )));
    }
    if fixture
        .get("normalized_rotation_degrees")
        .and_then(Value::as_u64)
        != Some(0)
    {
        return Err(invalid(format!(
            "{context}: normalized OCR evidence must be upright"
        )));
    }
    let pipeline_version = required_u64(fixture, "pipeline_version", &context)?;
    let relative_path = required_text(fixture, "normalized_ocr_path", &context)?;
    let expected_byte_length = required_u64(fixture, "normalized_ocr_byte_length", &context)?;
    let expected_sha256 = required_text(fixture, "normalized_ocr_sha256", &context)?;
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(format!(
            "{context}: normalized_ocr_sha256 must be 64 lowercase hexadecimal characters"
        )));
    }

    let path = resolve_confined_file(root, relative_path, &context)?;
    let bytes = fs::read(&path)?;
    if u64::try_from(bytes.len())? != expected_byte_length {
        return Err(invalid(format!(
            "{context}: normalized OCR byte length drifted"
        )));
    }
    if sha256_hex(&bytes) != expected_sha256 {
        return Err(invalid(format!(
            "{context}: normalized OCR SHA-256 drifted"
        )));
    }
    let fingerprint = decode_normalized_fingerprint(&path, &bytes)?;
    Ok((
        fixture_id,
        FixtureEvidence {
            pipeline_version,
            fingerprint,
        },
    ))
}

fn load_pair(value: &Value) -> Result<ComparisonPair, DynError> {
    let pair = required_object(value, "comparison pair")?;
    let id = required_text(pair, "id", "comparison pair")?.to_string();
    let context = format!("comparison pair {id}");
    let baseline_fixture_id = required_text(pair, "baseline_fixture_id", &context)?.to_string();
    let candidate_fixture_id = required_text(pair, "candidate_fixture_id", &context)?.to_string();
    if baseline_fixture_id == candidate_fixture_id {
        return Err(invalid(format!(
            "{context}: a fixture cannot be compared with itself"
        )));
    }
    let expected_relation = required_text(pair, "expected_relation", &context)?.to_string();
    if !EXPECTED_RELATIONS.contains(&expected_relation.as_str()) {
        return Err(invalid(format!(
            "{context}: unsupported expected_relation"
        )));
    }
    Ok(ComparisonPair {
        id,
        baseline_fixture_id,
        candidate_fixture_id,
        expected_relation,
    })
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
    for (value, count) in counts.into_iter().enumerate().filter(|(_, count)| *count > 0) {
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
    fixtures: &BTreeMap<String, FixtureEvidence>,
    pairs: &[ComparisonPair],
) -> Result<(), DynError> {
    if output_path.exists() {
        return Err(invalid(format!(
            "refusing to overwrite existing calibration report {}",
            output_path.display()
        )));
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut report = String::from(
        "pair_id\texpected_relation\tbaseline_fixture_id\tcandidate_fixture_id\tbaseline_pipeline_version\tcandidate_pipeline_version\tmean_absolute_difference\tmaximum_absolute_difference\tnonzero_cell_count\tp50_absolute_difference\tp90_absolute_difference\tp95_absolute_difference\tp99_absolute_difference\tdifference_histogram\tcell_absolute_differences_hex\n",
    );
    for pair in pairs {
        let baseline = fixtures.get(&pair.baseline_fixture_id).ok_or_else(|| {
            invalid(format!(
                "comparison pair {} references unknown baseline fixture {}",
                pair.id, pair.baseline_fixture_id
            ))
        })?;
        let candidate = fixtures.get(&pair.candidate_fixture_id).ok_or_else(|| {
            invalid(format!(
                "comparison pair {} references unknown candidate fixture {}",
                pair.id, pair.candidate_fixture_id
            ))
        })?;
        let difference = baseline.fingerprint.difference(&candidate.fingerprint);
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
            baseline.pipeline_version,
            candidate.pipeline_version,
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

    let partial_path = output_path.with_extension("partial");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial_path)?;
    output.write_all(report.as_bytes())?;
    output.flush()?;
    output.sync_all()?;
    drop(output);
    fs::rename(&partial_path, output_path)?;
    println!(
        "wrote {} photographed fingerprint comparisons to {}",
        pairs.len(),
        output_path.display()
    );
    Ok(())
}

fn run(manifest_path: &Path, output_path: &Path) -> Result<(), DynError> {
    let manifest_path = manifest_path.canonicalize()?;
    let root = manifest_path
        .parent()
        .ok_or_else(|| invalid("manifest path has no parent directory"))?
        .canonicalize()?;
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let manifest = required_object(&manifest, "manifest")?;
    if manifest.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err(invalid("unsupported photographed fixture manifest schema"));
    }
    required_true(manifest, "photographed", "manifest")?;

    let fixture_values = required_array(manifest, "fixtures", "manifest")?;
    let pair_values = required_array(manifest, "comparison_pairs", "manifest")?;
    if fixture_values.is_empty() || pair_values.is_empty() {
        return Err(invalid(
            "photographed calibration requires at least one fixture and one labeled pair",
        ));
    }

    let mut fixtures = BTreeMap::new();
    for value in fixture_values {
        let (fixture_id, evidence) = load_fixture(&root, value)?;
        if fixtures.insert(fixture_id.clone(), evidence).is_some() {
            return Err(invalid(format!(
                "duplicate photographed fixture id: {fixture_id}"
            )));
        }
    }

    let mut pair_ids = BTreeSet::new();
    let mut unordered_pairs = BTreeSet::new();
    let mut pairs = Vec::with_capacity(pair_values.len());
    for value in pair_values {
        let pair = load_pair(value)?;
        if !pair_ids.insert(pair.id.clone()) {
            return Err(invalid(format!(
                "duplicate comparison pair id: {}",
                pair.id
            )));
        }
        let mut key = [
            pair.baseline_fixture_id.clone(),
            pair.candidate_fixture_id.clone(),
        ];
        key.sort();
        if !unordered_pairs.insert(key) {
            return Err(invalid(format!(
                "duplicate unordered comparison pair: {} / {}",
                pair.baseline_fixture_id, pair.candidate_fixture_id
            )));
        }
        pairs.push(pair);
    }

    write_report(output_path, &fixtures, &pairs)
}

fn main() -> Result<(), DynError> {
    let mut args = env::args_os().skip(1);
    let manifest_path = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    let output_path = PathBuf::from(args.next().unwrap_or_else(|| usage()));
    if args.next().is_some() {
        usage();
    }
    run(&manifest_path, &output_path)
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
    fn unknown_relation_is_rejected() {
        let pair = serde_json::json!({
            "id": "pair-1",
            "baseline_fixture_id": "a",
            "candidate_fixture_id": "b",
            "expected_relation": "probably_same"
        });
        assert!(load_pair(&pair).is_err());
    }
}
