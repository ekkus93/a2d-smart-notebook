# M11 OCR Post-Closeout Review 2 Remediation TODO — 2026-09-19

Active checklist for fixing all findings from the second post-closeout source review.

Companion specification:

- `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_SPEC_2026-09-19.md`

Reviewed baseline:

- `master`: `ba6f87d7eab75730ce7c715f87ac1ae615d1c730`
- prior tracker: `docs/M11_OCR_POST_CLOSEOUT_REMEDIATION_TODO_2026-09-15.md`

This tracker reopens M11 remediation because the prior TODO reached zero unchecked boxes while several production-path invariants were still not enforced end-to-end.

## Operating rules

- [ ] Read the companion specification before code changes.
- [ ] Use Ralph Bridge for GitHub and CI operations; do not use the default GitHub tool.
- [ ] Start each slice from current `master`.
- [ ] After every successful merge, reload this TODO from current `master` and continue with the next unchecked item.
- [ ] Keep Rust authoritative for durable OCR state, input identity, source geometry, retry policy, cancellation, finalization, provenance, queue transitions, polygon invariants, and search indexing.
- [ ] Keep Android responsible for provider execution, lifecycle/orchestration, navigation, image presentation, display transforms, and user interaction.
- [ ] Keep core OCR local-first/accountless.
- [ ] Do not hand-edit generated UniFFI Kotlin bindings.
- [ ] Do not mark a feature complete because the correct Rust primitive exists; prove the shipped production Android path invokes it.
- [ ] Do not mark a production seam complete solely from fake-controller/composable/unit tests.
- [ ] If instrumentation classes change, update `.github/workflows/ci.yml` in the same PR.
- [ ] Merge only exact green PR heads.
- [ ] Verify post-merge `master` CI before using the result as the next base.
- [ ] CI/test/lint/format failures, merge conflicts, and transient API failures are implementation work, not user blockers.

## Findings to close

- [ ] Production Android still uses split OCR run/region/queue-completion persistence.
- [ ] Region persistence failure can still lead to successful queue completion because a recorded run exists.
- [ ] Production cancellation can contradict accepted/searchable OCR because the transactional commit point is not used end-to-end.
- [ ] Scanner `OcrOptimized` jobs can be missed by Page Viewer `Original` active-job lookup.
- [ ] Page Viewer Retry creates a fresh ordinary job instead of using a Rust-owned manual retry transition.
- [ ] Page Viewer uses hard-coded OCR source dimensions.
- [ ] Rust does not fully reject polygon coordinates beyond authoritative image bounds.
- [ ] Production OCR overlay lacks authoritative source geometry and the actual image/shared transform.
- [ ] Region hydration can still be silently partial because write and readback limits differ.
- [ ] Production integration tests do not prove queue/provider/transactional-finalizer composition.
- [ ] Some coroutine paths can convert cancellation into ordinary error state.
- [ ] Active-library lifecycle/disposal claims exceed current implementation.
- [ ] Stale OCR/Page Viewer text and final closeout/roadmap claims overstate completion.

---

# R11 — Wire transactional OCR finalization into production

## R11.1 Expose finalizer through UniFFI

- [ ] Audit the existing Rust `finalize_ocr_job` API and retain Rust as the sole transaction owner.
- [ ] Add supported UniFFI request/response types for transactional finalization.
- [ ] Include job ID.
- [ ] Include claimed attempt number or equivalent stale-worker guard.
- [ ] Include provider/model/version diagnostics.
- [ ] Represent `Detected`, `NoTextDetected`, and `Unavailable` explicitly.
- [ ] Include detected full text and every required text region in the same request.
- [ ] Return durable final job status.
- [ ] Return committed OCR run ID when applicable.
- [ ] Return retry/exhaustion state.
- [ ] Return whether cancellation or completion won.
- [ ] Preserve structured Rust validation/storage errors over FFI.
- [ ] Regenerate Kotlin bindings through the supported build process only.

## R11.2 Replace normal split Android persistence

