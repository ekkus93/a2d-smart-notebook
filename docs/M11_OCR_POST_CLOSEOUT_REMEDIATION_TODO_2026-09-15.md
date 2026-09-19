# M11 OCR Post-Closeout Remediation TODO — 2026-09-15

Active checklist for fixing all findings from the post-closeout Milestone 11 OCR review.

Companion specification:

- `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_SPEC_2026-09-15.md`

Review baseline:

- `master`: `da8a6528ce1a72c1612456bf643618b48b0bb2fc`

## Operating rules

- [ ] Read the companion specification before code changes.
- [ ] Use Ralph Bridge for GitHub and CI operations; do not use the default GitHub tool.
- [ ] Start each slice from current `master`.
- [ ] After every successful merge, reload this TODO from current `master` and continue with the next unchecked item.
- [ ] Keep Rust authoritative for durable OCR state, IDs, invariants, provenance, queue policy, transaction boundaries, and search indexing.
- [ ] Keep Android responsible for provider execution, lifecycle/orchestration, navigation, and presentation.
- [ ] Keep core OCR local-first/accountless.
- [ ] Do not hand-edit generated UniFFI Kotlin bindings.
- [ ] Do not mark a user-visible feature complete solely from isolated controller/composable/unit tests; exercise the production composition path.
- [ ] If instrumentation classes change, update `.github/workflows/ci.yml` in the same PR.
- [ ] Merge only exact green PR heads and verify post-merge `master` CI.
- [ ] CI/test/lint/format failures are implementation work, not user blockers.

## Defects to close

- [ ] Production OCR Search lacks a real search controller/gateway.
- [ ] Production Page Viewer does not hydrate persisted OCR via real readback.
- [ ] Production Page Viewer Start/Retry/Cancel OCR callbacks are effectively no-ops/defaults.
- [ ] Correction controller/gateway lack a complete reachable production UI path.
- [ ] OCR run and OCR region persistence are split commits.
- [ ] Queue finalization can succeed from a recorded run even when later region persistence failed.
- [ ] Cancellation can race with detected-result persistence and contradict searchable OCR state.
- [ ] Overlay derives geometry from polygon extrema rather than authoritative source image dimensions.
- [ ] Overlay is not guaranteed to share the displayed image transform.
- [ ] Rust does not fully enforce polygon bounds against source-image dimensions.
- [ ] Android region readback can silently truncate at the previous default limit.
- [ ] Queue/status/retry policy is duplicated across Rust layers with conflicting limits.
- [ ] CI does not sufficiently test production navigation/dependency composition.
- [ ] M11 closeout/roadmap/status text overstates production integration.

---

# R0 — Baseline and status reconciliation

## R0.1 Reconfirm baseline

- [ ] Resolve current `master` SHA through Ralph Bridge.
- [ ] Verify current `master` CI.
- [ ] Reconfirm reviewed wiring/consistency defects still exist.
- [ ] If `master` advanced from the review baseline, reconcile relevant changes before modifying code.

## R0.2 Mark remediation open honestly

- [ ] Add a post-closeout review/remediation note to `docs/M11_OCR_CLOSEOUT_TODO_2026-09-14.md`.
- [ ] Add an addendum to `docs/M11_OCR_CLOSEOUT_2026-09-15.md`.
- [ ] Update `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` so M11 is not represented as fully closed while remediation remains open.
- [ ] Preserve old PR/merge/CI evidence as historical fact.
- [ ] State explicitly that prior CI passed but did not exercise the missing production integration seams.

## R0 qualification

- [ ] Exact-head CI passes as required.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO from `master`.

---

# R1 — Production OCR dependency composition and Search

## R1.1 Define active-library OCR dependency ownership

- [ ] Identify the authoritative production owner of the open-library `A2dClient`.
- [ ] Define one lifecycle-safe production composition path for OCR dependencies.
- [ ] Avoid opening unrelated native clients from recomposing composables.
- [ ] Dispose/recreate dependencies correctly when the active library closes/changes.

