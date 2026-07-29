# FIX-001 UniFFI Binding Regeneration Evidence — 2026-07-29

## Status

FIX-001 implementation and source validation are complete. The committed Android Kotlin UniFFI binding matches the Rust FFI surface, Android compiles from a fresh checkout, Kotlin and Swift generation smoke tests pass, and permanent CI completed successfully on the validated source head recorded below.

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

## Preferred-scan trigger correction

The next full test run exposed a defect in migration 0006. Its two-phase synchronization first cleared every preferred flag after the page pointer already referenced the selected scan. The migration-0005 guard correctly rejected clearing that selected scan.

Migration 0007 fixes forward by:

- clearing only non-selected preferred scans;
- setting the selected scan after the clear phase;
- retaining the partial unique index and page-pointer/scan-flag guards.

Relevant commits:

- `f8f8eb0c3199bfc3f747e86fcec6ca3852946ea8` — add migration 0007 SQL
- `f8a188e8094c274d6ec5328efc7a686cebdfed57` — add migration 0007 to the immutable catalog

That correction reduced the `a2d-core` failures from ten to two. The remaining failures were test-fixture defects, not production failures:

- an extensionless corrected asset was opened by filename inference instead of decoded from bytes;
- the hash-mismatch test changed the file length, so length verification correctly failed before hash verification.

Both tests were corrected without weakening production validation in commit `35d5881b61a3489043679961c3aa3c2254ca5ff4`.

## Final permanent source validation

- Workflow run: `30490735230`
- Validated source commit: `792c95dd5c28d056480000487ef401bfe28ab1d5`
- Overall conclusion: success
- Failed steps: none
- Rust formatting: passed
- Full-workspace clippy with warnings denied: passed
- Full Rust test suite: passed
- Kotlin UniFFI generation smoke test: passed
- Swift UniFFI generation smoke test: passed
- Android Kotlin binding regeneration and drift comparison: passed
- Dependency and license policy: passed
- Android `arm64-v8a` and `x86_64` native builds: passed
- Android lint, JVM tests, and debug APK assembly: passed
- APK native-library, detector-linkage, production-symbol, and notices verification: passed
- Android emulator scanner, recovery, and FFI panic-containment tests: passed

Published artifacts for the successful run:

- `regenerated-android-ffi-files` — artifact ID `8739610990`
- `a2d-verified-debug-apk` — artifact ID `8739698618`
- `android-shared-analysis-results` — artifact ID `8739854665`
- `rustfmt-output` — artifact ID `8739584433`

## FIX-002 ownership policy

The permanent ownership policy is documented in `docs/BINDING_GENERATION.md` and summarized in `README.md` and `apps/ios/README.md`:

- the Android Kotlin binding is generated source committed to the repository;
- Android native libraries and desktop Kotlin/Swift smoke output are ephemeral;
- `bash tools/build-android-native.sh` is the authoritative Android generation command;
- `bash tools/generate-bindings.sh` is the Kotlin/Swift desktop smoke-generation command;
- an exported Rust FFI change requires regenerated Kotlin in the same commit;
- generated bindings must not be hand-edited;
- permanent CI drift detection is authoritative;
- stale-binding troubleshooting and exact artifact recovery are documented.

A repository search found no remaining documentation claiming that the committed Android Kotlin binding is uncommitted or ephemeral.
