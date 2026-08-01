# A2D Smart Notebook — Remediation Traceability Ledger

**Status:** Authoritative FIX-to-implementation ledger  
**Date:** 2026-07-31  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Remediation plan:** `docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md`  
**Reconciled roadmap:** `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

This ledger satisfies FIX-121 by mapping every remediation fix ID to its roadmap scope, representative production paths, focused evidence, completion commit, and validation state. Nearby code is not treated as proof when a stated acceptance requirement remains open.

## Status vocabulary

- **Complete:** required production behavior and focused evidence exist.
- **Partial:** meaningful implementation exists, but at least one acceptance requirement remains open.
- **Open:** the bounded remediation task has not been completed.

## Validation baseline

The last code-bearing remediation candidate was `d2cb054d2489cf2b0f1e66d9370b5650b31404d0`.

- Permanent full CI run `30673255456` passed.
- Milestone 7 native/fixture run `30673255457` passed.
- These runs covered the integrated implementation for completed FIX-001 through FIX-110: Rust formatting, strict Clippy, workspace tests, dependency policy, binding drift, Android lint/JVM tests/debug APK, both Android ABIs, APK symbol/notices checks, emulator integration, fixture drift, and Apple compile feasibility.
- Later branch-cleanup commits changed no production files.

## Traceability table

| Fix | Status | Milestone(s) | Primary implementation | Focused evidence | Completion commit | Validation |
|---|---|---|---|---|---|---|
| FIX-001 | Complete | 1, 2, 9 | `a2d-ffi` comparison/registration surfaces and checked-in Kotlin binding | Binding-generation test and permanent drift job | `55a8bd370b5f8781791548f675879b76b50d7ab9` | `30673255456` |
| FIX-002 | Complete | 1, 2, 15 | Root/Android/iOS binding policy and generation scripts | Regeneration and drift documentation | `99c6b764dd811ff7bab370d28d186f5d6e5b7bb3` | `30673255456` |
| FIX-010 | Complete | 3, 9 | Atomic preferred-scan workflow and migrations `0005`–`0008` | Preferred-scan migration/workflow tests | `c547bbfdf66ae7a131e386f2e448fd72fef940f6` | `30673255456` |
| FIX-011 | Complete | 3, 9 | All production preferred-scan mutations routed through the workflow | Core registration and workflow-gate tests | `9c18f6765b93887d3d60ebed2f4893bd7b8af80b` | `30673255456` |
| FIX-020 | Complete | 3, 5, 9 | `V01_STORAGE_DURABILITY_CONTRACT.md`; storage implementation | Durability terminology/source drift test | `1c78f4aa7552b22788f23e729e16773b49575021` | `30673255456` |
| FIX-021 | Complete | 3 | No-replace, synchronized asset finalization platform adapter | Collision, sync, permission and finalization regressions | `dc293354c27eaac3420cc50ba5e0fd9703672865` | `30673255456` |
| FIX-022 | Complete | 3, 9, 16 | Typed post-finalization failures and orphan-final-asset discovery | `FIX022_GENERIC_SCAN_REGISTRATION_ROLLBACK_EVIDENCE_2026-07-29.md`; orphan tests | `8879980845344d16467f07b1ec85421381b78cf4` | `30673255456` |
| FIX-023 | Complete | 3, 5, 8 | Explicit storage/PDF/QR/scanner cleanup reporting | Cleanup-failure and warning assertions | `f777098e2ff249ab27096848661d08cdcadef51d` | `30673255456` |
| FIX-024 | Complete | 3, 9, 16 | Validated canonical asset/staging path resolution | Traversal, symlink, root-containment tests | `fbbe13925416469f890b58a302400c0de7121722` | `30673255456` |
| FIX-030 | Complete | 7, 8, 9 | Rust-owned stored-scan processing/layout policy | Policy/layout mismatch and registration tests | `2ac991e9b69bcf5acd8b8df7d0c4a94ce7732906` | `30673255456` |
| FIX-031 | Complete | 8, 9 | Explicit Notebook Page versus Smart Page scanner scope | Unsupported-scope/layout tests | `2bff6606558b5ebd441337187d519b938fbd4079` | `30673255456` |
| FIX-040 | Complete | 2, 16 | `A2dFfiErrorDetails` and ordered detail projection | FFI detail mapping and generated-binding tests | `6d64bc9636faad08c450c93d7748cdd505714ac4` | `30673255456` |
| FIX-041 | Complete | 2, 4, 6 | Fallible production ID APIs across domain/core/FFI | RNG-failure and projection tests | `4ceffab412f41d79c6ce5c76e3a6bfc833398a5b` | `30673255456` |
| FIX-042 | Complete | 2, 19 | `ffi-test-panic` feature gate and production symbol exclusion | Panic-containment test build and APK verifier | `18f90fc17623b29b09e61ac8b4cb526703e5c408` | `30673255456` |
| FIX-043 | Complete | 2, 3, 6, 9 | Fallible portable clock and timestamp propagation | Clock-failure rollback/timestamp tests | `3ab520dabcecc276c44ec24320dd1eb42471dd31` | `30673255456` |
| FIX-044 | Complete | 4, 6, 16 | Registry corruption/failure classification preserved through core | Corrupt-registry versus unsupported-data tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-050 | Complete | 5, 6, 16 | Rust-owned Smart Page/PDF count policies | Request-limit and checked-range tests | `de953408b473e7e2461853fe377c7c04714f06ac` | `30673255456` |
| FIX-051 | Complete | 4, 16 | Bounded manifest JSON and semantic validation | Size/count/duplicate/unsupported-field tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-052 | Complete | 4, 6, 16 | Explicit registry result propagation | Corrupt/missing/unsupported design tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-060 | Complete | 9, 16 | Asset-backed stored-scan comparison in `a2d-core` | Missing/tampered/hash/fingerprint tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-061 | Complete | 9, 14 | Typed UniFFI comparison projection and generated binding | Projection, threshold-range and binding-drift tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-070 | Complete | 7, 8, 9 | Calibration contract, durable registration semantics, Android presentation | Core/JVM/emulator tests; `FIX_070_QUALITY_CALIBRATION_COMPLETION_2026-07-31.md` | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456`, `30673255457` |
| FIX-080 | Complete | 6, 8 | Cancellation-preserving coroutine helper and call sites | `CoroutineFailuresTest.kt` and ViewModel tests | `8ff1b53ee42bb52e3904d3812c893f2c695406ae` | `30673255456` |
| FIX-081 | Complete | 6, 8 | Recreation-safe QR token/path handling | Recreation, stale-token, orphan and cleanup tests | `ebaf44a9b3309c49963c53ba53f665f2a87e0869` | `30673255456` |
| FIX-082 | Complete | 6 | Saved-state Smart Page save token and bitmap ownership | Pending-save, stale callback and bitmap tests | `32695abb1c6a05a90c8eec554f652d489584c9fa` | `30673255456` |
| FIX-083 | Complete | 8 | CameraX terminal `Closed(cleanupWarning)` state | `CameraAdapterTerminalStateTest.kt` | `5bfff4b26ef14a8e1ccf15779897792e86d3cc48` | `30673255456` |
| FIX-090 | Complete | 5 | Hardened standalone PDF temp/write/verify/no-replace protocol | Destination, corrupt/warning, sync and cleanup tests | `abbb99d3a8691bd909068a15686b2fa2faa946e0` | `30673255456` |
| FIX-100 | Complete | 3, 16 | Migration-history SHA-256 verification/backfill | Migration history integrity tests | `716559d8c6757bb778bc953b010f19c150856be9` | `30673255456` |
| FIX-101 | Complete | 3, 16 | Bounded non-destructive storage/core integrity checker | Corruption, limit, cancellation, asset/orphan and relational tests | `ebe0abfabf226de3410dd048d94089c6e38999e3` | `30673255456` |
| FIX-110 | Complete | 8, 9, 16 | Rust scanner journal, FFI projection and Android recovery lifecycle | Core recovery tests and `ScannerRecoveryBridgeTest.kt` | `bd46b76c324c184bf855932c54ae1463617ef425` | `30673255456` |
| FIX-111 | **Partial** | 8, 16, 19 | Existing permission, CameraX, state-machine, presentation and recovery paths | Many focused cases exist; consolidated matrix, batch ordering and real low-storage evidence remain open | — | Partial coverage in `30673255456` |
| FIX-120 | Complete | 1–19 | Reconciled `A2D_SMART_NOTEBOOK_V01_TODO.md` | Source/evidence audit summarized below | `c139b861280da9cd697d7f1f06971029e998e9f0` | Exact documentation-head CI recorded outside this self-referential file |
| FIX-121 | Complete | Remediation-wide | This ledger | Every remediation ID has a status and evidence mapping | `ed8f29da378e3333ea115cd8b3860eeb12836b82` | Exact documentation-head CI recorded outside this self-referential file |
| FIX-130 | **Partial** | 1, 3–9, 16, 19 | Permanent CI/native/fixture/APK workflows | Most repaired invariants are gated; deliberate regression proof for every invariant is not consolidated | — | Code-bearing baseline green |
| FIX-131 | Open | 19 | Final remediation head and job evidence | Requires completion of all remediation phases | — | — |
| FIX-140 | Open | 2, 16 | Repository-wide Rust failure-erasure audit | No complete classified audit ledger | — | — |
| FIX-141 | Open | 3, 5, 8, 16 | Repository-wide production cleanup audit | Known paths hardened; complete classified audit absent | — | — |
| FIX-142 | Open | 4–7, 9, 13, 14, 16 | Repository-wide arithmetic/allocation audit | Implemented paths are bounded; complete audit ledger absent | — | — |
| FIX-150 | **Partial** | Architecture-wide | Decisions, READMEs, specification, roadmap and this ledger | Major contracts documented; complete cross-document agreement remains open | — | — |
| FIX-151 | Open | Documentation/release | All Markdown path references and future validator | Complete path audit/script absent | — | — |

## FIX-120 reconciliation conclusions

1. The blanket “Milestones 1–6 complete” header was removed.
2. Implementation-complete, partial, physical-evidence-pending, and not-implemented states are distinct.
3. Checked-in Kotlin bindings and Swift generation smoke are documented accurately.
4. Structured FFI details, fallible IDs, preferred-scan integrity, asset durability, PDF hardening, migration digests, and the integrity report are reflected as implemented.
5. The development manifest is not represented as an official product design.
6. Synthetic fixtures and thresholds are not represented as physical calibration.
7. Milestone 8.6 is reconciled case by case and remains partial.
8. Milestone 9.2 records asset-backed change regions and stable reason/confidence availability.
9. Milestones 9.3–14 and physical/release work remain visibly incomplete.
10. The next product implementation block is Milestone 9.3.

## Maintenance rule

Update this table in the same commit that changes a fix’s completion state. A row may cite an integrated completion commit when a fix required several commits, but it must never cite an uncommitted review, response, or evidence file.
