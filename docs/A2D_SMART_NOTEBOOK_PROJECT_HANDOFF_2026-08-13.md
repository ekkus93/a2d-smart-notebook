# A2D Smart Notebook Project Handoff — 2026-08-13

**Repository:** `ekkus93/a2d-smart-notebook`  
**Branch:** `master`  
**Pause-time source head:** `6f69545d7c76e020358cc44b35be2281ca039d80`  
**Pause-time date:** 2026-08-13  
**Authoritative roadmap:** `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`  
**Authoritative specification:** `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`

This document is the handoff for resuming A2D Smart Notebook work after the 2026-08-13 pause. It is intentionally more current than the checked/unchecked state in `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`. The roadmap has not yet been reconciled for the recently implemented Milestones 9.4, 9.5, and 8.5 because the current exact source head has not yet been confirmed green by permanent CI after the last formatting fix.

Do not interpret an unchecked item in the authoritative roadmap as proof that its implementation is absent. Conversely, do not mark the recently implemented items complete until the exact current candidate head has passed the required permanent CI gates.

---

## 1. Resume here

When work resumes, use this order unless a newer CI failure is supplied first:

1. Confirm that the working copy is exactly current `master` and note the new head if it has moved beyond `6f69545d7c76e020358cc44b35be2281ca039d80`.
2. If CI for `6f69545d7c76e020358cc44b35be2281ca039d80` or a descendant reports a failure, fix that failure before doing roadmap closure work.
3. Do not independently poll or monitor CI. The user monitors CI and will provide failures that require action.
4. Once the exact code-bearing head is confirmed green, reconcile `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` for:
   - Milestone 9.4 — Needs Review;
   - Milestone 9.5 — Version UI;
   - Milestone 8.5 — Batch scanner;
   - the milestone status table and recommended execution order.
5. After that closeout, the next substantive coding work is **Milestone 8.6 / FIX-111 — Camera failure matrix**.
6. After 8.6, continue with the physical calibration/evidence work and Milestones 10–19 as described below.

The current candidate should therefore be treated as **implementation present, final exact-head validation pending** rather than either “not implemented” or “fully closed.”

---

## 2. Product and architecture invariants

These are project-level constraints and should not be reopened casually.

### 2.1 Product model

- The first production client is Android.
- A future iOS client is expected.
- The core product must work without an account and without managed A2D cloud services.
- Users must be able to make manual backups containing their complete local data.
- Automatic cloud backup, replication, and multi-device synchronization are optional future paid functionality, not a core dependency.
- Core note capture, browsing, organization, backup, restore, export, and local processing must remain useful offline.

### 2.2 Shared Rust core

Rust is authoritative for portable/canonical behavior, including:

- typed identities and domain models;
- SQLite persistence and transactions;
- asset durability and integrity;
- Notebook/Page identity resolution;
- scan registration;
- scanner recovery semantics;
- batch scanner session semantics;
- comparison/revision policy;
- Needs Review data and decisions;
- backup/export formats when implemented;
- search semantics when implemented;
- skills/provider authorization when implemented;
- portable resource limits and validation.

Do not move canonical policy into Kotlin merely because Android needs to present it.

### 2.3 Android responsibilities

Kotlin/Jetpack Compose owns presentation and Android platform integration, including:

- Compose UI and navigation;
- CameraX integration;
- Android lifecycle and recreation behavior;
- Android permission handling;
- platform file/share/print surfaces;
- secure Android adapters when those milestones exist.

The Android layer may project Rust state but should not independently invent canonical identity, duplicate/revision classification, persistence rules, or durability claims.

### 2.4 Future iOS portability

Future iOS should use Swift/SwiftUI over the same Rust core. New Rust APIs should therefore avoid Android framework types, Kotlin lifecycle concepts, and Android-specific canonical representations.

---

## 3. Working conventions for future sessions

The following workflow has been important for avoiding lost work and unnecessary latency:

- Work directly on `master` unless the user explicitly asks for a branch or PR.
- Commit/push meaningful work frequently so a long chat ending does not strand code on a temporary branch.
- Do not create temporary/self-modifying CI workflows unless explicitly requested.
- Prefer the current repository source as the source of truth; do not overwrite newer `master` with a stale sandbox copy.
- Run local formatting, lint, unit, and static checks as far as the available environment permits.
- Do not spend time monitoring GitHub CI. The user monitors CI and reports failures.
- When a CI failure is supplied, fix the reported failure, push the fix to `master`, and stop instead of polling the replacement run.
- Preserve originals and durable/recoverable staged captures. Never “clean up” an ambiguous failure by deleting user data.
- Do not turn failures into empty/default/fabricated success values.
- Do not claim calibrated duplicate/revision or automatic-capture behavior while thresholds are still based only on synthetic evidence.

---

## 4. Current repository state at the pause

### 4.1 Exact head

The pause-time `master` head is:

```text
6f69545d7c76e020358cc44b35be2281ca039d80
```

Commit message:

```text
Format batch scanner core
```

This commit applies only `rustfmt` output to `crates/a2d-core/src/batch_scanner.rs` after permanent CI reported Rust formatting drift.

The immediately preceding formatting commit was:

```text
1f15c08a31bd9611a89958fe5a16919310365770
```

It formatted `crates/a2d-ffi/src/batch_scanner.rs`.

### 4.2 Validation status

At the pause point:

- The user reported permanent CI failing only because `cargo fmt` would change the two new batch-scanner Rust files.
- Both formatter diffs were applied and pushed.
- No newer CI result has yet been supplied after `6f69545d7c76e020358cc44b35be2281ca039d80`.
- Therefore, do **not** yet record Milestone 8.5 as permanently validated on the exact head.
- A future green run on this head or a descendant containing no substantive regression is the trigger for documentation reconciliation.

The `[EmulatorConsole]: Failed to start Emulator console for 5554` message seen in earlier Android runs was not itself the build-stopping problem; the emulator subsequently ran tests. Treat actual Gradle/instrumentation failures as authoritative.

---

## 5. Roadmap documentation is stale by design at this point

`docs/A2D_SMART_NOTEBOOK_V01_TODO.md` was last reconciled through Milestone 9.3 and still says:

- Milestone 8.5 is open;
- Milestone 9.4 is open;
- Milestone 9.5 is open;
- the recommended order still says to implement 9.4/9.5 and then 8.5.

That no longer matches the code. The implementation has advanced, but the roadmap was deliberately not checked off before exact-head validation.

When the current code is green, update the roadmap in one documentation reconciliation pass rather than treating these as new implementation tasks.

---

## 6. Recent milestone history

### 6.1 Milestone 9.3 — Safe revision rules

Milestone 9.3 is closed.

Important code-bearing head:

```text
e3984b8261de80c0c542c3dba5657c6914cef2bb
```

Roadmap reconciliation commit:

```text
c311f02dbd73c5fe758696744c9bfdaf93736869
```

Permanent CI for the code-bearing closeout was green. The implemented behavior includes preserving original scans, explicit revision decisions, safe preferred-pointer mutation, physical-copy handling, Wrong Scan handling, and audited Rust-owned decisions.

### 6.2 Milestone 9.4 — Needs Review

Milestone 9.4 implementation is functionally complete even though the authoritative TODO still has its boxes unchecked.

Known implementation commits include:

```text
2ff67d7c6b2b97e0b539cc55438a7c5072e80d94
215faf8782e8247eaae8b2d4bfe88efde12f50ac
09497a496cdcd20e2a10b6c19d4b2bd69b341ac4
```

A prior permanent CI run for the 9.4 implementation was green.

Implemented behavior includes:

- the full set of review kinds required by the current specification;
- Rust-owned list/filter/detail/resolve/defer behavior;
- audited resolution semantics;
- no-data-loss handling;
- Android/UniFFI projection used by later version and batch workflows.

Do not redo 9.4. The pending task is roadmap reconciliation after the current combined source head is green.

### 6.3 Milestone 9.5 — Version UI

Milestone 9.5 implementation is present in `master`.

The initial candidate was:

```text
f7c824b8a0dc5a115e7dbf853565d1123b2b06ae
```

Important follow-up fixes included:

```text
9ea9e2b220259536e7b309dd0f2f41a4e580cb8a  Fix Compose bitmap state lint
43fed393dfd20edee9a1b19113ef2ca0c0541fc5  Stabilize preferred UI assertion
503ce9e0956a65cfe2c48f3351d909565d74c187  Fix lazy version history UI scrolling
bc6d6168f0114c314660c4c2e8ae19bd3eb83f49  Fix version-history lazy item scrolling strategy
909f966627b380a70bf7dfc9f35d97240c75e685  Stabilize nested lazy assertions
fdc18a122acc16329901694847442aaaa54a295d  Fix merged-semantics version-history assertion
```

The repeated Android instrumentation failures were test/semantics issues rather than evidence that the version-history business logic was absent. The important lesson for future Compose tests is that `LazyColumn` virtualization and merged semantics can make nested test tags unavailable until the relevant lazy item is composed, and clickable containers may merge descendant semantics.

Implemented 9.5 behavior includes:

- Rust-owned paginated version timeline;
- stable newest-first ordering;
- preferred-version projection even when the preferred scan is outside the current page;
- version comparison through the existing Rust revision-evidence engine;
- Rust-projected comparison grid dimensions;
- side-by-side and overlay presentation;
- changed-region visualization;
- actions for Keep Both, Set Preferred, Another Physical Copy, Wrong Scan, and Move to Needs Review;
- action availability driven by Rust `allowedDecisions`, not Kotlin reclassification;
- terminal decisions remain viewable/comparable but cannot be decided again;
- scanner-to-version-history navigation uses the Rust-confirmed registered Page ID;
- preferred/changed-region/action instrumentation coverage.

The UI also explicitly warns that visual changed regions are evidence, not calibrated duplicate/revision classification.

Do not redo 9.5. Close its roadmap boxes only after the current combined source head is green.

---

## 7. Milestone 8.5 — Batch scanner

### 7.1 Current status

Milestone 8.5 has an implementation candidate in `master`, ending at the current pause-time head after formatter cleanup.

It is **not yet formally closed** because the exact current source head has not yet been confirmed green by permanent CI.

The authoritative TODO currently lists these acceptance criteria:

- keep the active Notebook fixed until explicitly changed;
- save and return immediately to camera;
- queue final processing/OCR;
- nonblocking saved confirmation;
- duplicate-page detection and session summary;
- Review Item integration;
- recreation/process-death behavior without duplicate registration.

The implementation addresses those criteria with the Milestone 11 OCR boundary noted below.

### 7.2 Rust-owned batch session

Primary file:

```text
crates/a2d-core/src/batch_scanner.rs
```

The Rust core owns durable batch-session behavior, including:

- a bounded session identifier;
- a single locked Notebook identity for the session;
- bounded capture entries;
- recovery-token binding;
- captured Page ID and capture timestamp;
- queued/saved/Needs Review entry status;
- registered Scan ID when saved;
- duplicate-page evidence;
- Review Item ID when created;
- user/developer-facing entry message;
- saved/queued/review summary counts;
- explicit batch completion;
- explicit acknowledgement/removal of a completed summary.

A new batch session requires the selected Notebook to exist, not be archived, and be the current active Notebook. Rust rejects a second active batch session rather than silently switching destinations.

### 7.3 Durable queue semantics

The implementation intentionally reuses the existing Rust scanner recovery journal rather than inventing a second canonical capture queue.

The sequence is conceptually:

1. Android reserves a staging path.
2. Rust creates a scanner-recovery record containing the Page, Notebook, layout, policy version, timestamp, and token.
3. Rust adds that recovery token to the durable batch session.
4. CameraX writes/finalizes the staged JPEG.
5. Android can return to camera capture without waiting for full durable scan registration.
6. A serial worker processes queued items one at a time.
7. The final staged JPEG is revalidated.
8. Page Code identity is re-decoded from the final capture.
9. Rust resolves the Page Code again against the locked Notebook.
10. Stored processing policy is resolved again.
11. Native full-resolution preview/marker analysis runs against the actual final capture.
12. Rust performs canonical `register_scan` through the batch-authorized path.
13. Only after successful Rust registration does the batch entry become Saved.
14. Recovery metadata remains available for interruption reconciliation until completion/acknowledgement.

