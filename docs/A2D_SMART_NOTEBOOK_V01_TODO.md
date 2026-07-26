# A2D Smart Notebook v0.1 — Implementation TODO

**Status:** Ready for implementation planning and Ralph-loop execution  
**Version:** 0.1  
**Date:** 2026-07-26  
**Repository:** `ekkus93/a2d-smart-notebook`  
**Authoritative specification:** `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`

---

## 0. Execution rules

This TODO is deliberately explicit so an implementation agent can execute it incrementally.

For every task:

- [ ] Read the corresponding specification section before changing code.
- [ ] Keep the Rust core authoritative for data and business logic.
- [ ] Add or update tests with the implementation.
- [ ] Run the narrowest relevant tests during development.
- [ ] Run full required checks before marking a milestone complete.
- [ ] Preserve original assets and existing data.
- [ ] Return structured errors and warnings.
- [ ] Add fixtures for portable formats and state transitions.
- [ ] Keep every referenced handoff file committed at the exact path named.

Do not:

- [ ] Put canonical domain rules in Kotlin ViewModels.
- [ ] Add Room as a second canonical database.
- [ ] Return empty data after an internal failure.
- [ ] Catch broad exceptions and continue without surfacing a warning.
- [ ] Replace invalid data with defaults silently.
- [ ] Delete or overwrite an existing original scan during rescan handling.
- [ ] Upload note data without a user-selected provider and explicit scope.
- [ ] Store model API keys in source, Gradle, Rust configuration, logs, or the canonical database.
- [ ] Give skills raw SQL, shell, arbitrary filesystem, or implicit network access.
- [ ] Mark a task complete when only a mock or stub exists unless the task explicitly calls for one.

A task is complete only when code compiles, tests pass, no placeholder remains in the completed path, error handling is explicit, and acceptance behavior is demonstrated.

---

# Milestone 1 — Repository, toolchain, Android shell, and CI

## 1.1 Create the workspace

Create:

```text
Cargo.toml
rust-toolchain.toml
deny.toml
crates/
apps/android/
apps/ios/
fixtures/
tools/
.github/workflows/
```

Initial crates:

```text
crates/a2d-domain
crates/a2d-identity
crates/a2d-layout
crates/a2d-storage
crates/a2d-image
crates/a2d-pdf
crates/a2d-search
crates/a2d-ocr
crates/a2d-model
crates/a2d-skills
crates/a2d-backup
crates/a2d-export
crates/a2d-sync-model
crates/a2d-core
crates/a2d-ffi
```

Tasks:

- [x] Create the Rust workspace with resolver 2.
- [x] Pin a stable Rust toolchain.
- [x] Set workspace-wide edition, license, repository metadata, and lint policy.
- [x] Add `.gitignore` entries for Rust, Gradle, Android Studio, native builds, test output, local libraries, generated PDFs, and secrets.
- [x] Add a `README.md` describing A2D, local-first/accountless operation, Android-first delivery, and the authoritative Rust core.
- [x] Add `apps/ios/README.md` explaining that iOS UI is deferred but Swift binding generation is mandatory.
- [x] Verify `cargo metadata` and `cargo build --workspace`.

Suggested skeleton:

```toml
[workspace]
resolver = "2"
members = [
  "crates/a2d-domain",
  "crates/a2d-identity",
  "crates/a2d-layout",
  "crates/a2d-storage",
  "crates/a2d-image",
  "crates/a2d-pdf",
  "crates/a2d-search",
  "crates/a2d-ocr",
  "crates/a2d-model",
  "crates/a2d-skills",
  "crates/a2d-backup",
  "crates/a2d-export",
  "crates/a2d-sync-model",
  "crates/a2d-core",
  "crates/a2d-ffi",
]
```

Acceptance:

- [x] A fresh clone builds the Rust workspace.
- [x] Responsibility boundaries are documented in crate-level READMEs or module docs.

## 1.2 Initialize Android

- [ ] Create a Kotlin Android application in `apps/android`.
- [ ] Use Jetpack Compose and Compose Navigation.
- [ ] Use package namespace `com.a2d.notebook`.
- [ ] Document the selected minimum Android API and why.
- [ ] Add a placeholder Home screen and navigation shell.
- [ ] Add unit and instrumentation test source sets.
- [ ] Confirm debug installation on an emulator or device.
- [ ] Do not add Room for canonical A2D data.

Acceptance:

- [ ] `./gradlew :app:assembleDebug` succeeds.
- [ ] The placeholder application launches.

## 1.3 Add CI

- [ ] Rust format check.
- [ ] Rust clippy with warnings denied.
- [ ] Rust unit/integration tests.
- [ ] Android lint and unit tests.
- [ ] Android debug assembly.
- [ ] Kotlin UniFFI binding generation drift check.
- [ ] Swift UniFFI binding generation smoke check.
- [ ] Dependency/license checks after policy configuration.
- [ ] Fixture compatibility checks after fixtures exist.

Required commands:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
./gradlew lint test assembleDebug
```

Acceptance:

- [ ] CI runs on pushes and pull requests.
- [ ] Deliberate formatting and test failures block CI.

---

# Milestone 2 — Domain model, structured errors, and UniFFI

## 2.1 Strongly typed identifiers

Create opaque Rust newtypes for every independently persisted entity — one that has its own row, can be referenced by another record, appears in provenance, or can cross the FFI boundary. Do not add identifiers for embedded value objects with no independent persistence or identity.

- [x] `InstallationId`
- [x] `NotebookDesignId`
- [x] `NotebookId`
- [x] `PageId`
- [x] `PageSetId`
- [x] `SmartPageId`
- [x] `PhysicalCopyId`
- [x] `ScanId`
- [x] `AssetId`
- [x] `OcrRunId`
- [x] `TextRegionId`
- [x] `TextCorrectionId`
- [x] `CollectionId`
- [x] `AnnotationId`
- [x] `ReviewItemId`
- [x] `SkillId`
- [x] `SkillRunId`
- [x] `AuditEventId`
- [x] `BackupId`

Example:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PageId(String);
```

Requirements:

- [x] Constructors validate format.
- [x] Display and serialization are canonical.
- [x] Parsing rejects invalid length/alphabet.
- [x] Production IDs use OS cryptographic randomness.
- [x] A deterministic RNG is available only through test interfaces.
- [x] Domain APIs do not pass raw identifier strings internally.

## 2.2 Structured error model

Implement an FFI-safe error envelope:

```rust
pub struct A2dError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub user_message_key: String,
    pub developer_message: String,
    pub retryable: bool,
    pub correlation_id: String,
    pub details: BTreeMap<String, String>,
}
```

