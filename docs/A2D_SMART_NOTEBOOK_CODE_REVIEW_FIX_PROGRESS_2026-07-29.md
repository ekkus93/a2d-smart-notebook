# A2D Smart Notebook Code Review Fix Progress — 2026-07-29

## Scope

- Repository: `ekkus93/a2d-smart-notebook`
- Branch: `master`
- Source plan: `docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md`
- Head at the start of this progress record: `afc7218ab16a630b933a506862bd317efc4ae3ba`

This file records what was implemented during the current Ralph Loop pass, what was validated, and what remains open. It is intentionally separate from the source TODO because the permanent CI/toolchain gates could not be run from the execution environment used for this pass.

## Landed work

### 1. Post-finalization asset recovery and durability

The storage layer now has a compiled and exported non-destructive orphan-final-asset discovery API.

Implemented behavior includes:

- A shared `AssetPersistenceFailureStage` vocabulary for filesystem/SQLite boundary failures.
- Typed orphan reports containing:
  - `AssetId`
  - asset kind
  - final relative path
  - byte length
  - SHA-256
- Canonical path and directory checks.
- Rejection of symlinks and non-regular files in canonical final-asset directories.
- Fail-closed handling of invalid/noncanonical asset filenames.
- No automatic deletion or import of unknown final files.
- Explicit failure when required directory synchronization is unsupported.
- Finalized-file metadata verification before success.
- Complete recovery evidence on page-set database rollback after a PDF was durably finalized.
- Regression tests that verify final files remain present and discoverable after rollback.

Representative commits:

- `7b2c4e1` — wire orphan asset recovery module
- `ebd6f83` — type asset recovery stages and orphan IDs
- `894abc0` — validate orphan final asset identities
- `f777098` — enrich asset finalization failures
- `d1e626a` — pin recovery details in collision tests
- `cee7711` — report page-set rollback recovery evidence
- `c322ef5` — cover outside-target asset symlinks

### 2. Rust-owned scan-layout resolution policy

A new Rust-owned layout resolver now maps canonical stored page state to the physical and processing policy used by durable scan registration.

Implemented behavior includes:

- Explicit v1 printable marker family: `tagStandard41h12`.
- Stable marker assignments:
  - top-left: 0
  - top-right: 1
  - bottom-right: 2
  - bottom-left: 3
- Corrected width of 900 pixels with height derived from physical page aspect ratio.
- Resolution of the bundled Notebook Page layout.
- Resolution of all bundled Smart Page paper/style layouts.
- Required Notebook Design lookup for Notebook Pages.
- Validation of design ownership, layout agreement, semantic marker roles, and physical trim dimensions.
- Typed fail-closed errors for unknown or contradictory layouts.
- No development-layout fallback for unknown stored layouts.
- One shared marker mapping consumed by the PDF renderer and scan registration.

Representative commits:

- `018ee0b` — add Rust-owned scan-layout resolution policy
- `70692cd` — harden scan-layout resolution invariants
- `09aa29e` — export the stable v1 marker layout
- `f363326` — resolve stored scan policy through core storage
- `8dd5976` — fix Notebook fixture integrity
- `bb38cf3` — accept dynamic marker assignments in the image layer

### 3. Layout-driven durable scan registration

Durable scan registration now resolves the stored page policy before image processing and uses it for marker mapping, physical geometry, rectification dimensions, and persisted provenance.

Implemented behavior includes:

- Stored Page/Notebook Design policy resolution before processing.
- Marker-family validation.
- Layout-derived corrected dimensions.
- Layout identity revalidation before and inside the database transaction.
- Structured pipeline provenance containing image pipeline, scan policy, layout, and marker family.
- Fallible scan and audit-event ID generation.
- Fallible timestamps in the registration journal and audit event.
- End-to-end Notebook Page registration coverage.
- End-to-end A4 Smart Page registration coverage verifying a 900 × 1273 corrected image.

Representative commits:

- `63e6378` — drive scan registration from stored layout policy
- `7accd97` — test layout-driven Notebook and Smart Page scans
- `d834847` — expose the core stored-scan-layout type

### 4. PDF marker-policy de-duplication

The PDF renderer no longer owns a separate private corner-marker ID map. It consumes the shared Rust layout marker policy used by scan registration.

### 5. Fail-closed page resolution and materialization

Core page-code and Notebook materialization paths were hardened to remove quiet fallback behavior.

Implemented behavior includes:

- Canonical layout recognition without swallowing bundled-registry construction errors.
- Propagation of bundled-registry failures.
- Fallible Notebook and Page ID generation.
- Fallible Notebook/Page creation timestamps.
- Removal of the obsolete private `now_ms()` helper that returned zero on clock failure.

Representative commits:

- `b2f22c4` — fail closed on page-resolution IDs and time
- `afc7218` — remove zero-time fallback helper

## Rolled-back experiment

A UniFFI record and Android integration were briefly added to project the stored scan-layout policy into camera preview processing. The repository commits generated Kotlin UniFFI bindings, while `tools/build-android-native-libs.sh` explicitly does not regenerate those bindings. The required generator/toolchain was unavailable in the execution environment.

The experimental UniFFI/Android changes were therefore fully removed rather than leaving `master` with guaranteed unresolved Kotlin symbols. The final tree has no `StoredScanLayoutPolicy` or `resolveStoredScanLayoutPolicy` references.

## Validation performed

The pass included:

- Cross-crate static review of the modified Rust interfaces and call sites.
- Focused Rust regression tests added for:
  - orphan discovery
  - invalid final filenames
  - temp-path collisions
  - final-path collisions
  - page-set database rollback after durable asset finalization
  - all bundled layout resolutions
  - unknown/contradictory layout rejection
  - stored Notebook and Smart Page policy resolution
  - dynamic marker assignments
  - Notebook registration
  - A4 Smart Page registration and corrected dimensions
- Android source compatibility was restored after the UniFFI experiment was removed.
- Searches confirmed no stale references to the removed generated-binding API.

## Validation not completed

The execution environment did not provide a usable Rust toolchain, Gradle environment, or GitHub Actions push-run status through the connector. Consequently, the following gates are not claimed as passed:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- Android Gradle compilation/tests
- permanent GitHub Actions workflows

The source TODO must not be marked fully complete until those gates run successfully.

## Open gaps

### A. Generic scan-registration rollback normalization

The generic scan-registration workflow retains a durable journal and committed asset IDs after database failure, but its rollback error has not yet been normalized to the new shared `AssetPersistenceFailureStage` contract with complete per-asset immutable evidence.

Required follow-up:

- classify the rollback as `DatabaseRegistrationRolledBack`;
- include each committed asset's ID, kind, final relative path, expected SHA-256, and byte length;
- preserve journal and staging evidence;
- add deterministic transaction-failure regression coverage.

### B. Opened-handle/no-follow asset verification

`AssetStore::verify` rejects symlinks and canonicalizes the stored path, but it still reopens the pathname for reading after validation. A path-substitution window remains.

Required follow-up:

- introduce an opened-handle API using platform-appropriate no-follow semantics;
- verify the opened file is a regular file contained by the library root;
- hash and measure the same opened handle;
- preserve portable Android/iOS behavior;
- add adversarial substitution/symlink tests.

This change may require a direct platform dependency and must be implemented with a working Cargo toolchain and lockfile verification.

### C. Android preview policy projection

Durable registration and PDF generation share the Rust policy, but Android live/full-resolution preview still uses hard-coded v1 marker IDs and development-page corrected dimensions.

Required follow-up:

- choose either a regenerated UniFFI projection or a focused raw C ABI/JNA policy query;
- regenerate and commit Kotlin bindings if UniFFI is used;
- make full-resolution preview consume Rust-resolved layout dimensions;
- make preview reject policy/layout conflicts rather than falling back;
- keep Android-only guidance thresholds separate from portable processing policy.

### D. Panic-capable convenience Page ID generation

`A2dCore::generate_page_id() -> String` still calls the infallible convenience generator. Changing this exported signature requires coordinated FFI/binding updates.

Required follow-up:

- replace it with a fallible operation;
- regenerate bindings;
- update Android callers and tests.

### E. Permanent validation and TODO reconciliation

After the open implementation items are addressed:

1. run the permanent Rust formatting/lint/test gates;
2. run Android compilation and tests;
3. run the GitHub Actions workflows;
4. inspect every task and subtask in the source TODO against the final code;
5. mark only verified items complete;
6. record any remaining physical-device or photographed-fixture validation separately.

## Recommended next Ralph Loop order

1. Obtain a working Rust/Android toolchain and run formatting plus focused tests on the current head.
2. Fix any compile, formatting, clippy, or test failures before adding more behavior.
3. Normalize generic scan-registration rollback evidence.
4. Implement opened-handle/no-follow asset verification.
5. Regenerate bindings and wire Rust layout policy into Android preview.
6. Re-run the full permanent gates and reconcile the source TODO line by line.
