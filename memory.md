# A2D Smart Notebook — session log

Append-only. `/summarize-memory` condenses this into `memory_summary.md`.

## 2026-07-26 — /init: CLAUDE.md, skills, hook

Ran `/init`. Added project `CLAUDE.md`, `.claude/skills/a2d-check`, `.claude/skills/a2d-task`,
and a `PostToolUse` rustfmt-on-edit hook (`.claude/settings.json` + `.claude/hooks/rustfmt-on-edit.sh`).
The hook was written and validated (`jq -e` schema check, raw pipe-test) but did not fire on a live
edit in the same session — likely because the settings watcher wasn't watching `.claude/` yet. Needs
`/hooks` opened once (or a restart) to activate; unverified live.

Also fixed a spec/TODO inconsistency: spec §13's core identifier list said `LogicalNotebookPageId`
while the spec's own §7 workflow text and TODO §2.1 both use `PageId`. Renamed to `PageId` in the
spec's list and added `CollectionId` (used throughout the spec — §15.8, browsing, capabilities,
backup — but missing from the §13 list). TODO §2.1's list already had both names right.

## 2026-07-26 — Milestone 1.1: Rust workspace

Implemented TODO 1.1 in full: root `Cargo.toml` (resolver 2, 15 members), `rust-toolchain.toml`,
`deny.toml`, 15 crate skeletons under `crates/` (each with a `//!` module doc stating its
responsibility boundary — used instead of per-crate README files, which the acceptance criterion
allows), extended `.gitignore` (Gradle/Android Studio/native builds/test output/generated
PDFs/secrets), rewrote root `README.md`, added `apps/ios/README.md`. Verified `cargo metadata`,
`cargo build --workspace`, `cargo fmt --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace --all-features` — all clean.

