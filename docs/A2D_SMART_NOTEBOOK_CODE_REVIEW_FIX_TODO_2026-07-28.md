# A2D Smart Notebook — Comprehensive Code Review Remediation TODO

**Status:** Ready for implementation  
**Date:** 2026-07-28  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Target branch:** `master`  
**Primary roadmap:** `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`  
**Authoritative specification:** `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`

---

## 0. Purpose

This TODO translates the 2026-07-28 comprehensive review of the current `master` implementation into an implementation-ready remediation plan.

The work in this file has four goals:

1. Close confirmed correctness, integrity, durability, FFI, lifecycle, and resource-limit defects.
2. Reopen roadmap tasks whose completion claims are stronger than the implementation.
3. Preserve the project’s local-first, accountless, Rust-authoritative architecture.
4. Leave intentionally unimplemented future milestones visibly incomplete rather than filling them with mocks or fake-success behavior.

This is a remediation plan, not permission to redesign unrelated product behavior or implement all of Milestones 10–19 prematurely.

---

## 1. Non-negotiable execution rules

For every task in this file:

- [ ] Work directly on `master` unless the user explicitly changes that instruction.
- [ ] Do not create a branch or pull request unless explicitly requested.
- [ ] Do not add temporary, self-modifying, one-use, or cleanup-push GitHub workflows.
- [ ] Keep Rust authoritative for canonical data, identity, persistence, validation, workflow policy, and portable resource limits.
- [ ] Keep Kotlin responsible for Android presentation, lifecycle integration, CameraX, platform pickers, print/share, and secure platform adapters.
- [ ] Do not convert a failure into `None`, `false`, an empty collection, a default value, or a success state.
- [ ] Re-throw `CancellationException` at every Kotlin coroutine boundary.
- [ ] Do not delete, replace, or mutate a committed original scan automatically.
- [ ] Do not declare a file durable until file contents and the containing directory have been synchronized according to the documented durability model.
- [ ] Do not use `rename` as a no-replace primitive unless the implementation proves that replacement cannot occur on every supported platform.
- [ ] Do not classify duplicate/revision confidence using uncalibrated thresholds.
- [ ] Do not put Android-only paths, URIs, lifecycle concepts, or framework types into portable domain APIs.
- [ ] Add focused regression tests before marking each defect fixed.
- [ ] Run the narrowest relevant tests during development and the permanent full CI workflow before marking the remediation complete.
- [ ] Keep every file referenced by this TODO committed at the exact path named.

A task is complete only when:

- The production implementation exists.
- The failure behavior is explicit.
- Focused tests cover success and failure.
- Existing tests remain green.
- Generated bindings and fixtures have no drift.
- No placeholder or silent fallback remains in the completed path.
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` accurately reflects the resulting state.

---

# Phase 1 — Immediate build and API consistency blockers

## FIX-001 — Regenerate and commit the Kotlin UniFFI bindings

**Priority:** P0  
**Primary paths:**

- `crates/a2d-ffi/src/scan_comparison.rs`
- `crates/a2d-ffi/src/milestone9.rs`
- `crates/a2d-ffi/tests/binding_generation.rs`
- `apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt`
- `tools/generate-bindings.sh`
- `tools/build-android-native.sh`
- `.github/workflows/ci.yml`

### Tasks

- [ ] Regenerate the Kotlin bindings from the current `a2d-ffi` interface using the repository’s permanent generation script.
- [ ] Commit the generated Kotlin file verbatim.
- [ ] Verify the committed binding contains:
  - [ ] `compareStoredScans`
  - [ ] `CompareStoredScansRequest`
  - [ ] `StoredScanComparisonEvidence`
  - [ ] `StoredScanComparisonConfidence`
  - [ ] `StoredScanComparisonReason`
  - [ ] `StoredScanQualityStatus`
  - [ ] `StoredScanChangeRegion`
  - [ ] `StoredScanChangedCell`
- [ ] Confirm the generated function signatures exactly match the Rust projection.
- [ ] Confirm the Android source compiles against the regenerated binding even if no UI calls the comparison API yet.
- [ ] Update binding-generation documentation that incorrectly says generated Kotlin bindings are not committed.
- [ ] State explicitly that the Android binding is checked in and guarded by CI drift detection.
- [ ] Search the repository for any other comments or docs claiming the Kotlin binding is uncommitted build output and correct them.

### Tests and validation

- [ ] Run `cargo test -p a2d-ffi --test binding_generation`.
- [ ] Run the Android binding-drift command used by permanent CI.
- [ ] Run `./gradlew :app:compileDebugKotlin` or the repository-equivalent compile task.
- [ ] Run the permanent full CI workflow and confirm the binding-drift job passes.

### Acceptance criteria

- [ ] `git diff` is clean after regenerating bindings a second time.
- [ ] Android compiles from a fresh checkout without locally regenerated uncommitted source.
- [ ] The current `master` head has a green permanent binding-drift job.

---

## FIX-002 — Add a permanent generated-binding ownership policy

**Priority:** P0  
**Primary paths:**

- `README.md`
- `apps/android/README.md` if present
- `apps/ios/README.md`
- `tools/generate-bindings.sh`
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

### Tasks

- [ ] Document which generated artifacts are committed and which are ephemeral.
- [ ] Document the authoritative generation command for Kotlin and Swift.
- [ ] Document that a Rust FFI surface change requires regenerated Kotlin output in the same commit.
- [ ] Document how Swift generation is smoke-tested even though no SwiftUI client exists yet.
- [ ] Document that CI generation drift is the source of truth; developers must not hand-edit generated binding files.
- [ ] Add a concise troubleshooting section for stale bindings.

### Acceptance criteria

- [ ] No repository documentation contradicts the actual checked-in binding policy.
- [ ] A contributor can identify the required regeneration command without inspecting CI internals.

---

# Phase 2 — Canonical preferred-scan integrity

## FIX-010 — Replace the unsafe preferred-scan repository primitive

**Priority:** P0  
**Primary paths:**

- `crates/a2d-storage/src/repository.rs`
- `crates/a2d-storage/src/workflow.rs`
- `crates/a2d-storage/src/lib.rs`
- `crates/a2d-core/src/`
- `crates/a2d-storage/tests/repository_and_assets.rs`
- `crates/a2d-core/src/milestone9_tests.rs`

### Problem to eliminate

The existing storage operation updates `pages.preferred_scan_id` without proving scan ownership or synchronizing `scans.preferred`. It can create contradictory canonical state.

### Tasks

- [ ] Deprecate or remove the public `PageRepository::set_preferred_scan(page_id, scan_id)` primitive.
- [ ] Introduce one transaction-only workflow operation for changing a page’s preferred scan.
- [ ] Require the operation to receive:
  - [ ] `PageId`
  - [ ] `ScanId`
  - [ ] authoritative `updated_at_ms`
  - [ ] audit actor/context
  - [ ] correlation ID or operation ID
- [ ] Load the target page and target scan inside the same transaction.
- [ ] Return a typed not-found error if either record does not exist.
- [ ] Reject the operation if `scan.page_id != page.id`.
- [ ] Reject a target scan whose required assets or immutable-original invariant is invalid.
- [ ] Clear `preferred = 1` from every other scan belonging to the page.
- [ ] Set `preferred = 1` on exactly the selected scan.
- [ ] Set `pages.preferred_scan_id` to exactly the same selected scan.
- [ ] Update `pages.updated_at_ms`.
- [ ] Insert an audit event in the same transaction.
- [ ] Return a typed result containing the previous preferred scan, new preferred scan, and page ID.
- [ ] Make the operation idempotent when the selected scan is already preferred:
  - [ ] Do not create contradictory duplicate state.
  - [ ] Decide whether to emit a no-op audit event and document the decision.
- [ ] Ensure rollback restores both page and scan flags if any step fails.

### Suggested API shape

```rust
pub struct SetPreferredScanRequest {
    pub page_id: PageId,
    pub scan_id: ScanId,
    pub changed_at_ms: i64,
    pub actor: String,
    pub correlation_id: String,
}

