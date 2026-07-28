use std::fs;
use std::path::{Path, PathBuf};

use a2d_domain::LayoutId;
use a2d_identity::qr::parse;
use serde::Deserialize;

#[derive(Deserialize)]
struct ValidVector {
    name: String,
    payload_text: String,
}

#[derive(Deserialize)]
struct MalformedVector {
    name: String,
    payload_text: String,
    expected_error: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/qr/v1")
}

fn known_layout(layout: &LayoutId) -> bool {
    matches!(
        layout.to_string().as_str(),
        "DEV-PAGE-V1" | "SP-A4-BLANK-V1" | "SP-LETTER-GRAPH-V1" | "ABCDEFGHIJKLMNOPQRST"
    )
}

fn read_json<T: for<'de> Deserialize<'de>>(name: &str) -> Vec<T> {
    serde_json::from_slice(&fs::read(root().join(name)).unwrap()).unwrap()
}

fn decode_png(path: &Path) -> String {
    let image = image::open(path).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(image);
    let grids = prepared.detect_grids();
    assert_eq!(
        grids.len(),
        1,
        "{} must contain exactly one QR",
        path.display()
    );
    grids[0].decode().unwrap().1
}

#[test]
fn every_valid_golden_vector_round_trips_through_png_and_the_canonical_parser() {
    for file in [
        "notebook_setup_vectors.json",
        "notebook_page_vectors.json",
        "smart_page_vectors.json",
    ] {
        for vector in read_json::<ValidVector>(file) {
            let rendered = root().join("rendered").join(format!("{}.png", vector.name));
            let decoded = decode_png(&rendered);
            assert_eq!(decoded, vector.payload_text, "vector {}", vector.name);
            let parsed = parse(&decoded, known_layout).unwrap();
            assert_eq!(
                parsed.encode().unwrap(),
                vector.payload_text,
                "vector {}",
                vector.name
            );
        }
    }
}

#[test]
fn every_malformed_golden_vector_returns_its_stable_expected_error() {
    for vector in read_json::<MalformedVector>("malformed_vectors.json") {
        let error = parse(&vector.payload_text, known_layout).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            vector.expected_error,
            "malformed vector {}",
            vector.name
        );
    }
}
