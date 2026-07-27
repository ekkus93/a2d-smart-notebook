# 0002. Corner-marker (AprilTag) detector selection

**Status:** Proposed — official detector provisionally selected; cross-platform validation pending  
**Date:** 2026-07-27  
**Decision owners/authors:** A2D project

## Context and problem

Spec §17.3 requires evaluating the official AprilTag 3 native detector library as the primary option for reading the four Corner Markers on every writable page, with a pure-Rust alternative considered only if it materially reduces packaging risk. The selected implementation must build reproducibly for the required Android ABIs, have a documented license review, produce deterministic fixture results within tolerance, expose marker ID/corners/quality data, and be structured so the same native implementation can target iOS later.

The first Milestone 7 implementation now uses `apriltag-sys = 0.4.0` to compile the bundled official AprilTag C source and exposes a narrow safe Rust API from `a2d-image`. This ADR records the provisional choice while the new Android and iOS CI validation runs. It must not move to Accepted until the evidence checklist below is complete.

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
- The same C implementation can in principle compile through Android NDK and Apple clang toolchains.

Risks:

- Adds a C FFI memory-safety boundary.
- Cross-compilation and final mobile packaging must be proven in CI.
- The crate is not itself an Android/iOS product integration layer; A2D owns that validation and packaging.

### Option 2 — Pure-Rust AprilTag-compatible detector

Adopt or build a detector without a C FFI boundary.

Potential advantages are a smaller unsafe surface and possibly simpler Rust-only packaging. This option has not demonstrated a material packaging, correctness, licensing, or maintenance advantage. Replacing the official detector before measuring those risks would add algorithmic and compatibility uncertainty without evidence.

## Decision

Provisionally select **Option 1: the official AprilTag 3 implementation through pinned `apriltag-sys = 0.4.0`**.

The decision becomes Accepted only after:

1. the desktop detector test passes in CI;
2. bundled-source builds pass for `arm64-v8a` and `x86_64`;
3. the same crate compiles for an iOS device target and Apple Silicon simulator target;
4. performance evidence is recorded from representative fixtures; and
5. packaging and third-party notice obligations are documented.

A pure-Rust alternative will be evaluated only if one of those validations exposes a material risk.

## Detailed rationale

The current wrapper already demonstrates the important architectural boundary:

- `GrayFrame` validates dimensions, row stride, buffer length, rotation, and explicit pixel limits before native allocation.
- `AprilTagDetector` owns the detector and tag-family pointers and destroys them in a defined order.
- Native images and detection arrays use private RAII guards.
- Detection output is copied into owned Rust values before native memory is released.
- Null pointers, invalid native array layouts, invalid IDs, non-finite geometry, and invalid quality values become structured errors.
- Semantic marker roles and orientation are resolved in Rust through the selected page layout.
- Unexpected marker IDs are preserved for diagnostics rather than silently discarded.

This keeps the unavoidable unsafe code narrow and reviewable while preserving the official detector's behavior.

## Security/privacy implications

Marker detection is local and requires no network access. The detector receives only an in-memory grayscale frame. The wrapper must continue to enforce maximum pixel counts and validated buffer geometry before native allocation or copying.

The current boundary copies the validated input into an AprilTag-owned image. This costs one bounded copy but avoids lending platform-owned camera memory to C beyond the Rust call. Zero-copy optimization must not be introduced unless its ownership and concurrent-frame lifetime can be proven safe.

The memory-safety boundary is limited to `crates/a2d-image/src/detector.rs`. Native pointers never appear in exported Rust types or future UniFFI records.

## Portability implications for Android and future iOS

`.cargo/config.toml` forces the crate's bundled C sources to compile statically, preventing accidental selection of a different system AprilTag installation.

`tools/validate-milestone7-native.sh` validates the Android NDK builds directly against `a2d-image`, rather than relying on `a2d-ffi` to pull the crate transitively before image APIs exist at the FFI layer.

`.github/workflows/milestone7-native.yml` adds:

- desktop detector execution;
- Android NDK builds for `arm64-v8a` and `x86_64`;
- compile checks for `aarch64-apple-ios` and `aarch64-apple-ios-sim`.

Passing compile checks establish build feasibility, not production iOS UI readiness.

## Compatibility/fixture implications

The detector test renders four `tagStandard41h12` tags through the same official library, places them into a generated grayscale frame, detects them, resolves semantic roles, and verifies orientation and quality fields.

That generated test is necessary but not sufficient. Milestone 7 still requires legally usable photographed, blur, glare, missing-marker, wrong-layout, duplicate, revision, and corrupted fixtures with explicit source/license metadata and tolerances.

Changing detector implementation or family after those fixtures are committed will require explicit compatibility review and may require re-deriving tolerances.

## License review

The committed engineering review is:

`docs/reviews/APRILTAG_LICENSE_REVIEW_2026-07-27.md`

Both `apriltag-sys 0.4.0` and the bundled official AprilTag source use BSD-2-Clause terms compatible with the Apache-2.0 A2D project. Binary distribution must reproduce the required notices and disclaimer. Release packaging must include third-party notices before an APK is distributed.

## Consequences and tradeoffs

Positive consequences:

- The project gets a mature official detector and recommended family.
- Marker identity, geometry, and quality data are available in the shared Rust core.
- Android and future iOS can share the same processing implementation.
- Host-library drift is prevented by bundled static compilation.

Costs and risks:

- The project owns a small unsafe FFI layer and must keep it heavily tested.
- Native mobile builds add CI time and toolchain complexity.
- The current safe boundary performs one frame copy.
- Physical fixture thresholds remain unmeasured and cannot yet drive auto-capture policy.

## Validation evidence

- [x] Official detector integrated as code, not prose only.
- [x] Exact dependency version pinned and bundled static build forced.
- [x] Safe ownership/error wrapper implemented with unit tests.
- [x] Marker ID, corners, center, decision margin, and Hamming-error data exposed.
- [x] Rust semantic-role, duplicate-ID, missing-role, and orientation handling implemented.
- [x] License review committed.
- [x] Generated desktop grayscale detector test implemented.
- [ ] Desktop detector test confirmed by the Milestone 7 CI workflow.
- [ ] Reproducible `arm64-v8a` Android build confirmed by CI.
- [ ] Reproducible `x86_64` Android build confirmed by CI.
- [ ] iOS device-target compile feasibility confirmed by CI.
- [ ] iOS simulator-target compile feasibility confirmed by CI.
- [ ] Representative photographed fixture results committed.
- [ ] Performance measurements recorded for representative fixture/device tiers.
- [ ] Final Android packaging into the application APK demonstrated.
- [ ] Third-party notices included in release packaging.

## Follow-up tasks

1. Run and repair the new native-validation workflow until all compile jobs pass.
2. Record workflow run links and measured output in this ADR.
3. Create the Milestone 7 fixture corpus and metadata format.
4. Add photographed/perturbed detector fixtures and tolerance assertions.
5. Expose the validated analysis path through `a2d-core`/UniFFI when Milestone 8 CameraX integration begins.
6. Move this ADR to Accepted only after the required evidence is committed.
7. Update `docs/decisions/README.md` when the status changes.

## Superseding ADR reference

None.
