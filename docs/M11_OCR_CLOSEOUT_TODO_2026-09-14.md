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
- [x] Explicit OCR result persistence with terminal states:
  - [x] `Detected`
  - [x] `NoTextDetected`
  - [x] `Unavailable`
- [x] Core API for recording OCR terminal outcomes.
- [x] FFI prepare/record APIs.
- [x] Android OCR workflow integration.
- [x] Rust-owned text-region persistence.
- [x] Core/FFI text-region recording.
- [x] Android text-region persistence wiring.
- [x] OCR readback for Page Viewer hydration.
- [x] OCR search index and `searchOcrText` FFI surface.

## Current active slice — Android OCR search UI integration

Branch: `ralph/m11-android-ocr-search-ui-slice`
Base: `faa81ac0a1b9b3e8f4adad6bf3d4889521ae26b4`

### M11-S1 — Android search adapter

- [x] Add an Android wrapper around `A2dClient.searchOcrText`.
- [x] Define Android-facing OCR search DTOs:
  - [x] query request
  - [x] result list state
  - [x] hit item state
  - [x] document kind display mapping for full-text vs text-region hits
- [x] Preserve Rust error semantics at the Android boundary.
- [x] Do not synthesize hits for no-text, unavailable, or cancelled OCR outcomes.
- [x] Add unit tests for wrapper mapping and validation-error propagation.

### M11-S2 — Search presentation surface

- [x] Add a local-first OCR search presentation surface.
- [x] Include clear empty states:
  - [x] no query entered
  - [x] no matching OCR text
  - [x] OCR search unavailable due to Rust error
- [x] Display for each hit:
  - [x] snippet
  - [x] page ID
  - [x] scan ID
  - [x] OCR run ID
  - [x] text region ID when present
  - [x] full-text vs text-region source
- [x] Keep presentation honest: the UI must say OCR search uses locally persisted OCR text and not a server index.

### M11-S3 — Navigation and Library Hub integration

- [x] Add an OCR Search destination to the Android navigation graph.
- [x] Add a Library Hub entry for OCR Search.
- [x] Ensure the route can navigate back to Library Hub.
- [x] Ensure the route can navigate to Page Viewer for a selected page hit when a page ID is present.
- [x] Update strings without truncating existing `strings.xml` resources.

### M11-S4 — Android UI tests

- [x] Add Compose/unit tests for OCR search presentation states.
- [x] Add instrumentation coverage if navigation or scroll behavior needs emulator validation.
- [x] If an instrumentation test class is added, update the CI emulator class list.
- [x] Avoid ambiguous duplicate text assertions; prefer test tags or scroll to explicit nodes.

### M11-S5 — PR validation and merge

- [ ] Open PR for Android OCR search UI integration.
- [ ] Validate exact PR head:
  - [ ] Rust fmt/clippy/tests
  - [ ] cargo-deny
  - [ ] Kotlin UniFFI binding drift
  - [ ] Android native/binding generation
  - [ ] Android lint/unit/APK
  - [ ] Android emulator instrumentation when present
  - [ ] Milestone 7 Native Validation when triggered
- [ ] Fix any failures with replacement commits on the same branch.
- [ ] Merge exact green PR head.
- [ ] Verify post-merge `master` CI.

## Remaining M11 slices after Android search UI

### M11-C1 — OCR correction workflow

- [ ] Define the user-correction model over OCR text.
- [ ] Persist corrected text separately from original OCR output.
- [ ] Preserve provenance for both original OCR and user correction.
- [ ] Decide and document search precedence:
  - [ ] original OCR only
  - [ ] corrected text only
  - [ ] both original and corrected text with source labels
- [ ] Add Rust storage/core/FFI tests.
- [ ] Add Android presentation for correction entry and review.

### M11-R1 — Text-region overlay UI

- [ ] Use stored polygons to render selectable OCR text regions in Page Viewer.
- [ ] Display selected region text and confidence when available.
- [ ] Keep overlay disabled when no Rust-owned region rows exist.
- [ ] Add tests for overlay state and no-region fallback.

### M11-P1 — Real Android OCR provider

- [ ] Add a real Android OCR provider behind the existing `AndroidOcrProvider` interface.
- [ ] Keep provider failures mapped to `Unavailable`, not fake empty detected text.
- [ ] Keep cancellation truthful.
- [ ] Keep provider availability/resource failures explicit.
- [ ] Add tests that the real-provider adapter preserves terminal semantics.
- [ ] Avoid adding any account-gated or network-only OCR dependency to core workflows.

### M11-Q1 — OCR queue/retry orchestration

- [ ] Connect the existing OCR queue/status contract to in-app OCR execution.
- [ ] Persist queued/running/terminal state transitions durably.
- [ ] Bound queue size and retry counts.
- [ ] Make cancellation observable and truthful.
- [ ] Add provider availability diagnostics.
- [ ] Add tests for retry, cancellation, and unavailable-provider behavior.

### M11-D1 — Documentation and closeout

- [ ] Add an M11 closeout document with merged PR chain, guarantees, limitations, and evidence.
- [ ] Record each deferred item explicitly with rationale.
- [ ] Update roadmap/status docs that still describe OCR/search as not implemented.
- [ ] Ensure docs distinguish:
  - [ ] OCR result persistence
  - [ ] OCR readback
  - [ ] OCR search index
  - [ ] OCR correction
  - [ ] real provider support
  - [ ] queue/retry orchestration
- [ ] Verify final closeout PR and post-merge `master` CI.

## Known cleanup

- [ ] Do not use the abandoned scratch branch `ralph/m11-ocr-search-index-slice`; it contains an early bad `Cargo.lock` edit and should not be merged.
- [ ] PR #43 was an accidental duplicate of the already-merged OCR contract branch. It should remain unmerged unless manually closed outside the current Ralph Bridge capability set.

## Current stopping condition

Continue implementing the checklist until all tasks/subtasks are completed and verified, or until a concrete blocker requires user assistance.
