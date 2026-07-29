#!/usr/bin/env python3
"""Verify photographed scan fixtures and labeled comparison pairs.

This verifier deliberately does not derive or accept production thresholds. It only proves that
physical evidence is attributable, immutable, internally consistent, and labeled strongly enough
for the Rust calibration reporter to measure it later.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

EXPECTED_RELATIONS = {
    "near_duplicate",
    "revision",
    "substantially_different",
}
HEX_DIGITS = frozenset("0123456789abcdef")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("fixtures/scans/photographed/manifest.json"),
    )
    return parser.parse_args()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def require_object(value: Any, context: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{context}: expected an object")
    return value


def require_array(value: Any, context: str) -> list[Any]:
    require(isinstance(value, list), f"{context}: expected an array")
    return value


def require_text(obj: dict[str, Any], key: str, context: str) -> str:
    value = obj.get(key)
    require(isinstance(value, str) and value.strip(), f"{context}: missing non-empty {key}")
    require("\t" not in value and "\r" not in value and "\n" not in value, f"{context}: {key} contains control whitespace")
    return value


def require_positive_int(obj: dict[str, Any], key: str, context: str) -> int:
    value = obj.get(key)
    require(isinstance(value, int) and not isinstance(value, bool) and value > 0, f"{context}: {key} must be a positive integer")
    return value


def require_positive_number(obj: dict[str, Any], key: str, context: str) -> float:
    value = obj.get(key)
    require(isinstance(value, (int, float)) and not isinstance(value, bool) and value > 0, f"{context}: {key} must be positive")
    return float(value)


def require_sha256(obj: dict[str, Any], key: str, context: str) -> str:
    value = require_text(obj, key, context)
    require(len(value) == 64 and set(value) <= HEX_DIGITS, f"{context}: {key} must be 64 lowercase hexadecimal characters")
    return value


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def resolve_fixture_path(root: Path, relative_path: str, context: str) -> Path:
    relative = Path(relative_path)
    require(not relative.is_absolute(), f"{context}: absolute paths are forbidden")
    require(relative.parts and all(part not in {"", ".", ".."} for part in relative.parts), f"{context}: path traversal or ambiguous components are forbidden")
    resolved = (root / relative).resolve()
    require(resolved.is_relative_to(root), f"{context}: path escapes the photographed fixture root")
    require(resolved.is_file(), f"{context}: missing file {resolved}")
    return resolved


def verify_file(
    root: Path,
    fixture: dict[str, Any],
    path_key: str,
    byte_length_key: str,
    sha256_key: str,
    context: str,
) -> Path:
    relative_path = require_text(fixture, path_key, context)
    path = resolve_fixture_path(root, relative_path, f"{context}.{path_key}")
    expected_length = require_positive_int(fixture, byte_length_key, context)
    expected_sha256 = require_sha256(fixture, sha256_key, context)
    require(path.stat().st_size == expected_length, f"{context}: {path_key} byte length drift")
    require(sha256(path) == expected_sha256, f"{context}: {path_key} SHA-256 drift")
    return path


def verify_fixture(root: Path, value: Any) -> dict[str, Any]:
    fixture = require_object(value, "fixture")
    fixture_id = require_text(fixture, "id", "fixture")
    context = f"fixture {fixture_id}"
    require(fixture.get("photographed") is True, f"{context}: photographed must be true")

    raw_format = require_text(fixture, "raw_capture_format", context)
    require(raw_format in {"jpeg", "png"}, f"{context}: unsupported raw_capture_format")
    normalized_format = require_text(fixture, "normalized_ocr_format", context)
    require(normalized_format == "png", f"{context}: normalized OCR evidence must be PNG")
    require(fixture.get("normalized_rotation_degrees") == 0, f"{context}: normalized OCR evidence must be upright")
    require_positive_int(fixture, "pipeline_version", context)

    verify_file(
        root,
        fixture,
        "raw_capture_path",
        "raw_capture_byte_length",
        "raw_capture_sha256",
        context,
    )
    normalized_path = verify_file(
        root,
        fixture,
        "normalized_ocr_path",
        "normalized_ocr_byte_length",
        "normalized_ocr_sha256",
        context,
    )
    require(normalized_path.suffix.lower() == ".png", f"{context}: normalized OCR path must end in .png")

    source = require_object(fixture.get("source"), f"{context}.source")
    require_text(source, "description", f"{context}.source")
    require_text(source, "consent", f"{context}.source")
    require_text(source, "license", f"{context}.source")
    require_text(source, "attribution", f"{context}.source")

    device = require_object(fixture.get("device"), f"{context}.device")
    require_text(device, "manufacturer", f"{context}.device")
    require_text(device, "model", f"{context}.device")
    require_text(device, "android_version", f"{context}.device")
    require_text(device, "build_fingerprint", f"{context}.device")

    camera = require_object(fixture.get("camera"), f"{context}.camera")
    require_text(camera, "camera_id", f"{context}.camera")
    require_text(camera, "lens_facing", f"{context}.camera")
    require_positive_int(camera, "width_px", f"{context}.camera")
    require_positive_int(camera, "height_px", f"{context}.camera")

    conditions = require_object(fixture.get("conditions"), f"{context}.conditions")
    require_text(conditions, "captured_at_utc", f"{context}.conditions")
    require_text(conditions, "lighting", f"{context}.conditions")
    require_text(conditions, "capture_angle", f"{context}.conditions")
    require_text(conditions, "stabilization", f"{context}.conditions")
    require_positive_number(conditions, "distance_cm", f"{context}.conditions")
    require_text(conditions, "notes", f"{context}.conditions")

    page = require_object(fixture.get("page"), f"{context}.page")
    require_text(page, "page_identity", f"{context}.page")
    require_text(page, "physical_sheet_id", f"{context}.page")
    require_text(page, "content_revision_id", f"{context}.page")
    require_text(page, "print_source", f"{context}.page")
    require_text(page, "paper", f"{context}.page")
    require_text(page, "writing_instrument", f"{context}.page")

    return fixture


def verify_pair(value: Any, fixtures: dict[str, dict[str, Any]]) -> str:
    pair = require_object(value, "comparison pair")
    pair_id = require_text(pair, "id", "comparison pair")
    context = f"comparison pair {pair_id}"
    baseline_id = require_text(pair, "baseline_fixture_id", context)
    candidate_id = require_text(pair, "candidate_fixture_id", context)
    relation = require_text(pair, "expected_relation", context)
    require(relation in EXPECTED_RELATIONS, f"{context}: unsupported expected_relation")
    require(baseline_id != candidate_id, f"{context}: a fixture cannot be compared with itself")
    require(baseline_id in fixtures, f"{context}: unknown baseline fixture {baseline_id}")
    require(candidate_id in fixtures, f"{context}: unknown candidate fixture {candidate_id}")

    baseline_page = require_object(fixtures[baseline_id].get("page"), f"fixture {baseline_id}.page")
    candidate_page = require_object(fixtures[candidate_id].get("page"), f"fixture {candidate_id}.page")
    same_identity = baseline_page["page_identity"] == candidate_page["page_identity"]
    same_sheet = baseline_page["physical_sheet_id"] == candidate_page["physical_sheet_id"]
    same_revision = baseline_page["content_revision_id"] == candidate_page["content_revision_id"]

    require(same_identity, f"{context}: calibration pairs must target the same page identity")
    if relation == "near_duplicate":
        require(same_sheet and same_revision, f"{context}: near_duplicate requires the same physical sheet and content revision")
    elif relation == "revision":
        require(same_sheet and not same_revision, f"{context}: revision requires one physical sheet with different content revisions")
    else:
        require(not (same_sheet and same_revision), f"{context}: substantially_different cannot reuse the same sheet and revision label")

    require_text(pair, "labeling_notes", context)
    return pair_id


def main() -> None:
    args = parse_args()
    manifest_path = args.manifest.resolve()
    require(manifest_path.is_file(), f"missing photographed fixture manifest: {manifest_path}")
    root = manifest_path.parent.resolve()
    manifest = require_object(json.loads(manifest_path.read_text(encoding="utf-8")), "manifest")

    require(manifest.get("schema_version") == 1, "unsupported photographed fixture manifest schema")
    require(manifest.get("photographed") is True, "photographed fixture manifest must set photographed=true")
    fixture_values = require_array(manifest.get("fixtures"), "manifest.fixtures")
    pair_values = require_array(manifest.get("comparison_pairs"), "manifest.comparison_pairs")

    fixtures: dict[str, dict[str, Any]] = {}
    raw_paths: set[str] = set()
    normalized_paths: set[str] = set()
    for value in fixture_values:
        fixture = verify_fixture(root, value)
        fixture_id = fixture["id"]
        require(fixture_id not in fixtures, f"duplicate photographed fixture id: {fixture_id}")
        raw_path = fixture["raw_capture_path"]
        normalized_path = fixture["normalized_ocr_path"]
        require(raw_path not in raw_paths, f"duplicate raw capture path: {raw_path}")
        require(normalized_path not in normalized_paths, f"duplicate normalized OCR path: {normalized_path}")
        fixtures[fixture_id] = fixture
        raw_paths.add(raw_path)
        normalized_paths.add(normalized_path)

    pair_ids: set[str] = set()
    pair_keys: set[tuple[str, str]] = set()
    for value in pair_values:
        pair_id = verify_pair(value, fixtures)
        pair = require_object(value, f"comparison pair {pair_id}")
        key = tuple(sorted((pair["baseline_fixture_id"], pair["candidate_fixture_id"])))
        require(pair_id not in pair_ids, f"duplicate comparison pair id: {pair_id}")
        require(key not in pair_keys, f"duplicate unordered comparison pair: {key[0]} / {key[1]}")
        pair_ids.add(pair_id)
        pair_keys.add(key)

    if pair_values:
        print(f"verified {len(fixtures)} photographed fixtures and {len(pair_values)} labeled comparison pairs")
    else:
        print("verified photographed fixture manifest; no photographed calibration pairs are committed")


if __name__ == "__main__":
    main()