- [ ] Identify every production call to record OCR run separately.
- [ ] Identify every production call to record OCR text regions separately.
- [ ] Identify every production call to complete an OCR queue job separately.
- [ ] Change `AndroidOcrWorkflow`/queue execution so one provider outcome becomes one finalization request.
- [ ] Change `AndroidOcrQueueProcessor` so success does not depend merely on `recordedRun != null`.
- [ ] Ensure a failed region/result persistence path cannot fall through to queue completion.
- [ ] Remove or narrow `recordedRun` semantics that permit failed workflow results to look complete.
- [ ] Keep legacy split FFI methods only when explicitly needed for compatibility/tests/migrations.
- [ ] Document any retained legacy method as forbidden for normal queue execution.

## R11.3 Transaction/failure behavior

- [ ] Verify Detected run + required regions + FTS effects + queue success commit in one Rust transaction.
- [ ] Verify NoTextDetected terminal run + queue completion are coherent.
- [ ] Verify Unavailable retry/terminal decision is coherent.
- [ ] Force region persistence failure after provider success.
- [ ] Verify OCR run rolls back.
- [ ] Verify all required regions roll back.
- [ ] Verify no FTS/search hit survives.
- [ ] Verify queue is not marked successful.
- [ ] Verify retry/error state is coherent.
- [ ] Verify process/restart readback observes only coherent durable state.

## R11.4 Production-path sentinel tests

- [ ] Exercise the normal Android queue executor/gateway path rather than directly calling the Rust finalizer from the test.
- [ ] Test successful Detected finalization end-to-end.
- [ ] Test region failure end-to-end.
- [ ] Test NoTextDetected end-to-end.
- [ ] Test Unavailable end-to-end.
- [ ] Add a sentinel that fails if production Android reverts to split record-run/record-regions/complete calls.
- [ ] Register any new instrumentation class in CI.

## R11 acceptance/qualification

- [ ] Normal production OCR terminal persistence uses exactly one Rust transactional finalizer.
- [ ] No partial detected-result success is durable.
- [ ] Queue success cannot be inferred from existence of a recorded OCR run.
- [ ] Required Rust/FFI/Android tests pass.
- [ ] Exact-head CI passes.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload this TODO from `master`.

---

# R12 — Enforce cancellation/commit race semantics in production

## R12.1 Production commit-point contract

- [ ] Before terminal commit, durable cancellation wins.
- [ ] After successful terminal commit, completion wins.
- [ ] Late cancellation cannot rewrite accepted completion to `Cancelled`.
- [ ] Cancelled-before-commit detected output cannot become durable/searchable OCR.
- [ ] A stale worker/attempt cannot finalize a superseded claim.
- [ ] Android does not invent cancellation/completion resolution.

## R12.2 Controlled race tests through production executor

- [ ] Cancellation before provider execution.
- [ ] Cancellation during provider execution.
- [ ] Cancellation after provider result but before finalizer invocation.
- [ ] Cancellation racing with finalizer commit.
- [ ] Cancellation after successful terminal commit.
- [ ] Stale claimed attempt finalization rejection.
- [ ] For every race, assert queue status.
- [ ] For every race, assert OCR run presence/absence.
- [ ] For every race, assert text-region presence/absence.
- [ ] For every race, assert search-index visibility/non-visibility.
- [ ] Verify restart/readback preserves the same durable resolution.

## R12 acceptance/qualification

- [ ] Queue terminal state cannot contradict accepted searchable OCR state.
- [ ] Production executor obeys the same semantics as Rust finalizer unit tests.
- [ ] Required CI passes on exact head.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R13 — Fix Page Viewer OCR job identity and manual retry

## R13.1 Remove input-kind mismatch

