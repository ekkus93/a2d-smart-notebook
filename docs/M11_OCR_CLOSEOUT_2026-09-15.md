# Milestone 11 OCR Closeout — 2026-09-15

Milestone 11 is implemented and qualified through the durable OCR queue/retry slice merged by PR #60. The post-merge `master` head for that slice is `38a1bae9713df6b805ed11f2ad4d579130d35b52`; permanent CI run `34958132321` passed on that exact head.

## Delivered chain

Milestone 11 now provides a local-first OCR pipeline whose durable/canonical state remains Rust-owned while Android owns platform OCR execution and presentation. The completed chain includes the OCR request/result/provider contract, durable queue/status contract, Rust-validated immutable input selection, explicit terminal-result persistence, text-region persistence, Page Viewer readback, local OCR search and Android search UI, user correction history, selectable text-region overlays, a bundled Android ML Kit provider, and restart-safe queue/retry orchestration.

The final queue slice was PR #60, `M11: add durable OCR queue and retry orchestration`. Its exact qualified PR head was `c861f91ffc10b8339eea4f6b56e39820063e8860`; the squash merge produced `38a1bae9713df6b805ed11f2ad4d579130d35b52` on `master`.

## Guarantees

- Scan saving is independent of OCR success. OCR is enqueued only after durable scan registration; an OCR/provider failure does not undo the saved scan.
- Rust owns durable OCR jobs, state transitions, retry policy, cancellation state, input identity, terminal OCR runs, provenance, text regions, corrections, and search indexing.
- Android executes the bundled local ML Kit provider off the main thread and reports outcomes through the Rust/UniFFI contract rather than fabricating canonical OCR state.
- Queue state survives process death. Interrupted running jobs are reconciled before new work is claimed.
- Active queue size and retry attempts are bounded; retry scheduling is persisted rather than represented only by an in-memory Android timer.
- Cancellation remains explicit and observable. Provider unavailable/failure/cancellation outcomes do not become fake detected empty text.
- Provider, provider-version, model, availability, error, attempt, retry, and last-run diagnostics remain available at the durable queue boundary.
- OCR text and text-region search use the local Rust-owned index; core OCR/search workflows require no account or managed/network OCR service.
- User corrections are stored separately from immutable original OCR output, retaining provenance for both.

## Distinct capabilities

These are separate implemented surfaces and should not be conflated in status documentation:

1. **OCR result persistence** — terminal OCR runs and provenance are durable Rust-owned records.
2. **OCR readback** — persisted OCR runs/regions can be hydrated for Page Viewer presentation.
3. **OCR search index** — Rust-owned local search indexes persisted OCR text and exposes typed search through UniFFI and Android UI.
4. **OCR correction** — user corrections are durable records separate from original OCR output; M11 search continues to index original OCR text.
5. **Real provider support** — Android uses the bundled local ML Kit text-recognition provider behind `AndroidOcrProvider`.
6. **Queue/retry orchestration** — durable Rust jobs are claimed/executed by an app-lifetime Android executor with restart reconciliation, bounded retries, cancellation, and diagnostics.

## Explicit limitations and deferrals

- M11 correction deliberately does **not** replace immutable original OCR in the current `searchOcrText` index. A later unified ranking/source-label design may search both original and corrected text without hiding provenance.
- Queue execution is an app-lifetime local executor backed by durable Rust state, not an Android WorkManager/background-service guarantee after the application is no longer allowed to execute. Durable jobs resume when the app starts again.
- The bundled provider is local and accountless. No cloud/network-only OCR provider is required or introduced by M11.
- OCR quality is not a substitute for the still-open physical print/camera calibration work elsewhere in the roadmap.
- Search scale/latency evidence for the eventual release target remains broader Milestone 12/release-validation work even though the OCR search index/API/UI are implemented.
- The abandoned branch `ralph/m11-ocr-search-index-slice` is explicitly not part of the merged chain; its early bad `Cargo.lock` edit must not be merged.
- PR #43 is an accidental duplicate of an already-merged OCR contract branch and remains intentionally outside the M11 merge chain.

## Qualification evidence

For PR #60 exact head `c861f91ffc10b8339eea4f6b56e39820063e8860`, required validation passed, including Rust formatting/Clippy/workspace tests, cargo-deny, Kotlin UniFFI binding drift checking, Android native/lint/unit/APK validation, Android emulator instrumentation, and Milestone 7 Native Validation.

After merge, permanent CI run `34958132321` passed on exact `master` head `38a1bae9713df6b805ed11f2ad4d579130d35b52`. This is the baseline for the documentation-only closeout PR.

## Closeout rule

Milestone 11 may be called implemented after this documentation reconciliation itself passes exact-head CI, merges, and the resulting `master` CI is green. This closeout does not claim completion of unrelated Milestone 12 scale evidence, Milestone 17 physical calibration, or Milestone 19 release-wide acceptance.