Decisions made (open items per spec, recorded per CLAUDE.md's "pick a default, flag it" policy):

- **Rust toolchain pinned to `1.94.1`** — matches the version already installed in this environment.
  No other constraint existed; revisit if a specific MSRV becomes load-bearing (e.g. a dependency
  requires older/newer).
- **Edition `2024`** — current default for new Rust projects as of this date; no spec constraint
  found, and no third-party crate compatibility concern yet since the workspace has zero
  dependencies.
- **License `Apache-2.0`** — matches the existing root `LICENSE` file, not a new choice.
- **Lint policy**: `[workspace.lints.clippy] all = "warn"` in the root `Cargo.toml`, with `-D
  warnings` enforced by the CI command (spec §31) rather than duplicated as `deny` in the manifest.
- **`deny.toml`**: minimal starting policy (permissive common OSS license allowlist: Apache-2.0,
  MIT, BSD-2/3-Clause, ISC, Unicode-3.0). `cargo-deny` is not installed in this environment, so the
  file is unverified — only `jq`/manual review, not `cargo deny check`. TODO 1.3 says dependency
  policy work resumes "after policy configuration," which this now provides a starting point for.

Not attempted: **Milestone 1.2 (Android)** — `gradle` is not installed locally (`adb` and a JDK are;
`gradle`/Android Studio are not), so scaffolding a real Gradle project and verifying
`./gradlew :app:assembleDebug` isn't feasible from this environment as-is. Left for a session with
Android tooling available, or for the user to confirm how they want the Gradle wrapper bootstrapped.

TODO 1.1 checkboxes (tasks + acceptance) are ticked. Milestone 1.1 is otherwise complete per the
"Definition of done" in `CLAUDE.md` (compiles, tests pass, no placeholders, checks green).

## 2026-07-26 — /spec-todo review + applied responses

Ran `/spec-todo` against the spec/TODO pair, then `/responses` to capture six open questions in
`docs/A2D_SMART_NOTEBOOK_V01_RESPONSES.md`. The user answered all six directly in that file (via a
separate push, pulled in here) with authoritative, detailed direction. Applied all seven required
edits:

1. **Identifier gap** — added `TextRegionId`, `TextCorrectionId`, `AnnotationId`, `ReviewItemId`,
   `AuditEventId` to spec §13 and TODO 2.1, plus a general rule: every independently persisted,
   referenceable, or FFI-crossing entity gets an opaque ID; embedded value objects don't.
2. **`docs/decisions/` ADR process established** — `README.md` (process + index), `ADR_TEMPLATE.md`,
   and two ADRs:
   - **0001 — QR v1 wire encoding and integrity**: drafted a full concrete grammar (canonical
     uppercase QR-alphanumeric text payload, `A2D:1:<type>:...:<crc>`, 128-bit IDs as 26-char
     Crockford Base32 with no alias-normalization, CRC-32C as 7-char Crockford Base32, 128-char
     max length, full strict-parser rejection list, golden-vector JSON schema). **Status:
     Proposed, not Accepted** — the response required a real Android-decoder spike (doesn't exist
     yet, no Android project) before acceptance; TODO 4.2/4.3 now block fixture commits on this
     ADR reaching Accepted. **This grammar is my draft implementing the user's stated
     requirements — it has not been reviewed by the user line-by-line and deserves a read before
     Milestone 4 starts.**
   - **0002 — AprilTag detector selection**: placeholder only (Milestone 7.1 hasn't run); pre-filled
     with the required validation-evidence checklist so the spike has a predetermined home.
   - Spec §10's repo tree and TODO Milestones 4.2/4.3/7.1 now reference the ADR directory.
3. **Built-in skills** — added "Compare Two Scans of One Page" to TODO 14.7 (deterministic
   alignment/diff, optional model explanation only, never inventing differences), matching spec
   §21.2. `Ask My Notes` clarified in both spec §7.10 and TODO 14.7 as a Search-UI surface backed
   by the same permissioned skill runtime as any other model skill — not a parallel ungoverned path.
4. **Model provider** — TODO 14.2 now names the user-configured local-network OpenAI-compatible
   endpoint as the required v0.1 provider (on-device/user-cloud remain architecturally supported
   but not required; the future managed A2D provider is explicitly out of scope, a non-goal).
   Added required deterministic test infrastructure (in-process `MockModelProvider`, fake
   OpenAI-compatible HTTP fixture server) so CI never needs live network/API keys.

Not yet done: nothing else was requested. Milestones 2, 4, 7, and 14 can now proceed without
another clarification round, per the responses file — except ADR 0001 needs the user's read-through
and ADR 0001/0002 both need their respective spikes before their dependent milestones pass the
fixture-commit / detector-selection checkpoints.

## 2026-07-26 — Ralph loop: Milestone 2.2 + 2.1 (errors, identifiers)

User said "Go for it. Continue on. Ralph loop it" — continuing autonomously through the TODO using
`/a2d-task`'s workflow rather than the global `/ralph-loop` skill (still the wrong one for this
repo — ESP32/Bluetooth rules). Implemented TODO 2.2 before 2.1 despite the TODO's ordering, because
2.1's identifier parsing needs `A2dError` to return, and CLAUDE.md's "every fallible path returns
the structured `A2dError` envelope" rule applies to `id.rs` too.

Both landed in `a2d-domain` (chosen over `a2d-identity`, which owns registration/resolution
*workflows* built on top of these types, per each crate's module doc).

- **`error.rs`**: `A2dError`/`ErrorCategory`/`ErrorSeverity`/`ErrorCode` per TODO 2.2's shape, plus
  `Outcome<T>` (`Completed`/`Cancelled`/`Failed(A2dError)`) so cancellable operations don't have to
  build a full `A2dError` just to say "the user cancelled" — reconciles a real tension between spec
  §27 (which lists "cancellation" as one of `ErrorCategory`'s values) and TODO 2.2 ("map
  cancellation separately from failure"): kept `ErrorCategory::Cancellation` for completeness, but
  `Outcome` is the intended path for actual cancellable call sites.
  - clippy's `result_large_err` fired on the first pass (`A2dError` was >128 bytes). Fixed by
    boxing the fields into a private `A2dErrorFields` behind `Deref`/`DerefMut`, so `A2dError` is
    just a pointer and every `Result<T, A2dError>` in the codebase stays cheap, while `err.code`
    etc. still work unchanged.
  - **`ErrorSeverity`'s levels (`Info`/`Warning`/`Error`/`Critical`) are an assumption** — spec/TODO
    never enumerate severity values. Flagged, not yet reviewed.
  - Two TODO 2.2 checkboxes left unchecked on purpose: redaction has no enforcement mechanism yet
    (nothing produces sensitive `details` values yet to enforce against), and "ban erasing
    conversions" is an ongoing discipline (already in CLAUDE.md), not a one-time deliverable.
    FFI/serialization tests deferred to 2.4, since no FFI boundary exists yet.
- **`id.rs`**: all 19 opaque ID newtypes via one `define_id!` macro (19 hand-written copies would
  have been the actual duplication). Canonical wire form is the **same** 26-char Crockford Base32
  scheme ADR 0001 specifies for QR-embedded IDs — applied to every ID type, not just the three that
  appear in QR codes, so the codebase has one ID format rather than two. This resolves TODO 4.1's
  "encode using a compact canonical alphabet" ahead of time; when Milestone 4.1 is implemented it
  should reuse this module and add QR-specific fixtures rather than re-deriving generation.
  - Deterministic test-only construction (`from_raw_for_test`) is gated behind a `test-util` Cargo
    feature (not bare `#[cfg(test)]`), since TODO 2.1 implies other crates' tests may need it too —
    `#[cfg(test)]` wouldn't be visible outside the defining crate.
  - `getrandom = "0.2"` added as `a2d-domain`'s first external dependency (Apache-2.0/MIT, allowed
    by `deny.toml`); confirmed crates.io is reachable from this environment.
  - Tests: round-trip, two known encoding vectors (all-zero, all-one), wrong length, invalid
    alphabet (including `I`), lowercase rejection, non-canonical first-character padding,
    generate-then-parse round trip, and a 10,000-sample uniqueness check.

TODO 2.1 fully checked. TODO 2.2 checked except the two items above. Full workspace gate
(`fmt --check`, `clippy -D warnings`, `test --all-features`) green throughout.

## 2026-07-26 — Ralph loop: Milestone 2.3 (domain entities)

All 16 entities in `a2d-domain/src/entities.rs`. This task had far more assumption surface than
2.1/2.2: spec §15 gives complete field lists for only 6 of the 16 entities (`NotebookDesign`,
`Notebook`, `Page`, `PhysicalCopy`, `Scan`, `Asset`); the other 10 (`PageSet`, `Collection`,
`ReviewItem`, `OcrRun`, `TextRegion`, `TextCorrection`, `Annotation`, `SkillDefinition`,
`SkillRun`, `AuditEvent`) are described only in prose. Every inferred entity is marked `INFERRED`
in its doc comment with a citation to what prose it was drawn from. **These will need a real
review pass once their owning milestones land** — most likely to shift: `SkillDefinition`/
`SkillRun` (Milestone 14 owns the actual permission model; I deliberately kept permissions/
network/mutation_policy as bare strings rather than guessing at enums, to avoid diverging from
whatever 14 actually defines), and `NotebookDesign`'s marker-related fields (Milestone 7 hasn't
picked AprilTag vs. pure-Rust yet).

New supporting types added beyond spec's literal §15 field lists, each justified inline in the
code:

- **`LayoutId`** — spec references `layout_id`/`setup_layout_id`/`page_layout_id` throughout but
  never lists it in §13's core-identifier list. Added as a distinct newtype (short registry
  token, not a 128-bit random ID) because a raw `String` would violate the "opaque newtype, never
  raw string" rule. Its validation shape (1-20 uppercase alphanumeric/hyphen) matches what ADR
  0001 already specifies for layout ids embedded in QR payloads. **This is my own addition, not
  something reviewed in the responses file — flagging explicitly**, unlike the
  ReviewItemId/AnnotationId/AuditEventId additions, which the user directly requested.
  `a2d-layout` (Milestone 5) will own the actual registry `LayoutId` values are validated against.
