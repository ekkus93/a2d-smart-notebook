# Responses — A2D Smart Notebook v0.1 spec/TODO review

**Status:** Applied — 2026-07-26  
**Date:** 2026-07-26  
**Applies to:** `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md` and `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`

All seven required edits below have been applied to the spec, TODO, and `docs/decisions/`. See
`memory.md` for a session summary. ADR 0001 (QR encoding) and ADR 0002 (AprilTag) are both
`Proposed`, not `Accepted` — each still requires the validation evidence its own file describes
before the milestones that depend on it (4 and 7 respectively) may proceed past that point.

These answers are authoritative for v0.1 implementation. Update the specification and TODO where directed below before beginning the affected work. Do not leave contradictory requirements in the documents.

---

## 1

**Q:** Should `ReviewItemId`, `AnnotationId`, and `AuditEventId` be added to Milestone 2.1's identifier list (and to spec §13's core-types list), matching the entities Milestone 3.1's schema and 2.3's entity list already require?

**A:** Yes.

Add opaque, strongly typed `ReviewItemId`, `AnnotationId`, and `AuditEventId` types to both spec §13 and TODO Milestone 2.1.

Apply the same rule consistently to all independently persisted and addressable domain entities. Because the current design stores text regions and text corrections as independent records, also add:

- `TextRegionId`
- `TextCorrectionId`

The resulting rule is:

> Every independently persisted entity that has its own row, can be referenced by another record, appears in provenance, or can cross the FFI boundary MUST use a dedicated opaque identifier type rather than a raw string or another entity's identifier type.

Do not add identifiers to embedded value objects that have no independent persistence or identity. Constructors, parsing, canonical serialization, FFI mapping, database constraints, and tests must follow the same requirements already specified for the existing identifier types.

Required document changes:

1. Update spec §13's core identifier list.
2. Update TODO Milestone 2.1.
3. Ensure Milestones 2.3 and 3.1 use the new types consistently.

---

## 2

**Q:** For the QR payload's binary encoding and integrity check (spec §14.2): do you want this decided via a deliberate write-up *before* Milestone 4 starts (given fixtures become permanent), rather than picked ad hoc mid-task like the other open decisions? If so, should I draft that now?

**A:** Yes. Draft and accept an architecture decision record before Milestone 4 creates permanent v1 fixtures.

The v0.1 direction is:

- Use a canonical **QR alphanumeric text payload**, not a custom opaque binary payload.
- Use uppercase ASCII only from the QR alphanumeric character set.
- Use an explicit `A2D` magic prefix, protocol version, code type, canonical ordered fields, and integrity field.
- Encode 128-bit identifiers using canonical Crockford Base32 without optional aliases or ambiguous-character normalization during strict parsing.
- Encode numeric fields in one documented canonical representation with bounds checks and no alternative spellings.
- Use **CRC-32C** over the complete canonical payload bytes before the integrity field.
- Encode the CRC in canonical uppercase Base32.
- Reject lowercase, noncanonical encodings, omitted required fields, extra fields, trailing data, bad CRC, unsupported versions, oversized payloads, and out-of-range values.
- Treat the CRC only as corruption detection. It does not establish publisher authenticity.
- Do not make the payload an `http:` or `https:` URL, and do not open malformed A2D content as a URL.

The ADR must specify:

1. Exact grammar and delimiters.
2. Code-type discriminators for `NotebookSetup`, `NotebookPage`, and `SmartPage`.
3. Canonical encoding of every field.
4. CRC coverage and byte representation.
5. Maximum payload length.
6. Strict-parser rules.
7. Examples and malformed examples.
8. Android and future-iOS decoder interoperability rationale.
9. Versioning and future signed-code extension strategy.
10. The exact golden-vector format committed under `fixtures/qr/v1/`.

Before finalizing the ADR, implement a small spike proving that the selected Android QR decoder returns the canonical payload reliably from rendered fixtures. The canonical parser and encoder remain in Rust.

Do not commit v1 golden fixtures until this ADR is accepted. After fixtures are committed, changing the v1 wire format requires a new protocol version rather than rewriting the fixtures.

---

## 3

**Q:** Should Milestone 14.7's built-in skill set include "compare two scans of one page" (per spec §21.2), or is it intentionally replaced by "Ask My Notes" — and if intentional, should the spec be updated to say so?

**A:** Include both. `Ask My Notes` is additive and does not replace scan comparison.

Update TODO Milestone 14.7 to include:

- `Compare Two Scans of One Page`

The feature should use deterministic image alignment, difference regions, fingerprints, and scan metadata as its authoritative inputs. A configured model may optionally explain likely changes in user-friendly language, but the model must not invent differences or replace the deterministic comparison output.

The result must identify:

- The two `ScanId` values.
- Alignment/registration status.
- Changed-region coordinates.
- Quality differences.
- Whether the comparison is complete, degraded, or inconclusive.
- Any model-generated interpretation as a separately labeled derived result.

The user must be able to open the visual comparison from the page-version workflow. Preserve both original scans regardless of comparison outcome.

Required document change: add this skill to TODO Milestone 14.7 so it matches spec §21.2.

---

## 4

