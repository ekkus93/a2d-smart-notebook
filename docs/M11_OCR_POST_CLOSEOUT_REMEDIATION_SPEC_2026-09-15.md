# M11 OCR Post-Closeout Remediation Specification — 2026-09-15

## Purpose

This specification defines remediation required after the post-closeout review of Milestone 11 OCR in `ekkus93/a2d-smart-notebook`.

Review baseline:

- `master`: `da8a6528ce1a72c1612456bf643618b48b0bb2fc`
- Original checklist: `docs/M11_OCR_CLOSEOUT_TODO_2026-09-14.md`

The review found that the lower-level OCR persistence/provider/queue work is substantially sound, but several items marked complete are not connected through the production Android composition path. It also found cross-layer consistency defects around result persistence, region persistence, cancellation, overlay geometry, and duplicated queue policy.

This remediation must fix those defects without weakening the project's architectural boundaries.

## Architectural invariants

### Rust owns durable truth

Rust remains authoritative for:

- OCR job IDs and durable job state.
- OCR run IDs and terminal OCR outcomes.
- Scan/asset identity and ownership validation.
- OCR provenance.
- Text-region persistence.
- Correction persistence/provenance.
- Search indexing.
- Retry policy and bounded attempts.
- Cancellation state.
- Cross-record consistency invariants.
- Durable OCR transaction boundaries.

Android/Kotlin must not invent canonical durable OCR state.

### Android owns platform execution and presentation

Android may own:

- ML Kit/provider execution.
- Coroutine/lifecycle orchestration.
- Navigation.
- Compose state/rendering.
- User interaction.
- Display transforms.

Durable OCR presentation state must be hydrated from Rust/UniFFI.

### Local-first remains mandatory

The remediation must not require accounts, cloud OCR, server search, or network-only operation.

### User-visible completion requires production wiring

A route, controller, adapter, or composable is not complete merely because it exists and has isolated tests. Production composition must connect the real dependency chain and be covered by integration tests.

## Findings and required remediation

### 1. OCR Search production route is disconnected

Observed review state:

- `A2dNavHost` opens `OcrSearchScreen` without a production `AndroidOcrSearchController`.
- The screen defaults to a null controller and its built-in not-connected error path.
- The FFI gateway/controller are primarily exercised in tests.

Required outcome:

- Obtain the active/open-library `A2dClient` from a production session owner.
- Construct/inject the real FFI search gateway/controller.
- Execute potentially blocking FFI search work off the UI thread.
- Preserve explicit validation/storage error states.
- Keep search local-first.
- Navigate a selected hit to the matching Page Viewer.

Acceptance criterion:

> Persisted OCR written through the real Rust storage path can be queried from the production OCR Search destination and opened in Page Viewer without a fake gateway.

### 2. Page Viewer OCR readback/actions are not wired in production

Observed review state:

- `FfiAndroidOcrReadback` exists.
- Production Page Viewer is invoked with default OCR state and no-op Start/Retry/Cancel callbacks.

Required outcome:

- Add/extend a production Page Viewer state holder/controller/ViewModel.
- Hydrate page, preferred/current scan, OCR job state, latest OCR result, and text regions from Rust/FFI.
- Wire Start, Retry, and Cancel to durable queue APIs.
- Preserve distinct never-requested, queued, running, detected, no-text, unavailable, cancellation-requested, and cancelled states as supported by the domain model.
- Refresh state after durable transitions.

Acceptance criterion:

> Opening Page Viewer through production navigation reflects Rust-owned OCR state and exposes working durable OCR actions.

### 3. OCR correction is not a reachable production workflow

Observed review state:

- Correction gateway/controller/data models exist.
- No complete production Compose/navigation path lets a user enter/review corrections.

Required outcome:

- Add a correction destination or Page Viewer correction surface.
- Reach it from a selected OCR region and/or supported full-text target.
- Hydrate correction history from Rust.
- Submit corrections through Rust/FFI.
- Keep original OCR immutable and visibly distinct from corrections.
- Preserve durable provenance.

`previous_text` must be audited. Rust should derive canonical prior/original text from durable references where possible rather than trusting arbitrary Android-supplied text. If correction-to-correction lineage exists, model the prior correction explicitly by durable ID.

