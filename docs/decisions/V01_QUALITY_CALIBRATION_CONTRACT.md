# A2D Smart Notebook v0.1 Quality Calibration Contract

**Status:** Normative and implemented for v0.1  
**Scope:** Scanner live guidance, manual capture warnings, automatic capture, durable quality status, initial preferred-scan establishment, later preferred-scan selection, and review-state creation

## 1. Core rule

Image-quality measurement and production classification are separate operations.

The application may always preserve and report bounded raw measurements. It may only present an unqualified production classification, enable automatic capture, or use quality as an authoritative preferred-scan ranking signal when the applicable threshold policy has reviewed photographed physical calibration evidence.

A synthetic test fixture can prove deterministic behavior and prevent regressions. It cannot prove that a threshold is suitable for real cameras, paper, lighting, motion blur, glare, device processing, or user capture distance.

## 2. Calibration states

- `Calibrated`: the threshold policy has versioned, reviewed photographed physical evidence and may make production classification claims.
- `Provisional`: thresholds exist for deterministic regression behavior, presentation guidance, or explicit manual warnings, but do not support an unqualified production acceptance claim.
- `Unavailable`: no applicable threshold policy exists. Raw measurements remain available where measurement succeeds.

Any state other than `Calibrated` emits `QUALITY_THRESHOLDS_UNCALIBRATED` wherever a consumer could otherwise mistake a provisional result for a production claim.

## 3. Threshold inventory

| Use | Current v0.1 values | Evidence classification | Allowed effect |
|---|---|---|---|
| Live marker/focus/exposure/glare/framing guidance | Android `SinglePageScannerPolicies.V1.guidance` | Presentation-only provisional, backed by synthetic regression fixtures | Show directional guidance and warnings. Never claim calibrated quality. |
| Manual capture warning | Android `SinglePageScannerPolicies.V1.captureThresholds` | Synthetic-fixture regression | Permit explicit user-reviewed capture while preserving warnings and raw metrics. |
| Automatic capture | Timing/debounce values exist, but `autoCaptureEnabled = false` | Physical calibration unavailable | Must remain disabled. Construction of an enabled policy fails unless calibration metadata is physically calibrated and versioned. |
| Durable scan quality status | Rust Milestone 9 threshold policy version 1 | Synthetic-fixture regression; calibration state `Provisional` | Preserve original and derived assets, persist raw metrics and policy provenance, emit `QUALITY_THRESHOLDS_UNCALIBRATED`, and store `NeedsReview` rather than an unqualified `Accepted` claim. |
| Needs Review creation | Existing-page and provisional quality state | Provisional | Create or retain review work. Calibration unavailability must not suppress review evidence or discard the original. |
| Initial preferred scan | First scan only, after explicit user approval and all identity/integrity checks | Workflow decision; not a quality ranking | May establish the page's initial preferred pointer because no competing scan exists. The page remains `NeedsReview` while production quality classification is unavailable. |
| Later preferred-scan selection | Existing transaction/integrity workflow | Calibration unavailable for quality ranking | Provisional quality cannot promote a later scan. A reviewed manual selection remains allowed through the authoritative atomic preferred-scan workflow. |

## 4. Durable preservation and review behavior

Calibration unavailability is not a reason to discard or refuse to preserve an explicitly user-approved original capture. The immutable original remains the primary recovery artifact.

For provisional or unavailable calibration:

1. Preserve the original and derived assets when all non-quality integrity checks succeed.
2. Preserve raw quality measurements, provisional status, threshold-policy version, evidence class, and calibration state in the registration result and durable audit event.
3. Persist `QUALITY_THRESHOLDS_UNCALIBRATED` with the scan and expose it in the registration result.
4. Do not represent provisional `Accepted` or `AcceptedWithWarnings` as calibrated production acceptance.
5. Store the durable scan and page quality state as `NeedsReview` while production classification is unavailable.
6. Permit the explicitly user-approved first scan to establish the initial preferred pointer only as a workflow initialization; do not treat that pointer as a quality endorsement.
7. Never promote a later scan over the existing preferred scan based on provisional quality evidence.
8. Preserve all measured values even when a threshold marks the provisional assessment as warning or accepted.

The current Android registration UI receives the Rust-owned durable status and therefore displays `NeedsReview`, not `Accepted`, for the provisional policy. Android's scanner policy independently exposes `PROVISIONAL`, synthetic-fixture evidence, and `QUALITY_THRESHOLDS_UNCALIBRATED`; automatic capture remains fail-closed.

## 5. Durable evidence locations

The v0.1 registration path records quality evidence in three complementary places:

- the Rust `RegisteredScanQualityEvidence` result, which contains raw `GrayQualityMetrics`, provisional status, optional production status, calibration metadata, and warning code;
- the scan row, whose durable status is `NeedsReview`, whose warnings include `QUALITY_THRESHOLDS_UNCALIBRATED`, and whose pipeline provenance includes threshold-policy version, calibration state, and evidence class;
- the `scan.registered` audit event, which retains raw focus, exposure, and glare measurements plus the complete calibration and classification provenance.

The UniFFI `RegisteredScan.qualityStatus` remains the durable review-oriented status. It deliberately does not expose a synthetic `Accepted` result as production quality. The reviewed Android capture artifact remains available in UI state with its raw preview measurements after registration.

## 6. Versioning rule

Threshold-policy version and physical-calibration version are independent:

- `threshold_policy_version` changes when numeric thresholds or classification logic change.
- `physical_calibration_version` identifies the reviewed photographed evidence set and calibration procedure.
- A calibrated policy requires both versions to be positive and must declare physically calibrated production evidence.
- Updating either version never mutates stored original image bytes.
- A future calibrated policy may populate production classification without changing the raw metric representation or rewriting prior scans.

## 7. Required physical evidence before enabling automatic capture

At minimum, a reviewed calibration set must cover:

- representative supported Android devices and camera pipelines;
- multiple lighting levels and color temperatures;
- motion blur and focus variation;
- specular glare and localized highlights;
- page coverage, edge margin, perspective, and capture distance;
- supported printable layouts, paper finishes, and marker print quality;
- false-positive and false-negative rates for every automatic-capture gate;
- explicit acceptance criteria and a reproducible calibration procedure.

Until that evidence is committed and reviewed, automatic capture remains disabled.
