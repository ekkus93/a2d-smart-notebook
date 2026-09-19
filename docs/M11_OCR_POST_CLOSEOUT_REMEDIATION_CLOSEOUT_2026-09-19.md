# M11 OCR Post-Closeout Remediation Closeout — 2026-09-19

This document is the final evidence and architecture record for the post-closeout remediation that followed the original Milestone 11 OCR closeout.

The remediation started from the post-closeout review baseline `da8a6528ce1a72c1612456bf643618b48b0bb2fc`. The current remediated baseline before this documentation slice is `89c7e27264643f53a0f83da0d061b56fe7989feb`, produced by PR #112 and verified by post-merge `master` CI run `35430897974`.

## Final status

M11 OCR production integration remediation is complete through R8 implementation and test hardening, with this R9/R10 documentation slice reconciling stale status text and final evidence. The remediated product status is:

- Production OCR Search uses the active-library Rust/FFI search path through production composition rather than the not-connected controller path.
- Production Page Viewer hydrates Rust-owned OCR job/result/region state and scan/display metadata.
- Production Start, Retry, and Cancel actions route through durable OCR queue APIs rather than no-op callbacks.
- OCR correction is reachable from production Page Viewer UI, persists through Rust/FFI, preserves immutable original OCR, and hydrates correction history on reopen.
- Detected OCR finalization is Rust-owned and transactionally coherent for run, required regions, FTS effects, and queue success.
- Cancellation/completion races follow one commit-point rule: cancellation wins before terminal commit; completion wins after a successful terminal commit.
- Overlay rendering requires authoritative source image geometry and uses the displayed image coordinate frame instead of polygon extrema.
- Rust rejects invalid persisted OCR geometry, including malformed, negative, non-finite, and out-of-bounds polygons.
- Android Page Viewer readback no longer silently requests only the old 50-region default as though the overlay were complete.
- OCR queue/status/retry policy is consolidated around the durable Rust queue owner and documented compatibility behavior.
- Production-path instrumentation now covers the wiring seams missed by the original closeout.

## Remediation PR chain

The following master commits were inspected through Ralph Bridge after each merge. Each listed commit is a guarded merge commit on `master`; CI was verified on the resulting exact master head during the loop.

| Area | PR | Resulting master SHA | Evidence |
|---|---:|---|---|
| R1 production OCR search/navigation | #73 | `45a9f67775e28353448fcc75d08964a61118dbcf` | Persisted OCR production navigation coverage and production route wiring. |
| R2 Page Viewer search-hit OCR hydration | #74 | `cb0cf25a551e6cdda1f99a0071a61f4bbf51675c` | Page Viewer OCR hydration from search navigation. |
| R2 durable OCR actions | #75 | `d904d40874550728470df9516bc2b35ef4ea8d49` | Start/Retry/Cancel production queue action wiring. |
| R2 action qualification | #76 | `461ac7467aca4cd3d36d40f429073097e9216d2c` | Page Viewer OCR queue action tests. |
| R2 readback-state qualification | #78 | `eba768dd9622fd594817f616469b84a104c9609f` | Page Viewer OCR readback state tests. |
| R2 active queue hydration | #79 | `8c61837d4ee68f7fe8b1de85329fab7a7dd22a43` | Active OCR queue state hydration by scan identity. |
| R2 region hydration | #80 | `cff86337c61e4479e2cb5b499dd73c33b171470d` | OCR region hydration from Rust readback. |
| R2 Rust-owned metadata hydration | #81 | `772e4e79068a9d42eaa55f00328d2de68b34ac31` | Page Viewer snapshot/display asset metadata hydration. |
| R3 correction workflow | #84 | `001a86df7ba1acb41b1cfe7f3fd38330b9df04c5` | Production OCR correction workflow, controller wiring, history/readback tests, and Rust-derived previous text. |
| R4 transactional finalization | #85 | `23f324675fdf23d23cb11f6c3d1bac1733c2f5df` | Rust transactional OCR job finalizer core. |
| R5 cancellation races | #86 | `76adf76f18d3b9b757f0aa1204c41158ad36678b` | Cancellation commit-point race tests. |
| R6 overlay/source geometry | #94 | `b5c3e8fb30dcc9ea60997aafd9d3f913a02244eb` | Source-geometry overlay requirement, region-limit removal, and geometry UI/unit tests. |
| R7 inventory | #96 | `441af709cfdaead627147f4d5cf923d581f9640a` | Queue policy duplication inventory. |
| R7 policy selection | #99 | `74e83147eda3594b63db7af8b044c3662538cd88` | Canonical OCR queue retry policy documentation. |
| R7 legacy policy deprecation | #100 | `cd70dda3865c34328b4fbce1d059eb93b18e30bd` | Adapter retry compatibility alignment. |
| R7 schema compatibility | #103 | `735c7780f8465421b0e3a8a145cd74ab287828fa` | Legacy OCR queue schema compatibility evidence. |
| R7 retry policy correction | #102 | `d90c2f675266b0704c66a19d51d8fe402f2f671d` | Canonical retry compatibility follow-up. |
| R7 database compatibility docs | #106 | `23ba85c2feaca7cc3ea509dd2b663cfaf318de23` | Queue database compatibility record. |
| R7 canonical retry behavior | #107 | `8f9fa7fc4876a7b8c8ecb9b259958fbe52a82b3e` | Canonical OCR retry behavior tests. |
| R7 manual retry diagnostics | #111 | `17247af3d219227d910f4de3976c56e7ba0cc5d0` | Rust-owned OCR attempt diagnostics and Android diagnostics tests. |
| R8 production integration hardening | #112 | `89c7e27264643f53a0f83da0d061b56fe7989feb` | Full production OCR navigation scenario and composition sentinels; PR-head CI run `35429371063`, post-merge master CI run `35430897974`. |

