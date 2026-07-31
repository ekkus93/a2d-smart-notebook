# FIX-023 Temporary Cleanup Failure Reporting Evidence — 2026-07-31

## Status

The FIX-023 production contract and permanent regression are implemented on `master`.

The asset commit path preserves the original persistence failure as the primary `A2dError`. A failure while removing the temporary file is attached as structured secondary evidence instead of being discarded or replacing the primary cause.

This evidence document does not claim exact-head permanent CI success. The final documentation commit must receive its own full permanent-CI validation before FIX-023 is treated as fully signed off.

## Production behavior

`crates/a2d-storage/src/assets.rs` centralizes pre-finalization temporary-file cleanup in `with_cleanup_result`.

For every cleanup outcome, the helper returns the original primary error:

- successful removal adds `temp_cleanup_completed=true`;
- an already-missing temporary path is treated as completed and adds `temp_cleanup_completed=true`;
- any other removal failure adds:
  - `temp_path`;
  - `temp_cleanup_completed=false`;
  - `temp_cleanup_error` containing the secondary filesystem error.

The cleanup helper does not construct a replacement `A2dError`, change the original error code, convert the operation to success, or hide the path that may require reviewed cleanup.

Post-finalization temporary-link removal is also handled explicitly. A removal failure returns an error with the finalized asset recovery details and does not claim that the asset was registered in SQLite.

## Regression coverage

`crates/a2d-storage/tests/temp_cleanup_reporting.rs` permanently guards the contract without exposing a cleanup-only test primitive through the portable production API.

The regression proves by source drift checks that:

- the centralized cleanup helper remains present;
- `remove_file` is matched explicitly;
- retained temporary paths remain visible;
- incomplete cleanup remains distinguishable from successful cleanup;
- the secondary cleanup error remains structured evidence;
- production cleanup does not regress to `remove_file(...).ok()`;
- every branch augments and returns the original primary error;
- the helper does not replace the primary failure with a newly constructed cleanup error.

This source-level guard is intentional. The failure being protected is a secondary filesystem failure inside a private commit-protocol helper. Exposing a cleanup-only public API or adding production environment-variable fault behavior would weaken the storage boundary merely to facilitate a test.

## Commits

- `ca9effe36c997620b378f9d6c3be6ffae784a5c8` — `Add FIX-023 cleanup reporting regression`
- `9b06d20f94744f5e29d9cfca69f9c61839ae403d` — `Format FIX-023 cleanup regression`

## Acceptance result

The implementation satisfies the FIX-023 behavioral acceptance condition:

- no production temporary cleanup failure is discarded silently;
- the original persistence failure remains primary;
- incomplete cleanup is reported with the temporary path and secondary error details;
- callers receive failure rather than a false saved/success state;
- permanent regression coverage prevents the cleanup path from returning to silent `Result::ok()` handling.

Exact-head permanent-CI validation remains required after this evidence document is committed.
