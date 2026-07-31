# FIX-024 Canonical Asset Path Evidence — 2026-07-31

## Status

The FIX-024 canonical asset-path behavior, focused regressions, and v0.1 limitation contract are implemented on `master`.

This document does not claim exact-head permanent CI success. The final evidence commit must receive the repository’s full permanent-CI validation before FIX-024 is treated as fully signed off.

## Production behavior

`AssetStore::resolve` in `crates/a2d-storage/src/assets.rs`:

- joins a database-stored relative path to the configured library root;
- reads final-component metadata without following a symbolic link;
- reports a missing referenced asset with `STORAGE_ASSET_MISSING`;
- rejects symbolic links with `STORAGE_ASSET_PATH_IS_SYMLINK`;
- rejects directories and other non-regular entries with `STORAGE_ASSET_PATH_NOT_FILE`;
- canonicalizes both the library root and candidate;
- rejects root escape with `STORAGE_ASSET_PATH_ESCAPES_ROOT`;
- returns the canonical candidate path that passed validation.

The implementation does not return the original unvalidated joined path.

## Regression coverage

Existing `crates/a2d-storage/tests/asset_commit_hardening.rs` coverage already proves:

- a valid asset resolves to its canonical path;
- a missing asset returns the dedicated missing-asset code;
- a symlink inside the library is rejected;
- a symlink inside the library pointing outside is rejected.

`crates/a2d-storage/tests/asset_path_resolution.rs` adds the remaining focused cases:

- traversal to an existing regular file outside the library root is rejected with `STORAGE_ASSET_PATH_ESCAPES_ROOT`;
- the outside file remains unchanged;
- a directory where an asset file is required is rejected with `STORAGE_ASSET_PATH_NOT_FILE`.

`crates/a2d-storage/tests/asset_path_resolution_documentation.rs` guards source/documentation alignment for:

- no-follow final-component metadata inspection;
- symlink and non-file rejection;
- canonical candidate construction;
- canonical-root containment;
- returning the validated canonical path;
- the documented bounded TOCTOU limitation and future validated-handle direction.

## TOCTOU limitation

`docs/decisions/V01_ASSET_PATH_RESOLUTION_CONTRACT.md` is the normative v0.1 contract.

The current implementation validates a canonical path and callers subsequently open that path. This minimizes but does not eliminate a time-of-check/time-of-use race against an actor that can concurrently rewrite the application-private library tree.

The contract therefore requires callers to resolve immediately before I/O, avoid suspension or user callbacks between validation and open, keep absolute paths out of canonical domain state, and verify immutable length/hash evidence where integrity matters.

The documented future hardening direction is an internal validated-handle API using directory-relative no-follow semantics on Android/Linux and an equivalent Apple platform adapter. Platform-specific handles must not become portable domain identifiers.

## Commits

- `7096c4401982f5827ddfba07bf60af4ab0f5db38` — `Add FIX-024 path traversal regressions`
- `e2427cd445781a17e03d6c8cec6ce80ade7f61c3` — `Harden FIX-024 traversal test ownership`
- `fbbe13925416469f890b58a302400c0de7121722` — `Document FIX-024 path resolution contract`
- `97a79f49795ebb5f0b35647853fa3d28b84824c8` — `Guard FIX-024 path contract drift`
- `357cb9234beccc490cd74a9a05f182939253f417` — `Fix FIX-024 source drift assertion`

## Acceptance result

The focused FIX-024 acceptance conditions are represented in production code, tests, and documentation:

- traversal and root escape remain explicit failures;
- symlinks and non-files are rejected;
- valid assets return the canonical validated path;
- missing assets retain their dedicated error;
- the residual path-based reopen limitation is explicit rather than silently treated as fully race-free.

Exact-head permanent-CI validation remains required after this evidence document is committed.
