---
name: lint-n-test
description: Lint the Rust and Android code and run all tests, reporting pass/fail for each. Use only when the user explicitly invokes /lint-n-test.
model: haiku
disable-model-invocation: true
allowed-tools:
  - Bash
---

# Lint and test

Run linting and the full test suite, in this order, stopping at the first hard failure:

```
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

If `apps/android/gradlew` exists, also run (from `apps/android`):

```
./gradlew lint test
```

## Rules

1. **Never report a skipped step as a pass.** If `Cargo.toml` doesn't exist at the repo root, mark the `cargo` steps NOT WIRED UP. If `apps/android/gradlew` doesn't exist, mark the Gradle steps NOT WIRED UP. Do not substitute a weaker command for either.
2. **Clippy warnings are errors.** Do not drop `-D warnings` to get a green run.
3. **This skill does not fix anything.** Report results only. If asked for a fix-and-verify pass separately, that's a different request.
4. **Report the actual failing output**, not a paraphrase of it, for any step that fails.

## Output format

End with a status table:

| Step | Result |
|---|---|
| cargo fmt --check | PASS / FAIL / NOT WIRED UP |
| cargo clippy | … |
| cargo test | … |
| gradlew lint | … |
| gradlew test | … |

Then one line: `Lint and test: GREEN` only if every applicable step passed. Otherwise state exactly what failed.
