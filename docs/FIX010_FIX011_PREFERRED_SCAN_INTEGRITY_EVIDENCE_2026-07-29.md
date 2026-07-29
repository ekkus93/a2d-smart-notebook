# FIX-010 / FIX-011 Preferred-Scan Integrity Evidence — 2026-07-29

## Status

FIX-010 and FIX-011 production implementation and focused Rust validation are complete. Explicit preferred-scan changes now use one Rust-owned typed transaction workflow, the database rejects unaudited pointer changes, first-scan registration remains transactionally consistent, and every known production mutation path has been audited.

This document records implementation and validation evidence. The source remediation TODO checkboxes remain subject to the repository-wide reconciliation pass described by FIX-120.

## Typed workflow contract

`a2d_storage::Storage::change_preferred_scan` accepts `ChangePreferredScanRequest`:

- `page_id: PageId`
- `scan_id: ScanId`
- `changed_at_ms: i64`
- `actor: String`
- `operation_id: AuditEventId`

It returns `ChangePreferredScanResult`:

- `page_id`
- `previous_preferred_scan_id`
- `preferred_scan_id`
- `changed`
- `audit_event_id`

The caller-generated `operation_id` is used as both the audit-event ID and correlation ID. The workflow therefore does not generate a panic-capable audit ID internally, and tests can deterministically inject an audit ID collision.

## Transaction behavior

Inside one immediate SQLite transaction, the workflow:

1. validates a positive authoritative timestamp and non-empty actor;
2. loads the page and candidate scan;
3. returns typed not-found errors for either missing record;
4. rejects a scan owned by another page;
5. validates that existing page-pointer and scan-flag state is internally consistent;
6. validates the candidate's original asset exists, has kind `Original`, and is immutable;
7. validates each referenced optional corrected, OCR, and thumbnail asset has its expected kind;
8. enters an exact page/scan/operation workflow context;
9. updates the page pointer and timestamp;
10. relies on schema triggers to clear the former scan flag and set the selected scan flag;
11. verifies exactly one preferred scan and pointer/flag agreement;
12. inserts `scan.preferred_changed` with actor, previous/new IDs, subject, and correlation ID;
13. removes the workflow context;
14. commits only after every postcondition and audit step succeeds.

Any validation, context, pointer update, trigger, postcondition, audit insertion, context cleanup, or commit failure rolls back the page pointer, scan flags, timestamp, audit row, and context row together.

## Idempotency policy

Selecting the already-consistent preferred scan is an explicit no-op:

- `changed` is `false`;
- the prior and selected scan IDs are returned;
- `updated_at_ms` is preserved;
- no audit event is inserted;
- no workflow context is created.

This avoids duplicate audit noise while preserving a typed result.

## Schema enforcement

Migration `0008_preferred_scan_workflow_gate.sql` adds `preferred_scan_mutation_context` and trigger `preferred_scan_pointer_update_requires_workflow`.

A real change to `pages.preferred_scan_id` is rejected with `A2D_PREFERRED_SCAN_WORKFLOW_REQUIRED` unless an exact page/scan context exists in the same transaction. Because SQL and the SQLite connection remain private to `a2d-storage`, ordinary callers cannot manufacture this context.

The migration also recreates `register_scan_updates_page` so first-scan registration:

- inserts a narrowly scoped `scan_registration` context only for a preferred inserted scan;
- updates page state, pointer, and timestamp;
- lets the existing consistency triggers synchronize flags;
- removes the context before the insertion transaction can commit.

Earlier migrations remain immutable:

- migration 0005 validates legacy state, adds the partial unique index, ownership checks, and pointer/flag guards;
- migrations 0006 and 0007 synchronize scan flags from page-pointer changes without violating those guards;
- migration 0008 adds workflow authorization without weakening the earlier invariants.

## Legacy API disposition

`Storage::set_preferred_scan` is retained only as a deprecated compatibility shim and always returns `STORAGE_PREFERRED_SCAN_WORKFLOW_REQUIRED`.

The older `PageRepository::set_preferred_scan` implementation remains available to internal compatibility/test code, but migration 0008 prevents it from performing any real pointer change without a workflow context. A same-value assignment may remain a SQLite no-op; it cannot change canonical state or create pointer/flag disagreement.

The supported production API for an explicit preference change is `Storage::change_preferred_scan(ChangePreferredScanRequest)`.

## FIX-011 mutation audit

The production mutation paths are:

1. **Explicit user preference change**
   - `Storage::change_preferred_scan`
   - typed request/result
   - actor, authoritative timestamp, and operation/correlation ID required
   - candidate asset and ownership validation
   - atomic audit insertion

2. **First-scan registration**
   - `a2d-core` scan registration transaction inserts the assets, scan, and registration audit event;
   - `register_scan_updates_page` updates the page pointer only when the inserted scan is preferred;
   - migration 0008 supplies and removes the exact trigger-owned registration context;
   - the partial unique index prevents a second preferred scan for the page.

3. **Migrations and corruption tests**
   - direct SQL touching `pages.preferred_scan_id` or `scans.preferred` is confined to immutable migrations, schema triggers, and tests that deliberately inject legacy/corrupt state;
   - those are not production mutation APIs.

No Kotlin or FFI layer implements preferred-scan business rules, and no production caller can independently mutate one side of the page-pointer/scan-flag invariant.

## Focused regression coverage

`crates/a2d-storage/tests/preferred_scan_consistency.rs` covers:

- switching preference synchronizes page pointer and both scan flags;
- typed previous/new/page result;
- timestamp update;
- complete audit contents;
- idempotent no-op with no timestamp or audit change;
- scan from another page;
- unknown page and unknown scan;
- invalid timestamp and actor;
- invalid referenced asset role;
- forced audit insertion failure rolling back page and scan changes;
- partial unique-index rejection of two preferred scans.

`crates/a2d-storage/tests/preferred_scan_workflow_gate.rs` covers:

- deprecated `Storage::set_preferred_scan` failing closed;
- explicit trait-level legacy mutation being rejected by the schema gate;
- unchanged page pointer and scan flags after both failures.

`crates/a2d-storage/tests/preferred_scan_migration.rs` constructs a version-4 legacy database containing contradictory preferred state and proves:

- migration 0005 rejects it visibly;
- migration version 5 is not recorded;
- the contradictory records are not silently rewritten or assigned a winner.

Existing scan-registration and reopen tests continue to cover first-scan insertion, later rescans, trigger synchronization, and persisted round trips.

## Implementation validation head

Implementation head before this evidence-only commit:

- commit: `24bb80048896a692a0587b98e9e6d137348abdcb`
- permanent workflow run: `30494530988`

Validated successfully on that head:

- Rust formatting drift check;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace --all-features`;
- dependency and license policy;
- Kotlin UniFFI regeneration and drift comparison;
- Android native ABI builds;
- Android lint, JVM tests, debug APK assembly, and APK verification.

The dependent Android emulator scanner/recovery/panic-containment job was still running when this evidence file was first committed. Final exact-head CI status must be recorded before declaring the documentation head fully validated.