- [ ] Document the scanner's normal OCR input selection.
- [ ] Document Page Viewer's current active-job lookup behavior.
- [ ] Select one Rust-owned active-job-by-scan contract.
- [ ] Prefer returning the actual durable active job/input identity rather than hard-coding `Original`.
- [ ] Ensure Page Viewer recognizes a scanner-created `OcrOptimized` queued job.
- [ ] Ensure Page Viewer recognizes a scanner-created `OcrOptimized` running job.
- [ ] Prevent Page Viewer Start from creating a second active job for the same scan merely because input kind differs.
- [ ] Preserve explicit behavior if parallel OCR inputs are intentionally allowed; otherwise enforce one active OCR job per scan.
- [ ] Update UI state mapping to use the durable job's actual input identity.

## R13.2 Rust-owned manual retry

- [ ] Add an explicit Rust manual-retry API/command.
- [ ] Define retryable terminal states.
- [ ] Define ineligible states.
- [ ] Define how manual retry interacts with automatic attempt count.
- [ ] Define a bounded manual retry policy.
- [ ] Preserve prior attempt/error/provider diagnostics.
- [ ] Ensure retry survives restart.
- [ ] Ensure Android Retry calls this API rather than ordinary enqueue.
- [ ] Ensure Retry cannot reset policy by creating an unrestricted fresh job.
- [ ] Refresh Page Viewer from durable state after retry.

## R13.3 Tests

- [ ] Seed OCR through the normal scanner `OcrOptimized` path and open Page Viewer.
- [ ] Verify Page Viewer shows queued/running state for that job.
- [ ] Verify Start does not create a duplicate active job.
- [ ] Verify eligible manual Retry transitions durable state correctly.
- [ ] Verify ineligible Retry is rejected.
- [ ] Verify retry bound/exhaustion.
- [ ] Verify attempt/retry diagnostics survive recreation/restart.
- [ ] Add composition sentinel that fails if Page Viewer returns to hard-coded `Original`.
- [ ] Add composition sentinel that fails if Retry returns to ordinary enqueue.

## R13 acceptance/qualification

- [ ] One coherent active OCR job identity is used across scanner and Page Viewer.
- [ ] Manual Retry is Rust-owned and bounded.
- [ ] Required CI passes.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R14 — Authoritative source geometry and real Page Viewer overlay

## R14.1 Expose actual OCR source geometry

- [ ] Identify the exact source asset used for OCR for each durable job/run.
- [ ] Expose scan ID.
- [ ] Expose source asset ID.
- [ ] Expose input/asset kind.
- [ ] Expose validated source width in pixels.
- [ ] Expose validated source height in pixels.
- [ ] Expose enough validated asset-location/access information for Android to display the source image.
- [ ] Validate source dimensions are positive and within Rust-owned limits.
- [ ] Remove production dependence on hard-coded `1800x2200` dimensions.
- [ ] Remove any other fabricated Page Viewer OCR dimensions.

## R14.2 Rust polygon bounds

- [ ] Document the exact source-pixel boundary convention.
- [ ] Reject malformed polygons.
- [ ] Reject NaN coordinates.
- [ ] Reject infinite coordinates.
- [ ] Reject negative coordinates.
- [ ] Reject `x` beyond source width.
- [ ] Reject `y` beyond source height.
- [ ] Reject invalid source dimensions.
- [ ] Reject region/run/input/source-asset geometry mismatches.
- [ ] Keep Android provider clamping defensive only.
- [ ] Add invariant tests for all four source edges and out-of-bounds cases.

## R14.3 Render actual image and shared transform

- [ ] Page Viewer loads/renders the actual OCR source image.
- [ ] Compute one content-fit transform from source pixels to displayed image rectangle.
- [ ] Preserve source aspect ratio.
- [ ] Preserve letterbox/pillarbox offsets.
- [ ] Apply the identical scale/translation to all persisted polygons.
- [ ] Use the identical transform for hit testing.
- [ ] Ignore/reject selection outside the rendered image bounds.
- [ ] Do not infer source dimensions from polygon extrema.
- [ ] Do not distort geometry through aspect-ratio clamping.
- [ ] Keep overlay disabled with an explicit reason when source geometry/image is unavailable.

## R14.4 Geometry tests

