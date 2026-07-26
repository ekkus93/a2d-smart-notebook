# Responses — A2D Smart Notebook v0.1 spec/TODO review

Covers: `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md` and `docs/A2D_SMART_NOTEBOOK_V01_TODO.md`,
reviewed via `/spec-todo`. Fill in each `A:` line and share back before implementation
of the affected milestones (2, 4, 14) begins.

---

## 1

**Q:** Should `ReviewItemId`, `AnnotationId`, and `AuditEventId` be added to Milestone 2.1's
identifier list (and to spec §13's core-types list), matching the entities Milestone 3.1's
schema and 2.3's entity list already require?

**A:**

---

## 2

**Q:** For the QR payload's binary encoding and integrity check (spec §14.2): do you want
this decided via a deliberate write-up *before* Milestone 4 starts (given fixtures become
permanent), rather than picked ad hoc mid-task like the other open decisions? If so, should
I draft that now?

**A:**

---

## 3

**Q:** Should Milestone 14.7's built-in skill set include "compare two scans of one page"
(per spec §21.2), or is it intentionally replaced by "Ask My Notes" — and if intentional,
should the spec be updated to say so (same class of drift as the `PageId`/`CollectionId`
fix already made)?

**A:**

---

## 4

**Q:** Is "Ask My Notes" intended to run through the full A2D Skill permission/proposal/audit
system (as TODO 14.7 implies), or as a separate core search feature outside that machinery
(as spec §7.10's placement suggests)?

**A:**

---

## 5

**Q:** For Milestone 14.2, which model-provider path is the actual v0.1 target — on-device,
local-network OpenAI-compatible endpoint, or user-provided cloud API key — and should a
mock/stub provider be built so it's testable without live network access in CI?

**A:**

---

## 6

**Q:** Do you want a `docs/decisions/` (or similar) ADR directory established now, with a
lightweight template, so the AprilTag decision (Milestone 7.1) and the QR encoding decision
(above) land in a durable, discoverable place rather than only in `memory.md`?

**A:**

---

Fill in the `A:` line under each question above, then share this file back (or paste the
answers) to resume implementation planning.