This preserves the distinction between **captured/queued** and **saved**. The UI must never claim “Saved” merely because CameraX wrote a file.

### 7.4 Batch authorization versus single-page approval

Single-page `register_scan` intentionally requires explicit review approval. Batch mode does not bypass that by having Kotlin simply set `userApproved = true`.

Instead, Rust requires an existing durable queued batch entry matching:

- the recovery token;
- the expected Page ID;
- the locked Notebook ID.

Only then does Rust internally authorize the existing registration path. This keeps the workflow distinction Rust-owned.

### 7.5 Duplicate pages

Rust detects whether the same Page ID already appears earlier in the active batch and records `duplicate_page` on the later capture.

Both captures are preserved. Duplicate detection does not silently replace or discard either image.

A saved duplicate produces canonical Needs Review integration rather than automatic destructive resolution.

### 7.6 Needs Review integration

Batch failures and ambiguous results create or associate canonical Rust Review Items.

Current mappings include:

- identity failure → `UnidentifiedPage`;
- processing/registration failure → `ProcessingFailure`;
- saved duplicate → `Duplicate`;
- saved scan with required revision actions → `Revision`;
- saved scan whose quality still needs review → `LowQuality`.

The batch session retains the Review Item ID and summary counts.

### 7.7 Process death and idempotency

Rust reconciliation handles recovery journal state as follows:

- `Captured` / `PreviewReady`: remains queued and may resume processing;
- `Registering`: reconcile against Rust/SQLite state instead of blindly issuing another registration;
- `Committed`: convert the batch entry to Saved with the existing Scan ID;
- missing recovery metadata for a queued entry: move the entry to Needs Review and do not fabricate/retry a registration.

Focused Rust tests cover:

- duplicate Page ID detection without replacing either capture;
- idempotent re-queue of the same recovery token;
- reopening/reconciling a committed recovery without registering a second scan.

### 7.8 UniFFI surface

Primary file:

```text
crates/a2d-ffi/src/batch_scanner.rs
```

The typed FFI projects:

- batch entry status;
- batch review reason;
- begin-session request;
- batch entry;
- batch session and summary counts;
- begin/list/queue/register/report-review/reconcile/complete/acknowledge methods.

Batch registration also reuses the same stored-policy evidence validation contract as single-page registration, including layout, processing-policy version, and pipeline version evidence.

### 7.9 Android batch worker

The Android implementation is intentionally integrated with the existing scanner/recovery code instead of introducing a second canonical state machine.

Important current files include:

```text
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/ScanRegistrationRequest.kt
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageCameraPipeline.kt
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/PolicyAwareSinglePageScannerRoute.kt
apps/android/app/src/main/kotlin/com/a2d/notebook/navigation/A2dNavHost.kt
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/home/HomeScreen.kt
```

The batch worker is serial to avoid unbounded concurrent image-processing memory and to avoid registration races.

It:

- opens or resumes at most one active batch session;
- keeps the Notebook fixed to `session.notebookId`;
- resolves Page Codes against that Notebook;
- creates durable recovery state before capture;
- queues the capture before CameraX finalization;
- releases the capture state after the staged JPEG is finalized;
- re-reads the final JPEG;
- enforces Rust-issued encoded/pixel limits;
- reads EXIF rotation;
- runs native policy-bound preview processing;
- re-decodes QR from the final image;
- re-resolves Page/Notebook identity;
- advances recovery state to preview-ready;
- calls `registerBatchScan`;
- reconciles the Rust session afterward;
- moves processing failures into Needs Review rather than dropping them.

### 7.10 Android UI and navigation

Current navigation includes:

```text
scanner/batch
```

Home exposes a separate Batch Scan action instead of overloading Scan One Page.

The compact Batch UI shows:

- the locked Notebook destination;
- explicit text that the destination remains fixed until the batch is finished;
- camera preview;
- current Page Code guidance;
- saved count;
- queued count;
- review count;
- duplicate count;
- capture-next-page action;
- finish-batch action;
- persisted completed-session summary;
- acknowledgement/close action.

