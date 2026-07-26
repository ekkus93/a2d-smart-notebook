# 0002. Corner-marker (AprilTag) detector selection

**Status:** Proposed — placeholder. Milestone 7.1 (the detector spike) has not started; this ADR
has no decision yet. It exists now so the spike's output has a predetermined home, per TODO 7.1's
requirement to "accept `docs/decisions/0002-apriltag-detector-selection.md`" at the end of that
milestone.
**Date:** 2026-07-26 (placeholder created); decision date TBD.
**Decision owners/authors:** TBD — to be filled in when Milestone 7.1 runs.

## Context and problem

Spec §17.3 requires evaluating the official AprilTag 3 native detector library as the primary
option for reading the four Corner Markers on every writable page, with a pure-Rust alternative
considered only if it materially reduces packaging risk. The selected implementation must build
reproducibly for the required Android ABIs, have a documented license review, produce
deterministic fixture results within tolerance, expose marker ID/corners/quality data, and be
structured so the same native implementation can target iOS later (spec §17.3).

## Constraints

- Spec §17.3's evaluation criteria (license, reproducible Android ABI builds, deterministic
  fixture results, iOS feasibility).
- TODO 7.1's requirement that the spike end with code and tests, not prose only.
- CLAUDE.md: "Don't invent thresholds" — any detection-quality thresholds here must be measured,
  not assumed.
- No native marker library or wrapper crate exists yet in this workspace.

## Options considered

_To be filled in during Milestone 7.1._ Expected candidates per spec §17.3:

1. Official AprilRobotics AprilTag 3 C library via a reviewed Rust FFI wrapper.
2. A pure-Rust AprilTag-compatible detector, considered only if option 1's packaging risk
   (cross-compilation for Android ABIs, license terms, binary size) proves materially worse.

## Decision

_Not yet made._

## Detailed rationale

_Not yet made._

## Security/privacy implications

_To be assessed during the spike — in particular, the memory-safety boundary of any `unsafe` FFI
wrapper around a C library, per the constraint that panics/UB MUST NOT cross into Rust as a
successful result._

## Portability implications for Android and future iOS

_To be assessed during the spike._

## Compatibility/fixture implications

Detector output feeds `fixtures/scans/` fixture expectations (marker roles, warnings, tolerances).
Changing detectors after fixtures are committed may require re-deriving tolerance values.

## Consequences and tradeoffs

_To be filled in during Milestone 7.1._

## Validation evidence

**Required before Accepted** (per TODO 7.1 and spec §17.3):

- [ ] License review, committed.
- [ ] Reproducible build results for every required Android ABI.
- [ ] Desktop fixture detection results against representative grayscale fixtures.
- [ ] Performance measurements (not guessed thresholds).
- [ ] Documented memory-safety boundary for the native wrapper.
- [ ] Packaging strategy (how the native library ships inside the Android app).
- [ ] Future iOS build feasibility assessment.

## Follow-up tasks

- Run the Milestone 7.1 spike and fill in this ADR completely.
- Update `docs/decisions/README.md`'s index row once this ADR's status changes.

## Superseding ADR reference

None.
