# Architecture decision records

This directory holds ADRs for choices the specification (`docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`)
and TODO (`docs/A2D_SMART_NOTEBOOK_V01_TODO.md`) deliberately leave open. Architecture decisions
MUST be recorded here — not only in `memory.md`, chat transcripts, commit messages, or TODO
checkboxes.

## Process

1. Copy `ADR_TEMPLATE.md` to `NNNN-short-slug.md`, using the next sequential four-digit number.
2. Fill in every section. Status starts at `Proposed`.
3. Where a milestone requires an ADR to reach `Accepted` before certain work proceeds (for
   example, before permanent golden fixtures are committed), the relevant TODO task says so
   explicitly and links back here.
4. Once accepted, an ADR's Decision section is treated like any other spec requirement: changing
   it later requires a new ADR that supersedes the old one, not a silent edit.

## Index

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-qr-v1-encoding-and-integrity.md) | Proposed | QR v1 wire encoding and integrity check |
| [0002](0002-apriltag-detector-selection.md) | Proposed | Corner-marker detector selection |
| [0003](0003-qr-image-decoder-boundary.md) | Accepted | Platform QR image decoding with canonical Rust payload validation |