### 7.11 OCR boundary

Milestone 8.5 must not pretend that OCR infrastructure already exists.

The final registration pipeline already produces the OCR-ready derived image asset, but the persistent OCR provider/job subsystem belongs to Milestone 11. The current batch entry message explicitly states that OCR remains queued/unavailable until that milestone exists.

When Milestone 11 is implemented, connect durable saved scans to the Rust-owned persistent OCR queue. Do not build a temporary Kotlin-only OCR queue just to make the 8.5 checkbox look complete.

### 7.12 Recent 8.5 CI failure

The latest supplied failure was:

```text
git diff --exit-code -- '*.rs'
```

after `rustfmt` changed only formatting in:

```text
crates/a2d-core/src/batch_scanner.rs
crates/a2d-ffi/src/batch_scanner.rs
```

Those exact formatter changes have been committed. No behavioral change was made by the latest two commits.

---

## 8. Immediate next task after exact-head validation: Milestone 8.6 / FIX-111

The authoritative TODO already marks several scanner-failure cases covered, including permission presentation, rotation/stale callback rejection, repeated-capture controller behavior, Notebook/destination gating, recovery journal process-death handling, and torch/cleanup warnings.

The remaining 8.6 work is the next substantive implementation/hardening block.

### 8.1 Full unavailable and bind-failure matrix

For every camera unavailable/bind failure, define and test:

- phase/state before failure;
- whether a staging reservation/file exists;
- whether a scanner recovery record exists;
- whether a batch entry exists;
- what remains durable;
- whether retry is automatic, manual, or prohibited;
- whether the camera generation changes;
- exact user-visible state/action.

Do not collapse camera-unavailable, bind failure, capture failure, processing failure, and cleanup warning into one generic state.

### 8.2 Backgrounding at every capture/finalization boundary

Test app background/stop/recreation/process-death around at least:

- before staging reservation;
- after reservation but before recovery creation;
- after recovery creation but before batch queueing;
- after queueing but before CameraX write starts;
- while CameraX writes;
- after JPEG finalization but before worker processing;
- during native preview processing;
- after preview-ready but before registration;
- during registering;
- after SQLite/asset commit but before recovery reconciliation;
- after recovery committed but before batch-session reconciliation;
- during batch completion/acknowledgement.

Expected behavior must be explicit and idempotent for every boundary.

### 8.3 Batch out-of-order behavior

8.5 currently uses a serial worker, but 8.6 still needs acceptance evidence for lifecycle/callback ordering anomalies, for example:

- capture callback from an obsolete camera generation;
- a delayed result after the user has advanced to another Page Code;
- worker/reconciliation result arriving after UI recreation;
- completed/failed work appearing in a different callback order than capture initiation.

Rust durable identity must remain authoritative regardless of callback order.

### 8.4 Real low-storage evidence

Add real size-limited-filesystem/ENOSPC-style evidence for:

- creating the staging reservation;
- CameraX writing the staged JPEG;
- Rust reading/finalizing staged input;
- immutable original asset commit;
- corrected/OCR/thumbnail asset commit;
- registration journal write;
- SQLite transaction/finalization;
- cleanup after a failed commit.

The result must document which files are retained for recovery and which may safely be deleted.

### 8.5 Consolidated FIX-111 matrix

Produce one authoritative table covering every case with columns equivalent to:

- failure/cancellation case;
- lifecycle/capture phase;
- Rust recovery phase;
- batch entry phase if applicable;
- files present before;
- files retained after;
- database state;
- automatic/manual retry policy;
- Needs Review behavior;
- user-visible result;
- focused automated/physical evidence reference.

This matrix should become the closure artifact for 8.6/FIX-111 rather than leaving behavior scattered across tests and implementation comments.

---

## 9. Physical evidence and threshold calibration remain major release work

Milestone 7 software/synthetic work is substantially complete, but physical evidence remains open and blocks production calibration claims.

Still required:

- legally/privacy-safe photographed Android fixtures;
- device metadata;
- print/paper metadata;
- lighting/capture metadata;
- source/consent/license metadata;
- representative physical `arm64-v8a` latency and memory measurements;
- detector/rectification/end-to-end measurements;
- physical printer and KDP validation;
- evidence-derived capture thresholds;
- evidence-derived duplicate/revision thresholds.