## R1.2 Wire production OCR Search

- [ ] Construct `FfiAndroidOcrSearchGateway` from the production client.
- [ ] Construct/inject `AndroidOcrSearchController` into the real OCR Search route.
- [ ] Remove normal reliance on the null/not-connected controller path.
- [ ] Preserve explicit unavailable-library/client error behavior.
- [ ] Run potentially blocking FFI search off the main thread.
- [ ] Preserve Rust query validation/error propagation.
- [ ] Preserve full-text vs region hit-source metadata.
- [ ] Verify hit selection routes to Page Viewer.

## R1.3 Production-path search tests

- [ ] Create/open a real test library.
- [ ] Persist real OCR searchable text through Rust/FFI.
- [ ] Navigate Library Hub -> OCR Search through the real nav graph.
- [ ] Submit a known query through the production route.
- [ ] Verify the real persisted hit and expected page/scan/run/region/source metadata.
- [ ] Select the hit and verify Page Viewer navigation.
- [ ] Add a real unavailable/closed-library error-path test.
- [ ] Register new instrumentation classes in CI if needed.

## R1 acceptance/qualification

- [ ] Production OCR Search returns real persisted OCR.
- [ ] Test fails if production controller wiring is removed.
- [ ] Required Rust/Android/FFI/CI gates pass on exact head.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R2 — Production Page Viewer OCR hydration and actions

## R2.1 Add real Page Viewer OCR state ownership

- [ ] Add/extend a controller/ViewModel/state holder consistent with the current Android architecture.
- [ ] Hydrate page and preferred/current scan from Rust-owned state.
- [ ] Hydrate display asset/image metadata.
- [ ] Hydrate durable OCR job state.
- [ ] Hydrate latest terminal OCR result through real FFI readback.
- [ ] Hydrate OCR text regions through real FFI readback.
- [ ] Distinguish never-requested/queued/running/detected/no-text/unavailable/cancellation-requested/cancelled states as supported.
- [ ] Do not fabricate canonical OCR state in Kotlin.

## R2.2 Wire Start OCR

- [ ] Replace production no-op Start callback with durable queue enqueue behavior.
- [ ] Validate page/scan/input through Rust APIs.
- [ ] Respect duplicate-active-job policy.
- [ ] Refresh viewer state after enqueue.

## R2.3 Wire Retry OCR

- [ ] Replace production no-op Retry callback with the authoritative retry API.
- [ ] Expose Retry only for eligible durable states.
- [ ] Respect canonical attempt limits.
- [ ] Refresh state after retry scheduling.

## R2.4 Wire Cancel OCR

- [ ] Replace production no-op Cancel callback with durable cancellation request behavior.
- [ ] Do not claim terminal cancellation before durable/provider resolution.
- [ ] Show cancellation-requested separately when the model supports it.
- [ ] Refresh after cancellation resolution.

## R2.5 Production-path Page Viewer tests

- [ ] Verify persisted `Detected` hydration.
- [ ] Verify `NoTextDetected` hydration.
- [ ] Verify `Unavailable` hydration.
- [ ] Verify queued/running/cancelled states.
- [ ] Verify regions hydrate.
- [ ] Verify Start enqueues a durable job.
- [ ] Verify Retry changes durable state only when eligible.
- [ ] Verify Cancel requests durable cancellation.
- [ ] Verify route leave/reopen does not lose durable state.

## R2 acceptance/qualification

- [ ] Production Page Viewer no longer depends on default empty OCR state/no-op controls.
- [ ] Required CI passes on exact PR head.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R3 — Reachable OCR correction workflow

## R3.1 Correct provenance semantics

- [ ] Audit `previous_text` across Rust/core/FFI/Android.
- [ ] Define whether it means immutable original OCR or prior correction value.
- [ ] Derive canonical original/prior text in Rust where possible rather than trusting arbitrary caller text.
- [ ] If correction-to-correction lineage exists, reference prior correction by durable ID.
- [ ] Add invariant tests for invalid targets/provenance.