- [ ] Square source image.
- [ ] Non-square portrait image.
- [ ] Non-square landscape image.
- [ ] Letterboxed horizontal case.
- [ ] Letterboxed vertical case.
- [ ] Polygon touching left edge.
- [ ] Polygon touching right edge.
- [ ] Polygon touching top edge.
- [ ] Polygon touching bottom edge.
- [ ] Selection/hit testing inside polygon.
- [ ] Selection outside rendered image.
- [ ] Orientation/rotation assumptions documented and tested where relevant.
- [ ] Production UI test fails if source geometry is disconnected.

## R14 acceptance/qualification

- [ ] Page Viewer image and OCR polygons share one real transform.
- [ ] Rust owns durable polygon/image bounds.
- [ ] No hard-coded production OCR dimensions remain.
- [ ] Required CI passes.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R15 — Complete and explicit OCR region hydration

## R15.1 Reconcile limits

- [ ] Inventory region write maximum.
- [ ] Inventory all readback/page-size/default limits.
- [ ] Define one Rust-owned maximum supported region count.
- [ ] Define bounded pagination for readback.
- [ ] Return total count when known.
- [ ] Return returned count.
- [ ] Return cursor/offset/page token.
- [ ] Return has-more/complete metadata.
- [ ] Reject writes beyond the authoritative maximum explicitly.

## R15.2 Page Viewer hydration

- [ ] Fetch all required region pages before claiming overlay hydration complete.
- [ ] Represent partial/loading state explicitly while additional pages remain.
- [ ] Do not present partial overlay as complete.
- [ ] Preserve stable region IDs across pagination.
- [ ] Prevent duplicate regions when pages/recreation overlap.
- [ ] Preserve selection where possible as hydration completes.
- [ ] Surface a real error if pagination fails rather than silently stopping.

## R15.3 Tests

- [ ] More than 50 regions.
- [ ] More than the previous 1,000-row readback ceiling.
- [ ] Exact authoritative maximum.
- [ ] Explicit rejection above maximum.
- [ ] Multi-page hydration completeness.
- [ ] Recreation during pagination.
- [ ] Failure on a later page produces explicit partial/error state.
- [ ] Search/overlay metadata remains coherent for all hydrated regions.

## R15 acceptance/qualification

- [ ] A supposedly complete overlay is never silently truncated.
- [ ] Read/write limits and UI semantics agree.
- [ ] Required CI passes.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R16 — Strengthen full production integration sentinels

## R16.1 End-to-end production scenario

- [ ] Open/create a real test library.
- [ ] Create/register a page and scan through supported APIs.
- [ ] Enqueue OCR through the normal queue path.
- [ ] Claim OCR through the normal executor path.
- [ ] Execute a deterministic test provider through the production provider boundary.
- [ ] Finalize through the transactional finalizer used by production Android.
- [ ] Verify durable queue status.
- [ ] Verify durable OCR run.
- [ ] Verify durable OCR regions.
- [ ] Verify local search visibility.
- [ ] Navigate Library Hub -> OCR Search.
- [ ] Submit a known query.
- [ ] Open hit in Page Viewer.
- [ ] Verify actual source image/geometry hydration.
- [ ] Verify OCR regions/overlay hydration.
- [ ] Open correction workflow.
- [ ] Persist correction.
- [ ] Reopen and verify immutable original OCR plus correction history.

## R16.2 Production seam sentinels

- [ ] Test fails if production queue processor reverts to split finalization APIs.
- [ ] Test fails if queue success again depends only on recorded run existence.
- [ ] Test fails if Page Viewer reverts to `Original` while scanner uses `OcrOptimized`.
- [ ] Test fails if Retry reverts to ordinary enqueue.
- [ ] Test fails if hard-coded OCR source dimensions return.
- [ ] Test fails if source-image/overlay transform wiring is removed.
- [ ] Test fails if correction production wiring/navigation is removed.

## R16.3 Failure scenarios

- [ ] Region persistence failure after provider success through production executor.
- [ ] Cancellation before transactional commit through production executor.
- [ ] Stale claimed attempt through production executor.
- [ ] Partial region hydration failure through production Page Viewer.
- [ ] Verify failures leave coherent durable/readback/search state.