Until that work is complete:

- automatic capture must remain disabled if production policy requires physical calibration;
- durable registration should continue to preserve raw quality evidence and use Needs Review where appropriate;
- version comparison may show changed-region evidence, but must not claim calibrated near-duplicate/revision/substantially-different classifications.

This work spans Milestones 7, 9.2, and 17.

---

## 10. Remaining roadmap after scanner closure

### 10.1 Milestone 10 — Library and page presentation

Still open:

- Home populated/empty states and recent Notebooks;
- scanning continuation;
- review count and backup state;
- Library hub;
- Notebook detail with unscanned logical slots;
- Smart Page, Page Set, and Collection browsing;
- full Page viewer;
- versions integration in the Page viewer;
- Trash, restore, permanent-delete consequences, and no ID reuse.

Do not renumber logical Notebook pages based on scan order.

### 10.2 Milestone 11 — OCR and correction

Still open:

- Rust OCR provider/job contract;
- bounded adapter validation;
- Android ML Kit adapter;
- persistent restart-safe OCR queue after durable scan save;
- deduplication;
- provenance/confidence/status/warnings;
- OCR failure → Needs Review without blocking scan browsing;
- text-region correction UI;
- correction history;
- corrected-text preference.

This milestone should also complete the OCR side of Batch Scanner’s “final processing/OCR” acceptance path.

### 10.3 Milestone 12 — Local search

Still open:

- Rust-owned FTS schema;
- transactional reindexing;
- typed filters/pagination/stable sorting;
- excerpts and explicit syntax failures;
- Android search UI;
- source-region navigation;
- 10,000-page scale evidence;
- search-index integrity integration.

### 10.4 Milestone 13 — Manual backup, restore, and export

This is a core product requirement, not an optional cloud feature.

Still open:

- bounded/versioned `.atnb` format;
- compatibility fixtures;
- encryption/authentication;
- streamed consistent backup;
- Android backup hub;
- non-mutating inspect;
- Replace restore;
- Merge restore;
- rollback behavior;
- idempotent IDs and explicit immutable-content conflicts;
- image/Markdown/text/JSON/searchable-PDF exporters;
- corruption/traversal/resource/space/cancellation/process-death tests.

### 10.5 Milestone 14 — Model providers and A2D Skills

Still open:

- provider capability contracts;
- secure secret-store handles;
- local-network OpenAI-compatible provider;
- versioned skill manifest;
- Rust-owned permission enforcement;
- read-only default;
- narrow tools only;
- proposal/audit model;
- prompt-injection/exfiltration defenses;
- deterministic and configured-model skills;
- Skills UI/history/revocation/provider-not-configured states.

Do not give skills raw SQL, shell, arbitrary filesystem, or implicit network access.

### 10.6 Milestone 15 — iOS readiness

Already present:

- Swift binding generation smoke;
- portable Rust API direction;
- Apple compile feasibility for configured shared code.

Still open:

- minimal Swift harness;
- enum/error/details mapping review;
- XCFramework packaging documentation/automation;
- adapter inventory;
- later API portability audit.

Production SwiftUI application remains post-v0.1 unless scope changes.

### 10.7 Milestone 16 — Security, diagnostics, and failure hardening

Still open includes:

- automatic secret/note-content redaction;
- user-controlled diagnostics export;
- review-resolution diagnostics;
- archive entry/count/size limits;
- provider/skill network/resource limits;
- complete dependency-update/fixture-review procedure;
- Android diagnostics UI;
- real disk-full evidence;
- broader corruption policy;
- future OCR/model/backup/restore failure injection.

### 10.8 Milestone 17 — Physical print validation

Still open in full physical terms:

- exact proof assets/design versions;
- home-printer US Letter/A4 matrix;
- Actual Size/Fit, grayscale, paper, toner variables;
- supported Android-phone matrix;
- KDP front/middle/back and gutter behavior;
- writing, shadow, angle, and lighting matrix;
- marker/QR success and geometry error;
- effective resolution;
- OCR quality;
- latency/failure reasons;
- evidence-derived threshold/device floor.

