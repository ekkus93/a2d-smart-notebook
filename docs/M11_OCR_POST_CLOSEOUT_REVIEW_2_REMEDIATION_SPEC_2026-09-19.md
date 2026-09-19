# M11 OCR Post-Closeout Review 2 Remediation Specification — 2026-09-19

## Purpose

This specification reopens Milestone 11 OCR remediation after a second source review of current `master` found that several previously checked post-closeout items were not actually enforced through the production Android execution path.

Reviewed baseline:

- repository: `ekkus93/a2d-smart-notebook`
- `master`: `ba6f87d7eab75730ce7c715f87ac1ae615d1c730`
- prior remediation tracker: `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_TODO_2026-09-15.md`
- prior remediation specification: `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_SPEC_2026-09-15.md`

The earlier remediation substantially improved the Rust core, production search/correction wiring, queue policy, and integration coverage. However, the second review found a recurring class of defect: the correct Rust primitive may exist and may have isolated tests while production Android still calls an older compatibility API. This specification requires production-path proof for every remediated invariant.

Until this specification and its companion TODO are fully complete, M11 must not be represented as fully remediated.

## Review findings that require reopening

The following findings are considered confirmed against the reviewed baseline:

1. The Rust transactional OCR finalizer exists, but normal production Android still persists OCR run, OCR regions, and queue completion through separate calls.
2. Region persistence failure can leave a recorded OCR run attached to a failed workflow result, after which the queue processor can still mark the job complete because a run ID exists.
3. Cancellation commit-point semantics are correct inside the new finalizer but are not guaranteed by the normal production Android execution path because that path does not use the finalizer.
4. Scanner-created OCR jobs normally use `OcrOptimized`, while Page Viewer active-job lookup and Start/Retry paths use `Original`; active jobs can therefore be missed and duplicate work can be enqueued for one scan.
5. Page Viewer Retry creates a new ordinary job instead of invoking a Rust-owned manual-retry transition with explicit durable eligibility.
6. Page Viewer supplies hard-coded OCR input dimensions rather than authoritative dimensions for the actual OCR source asset.
7. Rust rejects malformed, negative, NaN, and infinite polygon coordinates but does not fully enforce upper bounds against authoritative source-image width/height.
8. The production Page Viewer OCR overlay is not wired to an authoritative source frame and does not render the actual page/scan image under the same content-fit transform.
9. Region write limits and region readback limits differ; a supposedly complete overlay can still be silently partial.
10. Existing production-path tests cover Search, Page Viewer, correction, and actions better than the original closeout, but they do not prove the queue/provider/transactional-finalizer path that carries the strongest persistence invariants.
11. Some coroutine paths catch generic `Exception` without first rethrowing `CancellationException`.
12. Active-library client lifecycle/disposal is weaker than the prior checklist language implies.
13. Stale Page Viewer/M11 text remains, and current closeout/roadmap text overstates completion.

## Architectural invariants

### Rust owns durable truth

Rust remains authoritative for:

- OCR job identity and durable state.
- OCR input identity and source geometry.
- attempt count, automatic retry policy, and manual retry eligibility.
- claimed-attempt concurrency guards.
- cancellation state and completion/cancellation commit-point resolution.
- OCR run identity and immutable original OCR.
- text-region persistence and polygon bounds.
- correction provenance.
- search indexing.
- transaction boundaries.
- complete-versus-partial readback metadata.

Android must not synthesize canonical dimensions, retry budgets, queue eligibility, durable completion, or source-image identity.

### Android owns platform execution and presentation

Android owns:

- local OCR provider execution.
- coroutine/lifecycle orchestration.
- Compose state.
- navigation.
- actual page-image presentation.
- content-fit transforms and hit testing, using Rust-provided source geometry.
- explicit presentation of loading/partial/unavailable/cancelled states.

### Production path is the acceptance path

A Rust API, FFI method, gateway, controller, or unit test is not sufficient evidence by itself. For every user-visible or cross-record invariant in this remediation, at least one test must exercise the same dependency composition and production method chain that the shipped Android app uses.

A completion claim must fail if production Android is changed back to an old compatibility API.

## R11 — Transactional OCR finalization in production

Expose the existing Rust transactional finalization capability through UniFFI. Bind finalization to one claimed OCR attempt and include job ID, attempt/claim guard, provider/model/version diagnostics, explicit Detected/NoTextDetected/Unavailable outcome, detected text, and all required regions.

The finalizer response must report durable job status, committed OCR run ID when applicable, retry/exhaustion state, and whether cancellation or completion won.