## R16.4 CI registration/runtime

- [ ] Register new instrumentation classes in `.github/workflows/ci.yml` when required.
- [ ] Share expensive fixture/library setup where practical.
- [ ] Keep emulator runtime bounded.
- [ ] Avoid replacing production composition with feature-specific fakes for the dependency being tested.

## R16 acceptance/qualification

- [ ] CI covers the exact old-API/new-API wiring regression that escaped the prior closeout.
- [ ] Full exact-head CI passes.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R17 — Coroutine cancellation and active-library lifecycle

## R17.1 Cancellation audit

- [ ] Audit OCR Search suspend paths.
- [ ] Audit Page Viewer hydration suspend paths.
- [ ] Audit Start/Retry/Cancel suspend paths.
- [ ] Audit correction suspend paths.
- [ ] Audit queue/executor suspend paths.
- [ ] For broad `catch (Exception)` paths, rethrow `CancellationException` before ordinary error mapping.
- [ ] Ensure cancellation does not update stale UI state after navigation/recreation.
- [ ] Ensure cancellation does not appear as ordinary OCR/search failure unless deliberately specified.

## R17.2 Cancellation tests

- [ ] Cancel Search while FFI work is pending.
- [ ] Cancel Page Viewer hydration during navigation away.
- [ ] Cancel correction submission/hydration where supported.
- [ ] Verify cancellation is rethrown/preserved.
- [ ] Verify no stale error state is written after cancellation.

## R17.3 Active-library client lifecycle

- [ ] Identify one production owner of the active `A2dClient`.
- [ ] Ensure recomposing composables never open unrelated native clients.
- [ ] Define close/dispose behavior.
- [ ] Define active-library switch behavior.
- [ ] Dispose/recreate OCR gateways/controllers when the active library changes.
- [ ] Ensure Search/Page Viewer/correction/queue actions bind to the same active client.
- [ ] Preserve explicit no-library/unavailable behavior.
- [ ] If library switching is intentionally unsupported, document that limitation instead of claiming it is implemented.

## R17 acceptance/qualification

- [ ] Coroutine cancellation remains cancellation across OCR UI orchestration.
- [ ] Active-client lifecycle claims match real product behavior.
- [ ] Required CI passes.
- [ ] Merge exact green head.
- [ ] Verify post-merge `master` CI.
- [ ] Reload TODO.

---

# R18 — Documentation, stale text, final audit, and closeout

## R18.1 Reopen status honestly

- [ ] Update `docs/M11_OCR_CLOSEOUT_TODO_2026-09-14.md` with a second-review remediation note.
- [ ] Update `docs/M11_OCR_CLOSEOUT_2026-09-15.md` with a second-review addendum.
- [ ] Update `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` so M11 is not represented as fully remediated while this TODO remains open.
- [ ] Preserve all prior PR/merge/CI evidence as historical fact.
- [ ] State explicitly that prior CI passed while some production paths still called old compatibility APIs.

## R18.2 Reconcile stale source/resources

- [ ] Search Android resources for claims that Page Viewer/OCR APIs are future/unavailable when implemented.
- [ ] Correct stale Page Viewer placeholder wording.
- [ ] Correct stale OCR/M11 placeholder wording.
- [ ] Audit source comments for outdated dependency/composition claims.
- [ ] Preserve truthful limitations such as app-lifetime queue execution and broader Milestone 12 deferrals.

## R18.3 Record final architecture decisions

- [ ] Record final production transactional-finalization API shape.
- [ ] Record cancellation commit-point semantics.
- [ ] Record active-job/input identity semantics.
- [ ] Record manual retry semantics and bounds.
- [ ] Record source-image geometry convention.
- [ ] Record polygon bound convention.
- [ ] Record image/overlay transform convention.
- [ ] Record region pagination/completeness policy.
- [ ] Record active-library lifecycle limitation/behavior.
- [ ] Preserve correction provenance semantics.

## R18.4 Final source audit

