#!/usr/bin/env python3
"""Verify the deterministic synthetic scan corpus and its manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from PIL import Image, UnidentifiedImageError

REQUIRED_CATEGORIES = {
    "generated",
    "glare",
    "blur",
    "missing-marker",
    "wrong-layout",
    "duplicate",
    "revisions",
    "corrupted",
}
REQUIRED_QUALITY_STATES = {
    "Accepted",
    "AcceptedWithWarnings",
    "NeedsReview",
    "Rejected",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fixture-dir",
        type=Path,
        default=Path("fixtures/scans"),
    )
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def verify_fixture(root: Path, fixture: dict[str, Any]) -> None:
    fixture_id = fixture.get("id")
    relative_path = fixture.get("path")
    require(isinstance(fixture_id, str) and fixture_id, "fixture is missing a non-empty id")
    require(isinstance(relative_path, str) and relative_path, f"{fixture_id}: missing path")
    require(".." not in Path(relative_path).parts, f"{fixture_id}: path traversal is forbidden")

    path = root / relative_path
    require(path.is_file(), f"{fixture_id}: missing file {path}")
    require(fixture.get("source") == "project-generated", f"{fixture_id}: wrong source")
    require(fixture.get("license") == "Apache-2.0", f"{fixture_id}: wrong license")
    require(
        fixture.get("intended_quality_state") in REQUIRED_QUALITY_STATES,
        f"{fixture_id}: invalid intended quality state",
    )
    require(path.stat().st_size == fixture.get("byte_length"), f"{fixture_id}: byte length drift")
    require(sha256(path) == fixture.get("sha256"), f"{fixture_id}: SHA-256 drift")

    image_decode = fixture.get("image_decode")
    require(image_decode in {"success", "failure"}, f"{fixture_id}: invalid image_decode")
    if image_decode == "success":
        try:
            with Image.open(path) as image:
                image.load()
                require(image.width == fixture.get("width"), f"{fixture_id}: width drift")
                require(image.height == fixture.get("height"), f"{fixture_id}: height drift")
                require(image.mode == fixture.get("mode"), f"{fixture_id}: mode drift")
        except (OSError, UnidentifiedImageError) as error:
            raise SystemExit(f"{fixture_id}: expected image decode success: {error}") from error
    else:
        try:
            with Image.open(path) as image:
                image.load()
        except (OSError, UnidentifiedImageError):
            return
        raise SystemExit(f"{fixture_id}: expected image decode failure")


def main() -> None:
    args = parse_args()
    root = args.fixture_dir.resolve()
    manifest_path = root / "manifest.json"
    require(manifest_path.is_file(), f"missing manifest: {manifest_path}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    require(manifest.get("schema_version") == 1, "unsupported fixture manifest schema")
    generator = manifest.get("generator", {})
    require(generator.get("name") == "a2d-synthetic-scan-generator", "unexpected generator")
    require(generator.get("version") == 1, "unexpected generator version")
    require(generator.get("deterministic") is True, "generator must be deterministic")

    provenance = manifest.get("provenance", {})
    require(provenance.get("source") == "project-generated", "wrong corpus source")
    require(provenance.get("license") == "Apache-2.0", "wrong corpus license")
    require(provenance.get("photographed") is False, "synthetic corpus cannot claim photographed provenance")

    fixtures = manifest.get("fixtures")
    require(isinstance(fixtures, list) and fixtures, "manifest contains no fixtures")
    ids: set[str] = set()
    paths: set[str] = set()
    categories: set[str] = set()
    for fixture in fixtures:
        require(isinstance(fixture, dict), "fixture entry must be an object")
        fixture_id = fixture.get("id")
        relative_path = fixture.get("path")
        require(fixture_id not in ids, f"duplicate fixture id: {fixture_id}")
        require(relative_path not in paths, f"duplicate fixture path: {relative_path}")
        ids.add(fixture_id)
        paths.add(relative_path)
        categories.add(fixture.get("category"))
        verify_fixture(root, fixture)

    missing_categories = REQUIRED_CATEGORIES - categories
    require(not missing_categories, f"missing fixture categories: {sorted(missing_categories)}")
    require((root / "photographed").is_dir(), "photographed fixture directory is missing")
    print(f"verified {len(fixtures)} synthetic scan fixtures")


if __name__ == "__main__":
    main()
