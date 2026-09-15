# M11 OCR Closeout TODO — 2026-09-14

This file is the active tracking checklist for finishing Milestone 11 OCR work in `ekkus93/a2d-smart-notebook`.

## Operating rules

- Keep the core app local-first. Do not require accounts, managed cloud services, or network OCR for core workflows.
- Keep Rust as the owner of durable state, IDs, storage invariants, OCR provenance, search indexing, and terminal OCR semantics.
- Keep Android/Kotlin as presentation and platform-provider orchestration. Android must not fabricate scans, OCR runs, text, review items, or search results.
- Merge only exact green PR heads through the guarded Ralph Bridge merge path.
- Verify every post-merge `master` push CI run before using that commit as the next base.
- If adding or renaming Android instrumentation tests, update `.github/workflows/ci.yml` class lists in the same PR.
- Preserve strict formatting and generated-binding drift checks. Do not hand-edit generated Kotlin UniFFI bindings.

## Completed M11 chain

- [x] OCR request/result/provider contract.
- [x] OCR queue/status contract.
- [x] OCR input preparation through Rust scan/asset validation.
- [x] Explicit OCR result persistence with terminal states: `Detected`, `NoTextDetected`, and `Unavailable`.
- [x] Core API for recording OCR terminal outcomes.
- [x] FFI prepare/record APIs.
- [x] Android OCR workflow integration.
- [x] Rust-owned text-region persistence and Core/FFI recording.
- [x] Android text-region persistence wiring.
- [x] OCR readback for Page Viewer hydration.
- [x] OCR search index and `searchOcrText` FFI surface.
- [x] Android OCR search UI integration — PR #55, merged at `262a76ebd20a7571bd5224fda3711a9c6176eb46`, post-merge CI passed.
- [x] OCR correction workflow.
- [x] Text-region overlay UI.
- [x] Real bundled Android OCR provider.
- [x] Durable OCR queue/retry orchestration — PR #60, merged as `38a1bae9713df6b805ed11f2ad4d579130d35b52`, post-merge CI passed (`34958132321`).

### M11-Q1 — OCR queue/retry orchestration

- [x] Connect the existing OCR queue/status contract to in-app OCR execution.
- [x] Persist queued/running/terminal state transitions durably.
- [x] Bound queue size and retry counts.
- [x] Make cancellation observable and truthful.
- [x] Add provider availability diagnostics.
- [x] Add tests for retry, cancellation, and unavailable-provider behavior.

### M11-D1 — Documentation and closeout

- [x] Add an M11 closeout document with merged PR chain, guarantees, limitations, and evidence: `docs/M11_OCR_CLOSEOUT_2026-09-15.md`.
- [x] Record each deferred item explicitly with rationale.
- [ ] Update all roadmap/status docs that still describe OCR/search as not implemented.
- [x] Ensure closeout docs distinguish OCR result persistence, OCR readback, OCR search index, OCR correction, real provider support, and queue/retry orchestration.
- [ ] Verify final closeout PR and post-merge `master` CI.

## Known cleanup

- [x] Do not use the abandoned scratch branch `ralph/m11-ocr-search-index-slice`; it contains an early bad `Cargo.lock` edit and is explicitly excluded from the closeout chain.
- [x] PR #43 was an accidental duplicate of the already-merged OCR contract branch and is explicitly excluded from the closeout chain; it should remain unmerged unless closed separately.

## Current stopping condition

Continue implementing the checklist until all tasks/subtasks are completed and verified, or until a concrete blocker requires user assistance.