Acceptance criterion:

> A user can perform a correction in the production app, reopen the page, and observe immutable original OCR plus durable correction history.

### 4. Detected OCR finalization is not atomic

Observed review state:

1. Android records the OCR run.
2. Android records text regions separately.
3. Queue processing can finalize from `recordedRun` even if region persistence failed.

This can leave a detected/indexed OCR run and a successful queue state even though required region persistence failed.

Required outcome:

Introduce a Rust-owned transactional finalization API for a claimed OCR attempt. The transaction must coherently handle, as applicable:

- Claimed-job/input identity validation.
- Terminal OCR run persistence.
- Required text-region persistence.
- Search-index trigger effects.
- Queue terminal transition or retry decision.
- Attempt/error/provider diagnostic state.

For `Detected`, required run/regions/index effects/queue success must commit together. Failure of any required write rolls back terminal success.

For `NoTextDetected`, commit the terminal run and queue completion with no fabricated text.

For `Unavailable`, persist the outcome and coherent retry/terminal decision.

Acceptance criterion:

> Failure injection at any required persistence step cannot leave a successful queue job with a partially committed detected OCR result or orphan searchable text.

### 5. Cancellation-vs-completion race semantics must be explicit

Use a durable commit-point model:

1. Before terminal-result commit, a durable cancellation request wins.
2. Once transactional terminal OCR completion commits, completion wins.
3. A late cancellation request must not rewrite an already accepted completed job to `Cancelled`.
4. If cancellation is observed before commit, detected output must not become accepted searchable OCR.

The finalizer should reject stale workers using attempt number, claim generation, status predicate, or equivalent concurrency guard where practical.

Tests must cover cancellation:

- Before provider work.
- During provider execution.
- After provider return but before commit.
- Racing with commit.
- After successful commit.

Acceptance criterion:

> Queue state, terminal OCR state, search index, and durable text regions never disagree about whether a detected result was accepted.

### 6. OCR overlay uses an invalid coordinate frame

Observed review state:

- Overlay dimensions are inferred from polygon extrema.
- Polygons are not guaranteed to share the actual page-image display transform.
- Aspect-ratio clamping can distort coordinates.

Required outcome:

- Hydrate authoritative source image width/height and asset/scan identity.
- Render the actual page/scan image.
- Compute one content-fit transform for the image.
- Apply exactly the same scale and offset to OCR polygons.
- Preserve aspect ratio and letterboxing offsets.
- Do not derive the coordinate frame from OCR content extents.
- Do not clamp aspect ratio in a way that changes geometry.
- Keep hit testing in the same coordinate system.

Acceptance criterion:

> Known source-pixel polygons map to the same geometric location on the displayed image under the real image transform.

### 7. Rust must enforce polygon/image bounds

For each persisted point, Rust must enforce the documented boundary convention equivalent to:

```text
0 <= x <= source_image_width
0 <= y <= source_image_height
```

Rust must reject invalid image dimensions, NaN/infinite/negative/out-of-bounds coordinates, malformed polygons, and region/run/input mismatches. Android may clamp provider output defensively, but Android clamping is not the durable invariant.

### 8. Region readback must not silently truncate overlays

The reviewed Android readback uses a default limit of 50. A page may have more regions.

Choose and document one bounded strategy:

- Fetch all regions up to a Rust-owned maximum; or
- Page until complete; or
- Explicitly expose partial/truncated state and disclose it in UI.

Preferred Page Viewer behavior is complete bounded hydration consistent with provider/storage limits.

### 9. OCR queue/status/retry policy is duplicated

Observed review state includes queue/status definitions in both `a2d-ocr` and `a2d-core`, with contradictory attempt limits (including 25 vs runtime 3).

Required outcome:

- Audit all queue/status references.
- Select one authoritative durable queue owner.
- Select/document one runtime retry policy.
- Remove/deprecate duplicate dead definitions.
- Align FFI, Android mapping, tests, and documentation.
- Preserve compatibility with existing libraries; do not rewrite old migrations solely to make constants match.
- Add a forward migration only if schema change is necessary and migration-safe.

### 10. Production integration-test coverage is insufficient

Add production-path tests that use real application composition rather than injecting feature-specific fakes for the dependency being verified.

