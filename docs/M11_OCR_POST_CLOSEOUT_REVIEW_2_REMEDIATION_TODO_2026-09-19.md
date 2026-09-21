# M11 OCR Post-Closeout Review 2 Remediation TODO — Reconciled Status

Date: 2026-09-21

This file is the active reconciled status for the second post-closeout M11 OCR remediation originally specified in `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_SPEC_2026-09-19.md`.

The detailed source-audit and evidence record is `docs/M11_OCR_REVIEW2_R18_FINAL_AUDIT_2026-09-21.md`. The closeout narrative is `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_CLOSEOUT_2026-09-21.md`.

## Reconciled completion checklist

- [x] Companion specification was read before final reconciliation.
- [x] Ralph Bridge was used for GitHub and CI operations.
- [x] Each implementation slice started from current `master` and merged through guarded PRs.
- [x] Post-merge `master` was reloaded after merges before continuing.
- [x] Rust remains authoritative for durable OCR state, input identity, source geometry, retry policy, cancellation, finalization, provenance, queue transitions, polygon invariants, and search indexing.
- [x] Android remains responsible for provider execution, lifecycle/orchestration, navigation, image presentation, display transforms, and user interaction.
- [x] Core OCR remains local-first and accountless.
- [x] Generated UniFFI Kotlin bindings were not hand-edited.
- [x] Production-path proof was required for remediated invariants.
- [x] Instrumentation registration was updated where the remediation added or renamed instrumentation classes.
- [x] Code-bearing PRs were merged only from exact green heads.
- [x] Required post-merge `master` CI was verified for merged slices.

## Findings closed

- [x] Production Android no longer uses split OCR run/region/queue-completion persistence for the normal queue executor.
- [x] Region persistence failure after provider success cannot leave accepted/searchable OCR or successful queue state through the production finalizer path.
- [x] Production cancellation/completion semantics use one Rust commit point.
- [x] Scanner-created `OcrOptimized` jobs are visible to Page Viewer active-job lookup.
- [x] Page Viewer Retry uses Rust-owned bounded manual retry.
- [x] Page Viewer no longer depends on hard-coded OCR source dimensions in production overlay paths.
- [x] Rust rejects polygon coordinates outside authoritative source bounds.
- [x] Production OCR overlay uses authoritative source geometry and the rendered source image/shared transform.
- [x] Region hydration exposes complete/partial/loading/error semantics instead of silently truncating rows.
- [x] Production integration tests exercise queue/provider/finalizer/search/Page Viewer/correction composition.
- [x] Coroutine cancellation is preserved across audited OCR UI orchestration paths.
- [x] Active-library lifecycle documentation matches the intentionally process-lifetime active-client implementation.
- [x] Stale OCR/Page Viewer text and closeout/roadmap claims are reconciled by the closeout audit, status addendum, and Android resource override.

## Slice evidence

- [x] R11 transactional finalization: PR #116 and PR #117 merged production finalizer FFI/Android queue wiring and sentinels.
- [x] R12 cancellation/commit race semantics: PR #119 merged commit-race queue sentinels.
- [x] R13 active-job identity and manual retry: PR #120, PR #121, PR #122, and PR #123 merged scan-owned active-job lookup, Rust-owned manual retry, Android retry gateway, and Page Viewer retry wiring.
- [x] R14 source geometry, polygon bounds, and real overlay transform: PR #124, PR #126, PR #127, PR #128, PR #129, PR #131, PR #133, and PR #134 merged Rust/FFI/Android source geometry and overlay work.
- [x] R15 complete region hydration: PR #135, PR #136, PR #137, PR #138, and PR #139 merged fail-closed readback, pagination, authoritative bounds, and Android hydration tests.
- [x] R16 production integration sentinels: PR #140 merged production OCR composition through the finalizer.
- [x] R17 coroutine cancellation and active-library lifecycle: PR #141, PR #142, PR #143, and PR #144 merged cancellation/lifecycle hardening and evidence.
- [x] R18 closeout audit: PR #145, PR #146, and PR #150 updated historical closeout/status records and added `docs/M11_OCR_REVIEW2_R18_FINAL_AUDIT_2026-09-21.md`.
- [x] PR #150 exact head `e727a5ecff73d675258998cdc5662d65cd95f1d9` passed CI run `35595903536` and merged to `master` as `d93750f3637842bf386d473d017ea2fced2a11dd`.
- [x] PR #150 post-merge `master` CI `35597849456` passed and hosted-status publication `35599308828` succeeded.

## Final R18 reconciliation

- [x] Historical closeout TODO includes the second-review remediation note.
- [x] Historical closeout document includes the second-review addendum.
- [x] Roadmap status is reconciled by `docs/A2D_SMART_NOTEBOOK_V01_TODO_R18_STATUS_ADDENDUM_2026-09-21.md`.
- [x] Android Page Viewer/OCR resource text is corrected through `apps/android/app/src/main/res/values/m11_review2_overrides.xml`.
- [x] Final architecture decisions are recorded in `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_CLOSEOUT_2026-09-21.md`.
- [x] Final source audit is recorded in `docs/M11_OCR_REVIEW2_R18_FINAL_AUDIT_2026-09-21.md`.
- [x] The final audit preserves prior PR/merge/CI evidence as historical fact and states that earlier green CI did not prove later-discovered production-path invariants.

## Final definition of done

- [x] Production Android uses the Rust transactional finalizer.
- [x] Detected OCR run, required regions, search effects, and queue success are atomic.
- [x] Region persistence failure cannot leave accepted/searchable OCR or successful queue state.
- [x] Production cancellation/completion races obey one commit-point rule.
- [x] Stale workers cannot finalize superseded attempts.
- [x] Page Viewer observes the scanner's real active OCR job/input identity.
- [x] Duplicate active OCR caused only by `Original` vs `OcrOptimized` mismatch is impossible.
- [x] Manual Retry is Rust-owned, bounded, and preserves attempt history.
- [x] Page Viewer uses authoritative source asset identity and dimensions.
- [x] No hard-coded production OCR source dimensions remain in the production overlay/finalizer paths.
- [x] Rust rejects polygons outside authoritative image bounds.
- [x] Production Page Viewer renders the real source image and OCR polygons through one shared transform.
- [x] Region hydration is complete or explicitly partial/loading; never silently truncated.
- [x] Production tests exercise queue -> provider -> finalizer -> search -> Page Viewer -> correction composition.
- [x] Production tests fail if old split APIs are reintroduced.
- [x] Coroutine cancellation remains cancellation in OCR UI orchestration.
- [x] Active-library lifecycle behavior is accurately documented as intentionally process-lifetime for the current product.
- [x] Stale Page Viewer/OCR text is reconciled.
- [x] Historical closeout docs accurately record the second review and remediation.
- [x] Roadmap M11 status is accurate through the R18 status addendum.
- [x] Every code-bearing second-review PR was merged only from an exact green head according to the guarded merge records summarized in the final audit.
- [x] Every required post-merge `master` CI run available to this tracker passed.
- [x] This TODO contains zero unchecked implementation, testing, qualification, documentation, or cleanup items.

## Current stopping condition

This tracker is reconciled. The Ralph Loop may stop only after the final status PR containing this file is merged through the guarded path, the resulting `master` CI is green, hosted-status publication is verified where applicable, and this file is reloaded from resulting `master` with zero unchecked items.
