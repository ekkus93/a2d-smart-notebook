# 0001. QR v1 wire encoding and integrity check

**Status:** Accepted — the v1 wire grammar and permanent compatibility fixtures are frozen.
Physical printer/camera qualification remains a separate Milestone 17 evidence gate and does not
permit rewriting accepted v1 vectors.
**Date:** 2026-07-26
**Decision owners/authors:** Phillip Chin (product direction, via spec/TODO review responses); grammar drafted by Claude Code from that direction and requires human review before acceptance.

## Context and problem

Spec §14.2 deliberately leaves the QR payload's binary/text encoding and integrity mechanism
open, giving only a conceptual `a2d:1:b:<design-id>:...` sketch and permitting "a compact binary
encoding" as an implementation option. Milestone 4 (identity/QR protocol) needs a single,
concrete, canonical v1 wire format before it can generate golden fixtures — and per this
project's fixture policy, v1 vectors under `fixtures/` are **permanent** once committed. An
under-specified or binary format picked casually here would be difficult to walk back.

## Constraints

- Spec §14.1: versioned, compact, offline-decodable, free of private user content, strictly
  parsed, forward-compatible via explicit unsupported-version errors, integrity-checked,
  non-executable.
- Spec §14.3: parser MUST reject unknown required fields, invalid lengths, invalid alphabets,
  unsupported versions, invalid integrity checks, out-of-range page numbers, oversized payloads,
  and trailing executable/URL content — and MUST NOT reinterpret an invalid A2D code as arbitrary
  web content.
- Spec §14.4: v1 integrity checks detect corruption, not publisher authenticity; the design must
  leave room for a future signed-manifest extension.
- Must round-trip identically through Rust (canonical implementation), Android, and a future iOS
  decoder.
- Must physically print and scan reliably at small marker/QR sizes (spec §11.4 quiet-zone and
  printer-safe-margin constraints).

## Options considered

1. **Custom opaque binary payload**, base64- or base45-encoded for QR embedding. Most
   space-efficient, but requires a bespoke binary decoder on every platform and doesn't benefit
   from QR's native alphanumeric encoding mode.
2. **JSON payload.** Human-readable but verbose, wastes QR capacity, and JSON's flexible grammar
   works against "strictly parsed" and "reject unknown fields."
3. **Canonical uppercase alphanumeric text payload**, hand-rolled grammar, encoded so that
   standard QR alphanumeric mode is used natively by any platform's QR generator/decoder.
   Compact, human-inspectable, requires no special QR framing, and keeps the *interesting* parsing
   logic (field validation, integrity, versioning) in one canonical Rust implementation regardless
   of encoding mode. **Selected.**

## Decision

The v1 QR payload is a single ASCII string using only characters from the QR **alphanumeric**
character set (`0-9`, `A-Z` uppercase, space, `$ % * + - . / :`), so any standard QR encoder
selects alphanumeric mode automatically. No lowercase, no other bytes.

### Grammar

```text
payload      := "A2D" ":" version ":" type-code ":" type-fields ":" crc
version      := "1"                                   ; single ASCII digit, v1 only
type-code    := "S" | "B" | "M"                        ; NotebookSetup | NotebookPage | SmartPage
crc          := 7 x crockford-base32-char              ; CRC-32C over payload up to and
                                                        ; including the ":" preceding crc
id128        := 26 x crockford-base32-char             ; 128-bit ID, MSB-first, zero-padded
absent       := "-"                                    ; canonical "no value" sentinel
```

Field layout per `type-code` (all joined with `:`):

```text
S (NotebookSetup):  A2D:1:S:<design-id:id128>:<crc>
B (NotebookPage):   A2D:1:B:<design-id:id128>:<logical-page-number>:<layout-id>:<crc>
M (SmartPage):      A2D:1:M:<smart-page-id:id128>:<layout-id>:<visible-page-number|absent>:<page-set-id:id128|absent>:<crc>
```

- **`id128`** — a 128-bit identifier encoded as exactly 26 uppercase Crockford Base32 characters
  (alphabet `0123456789ABCDEFGHJKMNPQRSTVWXYZ` — excludes `I`, `L`, `O`, `U`). Strict parsing
  disables Crockford's optional leniency: `I`/`L`/`O` are rejected outright, never normalized to
  `1`/`1`/`0`. Fixed width, zero-padded on the most-significant end — no separators, no Crockford
  check symbol (integrity is handled by the CRC field instead).
