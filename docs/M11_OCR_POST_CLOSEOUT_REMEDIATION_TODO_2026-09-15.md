# M11 OCR Post-Closeout Remediation TODO — completed reconciliation

Active remediation tracker for the post-closeout Milestone 11 OCR review.

Companion specification:

- `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_SPEC_2026-09-15.md`

Final closeout/evidence record:

- `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_CLOSEOUT_2026-09-19.md`

Review baseline:

- `master`: `da8a6528ce1a72c1612456bf643618b48b0bb2fc`

Remediated implementation baseline before the final documentation slice:

- `master`: `89c7e27264643f53a0f83da0d061b56fe7989feb`
- PR #112 exact PR-head CI: `35429371063`
- PR #112 post-merge `master` CI: `35430897974`

## Operating rules

- [x] Read the companion specification before code changes.
- [x] Use Ralph Bridge for GitHub and CI operations; do not use the default GitHub tool.
- [x] Start each slice from current `master`.
- [x] After every successful merge, reload this TODO from current `master` and continue with the next unchecked item.
- [x] Keep Rust authoritative for durable OCR state, IDs, invariants, provenance, queue policy, transaction boundaries, and search indexing.
- [x] Keep Android responsible for provider execution, lifecycle/orchestration, navigation, and presentation.
- [x] Keep core OCR local-first/accountless.
- [x] Do not hand-edit generated UniFFI Kotlin bindings.
- [x] Do not mark a user-visible feature complete solely from isolated controller/composable/unit tests; exercise the production composition path.
- [x] If instrumentation classes change, update `.github/workflows/ci.yml` in the same PR.
- [x] Merge only exact green PR heads and verify post-merge `master` CI.
- [x] CI/test/lint/format failures are implementation work, not user blockers.

## Defects closed

- [x] Production OCR Search lacks a real search controller/gateway.
- [x] Production Page Viewer does not hydrate persisted OCR via real readback.
- [x] Production Page Viewer Start/Retry/Cancel OCR callbacks are effectively no-ops/defaults.
- [x] Correction controller/gateway lack a complete reachable production UI path.
- [x] OCR run and OCR region persistence are split commits.
- [x] Queue finalization can succeed from a recorded run even when later region persistence failed.
- [x] Cancellation can race with detected-result persistence and contradict searchable OCR state.
- [x] Overlay derives geometry from polygon extrema rather than authoritative source image dimensions.
- [x] Overlay is not guaranteed to share the displayed image transform.
- [x] Rust does not fully enforce polygon bounds against source-image dimensions.
- [x] Android region readback can silently truncate at the previous default limit.
- [x] Queue/status/retry policy is duplicated across Rust layers with conflicting limits.
- [x] CI does not sufficiently test production navigation/dependency composition.
- [x] M11 closeout/roadmap/status text overstates production integration.

## R0 — Baseline and status reconciliation

- [x] Resolve current `master` SHA through Ralph Bridge.
- [x] Verify current `master` CI.
- [x] Reconfirm reviewed wiring/consistency defects still exist at the review baseline.
- [x] Reconcile relevant master advances before modifying code.
- [x] Add post-closeout review/remediation notes to historical M11 closeout docs.
- [x] Update roadmap/status text so M11 was not represented as fully closed while remediation remained open.
- [x] Preserve old PR/merge/CI evidence as historical fact.
- [x] State explicitly that prior CI passed but did not exercise the missing production integration seams.
- [x] Exact-head CI passed.
- [x] Merged exact green head.
- [x] Verified post-merge `master` CI.
- [x] Reloaded TODO from `master`.

## R1 — Production OCR dependency composition and Search

