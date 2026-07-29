#!/usr/bin/env python3
"""Apply the FIX-020 durability terminology reconciliation exactly once.

The script uses exact replacements and aborts if the expected source text is absent or duplicated.
It is executed by a self-removing one-time workflow and is not retained after the resulting commit.
"""

from pathlib import Path


def replace_exact(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected exactly one replacement target, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_exact(
    "docs/A2D_SMART_NOTEBOOK_V01_SPEC.md",
    """### 16.3 Asset commit protocol

A scan registration that writes files and database rows MUST use a recoverable protocol:

1. Write asset to a temporary file.
2. Flush and close.
3. Compute and verify hash.
4. Atomically rename into the asset repository.
5. Commit database references in one transaction.
6. Record incomplete/orphan cleanup work if interrupted.

The system MUST NOT commit a database row pointing to an asset that was never durably written.
""",
    """### 16.3 Asset commit and durability contract

The normative v0.1 durability contract is `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`. Documentation, diagnostics, and tests MUST distinguish these layers:

- **Userspace flush:** `Write::flush()` passes writer-buffered bytes onward; it is not a persistence guarantee.
- **File-content and metadata synchronization:** `File::sync_all()` requests synchronization of file bytes and required metadata.
- **Directory-entry synchronization:** the affected directory is opened and synchronized after creation or removal of a filename.
- **SQLite transaction durability:** guarantees are determined separately by the selected journal and synchronous modes.

A scan registration that writes files and database rows MUST use this recoverable ordering:

1. Create-new and write the temporary asset file.
2. Call `flush()` without claiming durability.
3. Apply required metadata, including read-only permissions for originals.
4. Call `sync_all()` on the temporary file, close it, then re-read and verify byte length and SHA-256.
5. Create the final path atomically without replacement on the same filesystem.
6. Verify finalized metadata and synchronize the destination directory.
7. Remove the temporary directory entry and synchronize the temporary directory.
8. Only after the asset filesystem commit succeeds, insert asset and dependent rows in one SQLite transaction.
9. Report interrupted temporary or finalized-unregistered assets non-destructively.

SQLite v0.1 uses WAL mode with `synchronous=NORMAL`. This preserves database consistency and survives application-process crashes, but the latest committed transaction may be lost after an operating-system crash or power loss. Therefore A2D MUST NOT claim that a successful SQLite commit, a successful `flush()`, or the combined scan registration is fully power-loss durable.

The system MUST NOT commit a database row pointing to an asset whose required filesystem synchronization and directory-entry synchronization did not complete. The selected filesystem-first ordering may leave a synchronized unreferenced asset after interruption; recovery MUST report and preserve that file rather than silently deleting it.
""",
)

replace_exact(
    "docs/A2D_SMART_NOTEBOOK_V01_TODO.md",
    """- [x] Flush/close, compute SHA-256, then atomically rename. (Also re-reads the temp file after
      flush and re-hashes it to *verify* against the in-memory hash, not just compute once and
      trust it.)
""",
    """- [x] Distinguish userspace flush from persistence. `Write::flush()` is required but is never
      described as durable by itself.
- [x] Apply original read-only metadata, call `File::sync_all()`, close, then re-read and verify
      byte length and SHA-256 against the supplied bytes.
- [x] Finalize without replacement using a same-filesystem hard link, verify finalized metadata,
      synchronize the destination directory, remove the temporary link, and synchronize `tmp/`.
      The complete terminology and platform scope are normative in
      `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.
""",
)

replace_exact(
    "docs/A2D_SMART_NOTEBOOK_V01_TODO.md",
    """- [x] Commit references only after durable file creation. (`commit` only returns an `Asset` value
      after the atomic rename succeeds; there is no code path that could construct one, and
      therefore no path that could insert its DB row, before the rename happens.)
""",
    """- [x] Commit references only after the asset filesystem commit. `commit` returns an `Asset`
      only after file `sync_all()`, no-replace finalization, finalized-metadata verification,
      destination-directory synchronization, temporary-link removal, and temporary-directory
      synchronization all succeed. SQLite registration occurs afterward and has its own documented
      WAL/`synchronous=NORMAL` durability semantics.
""",
)

replace_exact(
    "docs/A2D_SMART_NOTEBOOK_V01_TODO.md",
    """- [x] A committed scan can never reference an original file that was never durably written.
      (`asset_row_is_only_insertable_after_the_file_is_durably_renamed_into_place` — proven
      structurally, not just asserted, per that test's own comment.)
""",
    """- [x] A committed scan can never reference an original file that did not complete the v0.1
      asset filesystem commit contract. The guarantee is stated separately from SQLite
      power-loss durability; WAL/`synchronous=NORMAL` may lose the latest transaction after an OS
      crash while preserving database consistency.
""",
)

replace_exact(
    "crates/a2d-storage/src/assets.rs",
    """//! The asset commit protocol (TODO 3.3, spec §16.3): create-new temp write → flush and sync →
//! compute and verify SHA-256 → atomic no-replace finalization → sync directories → caller commits
//! the DB row. This module owns only the filesystem half; the DB half is
//! `AssetRepository::insert_asset` (repository.rs).
""",
    """//! The asset filesystem commit protocol (TODO 3.3, spec §16.3): create-new temp write →
//! userspace flush → file-content and metadata `sync_all` → close and verify → atomic no-replace
//! finalization → destination-directory sync → temp-link removal → temp-directory sync. A
//! successful `flush()` alone is never treated as durable. Only after this function returns an
//! `Asset` may the caller attempt the separate SQLite transaction.
//!
//! Normative terminology, WAL/`synchronous=NORMAL` interaction, supported filesystem scope, and
//! platform limitations are defined in `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.
""",
)

replace_exact(
    "crates/a2d-storage/src/lib.rs",
    """//! Owns the canonical SQLite database. All SQL is private to this crate; no other crate issues SQL.
//!
//! `rusqlite` uses bundled SQLite for reproducible Android cross-compilation. Connections enable
//! foreign keys, WAL mode, synchronous NORMAL, and a bounded busy timeout. Numbered migrations are
//! applied transactionally and their exact SQL SHA-256 digests are recorded and revalidated on
//! every open; edited history, version gaps, and databases newer than the app fail closed.
""",
    """//! Owns the canonical SQLite database. All SQL is private to this crate; no other crate issues SQL.
//!
//! `rusqlite` uses bundled SQLite for reproducible Android cross-compilation. Connections enable
//! foreign keys, WAL mode, `synchronous=NORMAL`, and a bounded busy timeout. WAL/NORMAL preserves
//! database consistency and application-crash durability, but the latest committed transaction may
//! be rolled back after an operating-system crash or power loss; this crate does not call such a
//! commit fully power-loss durable. Asset files complete their separate file and directory
//! synchronization contract before database registration is attempted. See
//! `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`.
//!
//! Numbered migrations are applied transactionally and their exact SQL SHA-256 digests are recorded
//! and revalidated on every open; edited history, version gaps, and databases newer than the app
//! fail closed.
""",
)

replace_exact(
    "docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md",
    """## FIX-020 — Define and document the actual durability contract

**Priority:** P0  
**Primary paths:**

- `crates/a2d-storage/src/assets.rs`
- `crates/a2d-storage/src/lib.rs`
- `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

### Tasks

- [ ] Define what “durable” means for v0.1 on Android/Linux filesystems.
- [ ] Distinguish:
  - [ ] userspace flush
  - [ ] file-content synchronization
  - [ ] metadata synchronization
  - [ ] directory-entry synchronization
  - [ ] SQLite transaction durability
- [ ] Document the selected relationship between SQLite `synchronous` mode and asset-file synchronization.
- [ ] Do not claim a file is power-loss durable if only `Write::flush()` has succeeded.
- [ ] Record any platform limitations explicitly.

### Acceptance criteria

- [ ] Documentation and implementation use the same durability terminology.
""",
    """## FIX-020 — Define and document the actual durability contract

**Priority:** P0  
**Status:** Complete; normative contract in `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`  
**Primary paths:**

- `crates/a2d-storage/src/assets.rs`
- `crates/a2d-storage/src/lib.rs`
- `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`
- `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`
- `crates/a2d-storage/tests/durability_documentation.rs`

### Tasks

- [x] Define what “durable” means for v0.1 on Android/Linux filesystems.
- [x] Distinguish:
  - [x] userspace flush
  - [x] file-content synchronization
  - [x] metadata synchronization
  - [x] directory-entry synchronization
  - [x] SQLite transaction durability
- [x] Document the selected relationship between SQLite `synchronous` mode and asset-file synchronization.
- [x] Do not claim a file is power-loss durable if only `Write::flush()` has succeeded.
- [x] Record Android/Linux, non-Unix, provider-filesystem, cross-filesystem, future Apple, and hardware limitations explicitly.
- [x] Add a source-level drift test tying the terminology to the filesystem ordering and SQLite pragmas.

### Acceptance criteria

- [x] Documentation and implementation use the same durability terminology.
""",
)
