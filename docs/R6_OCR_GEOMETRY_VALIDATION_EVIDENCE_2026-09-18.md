# R6 OCR Geometry Validation Evidence — 2026-09-18

This note records the qualification scope for the R6 source-geometry validation slice.

## Implemented in PR #95

- OCR text-region batch persistence now carries explicit source image dimensions through Rust core and UniFFI.
- Rust validates source dimensions are positive before accepting durable text-region rows.
- Rust validates every persisted OCR polygon point uses the documented inclusive pixel-bound convention:
  - `0 <= x <= source_image_width`
  - `0 <= y <= source_image_height`
- Rust rejects non-finite, negative, and out-of-bounds persisted polygon coordinates before storage.
- Android OCR workflow passes prepared OCR input dimensions into the Rust-owned text-region persistence request.

## Test coverage added

- Core rejects invalid source dimensions.
- Core rejects polygon coordinates outside source dimensions.
- Android workflow tests verify prepared OCR dimensions are propagated into the region persistence batch.

## CI note

The first PR #95 full-CI attempt reached green Rust, cargo-deny, Kotlin UniFFI drift, Android build/lint/unit/APK, and green Milestone 7 Native Validation. The emulator job failed before instrumentation because the Android SDK emulator package download reported `Error on ZipFile unknown archive`. This was infrastructure setup failure before tests ran, so this evidence-only commit retriggers exact-head CI without changing implementation behavior.