pub struct SetPreferredScanResult {
    pub page_id: PageId,
    pub previous_preferred_scan_id: Option<ScanId>,
    pub preferred_scan_id: ScanId,
    pub changed: bool,
}
```

The exact names may differ, but the invariant must be one atomic Rust-owned operation.

### Schema hardening

- [ ] Add a migration that enforces at most one `scans.preferred = 1` row per page.
- [ ] Use a partial unique index, for example:

```sql
CREATE UNIQUE INDEX unique_preferred_scan_per_page
ON scans(page_id)
WHERE preferred = 1;
```

- [ ] Add a trigger or equivalent constraint that prevents `pages.preferred_scan_id` from pointing to a scan owned by another page if SQLite can enforce this safely.
- [ ] If a trigger is used:
  - [ ] Give it a stable name.
  - [ ] Map its error to a dedicated typed integrity code.
  - [ ] Add migration-upgrade tests with valid and invalid pre-existing state.
- [ ] Fail closed during migration if a pre-existing library contains contradictory preferred state.
- [ ] Do not silently select a winner during migration.

### Tests

- [ ] Setting the first preferred scan updates both tables.
- [ ] Switching preferred scans clears the prior flag and updates the page pointer.
- [ ] Selecting a scan from another page fails without changing either page.
- [ ] Selecting an unknown scan fails without changing state.
- [ ] Selecting an unknown page fails without changing state.
- [ ] Repeating the same preferred selection is idempotent.
- [ ] A forced audit insertion failure rolls back page and scan updates.
- [ ] The unique index rejects two preferred scans for one page.
- [ ] Migration rejects contradictory legacy data visibly.
- [ ] A reopened database round-trips one internally consistent preferred state.

### Acceptance criteria

- [ ] It is impossible through public Rust APIs to make `pages.preferred_scan_id` disagree with `scans.preferred`.
- [ ] It is impossible to prefer a scan belonging to another page.
- [ ] Milestone 9.3 can reuse this operation without adding Kotlin-side business rules.

---

## FIX-011 — Audit every existing preferred-scan call site

**Priority:** P0

### Tasks

- [ ] Search for all direct SQL and repository calls that modify:
  - [ ] `pages.preferred_scan_id`
  - [ ] `scans.preferred`
- [ ] Route every production mutation through the new atomic workflow.
- [ ] Keep insertion-time first-scan behavior transactionally consistent.
- [ ] Verify existing scan registration cannot bypass the workflow.
- [ ] Update tests whose setup directly creates contradictory state unless the test explicitly injects corruption.
- [ ] Label corruption-injection helpers clearly as test-only.

### Acceptance criteria

- [ ] No production path mutates one side of the preferred-scan invariant independently.

---

# Phase 3 — Asset durability, collision safety, and path integrity

## FIX-020 — Define and document the actual durability contract

**Priority:** P0  
**Primary paths:**

- `crates/a2d-storage/src/assets.rs`
- `crates/a2d-storage/src/lib.rs`
- `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

### Tasks

- [ ] Define what “durable” means for v0.1 on Android/Linux filesystems.
- [ ] Distinguish:
  - [ ] userspace flush
  - [ ] file-content synchronization
  - [ ] metadata synchronization
  - [ ] directory-entry synchronization
  - [ ] SQLite transaction durability
- [ ] Document the selected relationship between SQLite `synchronous` mode and asset-file synchronization.
- [ ] Do not claim a file is power-loss durable if only `Write::flush()` has succeeded.
- [ ] Record any platform limitations explicitly.

### Acceptance criteria

- [ ] Documentation and implementation use the same durability terminology.

---

## FIX-021 — Make asset finalization no-replace and power-loss aware

**Priority:** P0  
**Primary path:** `crates/a2d-storage/src/assets.rs`

### Tasks

- [ ] Replace `std::fs::rename` as the collision-prevention primitive.
- [ ] Use a no-replace finalization mechanism appropriate to supported platforms.
- [ ] Ensure an existing destination is never overwritten, even if an `AssetId` collision occurs.
- [ ] Return a dedicated critical integrity error on destination collision.
- [ ] Include the asset ID and destination path in structured error details.
- [ ] After writing the temporary file:
  - [ ] call `flush()`
  - [ ] call `sync_all()` on the file
  - [ ] close the file before finalization
- [ ] Re-read and hash the synchronized temporary file.
- [ ] Finalize into the destination without replacement.
- [ ] Synchronize the containing destination directory after finalization where supported.
- [ ] Synchronize the temporary directory after removal/rename if required by the selected durability model.
- [ ] Verify destination metadata after finalization.
- [ ] For immutable originals, set read-only permissions before returning success.
- [ ] Synchronize metadata after changing permissions when required.
- [ ] Never return an `Asset` value until every required durability step succeeds.

### Platform abstraction

- [ ] Isolate no-replace and directory-sync behavior behind a small internal platform module.
- [ ] Keep Android/Linux implementation explicit.
- [ ] Add a documented future Apple implementation path without embedding Android assumptions in public APIs.
- [ ] Return `Unsupported` explicitly if a platform cannot provide the required semantics; do not silently downgrade.

### Tests

- [ ] A pre-existing destination is never overwritten.
- [ ] Collision returns a dedicated typed integrity error.
- [ ] Original destination bytes remain unchanged after a collision attempt.
- [ ] A forced `sync_all()` failure returns failure and does not create a database row.
- [ ] A forced directory-sync failure returns an explicit recoverable/orphan state.
- [ ] A forced permission-setting failure reports the final asset path and asset ID.
- [ ] Successful commit returns an asset whose file exists and hashes correctly.
- [ ] Original assets are read-only after success.

### Acceptance criteria

- [ ] A cryptographic ID collision cannot overwrite any existing asset.
- [ ] A successful asset commit meets the documented durability contract.

---

## FIX-022 — Make post-finalization failure recoverable

**Priority:** P0

### Tasks

- [ ] Introduce a typed distinction between:
  - [ ] failure before final destination exists
  - [ ] failure after final destination exists but before DB registration
  - [ ] failure after DB registration begins
- [ ] Include recovery details for post-finalization failures:
  - [ ] asset ID
  - [ ] asset kind
  - [ ] final relative path
  - [ ] expected SHA-256
  - [ ] byte length
  - [ ] whether file sync completed
  - [ ] whether directory sync completed
- [ ] Add an explicit orphan-final-asset discovery API.
- [ ] Compare filesystem assets against database asset rows non-destructively.
- [ ] Never delete an unknown final asset automatically.
- [ ] Add a reviewed recovery action for confirmed unreferenced assets later; do not implement silent cleanup now.

### Tests

- [ ] Simulate interruption after finalization and before row insertion.
- [ ] Verify the unreferenced file is reported.
- [ ] Verify recovery reporting never deletes it.
- [ ] Verify a retry cannot overwrite the orphan.

---

## FIX-023 — Stop discarding temporary cleanup failures

**Priority:** P0  
**Primary path:** `crates/a2d-storage/src/assets.rs`

### Tasks

- [ ] Replace `remove_file(...).ok()` cleanup with explicit reporting.
- [ ] Preserve the original failure as the primary cause.
- [ ] Attach cleanup failure as structured secondary details or warnings.
- [ ] Return the temporary path when cleanup fails.
- [ ] Ensure user-visible layers can display that cleanup is incomplete without claiming the asset was saved.
- [ ] Apply the same rule to test cleanup only where test teardown failures matter; do not mask production cleanup failures.