### 10.9 Milestone 18 — UX and accessibility

Still open:

- every v0.1 screen;
- product-wide terminology;
- accessibility labels;
- font scaling;
- contrast;
- touch targets;
- non-color cues;
- haptic/audio preferences;
- backup visibility;
- model-provider disclosures;
- final accountless UX acceptance.

### 10.10 Milestone 19 — Release validation

Still required before v0.1 release:

- exact final-head permanent CI;
- complete accountless walkthrough;
- two identical physical Notebook copies with distinct identity;
- wrong-design behavior;
- batch out-of-order physical/manual acceptance;
- physical Smart Page print/scan;
- revision preservation;
- OCR correction;
- search;
- backup Replace/Merge;
- exporters;
- skills;
- complete offline core workflow;
- no known silent data loss/unhandled migration failure;
- physical marker validation;
- verified backup/restore;
- secure provider secrets;
- Rust-enforced skill permissions;
- complete release documentation/path validation.

---

## 11. Post-v0.1 work explicitly deferred

Unless scope changes, do not pull these into the current v0.1 critical path:

- production iOS SwiftUI client;
- A2D Sync account/subscription service;
- end-to-end encrypted managed multi-device synchronization;
- managed A2D AI;
- public skill marketplace;
- third-party Notebook Design SDK;
- additional page sizes/orientations;
- advanced handwriting/math OCR;
- diagram understanding;
- spread scanning/dewarping;
- desktop scanning stand;
- web viewer;
- collaboration;
- external task/calendar/email integrations.

---

## 12. Important code map

### Core/identity/storage

```text
crates/a2d-domain/
crates/a2d-storage/
crates/a2d-identity/
crates/a2d-core/
crates/a2d-ffi/
```

### Scanner durability and registration

```text
crates/a2d-core/src/scanner_recovery.rs
crates/a2d-ffi/src/scanner_recovery.rs
crates/a2d-core/src/milestone9.rs
crates/a2d-ffi/src/milestone9.rs
```

### Batch scanner

```text
crates/a2d-core/src/batch_scanner.rs
crates/a2d-ffi/src/batch_scanner.rs
```

### Android scanner

```text
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/camera/
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/
apps/android/app/src/main/kotlin/com/a2d/notebook/rustbridge/
```

The current Batch ViewModel/worker is integrated into the existing scanner package rather than creating a second independent canonical scanner architecture.

### Version UI

```text
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/version/VersionHistoryScreen.kt
apps/android/app/src/androidTest/kotlin/com/a2d/notebook/feature/version/VersionHistoryUiTest.kt
```

### Navigation/Home

```text
apps/android/app/src/main/kotlin/com/a2d/notebook/navigation/A2dNavHost.kt
apps/android/app/src/main/kotlin/com/a2d/notebook/feature/home/HomeScreen.kt
```

### Authoritative docs

```text
docs/A2D_SMART_NOTEBOOK_V01_SPEC.md
docs/A2D_SMART_NOTEBOOK_V01_TODO.md
docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md
docs/A2D_SMART_NOTEBOOK_REMEDIATION_TRACEABILITY_2026-07-31.md
```

---

## 13. Validation checklist when resuming

Start with the narrow checks for files actually changed, then expand.

At minimum, verify the equivalent of:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Then run the repository’s existing Android/native/generated-binding checks as defined by permanent CI, including:

- generated Kotlin UniFFI drift/contract checks;
- Android Kotlin compilation;
- JVM unit tests;
- Android lint;
- native ABI build/package verification;
- focused emulator instrumentation tests.

Do not assume the exact command line above is a substitute for the permanent workflow; inspect `.github/workflows/ci.yml` if the workflow has changed.

If a check produces a formatter diff, apply the formatter output rather than manually approximating its style.

---

## 14. Roadmap reconciliation to perform after a green exact head

Once permanent CI is confirmed green for the current combined code, update `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` as follows.

### 14.1 Milestone status table

Update the high-level table so it no longer says:

- 8.5 is unimplemented;
- 9.4 is unimplemented;
- 9.5 is unimplemented.

