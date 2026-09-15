# Milestone 10 — Home dashboard slice

**Status:** Implemented and validated  
**Date:** 2026-09-12  
**Scope:** First bounded Milestone 10 Library/page-presentation slice  
**Code head:** `2ad9acb6a696760bd36dce52387970d1a621de68`  
**Validation:** permanent CI run `34718419554` passed for the code head.

## Implemented behavior

This slice replaces the previous placeholder Home surface with a reusable local-first dashboard presentation model. At the time this slice was merged it intentionally did not invent persistence, search, OCR, backup, restore, collection, or trash backends that were then future roadmap work. OCR and local OCR search were subsequently implemented under Milestone 11; backup/restore, collection, and trash work remain governed by their current roadmap status.

The Home screen now exposes:

- Local-first/accountless copy: core workflows do not require an account or A2D server.
- Primary actions for Scan a Page, Batch Scan, Notebooks, Create Smart Pages, and Import.
- Empty-library guidance that points users to local library creation paths and manual backup/restore.
- Populated-library state with recent Notebook cards.
- Dashboard slots for unfinished scans, Needs Review count, generated Smart Pages, and backup status.
- Reusable `HomeDashboardState`, `HomeNotebookSummary`, and `HomeBackupStatus` types so later Rust-backed ViewModel work can supply real persisted counts without changing the Compose surface contract.

## Evidence

Permanent CI run `34718419554` passed on exact code head `2ad9acb6a696760bd36dce52387970d1a621de68` with:

- Rust format, strict Clippy, full Rust tests, and real asset ENOSPC evidence.
- cargo-deny dependency/license policy.
- Kotlin UniFFI generated binding drift check.
- Android native ABI build, scanner-staging ENOSPC evidence, lint, JVM tests, debug APK, APK verification, and artifact upload.
- Android emulator instrumentation including `HomeScreenUiTest`.

## Explicitly not completed by this slice

Milestone 10 remains partial. The following stay open:

- Data-backed Home ViewModel wired to Rust library summaries.
- Library hub for Notebooks, Smart Pages, Page Sets, Collections, imports, Needs Review, and Trash.
- Notebook detail with logical page slots and no scan-order renumbering.
- Smart Page/Page Set/Collection browsing.
- Page viewer completeness across all original/corrected/text/split/metadata/versions/annotations/related/skill-result surfaces; later milestones have added some Page Viewer OCR/version capabilities without closing the whole Milestone 10 surface.
- Trash, restore, and permanent-delete flows.
