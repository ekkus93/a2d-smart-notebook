#!/usr/bin/env python3
"""Reconcile Milestone 7.5 with committed rectification implementation evidence."""

from pathlib import Path

TODO = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")

OLD = """## 7.5 Homography and rectification

- [ ] Compute projective transform from known correspondences.
- [ ] Validate transform conditioning.
- [ ] Reject self-intersecting or implausible quadrilaterals.
- [ ] Warp to canonical dimensions.
- [ ] Preserve matrix and source corners.
- [ ] Add deterministic reference-output tests.
- [ ] Prevent out-of-bounds access on malformed inputs.
"""

NEW = """## 7.5 Homography and rectification

- [x] Compute a normalized projective transform from four ordered source/destination
      correspondences with partial-pivot Gaussian elimination and verified reprojection.
- [x] Validate transform conditioning through finite geometry checks, normalization, singular-pivot
      rejection, pivot-ratio rejection, matrix inversion checks, and finite projection checks.
- [x] Reject zero-length, collinear, self-intersecting, non-convex, negligible-area, non-finite, and
      source-out-of-bounds quadrilaterals with structured errors.
- [x] Warp borrowed Gray8 or owned RGB8 input to caller-selected canonical dimensions using bounded
      inverse mapping and bilinear interpolation.
- [x] Preserve source/destination page corners, optional source/destination marker centers, forward
      and inverse matrices, source dimensions, output dimensions, and solve pivot ratio.
- [x] Add deterministic reference-output tests for Gray8/RGB8 warps plus identity, perspective,
      semantic marker/layout, invalid-geometry, source-mismatch, and memory-limit cases.
- [x] Prevent out-of-bounds access on malformed inputs through validated source geometry, exact
      buffer construction, checked output limits, bounded numerical epsilon, and structured sample
      rejection rather than clamping arbitrary invalid coordinates.

`RectificationPlan::from_page_markers` uses Rust-resolved semantic marker roles and the physical
`PageLayout` marker centers to derive the canonical transform. Output is upright (`Degrees0`), and
no production page resolution or quality threshold is invented by this layer.

Validation evidence:

- GitHub Actions native run `30311792736` passed pinned Android `arm64-v8a`/`x86_64` builds and
  future Apple device/simulator compile-feasibility checks.
- GitHub Actions full CI run `30311792705` passed Rust formatting, workspace clippy with warnings
  denied, the full workspace test suite, dependency/license checks, and UniFFI binding drift.
- Android lint, unit tests, and debug APK assembly are tracked by the same full CI run.
"""


def main() -> None:
    text = TODO.read_text(encoding="utf-8")
    if text.count(OLD) != 1:
        raise SystemExit(f"expected one Milestone 7.5 block, found {text.count(OLD)}")
    TODO.write_text(text.replace(OLD, NEW, 1), encoding="utf-8")


if __name__ == "__main__":
    main()