- [x] Implement all categories from the spec.
- [ ] Redact secrets and raw note content from default diagnostics. (No concrete redaction
      mechanism exists yet — `A2dError.details` is documented as MUST NOT carry secrets/note
      content, enforced by each producing module, not by a generic filter. Revisit once a
      producer actually needs to pass user content through, e.g. OCR or model-provider errors.)
- [x] Map cancellation separately from failure.
- [ ] Ban conversions that erase failures into `None`, empty collections, or `false`. (No
      erasing conversion exists on `A2dError`/`Outcome` today, but "ban" implies an ongoing
      project-wide discipline — no clippy lint enforces it yet. Tracked as a standing rule in
      `CLAUDE.md`, not a one-time deliverable.)
- [x] Add stable unknown-internal-error handling with correlation ID.
- [ ] Add tests for FFI/serialization mapping and redaction. (No FFI/serde boundary exists yet;
      deferred to Milestone 2.4.)

## 2.3 Domain entities and invariants

Implement:

- [ ] `NotebookDesign`
- [ ] `Notebook`
- [ ] `Page`
- [ ] `PhysicalCopy`
- [ ] `Scan`
- [ ] `Asset`
- [ ] `PageSet`
- [ ] `Collection`
- [ ] `ReviewItem`
- [ ] `OcrRun`
- [ ] `TextRegion`
- [ ] `TextCorrection`
- [ ] `Annotation`
- [ ] `SkillDefinition`
- [ ] `SkillRun`
- [ ] `AuditEvent`

Suggested page kind:

```rust
pub enum PageKind {
    NotebookPage {
        notebook_id: NotebookId,
        design_id: NotebookDesignId,
        logical_page_number: u32,
    },
    SmartPage {
        smart_page_id: SmartPageId,
        page_set_id: Option<PageSetId>,
        visible_page_number: Option<u32>,
    },
}
```

Enforce:

- [ ] Notebook Page requires notebook, design, and logical page number.
- [ ] Smart Page requires a unique Smart Page ID.
- [ ] Page identity cannot change after creation.
- [ ] Preferred scan belongs to the same page.
- [ ] A scan always references an immutable original asset.
- [ ] Physical-copy index is unique per page.
- [ ] Derived records identify source and producer.
- [ ] Trashed records retain identity until permanent deletion.

## 2.4 UniFFI boundary

- [ ] Pin and configure UniFFI.
- [ ] Choose proc-macro or UDL mode and document the reason.
- [ ] Create `a2d-ffi` as a thin façade over `a2d-core`.
- [ ] Generate Kotlin bindings.
- [ ] Generate Swift bindings.
- [ ] Add a minimal `A2dClient` object.
- [ ] Add API snapshot or generated-binding drift tests.
- [ ] Prevent panics from appearing as successful FFI results.

Example API shape:

```rust
pub struct A2dClient {
    core: Arc<A2dCore>,
}

impl A2dClient {
    pub fn open(request: OpenLibraryRequest) -> Result<Arc<Self>, A2dFfiError>;
    pub async fn list_notebooks(&self) -> Result<Vec<NotebookSummary>, A2dFfiError>;
}
```

Acceptance:

- [ ] Android calls Rust and renders a typed response.
- [ ] Swift bindings generate in CI.
- [ ] `a2d-ffi` contains no SQL or business rules.

---

# Milestone 3 — Rust-owned SQLite and assets

## 3.1 Database bootstrap and migrations

- [ ] Open/create SQLite at an app-provided library path.
- [ ] Enable and verify foreign keys.
- [ ] Select and document journaling/synchronous settings.
- [ ] Implement numbered immutable migrations.
- [ ] Track schema version and migration identity.
- [ ] Return blocking errors on migration failure.
- [ ] Never recreate an empty database after migration failure.

Initial tables must cover notebooks, notebook designs, pages, physical copies, scans, assets, page sets, collections, OCR runs, text regions, corrections, annotations, review items, skill definitions/runs, audit events, backup history, and settings.

Add constraints and indexes for invariants and common queries.

Example guidance:

```sql
CREATE TABLE notebooks (
    id TEXT PRIMARY KEY NOT NULL,
    design_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE TABLE pages (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    notebook_id TEXT,
    design_id TEXT,
    logical_page_number INTEGER,
    smart_page_id TEXT,
    page_set_id TEXT,
    visible_page_number INTEGER,
    layout_id TEXT NOT NULL,
    title TEXT,
    state TEXT NOT NULL,
    preferred_scan_id TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(notebook_id) REFERENCES notebooks(id)
);

CREATE UNIQUE INDEX unique_notebook_logical_page
ON pages(notebook_id, logical_page_number)
WHERE notebook_id IS NOT NULL;
```

## 3.2 Repository and transaction layer

- [ ] Define repository traits inside Rust.
- [ ] Keep SQL implementations private to `a2d-storage`.
- [ ] Add explicit transaction APIs.
- [ ] Require transactions for notebook creation, generated page sets, scan registration, OCR replacement, and restore merge.
- [ ] Map constraints into typed errors.

Example:

```rust
storage.transaction(|tx| {
    tx.insert_page(&page)?;
    tx.insert_scan(&scan)?;
    tx.set_preferred_scan(&page.id, &scan.id)?;
    tx.insert_audit_event(&event)?;
    Ok(())
})
```

## 3.3 Asset repository

Use a private tree similar to:

```text
library/
├── library.sqlite
├── assets/
│   ├── originals/
│   ├── corrected/
│   ├── ocr/
│   ├── thumbnails/
│   └── exports/
└── tmp/
```

- [ ] Store relative paths in the database.
- [ ] Validate paths remain inside library root.
- [ ] Write temporary files first.
- [ ] Flush/close, compute SHA-256, then atomically rename.
- [ ] Mark originals immutable.
- [ ] Detect orphan temporary files without deleting unknown files silently.
- [ ] Commit references only after durable file creation.

## 3.4 Integrity and interruption tests

- [ ] Interrupt after temp write.
- [ ] Interrupt after rename but before DB commit.
- [ ] Force transaction rollback.
- [ ] Detect missing assets and hash mismatches.
- [ ] Test foreign-key failures and DB lock/busy handling.
- [ ] Map disk-full behavior explicitly.

Acceptance:

- [ ] A committed scan can never reference an original file that was never durably written.
- [ ] Recovery never deletes user data silently.

---

# Milestone 4 — Identity, QR protocol, and Notebook Designs

## 4.1 Random ID generation

- [ ] Generate 128-bit IDs from OS cryptographic randomness.
- [ ] Encode using a compact canonical alphabet.
- [ ] Detect persistence collisions as hard integrity events.
- [ ] Add known encoding vectors and malformed-input tests.
- [ ] Add a large sample uniqueness test.

## 4.2 QR payload model

