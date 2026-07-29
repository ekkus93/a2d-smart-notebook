# FIX-001 UniFFI Binding Regeneration Evidence — 2026-07-29

## Scope

This record documents the generated-artifact provenance used to repair the committed Android Kotlin UniFFI binding. It does not claim FIX-001 complete until permanent CI passes on the exact final `master` commit.

## Generator run

- Workflow: `CI`
- Run ID: `30488407969`
- Generator source commit: `2849303af3e8e5fe692da750c3a40f4498bd34b3`
- Binding job: `Kotlin UniFFI binding generation drift check`
- Artifact: `regenerated-android-ffi-files`
- Artifact ID: `8738669551`
- Artifact ZIP SHA-256: `dd615fc8347585d048ac9e52d41eb3b629fddcb5c870af21ac9e700b44484616`
- Artifact expiration reported by GitHub: `2026-10-27T20:25:46Z`

The generator step completed successfully. The job then failed at the expected drift comparison because the committed Kotlin binding was stale.

## Imported generated files

The artifact contained:

- `apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt`
  - byte length: `151166`
  - SHA-256: `72a71f7b072c4278677ec0cdf711ca65bbc3151e7c7e6f9def6b63053a2432b0`
- `Cargo.lock`
  - byte length: `55876`
  - SHA-256: `fc345ffa3f04cb4f285eb6653054e4b9368b2d4d01d229ee07452733b6a33ce4`

Before import, the workflow verified:

- the artifact source commit was an ancestor of the import head;
- no `crates/a2d-ffi` or `crates/a2d-core` file changed after the generator source commit;
- the downloaded ZIP matched GitHub's published digest;
- the generated binding contained `compareStoredScans`, `A2dFfiErrorDetail`, and `StoredScanComparisonEvidence`.

The generated Kotlin file was committed verbatim. UniFFI `--no-format` output contains generator-owned trailing whitespace; repository whitespace checks intentionally exclude this generated file rather than modifying it manually.

## Associated source cleanup

The same import commit applied two behavior-neutral clippy repairs discovered by permanent CI:

- removed an unnecessary `mut` from the PDF detector used during render verification;
- removed a redundant `.into_iter()` in a marker-layout test.

Rust formatting was applied after those handwritten-source changes.

## Import result

- Import workflow run: `30489289570`
- Import trigger commit: `ebbfbad08ba804c57ee636b9346825d24641a68c`
- Generated binding/source commit: `c3c290ba21bc167c5102052694d493384f5d3fa3`
- Artifact verification/import: passed
- Clippy source cleanup/format: passed
- Commit/push: passed

## First permanent validation after import

- Workflow run: `30489359526`
- Validation commit: `36b087f8586276c78526a2ab21d40e54b511fe69`
- Kotlin UniFFI binding regeneration and drift comparison: passed
- Android native ABI builds: passed
- Android lint, JVM tests, and debug APK assembly: passed
- APK native-library, detector-linkage, symbol, and notices verification: passed
- Rust formatting drift: passed
- Full-workspace clippy: failed while compiling `a2d-pdf` integration test `printable_compatibility`

The remaining Rust failure showed that `render_page_ops` is an intentionally tested public compatibility API. The implementation was made public and the crate-root re-export restored in commit `ba46eb1322909ddd87d2eb8c4544bcec737345bc`.

## Remaining acceptance gates

FIX-001 remains pending until permanent CI passes on the exact final validation head, including:

- Kotlin binding drift;
- Kotlin and Swift binding-generation smoke tests;
- full Rust formatting, clippy, and tests;
- Android compilation, unit tests, lint, native ABI packaging, and APK verification.