## R3.2 Add production correction presentation/navigation

- [ ] Add a correction destination or Page Viewer correction panel/dialog.
- [ ] Make it reachable from a selected OCR region and/or supported full-text target.
- [ ] Pass durable IDs as identity.
- [ ] Preserve back-navigation state.

## R3.3 Wire real correction gateway/controller

- [ ] Construct `FfiAndroidOcrCorrectionGateway` from the active production client.
- [ ] Use `AndroidOcrCorrectionController` in the real path.
- [ ] Hydrate correction history from Rust.
- [ ] Submit correction through Rust/FFI.
- [ ] Refresh history and Page Viewer state after success.
- [ ] Display immutable original OCR distinctly from corrected text.
- [ ] Preserve current original-only search semantics unless separately specified.

## R3.4 Production-path correction tests

- [ ] Navigate Page Viewer -> correction entry through production UI.
- [ ] Submit valid correction.
- [ ] Verify durable readback/history.
- [ ] Verify original OCR is unchanged.
- [ ] Verify persistence across route reopen/state recreation supported by the harness.
- [ ] Verify invalid target/provenance rejection.
- [ ] Verify empty/oversized correction rejection according to Rust policy.

## R3 acceptance/qualification

- [ ] Correction is reachable without test-only dependency injection.
- [ ] Original OCR remains immutable.
- [ ] Required CI passes on exact head.
- [ ] Merge, verify post-merge `master` CI, reload TODO.

---

# R4 — Transactional OCR finalization

## R4.1 Define atomic finalization API

- [ ] Define a Rust-owned finalization command for one claimed OCR job attempt.
- [ ] Include a stale-worker/concurrency guard such as attempt/claim generation/status predicate.
- [ ] Represent `Detected`, `NoTextDetected`, and `Unavailable` explicitly.
- [ ] Include detected full text and required regions in the same command.
- [ ] Include required provider/model/diagnostic provenance.
- [ ] Return durable job status, committed run ID when applicable, retry state, and cancellation/completion resolution.

## R4.2 Implement one Rust transaction

- [ ] Validate claimed job/input identity in Rust.
- [ ] For `Detected`, write OCR run and required regions in one transaction.
- [ ] Ensure FTS trigger/index effects are inside the same transaction.
- [ ] Finalize queue success in that transaction.
- [ ] For `NoTextDetected`, coherently commit terminal run + queue completion with no fake text.
- [ ] For `Unavailable`, coherently commit unavailable/retry/terminal decision.
- [ ] Roll back all terminal-success writes when any required detected-result write fails.

## R4.3 Replace split Android persistence path

- [ ] Remove the normal path that records OCR run and regions in separate commits.
- [ ] Ensure Android does not infer recognized success merely because an OCR run ID exists.
- [ ] Map finalization result into presentation/executor state truthfully.
- [ ] Retire/deprecate obsolete FFI methods if safe; otherwise document narrowed compatibility use.

## R4.4 Failure-injection tests

- [ ] Force region persistence failure after provider success.
- [ ] Verify OCR run rolls back.
- [ ] Verify no FTS hit survives.
- [ ] Verify queue is not marked successful.
- [ ] Verify coherent retry/error state.
- [ ] Verify process/restart readback observes only coherent durable state.

## R4 acceptance/qualification

- [ ] No partial detected-result success is durable.
- [ ] Required Rust/FFI/Android tests and CI pass.
- [ ] Merge exact green head, verify post-merge `master` CI, reload TODO.

---

# R5 — Cancellation/commit race semantics

## R5.1 Implement commit-point policy

- [ ] Before terminal commit, durable cancellation wins.
- [ ] After successful terminal commit, completion wins.
- [ ] Late cancellation must not rewrite a successfully committed job to `Cancelled`.
- [ ] A cancelled-before-commit detected result must not become accepted/searchable durable OCR.
- [ ] Reject stale worker finalization where practical.