The v1 wire encoding and integrity check are governed by
`docs/decisions/0001-qr-v1-encoding-and-integrity.md`. That ADR must reach **Accepted** status
(spike-validated against a real Android QR decoder) before this task's golden fixtures (4.3) are
committed. Do not invent an alternate encoding here — implement the ADR's canonical alphanumeric
text payload, uppercase Crockford Base32 identifiers, and CRC-32C integrity field.

Implement:

```rust
pub enum PageCode {
    NotebookSetup {
        version: u8,
        design_id: NotebookDesignId,
    },
    NotebookPage {
        version: u8,
        design_id: NotebookDesignId,
        logical_page_number: u32,
        layout_id: LayoutId,
    },
    SmartPage {
        version: u8,
        smart_page_id: SmartPageId,
        layout_id: LayoutId,
        visible_page_number: Option<u32>,
        page_set_id: Option<PageSetId>,
    },
}
```

- [ ] Define canonical v1 encoding.
- [ ] Define checksum/integrity bytes.
- [ ] Define maximum length.
- [ ] Reject unsupported versions, invalid lengths, invalid alphabet, out-of-range numbers, failed integrity, and trailing data.
- [ ] Never open invalid A2D payloads as arbitrary URLs.

## 4.3 Golden fixtures

Create:

```text
fixtures/qr/v1/
├── notebook_setup_vectors.json
├── notebook_page_vectors.json
├── smart_page_vectors.json
├── malformed_vectors.json
└── rendered/
```

- [ ] Store payload, expected value, and expected errors.
- [ ] Render QR images for valid vectors.
- [ ] Decode rendered images in integration tests.
- [ ] Treat v1 vectors as permanent compatibility fixtures.
- [ ] Do not commit these fixtures until `docs/decisions/0001-qr-v1-encoding-and-integrity.md` is Accepted. Once committed, changing the v1 wire format requires a new protocol version, not a fixture rewrite.

## 4.4 Notebook Design manifests

- [ ] Define versioned manifests with physical dimensions, layout IDs, marker family/roles, logical page count, and hash.
- [ ] Bundle initial official manifests.
- [ ] Resolve them fully offline.
- [ ] Track trust state.
- [ ] Leave extension fields for signed official designs.
- [ ] Reject unsupported required versions.

Acceptance:

- [ ] Setup and Page Codes round-trip through Rust, Kotlin, and rendered fixtures.

---

# Milestone 5 — Layout engine and Rust PDF generation

## 5.1 Canonical physical layout model

- [ ] Use fixed physical units, not authoritative screen pixels.
- [ ] Define page size, safe margins, content rectangle, four marker rectangles, QR rectangle, visible numbering, and calibration mark.
- [ ] Validate bounds, overlap, marker roles, and quiet zones.

Example:

```rust
pub struct PageLayout {
    pub id: LayoutId,
    pub physical_size: PhysicalSize,
    pub content_rect: PhysicalRect,
    pub markers: [MarkerPlacement; 4],
    pub qr_rect: PhysicalRect,
    pub visible_page_number_rect: Option<PhysicalRect>,
    pub calibration: CalibrationMark,
}
```

## 5.2 Smart Page layouts

Add US Letter and A4 portrait layouts for:

- [ ] Blank.
- [ ] Lined.
- [ ] Dot grid.
- [ ] Graph.

- [ ] Test deterministic dimensions and spacing.
- [ ] Test that markers and QR remain within printer-safe margins.
- [ ] Test no overlap with the writable region.

## 5.3 Bound-notebook layout

- [ ] Record the first trim-size decision.
- [ ] Define a larger left/gutter exclusion.
- [ ] Define fixed recto orientation.
- [ ] Define Setup Page and writable page layouts.
- [ ] Define logical numbering independent of manuscript PDF page number.
- [ ] Generate blank verso pages.
- [ ] Generate a complete proof interior PDF.

## 5.4 PDF renderer

The PDF renderer must live in Rust.

- [ ] Render vector Corner Markers without interpolation blur.
- [ ] Render QR at an integral module scale.
- [ ] Render line/grid styles deterministically.
- [ ] Use legally distributable fonts or avoid embedding unlicensed fonts.
- [ ] Generate single-page PDFs.
- [ ] Generate multipage Page Sets.
- [ ] Generate the bound-notebook proof interior.
- [ ] Write to a temp path and verify before returning success.

Suggested request:

```rust
pub struct GeneratePageSetRequest {
    pub title: Option<String>,
    pub paper_size: PaperSize,
    pub style: PageStyle,
    pub page_count: u32,
    pub starting_visible_page: u32,
    pub output_path: String,
}
```

## 5.5 Transactional generated-page registration

- [ ] Create Page Set and all unique page identities transactionally.
- [ ] Generate and verify PDF.
- [ ] Attach the PDF asset and mark success.
- [ ] On failure, roll back coherently or retain an explicit failed-generation record.
- [ ] Retry safely without duplicate logical records.

## 5.6 PDF tests

- [ ] Check page counts and metadata.
- [ ] Rasterize each generated page in tests.
- [ ] Decode every Page Code.
- [ ] Detect all four Corner Markers.
- [ ] Verify positions within tolerance.
- [ ] Simulate 95%, 100%, and 105% print scaling.
- [ ] Test truncated/corrupt output behavior.

Acceptance:

- [ ] A generated page can be printed, photographed, identified, and rectified using bundled metadata.

---

# Milestone 6 — Notebook and Smart Page workflows

## 6.1 Rust Notebook service

Implement:

- [ ] `resolve_notebook_setup_code`
- [ ] `create_notebook`
- [ ] `rename_notebook`
- [ ] `archive_notebook`
- [ ] `list_notebooks`
- [ ] `get_notebook`
- [ ] `set_active_notebook`
- [ ] `get_active_notebook`

Rules:

- [ ] Multiple notebooks may share one design.
- [ ] Names need not be unique.
- [ ] IDs are unique.
- [ ] Active notebook is explicit persistent state.
- [ ] The UI may require confirmation of a stale selection, but must not silently change it.

## 6.2 Page resolution

Given a parsed Page Code and optional active notebook:

- [ ] Resolve a Smart Page by unique ID.
- [ ] Resolve a Notebook Page only through a matching Notebook Design.
- [ ] Return ambiguity when multiple physical notebooks match and no active notebook is confirmed.
- [ ] Return conflict when active design differs.
- [ ] Support unassigned/review state for imports.
- [ ] Never auto-create a physical notebook from an ordinary page code.

Suggested result:

```rust
pub enum PageResolution {
    Resolved { page_id: PageId },
    RequiresNotebookSelection { candidates: Vec<NotebookSummary> },
    RequiresNotebookRegistration { design: NotebookDesignSummary },
    ConflictingActiveNotebook {
        active: NotebookSummary,
        detected_design: NotebookDesignId,
    },
    ImportedUnknownSmartPage { proposed: ImportedSmartPageStub },
    UnsupportedCode { reason: String },
}
```

