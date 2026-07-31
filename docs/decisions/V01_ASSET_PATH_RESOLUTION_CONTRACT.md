# v0.1 Asset Path Resolution Contract

**Status:** Normative for v0.1  
**Applies to:** Rust-owned library asset paths on Android/Linux  
**Primary implementation:** `crates/a2d-storage/src/assets.rs`

## 1. Stored path model

Database asset rows store library-relative paths. Portable domain APIs do not accept Android URIs, absolute platform paths, provider handles, or caller-selected roots as canonical asset identity.

`AssetStore::resolve` joins the stored relative path to the configured library root and returns the canonical absolute candidate that it validated. It never returns the unvalidated joined path.

## 2. Required validation

Before returning a path, `AssetStore::resolve` must:

1. read metadata without following the final path component as a symbolic link;
2. return `STORAGE_ASSET_MISSING` when the referenced entry does not exist;
3. reject symbolic links with `STORAGE_ASSET_PATH_IS_SYMLINK`;
4. reject directories and other non-regular entries with `STORAGE_ASSET_PATH_NOT_FILE`;
5. canonicalize the library root and candidate;
6. reject any candidate outside the canonical library root with `STORAGE_ASSET_PATH_ESCAPES_ROOT`;
7. return the canonical candidate path that passed those checks.

Traversal, symlink, missing-file, and wrong-file-type failures are integrity failures. Callers must not convert them to an empty result, fallback asset, or success state.

## 3. Path-based reopen and bounded TOCTOU limitation

The v0.1 implementation returns a validated canonical path and some callers subsequently open that path. Validation and open are therefore separate filesystem operations. A hostile process with sufficient access to mutate the application-private library tree could attempt path substitution during that interval.

The v0.1 implementation minimizes, but does not claim to eliminate, that time-of-check/time-of-use window:

- the library root is application-owned storage rather than a caller-selected shared directory;
- stored paths are relative and must remain below the canonical library root;
- the final component must be a regular non-symlink file at validation time;
- callers perform filesystem I/O immediately after resolution without user interaction, asynchronous suspension, or platform picker round-trips;
- verification rechecks recorded byte length and SHA-256 after opening and reading the file;
- immutable originals are committed read-only;
- failures remain explicit and do not trigger fallback path selection.

This model protects against malformed database paths, ordinary traversal, accidental symlinks, missing assets, and non-file entries. It is not a security boundary against a malicious same-UID process or another actor that can rewrite the application-private library tree concurrently.

## 4. Caller rules

A caller using a resolved path must:

- call `resolve` immediately before the filesystem operation;
- avoid storing the returned absolute path as canonical domain state;
- avoid suspending, invoking user code, or crossing a platform callback before opening;
- treat open/read failure as an explicit asset failure;
- verify immutable evidence such as recorded length and SHA-256 where the operation depends on asset integrity;
- never retry by opening the original unvalidated relative path directly.

## 5. Future handle-based hardening

A future storage hardening pass should replace path-based reopening with an internal validated-handle API. On Android/Linux, the preferred direction is directory-relative opening with no-follow semantics and post-open metadata verification, using facilities such as `openat2`/`openat` plus appropriate resolve flags where supported. A future Apple implementation must provide equivalent no-follow, root-constrained semantics through its platform adapter.

The portable public API must remain platform-neutral. Linux file descriptors, Android framework types, Apple URLs, and provider-specific handles must not become domain identifiers.

Until that handle-based implementation is available, this document is the required limitation statement. Code or documentation must not claim that `AssetStore::resolve` alone closes every hostile concurrent path-substitution race.