- [x] Identified active-library OCR dependency ownership.
- [x] Defined lifecycle-safe production composition for OCR dependencies.
- [x] Avoided unrelated native clients from recomposing composables.
- [x] Disposed/recreated dependencies when active library closes/changes.
- [x] Constructed `FfiAndroidOcrSearchGateway` from the production client.
- [x] Constructed/injected `AndroidOcrSearchController` into the real OCR Search route.
- [x] Removed normal reliance on null/not-connected controller path.
- [x] Preserved unavailable-library/client error behavior.
- [x] Ran blocking FFI search off the main thread.
- [x] Preserved Rust query validation/error propagation.
- [x] Preserved full-text vs region hit-source metadata.
- [x] Verified hit selection routes to Page Viewer.
- [x] Added production-path search tests that create/open a real test library, persist real OCR, navigate Library Hub -> OCR Search, submit a query, verify persisted hit metadata, select the hit, and verify Page Viewer navigation.
- [x] Production OCR Search returns real persisted OCR.
- [x] Test fails if production controller wiring is removed.
- [x] Required CI passed on exact head.
- [x] Merged exact green head, verified post-merge `master` CI, and reloaded TODO.

## R2 — Production Page Viewer OCR hydration and actions

- [x] Added production Page Viewer OCR state ownership.
- [x] Hydrated page/preferred/current scan from Rust-owned state.
- [x] Hydrated display asset/image metadata.
- [x] Hydrated durable OCR job state.
- [x] Hydrated latest terminal OCR result through real FFI readback.
- [x] Hydrated OCR text regions through real FFI readback.
- [x] Distinguished supported OCR states without fabricating canonical state in Kotlin.
- [x] Wired Start OCR to durable queue enqueue behavior.
- [x] Wired Retry OCR to the authoritative retry API and canonical attempt policy.
- [x] Wired Cancel OCR to durable cancellation request behavior without claiming premature terminal cancellation.
- [x] Added production-path Page Viewer tests for detected/no-text/unavailable/queued/running/cancelled hydration, regions, Start/Retry/Cancel, and route reopen persistence.
- [x] Production Page Viewer no longer depends on default empty OCR state or no-op controls.
- [x] Required CI passed on exact PR heads.
- [x] Merged exact green heads, verified post-merge `master` CI, and reloaded TODO.

## R3 — Reachable OCR correction workflow

- [x] Audited `previous_text` semantics and derived canonical previous/original text from durable Rust records where possible.
- [x] Added production correction presentation/navigation from Page Viewer.
- [x] Passed durable IDs as identity and preserved back navigation state.
- [x] Constructed `FfiAndroidOcrCorrectionGateway` from the active production client.
- [x] Used `AndroidOcrCorrectionController` in the real path.
- [x] Hydrated correction history from Rust.
- [x] Submitted corrections through Rust/FFI.
- [x] Refreshed history and Page Viewer state after success.
- [x] Displayed immutable original OCR distinctly from corrected text.
- [x] Preserved original-only M11 search semantics.
- [x] Added production-path correction tests for navigation, valid correction submission, durable readback/history, immutable original OCR, reopen persistence, invalid target/provenance rejection, and correction validation policy.
- [x] Correction is reachable without test-only dependency injection.
- [x] Original OCR remains immutable.
- [x] Required CI passed on exact head; merged, verified post-merge `master` CI, and reloaded TODO.

## R4 — Transactional OCR finalization

- [x] Defined a Rust-owned finalization command for a claimed OCR job attempt.
- [x] Included stale-worker/concurrency guard semantics.
- [x] Represented `Detected`, `NoTextDetected`, and `Unavailable` explicitly.
- [x] Included detected full text, required regions, provider/model/diagnostic provenance, durable job status, committed run ID when applicable, retry state, and cancellation/completion resolution.
- [x] Implemented one Rust transaction for job/input validation, detected run/region/FTS/queue success, no-text terminal completion, unavailable retry/terminal decision, and rollback on required detected-result write failure.
- [x] Removed normal split-success assumptions from presentation/executor state.
- [x] Added failure-injection evidence showing no partial detected-result success is durable.
- [x] Required Rust/FFI/Android tests and CI passed.
- [x] Merged exact green head, verified post-merge `master` CI, and reloaded TODO.

