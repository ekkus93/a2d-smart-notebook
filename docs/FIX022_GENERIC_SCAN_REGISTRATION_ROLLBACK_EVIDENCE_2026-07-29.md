# FIX-022 Generic Scan-Registration Rollback Evidence — 2026-07-29

## Status

The focused generic scan-registration rollback evidence gap is closed. When the generic registration transaction fails after all asset files have been finalized but before SQLite commit, the workflow now returns normalized `database_registration_rolled_back` evidence for every finalized asset and a deterministic regression proves database rollback, orphan visibility, and non-destructive retry behavior.

This document records the generic scan-registration follow-up identified after the broader FIX-022 recovery work. It does not claim that every future recovery action or reviewed orphan-cleanup workflow is implemented.

## Implementation head and validation

The implementation was validated at:

- source commit `68eef232d45e2e332af4aba8ba270bf2da8e4b66` — `Normalize generic scan rollback evidence`;
- formatting commit `1bd0db37b90fff52d1b753986b2ca61dfa6ebe7c` — `Format FIX-022 rollback regression`;
- permanent-CI restoration/validation head `cd75385b5428a386812f89858dd742cf534981c1`;
- permanent CI run `30503130909`.

That run completed successfully across Rust formatting, clippy with warnings denied, the full Rust workspace test suite, dependency/license policy, Kotlin UniFFI drift, Android native builds, Gradle lint/JVM tests, APK verification, and Android emulator instrumentation.

This evidence document is committed afterward and must receive its own exact-head permanent CI validation before the focused pass is finally closed.

## Normalized rollback stage

`crates/a2d-core/src/milestone9.rs` now uses the shared Rust-owned persistence stage:

- `AssetPersistenceFailureStage::DatabaseRegistrationRolledBack`
- serialized detail value: `database_registration_rolled_back`

The original transaction error remains the primary error. The workflow augments it with rollback and recovery evidence rather than replacing or hiding its code, category, severity, message, or correlation ID.

## Deterministic transaction boundary

Production `A2dCore::register_scan` delegates to one internal registration implementation with a successful no-op pre-commit guard.

The private guard executes only after the immediate SQLite transaction has:

1. reloaded and validated the current page;
2. inserted all four asset rows;
3. inserted the scan row;
4. inserted the audit row;
5. executed registration triggers;
6. verified the resulting page state and preferred-scan pointer.

The guard runs immediately before the transaction closure returns success. A test can therefore force an error at the last deterministic pre-commit boundary and exercise normal `Storage::transaction` rollback semantics without changing production behavior.

## Structured rollback evidence

Every post-finalization registration failure now includes:

- `asset_commit_failure_stage=database_registration_rolled_back`;
- asset commit journal path;
- retained scanner staging path;
- attempted scan ID;
- attempted audit-event ID;
- aggregate committed asset IDs;
- orphaned asset count;
- final-file-created state;
- file synchronization state;
- directory synchronization state;
- database-registration-started state;
- database-registration-committed state;
- an explicit reviewed-recovery action hint.

For each finalized asset, the error also includes immutable indexed evidence:

- asset ID;
- asset kind;
- final relative path;
- expected SHA-256;
- byte length.

The four entries cover the original, corrected, OCR, and thumbnail assets. No raw captured image or note content is placed in error details.

## Rollback regression

`crates/a2d-core/src/milestone9_tests.rs` contains:

- `database_failure_rolls_back_all_scan_rows_and_reports_each_finalized_asset`

The regression forces a storage-class error immediately before transaction commit and proves all of the following:

- the injected primary error code is preserved;
- the failure stage is `database_registration_rolled_back`;
- database registration started but did not commit;
- all four asset files completed file and directory synchronization before registration;
- the staging file remains available;
- the asset commit journal remains available;
- the journal records four `asset_committed` phases;
- the journal has no `database_committed` phase;
- no asset row exists for any finalized asset;
- no scan row exists;
- no scan-registration audit row exists;
- the page remains `Unscanned`;
- the page has no preferred scan;
- every finalized file still exists;
- non-destructive orphan discovery reports exactly those four files;
- discovered kinds, relative paths, hashes, and byte lengths match the error evidence.

## Retry behavior

The same regression then retries generic registration normally and proves:

- retry succeeds;
- retry generates fresh asset identities;
- no prior orphan is overwritten or adopted silently;
- all four prior orphan files remain discoverable and byte-for-byte represented by the same hash/length evidence;
- the prior journal and staging file remain available for reviewed recovery.

This satisfies the no-silent-cleanup rule: retry does not delete, replace, conceal, or implicitly import unknown finalized assets.

## Existing recovery API alignment

The regression uses the existing non-destructive:

- `Storage::discover_orphaned_final_assets`

That API compares final asset files against database rows and reports unreferenced files without deleting or mutating them. The generic registration workflow now emits enough immutable evidence to correlate its rollback with those discovery results.

## Temporary implementation machinery

Several one-time workflow and patch files were used while applying and formatting the exact source change through the available GitHub contents interface. All temporary files were removed before permanent validation, and `.github/workflows/ci.yml` was restored to its prior permanent read-only definition.

No temporary patch runner, trigger, script, or write-enabled CI job remains in the repository tree.

## Acceptance result

The focused generic scan-registration rollback gap is satisfied:

- finalized assets are not falsely reported as saved database records;
- SQLite rollback is proven after all intended rows were inserted inside the transaction;
- each finalized-but-unregistered asset has complete immutable recovery evidence;
- orphan reporting is non-destructive;
- retry cannot overwrite or silently consume prior orphan files.

The consolidated remediation TODO checkboxes were not rewritten in this focused commit. This evidence file is the authoritative completion record for this follow-up pass; broad roadmap checkbox reconciliation remains separate maintenance work.
