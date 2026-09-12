# FIX-111 — Milestone 8.6 Camera Failure Matrix

**Status:** Complete  
**Date:** 2026-09-12  
**Scope:** Milestone 8.6 / FIX-111  
**Completion evidence head:** `cb9cf1405bb48ec5d31b8febbc988d52d1a46a01`  
**Permanent CI evidence:** `34713845468`  
**Invariant:** a camera/lifecycle/storage failure must never fabricate save success, delete a committed original, or delete a staged image that is still owned by a durable scanner-recovery record.

## Phase vocabulary

The matrix uses these externally meaningful phases:

- `PermissionBlocked` — CameraX is not started because Android permission is absent.
- `CameraInitializing` — permission exists and CameraX is binding/rebinding.
- `Searching` — camera is bound and no capture is active.
- `Capturing` — a Rust scanner-recovery record and its exact staging path exist before CameraX writes.
- `Captured` — the recovery record owns a finalized staged image but preview processing is not yet durable.
- `PreviewReady` — Rust recorded successful preview processing; registration has not committed.
- `Registering` — Rust registration reconciliation may be required after interruption.
- `Committed` — SQLite/asset registration is durable under the documented storage contract; recovery acknowledgement may still be pending.
- `NeedsReview` — durable/recoverable data is retained and the user must resolve the case explicitly.

A CameraX bind/torch error is a platform-adapter failure, not a scan-registration success. A callback from an obsolete generation is evidence about an old operation and must not mutate the current generation.

## Authoritative matrix

| FIX-111 case | Expected phase after the event | Files that must remain | Files that may be removed | Retry | User-visible result | Evidence/status |
|---|---|---|---|---|---|---|
| Permission denied | `PermissionBlocked` | Existing library/assets/recoveries unchanged | None because capture never starts | Yes, permission request | Permission explanation + Retry | Implemented in `PolicyAwareSinglePageScannerRoute.kt`; covered by scanner UI tests |
| Permission permanently denied | `PermissionBlocked` | Existing library/assets/recoveries unchanged | None | Yes, through Android settings | Camera unavailable + Open Settings | Implemented in `PolicyAwareSinglePageScannerRoute.kt`; covered by scanner UI tests |
| Camera unavailable / bind failure | Camera state `Error`; scanner must not enter `Capturing` unless a capture was already durably prepared | Any pre-existing recoveries and assets | No owned recovery/staged image | Yes after camera becomes available | Explicit CameraX error; no Saved state | `CameraXAdapter.bind()` publishes `CameraAdapterState.Error`; Android scanner tests cover no-success behavior |
| Background/navigation during capture | Current UI generation leaves active capture; durable recovery remains authoritative | Recovery journal and its staging path if recovery exists | Only an unjournaled reservation may be cleaned | Yes, by recovery/review | Capture interruption/recovery available; never Saved from a stale callback | `shouldDeliverCameraCaptureCallback()` drops callbacks after close/rebind; `CameraCaptureCallbackGenerationTest.kt` and recovery bridge tests cover the boundary |
| Process killed after capture before registration | `Captured` or `PreviewReady` on restart | Finalized staged image + recovery journal | Nothing automatically | Yes, Review/Retry/Discard | Recovery prompt | `ScannerRecoveryBridgeTest.kt` and FIX-110 core recovery tests |
| Rotation during analysis | `Searching` or current capture phase; obsolete analysis result ignored | Current generation's files/recovery | No current-generation file | Yes / automatic continuation | No stale analysis mutation | Existing `bindGeneration` analysis suppression plus scanner presentation tests |
| Rapid repeated manual capture | First valid request remains authoritative | First request's staged/recovery data | No authoritative file due to duplicate click | Yes after first operation resolves | Capture-already-active / warning flow | `AutoCaptureStateMachineTest` manual/capture-active behavior |
| Rapid repeated automatic-capture callback | One active token; duplicates are stale/debounced | Authoritative capture/recovery | No authoritative file | Automatic after state permits | Debounce/stale effect, not duplicate registration | `AutoCaptureStateMachineTest` debounce/stale-token coverage |
| Wrong-design page | `Searching`/blocked identity; no capture authorization | Existing user data | None | Yes with correct page/notebook | Wrong Notebook/design guidance; auto/manual identity override blocked | `LiveScannerPresentationUiTest` and Rust identity-gate tests |
| Two identical Notebook candidates | Identity unresolved / blocked before capture | Existing user data | None | Yes after explicit Notebook selection | Choose matching Notebook | Rust page-resolution contract and scanner presentation tests |
| Low-storage staging failure | Capture preparation fails before CameraX is authorized, unless recovery was already persisted | Any persisted recovery record and file it owns | Only an unjournaled failed reservation; no finalized capture is fabricated | Yes after freeing space | Storage/capture-preparation failure; no Saved state | Real Linux tmpfs `ENOSPC` gate `ScannerStagingEnospcTest.kt` in workflow run `34713845468` |
| Low-storage asset finalization failure | Registration fails at exact Rust persistence stage; durable/recoverable input remains | Original staged capture/recovery plus any finalized-unregistered asset reported by structured details | Only explicitly identified temp files when cleanup succeeds | Yes after freeing space/reconciliation | Registration/storage error with retry/review; no Saved claim | Real Linux tmpfs `ENOSPC` gate `crates/a2d-storage/tests/enospc.rs` in workflow run `34713845468` |
| Stale CameraX callback after rebind | Current generation unchanged by obsolete callback | Recovery/staged data owned by old operation | Never delete recovery-owned data because callback is stale | Yes through reconciliation/current generation | Stale callback ignored/recovery retained | `CameraXAdapter.capture()` generation guard and `CameraCaptureCallbackGenerationTest.kt` |
| Torch unavailable / torch failure | Remain in camera-bound/searching state when possible; scanning without torch remains possible | All user data | None | Yes; continue without torch | Explicit no-torch / torch-control failure | `CameraXAdapter.setTorch()` and terminal/error handling tests |
| Cleanup failure | `Closed(cleanupWarning)` or operation error with retained recovery | Any recovery-owned staging and all committed assets | Cleanup target only if deletion actually succeeds | Retry cleanup/recovery when meaningful | Cleanup warning remains observable | `CameraAdapterTerminalStateTest.kt`; Rust asset cleanup details |

