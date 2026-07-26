# A2D Smart Notebook v0.1 — Product and System Specification

**Status:** Initial implementation specification  
**Version:** 0.1  
**Date:** 2026-07-26  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Primary client:** Android  
**Future client:** iOS  
**Shared core:** Rust  

---

## 1. Purpose

This document defines the first implementable version of **A2D Smart Notebook**, a local-first system that converts handwritten paper notes into durable, searchable digital records.

The system includes:

- **A2D Smart Notebooks:** purpose-designed, print-on-demand physical notebooks.
- **A2D Smart Pages:** uniquely identified loose pages generated as print-ready PDFs.
- **The A2D Android app:** the first user-facing client.
- **The A2D Rust core:** the authoritative data and business-logic implementation shared by Android and a future iOS application.
- **A2D Skills:** permissioned deterministic and LLM-assisted operations over user-owned notebook data.
- **Manual backup and restore:** included in the core product without an account.
- **A2D Sync:** a future optional paid service for automatic cloud backup and multi-device synchronization.

The implementation TODO is in `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`. This document and that TODO are a self-contained implementation handoff.

Normative words such as **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are intentional requirements.

---

## 2. Product summary

A user writes with an ordinary pen or pencil in a physical notebook or on an A2D Smart Page. Each scannable page contains four printed alignment markers based on AprilTags, one QR Page Code, a visible page number where appropriate, and a known page-layout definition.

The mobile application detects the markers and Page Code, validates capture quality, preserves the original image, rectifies the page geometry, creates processed derivatives, performs OCR, and stores all results in a local library.

The user can then:

- Browse notebooks and loose pages.
- Search OCR and corrected text.
- Preserve multiple scans of a changing physical page.
- Generate uniquely identified printable pages.
- Export their data.
- Back up and restore the entire library without creating an account.
- Optionally use local or user-configured LLMs through permissioned A2D Skills.
- Optionally subscribe to a future A2D Sync service.

---

## 3. Product principles

### 3.1 Local-first core

The core application MUST work without a user account, an A2D server, an Internet connection, a cloud AI provider, or a paid subscription.

Scanning, saving, browsing, basic OCR, basic search, exporting, manual backup, and manual restore MUST remain available without an account.

### 3.2 User ownership

The user MUST be able to export original images, processed images, OCR text, corrected text, metadata, tags and annotations, skill results, page and notebook identities, and version history.

A subscription ending MUST NOT make locally stored notes inaccessible.

### 3.3 Original evidence is immutable

The original captured or imported image MUST be preserved as an immutable asset. Processing, OCR, user correction, and LLM interpretation are derived data and MUST NOT overwrite the source asset.

### 3.4 No silent failure or unsafe fallback

The application MUST NOT:

- Silently assign a page to a conflicting notebook.
- Silently discard a previous scan.
- Silently downgrade an encrypted backup to an unencrypted backup.
- Silently treat failed OCR as empty authoritative text.
- Silently treat an LLM answer as source truth.
- Silently upload notebook data.
- Silently recover from database corruption by deleting data.
- Silently replace an unsupported page layout with a different layout.

Every degraded result MUST carry an explicit status, warning, or required user action.

### 3.5 Rust is authoritative

The shared Rust core MUST own canonical data, persistence, domain rules, portable formats, and platform-independent behavior. Kotlin and future Swift code MUST NOT duplicate domain rules.

### 3.6 Platform-native presentation

Android MUST use Kotlin and Jetpack Compose for presentation and Android integration. A future iOS client SHOULD use Swift and SwiftUI over the same Rust core.

### 3.7 LLMs are optional enhancements

The application MUST remain useful without an LLM. LLMs MAY enhance retrieval, summarization, extraction, and skill orchestration, but MUST NOT be required to capture, identify, save, browse, export, back up, or restore pages.

---

## 4. Goals

Version 0.1 MUST establish:

1. A shared Rust core that compiles independently of Android.
2. Generated Kotlin bindings for a narrow Rust application API.
3. An Android Compose application.
4. A local SQLite-backed library owned by Rust.
5. A versioned notebook/page identity model.
6. Offline generation and parsing of unique A2D Smart Page QR codes.
7. PDF generation for single pages and numbered page sets.
8. Registration of physical A2D Smart Notebooks.
9. Single-page and batch scanning of recto-only notebook pages.
10. Four-marker detection and page rectification.
11. Original and processed image preservation.
12. Explicit capture-quality results and review states.
13. Scan versioning and duplicate handling.
14. OCR through a provider adapter.
15. Basic full-text search.
16. Browsing of notebooks, pages, page sets, and collections.
17. Manual encrypted backup and restore.
18. Export of selected data.
19. A permissioned skills architecture with a small built-in skill set.
20. An LLM/provider abstraction that does not require A2D infrastructure.
21. Architecture and test fixtures suitable for a future iOS client.

