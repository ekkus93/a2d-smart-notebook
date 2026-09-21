# A2D Smart Notebook v0.1 Roadmap — M11 R18 Status Addendum

Date: 2026-09-21

This addendum updates the Milestone 11 status language in `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` without changing the broader v0.1 roadmap scope.

## Current M11 status

Milestone 11 OCR is implementation-complete for the M11 scope after the second post-closeout review remediation. The original 2026-09-15 closeout remains historical evidence, but the current M11 status must be read together with:

- `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_SPEC_2026-09-19.md`
- `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_TODO_2026-09-19.md`
- `docs/M11_OCR_POST_CLOSEOUT_REVIEW_2_CLOSEOUT_2026-09-21.md`
- `docs/M11_OCR_REVIEW2_R18_FINAL_AUDIT_2026-09-21.md`

The second-review remediation hardened the production path beyond the first closeout by requiring and verifying:

- normal Android queue execution through the Rust transactional OCR finalizer;
- one coherent transaction for accepted detected text, required regions, search effects, and queue terminal state;
- cancellation/completion commit-point semantics and stale-attempt rejection;
- scanner/Page Viewer agreement on the durable active OCR job/input identity;
- Rust-owned bounded manual retry instead of ordinary enqueue retry reset;
- authoritative source asset identity and source dimensions;
- Rust polygon upper-bound checks against source dimensions;
- Page Viewer source-image rendering and OCR polygons through one shared content-fit transform;
- explicit complete/partial/loading region hydration semantics;
- production-composition sentinels for queue, provider, finalizer, search, Page Viewer, and correction wiring;
- coroutine-cancellation preservation and honest active-library lifecycle documentation.

## Still deferred outside M11

The following remain outside the M11 second-review remediation and still belong to broader roadmap/release work:

- reviewed physical OCR-quality evidence and device/printer calibration;
- WorkManager/background-service guarantees beyond app-lifetime durable queue resumption;
- cloud OCR, accounts, server-side search, or managed services;
- unified original+corrected OCR search ranking/source labels;
- broader Milestone 12 search scale, filters, and integrity-report integration;
- full release validation across backup, restore, skills, physical validation, and product-wide accessibility.

This addendum supersedes any unqualified reading of the v0.1 roadmap that treats the first M11 closeout as the final production-path sufficiency statement.
