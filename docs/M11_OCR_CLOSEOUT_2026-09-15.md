# Milestone 11 OCR closeout — 2026-09-15

Milestone 11 now has a complete local-first OCR path on Android: durable Rust-owned queue state, a bundled on-device provider, OCR result and text-region persistence, readback, local search, correction, text-region overlays, and restart-safe queue/retry orchestration.

## Guarantees

- Rust owns durable OCR job state, scan/input identity validation, OCR runs, text regions, correction provenance, search indexing, retry limits, and terminal semantics.
- Android owns platform-provider invocation and presentation; it does not fabricate canonical OCR state.
- OCR execution uses the bundled on-device ML Kit provider and does not require an account, managed service, or network OCR.
- Scan saving remains independent from OCR success. Provider unavailability/failure is recorded explicitly rather than converted to fake detected text.
- Queue state survives process death. Interrupted running work is reconciled on restart, active queue size and retry attempts are bounded, and cancellation remains observable and truthful.
- Both single-page and batch durable scan registration paths enqueue OCR work after registration succeeds.
- Persisted OCR text and regions can be read in Page Viewer and searched through the local Rust-owned index.
- User corrections are stored separately from immutable original OCR output with provenance preserved.

## Functional boundaries

OCR result persistence, OCR readback, OCR search indexing, OCR correction, the real Android provider, and queue/retry orchestration are separate layers and are all implemented for the current Android/local-first scope. Search currently indexes immutable original OCR text/regions; unified ranking/source labels across original and corrected text are deferred. Advanced handwriting/math OCR, diagram understanding, spread scanning, and dewarping remain post-v0.1 work. iOS has no production OCR provider UI in this milestone.

## Validation evidence

The queue/retry orchestration slice was merged through PR #60. Its exact green PR head was `c861f91ffc10b8339eea4f6b56e39820063e8860`. The squash merge produced `master` commit `38a1bae9713df6b805ed11f2ad4d579130d35b52`.

Post-merge `master` CI passed on that exact commit:

- CI run `34958132321` — success.
- Publish Hosted CI Status run `34959290492` — success.

The validated queue slice includes Rust fmt/Clippy/workspace tests, cargo-deny, Kotlin UniFFI drift checking, Android native/lint/unit/APK validation, emulator instrumentation, and Milestone 7 Native Validation.

## Deferred items

- Unified search ranking/source labeling across original OCR and user-corrected text: deferred so immutable original OCR provenance is not silently hidden by correction precedence.
- Advanced handwriting/math OCR and document-understanding features: explicitly post-v0.1 product scope.
- Production iOS OCR adapter/presentation: deferred with the broader production iOS client.
- OCR-driven Needs Review policy beyond explicit persisted unavailable/failure diagnostics: future product-policy work; the current implementation preserves truthful queue/provider state without inventing review decisions.

## Historical cleanup

The abandoned `ralph/m11-ocr-search-index-slice` branch is not part of the merged implementation and must not be used as a merge source. PR #43 is an accidental duplicate of an already-merged OCR contract branch and is intentionally excluded from the M11 implementation chain.