---

## 5. Non-goals for v0.1

The following are explicitly deferred:

- A public community skill marketplace.
- A2D-managed cloud AI.
- A2D Sync implementation.
- Required user accounts.
- A web application.
- Collaborative editing.
- Real-time shared notebooks.
- Mathematical handwriting recognition.
- Reliable semantic interpretation of arbitrary diagrams.
- Variable-data printing of unique identifiers inside individual Amazon KDP copies.
- Scanning the verso/left-hand side of KDP notebook leaves.
- Full book-spine dewarping.
- A production iOS UI.
- Automatic creation of external tasks, calendar events, emails, or messages without explicit review.
- Arbitrary shell, filesystem, or database access for skills.

---

## 6. User-facing terminology

| Internal concept | User-facing term |
|---|---|
| Product/application | A2D Smart Notebook |
| Bound physical product | A2D Smart Notebook |
| Printable loose sheet | A2D Smart Page |
| Multiple generated pages | Page Set |
| Notebook edition/layout SKU | Notebook Design |
| Unique physical notebook copy | Notebook |
| Setup-page QR code | Setup Code |
| Per-page QR code | Page Code |
| AprilTags | Corner Markers |
| Unresolved scan queue | Needs Review |
| Optional paid cloud service | A2D Sync |
| Skill execution record | Skill History |

“KDP” and “AprilTag” are implementation terms and SHOULD NOT appear in ordinary user-facing workflows.

---

## 7. Primary user workflows

### 7.1 First launch

1. Explain that the library is stored locally and no account is required.
2. Request camera permission when scanning is first invoked, not before it is needed.
3. Create a new local library or restore a backup.
4. Show an empty home screen with Scan a Page, Add a Notebook, Create Smart Pages, and Import.

### 7.2 Add an A2D Smart Notebook

1. User selects **Add a Notebook**.
2. User scans the notebook Setup Code.
3. App resolves the Notebook Design.
4. User names this physical notebook.
5. Rust creates a unique local `NotebookId`.
6. Notebook becomes the active scan destination.
7. User may scan the first page.

### 7.3 Scan one notebook page

1. User opens a registered notebook.
2. User starts the scanner.
3. UI clearly shows the active notebook.
4. Live analysis detects all four Corner Markers and the Page Code.
5. App validates design, logical page number, focus, blur, glare, and framing.
6. App automatically captures only when required thresholds pass, unless the user explicitly chooses manual capture.
7. Rust rectifies and registers the scan.
8. User sees a corrected preview, warnings, and required actions.
9. OCR and indexing may continue asynchronously.

### 7.4 Batch scan

1. User selects a notebook and starts batch mode.
2. Each accepted page is saved without leaving the camera.
3. Pages may be scanned out of order.
4. Duplicate page IDs are detected.
5. Session completion shows accepted, warning, rejected, and review-required counts.

### 7.5 Generate one A2D Smart Page

1. User chooses paper size, orientation, and style.
2. Rust generates a globally unique page identity offline.
3. Rust creates a PDF containing the four Corner Markers, unique Page Code, layout, and optional visible title/page number.
4. The page appears in the library with `GeneratedNotScanned` state.
5. User prints, saves, or shares the PDF.

### 7.6 Generate a Page Set

1. User selects page count, starting visible number, paper size, and style.
2. Rust creates one `PageSetId` and one unique `PageId` per page.
3. Rust creates a multipage PDF.
4. Every page appears in the library before scanning.

### 7.7 Scan a generated Smart Page

The unique Page Code identifies the page without an active notebook. The app resolves the local record or creates an imported stub if the page originated on another installation.

### 7.8 Import images or PDFs

The app analyzes imported assets, identifies pages where possible, groups unresolved items, and never guesses a conflicting notebook silently.

### 7.9 Rescan an existing page

A new scan of an existing page is compared with previous scans and classified as near duplicate, better-quality replacement candidate, newer revision with additional writing, substantially different, or ambiguous. The default safe action is to preserve the new scan as another version.

### 7.10 Search and ask

Basic search operates over local text indexes without an LLM. “Ask My Notes” retrieves a bounded set of pages and produces an answer with source-page citations through a configured model.

“Ask My Notes” is a Search UI surface backed by a built-in, non-removable A2D Skill running through the same permissioned skill runtime as any other model-backed skill (§21). It is not a parallel, ungoverned LLM path: scope, effective permissions, provider/endpoint disclosure, prompt-injection separation, citations, and audit all apply exactly as they do to other skills.

### 7.11 Backup and restore

Manual backup and restore are core features. Automatic cloud backup and cross-device synchronization are reserved for the future paid A2D Sync service.