For Detected, one Rust transaction must atomically perform claimed-job/input validation, cancellation/stale-claim check, OCR run persistence, all required region persistence, search-index effects, and queue terminal-success transition. Any required write failure must roll back the entire accepted result.

Normal Android execution must stop using the split sequence:

- record OCR run;
- record OCR regions;
- complete OCR job.

The queue processor must call one finalization operation for one claimed attempt. A failed provider/result-persistence path must not be considered successful merely because an OCR run ID exists.

Retained legacy FFI methods must be explicitly documented as compatibility/test/migration-only and must not be used by the normal queue executor.

Required production-path tests:

- successful Detected finalization;
- region persistence failure after provider success;
- NoTextDetected;
- Unavailable;
- stale attempt rejection;
- search invisibility after rolled-back detected result;
- sentinel that fails if Android reverts to split persistence.

## R12 — Cancellation and commit-point semantics through the real executor

Production must implement one rule:

1. Before terminal commit, durable cancellation wins.
2. After successful terminal commit, completion wins.
3. Late cancellation cannot rewrite accepted completion to Cancelled.
4. If cancellation wins before commit, detected output cannot become searchable durable OCR.
5. A stale worker cannot finalize a superseded attempt.

Exercise the production executor with controlled synchronization for cancellation before provider execution, during provider execution, after provider return but before finalizer invocation, racing with finalizer commit, and after successful commit.

For every race assert queue status, OCR run presence/absence, region presence/absence, and search-index visibility.

## R13 — Page Viewer queue identity and manual retry

Page Viewer must not hard-code an OCR input kind that differs from scanner execution. Use a Rust-owned contract that either returns the active OCR job for a scan regardless of input kind or resolves the canonical input kind before lookup/enqueue.

Unless explicitly specified otherwise, one scan must not have simultaneous active `Original` and `OcrOptimized` jobs merely because callers chose different input kinds.

Add a Rust-owned manual retry command. Rust decides retry eligibility, retry bounds, interaction with automatic attempt count, retained diagnostics, cancellation behavior, and restart semantics. Android Retry must call this operation rather than ordinary enqueue and must not reset policy by creating a fresh unrestricted job.

Tests must begin with the normal scanner-created `OcrOptimized` job and prove Page Viewer sees it, avoids duplicate enqueue, and uses the durable manual retry transition.

## R14 — Authoritative source geometry and real image overlay

Rust must expose the actual OCR source asset identity plus validated source width/height. Page Viewer must not use hard-coded dimensions such as 1800x2200.

Rust polygon validation must enforce the documented source-pixel convention:

- `0 <= x <= source_width`
- `0 <= y <= source_height`

Rust must reject invalid dimensions, malformed polygons, NaN/infinite/negative coordinates, upper-bound violations, and region/run/input/source-asset mismatches.

Production Page Viewer must render the actual source image and OCR overlay in one shared coordinate model. Compute one content-fit transform, preserve aspect ratio and letterbox/pillarbox offsets, apply identical scale/translation to polygons, and use the same transform for hit testing.

Required geometry coverage includes square/non-square images, portrait/landscape, letterboxing, all four edges, selection mapping, touches outside rendered image bounds, and relevant orientation assumptions.

## R15 — Complete and explicit OCR region hydration

Reconcile region write and readback limits using a Rust-owned bounded pagination contract. Expose total count when known, page/cursor information, returned count, complete/has-more metadata, and the authoritative maximum supported region count.

Page Viewer may hydrate incrementally, but it must not present a partial overlay as complete. If rendering starts before all pages arrive, state/UI must explicitly show partial/loading state.

Tests must cover more than 50 regions, more than the prior 1,000-row ceiling when within the authoritative maximum, the exact maximum, rejection above maximum, recreation during pagination, and later-page failure without silent truncation.

## R16 — Production integration sentinels

Add at least one full production-composition scenario that:

1. creates/opens a real test library;
2. registers a page and scan through supported APIs;
3. enqueues and claims OCR through the normal queue path;
4. executes a deterministic test provider through the production provider boundary;
5. finalizes through the production transactional finalizer;
6. verifies queue, run, regions, and search visibility;
7. navigates Library Hub -> OCR Search;
8. searches known text;
9. opens the result in Page Viewer;
10. verifies real source image/geometry and regions;
11. opens correction UI;
12. persists a correction;
13. reopens and verifies immutable original OCR plus correction history.

