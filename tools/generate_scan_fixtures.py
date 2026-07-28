#!/usr/bin/env python3
"""Generate deterministic synthetic scan fixtures for Milestone 7.8.

The official AprilTag and canonical A2D QR rasters are produced first by the
fixture-only Rust helper. This script composes those assets into notebook pages
and applies deterministic camera-like transforms with Pillow.

Synthetic fixtures are useful algorithmic controls. They are not substitutes
for later photographed/device fixtures and must never be reported as such.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

from PIL import Image, ImageDraw, ImageEnhance, ImageFilter, ImageFont

GENERATOR_NAME = "a2d-synthetic-scan-generator"
GENERATOR_VERSION = 1
PAGE_WIDTH = 1400
PAGE_HEIGHT = 1900
SCENE_WIDTH = 1800
SCENE_HEIGHT = 2200
TAG_RASTER_SIZE = 154
TAG_QUIET_ZONE = 20
QR_SIZE = 300
BACKGROUND = (47, 50, 58)
MAIN_MARKERS = {
    "top_left": 0,
    "top_right": 1,
    "bottom_right": 2,
    "bottom_left": 3,
}
WRONG_MARKERS = {
    "top_left": 4,
    "top_right": 5,
    "bottom_right": 6,
    "bottom_left": 7,
}
MARKER_POSITIONS = {
    "top_left": (58, 58),
    "top_right": (PAGE_WIDTH - 58 - TAG_RASTER_SIZE - 2 * TAG_QUIET_ZONE, 58),
    "bottom_right": (
        PAGE_WIDTH - 58 - TAG_RASTER_SIZE - 2 * TAG_QUIET_ZONE,
        PAGE_HEIGHT - 58 - TAG_RASTER_SIZE - 2 * TAG_QUIET_ZONE,
    ),
    "bottom_left": (58, PAGE_HEIGHT - 58 - TAG_RASTER_SIZE - 2 * TAG_QUIET_ZONE),
}


@dataclass(frozen=True)
class FixtureSpec:
    fixture_id: str
    relative_path: str
    category: str
    intended_quality_state: str
    warnings: tuple[str, ...]
    image_decode: str = "success"
    expected_marker_roles: dict[str, int] | None = None
    expected_qr: str | None = "main"
    notes: str = ""
    transform: dict[str, Any] | None = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--support-dir",
        type=Path,
        default=Path("target/fixture-support"),
        help="directory produced by a2d-fixture-support",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("fixtures/scans"),
        help="fixture corpus output directory",
    )
    return parser.parse_args()


def load_default_font(size: int) -> ImageFont.ImageFont:
    # Pillow's bundled default font is used to avoid host font/license drift.
    return ImageFont.load_default(size=size)


def load_support_image(support_dir: Path, name: str) -> Image.Image:
    path = support_dir / name
    if not path.is_file():
        raise FileNotFoundError(f"missing fixture support raster: {path}")
    with Image.open(path) as image:
        image.load()
        return image.convert("L")


def tag_tile(support_dir: Path, tag_id: int) -> Image.Image:
    source = load_support_image(support_dir, f"tag-{tag_id}.pgm")
    scaled = source.resize(
        (TAG_RASTER_SIZE, TAG_RASTER_SIZE),
        resample=Image.Resampling.NEAREST,
    )
    tile_size = TAG_RASTER_SIZE + 2 * TAG_QUIET_ZONE
    tile = Image.new("L", (tile_size, tile_size), 255)
    tile.paste(scaled, (TAG_QUIET_ZONE, TAG_QUIET_ZONE))
    return tile.convert("RGB")


def qr_tile(support_dir: Path, name: str) -> Image.Image:
    source = load_support_image(support_dir, name)
    return source.resize((QR_SIZE, QR_SIZE), resample=Image.Resampling.NEAREST).convert("RGB")


def draw_content(page: Image.Image, revision: int) -> None:
    draw = ImageDraw.Draw(page)
    title_font = load_default_font(42)
    section_font = load_default_font(27)
    body_font = load_default_font(21)

    draw.text((270, 75), "A2D SMART NOTEBOOK", fill=(20, 24, 32), font=title_font)
    draw.text((270, 130), "Synthetic scan fixture - page 42", fill=(55, 60, 70), font=body_font)

    content_left = 145
    content_right = PAGE_WIDTH - 145
    top = 330
    for row in range(18):
        y = top + row * 72
        draw.line((content_left, y, content_right, y), fill=(188, 194, 205), width=2)

    draw.text((content_left, 275), "Tasks and notes", fill=(20, 24, 32), font=section_font)
    tasks = [
        ("Verify local backup", revision >= 1),
        ("Review marker detection", True),
        ("Keep original scan immutable", True),
        ("Calibrate quality thresholds on devices", False),
    ]
    for index, (label, checked) in enumerate(tasks):
        y = 380 + index * 116
        draw.rectangle((170, y, 216, y + 46), outline=(42, 48, 60), width=4)
        if checked:
            draw.line((178, y + 24, 193, y + 39), fill=(27, 94, 55), width=6)
            draw.line((192, y + 39, 209, y + 8), fill=(27, 94, 55), width=6)
        draw.text((245, y + 4), label, fill=(30, 34, 42), font=body_font)

    # Deterministic pseudo-handwriting and a small diagram provide high-frequency content.
    points: list[tuple[int, int]] = []
    for x in range(180, 1150, 9):
        y = 965 + int(13 * math.sin(x / 38.0) + 6 * math.sin(x / 13.0))
        points.append((x, y))
    draw.line(points, fill=(32, 64, 112), width=5, joint="curve")
    draw.text((180, 1010), "The first scan remains preserved.", fill=(32, 64, 112), font=body_font)

    draw.rounded_rectangle((185, 1160, 555, 1510), radius=24, outline=(58, 66, 80), width=5)
    draw.ellipse((265, 1240, 475, 1450), outline=(58, 66, 80), width=5)
    draw.line((370, 1160, 370, 1510), fill=(58, 66, 80), width=4)
    draw.line((185, 1335, 555, 1335), fill=(58, 66, 80), width=4)
    draw.text((620, 1185), "Diagram", fill=(20, 24, 32), font=section_font)
    draw.text((620, 1250), "Marker geometry", fill=(55, 60, 70), font=body_font)
    draw.text((620, 1300), "QR identity", fill=(55, 60, 70), font=body_font)
    draw.text((620, 1350), "Derived images", fill=(55, 60, 70), font=body_font)

    if revision >= 1:
        draw.rounded_rectangle((610, 1430, 1190, 1545), radius=18, fill=(236, 246, 238), outline=(52, 120, 72), width=4)
        draw.text((635, 1464), "Revision 2: added validation note", fill=(34, 92, 51), font=body_font)


def build_page(
    support_dir: Path,
    marker_roles: dict[str, int],
    qr_name: str,
    *,
    missing_role: str | None = None,
    duplicate_role: str | None = None,
    revision: int = 0,
) -> Image.Image:
    page = Image.new("RGB", (PAGE_WIDTH, PAGE_HEIGHT), (250, 249, 246))
    draw = ImageDraw.Draw(page)
    draw.rectangle((18, 18, PAGE_WIDTH - 19, PAGE_HEIGHT - 19), outline=(77, 81, 90), width=5)
    draw_content(page, revision)

    for role, tag_id in marker_roles.items():
        if role == missing_role:
            continue
        page.paste(tag_tile(support_dir, tag_id), MARKER_POSITIONS[role])

    qr = qr_tile(support_dir, qr_name)
    page.paste(qr, ((PAGE_WIDTH - QR_SIZE) // 2, 50))

    if duplicate_role is not None:
        duplicate = tag_tile(support_dir, marker_roles[duplicate_role])
        page.paste(duplicate, (PAGE_WIDTH // 2 - duplicate.width // 2, 1580))

    return page


def solve_linear(matrix: list[list[float]], values: list[float]) -> list[float]:
    size = len(values)
    augmented = [row[:] + [value] for row, value in zip(matrix, values, strict=True)]
    for pivot_column in range(size):
        pivot_row = max(range(pivot_column, size), key=lambda row: abs(augmented[row][pivot_column]))
        if abs(augmented[pivot_row][pivot_column]) < 1.0e-12:
            raise ValueError("perspective transform is singular")
        augmented[pivot_column], augmented[pivot_row] = augmented[pivot_row], augmented[pivot_column]
        pivot = augmented[pivot_column][pivot_column]
        augmented[pivot_column] = [value / pivot for value in augmented[pivot_column]]
        for row in range(size):
            if row == pivot_column:
                continue
            factor = augmented[row][pivot_column]
            if factor == 0.0:
                continue
            augmented[row] = [
                current - factor * pivot_value
                for current, pivot_value in zip(augmented[row], augmented[pivot_column], strict=True)
            ]
    return [augmented[row][-1] for row in range(size)]


def perspective_coefficients(
    destination: Sequence[tuple[float, float]],
    source: Sequence[tuple[float, float]],
) -> tuple[float, ...]:
    matrix: list[list[float]] = []
    values: list[float] = []
    for (x, y), (u, v) in zip(destination, source, strict=True):
        matrix.append([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y])
        values.append(u)
        matrix.append([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y])
        values.append(v)
    return tuple(solve_linear(matrix, values))


def perspective_scene(page: Image.Image, corners: Sequence[tuple[int, int]]) -> Image.Image:
    source = [
        (0.0, 0.0),
        (float(PAGE_WIDTH - 1), 0.0),
        (float(PAGE_WIDTH - 1), float(PAGE_HEIGHT - 1)),
        (0.0, float(PAGE_HEIGHT - 1)),
    ]
    destination = [(float(x), float(y)) for x, y in corners]
    coefficients = perspective_coefficients(destination, source)
    warped = page.convert("RGBA").transform(
        (SCENE_WIDTH, SCENE_HEIGHT),
        Image.Transform.PERSPECTIVE,
        coefficients,
        resample=Image.Resampling.BICUBIC,
        fillcolor=(0, 0, 0, 0),
    )
    background = Image.new("RGBA", (SCENE_WIDTH, SCENE_HEIGHT), BACKGROUND + (255,))
    return Image.alpha_composite(background, warped).convert("RGB")


def rotated_scene(page: Image.Image, degrees: float) -> Image.Image:
    rotated = page.convert("RGBA").rotate(
        degrees,
        expand=True,
        resample=Image.Resampling.BICUBIC,
        fillcolor=(0, 0, 0, 0),
    )
    maximum_width = SCENE_WIDTH - 100
    maximum_height = SCENE_HEIGHT - 100
    if rotated.width > maximum_width or rotated.height > maximum_height:
        scale = min(maximum_width / rotated.width, maximum_height / rotated.height)
        rotated = rotated.resize(
            (max(1, round(rotated.width * scale)), max(1, round(rotated.height * scale))),
            resample=Image.Resampling.LANCZOS,
        )
    scene = Image.new("RGBA", (SCENE_WIDTH, SCENE_HEIGHT), BACKGROUND + (255,))
    left = (SCENE_WIDTH - rotated.width) // 2
    top = (SCENE_HEIGHT - rotated.height) // 2
    scene.alpha_composite(rotated, (left, top))
    return scene.convert("RGB")


def glare(image: Image.Image, *, box: tuple[int, int, int, int], alpha: int, blur: float) -> Image.Image:
    overlay = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    draw.ellipse(box, fill=(255, 255, 255, alpha))
    overlay = overlay.filter(ImageFilter.GaussianBlur(radius=blur))
    return Image.alpha_composite(image.convert("RGBA"), overlay).convert("RGB")


def write_png(image: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, format="PNG", optimize=False, compress_level=9)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def fixture_entry(output_dir: Path, spec: FixtureSpec) -> dict[str, Any]:
    path = output_dir / spec.relative_path
    entry: dict[str, Any] = {
        "id": spec.fixture_id,
        "path": spec.relative_path,
        "category": spec.category,
        "source": "project-generated",
        "license": "Apache-2.0",
        "sha256": sha256(path),
        "byte_length": path.stat().st_size,
        "intended_quality_state": spec.intended_quality_state,
        "warnings": list(spec.warnings),
        "image_decode": spec.image_decode,
        "expected_marker_roles": spec.expected_marker_roles,
        "expected_qr": spec.expected_qr,
        "notes": spec.notes,
        "transform": spec.transform or {},
    }
    if spec.image_decode == "success":
        with Image.open(path) as image:
            entry["width"] = image.width
            entry["height"] = image.height
            entry["mode"] = image.mode
    return entry


def clear_generated_output(output_dir: Path) -> None:
    for name in (
        "generated",
        "glare",
        "blur",
        "missing-marker",
        "wrong-layout",
        "duplicate",
        "revisions",
        "corrupted",
    ):
        path = output_dir / name
        if path.exists():
            shutil.rmtree(path)
        path.mkdir(parents=True, exist_ok=True)
    (output_dir / "photographed").mkdir(parents=True, exist_ok=True)
    manifest = output_dir / "manifest.json"
    if manifest.exists():
        manifest.unlink()


def main() -> None:
    args = parse_args()
    support_dir = args.support_dir.resolve()
    output_dir = args.output_dir.resolve()
    payloads = json.loads((support_dir / "payloads.json").read_text(encoding="utf-8"))
    clear_generated_output(output_dir)

    base_page = build_page(support_dir, MAIN_MARKERS, "qr-main.pgm")
    mild_corners = [(205, 145), (1580, 230), (1650, 2050), (145, 1980)]
    severe_corners = [(430, 110), (1480, 390), (1720, 2030), (80, 1800)]
    mild_scene = perspective_scene(base_page, mild_corners)

    outputs: list[FixtureSpec] = []

    def add(image: Image.Image, spec: FixtureSpec) -> None:
        write_png(image, output_dir / spec.relative_path)
        outputs.append(spec)

    add(
        base_page,
        FixtureSpec(
            "generated-base-page",
            "generated/base-page.png",
            "generated",
            "Accepted",
            (),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "none"},
        ),
    )
    add(
        rotated_scene(base_page, 7.5),
        FixtureSpec(
            "generated-rotated-7-5",
            "generated/rotated-7-5-degrees.png",
            "generated",
            "AcceptedWithWarnings",
            ("moderate_rotation",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "rotation", "degrees": 7.5},
        ),
    )
    add(
        rotated_scene(base_page, 90.0),
        FixtureSpec(
            "generated-rotated-90",
            "generated/rotated-90-degrees.png",
            "generated",
            "AcceptedWithWarnings",
            ("orientation_normalization_required",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "rotation", "degrees": 90.0},
        ),
    )
    add(
        mild_scene,
        FixtureSpec(
            "generated-perspective-mild",
            "generated/perspective-mild.png",
            "generated",
            "AcceptedWithWarnings",
            ("mild_perspective",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "perspective", "corners": mild_corners},
        ),
    )
    add(
        perspective_scene(base_page, severe_corners),
        FixtureSpec(
            "generated-perspective-severe",
            "generated/perspective-severe.png",
            "generated",
            "NeedsReview",
            ("severe_perspective", "low_page_fill"),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "perspective", "corners": severe_corners},
        ),
    )
    add(
        ImageEnhance.Brightness(mild_scene).enhance(0.32),
        FixtureSpec(
            "generated-underexposed",
            "generated/underexposed.png",
            "generated",
            "NeedsReview",
            ("underexposed",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "brightness", "factor": 0.32},
        ),
    )
    add(
        ImageEnhance.Brightness(mild_scene).enhance(1.72),
        FixtureSpec(
            "generated-overexposed",
            "generated/overexposed.png",
            "generated",
            "NeedsReview",
            ("overexposed", "highlight_clipping"),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "brightness", "factor": 1.72},
        ),
    )
    add(
        mild_scene.filter(ImageFilter.GaussianBlur(radius=2.0)),
        FixtureSpec(
            "blur-moderate",
            "blur/gaussian-radius-2.png",
            "blur",
            "AcceptedWithWarnings",
            ("moderate_blur",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "gaussian_blur", "radius": 2.0},
        ),
    )
    add(
        mild_scene.filter(ImageFilter.GaussianBlur(radius=7.0)),
        FixtureSpec(
            "blur-severe",
            "blur/gaussian-radius-7.png",
            "blur",
            "Rejected",
            ("severe_blur",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "gaussian_blur", "radius": 7.0},
        ),
    )
    add(
        glare(mild_scene, box=(860, 420, 1510, 1150), alpha=135, blur=48.0),
        FixtureSpec(
            "glare-partial",
            "glare/partial-glare.png",
            "glare",
            "AcceptedWithWarnings",
            ("partial_glare",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "glare", "box": [860, 420, 1510, 1150], "alpha": 135, "blur": 48.0},
        ),
    )
    add(
        glare(mild_scene, box=(250, 230, 1580, 1830), alpha=220, blur=95.0),
        FixtureSpec(
            "glare-strong",
            "glare/strong-glare.png",
            "glare",
            "NeedsReview",
            ("strong_glare", "highlight_clipping"),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "glare", "box": [250, 230, 1580, 1830], "alpha": 220, "blur": 95.0},
        ),
    )

    missing_page = build_page(
        support_dir,
        MAIN_MARKERS,
        "qr-main.pgm",
        missing_role="bottom_right",
    )
    add(
        perspective_scene(missing_page, mild_corners),
        FixtureSpec(
            "missing-marker-bottom-right",
            "missing-marker/missing-bottom-right.png",
            "missing-marker",
            "Rejected",
            ("insufficient_markers",),
            expected_marker_roles={key: value for key, value in MAIN_MARKERS.items() if key != "bottom_right"},
            transform={"kind": "remove_marker", "role": "bottom_right"},
        ),
    )

    wrong_layout_page = build_page(support_dir, MAIN_MARKERS, "qr-wrong-layout.pgm")
    add(
        perspective_scene(wrong_layout_page, mild_corners),
        FixtureSpec(
            "wrong-layout-qr",
            "wrong-layout/wrong-layout-qr.png",
            "wrong-layout",
            "Rejected",
            ("wrong_layout_identity",),
            expected_marker_roles=MAIN_MARKERS,
            expected_qr="wrong_layout",
            transform={"kind": "replace_qr_layout"},
        ),
    )

    wrong_tags_page = build_page(support_dir, WRONG_MARKERS, "qr-main.pgm")
    add(
        perspective_scene(wrong_tags_page, mild_corners),
        FixtureSpec(
            "wrong-layout-tag-set",
            "wrong-layout/wrong-tag-set.png",
            "wrong-layout",
            "Rejected",
            ("unexpected_marker_ids",),
            expected_marker_roles=WRONG_MARKERS,
            transform={"kind": "replace_marker_set"},
        ),
    )

    duplicate_page = build_page(
        support_dir,
        MAIN_MARKERS,
        "qr-main.pgm",
        duplicate_role="top_left",
    )
    add(
        perspective_scene(duplicate_page, mild_corners),
        FixtureSpec(
            "duplicate-top-left-marker",
            "duplicate/duplicate-top-left.png",
            "duplicate",
            "NeedsReview",
            ("duplicate_marker_id",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "duplicate_marker", "role": "top_left"},
        ),
    )

    revision_original = build_page(support_dir, MAIN_MARKERS, "qr-main.pgm", revision=0)
    revision_updated = build_page(support_dir, MAIN_MARKERS, "qr-main.pgm", revision=1)
    add(
        perspective_scene(revision_original, mild_corners),
        FixtureSpec(
            "revision-original",
            "revisions/revision-original.png",
            "revisions",
            "AcceptedWithWarnings",
            ("mild_perspective",),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "revision", "revision": 1},
        ),
    )
    add(
        perspective_scene(revision_updated, mild_corners),
        FixtureSpec(
            "revision-updated",
            "revisions/revision-updated.png",
            "revisions",
            "AcceptedWithWarnings",
            ("mild_perspective", "content_changed"),
            expected_marker_roles=MAIN_MARKERS,
            transform={"kind": "revision", "revision": 2},
        ),
    )

    base_bytes = (output_dir / "generated/base-page.png").read_bytes()
    truncated_path = output_dir / "corrupted/truncated.png"
    truncated_path.write_bytes(base_bytes[: max(64, len(base_bytes) // 3)])
    outputs.append(
        FixtureSpec(
            "corrupted-truncated-png",
            "corrupted/truncated.png",
            "corrupted",
            "Rejected",
            ("corrupt_capture",),
            image_decode="failure",
            expected_marker_roles=None,
            expected_qr=None,
            notes="Valid PNG prefix truncated to one third of the original byte length.",
            transform={"kind": "truncate", "retained_fraction": 1.0 / 3.0},
        )
    )
    invalid_path = output_dir / "corrupted/not-an-image.bin"
    invalid_path.write_bytes(b"A2D synthetic fixture: deliberately not an image\n")
    outputs.append(
        FixtureSpec(
            "corrupted-not-an-image",
            "corrupted/not-an-image.bin",
            "corrupted",
            "Rejected",
            ("unsupported_or_corrupt_capture",),
            image_decode="failure",
            expected_marker_roles=None,
            expected_qr=None,
            transform={"kind": "invalid_bytes"},
        )
    )

    manifest = {
        "schema_version": 1,
        "generator": {
            "name": GENERATOR_NAME,
            "version": GENERATOR_VERSION,
            "script": "tools/generate_scan_fixtures.py",
            "support_binary": "a2d-fixture-support",
            "deterministic": True,
        },
        "provenance": {
            "source": "project-generated",
            "license": "Apache-2.0",
            "photographed": False,
            "statement": (
                "All assets in this manifest are synthetic controls generated by project code. "
                "They do not satisfy photographed/device-fixture requirements."
            ),
        },
        "identity": payloads,
        "marker_family": "tagStandard41h12",
        "marker_roles": MAIN_MARKERS,
        "fixtures": [fixture_entry(output_dir, spec) for spec in outputs],
    }
    (output_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