---

## 8. Architecture overview

```text
┌──────────────────────────────────────────────────────────────┐
│ Android application                                         │
│ Kotlin + Jetpack Compose                                     │
│ Navigation, ViewModels, CameraX, permissions, print/share,   │
│ Android file picker, WorkManager, secure platform adapters   │
└───────────────────────────┬──────────────────────────────────┘
                            │ typed UniFFI boundary
┌───────────────────────────▼──────────────────────────────────┐
│ A2D Rust core                                                │
│ Domain services, SQLite, identities, QR protocol, layouts,   │
│ PDF generation, scan registration, image processing policy, │
│ search, backup/restore, export, skills, model orchestration  │
└───────────────────────────┬──────────────────────────────────┘
                            │ future generated Swift bindings
┌───────────────────────────▼──────────────────────────────────┐
│ Future iOS application                                      │
│ Swift + SwiftUI, AVFoundation, Files, printing, Keychain     │
└──────────────────────────────────────────────────────────────┘
```

UniFFI is the initial binding strategy because it generates Kotlin and Swift bindings from a Rust interface. The build MUST pin a reviewed UniFFI version and MUST include Kotlin binding generation in CI. Swift binding generation MUST also run in CI as an iOS-readiness check even before an iOS app exists.

---

## 9. Responsibility boundaries

### 9.1 Rust core responsibilities

Rust MUST own:

- Domain types and invariants.
- Identifier generation and validation.
- QR payload parsing and encoding.
- Notebook Design manifests.
- Page layout definitions.
- SQLite schema and migrations.
- Transactions and integrity checks.
- Asset metadata and content hashes.
- Scan registration and state transitions.
- Page version and duplicate classification.
- Search indexing and queries.
- Backup archive creation, validation, restore, and merge.
- Portable export formats.
- Skill manifests, permissions, execution records, and proposals.
- Model-provider capability abstractions.
- Prompt assembly and retrieved-source boundaries.
- Audit records and provenance.
- Future sync object/revision definitions.

### 9.2 Android responsibilities

Kotlin/Android MUST own:

- Jetpack Compose screens and navigation.
- CameraX lifecycle and camera controls.
- Runtime permissions.
- Live camera preview and overlays.
- Android file and document pickers.
- Print framework and share sheet.
- WorkManager scheduling.
- Notifications.
- Android Keystore integration.
- Android OCR adapter, initially ML Kit Text Recognition where selected.
- Platform network and connectivity status.
- Mapping typed Rust results into presentation state.

Kotlin MUST NOT directly modify the canonical SQLite database.

### 9.3 Future iOS responsibilities

Swift/iOS SHOULD own SwiftUI presentation, AVFoundation camera integration, Files and Photos integration, printing and sharing, background tasks, Keychain storage, and iOS OCR/model adapters.

### 9.4 FFI boundary rules

The mobile-facing Rust API MUST:

- Expose use cases, not tables or SQL.
- Use typed requests and typed results.
- Return structured errors and warnings.
- Avoid panics crossing FFI.
- Avoid large images serialized as JSON or Base64.
- Use file paths, owned binary buffers for occasional payloads, or native-buffer adapters.
- Support cancellation for long operations.
- Make thread-affinity requirements explicit.
- Preserve backwards compatibility within a released app version.

Appropriate operations include `open_library`, `create_notebook`, `list_notebooks`, `generate_page_set_pdf`, `analyze_captured_page`, `register_scan`, `search_pages`, `create_backup`, `inspect_backup`, `restore_backup`, `run_skill`, and `approve_skill_proposal`.

The API MUST NOT expose generic SQL, arbitrary file access, shell execution, or unrestricted permission grants.

---

## 10. Recommended repository structure

```text
a2d-smart-notebook/
├── Cargo.toml
├── rust-toolchain.toml
├── crates/
│   ├── a2d-domain/
│   ├── a2d-identity/
│   ├── a2d-layout/
│   ├── a2d-storage/
│   ├── a2d-image/
│   ├── a2d-pdf/
│   ├── a2d-search/
│   ├── a2d-ocr/
│   ├── a2d-model/
│   ├── a2d-skills/
│   ├── a2d-backup/
│   ├── a2d-export/
│   ├── a2d-sync-model/
│   ├── a2d-core/
│   └── a2d-ffi/
├── apps/
│   ├── android/
│   └── ios/
├── fixtures/
│   ├── qr/
│   ├── page-layouts/
│   ├── scans/
│   ├── ocr/
│   ├── backups/
│   └── databases/
├── tools/
├── docs/
│   ├── A2D_SMART_NOTEBOOK_V01_SPEC.md
│   ├── A2D_SMART_NOTEBOOK_V01_TODO.md
│   └── decisions/
└── .github/workflows/
```