### Acceptance criteria

- [ ] No production cleanup failure is discarded silently.

---

## FIX-024 — Return the validated canonical asset path

**Priority:** P1  
**Primary path:** `crates/a2d-storage/src/assets.rs`

### Tasks

- [ ] Change `AssetStore::resolve` to return the canonicalized candidate path it validated.
- [ ] Verify the candidate is a regular file where a file is required.
- [ ] Reject symlinks when the caller requires immutable library-owned content.
- [ ] Defend against path substitution between validation and open:
  - [ ] Prefer opening a validated handle and operating on that handle.
  - [ ] If path-based reopening remains necessary, document and minimize the TOCTOU window.
- [ ] Keep traversal and root-escape rejection explicit.

### Tests

- [ ] Relative traversal is rejected.
- [ ] A symlink inside the library pointing outside is rejected.
- [ ] A valid nested asset returns its canonical path.
- [ ] A missing asset produces the dedicated missing-asset error rather than a generic canonicalization error.

---

# Phase 4 — Layout-driven scan processing

## FIX-030 — Resolve the actual stored page layout before preview and registration

**Priority:** P0  
**Primary paths:**

- `crates/a2d-core/src/milestone9.rs`
- `crates/a2d-core/src/milestone6.rs`
- `crates/a2d-layout/src/`
- `crates/a2d-ffi/src/milestone9.rs`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/`

### Problem to eliminate

Full-resolution processing and registration are hard-wired to the development writable-page layout, fixed marker IDs, and fixed corrected dimensions. That is not valid for Smart Pages or future Notebook Designs.

### Tasks

- [ ] Add a Rust-core layout-resolution service that accepts a stored `Page` or `LayoutId`.
- [ ] Resolve Notebook Page layouts through the stored `NotebookDesign` manifest.
- [ ] Resolve Smart Page layouts through the bundled canonical layout registry.
- [ ] Reject unknown or unavailable layouts with a typed unsupported/integrity error.
- [ ] Verify the page record’s `layout_id` agrees with its Notebook Design or Smart Page identity.
- [ ] Derive from the resolved layout:
  - [ ] marker family
  - [ ] marker semantic roles
  - [ ] expected marker IDs
  - [ ] physical page dimensions
  - [ ] source/destination corner ordering
  - [ ] canonical corrected aspect ratio
  - [ ] output dimensions selected by versioned policy
  - [ ] layout version/provenance
- [ ] Remove production dependence on `writable_page_layout()` from generic scan registration.
- [ ] Keep any development proof layout explicitly scoped to proof/test paths.
- [ ] Do not infer a fallback layout when resolution fails.

### Suggested internal API shape

```rust
pub struct ResolvedScanLayout {
    pub layout_id: LayoutId,
    pub physical_width_mm: f64,
    pub physical_height_mm: f64,
    pub marker_family: String,
    pub marker_roles: Vec<ResolvedMarkerRole>,
    pub corrected_width: u32,
    pub corrected_height: u32,
    pub layout_version: String,
}
```

### Tests

- [ ] Register a Notebook Page using its design layout.
- [ ] Register US Letter Smart Pages for every supported content style.
- [ ] Register A4 Smart Pages for every supported content style.
- [ ] Verify corrected output aspect ratio matches each physical layout.
- [ ] Reject a stored page whose layout does not agree with its QR payload.
- [ ] Reject a missing layout rather than falling back to the writable proof layout.
- [ ] Reject wrong marker IDs for a resolved layout.
- [ ] Verify future design-specific marker IDs can be represented without Kotlin changes.

### Acceptance criteria

- [ ] Generic scan registration contains no hard-coded assumption that every page is the development Notebook Page layout.
- [ ] Smart Pages and Notebook Pages use the same Rust-owned resolution mechanism.

---

## FIX-031 — Make preview and durable registration use one policy source

**Priority:** P0  
**Primary paths:**

- `crates/a2d-core/src/`
- `crates/a2d-ffi/src/`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerPolicy.kt`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/rustbridge/PagePreviewProcessing.kt`

### Tasks

- [ ] Define a versioned Rust-owned scan-processing policy record.
- [ ] Include only portable processing parameters in the Rust policy.
- [ ] Resolve layout-dependent values in Rust after page identity is known.
- [ ] Expose policy identity/version to Android.
- [ ] Prefer Android passing a policy version or opaque configuration ID rather than duplicating all canonical scalar values.
- [ ] Keep Android-only presentation thresholds separate and clearly named.
- [ ] Ensure preview reports the exact processing-policy version used.
- [ ] Ensure registration reuses or verifies the exact same policy version.
- [ ] Reject review artifacts produced under an unsupported or changed policy version.
- [ ] Remove duplicate canonical values from Kotlin once the Rust source is available.
- [ ] Preserve explicit resource limits; do not replace them with hidden defaults.

### Tests

- [ ] Preview and registration produce matching dimensions and pipeline version for one capture.
- [ ] A policy version mismatch blocks registration explicitly.
- [ ] Kotlin cannot silently select different marker IDs or rectification dimensions.
- [ ] Presentation-only guidance policy changes do not alter canonical processing.

---

## FIX-032 — Decide and implement Smart Page scanning support explicitly

**Priority:** P1

### Tasks

- [ ] Decide whether v0.1 single-page scanning must support known Smart Pages.
- [ ] Record the decision in the spec and main TODO.
- [ ] If Smart Pages are in scope:
  - [ ] Generalize identity gating so a resolved Smart Page can be captured without a Notebook ID.
  - [ ] Continue requiring an exact resolved `PageId`.
  - [ ] Do not invent or attach an active Notebook to a Smart Page.
  - [ ] Display “Smart Page” as the destination type.
  - [ ] Register with the resolved Smart Page record and layout.
  - [ ] Add known and imported-unknown Smart Page behavior.
- [ ] If Smart Pages are deliberately deferred:
  - [ ] Make the scanner entry point visibly Notebook-only.
  - [ ] Block Smart Pages with a specific typed/UI message.
  - [ ] Do not describe Milestone 8.4 as a general single-page scanner.
  - [ ] Add a future task for Smart Page scanning.

### Tests

- [ ] Known Smart Page behavior matches the recorded product decision.
- [ ] Smart Page identity never changes the active Notebook.
- [ ] Unknown imported Smart Pages remain explicit and are not silently registered.

---

# Phase 5 — Structured errors, panic removal, and portable primitives

## FIX-040 — Preserve `A2dError.details` through UniFFI

**Priority:** P0  
**Primary paths:**

- `crates/a2d-domain/src/error.rs`
- `crates/a2d-ffi/src/lib.rs`
- generated Kotlin and Swift bindings
- Android error presentation helpers

### Tasks

- [ ] Add an FFI-safe key/value detail record.
- [ ] Map every `A2dError.details` entry into the FFI envelope.
- [ ] Preserve deterministic ordering from the Rust `BTreeMap`.
- [ ] Preserve existing code, category, severity, message key, developer message, retryable flag, and correlation ID.
- [ ] Do not expose secrets or raw note content while adding detail transport.
- [ ] Add helpers for Android to access details by key without flattening errors into strings.
- [ ] Use detail keys for recovery-sensitive registration errors:
  - [ ] staging path
  - [ ] journal path
  - [ ] asset IDs
  - [ ] orphan final paths
  - [ ] comparison role
  - [ ] page/scan IDs
- [ ] Keep user-facing localization separate from developer diagnostics.

### Suggested FFI shape

```rust
#[derive(Clone, Debug, uniffi::Record)]
pub struct A2dFfiErrorDetail {
    pub key: String,
    pub value: String,
}
```

### Tests

- [ ] Rust-to-FFI mapping preserves zero details.
- [ ] Mapping preserves multiple details in deterministic order.
- [ ] A scan-registration recovery error exposes its staging/journal fields to Kotlin.
- [ ] A scan-comparison error exposes `comparison_role` and IDs.
- [ ] Kotlin and Swift generated bindings contain the detail type.
- [ ] Redaction tests prove prohibited values are not added by representative producers.

### Acceptance criteria

- [ ] Android receives all nonsecret recovery details intentionally attached by Rust.

---

## FIX-041 — Make ID generation fallible

**Priority:** P0  
**Primary paths:**

- `crates/a2d-domain/src/id.rs`
- all production call sites of `::generate()`
- `crates/a2d-core/src/`
- `crates/a2d-ffi/src/`

### Tasks

- [ ] Replace `getrandom(...).expect(...)` in production ID generation.
- [ ] Introduce a typed randomness-source error.
- [ ] Decide whether public ID generation returns `Result<Id, A2dError>` or uses an injected fallible generator owned by core.
- [ ] Update every production ID minting call site.
- [ ] Keep deterministic test generation available only through test interfaces.
- [ ] Do not fall back to timestamps, counters, weak PRNGs, all-zero IDs, or reused values.
- [ ] Ensure a failure to generate an ID prevents any partial database/file mutation.
- [ ] Ensure FFI methods return the typed error rather than panic.

### Tests

- [ ] Inject RNG failure before notebook creation; verify no rows are written.
- [ ] Inject RNG failure before page-set generation; verify no file or rows are committed.
- [ ] Inject RNG failure before scan registration; verify staging remains recoverable.
- [ ] FFI maps RNG failure as an error.
- [ ] Existing canonical encoding vectors remain unchanged.

---

## FIX-042 — Make correlation-ID generation failure-safe

**Priority:** P0

### Tasks

- [ ] Ensure constructing an error cannot panic because correlation-ID generation failed.
- [ ] Add a nonsecret emergency correlation representation for RNG failure.
- [ ] The emergency representation must:
  - [ ] be visibly noncanonical as an ordinary entity ID
  - [ ] avoid pretending uniqueness is guaranteed
  - [ ] include no user content
  - [ ] remain stable within the returned error
- [ ] Avoid recursively constructing an `A2dError` while trying to construct an `A2dError`.
- [ ] Document the emergency correlation behavior.

### Tests

- [ ] Simulated RNG failure while constructing another error returns a stable envelope without panic.
- [ ] The emergency ID is distinguishable from a normal 26-character entity ID.

---

## FIX-043 — Remove production panic-test exports

**Priority:** P0  
**Primary paths:**

- `crates/a2d-ffi/src/lib.rs`
- Android instrumentation panic tests
- native symbol verification tooling

### Tasks

- [ ] Put `trigger_panic_for_testing` behind an explicit test/debug Cargo feature.
- [ ] Ensure release Android native libraries do not export the method/symbol.
- [ ] Keep panic-containment validation through a dedicated test artifact or debug-only build.
- [ ] Update instrumentation tests to build against the test feature only.
- [ ] Add APK/native-symbol verification that release artifacts omit the test panic entry point.

### Acceptance criteria

- [ ] Production users cannot call an intentional Rust panic API.
- [ ] Panic containment remains tested.

---

## FIX-044 — Replace silent timestamp zero fallback

**Priority:** P1  
**Primary paths:**

- `crates/a2d-storage/src/lib.rs`
- `crates/a2d-storage/src/assets.rs`
- `crates/a2d-core/src/`
- any duplicated `now_ms()` implementation

### Tasks

- [ ] Introduce one shared fallible portable clock abstraction.
- [ ] Replace every `duration_since(...).unwrap_or(0)` production path.
- [ ] Use checked conversion from `u128` milliseconds to `i64`.
- [ ] Return a typed time-source or overflow error.
- [ ] Inject the clock into tests that require deterministic timestamps.
- [ ] Ensure clock failure rolls back transactions and leaves staged files recoverable.
- [ ] Do not use local Android time as the canonical fallback.

### Tests

- [ ] Pre-epoch/system clock failure produces an explicit error.
- [ ] Timestamp overflow produces an explicit error.
- [ ] No record is persisted with an invented zero timestamp.

---

# Phase 6 — Rust-owned resource limits and manifest validation

## FIX-050 — Move Smart Page generation limits into Rust

**Priority:** P0  
**Primary paths:**

- `crates/a2d-core/src/milestone6.rs`
- `crates/a2d-pdf/src/generate.rs`
- `crates/a2d-ffi/src/milestone6.rs`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/`