## R5.2 Race tests

- [ ] Cancellation before provider execution.
- [ ] Cancellation during provider execution.
- [ ] Cancellation after provider result but before transaction commit.
- [ ] Cancellation racing with commit.
- [ ] Cancellation after successful commit.
- [ ] Verify queue/run/regions/search-index consistency in every case.

## R5 acceptance/qualification

- [ ] Queue terminal state cannot contradict accepted searchable OCR state.
- [ ] Required CI passes.
- [ ] Merge exact green head, verify `master` CI, reload TODO.

---

# R6 — Correct OCR region geometry and durable validation

## R6.1 Expose authoritative source geometry

- [ ] Identify Rust-owned source image dimensions for the OCR input.
- [ ] Expose width/height plus source asset/scan identity through existing or new readback APIs.
- [ ] Validate dimensions are positive/valid.
- [ ] Avoid using polygon maxima as image dimensions.

## R6.2 Rust polygon bounds

- [ ] Document inclusive/exclusive pixel-bound convention.
- [ ] Reject NaN/infinite/negative coordinates.
- [ ] Reject coordinates outside source dimensions.
- [ ] Reject malformed polygons/invalid dimensions.
- [ ] Reject region/run/input geometry mismatches.
- [ ] Keep Android clamping only as defensive provider handling, not the invariant.

## R6.3 Fix overlay transform

- [ ] Render the actual page/scan image.
- [ ] Compute one content-fit transform for that image.
- [ ] Apply identical scale/offset to each persisted polygon.
- [ ] Preserve aspect ratio/letterboxing offsets.
- [ ] Remove polygon-extrema coordinate-frame inference.
- [ ] Remove aspect-ratio clamping that distorts geometry.
- [ ] Keep hit testing in the same coordinate system.
- [ ] Prevent selection outside rendered image bounds.

## R6.4 Eliminate silent region truncation

- [ ] Audit the previous default 50-region readback limit.
- [ ] Select complete bounded hydration, pagination, or explicit truncation metadata/UI.
- [ ] Prefer complete hydration up to a Rust-owned maximum consistent with provider/storage limits.
- [ ] Add test with more than 50 regions or explicit partial-state behavior.

## R6.5 Geometry tests

- [ ] Test fixed source dimensions and known polygons.
- [ ] Test non-square images.
- [ ] Test letterbox offsets.
- [ ] Test regions near all four edges.
- [ ] Test selection mapping.
- [ ] Test rotation/orientation assumptions if relevant to OCR input.

## R6 acceptance/qualification

- [ ] Overlay aligns with the real displayed image.
- [ ] Rust owns durable bounds validation.
- [ ] No silent partial overlay is presented as complete.
- [ ] Required CI passes.
- [ ] Merge exact green head, verify `master` CI, reload TODO.

---

# R7 — Queue/status/retry contract consolidation

## R7.1 Inventory duplicates

- [ ] Find all queue/status/limit definitions in `a2d-ocr`, `a2d-core`, storage, FFI, Android, tests, and docs.
- [ ] Identify live vs dead/compatibility definitions.
- [ ] Record contradictory attempt limits and semantics.

## R7.2 Select canonical owner/policy

- [ ] Choose one Rust owner for durable queue state transitions.
- [ ] Choose/document the authoritative maximum-attempt policy.
- [ ] Keep provider request/result interfaces separate from durable queue policy where appropriate.

## R7.3 Remove or deprecate duplication

- [ ] Replace duplicate status models with canonical types where feasible.
- [ ] Remove unused/dead retry-limit definitions.
- [ ] Align FFI conversion, Android mapping, tests, and docs.
- [ ] Preserve public compatibility with explicit deprecation if immediate removal is unsafe.

## R7.4 Database compatibility

- [ ] Determine whether schema constraints need a forward migration.
- [ ] Do not rewrite migration 0011 in place.
- [ ] Test existing-schema open/upgrade.
- [ ] Test existing job rows valid under the old schema.

