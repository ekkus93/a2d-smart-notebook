# R7 OCR Queue Database Compatibility — 2026-09-18

## Decision

No forward schema migration is required solely to consolidate the automatic OCR retry policy at three claimed attempts.

Migration `0011_ocr_job_queue.sql` is released history and remains immutable. Its `attempt_count` constraint permits values from 0 through 25. That storage constraint is intentionally more permissive than the current runtime orchestration policy; it is not the authoritative retry limit.

The authoritative automatic retry policy is `a2d_core::ocr_queue::MAX_OCR_JOB_ATTEMPTS = 3`. Core queue/finalization code decides whether another automatic attempt is eligible. Storage persists the observed attempt count and must remain capable of opening and reading libraries created under the earlier, more permissive schema.

## Compatibility consequences

- Do not rewrite migration 0011 to narrow its check constraint.
- Do not add a migration merely to change the database check from 25 to 3.
- Existing migration-0011 libraries remain structurally valid.
- Existing rows with attempt counts above the current runtime limit remain representable and readable; runtime must treat them as exhausted rather than schedule additional automatic work.
- New automatic orchestration must never create a fourth claimed attempt.
- Any future schema change for unrelated OCR queue data must be forward-only and must preserve existing rows.

## Required qualification

R7.4/R7.5 qualification must cover reopening an existing migration-0011 library, legacy attempt-count rows, exact three-attempt exhaustion, restart persistence, and UI/FFI diagnostics. This document records the schema decision; those behavioral tests remain required before R7 is complete.