### Tasks

- [ ] Define a versioned Rust-owned generation policy.
- [ ] Enforce a maximum page count in Rust before allocation.
- [ ] Enforce maximum resulting PDF page count, accounting for any recto/verso expansion.
- [ ] Enforce maximum starting visible page.
- [ ] Use checked addition for `starting_visible_page + offset`.
- [ ] Validate the final visible page number remains within the QR wire-format maximum.
- [ ] Use checked capacity conversions before allocating vectors.
- [ ] Reject requests whose estimated output exceeds explicit memory/byte limits where a safe estimate is possible.
- [ ] Keep Kotlin validation for immediate UX feedback, but treat it as presentation-only duplication of values obtained from Rust.
- [ ] Expose the Rust limits to Android or provide a typed validation API.
- [ ] Do not let direct FFI callers bypass the same limits.

### Tests

- [ ] Zero pages fails.
- [ ] Maximum allowed page count succeeds.
- [ ] Maximum plus one fails before allocation.
- [ ] Visible-page addition overflow fails explicitly.
- [ ] QR maximum numeric field is enforced.
- [ ] Direct FFI requests receive the same validation as Android UI requests.
- [ ] A maliciously large request cannot cause a process abort through vector allocation.

---

## FIX-051 — Harden Notebook Design manifest parsing

**Priority:** P1  
**Primary paths:**

- `crates/a2d-layout/src/manifest.rs`
- `crates/a2d-layout/manifests/`
- `crates/a2d-domain/src/entities.rs`

### Input and resource limits

- [ ] Add a maximum manifest byte length before JSON parsing.
- [ ] Reject excessive strings and collections.
- [ ] Set a maximum marker-role count.
- [ ] Set a reviewed maximum logical page count.
- [ ] Set supported trim-dimension ranges.
- [ ] Reject zero and implausible dimensions.

### Semantic validation

- [ ] Reject empty or whitespace-only design names.
- [ ] Reject empty marker family.
- [ ] Reject unsupported marker family for required v0.1 manifests.
- [ ] Reject duplicate marker roles.
- [ ] Validate marker-role grammar and required semantic role set.
- [ ] Verify setup and page layout IDs exist in the selected registry.
- [ ] Verify manifest trim dimensions agree with resolved layouts within an explicit exact/tolerance rule.
- [ ] Reject identical setup and page layout IDs if the product format requires them to differ.
- [ ] Validate `design_version > 0` if version zero is not meaningful.
- [ ] Reject unknown required fields/version according to the documented compatibility policy.

### Hashing and canonicalization

- [ ] Decide whether `manifest_hash` identifies exact source bytes or canonical semantic content.
- [ ] Record the decision in the manifest format documentation.
- [ ] If semantic content is intended:
  - [ ] define canonical JSON serialization
  - [ ] hash canonical bytes
  - [ ] add key-order and whitespace equivalence tests
- [ ] If exact bytes are intended:
  - [ ] rename/document the field accordingly
  - [ ] explain that whitespace changes create a new content hash

