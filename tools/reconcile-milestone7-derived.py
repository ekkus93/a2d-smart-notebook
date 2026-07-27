#!/usr/bin/env python3
"""Reconcile Milestone 7.7 with committed derived-image implementation evidence."""

from pathlib import Path

TODO = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")

OLD = """## 7.7 Derived images

- [ ] Corrected color image.
- [ ] OCR-optimized image.
- [ ] Thumbnail.
- [ ] Conservative contrast normalization.
- [ ] Bounded optional sharpening.
- [ ] Pipeline provenance/version.
- [ ] Memory-bounded processing.
- [ ] Cancellation-safe temporary outputs.
- [ ] Never overwrite original.
"""

NEW = """## 7.7 Derived images

- [x] Produce a new owned corrected RGB8 image through the validated `RectificationPlan`; output is
      upright and does not alias platform-owned or original capture memory.
- [x] Produce a new owned OCR-optimized Gray8 image through deterministic RGB luminance conversion,
      conservative contrast normalization, and optional bounded sharpening.
- [x] Produce an aspect-preserving RGB8 thumbnail with caller-selected maximum dimensions and no
      automatic upscaling.
- [x] Use explicit low/high histogram percentiles and a caller-selected maximum gain for contrast
      normalization; flat or already-wide inputs remain unchanged rather than forcing enhancement.
- [x] Apply sharpening only when explicitly configured, with validated positive amount, pixel-detail
      threshold, and a bounded pass count.
- [x] Preserve a nonzero pipeline version plus source/output dimensions, source rotation, homography
      matrix, applied contrast values, and sharpening configuration in result provenance.
- [x] Preflight per-image pixel/byte limits, total output bytes, platform addressability, and a
      conservative peak working-set estimate before allocating derived outputs.
- [x] Check shared cancellation state before every major stage and between sharpening passes. Partial
      buffers remain in memory only and are dropped on failure/cancellation; no partial result is
      returned or persisted by this layer.
- [x] Borrow the original `OwnedRgbImage` immutably and create separate owned outputs. Tests verify
      the original bytes and rotation remain unchanged.

`DerivedImagePipeline` is intentionally file-system agnostic. Atomic file publication and durable
rollback belong to the storage/worker transaction boundary; this image layer cannot overwrite the
original capture because it receives no path and performs no writes.

Validation evidence:

- GitHub Actions derived validation run `30313630025` applied the single reviewed slice-API fix,
  passed clippy with warnings denied and all `a2d-image` tests, and committed only formatted Rust
  source.
- GitHub Actions native run `30313769678` passed pinned Android `arm64-v8a`/`x86_64` builds and
  future Apple device/simulator compile-feasibility checks on the clean permanent workflow.
- GitHub Actions full CI run `30313769680` passed workspace formatting/clippy/tests,
  dependency/license checks, Android lint/tests/debug APK assembly, and UniFFI binding drift.
"""


def main() -> None:
    text = TODO.read_text(encoding="utf-8")
    if text.count(OLD) != 1:
        raise SystemExit(f"expected one Milestone 7.7 block, found {text.count(OLD)}")
    TODO.write_text(text.replace(OLD, NEW, 1), encoding="utf-8")


if __name__ == "__main__":
    main()
