# A2D Smart Notebook — Remediation Traceability Ledger

**Status:** Authoritative FIX-to-implementation ledger  
**Date:** 2026-07-31  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Remediation plan:** `docs/A2D_SMART_NOTEBOOK_CODE_REVIEW_FIX_TODO_2026-07-28.md`  
**Reconciled roadmap:** `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

This ledger satisfies FIX-121 by mapping every remediation fix ID to its affected roadmap area, representative production paths, focused tests/evidence, completion commit, and validation status. It also records incomplete fixes explicitly rather than treating the existence of nearby code as completion.

## Status vocabulary

- **Complete:** the required production behavior and focused evidence exist.
- **Partial:** meaningful implementation exists, but at least one stated acceptance requirement remains open.
- **Open:** the fix has not been executed as a bounded remediation task.
- **Superseded by roadmap work:** the remediation requirement correctly remains open because its implementation depends on a later product milestone.

## Validation baseline

The last code-bearing remediation candidate was `d2cb054d2489cf2b0f1e66d9370b5650b31404d0`.

- Permanent full CI: run `30673255456` — passed.
- Milestone 7 native/fixture validation: run `30673255457` — passed.
- Those runs covered the integrated code for completed FIX-001 through FIX-110, including Rust format/Clippy/tests, dependency policy, binding drift, Android lint/JVM tests/debug APK, both Android ABIs, APK symbol/notices checks, emulator integration, synthetic fixture drift, and Apple compile feasibility.
- Branch-cleanup commits after that candidate changed no production files.

## Traceability table

| Fix | Status | Affected milestone(s) | Representative production paths | Focused tests/evidence | Completion commit | Validation |
|---|---|---|---|---|---|---|
| FIX-001 | Complete | 1, 2, 9 | `crates/a2d-ffi/src/scan_comparison.rs`; `crates/a2d-ffi/src/milestone9.rs`; `apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt` | `crates/a2d-ffi/tests/binding_generation.rs`; Android binding-drift job | `55a8bd370b5f8781791548f675879b76b50d7ab9` | `30673255456` |
| FIX-002 | Complete | 1, 2, 15 | `README.md`; `apps/android/README.md`; `apps/ios/README.md`; `tools/generate-bindings.sh` | Binding regeneration/drift documentation and CI | `99c6b764dd811ff7bab370d28d186f5d6e5b7bb3` | `30673255456` |
| FIX-010 | Complete | 3, 9 | `crates/a2d-storage/src/workflow.rs`; `crates/a2d-storage/src/repository.rs`; migrations `0005`–`0008` | `crates/a2d-storage/tests/preferred_scan_migration.rs`; workflow tests | `c547bbfdf66ae7a131e386f2e448fd72fef940f6` | `30673255456` |
| FIX-011 | Complete | 3, 9 | Preferred-scan production call sites in storage/core registration | `crates/a2d-core/src/milestone9_tests.rs`; preferred-scan gate tests | `9c18f6765b93887d3d60ebed2f4893bd7b8af80b` | `30673255456` |
| FIX-020 | Complete | 3, 5, 9 | `docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md`; `crates/a2d-storage/src/assets.rs` | `crates/a2d-storage/tests/durability_documentation.rs` | `1c78f4aa7552b22788f23e729e16773b49575021` | `30673255456` |
| FIX-021 | Complete | 3 | `crates/a2d-storage/src/assets.rs`; asset finalization platform adapter | Asset collision/sync/permission/finalization regressions | `dc293354c27eaac3420cc50ba5e0fd9703672865` | `30673255456` |
| FIX-022 | Complete | 3, 9, 16 | Asset finalization failure records; orphan finalized-asset discovery; scan registration rollback context | `docs/FIX022_GENERIC_SCAN_REGISTRATION_ROLLBACK_EVIDENCE_2026-07-29.md`; orphan discovery tests | `8879980845344d16467f07b1ec85421381b78cf4` | `30673255456` |
| FIX-023 | Complete | 3, 5, 8 | Explicit temp/staging cleanup error handling in storage, PDF, QR, scanner paths | Cleanup failure tests and structured warning assertions | `f777098e2ff249ab27096848661d08cdcadef51d` | `30673255456` |
| FIX-024 | Complete | 3, 9, 16 | Canonical asset/path resolution in `crates/a2d-storage`; scanner staging containment | Path traversal, symlink, root containment and canonical-path tests | `fbbe13925416469f890b58a302400c0de7121722` | `30673255456` |
| FIX-030 | Complete | 7, 8, 9 | Rust-owned stored scan processing policy; layout registry resolution; `crates/a2d-core/src/milestone9.rs` | Core policy/layout mismatch and registration tests | `2ac991e9b69bcf5acd8b8df7d0c4a94ce7732906` | `30673255456` |
| FIX-031 | Complete | 8, 9 | Explicit Smart Page versus Notebook Page scanner scope in Rust/core and Android request projection | Scanner scope and unsupported-layout tests | `2bff6606558b5ebd441337187d519b938fbd4079` | `30673255456` |
| FIX-040 | Complete | 2, 16 | `crates/a2d-ffi/src/lib.rs` structured error projection | FFI ordered-details and generated binding tests | `6d64bc9636faad08c450c93d7748cdd505714ac4` | `30673255456` |
| FIX-041 | Complete | 2, 4, 6 | Fallible ID APIs in domain/core/FFI and Smart Page generation | RNG-failure and fallible projection tests | `4ceffab412f41d79c6ce5c76e3a6bfc833398a5b` | `30673255456` |
| FIX-042 | Complete | 2, 19 | `ffi-test-panic`-gated panic endpoint; production APK symbol exclusion | FFI panic containment test build; `tools/verify-android-apk.py` | `18f90fc17623b29b09e61ac8b4cb526703e5c408` | `30673255456` |
| FIX-043 | Complete | 2, 3, 6, 9 | Fallible portable clock and timestamp propagation | Clock-failure rollback and timestamp tests | `3ab520dabcecc276c44ec24320dd1eb42471dd31` | `30673255456` |
| FIX-044 | Complete | 4, 6, 16 | Registry lookup/parse failures preserve integrity/internal classification | Registry corruption and unsupported-data distinction tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-050 | Complete | 5, 6, 16 | Rust-owned Smart Page/PDF page and count policies | Smart Page request-limit and checked-range tests | `de953408b473e7e2461853fe377c7c04714f06ac` | `30673255456` |
| FIX-051 | Complete | 4, 16 | Bounded manifest JSON parsing, semantic limits, registry construction | Manifest size/count/duplicate/unsupported-field tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-052 | Complete | 4, 6, 16 | Explicit registry result propagation through core/FFI | Corrupt registry and missing/unsupported design tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-060 | Complete | 9, 16 | `crates/a2d-core/src/scan_comparison.rs`; verified corrected assets and stored fingerprints | Core tamper/missing/hash/fingerprint tests | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-061 | Complete | 9, 14 | `crates/a2d-ffi/src/scan_comparison.rs`; generated Kotlin binding | FFI projection and threshold-range tests; binding drift | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456` |
| FIX-070 | Complete | 7, 8, 9 | `crates/a2d-image/src/quality_calibration.rs`; `crates/a2d-core/src/milestone9.rs`; Android scanner policies/UI | Core registration quality tests; Android JVM and emulator presentation tests; `docs/FIX_070_QUALITY_CALIBRATION_COMPLETION_2026-07-31.md` | `d2cb054d2489cf2b0f1e66d9370b5650b31404d0` | `30673255456`; `30673255457` |
| FIX-080 | Complete | 6, 8 | `CoroutineFailures.kt`; Notebook and Smart Page ViewModels/screens | `CoroutineFailuresTest.kt` and ViewModel tests | `8ff1b53ee42bb52e3904d3812c893f2c695406ae` | `30673255456` |
| FIX-081 | Complete | 6, 8 | `apps/android/app/src/main/kotlin/com/a2d/notebook/rustbridge/QrCapture.kt` | Recreation, stale token, orphan listing and cleanup tests | `ebaf44a9b3309c49963c53ba53f665f2a87e0869` | `30673255456` |
| FIX-082 | Complete | 6 | `SmartPagesViewModel.kt`; `SmartPagesScreen.kt` | Saved-state pending-save, stale callback and bitmap ownership tests | `32695abb1c6a05a90c8eec554f652d489584c9fa` | `30673255456` |
| FIX-083 | Complete | 8 | `CameraXAdapter.kt` terminal `Closed(cleanupWarning)` state | `CameraAdapterTerminalStateTest.kt` | `5bfff4b26ef14a8e1ccf15779897792e86d3cc48` | `30673255456` |
| FIX-090 | Complete | 5 | `crates/a2d-pdf/src/generate.rs` | Existing-destination, corrupt/warning output, sync and cleanup tests | `abbb99d3a8691bd909068a15686b2fa2faa946e0` | `30673255456` |
| FIX-100 | Complete | 3, 16 | `crates/a2d-storage/src/migration_history.rs`; `migrations.rs` | `crates/a2d-storage/src/migration_history/tests.rs` | `716559d8c6757bb778bc953b010f19c150856be9` | `30673255456` |
| FIX-101 | Complete | 3, 16 | `crates/a2d-storage/src/integrity.rs`; core integrity façade | Integrity corruption, limits, cancellation, assets/orphans and relational invariant tests | `ebe0abfabf226de3410dd048d94089c6e38999e3` | `30673255456` |
| FIX-110 | Complete | 8, 9, 16 | `crates/a2d-core/src/scanner_recovery.rs`; FFI recovery projection; `SinglePageScannerViewModel.kt` | Core recovery tests; `ScannerRecoveryBridgeTest.kt`; registration/recreation tests | `bd46b76c324c184bf855932c54ae1463617ef425` | `30673255456` |
| FIX-111 | **Partial** | 8, 16, 19 | Existing CameraX, permission, state-machine, presentation and recovery paths | Permission, rotation, repeated capture, identity, recovery, torch and cleanup tests exist; consolidated matrix, batch ordering and real low-storage evidence remain open | — | Partial coverage in `30673255456` |
| FIX-120 | Complete | 1–19 | `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` | Source/evidence reconciliation recorded in this ledger | `c139b861280da9cd697d7f1f06971029e998e9f0` | Documentation-head CI pending at ledger creation |
| FIX-121 | Complete | Remediation-wide | This ledger | Table completeness review | `PENDING_CREATION_COMMIT` | Documentation-head CI pending at ledger creation |
| FIX-130 | **Partial** | 1, 3–9, 16, 19 | `.github/workflows/ci.yml`; permanent fixture/native/APK checks | Most listed invariants are permanent gates; deliberate regression proof for every repaired invariant is not yet consolidated | — | Current permanent CI is green for the code-bearing baseline |
| FIX-131 | Open | 19 | Final remediation head and CI evidence | Requires every remediation phase, not only FIX-120/121 | — | — |
| FIX-140 | Open | 2, 16 | Repository-wide Rust production audit | No complete classified audit ledger yet | — | — |
| FIX-141 | Open | 3, 5, 8, 16 | Repository-wide production filesystem cleanup audit | Many known paths are hardened, but full classified audit is not complete | — | — |
| FIX-142 | Open | 4–7, 9, 13, 14, 16 | Repository-wide arithmetic/allocation audit | Implemented paths have many checked limits; no complete audit ledger yet | — | — |
| FIX-150 | **Partial** | Architecture-wide | Decision docs, READMEs, specification, reconciled roadmap, this ledger | Major implemented contracts are documented; complete cross-document agreement remains open | — | — |
| FIX-151 | Open | Documentation/release | All Markdown path references; future validation script | No complete repository-local path audit/script yet | — | — |

## FIX-120 reconciliation conclusions

1. The old blanket “Milestones 1–6 complete” header was removed.
2. Implementation, partial completion, physical evidence, and not-implemented states are now distinct.
3. Checked-in Kotlin bindings and Swift smoke behavior are documented accurately.
4. Structured FFI details, fallible IDs, preferred-scan integrity, asset durability, PDF hardening, migration digests, and the integrity report are reflected as implemented.
5. The development manifest is not represented as an official product design.
6. Synthetic scan fixtures and thresholds are not represented as physical calibration.
7. Milestone 8.6 is reconciled case-by-case and remains partial.
8. Milestone 9.2 now records asset-backed changed-region comparison and stable reason/confidence availability.
9. Milestones 9.3–14 and the physical/release work remain visibly incomplete.
10. The next product implementation block is Milestone 9.3, not repeated remediation of already-complete paths.

## Maintenance rule

Update this table in the same commit that changes a fix’s completion state. A future row may cite an integrated completion commit when a fix required several small commits, but it must not cite an uncommitted review, response, or evidence file.
