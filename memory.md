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

## 2026-07-26 — Ralph loop: UniFFI-Kotlin wiring + the real ADR 0001 QR decoder spike

Both requested. Everything below ran for real on the `Medium_Phone_API_36.0` emulator, verified
via actual JUnit XML reports, not just build-success exit codes.

**UniFFI/Kotlin wiring** (closes Milestone 2's last open acceptance criterion, "Android calls
Rust and renders a typed response"):
- `rustup target add x86_64-linux-android` (+ aarch64/armv7/i686 for future devices) and
  `cargo-ndk` (already installed) cross-compiled `a2d-ffi` to `liba2d_ffi.so` for x86_64 — matches
  the emulator's actual ABI (confirmed from its boot log: `system-images/android-36/
  google_apis_playstore/x86_64/`), copied straight into `app/src/main/jniLibs/x86_64/` via
  `cargo ndk -o`.
- Generated Kotlin bindings from that `.so` (not the desktop one) via the existing
  `uniffi-bindgen` bin target, placed at `app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt` — package
  path matches source path per Kotlin convention.
- Added JNA (`net.java.dev.jna:jna:5.14.0@aar` — the Android-packaged variant; the plain desktop
  jar doesn't work on-device) since the generated bindings load the native lib through it.
- `com.a2d.notebook.rustbridge.A2dBridge` (spec §25's package) is the thin Kotlin façade feature
  code calls instead of importing `uniffi.a2d_ffi` directly.
- Home screen now renders a real `PageId` from `A2dClient.generatePageId()`; a new instrumented
  test (`homeScreenRendersARealPageIdGeneratedByRust`) asserts the rendered text matches the
  26-char canonical Crockford Base32 shape — proving the whole chain (Rust generate -> UniFFI/JNA
  -> Compose render) rather than just that it compiles.
- Lint caught a real issue, not noise: UniFFI's generated cleaner code calls
  `java.lang.ref.Cleaner` (API 33+), tripping `NewApi` against our `minSdk 26`. Checked the actual
  generated code before dismissing it — it's guarded by `Class.forName("java.lang.ref.Cleaner")`
  + `catch (ClassNotFoundException)` falling back to a JNA-based cleaner, a deliberate safe
  compatibility shim Lint's static analysis can't reason about. Added `app/lint.xml` suppressing
  `NewApi` scoped only to `src/main/kotlin/uniffi/**`, not the whole module.

**The real QR decoder spike** (ADR 0001's Validation Evidence):
- Added a minimal QR payload **encoder** (not the parser/decoder or golden fixtures -- those stay
  Milestone 4.2/4.3's job) implementing the ADR's exact grammar: `crates/a2d-identity/src/qr.rs`,
  `PageCode::encode()`. Needed CRC-32C (`crc32c` crate) and a 7-char Crockford Base32 encoding for
  it (32 bits -> 7 chars, same MSB-padding convention as the 128-bit id encoder, duplicated in
  miniature rather than generalizing `a2d-domain::id`'s internals for a second bit-width).
  8 new tests, all passing.
- Found a real gap while wiring this up: `LayoutId` (added in Milestone 2.3) had `as_str()` but
  no `Display` impl, so it couldn't be interpolated into `format!` strings. Fixed directly in
  `a2d-domain`.
- Exposed three `generate_example_*_qr_payload()` methods on `A2dClient` (one per code type:
  NotebookSetup/NotebookPage/SmartPage), each generating a **fresh random** payload per call, not
  a fixed fixture -- `a2d-core` -> `a2d-ffi` -> Kotlin, same pattern as `generate_page_id`.
- `QrDecoderSpikeTest.kt` (androidTest): Rust generates the canonical text across the real FFI
  boundary; ZXing (`com.google.zxing:core`, androidTestImplementation only -- explicitly not the
  production decoder choice, Milestone 7.4/12 still owns that) renders it to an actual QR bitmap
  and decodes it back; asserts byte-for-byte equality. Covers all three type-codes plus a
  small-render-size variant for the worst-case (SmartPage) payload. 7/7 tests passing on the real
  emulator (2 Home-screen tests + 5 QR spike tests).
- **Updated ADR 0001's Validation Evidence**: checked off "prove the grammar survives a real
  render/decode round trip" with a full account of what was and wasn't proven (ZXing isn't
  necessarily identical to whatever decoder ships in production). Left the *second* checklist
  item -- worst-case payload at the real physical layout's module size/damage tolerance -- open,
  since that needs Milestone 5's actual layout to test against, not just a smaller render size as
  a proxy. **ADR 0001's status stays Proposed, not Accepted** -- one of its two required items is
  still open, and `fixtures/qr/v1/` still MUST NOT be committed yet.

Full Rust gate green (63 tests total: 5 a2d-core + 18 a2d-domain + 4 a2d-ffi unit + 2 a2d-ffi
binding-generation + 8 a2d-identity + 8 a2d-storage unit + 18 a2d-storage integration). Full
Android gate green (`lint test assembleDebug`, 0 errors / 9 documented-tradeoff warnings) plus
7/7 on the real emulator.

**Build-artifact decision, made deliberately, not by default**: the cross-compiled `.so` stays
gitignored (already covered by the existing native-build pattern) — a compiled binary in git is
the wrong tradeoff when it's this cheap to rebuild. The generated Kotlin bindings
(`app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt`), though, **are committed** — text, diffable, and
nothing in the Gradle build regenerates them automatically (no Cargo/NDK Gradle integration
exists yet, e.g. the `mozilla/rust-android-gradle` plugin or a custom task). Without committing
it, a fresh clone's Android build would simply fail to find the source file. Formalized the
regeneration step as `tools/build-android-native.sh` (cross-compiles + regenerates bindings in
one command) rather than leaving it as unwritten-down manual steps — run it after any
a2d-ffi/a2d-core/a2d-domain/a2d-identity change, before building the Android app. Wiring this into
Gradle automatically is a real follow-up, not done here.

## 2026-07-26 — Ralph loop: Milestone 1.3 (CI) — Milestone 1 complete

User picked this up after asking "are there any other tasks that still need to be implemented" and
getting a full status survey. `act` (local GitHub Actions emulation) wasn't installed and pulling
its Docker images seemed like the wrong tradeoff; instead used the fact that this repo has a real
`gh`-authenticated GitHub remote we've been pushing to all session — pushed the workflow and
watched it run for real via `gh run watch`/`gh api .../logs`, exactly like every other tool this
session got validated against its actual target rather than assumed correct. **This paid off
immediately**: the first real run failed on 2 of 4 jobs, both genuine bugs no amount of local
reasoning would have caught.

`.github/workflows/ci.yml`: four jobs — `rust` (fmt/clippy/test), `deny` (`cargo deny check`),
`android` (`gradlew lint test assembleDebug`), `android-binding-drift` (cross-compiles `a2d-ffi`
for Android, regenerates Kotlin bindings via `tools/build-android-native.sh`, `git diff
--exit-code` against what's committed). No explicit Rust toolchain-install action — relies on
`rust-toolchain.toml` (already pins 1.94.1 + rustfmt/clippy) and rustup's auto-detection, which
GitHub's runners have preinstalled. "Swift UniFFI binding generation smoke check" isn't a
separate job — `cargo test` in the `rust` job already exercises
`crates/a2d-ffi/tests/binding_generation.rs`, which does that. "Fixture compatibility checks
after fixtures exist" isn't a job yet — no fixtures exist (blocked on ADR 0001).

**Real failure 1 — `cargo test` (binding_generation.rs)**: assumed the `a2d-ffi` cdylib was
already sitting in `target/debug/` before locating it. True in every local run this whole
session, but only because earlier, unrelated `cargo build -p a2d-ffi --lib`/`cargo ndk` commands
had already populated it as a side effect — `cargo test --workspace` alone does not reliably
build the cdylib flavor of a `crate-type = ["lib", "cdylib"]` package, since test binaries only
need the rlib flavor to link against. A textbook "worked on my machine" bug, caught only because
CI actually starts from a clean checkout. Fixed by having the test explicitly run `cargo build -p
a2d-ffi --lib` itself first; verified the fix locally too, by deleting the `.so` and re-running
before trusting it and pushing again.

**Real failure 2 — `cargo deny check`**, two distinct, legitimate findings:
- MPL-2.0 (uniffi's license) wasn't in `deny.toml`'s allow list. Added it with reasoning inline —
  weak/file-level copyleft, doesn't restrict how code merely *depending* on an MPL-2.0 library
  (us) can be licensed, unlike GPL.
- Every intra-workspace path dependency (`a2d-core` depending on `a2d-domain` via `path = "..."`)
  was flagged as a "wildcard" dependency. `allow-wildcard-paths = true` exists for exactly this,
  but only applies to crates marked `publish = false` — none of ours were. Added `publish = false`
  once at `[workspace.package]` (all 15 crates are internal to this app, never meant to publish
  independently — the more correct fix regardless of the cargo-deny angle) and
  `publish.workspace = true` to each crate.
- Installed `cargo-deny` locally afterward specifically to verify the fix directly rather than
  trusting the reasoning and re-pushing blind a second time.

Second push: all 4 jobs green for real (`gh run watch` confirmed `✓ master CI`). This sequence —
a genuine failing run followed by a genuine passing one, both real GitHub Actions runs, not
staged — is itself the evidence for TODO 1.3's "deliberate formatting and test failures block
CI" acceptance criterion; no separate demonstration was needed.

**Milestone 1 (Repository, toolchain, Android shell, and CI) is now fully complete** — 1.1, 1.2,
and 1.3 all done, all their acceptance criteria genuinely verified rather than assumed.

## 2026-07-26 — Ralph loop: closed two Milestone 2 gaps

User: "Close the small Milestone 2 gaps, then move into Milestone 4." Both gaps were left
unchecked earlier specifically because they needed infrastructure that didn't exist yet at the
time (storage, a real Android FFI consumer) — both now exist.

**"A scan always references an immutable original asset"**: `ScanRepository::insert_scan` (a2d-
storage) now looks up `original_asset_id` before inserting and rejects the scan -- never letting
it reach the database -- if the asset doesn't exist (`STORAGE_SCAN_ORIGINAL_ASSET_MISSING`) or
isn't marked immutable (`STORAGE_SCAN_ORIGINAL_ASSET_NOT_IMMUTABLE`). Two new tests. All existing
tests that insert scans already used immutable originals via `AssetStore::commit(...,
AssetKind::Original, ...)`, so nothing broke by accident -- which itself is a small positive
signal that the existing test fixtures were already doing the right thing.

**"Prevent panics from appearing as successful FFI results"** — the higher-stakes one, since
getting this wrong could crash the whole test process, not just fail a test. Read the generated
Kotlin bindings first (`uniffiCheckCallStatus` in `uniffi/a2d_ffi/a2d_ffi.kt`) before writing
anything: confirmed UniFFI's scaffolding sets `UniffiRustCallStatus.code = CALL_UNEXPECTED_ERROR`
on a caught panic, which the Kotlin wrapper turns into a thrown `InternalException` carrying the
panic message -- not a silent return, not a process abort. Only after confirming that did we
write `PanicPropagationTest.kt`, which calls `A2dClient.triggerPanicForTesting()` on the real
emulator and asserts on both facts (an exception was thrown, and it carries the actual panic
message, not a generic one). Genuinely ran on-device: 8/8 instrumented tests passed per the
actual JUnit XML report (`aRustPanicSurfacesAsAKotlinExceptionRatherThanASilentSuccessOrACrash`,
0.238s) -- the Gradle console's live progress line said "4/8 completed" at the moment the log was
captured, which would have been alarming if trusted at face value; checked the XML report instead
of assuming, and it was just a mid-run snapshot, not a real problem.

Full gate green throughout: Rust (fmt, clippy, `cargo deny check`, 33 test binaries — now 20 in
`a2d-storage`'s integration suite, up from 18), Android (`lint test assembleDebug`), 8/8
instrumented tests on the real emulator.

Both TODO 2.4/2.3 checkboxes ticked with the real evidence inline. Next: Milestone 4 (identity,
QR protocol, Notebook Designs) proper — the encoder from the QR spike work already exists ahead
of schedule (`a2d-identity::qr::PageCode::encode`); 4.2/4.3 (parser, strict validation, golden
fixtures) are still blocked on ADR 0001 reaching Accepted, but 4.1 (random ID generation, already
substantially covered by `a2d-domain::id`) and 4.4 (Notebook Design manifests) don't have that
dependency.

## 2026-07-26 — Ralph loop: Milestone 4.1, 4.2 (parser), and 4.4

Confirmed CI green for the Milestone 2 gap-closure commit first (`gh run view 30224009752` — all
4 jobs ✓) before starting new work, per this project's standing "verify for real" discipline.

**4.1 Random ID generation**: four of five bullets were already satisfied by Milestone 2.1's
`a2d_domain::id` and just needed ticking with a pointer to where. The one genuinely new piece:
"detect persistence collisions as hard integrity events." Realized every `id` column in the
schema is that table's primary key, so SQLite's extended error code already distinguishes the
two cases precisely — 1555 (`SQLITE_CONSTRAINT_PRIMARYKEY`) means a freshly generated 128-bit ID
already exists (should be near-impossible; an RNG failure or reused ID, a real integrity event),
while 2067 (`SQLITE_CONSTRAINT_UNIQUE`) means an ordinary business-rule unique index fired (e.g.
one logical page per notebook — caller/business error, not infrastructure). Split
`map_sql_error` on that code: 1555 now maps to `STORAGE_ID_COLLISION` /
`ErrorCategory::Integrity` / `ErrorSeverity::Critical`, distinct from
`STORAGE_UNIQUE_CONSTRAINT_VIOLATION` / `Validation`. This *changed* the behavior of an existing
test (`duplicate_insert_maps_to_a_validation_error...` was literally the ID-collision case, since
it re-inserted the same `NotebookDesign` — same ID, same PK) — renamed and rewrote it to assert
the new Integrity/Critical mapping, and added an explicit assertion to the sibling test that a
genuine business-rule unique-index violation (fresh ID, same logical-page-number) still stays
Validation, so the two paths don't silently drift back together later.

**4.2 QR payload model (parser half)**: the encoder already existed (built ahead of schedule for
the ADR 0001 Android spike); added `a2d_identity::qr::parse`, the strict decoder, directly against
the ADR's own "Strict-parser rules" list — one test per rule (lowercase/bad-charset, wrong magic
prefix, unsupported version, unknown type-code, wrong field count/trailing data, `id128` wrong
length, `id128` with I/L/O/U, numeric field leading-zero/sign/out-of-range, unregistered
`layout-id`, CRC mismatch, oversized payload), plus round-trip tests for all three `type-code`s
and a test parsing the ADR's own illustrative `NotebookPage` example (which correctly fails on CRC
mismatch, since that example's CRC was hand-illustrative, not computed against this
implementation — good: proves the parser never trusts an unverified integrity field even when the
rest of a payload looks legitimate). One real bug caught by the tests themselves: initially
checked the CRC *before* decoding fields, which meant hand-corrupted test payloads (bad id128
length, leading-zero numeric field, etc.) failed with a generic `CRC_MISMATCH` instead of their
specific field error, since corrupting a field also invalidates the CRC computed over the original
payload. Reordered to validate fields first, CRC last — matching the order the ADR's own rule list
actually gives, not just fixing tests to match code. The `layout-id` registry-membership check
takes the registry as a caller-supplied predicate (`impl Fn(&LayoutId) -> bool`) rather than
`a2d-identity` depending on `a2d-layout` directly, since `a2d-layout`'s real registry doesn't
exist until Milestone 5 — avoids a forward dependency that would need unwinding later. Golden
fixtures (4.3) remain untouched — ADR 0001 is still Proposed, v1 fixtures are permanent once
committed, and this parser's own thoroughness doesn't change that gate. Deviation recorded in the
TODO: 4.2's illustrative `PageCode` code sample gives each variant a `version: u8` field, but the
actual type has none — `version` is a fixed `"1"` literal in the wire grammar (the ADR: "v1
parsers understand `"1"` only"), so there's no per-value version to store; a future v2 grammar
gets its own parser function, not a runtime field.

**4.4 Notebook Design manifests**: no real physical Notebook Design exists yet (trim size, marker
family, real layouts are all Milestone 5 decisions), so this built the *mechanism* — a versioned
JSON manifest shape (`crates/a2d-layout/src/manifest.rs`), `parse_manifest` converting it into the
existing `NotebookDesign` entity (Milestone 2.3) and computing `manifest_hash` as the SHA-256 of
the manifest's exact source bytes, `ManifestRegistry` as a fully-offline in-memory `HashMap`
lookup (rejects two manifests sharing an id rather than letting the second silently shadow the
first), and one manifest deliberately named and documented as a development placeholder
(`manifests/dev-placeholder.json`, `bundled_placeholder_registry`) rather than pretending to bundle
something official. Left "Bundle initial official manifests" unchecked in the TODO for exactly
that reason — ticking it would overclaim; Milestone 5 supplies the real content, and this registry
mechanism shouldn't need to change when it does. Trust state is deliberately *not* a field inside
the manifest JSON itself — assigned by the loader from provenance (bundled-with-a-reviewed-build →
`Trusted` for v0.1) rather than self-declared by the manifest, leaving room for a future
signed-manifest extension (spec §14.4) to supply its own trust derivation through the same
`parse_manifest(json, trust_state)` call shape without changing the wire format. `serde`,
`serde_json`, `sha2` reused at the same versions `a2d-storage` already uses rather than picking
new ones.

Full gate green: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings`, `cargo test --workspace --all-features` (33 test binaries, all passing — up from 20 in
`a2d-storage` alone last session; `a2d-identity` now 29 tests, `a2d-layout` now 11), `cargo deny
check` (clean after adding `a2d-layout`'s three new deps, no new license/wildcard findings).

Milestone 4 remaining: 4.3 (golden fixtures) and the milestone's overall acceptance criterion stay
blocked on ADR 0001 reaching Accepted, which itself is blocked on Milestone 5 producing a real
physical layout to test the worst-case payload against (the ADR's one open Validation Evidence
item). Next reasonable step once picked back up: Milestone 5 (layout engine and PDF generation) —
starting it would also unblock ADR 0001's last open item and, transitively, Milestone 4.3.

## 2026-07-26 — Ralph loop: Milestone 5 (layout engine and PDF generation), 5.1-5.4

Confirmed CI green for Milestone 4's commit (`0bfb18d`, all 4 jobs) before starting, per this
project's "verify for real" discipline, then worked straight through Milestone 5.1-5.4 as one
continuous session (user: "Continue").

**5.1 (canonical physical layout model)**: new `a2d-layout::geometry` (`PhysicalSize`/
`PhysicalPoint`/`PhysicalRect`, millimeters, top-left origin/y-down — documented explicitly as a
choice, since PDF's own coordinate system is bottom-left/y-up and converting is `a2d-pdf`'s job,
not this crate's) and `a2d-layout::page_layout` (`PageLayout`, `MarkerRole`, `MarkerPlacement`,
`CalibrationMark`, matching the TODO's own suggested shape closely). `PageLayout::validate`
checks marker-role uniqueness, safe-margin bounds, and quiet-zone overlap between every
machine-readable element (markers + QR) and content/page-number/each-other. 25 tests, all passing
on the first real attempt after careful geometry derivation up front.

**5.2 (Smart Page layouts)**: `smart_page_layout(PaperSize, SmartPageStyle)` builds all 8
US Letter/A4 x Blank/Lined/DotGrid/Graph combinations. Added `ContentStyle` to `PageLayout`
(line/dot/graph spacing as physical measurements, not a rendering concern) despite it not being
in 5.1's original suggested struct — deliberate, since the TODO's own framing of 5.2 as "8
things" (not "2 layouts x 4 render styles") implies each combination needs to be independently
identifiable via its own `LayoutId`, which only makes sense if the style is layout metadata.
Physical constants (6mm safe margin, 3mm quiet zone, 18mm marker/QR size) recorded as CLAUDE.md
open-decision assumptions, not measured values.

**5.3 (bound-notebook layout)**: first trim-size decision — 152x229mm (6x9in), a common
print-on-demand journal trim. Extracted `layout_builder::build_layout` out of 5.2's
`smart_page_layout` so both layout families share one geometry formula (`left_margin_mm` vs.
`margin_mm` lets the notebook's 20mm gutter exclusion and Smart Pages' symmetric margins reuse
the same code) — refactored 5.2 to call it too, verified byte-identical behavior via its existing
test suite before moving on. `pdf_page_number_for_logical_page` maps logical page numbers to PDF
page positions around interleaved blank versos (logical 1 -> PDF page 3, etc.), satisfying "logical
page numbers != PDF page numbers" without needing a PDF renderer to exist yet. Deferred "generate
blank verso pages"/"generate proof interior PDF" to 5.4 explicitly rather than faking them here.
**One real bug caught by tests, not assumed away**: the notebook's 152mm width (much narrower than
Letter/A4's 210mm+) broke 5.2's original page-number placement formula (fixed horizontal offset
next to the QR) — ran out of room before the BR marker's quiet zone, genuine `validate()` failure.
Fixed by moving the page number into its own reserved horizontal strip above the marker/QR row
(scales with any page width) rather than patching the specific narrow-page numbers. This is
exactly the kind of bug "write the formula once, trust it, ship" would have missed — it only
surfaced because every concrete layout actually gets validated in tests, every time.

**5.4 (PDF renderer)**: new crate `a2d-pdf`, using `printpdf` 0.12 (vector drawing + `BuiltinFont`
standard-14 fonts, no embedded font file, so no font-licensing question at all) and `qrcode`
(module matrix only, no image/svg features — module squares rendered as our own vector polygons
for "integral module scale"). Before writing any code, read printpdf 0.12's actual source under
`~/.cargo/registry/src/.../printpdf-0.12.4/src/` directly (WebFetch on docs.rs/GitHub returned
prose without real code examples for this fast-evolving crate) to get exact struct/enum shapes —
paid off: `a2d-pdf` compiled clean on the very first `cargo build`, no trial-and-error API
discovery loop. Corner Markers are an explicit placeholder shape (bordered black square) — real
AprilTag bit pattern waits for Milestone 7's ADR 0002, documented prominently so it can't be
mistaken for a finished scannable marker. `generate_smart_page_pdf`/`generate_page_set_pdf`/
`generate_notebook_proof_interior_pdf` all commit via write-to-temp -> re-parse-to-verify (with
`PdfParseOptions{fail_on_error: true}`, not the lenient default) -> atomic rename, mirroring spec
§16.3's asset commit protocol. A `debug_assert_eq!` inside the notebook-interior loop ties its
page construction order directly to 5.3's `pdf_page_number_for_logical_page`, so the two
independently-written pieces can't silently drift apart. 17 tests, `cargo deny check` clean with
printpdf+qrcode's full transitive tree (nothing needed adding to `deny.toml`'s allow-list).

Full gate green after every one of 5.1/5.2/5.3/5.4 individually (not just at the end): `cargo fmt
--check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test
--workspace --all-features` (33 test binaries throughout, individual crate test counts climbing:
a2d-layout 25->32->43, a2d-pdf 0->17 new), `cargo deny check`. Four separate commits, four
separate real GitHub Actions runs, all confirmed green via `gh run view`/`gh run watch` before
moving to the next task — not batched into one lump commit at the end.

Remaining in Milestone 5: **5.5** (transactional generated-page registration — needs a2d-storage +
a2d-pdf wired together, likely in a2d-core, still essentially empty) and **5.6** (PDF tests
requiring rasterization + marker/QR detection from rendered images — genuinely blocked on
Milestone 7's detector existing; the TODO's own acceptance criterion "a generated page can be
printed, photographed, identified, and rectified" cannot be satisfied without it). Milestone 5's
overall acceptance is therefore blocked on Milestone 7 regardless of anything else done here.
