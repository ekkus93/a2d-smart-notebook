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
