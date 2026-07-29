# A2D Smart Notebook v0.1 Storage Durability Contract

**Status:** Accepted for v0.1  
**Date:** 2026-07-29  
**Scope:** Android and Linux local-library storage  
**Implementation:** `crates/a2d-storage/src/assets.rs`, `crates/a2d-storage/src/lib.rs`

## Decision

A2D v0.1 uses a **filesystem-first, recoverable persistence protocol**:

1. Asset file contents and required file metadata are synchronized.
2. The final asset directory entry is created without replacement and synchronized.
3. The temporary directory entry removal is synchronized.
4. Only after the filesystem commit succeeds may a caller insert the asset row and related scan rows in SQLite.
5. SQLite uses WAL journal mode with `PRAGMA synchronous=NORMAL`.

This intentionally gives asset files stronger power-loss synchronization than the latest SQLite transaction. The tradeoff is safe and recoverable: after an operating-system crash or power loss, a synchronized asset file may exist without its database row, but A2D must not create a database row for a file whose filesystem commit did not complete.

A2D therefore does **not** describe the combined file-plus-database operation as fully power-loss durable. It describes the two layers separately and reports any incomplete state for recovery.

## Normative terminology

The following terms are distinct and must not be used interchangeably.

### Userspace flush

A **userspace flush** is successful completion of `std::io::Write::flush()`.

It requests that buffered bytes held by the writer implementation be passed onward. It does not establish that the kernel, filesystem, storage controller, or physical medium has made the bytes persistent. A successful `flush()` is never sufficient by itself for A2D to claim file-content synchronization or power-loss durability.

### File-content synchronization

**File-content synchronization** means successful completion of `std::fs::File::sync_all()` on the temporary asset file after all bytes have been written.

A2D uses `sync_all()`, rather than only `sync_data()`, because original-asset read-only permissions are applied before synchronization and the v0.1 contract requires relevant file metadata to be included in the synchronization request.

### Metadata synchronization

**Metadata synchronization** means the file synchronization request covers filesystem metadata required by the committed representation, including file length and, for immutable originals, read-only permissions.

A2D verifies finalized metadata after creating the final directory entry. Metadata verification is an integrity check; it does not replace `sync_all()`.

### Directory-entry synchronization

**Directory-entry synchronization** means opening the affected directory and successfully calling `sync_all()` on that directory.

A2D synchronizes:

- the destination asset-kind directory after creating the final hard link; and
- `tmp/` after removing the temporary hard link.

Synchronizing only the file does not synchronize creation or removal of its directory names. Both directory operations are required before `AssetStore::commit` returns success.

### SQLite transaction durability

**SQLite transaction durability** describes whether a committed database transaction is guaranteed to survive an application crash, operating-system crash, or power loss under the selected SQLite configuration.

A2D v0.1 configures:

- `journal_mode=WAL`;
- `synchronous=NORMAL`;
- explicit immediate transactions for multi-row workflows.

Under WAL plus `synchronous=NORMAL`, SQLite maintains database consistency and transactions survive an application-process crash, but the latest committed transactions may be rolled back after an operating-system crash or power loss. A2D must not call such transactions fully power-loss durable.

## Asset filesystem commit protocol

`AssetStore::commit` may return an `Asset` only after all required steps succeed:

1. Generate the `AssetId`, timestamp, expected byte length, and expected SHA-256.
2. Create `tmp/<AssetId>.tmp` with create-new semantics.
3. Write all bytes.
4. Call `Write::flush()`.
5. For an original asset, set read-only permissions while the file is still open.
6. Call `File::sync_all()` to request synchronization of file contents and relevant metadata.
7. Close the temporary file handle.
8. Re-read the synchronized temporary file and verify byte length and SHA-256.
9. Create the final path with `hard_link`, which is same-filesystem, atomic, and no-replace: an existing destination causes failure instead of replacement.
10. Verify the final path is a regular non-symlink file with the expected length and, for originals, read-only permissions.
11. Synchronize the destination asset-kind directory.
12. Remove the temporary hard link.
13. Synchronize `tmp/`.
14. Return the typed `Asset` value.

Failure before step 9 leaves no final asset path. Failure from step 9 onward is a **finalized-unregistered** condition and must include recovery metadata. The final file must not be deleted automatically.

## Relationship between asset files and SQLite

Filesystem and SQLite commits cannot be one atomic transaction. A2D uses this ordering:

```text
synchronize asset file
    -> create and synchronize final directory entry
    -> remove and synchronize temporary entry
    -> return Asset
    -> begin/continue SQLite transaction
    -> insert asset and dependent rows
    -> commit SQLite transaction
```

