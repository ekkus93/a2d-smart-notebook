# A2D Smart Notebook v0.1 — Authoritative Implementation Roadmap

**Status:** Reconciled on 2026-07-31. Milestones 1–6 have substantial production implementation but are not blanket release-complete; Milestone 7 is software/synthetic-evidence complete with photographed and physical-device calibration evidence pending; Milestone 8.1–8.4 and scanner recovery are implemented; Milestone 8.5 and part of 8.6 remain open; Milestone 9.1–9.2 are implemented without calibrated duplicate/revision classification; Milestone 9.3–19 remain partial or open as stated below.  
**Version:** 0.1  
**Date:** 2026-07-31  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Authoritative specification:** `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`  
**Remediation plan:** `docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md`  
**Remediation traceability:** `docs/A2D_SMART_NOTEBOOK_REMEDIATION_TRACEABILITY_2026-07-31.md`

This file is the authoritative execution roadmap. A checked item means production code and focused evidence exist. A milestone marked **Partial** may contain many checked implementation items while still having explicit evidence, physical-validation, workflow, or product-scope gaps.

The last code-bearing remediation candidate, `d2cb054d2489cf2b0f1e66d9370b5650b31404d0`, passed permanent CI run `30673255456` and Milestone 7 native validation run `30673255457`. The later branch-cleanup commits changed no production files. Future code-bearing completion claims require permanent CI on the exact claimed source head.

---

## 0. Non-negotiable execution rules

For every task:

- [ ] Read the corresponding specification section before changing code.
- [ ] Work directly on `master` unless the user explicitly requests otherwise.
- [ ] Do not create branches, pull requests, or temporary/self-modifying workflows unless the user explicitly requests them.
- [ ] Keep Rust authoritative for canonical identity, data, persistence, validation, workflow policy, resource limits, backup semantics, search semantics, and skill permissions.
- [ ] Keep Kotlin responsible for Android presentation, lifecycle integration, CameraX, platform pickers, print/share, notifications, and secure platform adapters.
- [ ] Add or update focused tests with every implementation change.
- [ ] Run the narrowest relevant tests during development and permanent full CI before a code-bearing completion claim.
- [ ] Preserve original scans and existing user data.
- [ ] Return structured errors, warnings, cancellation, and recovery information.
- [ ] Keep portable fixtures and every referenced handoff/evidence file committed at the exact named path.

Do not:

- [ ] Add Room as a second canonical database.
- [ ] Convert a failure into `None`, `false`, an empty collection, a default value, or fabricated success.
- [ ] Catch coroutine cancellation and present it as an ordinary failure.
- [ ] Delete or overwrite an original scan during rescan or recovery handling.
- [ ] Declare data durable before the documented file, directory, and SQLite durability steps complete.
- [ ] Classify duplicate/revision confidence using uncalibrated thresholds.
- [ ] Upload note data without a user-selected provider and explicit scope.
- [ ] Store API keys, passwords, or recovery secrets in source, Gradle, ordinary Rust configuration, logs, or the canonical database.
- [ ] Give skills raw SQL, shell, arbitrary filesystem, or implicit network access.
- [ ] Mark a task complete when only a mock, placeholder, comment, or synthetic-only physical claim exists.

---

## 1. Reconciled milestone status