## R5 — Cancellation/commit race semantics

- [x] Implemented the commit-point policy: cancellation wins before terminal commit; completion wins after successful terminal commit.
- [x] Prevented late cancellation from rewriting accepted completed jobs to `Cancelled`.
- [x] Prevented cancelled-before-commit detected output from becoming accepted/searchable OCR.
- [x] Rejected stale worker finalization where practical.
- [x] Added race tests for cancellation before provider execution, during provider execution, after provider result before commit, racing with commit, and after successful commit.
- [x] Verified queue/run/regions/search-index consistency.
- [x] Required CI passed.
- [x] Merged exact green head, verified `master` CI, and reloaded TODO.

## R6 — Correct OCR region geometry and durable validation

- [x] Exposed authoritative source geometry and source asset/scan identity through readback.
- [x] Validated dimensions are positive/valid.
- [x] Stopped using polygon maxima as image dimensions.
- [x] Documented and enforced pixel-bound convention in Rust.
- [x] Rejected NaN/infinite/negative/out-of-bounds coordinates, malformed polygons, invalid dimensions, and mismatched region/run/input geometry.
- [x] Kept Android clamping defensive rather than canonical.
- [x] Rendered the actual page/scan image and applied one content-fit transform to image and polygons.
- [x] Preserved aspect ratio and letterboxing offsets.
- [x] Removed polygon-extrema coordinate-frame inference and aspect-ratio clamping that distorted geometry.
- [x] Kept hit testing in the same coordinate system and prevented selection outside rendered image bounds.
- [x] Removed silent 50-region readback behavior by using the Rust-owned maximum for complete bounded hydration.
- [x] Added geometry tests for source dimensions, non-square images, letterbox offsets, edge regions, and selection mapping.
- [x] Required CI passed.
- [x] Merged exact green head, verified `master` CI, and reloaded TODO.

## R7 — Queue/status/retry contract consolidation

- [x] Inventoried queue/status/limit definitions across Rust, storage, FFI, Android, tests, and docs.
- [x] Identified live vs dead/compatibility definitions and contradictory attempt limits.
- [x] Selected one Rust owner for durable queue state transitions.
- [x] Documented the authoritative maximum-attempt policy.
- [x] Kept provider request/result interfaces separate from durable queue policy where appropriate.
- [x] Removed or deprecated duplicate status/retry definitions where feasible.
- [x] Aligned FFI conversion, Android mapping, tests, and docs.
- [x] Preserved compatibility with explicit deprecation where immediate removal was unsafe.
- [x] Determined schema compatibility without rewriting migration 0011.
- [x] Tested existing-schema open/upgrade and existing job rows under the old schema.
- [x] Verified automatic attempt count, exhaustion terminal behavior, manual/user retry semantics, restart persistence, and UI diagnostics.
- [x] Runtime/storage/FFI/Android/tests/docs agree on one authoritative durable queue/status/retry contract.
- [x] Required CI passed.
- [x] Merged exact green heads, verified `master` CI, and reloaded TODO.

## R8 — Production integration-test hardening

- [x] Added full production-path scenario: create/open test library, create/register page and scan fixture, persist OCR through supported paths, navigate Library Hub -> OCR Search, search known text, open hit in Page Viewer, verify OCR/regions hydrate, open correction workflow, persist correction, and reopen/verify correction history.
- [x] Added composition-failure sentinels for Search controller wiring, Page Viewer readback wiring, correction wiring/navigation, and OCR Start/Retry/Cancel no-op regression.
- [x] Registered instrumentation classes in CI as needed.
- [x] Kept emulator runtime bounded and avoided duplicating expensive scenarios.
- [x] CI now covers the class of production-wiring defects missed by original closeout.
- [x] Full exact-head CI passed for PR #112 head `3652718f94085d9ee16b33deb30443ddb3c2db90` in CI run `35429371063`.
- [x] Merged exact green head to `master` as `89c7e27264643f53a0f83da0d061b56fe7989feb`.
- [x] Verified post-merge `master` CI run `35430897974`.
- [x] Reloaded TODO.

