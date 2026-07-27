#!/usr/bin/env python3
"""Reconcile Milestone 7.6 with committed quality measurement evidence."""

from pathlib import Path

TODO = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")

OLD = """## 7.6 Quality metrics

Implement raw metrics and classifications for:

- [ ] Blur/focus.
- [ ] Underexposure.
- [ ] Overexposure.
- [ ] Glare/highlight clipping.
- [ ] Page fill/framing.
- [ ] Marker confidence.
- [ ] Perspective severity.
- [ ] Effective content resolution.
- [ ] Possible curvature.

Put thresholds in versioned configuration. Never invent success if measurement fails.
"""

NEW = """## 7.6 Quality metrics

Implement raw metrics and classifications for:

- [x] Blur/focus through variance of the four-neighbor luminance Laplacian over interior pixels;
      frames too small to support that measurement report focus as unavailable rather than zero.
- [x] Underexposure through mean luminance and dark-pixel fraction.
- [x] Overexposure through mean luminance and highlight-clipped pixel fraction.
- [x] Glare/highlight clipping through both global highlight fraction and the worst populated tile in
      a caller-selected bounded grid, preserving localized glare that a global average can hide.
- [x] Page fill/framing through quadrilateral area fraction, four normalized border margins, minimum
      margin, and page-center offset.
- [x] Marker confidence through minimum/mean decision margin, maximum Hamming errors, and unexpected
      tag count; invalid native quality values remain structured capture-quality failures.
- [x] Perspective severity through edge-length ratio, opposing-edge imbalance, diagonal imbalance,
      and quadrilateral-to-bounding-box area ratio.
- [x] Effective content resolution through conservative source and canonical-output pixels per
      physical millimeter using the validated `PageLayout` size.
- [x] Possible curvature through explicit edge probes and normalized perpendicular deviation. With
      no probes, curvature remains unavailable; four corners alone never fabricate a flat-page result.

Thresholds live in an explicit nonzero-version `QualityPolicy`; the library supplies no default
production policy. Scalar and nested-band threshold ordering/direction are validated before use.
Callers declare which measurements are required. Missing required metrics classify as `NeedsReview`,
missing optional metrics remain visibly `Unavailable`, and a completely unevaluated capture resolves
to `NeedsReview` rather than `Accepted`.

Classification preserves the raw measurements, per-metric state, policy version, and one of the
specification states: `Accepted`, `AcceptedWithWarnings`, `NeedsReview`, or `Rejected`.

Validation evidence:

- GitHub Actions quality run `30312884513` passed canonical formatting, clippy with warnings denied,
  and all `a2d-image` tests before the formatted source was committed.
- GitHub Actions native run `30312966395` passed pinned Android `arm64-v8a`/`x86_64` builds and future
  Apple device/simulator compile-feasibility checks with the quality module included.
- GitHub Actions full CI run `30312966424` passed workspace formatting, clippy/tests,
  dependency/license checks, and UniFFI binding-drift validation for the clean quality implementation.
"""


def main() -> None:
    text = TODO.read_text(encoding="utf-8")
    if text.count(OLD) != 1:
        raise SystemExit(f"expected one Milestone 7.6 block, found {text.count(OLD)}")
    TODO.write_text(text.replace(OLD, NEW, 1), encoding="utf-8")


if __name__ == "__main__":
    main()