## Capture/finalization interruption boundaries

These boundaries preserve the following invariant set:

| Boundary | Required durable/recoverable result | Evidence/status |
|---|---|---|
| Before staging reservation | No new file and no recovery record | Scanner setup fails before capture authorization |
| After reservation, before recovery creation | Reservation may be cleaned; cleanup failure is visible | Reservation sentinel logic and staging ENOSPC gate |
| After recovery creation, before CameraX write | Recovery + reservation remain discoverable | CameraX reservation replacement is restricted to exact A2D sentinels |
| During CameraX write | Recovery remains; finalized/partial file is not treated as saved | Capture callbacks are generation guarded and finalized-file checks remain required |
| After JPEG finalization, before processing | Recovery + JPEG remain; restart can review/retry | Scanner recovery bridge tests |
| During native preview processing | Recovery + JPEG remain; cancellation/failure is explicit | Recovery bridge and processing cancellation tests |
| After preview-ready | Recovery phase is `PreviewReady`; JPEG remains | Rust recovery tests |
| During registration | `Registering` can reconcile after restart; no duplicate registration | FIX-110 reconciliation tests |
| After DB/asset commit before Android success callback | `Committed` reconciles to the existing scan; no second scan is created | FIX-110 reconciliation tests |
| During batch completion/acknowledgement | Completed batch summary persists before recovery acknowledgement; committed originals remain | Batch scanner core tests |

## Batch ordering contract

Batch correctness is keyed by durable recovery token/page identity, not callback arrival order. Obsolete camera-generation callbacks, delayed callbacks after the user advances, worker completion after UI recreation, and out-of-order processing must not attach one capture to another page or create a duplicate registration. The Rust batch session and scanner-recovery journal are the sources of truth.

Permanent evidence exists in `crates/a2d-core/tests/batch_scanner_ordering.rs`. The test queues two distinct captures, completes the second terminal result first, reopens the library, then completes the first terminal result, and verifies each batch entry keeps its original recovery token, page ID, status, and review item identity.

## Closure evidence

FIX-111 is complete at evidence head `cb9cf1405bb48ec5d31b8febbc988d52d1a46a01` because permanent CI run `34713845468` passed all of the following gates:

1. Camera capture callback invalidation after rebind/background without deleting recovery-owned files.
2. Batch out-of-order/recreation behavior at the durable-token boundary.
3. Real filesystem `ENOSPC` staging/write failure with explicit no-saved-state assertions.
4. Real filesystem `ENOSPC` asset persistence/finalization failure with structured persistence-stage assertions.
5. Existing scanner, recovery, panic-containment, scan-revision, Needs Review, and version UI emulator coverage.