- **`Provenance`** (embedded, no ID) — spec §15.10's "source page/scan IDs, producing component,
  version, timestamp, warnings, approval state" as a struct, attached to `OcrRun`, `TextCorrection`,
  `Annotation`, `SkillRun`.
- **`TrimSizeMm`, `TrustState`, `PageState`, `QualityStatus`, `CaptureSource`, `AssetKind`,
  `EncryptionState`, `ReviewItemKind`, `ReviewItemStatus`, `SkillRunStatus`** — small enums/structs
  for fields spec names but doesn't give a value set for (or, for `PageState`/`QualityStatus`,
  where spec gives an explicit example set). Deliberately did NOT add types for `manifest_hash`/
  `sha256` (kept as plain `String` rather than a `Sha256Digest` newtype) or `marker_family`/
  `marker_role_ids` (kept as `String`/`Vec<String>`) — these need real design work belonging to
  Milestones 5/7/16, not 2.3, and inventing a representation now risked being wrong twice.

Every entity's `id` field is private with a getter-only accessor (no setter anywhere in this
crate), which is how "identity cannot change after creation" is actually enforced — applied to
all 16 entities, not just `Page`, since spec states it for `Page` but the same reasoning holds
everywhere.

Of TODO 2.3's 8 "Enforce" bullets, 3 are checked as genuinely enforced here (compiler-enforced
`PageKind` variants, `id` immutability, `Page::set_preferred_scan`'s same-page check — the one
cross-record invariant a single call site can actually verify) and 1 more partially
(`Provenance`'s non-optional fields for derived records). The remaining bullets — Smart Page ID
*uniqueness*, physical-copy index uniqueness, "scan references an immutable asset" (requires
looking the asset up), and the trash/permanent-delete lifecycle — all span more than one record
or need a lookup, so they're deferred to the storage layer (Milestone 3) and left unchecked with
inline notes in the TODO rather than marked done.

Full workspace gate green (18 tests total in a2d-domain now).

## 2026-07-26 — Ralph loop: Milestone 2.4 (UniFFI boundary) — Milestone 2 complete

`uniffi = "0.32"` (crates.io reachable, confirmed earlier this session). **Chose proc-macro mode
over UDL** — TODO 2.4's open decision — because it keeps the interface next to the Rust code
describing it rather than duplicated in a separate `.udl` file, and it's UniFFI's current
recommended default. `#[uniffi::export]`/`#[derive(uniffi::Object/Record/Error)]` on the first
try, no version-mismatch iteration needed.

- **`a2d-core`**: new crate content. `A2dCore::open` genuinely validates/creates a library
  directory (no storage dependency, so this is real, not a stub); `generate_page_id`/
  `parse_page_id` re-expose Milestone 2.1's already-complete ID logic specifically so `a2d-ffi`
  has a real operation to prove the round trip with, rather than a fabricated `list_notebooks`
  stub returning an empty list before storage exists to back it.
- **`a2d-ffi`**: `A2dClient` (open/library_path/generate_page_id/parse_page_id) over `A2dCore`,
  `A2dFfiError` mapped from `A2dError` via `From`. Hit the same clippy `result_large_err` lint as
  2.2's `A2dError` — fixed the same way, boxing the error's fields (`A2dFfiErrorDetails`) inside
  the `A2dFfiError::Failed` variant.
- **Kotlin and Swift bindings genuinely generate** — not just claimed. `[lib] crate-type =
  ["lib", "cdylib"]` plus a `uniffi-bindgen` bin target; `tools/generate-bindings.sh` is the
  canonical regeneration command. Verified manually first (inspected real `.kt`/`.swift` output
  containing `A2dClient`, `OpenLibraryRequest`, etc.), then automated as
  `crates/a2d-ffi/tests/binding_generation.rs`, which regenerates into a temp dir and asserts on
  the output every `cargo test` run — this is the "drift test" TODO 2.4 asks for. Deliberately
  not a golden-file diff against checked-in bindings: generated output isn't committed (it's
  build output, `.gitignore`d via the existing `target` entry since it lands under
  `target/bindings/` by default), and diffing full generated source would be brittle against
  codegen formatting/version changes.