### Official versus development manifests

- [ ] Keep `dev-placeholder.json` visibly nonproduction.
- [ ] Prevent a release build from treating a development placeholder as an official product design unless an explicit release gate permits it.
- [ ] Add a build/test assertion distinguishing development and official manifests.
- [ ] Do not mark “bundle initial official manifests” complete until a reviewed official design exists.

### Tests

- [ ] Oversized manifest.
- [ ] Duplicate roles.
- [ ] Unsupported marker family.
- [ ] Missing referenced layout.
- [ ] Trim/layout mismatch.
- [ ] Excessive logical page count.
- [ ] Empty name.
- [ ] Unsupported schema version.
- [ ] Exact/canonical hash behavior according to the selected policy.

---

## FIX-052 — Propagate registry construction failures

**Priority:** P1  
**Primary path:** `crates/a2d-core/src/milestone6.rs`

### Tasks

- [ ] Remove `.ok()` conversions that turn `bundled_placeholder_registry()` failure into absence.
- [ ] Propagate manifest/registry corruption as a typed integrity/internal error.
- [ ] Reserve `UnsupportedCode` for genuinely valid but unsupported user data.
- [ ] Include design/layout IDs in nonsecret details.
- [ ] Audit the repository for similar `Result::ok`, `unwrap_or_default`, `unwrap_or(false)`, and empty-collection fallbacks in canonical paths.
- [ ] Fix every confirmed failure-erasing conversion found by the audit.

### Tests

- [ ] Inject a broken bundled registry and verify page resolution returns an error, not `UnsupportedCode`.
- [ ] A genuinely unknown design still returns the documented unsupported result.

---

# Phase 7 — Scan comparison integrity

## FIX-060 — Verify corrected assets before conclusive comparison

**Priority:** P0  
**Primary paths:**

- `crates/a2d-core/src/scan_comparison.rs`
- `crates/a2d-storage/src/assets.rs`
- `crates/a2d-storage/src/repository.rs`
- `crates/a2d-image/src/content_comparison.rs`

### Tasks

- [ ] Load each scan’s corrected asset record.
- [ ] Require a corrected asset for any comparison mode that claims corrected-content evidence.
- [ ] Verify the corrected asset file exists.
- [ ] Verify file byte length and SHA-256 against the asset record.
- [ ] Verify the fingerprint’s embedded corrected SHA-256 agrees with the asset record.
- [ ] Decide whether perceptual fingerprint bytes must be recomputed from the corrected/OCR image for a conclusive result.
- [ ] At minimum, do not call metadata-only equality `ConclusiveExactMatch` unless the underlying files have been verified.
- [ ] If full recomputation is deferred, rename confidence/reason codes so they explicitly describe verified stored fingerprint equality rather than actual image equality.
- [ ] Return degraded/inconclusive evidence when required assets cannot be verified.
- [ ] Do not silently skip verification because files are missing.
- [ ] Include verification status for baseline and candidate in the result.

### Suggested result additions

```rust
pub enum StoredScanAssetVerification {
    Verified,
    Missing,
    HashMismatch,
    Unavailable,
}
```

### Tests

- [ ] Identical verified assets produce the conclusive exact result.
- [ ] Matching fingerprint metadata with a missing file does not produce conclusive exact match.
- [ ] Matching fingerprint metadata with tampered bytes does not produce conclusive exact match.
- [ ] Fingerprint hash disagreeing with asset-row hash is a typed integrity error.
- [ ] Baseline and candidate verification status cross FFI.

---

## FIX-061 — Finish the Milestone 9.2 Android/FFI consumption path without inventing classification

**Priority:** P1

### Tasks

- [ ] Regenerate bindings as required by FIX-001.
- [ ] Add a thin Android rustbridge wrapper for stored-scan comparison if direct generated imports are not the project convention.
- [ ] Preserve raw evidence:
  - [ ] scan IDs
  - [ ] page ID
  - [ ] pipeline versions
  - [ ] quality status
  - [ ] physical-copy metadata
  - [ ] exact hash result
  - [ ] changed cells/regions
  - [ ] reasons
  - [ ] confidence availability
- [ ] Do not add duplicate/revision labels before photographed calibration.
- [ ] Add unit tests for Kotlin mapping and unavailable-confidence presentation.
- [ ] Keep visual comparison UI in Milestone 9.5 unless explicitly pulled forward.

---

# Phase 8 — Calibration honesty and quality-state semantics

## FIX-070 — Separate measured metrics from calibrated production classification

**Priority:** P1  
**Primary paths:**

