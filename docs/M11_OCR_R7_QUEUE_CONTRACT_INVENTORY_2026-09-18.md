# M11 R7 OCR queue/status/retry contract inventory — 2026-09-18

This inventory supports R7 of `M11_OCR_POST_CLOSEOUT_REMEDIATION_TODO_2026-09-15.md`. It records the live durable queue owner, compatibility/provider contracts, and the policy duplication that must be removed or explicitly deprecated.

## Canonical durable runtime path

The durable queue is owned by Rust core and storage, not Android and not the provider-adapter contract crate.

- `crates/a2d-core/src/scan_policy.rs` includes and re-exports `ocr_queue.rs`; this is the production core queue surface.
- `crates/a2d-core/src/ocr_queue.rs` owns enqueue, claim, cancellation request, interrupted-worker recovery, retry scheduling, and durable queue snapshots. Its authoritative automatic attempt limit is `MAX_OCR_JOB_ATTEMPTS = 3`.
- `crates/a2d-core/src/ocr_finalization.rs` uses that same `MAX_OCR_JOB_ATTEMPTS` when deciding whether an unavailable provider result is requeued or terminal. Finalization also owns the transaction commit point introduced by R4/R5.
- `crates/a2d-storage` persists the durable job fields projected by core (`status`, `attempt_count`, `retryable`, `next_retry_at_ms`, provider availability, last error, last run, and cancellation request). Storage is persistence, not the policy owner.
- `crates/a2d-ffi/src/ocr_queue.rs` is a projection of `a2d_core::OcrJobSnapshot`; it does not make retry decisions. Its queue status/provider-availability enums exist only because UniFFI requires foreign-boundary types.
- Android consumes the FFI projection and must not independently decide durable retry eligibility or invent terminal status.

## Duplicate/compatibility contract

`crates/a2d-ocr/src/lib.rs` contains an older in-memory provider/queue contract with its own `OcrQueueLimits`, `OcrJobStatus`, `OcrRetryState`, and `OcrJobRecord`. `OcrQueueLimits::default()` currently permits 25 attempts, which conflicts with the live durable core policy of 3 attempts.

The `a2d-core` crate does not depend on `a2d-ocr`; therefore these `a2d-ocr` queue types are not the production durable queue owner. They are compatibility/provider-contract surface and must not be used as authority for storage, FFI, Android UI, or retry scheduling.

## Status-model mapping

The live durable status vocabulary is intentionally equivalent across layers but has distinct projection types:

- storage: `PersistedOcrJobStatus`
- core: `CoreOcrJobStatus`
- FFI: `OcrQueueJobStatus`
- Android: generated UniFFI status consumed for presentation/orchestration

These projections are acceptable only while they remain mechanical mappings from the Rust durable owner. They must not carry independent transition rules or limits.

Provider-result status is a separate concept. `OcrRunStatus` (`Detected`, `NoTextDetected`, `Unavailable`) describes one provider result/run; it is not a queue state machine. Keeping provider result types separate from durable queue policy is intentional.

## Authoritative retry policy

The remediation target is:

1. Rust core owns durable queue transitions and automatic retry eligibility.
2. `MAX_OCR_JOB_ATTEMPTS = 3` is the single automatic-attempt policy used by claim/recovery/finalization.
3. FFI and Android expose/project durable state; they do not duplicate the attempt limit for decisions.
4. The legacy `a2d-ocr` in-memory queue contract must be removed if unused or explicitly compatibility-deprecated and aligned so its defaults cannot contradict production policy.
5. Manual/user retry, if retained as a distinct operation, must be specified separately from automatic retry and must still be enforced by Rust core.

## Database compatibility

R7 should not change the schema merely to consolidate policy. The current durable job row already stores attempt count and retry scheduling state. Unless implementation discovers a database CHECK constraint that encodes a conflicting attempt limit, no migration is required. Released migration 0011 must not be rewritten. Existing libraries and existing OCR job rows must remain readable; rows already at or above the canonical limit must fail closed or resolve terminally rather than being granted extra attempts.

## Implementation work remaining in R7

- Remove or compatibility-deprecate the legacy `a2d-ocr` queue state machine and its conflicting 25-attempt default; align any retained compatibility limit with the core policy without creating a reverse dependency from core to the adapter crate.
- Confirm no Android source contains an independent retry-attempt constant or transition rule; replace any such decision with Rust-owned state/policy.
- Confirm storage schema has no conflicting attempt-count constraint and add existing-schema/open tests as required by the TODO.
- Add tests proving exactly three automatic claims maximum, terminal exhaustion, restart persistence/recovery, and FFI/UI diagnostics derived from canonical Rust state.
- Reconcile the R7 checklist and companion spec only after those behavioral tests and exact-head CI pass.

This document is an inventory, not R7 completion evidence.