- [ ] No normal production queue path separately records OCR run, regions, then queue completion.
- [ ] No production queue success path depends merely on recorded run ID existence.
- [ ] Production executor calls the transactional finalizer.
- [ ] Production cancellation uses the same commit-point semantics.
- [ ] Page Viewer recognizes scanner-created active OCR jobs.
- [ ] Page Viewer Retry calls Rust-owned manual retry.
- [ ] No hard-coded production OCR source dimensions remain.
- [ ] Rust rejects out-of-bounds polygon coordinates against source dimensions.
- [ ] Page Viewer renders the actual source image under the polygon overlay.
- [ ] Image and polygon hit testing share one content-fit transform.
- [ ] No complete overlay silently truncates region rows.
- [ ] Coroutine cancellation is not converted to ordinary error by broad catches.
- [ ] Active-library lifecycle documentation matches implementation.
- [ ] Documentation contains no unqualified claim that the prior remediation remains fully sufficient.

## R18.5 Final evidence

For every second-review remediation PR record:

- [ ] PR number.
- [ ] Exact qualified PR-head SHA.
- [ ] Resulting `master` SHA.
- [ ] Exact CI run ID(s).
- [ ] Post-merge `master` CI run ID.
- [ ] Emulator/instrumentation evidence.
- [ ] Migration/database-compatibility evidence when applicable.
- [ ] Hosted-status publication when applicable.

## R18.6 Final CI/merge

- [ ] Open final docs/closeout PR if needed.
- [ ] Validate exact PR head with all required checks.
- [ ] Merge exact green head through the guarded path.
- [ ] Reload this TODO from resulting `master`.
- [ ] Verify post-merge `master` CI.
- [ ] Verify hosted-status publication when applicable.
- [ ] Confirm zero unchecked implementation/testing/qualification/documentation/cleanup items remain.

---

# Final definition of done

- [ ] Production Android uses the Rust transactional finalizer.
- [ ] Detected OCR run, required regions, search effects, and queue success are atomic.
- [ ] Region persistence failure cannot leave accepted/searchable OCR or successful queue state.
- [ ] Production cancellation/completion races obey one commit-point rule.
- [ ] Stale workers cannot finalize superseded attempts.
- [ ] Page Viewer observes the scanner's real active OCR job/input identity.
- [ ] Duplicate active OCR caused only by `Original` vs `OcrOptimized` mismatch is impossible.
- [ ] Manual Retry is Rust-owned, bounded, and preserves attempt history.
- [ ] Page Viewer uses authoritative source asset identity and dimensions.
- [ ] No hard-coded production OCR source dimensions remain.
- [ ] Rust rejects polygons outside authoritative image bounds.
- [ ] Production Page Viewer renders the real source image and OCR polygons through one shared transform.
- [ ] Region hydration is complete or explicitly partial/loading; never silently truncated.
- [ ] Production tests exercise queue -> provider -> finalizer -> search -> Page Viewer -> correction.
- [ ] Production tests fail if old split APIs are reintroduced.
- [ ] Coroutine cancellation remains cancellation in OCR UI orchestration.
- [ ] Active-library lifecycle behavior is implemented or accurately documented as limited.
- [ ] Stale Page Viewer/OCR text is reconciled.
- [ ] Historical closeout docs accurately record the second review and remediation.
- [ ] Roadmap M11 status is accurate.
- [ ] Every code-bearing PR was merged only from an exact green head.
- [ ] Every required post-merge `master` CI run passed.
- [ ] This TODO contains zero unchecked implementation, testing, qualification, documentation, or cleanup items.

## Stopping condition

The Ralph Loop may stop only when either:

1. Every task/subtask above is implemented, qualified, reconciled in this TODO, merged to `master`, and final post-merge `master` CI is green; or
2. A concrete blocker specifically requires user-supplied information, authorization, credentials, hardware, or a product/architecture decision not already resolved by the companion specification.

CI still running, a bounded CI wait timing out, ordinary implementation/test/lint/format failures, merge conflicts, or transient tool/API errors are not user blockers.
