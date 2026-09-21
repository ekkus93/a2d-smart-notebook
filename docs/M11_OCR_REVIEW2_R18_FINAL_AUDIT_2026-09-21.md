# M11 OCR Review 2 — R18 Final Source Audit Record

Date: 2026-09-21

This document supports `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_TODO_2026-09-19.md` R18. It records the source audit and first-parent merge evidence available from current `master` during final reconciliation.

## Scope

The second review reopened M11 because production Android paths had previously passed CI while still relying on older compatibility APIs. This audit therefore focuses on the shipped production seams, not merely on whether lower-level Rust primitives exist.

## First-parent remediation chain observed on `master`

Current audited base: `3201df62bf096f6a795a63ae67c210e059bf029d`.

| Area | PR | Resulting `master` SHA | Observed first-parent evidence |
|---|---:|---|---|
| R11 transactional finalizer through production queue | #116 | `af944a323a70a83f7657a88366be4b7d35fa00a2` | Commit message: exposed OCR finalizer over FFI, routed queue through finalizer result, added Android finalization workflow path, asserted queue processor submits finalizer requests. |
| R11 production sentinel hardening | #117 | `43ca389adde446dfaebc41234d1348660e4508de` | Commit message: strengthened production queue finalizer sentinels. |
| R12 commit-race semantics | #119 | `9cd0c7fdc279fce8dcbe48b78a7406acf2cf3a7c` | Commit message: added OCR commit-race queue sentinels. |
| R13 active job identity | #120 | `25bff1504271aaedb4c7dde7b471d10fe45f5199` | Commit message: made active OCR job lookup scan-owned; added queued/running `OcrOptimized` and duplicate-start regression tests. |
| R13 Rust-owned manual retry | #121 | `ac0e1f51d61222aa1fd99fa41615148ea45c35d6` | Commit message: staged and wired Rust-owned manual retry policy, durable retry history lookup, UniFFI exposure, and bounded retry tests. |
| R13 Android retry gateway | #122 | `ab093fde81281bf258ef18c2b299264ad8fb0338` | Commit message: added Android manual retry gateway. |
| R13 Page Viewer retry wiring | #123 | `6b097350f9026bf1ea9b37fa78f025699be95509` | Commit message: Page Viewer Retry calls Rust-owned bounded manual retry and asserts same job is requeued with cumulative attempt history. |
| R14 source geometry resolver | #124 | `ddba7c761402950bd6c2e10a27f35f242ff44a09` | Commit message: added Rust OCR source geometry resolver. |
| R14 UniFFI source geometry | #126 | `b5ba4c7bbdc7cc0c1720e86afa2c2a6c0d079f36` | Commit message: exposed OCR source geometry through UniFFI. |
| R14 Android source geometry state | #127 | `ddafd053f6f84cf08ab9b072119cb36ee0219d33` | Commit message: added Android OCR source geometry gateway and Page Viewer state. |
| R14 shared overlay transform | #128 | `514d0ac42f35c1f896966f884b8c1d9f1ad9b003` | Commit message: added shared content-fit transform and wired Page Viewer OCR requests to source geometry. |
| R14 polygon bounds | #129 | `0a13c660674e1bbc3308cd02e561c800b3df98cf` | Commit message: enforced OCR polygon source bounds and validated polygons against durable job source frame. |
| R14 transform edge coverage | #131 | `000e0b1602fe4b8a764160d275f34a9310f1d71b` | Commit message: completed content-fit geometry edge coverage. |
| R14 finalizer/source binding | #133 | `73c9464987740ffbb86cc0ff952b56d569614dbb` | Commit message: bound finalizer polygons to encoded source and enforced source bounds in legacy region recording. |
| R14 real image overlay | #134 | `f6bec4472133216c16835007dc0c683ed430294a` | Commit message: rendered OCR overlay on real source image, hydrated overlay source geometry from OCR readback, and registered instrumentation gate. |
| R15 partial-readback fail-closed behavior | #135 | `626b5d29cfc295a74d871c8ed7833852db0108cd` | Commit message: fail closed on partial OCR region readback. |
| R15 pagination contract | #136 | `716624ab0e86e34688a65a66ec6b345e4f6e8ad3` | Commit message: documented authoritative OCR pagination contract, added Rust-owned pagination, exposed through UniFFI, and hydrated Android pagination. |
| R15 cumulative maximum | #137 | `4e77b4c79a85d7fd7dba5ffc621c82beb6cc002a` | Commit message: enforced authoritative OCR region cap cumulatively and qualified exact capacity boundary. |
| R15 pagination boundary coverage | #138 | `af276394de5965319170a5dce0995a7cc8471cb9` | Commit message: qualified multi-page OCR region pagination. |
| R15 Android hydration qualification | #139 | `cd9318b55412c78e2270b6a24c2da9f6f13ad62f` | Commit message: exposed testable Android pagination hydrator and qualified Android readback pagination hydration. |
| R16 production composition sentinel | #140 | `9121ad6cb2b5d8dba952562f22b6e14d9696c746` | Commit message: qualified production OCR composition through finalizer and used immutable source in production composition sentinel. |
| R17 search cancellation | #141 | `8f8d4a82c316a924f7fc78b2a06419d3c2209270` | Commit message: preserved coroutine cancellation in OCR search and tested propagation. |
| R17 active-library lifecycle documentation | #142 | `9acd2d5cf1f74807bac5480dbefe609e45e3a292` | Commit message: documented active library client lifecycle. |
| R17 Page Viewer cancellation audit | #143 | `bedfba3c2d61f8df8967ffc9fefac5812a70ff2a` | Commit message: recorded Page Viewer cancellation audit. |
| R17 lifecycle/cancellation reconciliation | #144 | `43df2a03f4c696396a630119967238a38e6b6cef` | Commit message: reconciled cancellation and lifecycle evidence. |
| R18 closeout record start | #145 | `337847069b8bc350223290d45044287c37a146d9` | Commit message: added second-review closeout record and M11 closeout addendum. |
| R18 status note | #146 | `3201df62bf096f6a795a63ae67c210e059bf029d` | Commit message: recorded second-review remediation status. |