## 6.3 Android Notebook UI

- [ ] Add Notebook scanner.
- [ ] Notebook Design recognized screen.
- [ ] Name/customize Notebook.
- [ ] Created confirmation and Scan First Page action.
- [ ] Unsupported/invalid Setup Code state.
- [ ] Multiple-copy explanation.
- [ ] Active Notebook selector.

ViewModels must delegate all identity/business rules to Rust.

## 6.4 Smart Page UI

- [ ] Create Smart Pages landing screen.
- [ ] Single-page form.
- [ ] Page-set form.
- [ ] PDF preview.
- [ ] Android print/save/share integration.
- [ ] Generated page/set detail.
- [ ] Failed generation state with safe retry.

Acceptance:

- [ ] User can register two identical physical notebook copies separately.
- [ ] User can generate a unique Smart Page offline without an account.

---

# Milestone 7 — Marker detection and image-processing foundation

## 7.1 Complete a working detector spike

- [ ] Evaluate the official AprilTag 3 native library.
- [ ] Confirm license compatibility and commit the review.
- [ ] Build reproducibly for required Android ABIs.
- [ ] Wrap ownership and errors safely for Rust.
- [ ] Measure detection on representative grayscale fixtures.
- [ ] Confirm future iOS build feasibility.
- [ ] Compare a pure-Rust alternative only if it materially reduces packaging risk.
- [ ] Accept `docs/decisions/0002-apriltag-detector-selection.md`, naming the selected
      implementation and recording license review, Android ABI build results, desktop fixture
      results, performance measurements, the memory-safety boundary, packaging strategy, and
      future iOS feasibility.

The spike must end with code and tests, not prose only.

## 7.2 Image input types

- [ ] Define width, height, row stride, pixel format, rotation, and buffer ownership.
- [ ] Support reduced grayscale analysis frames.
- [ ] Support full-resolution image files for final processing.
- [ ] Reject impossible dimensions/strides.
- [ ] Enforce maximum decoded pixel count.
- [ ] Avoid Base64.

Example:

```rust
pub struct GrayFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub row_stride: usize,
    pub rotation_degrees: u16,
    pub bytes: &'a [u8],
}
```

## 7.3 Detection results

Return marker ID, four corners, center, decision margin, and error-quality data.

- [ ] Validate expected family.
- [ ] Resolve marker semantic roles from layout.
- [ ] Reject duplicate roles.
- [ ] Resolve orientation.
- [ ] Preserve detection quality.

## 7.4 QR decoder boundary

- [ ] Decide whether image decoding runs in platform or shared native code.
- [ ] Always send decoded payload to Rust for canonical parsing.
- [ ] Add blur/rotation/scale/damage fixtures.
- [ ] Never accept a decoder result without Rust validation.

## 7.5 Homography and rectification

- [ ] Compute projective transform from known correspondences.
- [ ] Validate transform conditioning.
- [ ] Reject self-intersecting or implausible quadrilaterals.
- [ ] Warp to canonical dimensions.
- [ ] Preserve matrix and source corners.
- [ ] Add deterministic reference-output tests.
- [ ] Prevent out-of-bounds access on malformed inputs.

## 7.6 Quality metrics

Implement raw metrics and classifications for:

- [ ] Blur/focus.
- [ ] Underexposure.
- [ ] Overexposure.
- [ ] Glare/highlight clipping.
- [ ] Page fill/framing.
- [ ] Marker confidence.
- [ ] Perspective severity.
- [ ] Effective content resolution.
- [ ] Possible curvature.

Put thresholds in versioned configuration. Never invent success if measurement fails.

## 7.7 Derived images

- [ ] Corrected color image.
- [ ] OCR-optimized image.
- [ ] Thumbnail.
- [ ] Conservative contrast normalization.
- [ ] Bounded optional sharpening.
- [ ] Pipeline provenance/version.
- [ ] Memory-bounded processing.
- [ ] Cancellation-safe temporary outputs.
- [ ] Never overwrite original.

## 7.8 Fixture corpus

Create legally usable fixtures under:

```text
fixtures/scans/
├── generated/
├── photographed/
├── glare/
├── blur/
├── missing-marker/
├── wrong-layout/
├── duplicate/
├── revisions/
└── corrupted/
```

Every fixture records source/license, expected identity, quality state, marker roles, warnings, and tolerances.

Acceptance:

- [ ] A desktop Rust test/CLI processes fixtures without Android.
- [ ] Android calls the same shared processing path.

---

# Milestone 8 — CameraX scanning

## 8.1 Camera adapter

- [ ] CameraX preview.
- [ ] Image analysis.
- [ ] Full-resolution capture.
- [ ] Correct lifecycle binding.
- [ ] Permission-denied handling.
- [ ] Background/foreground recovery.
- [ ] Rotation handling.
- [ ] Torch control where available.
- [ ] Bounded analysis backpressure.
- [ ] Reliably close every frame.

## 8.2 Live Rust/native analysis

- [ ] Extract luminance efficiently.
- [ ] Pass dimensions/stride/orientation correctly.
- [ ] Measure copies and latency.
- [ ] Analyze off the main thread.
- [ ] Cancel stale work.
- [ ] Render page/marker overlay.
- [ ] Show active Notebook prominently.
- [ ] Show actionable guidance.
- [ ] Block auto-capture on identity conflict.

## 8.3 Auto-capture state machine

Use explicit states:

```text
Idle
Searching
CandidateStable
Capturing
Processing
Accepted
NeedsReview
Rejected
Paused
```

- [ ] Require stable acceptable frames for a configured interval.
- [ ] Debounce repeated captures.
- [ ] Permit manual capture only through an explicit warning path.
- [ ] Cancel safely on navigation.
- [ ] Recover from capture failure without losing context.

## 8.4 Single-page scanner

- [ ] Active Notebook selector.
- [ ] Camera preview.
- [ ] Marker/QR status.
- [ ] Capture guidance.
- [ ] Manual capture.
- [ ] Torch.
- [ ] Corrected preview.
- [ ] Accept/retake/details.
- [ ] Warning details.
- [ ] Processing progress/cancel.

## 8.5 Batch scanner

- [ ] Keep active Notebook fixed until explicitly changed.
- [ ] Save and return immediately to camera.
- [ ] Queue final processing/OCR.
- [ ] Show nonblocking saved confirmation.
- [ ] Detect duplicate page.
- [ ] Show session summary and review items.
- [ ] Survive activity recreation without duplicate registration.

## 8.6 Camera tests

- [ ] Permission denied/permanently denied.
- [ ] Camera unavailable.
- [ ] Background during capture.
- [ ] Process killed after capture before registration.
- [ ] Rotation during analysis.
- [ ] Rapid repeated capture.
- [ ] Wrong-design page.
- [ ] Two identical Notebook candidates.
- [ ] Batch out of order.
- [ ] Low storage.