- **"Panics MUST NOT cross FFI as success" left unverified end-to-end.** Added
  `trigger_panic_for_testing` + a Rust `#[should_panic]` test, but that only proves Rust's own
  panic semantics, not that UniFFI's generated `extern "C"` scaffolding actually catches the
  unwind — there's no compiled Kotlin/Swift consumer to call through yet (needs Milestone 1.2's
  Android project or Milestone 15's Swift harness). Left this TODO checkbox unchecked rather than
  claim a guarantee I didn't verify.

Milestone 2's own acceptance criteria: "Android calls Rust and renders a typed response" and
"Swift bindings generate in CI" both stay unchecked (blocked on Milestone 1.2's Android project
and Milestone 1.3's CI pipeline, neither of which exist in this environment); "`a2d-ffi` contains
no SQL or business rules" is checked — true by construction, every exported method is a one-line
delegation to `a2d-core`.

**Milestone 2 (domain model, structured errors, and UniFFI) is now complete** modulo the two
environment-blocked acceptance items above. Full workspace gate green throughout (28 tests total:
4 a2d-core + 18 a2d-domain + 4 a2d-ffi unit + 2 a2d-ffi binding-generation).

## 2026-07-26 — Ralph loop: Milestone 3.1 (database bootstrap and migrations)

**Crate choice**: `rusqlite` with the `bundled` feature (compiles SQLite from source — matters
for reproducible Android cross-compilation later, and keeps local dev independent of whatever
system SQLite is installed) over `sqlx`, since nothing here needs async and a Tokio dependency
for a synchronous single-connection mobile DB would be unjustified. **Had to downgrade from
rusqlite 0.40 (the version `cargo add` picked) to `~0.32`** — 0.40's `libsqlite3-sys` build
script uses the unstable `cfg_select!` macro, which doesn't compile on our pinned stable
`1.94.1` toolchain. 0.32.1's `libsqlite3-sys` (0.30.1) predates that and builds cleanly. Worth
revisiting if a future toolchain bump makes `cfg_select!` available, or if rusqlite backports a
fix.

**Journaling mode**: WAL + `synchronous = NORMAL` — resolves the "SQLite journaling mode" open
item from CLAUDE.md with the standard modern pairing. Flagged for revisit once real device
measurements exist, per spec §29.

Full schema in one migration (`migrations/0001_initial.sql`, embedded via `include_str!`):
18 tables covering every TODO 3.1-listed area (notebooks, notebook designs, pages, physical
copies, scans, assets, page sets, collections, OCR runs, text regions, corrections, annotations,
review items, skill definitions/runs, audit events, backup history, settings) plus
`schema_migrations` for tracking. List/map-shaped columns (`marker_role_ids`, `warnings`,
`details`, polygon coordinates, permission lists) are JSON-encoded `TEXT` rather than normalized
into join tables — a deliberate v1 simplification, revisit if a query ever needs to filter inside
one. `Provenance` is flattened into `provenance_*` columns per table rather than a shared table,
matching that it's an embedded value object with no independent identity.

**`backup_history` has no Rust entity behind it** — Milestone 2.3's entity list never included a
`Backup` struct (real backup domain logic is Milestone 13's job), but TODO 3.1 explicitly lists
"backup history" among the initial tables, so a minimal table exists now ahead of its owning
Rust type. Flagging this asymmetry for whoever picks up Milestone 13.

**Closed two invariant gaps deferred from Milestone 2.3**: `unique_smart_page_id` and
`unique_physical_copy_index` are now real partial/composite unique indexes, tested here. Went
back and ticked those two TODO 2.3 checkboxes. The other two 2.3 deferrals (verifying a
referenced asset is actually immutable; the trash/permanent-delete lifecycle) still need the
repository/write-path layer (Milestone 3.2/10), not just schema — left unchecked.

Migration runner: `schema_migrations(version, name, applied_at_ms)` tracks not just a version
number but each migration's name, and refuses to proceed if a previously-applied version's
recorded name doesn't match the current code's name for it (tested) — catches a modified
"immutable" migration file rather than silently re-trusting it. Each migration applies inside a
transaction, so a failure can't leave a half-applied migration committed (rusqlite's `Transaction`
rolls back on drop unless explicitly committed — a library guarantee, not something reimplemented
here). `foreign_keys` and `journal_mode` are both re-queried after being set and `Storage::open`
fails closed if SQLite doesn't confirm them, rather than trusting the `PRAGMA` calls silently
succeeded.

**Not attempted**: Milestone 3.2 (repository/transaction traits mapping all 18 tables to/from
`a2d-domain` entities), 3.3 (asset commit protocol — temp write, hash, atomic rename), 3.4
(interruption/failure-injection tests). Each is substantial on its own — 3.2 alone means writing
and testing CRUD mapping for every entity — and this is a natural checkpoint to surface status
rather than pushing further without any visibility, given how much ground Milestones 1.1–3.1
already cover.

Full workspace gate green throughout (34 tests total: 4 a2d-core + 18 a2d-domain + 4 a2d-ffi unit
+ 2 a2d-ffi binding-generation + 6 a2d-storage).

## 2026-07-26 — Ralph loop: Milestone 3.2, 3.3, 3.4 — Milestone 3 complete

User said "Continue with Milestone 3.2, 3.3 and 3.4" after the checkpoint above.

**Found and fixed a real gap from Milestone 2.3 first**: every entity except `Page` only had an
`id()` getter, no public constructor — fine for enforcing "identity cannot change after
creation," but it meant nothing outside `a2d-domain` (including the repository layer this task
needed to write) could reconstruct an entity from a database row. Added `new()` to
`NotebookDesign`, `Notebook`, `Asset`, `Scan`, `PageSet`, `OcrRun`, `AuditEvent`, and a
`Page::from_stored` alongside the existing `Page::new` (which defaults `preferred_scan_id`/
`updated_at_ms` — the storage layer needs to set those explicitly when reading a row back, not
default them). Committed this separately since it's a standalone, separable correction to 2.3,
not new 3.2 work.