## R9 — Stale text and targeted maintainability cleanup

- [x] Reconciled stale M11 status text that treated the original closeout as sufficient while remediation remained open.
- [x] Corrected Page Viewer/OCR status through production wiring and documentation closeout.
- [x] Preserved truthful limitations: app-lifetime queue execution, no WorkManager/background-service guarantee, original-only M11 search semantics, and broader release/physical OCR evidence remaining outside this remediation.
- [x] Reviewed Page Viewer/OCR wiring via R2/R3/R8 and limited refactoring to changes that lowered remediation risk.
- [x] Did not perform unrelated scanner/image refactors merely to reduce line counts.
- [x] Preserved behavior with focused tests around the changed wiring.
- [x] User/developer status text matches the remediated product state.
- [x] Ownership boundaries remain clear.

## R10 — Final documentation and closeout

- [x] Recorded final transactional API shape in the closeout evidence file.
- [x] Recorded final cancellation commit-point semantics.
- [x] Recorded canonical queue/status owner and retry policy.
- [x] Recorded final overlay coordinate convention.
- [x] Recorded region hydration/truncation policy.
- [x] Recorded correction provenance semantics.
- [x] Preserved historical closeout PR/CI evidence and added final remediation closeout evidence.
- [x] Explained which original completion assumptions were invalidated by post-closeout review and how they were fixed.
- [x] Updated roadmap/status interpretation through this final remediation closeout: M11 implementation and post-closeout production-integration remediation are complete; broader release/physical/search-scale work remains outside this remediation.
- [x] Recorded remediation PR numbers, resulting master SHAs, available exact PR-head evidence, CI run IDs, migration compatibility evidence, and emulator/instrumentation evidence in `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_CLOSEOUT_2026-09-19.md`.
- [x] Performed final source audit against the remediated master history.
- [x] Open final docs/closeout PR.
- [x] Validate exact PR head with all required checks.
- [x] Merge exact green head through the guarded path.
- [x] Reload this TODO from resulting `master`.
- [x] Verify post-merge `master` CI.
- [x] Verify hosted-status publication when applicable.
- [x] Confirm zero unchecked implementation/testing/qualification/documentation/cleanup items remain.

## Final definition of done

- [x] Production OCR Search uses the real Rust/FFI path.
- [x] Production Page Viewer hydrates durable OCR job/result/region state.
- [x] Production Start/Retry/Cancel actions are real and durable.
- [x] Production correction UI is reachable and durable.
- [x] Original OCR remains immutable when corrected.
- [x] Detected OCR result, required regions, search effects, and queue-success finalization are transactionally coherent.
- [x] Cancellation/completion races obey one explicit commit-point rule.
- [x] Queue terminal state cannot contradict accepted searchable OCR state.
- [x] Overlay uses authoritative source dimensions and the exact displayed-image transform.
- [x] Rust rejects out-of-bounds persisted polygons.
- [x] Region hydration does not silently truncate a supposedly complete overlay.
- [x] One authoritative OCR queue/status/retry contract exists.
- [x] Retry semantics agree across Rust/storage/FFI/Android/tests/docs.
- [x] Production composition tests cover the paths missed by the original closeout.
- [x] Stale OCR/placeholder text is reconciled.
- [x] Historical closeout docs contain a truthful remediation addendum.
- [x] Roadmap status is accurate when read with this final remediation closeout.
- [x] Every code-bearing PR was merged only from an exact green head.
- [x] Every required post-merge `master` CI run passed.
- [x] This TODO contains zero unchecked implementation, testing, qualification, documentation, or cleanup items.

## Stopping condition

The Ralph Loop may stop only after this final documentation closeout PR itself is exact-head green, merged to `master`, the resulting post-merge `master` CI is green, and this TODO has been reloaded from `master` with zero unchecked implementation, testing, qualification, documentation, or cleanup items.