Acceptance:

- [ ] UI does not claim a page is saved until Rust confirms durable registration.
- [ ] Scanner never silently changes destination Notebook.

---

# Milestone 9 — Durable scan registration and revisions

## 9.1 Final scan registration

The Rust request includes captured path, capture source, parsed code, marker detections, layout ID, quality metrics, active Notebook, and timestamp.

- [ ] Validate staging path.
- [ ] Reparse and validate Page Code.
- [ ] Validate markers against layout.
- [ ] Resolve page identity.
- [ ] Process images.
- [ ] Register files and database records transactionally.
- [ ] Return typed warnings and required actions.

## 9.2 Fingerprints and comparison

- [ ] Cryptographic asset hash.
- [ ] Versioned perceptual fingerprint.
- [ ] Aligned change-region comparison.
- [ ] Confidence and reason reporting.
- [ ] Fixture-based threshold tuning only.

Suggested assessment:

```rust
pub enum ExistingPageScanAssessment {
    FirstScan,
    NearDuplicate { better_quality: bool },
    PossibleRevision { changed_regions: Vec<Region>, confidence: f32 },
    SubstantiallyDifferent,
    Ambiguous,
}
```

## 9.3 Safe revision rules

- [ ] Preserve the new original before prompting, or keep it in a recoverable staged state.
- [ ] Default proposal is Save as New Version.
- [ ] Replace Preferred changes preference only.
- [ ] Never delete the old original automatically.
- [ ] Another Physical Copy creates `PhysicalCopy`.
- [ ] Wrong Scan moves to Needs Review or is explicitly discarded.

## 9.4 Needs Review

Implement review kinds for unidentified page, Notebook selection, wrong Notebook, low quality, manual alignment, duplicate, revision, physical copy, OCR failure, processing failure, import conflict, and restore conflict.

- [ ] List/filter/detail/resolve APIs.
- [ ] Audited resolutions.
- [ ] Defer without data loss.

## 9.5 Version UI

- [ ] Timeline.
- [ ] Preferred indicator.
- [ ] Side-by-side or overlay comparison.
- [ ] Changed regions.
- [ ] Keep both.
- [ ] Set preferred.
- [ ] Mark another physical copy.
- [ ] Move unresolved item to review.

---

# Milestone 10 — Library and page presentation

## 10.1 Home

- [ ] Empty and populated states.
- [ ] Recent Notebooks.
- [ ] Continue scanning.
- [ ] Generated Smart Pages.
- [ ] Needs Review count.
- [ ] Last backup and changed-page count.
- [ ] Primary actions.

## 10.2 Library hub

- [ ] Notebooks.
- [ ] Smart Pages.
- [ ] Page Sets.
- [ ] Collections.
- [ ] Imports.
- [ ] Needs Review.
- [ ] Trash.
- [ ] Pagination/streaming for large libraries.
- [ ] Sorting by title, page number, scan date, and modified date.

## 10.3 Notebook detail

- [ ] Show logical page slots including unscanned pages.
- [ ] Grid and list views.
- [ ] Status badges.
- [ ] Scan and Batch Scan actions.
- [ ] Rename/archive.
- [ ] Never renumber from scan order.

## 10.4 Smart Pages, Page Sets, and Collections

- [ ] Standalone pages.
- [ ] Generated-not-scanned state.
- [ ] Page Set ordering.
- [ ] Reprint existing PDF.
- [ ] Create/rename/delete collection.
- [ ] Add/remove/reorder members.
- [ ] Removing collection membership does not delete a page.
- [ ] Page identity never changes.

## 10.5 Page viewer

Views:

- [ ] Corrected image.
- [ ] Original image.
- [ ] Extracted text.
- [ ] Split view.
- [ ] Metadata.
- [ ] Versions.
- [ ] Annotations.
- [ ] Related pages.
- [ ] Skill results.

Actions:

- [ ] Rename/title.
- [ ] Tags.
- [ ] Correct OCR.
- [ ] Rescan.
- [ ] Export.
- [ ] Run skill.
- [ ] Organize.
- [ ] Trash.

## 10.6 Trash

- [ ] Soft delete.
- [ ] Show consequences.
- [ ] Restore.
- [ ] Permanent-delete confirmation.
- [ ] Never reuse IDs.
- [ ] Offer backup/export before large deletion.

---

# Milestone 11 — OCR and correction

## 11.1 Rust OCR contract

```rust
pub struct OcrRequest {
    pub scan_id: ScanId,
    pub image_path: String,
    pub language_hints: Vec<String>,
}

pub struct OcrResult {
    pub provider: String,
    pub provider_version: String,
    pub full_text: String,
    pub regions: Vec<OcrRegion>,
    pub warnings: Vec<OcrWarning>,
}
```

- [ ] Validate adapter output.
- [ ] Normalize coordinates into canonical page space.
- [ ] Persist status and warnings.
- [ ] Support retry.
- [ ] Represent unavailable confidence distinctly.

## 11.2 Android OCR adapter

- [ ] Select bundled/unbundled ML Kit behavior explicitly.
- [ ] Handle model unavailable.
- [ ] Map blocks, lines, elements, coordinates, languages, and confidence.
- [ ] Return provider/model version where possible.
- [ ] Handle cancellation.
- [ ] Avoid retaining images after request completion.
- [ ] Add known-image tests.

## 11.3 Background OCR queue

- [ ] Queue only after durable scan save.
- [ ] Persist job state.
- [ ] Bounded retry for retryable errors.
- [ ] Permanent failure enters Needs Review.
- [ ] Prevent duplicate OCR after restart.
- [ ] Allow automatic OCR to be disabled.

## 11.4 Correction UI

- [ ] Full-text editing.
- [ ] Region-level correction.
- [ ] Low-confidence highlighting.
- [ ] Tap text to highlight source region.
- [ ] Correction history.
- [ ] Prefer corrected text in search.
- [ ] Preserve original OCR.

Acceptance:

- [ ] OCR failure does not block scanning, saving, or browsing.

---

# Milestone 12 — Local search

## 12.1 Rust-owned FTS

- [ ] Create FTS schema for OCR, corrections, titles, tags, annotations, and Notebook names.
- [ ] Reindex transactionally after correction.
- [ ] Define trashed-content behavior.
- [ ] Add rebuild and integrity-check paths.

## 12.2 Search API

Support text, Notebook, Collection, date, page number, tags, review status, version presence, pagination, and stable sorting.

Return:

```rust
pub struct SearchHit {
    pub page: PageSummary,
    pub excerpt: String,
    pub source: SearchMatchSource,
    pub region: Option<PageRegion>,
    pub score: f32,
}
```

- [ ] Validate/escape FTS input.
- [ ] Return explicit syntax errors if advanced syntax exists.
- [ ] Never return the whole library as a fallback after query failure.