Crates MAY be consolidated early if build overhead becomes excessive, but responsibility boundaries MUST remain explicit. The FFI crate MUST remain thin.

`docs/decisions/` holds architecture decision records (ADRs) for choices this specification leaves open (QR wire encoding, native marker detector selection, and similar). Architecture decisions MUST be recorded there, not only in session notes or commit messages.

---

## 11. Physical A2D Smart Notebook specification

### 11.1 Print model

The initial bound notebook is expected to be printed on demand from a common interior PDF. All physical copies of a Notebook Design therefore contain the same Setup Code and same per-page codes.

A printed code identifies Notebook Design, logical notebook page number, and page layout. It does not identify the individual physical copy. The app creates a unique `NotebookId` when the user registers a physical copy.

### 11.2 Recto-only design

Only right-hand/recto pages are usable and scannable.

The interior alternates:

```text
Recto: setup or writable A2D page
Verso: blank
Recto: writable A2D page
Verso: blank
```

Requirements:

- Corner Markers appear only on usable recto pages.
- Verso pages remain blank in v0.1.
- The gutter-side writable exclusion zone is larger than outer margins.
- The scanner assumes the gutter is on the left.
- User-facing page numbers are logical notebook page numbers, not manuscript PDF page numbers.
- Important writing areas MUST NOT extend into the expected spine-curvature zone.

### 11.3 Setup page

The setup page MUST include A2D Smart Notebook branding, Notebook Design name and version, a Setup Code, short accountless registration instructions, and optional handwritten fields for notebook name, owner, and start date.

The Setup Code resolves a versioned Notebook Design manifest.

### 11.4 Writable page

Each writable page MUST include four Corner Markers with fixed semantic positions, a Page Code containing Notebook Design and logical page number, a visible logical page number, a writable content rectangle, a quiet zone around all machine-readable markers, and no critical content in printer trim-risk regions.

### 11.5 Physical validation

No layout is release-ready until tested with physical author/proof copies under several phones, bright/dim/uneven lighting, pencil/ballpoint/gel/common marker writing, pages near the front/middle/back, typical hand shadows, moderate camera angles, and normal print/trim variation.

Physical validation results MUST be recorded as test fixtures and release evidence.

---

## 12. A2D Smart Page PDF generation

### 12.1 Supported outputs

v0.1 MUST support single unique pages and numbered page sets in US Letter and A4 portrait formats with blank, lined, dot-grid, and graph styles.

### 12.2 Identity

Every generated Smart Page MUST receive a cryptographically random 128-bit identifier generated locally. No server allocation is required.

Printing the same PDF more than once produces multiple physical copies with the same page identity. The scan workflow MUST detect significantly different content under the same identity and ask whether the scan is a revision or another physical copy.

### 12.3 PDF contents

Every generated page MUST contain four Corner Markers, a unique Page Code, visible page number when requested, optional page-set title, known layout identifier, printer-safe margins, a calibration reference, and an “Actual Size / 100%” print instruction where appropriate.

### 12.4 Generated state

Generated pages exist in the database before scanning and use explicit states such as `GeneratedNotScanned`, `Scanned`, `NeedsReview`, `Archived`, and `Trashed`.

---

## 13. Identifier model

All identifiers MUST be immutable and MUST never be reused.

Every independently persisted entity that has its own row, can be referenced by another record, appears in provenance, or can cross the FFI boundary MUST use a dedicated opaque identifier type rather than a raw string or another entity's identifier type. Embedded value objects with no independent persistence or identity MUST NOT receive their own identifier type.

Core types include:

```text
InstallationId
NotebookDesignId
NotebookId
PageId
PageSetId
SmartPageId
PhysicalCopyId
ScanId
AssetId
OcrRunId
TextRegionId
TextCorrectionId
CollectionId
AnnotationId
ReviewItemId
SkillId
SkillRunId
AuditEventId
BackupId
```

### 13.1 Installation identity

On first library creation, Rust generates an `InstallationId`, device/library signing identity where needed, and library cryptographic material. This is not a user account and contains no required personal information.

### 13.2 Notebook identity

`NotebookDesign + logical page number` identifies the printed template page. `NotebookId + logical page number` identifies the page within one registered physical notebook.

### 13.3 Smart Page identity

A generated Smart Page has its own globally unique `SmartPageId`. Collection membership and ordering are mutable metadata and MUST NOT change the page identity.

---

## 14. QR protocol

### 14.1 Requirements

The QR protocol MUST be versioned, compact, offline-decodable, free of private user content, strictly parsed, forward-compatible through explicit unsupported-version errors, protected by an integrity check, and non-executable.

### 14.2 Code types

v0.1 defines `NotebookSetup`, `NotebookPage`, and `SmartPage`.

A conceptual representation is:

```text
a2d:1:n:<design-id>:<integrity>
a2d:1:b:<design-id>:<logical-page>:<layout-id>:<integrity>
a2d:1:p:<smart-page-id>:<layout-id>:<visible-page>:<page-set-id-or-zero>:<integrity>
```

The implementation MAY use a compact binary encoding before QR rendering, but the canonical Rust model and test vectors MUST be documented and stable.

### 14.3 Parser behavior

The parser MUST reject unknown required fields, invalid lengths, invalid alphabets, unsupported versions, invalid integrity checks, out-of-range page numbers, oversized payloads, and trailing executable or URL content.

The parser MUST return a typed error and MUST NOT reinterpret an invalid A2D code as arbitrary web content.

### 14.4 Trust

v0.1 integrity checks detect corruption but do not prove publisher authenticity. The design MUST leave room for signed Notebook Design manifests and signed official design codes later.

---

## 15. Domain model

### 15.1 NotebookDesign

```text
id
schema_version
name
design_version
trim_size
logical_page_count
setup_layout_id
page_layout_id
marker_family
marker_role_ids
manifest_hash
trust_state
```

### 15.2 Notebook

```text
id
design_id
display_name
created_at
updated_at
archived_at
active_scan_destination
optional_color
optional_icon
optional_user_notes
```

### 15.3 Page

A page is one logical physical page identity.

```text
id
kind: NotebookPage | SmartPage
notebook_id?
notebook_design_id?
logical_page_number?
smart_page_id?
page_set_id?
visible_page_number?
layout_id
title
state
preferred_scan_id?
created_at
updated_at
```

### 15.4 PhysicalCopy

Used only when multiple physical printouts share one Smart Page QR identity.

```text
id
page_id
copy_index
created_at
display_label?
```

### 15.5 Scan

```text
id
page_id
physical_copy_id?
capture_source
captured_at
original_asset_id
corrected_asset_id?
ocr_asset_id?
thumbnail_asset_id?
pipeline_version
quality_status
warnings
preferred
supersedes_scan_id?
content_fingerprint
```

### 15.6 Asset

```text
id
kind
relative_path
media_type
byte_length
sha256
created_at
immutable
encryption_state
```

### 15.7 OCR and correction

OCR runs, text regions, and text corrections MUST preserve provider/version, polygons, confidence where available, source region, correction history, and timestamps.

### 15.8 Collection and page set

A generated Page Set is a creation relationship. A Collection is mutable organization. Moving a page between collections MUST NOT change its QR identity.

### 15.9 ReviewItem

A review item records kind, related page/scan, severity, status, details, resolution, and timestamps.

### 15.10 Provenance and audit

Any derived output MUST identify source page/scan IDs, producing component, component/model version, timestamp, warnings, and user approval state where applicable.

---

## 16. Persistence and transactions

### 16.1 SQLite ownership

The Rust core owns the SQLite database, migrations, transactions, and integrity checks. Kotlin MUST NOT use Room for canonical A2D data.

### 16.2 Database expectations

The implementation MUST enable and verify foreign keys, use a reviewed mobile journaling mode, use explicit transactions, version schemas, identify migrations, support backup-safe snapshots, and run appropriate integrity checks.

### 16.3 Asset commit protocol

A scan registration that writes files and database rows MUST use a recoverable protocol:

1. Write asset to a temporary file.
2. Flush and close.
3. Compute and verify hash.
4. Atomically rename into the asset repository.
5. Commit database references in one transaction.
6. Record incomplete/orphan cleanup work if interrupted.

The system MUST NOT commit a database row pointing to an asset that was never durably written.

### 16.4 Corruption handling

If corruption is detected, stop writes where necessary, preserve existing files, report a blocking actionable error, offer verified backup/restore or diagnostic export paths, and never “repair” by silently dropping rows or recreating an empty library.

---

## 17. Image capture and processing

### 17.1 Split between platform and core

Android owns live camera acquisition. Rust owns portable analysis policy and final scan processing.

### 17.2 Live-analysis flow

1. CameraX provides reduced-resolution luminance frames.
2. Android passes an efficient frame representation to the Rust/native analysis component.
3. Analysis returns marker detections, Page Code result, estimated page polygon, blur score, exposure/glare indicators, framing status, and capture recommendation.
4. Compose renders guidance.
5. Android captures a full-resolution still when thresholds pass.

The live path MUST avoid JSON/Base64 conversion and excessive copies.

### 17.3 AprilTag integration

The initial implementation SHOULD evaluate the official AprilTag 3 detector and its recommended standard tag family through a reviewed native wrapper. The selected implementation MUST build reproducibly for Android architectures, have a documented license review, produce deterministic fixture results within tolerance, expose marker ID/corners/quality information, and be structured so the same native implementation can be built for iOS later.