| Milestone | Status | Implemented now | Explicitly outstanding |
|---|---|---|---|
| 1 — Repository, Android shell, CI | **Implementation complete** | Workspace, Android shell, permanent CI, checked-in Kotlin binding drift, Swift generation smoke | Release-wide exact-head signoff remains Milestone 19 work |
| 2 — Domain, errors, UniFFI | **Partial** | Typed IDs/entities, structured FFI errors including details, fallible production ID generation, test-only panic injection | Diagnostic redaction enforcement, full failure-erasure audit, trash lifecycle |
| 3 — SQLite and assets | **Partial** | Migrations with digests, atomic preferred-scan workflow, no-replace durable asset finalization, orphan discovery, integrity checker | Real ENOSPC testing, later reviewed repair UX, future restore workflows |
| 4 — QR and designs | **Partial** | QR v1, permanent fixtures, strict parsing, development manifest registry | Initial official physical Notebook Design manifests |
| 5 — Layout and PDF | **Software complete / physical evidence pending** | Layouts, official markers, QR, PDF generation, raster compatibility, Rust limits, hardened finalization | Real printer/paper/camera acceptance |
| 6 — Notebook and Smart Pages | **Implementation complete / release evidence pending** | Rust workflows, Android UI, Rust-owned limits, cancellation and recreation hardening | Broader release walkthrough and physical workflows |
| 7 — Detection and image processing | **Software and synthetic evidence complete** | AprilTag, image inputs, QR boundary, rectification, metrics, derived images, deterministic corpus | Photographed fixtures, physical `arm64-v8a` measurements, calibrated thresholds, ADR 0002 acceptance |
| 8 — CameraX scanning | **Partial** | 8.1–8.4 and process-death recovery | Batch scanner and remaining 8.6 matrix cases |
| 9 — Durable scans and revisions | **Partial** | 9.1 durable registration; 9.2 asset-backed fingerprints, changed regions, reasons and confidence availability | Calibrated thresholds, 9.3 revision decisions, 9.4 Needs Review, 9.5 version UI |
| 10 — Library UI | **Not implemented** | — | Entire milestone |
| 11 — OCR | **Not implemented** | Domain/storage scaffolding only | Provider, queue, persistence workflow, correction UI |
| 12 — Search | **Not implemented** | Crate scaffolding only | FTS, API, UI, scale tests |
| 13 — Backup/restore/export | **Not implemented** | Crate scaffolding only | `.atnb`, encryption, create/inspect/restore/export workflows |
| 14 — Models and skills | **Not implemented** | Crate scaffolding only | Providers, runtime, permissions, tools, built-ins and UI |
| 15 — iOS readiness | **Partial** | Swift binding generation smoke and Apple target compile feasibility | Swift harness, mapping review, XCFramework and adapter inventory |
| 16 — Security and diagnostics | **Partial** | Many input limits, dependency policy, migration integrity, bounded non-destructive library integrity report | Redaction, diagnostic export, remaining parser/provider limits and failure injection |
| 17 — Physical print validation | **Not implemented** | Deterministic raster tests only | Physical printer/KDP/device matrix and measured thresholds |
| 18 — UX/accessibility | **Partial** | Implemented scanner/notebook/Smart Page screens have explicit states | Full screen set, accessibility and product-wide UX acceptance |
| 19 — Release validation | **Partial** | Core permanent CI gates exist | Full product workflows, backup/search/OCR/skills/manual release walkthrough |

---

# Milestone 1 — Repository, toolchain, Android shell, and CI

**Status: Implementation complete.**

- [x] Rust workspace, pinned toolchain, metadata, lint policy, crate boundaries, and project documentation.
- [x] Android Kotlin/Jetpack Compose shell with `minSdk = 26`, navigation, JVM tests, and instrumentation tests.
- [x] No Room canonical database.
- [x] Permanent CI runs Rust format, strict Clippy, workspace tests, dependency/license policy, Android lint/JVM tests/debug assembly, native ABI builds, binding drift, APK verification, and emulator integration.
- [x] Kotlin UniFFI output is committed at `apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt`; it is generated, never hand-edited, and permanent CI fails on drift.
- [x] Swift bindings are generated as a smoke check by `crates/a2d-ffi/tests/binding_generation.rs` even though no SwiftUI client exists.
- [x] Fixture compatibility and packaged shared-Rust execution are permanent CI gates.
- [ ] Milestone 19 must still validate the eventual complete v0.1 product head and workflows.

---

# Milestone 2 — Domain model, structured errors, and UniFFI

**Status: Partial. Core domain and FFI implementation are present; diagnostics/audit and lifecycle work remain.**

## 2.1 Strongly typed identifiers

- [x] Opaque typed IDs exist for independently persisted entities.
- [x] Canonical Crockford Base32 parse/display/serialization and malformed-input tests.
- [x] Production generation uses OS cryptographic randomness through fallible APIs.
- [x] Deterministic construction is restricted to test interfaces.
- [x] Persistence collisions map to critical integrity errors.