## 12.3 Search UI

- [ ] Search field.
- [ ] Local recent searches.
- [ ] Filter sheet.
- [ ] Excerpts and match source.
- [ ] Open/highlight source region.
- [ ] Empty/error states.

## 12.4 Scale tests

- [ ] Generate a 10,000-page fixture library.
- [ ] Measure index creation, common query latency, correction reindex, and memory usage.

---

# Milestone 13 — Manual backup, restore, and export

## 13.1 Define `.atnb`

Create a versioned archive manifest describing format, version, backup ID, timestamp, installation, encryption, database snapshot, assets, sizes, and SHA-256 hashes.

- [ ] Define canonical paths.
- [ ] Prevent traversal.
- [ ] Define size/count limits.
- [ ] Define compatibility rules.
- [ ] Add golden fixtures.
- [ ] Document excluded secrets.

## 13.2 Encryption

- [ ] Use reviewed Argon2id parameters.
- [ ] Use authenticated encryption such as XChaCha20-Poly1305.
- [ ] Generate unique random salt and nonce material.
- [ ] Authenticate format metadata.
- [ ] Zeroize sensitive keys where practical.
- [ ] Return wrong password/authentication distinctly.
- [ ] Never emit plaintext if encryption was requested.
- [ ] Add known-answer and tamper tests.

## 13.3 Create backup

- [ ] Acquire consistent DB snapshot.
- [ ] Stream files and hashes.
- [ ] Write temporary archive.
- [ ] Finalize encryption.
- [ ] Reopen and verify.
- [ ] Return only verified output.
- [ ] Record success after system save/copy confirmation where possible.
- [ ] Clean temporary files safely.

## 13.4 Android backup UI

- [ ] Backup hub.
- [ ] Library size/object summary.
- [ ] Password/recovery-key setup.
- [ ] Confirmation.
- [ ] System destination picker.
- [ ] Progress/cancel.
- [ ] Completion and last-backup record.
- [ ] Backup reminder settings.
- [ ] Disk-space and picker-cancellation states.

## 13.5 Inspect and restore

- [ ] Select and stage `.atnb`.
- [ ] Inspect version.
- [ ] Authenticate/decrypt.
- [ ] Verify manifest/hashes.
- [ ] Estimate required space.
- [ ] Show date and contents.
- [ ] Choose Replace or Merge.
- [ ] Do not mutate current library during inspection.

## 13.6 Replace restore

- [ ] Restore into a new directory.
- [ ] Verify database/assets.
- [ ] Atomically switch active library.
- [ ] Preserve prior library until success.
- [ ] Support rollback after verification failure.

## 13.7 Merge restore

- [ ] Import immutable IDs idempotently.
- [ ] Detect same ID/same hash.
- [ ] Treat same ID/different immutable content as an integrity conflict.
- [ ] Merge safe additive metadata.
- [ ] Preserve conflicting corrections.
- [ ] Create ReviewItems for unresolved conflicts.
- [ ] Never select a winner silently.

## 13.8 Exporters

Implement:

- [ ] Original images.
- [ ] Corrected images.
- [ ] Markdown with stable citations.
- [ ] Plain text.
- [ ] JSON metadata.
- [ ] Searchable PDF when quality permits.
- [ ] Complete backup.

- [ ] Stream large exports.
- [ ] Preserve ordering and provenance.
- [ ] Never alter source records on export failure.

## 13.9 Failure tests

- [ ] Wrong password.
- [ ] Truncated archive.
- [ ] Modified ciphertext/asset.
- [ ] Traversal entry.
- [ ] Resource-exhaustion archive.
- [ ] Unsupported version.
- [ ] Insufficient space.
- [ ] Cancellation/process death.
- [ ] Merge conflicts.

Acceptance:

- [ ] Round-trip preserves all identities, originals, text, and version relationships.
- [ ] Failed restore leaves the old library usable.

---

# Milestone 14 — Model providers and A2D Skills

## 14.1 Model capabilities

```rust
pub enum ModelCapability {
    GenerateText,
    AnalyzeImage,
    CreateEmbedding,
    Rerank,
}
```

- [ ] Define provider metadata and selection policy.
- [ ] Define request limits, timeout, and cancellation.
- [ ] Define local/network trust state.
- [ ] Store secrets only through platform secure-store handles.
- [ ] Never serialize API keys into ordinary domain objects.

## 14.2 Providers

Architecture supports:

- [ ] On-device provider.
- [ ] Local-network OpenAI-compatible endpoint.
- [ ] User-provided cloud provider.
- [ ] Future managed A2D provider (not implemented in v0.1 — non-goal per spec §5).

**The required v0.1 provider is a user-configured local-network OpenAI-compatible endpoint**
(compatible with llama.cpp, Ollama-compatible gateways, LM Studio, and similar user-controlled
services). This is the practical text-generation path v0.1 must ship and test against — it fits
the accountless, local-first product. The on-device and user-provided-cloud paths remain
architecturally supported but are not required production implementations for v0.1.

Provider requirements:

- [ ] User-configurable base URL.
- [ ] User-configurable model name.
- [ ] Optional bearer/API token stored only through an Android Keystore-backed secure-store handle.
- [ ] Explicit connection-test action.
- [ ] Display the exact host, model, and HTTP/HTTPS transport before sending note data.
- [ ] Require explicit user approval before first use with note content and whenever scope materially changes.
- [ ] Enforce timeouts, response-size limits, cancellation, authentication errors, rate-limit errors, malformed-response errors, and unreachable-host errors explicitly.
- [ ] Do not silently fall back to a different provider or model.
- [ ] Do not silently retry against a public endpoint.
- [ ] Restrict requests to the selected pages and fields.

Deterministic test infrastructure (required — CI MUST NOT depend on internet access, a real API
key, or a live LLM):

- [ ] In-process deterministic `MockModelProvider` in Rust for unit and skill-runtime tests.
- [ ] Local fake OpenAI-compatible HTTP server fixture for request/response contract, timeout, cancellation, authentication, malformed-payload, rate-limit, and size-limit tests.
- [ ] Mock providers are not selectable in production builds unless an explicit developer/debug feature is enabled.
- [ ] Tests verify citations and permission enforcement independently of model quality.

This task is not complete with only the mock provider working — it requires one real configurable
local-network OpenAI-compatible implementation *plus* the deterministic test infrastructure above.

## 14.3 Skill manifest

Example:

```yaml
id: extract-action-items
name: Extract Action Items
version: 1.0.0
runtime: llm-tool-workflow
permissions:
  - pages.read_text
  - pages.create_annotation
model_requirements:
  - generate_text
network: provider-only
mutation_policy: proposal
```

- [ ] Strict versioned schema.
- [ ] Permissions.
- [ ] Model requirements.
- [ ] Network declaration.
- [ ] Resource limits.
- [ ] Signature/trust extension.
- [ ] Reject unsupported required fields/version.

