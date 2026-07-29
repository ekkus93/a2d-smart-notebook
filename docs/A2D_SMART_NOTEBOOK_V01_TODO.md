# A2D Smart Notebook v0.1 — Implementation TODO

**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except photographed Android fixtures and physical-device performance evidence; Milestones 8.1–8.4 CameraX, live analysis/presentation safety, capture state machine, and single-page scanner complete  
**Version:** 0.1  
**Date:** 2026-07-28  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Authoritative specification:** `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`

---

## 0. Execution rules

This TODO is deliberately explicit so an implementation agent can execute it incrementally.

For every task:

- [ ] Read the corresponding specification section before changing code.
- [ ] Keep the Rust core authoritative for data and business logic.
- [ ] Add or update tests with the implementation.
- [ ] Run the narrowest relevant tests during development.
- [ ] Run full required checks before marking a milestone complete.
- [ ] Preserve original assets and existing data.
- [ ] Return structured errors and warnings.
- [ ] Add fixtures for portable formats and state transitions.
- [ ] Keep every referenced handoff file committed at the exact path named.

Do not:

- [ ] Put canonical domain rules in Kotlin ViewModels.
- [ ] Add Room as a second canonical database.
- [ ] Return empty data after an internal failure.
- [ ] Catch broad exceptions and continue without surfacing a warning.
- [ ] Replace invalid data with defaults silently.
- [ ] Delete or overwrite an existing original scan during rescan handling.
- [ ] Upload note data without a user-selected provider and explicit scope.
- [ ] Store model API keys in source, Gradle, Rust configuration, logs, or the canonical database.
- [ ] Give skills raw SQL, shell, arbitrary filesystem, or implicit network access.
- [ ] Mark a task complete when only a mock or stub exists unless the task explicitly calls for one.

A task is complete only when code compiles, tests pass, no placeholder remains in the completed path, error handling is explicit, and acceptance behavior is demonstrated.

---

# Milestone 1 — Repository, toolchain, Android shell, and CI

## 1.1 Create the workspace

Create:

```text
Cargo.toml
rust-toolchain.toml
deny.toml
crates/
apps/android/
apps/ios/
fixtures/
tools/
.github/workflows/
```

Initial crates:

```text
crates/a2d-domain
crates/a2d-identity
crates/a2d-layout
crates/a2d-storage
crates/a2d-image
crates/a2d-pdf
crates/a2d-search
crates/a2d-ocr
crates/a2d-model
crates/a2d-skills
crates/a2d-backup
crates/a2d-export
crates/a2d-sync-model
crates/a2d-core
crates/a2d-ffi
```

Tasks:

- [x] Create the Rust workspace with resolver 2.
- [x] Pin a stable Rust toolchain.
- [x] Set workspace-wide edition, license, repository metadata, and lint policy.
- [x] Add `.gitignore` entries for Rust, Gradle, Android Studio, native builds, test output, local libraries, generated PDFs, and secrets.
- [x] Add a `README.md` describing A2D, local-first/accountless operation, Android-first delivery, and the authoritative Rust core.
- [x] Add `apps/ios/README.md` explaining that iOS UI is deferred but Swift binding generation is mandatory.
- [x] Verify `cargo metadata` and `cargo build --workspace`.

Suggested skeleton:

```toml
[workspace]
resolver = "2"
members = [
  "crates/a2d-domain",
  "crates/a2d-identity",
  "crates/a2d-layout",
  "crates/a2d-storage",
  "crates/a2d-image",
  "crates/a2d-pdf",
  "crates/a2d-search",
  "crates/a2d-ocr",
  "crates/a2d-model",
  "crates/a2d-skills",
  "crates/a2d-backup",
  "crates/a2d-export",
  "crates/a2d-sync-model",
  "crates/a2d-core",
  "crates/a2d-ffi",
]
```

Acceptance:

- [x] A fresh clone builds the Rust workspace.
- [x] Responsibility boundaries are documented in crate-level READMEs or module docs.

## 1.2 Initialize Android

- [x] Create a Kotlin Android application in `apps/android`.
- [x] Use Jetpack Compose and Compose Navigation. (AGP 8.7.3 / Kotlin 2.0.21 / Compose BOM
      2024.10.00 / navigation-compose 2.8.4 — cached Gradle wrapper distributions and a working
      Android SDK + emulator were already present in this environment; verified end to end, not
      just configured.)
- [x] Use package namespace `com.a2d.notebook`.
- [x] Document the selected minimum Android API and why. (`minSdk = 26`, reasoning in
      `app/build.gradle.kts` — an open decision, spec/TODO leave it unspecified.)
- [x] Add a placeholder Home screen and navigation shell. (`A2dNavHost` with a single `home`
      route; more routes land as their screens do, spec §26.)
- [x] Add unit and instrumentation test source sets. (`src/test` — JVM, `src/androidTest` —
      Compose UI test that launches `MainActivity` for real.)
- [x] Confirm debug installation on an emulator or device. (`./gradlew :app:installDebug`
      against the pre-existing `Medium_Phone_API_36.0` AVD; confirmed independently via
      `adb shell pm list packages`.)
- [x] Do not add Room for canonical A2D data. (No Room dependency anywhere in this module.)

Acceptance:

- [x] `./gradlew :app:assembleDebug` succeeds.
- [x] The placeholder application launches. (Proven, not just claimed: an instrumented test
      launches `MainActivity` on the real emulator and asserts the Home screen's title is
      displayed — `HomeScreenLaunchTest`, 0 failures.)

## 1.3 Add CI

- [x] Rust format check. (`.github/workflows/ci.yml`, job `rust`.)
- [x] Rust clippy with warnings denied.
- [x] Rust unit/integration tests.
- [x] Android lint and unit tests. (Job `android`, `./gradlew lint test assembleDebug`.)
- [x] Android debug assembly.
- [x] Kotlin UniFFI binding generation drift check. (Job `android-binding-drift`: cross-compiles
      `a2d-ffi` for Android via `cargo-ndk`, regenerates the Kotlin bindings via
      `tools/build-android-native.sh`, then `git diff --exit-code` against what's committed —
      catches an `a2d-ffi` API change that wasn't followed by regenerating/committing the Android
      bindings. Verified passing on real GitHub Actions infrastructure, not just locally.)
- [x] Swift UniFFI binding generation smoke check. (Covered by the `rust` job's `cargo test`,
      which exercises `crates/a2d-ffi/tests/binding_generation.rs` — that test regenerates and
      asserts on both Kotlin and Swift bindings from the desktop build. No separate job.)
- [x] Dependency/license checks after policy configuration. (Job `deny`, `cargo deny check`
      against `deny.toml`. The first real CI run caught two genuine policy gaps this task fixed:
      MPL-2.0 wasn't allow-listed (uniffi's license) and workspace-internal path dependencies
      were flagged as "wildcard" — fixed by marking all 15 crates `publish = false`, which
      `allow-wildcard-paths` requires.)
- [x] Fixture compatibility checks. Permanent CI regenerates the deterministic scan corpus,
      verifies manifest metadata and hashes, fails on byte drift, enforces versioned synthetic
      processing envelopes, runs Android ZXing conformance tests, and exercises the packaged shared
      Rust analysis path on an Android emulator.

Required commands:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
./gradlew lint test assembleDebug
```

Acceptance:

- [x] CI runs on pushes and pull requests. (`on: push: branches: [master]` and `on:
      pull_request:` in the workflow file; the pushes so far have genuinely triggered it —
      confirmed via `gh run list`, not assumed.)
- [x] Deliberate formatting and test failures block CI. (Not simulated — the very first real
      run genuinely failed on 2 of 4 jobs, `cargo deny` and `cargo test`, both real bugs this
      task then fixed; the next push genuinely passed all 4. `gh run list` shows both outcomes:
      run `30220072378` `completed failure`, run `30223416584` all green. That sequence is
      itself the evidence for this criterion, not a separately staged demonstration.)

---

# Milestone 2 — Domain model, structured errors, and UniFFI

## 2.1 Strongly typed identifiers

Create opaque Rust newtypes for every independently persisted entity — one that has its own row, can be referenced by another record, appears in provenance, or can cross the FFI boundary. Do not add identifiers for embedded value objects with no independent persistence or identity.

- [x] `InstallationId`
- [x] `NotebookDesignId`
- [x] `NotebookId`
- [x] `PageId`
- [x] `PageSetId`
- [x] `SmartPageId`
- [x] `PhysicalCopyId`
- [x] `ScanId`
- [x] `AssetId`
- [x] `OcrRunId`
- [x] `TextRegionId`
- [x] `TextCorrectionId`
- [x] `CollectionId`
- [x] `AnnotationId`
- [x] `ReviewItemId`
- [x] `SkillId`
- [x] `SkillRunId`
- [x] `AuditEventId`
- [x] `BackupId`

Example:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PageId(String);
```

Requirements:

- [x] Constructors validate format.
- [x] Display and serialization are canonical.
- [x] Parsing rejects invalid length/alphabet.
- [x] Production IDs use OS cryptographic randomness.
- [x] A deterministic RNG is available only through test interfaces.
- [x] Domain APIs do not pass raw identifier strings internally.

## 2.2 Structured error model

Implement an FFI-safe error envelope:

```rust
pub struct A2dError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub user_message_key: String,
    pub developer_message: String,
    pub retryable: bool,
    pub correlation_id: String,
    pub details: BTreeMap<String, String>,
}
```

- [x] Implement all categories from the spec.
- [ ] Redact secrets and raw note content from default diagnostics. (No concrete redaction
      mechanism exists yet — `A2dError.details` is documented as MUST NOT carry secrets/note
      content, enforced by each producing module, not by a generic filter. Revisit once a
      producer actually needs to pass user content through, e.g. OCR or model-provider errors.)
- [x] Map cancellation separately from failure.
- [ ] Ban conversions that erase failures into `None`, empty collections, or `false`. (No
      erasing conversion exists on `A2dError`/`Outcome` today, but "ban" implies an ongoing
      project-wide discipline — no clippy lint enforces it yet. Tracked as a standing rule in
      `CLAUDE.md`, not a one-time deliverable.)
- [x] Add stable unknown-internal-error handling with correlation ID.
- [ ] Add tests for FFI/serialization mapping and redaction. (No FFI/serde boundary exists yet;
      deferred to Milestone 2.4.)

## 2.3 Domain entities and invariants

Implement:

- [x] `NotebookDesign`
- [x] `Notebook`
- [x] `Page`
- [x] `PhysicalCopy`
- [x] `Scan`
- [x] `Asset`
- [x] `PageSet`
- [x] `Collection`
- [x] `ReviewItem`
- [x] `OcrRun`
- [x] `TextRegion`
- [x] `TextCorrection`
- [x] `Annotation`
- [x] `SkillDefinition`
- [x] `SkillRun`
- [x] `AuditEvent`

Spec §15 gives full field lists only for `NotebookDesign`/`Notebook`/`Page`/`PhysicalCopy`/`Scan`/
`Asset`; the rest (`PageSet`, `Collection`, `ReviewItem`'s field shape, `OcrRun`, `TextRegion`,
`TextCorrection`, `Annotation`, `SkillDefinition`, `SkillRun`, `AuditEvent`) are described only in
prose elsewhere in the spec/TODO. Their fields in `a2d-domain/src/entities.rs` are inferred and
marked `INFERRED` in each doc comment — flagged in `memory.md` for review, since several will need
revisiting once Milestones 5/7/14 pin down real requirements (layouts, markers, skill permissions).

Suggested page kind:

```rust
pub enum PageKind {
    NotebookPage {
        notebook_id: NotebookId,
        design_id: NotebookDesignId,
        logical_page_number: u32,
    },
    SmartPage {
        smart_page_id: SmartPageId,
        page_set_id: Option<PageSetId>,
        visible_page_number: Option<u32>,
    },
}
```

Enforce:

- [x] Notebook Page requires notebook, design, and logical page number. (Compiler-enforced:
      `PageKind::NotebookPage` cannot be constructed without all three.)
- [x] Smart Page requires a **unique** Smart Page ID. (The "requires a Smart Page ID" half is
      compiler-enforced; *uniqueness* is now enforced by `unique_smart_page_id`, a partial unique
      index in `migrations/0001_initial.sql` — Milestone 3.1 — tested in `a2d-storage`.)
- [x] Page identity cannot change after creation. (`Page::id` — and every other entity's `id` —
      is private with a getter and no setter.)
- [x] Preferred scan belongs to the same page. (`Page::set_preferred_scan` rejects a scan
      belonging to a different page; tested.)
- [x] A scan always references an immutable original asset. (Closed once storage existed to
      check it: `ScanRepository::insert_scan` looks up `original_asset_id` and rejects the
      insert — before it ever reaches the database — if the referenced asset doesn't exist
      (`STORAGE_SCAN_ORIGINAL_ASSET_MISSING`) or isn't marked immutable
      (`STORAGE_SCAN_ORIGINAL_ASSET_NOT_IMMUTABLE`); both paths tested.)
- [x] Physical-copy index is unique per page. (`unique_physical_copy_index` in
      `migrations/0001_initial.sql` — Milestone 3.1, tested in `a2d-storage`.)
- [x] Derived records identify source and producer. (`OcrRun`, `TextCorrection`, `Annotation`,
      and `SkillRun` all require a non-optional `Provenance`, which itself requires
      `producing_component`/`component_version`.)
- [ ] Trashed records retain identity until permanent deletion. (`PageState::Trashed` exists as
      structural support, but the actual trash/restore/permanent-delete workflow that guarantees
      this belongs to Milestones 3 and 10.)

## 2.4 UniFFI boundary

