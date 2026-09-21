# Milestone 11 OCR Closeout — 2026-09-15

## Second post-closeout review addendum — 2026-09-21

The 2026-09-19 completion addendum below is historical evidence for the first post-closeout remediation, but it is not the final statement of M11 production-path sufficiency. A second source review at baseline `ba6f87d7eab75730ce7c715f87ac1ae615d1c730` found that prior CI had passed while some shipped Android paths still called older compatibility APIs rather than the stronger Rust primitives that the first remediation had added and tested in isolation.

The second-review requirements are specified in `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_SPEC_2026-09-19.md`, tracked in `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_TODO_2026-09-19.md`, and summarized in `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_CLOSEOUT_2026-09-21.md`. The second review specifically required production transactional finalization, cancellation commit-point enforcement through the real executor, scanner/Page Viewer active-job identity agreement, Rust-owned manual retry, authoritative source geometry and polygon bounds, a real shared image/overlay transform, explicit complete/partial region hydration, production-composition sentinels, cancellation propagation, and accurate active-client lifecycle documentation.

Accordingly, statements below that the first remediation was “complete” describe that earlier tracker state and its then-recorded CI evidence; they do not supersede the second-review tracker. M11 may be represented as fully remediated only after the second-review TODO reaches zero unchecked implementation/testing/qualification/documentation/cleanup items and its final post-merge `master` CI is green.

## Post-closeout remediation completion addendum — 2026-09-19

This document remains the historical record of the original M11 closeout and merge chain. The post-closeout review addendum below correctly identified that the original closeout overstated production integration. That remediation was implemented through PR #112 and reconciled in `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_CLOSEOUT_2026-09-19.md`.

Historical status at that point: M11 OCR implementation and the first post-closeout production-integration remediation were considered complete. Broader release/physical OCR-quality evidence, corrected+original unified search ranking, WorkManager/background-service guarantees, and Milestone 12 search scale remained outside that remediation unless separately implemented.

Final remediated implementation baseline before that documentation closeout slice: `89c7e27264643f53a0f83da0d061b56fe7989feb`. PR #112 exact-head CI run `35429371063` passed, and post-merge `master` CI run `35430897974` passed.

## Post-closeout review addendum — remediation open

This closeout document is a historical record of the original M11 merge chain and CI evidence. A later post-closeout source review found that the lower-level OCR infrastructure was substantially present, but the original closeout overstated production integration. Remediation was tracked in `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_TODO_2026-09-15.md` and specified in `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_SPEC_2026-09-15.md`.

Until that remediation was complete, M11 status was: core OCR persistence/provider/search/correction/queue infrastructure existed, but production composition and consistency remediation remained open. The known gaps included real OCR Search injection, real Page Viewer OCR hydration and durable actions, reachable correction UI, transactional detected-result finalization, explicit cancellation/commit race semantics, overlay geometry based on authoritative source dimensions, Rust polygon bounds validation, non-silent region hydration limits, queue/status/retry contract consolidation, and production-path integration tests.

The PR and CI evidence below remains valid historical evidence for the slices it exercised. It did not prove production composition seams later identified by either post-closeout review.

Milestone 11 was implemented and qualified through the durable OCR queue/retry slice merged by PR #60. The post-merge `master` head for that slice is `38a1bae9713df6b805ed11f2ad4d579130d35b52`; permanent CI run `34958132321` passed on that exact head.

## Delivered chain

Milestone 11 provides a local-first OCR pipeline whose durable/canonical state remains Rust-owned while Android owns platform OCR execution and presentation. The chain includes the OCR request/result/provider contract, durable queue/status contract, Rust-validated immutable input selection, explicit terminal-result persistence, text-region persistence, Page Viewer readback, local OCR search and Android search UI, user correction history, selectable text-region overlays, a bundled Android ML Kit provider, and restart-safe queue/retry orchestration. Later remediation hardened how these pieces compose in production.

The original final queue slice was PR #60, `M11: add durable OCR queue and retry orchestration`. Its exact qualified PR head was `c861f91ffc10b8339eea4f6b56e39820063e8860`; the squash merge produced `38a1bae9713df6b805ed11f2ad4d579130d35b52` on `master`.

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
- Mid-process active-library switching is intentionally unsupported; the current app uses one app-private `A2dClient` for the process lifetime and shares it across queue, Search, Page Viewer, and correction composition.
- The abandoned branch `ralph/m11-ocr-search-index-slice` is explicitly not part of the merged chain; its early bad `Cargo.lock` edit must not be merged.
- PR #43 is an accidental duplicate of an already-merged OCR contract branch and remains intentionally outside the M11 merge chain.

## Qualification evidence

For PR #60 exact head `c861f91ffc10b8339eea4f6b56e39820063e8860`, required validation passed, including Rust formatting/Clippy/workspace tests, cargo-deny, Kotlin UniFFI binding drift checking, Android native/lint/unit/APK validation, Android emulator instrumentation, and Milestone 7 Native Validation.

After merge, permanent CI run `34958132321` passed on exact `master` head `38a1bae9713df6b805ed11f2ad4d579130d35b52`. This remains historical evidence for that slice; later remediation has its own exact-head and post-merge evidence.

## Closeout rule

The original documentation closeout did not prove later-discovered production composition gaps. Current M11 remediation status is governed by `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_TODO_2026-09-19.md`. Unrelated Milestone 12 scale evidence, Milestone 17 physical calibration, and Milestone 19 release-wide acceptance remain outside M11 remediation.