## 14.4 Permission enforcement in Rust

- [ ] Read-only default.
- [ ] Scope by selected pages/Notebook/Collection.
- [ ] Network separately granted.
- [ ] Mutations become proposals.
- [ ] External actions require confirmation.
- [ ] Permission revocation.
- [ ] Per-run effective permission snapshot.
- [ ] Audit denied and approved tool calls.
- [ ] Privilege-escalation tests.

## 14.5 Narrow tools

Implement only explicit tools such as:

- [ ] `pages.search`
- [ ] `pages.read_metadata`
- [ ] `pages.read_text`
- [ ] `pages.read_image`
- [ ] `pages.create_annotation`
- [ ] `pages.add_tag`
- [ ] `collections.create`
- [ ] `exports.create`
- [ ] `model.generate_text`
- [ ] `model.analyze_image` when supported

No generic SQL, file, shell, or unrestricted network tool.

## 14.6 Prompt-injection controls

- [ ] Mark notebook content as untrusted.
- [ ] Separate policy, skill instructions, user request, retrieved data, and tool results.
- [ ] Deny permission changes originating from notebook content.
- [ ] Deny arbitrary URLs unless explicitly granted and allowlisted by policy.
- [ ] Add malicious-note and exfiltration fixtures.
- [ ] Ensure model output cannot directly mutate data.

## 14.7 Built-in skills

Deterministic:

- [ ] Export Selected Pages to Markdown with citations and explicit missing-OCR handling.

Deterministic with optional model explanation:

- [ ] Compare Two Scans of One Page. Deterministic image alignment, difference regions,
      fingerprints, and scan metadata are the authoritative comparison output. A configured
      model MAY optionally explain likely changes in user-friendly language, but MUST NOT
      invent differences or replace the deterministic result. The result identifies:
  - [ ] The two `ScanId` values.
  - [ ] Alignment/registration status.
  - [ ] Changed-region coordinates.
  - [ ] Quality differences.
  - [ ] Whether the comparison is complete, degraded, or inconclusive.
  - [ ] Any model-generated interpretation, labeled as a separate derived result.
  - [ ] Open the visual comparison from the page-version workflow (Milestone 9.5).
  - [ ] Preserve both original scans regardless of comparison outcome.

Configured-model skills:

- [ ] Summarize selected pages.
- [ ] Extract proposed action items.
- [ ] Find related pages.
- [ ] Ask My Notes with citations — a built-in, non-removable, read-only system skill backed by
      the same permissioned skill runtime as any other model skill (spec §7.10/§21). Default
      permissions are limited to the selected scope and normally only `pages.search`,
      `pages.read_metadata`, `pages.read_text`, `model.generate_text`, plus `pages.read_image`/
      `model.analyze_image` only when the user explicitly enables image use. It has no mutation
      permission and does not itself produce a mutation proposal; any follow-up action (tags,
      annotations, tasks, collections, exports, external actions) requires its own separately
      permissioned proposal. Being built in MUST NOT bypass permission checks or auditing.

Every model skill:

- [ ] Shows scope.
- [ ] Shows whether data leaves device.
- [ ] Preserves citations.
- [ ] Marks inference/low confidence.
- [ ] Saves only after appropriate user action.
- [ ] Never silently creates external tasks.

## 14.8 Skills UI

- [ ] Skills hub.
- [ ] Skill details and permissions.
- [ ] Scope selector.
- [ ] Run progress/cancel.
- [ ] Proposal review.
- [ ] Approve/edit/reject.
- [ ] Skill History.
- [ ] Permission revocation.
- [ ] Model-not-configured state.

Acceptance:

- [ ] A malicious page cannot grant network or mutation permission.
- [ ] Ask My Notes returns navigable page citations.

---

# Milestone 15 — iOS readiness

The iOS UI is not implemented in v0.1, but the shared core must be genuinely portable.

- [ ] Generate Swift bindings in CI.
- [ ] Compile a minimal Swift harness where the environment permits.
- [ ] Verify enums and errors map acceptably.
- [ ] Avoid Kotlin-specific types in FFI.
- [ ] Avoid Android URIs/paths in domain APIs.
- [ ] Use portable timestamps and binary/string representations.
- [ ] Document future XCFramework packaging.
- [ ] Inventory future adapters: AVFoundation, Files/Photos, print/share, Keychain, OCR, background tasks, notifications, and network provider.
- [ ] Ensure Rust interfaces do not assume Android lifecycle semantics.
- [ ] Provide desktop mock adapters.
- [ ] Keep QR, layouts, backups, migrations, search rules, and skill permissions platform-independent.

Acceptance:

- [ ] Adding iOS requires presentation/platform adapters, not a canonical-data redesign.

---

# Milestone 16 — Security, diagnostics, and failure hardening

## 16.1 Diagnostics

- [ ] Structured logs and correlation IDs.
- [ ] Redact note text, API keys, passwords, and recovery keys.
- [ ] Diagnostic export excludes note content by default.
- [ ] Optional user-selected support data.
- [ ] Record processing/model versions and review resolutions.

## 16.2 Input hardening

- [ ] QR length limits.
- [ ] Image pixel/dimension limits.
- [ ] PDF page/count limits.
- [ ] Archive entry/count/size limits.
- [ ] Path canonicalization.
- [ ] Parameterized SQL.
- [ ] Skill manifest parser limits.
- [ ] Model response limits.
- [ ] Network timeout/redirect policy.

## 16.3 Dependency review

- [ ] Pin native marker dependency.
- [ ] Record its license.
- [ ] Pin UniFFI.
- [ ] Review PDF, QR, image, crypto, and OCR licenses.
- [ ] Configure `cargo-deny` and vulnerability checks.
- [ ] Document dependency update procedure.
- [ ] Require fixture review before portable-format dependency changes.

## 16.4 Library integrity check

Implement an explicit non-destructive check for:

- [ ] Foreign keys.
- [ ] Schema version.
- [ ] Referenced asset existence.
- [ ] Optional full asset hashes.
- [ ] Orphans.
- [ ] Search-index consistency.
- [ ] Review report.
- [ ] No automatic destructive repair.

## 16.5 Failure injection

Automate:

- [ ] Disk full.
- [ ] DB locked/corrupt.
- [ ] Missing asset.
- [ ] Image decoder failure.
- [ ] Native detector failure.
- [ ] OCR unavailable.
- [ ] Model timeout.
- [ ] Backup/restore interruption.
- [ ] Process death.
- [ ] Permission denial.
- [ ] User cancellation.

Acceptance:

- [ ] Every injected failure has an explicit expected result.
- [ ] “The app did not crash” is never the only success criterion.

---

# Milestone 17 — Physical print validation