**3.2 — repository layer**: 8 traits (`NotebookDesign`/`Notebook`/`PageSet`/`Page`/`Asset`/`Scan`/
`OcrRun`/`AuditEvent`Repository), implemented directly on `rusqlite::Connection` rather than a
custom wrapper — `Transaction` derefs to `Connection`, so the exact same method call
(`tx.insert_page(...)`) works both inside and outside a transaction without a second
implementation. `Storage` re-implements each trait by delegating to its own connection (via a
small macro, `delegate_repository!`, to avoid hand-copying 8 near-identical delegation blocks).
Scoped to what this milestone's own transaction example and 3.3's asset protocol actually need
(7 entities) rather than all 18 tables — the rest arrive with the milestone that needs them.

List/map-shaped columns round-trip through `serde_json` (`json_columns.rs`), fallibly in both
directions — a corrupt JSON column or an unrecognized enum string reads back as an `Integrity`
error, never silently defaults to empty (tested: `a_corrupt_enum_column_fails_closed_instead_of_defaulting`).
SQLite constraint violations (UNIQUE/PRIMARY KEY/FOREIGN KEY/NOT NULL) map to specific
`STORAGE_*_VIOLATION` codes under `ErrorCategory::Validation`, not a generic storage error,
by reading `rusqlite`'s extended result codes.