## 2.2 Structured error model

- [x] `A2dError` carries code, category, severity, message key, developer message, retryability, correlation ID, and ordered details.
- [x] Cancellation is distinct from failure.
- [x] Unknown internal failures retain stable correlation IDs.
- [x] `A2dError.details` crosses UniFFI through `A2dFfiErrorDetail`/`A2dFfiErrorDetails` and is tested.
- [ ] Enforce automatic redaction of secrets and raw note content rather than relying only on producer discipline.
- [ ] Complete FIX-140’s project-wide failure-erasure audit.

## 2.3 Entities and invariants

- [x] Domain entities and typed invariants required by implemented milestones.
- [x] Preferred scan ownership is enforced by the atomic Rust storage workflow and database migrations, not only by an in-memory setter.
- [x] Scans require immutable original assets.
- [x] Physical-copy and Smart Page uniqueness constraints.
- [x] Derived records preserve producer/source provenance.
- [ ] Complete trash/restore/permanent-delete identity retention in Milestone 10.

## 2.4 UniFFI boundary

- [x] Thin Rust façade with no SQL or canonical business rules in `a2d-ffi`.
- [x] Kotlin and Swift generation.
- [x] Checked-in Kotlin drift policy.
- [x] Structured errors and details map across the boundary.
- [x] Production ID APIs are fallible.
- [x] Intentional panic injection exists only behind `ffi-test-panic`; production APK verification excludes it.
- [ ] Add a future Swift harness and review enum/error ergonomics under Milestone 15.

---

# Milestone 3 — Rust-owned SQLite and assets

**Status: Partial. Canonical storage integrity is implemented; some environmental failure evidence and future repair workflows remain.**

## 3.1 Database bootstrap and migrations

- [x] SQLite open/create, foreign-key verification, WAL and synchronous-mode verification.
- [x] Numbered immutable migrations.
- [x] Migration history verifies version, name, and SHA-256 content digest.
- [x] Legacy migration rows are sealed transactionally without changing original applied timestamps.
- [x] Future, missing, renamed, gapped, or digest-mismatched migration history fails closed.
- [x] Migration failures do not recreate an empty database.

## 3.2 Repository and transaction layer

- [x] Typed Rust repositories and explicit transactions.
- [x] Constraint failures map to structured errors.
- [x] Preferred-scan mutation is gated behind one audited transaction workflow.
- [x] Page pointer and scan flags remain synchronized; cross-page selection and contradictory legacy state fail closed.
- [ ] OCR replacement and restore merge transactions remain future milestone work.

## 3.3 Asset repository

- [x] Relative paths and canonical containment checks.
- [x] Temporary write, flush, file synchronization, close, reread, hash/length verification.
- [x] No-replace same-filesystem finalization and explicit collision errors.
- [x] Destination and temporary-directory synchronization under the v0.1 contract.
- [x] Immutable original metadata and post-finalization verification.
- [x] Explicit pre-finalization, post-finalization/pre-DB, and DB-phase failure context.
- [x] Non-destructive orphan temp and finalized-asset discovery.
- [x] Cleanup failures are preserved in structured diagnostics rather than discarded.
- [x] Path resolution returns the validated canonical path.
- [ ] Add user-reviewed orphan repair actions later; never silently delete unknown assets.

## 3.4 Integrity and interruption evidence

- [x] Transaction rollback, collision, missing/tampered asset, directory-sync, permission, and migration integrity tests.
- [x] Bounded non-destructive integrity report covers foreign keys, migration identity/digests, relational invariants, asset existence/hashes, temp files, and orphan finalized assets.
- [ ] Add real size-limited-filesystem/ENOSPC evidence.
- [ ] Search-index integrity remains unavailable until Milestone 12 exists.

---

# Milestone 4 — Identity, QR protocol, and Notebook Designs

**Status: Partial because official physical designs do not yet exist.**

