# A2D Smart Notebook v0.1 Quality Calibration Contract

**Status:** Normative for v0.1  
**Scope:** Scanner live guidance, manual capture warnings, automatic capture, durable quality status, preferred-scan eligibility, and review-state creation

## 1. Core rule

Image-quality measurement and production classification are separate operations.

The application may always preserve and report bounded raw measurements. It may only present an unqualified production classification, enable automatic capture, or use quality as an authoritative preferred-scan decision when the applicable threshold policy has reviewed photographed physical calibration evidence.

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
| Durable scan quality status | Current Rust Milestone 9 threshold mapping | Synthetic-fixture regression pending registration-result qualification | Preserve the original and raw metrics. Do not expose `Accepted` as an unqualified calibrated production result. |
| Needs Review creation | Existing-page and provisional quality warnings | Provisional | May create or retain review work; unavailable calibration must not suppress review evidence. |
| Preferred-scan selection | Existing transaction/integrity rules plus quality evidence | Calibration unavailable for quality ranking | Uncalibrated quality cannot automatically promote a scan. Manual reviewed selection remains allowed through the authoritative preferred-scan workflow. |

## 4. Durable preservation and review behavior

Calibration unavailability is not a reason to discard or refuse to preserve an explicitly user-approved original capture. The immutable original remains the primary recovery artifact.

For provisional or unavailable calibration:

1. Preserve the original and derived assets when all non-quality integrity checks succeed.
2. Preserve raw quality measurements and threshold-policy identity.
3. Emit `QUALITY_THRESHOLDS_UNCALIBRATED`.
4. Do not represent provisional `Accepted` as calibrated production acceptance.
5. Do not automatically make the scan preferred based solely on provisional quality.
6. Create or retain review state when existing-page, integrity, or provisional-quality evidence requires human review.

## 5. Versioning rule

Threshold-policy version and physical-calibration version are independent:

- `threshold_policy_version` changes when numeric thresholds or classification logic change.
- `physical_calibration_version` identifies the reviewed photographed evidence set and calibration procedure.
- A calibrated policy requires both versions to be positive and must declare physically calibrated production evidence.
- Updating either version never mutates stored original image bytes.

## 6. Required physical evidence before enabling automatic capture

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