**TODO 3.2's "require transactions for X" bullet is only partially honest-checked** — only "scan
registration" is actually demonstrated composed through `Storage::transaction` (mirrors the
TODO's own example almost verbatim). Notebook creation and page-set generation are single-row
inserts right now with nothing else yet to compose transactionally (that composition is
Milestone 6's job); "OCR replacement" and "restore merge" aren't implemented at all — OCR runs
are append-only here (no replace semantics defined), and restore merge needs the backup format,
Milestone 13. Left that checkbox unchecked with this explanation rather than claim more than what
exists.

**3.3 — asset commit protocol** (`assets.rs`, spec §16.3): temp write → flush/close → compute
*and verify* SHA-256 (re-reads the temp file after flush and re-hashes it, rather than trusting
the in-memory bytes were what actually landed on disk) → atomic rename → caller commits the DB
row separately. Deliberately does NOT touch the database itself — kept `AssetStore` (filesystem)
and the DB repository separate on purpose, so a caller composing a larger transaction (e.g. a
future scan registration: asset + scan + page update, all one commit) calls `AssetStore::commit`
first, then folds `Storage::transaction`'s repository calls around its result.
`sha2` added as a new dependency (RustCrypto, small, permissively licensed). Originals get marked
read-only on disk via `std::fs::Permissions::set_readonly` (portable, not Unix-specific) in
addition to the `immutable` DB flag. `resolve()` defends against a corrupted/tampered
`relative_path` value by canonicalizing and checking containment against the library root before
ever returning a path — writes don't need this (paths are always self-generated from an
`AssetId`), but reads do, defensively.

