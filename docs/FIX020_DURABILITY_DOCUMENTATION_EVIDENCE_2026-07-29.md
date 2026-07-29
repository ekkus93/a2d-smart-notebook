# FIX-020 Durability Documentation Evidence — 2026-07-29

## Status

FIX-020 is complete. The v0.1 storage durability vocabulary, guarantees, ordering, SQLite relationship, and platform limitations are now documented consistently across the normative specification, implementation TODO, remediation TODO, Rust module documentation, and a dedicated architecture decision.

## Normative contract

The authoritative durability contract is:

- `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`

It distinguishes:

- userspace flush;
- file-content synchronization;
- metadata synchronization;
- directory-entry synchronization;
- SQLite transaction durability;
- asset filesystem commit completion;
- finalized-but-unregistered recovery state.

It explicitly prohibits treating `Write::flush()` as a persistence guarantee and prohibits describing WAL/`synchronous=NORMAL` or the combined filesystem-plus-database operation as fully power-loss durable.

## Reconciled documentation and implementation terminology

The following paths now use the same contract terminology:

- `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`, §16.3;
- `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`, §§3.3–3.4;
- `docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md`, FIX-020;
- `crates/a2d-storage/src/assets.rs` module documentation;
- `crates/a2d-storage/src/lib.rs` module documentation.

The old shorthand “flush and atomically rename” wording was removed because it did not describe the implemented file `sync_all()`, metadata handling, no-replace hard-link finalization, destination-directory synchronization, temporary-link removal, temporary-directory synchronization, or separate SQLite durability semantics.

## Platform scope

The documented v0.1 contract is limited to supported native local filesystems, including Android app-private Linux-backed storage and ordinary Linux local filesystems, where `tmp/` and `assets/` share a filesystem and hard links plus file/directory synchronization are supported.

The contract explicitly excludes silent downgrade on unsupported document providers, cloud/network filesystems, cross-filesystem finalization, and filesystems without required hard-link or directory-sync semantics. Future Apple support requires separate validation.

## Drift prevention

`crates/a2d-storage/tests/durability_documentation.rs` checks that:

- every required durability layer remains defined;
- anti-overclaim language remains present;
- the asset implementation retains the documented ordering markers;
- SQLite remains configured for WAL, `synchronous=NORMAL`, and immediate transactions.

The test is a documentation/implementation drift guard. It does not claim that software can prove storage hardware or firmware honestly completed a synchronization request.

## Implementation reconciliation commit

The authoritative files were reconciled at:

- `d835b5c79a6bcf8f7f764c917353f47bef81a957` — `Reconcile FIX-020 durability terminology`

The one-time exact-match reconciliation script and workflow removed themselves in that commit and are not permanent repository machinery.
