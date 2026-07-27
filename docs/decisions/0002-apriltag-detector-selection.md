# 0002. Corner-marker (AprilTag) detector selection

**Status:** Proposed — official detector selected; native portability confirmed; fixture and packaging evidence pending  
**Date:** 2026-07-27  
**Decision owners/authors:** A2D project

## Context and problem

Spec §17.3 requires evaluating the official AprilTag 3 native detector library as the primary option for reading the four Corner Markers on every writable page, with a pure-Rust alternative considered only if it materially reduces packaging risk. The selected implementation must build reproducibly for the required Android ABIs, have a documented license review, produce deterministic fixture results within tolerance, expose marker ID/corners/quality data, and be structured so the same native implementation can target iOS later.

The first Milestone 7 implementation uses `apriltag-sys = 0.4.0` to compile the bundled official AprilTag C source and exposes a narrow safe Rust API from `a2d-image`. Pinned CI validation now confirms the desktop detector path, both required Android ABIs, and future iOS device/simulator compile feasibility. This ADR remains Proposed because representative photographed fixtures, device-tier performance measurements, final APK packaging, and release notices are still incomplete.

## Constraints

- The Rust core remains authoritative for marker semantics and image-processing policy.
- Android must pass luminance bytes with dimensions and stride; JSON/Base64 is prohibited.
- The native implementation must not be selected from a workstation installation implicitly.
- Native pointers, allocation ownership, and detector lifetimes must not cross the public Rust API.
- Failures must return structured `A2dError` values rather than empty detections or fabricated success.
- Detection-quality and performance thresholds must be measured, versioned, and recorded rather than guessed.
- v0.1 Android ABI support for this validation is `arm64-v8a` for physical devices and `x86_64` for the emulator/test environment.

## Options considered

### Option 1 — Official AprilTag 3 C implementation through `apriltag-sys`

Use the pinned `apriltag-sys` crate, force its bundled source build with `APRILTAG_SYS_METHOD = "raw,static"`, and contain all unsafe operations in a reviewed `a2d-image` wrapper.

Advantages:

- Uses the official detector implementation and recommended `tagStandard41h12` family.
- Exposes detector-native decision margin, Hamming-error count, center, and four corners.
- Bundled-source compilation avoids an OpenCV dependency and avoids host-library drift.
- The C source and Rust wrapper are both BSD-2-Clause compatible with the Apache-2.0 project.
- The same C implementation compiles through the tested Android NDK and Apple clang toolchains.

Risks:

- Adds a C FFI memory-safety boundary.
- Final Android application packaging still must be proven.
- The crate is not itself an Android/iOS product integration layer; A2D owns that validation and packaging.

### Option 2 — Pure-Rust AprilTag-compatible detector

Adopt or build a detector without a C FFI boundary.

Potential advantages are a smaller unsafe surface and possibly simpler Rust-only packaging. This option has not demonstrated a material packaging, correctness, licensing, or maintenance advantage. The pinned official implementation now cross-compiles successfully for the required Android ABIs, so the contingency that would trigger a pure-Rust comparison has not occurred. Replacing the official detector now would add algorithmic and compatibility uncertainty without evidence.

## Decision

Provisionally select **Option 1: the official AprilTag 3 implementation through pinned `apriltag-sys = 0.4.0`**.

The decision becomes Accepted only after:

1. the desktop detector test passes in CI — **complete**;
2. bundled-source builds pass for `arm64-v8a` and `x86_64` — **complete**;
3. the same crate compiles for an iOS device target and Apple Silicon simulator target — **complete**;
4. performance evidence is recorded from representative fixtures and target Android device tiers — **pending**; and
5. final APK packaging and third-party notice obligations are demonstrated — **pending**.

A pure-Rust alternative will be evaluated only if later fixture, performance, or application-packaging validation exposes a material risk.

## Detailed rationale

The current wrapper demonstrates the important architectural boundary:

- `GrayFrame` validates dimensions, row stride, buffer length, rotation, and explicit pixel limits before native allocation.
- `AprilTagDetector` owns the detector and tag-family pointers and destroys them in a defined order.
- Native images and detection arrays use private RAII guards.
- Detection output is copied into owned Rust values before native memory is released.
- Null pointers, invalid native array layouts, invalid IDs, non-finite geometry, and invalid quality values become structured errors.
- Semantic marker roles and orientation are resolved in Rust through the selected page layout.
- Unexpected marker IDs are preserved for diagnostics rather than silently discarded.
- Target-dependent generated C boolean fields are normalized through a private Rust conversion trait rather than exposing binding-specific types.

This keeps the unavoidable unsafe code narrow and reviewable while preserving the official detector's behavior.

## Security/privacy implications

Marker detection is local and requires no network access. The detector receives only an in-memory grayscale frame. The wrapper must continue to enforce maximum pixel counts and validated buffer geometry before native allocation or copying.

The current boundary copies the validated input into an AprilTag-owned image. This costs one bounded copy but avoids lending platform-owned camera memory to C beyond the Rust call. Zero-copy optimization must not be introduced unless its ownership and concurrent-frame lifetime can be proven safe.