- [x] Fallible cryptographic IDs, canonical encoding, vectors, malformed-input and uniqueness tests.
- [x] Canonical QR v1 grammar, CRC-32C, strict bounds/version/alphabet/range checks, and permanent fixtures.
- [x] Rendered QR fixtures decode through reviewed Rust and Android paths.
- [x] Versioned bounded manifest parser and offline registry.
- [x] Registry failures remain internal/integrity failures rather than becoming “unsupported user data.”
- [x] Current bundled manifest is explicitly a development placeholder.
- [ ] Bundle initial official reviewed Notebook Design manifests after physical geometry is selected and validated.

---

# Milestone 5 — Layout engine and Rust PDF generation

**Status: Software complete; physical evidence pending.**

- [x] Canonical physical layouts for Smart Pages and bound-notebook proof pages.
- [x] Official `tagStandard41h12` marker rendering, QR rendering, content styles, page numbering, blank versos, and proof interiors.
- [x] Rust-owned page/count/output limits and checked arithmetic.
- [x] Transactional generated-page registration and immutable PDF asset provenance.
- [x] Deterministic rasterization, QR decoding, marker detection/position checks, and 95/100/105% synthetic print scaling.
- [x] Standalone PDF output uses unique temp files, strict reparse/warning rejection, synchronization, no-replace finalization, and explicit cleanup/recovery details.
- [ ] Print on representative real printers/paper, photograph on supported Android devices, and record physical acceptance evidence.

---

# Milestone 6 — Notebook and Smart Page workflows

**Status: Implementation complete for the current product scope; release-level acceptance remains.**

- [x] Rust Notebook registration, listing, rename, archive, active destination, and page resolution services.
- [x] Android Notebook setup, recognition, duplicate-copy explanation, selection, rename, and archive UI.
- [x] Rust Smart Page generation and transactional registration.
- [x] Android generate, preview, print, save-copy, and share UI.
- [x] Smart Page generation limits and policy version are Rust-owned and projected to Android for presentation validation.
- [x] Kotlin coroutine cancellation is rethrown and not displayed as an ordinary failure.
- [x] QR capture and pending Smart Page save operations survive recreation or reject stale callbacks explicitly.
- [x] Preview bitmaps and temporary captures have explicit ownership/cleanup behavior.
- [ ] Complete product-wide manual acceptance under Milestone 19.

---

# Milestone 7 — Marker detection and image-processing foundation

**Status: Software and deterministic synthetic evidence complete; physical evidence pending.**

- [x] Pinned official AprilTag implementation, safe Rust boundary, license review, Android ABI builds, Apple compile feasibility.
- [x] Bounded encoded/decoded image types and borrowed Gray8 analysis frames.
- [x] Marker identity, role, orientation, corners, centers, margins, and Hamming evidence.
- [x] Android pixel decoding with Rust as canonical QR trust boundary.
- [x] Homography, rectification, raw quality metrics, derived corrected/OCR/thumbnail images, cancellation, and resource limits.
- [x] Deterministic synthetic corpus, manifest/digest checks, regeneration drift, processing envelopes, desktop processing, and packaged Android emulator execution.
- [x] Quality metrics and provisional policy evidence remain distinct from calibrated production classification.
- [x] Uncalibrated policy cannot enable automatic capture or durable accepted classification; registration remains `NeedsReview` with raw evidence.
- [ ] Commit legally/privacy-safe photographed Android fixtures with device, print, lighting, source/consent/license, and capture metadata.
- [ ] Record representative physical `arm64-v8a` latency, memory, detector, rectification, and end-to-end measurements.
- [ ] Derive versioned production thresholds only from reviewed physical evidence.
- [ ] Accept ADR 0002 only after those evidence gates pass.

---

# Milestone 8 — CameraX scanning

**Status: Partial. Single-page scanning and recovery are implemented; batch scanning and part of the camera failure matrix remain open.**

## 8.1 Camera adapter

- [x] Preview, YUV analysis, full-resolution staging capture, lifecycle binding, permission states, rotation, torch, keep-latest backpressure, and frame closure.
- [x] Closed terminal state preserves cleanup warnings.

## 8.2 Live shared-Rust analysis and presentation