- **`logical-page-number` / `visible-page-number`** — canonical decimal ASCII, no leading zeros
  (except the literal value `0`), no sign, bounded `0..=999999`. `visible-page-number` MAY be the
  `absent` sentinel (`-`) when the page has no visible number.
- **`layout-id`** — an uppercase token of 1–20 characters from the fixed layout registry owned by
  `a2d-layout` (spec §12.3/§5.2); the registry is the source of truth, not this ADR. An id-shaped
  but unregistered value is rejected as invalid, not silently accepted.
- **`page-set-id`** — an `id128`, or the `absent` sentinel (`-`) when the Smart Page isn't part of
  a page set.
- **`crc`** — CRC-32C (Castagnoli) computed over the exact ASCII bytes from the payload's first
  byte through the `:` immediately preceding the CRC field (inclusive of that delimiter),
  encoded as exactly 7 uppercase Crockford Base32 characters (32 bits → 7 chars, last char holds
  2 bits, zero-padded).

### Maximum length

Total payload MUST NOT exceed **128 characters**. The longest defined variant (SmartPage with a
full-length layout id and page-set id) is ~93 characters, leaving headroom without inviting
unbounded growth.

### Strict-parser rules

Reject, with a typed error and no partial acceptance, on any of:

- Any lowercase character, or any byte outside the QR alphanumeric set.
- Magic prefix other than exactly `A2D`.
- Unsupported `version` (v1 parsers understand `"1"` only — never guess at a newer version's
  grammar).
- Unknown `type-code`.
- Wrong field count for the resolved type (missing or extra fields), or trailing data after `crc`.
- Any `id128` field that isn't exactly 26 characters of the canonical Crockford alphabet (`I`,
  `L`, `O`, `U` are invalid characters here, not aliases to normalize).
- Any numeric field with a leading zero (other than literal `0`), a sign, non-digit characters, or
  a value out of its bounded range.
- A `layout-id` not present in the current layout registry.
- A recomputed CRC-32C that doesn't match the decoded `crc` field.
- Total length over 128 characters (checked before tokenizing).

A rejected payload MUST NOT be reinterpreted as a URL or as arbitrary web content, per spec §14.3.

### Examples (illustrative only — not golden vectors)

Valid NotebookPage: `A2D:1:B:01ARZ3NDEKTSV4RRFFQ69G5FAV:12:USLETTER-LINED:3KZ8QWY`

Malformed (lowercase, rejected before tokenizing): `a2d:1:b:...`

Malformed (bad CRC, rejected at integrity check):
`A2D:1:S:01ARZ3NDEKTSV4RRFFQ69G5FAV:0000000`

### Android/iOS interoperability rationale

Because the wire format is restricted to QR's alphanumeric character set, both platforms' stock
QR libraries (e.g. ZXing on Android, Vision/AVFoundation on iOS) select QR's alphanumeric
encoding mode automatically and hand back a plain ASCII string — no custom binary QR framing or
platform-specific decoder is needed on either side. The only implementation-specific work is
capturing that decoded string and handing it to the single canonical Rust parser; Rust remains
the sole place field validation, bounds checks, and the CRC live.

### Versioning and future signed-code extension

`version` is a single digit reserved 1–9. A future version (e.g. adding a signed-manifest
extension per spec §14.4) introduces a new `type-code`/field-layout combination under a bumped
`version`; v1 parsers reject unknown versions outright (typed "unsupported version" error) rather
than attempting to partially interpret a newer grammar. This preserves forward-compatibility
without silently downgrading a signed code to an unsigned interpretation.

### Golden-vector format (`fixtures/qr/v1/`)

- `notebook_setup_vectors.json`, `notebook_page_vectors.json`, `smart_page_vectors.json`: JSON
  array of `{ "name": string, "payload_text": string, "decoded": { ...typed fields matching the
  Rust `PageCode` variant... } }`.
- `malformed_vectors.json`: JSON array of `{ "name": string, "payload_text": string,
  "expected_error": string }`, where `expected_error` is a stable error-code string matching one
  of the rejection rules above.
- `rendered/`: QR PNG (or SVG) renders of every valid vector, named to match `name`, decoded by an
  integration test to confirm the rendered image round-trips through a real QR decode step before
  reaching the Rust parser.

## Detailed rationale

