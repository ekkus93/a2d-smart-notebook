# Milestone 10 — Library and page presentation closeout

Date: 2026-09-13
Repository: `ekkus93/a2d-smart-notebook`
Scope: v0.1 Android/Kotlin presentation surfaces over Rust-owned local data and workflow boundaries.

## Status

Milestone 10's Android presentation shell is implemented for the v0.1 visible library surface. The milestone is not a claim that OCR, search, backup/restore/export, generated-content listing persistence, import merge, or destructive trash lifecycle backends are complete. Those remain owned by their later milestones and by Rust APIs that must be added before Kotlin may display durable rows or perform mutations.

The completed Milestone 10 work intentionally keeps Kotlin as presentation/platform integration and keeps Rust authoritative for identity, persistence, review, asset ownership, restore/merge/destructive mutation policy, and future search/OCR/backup semantics.

## Landed slices and evidence

| Slice | PR | Exact green head | CI run | Result |
|---|---:|---|---:|---|
| Home/dashboard presentation | #32 | `095706c10f834e1fc9c17cac74c35e4179952a80` | `34719101740` | Passed full CI, including emulator instrumentation |
| Library hub presentation | #33 | `581ac4ed9f2a92c1c07c9778187414b3b62cd2c5` | `34735816808` | Passed full CI, including emulator instrumentation |
| Notebook detail presentation | #34 | `b9789c53a1d5053b9db3ff85429db08b4df1cfea` | `34741256017` | Passed full CI, including emulator instrumentation |
| Page browser presentation | #35 | `f59b9046c38b0f1ba24bd75e7da30f1f6284b49a` | `34743873826` | Passed full CI, including emulator instrumentation |
| Needs Review queue presentation | #36 | `99aecf6d0fdf50628780db5953719e2fa0569f06` | `34744586325` | Passed full CI, including emulator instrumentation |
| Smart Page/Page Set/Collection browser | #37 | `c62c13a5e48171e4772d3ce0fe8e9f58aa18df12` | `34745854995` | Passed full CI, including emulator instrumentation |
| Page viewer presentation | #38 | `67d72939a21aa1418f52b6af3e9501aa8978b224` | `34746984904` | Passed full CI, including emulator instrumentation |
| Trash workflow presentation | #39 | `ab88f6fc92ef65ff5433fe268a822d80aea87d3e` | `34760163147` | Passed full CI, including emulator instrumentation |
| Imports presentation | #40 | `72b5e29a30fe1860b8cc33358ed734f7156ec075` | `34772382945` | Passed full CI, including emulator instrumentation |

## Implemented user-visible surfaces

- Home/dashboard: local-first empty/populated state, primary actions, recent Notebook presentation, review count, unfinished-scan count, Smart Page count, backup-status placeholder, and library entry point.
- Library hub: visible destinations for Notebooks, Smart Pages, Pages, Page Sets, Collections, Imports, Needs Review, and Trash.
- Notebook detail: Notebook identity/status/actions with explicit Rust-owned logical page-slot boundary and no scan-order renumbering.
- Page browser: page-row presentation state, summary counts, open-page and open-versions actions, and explicit boundary that durable page rows must come from Rust.
- Page viewer: original/corrected/text/split/metadata/versions/annotations/related/skill-result sections with explicit unavailable/boundary states until Rust-backed records exist.
- Needs Review queue: local-first queue presentation, counts, item rows, defer/resolve/open-version callbacks, and explicit Rust review API ownership.
- Generated content library: Smart Page, Page Set, and Collection presentation with immutable identity/no-ID-reuse copy and explicit Rust generated-content API boundary.
- Trash: consequence-aware restore/permanent-delete presentation with explicit Rust ownership of deletion state, asset retention, audit events, and ID retirement.
- Imports: presentation for inspected/imported rows, source summaries, conflict counts, retry/review/open-page callbacks, and explicit Rust ownership of traversal/resource/conflict handling.

## Explicit non-claims

The following are not completed by Milestone 10 and must remain unchecked in their owning milestones:

- OCR provider, persistent OCR queue, OCR correction UI, OCR provenance persistence, and OCR failure handling beyond visible page-viewer placeholders.
- Rust FTS/search schema, search API, search result UI, excerpt generation, source-region navigation, and scale evidence.
- Manual `.atnb` backup, restore, merge restore, export formats, archive traversal limits, encryption, and backup compatibility fixtures.
- Rust-generated durable APIs for full page summaries, generated-content lists, import merge/commit, trash restore/permanent delete, and collection mutation.
- Model-provider configuration, skills, skill permissions, run history, citations, and provider-not-configured behavior beyond visible page-viewer placeholders.
- Product-wide accessibility/manual acceptance and physical print/camera validation.

## Permanent CI coverage now extended

The Android emulator instrumentation class list now includes presentation coverage for:

- `HomeScreenUiTest`
- `LibraryHubUiTest`
- `NotebookDetailUiTest`
- `PageBrowserUiTest`
- `PageViewerUiTest`
- `ImportLibraryUiTest`
- `TrashWorkflowUiTest`
- `NeedsReviewHubUiTest`
- `SmartPageLibraryUiTest`

Those tests run alongside the existing scanner, recovery, FFI panic-containment, scan-revision, Needs Review bridge, and version-history instrumentation tests. The standard workflow also continues to run Rust format, Clippy, workspace tests, cargo-deny, real asset ENOSPC evidence, Android native/binding generation, real scanner-staging ENOSPC evidence, Android lint/unit/APK verification, APK native-library/symbol/notices verification, and generated Kotlin binding drift checks.

## Follow-on roadmap after Milestone 10

The next software-only implementation area is Milestone 11 OCR. Milestone 7/17 physical photographed fixtures and calibration remain blocked on real device/printer evidence. Milestone 12 search depends on OCR/page text persistence. Milestone 13 backup/restore/export is a larger Rust-owned archive/encryption/restore workflow and should not be faked from Kotlin presentation screens.
