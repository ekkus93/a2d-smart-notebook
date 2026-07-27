# Unit Test Coverage TODO

**Status:** Open
**Date:** 2026-07-27
**Source:** Ad hoc coverage review of the Rust workspace (not tied to a spec/TODO milestone), conducted by comparing test-count-to-complexity per crate and reading the specific files with the largest gaps.

This document tracks concrete, identified unit-test gaps and one latent bug the review surfaced. It is not a new milestone — it supplements `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`. Follow the same completion bar as that file: a task here is done when it compiles, the new test(s) genuinely fail without the fix (where a fix is involved) and pass with it, `cargo fmt --check` / `cargo clippy --workspace --all-targets --all-features -- -D warnings` / `cargo test --workspace --all-features` all stay green, and — for Task 1 — the underlying behavior actually changed, not just a new test file appended.

Tasks are ordered by priority (highest risk first). Each is independently committable.

---

## Task 1 — Fix and test the zero/negative ruling-spacing infinite loop (highest priority)

**Why:** `content_style_ops` in `crates/a2d-pdf/src/render.rs` draws `ContentStyle::Lined`/`Graph`/`DotGrid` ruling with loops of the shape:

```rust
let mut y = content_rect.top() + line_spacing_mm;
while y < content_rect.bottom() {
    ...
    y += line_spacing_mm;
}
```

If `line_spacing_mm` (or `spacing_mm`) is `0.0`, this never terminates — an unbounded hang with unbounded `Vec<Op>` growth, not a crash. A negative value has the same effect (`y` never increases past `content_rect.bottom()`, or moves the wrong direction depending on sign). Nothing validates this anywhere today:

- `ContentStyle`'s variants (`crates/a2d-layout/src/page_layout.rs`) place no constraint on their `f64` fields.
- `PageLayout::validate()` checks marker-role uniqueness, safe-margin bounds, and quiet-zone overlap, but never inspects `content_style`.
- `content_style_ops` itself trusts its input unconditionally.

This is latent only because every current caller (`a2d_layout::smart_page::smart_page_layout`, `a2d_layout::notebook::{setup_page_layout, writable_page_layout}`) hardcodes positive constants (7.0mm / 5.0mm). Nothing stops a future caller — e.g. a user-configurable ruling style once Milestone 6+ builds UI around Notebook Design creation — from constructing a `ContentStyle` with a zero or negative spacing and hanging PDF generation.

### Subtasks

- [ ] Decide where the guard belongs (pick one, document the reasoning inline where the check lives):
  - [ ] Option A: reject at construction — add a fallible constructor for `ContentStyle::Lined`/`Graph`/`DotGrid` (or a `validate()` method on `ContentStyle`) that rejects non-positive spacing with a typed `A2dError`, and wire `PageLayout::validate()` to call it.
  - [ ] Option B: make `content_style_ops` itself defensive — treat non-positive spacing as a hard error (`Result<Vec<Op>, A2dError>` instead of `Vec<Op>`) rather than looping, so a malformed layout can never hang generation even if it slipped past `PageLayout::validate()`.
  - [ ] Recommended: do **both** — validate early (fail fast, clear error at layout-construction time) *and* make the renderer itself refuse to loop on a non-positive step (defense in depth, since `PageLayout` values can in principle be constructed by hand without going through `validate()`).
- [ ] Update `crates/a2d-layout/src/page_layout.rs`:
  - [ ] Add the chosen validation to `PageLayout::validate()` (or a new `ContentStyle` method it calls).
  - [ ] Add a new `A2dError` code (e.g. `LAYOUT_CONTENT_STYLE_SPACING_NOT_POSITIVE`) with category `Validation`.
  - [ ] Test: `PageLayout::validate()` rejects `ContentStyle::Lined { line_spacing_mm: 0.0 }`.
  - [ ] Test: rejects a negative `line_spacing_mm`.
  - [ ] Test: rejects `ContentStyle::Graph { spacing_mm: 0.0 }` and a negative `spacing_mm`.
  - [ ] Test: rejects `ContentStyle::DotGrid { spacing_mm: 0.0 }` and a negative `spacing_mm`.
  - [ ] Test: `ContentStyle::Blank` and any positive spacing still validate successfully (no false-positive rejection).