**Q:** Is "Ask My Notes" intended to run through the full A2D Skill permission/proposal/audit system (as TODO 14.7 implies), or as a separate core search feature outside that machinery (as spec §7.10's placement suggests)?

**A:** It is a first-class Search user experience implemented through the same secured A2D Skill runtime.

In the UI, `Ask My Notes` belongs under Search and may have a dedicated screen. Internally, it is a built-in, signed/trusted, read-only system skill. It MUST use the same enforcement path as other model-backed skills for:

- Explicit notebook/page/collection scope.
- Effective permission calculation in Rust.
- Provider and endpoint disclosure.
- Confirmation before selected note content leaves the device.
- Prompt-injection separation.
- Request limits, cancellation, and timeouts.
- Source-page citations.
- Low-confidence and inference labeling.
- Model/provider/version provenance.
- Audit events.

Its default permissions are limited to the selected scope and normally include only:

- `pages.search`
- `pages.read_metadata`
- `pages.read_text`
- `pages.read_image` only when the user enables image use and the provider supports it
- `model.generate_text`
- `model.analyze_image` only when explicitly selected

`Ask My Notes` has no mutation permission and therefore does not normally produce a mutation proposal. If a future follow-up action proposes tags, annotations, tasks, collections, exports, or external actions, that follow-up must become a separately permissioned proposal.

The built-in system skill may be non-removable, but it must not bypass permission checks or auditing merely because it is built in.

Required document clarification: spec §7.10 should state that the Search UI invokes the shared built-in skill/model runtime rather than a parallel ungoverned LLM path.

---

## 5

**Q:** For Milestone 14.2, which model-provider path is the actual v0.1 target — on-device, local-network OpenAI-compatible endpoint, or user-provided cloud API key — and should a mock/stub provider be built so it's testable without live network access in CI?

**A:** The required practical v0.1 provider is a **user-configured local-network OpenAI-compatible endpoint**.

This target fits the accountless, local-first core product and allows use with llama.cpp, Ollama-compatible gateways, LM Studio, and other user-controlled services where they expose the required OpenAI-compatible API.

v0.1 provider requirements:

- User-configurable base URL.
- User-configurable model name.
- Optional bearer/API token stored only through an Android Keystore-backed secure-store handle.
- Explicit connection-test action.
- Display the exact host, model, and whether transport is HTTP or HTTPS before sending note data.
- Require explicit user approval before first use of a provider with note content and whenever scope materially changes.
- Enforce timeouts, response-size limits, cancellation, authentication errors, rate-limit errors, malformed-response errors, and unreachable-host errors explicitly.
- Do not silently fall back to a different provider or model.
- Do not silently retry against a public endpoint.
- Restrict requests to the selected pages and fields.

The provider abstraction must still accommodate on-device and user-provided cloud providers later, but those are not required production implementations for v0.1. A manually configured HTTPS OpenAI-compatible endpoint may technically be remote, but v0.1 product documentation and acceptance testing should focus on a user-controlled local-network endpoint.

Yes, build deterministic test providers. Required test infrastructure:

1. An in-process deterministic `MockModelProvider` in Rust for unit and skill-runtime tests.
2. A local fake OpenAI-compatible HTTP server fixture for request/response contract, timeout, cancellation, authentication, malformed payload, rate-limit, and size-limit tests.
3. CI must never require internet access, a real API key, or a live LLM.
4. Mock providers must not be selectable in production builds unless an explicit developer/debug feature is enabled.
5. Tests must verify citations and permission enforcement independently of model quality.

A task is not complete merely because only the mock provider works. Milestone 14.2 requires one real configurable local-network OpenAI-compatible implementation plus the deterministic test infrastructure.

---

## 6

**Q:** Do you want a `docs/decisions/` (or similar) ADR directory established now, with a lightweight template, so the AprilTag decision (Milestone 7.1) and the QR encoding decision (above) land in a durable, discoverable place rather than only in `memory.md`?

**A:** Yes. Establish `docs/decisions/` now.

Create:

```text
docs/decisions/
├── README.md
├── ADR_TEMPLATE.md
├── 0001-qr-v1-encoding-and-integrity.md
└── 0002-apriltag-detector-selection.md
```

The numbering may continue sequentially for later decisions. Do not store architecture decisions only in `memory.md`, chat transcripts, commit messages, or TODO checkboxes.

Each ADR must include:

- Status: Proposed, Accepted, Superseded, or Rejected.
- Date.
- Decision owners/authors.
- Context and problem.
- Constraints.
- Options considered.
- Decision.
- Detailed rationale.
- Security/privacy implications.
- Portability implications for Android and future iOS.
- Compatibility/fixture implications.
- Consequences and tradeoffs.
- Validation evidence.
- Follow-up tasks.
- Superseding ADR reference when applicable.

`0001-qr-v1-encoding-and-integrity.md` must be accepted before permanent v1 QR fixtures are committed.

`0002-apriltag-detector-selection.md` must be accepted at the end of Milestone 7.1 and must record license review, Android ABI build results, desktop fixture results, performance measurements, memory-safety boundary, packaging strategy, and future iOS feasibility.

Update the repository-structure section of the spec and the relevant TODO milestones to reference the ADR directory as implementation output. These are repository documents that Claude Code must create and commit; they are not external prerequisite files.

---

## Required follow-up before affected milestones

Before beginning Milestones 2, 4, 7, or 14, apply the document corrections identified above so the spec, TODO, and ADRs agree.

Minimum required edits:

1. Add all required persistent entity ID types to spec §13 and TODO 2.1.
2. Clarify `Ask My Notes` as a Search UI backed by the shared built-in skill runtime.
3. Add scan comparison to TODO 14.7.
4. Make the local-network OpenAI-compatible provider the required v0.1 provider in TODO 14.2.
5. Add deterministic provider test infrastructure requirements.
6. Establish `docs/decisions/` and accept the QR ADR before committing QR v1 fixtures.
7. Record the AprilTag implementation decision in its own ADR.

After those edits, implementation may continue without another clarification round for these questions.