## R7.5 Retry tests

- [ ] Verify exact automatic attempt count.
- [ ] Verify exhaustion terminal behavior.
- [ ] Verify manual/user retry semantics if distinct.
- [ ] Verify attempt count survives restart.
- [ ] Verify UI diagnostics reflect canonical policy.

## R7 acceptance/qualification

- [ ] One authoritative durable queue/status/retry contract remains.
- [ ] Runtime/storage/FFI/Android/tests/docs agree.
- [ ] Required CI passes.
- [ ] Merge exact green head, verify `master` CI, reload TODO.

---

# R8 — Production integration-test hardening

## R8.1 Full production-path scenario

- [x] Open/create a test library.
- [x] Create/register a page and scan fixture through supported APIs.
- [x] Produce/persist OCR through the real supported path.
- [x] Navigate Library Hub -> OCR Search.
- [x] Search known text.
- [x] Open hit in Page Viewer.
- [x] Verify persisted OCR and regions hydrate.
- [x] Open correction workflow.
- [x] Persist correction.
- [x] Reopen and verify correction history.

## R8.2 Composition-failure sentinels

- [x] Search integration test fails if production controller wiring is removed.
- [x] Page Viewer test fails if OCR readback wiring is removed.
- [x] Correction test fails if production correction wiring/navigation is removed.
- [x] OCR action test fails if Start/Retry/Cancel revert to no-ops.

## R8.3 CI registration/runtime

- [x] Register new instrumentation classes in `.github/workflows/ci.yml` if explicit class lists are used.
- [x] Keep emulator runtime bounded.
- [x] Avoid duplicating expensive scenarios when one integrated scenario proves multiple composition seams.

## R8 acceptance/qualification

- [x] CI now covers the same class of production-wiring defects missed by the original closeout.
- [x] Full exact-head CI passes.
- [x] Merge exact green head, verify `master` CI, reload TODO.

---

# R9 — Stale text and targeted maintainability cleanup

## R9.1 Reconcile stale UI/developer text

- [x] Search resources/source for statements that OCR is unavailable/future solely because M11 has not happened.
- [x] Correct stale Page Viewer/OCR placeholder wording.
- [x] Reconcile stale application placeholder comments when no longer true.
- [x] Preserve truthful limitations such as app-lifetime queue execution.

## R9.2 Targeted refactor only where it lowers remediation risk

- [x] Review whether Page Viewer/OCR wiring leaves oversized Android orchestration files harder to maintain.
- [x] Extract cohesive OCR-specific state/orchestration where useful.
  - Review result: no additional extraction in R9; the current production composition keeps the app-lifetime client in `MainActivity`, OCR gateways/controllers are remembered from that client in navigation, and a structural move here would add churn without reducing remediation risk.
- [x] Do not perform unrelated scanner/image refactors merely to reduce line counts.
- [x] Preserve behavior with tests around extraction (no extraction performed; existing production-path instrumentation remains the regression sentinel).

## R9 acceptance/qualification

- [x] User/developer status text matches the remediated product.
- [x] Ownership boundaries remain clear.
- [ ] Required CI passes; merge/verify/reload if separate PR.

---

# R10 — Final documentation and closeout

## R10.1 Record final architecture decisions

- [ ] Update the companion spec with final transactional API shape.
- [ ] Record final cancellation commit-point semantics.
- [ ] Record canonical queue/status owner and retry limit.
- [ ] Record final overlay coordinate convention.
- [ ] Record region hydration/truncation policy.
- [ ] Record correction provenance semantics.

## R10.2 Reconcile historical closeout documents

- [ ] Add remediation completion reference to `docs/M11_OCR_CLOSEOUT_TODO_2026-09-14.md`.
- [ ] Add remediation completion addendum to `docs/M11_OCR_CLOSEOUT_2026-09-15.md`.
- [ ] Preserve historical PR/CI evidence.
- [ ] Explain which original completion assumptions were invalidated by post-closeout review and how they were fixed.