Keep explicit physical-calibration and release-evidence caveats.

### 14.2 Milestone 8.5

Review the exact code/evidence against each acceptance criterion and mark `[x]` only where the current exact-head evidence supports it:

- fixed Notebook;
- capture/return-to-camera flow;
- final processing queue and honest OCR boundary;
- nonblocking saved confirmation;
- duplicate/session summary;
- Review Item integration;
- recreation/process-death idempotency.

If the OCR wording in the roadmap is interpreted as requiring a live persistent OCR queue, keep that specific portion open until Milestone 11 rather than claiming a nonexistent subsystem. Prefer clarifying the dependency in the TODO over fabricating completion.

### 14.3 Milestone 9.4

Mark the implemented Needs Review kinds/APIs/audited no-data-loss resolution complete, with exact-head validation evidence.

### 14.4 Milestone 9.5

Mark timeline/preferred indicator, comparison/changed regions, and revision/review actions complete, with exact-head validation evidence.

### 14.5 Recommended execution order

Replace the stale ordering with approximately:

1. 9.4 — done;
2. 9.5 — done;
3. 8.5 — done or explicitly partial only for the Milestone 11 OCR dependency if that interpretation is retained;
4. 8.6/FIX-111 — next;
5. Milestone 7/17 physical evidence and calibration in parallel;
6. Milestones 10–12;
7. Milestone 13;
8. Milestone 14 + remaining 16;
9. Milestones 15, 17, 18, 19.

---

## 15. Risks and things not to regress

### 15.1 Do not weaken durability to make Batch feel faster

Returning to the camera quickly must mean “capture is durably recoverable/queued,” not “scan is already saved.” Only Rust registration may produce Saved.

### 15.2 Do not duplicate canonical recovery state in Kotlin

The scanner recovery journal and batch-session records are Rust-owned. Kotlin state may be recreated from them.

### 15.3 Do not silently re-register after interruption

A `Registering` recovery must be reconciled. If the outcome is ambiguous, preserve it for review rather than issuing another registration blindly.

### 15.4 Do not auto-resolve duplicate pages

Duplicate identity is evidence requiring user/review policy. Preserve both captures.

### 15.5 Do not promote provisional quality into production classification

Physical threshold calibration is still open.

### 15.6 Do not let Version UI reclassify scans in Kotlin

The UI consumes Rust revision/comparison evidence and allowed decisions.

### 15.7 Do not bolt on a fake OCR subsystem

Milestone 11 owns persistent OCR. Batch can preserve OCR-ready assets and a pending state until then.

### 15.8 Do not lose the current code because of chat/session boundaries

Keep meaningful changes committed to `master` frequently.

---

## 16. Suggested first message in the next work session

A concise restart request can be:

```text
Resume A2D Smart Notebook from docs/A2D_SMART_NOTEBOOK_PROJECT_HANDOFF_2026-08-13.md.
Use current master as the source of truth. First verify whether the post-6f69545d CI result is green or fix the CI failure I provide. Do not monitor CI yourself. Once the exact head is green, reconcile 8.5/9.4/9.5 in the authoritative TODO and then start Milestone 8.6/FIX-111.
```

If `master` has advanced, inspect the newer commits first and amend this plan rather than resetting to the old head.

---

## 17. Pause-point summary

The project is well beyond initial architecture/scaffolding. The major recent scanner/revision stack now includes:

- durable single-page registration;
- process-death scanner recovery;
- comparison evidence;
- safe revision decisions;
- Needs Review APIs;
- Version History UI/actions;
- an implementation candidate for durable Batch Scan.

The immediate technical risk is no longer “how should batch scanning work?” The implementation exists. The immediate task is to finish exact-head validation, reconcile documentation, and then harden scanner failure behavior through 8.6/FIX-111.

Beyond scanner hardening, the largest remaining product blocks are physical calibration, full Library UI, OCR, search, backup/restore/export, models/skills, security/diagnostics completion, physical validation, accessibility, and release acceptance.

The current pause-time code head is again:

```text
6f69545d7c76e020358cc44b35be2281ca039d80
```

Treat that as the reference point, not as a permanently green release claim.
