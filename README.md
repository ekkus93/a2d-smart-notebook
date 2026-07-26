# A2D Smart Notebook

A smart notebook for people who still like paper.

A2D turns handwritten pages — printed A2D Smart Pages or a bound A2D Smart Notebook — into a durable,
searchable digital library. Each writable page carries four printed Corner Markers and a Page Code so
an Android phone can locate, rectify, and file a scan automatically.

The system is **local-first and accountless**: the library lives on-device, and encrypted `.atnb`
backup/restore requires no account. An optional paid A2D Sync service is deferred beyond v0.1.

A shared **Rust core is authoritative** for all canonical data, persistence, and domain rules. The
Android application (Kotlin + Jetpack Compose) calls typed Rust use cases over a thin UniFFI boundary
and never duplicates domain logic; a future iOS application will do the same. See
`docs/A2D_SMART_NOTEBOOK_V01_SPEC.md` for the full specification and `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`
for the milestone breakdown.

## Layout

```text
crates/       Rust workspace: domain, storage, image, pdf, search, ocr, skills, backup, ffi, ...
apps/android/ Kotlin + Jetpack Compose Android application
apps/ios/     Future iOS application (UI deferred; Swift binding generation is required from day one)
fixtures/     Permanent compatibility fixtures (QR vectors, page layouts, scans, backups)
docs/         Specification and implementation TODO
```

## Building

```sh
cargo build --workspace
```

Android and CI setup follow in later milestones.