- `crates/a2d-core/src/milestone9.rs`
- `crates/a2d-image/src/quality.rs`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerPolicy.kt`
- registration result types

### Tasks

- [ ] Inventory every threshold currently used for:
  - [ ] live guidance
  - [ ] manual capture warning
  - [ ] automatic capture
  - [ ] durable scan quality status
  - [ ] Needs Review creation
- [ ] Mark each threshold as:
  - [ ] presentation-only provisional
  - [ ] synthetic-fixture regression value
  - [ ] physically calibrated production value
- [ ] Keep automatic capture disabled until required physical evidence exists.
- [ ] Prevent provisional values from producing an unqualified production “accepted quality” claim.
- [ ] Add an explicit calibration state to results, such as:
  - [ ] `Calibrated`
  - [ ] `Provisional`
  - [ ] `Unavailable`
- [ ] Preserve raw metrics regardless of calibration state.
- [ ] Use explicit warnings such as `QUALITY_THRESHOLDS_UNCALIBRATED` where appropriate.
- [ ] Do not block durable original preservation solely because calibration is unavailable.
- [ ] Define how uncalibrated quality affects preferred-scan selection and review state.

### Tests

- [ ] Uncalibrated policy never enables automatic capture.
- [ ] Registration preserves metrics and reports provisional status.
- [ ] The UI does not label provisional classification as calibrated.
- [ ] Future calibrated policy can be versioned without changing stored originals.

---

# Phase 9 — Kotlin coroutine, lifecycle, and temporary-file correctness

## FIX-080 — Stop swallowing coroutine cancellation

**Priority:** P1  
**Primary paths:**

- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/notebook/NotebookViewModel.kt`
- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/SmartPagesViewModel.kt`
- other Android ViewModels using `runCatching` around suspend calls

### Tasks

- [ ] Replace broad suspend `runCatching` patterns or explicitly rethrow `CancellationException`.
- [ ] Audit every `catch (Exception)` and `runCatching` in coroutine code.
- [ ] Ensure cancellation:
  - [ ] does not publish an ordinary error
  - [ ] does not retry
  - [ ] does not reset a successful prior result incorrectly
  - [ ] does not perform post-cancellation state mutation
- [ ] Use `try/finally` for busy-state cleanup where appropriate.
- [ ] Preserve the last stable UI state when an operation is cancelled by lifecycle destruction.

### Tests

- [ ] Cancelling notebook refresh does not display an error.
- [ ] Cancelling Smart Page generation does not display generation failure.
- [ ] Busy state clears or the ViewModel is destroyed without stale mutation.
- [ ] Real failures still display explicit errors.

---

## FIX-081 — Make QR capture state recreation-safe

**Priority:** P1  
**Primary path:** `apps/android/app/src/main/kotlin/com/a2d/notebook/rustbridge/QrCapture.kt`

### Tasks

- [ ] Move pending capture identity/path out of ephemeral Compose `remember` state.
- [ ] Use a ViewModel or saved-state-backed token suitable for Activity recreation.
- [ ] On recreation, recover a pending capture only if:
  - [ ] the path remains inside the approved cache/staging root
  - [ ] the file exists and is regular
  - [ ] the token matches the pending platform request
- [ ] Report a missing/stale pending capture explicitly.
- [ ] Replace ignored `delete()` results with visible cleanup warnings where appropriate.
- [ ] Add bounded orphan detection for abandoned QR capture files.
- [ ] Never decode a stale file from a previous capture token.

### Tests

- [ ] Recreation while camera app is open preserves the pending capture.
- [ ] Recreation with a missing file reports failure explicitly.
- [ ] Stale callback token is ignored without deleting a newer capture.
- [ ] Failed cleanup is surfaced.

---

## FIX-082 — Make Smart Page save/preview state recreation-safe

**Priority:** P1  
**Primary paths:**

- `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/SmartPagesScreen.kt`
- `SmartPagesViewModel.kt`
- `PdfPlatformActions.kt` or equivalent

### Tasks

- [ ] Move `pendingSavePath` and save-operation token into ViewModel/SavedStateHandle state.
- [ ] Preserve the pending generated asset ID rather than trusting an arbitrary path alone.
- [ ] Re-resolve the asset path through Rust or a library-owned validated path before copying.
- [ ] On document-picker return after recreation, either complete the exact pending save or show an explicit stale-operation error.
- [ ] Replace preview `.getOrNull()` with explicit preview state:
  - [ ] loading
  - [ ] ready
  - [ ] failed with message/correlation context
- [ ] Recycle or otherwise release replaced preview bitmaps.
- [ ] Cancel old preview rendering when a new generated PDF replaces it.
- [ ] Verify the generated file before share/print/save.
- [ ] Preserve platform copy failure details without exposing private internal paths to normal UI.

### Tests

- [ ] Recreation during `CreateDocument` preserves the operation.
- [ ] Missing generated PDF produces an explicit error.
- [ ] Preview decode failure is visible and does not prevent save/share if the PDF itself remains valid.
- [ ] Repeated generation does not retain all prior bitmaps.
- [ ] Stale picker result cannot save a newer PDF under an older token.

---

## FIX-083 — Preserve cleanup failure in CameraX terminal state

**Priority:** P2  
**Primary path:** `CameraXAdapter.kt`

### Tasks

- [ ] Do not publish `Error` and immediately overwrite it with an indistinguishable `Closed` state.
- [ ] Choose one explicit representation:
  - [ ] `Closed(cleanupWarning = ...)`
  - [ ] separate durable event channel
  - [ ] state plus retained diagnostic field
- [ ] Keep adapter closure idempotent.
- [ ] Preserve cleanup failure cause for diagnostics.
- [ ] Do not prevent closure solely because cleanup reporting failed.

### Tests

- [ ] Cleanup failure remains observable after `close()`.
- [ ] Successful close has no warning.
- [ ] Repeated close does not duplicate cleanup effects.

---

# Phase 10 — PDF output hardening

## FIX-090 — Harden standalone PDF write-and-verify behavior

**Priority:** P1  
**Primary path:** `crates/a2d-pdf/src/generate.rs`

### Tasks

- [ ] Replace the predictable shared `.pdf.tmp` path with a unique temp file in the destination directory.
- [ ] Prevent concurrent generators from sharing or overwriting the same temp path.
- [ ] Flush and `sync_all()` the temporary PDF before verification/finalization.
- [ ] Treat parser warnings according to a documented policy:
  - [ ] reject warnings that imply malformed output
  - [ ] return nonfatal warnings explicitly if allowed
- [ ] Use no-replace finalization by default.
- [ ] If replacement is a supported explicit operation, require a separate API and explicit caller intent.
- [ ] Synchronize the destination directory according to the documented durability model.
- [ ] Report failed temp cleanup explicitly.
- [ ] Preserve a failed temp file only when it is intentionally needed for diagnostics/recovery, and return its path as structured detail.
- [ ] Add output byte/page limits before generating very large PDFs.

### Tests

- [ ] Existing destination is not replaced by the create-new API.
- [ ] Concurrent generation uses independent temp files.
- [ ] Corrupt output is rejected and cleanup behavior is explicit.
- [ ] Parser warning policy is tested.
- [ ] Directory-sync and cleanup failures are represented.

---

# Phase 11 — Migration identity and schema integrity

## FIX-100 — Record and verify migration content digests

**Priority:** P1  
**Primary paths:**

- `crates/a2d-storage/src/migrations.rs`
- `crates/a2d-storage/src/lib.rs`
- a new numbered migration if the metadata table must change

### Tasks

- [ ] Add a stable SHA-256 digest to each compiled migration descriptor.
- [ ] Record the digest in migration history for new databases.
- [ ] Add a forward migration for existing `schema_migrations` tables if necessary.
- [ ] On open, verify version, name, and content digest.
- [ ] Define how legacy rows without a digest are upgraded:
  - [ ] verify against the known shipped migration identity
  - [ ] do not silently trust arbitrary content
- [ ] Return a critical integrity error on digest mismatch.
- [ ] Include version, recorded digest, and expected digest in structured details.
- [ ] Keep migration SQL immutable after release.

### Tests

- [ ] Fresh database records all migration digests.
- [ ] Reopen with matching digests succeeds.
- [ ] Same version/name with modified SQL digest fails closed.
- [ ] Legacy database upgrades without losing original applied timestamps.
- [ ] A failed digest upgrade leaves the existing database usable and unchanged.

---

## FIX-101 — Add a non-destructive canonical-state integrity checker

**Priority:** P1  
**Primary paths:**

- `crates/a2d-storage/src/`
- `crates/a2d-core/src/`
- future Android diagnostics UI only after Rust API is complete

### Checks

- [ ] `PRAGMA foreign_key_check`.
- [ ] Schema version/name/digest verification.
- [ ] Referenced asset existence.
- [ ] Optional full asset hash verification.
- [ ] Orphan temp files.
- [ ] Orphan finalized asset files.
- [ ] Preferred-scan consistency across page and scan tables.
- [ ] At most one active Notebook.
- [ ] Page kind column consistency.
- [ ] Scan original immutability.
- [ ] Stored fingerprint format validity.
- [ ] Generated PDF asset references.
- [ ] Future search-index consistency hook without pretending search exists now.

### Behavior

- [ ] Return a structured report with findings and severity.
- [ ] Do not repair or delete anything automatically.
- [ ] Include stable finding codes and affected IDs.
- [ ] Bound filesystem traversal and hashing work.
- [ ] Support cancellation.

### Tests

- [ ] Clean library report.
- [ ] Missing asset.
- [ ] Hash mismatch.
- [ ] Orphan final asset.
- [ ] Preferred-scan contradiction.
- [ ] Foreign-key violation fixture.
- [ ] Cancellation returns a distinct outcome.

---

# Phase 12 — Scanner recovery and camera test gaps

## FIX-110 — Add process-death-safe scanner staging recovery

**Priority:** P1  
**Primary paths:**

- scan registration journal code
- Android scanner ViewModel/SavedStateHandle
- Rust core recovery APIs

### Tasks

- [ ] Define a Rust-owned scanner staging/recovery record.
- [ ] Persist enough information before external capture/processing to identify:
  - [ ] staging token
  - [ ] file path under the private root
  - [ ] fixed page ID
  - [ ] fixed Notebook ID when applicable
  - [ ] capture timestamp
  - [ ] processing/registration phase
  - [ ] policy version
- [ ] On app restart, list recoverable scanner operations.
- [ ] Never auto-register a recovered file without explicit revalidation.
- [ ] Offer retry, review, or explicit discard.
- [ ] Discard only the selected unregistered staging file.
- [ ] Make retry idempotent and resistant to duplicate registration.
- [ ] Reconcile filesystem journals and SQLite state before claiming recovery success.

### Tests

- [ ] Process death after capture before preview.
- [ ] Process death after preview before registration.
- [ ] Process death during asset finalization.
- [ ] Process death after DB commit before Android receives success.
- [ ] Restart does not create a duplicate scan.
- [ ] User discard never deletes committed originals.

---

## FIX-111 — Complete the Milestone 8.6 camera failure matrix

**Priority:** P1

### Tasks

- [ ] Permission denied.
- [ ] Permission permanently denied.
- [ ] Camera unavailable/bind failure.
- [ ] Background during capture.
- [ ] Process killed after capture before registration.
- [ ] Rotation during analysis.
- [ ] Rapid repeated manual capture.
- [ ] Rapid repeated automatic-capture callback.
- [ ] Wrong-design page.
- [ ] Two identical Notebook candidates.
- [ ] Low-storage staging failure.
- [ ] Low-storage asset finalization failure.
- [ ] Stale CameraX callback after rebind.
- [ ] Torch failure and unavailable flash.
- [ ] Cleanup failure.

### Requirements

- [ ] For every case, specify the exact expected state-machine phase.
- [ ] Specify which files must exist afterward.
- [ ] Specify which files must not be deleted.
- [ ] Specify whether retry is allowed.
- [ ] Specify the user-visible warning/error.
- [ ] “The app did not crash” is not sufficient acceptance evidence.

---

# Phase 13 — Main roadmap reconciliation

## FIX-120 — Correct `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