- [x] One-copy luminance extraction, typed native ABI, off-main-thread scheduling, latency/copy metrics, cancellation, and stale-result suppression.
- [x] Marker/page overlays, active Notebook banner, actionable guidance, and strict identity gating.

## 8.3 Auto-capture state machine

- [x] Explicit phases, stable-frame policy, debounce, manual-warning confirmation, navigation cancellation, stale-token rejection, and typed terminal outcomes.
- [x] Production automatic capture remains disabled while quality thresholds are uncalibrated.

## 8.4 Single-page scanner

- [x] Active Notebook selection, live marker/QR status, guidance, manual capture, torch, corrected preview, warning details, cancellation, review, and durable registration.
- [x] Processing and registration resolve the stored page layout and one Rust-owned versioned policy.
- [x] Smart Page scope is explicit; unsupported scope does not silently use Notebook-page layout.
- [x] Saved UI appears only after Rust confirms durable registration.
- [x] Navigation, retake, destination changes, and cleanup respect staging/registration ownership.
- [x] Rust-owned process-death journal supports list, validate, review/retry, explicit discard, and idempotent reconciliation without deleting committed originals.

## 8.5 Batch scanner

- [ ] Keep the active Notebook fixed until explicitly changed.
- [ ] Save and return immediately to camera.
- [ ] Queue final processing/OCR.
- [ ] Nonblocking saved confirmation.
- [ ] Duplicate-page detection and session summary.
- [ ] Review-item integration.
- [ ] Recreation/process-death behavior without duplicate registration.

## 8.6 Camera failure matrix

Covered now:

- [x] Permission denied and permanently denied presentation policy.
- [x] Rotation during analysis and stale analysis/capture callback rejection.
- [x] Rapid repeated capture/auto-capture controller behavior.
- [x] Wrong-design, ambiguous Notebook, and conflicting destination gating.
- [x] Process death after staging capture through Rust-owned recovery journal and Android recovery bridge.
- [x] Torch unavailable/failure and CameraX cleanup warning behavior.

Still incomplete or not yet demonstrated at the required specificity:

- [ ] Full unavailable/bind-failure matrix with exact state/file/retry/UI outcomes.
- [ ] Background during every capture/finalization boundary.
- [ ] Batch out-of-order behavior.
- [ ] Real low-storage staging and asset-finalization evidence.
- [ ] One consolidated matrix documenting exact phase, retained/deleted files, retry policy, and user-visible result for every FIX-111 case.

---

# Milestone 9 — Durable scan registration and revisions

**Status: Partial. Durable registration and evidence-only comparison are implemented; revision decisions, review workflows, and version UI remain open.**

## 9.1 Final scan registration

- [x] Canonical staging confinement, symlink/regular-file checks, encoded limits, and concurrent-change detection.
- [x] Full-resolution Page Code, marker/layout, page/Notebook identity, and stored-record revalidation.
- [x] Layout- and policy-driven processing, corrected/OCR/thumbnail derivation, and cancellation.
- [x] Immutable asset journal plus one SQLite transaction for asset rows, scan, page state, preferred-scan invariant, and audit event.
- [x] Typed warnings, required actions, recovery token binding, and retryable retained staging on failure.
- [x] First scan may initialize the preferred pointer; later uncalibrated scans do not replace it automatically.
- [x] Raw metrics are retained and durable quality status is `NeedsReview` until calibrated policy exists.

## 9.2 Fingerprints and comparison

- [x] Corrected-asset SHA-256 embedded in a versioned content fingerprint.
- [x] Deterministic aligned `mean-grid-16x24-v1` perceptual signature.
- [x] Stored comparison verifies both corrected assets against recorded hashes before producing conclusive exact-match evidence.
- [x] Aligned changed cells and connected change regions.
- [x] Mean/maximum difference, pipeline and physical-copy context, confidence availability, and stable reason codes.
- [x] Typed Rust core and UniFFI `compareStoredScans` APIs.
- [ ] Tune duplicate/revision thresholds using reviewed photographed fixtures.
- [ ] Do not classify near-duplicate/revision/substantially-different outcomes until calibration exists.

## 9.3 Safe revision rules

