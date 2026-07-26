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
