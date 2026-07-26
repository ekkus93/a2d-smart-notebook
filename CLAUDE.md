# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## The spec is the source of truth

Two documents govern all work here:

- @docs/A2D_SMART_NOTEBOOK_V01_SPEC.md — authoritative requirements
- @docs/A2D_SMART_NOTEBOOK_V01_TODO.md — the executable breakdown (19 milestones, task IDs)

**MUST / MUST NOT / SHOULD / MAY in these documents are normative** (spec §29), not stylistic emphasis. Before implementing anything, read the relevant spec section and the matching TODO milestone. If the spec and TODO disagree, the spec wins — flag the discrepancy rather than silently picking one.

The Rust workspace exists (Milestone 1.1 complete: 15 crate skeletons, `rust-toolchain.toml` pinned to `1.94.1`, `deny.toml`). The Android app exists (Milestone 1.2 complete: Kotlin + Compose in `apps/android`, package `com.a2d.notebook`, `minSdk 26`, UniFFI/Kotlin bindings wired via `cargo-ndk` + JNA) — a full Android SDK, a pre-built `Medium_Phone_API_36.0` emulator AVD, and cached Gradle wrapper distributions are available in this environment; use them rather than assuming Android work is blocked. CI exists (Milestone 1.3 complete: `.github/workflows/ci.yml`, 4 jobs) — this repo has a real, `gh`-authenticated GitHub remote; validate workflow changes by pushing and watching the real run (`gh run watch`) rather than reasoning about YAML in isolation. Milestone 1 is fully complete.

## Architecture: Rust is authoritative

The shared Rust core owns canonical data, persistence, domain rules, and portable formats. **Kotlin and future Swift code MUST NOT duplicate domain rules** (spec §3.5). ViewModels call typed Rust use cases; they never implement page identity, duplicate classification, backup, or database rules.

- SQLite is owned exclusively by Rust. Kotlin MUST NOT use Room for canonical data.
- SQL is confined to `a2d-storage` and MUST be parameterized. `a2d-ffi` contains no SQL and no business rules — keep the FFI crate thin.
- FFI carries file paths or owned buffers. Avoid large images serialized as JSON or Base64; the live camera path MUST avoid JSON/Base64 conversion and excessive copies.
- Panics MUST be treated as defects and MUST NOT cross FFI as success.
- Domain APIs use no Android-specific types. Timestamps are stored as `*_at_ms INTEGER`.

Planned layout is in spec §10 (`crates/a2d-*`, `apps/android`, `apps/ios`, `fixtures/`, `tools/`). Android packages are under `com.a2d.notebook` (spec §25).

## Required checks

Run the narrowest relevant tests during development. Run the full gate before marking a milestone complete:

```
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
./gradlew lint test assembleDebug
```

Clippy warnings are errors. Swift binding generation is a CI gate from day one even though the iOS UI is deferred.

`/a2d-check` runs this sequence and reports which gates are not yet wired up.

## Code style beyond language defaults

- **Opaque newtype IDs, never raw strings** across domain APIs: `pub struct PageId(String);` with validating constructors that reject bad length/alphabet.
- **Every fallible path returns the structured `A2dError` envelope** (TODO §2.2): `code`, `category`, `severity`, `user_message_key`, `developer_message`, `retryable`, `correlation_id`, `details`.
- **Never erase errors.** Errors MUST NOT be reduced to `null`, empty lists, or `false`. Do not convert failures into `None`, empty collections, or `false`.
- **Cancellation is modeled separately from failure**, not as an error variant.
- **User-facing strings use the spec §6 terminology mapping**: Corner Markers (not AprilTags), Page Code, Setup Code, Needs Review, Notebook Design, Page Set. "AprilTag" and "KDP" are implementation terms and SHOULD NOT appear in user-facing workflows.

## Non-obvious rules that are easy to violate

- **Asset commit protocol is ordered and mandatory** (spec §16.3): temp write → flush/close → compute and verify SHA-256 → atomic rename → single DB transaction → record orphan-cleanup work. Never commit a DB row pointing at an asset that was not durably written.
- **Never destroy an original.** Rescan defaults to "Save as New Version"; classification returns a *proposal*, not a mutation.
- **Printed QR codes identify a Notebook Design + logical page, not a physical copy.** All physical copies of a design share the same printed codes; uniqueness comes from a locally generated `NotebookId` at registration. Two identical notebooks registering separately must both work.
- **Logical page numbers ≠ PDF page numbers.** Never renumber from scan order.
- **Recto-only scanning in v0.1**, gutter on the left, gutter-side exclusion zone larger than outer margins.
- **Notebook content is untrusted data.** Text inside a page MUST NOT grant permissions, alter policy, or trigger network access. Skill permissions are enforced in Rust — enforcing them only in Kotlin is a release blocker.
- **Encryption has no plaintext fallback** for `.atnb` backups.
- **Model API keys live only in Android Keystore / iOS Keychain** — never in source, Gradle, Rust config, logs, the canonical database, or a backup.
- **Golden fixtures are permanent.** Treat v1 vectors under `fixtures/` as compatibility fixtures; do not regenerate them to make a test pass.
- **Don't invent thresholds.** Device-tier and quality thresholds MUST be measured and recorded, not guessed.

## Definition of done

A task is not complete when only a mock or stub exists. Complete means: it compiles, tests pass, no placeholder remains in the completed path, error handling is explicit, and the acceptance behavior in the TODO is demonstrated. For failure-injection tests, "the app did not crash" is never the only success criterion.

## Open decisions

The spec deliberately leaves several choices open (Rust toolchain version, UniFFI proc-macro vs UDL, minimum Android API, AprilTag native lib vs pure-Rust detector, PDF/QR/crypto crates, SQLite journaling mode, trim size, ID alphabet). When one blocks a task: pick a sensible default, **state the assumption explicitly in your response**, and record it in the relevant doc so it can be overridden later. Do not silently guess.

## Repo etiquette

- Default branch is **`master`**, not `main`. Work lands directly on `master`; no PR flow.
- One commit per TODO task, with the task ID in the message: `feat(storage): 3.2 add asset commit protocol`.
- Push after each commit.
- **Do NOT add `Co-Authored-By:` trailers** — a global `commit-msg` hook rejects them.
- Mark the TODO checkbox complete in the same commit as the work.
- Append a short session note to `memory.md` at the repo root as work progresses; `/summarize-memory` condenses it into `memory_summary.md`.