An alphanumeric-text grammar over a custom binary payload trades a small amount of QR capacity
for: no bespoke binary decoder on any platform, human-inspectable payloads during debugging and
code review, and a strict, easy-to-audit grammar that maps cleanly onto spec §14.3's explicit
rejection list. Crockford Base32 was chosen for 128-bit IDs specifically because it excludes
visually ambiguous characters and fits entirely within the QR alphanumeric charset — a plain
hex or standard Base32 encoding would need lowercase or padding characters (`=`) that either
don't fit the alphanumeric charset (forcing a less efficient QR encoding mode) or reduce
legibility.

## Security/privacy implications

The payload carries no user content, names, or metadata — only opaque IDs, small enumerated
numbers, and a registry-bound layout token, consistent with spec §14.1's "free of private user
content" requirement. The CRC is explicitly documented (here and in the parser's own error
messages) as corruption detection only, not an authenticity proof — it MUST NOT be presented to
users or code as a signature.

## Portability implications for Android and future iOS

Both platforms need only a generic QR *decoder* (any library that reads QR alphanumeric-mode
text); no platform-specific binary framing code is required. All grammar, validation, and
integrity logic lives once in Rust and crosses the FFI boundary as a typed `PageCode` result.

## Compatibility/fixture implications

This grammar becomes permanent the moment `fixtures/qr/v1/` is committed. A defect discovered
after that point is fixed by incrementing `version`, not by editing v1 fixtures.

## Consequences and tradeoffs

- Slightly larger QR payload than a bespoke binary encoding would produce, in exchange for
  zero custom QR-framing code on Android/iOS.
- The `layout-id` registry becomes an implicit compatibility surface: removing or renaming a
  registered layout id breaks parsing of any already-printed page using it. Layout ids, once
  shipped, are as permanent as the QR grammar itself.
- A future signed-manifest extension requires a version bump, not a v1 grammar change.

## Validation evidence

- [x] Android spike proving a real Android QR decoder library returns this exact canonical
      string from a rendered QR image, for at least one vector of each `type-code`. Done —
      `apps/android/app/src/androidTest/kotlin/com/a2d/notebook/app/QrDecoderSpikeTest.kt`,
      run on the real `Medium_Phone_API_36.0` emulator (not a unit test / not mocked). Rust
      (`a2d-identity::qr::PageCode::encode`, called across the real UniFFI/JNA boundary — not a
      hand-typed fixture) generates a fresh payload per call; ZXing (`com.google.zxing:core`,
      standing in for "a real Android decoder" — not the final production library choice, that's
      still Milestone 7.4/12's job) renders it to a QR bitmap and decodes it back; the test
      asserts byte-for-byte equality. Covers all three `type-code`s (`S`/`B`/`M`), confirms each
      call produces a fresh random id (not a cached/constant value), and confirms the grammar's
      own shape (magic prefix, version, type code, canonical uppercase). 7/7 passing, verified
      via the actual JUnit XML report, not just a build-success exit code.
      **What this does NOT prove**: that ZXing's behavior matches whatever decoder the production
      app eventually ships with (ML Kit's internal decoder isn't guaranteed byte-identical to
      ZXing in every edge case) — treat this as strong evidence the grammar itself is sound, not
      a substitute for testing the actual shipped decoder once one is chosen.
- [x] The worst-case SmartPage payload, including a 20-character layout ID, maximum visible
      page number, and Page Set ID, is rendered inside the real 18mm QR rectangle of the configured
      US Letter Smart Page layout. Pure-Rust PDF rasterization at 95%, 100%, and 105% print scales
      decodes the exact canonical text and detects all four official corner markers within a 2mm
      center tolerance. The permanent fixture suite also decodes every committed valid vector and
      asserts stable errors for malformed vectors.

**Accepted scope.** This evidence freezes the QR v1 wire compatibility surface and confirms the
configured vector PDF geometry survives deterministic rasterization. It does not claim consumer
printer, paper, toner, lighting, camera, or damage-tolerance thresholds; those remain Milestone 17
physical-validation work. Any future wire-format correction requires a new protocol version rather
than editing v1 fixtures.

## Follow-up tasks

- ~~Run the Android decoder spike above; record results here.~~ Done — see Validation evidence.
- Preserve `fixtures/qr/v1/` permanently; correct defects through a new protocol version.
- Keep layout IDs immutable after release because printed Page Codes reference them directly.
- Complete Milestone 17 physical printer/camera qualification without converting synthetic raster
  tolerances into production capture thresholds.

## Superseding ADR reference

None.