## Final architecture decisions

### Transactional finalization

Rust owns the terminal OCR finalization boundary. A claimed OCR attempt is finalized through one Rust transaction that validates job/input identity, records the terminal run, persists required detected regions, lets FTS effects commit or roll back with the transaction, and resolves queue success/retry/terminal status coherently. A detected result is not considered durable success solely because an OCR run row exists.

### Cancellation commit point

Cancellation wins until the terminal OCR finalization transaction commits. After a successful terminal commit, completion wins and a later cancellation request must not rewrite accepted searchable OCR to `Cancelled`. Race coverage exercises cancellation before provider work, during provider work, after provider return but before commit, racing with commit, and after commit.

### Queue/status/retry owner

The durable Rust OCR queue is the canonical owner of job state transitions, retry eligibility, attempt diagnostics, and bounded retry policy. Compatibility definitions outside that owner are documented or deprecated rather than treated as competing runtime policy.

### Overlay coordinate convention

Persisted OCR polygons are interpreted in the authoritative source image coordinate frame. Rendering and hit testing require explicit source image dimensions and use the same content-fit transform as the displayed page/scan image. Polygon extrema are not a valid substitute for source dimensions.

### Region hydration policy

Page Viewer OCR region hydration uses the Rust-owned bounded maximum rather than the previous Android default that silently requested only 50 rows. If a future UI chooses partial hydration, it must expose partial/truncated state explicitly instead of presenting a complete overlay.

### Correction provenance

Original OCR remains immutable. Corrections are separate durable records with provenance. Rust derives prior/original correction context from durable OCR/correction records where possible rather than trusting arbitrary caller-supplied text.

## R9 stale text reconciliation

The stale status fixed by this closeout is the original post-closeout state that said M11 was fully closed while production composition and consistency defects remained. Current status should be read as: M11 OCR implementation and post-closeout production integration remediation are complete through PR #112; broader release/physical OCR quality, corrected+original unified ranking, WorkManager/background guarantees, and Milestone 12 search scale remain deliberately out of scope.

## Final validation rule

This documentation closeout must itself pass exact-head CI, merge through the guarded Ralph Bridge path, and be followed by a green post-merge `master` CI run before the Ralph Loop may stop. That final merge/CI evidence is intentionally verified after this file is merged, not assumed by this pre-merge document.
