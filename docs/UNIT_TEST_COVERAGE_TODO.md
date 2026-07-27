# Unit Test Coverage TODO

**Status:** Complete — implementation is committed only after the full Rust quality gate passes
**Date:** 2026-07-27
**Source:** Targeted coverage review of the Rust workspace, supplementing `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

This document records the concrete gaps found by the review and the implementation decisions used to close them. The review was intentionally risk-based rather than percentage-driven: the priority is dangerous loops, integrity transitions, migration upgrades, transaction rollback behavior, and FFI boundary behavior.

Completion gate used for every task:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

No task is considered complete merely because a new test exists. Where behavior was unsafe, the production behavior was changed and the test verifies the corrected invariant.

---

## Task 1 — Fix and test unsafe ruling spacing

### Decision

Use defense in depth:

1. `ContentStyle::validate()` rejects spacing that is non-finite or not strictly positive.
2. `PageLayout::validate()` invokes `ContentStyle::validate()` so malformed layouts fail early.
3. `a2d-pdf` validates again because a public `PageLayout` can be constructed or mutated without calling `PageLayout::validate()`.
4. Rendering uses integer-bounded iteration rather than floating-point `while` loops.
5. A defensive ruling-element ceiling rejects pathologically tiny positive spacing before allocating an excessive operation vector.

The ceiling is an input-hardening limit, not a product/UX threshold. It exists to make malformed or hostile layout values terminate with a typed error instead of hanging or exhausting memory.

### Implementation

- [x] Add `ContentStyle::validate()` in `crates/a2d-layout/src/page_layout.rs`.
- [x] Reject `0.0`, negative values, `NaN`, positive infinity, and negative infinity with `LAYOUT_CONTENT_STYLE_SPACING_INVALID`.
- [x] Call style validation from `PageLayout::validate()`.
- [x] Add acceptance tests for `Blank` and normal positive spacing.
- [x] Change `content_style_ops` to return `Result<Vec<Op>, A2dError>`.
- [x] Replace non-progressing `while` loops with precomputed, integer-bounded iteration.
- [x] Add `PDF_CONTENT_STYLE_ELEMENT_LIMIT_EXCEEDED` for excessively dense ruling.
- [x] Validate content ruling before allocating QR/marker operations.
- [x] Add renderer tests that bypass `PageLayout::validate()` and still receive typed errors.
- [x] Add a tiny-positive-spacing test that verifies bounded failure instead of memory growth.
- [x] Update the main implementation TODO’s Milestone 5.1/5.4 evidence.

### Acceptance

- [x] Invalid spacing cannot pass layout validation.
- [x] Bypassing layout validation cannot make PDF ruling enter an unbounded loop.
- [x] Tiny positive spacing cannot request an unbounded number of PDF operations.
- [x] Valid blank, lined, graph, and dot-grid layouts still render through the same public API.

The pre-fix hanging behavior is documented from code inspection. It is not retained as a permanent test because deliberately executing an unbounded loop is unsafe; the regression tests assert the new bounded error path directly.

---

## Task 2 — Test and harden generated-PDF asset assignment

### Decision

Do not preserve the original TODO’s proposed “second call silently overwrites the first asset” behavior. That would erase provenance and can orphan the previously associated PDF.

Generated-PDF assignment is now single-writer and idempotent:

- Unassigned page + asset → assign and update `updated_at_ms`.
- Same asset assigned again → idempotent success, no timestamp mutation.
- Different asset assigned later → typed integrity conflict, original association preserved.

The Rust-owned repository layer enforces the same rule so callers cannot bypass the domain invariant by issuing the typed repository operation directly.

### Implementation

- [x] Change `Page::set_generated_pdf_asset` to return `Result<(), A2dError>`.
- [x] Add `PAGE_GENERATED_PDF_ASSET_CONFLICT`.
- [x] Test initial assignment and timestamp update directly in `a2d-domain`.
- [x] Test idempotent reassignment of the same `AssetId`.
- [x] Test rejection of a different `AssetId` and preservation of prior state.
- [x] Propagate the domain result from `A2dCore::generate_and_register_page_set`.
- [x] Harden `PageRepository::set_generated_pdf_asset` against implicit replacement.
- [x] Add `STORAGE_GENERATED_PDF_ASSET_CONFLICT` and repository integration coverage.

### Acceptance

- [x] Domain behavior is verified without a database.
- [x] Repository callers cannot silently replace the association.
- [x] A rejected replacement leaves the original asset reference intact.

---

## Task 3 — Incremental migration upgrade test

### Decision

Construct a real database containing only migration `0001`, close it, then reopen it through normal `Storage::open`. Use the private migration machinery from the crate’s child test module rather than copying migration SQL.

A sentinel `applied_at_ms` value on migration 1 proves that reopening does not reapply or rewrite it.

### Implementation

- [x] Build a durable pre-`0002` database with the real `apply_migration(&MIGRATIONS[0])` path.
- [x] Confirm `generated_pdf_asset_id` is absent before upgrade.
- [x] Reopen through normal `Storage::open`.
- [x] Confirm migration 2 is recorded as `page_generated_pdf_asset`.
- [x] Confirm migration 1’s sentinel timestamp is unchanged.
- [x] Confirm each migration is recorded exactly once.
- [x] Exercise the new column through real typed page/asset repositories after upgrade.
- [x] Leave the test as the pattern future migrations must extend or mirror.

### Acceptance

- [x] A partially migrated user library upgrades in place.
- [x] Already-applied migrations are not rerun or rewritten.
- [x] The newly added schema is operational, not merely visible through `PRAGMA`.

---

## Task 4 — Test the orphaned-asset transaction failure path

### Decision

Do not use file/directory permission changes to force SQLite failure. That approach is unreliable with open file descriptors, WAL files, root-run containers, Windows, and CI permission models.

Use deterministic real-SQL fault injection instead: create a temporary SQLite trigger that aborts `INSERT` on `page_sets`. The PDF asset commit completes first, then the real registration transaction fails at its first SQL mutation.

This exercises production transaction/error behavior without adding a production mock provider or weakening encapsulation.

### Implementation

- [x] Create an abort trigger through the real `Storage::transaction` API in the test setup.
- [x] Call `generate_and_register_page_set` normally.
- [x] Assert the returned error includes `orphaned_asset_id` and the diagnostic note.
- [x] Assert the identified file exists under `assets/exports/`.
- [x] Assert `page_sets`, `pages`, and `assets` contain no rows from the failed attempt.
- [x] Prove the database transaction rolled back while the already-durable filesystem asset remained.

### Acceptance

- [x] The orphan annotation branch is exercised deterministically.
- [x] The test proves both sides of the known gap: file survives, database rows do not.
- [x] Automated orphan reconciliation remains correctly deferred to the integrity/review infrastructure milestone; this test does not misrepresent the gap as repaired.

---

## Task 5 — FFI wrapper delegation tests

### Decision

Prefix-only assertions are insufficient because malformed payloads could share the expected prefix. Generate through each `A2dClient` wrapper, parse through the canonical Rust QR parser, and assert the typed variant and important fields.

### Implementation

- [x] Test `generate_example_notebook_setup_qr_payload` through `A2dClient`.
- [x] Confirm repeated setup calls preserve fresh random identity.
- [x] Parse and assert `PageCode::NotebookSetup`.
- [x] Test and parse the notebook-page wrapper; verify logical page and layout.
- [x] Test and parse the Smart Page wrapper; verify layout, visible number, and absent Page Set.
- [x] Retain existing FFI error-mapping coverage for malformed page IDs.

### Acceptance

- [x] Every current example-QR `A2dClient` method has direct boundary-layer coverage.
- [x] Tests verify canonical round trips rather than only string prefixes.

---

## Tracking

| Task | Priority | Status |
|---|---|---|
| 1. Safe ruling spacing and bounded rendering | High | Complete |
| 2. Generated-PDF asset assignment invariant | High | Complete |
| 3. Incremental migration upgrade | Medium | Complete |
| 4. Orphaned-asset transaction failure | Medium | Complete |
| 5. FFI wrapper round trips | Low | Complete |

Future broad line/branch coverage measurement may use `cargo llvm-cov`, but no arbitrary percentage threshold is established here. Coverage percentages do not replace explicit tests for integrity, rollback, parser rejection, resource limits, and failure visibility.
