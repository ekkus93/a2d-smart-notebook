# M11 OCR R17 Cancellation Audit — 2026-09-20

Audited against `master` `9acd2d5cf1f74807bac5480dbefe609e45e3a292` after PR #142.

## Confirmed fixed

`AndroidOcrSearchController.submit()` preserves coroutine cancellation by rethrowing `CancellationException` before ordinary error mapping (PR #141).

## Confirmed Page Viewer cancellation gaps

`apps/android/app/src/main/kotlin/com/a2d/notebook/navigation/A2dNavHost.kt` contains suspend Page Viewer orchestration that performs FFI work inside cancellable `withContext(Dispatchers.IO)` calls and then catches broad `Exception` without first rethrowing `CancellationException`.

Affected functions on the audited head:

- `hydrateOcrCorrectionReviewForViewer`
- `submitOcrCorrectionForViewer`
- `hydrateOcrForViewer`
- `hydrateOcrReadback`
- `enqueueOcrJobForViewer`
- `manualRetryOcrJobForViewer`
- `cancelOcrJobForViewer`

A cancellation delivered while these operations are suspended can therefore be converted into ordinary Page Viewer error/presentation state instead of remaining coroutine cancellation. This contradicts R17.1/R17.2.

## Required remediation

Each broad catch in the affected suspend orchestration must preserve structured cancellation, e.g. by rethrowing `CancellationException` before mapping ordinary failures. Tests must cover cancellation during Page Viewer hydration/navigation-away and correction submission/hydration where supported, and assert that no stale error state is returned after cancellation.

## Active-client lifecycle

PR #142 documents the current product contract: one process-lifetime `A2dClient`, shared by queue/navigation/Search/Page Viewer/correction; runtime library switching is intentionally unsupported. If switching is introduced later, the queue runtime and all client-bound gateways/controllers must be disposed and recreated together.
