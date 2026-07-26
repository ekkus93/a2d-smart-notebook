---
name: a2d-check
description: Run the A2D required quality gates (cargo fmt/clippy/test/deny, Gradle lint/test/assembleDebug) in order and report which gates passed, failed, or are not yet wired up. Use before marking a milestone complete or when asked to verify the repo is green.
---

Run the required checks from spec §31 in this exact order, stopping at the first hard failure.

```
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
./gradlew lint test assembleDebug
```

## Rules

1. **Never report a skipped gate as a pass.** This repo is still being built out — `Cargo.toml`, `deny.toml`, and `apps/android/gradlew` may not exist yet. Before each gate, check whether its prerequisite exists:
   - `Cargo.toml` at the repo root → the four `cargo` gates apply
   - `deny.toml` → `cargo deny check` applies (also requires `cargo-deny` to be installed)
   - `apps/android/gradlew` → the Gradle gate applies (run it from `apps/android`)

   If a prerequisite is missing, mark that gate **NOT WIRED UP** and say why. Do not substitute a weaker command.

2. **Clippy warnings are errors.** Do not drop `-D warnings` to get a green run.

3. **Do not fix anything unless asked.** Report first. If the user asked for a fix-and-verify pass, fix the smallest thing that addresses the failure, then re-run from the failing gate onward.

4. **Report the actual output** of any failing gate, not a summary of it.

## Output format

End with a status table:

| Gate | Result |
|---|---|
| cargo fmt --check | PASS / FAIL / NOT WIRED UP |
| cargo clippy | … |
| cargo test | … |
| cargo deny check | … |
| gradlew lint test assembleDebug | … |

Then one line: `Milestone-complete gate: GREEN` only if every applicable gate passed and no gate is NOT WIRED UP that the current milestone requires. Otherwise state exactly what is blocking.