## R10.3 Update roadmap/status

- [ ] Update `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`.
- [ ] Ensure M11 production integration status is accurate.
- [ ] Keep broader search scale/release work in Milestone 12 as appropriate.
- [ ] Keep background-service/WorkManager behavior out of scope unless separately implemented.

## R10.4 Final evidence

For every remediation PR, record:

- [ ] PR number.
- [ ] Exact qualified PR-head SHA.
- [ ] Resulting `master` SHA.
- [ ] Required CI run IDs.
- [ ] Post-merge `master` CI run ID.
- [ ] Milestone 7 Native Validation when applicable.
- [ ] Migration compatibility evidence when applicable.
- [ ] Emulator/instrumentation evidence.

## R10.5 Final source audit

- [ ] No production OCR Search route omits the real controller.
- [ ] No production Page Viewer route relies on empty/default OCR state or no-op Start/Retry/Cancel callbacks.
- [ ] Correction gateway/controller are used by production code, not tests only.
- [ ] Old split OCR-run-then-region terminal-success path is gone from normal execution.
- [ ] Queue success does not depend merely on existence of an OCR run ID.
- [ ] Overlay does not derive source dimensions from polygon maxima.
- [ ] Previous silent 50-region behavior is gone or explicitly disclosed as partial state.
- [ ] Conflicting retry/status contracts/constants are gone or explicitly compatibility-deprecated.
- [ ] Documentation contains no unqualified stale claim that original M11 closeout remains fully sufficient.

## R10.6 Final CI/merge

- [ ] Open final docs/closeout PR if needed.
- [ ] Validate exact PR head with all required checks.
- [ ] Merge exact green head through the guarded path.
- [ ] Reload this TODO from resulting `master`.
- [ ] Verify post-merge `master` CI.
- [ ] Verify hosted-status publication when applicable.
- [ ] Confirm zero unchecked implementation/testing/qualification/documentation/cleanup items remain.

---

# Final definition of done

- [ ] Production OCR Search uses the real Rust/FFI path.
- [ ] Production Page Viewer hydrates durable OCR job/result/region state.
- [ ] Production Start/Retry/Cancel actions are real and durable.
- [ ] Production correction UI is reachable and durable.
- [ ] Original OCR remains immutable when corrected.
- [ ] Detected OCR result, required regions, search effects, and queue-success finalization are transactionally coherent.
- [ ] Cancellation/completion races obey one explicit commit-point rule.
- [ ] Queue terminal state cannot contradict accepted searchable OCR state.
- [ ] Overlay uses authoritative source dimensions and the exact displayed-image transform.
- [ ] Rust rejects out-of-bounds persisted polygons.
- [ ] Region hydration does not silently truncate a supposedly complete overlay.
- [ ] One authoritative OCR queue/status/retry contract exists.
- [ ] Retry semantics agree across Rust/storage/FFI/Android/tests/docs.
- [ ] Production composition tests cover the paths missed by the original closeout.
- [ ] Stale OCR/placeholder text is reconciled.
- [ ] Historical closeout docs contain a truthful remediation addendum.
- [ ] Roadmap status is accurate.
- [ ] Every code-bearing PR was merged only from an exact green head.
- [ ] Every required post-merge `master` CI run passed.
- [ ] This TODO contains zero unchecked implementation, testing, qualification, documentation, or cleanup items.

## Stopping condition

The Ralph Loop may stop only when either:

1. Every task/subtask above is implemented, qualified, reconciled in this TODO, merged to `master`, and final post-merge `master` CI is green; or
2. A concrete blocker specifically requires user-supplied information, authorization, credentials, hardware, or a product/architecture decision not already resolved by the companion specification.

CI still running, a bounded CI wait timing out, ordinary implementation/test/lint/format failures, merge conflicts, or transient tool/API errors are not user blockers.