**3.4 — integrity/interruption tests surfaced two real gaps I fixed as part of this task**,
not just tested existing behavior:
1. No way to re-verify a previously committed asset against the filesystem later (only checked at
   write time). Added `AssetStore::verify` — checks the file still exists and still hashes to the
   recorded value, both `Integrity`-category errors, not silently accepted.
2. `Storage::open`/`open_in_memory` never set `PRAGMA busy_timeout` (rusqlite's default is 0ms),
   meaning any second writer would have failed immediately with `SQLITE_BUSY` instead of waiting
   for the lock to clear. Set to 5000ms (a starting default, not a measured threshold, flagged
   inline like the journaling-mode choice) and verified by re-querying, same pattern as
   foreign_keys/journal_mode. Proved with a real two-thread test: one thread holds a write
   transaction for 300ms, the other's write blocks and then succeeds rather than erroring
   immediately (asserts elapsed time, not just success).

Two 3.4 bullets left honestly unchecked: "interrupt after rename but before DB commit" (this
failure mode is already structurally safe by the `AssetStore`/`Storage` split, but has no
dedicated test proving that specific window) and "map disk-full behavior explicitly" (needs a
genuine size-limited filesystem to test against, not portably available here — the generic I/O
error path would catch a real ENOSPC, but that's untested against the real condition).

**Milestone 3 (Rust-owned SQLite and assets) is now complete**, modulo the honestly-unchecked
items above. Full workspace gate green throughout (54 tests total across the workspace: 4
a2d-core + 18 a2d-domain + 4 a2d-ffi unit + 2 a2d-ffi binding-generation + 8 a2d-storage unit +
18 a2d-storage integration).

## 2026-07-26 — Correction: Android tooling is NOT blocked in this environment

**The earlier "Milestone 1.2 not attempted: no gradle/Android Studio" note above was wrong.** I
only checked `which gradle` (a system-wide binary), which is irrelevant — Android projects use
the Gradle *wrapper*, not a system install. The user pointed out an emulator is available; on
checking properly, this environment actually has: a full Android SDK (`~/Android/Sdk`, platforms
33–37, build-tools up to 36.0.0), a pre-created AVD (`Medium_Phone_API_36.0`), working KVM access
(`/home/phil/.../kvm` group membership confirmed), and **already-unpacked Gradle distributions**
under `~/.gradle/wrapper/dists/` (8.7 through 9.6.0) — meaning gradle builds have run here
successfully before. Bootstrapped `apps/android/gradlew` by invoking the unpacked
`gradle-8.14.3/bin/gradle` binary directly (`gradle wrapper --gradle-version 8.14.3`), since there
was no other way to generate the wrapper jar from scratch without either a system `gradle` or an
existing wrapper to bootstrap from.

**Lesson for future sessions in this repo: don't declare Android work blocked without checking
for `~/Android/Sdk`, AVDs, and `~/.gradle/wrapper/dists` first** — updated CLAUDE.md accordingly.

## 2026-07-26 — Ralph loop: Milestone 1.2 (initialize Android) — complete, verified on the emulator

Built the full Kotlin + Compose Android app scaffold in `apps/android`, then proved every
acceptance criterion against the real emulator rather than just configuring it and assuming it
works — `assembleDebug`, `lint`, `test` (JVM unit test), `installDebug` (confirmed independently
via `adb shell pm list packages`), and `connectedDebugAndroidTest` (a Compose UI test that
launches `MainActivity` for real and asserts the Home screen title renders — 0 failures) all ran
for real.

**Version choices (open decisions, flagged)**:
- AGP 8.7.3 / Kotlin 2.0.21 / Gradle 8.14.3 — picked from what was already cached locally rather
  than the newest available, to avoid gambling on an untested combination.
- `compileSdk`/`targetSdk` 35, not 36 — AGP 8.7.3 warns that it's only tested through
  compileSdk 35 (36 was the platform/build-tools/AVD version otherwise available). The app runs
  fine on the API 36 emulator regardless; compileSdk governs what the code compiles against, not
  the device's own platform version. Bumping AGP instead of lowering compileSdk was the other
  option; picked the lower-risk one since 8.7.3 was already a verified-working baseline.
- `minSdk 26` (Android 8.0) — spec/TODO don't specify a floor. Picked as a modern-but-broad
  default; reasoning is in `app/build.gradle.kts`.

**Iteration notes**:
- Renaming `mipmap-anydpi-v26` → `mipmap-anydpi` (lint: the `-v26` qualifier is redundant when
  minSdk is already 26) broke the *incremental* build with a stale "resource not found" error
  that a `clean` build didn't reproduce — Gradle's incremental resource merge didn't notice the
  directory rename. Not a real bug, just a reminder that a resource-directory rename needs a
  clean build to trust the result.
- Fixed the two real lint findings (not just noise): added a placeholder adaptive icon (background
  + foreground + monochrome layers, since `MonochromeLauncherIcon` wants all three) and
  `android:dataExtractionRules`/`fullBackupContent` — the modern/legacy replacements for
  `allowBackup`, which strengthens rather than duplicates the security reasoning already in the
  manifest (Android's own backup must never carry the canonical SQLite library, since that would
  bypass the app's own encrypted `.atnb` backup path).
- Left 8 lint warnings deliberately unaddressed: 1 `OldTargetApi` (the compileSdk 35 tradeoff
  above) and 7 `GradleDependency` (newer versions of core-ktx/activity-compose/compose-bom/
  navigation-compose/test.ext:junit/espresso-core exist) — chasing every dependency bump risked
  destabilizing an already-verified-working version set for no functional benefit at this stage.

**Not done**: no UniFFI/Kotlin binding wiring into this Android project yet, so Milestone 2's
"Android calls Rust and renders a typed response" acceptance criterion is still open — this
milestone only proves the Compose/navigation/test scaffold itself. That wiring, plus the actual
QR decoder spike ADR 0001 needs (now genuinely reachable given a real emulator exists), are the
natural next steps.

TODO 1.2 fully checked, including both acceptance criteria.
