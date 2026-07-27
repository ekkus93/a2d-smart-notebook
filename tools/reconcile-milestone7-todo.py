#!/usr/bin/env python3
"""Reconcile Milestone 7.1-7.4 checkboxes with committed implementation evidence."""

from pathlib import Path

TODO = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")

OLD = """# Milestone 7 — Marker detection and image-processing foundation

## 7.1 Complete a working detector spike

- [ ] Evaluate the official AprilTag 3 native library.
- [ ] Confirm license compatibility and commit the review.
- [ ] Build reproducibly for required Android ABIs.
- [ ] Wrap ownership and errors safely for Rust.
- [ ] Measure detection on representative grayscale fixtures.
- [ ] Confirm future iOS build feasibility.
- [ ] Compare a pure-Rust alternative only if it materially reduces packaging risk.
- [ ] Accept `docs/decisions/0002-apriltag-detector-selection.md`, naming the selected
      implementation and recording license review, Android ABI build results, desktop fixture
      results, performance measurements, the memory-safety boundary, packaging strategy, and
      future iOS feasibility.

The spike must end with code and tests, not prose only.

## 7.2 Image input types

- [ ] Define width, height, row stride, pixel format, rotation, and buffer ownership.
- [ ] Support reduced grayscale analysis frames.
- [ ] Support full-resolution image files for final processing.
- [ ] Reject impossible dimensions/strides.
- [ ] Enforce maximum decoded pixel count.
- [ ] Avoid Base64.

Example:

```rust
pub struct GrayFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub row_stride: usize,
    pub rotation_degrees: u16,
    pub bytes: &'a [u8],
}
```

## 7.3 Detection results

Return marker ID, four corners, center, decision margin, and error-quality data.

- [ ] Validate expected family.
- [ ] Resolve marker semantic roles from layout.
- [ ] Reject duplicate roles.
- [ ] Resolve orientation.
- [ ] Preserve detection quality.

## 7.4 QR decoder boundary

- [ ] Decide whether image decoding runs in platform or shared native code.
- [ ] Always send decoded payload to Rust for canonical parsing.
- [ ] Add blur/rotation/scale/damage fixtures.
- [ ] Never accept a decoder result without Rust validation.
"""

NEW = """# Milestone 7 — Marker detection and image-processing foundation

## 7.1 Complete a working detector spike

- [x] Evaluate the official AprilTag 3 native library. The pinned official implementation through
      `apriltag-sys = 0.4.0` is integrated behind the safe `a2d-image` API.
- [x] Confirm license compatibility and commit the review. See
      `docs/reviews/APRILTAG_LICENSE_REVIEW_2026-07-27.md`.
- [x] Build reproducibly for required Android ABIs. CI pins Android NDK `27.0.12077973` and
      `cargo-ndk 4.1.2` and builds `arm64-v8a` plus `x86_64` directly against `a2d-image`.
- [x] Wrap ownership and errors safely for Rust. Native detector, family, image, and detection-array
      lifetimes are private RAII guards; public results own their data and failures are typed.
- [ ] Measure detection on representative grayscale fixtures. The generated four-tag smoke test is
      deterministic, but photographed/perturbed fixtures and Android device-tier measurements are
      still required.
- [x] Confirm future iOS build feasibility. The same crate compiles for `aarch64-apple-ios` and
      `aarch64-apple-ios-sim`; this is compile feasibility only, not iOS application work.
- [x] Compare a pure-Rust alternative only if it materially reduces packaging risk. The contingency
      was reviewed and not triggered because the pinned official implementation cross-compiles for
      every required native target; a comparison remains conditional on later material risk.
- [ ] Accept `docs/decisions/0002-apriltag-detector-selection.md`, naming the selected
      implementation and recording license review, Android ABI build results, desktop fixture
      results, performance measurements, the memory-safety boundary, packaging strategy, and
      future iOS feasibility. The ADR remains Proposed until representative fixtures, Android
      device measurements, final APK packaging, and third-party notices are proven.

The spike ends with code, tests, pinned CI, and committed evidence rather than prose only.

## 7.2 Image input types

- [x] Define width, height, row stride, pixel format, rotation, and buffer ownership.
- [x] Support reduced grayscale analysis frames through borrowed validated `GrayFrame` input.
- [x] Support bounded JPEG and PNG full-resolution image files for final processing.
- [x] Reject impossible dimensions, strides, truncated rows, format mismatches, and output-size
      overflows with structured errors.
- [x] Enforce caller-selected encoded-byte, decoded-byte, and decoded-pixel limits before accepting
      full-resolution output.
- [x] Avoid Base64. Image boundaries use borrowed or owned byte buffers directly.

Implemented inputs expose borrowed Gray8 analysis frames plus owned RGB8/Gray8 decoded images.
Encoded files require an explicit declared format and limits; there is no format-guessing fallback.

## 7.3 Detection results

Return marker ID, four corners, center, decision margin, and error-quality data.

- [x] Validate expected `tagStandard41h12` family.
- [x] Resolve marker semantic roles from the selected layout.
- [x] Reject duplicate tag IDs and duplicate semantic roles.
- [x] Resolve page orientation from the semantic top edge.
- [x] Preserve center, four corners, decision margin, and Hamming-error quality data.

## 7.4 QR decoder boundary

- [x] Decide that Android performs bounded local QR image decoding while Rust remains the canonical
      payload trust boundary. See `docs/decisions/0003-qr-image-decoder-boundary.md`.
- [x] Always send decoded payload text to Rust for canonical grammar, version, bounds, layout, and
      CRC validation.
- [ ] Add legally usable blur, rotation, scale, glare, and damage fixtures for the shipped decoder.
- [x] Never accept a decoder result without Rust validation; decoder success alone cannot trigger an
      A2D identity or workflow success state.

Validation evidence for the reconciled work:

- GitHub Actions native run `30309797591` completed successfully on 2026-07-27.
- GitHub Actions full CI run `30309797600` completed successfully on 2026-07-27.
- Rust formatting, clippy with warnings denied, and the full workspace test suite passed.
- Dependency/license checks passed.
- Android lint, unit tests, and debug APK assembly passed.
- Kotlin UniFFI binding-drift validation passed.
- Pinned Android `arm64-v8a`/`x86_64` native builds and future Apple device/simulator compile checks
  passed.
"""


def main() -> None:
    text = TODO.read_text(encoding="utf-8")
    occurrences = text.count(OLD)
    if occurrences != 1:
        raise SystemExit(f"expected exactly one Milestone 7.1-7.4 block, found {occurrences}")
    TODO.write_text(text.replace(OLD, NEW, 1), encoding="utf-8")


if __name__ == "__main__":
    main()