The memory-safety boundary is limited to `crates/a2d-image/src/detector.rs`. Native pointers never appear in exported Rust types or future UniFFI records.

## Portability implications for Android and future iOS

`.cargo/config.toml` forces the crate's bundled C sources to compile statically, preventing accidental selection of a different system AprilTag installation.

`tools/validate-milestone7-native.sh` validates the Android NDK builds directly against `a2d-image`, rather than relying on `a2d-ffi` to pull the crate transitively before image APIs exist at the FFI layer. The script verifies the resolved NDK revision and `cargo-ndk` version before compiling.

`.github/workflows/milestone7-native.yml` pins and verifies:

- Ubuntu 24.04 for the Android/native job;
- Android NDK `27.0.12077973`;
- `cargo-ndk 4.1.2`;
- Android `arm64-v8a` and `x86_64` builds;
- compile checks for `aarch64-apple-ios` and `aarch64-apple-ios-sim`.

GitHub Actions run [`30308722258`](https://github.com/ekkus93/a2d-smart-notebook/actions/runs/30308722258) completed successfully on 2026-07-27. Its uploaded Android log records the exact selected toolchain and successful builds for both ABIs. Passing Apple compile checks establish future native build feasibility only; they do not represent iOS application or UI work.

## Compatibility/fixture implications

The detector test renders four `tagStandard41h12` tags through the same official library, places them into a generated grayscale frame, detects them, resolves semantic roles, and verifies orientation and quality fields.

In run `30308722258`, that generated four-tag frame was detected in `9.505117 ms` on the GitHub-hosted desktop runner. This is a deterministic smoke measurement, not a representative Android-device benchmark and not an auto-capture threshold.

The generated test is necessary but not sufficient. Milestone 7 still requires legally usable photographed, blur, glare, missing-marker, wrong-layout, duplicate, revision, and corrupted fixtures with explicit source/license metadata and tolerances.

Changing detector implementation or family after those fixtures are committed will require explicit compatibility review and may require re-deriving tolerances.

## License review

The committed engineering review is:

`docs/reviews/APRILTAG_LICENSE_REVIEW_2026-07-27.md`

Both `apriltag-sys 0.4.0` and the bundled official AprilTag source use BSD-2-Clause terms compatible with the Apache-2.0 A2D project. Binary distribution must reproduce the required notices and disclaimer. Release packaging must include third-party notices before an APK is distributed.

## Consequences and tradeoffs

Positive consequences:

- The project gets a mature official detector and recommended family.
- Marker identity, geometry, and quality data are available in the shared Rust core.
- Android and a future iOS client can share the same processing implementation.
- Host-library drift is prevented by bundled static compilation.
- Required Android native targets are now continuously checked with a pinned toolchain.

Costs and risks:

- The project owns a small unsafe FFI layer and must keep it heavily tested.
- Native mobile builds add CI time and toolchain complexity.
- The current safe boundary performs one frame copy.
- Physical fixture thresholds remain unmeasured and cannot yet drive auto-capture policy.
- Successful library cross-compilation does not prove final APK packaging or runtime loading.

## Validation evidence

- [x] Official detector integrated as code, not prose only.
- [x] Exact dependency version pinned and bundled static build forced.
- [x] Safe ownership/error wrapper implemented with unit tests.
- [x] Marker ID, corners, center, decision margin, and Hamming-error data exposed.
- [x] Rust semantic-role, duplicate-ID, missing-role, and orientation handling implemented.
- [x] License review committed.
- [x] Generated desktop grayscale detector test implemented.
- [x] Desktop detector test confirmed by Milestone 7 CI run `30308722258`.
- [x] Reproducible `arm64-v8a` Android build confirmed with NDK `27.0.12077973` and `cargo-ndk 4.1.2`.
- [x] Reproducible `x86_64` Android build confirmed with NDK `27.0.12077973` and `cargo-ndk 4.1.2`.
- [x] iOS device-target compile feasibility confirmed by CI.
- [x] iOS simulator-target compile feasibility confirmed by CI.
- [ ] Representative photographed fixture results committed.
- [ ] Performance measurements recorded for representative fixture/device tiers.
- [ ] Final Android packaging into the application APK demonstrated.
- [ ] Third-party notices included in release packaging.

## Follow-up tasks

1. Create the Milestone 7 fixture corpus and metadata format.
2. Add photographed/perturbed detector fixtures and tolerance assertions.
3. Measure representative analysis latency on supported Android device tiers.
4. Demonstrate `a2d-image`/AprilTag packaging and runtime loading in the Android APK when the shared analysis path reaches `a2d-ffi`.
5. Add required BSD-2-Clause notices to release packaging before distribution.
6. Expose the validated analysis path through `a2d-core`/UniFFI when Milestone 8 CameraX integration begins.
7. Move this ADR to Accepted only after the remaining required evidence is committed.
8. Update `docs/decisions/README.md` when the status changes.

## Superseding ADR reference

None.