- [ ] Preserve every new original in durable or recoverable staging before prompting.
- [ ] Default proposal: Save as New Version.
- [ ] Replace Preferred changes only the preferred pointer through the atomic Rust workflow.
- [ ] Never delete an older original automatically.
- [ ] Another Physical Copy creates/assigns `PhysicalCopy` explicitly.
- [ ] Wrong Scan moves to Needs Review or is explicitly discarded without deleting committed data.

## 9.4 Needs Review

- [ ] Review kinds for identity, Notebook selection/conflict, quality/alignment, duplicate/revision/physical copy, OCR/processing, import, and restore conflicts.
- [ ] List/filter/detail/resolve/defer APIs.
- [ ] Audited resolution with no data loss.

## 9.5 Version UI

- [ ] Timeline and preferred indicator.
- [ ] Side-by-side/overlay visual comparison and changed regions.
- [ ] Keep both, set preferred, mark another physical copy, and move unresolved cases to review.

---

# Milestone 10 — Library and page presentation

**Status: Not implemented.**

- [ ] Home populated/empty states, recent Notebooks, scanning continuation, Smart Pages, review count, backup state, and primary actions.
- [ ] Library hub for Notebooks, Smart Pages, Page Sets, Collections, imports, Needs Review, and Trash with pagination/sorting.
- [ ] Notebook detail with logical unscanned slots, statuses, scan actions, rename/archive, and no scan-order renumbering.
- [ ] Smart Page/Page Set/Collection browsing and immutable identity behavior.
- [ ] Page viewer for original/corrected/text/split/metadata/versions/annotations/related/skill results.
- [ ] Trash/restore/permanent-delete workflows with consequence display and no ID reuse.

---

# Milestone 11 — OCR and correction

**Status: Not implemented beyond domain/storage scaffolding.**

- [ ] Rust OCR contract, bounded adapter validation, canonical coordinates, provenance, status, warnings, retry, and unavailable confidence.
- [ ] Android ML Kit adapter selection, model-unavailable/cancellation/resource handling, and known-image tests.
- [ ] Persistent background OCR queue after durable scan save with restart-safe deduplication and Needs Review failures.
- [ ] Full-text/region correction UI, low-confidence highlighting, source linking, correction history, and corrected-text preference.
- [ ] OCR failure must not block scan saving or browsing.

---

# Milestone 12 — Local search

**Status: Not implemented.**

- [ ] Rust-owned FTS schema and transactional reindexing.
- [ ] Typed search API with filters, pagination, stable sorting, excerpts, and explicit syntax failures.
- [ ] Android search UI and source-region navigation.
- [ ] 10,000-page scale fixture and latency/memory evidence.
- [ ] Integrate search-index consistency into the integrity report after the index exists.

---

# Milestone 13 — Manual backup, restore, and export

**Status: Not implemented.**

- [ ] Versioned bounded `.atnb` manifest and permanent fixtures.
- [ ] Reviewed Argon2id plus authenticated encryption with unique salt/nonce material and explicit authentication failure.
- [ ] Consistent streamed backup, temporary output, verification, and platform destination confirmation.
- [ ] Android backup hub, progress/cancel, reminders, and failure states.
- [ ] Non-mutating inspect stage.
- [ ] Replace restore into a verified new library with rollback.
- [ ] Merge restore with idempotent IDs and explicit immutable-content conflicts.
- [ ] Original/corrected image, Markdown, text, JSON, searchable PDF, and complete-backup exporters.
- [ ] Corruption, traversal, resource exhaustion, space, cancellation/process-death, and merge-conflict tests.

---

# Milestone 14 — Model providers and A2D Skills

**Status: Not implemented beyond crate scaffolding.**

- [ ] Capability/provider contracts, secure-store handles, limits, cancellation, trust, and disclosure.
- [ ] Production user-configured local-network OpenAI-compatible provider plus deterministic mock/fake-server tests.
- [ ] Strict versioned skill manifest.
- [ ] Rust-owned read-only-default permission enforcement and audited proposals.
- [ ] Narrow tools only; no generic SQL/file/shell/network access.
- [ ] Prompt-injection and exfiltration defenses.
- [ ] Deterministic Markdown export and scan-comparison skills.
- [ ] Configured-model summarization/action-item/related-page/Ask My Notes skills with citations.
- [ ] Skills UI, permission revocation, run history, proposal review, and provider-not-configured states.

