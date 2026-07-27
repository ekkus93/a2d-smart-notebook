from pathlib import Path


TODO_PATH = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")


def replace_exactly_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected exactly one {label}; found {count}")
    return text.replace(old, new, 1)


def main() -> None:
    text = TODO_PATH.read_text()

    text = replace_exactly_once(
        text,
        "**Status:** Ready for implementation planning and Ralph-loop execution",
        "**Status:** Milestones 1–6 implemented; Milestone 7 is next",
        "old overall status marker",
    )
    text = replace_exactly_once(
        text,
        "**Date:** 2026-07-26",
        "**Date:** 2026-07-27",
        "old TODO date",
    )

    old_layout = """- [ ] Generate blank verso pages. Deferred to Milestone 5.4 (PDF renderer) — this task defines
      layout geometry, not PDF bytes; there's nothing to \"generate\" yet without a renderer.
- [ ] Generate a complete proof interior PDF. Same as above — Milestone 5.4's job once the PDF
      renderer exists; this task's layouts are what it will render from."""
    new_layout = """- [x] Generate blank verso pages. `a2d_pdf::generate_notebook_proof_interior_pdf` emits a
      blank verso after the Setup Page and after every logical-page recto.
- [x] Generate a complete proof interior PDF. The Rust proof generator creates the Setup Page,
      blank versos, writable rectos, deterministic logical numbering, and verified output bytes."""
    text = replace_exactly_once(
        text,
        old_layout,
        new_layout,
        "stale Milestone 5.3 marker block",
    )

    start_marker = "# Milestone 6 — Notebook and Smart Page workflows"
    end_marker = "\n---\n\n# Milestone 7 — Marker detection and image-processing foundation"
    start = text.index(start_marker)
    end = text.index(end_marker, start)

    milestone = """# Milestone 6 — Notebook and Smart Page workflows

**Status:** Complete — 2026-07-27

## 6.1 Rust Notebook service

Implemented in `crates/a2d-core/src/milestone6.rs`, with typed persistence operations in
`crates/a2d-storage` and UniFFI projections in `crates/a2d-ffi/src/milestone6.rs`.

- [x] `resolve_notebook_setup_code`
- [x] `create_notebook`
- [x] `rename_notebook`
- [x] `archive_notebook`
- [x] `list_notebooks`
- [x] `get_notebook`
- [x] `set_active_notebook`
- [x] `get_active_notebook`

Rules:

- [x] Multiple notebooks may share one design. Every registration mints a fresh `NotebookId` and
      an independent set of persistent logical-page identities.
- [x] Names need not be unique. Persistence and lookup use typed identifiers, not display names.
- [x] IDs are unique through cryptographic ID generation and database primary-key constraints.
- [x] Active Notebook is explicit persistent state. Migration 0003 adds a partial unique index so
      at most one non-archived Notebook can be the active scan destination.
- [x] The UI never silently changes the active Notebook. Selection and clearing are explicit user
      actions, and conflicts are returned as typed Rust results.

Notebook creation is transactional: the design, physical Notebook, logical page slots, and optional
active selection commit together or roll back together. Tests cover separate physical copies of one
design, duplicate display names, active-selection persistence, archiving, and rollback.

## 6.2 Page resolution

`A2dCore::resolve_page_code` owns the identity and destination rules. Android sends decoded text to
Rust and displays the typed `PageResolution`; it does not duplicate the QR grammar or infer a
Notebook locally.

- [x] Resolve a Smart Page by unique ID.
- [x] Resolve a Notebook Page only through a matching Notebook Design.
- [x] Return `RequiresNotebookSelection` when several physical Notebooks match and none is
      explicitly confirmed.
- [x] Return `ConflictingActiveNotebook` when the confirmed or active Notebook uses another design.
- [x] Return `ImportedUnknownSmartPage` for a valid but locally unknown Smart Page.
- [x] Never auto-create a physical Notebook from an ordinary Page Code.
- [x] Return `RequiresNotebookRegistration` when the design is recognized but no physical Notebook
      instance exists.
- [x] Return `UnsupportedCode` for Setup Codes used as page identifiers, unavailable designs,
      invalid logical page numbers, and incompatible layouts.

The variants cross UniFFI without being flattened into untyped success strings. Domain, storage,
core, and FFI tests cover resolved, ambiguous, conflicting, registration, unknown-import, and
unsupported outcomes.

## 6.3 Android Notebook UI

Implemented under `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/notebook/`.

- [x] Add a Notebook Setup Code scanner. Android captures a bounded still image, ZXing decodes QR
      text locally, and Rust performs canonical validation.
- [x] Notebook Design recognized screen.
- [x] Name/customize Notebook with optional color, icon, notes, and active-selection choice.
- [x] Created confirmation and first-page Page Code resolution action. Full-page camera capture
      remains correctly scoped to Milestones 8 and 9.
- [x] Unsupported/invalid Setup Code state remains visible as a typed Rust error.
- [x] Multiple-copy explanation and explicit “add another copy” path.
- [x] Active Notebook selector, clear action, rename, and archive controls.

`NotebookViewModel` owns presentation state and dispatch only. Identity, persistence, ambiguity,
conflict, and creation rules delegate to `A2dClient`. Camera and QR failures are surfaced explicitly;
they are never converted into successful scans.

## 6.4 Smart Page UI

Implemented under `apps/android/app/src/main/kotlin/com/a2d/notebook/feature/smartpage/`.

- [x] Create Smart Pages landing screen.
- [x] Single-page form.
- [x] Page-set form with a bounded 1–500 page count.
- [x] PDF preview using Android `PdfRenderer`.
- [x] Android print, Storage Access Framework save-copy, and content-URI share integration.
- [x] Generated page/set detail showing the Page Set ID and unique page-identity count.
- [x] Failed generation state with explicit retry. Retry calls Rust generation again, so a failed
      attempt never reuses identities or presents an incomplete output as successful.

`SmartPagesViewModel` delegates generation, identity creation, PDF registration, and validation to
Rust. Android owns only form presentation, preview, and platform print/save/share operations.

Acceptance:

- [x] A user can register two identical physical Notebook copies separately. Rust tests and the
      repeated registration flow prove independent Notebook and page identities for one design.
- [x] A user can generate a unique Smart Page offline without an account. The path uses local Rust,
      SQLite, and private asset storage without a network or account service.

Validation evidence:

- GitHub Actions run `30289813553` completed successfully on 2026-07-27.
- GitHub Actions run `30300119881` completed successfully on 2026-07-27.
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo deny check`
- Android lint, unit tests, and debug assembly
- Android NDK cross-build and Kotlin UniFFI binding-drift verification
"""

    text = text[:start] + milestone + text[end:]
    TODO_PATH.write_text(text)


if __name__ == "__main__":
    main()