- [ ] Update `crates/a2d-pdf/src/render.rs`:
  - [ ] Change `content_style_ops`'s signature to return `Result<Vec<Op>, A2dError>` (or push the check into a small guard function it calls first) and have `render_page_ops` propagate the error with `?`.
  - [ ] Test: calling the renderer with a hand-constructed (not going through `a2d_layout`'s builders) `PageLayout` whose `content_style` has zero spacing returns `Err` instead of hanging — assert on a bounded wall-clock time or, more directly, assert the `Err` variant and code, so the test itself can't hang if the fix regresses. Do not write a test that could hang on failure (e.g. an unbounded loop with no timeout) — assert the error path directly.
  - [ ] Confirm existing tests (`render_page_ops_draws_a_filled_polygon_pair_for_every_marker`, etc.) still pass with the signature change (update call sites/assertions for the new `Result` as needed).
- [ ] Run the full gate (`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`) and confirm green.
- [ ] Update `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`'s Milestone 5.1/5.4 notes if the `PageLayout`/`render_page_ops` public signatures changed, so the TODO's own evidence trail stays accurate.

### Acceptance

- [ ] A `PageLayout` with non-positive ruling spacing cannot pass `PageLayout::validate()`.
- [ ] `render_page_ops` cannot be made to hang by a non-positive spacing value even if a caller bypasses `validate()`.
- [ ] All new tests fail on the pre-fix code (verified by checking out the fix, confirming the test hangs or loops before the fix — do this manually during review, not as a permanent test) and pass after.

---

## Task 2 — Direct unit test for `Page::set_generated_pdf_asset`

**Why:** `crates/a2d-domain/src/entities.rs`'s `Page::set_preferred_scan` has a dedicated test (`preferred_scan_must_belong_to_the_same_page`) covering both the mismatch-rejection path and that `updated_at_ms` gets bumped. Its sibling, `set_generated_pdf_asset` (added for TODO 5.5), has no direct test in `a2d-domain` at all — it's only exercised indirectly through `a2d-storage`'s `set_generated_pdf_asset_attaches_a_committed_asset_and_round_trips`, which verifies the *stored* value round-trips correctly but never tests the entity method's own behavior in isolation (i.e., without a database in the loop).

### Subtasks

- [ ] Add a test in `crates/a2d-domain/src/entities.rs`'s `mod tests` (near `preferred_scan_must_belong_to_the_same_page`):
  - [ ] Construct a `Page` via the existing `gen_page` test helper (or equivalent).
  - [ ] Call `set_generated_pdf_asset` with a freshly generated `AssetId` and a `now_ms` distinct from the page's `created_at_ms`.
  - [ ] Assert `page.generated_pdf_asset_id == Some(asset_id)`.
  - [ ] Assert `page.updated_at_ms == now_ms` (mirroring how `set_preferred_scan`'s test checks the same for that method).
  - [ ] Assert calling it a second time with a different `AssetId` overwrites the first (no accumulation, no rejection — unlike `set_preferred_scan`, there is no "must belong to this page" concept here, so confirm the method is a plain, unconditional setter as documented).
- [ ] Run `cargo test -p a2d-domain` and confirm the new test passes.

### Acceptance

- [ ] `Page::set_generated_pdf_asset`'s field-setting and `updated_at_ms`-bumping behavior is verified without going through `a2d-storage` at all.

---

## Task 3 — Incremental migration upgrade test

**Why:** `Storage::migrate()` (`crates/a2d-storage/src/lib.rs`) applies whichever migrations in `MIGRATIONS` aren't yet recorded in `schema_migrations`, in version order — it is explicitly built to support a database that already has *some* but not all migrations applied. But the only existing tests are `open_creates_a_fresh_database_and_applies_migrations` (blank DB, everything applied) and `reopening_an_existing_database_does_not_reapply_migrations` (fully-migrated DB, nothing more applied). The realistic middle case — an existing user's library created before migration `0002_page_generated_pdf_asset.sql` existed, then opened with current code — has no test at all. This is exactly the scenario that migration numbering exists to support, and nobody has verified it actually works end-to-end.

### Subtasks

- [ ] Add a test in `crates/a2d-storage/src/lib.rs`'s `mod tests` (near the other migration tests):
  - [ ] Open a fresh database at a temp path.
  - [ ] Apply **only** migration `1` — either by calling the internal `apply_migration` machinery directly with `MIGRATIONS[0]` (if accessible from the test module) or by constructing the equivalent effect: run migration 1's SQL directly and record it in `schema_migrations` the same way `migrate()` would, so the on-disk state matches "a library created before 0002 existed."
  - [ ] Confirm the `pages` table does **not** yet have a `generated_pdf_asset_id` column (sanity check that the simulated pre-0002 state is real, e.g. via `PRAGMA table_info(pages)`).
  - [ ] Reopen the same database path through the normal `Storage::open` (full current `MIGRATIONS` list, including `0002`).
  - [ ] Assert this succeeds (no error).
  - [ ] Assert `schema_migrations` now has a row for version `2` with `name = "page_generated_pdf_asset"`.
  - [ ] Assert the `pages` table now has the `generated_pdf_asset_id` column and it is genuinely usable — e.g. insert a `Page`, call `set_generated_pdf_asset`-equivalent SQL (or go through the real repository call), and read it back successfully.
  - [ ] Assert migration `1` was **not** re-applied (e.g. confirm no error from re-running 0001's `CREATE TABLE` statements, or check `schema_migrations`'s row for version 1 still has its original `applied_at_ms`, proving it wasn't touched again).
- [ ] Run `cargo test -p a2d-storage` and confirm green.

### Acceptance

- [ ] A database created with only the migrations that existed *before* this session's `0002` is proven to upgrade cleanly and incrementally when opened with current code — not just "a blank DB gets everything" or "a fully-current DB gets nothing more."
- [ ] This test pattern is reusable for `0003` and beyond — note in the test's doc comment that future migrations should extend this test (or add a sibling) rather than leaving the incremental-upgrade path untested again.

---

## Task 4 — Test the orphaned-asset error path in `generate_and_register_page_set`

**Why:** `A2dCore::generate_and_register_page_set` (`crates/a2d-core/src/lib.rs`) documents a known gap: if the `Storage::transaction` fails *after* the PDF asset was already durably committed via `AssetStore::commit`, that asset file is orphaned, and the returned error carries `orphaned_asset_id` in its `details` map so it's at least diagnosable. This error-annotation code path has zero test coverage.

**Known difficulty:** every ID involved (`PageSetId`, each `SmartPageId`, each `PageId`, the `AssetId`) is generated internally and randomly, so there is no natural way from outside the function to force a collision or FK violation inside the transaction. Closing this gap needs a way to make the transaction fail *after* the asset commit succeeds, deterministically, without weakening the production code's design.

### Subtasks

- [ ] Pick a concrete failure-injection approach (evaluate feasibility before committing to one):
  - [ ] Option A (filesystem-level): after the asset is committed but before calling `generate_and_register_page_set`'s transaction, make the SQLite database file (or its containing directory) read-only via `std::fs::Permissions`, so the `INSERT`s inside the transaction fail with a real I/O/permission error. Restore permissions in a test-cleanup step (or rely on the temp-dir teardown) regardless of pass/fail.
  - [ ] Option B (structural refactor): extract the "commit asset, then run the transaction" sequence behind a small internal trait or callback that a test can intercept to force the transaction closure to return `Err` after the asset commit has already happened — without changing the *behavior* of the non-test code path. Only pursue this if Option A proves unreliable (e.g. permission changes don't reliably fail SQLite writes in the CI sandbox).
  - [ ] Prefer Option A first — it exercises the real code path with no test-only seams added to production code, consistent with this project's general preference for testing real behavior over injected mocks (see `CLAUDE.md`'s testing guidance).
- [ ] Implement the chosen approach in `crates/a2d-core/src/lib.rs`'s `mod tests`:
  - [ ] Trigger `generate_and_register_page_set` under the forced-failure condition.
  - [ ] Assert the call returns `Err`.
  - [ ] Assert the error's `details` map contains `orphaned_asset_id`.
  - [ ] Assert the asset file itself genuinely exists on disk at the expected path under `assets/exports/` (i.e. confirm it really was durably committed before the failure, not that the test is vacuously true because the commit itself also failed).
  - [ ] Assert no `PageSet`/`Page` rows exist for this attempt afterward (confirms the transaction genuinely rolled back, not just that an error was returned).
- [ ] If Option A is unreliable in this sandbox (permission changes don't affect SQLite's ability to write, e.g. running as root), fall back to Option B and note why in the test's doc comment.
- [ ] Run `cargo test -p a2d-core` and confirm green.

### Acceptance

- [ ] The orphaned-asset error-annotation branch is exercised by a real test, not just documented in a comment.
- [ ] The test proves the asset file survives on disk and the DB rows do not, matching the documented behavior exactly.

---

## Task 5 — FFI wrapper delegation tests (low priority)

**Why:** `A2dClient`'s `generate_example_notebook_setup_qr_payload`, `generate_example_notebook_page_qr_payload`, and `generate_example_smart_page_qr_payload` (`crates/a2d-ffi/src/lib.rs`) are one-line delegations to the already-tested `A2dCore` equivalents. They're low-risk (no logic beyond the delegation and error-type mapping), but currently have zero direct test coverage at the `a2d-ffi` layer — only their `a2d-core` counterparts are tested.

### Subtasks

- [ ] Add tests in `crates/a2d-ffi/src/lib.rs`'s `mod tests` (near `open_generate_and_parse_round_trip_through_the_ffi_types`):
  - [ ] `generate_example_notebook_setup_qr_payload` returns a well-formed payload (`starts_with("A2D:1:S:")`) through the FFI wrapper.
  - [ ] `generate_example_notebook_page_qr_payload` returns a well-formed payload (`starts_with("A2D:1:B:")`).
  - [ ] `generate_example_smart_page_qr_payload` returns a well-formed payload (`starts_with("A2D:1:M:")`).
- [ ] Run `cargo test -p a2d-ffi` and confirm green.

### Acceptance

- [ ] Every `A2dClient` method that crosses the FFI boundary has at least one direct test at the `a2d-ffi` layer, not only at the layer it delegates to.

---

## Tracking

| Task | Priority | Status |
|---|---|---|
| 1. Fix + test ruling-spacing infinite loop | High (latent bug) | Not started |
| 2. `Page::set_generated_pdf_asset` unit test | Medium | Not started |
| 3. Incremental migration upgrade test | Medium | Not started |
| 4. Orphaned-asset error path test | Medium (needs design work first) | Not started |
| 5. FFI wrapper delegation tests | Low | Not started |

Run the full gate (`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo deny check`) after each task before marking it complete, per this repo's standing checks (`CLAUDE.md`). One commit per task, pushed individually, is consistent with this repo's etiquette for `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` and should be followed here too.