---

# Milestone 15 — iOS readiness

**Status: Partial.**

- [x] Swift UniFFI generation smoke runs in Rust tests/permanent CI.
- [x] Shared Rust/image code compiles for Apple device and Apple Silicon simulator targets where configured.
- [x] Canonical Rust APIs avoid Kotlin framework types and Android lifecycle concepts.
- [ ] Compile a minimal Swift harness.
- [ ] Review enum/error/details mapping from Swift.
- [ ] Document and automate XCFramework packaging.
- [ ] Complete future adapter inventory and desktop mocks.
- [ ] Re-audit every later API for portable paths/timestamps/binary representations.

---

# Milestone 16 — Security, diagnostics, and failure hardening

**Status: Partial.**

## 16.1 Diagnostics

- [x] Structured error correlation IDs and ordered diagnostic details.
- [x] Processing/policy/pipeline versions are retained in implemented scan paths.
- [ ] Automatic redaction of note text, API keys, passwords, and recovery keys.
- [ ] User-controlled diagnostic export excluding note content by default.
- [ ] Review-resolution diagnostics after Milestone 9.4 exists.

## 16.2 Input hardening

- [x] QR length and grammar limits.
- [x] Image dimension/pixel/byte/work-set limits.
- [x] PDF page/count/output limits.
- [x] Path canonicalization and symlink/containment checks.
- [x] Parameterized SQL in implemented repositories.
- [x] Manifest parser semantic/resource limits.
- [ ] Archive entry/count/size limits.
- [ ] Skill manifest/model response/network timeout/redirect limits.

## 16.3 Dependencies

- [x] AprilTag and UniFFI pinned.
- [x] AprilTag license review and packaged notices.
- [x] PDF, QR, image, and transitive dependency/license policy enforced through `cargo-deny` and APK verification.
- [ ] Document a complete dependency-update and portable-format fixture-review procedure.

## 16.4 Library integrity check

- [x] Bounded, cancellable, non-destructive report.
- [x] Foreign-key and migration version/name/digest checks.
- [x] Preferred-scan, active-Notebook, page-kind, immutable-original, fingerprint, and generated-PDF reference checks.
- [x] Referenced asset existence and optional full hashes.
- [x] Orphan temporary and finalized asset reporting.
- [x] No automatic destructive repair.
- [ ] Search-index consistency after Milestone 12.
- [ ] Android diagnostics UI after the Rust report contract is finalized.

## 16.5 Failure injection

- [x] Missing/tampered asset, image decode, native processing, permission denial, cancellation, cleanup, migration tamper, and scanner process-death paths have focused tests.
- [ ] Real disk-full evidence.
- [ ] Corrupt-database policy beyond migration/integrity findings.
- [ ] OCR/model/backup/restore failures after those systems exist.

---

# Milestone 17 — Physical print validation

**Status: Not implemented. Deterministic raster tests are not physical proof.**

- [ ] Produce reviewed proof assets and record exact design/layout versions.
- [ ] Home-printer US Letter/A4, Actual Size/Fit, grayscale, paper/toner, and Android-phone matrix.
- [ ] KDP front/middle/back, gutter, writing, lighting, shadows, and angle matrix.
- [ ] Measure marker/QR success, geometry error, effective resolution, OCR quality, latency, and failure reasons.
- [ ] Derive versioned evidence-based thresholds and supported-device floor.

---

# Milestone 18 — UX completeness and accessibility

**Status: Partial.**

- [x] Implemented Notebook, Smart Page, and scanner flows have explicit loading/error/retry/review states and preserve destination visibility.
- [x] Original, corrected, warning, and provisional-quality states are distinguished in implemented scanner UI.
- [ ] Implement every v0.1 screen.
- [ ] Product-wide terminology, accessibility labels, font scaling, contrast, touch targets, non-color cues, haptic/audio preferences, backup visibility, and model-provider disclosures.
- [ ] Never require an account for core workflows.