Sentinels must fail if production reverts to split finalization, queue success depends only on recorded run existence, Page Viewer reverts to the wrong input kind, Retry reverts to ordinary enqueue, hard-coded dimensions return, source-image/overlay wiring is removed, or correction production wiring is removed.

Keep emulator runtime bounded by sharing expensive setup where practical.

## R17 — Coroutine cancellation and active-library lifecycle hardening

Audit OCR Search, Page Viewer hydration/actions, correction operations, and queue orchestration for broad exception catches. Suspend/coroutine paths that catch `Exception` must rethrow `CancellationException` before mapping ordinary failures unless explicitly documented otherwise.

Tests must verify cancellation does not become a visible ordinary OCR/search error and does not write stale UI state after navigation/recreation.

Define one production owner for the active/open `A2dClient`. No native client may be opened from recomposing composables. Define deterministic close/dispose and active-library replacement behavior. Search, Page Viewer, correction, and queue actions must bind to the same current client/session.

If the current product intentionally supports only one process-lifetime library with no switching, document that limitation honestly instead of claiming close/change lifecycle support.

## R18 — Status, documentation, stale text, and final closeout

Before declaring completion, update historical closeout and roadmap language so they no longer state that the second-review findings are fixed. Preserve all prior PR/CI evidence as historical fact.

Audit Android resources and source comments for pre-M11/pre-remediation placeholder text, including statements that Page Viewer/OCR APIs are future or unavailable when they now exist.

Record final decisions for transactional finalization, cancellation commit point, active-job identity, manual retry, source geometry, polygon bounds, overlay transform, region pagination/completeness, active-library lifecycle, and correction provenance.

The final source audit must verify:

- normal Android queue execution calls the transactional finalizer;
- no normal success path separately records run, regions, and queue completion;
- queue success cannot be inferred merely from a recorded run ID;
- Page Viewer observes scanner-created active OCR jobs;
- Retry uses Rust-owned manual retry;
- no hard-coded production OCR source dimensions remain;
- Rust checks upper polygon bounds;
- Page Viewer renders the actual source image under the polygon overlay;
- image and polygons share one transform;
- complete overlays do not silently truncate regions;
- coroutine cancellation is preserved;
- active-library lifecycle documentation matches implementation;
- documentation no longer claims the prior remediation was fully sufficient.

## CI requirements

Every code-bearing slice must pass applicable permanent gates on the exact PR head:

- Rust formatting drift check;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace --all-features`;
- cargo-deny;
- Kotlin UniFFI generated-binding drift check;
- Android native/binding generation;
- Android lint/unit tests;
- debug APK verification;
- Android emulator instrumentation;
- Milestone 7 Native Validation when triggered.

If instrumentation classes are added or renamed, update explicit CI class lists in the same PR.

No slice is complete until exact-head CI is green, the exact head is merged, post-merge master CI is green, and the companion TODO is reloaded from current master.

## Non-goals

Do not expand this remediation into cloud OCR, accounts/login, server search, WorkManager/background-service guarantees, generative correction, unified corrected+original search ranking, unrelated scanner/image refactors, a new global DI framework, or Milestone 12 scale benchmarking beyond evidence needed for these fixes.

## Definition of done

This second remediation is complete only when:

1. Production Android uses the Rust transactional finalizer.
2. Run, required regions, search effects, and queue terminal success are one coherent transaction.
3. Region failure cannot leave accepted searchable OCR or successful queue state.
4. Production cancellation/completion races obey one commit-point rule.
5. Stale workers cannot finalize superseded attempts.
6. Page Viewer recognizes the scanner's actual active OCR job/input identity.
7. Manual Retry is Rust-owned and bounded.
8. Page Viewer uses authoritative source-image identity and dimensions.
9. No hard-coded production OCR dimensions remain.
10. Rust rejects polygons outside authoritative source bounds.
11. Production Page Viewer renders the actual image and polygons with one shared transform.
12. Region hydration is complete or explicitly partial/loading.
13. Production tests exercise queue/provider/finalizer/search/viewer/correction composition.
14. Production tests fail if old split APIs are reintroduced.
15. Coroutine cancellation remains cancellation.
16. Active-library lifecycle behavior is implemented or accurately documented as limited.
17. Stale OCR/Page Viewer text is reconciled.
18. Roadmap and closeout documents accurately distinguish historical evidence from current completion.
19. Every code-bearing PR was qualified on its exact head.
20. Every required post-merge master CI run passed.
21. The companion TODO has zero unchecked implementation, testing, qualification, documentation, or cleanup items.