The ordering is deliberate:

- A database row must never be committed before the referenced asset completes the filesystem contract.
- If the process stops after finalization but before database registration, the result is an unreferenced final asset, not a dangling database reference.
- If a power loss preserves the asset but loses a recent SQLite `synchronous=NORMAL` transaction, recovery may also find an unreferenced final asset.
- Orphan discovery is non-destructive. Unknown final assets are reported and preserved.

A successful SQLite commit means the transaction completed under WAL plus `synchronous=NORMAL`; it does not strengthen or weaken the already-completed asset filesystem synchronization.

## Required status language

Implementation, diagnostics, tests, and documentation should use these phrases:

- **userspace flush completed**
- **file synchronization completed**
- **destination directory synchronization completed**
- **temporary directory synchronization completed**
- **asset filesystem commit completed**
- **SQLite transaction committed under WAL/NORMAL**
- **finalized but unregistered asset**

They must avoid unsupported statements such as:

- “`flush()` made the file durable”;
- “the rename made the asset power-loss durable”;
- “SQLite commit is power-loss durable” when using WAL/NORMAL;
- “the entire scan is power-loss durable” without separately proving both filesystem and database guarantees.

The existing structured fields retain these meanings:

- `file_sync_completed=true`: the temporary asset file completed `sync_all()`;
- `directory_sync_completed=true`: the destination asset-kind directory completed synchronization;
- `asset_commit_failure_stage=before_finalization`: no final asset path was created by this attempt;
- `asset_commit_failure_stage=finalized_unregistered`: the final path exists or may exist, but database registration has not completed;
- `asset_commit_failure_stage=database_registration_rolled_back`: filesystem finalization completed and the registration transaction failed or rolled back.

## Supported platform scope

The v0.1 contract applies only when the library root is on a native local filesystem that supports all required operations:

- Android app-private storage backed by the device's Linux filesystem;
- ordinary Linux local filesystems;
- the `tmp/` and `assets/` directories on the same filesystem;
- regular files, hard links, file `sync_all()`, and directory `sync_all()`.

The following are not covered by the v0.1 guarantee:

- Android Storage Access Framework document-provider paths;
- cloud-backed or network filesystems;
- filesystems that reject hard links or directory synchronization;
- cross-filesystem finalization;
- removable/shared-storage providers with weaker or undocumented semantics.

Unsupported required semantics must return an explicit error. The implementation must not silently substitute `rename`, omit directory synchronization, or downgrade to flush-only behavior.

The current non-Unix implementation returns `STORAGE_DIRECTORY_SYNC_UNSUPPORTED`. A future Apple implementation must validate its own no-replace finalization, directory synchronization, data-protection, and hardware-flush behavior before claiming this contract. Being a Unix target alone is not sufficient validation.

## Limits of the guarantee

Successful synchronization calls are the strongest portable requests A2D v0.1 makes to the operating system. They cannot protect against every failure, including:

- storage hardware or firmware that falsely reports completion;
- catastrophic media failure;
- filesystem or kernel defects;
- physical destruction or loss of the device;
- corruption outside A2D's process;
- an unavailable or dishonest underlying storage provider.

For these reasons, this contract is a crash-consistency and recovery contract, not a guarantee against all data loss. Manual verified backup remains required for disaster recovery.

## Recovery consequences

After interruption, A2D may encounter:

- a temporary file with no final path;
- a synchronized final asset with no database row;
- a database transaction that rolled back while finalized files remain;
- a complete file and database registration.

Recovery must classify these states, expose structured evidence, and preserve unknown files. It must never silently delete an orphan, invent a missing database row without verification, or declare success because cleanup failed quietly.

## Verification and change control

Any implementation change that affects one of the following requires this decision to be reviewed:

- `flush`, `sync_all`, permission, hard-link, unlink, or directory-sync ordering;
- supported filesystem locations;
- SQLite journal or synchronous mode;
- transaction ordering between asset finalization and database registration;
- recovery-stage definitions;
- a future iOS/Apple storage adapter.

A test in `crates/a2d-storage/tests/durability_documentation.rs` checks that the implementation and this contract retain the required terminology and configuration markers.

## Primary references

- Rust `File::sync_all`: https://doc.rust-lang.org/stable/std/fs/struct.File.html#method.sync_all
- SQLite `PRAGMA synchronous`: https://sqlite.org/pragma.html#pragma_synchronous
- Linux `fsync(2)`, including directory-entry synchronization: https://man7.org/linux/man-pages/man2/fsync.2.html
- POSIX `link()`, atomic hard-link creation and `EEXIST`: https://man7.org/linux/man-pages/man3/link.3p.html
