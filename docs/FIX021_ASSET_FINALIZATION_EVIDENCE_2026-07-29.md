# FIX-021 Asset Finalization Evidence — 2026-07-29

## Status

FIX-021 is implementation-complete. Asset finalization is no-replace, power-loss aware under the accepted v0.1 Android/Linux durability contract, explicit about unsupported platforms, and covered by deterministic failure-injection tests.

## Implementation head and validation

The implementation was validated at:

- commit `ce145d17bf56175592d787b04555bfc963521311`;
- permanent CI run `30499619013`;
- all Rust, dependency, Kotlin binding-drift, Android build/APK, and Android emulator jobs completed successfully.

This evidence document is committed afterward and must receive its own exact-head permanent CI validation before the work is considered finally closed.

## No-replace finalization

`crates/a2d-storage/src/asset_platform.rs` owns the platform-specific primitives.

For validated Android and Linux local filesystems:

- the synchronized temporary inode is finalized with `std::fs::hard_link`;
- hard-link creation is atomic and fails if the destination already exists;
- no code falls back to `std::fs::rename`;
- the destination asset-kind directory and `tmp/` are opened and synchronized with `sync_all()`.

Other targets, including Apple targets, return `io::ErrorKind::Unsupported`. Future Apple support requires separate validation of no-replace semantics, directory synchronization, data-protection interaction, and hardware-flush behavior.

## Asset commit ordering

`AssetStore::commit` returns an `Asset` only after this ordering succeeds:

1. Generate the asset identity, timestamp, expected length, and SHA-256.
2. Create the temporary path with create-new semantics.
3. Write all bytes.
4. Call `Write::flush()` without treating it as durable.
5. For immutable originals, set read-only permissions on the open file handle.
6. Call `File::sync_all()` so contents and required metadata are included in the synchronization request.
7. Close the file.
8. Re-read and verify byte length and SHA-256.
9. Create the final path atomically without replacement through the platform adapter.
10. Verify final metadata, file type, length, and read-only state for originals.
11. Synchronize the destination directory.
12. Remove the temporary hard link.
13. Synchronize the temporary directory.
14. Return the typed `Asset` value for later SQLite registration.

A caller cannot obtain an `Asset` value before every required successful finalization step completes.

## Typed errors and structured evidence

The implementation distinguishes these relevant failures:

- `STORAGE_ASSET_FINAL_PATH_COLLISION`: critical integrity event; an existing final path was preserved and not replaced.
- `STORAGE_ASSET_FINALIZATION_UNSUPPORTED`: the platform or filesystem cannot provide required same-filesystem no-replace semantics.
- `STORAGE_ASSET_FINALIZATION_FAILED`: finalization failed for another I/O reason.
- `STORAGE_ASSET_FILE_SYNC_FAILED`: the temporary file did not complete `sync_all()`.
- `STORAGE_ASSET_PERMISSION_SET_FAILED`: immutable-original permissions could not be applied.
- `STORAGE_ASSET_DESTINATION_DIRECTORY_SYNC_FAILED`: the final path exists but destination-directory synchronization did not complete.
- `STORAGE_ASSET_TEMP_DIRECTORY_SYNC_FAILED`: the final path exists and the temporary link was removed, but synchronization of that removal did not complete.
- `STORAGE_DIRECTORY_SYNC_UNSUPPORTED`: the platform cannot provide the required directory synchronization operation.

Errors retain the asset ID, kind, final relative path, expected SHA-256, byte length, failure stage, file-sync state, directory-sync state, and relevant absolute temporary/final paths.

## Collision safety

A deterministic collision regression proves that:

- a pre-existing destination produces `STORAGE_ASSET_FINAL_PATH_COLLISION`;
- the error category is `Integrity` and severity is `Critical`;
- the error includes the asset ID and final path;
- the original destination bytes remain unchanged;
- the failed attempt does not replace, truncate, or rewrite the existing asset;
- the failed attempt's temporary file is cleaned when cleanup succeeds.

Therefore an `AssetId` collision cannot overwrite an existing asset.

## Failure-injection coverage

`crates/a2d-storage/tests/asset_finalization.rs` uses test-only entry points that route through the production commit implementation while injecting one named failure stage.

Coverage includes:

- forced file `sync_all()` failure before finalization, with no final path and no database row;
- forced destination-directory synchronization failure, reported as `finalized_unregistered` while preserving the final file and temporary link;
- forced temporary-directory synchronization failure after unlink, preserving the final file and reporting incomplete synchronization of cleanup metadata;
- forced immutable-permission failure with asset ID and planned final path;
- successful original commit with verified bytes, length, SHA-256, read-only permissions, and no temporary orphan;
- source-level enforcement that the platform adapter supports only Android/Linux, uses hard links, returns Unsupported elsewhere, and contains no rename fallback.

The test-only APIs are compiled only with the `test-util` feature. Production callers cannot select an asset ID or inject failures.

## Durability contract alignment

`crates/a2d-storage/tests/durability_documentation.rs` now checks both `assets.rs` and `asset_platform.rs`. It guards the documented ordering, explicit Android/Linux platform scope, hard-link no-replace primitive, unsupported behavior, absence of `rename`, and the existing WAL/`synchronous=NORMAL` SQLite contract.

The normative durability decision remains `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.

## Acceptance result

FIX-021 acceptance criteria are satisfied in implementation and tests:

- a cryptographic identity collision cannot overwrite an existing asset;
- successful asset commit meets the documented v0.1 asset filesystem durability contract.

The remediation TODO checkboxes are not rewritten in this commit because the large consolidated TODO requires whole-file replacement through the available GitHub contents interface. This evidence file is the authoritative completion record for this focused pass; consolidated checkbox reconciliation remains a later roadmap-maintenance task.