**Priority:** P0  
**Primary path:** `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

### Header and status

- [ ] Replace the blanket “Milestones 1–6 complete” status with an accurate summary.
- [ ] Distinguish:
  - [ ] implementation complete
  - [ ] partial
  - [ ] physical evidence pending
  - [ ] reopened by remediation
  - [ ] not implemented
- [ ] Do not claim current CI success until the exact remediation head passes permanent CI.

### Milestone 1 corrections

- [ ] Mark binding generation/drift complete only after FIX-001 passes on current head.
- [ ] Correct the checked-in binding policy text.
- [ ] Remove or rewrite references to obsolete one-use validation workflows.

### Milestone 2 corrections

- [ ] Reopen FFI-safe error-envelope completion until details cross UniFFI.
- [ ] Reopen panic handling until production panic exports are removed and ID generation is fallible.
- [ ] Reopen preferred-scan invariant until FIX-010 is complete.
- [ ] Keep redaction work incomplete.

### Milestone 3 corrections

- [ ] Reopen durable asset creation until file/directory synchronization and no-replace finalization exist.
- [ ] Reopen preferred-scan repository integrity.
- [ ] Keep rename-before-DB interruption and disk-full tests incomplete until implemented.
- [ ] Record migration digest verification as required hardening.

### Milestone 4 corrections

- [ ] Keep official manifests incomplete.
- [ ] State clearly that the current bundled manifest is a development placeholder.
- [ ] Reopen manifest validation completeness until FIX-051 passes.
- [ ] Reopen random ID handling until RNG failure is typed.

### Milestone 5 corrections

- [ ] Keep physical print/photo acceptance incomplete.
- [ ] Qualify standalone PDF durability claims until FIX-090 is complete.
- [ ] Record Rust-owned PDF/page-count limits.

### Milestone 6 corrections

- [ ] Change overall status from unconditional complete to partial/remediation required.
- [ ] Record Kotlin cancellation and recreation gaps.
- [ ] Record Rust-owned Smart Page resource-limit requirement.
- [ ] Record registry failure propagation requirement.

### Milestone 7 corrections

- [ ] Preserve the distinction between deterministic synthetic fixtures and photographed/device evidence.
- [ ] Do not mark ADR 0002 Accepted before the documented evidence exists.
- [ ] Record provisional versus calibrated threshold semantics.

### Milestone 8 corrections

- [ ] Reclassify 8.4 as partial until layout-driven registration and the Smart Page scope decision are complete.
- [ ] Reconcile 8.6 item by item; mark only genuinely covered cases complete.
- [ ] Keep batch scanner incomplete.

### Milestone 9 corrections

- [ ] Mark aligned change-region comparison complete.
- [ ] Mark confidence and reason reporting complete.
- [ ] Record the stored-scan core and UniFFI comparison APIs.
- [ ] Keep fixture-based threshold tuning incomplete.
- [ ] Record asset verification as required before conclusive comparison.
- [ ] Reopen final registration’s full completion until durability and layout defects are fixed.
- [ ] Keep 9.3–9.5 incomplete.

### Milestones 10–14

- [ ] Keep Library, OCR, Search, Backup, Export, Model, and Skills tasks incomplete.
- [ ] Do not mark module-level placeholder comments as implementations.
- [ ] Add dependency notes where remediation tasks are prerequisites.

### Milestone 15

- [ ] Accurately record Swift generation smoke coverage.
- [ ] Keep the Swift harness, XCFramework packaging, error mapping review, and adapter inventory incomplete.

### Milestone 16

- [ ] Mark already implemented input/dependency hardening bullets individually.
- [ ] Keep diagnostics, redaction, integrity report, and unimplemented failure injection incomplete.
- [ ] Add preferred-scan, asset-orphan, and migration-digest checks.

### Milestones 17–19

- [ ] Keep physical and manual acceptance work incomplete.
- [ ] Do not count synthetic raster tests as physical proof.
- [ ] Keep release validation incomplete until every required product workflow exists.

### Acceptance criteria

- [ ] Every checked item in the main TODO has production code and evidence.
- [ ] Every known limitation is visible rather than hidden inside prose attached to a checked box.
- [ ] No section simultaneously claims the same task complete and incomplete.

---

## FIX-121 — Add a stable remediation traceability table

**Priority:** P1

### Tasks

- [ ] Add a short table to the main TODO or this file mapping each fix ID to:
  - [ ] affected milestone
  - [ ] primary code paths
  - [ ] test paths
  - [ ] completion commit
  - [ ] validation run
- [ ] Do not reference review files that are not committed.
- [ ] Update the table as fixes land.

---

# Phase 14 — CI and regression enforcement

## FIX-130 — Add permanent regression checks for the repaired invariants

**Priority:** P0  
**Primary path:** `.github/workflows/ci.yml`

### Tasks

- [ ] Keep all checks in the permanent workflow.
- [ ] Do not create temporary validation workflows.
- [ ] Ensure CI runs:
  - [ ] Rust formatting
  - [ ] clippy with warnings denied
  - [ ] full Rust tests
  - [ ] dependency/license policy
  - [ ] Kotlin binding drift
  - [ ] Swift generation smoke test
  - [ ] Android lint
  - [ ] Android JVM tests
  - [ ] Android debug assembly
  - [ ] required native ABIs
  - [ ] APK symbol/notices verification
  - [ ] emulator integration where supported
- [ ] Add focused checks for:
  - [ ] preferred-scan migration/invariant tests
  - [ ] no-replace asset collision test
  - [ ] asset verification before conclusive scan comparison
  - [ ] manifest-limit tests
  - [ ] Rust Smart Page request-limit tests
  - [ ] release artifact omission of panic-test exports
- [ ] Keep photographed fixtures optional until legally/privacy-safe fixtures exist, but fail clearly when a configured fixture manifest references missing files.

### Acceptance criteria

- [ ] Deliberate regression of each repaired invariant fails permanent CI.
- [ ] CI does not rely on a developer’s uncommitted generated files.

---

## FIX-131 — Verify the exact final `master` head

**Priority:** P0

### Tasks

- [ ] Run focused tests after each phase.
- [ ] Run the full permanent CI workflow on the final remediation head.
- [ ] Fetch every job result rather than relying on the workflow summary alone.
- [ ] Confirm no Rust tests were skipped due to an earlier formatting/build failure.
- [ ] Confirm binding drift is green.
- [ ] Confirm both Android ABIs package the expected production symbols and omit debug-only panic symbols.
- [ ] Record the exact commit SHA and workflow run in this TODO and the main roadmap.

---

# Phase 15 — Additional static audit prompted by the confirmed defect patterns

## FIX-140 — Audit failure-erasing Rust conversions

**Priority:** P1

### Search patterns

- [ ] `.ok()`
- [ ] `.ok_or(...)` used after discarding a richer error
- [ ] `.unwrap_or_default()`
- [ ] `.unwrap_or(false)`
- [ ] `.unwrap_or(0)`
- [ ] `unwrap_or_else` returning empty/default success values
- [ ] `.unwrap()` and `.expect()` in non-test production paths
- [ ] ignored `Result` values
- [ ] `let _ =` on fallible production operations

### Tasks

- [ ] Classify every match as safe, test-only, invariant panic, or defect.
- [ ] Replace every defect with typed propagation or explicit warning.
- [ ] Document any remaining invariant panic and prove untrusted input cannot reach it.
- [ ] Prefer returning an internal error over crashing at an FFI boundary.

### Acceptance criteria

- [ ] No production result is silently converted into successful absence/default behavior.

---

## FIX-141 — Audit ignored filesystem cleanup results

**Priority:** P1

### Tasks

- [ ] Search for `.delete()`, `remove_file`, `remove_dir_all`, and permission changes whose result is ignored.
- [ ] Separate harmless test teardown from production cleanup.
- [ ] Surface production cleanup failures as warnings/errors with affected path and ownership scope.
- [ ] Never leak private absolute paths into ordinary localized UI text.
- [ ] Keep details available in structured diagnostics.

---

## FIX-142 — Audit arithmetic and allocation boundaries

**Priority:** P1

### Tasks

- [ ] Search for unchecked addition/multiplication involving:
  - [ ] page counts
  - [ ] visible page numbers
  - [ ] image dimensions
  - [ ] byte lengths
  - [ ] PDF page counts
  - [ ] vector capacities
  - [ ] timestamp conversion
- [ ] Replace untrusted-input arithmetic with checked operations.
- [ ] Convert integer widths with `try_from` rather than `as` where overflow matters.
- [ ] Add upper bounds before allocating from FFI/user-controlled counts.

### Acceptance criteria

- [ ] Maliciously large requests return typed validation/resource errors rather than panic or abort.

---

# Phase 16 — Documentation and handoff quality

## FIX-150 — Update architectural documentation after implementation

**Priority:** P1

### Tasks

- [ ] Document the atomic preferred-scan workflow.
- [ ] Document asset no-replace and fsync behavior.
- [ ] Document orphan final-asset detection and non-destructive recovery.
- [ ] Document layout-driven scan processing.
- [ ] Document FFI error details.
- [ ] Document fallible ID and clock behavior.
- [ ] Document Rust-owned Smart Page limits.
- [ ] Document verified versus metadata-only scan-comparison semantics.
- [ ] Document provisional versus calibrated quality status.
- [ ] Document scanner process-death recovery.
- [ ] Document the generated binding policy.

### Acceptance criteria

- [ ] Code comments, spec, TODO, README, and CI behavior agree.

---

## FIX-151 — Verify every referenced file exists

**Priority:** P0

### Tasks

- [ ] Check every path named in this TODO.
- [ ] Check every path named in `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`.
- [ ] Remove or correct stale references.
- [ ] Do not reference an assistant-created review, response, template, or evidence file unless it is committed at that exact path.
- [ ] Add a script or test that validates repository-local Markdown backtick paths where practical.

---

# Explicitly out of scope for this remediation pass

The following remain roadmap work unless a fix above requires a narrow prerequisite:

- [ ] Batch scanner feature implementation beyond recovery/invariant prerequisites.
- [ ] Full Needs Review UI and resolution system.
- [ ] Version timeline and visual comparison UI.
- [ ] Library hub and page viewer.
- [ ] OCR provider and correction UI.
- [ ] Local FTS search.
- [ ] `.atnb` backup/restore.
- [ ] General exporters.
- [ ] Model-provider integrations.
- [ ] Skill runtime and permissions.
- [ ] Production iOS application.
- [ ] Physical printer/KDP validation itself.

These must remain unchecked in the main roadmap. Do not create placeholder success paths merely to satisfy this remediation file.

---

# Recommended implementation order

1. [ ] FIX-001 and FIX-002 — restore binding consistency.
2. [ ] FIX-010 and FIX-011 — close preferred-scan corruption paths.
3. [ ] FIX-020 through FIX-024 — close asset durability, collision, cleanup, and path defects.
4. [ ] FIX-030 and FIX-031 — make scan processing layout- and policy-driven.
5. [ ] FIX-040 through FIX-044 — preserve error context and eliminate ordinary production panic/default-time paths.
6. [ ] FIX-050 through FIX-052 — enforce Rust-owned limits and manifest/registry correctness.
7. [ ] FIX-060 and FIX-061 — make scan comparison evidence asset-backed and consumable.
8. [ ] FIX-070 — make calibration status honest.
9. [ ] FIX-080 through FIX-083 — repair Android cancellation, recreation, cleanup, and terminal-state behavior.
10. [ ] FIX-090 — harden standalone PDF output.
11. [ ] FIX-100 and FIX-101 — add migration digests and integrity reporting.
12. [ ] FIX-110 and FIX-111 — complete scanner recovery and failure tests.
13. [ ] FIX-120 and FIX-121 — reconcile the authoritative roadmap.
14. [ ] FIX-130 and FIX-131 — enforce and verify everything in permanent CI.
15. [ ] FIX-140 through FIX-151 — finish systematic audits and documentation.

---

# Final remediation acceptance checklist

- [ ] Current Kotlin bindings exactly match the Rust UniFFI surface.
- [ ] Preferred-scan state cannot become contradictory.
- [ ] Asset finalization cannot replace an existing asset.
- [ ] Successful asset commits meet the documented durability contract.
- [ ] Post-finalization failures are recoverable and visible.
- [ ] Asset path resolution returns the validated canonical path or handle.
- [ ] Scan preview and registration resolve the actual page layout.
- [ ] Preview and registration use one Rust-owned policy version.
- [ ] Smart Page scanner scope is explicit and tested.
- [ ] `A2dError.details` reaches Kotlin and Swift bindings.
- [ ] Production ID and correlation handling do not panic on RNG failure.
- [ ] Release artifacts do not expose intentional panic-test APIs.
- [ ] Clock failure never silently creates timestamp zero.
- [ ] Smart Page generation limits are enforced in Rust.
- [ ] Manifest parsing has explicit resource and semantic limits.
- [ ] Registry corruption is not converted into unsupported user data.
- [ ] Conclusive scan comparison verifies underlying corrected assets.
- [ ] Uncalibrated thresholds are labeled provisional and cannot enable automatic capture.
- [ ] Kotlin coroutine cancellation is never presented as an ordinary failure.
- [ ] Pending capture/save operations survive or explicitly reject recreation.
- [ ] PDF output does not silently replace existing files.
- [ ] Migration content changes are detected by digest.
- [ ] A non-destructive library integrity report exists.
- [ ] Scanner process-death recovery does not duplicate registration.
- [ ] Camera failure tests define exact expected state and data outcomes.
- [ ] `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` accurately matches implementation state.
- [ ] Permanent CI is green for the exact final `master` commit.
- [ ] Every referenced repository file exists at the exact path named.
- [ ] No original scan or user data is silently deleted, overwritten, or hidden by a fallback.