### 17.4 Rectification

Given four known semantic markers, validate marker identity/orientation, resolve canonical coordinates, estimate a projective transform, rectify into canonical dimensions, crop to the defined capture rectangle, and preserve transform/detection provenance.

Four-corner rectification assumes the capture area is approximately planar. The notebook design intentionally excludes the severe gutter-curvature region.

### 17.5 Derived images

The pipeline SHOULD generate a corrected color archival image, OCR-optimized image, and thumbnail. Enhancement MUST be restrained. Pencil strokes and faint marks MUST NOT be destroyed by aggressive thresholding.

### 17.6 Quality states

```text
Accepted
AcceptedWithWarnings
NeedsReview
Rejected
```

Hard rejection examples include invalid/conflicting Page Code, insufficient markers, no valid transform, unusable resolution, or corrupt capture. Warnings include moderate blur, partial glare, low contrast, possible curvature, manual corners, or unavailable OCR.

Manual alignment MAY be supported, but provenance MUST state that automatic marker alignment did not succeed.

---

## 18. Scan registration and versioning

When a scan targets an existing page, Rust compares geometric alignment, perceptual fingerprint, changed regions, capture quality, and the current preferred scan. The classifier returns a proposal, not an irreversible mutation.

Default policy:

- Preserve both scans.
- Allow the user to select a preferred scan.
- Never delete the previous original automatically.
- Store relationships between revisions.

If the same Smart Page QR contains substantially different writing, require a user choice between newer version, another physical copy, wrong scan, or unresolved review.

---

## 19. OCR architecture

### 19.1 Provider abstraction

Rust defines a normalized OCR request/result contract. Android v0.1 MAY use ML Kit Text Recognition through a Kotlin platform adapter.

The adapter returns full text, blocks/lines/elements, polygons/bounding boxes, confidence where available, recognized languages, provider/model version, and warnings.

### 19.2 OCR is derived data

OCR failure MUST NOT prevent the page image from being saved. Failure produces a saved scan, explicit OCR status, and review/retry option. It MUST NOT produce a fabricated empty “successful” transcription.

### 19.3 Handwriting expectations

v0.1 treats handwriting OCR as best-effort. User correction is authoritative for search preference but preserves machine output and correction history.

---

## 20. Search

Rust owns a local SQLite FTS index covering OCR text, corrected text, page titles, tags, annotations, notebook names, and approved skill-produced documents.

Every result includes page identity, notebook/collection context, excerpt, match source, region coordinates where available, and citation information.

Semantic search is optional in v0.1, but the model abstraction MUST allow a future embedding provider without changing page identity or canonical storage.

---

## 21. A2D Skills and models

### 21.1 Skill categories

- **Deterministic:** ordinary code, no model required.
- **Model-assisted:** OCR, embeddings, classifiers, or LLMs.
- **Agentic/tool-using:** model selects from explicitly granted A2D tools.

### 21.2 Built-in v0.1 skills

The initial built-in set SHOULD include export selected pages to Markdown, summarize selected pages, extract proposed action items, find related pages, and compare two scans of one page. Only Markdown export is required to work without a model.

### 21.3 Permission model

Capabilities include `pages.search`, `pages.read_metadata`, `pages.read_text`, `pages.read_image`, `pages.create_annotation`, `pages.add_tag`, `collections.create`, `exports.create`, `model.generate_text`, `model.analyze_image`, and separately granted network access.

Rules:

- Read-only by default.
- No raw database access.
- No shell access.
- No arbitrary filesystem access.
- No network access unless separately granted.
- Mutations produce proposals unless explicitly classified as low-risk and previously approved.
- External side effects require explicit confirmation.
- Permissions are revocable.
- Every run is audited.

### 21.4 Model providers

The architecture MUST support on-device/local models, local-network OpenAI-compatible endpoints, user-provided cloud API keys, and a future managed A2D AI service.

Provider secrets remain in platform secure storage and MUST NOT be written into the Rust database or ordinary backup.

### 21.5 Prompt-injection defense

Notebook content is untrusted data. The runtime MUST separate system policy, skill instructions, user request, retrieved notebook content, and tool results. Text inside a notebook page MUST NOT grant permissions, alter policy, or trigger network access.

### 21.6 Source citations

LLM answers MUST cite source pages. The UI MUST allow the user to open the cited page and relevant region where available. Results MUST distinguish directly supported statements, model inference, low-confidence interpretation, and incomplete retrieval.

---

## 22. Backup and restore

### 22.1 Core-product requirement

Manual full-library backup and restore are included without an account.

### 22.2 Backup format

The file extension is `.atnb`. The archive MUST contain a format/version manifest, a consistent database snapshot or canonical object export, original and derived assets, content hashes, layout/design manifests, OCR/corrections, collections/tags, skill results/provenance, and backup metadata.