## 17.1 Proof assets

- [ ] Generate first bound-notebook interior PDF.
- [ ] Generate cover draft.
- [ ] Generate Smart Page printer test pack.
- [ ] Include marker-size variants when needed.
- [ ] Record exact design/layout versions.

## 17.2 Home-printer matrix

Test US Letter and A4, Actual Size and Fit, grayscale, varied paper/toner, and several Android phones.

Record marker detection, QR detection, geometric error, OCR quality, capture time, and failure reason.

## 17.3 KDP proof matrix

Test front/middle/back pages, gutter markers, writing instruments, lighting, hand shadows, and common capture angles.

- [ ] Measure trim/gutter variation.
- [ ] Change released layouts only by creating a new version.
- [ ] Save representative proof fixtures where licensing/privacy permits.

## 17.4 Evidence-based thresholds

Define after measurement:

- [ ] Marker success rate.
- [ ] QR success rate.
- [ ] Maximum geometry error.
- [ ] Minimum effective resolution.
- [ ] Blur/glare thresholds.
- [ ] Supported device floor.
- [ ] Known unsupported conditions.

Do not invent thresholds before testing.

---

# Milestone 18 — UX completeness and accessibility

- [ ] Implement all v0.1 screens in the spec.
- [ ] Use A2D terminology consistently.
- [ ] Do not show “KDP notebook” or “AprilTag” in normal UI.
- [ ] Always show scan destination.
- [ ] Always show warning/review state.
- [ ] Distinguish original image, OCR, corrected text, and AI output.
- [ ] Provide empty/loading/error/retry states.
- [ ] Add screen-reader labels.
- [ ] Support dynamic type/font scaling.
- [ ] Meet contrast and touch-target requirements.
- [ ] Do not rely on color alone.
- [ ] Add configurable haptic/audio capture confirmation.
- [ ] Show last backup and changed-page count.
- [ ] Keep manual backup visible without a paid-sync dark pattern.
- [ ] Explain external-model scope and provider before upload.
- [ ] Never prompt for an account in core workflows.

---

# Milestone 19 — Release validation

## 19.1 Automated checks

- [ ] Rust format, clippy, and tests.
- [ ] Android lint, unit, and instrumentation tests.
- [ ] Kotlin and Swift binding generation.
- [ ] Backup compatibility fixtures.
- [ ] QR/layout fixtures.
- [ ] Native ABI builds.
- [ ] Dependency/license checks.

## 19.2 Manual acceptance walkthrough

- [ ] Fresh accountless install.
- [ ] Register two identical physical Notebooks.
- [ ] Scan page 1 into each and verify separation.
- [ ] Block or explicitly reassign wrong-design scan.
- [ ] Batch scan out of order.
- [ ] Generate one unique Smart Page.
- [ ] Generate a 20-page Page Set.
- [ ] Print and scan generated pages.
- [ ] Rescan with added writing and preserve old original.
- [ ] Correct OCR.
- [ ] Search and open highlighted source.
- [ ] Create encrypted backup.
- [ ] Restore using Replace.
- [ ] Restore using Merge with conflict.
- [ ] Export Markdown, images, and JSON.
- [ ] Run deterministic skill.
- [ ] Run model skill with citations.
- [ ] Deny unauthorized skill action.
- [ ] Complete core workflow offline.

## 19.3 Release blockers

Do not release with:

- [ ] Known silent data loss.
- [ ] Unhandled migration failure.
- [ ] Unverified backup restore.
- [ ] QR protocol without golden vectors.
- [ ] Printed markers not physically tested.
- [ ] Skill permissions enforced only in Kotlin.
- [ ] API keys outside secure storage.
- [ ] Scanner claiming success before durable Rust registration.
- [ ] Ordinary FFI panics.
- [ ] Core behavior dependent on A2D servers.

## 19.4 Documentation

- [ ] Build/run/test instructions.
- [ ] Android native build setup.
- [ ] Binding generation.
- [ ] Migration policy.
- [ ] QR/layout compatibility policy.
- [ ] Backup format/version policy.
- [ ] Fixture contribution requirements.
- [ ] Physical validation procedure.
- [ ] Verify all referenced files exist at exact paths.

---

# Post-v0.1 backlog — explicitly deferred

- [ ] Production iOS SwiftUI client.
- [ ] A2D Sync account/subscription service.
- [ ] End-to-end encrypted multi-device sync.
- [ ] Managed A2D AI.
- [ ] Public skill marketplace.
- [ ] Signed community skills.
- [ ] Third-party Notebook Design SDK.
- [ ] Additional page sizes and landscape.
- [ ] Advanced handwriting and mathematical OCR.
- [ ] Diagram understanding.
- [ ] Two-page spread scanning.
- [ ] Mesh/spine dewarping.
- [ ] Desktop scanning stand mode.
- [ ] Web viewer.
- [ ] Collaboration.
- [ ] External task/calendar/email integrations.

---

# Ralph-loop recommended slices

## Slice A — Bootstrap

Milestones 1 and 2.

Exit: Android displays a typed value returned by Rust; CI is green; Swift bindings generate.

## Slice B — Durable local core

Milestone 3 plus core Notebook/Page persistence.

Exit: accountless library can be created, closed, reopened, and migration-tested without silent recovery.

## Slice C — Identity and printable pages

Milestones 4 and 5 plus Smart Page UI.

Exit: generated PDF pages round-trip through QR and marker fixtures.

## Slice D — Scanner foundation

Milestone 7 plus live CameraX analysis.

Exit: Android displays marker/Page Code guidance from shared native analysis.

## Slice E — Durable scanning

Remaining Milestone 8 and all of Milestone 9.

Exit: single and batch scans are durably registered with safe versioning.

## Slice F — Library, OCR, and search

Milestones 10–12.

Exit: user can browse, correct, and search page-linked text.

## Slice G — Data ownership

Milestone 13.

Exit: encrypted backup/restore and exports pass round-trip and failure tests.

## Slice H — Skills

Milestone 14 plus relevant security tests.

Exit: deterministic and configured-model skills run with enforced permissions and citations.

## Slice I — Cross-platform readiness and release hardening

Milestones 15–19.

Exit: v0.1 acceptance criteria and physical proof tests are complete.

---

# Final completion checklist

- [ ] Every v0.1 acceptance criterion in the spec is satisfied.
- [ ] Completed tasks contain implementation and tests, not placeholders.
- [ ] Rust remains authoritative for canonical data and business logic.
- [ ] Kotlin remains presentation and Android platform integration.
- [ ] Swift bindings generate without redesigning Rust APIs.
- [ ] Core product requires no account or A2D server.
- [ ] Manual backup and restore are reliable.
- [ ] Original scan data is never silently lost.
- [ ] Every degraded path is visible and reviewable.
- [ ] All referenced files are present at exact repository paths.
