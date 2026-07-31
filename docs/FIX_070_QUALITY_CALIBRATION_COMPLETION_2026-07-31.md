# FIX-070 — Quality Calibration Contract Completion Record

**Status:** Complete; the exact final commit is required to pass the repository's permanent CI and native-validation workflows before it reaches `master`  
**Date:** 2026-07-31  
**Target branch:** `master`  
**Normative decision:** `docs/decisions/V01_QUALITY_CALIBRATION_CONTRACT.md`

## Scope completed

- [x] Inventory live-guidance, manual-warning, automatic-capture, durable-status, review-state, and preferred-scan threshold uses.
- [x] Classify each threshold use as presentation-only provisional, synthetic-fixture regression, physically calibrated production, or unavailable.
- [x] Keep automatic capture disabled until versioned photographed physical calibration exists.
- [x] Reject construction of an enabled Android automatic-capture policy unless its evidence is physically calibrated and versioned.
- [x] Separate raw measurement from production classification in the Rust image-quality contract.
- [x] Add explicit `Calibrated`, `Provisional`, and `Unavailable` states.
- [x] Add versioned threshold and physical-calibration provenance.
- [x] Preserve raw focus, exposure, and glare metrics regardless of calibration state.
- [x] Emit and persist `QUALITY_THRESHOLDS_UNCALIBRATED` for the current provisional policy.
- [x] Prevent provisional `Accepted` or `AcceptedWithWarnings` values from becoming an unqualified durable production acceptance claim.
- [x] Preserve explicitly user-approved original and derived scan assets when calibration is unavailable.
- [x] Store the durable scan and page quality state as `NeedsReview` while production classification is unavailable.
- [x] Define first-scan preference as explicit workflow initialization, not quality ranking.
- [x] Prevent later scans from automatically replacing the preferred scan based on provisional quality evidence.
- [x] Persist calibration provenance and raw measurements in the `scan.registered` audit event.
- [x] Include threshold-policy version, calibration state, and evidence class in scan pipeline provenance.
- [x] Present provisional calibration, evidence class, warning code, durable review status, and raw metrics explicitly in Android.
- [x] Preserve the existing UniFFI registration ABI: `qualityStatus` is the Rust-owned durable review status and therefore cannot expose provisional acceptance as calibrated production acceptance.

## Production implementation

### Rust image-quality contract

- `crates/a2d-image/src/quality_calibration.rs`
- `crates/a2d-image/src/lib.rs`

The image crate owns calibration state, evidence class, versioned metadata, production-classification qualification, and the stable uncalibrated warning code.

### Rust durable registration

- `crates/a2d-core/src/milestone9.rs`

Registration now returns `RegisteredScanQualityEvidence` inside the Rust core result. The evidence contains calibration metadata, provisional status, optional production status, raw `GrayQualityMetrics`, and the warning code. Current threshold policy version 1 is provisional synthetic-fixture evidence, so production status is absent and durable status is `NeedsReview`.

Raw measurements and classification provenance are written to the `scan.registered` audit event. Scan warnings include `QUALITY_THRESHOLDS_UNCALIBRATED`, and pipeline provenance identifies the threshold policy, calibration state, and evidence class.

### Android policy and presentation

- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerPolicyTypes.kt`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerPolicies.kt`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerContent.kt`
- `apps/android/app/src/main/res/values/strings.xml`

Android V1 declares provisional synthetic-fixture evidence and keeps automatic capture disabled. The registration screen labels the value from Rust as a durable review status, shows calibration state and warning code, states that production classification is unavailable, and exposes the measured values in details.

## Regression coverage

### Rust

- `crates/a2d-image/src/quality_calibration.rs`
  - provisional measurements survive without production acceptance;
  - uncalibrated policy never permits automatic capture;
  - a future calibrated policy is independently versioned and does not alter measurements.
- `crates/a2d-core/src/milestone9_tests.rs`
  - first registration preserves immutable assets and raw metrics while storing `NeedsReview`;
  - calibration state, evidence class, warning code, and versions are returned;
  - scan row and page state remain review-oriented;
  - provenance and warning persistence are verified;
  - Smart Page dimensions remain unchanged;
  - rescan keeps the prior preferred scan and requires review;
  - existing rollback and orphan-preservation behavior remains covered.

### Android JVM

- `apps/android/app/src/test/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerPolicyTest.kt`
  - provisional capture assessment is not production acceptance;
  - automatic capture is disabled;
  - an enabled uncalibrated policy is rejected;
  - a future physically calibrated policy can be versioned and enabled.

### Android emulator

- `apps/android/app/src/androidTest/kotlin/com/a2d/notebook/app/LiveScannerPresentationUiTest.kt`
  - the production calibration composable displays `PROVISIONAL`;
  - synthetic-fixture evidence and `QUALITY_THRESHOLDS_UNCALIBRATED` are visible;
  - production classification is explicitly unavailable;
  - the supported zero-node collection assertion proves that no calibrated presentation is rendered.

## CI-discovered repairs

Permanent CI found and closed two integration defects before signoff:

1. `cargo fmt` identified canonical formatting drift in the four FIX-070 Rust files. The exact formatter output was committed.
2. The Android emulator source set did not provide Compose's `assertDoesNotExist` extension. The test now uses the supported `onAllNodesWithText(...).assertCountEquals(0)` assertion with the same negative semantic guarantee.

A temporary branch-only workflow helper was used to commit the exact rustfmt output. It was then removed, and `.github/workflows/ci.yml` was restored byte-for-byte before the final candidate was tested. No workflow change is part of FIX-070's final tree.

## Permanent validation contract

The exact final FIX-070 commit must pass all checks attached to that commit from the unchanged permanent workflows:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
./gradlew lint test assembleDebug --no-daemon
Kotlin UniFFI binding drift check
Android emulator scanner/presentation/recovery/panic-containment suite
Synthetic fixture regeneration and drift checks
Official detector and Android native ABI validation
Future iOS native compile feasibility
```

The validated commit may be fast-forwarded to `master` only after every required job succeeds. GitHub's checks attached to the final commit are the authoritative execution evidence; this document records the contract and repair history without embedding a stale self-referential commit SHA.