### 22.3 Encryption

Encrypted backup MUST be the default. A reviewed construction SHOULD use a memory-hard password derivation function such as Argon2id and authenticated encryption such as XChaCha20-Poly1305 with unique salt/nonce material and authenticated metadata.

There MUST be no fallback from requested encryption to plaintext.

### 22.4 Backup creation

1. Validate library state.
2. Create a consistent snapshot.
3. Stream assets into a temporary archive.
4. Hash and authenticate contents.
5. Finalize and verify the archive.
6. Use the system file picker to copy it to the user-selected destination.
7. Record success only after verification.

### 22.5 Restore inspection

Before modifying the current library, Rust MUST inspect format version, password/authentication, object counts, asset checksums, free-space requirement, and compatibility.

### 22.6 Replace and merge

Restore supports Replace and Merge. Replace restores into a new verified library before switching. Merge imports by immutable identity, reconciles safe additive metadata, and creates review items for conflicts. A failed restore MUST leave the prior library usable.

---

## 23. Export

Supported v0.1 exports are original images, corrected images, searchable PDF where practical, Markdown, plain text, JSON metadata, and complete `.atnb` backup.

Exports MUST preserve page ordering and source references. Export failure MUST NOT alter source data.

---

## 24. Optional A2D Sync boundary

A2D Sync is not part of v0.1 implementation. The core SHOULD define future-compatible object revisions containing object ID/type, revision, originating installation, timestamps, content hash, tombstone, and encrypted-payload reference.

Future service expectations:

- Account required only for the paid service.
- End-to-end encrypted content.
- Local operation continues offline.
- Subscription expiration does not disable the local app.
- Manual backup remains available.
- Device revocation and conflict handling.
- Incremental object sync rather than whole-database replacement.

No v0.1 code MAY require an A2D Sync endpoint.

---

## 25. Android application structure

Recommended logical packages:

```text
com.a2d.notebook
├── app
├── navigation
├── design
├── feature.home
├── feature.notebooks
├── feature.scanner
├── feature.smartpages
├── feature.library
├── feature.pageviewer
├── feature.search
├── feature.skills
├── feature.backup
├── feature.settings
├── platform.camera
├── platform.files
├── platform.print
├── platform.ocr
├── platform.securestore
└── rustbridge
```

Kotlin owns temporary UI state such as current route, dialogs, text drafts, camera permission state, live capture overlays, and animations. Rust owns persistent/business state such as active notebook, pages/scans, review items, backup records, search, skill permissions, and provenance.

ViewModels MUST call typed Rust use cases. They MUST NOT implement page identity, duplicate classification, backup, or database rules.

---

## 26. Screen inventory for v0.1

Required screens or major states:

1. Welcome/local-first explanation.
2. Create or restore library.
3. Home empty state.
4. Home populated state.
5. Add Notebook scanner.
6. Notebook Design recognized.
7. Name Notebook.
8. Notebook detail.
9. Active Notebook selector.
10. Single-page scanner.
11. Batch scanner.
12. Capture review.
13. Scanner warning/error states.
14. Create Smart Pages.
15. Single-page configuration.
16. Page-set configuration.
17. PDF preview.
18. Generated page/set detail.
19. Library hub.
20. Notebook page grid/list.
21. Smart Pages.
22. Collections.
23. Needs Review.
24. Page viewer.
25. Image/text split view.
26. OCR correction.
27. Version history.
28. Scan comparison.
29. Basic search.
30. Search results/filters.
31. Ask My Notes.
32. Answer with citations.
33. Skills hub.
34. Skill permission review.
35. Skill proposal review.
36. Backup and restore hub.
37. Create backup.
38. Restore inspection.
39. Replace/merge selection.
40. Export options.
41. Settings.
42. AI provider settings.
43. Storage usage.
44. Backup reminders.

Paid sync screens are deferred.

---

## 27. Error model

Rust MUST expose a stable error envelope containing code, category, severity, user message key, developer message, retryable flag, details, and correlation ID.

Categories include validation, identity, unsupported format, capture quality, image processing, storage, integrity, migration, backup, restore, OCR, search, skill permission, model provider, platform adapter, cancellation, and internal error.

Rules:

- Panics MUST be treated as defects and MUST NOT cross FFI as success.
- Errors MUST NOT be reduced to `null`, empty lists, or `false`.
- User-facing messages MUST be actionable.
- Developer diagnostics MUST not expose secrets.
- Retriable and non-retriable failures MUST be distinguished.

---

## 28. Security and privacy