---

# Milestone 19 — Release validation

**Status: Partial; not a release candidate.**

## 19.1 Automated checks

- [x] Rust format, strict Clippy, workspace tests, dependency/license policy.
- [x] Android lint, JVM tests, debug APK, required native ABIs, symbol/notices verification, and emulator integration.
- [x] Kotlin binding drift and Swift binding generation smoke.
- [x] QR/layout/scan synthetic compatibility fixtures.
- [ ] Backup compatibility fixtures.
- [ ] Automated checks for future OCR/search/backup/model/skill workflows.

## 19.2 Manual acceptance

- [ ] Complete accountless install-to-use walkthrough across every implemented product workflow.
- [ ] Two identical physical Notebooks and separated page identity.
- [ ] Wrong-design handling and batch out-of-order scanning.
- [ ] Physical Smart Page print/scan.
- [ ] Revision preservation, OCR correction, search, backup Replace/Merge, exporters, and skills.
- [ ] Complete core workflow offline.

## 19.3 Release blockers

- [ ] No known silent data loss or unhandled migration failure.
- [ ] Verified backup/restore exists.
- [x] QR protocol has permanent golden vectors.
- [ ] Printed markers have physical validation.
- [ ] Skill permissions exist and are Rust-enforced.
- [ ] Provider secrets use secure storage.
- [x] Scanner cannot claim save before durable Rust registration.
- [x] Production artifacts omit intentional panic-test exports.
- [x] Core implemented behavior does not require A2D servers.

## 19.4 Documentation

- [x] Core build/test/native/binding/QR/layout/storage decisions are documented.
- [ ] Backup format, physical validation, provider/skill, and full release procedures.
- [ ] Complete FIX-151 repository-path validation.

---

# Post-v0.1 backlog — explicitly deferred

- [ ] Production iOS SwiftUI client.
- [ ] A2D Sync account/subscription service and end-to-end encrypted multi-device synchronization.
- [ ] Managed A2D AI and public skill marketplace.
- [ ] Third-party Notebook Design SDK.
- [ ] Additional page sizes/orientations, advanced handwriting/math OCR, diagram understanding, spread scanning, and dewarping.
- [ ] Desktop scanning stand, web viewer, collaboration, and external task/calendar/email integrations.

---

# Recommended execution order from the reconciled state

1. [ ] **Milestone 9.3 — Safe revision rules.** Build the audited decision workflow on the existing durable registration, comparison evidence, and preferred-scan transaction.
2. [ ] **Milestone 9.4 — Needs Review APIs**, followed by **9.5 version UI**.
3. [ ] **Milestone 8.5 batch scanner** and finish the consolidated **8.6/FIX-111 camera failure matrix**.
4. [ ] In parallel, collect **Milestone 7/17 photographed and physical-device evidence** and calibrate versioned capture/comparison thresholds.
5. [ ] Implement Milestones 10–12: Library, OCR, and search.
6. [ ] Implement Milestone 13: manual backup/restore/export.
7. [ ] Implement Milestone 14 and the remaining Milestone 16 security controls.
8. [ ] Complete Milestones 15, 17, 18, and 19 release readiness.
9. [ ] Complete remediation FIX-130/131, FIX-140–142, and FIX-150/151 as their prerequisite implementation surfaces stabilize.

---

# Final completion checklist

- [ ] Every v0.1 specification acceptance criterion is satisfied.
- [ ] Every checked roadmap item has production code and evidence.
- [ ] Rust remains authoritative for canonical data and business logic.
- [ ] Kotlin remains presentation and Android platform integration.
- [ ] Swift bindings and future Apple packaging require no canonical-data redesign.
- [ ] Core product requires no account or A2D server.
- [ ] Manual backup and restore are reliable and verified.
- [ ] Original scans and user data are never silently lost, overwritten, or hidden by a fallback.
- [ ] Every degraded path is visible and reviewable.
- [ ] All referenced repository files exist at exact paths.
- [ ] Permanent CI is green for the exact final release candidate.
