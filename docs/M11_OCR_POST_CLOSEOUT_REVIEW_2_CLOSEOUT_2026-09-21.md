# M11 OCR Post-Closeout Review 2 — Closeout Record

Date: 2026-09-21

This document records the second post-closeout remediation defined by `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_SPEC_2026-09-19.md` and tracked by `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_TODO_2026-09-19.md`.

## Why M11 was reopened

The first post-closeout remediation passed its recorded CI gates, but a second source review found that several production Android paths still used older compatibility APIs even though stronger Rust primitives already existed and had isolated tests. In particular, prior green CI did not prove that normal queue execution used transactional OCR finalization, that Page Viewer used the scanner's durable OCR job identity and authoritative source geometry, or that all coroutine/lifecycle paths preserved the intended invariants.

The earlier PR and CI records remain valid historical evidence for the exact slices they tested. They must not be interpreted as proof that the second-review production-path findings were already closed.

## Final architecture decisions

- **Transactional finalization:** normal Android queue execution submits one claimed-attempt finalization request to Rust. Rust owns validation, terminal outcome, OCR run persistence, required text regions, search-index effects, retry/exhaustion state, and queue terminal transition in one coherent transaction. Legacy split record-run/record-regions/complete methods are compatibility/test/migration surfaces, not the normal executor contract.
- **Cancellation commit point:** durable cancellation wins before terminal commit; successful terminal commit wins afterward. Late cancellation cannot rewrite accepted completion, cancelled-before-commit output cannot become searchable durable OCR, and stale claimed attempts cannot finalize superseded work.
- **Active OCR job identity:** scanner and Page Viewer use the durable Rust-owned active job for the scan and its actual input identity. Page Viewer does not invent `Original` when the scanner created an `OcrOptimized` job.
- **Manual retry:** retry is a Rust-owned bounded durable transition with explicit eligibility. Android Page Viewer Retry invokes that transition rather than creating an unrestricted fresh queue job; prior diagnostics and attempt history remain durable.
- **Source geometry:** the OCR job/run is bound to its actual source asset and validated source pixel width/height. Android does not fabricate OCR source dimensions.
- **Polygon bounds:** persisted region coordinates use source-pixel coordinates and Rust enforces finite, non-negative coordinates bounded by the authoritative source dimensions, including upper-edge checks and source/run/input consistency.
- **Overlay transform:** Page Viewer renders the actual OCR source image and computes one aspect-preserving content-fit transform. The same scale/translation and letterbox/pillarbox offsets are used for polygon drawing and hit testing; interaction outside the rendered image is not treated as an OCR-region hit.
- **Region hydration:** Rust owns the supported region maximum and paginated readback metadata. Page Viewer does not call a partial page complete; loading/partial/error state remains explicit until required pages are hydrated.
- **Active-library lifecycle:** the current product intentionally uses one app-private `A2dClient` for the process lifetime. Queue, Search, Page Viewer, and correction composition share that client. Mid-process library switching is intentionally unsupported; any future switch must coordinate disposal/recreation of the client and dependent OCR gateways/controllers.
- **Correction provenance:** user corrections remain separate durable records from immutable original OCR. The current OCR search contract continues to index original OCR; unified original+corrected ranking/source labels remain a separate design task.

## Production-path audit result

The second-review remediation established production-path sentinels for the invariants that escaped the earlier closeout: queue/provider/finalizer composition, cancellation/commit races, active-job identity, manual retry, source geometry and overlay transform, complete region hydration, Search/Page Viewer/correction composition, and coroutine cancellation propagation. The final R17 work also reconciled lifecycle documentation with the intentionally process-lifetime client implementation.

No completion claim in this closeout expands M11 into cloud OCR, accounts, server search, WorkManager/background-service guarantees, unified corrected+original search ranking, or Milestone 12 scale benchmarking. Queue execution remains app-lifetime and durable work resumes when the app can execute again.

## R17 evidence immediately preceding final closeout

- PR #141 exact head `df69e3d8e001a71abb2ea7015df0594bf0955b54`; merged `master` `8f8d4a82c316a924f7fc78b2a06419d3c2209270`; exact-head CI `35542220821`; post-merge CI `35544717141`.
- PR #142 exact head `e84510051fa0e233ab6858eea6f5e8a47b9b60aa`; merged `master` `9acd2d5cf1f74807bac5480dbefe609e45e3a292`; exact-head CI `35547909851`; post-merge CI `35549580451`.
- PR #143 exact head `39e452562d6bc4f0e802330669b35ef0a5e17c76`; merged `master` `bedfba3c2d61f8df8967ffc9fefac5812a70ff2a`; exact-head CI `35551236334`; post-merge CI `35554477106`; hosted-status publication `35555226089`.
- PR #144 exact head `2cc0626f54ea7c3739bfc168482d8327b8245c99`; merged `master` `43df2a03f4c696396a630119967238a38e6b6cef`; exact-head CI `35557897880`; post-merge CI `35560854298`; hosted-status publication `35561756704`.

## Closeout qualification rule

This document is not itself sufficient to close the tracker. Final closeout requires the R18 documentation/source audit to be reconciled in the companion TODO, this final documentation head to pass all required exact-head CI, guarded merge of that exact green head, green post-merge `master` CI and hosted-status publication where applicable, and zero unchecked implementation/testing/qualification/documentation/cleanup items in the second-review TODO.