- No network request occurs by default.
- Every network-capable feature shows its provider and scope.
- API keys use Android Keystore/future iOS Keychain.
- QR payloads contain no user name, notebook title, OCR, or private metadata.
- Imported files and notebook content are untrusted.
- Archive extraction MUST prevent path traversal and resource exhaustion.
- Image decoders MUST enforce size limits.
- SQL queries MUST be parameterized.
- Backups MUST be authenticated before restore.
- Skill permissions MUST be enforced in Rust, not only hidden in UI.
- Audit logs MUST record model/provider use and approved mutations.
- Logs MUST avoid raw note content by default.

---

## 29. Performance and resource targets

- Live marker guidance SHOULD update responsively on supported devices.
- Reduced analysis frames SHOULD be used for live detection.
- Full-resolution processing MUST run off the main thread.
- Scanning MUST continue while prior OCR/indexing work is queued.
- Large backup/export operations MUST stream data rather than load all assets into memory.
- Search SHOULD return initial results quickly for a library of at least 10,000 pages.
- Database operations MUST use bounded transactions.
- Cancellation MUST leave consistent state.

Exact device-tier thresholds MUST be measured and recorded rather than guessed.

---

## 30. Testing strategy

### 30.1 Rust unit tests

Cover IDs, QR parsing/encoding, layout validation, domain invariants, state transitions, duplicate/rescan classification, backup cryptography/archive validation, restore merge rules, search queries, skill permissions, and error mapping.

### 30.2 Rust integration tests

Cover fresh database creation, every migration path, transaction rollback, interrupted asset commit, backup/restore round trip, corrupt backup rejection, fixture scan processing, generated PDF/QR round trip, and FFI use cases.

### 30.3 Cross-language binding tests

CI MUST generate Kotlin bindings, compile the Android bridge, generate Swift bindings, detect accidental FFI API changes, and run fixture conformance tests through Kotlin where practical.

### 30.4 Android tests

Cover Compose navigation, permission flows, camera lifecycle interruption, activity recreation, file picker cancellation, printing/sharing, OCR adapter mapping, background work resumption, and error/review-state presentation.

### 30.5 Physical/device tests

Cover multiple Android devices, KDP proof copies, home-printed Smart Pages, printer scaling errors, lighting/glare, writing instruments, and pages near the spine.

### 30.6 Failure injection

Required tests include disk full, process killed between asset write and DB commit, corrupt database, corrupt image, invalid QR, missing marker, interrupted backup, wrong backup password, unsupported backup version, unavailable OCR, model timeout, denied skill permission, and user cancellation.

---

## 31. CI quality gates

Before merge:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
Android lint
Android unit tests
Kotlin binding generation check
Swift binding generation check
fixture/schema compatibility checks
```

Native marker/image dependencies MUST build in reproducible CI for supported Android ABIs before scanner work is considered complete.

---

## 32. v0.1 acceptance criteria

Version 0.1 is acceptable only when:

1. Android can create and reopen a local library without an account.
2. Rust owns and migrates the canonical SQLite database.
3. Kotlin can call Rust through generated typed bindings.
4. Swift bindings can be generated from the same interface.
5. User can register two identical physical notebooks as distinct notebooks.
6. User can scan recto-only notebook pages into the selected notebook.
7. Wrong-design scans are blocked or explicitly reassigned.
8. User can generate unique single Smart Pages and Page Sets as PDFs.
9. Generated QR codes round-trip through the Rust parser.
10. Original images survive all processing and rescanning operations.
11. A second scan never destroys the first original automatically.
12. OCR failure does not lose the page.
13. Basic search returns page-linked results.
14. Manual encrypted backup can be created, verified, restored, and merged.
15. Export produces usable user-owned files.
16. Built-in skills cannot exceed granted permissions.
17. LLM answers include source-page citations.
18. No network or account is required for core behavior.
19. Failure injection does not produce silent data loss.
20. Physical proof and printed-page tests meet documented capture thresholds.

---

## 33. Implementation order

1. Repository and CI foundation.
2. Rust domain, errors, FFI, and persistence.
3. Identity and QR protocol.
4. Page layouts and PDF generation.
5. Notebook/Smart Page business workflows.
6. Native image/marker processing spike.
7. Android camera and scanning.
8. Scan registration/versioning.
9. Library and page UI.
10. OCR and search.
11. Backup, restore, and export.
12. Skills/model architecture.
13. Hardening, physical validation, and release gates.

The TODO file provides the executable breakdown.

---

## 34. Primary implementation references

Implementers should validate pinned dependency behavior against primary documentation:

- UniFFI user guide for Kotlin and Swift binding generation.
- Android CameraX documentation for lifecycle-aware camera and image analysis.
- ML Kit Text Recognition documentation for the initial Android OCR adapter.
- Official AprilRobotics AprilTag repository and license for detector integration.

External documentation must not override the product invariants in this specification.