Required coverage includes:

- Library Hub -> OCR Search.
- Persisted OCR -> production search result.
- Search hit -> Page Viewer.
- Page Viewer OCR hydration.
- Start/Retry/Cancel durable behavior.
- Page Viewer -> correction entry/review.
- Correction persistence and reopen/readback.
- Atomic-finalization rollback.
- Late/racing cancellation.
- Overlay transform math with known source dimensions/polygons.
- Region counts above the previous implicit 50-row limit or explicit truncation behavior.

Tests should act as composition sentinels: removing a production controller/readback/action binding should make CI fail.

### 11. Documentation/status currently overstates completion

Preserve the earlier closeout and CI evidence as historical facts, but add a post-closeout review/remediation note to:

- `docs/M11_OCR_CLOSEOUT_TODO_2026-09-14.md`
- `docs/M11_OCR_CLOSEOUT_2026-09-15.md`
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`
- Relevant user-facing/developer-facing Android strings/comments.

Until remediation is complete, status should say that M11 core OCR infrastructure exists but post-closeout production-integration and consistency remediation is in progress.

## Production architecture expectations

There should be one clear production path from the active library/session owner to the open `A2dClient`, and from there to:

```text
active A2dClient
  -> OCR search gateway/controller
  -> Page Viewer OCR readback/state holder
  -> OCR correction gateway/controller
  -> OCR queue/executor controls
```

A full DI framework is not required. Avoid ad hoc native-client construction inside recomposing composables.

## Transaction/migration requirements

A new finalization request may contain:

- job ID
- attempt/claim concurrency guard
- provider diagnostics/provenance
- one explicit terminal outcome:
  - detected full text + regions
  - no text detected
  - unavailable with reason/retryability

The response should report durable job status, committed run ID when applicable, retry state, and cancellation/completion resolution.

All accepted terminal writes must be inside one SQLite transaction. Trigger-maintained FTS effects must roll back with the transaction.

If schema changes are required:

- Add forward-only migrations.
- Do not rewrite released migrations.
- Existing libraries through migration 0011 must open.
- Existing OCR jobs/runs/regions/corrections must remain readable.
- Include upgrade tests.

## CI requirements

Every code-bearing remediation PR must pass applicable existing gates:

- Rust formatting drift check.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `cargo test --workspace --all-features`.
- `cargo deny`.
- UniFFI generated-binding drift check.
- Android native library/binding generation.
- Android lint/unit tests.
- Debug APK verification.
- Android emulator instrumentation.
- Milestone 7 Native Validation when triggered.

If instrumentation classes change, update `.github/workflows/ci.yml` in the same PR.

No implementation slice is complete until its exact PR head is green, merged through the guarded path, and post-merge `master` CI is verified.

## Non-goals

Do not expand this remediation to:

- Cloud OCR.
- Accounts/login.
- Server search.
- Generative correction/rewrite.
- WorkManager/background-service guarantees beyond existing documented app-lifetime execution unless separately approved.
- A new global DI framework solely for this work.
- Unified original+corrected search ranking unless separately specified.
- Milestone 12 scale benchmarking beyond validation needed for these fixes.

## Definition of done

Remediation is complete only when:

1. Production OCR Search uses the real Rust/FFI search path.
2. Production Page Viewer hydrates real OCR job/result/region state.
3. Start/Retry/Cancel are real durable actions.
4. Production correction UI is reachable and durable.
5. Original OCR remains immutable when corrections are added.
6. Detected OCR result, required regions, search effects, and queue-success finalization are transactionally coherent.
7. Cancellation/completion races obey one documented commit-point rule.
8. Queue state cannot contradict accepted searchable OCR state.
9. Overlay uses authoritative source dimensions and the real image transform.
10. Rust rejects out-of-bounds persisted polygons.
11. Region hydration cannot silently truncate a supposedly complete overlay.
12. One authoritative queue/status/retry contract remains.
13. Production composition tests cover the previously missed wiring seams.
14. Existing Rust/Android/FFI/CI gates remain green.
15. Historical closeout/roadmap/status docs are reconciled honestly.
16. The companion remediation TODO has no unchecked implementation, qualification, documentation, or cleanup item.
17. Final exact-head CI and resulting post-merge `master` CI are green.
