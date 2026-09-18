# M11 R7 canonical OCR queue/status/retry policy — 2026-09-18

This decision record closes the policy-selection portion of R7 in `M11_OCR_POST_CLOSEOUT_REMEDIATION_TODO_2026-09-15.md` and follows the inventory in `M11_OCR_R7_QUEUE_CONTRACT_INVENTORY_2026-09-18.md`.

## Decision

Rust core is the sole authority for durable OCR queue state transitions and retry eligibility.

The production owner is `crates/a2d-core/src/ocr_queue.rs`, with transactional terminal-result resolution in `crates/a2d-core/src/ocr_finalization.rs`. Storage persists the state selected by core; FFI mechanically projects that state; Android presents and orchestrates it. None of those projection layers may independently decide whether a durable OCR job is retryable or terminal.

The authoritative automatic attempt policy is exactly three claimed attempts per durable OCR job. The existing `a2d_core::ocr_queue::MAX_OCR_JOB_ATTEMPTS = 3` is the policy source used by queue claim/recovery/finalization logic. R7 implementation must remove or explicitly compatibility-deprecate conflicting retry limits rather than introducing another copy of this constant.

## Status boundaries

Durable queue state and provider-result state are separate contracts.

- Durable queue state belongs to Rust core and is persisted by storage.
- `Detected`, `NoTextDetected`, and `Unavailable` describe provider/finalization outcomes, not an independent queue state machine.
- FFI status enums are boundary projections required for UniFFI and must remain mechanical mappings.
- Android may choose presentation labels and action visibility from Rust-owned state, but it must not implement a second transition table or retry counter policy.
- The older `a2d-ocr` in-memory `OcrJobStatus`/`OcrRetryState`/`OcrQueueLimits` surface is not authoritative production durable state. Any retained surface is compatibility/provider-contract code only.

## Automatic retry semantics

An attempt is consumed when the durable core claim path accepts a job for execution. A job may receive at most three such automatic attempts. Retryable provider unavailability may requeue only while another attempt remains. Once the third claimed attempt is exhausted, durable core must resolve the job terminally rather than scheduling a fourth automatic claim.

Interrupted-worker recovery and transactional finalization must apply the same three-attempt policy. Restart must not reset `attempt_count`, and a pre-existing row already at or above the limit must fail closed: it must not be granted additional automatic attempts merely because an older or compatibility contract once allowed more.

## Manual retry

Manual/user retry is a distinct product action, not permission for Android to reset or bypass the automatic attempt policy. If manual retry remains supported, Rust core must own its eligibility and durable transition. R7 implementation/tests must make the distinction explicit; callers must consume the returned durable state rather than reconstructing eligibility locally.

## Database compatibility

No schema change is justified solely by this policy consolidation. The durable schema already persists attempt/retry state. Migration `0011` remains immutable. R7 must verify that no persisted CHECK constraint encodes the conflicting legacy limit and must exercise existing-schema/open plus existing-row behavior. If implementation discovers a schema-level conflict, it requires a new forward migration rather than rewriting historical migration SQL.

## Compatibility rule for `a2d-ocr`

The legacy `a2d-ocr` queue model currently defaults to 25 attempts. That value contradicts production policy and must not survive R7 as an apparently authoritative default. Preferred resolution is removal when repository usage proves the queue model dead. If compatibility requires retention, it must be clearly deprecated/non-authoritative and aligned so its default cannot promise more attempts than durable core permits. Core must not acquire a reverse dependency on the adapter crate to accomplish this.

## Required proof before R7 closeout

R7 is not complete from this decision record alone. Code and tests must prove exactly three automatic claims maximum, exhaustion behavior, persistence across restart/recovery, FFI/UI diagnostics derived from Rust-owned state, and compatibility with existing schema/job rows. Exact-head CI and post-merge master CI remain required by the remediation TODO.