- [x] Pin and configure UniFFI. (`uniffi = "0.32"`, `crates/a2d-ffi/Cargo.toml`.)
- [x] Choose proc-macro or UDL mode and document the reason. (Proc-macro — see the reasoning in
      `crates/a2d-ffi/src/lib.rs`'s module doc.)
- [x] Create `a2d-ffi` as a thin façade over `a2d-core`. (Every exported method is a one-line
      delegation to an `a2d-core::A2dCore` method; no SQL or business rules live in `a2d-ffi`.)
- [x] Generate Kotlin bindings. (`tools/generate-bindings.sh`; proven by
      `crates/a2d-ffi/tests/binding_generation.rs`, which regenerates and asserts on the output
      every test run — not a one-off manual check.)
- [x] Generate Swift bindings. (Same script/test, `--language swift`.)
- [x] Add a minimal `A2dClient` object. (`open`, `library_path`, `generate_page_id`,
      `parse_page_id` — deliberately narrower than the TODO's `list_notebooks` example, since
      notebook listing needs storage (Milestone 3) that doesn't exist yet; see `a2d-core`'s module
      doc for why nothing here is a fabricated stub.)
- [x] Add API snapshot or generated-binding drift tests. (Not a golden-file diff against checked-in
      bindings — those aren't committed, since they're build output regenerated by Android/Xcode
      builds — but `binding_generation.rs` fails if the interface ever stops producing bindings
      that expose the expected symbols.)
- [x] Prevent panics from appearing as successful FFI results. Verified end-to-end now that a
      real Android consumer exists:
      `apps/android/app/src/androidTest/kotlin/com/a2d/notebook/app/PanicPropagationTest.kt`
      calls `A2dClient.triggerPanicForTesting()` on the real emulator and asserts it throws
      `uniffi.a2d_ffi.InternalException` carrying the original panic message, rather than
      returning normally or crashing the process — confirmed by inspecting the generated
      bindings first (`uniffiCheckCallStatus`'s panic branch) before writing a test that could,
      in principle, have crashed the test process if that inspection had been wrong.

Example API shape:

```rust
pub struct A2dClient {
    core: Arc<A2dCore>,
}

impl A2dClient {
    pub fn open(request: OpenLibraryRequest) -> Result<Arc<Self>, A2dFfiError>;
    pub async fn list_notebooks(&self) -> Result<Vec<NotebookSummary>, A2dFfiError>;
}
```

Acceptance:

- [x] Android calls Rust and renders a typed response. (`com.a2d.notebook.rustbridge.A2dBridge`
      calls `A2dClient.generatePageId()` across the real UniFFI/JNA boundary (Rust cross-compiled
      for Android via `cargo-ndk`); the Home screen renders the typed `String` result. Verified
      with an instrumented test asserting the rendered text is a real 26-char canonical
      Crockford Base32 `PageId`, run on the actual emulator — `HomeScreenLaunchTest.
      homeScreenRendersARealPageIdGeneratedByRust`.)
- [ ] Swift bindings generate in CI. (Generation itself works, proven by
      `binding_generation.rs` — but there is no CI pipeline yet, Milestone 1.3, so "in CI"
      specifically is unmet.)
- [x] `a2d-ffi` contains no SQL or business rules.

---

# Milestone 3 — Rust-owned SQLite and assets

## 3.1 Database bootstrap and migrations

- [x] Open/create SQLite at an app-provided library path.
- [x] Enable and verify foreign keys. (Verified, not just requested — `Storage::open` re-queries
      `PRAGMA foreign_keys` after setting it and fails closed if SQLite doesn't confirm it's on.)
- [x] Select and document journaling/synchronous settings. (WAL / NORMAL — reasoning in
      `a2d-storage/src/lib.rs`'s module doc; also verified by re-querying, same as foreign keys.)
- [x] Implement numbered immutable migrations. (`crates/a2d-storage/src/migrations/0001_initial.sql`,
      applied inside a transaction so a failure can't leave a half-applied migration committed.)
- [x] Track schema version and migration identity. (`schema_migrations(version, name,
      applied_at_ms)`; reopening detects a version whose recorded `name` doesn't match the code's
      name for it and fails closed rather than silently re-trusting a modified migration — tested.)
- [x] Return blocking errors on migration failure. (Every failure path returns `Result::Err`,
      propagated to the caller; nothing swallows a migration error.)
- [x] Never recreate an empty database after migration failure. (`apply_migration` never
      deletes/truncates the file; a failed migration's transaction rolls back on drop per
      rusqlite's own contract, leaving the database at its last successfully committed state.)

Initial tables must cover notebooks, notebook designs, pages, physical copies, scans, assets, page sets, collections, OCR runs, text regions, corrections, annotations, review items, skill definitions/runs, audit events, backup history, and settings.

Add constraints and indexes for invariants and common queries.

Example guidance:

```sql
CREATE TABLE notebooks (
    id TEXT PRIMARY KEY NOT NULL,
    design_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE TABLE pages (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    notebook_id TEXT,
    design_id TEXT,
    logical_page_number INTEGER,
    smart_page_id TEXT,
    page_set_id TEXT,
    visible_page_number INTEGER,
    layout_id TEXT NOT NULL,
    title TEXT,
    state TEXT NOT NULL,
    preferred_scan_id TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(notebook_id) REFERENCES notebooks(id)
);

CREATE UNIQUE INDEX unique_notebook_logical_page
ON pages(notebook_id, logical_page_number)
WHERE notebook_id IS NOT NULL;
```

## 3.2 Repository and transaction layer

- [x] Define repository traits inside Rust. (8 traits — `NotebookDesign`/`Notebook`/`PageSet`/
      `Page`/`Asset`/`Scan`/`OcrRun`/`AuditEvent`Repository — scoped to what this milestone's
      example and Milestone 3.3's asset protocol need; the other 10 tables get a repository when
      the milestone that needs them, e.g. 11/12/13/14, arrives.)
- [x] Keep SQL implementations private to `a2d-storage`. (All SQL strings live in
      `repository.rs`/`lib.rs`; the traits' public signatures only ever take/return typed
      `a2d_domain` values.)
- [x] Add explicit transaction APIs. (`Storage::transaction`, tested.)
- [ ] Require transactions for notebook creation, generated page sets, scan registration, OCR
      replacement, and restore merge. (Only "scan registration" is actually demonstrated composed
      through `Storage::transaction`, matching this section's own example almost verbatim —
      `scan_registration_composes_through_one_transaction_matching_the_todo_example`. Notebook
      creation and page-set generation are currently single-row inserts with nothing else to
      compose transactionally yet — that composition is Milestone 6's job. "OCR replacement" and
      "restore merge" aren't implemented at all: OCR runs are append-only here (each attempt is
      its own row, no in-place replace semantics defined), and restore merge needs the backup
      format, Milestone 13, which doesn't exist yet. Nothing here *prevents* calling a repository
      method outside a transaction, either — there's no enforcement mechanism, only the API.)
- [x] Map constraints into typed errors. (UNIQUE/PRIMARY KEY → `STORAGE_UNIQUE_CONSTRAINT_VIOLATION`,
      FOREIGN KEY → `STORAGE_FOREIGN_KEY_VIOLATION`, NOT NULL → `STORAGE_NOT_NULL_VIOLATION`, all
      `ErrorCategory::Validation` rather than a generic storage error; tested.)

Example:

```rust
storage.transaction(|tx| {
    tx.insert_page(&page)?;
    tx.insert_scan(&scan)?;
    tx.set_preferred_scan(&page.id, &scan.id)?;
    tx.insert_audit_event(&event)?;
    Ok(())
})
```

## 3.3 Asset repository

Use a private tree similar to:

```text
library/
├── library.sqlite
├── assets/
│   ├── originals/
│   ├── corrected/
│   ├── ocr/
│   ├── thumbnails/
│   └── exports/
└── tmp/
```

- [x] Store relative paths in the database. (`Asset.relative_path`, e.g. `assets/originals/<id>`.)
- [x] Validate paths remain inside library root. (`AssetStore::resolve`, tested against a real
      traversal-shaped path.)
- [x] Write temporary files first. (`tmp/<AssetId>.tmp`.)
- [x] Distinguish userspace flush from persistence. `Write::flush()` is required but is never
      described as durable by itself.
- [x] Apply original read-only metadata, call `File::sync_all()`, close, then re-read and verify
      byte length and SHA-256 against the supplied bytes.
- [x] Finalize without replacement using a same-filesystem hard link, verify finalized metadata,
      synchronize the destination directory, remove the temporary link, and synchronize `tmp/`.
      The complete terminology and platform scope are normative in
      `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.
- [x] Mark originals immutable. (Read-only file permission bit + `Asset.immutable = true`;
      tested.)
- [x] Detect orphan temporary files without deleting unknown files silently.
      (`AssetStore::list_orphaned_temp_files`; tested that it reports without deleting.)
- [x] Commit references only after the asset filesystem commit. `commit` returns an `Asset`
      only after file `sync_all()`, no-replace finalization, finalized-metadata verification,
      destination-directory synchronization, temporary-link removal, and temporary-directory
      synchronization all succeed. SQLite registration occurs afterward and has its own documented
      WAL/`synchronous=NORMAL` durability semantics.

## 3.4 Integrity and interruption tests

- [x] Interrupt after temp write. (Simulated by writing directly into `tmp/` without going
      through `commit`, then confirming `list_orphaned_temp_files` finds it and does not delete
      it.)
- [ ] Interrupt after rename but before DB commit. (Not separately tested. Structurally this
      failure mode is already safe — `commit` never touches the database, so a process dying
      between a successful `commit` and the caller's `insert_asset` just leaves an unreferenced
      but durably-written file, not a dangling DB reference or corruption — but there's no
      dedicated test proving that specific window.)
- [x] Force transaction rollback. (`a_failing_step_rolls_back_every_earlier_write_in_the_same_transaction`.)
- [x] Detect missing assets and hash mismatches. (`AssetStore::verify` — added during this task;
      re-checks a previously committed asset's file against its recorded hash. Tested for both a
      deleted file and a tampered one.)
- [x] Test foreign-key failures and DB lock/busy handling. (FK failures: tested. Lock/busy:
      `Storage::open`/`open_in_memory` now set and verify `PRAGMA busy_timeout` — added during
      this task, since it wasn't set at all before, meaning any concurrent writer would have
      failed immediately with `SQLITE_BUSY` rather than waiting; proven with a real two-thread
      test where a second writer blocks on a held write lock and then succeeds rather than
      erroring immediately.)
- [ ] Map disk-full behavior explicitly. (Not tested — a genuine disk-full condition needs
      environment-specific setup, e.g. a size-limited filesystem, not portably available here.
      The I/O error path (`map_io_error`) already handles an arbitrary `std::io::Error` from a
      failed write generically, which is what a real `ENOSPC` would surface through, but that's
      untested against the real condition specifically.)

Acceptance:

- [x] A committed scan can never reference an original file that did not complete the v0.1
      asset filesystem commit contract. The guarantee is stated separately from SQLite
      power-loss durability; WAL/`synchronous=NORMAL` may lose the latest transaction after an OS
      crash while preserving database consistency.
- [x] Recovery never deletes user data silently. (True by omission: no code in this crate deletes
      anything automatically — `list_orphaned_temp_files` only reports, migrations only add.)

---

# Milestone 4 — Identity, QR protocol, and Notebook Designs

## 4.1 Random ID generation

- [x] Generate 128-bit IDs from OS cryptographic randomness. Done in Milestone 2.1 —
      `a2d_domain::id::random_128` via `getrandom`, used by every `define_id!`-generated
      `::generate()`.
- [x] Encode using a compact canonical alphabet. Done in Milestone 2.1 — 26-char uppercase
      Crockford Base32 (`a2d_domain::id::encode_128`/`decode_128`), the same alphabet
      `docs/decisions/0001-qr-v1-encoding-and-integrity.md` reuses for QR `id128` fields.
- [x] Detect persistence collisions as hard integrity events. New this task —
      `crates/a2d-storage/src/repository.rs::map_sql_error` now distinguishes SQLite's
      `SQLITE_CONSTRAINT_PRIMARYKEY` (1555) from ordinary `SQLITE_CONSTRAINT_UNIQUE` (2067):
      every `id` column is that table's primary key, so a 1555 collision means a freshly
      generated 128-bit ID already exists — mapped to a dedicated `STORAGE_ID_COLLISION` /
      `ErrorCategory::Integrity` / `ErrorSeverity::Critical` error instead of the everyday
      `STORAGE_UNIQUE_CONSTRAINT_VIOLATION` / `Validation` used for business-rule unique indexes
      (e.g. one logical page per notebook). Verified by
      `reinserting_an_existing_id_is_reported_as_an_integrity_event_not_a_validation_error` and
      the sibling test confirming ordinary unique-index violations still map to `Validation`
      (`crates/a2d-storage/tests/repository_and_assets.rs`).
- [x] Add known encoding vectors and malformed-input tests. Done in Milestone 2.1 —
      `known_encoding_vector`, `rejects_wrong_length`, `rejects_invalid_alphabet`,
      `rejects_non_canonical_padding`, `rejects_lowercase` in `crates/a2d-domain/src/id.rs`.
- [x] Add a large sample uniqueness test. Done in Milestone 2.1 — `large_sample_is_unique`
      (10,000 samples) in `crates/a2d-domain/src/id.rs`.

## 4.2 QR payload model

The v1 wire encoding and integrity check are governed by
`docs/decisions/0001-qr-v1-encoding-and-integrity.md`. That ADR must reach **Accepted** status
before this task's golden fixtures (4.3) are committed — the Android decoder spike itself is done
(that ADR's Validation Evidence), but the physical-layout module-size/damage-tolerance item is
still open pending Milestone 5. Do not invent an alternate encoding here — implement the ADR's
canonical alphanumeric text payload, uppercase Crockford Base32 identifiers, and CRC-32C
integrity field.

**A minimal encoder already exists** ahead of this task, in `crates/a2d-identity/src/qr.rs`
(`PageCode::encode`) — built specifically to give the ADR's spike real payloads rather than
hand-typed fixtures. It covers only encoding, not the strict parser/decoder below, and has no
golden fixtures yet. Extend it rather than starting over.

Implement:

```rust
pub enum PageCode {
    NotebookSetup {
        version: u8,
        design_id: NotebookDesignId,
    },
    NotebookPage {
        version: u8,
        design_id: NotebookDesignId,
        logical_page_number: u32,
        layout_id: LayoutId,
    },
    SmartPage {
        version: u8,
        smart_page_id: SmartPageId,
        layout_id: LayoutId,
        visible_page_number: Option<u32>,
        page_set_id: Option<PageSetId>,
    },
}
```

- [x] Define canonical v1 encoding. `PageCode::encode` (pre-existing) plus new
      `a2d_identity::qr::parse`, implementing the ADR's grammar in both directions. Deviation
      from this task's illustrative `PageCode` sketch, recorded deliberately: the sketch gives
      each variant a `version: u8` field, but the actual implementation has no such field —
      `version` is a single ASCII digit fixed to `"1"` in the wire grammar itself (the ADR:
      "v1 parsers understand `"1"` only"), so there is nothing for a per-value `version` field to
      vary; the parser rejects any other digit as `QR_UNSUPPORTED_VERSION` rather than accepting
      it into a struct field. A future v2 grammar gets its own parse function, not a runtime
      version field on `PageCode`.
- [x] Define checksum/integrity bytes. CRC-32C over the payload through the delimiter preceding
      `crc`, encoded as 7 canonical Crockford Base32 characters (pre-existing `encode_crc`,
      reused by `parse`'s recomputation check).
- [x] Define maximum length. 128 characters, checked before tokenizing
      (`MAX_PAYLOAD_LEN` in `crates/a2d-identity/src/qr.rs`).
- [x] Reject unsupported versions, invalid lengths, invalid alphabet, out-of-range numbers, failed
      integrity, and trailing data. `a2d_identity::qr::parse`, one rejection test per rule in the
      ADR's "Strict-parser rules" list (lowercase/non-alphanumeric byte, wrong magic prefix,
      unsupported version, unknown type-code, wrong field count/trailing data, `id128` wrong
      length, `id128` containing I/L/O/U, numeric field with leading zero or sign, numeric field
      out of range, unregistered `layout-id`, CRC mismatch, oversized payload) plus round-trip
      tests for all three `type-code`s — 29 tests total in `crates/a2d-identity/src/qr.rs`,
      `cargo test -p a2d-identity`. The `layout-id` registry check takes the registry as a
      caller-supplied predicate (`impl Fn(&LayoutId) -> bool`) rather than depending on
      `a2d-layout` directly, since that crate's real registry doesn't exist until Milestone 5;
      wiring the real predicate through is `a2d-core`'s job once it does.
- [x] Never open invalid A2D payloads as arbitrary URLs. Satisfied by construction: `parse`'s
      signature only ever returns a typed `PageCode` or a typed `A2dError` — there is no code
      path that hands a rejected payload's raw string back to a caller for reinterpretation as a
      URL. The operational half of this rule (Android's scan-result handling never falling back
      to "open as a link" on a parse failure) is Milestone 8's job once that UI exists; nothing to
      verify there yet.

## 4.3 Golden fixtures

Create:

```text
fixtures/qr/v1/
├── notebook_setup_vectors.json
├── notebook_page_vectors.json
├── smart_page_vectors.json
├── malformed_vectors.json
└── rendered/
```

- [x] Store payload, expected value, and expected errors in permanent generated JSON vectors.
- [x] Render deterministic PNG QR images for every valid vector.
- [x] Decode every rendered PNG through `rqrr` and then the canonical Rust parser.
- [x] Treat v1 vectors as permanent compatibility fixtures; future changes require a new version.
- [x] ADR 0001 is Accepted. Changing the v1 wire format requires a new protocol version, not a fixture rewrite.

## 4.4 Notebook Design manifests

- [x] Define versioned manifests with physical dimensions, layout IDs, marker family/roles,
      logical page count, and hash. `crates/a2d-layout/src/manifest.rs`: a JSON `RawManifest`
      wire shape, `parse_manifest` converting it into the existing `a2d_domain::NotebookDesign`
      entity (Milestone 2.3) and computing `manifest_hash` as the SHA-256 of the manifest's exact
      source bytes.
- [ ] Bundle initial official manifests. **Not done, deliberately** — there is no real physical
      Notebook Design yet (trim size, marker family, and real page layouts are all Milestone 5
      decisions), so there is nothing "official" to bundle. What exists instead: one manifest
      explicitly named and documented as a development placeholder
      (`crates/a2d-layout/manifests/dev-placeholder.json`,
      `ManifestRegistry::bundled_placeholder_registry`), proving the loading mechanism works
      end-to-end without pretending it carries real product content. Milestone 5 replaces this
      with the first real manifest; the registry mechanism itself does not need to change.
- [x] Resolve them fully offline. `ManifestRegistry::resolve` is a synchronous in-memory
      `HashMap` lookup — no I/O, no network, populated entirely up front from bundled strings.
- [x] Track trust state. `TrustState` (Milestone 2.3) is assigned by the loader based on
      provenance (bundled-with-a-reviewed-build → `Trusted` for v0.1), not read from the manifest
      file itself.
- [x] Leave extension fields for signed official designs. Trust is deliberately *not* a field
      inside the manifest JSON — see the module doc comment in `manifest.rs` for why a
      self-declared trust field would be meaningless (a hostile manifest could just claim
      `"Trusted"`). A future signed-manifest extension supplies its own trust derivation (e.g.
      signature verification) through the same `parse_manifest(json, trust_state)` call shape,
      without changing the manifest wire format.
- [x] Reject unsupported required versions. `CURRENT_SCHEMA_VERSION = 1`; `parse_manifest`
      rejects `schema_version` greater than that (`MANIFEST_UNSUPPORTED_SCHEMA_VERSION`) and
      rejects `schema_version: 0` outright. 11 tests in `crates/a2d-layout/src/manifest.rs`,
      `cargo test -p a2d-layout`.

Acceptance:

- [x] Setup and Page Codes round-trip through Rust, Kotlin, and rendered fixtures. Permanent
      vectors exercise canonical encode/parse, rendered PNG decoding, and the Android ZXing path.

---

# Milestone 5 — Layout engine and Rust PDF generation

## 5.1 Canonical physical layout model

- [x] Use fixed physical units, not authoritative screen pixels. `crates/a2d-layout/src/geometry.rs`:
      `PhysicalSize`/`PhysicalPoint`/`PhysicalRect` are all millimeter-denominated (`f64`), never
      device pixels; module doc records the coordinate convention (top-left origin, y down) and
      that converting to PDF's bottom-left-origin space is `a2d-pdf`'s job (Milestone 5.4), not
      this crate's.
- [x] Define page size, safe margins, content rectangle, four marker rectangles, QR rectangle,
      visible numbering, and calibration mark. `crates/a2d-layout/src/page_layout.rs`:
      `PageLayout` (matches this task's own suggested shape almost exactly), `MarkerRole`/
      `MarkerPlacement`, `CalibrationMark`.
- [x] Validate bounds, overlap, marker roles, quiet zones, and content-style spacing.
      `PageLayout::validate` requires exactly one marker per role, keeps every element within the
      safe-margin inset, keeps machine-readable quiet zones clear, and rejects ruling spacing that
      is non-finite or not strictly positive. The spacing invariant is repeated inside the PDF
      renderer as defense in depth because callers can construct a `PageLayout` without invoking
      `validate()`.

Example:

```rust
pub struct PageLayout {
    pub id: LayoutId,
    pub physical_size: PhysicalSize,
    pub content_rect: PhysicalRect,
    pub markers: [MarkerPlacement; 4],
    pub qr_rect: PhysicalRect,
    pub visible_page_number_rect: Option<PhysicalRect>,
    pub calibration: CalibrationMark,
}
```

## 5.2 Smart Page layouts

Add US Letter and A4 portrait layouts for:

- [x] Blank.
- [x] Lined.
- [x] Dot grid.
- [x] Graph.

`crates/a2d-layout/src/smart_page.rs::smart_page_layout(PaperSize, SmartPageStyle)` builds all
8 combinations (2 paper sizes × 4 styles). Every combination shares identical marker/QR/
content-rect geometry within a paper size — only `ContentStyle` (line/dot/graph spacing) differs
— but each still gets its own `LayoutId` (e.g. `SP-LETTER-LINED-V1`) per this task's own framing
("US Letter and A4 portrait layouts for: Blank/Lined/Dot grid/Graph" as 8 distinct things, not 2).
Physical constants (6mm safe margin, 3mm quiet zone, 18mm marker/QR size, 7mm line spacing, 5mm
dot/graph spacing) are recorded starting assumptions per CLAUDE.md's open-decisions policy, not
measured values — Milestone 17's physical print validation is what actually confirms or revises
them.

- [x] Test deterministic dimensions and spacing.
      `dimensions_and_spacing_are_deterministic_across_calls`,
      `letter_and_a4_layouts_use_their_respective_physical_dimensions`,
      `each_style_carries_its_own_content_style_metadata`.
- [x] Test that markers and QR remain within printer-safe margins.
      `markers_and_qr_stay_within_the_printer_safe_margin`, checked for all 8 layouts.
- [x] Test no overlap with the writable region. `content_rect_never_overlaps_any_marker_or_the_qr_code`
      plus `every_paper_size_and_style_combination_produces_a_valid_layout`, which additionally
      runs the full `PageLayout::validate` (marker-role uniqueness, safe-margin bounds, and every
      machine-readable element's quiet zone) against all 8 layouts — 7 new tests, 32 total in the
      crate, `cargo test -p a2d-layout`.

## 5.3 Bound-notebook layout

- [x] Record the first trim-size decision. `crates/a2d-layout/src/notebook.rs::NOTEBOOK_TRIM_SIZE_MM`
      — 152mm x 229mm (6in x 9in), a common print-on-demand journal trim, recorded as a starting
      assumption per CLAUDE.md's open-decisions policy (not measured/validated until Milestone
      17). `crates/a2d-layout/manifests/dev-placeholder.json`'s `trim_width_mm`/`trim_height_mm`
      updated to match, so the placeholder manifest and this layout module agree.
- [x] Define a larger left/gutter exclusion. `GUTTER_MARGIN_MM = 20.0` vs. `OUTER_MARGIN_MM =
      6.0` for the other three edges, threaded through `layout_builder::build_layout`'s
      `left_margin_mm` parameter (shared with Milestone 5.2, `left_margin_mm == margin_mm` there
      since Smart Pages have no gutter). Test:
      `the_gutter_exclusion_is_wider_than_the_outer_margin`.
- [x] Define fixed recto orientation. Structural, not a flag: this module defines only
      `setup_page_layout`/`writable_page_layout` (both recto). There is no verso `PageLayout`
      constructor at all — a verso page has no markers, no QR, no content geometry, so there is
      nothing for one to return (spec: "Verso pages remain blank in v0.1"). See the module doc
      comment for why this is enforced by omission rather than an enum variant.
- [x] Define Setup Page and writable page layouts. `setup_page_layout()` (no visible page number)
      and `writable_page_layout()` (has one), both built from the shared `build_layout` helper
      introduced this task (extracted from Milestone 5.2's `smart_page_layout`, which now calls
      it too rather than duplicating the geometry logic). Layout ids (`DEV-SETUP-V1`,
      `DEV-PAGE-V1`) intentionally match the placeholder manifest's `setup_layout_id`/
      `page_layout_id` from Milestone 4.4.
- [x] Define logical numbering independent of manuscript PDF page number.
      `pdf_page_number_for_logical_page`: logical page 1 → PDF page 3, logical page 2 → PDF page
      5, and so on (PDF page 1 is the Setup Page, page 2 its blank verso, and every logical page
      after that consumes one recto PDF page plus the blank verso immediately before it). Tests
      confirm the mapping is deterministic, strictly increasing by 2, and never equal to the
      logical page number itself.
- [x] Generate blank verso pages. `a2d_pdf::generate_notebook_proof_interior_pdf` emits a
      blank verso after the Setup Page and after every logical-page recto.
- [x] Generate a complete proof interior PDF. The Rust proof generator creates the Setup Page,
      blank versos, writable rectos, deterministic logical numbering, and verified output bytes.

19 new tests in `crates/a2d-layout/src/notebook.rs`, 43 total in the crate now (up from 32),
`cargo test -p a2d-layout`. One real geometry bug caught by the tests themselves: the original
Milestone 5.2 number-placement formula (page number positioned a fixed horizontal offset right of
the QR) assumed enough page width for that offset plus the number's own width before reaching the
bottom-right marker's quiet zone — true for Letter/A4's ~210mm+ width, false for the notebook's
152mm trim, where `writable_page_layout` initially failed `validate()` with a genuine quiet-zone
violation. Fixed by moving the visible page number into its own reserved horizontal strip above
the marker/QR row (works regardless of page width) rather than composing correctly only for the
paper sizes it happened to be tested against first.

## 5.4 PDF renderer

The PDF renderer must live in Rust.

- [x] Render official `tagStandard41h12` Corner Markers without interpolation blur.
      `a2d-pdf` obtains Rust-owned marker pixels through `a2d-image`'s reviewed native wrapper and
      converts every dark module into vector PDF polygons. Stable IDs map TL=0, TR=1, BR=2, BL=3.
- [x] Render QR at an integral module scale. `render.rs::qr_ops`: encodes `qr_payload` via the
      `qrcode` crate (EC level M), then draws one filled vector square per dark module at
      `qr_rect.width / module_count` — vector fill, never a scaled raster QR image, so there's no
      resampling blur regardless of output resolution.
- [x] Render line/grid styles deterministically and with bounded work.
      `render.rs::content_style_ops` is a pure function of `ContentStyle` and `content_rect`, uses
      precomputed integer iteration counts instead of non-progressing floating-point `while`
      loops, rejects invalid spacing even if layout validation was bypassed, and enforces a
      defensive element ceiling so pathologically tiny positive spacing fails with a typed error
      instead of exhausting memory. `Lined`/`Graph` draw ruled lines; `DotGrid` draws small filled
      squares at intersections; `Blank` draws nothing.
- [x] Use legally distributable fonts or avoid embedding unlicensed fonts. `render.rs::page_number_ops`
      uses `printpdf::BuiltinFont::Helvetica` — one of the 14 standard PDF fonts every viewer/
      printer already has. No font file is embedded at all, so there is no font license to clear.
- [x] Generate single-page PDFs. `crates/a2d-pdf/src/generate.rs::generate_smart_page_pdf`
      (spec §7.5): fresh `SmartPageId`, real encoded QR payload via `a2d_identity::qr::PageCode`,
      single-page PDF.
- [x] Generate multipage Page Sets. `generate.rs::generate_page_set_pdf` (spec §7.6): one
      `PageSetId`, one fresh `SmartPageId` per page, ascending visible page numbers.
- [x] Generate the bound-notebook proof interior. `generate.rs::generate_notebook_proof_interior_pdf`:
      Setup Page + blank verso, then one writable-page + blank-verso pair per logical page, using
      Milestone 5.3's `a2d_layout::notebook` layouts. A `debug_assert_eq!` inside the loop ties
      this construction order to Milestone 5.3's independently defined
      `pdf_page_number_for_logical_page`, catching drift between the two immediately rather than
      only surfacing as a numbering bug once Milestone 6 wires up real scanning.
- [x] Write to a temp path and verify before returning success. `generate.rs::write_and_verify`:
      write → flush/close → re-read → re-parse with `PdfParseOptions { fail_on_error: true }` →
      only then atomically rename into place — the same write-then-verify-then-commit discipline
      spec §16.3 requires for asset commits, applied here to generated PDFs. Never leaves
      `output_path` pointing at unverified content.

Deviation from this task's suggested `GeneratePageSetRequest`/`PageStyle`: reuses
`a2d_layout::PaperSize`/`SmartPageStyle` (Milestone 5.2) rather than defining a duplicate
`PaperSize`/`PageStyle` pair, and omits `title`/`output_path: String` — no title-rendering region
exists in `PageLayout` yet (nothing to render it into), and `output_path` is a `&Path` parameter
rather than a struct field for this pass. 17 tests across
`crates/a2d-pdf/src/{coordinates,render,generate}.rs` (a new crate this task), `cargo test -p
a2d-pdf`.

## 5.5 Transactional generated-page registration

`A2dCore::generate_and_register_page_set` (`crates/a2d-core/src/lib.rs`) implements this for
Smart Page Sets. `A2dCore::open` now actually opens the SQLite database and asset store
(previously just created a bare directory — Milestone 3's storage existed but nothing wired it
into `A2dCore` yet). Added a new migration, `0002_page_generated_pdf_asset.sql` (0001 is
immutable per policy, so this is additive, not an edit), giving `Page` a
`generated_pdf_asset_id: Option<AssetId>` field so a generated page can remember which `Asset`
its PDF was committed as.

- [x] Create Page Set and all unique page identities transactionally. Every `PageSetId`,
      `SmartPageId`, and `PageId` involved is generated in memory first (via `a2d-pdf`'s
      `render_page_set_pdf_bytes`, refactored this task to return bytes + identities without
      touching disk, plus a thin `generate_page_set_pdf` wrapper that adds the temp-write-verify-
      rename disk path on top for standalone/CLI-style use — Milestone 5.4's file-writing API is
      unchanged, all 17 of its existing tests still pass unmodified). The `PageSet` row and every
      `Page` row are then inserted together inside one `Storage::transaction`.
- [x] Generate and verify PDF. Reuses Milestone 5.4's `render_page_set_pdf_bytes` (the QR/marker/
      ruling rendering) and the asset commit protocol's own verify step (`AssetStore::commit`
      re-hashes what it wrote before renaming into place, spec §16.3) — not a second, redundant
      verify step.
- [x] Attach the PDF asset and mark success. Every generated `Page`'s `generated_pdf_asset_id` is
      set to the committed `Asset`'s id before insertion; state is `GeneratedNotScanned`.
      Assignment is single-writer and idempotent: repeating the same `AssetId` succeeds without
      rewriting timestamps, while a different `AssetId` returns an explicit integrity conflict
      instead of silently replacing provenance. The typed storage repository enforces the same
      rule.
- [x] On failure, roll back coherently or retain an explicit failed-generation record.
      `Storage::transaction` rolls back every row from a failed attempt automatically (rollback-
      on-drop). **Documented, not fully closed gap**: if the DB transaction fails *after* the PDF
      asset was already durably committed to `assets/exports/`, that file is orphaned — no
      automated cleanup or review-item exists yet (needs Milestone 9.4/16 infrastructure that
      doesn't exist). The returned error carries the orphaned `AssetId` in its `details` so it's
      at least diagnosable, matching Milestone 3.3's own precedent of documenting this exact class
      of gap rather than building unneeded infrastructure ahead of the milestone that needs it.
      A deterministic SQLite abort-trigger test proves that the file exists while all attempted
      `PageSet`, `Page`, and `Asset` rows roll back.
- [x] Retry safely without duplicate logical records. Every ID minted is freshly random on every
      call (spec §12.2), so a retry after a failed (fully rolled-back) attempt cannot produce a
      duplicate logical record — there is nothing from the failed attempt left in the database to
      collide with, and a successful retry simply mints an entirely independent Page Set, which
      is correct behavior here, not a bug to guard against. Verified directly:
      `repeated_generation_produces_fully_independent_page_sets`.

8 new/changed tests: 2 in `a2d-storage` (`set_generated_pdf_asset_attaches_a_committed_asset_and_round_trips`,
`set_generated_pdf_asset_rejects_an_unknown_page_id`), 3 new in `a2d-core`
(`generate_and_register_page_set_persists_the_page_set_pages_and_asset` — which inspects the
result through a *second*, independent `Storage`/`AssetStore` handle opened against the same
files, the way a real second process would — plus the zero-page and independent-retry cases), all
of `a2d-core`'s existing tests still passing after `A2dCore::open` started doing real I/O.

## 5.6 PDF tests

- [x] Check page counts and metadata. Covered throughout Milestone 5.4/5.5's own tests (e.g.
      `generate_page_set_pdf_produces_one_pdf_page_and_one_unique_smart_page_id_per_page`,
      `generate_notebook_proof_interior_pdf_alternates_recto_and_blank_verso` — page counts,
      recto/verso alternation) via `PdfDocument::parse(...).page_count()` against the real
      generated bytes, not just trusting the generator's own bookkeeping.
- [x] Rasterize generated vector PDF pages in tests with the pinned pure-Rust `hayro` renderer.
- [x] Decode every committed Page Code fixture and the maximum-field printable Smart Page QR.
- [x] Detect all four official `tagStandard41h12` Corner Markers in full-page rasters.
- [x] Verify marker centers against physical-layout coordinates within a deterministic 2mm tolerance.
- [x] Simulate and validate 95%, 100%, and 105% print scaling.
- [x] Test truncated/corrupt output behavior. `write_and_verify_rejects_truncated_bytes_and_never_creates_the_output_path`
      (`crates/a2d-pdf/src/generate.rs`): truncates a real generated PDF's bytes to half length,
      confirms `write_and_verify` rejects it with `PDF_VERIFY_FAILED` and never creates
      `output_path` — the one bullet in this task achievable without the rasterizer or a real
      detector.

Acceptance:

- [ ] A generated page can be printed, photographed, identified, and rectified using bundled
      metadata. Deterministic PDF raster, QR, marker, position, and print-scale compatibility is
      complete; real printer/paper/camera evidence remains Milestones 7 and 17 work.

---

# Milestone 6 — Notebook and Smart Page workflows

**Status:** Complete — 2026-07-27

## 6.1 Rust Notebook service

Implemented in `crates/a2d-core/src/milestone6.rs`, with typed persistence operations in
`crates/a2d-storage` and UniFFI projections in `crates/a2d-ffi/src/milestone6.rs`.

- [x] `resolve_notebook_setup_code`
- [x] `create_notebook`
- [x] `rename_notebook`
- [x] `archive_notebook`
- [x] `list_notebooks`
- [x] `get_notebook`
- [x] `set_active_notebook`
- [x] `get_active_notebook`

Rules:

- [x] Multiple notebooks may share one design. Every registration mints a fresh `NotebookId` and
      an independent set of persistent logical-page identities.
- [x] Names need not be unique. Persistence and lookup use typed identifiers, not display names.
- [x] IDs are unique through cryptographic ID generation and database primary-key constraints.
- [x] Active Notebook is explicit persistent state. Migration 0003 adds a partial unique index so
      at most one non-archived Notebook can be the active scan destination.
- [x] The UI never silently changes the active Notebook. Selection and clearing are explicit user
      actions, and conflicts are returned as typed Rust results.

Notebook creation is transactional: the design, physical Notebook, logical page slots, and optional
active selection commit together or roll back together. Tests cover separate physical copies of one
design, duplicate display names, active-selection persistence, archiving, and rollback.

## 6.2 Page resolution

`A2dCore::resolve_page_code` owns the identity and destination rules. Android sends decoded text to
Rust and displays the typed `PageResolution`; it does not duplicate the QR grammar or infer a
Notebook locally.

- [x] Resolve a Smart Page by unique ID.
- [x] Resolve a Notebook Page only through a matching Notebook Design.
- [x] Return `RequiresNotebookSelection` when several physical Notebooks match and none is
      explicitly confirmed.
- [x] Return `ConflictingActiveNotebook` when the confirmed or active Notebook uses another design.
- [x] Return `ImportedUnknownSmartPage` for a valid but locally unknown Smart Page.
- [x] Never auto-create a physical Notebook from an ordinary Page Code.
- [x] Return `RequiresNotebookRegistration` when the design is recognized but no physical Notebook
      instance exists.
- [x] Return `UnsupportedCode` for Setup Codes used as page identifiers, unavailable designs,
      invalid logical page numbers, and incompatible layouts.

The variants cross UniFFI without being flattened into untyped success strings. Domain, storage,
core, and FFI tests cover resolved, ambiguous, conflicting, registration, unknown-import, and
unsupported outcomes.

## 6.3 Android Notebook UI

Implemented under `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/notebook/`.

- [x] Add a Notebook Setup Code scanner. Android captures a bounded still image, ZXing decodes QR
      text locally, and Rust performs canonical validation.
- [x] Notebook Design recognized screen.
- [x] Name/customize Notebook with optional color, icon, notes, and active-selection choice.
- [x] Created confirmation and first-page Page Code resolution action. Full-page camera capture
      remains correctly scoped to Milestones 8 and 9.
- [x] Unsupported/invalid Setup Code state remains visible as a typed Rust error.
- [x] Multiple-copy explanation and explicit “add another copy” path.
- [x] Active Notebook selector, clear action, rename, and archive controls.

`NotebookViewModel` owns presentation state and dispatch only. Identity, persistence, ambiguity,
conflict, and creation rules delegate to `A2dClient`. Camera and QR failures are surfaced explicitly;
they are never converted into successful scans.

## 6.4 Smart Page UI

Implemented under `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/`.

- [x] Create Smart Pages landing screen.
- [x] Single-page form.
- [x] Page-set form with a bounded 1–500 page count.
- [x] PDF preview using Android `PdfRenderer`.
- [x] Android print, Storage Access Framework save-copy, and content-URI share integration.
- [x] Generated page/set detail showing the Page Set ID and unique page-identity count.
- [x] Failed generation state with explicit retry. Retry calls Rust generation again, so a failed
      attempt never reuses identities or presents an incomplete output as successful.

`SmartPagesViewModel` delegates generation, identity creation, PDF registration, and validation to
Rust. Android owns only form presentation, preview, and platform print/save/share operations.

Acceptance:

- [x] A user can register two identical physical Notebook copies separately. Rust tests and the
      repeated registration flow prove independent Notebook and page identities for one design.
- [x] A user can generate a unique Smart Page offline without an account. The path uses local Rust,
      SQLite, and private asset storage without a network or account service.

Validation evidence:

- GitHub Actions run `30289813553` completed successfully on 2026-07-27.
- GitHub Actions run `30300119881` completed successfully on 2026-07-27.
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo deny check`
- Android lint, unit tests, and debug assembly
- Android NDK cross-build and Kotlin UniFFI binding-drift verification

---

# Milestone 7 — Marker detection and image-processing foundation

## 7.1 Complete a working detector spike

- [x] Evaluate the official AprilTag 3 native library. The pinned official implementation through
      `apriltag-sys = 0.4.0` is integrated behind the safe `a2d-image` API.
- [x] Confirm license compatibility and commit the review. See
      `docs/reviews/APRILTAG_LICENSE_REVIEW_2026-07-27.md`.
- [x] Build reproducibly for required Android ABIs. CI pins Android NDK `27.0.12077973` and
      `cargo-ndk 4.1.2` and builds `arm64-v8a` plus `x86_64` directly against `a2d-image`.
- [x] Wrap ownership and errors safely for Rust. Native detector, family, image, and detection-array
      lifetimes are private RAII guards; public results own their data and failures are typed.
- [ ] Complete representative photographed-fixture and physical Android device-tier measurements.
      The deterministic synthetic corpus now has calibrated regression envelopes and runs through
      desktop Rust plus the packaged `x86_64` Android integration boundary. Photographed fixtures and
      physical `arm64-v8a` measurements remain required before this evidence gate is complete.
- [x] Confirm future iOS build feasibility. The same crate compiles for `aarch64-apple-ios` and
      `aarch64-apple-ios-sim`; this is compile feasibility only, not iOS application work.
- [x] Compare a pure-Rust alternative only if it materially reduces packaging risk. The contingency
      was reviewed and not triggered because the pinned official implementation cross-compiles for
      every required native target; a comparison remains conditional on later material risk.
- [ ] Accept `docs/decisions/0002-apriltag-detector-selection.md`, naming the selected
      implementation and recording license review, Android ABI build results, fixture results,
      performance measurements, the memory-safety boundary, packaging strategy, and future iOS
      feasibility. The ADR remains Proposed until representative photographed fixtures and physical
      Android device-tier performance measurements are committed. APK packaging, third-party notices,
      synthetic fixture envelopes, and emulator runtime loading are already proven.

The spike ends with code, tests, pinned CI, and committed evidence rather than prose only.

## 7.2 Image input types

- [x] Define width, height, row stride, pixel format, rotation, and buffer ownership.
- [x] Support reduced grayscale analysis frames through borrowed validated `GrayFrame` input.
- [x] Support bounded JPEG and PNG full-resolution image files for final processing.
- [x] Reject impossible dimensions, strides, truncated rows, format mismatches, and output-size
      overflows with structured errors.
- [x] Enforce caller-selected encoded-byte, decoded-byte, and decoded-pixel limits before accepting
      full-resolution output.
- [x] Avoid Base64. Image boundaries use borrowed or owned byte buffers directly.

Implemented inputs expose borrowed Gray8 analysis frames plus owned RGB8/Gray8 decoded images.
Encoded files require an explicit declared format and limits; there is no format-guessing fallback.

## 7.3 Detection results

Return marker ID, four corners, center, decision margin, and error-quality data.

- [x] Validate expected `tagStandard41h12` family.
- [x] Resolve marker semantic roles from the selected layout.
- [x] Reject duplicate tag IDs and duplicate semantic roles.
- [x] Resolve page orientation from the semantic top edge.
- [x] Preserve center, four corners, decision margin, and Hamming-error quality data.

## 7.4 QR decoder boundary

- [x] Decide that Android performs bounded local QR image decoding while Rust remains the canonical
      payload trust boundary. See `docs/decisions/0003-qr-image-decoder-boundary.md`.
- [x] Always send decoded payload text to Rust for canonical grammar, version, bounds, layout, and
      CRC validation.
- [x] Add legally usable blur, rotation, scale, glare, distortion, and controlled-damage cases for
      the shipped ZXing decoder. Deterministic JVM conformance tests exercise pixel-buffer decoding
      separately from Android file I/O, while Rust remains the canonical payload trust boundary.
- [x] Never accept a decoder result without Rust validation; decoder success alone cannot trigger an
      A2D identity or workflow success state.

Validation evidence for the reconciled work:

- GitHub Actions native run `30326733326` completed successfully on 2026-07-28.
- GitHub Actions full CI run `30326733320` completed successfully on 2026-07-28.
- Android ZXing conformance tests cover scale, rotation, blur, glare, distortion, and controlled damage.
- Rust formatting, clippy with warnings denied, the full workspace test suite, and dependency/license
  checks passed.
- Android lint, unit tests, debug APK assembly, Kotlin UniFFI binding drift, both packaged native ABIs,
  detector linkage, and packaged notices passed verification.
- Future Apple device/simulator compile-feasibility checks passed; no iOS application work is claimed.

## 7.5 Homography and rectification

- [x] Compute a normalized projective transform from four ordered source/destination
      correspondences with partial-pivot Gaussian elimination and verified reprojection.
- [x] Validate transform conditioning through finite geometry checks, normalization, singular-pivot
      rejection, pivot-ratio rejection, matrix inversion checks, and finite projection checks.
- [x] Reject zero-length, collinear, self-intersecting, non-convex, negligible-area, non-finite, and
      source-out-of-bounds quadrilaterals with structured errors.
- [x] Warp borrowed Gray8 or owned RGB8 input to caller-selected canonical dimensions using bounded
      inverse mapping and bilinear interpolation.
- [x] Preserve source/destination page corners, optional source/destination marker centers, forward
      and inverse matrices, source dimensions, output dimensions, and solve pivot ratio.
- [x] Add deterministic reference-output tests for Gray8/RGB8 warps plus identity, perspective,
      semantic marker/layout, invalid-geometry, source-mismatch, and memory-limit cases.
- [x] Prevent out-of-bounds access on malformed inputs through validated source geometry, exact
      buffer construction, checked output limits, bounded numerical epsilon, and structured sample
      rejection rather than clamping arbitrary invalid coordinates.

`RectificationPlan::from_page_markers` uses Rust-resolved semantic marker roles and the physical
`PageLayout` marker centers to derive the canonical transform. Output is upright (`Degrees0`), and
no production page resolution or quality threshold is invented by this layer.

Validation evidence:

- GitHub Actions native run `30311792736` passed pinned Android `arm64-v8a`/`x86_64` builds and
  future Apple device/simulator compile-feasibility checks.
- GitHub Actions full CI run `30311792705` passed Rust formatting, workspace clippy with warnings
  denied, the full workspace test suite, dependency/license checks, and UniFFI binding drift.
- Android lint, unit tests, and debug APK assembly are tracked by the same full CI run.

## 7.6 Quality metrics

Implement raw metrics and classifications for:

- [x] Blur/focus through variance of the four-neighbor luminance Laplacian over interior pixels;
      frames too small to support that measurement report focus as unavailable rather than zero.
- [x] Underexposure through mean luminance and dark-pixel fraction.
- [x] Overexposure through mean luminance and highlight-clipped pixel fraction.
- [x] Glare/highlight clipping through both global highlight fraction and the worst populated tile in
      a caller-selected bounded grid, preserving localized glare that a global average can hide.
- [x] Page fill/framing through quadrilateral area fraction, four normalized border margins, minimum
      margin, and page-center offset.
- [x] Marker confidence through minimum/mean decision margin, maximum Hamming errors, and unexpected
      tag count; invalid native quality values remain structured capture-quality failures.
- [x] Perspective severity through edge-length ratio, opposing-edge imbalance, diagonal imbalance,
      and quadrilateral-to-bounding-box area ratio.
- [x] Effective content resolution through conservative source and canonical-output pixels per
      physical millimeter using the validated `PageLayout` size.
- [x] Possible curvature through explicit edge probes and normalized perpendicular deviation. With
      no probes, curvature remains unavailable; four corners alone never fabricate a flat-page result.

Thresholds live in an explicit nonzero-version `QualityPolicy`; the library supplies no default
production policy. Scalar and nested-band threshold ordering/direction are validated before use.
Callers declare which measurements are required. Missing required metrics classify as `NeedsReview`,
missing optional metrics remain visibly `Unavailable`, and a completely unevaluated capture resolves
to `NeedsReview` rather than `Accepted`.

Classification preserves the raw measurements, per-metric state, policy version, and one of the
specification states: `Accepted`, `AcceptedWithWarnings`, `NeedsReview`, or `Rejected`.

Validation evidence:

- GitHub Actions quality run `30312884513` passed canonical formatting, clippy with warnings denied,
  and all `a2d-image` tests before the formatted source was committed.
- GitHub Actions native run `30312966395` passed pinned Android `arm64-v8a`/`x86_64` builds and future
  Apple device/simulator compile-feasibility checks with the quality module included.
- GitHub Actions full CI run `30312966424` passed workspace formatting, clippy/tests,
  dependency/license checks, and UniFFI binding-drift validation for the clean quality implementation.

## 7.7 Derived images

- [x] Produce a new owned corrected RGB8 image through the validated `RectificationPlan`; output is
      upright and does not alias platform-owned or original capture memory.
- [x] Produce a new owned OCR-optimized Gray8 image through deterministic RGB luminance conversion,
      conservative contrast normalization, and optional bounded sharpening.
- [x] Produce an aspect-preserving RGB8 thumbnail with caller-selected maximum dimensions and no
      automatic upscaling.
- [x] Use explicit low/high histogram percentiles and a caller-selected maximum gain for contrast
      normalization; flat or already-wide inputs remain unchanged rather than forcing enhancement.
- [x] Apply sharpening only when explicitly configured, with validated positive amount, pixel-detail
      threshold, and a bounded pass count.
- [x] Preserve a nonzero pipeline version plus source/output dimensions, source rotation, homography
      matrix, applied contrast values, and sharpening configuration in result provenance.
- [x] Preflight per-image pixel/byte limits, total output bytes, platform addressability, and a
      conservative peak working-set estimate before allocating derived outputs.
- [x] Check shared cancellation state before every major stage and between sharpening passes. Partial
      buffers remain in memory only and are dropped on failure/cancellation; no partial result is
      returned or persisted by this layer.
- [x] Borrow the original `OwnedRgbImage` immutably and create separate owned outputs. Tests verify
      the original bytes and rotation remain unchanged.

`DerivedImagePipeline` is intentionally file-system agnostic. Atomic file publication and durable
rollback belong to the storage/worker transaction boundary; this image layer cannot overwrite the
original capture because it receives no path and performs no writes.

Validation evidence:

- GitHub Actions derived validation run `30313630025` applied the single reviewed slice-API fix,
  passed clippy with warnings denied and all `a2d-image` tests, and committed only formatted Rust
  source.
- GitHub Actions native run `30313769678` passed pinned Android `arm64-v8a`/`x86_64` builds and
  future Apple device/simulator compile-feasibility checks on the clean permanent workflow.
- GitHub Actions full CI run `30313769680` passed workspace formatting/clippy/tests,
  dependency/license checks, Android lint/tests/debug APK assembly, and UniFFI binding drift.

## 7.8 Fixture corpus

The deterministic synthetic foundation is committed under:

```text
fixtures/scans/
├── generated/
├── photographed/
├── glare/
├── blur/
├── missing-marker/
├── wrong-layout/
├── duplicate/
├── revisions/
└── corrupted/
```

- [x] Generate a content-bearing notebook page with the pinned official `tagStandard41h12` marker
      family and a canonical A2D Notebook Page QR payload. The fixture-only Rust helper renders the
      official tags and uses the existing `PageCode` encoder; Python does not reimplement either wire
      format.
- [x] Generate deterministic rotation, perspective, underexposure, overexposure, Gaussian blur,
      localized glare, missing-marker, wrong-layout QR, wrong marker-set, duplicate-marker, revision,
      truncated-image, and non-image controls with pinned Pillow `11.3.0`.
- [x] Record project-generated/Apache-2.0 provenance, expected QR identity, expected marker roles,
      intended quality state, warnings, transformation parameters, dimensions, mode, byte length, and
      SHA-256 in `fixtures/scans/manifest.json`.
- [x] Keep `fixtures/scans/photographed/` separate and protected from synthetic regeneration. Synthetic
      controls never claim photographed provenance.
- [x] Verify every manifest entry, digest, dimension, decode-success/decode-failure expectation, category,
      and provenance field through `tools/verify_scan_fixtures.py`.
- [x] Regenerate the corpus in read-only CI and compare every generated category plus the manifest
      byte-for-byte against committed assets so generator drift fails explicitly.
- [x] Add calibrated processing tolerances for each relevant synthetic fixture. Versioned entries in
      `fixtures/scans/processing-expectations.tsv` enforce fixture-specific dimensions, luminance/focus
      ranges, detector counts and IDs, decision-margin/Hamming bounds, and explicit resolution/error
      behavior. These are deterministic synthetic regression envelopes, not production quality thresholds
      or physical-camera tolerances.
- [ ] Add real photographed Android fixtures with source/consent/license, device, camera, lighting,
      print/page, and capture-condition metadata.

Acceptance:

- [x] A desktop Rust integration test processes all generated PNGs without Android through bounded
      encoded-image decoding, Gray8 conversion, and quality measurement; detects/resolves marker IDs
      `0..=3` on the canonical page; and confirms corrupt controls fail without fabricated success.
- [x] Android calls the same shared processing path. An Android instrumentation test reads the canonical
      synthetic PNG from the test APK and executes the typed Kotlin façade, generated UniFFI/JNA binding,
      packaged `x86_64` Rust library, bounded decoder, Gray8 conversion, official AprilTag detector,
      semantic marker resolution, and quality measurement without mocked detector results.

Validation evidence:

- GitHub Actions native/fixture run `30326733326` passed deterministic regeneration and byte-drift
  comparison, synthetic processing envelopes, raw-measurement artifact generation, pinned Android
  `arm64-v8a`/`x86_64` native builds, and future Apple compile-feasibility checks.
- GitHub Actions full CI run `30326733320` passed workspace formatting/clippy/tests,
  dependency/license checks, Android lint/tests/debug APK assembly, Kotlin binding drift, APK native
  ABI/linkage/notices verification, and the packaged shared-Rust Android emulator integration test.

Remaining Milestone 7 evidence:

- [ ] Commit real photographed Android fixtures with source/consent/license and device/capture metadata.
- [ ] Record representative physical `arm64-v8a` runtime, latency, and memory measurements across the
      supported Android device tiers.
- [ ] Recalibrate any production capture policy only from physical evidence; do not promote synthetic
      regression envelopes into production thresholds.
- [ ] Move ADR 0002 to Accepted after the physical evidence gates above pass.

---

# Milestone 8 — CameraX scanning

## 8.1 Camera adapter

- [x] CameraX preview. `CameraPreviewSurface` hosts `PreviewView` in Compose and binds only after
      view attachment so initial display rotation is authoritative without a delayed callback that
      can outlive disposal.
- [x] Image analysis. `CameraXAdapter` binds a YUV `ImageAnalysis` use case and emits owned,
      tightly packed luminance frames through explicit success/failure events.
- [x] Full-resolution capture. `ImageCapture` writes only to a caller-selected new staging file;
      existing files are rejected rather than overwritten silently.
- [x] Correct lifecycle binding. Preview, analysis, and capture bind together through
      `bindToLifecycle`; disposal and lifecycle destruction invalidate stale work, clear the
      analyzer, unbind use cases, and close the analysis executor.
- [x] Permission-denied handling. The Compose permission state distinguishes not requested,
      retryable denial, permanent denial, and granted states, with an explicit application-settings
      action.
- [x] Background/foreground recovery. CameraX lifecycle binding owns stop/start transitions, while
      permission state is refreshed on lifecycle resume.
- [x] Rotation handling. Preview, analysis, and capture target rotations are validated and updated
      together; invalid rotations surface an explicit adapter error.
- [x] Torch control where available. Torch requests reject absent flash hardware explicitly and
      stale asynchronous completions cannot publish state after rebind or closure.
- [x] Bounded analysis backpressure. Analysis uses `STRATEGY_KEEP_ONLY_LATEST` on a dedicated
      single-thread executor.
- [x] Reliably close every frame. Every `ImageProxy` is closed on success and recoverable failure;
      fatal JVM errors propagate only after frame closure. Unit tests cover row/pixel stride, crop,
      source buffer limits, ordinary failures, close failures, and fatal-error propagation.

Validation evidence:

- GitHub Actions CameraX adapter run `30339511067` passed workspace formatting/clippy/tests,
  permanent printable QR/page compatibility regeneration, both packaged Android native-library
  ABIs, Android lint and JVM tests, debug APK assembly, and APK native packaging verification.
- Milestones 8.2 and 8.3 now complete live shared-Rust frame analysis, copy/latency
  instrumentation, stale-work cancellation, marker/page overlays, active Notebook presentation,
  actionable guidance, strict identity gating, and the explicit auto-capture state machine.
  Single-page and batch scanner UI integration remain in Milestones 8.4 and 8.5; physical
  printer/camera and representative Android-device evidence also remain open.

## 8.2 Live Rust/native analysis

### 8.2A Frame transport, shared analysis, and scheduling

- [x] Extract luminance efficiently. `CameraFrameAnalyzer` performs exactly one owned copy from
      CameraX's cropped Y plane into a tightly packed read-only direct `ByteBuffer`, honoring source
      row stride, pixel stride, crop bounds, and source buffer limits without retaining `ImageProxy`.
- [x] Pass dimensions/stride/orientation correctly. The Android bridge passes width, height, packed
      row stride, and validated 0/90/180/270-degree rotation to the borrowed
      `a2d_live_analyze_gray_frame` ABI; Rust revalidates every scalar before constructing `GrayFrame`.
- [x] Measure copies and latency. Per-frame metrics record source and packed geometry, input byte
      count, one CameraX pixel-buffer copy, zero FFI input copies, one small result-payload copy,
      extraction time, queue time, native bridge time, scheduler time, and extraction-to-completion
      latency. Representative physical-device latency evidence remains open under Milestone 7.
- [x] Analyze off the main thread. CameraX extraction and synchronous Rust analysis use separate
      dedicated single-thread executors; no native detector or quality work runs on Android's main
      thread.
- [x] Cancel stale work. `LatestFrameAnalysisScheduler` permits one in-flight analysis and one
      keep-latest pending frame, reports superseded pending frames explicitly, and discards in-flight
      completions after a newer frame, cancellation, close, or policy replacement before they can
      update scanner state.

Validation evidence:

- Rust tests exercise the borrowed canonical Gray8 page through AprilTag detection, marker-role
  resolution, quality measurement, the versioned result/error codec, null-pointer validation, panic
  containment, and invalid-configuration rejection.
- Kotlin JVM tests cover direct-buffer ownership, crop/row/pixel-stride handling, one-copy accounting,
  off-caller-thread execution, keep-latest behavior, stale-result suppression, cancellation, explicit
  failures, and closed-scheduler submission rejection.
- Android instrumentation runs the canonical synthetic page through the packaged x86_64
  `liba2d_ffi.so` using the borrowed direct-buffer ABI and verifies marker identities, orientation, and
  quality output. `tools/verify-android-apk.py` also requires both live-analysis symbols in arm64-v8a
  and x86_64 APK libraries.
- GitHub Actions run `30354250368` passed Rust formatting/clippy/tests, both Android native ABIs,
  Kotlin binding drift, Android lint/JVM tests, APK verification, and the packaged shared-Rust emulator
  test. Its cargo-deny job identified only the missing permissive `Zlib` allowlist entry for transitive
  `foldhash`; `deny.toml` now documents/allows `Zlib`, and pinned `cargo-deny 0.20.2` subsequently
  completed with warnings only and no rejected dependency or license.

### 8.2B Scanner presentation and safety gating

- [x] Render page/marker overlay. `LiveScannerPreview` layers reusable Compose scanner chrome over
      `CameraPreviewSurface`; `LivePageMarkerOverlay` draws the resolved page boundary and each
      marker quadrilateral. `PreviewCoordinateMapper` matches CameraX `FILL_CENTER` cropping and
      maps 0/90/180/270-degree source rotations explicitly.
- [x] Show active Notebook prominently. The top scanner banner continuously displays the active
      Notebook name and Notebook Design. A missing destination is a prominent blocking state rather
      than an empty label or an inferred fallback.
- [x] Show actionable guidance. `buildLiveScannerPresentation` maps typed Rust/native marker,
      geometry, quality, analysis-error, and Page Code resolution results into explicit guidance such
      as show all corners, move closer/farther, hold steady, add light, reduce glare, select/register
      a Notebook, or use a supported page. Guidance thresholds are caller-supplied presentation
      policy only; no synthetic threshold is hidden as an authoritative production capture rule.
- [x] Block auto-capture on identity conflict. `IdentityAutoCaptureGate` allows eligibility only when
      Rust returns `PageResolution.Resolved` with a Notebook ID exactly equal to the displayed active
      Notebook. Missing identity, mismatches, ambiguity, required registration, imported Smart Pages,
      unsupported codes, and `ConflictingActiveNotebook` all remain explicitly blocked. The gate is
      ready for the Milestone 8.3 state machine and never changes the destination silently.

Validation evidence:

- Kotlin JVM tests cover exact-match eligibility, mismatches, Rust conflict/ambiguity/registration
  variants, missing destination/Page Code, marker completeness, framing, focus, lighting, glare,
  invalid presentation policies, and `FILL_CENTER` coordinate mapping with rotation.
- Android emulator Compose tests verify that the active Notebook remains visible, a wrong-Notebook
  result displays blocking guidance, auto-capture is visibly blocked, and a verified identity does
  not change the displayed destination.
- One-use validation completed the packaged x86_64 Rust build, Android lint, JVM tests, debug APK
  assembly, and the scanner-presentation emulator tests, then removed itself in commit
  `14febb773d0d0c6766ab4263cd58d5eebb4e7fa5`.
- A separate current-`master` validation attempt also passed workspace Rust/fixture gates, both
  packaged Android ABIs, Android lint/JVM tests, debug APK assembly, and APK packaging verification;
  only its obsolete cleanup push failed because the successful 8.2B validation commit had already
  advanced `master`.

## 8.3 Auto-capture state machine

`AutoCaptureStateMachine` is a synchronized, Android-object-free controller under
`feature/scanner/capture/`. It exposes the explicit `Idle`, `Searching`, `CandidateStable`,
`Capturing`, `Processing`, `Accepted`, `NeedsReview`, `Rejected`, and `Paused` phases and emits
explicit effects for camera capture, manual confirmation, debounce, cancellation, and stale callback
handling.

- [x] Require stable acceptable frames for a configured interval. A candidate must retain the same
      Rust-resolved Page ID, pass the strict `IdentityAutoCaptureGate`, and carry an explicit
      caller-supplied capture-policy approval across the configured monotonic interval. Excessive
      inter-frame gaps, Page ID changes, identity failures, or rejected policy assessments restart
      searching rather than inheriting prior stability. Presentation guidance thresholds are not
      silently promoted into capture acceptance.
- [x] Debounce repeated captures. A successful full-resolution capture records a per-page monotonic
      debounce window. The same page cannot immediately auto-capture again, while a different Page ID
      remains eligible for batch workflows. An explicit retake action may clear the same-page debounce
      after a rejected or review result.
- [x] Permit manual capture only through an explicit warning path. Manual capture first enters
      `Paused(AWAITING_MANUAL_CONFIRMATION)` and emits warning codes for bypassed stability, rejected
      capture policy, and recent-page repetition. Only the matching confirmation token starts a manual
      capture. Manual capture can never override missing, ambiguous, conflicting, or wrong-Notebook
      identity.
- [x] Cancel safely on navigation. Navigation increments the machine generation, enters
      `Paused(NAVIGATION)`, emits cancellation for active capture/processing work, clears candidate
      frames and pending warnings, and ignores every late tokened callback from the previous
      generation. Resume retains the explicit scan context but requires fresh frames.
- [x] Recover from capture failure without losing context. A matching camera failure returns to
      `Searching`, preserves the active Notebook/session context, exposes the typed failure, clears the
      stale frame, and permits a new stability interval and capture request. Successful captures move
      through `Processing` to explicit `Accepted`, `NeedsReview`, or `Rejected` outcomes.

Validation evidence:

- Kotlin JVM tests cover continuous stability timing, excessive frame gaps, Page ID changes,
  capture-policy rejection, same-page debounce, different-page progress, explicit retakes, manual
  warning confirmation/dismissal, identity-conflict denial, capture failure recovery, all terminal
  processing outcomes, navigation cancellation, generation invalidation, stale capture/processing
  callbacks, stop behavior, invalid policy, and non-monotonic frame rejection.
- GitHub Actions run `30339511067`, validation attempt job `90277827109`, passed workspace
  Rust/fixture gates, both packaged Android native ABIs, Android lint and JVM tests, debug APK
  assembly, APK native-symbol/notices verification, and one-use workflow cleanup on 2026-07-28.
- Milestone 8.4 now wires these effects to CameraX staging capture, final Rust processing, and
  explicit review as documented below. Milestone 8.5 still owns batch-session behavior.

## 8.4 Single-page scanner

- [x] Active Notebook selector. The scanner loads Rust-owned Notebook summaries, displays the active
      destination, persists an explicit change through `setActiveNotebook`, requires Rust to return at
      most one active Notebook, resets the camera generation on destination change, and locks the
      selector while capture processing or review is active.
- [x] Camera preview. `SinglePageScannerScreen` owns permission and CameraX lifecycle state, while
      `CameraPreviewSurface` remains the live preview surface. CameraX objects never enter the
      ViewModel.
- [x] Marker/QR status. One owned CameraX luminance frame fans out to independent keep-latest Rust
      marker/quality analysis and bounded ZXing Page Code decoding. QR text remains untrusted until
      Rust returns a typed `PageResolution`; missing codes, decoder failures, conflicts, ambiguity,
      required registration, unsupported codes, and stale results remain distinct.
- [x] Capture guidance. The screen reuses the Milestone 8.2 overlay, active-destination banner,
      actionable guidance, and strict identity gate. Live analysis and Page Code freshness are
      combined without silently inheriting an old identity.
- [x] Manual capture. The Milestone 8.3 state machine remains authoritative: capture requires a bound
      camera, active Notebook, current Rust-resolved page identity, and explicit confirmation when
      stability or calibrated capture-policy checks are bypassed. Tokened staging paths and callbacks
      reject stale captures.
- [x] Torch. The screen exposes CameraX torch control only when hardware reports it available and a
      capture is not processing; adapter failures remain explicit.
- [x] Corrected preview. A versioned native ABI borrows the encoded JPEG, catches panics, applies
      explicit decode/detector/resource policies, reruns AprilTag and quality analysis, rectifies with
      the shared `writable_page_layout`, executes the bounded Rust derived-image pipeline, and returns
      corrected RGB plus thumbnail buffers through a strictly decoded and explicitly freed payload.
- [x] Accept/save/retake/details. Review displays the corrected image, selected Notebook, captured
      Page ID, pipeline version, identity result, quality warnings, and retake/details actions. Save
      Scan sends the retained full-resolution Page Code payload, final marker roles/IDs, EXIF rotation,
      capture timestamp, warnings, fixed Page ID, and fixed Notebook ID to the typed Rust registration
      API. The UI says “Scan saved” only after Rust returns a `RegisteredScan`; failed registration
      preserves the staging file and remains explicitly retryable.
- [x] Warning details. Quality warnings may be reviewed, but identity is non-overridable. Approval is
      enabled only when final full-resolution Page Code resolution exactly matches both the capture
      request Page ID and its fixed active Notebook ID.
- [x] Processing progress/cancel. Full-resolution work runs off the main thread with a Rust
      cancellation token. `close()` requests cancellation immediately but defers native-token freeing
      until every synchronous JNA borrower returns, preventing stale completion and use-after-free;
      cancellation, rejection, cleanup failure, and processing failure remain explicit.
- [x] Durable registration progress. Navigation, Notebook switching, retake, and duplicate save
      requests are blocked while synchronous Rust registration is active. ViewModel cleanup never
      deletes the staging capture while registration is in progress, and registration success is
      always surfaced rather than discarded by a stale presentation-generation check.

Validation evidence:

- Rust tests cover cancellation as a distinct outcome, panic/error containment, result/error codec
  framing, bounded image payloads, and the shared image-processing path. Kotlin JVM tests cover live
  QR rotation/scheduling/cancellation, exact final page-and-Notebook matching, camera readiness,
  unsigned RGB conversion, navigation routes, and the existing capture/presentation controllers.
- GitHub Actions run `30339511067`, job `90339478476`, passed workspace Rust formatting, clippy and
  tests, printable fixture regeneration, arm64-v8a and x86_64 Android native builds, Android lint and
  JVM tests, debug APK assembly, and APK native packaging verification on 2026-07-28.
- `tools/verify-android-apk.py` permanently requires the live-analysis and full-resolution preview,
  buffer-free, cancellation, and durable-registration binding symbols in both packaged ABIs.
- GitHub Actions run `30403855033` applied the registration lifecycle hardening, then passed focused
  Android unit tests and lint before publishing the clean source commit.
- Physical-device usability/performance evidence and calibrated automatic-capture thresholds remain
  open; production automatic capture is therefore still disabled rather than relying on an invented
  threshold.

## 8.5 Batch scanner

- [ ] Keep active Notebook fixed until explicitly changed.
- [ ] Save and return immediately to camera.
- [ ] Queue final processing/OCR.
- [ ] Show nonblocking saved confirmation.
- [ ] Detect duplicate page.
- [ ] Show session summary and review items.
- [ ] Survive activity recreation without duplicate registration.

## 8.6 Camera tests

- [ ] Permission denied/permanently denied.
- [ ] Camera unavailable.
- [ ] Background during capture.
- [ ] Process killed after capture before registration.
- [ ] Rotation during analysis.
- [ ] Rapid repeated capture.
- [ ] Wrong-design page.
- [ ] Two identical Notebook candidates.
- [ ] Batch out of order.
- [ ] Low storage.

Acceptance:

- [x] UI does not claim a page is saved until Rust confirms durable registration. The corrected
      review remains explicitly unsaved until `A2dClient.registerScan` succeeds, and only the returned
      typed `RegisteredScan` populates the saved result, warnings, and required actions.
- [x] Scanner never silently changes destination Notebook. Selection is explicit, Rust-persisted,
      generation-scoped, fixed in every capture request, and rechecked against final Page Code identity.

---

# Milestone 9 — Durable scan registration and revisions

## 9.1 Final scan registration

The Android review artifact retains the Rust-resolved Page ID and Notebook, full-resolution Page Code
payload, final marker roles/IDs, image format and EXIF rotation, capture source and timestamp, and
explicit preview warnings. Rust treats every field as untrusted input and reopens, revalidates, and
reprocesses the staged image before any durable success is possible.

- [x] Validate staging path. Rust accepts only a regular, non-symlink file canonicalized beneath the
      library-owned `tmp/scanner-staging/` directory, bounds encoded size, rejects concurrent file
      changes, and never deletes an external or invalid source path.
- [x] Reparse and validate Page Code. Rust reparses canonical grammar/version/layout/CRC and rejects an
      unresolved, conflicting, or changed Page Code before committing assets.
- [x] Validate markers against layout. Full-resolution AprilTag detection and semantic role resolution
      rerun in Rust; exactly one TL/TR/BR/BL marker set must match the reviewed marker identities.
- [x] Resolve page identity. The reparsed code must resolve to the exact approved Page ID and fixed
      active Notebook, and the stored page record/layout must still agree inside the registration
      transaction.
- [x] Process images. Rust performs bounded decode, quality measurement, rectification, corrected-color
      generation, OCR-image generation, and thumbnail generation using the shared versioned pipeline.
- [x] Register files and database records transactionally. Original and derived assets are committed
      through the immutable asset store under a durable append-only filesystem journal; asset rows,
      scan row, page transition, preferred-scan invariant, and audit event commit in one SQLite
      transaction. Interrupted or failed commits retain the journal and staging path for explicit
      recovery and never return a saved result.
- [x] Return typed warnings and required actions. Success returns IDs and resolved paths for all assets,
      quality status, preferred/version status, typed quality/cleanup warnings, and explicit actions
      for existing-page review or incomplete cleanup.

Android integration:

- [x] CameraX writes directly into Rust's private scanner-staging directory rather than an unrelated
      cache directory.
- [x] Save Scan calls the generated UniFFI registration API off the main thread and shows durable
      success only after Rust returns.
- [x] Registration failure leaves the reviewed capture available for retry; retake deletes only an
      unregistered staging file and never touches committed originals.
- [x] Registration blocks destination changes and navigation, while lifecycle cleanup preserves a file
      that may still be borrowed by synchronous Rust registration.

Validation evidence:

- Rust tests cover first registration, preferred-page transition, existing-page rescan preservation,
  staging-path confinement, changed marker rejection, and retained interruption journals.
- Kotlin JVM tests cover request/rotation/marker/warning mapping and the tokened transition from review
  to accepted only after durable registration.
- GitHub Actions run `30403855033` passed focused Android unit tests and lint after applying the final
  lifecycle hardening. Permanent PR CI remains the merge gate for workspace Rust checks, dependency
  policy, both Android ABIs, binding drift, lint/unit/APK verification, and emulator integration.

Milestones 9.2–9.5 remain open; this section does not claim fingerprint comparison, revision decisions,
Needs Review resolution APIs, or version UI.

## 9.2 Fingerprints and comparison

- [x] Cryptographic asset hash. The immutable corrected asset's verified SHA-256 is embedded in the
      versioned scan content fingerprint.
- [x] Versioned perceptual fingerprint. Rust stores a deterministic `mean-grid-16x24-v1` luminance
      signature derived from the aligned, contrast-normalized OCR image. The representation exposes
      raw per-cell differences only; it does not invent duplicate/revision thresholds.
- [ ] Aligned change-region comparison.
- [ ] Confidence and reason reporting.
- [ ] Fixture-based threshold tuning only.

Suggested assessment:

```rust
pub enum ExistingPageScanAssessment {
    FirstScan,
    NearDuplicate { better_quality: bool },
    PossibleRevision { changed_regions: Vec<Region>, confidence: f32 },
    SubstantiallyDifferent,
    Ambiguous,
}
```

## 9.3 Safe revision rules

- [ ] Preserve the new original before prompting, or keep it in a recoverable staged state.
- [ ] Default proposal is Save as New Version.
- [ ] Replace Preferred changes preference only.
- [ ] Never delete the old original automatically.
- [ ] Another Physical Copy creates `PhysicalCopy`.
- [ ] Wrong Scan moves to Needs Review or is explicitly discarded.

## 9.4 Needs Review

Implement review kinds for unidentified page, Notebook selection, wrong Notebook, low quality, manual alignment, duplicate, revision, physical copy, OCR failure, processing failure, import conflict, and restore conflict.

- [ ] List/filter/detail/resolve APIs.
- [ ] Audited resolutions.
- [ ] Defer without data loss.

## 9.5 Version UI

- [ ] Timeline.
- [ ] Preferred indicator.
- [ ] Side-by-side or overlay comparison.
- [ ] Changed regions.
- [ ] Keep both.
- [ ] Set preferred.
- [ ] Mark another physical copy.
- [ ] Move unresolved item to review.

---

# Milestone 10 — Library and page presentation

## 10.1 Home

- [ ] Empty and populated states.
- [ ] Recent Notebooks.
- [ ] Continue scanning.
- [ ] Generated Smart Pages.
- [ ] Needs Review count.
- [ ] Last backup and changed-page count.
- [ ] Primary actions.

## 10.2 Library hub

- [ ] Notebooks.
- [ ] Smart Pages.
- [ ] Page Sets.
- [ ] Collections.
- [ ] Imports.
- [ ] Needs Review.
- [ ] Trash.
- [ ] Pagination/streaming for large libraries.
- [ ] Sorting by title, page number, scan date, and modified date.

## 10.3 Notebook detail

- [ ] Show logical page slots including unscanned pages.
- [ ] Grid and list views.
- [ ] Status badges.
- [ ] Scan and Batch Scan actions.
- [ ] Rename/archive.
- [ ] Never renumber from scan order.

## 10.4 Smart Pages, Page Sets, and Collections

- [ ] Standalone pages.
- [ ] Generated-not-scanned state.
- [ ] Page Set ordering.
- [ ] Reprint existing PDF.
- [ ] Create/rename/delete collection.
- [ ] Add/remove/reorder members.
- [ ] Removing collection membership does not delete a page.
- [ ] Page identity never changes.

## 10.5 Page viewer

Views:

- [ ] Corrected image.
- [ ] Original image.
- [ ] Extracted text.
- [ ] Split view.
- [ ] Metadata.
- [ ] Versions.
- [ ] Annotations.
- [ ] Related pages.
- [ ] Skill results.

Actions:

- [ ] Rename/title.
- [ ] Tags.
- [ ] Correct OCR.
- [ ] Rescan.
- [ ] Export.
- [ ] Run skill.
- [ ] Organize.
- [ ] Trash.

## 10.6 Trash

- [ ] Soft delete.
- [ ] Show consequences.
- [ ] Restore.
- [ ] Permanent-delete confirmation.
- [ ] Never reuse IDs.
- [ ] Offer backup/export before large deletion.

---

# Milestone 11 — OCR and correction

## 11.1 Rust OCR contract

```rust
pub struct OcrRequest {
    pub scan_id: ScanId,
    pub image_path: String,
    pub language_hints: Vec<String>,
}

pub struct OcrResult {
    pub provider: String,
    pub provider_version: String,
    pub full_text: String,
    pub regions: Vec<OcrRegion>,
    pub warnings: Vec<OcrWarning>,
}
```

- [ ] Validate adapter output.
- [ ] Normalize coordinates into canonical page space.
- [ ] Persist status and warnings.
- [ ] Support retry.
- [ ] Represent unavailable confidence distinctly.

## 11.2 Android OCR adapter

- [ ] Select bundled/unbundled ML Kit behavior explicitly.
- [ ] Handle model unavailable.
- [ ] Map blocks, lines, elements, coordinates, languages, and confidence.
- [ ] Return provider/model version where possible.
- [ ] Handle cancellation.
- [ ] Avoid retaining images after request completion.
- [ ] Add known-image tests.

## 11.3 Background OCR queue

- [ ] Queue only after durable scan save.
- [ ] Persist job state.
- [ ] Bounded retry for retryable errors.
- [ ] Permanent failure enters Needs Review.
- [ ] Prevent duplicate OCR after restart.
- [ ] Allow automatic OCR to be disabled.

## 11.4 Correction UI

- [ ] Full-text editing.
- [ ] Region-level correction.
- [ ] Low-confidence highlighting.
- [ ] Tap text to highlight source region.
- [ ] Correction history.
- [ ] Prefer corrected text in search.
- [ ] Preserve original OCR.

Acceptance:

- [ ] OCR failure does not block scanning, saving, or browsing.

---

# Milestone 12 — Local search

## 12.1 Rust-owned FTS

- [ ] Create FTS schema for OCR, corrections, titles, tags, annotations, and Notebook names.
- [ ] Reindex transactionally after correction.
- [ ] Define trashed-content behavior.
- [ ] Add rebuild and integrity-check paths.

## 12.2 Search API

Support text, Notebook, Collection, date, page number, tags, review status, version presence, pagination, and stable sorting.

Return:

```rust
pub struct SearchHit {
    pub page: PageSummary,
    pub excerpt: String,
    pub source: SearchMatchSource,
    pub region: Option<PageRegion>,
    pub score: f32,
}
```

- [ ] Validate/escape FTS input.
- [ ] Return explicit syntax errors if advanced syntax exists.
- [ ] Never return the whole library as a fallback after query failure.

## 12.3 Search UI

- [ ] Search field.
- [ ] Local recent searches.
- [ ] Filter sheet.
- [ ] Excerpts and match source.
- [ ] Open/highlight source region.
- [ ] Empty/error states.

## 12.4 Scale tests

- [ ] Generate a 10,000-page fixture library.
- [ ] Measure index creation, common query latency, correction reindex, and memory usage.

---

# Milestone 13 — Manual backup, restore, and export

## 13.1 Define `.atnb`

Create a versioned archive manifest describing format, version, backup ID, timestamp, installation, encryption, database snapshot, assets, sizes, and SHA-256 hashes.

- [ ] Define canonical paths.
- [ ] Prevent traversal.
- [ ] Define size/count limits.
- [ ] Define compatibility rules.
- [ ] Add golden fixtures.
- [ ] Document excluded secrets.

## 13.2 Encryption

- [ ] Use reviewed Argon2id parameters.
- [ ] Use authenticated encryption such as XChaCha20-Poly1305.
- [ ] Generate unique random salt and nonce material.
- [ ] Authenticate format metadata.
- [ ] Zeroize sensitive keys where practical.
- [ ] Return wrong password/authentication distinctly.
- [ ] Never emit plaintext if encryption was requested.
- [ ] Add known-answer and tamper tests.

## 13.3 Create backup

- [ ] Acquire consistent DB snapshot.
- [ ] Stream files and hashes.
- [ ] Write temporary archive.
- [ ] Finalize encryption.
- [ ] Reopen and verify.
- [ ] Return only verified output.
- [ ] Record success after system save/copy confirmation where possible.
- [ ] Clean temporary files safely.

## 13.4 Android backup UI

- [ ] Backup hub.
- [ ] Library size/object summary.
- [ ] Password/recovery-key setup.
- [ ] Confirmation.
- [ ] System destination picker.
- [ ] Progress/cancel.
- [ ] Completion and last-backup record.
- [ ] Backup reminder settings.
- [ ] Disk-space and picker-cancellation states.

## 13.5 Inspect and restore

- [ ] Select and stage `.atnb`.
- [ ] Inspect version.
- [ ] Authenticate/decrypt.
- [ ] Verify manifest/hashes.
- [ ] Estimate required space.
- [ ] Show date and contents.
- [ ] Choose Replace or Merge.
- [ ] Do not mutate current library during inspection.

## 13.6 Replace restore

- [ ] Restore into a new directory.
- [ ] Verify database/assets.
- [ ] Atomically switch active library.
- [ ] Preserve prior library until success.
- [ ] Support rollback after verification failure.

## 13.7 Merge restore

- [ ] Import immutable IDs idempotently.
- [ ] Detect same ID/same hash.
- [ ] Treat same ID/different immutable content as an integrity conflict.
- [ ] Merge safe additive metadata.
- [ ] Preserve conflicting corrections.
- [ ] Create ReviewItems for unresolved conflicts.
- [ ] Never select a winner silently.

## 13.8 Exporters

Implement:

- [ ] Original images.
- [ ] Corrected images.
- [ ] Markdown with stable citations.
- [ ] Plain text.
- [ ] JSON metadata.
- [ ] Searchable PDF when quality permits.
- [ ] Complete backup.

- [ ] Stream large exports.
- [ ] Preserve ordering and provenance.
- [ ] Never alter source records on export failure.

## 13.9 Failure tests

- [ ] Wrong password.
- [ ] Truncated archive.
- [ ] Modified ciphertext/asset.
- [ ] Traversal entry.
- [ ] Resource-exhaustion archive.
- [ ] Unsupported version.
- [ ] Insufficient space.
- [ ] Cancellation/process death.
- [ ] Merge conflicts.

Acceptance:

- [ ] Round-trip preserves all identities, originals, text, and version relationships.
- [ ] Failed restore leaves the old library usable.

---

# Milestone 14 — Model providers and A2D Skills

## 14.1 Model capabilities

```rust
pub enum ModelCapability {
    GenerateText,
    AnalyzeImage,
    CreateEmbedding,
    Rerank,
}
```

- [ ] Define provider metadata and selection policy.
- [ ] Define request limits, timeout, and cancellation.
- [ ] Define local/network trust state.
- [ ] Store secrets only through platform secure-store handles.
- [ ] Never serialize API keys into ordinary domain objects.

## 14.2 Providers

Architecture supports:

- [ ] On-device provider.
- [ ] Local-network OpenAI-compatible endpoint.
- [ ] User-provided cloud provider.
- [ ] Future managed A2D provider (not implemented in v0.1 — non-goal per spec §5).

**The required v0.1 provider is a user-configured local-network OpenAI-compatible endpoint**
(compatible with llama.cpp, Ollama-compatible gateways, LM Studio, and similar user-controlled
services). This is the practical text-generation path v0.1 must ship and test against — it fits
the accountless, local-first product. The on-device and user-provided-cloud paths remain
architecturally supported but are not required production implementations for v0.1.

Provider requirements:

- [ ] User-configurable base URL.
- [ ] User-configurable model name.
- [ ] Optional bearer/API token stored only through an Android Keystore-backed secure-store handle.
- [ ] Explicit connection-test action.
- [ ] Display the exact host, model, and HTTP/HTTPS transport before sending note data.
- [ ] Require explicit user approval before first use with note content and whenever scope materially changes.
- [ ] Enforce timeouts, response-size limits, cancellation, authentication errors, rate-limit errors, malformed-response errors, and unreachable-host errors explicitly.
- [ ] Do not silently fall back to a different provider or model.
- [ ] Do not silently retry against a public endpoint.
- [ ] Restrict requests to the selected pages and fields.

Deterministic test infrastructure (required — CI MUST NOT depend on internet access, a real API
key, or a live LLM):

- [ ] In-process deterministic `MockModelProvider` in Rust for unit and skill-runtime tests.
- [ ] Local fake OpenAI-compatible HTTP server fixture for request/response contract, timeout, cancellation, authentication, malformed-payload, rate-limit, and size-limit tests.
- [ ] Mock providers are not selectable in production builds unless an explicit developer/debug feature is enabled.
- [ ] Tests verify citations and permission enforcement independently of model quality.

This task is not complete with only the mock provider working — it requires one real configurable
local-network OpenAI-compatible implementation *plus* the deterministic test infrastructure above.

## 14.3 Skill manifest

Example:

```yaml
id: extract-action-items
name: Extract Action Items
version: 1.0.0
runtime: llm-tool-workflow
permissions:
  - pages.read_text
  - pages.create_annotation
model_requirements:
  - generate_text
network: provider-only
mutation_policy: proposal
```

- [ ] Strict versioned schema.
- [ ] Permissions.
- [ ] Model requirements.
- [ ] Network declaration.
- [ ] Resource limits.
- [ ] Signature/trust extension.
- [ ] Reject unsupported required fields/version.

## 14.4 Permission enforcement in Rust

- [ ] Read-only default.
- [ ] Scope by selected pages/Notebook/Collection.
- [ ] Network separately granted.
- [ ] Mutations become proposals.
- [ ] External actions require confirmation.
- [ ] Permission revocation.
- [ ] Per-run effective permission snapshot.
- [ ] Audit denied and approved tool calls.
- [ ] Privilege-escalation tests.

## 14.5 Narrow tools

Implement only explicit tools such as:

- [ ] `pages.search`
- [ ] `pages.read_metadata`
- [ ] `pages.read_text`
- [ ] `pages.read_image`
- [ ] `pages.create_annotation`
- [ ] `pages.add_tag`
- [ ] `collections.create`
- [ ] `exports.create`
- [ ] `model.generate_text`
- [ ] `model.analyze_image` when supported

No generic SQL, file, shell, or unrestricted network tool.

## 14.6 Prompt-injection controls

- [ ] Mark notebook content as untrusted.
- [ ] Separate policy, skill instructions, user request, retrieved data, and tool results.
- [ ] Deny permission changes originating from notebook content.
- [ ] Deny arbitrary URLs unless explicitly granted and allowlisted by policy.
- [ ] Add malicious-note and exfiltration fixtures.
- [ ] Ensure model output cannot directly mutate data.

## 14.7 Built-in skills

Deterministic:

- [ ] Export Selected Pages to Markdown with citations and explicit missing-OCR handling.

Deterministic with optional model explanation:

- [ ] Compare Two Scans of One Page. Deterministic image alignment, difference regions,
      fingerprints, and scan metadata are the authoritative comparison output. A configured
      model MAY optionally explain likely changes in user-friendly language, but MUST NOT
      invent differences or replace the deterministic result. The result identifies:
  - [ ] The two `ScanId` values.
  - [ ] Alignment/registration status.
  - [ ] Changed-region coordinates.
  - [ ] Quality differences.
  - [ ] Whether the comparison is complete, degraded, or inconclusive.
  - [ ] Any model-generated interpretation, labeled as a separate derived result.
  - [ ] Open the visual comparison from the page-version workflow (Milestone 9.5).
  - [ ] Preserve both original scans regardless of comparison outcome.

Configured-model skills:

- [ ] Summarize selected pages.
- [ ] Extract proposed action items.
- [ ] Find related pages.
- [ ] Ask My Notes with citations — a built-in, non-removable, read-only system skill backed by
      the same permissioned skill runtime as any other model skill (spec §7.10/§21). Default
      permissions are limited to the selected scope and normally only `pages.search`,
      `pages.read_metadata`, `pages.read_text`, `model.generate_text`, plus `pages.read_image`/
      `model.analyze_image` only when the user explicitly enables image use. It has no mutation
      permission and does not itself produce a mutation proposal; any follow-up action (tags,
      annotations, tasks, collections, exports, external actions) requires its own separately
      permissioned proposal. Being built in MUST NOT bypass permission checks or auditing.

Every model skill:

- [ ] Shows scope.
- [ ] Shows whether data leaves device.
- [ ] Preserves citations.
- [ ] Marks inference/low confidence.
- [ ] Saves only after appropriate user action.
- [ ] Never silently creates external tasks.

## 14.8 Skills UI

- [ ] Skills hub.
- [ ] Skill details and permissions.
- [ ] Scope selector.
- [ ] Run progress/cancel.
- [ ] Proposal review.
- [ ] Approve/edit/reject.
- [ ] Skill History.
- [ ] Permission revocation.
- [ ] Model-not-configured state.

Acceptance:

- [ ] A malicious page cannot grant network or mutation permission.
- [ ] Ask My Notes returns navigable page citations.

---

# Milestone 15 — iOS readiness

The iOS UI is not implemented in v0.1, but the shared core must be genuinely portable.

- [ ] Generate Swift bindings in CI.
- [ ] Compile a minimal Swift harness where the environment permits.
- [ ] Verify enums and errors map acceptably.
- [ ] Avoid Kotlin-specific types in FFI.
- [ ] Avoid Android URIs/paths in domain APIs.
- [ ] Use portable timestamps and binary/string representations.
- [ ] Document future XCFramework packaging.
- [ ] Inventory future adapters: AVFoundation, Files/Photos, print/share, Keychain, OCR, background tasks, notifications, and network provider.
- [ ] Ensure Rust interfaces do not assume Android lifecycle semantics.
- [ ] Provide desktop mock adapters.
- [ ] Keep QR, layouts, backups, migrations, search rules, and skill permissions platform-independent.

Acceptance:

- [ ] Adding iOS requires presentation/platform adapters, not a canonical-data redesign.

---

# Milestone 16 — Security, diagnostics, and failure hardening

## 16.1 Diagnostics

- [ ] Structured logs and correlation IDs.
- [ ] Redact note text, API keys, passwords, and recovery keys.
- [ ] Diagnostic export excludes note content by default.
- [ ] Optional user-selected support data.
- [ ] Record processing/model versions and review resolutions.

## 16.2 Input hardening

- [ ] QR length limits.
- [ ] Image pixel/dimension limits.
- [ ] PDF page/count limits.
- [ ] Archive entry/count/size limits.
- [ ] Path canonicalization.
- [ ] Parameterized SQL.
- [ ] Skill manifest parser limits.
- [ ] Model response limits.
- [ ] Network timeout/redirect policy.

## 16.3 Dependency review

- [ ] Pin native marker dependency.
- [ ] Record its license.
- [ ] Pin UniFFI.
- [ ] Review PDF, QR, image, crypto, and OCR licenses.
- [ ] Configure `cargo-deny` and vulnerability checks.
- [ ] Document dependency update procedure.
- [ ] Require fixture review before portable-format dependency changes.

## 16.4 Library integrity check

Implement an explicit non-destructive check for:

- [ ] Foreign keys.
- [ ] Schema version.
- [ ] Referenced asset existence.
- [ ] Optional full asset hashes.
- [ ] Orphans.
- [ ] Search-index consistency.
- [ ] Review report.
- [ ] No automatic destructive repair.

## 16.5 Failure injection

Automate:

- [ ] Disk full.
- [ ] DB locked/corrupt.
- [ ] Missing asset.
- [ ] Image decoder failure.
- [ ] Native detector failure.
- [ ] OCR unavailable.
- [ ] Model timeout.
- [ ] Backup/restore interruption.
- [ ] Process death.
- [ ] Permission denial.
- [ ] User cancellation.

Acceptance:

- [ ] Every injected failure has an explicit expected result.
- [ ] “The app did not crash” is never the only success criterion.

---

# Milestone 17 — Physical print validation

## 17.1 Proof assets

- [ ] Generate first bound-notebook interior PDF.
- [ ] Generate cover draft.
- [ ] Generate Smart Page printer test pack.
- [ ] Include marker-size variants when needed.
- [ ] Record exact design/layout versions.

## 17.2 Home-printer matrix

Test US Letter and A4, Actual Size and Fit, grayscale, varied paper/toner, and several Android phones.

Record marker detection, QR detection, geometric error, OCR quality, capture time, and failure reason.

## 17.3 KDP proof matrix

Test front/middle/back pages, gutter markers, writing instruments, lighting, hand shadows, and common capture angles.

- [ ] Measure trim/gutter variation.
- [ ] Change released layouts only by creating a new version.
- [ ] Save representative proof fixtures where licensing/privacy permits.

## 17.4 Evidence-based thresholds

Define after measurement:

- [ ] Marker success rate.
- [ ] QR success rate.
- [ ] Maximum geometry error.
- [ ] Minimum effective resolution.
- [ ] Blur/glare thresholds.
- [ ] Supported device floor.
- [ ] Known unsupported conditions.

Do not invent thresholds before testing.

---

# Milestone 18 — UX completeness and accessibility

- [ ] Implement all v0.1 screens in the spec.
- [ ] Use A2D terminology consistently.
- [ ] Do not show “KDP notebook” or “AprilTag” in normal UI.
- [ ] Always show scan destination.
- [ ] Always show warning/review state.
- [ ] Distinguish original image, OCR, corrected text, and AI output.
- [ ] Provide empty/loading/error/retry states.
- [ ] Add screen-reader labels.
- [ ] Support dynamic type/font scaling.
- [ ] Meet contrast and touch-target requirements.
- [ ] Do not rely on color alone.
- [ ] Add configurable haptic/audio capture confirmation.
- [ ] Show last backup and changed-page count.
- [ ] Keep manual backup visible without a paid-sync dark pattern.
- [ ] Explain external-model scope and provider before upload.
- [ ] Never prompt for an account in core workflows.

---

# Milestone 19 — Release validation

## 19.1 Automated checks

- [ ] Rust format, clippy, and tests.
- [ ] Android lint, unit, and instrumentation tests.
- [ ] Kotlin and Swift binding generation.
- [ ] Backup compatibility fixtures.
- [ ] QR/layout fixtures.
- [ ] Native ABI builds.
- [ ] Dependency/license checks.

## 19.2 Manual acceptance walkthrough

- [ ] Fresh accountless install.
- [ ] Register two identical physical Notebooks.
- [ ] Scan page 1 into each and verify separation.
- [ ] Block or explicitly reassign wrong-design scan.
- [ ] Batch scan out of order.
- [ ] Generate one unique Smart Page.
- [ ] Generate a 20-page Page Set.
- [ ] Print and scan generated pages.
- [ ] Rescan with added writing and preserve old original.
- [ ] Correct OCR.
- [ ] Search and open highlighted source.
- [ ] Create encrypted backup.
- [ ] Restore using Replace.
- [ ] Restore using Merge with conflict.
- [ ] Export Markdown, images, and JSON.
- [ ] Run deterministic skill.
- [ ] Run model skill with citations.
- [ ] Deny unauthorized skill action.
- [ ] Complete core workflow offline.

## 19.3 Release blockers

Do not release with:

- [ ] Known silent data loss.
- [ ] Unhandled migration failure.
- [ ] Unverified backup restore.
- [ ] QR protocol without golden vectors.
- [ ] Printed markers not physically tested.
- [ ] Skill permissions enforced only in Kotlin.
- [ ] API keys outside secure storage.
- [ ] Scanner claiming success before durable Rust registration.
- [ ] Ordinary FFI panics.
- [ ] Core behavior dependent on A2D servers.

## 19.4 Documentation

- [ ] Build/run/test instructions.
- [ ] Android native build setup.
- [ ] Binding generation.
- [ ] Migration policy.
- [ ] QR/layout compatibility policy.
- [ ] Backup format/version policy.
- [ ] Fixture contribution requirements.
- [ ] Physical validation procedure.
- [ ] Verify all referenced files exist at exact paths.

---

# Post-v0.1 backlog — explicitly deferred

- [ ] Production iOS SwiftUI client.
- [ ] A2D Sync account/subscription service.
- [ ] End-to-end encrypted multi-device sync.
- [ ] Managed A2D AI.
- [ ] Public skill marketplace.
- [ ] Signed community skills.
- [ ] Third-party Notebook Design SDK.
- [ ] Additional page sizes and landscape.
- [ ] Advanced handwriting and mathematical OCR.
- [ ] Diagram understanding.
- [ ] Two-page spread scanning.
- [ ] Mesh/spine dewarping.
- [ ] Desktop scanning stand mode.
- [ ] Web viewer.
- [ ] Collaboration.
- [ ] External task/calendar/email integrations.

---

# Ralph-loop recommended slices

## Slice A — Bootstrap

Milestones 1 and 2.

Exit: Android displays a typed value returned by Rust; CI is green; Swift bindings generate.

## Slice B — Durable local core

Milestone 3 plus core Notebook/Page persistence.

Exit: accountless library can be created, closed, reopened, and migration-tested without silent recovery.

## Slice C — Identity and printable pages

Milestones 4 and 5 plus Smart Page UI.

Exit: generated PDF pages round-trip through QR and marker fixtures.

## Slice D — Scanner foundation

Milestone 7 plus live CameraX analysis.

Exit: Android displays marker/Page Code guidance from shared native analysis.

## Slice E — Durable scanning

Remaining Milestone 8 and all of Milestone 9.

Exit: single and batch scans are durably registered with safe versioning.

## Slice F — Library, OCR, and search

Milestones 10–12.

Exit: user can browse, correct, and search page-linked text.

## Slice G — Data ownership

Milestone 13.

Exit: encrypted backup/restore and exports pass round-trip and failure tests.

## Slice H — Skills

Milestone 14 plus relevant security tests.

Exit: deterministic and configured-model skills run with enforced permissions and citations.

## Slice I — Cross-platform readiness and release hardening

Milestones 15–19.

Exit: v0.1 acceptance criteria and physical proof tests are complete.

---

# Final completion checklist

- [ ] Every v0.1 acceptance criterion in the spec is satisfied.
- [ ] Completed tasks contain implementation and tests, not placeholders.
- [ ] Rust remains authoritative for canonical data and business logic.
- [ ] Kotlin remains presentation and Android platform integration.
- [ ] Swift bindings generate without redesigning Rust APIs.
- [ ] Core product requires no account or A2D server.
- [ ] Manual backup and restore are reliable.
- [ ] Original scan data is never silently lost.
- [ ] Every degraded path is visible and reviewable.
- [ ] All referenced files are present at exact repository paths.