## Source audit findings on current `master`

### Transactional finalization

`crates/a2d-core/src/ocr_finalization.rs` contains `A2dCore::finalize_ocr_job`, which atomically validates a claimed running job, checks stale attempts and durable cancellation, persists the OCR run, persists required text regions, relies on transaction-triggered search effects, and transitions the queue. The normal queue path exposes this through `crates/a2d-ffi/src/ocr_queue.rs` as `A2dClient.finalizeOcrJob`.

`apps/android/app/src/main/kotlin/com/a2d/notebook/feature/ocr/OcrWorkflow.kt` uses `runClaimedJob` for queue execution and calls `gateway.finalizeOcrJob(...)`. `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/ocr/OcrQueueExecutor.kt` uses `AndroidOcrQueueProcessor.processNext`, which calls `runClaimedJob` and requires `finalizedJob`, not merely a recorded run ID. The split `recordOcrRun` and `recordOcrTextRegions` compatibility methods remain for non-queue workflows/tests and are not the normal queue executor contract.

### Cancellation and stale attempt semantics

`finalize_ocr_job` rejects stale attempt counts and lets durable cancellation win before terminal commit. The Android queue path passes the claimed attempt count from the durable job into `AndroidClaimedOcrStartRequest`, and the finalizer owns the cancellation/completion resolution.

### Active OCR job identity and retry

`crates/a2d-core/src/ocr_queue.rs` uses scan-owned active-job lookup so Page Viewer can see scanner-created `OcrOptimized` jobs and cannot create a duplicate active job by choosing `Original`. The R13 chain adds a Rust-owned bounded manual retry API and routes Page Viewer Retry through it.

### Source geometry, polygon bounds, and overlay transform

The R14 chain introduced source geometry resolution, UniFFI exposure, Android Page Viewer source geometry state, source-bound polygon validation, a shared content-fit transform, and rendering of the actual OCR source image under the overlay. Current finalizer validation checks source identity/dimensions and upper polygon bounds against the authoritative source frame.

### Region hydration completeness

The R15 chain introduced fail-closed partial readback, Rust-owned pagination metadata, cumulative maximum enforcement, and Android pagination hydration. The documented invariant is that a complete overlay is not silently truncated; partial/loading/error states must remain explicit until hydration is complete.

### Production-composition sentinels

The R16 chain added a production-composition sentinel covering queue/provider/finalizer/search/Page Viewer/correction composition and old-API regression risks. This is the slice that specifically addresses the class of defect that escaped the first post-closeout review.

### Coroutine cancellation and active-library lifecycle

The R17 chain preserved coroutine cancellation in OCR UI orchestration and documented that the current product intentionally uses one app-private `A2dClient` for the process lifetime. Mid-process library switching remains unsupported rather than silently opening unrelated native clients from recomposition.

## Remaining reconciliation caveats

Ralph Bridge first-parent inspection verifies the merged PR numbers and resulting `master` SHAs above. This audit did not recover every exact pre-squash PR-head SHA and PR-head CI run ID for R11 through R16 from the current repository files alone. The active TODO's R18.5 evidence checklist should remain open until those exact head/run identifiers are either recovered from PR metadata or the project accepts the merge-SHA/hosted-status records as the retained evidence format.

`docs/A2D_SMART_NOTEBOOK_V01_TODO.md` still needs a final status-language pass so Milestone 11 points at this second-review remediation closeout rather than only the 2026-09-15 closeout.

`apps/android/app/src/main/res/values/strings.xml` still contains at least one stale Page Viewer/OCR string: `page_viewer_text_unavailable` says OCR text remains unavailable until Milestone 11 supplies recognized and corrected text. That should be corrected in the final R18 stale-text pass.

## Interim conclusion

Implementation evidence for R11 through R17 exists on current `master`, but R18 should not be marked fully complete until the stale roadmap/resource text is corrected, the exact evidence policy is resolved in the active TODO, final exact-head CI passes, the guarded merge succeeds, post-merge `master` CI is green, and the active TODO reaches zero unchecked items.
